use std::path::{Path, PathBuf};

use super::adapters::AdapterRootSet;
use super::{CandidateField, RuntimeMapping, SessionCandidate, SessionIdentity, SourcePriority};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeLease {
    pub session_key: String,
    pub workspace_id: String,
    pub pane_id: String,
    pub generation: u64,
    pub observed_at: i64,
}

pub(crate) fn identity_from_report(
    roots: &AdapterRootSet,
    adapter: &str,
    session_ref: &crate::agent_resume::AgentSessionRef,
) -> Option<SessionIdentity> {
    if !super::adapters::ADAPTER_NAMES.contains(&adapter) {
        return None;
    }
    match session_ref.kind {
        crate::agent_resume::AgentSessionRefKind::Id => {
            SessionIdentity::id(adapter, &session_ref.value).ok()
        }
        crate::agent_resume::AgentSessionRefKind::Path => SessionIdentity::path(
            adapter,
            Path::new(&session_ref.value),
            Path::new(std::path::MAIN_SEPARATOR_STR),
            roots.allowed_roots(adapter),
            true,
        )
        .ok(),
    }
}

pub(crate) struct RuntimeCandidateInput<'a> {
    pub identity: SessionIdentity,
    pub cwd: PathBuf,
    pub workspace_id: &'a str,
    pub pane_id: &'a str,
    pub generation: u64,
    pub observed_at: i64,
}

/// Only files actually held by the foreground process are eligible. Never guess by cwd/recency.
pub(crate) fn grok_session_from_open_files(
    roots: &AdapterRootSet,
    files: &[PathBuf],
    terminal_title: &str,
) -> Option<(SessionIdentity, PathBuf, String)> {
    let mut candidates = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for file in files
        .iter()
        .filter(|file| file.file_name().is_some_and(|name| name == "events.jsonl"))
    {
        let Some(dir) = file.parent() else {
            continue;
        };
        let Some(identity) = identity_from_report(
            roots,
            "grok",
            &crate::agent_resume::AgentSessionRef {
                kind: crate::agent_resume::AgentSessionRefKind::Path,
                value: dir.to_string_lossy().into_owned(),
            },
        ) else {
            continue;
        };
        if !seen.insert(identity.stable_key.clone()) {
            continue;
        }
        let summary_path = dir.join("summary.json");
        if !std::fs::metadata(&summary_path).is_ok_and(|metadata| metadata.len() <= 256 * 1024) {
            continue;
        }
        let Some(summary) = std::fs::read_to_string(&summary_path)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        else {
            continue;
        };
        let title = summary
            .get("generated_title")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let identity = summary
            .get("id")
            .or_else(|| summary.get("session_id"))
            .or_else(|| summary.get("sessionId"))
            .and_then(|value| value.as_str())
            .and_then(|id| SessionIdentity::id("grok", id).ok())
            .unwrap_or(identity);
        candidates.push((identity, summary_path, title));
    }
    if candidates.len() == 1 {
        return candidates.pop();
    }
    // Some Grok processes retain an old session's event file after /new. The visible title
    // disambiguates only within the process-owned candidates; ambiguous ownership stays live-only.
    let visible = terminal_title
        .trim()
        .strip_suffix(" - grok")
        .unwrap_or(terminal_title.trim());
    let matches: Vec<_> = candidates
        .into_iter()
        .filter(|(_, _, title)| {
            !title.is_empty()
                && (visible.ends_with(title)
                    || visible.rsplit(" - ").next().is_some_and(|part| {
                        let prefix = part.trim_end_matches(['…', '.']);
                        prefix.chars().count() >= 8 && prefix != part && title.starts_with(prefix)
                    }))
        })
        .collect();
    (matches.len() == 1)
        .then(|| matches.into_iter().next())
        .flatten()
}

pub(crate) fn candidate_from_report(input: RuntimeCandidateInput<'_>) -> SessionCandidate {
    let source_key = format!("runtime:{}:{}", input.workspace_id, input.pane_id);
    SessionCandidate {
        identity: input.identity,
        title: None,
        cwd: Some(CandidateField {
            value: input.cwd,
            observed_at: input.observed_at,
            priority: SourcePriority::RuntimeReport,
            source_key: source_key.clone(),
        }),
        transcript_ref: None,
        first_activity_at: input.observed_at,
        last_activity_at: input.observed_at,
        adapter: "runtime".to_string(),
        root_key: "runtime".to_string(),
        source_key,
        observed_at: input.observed_at,
        aliases: Vec::new(),
        runtime: Some(RuntimeMapping {
            workspace_id: input.workspace_id.to_string(),
            pane_id: input.pane_id.to_string(),
            generation: input.generation,
        }),
        // A runtime report carries no transcript, so weight comes from the file scan instead.
        weight: super::adapters::SessionWeight::default(),
        session_class: None,
    }
}

pub(crate) fn unix_time_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grok_requires_process_owned_files_and_disambiguates_retained_sessions() {
        let root =
            std::env::temp_dir().join(format!("herduck-grok-identity-{}", std::process::id()));
        let mut config = crate::config::ProjectsConfig::default();
        config.adapters.grok.roots = vec![root.clone()];
        let mut files = Vec::new();
        for (id, title) in [
            ("first", "Pixel photo backup"),
            ("second", "Kimi interview research"),
        ] {
            let dir = root.join(id);
            std::fs::create_dir_all(&dir).expect("fixture");
            std::fs::write(
                dir.join("summary.json"),
                serde_json::json!({"id":id,"generated_title":title}).to_string(),
            )
            .expect("summary");
            files.push(dir.join("events.jsonl"));
        }
        let roots = super::super::adapters::resolve_roots(&config);
        assert!(grok_session_from_open_files(&roots, &[], "Pixel photo backup - grok").is_none());
        assert!(grok_session_from_open_files(&roots, &files, "unrelated - grok").is_none());
        let found = grok_session_from_open_files(
            &roots,
            &files,
            "working - Kimi interview research - grok",
        )
        .expect("unique process-owned match");
        assert_eq!(
            found.0,
            SessionIdentity::id("grok", "second").expect("identity")
        );
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[test]
    fn id_runtime_identity_uses_the_same_native_tuple_as_file_scans() {
        let roots = AdapterRootSet::default();
        let session_ref = crate::agent_resume::AgentSessionRef::id("same-id").unwrap();
        let identity = identity_from_report(&roots, "codex", &session_ref).unwrap();
        assert_eq!(
            identity,
            SessionIdentity::id("codex", "same-id").expect("file identity")
        );
    }
}
