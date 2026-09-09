use super::App;

const STARTER_CONFIG: &str = include_str!("../config/starter.toml");

/// The editor must never replace an existing file, even when its TOML is invalid.
fn prepare_config_file(path: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    use std::io::Write;

    let path = std::path::absolute(path)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut file) => file.write_all(STARTER_CONFIG.as_bytes())?,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if !path.is_file() {
                return Err(std::io::Error::other("config path is not a regular file"));
            }
        }
        Err(error) => return Err(error),
    }
    Ok(path)
}

impl App {
    pub(crate) fn open_settings_config_file(&mut self) {
        self.open_settings_config_file_with(crate::platform::open_text_file);
    }

    pub(super) fn open_settings_config_file_with(
        &mut self,
        open: impl FnOnce(&std::path::Path) -> std::io::Result<std::process::Child>,
    ) {
        self.poll_config_editor();
        if self.config_editor_child.is_some() {
            self.state.settings.status = "Editor is running. Save, then reopen Settings.".into();
            return;
        }
        let path = crate::config::config_path();
        self.state.settings.config_path = path.clone();
        let result = prepare_config_file(&path).and_then(|path| {
            self.state.settings.config_path = path.clone();
            open(&path)
        });
        match result {
            Ok(child) => {
                self.config_editor_child = Some(child);
                self.state.settings.status =
                    "Save the file, then reopen Settings to load changes.".into();
            }
            Err(error) => {
                tracing::warn!(path = %path.display(), %error, "could not open config editor");
                self.state.settings.status = format!("Cannot open config: {error}");
            }
        }
    }

    pub(crate) fn poll_config_editor(&mut self) {
        let Some(child) = self.config_editor_child.as_mut() else {
            return;
        };
        let error = match child.try_wait() {
            Ok(None) => return,
            Ok(Some(status)) if status.success() => None,
            Ok(Some(status)) => Some(format!(
                "Cannot open config: editor exited {status}. Open the displayed path manually."
            )),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => return,
            Err(error) => Some(format!("Cannot open config: {error}")),
        };
        self.config_editor_child = None;
        if let Some(error) = error {
            tracing::warn!(%error, "config editor failed");
            self.state.settings.status = error;
            self.render_dirty
                .store(true, std::sync::atomic::Ordering::Release);
        }
    }

    pub(super) fn update_config_file<F>(&mut self, error_context: &str, update: F) -> bool
    where
        F: FnOnce(&str) -> String,
    {
        self.update_config_file_checked(error_context, |content| Ok(update(content)))
    }

    pub(super) fn update_config_file_checked<F>(&mut self, error_context: &str, update: F) -> bool
    where
        F: FnOnce(&str) -> Result<String, String>,
    {
        #[cfg(test)]
        if std::env::var_os(crate::config::CONFIG_PATH_ENV_VAR).is_none() {
            return false;
        }

        let path = crate::config::config_path();
        if let Some(parent) = path.parent() {
            if let Err(err) = std::fs::create_dir_all(parent) {
                crate::logging::config_write_failed(&path, error_context, &err.to_string());
                self.state.config_diagnostic =
                    Some(format!("failed to save {error_context}: {err}"));
                self.config_diagnostic_deadline =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
                return false;
            }
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                STARTER_CONFIG.to_string()
            }
            Err(error) => {
                self.state.config_diagnostic =
                    Some(format!("failed to read configuration: {error}"));
                return false;
            }
        };
        let new_content = match update(&content) {
            Ok(content) => content,
            Err(error) => {
                self.state.config_diagnostic =
                    Some(format!("failed to save {error_context}: {error}"));
                return false;
            }
        };
        if let Err(err) = std::fs::write(&path, new_content) {
            crate::logging::config_write_failed(&path, error_context, &err.to_string());
            self.state.config_diagnostic = Some(format!("failed to save {error_context}: {err}"));
            self.config_diagnostic_deadline =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
            return false;
        }

        true
    }

    pub(super) fn mark_onboarding_complete(&mut self) {
        self.update_config_file("onboarding setting", |content| {
            crate::config::upsert_top_level_bool(content, "onboarding", false)
        });
    }

    pub(super) fn save_theme(&mut self, name: &str) {
        if self.update_config_file("theme", |content| {
            let content = crate::config::upsert_section_value(
                content,
                "theme",
                "name",
                &format!("\"{name}\""),
            );
            crate::config::upsert_section_bool(&content, "theme", "auto_switch", false)
        }) {
            self.apply_config_from_disk(false);
        }
    }

    pub(super) fn save_sound(&mut self, enabled: bool) {
        if self.update_config_file("sound setting", |content| {
            crate::config::upsert_section_bool(content, "ui.sound", "enabled", enabled)
        }) {
            self.apply_config_from_disk(false);
        }
    }

    pub(super) fn save_toast_delivery(&mut self, delivery: crate::config::ToastDelivery) {
        let value = match delivery {
            crate::config::ToastDelivery::Off => "\"off\"",
            crate::config::ToastDelivery::Herduck => "\"herduck\"",
            crate::config::ToastDelivery::Terminal => "\"terminal\"",
            crate::config::ToastDelivery::System => "\"system\"",
        };
        if self.update_config_file("toast setting", |content| {
            let content =
                crate::config::upsert_section_value(content, "ui.toast", "delivery", value);
            crate::config::remove_section_key(&content, "ui.toast", "enabled")
        }) {
            self.apply_config_from_disk(false);
        }
    }

    pub(super) fn save_agent_border_labels(&mut self, enabled: bool) {
        if self.update_config_file("agent border labels", |content| {
            crate::config::upsert_section_bool(
                content,
                "ui",
                "show_agent_labels_on_pane_borders",
                enabled,
            )
        }) {
            self.apply_config_from_disk(false);
        }
    }

    pub(super) fn save_pane_history_persistence(&mut self, enabled: bool) {
        if self.update_config_file("pane screen history", |content| {
            crate::config::upsert_section_bool(content, "experimental", "pane_history", enabled)
        }) {
            self.apply_config_from_disk(false);
        }
    }

    pub(super) fn save_switch_ascii_input_source_in_prefix(&mut self, enabled: bool) {
        if self.update_config_file("prefix ascii input source", |content| {
            crate::config::upsert_section_bool(
                content,
                "experimental",
                "switch_ascii_input_source_in_prefix",
                enabled,
            )
        }) {
            self.apply_config_from_disk(false);
        }
    }

    pub(super) fn save_agent_panel_sort(&mut self, sort: crate::app::state::AgentPanelSort) {
        let value = match sort {
            crate::app::state::AgentPanelSort::Spaces => {
                crate::config::AgentPanelSortConfig::Spaces.as_str()
            }
            crate::app::state::AgentPanelSort::Priority => {
                crate::config::AgentPanelSortConfig::Priority.as_str()
            }
        };
        if self.update_config_file("agent panel sort", |content| {
            crate::config::upsert_section_value(
                content,
                "ui",
                "agent_panel_sort",
                &format!("\"{value}\""),
            )
        }) {
            self.apply_config_from_disk(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_editor_resolves_relative_paths_and_never_overwrites_existing_content() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::path::PathBuf::from("target")
            .join(format!("config-editor-{}-{unique}", std::process::id()));
        let relative = root.join("folder's $(literal) space").join("config.toml");
        let path = prepare_config_file(&relative).unwrap();
        assert!(path.is_absolute());
        assert_eq!(path, std::path::absolute(&relative).unwrap());
        let config: crate::config::Config =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            config.projects.summary.mode,
            crate::config::SummaryModeConfig::Pending
        );
        assert!(config.projects.summary.providers.is_empty());
        assert!(config.projects.summary.title_providers.is_none());
        let invalid = "# preserve me\n[unfinished";
        std::fs::write(&path, invalid).unwrap();
        assert_eq!(prepare_config_file(&relative).unwrap(), path);
        assert_eq!(std::fs::read_to_string(path).unwrap(), invalid);
        assert!(prepare_config_file(&root).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
