//! Explicit, disposable install/upgrade/uninstall probe. Uses a fresh directory only.
use anyhow::{Context, Result, ensure};
use std::{fs, path::PathBuf};
use x_clipboard::installer::{self, Options};

fn main() -> Result<()> {
    let setup = PathBuf::from(
        std::env::args()
            .nth(1)
            .context("Specify the packaged setup EXE")?,
    );
    let target = PathBuf::from(
        std::env::args()
            .nth(2)
            .context("Specify a NEW disposable directory")?,
    );
    ensure!(!target.exists(), "Refusing an existing directory");
    let bytes = installer::payload(&setup)?;
    let options = Options {
        target: target.clone(),
        desktop: true,
        start_menu: true,
    };
    ensure!(
        installer::install(&bytes, &setup, &options)?.is_none(),
        "Installation integration warning"
    );
    let expected = fs::read(target.join("x-clipboard.exe"))?;
    ensure!(expected.starts_with(b"MZ"), "Not a Windows executable");
    fs::write(target.join("preserve-user-data.txt"), b"synthetic-preserve")?;
    ensure!(
        installer::install(&bytes, &setup, &options)?.is_none(),
        "Upgrade integration warning"
    );
    ensure!(
        fs::read(target.join("x-clipboard.exe"))? == expected,
        "Installed binary mismatch"
    );
    ensure!(
        fs::read(target.join("preserve-user-data.txt"))? == b"synthetic-preserve",
        "User data changed"
    );
    println!(
        "PASS: packaged payload checksum, real custom-path install, overwrite upgrade, binary integrity, preserved user data."
    );
    #[cfg(windows)]
    installer::uninstall(&target)?;
    println!(
        "Uninstall helper armed; verify application removal and preserved marker after this process exits."
    );
    Ok(())
}
