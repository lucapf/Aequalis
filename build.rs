//! The tables of names, written into `OUT_DIR` from `src/data/` and the unit table
//! (build/tables.rs). On Windows also the icon, manifest and version information of `aequalis.exe`
//! (apps/windows/res.rs).

#[path = "apps/windows/res.rs"]
mod res;

// The library's own unit table, JSON reader and names, as they are (each uses only the standard
// library).
#[allow(dead_code)]
#[path = "src/config.rs"]
mod config;
#[allow(dead_code)]
#[path = "src/json.rs"]
mod json;
#[allow(dead_code)]
#[path = "src/time/names.rs"]
mod names;
#[path = "build/tables.rs"]
mod tables;

fn main() {
    for file in [
        "build.rs",
        "apps/windows/res.rs",
        "build/tables.rs",
        "src/config.rs",
        "src/json.rs",
        "src/time/names.rs",
        "src/data/countries.json",
        "src/data/currencies.json",
        "src/data/places.json",
        "src/data/sizes.json",
    ] {
        println!("cargo:rerun-if-changed={file}");
    }
    tables::write(std::path::Path::new(&std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")));
    let version = env!("CARGO_PKG_VERSION");
    let manifest = res::manifest("Aequalis.CommandLine", false);
    res::embed("iconset/icons/windows/aequalis.ico", &manifest, &res::Version { version, description: "Aequalis (command line)", file_name: "aequalis.exe" });
}
