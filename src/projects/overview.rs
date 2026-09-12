//! Shared projection for project progress and evidence-backed next actions.
//! An idle Agent or a historical conversation is not proof that a project is done.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::activity::{plain_excerpt, ActivityBatch, ActivityOrigin};
use super::{ProjectKind, ProjectSummary};
use crate::config::TitleLanguage;

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
    #[serde(default)]
    pub language: TitleLanguage,
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
    language: TitleLanguage,
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
                description: work_description(phase, evidence, activity, language),
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
                id: suggestion_id(
                    project,
                    Some(&item.session_key),
                    "blocked",
                    &item.session_key,
                ),
                followup: None,
                title: language
                    .text("Reply in this conversation", "答复此会话后继续")
                    .into(),
                description: format!(
                    "{} {}",
                    item.description,
                    language.text(
                        "This conversation needs your answer before it can continue.",
                        "这个会话需要你的答复，才能继续推进。"
                    )
                ),
                reason: language
                    .text(
                        "The Agent is waiting for your input",
                        "Agent 正在等待你的答复",
                    )
                    .into(),
                source: SuggestionSource::LiveState,
                session_key: Some(item.session_key.clone()),
                prompt: None,
            },
        );
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
                let translated = activity.localization.get(step, language);
                push_suggestion(
                    &mut suggestions,
                    AdvanceSuggestion {
                        id: suggestion_id(
                            project,
                            Some(&item.session_key),
                            "conversation_step",
                            step,
                        ),
                        followup: None,
                        title: translated.map(str::to_string).unwrap_or_else(|| {
                            language
                                .text("Read the recorded next step", "查看原会话中的下一步")
                                .into()
                        }),
                        description: translated
                            .map(|text| {
                                recommendation_description(item, step, text, activity, language)
                            })
                            .unwrap_or_else(|| untranslated(activity, language)),
                        reason: language
                            .text(
                                "Next step recorded by the Agent",
                                "Agent 在会话中提出的下一步",
                            )
                            .into(),
                        source: SuggestionSource::Conversation,
                        session_key: Some(item.session_key.clone()),
                        prompt: translated.map(|text| followup_prompt(text, language)),
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
            let reason = match phase {
                WorkPhase::Ready => language.text(
                    "Agent is ready for your next instruction",
                    "Agent 已准备好接收下一条指令",
                ),
                WorkPhase::Paused => language.text(
                    "Agent is inactive and can continue from its previous context",
                    "Agent 已暂停，可以从之前的上下文继续",
                ),
                WorkPhase::History => language.text(
                    "Read the latest record and confirm the next step",
                    "根据最新记录确认下一步",
                ),
                WorkPhase::Open => language.text(
                    "Open conversation; activity is not confirmed",
                    "会话已打开，当前活动尚未确认",
                ),
                WorkPhase::Working => language.text(
                    "Agent is working; open it to see current progress",
                    "Agent 正在执行，打开会话可查看进展",
                ),
                WorkPhase::NeedsInput => continue,
            };
            push_suggestion(
                &mut suggestions,
                AdvanceSuggestion {
                    id: suggestion_id(project, Some(&item.session_key), "continue", item.latest_update.as_deref().unwrap_or_default()),
                    followup: None,
                    title: language.text("Continue the next unfinished step", "推进下一项未完成的工作").into(),
                    description: if language == TitleLanguage::Chinese {
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
                    prompt: Some(language.text(
                        "Continue from this conversation's goal and latest progress. Identify and carry out the next unfinished step without repeating completed work. Report the result, or ask me here if a necessary decision is missing. Use English for all progress updates, summaries, next steps and replies.",
                        "请结合当前会话的目标和最新进展，找出下一项尚未完成的工作并继续执行，避免重复已完成的内容。完成后汇报结果；缺少必要决定时在此会话中向我说明。进展总结、下一步建议和回复都请使用简体中文。"
                    ).into()),
                },
            );
        }
    }
    // Session-specific Agent actions take precedence over unlinked authored plan items.
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
                    id: suggestion_id(
                        project,
                        session_key.as_deref(),
                        "saved_blocker",
                        &cover.blocked_note,
                    ),
                    followup: None,
                    title: language
                        .text("Resolve the saved blocker", "解决计划中的阻塞")
                        .into(),
                    description: activity
                        .localization
                        .get(&cover.blocked_note, language)
                        .map(str::to_string)
                        .unwrap_or_else(|| untranslated(activity, language)),
                    reason: language
                        .text("Blocker from your saved plan", "来自你保存的计划：当前阻塞")
                        .into(),
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
                    id: suggestion_id(project, None, "saved_step", step),
                    followup: None,
                    title: language
                        .text("Next step from your saved plan", "计划中的下一步")
                        .into(),
                    description: activity
                        .localization
                        .get(step, language)
                        .map(str::to_string)
                        .unwrap_or_else(|| untranslated(activity, language)),
                    reason: language
                        .text("Next step from your saved plan", "来自你保存的计划：下一步")
                        .into(),
                    source: SuggestionSource::SavedPlan,
                    session_key: None,
                    prompt: None,
                },
            );
        }
    }
    ProjectOverview {
        language,
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

fn suggestion_id(
    project: &ProjectSummary,
    session: Option<&str>,
    kind: &str,
    evidence: &str,
) -> String {
    // Identity follows the original instruction, never a translation, runtime timestamp or label.
    let mut digest = Sha256::new();
    for value in [
        project.canonical_key.as_str(),
        session.unwrap_or_default(),
        kind,
        evidence,
    ] {
        digest.update((value.len() as u64).to_be_bytes());
        digest.update(value.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn untranslated(activity: &ActivityBatch, language: TitleLanguage) -> String {
    if activity.localization.loading {
        language.text("Preparing an English summary of the latest record. You can view the original conversation meanwhile.",
            "正在将最新记录整理为中文摘要，你可以先查看原会话。").into()
    } else {
        language.text("The latest record needs an English summary. View the original conversation; translated summaries require an available Summary source.",
            "最新记录尚无中文摘要，可先查看原会话。摘要服务可用后会自动转换。").into()
    }
}

fn work_description(
    phase: WorkPhase,
    evidence: Option<&super::activity::SessionActivity>,
    activity: &ActivityBatch,
    language: TitleLanguage,
) -> String {
    if let Some(evidence) = evidence {
        let request = evidence.latest_request.as_deref();
        let response = evidence.latest_response.as_deref();
        if phase == WorkPhase::Working {
            if let Some(request) = request {
                let Some(request) = activity.localization.get(request, language) else {
                    return untranslated(activity, language);
                };
                let request = plain_excerpt(request, 230);
                return if language == TitleLanguage::Chinese {
                    format!("正在推进这项请求：{request}")
                } else {
                    format!("Working on this request: {request}")
                };
            }
        }
        if let Some(text) = evidence.latest_update.as_deref().or(response).or(request) {
            return activity
                .localization
                .get(text, language)
                .map(str::to_string)
                .unwrap_or_else(|| untranslated(activity, language));
        }
    }
    match phase {
        WorkPhase::Working => language.text(
            "The Agent is working; a detailed update has not been recorded yet.",
            "Agent 正在执行，暂时还没有记录详细进展。",
        ),
        WorkPhase::NeedsInput => language.text(
            "The Agent is waiting for a decision or a reply in this conversation.",
            "Agent 正在等待你在此会话中作出决定或回复。",
        ),
        WorkPhase::Ready => language.text(
            "The Agent has finished its current turn and is ready for the next instruction.",
            "Agent 已完成当前一轮，可以接收下一条指令。",
        ),
        WorkPhase::Paused => language.text(
            "The conversation is retained and can be resumed from its previous context.",
            "会话记录已保留，可以从之前的上下文继续。",
        ),
        WorkPhase::Open => language.text(
            "The conversation is open; a detailed activity update is not available yet.",
            "会话已打开，暂时还没有详细的进展记录。",
        ),
        WorkPhase::History => language.text(
            "This recorded conversation can be reopened to continue from its earlier context.",
            "可以重新打开此历史会话，从之前的上下文继续。",
        ),
    }
    .into()
}

fn recommendation_description(
    item: &ProjectWorkItem,
    original_step: &str,
    step: &str,
    activity: &ActivityBatch,
    language: TitleLanguage,
) -> String {
    let context = item
        .latest_update
        .as_deref()
        .filter(|text| !text.contains(original_step.trim_end_matches(['。', '.'])))
        .and_then(|text| activity.localization.get(text, language))
        .map(|text| plain_excerpt(text, 200))
        .unwrap_or_default();
    if language == TitleLanguage::Chinese {
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

fn followup_prompt(step: &str, language: TitleLanguage) -> String {
    if language == TitleLanguage::Chinese {
        format!("请在当前会话中继续推进这项建议：\n\n{step}\n\n结合已有进展执行，避免重复已完成的工作，并汇报结果。进展总结、下一步建议和回复都请使用简体中文。")
    } else {
        format!("Continue in this conversation with this suggested follow-up:\n\n{step}\n\nUse the existing progress, avoid repeating completed work, and report the result. Use English for all progress updates, summaries, next steps and replies.")
    }
}

fn push_suggestion(suggestions: &mut Vec<AdvanceSuggestion>, suggestion: AdvanceSuggestion) {
    if suggestions.len() < 3
        && !suggestions
            .iter()
            .any(|existing| existing.id == suggestion.id)
    {
        suggestions.push(suggestion);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::projects::{IndexedSessionSummary, SessionClass, SessionRefKind, TopicCover};

    fn build_overview(
        project: &ProjectSummary,
        runtime: &HashMap<String, WorkPhase>,
        activity: &ActivityBatch,
    ) -> ProjectOverview {
        super::build_overview(project, runtime, activity, TitleLanguage::English)
    }

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
        assert_eq!(overview.suggestions[0].title, "Reply in this conversation");
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
            ..Default::default()
        };
        let overview = build_overview(&project, &HashMap::new(), &activity);
        assert_eq!(
            overview.suggestions[0].source,
            SuggestionSource::Conversation
        );
        assert_eq!(
            overview.suggestions[0].session_key.as_deref(),
            Some("session-2")
        );
        assert_eq!(overview.suggestions.len(), 3);
        assert!(overview
            .suggestions
            .iter()
            .all(|suggestion| suggestion.session_key.is_some()));
        // A small group still exposes authored plan provenance through the public API.
        project
            .sessions
            .retain(|session| session.stable_key == "session-2");
        let smaller = build_overview(&project, &HashMap::new(), &activity);
        assert_eq!(smaller.suggestions[1].source, SuggestionSource::SavedPlan);
        assert_eq!(
            smaller.suggestions[1].description,
            "Call the first customer"
        );
        assert!(smaller.suggestions[1].session_key.is_none());
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
            ..Default::default()
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

    #[test]
    fn configured_language_controls_prose_and_prompts_without_changing_action_identity() {
        let project = fixture_project();
        let request = "Finish the consent review.";
        let response = "The invitations are ready for review.";
        let step = "Submit for review, then deploy after approval.";
        let activity = ActivityBatch {
            sessions: vec![super::super::activity::SessionActivity {
                session_key: "session-0".into(),
                latest_request: Some(request.into()),
                latest_update: Some(response.into()),
                next_steps: vec![step.into()],
                ..Default::default()
            }],
            localization: super::super::localization::LocalizedText {
                language: TitleLanguage::Chinese,
                values: HashMap::from([
                    (request.into(), "完成同意书审核。".into()),
                    (response.into(), "邀请函已准备好，可以提交审核。".into()),
                    (step.into(), "提交审核，获批后再部署。".into()),
                ]),
                loading: false,
            },
            ..Default::default()
        };
        let states = HashMap::from([("session-0".into(), WorkPhase::Working)]);
        let zh = super::build_overview(&project, &states, &activity, TitleLanguage::Chinese);
        let en = super::build_overview(&project, &states, &activity, TitleLanguage::English);
        assert_eq!(zh.language, TitleLanguage::Chinese);
        assert_eq!(zh.work[0].description, "正在推进这项请求：完成同意书审核。");
        assert!(zh.suggestions[0]
            .description
            .contains("接下来建议提交审核，获批后再部署"));
        assert!(zh.suggestions[0]
            .prompt
            .as_deref()
            .unwrap()
            .contains("回复都请使用简体中文"));
        assert!(en.work[0].description.contains(request));
        assert!(en.suggestions[0]
            .prompt
            .as_deref()
            .unwrap()
            .contains("Use English for all progress updates"));
        assert_eq!(zh.suggestions[0].id, en.suggestions[0].id);
        assert_eq!(activity.sessions[0].next_steps, [step]);
        // Cached translations cannot leak back into English mode.
        for work in &en.work {
            assert!(!work.description.contains('邀'));
        }
        assert_eq!(zh.work[0].latest_update.as_deref(), Some(response));
    }

    #[test]
    fn opposite_language_evidence_is_read_only_until_a_valid_translation_is_available() {
        let project = fixture_project();
        for (language, response, step) in [
            (
                TitleLanguage::Chinese,
                "The change passed checks.",
                "Submit for review.",
            ),
            (TitleLanguage::English, "变更已通过检查。", "提交审核。"),
        ] {
            let activity = ActivityBatch {
                sessions: vec![super::super::activity::SessionActivity {
                    session_key: "session-0".into(),
                    latest_update: Some(response.into()),
                    next_steps: vec![step.into()],
                    ..Default::default()
                }],
                ..Default::default()
            };
            let overview = super::build_overview(&project, &HashMap::new(), &activity, language);
            assert!(overview.suggestions[0].prompt.is_none());
            assert_eq!(
                overview.suggestions[0].session_key.as_deref(),
                Some("session-0")
            );
            assert!(crate::projects::localization::matches_language(
                &overview.work[0].description,
                language
            ));
            assert!(crate::projects::localization::matches_language(
                &overview.suggestions[0].description,
                language
            ));
            assert!(!overview.suggestions[0].description.contains(step));
        }
    }

    #[test]
    fn english_summary_translates_chinese_evidence_and_keeps_complete_followup() {
        let project = fixture_project();
        let mut activity = ActivityBatch {
            sessions: vec![super::super::activity::SessionActivity {
                session_key: "session-0".into(),
                latest_update: Some("检查已通过。".into()),
                next_steps: vec!["请先提交审核，获得批准后再部署。".into()],
                ..Default::default()
            }],
            ..Default::default()
        };
        activity.localization.values = HashMap::from([
            ("检查已通过。".into(), "The checks passed.".into()),
            (
                "请先提交审核，获得批准后再部署。".into(),
                "Submit for review first, then deploy after approval.".into(),
            ),
        ]);
        let overview =
            super::build_overview(&project, &HashMap::new(), &activity, TitleLanguage::English);
        assert_eq!(overview.work[0].description, "The checks passed.");
        assert!(overview.suggestions[0]
            .prompt
            .as_deref()
            .unwrap()
            .contains("Submit for review first, then deploy after approval."));
        for suggestion in overview.suggestions {
            assert!(crate::projects::localization::matches_language(
                &suggestion.description,
                TitleLanguage::English
            ));
        }
    }
}
