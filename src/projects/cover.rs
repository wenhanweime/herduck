//! Authored Topic metadata. Classification never generates or rewrites these fields.

use serde::{Deserialize, Serialize};

pub(crate) const MAX_COVER_TEXT_CHARS: usize = 2000;
pub(crate) const MAX_NEXT_STEPS: usize = 3;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct TopicCover {
    #[schemars(length(max = 2000))]
    pub goal: String,
    #[schemars(length(max = 3))]
    pub next_steps: Vec<String>,
    #[schemars(length(max = 2000))]
    pub blocked_note: String,
    /// Reserved for a future validated history/runtime link; currently always absent.
    pub blocked_session_ref: Option<String>,
    /// Unix milliseconds; zero means this Topic has no saved cover yet.
    pub updated_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct TopicCoverPatch {
    /// Omitted fields remain unchanged. An empty string clears a text field.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 2000))]
    pub goal: Option<String>,
    /// Replaces the list; an empty list clears it. At most three entries.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 3))]
    pub next_steps: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 2000))]
    pub blocked_note: Option<String>,
    /// Optional optimistic concurrency guard, including zero for an unsaved cover.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_updated_at: Option<i64>,
}

impl TopicCoverPatch {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.goal.is_none() && self.next_steps.is_none() && self.blocked_note.is_none() {
            return Err("Provide a goal, next_steps, or blocked_note to update");
        }
        if self
            .next_steps
            .as_ref()
            .is_some_and(|steps| steps.len() > MAX_NEXT_STEPS)
        {
            return Err("A Topic cover can have at most three next steps");
        }
        let fields = self
            .goal
            .iter()
            .chain(self.blocked_note.iter())
            .chain(self.next_steps.iter().flatten());
        for field in fields {
            if field.chars().count() > MAX_COVER_TEXT_CHARS {
                return Err("Each Topic cover field must be at most 2000 characters");
            }
            if field
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
            {
                return Err("Topic cover text cannot contain terminal control characters");
            }
        }
        if self.expected_updated_at.is_some_and(|value| value < 0) {
            return Err("expected_updated_at must be zero or a saved cover timestamp");
        }
        Ok(())
    }

    pub(crate) fn apply(&self, cover: &mut TopicCover) {
        if let Some(goal) = &self.goal {
            cover.goal = goal.trim().to_string();
        }
        if let Some(steps) = &self.next_steps {
            cover.next_steps = steps
                .iter()
                .map(|step| step.trim().to_string())
                .filter(|step| !step.is_empty())
                .collect();
        }
        if let Some(note) = &self.blocked_note {
            cover.blocked_note = note.trim().to_string();
        }
    }
}

#[cfg(test)]
pub(crate) fn test_catalog() -> (super::ProjectCatalog, String) {
    use super::{
        CandidateField, SemanticAssignment, SessionCandidate, SessionClass, SessionIdentity,
        SourcePriority,
    };
    let mut catalog = super::ProjectCatalog::open_in_memory().expect("cover catalog");
    let identity = SessionIdentity::id("codex", "topic-cover-test").expect("session identity");
    let title = "Plan first user interviews";
    let session_key = identity.stable_key.clone();
    catalog
        .upsert_candidate(&SessionCandidate {
            identity,
            title: Some(CandidateField {
                value: title.into(),
                observed_at: 20,
                priority: SourcePriority::PrimaryIndex,
                source_key: "cover-test".into(),
            }),
            cwd: None,
            transcript_ref: None,
            first_activity_at: 10,
            last_activity_at: 20,
            adapter: "codex".into(),
            root_key: "cover-test".into(),
            source_key: "cover-test".into(),
            observed_at: 20,
            aliases: Vec::new(),
            runtime: None,
            weight: super::adapters::SessionWeight {
                turns: 4,
                chars: 400,
                known: true,
            },
            session_class: Some(SessionClass::Interactive),
        })
        .expect("session");
    catalog
        .apply_semantic_batch(
            &[SemanticAssignment {
                session_key,
                topic_key: super::semantic_topic_key("First users"),
                topic_label: "First users".into(),
                fingerprint: super::semantic_fingerprint(title, None, "codex"),
                backend_used: "codex".into(),
                model_used: None,
            }],
            21,
        )
        .expect("topic");
    let key = catalog.snapshot(50).expect("snapshot").topics[0]
        .canonical_key
        .clone();
    (catalog, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_or_partial_cover_json_defaults_missing_fields() {
        assert_eq!(
            serde_json::from_str::<TopicCover>("{}").unwrap(),
            TopicCover::default()
        );
        let cover: TopicCover = serde_json::from_str(r#"{"goal":"本周目标"}"#).unwrap();
        assert_eq!(cover.goal, "本周目标");
        assert!(cover.next_steps.is_empty());
        assert!(cover.blocked_note.is_empty());
        assert_eq!(cover.updated_at, 0);
        assert_eq!(cover.blocked_session_ref, None);
    }

    #[test]
    fn patches_distinguish_omitted_fields_from_explicit_clears() {
        let mut cover = TopicCover {
            goal: "Goal".into(),
            next_steps: vec!["Step".into()],
            blocked_note: "Waiting".into(),
            ..Default::default()
        };
        let patch: TopicCoverPatch = serde_json::from_str(r#"{"goal":"New goal"}"#).unwrap();
        patch.validate().unwrap();
        patch.apply(&mut cover);
        assert_eq!(cover.goal, "New goal");
        assert_eq!(cover.next_steps, ["Step"]);
        assert_eq!(cover.blocked_note, "Waiting");
        let clear: TopicCoverPatch =
            serde_json::from_str(r#"{"next_steps":[],"blocked_note":""}"#).unwrap();
        clear.validate().unwrap();
        clear.apply(&mut cover);
        assert!(cover.next_steps.is_empty());
        assert!(cover.blocked_note.is_empty());
    }

    #[test]
    fn patch_rejects_overflow_and_controls_without_restricting_unicode() {
        let mut patch = TopicCoverPatch {
            goal: Some("中文目标\n第二句".into()),
            ..Default::default()
        };
        assert!(patch.validate().is_ok());
        patch.next_steps = Some(vec!["step".into(); 4]);
        assert!(patch.validate().is_err());
        patch.next_steps = None;
        patch.goal = Some("\x1b[2J".into());
        assert!(patch.validate().is_err());
        patch.goal = Some("目".repeat(MAX_COVER_TEXT_CHARS));
        assert!(patch.validate().is_ok());
        patch.goal.as_mut().unwrap().push('标');
        assert!(patch.validate().is_err());
        assert!(TopicCoverPatch::default().validate().is_err());
    }
}
