//! Terminal-native brand art. Keep it in the client: never write it into a pane's PTY.
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::state::Palette;

pub(super) const TAGLINE: &str = "Make AI work for you.";

// Welcome art remains a text asset; the sidebar uses a real 32px PNG via Kitty.
const MASCOT: &str = include_str!("../../assets/brand/herduck-32-braille.txt");
const MINI: &str = include_str!("../../assets/brand/herduck-16-blocks.txt");
pub(super) const MASCOT_HEIGHT: u16 = 8;
const MENU_GAP: u16 = 2;
// A terminal cell is roughly twice as tall as it is wide: 2 columns ≈ 1 row.
const MENU_LEFT_INSET: u16 = 2;
const MENU_BOTTOM_INSET: u16 = 1;
const MIN_LABEL_WIDTH: u16 = 9;
const INLINE_LABEL_WIDTH: u16 = 16;
const NEW_BUTTON_SPACE: u16 = 6;
pub(crate) const MIN_FOOTER_ACTIONS_WIDTH: u16 = NEW_BUTTON_SPACE + 4;

fn mini_size() -> (u16, u16) {
    (4, 2)
}

/// Presentation-only placement, shared with the host graphics encoder.
pub(crate) fn sidebar_logo_rect(app: &crate::app::state::AppState) -> Option<Rect> {
    if !app.kitty_graphics_enabled
        || !matches!(
            app.mode,
            crate::app::Mode::Terminal | crate::app::Mode::Navigate | crate::app::Mode::GlobalMenu
        )
        || app.sidebar_collapsed
        || app.view.layout == crate::app::state::ViewLayout::Mobile
        || app.popup_pane.is_some()
    {
        return None;
    }
    let menu = app.global_launcher_rect();
    let (width, height) = mini_size();
    (menu.height >= height && menu.width >= width + MENU_GAP + 4)
        .then(|| Rect::new(menu.x, menu.bottom() - height, width, height))
}

/// Both list clipping and mouse hit testing use this footer, including the compact fallback.
pub(crate) fn brand_footer_rect(area: Rect) -> Rect {
    if area.is_empty() {
        return Rect::default();
    }
    let (icon_width, icon_height) = mini_size();
    // Keep room for navigation, both section headers, and list content in short windows.
    let height = if area.width >= MENU_LEFT_INSET + icon_width + MENU_GAP + 4
        && area.height >= icon_height + MENU_BOTTOM_INSET + 12
    {
        icon_height + MENU_BOTTOM_INSET
    } else {
        1
    };
    Rect::new(area.x, area.bottom() - height, area.width, height)
}

/// Reserve the brand and its visual inset, with room for the collapse toggle.
pub(crate) fn sidebar_brand_footer_rect(sidebar: Rect) -> Rect {
    brand_footer_rect(Rect::new(
        sidebar.x,
        sidebar.y,
        sidebar.width.saturating_sub(2),
        sidebar.height,
    ))
}

/// Both sidebar sections stop above the fixed brand footer.
pub(crate) fn sidebar_content_rect(sidebar: Rect) -> Rect {
    let footer = sidebar_brand_footer_rect(sidebar);
    Rect::new(
        sidebar.x,
        sidebar.y,
        sidebar.width.saturating_sub(1),
        sidebar.height.saturating_sub(footer.height),
    )
}

/// Only the new-workspace button stays under the workspace list.
pub(crate) fn workspace_footer_rect(area: Rect) -> Rect {
    if area.is_empty() {
        return Rect::default();
    }
    Rect::new(area.x, area.bottom() - 1, area.width, 1)
}

pub(crate) fn brand_menu_rect(footer: Rect, attention: bool) -> Rect {
    if footer.is_empty() {
        return Rect::default();
    }
    // Preserve the complete menu label in exceptionally narrow sidebars.
    let left_inset = if footer.width >= MENU_LEFT_INSET + 4 {
        MENU_LEFT_INSET
    } else {
        0
    };
    let available = footer.width.saturating_sub(left_inset);
    let (icon_width, icon_height) = mini_size();
    let icon_space = icon_width + MENU_GAP;
    let bottom_inset = if footer.height >= icon_height + MENU_BOTTOM_INSET {
        MENU_BOTTOM_INSET
    } else {
        0
    };
    let height = footer.height.saturating_sub(bottom_inset);
    let width = if height >= icon_height {
        if available >= icon_space + INLINE_LABEL_WIDTH {
            icon_space + INLINE_LABEL_WIDTH
        } else {
            icon_space + MIN_LABEL_WIDTH
        }
    } else if attention {
        17
    } else {
        15
    }
    .min(available);
    Rect::new(footer.x + left_inset, footer.y, width, height)
}

pub(super) fn render_mascot(frame: &mut Frame, area: Rect, p: &Palette, compact: bool) {
    let art = if compact { MINI } else { MASCOT };
    let width = art
        .lines()
        .map(super::text::display_width_u16)
        .max()
        .unwrap_or(0);
    let height = art.lines().count() as u16;
    if area.width < width || area.height < height {
        return;
    }
    let rect = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    let lines: Vec<_> = art
        .lines()
        .map(|line| Line::styled(line, Style::default().fg(p.text)))
        .collect();
    frame.render_widget(Paragraph::new(lines), rect);
}

pub(super) fn render_menu(frame: &mut Frame, area: Rect, p: &Palette, attention: bool) {
    let mut wordmark = Vec::new();
    if attention {
        wordmark.push(Span::styled(
            "● ",
            Style::default().fg(p.accent).add_modifier(Modifier::BOLD),
        ));
    }
    wordmark.extend([
        Span::styled("her", Style::default().fg(p.text)),
        Span::styled("duck", Style::default().fg(p.yellow)),
    ]);

    let (icon_width, icon_height) = mini_size();
    let icon_space = icon_width + MENU_GAP;
    if area.height >= icon_height && area.width >= icon_space + 4 {
        // The bitmap is emitted after the text frame, outside all agent PTYs.
        // Keep these cells empty; unsupported terminals still have the wordmark/menu.
        let inline = area.width >= icon_space + INLINE_LABEL_WIDTH;
        let label_height = if inline { 1 } else { 2 };
        let label = Rect::new(
            area.x + icon_space,
            area.bottom() - label_height,
            area.width.saturating_sub(icon_space),
            label_height,
        );
        let lines = if inline {
            wordmark.push(Span::styled(" · menu", Style::default().fg(p.overlay0)));
            vec![Line::from(wordmark)]
        } else if area.width < icon_space + MIN_LABEL_WIDTH {
            vec![
                Line::styled(
                    if attention { "●" } else { "" },
                    Style::default().fg(p.accent),
                ),
                Line::styled("menu", Style::default().fg(p.overlay0)),
            ]
        } else {
            vec![
                Line::from(wordmark),
                Line::styled("· menu", Style::default().fg(p.overlay0)),
            ]
        };
        frame.render_widget(Paragraph::new(lines), label);
    } else if area.width < if attention { 17 } else { 15 } {
        let mut menu = Vec::new();
        if attention && area.width >= 6 {
            menu.push(Span::styled("● ", Style::default().fg(p.accent)));
        }
        menu.push(Span::styled("menu", Style::default().fg(p.overlay1)));
        frame.render_widget(
            Paragraph::new(Line::from(menu)).alignment(Alignment::Left),
            area,
        );
    } else {
        wordmark.push(Span::styled(" · menu", Style::default().fg(p.overlay0)));
        frame.render_widget(
            Paragraph::new(Line::from(wordmark)).alignment(Alignment::Left),
            area,
        );
    }
}

pub(super) fn render_wordmark(frame: &mut Frame, area: Rect, p: &Palette) {
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("her", Style::default().fg(p.text)),
            Span::styled("duck", Style::default().fg(p.yellow)),
        ]))
        .style(Style::default().add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn compact_mark_has_balanced_insets_and_reserved_content_space() {
        assert_eq!(mini_size(), (4, 2));
        for width in [26, 34] {
            let sidebar = Rect::new(3, 2, width, 30);
            let footer = sidebar_brand_footer_rect(sidebar);
            let menu = brand_menu_rect(footer, false);
            assert_eq!(footer.height, 3, "reserve the mark and bottom inset");
            assert_eq!(menu.x - sidebar.x, 2);
            assert_eq!(sidebar.bottom() - menu.bottom(), 1);
            assert_eq!(menu.height, 2);
            let mut terminal = Terminal::new(TestBackend::new(width + 6, 34)).unwrap();
            terminal
                .draw(|frame| render_menu(frame, menu, &Palette::catppuccin(), false))
                .unwrap();
            let buffer = terminal.backend().buffer();
            for y in footer.y..footer.bottom() {
                for x in sidebar.x..menu.x {
                    assert_eq!(buffer[(x, y)].symbol(), " ");
                }
            }
            for x in sidebar.x..sidebar.right() {
                assert_eq!(buffer[(x, sidebar.bottom() - 1)].symbol(), " ");
            }
        }
    }

    #[test]
    fn menu_wordmark_clears_art_and_aligns_with_footer_bottom() {
        let (icon_width, icon_height) = mini_size();
        for attention in [false, true] {
            for label_width in [MIN_LABEL_WIDTH, INLINE_LABEL_WIDTH] {
                let width = icon_width + MENU_GAP + label_width;
                let area = Rect::new(2, 1, width, icon_height + 1);
                let mut terminal =
                    Terminal::new(TestBackend::new(width + 4, icon_height + 3)).unwrap();
                terminal
                    .draw(|frame| render_menu(frame, area, &Palette::catppuccin(), attention))
                    .unwrap();
                let buffer = terminal.backend().buffer();
                let label_y = area.bottom()
                    - if label_width == INLINE_LABEL_WIDTH {
                        1
                    } else {
                        2
                    };
                let label_x = area.x + icon_width + MENU_GAP;
                let wordmark_x = label_x + if attention { 2 } else { 0 };
                for (offset, ch) in "herduck".chars().enumerate() {
                    assert_eq!(
                        buffer[(wordmark_x + offset as u16, label_y)].symbol(),
                        ch.to_string()
                    );
                }
                for y in area.y..area.bottom() {
                    for x in area.x + icon_width..label_x {
                        assert_eq!(buffer[(x, y)].symbol(), " ");
                    }
                }
                for dy in 0..icon_height {
                    for dx in 0..icon_width {
                        assert_eq!(
                            buffer[(area.x + dx, area.bottom() - icon_height + dy)].symbol(),
                            " "
                        );
                    }
                }
            }
        }
    }
}
