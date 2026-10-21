//! Windows resources for a program, written by its build script without any tool (no `rc.exe`):
//! its icon (from an `.ico` file), its manifest and its version information, as a `.res` file the
//! MSVC linker embeds. Shared by the build scripts of `aequalis.exe` and `aequalis-tray.exe`.

/// What a program's file says about it (Explorer's Details, Task Manager, Settings › Apps).
pub struct Version<'a> {
    /// `0.2.8`.
    pub version: &'a str,
    /// The program, as Task Manager names it.
    pub description: &'a str,
    /// `aequalis-tray.exe`.
    pub file_name: &'a str,
}

/// The `.res` file: the icon group 1 (the program's icon), the manifest 1, the version 1.
pub fn build(ico: &[u8], manifest: &str, version: &Version) -> Result<Vec<u8>, String> {
    const RT_ICON: u16 = 3;
    const RT_GROUP_ICON: u16 = 14;
    const RT_VERSION: u16 = 16;
    const RT_MANIFEST: u16 = 24;
    let mut out = Vec::new();
    // A .res file starts with an empty entry.
    resource(&mut out, 0, 0, &[]);
    let images = icon_images(ico)?;
    // The group: the icon directory, each entry naming its image by id instead of by offset.
    let mut group = vec![0, 0, 1, 0];
    group.extend_from_slice(&(images.len() as u16).to_le_bytes());
    for (i, (entry, data)) in images.iter().enumerate() {
        resource(&mut out, RT_ICON, i as u16 + 1, data);
        group.extend_from_slice(&entry[..12]);
        group.extend_from_slice(&(i as u16 + 1).to_le_bytes());
    }
    resource(&mut out, RT_GROUP_ICON, 1, &group);
    resource(&mut out, RT_MANIFEST, 1, manifest.as_bytes());
    resource(&mut out, RT_VERSION, 1, &version_info(version)?);
    Ok(out)
}

/// One resource: its header (ordinal type and name, US English), its data, padded to 4 bytes.
fn resource(out: &mut Vec<u8>, kind: u16, id: u16, data: &[u8]) {
    let words = |out: &mut Vec<u8>, ws: &[u16]| ws.iter().for_each(|w| out.extend_from_slice(&w.to_le_bytes()));
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&32u32.to_le_bytes());
    words(out, &[0xFFFF, kind, 0xFFFF, id]);
    out.extend_from_slice(&0u32.to_le_bytes()); // data version
    let (flags, language) = if kind == 0 { (0, 0) } else { (0x1030u16, 0x0409u16) };
    words(out, &[flags, language]);
    out.extend_from_slice(&[0; 8]); // version, characteristics
    out.extend_from_slice(data);
    pad(out);
}

fn pad(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

/// An image of an `.ico` file: its 16-byte directory entry and its data (PNG or DIB).
type IconImage<'a> = ([u8; 16], &'a [u8]);

/// The images of an `.ico` file.
fn icon_images(ico: &[u8]) -> Result<Vec<IconImage<'_>>, String> {
    let word = |at: usize| ico.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    if word(0) != Some(0) || word(2) != Some(1) {
        return Err("not an .ico file".into());
    }
    let count = usize::from(word(4).ok_or("not an .ico file")?);
    (0..count)
        .map(|i| {
            let entry: [u8; 16] = ico.get(6 + 16 * i..22 + 16 * i).and_then(|e| e.try_into().ok()).ok_or("an .ico entry is cut")?;
            let size = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
            let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
            Ok((entry, ico.get(offset..offset + size).ok_or("an .ico image is cut")?))
        })
        .collect()
}

/// `VS_VERSIONINFO`: the fixed part (the version as numbers) and the strings Windows shows.
fn version_info(v: &Version) -> Result<Vec<u8>, String> {
    let numbers: Vec<u16> =
        v.version.split(['.', '-', '+']).take(3).map(|n| n.parse().map_err(|_| format!("version `{}`", v.version))).collect::<Result<_, _>>()?;
    let [major, minor, patch] = numbers[..] else { return Err(format!("version `{}`", v.version)) };
    let (ms, ls) = ((u32::from(major) << 16) | u32::from(minor), u32::from(patch) << 16);
    let fixed: Vec<u8> = [0xFEEF_04BD, 0x0001_0000, ms, ls, ms, ls, 0x3F, 0, 0x0004_0004, 1, 0, 0, 0].iter().flat_map(|d: &u32| d.to_le_bytes()).collect();
    let strings = [
        ("CompanyName", "Luca Palanca Falsini"),
        ("FileDescription", v.description),
        ("FileVersion", v.version),
        ("InternalName", v.file_name.trim_end_matches(".exe")),
        ("LegalCopyright", "MIT License"),
        ("OriginalFilename", v.file_name),
        ("ProductName", "Aequalis"),
        ("ProductVersion", v.version),
    ];
    let texts = strings.iter().map(|(key, value)| text_block(key, value)).collect();
    let table = block("040904b0", &[], 0, 1, texts);
    let string_info = block("StringFileInfo", &[], 0, 1, vec![table]);
    // US English, Unicode.
    let translation = block("Translation", &0x04B0_0409u32.to_le_bytes(), 4, 0, Vec::new());
    let var_info = block("VarFileInfo", &[], 0, 1, vec![translation]);
    Ok(block("VS_VERSION_INFO", &fixed, fixed.len() as u16, 0, vec![string_info, var_info]))
}

/// A version block: its length, its value's length, its type (1: text), its key, its value and its
/// children, each starting on 4 bytes.
fn block(key: &str, value: &[u8], value_length: u16, kind: u16, children: Vec<Vec<u8>>) -> Vec<u8> {
    let mut out = vec![0, 0];
    out.extend_from_slice(&value_length.to_le_bytes());
    out.extend_from_slice(&kind.to_le_bytes());
    key.encode_utf16().chain([0]).for_each(|w| out.extend_from_slice(&w.to_le_bytes()));
    pad(&mut out);
    out.extend_from_slice(value);
    for child in children {
        pad(&mut out);
        out.extend_from_slice(&child);
    }
    let length = out.len() as u16;
    out[..2].copy_from_slice(&length.to_le_bytes());
    out
}

/// A string of the version: its value's length counts UTF-16 units, the NUL included.
fn text_block(key: &str, value: &str) -> Vec<u8> {
    let units: Vec<u16> = value.encode_utf16().chain([0]).collect();
    let bytes: Vec<u8> = units.iter().flat_map(|w| w.to_le_bytes()).collect();
    block(key, &bytes, units.len() as u16, 1, Vec::new())
}

/// The manifest: Windows 10 and 11, UTF-8, long paths; for a window, the current common controls
/// and sharp text on every monitor (`gui`).
pub fn manifest(name: &str, gui: bool) -> String {
    let controls = if gui {
        r#"
  <dependency><dependentAssembly><assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/></dependentAssembly></dependency>"#
    } else {
        ""
    };
    let dpi = if gui {
        r#"
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>"#
    } else {
        ""
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="{name}" version="1.0.0.0"/>{controls}
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1"><application><supportedOS Id="{{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}}"/></application></compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>{dpi}
      <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
      <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
    </windowsSettings>
  </application>
</assembly>
"#
    )
}

/// For a build script: writes the `.res` of this program into `OUT_DIR` and has the linker embed it
/// in the program (only when building for Windows with MSVC; nothing elsewhere).
pub fn embed(ico_path: &str, manifest: &str, version: &Version) {
    println!("cargo:rerun-if-changed={ico_path}");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    if !windows || std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }
    let ico = std::fs::read(ico_path).unwrap_or_else(|e| panic!("{ico_path}: {e}"));
    let res = build(&ico, manifest, version).unwrap_or_else(|e| panic!("{ico_path}: {e}"));
    let path = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("resources.res");
    std::fs::write(&path, res).unwrap();
    println!("cargo:rustc-link-arg-bins={}", path.display());
    // The manifest is the one above, not one the linker would make.
    println!("cargo:rustc-link-arg-bins=/MANIFEST:NO");
}
