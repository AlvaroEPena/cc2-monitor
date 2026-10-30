//! Windows: embed the app icon and version info into the .exe.
//! Other platforms ship the icon through their desktop entry instead.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "CC2 Monitor");
        res.set("FileDescription", "CC2 Monitor");
        res.compile().expect("embed Windows resources");
    }
}
