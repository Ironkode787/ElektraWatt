// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The sections of a DXF file into a [`Drawing`]: the header variables the importer reads, the LAYER, LTYPE, STYLE
//! and BLOCK_RECORD tables, the blocks, the entities and the layouts and image definitions of the OBJECTS section.
//! Other sections and tables are skipped pair by pair. Entities stay as their pairs; [`crate::geom`] reads them.
//!
//! Limits (doc 14 §11): at most [`MAX_ENTITIES`] entities in the whole file (blocks included), each string within
//! the context's `max_text_bytes`. The structure is read leniently: a missing `ENDSEC`, `ENDBLK`, `SEQEND` or `EOF`
//! ends what it would have ended.

use std::collections::BTreeMap;

use ew_interop_api::{ImportError, Limits};

use crate::pairs::{Pair, Pairs, Val};

/// The most entities one file may hold (doc 14 §11: 5 million).
pub const MAX_ENTITIES: usize = 5_000_000;

/// One entity: its type, its pairs after the type, and its VERTEX or ATTRIB followers.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Entity {
    pub kind: String,
    pub pairs: Vec<Pair>,
    pub children: Vec<Entity>,
    pub line: Option<u32>,
}

impl Entity {
    /// The first value of `code`.
    pub fn get(&self, code: i32) -> Option<&Pair> {
        self.pairs.iter().find(|p| p.code == code)
    }

    /// The first string of `code`.
    pub fn str(&self, code: i32) -> Option<String> {
        self.get(code).map(Pair::text)
    }

    /// The first real number of `code`, else `default`.
    pub fn f(&self, code: i32, default: f64) -> f64 {
        self.get(code).and_then(Pair::num).unwrap_or(default)
    }

    /// The first integer of `code`, else `default`.
    pub fn i(&self, code: i32, default: i64) -> i64 {
        self.get(code).and_then(Pair::int).unwrap_or(default)
    }

    /// The point of codes `x` and `x + 10`.
    pub fn pt(&self, x: i32) -> (f64, f64) {
        (self.f(x, 0.0), self.f(x + 10, 0.0))
    }

    /// The handle (code 5), upper case.
    pub fn handle(&self) -> Option<String> {
        self.str(5).map(|h| h.trim().to_ascii_uppercase())
    }

    /// The layer (code 8), default `0`.
    pub fn layer(&self) -> String {
        self.str(8)
            .map_or_else(|| "0".to_owned(), |l| l.trim().to_owned())
    }

    /// Whether the entity lies in paper space (code 67 = 1).
    pub fn paper(&self) -> bool {
        self.i(67, 0) == 1
    }

    /// The owner handle (the first code 330), upper case.
    pub fn owner(&self) -> Option<String> {
        self.str(330).map(|h| h.trim().to_ascii_uppercase())
    }

    /// The extrusion direction (codes 210, 220, 230), default `(0, 0, 1)`.
    pub fn extrusion(&self) -> (f64, f64, f64) {
        (self.f(210, 0.0), self.f(220, 0.0), self.f(230, 1.0))
    }
}

/// A layer of the LAYER table.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Layer {
    /// The name as written.
    pub name: String,
    /// Off (a negative colour) or frozen (flag 1).
    pub hidden: bool,
    /// The linetype name.
    pub linetype: String,
}

/// A block of the BLOCKS section.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Block {
    pub name: String,
    pub base: (f64, f64),
    /// Block flags (1 anonymous, 4 external reference).
    pub flags: i64,
    pub entities: Vec<Entity>,
    /// The handle of its BLOCK_RECORD (the BLOCK entity's owner).
    pub record: Option<String>,
}

/// A paper-space layout of the OBJECTS section.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Layout {
    pub name: String,
    pub tab: i64,
    /// The handle of the block record holding its entities.
    pub record: Option<String>,
}

/// A whole file.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Drawing {
    /// `$ACADVER`, `AC1009` when absent.
    pub version: String,
    /// The header variables read, by name without `$`: their pairs.
    pub header: BTreeMap<String, Vec<Pair>>,
    /// Layers by upper-case name.
    pub layers: BTreeMap<String, Layer>,
    /// Linetypes by upper-case name: their dash elements (code 49).
    pub ltypes: BTreeMap<String, Vec<f64>>,
    /// Text styles by upper-case name: their font file (code 3).
    pub styles: BTreeMap<String, String>,
    /// Block record names by upper-case handle.
    pub records: BTreeMap<String, String>,
    /// Blocks by upper-case name.
    pub blocks: BTreeMap<String, Block>,
    /// The ENTITIES section in order.
    pub entities: Vec<Entity>,
    /// The layouts in tab order.
    pub layouts: Vec<Layout>,
    /// Image definition file names by upper-case handle.
    pub images: BTreeMap<String, String>,
    /// Whether the file is binary.
    pub binary: bool,
}

impl Drawing {
    /// The integer header variable `name`.
    pub fn header_int(&self, name: &str) -> Option<i64> {
        self.header.get(name)?.iter().find_map(Pair::int)
    }

    /// The point header variable `name` (codes 10 and 20).
    pub fn header_pt(&self, name: &str) -> Option<(f64, f64)> {
        let pairs = self.header.get(name)?;
        let x = pairs.iter().find(|p| p.code == 10)?.num()?;
        let y = pairs.iter().find(|p| p.code == 20)?.num()?;
        Some((x, y))
    }
}

struct Reader<'a> {
    pairs: Pairs<'a>,
    entities: usize,
}

fn limit(what: &str, limit: usize) -> ImportError {
    ImportError::LimitExceeded {
        what: what.to_owned(),
        limit: limit as u64,
    }
}

impl Reader<'_> {
    fn next(&mut self) -> Result<Option<Pair>, ImportError> {
        self.pairs.next_pair()
    }

    /// Whether the next pair is `0 <name>` (not consumed).
    fn at(&mut self, name: &str) -> Result<bool, ImportError> {
        Ok(self.pairs.peek()?.is_some_and(|p| p.is(name)))
    }

    /// Whether the next pair has code 0 or the input ends.
    fn at_zero(&mut self) -> Result<bool, ImportError> {
        Ok(self.pairs.peek()?.is_none_or(|p| p.code == 0))
    }

    /// The pairs up to the next code 0.
    fn body(&mut self) -> Result<Vec<Pair>, ImportError> {
        let mut out = Vec::new();
        while !self.at_zero()? {
            if let Some(p) = self.next()? {
                out.push(p);
            }
        }
        Ok(out)
    }

    /// Skips to the pair after `0 ENDSEC` (or the end).
    fn skip_section(&mut self) -> Result<(), ImportError> {
        while let Some(p) = self.next()? {
            if p.is("ENDSEC") {
                break;
            }
        }
        Ok(())
    }

    /// One entity at `0 <type>`, with its followers; `None` at `stop` (consumed) or the end.
    fn entity(&mut self, stop: &str) -> Result<Option<Entity>, ImportError> {
        match self.pairs.peek()? {
            None => return Ok(None),
            Some(p) if p.is("ENDSEC") || p.is("EOF") => return Ok(None),
            _ => {}
        }
        let Some(head) = self.next()? else {
            return Ok(None);
        };
        if head.is(stop) {
            return Ok(None);
        }
        self.entities += 1;
        if self.entities > MAX_ENTITIES {
            return Err(limit("DXF entity count", MAX_ENTITIES));
        }
        let kind = head.text().trim().to_ascii_uppercase();
        let pairs = self.body()?;
        let mut e = Entity {
            kind,
            pairs,
            children: Vec::new(),
            line: head.line,
        };
        let follower = match e.kind.as_str() {
            "POLYLINE" => Some("VERTEX"),
            "INSERT" if e.i(66, 0) == 1 => Some("ATTRIB"),
            _ => None,
        };
        if let Some(follower) = follower {
            while self.at(follower)? {
                let head = self.next()?.unwrap_or(Pair {
                    code: 0,
                    val: Val::Str(String::new()),
                    line: None,
                });
                self.entities += 1;
                if self.entities > MAX_ENTITIES {
                    return Err(limit("DXF entity count", MAX_ENTITIES));
                }
                e.children.push(Entity {
                    kind: follower.to_owned(),
                    pairs: self.body()?,
                    children: Vec::new(),
                    line: head.line,
                });
            }
            if self.at("SEQEND")? {
                self.next()?;
                self.body()?;
            }
        }
        Ok(Some(e))
    }
}

/// The drawing of `bytes` under `limits`.
///
/// # Errors
///
/// [`ImportError::Malformed`] for a file that is no DXF (no section) or whose pairs do not parse,
/// [`ImportError::LimitExceeded`] for a string or line over `max_text_bytes` or more than [`MAX_ENTITIES`] entities.
pub(crate) fn parse(bytes: &[u8], limits: &Limits) -> Result<Drawing, ImportError> {
    // The entities are counted in a streaming pass first, so a file over the limit is refused before any of them
    // is held (doc 14 §11's 256 MB for a hostile input). It stops at EOF, as the parse does: what follows (an MS-DOS
    // end-of-file mark) is not read.
    let mut scan = Pairs::new(bytes, limits);
    let mut count = 0usize;
    while let Some(p) = scan.next_pair()? {
        if p.code == 0 {
            if p.is("EOF") {
                break;
            }
            count += 1;
            if count > MAX_ENTITIES {
                return Err(limit("DXF entity count", MAX_ENTITIES));
            }
        }
    }
    let mut r = Reader {
        pairs: Pairs::new(bytes, limits),
        entities: 0,
    };
    let mut d = Drawing {
        version: "AC1009".to_owned(),
        binary: r.pairs.is_binary(),
        ..Drawing::default()
    };
    let mut sections = 0;
    while let Some(p) = r.next()? {
        if p.is("EOF") {
            break;
        }
        if !p.is("SECTION") {
            // A comment (999) or stray pair between sections.
            continue;
        }
        sections += 1;
        // The section's name is the pair right after `0 SECTION` (its other pairs, a header's variables, follow).
        let name = match r.pairs.peek()? {
            Some(p) if p.code == 2 => r.next()?.map(|p| p.text().trim().to_ascii_uppercase()),
            _ => None,
        };
        match name.as_deref() {
            Some("HEADER") => header(&mut r, &mut d)?,
            Some("TABLES") => tables(&mut r, &mut d)?,
            Some("BLOCKS") => blocks(&mut r, &mut d)?,
            Some("ENTITIES") => {
                while let Some(e) = r.entity("ENDSEC")? {
                    d.entities.push(e);
                }
                if r.at("ENDSEC")? {
                    r.next()?;
                }
            }
            Some("OBJECTS") => objects(&mut r, &mut d)?,
            _ => r.skip_section()?,
        }
    }
    if sections == 0 {
        return Err(ImportError::Malformed {
            message: "the file has no DXF section".to_owned(),
            line: None,
        });
    }
    d.layouts.sort_by(|a, b| (a.tab, &a.name).cmp(&(b.tab, &b.name)));
    Ok(d)
}

fn header(r: &mut Reader<'_>, d: &mut Drawing) -> Result<(), ImportError> {
    let mut current: Option<String> = None;
    while let Some(p) = r.next()? {
        if p.is("ENDSEC") || p.is("EOF") {
            break;
        }
        if p.code == 9 {
            let name = p.text().trim().trim_start_matches('$').to_ascii_uppercase();
            d.header.entry(name.clone()).or_default();
            current = Some(name);
            continue;
        }
        if let Some(name) = &current {
            if name == "ACADVER" && p.code == 1 {
                d.version = p.text().trim().to_ascii_uppercase();
                // Text is UTF-8 from AutoCAD 2007 (AC1021) on.
                r.pairs.utf8 = d.version.as_str() >= "AC1021";
            }
            d.header.entry(name.clone()).or_default().push(p);
        }
    }
    Ok(())
}

fn tables(r: &mut Reader<'_>, d: &mut Drawing) -> Result<(), ImportError> {
    while let Some(p) = r.next()? {
        if p.is("ENDSEC") || p.is("EOF") {
            break;
        }
        if p.code != 0 {
            continue;
        }
        if p.is("TABLE") || p.is("ENDTAB") {
            // The table head's own pairs.
            let _ = r.body()?;
            continue;
        }
        let kind = p.text().trim().to_ascii_uppercase();
        let body = r.body()?;
        let e = Entity {
            kind,
            pairs: body,
            children: Vec::new(),
            line: p.line,
        };
        let name = e.str(2).unwrap_or_default().trim().to_owned();
        match e.kind.as_str() {
            "LAYER" => {
                let colour = e.i(62, 7);
                let flags = e.i(70, 0);
                d.layers.insert(
                    name.to_ascii_uppercase(),
                    Layer {
                        hidden: colour < 0 || flags & 1 != 0,
                        linetype: e
                            .str(6)
                            .unwrap_or_else(|| "CONTINUOUS".to_owned())
                            .trim()
                            .to_owned(),
                        name,
                    },
                );
            }
            "LTYPE" => {
                let dashes = e
                    .pairs
                    .iter()
                    .filter(|p| p.code == 49)
                    .filter_map(Pair::num)
                    .collect();
                d.ltypes.insert(name.to_ascii_uppercase(), dashes);
            }
            "STYLE" => {
                d.styles.insert(
                    name.to_ascii_uppercase(),
                    e.str(3).unwrap_or_default().trim().to_owned(),
                );
            }
            "BLOCK_RECORD" => {
                if let Some(h) = e.handle() {
                    d.records.insert(h, name);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn blocks(r: &mut Reader<'_>, d: &mut Drawing) -> Result<(), ImportError> {
    while let Some(p) = r.next()? {
        if p.is("ENDSEC") || p.is("EOF") {
            break;
        }
        if !p.is("BLOCK") {
            let _ = r.body()?;
            continue;
        }
        let head = Entity {
            kind: "BLOCK".to_owned(),
            pairs: r.body()?,
            children: Vec::new(),
            line: p.line,
        };
        let mut entities = Vec::new();
        while let Some(e) = r.entity("ENDBLK")? {
            entities.push(e);
        }
        // ENDBLK's own pairs.
        let _ = r.body()?;
        let name = head
            .str(2)
            .or_else(|| head.str(3))
            .unwrap_or_default()
            .trim()
            .to_owned();
        d.blocks.insert(
            name.to_ascii_uppercase(),
            Block {
                base: head.pt(10),
                flags: head.i(70, 0),
                record: head.owner(),
                entities,
                name,
            },
        );
    }
    Ok(())
}

fn objects(r: &mut Reader<'_>, d: &mut Drawing) -> Result<(), ImportError> {
    while let Some(p) = r.next()? {
        if p.is("ENDSEC") || p.is("EOF") {
            break;
        }
        let kind = p.text().trim().to_ascii_uppercase();
        let e = Entity {
            kind,
            pairs: r.body()?,
            children: Vec::new(),
            line: p.line,
        };
        match e.kind.as_str() {
            "LAYOUT" => d.layouts.push(Layout {
                name: e.str(1).unwrap_or_default().trim().to_owned(),
                tab: e.i(71, 0),
                record: e.str(340).map(|h| h.trim().to_ascii_uppercase()),
            }),
            "IMAGEDEF" => {
                if let (Some(h), Some(file)) = (e.handle(), e.str(1)) {
                    d.images.insert(h, file);
                }
            }
            _ => {}
        }
    }
    Ok(())
}
