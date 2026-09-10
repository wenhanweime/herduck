use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
    Frame,
};

mod forms;
mod layout;
mod source_form;

pub(crate) use forms::settings_form;
pub(crate) use layout::{
    settings_button_rects, settings_can_start_session, settings_layout, settings_new_session_rect,
    settings_tab_rects,
};
use layout::{settings_buttons, SettingsButtonKind};

use super::widgets::{
    centered_popup_rect, panel_contrast_fg, render_action_button, render_modal_choice_list,
    render_panel_shell,
};
use crate::{
    app::{
        state::{ExperimentSetting, Palette},
        AppState,
    },
    config::ToastDelivery,
};

pub(crate) const SETTINGS_POPUP_WIDTH: u16 = 96;
pub(crate) const SETTINGS_POPUP_BASE_HEIGHT: u16 = 30;

pub(crate) fn settings_popup_height(app: &AppState) -> u16 {
    if app.settings.section != crate::app::state::SettingsSection::Integrations {
        return SETTINGS_POPUP_BASE_HEIGHT;
    }
    let list_rows = app.integration_recommendations.len().max(1) as u16;
    let footer_rows = integrations_footer_height(app, SETTINGS_POPUP_WIDTH - 23);
    // Leave room for navigation, section description, and installation feedback.
    (14 + list_rows + footer_rows).max(SETTINGS_POPUP_BASE_HEIGHT)
}

pub(super) fn render_settings_overlay(app: &AppState, frame: &mut Frame, area: Rect) {
    use crate::app::state::SettingsSection;

    let p = &app.palette;
    let Some(popup) = centered_popup_rect(area, SETTINGS_POPUP_WIDTH, settings_popup_height(app))
    else {
        return;
    };

    super::dim_background(frame, area);

    let Some(inner) = render_panel_shell(frame, popup, p.accent, p.panel_bg) else {
        return;
    };
    if inner.height < 4 || inner.width < 10 {
        return;
    }

    let layout = settings_layout(app, inner);
    let details = matches!(
        app.settings.section,
        SettingsSection::Sessions | SettingsSection::Summaries | SettingsSection::Titles
    );
    frame.render_widget(
        Paragraph::new(format!(" settings · {}", app.settings.section.label()))
            .style(Style::default().fg(p.text).add_modifier(Modifier::BOLD)),
        layout.title,
    );
    if details && layout.title.width >= 52 {
        frame.render_widget(
            Paragraph::new("read only ").style(Style::default().fg(p.overlay1)),
            Rect::new(layout.title.right() - 10, layout.title.y, 10, 1),
        );
    }
    for (section, rect) in &layout.navigation {
        let style = if *section == app.settings.section {
            Style::default()
                .fg(panel_contrast_fg(p))
                .bg(p.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(p.overlay1)
        };
        let badge = if app.settings_section_has_badge(*section) {
            "●"
        } else {
            " "
        };
        let label = if layout.compact_navigation {
            section.compact_label()
        } else {
            section.label()
        };
        frame.render_widget(
            Paragraph::new(format!(" {badge}{label}")).style(style),
            *rect,
        );
    }
    let divider = if layout.compact_navigation {
        vec![Line::from("─".repeat(layout.divider.width as usize))]
    } else {
        vec![Line::from("│"); layout.divider.height as usize]
    };
    frame.render_widget(
        Paragraph::new(divider).style(Style::default().fg(p.surface0)),
        layout.divider,
    );
    let content_area = layout.content;

    match app.settings.section {
        SettingsSection::Sessions | SettingsSection::Summaries | SettingsSection::Titles => {
            render_setup_form(app, frame, content_area, layout.config.is_empty());
            if !layout.config.is_empty() {
                forms::config_footer(app, layout.config.width).render(frame, layout.config, 0);
            }
        }
        SettingsSection::Theme => {
            render_settings_theme(app, frame, content_area);
        }
        SettingsSection::Sound => {
            render_settings_toggle(
                frame,
                content_area,
                p,
                "sound alerts",
                "play sounds when agents change state in background",
                app.sound_enabled(),
                app.settings.list.selected,
            );
        }
        SettingsSection::Toast => {
            render_modal_choice_list(
                frame,
                content_area,
                "notification popups",
                "choose where background popup notifications should appear",
                &[
                    ("off", ToastDelivery::Off),
                    ("inside herduck", ToastDelivery::Herduck),
                    ("via terminal", ToastDelivery::Terminal),
                    ("via system", ToastDelivery::System),
                ],
                app.toast_delivery(),
                app.settings.list.selected,
                p,
                2,
            );
        }
        SettingsSection::PaneLabels => {
            render_settings_toggle(
                frame,
                content_area,
                p,
                "agent border labels",
                "show detected agent names in split pane borders",
                app.agent_border_labels_enabled(),
                app.settings.list.selected,
            );
        }
        SettingsSection::Experiments => {
            render_settings_experiments(app, frame, content_area);
        }
        SettingsSection::Integrations => {
            render_settings_integrations(app, frame, content_area);
        }
    }

    for button in settings_buttons(app, inner) {
        let style = if button.kind == SettingsButtonKind::Apply {
            Style::default().fg(panel_contrast_fg(p)).bg(p.accent)
        } else {
            Style::default().fg(p.text).bg(p.surface0)
        };
        render_action_button(
            frame,
            button.rect,
            button.hint,
            button.label,
            style.add_modifier(Modifier::BOLD),
        );
    }
    let status = if !app.settings.status.is_empty() {
        app.settings.status.as_str()
    } else if let Some(diagnostic) = &app.config_diagnostic {
        diagnostic.as_str()
    } else if app.session_setup.history_restart_required {
        "History saved; restart HERDUCK."
    } else {
        ""
    };
    let hint = match app.settings.section {
        SettingsSection::Sessions | SettingsSection::Summaries | SettingsSection::Titles => {
            "Enter opens config · ↑↓ scroll · Tab section · Esc close"
        }
        SettingsSection::Theme => "↑↓ preview · Enter save · Esc cancel · Tab section",
        SettingsSection::Integrations => "Install adds available integrations · Tab section",
        _ => "Click / Enter saves immediately · Tab section",
    };
    frame.render_widget(
        Paragraph::new(if status.is_empty() { hint } else { status })
            .style(Style::default().fg(if status.is_empty() {
                p.overlay1
            } else {
                p.yellow
            }))
            .wrap(ratatui::widgets::Wrap { trim: false }),
        layout.status,
    );
}

fn render_setup_form(app: &AppState, frame: &mut Frame, area: Rect, inline_config: bool) {
    if area.is_empty() {
        return;
    }
    let Some(form) = settings_form(app, area.width, inline_config) else {
        return;
    };
    let offset = form.scroll_offset(area.height, app.settings.scroll);
    let max_scroll = form.max_scroll(area.height);
    form.render(frame, area, offset);
    for (visible, row, marker) in [
        (offset > 0, area.y, "↑"),
        (offset < max_scroll, area.bottom() - 1, "↓"),
    ] {
        if visible {
            frame.render_widget(
                Paragraph::new(marker).style(Style::default().fg(app.palette.overlay1)),
                Rect::new(area.right(), row, 1, 1),
            );
        }
    }
}

fn integrations_footer_paragraph(app: &AppState) -> Paragraph<'static> {
    let p = &app.palette;
    let mut footer_lines = Vec::new();
    if !app.integration_install_messages.is_empty() {
        for message in &app.integration_install_messages {
            footer_lines.push(Line::from(Span::styled(
                format!(" {message}"),
                Style::default().fg(p.overlay1),
            )));
        }
    } else {
        let found_any = app.integration_recommendations.iter().any(|item| {
            item.available || item.state != crate::integration::IntegrationStatusKind::NotInstalled
        });
        let hint = if app
            .integration_recommendations
            .iter()
            .any(crate::integration::IntegrationRecommendation::needs_install)
        {
            " press install to add available or outdated integrations"
        } else if found_any {
            " all detected integrations are installed"
        } else {
            " no supported agent CLIs found on PATH"
        };
        footer_lines.push(Line::from(Span::styled(
            hint.to_string(),
            Style::default().fg(p.overlay1),
        )));
    }
    Paragraph::new(footer_lines).wrap(ratatui::widgets::Wrap { trim: false })
}

fn integrations_footer_height(app: &AppState, width: u16) -> u16 {
    (integrations_footer_paragraph(app).line_count(width) as u16).min(6)
}

fn render_settings_integrations(app: &AppState, frame: &mut Frame, area: Rect) {
    let p = &app.palette;

    let footer = integrations_footer_paragraph(app);
    let footer_height = integrations_footer_height(app, area.width);

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(footer_height),
    ])
    .areas::<6>(area);

    frame.render_widget(
        Paragraph::new("agent integrations")
            .style(Style::default().fg(p.text).add_modifier(Modifier::BOLD)),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(
            "let agents report state directly instead of relying only on process detection",
        )
        .style(Style::default().fg(p.overlay1))
        .wrap(ratatui::widgets::Wrap { trim: false }),
        rows[1],
    );

    let mut lines = Vec::new();
    for item in &app.integration_recommendations {
        let marker = match item.state {
            crate::integration::IntegrationStatusKind::Current => "✓",
            crate::integration::IntegrationStatusKind::Outdated => "↻",
            crate::integration::IntegrationStatusKind::NotInstalled if item.available => "+",
            crate::integration::IntegrationStatusKind::NotInstalled => "–",
        };
        let marker_style = match item.state {
            crate::integration::IntegrationStatusKind::Current => Style::default().fg(p.green),
            crate::integration::IntegrationStatusKind::Outdated => Style::default().fg(p.yellow),
            crate::integration::IntegrationStatusKind::NotInstalled if item.available => {
                Style::default().fg(p.accent)
            }
            crate::integration::IntegrationStatusKind::NotInstalled => {
                Style::default().fg(p.overlay0)
            }
        };
        lines.push(Line::from(vec![
            Span::styled(format!(" {marker} "), marker_style),
            Span::styled(
                format!("{:<9}", item.label),
                Style::default().fg(p.subtext0),
            ),
            Span::styled(item.status_label(), Style::default().fg(p.overlay1)),
        ]));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            " no integration targets available",
            Style::default().fg(p.overlay1),
        )));
    }

    frame.render_widget(Paragraph::new(lines), rows[3]);
    frame.render_widget(footer, rows[5]);
}

fn render_settings_theme(app: &AppState, frame: &mut Frame, area: Rect) {
    use crate::app::state::THEME_NAMES;

    let p = &app.palette;
    let items: Vec<ListItem> = THEME_NAMES
        .iter()
        .map(|name| {
            let is_current = name.to_lowercase().replace([' ', '_'], "-")
                == app.theme_name.to_lowercase().replace([' ', '_'], "-");
            let marker = if is_current { " ✓" } else { "" };
            ListItem::new(Line::from(vec![
                Span::styled(*name, Style::default().fg(p.subtext0)),
                Span::styled(marker, Style::default().fg(p.green)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(p.surface0)
                .fg(p.text)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" ▸ ")
        .style(Style::default().fg(p.subtext0));

    let mut state = ListState::default().with_selected(Some(app.settings.list.selected));
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_settings_toggle(
    frame: &mut Frame,
    area: Rect,
    p: &Palette,
    title: &str,
    description: &str,
    current_value: bool,
    selected_idx: usize,
) {
    render_modal_choice_list(
        frame,
        area,
        title,
        description,
        &[("on", true), ("off", false)],
        current_value,
        selected_idx,
        p,
        1,
    );
}

fn render_settings_experiments(app: &AppState, frame: &mut Frame, area: Rect) {
    let p = &app.palette;
    let [desc_area, _, list_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas::<3>(area);

    super::widgets::render_modal_description(
        frame,
        desc_area,
        "optional features that are off by default",
        Style::default().fg(p.overlay1),
    );

    for (idx, setting) in ExperimentSetting::ALL.iter().copied().enumerate() {
        let marker = if setting.enabled(app) { "[✓]" } else { "[ ]" };
        let style = if app.settings.list.selected == idx {
            Style::default()
                .bg(p.surface0)
                .fg(p.text)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(p.subtext0)
        };
        let row = Rect::new(list_area.x, list_area.y + idx as u16, list_area.width, 1);
        frame.render_widget(
            Paragraph::new(format!(" {} {marker}", setting.label())).style(style),
            row,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{state::SettingsSection, Mode};
    use ratatui::{backend::TestBackend, Terminal};

    fn render(app: &AppState, width: u16, height: u16) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| render_settings_overlay(app, frame, frame.area()))
            .unwrap();
        terminal
    }

    fn screen_text(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    fn form_text(app: &AppState) -> String {
        settings_form(app, 40, true)
            .unwrap()
            .lines
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn sources_display_order_and_full_details_without_editing_controls() {
        let mut app = AppState::test_new();
        app.settings.section = SettingsSection::Summaries;
        let config: crate::config::Config = toml::from_str(r#"
[projects.summary]
mode = "auto"
providers = [
    { id = "my-api", kind = "openai_compatible", endpoint = "https://example.invalid/a/long/目录/v1/chat/completions", models = ["first", "second"], api_key_env = "SUMMARY_TEST_KEY" },
    { id = "codex", kind = "cli", command = "/opt/agent tools/codex", models = [] },
]
"#).unwrap();
        app.summary_config = config.projects.summary;
        let text = form_text(&app);
        for detail in [
            "Ordered fallback",
            "1. my-api  API",
            "2. codex  Agent",
            "├─ first",
            "└─ second",
            "https://example.invalid/a/long/目录/v1/chat/completions",
            "$SUMMARY_TEST_KEY",
            "/opt/agent tools/codex",
            "Agent default",
            "/tmp/herduck/config.toml",
            "reopen Settings",
        ] {
            assert!(text.contains(detail), "missing {detail}: {text}");
        }
        assert!(text.find("1. my-api").unwrap() < text.find("2. codex").unwrap());
        for old_control in ["+ Add source", "Save to", "Choose model", "▏"] {
            assert!(!text.contains(old_control));
        }
        let rendered = screen_text(&render(&app, 110, 32));
        assert!(rendered.contains("open config file"));
        assert!(!rendered.contains("start session"));
    }

    #[test]
    fn naming_inheritance_is_distinct_from_an_explicit_local_only_list() {
        let mut app = AppState::test_new();
        app.settings.section = SettingsSection::Titles;
        let config: crate::config::Config = toml::from_str("[projects.summary]\nmode='auto'\nproviders=[{id='codex',kind='cli',models=['name-model']}]\n").unwrap();
        app.summary_config = config.projects.summary;
        let inherited = form_text(&app);
        assert!(inherited.contains("Same as Summaries"));
        assert!(inherited.contains("1. codex  Agent"));
        app.summary_config.title_providers = Some(vec![]);
        let local = form_text(&app);
        assert!(local.contains("Independent naming priorities"));
        assert!(local.contains("No model sources configured."));
        assert!(local.contains("Use local session text for names."));
        assert!(!local.contains("1. codex"));
    }

    #[test]
    fn disabled_sources_show_saved_details_without_claiming_they_are_active() {
        let mut app = AppState::test_new();
        app.settings.section = SettingsSection::Summaries;
        let initial = form_text(&app);
        assert!(initial.contains("Generation       Off"));
        assert!(initial.contains("No model sources configured."));
        assert!(!initial.contains("opencode_free"));
        app.summary_config.providers_explicit = true;
        app.summary_config.providers = vec![crate::config::SummaryProviderConfig::cli(
            "codex",
            &["saved-model"],
        )];
        let saved = form_text(&app);
        assert!(saved.contains("Sources · inactive"));
        assert!(saved.contains("No Agent or API calls in this mode."));
        assert!(saved.contains("Existing topics and session names stay unchanged."));
        assert!(saved.contains("saved-model"));
        app.summary_config.mode = crate::config::SummaryModeConfig::Local;
        let local = form_text(&app);
        assert!(local.contains("Sources · inactive"));
        assert!(local.contains("No Agent or API calls in this mode."));
        assert!(local.contains("saved-model"));
        assert!(local.contains("Names use local session text."));
        assert!(local.contains("Existing topics stay unchanged."));
    }

    #[test]
    fn session_paths_and_default_agent_scope_are_read_only_and_untruncated() {
        let mut app = AppState::test_new();
        app.settings.section = SettingsSection::Sessions;
        let active = crate::config::ProjectsConfig::default();
        let mut saved = active.clone();
        saved
            .adapters
            .codex
            .roots
            .push(std::path::PathBuf::from(format!(
                "/tmp/{}/newly-added-history",
                "long-path/".repeat(12)
            )));
        app.session_setup.set_history_locations(&saved, &active);
        let text = form_text(&app);
        for detail in [
            "Default Agent    shell",
            "Start session    shell",
            "New tab / split  Shell",
            "Resume           Original Agent",
            "newly-added-history",
            "restarting HERDUCK",
        ] {
            assert!(text.contains(detail), "missing {detail}: {text}");
        }
    }

    #[test]
    fn read_only_pages_keep_actions_and_scrolled_config_location_visible() {
        for (width, height) in [(32, 16), (40, 16), (80, 24), (110, 32)] {
            for section in [
                SettingsSection::Sessions,
                SettingsSection::Summaries,
                SettingsSection::Titles,
            ] {
                let mut app = AppState::test_new();
                app.settings.section = section;
                app.settings.scroll = u16::MAX;
                let text = screen_text(&render(&app, width, height));
                assert!(text.contains("config"), "{width}x{height}: {text}");
                assert!(text.contains("close"), "{width}x{height}: {text}");
                assert!(!text.contains("save setup"));
                assert!(
                    text.contains("changes."),
                    "end of details must be reachable: {text}"
                );
            }
        }
        let mut app = AppState::test_new();
        app.settings.section = SettingsSection::Summaries;
        app.session_setup.history_restart_required = true;
        assert!(screen_text(&render(&app, 40, 16)).contains("History saved; restart HERDUCK."));
        app.config_diagnostic = Some("config.toml invalid; keeping current config".into());
        assert!(screen_text(&render(&app, 110, 32)).contains("keeping current config"));
        app.settings.status = "Cannot open config: editor unavailable".into();
        assert!(screen_text(&render(&app, 110, 32)).contains("Cannot open config"));
    }

    #[test]
    fn config_footer_stays_visible_and_long_paths_fall_back_to_scrolling() {
        let mut app = AppState::test_new();
        app.settings.section = SettingsSection::Summaries;
        app.summary_config.providers_explicit = true;
        app.summary_config.providers = (0..8)
            .map(|index| {
                crate::config::SummaryProviderConfig::cli(
                    &format!("source-{index}"),
                    &["first", "second"],
                )
            })
            .collect();
        let inner = Rect::new(8, 2, 94, 28);
        let layout = settings_layout(&app, inner);
        assert!(!layout.config.is_empty());
        assert!(layout.content.bottom() < layout.config.y);
        assert!(layout.config.bottom() <= layout.status.y);
        for scroll in [0, u16::MAX] {
            app.settings.scroll = scroll;
            let text = screen_text(&render(&app, 110, 32));
            assert!(text.contains("/tmp/herduck/config.toml"));
            assert!(text.contains("reopen Settings to load changes."));
            assert!(text.contains("open config file"));
        }
        app.settings.config_path = std::path::PathBuf::from(format!(
            "/tmp/{}/settings-tail.toml",
            "很长的配置目录/".repeat(80)
        ));
        assert!(settings_layout(&app, inner).config.is_empty());
        for (width, height) in [(32, 16), (40, 16), (80, 24), (110, 32)] {
            app.settings.scroll = u16::MAX;
            let text = screen_text(&render(&app, width, height));
            assert!(
                text.contains("changes."),
                "footer remains reachable at {width}x{height}"
            );
            assert!(text.contains("close"));
        }
        let terminal = render(&app, 110, 32);
        let content = settings_layout(&app, inner).content;
        // The filename can cross a terminal row. Join only the content cells,
        // excluding navigation, borders and the indentation of continuation rows.
        let visible_content = (content.y..content.bottom())
            .map(|y| {
                (content.x..content.right())
                    .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                    .collect::<String>()
                    .trim()
                    .to_owned()
            })
            .collect::<String>();
        assert!(
            visible_content.contains("settings-tail.toml"),
            "{visible_content}"
        );
    }

    #[test]
    fn experiments_pane_history_uses_settings_checkmark_marker() {
        let mut app = AppState::test_new();
        app.pane_history_persistence = true;
        app.settings.section = SettingsSection::Experiments;
        app.settings.list.selected = 0;
        app.mode = Mode::Settings;

        let mut terminal =
            Terminal::new(TestBackend::new(80, 24)).expect("test terminal should initialize");
        terminal
            .draw(|frame| render_settings_overlay(&app, frame, Rect::new(0, 0, 80, 24)))
            .expect("settings overlay should render");

        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("pane screen history [✓]"));
        assert!(!rendered.contains("[x]"));
    }

    #[test]
    fn experiments_pane_history_keeps_empty_checkbox_marker_when_disabled() {
        let mut app = AppState::test_new();
        app.pane_history_persistence = false;
        app.settings.section = SettingsSection::Experiments;
        app.settings.list.selected = 0;
        app.mode = Mode::Settings;

        let mut terminal =
            Terminal::new(TestBackend::new(80, 24)).expect("test terminal should initialize");
        terminal
            .draw(|frame| render_settings_overlay(&app, frame, Rect::new(0, 0, 80, 24)))
            .expect("settings overlay should render");

        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("pane screen history [ ]"));
    }

    #[test]
    fn experiments_renders_switch_ascii_input_source_row() {
        let mut app = AppState::test_new();
        app.switch_ascii_input_source_in_prefix = true;
        app.settings.section = SettingsSection::Experiments;
        app.settings.list.selected = 1;
        app.mode = Mode::Settings;

        let mut terminal =
            Terminal::new(TestBackend::new(80, 24)).expect("test terminal should initialize");
        terminal
            .draw(|frame| render_settings_overlay(&app, frame, Rect::new(0, 0, 80, 24)))
            .expect("settings overlay should render");

        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("ASCII input in prefix mode (macOS) [✓]"));
    }
}
