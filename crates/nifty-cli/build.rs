//! Embeds the application manifest the Windows backend needs into `nf` and `nf-runner`:
//! it selects Common Controls 6, for modern controls, and per-monitor DPI awareness.

use std::env;
use std::path::Path;

const MANIFEST: &str = "windows/nifty.manifest";

fn main() {
    println!("cargo::rerun-if-changed={MANIFEST}");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    assert!(
        env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc"),
        "Nifty's Windows apps are linked with the MSVC toolchain, which embeds their manifest"
    );
    let crate_dir = env::var("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR");
    let manifest = Path::new(&crate_dir).join(MANIFEST);
    println!("cargo::rustc-link-arg-bins=/MANIFEST:EMBED");
    println!(
        "cargo::rustc-link-arg-bins=/MANIFESTINPUT:{}",
        manifest.display()
    );
}
