use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::time::Duration;

use super::catalog::{CatalogError, ScanCompletion};
use super::domain::{
    PendingSemanticSession, PendingTitleSession, SemanticAssignment, SemanticTopicMerge,
    SessionTitleUpdate,
};
use super::{
    ProjectCatalog, ProjectSessionsPage, ProjectsSnapshot, SessionCandidate, SessionCursor,
};

const PROJECT_PAGE_SIZE: usize = 50;
const SCAN_WRITE_BATCH_SIZE: usize = 256;
/// How often a live server checks provider history for newly-created sessions.
///
/// Adapter scanners keep per-root watermarks and return a reused cache when nothing changed, so
/// this refresh is cheap in the steady state while ensuring a long-lived TUI does not show a
/// stale Projects/Topics tree indefinitely.
const BACKGROUND_SCAN_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectServiceError {
    pub code: &'static str,
    pub message: String,
}

impl ProjectServiceError {
    fn catalog(error: CatalogError) -> Self {
        let code = match error {
            CatalogError::NotFound => "not_found",
            CatalogError::AliasConflict => "alias_conflict",
            CatalogError::CrossBackendAlias => "cross_backend_alias",
            CatalogError::InvalidTopicMerge => "invalid_topic_merge",
            CatalogError::Corrupt => "catalog_corrupt",
            CatalogError::UnsupportedSchema(_) => "unsupported_schema",
            CatalogError::Sqlite(_) | CatalogError::Io(_) => "catalog_error",
        };
        Self {
            code,
            message: error.to_string(),
        }
    }

    fn unavailable() -> Self {
        Self {
            code: "catalog_unavailable",
            message: "Project Catalog is unavailable".to_string(),
        }
    }

    fn summary_cancelled() -> Self {
        Self {
            code: "summary_cancelled",
            message: "Summary settings changed; discarded previous work".to_string(),
        }
    }
}

pub(crate) enum ProjectCommand {
    ConfigureSummaries {
        language: crate::config::TitleLanguage,
        reply: mpsc::Sender<Result<u64, ProjectServiceError>>,
    },
    TitleLanguage {
        reply: mpsc::Sender<Result<crate::config::TitleLanguage, ProjectServiceError>>,
    },
    RenameSession {
        session_key: String,
        title: String,
        reply: mpsc::Sender<Result<u64, ProjectServiceError>>,
    },
    Upsert {
        candidate: Box<SessionCandidate>,
        reply: mpsc::Sender<Result<u64, ProjectServiceError>>,
    },
    UpsertScanBatch {
        candidates: Vec<SessionCandidate>,
        reply: mpsc::Sender<Result<u64, ProjectServiceError>>,
    },
    Assign {
        session_key: String,
        project_key: String,
        locked: bool,
        observed_at: i64,
        reply: mpsc::Sender<Result<u64, ProjectServiceError>>,
    },
    Unlock {
        session_key: String,
        observed_at: i64,
        reply: mpsc::Sender<Result<u64, ProjectServiceError>>,
    },
    SessionsPage {
        project_key: String,
        cursor: Option<SessionCursor>,
        limit: usize,
        reply: mpsc::Sender<Result<ProjectSessionsPage, ProjectServiceError>>,
    },
    ClearRuntime {
        session_key: String,
        generation: u64,
        reply: mpsc::Sender<Result<u64, ProjectServiceError>>,
    },
    CompleteScan {
        adapter: String,
        root_key: String,
        completion: ScanCompletion,
        seen_source_keys: HashSet<String>,
        excluded_source_keys: HashSet<String>,
        observed_at: i64,
        reply: mpsc::Sender<Result<u64, ProjectServiceError>>,
    },
    /// Sessions whose topic is missing or stale, for the classifier worker to pick up.
    PendingSemantic {
        limit: usize,
        reply: mpsc::Sender<Result<Vec<PendingSemanticSession>, ProjectServiceError>>,
    },
    /// Topic labels already in use, so later batches can reuse them.
    KnownTopics {
        limit: usize,
        reply: mpsc::Sender<Result<Vec<String>, ProjectServiceError>>,
    },
    BeginTopicMerge {
        cancelled: Arc<AtomicBool>,
        now: i64,
        reply: mpsc::Sender<Result<Vec<String>, ProjectServiceError>>,
    },
    ApplyTopicMerges {
        cancelled: Arc<AtomicBool>,
        merges: Vec<SemanticTopicMerge>,
        observed_at: i64,
        reply: mpsc::Sender<Result<u64, ProjectServiceError>>,
    },
    /// One classified batch, applied through the same serialized writer as every other mutation.
    ApplySemantic {
        cancelled: Arc<AtomicBool>,
        batch: Vec<SemanticAssignment>,
        observed_at: i64,
        reply: mpsc::Sender<Result<u64, ProjectServiceError>>,
    },
    PendingTitles {
        limit: usize,
        reply: mpsc::Sender<Result<Vec<PendingTitleSession>, ProjectServiceError>>,
    },
    ClaimTitles {
        cancelled: Arc<AtomicBool>,
        keys: Vec<String>,
        reply: mpsc::Sender<Result<u64, ProjectServiceError>>,
    },
    ApplyTitles {
        cancelled: Arc<AtomicBool>,
        updates: Vec<SessionTitleUpdate>,
        reply: mpsc::Sender<Result<u64, ProjectServiceError>>,
    },
    Shutdown,
}

pub(crate) struct ProjectService {
    sender: Option<mpsc::Sender<ProjectCommand>>,
    snapshot: Arc<RwLock<ProjectsSnapshot>>,
    worker: Option<std::thread::JoinHandle<()>>,
    scan_workers: Vec<std::thread::JoinHandle<()>>,
    summary_workers: Vec<std::thread::JoinHandle<()>>,
    summary_cancelled: Arc<AtomicBool>,
    scanner: Option<Arc<Mutex<super::adapters::AdapterScanner>>>,
    scans_in_progress: Arc<AtomicUsize>,
    shutdown: Arc<AtomicBool>,
}

impl ProjectService {
    #[cfg(test)]
    pub(crate) fn open(path: &Path, event_hub: crate::api::EventHub) -> Self {
        Self::open_with_config(path, event_hub, &crate::config::ProjectsConfig::default())
    }

    pub(crate) fn open_with_config(
        path: &Path,
        event_hub: crate::api::EventHub,
        config: &crate::config::ProjectsConfig,
    ) -> Self {
        match ProjectCatalog::open_with_config(path, config) {
            Ok(catalog) => Self::from_catalog(catalog, event_hub),
            Err(error) => {
                tracing::warn!(
                    category = "catalog_open",
                    "Project Catalog unavailable: {error}"
                );
                Self::degraded("catalog_open")
            }
        }
    }

    pub(crate) fn disabled() -> Self {
        Self {
            sender: None,
            snapshot: Arc::new(RwLock::new(ProjectsSnapshot::empty())),
            worker: None,
            scan_workers: Vec::new(),
            summary_workers: Vec::new(),
            summary_cancelled: Arc::new(AtomicBool::new(false)),
            scanner: None,
            scans_in_progress: Arc::new(AtomicUsize::new(0)),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    #[cfg(test)]
    pub(crate) fn in_memory(event_hub: crate::api::EventHub) -> Self {
        match ProjectCatalog::open_in_memory() {
            Ok(catalog) => Self::from_catalog(catalog, event_hub),
            Err(error) => panic!("open in-memory Project Catalog: {error}"),
        }
    }

    fn degraded(category: &str) -> Self {
        Self {
            sender: None,
            snapshot: Arc::new(RwLock::new(ProjectsSnapshot::degraded(category))),
            worker: None,
            scan_workers: Vec::new(),
            summary_workers: Vec::new(),
            summary_cancelled: Arc::new(AtomicBool::new(false)),
            scanner: None,
            scans_in_progress: Arc::new(AtomicUsize::new(0)),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    fn from_catalog(mut catalog: ProjectCatalog, event_hub: crate::api::EventHub) -> Self {
        if let Err(error) = catalog.reset_running_titles() {
            tracing::warn!(
                category = "title_runtime_reset",
                "Title state reset failed: {error}"
            );
        }
        if let Err(error) = catalog.clear_all_runtime_mappings() {
            tracing::warn!(
                category = "catalog_runtime_reset",
                "Project Catalog runtime reset failed: {error}"
            );
            return Self::degraded("catalog_runtime_reset");
        }
        let initial = catalog
            .snapshot(PROJECT_PAGE_SIZE)
            .unwrap_or_else(|_| ProjectsSnapshot::degraded("catalog_snapshot"));
        let snapshot = Arc::new(RwLock::new(initial));
        let worker_snapshot = Arc::clone(&snapshot);
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = Arc::clone(&shutdown);
        let (sender, receiver) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("herduck-project-catalog".to_string())
            .spawn(move || {
                while let Ok(command) = receiver.recv() {
                    if worker_shutdown.load(Ordering::Acquire) {
                        break;
                    }
                    if !process_command(&mut catalog, &worker_snapshot, &event_hub, command) {
                        break;
                    }
                }
            })
            .ok();

        if worker.is_none() {
            return Self::degraded("catalog_worker_start");
        }
        Self {
            sender: Some(sender),
            snapshot,
            worker,
            scan_workers: Vec::new(),
            summary_workers: Vec::new(),
            summary_cancelled: Arc::new(AtomicBool::new(false)),
            scanner: Some(Arc::new(Mutex::new(
                super::adapters::AdapterScanner::default(),
            ))),
            scans_in_progress: Arc::new(AtomicUsize::new(0)),
            shutdown,
        }
    }

    pub(crate) fn is_available(&self) -> bool {
        self.sender.is_some()
    }

    pub(crate) fn start_background_scan(&mut self, roots: &[super::adapters::AdapterRoot]) {
        let (Some(sender), Some(scanner)) = (self.sender.clone(), self.scanner.clone()) else {
            return;
        };
        let roots = roots.to_vec();
        let scans_in_progress = Arc::clone(&self.scans_in_progress);
        let shutdown = Arc::clone(&self.shutdown);
        match std::thread::Builder::new()
            .name("herduck-project-scan".to_string())
            .spawn(move || {
                loop {
                    if shutdown.load(Ordering::Acquire) {
                        return;
                    }
                    scans_in_progress.fetch_add(1, Ordering::Release);
                    run_background_scan(
                        sender.clone(),
                        Arc::clone(&scanner),
                        roots.clone(),
                        &shutdown,
                    );
                    scans_in_progress.fetch_sub(1, Ordering::Release);

                    // Sleep in short slices so shutdown and live handoff do not have to wait for
                    // the entire refresh interval.
                    let mut remaining = BACKGROUND_SCAN_INTERVAL;
                    while remaining > Duration::ZERO {
                        if shutdown.load(Ordering::Acquire) {
                            return;
                        }
                        let pause = remaining.min(Duration::from_millis(250));
                        std::thread::sleep(pause);
                        remaining = remaining.saturating_sub(pause);
                    }
                }
            }) {
            Ok(worker) => self.scan_workers.push(worker),
            Err(error) => {
                tracing::warn!(
                    category = "adapter_worker_start",
                    "Project adapter worker failed to start: {error}"
                );
            }
        }
    }

    pub(crate) fn snapshot(&self) -> ProjectsSnapshot {
        self.snapshot
            .read()
            .map(|snapshot| snapshot.clone())
            .unwrap_or_else(|_| ProjectsSnapshot::degraded("catalog_snapshot_lock"))
    }

    pub(crate) fn upsert_candidate(
        &self,
        candidate: SessionCandidate,
    ) -> Result<u64, ProjectServiceError> {
        self.request(|reply| ProjectCommand::Upsert {
            candidate: Box::new(candidate),
            reply,
        })
    }

    pub(crate) fn assign_session(
        &self,
        session_key: String,
        project_key: String,
        locked: bool,
        observed_at: i64,
    ) -> Result<u64, ProjectServiceError> {
        self.request(|reply| ProjectCommand::Assign {
            session_key,
            project_key,
            locked,
            observed_at,
            reply,
        })
    }

    pub(crate) fn rename_session(
        &self,
        session_key: String,
        title: String,
    ) -> Result<u64, ProjectServiceError> {
        let title = title.trim().to_string();
        if title.is_empty() || title.chars().count() > 200 || title.chars().any(char::is_control) {
            return Err(ProjectServiceError {
                code: "invalid_title",
                message: "Title must contain 1–200 characters without control characters".into(),
            });
        }
        self.request(|reply| ProjectCommand::RenameSession {
            session_key,
            title,
            reply,
        })
    }

    pub(crate) fn unlock_session(
        &self,
        session_key: String,
        observed_at: i64,
    ) -> Result<u64, ProjectServiceError> {
        self.request(|reply| ProjectCommand::Unlock {
            session_key,
            observed_at,
            reply,
        })
    }

    /// Reconfigure only summary work, preserving discovery, the Catalog and live session mappings.
    pub(crate) fn summaries_need_retry(&self) -> bool {
        self.summary_cancelled.load(Ordering::Acquire)
    }

    pub(crate) fn configure_summaries(
        &mut self,
        summary: &crate::config::SummaryConfig,
    ) -> Result<(), ProjectServiceError> {
        self.summary_cancelled.store(true, Ordering::Release);
        self.summary_workers.clear();
        // This serialized barrier follows any already accepted result. Later old-generation
        // claims/results carry the cancelled token and cannot undo the user's new preference.
        self.request(|reply| ProjectCommand::ConfigureSummaries {
            language: summary.title_language,
            reply,
        })?;
        self.summary_cancelled = Arc::new(AtomicBool::new(false));
        let config = super::semantic::SemanticConfig::from_summary(summary);
        let result = self.start_semantic_classification(config).and_then(|()| {
            self.start_title_generation(super::semantic::SemanticConfig::for_titles(summary))
        });
        if result.is_err() {
            self.summary_cancelled.store(true, Ordering::Release);
            self.summary_workers.clear();
        }
        result
    }

    /// Starts the topic classification worker.
    ///
    /// Runs off the input and render path, exactly like the file scan: classification calls out
    /// to another process and can take minutes, so it must never block a keystroke.
    fn start_semantic_classification(
        &mut self,
        config: super::semantic::SemanticConfig,
    ) -> Result<(), ProjectServiceError> {
        if !config.enabled
            || matches!(
                config.mode,
                crate::config::SummaryModeConfig::Local | crate::config::SummaryModeConfig::Pending
            )
            || config.backends.is_empty()
        {
            return Ok(());
        }
        let Some(sender) = self.sender.clone() else {
            return Err(ProjectServiceError::unavailable());
        };
        let shutdown = Arc::clone(&self.summary_cancelled);
        let worker = std::thread::Builder::new()
            .name("herduck-project-semantic".to_string())
            .spawn(move || {
                if !shutdown.load(Ordering::Acquire) {
                    super::semantic::run_classification_worker(&sender, &config, &shutdown);
                }
            })
            .map_err(|error| ProjectServiceError {
                code: "semantic_worker_start",
                message: format!("Could not start topic summaries: {error}"),
            })?;
        self.summary_workers.push(worker);
        Ok(())
    }

    /// Starts the asynchronous title generator after the adapter scan has populated the Catalog.
    fn start_title_generation(
        &mut self,
        config: super::semantic::SemanticConfig,
    ) -> Result<(), ProjectServiceError> {
        if !config.enabled || config.mode == crate::config::SummaryModeConfig::Pending {
            return Ok(());
        }
        let Some(sender) = self.sender.clone() else {
            return Err(ProjectServiceError::unavailable());
        };
        let shutdown = Arc::clone(&self.summary_cancelled);
        let worker = std::thread::Builder::new()
            .name("herduck-session-titles".to_string())
            .spawn(move || {
                if !shutdown.load(Ordering::Acquire) {
                    super::title::run_title_generation_worker(&sender, &config, &shutdown);
                }
            })
            .map_err(|error| ProjectServiceError {
                code: "title_worker_start",
                message: format!("Could not start title summaries: {error}"),
            })?;
        self.summary_workers.push(worker);
        Ok(())
    }

    pub(crate) fn sessions_page(
        &self,
        project_key: String,
        cursor: Option<SessionCursor>,
        limit: usize,
    ) -> Result<ProjectSessionsPage, ProjectServiceError> {
        let sender = self
            .sender
            .as_ref()
            .ok_or_else(ProjectServiceError::unavailable)?;
        let (reply_tx, reply_rx) = mpsc::channel();
        sender
            .send(ProjectCommand::SessionsPage {
                project_key,
                cursor,
                limit,
                reply: reply_tx,
            })
            .map_err(|_| ProjectServiceError::unavailable())?;
        reply_rx
            .recv()
            .map_err(|_| ProjectServiceError::unavailable())?
    }

    pub(crate) fn clear_runtime_mapping(
        &self,
        session_key: String,
        generation: u64,
    ) -> Result<u64, ProjectServiceError> {
        self.request(|reply| ProjectCommand::ClearRuntime {
            session_key,
            generation,
            reply,
        })
    }

    fn request(
        &self,
        command: impl FnOnce(mpsc::Sender<Result<u64, ProjectServiceError>>) -> ProjectCommand,
    ) -> Result<u64, ProjectServiceError> {
        let sender = self
            .sender
            .as_ref()
            .ok_or_else(ProjectServiceError::unavailable)?;
        let (reply_tx, reply_rx) = mpsc::channel();
        sender
            .send(command(reply_tx))
            .map_err(|_| ProjectServiceError::unavailable())?;
        reply_rx
            .recv()
            .map_err(|_| ProjectServiceError::unavailable())?
    }
}

impl Drop for ProjectService {
    fn drop(&mut self) {
        self.summary_cancelled.store(true, Ordering::Release);
        self.summary_workers.clear();
        self.shutdown.store(true, Ordering::Release);
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(ProjectCommand::Shutdown);
        }
        // Adapter scans and semantic backends may be blocked in filesystem or
        // subprocess work. They observe `shutdown` between bounded operations;
        // detaching their handles prevents process shutdown from waiting on
        // unrelated background discovery work.
        self.scan_workers.clear();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run_background_scan(
    sender: mpsc::Sender<ProjectCommand>,
    scanner: Arc<Mutex<super::adapters::AdapterScanner>>,
    roots: Vec<super::adapters::AdapterRoot>,
    shutdown: &AtomicBool,
) {
    for root in roots {
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        let scan = match scanner.lock() {
            Ok(mut scanner) => scanner.scan_root(&root, false),
            Err(_) => {
                tracing::warn!(
                    adapter = root.adapter,
                    category = "adapter_cache_lock",
                    "Project adapter cache is unavailable"
                );
                continue;
            }
        };
        tracing::debug!(
            adapter = scan.adapter,
            root = %scan.root_key,
            reused_cache = scan.reused_cache,
            candidates = scan.candidates.len(),
            excluded = scan.excluded_source_keys.len(),
            completion = ?scan.completion,
            "Project adapter scan completed"
        );
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        let mut completion = scan.completion;
        if !scan.reused_cache {
            let mut candidates = scan.candidates.into_iter();
            loop {
                let batch = candidates
                    .by_ref()
                    .take(SCAN_WRITE_BATCH_SIZE)
                    .collect::<Vec<_>>();
                if batch.is_empty() {
                    break;
                }
                let result = request_on_sender(&sender, |reply| ProjectCommand::UpsertScanBatch {
                    candidates: batch,
                    reply,
                });
                if let Err(error) = result {
                    tracing::warn!(
                        adapter = scan.adapter,
                        category = error.code,
                        "Project adapter candidate was rejected"
                    );
                    if completion == ScanCompletion::Complete {
                        completion = ScanCompletion::Degraded;
                    }
                }
            }
        }
        let observed_at = super::runtime::unix_time_ms();
        if let Err(error) = request_on_sender(&sender, |reply| ProjectCommand::CompleteScan {
            adapter: scan.adapter.to_string(),
            root_key: scan.root_key,
            completion,
            seen_source_keys: scan.seen_source_keys,
            excluded_source_keys: scan.excluded_source_keys,
            observed_at,
            reply,
        }) {
            tracing::warn!(
                adapter = scan.adapter,
                category = error.code,
                "Project adapter completion was not committed"
            );
        }
    }
}

fn request_on_sender(
    sender: &mpsc::Sender<ProjectCommand>,
    command: impl FnOnce(mpsc::Sender<Result<u64, ProjectServiceError>>) -> ProjectCommand,
) -> Result<u64, ProjectServiceError> {
    let (reply_tx, reply_rx) = mpsc::channel();
    sender
        .send(command(reply_tx))
        .map_err(|_| ProjectServiceError::unavailable())?;
    reply_rx
        .recv()
        .map_err(|_| ProjectServiceError::unavailable())?
}

/// Reads sessions awaiting classification from the classifier worker thread.
pub(crate) fn request_pending_semantic(
    sender: &mpsc::Sender<ProjectCommand>,
    limit: usize,
) -> Result<Vec<PendingSemanticSession>, ProjectServiceError> {
    let (reply_tx, reply_rx) = mpsc::channel();
    sender
        .send(ProjectCommand::PendingSemantic {
            limit,
            reply: reply_tx,
        })
        .map_err(|_| ProjectServiceError::unavailable())?;
    reply_rx
        .recv()
        .map_err(|_| ProjectServiceError::unavailable())?
}

/// Reads topics already in use from the classifier worker thread.
pub(crate) fn request_known_topics(
    sender: &mpsc::Sender<ProjectCommand>,
    limit: usize,
) -> Result<Vec<String>, ProjectServiceError> {
    let (reply_tx, reply_rx) = mpsc::channel();
    sender
        .send(ProjectCommand::KnownTopics {
            limit,
            reply: reply_tx,
        })
        .map_err(|_| ProjectServiceError::unavailable())?;
    reply_rx
        .recv()
        .map_err(|_| ProjectServiceError::unavailable())?
}

/// Applies a classified batch from the classifier worker thread.
pub(crate) fn request_apply_semantic(
    sender: &mpsc::Sender<ProjectCommand>,
    cancelled: &Arc<AtomicBool>,
    batch: Vec<SemanticAssignment>,
    observed_at: i64,
) -> Result<u64, ProjectServiceError> {
    request_on_sender(sender, |reply| ProjectCommand::ApplySemantic {
        cancelled: Arc::clone(cancelled),
        batch,
        observed_at,
        reply,
    })
}

pub(crate) fn request_title_language(
    sender: &mpsc::Sender<ProjectCommand>,
) -> Result<crate::config::TitleLanguage, ProjectServiceError> {
    let (reply, receiver) = mpsc::channel();
    sender
        .send(ProjectCommand::TitleLanguage { reply })
        .map_err(|_| ProjectServiceError::unavailable())?;
    receiver
        .recv()
        .map_err(|_| ProjectServiceError::unavailable())?
}

pub(crate) fn request_pending_titles(
    sender: &mpsc::Sender<ProjectCommand>,
    limit: usize,
) -> Result<Vec<PendingTitleSession>, ProjectServiceError> {
    let (reply_tx, reply_rx) = mpsc::channel();
    sender
        .send(ProjectCommand::PendingTitles {
            limit,
            reply: reply_tx,
        })
        .map_err(|_| ProjectServiceError::unavailable())?;
    reply_rx
        .recv()
        .map_err(|_| ProjectServiceError::unavailable())?
}

pub(crate) fn request_apply_titles(
    sender: &mpsc::Sender<ProjectCommand>,
    cancelled: &Arc<AtomicBool>,
    updates: Vec<SessionTitleUpdate>,
) -> Result<u64, ProjectServiceError> {
    request_on_sender(sender, |reply| ProjectCommand::ApplyTitles {
        cancelled: Arc::clone(cancelled),
        updates,
        reply,
    })
}

pub(crate) fn request_claim_titles(
    sender: &mpsc::Sender<ProjectCommand>,
    cancelled: &Arc<AtomicBool>,
    keys: Vec<String>,
) -> Result<u64, ProjectServiceError> {
    request_on_sender(sender, |reply| ProjectCommand::ClaimTitles {
        cancelled: Arc::clone(cancelled),
        keys,
        reply,
    })
}

pub(crate) fn request_begin_topic_merge(
    sender: &mpsc::Sender<ProjectCommand>,
    cancelled: &Arc<AtomicBool>,
    now: i64,
) -> Result<Vec<String>, ProjectServiceError> {
    let (reply_tx, reply_rx) = mpsc::channel();
    sender
        .send(ProjectCommand::BeginTopicMerge {
            cancelled: Arc::clone(cancelled),
            now,
            reply: reply_tx,
        })
        .map_err(|_| ProjectServiceError::unavailable())?;
    reply_rx
        .recv()
        .map_err(|_| ProjectServiceError::unavailable())?
}

pub(crate) fn request_apply_topic_merges(
    sender: &mpsc::Sender<ProjectCommand>,
    cancelled: &Arc<AtomicBool>,
    merges: Vec<SemanticTopicMerge>,
    observed_at: i64,
) -> Result<u64, ProjectServiceError> {
    request_on_sender(sender, |reply| ProjectCommand::ApplyTopicMerges {
        cancelled: Arc::clone(cancelled),
        merges,
        observed_at,
        reply,
    })
}

fn process_command(
    catalog: &mut ProjectCatalog,
    snapshot: &RwLock<ProjectsSnapshot>,
    event_hub: &crate::api::EventHub,
    command: ProjectCommand,
) -> bool {
    match &command {
        ProjectCommand::ClaimTitles {
            cancelled, reply, ..
        }
        | ProjectCommand::ApplyTitles {
            cancelled, reply, ..
        }
        | ProjectCommand::ApplySemantic {
            cancelled, reply, ..
        }
        | ProjectCommand::ApplyTopicMerges {
            cancelled, reply, ..
        } if cancelled.load(Ordering::Acquire) => {
            let _ = reply.send(Err(ProjectServiceError::summary_cancelled()));
            return true;
        }
        ProjectCommand::BeginTopicMerge {
            cancelled, reply, ..
        } if cancelled.load(Ordering::Acquire) => {
            let _ = reply.send(Err(ProjectServiceError::summary_cancelled()));
            return true;
        }
        _ => {}
    }
    match command {
        ProjectCommand::ConfigureSummaries { language, reply } => {
            finish_mutation(catalog, snapshot, event_hub, reply, |catalog| {
                catalog.reset_running_titles()?;
                // Also advances the revision so clients see running titles requeued.
                catalog.set_title_language(language)
            });
        }
        ProjectCommand::Upsert { candidate, reply } => {
            finish_mutation(catalog, snapshot, event_hub, reply, |catalog| {
                catalog.upsert_candidate(&candidate)
            });
        }
        ProjectCommand::UpsertScanBatch { candidates, reply } => {
            let result = catalog
                .upsert_scanned_candidates(&candidates)
                .map_err(ProjectServiceError::catalog);
            let _ = reply.send(result);
        }
        ProjectCommand::Assign {
            session_key,
            project_key,
            locked,
            observed_at,
            reply,
        } => finish_mutation(catalog, snapshot, event_hub, reply, |catalog| {
            catalog.assign_session(&session_key, &project_key, locked, observed_at)
        }),
        ProjectCommand::TitleLanguage { reply } => {
            let _ = reply.send(
                catalog
                    .title_language()
                    .map_err(ProjectServiceError::catalog),
            );
        }
        ProjectCommand::RenameSession {
            session_key,
            title,
            reply,
        } => finish_mutation(catalog, snapshot, event_hub, reply, |catalog| {
            catalog.rename_session(&session_key, &title)
        }),
        ProjectCommand::Unlock {
            session_key,
            observed_at,
            reply,
        } => finish_mutation(catalog, snapshot, event_hub, reply, |catalog| {
            catalog.unlock_session(&session_key, observed_at)
        }),
        ProjectCommand::KnownTopics { limit, reply } => {
            let result = catalog
                .known_topics(limit)
                .map_err(ProjectServiceError::catalog);
            let _ = reply.send(result);
        }
        ProjectCommand::BeginTopicMerge { now, reply, .. } => {
            let result = catalog
                .begin_topic_merge(now)
                .map_err(ProjectServiceError::catalog);
            let _ = reply.send(result);
        }
        ProjectCommand::ApplyTopicMerges {
            merges,
            observed_at,
            reply,
            ..
        } => finish_mutation(catalog, snapshot, event_hub, reply, |catalog| {
            catalog.apply_topic_merges(&merges, observed_at)
        }),
        ProjectCommand::PendingSemantic { limit, reply } => {
            let result = catalog
                .pending_semantic_sessions(limit)
                .map_err(ProjectServiceError::catalog);
            let _ = reply.send(result);
        }
        ProjectCommand::ApplySemantic {
            batch,
            observed_at,
            reply,
            ..
        } => finish_mutation(catalog, snapshot, event_hub, reply, |catalog| {
            catalog.apply_semantic_batch(&batch, observed_at)
        }),
        ProjectCommand::PendingTitles { limit, reply } => {
            let result = catalog
                .pending_title_sessions(limit)
                .map_err(ProjectServiceError::catalog);
            let _ = reply.send(result);
        }
        ProjectCommand::ClaimTitles { keys, reply, .. } => {
            finish_mutation(catalog, snapshot, event_hub, reply, |catalog| {
                catalog.claim_title_batch(&keys)
            });
        }
        ProjectCommand::ApplyTitles { updates, reply, .. } => {
            finish_mutation(catalog, snapshot, event_hub, reply, |catalog| {
                catalog.apply_title_batch(&updates)
            });
        }
        ProjectCommand::SessionsPage {
            project_key,
            cursor,
            limit,
            reply,
        } => {
            let result = catalog
                .sessions_page(&project_key, cursor.as_ref(), limit)
                .and_then(|(sessions, next_cursor)| {
                    Ok(ProjectSessionsPage {
                        projects_schema_version: crate::projects::domain::PROJECTS_SCHEMA_VERSION,
                        revision: catalog.revision()?,
                        project_key,
                        sessions,
                        next_cursor,
                    })
                })
                .map_err(ProjectServiceError::catalog);
            let _ = reply.send(result);
        }
        ProjectCommand::ClearRuntime {
            session_key,
            generation,
            reply,
        } => finish_mutation(catalog, snapshot, event_hub, reply, |catalog| {
            catalog.clear_runtime_mapping(&session_key, generation)
        }),
        ProjectCommand::CompleteScan {
            adapter,
            root_key,
            completion,
            seen_source_keys,
            excluded_source_keys,
            observed_at,
            reply,
        } => finish_mutation(catalog, snapshot, event_hub, reply, |catalog| {
            catalog.complete_root_scan(
                &adapter,
                &root_key,
                completion,
                &seen_source_keys,
                &excluded_source_keys,
                observed_at,
            )
        }),
        ProjectCommand::Shutdown => return false,
    }
    true
}

fn finish_mutation(
    catalog: &mut ProjectCatalog,
    snapshot: &RwLock<ProjectsSnapshot>,
    event_hub: &crate::api::EventHub,
    reply: mpsc::Sender<Result<u64, ProjectServiceError>>,
    mutation: impl FnOnce(&mut ProjectCatalog) -> Result<u64, CatalogError>,
) {
    let result = mutation(catalog).map_err(ProjectServiceError::catalog);
    let result = match result {
        Ok(revision) => match catalog.snapshot(PROJECT_PAGE_SIZE) {
            Ok(next_snapshot) => {
                if let Ok(mut cached) = snapshot.write() {
                    *cached = next_snapshot.clone();
                }
                event_hub.push(crate::api::schema::EventEnvelope {
                    event: crate::api::schema::EventKind::ProjectSnapshotUpdated,
                    data: crate::api::schema::EventData::ProjectSnapshotUpdated {
                        revision,
                        scan_status: next_snapshot.scan_status,
                    },
                });
                Ok(revision)
            }
            Err(error) => Err(ProjectServiceError::catalog(error)),
        },
        Err(error) => Err(error),
    };
    let _ = reply.send(result);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::{RuntimeMapping, SessionCandidate, SessionIdentity};

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "herdr-project-service-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).expect("service fixture root");
        path
    }

    fn candidate(id: &str) -> SessionCandidate {
        SessionCandidate {
            identity: SessionIdentity::id("codex", id).unwrap(),
            title: None,
            cwd: None,
            transcript_ref: None,
            first_activity_at: 1,
            last_activity_at: 2,
            adapter: "codex".to_string(),
            root_key: "root".to_string(),
            source_key: id.to_string(),
            observed_at: 2,
            aliases: Vec::new(),
            runtime: None,
            weight: Default::default(),
            session_class: Some(crate::projects::SessionClass::Interactive),
        }
    }

    #[test]
    fn local_summary_mode_does_not_start_a_topic_classifier() {
        let mut service = ProjectService::in_memory(crate::api::EventHub::default());
        service
            .start_semantic_classification(super::super::semantic::SemanticConfig {
                enabled: true,
                mode: crate::config::SummaryModeConfig::Local,
                ..super::super::semantic::SemanticConfig::default()
            })
            .unwrap();
        assert!(service.summary_workers.is_empty());
    }

    #[test]
    fn pending_starts_no_summary_workers_and_local_only_starts_titles() {
        let mut service = ProjectService::in_memory(crate::api::EventHub::default());
        let mut summary = crate::config::SummaryConfig::default();
        service.configure_summaries(&summary).unwrap();
        assert!(service.summary_workers.is_empty());
        summary.mode = crate::config::SummaryModeConfig::Local;
        service.configure_summaries(&summary).unwrap();
        assert_eq!(service.summary_workers.len(), 1);
        let old = Arc::clone(&service.summary_cancelled);
        summary.mode = crate::config::SummaryModeConfig::Pending;
        service.configure_summaries(&summary).unwrap();
        assert!(old.load(Ordering::Acquire));
        assert!(service.summary_workers.is_empty());
        assert!(!service.shutdown.load(Ordering::Acquire));
    }

    #[test]
    fn empty_naming_override_finishes_local_titles_without_failed_retries() {
        use crate::projects::{CandidateField, SourcePriority};
        let root = temp_dir("empty-naming-chain");
        let path = root.join("catalog.sqlite3");
        let mut service = ProjectService::open(&path, crate::api::EventHub::default());
        let mut session = candidate("offline-name");
        session.title = Some(CandidateField {
            value: "Improve terminal session search and keyboard navigation".into(),
            observed_at: 2,
            priority: SourcePriority::PrimaryIndex,
            source_key: "fixture".into(),
        });
        service.upsert_candidate(session).unwrap();
        let summary = crate::config::SummaryConfig {
            mode: crate::config::SummaryModeConfig::Auto,
            providers: vec![],
            title_providers: Some(vec![]),
            ..crate::config::SummaryConfig::default()
        };
        service.configure_summaries(&summary).unwrap();
        let connection = rusqlite::Connection::open(&path).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let (source, status, error): (String, String, Option<String>) = connection
                .query_row(
                    "SELECT title_source,title_status,title_error FROM sessions",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
            assert_ne!(
                status, "failed",
                "an explicit empty list is not a backend failure"
            );
            if status == "done" {
                assert_eq!(source, "local");
                assert_eq!(error, None);
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "local title did not finish"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(request_pending_titles(service.sender.as_ref().unwrap(), 10)
            .unwrap()
            .is_empty());
        drop(connection);
        drop(service);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_summary_configuration_can_retry_unchanged_settings() {
        let mut service = ProjectService::in_memory(crate::api::EventHub::default());
        let sender = service.sender.take();
        let summary = crate::config::SummaryConfig::default();
        assert!(service.configure_summaries(&summary).is_err());
        assert!(service.summaries_need_retry());
        service.sender = sender;
        service.configure_summaries(&summary).unwrap();
        assert!(!service.summaries_need_retry());
        assert!(service.is_available());
    }

    #[test]
    fn summary_reconfigure_requeues_claims_and_rejects_all_late_writes() {
        let mut service = ProjectService::in_memory(crate::api::EventHub::default());
        service
            .configure_summaries(&crate::config::SummaryConfig::default())
            .unwrap();
        let mut value = candidate("preserved");
        value.runtime = Some(RuntimeMapping {
            workspace_id: "w1".into(),
            pane_id: "p1".into(),
            generation: 7,
        });
        let key = value.identity.stable_key.clone();
        service.upsert_candidate(value).unwrap();
        let sender = service.sender.as_ref().unwrap().clone();
        let old = Arc::clone(&service.summary_cancelled);
        request_claim_titles(&sender, &old, vec![key.clone()]).unwrap();
        let before = service.snapshot();
        service
            .configure_summaries(&crate::config::SummaryConfig::default())
            .unwrap();
        let after = service.snapshot();
        assert!(after.revision > before.revision);
        assert_eq!(after.projects, before.projects);
        assert_eq!(after.topics, before.topics);
        assert!(request_pending_titles(&sender, 10)
            .unwrap()
            .iter()
            .any(|row| row.stable_key == key && row.status == "pending"));
        let update = SessionTitleUpdate {
            stable_key: key.clone(),
            title: "must not apply".into(),
            source: "model".into(),
            status: "done".into(),
            error: None,
            backend: Some("test".into()),
            model: None,
            fingerprint: "old-settings".into(),
            generated_at: 10,
        };
        let results = [
            request_claim_titles(&sender, &old, vec![key.clone()]),
            request_apply_titles(&sender, &old, vec![update]),
            request_apply_semantic(&sender, &old, vec![], 10),
            request_apply_topic_merges(&sender, &old, vec![], 10),
        ];
        for result in results {
            assert_eq!(result.unwrap_err().code, "summary_cancelled");
        }
        assert_eq!(
            request_begin_topic_merge(&sender, &old, 10)
                .unwrap_err()
                .code,
            "summary_cancelled"
        );
        assert_eq!(service.snapshot(), after);
        request_claim_titles(&sender, &service.summary_cancelled, vec![key]).unwrap();
        assert!(service.snapshot().revision > after.revision);
    }

    #[test]
    fn worker_serializes_mutations_and_publishes_monotonic_revisions() {
        let hub = crate::api::EventHub::default();
        let service = ProjectService::in_memory(hub.clone());
        let first = service.upsert_candidate(candidate("one")).unwrap();
        let second = service.upsert_candidate(candidate("two")).unwrap();
        assert!(second > first);
        assert_eq!(service.snapshot().revision, second);
        let events = hub.events_after(0);
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events.last().map(|(_, event)| &event.data),
            Some(crate::api::schema::EventData::ProjectSnapshotUpdated { revision, .. })
                if *revision == second
        ));
    }

    #[test]
    fn disabled_service_is_nonfatal_and_reports_unavailable_mutations() {
        let service = ProjectService::disabled();
        assert!(service.snapshot().projects.is_empty());
        let error = service.upsert_candidate(candidate("one")).unwrap_err();
        assert_eq!(error.code, "catalog_unavailable");
    }

    #[test]
    fn background_scan_returns_immediately_and_commits_through_writer_queue() {
        let root_path = temp_dir("background");
        std::fs::write(
            root_path.join("rollout-background.jsonl"),
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"background\",\"cwd\":\"/tmp\"}}\n",
        )
        .expect("background fixture");
        let canonical = std::fs::canonicalize(&root_path).expect("canonical fixture root");
        let root = super::super::adapters::AdapterRoot {
            adapter: "codex",
            root_key: canonical.to_string_lossy().into_owned(),
            path: canonical,
            origin: super::super::adapters::AdapterRootOrigin::Explicit,
            preflight: None,
        };
        let hub = crate::api::EventHub::default();
        let mut service = ProjectService::in_memory(hub.clone());
        let started = std::time::Instant::now();
        service.start_background_scan(&[root]);
        assert!(started.elapsed() < std::time::Duration::from_millis(200));

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            let snapshot = service.snapshot();
            if snapshot
                .scan_status
                .iter()
                .any(|status| status.adapter == "codex" && status.state == "ready")
            {
                assert_eq!(snapshot.projects[0].sessions.len(), 1);
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "background scan timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(hub.events_after(0).len(), 1);
        let _ = std::fs::remove_dir_all(root_path);
    }

    #[test]
    fn opening_service_clears_runtime_mappings_left_by_previous_process() {
        let root = temp_dir("stale-runtime");
        let path = root.join("catalog.sqlite3");
        let mut catalog = ProjectCatalog::open(&path).expect("seed catalog");
        let mut item = candidate("stale-runtime");
        item.runtime = Some(RuntimeMapping {
            workspace_id: "old-workspace".to_string(),
            pane_id: "old-pane".to_string(),
            generation: 99,
        });
        catalog
            .upsert_candidate(&item)
            .expect("seed runtime mapping");
        assert!(catalog.snapshot(50).expect("seed snapshot").projects[0].sessions[0].live);
        drop(catalog);

        let service = ProjectService::open(&path, crate::api::EventHub::default());
        let snapshot = service.snapshot();
        assert!(!snapshot.projects[0].sessions[0].live);
        let _ = std::fs::remove_dir_all(root);
    }
}
