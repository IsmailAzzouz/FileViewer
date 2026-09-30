//! Build script for configuring Windows linker flags and GPU driver exports.

fn main() {
    #[cfg(target_os = "windows")]
    {
        // Instruct MSVC linker to export GPU preference driver symbols in the PE export directory
        println!("cargo:rustc-link-arg-bins=/EXPORT:NvOptimusEnablement");
        println!("cargo:rustc-link-arg-bins=/EXPORT:AmdPowerXpressRequestHighPerformance");
    }
}
