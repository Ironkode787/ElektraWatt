// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The program's icon in `elektrawatt.exe` (FT-1710): on a Windows MSVC target the build writes a compiled resource
//! file holding `assets/icons/app/elektrawatt.ico` (the rendering of the menu button's mark, `elektrawatt.svg`; the
//! test `app_icon` keeps the two equal) as icon group 1 and hands it to the linker, which takes a `.res` file as an
//! input (link.exe and lld-link alike). Explorer, the task bar and the setup's shortcut show the first icon group of
//! the executable. No resource compiler and no build dependency: the `.res` format is a sequence of headed records.
//! On every other target the script does nothing.

use std::path::Path;

/// The icon, relative to this package.
const ICO: &str = "../../assets/icons/app/elektrawatt.ico";

/// The resource types of an icon image and of an icon group.
const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;

fn main() {
    println!("cargo:rerun-if-changed={ICO}");
    println!("cargo:rerun-if-changed=build.rs");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|env| env == "msvc");
    if !(windows && msvc) {
        return;
    }
    let ico = std::fs::read(ICO).unwrap_or_else(|e| panic!("{ICO}: {e}"));
    let res = resource_file(&ico).unwrap_or_else(|e| panic!("{ICO}: {e}"));
    let out = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    let path = Path::new(&out).join("elektrawatt-icon.res");
    std::fs::write(&path, res).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    println!("cargo:rustc-link-arg-bins={}", path.display());
}

/// The `.res` file of the icon file `ico`: the empty leading record, one `RT_ICON` record per image (IDs from 1) and
/// the `RT_GROUP_ICON` record 1 that lists them.
fn resource_file(ico: &[u8]) -> Result<Vec<u8>, String> {
    let u16_at = |at: usize| -> Result<u16, String> {
        ico.get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .ok_or_else(|| "the icon file ends early".to_owned())
    };
    let u32_at = |at: usize| -> Result<u32, String> {
        ico.get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| "the icon file ends early".to_owned())
    };
    if u16_at(0)? != 0 || u16_at(2)? != 1 {
        return Err("not an icon file".to_owned());
    }
    let count = u16_at(4)?;
    let mut res = Vec::new();
    record(&mut res, 0, 0, &[]);
    let mut group = Vec::new();
    group.extend_from_slice(&[0, 0, 1, 0]);
    group.extend_from_slice(&count.to_le_bytes());
    for i in 0..count {
        let entry = 6 + 16 * usize::from(i);
        let size = u32_at(entry + 8)?;
        let offset = u32_at(entry + 12)?;
        let start = usize::try_from(offset).map_err(|e| e.to_string())?;
        let end = start + usize::try_from(size).map_err(|e| e.to_string())?;
        let image = ico.get(start..end).ok_or("an image lies outside the icon file")?;
        let id = i + 1;
        record(&mut res, RT_ICON, id, image);
        // The directory entry's first twelve bytes (width, height, colours, reserved, planes, bit count, size), then
        // the image's resource ID.
        group.extend_from_slice(&ico[entry..entry + 12]);
        group.extend_from_slice(&id.to_le_bytes());
    }
    record(&mut res, RT_GROUP_ICON, 1, &group);
    Ok(res)
}

/// Appends one resource record: its 32-byte header (data size, header size, the numeric type and name, data version,
/// memory flags moveable and discardable, language neutral, version, characteristics), the data, padding to four bytes.
fn record(res: &mut Vec<u8>, kind: u16, name: u16, data: &[u8]) {
    let size = u32::try_from(data.len()).expect("an icon image under 4 GiB");
    res.extend_from_slice(&size.to_le_bytes());
    res.extend_from_slice(&32u32.to_le_bytes());
    for id in [kind, name] {
        res.extend_from_slice(&0xFFFFu16.to_le_bytes());
        res.extend_from_slice(&id.to_le_bytes());
    }
    res.extend_from_slice(&0u32.to_le_bytes());
    let flags: u16 = if kind == 0 { 0 } else { 0x1010 };
    res.extend_from_slice(&flags.to_le_bytes());
    res.extend_from_slice(&0u16.to_le_bytes());
    res.extend_from_slice(&0u32.to_le_bytes());
    res.extend_from_slice(&0u32.to_le_bytes());
    res.extend_from_slice(data);
    while res.len() % 4 != 0 {
        res.push(0);
    }
}
