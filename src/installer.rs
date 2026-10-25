//! Portable package installation. This module never opens clipboard storage.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
};

const MAGIC: &[u8; 16] = b"XCLIPBOARDSETUP!";
pub fn payload(executable: &Path) -> Result<Vec<u8>> {
    let mut file = fs::File::open(executable)?;
    let length = file.metadata()?.len();
    ensure!(length >= 56, "安装器尚未打包应用文件");
    file.seek(SeekFrom::End(-56))?;
    let mut footer = [0u8; 56];
    file.read_exact(&mut footer)?;
    ensure!(
        &footer[40..] == MAGIC,
        "安装器未包含应用包，请下载正式安装版"
    );
    let size = u64::from_le_bytes(footer[32..40].try_into()?);
    ensure!(
        size > 0 && size <= 256 * 1024 * 1024 && size <= length - 56,
        "安装包长度无效"
    );
    file.seek(SeekFrom::Start(length - 56 - size))?;
    let mut bytes = vec![0; size as usize];
    file.read_exact(&mut bytes)?;
    ensure!(
        Sha256::digest(&bytes)[..] == footer[..32],
        "安装包已损坏，SHA-256 不匹配"
    );
    Ok(bytes)
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Options {
    pub target: PathBuf,
    pub desktop: bool,
    pub start_menu: bool,
}
fn validate_target(target: &Path) -> Result<()> {
    ensure!(
        target.is_absolute() && target.file_name().is_some() && target.parent().is_some(),
        "请选择绝对安装路径，不能安装到磁盘根目录"
    );
    ensure!(
        !target
            .components()
            .any(|c| matches!(c, Component::ParentDir)),
        "安装路径不能包含 .."
    );
    for ancestor in target.ancestors() {
        if ancestor.exists() {
            ensure!(
                !fs::symlink_metadata(ancestor)?.file_type().is_symlink(),
                "安装路径不能包含符号链接或目录联接"
            );
        }
    }
    Ok(())
}
#[cfg(windows)]
fn check_closed(target: &Path) -> Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    for name in ["x-clipboard.exe", "pdfium.dll", "uninstall.exe"] {
        let path = target.join(name);
        if path.exists() {
            fs::OpenOptions::new()
                .read(true)
                .write(true)
                .share_mode(0)
                .open(path)
                .context("应用正在运行或安装目录不可写，请从托盘退出 X Clipboard 后重试")?;
        }
    }
    Ok(())
}
#[cfg(not(windows))]
fn check_closed(_: &Path) -> Result<()> {
    Ok(())
}

/// Returns a non-fatal shortcut/registration warning after successful file installation.
pub fn install(bytes: &[u8], setup: &Path, options: &Options) -> Result<Option<String>> {
    validate_target(&options.target)?;
    fs::create_dir_all(&options.target).context("无法创建安装目录，请选择当前用户可写的位置")?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(options.target.join(".installation.lock"))?;
    fs2::FileExt::try_lock_exclusive(&lock).context("此目录已有安装任务正在进行")?;
    check_closed(&options.target)?;
    let stage = options
        .target
        .join(format!(".setup-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&stage)?;
    let result = install_staged(bytes, setup, options, &stage);
    // Keep the backup if a rollback itself failed.
    if !stage.join("rollback-failed").exists() {
        let _ = fs::remove_dir_all(&stage);
    }
    result?;
    Ok(integrate(options)
        .err()
        .map(|e| format!("应用已安装，但快捷方式或卸载注册未完成：{e:#}")))
}
fn install_staged(bytes: &[u8], setup: &Path, options: &Options, stage: &Path) -> Result<()> {
    let files = crate::updater::extract(bytes, &stage.join("payload"))?;
    fs::copy(setup, stage.join("payload/uninstall.exe"))?;
    let mut files = files;
    files.push("uninstall.exe".into());
    // The manifest is used only to delete files from this exact installation.
    let manifest = stage.join("payload/installation.json");
    fs::write(manifest, serde_json::to_vec(&files)?)?;
    files.push("installation.json".into());
    let mut changed = Vec::new();
    let result: Result<()> = (|| {
        for name in &files {
            let destination = options.target.join(name);
            // Check nested destinations as well as the install directory itself.
            for ancestor in destination.ancestors() {
                if ancestor.exists() {
                    ensure!(
                        !fs::symlink_metadata(ancestor)?.file_type().is_symlink(),
                        "目标文件包含符号链接或目录联接"
                    );
                }
            }
            let backup = stage.join("backup").join(name);
            let existed = destination.exists();
            if existed {
                fs::create_dir_all(backup.parent().unwrap())?;
                fs::copy(&destination, &backup)?;
            }
            changed.push((destination.clone(), backup, existed));
            fs::create_dir_all(destination.parent().unwrap())?;
            fs::copy(stage.join("payload").join(name), destination)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        for (destination, backup, existed) in changed.into_iter().rev() {
            let restored = if existed {
                fs::copy(backup, destination).map(|_| ())
            } else {
                fs::remove_file(destination)
            };
            if let Err(rollback) = restored {
                let _ = fs::write(stage.join("rollback-failed"), rollback.to_string());
                return Err(error.context(format!(
                    "回滚失败：{rollback}；备份保留在 {}",
                    stage.display()
                )));
            }
        }
        return Err(error);
    }
    Ok(())
}
#[cfg(windows)]
fn powershell() -> Result<std::process::Command> {
    use std::os::windows::process::CommandExt;
    let executable = PathBuf::from(std::env::var_os("SystemRoot").context("SystemRoot 缺失")?)
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut command = std::process::Command::new(executable);
    command
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .creation_flags(0x08000000);
    Ok(command)
}
#[cfg(windows)]
fn integrate(options: &Options) -> Result<()> {
    let script = options.target.join("install-integration.ps1");
    let plan = options.target.join("install-options.json");
    fs::write(&script, include_str!("../scripts/install-integration.ps1"))?;
    fs::write(&plan, serde_json::to_vec(options)?)?;
    let output = powershell()?
        .arg(&script)
        .arg("-Plan")
        .arg(&plan)
        .arg("-Version")
        .arg(env!("CARGO_PKG_VERSION"))
        .output()?;
    let _ = fs::remove_file(&script);
    let _ = fs::remove_file(&plan);
    ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
#[cfg(not(windows))]
fn integrate(_: &Options) -> Result<()> {
    Ok(())
}

#[cfg(windows)]
pub fn uninstall(target: &Path) -> Result<()> {
    validate_target(target)?;
    // The running uninstaller is intentionally excluded from this sharing check.
    use std::os::windows::fs::OpenOptionsExt;
    let exe = target.join("x-clipboard.exe");
    if exe.exists() {
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(exe)
            .context("请先从托盘退出 X Clipboard")?;
    }
    let script = std::env::temp_dir().join(format!(
        "x-clipboard-uninstall-{}.ps1",
        uuid::Uuid::new_v4()
    ));
    fs::write(&script, include_str!("../scripts/uninstall.ps1"))?;
    powershell()?
        .arg(script)
        .arg("-Target")
        .arg(target)
        .arg("-ParentPid")
        .arg(std::process::id().to_string())
        .spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn archive() -> Vec<u8> {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for name in ["x-clipboard.exe", "pdfium.dll", "licenses/LICENSE.txt"] {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"new-app").unwrap();
        }
        zip.finish().unwrap().into_inner()
    }
    #[test]
    fn custom_directory_install_preserves_data_and_rolls_back_failed_upgrade() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("自定义 path/app");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("private.key"), b"preserve").unwrap();
        let setup = dir.path().join("setup.exe");
        fs::write(&setup, b"MZ-setup-fixture").unwrap();
        let options = Options {
            target: target.clone(),
            desktop: false,
            start_menu: false,
        };
        let stage = dir.path().join("stage");
        install_staged(&archive(), &setup, &options, &stage).unwrap();
        assert_eq!(
            fs::read(target.join("x-clipboard.exe")).unwrap(),
            b"new-app"
        );
        assert_eq!(fs::read(target.join("private.key")).unwrap(), b"preserve");
        assert!(target.join("installation.json").exists());
        assert_eq!(
            fs::read(target.join("uninstall.exe")).unwrap(),
            b"MZ-setup-fixture"
        );
        fs::write(target.join("x-clipboard.exe"), b"old-app").unwrap();
        fs::remove_file(target.join("pdfium.dll")).unwrap();
        fs::create_dir(target.join("pdfium.dll")).unwrap();
        assert!(
            install_staged(
                &archive(),
                &setup,
                &options,
                &dir.path().join("failed-stage")
            )
            .is_err()
        );
        assert_eq!(
            fs::read(target.join("x-clipboard.exe")).unwrap(),
            b"old-app"
        );
        assert_eq!(fs::read(target.join("private.key")).unwrap(), b"preserve");
    }
    #[test]
    fn payload_checksum_and_bounds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("setup.exe");
        let bytes = b"zip fixture";
        let mut exe = b"MZ fixture".to_vec();
        exe.extend(bytes);
        exe.extend(Sha256::digest(bytes));
        exe.extend((bytes.len() as u64).to_le_bytes());
        exe.extend(MAGIC);
        fs::write(&path, &exe).unwrap();
        assert_eq!(payload(&path).unwrap(), bytes);
        exe[10] ^= 1;
        fs::write(&path, &exe).unwrap();
        assert!(payload(&path).is_err());
        fs::write(&path, b"MZ").unwrap();
        assert!(payload(&path).is_err());
    }
    #[test]
    fn rejects_relative_root_and_parent_traversal() {
        assert!(validate_target(Path::new("relative/app")).is_err());
        let root = std::env::current_dir()
            .unwrap()
            .ancestors()
            .last()
            .unwrap()
            .to_path_buf();
        assert!(validate_target(&root).is_err());
        assert!(validate_target(&std::env::temp_dir().join("app/../elsewhere")).is_err());
    }
}
