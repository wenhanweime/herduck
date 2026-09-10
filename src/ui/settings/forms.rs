//! Read-only configuration details with shared wrapping and scrolling geometry.

use crate::app::{state::SettingsSection, AppState};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};

pub(crate) struct SettingsForm {
    pub lines: Vec<Line<'static>>,
    rows: Vec<FormRow>,
    width: u16,
    height: u16,
}

struct FormRow {
    prefix: Line<'static>,
    value: Line<'static>,
    indent: u16,
    height: u16,
}

impl SettingsForm {
    pub(super) fn new(width: u16) -> Self {
        Self {
            lines: Vec::new(),
            rows: Vec::new(),
            width: width.max(1),
            height: 0,
        }
    }

    pub(super) fn text(&mut self, text: impl Into<String>, style: Style) {
        self.line(Line::from(Span::styled(text.into(), style)));
    }

    pub(super) fn line(&mut self, line: Line<'static>) {
        self.prefixed(Line::default(), line);
    }

    /// Wrap values inside their own column, so continuation rows keep their indentation.
    pub(super) fn prefixed(&mut self, prefix: Line<'static>, value: Line<'static>) {
        let indent = prefix.width().min(u16::MAX as usize) as u16;
        if indent >= self.width {
            self.line(prefix);
            self.line(value);
            return;
        }
        let height = Paragraph::new(value.clone())
            .wrap(Wrap { trim: false })
            .line_count(self.width - indent)
            .max(1)
            .min(u16::MAX as usize) as u16;
        self.height = self.height.saturating_add(height);
        let mut spans = prefix.spans.clone();
        spans.extend(value.spans.clone());
        self.lines.push(Line::from(spans));
        self.rows.push(FormRow {
            prefix,
            value,
            indent,
            height,
        });
    }

    pub(super) fn field(&mut self, label: &str, value: &str, app: &AppState) {
        let label_style = Style::default().fg(app.palette.overlay1);
        if self.width < 40 {
            self.text(label, label_style);
            self.prefixed(
                Line::from("  "),
                Line::from(Span::styled(
                    value.to_owned(),
                    Style::default().fg(app.palette.text),
                )),
            );
        } else {
            self.prefixed(
                Line::from(Span::styled(format!("{label:<17}"), label_style)),
                Line::from(Span::styled(
                    value.to_owned(),
                    Style::default().fg(app.palette.text),
                )),
            );
        }
    }

    pub(super) fn section(&mut self, label: &str, app: &AppState) {
        if !self.lines.is_empty() {
            self.text("", Style::default());
        }
        let label_width = crate::ui::text::display_width_u16(label);
        self.line(Line::from(vec![
            Span::styled(label.to_owned(), Style::default().fg(app.palette.overlay1)),
            Span::styled(
                format!(
                    " {}",
                    "─".repeat(self.width.saturating_sub(label_width + 1) as usize)
                ),
                Style::default().fg(app.palette.surface0),
            ),
        ]));
    }

    pub(super) fn height(&self) -> u16 {
        self.height
    }

    pub(super) fn render(&self, frame: &mut Frame, area: Rect, scroll: u16) {
        if area.is_empty() {
            return;
        }
        let offset = self.scroll_offset(area.height, scroll);
        let bottom = offset.saturating_add(area.height);
        let mut top = 0u16;
        for row in &self.rows {
            let end = top.saturating_add(row.height);
            let visible_top = top.max(offset);
            let visible_end = end.min(bottom);
            if visible_top < visible_end {
                let y = area.y + visible_top - offset;
                if visible_top == top && row.indent > 0 {
                    frame.render_widget(
                        Paragraph::new(row.prefix.clone()),
                        Rect::new(area.x, y, row.indent.min(area.width), 1),
                    );
                }
                frame.render_widget(
                    Paragraph::new(row.value.clone())
                        .wrap(Wrap { trim: false })
                        .scroll((visible_top - top, 0)),
                    Rect::new(
                        area.x + row.indent,
                        y,
                        area.width.saturating_sub(row.indent),
                        visible_end - visible_top,
                    ),
                );
            }
            top = end;
            if top >= bottom {
                break;
            }
        }
    }

    pub(crate) fn max_scroll(&self, height: u16) -> u16 {
        self.height.saturating_sub(height)
    }

    pub(crate) fn scroll_offset(&self, height: u16, scroll: u16) -> u16 {
        scroll.min(self.max_scroll(height))
    }
}

pub(crate) fn settings_form(
    app: &AppState,
    width: u16,
    inline_config: bool,
) -> Option<SettingsForm> {
    let mut form = match app.settings.section {
        SettingsSection::Sessions => Some(session_form(app, width)),
        SettingsSection::Summaries | SettingsSection::Titles => {
            Some(super::source_form::source_form(app, width))
        }
        _ => None,
    }?;
    if inline_config {
        form.text("", Style::default());
        config_location(&mut form, app);
    }
    Some(form)
}

pub(super) fn config_footer(app: &AppState, width: u16) -> SettingsForm {
    let mut form = SettingsForm::new(width);
    config_location(&mut form, app);
    form
}

fn config_location(form: &mut SettingsForm, app: &AppState) {
    let style = Style::default().fg(app.palette.overlay1);
    form.text(
        "─".repeat(form.width as usize),
        Style::default().fg(app.palette.surface0),
    );
    form.prefixed(
        Line::from(Span::styled("Config  ", style)),
        Line::from(Span::styled(
            app.settings.config_path.display().to_string(),
            Style::default().fg(app.palette.text),
        )),
    );
    form.text(
        "Save the file, then reopen Settings to load changes.",
        style,
    );
}

fn session_form(app: &AppState, width: u16) -> SettingsForm {
    let mut form = SettingsForm::new(width);
    let text = Style::default().fg(app.palette.overlay1);
    let setup = &app.session_setup;
    let agent = if setup.agent.is_empty() {
        "shell"
    } else {
        &setup.agent
    };
    form.field("Default Agent", agent, app);
    form.section("Session behavior", app);
    form.field("Start session", agent, app);
    form.field("New tab / split", "Shell", app);
    form.field("Resume", "Original Agent", app);
    form.section("History directories", app);
    for location in &setup.history_locations {
        form.line(Line::from(vec![
            Span::styled(
                location.adapter,
                Style::default()
                    .fg(app.palette.text)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                if location.restart_required {
                    " · restart required"
                } else {
                    ""
                },
                Style::default().fg(app.palette.yellow),
            ),
        ]));
        for (index, path) in location.paths.iter().enumerate() {
            form.prefixed(
                Line::from(Span::styled(
                    if index + 1 == location.paths.len() {
                        "└─ "
                    } else {
                        "├─ "
                    },
                    text,
                )),
                Line::from(Span::styled(path.clone(), text)),
            );
        }
    }
    form.section("Reload", app);
    form.text(
        "History directory changes require restarting HERDUCK.",
        text,
    );
    form.text("Extra directories: [projects.adapters.<agent>].", text);
    form
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn wrapped_values_keep_their_column_and_scroll_as_complete_rows() {
        for width in [24, 38, 73] {
            let mut form = SettingsForm::new(width);
            form.prefixed(
                Line::from("Endpoint    "),
                Line::from("https://example.invalid/目录/a-very-long-directory/路径/another-long-directory/a-very-long-value/endpoint"),
            );
            form.text("Final visible row", Style::default());
            let render = |height, scroll| {
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                terminal
                    .draw(|frame| form.render(frame, frame.area(), scroll))
                    .unwrap();
                terminal
            };
            let full = render(form.height(), 0);
            // The endpoint wraps even at the widest size, keeping its continuation indented.
            for x in 0..12 {
                assert_eq!(full.backend().buffer()[(x, 1)].symbol(), " ");
            }
            let viewport = 3;
            for offset in 0..=form.max_scroll(viewport) {
                let scrolled = render(viewport, offset);
                for y in 0..viewport {
                    for x in 0..width {
                        assert_eq!(
                            scrolled.backend().buffer()[(x, y)],
                            full.backend().buffer()[(x, y + offset)],
                            "{width} columns, scroll {offset}, cell ({x}, {y})"
                        );
                    }
                }
            }
        }
    }
}
