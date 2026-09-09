//! Read-only session configuration. Explicit session launches reuse the runtime API.

use super::{App, Mode};
use crate::projects::adapters::ADAPTER_NAMES;

const AGENTS: &[&str] = &["claude", "codex", "opencode", "pi", "hermes", "grok"];

#[derive(Debug, Clone)]
pub(crate) struct HistoryLocation {
    pub adapter: &'static str,
    pub paths: Vec<String>,
    pub restart_required: bool,
}

/// Applied session settings and history locations, with no editable draft.
#[derive(Debug, Clone, Default)]
pub(crate) struct SessionSetupState {
    pub agent: String,
    pub detected_agents: Vec<String>,
    pub history_locations: Vec<HistoryLocation>,
    pub history_restart_required: bool,
}

impl SessionSetupState {
    pub(crate) fn new(agent: &str) -> Self {
        let mut setup = Self::default();
        setup.set_saved_agent(agent);
        setup
    }

    pub(crate) fn set_saved_agent(&mut self, agent: &str) {
        self.agent = if agent.is_empty() { "shell" } else { agent }.into();
    }

    /// Display the saved configuration even while the Catalog keeps its active roots until restart.
    pub(crate) fn set_history_locations(
        &mut self,
        saved: &crate::config::ProjectsConfig,
        active: &crate::config::ProjectsConfig,
    ) {
        let saved_roots = crate::projects::adapters::resolve_roots(saved);
        let active_roots = crate::projects::adapters::resolve_roots(active);
        self.history_restart_required = saved.adapters != active.adapters;
        self.history_locations = ADAPTER_NAMES
            .iter()
            .map(|agent| {
                let mut roots = saved_roots
                    .roots()
                    .iter()
                    .filter(|root| root.adapter == *agent)
                    .collect::<Vec<_>>();
                let is_pending = |root: &crate::projects::adapters::AdapterRoot| {
                    !active_roots
                        .roots()
                        .iter()
                        .any(|active| active.adapter == root.adapter && active.path == root.path)
                };
                // New directories come first so the saved change is visible in a narrow row.
                roots.sort_by_key(|root| !is_pending(root));
                let pending = roots.iter().any(|root| is_pending(root));
                let paths = roots
                    .into_iter()
                    .map(|root| {
                        if root.path.as_os_str().is_empty() {
                            return "standard location unavailable".to_string();
                        }
                        format!(
                            "{}{}",
                            root.path.display(),
                            if root.path.is_dir() {
                                ""
                            } else {
                                " (not found yet)"
                            }
                        )
                    })
                    .collect::<Vec<_>>();
                HistoryLocation {
                    adapter: agent,
                    paths,
                    restart_required: pending,
                }
            })
            .collect();
    }
}

pub(crate) fn new_session_method(
    agent: &str,
    name: String,
    cwd: Option<String>,
) -> crate::api::schema::Method {
    use crate::api::schema::{AgentStartParams, Method, WorkspaceCreateParams};
    if agent.is_empty() || agent == "shell" {
        Method::WorkspaceCreate(WorkspaceCreateParams {
            cwd,
            focus: true,
            label: None,
            env: Default::default(),
        })
    } else {
        Method::AgentStart(AgentStartParams {
            name,
            cwd,
            workspace_id: None,
            tab_id: None,
            split: None,
            focus: true,
            argv: vec![agent.into()],
            env: Default::default(),
        })
    }
}

impl App {
    pub(crate) fn refresh_session_setup(&mut self) {
        let setup = &mut self.state.session_setup;
        setup.detected_agents = AGENTS
            .iter()
            .filter(|agent| crate::integration::command_available(agent))
            .map(|agent| (*agent).to_string())
            .collect();
    }

    pub(crate) fn finish_onboarding(&mut self) {
        self.mark_onboarding_complete();
        if self.state.mode == Mode::Onboarding {
            self.state.mode = if self.state.active.is_some() {
                Mode::Terminal
            } else {
                Mode::Navigate
            };
        }
    }

    pub(crate) fn new_session_from_settings(&mut self) {
        if !crate::ui::settings_can_start_session(&self.state) {
            return;
        }
        let agent = self.state.session_setup.agent.clone();
        let names = self
            .collect_agent_infos()
            .into_iter()
            .filter_map(|agent| agent.name)
            .collect::<Vec<_>>();
        let mut index = 1;
        let name = loop {
            let name = format!("{agent}-{index}");
            if !names.contains(&name) {
                break name;
            }
            index += 1;
        };
        let follow = self.workspace_creation_source().and_then(|index| {
            self.focused_pane_cwd_in_workspace(index)
                .or_else(|| self.seed_cwd_from_workspace(index))
        });
        let cwd = Some(
            self.resolve_new_terminal_cwd(follow)
                .to_string_lossy()
                .into_owned(),
        );
        let response =
            self.dispatch_api_request("tui.session.create", new_session_method(&agent, name, cwd));
        let error = match serde_json::from_str::<serde_json::Value>(&response) {
            Ok(response) if response.get("result").is_some() => None,
            Ok(response) => Some(
                response
                    .pointer("/error/message")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("Could not create a session.")
                    .to_string(),
            ),
            Err(_) => Some("Could not read the session creation response.".to_string()),
        };
        if let Some(error) = error {
            self.state.settings.status = error;
            return;
        }
        if let Some(palette) = self.state.settings.original_palette.take() {
            self.state.palette = palette;
        }
        if let Some(theme) = self.state.settings.original_theme.take() {
            self.state.theme_name = theme;
        }
        self.state.sidebar_view = super::state::SidebarView::SpacesAgents;
        self.state.mode = Mode::Terminal;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_default_agent_remains_a_plain_shell() {
        let mut setup = SessionSetupState::new("");
        setup.detected_agents = vec!["codex".into()];
        assert_eq!(setup.agent, "shell");
        setup.set_saved_agent("pi");
        assert_eq!(setup.agent, "pi");
        setup.set_saved_agent("");
        assert_eq!(setup.agent, "shell");
        assert!(matches!(
            new_session_method("", "first".into(), None),
            crate::api::schema::Method::WorkspaceCreate(_)
        ));
    }

    #[test]
    fn session_creation_reuses_api_and_does_not_send_an_initial_prompt() {
        let crate::api::schema::Method::WorkspaceCreate(shell) =
            new_session_method("shell", "first".into(), Some("/tmp/project".into()))
        else {
            panic!("shell request");
        };
        assert_eq!(shell.cwd.as_deref(), Some("/tmp/project"));
        assert!(shell.focus);
        let crate::api::schema::Method::AgentStart(params) =
            new_session_method("codex", "first".into(), Some("/tmp/project".into()))
        else {
            panic!("agent request");
        };
        assert_eq!(params.argv, ["codex"]);
        assert_eq!(params.cwd.as_deref(), Some("/tmp/project"));
        assert!(params.focus);
        assert!(params.env.is_empty());
    }
}
