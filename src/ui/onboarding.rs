use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::widgets::{
    action_button_width, modal_stack_areas, panel_contrast_fg, render_action_button,
    render_modal_shell,
};
use crate::app::AppState;

const ONBOARDING_PREFIX_LABEL: &str = "ctrl+b";

fn configuration_label(area: Rect) -> &'static str {
    if area.width < 48 {
        "config"
    } else {
        "view configuration"
    }
}

fn configuration_hint(area: Rect) -> Option<&'static str> {
    (area.width >= 48).then_some("↵")
}

pub(super) fn render_onboarding_overlay(app: &AppState, frame: &mut Frame, area: Rect) {
    super::dim_background(frame, area);
    render_onboarding_welcome(app, frame, area);
}

pub(crate) fn onboarding_welcome_continue_rect(area: Rect) -> Rect {
    Rect::new(
        area.x,
        area.y,
        action_button_width(configuration_hint(area), configuration_label(area)),
        1,
    )
}

pub(crate) fn onboarding_welcome_skip_rect(area: Rect) -> Rect {
    let next = onboarding_welcome_continue_rect(area);
    Rect::new(
        next.right() + 2,
        area.y,
        action_button_width((area.width >= 48).then_some("esc"), "skip"),
        1,
    )
}

fn render_onboarding_welcome(app: &AppState, frame: &mut Frame, area: Rect) {
    let Some(inner) = render_modal_shell(frame, area, 64, 16, &app.palette) else {
        return;
    };
    if inner.height < 4 {
        return;
    }

    let stack = modal_stack_areas(inner, 2, 0, 1, 1);
    let header_rows =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas::<2>(stack.header);
    let content_rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas::<4>(stack.content);

    frame.render_widget(
        Paragraph::new("  herduck · welcome").style(
            Style::default()
                .fg(app.palette.text)
                .add_modifier(Modifier::BOLD),
        ),
        header_rows[0],
    );

    let detected = if app.session_setup.detected_agents.is_empty() {
        "none; a plain terminal works too".into()
    } else {
        app.session_setup.detected_agents.join(", ")
    };
    frame.render_widget(
        Paragraph::new(format!("  Agents found: {detected}"))
            .style(Style::default().fg(app.palette.overlay1)),
        content_rows[1],
    );
    frame.render_widget(
        Paragraph::new("  terminal workspace manager for coding agents")
            .style(Style::default().fg(app.palette.overlay0)),
        header_rows[1],
    );

    frame.render_widget(
        Paragraph::new(
            "  this is a mouse-first terminal.\n  click the sidebar to switch workspaces, drag pane\n  borders to resize, right-click for context menus.",
        )
        .style(Style::default().fg(app.palette.overlay1)),
        content_rows[0],
    );

    let key_line = Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            ONBOARDING_PREFIX_LABEL,
            Style::default()
                .fg(app.palette.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " enters prefix mode · ",
            Style::default().fg(app.palette.overlay1),
        ),
        Span::styled(
            "?",
            Style::default()
                .fg(app.palette.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " shows keybinds and settings",
            Style::default().fg(app.palette.overlay1),
        ),
    ]);
    frame.render_widget(Paragraph::new(key_line), content_rows[2]);

    frame.render_widget(
        Paragraph::new("  Configure Agents and summaries in config.toml.\n  Settings shows the current configuration and priorities.")
            .style(Style::default().fg(app.palette.overlay1))
            .wrap(ratatui::widgets::Wrap { trim: false }),
        content_rows[3],
    );

    let actions = stack.actions.unwrap_or_default();
    let continue_rect = onboarding_welcome_continue_rect(actions);
    render_action_button(
        frame,
        continue_rect,
        configuration_hint(actions),
        configuration_label(actions),
        Style::default()
            .fg(panel_contrast_fg(&app.palette))
            .bg(app.palette.accent)
            .add_modifier(Modifier::BOLD),
    );
    render_action_button(
        frame,
        onboarding_welcome_skip_rect(stack.actions.unwrap_or_default()),
        (actions.width >= 48).then_some("esc"),
        "skip",
        Style::default()
            .fg(app.palette.text)
            .bg(app.palette.surface0),
    );
}
