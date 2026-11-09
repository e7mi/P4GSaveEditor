fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/p4g.ico");
        res.set("ProductName", "P4G Save Editor");
        res.set("FileDescription", "Persona 4 Golden Save Editor");
        res.set("FileVersion", env!("CARGO_PKG_VERSION"));
        res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
        if let Err(error) = res.compile() {
            println!("cargo:warning=icon resource: {error}");
        }
    }
}
