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
const MINI_WIDTH: u16 = 8;
const MINI_HEIGHT: u16 = 4;
const MENU_GAP: u16 = 2;
const STACKED_MENU_WIDTH: u16 = MINI_WIDTH + MENU_GAP + 9;
const INLINE_MENU_WIDTH: u16 = MINI_WIDTH + MENU_GAP + 16;
const NEW_BUTTON_SPACE: u16 = 6;
pub(crate) const MIN_FOOTER_ACTIONS_WIDTH: u16 = NEW_BUTTON_SPACE + 4;

/// Both list clipping and mouse hit testing use this footer, including the compact fallback.
pub(crate) fn brand_footer_rect(area: Rect) -> Rect {
    if area.is_empty() {
        return Rect::default();
    }
    // Leave two header rows and at least four content rows in the workspace section.
    let height = if area.width >= STACKED_MENU_WIDTH + NEW_BUTTON_SPACE && area.height >= 10 {
        MINI_HEIGHT
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
    let width = if footer.height >= MINI_HEIGHT {
        if footer.width.saturating_sub(NEW_BUTTON_SPACE) >= INLINE_MENU_WIDTH {
            INLINE_MENU_WIDTH
        } else {
            STACKED_MENU_WIDTH
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

    if area.height >= MINI_HEIGHT && area.width >= STACKED_MENU_WIDTH {
        render_mascot(
            frame,
            Rect::new(area.x, area.y, MINI_WIDTH, MINI_HEIGHT),
            p,
            true,
        );
        let label = Rect::new(
            area.x + MINI_WIDTH + MENU_GAP,
            area.y + 1,
            area.width - MINI_WIDTH - MENU_GAP,
            2,
        );
        let lines = if area.width >= INLINE_MENU_WIDTH {
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
