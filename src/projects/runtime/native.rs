//! Connect foreground agents to indexed native history using process-owned evidence.
//! Directory proximity and recent activity are never sufficient identity evidence.
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use crate::projects::{adapters::AdapterRootSet, SessionIdentity};

fn bounded_json(path: &Path) -> Option<serde_json::Value> {
    let file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > 64 * 1024 {
        return None;
    }
    serde_json::from_reader(file.take(64 * 1024)).ok()
}

fn allowed_path(roots: &AdapterRootSet, backend: &str, path: &Path) -> Option<PathBuf> {
    let path = path.canonicalize().ok()?;
    roots
        .allowed_roots(backend)
        .iter()
        .any(|root| root.canonicalize().is_ok_and(|root| path.starts_with(root)))
        .then_some(path)
}

fn codex_owned_id(path: &Path) -> Option<String> {
    // The first rollout record is metadata. Bound reads even for malformed/untrusted histories.
    let file = std::fs::File::open(path).ok()?;
    let mut line = String::new();
    BufReader::new(file.take(64 * 1024))
        .read_line(&mut line)
        .ok()?;
    let value: serde_json::Value = serde_json::from_str(&line).ok()?;
    if value.get("type")?.as_str()? != "session_meta" {
        return None;
    }
    let meta = value.get("payload")?;
    // A TUI can hold its child-agent transcripts too. They do not identify the parent pane.
    if !matches!(
        meta.get("source").and_then(|v| v.as_str()),
        Some("cli" | "vscode")
    ) {
        return None;
    }
    meta.get("id")?.as_str().map(str::to_string)
}

fn claude_registered_id(path: &Path, pid: u32, start: &str) -> Option<String> {
    let value = bounded_json(path)?;
    if value.get("pid")?.as_u64()? != u64::from(pid) {
        return None;
    }
    let marker = value
        .get("procStart")?
        .as_str()?
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if marker != start || start.is_empty() {
        return None;
    }
    value.get("sessionId")?.as_str().map(str::to_string)
}

/// Discovery only links an existing indexed identity. It never replaces a generated/custom title.
pub(crate) fn session_from_process_evidence(
    roots: &AdapterRootSet,
    lookup: impl Fn(&SessionIdentity) -> Option<PathBuf>,
    backend: &str,
    processes: &[(u32, Vec<PathBuf>, Option<String>)],
) -> Option<SessionIdentity> {
    let mut identities = std::collections::HashMap::new();
    let mut owned_ids = std::collections::HashSet::new();
    for (pid, files, start) in processes {
        let mut evidence = Vec::new();
        if backend == "codex" {
            for file in files {
                let Some(path) = allowed_path(roots, backend, file) else {
                    continue;
                };
                if path.extension().is_none_or(|ext| ext != "jsonl") {
                    continue;
                }
                if let Some(id) = codex_owned_id(&path) {
                    evidence.push((id, Some(path)));
                }
            }
        } else if backend == "claude" {
            let Some(start) = start else {
                continue;
            };
            for root in roots.allowed_roots(backend) {
                // Claude stores its PID registry beside the configured projects root.
                if root.file_name().is_none_or(|name| name != "projects") {
                    continue;
                }
                let Some(home) = root.parent() else {
                    continue;
                };
                if let Some(id) = claude_registered_id(
                    &home.join("sessions").join(format!("{pid}.json")),
                    *pid,
                    start,
                ) {
                    evidence.push((id, None));
                }
            }
        }
        for (id, owned_path) in evidence {
            owned_ids.insert(id.clone());
            let Ok(identity) = SessionIdentity::id(backend, &id) else {
                continue;
            };
            let Some(path) = lookup(&identity).and_then(|p| allowed_path(roots, backend, &p))
            else {
                continue;
            };
            if owned_path.as_ref().is_some_and(|owned| *owned != path) {
                continue;
            }
            identities.insert(identity.stable_key.clone(), identity);
        }
    }
    if owned_ids.len() == 1 && identities.len() == 1 {
        identities.into_values().next()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::{
        IndexedSessionSummary, ProjectKind, ProjectSummary, ProjectsSnapshot, SessionClass,
    };

    fn lookup(snapshot: &ProjectsSnapshot) -> impl Fn(&SessionIdentity) -> Option<PathBuf> + '_ {
        |identity| {
            snapshot
                .projects
                .iter()
                .flat_map(|p| &p.sessions)
                .find(|s| s.stable_key == identity.stable_key)
                .and_then(|s| s.transcript_ref.as_ref().map(PathBuf::from))
        }
    }

    fn fixture(
        backend: &str,
        ids: &[&str],
    ) -> (PathBuf, AdapterRootSet, ProjectsSnapshot, Vec<PathBuf>) {
        let base = std::env::temp_dir().join(format!(
            "herduck-native-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let root = base.join(if backend == "claude" {
            "projects"
        } else {
            "sessions"
        });
        std::fs::create_dir_all(&root).expect("root");
        let mut config = crate::config::ProjectsConfig::default();
        if backend == "claude" {
            config.adapters.claude.roots = vec![root.clone()];
        } else {
            config.adapters.codex.roots = vec![root.clone()];
        }
        let roots = crate::projects::adapters::resolve_roots(&config);
        let mut snapshot = ProjectsSnapshot::empty();
        let mut sessions = Vec::new();
        let mut paths = Vec::new();
        for id in ids {
            let path = root.join(format!("{id}.jsonl"));
            std::fs::write(
                &path,
                serde_json::json!({"type":"session_meta","payload":{"id":id,"source":"cli"}})
                    .to_string(),
            )
            .expect("transcript");
            let identity = SessionIdentity::id(backend, id).expect("identity");
            sessions.push(IndexedSessionSummary {
                stable_key: identity.stable_key,
                backend: backend.into(),
                ref_kind: identity.ref_kind,
                ref_value: (*id).into(),
                title: "【Herduck】修复会话标题关联".into(),
                cwd: None,
                first_activity_at: 0,
                last_activity_at: 0,
                live: false,
                workspace_id: None,
                pane_id: None,
                runtime_generation: None,
                session_class: SessionClass::Interactive,
                topic_label: None,
                transcript_ref: Some(path.to_string_lossy().into_owned()),
            });
            paths.push(path);
        }
        snapshot.projects.push(ProjectSummary {
            canonical_key: "fixture".into(),
            kind: ProjectKind::Cwd,
            display_name: "fixture".into(),
            canonical_path: root.to_string_lossy().into_owned(),
            sessions,
            automation: vec![],
            thin_count: 0,
            next_cursor: None,
            cover: None,
        });
        (base, roots, snapshot, paths)
    }

    #[test]
    fn codex_uses_unique_owned_root_transcript_and_preserves_summary() {
        let (base, roots, snapshot, paths) = fixture("codex", &["one", "two"]);
        let before = snapshot.clone();
        let find = |files| {
            session_from_process_evidence(&roots, lookup(&snapshot), "codex", &[(10, files, None)])
        };
        assert!(find(vec![]).is_none());
        assert_eq!(
            find(vec![paths[0].clone(), paths[0].clone()])
                .expect("unique")
                .canonical_ref_value,
            "one"
        );
        assert!(find(paths.clone()).is_none());
        let mut partial = snapshot.clone();
        partial.projects[0].sessions.pop();
        assert!(session_from_process_evidence(
            &roots,
            lookup(&partial),
            "codex",
            &[(10, paths.clone(), None)]
        )
        .is_none());
        // Child agents share the parent's process but are not the pane's current session.
        std::fs::write(&paths[1],serde_json::json!({"type":"session_meta","payload":{"id":"two","source":{"subagent":{}}}}).to_string()).expect("child");
        assert_eq!(
            find(paths.clone()).expect("parent").canonical_ref_value,
            "one"
        );
        let outside = base.join("outside.jsonl");
        std::fs::copy(&paths[0], &outside).expect("copy");
        assert!(find(vec![outside]).is_none());
        assert_eq!(snapshot, before);
        std::fs::remove_dir_all(base).expect("cleanup");
    }

    #[test]
    fn claude_registry_requires_matching_process_birth_and_catalog_history() {
        let (base, roots, snapshot, _) = fixture("claude", &["one", "two"]);
        let registry = base.join("sessions");
        std::fs::create_dir_all(&registry).expect("registry");
        let write = |pid, id, start| {
            std::fs::write(
                registry.join(format!("{pid}.json")),
                serde_json::json!({"pid":pid,"sessionId":id,"procStart":start}).to_string(),
            )
            .expect("record")
        };
        let find = |processes: Vec<(u32, Vec<PathBuf>, Option<String>)>| {
            session_from_process_evidence(&roots, lookup(&snapshot), "claude", &processes)
        };
        write(10, "one", "start");
        assert_eq!(
            find(vec![(10, vec![], Some("start".into()))])
                .expect("current")
                .canonical_ref_value,
            "one"
        );
        assert!(find(vec![(10, vec![], Some("new birth".into()))]).is_none());
        assert!(find(vec![(10, vec![], None)]).is_none());
        std::fs::copy(registry.join("10.json"), registry.join("11.json")).expect("stale pid");
        assert!(find(vec![(11, vec![], Some("start".into()))]).is_none());
        write(11, "two", "start");
        assert!(find(vec![
            (10, vec![], Some("start".into())),
            (11, vec![], Some("start".into()))
        ])
        .is_none());
        write(10, "missing", "start");
        assert!(find(vec![(10, vec![], Some("start".into()))]).is_none());
        std::fs::remove_dir_all(base).expect("cleanup");
    }
}
