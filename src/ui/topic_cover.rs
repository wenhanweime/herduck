use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Clear, Paragraph, Wrap},
    Frame,
};
use unicode_width::UnicodeWidthChar;

use super::projects::ProjectTreeRow;
use super::widgets::{
    action_button_row_rects, centered_popup_rect, panel_contrast_fg, render_action_button,
    render_modal_header, render_modal_shell, ActionButtonSpec,
};
use crate::app::state::{AppState, ProjectFilter, ProjectRowHitArea, TopicDetailGeometry};
use crate::projects::overview::{ProjectOverview, WorkPhase};
use crate::projects::{ProjectSummary, TopicCover};

const EDITOR_WIDTH: u16 = 76;
const EDITOR_HEIGHT: u16 = 23;
const FIELD_LABELS: [&str; 5] = [
    "This week's goal",
    "Next step 1",
    "Next step 2",
    "Next step 3",
    "What's blocked",
];

const FIELD_LABELS_ZH: [&str; 5] = ["本周目标", "下一步 1", "下一步 2", "下一步 3", "当前阻塞"];

fn selected_topic(app: &AppState) -> Option<&ProjectSummary> {
    app.selected_project_summary()
}

pub(crate) fn topic_detail_rows(app: &AppState) -> Vec<ProjectTreeRow> {
    let Some(topic) = selected_topic(app) else {
        return Vec::new();
    };
    let overview = app.visible_project_overview();
    let mut sessions: Vec<_> = topic
        .sessions
        .iter()
        .filter(|session| app.projects.filter != ProjectFilter::Open || session.live)
        .collect();
    sessions.sort_by_key(|session| {
        overview
            .as_ref()
            .and_then(|overview| {
                overview
                    .work
                    .iter()
                    .find(|item| item.session_key == session.stable_key)
            })
            .map(|item| item.phase)
            .unwrap_or_else(|| {
                app.project_session_phase(session)
                    .unwrap_or(if session.live {
                        WorkPhase::Open
                    } else {
                        WorkPhase::History
                    })
            })
            .attention_order()
    });
    let mut rows: Vec<_> = sessions
        .into_iter()
        .cloned()
        .map(ProjectTreeRow::Session)
        .collect();
    if topic.next_cursor.is_some() && app.projects.filter == ProjectFilter::All {
        rows.push(ProjectTreeRow::LoadOlder {
            project_key: topic.canonical_key.clone(),
        });
    }
    rows
}

fn row_height(row: &ProjectTreeRow, overview: Option<&ProjectOverview>, width: u16) -> u16 {
    match row {
        ProjectTreeRow::Session(session) => overview
            .and_then(|overview| {
                super::project_overview::session_height(overview, &session.stable_key, width)
            })
            .unwrap_or(2),
        _ => 1,
    }
}

fn scroll_to_show_row(
    rows: &[ProjectTreeRow],
    overview: Option<&ProjectOverview>,
    index: usize,
    area: Rect,
) -> usize {
    let index = index.min(rows.len().saturating_sub(1));
    let Some(row) = rows.get(index) else {
        return 0;
    };
    let mut height = row_height(row, overview, area.width);
    let mut start = index;
    while start > 0 {
        let next = height.saturating_add(1).saturating_add(row_height(
            &rows[start - 1],
            overview,
            area.width,
        ));
        if next > area.height {
            break;
        }
        height = next;
        start -= 1;
    }
    start
}

pub(crate) fn topic_detail_selection_scroll(app: &AppState) -> usize {
    let rows = topic_detail_rows(app);
    let selected = app
        .projects
        .topic_detail_selected
        .min(rows.len().saturating_sub(1));
    let overview = app.visible_project_overview();
    let minimum = scroll_to_show_row(
        &rows,
        overview.as_ref(),
        selected,
        app.view.topic_detail.sessions,
    );
    app.projects.topic_detail_scroll.clamp(minimum, selected)
}

fn context_lines(app: &AppState, cover: &TopicCover) -> Vec<String> {
    [
        (app.title_language.text("Goal: ", "目标："), &cover.goal),
        (
            app.title_language.text("Saved blocker: ", "计划中的阻塞："),
            &cover.blocked_note,
        ),
    ]
    .into_iter()
    .filter(|(_, value)| !value.trim().is_empty())
    .map(|(label, value)| format!("{label}{}", single_line(value)))
    .collect()
}

fn context_height(text: &str, width: u16) -> u16 {
    Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .line_count(width)
        .min(2) as u16
}

pub(crate) fn topic_detail_geometry(app: &AppState, area: Rect) -> TopicDetailGeometry {
    if area.width < 4 || area.height < 4 {
        return TopicDetailGeometry::default();
    }
    let inner = Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2);
    let is_topic = selected_topic(app)
        .is_some_and(|topic| topic.kind == crate::projects::ProjectKind::Semantic);
    let toolbar_on_next_line = inner.width < 64 && inner.height >= 8;
    let header_height = 1 + u16::from(toolbar_on_next_line);
    let controls_y = inner.y + u16::from(toolbar_on_next_line);
    let edit_width = if is_topic { inner.width.min(15) } else { 0 };
    let edit = Rect::new(inner.right() - edit_width, controls_y, edit_width, 1);
    let refresh_width = inner.width.saturating_sub(edit_width).min(12);
    let refresh = Rect::new(
        edit.x.saturating_sub(refresh_width),
        controls_y,
        refresh_width,
        1,
    );
    let title = Rect::new(
        inner.x,
        inner.y,
        if toolbar_on_next_line {
            inner.width
        } else {
            inner.width.saturating_sub(edit_width + refresh_width + 1)
        },
        1,
    );
    let context_y = inner.y + header_height + u16::from(inner.height >= 10);
    let cover_height = selected_topic(app)
        .and_then(|topic| topic.cover.as_ref())
        .map(|cover| {
            context_lines(app, cover)
                .iter()
                .map(|text| context_height(text, inner.width))
                .sum::<u16>()
        })
        .unwrap_or(0)
        .min(inner.bottom().saturating_sub(context_y + 4));
    let cover = Rect::new(inner.x, context_y, inner.width, cover_height);
    let footer = Rect::new(inner.x, inner.bottom() - 1, inner.width, 1);
    let heading = Rect::new(
        inner.x,
        cover.bottom(),
        inner.width,
        u16::from(footer.y.saturating_sub(cover.bottom()) >= 3),
    );
    let sessions_y = heading.bottom() + u16::from(footer.y.saturating_sub(heading.bottom()) >= 5);
    let sessions = Rect::new(
        inner.x,
        sessions_y,
        inner.width,
        footer.y.saturating_sub(sessions_y),
    );
    let rows = topic_detail_rows(app);
    let overview = app.visible_project_overview();
    let max_scroll = scroll_to_show_row(
        &rows,
        overview.as_ref(),
        rows.len().saturating_sub(1),
        sessions,
    );
    let normalized_scroll = app.projects.topic_detail_scroll.min(max_scroll);
    let mut row_hits = Vec::new();
    let mut overview_hits = Vec::new();
    let mut y = sessions.y;
    for (row_index, row) in rows.iter().enumerate().skip(normalized_scroll) {
        let remaining = sessions.bottom().saturating_sub(y);
        if remaining == 0
            || (remaining < row_height(row, overview.as_ref(), sessions.width).min(3)
                && row_index != normalized_scroll)
        {
            break;
        }
        let height = row_height(row, overview.as_ref(), sessions.width).min(remaining);
        let rect = Rect::new(sessions.x, y, sessions.width, height);
        if let Some(action) = row.action() {
            row_hits.push(ProjectRowHitArea {
                rect,
                action,
                row_index,
            });
        }
        if let (Some(overview), ProjectTreeRow::Session(session)) = (&overview, row) {
            overview_hits.extend(super::project_overview::session_hit_areas(
                overview,
                &session.stable_key,
                rect,
            ));
        }
        y = rect.bottom().saturating_add(1);
    }
    TopicDetailGeometry {
        title,
        edit,
        cover,
        refresh,
        overview_hits,
        heading,
        sessions,
        footer,
        row_hits,
        normalized_scroll,
    }
}

pub(super) fn render_topic_detail(app: &AppState, frame: &mut Frame, area: Rect) {
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new("").style(Style::default().bg(app.palette.panel_bg)),
        area,
    );
    let geometry = topic_detail_geometry(app, area);
    let Some(topic) = selected_topic(app) else {
        frame.render_widget(
            Paragraph::new(app.title_language.text(
                "Project is no longer available. Esc returns to the list.",
                "项目已不可用，按 Esc 返回列表。",
            ))
            .style(Style::default().fg(app.palette.subtext0))
            .wrap(Wrap { trim: false }),
            area,
        );
        return;
    };
    frame.render_widget(
        Paragraph::new(format!(
            "{} · {}",
            if topic.kind == crate::projects::ProjectKind::Semantic {
                app.title_language.text("Work", "工作")
            } else {
                app.title_language.text("Project", "项目")
            },
            topic.display_name
        ))
        .style(
            Style::default()
                .fg(app.palette.text)
                .add_modifier(Modifier::BOLD),
        ),
        geometry.title,
    );
    render_action_button(
        frame,
        geometry.edit,
        Some("e"),
        app.title_language.text("edit plan", "编辑计划"),
        Style::default()
            .fg(panel_contrast_fg(&app.palette))
            .bg(app.palette.accent)
            .add_modifier(Modifier::BOLD),
    );
    render_action_button(
        frame,
        geometry.refresh,
        Some("r"),
        app.title_language.text("refresh", "刷新"),
        Style::default().fg(app.palette.subtext0),
    );
    if let Some(cover) = &topic.cover {
        let lines = context_lines(app, cover);
        let mut y = geometry.cover.y;
        for (index, text) in lines.iter().enumerate() {
            let remaining = geometry.cover.bottom().saturating_sub(y);
            let height = context_height(text, geometry.cover.width)
                .min(remaining.saturating_sub((lines.len() - index - 1) as u16));
            super::project_overview::render_prose(
                frame,
                Rect::new(geometry.cover.x, y, geometry.cover.width, height),
                text,
                Style::default().fg(app.palette.subtext0),
            );
            y += height;
        }
    }
    let overview = app.visible_project_overview();
    if let Some(overview) = &overview {
        frame.render_widget(
            Paragraph::new(super::project_overview::scope(overview))
                .style(Style::default().fg(app.palette.subtext0)),
            geometry.heading,
        );
    }
    let rows = topic_detail_rows(app);
    if rows.is_empty() {
        let text = if app.title_language == crate::config::TitleLanguage::Chinese {
            if app.projects.filter == ProjectFilter::Open {
                "当前没有打开的会话。".to_string()
            } else {
                "这里还没有会话。".to_string()
            }
        } else {
            format!(
                "No {}conversations in this {}{}.",
                if app.projects.filter == ProjectFilter::Open {
                    "open "
                } else {
                    ""
                },
                if topic.kind == crate::projects::ProjectKind::Semantic {
                    "Work"
                } else {
                    "Project"
                },
                if app.projects.filter == ProjectFilter::Open {
                    ""
                } else {
                    " yet"
                }
            )
        };
        frame.render_widget(
            Paragraph::new(text).style(Style::default().fg(app.palette.overlay0)),
            geometry.sessions,
        );
    }
    for hit in &geometry.row_hits {
        let Some(row) = rows.get(hit.row_index) else {
            continue;
        };
        let selected = hit.row_index == app.projects.topic_detail_selected;
        let style = Style::default().fg(app.palette.subtext0).bg(if selected {
            app.palette.surface0
        } else {
            app.palette.panel_bg
        });
        match row {
            ProjectTreeRow::Session(session) => {
                if let Some((overview, item)) = overview.as_ref().and_then(|overview| {
                    overview
                        .work
                        .iter()
                        .find(|item| item.session_key == session.stable_key)
                        .map(|item| (overview, item))
                }) {
                    super::project_overview::render_session_overview(
                        app, frame, hit.rect, overview, item, selected,
                    );
                } else {
                    // Paged history outside the evidence window remains reachable and read-only.
                    frame.render_widget(
                        Paragraph::new(vec![
                            Line::from(format!(
                                "{} {} · {}",
                                if selected { "▸" } else { " " },
                                session.title,
                                session.backend
                            )),
                            super::projects::session_status_line(app, session),
                        ])
                        .style(style),
                        hit.rect,
                    );
                }
            }
            ProjectTreeRow::LoadOlder { .. } => frame.render_widget(
                Paragraph::new(
                    app.title_language
                        .text("Load older conversations…", "加载更早的会话…"),
                )
                .style(style),
                hit.rect,
            ),
            _ => {}
        }
    }
    let shown = match (geometry.row_hits.first(), geometry.row_hits.last()) {
        (Some(first), Some(last)) => format!(
            "{}–{} / {}{}  ",
            first.row_index + 1,
            last.row_index + 1,
            rows.len(),
            if last.row_index + 1 < rows.len() {
                " ↓"
            } else {
                ""
            }
        ),
        _ => String::new(),
    };
    frame.render_widget(
        Paragraph::new(format!(
            "{shown}{}",
            app.title_language.text(
                "↑↓ scroll · Enter view · Esc back",
                "↑↓ 浏览 · Enter 查看 · Esc 返回"
            )
        ))
        .style(Style::default().fg(app.palette.overlay0)),
        geometry.footer,
    );
}

fn single_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) struct TopicCoverEditorGeometry {
    pub inner: Rect,
    pub fields: Vec<(usize, Rect)>,
    pub error: Rect,
    pub save: Rect,
    pub cancel: Rect,
}

pub(crate) fn topic_cover_editor_geometry(
    app: &AppState,
    area: Rect,
) -> Option<TopicCoverEditorGeometry> {
    let editor = app.projects.cover_editor.as_ref()?;
    let popup = centered_popup_rect(area, EDITOR_WIDTH, EDITOR_HEIGHT)?;
    let inner = Rect::new(popup.x + 1, popup.y + 1, popup.width - 2, popup.height - 2);
    if inner.height < 7 || inner.width < 12 {
        return None;
    }
    let visible = (usize::from(inner.height.saturating_sub(4)) / 3).clamp(1, 5);
    let first = editor
        .focused_field
        .saturating_sub(visible - 1)
        .min(5 - visible);
    let fields = (first..first + visible)
        .enumerate()
        .map(|(offset, index)| {
            (
                index,
                Rect::new(inner.x, inner.y + 2 + offset as u16 * 3, inner.width, 3),
            )
        })
        .collect();
    let error = Rect::new(inner.x, inner.bottom() - 2, inner.width, 1);
    let buttons = action_button_row_rects(
        inner,
        &[
            ActionButtonSpec {
                hint: Some("↵"),
                label: app.title_language.text("save", "保存"),
            },
            ActionButtonSpec {
                hint: Some("esc"),
                label: app.title_language.text("cancel", "取消"),
            },
        ],
        2,
        inner.height - 1,
    );
    Some(TopicCoverEditorGeometry {
        inner,
        fields,
        error,
        save: buttons[0],
        cancel: buttons[1],
    })
}

pub(super) fn render_topic_cover_editor(app: &AppState, frame: &mut Frame, area: Rect) {
    super::dim_background(frame, area);
    let Some(editor) = app.projects.cover_editor.as_ref() else {
        return;
    };
    let Some(inner) = render_modal_shell(frame, area, EDITOR_WIDTH, EDITOR_HEIGHT, &app.palette)
    else {
        return;
    };
    let Some(layout) = topic_cover_editor_geometry(app, area) else {
        frame.render_widget(
            Paragraph::new(app.title_language.text(
                "Enlarge the terminal to edit. Esc cancels.",
                "请扩大终端窗口后编辑，按 Esc 取消。",
            ))
            .wrap(Wrap { trim: false }),
            inner,
        );
        return;
    };
    let inner = layout.inner;
    render_modal_header(
        frame,
        Rect::new(inner.x, inner.y, inner.width, 1),
        &format!(
            "{} · {}",
            app.title_language.text("Work plan", "工作计划"),
            editor.topic_name
        ),
        &app.palette,
    );
    frame.render_widget(
        Paragraph::new(app.title_language.text(
            "tab next field · shift-enter new line · ctrl-u clear",
            "Tab 下一项 · Shift-Enter 换行 · Ctrl-U 清空",
        ))
        .style(Style::default().fg(app.palette.overlay0)),
        Rect::new(inner.x, inner.y + 1, inner.width, 1),
    );
    for (index, area) in &layout.fields {
        let focused = *index == editor.focused_field;
        frame.render_widget(
            Paragraph::new(
                app.title_language
                    .text(FIELD_LABELS[*index], FIELD_LABELS_ZH[*index]),
            )
            .style(Style::default().fg(if focused {
                app.palette.accent
            } else {
                app.palette.subtext0
            })),
            Rect::new(area.x, area.y, area.width, 1),
        );
        let input = Rect::new(area.x, area.y + 1, area.width, 2);
        let value = &editor.fields[*index];
        let (text, scroll, column, row) = editor_text_layout(
            value,
            if focused { editor.cursor } else { 0 },
            input.width,
            input.height,
        );
        frame.render_widget(
            Paragraph::new(text).scroll((scroll, 0)).style(
                Style::default().fg(app.palette.text).bg(if focused {
                    app.palette.surface1
                } else {
                    app.palette.surface0
                }),
            ),
            input,
        );
        if focused {
            frame.set_cursor_position((input.x + column, input.y + row));
        }
    }
    frame.render_widget(
        Paragraph::new(
            if editor.error == "Each field can contain up to 2000 characters" {
                app.title_language
                    .text(&editor.error, "每项最多可以填写 2000 个字符")
            } else {
                &editor.error
            },
        )
        .style(Style::default().fg(app.palette.red)),
        layout.error,
    );
    render_action_button(
        frame,
        layout.save,
        Some("↵"),
        app.title_language.text("save", "保存"),
        Style::default()
            .fg(panel_contrast_fg(&app.palette))
            .bg(app.palette.accent)
            .add_modifier(Modifier::BOLD),
    );
    render_action_button(
        frame,
        layout.cancel,
        Some("esc"),
        app.title_language.text("cancel", "取消"),
        Style::default()
            .fg(app.palette.text)
            .bg(app.palette.surface0),
    );
}

/// Explicit character wrapping keeps the IME cursor on the actual input cell, including CJK.
fn editor_text_layout(
    text: &str,
    cursor: usize,
    width: u16,
    height: u16,
) -> (String, u16, u16, u16) {
    let width = usize::from(width.max(1));
    let height = usize::from(height.max(1));
    let mut rendered = String::new();
    let (mut row, mut column) = (0usize, 0usize);
    let mut anchor = (0usize, 0usize);
    for (index, character) in text.char_indices() {
        let size = if character == '\t' {
            4.min(width)
        } else {
            character.width().unwrap_or(0)
        };
        if character != '\n' && column + size > width {
            rendered.push('\n');
            row += 1;
            column = 0;
        }
        if index == cursor {
            anchor = (row, column);
        }
        if character == '\n' {
            rendered.push('\n');
            row += 1;
            column = 0;
        } else {
            if character == '\t' {
                rendered.push_str(&" ".repeat(size));
            } else {
                rendered.push(character);
            }
            column += size;
        }
    }
    if cursor == text.len() {
        anchor = (row, column);
    }
    if anchor.1 >= width {
        anchor = (anchor.0 + 1, 0);
    }
    let scroll = anchor.0.saturating_sub(height - 1);
    (
        rendered,
        scroll.min(u16::MAX as usize) as u16,
        anchor.1.min(width - 1) as u16,
        (anchor.0 - scroll) as u16,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::TopicCoverPatch;

    fn fixture(filled: bool) -> AppState {
        let (mut catalog, key) = crate::projects::cover::test_catalog();
        if filled {
            catalog
                .update_topic_cover(
                    &key,
                    &TopicCoverPatch {
                        goal: Some("Interview five users".into()),
                        next_steps: Some(vec![
                            "Send invitations".into(),
                            "Schedule calls".into(),
                            "Prepare questions".into(),
                        ]),
                        blocked_note: Some("Waiting for replies".into()),
                        ..Default::default()
                    },
                    100,
                )
                .unwrap();
        }
        let mut state = AppState::test_new();
        state.projects.snapshot = catalog.snapshot(50).unwrap();
        state.open_topic_detail(&key);
        state
    }

    fn render_text(state: &AppState, width: u16, height: u16) -> String {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| render_topic_detail(state, frame, frame.area()))
            .unwrap();
        buffer_text(terminal.backend().buffer())
    }

    fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
        (buffer.area.y..buffer.area.bottom())
            .map(|y| {
                let mut line = String::new();
                let mut x = buffer.area.x;
                while x < buffer.area.right() {
                    let symbol = buffer[(x, y)].symbol();
                    line.push_str(symbol);
                    x += (unicode_width::UnicodeWidthStr::width(symbol) as u16).max(1);
                }
                line
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn topic_cover_detail_shows_authored_fields_above_clickable_conversations() {
        let state = fixture(true);
        let text = render_text(&state, 120, 26);
        for expected in [
            "First users",
            "Interview five users",
            "Waiting for replies",
            "Plan first user interviews",
        ] {
            assert!(text.contains(expected), "missing {expected}:\n{text}");
        }
        let geometry = topic_detail_geometry(&state, Rect::new(0, 0, 120, 26));
        assert!(geometry.cover.bottom() <= geometry.heading.y);
        assert!(geometry.heading.bottom() <= geometry.sessions.y);
        assert_eq!(geometry.row_hits.len(), 1);
        assert!(geometry.row_hits[0].rect.y >= geometry.sessions.y);
        assert!(geometry.row_hits[0].rect.bottom() <= geometry.sessions.bottom());
    }

    #[test]
    fn chinese_detail_and_editor_localize_controls_without_rewriting_authored_text() {
        let mut state = fixture(true);
        state.title_language = crate::config::TitleLanguage::Chinese;
        let saved = state.selected_project_summary().unwrap().cover.clone();
        let text = render_text(&state, 124, 32);
        for expected in [
            "工作 ·",
            "编辑计划",
            "刷新",
            "下一步建议",
            "历史会话",
            "查看会话",
        ] {
            assert!(text.contains(expected), "missing {expected}: {text}");
        }
        for unwanted in [
            "edit plan",
            "refresh",
            "What's happening",
            "Suggested follow-ups",
            "Conversations",
            "read-only history",
        ] {
            assert!(!text.contains(unwanted), "mixed UI: {text}");
        }
        state.open_topic_cover_editor();
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 32)).unwrap();
        terminal
            .draw(|frame| render_topic_cover_editor(&state, frame, frame.area()))
            .unwrap();
        let text = buffer_text(terminal.backend().buffer());
        for expected in [
            "工作计划",
            "本周目标",
            "下一步 1",
            "当前阻塞",
            "保存",
            "取消",
            "Interview five users",
            "Send invitations",
            "Schedule calls",
            "Prepare questions",
            "Waiting for replies",
        ] {
            assert!(text.contains(expected), "missing {expected}: {text}");
        }
        assert_eq!(state.selected_project_summary().unwrap().cover, saved);
    }

    #[test]
    fn empty_topic_and_folder_project_open_with_progress_instead_of_a_blank_cover() {
        let mut state = fixture(false);
        let topic_key = state.projects.snapshot.topics[0].canonical_key.clone();
        let folder_key = state.projects.snapshot.projects[0].canonical_key.clone();
        for (key, label, editable) in [
            (&topic_key, "Work ·", true),
            (&folder_key, "Project ·", false),
        ] {
            assert!(state.open_topic_detail(key));
            let text = render_text(&state, 124, 32);
            for expected in [label, "Next:", "Plan first user interviews", "History"] {
                assert!(text.contains(expected), "missing {expected}:\n{text}");
            }
            assert!(!text.contains("Write this week's goal"));
            let geometry = topic_detail_geometry(&state, Rect::new(0, 0, 124, 32));
            assert_eq!(geometry.cover.height, 0);
            assert_eq!(geometry.edit.width > 0, editable);
            assert!(!geometry.overview_hits.is_empty());
            assert!(!geometry.row_hits.is_empty());
        }
    }

    #[test]
    fn topic_cover_is_fixed_while_sessions_scroll_and_empty_cover_leaves_more_room() {
        let mut state = fixture(true);
        let session = state.projects.snapshot.topics[0].sessions[0].clone();
        state.projects.snapshot.topics[0].sessions = (0..20)
            .map(|index| {
                let mut session = session.clone();
                session.stable_key = format!("session-{index}");
                session
            })
            .collect();
        let area = Rect::new(0, 0, 90, 20);
        let before = topic_detail_geometry(&state, area);
        state.projects.topic_detail_scroll = 8;
        let after = topic_detail_geometry(&state, area);
        assert_eq!(before.cover, after.cover);
        assert_eq!(before.sessions, after.sessions);
        assert_eq!(after.row_hits[0].row_index, 8);
        state.projects.snapshot.topics[0].cover = None;
        assert!(topic_detail_geometry(&state, area).sessions.height > after.sessions.height);
        let text = render_text(&state, 90, 20);
        assert!(!text.contains("Write this week's goal"));
        assert!(text.contains("History"));
    }

    #[test]
    fn topic_cover_small_layout_keeps_conversations_and_editor_actions_reachable() {
        let mut state = fixture(true);
        for (width, height) in [(24, 8), (40, 12), (80, 24)] {
            let area = Rect::new(0, 0, width, height);
            let geometry = topic_detail_geometry(&state, area);
            assert!(geometry.sessions.height >= 2, "{width}x{height}");
            assert!(geometry.cover.bottom() <= geometry.sessions.y);
            assert!(geometry.footer.bottom() <= area.bottom());
            assert!(!render_text(&state, width, height).is_empty());
        }
        state.open_topic_cover_editor();
        for field in 0..5 {
            state.projects.cover_editor.as_mut().unwrap().focus(field);
            let layout = topic_cover_editor_geometry(&state, Rect::new(0, 0, 44, 16)).unwrap();
            assert!(layout.fields.iter().any(|(index, _)| *index == field));
            assert!(layout
                .fields
                .iter()
                .all(|(_, area)| area.bottom() <= layout.error.y));
            assert!(layout.save.bottom() <= 16 && layout.save.width > 0);
            assert!(layout.cancel.bottom() <= 16 && layout.cancel.width > 0);
        }
    }

    #[test]
    fn topic_cover_narrow_layout_keeps_blocker_visible_with_three_steps() {
        let state = fixture(true);
        // Context stays above progress; the full authored plan is accessible in its editor.
        let text = render_text(&state, 70, 15);
        for expected in [
            "Goal: Interview five users",
            "Saved blocker: Waiting for replies",
            "Plan first user interviews",
        ] {
            assert!(text.contains(expected), "missing {expected}:\n{text}");
        }
        // One fewer row still retains both context lines and the session.
        let compact = render_text(&state, 70, 14);
        assert!(compact.contains("Goal: Interview five users"));
        assert!(compact.contains("Saved blocker: Waiting for replies"));
        assert!(compact.contains("Plan first user interviews"));
    }

    fn progress_fixture() -> AppState {
        use crate::projects::activity::{ActivityBatch, SessionActivity};
        let mut state = AppState::test_new();
        let mut project = crate::projects::overview::tests::fixture_project();
        project.cover = Some(TopicCover {
            goal: "Learn why new users leave".into(),
            next_steps: vec![
                "Authored step one".into(),
                "Authored step two".into(),
                "Authored step three".into(),
            ],
            blocked_note: "Waiting for a finance decision".into(),
            ..Default::default()
        });
        let activity = ActivityBatch {
            sessions: vec![
                SessionActivity {
                    session_key: "session-0".into(),
                    latest_request: Some("Check the consent form.".into()),
                    next_steps: vec!["Send the approved invitations.".into()],
                    ..Default::default()
                },
                SessionActivity {
                    session_key: "session-1".into(),
                    latest_update: Some("Compared three prices; finance must choose one.".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let overview = crate::projects::overview::build_overview(
            &project,
            &std::collections::HashMap::from([
                ("session-0".into(), WorkPhase::Working),
                ("session-1".into(), WorkPhase::NeedsInput),
            ]),
            &activity,
            state.title_language,
        );
        let key = project.canonical_key.clone();
        state.projects.snapshot.topics = vec![project];
        state.open_topic_detail(&key);
        state.replace_project_overview(overview);
        state
    }

    #[test]
    fn detail_shows_each_session_once_with_its_progress_and_own_actions() {
        let state = progress_fixture();
        let text = render_text(&state, 120, 36);
        for title in [
            "Confirm pricing",
            "Prepare invitations",
            "Review interview notes",
        ] {
            assert_eq!(text.matches(title).count(), 1, "duplicate session: {text}");
        }
        assert!(text.find("Confirm pricing") < text.find("Prepare invitations"));
        assert!(text.find("Prepare invitations") < text.find("Review interview notes"));
        assert!(text.find("Learn why new users leave") < text.find("Confirm pricing"));
        assert_eq!(text.matches("Check the consent form.").count(), 1);
        assert_eq!(text.matches("Send the approved invitations.").count(), 1);
        assert!(
            !text.contains("Authored step"),
            "the complete plan belongs in the editor"
        );
        let rows = topic_detail_rows(&state);
        let geometry = topic_detail_geometry(&state, Rect::new(0, 0, 120, 36));
        for hit in &geometry.overview_hits {
            let row = geometry
                .row_hits
                .iter()
                .find(|row| row.rect.intersection(hit.rect) == hit.rect)
                .unwrap();
            let ProjectTreeRow::Session(session) = &rows[row.row_index] else {
                panic!("action outside a session")
            };
            assert_eq!(
                hit.session_key.as_deref(),
                Some(session.stable_key.as_str())
            );
        }
        assert_eq!(
            geometry
                .overview_hits
                .iter()
                .filter(|hit| hit.shortcut.is_some())
                .count(),
            3
        );
    }

    #[test]
    fn detail_scroll_reaches_variable_height_sessions_and_paged_history() {
        let mut state = progress_fixture();
        let seed = state.projects.snapshot.topics[0].sessions[2].clone();
        for index in 3..65 {
            let mut session = seed.clone();
            session.stable_key = format!("session-{index}");
            session.title = format!("Earlier interview {index}");
            state.projects.snapshot.topics[0].sessions.push(session);
        }
        state.projects.snapshot.topics[0].next_cursor = Some(crate::projects::SessionCursor {
            last_activity_at: 1,
            stable_key: "older".into(),
        });
        for (width, height) in [(44, 14), (80, 24), (120, 32)] {
            let area = Rect::new(0, 0, width, height);
            state.view.topic_detail = topic_detail_geometry(&state, area);
            let rows = topic_detail_rows(&state);
            for (index, row) in rows.iter().enumerate() {
                state.projects.topic_detail_selected = index;
                state.projects.topic_detail_scroll = topic_detail_selection_scroll(&state);
                let geometry = topic_detail_geometry(&state, area);
                let selected = geometry
                    .row_hits
                    .iter()
                    .find(|hit| hit.row_index == index)
                    .unwrap_or_else(|| panic!("selection {index} hidden at {width}x{height}"));
                assert_eq!(
                    selected.rect.height,
                    row_height(
                        row,
                        state.visible_project_overview().as_ref(),
                        geometry.sessions.width
                    )
                    .min(geometry.sessions.height)
                );
                assert!(geometry
                    .row_hits
                    .iter()
                    .all(|hit| hit.rect.intersection(geometry.sessions) == hit.rect));
                assert!(geometry
                    .overview_hits
                    .iter()
                    .all(|hit| hit.rect.intersection(geometry.sessions) == hit.rect));
            }
            assert!(matches!(
                rows.last(),
                Some(ProjectTreeRow::LoadOlder { .. })
            ));
            let text = render_text(&state, width, height);
            assert!(text.contains("Load older conversations"), "{text}");
        }
    }

    #[test]
    fn overview_status_reordering_preserves_selected_session_and_scroll_anchor() {
        let mut state = progress_fixture();
        state.projects.topic_detail_selected = 1;
        state.projects.topic_detail_scroll = 1;
        let original = topic_detail_rows(&state)[1].identity();
        let mut next = state.visible_project_overview().unwrap();
        next.work[0].phase = WorkPhase::NeedsInput;
        next.work[1].phase = WorkPhase::Ready;
        state.replace_project_overview(next);
        let rows = topic_detail_rows(&state);
        assert_eq!(
            rows[state.projects.topic_detail_selected].identity(),
            original
        );
        assert_eq!(
            rows[state.projects.topic_detail_scroll].identity(),
            original
        );
        state.projects.filter = ProjectFilter::Open;
        assert_eq!(topic_detail_rows(&state).len(), 2);
        state.assert_invariants_for_test();
    }

    #[test]
    fn topic_cover_editor_ime_cursor_tracks_cjk_and_explicit_newlines() {
        let (text, scroll, column, row) =
            editor_text_layout("中文\n目标", "中文\n目标".len(), 4, 2);
        assert_eq!(
            text, "中文\n目标",
            "a newline at the wrapping boundary must not create a blank line"
        );
        assert_eq!((scroll, column, row), (1, 0, 1));
        let (text, scroll, column, row) = editor_text_layout("hello world", 7, 6, 2);
        assert_eq!(text, "hello \nworld");
        assert_eq!((scroll, column, row), (0, 1, 1));
        let (text, _, column, row) = editor_text_layout("a\t中", 2, 6, 2);
        assert_eq!(text, "a    \n中");
        assert_eq!((column, row), (0, 1));
    }

    #[test]
    fn topic_cover_keeps_an_authored_topic_visible_without_conversations() {
        let mut state = fixture(true);
        state.sidebar_view = crate::app::state::SidebarView::Clusters;
        state.projects.snapshot.topics[0].sessions.clear();
        assert!(super::super::projects::project_tree_rows(&state)
            .iter()
            .any(|row| matches!(row, ProjectTreeRow::Project { .. })));
        let text = render_text(&state, 120, 26);
        assert!(text.contains("Interview five users"));
        assert!(text.contains("No conversations in this Work yet"));
    }
}
