use ratatui::{
    layout::{Constraint, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};

use super::{
    brand::{render_mascot, TAGLINE},
    widgets::{
        action_button_width, centered_popup_rect, modal_stack_areas, panel_contrast_fg,
        render_action_button, render_panel_shell,
    },
};
use crate::app::AppState;

pub(crate) struct OnboardingLayout {
    pub popup: Rect,
    pub header: Rect,
    pub content: Rect,
    pub actions: Rect,
}

/// Rendering and mouse handling share the same responsive geometry.
pub(crate) fn onboarding_layout(area: Rect) -> Option<OnboardingLayout> {
    let popup = centered_popup_rect(area, 78, 24)?;
    let inner = popup.inner(Margin::new(2, 1));
    if inner.height < 5 || inner.width < 16 {
        return None;
    }
    let stack = modal_stack_areas(inner, 2, 0, 1, 1);
    Some(OnboardingLayout {
        popup,
        header: stack.header,
        content: stack.content,
        actions: stack.actions.unwrap_or_default(),
    })
}

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
    let Some(layout) = onboarding_layout(area) else {
        return;
    };
    let p = &app.palette;
    render_panel_shell(frame, layout.popup, p.accent, p.panel_bg);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("her", Style::default().fg(p.text)),
                Span::styled("duck", Style::default().fg(p.yellow)),
                Span::styled(" · welcome", Style::default().fg(p.overlay1)),
            ])
            .style(Style::default().add_modifier(Modifier::BOLD)),
            Line::styled(TAGLINE, Style::default().fg(p.text)),
        ]),
        layout.header,
    );

    let content = layout.content;
    let copy = if content.width >= 66 && content.height >= 11 {
        let [mascot, _, copy] = Layout::horizontal([
            Constraint::Length(29),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .areas::<3>(content);
        render_mascot(frame, mascot, p, false);
        copy
    } else if content.height >= 12 {
        let [mascot, _, copy] = Layout::vertical([
            Constraint::Length(4),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas::<3>(content);
        render_mascot(frame, mascot, p, true);
        copy
    } else {
        content
    };

    let detected = if app.session_setup.detected_agents.is_empty() {
        "none; use a shell".into()
    } else {
        app.session_setup.detected_agents.join(", ")
    };
    let prefix = crate::config::format_key_combo((app.prefix_code, app.prefix_mods));
    let help = app
        .keybinds
        .help
        .label()
        .map(|label| label.replace("prefix+", &format!("{prefix}, ")))
        .unwrap_or_else(|| "menu".into());
    let mut lines = vec![
        Line::styled("Your Agents. One terminal.", Style::default().fg(p.text)),
        Line::raw("Run work side by side."),
        Line::raw("Find past work in Sessions."),
    ];
    if copy.width >= 32 {
        lines.push(Line::raw("Group it in Projects and Topics."));
    }
    if copy.height >= 10 {
        lines.extend([
            Line::raw(""),
            Line::raw("Click sidebar tabs to switch."),
            Line::raw("Drag pane borders to resize."),
            Line::raw(""),
        ]);
    }
    // Actions stay fixed below the body even if a long agent list needs clipping.
    lines.push(Line::styled(
        format!("{help} · keybinds"),
        Style::default().fg(p.accent),
    ));
    lines.push(Line::raw(format!("Agents found: {detected}")));
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(p.overlay1))
            .wrap(Wrap { trim: false }),
        copy,
    );

    let actions = layout.actions;
    render_action_button(
        frame,
        onboarding_welcome_continue_rect(actions),
        configuration_hint(actions),
        configuration_label(actions),
        Style::default()
            .fg(panel_contrast_fg(p))
            .bg(p.accent)
            .add_modifier(Modifier::BOLD),
    );
    render_action_button(
        frame,
        onboarding_welcome_skip_rect(actions),
        (actions.width >= 48).then_some("esc"),
        "skip",
        Style::default().fg(p.text).bg(p.surface0),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn welcome_keeps_actions_visible_at_every_supported_size() {
        for (width, height) in [(110, 32), (80, 24), (60, 24), (40, 16), (32, 16)] {
            let app = AppState::test_new();
            let area = Rect::new(0, 0, width, height);
            let layout = onboarding_layout(area).unwrap();
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| render_onboarding_welcome(&app, frame, area))
                .unwrap();
            let buffer = terminal.backend().buffer();
            for button in [
                onboarding_welcome_continue_rect(layout.actions),
                onboarding_welcome_skip_rect(layout.actions),
            ] {
                assert!(layout.popup.contains((button.x, button.y).into()));
                assert!(button.right() < layout.popup.right());
                let label: String = (button.x..button.right())
                    .map(|x| buffer[(x, button.y)].symbol())
                    .collect();
                assert!(!label.trim().is_empty(), "{width}×{height}: action lost");
            }
            let screen: String = buffer.content().iter().map(|cell| cell.symbol()).collect();
            assert!(
                screen.contains(TAGLINE),
                "{width}×{height}: missing tagline"
            );
            assert!(screen.contains("config"));
            assert!(screen.contains("skip"));
        }
    }

    #[test]
    fn welcome_displays_the_configured_help_binding() {
        let mut app = AppState::test_new();
        app.prefix_code = crossterm::event::KeyCode::Char('a');
        app.keybinds.help = crate::config::ActionKeybinds::prefix("h");
        let mut terminal = Terminal::new(TestBackend::new(110, 32)).unwrap();
        terminal
            .draw(|frame| render_onboarding_welcome(&app, frame, frame.area()))
            .unwrap();
        let screen: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(screen.contains("ctrl+a, h"));
        assert!(!screen.contains("ctrl+b"));
    }
}
