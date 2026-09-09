use std::path::{Path, PathBuf};

use tracing::warn;

use super::{model::LoadedConfig, Config, CONFIG_PATH_ENV_VAR, LEGACY_CONFIG_PATH_ENV_VAR};

const KNOWN_TOP_LEVEL_CONFIG_KEYS: &[&str] = &[
    "advanced",
    "experimental",
    "keys",
    "onboarding",
    "projects",
    "remote",
    "session",
    "terminal",
    "theme",
    "ui",
    "update",
    "worktrees",
];

/// Directory name for a fresh installation; existing ORK3 roots remain in use.
pub fn app_dir_name() -> &'static str {
    if cfg!(debug_assertions) {
        "herduck-dev"
    } else {
        crate::build_info::PRODUCT_NAME
    }
}

const RUNTIME_NAMESPACE_ENV_VAR: &str = "HERDUCK_RUNTIME_NAMESPACE";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeNamespace {
    Herduck,
    Ork3,
}

impl RuntimeNamespace {
    fn select(current_exists: bool, legacy_exists: bool) -> Self {
        if !current_exists && legacy_exists {
            Self::Ork3
        } else {
            Self::Herduck
        }
    }

    fn product_name(self) -> &'static str {
        match self {
            Self::Herduck => crate::build_info::PRODUCT_NAME,
            Self::Ork3 => "ork3",
        }
    }

    fn dir_name(self) -> &'static str {
        match self {
            Self::Herduck => app_dir_name(),
            Self::Ork3 if cfg!(debug_assertions) => "ork3-dev",
            Self::Ork3 => "ork3",
        }
    }

    fn from_inherited(value: Option<&str>) -> Option<Self> {
        match value {
            Some("herduck") => Some(Self::Herduck),
            Some("ork3") => Some(Self::Ork3),
            _ => None,
        }
    }
}

#[derive(Clone)]
struct RuntimePaths {
    namespace: RuntimeNamespace,
    config: PathBuf,
    state: PathBuf,
}

fn discover_runtime_paths() -> RuntimePaths {
    let current = config_dir_for_name(app_dir_name());
    let legacy = config_dir_for_name(RuntimeNamespace::Ork3.dir_name());
    let inherited = std::env::var(RUNTIME_NAMESPACE_ENV_VAR).ok();
    let namespace = RuntimeNamespace::from_inherited(inherited.as_deref())
        .unwrap_or_else(|| RuntimeNamespace::select(current.exists(), legacy.is_dir()));
    RuntimePaths {
        namespace,
        config: match namespace {
            RuntimeNamespace::Herduck => current,
            RuntimeNamespace::Ork3 => legacy,
        },
        state: state_dir_for_name(namespace.dir_name()),
    }
}

fn cached_runtime_paths(
    paths: &std::sync::OnceLock<RuntimePaths>,
    discover: impl FnOnce() -> RuntimePaths,
) -> RuntimePaths {
    paths.get_or_init(discover).clone()
}

fn runtime_paths() -> RuntimePaths {
    #[cfg(not(test))]
    {
        // A live server must not switch storage when another process creates the new root.
        static PATHS: std::sync::OnceLock<RuntimePaths> = std::sync::OnceLock::new();
        cached_runtime_paths(&PATHS, discover_runtime_paths)
    }
    #[cfg(test)]
    {
        // Unit tests deliberately vary XDG roots; the cache contract is tested with a local cell.
        discover_runtime_paths()
    }
}

pub(crate) fn uses_legacy_namespace() -> bool {
    runtime_paths().namespace == RuntimeNamespace::Ork3
}

pub(crate) fn runtime_product_name() -> &'static str {
    runtime_paths().namespace.product_name()
}

pub(crate) fn apply_runtime_namespace_env(command: &mut std::process::Command) {
    // Daemon and handoff children retain the parent's choice even if a new root appears.
    command.env(RUNTIME_NAMESPACE_ENV_VAR, runtime_product_name());
}

pub fn config_dir() -> PathBuf {
    runtime_paths().config
}

pub fn state_dir() -> PathBuf {
    runtime_paths().state
}

fn config_dir_for_name(app_name: &str) -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(dir).join(app_name);
    }
    platform_config_dir(app_name)
}

fn state_dir_for_name(app_name: &str) -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_STATE_HOME") {
        return PathBuf::from(dir).join(app_name);
    }
    platform_state_dir(app_name)
}

#[cfg(windows)]
fn platform_config_dir(app_name: &str) -> PathBuf {
    if let Ok(dir) = std::env::var("APPDATA") {
        return PathBuf::from(dir).join(app_name);
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        return PathBuf::from(profile)
            .join("AppData")
            .join("Roaming")
            .join(app_name);
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(format!(".config/{app_name}"));
    }
    std::env::temp_dir().join(app_name)
}

#[cfg(not(windows))]
fn platform_config_dir(app_name: &str) -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(format!(".config/{app_name}"))
    } else {
        std::env::temp_dir().join(app_name)
    }
}

#[cfg(windows)]
fn platform_state_dir(app_name: &str) -> PathBuf {
    if let Ok(dir) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(dir).join(app_name);
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        return PathBuf::from(profile)
            .join("AppData")
            .join("Local")
            .join(app_name);
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(format!(".local/state/{app_name}"));
    }
    std::env::temp_dir().join(format!("{app_name}-state"))
}

#[cfg(not(windows))]
fn platform_state_dir(app_name: &str) -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(format!(".local/state/{app_name}"))
    } else {
        std::env::temp_dir().join(format!("{app_name}-state"))
    }
}

fn read_optional_config(path: &Path) -> std::io::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

impl Config {
    pub fn load() -> LoadedConfig {
        let path = config_path();
        let content = match read_optional_config(&path) {
            Ok(Some(content)) => content,
            Ok(None) => {
                return LoadedConfig {
                    config: Self::default(),
                    diagnostics: Vec::new(),
                    invalid_sections: Vec::new(),
                };
            }
            Err(err) => {
                warn!(err = %err, "config read error, using defaults");
                return LoadedConfig {
                    config: Self::default(),
                    diagnostics: vec![format!("config read error: {err}; using defaults")],
                    invalid_sections: Vec::new(),
                };
            }
        };

        match toml::from_str::<Config>(&content) {
            Ok(config) => {
                let mut diagnostics = unknown_top_level_section_diagnostics_from_str(&content);
                diagnostics.extend(config.collect_diagnostics());
                LoadedConfig {
                    config,
                    diagnostics,
                    invalid_sections: Vec::new(),
                }
            }
            Err(err) => {
                warn!(err = %err, "config parse error, using defaults");
                LoadedConfig {
                    config: Self::default(),
                    diagnostics: vec![format!("config parse error: {err}; using defaults")],
                    invalid_sections: Vec::new(),
                }
            }
        }
    }
}

pub(crate) fn resolve_config_relative_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }

    config_path()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(path)
}

pub fn config_path() -> PathBuf {
    if let Some(path) = config_path_override(
        std::env::var_os(CONFIG_PATH_ENV_VAR),
        std::env::var_os(LEGACY_CONFIG_PATH_ENV_VAR),
    ) {
        return PathBuf::from(path);
    }
    config_dir().join("config.toml")
}

fn config_path_override(
    current: Option<std::ffi::OsString>,
    legacy: Option<std::ffi::OsString>,
) -> Option<std::ffi::OsString> {
    current.or(legacy)
}

pub fn config_diagnostic_summary(diagnostics: &[String]) -> Option<String> {
    if diagnostics.is_empty() {
        return None;
    }

    let target = config_path()
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.toml")
        .to_string();
    let read_error = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.starts_with("config read error:"));
    let impact = if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("using defaults"))
    {
        if read_error {
            " unreadable; using defaults"
        } else {
            " invalid; using defaults"
        }
    } else if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("keeping current config"))
    {
        if read_error {
            " unreadable; keeping current config"
        } else {
            " invalid; keeping current config"
        }
    } else {
        ""
    };

    Some(format!("{target}{impact}; herduck config check"))
}

pub fn load_live_config() -> Result<LoadedConfig, Vec<String>> {
    let path = config_path();
    let content = match read_optional_config(&path) {
        Ok(Some(content)) => content,
        Ok(None) => {
            return Ok(LoadedConfig {
                config: Config::default(),
                diagnostics: Vec::new(),
                invalid_sections: Vec::new(),
            });
        }
        Err(err) => {
            return Err(vec![format!(
                "config read error: {err}; keeping current config"
            )]);
        }
    };
    load_live_config_from_str(&content)
}

fn load_live_config_from_str(content: &str) -> Result<LoadedConfig, Vec<String>> {
    let value = content
        .parse::<toml::Value>()
        .map_err(|err| vec![format!("config parse error: {err}; keeping current config")])?;
    let table = value.as_table().ok_or_else(|| {
        vec![
            "config parse error: top-level config must be a table; keeping current config"
                .to_string(),
        ]
    })?;

    let mut config = Config::default();
    let mut diagnostics = unknown_top_level_section_diagnostics(table);
    let mut invalid_sections = Vec::new();

    if let Some(value) = table.get("onboarding") {
        match value.clone().try_into::<Option<bool>>() {
            Ok(onboarding) => config.onboarding = onboarding,
            Err(err) => diagnostics.push(format!(
                "invalid onboarding setting: {err}; keeping current onboarding state"
            )),
        }
    }

    load_live_section(
        table,
        "theme",
        "theme config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.theme = section,
    );
    load_live_section(
        table,
        "keys",
        "keybinding config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.keys = section,
    );
    load_live_section(
        table,
        "terminal",
        "terminal config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.terminal = section,
    );
    load_live_section(
        table,
        "session",
        "session config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.session = section,
    );
    load_live_section(
        table,
        "update",
        "update config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.update = section,
    );
    load_live_section(
        table,
        "ui",
        "ui config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.ui = section,
    );
    load_live_section(
        table,
        "advanced",
        "advanced config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.advanced = section,
    );
    load_live_section(
        table,
        "worktrees",
        "worktree config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.worktrees = section,
    );
    load_live_section(
        table,
        "projects",
        "Projects adapter config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.projects = section,
    );
    load_live_section(
        table,
        "experimental",
        "experimental config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.experimental = section,
    );
    load_live_section(
        table,
        "remote",
        "remote config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.remote = section,
    );

    Ok(LoadedConfig {
        config,
        diagnostics,
        invalid_sections,
    })
}

fn unknown_top_level_section_diagnostics_from_str(content: &str) -> Vec<String> {
    content
        .parse::<toml::Value>()
        .ok()
        .and_then(|value| value.as_table().map(unknown_top_level_section_diagnostics))
        .unwrap_or_default()
}

fn unknown_top_level_section_diagnostics(
    table: &toml::map::Map<String, toml::Value>,
) -> Vec<String> {
    table
        .iter()
        .filter_map(|(key, value)| unknown_top_level_section_diagnostic(key, value))
        .collect()
}

fn unknown_top_level_section_diagnostic(key: &str, value: &toml::Value) -> Option<String> {
    if KNOWN_TOP_LEVEL_CONFIG_KEYS.contains(&key) {
        return None;
    }

    let header = if value.is_table() {
        format!("[{key}]")
    } else if value
        .as_array()
        .is_some_and(|items| !items.is_empty() && items.iter().all(toml::Value::is_table))
    {
        format!("[[{key}]]")
    } else {
        return None;
    };

    if key == "toast" {
        Some(format!(
            "unknown config section {header}; did you mean [ui.toast]? ignoring section"
        ))
    } else {
        Some(format!("unknown config section {header}; ignoring section"))
    }
}

fn load_live_section<T>(
    table: &toml::map::Map<String, toml::Value>,
    section: &'static str,
    label: &str,
    diagnostics: &mut Vec<String>,
    invalid_sections: &mut Vec<String>,
    apply: impl FnOnce(T),
) where
    T: serde::de::DeserializeOwned,
{
    let Some(value) = table.get(section) else {
        return;
    };

    match value.clone().try_into::<T>() {
        Ok(section_config) => apply(section_config),
        Err(err) => {
            diagnostics.push(format!(
                "invalid {label}: {err}; keeping current {section} settings"
            ));
            invalid_sections.push(section.to_string());
        }
    }
}

pub(crate) fn upsert_top_level_bool(content: &str, key: &str, value: bool) -> String {
    let replacement = format!("{key} = {value}");
    let mut lines: Vec<String> = content.lines().map(|line| line.to_string()).collect();
    let mut in_section = false;

    for line in &mut lines {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_section = true;
            continue;
        }
        if in_section {
            continue;
        }
        if trimmed.starts_with(&format!("{key} ")) || trimmed.starts_with(&format!("{key}=")) {
            *line = replacement.clone();
            return lines.join("\n") + "\n";
        }
    }

    if lines.is_empty() {
        format!("{replacement}\n")
    } else {
        format!("{replacement}\n{}\n", lines.join("\n").trim_end())
    }
}

/// Write a key = value pair in a TOML section (creates section if missing).
pub fn upsert_section_value(content: &str, section: &str, key: &str, value: &str) -> String {
    upsert_section_raw(content, section, key, value)
}

pub fn upsert_section_bool(content: &str, section: &str, key: &str, value: bool) -> String {
    upsert_section_raw(content, section, key, &value.to_string())
}

pub fn remove_section_key(content: &str, section: &str, key: &str) -> String {
    let header = format!("[{section}]");
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut i = 0;
    let mut in_section = false;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_section = trimmed == header;
            result.push(line.to_string());
            i += 1;
            continue;
        }

        if in_section
            && (trimmed.starts_with(&format!("{key} ")) || trimmed.starts_with(&format!("{key}=")))
        {
            i += 1;
            continue;
        }

        result.push(line.to_string());
        i += 1;
    }

    result.join("\n") + "\n"
}

pub fn remove_keybinding_config_sections(content: &str) -> (String, bool) {
    let mut result = Vec::new();
    let mut removed = false;
    let mut skipping_key_section = false;
    let mut in_table = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if let Some(table_name) = toml_table_header_name(trimmed) {
            in_table = true;
            skipping_key_section = is_keys_table_name(table_name);
            if skipping_key_section {
                removed = true;
                continue;
            }
        } else if skipping_key_section || (!in_table && is_top_level_keys_assignment(trimmed)) {
            removed = true;
            continue;
        }

        result.push(line.to_string());
    }

    let mut updated = result.join("\n");
    if content.ends_with('\n') || !updated.is_empty() {
        updated.push('\n');
    }
    (updated, removed)
}

fn toml_table_header_name(trimmed: &str) -> Option<&str> {
    if let Some(name) = trimmed
        .strip_prefix("[[")
        .and_then(|value| value.strip_suffix("]]"))
    {
        return Some(name.trim());
    }
    trimmed
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .map(str::trim)
}

fn is_keys_table_name(name: &str) -> bool {
    name == "keys" || name.starts_with("keys.")
}

fn is_top_level_keys_assignment(trimmed: &str) -> bool {
    trimmed.starts_with("keys ") || trimmed.starts_with("keys=") || trimmed.starts_with("keys.")
}

fn upsert_section_raw(content: &str, section: &str, key: &str, value: &str) -> String {
    let header = format!("[{section}]");
    let assignment = format!("{key} = {value}");
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut i = 0;
    let mut found_section = false;
    let mut inserted = false;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed == header {
            found_section = true;
            result.push(line.to_string());
            i += 1;

            while i < lines.len() {
                let current = lines[i];
                let current_trimmed = current.trim();
                if current_trimmed.starts_with('[') && current_trimmed.ends_with(']') {
                    if !inserted {
                        result.push(assignment.clone());
                        inserted = true;
                    }
                    break;
                }

                if current_trimmed.starts_with(&format!("{key} "))
                    || current_trimmed.starts_with(&format!("{key}="))
                {
                    result.push(assignment.clone());
                    inserted = true;
                } else {
                    result.push(current.to_string());
                }
                i += 1;
            }

            continue;
        }

        result.push(line.to_string());
        i += 1;
    }

    if !found_section {
        if !result.is_empty() && !result.last().is_some_and(|line| line.trim().is_empty()) {
            result.push(String::new());
        }
        result.push(header);
        result.push(assignment);
    } else if !inserted {
        result.push(assignment);
    }

    result.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_roots_and_sockets_select_one_namespace_without_moving_legacy_data() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let base = std::env::temp_dir().join(format!("herduck-namespace-{}", std::process::id()));
        let config_home = base.join("config");
        let state_home = base.join("state");
        let _ = std::fs::remove_dir_all(&base);
        let variables = [
            "XDG_CONFIG_HOME",
            "XDG_STATE_HOME",
            RUNTIME_NAMESPACE_ENV_VAR,
        ];
        let saved: Vec<_> = variables
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect();
        std::env::set_var("XDG_CONFIG_HOME", &config_home);
        std::env::set_var("XDG_STATE_HOME", &state_home);
        std::env::remove_var(RUNTIME_NAMESPACE_ENV_VAR);

        let assert_namespace = |namespace: RuntimeNamespace| {
            let config = config_home.join(namespace.dir_name());
            assert_eq!(config_dir(), config);
            assert_eq!(state_dir(), state_home.join(namespace.dir_name()));
            assert_eq!(uses_legacy_namespace(), namespace == RuntimeNamespace::Ork3);
            assert_eq!(
                crate::session::api_socket_path_for(Some("work")),
                config
                    .join("sessions/work")
                    .join(format!("{}.sock", namespace.product_name()))
            );
            assert_eq!(
                crate::session::client_socket_path_for(Some("work")),
                config
                    .join("sessions/work")
                    .join(format!("{}-client.sock", namespace.product_name()))
            );
        };
        assert_namespace(RuntimeNamespace::Herduck);
        let legacy = config_home.join(RuntimeNamespace::Ork3.dir_name());
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("session.json"), "legacy-session").unwrap();
        assert_namespace(RuntimeNamespace::Ork3);
        assert!(!config_home.join(app_dir_name()).exists());
        std::fs::create_dir_all(config_home.join(app_dir_name())).unwrap();
        assert_namespace(RuntimeNamespace::Herduck);
        assert_eq!(
            std::fs::read_to_string(legacy.join("session.json")).unwrap(),
            "legacy-session"
        );

        // A daemon/handoff launched from the old runtime retains that namespace when both exist.
        std::env::set_var(RUNTIME_NAMESPACE_ENV_VAR, "ork3");
        assert_namespace(RuntimeNamespace::Ork3);
        for (key, value) in saved {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn runtime_namespace_cache_preserves_roots_after_new_directory_appears() {
        let paths = std::sync::OnceLock::new();
        let candidate = |current_exists| {
            let namespace = RuntimeNamespace::select(current_exists, true);
            RuntimePaths {
                namespace,
                config: Path::new("config").join(namespace.dir_name()),
                state: Path::new("state").join(namespace.dir_name()),
            }
        };
        let first = cached_runtime_paths(&paths, || candidate(false));
        let later = cached_runtime_paths(&paths, || candidate(true));
        assert_eq!(later.namespace, RuntimeNamespace::Ork3);
        assert_eq!(later.config, first.config);
        assert_eq!(later.state, first.state);
        assert_eq!(later.namespace.product_name(), "ork3");
    }

    #[test]
    fn config_override_prefers_herduck_and_accepts_ork3() {
        use std::ffi::OsString;
        let current = Some(OsString::from("new-config.toml"));
        let legacy = Some(OsString::from("old-config.toml"));
        assert_eq!(
            config_path_override(current.clone(), legacy.clone()),
            current
        );
        assert_eq!(config_path_override(None, legacy.clone()), legacy);
        assert_eq!(config_path_override(None, None), None);
    }

    #[test]
    fn upsert_top_level_bool_replaces_existing_value() {
        let content = "onboarding = true\n[keys]\nprefix = \"ctrl+b\"\n";
        let updated = upsert_top_level_bool(content, "onboarding", false);
        assert!(updated.contains("onboarding = false"));
        assert!(!updated.contains("onboarding = true"));
    }

    #[test]
    fn upsert_section_bool_adds_missing_section() {
        let updated = upsert_section_bool("", "ui.toast", "enabled", true);
        assert!(updated.contains("[ui.toast]"));
        assert!(updated.contains("enabled = true"));
    }

    #[test]
    fn remove_section_key_removes_matching_key_from_section() {
        let content =
            "[ui.toast]\nenabled = true\ndelivery = \"herdr\"\n[ui.sound]\nenabled = true\n";
        let updated = remove_section_key(content, "ui.toast", "enabled");
        assert!(!updated.contains("[ui.toast]\nenabled = true"));
        assert!(updated.contains("delivery = \"herdr\""));
        assert!(updated.contains("[ui.sound]\nenabled = true"));
    }

    #[test]
    fn config_diagnostic_summary_uses_compact_actionable_banner() {
        let diagnostics = vec![
            "one".to_string(),
            "two".to_string(),
            "three".to_string(),
            "four".to_string(),
            "five".to_string(),
        ];

        assert_eq!(
            config_diagnostic_summary(&diagnostics).as_deref(),
            Some("config.toml; herduck config check")
        );
    }

    #[test]
    fn config_diagnostic_summary_reports_default_fallback() {
        let diagnostics = vec![
            "config parse error: TOML parse error at line 33, column 8\n   |\n33 | type = \"popup\"\n   |        ^^^^^^^\nunknown variant `popup`; using defaults"
                .to_string(),
        ];

        assert_eq!(
            config_diagnostic_summary(&diagnostics).as_deref(),
            Some("config.toml invalid; using defaults; herduck config check")
        );
    }

    #[test]
    fn config_diagnostic_summary_reports_unreadable_config_impact() {
        let startup = vec!["config read error: permission denied; using defaults".to_string()];
        assert_eq!(
            config_diagnostic_summary(&startup).as_deref(),
            Some("config.toml unreadable; using defaults; herduck config check")
        );

        let reload =
            vec!["config read error: permission denied; keeping current config".to_string()];
        assert_eq!(
            config_diagnostic_summary(&reload).as_deref(),
            Some("config.toml unreadable; keeping current config; herduck config check")
        );
    }

    #[test]
    fn config_diagnostic_summary_reports_retained_live_config() {
        let diagnostics = vec![
            "config parse error: TOML parse error at line 7, column 4; keeping current config"
                .to_string(),
        ];

        assert_eq!(
            config_diagnostic_summary(&diagnostics).as_deref(),
            Some("config.toml invalid; keeping current config; herduck config check")
        );
    }

    #[test]
    fn config_loaders_report_unreadable_path() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let path =
            std::env::temp_dir().join(format!("herdr-config-unreadable-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        std::env::set_var(CONFIG_PATH_ENV_VAR, &path);

        let startup = Config::load();
        assert!(startup
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("config read error")
                && diagnostic.contains("using defaults")));

        let reload = load_live_config().unwrap_err();
        assert!(reload.iter().any(|diagnostic| {
            diagnostic.contains("config read error")
                && diagnostic.contains("keeping current config")
        }));

        std::env::remove_var(CONFIG_PATH_ENV_VAR);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn load_live_config_parses_session_section() {
        let loaded = load_live_config_from_str(
            r#"
[session]
resume_agents_on_restore = true
agent_idle_timeout_secs = 1800
"#,
        )
        .unwrap();

        assert!(loaded.config.session.resume_agents_on_restore);
        assert_eq!(loaded.config.session.agent_idle_timeout_secs, 1800);
        assert!(loaded.diagnostics.is_empty());
        assert!(loaded.invalid_sections.is_empty());
    }

    #[test]
    fn load_live_config_warns_about_unknown_top_level_sections() {
        let loaded = load_live_config_from_str(
            r#"
[toast]
delivery = "system"

[ui.toast]
delivery = "herdr"
"#,
        )
        .unwrap();

        assert_eq!(
            loaded.diagnostics,
            vec!["unknown config section [toast]; did you mean [ui.toast]? ignoring section"]
        );
        assert!(loaded.invalid_sections.is_empty());
        assert_eq!(
            loaded.config.ui.toast.delivery,
            super::super::ToastDelivery::Herduck
        );
    }

    #[test]
    fn load_live_config_does_not_warn_about_unknown_top_level_scalar_values() {
        let loaded = load_live_config_from_str(
            r#"
plugin = []

[ui.toast]
delivery = "herdr"
"#,
        )
        .unwrap();

        assert!(loaded.diagnostics.is_empty());
        assert_eq!(
            loaded.config.ui.toast.delivery,
            super::super::ToastDelivery::Herduck
        );
    }

    #[test]
    fn startup_config_load_warns_about_unknown_top_level_sections() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let path = std::env::temp_dir().join(format!(
            "herdr-config-unknown-section-{}.toml",
            std::process::id()
        ));
        std::fs::write(
            &path,
            r#"
[[plugin]]
id = "example"

[ui.toast]
delivery = "system"
"#,
        )
        .unwrap();
        std::env::set_var(CONFIG_PATH_ENV_VAR, &path);

        let loaded = Config::load();

        assert_eq!(
            loaded.diagnostics,
            vec!["unknown config section [[plugin]]; ignoring section"]
        );
        assert_eq!(
            loaded.config.ui.toast.delivery,
            super::super::ToastDelivery::System
        );

        std::env::remove_var(CONFIG_PATH_ENV_VAR);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn remove_keybinding_config_sections_removes_keys_tables_only() {
        let content = r#"onboarding = false

[theme]
name = "catppuccin"

[keys]
prefix = "ctrl+a"
new_tab = "c"

[[keys.command]]
key = "g"
command = "lazygit"

[keys.indexed]
tabs = "ctrl"

[ui]
mouse_capture = false
"#;

        let (updated, removed) = remove_keybinding_config_sections(content);

        assert!(removed);
        assert!(updated.contains("onboarding = false"));
        assert!(updated.contains("[theme]\nname = \"catppuccin\""));
        assert!(updated.contains("[ui]\nmouse_capture = false"));
        assert!(!updated.contains("[keys]"));
        assert!(!updated.contains("[[keys.command]]"));
        assert!(!updated.contains("[keys.indexed]"));
        assert!(toml::from_str::<toml::Value>(&updated).is_ok());
    }

    #[test]
    fn remove_keybinding_config_sections_reports_noop_without_keys() {
        let content = "[ui]\nmouse_capture = true\n";
        let (updated, removed) = remove_keybinding_config_sections(content);
        assert!(!removed);
        assert_eq!(updated, content);
    }
}
