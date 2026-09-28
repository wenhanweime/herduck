use std::time::Instant;

use bytes::Bytes;
use ratatui::layout::Rect;

use super::App;

struct PendingAgentResumeCandidate {
    pane_id: crate::layout::PaneId,
    terminal_id: crate::terminal::TerminalId,
    cwd: std::path::PathBuf,
    plan: crate::agent_resume::AgentResumePlan,
    rows: u16,
    cols: u16,
}

impl App {
    /// Wakes a greyed Agent. Dormant rows never enter the automatic restore
    /// queue; a user focus, click, or typed/pasted input may send the native
    /// resume command. Typed content is held until the CLI reaches idle.
    pub(crate) fn activate_dormant_agent_for_pane(
        &mut self,
        ws_idx: usize,
        pane_id: crate::layout::PaneId,
    ) -> bool {
        let Some(pane) = self
            .state
            .workspaces
            .get(ws_idx)
            .and_then(|ws| ws.panes.get(&pane_id))
        else {
            return false;
        };
        let terminal_id = pane.attached_terminal_id.clone();
        let Some(session) = self
            .state
            .terminals
            .get(&terminal_id)
            .and_then(|terminal| terminal.dormant_agent_session.clone())
        else {
            return false;
        };
        let Some(plan) =
            crate::agent_resume::plan(&session.source, &session.agent, &session.session_ref)
        else {
            return false;
        };
        let (rows, cols) = self
            .state
            .view
            .pane_infos
            .iter()
            .find(|info| info.id == pane_id)
            .map(|info| (info.inner_rect.height.max(1), info.inner_rect.width.max(1)))
            .unwrap_or((24, 80));

        if let Some(runtime) = self.terminal_runtimes.get(&terminal_id) {
            // A TERM grace period may still be in flight. Never type the resume command
            // into the old Agent; leave the row grey and let a later explicit click retry.
            if runtime
                .child_pid()
                .and_then(crate::detect::foreground_job)
                .and_then(|job| crate::detect::identify_agent_in_job(&job))
                .is_some()
            {
                return false;
            }
            let Some(command) = shell_command_from_argv(&plan.argv) else {
                return false;
            };
            let mut input = command;
            input.push('\r');
            if runtime.try_send_bytes(Bytes::from(input)).is_err() {
                return false;
            }
            if let Some(terminal) = self.state.terminals.get_mut(&terminal_id) {
                terminal.clear_dormant_agent();
                terminal.set_persisted_agent_session(session);
                terminal.set_agent_name(plan.agent.clone());
                terminal.state = crate::detect::AgentState::Working;
                terminal.fallback_state = crate::detect::AgentState::Working;
            }
            self.ensure_dormant_wake_draft(pane_id);
            self.schedule_session_save();
            return true;
        }

        if let Some(terminal) = self.state.terminals.get_mut(&terminal_id) {
            terminal.pending_agent_resume_plan = Some(plan);
        }
        if self.start_pending_agent_resume_for_terminal(&terminal_id, rows, cols, true) {
            if let Some(terminal) = self.state.terminals.get_mut(&terminal_id) {
                terminal.clear_dormant_agent();
                terminal.set_persisted_agent_session(session.clone());
                terminal.set_agent_name(session.agent);
            }
            self.ensure_dormant_wake_draft(pane_id);
            self.schedule_session_save();
            return true;
        }
        false
    }

    pub(crate) fn pane_intercepts_dormant_wake(
        &self,
        ws_idx: usize,
        pane_id: crate::layout::PaneId,
    ) -> bool {
        if self.state.dormant_wake_drafts.contains_key(&pane_id) {
            return true;
        }
        self.state
            .workspaces
            .get(ws_idx)
            .and_then(|ws| ws.panes.get(&pane_id))
            .and_then(|pane| self.state.terminals.get(&pane.attached_terminal_id))
            .is_some_and(|terminal| terminal.dormant_agent_session.is_some())
    }

    pub(crate) fn ensure_dormant_wake_draft(&mut self, pane_id: crate::layout::PaneId) {
        self.state
            .dormant_wake_drafts
            .entry(pane_id)
            .or_insert_with(|| crate::app::state::DormantWakeDraft {
                text: String::new(),
                submitted: false,
            });
    }

    pub(crate) fn handle_dormant_wake_key(
        &mut self,
        ws_idx: usize,
        pane_id: crate::layout::PaneId,
        key: crate::input::TerminalKey,
    ) {
        if key.kind == crossterm::event::KeyEventKind::Release {
            return;
        }
        self.ensure_dormant_wake_draft(pane_id);
        if self
            .state
            .dormant_wake_drafts
            .get(&pane_id)
            .is_some_and(|draft| draft.submitted)
        {
            return;
        }
        let dormant = self
            .state
            .workspaces
            .get(ws_idx)
            .and_then(|ws| ws.panes.get(&pane_id))
            .and_then(|pane| self.state.terminals.get(&pane.attached_terminal_id))
            .is_some_and(|terminal| terminal.dormant_agent_session.is_some());
        if dormant {
            let _ = self.activate_dormant_agent_for_pane(ws_idx, pane_id);
        }

        match key.code {
            crossterm::event::KeyCode::Esc => {
                if let Some(draft) = self.state.dormant_wake_drafts.get_mut(&pane_id) {
                    draft.text.clear();
                }
            }
            crossterm::event::KeyCode::Backspace => {
                if let Some(draft) = self.state.dormant_wake_drafts.get_mut(&pane_id) {
                    draft.text.pop();
                }
            }
            crossterm::event::KeyCode::Enter
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT) =>
            {
                if let Some(draft) = self.state.dormant_wake_drafts.get_mut(&pane_id) {
                    draft.text.push('\n');
                }
            }
            crossterm::event::KeyCode::Enter => {
                self.submit_dormant_wake_draft(ws_idx, pane_id);
            }
            crossterm::event::KeyCode::Char(character)
                if !key.modifiers.intersects(
                    crossterm::event::KeyModifiers::CONTROL
                        | crossterm::event::KeyModifiers::ALT
                        | crossterm::event::KeyModifiers::SUPER,
                ) && !character.is_control() =>
            {
                if let Some(draft) = self.state.dormant_wake_drafts.get_mut(&pane_id) {
                    draft.text.push(character);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn append_dormant_wake_paste(
        &mut self,
        ws_idx: usize,
        pane_id: crate::layout::PaneId,
        text: &str,
    ) {
        self.ensure_dormant_wake_draft(pane_id);
        if self
            .state
            .dormant_wake_drafts
            .get(&pane_id)
            .is_some_and(|draft| draft.submitted)
        {
            return;
        }
        let dormant = self
            .state
            .workspaces
            .get(ws_idx)
            .and_then(|ws| ws.panes.get(&pane_id))
            .and_then(|pane| self.state.terminals.get(&pane.attached_terminal_id))
            .is_some_and(|terminal| terminal.dormant_agent_session.is_some());
        if dormant {
            let _ = self.activate_dormant_agent_for_pane(ws_idx, pane_id);
        }
        if let Some(draft) = self.state.dormant_wake_drafts.get_mut(&pane_id) {
            draft
                .text
                .extend(text.chars().filter(|character| *character != '\r'));
        }
    }

    fn submit_dormant_wake_draft(&mut self, ws_idx: usize, pane_id: crate::layout::PaneId) {
        let dormant = self
            .state
            .workspaces
            .get(ws_idx)
            .and_then(|ws| ws.panes.get(&pane_id))
            .and_then(|pane| self.state.terminals.get(&pane.attached_terminal_id))
            .is_some_and(|terminal| terminal.dormant_agent_session.is_some());
        if dormant && !self.activate_dormant_agent_for_pane(ws_idx, pane_id) {
            return;
        }
        let Some(draft) = self.state.dormant_wake_drafts.get_mut(&pane_id) else {
            return;
        };
        if draft.text.trim().is_empty() {
            return;
        }
        let text = draft.text.clone();
        draft.submitted = true;
        self.pending_catalog_submissions.insert(pane_id, text);
        if let Some(state) = self
            .state
            .workspaces
            .get(ws_idx)
            .and_then(|ws| ws.panes.get(&pane_id))
            .and_then(|pane| self.state.terminals.get(&pane.attached_terminal_id))
            .map(|terminal| terminal.state)
        {
            self.flush_pending_catalog_submission(pane_id, state);
        }
    }

    pub(crate) fn release_idle_dormant_wake_draft(
        &mut self,
        pane_id: crate::layout::PaneId,
        state: crate::detect::AgentState,
    ) {
        if state != crate::detect::AgentState::Idle {
            return;
        }
        let Some(draft) = self.state.dormant_wake_drafts.get(&pane_id) else {
            return;
        };
        if draft.submitted || !draft.text.is_empty() {
            return;
        }
        self.state.dormant_wake_drafts.remove(&pane_id);
    }

    pub(crate) fn has_pending_agent_resumes(&self) -> bool {
        self.state
            .terminals
            .values()
            .any(|terminal| terminal.pending_agent_resume_plan.is_some())
    }

    pub(crate) fn sync_pending_agent_resume_deadline(&mut self, now: Instant) {
        if !self.has_pending_agent_resumes() {
            self.pending_agent_resume_deadline = None;
            return;
        }
        if self.pending_agent_resume_candidates().is_empty() {
            self.pending_agent_resume_deadline = None;
            return;
        }
        self.pending_agent_resume_deadline
            .get_or_insert(now + super::PENDING_AGENT_RESUME_THEME_WAIT);
    }

    pub(crate) fn pending_agent_resume_due(&self, now: Instant) -> bool {
        self.pending_agent_resume_deadline
            .is_some_and(|deadline| now >= deadline)
    }

    pub(crate) fn start_pending_agent_resumes(&mut self, allow_empty_theme: bool) -> bool {
        let pending = self.pending_agent_resume_candidates();
        let mut changed = false;
        for PendingAgentResumeCandidate {
            pane_id,
            terminal_id,
            cwd,
            plan,
            rows,
            cols,
        } in pending
        {
            if self.terminal_runtimes.get(&terminal_id).is_some() {
                continue;
            }
            changed |= self.start_pending_agent_resume(
                pane_id,
                terminal_id,
                cwd,
                plan,
                rows,
                cols,
                allow_empty_theme,
            );
        }

        if changed {
            self.schedule_session_save();
        }
        if !self.has_pending_agent_resumes() || self.pending_agent_resume_candidates().is_empty() {
            self.pending_agent_resume_deadline = None;
        }
        changed
    }

    fn pending_agent_resume_candidates(&self) -> Vec<PendingAgentResumeCandidate> {
        let terminal_area = self.state.view.terminal_area;
        if terminal_area.width == 0 || terminal_area.height == 0 {
            return Vec::new();
        };

        let mut pending = Vec::new();
        for (ws_idx, ws) in self.state.workspaces.iter().enumerate() {
            for (tab_idx, tab) in ws.tabs.iter().enumerate() {
                for info in
                    self.pending_agent_resume_pane_infos(ws_idx, tab_idx, tab, terminal_area)
                {
                    let Some(pane) = tab.panes.get(&info.id) else {
                        continue;
                    };
                    if self
                        .terminal_runtimes
                        .get(&pane.attached_terminal_id)
                        .is_some()
                    {
                        continue;
                    }
                    let Some(terminal) = self.state.terminals.get(&pane.attached_terminal_id)
                    else {
                        continue;
                    };
                    let Some(plan) = terminal.pending_agent_resume_plan.clone() else {
                        continue;
                    };
                    pending.push(PendingAgentResumeCandidate {
                        pane_id: info.id,
                        terminal_id: pane.attached_terminal_id.clone(),
                        cwd: terminal.cwd.clone(),
                        plan,
                        rows: info.inner_rect.height,
                        cols: info.inner_rect.width,
                    });
                }
            }
        }
        pending
    }

    fn pending_agent_resume_pane_infos(
        &self,
        ws_idx: usize,
        tab_idx: usize,
        tab: &crate::workspace::Tab,
        terminal_area: Rect,
    ) -> Vec<crate::layout::PaneInfo> {
        let mut pane_infos = derived_pending_agent_resume_pane_infos(
            tab,
            terminal_area,
            self.state.pane_borders,
            self.state.pane_gaps,
        );

        if self.state.active == Some(ws_idx)
            && self
                .state
                .workspaces
                .get(ws_idx)
                .is_some_and(|ws| tab_idx == ws.active_tab_index())
        {
            for visible_info in &self.state.view.pane_infos {
                if let Some(info) = pane_infos
                    .iter_mut()
                    .find(|info| info.id == visible_info.id)
                {
                    *info = visible_info.clone();
                } else {
                    pane_infos.push(visible_info.clone());
                }
            }
        }

        pane_infos
    }

    pub(crate) fn start_pending_agent_resume_for_terminal(
        &mut self,
        terminal_id: &crate::terminal::TerminalId,
        rows: u16,
        cols: u16,
        allow_empty_theme: bool,
    ) -> bool {
        if self.terminal_runtimes.get(terminal_id).is_some() {
            return false;
        }
        let Some((pane_id, cwd, plan)) = self.state.workspaces.iter().find_map(|ws| {
            ws.tabs.iter().find_map(|tab| {
                tab.layout.pane_ids().into_iter().find_map(|pane_id| {
                    let pane = tab.panes.get(&pane_id)?;
                    if &pane.attached_terminal_id != terminal_id {
                        return None;
                    }
                    let terminal = self.state.terminals.get(terminal_id)?;
                    Some((
                        pane_id,
                        terminal.cwd.clone(),
                        terminal.pending_agent_resume_plan.clone()?,
                    ))
                })
            })
        }) else {
            return false;
        };

        let changed = self.start_pending_agent_resume(
            pane_id,
            terminal_id.clone(),
            cwd,
            plan,
            rows,
            cols,
            allow_empty_theme,
        );
        if changed {
            self.schedule_session_save();
        }
        if !self.has_pending_agent_resumes() {
            self.pending_agent_resume_deadline = None;
        }
        changed
    }

    fn start_pending_agent_resume(
        &mut self,
        pane_id: crate::layout::PaneId,
        terminal_id: crate::terminal::TerminalId,
        cwd: std::path::PathBuf,
        plan: crate::agent_resume::AgentResumePlan,
        rows: u16,
        cols: u16,
        allow_empty_theme: bool,
    ) -> bool {
        let host_terminal_theme = self.state.host_terminal_theme;
        if host_terminal_theme.is_empty() && !allow_empty_theme {
            return false;
        }

        let Some(resume_command) = shell_command_from_argv(&plan.argv) else {
            tracing::warn!(
                pane = pane_id.raw(),
                terminal = %terminal_id,
                agent = %plan.agent,
                "failed to start deferred agent resume with empty argv"
            );
            return false;
        };
        let Some(launch_env) = self
            .find_pane(pane_id)
            .and_then(|(ws_idx, _)| self.pane_launch_env(ws_idx, pane_id, Vec::new()))
        else {
            return false;
        };
        let launch_env = launch_env.with_interactive_agent_colors();

        let runtime = match crate::terminal::TerminalRuntime::spawn(
            pane_id,
            rows,
            cols,
            cwd,
            self.state.pane_scrollback_limit_bytes,
            host_terminal_theme,
            crate::pane::PaneShellConfig::new(&self.state.default_shell, self.state.shell_mode),
            &launch_env,
            self.event_tx.clone(),
            self.render_notify.clone(),
            self.render_dirty.clone(),
        ) {
            Ok(runtime) => runtime,
            Err(err) => {
                tracing::warn!(
                    pane = pane_id.raw(),
                    terminal = %terminal_id,
                    agent = %plan.agent,
                    err = %err,
                    "failed to start shell for deferred agent resume"
                );
                if let Some(terminal) = self.state.terminals.get_mut(&terminal_id) {
                    terminal.clear_agent_runtime_identity_after_respawn();
                }
                return false;
            }
        };

        let mut input = resume_command;
        input.push('\r');
        if let Err(err) = runtime.try_send_bytes(Bytes::from(input)) {
            tracing::warn!(
                pane = pane_id.raw(),
                terminal = %terminal_id,
                agent = %plan.agent,
                err = %err,
                "failed to send deferred agent resume command to shell"
            );
            runtime.shutdown();
            return false;
        }

        self.terminal_runtimes.insert(terminal_id.clone(), runtime);
        if let Some(terminal) = self.state.terminals.get_mut(&terminal_id) {
            terminal.pending_agent_resume_plan = None;
            terminal.respawn_shell_on_exit = false;
        }
        true
    }
}

fn derived_pending_agent_resume_pane_infos(
    tab: &crate::workspace::Tab,
    terminal_area: Rect,
    pane_borders: bool,
    pane_gaps: bool,
) -> Vec<crate::layout::PaneInfo> {
    crate::ui::apply_pane_chrome(tab.layout.panes(terminal_area), pane_borders, pane_gaps)
        .into_iter()
        .map(|mut info| {
            let pane_inner = crate::ui::pane_inner_rect(info.rect, info.borders);
            info.inner_rect = stable_terminal_inner_rect(pane_inner);
            info
        })
        .collect()
}

fn stable_terminal_inner_rect(pane_inner: Rect) -> Rect {
    if pane_inner.width <= 4 {
        return pane_inner;
    }

    Rect::new(
        pane_inner.x,
        pane_inner.y,
        pane_inner.width.saturating_sub(1),
        pane_inner.height,
    )
}

fn shell_command_from_argv(argv: &[String]) -> Option<String> {
    let mut parts = argv.iter();
    let first = shell_quote(parts.next()?);
    let mut command = first;
    for part in parts {
        command.push(' ');
        command.push_str(&shell_quote(part));
    }
    Some(command)
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    if value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'_' | b'-' | b'.' | b'/' | b':' | b'@' | b'%' | b'+' | b'='
            )
    }) {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        let (_api_tx, api_rx) = tokio::sync::mpsc::unbounded_channel();
        App::new(
            &crate::config::Config::default(),
            true,
            None,
            api_rx,
            crate::api::EventHub::default(),
        )
    }

    #[tokio::test]
    async fn activating_a_dormant_agent_sends_the_native_resume_command() {
        let mut app = test_app();
        let workspace = crate::workspace::Workspace::test_new("dormant");
        let pane_id = workspace.tabs[0].root_pane;
        let terminal_id = workspace.terminal_id(pane_id).cloned().unwrap();
        app.state.workspaces = vec![workspace];
        app.state.active = Some(0);
        app.state.ensure_test_terminals();
        let (runtime, mut receiver) = crate::terminal::TerminalRuntime::test_with_channel(80, 24);
        app.terminal_runtimes.insert(terminal_id.clone(), runtime);
        app.state
            .terminals
            .get_mut(&terminal_id)
            .expect("test terminal should exist")
            .mark_agent_dormant(crate::agent_resume::PersistedAgentSession {
                source: "herdr:codex".into(),
                agent: "codex".into(),
                session_ref: crate::agent_resume::AgentSessionRef::id("idle-session")
                    .expect("test session id should be valid"),
            });

        assert!(app.activate_dormant_agent_for_pane(0, pane_id));
        assert_eq!(
            receiver.try_recv().expect("resume command"),
            bytes::Bytes::from_static(b"codex resume idle-session\r")
        );
        let terminal = app
            .state
            .terminals
            .get(&terminal_id)
            .expect("terminal should survive activation");
        assert!(terminal.dormant_agent_session.is_none());
        assert!(!terminal.agent_inactive);
        assert_eq!(
            terminal
                .persisted_agent_session
                .as_ref()
                .map(|session| session.session_ref.value.as_str()),
            Some("idle-session")
        );
        assert!(app.state.dormant_wake_drafts.contains_key(&pane_id));
    }

    fn dormant_wake_test_app() -> (
        App,
        crate::layout::PaneId,
        tokio::sync::mpsc::Receiver<bytes::Bytes>,
    ) {
        let mut app = test_app();
        let workspace = crate::workspace::Workspace::test_new("dormant");
        let pane_id = workspace.tabs[0].root_pane;
        let terminal_id = workspace.terminal_id(pane_id).cloned().unwrap();
        app.state.workspaces = vec![workspace];
        app.state.active = Some(0);
        app.state.ensure_test_terminals();
        app.state.mode = crate::app::Mode::Terminal;
        let (runtime, receiver) = crate::terminal::TerminalRuntime::test_with_channel(80, 24);
        app.terminal_runtimes.insert(terminal_id.clone(), runtime);
        app.state
            .terminals
            .get_mut(&terminal_id)
            .expect("test terminal should exist")
            .mark_agent_dormant(crate::agent_resume::PersistedAgentSession {
                source: "herdr:codex".into(),
                agent: "codex".into(),
                session_ref: crate::agent_resume::AgentSessionRef::id("idle-session")
                    .expect("test session id should be valid"),
            });
        (app, pane_id, receiver)
    }

    #[tokio::test]
    async fn typing_in_a_dormant_pane_resumes_and_holds_the_message_until_idle() {
        let (mut app, pane_id, mut receiver) = dormant_wake_test_app();

        app.handle_terminal_key_headless(crate::input::TerminalKey::new(
            crossterm::event::KeyCode::Char('h'),
            crossterm::event::KeyModifiers::empty(),
        ));
        app.handle_terminal_key_headless(crate::input::TerminalKey::new(
            crossterm::event::KeyCode::Char('i'),
            crossterm::event::KeyModifiers::empty(),
        ));
        assert_eq!(
            receiver.try_recv().expect("resume command"),
            bytes::Bytes::from_static(b"codex resume idle-session\r")
        );
        assert!(
            receiver.try_recv().is_err(),
            "typed keys must not go to the leftover shell"
        );
        assert_eq!(
            app.state
                .dormant_wake_drafts
                .get(&pane_id)
                .map(|draft| draft.text.as_str()),
            Some("hi")
        );

        app.handle_terminal_key_headless(crate::input::TerminalKey::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::empty(),
        ));
        assert_eq!(
            app.pending_catalog_submissions
                .get(&pane_id)
                .map(String::as_str),
            Some("hi")
        );
        assert!(app
            .state
            .dormant_wake_drafts
            .get(&pane_id)
            .is_some_and(|draft| draft.submitted));
        assert!(receiver.try_recv().is_err(), "wait for the idle prompt");

        app.flush_pending_catalog_submission(pane_id, crate::detect::AgentState::Idle);
        assert_eq!(
            receiver.try_recv().expect("queued message"),
            bytes::Bytes::from_static(b"hi\r")
        );
        assert!(app.pending_catalog_submissions.is_empty());
        assert!(!app.state.dormant_wake_drafts.contains_key(&pane_id));
    }

    #[tokio::test]
    async fn click_wake_captures_keys_until_the_agent_is_idle() {
        let (mut app, pane_id, mut receiver) = dormant_wake_test_app();
        assert!(app.activate_dormant_agent_for_pane(0, pane_id));
        assert_eq!(
            receiver.try_recv().expect("resume command"),
            bytes::Bytes::from_static(b"codex resume idle-session\r")
        );

        app.handle_terminal_key_headless(crate::input::TerminalKey::new(
            crossterm::event::KeyCode::Char('x'),
            crossterm::event::KeyModifiers::empty(),
        ));
        assert!(
            receiver.try_recv().is_err(),
            "keys typed while the CLI is starting must not leak into the shell"
        );
        assert_eq!(
            app.state
                .dormant_wake_drafts
                .get(&pane_id)
                .map(|draft| draft.text.as_str()),
            Some("x")
        );
    }

    #[tokio::test]
    async fn empty_wake_draft_releases_the_pane_once_idle() {
        let (mut app, pane_id, mut receiver) = dormant_wake_test_app();
        assert!(app.activate_dormant_agent_for_pane(0, pane_id));
        assert_eq!(
            receiver.try_recv().expect("resume command"),
            bytes::Bytes::from_static(b"codex resume idle-session\r")
        );

        app.release_idle_dormant_wake_draft(pane_id, crate::detect::AgentState::Idle);
        assert!(!app.state.dormant_wake_drafts.contains_key(&pane_id));

        app.handle_terminal_key_headless(crate::input::TerminalKey::new(
            crossterm::event::KeyCode::Char('z'),
            crossterm::event::KeyModifiers::empty(),
        ));
        assert_eq!(
            receiver.try_recv().expect("live key"),
            bytes::Bytes::from_static(b"z")
        );
    }

    #[tokio::test]
    async fn dormant_wake_paste_and_backspace_edit_the_draft() {
        let (mut app, pane_id, mut receiver) = dormant_wake_test_app();
        app.append_dormant_wake_paste(0, pane_id, "你好\r吗");
        assert_eq!(
            receiver.try_recv().expect("resume command"),
            bytes::Bytes::from_static(b"codex resume idle-session\r")
        );
        assert_eq!(
            app.state
                .dormant_wake_drafts
                .get(&pane_id)
                .map(|draft| draft.text.as_str()),
            Some("你好吗")
        );
        app.handle_terminal_key_headless(crate::input::TerminalKey::new(
            crossterm::event::KeyCode::Backspace,
            crossterm::event::KeyModifiers::empty(),
        ));
        assert_eq!(
            app.state
                .dormant_wake_drafts
                .get(&pane_id)
                .map(|draft| draft.text.as_str()),
            Some("你好")
        );
        assert!(receiver.try_recv().is_err());
    }

    #[tokio::test]
    async fn client_paste_into_a_dormant_pane_is_held_in_the_wake_draft() {
        let (mut app, pane_id, mut receiver) = dormant_wake_test_app();
        app.route_client_events(
            vec![crate::raw_input::RawInputEvent::Paste("跟进\n一下".into())],
            false,
        );
        assert_eq!(
            receiver.try_recv().expect("resume command"),
            bytes::Bytes::from_static(b"codex resume idle-session\r")
        );
        assert_eq!(
            app.state
                .dormant_wake_drafts
                .get(&pane_id)
                .map(|draft| draft.text.as_str()),
            Some("跟进\n一下")
        );
        assert!(receiver.try_recv().is_err());
    }

    #[cfg(unix)]
    fn long_running_test_argv() -> Vec<String> {
        vec!["/bin/sh".into(), "-c".into(), "sleep 5".into()]
    }

    #[cfg(unix)]
    fn marker_resume_test_argv() -> Vec<String> {
        vec![
            "/bin/sh".into(),
            "-c".into(),
            "printf '%s' 'restored agent: shell quoted | marker'; sleep 5".into(),
        ]
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn pending_agent_resume_waits_for_host_theme_before_launch() {
        let mut app = test_app();
        let workspace = crate::workspace::Workspace::test_new("restored");
        let pane_id = workspace.tabs[0].root_pane;
        let terminal_id = workspace.terminal_id(pane_id).cloned().unwrap();
        let pane_infos = workspace.tabs[0]
            .layout
            .panes(ratatui::layout::Rect::new(0, 0, 100, 30));
        app.state.workspaces = vec![workspace];
        app.state.active = Some(0);
        app.state.ensure_test_terminals();
        app.state.view.terminal_area = ratatui::layout::Rect::new(0, 0, 100, 30);
        app.state.view.pane_infos = pane_infos;
        let terminal = app
            .state
            .terminals
            .get_mut(&terminal_id)
            .expect("test terminal should exist");
        terminal.pending_agent_resume_plan = Some(crate::agent_resume::AgentResumePlan {
            agent: "codex".into(),
            argv: marker_resume_test_argv(),
            dedupe_key: "herdr:codex\0codex\0Id\0codex-session".into(),
        });

        assert!(!app.start_pending_agent_resumes(false));
        assert!(app.terminal_runtimes.get(&terminal_id).is_none());

        app.state.host_terminal_theme = crate::terminal_theme::TerminalTheme {
            foreground: Some(crate::terminal_theme::RgbColor {
                r: 220,
                g: 220,
                b: 220,
            }),
            background: Some(crate::terminal_theme::RgbColor {
                r: 20,
                g: 20,
                b: 20,
            }),
        };

        assert!(app.start_pending_agent_resumes(false));
        assert!(app.terminal_runtimes.get(&terminal_id).is_some());
        let terminal = app
            .state
            .terminals
            .get(&terminal_id)
            .expect("terminal should survive launch");
        assert!(terminal.pending_agent_resume_plan.is_none());
        assert!(!terminal.respawn_shell_on_exit);

        let runtime = app
            .terminal_runtimes
            .get(&terminal_id)
            .expect("pending resume should leave a shell runtime");
        let marker = "restored agent: shell quoted | marker";
        for _ in 0..20 {
            if runtime
                .snapshot_history()
                .is_some_and(|text| text.contains(marker))
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        assert!(
            runtime
                .snapshot_history()
                .expect("runtime should expose terminal history")
                .contains(marker),
            "deferred restore should inject the resume argv into the restored shell"
        );

        for (_, runtime) in app.terminal_runtimes.drain() {
            runtime.shutdown();
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn pending_agent_resume_can_launch_after_theme_wait_expires() {
        let mut app = test_app();
        let workspace = crate::workspace::Workspace::test_new("restored");
        let pane_id = workspace.tabs[0].root_pane;
        let terminal_id = workspace.terminal_id(pane_id).cloned().unwrap();
        app.state.view.pane_infos = workspace.tabs[0]
            .layout
            .panes(ratatui::layout::Rect::new(0, 0, 100, 30));
        app.state.view.terminal_area = ratatui::layout::Rect::new(0, 0, 100, 30);
        app.state.workspaces = vec![workspace];
        app.state.active = Some(0);
        app.state.ensure_test_terminals();
        app.state
            .terminals
            .get_mut(&terminal_id)
            .expect("test terminal should exist")
            .pending_agent_resume_plan = Some(crate::agent_resume::AgentResumePlan {
            agent: "codex".into(),
            argv: long_running_test_argv(),
            dedupe_key: "herdr:codex\0codex\0Id\0codex-session".into(),
        });

        app.sync_pending_agent_resume_deadline(std::time::Instant::now());
        assert!(!app.start_pending_agent_resumes(false));
        assert!(app.start_pending_agent_resumes(true));
        assert!(app.terminal_runtimes.get(&terminal_id).is_some());

        for (_, runtime) in app.terminal_runtimes.drain() {
            runtime.shutdown();
        }
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn pending_agent_resume_launches_hidden_panes_with_current_terminal_area() {
        let mut app = test_app();
        let active_workspace = crate::workspace::Workspace::test_new("active");
        let active_pane = active_workspace.tabs[0].root_pane;
        let active_terminal = active_workspace.terminal_id(active_pane).cloned().unwrap();
        let hidden_workspace = crate::workspace::Workspace::test_new("hidden");
        let hidden_pane = hidden_workspace.tabs[0].root_pane;
        let hidden_terminal = hidden_workspace.terminal_id(hidden_pane).cloned().unwrap();
        app.state.view.pane_infos = active_workspace.tabs[0]
            .layout
            .panes(ratatui::layout::Rect::new(0, 0, 100, 30));
        app.state.view.terminal_area = ratatui::layout::Rect::new(0, 0, 100, 30);
        app.state.workspaces = vec![active_workspace, hidden_workspace];
        app.state.active = Some(0);
        app.state.ensure_test_terminals();
        app.state.host_terminal_theme = crate::terminal_theme::TerminalTheme {
            foreground: Some(crate::terminal_theme::RgbColor {
                r: 220,
                g: 220,
                b: 220,
            }),
            background: Some(crate::terminal_theme::RgbColor {
                r: 20,
                g: 20,
                b: 20,
            }),
        };
        for terminal_id in [&active_terminal, &hidden_terminal] {
            app.state
                .terminals
                .get_mut(terminal_id)
                .expect("test terminal should exist")
                .pending_agent_resume_plan = Some(crate::agent_resume::AgentResumePlan {
                agent: "codex".into(),
                argv: long_running_test_argv(),
                dedupe_key: format!("herdr:codex\0codex\0Id\0{terminal_id}"),
            });
        }
        app.pending_agent_resume_deadline =
            Some(std::time::Instant::now() - std::time::Duration::from_millis(1));

        assert!(app.start_pending_agent_resumes(false));
        assert!(app.terminal_runtimes.get(&active_terminal).is_some());
        assert!(app.terminal_runtimes.get(&hidden_terminal).is_some());
        assert!(
            app.pending_agent_resume_deadline.is_none(),
            "launched pending resumes should clear the wakeup deadline"
        );

        for (_, runtime) in app.terminal_runtimes.drain() {
            runtime.shutdown();
        }
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn pending_agent_resume_launches_inactive_tab_panes_with_current_terminal_area() {
        let mut app = test_app();
        let mut workspace = crate::workspace::Workspace::test_new("tabs");
        let active_pane = workspace.tabs[0].root_pane;
        let inactive_tab = workspace.test_add_tab(Some("agents"));
        let inactive_pane = workspace.tabs[inactive_tab].root_pane;
        let inactive_terminal = workspace.tabs[inactive_tab]
            .terminal_id(inactive_pane)
            .cloned()
            .unwrap();
        app.state.view.pane_infos = workspace.tabs[0]
            .layout
            .panes(ratatui::layout::Rect::new(0, 0, 100, 30));
        app.state.view.terminal_area = ratatui::layout::Rect::new(0, 0, 100, 30);
        app.state.workspaces = vec![workspace];
        app.state.active = Some(0);
        app.state.ensure_test_terminals();
        assert!(app
            .state
            .workspaces
            .first()
            .and_then(|ws| ws.tabs[0].terminal_id(active_pane))
            .is_some());
        app.state.host_terminal_theme = crate::terminal_theme::TerminalTheme {
            foreground: Some(crate::terminal_theme::RgbColor {
                r: 220,
                g: 220,
                b: 220,
            }),
            background: Some(crate::terminal_theme::RgbColor {
                r: 20,
                g: 20,
                b: 20,
            }),
        };
        app.state
            .terminals
            .get_mut(&inactive_terminal)
            .expect("inactive tab terminal should exist")
            .pending_agent_resume_plan = Some(crate::agent_resume::AgentResumePlan {
            agent: "codex".into(),
            argv: long_running_test_argv(),
            dedupe_key: "herdr:codex\0codex\0Id\0inactive-tab-session".into(),
        });

        assert!(app.start_pending_agent_resumes(false));
        assert!(app.terminal_runtimes.get(&inactive_terminal).is_some());
        assert!(
            app.state
                .terminals
                .get(&inactive_terminal)
                .expect("inactive tab terminal should still exist")
                .pending_agent_resume_plan
                .is_none(),
            "inactive tab restored panes should not wait for tab focus"
        );

        for (_, runtime) in app.terminal_runtimes.drain() {
            runtime.shutdown();
        }
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn pending_agent_resume_launches_zoom_hidden_active_tab_panes() {
        let mut app = test_app();
        let mut workspace = crate::workspace::Workspace::test_new("zoomed");
        let hidden_pane = workspace.tabs[0].root_pane;
        let visible_pane = workspace.test_split(ratatui::layout::Direction::Horizontal);
        workspace.tabs[0].zoomed = true;
        let hidden_terminal = workspace.terminal_id(hidden_pane).cloned().unwrap();
        app.state.view.pane_infos = vec![crate::layout::PaneInfo {
            id: visible_pane,
            rect: ratatui::layout::Rect::new(0, 0, 100, 30),
            inner_rect: ratatui::layout::Rect::new(1, 1, 98, 28),
            scrollbar_rect: None,
            borders: ratatui::widgets::Borders::ALL,
            is_focused: true,
        }];
        app.state.view.terminal_area = ratatui::layout::Rect::new(0, 0, 100, 30);
        app.state.workspaces = vec![workspace];
        app.state.active = Some(0);
        app.state.ensure_test_terminals();
        app.state.host_terminal_theme = crate::terminal_theme::TerminalTheme {
            foreground: Some(crate::terminal_theme::RgbColor {
                r: 220,
                g: 220,
                b: 220,
            }),
            background: Some(crate::terminal_theme::RgbColor {
                r: 20,
                g: 20,
                b: 20,
            }),
        };
        app.state
            .terminals
            .get_mut(&hidden_terminal)
            .expect("hidden zoom pane terminal should exist")
            .pending_agent_resume_plan = Some(crate::agent_resume::AgentResumePlan {
            agent: "codex".into(),
            argv: long_running_test_argv(),
            dedupe_key: "herdr:codex\0codex\0Id\0zoom-hidden-session".into(),
        });

        assert!(app.start_pending_agent_resumes(false));
        assert!(app.terminal_runtimes.get(&hidden_terminal).is_some());
        assert!(
            app.state
                .terminals
                .get(&hidden_terminal)
                .expect("hidden zoom pane terminal should still exist")
                .pending_agent_resume_plan
                .is_none(),
            "zoom-hidden restored panes should not wait for pane focus"
        );

        for (_, runtime) in app.terminal_runtimes.drain() {
            runtime.shutdown();
        }
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn pending_agent_resume_uses_current_terminal_area_for_background_panes() {
        let mut app = test_app();
        let previous_workspace = crate::workspace::Workspace::test_new("previous");
        let previous_pane = previous_workspace.tabs[0].root_pane;
        let previous_terminal = previous_workspace
            .terminal_id(previous_pane)
            .cloned()
            .unwrap();
        let current_workspace = crate::workspace::Workspace::test_new("current");
        app.state.view.pane_infos = previous_workspace.tabs[0]
            .layout
            .panes(ratatui::layout::Rect::new(0, 0, 100, 30));
        app.state.view.terminal_area = ratatui::layout::Rect::new(0, 0, 80, 24);
        app.state.workspaces = vec![previous_workspace, current_workspace];
        app.state.active = Some(1);
        app.state.ensure_test_terminals();
        app.state.host_terminal_theme = crate::terminal_theme::TerminalTheme {
            foreground: Some(crate::terminal_theme::RgbColor {
                r: 220,
                g: 220,
                b: 220,
            }),
            background: Some(crate::terminal_theme::RgbColor {
                r: 20,
                g: 20,
                b: 20,
            }),
        };
        app.state
            .terminals
            .get_mut(&previous_terminal)
            .expect("test terminal should exist")
            .pending_agent_resume_plan = Some(crate::agent_resume::AgentResumePlan {
            agent: "codex".into(),
            argv: long_running_test_argv(),
            dedupe_key: "herdr:codex\0codex\0Id\0codex-session".into(),
        });

        app.sync_pending_agent_resume_deadline(std::time::Instant::now());
        assert!(app.pending_agent_resume_deadline.is_some());
        assert!(app.start_pending_agent_resumes(false));
        assert!(app.terminal_runtimes.get(&previous_terminal).is_some());
        assert!(
            app.state
                .terminals
                .get(&previous_terminal)
                .expect("previous terminal should still exist")
                .pending_agent_resume_plan
                .is_none(),
            "background restored panes should not wait for focus once terminal area is known"
        );

        for (_, runtime) in app.terminal_runtimes.drain() {
            runtime.shutdown();
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn pending_agent_resume_launches_with_inner_rect_size() {
        let mut app = test_app();
        let mut workspace = crate::workspace::Workspace::test_new("split");
        let pane_id = workspace.test_split(ratatui::layout::Direction::Horizontal);
        let terminal_id = workspace.terminal_id(pane_id).cloned().unwrap();
        app.state.view.pane_infos = vec![crate::layout::PaneInfo {
            id: pane_id,
            rect: ratatui::layout::Rect::new(0, 0, 100, 30),
            inner_rect: ratatui::layout::Rect::new(1, 1, 98, 28),
            scrollbar_rect: None,
            borders: ratatui::widgets::Borders::ALL,
            is_focused: true,
        }];
        app.state.view.terminal_area = ratatui::layout::Rect::new(0, 0, 100, 30);
        app.state.workspaces = vec![workspace];
        app.state.active = Some(0);
        app.state.ensure_test_terminals();
        app.state.host_terminal_theme = crate::terminal_theme::TerminalTheme {
            foreground: Some(crate::terminal_theme::RgbColor {
                r: 220,
                g: 220,
                b: 220,
            }),
            background: Some(crate::terminal_theme::RgbColor {
                r: 20,
                g: 20,
                b: 20,
            }),
        };
        app.state
            .terminals
            .get_mut(&terminal_id)
            .expect("test terminal should exist")
            .pending_agent_resume_plan = Some(crate::agent_resume::AgentResumePlan {
            agent: "codex".into(),
            argv: long_running_test_argv(),
            dedupe_key: "herdr:codex\0codex\0Id\0codex-session".into(),
        });

        assert!(app.start_pending_agent_resumes(false));
        assert_eq!(
            app.terminal_runtimes
                .get(&terminal_id)
                .expect("pending resume should launch")
                .current_size(),
            (28, 98)
        );

        for (_, runtime) in app.terminal_runtimes.drain() {
            runtime.shutdown();
        }
    }

    #[test]
    fn shell_command_from_argv_quotes_resume_arguments() {
        let argv = vec![
            "claude".to_string(),
            "--resume".to_string(),
            "session with ' quote".to_string(),
        ];

        assert_eq!(
            shell_command_from_argv(&argv).as_deref(),
            Some("claude --resume 'session with '\\'' quote'")
        );
        assert_eq!(shell_command_from_argv(&[]), None);
    }
}
