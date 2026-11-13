// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded EDZ archive reading. No archive path is ever used as a filesystem path.

use std::collections::BTreeSet;
use std::io::{Cursor, Read};

use crate::{MAX_FILE, MAX_INPUT, MAX_TOTAL};

pub(crate) fn safe_name(name: &str) -> Result<String, String> {
    let name = name.replace('\\', "/");
    if name.len() > 1024
        || name.contains(':')
        || name.chars().any(char::is_control)
        || name.split('/').any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err("eplan-archive-path".into());
    }
    Ok(name)
}

pub(crate) fn read(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    read_matching(bytes, &|_| true).map(|(files, _)| files)
}

/// The members `keep` names, and the names of the others, which are counted into the total and the ratio but never
/// opened: a member above [`MAX_FILE`] that nobody reads (a 3D model, a picture) does not refuse the archive.
pub(crate) fn read_matching(
    bytes: &[u8],
    keep: &dyn Fn(&str) -> bool,
) -> Result<(Vec<(String, Vec<u8>)>, Vec<String>), String> {
    if bytes.len() as u64 > MAX_INPUT {
        return Err("eplan-input-limit".into());
    }
    if bytes.starts_with(b"PK") {
        return zip(bytes, keep);
    }
    use sevenz_rust2::{ArchiveReader, Password};
    crate::seven::check(bytes)?;
    let mut reader = ArchiveReader::new(Cursor::new(bytes), Password::empty())
        .map_err(|_| "eplan-archive-invalid".to_owned())?;
    reader.set_thread_count(1);
    let mut decoded = 0u64;
    for block in &reader.archive().blocks {
        decoded = decoded
            .checked_add(block.get_unpack_size())
            .ok_or("eplan-input-limit")?;
        if decoded > MAX_TOTAL
            || block
                .coders
                .iter()
                .any(|c| block.get_unpack_size_for_coder(c) > MAX_TOTAL)
        {
            return Err("eplan-input-limit".into());
        }
    }
    let mut total = 0u64;
    let mut names = BTreeSet::new();
    for file in &reader.archive().files {
        if file.is_directory {
            continue;
        }
        let name = safe_name(&file.name)?;
        if !names.insert(name.to_ascii_lowercase()) || file.is_anti_item {
            return Err("eplan-archive-path".into());
        }
        total = total.checked_add(file.size).ok_or("eplan-input-limit")?;
        if (file.size > MAX_FILE && keep(&name)) || total > MAX_TOTAL {
            return Err("eplan-input-limit".into());
        }
    }
    if total > (bytes.len() as u64).saturating_mul(200) {
        return Err("eplan-input-limit".into());
    }
    let mut files = Vec::new();
    let mut skipped = Vec::new();
    reader
        .for_each_entries(|entry, stream| {
            if !entry.is_directory && !keep(&entry.name.replace('\\', "/")) {
                skipped.push(entry.name.replace('\\', "/"));
            } else if !entry.is_directory {
                let mut data = Vec::new();
                stream.take(MAX_FILE + 1).read_to_end(&mut data)?;
                if data.len() as u64 != entry.size || data.len() as u64 > MAX_FILE {
                    return Err(sevenz_rust2::Error::Other("entry size mismatch".into()));
                }
                files.push((entry.name.replace('\\', "/"), data));
            }
            Ok(true)
        })
        .map_err(|_| "eplan-archive-invalid".to_owned())?;
    Ok((files, skipped))
}

fn u16_at(b: &[u8], at: usize) -> Result<usize, String> {
    Ok(u16::from_le_bytes(
        b.get(at..at + 2)
            .ok_or("eplan-archive-invalid")?
            .try_into()
            .map_err(|_| "eplan-archive-invalid")?,
    ) as usize)
}
fn u32_at(b: &[u8], at: usize) -> Result<usize, String> {
    Ok(u32::from_le_bytes(
        b.get(at..at + 4)
            .ok_or("eplan-archive-invalid")?
            .try_into()
            .map_err(|_| "eplan-archive-invalid")?,
    ) as usize)
}

/// Stored and raw-deflate ZIP, including ordinary comments and data descriptors. ZIP64/encryption refused.
fn zip(b: &[u8], keep: &dyn Fn(&str) -> bool) -> Result<(Vec<(String, Vec<u8>)>, Vec<String>), String> {
    let end = (b.len().saturating_sub(65557)..b.len().saturating_sub(21))
        .rev()
        .find(|&i| {
            b.get(i..i + 4) == Some(b"PK\x05\x06")
                && u16_at(b, i + 20).ok().is_some_and(|n| i + 22 + n == b.len())
        })
        .ok_or("eplan-archive-invalid")?;
    let count = u16_at(b, end + 10)?;
    if count > 4096 || u16_at(b, end + 4)? != 0 || u16_at(b, end + 6)? != 0 || u16_at(b, end + 8)? != count {
        return Err("eplan-input-limit".into());
    }
    let start = u32_at(b, end + 16)?;
    if start.checked_add(u32_at(b, end + 12)?) != Some(end) {
        return Err("eplan-archive-invalid".into());
    }
    let mut at = start;
    let mut total = 0u64;
    let mut names = BTreeSet::new();
    let mut result = Vec::new();
    let mut skipped = Vec::new();
    let mut ranges = Vec::new();
    for _ in 0..count {
        if b.get(at..at + 4) != Some(b"PK\x01\x02") {
            return Err("eplan-archive-invalid".into());
        }
        let flags = u16_at(b, at + 8)?;
        let method = u16_at(b, at + 10)?;
        let crc = u32_at(b, at + 16)? as u32;
        let compressed = u32_at(b, at + 20)?;
        let size = u32_at(b, at + 24)?;
        let len = u16_at(b, at + 28)?;
        let extra = u16_at(b, at + 30)?;
        let comment = u16_at(b, at + 32)?;
        let local = u32_at(b, at + 42)?;
        let name = std::str::from_utf8(b.get(at + 46..at + 46 + len).ok_or("eplan-archive-invalid")?)
            .map_err(|_| "eplan-archive-path")?;
        at = at
            .checked_add(46 + len + extra + comment)
            .filter(|v| *v <= end)
            .ok_or("eplan-archive-invalid")?;
        if flags & (1 | 64) != 0 || !matches!(method, 0 | 8) {
            return Err("eplan-archive-method".into());
        }
        if name.ends_with('/') && size == 0 {
            continue;
        }
        let name = safe_name(name)?;
        if !names.insert(name.to_ascii_lowercase()) {
            return Err("eplan-archive-path".into());
        }
        total = total.checked_add(size as u64).ok_or("eplan-input-limit")?;
        let wanted = keep(&name);
        if (size as u64 > MAX_FILE && wanted)
            || total > MAX_TOTAL
            || size > compressed.saturating_mul(200).max(1024)
        {
            return Err("eplan-input-limit".into());
        }
        if b.get(local..local + 4) != Some(b"PK\x03\x04")
            || u16_at(b, local + 6)? != flags
            || u16_at(b, local + 8)? != method
        {
            return Err("eplan-archive-invalid".into());
        }
        let local_len = u16_at(b, local + 26)?;
        let local_extra = u16_at(b, local + 28)?;
        let local_name = std::str::from_utf8(
            b.get(local + 30..local + 30 + local_len)
                .ok_or("eplan-archive-invalid")?,
        )
        .map_err(|_| "eplan-archive-path")?;
        if safe_name(local_name)? != name {
            return Err("eplan-archive-invalid".into());
        }
        let data_start = local
            .checked_add(30 + local_len + local_extra)
            .ok_or("eplan-archive-invalid")?;
        let data_end = data_start
            .checked_add(compressed)
            .filter(|v| *v <= start)
            .ok_or("eplan-archive-invalid")?;
        if ranges.iter().any(|&(a, z)| local < z && data_end > a) {
            return Err("eplan-archive-invalid".into());
        }
        ranges.push((local, data_end));
        if !wanted {
            skipped.push(name);
            continue;
        }
        let data = b.get(data_start..data_end).ok_or("eplan-archive-invalid")?;
        let data = if method == 0 {
            data.to_vec()
        } else {
            miniz_oxide::inflate::decompress_to_vec_with_limit(data, size)
                .map_err(|_| "eplan-archive-invalid")?
        };
        if data.len() != size || crc32fast::hash(&data) != crc {
            return Err("eplan-archive-invalid".into());
        }
        result.push((name, data));
    }
    if at != end {
        return Err("eplan-archive-invalid".into());
    }
    Ok((result, skipped))
}
