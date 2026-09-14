use crate::api::schema::{
    Method, ProjectFollowupGetParams, ProjectFollowupStartParams, ProjectOverviewGetParams,
    ProjectSnapshotParams, Request, ResponseResult, SuccessResponse, TopicCoverGetParams,
    TopicCoverPatch, TopicCoverUpdateParams,
};

pub(super) fn run_topic_command(args: &[String]) -> std::io::Result<i32> {
    run_group_command(args, true)
}

pub(super) fn run_project_command(args: &[String]) -> std::io::Result<i32> {
    run_group_command(args, false)
}

fn run_group_command(args: &[String], topics: bool) -> std::io::Result<i32> {
    if args.is_empty()
        || args
            .iter()
            .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
        || args == ["help"]
    {
        print_help(topics);
        return Ok(if args.is_empty() { 2 } else { 0 });
    }
    let parsed = if !topics && args.first().is_some_and(|arg| arg == "cover") {
        Err("Saved plans belong to Work; use herduck topic cover".into())
    } else {
        parse_method(args)
    };
    let method = match parsed {
        Ok(method) => method,
        Err(message) => {
            eprintln!("{message}");
            print_help(topics);
            return Ok(2);
        }
    };
    let list = matches!(method, Method::ProjectSnapshot(_));
    let response = super::send_request(&Request {
        id: if topics { "cli:topic" } else { "cli:project" }.into(),
        method,
    })?;
    if list && response.get("error").is_none() {
        let response: SuccessResponse = serde_json::from_value(response)?;
        let ResponseResult::ProjectSnapshot { snapshot } = response.result else {
            return Err(std::io::Error::other("Unexpected response to group list"));
        };
        let groups = if topics {
            serde_json::json!({"topics": snapshot.topics, "revision": snapshot.revision})
        } else {
            serde_json::json!({"projects": snapshot.projects, "revision": snapshot.revision})
        };
        println!("{}", serde_json::to_string(&groups)?);
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
        [overview, get, key] if overview == "overview" && get == "get" => {
            Ok(Method::ProjectOverviewGet(ProjectOverviewGetParams {
                project_key: key.clone(),
                refresh: false,
            }))
        }
        [overview, get, key, refresh]
            if overview == "overview" && get == "get" && refresh == "--refresh" =>
        {
            Ok(Method::ProjectOverviewGet(ProjectOverviewGetParams {
                project_key: key.clone(),
                refresh: true,
            }))
        }
        [cover, get, key] if cover == "cover" && get == "get" => {
            Ok(Method::TopicCoverGet(TopicCoverGetParams {
                topic_key: key.clone(),
            }))
        }
        [followup, start, key, suggestion_id] if followup == "followup" && start == "start" => {
            Ok(Method::ProjectFollowupStart(ProjectFollowupStartParams {
                project_key: key.clone(),
                suggestion_id: suggestion_id.clone(),
            }))
        }
        [followup, get, id] if followup == "followup" && get == "get" => {
            Ok(Method::ProjectFollowupGet(ProjectFollowupGetParams {
                followup_id: id.clone(),
            }))
        }
        [followup, cancel, id] if followup == "followup" && cancel == "cancel" => {
            Ok(Method::ProjectFollowupCancel(ProjectFollowupGetParams {
                followup_id: id.clone(),
            }))
        }
        [cover, update, key, flags @ ..] if cover == "cover" && update == "update" => {
            Ok(Method::TopicCoverUpdate(TopicCoverUpdateParams {
                topic_key: key.clone(),
                patch: parse_patch(flags)?,
            }))
        }
        _ => Err("Unknown or incomplete Work/Project command".into()),
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

fn print_help(topics: bool) {
    if !topics {
        eprintln!("herduck project commands (JSON output):\n  herduck project list\n  herduck project overview get <project-key> [--refresh]\n  herduck project followup start <project-key> <suggestion-id>\n  herduck project followup get <followup-id>\n  herduck project followup cancel <followup-id>");
        return;
    }
    eprintln!("herduck topic commands for Work (JSON output):");
    eprintln!("  herduck topic list");
    eprintln!("  herduck topic overview get <topic-key> [--refresh]");
    eprintln!("  herduck topic followup start <topic-key> <suggestion-id>");
    eprintln!("  herduck topic followup get <followup-id>");
    eprintln!("  herduck topic followup cancel <followup-id>");
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
    fn overview_cli_preserves_the_key_and_refresh_is_opt_in() {
        for refresh in [false, true] {
            let mut values = args(&["overview", "get", "folder with spaces"]);
            if refresh {
                values.push("--refresh".into());
            }
            let Method::ProjectOverviewGet(params) = parse_method(&values).unwrap() else {
                panic!("overview method");
            };
            assert_eq!(params.project_key, "folder with spaces");
            assert_eq!(params.refresh, refresh);
            for group in ["topic", "project"] {
                let mut command = vec!["herduck".to_string(), group.to_string()];
                command.extend(values.clone());
                assert!(super::super::spec::command()
                    .try_get_matches_from(command)
                    .is_ok());
            }
        }
        for invalid in [
            vec!["overview"],
            vec!["overview", "get"],
            vec!["overview", "get", "key", "--unknown"],
        ] {
            assert!(parse_method(&args(&invalid)).is_err());
        }
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
