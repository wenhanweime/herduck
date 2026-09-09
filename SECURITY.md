# Security

Herduck's initial support scope is the current source tree on macOS and Linux. There is no separate
long-term support release or guaranteed response time yet. Update by rebuilding from reviewed
source; automatic update checks and self-update are disabled by default.

## Reporting a vulnerability

Do not publish credentials, exploit details, private session logs, or conversation contents in
an issue or discussion. Use this repository's **Security → Report a vulnerability** action if
private vulnerability reporting is enabled. If the action is unavailable, ask the repository
owner for a private reporting channel without including vulnerability details. A verified private
channel must be established before public release.

Include the affected commit/version and platform, a minimal reproduction using synthetic data,
the impact, and any safe workaround. Do not test against another person's sessions or systems.

## Data and provider boundaries

Herduck reads local agent history and terminal state to build its session library. Local state can
contain sensitive working directories and conversation content; protect the account and filesystem
that hold the configuration and catalog. Never commit your personal configuration or logs.

Fresh installations leave summary generation off. Enabling it requires configuration in
`config.toml`. A local Agent CLI can use a remote model through its own configuration. A custom
API sends selected conversation content to the configured endpoint. Offline naming launches no
Agent and makes no network request. Keep API credentials in environment variables referenced by
`api_key_env`, and review the provider's data policy before use.

Herduck reuses existing ORK3 configuration and state without moving the files. That preserves the
previously configured generation mode and providers; renaming the application does not reset consent
or make an existing remote provider local. Review the active paths and sources in Settings.

The public-source checker catches selected private paths and machine-specific defaults. It does
not replace credential scanning, dependency review, or inspection of Git history before publication.
