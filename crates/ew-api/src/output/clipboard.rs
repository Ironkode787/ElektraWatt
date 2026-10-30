// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The clipboard below the shell (WP-S2.05 part 3, spec §8.1 to §8.3, §8.6; D12): [`Session::copy_fragment`] writes a
//! selection as a project fragment, [`parse_fragment`] reads clipboard text back into the parameters of `edit.paste`,
//! and [`Session::tag_list`] gives the "Copy tag list" text.
//!
//! **The format** (`#ew-fragment v1`, the project-fragment format of architecture §5.5). UTF-8 text, LF line ends, a
//! final LF: line 1 `#ew-fragment v1`; line 2 the header `{"$fragment": {"format", "project", "grid": [num, den],
//! "page_kind", "anchor": [x, y], "ghost": [[x0, y0, x1, y1], …]}}`; then one canonical record line per record, one
//! `{"$lib": <libdoc>}` line per library copy and one `{"$node": {"id", "aspect", "path", "aliases"}}` line per
//! structure node. The record kinds a fragment carries are devices, functions, representations, connections,
//! junctions, potential definitions, page items and structure boxes. Another application sees JSON Lines after one
//! comment line (FR-UI-040).
//! Within v1 the `$fragment` header is open (a member this version does not know is ignored, room for later minor
//! additions such as WP-S1.19's macros), while a `$node` line's members are closed (`id`, `aspect`, `path`,
//! `aliases`; any other is refused).
//!
//! **Order** (§9 item 3; FT-439's rule that machine text is not ordered by UUID): records by (kind rank, position,
//! ID): a representation's insertion point (x, then y), a junction's point, a function's and a device's first copied
//! representation, a connection's and a potential definition's end, a page item's anchor point (a graphic's first
//! point, a text's or marker's `at`), a structure box's first outline vertex; library copies by (kind, part number or
//! key, ID); nodes by (aspect, path, ID). `edit.paste` mints the new IDs in (kind rank, fragment order), so the online
//! numbering, which numbers new devices in ID order, numbers the copies by position.
//!
//! **What a copy drops** (§8.2 item 6): an import lineage (`source_ref`: a pasted record is new, not imported), a
//! PLC station aspect (its configuration project is not copied), a cable core (the copy is a plain wire), a bus
//! line's cable, an assembly cover outside the copy and a junction's sequence entries outside the copy. A page item
//! of a kind this version does not know, and an image (its asset is not in the fragment), is dropped as `page_item`, and a wire marker that keeps none of its copied
//! connections as `wire_marker`. Bonds, cables and conductors are listed in [`CopiedFragment::skipped`]. The
//! fragment's records keep their source IDs; it is data, not machine text, so the shell's ID alias does not apply to
//! it.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ew_commands::commands::{PASTE_KINDS, PASTE_MAX_RECORDS};
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::{PageOrder, PageRef, Q};
use ew_model::EndpointKey;
use ew_schema::{
    Connection, ConnectionKind, Device, Endpoint, Function, GraphicShape, Junction, Kind, LibDocument, Page,
    PageItem, PageItemKind, PotentialDef, Record, Representation, Slot, StructureAssignment, StructureBox,
    StructureNode,
};
use serde_json::{Value, json};

use crate::Session;

/// The first line of every fragment.
pub const FRAGMENT_HEADER: &str = "#ew-fragment v1";

/// The largest fragment text [`parse_fragment`] reads, in bytes.
pub const FRAGMENT_MAX_BYTES: usize = 4 << 20;

/// The longest line of a fragment, in bytes.
pub const FRAGMENT_MAX_LINE: usize = 1 << 20;

/// The deepest nesting of arrays and objects in a fragment line.
pub const FRAGMENT_MAX_DEPTH: usize = 64;

/// A selection written as a fragment (§8.2).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CopiedFragment {
    /// The fragment text.
    pub text: String,
    /// How many records it holds.
    pub records: usize,
    /// How many library copies it holds.
    pub copies: usize,
    /// How many selected connections were dropped because an end lies outside the copy.
    pub dropped_connections: usize,
    /// Number of selected page items omitted from the fragment or copied with a shorter wire-marker connection list.
    pub incomplete_page_items: usize,
    /// What was reached but not copied, by kind (`bond`, `cable`, `conductor`, `page_item`, `plc_station`,
    /// `wire_marker`), in name order.
    pub skipped: Vec<(String, usize)>,
}

/// Why a selection is not copied.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CopyError {
    /// Nothing copyable is selected: no representation, page item or structure box.
    Empty,
    /// The selection spans several pages.
    NotOnePage,
    /// The selection is generated content (the IDs name no stored record).
    Generated,
}

impl fmt::Display for CopyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            CopyError::Empty => "nothing copyable is selected",
            CopyError::NotOnePage => "the selection spans several pages; copy from one page",
            CopyError::Generated => "generated content is not copied",
        })
    }
}

/// Clipboard text read as a fragment (§8.3).
#[derive(Clone, PartialEq, Debug)]
pub struct ParsedFragment {
    /// The `fragment` member of `edit.paste`: `{"format", "project", "grid", "page_kind", "anchor", "records",
    /// "copies", "nodes"}`, records, copies and nodes as read.
    pub params: Value,
    /// How many records it holds.
    pub records: usize,
    /// The source page's connection grid `(num, den)`.
    pub grid: (i64, i64),
    /// The ghost rectangles relative to the anchor, `[x0, y0, x1, y1]` in nm.
    pub ghost: Vec<[i64; 4]>,
}

/// Why clipboard text is not pasted (§8.3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FragmentError {
    /// The text does not start with `#ew-fragment v1`.
    NotAFragment,
    /// A newer ElektraWatt wrote it: a later fragment version, a later format or a later record version.
    Newer {
        /// The version that is newer (`v2`, a format such as `0.2`, a record's `$v`).
        version: String,
    },
    /// The text exceeds a limit.
    TooLarge(String),
    /// A line is not what the format allows.
    Invalid {
        /// The 1-based line.
        line: usize,
        /// What is wrong.
        message: String,
    },
}

impl fmt::Display for FragmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FragmentError::NotAFragment => f.write_str("the clipboard holds no ElektraWatt fragment"),
            FragmentError::Newer { version } => write!(
                f,
                "copied from a newer ElektraWatt, which writes a newer format ({version})"
            ),
            FragmentError::TooLarge(what) => write!(f, "the fragment is too large: {what}"),
            FragmentError::Invalid { line, message } => write!(f, "fragment line {line}: {message}"),
        }
    }
}

impl std::error::Error for FragmentError {}

/// The greatest nesting of arrays and objects in `line` (strings skipped).
fn depth(line: &str) -> usize {
    let (mut d, mut max, mut in_str, mut escaped) = (0usize, 0usize, false, false);
    for b in line.bytes() {
        if in_str {
            match (escaped, b) {
                (true, _) => escaped = false,
                (false, b'\\') => escaped = true,
                (false, b'"') => in_str = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'[' | b'{' => {
                d += 1;
                max = max.max(d);
            }
            b']' | b'}' => d = d.saturating_sub(1),
            _ => {}
        }
    }
    max
}

/// `"0.1"` as `(0, 1)`.
fn format_version(s: &str) -> Option<(u32, u32)> {
    let (a, b) = s.split_once('.')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

/// An integer pair `[a, b]`.
fn pair(v: &Value) -> Option<(i64, i64)> {
    match v.as_array()?.as_slice() {
        [a, b] => Some((a.as_i64()?, b.as_i64()?)),
        _ => None,
    }
}

/// Reads clipboard text as a fragment (§8.3). The text is untrusted: at most 4 MiB, 10,000 record lines, 1 MiB per
/// line and a nesting of 64; every line strict JSON (repeated member names refused). Within v1 a header member this
/// version does not know is ignored; another `format` or a record of another schema version is refused (newer as
/// [`FragmentError::Newer`], older as [`FragmentError::Invalid`]). A record must be of a pasted kind and read as a
/// record; a library copy as a library document; IDs are distinct. Line ends may be CRLF (a clipboard may convert
/// them); empty lines are skipped. A leading byte-order mark is skipped. No semantic check: that is `edit.paste`'s.
pub fn parse_fragment(text: &str) -> Result<ParsedFragment, FragmentError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l));
    let first = lines.next().unwrap_or_default();
    if first != FRAGMENT_HEADER {
        if let Some(v) = first.strip_prefix("#ew-fragment v")
            && v.parse::<u32>().is_ok_and(|n| n > 1)
        {
            return Err(FragmentError::Newer {
                version: format!("v{v}"),
            });
        }
        return Err(FragmentError::NotAFragment);
    }
    if text.len() > FRAGMENT_MAX_BYTES {
        return Err(FragmentError::TooLarge(format!(
            "{} bytes, at most {FRAGMENT_MAX_BYTES}",
            text.len()
        )));
    }
    let invalid = |line: usize, message: &str| FragmentError::Invalid {
        line,
        message: message.to_owned(),
    };
    let mut header: Option<Value> = None;
    let mut records: Vec<Value> = Vec::new();
    let mut copies: Vec<Value> = Vec::new();
    let mut nodes: Vec<Value> = Vec::new();
    let mut ids: BTreeSet<Uuid> = BTreeSet::new();
    for (n, line) in lines.enumerate() {
        let line_no = n + 2;
        if line.is_empty() {
            continue;
        }
        if line.len() > FRAGMENT_MAX_LINE {
            return Err(FragmentError::TooLarge(format!(
                "line {line_no} has {} bytes, at most {FRAGMENT_MAX_LINE}",
                line.len()
            )));
        }
        if depth(line) > FRAGMENT_MAX_DEPTH {
            return Err(FragmentError::TooLarge(format!(
                "line {line_no} nests deeper than {FRAGMENT_MAX_DEPTH}"
            )));
        }
        let value = ew_schema::parse_value(line).map_err(|e| invalid(line_no, &e.to_string()))?;
        let Some(obj) = value.as_object() else {
            return Err(invalid(line_no, "a line of a fragment is a JSON object"));
        };
        if header.is_none() {
            let Some(h) = obj.get("$fragment").filter(|_| obj.len() == 1) else {
                return Err(invalid(line_no, "line 2 is the `$fragment` header"));
            };
            header = Some(read_header(h, line_no)?);
            continue;
        }
        if obj.len() == 1 && obj.contains_key("$lib") {
            let doc = &obj["$lib"];
            LibDocument::from_value(doc.clone()).map_err(|e| invalid(line_no, &e.to_string()))?;
            copies.push(doc.clone());
        } else if obj.len() == 1 && obj.contains_key("$node") {
            nodes.push(read_node(&obj["$node"], line_no)?);
        } else if let Some(k) = obj.get("$k").and_then(Value::as_str) {
            let Some(kind) = Kind::from_name(k).filter(|k| PASTE_KINDS.contains(k)) else {
                return Err(invalid(
                    line_no,
                    &format!(
                        "a record of kind {k} is not pasted (only devices, functions, representations, connections, \
                         junctions, potential definitions, page items and structure boxes)"
                    ),
                ));
            };
            let v = obj.get("$v").and_then(Value::as_u64).unwrap_or(1);
            let ours = u64::from(kind.schema_version());
            match v.cmp(&ours) {
                Ordering::Greater => {
                    return Err(FragmentError::Newer {
                        version: format!("{k} $v {v}"),
                    });
                }
                Ordering::Less => {
                    return Err(invalid(
                        line_no,
                        "copied from an older ElektraWatt format; open and save that project with this version first",
                    ));
                }
                Ordering::Equal => {}
            }
            let record = Record::from_value(value.clone()).map_err(|e| invalid(line_no, &e.to_string()))?;
            if let Some(id) = record.id()
                && !ids.insert(id)
            {
                return Err(invalid(line_no, "the record's ID is listed twice"));
            }
            records.push(value);
            if records.len() > PASTE_MAX_RECORDS {
                return Err(FragmentError::TooLarge(format!(
                    "more than {PASTE_MAX_RECORDS} records"
                )));
            }
        } else if obj.keys().any(|k| k.starts_with('$')) {
            return Err(invalid(line_no, "an unknown `$` line"));
        } else {
            return Err(invalid(line_no, "a line without `$k` is no record"));
        }
    }
    let Some(header) = header else {
        return Err(invalid(2, "the `$fragment` header is missing"));
    };
    let grid = pair(&header["grid"]).unwrap_or_default();
    let ghost = header["ghost"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| match r.as_array()?.as_slice() {
            [a, b, c, d] => Some([a.as_i64()?, b.as_i64()?, c.as_i64()?, d.as_i64()?]),
            _ => None,
        })
        .collect();
    let count = records.len();
    let params = json!({
        "format": header["format"],
        "project": header["project"],
        "grid": header["grid"],
        "page_kind": header["page_kind"],
        "anchor": header["anchor"],
        "records": records,
        "copies": copies,
        "nodes": nodes,
    });
    Ok(ParsedFragment {
        params,
        records: count,
        grid,
        ghost,
    })
}

/// The `$fragment` header: its known members checked, the others ignored (§8.3).
fn read_header(h: &Value, line: usize) -> Result<Value, FragmentError> {
    let invalid = |message: String| FragmentError::Invalid { line, message };
    let Some(obj) = h.as_object() else {
        return Err(invalid("the `$fragment` header is an object".into()));
    };
    let member = |name: &str| {
        obj.get(name)
            .ok_or_else(|| invalid(format!("the header has no `{name}`")))
    };
    let format = member("format")?
        .as_str()
        .ok_or_else(|| invalid("`format` is a text".into()))?;
    let theirs = format_version(format).ok_or_else(|| invalid(format!("`format` {format} is no version")))?;
    let ours = format_version(ew_schema::FORMAT).expect("the format version reads");
    match theirs.cmp(&ours) {
        Ordering::Greater => {
            return Err(FragmentError::Newer {
                version: format.to_owned(),
            });
        }
        Ordering::Less => {
            return Err(invalid(
                "copied from an older ElektraWatt format; open and save that project with this version first"
                    .into(),
            ));
        }
        Ordering::Equal => {}
    }
    if member("project")?
        .as_str()
        .and_then(|s| s.parse::<Id<()>>().ok())
        .is_none()
    {
        return Err(invalid("`project` is a UUID".into()));
    }
    if pair(member("grid")?).is_none_or(|(n, d)| n < 1 || d < 1) {
        return Err(invalid("`grid` is [num, den] of positive integers".into()));
    }
    if !member("page_kind")?.is_string() {
        return Err(invalid("`page_kind` is a text".into()));
    }
    if pair(member("anchor")?).is_none() {
        return Err(invalid("`anchor` is [x, y] in nm".into()));
    }
    let rects_ok = member("ghost")?.as_array().is_some_and(|a| {
        a.iter().all(|r| {
            r.as_array()
                .is_some_and(|r| r.len() == 4 && r.iter().all(Value::is_i64))
        })
    });
    if !rects_ok {
        return Err(invalid("`ghost` is a list of [x0, y0, x1, y1]".into()));
    }
    Ok(h.clone())
}

/// A `$node` line's value: `{"id", "aspect", "path", "aliases"}`.
fn read_node(v: &Value, line: usize) -> Result<Value, FragmentError> {
    let invalid = |message: &str| FragmentError::Invalid {
        line,
        message: message.to_owned(),
    };
    let obj = v.as_object().ok_or_else(|| invalid("`$node` is an object"))?;
    if obj
        .keys()
        .any(|k| !matches!(k.as_str(), "id" | "aspect" | "path" | "aliases"))
    {
        return Err(invalid("`$node` has the members id, aspect, path and aliases"));
    }
    let texts = |v: Option<&Value>| {
        v.and_then(Value::as_array)
            .is_some_and(|a| a.iter().all(Value::is_string))
    };
    let id_ok = obj
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(|s| s.parse::<Id<()>>().is_ok());
    let path_ok = texts(obj.get("path")) && obj["path"].as_array().is_some_and(|a| !a.is_empty());
    if !id_ok || !obj.get("aspect").is_some_and(Value::is_string) || !path_ok {
        return Err(invalid("`$node` needs an ID, an aspect and a path"));
    }
    if obj.get("aliases").is_some_and(|a| !texts(Some(a))) {
        return Err(invalid("`aliases` is a list of texts"));
    }
    Ok(v.clone())
}

/// The order of a record line: (kind rank, position, ID).
type RecordKey = (u8, (bool, i64, i64), Uuid);

/// The order of a `$node` line: (aspect, path, ID).
type NodeKey = (String, Vec<String>, Uuid);

/// A position key, `None` sorting last.
type PosKey = Option<(i64, i64)>;

fn key_of(p: PosKey) -> (bool, i64, i64) {
    match p {
        Some((x, y)) => (false, x, y),
        None => (true, 0, 0),
    }
}

/// The library copies a record names, by (kind, copy ID).
fn named_copies(record: &Record, out: &mut BTreeSet<(Kind, Uuid)>) {
    match record {
        Record::Representation(r) => {
            out.insert((Kind::Symbol, r.symbol.as_uuid()));
        }
        Record::Function(f) => {
            out.insert((Kind::Fndef, f.def.as_uuid()));
        }
        Record::Device(d) => {
            for p in &d.parts {
                out.insert((Kind::Part, p.part.copy.as_uuid()));
            }
            if let Some(s) = &d.aspects.strip {
                for a in &s.accessories {
                    out.insert((Kind::Part, a.part.copy.as_uuid()));
                }
            }
            if let Some(a) = &d.aspects.assembly {
                for c in &a.configs {
                    if let Some(p) = &c.part {
                        out.insert((Kind::Part, p.copy.as_uuid()));
                    }
                }
            }
        }
        Record::Connection(c) => {
            if let Some(p) = &c.overrides.wire_part {
                out.insert((Kind::Part, p.copy.as_uuid()));
            }
        }
        Record::PotentialDef(p) => {
            if let Some(w) = &p.props.wire_part {
                out.insert((Kind::Part, w.copy.as_uuid()));
            }
        }
        _ => {}
    }
}

/// The structure nodes a structure assignment names.
fn named_nodes(s: &StructureAssignment) -> Vec<Id<StructureNode>> {
    [
        &s.functional_assignment,
        &s.function,
        &s.installation_site,
        &s.location,
        &s.document_kind,
        &s.user_defined,
        &s.type_,
    ]
    .into_iter()
    .filter_map(|slot| match slot {
        Some(Slot::Node(n)) => Some(*n),
        _ => None,
    })
    .collect()
}

/// The anchor point a page item orders by: a graphic's first point, a text's or marker's `at`.
fn page_item_pos(item: &PageItem) -> PosKey {
    match &item.kind {
        PageItemKind::Graphic(g) => match &g.shape {
            GraphicShape::Line(l) => Some((l.a.x.0, l.a.y.0)),
            GraphicShape::Polyline(p) | GraphicShape::Polygon(p) => p.points.first().map(|p| (p.x.0, p.y.0)),
            GraphicShape::Rect(r) => Some((r.min.x.0, r.min.y.0)),
            GraphicShape::Circle(c) => Some((c.centre.x.0, c.centre.y.0)),
            GraphicShape::Arc(a) => Some((a.centre.x.0, a.centre.y.0)),
            GraphicShape::Unknown(_) => None,
        },
        PageItemKind::FreeText(t) => Some((t.at.x.0, t.at.y.0)),
        PageItemKind::WireMarker(m) => Some((m.at.x.0, m.at.y.0)),
        PageItemKind::Image(i) => Some((i.rect.0.min().x.0, i.rect.0.min().y.0)),
        PageItemKind::Unknown(_) => None,
    }
}

/// The bounds of a structure box's outline, `[x0, y0, x1, y1]`; `None` for an empty outline.
fn outline_bounds(b: &StructureBox) -> Option<[i64; 4]> {
    b.outline.iter().fold(None, |acc, p| {
        Some(match acc {
            None => [p.x.0, p.y.0, p.x.0, p.y.0],
            Some([x0, y0, x1, y1]) => [x0.min(p.x.0), y0.min(p.y.0), x1.max(p.x.0), y1.max(p.y.0)],
        })
    })
}

/// The largest grid point not greater than `v`.
fn floor_to_grid(grid: ew_units::Grid, v: i64) -> i64 {
    let k = grid.snap(ew_units::Nm(v));
    match grid.at(k) {
        Some(p) if p.0 <= v => p.0,
        _ => grid.at(k.saturating_sub(1)).map_or(v, |p| p.0),
    }
}

/// The functions a representation draws.
fn drawn(r: &Representation) -> impl Iterator<Item = Id<Function>> + '_ {
    r.functions
        .iter()
        .copied()
        .chain(r.pin_map.as_slice().iter().map(|p| p.function))
}

impl Session {
    /// The selection `ids` as a fragment (spec §8.2): the representations, connections, junctions, page items and
    /// structure boxes it names on one page, closed over what they need; [`CopyError::Empty`] without a
    /// representation, page item or structure box, [`CopyError::NotOnePage`] across pages, [`CopyError::Generated`]
    /// when no ID names a stored record of these kinds and some names none at all (generated content). Equal
    /// selections on equal snapshots give byte-equal text.
    pub fn copy_fragment(&self, ids: &[Uuid]) -> Result<CopiedFragment, CopyError> {
        let q = self.query();
        let mut reps: BTreeSet<Id<Representation>> = BTreeSet::new();
        let mut sel_conns: BTreeSet<Id<Connection>> = BTreeSet::new();
        let mut junctions: BTreeSet<Id<Junction>> = BTreeSet::new();
        let mut sel_items: BTreeSet<Id<PageItem>> = BTreeSet::new();
        let mut sel_boxes: BTreeSet<Id<StructureBox>> = BTreeSet::new();
        let mut pages: BTreeSet<Id<Page>> = BTreeSet::new();
        let mut unknown = false;
        for u in ids {
            if let Some(r) = q.reps().get(Id::from_uuid(*u)) {
                reps.insert(r.id);
                pages.insert(r.page);
            } else if let Some(c) = q.connections().get(Id::from_uuid(*u)) {
                sel_conns.insert(c.id);
                pages.extend(c.page);
            } else if let Some(j) = q.junctions().get(Id::from_uuid(*u)) {
                junctions.insert(j.id);
                pages.insert(j.page);
            } else if let Some(i) = q.page_items().get(Id::from_uuid(*u)) {
                sel_items.insert(i.id);
                pages.insert(i.page);
            } else if let Some(b) = q.structure_boxes().get(Id::from_uuid(*u)) {
                sel_boxes.insert(b.id);
                pages.insert(b.page);
            } else if !self.names_record(&q, *u) {
                unknown = true;
            }
        }
        if pages.len() > 1 {
            return Err(CopyError::NotOnePage);
        }
        let Some(&page_id) = pages.first() else {
            return Err(if unknown {
                CopyError::Generated
            } else {
                CopyError::Empty
            });
        };
        let Some(page) = q.pages().get(page_id).cloned() else {
            return Err(CopyError::Generated);
        };
        if matches!(
            page.kind,
            ew_schema::PageType::Report
                | ew_schema::PageType::TableOfContents
                | ew_schema::PageType::SectionDivider
        ) {
            return Err(CopyError::Generated);
        }
        // 1. Representations, their functions, the functions' devices and all of their functions; a terminal's strip.
        let mut skipped: BTreeMap<String, usize> = BTreeMap::new();
        let is_cable = |d: Id<Device>| q.devices().get(d).is_some_and(|d| d.aspects.cable.is_some());
        let mut cables: BTreeSet<Id<Device>> = BTreeSet::new();
        reps.retain(|r| {
            let Some(rep) = q.reps().get(*r) else { return false };
            let cable = drawn(rep).find_map(|f| {
                q.functions()
                    .get(f)
                    .and_then(|f| f.device)
                    .filter(|d| is_cable(*d))
            });
            if let Some(c) = cable {
                cables.insert(c);
            }
            cable.is_none()
        });
        if !cables.is_empty() {
            skipped.insert("cable".into(), cables.len());
        }
        if reps.is_empty() && sel_items.is_empty() && sel_boxes.is_empty() {
            return Err(CopyError::Empty);
        }
        let mut functions: BTreeSet<Id<Function>> = BTreeSet::new();
        let mut devices: BTreeSet<Id<Device>> = BTreeSet::new();
        for r in &reps {
            if let Some(rep) = q.reps().get(*r) {
                functions.extend(drawn(rep));
            }
        }
        // The functions the copied representations draw: a copied wire ends at a drawn connection point.
        let drawn_fns = functions.clone();
        for f in functions.clone() {
            if let Some(d) = q.functions().get(f).and_then(|f| f.device) {
                devices.insert(d);
            }
        }
        for d in devices.clone() {
            functions.extend(q.idx().functions_of_device(d));
            if let Some(t) = q.devices().get(d).and_then(|d| d.aspects.terminal.as_ref()) {
                devices.insert(t.strip);
            }
        }
        // 2. and 3. Connections with both ends copied; junctions where two such connections meet, to a fixed point.
        let page_conns: Vec<std::sync::Arc<Connection>> = q
            .idx()
            .connections_by_page(page_id)
            .filter_map(|c| q.connections().get(c).cloned())
            .collect();
        let page_junctions: Vec<Id<Junction>> = q.idx().junctions_by_page(page_id).collect();
        let inside = |e: &Endpoint, js: &BTreeSet<Id<Junction>>| match e {
            Endpoint::Cp(f, _) => drawn_fns.contains(f),
            Endpoint::Junction(j) => js.contains(j),
            Endpoint::Unknown(_) => false,
        };
        loop {
            let mut more = Vec::new();
            for j in &page_junctions {
                if junctions.contains(j) {
                    continue;
                }
                let here = Endpoint::Junction(*j);
                let meeting = page_conns
                    .iter()
                    .filter(|c| {
                        (c.a == here && inside(&c.b, &junctions)) || (c.b == here && inside(&c.a, &junctions))
                    })
                    .count();
                if meeting >= 2 {
                    more.push(*j);
                }
            }
            if more.is_empty() {
                break;
            }
            junctions.extend(more);
        }
        let connections: BTreeSet<Id<Connection>> = page_conns
            .iter()
            .filter(|c| inside(&c.a, &junctions) && inside(&c.b, &junctions))
            .map(|c| c.id)
            .collect();
        let dropped_connections = sel_conns.iter().filter(|c| !connections.contains(c)).count();
        // 4. Selected page items: graphics and free texts as they are, a wire marker only with its copied connections;
        //    an unknown variant and a marker left without a connection are counted, not copied.
        let mut items: Vec<PageItem> = Vec::new();
        let mut unknown_items = 0usize;
        let mut emptied_markers = 0usize;
        let mut incomplete_page_items = 0usize;
        for id in &sel_items {
            let Some(item) = q.page_items().get(*id) else {
                continue;
            };
            match &item.kind {
                PageItemKind::Graphic(_) | PageItemKind::FreeText(_) => items.push((**item).clone()),
                PageItemKind::WireMarker(m) => {
                    let kept: Vec<Id<Connection>> = m
                        .connections
                        .iter()
                        .copied()
                        .filter(|c| connections.contains(c))
                        .collect();
                    if kept.len() != m.connections.len() || kept.is_empty() {
                        incomplete_page_items += 1;
                    }
                    if kept.is_empty() {
                        emptied_markers += 1;
                    } else {
                        let mut item = (**item).clone();
                        if let PageItemKind::WireMarker(marker) = &mut item.kind {
                            marker.connections = kept;
                        }
                        items.push(item);
                    }
                }
                // An image names an asset of this project's store, which a fragment does not carry: it is not copied.
                PageItemKind::Image(_) | PageItemKind::Unknown(_) => {
                    unknown_items += 1;
                    incomplete_page_items += 1;
                }
            }
        }
        if unknown_items > 0 {
            skipped.insert("page_item".into(), unknown_items);
        }
        if emptied_markers > 0 {
            skipped.insert("wire_marker".into(), emptied_markers);
        }
        let boxes: Vec<StructureBox> = sel_boxes
            .iter()
            .filter_map(|b| q.structure_boxes().get(*b).map(|b| (**b).clone()))
            .collect();
        // Potential definitions at copied endpoints; bonds there are not copied.
        let mut potentials: Vec<std::sync::Arc<PotentialDef>> = q
            .potential_defs()
            .iter()
            .filter(|(_, p)| match &p.at {
                Endpoint::Cp(f, _) => functions.contains(f),
                Endpoint::Junction(j) => junctions.contains(j),
                Endpoint::Unknown(_) => false,
            })
            .map(|(_, p)| p.clone())
            .collect();
        potentials.sort_by_key(|p| p.id);
        let mut bonds: BTreeSet<Id<ew_schema::Bond>> = BTreeSet::new();
        let endpoints = functions
            .iter()
            .flat_map(|f| {
                q.idx()
                    .connected_cps_of_function(*f)
                    .into_iter()
                    .map(move |(i, _)| Endpoint::Cp(*f, i))
            })
            .chain(junctions.iter().map(|j| Endpoint::Junction(*j)));
        for e in endpoints {
            if let Some(k) = EndpointKey::of(&e) {
                bonds.extend(q.idx().bonds_at(k));
            }
        }
        if !bonds.is_empty() {
            skipped.insert("bond".into(), bonds.len());
        }
        // The records, with what a copy drops.
        let pos_of_rep = |r: Id<Representation>| q.reps().get(r).map(|r| (r.pos.x.0, r.pos.y.0));
        let pos_of_function = |f: Id<Function>| -> PosKey {
            q.idx()
                .reps_of_function(f)
                .filter(|r| reps.contains(r))
                .filter_map(pos_of_rep)
                .min()
        };
        let pos_of_device = |d: Id<Device>| -> PosKey {
            q.idx()
                .functions_of_device(d)
                .filter_map(pos_of_function)
                .min()
                .or_else(|| {
                    // A strip without its own copied representation: its first copied terminal.
                    q.idx()
                        .terminals_of_strip(d)
                        .filter(|t| devices.contains(t))
                        .flat_map(|t| q.idx().functions_of_device(t).collect::<Vec<_>>())
                        .filter_map(pos_of_function)
                        .min()
                })
        };
        let pos_of_end = |e: &Endpoint| -> PosKey {
            match e {
                Endpoint::Cp(f, _) => pos_of_function(*f),
                Endpoint::Junction(j) => q.junctions().get(*j).map(|j| (j.at.x.0, j.at.y.0)),
                Endpoint::Unknown(_) => None,
            }
        };
        let mut lines: Vec<(RecordKey, Record)> = Vec::new();
        let rank = |k: Kind| k.rank().unwrap_or(u8::MAX);
        let mut conductors = 0usize;
        let mut stations = 0usize;
        for d in &devices {
            let Some(dev) = q.devices().get(*d) else { continue };
            let mut dev = (**dev).clone();
            dev.source_ref = None;
            if dev.aspects.plc_station.take().is_some() {
                stations += 1;
            }
            if let Some(a) = dev.aspects.assembly.as_mut() {
                for c in &mut a.configs {
                    c.covers = c.covers.iter().copied().filter(|x| devices.contains(x)).collect();
                }
            }
            let key = (rank(Kind::Device), key_of(pos_of_device(*d)), d.as_uuid());
            lines.push((key, Record::Device(dev)));
        }
        for f in &functions {
            let Some(func) = q.functions().get(*f) else {
                continue;
            };
            let mut func = (**func).clone();
            func.source_ref = None;
            let pos = pos_of_function(*f).or_else(|| func.device.and_then(pos_of_device));
            lines.push((
                (rank(Kind::Function), key_of(pos), f.as_uuid()),
                Record::Function(func),
            ));
        }
        for r in &reps {
            let Some(rep) = q.reps().get(*r) else { continue };
            let mut rep = (**rep).clone();
            rep.source_ref = None;
            let key = (rank(Kind::Representation), key_of(pos_of_rep(*r)), r.as_uuid());
            lines.push((key, Record::Representation(rep)));
        }
        for c in &connections {
            let Some(con) = q.connections().get(*c) else {
                continue;
            };
            let mut con = (**con).clone();
            con.source_ref = None;
            match &mut con.kind {
                ConnectionKind::Core(_) => {
                    con.kind = ConnectionKind::Wire;
                    conductors += 1;
                }
                ConnectionKind::Bus(b) => b.cable = None,
                _ => {}
            }
            let key = (rank(Kind::Connection), key_of(pos_of_end(&con.a)), c.as_uuid());
            lines.push((key, Record::Connection(con)));
        }
        for j in &junctions {
            let Some(jn) = q.junctions().get(*j) else { continue };
            let mut jn = (**jn).clone();
            jn.source_ref = None;
            jn.sequence.retain(|c| connections.contains(c));
            let key = (
                rank(Kind::Junction),
                key_of(Some((jn.at.x.0, jn.at.y.0))),
                j.as_uuid(),
            );
            lines.push((key, Record::Junction(jn)));
        }
        for p in potentials {
            let mut p = (*p).clone();
            p.source_ref = None;
            let key = (
                rank(Kind::PotentialDef),
                key_of(pos_of_end(&p.at)),
                p.id.as_uuid(),
            );
            lines.push((key, Record::PotentialDef(p)));
        }
        for item in &items {
            let key = (
                rank(Kind::PageItem),
                key_of(page_item_pos(item)),
                item.id.as_uuid(),
            );
            lines.push((key, Record::PageItem(item.clone())));
        }
        for b in &boxes {
            let key = (
                rank(Kind::StructureBox),
                key_of(b.outline.first().map(|p| (p.x.0, p.y.0))),
                b.id.as_uuid(),
            );
            lines.push((key, Record::StructureBox(b.clone())));
        }
        if conductors > 0 {
            skipped.insert("conductor".into(), conductors);
        }
        if stations > 0 {
            skipped.insert("plc_station".into(), stations);
        }
        lines.sort_by_key(|l| l.0);
        // 5. Library copies and structure nodes.
        let mut named: BTreeSet<(Kind, Uuid)> = BTreeSet::new();
        let mut nodes: BTreeSet<Id<StructureNode>> = BTreeSet::new();
        for (_, r) in &lines {
            named_copies(r, &mut named);
            match r {
                Record::Device(d) => nodes.extend(named_nodes(&d.structure)),
                Record::StructureBox(b) => nodes.extend(named_nodes(&b.assigns)),
                _ => {}
            }
        }
        // A part's closure holds its manufacturer (WP-S1.19 D10): the held manufacturer copy travels with the part.
        let makers: Vec<Uuid> = named
            .iter()
            .filter(|(k, _)| *k == Kind::Part)
            .filter_map(|(_, id)| q.parts().get(Id::from_uuid(*id)).cloned())
            .filter_map(|p| ew_derive::manufacturer_copy(&q, &p.item.manufacturer))
            .map(|m| m.id.as_uuid())
            .collect();
        named.extend(makers.into_iter().map(|id| (Kind::Manufacturer, id)));
        let mut libs: Vec<((Kind, String, Uuid), String)> = Vec::new();
        for (kind, id) in named {
            let doc = match kind {
                Kind::Part => q
                    .parts()
                    .get(Id::from_uuid(id))
                    .map(|c| LibDocument::Part((**c).clone())),
                Kind::Symbol => q
                    .symbols()
                    .get(Id::from_uuid(id))
                    .map(|c| LibDocument::Symbol((**c).clone())),
                Kind::Fndef => q
                    .fndefs()
                    .get(Id::from_uuid(id))
                    .map(|c| LibDocument::Fndef((**c).clone())),
                Kind::Manufacturer => q
                    .manufacturers()
                    .get(Id::from_uuid(id))
                    .map(|c| LibDocument::Manufacturer((**c).clone())),
                _ => None,
            };
            let Some(doc) = doc else { continue };
            let Ok(value) = serde_json::to_value(&doc) else {
                continue;
            };
            let title = value
                .get("part_number")
                .or_else(|| value.get("key"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            // The `lib/` member order of the document itself (`$k`, `id`, the copy metadata, the item's members).
            let line = format!("{{\"$lib\":{}}}", serde_json::to_string(&doc).unwrap_or_default());
            libs.push(((kind, title, id), line));
        }
        libs.sort_by(|a, b| a.0.cmp(&b.0));
        let mut node_lines: Vec<(NodeKey, String)> = Vec::new();
        for n in nodes {
            let Some(node) = q.structure_nodes().get(n) else {
                continue;
            };
            let aspect = serde_json::to_value(&node.aspect)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default();
            let path = node_path(&q, n);
            let aliases: Vec<&str> = node.aliases.iter().map(|a| a.as_str()).collect();
            let line = format!(
                "{{\"$node\":{}}}",
                ordered(&[
                    ("id", json!(n.as_uuid().hyphenated().to_string())),
                    ("aspect", json!(aspect)),
                    ("path", json!(path)),
                    ("aliases", json!(aliases)),
                ])
            );
            node_lines.push(((aspect, path, n.as_uuid()), line));
        }
        node_lines.sort_by(|a, b| a.0.cmp(&b.0));
        // The anchor and the ghost from the copied representations' bodies, the page items' bounds and the boxes'
        // outlines.
        let folio = q
            .get::<PageOrder>(())
            .entries
            .iter()
            .find(|e| e.page == PageRef::Drawn(page_id))
            .map(|e| e.folio.clone());
        let tables = folio.and_then(|f| self.page_tables(&f));
        let bodies: Vec<[i64; 4]> = lines
            .iter()
            .filter_map(|(_, r)| match r {
                Record::Representation(rep) => {
                    let t = tables.as_ref()?.reps.iter().find(|t| t.id == rep.id.as_uuid())?;
                    Some(t.body)
                }
                Record::PageItem(item) => {
                    let t = tables
                        .as_ref()?
                        .items
                        .iter()
                        .find(|t| t.id == item.id.as_uuid())?;
                    Some(t.bounds)
                }
                Record::StructureBox(b) => outline_bounds(b),
                _ => None,
            })
            .collect();
        let min_x = bodies.iter().map(|b| b[0].min(b[2])).min().unwrap_or(0);
        let min_y = bodies.iter().map(|b| b[1].min(b[3])).min().unwrap_or(0);
        let anchor = (floor_to_grid(page.grid, min_x), floor_to_grid(page.grid, min_y));
        let ghost: Vec<[i64; 4]> = bodies
            .iter()
            .map(|b| [b[0] - anchor.0, b[1] - anchor.1, b[2] - anchor.0, b[3] - anchor.1])
            .collect();
        let page_kind = serde_json::to_value(&page.kind)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        let header = format!(
            "{{\"$fragment\":{}}}",
            ordered(&[
                ("format", json!(ew_schema::FORMAT)),
                (
                    "project",
                    json!(q.project().id.as_uuid().hyphenated().to_string())
                ),
                ("grid", json!([page.grid.num(), page.grid.den()])),
                ("page_kind", json!(page_kind)),
                ("anchor", json!([anchor.0, anchor.1])),
                ("ghost", json!(ghost)),
            ])
        );
        let mut text = String::new();
        text.push_str(FRAGMENT_HEADER);
        text.push('\n');
        text.push_str(&header);
        text.push('\n');
        for (_, r) in &lines {
            text.push_str(&serde_json::to_string(r).unwrap_or_default());
            text.push('\n');
        }
        for (_, l) in &libs {
            text.push_str(l);
            text.push('\n');
        }
        for (_, l) in &node_lines {
            text.push_str(l);
            text.push('\n');
        }
        Ok(CopiedFragment {
            text,
            records: lines.len(),
            copies: libs.len(),
            dropped_connections,
            incomplete_page_items,
            skipped: skipped.into_iter().collect(),
        })
    }

    /// Whether `u` names a stored record of any kind the copy looks at (a device, function, page, potential
    /// definition): a selection of such IDs without a representation is [`CopyError::Empty`], not generated content.
    fn names_record(&self, q: &Q, u: Uuid) -> bool {
        q.devices().contains(Id::from_uuid(u))
            || q.functions().contains(Id::from_uuid(u))
            || q.pages().contains(Id::from_uuid(u))
            || q.potential_defs().contains(Id::from_uuid(u))
            || q.structure_nodes().contains(Id::from_uuid(u))
    }

    /// The "Copy tag list" text (§8.2, FR-UI-040): the devices `ids` reach (a representation its functions' devices,
    /// a function its device, a device itself), each as its tag displayed on its main page, without repeats, in
    /// natural order, one per line with a final LF; empty for none.
    pub fn tag_list(&self, ids: &[Uuid]) -> String {
        let q = self.query();
        let mut devices: BTreeSet<Id<Device>> = BTreeSet::new();
        for u in ids {
            if let Some(r) = q.reps().get(Id::from_uuid(*u)) {
                devices.extend(drawn(r).filter_map(|f| q.functions().get(f).and_then(|f| f.device)));
            } else if let Some(f) = q.functions().get(Id::from_uuid(*u)) {
                devices.extend(f.device);
            } else if q.devices().contains(Id::from_uuid(*u)) {
                devices.insert(Id::from_uuid(*u));
            }
        }
        let mut tags: Vec<String> = devices
            .into_iter()
            .map(|d| super::properties::device_tag_text(&q, d))
            .filter(|t| !t.is_empty())
            .collect();
        tags.sort_by(|a, b| ew_mltext::natural_cmp(a, b).then_with(|| a.cmp(b)));
        tags.dedup();
        tags.into_iter().map(|t| t + "\n").collect()
    }
}

/// A compact JSON object with its members in the given order (the header and `$node` lines of §8.1).
fn ordered(members: &[(&str, Value)]) -> String {
    let parts: Vec<String> = members
        .iter()
        .map(|(k, v)| format!("{}:{v}", Value::from(*k)))
        .collect();
    format!("{{{}}}", parts.join(","))
}

/// The codes of a node's path, root first.
pub(crate) fn node_path(q: &Q, node: Id<StructureNode>) -> Vec<String> {
    let mut codes = Vec::new();
    let mut cur = Some(node);
    while let Some(n) = cur.and_then(|n| q.structure_nodes().get(n)) {
        if codes.len() > q.structure_nodes().len() {
            break;
        }
        codes.push(n.code.as_str().to_owned());
        cur = n.parent;
    }
    codes.reverse();
    codes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_skips_strings() {
        assert_eq!(depth(r#"{"a":[[1]],"b":"[[[[["}"#), 3);
        assert_eq!(depth(r#"{"a":"\"[{"}"#), 1);
    }

    #[test]
    fn floor_to_grid_is_the_largest_point_not_greater() {
        let g = ew_units::Grid::QET_10PX;
        let p = g.at(7).unwrap().0;
        assert_eq!(floor_to_grid(g, p), p);
        assert_eq!(floor_to_grid(g, p + 1), p);
        assert_eq!(floor_to_grid(g, p - 1), g.at(6).unwrap().0);
        assert_eq!(floor_to_grid(g, -1), g.at(-1).unwrap().0);
    }
}
