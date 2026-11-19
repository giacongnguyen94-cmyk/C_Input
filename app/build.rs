fn main() {
    println!("cargo:rerun-if-changed=res/app.ico");
    println!("cargo:rerun-if-changed=res/tray_on.ico");
    println!("cargo:rerun-if-changed=res/tray_off.ico");
    println!("cargo:rerun-if-changed=res/app.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("res/app.ico")
        .set_icon_with_id("res/tray_on.ico", "101")
        .set_icon_with_id("res/tray_off.ico", "102")
        .set_manifest_file("res/app.manifest")
        .set("ProductName", "C_Input")
        .set("FileDescription", "C_Input - Bộ gõ tiếng Việt")
        .set("CompanyName", "C_Input")
        .set("LegalCopyright", "C_Input")
        .set("OriginalFilename", "C_Input.exe");
    res.compile().expect("không nhúng được tài nguyên (icon, manifest)");
}
