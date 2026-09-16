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

// The approved pixel outlines use single-cell Braille glyphs: 2 × 4 pixels per cell.
// Keep these in the theme's foreground so the glasses remain legible on light and dark terminals.
const MASCOT: &str = include_str!("../../assets/brand/herduck-32-braille.txt");
const MINI: &str = include_str!("../../assets/brand/herduck-16-braille.txt");
pub(super) const MASCOT_HEIGHT: u16 = 8;
const MENU_GAP: u16 = 2;
const MIN_LABEL_WIDTH: u16 = 9;
const INLINE_LABEL_WIDTH: u16 = 16;
const NEW_BUTTON_SPACE: u16 = 6;
pub(crate) const MIN_FOOTER_ACTIONS_WIDTH: u16 = NEW_BUTTON_SPACE + 4;

fn mini_size() -> (u16, u16) {
    (
        MINI.lines()
            .map(super::text::display_width_u16)
            .max()
            .unwrap_or(0),
        MINI.lines().count() as u16,
    )
}

/// Both list clipping and mouse hit testing use this footer, including the compact fallback.
pub(crate) fn brand_footer_rect(area: Rect) -> Rect {
    if area.is_empty() {
        return Rect::default();
    }
    let (icon_width, icon_height) = mini_size();
    // Leave two header rows and at least four content rows in the workspace section.
    let height = if area.width >= icon_width + MENU_GAP + MIN_LABEL_WIDTH + NEW_BUTTON_SPACE
        && area.height >= icon_height + 6
    {
        icon_height
    } else {
        1
    };
    Rect::new(area.x, area.bottom() - height, area.width, height)
}

pub(crate) fn brand_menu_rect(footer: Rect, attention: bool) -> Rect {
    if footer.is_empty() {
        return Rect::default();
    }
    // A very narrow sidebar prioritizes the menu; the separate new button is then hidden.
    let available = if footer.width >= MIN_FOOTER_ACTIONS_WIDTH {
        footer.width - NEW_BUTTON_SPACE
    } else {
        footer.width
    };
    let (icon_width, icon_height) = mini_size();
    let icon_space = icon_width + MENU_GAP;
    let width = if footer.height >= icon_height {
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
    Rect::new(footer.right() - width, footer.y, width, footer.height)
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
    if area.height >= icon_height && area.width >= icon_space + MIN_LABEL_WIDTH {
        render_mascot(
            frame,
            Rect::new(area.x, area.bottom() - icon_height, icon_width, icon_height),
            p,
            true,
        );
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
            Paragraph::new(Line::from(menu)).alignment(Alignment::Right),
            area,
        );
    } else {
        wordmark.push(Span::styled(" · menu", Style::default().fg(p.overlay0)));
        frame.render_widget(
            Paragraph::new(Line::from(wordmark)).alignment(Alignment::Right),
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
                for (dy, line) in MINI.lines().enumerate() {
                    for (dx, ch) in line.chars().enumerate() {
                        assert_eq!(
                            buffer[(area.x + dx as u16, area.bottom() - icon_height + dy as u16)]
                                .symbol(),
                            ch.to_string()
                        );
                    }
                }
            }
        }
    }
}
