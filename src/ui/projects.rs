use std::collections::HashSet;

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::app::state::{
    AppState, ProjectFilter, ProjectRowHitArea, ProjectSessionActivation, ProjectTreeAction,
};
use crate::projects::{AutomationTemplateSummary, IndexedSessionSummary, ProjectKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProjectTreeRow {
    Project {
        project_key: String,
        display_name: String,
        session_count: usize,
        collapsed: bool,
        kind: ProjectKind,
    },
    Session(IndexedSessionSummary),
    Automation(AutomationTemplateSummary),
    Thin {
        project_key: String,
        count: usize,
    },
    LoadOlder {
        project_key: String,
    },
    Empty(String),
    Diagnostic(String),
    ScanStatus(String),
}

impl ProjectTreeRow {
    pub(crate) fn action(&self) -> Option<ProjectTreeAction> {
        match self {
            Self::Project { project_key, .. } => Some(ProjectTreeAction::ToggleProject {
                project_key: project_key.clone(),
            }),
            Self::Session(session) => {
                let activation = match (
                    session.live,
                    session.workspace_id.as_ref(),
                    session.pane_id.as_ref(),
                    session.runtime_generation,
                ) {
                    (true, Some(workspace_id), Some(pane_id), Some(runtime_generation)) => {
                        ProjectSessionActivation::Live {
                            session_key: session.stable_key.clone(),
                            workspace_id: workspace_id.clone(),
                            pane_id: pane_id.clone(),
                            runtime_generation,
                        }
                    }
                    _ => ProjectSessionActivation::History {
                        session_key: session.stable_key.clone(),
                    },
                };
                Some(ProjectTreeAction::Activate(activation))
            }
            Self::Thin { project_key, .. } => Some(ProjectTreeAction::ToggleThin {
                project_key: project_key.clone(),
            }),
            Self::Automation(_) => None,
            Self::LoadOlder { project_key } => Some(ProjectTreeAction::LoadOlder {
                project_key: project_key.clone(),
            }),
            Self::Empty(_) | Self::Diagnostic(_) | Self::ScanStatus(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectSidebarGeometry {
    pub sidebar_tabs: [Rect; 4],
    pub filter_tabs: [Rect; 3],
    pub search: Rect,
    pub tree: Rect,
    pub row_hits: Vec<ProjectRowHitArea>,
    pub normalized_scroll: usize,
}

pub(crate) fn project_tree_rows(app: &AppState) -> Vec<ProjectTreeRow> {
    if let Some(category) = app.projects.snapshot.diagnostic_category.as_ref() {
        return vec![ProjectTreeRow::Diagnostic(format!(
            "Catalog unavailable · {category}"
        ))];
    }

    let query = app.projects.query.trim().to_lowercase();
    let mut rows = Vec::new();
    if app.sidebar_view == crate::app::state::SidebarView::Sessions {
        let mut seen = HashSet::new();
        let mut sessions = app
            .projects
            .snapshot
            .projects
            .iter()
            .flat_map(|project| project.sessions.iter())
            .filter(|session| seen.insert(session.stable_key.clone()))
            .filter(|session| {
                (app.projects.filter != ProjectFilter::Live || session.live)
                    && (app.projects.filter != ProjectFilter::Unclassified
                        || session.topic_label.is_none())
            })
            .filter(|session| {
                query.is_empty()
                    || format!(
                        "{} {} {}",
                        session.title,
                        session.backend,
                        session.cwd.as_deref().unwrap_or_default()
                    )
                    .to_lowercase()
                    .contains(&query)
            })
            .cloned()
            .collect::<Vec<_>>();
        sessions.sort_by(|left, right| {
            right
                .last_activity_at
                .cmp(&left.last_activity_at)
                .then_with(|| left.stable_key.cmp(&right.stable_key))
        });
        rows.extend(sessions.into_iter().map(ProjectTreeRow::Session));
        if rows.is_empty() {
            rows.push(ProjectTreeRow::Empty(if query.is_empty() {
                "No sessions yet".to_string()
            } else {
                "No matching sessions".to_string()
            }));
        }
        return rows;
    }
    let grouping = app
        .sidebar_view
        .project_grouping()
        .unwrap_or(app.projects.grouping);
    let groups = match grouping {
        crate::app::state::ProjectGrouping::Directories => &app.projects.snapshot.projects,
        crate::app::state::ProjectGrouping::Topics => &app.projects.snapshot.topics,
    };
    for project in groups {
        if app.projects.filter == ProjectFilter::Unclassified
            && project.kind != ProjectKind::Unclassified
        {
            continue;
        }

        let project_matches = query.is_empty()
            || format!("{} {}", project.display_name, project.canonical_path)
                .to_lowercase()
                .contains(&query);
        let sessions = project
            .sessions
            .iter()
            .filter(|session| app.projects.filter != ProjectFilter::Live || session.live)
            .filter(|session| {
                project_matches
                    || format!(
                        "{} {} {}",
                        session.title,
                        session.backend,
                        session.cwd.as_deref().unwrap_or_default()
                    )
                    .to_lowercase()
                    .contains(&query)
            })
            .cloned()
            .collect::<Vec<_>>();
        let automation = project
            .automation
            .iter()
            .filter(|_| app.projects.filter == ProjectFilter::All)
            .filter(|template| {
                project_matches
                    || format!("{} {}", template.title, template.backend)
                        .to_lowercase()
                        .contains(&query)
            })
            .cloned()
            .collect::<Vec<_>>();
        if sessions.is_empty() && automation.is_empty() {
            continue;
        }

        let collapsed =
            query.is_empty() && app.collapsed_project_keys.contains(&project.canonical_key);
        rows.push(ProjectTreeRow::Project {
            project_key: project.canonical_key.clone(),
            display_name: project.display_name.clone(),
            session_count: automation.iter().fold(sessions.len(), |count, template| {
                count.saturating_add(usize::try_from(template.count).unwrap_or(usize::MAX))
            }),
            collapsed,
            kind: project.kind,
        });
        if !collapsed {
            let collapse_thin = query.is_empty()
                && app.projects.filter == ProjectFilter::All
                && !app
                    .projects
                    .expanded_thin_keys
                    .contains(&project.canonical_key);
            let thin = if collapse_thin {
                sessions.len().min(project.thin_count as usize)
            } else {
                0
            };
            let substantive_len = sessions.len().saturating_sub(thin);
            rows.extend(
                sessions
                    .into_iter()
                    .take(substantive_len)
                    .map(ProjectTreeRow::Session),
            );
            if thin > 0 {
                rows.push(ProjectTreeRow::Thin {
                    project_key: project.canonical_key.clone(),
                    count: thin,
                });
            }
            rows.extend(automation.into_iter().map(ProjectTreeRow::Automation));
            if project.next_cursor.is_some() && app.projects.filter == ProjectFilter::All {
                rows.push(ProjectTreeRow::LoadOlder {
                    project_key: project.canonical_key.clone(),
                });
            }
        }
    }

    if rows.is_empty() {
        // Only a genuinely empty Catalog gets the "not indexed yet" copy; an empty filter or
        // search result must not claim the Catalog is empty.
        let catalog_is_empty = groups.is_empty();
        let label = if !app.projects.query.trim().is_empty() {
            "No matching sessions"
        } else if catalog_is_empty {
            match grouping {
                crate::app::state::ProjectGrouping::Directories => "No indexed projects yet",
                crate::app::state::ProjectGrouping::Topics => "No clustered topics yet",
            }
        } else {
            "No sessions in this filter"
        };
        rows.push(ProjectTreeRow::Empty(label.to_string()));
        if catalog_is_empty {
            rows.extend(scan_status_rows(app));
        }
    }
    rows
}

/// Renders adapter scan state so an empty or partial Catalog is explained rather than
/// looking like a Catalog with nothing in it.
fn scan_status_rows(app: &AppState) -> Vec<ProjectTreeRow> {
    app.projects
        .snapshot
        .scan_status
        .iter()
        .map(|status| {
            let detail = status
                .diagnostic_category
                .as_deref()
                .map(|category| format!(" · {category}"))
                .unwrap_or_default();
            ProjectTreeRow::ScanStatus(format!("{}: {}{detail}", status.adapter, status.state))
        })
        .collect()
}

/// The number of projects still awaiting review. This is the only place the pending count is
/// derived, so the `unclassified` chip stays the single display of it.
pub(crate) fn unclassified_pending_count(app: &AppState) -> usize {
    let grouping = app
        .sidebar_view
        .project_grouping()
        .unwrap_or(app.projects.grouping);
    if grouping == crate::app::state::ProjectGrouping::Topics {
        return 0;
    }
    app.projects
        .snapshot
        .projects
        .iter()
        .filter(|project| project.kind == ProjectKind::Unclassified)
        .count()
}

pub(crate) struct UnclassifiedChip {
    pub label: String,
    pub disabled: bool,
}

/// The `unclassified` chip's rendered contract, kept out of the render loop so both the drawing
/// code and the tests read the same label and disabled state.
pub(crate) fn unclassified_chip(app: &AppState) -> UnclassifiedChip {
    let pending = unclassified_pending_count(app);
    UnclassifiedChip {
        label: if pending == 0 {
            "unclass".to_string()
        } else {
            format!("unclass {pending}")
        },
        disabled: pending == 0,
    }
}

/// Lays out the visible tree rows, returning each row with the rect it occupies.
///
/// The current session's row is two lines tall so it can carry a status line. Hit testing and
/// rendering must agree on those heights, so both read this one function — computing them twice
/// would let a click land on the row above or below.
fn visible_row_layout<'a>(
    app: &AppState,
    rows: &'a [ProjectTreeRow],
    tree: Rect,
    scroll: usize,
) -> Vec<(&'a ProjectTreeRow, Rect)> {
    let mut laid_out = Vec::new();
    let mut y = tree.y;
    let bottom = tree.y.saturating_add(tree.height);
    for row in rows.iter().skip(scroll) {
        if y >= bottom {
            break;
        }
        let height = row_height(app, row).min(bottom.saturating_sub(y));
        laid_out.push((row, Rect::new(tree.x, y, tree.width, height)));
        y = y.saturating_add(height);
    }
    laid_out
}

/// Height of one tree row, in lines.
///
/// The current session gets a second line for its agent status. That extra line is what makes the
/// current row unmistakable — a one-line row differing only in background was reported as "no
/// highlight" four times.
fn row_height(app: &AppState, row: &ProjectTreeRow) -> u16 {
    match row {
        ProjectTreeRow::Session(session) if is_current_session(app, session) => 2,
        _ => 1,
    }
}

pub(crate) fn project_sidebar_geometry(app: &AppState, area: Rect) -> ProjectSidebarGeometry {
    let content = Rect::new(area.x, area.y, area.width.saturating_sub(1), area.height);
    if content.width == 0 || content.height == 0 {
        return ProjectSidebarGeometry {
            sidebar_tabs: [Rect::default(); 4],
            filter_tabs: [Rect::default(); 3],
            search: Rect::default(),
            tree: Rect::default(),
            row_hits: Vec::new(),
            normalized_scroll: 0,
        };
    }

    let tab_gap = u16::from(content.width >= 5);
    let tab_inner = content.width.saturating_sub(tab_gap.saturating_mul(3));
    // Keep the four peer tabs balanced while giving the first tab a little room for its label.
    // At the normal sidebar width this fits every label; narrower sidebars still retain distinct
    // clickable rects for each tab.
    let first_width = tab_inner.saturating_mul(7) / 31;
    let second_width = tab_inner.saturating_sub(first_width).saturating_mul(8) / 24;
    let third_width = tab_inner
        .saturating_sub(first_width.saturating_add(second_width))
        .saturating_mul(8)
        / 16;
    let fourth_width = tab_inner.saturating_sub(
        first_width
            .saturating_add(second_width)
            .saturating_add(third_width),
    );
    let sidebar_tabs = [
        Rect::new(content.x, content.y, first_width, 1),
        Rect::new(
            content.x + first_width + tab_gap,
            content.y,
            second_width,
            1,
        ),
        Rect::new(
            content.x + first_width + tab_gap + second_width + tab_gap,
            content.y,
            third_width,
            1,
        ),
        Rect::new(
            content.x + first_width + tab_gap + second_width + tab_gap + third_width + tab_gap,
            content.y,
            fourth_width,
            1,
        ),
    ];
    let controls_y = content.y.saturating_add(1);
    // Filter chips paint a background when selected, so two adjacent chips would read as one block.
    // They reuse the view tabs' gap rule: the gap is dropped only when the sidebar is too narrow to
    // afford it, and the chips shrink before the separation does.
    let filter_inner = content.width.saturating_sub(tab_gap.saturating_mul(2));
    let first_width = filter_inner.min(5);
    let second_width = filter_inner.saturating_sub(first_width).min(6);
    let third_width = filter_inner.saturating_sub(first_width.saturating_add(second_width));
    let filter_tabs = [
        Rect::new(content.x, controls_y, first_width, 1),
        Rect::new(
            content.x + first_width + tab_gap,
            controls_y,
            second_width,
            1,
        ),
        Rect::new(
            content.x + first_width + tab_gap + second_width + tab_gap,
            controls_y,
            third_width,
            1,
        ),
    ];
    let search = Rect::new(
        content.x,
        content.y.saturating_add(2),
        content.width,
        u16::from(content.height >= 3),
    );
    let tree_y = content.y.saturating_add(3);
    let tree = Rect::new(
        content.x,
        tree_y,
        content.width,
        content.height.saturating_sub(3),
    );

    let rows = project_tree_rows(app);
    let viewport = usize::from(tree.height);
    let max_scroll = rows.len().saturating_sub(viewport);
    let normalized_scroll = app.projects.scroll.min(max_scroll);
    let row_hits = visible_row_layout(app, &rows, tree, normalized_scroll)
        .into_iter()
        .enumerate()
        .filter_map(|(visible_idx, (row, rect))| {
            row.action().map(|action| ProjectRowHitArea {
                rect,
                action,
                row_index: normalized_scroll + visible_idx,
            })
        })
        .collect();

    ProjectSidebarGeometry {
        sidebar_tabs,
        filter_tabs,
        search,
        tree,
        row_hits,
        normalized_scroll,
    }
}

pub(crate) fn render_sidebar_tabs(app: &AppState, frame: &mut Frame, tabs: [Rect; 4]) {
    let labels = ["Spaces", "Sessions", "Projects", "Clusters"];
    for (index, (label, rect)) in labels.into_iter().zip(tabs).enumerate() {
        if rect.width == 0 {
            continue;
        }
        let active = matches!(
            (index, app.sidebar_view),
            (0, crate::app::state::SidebarView::SpacesAgents)
                | (1, crate::app::state::SidebarView::Sessions)
                | (2, crate::app::state::SidebarView::Projects)
                | (3, crate::app::state::SidebarView::Clusters)
        );
        let style = if active {
            Style::default()
                .fg(app.palette.text)
                .bg(app.palette.surface0)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.palette.overlay0)
        };
        frame.render_widget(Paragraph::new(label).style(style).centered(), rect);
    }
}

/// A stable colour per agent, so five backends are distinguishable at a glance.
///
/// Unknown backends fall back to the neutral metadata colour rather than being assigned a colour
/// that would collide with a known one.
fn backend_color(app: &AppState, backend: &str) -> ratatui::style::Color {
    match backend {
        "codex" => app.palette.blue,
        "claude" => app.palette.peach,
        "grok" => app.palette.teal,
        "pi" => app.palette.mauve,
        "opencode" => app.palette.yellow,
        _ => app.palette.overlay0,
    }
}

fn project_name_color(app: &AppState, kind: ProjectKind) -> ratatui::style::Color {
    match kind {
        ProjectKind::GitCommonDir | ProjectKind::Cwd => app.palette.subtext0,
        ProjectKind::Semantic => app.palette.mauve,
        ProjectKind::Unclassified => app.palette.overlay1,
    }
}

/// Whether this Catalog session is the one currently focused in the terminal.
///
/// Derived from the runtime mapping the server owns rather than from any TUI selection state, so
/// it stays true after the user leaves `Navigate` mode. All three mapping fields must be present,
/// matching the activation contract in `ProjectTreeRow::action`: a partial mapping is read-only
/// history and must never render as open.
///
/// The generation is not re-checked here because the leases live on `App`, not `AppState`. It does
/// not need to be: a pane exit clears the runtime mapping, which drops `live` in the next snapshot.
/// A snapshot that lags a pane exit by one frame can only mis-highlight a row cosmetically; the
/// activation path in `app::input` still re-verifies the generation before focusing anything.
pub(crate) fn is_open_session(app: &AppState, session: &IndexedSessionSummary) -> bool {
    if !session.live {
        return false;
    }
    let (Some(workspace_id), Some(pane_id), Some(_generation)) = (
        session.workspace_id.as_deref(),
        session.pane_id.as_deref(),
        session.runtime_generation,
    ) else {
        return false;
    };

    let Some(active_ws_idx) = app.active else {
        return false;
    };
    let Some(ws) = app.workspaces.get(active_ws_idx) else {
        return false;
    };
    if ws.id != workspace_id {
        return false;
    }
    let Some(focused) = ws.focused_pane_id() else {
        return false;
    };
    ws.public_pane_number(focused)
        .map(|number| crate::workspace::public_pane_id_for_number(&ws.id, number))
        .is_some_and(|focused_public_id| focused_public_id == pane_id)
}

/// Whether this row is the session the user is currently working in.
///
/// Deliberately independent of `app.mode`. Clicking a session switches the mode to `Terminal` (a
/// live pane) or `ProjectHistory` (a read-only preview), so gating the highlight on `Navigate`
/// meant the row lost every marking at the exact moment it became the current one — which is what
/// "clicking a session shows no highlight" kept reporting. Herdr's Sessions sidebar derives
/// `is_active` from `app.active` for the same reason: the active row stays marked in every mode.
fn is_current_session(app: &AppState, session: &IndexedSessionSummary) -> bool {
    if app
        .projects
        .history_session_key
        .as_deref()
        .is_some_and(|key| key == session.stable_key)
    {
        return true;
    }
    is_open_session(app, session)
}

/// Agent state of a live session's pane, for the status line on an expanded row.
///
/// A historical session has no pane and therefore no state; `None` means "do not claim one".
fn session_agent_state(
    app: &AppState,
    session: &IndexedSessionSummary,
) -> Option<(crate::detect::AgentState, bool)> {
    if !session.live {
        return None;
    }
    let pane_id = session.pane_id.as_deref()?;
    let workspace_id = session.workspace_id.as_deref()?;
    let ws = app.workspaces.iter().find(|ws| ws.id == workspace_id)?;
    let pane =
        ws.tabs
            .iter()
            .flat_map(|tab| tab.panes.iter())
            .find_map(|(candidate_id, pane)| {
                ws.public_pane_number(*candidate_id)
                    .map(|number| crate::workspace::public_pane_id_for_number(&ws.id, number))
                    .filter(|public| public == pane_id)
                    .map(|_| pane)
            })?;
    app.terminals
        .get(&pane.attached_terminal_id)
        .map(|terminal| (terminal.state, pane.seen))
}

pub(crate) fn render_projects_sidebar(app: &AppState, frame: &mut Frame, area: Rect) {
    let geometry = project_sidebar_geometry(app, area);
    if area.width > 0 {
        let separator_x = area.x + area.width.saturating_sub(1);
        let separator_style = if app.mode == crate::app::Mode::Navigate {
            Style::default().fg(app.palette.accent)
        } else {
            Style::default()
                .fg(app.palette.overlay0)
                .add_modifier(Modifier::DIM)
        };
        for y in area.y..area.y + area.height {
            frame.buffer_mut()[(separator_x, y)]
                .set_symbol("│")
                .set_style(separator_style);
        }
    }
    render_sidebar_tabs(app, frame, geometry.sidebar_tabs);

    let filters = [
        (ProjectFilter::All, "all".to_string(), false),
        (ProjectFilter::Live, "live".to_string(), false),
        {
            let chip = unclassified_chip(app);
            (ProjectFilter::Unclassified, chip.label, chip.disabled)
        },
    ];
    for ((filter, label, disabled), rect) in filters.into_iter().zip(geometry.filter_tabs) {
        if rect.width == 0 {
            continue;
        }
        let style = if disabled {
            // `overlay0` + DIM, never `surface_dim`: that slot is a background and sits ~1.1:1
            // against `panel_bg`, so using it as a foreground renders the chip unreadable.
            Style::default()
                .fg(app.palette.overlay0)
                .add_modifier(Modifier::DIM)
        } else if app.projects.filter == filter {
            Style::default()
                .fg(app.palette.text)
                .bg(app.palette.surface1)
        } else {
            Style::default().fg(app.palette.overlay0)
        };
        frame.render_widget(Paragraph::new(label).style(style).centered(), rect);
    }

    if geometry.search.height > 0 {
        let (prefix, query) = if app.projects.query.is_empty() {
            ("/ ", "search groups, sessions, agents")
        } else {
            ("/ ", app.projects.query.as_str())
        };
        let query_style = if app.projects.query.is_empty() {
            Style::default().fg(app.palette.overlay0)
        } else {
            Style::default().fg(app.palette.text)
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(prefix, Style::default().fg(app.palette.accent)),
                Span::styled(query, query_style),
            ])),
            geometry.search,
        );
    }

    let rows = project_tree_rows(app);
    let show_session_subject = app
        .sidebar_view
        .project_grouping()
        .unwrap_or(app.projects.grouping)
        == crate::app::state::ProjectGrouping::Directories;
    // Text of the previous session row, so a row that would render identically to the one above it
    // can reveal a later clause instead. Cleared at each Project header because only siblings sit
    // next to each other; two projects may legitimately open with the same words.
    let mut previous_session_text: Option<String> = None;
    let laid_out = visible_row_layout(app, &rows, geometry.tree, geometry.normalized_scroll);
    for (visible_idx, (row, row_rect)) in laid_out.into_iter().enumerate() {
        let absolute_idx = geometry.normalized_scroll + visible_idx;
        // Only the first line carries the row text; the second, when present, is the status line.
        let rect = Rect::new(row_rect.x, row_rect.y, row_rect.width, 1);
        let cursor = app.projects.selected_row == absolute_idx;
        let current =
            matches!(row, ProjectTreeRow::Session(session) if is_current_session(app, session));
        // Neither state is gated on `app.mode`. Clicking a session switches the mode away from
        // `Navigate`, so gating here erased the marking exactly when the row became the current
        // one. Current wins the stronger fill because "what am I working in" outranks "where is
        // the keyboard cursor".
        //
        // The background has to travel with the text: painting it first and then rendering an
        // unstyled `Paragraph` over the same rect resets every cell the glyphs occupy.
        let row_style = match (current, cursor) {
            (true, _) => Style::default().bg(app.palette.surface1),
            (false, true) => Style::default().bg(app.palette.surface0),
            (false, false) => Style::default(),
        };
        // Fill the whole row, including a second line, before any text lands on it.
        if current || cursor {
            frame.render_widget(Paragraph::new("").style(row_style), row_rect);
        }
        let line = match row {
            ProjectTreeRow::Project {
                display_name,
                session_count,
                collapsed,
                kind,
                ..
            } => {
                // A new Project starts a new sibling group; two projects may legitimately open
                // with the same words without one hiding the other.
                previous_session_text = None;
                let marker = if *collapsed { "▸" } else { "▾" };
                let badge = if *kind == ProjectKind::Unclassified {
                    " ?"
                } else {
                    ""
                };
                Line::from(vec![
                    Span::styled(
                        format!("{marker} "),
                        Style::default().fg(app.palette.accent),
                    ),
                    Span::styled(
                        display_name.clone(),
                        Style::default()
                            .fg(project_name_color(app, *kind))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" {session_count}{badge}"),
                        Style::default().fg(app.palette.overlay0),
                    ),
                ])
            }
            ProjectTreeRow::Session(session) => {
                let label = super::session_label::session_label(
                    &session.title,
                    session.topic_label.as_deref(),
                );
                let task = super::session_label::disambiguate_from(
                    &label.task,
                    previous_session_text.as_deref(),
                    &session.title,
                );
                previous_session_text = Some(task.clone());
                let mut spans = vec![Span::styled(
                    if session.live { "  ● " } else { "  ○ " },
                    Style::default().fg(if session.live {
                        app.palette.green
                    } else {
                        app.palette.overlay0
                    }),
                )];
                // No topic prefix here on purpose. A topic groups dozens of sessions — the three
                // largest on this machine cover 63, 54 and 51 — so printing it on every row
                // repeats the group name instead of telling the rows apart, and eats the narrow
                // width the title needs. The tree's parent node already states the grouping.
                // A title the cleaner could make no sense of is dimmed rather than dropped: the
                // session is still real and openable, it just cannot describe itself.
                let title_style = if super::session_label::is_low_signal(&session.title) {
                    Style::default().fg(app.palette.overlay0)
                } else {
                    Style::default()
                        .fg(app.palette.text)
                        .add_modifier(Modifier::BOLD)
                };
                // Agent first: it is the one field every row has, it is short and fixed-width
                // enough to scan down the column, and trailing it meant a clipped title hid it.
                spans.push(Span::styled(
                    format!("{} ", session.backend),
                    Style::default().fg(backend_color(app, &session.backend)),
                ));
                if show_session_subject {
                    if let Some(subject) = label.subject.as_deref() {
                        spans.push(Span::styled(
                            format!("{} ", subject),
                            Style::default().fg(backend_color(app, &session.backend)),
                        ));
                    }
                }
                spans.push(Span::styled(task, title_style));
                Line::from(spans)
            }
            ProjectTreeRow::Automation(template) => Line::from(vec![
                Span::styled("  ◇ ", Style::default().fg(app.palette.overlay0)),
                Span::styled(
                    format!("{} × {}", template.title, template.count),
                    Style::default().fg(app.palette.subtext0),
                ),
                Span::styled(
                    format!(" · {} automation", template.backend),
                    Style::default().fg(app.palette.overlay0),
                ),
            ]),
            ProjectTreeRow::Thin { count, .. } => Line::from(Span::styled(
                format!("  +{count} short sessions"),
                Style::default()
                    .fg(app.palette.overlay0)
                    .add_modifier(Modifier::DIM),
            )),
            ProjectTreeRow::LoadOlder { .. } => Line::from(Span::styled(
                "  Load older…",
                Style::default().fg(app.palette.accent),
            )),
            ProjectTreeRow::Empty(message) | ProjectTreeRow::Diagnostic(message) => {
                Line::from(Span::styled(
                    format!(" {message}"),
                    Style::default().fg(app.palette.overlay0),
                ))
            }
            ProjectTreeRow::ScanStatus(message) => Line::from(Span::styled(
                format!("   {message}"),
                Style::default()
                    .fg(app.palette.overlay0)
                    .add_modifier(Modifier::DIM),
            )),
        };
        frame.render_widget(Paragraph::new(line).style(row_style), rect);
        // Drawn after the text: every row body starts with an indent space that would otherwise
        // overwrite the bar. Reusing that indent column keeps the marker free of title width.
        if current && rect.width > 0 {
            frame.buffer_mut()[(rect.x, rect.y)]
                .set_symbol("▎")
                .set_fg(app.palette.accent);
        }

        // The status line of an expanded current row.
        if row_rect.height > 1 {
            if let ProjectTreeRow::Session(session) = row {
                let status_rect = Rect::new(row_rect.x, row_rect.y + 1, row_rect.width, 1);
                frame.render_widget(
                    Paragraph::new(session_status_line(app, session)).style(row_style),
                    status_rect,
                );
            }
        }
    }
}

/// Second line of the current session's row: what it is doing and where it lives.
///
/// A live session reports its agent state (`working` / `done` / `idle` / `blocked`) using the same
/// vocabulary as the Sessions sidebar. A historical session has no pane, so it says so rather than
/// borrowing a state it does not have.
fn session_status_line<'a>(app: &AppState, session: &IndexedSessionSummary) -> Line<'a> {
    let mut spans = vec![Span::raw("    ")];
    match session_agent_state(app, session) {
        Some((state, seen)) => {
            let (glyph, glyph_style) = super::status::state_dot(state, seen, &app.palette);
            spans.push(Span::styled(format!("{glyph} "), glyph_style));
            spans.push(Span::styled(
                super::status::state_label(state, seen),
                Style::default().fg(super::status::state_label_color(state, seen, &app.palette)),
            ));
        }
        None => spans.push(Span::styled(
            "read-only history",
            Style::default().fg(app.palette.overlay0),
        )),
    }
    if let Some(cwd) = session.cwd.as_deref() {
        spans.push(Span::styled(
            format!("  {cwd}"),
            Style::default()
                .fg(app.palette.overlay0)
                .add_modifier(Modifier::DIM),
        ));
    }
    Line::from(spans)
}

/// Renders the read-only preview of a historical session.
///
/// Reads the agent's own transcript file. Nothing here writes to a PTY or resumes an agent:
/// opening history must never start work.
pub(crate) fn render_project_history(app: &AppState, frame: &mut Frame, area: Rect) {
    // Every other overlay clears first. Without this the preview paints on top of whatever the
    // terminal was showing, so the pane's own output stayed visible between the transcript lines
    // and only went away once a click forced a full repaint.
    frame.render_widget(Clear, area);

    let session = app
        .projects
        .history_session_key
        .as_deref()
        .and_then(|session_key| {
            app.projects
                .snapshot
                .projects
                .iter()
                .chain(app.projects.snapshot.topics.iter())
                .flat_map(|project| project.sessions.iter())
                .find(|session| session.stable_key == session_key)
        });

    let Some(session) = session else {
        let block = Block::default()
            .title(" Historical session ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.palette.overlay0));
        frame.render_widget(
            Paragraph::new("This session is no longer available in the current snapshot.")
                .block(block)
                .style(Style::default().fg(app.palette.subtext0))
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    };

    let label = super::session_label::session_label(&session.title, session.topic_label.as_deref());
    let full_context = app.projects.history_view == crate::app::state::ProjectHistoryView::Full;
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                format!("{} ", session.backend),
                Style::default().fg(backend_color(app, &session.backend)),
            ),
            Span::styled(
                session.cwd.as_deref().unwrap_or("unknown cwd").to_string(),
                Style::default().fg(app.palette.overlay0),
            ),
        ]),
        Line::from(""),
    ];

    let transcript = if full_context {
        crate::projects::transcript::read_full_transcript(
            &session.backend,
            session.transcript_ref.as_deref(),
        )
    } else {
        crate::projects::transcript::read_transcript(
            &session.backend,
            session.transcript_ref.as_deref(),
        )
    };
    match transcript {
        Ok(transcript) => {
            for message in &transcript.messages {
                // Keep the speaker marker and the body on the same visual track. Plain assistant
                // prose is common, so relying only on Markdown syntax would make the whole reply
                // look like unstyled white text again.
                let (marker, accent) = match message.role {
                    crate::projects::transcript::TranscriptRole::User => {
                        (" you ", app.palette.accent)
                    }
                    crate::projects::transcript::TranscriptRole::Assistant => {
                        (" agent ", app.palette.green)
                    }
                };
                let body_style = match message.role {
                    crate::projects::transcript::TranscriptRole::User => {
                        Style::default().fg(app.palette.text)
                    }
                    crate::projects::transcript::TranscriptRole::Assistant => {
                        Style::default().fg(app.palette.green)
                    }
                };
                let render_body = |text: &str| match message.role {
                    crate::projects::transcript::TranscriptRole::User => {
                        super::markdown::markdown_lines(text, &app.palette)
                    }
                    crate::projects::transcript::TranscriptRole::Assistant => {
                        super::markdown::markdown_lines_with_base(text, &app.palette, body_style)
                    }
                };
                lines.push(Line::from(vec![
                    Span::styled(
                        marker,
                        // `panel_contrast_fg` rather than `panel_bg` directly: the tab bar's
                        // active chip solves the same "text on an accent fill" problem, and it
                        // handles a `Reset` panel background that would otherwise be invisible.
                        Style::default()
                            .fg(super::widgets::panel_contrast_fg(&app.palette))
                            .bg(accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(""),
                ]));
                if full_context {
                    // Entering history is intentionally read-only until submission, but it must
                    // preserve the complete readable conversation rather than the preview's
                    // three-line summary.
                    lines.extend(render_body(&message.text));
                } else {
                    let (excerpt, elided) =
                        crate::projects::transcript::preview_excerpt(&message.text, message.role);
                    lines.extend(render_body(&excerpt));
                    if elided {
                        lines.push(Line::from(Span::styled(
                            "  …",
                            Style::default().fg(app.palette.overlay0),
                        )));
                    }
                }
                lines.push(Line::from(""));
            }
            if transcript.truncated {
                lines.push(Line::from(Span::styled(
                    "… earlier turns not shown",
                    Style::default().fg(app.palette.overlay0),
                )));
            }
        }
        Err(error) => {
            lines.push(Line::from(Span::styled(
                error.message(),
                Style::default().fg(app.palette.peach),
            )));
            // A failed resume is why the user landed here, so keep explaining it rather than
            // replacing that context with the read failure alone.
            if let Some(reason) = app.projects.history_fallback_reason.as_deref() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    reason.to_string(),
                    Style::default().fg(app.palette.overlay0),
                )));
            }
        }
    }

    if !full_context {
        lines.push(Line::from(vec![
            Span::styled(
                "Esc",
                Style::default()
                    .fg(app.palette.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" back   ", Style::default().fg(app.palette.overlay0)),
            Span::styled(
                "Enter / click again",
                Style::default()
                    .fg(app.palette.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " open full context (agent stays stopped)",
                Style::default().fg(app.palette.overlay0),
            ),
        ]));
    }

    let title = label.topic.map_or_else(
        || format!(" {} ", label.task),
        |topic| format!(" 【{topic}】{} ", label.task),
    );
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.palette.overlay0));
    if full_context {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let [conversation_area, composer_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).areas(inner);
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((app.projects.history_scroll.min(u16::MAX as usize) as u16, 0)),
            conversation_area,
        );
        let draft = if app.projects.history_draft.is_empty() {
            Line::from(Span::styled(
                "Type a message…",
                Style::default().fg(app.palette.overlay0),
            ))
        } else {
            Line::from(app.projects.history_draft.clone())
        };
        let composer_block = Block::default()
            .title(" message · Enter sends + resumes · Shift+Enter newline · Esc back ")
            .borders(Borders::TOP)
            .border_style(Style::default().fg(app.palette.overlay0));
        let composer_inner = composer_block.inner(composer_area);
        let (composer_scroll, cursor_column, cursor_row) = history_composer_cursor(
            &app.projects.history_draft,
            composer_inner.width,
            composer_inner.height,
        );
        frame.render_widget(
            Paragraph::new(draft)
                .block(composer_block)
                .style(Style::default().fg(app.palette.text))
                .wrap(Wrap { trim: false })
                .scroll((composer_scroll, 0)),
            composer_area,
        );
        frame.set_cursor_position((
            composer_inner.x.saturating_add(cursor_column),
            composer_inner.y.saturating_add(cursor_row),
        ));
    } else {
        frame.render_widget(
            Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false })
                .scroll((app.projects.history_scroll.min(u16::MAX as usize) as u16, 0)),
            area,
        );
    }
}

/// Cursor geometry for the historical-session composer.
///
/// A real frame cursor is required for terminal IMEs: without an anchor macOS composition can
/// accept keystrokes internally while showing no candidate window or committed text, which reads
/// to the user as an input box that cannot be typed into.
fn history_composer_cursor(draft: &str, width: u16, height: u16) -> (u16, u16, u16) {
    let width = width.max(1) as usize;
    let height = height.max(1) as usize;
    let mut row = 0usize;
    let mut column = 0usize;

    for character in draft.chars() {
        if character == '\n' {
            row = row.saturating_add(1);
            column = 0;
            continue;
        }
        let character_width = super::text::display_width(&character.to_string()).max(1);
        if column.saturating_add(character_width) > width {
            row = row.saturating_add(1);
            column = 0;
        }
        column = column.saturating_add(character_width);
        if column >= width {
            row = row.saturating_add(column / width);
            column %= width;
        }
    }

    let scroll = row.saturating_sub(height.saturating_sub(1));
    (
        scroll.min(u16::MAX as usize) as u16,
        column.min(width.saturating_sub(1)) as u16,
        row.saturating_sub(scroll).min(height.saturating_sub(1)) as u16,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::{
        AdapterScanStatus, IndexedSessionSummary, ProjectSummary, ProjectsSnapshot, SessionRefKind,
    };

    fn snapshot() -> ProjectsSnapshot {
        ProjectsSnapshot {
            projects_schema_version: 1,
            revision: 1,
            projects: vec![ProjectSummary {
                canonical_key: "p1".into(),
                kind: ProjectKind::Cwd,
                display_name: "ait".into(),
                canonical_path: "/tmp/ait".into(),
                sessions: vec![IndexedSessionSummary {
                    stable_key: "s1".into(),
                    backend: "codex".into(),
                    ref_kind: SessionRefKind::Id,
                    ref_value: "s1".into(),
                    title: "Dense Tree".into(),
                    cwd: Some("/tmp/ait".into()),
                    first_activity_at: 1,
                    last_activity_at: 2,
                    live: true,
                    workspace_id: Some("workspace-1".into()),
                    pane_id: Some("workspace-1:p1".into()),
                    runtime_generation: Some(4),
                    session_class: crate::projects::SessionClass::Interactive,
                    topic_label: None,
                    transcript_ref: None,
                }],
                automation: Vec::new(),
                thin_count: 0,
                next_cursor: None,
            }],
            topics: Vec::new(),
            scan_status: Vec::new(),
            diagnostic_category: None,
        }
    }

    #[test]
    fn filters_are_one_row_and_share_geometry_with_hit_testing() {
        let mut state = AppState::test_new();
        state.projects.snapshot = snapshot();
        let geometry = project_sidebar_geometry(&state, Rect::new(0, 0, 30, 12));
        assert!(geometry.sidebar_tabs.iter().all(|rect| rect.height == 1));
        assert_eq!(geometry.sidebar_tabs[0].y, geometry.filter_tabs[0].y - 1);
        assert_eq!(geometry.sidebar_tabs[3].right(), 29);
        assert!(
            geometry.sidebar_tabs[1].x > geometry.sidebar_tabs[0].right(),
            "Sessions/Projects/Clusters tabs must not abut"
        );
        assert!(geometry.filter_tabs.iter().all(|rect| rect.height == 1));
        assert!(
            geometry.filter_tabs[1].x > geometry.filter_tabs[0].right(),
            "all/live/unclass chips paint backgrounds and must not abut"
        );
        assert!(
            geometry.filter_tabs[2].x > geometry.filter_tabs[1].right(),
            "all/live/unclass chips paint backgrounds and must not abut"
        );
        assert!(
            geometry.filter_tabs[2].right() <= geometry.sidebar_tabs[3].right(),
            "filter chips must stay inside the sidebar content column"
        );
        assert_eq!(geometry.row_hits.len(), 2);
        assert_eq!(geometry.row_hits[0].rect.height, 1);
    }

    #[test]
    fn sessions_tab_is_a_flat_activity_sorted_session_list() {
        let mut state = AppState::test_new();
        state.sidebar_view = crate::app::state::SidebarView::Sessions;
        let mut snapshot = snapshot();
        let mut newer = snapshot.projects[0].sessions[0].clone();
        newer.stable_key = "s-newer".into();
        newer.ref_value = "s-newer".into();
        newer.title = "Newer session".into();
        newer.last_activity_at = 30;
        newer.live = false;
        newer.workspace_id = None;
        newer.pane_id = None;
        newer.runtime_generation = None;

        let mut duplicate = newer.clone();
        duplicate.title = "Duplicate copy".into();
        duplicate.last_activity_at = 40;

        let mut other_project = snapshot.projects[0].clone();
        other_project.canonical_key = "p2".into();
        other_project.display_name = "other".into();
        other_project.canonical_path = "/tmp/other".into();
        other_project.sessions = vec![duplicate, newer];
        snapshot.projects[0].sessions.push(newer_session());
        snapshot.projects.push(other_project);
        state.projects.snapshot = snapshot;

        let rows = project_tree_rows(&state);
        assert!(rows
            .iter()
            .all(|row| matches!(row, ProjectTreeRow::Session(_))));
        let sessions = rows
            .iter()
            .filter_map(|row| match row {
                ProjectTreeRow::Session(session) => Some(session),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(sessions.len(), 3, "duplicate stable keys must be collapsed");
        assert_eq!(sessions[0].stable_key, "s-newer");
        assert_eq!(sessions[0].last_activity_at, 40);
        assert_eq!(sessions[1].stable_key, "s2");
        assert_eq!(sessions[1].last_activity_at, 10);
        assert_eq!(sessions[2].stable_key, "s1");
    }

    fn newer_session() -> IndexedSessionSummary {
        IndexedSessionSummary {
            stable_key: "s2".into(),
            backend: "claude".into(),
            ref_kind: SessionRefKind::Id,
            ref_value: "s2".into(),
            title: "Older session".into(),
            cwd: Some("/tmp/ait".into()),
            first_activity_at: 1,
            last_activity_at: 10,
            live: false,
            workspace_id: None,
            pane_id: None,
            runtime_generation: None,
            session_class: crate::projects::SessionClass::Interactive,
            topic_label: None,
            transcript_ref: None,
        }
    }

    #[test]
    fn sessions_tab_applies_live_unclassified_and_search_filters() {
        let mut state = AppState::test_new();
        state.sidebar_view = crate::app::state::SidebarView::Sessions;
        let mut snapshot = snapshot();
        let mut historical = newer_session();
        historical.topic_label = Some("topic".into());
        historical.title = "Historical topic".into();
        snapshot.projects[0].sessions.push(historical);
        state.projects.snapshot = snapshot;

        state.projects.filter = ProjectFilter::Live;
        assert!(project_tree_rows(&state)
            .iter()
            .all(|row| { matches!(row, ProjectTreeRow::Session(session) if session.live) }));

        state.projects.filter = ProjectFilter::Unclassified;
        assert!(project_tree_rows(&state).iter().all(|row| {
            matches!(row, ProjectTreeRow::Session(session) if session.topic_label.is_none())
        }));

        state.projects.filter = ProjectFilter::All;
        state.projects.query = "historical".into();
        let rows = project_tree_rows(&state);
        assert!(
            matches!(&rows[..], [ProjectTreeRow::Session(session)] if session.title == "Historical topic")
        );
    }

    /// Builds a state whose single session is genuinely mapped to the focused pane, so `Open` is
    /// exercised against real workspace/pane identity rather than a hand-written id string.
    fn state_with_open_session() -> AppState {
        let mut state = AppState::test_new();
        state.workspaces = vec![crate::workspace::Workspace::test_new("project-live")];
        state.ensure_test_terminals();
        state.active = Some(0);
        state.selected = 0;

        let ws = &state.workspaces[0];
        let workspace_id = ws.id.clone();
        let focused = ws.focused_pane_id().expect("focused pane");
        let public_pane_id = crate::workspace::public_pane_id_for_number(
            &workspace_id,
            ws.public_pane_number(focused).expect("public pane number"),
        );

        let mut snapshot = snapshot();
        let session = &mut snapshot.projects[0].sessions[0];
        session.live = true;
        session.workspace_id = Some(workspace_id);
        session.pane_id = Some(public_pane_id);
        session.runtime_generation = Some(9);
        state.projects.snapshot = snapshot;
        state
    }

    /// One rendered cell, reduced to what the highlight tests care about.
    struct RenderedCell {
        symbol: String,
        fg: Option<ratatui::style::Color>,
        bg: Option<ratatui::style::Color>,
    }

    fn row_cells(state: &AppState, area: Rect) -> Vec<Vec<RenderedCell>> {
        let backend = ratatui::backend::TestBackend::new(area.width, area.height);
        let mut terminal = ratatui::Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render_projects_sidebar(state, frame, area))
            .expect("draw sidebar");
        let buffer = terminal.backend().buffer();
        (area.y..area.bottom())
            .map(|y| {
                (area.x..area.right())
                    .map(|x| {
                        let cell = &buffer[(x, y)];
                        RenderedCell {
                            symbol: cell.symbol().to_string(),
                            fg: cell.style().fg,
                            bg: cell.style().bg,
                        }
                    })
                    .collect()
            })
            .collect()
    }

    /// Joins a rendered row back into text.
    ///
    /// A wide glyph occupies two buffer cells: the first holds the character, the second an empty
    /// continuation. Those continuations must be dropped, or every CJK character comes back with a
    /// space wedged after it and no substring match can ever succeed.
    fn rendered_text(state: &AppState, area: Rect) -> String {
        row_cells(state, area)
            .iter()
            .map(|row| {
                let mut text = String::new();
                let mut skip = 0usize;
                for cell in row {
                    if skip > 0 {
                        skip -= 1;
                        continue;
                    }
                    skip = crate::ui::text::display_width(&cell.symbol).saturating_sub(1);
                    text.push_str(&cell.symbol);
                }
                text
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Background of a cell that actually holds a glyph.
    ///
    /// The accent bar and blank padding are skipped on purpose: a highlight only counts if it
    /// survives *behind the row text*, which is exactly what the previous assertion missed.
    fn text_cell_background(row: &[RenderedCell]) -> Option<ratatui::style::Color> {
        row.iter()
            .find(|cell| !cell.symbol.trim().is_empty() && cell.symbol != "▎")
            .and_then(|cell| cell.bg)
    }

    /// Regression guard for the reported "clicking a session shows no highlight".
    ///
    /// The row background was painted first and then an unstyled `Paragraph` was rendered over the
    /// same rect, which resets every cell the glyphs occupy. The accent bar survived because it is
    /// written after the text, which is why the earlier assertion did not catch this.
    #[test]
    fn highlight_survives_the_row_text_render() {
        let mut state = state_with_open_session();

        for mode in [crate::app::Mode::Navigate, crate::app::Mode::Terminal] {
            state.mode = mode;
            let rows = row_cells(&state, Rect::new(0, 0, 30, 12));
            let open_row = rows
                .iter()
                .find(|row| row[0].symbol == "▎")
                .unwrap_or_else(|| panic!("open row must render in {mode:?}"));

            assert_eq!(
                text_cell_background(open_row),
                Some(state.palette.surface1),
                "in {mode:?} the highlight must still be behind the row text, not only the bar"
            );
        }
    }

    /// The fourth report of "clicking a session shows no highlight".
    ///
    /// Clicking switches the mode to `Terminal` (live pane) or `ProjectHistory` (read-only
    /// preview), so a highlight gated on `Navigate` disappeared exactly when the row became the
    /// current one.
    #[test]
    fn the_history_session_stays_marked_in_project_history_mode() {
        let mut state = AppState::test_new();
        let mut snapshot = snapshot();
        let session = &mut snapshot.projects[0].sessions[0];
        // A historical session: no runtime mapping at all.
        session.live = false;
        session.workspace_id = None;
        session.pane_id = None;
        session.runtime_generation = None;
        let key = session.stable_key.clone();
        state.projects.snapshot = snapshot;
        state.projects.history_session_key = Some(key);
        state.mode = crate::app::Mode::ProjectHistory;

        let rows = row_cells(&state, Rect::new(0, 0, 40, 12));
        let marked = rows
            .iter()
            .find(|row| row[0].symbol == "▎")
            .expect("the previewed session must keep its accent bar");
        assert_eq!(
            text_cell_background(marked),
            Some(state.palette.surface1),
            "the previewed session must stay filled behind its text"
        );
    }

    /// A one-line row differing only in background was reported as "no highlight" four times, so
    /// the current row also grows a status line.
    #[test]
    fn the_current_session_row_expands_with_a_status_line() {
        let mut state = state_with_open_session();
        state.mode = crate::app::Mode::Terminal;

        let rows = project_tree_rows(&state);
        let tree = project_sidebar_geometry(&state, Rect::new(0, 0, 40, 12)).tree;
        let layout = visible_row_layout(&state, &rows, tree, 0);
        let current = layout
            .iter()
            .find(|(row, _)| {
                matches!(row, ProjectTreeRow::Session(session) if is_current_session(&state, session))
            })
            .expect("an open session must be laid out");
        assert_eq!(
            current.1.height, 2,
            "the current row must be two lines tall"
        );

        // Hit testing must agree, or a click lands on a neighbour.
        let geometry = project_sidebar_geometry(&state, Rect::new(0, 0, 40, 12));
        let hit = geometry
            .row_hits
            .iter()
            .find(|hit| hit.rect.y == current.1.y)
            .expect("the expanded row must still be clickable");
        assert_eq!(hit.rect.height, 2);

        let text = rendered_text(&state, Rect::new(0, 0, 40, 12));
        assert!(
            text.contains("idle") || text.contains("done") || text.contains("working"),
            "the status line must name an agent state:\n{text}"
        );
    }

    /// A click below an expanded row must select that row, not its neighbour.
    ///
    /// The click handler derived the index from `scroll + (mouse.row - tree.y)`, which assumed
    /// every row is one line. Once the current session's row grew to two, every row under it
    /// resolved one short — so clicking a different session re-selected the one already open and
    /// the preview appeared not to change.
    #[test]
    fn a_click_below_an_expanded_row_resolves_to_the_right_row() {
        let mut state = state_with_open_session();
        let mut snapshot = state.projects.snapshot.clone();
        // A second session below the open one.
        let mut sibling = snapshot.projects[0].sessions[0].clone();
        sibling.stable_key = "s-below".into();
        sibling.title = "第二个会话".into();
        sibling.live = false;
        sibling.workspace_id = None;
        sibling.pane_id = None;
        sibling.runtime_generation = None;
        snapshot.projects[0].sessions.push(sibling);
        state.projects.snapshot = snapshot;

        let geometry = project_sidebar_geometry(&state, Rect::new(0, 0, 40, 12));
        let rows = project_tree_rows(&state);
        let expanded = geometry
            .row_hits
            .iter()
            .find(|hit| hit.rect.height == 2)
            .expect("the open session must be expanded");
        let below = geometry
            .row_hits
            .iter()
            .find(|hit| hit.rect.y > expanded.rect.y)
            .expect("a row must follow the expanded one");

        // The row index the hit carries must be the row actually laid out there — not what naive
        // one-line-per-row arithmetic would produce.
        let naive = usize::from(below.rect.y - geometry.tree.y);
        assert_ne!(
            below.row_index, naive,
            "this fixture must exercise the off-by-one, otherwise it proves nothing"
        );
        assert!(matches!(
            rows.get(below.row_index),
            Some(ProjectTreeRow::Session(_))
        ));
    }

    /// A row that is neither current nor under the cursor stays one line, or the tree wastes the
    /// sidebar's height.
    #[test]
    fn ordinary_rows_stay_one_line() {
        let mut state = AppState::test_new();
        state.projects.snapshot = snapshot();
        state.mode = crate::app::Mode::Terminal;
        let rows = project_tree_rows(&state);
        let tree = project_sidebar_geometry(&state, Rect::new(0, 0, 40, 12)).tree;
        for (_, rect) in visible_row_layout(&state, &rows, tree, 0) {
            assert_eq!(rect.height, 1);
        }
    }

    /// The core of the reported bug: the old code gated every highlight on `Mode::Navigate`, so
    /// stepping into the terminal erased any trace of which session was on screen.
    #[test]
    fn open_session_row_stays_marked_outside_navigate_mode() {
        let mut state = state_with_open_session();
        let area = Rect::new(0, 0, 30, 12);

        for mode in [crate::app::Mode::Navigate, crate::app::Mode::Terminal] {
            state.mode = mode;
            let rows = row_cells(&state, area);
            let bar = rows
                .iter()
                .find(|row| row[0].symbol == "▎")
                .unwrap_or_else(|| panic!("open session must keep its accent bar in {mode:?}"));
            assert_eq!(
                bar[0].fg,
                Some(state.palette.accent),
                "the bar must use the accent colour in {mode:?}"
            );
        }
    }

    /// A partial runtime mapping is read-only history per `ProjectTreeRow::action`, so it must not
    /// claim to be the open session either.
    #[test]
    fn incomplete_runtime_mapping_is_not_open() {
        let mut state = state_with_open_session();
        state.mode = crate::app::Mode::Terminal;
        state.projects.snapshot.projects[0].sessions[0].runtime_generation = None;

        let rows = row_cells(&state, Rect::new(0, 0, 30, 12));
        assert!(
            !rows.iter().any(|row| row[0].symbol == "▎"),
            "a session without a full runtime mapping must not render as open"
        );
    }

    /// A session mapped to a pane that is not the focused one is live but not open.
    #[test]
    fn unfocused_live_session_is_not_open() {
        let mut state = state_with_open_session();
        state.mode = crate::app::Mode::Terminal;
        state.projects.snapshot.projects[0].sessions[0].pane_id =
            Some("project-live:pZZ".to_string());

        let rows = row_cells(&state, Rect::new(0, 0, 30, 12));
        assert!(
            !rows.iter().any(|row| row[0].symbol == "▎"),
            "only the focused pane's session may render as open"
        );
    }

    /// Cursor and open must be told apart, otherwise "where am I" and "what is running" collapse
    /// into one indistinguishable highlight.
    #[test]
    fn cursor_and_open_rows_use_distinct_backgrounds() {
        let mut state = state_with_open_session();
        state.mode = crate::app::Mode::Navigate;
        // Row 0 is the project header, row 1 the open session: park the cursor on the header so
        // the two states land on different rows.
        state.projects.selected_row = 0;

        let rows = row_cells(&state, Rect::new(0, 0, 30, 12));
        // Read both backgrounds off a cell that actually holds a glyph. The previous version took
        // "the first cell in the row with any background", which the accent bar satisfied — so it
        // passed even while the row text was rendering over the highlight and erasing it.
        let tree_top = usize::from(
            project_sidebar_geometry(&state, Rect::new(0, 0, 30, 12))
                .tree
                .y,
        );
        let cursor_bg = text_cell_background(&rows[tree_top])
            .expect("cursor row must paint a background behind its text");
        let open_row = rows
            .iter()
            .position(|row| row[0].symbol == "▎")
            .expect("open row must render its accent bar");
        let open_bg = text_cell_background(&rows[open_row])
            .expect("open row must paint a background behind its text");

        assert_ne!(
            open_row, tree_top,
            "cursor and open must land on different rows for this comparison to mean anything"
        );
        assert_ne!(
            cursor_bg, open_bg,
            "cursor and open rows must be visually distinguishable"
        );
    }

    /// A topic groups dozens of sessions — the three largest on this machine cover 63, 54 and 51 —
    /// so printing it on every row repeats the group name instead of telling the rows apart. The
    /// tree's parent node already states the grouping, and the row keeps the width for its title.
    #[test]
    fn session_rows_do_not_repeat_the_group_topic() {
        let mut state = AppState::test_new();
        let mut snapshot = snapshot();
        let session = &mut snapshot.projects[0].sessions[0];
        session.title = "看下 mihomo 的分流规则怎么配".to_string();
        session.topic_label = Some("proxy 网络配置".to_string());
        state.projects.snapshot = snapshot;

        let text = rendered_text(&state, Rect::new(0, 0, 60, 12));
        assert!(
            !text.contains("【proxy 网络配置】"),
            "the group topic must not be repeated on every row:\n{text}"
        );
        assert!(text.contains("mihomo"), "task text must survive:\n{text}");
    }

    #[test]
    fn directory_rows_show_subject_label_but_cluster_rows_hide_it() {
        let mut state = AppState::test_new();
        let mut snapshot = snapshot();
        snapshot.projects[0].sessions[0].title = "【简历】整理腾讯 WorkBuddy 面试准备".into();
        let mut topic = snapshot.projects[0].clone();
        topic.kind = ProjectKind::Semantic;
        topic.display_name = "面试准备".into();
        topic.sessions[0].topic_label = Some("面试准备".into());
        snapshot.topics = vec![topic];
        state.projects.snapshot = snapshot;

        state.sidebar_view = crate::app::state::SidebarView::Projects;
        let projects_text = rendered_text(&state, Rect::new(0, 0, 80, 12));
        assert!(
            projects_text.contains("简历"),
            "Projects should show the subject label:\n{projects_text}"
        );
        assert!(
            projects_text.contains("整理腾讯"),
            "Projects should show the task text:\n{projects_text}"
        );

        state.sidebar_view = crate::app::state::SidebarView::Clusters;
        let clusters_text = rendered_text(&state, Rect::new(0, 0, 80, 12));
        assert!(
            !clusters_text.contains("简历"),
            "Clusters should not repeat the subject label:\n{clusters_text}"
        );
        assert!(
            clusters_text.contains("整理腾讯"),
            "Clusters should keep the task text:\n{clusters_text}"
        );
    }

    /// The reported naming failures, asserted end to end through the renderer rather than only
    /// against the cleaning function.
    #[test]
    fn session_row_drops_resume_noise_and_folds_a_doubled_paste() {
        let mut state = AppState::test_new();
        let mut snapshot = snapshot();
        snapshot.projects[0].sessions[0].title =
            "看下这个看下这个 grok 项目 grok --resume 01a01579-2b40-7e01-86b0-730297b95b12"
                .to_string();
        state.projects.snapshot = snapshot;

        let text = rendered_text(&state, Rect::new(0, 0, 60, 12));
        assert!(!text.contains("--resume"), "resume flag leaked:\n{text}");
        assert!(!text.contains("01a01579"), "session uuid leaked:\n{text}");
        assert!(
            !text.contains("看下这个看下这个"),
            "doubled paste was not folded:\n{text}"
        );
    }

    /// A stored title is capped at 96 *characters*; CJK is two columns each, so rendering it
    /// verbatim would ask for ~192 columns.
    ///
    /// Asserting that nothing spills past the sidebar edge would prove nothing — the terminal
    /// clips at the viewport regardless. This renders into a viewport far wider than the title
    /// budget, so an unclipped title would have room to show itself, and pins the width instead.
    #[test]
    fn long_cjk_title_is_clipped_by_width_before_it_reaches_the_row() {
        let mut state = AppState::test_new();
        let mut snapshot = snapshot();
        snapshot.projects[0].sessions[0].title = "看".repeat(96);
        state.projects.snapshot = snapshot;

        let area = Rect::new(0, 0, 200, 12);
        let rows = row_cells(&state, area);
        let session_row = rows
            .iter()
            .find(|row| row.iter().any(|cell| cell.symbol == "●"))
            .expect("session row");
        // Measure to the last painted glyph. Trimming the joined string is not enough: the row
        // divider and background fill occupy trailing cells.
        let painted = session_row
            .iter()
            .rposition(|cell| !cell.symbol.trim().is_empty() && cell.symbol != "│")
            .map_or(0, |index| index + 1);
        assert!(
            painted < 96,
            "an unclipped 192-column title reached the row: {painted} columns"
        );
    }

    /// `surface_dim` and `panel_bg` are background slots. `surface_dim` sits at roughly 1.1:1
    /// against `panel_bg`, so any glyph drawn in either colour is invisible on the panel. This
    /// walks every cell the Projects sidebar paints and refuses both slots as a foreground.
    ///
    /// The snapshot deliberately carries every row variant — thin, automation, load-older and
    /// scan status — because a guard that never renders a variant cannot protect it.
    #[test]
    fn no_foreground_uses_a_background_palette_slot() {
        let mut state = AppState::test_new();
        let mut snapshot = snapshot();
        let project = &mut snapshot.projects[0];
        project.thin_count = 1;
        project.sessions.push(IndexedSessionSummary {
            stable_key: "s-thin".into(),
            backend: "codex".into(),
            ref_kind: SessionRefKind::Id,
            ref_value: "s-thin".into(),
            title: "hi".into(),
            cwd: Some("/tmp/ait".into()),
            first_activity_at: 1,
            last_activity_at: 1,
            live: false,
            workspace_id: None,
            pane_id: None,
            runtime_generation: None,
            session_class: crate::projects::SessionClass::Interactive,
            topic_label: None,
            transcript_ref: None,
        });
        project.automation.push(AutomationTemplateSummary {
            representative_session_key: "s-auto".into(),
            title: "watchdog".into(),
            backend: "codex".into(),
            count: 12,
            last_activity_at: 3,
        });
        project.next_cursor = Some(crate::projects::SessionCursor {
            last_activity_at: 1,
            stable_key: "s-thin".into(),
        });
        snapshot.scan_status = vec![AdapterScanStatus {
            adapter: "codex".into(),
            state: "scanning".into(),
            diagnostic_category: None,
        }];
        state.projects.snapshot = snapshot;

        for mode in [crate::app::Mode::Navigate, crate::app::Mode::Terminal] {
            state.mode = mode;
            let area = Rect::new(0, 0, 30, 14);
            let backend = ratatui::backend::TestBackend::new(30, 14);
            let mut terminal = ratatui::Terminal::new(backend).expect("test terminal");
            terminal
                .draw(|frame| render_projects_sidebar(&state, frame, area))
                .expect("draw sidebar");

            let buffer = terminal.backend().buffer();
            let rendered = (area.y..area.bottom())
                .map(|y| {
                    (area.x..area.right())
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                rendered.contains("short sessions"),
                "thin row must render so the guard covers it:\n{rendered}"
            );
            assert!(
                rendered.contains("Load older"),
                "load-older row must render so the guard covers it:\n{rendered}"
            );

            for y in area.y..area.bottom() {
                for x in area.x..area.right() {
                    let cell = &buffer[(x, y)];
                    if cell.symbol().trim().is_empty() {
                        continue;
                    }
                    let fg = cell.style().fg;
                    assert_ne!(
                        fg,
                        Some(state.palette.surface_dim),
                        "cell ({x},{y}) {:?} in {mode:?} draws with the surface_dim background slot",
                        cell.symbol()
                    );
                    assert_ne!(
                        fg,
                        Some(state.palette.panel_bg),
                        "cell ({x},{y}) {:?} in {mode:?} draws with the panel_bg background slot",
                        cell.symbol()
                    );
                }
            }
        }
    }

    #[test]
    fn thin_row_is_clickable_and_expands_sessions() {
        let mut state = AppState::test_new();
        let mut snapshot = snapshot();
        snapshot.projects[0].thin_count = 1;
        snapshot.projects[0].sessions.push(IndexedSessionSummary {
            stable_key: "s-thin".into(),
            backend: "codex".into(),
            ref_kind: SessionRefKind::Id,
            ref_value: "s-thin".into(),
            title: "hi".into(),
            cwd: Some("/tmp/ait".into()),
            first_activity_at: 1,
            last_activity_at: 1,
            live: false,
            workspace_id: None,
            pane_id: None,
            runtime_generation: None,
            session_class: crate::projects::SessionClass::Interactive,
            topic_label: None,
            transcript_ref: None,
        });
        state.projects.snapshot = snapshot;
        let rows = project_tree_rows(&state);
        assert!(matches!(
            rows.last(),
            Some(ProjectTreeRow::Thin {
                project_key,
                count: 1
            }) if project_key == "p1"
        ));
        assert!(matches!(
            rows.last().and_then(ProjectTreeRow::action),
            Some(ProjectTreeAction::ToggleThin { ref project_key }) if project_key == "p1"
        ));

        state.projects.expanded_thin_keys.insert("p1".into());
        let expanded = project_tree_rows(&state);
        assert_eq!(
            expanded
                .iter()
                .filter(|row| matches!(row, ProjectTreeRow::Session(_)))
                .count(),
            2
        );
        assert!(!expanded
            .iter()
            .any(|row| matches!(row, ProjectTreeRow::Thin { .. })));
    }

    #[test]
    fn live_leaf_has_one_typed_focus_activation() {
        let mut state = AppState::test_new();
        state.projects.snapshot = snapshot();
        let rows = project_tree_rows(&state);
        assert!(matches!(
            rows[1].action(),
            Some(ProjectTreeAction::Activate(ProjectSessionActivation::Live {
                ref workspace_id,
                ref pane_id,
                runtime_generation: 4,
                ..
            })) if workspace_id == "workspace-1" && pane_id == "workspace-1:p1"
        ));
    }

    #[test]
    fn incomplete_live_mapping_is_read_only_history() {
        let mut state = AppState::test_new();
        let mut snapshot = snapshot();
        snapshot.projects[0].sessions[0].runtime_generation = None;
        state.projects.snapshot = snapshot;
        assert!(matches!(
            project_tree_rows(&state)[1].action(),
            Some(ProjectTreeAction::Activate(
                ProjectSessionActivation::History { .. }
            ))
        ));
    }

    /// The preview must show the conversation, not a metadata card.
    #[test]
    fn history_preview_renders_the_real_conversation() {
        use std::io::Write as _;

        let dir = std::env::temp_dir().join("ork3-history-preview");
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("transcript.jsonl");
        let mut file = std::fs::File::create(&path).expect("create");
        writeln!(
            file,
            r#"{{"type":"response_item","payload":{{"role":"user","content":[{{"type":"text","text":"看下天气"}}]}}}}"#
        )
        .expect("write");
        writeln!(
            file,
            r#"{{"type":"response_item","payload":{{"role":"assistant","content":[{{"type":"text","text":"今天晴朗"}}]}}}}"#
        )
        .expect("write");

        let mut state = AppState::test_new();
        let mut snapshot = snapshot();
        let session = &mut snapshot.projects[0].sessions[0];
        session.transcript_ref = Some(path.to_string_lossy().into_owned());
        let session_key = session.stable_key.clone();
        state.projects.snapshot = snapshot;
        state.projects.history_session_key = Some(session_key);

        let area = Rect::new(0, 0, 60, 20);
        let backend = ratatui::backend::TestBackend::new(area.width, area.height);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| render_project_history(&state, frame, area))
            .expect("draw");
        let buffer = terminal.backend().buffer();
        let text = (area.y..area.bottom())
            .map(|y| {
                let mut line = String::new();
                let mut skip = 0usize;
                for x in area.x..area.right() {
                    if skip > 0 {
                        skip -= 1;
                        continue;
                    }
                    let symbol = buffer[(x, y)].symbol();
                    skip = crate::ui::text::display_width(symbol).saturating_sub(1);
                    line.push_str(symbol);
                }
                line
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.contains("看下天气"), "user turn missing:\n{text}");
        assert!(text.contains("今天晴朗"), "assistant turn missing:\n{text}");
        assert!(text.contains("you"), "role markers missing:\n{text}");
        assert!(
            text.contains("click again"),
            "full-context affordance missing:\n{text}"
        );
        assert!(
            text.contains("stays stopped"),
            "preview must explain that navigation is process-free:\n{text}"
        );
        let assistant_cell = (area.y..area.bottom()).find_map(|y| {
            (area.x..area.right())
                .find_map(|x| (buffer[(x, y)].symbol() == "今").then_some(buffer[(x, y)].fg))
        });
        assert_eq!(
            assistant_cell,
            Some(state.palette.green),
            "assistant prose should use the agent body colour"
        );
    }

    #[test]
    fn full_history_renders_lines_that_the_preview_elides() {
        use std::io::Write as _;

        let dir = std::env::temp_dir().join("ork3-history-full-context");
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("transcript.jsonl");
        let mut file = std::fs::File::create(&path).expect("create");
        let body = "line one\nline two\nline three\nFULL CONTEXT SENTINEL";
        let record = serde_json::json!({
            "type": "response_item",
            "payload": {"role": "user", "content": [{"type": "text", "text": body}]},
        });
        writeln!(file, "{record}").expect("write");

        let mut state = AppState::test_new();
        let mut snapshot = snapshot();
        let session = &mut snapshot.projects[0].sessions[0];
        session.transcript_ref = Some(path.to_string_lossy().into_owned());
        state.projects.history_session_key = Some(session.stable_key.clone());
        state.projects.snapshot = snapshot;
        state.projects.history_view = crate::app::state::ProjectHistoryView::Full;

        let area = Rect::new(0, 0, 70, 20);
        let backend = ratatui::backend::TestBackend::new(area.width, area.height);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| render_project_history(&state, frame, area))
            .expect("draw");
        let text = (area.y..area.bottom())
            .map(|y| {
                (area.x..area.right())
                    .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            text.contains("FULL CONTEXT SENTINEL"),
            "the entered session must show text beyond the three-line preview:\n{text}"
        );
        assert!(
            text.contains("Enter sends + resumes"),
            "composer missing:\n{text}"
        );
    }

    #[test]
    fn full_history_exposes_a_real_composer_cursor_for_ime_input() {
        let mut state = AppState::test_new();
        state.projects.snapshot = snapshot();
        state.projects.history_session_key = Some("s1".into());
        state.projects.history_view = crate::app::state::ProjectHistoryView::Full;
        state.projects.history_draft = "中文".into();
        state.mode = crate::app::Mode::ProjectHistory;

        let area = Rect::new(0, 0, 60, 16);
        let (_buffer, cursor) =
            crate::server::render_stream::render_virtual(&mut state, area, false);

        assert!(
            cursor.is_some_and(|cursor| cursor.visible),
            "the composer must publish a visible host cursor so terminal IMEs can commit text"
        );
    }

    /// A backend without a transcript file must say so rather than render an empty conversation
    /// that reads like data loss.
    /// The preview must show the agent's Markdown the way its own CLI does, not the source marks.
    #[test]
    fn history_preview_renders_markdown_instead_of_its_source_marks() {
        use std::io::Write as _;

        let dir = std::env::temp_dir().join("ork3-history-markdown");
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("transcript.jsonl");
        let mut file = std::fs::File::create(&path).expect("create");
        // Built with serde so the Markdown body keeps its real newlines; a raw string literal
        // cannot carry the `\n` escapes this JSON needs.
        // An agent turn is summarised by its *closing* lines, so the Markdown under test sits at
        // the end: a heading on the opening line would be correctly elided as narration.
        let body = "先查一下配置\n再看一处\n## 现状总览\n你说的 **PAI** 在 `~/.claude/` 下";
        let record = serde_json::json!({
            "type": "response_item",
            "payload": {"role": "assistant", "content": [{"type": "text", "text": body}]},
        });
        writeln!(file, "{record}").expect("write");

        let mut state = AppState::test_new();
        let mut snapshot = snapshot();
        let session = &mut snapshot.projects[0].sessions[0];
        session.transcript_ref = Some(path.to_string_lossy().into_owned());
        let session_key = session.stable_key.clone();
        state.projects.snapshot = snapshot;
        state.projects.history_session_key = Some(session_key);

        let area = Rect::new(0, 0, 70, 20);
        let backend = ratatui::backend::TestBackend::new(area.width, area.height);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| render_project_history(&state, frame, area))
            .expect("draw");
        let buffer = terminal.backend().buffer();
        let text = (area.y..area.bottom())
            .map(|y| {
                let mut line = String::new();
                let mut skip = 0usize;
                for x in area.x..area.right() {
                    if skip > 0 {
                        skip -= 1;
                        continue;
                    }
                    let symbol = buffer[(x, y)].symbol();
                    skip = crate::ui::text::display_width(symbol).saturating_sub(1);
                    line.push_str(symbol);
                }
                line
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            text.contains("现状总览"),
            "heading text must survive:\n{text}"
        );
        assert!(
            !text.contains("##"),
            "heading hashes must not render:\n{text}"
        );
        assert!(
            !text.contains("**"),
            "bold markers must not render:\n{text}"
        );
        assert!(text.contains("PAI"), "bold text must survive:\n{text}");
        assert!(
            !text.contains("先查一下配置"),
            "an agent's opening narration must not be the summary:\n{text}"
        );
    }

    #[test]
    fn history_preview_explains_a_backend_without_a_transcript() {
        let mut state = AppState::test_new();
        let mut snapshot = snapshot();
        let session = &mut snapshot.projects[0].sessions[0];
        session.backend = "opencode".to_string();
        session.transcript_ref = None;
        let session_key = session.stable_key.clone();
        state.projects.snapshot = snapshot;
        state.projects.history_session_key = Some(session_key);

        let area = Rect::new(0, 0, 60, 20);
        let backend = ratatui::backend::TestBackend::new(area.width, area.height);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| render_project_history(&state, frame, area))
            .expect("draw");
        let buffer = terminal.backend().buffer();
        let text = (area.y..area.bottom())
            .map(|y| {
                (area.x..area.right())
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            text.contains("OpenCode"),
            "the unsupported backend must be named:\n{text}"
        );
    }

    #[test]
    fn search_expands_without_mutating_persisted_fold() {
        let mut state = AppState::test_new();
        state.projects.snapshot = snapshot();
        state.collapsed_project_keys.insert("p1".into());
        state.projects.query = "dense".into();
        let rows = project_tree_rows(&state);
        assert_eq!(rows.len(), 2);
        assert!(state.collapsed_project_keys.contains("p1"));
    }

    #[test]
    fn empty_catalog_shows_indexed_copy_and_scan_status_without_sample_data() {
        let mut state = AppState::test_new();
        state.projects.snapshot = ProjectsSnapshot {
            projects_schema_version: 1,
            revision: 3,
            projects: Vec::new(),
            topics: Vec::new(),
            scan_status: vec![
                AdapterScanStatus {
                    adapter: "codex".into(),
                    state: "ready".into(),
                    diagnostic_category: None,
                },
                AdapterScanStatus {
                    adapter: "claude".into(),
                    state: "failed".into(),
                    diagnostic_category: Some("root_unavailable".into()),
                },
            ],
            diagnostic_category: None,
        };

        let rows = project_tree_rows(&state);

        assert_eq!(
            rows[0],
            ProjectTreeRow::Empty("No indexed projects yet".to_string())
        );
        assert_eq!(
            rows[1],
            ProjectTreeRow::ScanStatus("codex: ready".to_string())
        );
        assert_eq!(
            rows[2],
            ProjectTreeRow::ScanStatus("claude: failed · root_unavailable".to_string())
        );
        assert_eq!(rows.len(), 3);
        // No fabricated project or session rows stand in for the empty Catalog.
        assert!(!rows.iter().any(|row| matches!(
            row,
            ProjectTreeRow::Project { .. } | ProjectTreeRow::Session(_)
        )));
    }

    #[test]
    fn locking_last_unclassified_project_zeroes_chip_count_and_leaves_no_pseudo_node() {
        let mut state = AppState::test_new();
        let mut snapshot = snapshot();
        snapshot.projects[0].kind = ProjectKind::Unclassified;
        state.projects.snapshot = snapshot;

        assert_eq!(unclassified_pending_count(&state), 1);
        let chip = unclassified_chip(&state);
        assert_eq!(chip.label, "unclass 1");
        assert!(!chip.disabled);
        state.projects.filter = ProjectFilter::Unclassified;
        assert_eq!(project_tree_rows(&state).len(), 2);

        // Moving the last pending session to a real Project and locking it clears the queue.
        state.projects.snapshot.projects[0].kind = ProjectKind::Cwd;

        assert_eq!(unclassified_pending_count(&state), 0);
        let chip = unclassified_chip(&state);
        assert_eq!(chip.label, "unclass");
        assert!(
            chip.disabled,
            "chip must be disabled once nothing is pending"
        );
        let rows = project_tree_rows(&state);
        assert_eq!(
            rows[0],
            ProjectTreeRow::Empty("No sessions in this filter".to_string())
        );
        assert!(!rows.iter().any(|row| matches!(
            row,
            ProjectTreeRow::Project { .. } | ProjectTreeRow::Session(_)
        )));
    }

    #[test]
    fn directory_and_topic_modes_render_independent_parent_groups() {
        let mut state = AppState::test_new();
        let mut snapshot = snapshot();
        let mut topic = snapshot.projects[0].clone();
        topic.canonical_key = "topic-1".into();
        topic.kind = ProjectKind::Semantic;
        topic.display_name = "ORK3 architecture".into();
        topic.canonical_path = "semantic-topic-key".into();
        snapshot.topics.push(topic);
        state.projects.snapshot = snapshot;

        let directory_rows = project_tree_rows(&state);
        assert!(matches!(
            &directory_rows[0],
            ProjectTreeRow::Project { display_name, kind: ProjectKind::Cwd, .. }
                if display_name == "ait"
        ));

        state.sidebar_view = crate::app::state::SidebarView::Clusters;
        let topic_rows = project_tree_rows(&state);
        assert!(matches!(
            &topic_rows[0],
            ProjectTreeRow::Project { display_name, kind: ProjectKind::Semantic, .. }
                if display_name == "ORK3 architecture"
        ));
        assert!(!topic_rows.iter().any(|row| matches!(
            row,
            ProjectTreeRow::Project { display_name, .. } if display_name == "ait"
        )));
    }

    #[test]
    fn automation_templates_render_as_one_non_activating_directory_row() {
        let mut state = AppState::test_new();
        let mut snapshot = snapshot();
        snapshot.projects[0].automation = vec![AutomationTemplateSummary {
            representative_session_key: "automation-1".into(),
            title: "OpenClaw watchdog".into(),
            backend: "codex".into(),
            count: 8_955,
            last_activity_at: 3,
        }];
        state.projects.snapshot = snapshot;

        let rows = project_tree_rows(&state);
        assert!(matches!(
            rows.last(),
            Some(ProjectTreeRow::Automation(template)) if template.count == 8_955
        ));
        assert!(rows.last().and_then(ProjectTreeRow::action).is_none());
        assert!(matches!(
            &rows[0],
            ProjectTreeRow::Project {
                session_count: 8_956,
                ..
            }
        ));

        state.sidebar_view = crate::app::state::SidebarView::Clusters;
        assert!(!project_tree_rows(&state)
            .iter()
            .any(|row| matches!(row, ProjectTreeRow::Automation(_))));
    }
}
