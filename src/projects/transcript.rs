//! Reads an agent's own transcript file so a historical session can be previewed read-only.
//!
//! This is a *reader*. It opens the file the agent already wrote, never a PTY, and never spawns or
//! resumes an agent: opening history must not start work. It reuses `adapters::visit_json_lines`,
//! so the scan-byte and line-length bounds that protect the scanner protect this path too.
//!
//! Only the four backends that record a file-backed transcript are supported. OpenCode keeps its
//! messages in a SQLite database rather than a transcript file and has no `transcript_ref`, so it
//! reports `Unsupported` instead of silently rendering an empty conversation.

use std::path::Path;

use serde_json::Value;

use super::adapters::{strip_injected_preamble, visit_json_lines};

/// Upper bound on messages held for one preview.
///
/// A long session can hold thousands of turns; the preview exists to remind the user what the
/// session was about, so it keeps the opening of the conversation and reports the rest as elided
/// rather than holding an unbounded transcript in memory.
const MAX_MESSAGES: usize = 500;

/// Longest single message kept, in characters. Long tool dumps are clipped, not dropped.
const MAX_MESSAGE_CHARS: usize = 4_000;

/// Full-context history is still bounded by the JSONL reader's 8 MiB scan ceiling, so retaining
/// every readable character it encounters cannot grow without limit.
const MAX_FULL_MESSAGE_CHARS: usize = 8 * 1024 * 1024;

/// Lines of prose kept per turn in the preview.
///
/// The preview answers "what was this session about", not "what exactly was said". Measured over
/// this machine's transcripts, 753 turns are a single line and only 69 run past ten, so this
/// changes almost nothing for ordinary replies while stopping one long answer from filling the
/// pane.
const MAX_PREVIEW_LINES: usize = 3;

/// Reduces a turn to the first few lines of actual prose.
///
/// Fenced code blocks are dropped rather than counted: a preview of a coding session is mostly
/// commands and diffs, and those crowd out the sentences that say what happened. The full text is
/// still on disk — this is a summary, and it says so when it elides.
pub(crate) fn preview_excerpt(text: &str, role: TranscriptRole) -> (String, bool) {
    let mut prose: Vec<&str> = Vec::new();
    let mut in_fence = false;
    let mut dropped_code = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            dropped_code = true;
            continue;
        }
        if in_fence || trimmed.is_empty() {
            continue;
        }
        prose.push(line);
    }

    let over_budget = prose.len() > MAX_PREVIEW_LINES;
    let kept: Vec<&str> = match role {
        // The user's opening words are the request.
        TranscriptRole::User => prose.into_iter().take(MAX_PREVIEW_LINES).collect(),
        // An agent's reply narrates its way to an answer — "我先看下项目实际结构" first, the
        // outcome last. Measured across this machine's transcripts, 58 of 103 turns run past
        // three segments and the conclusion is always at the end, so the tail is the summary.
        TranscriptRole::Assistant => {
            let skip = prose.len().saturating_sub(MAX_PREVIEW_LINES);
            prose.into_iter().skip(skip).collect()
        }
    };

    (kept.join("\n"), over_budget || dropped_code)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptMessage {
    pub role: TranscriptRole,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcript {
    pub messages: Vec<TranscriptMessage>,
    /// True when the session had more messages than the preview kept.
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptError {
    /// The backend does not keep a file-backed transcript.
    Unsupported(String),
    /// The Catalog has no transcript path for this session.
    NoTranscript,
    /// The file is gone, unreadable, or not parseable.
    Unreadable,
    /// The file parsed but held no user or assistant turns.
    Empty,
}

impl TranscriptError {
    /// A sentence shown on the preview card in place of the conversation.
    pub fn message(&self) -> String {
        match self {
            Self::Unsupported(backend) => {
                format!("{backend} does not store a readable transcript file for this session.")
            }
            Self::NoTranscript => "This session has no transcript on disk to preview.".to_string(),
            Self::Unreadable => {
                "The transcript file could not be read. It may have been moved or deleted."
                    .to_string()
            }
            Self::Empty => "This session's transcript has no conversation turns.".to_string(),
        }
    }
}

/// Reads the conversation for one session.
///
/// `transcript_ref` is the path the scanner recorded, so it already passed the adapter's
/// allowlisted-root and symlink checks when it was written.
pub fn read_transcript(
    backend: &str,
    transcript_ref: Option<&str>,
) -> Result<Transcript, TranscriptError> {
    read_transcript_with_collector(backend, transcript_ref, Collector::preview())
}

/// Reads all readable conversation turns inside the bounded JSONL scan window.
///
/// Unlike [`read_transcript`], this does not apply the preview's per-message or message-count
/// clipping. It is used only after the user explicitly enters a historical session's full context.
pub fn read_full_transcript(
    backend: &str,
    transcript_ref: Option<&str>,
) -> Result<Transcript, TranscriptError> {
    read_transcript_with_collector(backend, transcript_ref, Collector::full())
}

fn read_transcript_with_collector(
    backend: &str,
    transcript_ref: Option<&str>,
    mut collector: Collector,
) -> Result<Transcript, TranscriptError> {
    let Some(reference) = transcript_ref.filter(|value| !value.is_empty()) else {
        return Err(match backend {
            "opencode" => TranscriptError::Unsupported("OpenCode".to_string()),
            _ => TranscriptError::NoTranscript,
        });
    };
    let path = Path::new(reference);

    let parsed = match backend {
        "codex" => read_codex(path, &mut collector),
        "claude" => read_claude(path, &mut collector),
        "pi" => read_pi(path, &mut collector),
        // Grok records the session as a directory; the conversation lives beside the summary the
        // scanner indexed.
        "grok" => read_grok(path, &mut collector),
        other => return Err(TranscriptError::Unsupported(other.to_string())),
    };
    parsed.map_err(|()| TranscriptError::Unreadable)?;

    if collector.messages.is_empty() {
        return Err(TranscriptError::Empty);
    }
    Ok(Transcript {
        messages: collector.messages,
        truncated: collector.truncated,
    })
}

struct Collector {
    messages: Vec<TranscriptMessage>,
    truncated: bool,
    max_messages: usize,
    max_message_chars: usize,
}

impl Collector {
    fn preview() -> Self {
        Self {
            messages: Vec::new(),
            truncated: false,
            max_messages: MAX_MESSAGES,
            max_message_chars: MAX_MESSAGE_CHARS,
        }
    }

    fn full() -> Self {
        Self {
            messages: Vec::new(),
            truncated: false,
            max_messages: usize::MAX,
            max_message_chars: MAX_FULL_MESSAGE_CHARS,
        }
    }

    /// Records one segment of the conversation.
    ///
    /// Consecutive assistant segments are folded into the single reply they belong to. An agent
    /// working through one request emits a `thinking → text → tool_use → tool_result` loop, and
    /// each `text` in it is narration between tool calls, not a separate answer: one real request
    /// on this machine produced 23 of them. Treating each as its own message made a preview read as
    /// if the agent had replied 23 times to one question.
    ///
    /// Returns `Some(())` once full so the caller can stop reading.
    fn push(&mut self, role: TranscriptRole, raw: &str) -> Option<()> {
        // Harness preamble is not something the user or the agent said, so it is stripped here the
        // same way titles strip it.
        let text = match role {
            TranscriptRole::User => strip_wrapper_blocks(strip_injected_preamble(raw)),
            TranscriptRole::Assistant => raw.trim().to_string(),
        };
        let text = text.trim();
        if text.is_empty() {
            return None;
        }

        if role == TranscriptRole::Assistant {
            if let Some(open) = self
                .messages
                .last_mut()
                .filter(|message| message.role == TranscriptRole::Assistant)
            {
                // Keep the running reply within the same bound a single message gets.
                if open.text.chars().count() < self.max_message_chars {
                    open.text.push('\n');
                    open.text.push_str(text);
                    open.text = clip_chars(&open.text, self.max_message_chars);
                }
                return None;
            }
        }

        if self.messages.len() >= self.max_messages {
            self.truncated = true;
            return Some(());
        }
        self.messages.push(TranscriptMessage {
            role,
            text: clip_chars(text, self.max_message_chars),
        });
        None
    }
}

fn clip_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let head = text.chars().take(max_chars).collect::<String>();
    format!("{head}…")
}

/// Wrapper blocks that harnesses inject into a turn and that no person typed.
///
/// `strip_injected_preamble` handles the markers the *scanner* needs for titles. A preview shows
/// far more turns than a title scan ever reads, which surfaces wrappers the scanner never had to
/// care about: probing this machine's real transcripts showed Codex opening with
/// `<codex_internal_context>` and Grok with `<system-reminder>`. Both are machinery, not
/// conversation, so a preview that leads with them buries the actual exchange.
const WRAPPER_TAGS: [&str; 4] = [
    "codex_internal_context",
    "system-reminder",
    "user_instructions",
    "environment_context",
];

/// Removes whole `<tag>…</tag>` wrapper blocks, keeping anything the user wrote around them.
fn strip_wrapper_blocks(value: &str) -> String {
    let mut rest = value;
    let mut output = String::with_capacity(value.len());
    'outer: loop {
        let trimmed = rest.trim_start();
        for tag in WRAPPER_TAGS {
            let open = format!("<{tag}");
            if !trimmed.starts_with(&open) {
                continue;
            }
            let close = format!("</{tag}>");
            match trimmed.find(&close) {
                Some(index) => {
                    rest = &trimmed[index + close.len()..];
                    continue 'outer;
                }
                // An unclosed wrapper swallows the rest of the turn.
                None => return output.trim().to_string(),
            }
        }
        break;
    }
    output.push_str(rest);
    output.trim().to_string()
}

/// Content segment types that carry something a person would read.
///
/// Everything else in a turn — `thinking`, `tool_use`, `tool_result`, image blocks — is machinery.
/// A preview that renders it shows the agent talking to itself instead of the conversation.
const READABLE_SEGMENT_TYPES: [&str; 3] = ["text", "input_text", "output_text"];

/// Joins every readable segment of a turn's content.
///
/// Deliberately not `adapters::first_text`, which returns only the *first* segment: a real Codex
/// user turn was observed carrying AGENTS.md in segment 0 and `<environment_context>` in segment 1,
/// so a request in a later segment would have been dropped entirely. `first_text` stays as it is
/// because the title scanner's contract depends on it.
fn all_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => {
            let joined = items
                .iter()
                .filter_map(|item| {
                    // A bare string segment has no type to check; typed segments must be readable.
                    if let Some(text) = item.as_str() {
                        return Some(text.to_string());
                    }
                    let kind = item.get("type").and_then(Value::as_str)?;
                    if !READABLE_SEGMENT_TYPES.contains(&kind) {
                        return None;
                    }
                    item.get("text").and_then(Value::as_str).map(str::to_string)
                })
                .collect::<Vec<_>>()
                .join("\n");
            (!joined.is_empty()).then_some(joined)
        }
        _ => None,
    }
}

/// Unwraps `<user_query>…</user_query>`, which Grok wraps around every real user turn.
///
/// Unlike the wrapper tags this keeps the *contents* and drops only the tags: the query is the one
/// part of a Grok user turn worth showing.
fn unwrap_user_query(value: &str) -> String {
    let mut out = String::new();
    let mut rest = value;
    while let Some(start) = rest.find("<user_query>") {
        let after_open = start + "<user_query>".len();
        let Some(end) = rest[after_open..].find("</user_query>") else {
            break;
        };
        out.push_str(rest[after_open..after_open + end].trim());
        rest = &rest[after_open + end + "</user_query>".len()..];
    }
    if out.is_empty() {
        value.to_string()
    } else {
        out
    }
}

fn read_codex(path: &Path, collector: &mut Collector) -> Result<(), ()> {
    visit_json_lines(path, |value| {
        // `response_item` carries the durable turn record. `event_msg/user_message` repeats the
        // same user text as a live event, so reading only response items avoids duplicates.
        if value.get("type").and_then(Value::as_str) != Some("response_item") {
            return None;
        }
        let role = match value.pointer("/payload/role").and_then(Value::as_str) {
            Some("user") => TranscriptRole::User,
            Some("assistant") => TranscriptRole::Assistant,
            // `developer` turns are injected instructions, not conversation.
            _ => return None,
        };
        let text = value.pointer("/payload/content").and_then(all_text)?;
        collector.push(role, &text)
    })
}

fn read_claude(path: &Path, collector: &mut Collector) -> Result<(), ()> {
    visit_json_lines(path, |value| {
        let role = match value.get("type").and_then(Value::as_str) {
            Some("user") => TranscriptRole::User,
            Some("assistant") => TranscriptRole::Assistant,
            _ => return None,
        };
        // Claude files tool results under `type:"user"`. They are excluded by the segment-type
        // whitelist in `all_text`: a record whose only segments are `tool_result` yields no text
        // and is skipped, so it never reaches the preview as a user turn.
        let text = all_text(value.pointer("/message/content")?)?;
        collector.push(role, &text)
    })
}

fn read_pi(path: &Path, collector: &mut Collector) -> Result<(), ()> {
    visit_json_lines(path, |value| {
        if value.get("type").and_then(Value::as_str) != Some("message") {
            return None;
        }
        let role = match value.pointer("/message/role").and_then(Value::as_str) {
            Some("user") => TranscriptRole::User,
            Some("assistant") => TranscriptRole::Assistant,
            _ => return None,
        };
        let text = value.pointer("/message/content").and_then(all_text)?;
        collector.push(role, &text)
    })
}

/// Grok indexes `summary.json`; the turns live in `chat_history.jsonl` in the same directory.
fn read_grok(summary_path: &Path, collector: &mut Collector) -> Result<(), ()> {
    let history = summary_path.parent().ok_or(())?.join("chat_history.jsonl");
    visit_json_lines(&history, |value| {
        let role = match value.get("type").and_then(Value::as_str) {
            Some("user") => TranscriptRole::User,
            Some("assistant") => TranscriptRole::Assistant,
            // The system prompt is not conversation.
            _ => return None,
        };
        let raw = value
            .get("content")
            .and_then(all_text)
            .or_else(|| value.pointer("/message/content").and_then(all_text))?;
        let text = match role {
            TranscriptRole::User => unwrap_user_query(&raw),
            TranscriptRole::Assistant => raw,
        };
        collector.push(role, &text)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_jsonl(name: &str, lines: &[&str]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ork3-transcript-{name}"));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("transcript.jsonl");
        let mut file = std::fs::File::create(&path).expect("create");
        for line in lines {
            writeln!(file, "{line}").expect("write");
        }
        path
    }

    #[test]
    fn codex_response_items_become_conversation_turns() {
        let path = write_jsonl(
            "codex",
            &[
                r#"{"type":"session_meta","payload":{"id":"abc","cwd":"/tmp"}}"#,
                r#"{"type":"response_item","payload":{"role":"user","content":[{"type":"text","text":"看下天气"}]}}"#,
                r#"{"type":"response_item","payload":{"role":"assistant","content":[{"type":"text","text":"今天晴"}]}}"#,
            ],
        );
        let transcript = read_transcript("codex", path.to_str()).expect("transcript");
        assert_eq!(transcript.messages.len(), 2);
        assert_eq!(transcript.messages[0].role, TranscriptRole::User);
        assert_eq!(transcript.messages[0].text, "看下天气");
        assert_eq!(transcript.messages[1].role, TranscriptRole::Assistant);
    }

    /// Codex writes each user turn twice — once as a live `event_msg` and once as the durable
    /// `response_item`. Reading both would show every user message twice.
    #[test]
    fn codex_event_messages_do_not_duplicate_response_items() {
        let path = write_jsonl(
            "codex-dupe",
            &[
                r#"{"type":"event_msg","payload":{"type":"user_message","message":"看下天气"}}"#,
                r#"{"type":"response_item","payload":{"role":"user","content":[{"type":"text","text":"看下天气"}]}}"#,
            ],
        );
        let transcript = read_transcript("codex", path.to_str()).expect("transcript");
        assert_eq!(transcript.messages.len(), 1);
    }

    /// Injected instruction blocks are not something a person said.
    #[test]
    fn injected_preamble_is_stripped_from_user_turns() {
        let path = write_jsonl(
            "codex-preamble",
            &[
                r#"{"type":"response_item","payload":{"role":"user","content":[{"type":"text","text":"<user_instructions>be nice</user_instructions>修复登录"}]}}"#,
            ],
        );
        let transcript = read_transcript("codex", path.to_str()).expect("transcript");
        assert_eq!(transcript.messages[0].text, "修复登录");
    }

    /// Probing this machine's real transcripts showed Codex leading with
    /// `<codex_internal_context>` and Grok with `<system-reminder>` — machinery the scanner's
    /// title path never had to strip because it reads far fewer turns.
    #[test]
    fn harness_wrapper_blocks_are_stripped_from_a_preview() {
        let path = write_jsonl(
            "codex-wrapper",
            &[
                r#"{"type":"response_item","payload":{"role":"user","content":[{"type":"text","text":"<codex_internal_context source=\"goal\">continue</codex_internal_context>看下天气"}]}}"#,
                r#"{"type":"response_item","payload":{"role":"user","content":[{"type":"text","text":"<system-reminder>be brief</system-reminder>再看一次"}]}}"#,
            ],
        );
        let transcript = read_transcript("codex", path.to_str()).expect("transcript");
        assert_eq!(transcript.messages[0].text, "看下天气");
        assert_eq!(transcript.messages[1].text, "再看一次");
    }

    /// A turn that is nothing but a wrapper carries no conversation and must not become a blank
    /// bubble in the preview.
    #[test]
    fn a_turn_of_pure_wrapper_is_dropped() {
        let path = write_jsonl(
            "codex-only-wrapper",
            &[
                r#"{"type":"response_item","payload":{"role":"user","content":[{"type":"text","text":"<system-reminder>machinery</system-reminder>"}]}}"#,
                r#"{"type":"response_item","payload":{"role":"user","content":[{"type":"text","text":"真正的问题"}]}}"#,
            ],
        );
        let transcript = read_transcript("codex", path.to_str()).expect("transcript");
        assert_eq!(transcript.messages.len(), 1);
        assert_eq!(transcript.messages[0].text, "真正的问题");
    }

    /// Claude records tool results as `type:"user"`. On a real transcript they outnumbered genuine
    /// user turns 65 to 3, which made a preview look like the assistant monologuing.
    /// A user's opening words are the request, so the head is the summary.
    #[test]
    fn a_user_turn_is_summarised_by_its_opening_lines() {
        let (excerpt, elided) = preview_excerpt("一\n二\n三\n四\n五", TranscriptRole::User);
        assert_eq!(excerpt, "一\n二\n三");
        assert!(elided, "the cut must be announced");
    }

    /// An agent narrates its way to an answer: "我先看下…" first, the outcome last. Measured across
    /// this machine's transcripts the conclusion is always in the final segment.
    #[test]
    fn an_agent_turn_is_summarised_by_its_closing_lines() {
        let (excerpt, elided) = preview_excerpt(
            "我先看下项目结构\n再查配置\n找到了原因\n已修复并验证",
            TranscriptRole::Assistant,
        );
        assert_eq!(excerpt, "再查配置\n找到了原因\n已修复并验证");
        assert!(elided);
    }

    #[test]
    fn full_transcript_keeps_text_beyond_preview_message_limit() {
        let long_text = format!("{}FULL-CONTEXT-END", "x".repeat(MAX_MESSAGE_CHARS + 32));
        let record = serde_json::json!({
            "type": "response_item",
            "payload": {
                "role": "user",
                "content": [{"type": "text", "text": long_text}],
            },
        });
        let encoded = record.to_string();
        let path = write_jsonl("full-message", &[encoded.as_str()]);

        let preview = read_transcript("codex", path.to_str()).expect("preview");
        let full = read_full_transcript("codex", path.to_str()).expect("full transcript");

        assert!(!preview.messages[0].text.contains("FULL-CONTEXT-END"));
        assert!(full.messages[0].text.contains("FULL-CONTEXT-END"));
    }

    /// One request produced 23 narration segments on this machine. Each was rendered as its own
    /// message, so a preview read as if the agent had answered 23 times.
    #[test]
    fn consecutive_agent_segments_fold_into_one_reply() {
        let path = write_jsonl(
            "claude-fold",
            &[
                r#"{"type":"user","message":{"content":"帮我配置 pi"}}"#,
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"我先找配置"}]}}"#,
                r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"listing","tool_use_id":"t1"}]}}"#,
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"找到了两边"}]}}"#,
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"配置完成并已验证"}]}}"#,
                r#"{"type":"user","message":{"content":"还有问题"}}"#,
            ],
        );
        let transcript = read_transcript("claude", path.to_str()).expect("transcript");
        let roles: Vec<_> = transcript.messages.iter().map(|m| m.role).collect();
        assert_eq!(
            roles,
            vec![
                TranscriptRole::User,
                TranscriptRole::Assistant,
                TranscriptRole::User
            ],
            "one request must yield one reply, not one per narration segment"
        );
        assert!(transcript.messages[1].text.contains("我先找配置"));
        assert!(transcript.messages[1].text.contains("配置完成并已验证"));
    }

    /// A preview of a coding session is mostly commands and diffs; those crowd out the sentences
    /// that say what happened.
    #[test]
    fn preview_drops_fenced_code_blocks() {
        let (excerpt, elided) = preview_excerpt(
            "先看配置\n```sh\nls -la\ncat foo\n```\n结果如上",
            TranscriptRole::User,
        );
        assert_eq!(excerpt, "先看配置\n结果如上");
        assert!(elided, "dropping code must be announced");
        assert!(!excerpt.contains("ls -la"));
    }

    /// A short reply is the common case — 753 of this machine's turns are a single line — and must
    /// pass through untouched and unmarked.
    #[test]
    fn a_short_reply_is_not_marked_as_elided() {
        let (excerpt, elided) =
            preview_excerpt("我先找一下 pi 的配置位置。", TranscriptRole::Assistant);
        assert_eq!(excerpt, "我先找一下 pi 的配置位置。");
        assert!(!elided, "nothing was cut, so nothing may claim it was");
    }

    /// Blank lines are layout, not content: they must not consume the line budget.
    #[test]
    fn blank_lines_do_not_consume_the_budget() {
        let (excerpt, _) = preview_excerpt("一\n\n\n二\n\n三", TranscriptRole::User);
        assert_eq!(excerpt, "一\n二\n三");
    }

    #[test]
    fn claude_tool_results_are_not_user_turns() {
        let path = write_jsonl(
            "claude-tools",
            &[
                r#"{"type":"user","message":{"content":"帮我配置 pi"}}"#,
                // A `text` key on the tool_result on purpose: the segment-type whitelist must
                // reject it by *type*, not merely because the payload lacks readable text.
                r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"file listing","text":"LEAKED","is_error":false,"tool_use_id":"t1"}]}}"#,
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"好的"}]}}"#,
                r#"{"type":"user","message":{"content":"还有问题"}}"#,
            ],
        );
        let transcript = read_transcript("claude", path.to_str()).expect("transcript");
        let users = transcript
            .messages
            .iter()
            .filter(|message| message.role == TranscriptRole::User)
            .count();
        assert_eq!(users, 2, "only genuine user turns count");
        assert!(
            !transcript
                .messages
                .iter()
                .any(|m| m.text.contains("LEAKED")),
            "a tool_result record must be rejected by type"
        );
        assert_eq!(transcript.messages[0].text, "帮我配置 pi");
        assert_eq!(transcript.messages[2].text, "还有问题");
    }

    /// `thinking` and `tool_use` segments are the agent's machinery, not what it said.
    #[test]
    fn assistant_thinking_and_tool_use_segments_are_skipped() {
        let path = write_jsonl(
            "claude-thinking",
            &[
                r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"internal reasoning","signature":"sig","text":"LEAKED"},{"type":"text","text":"我先找一下配置"}]}}"#,
                r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"Bash","text":"LEAKED","input":{"command":"ls"}}]}}"#,
            ],
        );
        let transcript = read_transcript("claude", path.to_str()).expect("transcript");
        assert_eq!(
            transcript.messages.len(),
            1,
            "tool_use turn carries no prose"
        );
        assert_eq!(transcript.messages[0].text, "我先找一下配置");
        // The fixtures put a `text` key on the machinery segments on purpose: the type whitelist,
        // not the absence of a `text` key, has to be what excludes them.
        assert!(
            !transcript.messages[0].text.contains("LEAKED"),
            "machinery segments must be excluded by type, not by luck: {:?}",
            transcript.messages[0].text
        );
    }

    /// A real Codex user turn was observed with AGENTS.md in segment 0 and the actual request in a
    /// later segment, so taking only the first segment dropped the request.
    #[test]
    fn codex_joins_every_readable_segment() {
        let path = write_jsonl(
            "codex-segments",
            &[
                r#"{"type":"response_item","payload":{"role":"user","content":[{"type":"input_text","text":"第一段"},{"type":"input_text","text":"真正的请求"}]}}"#,
            ],
        );
        let transcript = read_transcript("codex", path.to_str()).expect("transcript");
        assert!(
            transcript.messages[0].text.contains("真正的请求"),
            "later segments must not be dropped: {:?}",
            transcript.messages[0].text
        );
    }

    /// Grok wraps every real user turn in `<user_query>`; unlike the other wrappers its *contents*
    /// are the only part worth showing.
    #[test]
    fn grok_user_query_is_unwrapped_not_discarded() {
        let dir = std::env::temp_dir().join("ork3-transcript-grok-query");
        std::fs::create_dir_all(&dir).expect("dir");
        let summary = dir.join("summary.json");
        std::fs::write(&summary, "{}").expect("summary");
        let mut history = std::fs::File::create(dir.join("chat_history.jsonl")).expect("history");
        writeln!(
            history,
            r#"{{"type":"user","content":"<user_query>\n修复登录问题\n</user_query>"}}"#
        )
        .expect("write");
        writeln!(
            history,
            r#"{{"type":"user","content":"<user_info>\nOS: macos\n</user_info>"}}"#
        )
        .expect("write");

        let transcript = read_transcript("grok", summary.to_str()).expect("transcript");
        assert_eq!(
            transcript.messages.len(),
            1,
            "pure machine blocks are dropped"
        );
        assert_eq!(transcript.messages[0].text, "修复登录问题");
    }

    #[test]
    fn claude_user_and_assistant_lines_are_read() {
        let path = write_jsonl(
            "claude",
            &[
                r#"{"type":"user","sessionId":"s","cwd":"/tmp","message":{"content":[{"type":"text","text":"帮我看下"}]}}"#,
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"好的"}]}}"#,
                r#"{"type":"system","message":{"content":"ignored"}}"#,
            ],
        );
        let transcript = read_transcript("claude", path.to_str()).expect("transcript");
        assert_eq!(transcript.messages.len(), 2);
    }

    #[test]
    fn pi_nested_message_role_is_read() {
        let path = write_jsonl(
            "pi",
            &[
                r#"{"type":"session","id":"x","cwd":"/tmp"}"#,
                r#"{"type":"message","message":{"role":"user","content":[{"type":"text","text":"看下 ork3"}]}}"#,
                r#"{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"在看了"}]}}"#,
            ],
        );
        let transcript = read_transcript("pi", path.to_str()).expect("transcript");
        assert_eq!(transcript.messages.len(), 2);
        assert_eq!(transcript.messages[0].text, "看下 ork3");
    }

    /// Grok indexes `summary.json` but stores turns in a sibling file.
    #[test]
    fn grok_reads_chat_history_beside_the_indexed_summary() {
        let dir = std::env::temp_dir().join("ork3-transcript-grok");
        std::fs::create_dir_all(&dir).expect("dir");
        let summary = dir.join("summary.json");
        std::fs::write(&summary, "{}").expect("summary");
        let mut history = std::fs::File::create(dir.join("chat_history.jsonl")).expect("history");
        writeln!(history, r#"{{"type":"system","content":"prompt"}}"#).expect("write");
        writeln!(history, r#"{{"type":"user","content":"跑一下测试"}}"#).expect("write");
        writeln!(history, r#"{{"type":"assistant","content":"好"}}"#).expect("write");

        let transcript = read_transcript("grok", summary.to_str()).expect("transcript");
        assert_eq!(
            transcript.messages.len(),
            2,
            "system prompt must be skipped"
        );
        assert_eq!(transcript.messages[0].text, "跑一下测试");
    }

    /// OpenCode keeps messages in SQLite and has no transcript file. Saying so beats rendering an
    /// empty conversation that looks like data loss.
    #[test]
    fn opencode_reports_unsupported_rather_than_empty() {
        let error = read_transcript("opencode", None).expect_err("unsupported");
        assert!(matches!(error, TranscriptError::Unsupported(_)));
        assert!(error.message().contains("OpenCode"));
    }

    #[test]
    fn missing_file_is_reported_as_unreadable() {
        let error = read_transcript("codex", Some("/nonexistent/transcript.jsonl"))
            .expect_err("unreadable");
        assert_eq!(error, TranscriptError::Unreadable);
    }

    #[test]
    fn transcript_without_turns_is_reported_as_empty() {
        let path = write_jsonl(
            "codex-empty",
            &[r#"{"type":"session_meta","payload":{"id":"abc","cwd":"/tmp"}}"#],
        );
        let error = read_transcript("codex", path.to_str()).expect_err("empty");
        assert_eq!(error, TranscriptError::Empty);
    }
}
