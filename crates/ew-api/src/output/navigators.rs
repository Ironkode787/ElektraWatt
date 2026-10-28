// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The navigators' tables (WP-S2.06 §4.2, §6.1, D2, D3): one row source per navigator,
//! [`Session::navigator_table`], a plain function over `Q` that the view model fetches once per (kind, revision) for
//! an open navigator only (no memoised family).
//!
//! **The table contract** (§6.1). Nodes come in tree order, a parent before its children, siblings in the provider's
//! default order. **Keys** are path strings `<parent key>/<role word>:<id or text>` (no leading `/` at the top level):
//! a record node carries its UUID text, a group node its label (`function:=P01/location:+J-E-2416/device:<uuid>`); the
//! Pages keys stay as built (`s:<n>`, `s:front`, `…/p:<uuid>`). **Labels** are what the drawing prints (a device's
//! short tag, a folio, a representation's `<folio>-<cell>`). **Badges** are fixed English words. **Tokens** are the
//! label and every cell, case-folded and NFC, split at whitespace, at `- . _ : / + = ( ) ,` and at every letter/digit
//! boundary ([`tokens`]). **Facets** come from the closed list `page`, `potential`, `state`, `kind`, `part`, and the
//! flags `unplaced` and `frozen`. **Drawables** are the representations (and, in part 2, connections) that draw a
//! node, main first, then page order, cell row, cell column, ID; [`NavTable::index`] maps every record and drawable ID
//! to the nodes that carry it, in node order.
//!
//! Part 1 builds Pages and Devices; the other kinds return their columns and no nodes until part 2.

use std::collections::BTreeMap;

use ew_derive::ew_ids::Id;
/// The record IDs of the tables, re-exported so that the view model names them without naming `ew-ids`.
pub use ew_derive::ew_ids::Uuid;
/// The natural sort key of a label or cell text, re-exported so that the view model sorts without naming `ew-mltext`.
pub use ew_derive::ew_mltext::NaturalKey;
use ew_derive::ew_mltext::{natural_key, to_nfc};
use ew_derive::ew_rds::Aspect;
use ew_derive::ew_schema::{Device, DeviceState, FnRole, Function, Page, Representation};
use ew_derive::{DeviceTag, PageOrder, PageRef, Q, RepAnchorCell, StructureOf};

use super::PageRow;
use super::view::PageFrameOf;
use crate::Session;

/// The eleven navigators (D4), in View ▸ Navigators order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum NavKind {
    /// Pages.
    Pages,
    /// Devices.
    Devices,
    /// Connections (part 2).
    Connections,
    /// Terminal strips (part 2).
    Strips,
    /// Cables (part 2).
    Cables,
    /// PLC (part 2).
    Plc,
    /// Potentials (part 2).
    Potentials,
    /// Interruption points (part 2).
    Ips,
    /// Structure identifiers (part 2).
    Structure,
    /// Parts in use (part 2).
    Parts,
    /// Report blocks (part 2).
    Blocks,
}

impl NavKind {
    /// The eleven kinds in menu order.
    pub const ALL: [NavKind; 11] = [
        NavKind::Pages,
        NavKind::Devices,
        NavKind::Connections,
        NavKind::Strips,
        NavKind::Cables,
        NavKind::Plc,
        NavKind::Potentials,
        NavKind::Ips,
        NavKind::Structure,
        NavKind::Parts,
        NavKind::Blocks,
    ];

    /// The kind's word: the `kind` parameter of the `navigator.*` actions and the describe text.
    pub fn word(self) -> &'static str {
        match self {
            NavKind::Pages => "pages",
            NavKind::Devices => "devices",
            NavKind::Connections => "connections",
            NavKind::Strips => "strips",
            NavKind::Cables => "cables",
            NavKind::Plc => "plc",
            NavKind::Potentials => "potentials",
            NavKind::Ips => "ips",
            NavKind::Structure => "structure",
            NavKind::Parts => "parts",
            NavKind::Blocks => "blocks",
        }
    }

    /// The kind of a word.
    pub fn from_word(w: &str) -> Option<NavKind> {
        NavKind::ALL.into_iter().find(|k| k.word() == w)
    }
}

/// What a navigator row is: its describe word and accessible kind.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum NavRole {
    /// A section group (or front matter).
    Section,
    /// A function group (`=`) or a function record.
    Function,
    /// A location group (`+`).
    Location,
    /// A page.
    Page,
    /// A device.
    Device,
    /// A representation.
    Representation,
    /// A connection.
    Connection,
    /// A potential.
    Potential,
    /// A terminal strip.
    Strip,
    /// A terminal.
    Terminal,
    /// A cable.
    Cable,
    /// A cable conductor.
    Conductor,
    /// A PLC station.
    Station,
    /// A PLC module.
    Module,
    /// A PLC channel.
    Channel,
    /// A name group (interruption points).
    Name,
    /// An interruption point.
    Ip,
    /// A structure node.
    StructureNode,
    /// A manufacturer group.
    Manufacturer,
    /// A part (BOM row).
    Part,
    /// A report block.
    ReportBlock,
}

impl NavRole {
    /// The role's word.
    pub fn word(self) -> &'static str {
        match self {
            NavRole::Section => "section",
            NavRole::Function => "function",
            NavRole::Location => "location",
            NavRole::Page => "page",
            NavRole::Device => "device",
            NavRole::Representation => "representation",
            NavRole::Connection => "connection",
            NavRole::Potential => "potential",
            NavRole::Strip => "strip",
            NavRole::Terminal => "terminal",
            NavRole::Cable => "cable",
            NavRole::Conductor => "conductor",
            NavRole::Station => "station",
            NavRole::Module => "module",
            NavRole::Channel => "channel",
            NavRole::Name => "name",
            NavRole::Ip => "ip",
            NavRole::StructureNode => "structure_node",
            NavRole::Manufacturer => "manufacturer",
            NavRole::Part => "part",
            NavRole::ReportBlock => "report_block",
        }
    }
}

/// One column of a navigator's table.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NavColumn {
    /// The column key (`folio`, `designation`).
    pub key: String,
    /// The column label (`Folio`, `Designation`).
    pub label: String,
    /// Whether the column is derived (drawn italic in table mode).
    pub derived: bool,
    /// Whether it is the key column (frozen at the left in table mode).
    pub key_column: bool,
    /// The edit route `(kind, path)` of §8.2's table, `None` for a read-only column.
    pub route: Option<(String, String)>,
    /// Why the column is read-only, when it is.
    pub read_only: Option<String>,
}

/// A representation or connection that draws a row.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Drawable {
    /// The page record it is drawn on (its page UUID).
    pub page: Uuid,
    /// The page's printed folio.
    pub folio: String,
    /// The anchor cell (`E4`), empty without a frame grid.
    pub cell: String,
    /// The representation or connection.
    pub record: Uuid,
    /// Whether it draws a main function.
    pub main: bool,
}

/// What opening a row shows: a page, with records to select there.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NavOpen {
    /// The page (its page UUID, as `PageRow::id`).
    pub page: Uuid,
    /// The records to select on it.
    pub select: Vec<Uuid>,
}

/// One node of a navigator's tree.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NavNode {
    /// The node's key, unique in the table and stable across revisions.
    pub key: String,
    /// The index of the parent node, `None` at the top level.
    pub parent: Option<u32>,
    /// What the node is.
    pub role: NavRole,
    /// The printed label.
    pub label: String,
    /// The fixed badge words (`unplaced`, `frozen`, `spare`, `generated`, `QET grid`, `main`, `aux`, `external`).
    pub badges: Vec<String>,
    /// The records the row stands for.
    pub records: Vec<Uuid>,
    /// What draws them, main first, then page order, cell row, cell column, ID.
    pub drawables: Vec<Drawable>,
    /// What Enter opens.
    pub open: Option<NavOpen>,
    /// The cell texts, aligned with the table's columns, for a `table_row` node; empty otherwise.
    pub cells: Vec<String>,
    /// Whether the node is a row of the table.
    pub table_row: bool,
    /// The filter tokens: the label and the cells, sorted and deduplicated ([`tokens`]).
    pub tokens: Vec<String>,
    /// The facets `(name, value)`; a flag has the value `""`.
    pub facets: Vec<(String, String)>,
    /// The natural keys: the label's, then one per cell.
    pub natural: Vec<NaturalKey>,
}

/// The table of one navigator at one revision.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NavTable {
    /// The navigator.
    pub kind: NavKind,
    /// The revision it was built at.
    pub rev: u64,
    /// The table's columns.
    pub columns: Vec<NavColumn>,
    /// The nodes in tree order.
    pub nodes: Vec<NavNode>,
    /// Every record and drawable ID to the nodes that carry it, in node order.
    pub index: BTreeMap<Uuid, Vec<u32>>,
}

/// The filter tokens of a text (§6.1 item 4): case-folded and NFC, split at whitespace, at `- . _ : / + = ( ) ,` and
/// at every letter/digit boundary, empty pieces dropped, in order of appearance (callers sort and deduplicate). The
/// view model splits a query with the same function, so a node token and a query token are always comparable.
pub fn tokens(text: &str) -> Vec<String> {
    let folded = to_nfc(&text.to_lowercase()).into_owned();
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut last: Option<bool> = None;
    for ch in folded.chars() {
        if ch.is_whitespace() || "-._:/+=(),".contains(ch) {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            last = None;
            continue;
        }
        let digit = ch.is_numeric();
        let alpha = ch.is_alphabetic();
        if let Some(prev) = last
            && (alpha || digit)
            && prev != digit
            && !cur.is_empty()
        {
            out.push(std::mem::take(&mut cur));
        }
        cur.push(ch);
        last = if alpha || digit { Some(digit) } else { None };
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The natural sort key of a text (`ew_mltext::natural_key`), for a view model that builds rows of its own (the Pages
/// tree) or a synthetic table in a test, without naming `ew-mltext`.
pub fn nav_natural_key(text: &str) -> NaturalKey {
    natural_key(text)
}

/// The tokens of a label and its cells, sorted and deduplicated.
fn node_tokens(label: &str, cells: &[String]) -> Vec<String> {
    let mut t: Vec<String> = std::iter::once(label)
        .chain(cells.iter().map(String::as_str))
        .flat_map(tokens)
        .collect();
    t.sort();
    t.dedup();
    t
}

/// The natural keys of a label and its cells.
fn natural(label: &str, cells: &[String]) -> Vec<NaturalKey> {
    std::iter::once(label)
        .chain(cells.iter().map(String::as_str))
        .map(natural_key)
        .collect()
}

/// A column of a report kind or of this module: its label, no route, read-only with "no command yet" unless a route
/// of §8.2 is given.
fn column(key: &str, label: &str, key_column: bool, route: Option<(&str, &str)>) -> NavColumn {
    NavColumn {
        key: key.to_owned(),
        label: label.to_owned(),
        derived: false,
        key_column,
        route: route.map(|(k, p)| (k.to_owned(), p.to_owned())),
        read_only: route.is_none().then(|| "no command yet".to_owned()),
    }
}

/// A column key as a label: the first letter in capitals and `_` read as a space (`page_cell` → "Page cell").
fn key_label(key: &str) -> String {
    let text = key.replace('_', " ");
    let mut chars = text.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The columns of a report kind, the first one the key column.
fn kind_columns(kind: &str) -> Vec<NavColumn> {
    ew_reports::Registry::builtin()
        .by_name(kind)
        .map(|k| {
            k.columns()
                .iter()
                .enumerate()
                .map(|(i, c)| column(&c.key, &super::view::neutral(&c.label), i == 0, None))
                .collect()
        })
        .unwrap_or_default()
}

/// The columns of a navigator (§6.6, §7.1; the routes of §8.2 for part 1's kinds).
fn columns_of(kind: NavKind) -> Vec<NavColumn> {
    let own = |keys: &[&str]| -> Vec<NavColumn> {
        keys.iter()
            .enumerate()
            .map(|(i, k)| column(k, &key_label(k), i == 0, None))
            .collect()
    };
    match kind {
        NavKind::Pages => vec![
            column("folio", "Folio", true, None),
            column("title", "Title", false, Some(("page", "title"))),
            column("type", "Type", false, Some(("page", "kind"))),
            column("frame", "Frame", false, None),
            column("generated", "Generated", false, None),
        ],
        NavKind::Devices => kind_columns("structure_overview")
            .into_iter()
            .map(|mut c| {
                if c.key == "function_text" {
                    c.route = Some(("device".to_owned(), "function_text".to_owned()));
                    c.read_only = None;
                }
                c
            })
            .collect(),
        NavKind::Connections => kind_columns("wire_list"),
        NavKind::Strips => {
            let mut c = own(&["strip"]);
            c.extend(kind_columns("terminal_diagram").into_iter().map(|mut c| {
                c.key_column = false;
                c
            }));
            c
        }
        NavKind::Cables => kind_columns("cable_overview_diagrams"),
        NavKind::Plc => kind_columns("plc_overview"),
        NavKind::Potentials => kind_columns("potential_overview"),
        NavKind::Ips => own(&["name", "page_cell", "direction", "partner", "external"]),
        NavKind::Structure => {
            let mut c = kind_columns("structure_overview");
            c.push(column("usage", "Usage", false, None));
            c
        }
        NavKind::Parts => kind_columns("parts_summary"),
        NavKind::Blocks => own(&["title", "kind", "first_folio", "section", "pages"]),
    }
}

/// The word of a device state (`used`, `spare`, `reserved`, `not_fitted`).
fn state_word(s: &DeviceState) -> &'static str {
    match s {
        DeviceState::Spare => "spare",
        DeviceState::Reserved => "reserved",
        DeviceState::NotFitted => "not_fitted",
        _ => "used",
    }
}

/// A table under construction.
struct Builder {
    nodes: Vec<NavNode>,
}

impl Builder {
    /// Appends a node and returns its index.
    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        parent: Option<u32>,
        role: NavRole,
        segment: &str,
        label: String,
        badges: Vec<String>,
        records: Vec<Uuid>,
        drawables: Vec<Drawable>,
        cells: Option<Vec<String>>,
        facets: Vec<(String, String)>,
    ) -> u32 {
        let key = match parent {
            Some(p) => format!("{}/{segment}", self.nodes[p as usize].key),
            None => segment.to_owned(),
        };
        let open = drawables.first().map(|d| NavOpen {
            page: d.page,
            select: drawables
                .iter()
                .filter(|x| x.page == d.page)
                .map(|x| x.record)
                .collect(),
        });
        let table_row = cells.is_some();
        let cells = cells.unwrap_or_default();
        let at = u32::try_from(self.nodes.len()).unwrap_or(u32::MAX);
        self.nodes.push(NavNode {
            key,
            parent,
            role,
            tokens: node_tokens(&label, &cells),
            natural: natural(&label, &cells),
            label,
            badges,
            records,
            drawables,
            open,
            cells,
            table_row,
            facets,
        });
        at
    }

    /// The table with its index.
    fn finish(self, kind: NavKind, rev: u64) -> NavTable {
        let mut index: BTreeMap<Uuid, Vec<u32>> = BTreeMap::new();
        for (i, n) in self.nodes.iter().enumerate() {
            let i = u32::try_from(i).unwrap_or(u32::MAX);
            let ids = n.records.iter().chain(n.drawables.iter().map(|d| &d.record));
            for id in ids {
                let v = index.entry(*id).or_default();
                if v.last() != Some(&i) {
                    v.push(i);
                }
            }
        }
        NavTable {
            kind,
            rev,
            columns: columns_of(kind),
            nodes: self.nodes,
            index,
        }
    }
}

impl Session {
    /// The table of navigator `kind` at the current revision (WP-S2.06 §4.2, §6.1): a plain function over `Q`,
    /// deterministic, reading memoised queries and `ew_reports::project_rows` only. Part 1 builds Pages and Devices;
    /// every other kind returns its columns and no nodes.
    pub fn navigator_table(&self, kind: NavKind) -> NavTable {
        let rev = self.world().rev.0;
        let mut b = Builder { nodes: Vec::new() };
        match kind {
            NavKind::Pages => self.pages_table(&mut b),
            NavKind::Devices => devices_table(&self.query(), &mut b),
            _ => {}
        }
        b.finish(kind, rev)
    }

    /// The Pages tree as built (WP-S2.03 §6.1, section view): section nodes (front matter first, as the page order
    /// meets them), the pages under them.
    fn pages_table(&self, b: &mut Builder) {
        let q = self.query();
        let order = q.get::<PageOrder>(());
        let rows: Vec<PageRow> = self.page_rows();
        let mut sections: BTreeMap<String, u32> = BTreeMap::new();
        for (row, entry) in rows.iter().zip(order.entries.iter()) {
            let (skey, slabel) = match &row.section {
                None => ("s:front".to_owned(), "Front matter".to_owned()),
                Some((n, title)) => (format!("s:{n}"), format!("Section {n} - {title}")),
            };
            let parent = match sections.get(&skey) {
                Some(p) => *p,
                None => {
                    let p = b.push(
                        None,
                        NavRole::Section,
                        &skey,
                        slabel,
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        None,
                        vec![("kind".to_owned(), "section".to_owned())],
                    );
                    sections.insert(skey, p);
                    p
                }
            };
            let mut badges = Vec::new();
            if row.generated {
                badges.push("generated".to_owned());
            }
            if row.qet_grid {
                badges.push("QET grid".to_owned());
            }
            let frame = q
                .get::<PageFrameOf>(entry.page)
                .as_ref()
                .as_ref()
                .and_then(|f| f.form.meta.key.as_ref().map(|k| k.as_str().to_owned()))
                .unwrap_or_default();
            let cells = vec![
                row.folio.clone(),
                row.title.clone(),
                row.kind.clone(),
                frame,
                if row.generated { "yes" } else { "no" }.to_owned(),
            ];
            let records = match entry.page {
                PageRef::Drawn(p) => vec![p.as_uuid()],
                _ => Vec::new(),
            };
            let at = b.push(
                Some(parent),
                NavRole::Page,
                &format!("p:{}", row.id),
                row.folio.clone(),
                badges,
                records,
                Vec::new(),
                Some(cells),
                vec![
                    ("page".to_owned(), row.folio.to_lowercase()),
                    ("kind".to_owned(), "page".to_owned()),
                ],
            );
            b.nodes[at as usize].open = Some(NavOpen {
                page: row.id,
                select: Vec::new(),
            });
        }
    }
}

/// The page order's folio and position of every drawn page.
fn folios(q: &Q) -> BTreeMap<Id<Page>, (usize, String, Uuid)> {
    q.get::<PageOrder>(())
        .entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e.page {
            PageRef::Drawn(p) => Some((p, (i, e.folio.clone(), super::page_uuid(e.page)))),
            _ => None,
        })
        .collect()
}

/// A drawable with its sort key: (not main, page position, cell row, cell column, ID).
type Keyed = ((bool, usize, u32, u32, Uuid), Drawable);

/// A device with its natural tag key.
type Ranked = (NaturalKey, Id<Device>);

/// The drawable of a representation, with its sort key.
fn drawable(
    q: &Q,
    folios: &BTreeMap<Id<Page>, (usize, String, Uuid)>,
    rep: Id<Representation>,
    main: bool,
) -> Option<Keyed> {
    let r = q.reps().get(rep)?;
    let (pos, folio, page) = folios.get(&r.page)?.clone();
    let cell = q.get::<RepAnchorCell>(rep).as_ref().clone();
    let (row, col, text) = cell.map_or((u32::MAX, u32::MAX, String::new()), |c| {
        (c.row, c.column, c.to_string())
    });
    Some((
        (!main, pos, row, col, rep.as_uuid()),
        Drawable {
            page,
            folio,
            cell: text,
            record: rep.as_uuid(),
            main,
        },
    ))
}

/// Drawables sorted by their keys, deduplicated by record.
fn sorted(mut v: Vec<Keyed>) -> Vec<Drawable> {
    v.sort_by_key(|a| a.0);
    let mut seen = std::collections::BTreeSet::new();
    v.into_iter()
        .filter(|(_, d)| seen.insert(d.record))
        .map(|(_, d)| d)
        .collect()
}

/// The label of a representation row: `<folio>-<cell>`, the folio alone without a cell.
fn rep_label(d: &Drawable) -> String {
    if d.cell.is_empty() {
        d.folio.clone()
    } else {
        format!("{}-{}", d.folio, d.cell)
    }
}

/// The name of a function's FnDef: its key, else the copy's UUID text.
fn fndef_name(q: &Q, f: &Function) -> String {
    q.fndefs()
        .get(f.def)
        .and_then(|c| c.item.meta.key.as_ref().map(|k| k.as_str().to_owned()))
        .or_else(|| {
            super::editing::starter_fndef(f.def)
                .and_then(|c| c.item.meta.key.as_ref().map(|k| k.as_str().to_owned()))
        })
        .unwrap_or_else(|| f.def.as_uuid().to_string())
}

/// One device of the Devices table: the device row with its cells (from the structure overview).
struct DeviceRow {
    cells: Vec<String>,
    part: String,
}

/// The Devices tree (§6.7): function nodes, location nodes inside them, the devices that are not children of another
/// listed device in natural tag order; child devices under their parent (position, then natural tag); under each
/// device its functions (main first, FnDef name, ID), under each function its representations. Terminals are not
/// listed (they are the Terminal strips navigator's rows, as in the structure overview).
fn devices_table(q: &Q, b: &mut Builder) {
    let Some(set) = ew_reports::project_rows(q, "structure_overview", None) else {
        return;
    };
    let part_col = set.column("part");
    let mut rows: BTreeMap<Id<Device>, DeviceRow> = BTreeMap::new();
    for (i, r) in set.rows.iter().enumerate() {
        let Some(&src) = r.sources.first() else { continue };
        let id: Id<Device> = Id::from_uuid(src.as_uuid());
        if !q.devices().contains(id) {
            continue;
        }
        let cells = ew_reports::csv::row_texts(q, &set, i);
        let part = part_col.and_then(|c| cells.get(c).cloned()).unwrap_or_default();
        rows.insert(id, DeviceRow { cells, part });
    }
    let folios = folios(q);
    let tag = |d: Id<Device>| {
        q.get::<DeviceTag>(d)
            .as_ref()
            .as_ref()
            .map(ew_derive::Tag::short)
            .unwrap_or_default()
    };
    let aspect_text = |d: Id<Device>, a: Aspect| q.get::<StructureOf>(d).designation(|x| x == a).to_string();
    // The top-level devices by (function, location), in natural tag order.
    let top: Vec<Id<Device>> = rows
        .keys()
        .copied()
        .filter(|d| {
            q.devices()
                .get(*d)
                .and_then(|x| x.parent)
                .is_none_or(|p| !rows.contains_key(&p))
        })
        .collect();
    let mut groups: BTreeMap<(String, String), Vec<Ranked>> = BTreeMap::new();
    for d in top {
        let key = (aspect_text(d, Aspect::Function), aspect_text(d, Aspect::Location));
        groups.entry(key).or_default().push((natural_key(&tag(d)), d));
    }
    let mut group_list: Vec<((String, String), Vec<Ranked>)> = groups.into_iter().collect();
    group_list.sort_by(|a, b| {
        let k = |s: &str| (s.is_empty(), natural_key(s));
        k(&a.0.0).cmp(&k(&b.0.0)).then_with(|| k(&a.0.1).cmp(&k(&b.0.1)))
    });
    let mut fnodes: BTreeMap<String, u32> = BTreeMap::new();
    let none = "(none)";
    for ((fun, loc), mut devices) in group_list {
        let flabel = if fun.is_empty() {
            none.to_owned()
        } else {
            fun.clone()
        };
        let fseg = format!("function:{}", if fun.is_empty() { "-" } else { &fun });
        let fnode = match fnodes.get(&fseg) {
            Some(n) => *n,
            None => {
                let n = b.push(
                    None,
                    NavRole::Function,
                    &fseg,
                    flabel,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    None,
                    vec![("kind".to_owned(), "function".to_owned())],
                );
                fnodes.insert(fseg.clone(), n);
                n
            }
        };
        let llabel = if loc.is_empty() {
            none.to_owned()
        } else {
            loc.clone()
        };
        let lseg = format!("location:{}", if loc.is_empty() { "-" } else { &loc });
        let lnode = b.push(
            Some(fnode),
            NavRole::Location,
            &lseg,
            llabel,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            vec![("kind".to_owned(), "location".to_owned())],
        );
        devices.sort();
        for (_, d) in devices {
            device_node(q, b, &rows, &folios, &tag, lnode, d);
        }
    }
}

/// A device node with its child devices, functions and representations.
fn device_node(
    q: &Q,
    b: &mut Builder,
    rows: &BTreeMap<Id<Device>, DeviceRow>,
    folios: &BTreeMap<Id<Page>, (usize, String, Uuid)>,
    tag: &dyn Fn(Id<Device>) -> String,
    parent: u32,
    d: Id<Device>,
) {
    let Some(row) = rows.get(&d) else { return };
    let Some(dev) = q.devices().get(d) else { return };
    // The functions: main first, then FnDef name, then ID.
    let mut fns: Vec<(bool, String, Id<Function>)> = q
        .idx()
        .functions_of_device(d)
        .filter_map(|f| {
            q.functions()
                .get(f)
                .map(|x| (x.role != FnRole::Main, fndef_name(q, x), f))
        })
        .collect();
    fns.sort();
    let fn_drawables: Vec<(Id<Function>, bool, Vec<Drawable>)> = fns
        .iter()
        .map(|(aux, _, f)| {
            let reps = q
                .idx()
                .reps_of_function(*f)
                .filter_map(|r| drawable(q, folios, r, !aux))
                .collect();
            (*f, !aux, sorted(reps))
        })
        .collect();
    let all: Vec<Keyed> = fn_drawables
        .iter()
        .flat_map(|(_, _, ds)| ds.iter())
        .filter_map(|x| {
            let r: Id<Representation> = Id::from_uuid(x.record);
            drawable(q, folios, r, x.main)
        })
        .collect();
    let drawables = sorted(all);
    let label = tag(d);
    let mut badges = Vec::new();
    let mut facets = vec![("kind".to_owned(), "device".to_owned())];
    if drawables.is_empty() {
        badges.push("unplaced".to_owned());
        facets.push(("unplaced".to_owned(), String::new()));
    }
    if dev.tag.frozen.is_some() {
        badges.push("frozen".to_owned());
        facets.push(("frozen".to_owned(), String::new()));
    }
    if dev.state == DeviceState::Spare {
        badges.push("spare".to_owned());
    }
    facets.push(("state".to_owned(), state_word(&dev.state).to_owned()));
    if !row.part.is_empty() {
        facets.push(("part".to_owned(), row.part.to_lowercase()));
    }
    let mut pages: Vec<String> = drawables.iter().map(|x| x.folio.to_lowercase()).collect();
    pages.sort();
    pages.dedup();
    facets.extend(pages.into_iter().map(|p| ("page".to_owned(), p)));
    let node = b.push(
        Some(parent),
        NavRole::Device,
        &format!("device:{}", d.as_uuid()),
        label,
        badges,
        vec![d.as_uuid()],
        drawables,
        Some(row.cells.clone()),
        facets,
    );

    // Child devices (by `parent`), in position order, then natural tag.
    let mut children: Vec<(Option<u32>, NaturalKey, Id<Device>)> = rows
        .keys()
        .copied()
        .filter(|c| q.devices().get(*c).and_then(|x| x.parent) == Some(d))
        .map(|c| {
            let pos = q.devices().get(c).and_then(|x| x.position);
            (pos, natural_key(&tag(c)), c)
        })
        .collect();
    children.sort_by(|a, b| (a.0.is_none(), a.0, &a.1, a.2).cmp(&(b.0.is_none(), b.0, &b.1, b.2)));
    for (_, _, c) in children {
        device_node(q, b, rows, folios, tag, node, c);
    }
    for ((_, name, f), (_, main, ds)) in fns.iter().zip(fn_drawables) {
        let mut badges = vec![if main { "main" } else { "aux" }.to_owned()];
        let mut facets = vec![("kind".to_owned(), "function".to_owned())];
        if ds.is_empty() {
            badges.push("unplaced".to_owned());
            facets.push(("unplaced".to_owned(), String::new()));
        }
        let mut pages: Vec<String> = ds.iter().map(|x| x.folio.to_lowercase()).collect();
        pages.sort();
        pages.dedup();
        facets.extend(pages.into_iter().map(|p| ("page".to_owned(), p)));
        let fnode = b.push(
            Some(node),
            NavRole::Function,
            &format!("function:{}", f.as_uuid()),
            name.clone(),
            badges,
            vec![f.as_uuid()],
            ds.clone(),
            None,
            facets,
        );
        for x in ds {
            let facets = vec![
                ("kind".to_owned(), "representation".to_owned()),
                ("page".to_owned(), x.folio.to_lowercase()),
            ];
            let badges = vec![if x.main { "main" } else { "aux" }.to_owned()];
            b.push(
                Some(fnode),
                NavRole::Representation,
                &format!("representation:{}", x.record),
                rep_label(&x),
                badges,
                vec![x.record],
                vec![x.clone()],
                None,
                facets,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_split_at_separators_and_letter_digit_boundaries() {
        assert_eq!(tokens("-201U3"), ["201", "u", "3"]);
        assert_eq!(tokens("=P01 +J-E-2416"), ["p", "01", "j", "e", "2416"]);
        assert_eq!(tokens("Wire list (1),x_y"), ["wire", "list", "1", "x", "y"]);
        assert_eq!(tokens("  "), Vec::<String>::new());
        assert_eq!(tokens("Ä1"), ["ä", "1"]);
    }

    #[test]
    fn kinds_and_roles_have_their_words() {
        let words: Vec<&str> = NavKind::ALL.iter().map(|k| k.word()).collect();
        assert_eq!(
            words,
            [
                "pages",
                "devices",
                "connections",
                "strips",
                "cables",
                "plc",
                "potentials",
                "ips",
                "structure",
                "parts",
                "blocks"
            ]
        );
        for k in NavKind::ALL {
            assert_eq!(NavKind::from_word(k.word()), Some(k));
        }
        assert_eq!(NavRole::StructureNode.word(), "structure_node");
        assert_eq!(key_label("page_cell"), "Page cell");
        assert_eq!(key_label("first_folio"), "First folio");
    }
}
