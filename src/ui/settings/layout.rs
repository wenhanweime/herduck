//! One geometry model for settings rendering, navigation, and mouse actions.

use ratatui::layout::Rect;

use crate::app::{state::SettingsSection, AppState};
use crate::ui::widgets::{action_button_row_rects, ActionButtonSpec};

pub(crate) struct SettingsLayout {
    pub title: Rect,
    pub divider: Rect,
    pub navigation: Vec<(SettingsSection, Rect)>,
    pub compact_navigation: bool,
    pub content: Rect,
    pub config: Rect,
    pub status: Rect,
}

pub(crate) fn settings_layout(app: &AppState, inner: Rect) -> SettingsLayout {
    let status_height = 2.min(inner.height.saturating_sub(1));
    let status = Rect::new(
        inner.x,
        inner.bottom().saturating_sub(status_height + 1),
        inner.width,
        status_height,
    );
    let title = Rect::new(inner.x, inner.y, inner.width, inner.height.min(1));
    let vertical = inner.width >= 70 && inner.height >= 18;
    let mut config = Rect::default();
    if vertical
        && matches!(
            app.settings.section,
            SettingsSection::Sessions | SettingsSection::Summaries | SettingsSection::Titles
        )
    {
        let width = inner.width.saturating_sub(2);
        let height = super::forms::config_footer(app, width).height();
        let available = status.y.saturating_sub(inner.y + 2);
        // Keep all categories and at least twelve body rows visible. A long footer
        // falls back to the scrolled form, preserving the complete config path.
        if available >= height.saturating_add(13) {
            config = Rect::new(inner.x + 1, status.y - height, width, height);
        }
    }
    let content_bottom = if config.is_empty() {
        status.y
    } else {
        config.y
    };
    let mut navigation = Vec::new();
    let (divider, content) = if vertical {
        let top = inner.y + 2;
        let height = content_bottom.saturating_sub(top + 1);
        for (index, section) in SettingsSection::ALL.iter().copied().enumerate() {
            let gap = usize::from(index >= 2) + usize::from(index >= 5) + usize::from(index >= 7);
            navigation.push((
                section,
                Rect::new(inner.x + 1, top + (index + gap) as u16, 17, 1),
            ));
        }
        (
            Rect::new(inner.x + 18, top, 1, height),
            Rect::new(inner.x + 20, top, inner.width.saturating_sub(21), height),
        )
    } else {
        let mut x = inner.x;
        let mut y = inner.y + 1;
        for section in SettingsSection::ALL.iter().copied() {
            // Reserve badge space in every cell so an update cannot move other categories.
            let width = (section.compact_label().len() as u16 + 3).min(inner.width);
            if x > inner.x && x + width > inner.right() {
                x = inner.x;
                y += 1;
            }
            if y < status.y {
                navigation.push((section, Rect::new(x, y, width, 1)));
            }
            x += width + 1;
        }
        let top = (y + 2).min(status.y);
        (
            Rect::new(inner.x, y + 1, inner.width, u16::from(y + 1 < status.y)),
            Rect::new(inner.x, top, inner.width.saturating_sub(1), status.y - top),
        )
    };
    SettingsLayout {
        title,
        divider,
        navigation,
        compact_navigation: !vertical,
        content,
        config,
        status,
    }
}

pub(crate) fn settings_tab_rects(app: &AppState, inner: Rect) -> Vec<(SettingsSection, Rect)> {
    settings_layout(app, inner).navigation
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsButtonKind {
    Apply,
    Start,
    Close,
}

pub(crate) struct SettingsButton {
    pub kind: SettingsButtonKind,
    pub rect: Rect,
    pub label: &'static str,
    pub hint: Option<&'static str>,
}

pub(crate) fn settings_can_start_session(app: &AppState) -> bool {
    app.settings.section == SettingsSection::Sessions
}

pub(crate) fn settings_show_primary_action(app: &AppState) -> bool {
    match app.settings.section {
        SettingsSection::Theme => true,
        SettingsSection::Sessions | SettingsSection::Summaries | SettingsSection::Titles => true,
        SettingsSection::Integrations => app
            .integration_recommendations
            .iter()
            .any(crate::integration::IntegrationRecommendation::needs_install),
        _ => false,
    }
}

pub(crate) fn settings_buttons(app: &AppState, inner: Rect) -> Vec<SettingsButton> {
    let compact = inner.width < 60;
    let details = matches!(
        app.settings.section,
        SettingsSection::Sessions | SettingsSection::Summaries | SettingsSection::Titles
    );
    let mut actions = Vec::new();
    if settings_show_primary_action(app) {
        let label = if details {
            if compact && app.settings.section == SettingsSection::Sessions {
                "config"
            } else if compact {
                "open config"
            } else {
                "open config file"
            }
        } else if app.settings.section == SettingsSection::Integrations {
            "install"
        } else {
            "apply"
        };
        actions.push((SettingsButtonKind::Apply, (!compact).then_some("↵"), label));
    }
    if settings_can_start_session(app) {
        actions.push((
            SettingsButtonKind::Start,
            (!compact).then_some("ctrl+n"),
            if compact { "start" } else { "start session" },
        ));
    }
    actions.push((
        SettingsButtonKind::Close,
        (!compact).then_some("esc"),
        "close",
    ));
    let specs = actions
        .iter()
        .map(|(_, hint, label)| ActionButtonSpec { hint: *hint, label })
        .collect::<Vec<_>>();
    let mut rects = action_button_row_rects(inner, &specs, 1, inner.height.saturating_sub(1));
    if details && !compact {
        let right = rects.last().map_or(inner.right(), |rect| rect.right());
        let shift = inner.right().saturating_sub(right + 1);
        for rect in &mut rects {
            rect.x += shift;
        }
    }
    actions
        .into_iter()
        .zip(rects)
        .map(|((kind, hint, label), rect)| SettingsButton {
            kind,
            rect,
            label,
            hint,
        })
        .collect()
}

pub(crate) fn settings_button_rects(app: &AppState, inner: Rect) -> (Option<Rect>, Rect) {
    let buttons = settings_buttons(app, inner);
    (
        buttons
            .iter()
            .find(|button| button.kind == SettingsButtonKind::Apply)
            .map(|button| button.rect),
        buttons
            .iter()
            .find(|button| button.kind == SettingsButtonKind::Close)
            .map(|button| button.rect)
            .unwrap_or_default(),
    )
}

pub(crate) fn settings_new_session_rect(app: &AppState, inner: Rect) -> Option<Rect> {
    settings_buttons(app, inner)
        .into_iter()
        .find(|button| button.kind == SettingsButtonKind::Start)
        .map(|button| button.rect)
}
