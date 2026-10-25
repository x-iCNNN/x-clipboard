//! Encrypted recovery after an abrupt child-process exit without Rust destructors.
use x_clipboard::{
    model::{ClipContent, Settings},
    storage::HistoryStore,
};
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "--child") {
        let store = HistoryStore::open_with_key(std::path::Path::new(&args[2]), &[19; 32])?;
        store.capture(
            &ClipContent::text("abrupt-exit-private-fixture-73251"),
            &Settings::default(),
        )?;
        std::process::exit(23);
    }
    let root = tempfile::tempdir()?;
    let status = std::process::Command::new(std::env::current_exe()?)
        .arg("--child")
        .arg(root.path())
        .status()?;
    anyhow::ensure!(
        status.code() == Some(23),
        "child failed before committed write"
    );
    for file in ["clipboard.db", "clipboard.db-wal"] {
        if let Ok(bytes) = std::fs::read(root.path().join(file)) {
            anyhow::ensure!(
                !bytes
                    .windows(31)
                    .any(|part| part == b"abrupt-exit-private-fixture-73251"),
                "plaintext found in database files"
            );
        }
    }
    let store = HistoryStore::open_with_key(root.path(), &[19; 32])?;
    let entries = store.list()?;
    anyhow::ensure!(entries.len() == 1, "committed history was lost");
    anyhow::ensure!(
        store.content(&entries[0].id)?.text == "abrupt-exit-private-fixture-73251",
        "payload mismatch"
    );
    store.checkpoint()?;
    println!(
        "PASS: abrupt process exit, encrypted database/WAL, committed payload recovery, checkpoint"
    );
    Ok(())
}
