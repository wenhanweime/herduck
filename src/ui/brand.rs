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

// Single-cell characters keep the sleepy face, hood and laptop aligned in every font.
const MASCOT: &[&str] = &[
    r"          .-======-.",
    r"       .-'          '-.",
    r"     .'   .--------.   '.",
    r"   .-\   /          \   /-.",
    r"  (___) |  ━━   ━━  | (___)",
    r"    |   |      ___..|____",
    r"    |    \    (_________>",
    r"   /      '._   _.'  ________",
    r"  (    .--.  '-'    /       /",
    r"   \  (    \______/  </>  /",
    r"    '.__\________/_______/",
];

const MINI: &[&str] = &[
    r"     .-====-.",
    r"  __/ .----. \__",
    r" (__) | ━ ━| (__)",
    r"    \  \___====>",
];

pub(super) fn render_mascot(frame: &mut Frame, area: Rect, p: &Palette, compact: bool) {
    let art = if compact { MINI } else { MASCOT };
    let width = art
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0) as u16;
    if area.width < width || area.height < art.len() as u16 {
        return;
    }
    let rect = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - art.len() as u16) / 2,
        width,
        art.len() as u16,
    );
    let lines: Vec<_> = art
        .iter()
        .enumerate()
        .map(|(i, line)| {
            // The bill is gold; the hood and laptop use the current theme's foreground.
            let bill = if compact && i == 3 {
                Some(11)
            } else if !compact && i == 6 {
                Some(14)
            } else {
                None
            };
            match bill {
                Some(start) => Line::from(vec![
                    Span::styled(&line[..start], Style::default().fg(p.text)),
                    Span::styled(&line[start..], Style::default().fg(p.yellow)),
                ]),
                None => Line::styled(*line, Style::default().fg(p.text)),
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), rect);
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
