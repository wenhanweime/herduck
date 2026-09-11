use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use super::{bump_revision, CatalogError, ProjectCatalog};
use crate::projects::{TopicCover, TopicCoverPatch};

pub(super) const CREATE_TOPIC_COVERS_SQL: &str = "
    CREATE TABLE IF NOT EXISTS topic_covers (
        topic_key TEXT PRIMARY KEY REFERENCES projects(canonical_key),
        goal TEXT NOT NULL DEFAULT '',
        next_steps TEXT NOT NULL DEFAULT '[]',
        blocked_note TEXT NOT NULL DEFAULT '',
        blocked_session_ref TEXT,
        updated_at INTEGER NOT NULL DEFAULT 0
    );";

impl ProjectCatalog {
    pub(super) fn migrate_v6_to_v7(&mut self) -> Result<(), CatalogError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(CREATE_TOPIC_COVERS_SQL)?;
        transaction.pragma_update(None, "user_version", 7)?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn topic_cover(&self, topic_key: &str) -> Result<TopicCover, CatalogError> {
        require_topic(&self.connection, topic_key)?;
        Ok(read_cover(&self.connection, topic_key)?.unwrap_or_default())
    }

    pub(super) fn saved_topic_cover(
        &self,
        topic_key: &str,
    ) -> Result<Option<TopicCover>, CatalogError> {
        read_cover(&self.connection, topic_key)
    }

    /// Preserve the authored Topic even when reclassification moves its last conversation.
    /// Covers are never copied to another Topic or merged by a model.
    pub(super) fn topics_with_only_a_cover(&self) -> Result<Vec<(String, String)>, CatalogError> {
        let mut statement = self.connection.prepare(
            "SELECT p.canonical_path, p.display_name
             FROM topic_covers c JOIN projects p ON p.canonical_key = c.topic_key
             WHERE p.kind = 'semantic' AND NOT EXISTS (
                SELECT 1 FROM semantic_assignments sa JOIN sessions s ON s.stable_key = sa.session_key
                WHERE sa.topic_key = p.canonical_path AND s.session_class = 'interactive'
             ) ORDER BY c.updated_at DESC, c.topic_key ASC",
        )?;
        let topics = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(topics)
    }

    pub(crate) fn update_topic_cover(
        &mut self,
        topic_key: &str,
        patch: &TopicCoverPatch,
        observed_at: i64,
    ) -> Result<u64, CatalogError> {
        patch.validate().map_err(CatalogError::InvalidTopicCover)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_topic(&transaction, topic_key)?;
        let mut cover = read_cover(&transaction, topic_key)?.unwrap_or_default();
        if patch
            .expected_updated_at
            .is_some_and(|expected| expected != cover.updated_at)
        {
            return Err(CatalogError::TopicCoverConflict);
        }
        patch.apply(&mut cover);
        cover.updated_at = observed_at.max(cover.updated_at.saturating_add(1)).max(1);
        let steps = serde_json::to_string(&cover.next_steps).map_err(std::io::Error::other)?;
        transaction.execute(
            "INSERT INTO topic_covers(topic_key, goal, next_steps, blocked_note, blocked_session_ref, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(topic_key) DO UPDATE SET goal = excluded.goal,
                next_steps = excluded.next_steps, blocked_note = excluded.blocked_note,
                blocked_session_ref = excluded.blocked_session_ref, updated_at = excluded.updated_at",
            params![topic_key, cover.goal, steps, cover.blocked_note, cover.blocked_session_ref, cover.updated_at],
        )?;
        let revision = bump_revision(&transaction)?;
        transaction.commit()?;
        Ok(revision)
    }
}

fn require_topic(connection: &Connection, topic_key: &str) -> Result<(), CatalogError> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE canonical_key = ?1 AND kind = 'semantic')",
        [topic_key],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(CatalogError::NotFound)
    }
}

fn read_cover(
    connection: &Connection,
    topic_key: &str,
) -> Result<Option<TopicCover>, CatalogError> {
    Ok(connection
        .query_row(
            "SELECT goal, next_steps, blocked_note, blocked_session_ref, updated_at
         FROM topic_covers WHERE topic_key = ?1",
            [topic_key],
            |row| {
                let steps: String = row.get(1)?;
                let next_steps = serde_json::from_str(&steps).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok(TopicCover {
                    goal: row.get(0)?,
                    next_steps,
                    blocked_note: row.get(2)?,
                    blocked_session_ref: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        )
        .optional()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::{
        semantic_fingerprint, semantic_topic_key, SemanticAssignment, SemanticTopicMerge,
    };

    fn seed_topic(catalog: &mut ProjectCatalog, label: &str, session: &str) -> String {
        let item = super::super::tests::candidate("codex", session, 20);
        catalog.upsert_candidate(&item).unwrap();
        catalog
            .apply_semantic_batch(
                &[SemanticAssignment {
                    session_key: item.identity.stable_key.clone(),
                    topic_key: semantic_topic_key(label),
                    topic_label: label.into(),
                    fingerprint: semantic_fingerprint(
                        &item.title.as_ref().unwrap().value,
                        None,
                        "codex",
                    ),
                    backend_used: "codex".into(),
                    model_used: None,
                }],
                21,
            )
            .unwrap();
        catalog
            .snapshot(50)
            .unwrap()
            .topics
            .iter()
            .find(|topic| topic.display_name == label)
            .unwrap()
            .canonical_key
            .clone()
    }

    #[test]
    fn topic_cover_migrates_v6_without_changing_sessions_and_survives_reopen() {
        let dir = super::super::tests::temp_dir("cover-migration");
        let path = dir.join("catalog.sqlite3");
        let mut catalog = ProjectCatalog::open(&path).unwrap();
        let key = seed_topic(&mut catalog, "首批用户", "cover-old");
        let before = catalog.snapshot(50).unwrap();
        catalog
            .connection
            .execute_batch("DROP TABLE topic_covers; PRAGMA user_version = 6;")
            .unwrap();
        drop(catalog);
        let mut catalog = ProjectCatalog::open(&path).unwrap();
        assert_eq!(catalog.snapshot(50).unwrap(), before);
        assert_eq!(catalog.topic_cover(&key).unwrap(), TopicCover::default());
        let patch = TopicCoverPatch {
            goal: Some("这周访谈五位用户".into()),
            next_steps: Some(vec!["发邀请".into(), "约时间".into()]),
            blocked_note: Some("等反馈".into()),
            expected_updated_at: Some(0),
        };
        let revision = catalog.update_topic_cover(&key, &patch, 30).unwrap();
        let saved = catalog.topic_cover(&key).unwrap();
        drop(catalog);
        let mut catalog = ProjectCatalog::open(&path).unwrap();
        assert_eq!(catalog.topic_cover(&key).unwrap(), saved);
        let after = catalog.snapshot(50).unwrap();
        assert_eq!(after.revision, revision);
        assert_eq!(after.projects, before.projects);
        assert_eq!(after.topics[0].sessions, before.topics[0].sessions);
        assert_eq!(
            after.topics[0].canonical_key,
            before.topics[0].canonical_key
        );
        assert_eq!(after.topics[0].cover.as_ref(), Some(&saved));
        catalog
            .connection
            .pragma_update(None, "user_version", 6)
            .unwrap();
        catalog.migrate().unwrap();
        assert_eq!(
            catalog.topic_cover(&key).unwrap(),
            saved,
            "migration is idempotent"
        );
        drop(catalog);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn topic_cover_patch_is_atomic_and_detects_stale_editors() {
        let mut catalog = ProjectCatalog::open_in_memory().unwrap();
        let key = seed_topic(&mut catalog, "Goal", "cover-patch");
        catalog
            .update_topic_cover(
                &key,
                &TopicCoverPatch {
                    goal: Some("Original".into()),
                    next_steps: Some(vec!["Keep this step".into()]),
                    ..Default::default()
                },
                100,
            )
            .unwrap();
        let before = catalog.topic_cover(&key).unwrap();
        let revision = catalog.revision().unwrap();
        let invalid = TopicCoverPatch {
            goal: Some("Must not be saved".into()),
            next_steps: Some(vec!["Step".into(); 4]),
            ..Default::default()
        };
        assert!(matches!(
            catalog.update_topic_cover(&key, &invalid, 101),
            Err(CatalogError::InvalidTopicCover(_))
        ));
        assert_eq!(catalog.topic_cover(&key).unwrap(), before);
        assert_eq!(catalog.revision().unwrap(), revision);
        let mut patch = TopicCoverPatch {
            goal: Some("Agent edit".into()),
            expected_updated_at: Some(100),
            ..Default::default()
        };
        catalog.update_topic_cover(&key, &patch, 99).unwrap();
        let after = catalog.topic_cover(&key).unwrap();
        assert_eq!(after.goal, "Agent edit");
        assert_eq!(after.next_steps, before.next_steps);
        assert_eq!(
            after.updated_at, 101,
            "timestamp remains monotonic when the clock moves back"
        );
        patch.goal = Some("Stale editor".into());
        assert!(matches!(
            catalog.update_topic_cover(&key, &patch, 102),
            Err(CatalogError::TopicCoverConflict)
        ));
        assert_eq!(catalog.topic_cover(&key).unwrap(), after);
        assert!(matches!(
            catalog.update_topic_cover("missing", &patch, 103),
            Err(CatalogError::NotFound)
        ));
        let directory_key = catalog.snapshot(50).unwrap().projects[0]
            .canonical_key
            .clone();
        assert!(matches!(
            catalog.topic_cover(&directory_key),
            Err(CatalogError::NotFound)
        ));
        assert!(matches!(
            catalog.update_topic_cover(&directory_key, &patch, 103),
            Err(CatalogError::NotFound)
        ));
    }

    #[test]
    fn topic_cover_remains_accessible_after_automatic_topic_merge() {
        let mut catalog = ProjectCatalog::open_in_memory().unwrap();
        let from = seed_topic(&mut catalog, "Old topic", "cover-merge-a");
        let into = seed_topic(&mut catalog, "Current topic", "cover-merge-b");
        catalog
            .update_topic_cover(
                &from,
                &TopicCoverPatch {
                    goal: Some("Human goal".into()),
                    ..Default::default()
                },
                30,
            )
            .unwrap();
        let saved = catalog.topic_cover(&from).unwrap();
        catalog
            .apply_topic_merges(
                &[SemanticTopicMerge {
                    from: vec!["Old topic".into()],
                    into: "Current topic".into(),
                }],
                40,
            )
            .unwrap();
        let snapshot = catalog.snapshot(50).unwrap();
        let retained = snapshot
            .topics
            .iter()
            .find(|topic| topic.canonical_key == from)
            .unwrap();
        assert!(retained.sessions.is_empty());
        assert_eq!(retained.cover.as_ref(), Some(&saved));
        assert_eq!(catalog.topic_cover(&into).unwrap(), TopicCover::default());
        assert_eq!(catalog.topic_cover(&from).unwrap(), saved);
    }
}
