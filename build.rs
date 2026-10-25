fn main() {
    println!("cargo:rerun-if-changed=assets/branding/app-icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/branding/app-icon.ico")
            .set("ProductName", "X Clipboard")
            .set("FileDescription", "X Clipboard")
            .compile()
            .expect("Windows application icon resource must compile");
    }
}
