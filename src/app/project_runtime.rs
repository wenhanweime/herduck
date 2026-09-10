use super::App;

impl App {
    pub(crate) fn sync_all_project_runtime_mappings(&mut self) {
        if !self.project_service.is_available() {
            return;
        }
        let pane_ids = self
            .state
            .workspaces
            .iter()
            .flat_map(|workspace| {
                workspace
                    .tabs
                    .iter()
                    .flat_map(|tab| tab.panes.keys().copied())
            })
            .collect::<Vec<_>>();
        for pane_id in pane_ids {
            self.sync_project_runtime_for_pane(pane_id, false);
        }
    }

    pub(crate) fn sync_project_runtime_for_pane(
        &mut self,
        pane_id: crate::layout::PaneId,
        force_metadata_refresh: bool,
    ) {
        if !self.project_service.is_available() {
            return;
        }
        let report = self.find_pane(pane_id).and_then(|(ws_idx, pane)| {
            let terminal = self.state.terminals.get(&pane.attached_terminal_id)?;
            let session = terminal.persisted_agent_session.clone();
            Some((
                session,
                terminal.cwd.clone(),
                self.public_workspace_id(ws_idx),
                self.public_pane_id(ws_idx, pane_id)?,
                terminal.effective_agent_label().map(str::to_string),
                terminal.terminal_title.clone().unwrap_or_default(),
                pane.attached_terminal_id.clone(),
            ))
        });
        let Some((session, cwd, workspace_id, public_pane_id, agent, title, terminal_id)) = report
        else {
            self.clear_project_runtime_for_pane(pane_id);
            return;
        };
        let mut discovered = None;
        let identity = if let Some(session) = session.as_ref() {
            crate::projects::runtime::identity_from_report(
                &self.project_roots,
                &session.agent,
                &session.session_ref,
            )
        } else if let Some(agent) = agent.as_deref() {
            let job = self
                .terminal_runtimes
                .get(&terminal_id)
                .and_then(|runtime| runtime.child_pid())
                .and_then(crate::detect::foreground_job);
            if agent == "grok" {
                let files: Vec<_> = job
                    .as_ref()
                    .into_iter()
                    .flat_map(|job| job.processes.iter())
                    .filter(|process| {
                        crate::detect::identify_agent(&process.name)
                            == Some(crate::detect::Agent::Grok)
                            || process.argv0.as_deref() == Some("grok")
                    })
                    .flat_map(|process| crate::platform::process_open_files(process.pid))
                    .collect();
                discovered = crate::projects::runtime::grok_session_from_open_files(
                    &self.project_roots,
                    &files,
                    &title,
                );
            }
            discovered
                .as_ref()
                .map(|(identity, _, _)| identity.clone())
                .or_else(|| {
                    job.and_then(|_| {
                        crate::projects::SessionIdentity::id(
                            agent,
                            &format!("herduck-live:{terminal_id:?}"),
                        )
                        .ok()
                    })
                })
        } else {
            None
        };
        let Some(identity) = identity else {
            self.clear_project_runtime_for_pane(pane_id);
            return;
        };

        let existing = self.project_runtime_leases.get(&pane_id).cloned();
        let same_mapping = existing.as_ref().is_some_and(|lease| {
            lease.session_key == identity.stable_key
                && lease.workspace_id == workspace_id
                && lease.pane_id == public_pane_id
        });
        if same_mapping && !force_metadata_refresh {
            return;
        }

        let generation = if let Some(existing) = existing {
            if same_mapping {
                existing.generation
            } else {
                self.clear_project_runtime_for_pane(pane_id);
                self.take_next_project_runtime_generation()
            }
        } else {
            self.take_next_project_runtime_generation()
        };
        let observed_at = crate::projects::runtime::unix_time_ms().max(
            self.project_runtime_leases
                .get(&pane_id)
                .map(|lease| lease.observed_at.saturating_add(1))
                .unwrap_or_default(),
        );
        let mut candidate = crate::projects::runtime::candidate_from_report(
            crate::projects::runtime::RuntimeCandidateInput {
                identity: identity.clone(),
                cwd,
                workspace_id: &workspace_id,
                pane_id: &public_pane_id,
                generation,
                observed_at,
            },
        );
        if let Some((_, path, native_title)) = discovered {
            let source_key = candidate.source_key.clone();
            candidate.transcript_ref = Some(crate::projects::CandidateField {
                value: path.to_string_lossy().into_owned(),
                observed_at,
                priority: crate::projects::SourcePriority::RuntimeReport,
                source_key: source_key.clone(),
            });
            candidate.title = Some(crate::projects::CandidateField {
                value: native_title,
                observed_at,
                priority: crate::projects::SourcePriority::RuntimeReport,
                source_key,
            });
        } else if session.is_none() {
            candidate.title = Some(crate::projects::CandidateField {
                value: if title.is_empty() {
                    format!("{} · 运行中", agent.as_deref().unwrap_or("agent"))
                } else {
                    title
                },
                observed_at,
                priority: crate::projects::SourcePriority::RuntimeReport,
                source_key: candidate.source_key.clone(),
            });
        }
        match self.project_service.upsert_candidate(candidate) {
            Ok(_) => {
                self.project_runtime_leases.insert(
                    pane_id,
                    crate::projects::runtime::RuntimeLease {
                        session_key: identity.stable_key,
                        workspace_id,
                        pane_id: public_pane_id,
                        generation,
                        observed_at,
                    },
                );
                self.replace_projects_snapshot(self.project_service.snapshot());
            }
            Err(error) => tracing::warn!(
                adapter = agent.as_deref().unwrap_or("unknown"),
                category = error.code,
                "Project runtime report was not committed"
            ),
        }
    }

    pub(crate) fn clear_project_runtime_for_pane(&mut self, pane_id: crate::layout::PaneId) {
        let Some(lease) = self.project_runtime_leases.remove(&pane_id) else {
            return;
        };
        match self
            .project_service
            .clear_runtime_mapping(lease.session_key, lease.generation)
        {
            Ok(_) => self.replace_projects_snapshot(self.project_service.snapshot()),
            Err(error) => {
                tracing::warn!(
                    category = error.code,
                    "Project runtime lease could not be cleared"
                );
            }
        }
    }

    pub(crate) fn clear_project_runtime_for_panes(
        &mut self,
        pane_ids: impl IntoIterator<Item = crate::layout::PaneId>,
    ) {
        for pane_id in pane_ids {
            self.clear_project_runtime_for_pane(pane_id);
        }
    }

    #[cfg(unix)]
    pub(crate) fn activate_project_service_after_handoff(&mut self) {
        if self.project_service.is_available() {
            return;
        }
        let mut service = crate::projects::ProjectService::open_with_config(
            &crate::session::data_dir().join("projects/catalog.sqlite3"),
            self.event_hub.clone(),
            &self.loaded_projects_config,
        );
        service.start_background_scan(self.project_roots.roots());
        if let Err(error) = service.configure_summaries(&self.loaded_projects_config.summary) {
            tracing::warn!("Could not configure summaries: {}", error.message);
        }
        self.project_service = service;
        self.project_runtime_leases.clear();
        self.next_project_runtime_generation = 1;
        self.sync_all_project_runtime_mappings();
        self.replace_projects_snapshot(self.project_service.snapshot());
    }

    fn take_next_project_runtime_generation(&mut self) -> u64 {
        let generation = self.next_project_runtime_generation;
        self.next_project_runtime_generation =
            self.next_project_runtime_generation.saturating_add(1);
        generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::AgentState;
    use crate::events::AppEvent;

    fn app_with_project_runtime() -> (App, crate::layout::PaneId) {
        let event_hub = crate::api::EventHub::default();
        let (_api_tx, api_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(
            &crate::config::Config::default(),
            true,
            None,
            api_rx,
            event_hub.clone(),
        );
        app.project_service = crate::projects::ProjectService::in_memory(event_hub);
        app.state.workspaces = vec![crate::workspace::Workspace::test_new("runtime")];
        app.state.ensure_test_terminals();
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        (app, pane_id)
    }

    fn report(app: &mut App, pane_id: crate::layout::PaneId, id: &str, seq: u64, start: &str) {
        app.handle_internal_event(AppEvent::AgentSessionReported {
            pane_id,
            source: "herdr:codex".to_string(),
            agent_label: "codex".to_string(),
            seq: Some(seq),
            session_ref: crate::agent_resume::AgentSessionRef::id(id),
            session_start_source: Some(start.to_string()),
        });
    }

    #[test]
    fn accepted_session_report_upserts_live_mapping_and_pane_exit_clears_it() {
        let (mut app, pane_id) = app_with_project_runtime();
        report(&mut app, pane_id, "live-one", 1, "startup");
        let snapshot = app.project_service.snapshot();
        let session = snapshot.projects[0].sessions.first().expect("live session");
        assert!(session.live);
        assert_eq!(
            session.workspace_id.as_deref(),
            Some(app.state.workspaces[0].id.as_str())
        );
        assert_eq!(session.runtime_generation, Some(1));
        let stable_key = session.stable_key.clone();
        assert!(app.state.projects.snapshot.topics.is_empty());
        assert!(app
            .state
            .projects
            .snapshot
            .projects
            .iter()
            .flat_map(|project| &project.sessions)
            .any(|session| session.stable_key == stable_key && session.live));

        app.handle_internal_event(AppEvent::PaneDied { pane_id });
        let snapshot = app.project_service.snapshot();
        assert!(!snapshot.projects[0].sessions[0].live);
        assert!(app.project_runtime_leases.is_empty());
    }

    #[tokio::test]
    async fn idle_timeout_keeps_catalog_mapping_and_native_conversation_live() {
        let (mut app, pane_id) = app_with_project_runtime();
        let terminal_id = app.state.terminal_id_for_pane(0, pane_id).unwrap();
        let (runtime, mut receiver) = crate::terminal::TerminalRuntime::test_with_channel(80, 24);
        app.terminal_runtimes.insert(terminal_id.clone(), runtime);
        report(&mut app, pane_id, "inactive-native-session", 1, "startup");
        app.handle_internal_event(AppEvent::StateChanged {
            pane_id,
            agent: Some(crate::detect::Agent::Codex),
            state: AgentState::Idle,
            visible_blocker: false,
            visible_working: false,
            process_exited: false,
            observed_at: std::time::Instant::now(),
        });
        assert_eq!(app.state.terminals[&terminal_id].state, AgentState::Idle);
        let snapshot = app.project_service.snapshot();
        let lease = app.project_runtime_leases[&pane_id].clone();
        let native_session = app.state.terminals[&terminal_id]
            .persisted_agent_session
            .clone();
        assert!(native_session.is_some());
        let now = std::time::Instant::now();
        app.terminal_runtimes
            .get(&terminal_id)
            .unwrap()
            .test_set_last_activity_at(now - std::time::Duration::from_secs(3600));

        assert!(app.refresh_agent_inactivity(now));
        assert_eq!(app.project_runtime_leases[&pane_id], lease);
        let after = app.project_service.snapshot();
        assert_eq!(after.revision, snapshot.revision);
        let session = &after.projects[0].sessions[0];
        assert!(session.live);
        assert_eq!(session.stable_key, lease.session_key);
        assert_eq!(session.runtime_generation, Some(lease.generation));
        assert_eq!(session.pane_id.as_deref(), Some(lease.pane_id.as_str()));
        assert_eq!(
            app.state.terminals[&terminal_id].persisted_agent_session,
            native_session
        );

        app.terminal_runtimes
            .get(&terminal_id)
            .unwrap()
            .try_send_bytes(bytes::Bytes::from_static(b"continue this conversation\r"))
            .unwrap();
        assert_eq!(receiver.try_recv().unwrap(), "continue this conversation\r");
        assert!(app.refresh_agent_inactivity(std::time::Instant::now()));
        assert!(!app.state.terminals[&terminal_id].agent_inactive);
        assert_eq!(app.project_runtime_leases[&pane_id], lease);
        assert_eq!(
            app.state.terminals[&terminal_id].persisted_agent_session,
            native_session
        );
    }

    #[test]
    fn replacement_session_clears_old_generation_and_maps_new_native_identity() {
        let (mut app, pane_id) = app_with_project_runtime();
        report(&mut app, pane_id, "session-a", 1, "startup");
        report(&mut app, pane_id, "session-b", 2, "clear");
        let snapshot = app.project_service.snapshot();
        let sessions = snapshot
            .projects
            .iter()
            .flat_map(|project| &project.sessions)
            .collect::<Vec<_>>();
        assert_eq!(sessions.len(), 2);
        let old = sessions
            .iter()
            .find(|session| session.title.contains("session-a") || !session.live)
            .expect("old session");
        assert!(!old.live);
        let live = sessions
            .iter()
            .find(|session| session.live)
            .expect("new live");
        assert_eq!(live.runtime_generation, Some(2));
    }

    #[test]
    fn cwd_report_refreshes_runtime_metadata_without_rotating_generation() {
        let (mut app, pane_id) = app_with_project_runtime();
        report(&mut app, pane_id, "cwd-session", 1, "startup");
        let cwd =
            std::env::temp_dir().join(format!("herdr-project-runtime-cwd-{}", std::process::id()));
        std::fs::create_dir_all(&cwd).expect("runtime cwd");
        app.handle_internal_event(AppEvent::TerminalCwdReported {
            pane_id,
            cwd: cwd.clone(),
        });
        let snapshot = app.project_service.snapshot();
        let session = snapshot
            .projects
            .iter()
            .flat_map(|project| &project.sessions)
            .find(|session| session.live)
            .expect("live session");
        assert_eq!(session.runtime_generation, Some(1));
        assert_eq!(session.cwd.as_deref(), cwd.to_str());
        let _ = std::fs::remove_dir_all(cwd);
    }

    #[test]
    fn api_pane_close_clears_live_mapping_for_every_agent_state() {
        for (index, agent_state) in [AgentState::Working, AgentState::Idle, AgentState::Blocked]
            .into_iter()
            .enumerate()
        {
            let (mut app, pane_id) = app_with_project_runtime();
            let terminal_id = app
                .state
                .terminal_id_for_pane(0, pane_id)
                .expect("test pane terminal");
            app.state
                .terminals
                .get_mut(&terminal_id)
                .expect("test terminal")
                .state = agent_state;
            report(
                &mut app,
                pane_id,
                &format!("close-state-{index}"),
                1,
                "startup",
            );

            let public_pane_id = app.public_pane_id(0, pane_id).expect("public pane id");
            let response = app.dispatch_api_request(
                "close",
                crate::api::schema::Method::PaneClose(crate::api::schema::PaneTarget {
                    pane_id: public_pane_id,
                }),
            );
            assert!(response.contains("\"ok\""), "close failed: {response}");

            let snapshot = app.project_service.snapshot();
            let session = snapshot.projects[0]
                .sessions
                .first()
                .expect("closed session");
            assert!(!session.live, "session remained live for {agent_state:?}");
            assert_eq!(session.workspace_id, None);
            assert_eq!(session.pane_id, None);
            assert_eq!(session.runtime_generation, None);
            assert!(app.project_runtime_leases.is_empty());
            assert_eq!(app.state.projects.snapshot, snapshot);
        }
    }

    #[test]
    fn api_pane_close_only_clears_the_closed_pane_in_a_multi_pane_workspace() {
        let (mut app, first_pane) = app_with_project_runtime();
        let second_pane =
            app.state.workspaces[0].test_split(ratatui::layout::Direction::Horizontal);
        app.state.ensure_test_terminals();
        report(&mut app, first_pane, "multi-first", 1, "startup");
        report(&mut app, second_pane, "multi-second", 1, "startup");

        let first_public = app.public_pane_id(0, first_pane).expect("first pane id");
        let response = app.dispatch_api_request(
            "close-first",
            crate::api::schema::Method::PaneClose(crate::api::schema::PaneTarget {
                pane_id: first_public,
            }),
        );
        assert!(response.contains("\"ok\""), "close failed: {response}");

        let snapshot = app.project_service.snapshot();
        let sessions = snapshot.projects[0].sessions.as_slice();
        let first = sessions
            .iter()
            .find(|session| session.ref_value == "multi-first")
            .unwrap_or_else(|| panic!("first session missing: {sessions:?}"));
        let second = sessions
            .iter()
            .find(|session| session.ref_value == "multi-second")
            .unwrap_or_else(|| panic!("second session missing: {sessions:?}"));
        assert!(!first.live);
        assert!(second.live);
        assert_eq!(
            second.pane_id.as_deref(),
            app.public_pane_id(0, second_pane).as_deref()
        );
        assert_eq!(app.project_runtime_leases.len(), 1);

        let second_public = app.public_pane_id(0, second_pane).expect("second pane id");
        let response = app.dispatch_api_request(
            "close-second",
            crate::api::schema::Method::PaneClose(crate::api::schema::PaneTarget {
                pane_id: second_public,
            }),
        );
        assert!(response.contains("\"ok\""), "close failed: {response}");
        assert!(app.project_runtime_leases.is_empty());
        assert!(app.state.workspaces.is_empty());
        assert_eq!(app.state.projects.snapshot, app.project_service.snapshot());
    }

    #[test]
    fn tab_and_workspace_close_clear_every_removed_runtime_mapping() {
        let (mut app, first_pane) = app_with_project_runtime();
        let second_tab = app.state.workspaces[0].test_add_tab(Some("second"));
        app.state.ensure_test_terminals();
        let second_pane = app.state.workspaces[0].tabs[second_tab].root_pane;
        report(&mut app, first_pane, "tab-first", 1, "startup");
        report(&mut app, second_pane, "tab-second", 1, "startup");

        let first_tab_id = app.public_tab_id(0, 0).expect("first tab id");
        let response = app.dispatch_api_request(
            "close-tab",
            crate::api::schema::Method::TabClose(crate::api::schema::TabTarget {
                tab_id: first_tab_id,
            }),
        );
        assert!(response.contains("\"ok\""), "tab close failed: {response}");
        let snapshot = app.project_service.snapshot();
        let sessions = snapshot.projects[0].sessions.as_slice();
        assert!(
            !sessions
                .iter()
                .find(|session| session.ref_value == "tab-first")
                .expect("closed tab session")
                .live
        );
        assert!(
            sessions
                .iter()
                .find(|session| session.ref_value == "tab-second")
                .expect("surviving tab session")
                .live
        );
        assert_eq!(app.project_runtime_leases.len(), 1);

        let workspace_id = app.public_workspace_id(0);
        let response = app.dispatch_api_request(
            "close-workspace",
            crate::api::schema::Method::WorkspaceClose(crate::api::schema::WorkspaceTarget {
                workspace_id,
            }),
        );
        assert!(
            response.contains("\"ok\""),
            "workspace close failed: {response}"
        );
        assert!(app.project_runtime_leases.is_empty());
        assert!(app.project_service.snapshot().projects[0]
            .sessions
            .iter()
            .all(|session| !session.live));
        assert_eq!(app.state.projects.snapshot, app.project_service.snapshot());
    }

    #[test]
    fn rejected_last_tab_close_keeps_runtime_mapping_live() {
        let (mut app, pane_id) = app_with_project_runtime();
        report(&mut app, pane_id, "last-tab", 1, "startup");
        let tab_id = app.public_tab_id(0, 0).expect("tab id");

        let response = app.dispatch_api_request(
            "close-last-tab",
            crate::api::schema::Method::TabClose(crate::api::schema::TabTarget { tab_id }),
        );

        assert!(response.contains("tab_close_failed"));
        assert_eq!(app.project_runtime_leases.len(), 1);
        assert!(app.project_service.snapshot().projects[0].sessions[0].live);
    }
}
