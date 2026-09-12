use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph, Wrap},
    Frame,
};
use unicode_width::UnicodeWidthChar;

use super::projects::ProjectTreeRow;
use super::widgets::{
    action_button_row_rects, centered_popup_rect, panel_contrast_fg, render_action_button,
    render_modal_header, render_modal_shell, render_panel_shell, ActionButtonSpec,
};
use crate::app::state::{AppState, ProjectFilter, ProjectRowHitArea, TopicDetailGeometry};
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
    let mut rows: Vec<_> = topic
        .sessions
        .iter()
        .filter(|session| app.projects.filter != ProjectFilter::Open || session.live)
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

fn cover_is_empty(cover: &TopicCover) -> bool {
    cover.goal.is_empty() && cover.next_steps.is_empty() && cover.blocked_note.is_empty()
}

pub(crate) fn topic_detail_geometry(app: &AppState, area: Rect) -> TopicDetailGeometry {
    if area.width < 4 || area.height < 4 {
        return TopicDetailGeometry::default();
    }
    let inner = Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2);
    let is_topic = selected_topic(app)
        .is_some_and(|topic| topic.kind == crate::projects::ProjectKind::Semantic);
    let edit_width = if is_topic { inner.width.min(15) } else { 0 };
    let edit = Rect::new(inner.right() - edit_width, inner.y, edit_width, 1);
    let refresh_width = inner.width.saturating_sub(edit_width).min(12);
    let refresh = Rect::new(
        edit.x.saturating_sub(refresh_width),
        inner.y,
        refresh_width,
        1,
    );
    let title = Rect::new(
        inner.x,
        inner.y,
        inner.width.saturating_sub(edit_width + refresh_width + 1),
        1,
    );
    let filled = selected_topic(app)
        .and_then(|topic| topic.cover.as_ref())
        .is_some_and(|cover| !cover_is_empty(cover));
    // Automatic progress is useful before a plan is written. The authored plan is
    // a compact supplement, with at least one conversation and navigation kept visible.
    let cover_height = if filled {
        if inner.height >= 24 {
            if inner.width >= 82 {
                6
            } else {
                7
            }
        } else if inner.width < 80 && inner.height >= 13 {
            5
        } else {
            3
        }
    } else {
        0
    }
    .min(inner.height.saturating_sub(6));
    let preferred = 24;
    // An empty plan gives the conversation list extra space, rather than growing
    // the overview until only one conversation remains visible.
    let reserved = if filled { cover_height + 6 } else { 8 };
    let overview_height = preferred.min(inner.height.saturating_sub(reserved));
    let overview = Rect::new(inner.x, inner.y + 2, inner.width, overview_height);
    let cover = Rect::new(inner.x, overview.bottom(), inner.width, cover_height);
    let heading = Rect::new(
        inner.x,
        cover.bottom(),
        inner.width,
        1.min(inner.height.saturating_sub(2)),
    );
    let footer = Rect::new(inner.x, inner.bottom() - 1, inner.width, 1);
    let sessions = Rect::new(
        inner.x,
        heading.bottom(),
        inner.width,
        footer.y.saturating_sub(heading.bottom()),
    );
    let rows = topic_detail_rows(app);
    let visible = usize::from(sessions.height.div_ceil(2)).max(1);
    let normalized_scroll = app
        .projects
        .topic_detail_scroll
        .min(rows.len().saturating_sub(visible));
    let row_hits = rows
        .iter()
        .enumerate()
        .skip(normalized_scroll)
        .take(visible)
        .filter_map(|(row_index, row)| {
            let y = sessions.y + ((row_index - normalized_scroll) as u16).saturating_mul(2);
            if y >= sessions.bottom() {
                return None;
            }
            row.action().map(|action| ProjectRowHitArea {
                rect: Rect::new(sessions.x, y, sessions.width, 2.min(sessions.bottom() - y)),
                action,
                row_index,
            })
        })
        .collect();
    TopicDetailGeometry {
        title,
        edit,
        cover,
        overview,
        refresh,
        overview_hits: app
            .visible_project_overview()
            .map(|value| super::project_overview::overview_hit_areas(&value, overview))
            .unwrap_or_default(),
        heading,
        sessions,
        footer,
        row_hits,
        normalized_scroll,
    }
}

pub(super) fn render_topic_detail(app: &AppState, frame: &mut Frame, area: Rect) {
    frame.render_widget(Clear, area);
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
                app.title_language.text("Topic", "主题")
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
    if let Some(overview) = app.visible_project_overview() {
        super::project_overview::render_project_overview(app, frame, geometry.overview, &overview);
    }
    let empty_cover = TopicCover::default();
    render_cover(
        app,
        frame,
        geometry.cover,
        topic.cover.as_ref().unwrap_or(&empty_cover),
    );
    frame.render_widget(
        Paragraph::new(if topic.next_cursor.is_some() {
            app.title_language.text(
                "Conversations · older history below",
                "会话 · 下方可加载更早记录",
            )
        } else {
            app.title_language.text("Conversations", "会话")
        })
        .style(
            Style::default()
                .fg(app.palette.subtext0)
                .add_modifier(Modifier::BOLD),
        ),
        geometry.heading,
    );
    let rows = topic_detail_rows(app);
    if rows.is_empty() {
        frame.render_widget(
            Paragraph::new(
                if app.title_language == crate::config::TitleLanguage::Chinese {
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
                            "Topic"
                        } else {
                            "Project"
                        },
                        if app.projects.filter == ProjectFilter::Open {
                            ""
                        } else {
                            " yet"
                        }
                    )
                },
            )
            .style(Style::default().fg(app.palette.overlay0)),
            geometry.sessions,
        );
    }
    for hit in &geometry.row_hits {
        let Some(row) = rows.get(hit.row_index) else {
            continue;
        };
        let selected = hit.row_index == app.projects.topic_detail_selected;
        let style = Style::default().fg(app.palette.text).bg(if selected {
            app.palette.surface1
        } else {
            app.palette.panel_bg
        });
        match row {
            ProjectTreeRow::Session(session) => {
                let label = super::session_label::session_label(
                    &session.title,
                    session.topic_label.as_deref(),
                );
                let title = Line::from(vec![
                    Span::styled(
                        format!("{} ", session.backend),
                        Style::default().fg(app.palette.accent),
                    ),
                    Span::styled(label.task, Style::default().add_modifier(Modifier::BOLD)),
                ]);
                frame.render_widget(
                    Paragraph::new(vec![
                        title,
                        super::projects::session_status_line(app, session),
                    ])
                    .style(style),
                    hit.rect,
                );
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
    frame.render_widget(
        Paragraph::new(if topic.kind == crate::projects::ProjectKind::Semantic {
            app.title_language.text(
                "1–3 follow-up · ↑↓ conversations · enter open · e plan · esc back",
                "1–3 推进建议 · ↑↓ 选择会话 · Enter 打开 · e 计划 · Esc 返回",
            )
        } else {
            app.title_language.text(
                "1–3 follow-up · ↑↓ conversations · enter open · r refresh · esc back",
                "1–3 推进建议 · ↑↓ 选择会话 · Enter 打开 · r 刷新 · Esc 返回",
            )
        })
        .style(Style::default().fg(app.palette.overlay0)),
        geometry.footer,
    );
}

fn render_cover(app: &AppState, frame: &mut Frame, area: Rect, cover: &TopicCover) {
    if cover_is_empty(cover) || area.height == 0 {
        return;
    }
    if area.height <= 5 {
        let next = cover
            .next_steps
            .iter()
            .enumerate()
            .map(|(i, step)| format!("{}. {step}", i + 1))
            .collect::<Vec<_>>()
            .join("; ");
        let mut lines = vec![cover_line(
            app,
            app.title_language.text("Goal", "目标"),
            &cover.goal,
            app.title_language.text("Not set", "尚未填写"),
            area.width,
        )];
        if area.height >= 5 && !cover.next_steps.is_empty() {
            lines.extend(
                cover.next_steps.iter().enumerate().map(|(index, step)| {
                    Line::from(format!("{}. {}", index + 1, single_line(step)))
                }),
            );
        } else {
            lines.push(cover_line(
                app,
                app.title_language.text("Next", "下一步"),
                &next,
                app.title_language.text("Not set", "尚未填写"),
                area.width,
            ));
        }
        lines.push(cover_line(
            app,
            app.title_language.text("Blocked", "阻塞"),
            &cover.blocked_note,
            app.title_language.text("None recorded", "暂无记录"),
            area.width,
        ));
        frame.render_widget(
            Paragraph::new(lines).style(Style::default().fg(app.palette.text)),
            area,
        );
        return;
    }
    let Some(inner) = render_panel_shell(frame, area, app.palette.overlay0, app.palette.panel_bg)
    else {
        return;
    };
    if inner.width >= 80 && inner.height >= 4 {
        let columns = Layout::horizontal([
            Constraint::Ratio(1, 3),
            Constraint::Length(2),
            Constraint::Ratio(1, 3),
            Constraint::Length(2),
            Constraint::Ratio(1, 3),
        ])
        .split(inner);
        cover_section(
            app,
            frame,
            columns[0],
            app.title_language.text("This week's goal", "本周目标"),
            &cover.goal,
            app.title_language.text("Add a goal", "填写目标"),
        );
        let steps = if cover.next_steps.is_empty() {
            app.title_language
                .text("Add up to three next steps", "最多填写三项下一步")
                .to_string()
        } else {
            cover
                .next_steps
                .iter()
                .enumerate()
                .map(|(i, step)| {
                    format!(
                        "{}. {}",
                        i + 1,
                        super::text::truncate_end(
                            &single_line(step),
                            usize::from(columns[2].width.saturating_sub(3))
                        )
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        cover_section(
            app,
            frame,
            columns[2],
            app.title_language.text("Next steps", "下一步"),
            &steps,
            app.title_language
                .text("Add up to three next steps", "最多填写三项下一步"),
        );
        cover_section(
            app,
            frame,
            columns[4],
            app.title_language.text("What's blocked", "当前阻塞"),
            &cover.blocked_note,
            app.title_language
                .text("Add a blocker if needed", "如有阻塞请在此填写"),
        );
    } else {
        let mut lines = vec![cover_line(
            app,
            app.title_language.text("Goal", "目标"),
            &cover.goal,
            app.title_language
                .text("Add this week's goal", "填写本周目标"),
            inner.width,
        )];
        // Reserve a line each for the goal and blocker before expanding the steps.
        let expanded_height = cover.next_steps.len().max(1) as u16 + 2;
        if inner.height >= expanded_height {
            if inner.height > expanded_height {
                lines.push(Line::from(Span::styled(
                    app.title_language.text("Next steps", "下一步"),
                    Style::default().fg(app.palette.accent),
                )));
            }
            if cover.next_steps.is_empty() {
                lines.push(Line::from(
                    app.title_language
                        .text("Add up to three next steps", "最多填写三项下一步"),
                ));
            } else {
                lines.extend(cover.next_steps.iter().enumerate().map(|(i, step)| {
                    Line::from(format!(
                        "{}. {}",
                        i + 1,
                        super::text::truncate_end(
                            &single_line(step),
                            usize::from(inner.width.saturating_sub(3))
                        )
                    ))
                }));
            }
        } else {
            let steps = cover
                .next_steps
                .iter()
                .enumerate()
                .map(|(i, step)| format!("{}. {step}", i + 1))
                .collect::<Vec<_>>()
                .join("; ");
            lines.push(cover_line(
                app,
                app.title_language.text("Next", "下一步"),
                &steps,
                app.title_language.text("Add next steps", "填写下一步"),
                inner.width,
            ));
        }
        lines.push(cover_line(
            app,
            app.title_language.text("Blocked", "阻塞"),
            &cover.blocked_note,
            app.title_language
                .text("Add a blocker if needed", "如有阻塞请在此填写"),
            inner.width,
        ));
        frame.render_widget(
            Paragraph::new(lines).style(Style::default().fg(app.palette.text)),
            inner,
        );
    }
}

fn single_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn cover_line(
    app: &AppState,
    label: &str,
    value: &str,
    placeholder: &str,
    width: u16,
) -> Line<'static> {
    let prefix = format!("{label}: ");
    let available =
        usize::from(width).saturating_sub(unicode_width::UnicodeWidthStr::width(prefix.as_str()));
    Line::from(vec![
        Span::styled(prefix, Style::default().fg(app.palette.accent)),
        Span::raw(super::text::truncate_end(
            &single_line(if value.is_empty() { placeholder } else { value }),
            available,
        )),
    ])
}

fn cover_section(
    app: &AppState,
    frame: &mut Frame,
    area: Rect,
    label: &str,
    value: &str,
    placeholder: &str,
) {
    frame.render_widget(
        Paragraph::new(label).style(
            Style::default()
                .fg(app.palette.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Rect::new(area.x, area.y, area.width, area.height.min(1)),
    );
    frame.render_widget(
        Paragraph::new(if value.is_empty() { placeholder } else { value })
            .style(Style::default().fg(if value.is_empty() {
                app.palette.overlay0
            } else {
                app.palette.text
            }))
            .wrap(Wrap { trim: false }),
        Rect::new(
            area.x,
            area.y + 1,
            area.width,
            area.height.saturating_sub(1),
        ),
    );
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
            app.title_language.text("Topic plan", "主题计划"),
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
            "1. Send invitations",
            "2. Schedule calls",
            "3. Prepare questions",
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
            "主题 ·",
            "编辑计划",
            "刷新",
            "现在在做什么",
            "接下来可以这样推进",
            "历史会话",
            "历史记录 · 只读",
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
            "主题计划",
            "本周目标",
            "下一步 1",
            "当前阻塞",
            "保存",
            "取消",
            "Interview five users",
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
            (&topic_key, "Topic ·", true),
            (&folder_key, "Project ·", false),
        ] {
            assert!(state.open_topic_detail(key));
            let text = render_text(&state, 124, 32);
            for expected in [
                label,
                "What's happening",
                "Suggested follow-ups",
                "Plan first user interviews",
                "recorded conversation",
            ] {
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
        assert!(text.contains("Suggested follow-ups"));
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
        // Five inner cover rows fit the goal, all three steps, and the blocker.
        let text = render_text(&state, 70, 15);
        for expected in [
            "Goal: Interview five users",
            "1. Send invitations",
            "2. Schedule calls",
            "3. Prepare questions",
            "Blocked: Waiting for replies",
            "Plan first user interviews",
        ] {
            assert!(text.contains(expected), "missing {expected}:\n{text}");
        }
        // One fewer row uses the compact steps line, retaining the blocker and sessions.
        let compact = render_text(&state, 70, 14);
        assert!(compact.contains("Next: 1. Send invitations"));
        assert!(compact.contains("Blocked: Waiting for replies"));
        assert!(compact.contains("Plan first user interviews"));
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
        assert!(text.contains("No conversations in this Topic yet"));
    }
}
