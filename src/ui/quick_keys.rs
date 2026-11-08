use crate::{
    app::AppState,
    quick_keys::{modifier, ItemKind},
};
use ratatui::{
    layout::{Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use super::widgets::panel_contrast_fg;

const KEY_PADDING: usize = 2;
const KEY_GAP: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockTarget {
    Item(usize),
    Left,
    Right,
    Edit,
    Toggle,
}

pub(crate) fn content_width(app: &AppState) -> u16 {
    app.quick_keys
        .config
        .items
        .iter()
        .map(|item| key_width(&item.label).saturating_add(KEY_GAP))
        .fold(0u16, u16::saturating_add)
}

fn key_width(label: &str) -> u16 {
    (label.width() as u16).saturating_add(2 * KEY_PADDING as u16)
}

fn control_width(rect: Rect) -> u16 {
    if rect.width >= 24 {
        5
    } else {
        rect.width.min(3)
    }
}

struct ControlRects {
    left: Rect,
    right: Rect,
    edit: Rect,
    toggle: Rect,
}

fn control_rects(rect: Rect) -> ControlRects {
    let toggle_w = rect.width.min(3);
    let toggle = Rect::new(rect.right() - toggle_w, rect.y, toggle_w, rect.height);
    let available = rect.width - toggle_w;
    let gap = u16::from(available > 0);
    let edit_w = control_width(rect).min(available.saturating_sub(gap));
    let edit = Rect::new(toggle.x - gap - edit_w, rect.y, edit_w, rect.height);
    let scroll_w = if edit.x - rect.x > control_width(rect) * 2 {
        control_width(rect)
    } else {
        0
    };
    ControlRects {
        left: Rect::new(edit.x - scroll_w * 2, rect.y, scroll_w, rect.height),
        right: Rect::new(edit.x - scroll_w, rect.y, scroll_w, rect.height),
        edit,
        toggle,
    }
}

fn restore_rect(rect: Rect) -> Rect {
    let width = rect.width.min(12);
    Rect::new(rect.right() - width, rect.y, width, rect.height)
}

pub(crate) fn viewport(rect: Rect) -> Rect {
    let controls = control_rects(rect);
    Rect::new(rect.x, rect.y, controls.left.x - rect.x, rect.height)
}

pub(crate) fn target(app: &AppState, col: u16, row: u16) -> Option<DockTarget> {
    let rect = app.quick_keys.rect;
    if !rect.contains(Position::new(col, row)) {
        return None;
    }
    let position = Position::new(col, row);
    if app.quick_keys.collapsed {
        return restore_rect(rect)
            .contains(position)
            .then_some(DockTarget::Toggle);
    }
    let controls = control_rects(rect);
    for (target, area) in [
        (DockTarget::Toggle, controls.toggle),
        (DockTarget::Edit, controls.edit),
        (DockTarget::Left, controls.left),
        (DockTarget::Right, controls.right),
    ] {
        if area.contains(position) {
            return Some(target);
        }
    }
    let view = viewport(rect);
    if !view.contains(position) {
        return None;
    }
    let pos = col - view.x + app.quick_keys.offset;
    let mut x = 0u16;
    for (idx, item) in app.quick_keys.config.items.iter().enumerate() {
        let width = key_width(&item.label);
        if pos >= x && pos < x.saturating_add(width) {
            return Some(DockTarget::Item(idx));
        }
        x = x.saturating_add(width).saturating_add(KEY_GAP);
    }
    None
}

fn button_spans(
    label: &str,
    width: u16,
    row: u16,
    height: u16,
    style: Style,
    border: Color,
) -> Vec<Span<'static>> {
    let outline = Style::default().fg(border).bg(Color::Reset);
    if height >= 3 && width >= 2 {
        if row == 0 || row + 1 == height {
            let (left, right) = if row == 0 {
                ('╭', '╮')
            } else {
                ('╰', '╯')
            };
            return vec![Span::styled(
                format!("{left}{}{right}", "─".repeat(usize::from(width - 2))),
                outline,
            )];
        }
        let inner = usize::from(width - 2);
        let padding = inner.saturating_sub(label.width());
        return vec![
            Span::styled("│", outline),
            Span::styled(
                format!(
                    "{}{}{}",
                    " ".repeat(padding / 2),
                    label,
                    " ".repeat(padding - padding / 2)
                ),
                style,
            ),
            Span::styled("│", outline),
        ];
    }
    let padding = usize::from(width).saturating_sub(label.width());
    vec![Span::styled(
        format!(
            "{}{}{}",
            " ".repeat(padding / 2),
            label,
            " ".repeat(padding - padding / 2)
        ),
        style,
    )]
}

fn render_button(frame: &mut Frame, rect: Rect, label: &str, style: Style, border: Color) {
    if rect.is_empty() {
        return;
    }
    frame.render_widget(
        Paragraph::new(
            (0..rect.height)
                .map(|row| {
                    Line::from(button_spans(
                        label,
                        rect.width,
                        row,
                        rect.height,
                        style,
                        border,
                    ))
                })
                .collect::<Vec<_>>(),
        ),
        rect,
    );
}

pub(crate) fn render(app: &AppState, frame: &mut Frame) {
    let rect = app.quick_keys.rect;
    if rect.is_empty() {
        return;
    }
    let p = &app.palette;
    // The main canvas uses the terminal's default background, not the modal surface.
    let base = Style::default().fg(p.text).bg(Color::Reset);
    frame.render_widget(
        Paragraph::new(" ".repeat(rect.width as usize)).style(base),
        rect,
    );
    let pressed = app.quick_keys.press.and_then(|(col, _, dragged)| {
        if dragged {
            None
        } else {
            target(app, col, rect.y)
        }
    });
    let toggle_style = if pressed == Some(DockTarget::Toggle) {
        base.bg(p.surface1)
    } else {
        base.fg(p.accent)
    };
    if app.quick_keys.collapsed {
        render_button(
            frame,
            restore_rect(rect),
            "⌄ 快捷键",
            toggle_style,
            p.surface1,
        );
        return;
    }
    let keys: Vec<_> = app
        .quick_keys
        .config
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let active = item.kind == ItemKind::Modifier
                && modifier(&item.payload).is_some_and(|m| app.quick_keys.modifiers.contains(m));
            let locked = item.kind == ItemKind::Modifier
                && modifier(&item.payload).is_some_and(|m| app.quick_keys.locked.contains(m));
            let activated = app
                .quick_keys
                .activated
                .is_some_and(|(idx, _)| idx == index);
            let highlighted = pressed == Some(DockTarget::Item(index)) || activated;
            let style = if active {
                base.fg(panel_contrast_fg(p))
                    .bg(p.accent)
                    .add_modifier(if locked {
                        Modifier::BOLD | Modifier::UNDERLINED
                    } else {
                        Modifier::BOLD
                    })
            } else if highlighted {
                base.fg(p.text).bg(p.surface1)
            } else if item.kind == ItemKind::Text && item.payload == "继续" {
                base.fg(p.accent)
            } else {
                base
            };
            let border = if active || highlighted {
                p.accent
            } else {
                p.surface1
            };
            (item, style, border)
        })
        .collect();
    let gap = " ".repeat(usize::from(KEY_GAP));
    let lines: Vec<_> = (0..rect.height)
        .map(|row| {
            Line::from(
                keys.iter()
                    .flat_map(|(item, style, border)| {
                        let mut spans = button_spans(
                            &item.label,
                            key_width(&item.label),
                            row,
                            rect.height,
                            *style,
                            *border,
                        );
                        spans.push(Span::styled(gap.as_str(), base));
                        spans
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let view = viewport(rect);
    frame.render_widget(
        Paragraph::new(lines).scroll((0, app.quick_keys.offset)),
        view,
    );
    let controls = control_rects(rect);
    let label_y = rect.y + rect.height / 2;
    for (area, label, enabled) in [
        (controls.left, "‹", app.quick_keys.offset > 0),
        (
            controls.right,
            "›",
            content_width(app) > app.quick_keys.offset + view.width,
        ),
    ] {
        if area.is_empty() {
            continue;
        }
        frame.render_widget(
            Paragraph::new(format!(
                "{:^width$}",
                if enabled { label } else { "" },
                width = usize::from(area.width)
            ))
            .style(base.fg(p.overlay0)),
            Rect::new(area.x, label_y, area.width, 1),
        );
    }
    render_button(frame, controls.edit, "⋯", base.fg(p.accent), p.surface1);
    render_button(
        frame,
        controls.toggle,
        "⌃",
        toggle_style,
        if pressed == Some(DockTarget::Toggle) {
            p.accent
        } else {
            p.surface1
        },
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorAction {
    Select(usize),
    Previous,
    Next,
    MoveUp,
    MoveDown,
    Add,
    Delete,
    Field(usize),
    Save,
    Cancel,
    Reset,
}

pub(crate) fn editor_rect(screen: Rect) -> Rect {
    let width = screen.width.min(68);
    let height = screen.height.min(22);
    Rect::new(
        screen.x + (screen.width - width) / 2,
        screen.y + (screen.height - height) / 2,
        width,
        height,
    )
}

pub(crate) fn editor_hits(app: &AppState, screen: Rect) -> Vec<(Rect, EditorAction)> {
    let Some(editor) = &app.quick_keys.editor else {
        return Vec::new();
    };
    let rect = editor_rect(screen);
    if rect.width < 10 || rect.height < 15 {
        return vec![(rect, EditorAction::Cancel)];
    }
    let x = rect.x + 1;
    let width = rect.width.saturating_sub(2);
    let mut hits = Vec::new();
    let start = editor.selected.saturating_sub(4);
    for (row, idx) in (start..editor.draft.items.len()).take(5).enumerate() {
        hits.push((
            Rect::new(x, rect.y + 2 + row as u16, width, 1),
            EditorAction::Select(idx),
        ));
    }
    let controls = [
        EditorAction::MoveUp,
        EditorAction::MoveDown,
        EditorAction::Add,
        EditorAction::Delete,
    ];
    for (i, action) in controls.into_iter().enumerate() {
        hits.push((
            Rect::new(x + (width * i as u16 / 4), rect.y + 7, width / 4, 1),
            action,
        ));
    }
    hits.push((
        Rect::new(x, rect.y + 8, width / 2, 1),
        EditorAction::Previous,
    ));
    hits.push((
        Rect::new(x + width / 2, rect.y + 8, width - width / 2, 1),
        EditorAction::Next,
    ));
    for field in 0..4 {
        hits.push((
            Rect::new(x, rect.y + 9 + field as u16, width, 1),
            EditorAction::Field(field),
        ));
    }
    let y = rect.bottom().saturating_sub(2);
    for (i, action) in [
        EditorAction::Save,
        EditorAction::Cancel,
        EditorAction::Reset,
    ]
    .into_iter()
    .enumerate()
    {
        hits.push((Rect::new(x + width * i as u16 / 3, y, width / 3, 1), action));
    }
    hits
}

pub(crate) fn render_editor(app: &AppState, frame: &mut Frame) {
    let Some(editor) = &app.quick_keys.editor else {
        return;
    };
    let rect = editor_rect(frame.area());
    let p = &app.palette;
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Block::default()
            .title(" 快捷栏 · 编辑 ")
            .borders(Borders::ALL)
            .style(Style::default().bg(p.panel_bg).fg(p.text))
            .border_style(Style::default().fg(p.accent)),
        rect,
    );
    if rect.width < 10 || rect.height < 15 {
        frame.render_widget(
            Paragraph::new("请增大窗口；Esc 返回")
                .style(Style::default().fg(p.text).bg(p.panel_bg)),
            Rect::new(
                rect.x + 1,
                rect.y + 1,
                rect.width.saturating_sub(2),
                rect.height.saturating_sub(2),
            ),
        );
        return;
    }
    for (area, action) in editor_hits(app, frame.area()) {
        let mut selected = false;
        let text = match action {
            EditorAction::Select(idx) => {
                selected = idx == editor.selected;
                format!(
                    " {}  {}",
                    if selected { "›" } else { " " },
                    editor.draft.items[idx].label
                )
            }
            EditorAction::Previous => " ‹上一项 ".into(),
            EditorAction::Next => " 下一项› ".into(),
            EditorAction::MoveUp => " ↑前移 ".into(),
            EditorAction::MoveDown => " ↓后移 ".into(),
            EditorAction::Add => " +新增 ".into(),
            EditorAction::Delete => " 删除 ".into(),
            EditorAction::Save => " 保存 ".into(),
            EditorAction::Cancel => " 取消 ".into(),
            EditorAction::Reset => " 恢复默认 ".into(),
            EditorAction::Field(field) => {
                selected = editor.field == field;
                if let Some(item) = editor.item() {
                    match field {
                        0 => format!("名称  {}", item.label),
                        1 => format!("类型  {}  ‹ ›", item.kind.label()),
                        2 => format!("内容  {}", item.payload.replace('\n', "↵")),
                        _ => format!("[{}] 文本追加回车", if item.auto_enter { "x" } else { " " }),
                    }
                } else {
                    "请点 +新增 添加快捷键".into()
                }
            }
        };
        let style = Style::default()
            .bg(if selected { p.surface0 } else { p.panel_bg })
            .fg(if selected { p.accent } else { p.text });
        let line = Paragraph::new(text.clone()).style(style);
        // Keep the insertion end of long UTF-8 fields visible without byte slicing.
        let scroll = if matches!(action, EditorAction::Field(0 | 2)) {
            text.width()
                .saturating_sub(area.width.saturating_sub(1) as usize) as u16
        } else {
            0
        };
        frame.render_widget(line.scroll((0, scroll)), area);
        if selected && matches!(action, EditorAction::Field(0 | 2)) && area.width > 0 {
            let caret = text.width().saturating_sub(scroll as usize) as u16;
            frame.set_cursor_position((area.x + caret.min(area.width - 1), area.y));
        }
    }
    let hint = if editor.error.is_empty() {
        editor.item().map_or("", |item| item.kind.hint())
    } else {
        &editor.error
    };
    let y = (rect.y + 14).min(rect.bottom().saturating_sub(3));
    if y > rect.y + 12 {
        frame.render_widget(
            Paragraph::new(hint).style(Style::default().bg(p.panel_bg).fg(
                if editor.error.is_empty() {
                    p.overlay0
                } else {
                    p.red
                },
            )),
            Rect::new(rect.x + 1, y, rect.width.saturating_sub(2), 1),
        );
    }
    if rect.height > 18 {
        frame.render_widget(
            Paragraph::new("Tab 切换字段 · ↑↓选择 · Ctrl S 保存 · Esc 取消")
                .style(Style::default().fg(p.overlay0).bg(p.panel_bg)),
            Rect::new(
                rect.x + 1,
                rect.bottom() - 4,
                rect.width.saturating_sub(2),
                1,
            ),
        );
    }
}

pub(crate) fn render_notice(app: &AppState, frame: &mut Frame) {
    if let Some((message, at)) = &app.quick_keys.notice {
        if at.elapsed().as_secs() < 5 {
            let rect = app.quick_keys.rect;
            if rect.y > 0 {
                frame.render_widget(Clear, Rect::new(rect.x, rect.y - 1, rect.width, 1));
                frame.render_widget(
                    Paragraph::new(message.as_str()).style(
                        Style::default()
                            .fg(app.palette.accent)
                            .bg(app.palette.panel_bg),
                    ),
                    Rect::new(rect.x, rect.y - 1, rect.width, 1),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quick_keys_render_and_editor_fit_mobile_and_tiny_screens() {
        for width in [1, 8, 20, 38, 80, 120] {
            let mut app = AppState::test_new();
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, 24)).unwrap();
            super::super::compute_view(&mut app, Rect::new(0, 0, width, 24));
            assert_eq!(app.quick_keys.rect.height, 3);
            assert_eq!(app.quick_keys.rect.bottom(), 24);
            assert!(app.view.terminal_area.bottom() <= app.quick_keys.rect.y);
            terminal
                .draw(|frame| super::super::render(&app, frame))
                .unwrap();
            let controls = control_rects(app.quick_keys.rect);
            assert_eq!(
                terminal.backend().buffer()[(controls.toggle.x + controls.toggle.width / 2, 22)]
                    .symbol(),
                "⌃"
            );
            if !controls.edit.is_empty() {
                assert_eq!(
                    terminal.backend().buffer()[(controls.edit.x + controls.edit.width / 2, 22)]
                        .symbol(),
                    "⋯",
                    "the mobile switcher must leave the dock visible"
                );
            }
            app.quick_keys.open_editor();
            terminal
                .draw(|frame| super::super::render(&app, frame))
                .unwrap();
            if width >= 10 {
                assert_eq!(
                    terminal.get_cursor_position().unwrap().y,
                    editor_rect(Rect::new(0, 0, width, 24)).y + 9
                );
            }
        }
    }
    #[test]
    fn quick_keys_narrow_layout_keeps_one_key_row_with_clickable_padding() {
        let mut app = AppState::test_new();
        app.quick_keys.rect = Rect::new(26, 17, 32, 3);
        assert_eq!(viewport(app.quick_keys.rect).x, 26);
        for row in 17..20 {
            assert_eq!(target(&app, 25, row), None, "sidebar is outside the dock");
            assert_eq!(target(&app, 26, row), Some(DockTarget::Item(0)));
            assert_eq!(target(&app, 32, row), Some(DockTarget::Item(0)));
            assert_eq!(target(&app, 33, row), None, "gap must not activate a key");
            assert_eq!(target(&app, 43, row), Some(DockTarget::Left));
            assert_eq!(target(&app, 48, row), Some(DockTarget::Right));
            assert_eq!(target(&app, 53, row), Some(DockTarget::Edit));
            assert_eq!(target(&app, 54, row), None);
            assert_eq!(target(&app, 57, row), Some(DockTarget::Toggle));
        }
        app.quick_keys.offset = 9;
        assert_eq!(target(&app, 27, 18), Some(DockTarget::Item(1)));
        assert_eq!(target(&app, 27, 16), None);
        assert_eq!(target(&app, 27, 20), None);
        for height in [0, 1, 4, 5, 13, 14, 24, 60] {
            super::super::compute_view(&mut app, Rect::new(0, 0, 32, height));
            assert_eq!(
                app.quick_keys.rect.height,
                if height >= 10 {
                    3
                } else {
                    u16::from(height >= 5)
                }
            );
            assert!(app.view.terminal_area.bottom() <= app.quick_keys.rect.y);
        }
    }

    #[test]
    fn rounded_keys_start_at_terminal_column_and_frame_labels_symmetrically() {
        let mut app = AppState::test_new();
        app.mode = crate::app::Mode::Terminal;
        super::super::compute_view(&mut app, Rect::new(0, 0, 80, 24));
        let dock = app.quick_keys.rect;
        assert_eq!(dock.x, app.view.sidebar_rect.right());
        assert_eq!(dock.x, app.view.terminal_area.x);
        assert_eq!(app.view.sidebar_rect.bottom(), 24);
        assert_eq!(dock, Rect::new(26, 21, 54, 3));
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| super::super::render(&app, frame))
            .unwrap();
        let buffer = terminal.backend().buffer();
        for (row, expected) in [(21, "╭─────╮"), (22, "│ Esc │"), (23, "╰─────╯")]
        {
            let text: String = (26..33).map(|col| buffer[(col, row)].symbol()).collect();
            assert_eq!(text, expected);
        }
        assert_eq!(target(&app, 25, 22), None);
    }

    #[test]
    fn quick_keys_collapse_reclaims_height_and_keeps_a_restore_entry() {
        for width in [32, 80, 120] {
            let mut app = AppState::test_new();
            app.mode = crate::app::Mode::Terminal;
            let screen = Rect::new(0, 0, width, 24);
            super::super::compute_view(&mut app, screen);
            let expanded = app.view.terminal_area;
            let sidebar = app.view.sidebar_rect;
            app.quick_keys.offset = 11;
            app.quick_keys.toggle_collapsed();
            super::super::compute_view(&mut app, screen);
            assert_eq!(app.quick_keys.rect.height, 1);
            assert_eq!(app.view.terminal_area.height, expanded.height + 2);
            assert_eq!(app.view.sidebar_rect, sidebar);
            assert_eq!(app.quick_keys.offset, 11);
            let entry = restore_rect(app.quick_keys.rect);
            assert_eq!(target(&app, entry.x, entry.y), Some(DockTarget::Toggle));
            assert_eq!(target(&app, app.quick_keys.rect.x, entry.y), None);
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, 24)).unwrap();
            terminal
                .draw(|frame| super::super::render(&app, frame))
                .unwrap();
            assert_eq!(
                terminal.backend().buffer()[(entry.x + 2, entry.y)].symbol(),
                "⌄"
            );
            assert_eq!(
                terminal.backend().buffer()[(entry.x + 4, entry.y)].symbol(),
                "快"
            );
            assert_eq!(
                terminal.backend().buffer()[(app.quick_keys.rect.x, entry.y)].symbol(),
                " "
            );
            app.quick_keys.toggle_collapsed();
            super::super::compute_view(&mut app, screen);
            assert_eq!(app.view.terminal_area, expanded);
            assert_eq!(app.quick_keys.offset, 11);
        }
    }
}
