use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::{
    app::{
        state::{AppState, ExperimentSetting, SettingsSection, THEME_NAMES},
        App, Mode,
    },
    config::ToastDelivery,
};

#[derive(Debug, Clone, PartialEq, Eq)]
// The shared `Save` verb is semantic: these actions persist settings.
#[allow(clippy::enum_variant_names)]
pub(super) enum SettingsAction {
    SaveTheme(String),
    OpenConfigFile,
    NewSession,
    SaveSound(bool),
    SaveToastDelivery(ToastDelivery),
    SaveAgentBorderLabels(bool),
    SavePaneHistory(bool),
    SaveSwitchAsciiInputSourceInPrefix(bool),
    InstallRecommendedIntegrations,
}

/// Map an Experiments row index to the toggle action that flips it.
fn experiment_toggle_action(state: &AppState, idx: usize) -> Option<SettingsAction> {
    match ExperimentSetting::ALL.get(idx).copied()? {
        ExperimentSetting::PaneHistory => Some(SettingsAction::SavePaneHistory(
            !ExperimentSetting::PaneHistory.enabled(state),
        )),
        ExperimentSetting::SwitchAsciiInputSourceInPrefix => {
            Some(SettingsAction::SaveSwitchAsciiInputSourceInPrefix(
                !ExperimentSetting::SwitchAsciiInputSourceInPrefix.enabled(state),
            ))
        }
    }
}

impl App {
    pub(crate) fn handle_settings_key(&mut self, key: KeyEvent) {
        let previous_section = self.state.settings.section;
        if let Some(action) = update_settings_state(&mut self.state, key) {
            match action {
                SettingsAction::OpenConfigFile => self.open_settings_config_file(),
                SettingsAction::NewSession => self.new_session_from_settings(),
                SettingsAction::SaveTheme(name) => self.save_theme(&name),
                SettingsAction::SaveSound(enabled) => self.save_sound(enabled),
                SettingsAction::SaveToastDelivery(delivery) => self.save_toast_delivery(delivery),
                SettingsAction::SaveAgentBorderLabels(enabled) => {
                    self.save_agent_border_labels(enabled)
                }
                SettingsAction::SavePaneHistory(enabled) => {
                    self.save_pane_history_persistence(enabled)
                }
                SettingsAction::SaveSwitchAsciiInputSourceInPrefix(enabled) => {
                    self.save_switch_ascii_input_source_in_prefix(enabled)
                }
                SettingsAction::InstallRecommendedIntegrations => {
                    self.install_recommended_integrations()
                }
            }
        }
        if previous_section != SettingsSection::Integrations
            && self.state.settings.section == SettingsSection::Integrations
        {
            self.refresh_integration_recommendations();
        }
        if previous_section != SettingsSection::Sessions
            && self.state.settings.section == SettingsSection::Sessions
        {
            self.refresh_session_setup();
        }
    }
}

fn normalize_theme_name(name: &str) -> String {
    name.to_lowercase().replace([' ', '_'], "-")
}

fn current_theme_index(theme_name: &str) -> usize {
    let normalized = normalize_theme_name(theme_name);
    THEME_NAMES
        .iter()
        .position(|name| normalize_theme_name(name) == normalized)
        .unwrap_or(0)
}

fn toast_delivery_index(delivery: ToastDelivery) -> usize {
    match delivery {
        ToastDelivery::Off => 0,
        ToastDelivery::Herduck => 1,
        ToastDelivery::Terminal => 2,
        ToastDelivery::System => 3,
    }
}

fn toast_delivery_for_index(idx: usize) -> ToastDelivery {
    match idx {
        0 => ToastDelivery::Off,
        1 => ToastDelivery::Herduck,
        2 => ToastDelivery::Terminal,
        _ => ToastDelivery::System,
    }
}

fn preview_selected_theme(state: &mut AppState) {
    use crate::app::state::Palette;

    let name = THEME_NAMES[state.settings.list.selected];
    if let Some(mut palette) = Palette::from_name(name) {
        if let Some(custom) = &state.theme_runtime.custom {
            palette = palette.with_overrides(custom);
        }
        if let Some(accent) = &state.theme_runtime.legacy_accent {
            palette.accent = crate::config::parse_color(accent);
        }
        state.palette = palette;
        state.theme_name = name.to_string();
    }
}

fn cancel_settings(state: &mut AppState) {
    if let Some(palette) = state.settings.original_palette.take() {
        state.palette = palette;
    }
    if let Some(theme_name) = state.settings.original_theme.take() {
        state.theme_name = theme_name;
    }
    super::modal::leave_modal(state);
}

fn integrations_need_install(state: &AppState) -> bool {
    state
        .integration_recommendations
        .iter()
        .any(crate::integration::IntegrationRecommendation::needs_install)
}

fn apply_settings(state: &mut AppState) -> Option<SettingsAction> {
    match state.settings.section {
        SettingsSection::Sessions | SettingsSection::Summaries | SettingsSection::Titles => {
            Some(SettingsAction::OpenConfigFile)
        }
        SettingsSection::Theme => {
            let theme_name = state.theme_name.clone();
            state.settings.original_palette = None;
            state.settings.original_theme = None;
            super::modal::leave_modal(state);
            Some(SettingsAction::SaveTheme(theme_name))
        }
        SettingsSection::Integrations if integrations_need_install(state) => {
            Some(SettingsAction::InstallRecommendedIntegrations)
        }
        SettingsSection::Integrations => None,
        _ => {
            super::modal::leave_modal(state);
            None
        }
    }
}

pub(crate) fn select_settings_section(state: &mut AppState, section: SettingsSection) {
    state.settings.section = section;
    state.settings.scroll = 0;
    state.settings.status.clear();
    state.settings.list.select(match section {
        SettingsSection::Theme => current_theme_index(&state.theme_name),
        SettingsSection::Sound => usize::from(!state.sound_enabled()),
        SettingsSection::Toast => toast_delivery_index(state.toast_delivery()),
        SettingsSection::PaneLabels => usize::from(!state.agent_border_labels_enabled()),
        _ => 0,
    });
}

fn cycle_settings_section(state: &mut AppState, forward: bool) {
    let sections = SettingsSection::ALL;
    let index = sections
        .iter()
        .position(|section| *section == state.settings.section)
        .unwrap_or(0);
    let next = if forward {
        (index + 1) % sections.len()
    } else {
        (index + sections.len() - 1) % sections.len()
    };
    select_settings_section(state, sections[next]);
}

pub(super) fn update_settings_state(state: &mut AppState, key: KeyEvent) -> Option<SettingsAction> {
    match state.settings.section {
        SettingsSection::Sessions | SettingsSection::Summaries | SettingsSection::Titles => {
            return update_config_details_key(state, key)
        }
        _ => {}
    }

    if key.code == KeyCode::Char('n')
        && key
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL)
    {
        return crate::ui::settings_can_start_session(state).then_some(SettingsAction::NewSession);
    }
    if let Some(super::modal::ModalAction::Close) =
        super::modal::modal_action_from_key(&key, super::modal::SETTINGS_ACTIONS)
    {
        cancel_settings(state);
        return None;
    }
    match key.code {
        KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => cycle_settings_section(state, true),
        KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
            cycle_settings_section(state, false)
        }
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Down | KeyCode::Char('j') => {
            let down = matches!(key.code, KeyCode::Down | KeyCode::Char('j'));
            match state.settings.section {
                SettingsSection::Sound | SettingsSection::PaneLabels => {
                    state.settings.list.selected = 1 - state.settings.list.selected.min(1);
                }
                SettingsSection::Theme | SettingsSection::Toast | SettingsSection::Experiments => {
                    let count = match state.settings.section {
                        SettingsSection::Theme => THEME_NAMES.len(),
                        SettingsSection::Toast => 4,
                        _ => ExperimentSetting::ALL.len(),
                    };
                    let previous = state.settings.list.selected;
                    if down {
                        state.settings.list.move_next(count);
                    } else {
                        state.settings.list.move_prev();
                    }
                    if state.settings.section == SettingsSection::Theme
                        && state.settings.list.selected != previous
                    {
                        preview_selected_theme(state);
                    }
                }
                _ => {}
            }
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            return match state.settings.section {
                SettingsSection::Sound => {
                    Some(SettingsAction::SaveSound(state.settings.list.selected == 0))
                }
                SettingsSection::Toast => Some(SettingsAction::SaveToastDelivery(
                    toast_delivery_for_index(state.settings.list.selected),
                )),
                SettingsSection::PaneLabels => Some(SettingsAction::SaveAgentBorderLabels(
                    state.settings.list.selected == 0,
                )),
                SettingsSection::Experiments => {
                    experiment_toggle_action(state, state.settings.list.selected)
                }
                SettingsSection::Integrations => apply_settings(state),
                SettingsSection::Theme if key.code == KeyCode::Enter => apply_settings(state),
                _ => None,
            }
        }
        _ => {}
    }
    None
}

fn update_config_details_key(state: &mut AppState, key: KeyEvent) -> Option<SettingsAction> {
    let ctrl = key
        .modifiers
        .contains(crossterm::event::KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => cancel_settings(state),
        KeyCode::Enter => return Some(SettingsAction::OpenConfigFile),
        KeyCode::Char('n' | 'N') if ctrl => {
            return crate::ui::settings_can_start_session(state)
                .then_some(SettingsAction::NewSession)
        }
        KeyCode::Left | KeyCode::BackTab => cycle_settings_section(state, false),
        KeyCode::Right | KeyCode::Tab => cycle_settings_section(state, true),
        KeyCode::Up
        | KeyCode::Down
        | KeyCode::PageUp
        | KeyCode::PageDown
        | KeyCode::Home
        | KeyCode::End => {
            let area = state.settings_content_rect();
            if let Some(form) = crate::ui::settings_form(state, area.width) {
                let max = form.max_scroll(area.height);
                let scroll = state.settings.scroll.min(max);
                state.settings.scroll = match key.code {
                    KeyCode::Up => scroll.saturating_sub(1),
                    KeyCode::Down => scroll.saturating_add(1).min(max),
                    KeyCode::PageUp => scroll.saturating_sub(area.height.max(1)),
                    KeyCode::PageDown => scroll.saturating_add(area.height.max(1)).min(max),
                    KeyCode::Home => 0,
                    _ => max,
                };
            }
        }
        _ => {}
    }
    None
}

pub(crate) fn open_settings(state: &mut AppState) {
    open_settings_at(state, SettingsSection::Theme);
}

pub(crate) fn open_settings_at(state: &mut AppState, section: SettingsSection) {
    state.request_reload_config = true;
    state.integration_install_messages.clear();
    state.settings.original_palette = Some(state.palette.clone());
    state.settings.original_theme = Some(state.theme_name.clone());
    select_settings_section(state, section);
    state.mode = Mode::Settings;
}

impl AppState {
    fn settings_popup_rect(&self) -> Rect {
        crate::ui::centered_popup_rect(
            self.screen_rect(),
            crate::ui::SETTINGS_POPUP_WIDTH,
            crate::ui::settings_popup_height(self),
        )
        .unwrap_or_default()
    }

    fn settings_inner_rect(&self) -> Rect {
        let popup = self.settings_popup_rect();
        Rect::new(
            popup.x + 1,
            popup.y + 1,
            popup.width.saturating_sub(2),
            popup.height.saturating_sub(2),
        )
    }

    fn settings_tab_at(&self, col: u16, row: u16) -> Option<SettingsSection> {
        let inner = self.settings_inner_rect();
        crate::ui::settings_tab_rects(self, inner)
            .into_iter()
            .find_map(|(section, rect)| rect.contains((col, row).into()).then_some(section))
    }

    pub(crate) fn settings_content_rect(&self) -> Rect {
        let inner = self.settings_inner_rect();
        crate::ui::settings_layout(inner).content
    }

    fn settings_list_index_at(&self, col: u16, row: u16) -> Option<usize> {
        let area = self.settings_content_rect();
        if row < area.y || row >= area.y + area.height || col < area.x || col >= area.x + area.width
        {
            return None;
        }

        match self.settings.section {
            SettingsSection::Theme => {
                let max_visible = area.height as usize;
                let scroll = if self.settings.list.selected >= max_visible {
                    self.settings.list.selected - max_visible + 1
                } else {
                    0
                };
                let idx = scroll + (row - area.y) as usize;
                (idx < THEME_NAMES.len()).then_some(idx)
            }
            SettingsSection::Sound => {
                let list_y = area.y + 3;
                if row >= list_y && row < list_y + 2 {
                    Some((row - list_y) as usize)
                } else {
                    None
                }
            }
            SettingsSection::Toast => {
                let list_y = area.y + 3;
                if row >= list_y && row < list_y + 8 {
                    Some(((row - list_y) / 2) as usize)
                } else {
                    None
                }
            }
            SettingsSection::PaneLabels => {
                let list_y = area.y + 3;
                if row >= list_y && row < list_y + 2 {
                    Some((row - list_y) as usize)
                } else {
                    None
                }
            }
            SettingsSection::Experiments => {
                let list_y = area.y + 3;
                if row >= list_y && row < list_y + ExperimentSetting::ALL.len() as u16 {
                    Some((row - list_y) as usize)
                } else {
                    None
                }
            }
            SettingsSection::Integrations
            | SettingsSection::Titles
            | SettingsSection::Summaries
            | SettingsSection::Sessions => None,
        }
    }

    pub(super) fn handle_settings_mouse(&mut self, mouse: MouseEvent) -> Option<SettingsAction> {
        match mouse.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let area = self.settings_content_rect();
                if area.contains((mouse.column, mouse.row).into()) {
                    let down = mouse.kind == MouseEventKind::ScrollDown;
                    if let Some(form) = crate::ui::settings_form(self, area.width) {
                        let scroll = form.scroll_offset(area.height, self.settings.scroll);
                        self.settings.scroll = if down {
                            scroll.saturating_add(1).min(form.max_scroll(area.height))
                        } else {
                            scroll.saturating_sub(1)
                        };
                    }
                }
                None
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(section) = self.settings_tab_at(mouse.column, mouse.row) {
                    select_settings_section(self, section);
                    return None;
                }
                let area = self.settings_content_rect();
                if crate::ui::settings_form(self, area.width).is_some()
                    && area.contains((mouse.column, mouse.row).into())
                {
                    return None;
                }
                if let Some(idx) = self.settings_list_index_at(mouse.column, mouse.row) {
                    self.settings.list.select(idx);
                    return match self.settings.section {
                        SettingsSection::Theme => {
                            preview_selected_theme(self);
                            None
                        }
                        SettingsSection::Sound => {
                            let enabled = idx == 0;
                            Some(SettingsAction::SaveSound(enabled))
                        }
                        SettingsSection::Toast => {
                            let delivery = toast_delivery_for_index(idx);
                            Some(SettingsAction::SaveToastDelivery(delivery))
                        }
                        SettingsSection::PaneLabels => {
                            let enabled = idx == 0;
                            Some(SettingsAction::SaveAgentBorderLabels(enabled))
                        }
                        SettingsSection::Experiments => experiment_toggle_action(self, idx),
                        SettingsSection::Integrations
                        | SettingsSection::Titles
                        | SettingsSection::Summaries
                        | SettingsSection::Sessions => None,
                    };
                }

                let inner = self.settings_inner_rect();
                if crate::ui::settings_new_session_rect(self, inner)
                    .is_some_and(|rect| rect.contains((mouse.column, mouse.row).into()))
                {
                    return Some(SettingsAction::NewSession);
                }
                let (apply, close) = crate::ui::settings_button_rects(self, inner);
                let mut buttons = vec![(close, super::modal::ModalAction::Close)];
                if let Some(apply) = apply {
                    buttons.insert(0, (apply, super::modal::ModalAction::Apply));
                }
                match super::modal::modal_action_from_buttons(mouse.column, mouse.row, &buttons) {
                    Some(super::modal::ModalAction::Apply) => apply_settings(self),
                    Some(super::modal::ModalAction::Close) => {
                        cancel_settings(self);
                        None
                    }
                    _ => {
                        if !self
                            .settings_popup_rect()
                            .contains((mouse.column, mouse.row).into())
                        {
                            cancel_settings(self);
                        }
                        None
                    }
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEventKind};

    use super::super::{app_for_mouse_test, mouse, state_with_workspaces};
    use super::*;
    #[test]
    fn configuration_details_cannot_be_edited_with_keys_or_clicks() {
        let mut state = state_with_workspaces(&["test"]);
        let before = state.summary_config.clone();
        let agent = state.session_setup.agent.clone();
        for section in [
            SettingsSection::Sessions,
            SettingsSection::Summaries,
            SettingsSection::Titles,
        ] {
            open_settings_at(&mut state, section);
            crate::ui::compute_view(&mut state, Rect::new(0, 0, 80, 24));
            for key in [
                KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
                KeyEvent::new(KeyCode::Char('+'), KeyModifiers::NONE),
                KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
                KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
                KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
                KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE),
                KeyEvent::new(KeyCode::Up, KeyModifiers::ALT),
            ] {
                assert_eq!(update_settings_state(&mut state, key), None);
            }
            let area = state.settings_content_rect();
            for row in area.y..area.bottom() {
                assert_eq!(
                    state.handle_settings_mouse(mouse(
                        MouseEventKind::Down(MouseButton::Left),
                        area.x + 1,
                        row
                    )),
                    None
                );
            }
            assert_eq!(state.summary_config, before);
            assert_eq!(state.session_setup.agent, agent);
            assert_eq!(state.mode, Mode::Settings);
            assert_eq!(
                update_settings_state(
                    &mut state,
                    KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
                ),
                Some(SettingsAction::OpenConfigFile)
            );
            assert_eq!(
                update_settings_state(
                    &mut state,
                    KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL)
                ),
                (section == SettingsSection::Sessions).then_some(SettingsAction::NewSession)
            );
        }
    }

    #[test]
    fn settings_visible_tabs_have_matching_bounded_mouse_targets() {
        let mut state = state_with_workspaces(&["test"]);
        for (width, height) in [(32, 16), (40, 16), (80, 24), (110, 32)] {
            for section in SettingsSection::ALL {
                open_settings_at(&mut state, *section);
                crate::ui::compute_view(&mut state, Rect::new(0, 0, width, height));
                let inner = state.settings_inner_rect();
                let tabs = crate::ui::settings_tab_rects(&state, inner);
                assert_eq!(tabs.len(), SettingsSection::ALL.len());
                for (visible, rect) in tabs {
                    assert!(rect.right() <= inner.right());
                    assert_eq!(state.settings_tab_at(rect.x, rect.y), Some(visible));
                }
            }
        }
    }

    #[test]
    fn config_open_start_and_close_buttons_stay_separate_in_narrow_terminals() {
        let mut state = state_with_workspaces(&["test"]);
        for (width, height) in [(32, 16), (40, 16), (60, 16), (80, 24), (110, 32)] {
            for section in [
                SettingsSection::Sessions,
                SettingsSection::Summaries,
                SettingsSection::Titles,
            ] {
                open_settings_at(&mut state, section);
                crate::ui::compute_view(&mut state, Rect::new(0, 0, width, height));
                let inner = state.settings_inner_rect();
                let (open, close) = crate::ui::settings_button_rects(&state, inner);
                let start = crate::ui::settings_new_session_rect(&state, inner);
                assert_eq!(start.is_some(), section == SettingsSection::Sessions);
                let mut buttons = vec![(open.unwrap(), Some(SettingsAction::OpenConfigFile))];
                if let Some(start) = start {
                    buttons.push((start, Some(SettingsAction::NewSession)));
                }
                buttons.push((close, None));
                for pair in buttons.windows(2) {
                    assert!(pair[0].0.right() < pair[1].0.x);
                }
                for (rect, action) in buttons {
                    assert!(rect.width > 0 && rect.height == 1);
                    assert!(inner.contains((rect.x, rect.y).into()));
                    assert!(rect.right() <= inner.right());
                    assert_eq!(
                        state.handle_settings_mouse(mouse(
                            MouseEventKind::Down(MouseButton::Left),
                            rect.x,
                            rect.y
                        )),
                        action
                    );
                }
                assert_ne!(state.mode, Mode::Settings);
            }
        }
    }

    #[test]
    fn configuration_details_scroll_without_changing_priorities() {
        let mut state = state_with_workspaces(&["test"]);
        state.summary_config.providers_explicit = true;
        state.summary_config.providers = (0..8)
            .map(|_| {
                crate::config::SummaryProviderConfig::cli("codex", &["model-one", "model-two"])
            })
            .collect();
        let before = state.summary_config.clone();
        for (width, height) in [(32, 16), (40, 16), (80, 24), (110, 32)] {
            open_settings_at(&mut state, SettingsSection::Summaries);
            crate::ui::compute_view(&mut state, Rect::new(0, 0, width, height));
            let area = state.settings_content_rect();
            let max = crate::ui::settings_form(&state, area.width)
                .unwrap()
                .max_scroll(area.height);
            assert!(max > 0);
            update_settings_state(&mut state, KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
            assert_eq!(state.settings.scroll, max);
            state.handle_settings_mouse(mouse(MouseEventKind::ScrollUp, area.x, area.y));
            assert_eq!(state.settings.scroll, max - 1);
            update_settings_state(&mut state, KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
            assert_eq!(state.settings.scroll, 0);
            update_settings_state(
                &mut state,
                KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
            );
            assert_eq!(state.settings.scroll, area.height.min(max));
            update_settings_state(&mut state, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
            assert_eq!(state.settings.section, SettingsSection::Titles);
            assert_eq!(state.settings.scroll, 0);
            assert_eq!(state.summary_config, before);
        }
    }

    #[test]
    fn settings_cancel_restores_previewed_theme_from_other_sections() {
        let mut state = state_with_workspaces(&["test"]);
        let original_palette = state.palette.clone();
        let original_theme = state.theme_name.clone();

        open_settings(&mut state);
        update_settings_state(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::empty()),
        );
        assert_ne!(state.theme_name, original_theme);

        update_settings_state(
            &mut state,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()),
        );
        assert_eq!(
            state.settings.section,
            crate::app::state::SettingsSection::PaneLabels
        );

        update_settings_state(
            &mut state,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
        );

        assert_eq!(state.mode, Mode::Terminal);
        assert_eq!(state.theme_name, original_theme);
        assert_eq!(state.palette.accent, original_palette.accent);
        assert_eq!(state.palette.panel_bg, original_palette.panel_bg);
    }

    #[test]
    fn settings_sound_toggle_returns_save_action() {
        let mut state = state_with_workspaces(&["test"]);
        open_settings(&mut state);
        state.settings.section = crate::app::state::SettingsSection::Sound;
        state.settings.list.selected = 0;

        let action = update_settings_state(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        );

        assert_eq!(action, Some(SettingsAction::SaveSound(true)));
        assert!(!state.sound.enabled);
        assert_eq!(state.mode, Mode::Settings);
    }

    #[test]
    fn settings_experiments_toggles_pane_history() {
        let mut state = state_with_workspaces(&["test"]);
        state.pane_history_persistence = false;
        open_settings_at(&mut state, SettingsSection::Experiments);

        let action = update_settings_state(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        );

        assert_eq!(action, Some(SettingsAction::SavePaneHistory(true)));
        assert_eq!(state.mode, Mode::Settings);
    }

    #[test]
    fn settings_experiments_down_then_toggle_switches_ascii_input_source() {
        let mut state = state_with_workspaces(&["test"]);
        state.switch_ascii_input_source_in_prefix = false;
        open_settings_at(&mut state, SettingsSection::Experiments);

        update_settings_state(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::empty()),
        );
        assert_eq!(state.settings.list.selected, 1);

        let action = update_settings_state(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        );

        assert_eq!(
            action,
            Some(SettingsAction::SaveSwitchAsciiInputSourceInPrefix(true))
        );
        assert_eq!(state.mode, Mode::Settings);
    }

    #[test]
    fn settings_tab_cycle_places_experiments_last() {
        let mut state = state_with_workspaces(&["test"]);
        open_settings_at(&mut state, SettingsSection::Toast);

        update_settings_state(
            &mut state,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()),
        );
        assert_eq!(state.settings.section, SettingsSection::Integrations);

        update_settings_state(
            &mut state,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()),
        );
        assert_eq!(state.settings.section, SettingsSection::Experiments);

        update_settings_state(
            &mut state,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()),
        );
        assert_eq!(state.settings.section, SettingsSection::Theme);

        update_settings_state(
            &mut state,
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::empty()),
        );
        assert_eq!(state.settings.section, SettingsSection::Experiments);

        update_settings_state(
            &mut state,
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::empty()),
        );
        assert_eq!(state.settings.section, SettingsSection::Integrations);

        update_settings_state(
            &mut state,
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::empty()),
        );
        assert_eq!(state.settings.section, SettingsSection::Toast);
    }

    #[test]
    fn integrations_enter_does_nothing_when_nothing_needs_install() {
        let mut state = state_with_workspaces(&["test"]);
        open_settings_at(&mut state, SettingsSection::Integrations);

        let enter_action = update_settings_state(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        );
        assert_eq!(enter_action, None);

        let space_action = update_settings_state(
            &mut state,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty()),
        );
        assert_eq!(space_action, None);
    }

    #[test]
    fn settings_hover_does_not_change_selection() {
        let mut app = app_for_mouse_test();
        open_settings(&mut app.state);
        app.state.settings.list.select(0);

        let area = app.state.settings_content_rect();
        app.handle_mouse(mouse(MouseEventKind::Moved, area.x + 2, area.y + 2));

        assert_eq!(app.state.settings.list.selected, 0);
    }

    #[test]
    fn settings_mouse_click_toggles_pane_history() {
        let mut app = app_for_mouse_test();
        app.state.pane_history_persistence = false;
        open_settings_at(&mut app.state, SettingsSection::Experiments);

        let area = app.state.settings_content_rect();
        let action = app.state.handle_settings_mouse(mouse(
            MouseEventKind::Down(crossterm::event::MouseButton::Left),
            area.x + 2,
            area.y + 3,
        ));

        assert_eq!(action, Some(SettingsAction::SavePaneHistory(true)));
        assert_eq!(app.state.settings.list.selected, 0);
    }

    #[test]
    fn settings_mouse_click_toggles_switch_ascii_input_source_row() {
        let mut app = app_for_mouse_test();
        app.state.switch_ascii_input_source_in_prefix = false;
        open_settings_at(&mut app.state, SettingsSection::Experiments);

        let area = app.state.settings_content_rect();
        let action = app.state.handle_settings_mouse(mouse(
            MouseEventKind::Down(crossterm::event::MouseButton::Left),
            area.x + 2,
            area.y + 4,
        ));

        assert_eq!(
            action,
            Some(SettingsAction::SaveSwitchAsciiInputSourceInPrefix(true))
        );
        assert_eq!(app.state.settings.list.selected, 1);
    }

    #[test]
    fn integration_update_badge_only_tracks_outdated_recommendations() {
        let mut state = state_with_workspaces(&["test"]);
        state.integration_recommendations = vec![integration_recommendation(
            crate::integration::IntegrationStatusKind::NotInstalled,
            true,
        )];
        assert!(!state.integration_updates_available());

        state.integration_recommendations = vec![integration_recommendation(
            crate::integration::IntegrationStatusKind::NotInstalled,
            false,
        )];
        assert!(!state.integration_updates_available());

        state.integration_recommendations = vec![integration_recommendation(
            crate::integration::IntegrationStatusKind::Current,
            true,
        )];
        assert!(!state.integration_updates_available());

        state.integration_recommendations = vec![integration_recommendation(
            crate::integration::IntegrationStatusKind::Outdated,
            true,
        )];
        assert!(state.integration_updates_available());
    }

    #[test]
    fn settings_tab_hit_area_includes_integration_update_badge() {
        let mut state = state_with_workspaces(&["test"]);
        state.integration_recommendations = vec![integration_recommendation(
            crate::integration::IntegrationStatusKind::Outdated,
            true,
        )];
        open_settings_at(&mut state, SettingsSection::Integrations);
        crate::ui::compute_view(&mut state, Rect::new(0, 0, 110, 32));
        let rect = crate::ui::settings_tab_rects(&state, state.settings_inner_rect())
            .into_iter()
            .find(|(section, _)| *section == SettingsSection::Integrations)
            .unwrap()
            .1;
        assert!(rect.width >= SettingsSection::Integrations.label().len() as u16 + 2);
        assert_eq!(
            state.settings_tab_at(rect.right() - 1, rect.y),
            Some(SettingsSection::Integrations)
        );
    }

    fn integration_recommendation(
        state: crate::integration::IntegrationStatusKind,
        available: bool,
    ) -> crate::integration::IntegrationRecommendation {
        crate::integration::IntegrationRecommendation {
            target: crate::api::schema::IntegrationTarget::Claude,
            label: "claude",
            command: "claude",
            available,
            path: std::path::PathBuf::from("/tmp/herdr-test-integration"),
            state,
        }
    }
}
