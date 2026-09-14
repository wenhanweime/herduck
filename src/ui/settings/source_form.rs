//! Applied source priorities. All editing happens in config.toml.

use super::forms::SettingsForm;
use crate::{
    app::{state::SettingsSection, AppState},
    config::{SummaryModeConfig, SummaryProviderKind, TitleLanguage},
};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

pub(super) fn source_form(app: &AppState, width: u16) -> SettingsForm {
    let mut form = SettingsForm::new(width);
    let heading = Style::default()
        .fg(app.palette.text)
        .add_modifier(Modifier::BOLD);
    let value = Style::default().fg(app.palette.text);
    let text = Style::default().fg(app.palette.overlay1);
    let titles = app.settings.section == SettingsSection::Titles;
    let config = &app.summary_config;
    let inactive = matches!(
        config.mode,
        SummaryModeConfig::Pending | SummaryModeConfig::Local
    );
    form.field(
        "Generation",
        match config.mode {
            SummaryModeConfig::Pending => "Off",
            SummaryModeConfig::Local => "Offline names only",
            _ => "Ordered fallback",
        },
        app,
    );
    if titles {
        form.field(
            "Language",
            if config.title_language == TitleLanguage::Chinese {
                "中文"
            } else {
                "English"
            },
            app,
        );
        form.field(
            "Source order",
            if config.title_providers.is_none() {
                "Same as Summaries"
            } else {
                "Independent naming priorities"
            },
            app,
        );
    }
    form.section(
        if inactive {
            "Sources · inactive"
        } else {
            "Sources · tried top to bottom"
        },
        app,
    );
    if inactive {
        form.text("No Agent or API calls in this mode.", text);
    }
    // An explicit empty naming list is local-only; only None inherits summaries.
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
        if index > 0 {
            form.text("", text);
        }
        form.prefixed(
            Line::from(Span::styled(
                format!("{}. ", index + 1),
                Style::default().fg(app.palette.accent),
            )),
            Line::from(vec![
                Span::styled(provider.id.clone(), heading),
                Span::styled(if agent { "  Agent" } else { "  API" }, text),
            ]),
        );
        let models = if provider.models.is_empty() {
            vec![if agent {
                "Agent default"
            } else {
                "not configured"
            }]
        } else {
            provider.models.iter().map(String::as_str).collect()
        };
        for (model_index, model) in models.iter().enumerate() {
            form.prefixed(
                Line::from(Span::styled(
                    if model_index + 1 == models.len() {
                        "   └─ "
                    } else {
                        "   ├─ "
                    },
                    text,
                )),
                Line::from(Span::styled((*model).to_owned(), value)),
            );
        }
        let mut detail = |label: &str, detail: String| {
            form.prefixed(
                Line::from(Span::styled(format!("   {label:<9}"), text)),
                Line::from(Span::styled(detail, text)),
            );
        };
        if agent {
            detail(
                "Command",
                provider
                    .command
                    .as_deref()
                    .unwrap_or(&provider.id)
                    .to_owned(),
            );
        } else {
            detail(
                "Endpoint",
                provider
                    .endpoint
                    .as_deref()
                    .unwrap_or("not configured")
                    .to_owned(),
            );
            detail(
                "Key env",
                provider
                    .api_key_env
                    .as_ref()
                    .map(|key| format!("${key}"))
                    .unwrap_or_else(|| "not required".into()),
            );
        }
    }
    if !inactive {
        form.text("First usable result wins.", text);
    }
    form.section(if inactive { "Result" } else { "Fallback" }, app);
    match config.mode {
        SummaryModeConfig::Pending => form.text(
            "Existing Work groups and session names stay unchanged.",
            text,
        ),
        SummaryModeConfig::Local => {
            form.text("Names use local session text.", value);
            form.text("Existing Work groups stay unchanged.", text);
        }
        _ => {
            form.text("If every source fails", value);
            form.text(
                if titles {
                    "Use local session text for names."
                } else {
                    "Keep Work groups · use local text for names."
                },
                text,
            );
        }
    }
    if titles {
        form.text("Manual names are always kept.", text);
    }
    form
}
