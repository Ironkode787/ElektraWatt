// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Read-only subset of the observed SLK 1.2 / EOX FE53 symbol database.
//! Only the companion drawing tables are read. Unknown records remain explicit import findings.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use crate::{Budget, ImportedSymbol, MAX_FILE, MAX_TOTAL, Pin};

/// The only companion files the reader needs. User, lock and workstation records are never opened.
pub const FILES: [&str; 8] = [
    "Symbol.eox",
    "Symbol.eod",
    "Variant.eox",
    "Variant.eod",
    "Instance.eox",
    "Instance.eod",
    "Blob.eox",
    "Blob.eod",
];

const INVALID: &str = "eplan-slk-invalid";
const LIMIT: &str = "eplan-input-limit";

/// The single sibling database directory named by an SLK 1.2 descriptor.
///
/// # Errors
/// Unknown encoding/version, absolute paths, traversal and paths outside a single sibling directory.
pub fn directory(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() > 8192 {
        return Err(LIMIT.into());
    }
    let text = if bytes.starts_with(&[0xff, 0xfe]) {
        if !bytes.len().is_multiple_of(2) {
            return Err(INVALID.into());
        }
        String::from_utf16(
            &bytes[2..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect::<Vec<_>>(),
        )
        .map_err(|_| INVALID)?
    } else {
        std::str::from_utf8(bytes)
            .map_err(|_| "eplan-slk-encoding")?
            .to_owned()
    };
    let mut lines = text.trim_start_matches('\u{feff}').lines();
    let name = lines.next().ok_or(INVALID)?.trim();
    if lines.next().map(str::trim) != Some("1.2") || lines.any(|s| !s.trim().is_empty()) {
        return Err("eplan-slk-version".into());
    }
    if name.is_empty()
        || !name.to_ascii_lowercase().ends_with(".sdb")
        || name.starts_with('.')
        || name.ends_with([' ', '.'])
        || name.chars().any(|c| c.is_control() || "/\\:<>\"|?*".contains(c))
    {
        return Err("eplan-archive-path".into());
    }
    Ok(name.to_owned())
}

fn be16(b: &[u8], at: usize) -> Result<u16, String> {
    Ok(u16::from_be_bytes(
        b.get(at..at + 2)
            .ok_or(INVALID)?
            .try_into()
            .map_err(|_| INVALID)?,
    ))
}
fn be32(b: &[u8], at: usize) -> Result<u32, String> {
    Ok(u32::from_be_bytes(
        b.get(at..at + 4)
            .ok_or(INVALID)?
            .try_into()
            .map_err(|_| INVALID)?,
    ))
}

fn records<'a>(files: &'a BTreeMap<String, Vec<u8>>, table: &str) -> Result<Vec<&'a [u8]>, String> {
    let index = files.get(&format!("{table}.eox")).ok_or(INVALID)?;
    let data = files.get(&format!("{table}.eod")).ok_or(INVALID)?;
    if index.get(..8) != Some(&[0xfe, 0x53, 2, 2, 4, 4, 15, 255]) {
        return Err("eplan-slk-version".into());
    }
    let size = index
        .get(12..15)
        .ok_or(INVALID)?
        .iter()
        .fold(0usize, |n, b| n * 256 + usize::from(*b))
        + 1;
    if !(32..=4096).contains(&size) {
        return Err("eplan-slk-version".into());
    }
    if data.is_empty() || data == &[0] {
        return Ok(Vec::new());
    }
    if !data.len().is_multiple_of(size) || data.len() / size > 65536 {
        return Err(INVALID.into());
    }
    let rows: Vec<_> = data.chunks_exact(size).collect();
    if rows.iter().any(|r| !matches!(r.last(), Some(0 | 10))) {
        return Err(INVALID.into());
    }
    Ok(rows)
}

type Blobs<'a> = BTreeMap<(u16, u32), BTreeMap<u16, &'a [u8]>>;

fn blobs<'a>(rows: &[&'a [u8]]) -> Result<Blobs<'a>, String> {
    let mut out: Blobs<'_> = BTreeMap::new();
    for row in rows {
        if row.len() != 256 {
            return Err("eplan-slk-version".into());
        }
        let table = be16(row, 0)?;
        // Ignore database metadata, not drawing streams.
        if ![7, 12, 16].contains(&table) {
            continue;
        }
        let key = (table, be32(row, 2)?);
        if out
            .entry(key)
            .or_default()
            .insert(be16(row, 6)?, &row[11..255])
            .is_some()
        {
            return Err(INVALID.into());
        }
    }
    Ok(out)
}

fn stream(row: &[u8], offset: usize, table: u16, blobs: &Blobs<'_>) -> Result<Vec<u8>, String> {
    let mut data = row.get(offset..row.len() - 1).ok_or(INVALID)?.to_vec();
    if let Some(chunks) = blobs.get(&(table, be32(row, 0)?)) {
        for (i, (seq, chunk)) in chunks.iter().enumerate() {
            if usize::from(*seq) != i {
                return Err(INVALID.into());
            }
            data.extend_from_slice(chunk);
            if data.len() as u64 > MAX_FILE {
                return Err(LIMIT.into());
            }
        }
    }
    match data.get(..2) {
        Some([255, 255]) => Ok(data[2..].to_vec()),
        Some([0, 1] | [0, 2]) => {
            let capacity =
                u32::from_le_bytes(data.get(2..6).ok_or(INVALID)?.try_into().map_err(|_| INVALID)?) as usize;
            let length =
                u32::from_le_bytes(data.get(6..10).ok_or(INVALID)?.try_into().map_err(|_| INVALID)?) as usize;
            if capacity as u64 > MAX_FILE
                || length as u64 > MAX_FILE
                || capacity > length.saturating_mul(200).max(1024)
            {
                return Err(LIMIT.into());
            }
            let compressed = data.get(10..10 + length).ok_or(INVALID)?;
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(compressed, capacity)
                .map_err(|_| INVALID.into())
        }
        _ => Err("eplan-slk-record-unsupported".into()),
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let b = self.bytes.get(self.at..self.at + n).ok_or(INVALID)?;
        self.at += n;
        Ok(b)
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().map_err(|_| INVALID)?))
    }
    fn f64(&mut self) -> Result<f64, String> {
        let value = f64::from_le_bytes(self.take(8)?.try_into().map_err(|_| INVALID)?);
        if !value.is_finite() || value.abs() > 1_000_000.0 {
            return Err("eplan-geometry-invalid".into());
        }
        Ok(value)
    }
    fn point(&mut self) -> Result<[i64; 2], String> {
        Ok([
            (self.f64()? * 1e6).round() as i64,
            (-self.f64()? * 1e6).round() as i64,
        ])
    }
}

fn graphic(data: &[u8], class: u16) -> Result<Option<Value>, String> {
    if ![31, 32, 89].contains(&class) {
        return Ok(None);
    }
    let mut c = Cursor { bytes: data, at: 0 };
    let (mut a, mut b) = (None, None);
    let (mut start, mut end, mut rotation) = (0.0, 0.0, 0.0);
    while c.at + 2 <= c.bytes.len() {
        match c.u16()? {
            0 => break,
            60001 | 60401 => {
                c.take(6)?;
            }
            60404 => {
                c.take(1)?;
            }
            60411 | 60412 => {
                c.take(2)?;
            }
            60531 | 60555 | 61651 => a = Some(c.point()?),
            60532 | 60556 | 61652 => b = Some(c.point()?),
            60553 => start = c.f64()?,
            60554 => end = c.f64()?,
            61653 => rotation = c.f64()?,
            _ => return Ok(None),
        }
    }
    let (Some(a), Some(b)) = (a, b) else {
        return Ok(None);
    };
    let stroke = json!({"pen":"p026"});
    Ok(match class {
        31 => Some(json!({"line":{"from":a,"to":b,"stroke":stroke}})),
        89 if rotation.abs() < 1e-8 => Some(
            json!({"rect":{"rect":[[a[0].min(b[0]),a[1].min(b[1])],[a[0].max(b[0]),a[1].max(b[1])]],"stroke":stroke}}),
        ),
        32 if (start - end).abs() < 1e-8 || ((start - end).abs() - std::f64::consts::TAU).abs() < 1e-8 => {
            let r = (b[0] as f64).hypot(b[1] as f64).round() as i64;
            (r > 0).then(|| json!({"circle":{"centre":a,"r":r,"stroke":stroke}}))
        }
        _ => None,
    })
}

fn pins(data: &[u8], budget: &mut Budget) -> Result<Vec<Pin>, String> {
    let mut c = Cursor { bytes: data, at: 0 };
    let mut pins = BTreeMap::new();
    while c.at + 2 <= c.bytes.len() {
        match c.u16()? {
            60001 | 60821 => {
                c.take(6)?;
            }
            60003 => {
                c.take(2)?;
            }
            60404 => {
                c.take(1)?;
            }
            60823 => {
                if c.u16()? != 48 {
                    return Err("eplan-slk-record-unsupported".into());
                }
                let (mut index, mut pos, mut dir) = (0, [0, 0], None);
                loop {
                    match c.u16()? {
                        65535 => break,
                        60861 => index = c.u16()?,
                        60862 => pos = c.point()?,
                        60863 => {
                            dir = match c.take(1)?[0] {
                                1 => Some("up"),
                                2 => Some("right"),
                                4 => Some("down"),
                                8 => Some("left"),
                                _ => None,
                            }
                        }
                        _ => return Err("eplan-slk-record-unsupported".into()),
                    }
                }
                budget.item(pins.len())?;
                if pins.len() >= 4096
                    || pins
                        .insert(
                            index,
                            Pin {
                                designation: (u32::from(index) + 1).to_string(),
                                position: pos,
                                direction: dir.map(str::to_owned),
                            },
                        )
                        .is_some()
                {
                    return Err(INVALID.into());
                }
            }
            // Following structures contain text/property placements, not symbol connections.
            0 | 60005 => break,
            _ => return Err("eplan-slk-record-unsupported".into()),
        }
    }
    Ok(pins.into_values().collect())
}

/// Decode the supported drawing subset from fixed companion tables, without EPLAN or file execution.
/// The A variant is imported; EW generates its eight orientations. Electrical meaning requires review.
///
/// # Errors
/// Malformed/unsupported tables, duplicate identities, broken continuation streams or resource limits.
pub fn read(files: &BTreeMap<String, Vec<u8>>) -> Result<Vec<ImportedSymbol>, String> {
    if files.values().any(|b| b.len() as u64 > MAX_FILE)
        || files.values().map(|b| b.len() as u64).sum::<u64>() > MAX_TOTAL
    {
        return Err(LIMIT.into());
    }
    let symbols = records(files, "Symbol")?;
    let variants = records(files, "Variant")?;
    let instances = records(files, "Instance")?;
    let blobs = blobs(&records(files, "Blob")?)?;
    let mut names = BTreeMap::new();
    for r in symbols {
        let id = be32(r, 0)?;
        if id == 0 || be16(r, 4)? != 20 {
            return Err(INVALID.into());
        }
        let name = r
            .get(26..90)
            .ok_or(INVALID)?
            .split(|b| *b == 0)
            .next()
            .unwrap_or_default();
        let name = std::str::from_utf8(name)
            .ok()
            .filter(|s| !s.is_empty() && !s.chars().any(char::is_control))
            .map(str::to_owned)
            .unwrap_or_else(|| format!("Symbol {id}"));
        if names.insert(id, name).is_some() {
            return Err(INVALID.into());
        }
    }
    let mut out = BTreeMap::new();
    let mut decoded = 0u64;
    let mut budget = Budget::default();
    let mut seen = BTreeSet::new();
    for r in variants {
        let id = be32(r, 0)?;
        if id == 0 || be16(r, 4)? != 45 || be16(r, 24)? != 20 || !seen.insert(id) {
            return Err(INVALID.into());
        }
        let name = names.get(&be32(r, 26)?).ok_or(INVALID)?;
        if r.get(30..32) != Some(&[0, 0]) {
            continue;
        }
        let data = stream(r, 32, 16, &blobs)?;
        decoded += data.len() as u64;
        if decoded > MAX_TOTAL {
            return Err(LIMIT.into());
        }
        let mut notes = vec![
            "eplan-slk-subset".into(),
            "eplan-function-review".into(),
            "eplan-style-review".into(),
        ];
        let pins = match pins(&data, &mut budget) {
            Ok(pins) => pins,
            Err(e) if e == "eplan-slk-record-unsupported" => {
                notes.push("eplan-slk-pins-review".into());
                Vec::new()
            }
            Err(e) => return Err(e),
        };
        budget.symbol()?;
        out.insert(
            id,
            ImportedSymbol {
                source: "SLK".into(),
                name: name.clone(),
                graphics: Vec::new(),
                pins,
                notes,
            },
        );
    }
    seen.clear();
    for r in instances {
        if be32(r, 0)? == 0 || !seen.insert(be32(r, 0)?) {
            return Err(INVALID.into());
        }
        if be16(r, 24)? != 45 {
            continue;
        }
        let Some(symbol) = out.get_mut(&be32(r, 26)?) else {
            continue;
        };
        let data = stream(r, 31, 7, &blobs)?;
        decoded += data.len() as u64;
        if decoded > MAX_TOTAL {
            return Err(LIMIT.into());
        }
        match graphic(&data, be16(r, 4)?)? {
            Some(g) => {
                budget.item(symbol.graphics.len())?;
                symbol.graphics.push(g);
            }
            None => {
                if !symbol.notes.iter().any(|n| n == "eplan-slk-graphics-incomplete") {
                    symbol.notes.push("eplan-slk-graphics-incomplete".into());
                }
            }
        }
    }
    let result: Vec<_> = out.into_values().collect();
    if result.is_empty() {
        return Err("eplan-no-symbols".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(size: usize) -> Vec<u8> {
        let mut h = vec![0; 32];
        h[..8].copy_from_slice(&[0xfe, 0x53, 2, 2, 4, 4, 15, 255]);
        h[12..15].copy_from_slice(&((size - 1) as u32).to_be_bytes()[1..]);
        h
    }
    fn record(size: usize, class: u16, parent: u16) -> Vec<u8> {
        let mut r = vec![0; size];
        r[..4].copy_from_slice(&1u32.to_be_bytes());
        r[4..6].copy_from_slice(&class.to_be_bytes());
        r[24..26].copy_from_slice(&parent.to_be_bytes());
        r[26..30].copy_from_slice(&1u32.to_be_bytes());
        r[size - 1] = 10;
        r
    }
    fn fixture() -> BTreeMap<String, Vec<u8>> {
        let mut files = BTreeMap::new();
        for (table, size) in [
            ("Symbol", 241),
            ("Variant", 535),
            ("Instance", 134),
            ("Blob", 256),
        ] {
            files.insert(format!("{table}.eox"), header(size));
            files.insert(format!("{table}.eod"), vec![0]);
        }
        let mut s = record(241, 20, 0);
        s[26..90].fill(0);
        s[26..31].copy_from_slice(b"TEST\0");
        files.insert("Symbol.eod".into(), s);
        let mut v = record(535, 45, 20);
        let mut p = vec![255, 255];
        for x in [60823u16, 48, 60862] {
            p.extend(x.to_le_bytes());
        }
        p.extend(0f64.to_le_bytes());
        p.extend(0f64.to_le_bytes());
        p.extend(60863u16.to_le_bytes());
        p.push(8);
        p.extend(65535u16.to_le_bytes());
        v[32..32 + p.len()].copy_from_slice(&p);
        files.insert("Variant.eod".into(), v);
        let mut i = record(134, 31, 45);
        let mut p = vec![255, 255];
        for (tag, point) in [(60531u16, [0f64, 0f64]), (60532, [8f64, 0f64])] {
            p.extend(tag.to_le_bytes());
            for n in point {
                p.extend(n.to_le_bytes());
            }
        }
        i[31..31 + p.len()].copy_from_slice(&p);
        files.insert("Instance.eod".into(), i);
        files
    }

    #[test]
    fn native_tables_recover_drawing_and_direction_without_eplan() {
        let imported = read(&fixture()).unwrap();
        let s = &imported[0];
        assert_eq!(s.name, "TEST");
        assert_eq!(s.graphics[0]["line"]["to"], json!([8_000_000, 0]));
        assert_eq!(s.pins[0].direction.as_deref(), Some("left"));
        assert_eq!(s.pins[0].position, [0, 0]);
        assert!(s.notes.iter().any(|n| n == "eplan-function-review"));
    }

    #[test]
    fn descriptor_and_table_bounds_fail_closed() {
        assert_eq!(directory(b"Test.sdb\n1.2\n").unwrap(), "Test.sdb");
        let utf16: Vec<_> = [0xff, 0xfe]
            .into_iter()
            .chain("Test.sdb\n1.2".encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        assert_eq!(directory(&utf16).unwrap(), "Test.sdb");
        for (s, key) in [
            ("../Test.sdb\n1.2", "eplan-archive-path"),
            ("C:\\Test.sdb\n1.2", "eplan-archive-path"),
            ("Test.sdb\n2.0", "eplan-slk-version"),
            ("Test.sdb\n1.2\nextra", "eplan-slk-version"),
        ] {
            assert_eq!(directory(s.as_bytes()).unwrap_err(), key);
        }
        let mut f = fixture();
        f.get_mut("Instance.eod").unwrap().pop();
        assert_eq!(read(&f).unwrap_err(), INVALID);
        let mut f = fixture();
        let row = f["Instance.eod"].clone();
        f.get_mut("Instance.eod").unwrap().extend(row);
        assert_eq!(read(&f).unwrap_err(), INVALID);
        let mut f = fixture();
        f.get_mut("Instance.eox").unwrap()[12..15].fill(255);
        assert_eq!(read(&f).unwrap_err(), "eplan-slk-version");
    }

    #[test]
    fn compressed_stream_and_corruption_are_checked() {
        let raw = vec![0u8; 100];
        let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);
        let mut row = record(134, 31, 45);
        let mut data = vec![0, 2];
        data.extend(244u32.to_le_bytes());
        data.extend((compressed.len() as u32).to_le_bytes());
        data.extend(&compressed);
        row[31..31 + data.len()].copy_from_slice(&data);
        assert_eq!(stream(&row, 31, 7, &BTreeMap::new()).unwrap(), raw);
        row[41] ^= 255;
        assert_eq!(stream(&row, 31, 7, &BTreeMap::new()).unwrap_err(), INVALID);
        row[33..37].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(stream(&row, 31, 7, &BTreeMap::new()).unwrap_err(), LIMIT);
    }

    #[test]
    fn continuation_chunks_are_ordered_and_bounded() {
        let mut state = 91u32;
        let raw: Vec<u8> = (0..1000)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect();
        let zipped = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);
        let mut encoded = vec![0, 1];
        encoded.extend(1220u32.to_le_bytes());
        encoded.extend((zipped.len() as u32).to_le_bytes());
        encoded.extend(zipped);
        let mut row = record(535, 45, 20);
        row[32..534].copy_from_slice(&encoded[..502]);
        let mut chunks = BTreeMap::new();
        for (i, chunk) in encoded[502..].chunks(244).enumerate() {
            chunks.insert(i as u16, chunk);
        }
        let mut blobs = BTreeMap::from([((16, 1), chunks)]);
        assert_eq!(stream(&row, 32, 16, &blobs).unwrap(), raw);
        blobs.get_mut(&(16, 1)).unwrap().remove(&0);
        assert_eq!(stream(&row, 32, 16, &blobs).unwrap_err(), INVALID);
        let mut files = fixture();
        *files.get_mut("Instance.eod").unwrap().last_mut().unwrap() = 0;
        assert!(
            read(&files).is_ok(),
            "observed zero padding is also valid at a record boundary"
        );
    }
}
