//! Build identity helpers.

/// Numeric release base used by the inherited update comparison code.
pub const BASE_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION_MAJOR"),
    ".",
    env!("CARGO_PKG_VERSION_MINOR"),
    ".",
    env!("CARGO_PKG_VERSION_PATCH")
);

/// Canonical product name used by commands, data directories, sockets, and logs.
pub const PRODUCT_NAME: &str = "herduck";

pub fn channel() -> &'static str {
    non_empty(option_env!("HERDUCK_BUILD_CHANNEL"))
        .or_else(|| non_empty(option_env!("HERDR_BUILD_CHANNEL")))
        .unwrap_or("stable")
}

pub fn build_id() -> Option<&'static str> {
    non_empty(option_env!("HERDUCK_BUILD_ID")).or_else(|| non_empty(option_env!("HERDR_BUILD_ID")))
}

pub fn version() -> String {
    match channel() {
        "stable" => env!("CARGO_PKG_VERSION").to_string(),
        channel => match build_id() {
            Some(build_id) => format!("{BASE_VERSION}-{channel}.{build_id}"),
            None => format!("{BASE_VERSION}-{channel}"),
        },
    }
}

pub fn is_preview() -> bool {
    channel() == "preview"
}

fn non_empty(value: Option<&'static str>) -> Option<&'static str> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn stable_version_defaults_to_cargo_version() {
        assert!(crate::update::Version::parse(super::BASE_VERSION).is_some());
        if super::channel() == "stable" {
            assert_eq!(super::version(), env!("CARGO_PKG_VERSION"));
        }
    }
}
