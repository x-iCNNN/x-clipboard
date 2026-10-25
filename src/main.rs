#![cfg_attr(windows, windows_subsystem = "windows")]
mod ui;
fn main() -> anyhow::Result<()> {
    let root = if let Some(path) = std::env::var_os("X_CLIPBOARD_DATA_DIR") {
        std::path::PathBuf::from(path)
    } else {
        directories::ProjectDirs::from("com", "XClipboard", "XClipboard")
            .ok_or_else(|| anyhow::anyhow!("无法确定用户数据目录"))?
            .data_local_dir()
            .to_path_buf()
    };
    std::fs::create_dir_all(&root)?;
    let instance = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.join("instance.lock"))?;
    fs2::FileExt::try_lock_exclusive(&instance)
        .map_err(|_| anyhow::anyhow!("X Clipboard 已在运行，请通过托盘打开"))?;
    ui::run(x_clipboard::service::Service::start(root)?)
}
