//! Seed a fresh, paused, empty database for reproducible welcome-window memory measurements.
fn main() -> anyhow::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("new data directory required"))?;
    let root = std::path::PathBuf::from(path);
    anyhow::ensure!(
        !root.join("clipboard.db").exists() && !root.join("clipboard-key.dat").exists(),
        "refusing to overwrite existing data"
    );
    let store = x_clipboard::storage::HistoryStore::open(&root)?;
    store.save_settings(&x_clipboard::model::Settings {
        paused: true,
        shortcut: "Ctrl+Alt+Shift+9".into(),
        ..Default::default()
    })?;
    store.checkpoint()?;
    Ok(())
}
