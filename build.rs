//! Build script for configuring Windows linker flags and GPU driver exports.

fn main() {
    #[cfg(target_os = "windows")]
    {
        // Instruct MSVC linker to export GPU preference driver symbols in the PE export directory
        println!("cargo:rustc-link-arg-bins=/EXPORT:NvOptimusEnablement");
        println!("cargo:rustc-link-arg-bins=/EXPORT:AmdPowerXpressRequestHighPerformance");

        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "FileViewer");
        res.set("FileDescription", "FileViewer - Fast GPU-Accelerated Viewer & Editor");
        res.set("LegalCopyright", "Copyright © 2026");
        res.set("ProductVersion", "0.1.0");
        res.set("FileVersion", "0.1.0");
        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=Failed to compile Windows resource: {}", e);
        }
    }
}
