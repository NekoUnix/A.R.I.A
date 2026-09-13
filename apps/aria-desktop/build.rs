fn main() {
    let target = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    if target == "macos" {
        println!("cargo:rerun-if-changed=native/syphon.m");
        cc::Build::new()
            .file("native/syphon.m")
            .flag("-fobjc-arc")
            .compile("aria_syphon");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=Metal");
    } else if target == "linux" {
        println!("cargo:rerun-if-changed=../../native/linux-canvas/transport.c");
        println!("cargo:rerun-if-changed=../../native/linux-canvas/transport.h");
        cc::Build::new()
            .file("../../native/linux-canvas/transport.c")
            .flag("-std=c11")
            .warnings(true)
            .compile("aria_canvas");
    }
}
