//! Embeds the app icon and version info in zenith.exe.

fn main() {
    const ICON: &str = "../../assets/icon/zenith.ico";
    println!("cargo:rerun-if-changed={ICON}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon(ICON)
        .set("ProductName", "Zenith")
        .set("FileDescription", "Zenith")
        .set("LegalCopyright", "MIT License");
    res.compile().expect("embedding the icon resource");
}
