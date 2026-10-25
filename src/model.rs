use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_CAPTURE_BYTES: usize = 64 * 1024 * 1024;
pub const THUMBNAIL_WIDTH: u32 = 96;
pub const THUMBNAIL_HEIGHT: u32 = 64;
pub const MAX_THUMBNAIL_BYTES: usize = 32 * 1024;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormatData {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// All supported formats from one clipboard version, captured before storage I/O.
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct ClipContent {
    pub formats: Vec<FormatData>,
    pub text: String,
    pub files: Vec<String>,
    pub source: String,
}

impl ClipContent {
    pub fn text(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            formats: vec![FormatData {
                name: "text/plain".into(),
                bytes: text.as_bytes().to_vec(),
            }],
            text,
            ..Self::default()
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.formats.is_empty(), "剪贴板没有可保存的格式");
        ensure!(
            self.bytes() <= MAX_CAPTURE_BYTES as u64,
            "单次复制超过 64 MB，未保存；请复制较小的内容"
        );
        let mut names: Vec<_> = self.formats.iter().map(|f| &f.name).collect();
        names.sort();
        ensure!(
            !names.windows(2).any(|p| p[0] == p[1]),
            "同次复制包含重复格式名称"
        );
        Ok(())
    }
    pub fn bytes(&self) -> u64 {
        self.formats.iter().map(|f| f.bytes.len() as u64).sum()
    }
    pub fn fingerprint(&self) -> String {
        let mut formats: Vec<_> = self.formats.iter().collect();
        formats.sort_by(|a, b| a.name.cmp(&b.name));
        let mut digest = Sha256::new();
        for format in formats {
            digest.update((format.name.len() as u64).to_le_bytes());
            digest.update(format.name.as_bytes());
            digest.update((format.bytes.len() as u64).to_le_bytes());
            digest.update(&format.bytes);
        }
        format!("{:x}", digest.finalize())
    }
    pub fn kind(&self) -> &'static str {
        if !self.files.is_empty() {
            "file"
        } else if self.formats.iter().any(|f| {
            matches!(
                f.name.as_str(),
                "image/png" | "windows:8" | "windows:17" | "windows:PNG"
            )
        }) {
            "image"
        } else if self.formats.iter().any(|f| {
            f.name.contains("html")
                || f.name.contains("HTML")
                || f.name.contains("rtf")
                || f.name.contains("Rich Text")
        }) {
            "rich"
        } else {
            "text"
        }
    }
    pub fn title(&self) -> String {
        if let Some(file) = self.files.first() {
            let name = std::path::Path::new(file)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            return if self.files.len() > 1 {
                format!("{name} 等 {} 个项目", self.files.len())
            } else {
                name.into_owned()
            };
        }
        if !self.text.is_empty() {
            return self
                .text
                .chars()
                .take(100)
                .collect::<String>()
                .replace(['\r', '\n'], " ");
        }
        match self.kind() {
            "image" => "图片",
            "rich" => "富文本",
            _ => "剪贴板内容",
        }
        .into()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub fingerprint: String,
    pub title: String,
    pub text: String,
    pub kind: String,
    pub source: String,
    pub copied_at: i64,
    pub order: i64,
    pub pinned: bool,
    pub favorite: bool,
    pub bytes: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Settings {
    pub language: String,
    pub theme: String,
    pub shortcut: String,
    pub max_entries: usize,
    pub max_bytes: u64,
    pub paused: bool,
    pub start_at_login: bool,
    pub close_to_tray: bool,
    pub compact: bool,
    pub position: String,
    pub excluded_apps: Vec<String>,
    pub setup_complete: bool,
    pub guide_seen: bool,
    pub respect_sensitive: bool,
    pub recent_symbols: Vec<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "zh-CN".into(),
            theme: "system".into(),
            shortcut: if cfg!(windows) {
                "Win+V"
            } else if cfg!(target_os = "macos") {
                "Super+Shift+V"
            } else {
                "Ctrl+Alt+V"
            }
            .into(),
            max_entries: 10_000,
            max_bytes: 2 * 1024 * 1024 * 1024,
            paused: false,
            start_at_login: false,
            close_to_tray: true,
            compact: false,
            position: "cursor".into(),
            excluded_apps: vec![],
            setup_complete: false,
            guide_seen: false,
            respect_sensitive: true,
            recent_symbols: vec![],
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        ensure!(crate::i18n::supported(&self.language), "未知的界面语言");
        ensure!(
            (1..=1_000_000).contains(&self.max_entries),
            "保留条数须在 1 到 1,000,000 之间"
        );
        ensure!(self.max_bytes >= 1024 * 1024, "容量上限不能小于 1 MB");
        ensure!(
            ["light", "dark", "system"].contains(&self.theme.as_str())
                || crate::themes::THEME_NAMES.contains(&self.theme.as_str()),
            "未知的主题"
        );
        ensure!(
            ["cursor", "center"].contains(&self.position.as_str()),
            "未知的面板位置"
        );
        Ok(())
    }
}
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn language_defaults_for_old_settings_and_rejects_unknown_values() {
        let old: Settings = serde_json::from_str(r#"{"theme":"dark","guide_seen":true}"#).unwrap();
        assert_eq!(old.language, "zh-CN");
        assert!(old.guide_seen);
        let mut settings = old;
        for (language, _) in crate::i18n::LANGUAGES {
            settings.language = (*language).into();
            settings.validate().unwrap();
        }
        settings.language = "unknown".into();
        assert!(settings.validate().is_err());
    }
    #[test]
    fn dedup_is_format_sensitive_and_preserves_whitespace() {
        let plain = ClipContent::text("Hello ");
        assert_ne!(
            plain.fingerprint(),
            ClipContent::text("Hello").fingerprint()
        );
        assert_ne!(
            plain.fingerprint(),
            ClipContent::text("hello ").fingerprint()
        );
        let mut rich = plain.clone();
        rich.formats.push(FormatData {
            name: "text/html".into(),
            bytes: b"<b>Hello </b>".to_vec(),
        });
        assert_ne!(plain.fingerprint(), rich.fingerprint());
        let hash = rich.fingerprint();
        rich.formats.reverse();
        assert_eq!(hash, rich.fingerprint());
    }
}
