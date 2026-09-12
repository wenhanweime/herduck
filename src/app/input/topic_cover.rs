use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::api::schema::{
    ErrorResponse, Method, ResponseResult, SuccessResponse, TopicCoverUpdateParams,
};
use crate::app::state::{
    AppState, Mode, ProjectOverviewAction, ProjectOverviewHit, TopicCoverEditor,
};
use crate::app::App;
use crate::projects::{cover::MAX_COVER_TEXT_CHARS, TopicCoverPatch};

impl AppState {
    pub(crate) fn open_topic_detail(&mut self, topic_key: &str) -> bool {
        if !self
            .projects
            .snapshot
            .topics
            .iter()
            .chain(self.projects.snapshot.projects.iter())
            .any(|topic| topic.canonical_key == topic_key)
        {
            return false;
        }
        if self.projects.topic_detail_key.as_deref() != Some(topic_key) {
            self.projects.topic_detail_selected = 0;
            self.projects.topic_detail_scroll = 0;
            self.projects.overview = None;
        }
        self.projects.topic_detail_key = Some(topic_key.to_string());
        self.projects.search_focused = false;
        self.mode = Mode::TopicDetail;
        true
    }

    pub(crate) fn open_topic_cover_editor(&mut self) {
        let Some(topic) = self.projects.snapshot.topics.iter().find(|topic| {
            Some(topic.canonical_key.as_str()) == self.projects.topic_detail_key.as_deref()
        }) else {
            return;
        };
        let cover = topic.cover.clone().unwrap_or_default();
        let fields = [
            cover.goal,
            cover.next_steps.first().cloned().unwrap_or_default(),
            cover.next_steps.get(1).cloned().unwrap_or_default(),
            cover.next_steps.get(2).cloned().unwrap_or_default(),
            cover.blocked_note,
        ];
        self.projects.cover_editor = Some(TopicCoverEditor {
            topic_key: topic.canonical_key.clone(),
            topic_name: topic.display_name.clone(),
            cursor: fields[0].len(),
            fields,
            focused_field: 0,
            expected_updated_at: cover.updated_at,
            error: String::new(),
        });
        self.mode = Mode::EditTopicCover;
    }
}

impl TopicCoverEditor {
    pub(crate) fn focus(&mut self, index: usize) {
        self.focused_field = index.min(self.fields.len() - 1);
        self.cursor = self.fields[self.focused_field].len();
    }

    pub(crate) fn insert(&mut self, text: &str) {
        let text: String = text
            .replace("\r\n", "\n")
            .chars()
            .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
            .collect();
        let field = &mut self.fields[self.focused_field];
        if field.chars().count() + text.chars().count() > MAX_COVER_TEXT_CHARS {
            self.error = "Each field can contain up to 2000 characters".into();
            return;
        }
        field.insert_str(self.cursor, &text);
        self.cursor += text.len();
        self.error.clear();
    }

    fn patch(&self) -> TopicCoverPatch {
        TopicCoverPatch {
            goal: Some(self.fields[0].clone()),
            next_steps: Some(
                self.fields[1..4]
                    .iter()
                    .filter(|value| !value.trim().is_empty())
                    .cloned()
                    .collect(),
            ),
            blocked_note: Some(self.fields[4].clone()),
            expected_updated_at: Some(self.expected_updated_at),
        }
    }
}

impl App {
    pub(crate) fn handle_topic_detail_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Tab => self.state.mode = Mode::Navigate,
            KeyCode::Char('e') if key.modifiers.is_empty() => self.state.open_topic_cover_editor(),
            KeyCode::Char('r') if key.modifiers.is_empty() => {
                self.refresh_project_overview(true);
            }
            KeyCode::Char(number @ '1'..='3') if key.modifiers.is_empty() => {
                let shortcut = number as u8 - b'0';
                let hit = self
                    .state
                    .view
                    .topic_detail
                    .overview_hits
                    .iter()
                    .find(|hit| hit.shortcut == Some(shortcut))
                    .cloned();
                if let Some(hit) = hit {
                    self.activate_project_overview_hit(hit);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_topic_detail_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_topic_detail_selection(1),
            KeyCode::PageUp => self.move_topic_detail_selection(-5),
            KeyCode::PageDown => self.move_topic_detail_selection(5),
            KeyCode::Enter => {
                let action = crate::ui::topic_detail_rows(&self.state)
                    .get(self.state.projects.topic_detail_selected)
                    .and_then(crate::ui::ProjectTreeRow::action);
                if let Some(action) = action {
                    self.execute_project_tree_action(action);
                }
            }
            _ => {}
        }
    }

    fn activate_project_overview_item(&mut self, session_key: Option<String>) {
        let Some(key) = session_key else {
            self.state.open_topic_cover_editor();
            return;
        };
        let action = self.state.selected_project_summary().and_then(|project| {
            project
                .sessions
                .iter()
                .find(|session| session.stable_key == key)
                .and_then(|session| crate::ui::ProjectTreeRow::Session(session.clone()).action())
        });
        if let Some(action) = action {
            self.execute_project_tree_action(action);
        }
    }

    fn activate_project_overview_hit(&mut self, hit: ProjectOverviewHit) {
        match hit.action {
            ProjectOverviewAction::OpenConversation => {
                self.activate_project_overview_item(hit.session_key)
            }
            ProjectOverviewAction::Continue {
                project_key,
                suggestion_id,
            } => match self.start_project_followup(&project_key, &suggestion_id) {
                Ok(followup) => {
                    if followup.state != crate::projects::followup::FollowupState::Cancelled {
                        if let Some((index, pane_id)) = self.parse_pane_id(&followup.pane_id) {
                            self.state.projects.history_session_key = None;
                            self.focus_pane_internal_via_api(index, pane_id);
                            self.state.mode = Mode::Terminal;
                        }
                    }
                    self.project_followup_feedback(&followup.message, false);
                }
                Err((_, message)) => self.project_followup_feedback(&message, true),
            },
            ProjectOverviewAction::CancelFollowup(id) => {
                if let Some(followup) = self.cancel_project_followup(&id) {
                    self.project_followup_feedback(&followup.message, false);
                    self.refresh_project_overview(false);
                }
            }
        }
    }

    fn project_followup_feedback(&mut self, message: &str, failed: bool) {
        let previous = self.state.toast.clone();
        self.state.toast = Some(crate::app::state::ToastNotification {
            kind: if failed {
                crate::app::ToastKind::NeedsAttention
            } else {
                crate::app::ToastKind::Finished
            },
            title: if failed {
                self.state
                    .title_language
                    .text("Follow-up not sent", "跟进未发送")
            } else {
                self.state
                    .title_language
                    .text("Project follow-up", "项目跟进")
            }
            .into(),
            context: message.into(),
            position: None,
            target: None,
        });
        self.sync_toast_deadline(previous);
    }

    fn move_topic_detail_selection(&mut self, delta: isize) {
        let last = crate::ui::topic_detail_rows(&self.state)
            .len()
            .saturating_sub(1);
        let selected = self
            .state
            .projects
            .topic_detail_selected
            .saturating_add_signed(delta)
            .min(last);
        self.state.projects.topic_detail_selected = selected;
        self.state.projects.topic_detail_scroll =
            crate::ui::topic_detail_selection_scroll(&self.state);
    }

    pub(crate) fn handle_topic_cover_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Esc {
            self.cancel_topic_cover_editor();
            return;
        }
        if (key.code == KeyCode::Enter && key.modifiers.is_empty())
            || (key.code == KeyCode::Char('s') && key.modifiers == KeyModifiers::CONTROL)
        {
            self.save_topic_cover();
            return;
        }
        let Some(editor) = self.state.projects.cover_editor.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Tab | KeyCode::Down => editor.focus((editor.focused_field + 1) % 5),
            KeyCode::BackTab | KeyCode::Up => editor.focus((editor.focused_field + 4) % 5),
            KeyCode::Home => editor.cursor = 0,
            KeyCode::End => editor.cursor = editor.fields[editor.focused_field].len(),
            KeyCode::Left => {
                editor.cursor =
                    previous_boundary(&editor.fields[editor.focused_field], editor.cursor)
            }
            KeyCode::Right => {
                editor.cursor = next_boundary(&editor.fields[editor.focused_field], editor.cursor)
            }
            KeyCode::Backspace => {
                let previous =
                    previous_boundary(&editor.fields[editor.focused_field], editor.cursor);
                editor.fields[editor.focused_field].replace_range(previous..editor.cursor, "");
                editor.cursor = previous;
            }
            KeyCode::Delete => {
                let next = next_boundary(&editor.fields[editor.focused_field], editor.cursor);
                editor.fields[editor.focused_field].replace_range(editor.cursor..next, "");
            }
            KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => {
                editor.fields[editor.focused_field].clear();
                editor.cursor = 0;
            }
            KeyCode::Enter if key.modifiers == KeyModifiers::SHIFT => editor.insert("\n"),
            KeyCode::Char(c)
                if !key.modifiers.intersects(
                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                ) =>
            {
                editor.insert(&c.to_string());
            }
            _ => {}
        }
    }

    fn cancel_topic_cover_editor(&mut self) {
        self.state.projects.cover_editor = None;
        self.state.mode = Mode::TopicDetail;
    }

    fn save_topic_cover(&mut self) {
        let Some(editor) = self.state.projects.cover_editor.as_ref() else {
            return;
        };
        let params = TopicCoverUpdateParams {
            topic_key: editor.topic_key.clone(),
            patch: editor.patch(),
        };
        let response = self
            .dispatch_runtime_mutation("tui.topic.cover.update", Method::TopicCoverUpdate(params));
        if matches!(
            serde_json::from_str::<SuccessResponse>(&response),
            Ok(SuccessResponse {
                result: ResponseResult::TopicCoverUpdated { .. },
                ..
            })
        ) {
            self.sync_projects_snapshot();
            self.cancel_topic_cover_editor();
        } else if let Some(editor) = self.state.projects.cover_editor.as_mut() {
            editor.error = serde_json::from_str::<ErrorResponse>(&response)
                .map(|response| {
                    self.state
                        .title_language
                        .text(
                            &response.error.message,
                            match response.error.code.as_str() {
                                "conflict" => "计划已在别处更新，请重新打开后再保存。",
                                "not_found" => "此主题已不可用，请返回列表。",
                                _ => "无法保存计划，请检查输入后重试。",
                            },
                        )
                        .to_string()
                })
                .unwrap_or_else(|_| {
                    self.state
                        .title_language
                        .text("Could not save Topic cover", "无法保存主题计划，请重试。")
                        .into()
                });
        }
    }

    pub(super) fn handle_topic_cover_mouse(&mut self, mouse: MouseEvent) -> bool {
        let position = ratatui::layout::Position::new(mouse.column, mouse.row);
        if self.state.mode == Mode::EditTopicCover {
            if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                let layout = crate::ui::topic_cover_editor_geometry(
                    &self.state,
                    self.state.onboarding_full_area(),
                );
                if let Some(layout) = layout {
                    if layout.save.contains(position) {
                        self.save_topic_cover();
                    } else if layout.cancel.contains(position) {
                        self.cancel_topic_cover_editor();
                    } else if let Some((index, _)) = layout
                        .fields
                        .iter()
                        .find(|(_, rect)| rect.contains(position))
                    {
                        if let Some(editor) = self.state.projects.cover_editor.as_mut() {
                            editor.focus(*index);
                        }
                    }
                }
            }
            return true;
        }
        if self.state.mode != Mode::TopicDetail || !self.state.view.terminal_area.contains(position)
        {
            return false;
        }
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let layout = &self.state.view.topic_detail;
                if layout.edit.contains(position) {
                    self.state.open_topic_cover_editor();
                } else if layout.refresh.contains(position) {
                    self.refresh_project_overview(true);
                } else if let Some(hit) = layout
                    .overview_hits
                    .iter()
                    .find(|hit| hit.rect.contains(position))
                    .cloned()
                {
                    self.activate_project_overview_hit(hit);
                } else if layout.footer.contains(position) {
                    self.state.mode = Mode::Navigate;
                } else if let Some(hit) = layout
                    .row_hits
                    .iter()
                    .find(|hit| hit.rect.contains(position))
                    .cloned()
                {
                    self.state.projects.topic_detail_selected = hit.row_index;
                    self.execute_project_tree_action(hit.action);
                }
            }
            MouseEventKind::ScrollUp => {
                self.state.projects.topic_detail_scroll =
                    self.state.projects.topic_detail_scroll.saturating_sub(1)
            }
            MouseEventKind::ScrollDown => {
                let count = crate::ui::topic_detail_rows(&self.state).len();
                self.state.projects.topic_detail_scroll = self
                    .state
                    .projects
                    .topic_detail_scroll
                    .saturating_add(1)
                    .min(count.saturating_sub(1));
            }
            _ => {}
        }
        true
    }
}

fn previous_boundary(text: &str, cursor: usize) -> usize {
    text[..cursor]
        .char_indices()
        .next_back()
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn next_boundary(text: &str, cursor: usize) -> usize {
    text[cursor..]
        .chars()
        .next()
        .map(|c| cursor + c.len_utf8())
        .unwrap_or(cursor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::schema::{Request, TopicCoverGetParams};

    fn fixture() -> (App, String) {
        let mut app = super::super::app_for_mouse_test();
        let (service, key) =
            crate::projects::ProjectService::with_test_topic(crate::api::EventHub::default());
        app.project_service = service;
        app.state.projects.snapshot = app.project_service.snapshot();
        app.state.sidebar_view = crate::app::state::SidebarView::Clusters;
        app.state.projects.grouping = crate::app::state::ProjectGrouping::Topics;
        assert!(app.state.open_topic_detail(&key));
        (app, key)
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn api(app: &mut App, method: Method) -> serde_json::Value {
        serde_json::from_str(&app.handle_api_request(Request {
            id: "cover-test".into(),
            method,
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn project_overview_api_matches_ui_for_topics_and_folder_projects_without_writes() {
        use crate::api::schema::ProjectOverviewGetParams;
        let (mut app, topic_key) = fixture();
        let folder_key = app.state.projects.snapshot.projects[0]
            .canonical_key
            .clone();
        let revision = app.project_service.snapshot().revision;
        for key in [&topic_key, &folder_key] {
            assert!(app.state.open_topic_detail(key));
            app.sync_project_overview();
            let visible = app.state.visible_project_overview().unwrap();
            let response = api(
                &mut app,
                Method::ProjectOverviewGet(ProjectOverviewGetParams {
                    project_key: key.to_string(),
                    refresh: true,
                }),
            );
            assert_eq!(
                response["result"]["overview"],
                serde_json::to_value(&visible).unwrap()
            );
            assert_eq!(visible.counts.history, 1);
            assert!(!visible.suggestions.is_empty());
            assert_eq!(app.project_service.snapshot().revision, revision);
        }
        // API reads of another group must not move the person's current selection.
        api(
            &mut app,
            Method::ProjectOverviewGet(ProjectOverviewGetParams {
                project_key: topic_key,
                refresh: false,
            }),
        );
        assert_eq!(
            app.state.projects.topic_detail_key.as_ref(),
            Some(&folder_key)
        );
        let missing = api(
            &mut app,
            Method::ProjectOverviewGet(ProjectOverviewGetParams {
                project_key: "missing".into(),
                refresh: false,
            }),
        );
        assert_eq!(missing["error"]["code"], "not_found");
        assert!(app.state.terminals.is_empty());
    }

    #[tokio::test]
    async fn language_change_replaces_cached_overview_for_ui_api_and_feedback() {
        use crate::api::schema::ProjectOverviewGetParams;
        use crate::config::TitleLanguage;
        let (mut app, topic_key) = fixture();
        app.sync_project_overview();
        let original = app.state.visible_project_overview().unwrap();
        assert_eq!(original.language, TitleLanguage::English);
        let saved = app.state.selected_project_summary().unwrap().cover.clone();
        for language in [TitleLanguage::Chinese, TitleLanguage::English] {
            let mut summary = app.state.summary_config.clone();
            summary.title_language = language;
            app.project_service.configure_summaries(&summary).unwrap();
            app.state.title_language = language;
            app.state.summary_config = summary;
            // No draw may display a cached briefing in the old language, even before the next tick.
            assert_eq!(
                app.state.visible_project_overview().unwrap().language,
                language
            );
            app.sync_project_overview();
            let visible = app.state.visible_project_overview().unwrap();
            let response = api(
                &mut app,
                Method::ProjectOverviewGet(ProjectOverviewGetParams {
                    project_key: topic_key.clone(),
                    refresh: false,
                }),
            );
            assert_eq!(
                response["result"]["overview"],
                serde_json::to_value(&visible).unwrap()
            );
            assert_eq!(visible.suggestions[0].id, original.suggestions[0].id);
            assert!(crate::projects::localization::matches_language(
                &visible.work[0].description,
                language
            ));
            app.project_followup_feedback(
                language.text("The instruction was sent.", "指令已发送。"),
                false,
            );
            assert_eq!(
                app.state.toast.as_ref().unwrap().title,
                language.text("Project follow-up", "项目跟进")
            );
        }
        assert_eq!(app.state.selected_project_summary().unwrap().cover, saved);
        assert!(app.state.terminals.is_empty());
    }

    #[tokio::test]
    async fn overview_conversation_links_stay_read_only_from_keyboard_and_mouse() {
        for use_mouse in [false, true] {
            let (mut app, _) = fixture();
            let session_key = app.state.selected_project_summary().unwrap().sessions[0]
                .stable_key
                .clone();
            app.sync_project_overview();
            crate::ui::compute_view(&mut app.state, ratatui::layout::Rect::new(0, 0, 160, 38));
            let hit = app
                .state
                .view
                .topic_detail
                .overview_hits
                .iter()
                .find(|hit| {
                    hit.shortcut.is_none() && hit.session_key.as_ref() == Some(&session_key)
                })
                .unwrap()
                .clone();
            assert_eq!(hit.session_key.as_ref(), Some(&session_key));
            if use_mouse {
                app.handle_mouse(super::super::mouse(
                    MouseEventKind::Down(MouseButton::Left),
                    hit.rect.x,
                    hit.rect.y,
                ));
            } else {
                app.route_client_input(b"\r".to_vec());
            }
            assert_eq!(app.state.mode, Mode::ProjectHistory);
            assert_eq!(
                app.state.projects.history_session_key.as_ref(),
                Some(&session_key)
            );
            assert!(
                app.state.terminals.is_empty(),
                "viewing a conversation must not start an Agent"
            );
            assert!(app.state.workspaces.is_empty());
            app.state.assert_invariants_for_test();
        }
    }

    #[tokio::test]
    async fn overview_live_state_changes_and_actions_keep_the_existing_pane() {
        use crate::detect::AgentState;
        use crate::projects::overview::WorkPhase;
        let (mut app, topic_key) = fixture();
        app.state.workspaces = vec![crate::workspace::Workspace::test_new("review")];
        app.state.ensure_test_terminals();
        app.state.active = Some(0);
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        let terminal_id = app.state.workspaces[0]
            .terminal_id(pane_id)
            .unwrap()
            .clone();
        let workspace_id = app.public_workspace_id(0);
        let public_pane_id = app.public_pane_id(0, pane_id).unwrap();
        let snapshot = &mut app.state.projects.snapshot;
        for group in snapshot
            .projects
            .iter_mut()
            .chain(snapshot.topics.iter_mut())
        {
            let session = &mut group.sessions[0];
            session.live = true;
            session.workspace_id = Some(workspace_id.clone());
            session.pane_id = Some(public_pane_id.clone());
            session.runtime_generation = Some(1);
        }
        for (status, expected) in [
            (AgentState::Working, WorkPhase::Working),
            (AgentState::Blocked, WorkPhase::NeedsInput),
            (AgentState::Idle, WorkPhase::Ready),
        ] {
            app.state.terminals.get_mut(&terminal_id).unwrap().state = status;
            assert!(app.sync_project_overview());
            assert_eq!(
                app.state.visible_project_overview().unwrap().work[0].phase,
                expected
            );
        }
        app.state
            .terminals
            .get_mut(&terminal_id)
            .unwrap()
            .set_agent_inactive(true);
        app.sync_project_overview();
        assert_eq!(
            app.state.visible_project_overview().unwrap().work[0].phase,
            WorkPhase::Paused
        );
        app.state
            .terminals
            .get_mut(&terminal_id)
            .unwrap()
            .set_agent_inactive(false);
        app.state.terminals.get_mut(&terminal_id).unwrap().state = AgentState::Blocked;
        app.state.open_topic_detail(&topic_key);
        app.sync_project_overview();
        crate::ui::compute_view(&mut app.state, ratatui::layout::Rect::new(0, 0, 160, 38));
        app.handle_topic_detail_key(key(KeyCode::Char('1')));
        assert_eq!(app.state.mode, Mode::Terminal);
        assert_eq!(app.state.workspaces.len(), 1);
        assert_eq!(app.state.workspaces[0].tabs.len(), 1);
        assert_eq!(app.state.workspaces[0].focused_pane_id(), Some(pane_id));
        assert_eq!(app.state.terminals.len(), 1);
        app.state.assert_invariants_for_test();
    }

    #[tokio::test]
    async fn topic_cover_editor_and_api_share_saved_state_and_report_conflicts() {
        let (mut app, topic_key) = fixture();
        app.handle_topic_detail_key(key(KeyCode::Char('e')));
        assert_eq!(app.state.mode, Mode::EditTopicCover);
        for text in ["本周访谈五位用户", "发出邀请", "约好时间", "", "等待回复"]
        {
            app.paste_into_active_text_input(text);
            app.handle_topic_cover_key(key(KeyCode::Tab));
        }
        app.handle_topic_cover_key(key(KeyCode::Enter));
        assert_eq!(app.state.mode, Mode::TopicDetail);
        let response = api(
            &mut app,
            Method::TopicCoverGet(TopicCoverGetParams {
                topic_key: topic_key.clone(),
            }),
        );
        let cover = &response["result"]["cover"];
        assert_eq!(cover["goal"], "本周访谈五位用户");
        assert_eq!(
            cover["next_steps"],
            serde_json::json!(["发出邀请", "约好时间"])
        );
        assert_eq!(cover["blocked_note"], "等待回复");
        assert_eq!(
            app.state.projects.snapshot.topics[0]
                .cover
                .as_ref()
                .unwrap()
                .goal,
            "本周访谈五位用户"
        );

        app.state.open_topic_cover_editor();
        let before_editor = app
            .state
            .projects
            .cover_editor
            .as_ref()
            .unwrap()
            .fields
            .clone();
        let update = api(
            &mut app,
            Method::TopicCoverUpdate(TopicCoverUpdateParams {
                topic_key: topic_key.clone(),
                patch: TopicCoverPatch {
                    goal: Some("Agent 更新的目标".into()),
                    ..Default::default()
                },
            }),
        );
        assert!(update.get("error").is_none(), "{update}");
        app.sync_projects_snapshot();
        assert_eq!(
            app.state.projects.snapshot.topics[0]
                .cover
                .as_ref()
                .unwrap()
                .goal,
            "Agent 更新的目标"
        );
        assert_eq!(
            app.state.projects.cover_editor.as_ref().unwrap().fields,
            before_editor,
            "snapshot refresh must not replace a draft"
        );
        app.handle_topic_cover_key(key(KeyCode::Enter));
        assert_eq!(app.state.mode, Mode::EditTopicCover);
        assert!(app
            .state
            .projects
            .cover_editor
            .as_ref()
            .unwrap()
            .error
            .contains("changed"));
        app.handle_topic_cover_key(key(KeyCode::Esc));
        assert!(app.state.projects.cover_editor.is_none());
        assert_eq!(
            app.project_service.topic_cover(topic_key).unwrap().goal,
            "Agent 更新的目标"
        );
        app.state.assert_invariants_for_test();
    }

    #[tokio::test]
    async fn topic_cover_cancel_unicode_cursor_and_paste_do_not_write() {
        let (mut app, topic_key) = fixture();
        app.state.open_topic_cover_editor();
        app.paste_into_active_text_input("目标🙂");
        app.handle_topic_cover_key(key(KeyCode::Left));
        app.handle_topic_cover_key(key(KeyCode::Backspace));
        app.paste_into_active_text_input("新");
        assert_eq!(
            app.state.projects.cover_editor.as_ref().unwrap().fields[0],
            "目新🙂"
        );
        app.handle_topic_cover_key(key(KeyCode::Home));
        app.handle_topic_cover_key(key(KeyCode::Delete));
        assert_eq!(
            app.state.projects.cover_editor.as_ref().unwrap().fields[0],
            "新🙂"
        );
        app.paste_into_active_text_input(&"大".repeat(MAX_COVER_TEXT_CHARS));
        assert_eq!(
            app.state.projects.cover_editor.as_ref().unwrap().fields[0],
            "新🙂",
            "oversized paste is rejected as a whole"
        );
        assert!(!app
            .state
            .projects
            .cover_editor
            .as_ref()
            .unwrap()
            .error
            .is_empty());
        app.handle_topic_cover_key(key(KeyCode::Esc));
        assert_eq!(
            app.project_service.topic_cover(topic_key).unwrap(),
            crate::projects::TopicCover::default()
        );
    }

    #[tokio::test]
    async fn topic_cover_mouse_opens_details_edits_and_saves_without_starting_an_agent() {
        let (mut app, topic_key) = fixture();
        let area = ratatui::layout::Rect::new(0, 0, 120, 32);
        app.state.mode = Mode::Navigate;
        crate::ui::compute_view(&mut app.state, area);
        let topic_hit = app.state.view.project_row_hit_areas.iter().find(|hit| matches!(&hit.action,
            crate::app::state::ProjectTreeAction::ToggleProject { project_key, .. } if project_key == &topic_key)).unwrap().rect;
        app.handle_mouse(super::super::mouse(
            MouseEventKind::Down(MouseButton::Left),
            topic_hit.x,
            topic_hit.y,
        ));
        assert_eq!(app.state.mode, Mode::TopicDetail);
        crate::ui::compute_view(&mut app.state, area);
        let edit = app.state.view.topic_detail.edit;
        app.handle_mouse(super::super::mouse(
            MouseEventKind::Down(MouseButton::Left),
            edit.x,
            edit.y,
        ));
        assert_eq!(app.state.mode, Mode::EditTopicCover);
        app.paste_into_active_text_input("Mouse-edited goal");
        let layout = crate::ui::topic_cover_editor_geometry(&app.state, area).unwrap();
        app.handle_mouse(super::super::mouse(
            MouseEventKind::Down(MouseButton::Left),
            layout.save.x,
            layout.save.y,
        ));
        assert_eq!(app.state.mode, Mode::TopicDetail);
        assert_eq!(
            app.project_service.topic_cover(topic_key).unwrap().goal,
            "Mouse-edited goal"
        );
        assert!(app.state.terminals.is_empty());
        assert!(app.state.workspaces.is_empty());
    }

    #[tokio::test]
    async fn topic_cover_api_rejects_unknown_topics_and_overflow_atomically() {
        let (mut app, topic_key) = fixture();
        let revision = app.project_service.snapshot().revision;
        let response = api(
            &mut app,
            Method::TopicCoverUpdate(TopicCoverUpdateParams {
                topic_key: topic_key.clone(),
                patch: TopicCoverPatch {
                    goal: Some("Do not save".into()),
                    next_steps: Some(vec!["step".into(); 4]),
                    ..Default::default()
                },
            }),
        );
        assert_eq!(response["error"]["code"], "invalid_topic_cover");
        assert_eq!(app.project_service.snapshot().revision, revision);
        assert_eq!(
            app.project_service.topic_cover(topic_key).unwrap(),
            crate::projects::TopicCover::default()
        );
        let response = api(
            &mut app,
            Method::TopicCoverGet(TopicCoverGetParams {
                topic_key: "missing".into(),
            }),
        );
        assert_eq!(response["error"]["code"], "not_found");
    }

    #[tokio::test]
    async fn topic_cover_headless_client_keys_use_the_same_editor_and_writer() {
        let (mut app, topic_key) = fixture();
        app.route_client_input(b"e".to_vec());
        assert_eq!(app.state.mode, Mode::EditTopicCover);
        app.route_client_input("通过客户端写目标".as_bytes().to_vec());
        app.route_client_input(b"\r".to_vec());
        assert_eq!(app.state.mode, Mode::TopicDetail);
        assert_eq!(
            app.project_service.topic_cover(topic_key).unwrap().goal,
            "通过客户端写目标"
        );
    }
}
