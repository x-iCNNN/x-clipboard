//! Offline UI messages. Clipboard payloads never pass through this module.
use std::{
    collections::BTreeMap,
    sync::{
        LazyLock,
        atomic::{AtomicUsize, Ordering},
    },
};

pub const LANGUAGES: &[(&str, &str)] = &[
    ("zh-CN", "简体中文"),
    ("zh-TW", "繁體中文"),
    ("en", "English"),
    ("ja", "日本語"),
    ("ko", "한국어"),
    ("es", "Español"),
    ("fr", "Français"),
    ("de", "Deutsch"),
    ("pt", "Português"),
];
static LOCALE: AtomicUsize = AtomicUsize::new(0);
static MESSAGES: LazyLock<BTreeMap<String, Vec<String>>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../assets/locales/ui.json"))
        .expect("bundled UI translations must be valid")
});

pub fn supported(locale: &str) -> bool {
    LANGUAGES.iter().any(|(id, _)| *id == locale)
}
pub fn set_locale(locale: &str) {
    LOCALE.store(
        LANGUAGES
            .iter()
            .position(|(id, _)| *id == locale)
            .unwrap_or(0),
        Ordering::Relaxed,
    );
}
pub fn language_name(locale: &str) -> &'static str {
    LANGUAGES
        .iter()
        .find(|(id, _)| *id == locale)
        .unwrap_or(&LANGUAGES[0])
        .1
}
fn translated(source: &str, index: usize) -> &str {
    if index == 0 {
        return source;
    }
    MESSAGES
        .get(source)
        .and_then(|values| values.get(index))
        .map(String::as_str)
        .unwrap_or(source)
}
pub fn tr(source: &str) -> &str {
    translated(source, LOCALE.load(Ordering::Relaxed))
}
/// Named placeholders allow each language to use its own word order.
pub fn message(source: &str, args: &[(&str, String)]) -> String {
    let template = tr(source);
    // Scan the template once so user text containing braces is never substituted again.
    let mut output = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        output.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('}').map(|end| start + end) else {
            output.push_str(&rest[start..]);
            return output;
        };
        let name = &rest[start + 1..end];
        if let Some((_, value)) = args.iter().find(|(key, _)| *key == name) {
            output.push_str(value);
        } else {
            output.push_str(&rest[start..=end]);
        }
        rest = &rest[end + 1..];
    }
    output.push_str(rest);
    output
}

#[cfg(test)]
mod tests {
    use super::{LANGUAGES, MESSAGES, translated};
    #[test]
    fn catalog_has_every_language_and_preserves_placeholders() {
        fn placeholders(value: &str) -> Vec<&str> {
            let mut result = vec![];
            let mut rest = value;
            while let Some(start) = rest.find('{') {
                let end = rest[start..].find('}').unwrap() + start;
                result.push(&rest[start..=end]);
                rest = &rest[end + 1..];
            }
            result.sort();
            result
        }
        assert!(MESSAGES.len() > 300);
        for (source, values) in MESSAGES.iter() {
            assert_eq!(values.len(), LANGUAGES.len(), "{source}");
            for value in values {
                assert!(!value.trim().is_empty(), "{source}");
                assert_eq!(
                    placeholders(source),
                    placeholders(value),
                    "{source}: {value}"
                );
            }
        }
        assert_eq!(translated("通用设置", 2), "General settings");
        assert_eq!(translated("剪贴板", 2), "Clipboard");
        assert_eq!(
            translated("unknown fixture {untouched}", 2),
            "unknown fixture {untouched}"
        );
    }
}
