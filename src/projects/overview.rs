//! Shared projection for project progress and evidence-backed next actions.
//! An idle Agent or a historical conversation is not proof that a project is done.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::activity::{plain_excerpt, ActivityBatch, ActivityOrigin};
use super::{ProjectKind, ProjectSummary};

pub(crate) const OVERVIEW_SESSION_LIMIT: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkPhase {
    Working,
    NeedsInput,
    Ready,
    Paused,
    Open,
    History,
}

impl WorkPhase {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::NeedsInput => "needs input",
            Self::Ready => "ready to continue",
            Self::Paused => "inactive · ready to continue",
            Self::Open => "open",
            Self::History => "recorded conversation",
        }
    }

    pub(crate) fn attention_order(self) -> u8 {
        match self {
            Self::NeedsInput => 0,
            Self::Working => 1,
            Self::Ready => 2,
            Self::Paused => 3,
            Self::Open => 4,
            Self::History => 5,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProgressCounts {
    pub working: usize,
    pub needs_input: usize,
    pub ready: usize,
    pub paused: usize,
    pub open: usize,
    pub history: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectWorkItem {
    pub session_key: String,
    pub title: String,
    pub backend: String,
    pub phase: WorkPhase,
    /// Human-readable account of the current request or latest result, not the session title.
    pub description: String,
    pub last_activity_at: i64,
    pub latest_update: Option<String>,
    pub update_origin: Option<ActivityOrigin>,
    pub evidence_read_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionSource {
    LiveState,
    SavedPlan,
    Conversation,
    RecentActivity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AdvanceSuggestion {
    /// Identifies this exact suggestion and its evidence, so a stale click cannot run a new one.
    pub id: String,
    pub title: String,
    pub description: String,
    pub reason: String,
    pub source: SuggestionSource,
    /// Stable Catalog identity. None means an authored, unlinked plan item.
    pub session_key: Option<String>,
    /// A user must explicitly choose Continue before this instruction is sent to the Agent.
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub followup: Option<super::followup::ProjectFollowup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectOverview {
    pub project_key: String,
    pub display_name: String,
    pub kind: ProjectKind,
    pub counts: ProgressCounts,
    pub observed_sessions: usize,
    pub more_history_available: bool,
    pub latest_activity_at: Option<i64>,
    pub activity_loading: bool,
    pub work: Vec<ProjectWorkItem>,
    pub suggestions: Vec<AdvanceSuggestion>,
}

pub(crate) fn build_overview(
    project: &ProjectSummary,
    runtime: &HashMap<String, WorkPhase>,
    activity: &ActivityBatch,
) -> ProjectOverview {
    let mut sessions: Vec<_> = project.sessions.iter().collect();
    sessions.sort_by(|a, b| {
        b.last_activity_at
            .cmp(&a.last_activity_at)
            .then_with(|| a.stable_key.cmp(&b.stable_key))
    });
    let more_history_available =
        project.next_cursor.is_some() || sessions.len() > OVERVIEW_SESSION_LIMIT;
    sessions.truncate(OVERVIEW_SESSION_LIMIT);
    let mut counts = ProgressCounts::default();
    let work: Vec<_> = sessions
        .into_iter()
        .map(|session| {
            let phase = if session.live {
                runtime
                    .get(&session.stable_key)
                    .copied()
                    .unwrap_or(WorkPhase::Open)
            } else {
                WorkPhase::History
            };
            match phase {
                WorkPhase::Working => counts.working += 1,
                WorkPhase::NeedsInput => counts.needs_input += 1,
                WorkPhase::Ready => counts.ready += 1,
                WorkPhase::Paused => counts.paused += 1,
                WorkPhase::Open => counts.open += 1,
                WorkPhase::History => counts.history += 1,
            }
            let evidence = activity
                .sessions
                .iter()
                .find(|entry| entry.session_key == session.stable_key);
            ProjectWorkItem {
                session_key: session.stable_key.clone(),
                title: session.title.clone(),
                backend: session.backend.clone(),
                phase,
                description: work_description(phase, evidence),
                last_activity_at: session.last_activity_at,
                latest_update: evidence.and_then(|entry| entry.latest_update.clone()),
                update_origin: evidence.and_then(|entry| entry.origin),
                evidence_read_at: evidence.map(|entry| entry.read_at).filter(|time| *time > 0),
            }
        })
        .collect();
    let mut suggestions = Vec::new();
    // A concrete live blocker is the first thing the person can act on.
    for item in work
        .iter()
        .filter(|item| item.phase == WorkPhase::NeedsInput)
    {
        push_suggestion(
            &mut suggestions,
            AdvanceSuggestion {
                id: String::new(),
                followup: None,
                title: format!("Unblock {}", plain_excerpt(&item.title, 100)),
                description: format!(
                    "{} {}",
                    item.description,
                    if chinese(&item.description) {
                        "这个会话需要你的答复，才能继续推进。"
                    } else {
                        "This conversation needs your answer before it can continue."
                    }
                ),
                reason: format!("{} is waiting for your input", item.backend),
                source: SuggestionSource::LiveState,
                session_key: Some(item.session_key.clone()),
                prompt: None,
            },
        );
    }
    if let Some(cover) = &project.cover {
        if !cover.blocked_note.trim().is_empty() {
            let session_key = cover
                .blocked_session_ref
                .as_ref()
                .filter(|key| work.iter().any(|item| &item.session_key == *key))
                .cloned();
            push_suggestion(
                &mut suggestions,
                AdvanceSuggestion {
                    id: String::new(),
                    followup: None,
                    title: format!("Resolve: {}", plain_excerpt(&cover.blocked_note, 140)),
                    description: cover.blocked_note.clone(),
                    reason: "Blocker from your saved plan".into(),
                    source: SuggestionSource::SavedPlan,
                    session_key,
                    prompt: None,
                },
            );
        }
        for step in &cover.next_steps {
            push_suggestion(
                &mut suggestions,
                AdvanceSuggestion {
                    id: String::new(),
                    followup: None,
                    title: plain_excerpt(step, 180),
                    description: step.clone(),
                    reason: "Next step from your saved plan".into(),
                    source: SuggestionSource::SavedPlan,
                    session_key: None,
                    prompt: None,
                },
            );
        }
    }
    for item in &work {
        if item.phase == WorkPhase::NeedsInput {
            continue;
        }
        if let Some(evidence) = activity
            .sessions
            .iter()
            .find(|entry| entry.session_key == item.session_key)
        {
            for step in &evidence.next_steps {
                push_suggestion(
                    &mut suggestions,
                    AdvanceSuggestion {
                        id: String::new(),
                        followup: None,
                        title: step.clone(),
                        description: recommendation_description(item, step),
                        reason: format!(
                            "From {} · {}",
                            item.backend,
                            plain_excerpt(&item.title, 90)
                        ),
                        source: SuggestionSource::Conversation,
                        session_key: Some(item.session_key.clone()),
                        prompt: Some(followup_prompt(step)),
                    },
                );
            }
        }
    }
    for phase in [
        WorkPhase::Ready,
        WorkPhase::Paused,
        WorkPhase::History,
        WorkPhase::Open,
        WorkPhase::Working,
    ] {
        for item in work.iter().filter(|item| item.phase == phase) {
            if suggestions
                .iter()
                .any(|suggestion| suggestion.session_key.as_deref() == Some(&item.session_key))
            {
                continue;
            }
            let (verb, reason) = match phase {
                WorkPhase::Ready => ("Review", "Agent is ready for your next instruction"),
                WorkPhase::Paused => (
                    "Continue",
                    "Agent is inactive and can continue from its previous context",
                ),
                WorkPhase::History => (
                    "Follow up on",
                    "Read the latest record and confirm the next step",
                ),
                WorkPhase::Open => ("Check", "Open conversation; activity is not confirmed"),
                WorkPhase::Working => (
                    "Follow",
                    "Agent is working; open it to see current progress",
                ),
                WorkPhase::NeedsInput => continue,
            };
            push_suggestion(
                &mut suggestions,
                AdvanceSuggestion {
                    id: String::new(),
                    followup: None,
                    title: format!("{verb} {}", plain_excerpt(&item.title, 100)),
                    description: if chinese(&item.description) {
                        format!(
                            "{} 接下来可以让原 Agent 根据这些进展，找出并完成下一项未完成的工作。",
                            item.description
                        )
                    } else {
                        format!("{} Continue in this conversation to identify and complete the next unfinished step.", item.description)
                    },
                    reason: reason.into(),
                    source: if phase == WorkPhase::History {
                        SuggestionSource::RecentActivity
                    } else {
                        SuggestionSource::LiveState
                    },
                    session_key: Some(item.session_key.clone()),
                    prompt: Some(if chinese(&item.description) {
                        "请结合当前会话的目标和最新进展，找出下一项尚未完成的工作并继续执行，避免重复已完成的内容。完成后汇报结果；缺少必要决定时在此会话中向我说明。".into()
                    } else {
                        "Continue from this conversation's goal and latest progress. Identify and carry out the next unfinished step without repeating completed work. Report the result, or ask me here if a necessary decision is missing.".into()
                    }),
                },
            );
        }
    }
    for suggestion in &mut suggestions {
        // Runtime reports and native resume also advance last_activity_at. Only
        // changed content should turn the same recommendation into a new action.
        let mut digest = Sha256::new();
        for value in [
            project.canonical_key.as_str(),
            suggestion.session_key.as_deref().unwrap_or_default(),
            &suggestion.title,
            &suggestion.description,
            suggestion.prompt.as_deref().unwrap_or_default(),
        ] {
            digest.update((value.len() as u64).to_be_bytes());
            digest.update(value.as_bytes());
        }
        suggestion.id = format!("{:x}", digest.finalize());
    }
    ProjectOverview {
        project_key: project.canonical_key.clone(),
        display_name: project.display_name.clone(),
        kind: project.kind,
        observed_sessions: work.len(),
        more_history_available,
        latest_activity_at: work.first().map(|item| item.last_activity_at),
        activity_loading: activity.loading,
        counts,
        work,
        suggestions,
    }
}

pub(crate) fn chinese(text: &str) -> bool {
    text.chars().any(|c| matches!(c, '\u{3400}'..='\u{9fff}'))
}

fn work_description(
    phase: WorkPhase,
    evidence: Option<&super::activity::SessionActivity>,
) -> String {
    if let Some(evidence) = evidence {
        let request = evidence.latest_request.as_deref();
        let response = evidence.latest_response.as_deref();
        if phase == WorkPhase::Working {
            if let Some(request) = request {
                let request = plain_excerpt(request, 230);
                return if chinese(&request) {
                    format!("正在推进这项请求：{request}")
                } else {
                    format!("Working on this request: {request}")
                };
            }
        }
        if let Some(text) = evidence.latest_update.as_deref().or(response).or(request) {
            return text.to_string();
        }
    }
    match phase {
        WorkPhase::Working => "The Agent is working; a detailed update has not been recorded yet.",
        WorkPhase::NeedsInput => {
            "The Agent is waiting for a decision or a reply in this conversation."
        }
        WorkPhase::Ready => {
            "The Agent has finished its current turn and is ready for the next instruction."
        }
        WorkPhase::Paused => {
            "The conversation is retained and can be resumed from its previous context."
        }
        WorkPhase::Open => {
            "The conversation is open; a detailed activity update is not available yet."
        }
        WorkPhase::History => {
            "This recorded conversation can be reopened to continue from its earlier context."
        }
    }
    .into()
}

fn recommendation_description(item: &ProjectWorkItem, step: &str) -> String {
    let context = item
        .latest_update
        .as_deref()
        .filter(|text| !text.contains(step.trim_end_matches(['。', '.'])))
        .map(|text| plain_excerpt(text, 200))
        .unwrap_or_default();
    if chinese(step) {
        format!(
            "接下来建议{}。{}",
            step.trim_end_matches(['。', '.']),
            context
        )
    } else {
        format!(
            "The Agent recommends: {}. {}",
            step.trim_end_matches('.'),
            context
        )
    }
}

fn followup_prompt(step: &str) -> String {
    if chinese(step) {
        format!("请在当前会话中继续推进这项建议：\n\n{step}\n\n结合已有进展执行，避免重复已完成的工作，并汇报结果。")
    } else {
        format!("Continue in this conversation with this suggested follow-up:\n\n{step}\n\nUse the existing progress, avoid repeating completed work, and report the result.")
    }
}

fn push_suggestion(suggestions: &mut Vec<AdvanceSuggestion>, suggestion: AdvanceSuggestion) {
    if suggestions.len() < 3
        && !suggestions
            .iter()
            .any(|existing| existing.title == suggestion.title)
    {
        suggestions.push(suggestion);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::projects::{IndexedSessionSummary, SessionClass, SessionRefKind, TopicCover};

    pub(crate) fn fixture_project() -> ProjectSummary {
        let sessions = [
            "Prepare invitations",
            "Confirm pricing",
            "Review interview notes",
        ]
        .into_iter()
        .enumerate()
        .map(|(index, title)| IndexedSessionSummary {
            stable_key: format!("session-{index}"),
            backend: "codex".into(),
            ref_kind: SessionRefKind::Id,
            ref_value: format!("native-{index}"),
            title: title.into(),
            cwd: None,
            first_activity_at: 1,
            last_activity_at: 100 - index as i64,
            live: index < 2,
            workspace_id: None,
            pane_id: None,
            runtime_generation: None,
            session_class: SessionClass::Interactive,
            topic_label: None,
            transcript_ref: None,
        })
        .collect();
        ProjectSummary {
            canonical_key: "topic".into(),
            kind: ProjectKind::Semantic,
            display_name: "First customers".into(),
            canonical_path: "topic".into(),
            sessions,
            automation: vec![],
            thin_count: 0,
            next_cursor: None,
            cover: None,
        }
    }

    #[test]
    fn empty_cover_still_has_progress_and_actionable_evidence() {
        let project = fixture_project();
        let states = HashMap::from([
            ("session-0".into(), WorkPhase::Working),
            ("session-1".into(), WorkPhase::NeedsInput),
        ]);
        let overview = build_overview(&project, &states, &ActivityBatch::default());
        assert_eq!(overview.counts.working, 1);
        assert_eq!(overview.counts.needs_input, 1);
        assert_eq!(overview.counts.history, 1);
        assert_eq!(
            overview.suggestions[0].session_key.as_deref(),
            Some("session-1")
        );
        assert_eq!(overview.suggestions[0].source, SuggestionSource::LiveState);
        assert!(overview.suggestions[0].title.contains("Confirm pricing"));
        assert_eq!(overview.work[2].phase, WorkPhase::History);
    }

    #[test]
    fn authored_plan_and_conversation_steps_keep_their_provenance() {
        let mut project = fixture_project();
        project.cover = Some(TopicCover {
            goal: "Keep the authored goal".into(),
            next_steps: vec!["Call the first customer".into()],
            ..Default::default()
        });
        let before = project.cover.clone();
        let activity = ActivityBatch {
            sessions: vec![super::super::activity::SessionActivity {
                session_key: "session-2".into(),
                next_steps: vec!["Compare the interview findings".into()],
                ..Default::default()
            }],
            loading: false,
        };
        let overview = build_overview(&project, &HashMap::new(), &activity);
        assert_eq!(overview.suggestions[0].source, SuggestionSource::SavedPlan);
        assert_eq!(
            overview.suggestions[1].source,
            SuggestionSource::Conversation
        );
        assert_eq!(
            overview.suggestions[1].session_key.as_deref(),
            Some("session-2")
        );
        assert_eq!(project.cover, before);
    }

    #[test]
    fn history_never_borrows_a_live_state_and_pagination_is_explicit() {
        let mut project = fixture_project();
        project.next_cursor = Some(crate::projects::SessionCursor {
            last_activity_at: 1,
            stable_key: "older".into(),
        });
        let states = HashMap::from([("session-2".into(), WorkPhase::Working)]);
        let overview = build_overview(&project, &states, &ActivityBatch::default());
        assert!(overview.more_history_available);
        assert_eq!(overview.counts.working, 0);
        assert_eq!(overview.work[2].phase, WorkPhase::History);
    }

    #[test]
    fn followup_id_survives_runtime_updates_but_changes_with_its_instruction() {
        let mut project = fixture_project();
        let mut activity = ActivityBatch {
            sessions: vec![super::super::activity::SessionActivity {
                session_key: "session-0".into(),
                latest_update: Some("The consent form has been checked.".into()),
                next_steps: vec!["Submit for review, then send after approval.".into()],
                ..Default::default()
            }],
            loading: false,
        };
        let before = build_overview(&project, &HashMap::new(), &activity);
        project.sessions[0].last_activity_at += 5_000;
        activity.sessions[0].read_at += 5_000;
        let refreshed = build_overview(&project, &HashMap::new(), &activity);
        assert_eq!(before.suggestions[0].id, refreshed.suggestions[0].id);
        activity.sessions[0].next_steps[0] = "Revise the consent form before submitting.".into();
        let changed = build_overview(&project, &HashMap::new(), &activity);
        assert_ne!(before.suggestions[0].id, changed.suggestions[0].id);
    }
}
