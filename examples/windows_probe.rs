//! Opt-in system probe. Uses an isolated database, temporarily writes test clipboard
//! data, and restores the supported formats captured on entry. Never logs content.
#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    use anyhow::Context;
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    use windows::{
        Win32::{
            Foundation::*,
            System::{DataExchange::*, Memory::*},
            UI::WindowsAndMessaging::*,
        },
        core::{PCWSTR, w},
    };
    use x_clipboard::{
        model::ClipContent,
        platform::ClipboardBackend,
        service::{Command, Service},
    };
    let root = tempfile::tempdir()?;
    let service = Service::start(root.path().to_path_buf()).context("start service")?;
    let wait = |condition: &dyn Fn() -> bool| -> anyhow::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !condition() {
            unsafe {
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            anyhow::ensure!(Instant::now() < deadline, "probe timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    };
    wait(&|| service.snapshot().ready)?;
    // Keep original clipboard data in memory only, never in the probe report.
    println!("stage: original clipboard");
    let original = service
        .backend
        .read(0, false)
        .context("snapshot original clipboard")?;
    struct Restore {
        backend: Arc<dyn ClipboardBackend>,
        original: Option<ClipContent>,
    }
    impl Drop for Restore {
        fn drop(&mut self) {
            if let Some(content) = &self.original {
                let _ = self.backend.write(content, false);
            }
        }
    }
    let restore = Restore {
        backend: service.backend.clone(),
        original,
    };
    let emitter = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("X Clipboard test emitter"),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            None,
            None,
        )?
    };
    let emit = |formats: &[(u32, Vec<u8>)]| -> anyhow::Result<()> {
        unsafe {
            let deadline = Instant::now() + Duration::from_secs(1);
            while OpenClipboard(Some(emitter)).is_err() {
                anyhow::ensure!(Instant::now() < deadline, "clipboard remained locked");
                std::thread::sleep(Duration::from_millis(10));
            }
            let result = (|| -> anyhow::Result<()> {
                EmptyClipboard().context("empty emitter clipboard")?;
                for (id, bytes) in formats {
                    let h = GlobalAlloc(GMEM_MOVEABLE, bytes.len()).context("allocate")?;
                    let ptr = GlobalLock(h);
                    anyhow::ensure!(!ptr.is_null(), "allocation failed");
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.cast::<u8>(), bytes.len());
                    let _ = GlobalUnlock(h);
                    SetClipboardData(*id, Some(HANDLE(h.0)))
                        .with_context(|| format!("publish format {id}"))?;
                }
                Ok(())
            })();
            let _ = CloseClipboard();
            result
        }
    };
    let utf = |text: &str| {
        text.encode_utf16()
            .chain(Some(0))
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    };
    let reg = |name: &str| {
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        unsafe { RegisterClipboardFormatW(PCWSTR(wide.as_ptr())) }
    };
    emit(&[(13, utf("X Clipboard probe alpha"))])?;
    wait(&|| service.snapshot().entries.len() == 1)?;
    println!("stage: text captured");
    let first = service.snapshot().entries[0].id.clone();
    emit(&[(13, utf("X Clipboard probe beta"))])?;
    wait(&|| service.snapshot().entries.len() == 2)?;
    emit(&[(13, utf("X Clipboard probe alpha"))])?;
    wait(&|| service.snapshot().entries[0].id == first)?;
    anyhow::ensure!(
        service.snapshot().entries.len() == 2,
        "duplicate created a new row"
    );
    let html=b"Version:0.9\r\n<html><body><!--StartFragment--><b>probe</b><!--EndFragment--></body></html>\0".to_vec();
    let rtf = b"{\\rtf1\\ansi probe}\0".to_vec();
    emit(&[
        (13, utf("X Clipboard probe alpha")),
        (reg("HTML Format"), html.clone()),
        (reg("Rich Text Format"), rtf.clone()),
    ])?;
    wait(&|| service.snapshot().entries.len() == 3)?;
    let rich = service.snapshot().entries[0].id.clone();
    println!("stage: multi format captured");
    let read = service.backend.read(0, false)?.unwrap();
    anyhow::ensure!(
        read.formats.len() == 3,
        "multi-format capture lost a format"
    );
    let revision = service.snapshot().revision;
    service.send(Command::Write(rich.clone(), false, None));
    wait(&|| service.snapshot().revision > revision)?;
    wait(&|| {
        if unsafe { OpenClipboard(Some(emitter)) }.is_ok() {
            {
                let result = unsafe { IsClipboardFormatAvailable(reg("HTML Format")) }.is_ok();
                unsafe {
                    let _ = CloseClipboard();
                }
                result
            }
        } else {
            false
        }
    })?;
    std::thread::sleep(Duration::from_millis(150));
    anyhow::ensure!(
        service.snapshot().entries.len() == 3,
        "self write created history"
    );
    println!("stage: rich write complete");
    let revision = service.snapshot().revision;
    service.send(Command::Write(rich, true, None));
    wait(&|| service.snapshot().revision > revision)?;
    unsafe {
        OpenClipboard(Some(emitter))?;
        let html_present = IsClipboardFormatAvailable(reg("HTML Format")).is_ok();
        let _ = CloseClipboard();
        anyhow::ensure!(!html_present, "plain paste retained HTML");
    }
    println!("stage: plain write complete");
    let image = image::RgbaImage::from_pixel(16, 16, image::Rgba([7, 128, 110, 255]));
    let mut bytes = std::io::Cursor::new(vec![]);
    image.write_to(&mut bytes, image::ImageFormat::Png)?;
    emit(&[(reg("PNG"), bytes.into_inner())])?;
    wait(&|| service.snapshot().entries.len() == 4)?;
    service.preview(service.snapshot().entries[0].id.clone(), 0);
    wait(&|| {
        service
            .snapshot()
            .preview
            .as_ref()
            .is_some_and(|p| p.image.is_some())
    })?;
    println!("stage: PNG preview complete");
    let file = root.path().join("probe.txt");
    std::fs::write(&file, "file-preview-probe")?;
    let path = file.to_string_lossy();
    let mut drop = 20u32.to_le_bytes().to_vec();
    drop.extend([0; 12]);
    drop.extend(1u32.to_le_bytes());
    drop.extend(path.encode_utf16().chain([0, 0]).flat_map(u16::to_le_bytes));
    emit(&[
        (15, drop),
        (reg("Preferred DropEffect"), 2u32.to_le_bytes().to_vec()),
    ])?;
    wait(&|| service.snapshot().entries.len() == 5)?;
    let file_id = service.snapshot().entries[0].id.clone();
    let revision = service.snapshot().revision;
    service.send(Command::Write(file_id.clone(), false, None));
    wait(&|| service.snapshot().revision > revision)?;
    unsafe {
        OpenClipboard(Some(emitter))?;
        let h = GetClipboardData(reg("Preferred DropEffect"))?;
        let ptr = GlobalLock(HGLOBAL(h.0));
        let effect = *(ptr as *const u32);
        let _ = GlobalUnlock(HGLOBAL(h.0));
        let _ = CloseClipboard();
        anyhow::ensure!(effect == 1, "file history replayed a move operation");
    }
    println!("stage: file copy effect verified");
    std::fs::remove_file(&file)?;
    service.preview(file_id, 0);
    wait(&|| {
        service
            .snapshot()
            .preview
            .as_ref()
            .is_some_and(|p| p.error.is_some())
    })?;
    println!("stage: missing-file verified");
    for i in 0..20 {
        emit(&[(13, utf(&format!("paced-copy-{i}")))])?;
        std::thread::sleep(Duration::from_millis(30));
    }
    wait(&|| service.snapshot().entries.len() == 25)?;
    service.send(Command::Protect(first.clone(), Some(true), Some(true)));
    wait(&|| {
        service
            .snapshot()
            .entries
            .iter()
            .any(|e| e.id == first && e.pinned && e.favorite)
    })?;
    service.send(Command::Clear(false));
    wait(&|| service.snapshot().entries.len() == 1)?;
    std::mem::drop(restore);
    service.send(Command::Shutdown);
    wait(&|| service.snapshot().shutdown_complete)?;
    unsafe {
        let _ = DestroyWindow(emitter);
    }

    println!(
        "PASS: text capture, duplicate ordering, format-sensitive identity, HTML+RTF preservation, self-write suppression, plain-text write, PNG preview, file copy semantics, missing-file preview, 20 paced copies, protected clear, graceful checkpoint"
    );
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("Windows probe only");
}
