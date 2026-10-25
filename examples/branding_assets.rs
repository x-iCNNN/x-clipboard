//! Rebuilds the original vector application's PNG assets using GPUI's SVG renderer.
#[cfg(feature = "native")]
fn main() -> anyhow::Result<()> {
    use gpui_kit::{DevicePixels, SvgRenderer, SvgSize, size};
    let root = std::path::Path::new("assets/branding");
    let renderer = SvgRenderer::new(std::sync::Arc::new(gpui_kit::assets::AllAssets));
    for (file, prefix) in [
        ("app-icon.svg", "app-icon"),
        ("app-icon-mono.svg", "app-icon-mono"),
    ] {
        let svg = renderer.parse_svg(&std::fs::read(root.join(file))?)?;
        for pixels in [16, 24, 32, 48, 64, 128, 256, 512, 1024] {
            let image = renderer.render_parsed(
                &svg,
                SvgSize::ExactSize(size(DevicePixels(pixels), DevicePixels(pixels))),
            )?;
            let mut rgba = image.as_bytes(0).unwrap().to_vec();
            for pixel in rgba.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            image::save_buffer(
                root.join(format!("{prefix}-{pixels}.png")),
                &rgba,
                pixels as u32,
                pixels as u32,
                image::ColorType::Rgba8,
            )?;
        }
    }
    println!("Rendered original SVG icons at nine sizes.");
    Ok(())
}
#[cfg(not(feature = "native"))]
fn main() {
    eprintln!("Requires the native feature.");
}
