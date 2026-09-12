use std::collections::HashMap;

use crate::projects::activity::ActivityBatch;
use crate::projects::overview::{build_overview, ProjectOverview, WorkPhase};
use crate::projects::{IndexedSessionSummary, ProjectSummary};

use super::state::{AppState, Mode};
use super::App;

impl AppState {
    pub(crate) fn selected_project_summary(&self) -> Option<&ProjectSummary> {
        let key = self.projects.topic_detail_key.as_deref()?;
        self.projects
            .snapshot
            .topics
            .iter()
            .chain(self.projects.snapshot.projects.iter())
            .find(|project| project.canonical_key == key)
    }

    /// Shared runtime facts, also used by the public project.overview.get API.
    /// No view selection, "seen" flags, I/O, or process work determines the phase.
    pub(crate) fn project_session_phase(
        &self,
        session: &IndexedSessionSummary,
    ) -> Option<WorkPhase> {
        if !session.live {
            return None;
        }
        let workspace_id = session.workspace_id.as_deref()?;
        let pane_id = session.pane_id.as_deref()?;
        let workspace = self
            .workspaces
            .iter()
            .find(|workspace| workspace.id == workspace_id)?;
        let pane = workspace
            .tabs
            .iter()
            .flat_map(|tab| &tab.panes)
            .find_map(|(id, pane)| {
                let number = workspace.public_pane_number(*id)?;
                (crate::workspace::public_pane_id_for_number(&workspace.id, number) == pane_id)
                    .then_some(pane)
            })?;
        let terminal = self.terminals.get(&pane.attached_terminal_id)?;
        if terminal.agent_inactive {
            return Some(WorkPhase::Paused);
        }
        Some(match terminal.state {
            crate::detect::AgentState::Working => WorkPhase::Working,
            crate::detect::AgentState::Blocked => WorkPhase::NeedsInput,
            crate::detect::AgentState::Idle => WorkPhase::Ready,
            crate::detect::AgentState::Unknown => WorkPhase::Open,
        })
    }

    pub(crate) fn project_overview(
        &self,
        project: &ProjectSummary,
        activity: &ActivityBatch,
    ) -> ProjectOverview {
        let runtime: HashMap<_, _> = project
            .sessions
            .iter()
            .filter_map(|session| {
                self.project_session_phase(session)
                    .map(|phase| (session.stable_key.clone(), phase))
            })
            .collect();
        build_overview(project, &runtime, activity, self.title_language)
    }

    pub(crate) fn visible_project_overview(&self) -> Option<ProjectOverview> {
        let project = self.selected_project_summary()?;
        self.projects
            .overview
            .as_ref()
            .filter(|overview| {
                overview.project_key == project.canonical_key
                    && overview.language == self.title_language
            })
            .cloned()
            .or_else(|| Some(self.project_overview(project, &ActivityBatch::default())))
    }
}

impl App {
    pub(crate) fn sync_project_overview(&mut self) -> bool {
        self.refresh_project_overview(false)
    }

    pub(crate) fn refresh_project_overview(&mut self, force: bool) -> bool {
        if !matches!(self.state.mode, Mode::TopicDetail | Mode::EditTopicCover) {
            return false;
        }
        let Some(project) = self.state.selected_project_summary() else {
            return false;
        };
        let activity = self.project_service.recent_activity(project, force);
        let mut next = self.state.project_overview(project, &activity);
        self.annotate_project_followups(&mut next);
        if self.state.projects.overview.as_ref() == Some(&next) {
            return false;
        }
        self.state.projects.overview = Some(next);
        true
    }
}
