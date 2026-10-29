// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The XLSX reader of the parts import (D11): an Office Open XML workbook read from memory, the first or the named
//! sheet's cells as their displayed text, a formula's cached value (never evaluated).
//!
//! **The archive.** The ZIP central directory is read first; its declared unpacked sizes may not add up to more than
//! the loader's input limit (doc 05 §3.6), ZIP64, encrypted entries and methods other than stored and deflate are
//! refused. An entry is inflated (miniz_oxide, raw deflate) into a buffer capped at its declared size plus one byte,
//! and never above the input limit: an entry that inflates to more, or to another size than declared, refuses the
//! workbook (a compression bomb stops at the cap, it is not read).
//!
//! **The XML** (quick-xml, streamed): `_rels/.rels` names the workbook, the workbook's relationships its sheets and
//! shared strings. The shared-strings table is bounded by the loader limits: at most `max_xml_nodes` strings, each at
//! most `max_text_bytes`, all together at most `max_input_bytes`. Elements nest at most `max_xml_depth` deep. Cells
//! are streamed into the rows they name; at most [`MAX_COLUMNS`] columns and [`MAX_ROWS`] rows.
//!
//! **Values.** A shared or inline string as it is (phonetic runs left out); a number in its shortest form (`2904602`,
//! `2.5`; a date is its serial number, as number formats are not read); a boolean `TRUE` or `FALSE`; an error value
//! (`#N/A`) as [`Cell::Error`]; a formula's string result as it is.

use std::collections::BTreeMap;

use ew_interop_api::{ImportError, Limits};
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::table::{Cell, MAX_COLUMNS, MAX_ROWS, malformed, too_many};

/// The most entries of an XLSX workbook (the loader's file count, doc 05 §3.6).
const MAX_ZIP_ENTRIES: usize = 100_000;

/// The relationship types the reader follows (their last path segment).
const OFFICE_DOCUMENT: &str = "/officeDocument";
const SHARED_STRINGS: &str = "/sharedStrings";

fn unreadable(why: impl std::fmt::Display) -> ImportError {
    malformed(format!("not a readable XLSX workbook: {why}"), None)
}

fn unsupported(what: impl Into<String>) -> ImportError {
    ImportError::Unsupported { message: what.into() }
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        b.get(at..at.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        b.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

/// An entry of the central directory.
struct Entry {
    flags: u16,
    method: u16,
    compressed: u64,
    size: u64,
    local: u64,
}

/// A ZIP archive in memory: its entries by name.
struct Archive<'a> {
    bytes: &'a [u8],
    entries: BTreeMap<String, Entry>,
}

impl<'a> Archive<'a> {
    /// Reads the central directory. The declared unpacked sizes together may not exceed `limits.max_input_bytes`.
    fn open(bytes: &'a [u8], limits: &Limits) -> Result<Archive<'a>, ImportError> {
        const EOCD: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
        const CDH: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];
        let from = bytes.len().saturating_sub(22 + 65_535);
        let eocd = (from..bytes.len().saturating_sub(21))
            .rev()
            .find(|&i| bytes[i..i + 4] == EOCD)
            .ok_or_else(|| unreadable("no ZIP central directory"))?;
        let broken = || unreadable("a broken ZIP central directory");
        let entries = u16_at(bytes, eocd + 10).ok_or_else(broken)?;
        let offset = u32_at(bytes, eocd + 16).ok_or_else(broken)?;
        if entries == 0xffff || offset == 0xffff_ffff {
            return Err(unsupported("a ZIP64 workbook"));
        }
        let entries = usize::from(entries);
        if entries > MAX_ZIP_ENTRIES {
            return Err(too_many("workbook entry count", MAX_ZIP_ENTRIES));
        }
        let mut at = usize::try_from(offset).map_err(|_| broken())?;
        let mut total: u64 = 0;
        let mut out = BTreeMap::new();
        for _ in 0..entries {
            if bytes.get(at..at + 4) != Some(&CDH[..]) {
                return Err(broken());
            }
            let field16 = |o: usize| u16_at(bytes, at + o).ok_or_else(broken);
            let field32 = |o: usize| u32_at(bytes, at + o).ok_or_else(broken);
            let (flags, method) = (field16(8)?, field16(10)?);
            let (compressed, size, local) = (field32(20)?, field32(24)?, field32(42)?);
            let (n, e, c) = (
                usize::from(field16(28)?),
                usize::from(field16(30)?),
                usize::from(field16(32)?),
            );
            if [compressed, size, local].contains(&0xffff_ffff) {
                return Err(unsupported("a ZIP64 workbook"));
            }
            let name = bytes.get(at + 46..at + 46 + n).ok_or_else(broken)?;
            let name = String::from_utf8_lossy(name).into_owned();
            total = total.saturating_add(u64::from(size));
            if total > limits.max_input_bytes {
                return Err(ImportError::LimitExceeded {
                    what: "unpacked workbook size".into(),
                    limit: limits.max_input_bytes,
                });
            }
            let entry = Entry {
                flags,
                method,
                compressed: u64::from(compressed),
                size: u64::from(size),
                local: u64::from(local),
            };
            if out.insert(name.clone(), entry).is_some() {
                return Err(unreadable(format!("the entry {name} is listed twice")));
            }
            at += 46 + n + e + c;
        }
        Ok(Archive { bytes, entries: out })
    }

    /// The unpacked bytes of the entry `name`, `None` when the archive has none. Inflated into a buffer of at most
    /// the declared size plus one byte and at most `limits.max_input_bytes`: more output refuses the workbook.
    fn read(&self, name: &str, limits: &Limits) -> Result<Option<Vec<u8>>, ImportError> {
        let Some(entry) = self.entries.get(name) else {
            return Ok(None);
        };
        if entry.flags & 1 != 0 {
            return Err(unsupported("an encrypted workbook"));
        }
        let broken = || unreadable(format!("the entry {name} is broken"));
        let at = usize::try_from(entry.local).map_err(|_| broken())?;
        if u32_at(self.bytes, at) != Some(0x0403_4b50) {
            return Err(broken());
        }
        let n = usize::from(u16_at(self.bytes, at + 26).ok_or_else(broken)?);
        let e = usize::from(u16_at(self.bytes, at + 28).ok_or_else(broken)?);
        let start = at + 30 + n + e;
        let len = usize::try_from(entry.compressed).map_err(|_| broken())?;
        let data = self
            .bytes
            .get(start..start.checked_add(len).ok_or_else(broken)?)
            .ok_or_else(broken)?;
        let cap = entry.size.min(limits.max_input_bytes).saturating_add(1);
        let out = match entry.method {
            0 => data.to_vec(),
            8 => {
                let cap = usize::try_from(cap).unwrap_or(usize::MAX);
                match miniz_oxide::inflate::decompress_to_vec_with_limit(data, cap) {
                    Ok(out) => out,
                    Err(err) if err.status == miniz_oxide::inflate::TINFLStatus::HasMoreOutput => {
                        return Err(ImportError::LimitExceeded {
                            what: format!("unpacked size of {name}"),
                            limit: entry.size.min(limits.max_input_bytes),
                        });
                    }
                    Err(_) => return Err(unreadable(format!("the entry {name} cannot be inflated"))),
                }
            }
            m => return Err(unsupported(format!("compression method {m} of {name}"))),
        };
        if out.len() as u64 != entry.size {
            return Err(unreadable(format!(
                "the entry {name} unpacks to {} bytes, not the {} its directory states",
                out.len(),
                entry.size
            )));
        }
        Ok(Some(out))
    }

    /// The entry `name` as UTF-8 text (a byte-order mark dropped); `None` when the archive has none.
    fn text(&self, name: &str, limits: &Limits) -> Result<Option<String>, ImportError> {
        let Some(bytes) = self.read(name, limits)? else {
            return Ok(None);
        };
        let text = String::from_utf8(bytes).map_err(|_| unreadable(format!("{name} is not UTF-8")))?;
        Ok(Some(match text.strip_prefix('\u{feff}') {
            Some(rest) => rest.to_owned(),
            None => text,
        }))
    }
}

/// The value of attribute `key` (a local name; a prefixed attribute matches when `prefixed`).
fn attr(e: &BytesStart<'_>, key: &str, prefixed: bool) -> Result<Option<String>, ImportError> {
    for a in e.attributes() {
        let a = a.map_err(unreadable)?;
        if a.key.local_name().as_ref() == key.as_bytes() && a.key.prefix().is_some() == prefixed {
            let v = a.normalized_value(XmlVersion::Implicit1_0).map_err(unreadable)?;
            return Ok(Some(v.into_owned()));
        }
    }
    Ok(None)
}

/// A streaming pass over one XML part: calls `on` for every element start (`true` for an empty element), element
/// end and text (character data and references resolved), with the element depth bounded by `limits.max_xml_depth`.
fn walk(
    xml: &str,
    limits: &Limits,
    mut on: impl FnMut(Node<'_, '_>) -> Result<(), ImportError>,
) -> Result<(), ImportError> {
    let mut reader = Reader::from_str(xml);
    let mut depth: u32 = 0;
    loop {
        match reader.read_event().map_err(unreadable)? {
            Event::Start(e) => {
                depth += 1;
                if depth > limits.max_xml_depth {
                    return Err(ImportError::LimitExceeded {
                        what: "XML depth".into(),
                        limit: u64::from(limits.max_xml_depth),
                    });
                }
                on(Node::Start(&e, false))?;
            }
            Event::Empty(e) => {
                on(Node::Start(&e, true))?;
                on(Node::End(e.local_name().as_ref()))?;
            }
            Event::End(e) => {
                depth = depth.saturating_sub(1);
                on(Node::End(e.local_name().as_ref()))?;
            }
            Event::Text(t) => on(Node::Text(&t.xml10_content().map_err(unreadable)?))?,
            Event::CData(t) => on(Node::Text(&t.decode().map_err(unreadable)?))?,
            Event::GeneralRef(r) => {
                let c = match r.resolve_char_ref().map_err(unreadable)? {
                    Some(c) => c,
                    None => match &*r {
                        b"lt" => '<',
                        b"gt" => '>',
                        b"amp" => '&',
                        b"apos" => '\'',
                        b"quot" => '"',
                        other => {
                            return Err(unreadable(format!(
                                "an unknown entity `&{};`",
                                String::from_utf8_lossy(other)
                            )));
                        }
                    },
                };
                on(Node::Text(c.encode_utf8(&mut [0; 4])))?;
            }
            Event::Eof => return Ok(()),
            _ => {}
        }
    }
}

/// One step of [`walk`].
enum Node<'e, 'x> {
    /// An element starts (`true`: it is empty, its end follows at once).
    Start(&'e BytesStart<'x>, bool),
    /// An element ends: its local name.
    End(&'e [u8]),
    /// Text.
    Text(&'e str),
}

/// A part's relationships: ID → (target path in the archive, type).
fn relationships(
    book: &Archive<'_>,
    rels: &str,
    base: &str,
    limits: &Limits,
) -> Result<BTreeMap<String, (String, String)>, ImportError> {
    let mut out = BTreeMap::new();
    let Some(xml) = book.text(rels, limits)? else {
        return Ok(out);
    };
    walk(&xml, limits, |node| {
        if let Node::Start(e, _) = node
            && e.local_name().as_ref() == b"Relationship"
        {
            let id = attr(e, "Id", false)?.unwrap_or_default();
            let target = attr(e, "Target", false)?.unwrap_or_default();
            let kind = attr(e, "Type", false)?.unwrap_or_default();
            out.insert(id, (resolve(base, &target)?, kind));
        }
        Ok(())
    })?;
    Ok(out)
}

/// The archive path of `target` relative to the directory `base` (`xl/`), or absolute (`/xl/...`).
fn resolve(base: &str, target: &str) -> Result<String, ImportError> {
    let (mut parts, rest): (Vec<&str>, &str) = match target.strip_prefix('/') {
        Some(abs) => (Vec::new(), abs),
        None => (base.split('/').filter(|s| !s.is_empty()).collect(), target),
    };
    for seg in rest.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts
                    .pop()
                    .ok_or_else(|| unreadable(format!("the target {target} leaves the archive")))?;
            }
            s => parts.push(s),
        }
    }
    Ok(parts.join("/"))
}

/// The directory of an archive path, with its trailing `/` (`xl/` of `xl/workbook.xml`).
fn dir_of(path: &str) -> &str {
    path.rfind('/').map_or("", |i| &path[..=i])
}

/// The relationships file of a part (`xl/_rels/workbook.xml.rels` of `xl/workbook.xml`).
fn rels_of(path: &str) -> String {
    let (dir, file) = path
        .rfind('/')
        .map_or(("", path), |i| (&path[..=i], &path[i + 1..]));
    format!("{dir}_rels/{file}.rels")
}

/// Appends `text` to `buf`, refusing a value above `limits.max_text_bytes`.
fn push_text(buf: &mut String, text: &str, limits: &Limits, what: &str) -> Result<(), ImportError> {
    if buf.len() + text.len() > limits.max_text_bytes as usize {
        return Err(ImportError::LimitExceeded {
            what: what.to_owned(),
            limit: u64::from(limits.max_text_bytes),
        });
    }
    buf.push_str(text);
    Ok(())
}

/// The shared-strings table: at most `limits.max_xml_nodes` strings of at most `limits.max_text_bytes` each, at most
/// `limits.max_input_bytes` together.
fn shared_strings(xml: &str, limits: &Limits) -> Result<Vec<String>, ImportError> {
    let mut out: Vec<String> = Vec::new();
    let (mut total, mut current, mut in_t, mut phonetic) = (0u64, None::<String>, false, false);
    walk(xml, limits, |node| {
        match node {
            Node::Start(e, empty) => match e.local_name().as_ref() {
                b"si" => {
                    if out.len() >= limits.max_xml_nodes as usize {
                        return Err(ImportError::LimitExceeded {
                            what: "shared string count".into(),
                            limit: u64::from(limits.max_xml_nodes),
                        });
                    }
                    current = Some(String::new());
                }
                b"t" if !empty => in_t = true,
                b"rPh" if !empty => phonetic = true,
                _ => {}
            },
            Node::End(name) => match name {
                b"si" => {
                    let s = current.take().unwrap_or_default();
                    total = total.saturating_add(s.len() as u64);
                    if total > limits.max_input_bytes {
                        return Err(ImportError::LimitExceeded {
                            what: "shared strings size".into(),
                            limit: limits.max_input_bytes,
                        });
                    }
                    out.push(s);
                }
                b"t" => in_t = false,
                b"rPh" => phonetic = false,
                _ => {}
            },
            Node::Text(t) => {
                if in_t
                    && !phonetic
                    && let Some(s) = current.as_mut()
                {
                    push_text(s, t, limits, "shared string size")?;
                }
            }
        }
        Ok(())
    })?;
    Ok(out)
}

/// The 0-based column of a cell reference (`B2` → 1), `None` without letters.
fn column_of(reference: &str) -> Option<usize> {
    let letters: Vec<u8> = reference.bytes().take_while(u8::is_ascii_uppercase).collect();
    if letters.is_empty() || letters.len() > 3 {
        return None;
    }
    let n = letters
        .iter()
        .fold(0usize, |n, b| n * 26 + usize::from(b - b'A') + 1);
    Some(n - 1)
}

/// The displayed text of a cell of type `t` (`s`, `str`, `inlineStr`, `b`, `e`, `n` or none) and raw value `v`.
fn display(t: &str, v: &str, inline: &str, strings: &[String]) -> Result<Cell, ImportError> {
    Ok(Cell::Text(match t {
        "s" => {
            let i: usize = v
                .trim()
                .parse()
                .map_err(|_| unreadable(format!("a shared-string index `{v}`")))?;
            strings
                .get(i)
                .cloned()
                .ok_or_else(|| unreadable(format!("the shared string {i} does not exist")))?
        }
        "inlineStr" => inline.to_owned(),
        "b" => if v.trim() == "1" { "TRUE" } else { "FALSE" }.to_owned(),
        "e" => return Ok(Cell::Error(v.to_owned())),
        "n" | "" => match v.trim().parse::<f64>() {
            Ok(f) if f.is_finite() => f.to_string(),
            _ => v.to_owned(),
        },
        _ => v.to_owned(),
    }))
}

/// One cell as the sheet's XML gives it, while it is read.
#[derive(Default)]
struct RawCell {
    column: usize,
    kind: String,
    value: String,
    inline: String,
}

/// Reads the sheet `sheet` (the first when `None`) of an XLSX workbook into records (row number, cells).
pub(crate) fn read(
    bytes: &[u8],
    sheet: Option<&str>,
    limits: &Limits,
) -> Result<Vec<(u32, Vec<Cell>)>, ImportError> {
    let book = Archive::open(bytes, limits)?;
    let root = relationships(&book, "_rels/.rels", "", limits)?;
    let workbook = root
        .values()
        .find(|(_, kind)| kind.ends_with(OFFICE_DOCUMENT))
        .map_or_else(|| "xl/workbook.xml".to_owned(), |(t, _)| t.clone());
    let book_xml = book
        .text(&workbook, limits)?
        .ok_or_else(|| unreadable(format!("no workbook part {workbook}")))?;
    let rels = relationships(&book, &rels_of(&workbook), dir_of(&workbook), limits)?;
    // The sheets in workbook order: (name, relationship ID).
    let mut sheets: Vec<(String, String)> = Vec::new();
    walk(&book_xml, limits, |node| {
        if let Node::Start(e, _) = node
            && e.local_name().as_ref() == b"sheet"
        {
            sheets.push((
                attr(e, "name", false)?.unwrap_or_default(),
                attr(e, "id", true)?.unwrap_or_default(),
            ));
        }
        Ok(())
    })?;
    let (_, rid) = match sheet {
        Some(s) => sheets
            .iter()
            .find(|(n, _)| n == s)
            .ok_or_else(|| malformed(format!("the workbook has no sheet named `{s}`"), None))?,
        None => sheets
            .first()
            .ok_or_else(|| malformed("the workbook has no sheet", None))?,
    };
    let path = rels
        .get(rid)
        .map(|(t, _)| t.clone())
        .ok_or_else(|| unreadable(format!("the sheet relationship {rid} is missing")))?;
    let strings_path = rels
        .values()
        .find(|(_, kind)| kind.ends_with(SHARED_STRINGS))
        .map(|(t, _)| t.clone());
    let strings = match strings_path {
        Some(p) => match book.text(&p, limits)? {
            Some(xml) => shared_strings(&xml, limits)?,
            None => Vec::new(),
        },
        None => Vec::new(),
    };
    let xml = book
        .text(&path, limits)?
        .ok_or_else(|| unreadable(format!("no sheet part {path}")))?;

    let mut grid: BTreeMap<u32, BTreeMap<usize, Cell>> = BTreeMap::new();
    let (mut row, mut next_column) = (0u32, 0usize);
    let mut cell: Option<RawCell> = None;
    // Inside <v>, inside <is>, inside <t> of <is>, inside a phonetic run.
    let (mut in_v, mut in_is, mut in_t, mut phonetic) = (false, false, false, false);
    walk(&xml, limits, |node| {
        match node {
            Node::Start(e, _) => match e.local_name().as_ref() {
                b"row" => {
                    row = match attr(e, "r", false)? {
                        Some(r) => r
                            .trim()
                            .parse()
                            .map_err(|_| unreadable(format!("a row number `{r}`")))?,
                        None => row.saturating_add(1),
                    };
                    next_column = 0;
                }
                b"c" => {
                    let column = match attr(e, "r", false)? {
                        Some(r) => {
                            column_of(&r).ok_or_else(|| unreadable(format!("a cell reference `{r}`")))?
                        }
                        None => next_column,
                    };
                    if column >= MAX_COLUMNS {
                        return Err(too_many("column count", MAX_COLUMNS));
                    }
                    next_column = column + 1;
                    cell = Some(RawCell {
                        column,
                        kind: attr(e, "t", false)?.unwrap_or_default(),
                        ..RawCell::default()
                    });
                }
                b"v" => in_v = true,
                b"is" => in_is = true,
                b"t" if in_is => in_t = true,
                b"rPh" => phonetic = true,
                _ => {}
            },
            Node::End(name) => match name {
                b"c" => {
                    if let Some(c) = cell.take() {
                        let value = display(&c.kind, &c.value, &c.inline, &strings)?;
                        if value != Cell::Text(String::new()) {
                            if !grid.contains_key(&row) && grid.len() > MAX_ROWS {
                                return Err(too_many("row count", MAX_ROWS));
                            }
                            grid.entry(row).or_default().insert(c.column, value);
                        }
                    }
                }
                b"v" => in_v = false,
                b"is" => in_is = false,
                b"t" => in_t = false,
                b"rPh" => phonetic = false,
                _ => {}
            },
            Node::Text(t) => {
                if let Some(c) = cell.as_mut() {
                    let at = format!("cell size at line {row}");
                    if in_v {
                        push_text(&mut c.value, t, limits, &at)?;
                    } else if in_t && !phonetic {
                        push_text(&mut c.inline, t, limits, &at)?;
                    }
                }
            }
        }
        Ok(())
    })?;
    Ok(grid
        .into_iter()
        .map(|(row, cols)| {
            let width = cols.keys().next_back().map_or(0, |c| c + 1);
            let mut line = vec![Cell::Text(String::new()); width];
            for (c, v) in cols {
                line[c] = v;
            }
            (row, line)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_resolve_inside_the_archive() {
        assert_eq!(
            resolve("xl/", "worksheets/sheet1.xml").unwrap(),
            "xl/worksheets/sheet1.xml"
        );
        assert_eq!(
            resolve("xl/", "/xl/sharedStrings.xml").unwrap(),
            "xl/sharedStrings.xml"
        );
        assert_eq!(
            resolve("xl/worksheets/", "../media/a.png").unwrap(),
            "xl/media/a.png"
        );
        assert!(resolve("", "../x").is_err());
        assert_eq!(rels_of("xl/workbook.xml"), "xl/_rels/workbook.xml.rels");
        assert_eq!(dir_of("xl/workbook.xml"), "xl/");
    }

    #[test]
    fn columns_and_values() {
        assert_eq!(column_of("A1"), Some(0));
        assert_eq!(column_of("Z9"), Some(25));
        assert_eq!(column_of("AA3"), Some(26));
        assert_eq!(column_of("12"), None);
        let strings = vec!["x".to_owned()];
        let text = |t, v| display(t, v, "inline", &strings).unwrap();
        assert_eq!(text("n", "10"), Cell::Text("10".into()));
        assert_eq!(text("", "2.50"), Cell::Text("2.5".into()));
        assert_eq!(text("s", "0"), Cell::Text("x".into()));
        assert_eq!(text("b", "1"), Cell::Text("TRUE".into()));
        assert_eq!(text("e", "#N/A"), Cell::Error("#N/A".into()));
        assert_eq!(text("inlineStr", ""), Cell::Text("inline".into()));
        assert!(display("s", "7", "", &strings).is_err());
    }

    #[test]
    fn the_shared_strings_are_bounded() {
        let xml =
            "<sst><si><t>a&amp;b</t></si><si><r><t>c</t></r><r><t>d</t></r><rPh><t>x</t></rPh></si></sst>";
        assert_eq!(shared_strings(xml, &Limits::default()).unwrap(), ["a&b", "cd"]);
        let few = Limits {
            max_xml_nodes: 1,
            ..Limits::default()
        };
        assert!(matches!(
            shared_strings(xml, &few),
            Err(ImportError::LimitExceeded { .. })
        ));
        let short = Limits {
            max_text_bytes: 2,
            ..Limits::default()
        };
        assert!(matches!(
            shared_strings(xml, &short),
            Err(ImportError::LimitExceeded { .. })
        ));
        let small = Limits {
            max_input_bytes: 3,
            ..Limits::default()
        };
        assert!(matches!(
            shared_strings(xml, &small),
            Err(ImportError::LimitExceeded { .. })
        ));
    }
}
