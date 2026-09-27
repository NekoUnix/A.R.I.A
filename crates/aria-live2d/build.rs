fn main() {
    println!("cargo:rerun-if-changed=vendor/purism-core/PurismCoreBundle.h");
    println!("cargo:rerun-if-changed=src/purism.c");
    cc::Build::new()
        .file("src/purism.c")
        .include("vendor/purism-core")
        .std("c11")
        .warnings(false)
        .compile("aria_purism_core");
    if std::env::var("CARGO_CFG_TARGET_FAMILY").as_deref() == Ok("unix") {
        println!("cargo:rustc-link-lib=m");
    }
}
