use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::app::state::{AppState, ProjectOverviewAction, ProjectOverviewHit};
use crate::config::TitleLanguage;
use crate::projects::followup::FollowupState;
use crate::projects::overview::{
    AdvanceSuggestion, ProjectOverview, ProjectWorkItem, SuggestionSource, WorkPhase,
};

use super::widgets::{panel_contrast_fg, render_action_button};

struct SuggestionArea {
    index: usize,
    body: Rect,
    primary: Rect,
    secondary: Rect,
}

#[derive(Default)]
struct SessionLayout {
    heading: Rect,
    progress: Rect,
    suggestions: Vec<SuggestionArea>,
    view: Rect,
}

fn action_label(suggestion: &AdvanceSuggestion, language: TitleLanguage) -> &'static str {
    let zh = language == TitleLanguage::Chinese;
    if let Some(followup) = &suggestion.followup {
        return match (followup.state, zh) {
            (FollowupState::Queued, true) => "取消待发送的跟进",
            (FollowupState::Queued, false) => "Cancel queued follow-up",
            (FollowupState::Sent, true) => "已交给 Agent · 查看",
            (FollowupState::Sent, false) => "Sent to Agent · view",
            (FollowupState::Cancelled, true) => "已取消 · 查看会话",
            (FollowupState::Cancelled, false) => "Cancelled · view session",
        };
    }
    if suggestion.prompt.is_none() && suggestion.source == SuggestionSource::Conversation {
        return language.text("View conversation →", "查看会话 →");
    }
    match (
        suggestion.prompt.is_some(),
        suggestion.session_key.is_some(),
        zh,
    ) {
        (true, _, true) => "推进此建议 →",
        (true, _, false) => "Continue with this →",
        (false, true, true) => "打开会话答复 →",
        (false, true, false) => "Open to answer →",
        (false, false, true) => "编辑计划 →",
        (false, false, false) => "Edit plan →",
    }
}

fn primary_action(
    overview: &ProjectOverview,
    suggestion: &AdvanceSuggestion,
) -> ProjectOverviewAction {
    if let Some(followup) = &suggestion.followup {
        return if followup.state == FollowupState::Queued {
            ProjectOverviewAction::CancelFollowup(followup.id.clone())
        } else {
            ProjectOverviewAction::OpenConversation
        };
    }
    if suggestion.prompt.is_some() {
        ProjectOverviewAction::Continue {
            project_key: overview.project_key.clone(),
            suggestion_id: suggestion.id.clone(),
        }
    } else {
        ProjectOverviewAction::OpenConversation
    }
}

fn suggestions_for<'a>(
    overview: &'a ProjectOverview,
    key: &'a str,
) -> impl Iterator<Item = (usize, &'a AdvanceSuggestion)> {
    overview
        .suggestions
        .iter()
        .enumerate()
        .filter(move |(_, suggestion)| suggestion.session_key.as_deref() == Some(key))
}

fn next_step_text(suggestion: &AdvanceSuggestion, language: TitleLanguage) -> Option<String> {
    if suggestion
        .followup
        .as_ref()
        .is_some_and(|followup| followup.state == FollowupState::Queued)
    {
        return Some(if language == TitleLanguage::Chinese {
            format!("当前工作结束后继续：{}", suggestion.title)
        } else {
            format!("Queued for after the current work: {}", suggestion.title)
        });
    }
    // The progress above already explains the pending decision or missing translation.
    if suggestion.prompt.is_none() && suggestion.source != SuggestionSource::SavedPlan {
        return None;
    }
    let step = if suggestion.source == SuggestionSource::SavedPlan {
        &suggestion.description
    } else {
        &suggestion.title
    };
    Some(format!(
        "{}{}",
        language.text("Next: ", "下一步建议："),
        step
    ))
}

fn content_width(width: u16) -> u16 {
    width.saturating_sub(2).min(120)
}

fn text_height(text: &str, width: u16, maximum: u16) -> u16 {
    if text.is_empty() || width == 0 {
        return 0;
    }
    Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .line_count(width)
        .min(usize::from(maximum)) as u16
}

pub(super) fn session_height(overview: &ProjectOverview, key: &str, width: u16) -> Option<u16> {
    let item = overview.work.iter().find(|item| item.session_key == key)?;
    let width = content_width(width);
    let mut height = 1 + text_height(
        &item.description,
        width,
        if item.phase == WorkPhase::History {
            2
        } else {
            4
        },
    );
    let mut has_suggestion = false;
    for (_, suggestion) in suggestions_for(overview, key) {
        has_suggestion = true;
        height += 1 + next_step_text(suggestion, overview.language)
            .map_or(0, |text| text_height(&text, width, 3));
    }
    Some(height + u16::from(!has_suggestion))
}

fn layout(overview: &ProjectOverview, key: &str, area: Rect) -> SessionLayout {
    let mut result = SessionLayout::default();
    let Some(item) = overview.work.iter().find(|item| item.session_key == key) else {
        return result;
    };
    if area.width <= 2 || area.height == 0 {
        return result;
    }
    let width = content_width(area.width);
    let x = area.x + 2;
    result.heading = Rect::new(x, area.y, width, 1);
    let mut y = area.y + 1;
    if y == area.bottom() {
        return result;
    }
    // Keep each visible action with its own instruction. Short viewports omit later
    // actions instead of exposing a shortcut whose source is off screen.
    let suggestions: Vec<_> = suggestions_for(overview, key)
        .take(usize::from((area.height.saturating_sub(1) / 2).max(1)))
        .collect();
    let action_rows = suggestions.len().max(1) as u16;
    let text_rows = suggestions
        .iter()
        .filter(|(_, suggestion)| next_step_text(suggestion, overview.language).is_some())
        .count() as u16;
    let progress_height = text_height(
        &item.description,
        width,
        if item.phase == WorkPhase::History {
            2
        } else {
            4
        },
    )
    .min(area.bottom().saturating_sub(y + action_rows + text_rows));
    result.progress = Rect::new(x, y, width, progress_height);
    y += progress_height;
    if suggestions.is_empty() {
        result.view = Rect::new(
            x,
            y,
            width.min(
                overview
                    .language
                    .text("View conversation →", "查看会话 →")
                    .width() as u16,
            ),
            1,
        );
        return result;
    }
    for (position, (index, suggestion)) in suggestions.iter().enumerate() {
        let later = &suggestions[position + 1..];
        let reserved = 1
            + later.len() as u16
            + later
                .iter()
                .filter(|(_, item)| next_step_text(item, overview.language).is_some())
                .count() as u16;
        let height = next_step_text(suggestion, overview.language)
            .map_or(0, |text| text_height(&text, width, 3))
            .min(area.bottom().saturating_sub(y + reserved));
        let body = Rect::new(x, y, width, height);
        y += height;
        let primary_width =
            (action_label(suggestion, overview.language).width() as u16 + 5).min(width);
        let primary = Rect::new(x, y, primary_width, 1);
        let secondary_width = overview
            .language
            .text("View conversation", "查看会话")
            .width() as u16;
        let secondary = if !matches!(
            primary_action(overview, suggestion),
            ProjectOverviewAction::OpenConversation
        ) && width >= primary_width + 2 + secondary_width
        {
            Rect::new(primary.right() + 2, y, secondary_width, 1)
        } else {
            Rect::default()
        };
        result.suggestions.push(SuggestionArea {
            index: *index,
            body,
            primary,
            secondary,
        });
        y += 1;
    }
    result
}

pub(super) fn session_hit_areas(
    overview: &ProjectOverview,
    key: &str,
    area: Rect,
) -> Vec<ProjectOverviewHit> {
    let layout = layout(overview, key, area);
    let mut hits = Vec::new();
    let mut open = |rect: Rect| {
        if rect.width > 0 && rect.height > 0 {
            hits.push(ProjectOverviewHit {
                rect,
                session_key: Some(key.into()),
                action: ProjectOverviewAction::OpenConversation,
                shortcut: None,
            });
        }
    };
    open(layout.heading);
    open(layout.view);
    for entry in &layout.suggestions {
        open(entry.secondary);
    }
    for entry in layout.suggestions {
        let suggestion = &overview.suggestions[entry.index];
        hits.push(ProjectOverviewHit {
            rect: entry.primary,
            session_key: Some(key.into()),
            action: primary_action(overview, suggestion),
            shortcut: Some(entry.index as u8 + 1),
        });
    }
    hits
}

pub(super) fn scope(overview: &ProjectOverview) -> String {
    let mut parts = Vec::new();
    if overview.more_history_available {
        parts.push(if overview.language == TitleLanguage::Chinese {
            format!("最近 {} 个会话", overview.observed_sessions)
        } else {
            format!("Latest {}", overview.observed_sessions)
        });
    }
    for (count, label) in [
        (
            overview.counts.needs_input,
            overview.language.text("need your answer", "等待答复"),
        ),
        (
            overview.counts.working,
            overview.language.text("working", "正在进行"),
        ),
        (
            overview.counts.ready,
            overview.language.text("ready to continue", "可以继续"),
        ),
        (
            overview.counts.paused,
            overview.language.text("paused", "已暂停"),
        ),
        (
            overview.counts.open,
            overview.language.text("open", "已打开"),
        ),
        (
            overview.counts.history,
            overview.language.text("in history", "历史会话"),
        ),
    ] {
        if count > 0 {
            parts.push(format!("{count} {label}"));
        }
    }
    parts.join(" · ")
}

fn phase_style(app: &AppState, phase: WorkPhase) -> (Color, &'static str) {
    let language = app.title_language;
    match phase {
        WorkPhase::NeedsInput => (
            app.palette.yellow,
            language.text("Needs your answer", "等待你的答复"),
        ),
        WorkPhase::Working => (app.palette.blue, language.text("Working", "正在进行")),
        WorkPhase::Ready => (
            app.palette.green,
            language.text("Ready to continue", "可以继续"),
        ),
        WorkPhase::Paused => (app.palette.subtext0, language.text("Paused", "已暂停")),
        WorkPhase::Open => (app.palette.subtext0, language.text("Open", "已打开")),
        WorkPhase::History => (app.palette.overlay0, language.text("History", "历史会话")),
    }
}

pub(super) fn render_prose(frame: &mut Frame, area: Rect, text: &str, style: Style) {
    let paragraph = Paragraph::new(text).style(style).wrap(Wrap { trim: true });
    let clipped = paragraph.line_count(area.width) > usize::from(area.height);
    frame.render_widget(paragraph, area);
    if clipped && area.width > 0 && area.height > 0 {
        frame.render_widget(
            Paragraph::new("…").style(style),
            Rect::new(area.right() - 1, area.bottom() - 1, 1, 1),
        );
    }
}

pub(super) fn render_session_overview(
    app: &AppState,
    frame: &mut Frame,
    area: Rect,
    overview: &ProjectOverview,
    item: &ProjectWorkItem,
    selected: bool,
) {
    let layout = layout(overview, &item.session_key, area);
    let (color, phase) = phase_style(app, item.phase);
    if area.width > 0 {
        for y in area.y..area.bottom() {
            frame.render_widget(
                Paragraph::new(if y == area.y && selected {
                    "▸"
                } else {
                    "│"
                })
                .style(Style::default().fg(if selected {
                    app.palette.accent
                } else {
                    color
                })),
                Rect::new(area.x, y, 1, 1),
            );
        }
    }
    let heading_style = Style::default().bg(if selected {
        app.palette.surface0
    } else {
        app.palette.panel_bg
    });
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                phase,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {}", item.title),
                Style::default()
                    .fg(app.palette.text)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" · {}", item.backend),
                Style::default().fg(app.palette.subtext0),
            ),
        ]))
        .style(heading_style),
        layout.heading,
    );
    render_prose(
        frame,
        layout.progress,
        &item.description,
        Style::default().fg(if item.phase == WorkPhase::History {
            app.palette.subtext0
        } else {
            app.palette.text
        }),
    );
    for entry in layout.suggestions {
        let suggestion = &overview.suggestions[entry.index];
        if let Some(text) = next_step_text(suggestion, overview.language) {
            render_prose(
                frame,
                entry.body,
                &text,
                Style::default().fg(app.palette.text),
            );
        }
        render_action_button(
            frame,
            entry.primary,
            Some(&(entry.index + 1).to_string()),
            action_label(suggestion, overview.language),
            Style::default()
                .fg(panel_contrast_fg(&app.palette))
                .bg(app.palette.accent)
                .add_modifier(Modifier::BOLD),
        );
        frame.render_widget(
            Paragraph::new(overview.language.text("View conversation", "查看会话"))
                .style(Style::default().fg(app.palette.subtext0)),
            entry.secondary,
        );
    }
    frame.render_widget(
        Paragraph::new(overview.language.text("View conversation →", "查看会话 →"))
            .style(Style::default().fg(app.palette.subtext0)),
        layout.view,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::activity::{ActivityBatch, ActivityOrigin, SessionActivity};
    use crate::projects::overview::tests::fixture_project;
    use crate::projects::ProjectSummary;
    use std::collections::HashMap;

    fn build_overview(
        project: &ProjectSummary,
        runtime: &HashMap<String, WorkPhase>,
        activity: &ActivityBatch,
    ) -> ProjectOverview {
        crate::projects::overview::build_overview(
            project,
            runtime,
            activity,
            TitleLanguage::English,
        )
    }

    fn fixture() -> ProjectOverview {
        build_overview(
            &fixture_project(),
            &HashMap::from([("session-0".into(), WorkPhase::Working)]),
            &ActivityBatch {
                sessions: vec![SessionActivity {
                    session_key: "session-0".into(),
                    latest_update: Some("The invitations are ready for approval.".into()),
                    latest_request: Some(
                        "Finish the interview invitations and check the consent form.".into(),
                    ),
                    latest_response: Some("The invitations are ready for approval.".into()),
                    origin: Some(ActivityOrigin::AgentResponse),
                    next_steps: vec![
                        "Submit the invitations for approval, then send the approved copy.".into(),
                    ],
                    read_at: 100,
                }],
                loading: false,
                ..Default::default()
            },
        )
    }

    #[test]
    fn chinese_controls_counts_empty_states_and_hit_areas_follow_the_saved_language() {
        let mut overview = crate::projects::overview::build_overview(
            &fixture_project(),
            &HashMap::from([("session-0".into(), WorkPhase::Working)]),
            &ActivityBatch::default(),
            TitleLanguage::Chinese,
        );
        // Even source titles in another language cannot select English controls.
        overview.work[0].description = "正在核对同意书，邀请函准备提交审核。".into();
        overview.suggestions[0].description = "建议先提交审核，获得批准后再发送邀请函。".into();
        for (width, height) in [(120, 24), (76, 16), (48, 16)] {
            let (text, area) = draw(&overview, width, height);
            for expected in ["正在进行", "推进此建议"] {
                assert!(text.contains(expected), "missing {expected}: {text}");
            }
            for english in ["working", "Continue with this", "View conversation"] {
                assert!(!text.contains(english), "mixed control {english}: {text}");
            }
            for hit in session_hit_areas(&overview, &overview.work[0].session_key, area) {
                assert_eq!(hit.rect.intersection(area), hit.rect);
            }
        }
        overview.suggestions.clear();
        let (text, _) = draw(&overview, 120, 24);
        assert!(text.contains("查看会话"));
        overview.language = TitleLanguage::English;
        let (text, _) = draw(&overview, 120, 24);
        assert!(text.contains("Working"));
        assert!(text.contains("View conversation"));
    }

    #[test]
    fn queued_sent_and_cancelled_actions_do_not_infer_language_from_description() {
        let mut overview = fixture();
        for (state, zh, en) in [
            (
                FollowupState::Queued,
                "取消待发送的跟进",
                "Cancel queued follow-up",
            ),
            (
                FollowupState::Sent,
                "已交给 Agent · 查看",
                "Sent to Agent · view",
            ),
            (
                FollowupState::Cancelled,
                "已取消 · 查看会话",
                "Cancelled · view session",
            ),
        ] {
            overview.suggestions[0].followup = Some(crate::projects::followup::ProjectFollowup {
                id: "same-action".into(),
                project_key: overview.project_key.clone(),
                session_key: "session-0".into(),
                pane_id: "pane-0".into(),
                title: "A previously selected instruction".into(),
                state,
                message: String::new(),
                created_at: 1,
            });
            assert_eq!(
                action_label(&overview.suggestions[0], TitleLanguage::Chinese),
                zh
            );
            overview.suggestions[0].description = "原始建议使用中文。".into();
            assert_eq!(
                action_label(&overview.suggestions[0], TitleLanguage::English),
                en
            );
        }
    }

    fn draw(overview: &ProjectOverview, width: u16, height: u16) -> (String, Rect) {
        let mut state = AppState::test_new();
        state.title_language = overview.language;
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width + 6, height + 4))
                .unwrap();
        let area = Rect::new(3, 2, width, height);
        terminal
            .draw(|frame| {
                render_session_overview(&state, frame, area, overview, &overview.work[0], true)
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let text = (area.y..area.bottom())
            .map(|y| {
                let mut line = String::new();
                let mut x = area.x;
                while x < area.right() {
                    let symbol = buffer[(x, y)].symbol();
                    line.push_str(symbol);
                    // A wide glyph's next cell is padding, not an actual space in the text.
                    x += (symbol.width() as u16).max(1);
                }
                line
            })
            .collect::<Vec<_>>()
            .join("\n");
        (text, area)
    }

    #[test]
    fn briefing_shows_real_work_and_distinct_continue_and_view_actions() {
        let overview = fixture();
        for (width, height) in [(120, 24), (76, 16), (48, 16)] {
            let (text, area) = draw(&overview, width, height);
            for expected in ["consent form", "Continue with this", "approved copy"] {
                assert!(
                    text.contains(expected),
                    "missing {expected} at {width}x{height}:\n{text}"
                );
            }
            assert!(!text.contains("CURRENT WORK"));
            let hits = session_hit_areas(&overview, &overview.work[0].session_key, area);
            assert!(hits.iter().any(|hit| matches!(
                hit.action,
                ProjectOverviewAction::Continue { .. }
            ) && hit.shortcut == Some(1)));
            assert!(hits.iter().any(|hit| matches!(
                hit.action,
                ProjectOverviewAction::OpenConversation
            ) && hit.session_key.as_deref() == Some("session-0")));
            for (index, hit) in hits.iter().enumerate() {
                assert_eq!(hit.rect.intersection(area), hit.rect);
                assert!(hit.rect.width > 0 && hit.rect.height > 0);
                assert!(hits.iter().skip(index + 1).all(|other| hit
                    .rect
                    .intersection(other.rect)
                    .area()
                    == 0));
            }
        }
    }

    #[test]
    fn small_briefing_keeps_a_deliberate_action_inside_its_area() {
        let overview = fixture();
        for (width, height) in [(68, 2), (40, 4), (24, 6)] {
            let (text, area) = draw(&overview, width, height);
            assert!(text.contains("Continue"), "{text}");
            let hits = session_hit_areas(&overview, &overview.work[0].session_key, area);
            assert!(hits.iter().any(|hit| hit.shortcut == Some(1)));
            assert!(hits
                .iter()
                .all(|hit| hit.rect.intersection(area) == hit.rect));
        }
    }

    #[test]
    fn blocked_briefing_opens_the_conversation_without_sending_an_answer() {
        let overview = build_overview(
            &fixture_project(),
            &HashMap::from([("session-0".into(), WorkPhase::NeedsInput)]),
            &ActivityBatch::default(),
        );
        let (text, area) = draw(&overview, 76, 16);
        assert!(text.contains("Open to answer"));
        let first = session_hit_areas(&overview, &overview.work[0].session_key, area)
            .into_iter()
            .find(|hit| hit.shortcut == Some(1))
            .unwrap();
        assert_eq!(first.session_key.as_deref(), Some("session-0"));
        assert!(matches!(
            first.action,
            ProjectOverviewAction::OpenConversation
        ));
    }

    #[test]
    fn multiple_suggestions_stay_with_their_session_and_exact_instruction() {
        let mut overview = fixture();
        let mut other = overview.suggestions[0].clone();
        other.id = "second-instruction-for-same-session".into();
        other.title = "Check the final invitation wording.".into();
        overview.suggestions[2] = other;
        let (text, area) = draw(&overview, 76, 20);
        assert_eq!(text.matches("Prepare invitations").count(), 1);
        assert!(text.contains("approved copy") && text.contains("final invitation wording"));
        let hits = session_hit_areas(&overview, "session-0", area);
        let actions: Vec<_> = hits
            .iter()
            .filter_map(|hit| {
                if let ProjectOverviewAction::Continue { suggestion_id, .. } = &hit.action {
                    Some((hit.shortcut, suggestion_id.as_str()))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            actions,
            [
                (Some(1), overview.suggestions[0].id.as_str()),
                (Some(3), "second-instruction-for-same-session")
            ]
        );
        assert!(hits
            .iter()
            .all(|hit| hit.session_key.as_deref() == Some("session-0")));
    }
}
