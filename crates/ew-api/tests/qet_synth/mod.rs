// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The synthetic QElectroTech project of the QET import spike (doc 14 §4, doc 20 §2.3): folios 101 to 103 of
//! the golden project drawn the way QET stores a drawing, built from the public golden tables
//! (`golden/J-E-2416/tables`) and the public starter content (`content/`). Nothing private is read.
//!
//! What the builder does (QET 0.100 `.qet` XML, the subset the importer reads):
//!
//! - one `<diagram>` per drawn page; one embedded element definition per golden symbol (a black box per
//!   connection-point count, an interruption point per direction), its terminals the symbol's CPs on the
//!   10 px grid relative to the symbol's anchor, named by the function CP designation they map to;
//! - one `<element>` per device representation at the golden cell centre plus the table's offset, labelled
//!   with the device key as printed; interruption points as `next_report` / `previous_report` elements with
//!   a user text holding the IP name, paired by name;
//! - terminal strips as QET draws them: one `terminal` element per terminal, stacked per segment column, the
//!   first of each column labelled with the strip tag;
//! - junctions as one-terminal elements; every `wire` connection as a `<conductor>`, every third one with
//!   QET's older integer terminal ends, the rest with element UUID and terminal UUID.
//!
//! Bridges, cores and everything on other folios are not drawn; [`Fixture::skipped`] lists them. UUIDs come
//! from a counter (deterministic), element UUIDs are written upper case on every other element while all
//! references are lower case, so the importer's normalisation is exercised.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde_json::Value;

use crate::golden_common::repo;

pub mod scan;

/// The drawn folios of the fixture, in order.
pub const FOLIOS: [&str; 3] = ["101", "102", "103"];

/// The first UUID counter value: its hex digits contain letters, so upper and lower case differ.
const UUID_BASE: u64 = 0x00AB_C000;
/// The frame of the QET-compatible page: header, column width, row height (px).
const HEADER: i64 = 20;
const COL_PX: i64 = 60;
const ROW_PX: i64 = 80;
/// Distance of consecutive terminals of a strip column (px).
const STRIP_PITCH: i64 = 20;

type Row = BTreeMap<String, String>;

/// Definition terminals before they get UUIDs: position, orientation, name.
type Terms = Vec<(i64, i64, char, String)>;

fn col<'a>(r: &'a Row, c: &str) -> &'a str {
    r.get(c).map_or("", String::as_str)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn uuid_text(n: u64, upper: bool) -> String {
    let t = format!("{{00000000-0000-4000-8000-{n:012x}}}");
    if upper { t.to_ascii_uppercase() } else { t }
}

// ---------------------------------------------------------------------------------------------------------
// The golden tables and the starter content

#[derive(Clone, Debug)]
struct Cp {
    idx: u16,
    designation: String,
    side: Option<String>,
    clamp: Option<String>,
}

#[derive(Clone, Debug)]
struct SymCp {
    /// Position relative to the symbol anchor, px on the 10 px grid.
    pos: (i64, i64),
    fn_cp: u16,
    dir: char,
}

struct Golden {
    pages: Vec<Row>,
    reps: Vec<Row>,
    functions: Vec<Row>,
    devices: Vec<Row>,
    terminals: Vec<Row>,
    connections: Vec<Row>,
    junctions: Vec<Row>,
    part_cps: Vec<Row>,
    part_templates: Vec<Row>,
    fndefs: BTreeMap<String, Vec<Cp>>,
    symbols: BTreeMap<String, Result<Vec<SymCp>, String>>,
}

fn split(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for ch in line.chars() {
        match ch {
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// The rows of a golden table in file order.
fn rows(name: &str) -> Vec<Row> {
    let path = repo().join("golden/J-E-2416/tables").join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
    let mut lines = text.lines();
    let header = split(lines.next().expect("a header"));
    lines
        .map(|l| header.iter().cloned().zip(split(l)).collect())
        .collect()
}

/// nm to px, rounded to the nearest 10 px grid point (QET terminals sit on the 10 px grid; 1 px =
/// 793 750 / 3 nm).
fn grid_px(nm: i64) -> i64 {
    let n = i128::from(nm) * 3;
    let unit = 7_937_500i128;
    let q = if n >= 0 {
        (n + unit / 2) / unit
    } else {
        -((-n + unit / 2) / unit)
    };
    i64::try_from(q * 10).expect("a page coordinate")
}

fn pair_px(v: &Value) -> (i64, i64) {
    let a = v.as_array().expect("a point");
    (grid_px(a[0].as_i64().unwrap()), grid_px(a[1].as_i64().unwrap()))
}

fn clamp_text(v: &Value) -> Option<String> {
    match v {
        Value::Object(o) => {
            if let Some(Value::String(c)) = o.get("clamp") {
                Some(c.clone())
            } else {
                o.get("shaft").and_then(Value::as_u64).map(|n| format!("s{n}"))
            }
        }
        Value::String(s) if s.starts_with("rail") => Some("rail".to_owned()),
        _ => None,
    }
}

fn json_files(sub: &str) -> Vec<(String, Value)> {
    let dir = repo().join("content").join(sub);
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
            let key = v["meta"]["key"].as_str().expect("a key").to_owned();
            (key, v)
        })
        .collect()
}

impl Golden {
    fn load() -> Golden {
        let fndefs = json_files("fndefs")
            .into_iter()
            .map(|(k, v)| {
                let cps = v["cps"].as_array().map_or_else(Vec::new, |cps| {
                    cps.iter()
                        .map(|c| Cp {
                            idx: u16::try_from(c["idx"].as_u64().unwrap()).unwrap(),
                            designation: c["designation"].as_str().unwrap().to_owned(),
                            side: c["side"].as_str().map(str::to_owned),
                            clamp: clamp_text(&c["clamp"]),
                        })
                        .collect()
                });
                (k, cps)
            })
            .collect();
        let symbols = json_files("symbols/qet10")
            .into_iter()
            .map(|(k, v)| {
                let anchor = match &v["anchor"] {
                    Value::Null => (0, 0),
                    a if a.get("named").is_some() => pair_px(&a["named"]),
                    other => {
                        return (
                            k.clone(),
                            Err(format!("symbol {k}: anchor {other} is not drawn by the fixture")),
                        );
                    }
                };
                let cps = v["cps"].as_array().map_or_else(Vec::new, |cps| {
                    cps.iter()
                        .map(|c| {
                            let p = pair_px(&c["pos"]);
                            SymCp {
                                pos: (p.0 - anchor.0, p.1 - anchor.1),
                                fn_cp: u16::try_from(c["fn_cp"].as_u64().unwrap()).unwrap(),
                                dir: match c["dir"].as_str() {
                                    Some("down") => 's',
                                    Some("left") => 'w',
                                    Some("right") => 'e',
                                    _ => 'n',
                                },
                            }
                        })
                        .collect()
                });
                (k, Ok(cps))
            })
            .collect();
        Golden {
            pages: rows("pages.csv"),
            reps: rows("representations.csv"),
            functions: rows("functions.csv"),
            devices: rows("devices.csv"),
            terminals: rows("terminals.csv"),
            connections: rows("connections.csv"),
            junctions: rows("junctions.csv"),
            part_cps: rows("part_cps.csv"),
            part_templates: rows("part_templates.csv"),
            fndefs,
            symbols,
        }
    }

    fn function(&self, key: &str) -> &Row {
        self.functions
            .iter()
            .find(|r| col(r, "key") == key)
            .unwrap_or_else(|| panic!("no function `{key}`"))
    }

    fn device(&self, key: &str) -> &Row {
        self.devices
            .iter()
            .find(|r| col(r, "key") == key)
            .unwrap_or_else(|| panic!("no device `{key}`"))
    }

    fn is_terminal(&self, key: &str) -> bool {
        self.terminals.iter().any(|r| col(r, "key") == key)
    }

    fn is_strip(&self, device: &str) -> bool {
        self.devices.iter().any(|r| {
            col(r, "key") == device
                && ["strip_label_mode", "strip_groups", "strip_field_side"]
                    .iter()
                    .any(|c| !col(r, c).is_empty())
        })
    }

    /// The part keys of a device or terminal, in table order, quantities dropped.
    fn parts_of(&self, device: &str) -> Vec<String> {
        let row = self
            .devices
            .iter()
            .chain(&self.terminals)
            .find(|r| col(r, "key") == device);
        row.map_or_else(Vec::new, |r| {
            col(r, "parts")
                .split(';')
                .filter(|p| !p.is_empty())
                .map(|p| p.split('*').next().unwrap().to_owned())
                .collect()
        })
    }

    fn pattern(&self, part: &str) -> Vec<Cp> {
        self.part_cps
            .iter()
            .filter(|r| col(r, "part") == part)
            .map(|r| Cp {
                idx: col(r, "idx").parse().unwrap(),
                designation: col(r, "designation").to_owned(),
                side: Some(col(r, "side")).filter(|s| !s.is_empty()).map(str::to_owned),
                clamp: Some(col(r, "clamp")).filter(|s| !s.is_empty()).map(str::to_owned),
            })
            .collect()
    }

    /// The CPs of a function by the template-origin rule of the golden README.
    fn function_cps(&self, key: &str) -> Vec<Cp> {
        let f = self.function(key);
        let fndef = col(f, "fndef");
        if col(f, "kind") != "ip" {
            let main = col(f, "role") != "aux";
            let parts = self.parts_of(col(f, "device"));
            for p in &parts {
                for t in self.part_templates.iter().filter(|t| col(t, "part") == p) {
                    if col(t, "fndef") == fndef && (col(t, "main") == "true") == main {
                        let pattern = self.pattern(p);
                        return col(t, "cps")
                            .split(';')
                            .map(|i| {
                                let i: u16 = i.parse().unwrap();
                                pattern.iter().find(|c| c.idx == i).expect("a pattern CP").clone()
                            })
                            .collect();
                    }
                }
            }
            if col(f, "kind") == "black_box" {
                for p in &parts {
                    let pattern = self.pattern(p);
                    if !pattern.is_empty() {
                        return pattern;
                    }
                }
            }
        }
        self.fndefs
            .get(fndef)
            .unwrap_or_else(|| panic!("no FnDef `{fndef}`"))
            .clone()
    }

    /// An endpoint of `connections.csv` in the notation of the golden README.
    fn resolve(&self, text: &str) -> Ep {
        if let Some(j) = text.strip_prefix('@') {
            return Ep::Junction(j.to_owned());
        }
        if let Some(f) = text.strip_prefix("ip:") {
            return Ep::Cp(f.to_owned(), 0);
        }
        let (owner, rest) = text
            .split_once(':')
            .unwrap_or_else(|| panic!("endpoint `{text}`"));
        if self.is_strip(owner) {
            let (designation, sel) = rest
                .split_once('/')
                .unwrap_or_else(|| panic!("endpoint `{text}`"));
            let terminal = format!("{owner}:{designation}");
            let field_side = Some(col(self.device(owner), "strip_field_side"))
                .filter(|s| !s.is_empty())
                .unwrap_or("side2");
            let hits: Vec<Cp> = self
                .function_cps(&terminal)
                .into_iter()
                .filter(|c| match sel {
                    "field" => c.side.as_deref() == Some(field_side),
                    "internal" => c.side.is_some() && c.side.as_deref() != Some(field_side),
                    s => c.clamp.as_deref() == Some(s),
                })
                .collect();
            assert_eq!(hits.len(), 1, "endpoint `{text}` selects {} CPs", hits.len());
            return Ep::Cp(terminal, hits[0].idx);
        }
        let (designation, nth) = match rest.rsplit_once('#') {
            Some((d, n)) if n.bytes().all(|b| b.is_ascii_digit()) && !n.is_empty() => {
                (d, Some(n.parse::<usize>().unwrap()))
            }
            _ => (rest, None),
        };
        let mut hits = Vec::new();
        for f in self
            .functions
            .iter()
            .filter(|f| col(f, "device") == owner && col(f, "kind") != "ip")
        {
            let key = col(f, "key");
            for c in self.function_cps(key) {
                if c.designation == designation {
                    hits.push(Ep::Cp(key.to_owned(), c.idx));
                }
            }
        }
        match nth {
            Some(n) => hits.swap_remove(n - 1),
            None => {
                assert_eq!(hits.len(), 1, "endpoint `{text}` is ambiguous or unknown");
                hits.remove(0)
            }
        }
    }
}

/// A resolved golden endpoint.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Ep {
    /// Function key and CP index.
    Cp(String, u16),
    /// Junction key.
    Junction(String),
}

/// The cell `<row><column>` (`B8`) as the centre of the cell in page px.
fn cell_centre(cell: &str) -> (i64, i64) {
    let row = i64::from(u32::from(cell.chars().next().unwrap()) - u32::from('A'));
    let column: i64 = cell[1..].parse().unwrap();
    (
        HEADER + COL_PX * column + COL_PX / 2,
        HEADER + ROW_PX * row + ROW_PX / 2,
    )
}

fn offset(r: &Row) -> (i64, i64) {
    let n = |c: &str| col(r, c).parse::<i64>().unwrap_or(0);
    (n("dx"), n("dy"))
}

// ---------------------------------------------------------------------------------------------------------
// The drawing

struct DefTerm {
    x: i64,
    y: i64,
    orient: char,
    name: String,
    uuid: u64,
}

struct Def {
    name: String,
    link_type: &'static str,
    /// `<kindInformation name="type">`, for terminal elements.
    kind_type: Option<&'static str>,
    terminals: Vec<DefTerm>,
    uuid: u64,
}

struct Elem {
    uuid: u64,
    upper: bool,
    def: usize,
    x: i64,
    y: i64,
    orientation: u8,
    label: Option<String>,
    function: Option<String>,
    /// The user texts: (uuid, text).
    texts: Vec<(u64, String)>,
    /// The instance terminal IDs, one per definition terminal.
    term_ids: Vec<u32>,
    links: Vec<u64>,
}

#[derive(Clone, Copy)]
struct End {
    elem: usize,
    term: usize,
}

struct Conductor {
    a: End,
    b: End,
    integer: bool,
}

#[derive(Default)]
struct Diagram {
    folio: String,
    title: String,
    elems: Vec<Elem>,
    conductors: Vec<Conductor>,
}

/// How a golden function sits on a page: its element and which definition terminal each golden CP is.
struct Placed {
    elem: usize,
    cps: BTreeMap<u16, usize>,
}

/// Counts of one folio of the fixture.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct FolioStats {
    /// The folio label.
    pub folio: String,
    /// Elements in all.
    pub elements: usize,
    /// Device elements (symbols of normal functions).
    pub devices: usize,
    /// Terminal elements.
    pub terminals: usize,
    /// Report elements (interruption points).
    pub reports: usize,
    /// Junction elements.
    pub junctions: usize,
    /// Conductors in all.
    pub conductors: usize,
    /// Conductors written with integer terminal ends.
    pub integer_ends: usize,
}

/// A golden connection the fixture does not draw.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Skipped {
    /// The connection key (`W35.103`, `BR1.1`).
    pub key: String,
    /// Its kind (`core`, `bridge`).
    pub kind: String,
    /// The page cell of the table; empty for bridges.
    pub page: String,
}

/// The built fixture.
#[derive(Clone, Debug)]
pub struct Fixture {
    /// The `.qet` text.
    pub text: String,
    /// Counts per folio, in folio order.
    pub folios: Vec<FolioStats>,
    /// The golden connections on the folios, and the bridges of the drawn strips, that are not drawn.
    pub skipped: Vec<Skipped>,
    /// Interruption points drawn: pairs linked to a partner on the folios.
    pub ip_pairs: usize,
    /// Interruption points drawn without a partner on the folios.
    pub ip_unpaired: usize,
    /// Terminal strips (including 1X1, which has no strip segment) drawn.
    pub strips: Vec<String>,
}

impl Fixture {
    /// The number of conductors drawn on all folios.
    pub fn conductors(&self) -> usize {
        self.folios.iter().map(|f| f.conductors).sum()
    }
}

struct Builder {
    g: Golden,
    counter: u64,
    defs: Vec<Def>,
    /// Definition signature → index.
    def_of: BTreeMap<String, usize>,
    /// Base name → number of variants so far.
    variants: BTreeMap<String, usize>,
    diagrams: Vec<Diagram>,
    /// IP function key → (diagram, element).
    ips: BTreeMap<String, (usize, usize)>,
    skipped: Vec<Skipped>,
    strips: BTreeSet<String>,
}

impl Builder {
    fn next(&mut self) -> u64 {
        let n = self.counter;
        self.counter += 1;
        n
    }

    /// The definition with this signature, created by `make` on first use.
    fn def(
        &mut self,
        base: &str,
        signature: String,
        link_type: &'static str,
        kind_type: Option<&'static str>,
        terminals: Vec<(i64, i64, char, String)>,
    ) -> usize {
        if let Some(i) = self.def_of.get(&signature) {
            return *i;
        }
        let k = self.variants.entry(base.to_owned()).or_default();
        *k += 1;
        let name = if *k == 1 {
            base.to_owned()
        } else {
            format!("{base}-{k}")
        };
        let uuid = self.next();
        let terminals = terminals
            .into_iter()
            .map(|(x, y, orient, name)| DefTerm {
                x,
                y,
                orient,
                name,
                uuid: self.next(),
            })
            .collect();
        self.defs.push(Def {
            name,
            link_type,
            kind_type,
            terminals,
            uuid,
        });
        self.def_of.insert(signature, self.defs.len() - 1);
        self.defs.len() - 1
    }

    /// A new element of definition `def` at `(x, y)`; its terminals get the diagram's next integer IDs.
    fn elem(&mut self, d: usize, def: usize, x: i64, y: i64) -> usize {
        let uuid = self.next();
        let n = self.defs[def].terminals.len();
        let diagram = &mut self.diagrams[d];
        let first: u32 = diagram
            .elems
            .iter()
            .map(|e| u32::try_from(e.term_ids.len()).unwrap())
            .sum::<u32>()
            + 1;
        let index = diagram.elems.len();
        diagram.elems.push(Elem {
            uuid,
            upper: index.is_multiple_of(2),
            def,
            x,
            y,
            orientation: 0,
            label: None,
            function: None,
            texts: Vec::new(),
            term_ids: (first..first + u32::try_from(n).unwrap()).collect(),
            links: Vec::new(),
        });
        index
    }

    fn user_text(&mut self, d: usize, e: usize, text: &str) {
        let uuid = self.next();
        self.diagrams[d].elems[e].texts.push((uuid, text.to_owned()));
    }

    /// The definition of a symbol drawn for a function with CPs `cps`: the symbol's CPs (or, for the black
    /// box, one terminal per function CP on its top and bottom edge) as terminals, and the CP map.
    fn symbol_def(
        &mut self,
        symbol: &str,
        cps: &[Cp],
        link_type: &'static str,
        kind_type: Option<&'static str>,
    ) -> (usize, BTreeMap<u16, usize>) {
        let sym = self
            .g
            .symbols
            .get(symbol)
            .unwrap_or_else(|| panic!("no symbol `{symbol}`"))
            .clone()
            .unwrap_or_else(|e| panic!("{e}"));
        let designation = |idx: u16| {
            cps.iter()
                .find(|c| c.idx == idx)
                .map_or_else(String::new, |c| c.designation.clone())
        };
        let (terms, map): (Terms, BTreeMap<u16, usize>) = if sym.is_empty() {
            // The black box: the box is 100 px wide and 60 px high around its anchor (left, middle);
            // the first half of the CPs sits on the top edge, the rest on the bottom edge.
            let top = cps.len().div_ceil(2);
            let mut terms = Vec::new();
            let mut map = BTreeMap::new();
            for (j, c) in cps.iter().enumerate() {
                let (row, k) = if j < top { (-30, j) } else { (30, j - top) };
                let x = 10 + 20 * i64::try_from(k).unwrap();
                terms.push((x, row, if row < 0 { 'n' } else { 's' }, c.designation.clone()));
                map.insert(c.idx, j);
            }
            (terms, map)
        } else {
            let mut seen = BTreeSet::new();
            for c in &sym {
                assert!(seen.insert(c.pos), "symbol {symbol}: two CPs on {:?}", c.pos);
            }
            (
                sym.iter()
                    .map(|c| (c.pos.0, c.pos.1, c.dir, designation(c.fn_cp)))
                    .collect(),
                sym.iter().enumerate().map(|(j, c)| (c.fn_cp, j)).collect(),
            )
        };
        let names: Vec<&str> = terms.iter().map(|t| t.3.as_str()).collect();
        let signature = format!("{symbol}|{link_type}|{kind_type:?}|{}", names.join(";"));
        (self.def(symbol, signature, link_type, kind_type, terms), map)
    }

    fn build(mut self) -> Fixture {
        for (i, folio) in FOLIOS.iter().enumerate() {
            let page = self
                .g
                .pages
                .iter()
                .find(|p| col(p, "key") == *folio)
                .unwrap_or_else(|| panic!("no page {folio}"));
            self.diagrams.push(Diagram {
                folio: (*folio).to_owned(),
                title: col(page, "title").to_owned(),
                ..Diagram::default()
            });
            self.draw_page(i, folio);
        }
        let (ip_pairs, ip_unpaired) = self.link_ips();
        self.skip_bridges();
        let folios: Vec<FolioStats> = self.diagrams.iter().map(|d| self.stats(d)).collect();
        let text = self.write();
        Fixture {
            text,
            folios,
            skipped: self.skipped,
            ip_pairs,
            ip_unpaired,
            strips: self.strips.into_iter().collect(),
        }
    }

    fn stats(&self, d: &Diagram) -> FolioStats {
        let kind = |e: &Elem| self.defs[e.def].link_type;
        let count = |t: &str| d.elems.iter().filter(|e| kind(e) == t).count();
        FolioStats {
            folio: d.folio.clone(),
            elements: d.elems.len(),
            devices: count("simple")
                - d.elems
                    .iter()
                    .filter(|e| self.defs[e.def].name == "junction")
                    .count(),
            terminals: count("terminal"),
            reports: count("next_report") + count("previous_report"),
            junctions: d
                .elems
                .iter()
                .filter(|e| self.defs[e.def].name == "junction")
                .count(),
            conductors: d.conductors.len(),
            integer_ends: d.conductors.iter().filter(|c| c.integer).count(),
        }
    }

    fn draw_page(&mut self, d: usize, folio: &str) {
        let mut placed: BTreeMap<String, Placed> = BTreeMap::new();
        let reps: Vec<Row> = self
            .g
            .reps
            .iter()
            .filter(|r| col(r, "page") == folio)
            .cloned()
            .collect();
        // Strips whose first terminal element already carries the strip label, by (strip, column).
        let mut labelled: BTreeSet<String> = BTreeSet::new();
        for rep in &reps {
            let functions: Vec<&str> = col(rep, "functions").split(';').collect();
            assert_eq!(
                functions.len(),
                1,
                "representation `{}` draws several functions",
                col(rep, "key")
            );
            let fkey = functions[0];
            let f = self.g.function(fkey).clone();
            let (cx, cy) = cell_centre(col(rep, "cell"));
            let (dx, dy) = offset(rep);
            let (x, y) = (cx + dx, cy + dy);
            let symbol = col(rep, "symbol");
            if col(&f, "kind") == "ip" {
                self.draw_ip(d, &f, symbol, (x, y), &mut placed);
                continue;
            }
            let device = col(&f, "device").to_owned();
            assert!(
                !self
                    .g
                    .functions
                    .iter()
                    .any(|o| col(o, "device") == device && col(o, "role") == "aux"),
                "device {device} has auxiliary functions: master and slave links are not drawn by the fixture"
            );
            if self.g.is_strip(&device) {
                self.draw_strip_column(d, &device, rep, (x, y), &mut labelled, &mut placed);
            } else if self.g.is_terminal(&device) {
                self.draw_symbol_terminal(d, &device, fkey, symbol, (x, y), &mut labelled, &mut placed);
            } else {
                let cps = self.g.function_cps(fkey);
                let (def, map) = self.symbol_def(symbol, &cps, "simple", None);
                let e = self.elem(d, def, x, y);
                let row = self.g.device(&device);
                let text = col(row, "function_text").to_owned();
                let el = &mut self.diagrams[d].elems[e];
                el.label = Some(device.clone());
                el.function = Some(text).filter(|t| !t.is_empty());
                placed.insert(fkey.to_owned(), Placed { elem: e, cps: map });
            }
        }
        self.draw_junctions(d, folio, &mut placed);
        self.draw_conductors(d, folio, &placed);
    }

    fn draw_ip(
        &mut self,
        d: usize,
        f: &Row,
        symbol: &str,
        at: (i64, i64),
        placed: &mut BTreeMap<String, Placed>,
    ) {
        let link = match col(f, "ip_direction") {
            "source" => "next_report",
            "target" => "previous_report",
            other => panic!(
                "interruption point `{}`: direction `{other}` is not drawn",
                col(f, "key")
            ),
        };
        let cps = self.g.function_cps(col(f, "key"));
        let (def, map) = self.symbol_def(symbol, &cps, link, None);
        let e = self.elem(d, def, at.0, at.1);
        self.user_text(d, e, col(f, "ip_name"));
        placed.insert(col(f, "key").to_owned(), Placed { elem: e, cps: map });
        self.ips.insert(col(f, "key").to_owned(), (d, e));
    }

    /// A terminal of a strip drawn by its own symbol (1X1: no strip segment): a terminal element at the
    /// representation's position. The first terminal of the strip carries the strip tag.
    #[allow(clippy::too_many_arguments)]
    fn draw_symbol_terminal(
        &mut self,
        d: usize,
        terminal: &str,
        fkey: &str,
        symbol: &str,
        at: (i64, i64),
        labelled: &mut BTreeSet<String>,
        placed: &mut BTreeMap<String, Placed>,
    ) {
        let trow = self
            .g
            .terminals
            .iter()
            .find(|r| col(r, "key") == terminal)
            .unwrap()
            .clone();
        let strip = col(&trow, "strip").to_owned();
        let cps = self.g.function_cps(fkey);
        let pe = col(self.g.function(fkey), "fndef") == "ew.fn.terminal-pe";
        let (def, map) = self.symbol_def(
            symbol,
            &cps,
            "terminal",
            Some(if pe { "ground" } else { "generic" }),
        );
        let e = self.elem(d, def, at.0, at.1);
        self.strips.insert(strip.clone());
        if labelled.insert(format!("{strip}|single")) {
            self.diagrams[d].elems[e].label = Some(strip);
        }
        self.user_text(d, e, col(&trow, "designation"));
        placed.insert(fkey.to_owned(), Placed { elem: e, cps: map });
    }

    /// The terminals of one strip segment as a column of `terminal` elements.
    fn draw_strip_column(
        &mut self,
        d: usize,
        strip: &str,
        rep: &Row,
        at: (i64, i64),
        labelled: &mut BTreeSet<String>,
        placed: &mut BTreeMap<String, Placed>,
    ) {
        // The segment representations of the strip on this page, in table order; this is segment j of m.
        let segments: Vec<&Row> = self
            .g
            .reps
            .iter()
            .filter(|r| col(r, "page") == col(rep, "page") && col(r, "functions") == col(rep, "functions"))
            .collect();
        let j = segments
            .iter()
            .position(|r| col(r, "key") == col(rep, "key"))
            .unwrap();
        let m = segments.len();
        let mut terminals: Vec<Row> = self
            .g
            .terminals
            .iter()
            .filter(|r| col(r, "strip") == strip)
            .cloned()
            .collect();
        terminals.sort_by_key(|r| col(r, "seq").parse::<u32>().unwrap());
        let n = terminals.len();
        let chunk = &terminals[j * n / m..(j + 1) * n / m];
        self.strips.insert(strip.to_owned());
        for (k, t) in chunk.iter().enumerate() {
            let tkey = col(t, "key");
            let pe = col(self.g.function(tkey), "fndef") == "ew.fn.terminal-pe";
            let (base, kind) = if pe {
                ("strip-terminal-pe", "ground")
            } else {
                ("strip-terminal", "generic")
            };
            let def = self.def(
                base,
                base.to_owned(),
                "terminal",
                Some(kind),
                vec![(0, -10, 'n', "1".to_owned()), (0, 10, 's', "2".to_owned())],
            );
            let y = at.1 + STRIP_PITCH * i64::try_from(k).unwrap();
            let e = self.elem(d, def, at.0, y);
            self.diagrams[d].elems[e].orientation = 1;
            if k == 0 && labelled.insert(format!("{strip}|{}", col(rep, "key"))) {
                self.diagrams[d].elems[e].label = Some(strip.to_owned());
            }
            self.user_text(d, e, col(t, "designation"));
            // Golden CP 0 (clamp a, side 1) is the top terminal, CP 1 (clamp b, side 2) the bottom one.
            placed.insert(
                tkey.to_owned(),
                Placed {
                    elem: e,
                    cps: BTreeMap::from([(0, 0), (1, 1)]),
                },
            );
        }
    }

    fn draw_junctions(&mut self, d: usize, folio: &str, placed: &mut BTreeMap<String, Placed>) {
        let junctions: Vec<Row> = self
            .g
            .junctions
            .iter()
            .filter(|r| col(r, "page") == folio)
            .cloned()
            .collect();
        for j in junctions {
            let def = self.def(
                "junction",
                "junction".to_owned(),
                "simple",
                None,
                vec![(0, 0, 'n', String::new())],
            );
            let (cx, cy) = cell_centre(col(&j, "cell"));
            let (dx, dy) = offset(&j);
            let e = self.elem(d, def, cx + dx, cy + dy);
            placed.insert(
                format!("@{}", col(&j, "key")),
                Placed {
                    elem: e,
                    cps: BTreeMap::from([(0, 0)]),
                },
            );
        }
    }

    fn end(&self, placed: &BTreeMap<String, Placed>, text: &str) -> End {
        let (key, idx) = match self.g.resolve(text) {
            Ep::Cp(f, i) => (f, i),
            Ep::Junction(j) => (format!("@{j}"), 0),
        };
        let p = placed
            .get(&key)
            .unwrap_or_else(|| panic!("endpoint `{text}`: `{key}` is not drawn on this page"));
        let term = *p.cps.get(&idx).unwrap_or_else(|| {
            panic!("endpoint `{text}`: CP {idx} of `{key}` has no terminal in its element")
        });
        End { elem: p.elem, term }
    }

    fn draw_conductors(&mut self, d: usize, folio: &str, placed: &BTreeMap<String, Placed>) {
        let conns: Vec<Row> = self
            .g
            .connections
            .iter()
            .filter(|r| col(r, "page") == folio)
            .cloned()
            .collect();
        let mut written = 0usize;
        for c in &conns {
            let kind = col(c, "kind");
            if !(kind.is_empty() || kind == "wire") {
                self.skipped.push(Skipped {
                    key: col(c, "key").to_owned(),
                    kind: kind.to_owned(),
                    page: folio.to_owned(),
                });
                continue;
            }
            let a = self.end(placed, col(c, "a"));
            let b = self.end(placed, col(c, "b"));
            self.diagrams[d].conductors.push(Conductor {
                a,
                b,
                integer: written % 3 == 2,
            });
            written += 1;
        }
    }

    /// Links the interruption points of one name that appear as a source and as a target on the folios.
    fn link_ips(&mut self) -> (usize, usize) {
        let mut by_name: BTreeMap<String, (Vec<String>, Vec<String>)> = BTreeMap::new();
        for key in self.ips.keys() {
            let f = self.g.function(key);
            let slot = by_name.entry(col(f, "ip_name").to_owned()).or_default();
            match col(f, "ip_direction") {
                "source" => slot.0.push(key.clone()),
                _ => slot.1.push(key.clone()),
            }
        }
        let (mut pairs, mut unpaired) = (0, 0);
        for (sources, targets) in by_name.values() {
            if let ([s], [t]) = (sources.as_slice(), targets.as_slice()) {
                let (sd, se) = self.ips[s];
                let (td, te) = self.ips[t];
                let su = self.diagrams[sd].elems[se].uuid;
                let tu = self.diagrams[td].elems[te].uuid;
                self.diagrams[sd].elems[se].links.push(tu);
                self.diagrams[td].elems[te].links.push(su);
                pairs += 1;
            } else {
                unpaired += sources.len() + targets.len();
            }
        }
        (pairs, unpaired)
    }

    /// The bridges of the strips drawn on the folios are not drawn: QET 0.100+ stores bridges in
    /// `<terminal_strips>`, which the importer does not read yet (doc 14 §4.11).
    fn skip_bridges(&mut self) {
        for c in &self.g.connections {
            if col(c, "kind") != "bridge" {
                continue;
            }
            let strip = col(c, "a").split_once(':').map_or("", |s| s.0);
            if self.strips.contains(strip) {
                self.skipped.push(Skipped {
                    key: col(c, "key").to_owned(),
                    kind: "bridge".to_owned(),
                    page: String::new(),
                });
            }
        }
    }

    // -----------------------------------------------------------------------------------------------------
    // XML

    fn write(&self) -> String {
        let mut x = String::new();
        let w = &mut x;
        writeln!(w, r#"<?xml version="1.0" encoding="UTF-8"?>"#).unwrap();
        writeln!(
            w,
            r#"<project version="0.100" title="Synthetic J-E-2416 101-103">"#
        )
        .unwrap();
        writeln!(w, "  <properties/>").unwrap();
        writeln!(
            w,
            r#"  <newdiagrams><border cols="17" colsize="60" rows="8" rowsize="80" displaycols="true" displayrows="true"/><report label="%F-%l%c"/></newdiagrams>"#
        )
        .unwrap();
        for (i, d) in self.diagrams.iter().enumerate() {
            self.write_diagram(w, i + 1, d);
        }
        self.write_collection(w);
        writeln!(w, "</project>").unwrap();
        x
    }

    fn write_diagram(&self, w: &mut String, order: usize, d: &Diagram) {
        writeln!(
            w,
            r#"  <diagram order="{order}" folio="{}" title="{}" cols="17" colsize="60" rows="8" rowsize="80" height="660" author="" date="" indexrev="" version="0.100.0">"#,
            esc(&d.folio),
            esc(&d.title)
        )
        .unwrap();
        writeln!(w, "    <elements>").unwrap();
        for e in &d.elems {
            let def = &self.defs[e.def];
            writeln!(
                w,
                r#"      <element type="embed://import/synth/{}.elmt" uuid="{}" x="{}" y="{}" z="10" orientation="{}" prefix="" freezeLabel="false">"#,
                esc(&def.name),
                uuid_text(e.uuid, e.upper),
                e.x,
                e.y,
                e.orientation
            )
            .unwrap();
            writeln!(w, "        <terminals>").unwrap();
            for (t, id) in def.terminals.iter().zip(&e.term_ids) {
                writeln!(
                    w,
                    r#"          <terminal x="{}" y="{}" orientation="{}" id="{id}"/>"#,
                    t.x, t.y, t.orient
                )
                .unwrap();
            }
            writeln!(w, "        </terminals>").unwrap();
            writeln!(w, "        <inputs/>").unwrap();
            let mut infos = Vec::new();
            if let Some(l) = &e.label {
                infos.push(("label", l));
            }
            if let Some(f) = &e.function {
                infos.push(("function", f));
            }
            if infos.is_empty() {
                writeln!(w, "        <elementInformations/>").unwrap();
            } else {
                writeln!(w, "        <elementInformations>").unwrap();
                for (n, v) in infos {
                    writeln!(
                        w,
                        r#"          <elementInformation name="{n}" show="1">{}</elementInformation>"#,
                        esc(v)
                    )
                    .unwrap();
                }
                writeln!(w, "        </elementInformations>").unwrap();
            }
            if e.texts.is_empty() {
                writeln!(w, "        <dynamic_texts/>").unwrap();
            } else {
                writeln!(w, "        <dynamic_texts>").unwrap();
                for (u, t) in &e.texts {
                    writeln!(
                        w,
                        r#"          <dynamic_elmt_text text_from="UserText" uuid="{}" x="0" y="0" rotation="0" font="Sans Serif,9,-1,5,50,0,0,0,0,0" frame="false" text_width="-1" keep_visual_rotation="true" Halignment="AlignLeft" Valignment="AlignTop"><text>{}</text></dynamic_elmt_text>"#,
                        uuid_text(*u, false),
                        esc(t)
                    )
                    .unwrap();
                }
                writeln!(w, "        </dynamic_texts>").unwrap();
            }
            writeln!(w, "        <texts_groups/>").unwrap();
            if e.links.is_empty() {
                writeln!(w, "        <links_uuids/>").unwrap();
            } else {
                writeln!(w, "        <links_uuids>").unwrap();
                for l in &e.links {
                    writeln!(w, r#"          <link_uuid uuid="{}"/>"#, uuid_text(*l, false)).unwrap();
                }
                writeln!(w, "        </links_uuids>").unwrap();
            }
            writeln!(w, "      </element>").unwrap();
        }
        writeln!(w, "    </elements>").unwrap();
        writeln!(w, "    <conductors>").unwrap();
        for c in &d.conductors {
            let (ea, eb) = (&d.elems[c.a.elem], &d.elems[c.b.elem]);
            let ends = if c.integer {
                format!(
                    r#"terminal1="{}" terminal2="{}""#,
                    ea.term_ids[c.a.term], eb.term_ids[c.b.term]
                )
            } else {
                format!(
                    r#"element1="{}" terminal1="{}" element2="{}" terminal2="{}""#,
                    uuid_text(ea.uuid, false),
                    uuid_text(self.defs[ea.def].terminals[c.a.term].uuid, false),
                    uuid_text(eb.uuid, false),
                    uuid_text(self.defs[eb.def].terminals[c.b.term].uuid, false),
                )
            };
            writeln!(
                w,
                r#"      <conductor {ends} num="" function="" tension_protocol="" conductor_color="" conductor_section="" type="multi" x="0" y="0" freezeLabel="false"/>"#
            )
            .unwrap();
        }
        writeln!(w, "    </conductors>").unwrap();
        writeln!(w, "  </diagram>").unwrap();
    }

    fn write_collection(&self, w: &mut String) {
        writeln!(w, "  <collection>").unwrap();
        writeln!(
            w,
            r#"    <category name="import"><names><name lang="en">Imported elements</name></names><category name="synth"><names><name lang="en">Synthetic</name></names>"#
        )
        .unwrap();
        for def in &self.defs {
            let (mut x0, mut y0, mut x1, mut y1) = (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
            for t in &def.terminals {
                x0 = x0.min(t.x);
                y0 = y0.min(t.y);
                x1 = x1.max(t.x);
                y1 = y1.max(t.y);
            }
            let (x0, y0, x1, y1) = (x0 - 10, y0 - 10, x1 + 10, y1 + 10);
            writeln!(w, r#"      <element name="{}.elmt">"#, esc(&def.name)).unwrap();
            writeln!(
                w,
                r#"        <definition type="element" version="0.100.0" link_type="{}" width="{}" height="{}" hotspot_x="{}" hotspot_y="{}">"#,
                def.link_type,
                x1 - x0,
                y1 - y0,
                -x0,
                -y0
            )
            .unwrap();
            writeln!(w, r#"          <uuid uuid="{}"/>"#, uuid_text(def.uuid, false)).unwrap();
            writeln!(
                w,
                r#"          <names><name lang="en">{}</name></names>"#,
                esc(&def.name)
            )
            .unwrap();
            if let Some(k) = def.kind_type {
                writeln!(
                    w,
                    r#"          <kindInformations><kindInformation name="type" show="1">{k}</kindInformation></kindInformations>"#
                )
                .unwrap();
            }
            writeln!(w, "          <informations/>").unwrap();
            writeln!(w, "          <description>").unwrap();
            writeln!(
                w,
                r#"            <rect x="{x0}" y="{y0}" width="{}" height="{}" antialias="false" style="line-style:normal;line-weight:normal;filling:none;color:black"/>"#,
                x1 - x0,
                y1 - y0
            )
            .unwrap();
            for t in &def.terminals {
                writeln!(
                    w,
                    r#"            <terminal x="{}" y="{}" orientation="{}" name="{}" type="Generic" uuid="{}"/>"#,
                    t.x,
                    t.y,
                    t.orient,
                    esc(&t.name),
                    uuid_text(t.uuid, false)
                )
                .unwrap();
            }
            writeln!(w, "          </description>").unwrap();
            writeln!(w, "        </definition>").unwrap();
            writeln!(w, "      </element>").unwrap();
        }
        writeln!(w, "    </category></category>").unwrap();
        writeln!(w, "  </collection>").unwrap();
    }
}

/// Builds the fixture. Deterministic: two builds give the same text.
pub fn build() -> Fixture {
    Builder {
        g: Golden::load(),
        counter: UUID_BASE,
        defs: Vec::new(),
        def_of: BTreeMap::new(),
        variants: BTreeMap::new(),
        diagrams: Vec::new(),
        ips: BTreeMap::new(),
        skipped: Vec::new(),
        strips: BTreeSet::new(),
    }
    .build()
}
