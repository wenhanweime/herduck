use crate::api::schema::{
    Method, ProjectSnapshotParams, Request, ResponseResult, SuccessResponse, TopicCoverGetParams,
    TopicCoverPatch, TopicCoverUpdateParams,
};

pub(super) fn run_topic_command(args: &[String]) -> std::io::Result<i32> {
    if args.is_empty()
        || args
            .iter()
            .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
        || args == ["help"]
    {
        print_help();
        return Ok(if args.is_empty() { 2 } else { 0 });
    }
    let method = match parse_method(args) {
        Ok(method) => method,
        Err(message) => {
            eprintln!("{message}");
            print_help();
            return Ok(2);
        }
    };
    let list = matches!(method, Method::ProjectSnapshot(_));
    let response = super::send_request(&Request {
        id: "cli:topic".into(),
        method,
    })?;
    if list && response.get("error").is_none() {
        let response: SuccessResponse = serde_json::from_value(response)?;
        let ResponseResult::ProjectSnapshot { snapshot } = response.result else {
            return Err(std::io::Error::other("Unexpected response to topic list"));
        };
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "topics": snapshot.topics, "revision": snapshot.revision,
            }))?
        );
        Ok(0)
    } else {
        super::print_response(&response)
    }
}

fn parse_method(args: &[String]) -> Result<Method, String> {
    match args {
        [list] if list == "list" => Ok(Method::ProjectSnapshot(ProjectSnapshotParams {
            projects_schema_version: crate::projects::domain::PROJECTS_SCHEMA_VERSION,
        })),
        [cover, get, key] if cover == "cover" && get == "get" => {
            Ok(Method::TopicCoverGet(TopicCoverGetParams {
                topic_key: key.clone(),
            }))
        }
        [cover, update, key, flags @ ..] if cover == "cover" && update == "update" => {
            Ok(Method::TopicCoverUpdate(TopicCoverUpdateParams {
                topic_key: key.clone(),
                patch: parse_patch(flags)?,
            }))
        }
        _ => Err("Unknown or incomplete topic command".into()),
    }
}

fn parse_patch(args: &[String]) -> Result<TopicCoverPatch, String> {
    let mut patch = TopicCoverPatch::default();
    let mut clear_steps = false;
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        if flag == "--clear-next-steps" {
            if patch.next_steps.is_some() {
                return Err("Use either --next-step or --clear-next-steps once".into());
            }
            clear_steps = true;
            patch.next_steps = Some(Vec::new());
            index += 1;
            continue;
        }
        if !matches!(
            flag,
            "--goal" | "--next-step" | "--blocked-note" | "--expected-updated-at"
        ) {
            return Err(format!("Unknown option: {flag}"));
        }
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("Missing value for {flag}"))?;
        match flag {
            "--goal" if patch.goal.is_none() => patch.goal = Some(value.clone()),
            "--blocked-note" if patch.blocked_note.is_none() => {
                patch.blocked_note = Some(value.clone())
            }
            "--next-step" if !clear_steps => patch
                .next_steps
                .get_or_insert_with(Vec::new)
                .push(value.clone()),
            "--expected-updated-at" if patch.expected_updated_at.is_none() => {
                patch.expected_updated_at =
                    Some(value.parse().map_err(|_| "Invalid updated_at timestamp")?);
            }
            _ => return Err(format!("Repeated or conflicting option: {flag}")),
        }
        index += 2;
    }
    patch.validate().map_err(String::from)?;
    Ok(patch)
}

fn print_help() {
    eprintln!("herduck topic commands (JSON output):");
    eprintln!("  herduck topic list");
    eprintln!("  herduck topic cover get <topic-key>");
    eprintln!("  herduck topic cover update <topic-key> [--goal TEXT] [--next-step TEXT ... | --clear-next-steps] [--blocked-note TEXT] [--expected-updated-at N]");
    eprintln!(
        "Omitted fields stay unchanged. Use empty text or --clear-next-steps to clear a field."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn topic_cli_edits_are_partial_and_clear_only_explicit_fields() {
        let Method::TopicCoverUpdate(params) = parse_method(&args(&[
            "cover",
            "update",
            "topic-1",
            "--goal",
            "新的目标",
            "--expected-updated-at",
            "42",
        ]))
        .unwrap() else {
            panic!("update")
        };
        assert_eq!(params.topic_key, "topic-1");
        assert_eq!(params.patch.goal.as_deref(), Some("新的目标"));
        assert_eq!(params.patch.next_steps, None);
        assert_eq!(params.patch.blocked_note, None);
        assert_eq!(params.patch.expected_updated_at, Some(42));
        let clear = parse_patch(&args(&["--blocked-note", "", "--clear-next-steps"])).unwrap();
        assert_eq!(clear.goal, None);
        assert_eq!(clear.blocked_note.as_deref(), Some(""));
        assert_eq!(clear.next_steps, Some(Vec::new()));
    }

    #[test]
    fn topic_cli_rejects_ambiguous_or_invalid_updates_before_connecting() {
        for invalid in [
            vec![],
            vec!["--goal"],
            vec!["--goal", "one", "--goal", "two"],
            vec!["--clear-next-steps", "--next-step", "one"],
            vec!["--next-step", "one", "--clear-next-steps"],
            vec![
                "--next-step",
                "1",
                "--next-step",
                "2",
                "--next-step",
                "3",
                "--next-step",
                "4",
            ],
            vec!["--goal", "one", "--expected-updated-at", "-1"],
        ] {
            assert!(
                parse_patch(&args(&invalid)).is_err(),
                "accepted {invalid:?}"
            );
        }
        assert!(parse_patch(&args(&[
            "--next-step",
            "一",
            "--next-step",
            "二",
            "--next-step",
            "三"
        ]))
        .is_ok());
    }
}
