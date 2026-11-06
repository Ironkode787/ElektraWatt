// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The 7z header walked under EW's own bounds before `sevenz-rust2` allocates for it (ID-1460): the start header
//! and its CRC, the entry, folder, coder and stream counts, the declared sizes, the dictionaries and the ratio.
//! A compressed (encoded) header is decoded here under the same bounds, and its header walked in turn. The walk
//! follows the grammar `sevenz-rust2` 0.24.2 reads, so the counts it later allocates for are the ones bounded here.

use std::io::Read;

use crate::{MAX_FILE, MAX_TOTAL};

const INVALID: &str = "eplan-archive-invalid";
const LIMIT: &str = "eplan-input-limit";
const METHOD: &str = "eplan-archive-method";

/// The most entries, folders, packed streams and unpacked streams of one archive.
const MAX_COUNT: u64 = 4096;
/// The most coders of one folder.
const MAX_CODERS: u64 = 4;
/// The largest dictionary of a coder: the decoder allocates it whole (LZMA2) or up to the unpacked size (LZMA).
const MAX_DICTIONARY: u64 = MAX_TOTAL;
/// The most unpacked bytes per packed byte of a folder; a folder of at most 1 KiB is exempt.
const RATIO: u64 = 200;

const SIGNATURE: usize = 32;
const END: u8 = 0x00;
const HEADER: u8 = 0x01;
const ARCHIVE_PROPERTIES: u8 = 0x02;
const MAIN_STREAMS_INFO: u8 = 0x04;
const FILES_INFO: u8 = 0x05;
const PACK_INFO: u8 = 0x06;
const UNPACK_INFO: u8 = 0x07;
const SUB_STREAMS_INFO: u8 = 0x08;
const SIZE: u8 = 0x09;
const CRC: u8 = 0x0A;
const FOLDER: u8 = 0x0B;
const CODERS_UNPACK_SIZE: u8 = 0x0C;
const NUM_UNPACK_STREAM: u8 = 0x0D;
const ENCODED_HEADER: u8 = 0x17;

const COPY: &[u8] = &[0x00];
const LZMA: &[u8] = &[0x03, 0x01, 0x01];
const LZMA2: &[u8] = &[0x21];

struct Bytes<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Bytes<'a> {
    fn byte(&mut self) -> Result<u8, String> {
        let v = *self.b.get(self.at).ok_or(INVALID)?;
        self.at += 1;
        Ok(v)
    }
    fn take(&mut self, n: u64) -> Result<&'a [u8], String> {
        let end = usize::try_from(n)
            .ok()
            .and_then(|n| self.at.checked_add(n))
            .filter(|e| *e <= self.b.len())
            .ok_or(INVALID)?;
        let s = &self.b[self.at..end];
        self.at = end;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().map_err(|_| INVALID)?))
    }
    /// A 7z NUMBER: the first byte's leading ones count the bytes that follow.
    fn number(&mut self) -> Result<u64, String> {
        let first = self.byte()?;
        let mut value = 0u64;
        for i in 0..8 {
            let mask = 0x80u8 >> i;
            if first & mask == 0 {
                return Ok(value | u64::from(first & (mask - 1)) << (8 * i));
            }
            value |= u64::from(self.byte()?) << (8 * i);
        }
        Ok(value)
    }
    fn count(&mut self) -> Result<usize, String> {
        let n = self.number()?;
        if n > MAX_COUNT {
            return Err(LIMIT.into());
        }
        Ok(n as usize)
    }
    /// The "all defined" byte, else a bit vector of `n` items.
    fn defined(&mut self, n: usize) -> Result<Vec<bool>, String> {
        if self.byte()? != 0 {
            return Ok(vec![true; n]);
        }
        let bits = self.take(n.div_ceil(8) as u64)?;
        Ok((0..n).map(|i| bits[i / 8] & (0x80 >> (i % 8)) != 0).collect())
    }
    fn digests(&mut self, n: usize) -> Result<Vec<Option<u32>>, String> {
        let defined = self.defined(n)?;
        defined
            .into_iter()
            .map(|d| if d { self.u32().map(Some) } else { Ok(None) })
            .collect()
    }
    /// Property records up to their end byte, each skipped by its declared size.
    fn skip_properties(&mut self) -> Result<(), String> {
        while self.byte()? != END {
            let n = self.number()?;
            self.take(n)?;
        }
        Ok(())
    }
}

/// One folder: its simple coders (method and properties) in declaration order, their unpack sizes and the index
/// of the coder whose output is the folder's.
#[derive(Default)]
struct Folder<'a> {
    coders: Vec<(&'a [u8], &'a [u8])>,
    sizes: Vec<u64>,
    last: usize,
    crc: Option<u32>,
}

impl Folder<'_> {
    fn unpack(&self) -> u64 {
        self.sizes.get(self.last).copied().unwrap_or(0)
    }
}

#[derive(Default)]
struct Streams<'a> {
    pack_pos: u64,
    packs: Vec<u64>,
    folders: Vec<Folder<'a>>,
}

/// The dictionary a coder's decoder allocates is bounded; only the copy, LZMA and LZMA2 methods are read.
fn dictionary(method: &[u8], props: &[u8]) -> Result<(), String> {
    let size = match method {
        COPY => return Ok(()),
        LZMA => {
            let p: [u8; 5] = props.try_into().map_err(|_| INVALID)?;
            u64::from(u32::from_le_bytes([p[1], p[2], p[3], p[4]]))
        }
        LZMA2 => match props {
            [bits] if *bits <= 40 => {
                let bits = u64::from(*bits);
                (2 | (bits & 1)) << (bits / 2 + 11)
            }
            _ => return Err(INVALID.into()),
        },
        _ => return Err(METHOD.into()),
    };
    if size > MAX_DICTIONARY {
        return Err(LIMIT.into());
    }
    Ok(())
}

fn folder<'a>(r: &mut Bytes<'a>) -> Result<Folder<'a>, String> {
    let n = r.number()?;
    if n == 0 {
        return Err(INVALID.into());
    }
    if n > MAX_CODERS {
        return Err(LIMIT.into());
    }
    let n = n as usize;
    let mut f = Folder::default();
    for _ in 0..n {
        let flags = r.byte()?;
        if flags & 0xC0 != 0 {
            return Err(INVALID.into());
        }
        // A complex coder (several streams, such as BCJ2) is not one of the methods read.
        if flags & 0x10 != 0 {
            return Err(METHOD.into());
        }
        let method = r.take(u64::from(flags & 0x0F))?;
        let props = if flags & 0x20 != 0 {
            let len = r.number()?;
            if len > 64 {
                return Err(INVALID.into());
            }
            r.take(len)?
        } else {
            &[]
        };
        dictionary(method, props)?;
        f.coders.push((method, props));
    }
    let (mut ins, mut outs) = (vec![false; n], vec![false; n]);
    for _ in 1..n {
        let (i, o) = (r.number()?, r.number()?);
        let (i, o) = (usize::try_from(i).unwrap_or(n), usize::try_from(o).unwrap_or(n));
        if i >= n || o >= n || ins[i] || outs[o] {
            return Err(INVALID.into());
        }
        ins[i] = true;
        outs[o] = true;
    }
    f.last = outs.iter().position(|bound| !bound).ok_or(INVALID)?;
    Ok(f)
}

/// A StreamsInfo; `max_unpack` bounds each declared unpacked size, `input` is the whole archive.
fn streams<'a>(r: &mut Bytes<'a>, max_unpack: u64, input: usize) -> Result<Streams<'a>, String> {
    let mut s = Streams::default();
    let mut id = r.byte()?;
    if id == PACK_INFO {
        s.pack_pos = r.number()?;
        let n = r.count()?;
        id = r.byte()?;
        if id == SIZE {
            for _ in 0..n {
                s.packs.push(r.number()?);
            }
            id = r.byte()?;
        }
        if id == CRC {
            r.digests(n)?;
            id = r.byte()?;
        }
        if id != END {
            return Err(INVALID.into());
        }
        id = r.byte()?;
    }
    if id == UNPACK_INFO {
        if r.byte()? != FOLDER {
            return Err(INVALID.into());
        }
        let n = r.count()?;
        if r.byte()? != 0 {
            return Err(INVALID.into());
        }
        for _ in 0..n {
            s.folders.push(folder(r)?);
        }
        if r.byte()? != CODERS_UNPACK_SIZE {
            return Err(INVALID.into());
        }
        for f in &mut s.folders {
            for _ in 0..f.coders.len() {
                let size = r.number()?;
                if size > max_unpack {
                    return Err(LIMIT.into());
                }
                f.sizes.push(size);
            }
        }
        id = r.byte()?;
        if id == CRC {
            for (f, crc) in s.folders.iter_mut().zip(r.digests(n)?) {
                f.crc = crc;
            }
            id = r.byte()?;
        }
        if id != END {
            return Err(INVALID.into());
        }
        id = r.byte()?;
    }
    if id == SUB_STREAMS_INFO {
        let mut counts = vec![1usize; s.folders.len()];
        id = r.byte()?;
        if id == NUM_UNPACK_STREAM {
            let mut total = 0;
            for c in &mut counts {
                *c = r.count()?;
                total += *c;
                if total as u64 > MAX_COUNT {
                    return Err(LIMIT.into());
                }
            }
            id = r.byte()?;
        }
        if id == SIZE {
            for (f, &c) in s.folders.iter().zip(&counts) {
                let mut sum = 0u64;
                for _ in 1..c.max(1) {
                    sum = sum.checked_add(r.number()?).ok_or(INVALID)?;
                }
                if sum > f.unpack() {
                    return Err(INVALID.into());
                }
            }
            id = r.byte()?;
        }
        let digests = s
            .folders
            .iter()
            .zip(&counts)
            .map(|(f, &c)| if c == 1 && f.crc.is_some() { 0 } else { c })
            .sum();
        if id == CRC {
            r.digests(digests)?;
            id = r.byte()?;
        }
        if id != END {
            return Err(INVALID.into());
        }
        id = r.byte()?;
    }
    if id != END {
        return Err(INVALID.into());
    }
    let packed = s
        .packs
        .iter()
        .try_fold(s.pack_pos, |a, &p| a.checked_add(p))
        .ok_or(INVALID)?;
    if packed > (input - SIGNATURE) as u64 {
        return Err(INVALID.into());
    }
    // Each folder reads one packed stream (its coders are simple), in order.
    if s.folders.len() > s.packs.len() {
        return Err(INVALID.into());
    }
    for (f, &p) in s.folders.iter().zip(&s.packs) {
        if f.unpack() > p.saturating_mul(RATIO).max(1024) {
            return Err(LIMIT.into());
        }
    }
    Ok(s)
}

/// A plain Header: archive properties, the main streams and the files' count, each bounded.
fn header(r: &mut Bytes<'_>, input: usize) -> Result<(), String> {
    let mut id = r.byte()?;
    if id == ARCHIVE_PROPERTIES {
        r.skip_properties()?;
        id = r.byte()?;
    }
    if id == MAIN_STREAMS_INFO {
        let s = streams(r, MAX_TOTAL, input)?;
        let total = s.folders.iter().try_fold(0u64, |a, f| a.checked_add(f.unpack()));
        if total.is_none_or(|t| t > MAX_TOTAL) {
            return Err(LIMIT.into());
        }
        id = r.byte()?;
    }
    if id == FILES_INFO {
        r.count()?;
        r.skip_properties()?;
        id = r.byte()?;
    }
    if id != END {
        return Err(INVALID.into());
    }
    Ok(())
}

/// Decodes the one-folder, one-coder stream of an encoded header, within its declared size.
fn decode(s: &Streams<'_>, bytes: &[u8]) -> Result<Vec<u8>, String> {
    let (Some(f), Some(&pack)) = (s.folders.first(), s.packs.first()) else {
        return Err(INVALID.into());
    };
    let &[(method, props)] = f.coders.as_slice() else {
        return Err(METHOD.into());
    };
    let start = (SIGNATURE as u64 + s.pack_pos) as usize;
    let packed = bytes.get(start..start + pack as usize).ok_or(INVALID)?;
    let size = f.unpack();
    let mut out = Vec::with_capacity(size as usize);
    let read = match method {
        COPY => Read::take(packed, size).read_to_end(&mut out),
        LZMA => lzma_rust2::LzmaReader::new_with_props(
            packed,
            size,
            props[0],
            u32::from_le_bytes([props[1], props[2], props[3], props[4]]),
            None,
        )
        .and_then(|d| d.take(size).read_to_end(&mut out)),
        _ => {
            let bits = u32::from(props[0]);
            let dictionary = if bits == 40 {
                u32::MAX
            } else {
                (2 | (bits & 1)) << (bits / 2 + 11)
            };
            lzma_rust2::Lzma2Reader::new(packed, dictionary, None)
                .take(size)
                .read_to_end(&mut out)
        }
    };
    if read.is_err() || out.len() as u64 != size || f.crc.is_some_and(|c| c != crc32fast::hash(&out)) {
        return Err(INVALID.into());
    }
    Ok(out)
}

/// Walks the archive's headers under the bounds above before `sevenz-rust2` reads them.
///
/// # Errors
/// `eplan-archive-invalid` for a damaged or inconsistent header, `eplan-archive-method` for a method other than
/// copy, LZMA and LZMA2 (encryption included), `eplan-input-limit` for a count, size, dictionary or ratio over
/// its bound.
pub(crate) fn check(bytes: &[u8]) -> Result<(), String> {
    let start = bytes.get(8..SIGNATURE).ok_or(INVALID)?;
    let mut r = Bytes { b: start, at: 0 };
    if bytes[6] != 0 || r.u32()? != crc32fast::hash(&start[4..]) {
        return Err(INVALID.into());
    }
    let offset = r.take(8)?;
    let size = r.take(8)?;
    let (offset, size) = (
        u64::from_le_bytes(offset.try_into().map_err(|_| INVALID)?),
        u64::from_le_bytes(size.try_into().map_err(|_| INVALID)?),
    );
    let crc = r.u32()?;
    if size > MAX_FILE {
        return Err(LIMIT.into());
    }
    let begin = offset
        .checked_add(SIGNATURE as u64)
        .filter(|b| b.saturating_add(size) <= bytes.len() as u64)
        .ok_or(INVALID)? as usize;
    let next = &bytes[begin..begin + size as usize];
    if next.is_empty() || crc32fast::hash(next) != crc {
        return Err(INVALID.into());
    }
    let mut r = Bytes { b: next, at: 0 };
    match r.byte()? {
        HEADER => header(&mut r, bytes.len()),
        ENCODED_HEADER => {
            let s = streams(&mut r, MAX_FILE, bytes.len())?;
            let decoded = decode(&s, bytes)?;
            let mut r = Bytes { b: &decoded, at: 0 };
            if r.byte()? != HEADER {
                return Err(INVALID.into());
            }
            header(&mut r, bytes.len())
        }
        _ => Err(INVALID.into()),
    }
}
