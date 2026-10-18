//! On Windows: the icon, manifest and version information of `aequalis.exe` (apps/windows/res.rs).
//! Nothing elsewhere.

#[path = "apps/windows/res.rs"]
mod res;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=apps/windows/res.rs");
    let version = env!("CARGO_PKG_VERSION");
    let manifest = res::manifest("Aequalis.CommandLine", false);
    res::embed("iconset/icons/windows/aequalis.ico", &manifest, &res::Version { version, description: "Aequalis (command line)", file_name: "aequalis.exe" });
}
