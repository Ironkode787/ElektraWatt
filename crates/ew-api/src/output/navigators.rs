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
//! Part 1 built Pages and Devices; part 2 the nine further providers (§7.1): Connections and Potentials from the wire
//! list and the potential overview, Terminal strips from each strip's terminal diagram, Cables from the cable
//! overview, PLC from the PLC overview, Parts in use from the parts summary in component mode, Structure identifiers
//! from the structure overview's node rows, and Interruption points and Report blocks with their own columns. A data
//! row without records (a heading) is never a table row. Part 2 also adds [`Session::highlight_members`] (§7.3).

use std::collections::BTreeMap;

use ew_derive::ew_ids::Id;
/// The record IDs of the tables, re-exported so that the view model names them without naming `ew-ids`.
pub use ew_derive::ew_ids::Uuid;
/// The natural sort key of a label or cell text, re-exported so that the view model sorts without naming `ew-mltext`.
pub use ew_derive::ew_mltext::NaturalKey;
use ew_derive::ew_mltext::{natural_key, to_nfc};
use ew_derive::ew_rds::Aspect;
use ew_derive::ew_schema::{
    Device, DeviceState, FnDef, FnRole, Function, Page, Representation, TemplateFn, template_entry_of,
};
use ew_derive::{DeviceTag, PageOrder, PageRef, Q, RepAnchorCell, StructureOf};

use super::PageRow;
use super::view::PageFrameOf;
use crate::Session;

/// The twelve navigators (D4; WP-S2.09 D8 adds the message panel last), in View ▸ Navigators order.
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
    /// The message panel (WP-S2.09 part 2, D8): the findings of the project's online check scheme.
    Messages,
}

impl NavKind {
    /// The twelve kinds in menu order.
    pub const ALL: [NavKind; 12] = [
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
        NavKind::Messages,
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
            NavKind::Messages => "messages",
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
    /// A strip accessory (WP-S2.07 D3): a row of the Terminal strips navigator.
    Accessory,
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
    /// A severity group of the message panel (WP-S2.09 part 2).
    Severity,
    /// A rule group of the message panel.
    Rule,
    /// A finding of the message panel.
    Finding,
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
            NavRole::Accessory => "accessory",
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
            NavRole::Severity => "severity",
            NavRole::Rule => "rule",
            NavRole::Finding => "finding",
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
    /// A strip-editor column whose cells compile through `Session::strip_edit` (WP-S2.07 §6.3); `None` elsewhere.
    pub strip_cell: Option<super::strip_edit::StripCell>,
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
    /// The records' kinds for edit routing, aligned with `records`; empty where the builder does not state them (not
    /// printed).
    pub kinds: Vec<String>,
    /// For a Potentials row of a potential known only at connection points (`kinds` is `cp`): the CP its record names
    /// (the function and the CP index), where a Name typed in table mode, with Type and Voltage if typed with it,
    /// defines it with `potential.define` (the G-PP triage's row 8, amending ID-752; WP-S1.16 D4); `None` elsewhere
    /// (not printed). It carries no name: the definition takes the typed one.
    pub define_at: Option<(Uuid, u16)>,
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
        strip_cell: None,
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

/// The editable columns of table mode (WP-S2.06 part 3, §8.2; D10): (navigator, column key, route kind, route path).
/// Every other column is read-only.
const EDITABLE_COLUMNS: [(NavKind, &str, &str, &str); 22] = [
    (NavKind::Pages, "title", "page", "title"),
    (NavKind::Pages, "type", "page", "kind"),
    (NavKind::Devices, "function_text", "device", "function_text"),
    (NavKind::Connections, "colour", "connection", "overrides.colour"),
    (
        NavKind::Connections,
        "cross_section",
        "connection",
        "overrides.csa",
    ),
    (NavKind::Strips, "terminal", "device", "terminal.designation"),
    (NavKind::Strips, "label", "device", "function_text"),
    (NavKind::Strips, "state", "device", "state"),
    (NavKind::Cables, "colour", "conductor", "colour"),
    (NavKind::Cables, "cross_section", "conductor", "csa"),
    // WP-S2.07 part 2 (§7, FR-CBL-016): a core's name (an identifier of its cable), shield and pair; a cable's length
    // through its typed editor (FT-429).
    (NavKind::Cables, "core", "conductor", "core"),
    (NavKind::Cables, "shield", "conductor", "shield"),
    (NavKind::Cables, "pair", "conductor", "pair"),
    (NavKind::Cables, "length", "device", "cable.length"),
    (NavKind::Potentials, "potential", "potential_def", "name"),
    // The G-PP triage's row 8: Type and Voltage set a definition's `ptype` and `voltage`; on a potential without one
    // the Name defines it, with Type and Voltage typed in the same gesture (WP-S1.16 D4; the view model compiles
    // `potential.define` at `NavNode::define_at`).
    (NavKind::Potentials, "kind", "potential_def", "ptype"),
    (NavKind::Potentials, "voltage", "potential_def", "voltage"),
    (NavKind::Potentials, "class", "potential_def", "circuit_class"),
    (NavKind::Potentials, "colour", "potential_def", "props.colour"),
    (NavKind::Potentials, "cross_section", "potential_def", "props.csa"),
    (NavKind::Structure, "designation", "structure_node", "code"),
    (
        NavKind::Structure,
        "function_text",
        "structure_node",
        "description",
    ),
];

/// The derived columns (WP-S2.06 part 3, §8.1, §8.2): cells the derivation computes from other records rather than a
/// field of the row's record (a connection's or terminal's potential, the folios a row is drawn on, counts). Drawn
/// italic in table mode and read-only with the reason "derived".
const DERIVED_COLUMNS: [(NavKind, &str); 10] = [
    (NavKind::Pages, "changed_in"),
    (NavKind::Connections, "potential"),
    (NavKind::Cables, "potential"),
    (NavKind::Connections, "pages"),
    (NavKind::Strips, "potential"),
    (NavKind::Potentials, "aliases"),
    (NavKind::Potentials, "members"),
    (NavKind::Potentials, "pages"),
    (NavKind::Structure, "usage"),
    (NavKind::Blocks, "pages"),
];

/// The strip-editor columns of the Terminal strips navigator (WP-S2.07 §6.2, §6.3): (column key, its compile).
const STRIP_CELLS: [(&str, super::strip_edit::StripCell); 3] = [
    ("group", super::strip_edit::StripCell::Group),
    ("bridge_part", super::strip_edit::StripCell::BridgePart),
    ("part", super::strip_edit::StripCell::AccessoryPart),
];

/// The columns of a navigator (§6.6, §7.1) with the routes of §8.2's table and the derived flags.
fn columns_of(kind: NavKind) -> Vec<NavColumn> {
    let mut columns = plain_columns_of(kind);
    for c in &mut columns {
        if kind == NavKind::Strips
            && let Some((_, cell)) = STRIP_CELLS.iter().find(|(k, _)| *k == c.key)
        {
            c.strip_cell = Some(*cell);
        }
        if DERIVED_COLUMNS.contains(&(kind, c.key.as_str())) {
            c.derived = true;
            c.route = None;
            c.read_only = Some("derived".to_owned());
        } else if let Some((_, _, k, p)) = EDITABLE_COLUMNS
            .iter()
            .find(|(n, key, _, _)| *n == kind && *key == c.key)
        {
            c.route = Some(((*k).to_owned(), (*p).to_owned()));
            c.read_only = None;
        }
    }
    columns
}

/// The columns of a navigator before §8.2's routes (§6.6, §7.1).
fn plain_columns_of(kind: NavKind) -> Vec<NavColumn> {
    let own = |keys: &[&str]| -> Vec<NavColumn> {
        keys.iter()
            .enumerate()
            .map(|(i, k)| column(k, &key_label(k), i == 0, None))
            .collect()
    };
    match kind {
        NavKind::Pages => vec![
            column("folio", "Folio", true, None),
            column("title", "Title", false, None),
            column("type", "Type", false, None),
            column("frame", "Frame", false, None),
            column("generated", "Generated", false, None),
            // WP-S1.18 part 2 (D7): the page change status of doc 15 §5.6.
            column("changed_in", "Changed in", false, None),
        ],
        NavKind::Devices => kind_columns("structure_overview"),
        NavKind::Connections => kind_columns("wire_list"),
        // WP-S2.07 D2, D6: `pos` after `strip`, `bridge_part` after `bridge`.
        NavKind::Strips => {
            let mut c = own(&["strip"]);
            c.push(column("pos", "Pos", false, None));
            for mut k in kind_columns("terminal_diagram") {
                k.key_column = false;
                let bridge = k.key == "bridge";
                c.push(k);
                if bridge {
                    c.push(column("bridge_part", "Bridge part", false, None));
                }
            }
            c
        }
        // WP-S2.07 part 2 (§7): the cable overview's columns (a core row's From and To are the far ends of its
        // wires, as the cable diagram prints them), then a core's derived potential, its shield flag and pair.
        NavKind::Cables => {
            let mut c = kind_columns("cable_overview_diagrams");
            c.extend(["potential", "shield", "pair"].map(|k| column(k, &key_label(k), false, None)));
            c
        }
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
        // WP-S2.09 §7.1: every column is derived and read-only; `rule` is the key column.
        NavKind::Messages => MESSAGE_COLUMNS
            .iter()
            .map(|k| NavColumn {
                key: (*k).to_owned(),
                label: key_label(k),
                derived: true,
                key_column: *k == "rule",
                route: None,
                read_only: Some("derived".to_owned()),
                strip_cell: None,
            })
            .collect(),
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
            kinds: Vec::new(),
            define_at: None,
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
    /// deterministic, reading memoised queries and `ew_reports::project_rows` only, for every kind of [`NavKind`].
    pub fn navigator_table(&self, kind: NavKind) -> NavTable {
        let rev = self.world().rev.0;
        let mut b = Builder { nodes: Vec::new() };
        let q = self.query();
        match kind {
            NavKind::Pages => self.pages_table(&mut b),
            NavKind::Devices => devices_table(&q, &mut b),
            NavKind::Connections => connections_table(&q, &mut b),
            NavKind::Strips => strips_table(&q, &mut b),
            NavKind::Cables => cables_table(&q, &mut b),
            NavKind::Plc => plc_table(&q, &mut b),
            NavKind::Potentials => potentials_table(&q, &mut b),
            NavKind::Ips => ips_table(&q, &mut b),
            NavKind::Structure => structure_table(&q, &mut b),
            NavKind::Parts => parts_table(&q, &mut b),
            NavKind::Blocks => blocks_table(&q, &mut b),
            NavKind::Messages => self.messages_table(&q, &mut b),
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
                row.changed_in.clone().unwrap_or_default(),
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

/// The human text of a function's kind (FT-670, FT-664): the FnDef's last category entry, then the one before it
/// lower-cased, then the CP designations of the function's template entry (else of the FnDef, in `idx` order) joined
/// by `-`. Category `["Relay", "Contact", "NO"]` with CPs 13 and 14 gives `NO contact 13-14`; a last entry equal to
/// the CP designations run together is dropped and the one before it capitalised (`["Relay", "Coil", "A1A2"]` with
/// CPs A1 and A2 gives `Coil A1-A2`); a category of fewer
/// than two non-empty entries, or a missing FnDef copy, gives [`fndef_name`]. The Properties grid's "Part function"
/// row and the picker's meta line read it.
pub(crate) fn function_kind_text(q: &Q, f: &Function) -> String {
    let fndefs = q.fndefs();
    let def = fndefs
        .get(f.def)
        .map(|c| &c.item)
        .or_else(|| super::editing::starter_fndef(f.def).map(|c| &c.item));
    let Some(def) = def else {
        return fndef_name(q, f);
    };
    let cps = template_cp_designations(q, f, def).unwrap_or_else(|| {
        let mut d: Vec<(u16, &str)> = def.cps.iter().map(|c| (c.idx, c.designation.as_str())).collect();
        d.sort_unstable();
        d.into_iter().map(|(_, s)| s.to_owned()).collect()
    });
    kind_text(def, &cps).unwrap_or_else(|| fndef_name(q, f))
}

/// The kind text of a FnDef with the given CP designations (see [`function_kind_text`]): `None` when its category has
/// fewer than two non-empty entries. The place catalogue reads it for a template entry not yet placed.
pub(crate) fn kind_text(def: &FnDef, cps: &[String]) -> Option<String> {
    let (a, b, c) = &def.category;
    let cat: Vec<&str> = [a, b, c]
        .iter()
        .map(|i| i.as_str())
        .filter(|s| !s.is_empty())
        .collect();
    let [.., before, last] = cat.as_slice() else {
        return None;
    };
    // Cycle 37 (window check W37): a last entry that only repeats the CP designations run together (a coil's
    // `A1A2`) is dropped, and the entry before it leads, capitalised: `Coil A1-A2`, not `A1A2 coil A1-A2`.
    let joined: String = cps.concat();
    let mut out = if !joined.is_empty() && last.eq_ignore_ascii_case(&joined) {
        let mut chars = before.chars();
        chars
            .next()
            .map(|c| c.to_uppercase().chain(chars).collect())
            .unwrap_or_default()
    } else {
        format!("{last} {}", before.to_lowercase())
    };
    if !cps.is_empty() {
        out.push(' ');
        out.push_str(&cps.join("-"));
    }
    Some(out)
}

/// The Properties grid's "Part function" text of a function (FT-670): [`function_kind_text`] when the function
/// records `template_entry` and that entry resolves among its device's part uses (`ew_schema::template_entry_of`);
/// `None` otherwise (the row is hidden).
pub(crate) fn part_function_text(q: &Q, f: &Function) -> Option<String> {
    f.template_entry?;
    template_entry_resolves(q, f).then(|| function_kind_text(q, f))
}

/// Whether a function's device has a part use whose template has an entry naming the function's FnDef copy.
fn template_entry_resolves(q: &Q, f: &Function) -> bool {
    let Some(device) = f.device.and_then(|d| q.devices().get(d).cloned()) else {
        return false;
    };
    let parts = q.parts();
    let uses = device
        .parts
        .iter()
        .filter_map(|u| parts.get(u.part.copy).map(|p| ((), &p.item)));
    template_entry_of(uses, f.template_entry, |t| names_def(t, f)).is_some()
}

/// Whether a template entry names the function's FnDef copy (`v5(NS_LIBCOPY, library ‖ item)`).
fn names_def(t: &TemplateFn, f: &Function) -> bool {
    let copy: Id<FnDef> =
        ew_derive::ew_ids::v5::lib_copy_id(t.fndef.library.as_uuid(), t.fndef.item.as_uuid());
    copy == f.def
}

/// The CP designations of a template-origin function's entry, in function CP order (`ew_schema::template_entry_of`
/// and `TemplateFn::function_cps`); `None` when the function follows no template entry or the entry selects none.
fn template_cp_designations(q: &Q, f: &Function, def: &FnDef) -> Option<Vec<String>> {
    let device = q.devices().get(f.device?)?;
    let parts = q.parts();
    let uses = device
        .parts
        .iter()
        .filter_map(|u| parts.get(u.part.copy).map(|p| ((), &p.item)));
    let ((), part, n) = template_entry_of(uses, f.template_entry, |t| names_def(t, f))?;
    let pattern: Vec<u16> = part.cp_pattern.iter().map(|c| c.idx).collect();
    let def_idx: Vec<u16> = def.cps.iter().map(|c| c.idx).collect();
    let mut pairs = part.function_template[n].function_cps(&pattern, &def_idx);
    pairs.sort_by_key(|t| t.idx);
    let out: Vec<String> = pairs
        .iter()
        .filter_map(|t| part.cp_pattern.iter().find(|c| c.idx == t.pattern))
        .map(|c| c.designation.as_str().to_owned())
        .collect();
    (!out.is_empty()).then_some(out)
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
        // Cycle 37 (window check W37): the function's kind in words (`Coil A1-A2`, `NO contact 13-14`), its FnDef key
        // only for a FnDef without a category (doc 16a P2); the order stays the FnDef key's.
        let label = q
            .functions()
            .get(*f)
            .map_or_else(|| name.clone(), |x| function_kind_text(q, x));
        let fnode = b.push(
            Some(node),
            NavRole::Function,
            &format!("function:{}", f.as_uuid()),
            label,
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

// ---------------------------------------------------------------------------------------------------------------
// Part 2: the nine further providers (§7.1)

/// A plain group node (no records, no cells): a potential, a strip heading, a section, an aspect, a manufacturer.
fn group(b: &mut Builder, parent: Option<u32>, role: NavRole, segment: &str, label: String) -> u32 {
    b.push(
        parent,
        role,
        segment,
        label,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
        vec![("kind".to_owned(), role.word().to_owned())],
    )
}

/// `kind:<role>`, then one `page:<folio>` per distinct folio of the drawables (sorted).
fn base_facets(role: NavRole, drawables: &[Drawable]) -> Vec<(String, String)> {
    let mut facets = vec![("kind".to_owned(), role.word().to_owned())];
    let mut pages: Vec<String> = drawables.iter().map(|x| x.folio.to_lowercase()).collect();
    pages.sort();
    pages.dedup();
    facets.extend(pages.into_iter().map(|p| ("page".to_owned(), p)));
    facets
}

/// The folio → page UUID of every page of the page order (drawn, divider and generated pages).
fn folio_pages(q: &Q) -> BTreeMap<String, Uuid> {
    let mut out = BTreeMap::new();
    for e in &q.get::<PageOrder>(()).entries {
        out.entry(e.folio.clone()).or_insert(super::page_uuid(e.page));
    }
    out
}

/// The drawables of a device (§6.1 item 6): the representations of its functions, main first, then page order, cell
/// row, cell column, ID.
fn device_drawables(
    q: &Q,
    folios: &BTreeMap<Id<Page>, (usize, String, Uuid)>,
    d: Id<Device>,
) -> Vec<Drawable> {
    let mut v: Vec<Keyed> = Vec::new();
    for f in q.idx().functions_of_device(d) {
        let main = q.functions().get(f).is_some_and(|x| x.role == FnRole::Main);
        v.extend(
            q.idx()
                .reps_of_function(f)
                .filter_map(|r| drawable(q, folios, r, main)),
        );
    }
    sorted(v)
}

/// The drawable of a connection: its page, and the cell of its first end that is a CP drawn on that page (else of
/// its second end, else none). Every connection drawable counts as main.
fn connection_drawable(
    q: &Q,
    folios: &BTreeMap<Id<Page>, (usize, String, Uuid)>,
    c: Id<ew_derive::ew_schema::Connection>,
) -> Option<Keyed> {
    use ew_derive::ew_schema::Endpoint;
    let con = q.connections().get(c)?;
    let page = con.page?;
    let (pos, folio, page_uuid) = folios.get(&page)?.clone();
    let cell = [&con.a, &con.b].into_iter().find_map(|e| match e {
        Endpoint::Cp(f, idx) => ew_derive::cp_cell(q, *f, *idx, page),
        _ => None,
    });
    let (row, col, text) = cell.map_or((u32::MAX, u32::MAX, String::new()), |c| {
        (c.row, c.column, c.to_string())
    });
    Some((
        (false, pos, row, col, c.as_uuid()),
        Drawable {
            page: page_uuid,
            folio,
            cell: text,
            record: c.as_uuid(),
            main: true,
        },
    ))
}

/// The drawables of a list of connections, sorted.
fn connection_drawables(
    q: &Q,
    folios: &BTreeMap<Id<Page>, (usize, String, Uuid)>,
    cs: &[Id<ew_derive::ew_schema::Connection>],
) -> Vec<Drawable> {
    sorted(
        cs.iter()
            .filter_map(|c| connection_drawable(q, folios, *c))
            .collect(),
    )
}

/// The first UUID of a report row's ID.
fn row_entity(id: &ew_reports::RowId, n: usize) -> Option<Uuid> {
    id.entities().nth(n)
}

/// A report row's ID as a key segment: its text form with `/` read as `,` (the text form escapes a `/` inside a
/// name, so the segment never holds one).
fn row_segment(id: &ew_reports::RowId) -> String {
    id.to_string().replace('/', ",")
}

/// The cell text of `key` in a row's texts.
fn cell_of<'a>(set: &ew_reports::RowSet, texts: &'a [String], key: &str) -> &'a str {
    set.column(key)
        .and_then(|c| texts.get(c))
        .map_or("", String::as_str)
}

/// One data row of the wire list as the Connections and Potentials navigators show it.
struct WireListRow {
    /// The row ID's key segment.
    segment: String,
    /// The label: the wire number, else `<from> <from CP> → <to> <to CP>`.
    label: String,
    /// The cells.
    cells: Vec<String>,
    /// The connections the row stands for.
    records: Vec<Uuid>,
    /// Those that are drawn.
    drawables: Vec<Drawable>,
    /// The potential cell.
    potential: String,
    /// Whether it is a bridge row (after the "Bridges" heading).
    bridge: bool,
}

/// The wire list's data rows (§7.1, Connections): a physical-wire row stands for the constituent connections of its
/// physical wire (`physical_wires_of` of every potential), a bridge row for the bridge connections of its strip and
/// group.
fn wire_list_rows(q: &Q) -> Vec<WireListRow> {
    use ew_derive::ew_schema::{Connection, ConnectionKind, Endpoint};
    use ew_derive::{PhysicalWiresOf, PotentialPartition};
    let Some(set) = ew_reports::project_rows(q, "wire_list", None) else {
        return Vec::new();
    };
    let folios = folios(q);
    let partition = q.get::<PotentialPartition>(());
    let mut constituents: BTreeMap<Uuid, Vec<Id<Connection>>> = BTreeMap::new();
    for p in partition.potentials() {
        for w in &q.get::<PhysicalWiresOf>(p.id).wires {
            constituents
                .entry(w.id.as_uuid())
                .or_insert_with(|| w.constituents.clone());
        }
    }
    let strip_of = |e: &Endpoint| -> Option<Id<Device>> {
        let Endpoint::Cp(f, _) = e else { return None };
        let d = q.functions().get(*f)?.device?;
        q.devices().get(d)?.aspects.terminal.as_ref().map(|t| t.strip)
    };
    let mut bridges: BTreeMap<(Uuid, u32), Vec<Id<Connection>>> = BTreeMap::new();
    for (_, c) in q.connections().iter() {
        let ConnectionKind::Bridge(spec) = &c.kind else {
            continue;
        };
        if let Some(strip) = strip_of(&c.a).or_else(|| strip_of(&c.b)) {
            bridges
                .entry((strip.as_uuid(), u32::from(spec.group)))
                .or_default()
                .push(c.id);
        }
    }
    let mut out = Vec::new();
    let mut after_heading = false;
    for (i, r) in set.rows.iter().enumerate() {
        if r.kind != ew_reports::RowKind::Data {
            after_heading = true;
            continue;
        }
        let texts = ew_reports::csv::row_texts(q, &set, i);
        let mut cons: Vec<Id<Connection>> = if after_heading {
            let group = r.id.0.iter().find_map(|k| match k {
                ew_reports::RowKey::Index(g) => Some(*g),
                _ => None,
            });
            match (row_entity(&r.id, 0), group) {
                (Some(strip), Some(g)) => bridges.get(&(strip, g)).cloned().unwrap_or_default(),
                _ => Vec::new(),
            }
        } else {
            row_entity(&r.id, 0)
                .and_then(|w| constituents.get(&w).cloned())
                .unwrap_or_default()
        };
        cons.sort();
        cons.dedup();
        let number = cell_of(&set, &texts, "no");
        let label = if number.is_empty() {
            format!(
                "{} {} \u{2192} {} {}",
                cell_of(&set, &texts, "from"),
                cell_of(&set, &texts, "from_cp"),
                cell_of(&set, &texts, "to"),
                cell_of(&set, &texts, "to_cp")
            )
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
        } else {
            number.to_owned()
        };
        out.push(WireListRow {
            segment: row_segment(&r.id),
            label,
            potential: cell_of(&set, &texts, "potential").to_owned(),
            records: cons.iter().map(|c| c.as_uuid()).collect(),
            drawables: connection_drawables(q, &folios, &cons),
            cells: texts,
            bridge: after_heading,
        });
    }
    out
}

/// Pushes one wire-list row as a connection node.
fn push_wire_row(b: &mut Builder, parent: u32, w: &WireListRow, table_row: bool) -> u32 {
    let mut facets = base_facets(NavRole::Connection, &w.drawables);
    for p in w
        .potential
        .split('\u{21C4}')
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        facets.push(("potential".to_owned(), p.to_lowercase()));
    }
    b.push(
        Some(parent),
        NavRole::Connection,
        &format!("connection:{}", w.segment),
        w.label.clone(),
        Vec::new(),
        w.records.clone(),
        w.drawables.clone(),
        table_row.then(|| w.cells.clone()),
        facets,
    )
}

/// Connections (§7.1): potential → physical-wire rows; a `Bridges` group → the bridge rows.
fn connections_table(q: &Q, b: &mut Builder) {
    let rows = wire_list_rows(q);
    let mut groups: BTreeMap<String, u32> = BTreeMap::new();
    let mut bridges: Option<u32> = None;
    for w in &rows {
        let parent = if w.bridge {
            *bridges.get_or_insert_with(|| {
                group(b, None, NavRole::Section, "section:Bridges", "Bridges".to_owned())
            })
        } else {
            let name = if w.potential.is_empty() {
                "(none)".to_owned()
            } else {
                w.potential.clone()
            };
            match groups.get(&name) {
                Some(g) => *g,
                None => {
                    let g = group(
                        b,
                        None,
                        NavRole::Potential,
                        &format!("potential:{name}"),
                        name.clone(),
                    );
                    groups.insert(name, g);
                    g
                }
            }
        };
        push_wire_row(b, parent, w, true);
    }
}

/// The strips of the project in natural tag order.
fn strips(q: &Q) -> Vec<Id<Device>> {
    let mut v: Vec<(NaturalKey, Id<Device>)> = q
        .devices()
        .ids()
        .filter(|d| q.devices().get(*d).is_some_and(|x| x.aspects.strip.is_some()))
        .map(|d| (natural_key(&short_tag(q, d)), d))
        .collect();
    v.sort();
    v.into_iter().map(|(_, d)| d).collect()
}

/// A device's short tag.
fn short_tag(q: &Q, d: Id<Device>) -> String {
    q.get::<DeviceTag>(d)
        .as_ref()
        .as_ref()
        .map(ew_derive::Tag::short)
        .unwrap_or_default()
}

/// Terminal strips (§7.1; WP-S2.07 D2, D3): strip → its rows in mounting order: the accessories placed before the
/// first terminal, then each terminal (from the strip's terminal diagram) followed by the accessories placed after it.
/// The table has a leading `strip` column, `pos` (the terminal's `seq`, an accessory's `<after_seq>.<k>`) and
/// `bridge_part` (the part number of the terminal's bridge, D6). Every node of a strip carries the facet `strip` (the
/// case-folded strip tag).
fn strips_table(q: &Q, b: &mut Builder) {
    let folios = folios(q);
    // The bridge parts by terminal (D6): the part number of the group's part, when it has one.
    let mut bridge_parts: BTreeMap<Id<Device>, String> = BTreeMap::new();
    for (_, _, g) in ew_reports::bridge_groups(q) {
        let number = g
            .part
            .as_ref()
            .and_then(|p| q.parts().get(p.copy))
            .map(|c| c.item.part_number.as_str().to_owned())
            .unwrap_or_default();
        for t in &g.terminals {
            bridge_parts.entry(*t).or_insert_with(|| number.clone());
        }
    }
    for strip in strips(q) {
        let Some(set) = ew_reports::project_rows(q, "terminal_diagram", Some(strip.as_uuid())) else {
            continue;
        };
        let tag = short_tag(q, strip);
        let strip_facet = ("strip".to_owned(), tag.to_lowercase());
        let mut terminals = Vec::new();
        for (i, r) in set.rows.iter().enumerate() {
            if r.kind != ew_reports::RowKind::Data {
                continue;
            }
            let Some(t) = row_entity(&r.id, 1) else { continue };
            terminals.push((i, t));
        }
        let spare = |t: Uuid| {
            q.devices()
                .get(Id::from_uuid(t))
                .is_some_and(|d| d.state == DeviceState::Spare)
        };
        let n_spare = terminals.iter().filter(|(_, t)| spare(*t)).count();
        let drawables = device_drawables(q, &folios, strip);
        // The strip's own drawables are its strip segments.
        let strip_drawn = !drawables.is_empty();
        let mut badges = Vec::new();
        if n_spare > 0 {
            badges.push(format!("{n_spare} spare"));
        }
        let mut facets = base_facets(NavRole::Strip, &drawables);
        facets.push(strip_facet.clone());
        let snode = b.push(
            None,
            NavRole::Strip,
            &format!("strip:{}", strip.as_uuid()),
            tag.clone(),
            badges,
            vec![strip.as_uuid()],
            drawables,
            None,
            facets,
        );
        // D3: the accessories in (after_seq, list index) order, each with its `pos` and the index of the terminal
        // it follows (`None`: before the first terminal).
        let seqs: Vec<u32> = terminals
            .iter()
            .map(|(_, t)| {
                q.devices()
                    .get(Id::from_uuid(*t))
                    .and_then(|d| d.aspects.terminal.as_ref())
                    .map_or(0, |a| a.seq)
            })
            .collect();
        let accessories = accessory_rows(q, strip, &seqs);
        let mut next_accessory = 0;
        let mut push_accessories = |b: &mut Builder, after: Option<usize>| {
            while let Some(a) = accessories.get(next_accessory)
                && a.after == after
            {
                let cells = vec![
                    tag.clone(),
                    a.pos.clone(),
                    String::new(),
                    a.word.to_owned(),
                    String::new(),
                    a.part.clone(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                ];
                let mut facets = vec![("kind".to_owned(), "accessory".to_owned()), strip_facet.clone()];
                if !a.part.is_empty() {
                    facets.push(("part".to_owned(), a.part.to_lowercase()));
                }
                b.push(
                    Some(snode),
                    NavRole::Accessory,
                    &format!("accessory:{}", a.index),
                    a.word.to_owned(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Some(cells),
                    facets,
                );
                next_accessory += 1;
            }
        };
        push_accessories(b, None);
        // `bridge_part` follows `bridge`.
        let bridge_part_at = set.column("bridge").map_or(usize::MAX, |c| c + 1);
        for (k, (i, t)) in terminals.into_iter().enumerate() {
            let texts = ew_reports::csv::row_texts(q, &set, i);
            let id: Id<Device> = Id::from_uuid(t);
            let drawables = device_drawables(q, &folios, id);
            let mut facets = base_facets(NavRole::Terminal, &drawables);
            facets.push(strip_facet.clone());
            let mut badges = Vec::new();
            // §6.8: a terminal with no representation of its own on a strip that no strip segment draws is unplaced
            // (a strip's segments draw all its terminals).
            if drawables.is_empty() && !strip_drawn {
                badges.push("unplaced".to_owned());
                facets.push(("unplaced".to_owned(), String::new()));
            }
            if let Some(d) = q.devices().get(id) {
                if d.state == DeviceState::Spare {
                    badges.push("spare".to_owned());
                }
                facets.push(("state".to_owned(), state_word(&d.state).to_owned()));
            }
            let potential = cell_of(&set, &texts, "potential");
            if !potential.is_empty() {
                facets.push(("potential".to_owned(), potential.to_lowercase()));
            }
            let part = cell_of(&set, &texts, "part");
            if !part.is_empty() {
                facets.push(("part".to_owned(), part.to_lowercase()));
            }
            let label = cell_of(&set, &texts, "terminal").to_owned();
            let mut cells = vec![tag.clone(), seqs[k].to_string()];
            let bridge_part = bridge_parts.get(&id).cloned().unwrap_or_default();
            let n = texts.len();
            for (c, text) in texts.into_iter().enumerate() {
                if c == bridge_part_at {
                    cells.push(bridge_part.clone());
                }
                cells.push(text);
            }
            if bridge_part_at >= n {
                cells.push(bridge_part);
            }
            b.push(
                Some(snode),
                NavRole::Terminal,
                &format!("terminal:{t}"),
                label,
                badges,
                vec![t],
                drawables,
                Some(cells),
                facets,
            );
            push_accessories(b, Some(k));
        }
    }
}

/// One accessory row of a strip (WP-S2.07 D3).
struct AccessoryRow {
    /// Its index in `Strip.accessories`.
    index: usize,
    /// The index of the terminal (in mounting order) it follows; `None` before the first terminal.
    after: Option<usize>,
    /// `<after_seq>.<k>`.
    pos: String,
    /// The kind word (`End bracket`).
    word: &'static str,
    /// The part number, empty when the copy is missing.
    part: String,
}

/// The accessory rows of `strip` over its terminals' `seqs` (mounting order): each placed after the terminal whose
/// `seq` is the greatest not above its `after_seq` (before the first terminal when none is), in (after_seq, list
/// index) order; `k` counts from 1 among equal `after_seq` in list order.
fn accessory_rows(q: &Q, strip: Id<Device>, seqs: &[u32]) -> Vec<AccessoryRow> {
    let Some(aspect) = q.devices().get(strip).and_then(|d| d.aspects.strip.clone()) else {
        return Vec::new();
    };
    let mut order: Vec<(u32, usize)> = aspect
        .accessories
        .iter()
        .enumerate()
        .map(|(i, a)| (a.after_seq, i))
        .collect();
    order.sort_unstable();
    let mut out = Vec::with_capacity(order.len());
    let mut k = 0u32;
    let mut last: Option<u32> = None;
    for (after_seq, index) in order {
        k = if last == Some(after_seq) { k + 1 } else { 1 };
        last = Some(after_seq);
        let a = &aspect.accessories[index];
        let after = seqs.iter().rposition(|s| *s <= after_seq);
        let part = q
            .parts()
            .get(a.part.copy)
            .map(|c| c.item.part_number.as_str().to_owned())
            .unwrap_or_default();
        out.push(AccessoryRow {
            index,
            after,
            pos: format!("{after_seq}.{k}"),
            word: crate::output::strip_edit::accessory_word(&a.kind),
            part,
        });
    }
    // Stable by placement, so that the rows follow the terminals in one pass.
    out.sort_by_key(|a| a.after.map_or(0, |x| x + 1));
    out
}

// Cables (§7.1): the per-cable rows of the cable overview as cable rows, the core rows as conductor rows under them;
/// the headings are dropped.
fn cables_table(q: &Q, b: &mut Builder) {
    use ew_derive::ew_schema::{Conductor, ConnectionKind};
    let Some(set) = ew_reports::project_rows(q, "cable_overview_diagrams", None) else {
        return;
    };
    let folios = folios(q);
    let partition = q.get::<ew_derive::PotentialPartition>(());
    let mut cables: BTreeMap<Uuid, u32> = BTreeMap::new();
    for (i, r) in set.rows.iter().enumerate() {
        if r.kind != ew_reports::RowKind::Data {
            continue;
        }
        let Some(cable) = row_entity(&r.id, 0) else {
            continue;
        };
        let texts = ew_reports::csv::row_texts(q, &set, i);
        let part = cell_of(&set, &texts, "part").to_lowercase();
        match row_entity(&r.id, 1) {
            None => {
                let drawables = device_drawables(q, &folios, Id::from_uuid(cable));
                let mut facets = base_facets(NavRole::Cable, &drawables);
                if !part.is_empty() {
                    facets.push(("part".to_owned(), part));
                }
                let label = cell_of(&set, &texts, "cable").to_owned();
                let mut texts = texts;
                texts.extend([String::new(), String::new(), String::new()]);
                let n = b.push(
                    None,
                    NavRole::Cable,
                    &format!("cable:{cable}"),
                    label,
                    Vec::new(),
                    vec![cable],
                    drawables,
                    Some(texts),
                    facets,
                );
                cables.insert(cable, n);
            }
            Some(core) => {
                let Some(&parent) = cables.get(&cable) else {
                    continue;
                };
                let cid: Id<Conductor> = Id::from_uuid(core);
                let mut cons: Vec<Id<ew_derive::ew_schema::Connection>> = q
                    .connections()
                    .iter()
                    .filter(|(_, c)| matches!(c.kind, ConnectionKind::Core(x) if x == cid))
                    .map(|(_, c)| c.id)
                    .collect();
                cons.sort();
                let drawables = connection_drawables(q, &folios, &cons);
                let facets = base_facets(NavRole::Conductor, &drawables);
                // The potential of the core's wires (the first end of its first connection that has one), its shield
                // flag and pair.
                let potential = cons
                    .iter()
                    .filter_map(|c| q.connections().get(*c))
                    .flat_map(|c| [&c.a, &c.b])
                    .find_map(|e| {
                        let key = ew_derive::EndpointKey::of(e)?;
                        let p = partition.potential_at(key)?;
                        Some(q.get::<ew_derive::PotentialNameOf>(p.id).name.text.clone())
                    })
                    .unwrap_or_default();
                let conductor = q.conductors().get(cid);
                let shield = conductor.map_or("", |c| if c.shield { "yes" } else { "no" });
                let pair = conductor
                    .and_then(|c| c.pair)
                    .map_or_else(String::new, |p| p.to_string());
                let label = cell_of(&set, &texts, "core").to_owned();
                let mut texts = texts;
                texts.extend([potential, shield.to_owned(), pair]);
                b.push(
                    Some(parent),
                    NavRole::Conductor,
                    &format!("conductor:{core}"),
                    label,
                    Vec::new(),
                    vec![cable, core],
                    drawables,
                    Some(texts),
                    facets,
                );
            }
        }
    }
}

/// PLC (§7.1): station → module → channel rows of the PLC overview.
fn plc_table(q: &Q, b: &mut Builder) {
    use ew_derive::PlcRows;
    let Some(set) = ew_reports::project_rows(q, "plc_overview", None) else {
        return;
    };
    let folios = folios(q);
    // The channel → (module, representation) of every station's PLC rows (the rows the PLC overview lists).
    let mut channels: BTreeMap<Uuid, (Id<Device>, Option<Id<Representation>>)> = BTreeMap::new();
    for station in ew_derive::stations(q) {
        for r in q.get::<PlcRows>(station).iter() {
            channels.insert(r.channel.as_uuid(), (r.module, r.rep));
        }
    }
    let mut stations: BTreeMap<Option<Uuid>, u32> = BTreeMap::new();
    let mut modules: BTreeMap<Uuid, u32> = BTreeMap::new();
    for (i, r) in set.rows.iter().enumerate() {
        if r.kind != ew_reports::RowKind::Data {
            continue;
        }
        let Some(ch) = row_entity(&r.id, 0) else { continue };
        let Some(&(module, rep)) = channels.get(&ch) else {
            continue;
        };
        let texts = ew_reports::csv::row_texts(q, &set, i);
        // The station: the module device's parent with the station aspect, else none.
        let st = q
            .devices()
            .get(module)
            .and_then(|d| d.parent)
            .filter(|p| {
                q.devices()
                    .get(*p)
                    .is_some_and(|x| x.aspects.plc_station.is_some())
            })
            .map(|s| s.as_uuid());
        let snode = match stations.get(&st) {
            Some(n) => *n,
            None => {
                let n = match st {
                    Some(s) => {
                        let id: Id<Device> = Id::from_uuid(s);
                        let drawables = device_drawables(q, &folios, id);
                        let facets = base_facets(NavRole::Station, &drawables);
                        b.push(
                            None,
                            NavRole::Station,
                            &format!("station:{s}"),
                            short_tag(q, id),
                            Vec::new(),
                            vec![s],
                            drawables,
                            None,
                            facets,
                        )
                    }
                    None => group(b, None, NavRole::Station, "station:-", "(none)".to_owned()),
                };
                stations.insert(st, n);
                n
            }
        };
        let mnode = match modules.get(&module.as_uuid()) {
            Some(n) => *n,
            None => {
                let drawables = device_drawables(q, &folios, module);
                let facets = base_facets(NavRole::Module, &drawables);
                let n = b.push(
                    Some(snode),
                    NavRole::Module,
                    &format!("module:{}", module.as_uuid()),
                    short_tag(q, module),
                    Vec::new(),
                    vec![module.as_uuid()],
                    drawables,
                    None,
                    facets,
                );
                modules.insert(module.as_uuid(), n);
                n
            }
        };
        let state = cell_of(&set, &texts, "state").to_owned();
        let mut badges = Vec::new();
        let mut facets = vec![("kind".to_owned(), "channel".to_owned())];
        if state == "spare" {
            badges.push("spare".to_owned());
        }
        if ["used", "spare", "reserved", "not_fitted"].contains(&state.as_str()) {
            facets.push(("state".to_owned(), state));
        }
        let page = rep
            .and_then(|r| q.reps().get(r))
            .and_then(|r| folios.get(&r.page).cloned());
        if let Some((_, folio, _)) = &page {
            facets.push(("page".to_owned(), folio.to_lowercase()));
        }
        let address = cell_of(&set, &texts, "address");
        let label = if address.is_empty() {
            cell_of(&set, &texts, "channel").to_owned()
        } else {
            address.to_owned()
        };
        let n = b.push(
            Some(mnode),
            NavRole::Channel,
            &format!("channel:{ch}"),
            label,
            badges,
            vec![ch, module.as_uuid()],
            Vec::new(),
            Some(texts),
            facets,
        );
        b.nodes[n as usize].open = page.map(|(_, _, p)| NavOpen {
            page: p,
            select: Vec::new(),
        });
    }
}

/// Potentials (§7.1): the potential overview's rows, each with the physical-wire rows of the wire list whose potential
/// cell names it (a bond wire's `A ⇄ B` names both).
fn potentials_table(q: &Q, b: &mut Builder) {
    let Some(set) = ew_reports::project_rows(q, "potential_overview", None) else {
        return;
    };
    let wires = wire_list_rows(q);
    let pages = folio_pages(q);
    let partition = q.get::<ew_derive::PotentialPartition>(());
    for (i, r) in set.rows.iter().enumerate() {
        if r.kind != ew_reports::RowKind::Data {
            continue;
        }
        let Some(record) = row_entity(&r.id, 0) else {
            continue;
        };
        let texts = ew_reports::csv::row_texts(q, &set, i);
        let name = cell_of(&set, &texts, "potential").to_owned();
        let bond =
            r.id.0
                .iter()
                .any(|k| matches!(k, ew_reports::RowKey::Name(n) if n == "bond"));
        let mut facets = vec![("kind".to_owned(), "potential".to_owned())];
        let first = cell_of(&set, &texts, "pages")
            .split([',', '\u{2013}'])
            .map(str::trim)
            .find(|s| !s.is_empty())
            .map(str::to_owned);
        if !bond {
            facets.push(("potential".to_owned(), name.to_lowercase()));
        }
        if let Some(f) = &first {
            facets.push(("page".to_owned(), f.to_lowercase()));
        }
        let segment = format!("potential:{}", row_segment(&r.id));
        let n = b.push(
            None,
            NavRole::Potential,
            &segment,
            name.clone(),
            Vec::new(),
            vec![record],
            Vec::new(),
            Some(texts),
            facets,
        );
        // The overview's row entity: the bond, the potential's definition, or (a potential without one) its CP.
        let kind = if bond {
            "bond"
        } else if q.potential_defs().contains(Id::from_uuid(record)) {
            "potential_def"
        } else {
            "cp"
        };
        b.nodes[n as usize].kinds = vec![kind.to_owned()];
        if kind == "cp" {
            b.nodes[n as usize].define_at = partition
                .potentials()
                .flat_map(|p| p.cps.iter())
                .find(|cp| cp.id().as_uuid() == record)
                .map(|cp| (cp.function.as_uuid(), cp.idx));
        }
        b.nodes[n as usize].open = first.and_then(|f| pages.get(&f)).map(|p| NavOpen {
            page: *p,
            select: Vec::new(),
        });
        if bond {
            continue;
        }
        for w in wires
            .iter()
            .filter(|w| !w.bridge && w.potential.split('\u{21C4}').map(str::trim).any(|p| p == name))
        {
            push_wire_row(b, n, w, false);
        }
    }
}

/// Interruption points (§7.1): name → point rows, from the interruption-point pairing.
fn ips_table(q: &Q, b: &mut Builder) {
    use ew_derive::ew_schema::IpDirection;
    use ew_derive::{IpPairingQ, RepAnchorCell};
    let pairing = q.get::<IpPairingQ>(());
    let folios = folios(q);
    let page_cell = |rep: Option<Id<Representation>>| -> String {
        let Some(r) = rep.and_then(|r| q.reps().get(r)) else {
            return String::new();
        };
        let folio = folios.get(&r.page).map(|x| x.1.clone()).unwrap_or_default();
        match q.get::<RepAnchorCell>(r.id).as_ref() {
            Some(c) => format!("{folio}-{c}"),
            None => folio,
        }
    };
    let mut names: BTreeMap<String, u32> = BTreeMap::new();
    let mut points: Vec<(NaturalKey, &ew_derive::IpInfo)> =
        pairing.ips.values().map(|i| (natural_key(&i.name), i)).collect();
    points.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.function.cmp(&b.1.function)));
    for (_, info) in points {
        let gnode = match names.get(&info.name) {
            Some(n) => *n,
            None => {
                let n = group(
                    b,
                    None,
                    NavRole::Name,
                    &format!("name:{}", info.name),
                    info.name.clone(),
                );
                names.insert(info.name.clone(), n);
                n
            }
        };
        let partner = if info.external {
            "external".to_owned()
        } else {
            pairing
                .partners(info.function)
                .iter()
                .map(|p| page_cell(pairing.ips.get(p).and_then(|x| x.rep)))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let direction = match info.direction {
            IpDirection::Source => "source",
            IpDirection::Target => "target",
            IpDirection::Undirected => "undirected",
            IpDirection::Unknown(_) => "unknown",
        };
        let pc = page_cell(info.rep);
        let cells = vec![
            info.name.clone(),
            pc.clone(),
            direction.to_owned(),
            partner,
            if info.external { "yes" } else { "no" }.to_owned(),
        ];
        let drawables = sorted(
            info.rep
                .and_then(|r| drawable(q, &folios, r, true))
                .into_iter()
                .collect(),
        );
        let facets = base_facets(NavRole::Ip, &drawables);
        let badges = if info.external {
            vec!["external".to_owned()]
        } else {
            Vec::new()
        };
        b.push(
            Some(gnode),
            NavRole::Ip,
            &format!("ip:{}", info.function.as_uuid()),
            if pc.is_empty() { info.name.clone() } else { pc },
            badges,
            vec![info.function.as_uuid()],
            drawables,
            Some(cells),
            facets,
        );
    }
}

/// Structure identifiers (§7.1): aspect → structure nodes (nested by `parent`), the structure-node rows of the
/// structure overview with a `usage` column (the devices and pages that name the node). A node's aliases are filter
/// tokens (they are never printed, D-03).
fn structure_table(q: &Q, b: &mut Builder) {
    use ew_derive::ew_schema::{Open, StructureNode};
    use ew_derive::{DevicesByStructure, PageStructure};
    let Some(set) = ew_reports::project_rows(q, "structure_overview", None) else {
        return;
    };
    let by_device = q.get::<DevicesByStructure>(());
    let mut by_page: BTreeMap<Id<StructureNode>, usize> = BTreeMap::new();
    for p in q.pages().ids() {
        for s in q.get::<PageStructure>(p).slots.values() {
            *by_page.entry(s.node).or_default() += 1;
        }
    }
    // The structure-node rows, by node.
    let mut rows: Vec<(Id<StructureNode>, Vec<String>)> = Vec::new();
    for (i, r) in set.rows.iter().enumerate() {
        let Some(u) = row_entity(&r.id, 0) else { continue };
        let id: Id<StructureNode> = Id::from_uuid(u);
        if !q.structure_nodes().contains(id) {
            continue;
        }
        let mut texts = ew_reports::csv::row_texts(q, &set, i);
        let usage = by_device.get(&id).map_or(0, Vec::len) + by_page.get(&id).copied().unwrap_or(0);
        texts.push(usage.to_string());
        rows.push((id, texts));
    }
    let aspect_of = |id: Id<StructureNode>| match q.structure_nodes().get(id).map(|n| n.aspect.clone()) {
        Some(Open::Known(a)) => Some(a),
        _ => None,
    };
    let mut aspects: Vec<Option<Aspect>> = rows.iter().map(|(id, _)| aspect_of(*id)).collect();
    aspects.sort();
    aspects.dedup();
    // Aspects in declaration order, an unknown aspect last.
    aspects.sort_by_key(|a| (a.is_none(), *a));
    let mut placed: BTreeMap<Id<StructureNode>, u32> = BTreeMap::new();
    for aspect in aspects {
        let (role, word) = match aspect {
            Some(Aspect::Function) => (NavRole::Function, "function"),
            Some(Aspect::Location) => (NavRole::Location, "location"),
            Some(a) => (NavRole::StructureNode, a.as_str()),
            None => (NavRole::StructureNode, "unknown"),
        };
        let anode = group(b, None, role, &format!("aspect:{word}"), word.to_owned());
        let mine: Vec<&(Id<StructureNode>, Vec<String>)> =
            rows.iter().filter(|(id, _)| aspect_of(*id) == aspect).collect();
        // Parents first (by depth), then natural designation; a parent outside the aspect puts the node under it.
        let mut order = mine;
        order.sort_by_key(|(id, t)| {
            (
                depth(q, *id),
                natural_key(t.first().map_or("", String::as_str)),
                *id,
            )
        });
        for (id, texts) in order {
            let parent = q
                .structure_nodes()
                .get(*id)
                .and_then(|n| n.parent)
                .and_then(|p| placed.get(&p).copied())
                .unwrap_or(anode);
            let facets = vec![("kind".to_owned(), "structure_node".to_owned())];
            let n = b.push(
                Some(parent),
                NavRole::StructureNode,
                &format!("structure_node:{}", id.as_uuid()),
                texts.first().cloned().unwrap_or_default(),
                Vec::new(),
                vec![id.as_uuid()],
                Vec::new(),
                Some(texts.clone()),
                facets,
            );
            if let Some(node) = q.structure_nodes().get(*id) {
                let t = &mut b.nodes[n as usize].tokens;
                t.extend(node.aliases.iter().flat_map(|a| tokens(a.as_str())));
                t.sort();
                t.dedup();
            }
            placed.insert(*id, n);
        }
    }
}

/// The depth of a structure node (0 at the top), bounded by the table's size.
fn depth(q: &Q, id: Id<ew_derive::ew_schema::StructureNode>) -> usize {
    let mut d = 0;
    let mut at = q.structure_nodes().get(id).and_then(|n| n.parent);
    while let Some(p) = at {
        d += 1;
        if d > q.structure_nodes().len() {
            break;
        }
        at = q.structure_nodes().get(p).and_then(|n| n.parent);
    }
    d
}

/// Parts in use (§7.1): manufacturer → part (the parts summary's rows in component mode) → the devices it counts.
fn parts_table(q: &Q, b: &mut Builder) {
    let Some(set) = ew_reports::project_rows(q, "parts_summary", None) else {
        return;
    };
    let folios = folios(q);
    let mut makers: BTreeMap<String, u32> = BTreeMap::new();
    for (i, r) in set.rows.iter().enumerate() {
        if r.kind != ew_reports::RowKind::Data {
            continue;
        }
        let texts = ew_reports::csv::row_texts(q, &set, i);
        let maker = cell_of(&set, &texts, "manufacturer").to_owned();
        let maker = if maker.is_empty() {
            "(none)".to_owned()
        } else {
            maker
        };
        let mnode = match makers.get(&maker) {
            Some(n) => *n,
            None => {
                let n = group(
                    b,
                    None,
                    NavRole::Manufacturer,
                    &format!("manufacturer:{maker}"),
                    maker.clone(),
                );
                makers.insert(maker, n);
                n
            }
        };
        let devices: Vec<Id<Device>> = r.sources.iter().map(|s| Id::from_uuid(s.as_uuid())).collect();
        let number = cell_of(&set, &texts, "part_number").to_owned();
        let mut facets = vec![("kind".to_owned(), "part".to_owned())];
        if !number.is_empty() {
            facets.push(("part".to_owned(), number.to_lowercase()));
        }
        let pnode = b.push(
            Some(mnode),
            NavRole::Part,
            &format!("part:{}", row_segment(&r.id)),
            number.clone(),
            Vec::new(),
            devices.iter().map(|d| d.as_uuid()).collect(),
            Vec::new(),
            Some(texts),
            facets,
        );
        for d in devices {
            let drawables = device_drawables(q, &folios, d);
            let mut facets = base_facets(NavRole::Device, &drawables);
            if !number.is_empty() {
                facets.push(("part".to_owned(), number.to_lowercase()));
            }
            b.push(
                Some(pnode),
                NavRole::Device,
                &format!("device:{}", d.as_uuid()),
                short_tag(q, d),
                Vec::new(),
                vec![d.as_uuid()],
                drawables,
                None,
                facets,
            );
        }
    }
}

/// Report blocks (§7.1): section (by number, front matter first) → one row per report block with its title, kind,
/// first folio (an embedded block's page), section and number of generated pages.
fn blocks_table(q: &Q, b: &mut Builder) {
    use ew_derive::ew_schema::{ReportBlock, ReportSource, Section};
    let order = q.get::<PageOrder>(());
    // (sort key: section number, first page position, ID), cells, ID, open page, section.
    type BlockRow = (
        (Option<u16>, usize, Uuid),
        Vec<String>,
        Uuid,
        Option<Uuid>,
        Option<Id<Section>>,
    );
    let mut rows: Vec<BlockRow> = Vec::new();
    for id in q.report_blocks().ids() {
        let Some(block) = q.report_blocks().get(id) else {
            continue;
        };
        let block: &ReportBlock = block;
        let generated: Vec<(usize, &ew_derive::PageEntry)> = order
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| matches!(e.page, PageRef::Generated { block, .. } if block == id))
            .collect();
        let host = block.embedded.as_ref().and_then(|e| {
            order
                .entries
                .iter()
                .enumerate()
                .find(|(_, e2)| e2.page == PageRef::Drawn(e.page))
        });
        let first = generated.first().copied().or(host);
        let section = first.map_or(block.section, |(_, e)| e.section);
        let number = section.and_then(|s| q.sections().get(s)).map(|s| s.number);
        let kind = match &block.source {
            ReportSource::Kind(k) => ew_reports::kind_name(k).unwrap_or("unknown").to_owned(),
            ReportSource::SqlQuery(_) => "sql".to_owned(),
            _ => "unknown".to_owned(),
        };
        let title = super::view::neutral(&block.title);
        let title = if title.is_empty() { kind.clone() } else { title };
        let cells = vec![
            title,
            kind,
            first.map(|(_, e)| e.folio.clone()).unwrap_or_default(),
            number.map(|n| n.to_string()).unwrap_or_default(),
            generated.len().to_string(),
        ];
        let open = generated.first().map(|(_, e)| super::page_uuid(e.page));
        rows.push((
            (number, first.map_or(usize::MAX, |(i, _)| i), id.as_uuid()),
            cells,
            id.as_uuid(),
            open,
            section,
        ));
    }
    rows.sort_by_key(|r| r.0);
    let mut sections: BTreeMap<Option<u16>, u32> = BTreeMap::new();
    for ((number, _, _), cells, id, open, section) in rows {
        let snode = match sections.get(&number) {
            Some(n) => *n,
            None => {
                let (segment, label) = match (number, section.and_then(|s| q.sections().get(s))) {
                    (Some(n), Some(s)) => (
                        format!("section:{n}"),
                        format!("Section {n} - {}", super::view::neutral(&s.title)),
                    ),
                    _ => ("section:front".to_owned(), "Front matter".to_owned()),
                };
                let n = group(b, None, NavRole::Section, &segment, label);
                sections.insert(number, n);
                n
            }
        };
        let facets = vec![("kind".to_owned(), "report_block".to_owned())];
        let n = b.push(
            Some(snode),
            NavRole::ReportBlock,
            &format!("report_block:{id}"),
            cells[0].clone(),
            Vec::new(),
            vec![id],
            Vec::new(),
            Some(cells),
            facets,
        );
        // A paged block opens its first generated page with the block selected (R30 F9).
        b.nodes[n as usize].open = open.map(|p| NavOpen {
            page: p,
            select: vec![id],
        });
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Highlighting (§7.3, D14)

/// What a highlight shows (§4.2): a potential, named by a potential overview row's record, or the net of a connection.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum HighlightTarget {
    /// The potential whose ID (a definition's or a CP's) is this.
    Potential(Uuid),
    /// The net that contains this connection.
    Net(Uuid),
}

impl Session {
    /// The connection IDs a highlight draws (WP-S2.06 §4.2, §7.3), sorted: for a potential the connections with an end
    /// on it (by the potential partition; a bond's `via` counts for both of its potentials), for a connection the
    /// connections of its page-local net (the connection alone when it joins no net, a bridge). Empty for an unknown
    /// ID. Reads memoised queries only.
    pub fn highlight_members(&self, target: &HighlightTarget) -> Vec<Uuid> {
        use ew_derive::ew_schema::Connection;
        use ew_derive::{PageLocalNets, PotentialId, PotentialPartition};
        use ew_model::EndpointKey;
        let q = self.query();
        let mut out: Vec<Uuid> = match target {
            HighlightTarget::Potential(u) => {
                let partition = q.get::<PotentialPartition>(());
                let Some(p) = partition
                    .potentials()
                    .find(|p| match p.id {
                        PotentialId::Def(d) => d.as_uuid() == *u,
                        PotentialId::Cp(c) => c.as_uuid() == *u,
                    })
                    .map(|p| p.id)
                else {
                    return Vec::new();
                };
                q.connections()
                    .iter()
                    .filter(|(_, c)| {
                        [&c.a, &c.b].into_iter().any(|e| {
                            EndpointKey::of(e)
                                .and_then(|k| partition.potential_at(k))
                                .is_some_and(|x| x.id == p)
                        })
                    })
                    .map(|(_, c)| c.id.as_uuid())
                    .collect()
            }
            HighlightTarget::Net(u) => {
                let id: Id<Connection> = Id::from_uuid(*u);
                let Some(c) = q.connections().get(id) else {
                    return Vec::new();
                };
                let net = c.page.and_then(|page| {
                    q.get::<PageLocalNets>(page)
                        .nets
                        .iter()
                        .find(|n| n.connections.contains(&id))
                        .map(|n| n.connections.iter().map(|x| x.as_uuid()).collect::<Vec<_>>())
                });
                net.unwrap_or_else(|| vec![*u])
            }
        };
        out.sort();
        out.dedup();
        out
    }
}

// ---------------------------------------------------------------------------------------------------------------
// WP-S2.09 part 2: the message panel (§7.1, D8)

/// The columns of the message panel's table rows (§7.1), in order; `rule` is the key column.
const MESSAGE_COLUMNS: [&str; 7] = [
    "severity",
    "rule",
    "message",
    "object",
    "page_cell",
    "fix",
    "state",
];

/// The severities in the panel's group order, with the group labels.
const SEVERITY_GROUPS: [(crate::checks::Severity, &str); 3] = [
    (crate::checks::Severity::Error, "Errors"),
    (crate::checks::Severity::Warning, "Warnings"),
    (crate::checks::Severity::Info, "Infos"),
];

/// The word of a rule kind (`safety`, `conflict`, `completeness`, `hygiene`, `integrity`): the `kind` facet of a
/// finding row and the detail line's kind.
fn rule_kind_word(kind: crate::checks::RuleKind) -> &'static str {
    use crate::checks::RuleKind;
    match kind {
        RuleKind::Safety => "safety",
        RuleKind::Conflict => "conflict",
        RuleKind::Completeness => "completeness",
        RuleKind::Hygiene => "hygiene",
        RuleKind::Integrity => "integrity",
    }
}

/// The drawables of one finding target (§6.1 item 6 of WP-S2.06): a device the representations of its functions; a
/// function its representations; a representation and a connection themselves; a conductor its core connections;
/// every other record none.
fn target_drawables(
    q: &Q,
    folios: &BTreeMap<Id<Page>, (usize, String, Uuid)>,
    target: &crate::checks::Target,
) -> Vec<Drawable> {
    use ew_derive::ew_schema::{Conductor, ConnectionKind, Kind};
    let id = target.id.as_uuid();
    match target.kind {
        Kind::Device => device_drawables(q, folios, Id::from_uuid(id)),
        Kind::Function => {
            let f: Id<Function> = Id::from_uuid(id);
            let main = q.functions().get(f).is_none_or(|x| x.role == FnRole::Main);
            sorted(
                q.idx()
                    .reps_of_function(f)
                    .filter_map(|r| drawable(q, folios, r, main))
                    .collect(),
            )
        }
        Kind::Representation => sorted(drawable(q, folios, Id::from_uuid(id), true).into_iter().collect()),
        Kind::Connection => connection_drawables(q, folios, &[Id::from_uuid(id)]),
        Kind::Conductor => {
            let cid: Id<Conductor> = Id::from_uuid(id);
            let mut cons: Vec<Id<ew_derive::ew_schema::Connection>> = q
                .connections()
                .iter()
                .filter(|(_, c)| matches!(c.kind, ConnectionKind::Core(x) if x == cid))
                .map(|(_, c)| c.id)
                .collect();
            cons.sort();
            connection_drawables(q, folios, &cons)
        }
        _ => Vec::new(),
    }
}

/// Sets a node's first natural key to its rank (its index), so that the default label sort keeps the provider's order.
fn rank(b: &mut Builder, at: u32) {
    let r = natural_key(&at.to_string());
    if let Some(first) = b.nodes[at as usize].natural.first_mut() {
        *first = r;
    }
}

impl Session {
    /// The check scheme of the online checks (WP-S2.09 §4.2): the project's `active_scheme`
    /// ([`Session::project_check_scheme`] in [`crate::checks::CheckMode::Online`]), else `drafting`. The message panel
    /// and the property grid's header findings run it.
    pub fn online_scheme(&self) -> &'static crate::checks::Scheme {
        self.project_check_scheme(crate::checks::CheckMode::Online)
            .unwrap_or(&crate::checks::DRAFTING)
    }

    /// The message panel's tree (WP-S2.09 §7.1): severity groups (Error, Warning, Info; an empty one left out) → rule
    /// groups in rule-ID order (label `<rule ID> <title>`, badge the number of its findings) → the findings in the
    /// report's order (label `<subject>: <message>`, key `finding:<fingerprint>`), from the online scheme's run. A
    /// node's first natural key is its rank in this order, so that the default label sort shows the provider's order.
    fn messages_table(&self, q: &Q, b: &mut Builder) {
        use crate::checks::{self, CheckMode};
        let report = checks::run(q, self.online_scheme(), CheckMode::Online);
        let folios = folios(q);
        for (severity, label) in SEVERITY_GROUPS {
            let of_severity: Vec<&checks::Finding> = report
                .findings
                .iter()
                .filter(|f| f.severity == severity)
                .collect();
            if of_severity.is_empty() {
                continue;
            }
            let word = severity.name();
            let snode = group(
                b,
                None,
                NavRole::Severity,
                &format!("severity:{word}"),
                label.to_owned(),
            );
            rank(b, snode);
            let rules: std::collections::BTreeSet<&'static str> =
                of_severity.iter().map(|f| f.rule).collect();
            for rule in rules {
                let findings: Vec<&checks::Finding> =
                    of_severity.iter().copied().filter(|f| f.rule == rule).collect();
                let meta = checks::rule(rule).map(|r| r.meta());
                let rnode = b.push(
                    Some(snode),
                    NavRole::Rule,
                    &format!("rule:{rule}"),
                    match meta {
                        Some(m) if !m.title.is_empty() => format!("{rule} {}", m.title),
                        _ => rule.to_owned(),
                    },
                    vec![findings.len().to_string()],
                    Vec::new(),
                    Vec::new(),
                    None,
                    vec![("kind".to_owned(), NavRole::Rule.word().to_owned())],
                );
                rank(b, rnode);
                let kind = meta.map(|m| rule_kind_word(m.kind));
                for f in findings {
                    finding_node(q, b, &folios, rnode, f, kind);
                }
            }
        }
    }
}

/// The drawables of a finding's primary target among `candidates` (the place key of each target's first drawable,
/// its printed label, its drawables, in target order): the smallest place key, a tie broken on the label in natural
/// order and then by bytes (FT-588); the target order decides only between equal labels.
fn primary_drawables<K: Ord>(candidates: impl Iterator<Item = (K, String, Vec<Drawable>)>) -> Vec<Drawable> {
    candidates
        .map(|(key, label, ds)| (key, natural_key(&label), label, ds))
        .min_by(|a, b| (&a.0, &a.1, &a.2).cmp(&(&b.0, &b.1, &b.2)))
        .map(|(_, _, _, ds)| ds)
        .unwrap_or_default()
}

/// One finding row (§7.1): its records are the targets, its drawables theirs; its page is the finding's page, else the
/// page of the primary target's first drawable; Enter opens that page with the targets' drawables on it selected.
fn finding_node(
    q: &Q,
    b: &mut Builder,
    folios: &BTreeMap<Id<Page>, (usize, String, Uuid)>,
    parent: u32,
    f: &crate::checks::Finding,
    kind: Option<&'static str>,
) {
    let mut drawables = Vec::new();
    for t in &f.targets {
        drawables.extend(target_drawables(q, folios, t));
    }
    let mut seen = std::collections::BTreeSet::new();
    drawables.retain(|d| seen.insert(d.record));
    // The primary target is the one drawn first (main drawables first, then page order and cell), not the first in
    // target order: targets sort by ID, which is random in a new project, so a finding over several members (a
    // potential's) would open another page in another copy of the project.
    let positions: BTreeMap<Uuid, usize> = folios.values().map(|(pos, _, u)| (*u, *pos)).collect();
    let first_key = |ds: &[Drawable]| {
        ds.first().map(|d| {
            (
                !d.main,
                positions.get(&d.page).copied().unwrap_or(usize::MAX),
                natural_key(&d.cell),
            )
        })
    };
    // A tie (two targets drawn first in the same cell) breaks on the targets' printed labels in natural order
    // (FT-588), never on the target (ID) order.
    let label = |t: &crate::checks::Target| super::properties::order_key(q, t.kind, t.id.as_uuid(), None).0;
    let primary = primary_drawables(
        f.targets
            .iter()
            .map(|t| (target_drawables(q, folios, t), t))
            .filter(|(ds, _)| !ds.is_empty())
            .map(|(ds, t)| (first_key(&ds), label(t), ds)),
    );
    let page: Option<(String, Uuid)> = match f.page {
        Some(p) => folios.get(&p).map(|(_, folio, u)| (folio.clone(), *u)),
        None => primary.first().map(|d| (d.folio.clone(), d.page)),
    };
    let page_cell = match &page {
        Some((folio, u)) => primary
            .iter()
            .find(|d| d.page == *u)
            .map_or_else(|| folio.clone(), rep_label),
        None => String::new(),
    };
    let waived = matches!(f.state, crate::checks::FindingState::Waived { .. });
    let state = if waived { "waived" } else { "open" };
    let word = f.severity.name();
    let cells = vec![
        word.to_owned(),
        f.rule.to_owned(),
        f.message.clone(),
        f.subject.clone(),
        page_cell,
        f.fix.as_ref().map(|x| x.label.clone()).unwrap_or_default(),
        state.to_owned(),
    ];
    let mut badges = vec![word.to_owned()];
    if waived {
        badges.push("waived".to_owned());
    }
    if f.fix.is_some() {
        badges.push("fix".to_owned());
    }
    let mut facets = Vec::new();
    if let Some(k) = kind {
        facets.push(("kind".to_owned(), k.to_owned()));
    }
    if let Some((folio, _)) = &page {
        facets.push(("page".to_owned(), folio.to_lowercase()));
    }
    facets.push(("state".to_owned(), state.to_owned()));
    let label = if f.subject.is_empty() {
        f.message.clone()
    } else {
        format!("{}: {}", f.subject, f.message)
    };
    let open_page = page
        .as_ref()
        .map(|(_, u)| *u)
        .or_else(|| drawables.first().map(|d| d.page));
    let records: Vec<Uuid> = f.targets.iter().map(|t| t.id.as_uuid()).collect();
    let select = |p: Uuid| -> Vec<Uuid> {
        drawables
            .iter()
            .filter(|d| d.page == p)
            .map(|d| d.record)
            .collect()
    };
    let open = open_page.map(|p| NavOpen {
        page: p,
        select: select(p),
    });
    let at = b.push(
        Some(parent),
        NavRole::Finding,
        &format!("finding:{}", f.fingerprint),
        label,
        badges,
        records,
        drawables,
        Some(cells),
        facets,
    );
    rank(b, at);
    b.nodes[at as usize].open = open;
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

    /// FT-588: two targets drawn first in the same cell; the primary is the one whose label prints first in natural
    /// order (`-K2` before `-K10`), in either target (ID) order.
    #[test]
    fn a_primary_target_tie_breaks_on_the_printed_label() {
        let drawable = |record: u128| Drawable {
            page: Uuid::from_u128(1),
            folio: "1".to_owned(),
            cell: "E4".to_owned(),
            record: Uuid::from_u128(record),
            main: true,
        };
        let key = (false, 0usize, natural_key("E4"));
        let k10 = (key.clone(), "-K10:1".to_owned(), vec![drawable(10)]);
        let k2 = (key.clone(), "-K2:1".to_owned(), vec![drawable(2)]);
        for order in [vec![k10.clone(), k2.clone()], vec![k2.clone(), k10.clone()]] {
            let p = primary_drawables(order.into_iter());
            assert_eq!(p[0].record, Uuid::from_u128(2));
        }
        // The place key still decides first.
        let earlier = (
            (false, 0usize, natural_key("A1")),
            "-K10:1".to_owned(),
            vec![drawable(10)],
        );
        let p = primary_drawables([k2, earlier].into_iter());
        assert_eq!(p[0].record, Uuid::from_u128(10));
        assert!(primary_drawables(std::iter::empty::<((), String, Vec<Drawable>)>()).is_empty());
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
                "blocks",
                "messages"
            ]
        );
        for k in NavKind::ALL {
            assert_eq!(NavKind::from_word(k.word()), Some(k));
        }
        assert_eq!(NavRole::StructureNode.word(), "structure_node");
        assert_eq!(key_label("page_cell"), "Page cell");
        assert_eq!(key_label("first_folio"), "First folio");
    }

    /// Whether a value type is a scalar of D10: Text, Ident, MlText, Bool, Int, Dec3, Quantity, Csa, Nm, Colour, Enum,
    /// or Optional of one.
    fn scalar(ty: &crate::output::FieldType) -> bool {
        use crate::output::FieldType as T;
        matches!(
            ty.inner(),
            T::Text
                | T::Ident
                | T::MlText
                | T::Bool
                | T::Int { .. }
                | T::Dec3
                | T::Quantity
                | T::Csa
                | T::Nm
                | T::Colour
                | T::Enum { .. }
        )
    }

    /// TB1 (D10, §8.2): each route of the editable-column table is an `EditRoute::FieldsSet` of `field_catalogue` with
    /// a scalar `FieldType`, and the catalogue's identifier flag (which table mode's policy reads) is set exactly for
    /// §8.2's identifier rows (the terminal designation, the potential's name and the structure node's code); every
    /// column of every navigator carries the route or the read-only reason of §8.2's precedence at column level
    /// ("derived" before "no command yet").
    #[test]
    fn tb1_the_editable_columns_are_scalar_fields_set_routes_of_the_catalogue() {
        use crate::output::{EditRoute, field_catalogue};
        let identifiers = [
            ("device", "terminal.designation"),
            ("potential_def", "name"),
            ("structure_node", "code"),
        ];
        for (nav, key, kind, path) in EDITABLE_COLUMNS {
            let catalogue = field_catalogue(kind).unwrap_or_else(|| panic!("{kind}: no catalogue"));
            let spec = catalogue
                .iter()
                .find(|f| f.path == path)
                .unwrap_or_else(|| panic!("{kind}.{path}: not in the catalogue"));
            assert!(
                matches!(&spec.route, EditRoute::FieldsSet { kind: k, .. } if k == kind),
                "{kind}.{path}: route {:?}",
                spec.route
            );
            // A cable's length source is JSON with its own typed editor (`parse_field_input`, WP-S2.07 §7).
            let typed = (kind, path) == ("device", "cable.length");
            assert!(scalar(&spec.ty) || typed, "{kind}.{path}: {}", spec.ty.word());
            assert_eq!(
                spec.identifier,
                identifiers.contains(&(kind, path)),
                "{kind}.{path}: identifier"
            );
            let col = columns_of(nav)
                .into_iter()
                .find(|c| c.key == key)
                .unwrap_or_else(|| panic!("{nav:?}: no column {key}"));
            assert_eq!(col.route, Some((kind.to_owned(), path.to_owned())));
            assert_eq!(col.read_only, None);
            assert!(!col.derived);
        }
        for nav in NavKind::ALL {
            for c in columns_of(nav) {
                let listed = EDITABLE_COLUMNS
                    .iter()
                    .any(|(n, k, _, _)| *n == nav && *k == c.key);
                match (listed, c.derived) {
                    (true, _) => assert!(c.route.is_some(), "{nav:?}.{}", c.key),
                    (false, true) => {
                        assert_eq!(c.read_only.as_deref(), Some("derived"), "{nav:?}.{}", c.key);
                        assert_eq!(c.route, None);
                    }
                    (false, false) => {
                        assert_eq!(
                            c.read_only.as_deref(),
                            Some("no command yet"),
                            "{nav:?}.{}",
                            c.key
                        );
                        assert_eq!(c.route, None);
                    }
                }
            }
        }
    }
}
