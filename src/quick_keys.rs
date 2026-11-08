//! Client presentation preferences for the one-line terminal dock.
use crossterm::event::{KeyCode, KeyModifiers};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use crate::input::TerminalKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ItemKind {
    Key,
    Chord,
    Text,
    Sequence,
    Modifier,
    AppAction,
}

impl ItemKind {
    pub const ALL: [Self; 6] = [
        Self::Key,
        Self::Chord,
        Self::Text,
        Self::Sequence,
        Self::Modifier,
        Self::AppAction,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Key => "单键",
            Self::Chord => "组合键",
            Self::Text => "文本",
            Self::Sequence => "按键序列",
            Self::Modifier => "修饰键",
            Self::AppAction => "应用动作",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            Self::Key => "esc / tab / up / home / f1",
            Self::Chord => "ctrl+c / alt+b / cmd+s",
            Self::Text => "原样输入文本；可勾选追加回车",
            Self::Sequence => "ctrl+b, %, enter（逗号分隔；逗号键用 comma）",
            Self::Modifier => "ctrl / alt / shift / cmd（单击一次，双击锁定）",
            Self::AppAction => "copy / paste / input_source / herduck / close",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct QuickKey {
    pub label: String,
    pub kind: ItemKind,
    pub payload: String,
    #[serde(default)]
    pub auto_enter: bool,
}

impl QuickKey {
    pub fn new(label: &str, kind: ItemKind, payload: &str) -> Self {
        Self {
            label: label.into(),
            kind,
            payload: payload.into(),
            auto_enter: false,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.label.trim().is_empty()
            || self.label.chars().count() > 32
            || self.label.chars().any(char::is_control)
        {
            return Err("名称需为 1–32 个可见字符".into());
        }
        if self.payload.len() > 8192 {
            return Err("内容过长（最多 8192 字节）".into());
        }
        match self.kind {
            ItemKind::Key | ItemKind::Chord => {
                self.keys()?;
            }
            ItemKind::Sequence => {
                self.keys()?;
            }
            ItemKind::Modifier => {
                if modifier(&self.payload).is_none() {
                    return Err("修饰键请选择 ctrl / alt / shift / cmd".into());
                }
            }
            ItemKind::AppAction
                if !matches!(
                    self.payload.as_str(),
                    "copy" | "paste" | "input_source" | "herduck" | "close"
                ) =>
            {
                return Err("未知应用动作".into())
            }
            ItemKind::Text if self.payload.is_empty() => return Err("请输入文本".into()),
            _ => {}
        }
        Ok(())
    }
    pub fn keys(&self) -> Result<Vec<TerminalKey>, String> {
        let parts: Vec<_> = if self.kind == ItemKind::Sequence {
            self.payload.split(',').collect()
        } else {
            vec![self.payload.as_str()]
        };
        if parts.len() > 32 {
            return Err("序列最多 32 步".into());
        }
        parts
            .into_iter()
            .map(|part| parse_key(part.trim()).ok_or_else(|| format!("无法识别按键：{part}")))
            .collect()
    }
}

pub(crate) fn parse_key(value: &str) -> Option<TerminalKey> {
    // Keep the shared keybinding grammar, adding navigation names it doesn't need.
    if let Some((prefix, base)) = value.rsplit_once('+') {
        if let Some(mut key) = parse_key(base) {
            for part in prefix.split('+') {
                key.modifiers |= modifier(part)?;
            }
            return Some(key);
        }
    }
    let code = match value.to_ascii_lowercase().as_str() {
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "pgup" | "pageup" => Some(KeyCode::PageUp),
        "pgdn" | "pagedown" => Some(KeyCode::PageDown),
        "delete" | "del" => Some(KeyCode::Delete),
        "insert" => Some(KeyCode::Insert),
        _ => None,
    };
    if let Some(code) = code {
        return Some(TerminalKey::new(code, KeyModifiers::empty()));
    }
    let (code, modifiers) = crate::config::parse_key_combo(value)?;
    if matches!(code, KeyCode::F(n) if !(1..=12).contains(&n)) {
        return None;
    }
    Some(TerminalKey::new(code, modifiers))
}

pub(crate) fn modifier(value: &str) -> Option<KeyModifiers> {
    match value.trim().to_ascii_lowercase().as_str() {
        "ctrl" | "control" => Some(KeyModifiers::CONTROL),
        "alt" | "opt" => Some(KeyModifiers::ALT),
        "shift" => Some(KeyModifiers::SHIFT),
        "cmd" | "super" => Some(KeyModifiers::SUPER),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Configuration {
    pub version: u8,
    pub items: Vec<QuickKey>,
}

impl Default for Configuration {
    fn default() -> Self {
        use ItemKind::*;
        let mut items = vec![
            QuickKey::new("Esc", Key, "esc"),
            QuickKey::new("Shift", Modifier, "shift"),
            QuickKey::new("中/英", AppAction, "input_source"),
            QuickKey::new("Herduck", AppAction, "herduck"),
            QuickKey::new("继续", Text, "继续"),
            QuickKey::new("Ctrl C", Chord, "ctrl+c"),
            QuickKey::new("Cmd C", AppAction, "copy"),
            QuickKey::new("Cmd V", AppAction, "paste"),
            QuickKey::new("关闭", AppAction, "close"),
        ];
        items[4].auto_enter = true;
        items.extend(
            [("↑", "up"), ("↓", "down"), ("←", "left"), ("→", "right")]
                .map(|(label, key)| QuickKey::new(label, Key, key)),
        );
        Self { version: 1, items }
    }
}

impl Configuration {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("不支持的快捷栏配置版本".into());
        }
        if self.items.len() > 64 {
            return Err("最多可添加 64 个快捷键".into());
        }
        for item in &self.items {
            item.validate()?;
        }
        Ok(())
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let data = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        let parent = path.parent().ok_or("配置路径无效")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&temporary, data).map_err(|e| e.to_string())?;
        std::fs::rename(&temporary, path).map_err(|e| e.to_string())
    }
}

pub(crate) struct Editor {
    pub draft: Configuration,
    pub selected: usize,
    pub field: usize,
    pub error: String,
}
impl Editor {
    pub fn item(&self) -> Option<&QuickKey> {
        self.draft.items.get(self.selected)
    }
    pub fn insert(&mut self, text: &str) {
        let field = self.field;
        if let Some(item) = self.draft.items.get_mut(self.selected) {
            let target = match field {
                0 => &mut item.label,
                2 => &mut item.payload,
                _ => return,
            };
            let clean: String = text
                .chars()
                .filter(|c| !c.is_control() || *c == '\n')
                .collect();
            if target.len() + clean.len() <= 8192 {
                target.push_str(&clean);
            }
        }
    }
}

pub(crate) struct QuickKeysState {
    pub config: Configuration,
    pub offset: u16,
    pub collapsed: bool,
    pub rect: ratatui::layout::Rect,
    pub editor: Option<Editor>,
    pub path: Option<PathBuf>,
    pub modifiers: KeyModifiers,
    pub locked: KeyModifiers,
    pub modified_keys: std::collections::HashMap<KeyCode, TerminalKey>,
    pub last_modifier: Option<(KeyModifiers, Instant)>,
    pub press: Option<(u16, u16, bool)>,
    pub activated: Option<(usize, Instant)>,
    pub notice: Option<(String, Instant)>,
}
impl Default for QuickKeysState {
    fn default() -> Self {
        Self {
            config: Configuration::default(),
            offset: 0,
            collapsed: false,
            rect: ratatui::layout::Rect::default(),
            editor: None,
            path: None,
            modifiers: KeyModifiers::empty(),
            locked: KeyModifiers::empty(),
            modified_keys: std::collections::HashMap::new(),
            last_modifier: None,
            press: None,
            activated: None,
            notice: None,
        }
    }
}
impl QuickKeysState {
    pub fn toggle_collapsed(&mut self) {
        self.collapsed = !self.collapsed;
        self.press = None;
        self.activated = None;
        if self.collapsed {
            self.modifiers = KeyModifiers::empty();
            self.locked = KeyModifiers::empty();
            self.last_modifier = None;
            // Already pressed keys must retain their matching release modifiers.
        }
    }

    pub fn load() -> Self {
        let path = crate::config::config_path().with_file_name("quick-keys.json");
        let mut state = Self {
            path: Some(path.clone()),
            ..Self::default()
        };
        match std::fs::read(&path) {
            Ok(data) => match serde_json::from_slice::<Configuration>(&data)
                .map_err(|e| e.to_string())
                .and_then(|config| {
                    config.validate()?;
                    Ok(config)
                }) {
                Ok(config) => state.config = config,
                Err(e) => {
                    tracing::warn!("invalid quick keys configuration: {e}");
                    state.notify(format!("快捷栏配置未载入：{e}"));
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                tracing::warn!("cannot read quick keys configuration: {e}");
                state.notify(format!("快捷栏配置未载入：{e}"));
            }
        }
        state
    }
    pub fn notify(&mut self, text: String) {
        self.notice = Some((text, Instant::now()));
    }
    pub fn notice_deadline(&self) -> Option<Instant> {
        self.notice
            .as_ref()
            .map(|(_, at)| *at + std::time::Duration::from_secs(5))
    }
    pub fn activation_deadline(&self) -> Option<Instant> {
        self.activated
            .map(|(_, at)| at + std::time::Duration::from_millis(160))
    }
    pub fn expire_activation(&mut self, now: Instant) -> bool {
        if self
            .activation_deadline()
            .is_some_and(|deadline| now >= deadline)
        {
            self.activated = None;
            true
        } else {
            false
        }
    }
    pub fn expire_notice(&mut self, now: Instant) -> bool {
        if self
            .notice_deadline()
            .is_some_and(|deadline| now >= deadline)
        {
            self.notice = None;
            true
        } else {
            false
        }
    }
    pub fn open_editor(&mut self) {
        self.editor = Some(Editor {
            draft: self.config.clone(),
            selected: 0,
            field: 0,
            error: String::new(),
        });
    }
    pub fn apply_modifiers(&mut self, mut key: TerminalKey) -> TerminalKey {
        use crossterm::event::KeyEventKind;
        if key.kind != KeyEventKind::Press {
            let modified = if key.kind == KeyEventKind::Release {
                self.modified_keys.remove(&key.code)
            } else {
                self.modified_keys.get(&key.code).copied()
            };
            return modified.map_or(key, |modified| TerminalKey {
                kind: key.kind,
                ..modified
            });
        }
        if matches!(key.code, KeyCode::Modifier(_)) {
            return key;
        }
        let original = key.code;
        key.modifiers |= self.modifiers;
        if key.modifiers.contains(KeyModifiers::SHIFT) {
            if let KeyCode::Char(c) = key.code {
                if c.is_ascii_lowercase() {
                    key.code = KeyCode::Char(c.to_ascii_uppercase());
                }
            }
        }
        if !self.modifiers.is_empty() {
            self.modified_keys.insert(original, key);
        }
        self.modifiers = self.locked;
        key
    }
    pub fn toggle_modifier(&mut self, modifier: KeyModifiers) {
        let double = self
            .last_modifier
            .is_some_and(|(previous, at)| previous == modifier && at.elapsed().as_millis() < 400);
        if double && self.modifiers.contains(modifier) && !self.locked.contains(modifier) {
            self.locked.insert(modifier);
        } else {
            self.locked.remove(modifier);
            self.modifiers.toggle(modifier);
        }
        self.last_modifier = Some((modifier, Instant::now()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quick_keys_config_roundtrip_and_reject_invalid_sequences() {
        let mut config = Configuration::default();
        config.items.reverse();
        config
            .items
            .push(QuickKey::new("分屏", ItemKind::Sequence, "ctrl+b, %"));
        config.validate().unwrap();
        let path =
            std::env::temp_dir().join(format!("herduck-quick-keys-{}.json", std::process::id()));
        config.save(&path).unwrap();
        let restored: Configuration =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(restored, config);
        std::fs::remove_file(path).unwrap();
        assert!(QuickKey::new("坏序列", ItemKind::Sequence, "ctrl+b, typo")
            .validate()
            .is_err());
        assert!(QuickKey::new("坏动作", ItemKind::AppAction, "quit_server")
            .validate()
            .is_err());
    }
    #[test]
    fn quick_keys_shift_once_and_double_tap_lock() {
        let mut state = QuickKeysState::default();
        state.toggle_modifier(KeyModifiers::SHIFT);
        assert_eq!(
            state
                .apply_modifiers(TerminalKey::new(KeyCode::Char('a'), KeyModifiers::empty()))
                .code,
            KeyCode::Char('A')
        );
        assert!(state.modifiers.is_empty());
        state.last_modifier = None;
        state.toggle_modifier(KeyModifiers::CONTROL);
        state.toggle_modifier(KeyModifiers::CONTROL);
        state.apply_modifiers(TerminalKey::new(KeyCode::Char('c'), KeyModifiers::empty()));
        assert!(state.modifiers.contains(KeyModifiers::CONTROL));
        state.toggle_modifier(KeyModifiers::CONTROL);
        assert!(state.modifiers.is_empty());
        state.modifiers = KeyModifiers::SHIFT;
        state.locked = KeyModifiers::SHIFT;
        state.apply_modifiers(TerminalKey::new(KeyCode::Char('a'), KeyModifiers::empty()));
        state.toggle_collapsed();
        assert!(state.modifiers.is_empty());
        assert!(state.locked.is_empty());
        let mut release = TerminalKey::new(KeyCode::Char('a'), KeyModifiers::empty());
        release.kind = crossterm::event::KeyEventKind::Release;
        let release = state.apply_modifiers(release);
        assert_eq!(release.code, KeyCode::Char('A'));
        assert!(release.modifiers.contains(KeyModifiers::SHIFT));
        assert_eq!(
            state
                .apply_modifiers(TerminalKey::new(KeyCode::Char('b'), KeyModifiers::empty()))
                .code,
            KeyCode::Char('b')
        );
    }
}
