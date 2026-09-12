use std::collections::HashMap;

use crate::config::TitleLanguage;
use crate::projects::followup::{FollowupState, ProjectFollowup};

use super::App;

const MAX_FOLLOWUPS: usize = 128;
const MAX_PENDING: usize = 16;

struct FollowupTask {
    result: ProjectFollowup,
    terminal_id: crate::terminal::TerminalId,
    pane_id: crate::layout::PaneId,
    backend: String,
    prompt: String,
    awaiting_agent: bool,
    message: FollowupMessage,
}

enum FollowupMessage {
    Queued,
    Sent,
    Cancelled,
    AgentExited,
    TargetChanged,
}

impl FollowupMessage {
    fn text(&self, language: TitleLanguage) -> &'static str {
        match self {
            Self::Queued => language.text(
                "Queued for the original Agent; it will be sent when the Agent is ready.",
                "已加入原 Agent 的跟进队列，空闲后会自动发送。",
            ),
            Self::Sent => language.text(
                "Follow-up sent to the original Agent.",
                "跟进指令已发送给原 Agent。",
            ),
            Self::Cancelled => language.text(
                "Cancelled before the follow-up was sent.",
                "已取消，跟进指令未发送。",
            ),
            Self::AgentExited => language.text(
                "The original Agent exited before the follow-up was sent.",
                "原 Agent 已退出，跟进指令未发送。",
            ),
            Self::TargetChanged => language.text(
                "The original session or Agent changed; the follow-up was not sent.",
                "原会话或 Agent 已改变，跟进指令未发送。",
            ),
        }
    }
}

impl FollowupTask {
    fn localized_result(&self, language: TitleLanguage) -> ProjectFollowup {
        let mut result = self.result.clone();
        result.message = self.message.text(language).into();
        result
    }
}

#[derive(Default)]
pub(crate) struct ProjectFollowupQueue {
    tasks: HashMap<String, FollowupTask>,
}

impl App {
    pub(crate) fn project_followup(&self, id: &str) -> Option<ProjectFollowup> {
        self.project_followups
            .tasks
            .get(id)
            .map(|task| task.localized_result(self.state.title_language))
    }

    pub(crate) fn start_project_followup(
        &mut self,
        project_key: &str,
        suggestion_id: &str,
    ) -> Result<ProjectFollowup, (&'static str, String)> {
        if let Some(existing) = self.project_followup(suggestion_id) {
            if existing.project_key != project_key {
                return Err((
                    "not_found",
                    self.state
                        .title_language
                        .text(
                            "This follow-up belongs to another project.",
                            "这条跟进属于另一个项目。",
                        )
                        .into(),
                ));
            }
            return Ok(existing);
        }
        let project = self
            .project_service
            .project_summary(project_key)
            .map_err(|error| {
                (
                    error.code,
                    self.state
                        .title_language
                        .text(&error.message, "无法读取此项目，请刷新后重试。")
                        .into(),
                )
            })?;
        let activity = self.project_service.recent_activity(&project, false);
        let overview = self.state.project_overview(&project, &activity);
        let suggestion = overview
            .suggestions
            .iter()
            .find(|item| item.id == suggestion_id)
            .ok_or((
                "conflict",
                self.state
                    .title_language
                    .text(
                        "This suggestion has changed. Refresh the project and choose it again.",
                        "这条建议已更新，请刷新项目后重新选择。",
                    )
                    .into(),
            ))?;
        let prompt = suggestion.prompt.clone().ok_or((
            "invalid_params",
            self.state
                .title_language
                .text(
                    "This suggestion needs your answer or a plan edit. Open it to continue.",
                    "这条建议暂时不能直接执行，请打开会话查看或编辑计划。",
                )
                .into(),
        ))?;
        let session = project
            .sessions
            .iter()
            .find(|item| Some(&item.stable_key) == suggestion.session_key.as_ref())
            .ok_or((
                "not_found",
                self.state
                    .title_language
                    .text(
                        "The original conversation is no longer available.",
                        "原会话已不可用。",
                    )
                    .into(),
            ))?
            .clone();
        self.queue_project_followup(
            project_key,
            suggestion_id,
            &session,
            &suggestion.title,
            prompt,
        )
    }

    fn queue_project_followup(
        &mut self,
        project_key: &str,
        suggestion_id: &str,
        session: &crate::projects::IndexedSessionSummary,
        title: &str,
        prompt: String,
    ) -> Result<ProjectFollowup, (&'static str, String)> {
        if self
            .project_followups
            .tasks
            .values()
            .filter(|task| task.result.state == FollowupState::Queued)
            .count()
            >= MAX_PENDING
        {
            return Err((
                "busy",
                self.state
                    .title_language
                    .text(
                        "Wait for an earlier follow-up to be delivered before adding another.",
                        "请等待已有跟进发送后再添加新的建议。",
                    )
                    .into(),
            ));
        }
        if self.project_followups.tasks.len() >= MAX_FOLLOWUPS {
            let oldest = self
                .project_followups
                .tasks
                .iter()
                .filter(|(_, task)| task.result.state != FollowupState::Queued)
                .min_by_key(|(_, task)| task.result.created_at)
                .map(|(id, _)| id.clone());
            if let Some(id) = oldest {
                self.project_followups.tasks.remove(&id);
            }
        }
        let target = self
            .project_runtime_leases
            .iter()
            .find(|(_, lease)| lease.session_key == session.stable_key)
            .and_then(|(pane_id, _)| self.find_pane(*pane_id).map(|(index, _)| (index, *pane_id)))
            .or_else(|| self.pane_for_catalog_session(session));
        let (ws_idx, pane_id, mut awaiting_agent) = match target {
            Some((index, pane_id)) => (index, pane_id, false),
            None => {
                if session.ref_value.starts_with("herduck-live:") {
                    return Err((
                        "not_found",
                        self.state
                            .title_language
                            .text(
                                "This Agent no longer has a running conversation to continue.",
                                "此 Agent 已没有可继续的运行中会话。",
                            )
                            .into(),
                    ));
                }
                if session
                    .cwd
                    .as_deref()
                    .is_none_or(|path| !std::path::Path::new(path).is_dir())
                {
                    return Err(("resume_failed", self.state.title_language.text("The original working directory is unavailable. Open the conversation to choose where to continue.", "原工作目录已不可用，请打开会话选择继续的位置。").into()));
                }
                let (index, pane_id) =
                    self.spawn_catalog_session_resume(session)
                        .map_err(|reason| {
                            (
                                "resume_failed",
                                self.state
                                    .title_language
                                    .text(
                                        &reason,
                                        "无法恢复原会话，请检查 Agent 和工作目录后重试。",
                                    )
                                    .into(),
                            )
                        })?;
                (index, pane_id, true)
            }
        };
        self.sync_project_runtime_for_pane(pane_id, true);
        let terminal_id = self
            .state
            .workspaces
            .get(ws_idx)
            .and_then(|workspace| workspace.terminal_id(pane_id))
            .cloned()
            .ok_or((
                "not_found",
                self.state
                    .title_language
                    .text(
                        "The original pane is no longer available.",
                        "原会话窗口已不可用。",
                    )
                    .into(),
            ))?;
        if self.terminal_runtimes.get(&terminal_id).is_none() {
            let (rows, cols) = self.state.estimate_pane_size();
            if !self.start_pending_agent_resume_for_terminal(&terminal_id, rows, cols, true) {
                return Err((
                    "resume_failed",
                    self.state
                        .title_language
                        .text(
                            "The original Agent could not be resumed in its pane.",
                            "无法在原窗口恢复此 Agent。",
                        )
                        .into(),
                ));
            }
            awaiting_agent = true;
            if let Some(terminal) = self.state.terminals.get_mut(&terminal_id) {
                terminal.state = crate::detect::AgentState::Working;
                terminal.fallback_state = crate::detect::AgentState::Working;
            }
        }
        if self
            .project_followups
            .tasks
            .values()
            .any(|task| task.pane_id == pane_id && task.result.state == FollowupState::Queued)
            || self.pending_catalog_submissions.contains_key(&pane_id)
        {
            return Err((
                "busy",
                self.state
                    .title_language
                    .text(
                        "This conversation already has a follow-up waiting to be delivered.",
                        "此会话已有一条等待发送的跟进。",
                    )
                    .into(),
            ));
        }
        let public_pane_id = self.public_pane_id(ws_idx, pane_id).ok_or((
            "not_found",
            self.state
                .title_language
                .text(
                    "The original pane is no longer available.",
                    "原会话窗口已不可用。",
                )
                .into(),
        ))?;
        let result = ProjectFollowup {
            id: suggestion_id.to_string(),
            project_key: project_key.to_string(),
            session_key: session.stable_key.clone(),
            pane_id: public_pane_id,
            title: title.to_string(),
            state: FollowupState::Queued,
            message: String::new(),
            created_at: crate::projects::runtime::unix_time_ms(),
        };
        self.project_followups.tasks.insert(
            suggestion_id.to_string(),
            FollowupTask {
                result,
                terminal_id,
                pane_id,
                backend: session.backend.clone(),
                prompt,
                awaiting_agent,
                message: FollowupMessage::Queued,
            },
        );
        self.flush_project_followups();
        self.project_followup(suggestion_id).ok_or((
            "internal_error",
            self.state
                .title_language
                .text(
                    "Could not retain the follow-up status.",
                    "无法保留跟进状态。",
                )
                .into(),
        ))
    }

    pub(crate) fn cancel_project_followups(&mut self, pane_id: crate::layout::PaneId) {
        for task in
            self.project_followups.tasks.values_mut().filter(|task| {
                task.pane_id == pane_id && task.result.state == FollowupState::Queued
            })
        {
            task.result.state = FollowupState::Cancelled;
            task.message = FollowupMessage::AgentExited;
            task.prompt.clear();
        }
    }

    pub(crate) fn cancel_project_followup(&mut self, id: &str) -> Option<ProjectFollowup> {
        let task = self.project_followups.tasks.get_mut(id)?;
        if task.result.state == FollowupState::Queued {
            task.result.state = FollowupState::Cancelled;
            task.message = FollowupMessage::Cancelled;
            task.prompt.clear();
        }
        Some(task.localized_result(self.state.title_language))
    }

    pub(crate) fn annotate_project_followups(
        &self,
        overview: &mut crate::projects::overview::ProjectOverview,
    ) {
        for suggestion in &mut overview.suggestions {
            suggestion.followup = self.project_followup(&suggestion.id).or_else(|| {
                self.project_followups
                    .tasks
                    .values()
                    .find(|task| {
                        task.result.state == FollowupState::Queued
                            && task.result.project_key == overview.project_key
                            && Some(&task.result.session_key) == suggestion.session_key.as_ref()
                    })
                    .map(|task| task.localized_result(self.state.title_language))
            });
        }
    }

    pub(crate) fn flush_project_followups(&mut self) {
        let pending: Vec<_> = self
            .project_followups
            .tasks
            .iter()
            .filter(|(_, task)| task.result.state == FollowupState::Queued)
            .map(|(id, task)| {
                (
                    id.clone(),
                    task.pane_id,
                    task.terminal_id.clone(),
                    task.backend.clone(),
                    task.result.session_key.clone(),
                    task.awaiting_agent,
                )
            })
            .collect();
        for (id, pane_id, terminal_id, backend, session_key, awaiting_agent) in pending {
            let target = self.find_pane(pane_id).and_then(|(index, pane)| {
                if pane.attached_terminal_id != terminal_id {
                    return None;
                }
                let terminal = self.state.terminals.get(&terminal_id)?;
                let identity_matches = self
                    .project_runtime_leases
                    .get(&pane_id)
                    .is_some_and(|lease| lease.session_key == session_key);
                let label = terminal.effective_agent_label();
                let confirmed = label == Some(backend.as_str());
                // A native resume has a bound session before its process or hook
                // can be detected. Wait for that first observation, but never
                // write to an unidentified process or accept a different Agent.
                (identity_matches && (confirmed || (awaiting_agent && label.is_none())))
                    .then_some((index, terminal.state, confirmed))
            });
            let Some((ws_idx, state, confirmed)) = target else {
                if let Some(task) = self.project_followups.tasks.get_mut(&id) {
                    task.result.state = FollowupState::Cancelled;
                    task.message = FollowupMessage::TargetChanged;
                    task.prompt.clear();
                }
                continue;
            };
            if !confirmed {
                continue;
            }
            if let Some(task) = self.project_followups.tasks.get_mut(&id) {
                task.awaiting_agent = false;
            }
            if state != crate::detect::AgentState::Idle {
                continue;
            }
            let Some(task) = self.project_followups.tasks.get(&id) else {
                continue;
            };
            let Some(runtime) =
                self.state
                    .runtime_for_pane_in_workspace(&self.terminal_runtimes, ws_idx, pane_id)
            else {
                continue;
            };
            let mut payload = if runtime
                .input_state()
                .is_some_and(|input| input.bracketed_paste)
            {
                format!("\x1b[200~{}\x1b[201~", task.prompt).into_bytes()
            } else {
                // All instructions are one logical input even on Agents without bracketed paste.
                crate::projects::activity::plain_excerpt(&task.prompt, 8_000).into_bytes()
            };
            payload.extend(runtime.encode_terminal_key(crate::input::TerminalKey::new(
                crossterm::event::KeyCode::Enter,
                crossterm::event::KeyModifiers::empty(),
            )));
            match runtime.try_send_bytes(bytes::Bytes::from(payload)) {
                Ok(()) => {
                    if let Some(task) = self.project_followups.tasks.get_mut(&id) {
                        task.result.state = FollowupState::Sent;
                        task.message = FollowupMessage::Sent;
                        task.prompt.clear();
                    }
                }
                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                    self.cancel_project_followups(pane_id);
                }
                Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::{Agent, AgentState};

    fn fixture(
        state: AgentState,
    ) -> (
        App,
        crate::projects::IndexedSessionSummary,
        tokio::sync::mpsc::Receiver<bytes::Bytes>,
    ) {
        let (_, api_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(
            &crate::config::Config::default(),
            true,
            None,
            api_rx,
            crate::api::EventHub::default(),
        );
        app.project_service = crate::projects::ProjectService::disabled();
        app.state.workspaces = vec![crate::workspace::Workspace::test_new("followup-test")];
        app.state.ensure_test_terminals();
        app.state.active = Some(0);
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        let terminal_id = app.state.terminal_id_for_pane(0, pane_id).unwrap();
        let terminal = app.state.terminals.get_mut(&terminal_id).unwrap();
        terminal.detected_agent = Some(Agent::Codex);
        terminal.state = state;
        let (runtime, receiver) = crate::terminal::TerminalRuntime::test_with_channel(80, 24);
        app.terminal_runtimes.insert(terminal_id, runtime);
        let mut session = crate::projects::overview::tests::fixture_project()
            .sessions
            .remove(0);
        session.backend = "codex".into();
        session.workspace_id = Some(app.public_workspace_id(0));
        session.pane_id = Some(app.public_pane_id(0, pane_id).unwrap());
        app.project_runtime_leases.insert(
            pane_id,
            crate::projects::runtime::RuntimeLease {
                session_key: session.stable_key.clone(),
                workspace_id: session.workspace_id.clone().unwrap(),
                pane_id: session.pane_id.clone().unwrap(),
                generation: 1,
                observed_at: 1,
            },
        );
        (app, session, receiver)
    }

    #[tokio::test]
    async fn explicit_followup_reuses_the_idle_agent_and_retries_are_idempotent() {
        let (mut app, session, mut receiver) = fixture(AgentState::Idle);
        let before = app.state.workspaces[0].tabs.len();
        let result = app
            .queue_project_followup(
                "project",
                "suggestion",
                &session,
                "Submit review",
                "Submit for review, then redeploy after approval.".into(),
            )
            .unwrap();
        assert_eq!(result.state, FollowupState::Sent);
        assert_eq!(
            receiver.try_recv().unwrap(),
            "Submit for review, then redeploy after approval.\r"
        );
        assert_eq!(
            app.start_project_followup("project", "suggestion").unwrap(),
            result
        );
        app.flush_project_followups();
        assert!(receiver.try_recv().is_err());
        assert_eq!(app.state.workspaces[0].tabs.len(), before);
        app.state.assert_invariants_for_test();
    }

    #[tokio::test]
    async fn changing_language_localizes_delivery_status_without_repeating_or_rewriting_input() {
        let (mut app, session, mut receiver) = fixture(AgentState::Working);
        app.state.title_language = TitleLanguage::Chinese;
        let instruction = "提交审核，获批后再部署。进展总结和回复都请使用简体中文。";
        let queued = app
            .queue_project_followup(
                "project",
                "same-instruction",
                &session,
                "提交审核",
                instruction.into(),
            )
            .unwrap();
        assert_eq!(
            queued.message,
            "已加入原 Agent 的跟进队列，空闲后会自动发送。"
        );
        app.state.title_language = TitleLanguage::English;
        let again = app
            .start_project_followup("project", "same-instruction")
            .unwrap();
        assert!(again.message.starts_with("Queued for the original Agent"));
        assert!(receiver.try_recv().is_err());
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        let terminal_id = app.state.terminal_id_for_pane(0, pane_id).unwrap();
        app.state.terminals.get_mut(&terminal_id).unwrap().state = AgentState::Idle;
        app.flush_project_followups();
        assert_eq!(receiver.try_recv().unwrap(), format!("{instruction}\r"));
        assert_eq!(
            app.project_followup("same-instruction").unwrap().message,
            "Follow-up sent to the original Agent."
        );
        app.state.title_language = TitleLanguage::Chinese;
        assert_eq!(
            app.start_project_followup("project", "same-instruction")
                .unwrap()
                .message,
            "跟进指令已发送给原 Agent。"
        );
        app.flush_project_followups();
        assert!(receiver.try_recv().is_err());
        let error = app
            .start_project_followup("different-project", "same-instruction")
            .unwrap_err();
        assert_eq!(error.1, "这条跟进属于另一个项目。");
        app.state.assert_invariants_for_test();
    }

    #[tokio::test]
    async fn busy_and_blocked_agents_receive_nothing_until_they_are_ready() {
        for state in [
            AgentState::Working,
            AgentState::Blocked,
            AgentState::Unknown,
        ] {
            let (mut app, session, mut receiver) = fixture(state);
            let result = app
                .queue_project_followup(
                    "project",
                    "suggestion",
                    &session,
                    "Continue",
                    "Continue the reviewed plan.".into(),
                )
                .unwrap();
            assert_eq!(result.state, FollowupState::Queued);
            assert!(receiver.try_recv().is_err());
            let pane_id = app.state.workspaces[0].tabs[0].root_pane;
            let terminal_id = app.state.terminal_id_for_pane(0, pane_id).unwrap();
            app.state.terminals.get_mut(&terminal_id).unwrap().state = AgentState::Idle;
            app.flush_project_followups();
            assert_eq!(
                receiver.try_recv().unwrap(),
                "Continue the reviewed plan.\r"
            );
            assert_eq!(
                app.project_followup("suggestion").unwrap().state,
                FollowupState::Sent
            );
            app.flush_project_followups();
            assert!(receiver.try_recv().is_err());
        }
    }

    #[tokio::test]
    async fn a_queued_followup_can_be_cancelled_without_sending_input() {
        let (mut app, session, mut receiver) = fixture(AgentState::Working);
        app.queue_project_followup(
            "project",
            "suggestion",
            &session,
            "Continue",
            "Don't send this after cancellation.".into(),
        )
        .unwrap();
        assert_eq!(
            app.cancel_project_followup("suggestion").unwrap().state,
            FollowupState::Cancelled
        );
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        let terminal_id = app.state.terminal_id_for_pane(0, pane_id).unwrap();
        app.state.terminals.get_mut(&terminal_id).unwrap().state = AgentState::Idle;
        app.flush_project_followups();
        assert!(receiver.try_recv().is_err());
    }

    #[tokio::test]
    async fn native_resume_waits_for_expected_agent_identity_before_idle_delivery() {
        for lose_identity_after_detection in [false, true] {
            let (mut app, session, mut receiver) = fixture(AgentState::Working);
            app.queue_project_followup(
                "project",
                "suggestion",
                &session,
                "Continue",
                "Continue after the original Agent is ready.".into(),
            )
            .unwrap();
            let pane_id = app.state.workspaces[0].tabs[0].root_pane;
            let terminal_id = app.state.terminal_id_for_pane(0, pane_id).unwrap();
            app.project_followups
                .tasks
                .get_mut("suggestion")
                .unwrap()
                .awaiting_agent = true;
            let terminal = app.state.terminals.get_mut(&terminal_id).unwrap();
            terminal.detected_agent = None;
            terminal.state = AgentState::Idle;
            app.flush_project_followups();
            assert_eq!(
                app.project_followup("suggestion").unwrap().state,
                FollowupState::Queued
            );
            assert!(receiver.try_recv().is_err());
            let terminal = app.state.terminals.get_mut(&terminal_id).unwrap();
            terminal.detected_agent = Some(Agent::Codex);
            terminal.state = AgentState::Working;
            app.flush_project_followups();
            assert!(!app.project_followups.tasks["suggestion"].awaiting_agent);
            assert!(receiver.try_recv().is_err());
            let terminal = app.state.terminals.get_mut(&terminal_id).unwrap();
            terminal.state = AgentState::Idle;
            if lose_identity_after_detection {
                terminal.detected_agent = None;
            }
            app.flush_project_followups();
            if lose_identity_after_detection {
                assert_eq!(
                    app.project_followup("suggestion").unwrap().state,
                    FollowupState::Cancelled
                );
                assert!(receiver.try_recv().is_err());
            } else {
                assert_eq!(
                    app.project_followup("suggestion").unwrap().state,
                    FollowupState::Sent
                );
                assert_eq!(
                    receiver.try_recv().unwrap(),
                    "Continue after the original Agent is ready.\r"
                );
            }
        }
    }

    #[tokio::test]
    async fn changed_session_or_agent_never_receives_the_previous_followup() {
        for change_agent in [false, true] {
            let (mut app, session, mut receiver) = fixture(AgentState::Working);
            app.queue_project_followup(
                "project",
                "suggestion",
                &session,
                "Continue",
                "Original conversation only.".into(),
            )
            .unwrap();
            let pane_id = app.state.workspaces[0].tabs[0].root_pane;
            let terminal_id = app.state.terminal_id_for_pane(0, pane_id).unwrap();
            app.state.terminals.get_mut(&terminal_id).unwrap().state = AgentState::Idle;
            if change_agent {
                app.state
                    .terminals
                    .get_mut(&terminal_id)
                    .unwrap()
                    .detected_agent = Some(Agent::Claude);
            } else {
                app.project_runtime_leases
                    .get_mut(&pane_id)
                    .unwrap()
                    .session_key = "another-session".into();
            }
            app.flush_project_followups();
            assert_eq!(
                app.project_followup("suggestion").unwrap().state,
                FollowupState::Cancelled
            );
            assert!(receiver.try_recv().is_err());
        }
    }

    #[tokio::test]
    async fn replaced_terminal_or_removed_pane_cancels_the_followup() {
        for replace_terminal in [false, true] {
            let (mut app, session, mut receiver) = fixture(AgentState::Working);
            app.queue_project_followup(
                "project",
                "suggestion",
                &session,
                "Continue",
                "Original terminal only.".into(),
            )
            .unwrap();
            let pane_id = app.state.workspaces[0].tabs[0].root_pane;
            if replace_terminal {
                let replacement = crate::terminal::TerminalId::alloc();
                app.state.workspaces[0].tabs[0]
                    .panes
                    .get_mut(&pane_id)
                    .unwrap()
                    .attached_terminal_id = replacement;
            } else {
                app.state.workspaces.clear();
            }
            app.flush_project_followups();
            assert_eq!(
                app.project_followup("suggestion").unwrap().state,
                FollowupState::Cancelled
            );
            assert!(receiver.try_recv().is_err());
        }
    }

    #[tokio::test]
    async fn inactive_agent_is_continued_in_the_same_runtime() {
        let (mut app, session, mut receiver) = fixture(AgentState::Idle);
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        let terminal_id = app.state.terminal_id_for_pane(0, pane_id).unwrap();
        app.state
            .terminals
            .get_mut(&terminal_id)
            .unwrap()
            .set_agent_inactive(true);
        let result = app
            .queue_project_followup(
                "project",
                "suggestion",
                &session,
                "Continue",
                "Continue here.".into(),
            )
            .unwrap();
        assert_eq!(result.state, FollowupState::Sent);
        assert_eq!(receiver.try_recv().unwrap(), "Continue here.\r");
        assert_eq!(
            app.state.terminal_id_for_pane(0, pane_id),
            Some(terminal_id)
        );
    }

    #[tokio::test]
    async fn full_input_channel_retries_one_atomic_submission_and_closed_channel_cancels() {
        let (mut app, session, mut receiver) = fixture(AgentState::Idle);
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        let terminal_id = app.state.terminal_id_for_pane(0, pane_id).unwrap();
        let capacity = receiver.capacity();
        for _ in 0..capacity {
            app.terminal_runtimes
                .get(&terminal_id)
                .unwrap()
                .try_send_bytes(bytes::Bytes::from_static(b"x"))
                .unwrap();
        }
        assert_eq!(
            app.queue_project_followup(
                "project",
                "suggestion",
                &session,
                "Continue",
                "One message.".into()
            )
            .unwrap()
            .state,
            FollowupState::Queued
        );
        for _ in 0..capacity {
            assert_eq!(receiver.try_recv().unwrap(), "x");
        }
        app.flush_project_followups();
        assert_eq!(receiver.try_recv().unwrap(), "One message.\r");
        app.flush_project_followups();
        assert!(receiver.try_recv().is_err());
        drop(receiver);
        assert_eq!(
            app.queue_project_followup(
                "project",
                "another",
                &session,
                "Continue",
                "No receiver.".into()
            )
            .unwrap()
            .state,
            FollowupState::Cancelled
        );
    }

    #[tokio::test]
    async fn stale_suggestion_is_rejected_before_any_runtime_is_created() {
        let (mut app, _, mut receiver) = fixture(AgentState::Idle);
        let (service, key) =
            crate::projects::ProjectService::with_test_topic(crate::api::EventHub::default());
        app.project_service = service;
        let before = app.state.workspaces[0].tabs.len();
        assert_eq!(
            app.start_project_followup(&key, "stale-suggestion")
                .unwrap_err()
                .0,
            "conflict"
        );
        assert_eq!(app.state.workspaces[0].tabs.len(), before);
        assert!(receiver.try_recv().is_err());
    }
}
