use crate::model::{ClipContent, MAX_THUMBNAIL_BYTES, THUMBNAIL_HEIGHT, THUMBNAIL_WIDTH};
use anyhow::{Context, Result, ensure};
use std::{
    io::{Cursor, Read},
    path::Path,
};

#[derive(Clone, Default)]
pub struct Preview {
    pub id: String,
    pub title: String,
    pub text: String,
    pub image: Option<(u32, u32, Vec<u8>)>,
    pub files: Vec<String>,
    pub is_directory: bool,
    pub formats: Vec<String>,
    pub page: u16,
    pub pages: u16,
    pub error: Option<String>,
    pub markup: Option<String>,
}
pub fn generate(id: String, content: ClipContent, page: u16) -> Preview {
    let mut result = Preview {
        id,
        title: content.title(),
        text: content.text.chars().take(1_048_576).collect(),
        files: content.files.clone(),
        formats: content.formats.iter().map(|f| f.name.clone()).collect(),
        page,
        ..Default::default()
    };
    if let Err(error) = fill(&content, &mut result) {
        result.error = Some(error.to_string());
    }
    result
}
fn fill(content: &ClipContent, result: &mut Preview) -> Result<()> {
    if let Some(path) = content.files.first() {
        let path = Path::new(path);
        let meta = path.metadata().with_context(|| {
            format!(
                "无法访问文件引用：{}（文件可能已移动、删除或没有权限）",
                path.display()
            )
        })?;
        result.text = format!(
            "{}\n\n大小：{} 字节\n项目数：{}\n仅保存引用；不会备份原文件。",
            path.display(),
            meta.len(),
            content.files.len()
        );
        result.is_directory = meta.is_dir();
        if result.is_directory {
            let mut names = Vec::new();
            for item in std::fs::read_dir(path)?.take(200) {
                let item = item?;
                names.push(format!(
                    "{} {}",
                    if item.file_type()?.is_dir() {
                        "▸"
                    } else {
                        "·"
                    },
                    item.file_name().to_string_lossy()
                ));
            }
            result.text = format!(
                "{}\n\n直接子项（最多显示 200 项）：\n{}",
                result.text,
                names.join("\n")
            );
            return Ok(());
        }
        let ext = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        if ["png", "jpg", "jpeg", "gif", "webp", "bmp", "tiff"].contains(&ext.as_str()) {
            ensure!(
                meta.len() <= 64 * 1024 * 1024,
                "图片超过 64 MB 预览上限，请使用默认应用打开"
            );
            return decode_image(&std::fs::read(path)?, result);
        }
        if ext == "pdf" {
            return pdf(path, result);
        }
        if [
            "txt", "md", "rs", "js", "ts", "tsx", "jsx", "json", "toml", "yaml", "yml", "xml",
            "html", "css", "py", "log", "csv", "ini", "sh", "sql", "c", "cpp", "h",
        ]
        .contains(&ext.as_str())
        {
            let mut bytes = Vec::new();
            std::fs::File::open(path)?
                .take(1024 * 1024)
                .read_to_end(&mut bytes)?;
            result.text =
                String::from_utf8(bytes).context("此文本不是 UTF-8，请使用默认应用打开")?;
            if meta.len() > 1024 * 1024 {
                result.text.push_str("\n\n[预览仅显示前 1 MB]");
            }
            if ext == "md" {
                let mut html = String::new();
                pulldown_cmark::html::push_html(
                    &mut html,
                    pulldown_cmark::Parser::new_ext(&result.text, pulldown_cmark::Options::all()),
                );
                result.markup = Some(sanitize_html(&html));
            }
        }
        return Ok(());
    }
    if content.kind() == "image" {
        if let Some(png) = content
            .formats
            .iter()
            .find(|f| matches!(f.name.as_str(), "image/png" | "windows:PNG"))
        {
            return decode_image(&png.bytes, result);
        }
        if let Some(dib) = content
            .formats
            .iter()
            .find(|f| matches!(f.name.as_str(), "windows:17" | "windows:8"))
        {
            return decode_image(&dib_to_bmp(&dib.bytes)?, result);
        }
    }
    if let Some(format) = content
        .formats
        .iter()
        .find(|f| f.name == "text/html" || f.name == "windows:HTML Format")
    {
        let raw = String::from_utf8_lossy(&format.bytes[..format.bytes.len().min(1_048_576)]);
        let start = raw
            .find("<!--StartFragment-->")
            .map(|n| n + 20)
            .or_else(|| raw.find('<'))
            .unwrap_or(0);
        let end = raw.find("<!--EndFragment-->").unwrap_or(raw.len());
        if let Some(fragment) = raw.get(start..end) {
            result.markup = Some(sanitize_html(fragment));
        }
    }
    Ok(())
}
fn sanitize_html(html: &str) -> String {
    ammonia::Builder::default()
        .rm_tags(&[
            "img", "picture", "source", "video", "audio", "iframe", "object", "embed", "svg",
        ])
        .rm_tag_attributes("a", &["href"])
        .clean(html)
        .to_string()
}
fn decode_image(bytes: &[u8], result: &mut Preview) -> Result<()> {
    let image = decode_bitmap(bytes)?;
    let image = if image.width() > 4096 || image.height() > 4096 {
        image.thumbnail(4096, 4096)
    } else {
        image
    };
    result.image = Some((image.width(), image.height(), image.to_rgba8().into_raw()));
    Ok(())
}
pub fn thumbnail(content: &ClipContent) -> Result<image::RgbaImage> {
    let image = if let Some(png) = content
        .formats
        .iter()
        .find(|format| matches!(format.name.as_str(), "image/png" | "windows:PNG"))
    {
        decode_bitmap(&png.bytes)?
    } else {
        let dib = content
            .formats
            .iter()
            .find(|format| matches!(format.name.as_str(), "windows:17" | "windows:8"))
            .context("图片格式不支持缩略图")?;
        decode_bitmap(&dib_to_bmp(&dib.bytes)?)?
    };
    Ok(image
        .thumbnail(THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT)
        .to_rgba8())
}
pub fn encode_thumbnail(image: &image::RgbaImage) -> Result<Vec<u8>> {
    ensure!(
        image.width() <= THUMBNAIL_WIDTH && image.height() <= THUMBNAIL_HEIGHT,
        "缩略图尺寸超出限制"
    );
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png)?;
    let bytes = bytes.into_inner();
    ensure!(bytes.len() <= MAX_THUMBNAIL_BYTES, "缩略图数据超出限制");
    Ok(bytes)
}
pub fn decode_thumbnail(bytes: &[u8]) -> Result<image::RgbaImage> {
    ensure!(bytes.len() <= MAX_THUMBNAIL_BYTES, "缩略图数据超出限制");
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(THUMBNAIL_WIDTH);
    limits.max_image_height = Some(THUMBNAIL_HEIGHT);
    limits.max_alloc = Some(1024 * 1024);
    reader.limits(limits);
    Ok(reader.decode()?.to_rgba8())
}
fn decode_bitmap(bytes: &[u8]) -> Result<image::DynamicImage> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(128 * 1024 * 1024);
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    reader.limits(limits);
    reader.decode().context("图片解码失败")
}
pub fn dib_to_bmp(bytes: &[u8]) -> Result<Vec<u8>> {
    ensure!(bytes.len() >= 40, "DIB 图像头无效");
    let value = |p: usize| u32::from_le_bytes(bytes[p..p + 4].try_into().unwrap());
    let header = value(0);
    ensure!(
        header >= 40 && header as usize <= bytes.len(),
        "DIB 图像头无效"
    );
    let bits = u16::from_le_bytes([bytes[14], bytes[15]]);
    let compression = value(16);
    let used = value(32);
    let palette = if used > 0 {
        used
    } else if bits <= 8 {
        1u32 << bits
    } else {
        0
    };
    let masks = if header == 40 && compression == 3 {
        12
    } else if header == 40 && compression == 6 {
        16
    } else {
        0
    };
    let offset = 14u64 + header as u64 + palette as u64 * 4 + masks;
    ensure!(offset <= bytes.len() as u64 + 14, "DIB 调色板无效");
    let mut bmp = b"BM".to_vec();
    bmp.extend(((bytes.len() + 14) as u32).to_le_bytes());
    bmp.extend([0; 4]);
    bmp.extend((offset as u32).to_le_bytes());
    bmp.extend(bytes);
    Ok(bmp)
}
fn pdf(path: &Path, result: &mut Preview) -> Result<()> {
    use pdfium_render::prelude::*;
    // pdfium-render permits only one initialization per process. Keep it alive across pages.
    static RUNTIME: std::sync::Mutex<Option<Pdfium>> = std::sync::Mutex::new(None);
    let mut runtime = RUNTIME
        .lock()
        .map_err(|_| anyhow::anyhow!("PDF 预览服务已中断，请重启应用"))?;
    if runtime.is_none() {
        let library = std::env::current_exe()?
            .parent()
            .context("找不到程序目录")?
            .join(Pdfium::pdfium_platform_library_name());
        let bindings = Pdfium::bind_to_library(library)
            .or_else(|_| Pdfium::bind_to_system_library())
            .context("PDF 预览组件未安装：请将 PDFium 动态库放入程序目录，或使用默认应用打开")?;
        *runtime = Some(Pdfium::new(bindings));
    }
    let pdfium = runtime.as_ref().expect("initialized PDFium runtime");
    let document = pdfium
        .load_pdf_from_file(path, None)
        .context("无法打开 PDF（可能损坏或需要密码）")?;
    result.pages = document
        .pages()
        .len()
        .try_into()
        .context("PDF 页数超过预览上限")?;
    ensure!(result.pages > 0, "PDF 没有页面");
    result.page = result.page.min(result.pages - 1);
    let page = document.pages().get(result.page.into())?;
    let bitmap = page.render_with_config(
        &PdfRenderConfig::new()
            .set_target_width(1200)
            .set_maximum_height(1800),
    )?;
    let image = bitmap.as_image()?.to_rgba8();
    result.image = Some((image.width(), image.height(), image.into_raw()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FormatData;
    #[test]
    fn rich_preview_cannot_load_external_resources_or_scripts() {
        let input = "<b>Hello</b><img src='https://example.org/tracker'><script>alert(1)</script><a href='file:///private'>link</a>";
        let output = sanitize_html(input);
        assert!(output.contains("<b>Hello</b>"));
        for forbidden in ["<img", "<script", "href=", "example.org", "file://"] {
            assert!(!output.contains(forbidden));
        }
    }
    #[test]
    fn missing_reference_is_an_error_without_recovery_claim() {
        let root = tempfile::tempdir().unwrap();
        let mut content = ClipContent::text("");
        content.files.push(
            root.path()
                .join("missing.txt")
                .to_string_lossy()
                .into_owned(),
        );
        let preview = generate("missing".into(), content, 0);
        assert!(preview.error.unwrap().contains("无法访问文件引用"));
    }
    #[test]
    fn folder_preview_never_recurses() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("child")).unwrap();
        std::fs::write(root.path().join("child/private.txt"), "nested").unwrap();
        let content = ClipContent {
            files: vec![root.path().to_string_lossy().into_owned()],
            ..Default::default()
        };
        let preview = generate("folder".into(), content, 0);
        assert!(preview.text.contains("child"));
        assert!(!preview.text.contains("private.txt"));
    }
    #[test]
    fn truncated_dib_is_rejected_without_panicking() {
        assert!(dib_to_bmp(&[0; 12]).is_err());
        let mut bytes = vec![0; 40];
        bytes[..4].copy_from_slice(&124u32.to_le_bytes());
        assert!(dib_to_bmp(&bytes).is_err());
    }
    #[test]
    fn stored_thumbnail_is_small_lossless_and_bounded() {
        let pixels = image::RgbaImage::from_fn(96, 64, |x, y| {
            image::Rgba([x as u8, y as u8, (x ^ y) as u8, 127])
        });
        let png = encode_thumbnail(&pixels).unwrap();
        assert!(png.len() <= MAX_THUMBNAIL_BYTES);
        assert_eq!(decode_thumbnail(&png).unwrap(), pixels);
        assert!(decode_thumbnail(&vec![0; MAX_THUMBNAIL_BYTES + 1]).is_err());
        assert!(decode_thumbnail(b"broken PNG").is_err());
        let large = image::RgbaImage::new(97, 64);
        assert!(encode_thumbnail(&large).is_err());
        let mut png = Cursor::new(Vec::new());
        large.write_to(&mut png, image::ImageFormat::Png).unwrap();
        assert!(decode_thumbnail(png.get_ref()).is_err());
    }
    #[test]
    fn png_preview_is_in_memory() {
        let image = image::RgbaImage::from_pixel(2, 3, image::Rgba([1, 2, 3, 255]));
        let mut bytes = Cursor::new(vec![]);
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let content = ClipContent {
            formats: vec![FormatData {
                name: "image/png".into(),
                bytes: bytes.into_inner(),
            }],
            ..Default::default()
        };
        let small = thumbnail(&content).unwrap();
        assert!(small.width() <= 96 && small.height() <= 64);
        let result = generate("image".into(), content, 0);
        let (w, h, rgba) = result.image.unwrap();
        assert_eq!((w, h, rgba.len()), (2, 3, 24));
        assert!(result.error.is_none());
    }
}
