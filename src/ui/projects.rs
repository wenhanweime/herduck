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
use crate::projects::{
    AutomationTemplateSummary, IndexedSessionSummary, ProjectKind, ProjectSummary,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ProjectTreeActivity {
    Inactive,
    Live,
    Current,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProjectTreeRow {
    Project {
        project_key: String,
        display_name: String,
        session_count: usize,
        collapsed: bool,
        kind: ProjectKind,
        activity: ProjectTreeActivity,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProjectTreeRowIdentity {
    Project(String),
    Session(String),
    Automation(String),
    Thin(String),
    LoadOlder(String),
}

impl ProjectTreeRow {
    /// Stable logical identity used to keep the user's selection on the same Catalog item when
    /// an incremental snapshot inserts, removes, or reorders rows around it.
    pub(crate) fn identity(&self) -> Option<ProjectTreeRowIdentity> {
        match self {
            Self::Project { project_key, .. } => {
                Some(ProjectTreeRowIdentity::Project(project_key.clone()))
            }
            Self::Session(session) => {
                Some(ProjectTreeRowIdentity::Session(session.stable_key.clone()))
            }
            Self::Automation(template) => Some(ProjectTreeRowIdentity::Automation(
                template.representative_session_key.clone(),
            )),
            Self::Thin { project_key, .. } => {
                Some(ProjectTreeRowIdentity::Thin(project_key.clone()))
            }
            Self::LoadOlder { project_key } => {
                Some(ProjectTreeRowIdentity::LoadOlder(project_key.clone()))
            }
            Self::Empty(_) | Self::Diagnostic(_) | Self::ScanStatus(_) => None,
        }
    }

    pub(crate) fn action(&self) -> Option<ProjectTreeAction> {
        match self {
            Self::Project {
                project_key,
                collapsed,
                ..
            } => Some(ProjectTreeAction::ToggleProject {
                project_key: project_key.clone(),
                collapsed: *collapsed,
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
    pub filter_tabs: [Rect; 2],
    pub search: Rect,
    pub tree: Rect,
    pub row_hits: Vec<ProjectRowHitArea>,
    pub normalized_scroll: usize,
}

/// Every Catalog session currently available to the sidebar, independent of which hierarchy
/// exposed it. Directory snapshots intentionally hide disposable agent cwd groups, while Topics
/// can still carry those sessions; taking the union keeps Sessions complete without presenting an
/// internal holding Project.
fn snapshot_sessions(app: &AppState) -> Vec<IndexedSessionSummary> {
    let mut seen = HashSet::new();
    app.projects
        .snapshot
        .projects
        .iter()
        .chain(app.projects.snapshot.topics.iter())
        .flat_map(|project| project.sessions.iter())
        .filter(|session| seen.insert(session.stable_key.clone()))
        .cloned()
        .collect()
}

fn session_matches_query(session: &IndexedSessionSummary, query: &str) -> bool {
    query.is_empty()
        || crate::app::state::text_matches_query(
            query,
            &format!(
                "{} {} {} {}",
                session.title,
                session.backend,
                session.cwd.as_deref().unwrap_or_default(),
                session.topic_label.as_deref().unwrap_or_default()
            ),
        )
}

fn session_tree_activity(app: &AppState, session: &IndexedSessionSummary) -> ProjectTreeActivity {
    if is_current_session(app, session) {
        ProjectTreeActivity::Current
    } else if session.live {
        ProjectTreeActivity::Live
    } else {
        ProjectTreeActivity::Inactive
    }
}

fn project_tree_activity(app: &AppState, project: &ProjectSummary) -> ProjectTreeActivity {
    if matches!(
        app.mode,
        crate::app::Mode::TopicDetail | crate::app::Mode::EditTopicCover
    ) {
        return if app.projects.topic_detail_key.as_deref() == Some(&project.canonical_key) {
            ProjectTreeActivity::Current
        } else if project.sessions.iter().any(|session| session.live) {
            ProjectTreeActivity::Live
        } else {
            ProjectTreeActivity::Inactive
        };
    }
    project
        .sessions
        .iter()
        .map(|session| session_tree_activity(app, session))
        .max()
        .unwrap_or(ProjectTreeActivity::Inactive)
}

pub(crate) fn project_tree_rows(app: &AppState) -> Vec<ProjectTreeRow> {
    if let Some(category) = app.projects.snapshot.diagnostic_category.as_ref() {
        return vec![ProjectTreeRow::Diagnostic(format!(
            "{} · {category}",
            app.title_language
                .text("Catalog unavailable", "会话目录暂不可用")
        ))];
    }

    let query = app.projects.query.trim();
    let mut rows = Vec::new();
    let all_sessions = snapshot_sessions(app);
    if app.sidebar_view == crate::app::state::SidebarView::Sessions {
        let mut sessions = all_sessions
            .into_iter()
            .filter(|session| app.projects.filter != ProjectFilter::Open || session.live)
            .filter(|session| session_matches_query(session, query))
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
                app.title_language
                    .text("No sessions yet", "还没有会话")
                    .to_string()
            } else {
                app.title_language
                    .text("No matching sessions", "没有匹配的会话")
                    .to_string()
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

    enum TopLevelItem<'a> {
        Session(&'a IndexedSessionSummary),
        Project(&'a crate::projects::ProjectSummary),
    }

    let mut top_level = groups
        .iter()
        .filter(|project| {
            grouping == crate::app::state::ProjectGrouping::Topics
                || project.kind != ProjectKind::Unclassified
        })
        .map(TopLevelItem::Project)
        .collect::<Vec<_>>();

    // Internal Unclassified/ephemeral groups are storage details, not user navigation. Sessions
    // without a visible directory parent are still useful, so show them directly at the top level.
    if grouping == crate::app::state::ProjectGrouping::Directories {
        let represented = groups
            .iter()
            .filter(|project| project.kind != ProjectKind::Unclassified)
            .flat_map(|project| project.sessions.iter())
            .map(|session| session.stable_key.as_str())
            .collect::<HashSet<_>>();
        let parentless = all_sessions
            .iter()
            .filter(|session| !represented.contains(session.stable_key.as_str()))
            .filter(|session| app.projects.filter != ProjectFilter::Open || session.live)
            .filter(|session| session_matches_query(session, query))
            .collect::<Vec<_>>();
        top_level.extend(parentless.into_iter().map(TopLevelItem::Session));
    }

    top_level.sort_by(|left, right| {
        let latest = |item: &TopLevelItem<'_>| match item {
            TopLevelItem::Session(session) => session.last_activity_at,
            TopLevelItem::Project(project) => project
                .sessions
                .iter()
                .map(|session| session.last_activity_at)
                .max()
                .unwrap_or(i64::MIN),
        };
        latest(right)
            .cmp(&latest(left))
            .then_with(|| match (left, right) {
                (TopLevelItem::Session(left), TopLevelItem::Session(right)) => {
                    left.stable_key.cmp(&right.stable_key)
                }
                (TopLevelItem::Project(left), TopLevelItem::Project(right)) => {
                    left.canonical_key.cmp(&right.canonical_key)
                }
                (TopLevelItem::Session(left), TopLevelItem::Project(right)) => {
                    left.stable_key.cmp(&right.canonical_key)
                }
                (TopLevelItem::Project(left), TopLevelItem::Session(right)) => {
                    left.canonical_key.cmp(&right.stable_key)
                }
            })
    });

    for item in top_level {
        let project = match item {
            TopLevelItem::Session(session) => {
                rows.push(ProjectTreeRow::Session((*session).clone()));
                continue;
            }
            TopLevelItem::Project(project) => project,
        };
        let project_matches = query.is_empty()
            || crate::app::state::text_matches_query(
                query,
                &format!("{} {}", project.display_name, project.canonical_path),
            );
        let sessions = project
            .sessions
            .iter()
            .filter(|session| app.projects.filter != ProjectFilter::Open || session.live)
            .filter(|session| project_matches || session_matches_query(session, query))
            .cloned()
            .collect::<Vec<_>>();
        let automation = project
            .automation
            .iter()
            .filter(|_| app.projects.filter == ProjectFilter::All)
            .filter(|template| {
                project_matches
                    || crate::app::state::text_matches_query(
                        query,
                        &format!("{} {}", template.title, template.backend),
                    )
            })
            .cloned()
            .collect::<Vec<_>>();
        let authored_topic = project.kind == ProjectKind::Semantic
            && project.cover.is_some()
            && app.projects.filter == ProjectFilter::All
            && project_matches;
        if sessions.is_empty() && automation.is_empty() && !authored_topic {
            continue;
        }

        let activity = project_tree_activity(app, project);
        // Running/current groups are part of the user's working set, so expose their sessions by
        // default. An explicit fold remains authoritative and search still expands temporarily.
        let default_collapsed = grouping == crate::app::state::ProjectGrouping::Topics
            && activity == ProjectTreeActivity::Inactive;
        let collapsed = query.is_empty()
            && (app.collapsed_project_keys.contains(&project.canonical_key)
                || (!app.expanded_project_keys.contains(&project.canonical_key)
                    && default_collapsed));
        rows.push(ProjectTreeRow::Project {
            project_key: project.canonical_key.clone(),
            display_name: project.display_name.clone(),
            session_count: automation.iter().fold(sessions.len(), |count, template| {
                count.saturating_add(usize::try_from(template.count).unwrap_or(usize::MAX))
            }),
            collapsed,
            kind: project.kind,
            activity,
        });
        if !collapsed {
            let collapse_thin = grouping == crate::app::state::ProjectGrouping::Directories
                && query.is_empty()
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
        let catalog_is_empty = match grouping {
            crate::app::state::ProjectGrouping::Directories => {
                app.projects.snapshot.projects.is_empty() && all_sessions.is_empty()
            }
            crate::app::state::ProjectGrouping::Topics => groups.is_empty(),
        };
        let label = if !app.projects.query.trim().is_empty() {
            app.title_language
                .text("No matching sessions", "没有匹配的会话")
        } else if catalog_is_empty {
            match grouping {
                crate::app::state::ProjectGrouping::Directories => app
                    .title_language
                    .text("No indexed projects yet", "还没有索引到项目"),
                crate::app::state::ProjectGrouping::Topics => {
                    app.title_language.text("No work yet", "还没有工作")
                }
            }
        } else {
            app.title_language
                .text("No sessions in this filter", "当前筛选条件下没有会话")
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
            ProjectTreeRow::ScanStatus(format!(
                "{}: {}{detail}",
                status.adapter,
                app.title_language.text(
                    &status.state,
                    match status.state.as_str() {
                        "ready" => "已就绪",
                        "failed" => "读取失败",
                        "scanning" => "正在扫描",
                        "disabled" => "已关闭",
                        "pending" => "等待扫描",
                        _ => &status.state,
                    }
                )
            ))
        })
        .collect()
}

/// Lays out the visible tree rows, returning each row with the rect it occupies.
///
/// Live sessions and the current history preview are two lines tall. Hit testing and
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
/// Live sessions keep their status line when focus moves to another pane or history preview.
fn row_height(app: &AppState, row: &ProjectTreeRow) -> u16 {
    match row {
        ProjectTreeRow::Session(session) if session.live || is_current_session(app, session) => 2,
        _ => 1,
    }
}

/// Earliest row that keeps the target's full height visible at the bottom of the viewport.
fn scroll_to_show_row(
    app: &AppState,
    rows: &[ProjectTreeRow],
    target: usize,
    viewport: u16,
) -> usize {
    if rows.is_empty() {
        return 0;
    }
    let target = target.min(rows.len() - 1);
    let mut scroll = target;
    let mut height = 0;
    for (index, row) in rows[..=target].iter().enumerate().rev() {
        height += usize::from(row_height(app, row));
        if height > usize::from(viewport.max(1)) && index < target {
            break;
        }
        scroll = index;
    }
    scroll
}

pub(crate) fn project_tree_max_scroll(app: &AppState, viewport: u16) -> usize {
    let rows = project_tree_rows(app);
    scroll_to_show_row(app, &rows, rows.len().saturating_sub(1), viewport)
}

pub(crate) fn project_tree_scroll_for_selection(app: &AppState, viewport: u16) -> usize {
    let rows = project_tree_rows(app);
    let last = rows.len().saturating_sub(1);
    let selected = app.projects.selected_row.min(last);
    let minimum = scroll_to_show_row(app, &rows, selected, viewport);
    let maximum = scroll_to_show_row(app, &rows, last, viewport);
    app.projects.scroll.clamp(minimum, selected).min(maximum)
}

/// Keep all four navigation names readable as the sidebar narrows.
pub(super) fn sidebar_tab_height(width: u16, available_height: u16) -> u16 {
    available_height.min(if width >= 31 {
        1
    } else if width >= 17 {
        2
    } else {
        4
    })
}

pub(crate) fn project_sidebar_geometry(app: &AppState, area: Rect) -> ProjectSidebarGeometry {
    let content = Rect::new(area.x, area.y, area.width.saturating_sub(1), area.height);
    if content.width == 0 || content.height == 0 {
        return ProjectSidebarGeometry {
            sidebar_tabs: [Rect::default(); 4],
            filter_tabs: [Rect::default(); 2],
            search: Rect::default(),
            tree: Rect::default(),
            row_hits: Vec::new(),
            normalized_scroll: 0,
        };
    }

    let tab_height = sidebar_tab_height(content.width, content.height);
    let sidebar_tabs = if content.width < 31 && tab_height > 1 {
        let columns = if content.width >= 17 { 2 } else { 1 };
        let first_width = if columns == 2 {
            (content.width - 1) / 2
        } else {
            content.width
        };
        std::array::from_fn(|index| {
            let column = index as u16 % columns;
            let row = index as u16 / columns;
            let x = content.x + column * (first_width + 1);
            Rect::new(
                x,
                content.y + row,
                if column == 0 {
                    first_width
                } else {
                    content.right() - x
                },
                u16::from(row < tab_height),
            )
        })
    } else {
        let gap = u16::from(content.width >= 31);
        let available = content.width.saturating_sub(gap * 3);
        let mut x = content.x;
        std::array::from_fn(|index| {
            let minimum = [6, 8, 8, 6][index];
            let width = if available >= 28 {
                minimum + (available - 28) / 4 + u16::from((index as u16) < (available - 28) % 4)
            } else if index == 3 {
                content.right().saturating_sub(x)
            } else {
                available * minimum / 28
            };
            let rect = Rect::new(x, content.y, width, 1);
            x += width + gap;
            rect
        })
    };
    let controls_y = content.y.saturating_add(tab_height);
    let filter_height = content.height.saturating_sub(tab_height).min(1);
    // Filter chips paint a background when selected, so two adjacent chips would read as one block.
    // Keep their own gap while the sidebar can still fit both labels; top tabs use a separate rule
    // because four full labels need considerably more width.
    let filter_gap = u16::from(content.width >= 12);
    let filter_inner = content.width.saturating_sub(filter_gap);
    let first_width = filter_inner.min(5);
    let second_width = filter_inner.saturating_sub(first_width).min(6);
    let filter_tabs = [
        Rect::new(content.x, controls_y, first_width, filter_height),
        Rect::new(
            content.x + first_width + filter_gap,
            controls_y,
            second_width,
            filter_height,
        ),
    ];
    let search_offset = tab_height.saturating_add(filter_height);
    let search_height = content.height.saturating_sub(search_offset).min(1);
    let search = Rect::new(
        content.x,
        content.y.saturating_add(search_offset),
        content.width,
        search_height,
    );
    let tree_offset = search_offset.saturating_add(search_height);
    let tree_y = content.y.saturating_add(tree_offset);
    let tree = Rect::new(
        content.x,
        tree_y,
        content.width,
        super::brand::brand_footer_rect(content)
            .y
            .saturating_sub(tree_y),
    );

    let rows = project_tree_rows(app);
    let max_scroll = scroll_to_show_row(app, &rows, rows.len().saturating_sub(1), tree.height);
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
    if let Some(strip) = tabs
        .iter()
        .copied()
        .filter(|rect| rect.width > 0 && rect.height > 0)
        .reduce(|left, right| left.union(right))
    {
        frame.render_widget(Clear, strip);
        frame.render_widget(
            Paragraph::new("").style(Style::default().bg(app.palette.panel_bg)),
            strip,
        );
    }

    let labels = ["Agents", "Sessions", "Projects", "Work"];
    for (index, (label, rect)) in labels.into_iter().zip(tabs).enumerate() {
        if rect.width == 0 || rect.height == 0 {
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
                .fg(super::widgets::panel_contrast_fg(&app.palette))
                .bg(app.palette.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(app.palette.text)
                .bg(app.palette.surface0)
                .add_modifier(Modifier::BOLD)
        };
        frame.render_widget(Paragraph::new("").style(style), rect);
        let label_rect = Rect::new(
            rect.x,
            rect.y + rect.height.saturating_sub(1) / 2,
            rect.width,
            1,
        );
        frame.render_widget(Paragraph::new(label).style(style).centered(), label_rect);
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
    if matches!(
        app.mode,
        crate::app::Mode::TopicDetail | crate::app::Mode::EditTopicCover
    ) {
        return false;
    }
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
) -> Option<(crate::detect::AgentState, bool, bool)> {
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
        .map(|terminal| (terminal.state, pane.seen, terminal.agent_inactive))
}

pub(crate) fn render_projects_sidebar(app: &AppState, frame: &mut Frame, area: Rect) {
    let geometry = project_sidebar_geometry(app, area);
    if area.width > 0 {
        let separator_x = area.x + area.width.saturating_sub(1);
        let divider_active = app.mode == crate::app::Mode::Navigate;
        let separator_style = if divider_active {
            Style::default()
                .fg(app.palette.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(app.palette.overlay0)
                .add_modifier(Modifier::DIM)
        };
        for y in area.y..area.y + area.height {
            frame.buffer_mut()[(separator_x, y)]
                .set_symbol(if divider_active { "┃" } else { "│" })
                .set_style(separator_style);
        }
        if divider_active && area.height >= 5 {
            frame.buffer_mut()[(separator_x, area.y + area.height / 2)]
                .set_symbol("↔")
                .set_style(separator_style);
        }
    }
    render_sidebar_tabs(app, frame, geometry.sidebar_tabs);

    let filters = [
        (ProjectFilter::All, app.title_language.text("all", "全部")),
        (ProjectFilter::Open, app.title_language.text("open", "打开")),
    ];
    for ((filter, label), rect) in filters.into_iter().zip(geometry.filter_tabs) {
        if rect.width == 0 {
            continue;
        }
        let style = if app.projects.filter == filter {
            Style::default()
                .fg(app.palette.text)
                .bg(app.palette.surface1)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.palette.overlay0)
        };
        frame.render_widget(Paragraph::new(label).style(style).centered(), rect);
    }

    if geometry.search.height > 0 {
        let focused = app.projects.search_focused;
        let (prefix, query) = if focused {
            (
                app.title_language.text("▌ SEARCH ", "▌ 搜索 "),
                if app.projects.query.is_empty() {
                    app.title_language.text("type to filter", "输入内容筛选")
                } else {
                    app.projects.query.as_str()
                },
            )
        } else if app.projects.query.is_empty() {
            let placeholder = match app.sidebar_view {
                crate::app::state::SidebarView::Sessions => app
                    .title_language
                    .text("search sessions, agents, paths", "搜索会话、Agent 或路径"),
                crate::app::state::SidebarView::Projects => app.title_language.text(
                    "search projects, sessions, agents",
                    "搜索项目、会话或 Agent",
                ),
                crate::app::state::SidebarView::Clusters => app
                    .title_language
                    .text("search work, sessions, agents", "搜索工作、会话或 Agent"),
                crate::app::state::SidebarView::SpacesAgents => {
                    app.title_language.text("search sessions", "搜索会话")
                }
            };
            (" / ", placeholder)
        } else {
            (" / ", app.projects.query.as_str())
        };
        let query_style = if app.projects.query.is_empty() {
            Style::default().fg(app.palette.overlay0)
        } else {
            Style::default().fg(app.palette.text)
        };
        let field_style = Style::default()
            .bg(if focused {
                app.palette.surface1
            } else {
                app.palette.surface0
            })
            .fg(app.palette.text);
        let prefix_style = Style::default()
            .fg(app.palette.accent)
            .add_modifier(if focused {
                Modifier::BOLD
            } else {
                Modifier::empty()
            });
        let mut spans = vec![
            Span::styled(prefix, prefix_style),
            Span::styled(query, query_style),
        ];
        if focused {
            spans.push(Span::styled("▏", Style::default().fg(app.palette.accent)));
        }
        frame.render_widget(
            Paragraph::new(Line::from(spans)).style(field_style),
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
        let highlighted_session =
            current || matches!(row, ProjectTreeRow::Session(session) if session.live);
        let project_activity = match row {
            ProjectTreeRow::Project { activity, .. } => *activity,
            _ => ProjectTreeActivity::Inactive,
        };
        // Live sessions keep the stronger fill independently of focus, cursor position, or mode.
        // The accent bar still distinguishes the current session from other live sessions.
        //
        // The background has to travel with the text: painting it first and then rendering an
        // unstyled `Paragraph` over the same rect resets every cell the glyphs occupy.
        let selected_project = matches!(row, ProjectTreeRow::Project { project_key, .. }
            if matches!(app.mode, crate::app::Mode::TopicDetail | crate::app::Mode::EditTopicCover)
                && app.projects.topic_detail_key.as_deref() == Some(project_key));
        let row_style = match (
            highlighted_session || selected_project,
            project_activity,
            cursor,
        ) {
            (true, _, _) => Style::default().bg(app.palette.surface1),
            (false, ProjectTreeActivity::Current | ProjectTreeActivity::Live, _)
            | (false, ProjectTreeActivity::Inactive, true) => {
                Style::default().bg(app.palette.surface0)
            }
            (false, ProjectTreeActivity::Inactive, false) => Style::default(),
        };
        // Fill the whole row, including a second line, before any text lands on it.
        if highlighted_session || cursor || project_activity != ProjectTreeActivity::Inactive {
            frame.render_widget(Paragraph::new("").style(row_style), row_rect);
        }
        let line = match row {
            ProjectTreeRow::Project {
                display_name,
                session_count,
                collapsed,
                kind,
                activity,
                ..
            } => {
                // A new Project starts a new sibling group; two projects may legitimately open
                // with the same words without one hiding the other.
                previous_session_text = None;
                let marker = if *collapsed { "▸" } else { "▾" };
                let activity_color = match activity {
                    ProjectTreeActivity::Current => app.palette.accent,
                    ProjectTreeActivity::Live => app.palette.green,
                    ProjectTreeActivity::Inactive => app.palette.overlay0,
                };
                let name_color = if *activity == ProjectTreeActivity::Current {
                    app.palette.text
                } else {
                    project_name_color(app, *kind)
                };
                let mut spans = vec![
                    Span::styled(format!(" {marker} "), Style::default().fg(activity_color)),
                    Span::styled(
                        display_name.clone(),
                        Style::default().fg(name_color).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" {session_count}"),
                        Style::default().fg(app.palette.overlay0),
                    ),
                ];
                if *activity != ProjectTreeActivity::Inactive {
                    spans.push(Span::styled(" ●", Style::default().fg(activity_color)));
                }
                Line::from(spans)
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
                    format!(
                        " · {} {}",
                        template.backend,
                        app.title_language.text("automation", "自动任务")
                    ),
                    Style::default().fg(app.palette.overlay0),
                ),
            ]),
            ProjectTreeRow::Thin { count, .. } => Line::from(Span::styled(
                format!(
                    "  +{count} {}",
                    app.title_language.text("short sessions", "简短会话")
                ),
                Style::default()
                    .fg(app.palette.overlay0)
                    .add_modifier(Modifier::DIM),
            )),
            ProjectTreeRow::LoadOlder { .. } => Line::from(Span::styled(
                app.title_language.text("  Load older…", "  加载更早记录…"),
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
        // A parent keeps its own marker when its detail or one of its children is open.
        // Use a different glyph from the session marker so hierarchy and cursor stay legible.
        if project_activity == ProjectTreeActivity::Current && rect.width > 0 {
            frame.buffer_mut()[(rect.x, rect.y)]
                .set_symbol("▌")
                .set_fg(app.palette.accent);
        }

        // The status line stays visible for every live session.
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

/// Second line of an expanded session row: what it is doing and where it lives.
///
/// A live session reports its status (`working` / `done` / `idle` / `inactive` / `blocked`) using the same
/// vocabulary as the Sessions sidebar. A historical session has no pane, so it says so rather than
/// borrowing a state it does not have.
pub(super) fn session_status_line<'a>(app: &AppState, session: &IndexedSessionSummary) -> Line<'a> {
    let mut spans = vec![Span::raw("    ")];
    match session_agent_state(app, session) {
        Some((_, _, true)) => spans.push(Span::styled(
            app.title_language.text("○ inactive", "○ 已暂停"),
            Style::default().fg(app.palette.overlay0),
        )),
        Some((state, seen, false)) => {
            let (glyph, glyph_style) = super::status::state_dot(state, seen, &app.palette);
            spans.push(Span::styled(format!("{glyph} "), glyph_style));
            spans.push(Span::styled(
                app.title_language.text(
                    super::status::state_label(state, seen),
                    match (state, seen) {
                        (crate::detect::AgentState::Working, _) => "正在进行",
                        (crate::detect::AgentState::Blocked, _) => "等待答复",
                        (crate::detect::AgentState::Idle, false) => "本轮已完成",
                        _ => "空闲",
                    },
                ),
                Style::default().fg(super::status::state_label_color(state, seen, &app.palette)),
            ));
        }
        None if session.live => spans.push(Span::styled(
            app.title_language.text("open", "已打开"),
            Style::default().fg(app.palette.green),
        )),
        None => spans.push(Span::styled(
            app.title_language
                .text("read-only history", "历史记录 · 只读"),
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
            .title(
                app.title_language
                    .text(" Historical session ", " 历史会话 "),
            )
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.palette.overlay0));
        frame.render_widget(
            Paragraph::new(app.title_language.text(
                "This session is no longer available in the current snapshot.",
                "此会话已不在当前记录中。",
            ))
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
                session
                    .cwd
                    .as_deref()
                    .unwrap_or(app.title_language.text("unknown cwd", "工作目录未知"))
                    .to_string(),
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
                // A session that is mostly plain prose has almost no Markdown to colour, so the
                // speaker labels carry the visual rhythm. A filled chip reads at a glance where a
                // coloured glyph inside monospace prose did not.
                let (marker, accent) = match message.role {
                    crate::projects::transcript::TranscriptRole::User => {
                        (app.title_language.text(" you ", " 你 "), app.palette.accent)
                    }
                    crate::projects::transcript::TranscriptRole::Assistant => (
                        app.title_language.text(" agent ", " Agent "),
                        app.palette.green,
                    ),
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
                    lines.extend(super::markdown::markdown_lines(&message.text, &app.palette));
                } else {
                    let (excerpt, elided) =
                        crate::projects::transcript::preview_excerpt(&message.text, message.role);
                    lines.extend(super::markdown::markdown_lines(&excerpt, &app.palette));
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
                    app.title_language
                        .text("… earlier turns not shown", "… 更早的对话未显示"),
                    Style::default().fg(app.palette.overlay0),
                )));
            }
        }
        Err(error) => {
            lines.push(Line::from(Span::styled(
                app.title_language
                    .text(
                        &error.message(),
                        "无法读取会话记录，请检查记录文件及读取权限。",
                    )
                    .to_string(),
                Style::default().fg(app.palette.peach),
            )));
            // A failed resume is why the user landed here, so keep explaining it rather than
            // replacing that context with the read failure alone.
            if let Some(reason) = app.projects.history_fallback_reason.as_deref() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    app.title_language
                        .text(
                            reason,
                            "暂时无法恢复原会话，请检查 Agent 与工作目录后重试。",
                        )
                        .to_string(),
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
            Span::styled(
                app.title_language.text(" back   ", " 返回   "),
                Style::default().fg(app.palette.overlay0),
            ),
            Span::styled(
                app.title_language
                    .text("Enter / click again", "Enter / 再次点击"),
                Style::default()
                    .fg(app.palette.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                app.title_language.text(
                    " open full context (agent stays stopped)",
                    " 打开完整上下文（Agent 保持停止）",
                ),
                Style::default().fg(app.palette.overlay0),
            ),
        ]));
    }

    if let Some(reason) = app
        .projects
        .history_fallback_reason
        .as_deref()
        .filter(|reason| !reason.starts_with("Read-only preview."))
    {
        lines.push(Line::from(Span::styled(
            app.title_language
                .text(
                    reason,
                    "暂时无法恢复原会话，请检查 Agent 与工作目录后重试。",
                )
                .to_string(),
            Style::default().fg(app.palette.peach),
        )));
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
                app.title_language.text("Type a message…", "输入消息…"),
                Style::default().fg(app.palette.overlay0),
            ))
        } else {
            Line::from(app.projects.history_draft.clone())
        };
        let composer_block = Block::default()
            .title(app.title_language.text(
                " Enter resumes · also sends a draft · Shift+Enter new line ",
                " Enter 恢复 · 有内容时同时发送 · Shift+Enter 换行 ",
            ))
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
                cover: None,
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

    fn topic_with_sessions(key: &str, display_name: &str, count: usize) -> ProjectSummary {
        let mut topic = snapshot().projects.remove(0);
        topic.canonical_key = key.to_string();
        topic.canonical_path = key.to_string();
        topic.kind = ProjectKind::Semantic;
        topic.display_name = display_name.to_string();
        let seed = topic.sessions.remove(0);
        topic.sessions = (0..count)
            .map(|index| {
                let mut session = seed.clone();
                session.stable_key = format!("{key}-session-{index}");
                session.ref_value = session.stable_key.clone();
                session.title = format!("{display_name} task {index}");
                session.last_activity_at = 10_000_i64.saturating_sub(index as i64);
                session.live = false;
                session.workspace_id = None;
                session.pane_id = None;
                session.runtime_generation = None;
                session.topic_label = Some(display_name.to_string());
                session
            })
            .collect();
        topic
    }

    #[test]
    fn focusing_a_history_topic_does_not_reorder_group_headers() {
        let mut state = AppState::test_new();
        state.sidebar_view = crate::app::state::SidebarView::Clusters;
        state.projects.snapshot = snapshot();
        state.projects.snapshot.topics = vec![
            topic_with_sessions("a", "A", 2),
            topic_with_sessions("b", "B", 2),
        ];
        let keys = |state: &AppState| {
            project_tree_rows(state)
                .into_iter()
                .filter_map(|row| {
                    if let ProjectTreeRow::Project { project_key, .. } = row {
                        Some(project_key)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        };
        let before = keys(&state);
        state.projects.history_session_key = Some("b-session-0".into());
        assert_eq!(keys(&state), before);
    }

    #[test]
    fn topics_do_not_hide_recent_sessions_using_a_total_thin_count() {
        let mut state = AppState::test_new();
        state.sidebar_view = crate::app::state::SidebarView::Clusters;
        let mut topic = topic_with_sessions("recent", "Recent", 2);
        topic.thin_count = 20;
        state
            .expanded_project_keys
            .insert(topic.canonical_key.clone());
        state.projects.snapshot.topics = vec![topic];

        let rows = project_tree_rows(&state);
        let sessions = rows
            .iter()
            .filter_map(|row| match row {
                ProjectTreeRow::Session(session) => Some(session),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(sessions.len(), 2, "both loaded sessions must stay visible");
        assert!(sessions
            .windows(2)
            .all(|pair| pair[0].last_activity_at >= pair[1].last_activity_at));
        assert!(!rows
            .iter()
            .any(|row| matches!(row, ProjectTreeRow::Thin { .. })));
    }

    #[test]
    fn sidebar_headers_and_menu_remain_visible_in_every_view() {
        use crate::app::state::SidebarView;
        let mut state = AppState::test_new();
        state.workspaces = (0..12)
            .map(|index| crate::workspace::Workspace::test_new(&format!("project-{index}")))
            .collect();
        for view in [
            SidebarView::SpacesAgents,
            SidebarView::Sessions,
            SidebarView::Projects,
            SidebarView::Clusters,
        ] {
            for (width, height) in [
                (7, 24),
                (18, 24),
                (25, 24),
                (26, 21),
                (26, 22),
                (26, 30),
                (34, 30),
            ] {
                for attention in [false, true] {
                    state.sidebar_view = view;
                    state.sidebar_collapsed = false;
                    state.sidebar_min_width = 7;
                    state.sidebar_width = width;
                    state.sidebar_spaces.rows =
                        vec![vec![crate::config::SpaceSidebarToken::Workspace]];
                    state.sidebar_spaces.row_gap = 0;
                    state.projects.snapshot = snapshot();
                    state.update_available = attention.then(|| "0.2.0".into());
                    let area = Rect::new(0, 0, 120, height);
                    crate::ui::compute_view(&mut state, area);
                    let mut terminal =
                        ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, height))
                            .expect("terminal");
                    terminal
                        .draw(|frame| crate::ui::render(&state, frame))
                        .expect("render");
                    let buffer = terminal.backend().buffer();
                    let menu = state.global_launcher_rect();
                    let text = (menu.y..menu.bottom())
                        .map(|y| {
                            (menu.x..menu.right())
                                .map(|x| buffer[(x, y)].symbol())
                                .collect::<String>()
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    assert!(text.contains("menu"), "{view:?} {width}×{height}: {text}");
                    if width >= 25 {
                        assert!(
                            text.contains("herduck"),
                            "{view:?} {width}×{height}: {text}"
                        );
                    }
                    if menu.height == 4 {
                        assert!(text.contains('⡠'), "{view:?}: missing duck outline");
                    }
                    if width == 26 && height == 30 {
                        assert_eq!(menu.height, 4, "default sidebar must display the duck");
                    }
                    if view == SidebarView::SpacesAgents && width == 26 && height == 21 {
                        assert_eq!(
                            menu.height, 1,
                            "short workspace section must keep its content"
                        );
                    }
                    if attention {
                        assert!(text.contains('●'), "{view:?}: missing attention badge");
                    }
                    if view == SidebarView::SpacesAgents {
                        let new_button = state.sidebar_new_button_rect();
                        assert_eq!(new_button.height, u16::from(width > 7));
                        assert!(!menu.intersects(new_button));
                        assert!(!state.view.workspace_card_areas.is_empty());
                        assert!(state
                            .view
                            .workspace_card_areas
                            .iter()
                            .all(|card| card.rect.bottom() <= menu.y));
                        let list = crate::ui::workspace_list_rect(
                            state.view.sidebar_rect,
                            state.sidebar_section_split,
                        );
                        let scrollbar = crate::ui::workspace_list_scrollbar_rect(&state, list)
                            .expect("populated list must scroll");
                        assert!(scrollbar.bottom() <= menu.y);
                        let y = state
                            .view
                            .project_sidebar_tabs
                            .iter()
                            .map(|rect| rect.bottom())
                            .max()
                            .unwrap();
                        let text = (0..state.view.sidebar_rect.width)
                            .map(|x| buffer[(x, y)].symbol())
                            .collect::<String>();
                        if width >= 18 {
                            assert!(text.contains("spaces"), "{text}");
                        }
                    } else {
                        assert!(state.view.project_tree_rect.bottom() <= menu.y);
                    }
                }
            }
        }
    }

    #[test]
    fn filters_are_one_row_and_share_geometry_with_hit_testing() {
        let mut state = AppState::test_new();
        state.projects.snapshot = snapshot();
        let geometry = project_sidebar_geometry(&state, Rect::new(0, 0, 40, 12));
        assert!(geometry.sidebar_tabs.iter().all(|rect| rect.height == 1));
        assert_eq!(geometry.sidebar_tabs[0].bottom(), geometry.filter_tabs[0].y);
        assert_eq!(geometry.sidebar_tabs[3].right(), 39);
        assert!(
            geometry.sidebar_tabs[1].x > geometry.sidebar_tabs[0].right(),
            "Sessions/Projects/Work tabs must not abut"
        );
        assert!(geometry.filter_tabs.iter().all(|rect| rect.height == 1));
        assert!(
            geometry.filter_tabs[1].x > geometry.filter_tabs[0].right(),
            "all/open chips paint backgrounds and must not abut"
        );
        assert!(
            geometry.filter_tabs[1].right() <= geometry.sidebar_tabs[3].right(),
            "filter chips must stay inside the sidebar content column"
        );
        assert_eq!(geometry.row_hits.len(), 2);
        assert_eq!(geometry.row_hits[0].rect.height, 1);
    }

    #[test]
    fn semantic_grouping_tab_is_named_work() {
        let mut state = AppState::test_new();
        state.projects.snapshot = snapshot();
        let text = rendered_text(&state, Rect::new(0, 0, 60, 8));
        assert!(text.contains("Work"));
        assert!(!text.contains("Clusters"));
    }

    #[test]
    fn top_level_tabs_keep_english_names_and_clear_both_rows_in_narrow_sidebars() {
        let mut state = AppState::test_new();
        state.sidebar_view = crate::app::state::SidebarView::Sessions;
        for palette in [
            crate::app::state::Palette::catppuccin(),
            crate::app::state::Palette::catppuccin_latte(),
            crate::app::state::Palette::terminal(),
        ] {
            state.palette = palette;
            for language in [
                crate::config::TitleLanguage::Chinese,
                crate::config::TitleLanguage::English,
            ] {
                state.title_language = language;
                for width in [18, 25, 30, 40] {
                    let area = Rect::new(0, 0, width, 12);
                    let geometry = project_sidebar_geometry(&state, area);
                    let tabs = geometry.sidebar_tabs;
                    let bottom = tabs.iter().map(|rect| rect.bottom()).max().unwrap();
                    assert_eq!(geometry.filter_tabs[0].y, bottom);
                    assert_eq!(bottom, if width < 32 { 2 } else { 1 });
                    let mut terminal =
                        ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, 12))
                            .unwrap();
                    terminal
                        .draw(|frame| {
                            for y in 0..bottom {
                                frame.render_widget(
                                    Paragraph::new("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"),
                                    Rect::new(0, y, width, 1),
                                );
                            }
                            render_sidebar_tabs(&state, frame, tabs);
                        })
                        .unwrap();
                    let buffer = terminal.backend().buffer();
                    for (index, (rect, label)) in tabs
                        .into_iter()
                        .zip(["Agents", "Sessions", "Projects", "Work"])
                        .enumerate()
                    {
                        let text = (rect.x..rect.right())
                            .map(|x| buffer[(x, rect.y)].symbol())
                            .collect::<String>();
                        assert_eq!(text.trim(), label, "{language:?} at width {width}");
                        let (fg, bg) = if index == 1 {
                            (
                                super::super::widgets::panel_contrast_fg(&state.palette),
                                state.palette.accent,
                            )
                        } else {
                            (state.palette.text, state.palette.surface0)
                        };
                        for x in rect.x..rect.right() {
                            let cell = &buffer[(x, rect.y)];
                            assert_eq!(cell.style().bg, Some(bg));
                            if !cell.symbol().trim().is_empty() {
                                assert_eq!(cell.style().fg, Some(fg));
                                assert!(cell.modifier.contains(Modifier::BOLD));
                            }
                        }
                        assert!(tabs
                            .iter()
                            .enumerate()
                            .all(|(other, area)| other == index
                                || area.intersection(rect).area() == 0));
                    }
                    for y in 0..bottom {
                        let line = (0..width - 1)
                            .map(|x| buffer[(x, y)].symbol())
                            .collect::<String>();
                        assert!(!line.contains('x'), "underlying content leaked: {line}");
                    }
                }
            }
        }
    }

    #[test]
    fn focused_search_field_uses_a_filled_surface_label_and_cursor_marker() {
        let mut state = AppState::test_new();
        state.sidebar_view = crate::app::state::SidebarView::Projects;
        state.mode = crate::app::Mode::Navigate;
        state.projects.search_focused = true;
        state.projects.snapshot = snapshot();
        let area = Rect::new(0, 0, 40, 12);
        let geometry = project_sidebar_geometry(&state, area);
        let cells = row_cells(&state, area);
        let search_row = &cells[usize::from(geometry.search.y - area.y)];
        let search_start = usize::from(geometry.search.x - area.x);

        assert_eq!(search_row[search_start].symbol, "▌");
        assert!(
            search_row[search_start..usize::from(geometry.search.right() - area.x)]
                .iter()
                .all(|cell| cell.bg == Some(state.palette.surface1))
        );
        let text = rendered_text(&state, area);
        assert!(text.contains("SEARCH"));
        assert!(text.contains("▏"));
    }

    #[test]
    fn navigate_mode_sidebar_divider_shows_a_resize_grip() {
        let mut state = AppState::test_new();
        state.sidebar_view = crate::app::state::SidebarView::Projects;
        state.mode = crate::app::Mode::Navigate;
        state.projects.snapshot = snapshot();
        let area = Rect::new(0, 0, 40, 12);
        let cells = row_cells(&state, area);

        assert_eq!(
            cells[usize::from(area.height / 2)][usize::from(area.width - 1)].symbol,
            "↔"
        );
    }

    #[test]
    fn topics_default_to_header_only_so_large_groups_do_not_hide_newer_topics() {
        let mut state = AppState::test_new();
        state.sidebar_view = crate::app::state::SidebarView::Clusters;
        let mut snapshot = snapshot();
        snapshot.topics = vec![
            topic_with_sessions("topic-latest", "最新主题", 50),
            topic_with_sessions("topic-second", "第二主题", 50),
            topic_with_sessions("topic-third", "第三主题", 50),
        ];
        state.projects.snapshot = snapshot;

        let rows = project_tree_rows(&state);

        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|row| matches!(
            row,
            ProjectTreeRow::Project {
                collapsed: true,
                ..
            }
        )));
        assert!(matches!(
            &rows[0],
            ProjectTreeRow::Project { display_name, .. } if display_name == "最新主题"
        ));
        assert!(matches!(
            &rows[1],
            ProjectTreeRow::Project { display_name, .. } if display_name == "第二主题"
        ));
    }

    #[test]
    fn current_and_live_topics_expand_without_overriding_recency() {
        let mut state = AppState::test_new();
        state.sidebar_view = crate::app::state::SidebarView::Clusters;
        let mut snapshot = snapshot();
        let mut newest_history = topic_with_sessions("topic-latest", "最新主题", 2);
        newest_history.sessions[0].last_activity_at = 30_000;
        let current = topic_with_sessions("topic-current", "当前主题", 2);
        let mut live = topic_with_sessions("topic-live", "运行主题", 2);
        live.sessions[0].live = true;
        snapshot.topics = vec![newest_history, current, live];
        state.projects.history_session_key = Some("topic-current-session-0".into());
        state.projects.snapshot = snapshot;

        let rows = project_tree_rows(&state);

        assert!(matches!(
            &rows[..],
            [
                ProjectTreeRow::Project {
                    project_key: history,
                    collapsed: true,
                    activity: ProjectTreeActivity::Inactive,
                    ..
                },
                ProjectTreeRow::Project {
                    project_key: current,
                    collapsed: false,
                    activity: ProjectTreeActivity::Current,
                    ..
                },
                ProjectTreeRow::Session(_),
                ProjectTreeRow::Session(_),
                ProjectTreeRow::Project {
                    project_key: live,
                    collapsed: false,
                    activity: ProjectTreeActivity::Live,
                    ..
                },
                ProjectTreeRow::Session(_),
                ProjectTreeRow::Session(_),
            ] if current == "topic-current" && live == "topic-live" && history == "topic-latest"
        ));
    }

    #[test]
    fn explicit_collapse_keeps_a_live_topic_folded_without_pinning() {
        let mut state = AppState::test_new();
        state.sidebar_view = crate::app::state::SidebarView::Clusters;
        let mut snapshot = snapshot();
        let mut live = topic_with_sessions("topic-live", "运行主题", 2);
        live.sessions[0].live = true;
        snapshot.topics = vec![topic_with_sessions("topic-history", "历史主题", 2), live];
        state.projects.snapshot = snapshot;
        state.collapsed_project_keys.insert("topic-live".into());

        let rows = project_tree_rows(&state);

        assert!(matches!(
            &rows[..],
            [
                ProjectTreeRow::Project {
                    project_key: history,
                    collapsed: true,
                    activity: ProjectTreeActivity::Inactive,
                    ..
                },
                ProjectTreeRow::Project {
                    project_key: live,
                    collapsed: true,
                    activity: ProjectTreeActivity::Live,
                    ..
                }
            ] if live == "topic-live" && history == "topic-history"
        ));
    }

    #[test]
    fn explicitly_expanded_topic_overrides_the_header_only_default() {
        let mut state = AppState::test_new();
        state.sidebar_view = crate::app::state::SidebarView::Clusters;
        let mut snapshot = snapshot();
        snapshot.topics = vec![topic_with_sessions("topic-latest", "最新主题", 2)];
        state.projects.snapshot = snapshot;
        state.expanded_project_keys.insert("topic-latest".into());

        let rows = project_tree_rows(&state);

        assert!(matches!(
            &rows[..],
            [
                ProjectTreeRow::Project {
                    collapsed: false,
                    ..
                },
                ProjectTreeRow::Session(_),
                ProjectTreeRow::Session(_)
            ]
        ));
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
    fn sessions_tab_applies_open_and_search_filters() {
        let mut state = AppState::test_new();
        state.sidebar_view = crate::app::state::SidebarView::Sessions;
        let mut snapshot = snapshot();
        let mut historical = newer_session();
        historical.topic_label = Some("topic".into());
        historical.title = "Historical topic".into();
        snapshot.projects[0].sessions.push(historical);
        state.projects.snapshot = snapshot;

        state.projects.filter = ProjectFilter::Open;
        assert!(project_tree_rows(&state)
            .iter()
            .all(|row| { matches!(row, ProjectTreeRow::Session(session) if session.live) }));

        state.projects.filter = ProjectFilter::All;
        state.projects.query = "historical".into();
        let rows = project_tree_rows(&state);
        assert!(
            matches!(&rows[..], [ProjectTreeRow::Session(session)] if session.title == "Historical topic")
        );
    }

    #[test]
    fn automation_search_is_case_insensitive_and_matches_multiple_terms() {
        let mut state = AppState::test_new();
        state.sidebar_view = crate::app::state::SidebarView::Projects;
        let mut snapshot = snapshot();
        snapshot.projects[0].sessions.clear();
        snapshot.projects[0].automation = vec![AutomationTemplateSummary {
            representative_session_key: "automation-search".into(),
            backend: "Codex".into(),
            title: "Daily Backup".into(),
            count: 1,
            last_activity_at: 3,
        }];
        state.projects.snapshot = snapshot;
        state.projects.query = "BACKUP codex".into();

        let rows = project_tree_rows(&state);

        assert!(rows
            .iter()
            .any(|row| matches!(row, ProjectTreeRow::Automation(template) if template.representative_session_key == "automation-search")));
    }

    #[test]
    fn sessions_and_projects_include_topic_only_sessions_without_an_internal_parent() {
        let mut state = AppState::test_new();
        let mut snapshot = snapshot();
        let mut topic = topic_with_sessions("topic-only", "Recovered topic", 1);
        topic.sessions[0].stable_key = "hidden-runtime".into();
        topic.sessions[0].title = "Fresh runtime session".into();
        snapshot.projects.clear();
        snapshot.topics = vec![topic];
        state.projects.snapshot = snapshot;

        state.sidebar_view = crate::app::state::SidebarView::Sessions;
        assert!(matches!(
            &project_tree_rows(&state)[..],
            [ProjectTreeRow::Session(session)] if session.stable_key == "hidden-runtime"
        ));

        state.sidebar_view = crate::app::state::SidebarView::Projects;
        assert!(matches!(
            &project_tree_rows(&state)[..],
            [ProjectTreeRow::Session(session)] if session.stable_key == "hidden-runtime"
        ));
    }

    #[test]
    fn library_orders_history_and_parentless_sessions_by_recency_not_liveness() {
        let mut state = AppState::test_new();
        state.sidebar_view = crate::app::state::SidebarView::Projects;
        let mut snapshot = snapshot();
        snapshot.projects[0].sessions[0].last_activity_at = 10;

        let mut parentless = snapshot.projects[0].clone();
        parentless.canonical_key = "holding".into();
        parentless.kind = ProjectKind::Unclassified;
        parentless.sessions[0].stable_key = "parentless".into();
        parentless.sessions[0].last_activity_at = 20;
        parentless.sessions[0].live = false;

        let mut recent = snapshot.projects[0].clone();
        recent.canonical_key = "recent".into();
        recent.display_name = "recent project".into();
        recent.sessions[0].stable_key = "recent-session".into();
        recent.sessions[0].last_activity_at = 30;
        recent.sessions[0].live = false;
        snapshot.projects.push(parentless);
        snapshot.projects.push(recent);
        state.projects.snapshot = snapshot;

        let rows = project_tree_rows(&state);
        assert!(matches!(
            &rows[3],
            ProjectTreeRow::Project {
                project_key,
                activity: ProjectTreeActivity::Live,
                ..
            } if project_key == "p1"
        ));
        assert!(matches!(
            &rows[0],
            ProjectTreeRow::Project { project_key, .. } if project_key == "recent"
        ));
        assert!(matches!(
            &rows[2],
            ProjectTreeRow::Session(session) if session.stable_key == "parentless"
        ));
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

    #[test]
    fn parent_of_current_session_uses_current_highlight_and_activity_marker() {
        let mut state = state_with_open_session();
        state.sidebar_view = crate::app::state::SidebarView::Projects;
        // Keep the keyboard cursor on the child so the parent highlight must come from activity.
        state.projects.selected_row = 1;
        let area = Rect::new(0, 0, 40, 12);
        let tree_top = usize::from(project_sidebar_geometry(&state, area).tree.y);
        let rows = row_cells(&state, area);
        let parent = &rows[tree_top];

        assert_eq!(
            text_cell_background(parent),
            Some(state.palette.surface0),
            "the current session's parent must use a contextual fill below the session itself"
        );
        assert!(parent
            .iter()
            .any(|cell| { cell.symbol == "●" && cell.fg == Some(state.palette.accent) }));
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

    #[test]
    fn live_rows_keep_two_filled_lines_across_tabs_and_focus_changes() {
        let mut state = state_with_open_session();
        state
            .workspaces
            .push(crate::workspace::Workspace::test_new("other-workspace"));
        state.ensure_test_terminals();
        let mut topic = state.projects.snapshot.projects[0].clone();
        topic.kind = ProjectKind::Semantic;
        state.projects.snapshot.topics = vec![topic];
        let area = Rect::new(0, 0, 40, 12);

        for palette in [
            crate::app::state::Palette::catppuccin(),
            crate::app::state::Palette::catppuccin_latte(),
            crate::app::state::Palette::terminal(),
        ] {
            state.palette = palette;
            for view in [
                crate::app::state::SidebarView::Sessions,
                crate::app::state::SidebarView::Projects,
                crate::app::state::SidebarView::Clusters,
            ] {
                state.sidebar_view = view;
                for active in [0, 1] {
                    state.active = Some(active);
                    for mode in [crate::app::Mode::Navigate, crate::app::Mode::Terminal] {
                        state.mode = mode;
                        let geometry = project_sidebar_geometry(&state, area);
                        let hit = geometry
                            .row_hits
                            .iter()
                            .find(|hit| {
                                matches!(
                                    hit.action,
                                    ProjectTreeAction::Activate(
                                        ProjectSessionActivation::Live { .. }
                                    )
                                )
                            })
                            .expect("live session must remain visible");
                        assert_eq!(hit.rect.height, 2, "{view:?}, {mode:?}, focus {active}");

                        let cells = row_cells(&state, area);
                        for y in hit.rect.y..hit.rect.bottom() {
                            assert!(cells[usize::from(y)]
                                [usize::from(hit.rect.x)..usize::from(hit.rect.right())]
                                .iter()
                                .all(|cell| cell.bg == Some(state.palette.surface1)));
                        }
                        let title = &cells[usize::from(hit.rect.y)];
                        assert!(title.iter().any(|cell| {
                            cell.symbol == "●" && cell.fg == Some(state.palette.green)
                        }));
                        assert_eq!(title[0].symbol == "▎", active == 0);
                    }
                }
            }
        }
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

    /// Unopened history stays compact while live sessions retain their status lines.
    #[test]
    fn ordinary_history_rows_stay_one_line() {
        let mut state = AppState::test_new();
        state.projects.snapshot = snapshot();
        let session = &mut state.projects.snapshot.projects[0].sessions[0];
        session.live = false;
        session.workspace_id = None;
        session.pane_id = None;
        session.runtime_generation = None;
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

    #[test]
    fn opened_project_and_topic_keep_their_parent_highlight_in_detail_and_editor() {
        for topics in [false, true] {
            let mut state = AppState::test_new();
            let mut selected = snapshot().projects.remove(0);
            selected.canonical_key = "selected-parent".into();
            selected.display_name = "Selected parent".into();
            selected.sessions[0].live = false;
            if topics {
                selected.kind = ProjectKind::Semantic;
            }
            let mut other = selected.clone();
            other.canonical_key = "another-parent".into();
            other.display_name = "Another parent".into();
            if topics {
                state.projects.snapshot.topics = vec![other, selected];
                state.sidebar_view = crate::app::state::SidebarView::Clusters;
            } else {
                state.projects.snapshot.projects = vec![other, selected];
                state.sidebar_view = crate::app::state::SidebarView::Projects;
            }
            assert!(state.open_topic_detail("selected-parent"));
            // The key, not an old cursor row, owns the current content selection.
            state.projects.selected_row = 0;
            for mode in [
                crate::app::Mode::TopicDetail,
                crate::app::Mode::EditTopicCover,
            ] {
                state.mode = mode;
                let rows = row_cells(&state, Rect::new(0, 0, 40, 16));
                let marked = rows
                    .iter()
                    .find(|row| row[0].symbol == "▌")
                    .expect("parent remains marked");
                let text: String = marked.iter().map(|cell| cell.symbol.as_str()).collect();
                assert!(text.contains("Selected parent"), "{text}");
                assert_eq!(text_cell_background(marked), Some(state.palette.surface1));
            }
        }
    }

    #[test]
    fn open_child_marks_its_parent_without_losing_the_child_marker() {
        let mut state = state_with_open_session();
        state.mode = crate::app::Mode::Terminal;
        let rows = row_cells(&state, Rect::new(0, 0, 40, 16));
        assert!(rows.iter().any(|row| row[0].symbol == "▌"), "parent marker");
        assert!(rows.iter().any(|row| row[0].symbol == "▎"), "child marker");
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
    fn directory_rows_show_subject_label_but_topic_rows_hide_it() {
        let mut state = AppState::test_new();
        let mut snapshot = snapshot();
        snapshot.projects[0].sessions[0].title = "【简历】整理腾讯 WorkBuddy 面试准备".into();
        let mut topic = snapshot.projects[0].clone();
        topic.kind = ProjectKind::Semantic;
        topic.display_name = "面试准备".into();
        topic.sessions[0].topic_label = Some("面试准备".into());
        snapshot.topics = vec![topic];
        state.projects.snapshot = snapshot;
        state.expanded_project_keys.insert("p1".into());

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
        let topics_text = rendered_text(&state, Rect::new(0, 0, 80, 12));
        assert!(
            !topics_text.contains("简历"),
            "Topics should not repeat the subject label:\n{topics_text}"
        );
        assert!(
            topics_text.contains("整理腾讯"),
            "Topics should keep the task text:\n{topics_text}"
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
            .rposition(|cell| {
                !cell.symbol.trim().is_empty() && !matches!(cell.symbol.as_str(), "│" | "┃" | "↔")
            })
            .map_or(0, |index| index + 1);
        assert!(
            painted < 96,
            "an unclipped 192-column title reached the row: {painted} columns"
        );
    }

    /// `surface_dim` and `panel_bg` are background slots. `surface_dim` sits at roughly 1.1:1
    /// against `panel_bg`, so any glyph drawn in either colour is invisible on the panel. This
    /// walks every cell and permits these foregrounds only as contrast text on an accent fill.
    ///
    /// The snapshot deliberately carries every row variant — thin, automation, load-older and
    /// scan status — because a guard that never renders a variant cannot protect it.
    #[test]
    fn background_foregrounds_are_only_used_on_accent_fills() {
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
                    if cell.style().bg == Some(state.palette.accent) {
                        assert_eq!(
                            fg,
                            Some(super::super::widgets::panel_contrast_fg(&state.palette))
                        );
                        continue;
                    }
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

        let dir = std::env::temp_dir().join("herduck-history-preview");
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
    }

    #[test]
    fn full_history_renders_lines_that_the_preview_elides() {
        use std::io::Write as _;

        let dir = std::env::temp_dir().join("herduck-history-full-context");
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
            text.contains("Enter") && text.contains("Shift+Enter"),
            "composer missing:\n{text}"
        );
        assert!(text.contains("Enter resumes"));
        assert!(!text.contains('恢'));
    }

    #[test]
    fn history_resume_and_sidebar_controls_follow_the_selected_language() {
        let mut state = AppState::test_new();
        state.projects.snapshot = snapshot();
        state.projects.history_session_key = Some("s1".into());
        state.projects.history_view = crate::app::state::ProjectHistoryView::Full;
        state.sidebar_view = crate::app::state::SidebarView::Projects;
        for language in [
            crate::config::TitleLanguage::Chinese,
            crate::config::TitleLanguage::English,
        ] {
            state.title_language = language;
            let sidebar = rendered_text(&state, Rect::new(0, 0, 60, 20));
            assert!(sidebar.contains("Projects"), "{sidebar}");
            assert!(
                sidebar.contains(language.text("search projects", "搜索项目")),
                "{sidebar}"
            );
            assert!(sidebar.contains(language.text("all", "全部")), "{sidebar}");
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 24)).unwrap();
            terminal
                .draw(|frame| render_project_history(&state, frame, frame.area()))
                .unwrap();
            let buffer = terminal.backend().buffer();
            let text = (0..24)
                .map(|y| {
                    let mut line = String::new();
                    let mut x = 0;
                    while x < 100 {
                        let symbol = buffer[(x, y)].symbol();
                        line.push_str(symbol);
                        x += crate::ui::text::display_width(symbol).max(1) as u16;
                    }
                    line
                })
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                text.contains(language.text("Enter resumes", "Enter 恢复")),
                "{text}"
            );
            assert!(
                text.contains(language.text("Type a message", "输入消息")),
                "{text}"
            );
            assert!(
                !text.contains(language.text("Enter 恢复", "Enter resumes")),
                "{text}"
            );
        }
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

        let dir = std::env::temp_dir().join("herduck-history-markdown");
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
    fn unclassified_sessions_render_without_an_internal_parent_node() {
        let mut state = AppState::test_new();
        let mut snapshot = snapshot();
        snapshot.projects[0].kind = ProjectKind::Unclassified;
        snapshot.projects[0].display_name = "Unclassified".into();
        state.projects.snapshot = snapshot;
        state.sidebar_view = crate::app::state::SidebarView::Projects;

        let rows = project_tree_rows(&state);
        assert!(
            matches!(&rows[..], [ProjectTreeRow::Session(session)] if session.stable_key == "s1")
        );
        let text = rendered_text(&state, Rect::new(0, 0, 40, 10));
        assert!(!text.to_lowercase().contains("unclass"));
    }

    #[test]
    fn directory_and_topic_modes_render_independent_parent_groups() {
        let mut state = AppState::test_new();
        let mut snapshot = snapshot();
        let mut topic = snapshot.projects[0].clone();
        topic.canonical_key = "topic-1".into();
        topic.kind = ProjectKind::Semantic;
        topic.display_name = "HERDUCK architecture".into();
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
                if display_name == "HERDUCK architecture"
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
