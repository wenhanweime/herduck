use crate::{
    app::{App, Mode},
    events::AppEvent,
    input::{KeyboardProtocol, TerminalKey},
    quick_keys::{modifier, Configuration, ItemKind, QuickKey},
    ui::quick_keys::{self, DockTarget, EditorAction},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Position;

impl App {
    pub(crate) fn apply_quick_key_modifiers(&mut self, key: TerminalKey) -> Option<TerminalKey> {
        let injected_cmd = self
            .state
            .quick_keys
            .modifiers
            .contains(KeyModifiers::SUPER)
            && key.kind == crossterm::event::KeyEventKind::Press;
        let modified = self.state.quick_keys.apply_modifiers(key);
        if injected_cmd {
            let supports_cmd = self
                .state
                .active
                .and_then(|ws_idx| {
                    self.state
                        .focused_runtime_in_workspace(&self.terminal_runtimes, ws_idx)
                })
                .is_some_and(|runtime| {
                    matches!(runtime.keyboard_protocol(), KeyboardProtocol::Kitty { .. })
                });
            if !supports_cmd {
                self.state.quick_keys.modified_keys.remove(&key.code);
                self.state
                    .quick_keys
                    .notify("此终端不支持 Cmd 组合键；请改用 Ctrl 或应用动作".into());
                return None;
            }
        }
        Some(modified)
    }

    pub(crate) fn quick_keys_editor_key(&mut self, key: KeyEvent) -> bool {
        if self.state.quick_keys.editor.is_none() {
            return false;
        }
        if key.kind == crossterm::event::KeyEventKind::Release {
            return true;
        }
        if key.code == KeyCode::Esc {
            self.edit_quick_keys(EditorAction::Cancel);
            return true;
        }
        if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.edit_quick_keys(EditorAction::Save);
            return true;
        }
        let Some(editor) = self.state.quick_keys.editor.as_mut() else {
            return true;
        };
        match key.code {
            KeyCode::Tab => editor.field = (editor.field + 1) % 4,
            KeyCode::BackTab => editor.field = (editor.field + 3) % 4,
            KeyCode::Up => editor.selected = editor.selected.saturating_sub(1),
            KeyCode::Down => {
                editor.selected =
                    (editor.selected + 1).min(editor.draft.items.len().saturating_sub(1))
            }
            KeyCode::Backspace => {
                if let Some(item) = editor.draft.items.get_mut(editor.selected) {
                    match editor.field {
                        0 => {
                            item.label.pop();
                        }
                        2 => {
                            item.payload.pop();
                        }
                        _ => {}
                    }
                }
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ')
                if editor.field == 1 || editor.field == 3 =>
            {
                let field = editor.field;
                if let Some(item) = editor.draft.items.get_mut(editor.selected) {
                    if field == 1 {
                        let index = ItemKind::ALL
                            .iter()
                            .position(|kind| *kind == item.kind)
                            .unwrap_or(0);
                        let step = if key.code == KeyCode::Left { 5 } else { 1 };
                        item.kind = ItemKind::ALL[(index + step) % 6];
                    } else {
                        item.auto_enter = !item.auto_enter;
                    }
                }
            }
            KeyCode::Char(c)
                if !key.modifiers.intersects(
                    KeyModifiers::CONTROL | KeyModifiers::SUPER | KeyModifiers::ALT,
                ) =>
            {
                editor.insert(&c.to_string())
            }
            _ => {}
        }
        true
    }

    fn edit_quick_keys(&mut self, action: EditorAction) {
        if action == EditorAction::Cancel {
            self.state.quick_keys.editor = None;
            return;
        }
        if action == EditorAction::Save {
            let Some(editor) = &mut self.state.quick_keys.editor else {
                return;
            };
            let result = match &self.state.quick_keys.path {
                Some(path) => editor.draft.save(path),
                None => editor.draft.validate(),
            };
            match result {
                Ok(()) => {
                    self.state.quick_keys.config = editor.draft.clone();
                    self.state.quick_keys.editor = None;
                    self.state.quick_keys.modifiers = KeyModifiers::empty();
                    self.state.quick_keys.locked = KeyModifiers::empty();
                    self.state.quick_keys.notify("快捷栏已保存".into());
                }
                Err(error) => editor.error = error,
            }
            return;
        }
        let Some(editor) = self.state.quick_keys.editor.as_mut() else {
            return;
        };
        match action {
            EditorAction::Select(idx) => editor.selected = idx,
            EditorAction::Previous => editor.selected = editor.selected.saturating_sub(1),
            EditorAction::Next => {
                editor.selected =
                    (editor.selected + 1).min(editor.draft.items.len().saturating_sub(1))
            }
            EditorAction::MoveUp if editor.selected > 0 => {
                editor
                    .draft
                    .items
                    .swap(editor.selected, editor.selected - 1);
                editor.selected -= 1;
            }
            EditorAction::MoveDown if editor.selected + 1 < editor.draft.items.len() => {
                editor
                    .draft
                    .items
                    .swap(editor.selected, editor.selected + 1);
                editor.selected += 1;
            }
            EditorAction::Add if editor.draft.items.len() < 64 => {
                editor
                    .draft
                    .items
                    .push(QuickKey::new("新快捷键", ItemKind::Text, ""));
                editor.selected = editor.draft.items.len() - 1;
                editor.field = 0;
            }
            EditorAction::Delete => {
                if editor.selected < editor.draft.items.len() {
                    editor.draft.items.remove(editor.selected);
                    editor.selected = editor
                        .selected
                        .min(editor.draft.items.len().saturating_sub(1));
                }
            }
            EditorAction::Field(field) => {
                editor.field = field;
                if let Some(item) = editor.draft.items.get_mut(editor.selected) {
                    if field == 1 {
                        let index = ItemKind::ALL
                            .iter()
                            .position(|kind| *kind == item.kind)
                            .unwrap_or(0);
                        item.kind = ItemKind::ALL[(index + 1) % 6];
                    }
                    if field == 3 {
                        item.auto_enter = !item.auto_enter;
                    }
                }
            }
            EditorAction::Reset => {
                editor.draft = Configuration::default();
                editor.selected = 0;
            }
            _ => {}
        }
        editor.error.clear();
    }

    pub(crate) fn quick_keys_mouse(&mut self, mouse: MouseEvent) -> bool {
        if self.state.quick_keys.editor.is_some() {
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                let screen = self.quick_keys_screen();
                if let Some((_, action)) = quick_keys::editor_hits(&self.state, screen)
                    .into_iter()
                    .find(|(rect, _)| rect.contains(Position::new(mouse.column, mouse.row)))
                {
                    self.edit_quick_keys(action);
                }
            }
            return true;
        }
        // Other modal surfaces keep control of their own input.
        if !matches!(
            self.state.mode,
            Mode::Terminal | Mode::Navigate | Mode::Prefix | Mode::Copy
        ) {
            return false;
        }
        let rect = self.state.quick_keys.rect;
        let inside = rect.contains(Position::new(mouse.column, mouse.row));
        if !inside && self.state.quick_keys.press.is_none() {
            return false;
        }
        if self.state.quick_keys.collapsed
            && self.state.quick_keys.press.is_none()
            && quick_keys::target(&self.state, mouse.column, mouse.row) != Some(DockTarget::Toggle)
        {
            return false;
        }
        let width = quick_keys::viewport(rect).width;
        let max = quick_keys::content_width(&self.state).saturating_sub(width);
        match mouse.kind {
            MouseEventKind::ScrollRight | MouseEventKind::ScrollDown
                if inside && !self.state.quick_keys.collapsed =>
            {
                self.state.quick_keys.offset =
                    self.state.quick_keys.offset.saturating_add(8).min(max);
            }
            MouseEventKind::ScrollLeft | MouseEventKind::ScrollUp
                if inside && !self.state.quick_keys.collapsed =>
            {
                self.state.quick_keys.offset = self.state.quick_keys.offset.saturating_sub(8);
            }
            MouseEventKind::Down(MouseButton::Left) if inside => {
                self.state.quick_keys.activated = None;
                self.state.quick_keys.press =
                    Some((mouse.column, self.state.quick_keys.offset, false));
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some((start, initial, dragged)) = self.state.quick_keys.press.as_mut() {
                    let delta = i32::from(*start) - i32::from(mouse.column);
                    if delta.abs() >= 2 {
                        *dragged = true;
                    }
                    if *dragged && !self.state.quick_keys.collapsed {
                        self.state.quick_keys.offset =
                            (i32::from(*initial) + delta).clamp(0, i32::from(max)) as u16;
                    }
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if let Some((start, _, dragged)) = self.state.quick_keys.press.take() {
                    if inside && !dragged && start.abs_diff(mouse.column) < 2 {
                        match quick_keys::target(&self.state, mouse.column, mouse.row) {
                            Some(DockTarget::Left) => {
                                self.state.quick_keys.offset = self
                                    .state
                                    .quick_keys
                                    .offset
                                    .saturating_sub(width.max(1) / 2 + 1)
                            }
                            Some(DockTarget::Right) => {
                                self.state.quick_keys.offset = self
                                    .state
                                    .quick_keys
                                    .offset
                                    .saturating_add(width.max(1) / 2 + 1)
                                    .min(max)
                            }
                            Some(DockTarget::Edit) => {
                                self.state.mode = Mode::Terminal;
                                self.state.quick_keys.open_editor();
                            }
                            Some(DockTarget::Toggle) => {
                                self.state.quick_keys.toggle_collapsed();
                            }
                            Some(DockTarget::Item(idx)) => self.activate_quick_key(idx),
                            None => {}
                        }
                    }
                }
            }
            _ => {}
        }
        true
    }

    fn quick_keys_screen(&self) -> ratatui::layout::Rect {
        self.state.screen_rect()
    }

    fn activate_quick_key(&mut self, index: usize) {
        let Some(item) = self.state.quick_keys.config.items.get(index).cloned() else {
            return;
        };
        self.state.quick_keys.activated = Some((index, std::time::Instant::now()));
        match item.kind {
            ItemKind::Modifier => {
                self.state.mode = Mode::Terminal;
                if let Some(modifier) = modifier(&item.payload) {
                    self.state.quick_keys.toggle_modifier(modifier);
                }
            }
            ItemKind::AppAction => match item.payload.as_str() {
                "herduck" => {
                    self.state.mode = Mode::Prefix;
                }
                "close" => super::modal::request_detach(&mut self.state),
                "copy" => {
                    if self.state.selection.is_some() {
                        self.state.copy_selection(&self.terminal_runtimes);
                        if let Some(content) = self.state.request_clipboard_write.take() {
                            let _ = self.event_tx.try_send(AppEvent::ClipboardWrite { content });
                        }
                    } else {
                        self.state.enter_copy_mode(&self.terminal_runtimes);
                        self.state
                            .quick_keys
                            .notify("已进入选择模式；选择文本后复制".into());
                    }
                }
                "paste" | "input_source" => {
                    if item.payload == "paste" {
                        self.state.mode = Mode::Terminal;
                    }
                    let action = if item.payload == "paste" {
                        crate::protocol::HostAction::PasteClipboard
                    } else {
                        crate::protocol::HostAction::ToggleInputSource
                    };
                    let target = if item.payload == "paste" {
                        self.state
                            .popup_pane
                            .as_ref()
                            .map(|pane| pane.terminal_id.clone())
                            .or_else(|| {
                                let ws = self
                                    .state
                                    .active
                                    .and_then(|i| self.state.workspaces.get(i))?;
                                ws.terminal_id(ws.focused_pane_id()?).cloned()
                            })
                    } else {
                        None
                    };
                    if self
                        .event_tx
                        .try_send(AppEvent::HostAction { action, target })
                        .is_err()
                    {
                        self.state.quick_keys.notify("操作队列繁忙，请重试".into());
                    }
                }
                _ => {}
            },
            ItemKind::Text => {
                self.state.mode = Mode::Terminal;
                if let Some(ws_idx) = self.state.active {
                    if let Some(runtime) = self
                        .state
                        .focused_runtime_in_workspace(&self.terminal_runtimes, ws_idx)
                    {
                        let mut bytes = item.payload.into_bytes();
                        if item.auto_enter {
                            bytes.extend(runtime.encode_terminal_key(TerminalKey::new(
                                KeyCode::Enter,
                                KeyModifiers::empty(),
                            )));
                        }
                        if runtime.try_send_bytes(bytes.into()).is_err() {
                            self.state
                                .quick_keys
                                .notify("终端输入队列繁忙，请重试".into());
                        }
                    }
                }
            }
            ItemKind::Key | ItemKind::Chord | ItemKind::Sequence => {
                self.state.mode = Mode::Terminal;
                let Ok(keys) = item.keys() else {
                    return;
                };
                let keys: Vec<_> = keys
                    .into_iter()
                    .map(|key| {
                        let original = key.code;
                        let modified = self.state.quick_keys.apply_modifiers(key);
                        // Dock taps have no paired physical key release.
                        self.state.quick_keys.modified_keys.remove(&original);
                        modified
                    })
                    .collect();
                let Some(ws) = self.state.active.and_then(|i| self.state.workspaces.get(i)) else {
                    return;
                };
                let Some(pane_id) = ws.focused_pane_id() else {
                    return;
                };
                let Some(terminal_id) = ws.terminal_id(pane_id).cloned() else {
                    return;
                };
                let Some(runtime) = self.terminal_runtimes.get(&terminal_id) else {
                    return;
                };
                if keys
                    .iter()
                    .any(|key| key.modifiers.contains(KeyModifiers::SUPER))
                    && !matches!(runtime.keyboard_protocol(), KeyboardProtocol::Kitty { .. })
                {
                    self.state
                        .quick_keys
                        .notify("此终端不支持 Cmd 组合键；请改用 Ctrl 或应用动作".into());
                    return;
                }
                if item.kind == ItemKind::Sequence {
                    let sender = self.event_tx.clone();
                    tokio::spawn(async move {
                        for key in keys {
                            if sender
                                .send(AppEvent::TerminalInput {
                                    terminal_id: terminal_id.clone(),
                                    key,
                                })
                                .await
                                .is_err()
                            {
                                break;
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(40)).await;
                        }
                    });
                } else {
                    for key in keys {
                        let bytes = runtime.encode_terminal_key(key);
                        runtime.scroll_reset();
                        if runtime.try_send_bytes(bytes.into()).is_err() {
                            self.state
                                .quick_keys
                                .notify("终端输入队列繁忙，请重试".into());
                            break;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_input() -> (App, tokio::sync::mpsc::Receiver<bytes::Bytes>) {
        let mut app = super::super::app_for_mouse_test();
        app.state.quick_keys = crate::quick_keys::QuickKeysState::default();
        app.state.workspaces = vec![crate::workspace::Workspace::test_new("dock")];
        app.state.ensure_test_terminals();
        app.state.active = Some(0);
        let ws = &app.state.workspaces[0];
        let terminal_id = ws
            .terminal_id(ws.focused_pane_id().unwrap())
            .unwrap()
            .clone();
        let (runtime, receiver) = crate::terminal::TerminalRuntime::test_with_channel(80, 24);
        app.terminal_runtimes.insert(terminal_id, runtime);
        (app, receiver)
    }

    #[tokio::test]
    async fn quick_keys_send_controls_chinese_text_and_application_cursor() {
        let (mut app, mut receiver) = app_with_input();
        app.activate_quick_key(0);
        assert_eq!(receiver.try_recv().unwrap().as_ref(), b"\x1b");
        app.activate_quick_key(5);
        assert_eq!(receiver.try_recv().unwrap().as_ref(), b"\x03");
        app.activate_quick_key(4);
        assert_eq!(receiver.try_recv().unwrap().as_ref(), "继续\r".as_bytes());
        let runtime = app.terminal_runtimes.values().next().unwrap();
        runtime.test_process_pty_bytes(b"\x1b[?1h");
        app.activate_quick_key(9);
        assert_eq!(receiver.try_recv().unwrap().as_ref(), b"\x1bOA");
        app.state
            .quick_keys
            .config
            .items
            .push(QuickKey::new("Cmd S", ItemKind::Chord, "cmd+s"));
        app.activate_quick_key(13);
        assert!(
            receiver.try_recv().is_err(),
            "legacy Cmd must not send a bare s"
        );
        assert!(app.state.quick_keys.notice.is_some());
    }

    #[tokio::test]
    async fn quick_keys_paste_requests_client_clipboard_and_editor_ignores_release() {
        let (mut app, mut receiver) = app_with_input();
        let expected_target = app.state.workspaces[0]
            .terminal_id(app.state.workspaces[0].focused_pane_id().unwrap())
            .cloned();
        app.state.mode = Mode::Prefix;
        app.activate_quick_key(7);
        assert_eq!(app.state.mode, Mode::Terminal);
        match app.event_rx.try_recv().expect("host paste request") {
            AppEvent::HostAction {
                action: crate::protocol::HostAction::PasteClipboard,
                target,
            } => {
                assert_eq!(
                    target, expected_target,
                    "capture origin terminal when the button is pressed"
                );
            }
            other => panic!("unexpected event: {other:?}"),
        }
        assert!(receiver.try_recv().is_err());
        app.state.quick_keys.open_editor();
        let before = app
            .state
            .quick_keys
            .editor
            .as_ref()
            .unwrap()
            .item()
            .unwrap()
            .label
            .clone();
        app.quick_keys_editor_key(KeyEvent::new_with_kind(
            KeyCode::Char('x'),
            KeyModifiers::empty(),
            crossterm::event::KeyEventKind::Release,
        ));
        assert_eq!(
            app.state
                .quick_keys
                .editor
                .as_ref()
                .unwrap()
                .item()
                .unwrap()
                .label,
            before
        );
    }
    #[tokio::test]
    async fn quick_keys_drag_does_not_activate_and_close_only_detaches() {
        let mut app = super::super::app_for_mouse_test();
        app.state.quick_keys = crate::quick_keys::QuickKeysState::default();
        app.state.mode = Mode::Terminal;
        app.state.quick_keys.rect = ratatui::layout::Rect::new(0, 19, 32, 1);
        let event = |kind, column| MouseEvent {
            kind,
            column,
            row: 19,
            modifiers: KeyModifiers::empty(),
        };
        app.quick_keys_mouse(event(MouseEventKind::Down(MouseButton::Left), 14));
        app.quick_keys_mouse(event(MouseEventKind::Drag(MouseButton::Left), 4));
        app.quick_keys_mouse(event(MouseEventKind::Up(MouseButton::Left), 4));
        assert_eq!(app.state.quick_keys.offset, 10);
        assert!(app.state.quick_keys.modifiers.is_empty());
        assert!(app.state.quick_keys.activated.is_none());
        app.state.detach_exits = false;
        app.activate_quick_key(8);
        assert!(app.state.detach_requested);
        assert!(!app.state.should_quit);
    }
    #[tokio::test]
    async fn quick_keys_tap_highlights_until_timer_and_sends_only_on_release() {
        let (mut app, mut receiver) = app_with_input();
        app.state.quick_keys.rect = ratatui::layout::Rect::new(0, 23, 80, 1);
        let event = |kind, column| MouseEvent {
            kind,
            column,
            row: 23,
            modifiers: KeyModifiers::empty(),
        };
        app.quick_keys_mouse(event(MouseEventKind::Down(MouseButton::Left), 6));
        assert!(app.state.quick_keys.press.is_some());
        assert!(receiver.try_recv().is_err());
        app.quick_keys_mouse(event(MouseEventKind::Up(MouseButton::Left), 6));
        assert_eq!(receiver.try_recv().unwrap().as_ref(), b"\x1b");
        assert!(receiver.try_recv().is_err());
        assert!(app.state.quick_keys.press.is_none());
        assert_eq!(app.state.quick_keys.activated.unwrap().0, 0);
        let deadline = app.state.quick_keys.activation_deadline().unwrap();
        assert!(!app
            .state
            .quick_keys
            .expire_activation(deadline - std::time::Duration::from_millis(1)));
        assert!(app.state.quick_keys.expire_activation(deadline));
        assert!(app.state.quick_keys.activated.is_none());
    }
    #[tokio::test]
    async fn quick_keys_edit_is_transactional_and_utf8_safe() {
        let mut app = super::super::app_for_mouse_test();
        app.state.quick_keys = crate::quick_keys::QuickKeysState::default();
        app.state.quick_keys.path = None;
        app.state.quick_keys.open_editor();
        app.edit_quick_keys(EditorAction::MoveDown);
        app.edit_quick_keys(EditorAction::Add);
        app.quick_keys_editor_key(KeyEvent::new(KeyCode::Char('中'), KeyModifiers::empty()));
        app.quick_keys_editor_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
        app.edit_quick_keys(EditorAction::Cancel);
        assert_eq!(app.state.quick_keys.config, Configuration::default());
        app.state.quick_keys.open_editor();
        app.edit_quick_keys(EditorAction::MoveDown);
        app.edit_quick_keys(EditorAction::Save);
        assert_eq!(app.state.quick_keys.config.items[0].label, "Shift");
    }

    #[tokio::test]
    async fn quick_keys_toggle_on_release_preserves_config_without_sending_input() {
        let (mut app, mut receiver) = app_with_input();
        app.state.mode = Mode::Terminal;
        let screen = ratatui::layout::Rect::new(0, 0, 80, 24);
        crate::ui::compute_view(&mut app.state, screen);
        let expanded = app.state.view.terminal_area;
        let config = app.state.quick_keys.config.clone();
        app.state.quick_keys.offset = 11;
        app.state.quick_keys.modifiers = KeyModifiers::SHIFT;
        app.state.quick_keys.locked = KeyModifiers::SHIFT;
        let event = |kind, row| MouseEvent {
            kind,
            column: 78,
            row,
            modifiers: KeyModifiers::empty(),
        };
        app.quick_keys_mouse(event(MouseEventKind::Down(MouseButton::Left), 22));
        assert!(!app.state.quick_keys.collapsed);
        app.quick_keys_mouse(event(MouseEventKind::Up(MouseButton::Left), 22));
        assert!(app.state.quick_keys.collapsed);
        assert!(app.state.quick_keys.modifiers.is_empty());
        assert!(app.state.quick_keys.locked.is_empty());
        assert!(app.state.quick_keys.press.is_none());
        assert!(receiver.try_recv().is_err());
        crate::ui::compute_view(&mut app.state, screen);
        assert_eq!(app.state.view.terminal_area.height, expanded.height + 2);
        app.quick_keys_mouse(event(MouseEventKind::ScrollDown, 23));
        assert_eq!(app.state.quick_keys.offset, 11);
        app.quick_keys_mouse(event(MouseEventKind::Down(MouseButton::Left), 23));
        app.quick_keys_mouse(event(MouseEventKind::Up(MouseButton::Left), 23));
        assert!(!app.state.quick_keys.collapsed);
        crate::ui::compute_view(&mut app.state, screen);
        assert_eq!(app.state.view.terminal_area, expanded);
        assert_eq!(app.state.quick_keys.config, config);
        assert_eq!(app.state.quick_keys.offset, 11);
        assert!(receiver.try_recv().is_err());
    }
}
