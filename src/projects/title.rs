//! Generates a readable title for one agent session.
//!
//! The heuristic this replaces picked "the longest of the first four user turns", which on this
//! machine chose a *pasted* copy of a previous agent's answer (167 characters) over the user's
//! actual request (142 characters). Length cannot express which sentence is the subject.
//!
//! The pipeline is: cleaned semantic envelope → small non-reasoning model → strict validation →
//! persisted title. A deterministic fallback covers a missing or failing model, so a session
//! always has something readable to show.
//!
//! This module owns *no* rendering. It produces text; the sidebar decides how to clip it.
//!
//! The backend call and scheduler are part of the same pipeline: title generation runs in the
//! ProjectService worker, persists `generated_title`, and publishes a snapshot revision so every
//! client surface sees the same value. Model failures remain readable through the deterministic
//! fallback and are retryable on a later run.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

use serde_json::Value;

use super::domain::{title_input_fingerprint, PendingTitleSession, SessionTitleUpdate};
use super::semantic::{BatchError, SemanticConfig};
use super::service::{self, ProjectCommand};
use super::transcript::{Transcript, TranscriptRole};

/// Widest a stored title may be, in characters.
const TITLE_MAX_CHARS: usize = 72;

/// Most intents sent to the model for one session.
const MAX_INTENTS: usize = 5;

/// Character budget for one session's envelope, so a batch prompt stays bounded.
const MAX_ENVELOPE_CHARS: usize = 2_500;

/// Longest agent outcome quoted in an envelope.
const MAX_OUTCOME_CHARS: usize = 300;

/// Subjects that identify nothing. A title built on these is no better than no title.
const BANNED_SUBJECTS: [&str; 8] = [
    "workspace",
    "任务",
    "会话",
    "项目",
    "agent",
    "session",
    "project",
    "task",
];

/// Claims of completion the model must not invent.
const COMPLETION_CLAIMS: [&str; 4] = ["已完成", "已修复", "已发布", "已上线"];

/// Openers that carry no subject of their own.
const LOW_SIGNAL_OPENERS: [&str; 12] = [
    "继续",
    "接着",
    "在吗",
    "好的",
    "收到",
    "看下",
    "看看",
    "处理一下",
    "hi",
    "hello",
    "hey",
    "test",
];

/// Continuation prompts are meaningful to the runtime but do not identify the work being resumed.
const LOW_SIGNAL_CONTINUATIONS: [&str; 6] = [
    "continue from where you left off",
    "continue where you left off",
    "continue from the previous session",
    "继续上次",
    "继续之前",
    "恢复之前",
];

/// One session's cleaned input, as sent to the title model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TitleEnvelope {
    /// Batch-local index. The model addresses sessions by this, never by stable key, so a
    /// hallucinated identifier cannot land on an unrelated session.
    pub index: usize,
    pub backend: String,
    /// Provider title retained as weak evidence (never trusted as the semantic answer).
    pub native_title: Option<String>,
    /// Directory or repository basename, the most reliable subject evidence.
    pub folder: Option<String>,
    /// High-signal user intents, in the order they were said.
    pub intents: Vec<String>,
    /// The agent's most recent conclusion, for subject evidence only.
    pub outcome: Option<String>,
}

/// Builds the envelope for one session.
///
/// `intents` and `outcome` come from the transcript reader, which has already folded agent turns,
/// dropped tool plumbing and stripped harness wrappers.
pub(crate) fn build_envelope(
    index: usize,
    backend: &str,
    folder: Option<&str>,
    user_intents: &[String],
    latest_outcome: Option<&str>,
) -> TitleEnvelope {
    let mut budget = MAX_ENVELOPE_CHARS;
    let intents = select_intents(user_intents)
        .into_iter()
        .take_while(|intent| {
            let cost = intent.chars().count();
            if cost > budget {
                return false;
            }
            budget -= cost;
            true
        })
        .collect();

    TitleEnvelope {
        index,
        backend: backend.to_string(),
        native_title: None,
        folder: folder
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        intents,
        outcome: latest_outcome
            .map(|value| clip(value, MAX_OUTCOME_CHARS))
            .filter(|value| !value.is_empty()),
    }
}

/// Picks the intents most likely to name the session's subject.
///
/// Ranked by information score, then restored to the order they were said: a title reads better
/// when its evidence is chronological, but the *selection* must not be positional — the pasted
/// reply that broke the old heuristic was the very first turn.
fn select_intents(intents: &[String]) -> Vec<String> {
    let has_non_artifact = intents
        .iter()
        .any(|intent| !is_low_signal(intent) && !is_likely_pasted_answer(intent));
    let mut scored: Vec<(usize, i32, &String)> = intents
        .iter()
        .enumerate()
        .filter(|(_, intent)| {
            !is_low_signal(intent) && (!has_non_artifact || !is_likely_pasted_answer(intent))
        })
        .map(|(position, intent)| (position, information_score(intent), intent))
        .collect();

    scored.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    scored.truncate(MAX_INTENTS);
    scored.sort_by_key(|(position, _, _)| *position);
    scored
        .into_iter()
        .map(|(_, _, intent)| intent.clone())
        .collect()
}

/// Detects a pasted answer or recommendation block masquerading as a user intent.
fn is_likely_pasted_answer(intent: &str) -> bool {
    let lowered = intent.to_lowercase();
    let markers = [
        "推荐顺序",
        "bundle id",
        "最合适",
        "图标是",
        "dock",
        "方案如下",
        "总结：",
        "recommendation",
    ];
    markers
        .iter()
        .filter(|marker| lowered.contains(**marker))
        .count()
        >= 2
}

/// Whether an intent says too little to name a subject.
pub(crate) fn is_low_signal(intent: &str) -> bool {
    let trimmed = intent.trim();
    let meaningful = trimmed.chars().filter(|c| c.is_alphanumeric()).count();
    if meaningful < 3 && !trimmed.chars().any(is_cjk) {
        return true;
    }
    let lowered = trimmed.to_lowercase();
    // Only a *whole* turn of filler is low signal. "继续修复 ork3 的侧栏" carries a subject and
    // must survive, so this compares the entire trimmed turn rather than its prefix.
    LOW_SIGNAL_OPENERS.iter().any(|opener| {
        lowered == *opener || lowered.trim_end_matches(['。', '，', '?', '？']) == *opener
    }) || LOW_SIGNAL_CONTINUATIONS
        .iter()
        .any(|phrase| lowered.trim_end_matches(['。', '.', '!', '?', '！', '？']) == *phrase)
        || trimmed.chars().count() < 3
}

fn is_cjk(character: char) -> bool {
    matches!(character, '\u{4e00}'..='\u{9fff}')
}

/// Scores how likely an intent is to name a concrete subject.
///
/// The signals are the ones a coding session is searched by later: repository and module names,
/// paths, error codes, identifiers. Generic verbs score nothing.
fn information_score(intent: &str) -> i32 {
    let mut score = 0i32;
    for token in identifier_tokens(intent) {
        // A path or a filename is the strongest subject evidence.
        if token.contains('/') || token.contains('\\') {
            score += 3;
        }
        // `kebab-case`, `snake_case` and `CamelCase` identifiers name real things.
        if token.contains('-') || token.contains('_') {
            score += 2;
        }
        if token.chars().next().is_some_and(char::is_uppercase)
            && token.chars().skip(1).any(char::is_uppercase)
        {
            score += 2;
        }
        // An HTTP status or an error number.
        if token.chars().all(|c| c.is_ascii_digit()) && token.len() == 3 {
            score += 1;
        }
        // Any remaining latin run of real length is a product, module or API name. This is what
        // makes `ork3` / `projects` / `cluster` inside CJK prose count at all.
        if token.chars().count() >= 3 {
            score += 1;
        }
    }
    // Longer turns tend to carry more evidence, but only up to a point: this must not become the
    // "pick the longest" rule it replaces.
    score += i32::try_from(intent.chars().count() / 60)
        .unwrap_or(0)
        .min(1);
    score
}

/// Splits out latin identifier runs, ignoring CJK.
///
/// CJK prose has no spaces, so `split_whitespace` returned "看下ork3这个项目，他的projects…" as a
/// single token and scored nothing, while a pasted English reply scored on its spacing alone. This
/// finds the identifiers *inside* the prose instead.
fn identifier_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for character in value.chars() {
        // Dots split a token: `com.google.Chrome.canary` is a bundle id, not four subjects, and
        // scoring it once keeps a pasted paragraph from outranking a real request.
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '/' | '\\') {
            current.push(character);
            continue;
        }
        if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens.retain(|token| token.chars().any(|c| c.is_ascii_alphanumeric()));
    tokens
}

/// Why a candidate title was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TitleRejection {
    Empty,
    /// Not `【subject】task`.
    MalformedShape,
    /// Subject names nothing, e.g. `【项目】`.
    BannedSubject(String),
    /// Claims work finished without evidence.
    UnsupportedCompletionClaim(String),
    /// Model machinery leaked into the answer.
    ModelArtifact,
}

impl TitleRejection {
    pub(crate) fn reason(&self) -> String {
        match self {
            Self::Empty => "title was empty after cleaning".to_string(),
            Self::MalformedShape => "title is not 【subject】task".to_string(),
            Self::BannedSubject(subject) => format!("subject `{subject}` identifies nothing"),
            Self::UnsupportedCompletionClaim(word) => {
                format!("title claims `{word}` without evidence")
            }
            Self::ModelArtifact => "title contains model machinery".to_string(),
        }
    }
}

/// Strips model formatting from a candidate title.
///
/// Small models wrap answers in fences, quote them, or leak a reasoning block even when told not
/// to. Cleaning happens before shape validation so a otherwise-good title is not rejected for its
/// packaging.
pub(crate) fn clean_candidate(raw: &str) -> String {
    let mut value = raw.to_string();

    // A reasoning block may be unclosed when the model runs out of budget mid-thought, so an
    // unterminated `<think>` discards everything after it rather than leaking it as the title.
    if let Some(start) = value.find("<think>") {
        match value[start..].find("</think>") {
            Some(end) => {
                let after = start + end + "</think>".len();
                value = format!("{}{}", &value[..start], &value[after..]);
            }
            None => value.truncate(start),
        }
    }

    let cleaned: String = value
        .lines()
        .filter(|line| !line.trim_start().starts_with("```"))
        .collect::<Vec<_>>()
        .join(" ");

    let collapsed = strip_ansi(&cleaned)
        .chars()
        // Remaining C0/C1 control characters.
        .filter(|c| !c.is_control() && !matches!(*c, '\u{80}'..='\u{9f}'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    collapsed
        .trim()
        .trim_matches(|c: char| c == '"' || c == '\'' || c == '`' || c == '“' || c == '”')
        .trim()
        .to_string()
}

/// Validates a cleaned title against the stored contract.
pub(crate) fn validate(cleaned: &str) -> Result<String, TitleRejection> {
    if cleaned.is_empty() {
        return Err(TitleRejection::Empty);
    }
    if cleaned.contains("<think>") || cleaned.contains("tool_use") {
        return Err(TitleRejection::ModelArtifact);
    }

    let rest = cleaned
        .strip_prefix('【')
        .ok_or(TitleRejection::MalformedShape)?;
    let (subject, task) = rest
        .split_once('】')
        .ok_or(TitleRejection::MalformedShape)?;

    let subject_width = subject.chars().count();
    let task = task.trim();
    if !(2..=24).contains(&subject_width) || task.chars().count() < 6 {
        return Err(TitleRejection::MalformedShape);
    }

    let lowered = subject.to_lowercase();
    if BANNED_SUBJECTS
        .iter()
        .any(|banned| lowered == *banned || lowered.trim() == *banned)
    {
        return Err(TitleRejection::BannedSubject(subject.to_string()));
    }
    if let Some(claim) = COMPLETION_CLAIMS.iter().find(|claim| task.contains(*claim)) {
        return Err(TitleRejection::UnsupportedCompletionClaim(
            (*claim).to_string(),
        ));
    }

    Ok(clip(&format!("【{subject}】{task}"), TITLE_MAX_CHARS))
}

/// Builds a title without a model.
///
/// Used when every backend fails or the generator is disabled. It must never look broken: the
/// owner chose not to mark rule-built titles in the UI, so this has to read like a real title.
pub(crate) fn fallback_title(envelope: &TitleEnvelope) -> String {
    let subject = envelope
        .folder
        .as_deref()
        .map(str::trim)
        .filter(|value| {
            !value.is_empty() && !BANNED_SUBJECTS.contains(&value.to_lowercase().as_str())
        })
        .map(|value| clip(value, 24))
        .unwrap_or_else(|| title_case(&envelope.backend));

    // Prefer the highest-information intent when no model is available to judge. This is the
    // deterministic fallback promised by the title spec: entity names, paths and identifiers
    // beat a generic continuation request. Ties retain transcript order, so a pasted answer
    // cannot win merely because it is longer or appears later.
    let task = envelope
        .intents
        .iter()
        .enumerate()
        .filter_map(|(position, intent)| {
            let clause = first_clause(intent);
            (clause.chars().count() >= 6).then_some((
                information_score(intent)
                    - if is_likely_pasted_answer(intent) { 8 } else { 0 },
                position,
                clause,
            ))
        })
        .max_by(|left, right| left.0.cmp(&right.0).then_with(|| right.1.cmp(&left.1)))
        .map(|(_, _, clause)| clause)
        .unwrap_or_else(|| "等待补充具体任务信息".to_string());

    clip(&format!("【{subject}】{task}"), TITLE_MAX_CHARS)
}

/// Builds the title prompt for a bounded batch of cleaned envelopes.
pub(crate) fn build_prompt(batch: &[TitleEnvelope]) -> String {
    let mut prompt = String::from(
        "给下面每个编码会话起一个标题。\n格式必须是【具体对象】具体任务，对象 2-24 字，任务 6-42 字。\n\
         对象使用产品名、仓库名、模块名、文件名或错误码，禁止使用 Workspace、任务、会话、项目、Agent。\n\
         不要默认使用第一条请求或 native_title；优先选择能概括整体工作的高信息量请求，并综合多条请求。忽略复制的回答、推荐清单、执行日志等非任务内容。\n\
         没有明确证据不要写“已完成”“已修复”“已发布”“已上线”。\n\
         只输出 JSON：{\"items\":[{\"id\":1,\"title\":\"【对象】具体任务\"}]}，不要解释。\n\n",
    );
    for item in batch {
        prompt.push_str(&format!(
            "{}. backend={} folder={} native_title={:?} intents={:?} outcome={:?}\n",
            item.index,
            item.backend,
            item.folder.as_deref().unwrap_or("unknown"),
            item.native_title,
            item.intents,
            item.outcome
        ));
    }
    prompt
}

/// Parses and validates one model reply. Partial or duplicate replies are rejected as a batch.
pub(crate) fn parse_response(
    response: &str,
    batch: &[TitleEnvelope],
) -> Result<Vec<(usize, String)>, BatchError> {
    let start = response
        .find('{')
        .ok_or_else(|| BatchError::Failed("title reply has no JSON object".to_string()))?;
    let end = response
        .rfind('}')
        .ok_or_else(|| BatchError::Failed("title reply has no JSON closing brace".to_string()))?;
    let parsed: Value = serde_json::from_str(&response[start..=end])
        .map_err(|error| BatchError::Failed(format!("invalid title JSON: {error}")))?;
    let items = parsed
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| BatchError::Failed("title reply missing items".to_string()))?;
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::with_capacity(items.len());
    for item in items {
        let id = item
            .get("id")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .filter(|id| batch.iter().any(|entry| entry.index == *id))
            .ok_or_else(|| BatchError::Failed("title reply contains unknown id".to_string()))?;
        if !seen.insert(id) {
            return Err(BatchError::Failed(
                "title reply contains duplicate id".to_string(),
            ));
        }
        let raw = item
            .get("title")
            .and_then(Value::as_str)
            .ok_or_else(|| BatchError::Failed("title item is missing title".to_string()))?;
        let title =
            validate(&clean_candidate(raw)).map_err(|error| BatchError::Failed(error.reason()))?;
        result.push((id, title));
    }
    if result.len() != batch.len() {
        return Err(BatchError::Failed(
            "title reply does not cover the batch".to_string(),
        ));
    }
    Ok(result)
}

fn envelope_for_session(index: usize, session: &PendingTitleSession) -> TitleEnvelope {
    let transcript = super::transcript::read_full_transcript(
        &session.backend,
        session.transcript_ref.as_deref(),
    )
    .ok();
    let (intents, outcome) = transcript
        .as_ref()
        .map(extract_transcript_evidence)
        .unwrap_or_else(|| {
            if session.native_title.trim().is_empty() {
                (Vec::new(), None)
            } else {
                (vec![session.native_title.clone()], None)
            }
        });
    let folder = session
        .cwd
        .as_deref()
        .and_then(|cwd| Path::new(cwd).file_name())
        .and_then(|name| name.to_str());
    let mut envelope = build_envelope(
        index,
        &session.backend,
        folder,
        &intents,
        outcome.as_deref(),
    );
    envelope.native_title = Some(session.native_title.clone());
    envelope
}

fn extract_transcript_evidence(transcript: &Transcript) -> (Vec<String>, Option<String>) {
    let intents = transcript
        .messages
        .iter()
        .filter(|message| message.role == TranscriptRole::User)
        .map(|message| message.text.clone())
        .collect::<Vec<_>>();
    let outcome = transcript
        .messages
        .iter()
        .rev()
        .find(|message| message.role == TranscriptRole::Assistant)
        .map(|message| message.text.clone());
    (intents, outcome)
}

fn title_backends(config: &SemanticConfig) -> Vec<super::semantic::BackendSpec> {
    let mut result = config
        .backends
        .iter()
        .filter(|backend| backend.name == "opencode" || backend.name == "pi")
        .cloned()
        .collect::<Vec<_>>();
    if !result.iter().any(|backend| backend.name == "hermes") {
        result.push(super::semantic::BackendSpec {
            name: "hermes".to_string(),
            models: Vec::new(),
        });
    }
    result
}

/// Generates titles in the background without blocking Catalog reads or the TUI input path.
pub(crate) fn run_title_generation_worker(
    sender: &Sender<ProjectCommand>,
    config: &SemanticConfig,
    shutdown: &AtomicBool,
) {
    while !shutdown.load(Ordering::Acquire) {
        let pending = match service::request_pending_titles(sender, config.max_sessions_per_run) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(
                    category = "title_pending",
                    "Could not read pending titles: {error:?}"
                );
                break;
            }
        };
        if pending.is_empty() {
            // Keep polling for sessions discovered by later adapter refreshes. This worker used
            // to exit after two idle rounds, so titles for sessions created while Herdr stayed
            // open were never generated until the next restart.
            tracing::debug!(category = "title_idle", "No sessions awaiting title generation");
            std::thread::sleep(config.idle_backfill);
            continue;
        }
        tracing::info!(
            category = "title_start",
            "Generating titles for {} sessions",
            pending.len()
        );
        let mut updates = Vec::new();
        let mut backend_succeeded = false;
        for chunk in pending.chunks(config.batch_size.max(1)) {
            if shutdown.load(Ordering::Acquire) {
                return;
            }
            let keys = chunk
                .iter()
                .map(|session| session.stable_key.clone())
                .collect();
            if let Err(error) = service::request_claim_titles(sender, keys) {
                tracing::warn!(
                    category = "title_claim",
                    "Could not claim title batch: {error:?}"
                );
                return;
            }
            let envelopes = chunk
                .iter()
                .enumerate()
                .map(|(offset, session)| envelope_for_session(offset + 1, session))
                .collect::<Vec<_>>();
            let fingerprints = chunk
                .iter()
                .zip(envelopes.iter())
                .map(|(session, envelope)| {
                    (
                        session.stable_key.clone(),
                        title_input_fingerprint(
                            &envelope.backend,
                            envelope.folder.as_deref(),
                            &envelope.intents,
                            envelope.outcome.as_deref(),
                        ),
                    )
                })
                .collect::<Vec<_>>();
            let generated = title_backends(config).iter().find_map(|backend| {
                let models = if backend.models.is_empty() {
                    vec![None]
                } else {
                    backend
                        .models
                        .iter()
                        .map(|model| Some(model.as_str()))
                        .collect()
                };
                models.into_iter().find_map(|model| {
                    let prompt = build_prompt(&envelopes);
                    let output =
                        super::semantic::run_backend(&backend.name, model, &prompt, config.timeout)
                            .ok()?;
                    parse_response(&output, &envelopes)
                        .ok()
                        .map(|items| (backend.name.clone(), model.map(str::to_string), items))
                })
            });
            if let Some((backend, model, items)) = generated {
                backend_succeeded = true;
                let titles = items
                    .into_iter()
                    .collect::<std::collections::HashMap<_, _>>();
                for (offset, (session, (_, fingerprint))) in
                    chunk.iter().zip(fingerprints.iter()).enumerate()
                {
                    if let Some(title) = titles.get(&(offset + 1)) {
                        updates.push(SessionTitleUpdate {
                            stable_key: session.stable_key.clone(),
                            title: title.clone(),
                            source: "model".to_string(),
                            status: "done".to_string(),
                            error: None,
                            backend: Some(backend.clone()),
                            model: model.clone(),
                            fingerprint: fingerprint.clone(),
                            generated_at: super::runtime::unix_time_ms(),
                        });
                    }
                }
            } else {
                for (offset, (session, (_, fingerprint))) in
                    chunk.iter().zip(fingerprints.iter()).enumerate()
                {
                    let envelope = &envelopes[offset];
                    updates.push(SessionTitleUpdate {
                        stable_key: session.stable_key.clone(),
                        title: fallback_title(envelope),
                        source: "heuristic".to_string(),
                        status: "failed".to_string(),
                        error: Some("all title backends failed".to_string()),
                        backend: None,
                        model: None,
                        fingerprint: fingerprint.clone(),
                        generated_at: super::runtime::unix_time_ms(),
                    });
                }
            }
        }
        if !updates.is_empty() {
            let update_count = updates.len();
            if let Err(error) = service::request_apply_titles(sender, updates) {
                tracing::warn!(
                    category = "title_apply",
                    "Could not persist generated titles: {error:?}"
                );
            }
            tracing::info!(
                category = "title_done",
                "Persisted {} session title results",
                update_count
            );
        }
        if !backend_succeeded {
            // Keep failed rows retryable, but do not spin on them while every backend is down.
            std::thread::sleep(config.idle_backfill);
        }
    }
}

/// Removes whole ANSI escape sequences.
///
/// Dropping only the ESC byte leaves `[31m` behind as ordinary text, which then becomes part of
/// the title.
fn strip_ansi(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(character) = chars.next() {
        if character != '\u{1b}' {
            out.push(character);
            continue;
        }
        // CSI sequences end at their final byte in `@`..`~`; anything else is a short escape.
        if chars.peek() == Some(&'[') {
            chars.next();
            for inner in chars.by_ref() {
                if ('\u{40}'..='\u{7e}').contains(&inner) {
                    break;
                }
            }
        } else {
            chars.next();
        }
    }
    out
}

fn first_clause(value: &str) -> String {
    let end = value
        .find(['。', '！', '？', '\n', '!', '?'])
        .unwrap_or(value.len());
    clip(value[..end].trim(), 42)
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + chars.as_str()
    })
}

fn clip(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.trim().to_string();
    }
    value
        .chars()
        .take(max_chars)
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intents(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    /// The failure that triggered this module: a pasted copy of a previous agent's answer was
    /// longer than the real request, so the old "pick the longest" rule chose it.
    ///
    /// Scoring cannot reliably tell a pasted *answer* from a request — both name real products —
    /// and one occurrence on this machine is too thin a sample to build a heuristic on. What the
    /// envelope must guarantee is that the real request is *present and ranked*, so the model
    /// gets to make that judgement instead of the request being dropped before it is ever seen.
    #[test]
    fn a_pasted_reply_never_displaces_the_real_request_from_the_envelope() {
        let pasted = "推荐顺序： 1. Chrome Canary（最合适） bundle ID 是 com.google.Chrome.canary，图标是黄色，Dock 会变成两个 App";
        let request = "看下ork3这个项目，他的projects 抓取和cluster聚类 聚类出来的内容有哪些问题";
        let envelope = build_envelope(
            0,
            "claude",
            Some("ork3"),
            &intents(&[pasted, request]),
            None,
        );

        assert!(
            envelope
                .intents
                .iter()
                .any(|intent| intent.contains("ork3")),
            "the real request must reach the model: {:?}",
            envelope.intents
        );
        // And the folder gives the model an unambiguous subject even if it misreads the intents.
        assert_eq!(envelope.folder.as_deref(), Some("ork3"));
    }

    /// Whatever the model decides, the deterministic fallback must not repeat the old mistake of
    /// letting a pasted answer become the title.
    #[test]
    fn the_fallback_prefers_the_folder_over_a_pasted_reply() {
        let pasted = "推荐顺序： 1. Chrome Canary（最合适） bundle ID 是 com.google.Chrome.canary";
        let request = "看下ork3这个项目 projects 抓取和 cluster 聚类有哪些问题";
        let envelope = build_envelope(
            0,
            "claude",
            Some("ork3"),
            &intents(&[pasted, request]),
            None,
        );
        let title = fallback_title(&envelope);
        assert!(title.starts_with("【ork3】"), "{title}");
        assert!(!title.contains("Chrome Canary"), "{title}");
    }

    #[test]
    fn filler_turns_are_excluded_but_filler_plus_subject_survives() {
        let envelope = build_envelope(
            0,
            "codex",
            None,
            &intents(&["继续", "在吗", "继续修复 ork3 的侧栏高亮"]),
            None,
        );
        assert_eq!(envelope.intents.len(), 1);
        assert!(envelope.intents[0].contains("侧栏高亮"));
    }

    #[test]
    fn intents_are_returned_in_the_order_they_were_said() {
        let envelope = build_envelope(
            0,
            "codex",
            None,
            &intents(&[
                "先看 src/ui/projects.rs 的渲染",
                "再查 catalog.sqlite3 的迁移",
            ]),
            None,
        );
        assert!(envelope.intents[0].contains("projects.rs"));
        assert!(envelope.intents[1].contains("catalog.sqlite3"));
    }

    #[test]
    fn at_most_five_intents_are_sent() {
        let many = (0..12)
            .map(|index| format!("修复 module-{index}/handler.rs 的空指针"))
            .collect::<Vec<_>>();
        let envelope = build_envelope(0, "codex", None, &many, None);
        assert_eq!(envelope.intents.len(), MAX_INTENTS);
    }

    #[test]
    fn a_valid_title_round_trips() {
        let title = validate(&clean_candidate("【ork3】修复侧栏高亮渲染")).expect("valid");
        assert_eq!(title, "【ork3】修复侧栏高亮渲染");
    }

    #[test]
    fn a_banned_subject_is_rejected() {
        let error = validate(&clean_candidate("【项目】修复侧栏高亮")).expect_err("banned");
        assert!(matches!(error, TitleRejection::BannedSubject(_)));
    }

    #[test]
    fn a_title_without_the_entity_prefix_is_rejected() {
        let error = validate(&clean_candidate("修复 ork3 的侧栏高亮")).expect_err("shape");
        assert_eq!(error, TitleRejection::MalformedShape);
    }

    #[test]
    fn an_unsupported_completion_claim_is_rejected() {
        let error = validate(&clean_candidate("【ork3】侧栏高亮已修复")).expect_err("completion");
        assert!(matches!(
            error,
            TitleRejection::UnsupportedCompletionClaim(_)
        ));
    }

    /// Small models leak fences, quotes and reasoning blocks even when told not to.
    #[test]
    fn model_packaging_is_cleaned_before_validation() {
        let raw = "```\n\"【ork3】修复侧栏高亮渲染\"\n```";
        assert_eq!(
            validate(&clean_candidate(raw)).expect("valid"),
            "【ork3】修复侧栏高亮渲染"
        );
    }

    #[test]
    fn a_reasoning_block_is_removed() {
        let raw = "<think>让我想想这个会话在做什么</think>【ork3】修复侧栏高亮";
        assert_eq!(
            validate(&clean_candidate(raw)).expect("valid"),
            "【ork3】修复侧栏高亮"
        );
    }

    /// A model that runs out of budget mid-thought leaves `<think>` unclosed; the fragment must
    /// not become the title.
    #[test]
    fn an_unclosed_reasoning_block_does_not_leak() {
        let error = validate(&clean_candidate("<think>这个会话看起来是在")).expect_err("empty");
        assert_eq!(error, TitleRejection::Empty);
    }

    #[test]
    fn control_characters_are_stripped() {
        let raw = "\u{1b}[31m【ork3】修复侧栏高亮\u{1b}[0m";
        let cleaned = clean_candidate(raw);
        assert!(!cleaned.contains('\u{1b}'), "{cleaned:?}");
        assert!(validate(&cleaned).is_ok(), "{cleaned:?}");
    }

    #[test]
    fn a_title_is_clipped_to_the_stored_limit() {
        let long = format!("【ork3】{}", "修复".repeat(60));
        let title = validate(&clean_candidate(&long)).expect("valid");
        assert!(title.chars().count() <= TITLE_MAX_CHARS);
    }

    /// The owner chose not to mark rule-built titles in the UI, so a fallback has to read like a
    /// real title rather than a placeholder.
    #[test]
    fn the_fallback_names_a_subject_and_a_task() {
        let envelope = build_envelope(
            0,
            "claude",
            Some("ork3"),
            &intents(&["看下ork3这个项目 projects 抓取和 cluster 聚类有哪些问题"]),
            None,
        );
        let title = fallback_title(&envelope);
        assert!(title.starts_with("【ork3】"), "{title}");
        assert!(title.chars().count() <= TITLE_MAX_CHARS);
        assert!(
            validate(&title).is_ok(),
            "the fallback must be valid: {title}"
        );
    }

    /// With no evidence at all the fallback still has to be readable and must not invent a state.
    #[test]
    fn the_fallback_survives_an_empty_envelope() {
        let envelope = build_envelope(0, "codex", None, &[], None);
        let title = fallback_title(&envelope);
        assert!(title.starts_with("【Codex】"), "{title}");
        assert!(!title.contains("已完成"));
    }

    /// A folder named `Workspace` identifies nothing; the backend is a better subject.
    #[test]
    fn a_banned_folder_is_not_used_as_the_fallback_subject() {
        let envelope = build_envelope(0, "codex", Some("Workspace"), &[], None);
        assert!(fallback_title(&envelope).starts_with("【Codex】"));
    }

    #[test]
    fn model_response_requires_every_input_and_validates_titles() {
        let batch = vec![build_envelope(
            1,
            "claude",
            Some("ork3"),
            &intents(&["修复 projects 页面标题"]),
            None,
        )];
        let parsed = parse_response(
            r#"{"items":[{"id":1,"title":"【ork3】修复 Projects 页面标题"}]}"#,
            &batch,
        )
        .expect("valid title response");
        assert_eq!(parsed[0].1, "【ork3】修复 Projects 页面标题");
        assert!(parse_response(r#"{"items":[]}"#, &batch).is_err());
    }
}
