//! Read-only configuration details with shared wrapping and scrolling geometry.

use crate::app::{state::SettingsSection, AppState};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

pub(crate) struct SettingsForm {
    pub lines: Vec<Line<'static>>,
    width: u16,
    height: u16,
}

impl SettingsForm {
    pub(super) fn new(width: u16) -> Self {
        Self {
            lines: Vec::new(),
            width: width.max(1),
            height: 0,
        }
    }

    pub(super) fn text(&mut self, text: impl Into<String>, style: Style) {
        let line = Line::from(Span::styled(text.into(), style));
        let height = Paragraph::new(line.clone())
            .wrap(Wrap { trim: false })
            .line_count(self.width)
            .max(1)
            .min(u16::MAX as usize) as u16;
        self.height = self.height.saturating_add(height);
        self.lines.push(line);
    }

    pub(crate) fn max_scroll(&self, height: u16) -> u16 {
        self.height.saturating_sub(height)
    }

    pub(crate) fn scroll_offset(&self, height: u16, scroll: u16) -> u16 {
        scroll.min(self.max_scroll(height))
    }
}

pub(crate) fn settings_form(app: &AppState, width: u16) -> Option<SettingsForm> {
    match app.settings.section {
        SettingsSection::Sessions => Some(session_form(app, width)),
        SettingsSection::Summaries | SettingsSection::Titles => {
            Some(super::source_form::source_form(app, width))
        }
        _ => None,
    }
}

pub(super) fn config_location(form: &mut SettingsForm, app: &AppState) {
    let style = Style::default().fg(app.palette.overlay1);
    form.text("", style);
    form.text(
        "Edit in config.toml",
        Style::default()
            .fg(app.palette.text)
            .add_modifier(Modifier::BOLD),
    );
    form.text(app.settings.config_path.display().to_string(), style);
    form.text(
        "Save the file, then reopen Settings to load changes.",
        style,
    );
}

fn session_form(app: &AppState, width: u16) -> SettingsForm {
    let mut form = SettingsForm::new(width);
    let heading = Style::default()
        .fg(app.palette.text)
        .add_modifier(Modifier::BOLD);
    let text = Style::default().fg(app.palette.overlay1);
    let setup = &app.session_setup;
    form.text("Sessions · read only", heading);
    form.text(
        format!(
            "Default Agent: {}",
            if setup.agent.is_empty() {
                "shell"
            } else {
                &setup.agent
            }
        ),
        Style::default().fg(app.palette.text),
    );
    form.text("Start session uses this Agent. Tabs and splits open a shell; Resume uses the original Agent.", text);
    form.text("", text);
    form.text("History directories", heading);
    for location in &setup.history_locations {
        form.text(
            format!("{}:", location.adapter),
            Style::default().fg(app.palette.text),
        );
        for path in &location.paths {
            form.text(format!("  {path}"), text);
        }
        if location.restart_required {
            form.text(
                "  New directories take effect after restarting HERDUCK.",
                Style::default().fg(app.palette.yellow),
            );
        }
    }
    form.text(
        "History is read only. Configure extra directories in [projects.adapters.<agent>].",
        text,
    );
    config_location(&mut form, app);
    form
}
