//! Turns a stored Catalog title into something a person can scan in a narrow sidebar.
//!
//! This is presentation only. The Catalog keeps the raw title it observed; nothing here is
//! persisted or sent over the API, so cleaning rules can change without a schema migration.
//!
//! Stored titles are the user's own opening words, which means they arrive carrying whatever the
//! user pasted: resume commands, session UUIDs, shell prompt lines, and duplicated fragments from
//! a doubled paste. Rendering that verbatim fills the sidebar with noise before the actual request
//! starts.

use super::text::{display_width, truncate_end};

/// Widest a rendered session title may be, in terminal columns.
///
/// Deliberately a display width rather than a character count: the stored title is capped at 96
/// *characters*, and CJK text is two columns per character, so a stored title can be 192 columns —
/// several times a typical sidebar.
const TITLE_MAX_WIDTH: usize = 48;

/// Longest topic prefix kept before it crowds out the task itself.
const TOPIC_MAX_WIDTH: usize = 16;

/// Sentence-ending punctuation, Latin and CJK, used to cut a title at its first clause.
const SENTENCE_ENDINGS: [char; 6] = ['。', '！', '？', '!', '?', '\n'];

/// A session title reduced to the shortest form that still says what the session is about.
pub(crate) struct SessionLabel {
    /// Optional entity label encoded by the title generator (`【对象】任务`). This is presentation
    /// metadata, not part of the task text; callers can choose whether to render it.
    pub subject: Option<String>,
    /// Semantic topic, when the classifier assigned one.
    pub topic: Option<String>,
    /// The user's request, cleaned and clipped.
    pub task: String,
}

/// Openers that assign the agent a role rather than state the task.
///
/// Real dispatch sessions on this machine open with "你是一位为资深 AI 工程师服务的技术内容编辑。
/// 下面是…" — four of them share the first 100 characters verbatim. The role sentence is boilerplate
/// the user pasted from a template; the request is whatever follows it. Matching the *shape*
/// ("你是…。") rather than a fixed vocabulary keeps this working as templates change.
const ROLE_ASSIGNMENT_OPENERS: [&str; 4] = ["你是", "You are", "作为", "扮演"];

/// Drops a leading role-assignment sentence so the request itself leads the label.
///
/// Only the first sentence is considered, and only when something substantive follows it: a title
/// that is *nothing but* a role assignment keeps it, because a role is still better than a blank
/// row.
fn strip_role_assignment(value: &str) -> &str {
    let trimmed = value.trim_start();
    if !ROLE_ASSIGNMENT_OPENERS
        .iter()
        .any(|opener| trimmed.starts_with(opener))
    {
        return value;
    }
    let Some(index) = trimmed.find(SENTENCE_ENDINGS) else {
        return value;
    };
    let rest = trimmed[index..]
        .trim_start_matches(SENTENCE_ENDINGS)
        .trim_start();
    // Require enough text left to be worth showing, otherwise the role was the whole title.
    if rest.chars().count() < 4 {
        return value;
    }
    rest
}

/// Builds the label rendered for one session row.
pub(crate) fn session_label(title: &str, topic_label: Option<&str>) -> SessionLabel {
    let topic = topic_label
        .map(clean_title)
        .filter(|value| !value.is_empty())
        .map(|value| truncate_end(&value, TOPIC_MAX_WIDTH));

    let cleaned = clean_title(strip_role_assignment(title));
    let (subject, task_text) = split_subject(&cleaned);
    let task = if cleaned.is_empty() {
        // Everything was noise. The raw title is still better than an empty row, so fall back to
        // it rather than inventing a placeholder that hides which session this is.
        truncate_end(title.trim(), TITLE_MAX_WIDTH)
    } else {
        truncate_end(&first_sentence(task_text), TITLE_MAX_WIDTH)
    };

    SessionLabel {
        subject,
        topic,
        task,
    }
}

/// Separates the generated entity prefix from the task text.
fn split_subject(value: &str) -> (Option<String>, &str) {
    let Some(rest) = value.strip_prefix('【') else {
        return (None, value);
    };
    let Some(end) = rest.find('】') else {
        return (None, value);
    };
    let subject = rest[..end].trim();
    let task = rest[end + '】'.len_utf8()..].trim_start();
    if subject.is_empty() || task.is_empty() {
        return (None, value);
    }
    (Some(subject.to_string()), task)
}

/// How much of two adjacent rows must match before they read as duplicates.
const DUPLICATE_PREFIX_WIDTH: usize = 12;

/// Extends a label when it would be indistinguishable from the row above it.
///
/// Four sessions on this machine share the same first 100 characters because they were dispatched
/// from one template. Clipping each to its first clause makes them render identically, so the list
/// stops being a list. Rather than inventing a suffix, this reveals *more* of the text the sessions
/// genuinely differ in — templates diverge eventually, so showing a later clause distinguishes them.
///
/// Returns the label unchanged when the two are genuinely the same text; a fake difference would be
/// worse than an honest repeat.
pub(crate) fn disambiguate_from(label: &str, previous: Option<&str>, full_title: &str) -> String {
    let Some(previous) = previous else {
        return label.to_string();
    };
    if !shares_prefix(label, previous) {
        return label.to_string();
    }

    let cleaned = clean_title(strip_role_assignment(full_title));
    let (_, task_text) = split_subject(&cleaned);
    let mut rest = task_text;
    // Walk forward one clause at a time, looking for the first that reads differently.
    for _ in 0..3 {
        let Some(index) = rest.find(SENTENCE_ENDINGS) else {
            break;
        };
        rest = rest[index..]
            .trim_start_matches(SENTENCE_ENDINGS)
            .trim_start();
        if rest.is_empty() {
            break;
        }
        let candidate = truncate_end(&first_sentence(rest), TITLE_MAX_WIDTH);
        if !shares_prefix(&candidate, previous) && !candidate.trim().is_empty() {
            return candidate;
        }
    }
    label.to_string()
}

fn shares_prefix(left: &str, right: &str) -> bool {
    let head = |value: &str| take_width(value, DUPLICATE_PREFIX_WIDTH);
    !left.trim().is_empty() && head(left) == head(right)
}

fn take_width(value: &str, max_width: usize) -> String {
    let mut output = String::new();
    let mut width = 0usize;
    for character in value.chars() {
        let next = display_width(&character.to_string());
        if width + next > max_width {
            break;
        }
        width += next;
        output.push(character);
    }
    output
}

/// Strips the boilerplate that surrounds a user's actual words.
pub(crate) fn clean_title(value: &str) -> String {
    let mut cleaned = String::with_capacity(value.len());
    let mut rest = value;

    // Resume invitations are pasted back in verbatim by users continuing a session, and carry a
    // UUID that is pure noise in a sidebar.
    while let Some(start) = find_resume_command(rest) {
        cleaned.push_str(&rest[..start.0]);
        cleaned.push(' ');
        rest = &rest[start.1..];
    }
    cleaned.push_str(rest);

    // Line-based filtering has to run before whitespace is collapsed, or the line boundaries it
    // keys on are already gone.
    let cleaned = strip_shell_noise(&cleaned);
    let cleaned = strip_bare_uuids(&cleaned);
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    collapse_immediate_repeat(collapsed.trim())
}

/// Whether a title says so little that showing it teaches the user nothing.
///
/// Used to decide when a session should be de-emphasised, not to discard the title: a row with a
/// weak title is still a real session the user may want to open.
pub(crate) fn is_low_signal(value: &str) -> bool {
    let cleaned = clean_title(value);
    if cleaned.is_empty() {
        return true;
    }
    if is_default_session_title(&cleaned) {
        return true;
    }
    let meaningful = cleaned
        .chars()
        .filter(|character| character.is_alphanumeric())
        .count();
    meaningful < 3
}

/// Matches the `New session - <ISO timestamp>` placeholder agents write before any user turn.
fn is_default_session_title(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("New session - ") else {
        return false;
    };
    rest.starts_with(|character: char| character.is_ascii_digit())
}

/// Finds a `<agent> --resume <uuid>` run, returning its byte range.
fn find_resume_command(value: &str) -> Option<(usize, usize)> {
    let marker = value.find("--resume")?;
    // Include the agent name that precedes the flag, e.g. "grok --resume ...".
    let start = value[..marker]
        .rfind(|character: char| character.is_whitespace())
        .map_or(marker, |index| {
            let candidate = value[..index].trim_end();
            candidate
                .rfind(|character: char| character.is_whitespace())
                .map_or(0, |previous| previous + 1)
        });
    let after_flag = marker + "--resume".len();
    let rest = &value[after_flag..];
    let consumed = rest
        .char_indices()
        .find(|(_, character)| !character.is_whitespace())
        .map_or(rest.len(), |(index, _)| {
            rest[index..]
                .find(char::is_whitespace)
                .map_or(rest.len(), |offset| index + offset)
        });
    Some((start, after_flag + consumed))
}

/// Drops standalone UUIDs, which identify a session but never describe it.
fn strip_bare_uuids(value: &str) -> String {
    value
        .split_whitespace()
        .filter(|token| !is_uuid_like(token.trim_matches(|c: char| c == '`' || c == '"')))
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_uuid_like(token: &str) -> bool {
    let groups = token.split('-').collect::<Vec<_>>();
    groups.len() == 5
        && groups
            .iter()
            .all(|group| !group.is_empty() && group.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Removes terminal chrome that lands in a title when a user pastes a whole shell session.
fn strip_shell_noise(value: &str) -> String {
    value
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with("Last login:") && !trimmed.starts_with('❯')
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Collapses a phrase that was immediately pasted twice, e.g. "看下这个看下这个".
///
/// Only an exact adjacent repeat is folded. A title that legitimately repeats a word with anything
/// between the two copies is left alone.
fn collapse_immediate_repeat(value: &str) -> String {
    let characters = value.chars().collect::<Vec<_>>();
    let length = characters.len();
    // Longest repeat first, so "abab" folds to "ab" rather than stopping at a shorter match.
    for span in (2..=length / 2).rev() {
        if characters[..span] == characters[span..span * 2] {
            let remainder = characters[span..].iter().collect::<String>();
            return collapse_immediate_repeat(remainder.trim());
        }
    }
    value.to_string()
}

/// Keeps the first clause, so a long request is represented by its opening ask.
fn first_sentence(value: &str) -> String {
    match value.find(SENTENCE_ENDINGS) {
        Some(index) if index > 0 => value[..index].trim().to_string(),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_command_and_uuid_are_stripped() {
        let cleaned =
            clean_title("看下这个 grok 项目 grok --resume 01a01579-2b40-7e01-86b0-730297b95b12");
        assert!(!cleaned.contains("--resume"), "{cleaned}");
        assert!(!cleaned.contains("01a01579"), "{cleaned}");
        assert!(cleaned.contains("项目"), "{cleaned}");
    }

    #[test]
    fn bare_uuid_is_dropped_but_ordinary_hyphenated_words_survive() {
        assert_eq!(
            clean_title("fix 01a01579-2b40-7e01-86b0-730297b95b12 now"),
            "fix now"
        );
        assert_eq!(
            clean_title("sample-project cluster work"),
            "sample-project cluster work"
        );
    }

    #[test]
    fn doubled_paste_is_folded_once() {
        assert_eq!(clean_title("看下这个看下这个"), "看下这个");
        // A repeat that is not adjacent must survive untouched.
        assert_eq!(clean_title("修复 A 然后修复 B"), "修复 A 然后修复 B");
    }

    #[test]
    fn shell_chrome_is_removed() {
        let cleaned = clean_title("Last login: Tue Aug 19\n看下 mihomo 配置");
        assert!(!cleaned.contains("Last login"), "{cleaned}");
        assert!(cleaned.contains("mihomo"), "{cleaned}");
    }

    /// Four sessions on this machine share the same first 100 characters because they were
    /// dispatched from one role template. The request begins after the role sentence.
    #[test]
    fn role_assignment_opener_is_dropped_so_the_task_leads() {
        let label = session_label(
            "你是一位为资深 AI 工程师服务的技术内容编辑。把今天的会话记录整理成周报",
            None,
        );
        assert!(
            label.task.starts_with("把今天的会话记录"),
            "the request must lead the label: {:?}",
            label.task
        );
    }

    #[test]
    fn generated_subject_is_metadata_and_task_has_no_repeated_prefix() {
        let label = session_label("【简历】整理腾讯 WorkBuddy 面试准备", None);
        assert_eq!(label.subject.as_deref(), Some("简历"));
        assert_eq!(label.task, "整理腾讯 WorkBuddy 面试准备");
    }

    /// A title that is only a role assignment keeps it: a role still identifies the session, and a
    /// blank row identifies nothing.
    #[test]
    fn a_title_that_is_only_a_role_keeps_it() {
        let label = session_label("你是一名 iOS 启动性能专家。", None);
        assert!(label.task.contains("iOS"), "{:?}", label.task);
    }

    /// Sibling rows that clip to the same clause stop being a list.
    #[test]
    fn adjacent_duplicate_rows_reveal_a_later_clause() {
        let title = "仓库 herduck 的审阅任务。请修复侧栏高亮的具体问题";
        let first = session_label(title, None).task;
        let second = disambiguate_from(&first, Some(&first), title);
        assert_ne!(second, first, "the second row must not repeat the first");
        assert!(second.contains("侧栏高亮"), "{second:?}");
    }

    /// Two rows whose text is genuinely identical stay identical: inventing a difference would
    /// misrepresent the data.
    #[test]
    fn a_genuinely_identical_title_is_left_alone() {
        let title = "继续";
        let first = session_label(title, None).task;
        let second = disambiguate_from(&first, Some(&first), title);
        assert_eq!(second, first);
    }

    #[test]
    fn low_signal_titles_are_recognised() {
        assert!(is_low_signal("在吗"));
        assert!(is_low_signal("hi"));
        assert!(is_low_signal("New session - 2026-08-14T12:33:40.778Z"));
        assert!(!is_low_signal("看下 mihomo 的分流规则"));
    }

    /// The stored cap is 96 *characters*; CJK is two columns each, so the rendered title has to be
    /// clipped by width or it overruns the sidebar by a factor of two.
    #[test]
    fn label_is_clipped_by_display_width_not_character_count() {
        let long = "看".repeat(96);
        let label = session_label(&long, None);
        assert!(
            display_width(&label.task) <= TITLE_MAX_WIDTH,
            "width was {}",
            display_width(&label.task)
        );
    }

    #[test]
    fn topic_and_task_share_the_width_budget() {
        let label = session_label(&"看".repeat(96), Some("sample-project 聚类与开发"));
        let topic = label.topic.expect("topic");
        assert!(display_width(&topic) <= TOPIC_MAX_WIDTH);
        assert!(
            display_width(&topic) + display_width(&label.task) + 2 <= TITLE_MAX_WIDTH,
            "combined label must fit the sidebar"
        );
    }

    #[test]
    fn first_clause_stands_in_for_a_long_request() {
        let label = session_label("修复登录问题。然后再看别的事情。", None);
        assert_eq!(label.task, "修复登录问题");
    }

    /// A title made entirely of noise must still render something identifying rather than an
    /// empty row.
    #[test]
    fn fully_stripped_title_falls_back_to_the_raw_text() {
        let label = session_label("01a01579-2b40-7e01-86b0-730297b95b12", None);
        assert!(!label.task.is_empty());
    }
}
