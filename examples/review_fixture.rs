//! Creates an isolated encrypted database containing synthetic native-UI review data.
//! Usage: cargo run --no-default-features --example review_fixture -- <new-directory>
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeMap, path::Path};
use x_clipboard::{
    model::{ClipContent, FormatData, Settings},
    storage::HistoryStore,
};

fn main() -> Result<()> {
    let argument = std::env::args().nth(1).context("请指定新的 UI 验收目录")?;
    let root = std::path::PathBuf::from(argument);
    ensure!(
        !root.exists(),
        "验收目录已经存在；请使用新目录，不覆盖已有数据库"
    );
    std::fs::create_dir_all(&root)?;
    let root = root.canonicalize()?;
    let files = root.join("fixtures");
    std::fs::create_dir(&files)?;
    let folder = files.join("品牌素材");
    std::fs::create_dir(&folder)?;
    std::fs::write(
        folder.join("说明.txt"),
        "此目录只包含人工合成的界面验收文件。",
    )?;
    std::fs::write(
        folder.join("配色.json"),
        "{\"primary\":\"#087b64\",\"surface\":\"#ffffff\"}",
    )?;
    let code = files.join("clipboard.rs");
    std::fs::write(
        &code,
        "// 复制快照按内容和格式去重\nfn capture(content: ClipboardContent) -> Result<()> {\n    let fingerprint = content.fingerprint();\n    history.upsert(fingerprint, content)?;\n    Ok(())\n}\n",
    )?;
    let markdown = files.join("发布检查清单.md");
    std::fs::write(
        &markdown,
        "# 品牌发布检查清单\n\n让每一次复制，都有迹可循。\n\n- [x] 检查浅色与深色主题\n- [x] 整理图片与代码素材\n- [ ] 验证快捷键和原窗口回贴\n\n> 好的工具，应当在需要时出现，在工作时退到身后。\n\n| 项目 | 状态 |\n|---|---|\n| 文本与图片 | 已准备 |\n| 原生交互 | 验收中 |\n",
    )?;
    let text_file = files.join("项目说明.txt");
    std::fs::write(
        &text_file,
        "X Clipboard\n\n本地加密保存，离线优先。\n文件历史仅保存引用，不会备份原文件。\n",
    )?;
    let pdf = files.join("品牌设计规范.pdf");
    write_pdf(&pdf)?;
    let mut picture = image::RgbaImage::new(960, 640);
    for (x, y, pixel) in picture.enumerate_pixels_mut() {
        let hill = 390. + 90. * (x as f32 / 145.).sin();
        let sun = (x as i32 - 720).pow(2) + (y as i32 - 150).pow(2) < 80_i32.pow(2);
        *pixel = if sun {
            image::Rgba([244, 205, 123, 255])
        } else if y as f32 > hill {
            image::Rgba([55, 111, 91, 255])
        } else {
            image::Rgba([215, 235, 225, 255])
        };
    }
    let png = files.join("春日灵感.png");
    picture.save(&png)?;
    let broken = files.join("损坏的图片.png");
    std::fs::write(&broken, b"synthetic invalid image fixture")?;
    let missing = files.join("已移动的参考文件.pdf");

    let store = HistoryStore::open(&root)?;
    let settings = Settings {
        setup_complete: true,
        paused: true,
        shortcut: "Ctrl+Alt+Shift+9".into(),
        recent_symbols: vec![
            "😀".into(),
            "👍".into(),
            "✨".into(),
            "→".into(),
            "￥".into(),
        ],
        ..Settings::default()
    };
    store.save_settings(&settings)?;
    let mut ids = BTreeMap::new();
    let mut add = |name: &str, content: ClipContent, pin: bool, favorite: bool| -> Result<()> {
        let entry = store.capture(&content, &settings)?;
        if pin || favorite {
            store.protect(&entry.id, Some(pin), Some(favorite))?;
        }
        ids.insert(name.to_owned(), entry.id);
        Ok(())
    };
    add(
        "missing",
        reference(&missing, "文件资源管理器"),
        false,
        false,
    )?;
    add(
        "preview-error",
        reference(&broken, "文件资源管理器"),
        false,
        false,
    )?;
    add(
        "file",
        reference(&text_file, "文件资源管理器"),
        false,
        false,
    )?;
    add("folder", reference(&folder, "文件资源管理器"), false, false)?;
    add("markdown", reference(&markdown, "Obsidian"), false, true)?;
    add("pdf", reference(&pdf, "文件资源管理器"), false, false)?;
    add("code", reference(&code, "Visual Studio Code"), false, false)?;
    let mut rich = ClipContent::text("让每一次复制，都有迹可循。\n保留格式的产品介绍与项目说明。");
    rich.source = "Notion".into();
    rich.formats.push(FormatData { name: "text/html".into(), bytes: "<h2>让每一次复制，都有迹可循。</h2><p>一个安静、可靠的剪贴板，让你专注手头的工作。</p><blockquote>好的工具，应当在需要时出现，在工作时退到身后。</blockquote><h3>为你的工作流而设计</h3><ul><li>快速找回刚才复制的内容</li><li>收藏重要片段，置顶常用内容</li><li>保留原始格式与完整信息</li></ul>".as_bytes().to_vec() });
    add("rich", rich, false, true)?;
    add(
        "image",
        ClipContent {
            formats: vec![FormatData {
                name: "image/png".into(),
                bytes: std::fs::read(&png)?,
            }],
            source: "Figma".into(),
            ..Default::default()
        },
        false,
        false,
    )?;
    let mut text = ClipContent::text(
        "发布前检查清单\n确认文案、导出资源、检查明暗主题。\n完成交互验收后再发布。",
    );
    text.source = "备忘录".into();
    add("text", text, true, false)?;
    let mut long = ClipContent::text(
        "品牌视觉与交互细节：保持层级清晰、操作可发现，让每一次复制都能快速找回。".repeat(12),
    );
    long.source = "浏览器".into();
    add("long-text", long, false, false)?;
    store.checkpoint()?;
    std::fs::write(
        root.join("UI-REVIEW-FIXTURE.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"schema":1,"synthetic":true,"ids":ids}))?,
    )?;
    println!("Synthetic encrypted UI fixture created: {}", root.display());
    Ok(())
}

fn reference(path: &Path, source: &str) -> ClipContent {
    let path = path.to_string_lossy().into_owned();
    ClipContent {
        formats: vec![FormatData {
            name: "text/uri-list".into(),
            bytes: path.as_bytes().to_vec(),
        }],
        files: vec![path],
        source: source.into(),
        ..Default::default()
    }
}

fn write_pdf(path: &Path) -> Result<()> {
    let streams = [
        "0.03 0.48 0.39 rg 30 165 260 35 re f\n0.87 0.94 0.9 rg 30 35 260 110 re f\n",
        "0.03 0.48 0.39 rg 30 35 50 95 re f\n0.4 0.65 0.53 rg 100 35 50 135 re f\n0.7 0.82 0.73 rg 170 35 50 165 re f\n",
    ];
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 320 240] /Resources << >> /Contents 5 0 R >>"
            .to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 320 240] /Resources << >> /Contents 6 0 R >>"
            .to_owned(),
        format!(
            "<< /Length {} >>\nstream\n{}endstream",
            streams[0].len(),
            streams[0]
        ),
        format!(
            "<< /Length {} >>\nstream\n{}endstream",
            streams[1].len(),
            streams[1]
        ),
    ];
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend(b"xref\n0 7\n0000000000 65535 f \n");
    for offset in offsets {
        pdf.extend(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend(
        format!("trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    std::fs::write(path, pdf)?;
    Ok(())
}
