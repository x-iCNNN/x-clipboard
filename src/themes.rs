// GPUI Kit v0.8.0 bundled theme catalog. See assets/themes/README.md.
pub const THEME_SETS: &[&str] = &[
    include_str!("../assets/themes/adventure.json"),
    include_str!("../assets/themes/alduin.json"),
    include_str!("../assets/themes/asciinema.json"),
    include_str!("../assets/themes/aurora.json"),
    include_str!("../assets/themes/ayu.json"),
    include_str!("../assets/themes/catppuccin.json"),
    include_str!("../assets/themes/everforest.json"),
    include_str!("../assets/themes/fahrenheit.json"),
    include_str!("../assets/themes/flexoki.json"),
    include_str!("../assets/themes/gruvbox.json"),
    include_str!("../assets/themes/harper.json"),
    include_str!("../assets/themes/hybrid.json"),
    include_str!("../assets/themes/jellybeans.json"),
    include_str!("../assets/themes/kibble.json"),
    include_str!("../assets/themes/macos-classic.json"),
    include_str!("../assets/themes/mellifluous.json"),
    include_str!("../assets/themes/molokai.json"),
    include_str!("../assets/themes/solarized.json"),
    include_str!("../assets/themes/spaceduck.json"),
    include_str!("../assets/themes/tokyonight.json"),
    include_str!("../assets/themes/twilight.json"),
];
pub const THEME_NAMES: &[&str] = &[
    "Adventure",
    "Adventure Time",
    "Alduin",
    "Asciinema",
    "Aurora Light",
    "Ayu Dark",
    "Ayu Light",
    "Catppuccin Frappe",
    "Catppuccin Latte",
    "Catppuccin Macchiato",
    "Catppuccin Mocha",
    "Everforest Dark",
    "Everforest Light",
    "Fahrenheit",
    "Flexoki Dark",
    "Flexoki Light",
    "Gruvbox Dark",
    "Gruvbox Light",
    "Harper",
    "Hybrid Dark",
    "Hybrid Light",
    "Jellybeans",
    "Kibble",
    "Mellifluous Dark",
    "Mellifluous Light",
    "Molokai Dark",
    "Molokai Light",
    "Solarized Dark",
    "Solarized Light",
    "Spaceduck",
    "Tokyo Moon",
    "Tokyo Night",
    "Tokyo Storm",
    "Twilight",
    "macOS Classic Dark",
    "macOS Classic Light",
];

#[cfg(test)]
mod tests {
    #[test]
    fn catalog_preserves_all_bundled_theme_names_and_accepts_saved_settings() {
        let mut actual = Vec::new();
        for source in super::THEME_SETS {
            let set: serde_json::Value = serde_json::from_str(source).unwrap();
            for theme in set["themes"].as_array().unwrap() {
                actual.push(theme["name"].as_str().unwrap().to_owned());
            }
        }
        actual.sort();
        assert_eq!(actual, super::THEME_NAMES);
        assert!(super::THEME_SETS.len() > 20);
        let directory = tempfile::tempdir().unwrap();
        let key = [0x3a; 32];
        let store = crate::storage::HistoryStore::open_with_key(directory.path(), &key).unwrap();
        for name in super::THEME_NAMES {
            let settings = crate::model::Settings {
                theme: name.to_string(),
                ..Default::default()
            };
            settings.validate().unwrap();
            let json = serde_json::to_string(&settings).unwrap();
            let reopened: crate::model::Settings = serde_json::from_str(&json).unwrap();
            assert_eq!(reopened.theme, *name);
            store.save_settings(&settings).unwrap();
            assert_eq!(store.settings().unwrap().theme, *name);
        }
        drop(store);
        let reopened = crate::storage::HistoryStore::open_with_key(directory.path(), &key).unwrap();
        assert_eq!(
            reopened.settings().unwrap().theme,
            *super::THEME_NAMES.last().unwrap()
        );
        let unknown = crate::model::Settings {
            theme: "missing theme".into(),
            ..Default::default()
        };
        assert!(unknown.validate().is_err());
    }
}
