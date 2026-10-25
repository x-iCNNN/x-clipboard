use anyhow::{Context, Result, ensure};
use std::{fs, io::Write, path::Path};
use zeroize::Zeroizing;

pub fn database_key(root: &Path) -> Result<Zeroizing<Vec<u8>>> {
    fs::create_dir_all(root)?;
    #[cfg(windows)]
    {
        let path = root.join("clipboard-key.dat");
        if path.try_exists()? {
            let key = Zeroizing::new(
                protect(&fs::read(path)?, true)
                    .context("密钥不可用：无法解锁 Windows 账户保护的数据库密钥")?,
            );
            ensure!(key.len() == 32, "数据库密钥已损坏");
            return Ok(key);
        }
        ensure!(
            !root.join("clipboard.db").try_exists()?,
            "密钥文件丢失；请恢复 clipboard-key.dat，不会覆盖已有数据库"
        );
        let key = new_key()?;
        let protected = protect(&key, false)?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(&protected)?;
        file.sync_all()?;
        Ok(key)
    }
    #[cfg(not(windows))]
    {
        use sha2::{Digest, Sha256};
        let account = format!(
            "{:x}",
            Sha256::digest(root.canonicalize()?.to_string_lossy().as_bytes())
        );
        let entry = keyring::Entry::new("com.xclipboard.local", &account)?;
        match entry.get_secret() {
            Ok(key) => {
                ensure!(key.len() == 32, "系统密钥库中的密钥已损坏");
                Ok(Zeroizing::new(key))
            }
            Err(keyring::Error::NoEntry) => {
                ensure!(
                    !root.join("clipboard.db").try_exists()?,
                    "系统密钥库中找不到原密钥；不会创建新密钥覆盖已有数据"
                );
                let key = new_key()?;
                entry
                    .set_secret(&key)
                    .context("无法保存到系统密钥库；请先解锁密钥库")?;
                Ok(key)
            }
            Err(e) => Err(e).context("系统密钥库不可用，请解锁后重试"),
        }
    }
}
fn new_key() -> Result<Zeroizing<Vec<u8>>> {
    let mut key = Zeroizing::new(vec![0; 32]);
    getrandom::fill(&mut key).map_err(|_| anyhow::anyhow!("无法生成安全随机密钥"))?;
    Ok(key)
}
#[cfg(windows)]
fn protect(bytes: &[u8], decrypt: bool) -> Result<Vec<u8>> {
    use windows::Win32::{
        Foundation::{HLOCAL, LocalFree},
        Security::Cryptography::*,
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len().try_into()?,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        if decrypt {
            CryptUnprotectData(
                &input,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )?;
        } else {
            CryptProtectData(
                &input,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )?;
        }
        let value = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
        Ok(value)
    }
}
