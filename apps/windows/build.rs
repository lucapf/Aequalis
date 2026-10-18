//! The icon, manifest and version information of `aequalis-tray.exe` (res.rs).

mod res;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=res.rs");
    let manifest = res::manifest("Aequalis.Tray", true);
    res::embed(
        "../../iconset/icons/windows/aequalis.ico",
        &manifest,
        &res::Version { version: env!("CARGO_PKG_VERSION"), description: "Aequalis", file_name: "aequalis-tray.exe" },
    );
}
