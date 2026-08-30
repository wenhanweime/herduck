//! Renders an agent's Markdown output the way the agent's own CLI shows it.
//!
//! Transcripts store exactly what the agent emitted, which is Markdown: measured over 3,136 lines
//! of assistant output on this machine, inline code appears 749 times, bold 630, tables 182,
//! `##` headings 113, fenced blocks 112, and lists 206. Printing that verbatim shows the user a
//! screen of `##`, `**` and backticks instead of the structure those marks encode.
//!
//! This is presentation only: nothing here is persisted or sent over the API.
//!
//! Tables are deliberately passed through as their source lines. Aligning columns needs a width
//! budget the preview pane cannot promise, and a mis-aligned table reads worse than its source.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::app::state::Palette;

/// Indent applied per nesting level of a list, in columns.
const LIST_INDENT: usize = 2;

/// Renders one Markdown document into styled lines.
pub(crate) fn markdown_lines<'a>(body: &str, palette: &Palette) -> Vec<Line<'a>> {
    markdown_lines_with_base(body, palette, Style::default().fg(palette.text))
}

/// Render Markdown with a caller-selected prose colour.
///
/// History views use this for assistant turns so long stretches of ordinary agent output remain
/// visually distinct even when the text contains little Markdown syntax.
pub(crate) fn markdown_lines_with_base<'a>(
    body: &str,
    palette: &Palette,
    base_style: Style,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    let mut in_fence = false;

    for raw in body.lines() {
        let trimmed_end = raw.trim_end();
        let trimmed = trimmed_end.trim_start();

        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }

        if in_fence {
            // A gutter bar plus a background makes a code block scannable without a border, and
            // matches how release notes render fenced code.
            let code_style = Style::default().fg(palette.text).bg(palette.surface1);
            lines.push(Line::from(vec![
                Span::styled(
                    "▏",
                    Style::default().fg(palette.accent).bg(palette.surface1),
                ),
                Span::styled(format!(" {trimmed_end}"), code_style),
            ]));
            continue;
        }

        if trimmed.is_empty() {
            lines.push(Line::raw(""));
            continue;
        }

        if let Some(line) = heading_line(trimmed, palette, base_style) {
            lines.push(line);
            continue;
        }

        // Table rows keep their source form; see the module note.
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            lines.push(Line::from(Span::styled(
                trimmed_end.to_string(),
                Style::default().fg(palette.subtext0),
            )));
            continue;
        }

        if let Some(line) = list_line(trimmed_end, palette, base_style) {
            lines.push(line);
            continue;
        }

        lines.push(Line::from(inline_spans(trimmed_end, base_style, palette)));
    }

    lines
}

/// `#` through `######`, rendered as a weight ladder rather than literal hashes.
fn heading_line<'a>(trimmed: &str, palette: &Palette, base_style: Style) -> Option<Line<'a>> {
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    // A run of hashes with no space is not a heading — `#1` and `#!/bin/sh` are ordinary text.
    let rest = trimmed[hashes..].strip_prefix(' ')?.trim();
    if rest.is_empty() {
        return None;
    }

    let style = match hashes {
        1 | 2 => base_style.add_modifier(Modifier::BOLD),
        3 => Style::default()
            .fg(palette.accent)
            .add_modifier(Modifier::BOLD),
        _ => Style::default().fg(palette.subtext0).add_modifier(
            // Deeper headings stay distinguishable without competing with the top two levels.
            Modifier::BOLD,
        ),
    };
    // Strip inline marks inside headings so `## **Done**` does not render its asterisks.
    let mut spans = inline_spans(rest, style, palette);
    spans.insert(0, Span::raw(""));
    Some(Line::from(spans))
}

/// Bulleted and ordered list items, preserving nesting depth.
fn list_line<'a>(raw: &str, palette: &Palette, base_style: Style) -> Option<Line<'a>> {
    let indent_columns = raw.len() - raw.trim_start().len();
    let trimmed = raw.trim_start();
    let depth = indent_columns / LIST_INDENT;

    let (marker, rest) = if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
    {
        // Nested levels get a lighter glyph so depth is visible without counting spaces.
        let glyph = if depth == 0 { "•" } else { "▪" };
        (glyph.to_string(), rest)
    } else {
        let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let rest = trimmed[digits..].strip_prefix(". ")?;
        (format!("{}.", &trimmed[..digits]), rest)
    };

    let pad = " ".repeat(depth * LIST_INDENT + 1);
    let mut spans = vec![
        Span::raw(pad),
        Span::styled(format!("{marker} "), Style::default().fg(palette.accent)),
    ];
    spans.extend(inline_spans(rest, base_style, palette));
    Some(Line::from(spans))
}

/// Splits a line into `code`, **bold** and plain runs.
///
/// Written as a single left-to-right scan rather than nested passes so a backtick inside bold, or
/// an asterisk inside code, cannot be re-interpreted by a later pass.
fn inline_spans<'a>(text: &str, base: Style, palette: &Palette) -> Vec<Span<'a>> {
    let code_style = Style::default().fg(palette.accent).bg(palette.surface0);
    let bold_style = base.add_modifier(Modifier::BOLD);

    let mut spans = Vec::new();
    let mut plain = String::new();
    let mut rest = text;

    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("**") {
            if let Some(end) = after.find("**") {
                push_plain(&mut spans, &mut plain, base);
                let (bold, tail) = after.split_at(end);
                if !bold.is_empty() {
                    spans.push(Span::styled(bold.to_string(), bold_style));
                }
                rest = &tail[2..];
                continue;
            }
        }
        if let Some(after) = rest.strip_prefix('`') {
            if let Some(end) = after.find('`') {
                push_plain(&mut spans, &mut plain, base);
                let (code, tail) = after.split_at(end);
                if !code.is_empty() {
                    spans.push(Span::styled(code.to_string(), code_style));
                }
                rest = &tail[1..];
                continue;
            }
        }
        // Unmatched marker, or ordinary text: consume one character and keep scanning.
        let mut chars = rest.chars();
        if let Some(character) = chars.next() {
            plain.push(character);
        }
        rest = chars.as_str();
    }

    push_plain(&mut spans, &mut plain, base);
    if spans.is_empty() {
        spans.push(Span::styled(String::new(), base));
    }
    spans
}

fn push_plain<'a>(spans: &mut Vec<Span<'a>>, plain: &mut String, base: Style) {
    if !plain.is_empty() {
        spans.push(Span::styled(std::mem::take(plain), base));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> Palette {
        Palette::catppuccin()
    }

    fn text_of(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    #[test]
    fn headings_lose_their_hashes_and_gain_weight() {
        let lines = markdown_lines("## PAI 安全系统现状总览", &palette());
        assert_eq!(text_of(&lines[0]), "PAI 安全系统现状总览");
        assert!(lines[0]
            .spans
            .iter()
            .any(|span| span.style.add_modifier.contains(Modifier::BOLD)));
    }

    /// `#1` and shebangs are not headings.
    #[test]
    fn a_hash_without_a_space_is_ordinary_text() {
        let lines = markdown_lines("#1 是第一项", &palette());
        assert_eq!(text_of(&lines[0]), "#1 是第一项");
    }

    #[test]
    fn bold_and_inline_code_lose_their_markers() {
        let lines = markdown_lines("你说的 **PAI** 在 `~/.claude/` 下", &palette());
        let rendered = text_of(&lines[0]);
        assert!(!rendered.contains("**"), "{rendered}");
        assert!(!rendered.contains('`'), "{rendered}");
        assert!(rendered.contains("PAI"), "{rendered}");
        assert!(rendered.contains("~/.claude/"), "{rendered}");
    }

    /// An unmatched marker is literal text, not the start of a run that eats the rest of the line.
    #[test]
    fn an_unclosed_marker_stays_literal() {
        let lines = markdown_lines("成本 ** 上升，见 `config", &palette());
        assert_eq!(text_of(&lines[0]), "成本 ** 上升，见 `config");
    }

    /// A backtick inside bold must not be re-read as a code fence by a second pass.
    #[test]
    fn markers_do_not_nest_across_passes() {
        let lines = markdown_lines("**a`b**c", &palette());
        assert_eq!(text_of(&lines[0]), "a`bc");
    }

    #[test]
    fn nested_list_depth_is_preserved() {
        let lines = markdown_lines("- Base\n  - ARCHITECTURE.md\n", &palette());
        let top = text_of(&lines[0]);
        let nested = text_of(&lines[1]);
        assert!(top.contains('•'), "{top}");
        assert!(nested.contains('▪'), "{nested}");
        let top_indent = top.len() - top.trim_start().len();
        let nested_indent = nested.len() - nested.trim_start().len();
        assert!(
            nested_indent > top_indent,
            "nesting must stay visible: {top_indent} vs {nested_indent}"
        );
    }

    #[test]
    fn ordered_lists_keep_their_numbers() {
        let lines = markdown_lines("1. Behavioral anomaly detection", &palette());
        let rendered = text_of(&lines[0]);
        assert!(rendered.contains("1."), "{rendered}");
        assert!(rendered.contains("Behavioral"), "{rendered}");
    }

    #[test]
    fn fenced_code_is_backgrounded_and_the_fence_is_dropped() {
        let lines = markdown_lines("```sh\nls -la\n```", &palette());
        assert_eq!(lines.len(), 1, "fence markers must not render");
        assert!(text_of(&lines[0]).contains("ls -la"));
        assert!(lines[0]
            .spans
            .iter()
            .all(|span| span.style.bg == Some(palette().surface1)));
    }

    /// Markdown inside a fence is code, not markup.
    #[test]
    fn markers_inside_a_fence_are_left_alone() {
        let lines = markdown_lines("```\n## not a heading **not bold**\n```", &palette());
        assert!(text_of(&lines[0]).contains("## not a heading **not bold**"));
    }

    /// Aligning columns needs a width budget the pane cannot promise; the source reads better than
    /// a mis-aligned grid.
    #[test]
    fn table_rows_pass_through_unchanged() {
        let lines = markdown_lines("| 模块 | 状态 |", &palette());
        assert_eq!(text_of(&lines[0]), "| 模块 | 状态 |");
    }
}
