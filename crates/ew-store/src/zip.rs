// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The deterministic ZIP writer and its strict reader (doc 05 §12, §14.2; WP-S1.18 D11): the container of the as-built
//! archive, on `miniz_oxide`'s raw deflate (already in the graph, decision log ID-827), so that the archive's bytes
//! depend on no ZIP crate's defaults.
//!
//! **The writer** ([`write()`]) takes the entries in the order the caller gives them and writes, for each, a local file
//! header and its data, then the central directory and the end record. Every field that is not the entry's name,
//! method, data, CRC-32 or offset is fixed:
//!
//! - version needed 2.0, version made by 2.0 on Unix (`0x0314`), so that the external attributes carry mode `0644`
//!   (`0o100644 << 16`), as doc 05 §12 asks;
//! - general-purpose flags `0x0800` (the names are UTF-8), never a data descriptor or encryption;
//! - DOS time 1980-01-01 00:00:00 (`time 0x0000`, `date 0x0021`);
//! - no extra fields, no file or archive comments, no ZIP64 (an entry or the archive beyond 4 GiB − 1 bytes, or more
//!   than 65,535 entries, is refused);
//! - [`Method::Deflated`] entries are raw deflate at the fixed level [`DEFLATE_LEVEL`], [`Method::Stored`] entries as
//!   they are.
//!
//! The same entries therefore give the same bytes on every platform and run (the compressor is pinned through
//! `Cargo.lock`).
//!
//! **The reader** ([`read()`]) accepts exactly the shape the writer makes and refuses everything else, so that a damaged
//! or foreign file is never taken for an archive: one end record at the very end (no comment), one disk, the central
//! directory directly before the end record, the local entries packed from offset 0 without gaps or overlaps in
//! central-directory order, local and central headers that agree, methods stored and deflate, and the writer's fixed
//! versions, flags, time, date and attributes; no extra fields; names that are relative `/`-separated paths without
//! `..`, empty segments, backslashes, colons, control characters or a trailing `/`, no two names equal ignoring ASCII
//! case and no name
//! that is the directory of another (doc 05 §12's extractor rules). Every entry inflates
//! into a buffer capped at its declared size, which must match exactly, as must its CRC-32. The declared sizes are
//! bounded by [`ZipLimits`] before anything is inflated, and a deflated entry may expand at most
//! [`ZipLimits::max_ratio`] to 1 (doc 05 §12: 200 : 1), so a compression bomb is refused unread.

use std::collections::BTreeSet;
use std::fmt;

/// The deflate level of every deflated entry (doc 05 §12: "the Deflate level is fixed").
pub const DEFLATE_LEVEL: u8 = 6;

/// The DOS date of every entry: 1980-01-01 (`(1980 − 1980) << 9 | 1 << 5 | 1`).
const DOS_DATE: u16 = 0x0021;
/// The DOS time of every entry: 00:00:00.
const DOS_TIME: u16 = 0x0000;
/// The general-purpose flags: bit 11, the names are UTF-8.
const FLAGS: u16 = 0x0800;
/// Version needed to extract: 2.0 (deflate).
const VERSION_NEEDED: u16 = 20;
/// Version made by: 2.0 on Unix (3), so that the external attributes are a Unix mode.
const VERSION_MADE_BY: u16 = 0x0314;
/// The external attributes: a regular file with mode 0644.
const EXTERNAL_ATTRIBUTES: u32 = 0o100_644 << 16;

const LOCAL_SIGNATURE: u32 = 0x0403_4b50;
const CENTRAL_SIGNATURE: u32 = 0x0201_4b50;
const END_SIGNATURE: u32 = 0x0605_4b50;
const END_RECORD: usize = 22;

/// How an entry's data are stored.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Method {
    /// As they are (method 0): the archive's `mimetype`.
    Stored,
    /// Raw deflate at [`DEFLATE_LEVEL`] (method 8): every other entry.
    Deflated,
}

impl Method {
    fn code(self) -> u16 {
        match self {
            Method::Stored => 0,
            Method::Deflated => 8,
        }
    }
}

/// One entry the reader found.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Entry {
    /// The path.
    pub name: String,
    /// How it was stored.
    pub method: Method,
    /// The data, inflated.
    pub data: Vec<u8>,
}

/// The reader's bounds (doc 05 §12: 2 GiB expanded, 512 MiB per entry, 200 : 1 per entry, as for every archive
/// reader).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ZipLimits {
    /// The most entries.
    pub max_entries: usize,
    /// The largest declared size of one entry, in bytes.
    pub max_entry_bytes: u64,
    /// The largest sum of the declared sizes, in bytes.
    pub max_total_bytes: u64,
    /// The largest ratio of a deflated entry's declared size to its compressed size; entries of at most
    /// [`RATIO_FLOOR`] bytes are exempt (a few kilobytes cannot be a bomb).
    pub max_ratio: u64,
}

/// The declared size up to which a deflated entry is not held to [`ZipLimits::max_ratio`].
pub const RATIO_FLOOR: u64 = 64 * 1024;

impl Default for ZipLimits {
    fn default() -> ZipLimits {
        ZipLimits {
            max_entries: 65_535,
            max_entry_bytes: 512 * 1024 * 1024,
            max_total_bytes: 2 * 1024 * 1024 * 1024,
            max_ratio: 200,
        }
    }
}

/// Why an archive could not be written or read.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ZipError {
    /// A name is not a relative `/`-separated path the reader accepts (it says why).
    Name {
        /// The name.
        name: String,
        /// What is wrong.
        why: &'static str,
    },
    /// Two names are equal, or equal ignoring ASCII case.
    Duplicate {
        /// The second of them.
        name: String,
    },
    /// The archive or an entry is too large for a ZIP without ZIP64, or has too many entries.
    TooLarge,
    /// A bound of [`ZipLimits`] is exceeded.
    Limit {
        /// The entry, when one entry exceeds it.
        name: Option<String>,
        /// Which bound.
        what: &'static str,
    },
    /// The bytes are not a ZIP of the writer's shape.
    Malformed {
        /// What is wrong.
        what: String,
    },
    /// An entry's data do not inflate, or inflate to another size or CRC-32 than declared.
    Corrupt {
        /// The entry.
        name: String,
        /// What is wrong.
        what: String,
    },
}

impl fmt::Display for ZipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ZipError::Name { name, why } => write!(f, "the entry name `{name}` is not allowed: {why}"),
            ZipError::Duplicate { name } => write!(f, "the entry name `{name}` occurs twice"),
            ZipError::TooLarge => f.write_str("the archive is too large for a ZIP without ZIP64"),
            ZipError::Limit { name: Some(n), what } => {
                write!(f, "the entry `{n}` exceeds the limit on {what}")
            }
            ZipError::Limit { name: None, what } => write!(f, "the archive exceeds the limit on {what}"),
            ZipError::Malformed { what } => write!(f, "not an archive of the expected form: {what}"),
            ZipError::Corrupt { name, what } => write!(f, "the entry `{name}` is damaged: {what}"),
        }
    }
}

impl std::error::Error for ZipError {}

/// The CRC-32 of ZIP (IEEE 802.3, reflected polynomial `0xEDB88320`).
///
/// ```
/// assert_eq!(ew_store::zip::crc32(b"123456789"), 0xCBF4_3926);
/// assert_eq!(ew_store::zip::crc32(b""), 0);
/// ```
pub fn crc32(data: &[u8]) -> u32 {
    const TABLE: [u32; 256] = {
        let mut table = [0u32; 256];
        let mut i = 0;
        while i < 256 {
            let mut c = i as u32;
            let mut k = 0;
            while k < 8 {
                c = if c & 1 == 1 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
                k += 1;
            }
            table[i] = c;
            i += 1;
        }
        table
    };
    let mut c = !0u32;
    for b in data {
        c = TABLE[usize::from((c as u8) ^ b)] ^ (c >> 8);
    }
    !c
}

/// Whether `name` is a path the reader accepts: why not, when it is not.
fn name_problem(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        return Some("empty");
    }
    if name.len() > usize::from(u16::MAX) {
        return Some("longer than 65,535 bytes");
    }
    if name.contains('\\') {
        return Some("a backslash");
    }
    if name.contains(':') {
        return Some("a colon (a drive letter or a stream)");
    }
    if name.chars().any(char::is_control) {
        return Some("a control character");
    }
    if name.starts_with('/') {
        return Some("an absolute path");
    }
    if name.ends_with('/') {
        return Some("a directory entry");
    }
    if name.split('/').any(|s| s.is_empty() || s == "." || s == "..") {
        return Some("an empty, `.` or `..` segment");
    }
    None
}

/// Checks every name, that no two are equal ignoring ASCII case, and that no name is the directory of another
/// (`a` and `a/b`: a file where an extractor needs a directory).
fn check_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<(), ZipError> {
    let mut files = BTreeSet::new();
    let mut dirs = BTreeSet::new();
    for name in names {
        if let Some(why) = name_problem(name) {
            return Err(ZipError::Name {
                name: name.to_owned(),
                why,
            });
        }
        let lower = name.to_ascii_lowercase();
        let mut at = 0;
        while let Some(i) = lower[at..].find('/') {
            dirs.insert(lower[..at + i].to_owned());
            at += i + 1;
        }
        if !files.insert(lower) {
            return Err(ZipError::Duplicate {
                name: name.to_owned(),
            });
        }
    }
    if let Some(both) = files.intersection(&dirs).next() {
        return Err(ZipError::Name {
            name: both.clone(),
            why: "both a file and the directory of another entry",
        });
    }
    Ok(())
}

fn u16_of(n: usize) -> Result<u16, ZipError> {
    u16::try_from(n).map_err(|_| ZipError::TooLarge)
}

fn u32_of(n: usize) -> Result<u32, ZipError> {
    // 0xFFFF_FFFF is ZIP64's marker: never written as a size or an offset.
    u32::try_from(n)
        .ok()
        .filter(|v| *v != u32::MAX)
        .ok_or(ZipError::TooLarge)
}

/// Writes `entries` (name, method, data), in the given order, as a ZIP of the fixed shape of the module documentation.
///
/// # Errors
///
/// [`ZipError::Name`] or [`ZipError::Duplicate`] for a name the reader would refuse; [`ZipError::TooLarge`] beyond the
/// sizes of a ZIP without ZIP64.
///
/// ```
/// use ew_store::zip::{Method, ZipLimits, read, write};
///
/// let bytes = write(&[("mimetype", Method::Stored, b"text/plain".as_slice()), ("a/b.txt", Method::Deflated, b"hi")])
///     .unwrap();
/// assert_eq!(&bytes[30..38], b"mimetype");
/// let entries = read(&bytes, &ZipLimits::default()).unwrap();
/// assert_eq!(entries[1].name, "a/b.txt");
/// assert_eq!(entries[1].data, b"hi");
/// ```
pub fn write(entries: &[(&str, Method, &[u8])]) -> Result<Vec<u8>, ZipError> {
    check_names(entries.iter().map(|(name, _, _)| *name))?;
    let count = u16_of(entries.len())?;
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    for (name, method, data) in entries {
        let packed = match method {
            Method::Stored => data.to_vec(),
            Method::Deflated => miniz_oxide::deflate::compress_to_vec(data, DEFLATE_LEVEL),
        };
        let offset = u32_of(out.len())?;
        let crc = crc32(data);
        let (csize, usize_) = (u32_of(packed.len())?, u32_of(data.len())?);
        let name_len = u16_of(name.len())?;
        // The local file header.
        put32(&mut out, LOCAL_SIGNATURE);
        put16(&mut out, VERSION_NEEDED);
        put16(&mut out, FLAGS);
        put16(&mut out, method.code());
        put16(&mut out, DOS_TIME);
        put16(&mut out, DOS_DATE);
        put32(&mut out, crc);
        put32(&mut out, csize);
        put32(&mut out, usize_);
        put16(&mut out, name_len);
        put16(&mut out, 0);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&packed);
        // Its central-directory header.
        put32(&mut central, CENTRAL_SIGNATURE);
        put16(&mut central, VERSION_MADE_BY);
        put16(&mut central, VERSION_NEEDED);
        put16(&mut central, FLAGS);
        put16(&mut central, method.code());
        put16(&mut central, DOS_TIME);
        put16(&mut central, DOS_DATE);
        put32(&mut central, crc);
        put32(&mut central, csize);
        put32(&mut central, usize_);
        put16(&mut central, name_len);
        put16(&mut central, 0); // extra field length
        put16(&mut central, 0); // comment length
        put16(&mut central, 0); // disk number start
        put16(&mut central, 0); // internal attributes
        put32(&mut central, EXTERNAL_ATTRIBUTES);
        put32(&mut central, offset);
        central.extend_from_slice(name.as_bytes());
    }
    let cd_offset = u32_of(out.len())?;
    let cd_size = u32_of(central.len())?;
    out.extend_from_slice(&central);
    // The end of central directory record.
    put32(&mut out, END_SIGNATURE);
    put16(&mut out, 0); // this disk
    put16(&mut out, 0); // the disk with the central directory
    put16(&mut out, count);
    put16(&mut out, count);
    put32(&mut out, cd_size);
    put32(&mut out, cd_offset);
    put16(&mut out, 0); // comment length
    u32_of(out.len())?;
    Ok(out)
}

fn put16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// A bounds-checked little-endian cursor over the archive.
struct At<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> At<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], ZipError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|e| *e <= self.bytes.len())
            .ok_or_else(|| ZipError::Malformed {
                what: format!("truncated at byte {}", self.pos),
            })?;
        let out = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    fn u16(&mut self) -> Result<u16, ZipError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, ZipError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

fn malformed(what: impl Into<String>) -> ZipError {
    ZipError::Malformed { what: what.into() }
}

/// One central-directory header, as read.
struct Central {
    name: String,
    method: Method,
    flags: u16,
    crc: u32,
    csize: u32,
    usize_: u32,
    offset: u32,
}

/// Reads an archive of the writer's shape (see the module documentation), every entry inflated and checked, in
/// central-directory order.
///
/// # Errors
///
/// [`ZipError`]: a shape the writer does not make, a bound of `limits`, a damaged entry.
pub fn read(bytes: &[u8], limits: &ZipLimits) -> Result<Vec<Entry>, ZipError> {
    // The end record: the last 22 bytes (no archive comment).
    let end_at = bytes
        .len()
        .checked_sub(END_RECORD)
        .ok_or_else(|| malformed("shorter than an end record"))?;
    let mut end = At { bytes, pos: end_at };
    if end.u32()? != END_SIGNATURE {
        return Err(malformed(
            "no end record at the end (an archive comment or trailing bytes)",
        ));
    }
    let (disk, cd_disk) = (end.u16()?, end.u16()?);
    let (on_disk, total) = (end.u16()?, end.u16()?);
    let (cd_size, cd_offset) = (end.u32()?, end.u32()?);
    let comment = end.u16()?;
    if disk != 0 || cd_disk != 0 || on_disk != total {
        return Err(malformed("a multi-disk archive"));
    }
    if comment != 0 {
        return Err(malformed("an archive comment"));
    }
    if usize::from(total) > limits.max_entries {
        return Err(ZipError::Limit {
            name: None,
            what: "the number of entries",
        });
    }
    let cd_start = usize::try_from(cd_offset).map_err(|_| ZipError::TooLarge)?;
    let cd_len = usize::try_from(cd_size).map_err(|_| ZipError::TooLarge)?;
    if cd_start.checked_add(cd_len) != Some(end_at) {
        return Err(malformed("the central directory does not end at the end record"));
    }
    // The central directory.
    let mut cd = At {
        bytes: &bytes[..end_at],
        pos: cd_start,
    };
    let mut headers: Vec<Central> = Vec::with_capacity(usize::from(total));
    let mut declared: u64 = 0;
    for _ in 0..total {
        if cd.u32()? != CENTRAL_SIGNATURE {
            return Err(malformed("a central-directory header without its signature"));
        }
        let made_by = cd.u16()?;
        let needed = cd.u16()?;
        let flags = cd.u16()?;
        let method = match cd.u16()? {
            0 => Method::Stored,
            8 => Method::Deflated,
            m => return Err(malformed(format!("compression method {m}"))),
        };
        let (time, date) = (cd.u16()?, cd.u16()?);
        let crc = cd.u32()?;
        let (csize, usize_) = (cd.u32()?, cd.u32()?);
        let (name_len, extra_len, comment_len) = (cd.u16()?, cd.u16()?, cd.u16()?);
        let (disk_start, internal, external) = (cd.u16()?, cd.u16()?, cd.u32()?);
        let offset = cd.u32()?;
        if needed != VERSION_NEEDED || made_by != VERSION_MADE_BY {
            return Err(malformed(format!(
                "versions {made_by:#06x}/{needed} (ZIP64 or another writer)"
            )));
        }
        if flags != FLAGS {
            return Err(malformed(format!(
                "general-purpose flags {flags:#06x} (encryption, a data descriptor or names that are not UTF-8)"
            )));
        }
        if (time, date) != (DOS_TIME, DOS_DATE) || internal != 0 || external != EXTERNAL_ATTRIBUTES {
            return Err(malformed(
                "a time, a date or attributes other than the fixed ones",
            ));
        }
        if extra_len != 0 || comment_len != 0 || disk_start != 0 {
            return Err(malformed("an extra field, an entry comment or another disk"));
        }
        if csize == u32::MAX || usize_ == u32::MAX || offset == u32::MAX {
            return Err(malformed("ZIP64 sizes"));
        }
        let name = std::str::from_utf8(cd.take(usize::from(name_len))?)
            .map_err(|_| malformed("an entry name that is not UTF-8"))?
            .to_owned();
        if u64::from(usize_) > limits.max_entry_bytes {
            return Err(ZipError::Limit {
                name: Some(name),
                what: "the size of one entry",
            });
        }
        if method == Method::Stored && csize != usize_ {
            return Err(malformed(format!("the stored entry `{name}` has two sizes")));
        }
        if method == Method::Deflated
            && u64::from(usize_) > RATIO_FLOOR
            && u64::from(usize_) > limits.max_ratio.saturating_mul(u64::from(csize.max(1)))
        {
            return Err(ZipError::Limit {
                name: Some(name),
                what: "the compression ratio",
            });
        }
        declared = declared.saturating_add(u64::from(usize_));
        if declared > limits.max_total_bytes {
            return Err(ZipError::Limit {
                name: None,
                what: "the expanded size",
            });
        }
        headers.push(Central {
            name,
            method,
            flags,
            crc,
            csize,
            usize_,
            offset,
        });
    }
    if cd.pos != end_at {
        return Err(malformed(
            "bytes between the central directory and the end record",
        ));
    }
    check_names(headers.iter().map(|h| h.name.as_str()))?;
    // The local entries: packed from offset 0, in central-directory order, up to the central directory.
    let mut pos = 0usize;
    let mut out = Vec::with_capacity(headers.len());
    for h in headers {
        if usize::try_from(h.offset).ok() != Some(pos) {
            return Err(malformed(format!(
                "the entry `{}` does not start where the previous one ends",
                h.name
            )));
        }
        let mut local = At {
            bytes: &bytes[..cd_start],
            pos,
        };
        if local.u32()? != LOCAL_SIGNATURE {
            return Err(malformed(format!(
                "the local header of `{}` has no signature",
                h.name
            )));
        }
        let needed = local.u16()?;
        let flags = local.u16()?;
        let method = local.u16()?;
        let (time, date) = (local.u16()?, local.u16()?);
        let crc = local.u32()?;
        let (csize, usize_) = (local.u32()?, local.u32()?);
        let (name_len, extra_len) = (local.u16()?, local.u16()?);
        let name = local.take(usize::from(name_len))?;
        if needed != VERSION_NEEDED
            || (time, date) != (DOS_TIME, DOS_DATE)
            || flags != h.flags
            || method != h.method.code()
            || crc != h.crc
            || csize != h.csize
            || usize_ != h.usize_
            || extra_len != 0
            || name != h.name.as_bytes()
        {
            return Err(malformed(format!(
                "the local header of `{}` disagrees with the central directory",
                h.name
            )));
        }
        let packed = local.take(usize::try_from(h.csize).map_err(|_| ZipError::TooLarge)?)?;
        pos = local.pos;
        let size = usize::try_from(h.usize_).map_err(|_| ZipError::TooLarge)?;
        let data =
            match h.method {
                Method::Stored => packed.to_vec(),
                Method::Deflated => miniz_oxide::inflate::decompress_to_vec_with_limit(packed, size)
                    .map_err(|e| ZipError::Corrupt {
                        name: h.name.clone(),
                        what: format!("it does not inflate ({e})"),
                    })?,
            };
        if data.len() != size {
            return Err(ZipError::Corrupt {
                name: h.name,
                what: format!("it inflates to {} bytes, not {size}", data.len()),
            });
        }
        if crc32(&data) != h.crc {
            return Err(ZipError::Corrupt {
                name: h.name,
                what: "its CRC-32 does not match".to_owned(),
            });
        }
        out.push(Entry {
            name: h.name,
            method: h.method,
            data,
        });
    }
    if pos != cd_start {
        return Err(malformed(
            "bytes between the last entry and the central directory",
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn sample() -> Vec<u8> {
        write(&[
            (
                "mimetype",
                Method::Stored,
                b"application/vnd.elektrawatt.archive+zip",
            ),
            ("a.txt", Method::Deflated, b"alpha alpha alpha\n"),
            ("dir/b.bin", Method::Deflated, &[0u8, 1, 2, 3, 255]),
            ("empty", Method::Deflated, b""),
        ])
        .unwrap()
    }

    /// The fixed fields of doc 05 §12: DOS time 1980-01-01 00:00, the UTF-8 flag, no extra field, mode 0644, the
    /// `mimetype` name at byte 30 and its stored value at byte 38.
    #[test]
    fn the_fixed_fields() {
        let bytes = sample();
        assert_eq!(&bytes[0..4], &LOCAL_SIGNATURE.to_le_bytes());
        assert_eq!(u16::from_le_bytes([bytes[6], bytes[7]]), 0x0800, "UTF-8 flag");
        assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 0, "stored");
        assert_eq!(u16::from_le_bytes([bytes[10], bytes[11]]), 0, "00:00:00");
        assert_eq!(u16::from_le_bytes([bytes[12], bytes[13]]), 0x0021, "1980-01-01");
        assert_eq!(u16::from_le_bytes([bytes[28], bytes[29]]), 0, "no extra field");
        assert_eq!(&bytes[30..38], b"mimetype");
        assert_eq!(&bytes[38..77], b"application/vnd.elektrawatt.archive+zip");
        // The end record is the last 22 bytes; the central directory's first header carries mode 0644.
        let end = bytes.len() - 22;
        assert_eq!(&bytes[end..end + 4], &END_SIGNATURE.to_le_bytes());
        let cd = u32::from_le_bytes(bytes[end + 16..end + 20].try_into().unwrap()) as usize;
        assert_eq!(
            u32::from_le_bytes(bytes[cd + 38..cd + 42].try_into().unwrap()),
            0o100_644 << 16
        );
        assert_eq!(u16::from_le_bytes([bytes[cd + 4], bytes[cd + 5]]), 0x0314);
    }

    /// The same entries give the same bytes; the reader returns them in order with their methods.
    #[test]
    fn deterministic_and_round_trips() {
        assert_eq!(sample(), sample());
        let entries = read(&sample(), &ZipLimits::default()).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["mimetype", "a.txt", "dir/b.bin", "empty"]);
        assert_eq!(entries[0].method, Method::Stored);
        assert_eq!(entries[1].method, Method::Deflated);
        assert_eq!(entries[1].data, b"alpha alpha alpha\n");
        assert_eq!(entries[2].data, [0u8, 1, 2, 3, 255]);
        assert!(entries[3].data.is_empty());
    }

    /// Names the extractor rules refuse are refused by the writer too, so that the writer never makes what the reader
    /// refuses.
    #[test]
    fn bad_names_are_refused() {
        for bad in [
            "", "/abs", "a\\b", "c:x", "a//b", "a/../b", "./a", "dir/", "nul\0",
        ] {
            assert!(
                matches!(write(&[(bad, Method::Stored, b"")]), Err(ZipError::Name { .. })),
                "{bad:?}"
            );
        }
        assert!(matches!(
            write(&[("A.txt", Method::Stored, b""), ("a.TXT", Method::Stored, b"")]),
            Err(ZipError::Duplicate { .. })
        ));
        assert!(matches!(
            write(&[("a", Method::Stored, b""), ("A/b", Method::Stored, b"")]),
            Err(ZipError::Name { .. })
        ));
    }

    /// A flipped byte anywhere is refused or changes an entry's data; it never reads as the original.
    #[test]
    fn damage_is_detected() {
        let good = sample();
        let original = read(&good, &ZipLimits::default()).unwrap();
        for i in 0..good.len() {
            let mut bad = good.clone();
            bad[i] ^= 0x55;
            if let Ok(entries) = read(&bad, &ZipLimits::default()) {
                assert_ne!(entries, original, "byte {i}");
            }
        }
        // Trailing bytes and a truncated archive.
        let mut longer = good.clone();
        longer.push(0);
        assert!(read(&longer, &ZipLimits::default()).is_err());
        assert!(read(&good[..good.len() - 1], &ZipLimits::default()).is_err());
        assert!(read(&[], &ZipLimits::default()).is_err());
    }

    /// The bounds are applied to the declared sizes before anything is inflated.
    #[test]
    fn limits_refuse_before_inflating() {
        let big = vec![0u8; 200_000];
        let bytes = write(&[("z", Method::Deflated, &big)]).unwrap();
        let small = ZipLimits {
            max_entry_bytes: 100_000,
            ..ZipLimits::default()
        };
        assert!(matches!(read(&bytes, &small), Err(ZipError::Limit { .. })));
        let total = ZipLimits {
            max_total_bytes: 100_000,
            ..ZipLimits::default()
        };
        assert!(matches!(read(&bytes, &total), Err(ZipError::Limit { .. })));
        // 200,000 zeros deflate far beyond 200 : 1.
        assert!(matches!(
            read(&bytes, &ZipLimits::default()),
            Err(ZipError::Limit {
                what: "the compression ratio",
                ..
            })
        ));
        let entries = ZipLimits {
            max_entries: 0,
            ..ZipLimits::default()
        };
        assert!(matches!(read(&bytes, &entries), Err(ZipError::Limit { .. })));
    }

    proptest! {
        /// Any entries round-trip through the writer and the reader, in order, and write to the same bytes twice.
        #[test]
        fn round_trip(files in proptest::collection::btree_map("[a-z]{1,4}/[a-z0-9_.]{1,8}", proptest::collection::vec(any::<u8>(), 0..300), 0..8)) {
            // `.` and `..` segments are refused by design; the generator's other names are all accepted.
            prop_assume!(files.keys().all(|n| !n.ends_with("/.") && !n.ends_with("/..")));
            let entries: Vec<(&str, Method, &[u8])> = files
                .iter()
                .enumerate()
                .map(|(i, (n, d))| (n.as_str(), if i % 2 == 0 { Method::Deflated } else { Method::Stored }, d.as_slice()))
                .collect();
            // Names equal ignoring case cannot occur: the generator writes lower case only.
            let bytes = write(&entries).unwrap();
            prop_assert_eq!(&bytes, &write(&entries).unwrap());
            let read_back = read(&bytes, &ZipLimits::default()).unwrap();
            prop_assert_eq!(read_back.len(), entries.len());
            for (e, (n, m, d)) in read_back.iter().zip(&entries) {
                prop_assert_eq!(e.name.as_str(), *n);
                prop_assert_eq!(e.method, *m);
                prop_assert_eq!(e.data.as_slice(), *d);
            }
        }
    }
}
