//! Opt-in PDFium runtime probe. Generates a two-page document in a temporary directory.
fn main() -> anyhow::Result<()> {
    use x_clipboard::{model::ClipContent, preview};
    let root = tempfile::tempdir()?;
    let path = root.path().join("two-pages.pdf");
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![0];
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 320 240] /Resources << >> /Contents 5 0 R >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 320 240] /Resources << >> /Contents 6 0 R >>",
        "<< /Length 30 >>\nstream\n0 0.5 0 rg 20 20 80 80 re f\n\nendstream",
        "<< /Length 30 >>\nstream\n0.5 0 0 rg 40 40 80 80 re f\n\nendstream",
    ];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend(format!("{} 0 obj\n{}\nendobj\n", index + 1, object).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend(b"xref\n0 7\n0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        pdf.extend(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend(
        format!("trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    std::fs::write(&path, pdf)?;
    let content = ClipContent {
        files: vec![path.to_string_lossy().into_owned()],
        ..Default::default()
    };
    let first = preview::generate("pdf".into(), content.clone(), 0);
    let second = preview::generate("pdf".into(), content, 1);
    anyhow::ensure!(first.error.is_none(), "first page: {:?}", first.error);
    anyhow::ensure!(second.error.is_none(), "second page: {:?}", second.error);
    anyhow::ensure!(
        first.pages == 2 && second.page == 1,
        "incorrect page navigation"
    );
    anyhow::ensure!(
        first.image.is_some() && second.image.is_some() && first.image != second.image,
        "pages not rendered distinctly"
    );
    println!("PASS: PDFium runtime loaded, two pages rendered, page navigation changes the image");
    Ok(())
}
