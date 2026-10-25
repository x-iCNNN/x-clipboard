//! Live GitHub check of manual and startup automatic update scheduling.
use anyhow::{Result, bail, ensure};
use std::time::{Duration, Instant};
use x_clipboard::updater::{State, Updater};

fn wait(updater: &Updater) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        match updater.snapshot().1 {
            State::Latest | State::Available(_) | State::Ready(_) => return Ok(()),
            State::Error(error) => bail!(error),
            _ => ensure!(Instant::now() < deadline, "GitHub check timed out"),
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
fn main() -> Result<()> {
    ensure!(x_clipboard::updater::SUPPORTED, "Requires Windows x64");
    let root = tempfile::tempdir()?;
    let manual = Updater::new(root.path().to_path_buf());
    manual.check(false);
    wait(&manual)?;
    println!("PASS: manual asynchronous GitHub check");
    let mut automatic = Updater::new(root.path().to_path_buf());
    automatic.tick(false);
    ensure!(
        matches!(automatic.snapshot().1, State::Idle),
        "Disabled automatic update ran"
    );
    std::thread::sleep(Duration::from_secs(21));
    automatic.tick(true);
    ensure!(
        !matches!(automatic.snapshot().1, State::Idle),
        "Startup automatic check did not run"
    );
    wait(&automatic)?;
    println!("PASS: disabled setting, startup delay, automatic asynchronous GitHub check");
    Ok(())
}
