//! Applied source priorities. All editing happens in config.toml.

use super::forms::{config_location, SettingsForm};
use crate::{
    app::{state::SettingsSection, AppState},
    config::{SummaryModeConfig, SummaryProviderKind, TitleLanguage},
};
use ratatui::style::{Modifier, Style};

pub(super) fn source_form(app: &AppState, width: u16) -> SettingsForm {
    let mut form = SettingsForm::new(width);
    let heading = Style::default()
        .fg(app.palette.text)
        .add_modifier(Modifier::BOLD);
    let value = Style::default().fg(app.palette.text);
    let text = Style::default().fg(app.palette.overlay1);
    let titles = app.settings.section == SettingsSection::Titles;
    let config = &app.summary_config;
    form.text(
        if titles {
            "Session names · read only"
        } else {
            "Summary sources · read only"
        },
        heading,
    );
    form.text(
        format!(
            "Generation: {}",
            match config.mode {
                SummaryModeConfig::Pending => "Off",
                SummaryModeConfig::Local => "Offline names only",
                _ => "Ordered fallback",
            }
        ),
        value,
    );
    if titles {
        form.text(
            format!(
                "Language: {}",
                if config.title_language == TitleLanguage::Chinese {
                    "中文"
                } else {
                    "English"
                }
            ),
            value,
        );
        form.text(
            if config.title_providers.is_none() {
                "Sources: use Summary priorities (default)"
            } else {
                "Sources: independent naming priorities"
            },
            text,
        );
    }
    form.text(
        "Priority: top to bottom; stop at the first usable result.",
        text,
    );
    if config.mode == SummaryModeConfig::Pending {
        form.text("Generation is off. These sources are not being used.", text);
    } else if config.mode == SummaryModeConfig::Local {
        form.text("Offline names only. No model source is being used.", text);
    }
    form.text("", text);
    let explicit_names = titles.then_some(config.title_providers.as_ref()).flatten();
    let sources = explicit_names.or_else(|| {
        (config.providers_explicit
            || matches!(
                config.mode,
                SummaryModeConfig::Auto | SummaryModeConfig::Llm
            ))
        .then_some(&config.providers)
    });
    if sources.is_none_or(|sources| sources.is_empty()) {
        form.text("No model sources configured.", text);
    }
    for (index, provider) in sources.into_iter().flatten().enumerate() {
        let agent = provider.kind == SummaryProviderKind::Cli;
        form.text(
            format!(
                "{}. {} · {}",
                index + 1,
                if agent { "Agent" } else { "API" },
                provider.id
            ),
            heading,
        );
        form.text(
            format!(
                "   Models: {}",
                if provider.models.is_empty() {
                    if agent {
                        "Agent default".into()
                    } else {
                        "not configured".into()
                    }
                } else {
                    provider.models.join(" → ")
                }
            ),
            value,
        );
        if agent {
            form.text(
                format!(
                    "   Command: {}",
                    provider.command.as_deref().unwrap_or(&provider.id)
                ),
                text,
            );
        } else {
            form.text(
                format!(
                    "   URL: {}",
                    provider.endpoint.as_deref().unwrap_or("not configured")
                ),
                text,
            );
            form.text(
                format!(
                    "   API key: {}",
                    provider
                        .api_key_env
                        .as_ref()
                        .map(|key| format!("from ${key}"))
                        .unwrap_or_else(|| "not required".into())
                ),
                text,
            );
        }
        form.text("", text);
    }
    form.text(
        if titles {
            "Final fallback: a name from local session text. Manual names are kept."
        } else {
            "If all sources fail: keep existing topics; names use local session text."
        },
        text,
    );
    config_location(&mut form, app);
    form
}
