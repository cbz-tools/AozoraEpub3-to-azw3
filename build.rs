use std::path::Path;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(repository_fixture_tests)");
    println!("cargo:rerun-if-changed=Cargo.toml");

    // winresource derives FileVersion and ProductVersion from Cargo's package
    // metadata, so the release version has a single source of truth in Cargo.toml.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set("ProductName", "AozoraEpub3-to-azw3")
            .set("FileDescription", env!("CARGO_PKG_DESCRIPTION"))
            .set("OriginalFilename", "AozoraEpub3-to-azw3.exe")
            .compile()
            .expect("failed to compile Windows version resource");
    }

    let manifest_dir =
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by Cargo");
    let manifest_dir = Path::new(&manifest_dir);
    if manifest_dir.join("tests/fixtures/source-assets").is_dir() {
        println!("cargo:rustc-cfg=repository_fixture_tests");
    }

    println!("cargo:rerun-if-changed=tests/fixtures/source-assets");
    println!("cargo:rerun-if-changed=tests/fixtures/sovereign-stars/Sovereign_Stars_Vol_1.epub");
}
