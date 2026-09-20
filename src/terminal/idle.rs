//! Bounded termination of an idle Agent job, without closing its PTY or shell.

use crate::{detect::Agent, platform::ForegroundJob};

pub(crate) struct IdleAgentStop {
    processes: Vec<(u32, String)>,
    pub(crate) includes_pty_child: bool,
}

fn agent_job_pids(child_pid: u32, expected: Agent, job: &ForegroundJob) -> Option<Vec<u32>> {
    if child_pid == 0 || job.process_group_id == 0 {
        return None;
    }
    if crate::detect::identify_agent_in_job(job)?.0 != expected {
        return None;
    }
    // A child Agent behind a shell in the same group is not permission to kill the shell.
    // Interactive shell jobs normally have a separate foreground process group.
    if let Some(child) = job
        .processes
        .iter()
        .find(|process| process.pid == child_pid)
    {
        let child_job = ForegroundJob {
            process_group_id: child_pid,
            processes: vec![child.clone()],
        };
        if crate::detect::identify_agent_in_job(&child_job)?.0 != expected {
            return None;
        }
    }
    let mut pids: Vec<_> = job.processes.iter().map(|process| process.pid).collect();
    pids.retain(|pid| *pid != 0);
    pids.sort_unstable();
    pids.dedup();
    (!pids.is_empty()).then_some(pids)
}

impl IdleAgentStop {
    pub(crate) fn capture(child_pid: u32, expected: Agent) -> Option<Self> {
        let job = crate::platform::foreground_job(child_pid)?;
        let pids = agent_job_pids(child_pid, expected, &job)?;
        // Refuse ambiguous identities. In particular, never escalate against a reused PID.
        let processes = pids
            .into_iter()
            .map(|pid| crate::platform::process_instance_id(pid).map(|marker| (pid, marker)))
            .collect::<Option<Vec<_>>>()?;
        let current_job = crate::platform::foreground_job(child_pid)?;
        if current_job.process_group_id != job.process_group_id
            || agent_job_pids(child_pid, expected, &current_job)?
                != processes.iter().map(|(pid, _)| *pid).collect::<Vec<_>>()
        {
            return None;
        }
        Some(Self {
            includes_pty_child: processes.iter().any(|(pid, _)| *pid == child_pid),
            processes,
        })
    }

    pub(crate) fn start(self) {
        let pids = self.survivors();
        crate::platform::signal_processes(&pids, crate::platform::Signal::Terminate);
        // Do not wait in the server's input/render loop. Only the original process instances
        // can receive the escalation; a new Agent in this terminal is never targeted.
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let pids = self.survivors();
            if !pids.is_empty() {
                tracing::info!(
                    ?pids,
                    "forcing idle agent processes to exit after grace period"
                );
                crate::platform::signal_processes(&pids, crate::platform::Signal::Kill);
            }
        });
    }

    fn survivors(&self) -> Vec<u32> {
        self.processes
            .iter()
            .filter(|(pid, marker)| {
                crate::platform::process_instance_id(*pid).as_ref() == Some(marker)
            })
            .map(|(pid, _)| *pid)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ForegroundProcess;

    fn process(pid: u32, argv: &[&str]) -> ForegroundProcess {
        ForegroundProcess {
            pid,
            name: argv[0].into(),
            argv0: Some(argv[0].into()),
            argv: Some(argv.iter().map(|arg| (*arg).into()).collect()),
            cmdline: None,
        }
    }

    #[test]
    fn idle_agent_targets_native_and_wrapped_codex_and_grok() {
        for (agent, argv) in [
            (Agent::Codex, vec!["codex"]),
            (
                Agent::Codex,
                vec!["node", "/opt/node_modules/@openai/codex/bin/codex.js"],
            ),
            (Agent::Grok, vec!["grok"]),
            (Agent::Grok, vec!["grok-1.0.30-macos-aarch64"]),
        ] {
            let job = ForegroundJob {
                process_group_id: 20,
                processes: vec![process(20, &argv)],
            };
            assert_eq!(agent_job_pids(10, agent, &job), Some(vec![20]));
            assert_eq!(agent_job_pids(20, agent, &job), Some(vec![20]));
        }
    }

    #[test]
    fn stale_agent_labels_cannot_terminate_shell_or_another_job() {
        for argv in [vec!["zsh"], vec!["sleep", "100"], vec!["claude"]] {
            let job = ForegroundJob {
                process_group_id: 20,
                processes: vec![process(20, &argv)],
            };
            assert_eq!(agent_job_pids(10, Agent::Codex, &job), None);
        }
        let job = ForegroundJob {
            process_group_id: 10,
            processes: vec![process(10, &["zsh"]), process(20, &["codex"])],
        };
        assert_eq!(agent_job_pids(10, Agent::Codex, &job), None);
    }

    #[test]
    fn versioned_grok_paths_are_recognized_without_accepting_arbitrary_prefixes() {
        let job = ForegroundJob {
            process_group_id: 20,
            processes: vec![process(
                20,
                &["/opt/grok/downloads/grok-1.0.30-linux-x86_64"],
            )],
        };
        assert_eq!(agent_job_pids(10, Agent::Grok, &job), Some(vec![20]));
        for name in [
            "grok-worker",
            "grok-1..30-macos-aarch64",
            "grok-1.0.30-not-a-platform",
        ] {
            assert_eq!(crate::detect::identify_agent(name), None);
        }
    }

    #[test]
    fn escalation_ignores_processes_with_a_different_birth_marker() {
        let target = IdleAgentStop {
            processes: vec![(std::process::id(), "not-this-process".into())],
            includes_pty_child: false,
        };
        assert!(target.survivors().is_empty());
    }
}
