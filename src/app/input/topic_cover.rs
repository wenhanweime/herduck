use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::api::schema::{
    ErrorResponse, Method, ResponseResult, SuccessResponse, TopicCoverUpdateParams,
};
use crate::app::state::{AppState, Mode, TopicCoverEditor};
use crate::app::App;
use crate::projects::{cover::MAX_COVER_TEXT_CHARS, TopicCoverPatch};

impl AppState {
    pub(crate) fn open_topic_detail(&mut self, topic_key: &str) -> bool {
        if !self
            .projects
            .snapshot
            .topics
            .iter()
            .any(|topic| topic.canonical_key == topic_key)
        {
            return false;
        }
        if self.projects.topic_detail_key.as_deref() != Some(topic_key) {
            self.projects.topic_detail_selected = 0;
            self.projects.topic_detail_scroll = 0;
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
        let visible = usize::from(self.state.view.topic_detail.sessions.height.div_ceil(2)).max(1);
        self.state.projects.topic_detail_scroll = self
            .state
            .projects
            .topic_detail_scroll
            .clamp(selected.saturating_sub(visible - 1), selected);
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
                .map(|response| response.error.message)
                .unwrap_or_else(|_| "Could not save Topic cover".into());
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
                let visible =
                    usize::from(self.state.view.topic_detail.sessions.height.div_ceil(2)).max(1);
                self.state.projects.topic_detail_scroll = (self.state.projects.topic_detail_scroll
                    + 1)
                .min(count.saturating_sub(visible));
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
