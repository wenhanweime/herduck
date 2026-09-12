use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::app::state::{AppState, ProjectOverviewAction, ProjectOverviewHit};
use crate::projects::followup::FollowupState;
use crate::projects::overview::{chinese, AdvanceSuggestion, ProjectOverview, WorkPhase};

use super::widgets::{panel_contrast_fg, render_action_button};

struct WorkArea {
    index: usize,
    body: Rect,
    source: Rect,
}

struct SuggestionArea {
    index: usize,
    body: Rect,
    source: Rect,
    primary: Rect,
    secondary: Rect,
}

struct BriefingLayout {
    heading: Rect,
    next_heading: Rect,
    work: Vec<WorkArea>,
    suggestions: Vec<SuggestionArea>,
}

fn action_label(suggestion: &AdvanceSuggestion) -> &'static str {
    let zh = chinese(&suggestion.description);
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

fn layout(overview: &ProjectOverview, area: Rect) -> BriefingLayout {
    let mut result = BriefingLayout {
        heading: Rect::default(),
        next_heading: Rect::default(),
        work: Vec::new(),
        suggestions: Vec::new(),
    };
    if area.width == 0 || area.height == 0 {
        return result;
    }
    let width = area.width.min(120);
    let mut y = area.y;
    if area.height >= 7 {
        result.heading = Rect::new(area.x, y, width, 1);
        y += 1;
        let mut indices: Vec<_> = overview
            .work
            .iter()
            .enumerate()
            .filter(|(_, item)| item.phase != WorkPhase::History)
            .map(|(index, _)| index)
            .collect();
        indices.sort_by_key(|index| overview.work[*index].phase.attention_order());
        if indices.is_empty() {
            indices.extend(0..overview.work.len().min(2));
        }
        let count = if area.height >= 23 { 2 } else { 1 };
        let body_height = if width >= 95 { 2 } else { 3 };
        for index in indices.into_iter().take(count) {
            let remaining = area.bottom().saturating_sub(y);
            let height = body_height.min(remaining.saturating_sub(6));
            if height == 0 {
                break;
            }
            let body = Rect::new(area.x, y, width, height);
            let source = Rect::new(area.x, body.bottom(), width, 1);
            result.work.push(WorkArea {
                index,
                body,
                source,
            });
            y = source.bottom();
        }
        if y + 3 < area.bottom() {
            y += 1;
        }
        result.next_heading = Rect::new(area.x, y, width, 1);
        y += 1;
    }
    let remaining = area.bottom().saturating_sub(y);
    if remaining == 0 {
        return result;
    }
    let minimum = if width >= 100 { 5 } else { 6 };
    let count = usize::from((remaining / minimum).clamp(1, 3)).min(overview.suggestions.len());
    for index in 0..count {
        let height = (remaining / count as u16).min(8);
        let source_height = u16::from(height >= 3);
        let body_height = height.saturating_sub(source_height + 1).min(6);
        let body = Rect::new(area.x, y, width, body_height);
        let source = Rect::new(area.x, body.bottom(), width, source_height);
        let action_y = source.bottom();
        let primary_width =
            (action_label(&overview.suggestions[index]).width() as u16 + 5).min(width);
        let primary = Rect::new(area.x, action_y, primary_width, 1);
        let secondary_x = primary.right().saturating_add(2);
        let secondary = Rect::new(
            secondary_x,
            action_y,
            width.saturating_sub(primary_width + 2),
            1,
        );
        result.suggestions.push(SuggestionArea {
            index,
            body,
            source,
            primary,
            secondary,
        });
        y += height;
    }
    result
}

pub(crate) fn overview_hit_areas(
    overview: &ProjectOverview,
    area: Rect,
) -> Vec<ProjectOverviewHit> {
    let layout = layout(overview, area);
    let mut hits = Vec::new();
    for entry in layout.work {
        hits.push(ProjectOverviewHit {
            rect: entry.body.union(entry.source),
            session_key: Some(overview.work[entry.index].session_key.clone()),
            action: ProjectOverviewAction::OpenConversation,
            shortcut: None,
        });
    }
    for entry in layout.suggestions {
        let suggestion = &overview.suggestions[entry.index];
        hits.push(ProjectOverviewHit {
            rect: entry.primary,
            session_key: suggestion.session_key.clone(),
            action: primary_action(overview, suggestion),
            shortcut: Some(entry.index as u8 + 1),
        });
        if entry.secondary.width >= 12 && suggestion.session_key.is_some() {
            hits.push(ProjectOverviewHit {
                rect: entry.secondary,
                session_key: suggestion.session_key.clone(),
                action: ProjectOverviewAction::OpenConversation,
                shortcut: None,
            });
        }
    }
    hits
}

fn clipped(text: &str, width: u16) -> String {
    super::text::truncate_end(text, usize::from(width))
}

fn scope(overview: &ProjectOverview) -> String {
    let mut parts = Vec::new();
    if overview.more_history_available {
        parts.push(format!("Latest {}", overview.observed_sessions));
    }
    for (count, label) in [
        (overview.counts.working, "working"),
        (overview.counts.needs_input, "need input"),
        (overview.counts.ready, "ready"),
        (overview.counts.paused, "paused"),
    ] {
        if count > 0 {
            parts.push(format!("{count} {label}"));
        }
    }
    parts.join(" · ")
}

pub(super) fn render_project_overview(
    app: &AppState,
    frame: &mut Frame,
    area: Rect,
    overview: &ProjectOverview,
) {
    let layout = layout(overview, area);
    let zh = overview.work.iter().any(|item| chinese(&item.description));
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                if zh {
                    "现在在做什么"
                } else {
                    "What's happening"
                },
                Style::default()
                    .fg(app.palette.text)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("   {}", scope(overview)),
                Style::default().fg(app.palette.overlay0),
            ),
        ])),
        layout.heading,
    );
    for entry in layout.work {
        let item = &overview.work[entry.index];
        frame.render_widget(
            Paragraph::new(item.description.as_str())
                .style(Style::default().fg(app.palette.text))
                .wrap(Wrap { trim: true }),
            entry.body,
        );
        frame.render_widget(
            Paragraph::new(clipped(
                &format!("{} · {} · {}", item.title, item.backend, item.phase.label()),
                entry.source.width,
            ))
            .style(Style::default().fg(app.palette.subtext0)),
            entry.source,
        );
    }
    frame.render_widget(
        Paragraph::new(if zh {
            "接下来可以这样推进"
        } else {
            "Suggested follow-ups"
        })
        .style(
            Style::default()
                .fg(app.palette.text)
                .add_modifier(Modifier::BOLD),
        ),
        layout.next_heading,
    );
    for entry in layout.suggestions {
        let suggestion = &overview.suggestions[entry.index];
        let text = if let Some(followup) = suggestion
            .followup
            .as_ref()
            .filter(|item| item.state == FollowupState::Queued)
        {
            if chinese(&suggestion.description) {
                format!(
                    "已安排下一轮跟进：{}。Agent 完成当前工作后会收到这条指令。",
                    followup.title.trim_end_matches(['。', '.'])
                )
            } else {
                format!(
                    "Queued next: {}. The Agent will receive this after its current work.",
                    followup.title.trim_end_matches(['。', '.'])
                )
            }
        } else {
            suggestion.description.clone()
        };
        frame.render_widget(
            Paragraph::new(text)
                .style(Style::default().fg(app.palette.text))
                .wrap(Wrap { trim: true }),
            entry.body,
        );
        let source = suggestion
            .session_key
            .as_ref()
            .and_then(|key| overview.work.iter().find(|item| &item.session_key == key))
            .map(|item| format!("{} · {}", item.title, item.backend))
            .unwrap_or_else(|| suggestion.reason.clone());
        frame.render_widget(
            Paragraph::new(clipped(&source, entry.source.width))
                .style(Style::default().fg(app.palette.subtext0)),
            entry.source,
        );
        render_action_button(
            frame,
            entry.primary,
            Some(&(entry.index + 1).to_string()),
            action_label(suggestion),
            Style::default()
                .fg(panel_contrast_fg(&app.palette))
                .bg(app.palette.accent)
                .add_modifier(Modifier::BOLD),
        );
        if entry.secondary.width >= 12 && suggestion.session_key.is_some() {
            frame.render_widget(
                Paragraph::new(if chinese(&suggestion.description) {
                    "查看会话"
                } else {
                    "View conversation"
                })
                .style(Style::default().fg(app.palette.subtext0)),
                entry.secondary,
            );
        }
    }
    if overview.suggestions.is_empty() {
        let y = layout.next_heading.bottom().max(area.y);
        frame.render_widget(
            Paragraph::new("Follow-ups appear as conversations record their progress.")
                .style(Style::default().fg(app.palette.subtext0))
                .wrap(Wrap { trim: true }),
            Rect::new(area.x, y, area.width, area.bottom().saturating_sub(y)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::activity::{ActivityBatch, ActivityOrigin, SessionActivity};
    use crate::projects::overview::{build_overview, tests::fixture_project};
    use std::collections::HashMap;

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
            },
        )
    }

    fn draw(overview: &ProjectOverview, width: u16, height: u16) -> (String, Rect) {
        let state = AppState::test_new();
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width + 6, height + 4))
                .unwrap();
        let area = Rect::new(3, 2, width, height);
        terminal
            .draw(|frame| render_project_overview(&state, frame, area, overview))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let text = (area.y..area.bottom())
            .map(|y| {
                (area.x..area.right())
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
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
            for expected in [
                "What's happening",
                "consent form",
                "Suggested follow-ups",
                "Continue with this",
                "approved copy",
            ] {
                assert!(
                    text.contains(expected),
                    "missing {expected} at {width}x{height}:\n{text}"
                );
            }
            assert!(!text.contains("CURRENT WORK"));
            let hits = overview_hit_areas(&overview, area);
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
            let hits = overview_hit_areas(&overview, area);
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
        let first = overview_hit_areas(&overview, area)
            .into_iter()
            .find(|hit| hit.shortcut == Some(1))
            .unwrap();
        assert_eq!(first.session_key.as_deref(), Some("session-0"));
        assert!(matches!(
            first.action,
            ProjectOverviewAction::OpenConversation
        ));
    }
}
