//! Bounded, on-demand reading of the most recent local conversation evidence.
//! No model calls, PTYs, Catalog writes, or rendering happen in this worker.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::transcript::{Transcript, TranscriptRole};
use super::{IndexedSessionSummary, ProjectSummary};

const RECENT_SESSIONS: usize = 4;
const CACHE_CAPACITY: usize = 128;
const CACHE_TTL: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActivityOrigin {
    UserRequest,
    AgentResponse,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionActivity {
    pub session_key: String,
    pub latest_update: Option<String>,
    pub origin: Option<ActivityOrigin>,
    pub latest_request: Option<String>,
    pub latest_response: Option<String>,
    /// Explicit next steps quoted from the latest recorded assistant response.
    pub next_steps: Vec<String>,
    pub read_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ActivityBatch {
    pub sessions: Vec<SessionActivity>,
    pub loading: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Fingerprint {
    backend: String,
    path: String,
    activity_at: i64,
}

struct CachedActivity {
    fingerprint: Fingerprint,
    request_id: u64,
    requested_at: Instant,
    pending: bool,
    value: SessionActivity,
}

struct ReadRequest {
    session_key: String,
    fingerprint: Fingerprint,
    request_id: u64,
}

#[derive(Default)]
pub(crate) struct ActivityReader {
    sender: Mutex<Option<mpsc::SyncSender<ReadRequest>>>,
    cache: Arc<Mutex<HashMap<String, CachedActivity>>>,
    shutdown: Arc<AtomicBool>,
    next_request: AtomicU64,
}

impl ActivityReader {
    pub(crate) fn for_project(&self, project: &ProjectSummary, refresh: bool) -> ActivityBatch {
        let mut sessions: Vec<_> = project.sessions.iter().collect();
        sessions.sort_by(|a, b| {
            b.last_activity_at
                .cmp(&a.last_activity_at)
                .then_with(|| a.stable_key.cmp(&b.stable_key))
        });
        let mut batch = ActivityBatch::default();
        for session in sessions.into_iter().take(RECENT_SESSIONS) {
            let (value, pending) = self.for_session(session, refresh);
            batch.sessions.push(value);
            batch.loading |= pending;
        }
        batch
    }

    fn for_session(
        &self,
        session: &IndexedSessionSummary,
        refresh: bool,
    ) -> (SessionActivity, bool) {
        let empty = SessionActivity {
            session_key: session.stable_key.clone(),
            ..Default::default()
        };
        let Some(path) = session
            .transcript_ref
            .as_ref()
            .filter(|path| !path.is_empty())
        else {
            return (empty, false);
        };
        if !matches!(session.backend.as_str(), "codex" | "claude" | "pi" | "grok") {
            return (empty, false);
        }
        let fingerprint = Fingerprint {
            backend: session.backend.clone(),
            path: path.clone(),
            activity_at: session.last_activity_at,
        };
        let Ok(mut cache) = self.cache.lock() else {
            return (empty, false);
        };
        if let Some(entry) = cache.get(&session.stable_key) {
            if entry.fingerprint == fingerprint
                && (entry.pending || (!refresh && entry.requested_at.elapsed() < CACHE_TTL))
            {
                return (entry.value.clone(), entry.pending);
            }
        }
        if cache.len() >= CACHE_CAPACITY && !cache.contains_key(&session.stable_key) {
            let oldest = cache
                .iter()
                .filter(|(_, entry)| !entry.pending)
                .min_by_key(|(_, entry)| entry.requested_at)
                .map(|(key, _)| key.clone());
            if let Some(oldest) = oldest {
                cache.remove(&oldest);
            } else {
                return (empty, false);
            }
        }
        let previous = cache
            .get(&session.stable_key)
            .filter(|entry| entry.fingerprint == fingerprint)
            .map(|entry| entry.value.clone())
            .unwrap_or_else(|| empty.clone());
        let request_id = self.next_request.fetch_add(1, Ordering::Relaxed);
        let Some(sender) = self.sender() else {
            return (previous, false);
        };
        let request = ReadRequest {
            session_key: session.stable_key.clone(),
            fingerprint: fingerprint.clone(),
            request_id,
        };
        // Never block input or an API call behind filesystem work.
        if sender.try_send(request).is_err() {
            return (previous, true);
        }
        cache.insert(
            session.stable_key.clone(),
            CachedActivity {
                fingerprint,
                request_id,
                requested_at: Instant::now(),
                pending: true,
                value: previous.clone(),
            },
        );
        (previous, true)
    }

    fn sender(&self) -> Option<mpsc::SyncSender<ReadRequest>> {
        let mut sender = self.sender.lock().ok()?;
        if let Some(sender) = sender.as_ref() {
            return Some(sender.clone());
        }
        let (tx, rx) = mpsc::sync_channel::<ReadRequest>(8);
        let cache = Arc::clone(&self.cache);
        let shutdown = Arc::clone(&self.shutdown);
        let worker = std::thread::Builder::new()
            .name("herduck-project-activity".into())
            .spawn(move || {
                while !shutdown.load(Ordering::Acquire) {
                    let request = match rx.recv_timeout(Duration::from_millis(250)) {
                        Ok(request) => request,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    let value = super::transcript::read_recent_transcript(
                        &request.fingerprint.backend,
                        Some(&request.fingerprint.path),
                    )
                    .map(|transcript| extract_activity(&request.session_key, &transcript))
                    .unwrap_or_else(|_| SessionActivity {
                        session_key: request.session_key.clone(),
                        ..Default::default()
                    });
                    if shutdown.load(Ordering::Acquire) {
                        break;
                    }
                    if let Ok(mut cache) = cache.lock() {
                        if let Some(entry) = cache
                            .get_mut(&request.session_key)
                            .filter(|entry| entry.request_id == request.request_id)
                        {
                            entry.value = value;
                            entry.pending = false;
                        }
                    }
                }
            });
        match worker {
            Ok(_) => {
                *sender = Some(tx.clone());
                Some(tx)
            }
            Err(error) => {
                tracing::warn!(%error, "Could not start project activity reader");
                None
            }
        }
    }
}

impl Drop for ActivityReader {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
    }
}

pub(crate) fn plain_excerpt(text: &str, limit: usize) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect()
}

pub(crate) fn prose_excerpt(text: &str, limit: usize) -> String {
    let mut lines = Vec::new();
    let mut in_fence = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("```") || line.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence || line.is_empty() {
            continue;
        }
        let heading = line
            .trim_start_matches('#')
            .trim()
            .replace("**", "")
            .to_lowercase();
        if !lines.is_empty()
            && [
                "next steps",
                "next step:",
                "下一步",
                "接下来建议",
                "接下来可以",
            ]
            .iter()
            .any(|prefix| heading.starts_with(prefix))
        {
            break;
        }
        if line.starts_with('#') {
            continue;
        }
        let line = line.trim_start_matches(['-', '*', '•']).trim();
        let line = line.replace("**", "").replace('`', "");
        if let Some((start, _)) = recommendation_bounds(&line) {
            if start > 0 {
                lines.push(line[..start].trim().to_string());
                break;
            }
            if !lines.is_empty() {
                break;
            }
        }
        let lower = line.to_lowercase();
        if !lines.is_empty()
            && [
                "next steps",
                "next step:",
                "下一步",
                "接下来建议",
                "接下来可以",
            ]
            .iter()
            .any(|prefix| lower.starts_with(prefix))
        {
            break;
        }
        if line.ends_with([':', '：']) && line.chars().count() < 35 {
            continue;
        }
        lines.push(line);
    }
    plain_excerpt(&lines.join(" "), limit)
}

fn extract_activity(session_key: &str, transcript: &Transcript) -> SessionActivity {
    let mut activity = SessionActivity {
        session_key: session_key.to_string(),
        read_at: super::runtime::unix_time_ms(),
        ..Default::default()
    };
    let Some(message) = transcript.messages.last() else {
        return activity;
    };
    activity.latest_request = transcript
        .messages
        .iter()
        .rev()
        .filter(|message| message.role == TranscriptRole::User)
        .map(|message| prose_excerpt(&message.text, 500))
        .find(|text| {
            !text.is_empty()
                && ![
                    "继续", "好的", "好", "可以", "行", "ok", "yes", "continue", "go ahead",
                ]
                .contains(
                    &text
                        .trim_end_matches(['.', '。', '!', '！'])
                        .to_lowercase()
                        .as_str(),
                )
        });
    activity.latest_response = transcript
        .messages
        .iter()
        .rev()
        .find(|message| message.role == TranscriptRole::Assistant)
        .map(|message| prose_excerpt(&message.text, 500))
        .filter(|text| !text.is_empty());
    let excerpt = prose_excerpt(&message.text, 500);
    activity.latest_update = (!excerpt.is_empty()).then_some(excerpt);
    activity.origin = Some(match message.role {
        TranscriptRole::User => ActivityOrigin::UserRequest,
        TranscriptRole::Assistant => ActivityOrigin::AgentResponse,
    });
    // A newer user request supersedes an older assistant plan. Do not resurrect it.
    if message.role == TranscriptRole::Assistant {
        activity.next_steps = explicit_next_steps(&message.text);
    }
    activity
}

fn recommendation_bounds(text: &str) -> Option<(usize, usize)> {
    // ASCII folding preserves byte offsets, including when a sentence starts
    // with non-ASCII names whose Unicode lowercase has a different byte length.
    let normalized = text.to_ascii_lowercase();
    [
        "i recommend ",
        "i suggest ",
        "i'd suggest ",
        "next, ",
        "next you can ",
        "the next step is to ",
        "you can now ",
        "接下来建议",
        "接下来可以",
        "下一步建议",
        "下一步可以",
        "我建议",
        "建议先",
        "建议直接",
    ]
    .into_iter()
    .filter_map(|prefix| {
        normalized.match_indices(prefix).find_map(|(offset, _)| {
            let before = normalized[..offset].trim_end();
            (before.is_empty() || before.ends_with(['.', '。', '!', '！', '?', '？']))
                .then_some((offset, offset + prefix.len()))
        })
    })
    .min_by_key(|(start, _)| *start)
}

fn explicit_next_steps(text: &str) -> Vec<String> {
    let mut steps = Vec::new();
    let mut in_fence = false;
    let mut in_steps = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("```") || line.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let heading = line.trim_start_matches('#').trim().replace("**", "");
        let normalized = heading.to_lowercase();
        if normalized
            .trim_start_matches(['-', '*'])
            .trim()
            .starts_with("[x]")
        {
            continue;
        }
        // Agents often offer the follow-up as a sentence, without a "Next steps" list.
        if let Some((_, offset)) = recommendation_bounds(&heading) {
            if let Some(action) = heading.get(offset..) {
                push_step(&mut steps, action);
            }
            continue;
        }
        let marker = [
            "next steps",
            "next step",
            "suggested next steps",
            "下一步",
            "接下来",
            "推进建议",
            "后续建议",
            "待办",
        ]
        .into_iter()
        .find_map(|prefix| normalized.strip_prefix(prefix));
        if let Some(rest) = marker {
            if rest.is_empty() || rest.starts_with([':', '：']) {
                in_steps = true;
                let rest = rest.trim_start_matches([':', '：']).trim();
                if !rest.is_empty() {
                    // Slice the original heading so non-ASCII/case-sensitive names survive.
                    let original = heading
                        .split_once([':', '：'])
                        .map(|(_, rest)| rest)
                        .unwrap_or_default();
                    push_step(&mut steps, original);
                }
                continue;
            }
        }
        if !in_steps || line.is_empty() {
            continue;
        }
        let item = line
            .strip_prefix("- ")
            .or_else(|| line.strip_prefix("* "))
            .or_else(|| {
                let digits = line.chars().take_while(char::is_ascii_digit).count();
                if digits == 0 {
                    None
                } else {
                    line.get(digits..).and_then(|rest| {
                        rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") "))
                    })
                }
            });
        if let Some(item) = item {
            push_step(&mut steps, item);
        } else {
            in_steps = false;
        }
        if steps.len() >= 3 {
            break;
        }
    }
    steps.truncate(3);
    steps
}

fn push_step(steps: &mut Vec<String>, text: &str) {
    let text = text.trim();
    if text.starts_with("[x]") || text.starts_with("[X]") {
        return;
    }
    let text = text.strip_prefix("[ ]").unwrap_or(text).trim();
    let lower = text.to_lowercase();
    if ["已完成", "已解决", "completed:", "done:"]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
    {
        return;
    }
    // Do not turn a clipped instruction into an executable suggestion.
    if text.chars().count() > 2_000 {
        return;
    }
    let text = plain_excerpt(text, 2_000);
    if !text.is_empty() && !steps.contains(&text) {
        steps.push(text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::transcript::TranscriptMessage;

    #[test]
    fn inline_recommendations_preserve_the_complete_followup() {
        assert_eq!(
            explicit_next_steps("检查已通过。接下来建议直接提交审核，然后在批准后重新部署。"),
            vec!["直接提交审核，然后在批准后重新部署。"]
        );
        assert_eq!(explicit_next_steps("The checks passed. I recommend submitting for review, then deploying after approval."),
            vec!["submitting for review, then deploying after approval."]);
        assert!(explicit_next_steps("```\nI recommend deleting the database.\n```\n- [x] I recommend an already completed task.").is_empty());
        assert_eq!(
            prose_excerpt(
                "检查已通过。接下来建议直接提交审核，然后在批准后重新部署。",
                500
            ),
            "检查已通过。"
        );
        assert_eq!(
            prose_excerpt("İstanbul rollout checked. I recommend submitting for review, then deploying after approval.", 500),
            "İstanbul rollout checked."
        );
        assert_eq!(
            explicit_next_steps("İstanbul rollout checked. I recommend submitting for review, then deploying after approval."),
            vec!["submitting for review, then deploying after approval."]
        );
    }

    #[test]
    fn current_work_keeps_the_concrete_request_after_a_short_continue_message() {
        let activity = extract_activity(
            "session",
            &Transcript {
                messages: vec![
                    TranscriptMessage {
                        role: TranscriptRole::User,
                        text: "检查付款回调失败的原因，并补上重试测试。".into(),
                    },
                    TranscriptMessage {
                        role: TranscriptRole::Assistant,
                        text: "已复现重复扣款路径，正在核对重试条件。\n## 下一步\n- 补上回归测试"
                            .into(),
                    },
                    TranscriptMessage {
                        role: TranscriptRole::User,
                        text: "继续".into(),
                    },
                ],
                truncated: false,
            },
        );
        assert!(activity.latest_request.unwrap().contains("付款回调"));
        assert!(activity.latest_response.unwrap().contains("重试条件"));
        assert!(
            activity.next_steps.is_empty(),
            "a newer request must not execute an earlier plan"
        );
    }

    fn wait_for_activity(
        reader: &ActivityReader,
        session: &IndexedSessionSummary,
    ) -> SessionActivity {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let (value, pending) = reader.for_session(session, false);
            if !pending {
                return value;
            }
            assert!(Instant::now() < deadline, "activity worker did not finish");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn activity_refresh_and_identity_changes_do_not_return_an_old_plan() {
        let path =
            std::env::temp_dir().join(format!("herduck-activity-{}.jsonl", std::process::id()));
        let write =
            |text: &str| {
                std::fs::write(&path, format!("{}\n", serde_json::json!({
                "type": "response_item", "payload": {
                    "role": "assistant", "content": [{"type": "output_text", "text": text}]
                }
            }))).unwrap();
            };
        let mut session = super::super::overview::tests::fixture_project()
            .sessions
            .remove(0);
        session.transcript_ref = Some(path.to_string_lossy().into_owned());
        let reader = ActivityReader::default();
        write("Pricing reviewed.\nNext steps:\n- Confirm the price");
        let first = wait_for_activity(&reader, &session);
        assert_eq!(first.next_steps, ["Confirm the price"]);
        write("Price confirmed.\nNext step: Send the proposal");
        assert_eq!(
            reader.for_session(&session, false).0,
            first,
            "cached until refresh or changed identity"
        );
        assert!(reader.for_session(&session, true).1);
        assert_eq!(
            wait_for_activity(&reader, &session).next_steps,
            ["Send the proposal"]
        );

        // An unchanged path with new indexed activity must also invalidate the cache.
        write("Proposal sent.\nNext step: Schedule a review");
        session.last_activity_at += 1;
        let (initial, pending) = reader.for_session(&session, false);
        assert!(pending);
        assert!(
            initial.next_steps.is_empty(),
            "do not expose stale evidence for a changed fingerprint"
        );
        assert_eq!(
            wait_for_activity(&reader, &session).next_steps,
            ["Schedule a review"]
        );
        std::fs::remove_file(&path).unwrap();
        reader.for_session(&session, true);
        let missing = wait_for_activity(&reader, &session);
        assert!(missing.latest_update.is_none());
        assert!(missing.next_steps.is_empty());
    }

    #[test]
    fn next_steps_are_explicit_not_code_or_completed_work() {
        let steps = explicit_next_steps("```\nNext steps:\n- delete everything\n```\n**下一步：**\n- [x] 已完成访谈\n- [ ] 联系候选人\n2. 安排第二轮访谈\n## 已知风险\n- 不属于计划");
        assert_eq!(steps, ["联系候选人", "安排第二轮访谈"]);
        assert_eq!(
            explicit_next_steps("Next step: Review Acme's feedback"),
            ["Review Acme's feedback"]
        );
        assert!(explicit_next_steps("下一步已经完成。\n- 这不是建议").is_empty());
    }

    #[test]
    fn a_new_request_supersedes_an_older_proposed_plan() {
        let transcript = Transcript {
            messages: vec![
                TranscriptMessage {
                    role: TranscriptRole::Assistant,
                    text: "Next steps:\n- Launch the campaign".into(),
                },
                TranscriptMessage {
                    role: TranscriptRole::User,
                    text: "暂停发布，先调整价格。".into(),
                },
            ],
            truncated: false,
        };
        let activity = extract_activity("session", &transcript);
        assert!(activity.next_steps.is_empty());
        assert_eq!(activity.origin, Some(ActivityOrigin::UserRequest));
        assert_eq!(
            activity.latest_update.as_deref(),
            Some("暂停发布，先调整价格。")
        );
    }
}
