// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The navigators' tables: one row source per navigator,
//! [`Session::navigator_table`], a plain function over `Q` that the view model fetches once per (kind, revision) for
//! an open navigator only (no memoised family).
//!
//! **The table contract**. Nodes come in tree order, a parent before its children, siblings in the provider's
//! default order. **Keys** are path strings `<parent key>/<role word>:<id or text>` (no leading `/` at the top level):
//! a record node carries its UUID text, a group node its label (`function:=P01/location:+J-E-2416/device:<uuid>`); the
//! Pages keys stay as built (`s:<n>`, `s:front`, `…/p:<uuid>`). **Labels** are what the drawing prints (a device's
//! short tag, a folio, a representation's `<folio>-<cell>`); labels, cells and headings are texts, which the
//! reader renders ([`NavTable::render`]). **Badges** are fixed English words. **Tokens** are the rendered words of the
//! label and every cell, case-folded and NFC, split at whitespace, at `- . _ : / + = ( ) ,` and at every letter/digit
//! boundary ([`tokens`]). **Facets** come from the closed list `page`, `potential`, `state`, `kind`, `part`, and the
//! flags `unplaced` and `frozen`. **Drawables** are the representations (and, in part 2, connections) that draw a
//! node, main first, then page order, cell row, cell column, ID; [`NavTable::index`] maps every record and drawable ID
//! to the nodes that carry it, in node order.
//!
//! Part 1 built Pages and Devices; part 2 the nine further providers: Connections and Potentials from the wire
//! list and the potential overview, Terminal strips from each strip's terminal diagram, Cables from the cable
//! overview, PLC from the PLC overview, Parts in use from the parts summary in component mode, Structure identifiers
//! from the structure overview's node rows, and Interruption points and Report blocks with their own columns. A data
//! row without records (a heading) is never a table row. Part 2 also adds [`Session::highlight_members`].
//! PLC is the PLC navigator and grid: configuration project → station → module →
//! channel from `plc_rows(station)`, with the grid's own columns, typed editors and facets.

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::{PageOrder, PageRef, Q, RepAnchorCell};
use ew_ids::Id;
/// The record IDs of the tables, re-exported so that the view model names them without naming `ew-ids`.
pub use ew_ids::Uuid;
/// The naming schemes of derived IDs, re-exported so that the view model mints a draft's item IDs without naming
/// `ew-ids`.
pub use ew_ids::v5;
/// The natural sort key of a label or cell text, re-exported so that the view model sorts without naming `ew-mltext`.
pub use ew_mltext::NaturalKey;
use ew_mltext::{natural_key, to_nfc};
use ew_schema::{Device, DeviceState, FnRole, Page, Representation};

use crate::Session;

mod blocks;
mod bus;
mod cables;
mod columns;
mod connections;
mod devices;
mod fluid;
mod function_kind;
mod highlight;
mod ips;
pub(super) mod libraries;
mod messages;
mod pages;
mod parts;
mod plc;
mod potentials;
mod strips;
mod structure;
mod unplaced;
use blocks::blocks_table;
use bus::bus_table;
use cables::cables_table;
use columns::columns_of;
use connections::connections_table;
use devices::devices_table;
use fluid::{fluid_table, media_table, tubes_table};
pub(crate) use function_kind::{
    fndef_name, function_kind_or_key, function_kind_text, function_kind_words, kind_text, part_function_text,
};
pub use highlight::HighlightTarget;
use ips::ips_table;
pub use libraries::{LibraryCopyDetail, MacroDetail, MacroThumbnail, MacroVariable};
pub(crate) use messages::finding_page;
pub use messages::names_subject;
use parts::parts_table;
use plc::plc_table;
use potentials::potentials_table;
use strips::strips_table;
use structure::structure_table;
use unplaced::unplaced_table;

/// The navigators (adds the message panel last), in View ▸ Navigators order.
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
    /// The PLC grid: the PLC navigator's channels as a table in the bottom dock (doc 16 §2.6, the PLC workspace).
    PlcGrid,
    /// The bus networks: logical nets with their ports, physical nets with their derived segments (FR-UI-006).
    Bus,
    /// The fluid devices with their lines, and the media (FR-UI-006, doc 11 §7).
    Fluid,
    /// The tube table: one row per tube (doc 11 §7), in the bottom dock of the Fluid workspace.
    Tubes,
    /// The medium table: one row per medium (doc 11 §7), in the bottom dock of the Fluid workspace.
    Media,
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
    /// The unplaced devices of each layout space (doc 16 §5.2's row).
    Unplaced,
    /// The macros of every library tier and of the project (FR-MAC-006).
    Macros,
    /// The installed libraries and the project's library copies with their state.
    Libraries,
    /// The message panel: the findings of the project's online check scheme.
    Messages,
}

impl NavKind {
    /// The kinds in menu order.
    pub const ALL: [NavKind; 20] = [
        NavKind::Pages,
        NavKind::Devices,
        NavKind::Connections,
        NavKind::Strips,
        NavKind::Cables,
        NavKind::Plc,
        NavKind::PlcGrid,
        NavKind::Bus,
        NavKind::Fluid,
        NavKind::Tubes,
        NavKind::Media,
        NavKind::Potentials,
        NavKind::Ips,
        NavKind::Structure,
        NavKind::Parts,
        NavKind::Blocks,
        NavKind::Unplaced,
        NavKind::Macros,
        NavKind::Libraries,
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
            NavKind::PlcGrid => "plc_grid",
            NavKind::Bus => "bus",
            NavKind::Fluid => "fluid",
            NavKind::Tubes => "tubes",
            NavKind::Media => "media",
            NavKind::Potentials => "potentials",
            NavKind::Ips => "ips",
            NavKind::Structure => "structure",
            NavKind::Parts => "parts",
            NavKind::Blocks => "blocks",
            NavKind::Unplaced => "unplaced",
            NavKind::Macros => "macros",
            NavKind::Libraries => "libraries",
            NavKind::Messages => "messages",
        }
    }

    /// The kind of a word.
    pub fn from_word(w: &str) -> Option<NavKind> {
        NavKind::ALL.into_iter().find(|k| k.word() == w)
    }

    /// The navigator whose columns this one shows: the PLC grid the PLC navigator's (its rows too), the fluid navigator
    /// the tube table's (its table mode lists the tubes), every other its own.
    pub fn source(self) -> NavKind {
        match self {
            NavKind::PlcGrid => NavKind::Plc,
            NavKind::Fluid => NavKind::Tubes,
            k => k,
        }
    }

    /// Whether the navigator is a table of the bottom dock that opens in table mode (the PLC grid, the tube and medium
    /// tables).
    pub fn bottom_table(self) -> bool {
        matches!(self, NavKind::PlcGrid | NavKind::Tubes | NavKind::Media)
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
    /// A strip accessory: a row of the Terminal strips navigator.
    Accessory,
    /// A cable.
    Cable,
    /// A cable conductor.
    Conductor,
    /// A PLC configuration project: the top level of the PLC navigator.
    ConfigProject,
    /// A PLC station.
    Station,
    /// A rack of a PLC station.
    Rack,
    /// A PLC module.
    Module,
    /// A PLC channel.
    Channel,
    /// A bus port: a function with the `bus_port` aspect, under its station or in the bus navigator.
    BusPort,
    /// A logical or physical bus net.
    Net,
    /// A derived segment of a physical bus net.
    Segment,
    /// A fluid medium.
    Medium,
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
    /// A severity group of the message panel.
    Severity,
    /// A rule group of the message panel.
    Rule,
    /// A finding of the message panel.
    Finding,
    /// A library tier group of the Macros and Libraries navigators.
    Tier,
    /// An installed library.
    Library,
    /// The group of the project's own library copies ("In this project"), which is not a tier.
    ProjectCopies,
    /// A library copy the project holds.
    LibCopy,
    /// A macro.
    Macro,
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
            NavRole::ConfigProject => "plc_config_project",
            NavRole::Station => "station",
            NavRole::Rack => "rack",
            NavRole::Module => "module",
            NavRole::Channel => "channel",
            NavRole::BusPort => "bus_port",
            NavRole::Net => "net",
            NavRole::Segment => "segment",
            NavRole::Medium => "medium",
            NavRole::Name => "name",
            NavRole::Ip => "ip",
            NavRole::StructureNode => "structure_node",
            NavRole::Manufacturer => "manufacturer",
            NavRole::Part => "part",
            NavRole::ReportBlock => "report_block",
            NavRole::Severity => "severity",
            NavRole::Rule => "rule",
            NavRole::Finding => "finding",
            NavRole::Tier => "tier",
            NavRole::Library => "library",
            NavRole::ProjectCopies => "project_copies",
            NavRole::LibCopy => "library_copy",
            NavRole::Macro => "macro",
        }
    }
}

/// One column of a navigator's table, its heading a [`crate::Text`] as the session builds it and a string once rendered
/// ([`NavTable::render`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NavColumn<W = String> {
    /// The column key (`folio`, `designation`).
    pub key: String,
    /// The heading (`Folio`, `Designation`): the session's `column-…` words, or the list's heading for a column of a
    /// list kind.
    pub label: W,
    /// Whether the column is derived (drawn italic in table mode, and read-only for that reason).
    pub derived: bool,
    /// Whether it is the key column (frozen at the left in table mode).
    pub key_column: bool,
    /// The edit route `(kind, path)` of the edit mapping (doc 16 §7.2), `None` for a read-only column.
    pub route: Option<(String, String)>,
    /// A strip-editor column whose cells compile through `Session::strip_edit`; `None` elsewhere.
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

/// One node of a navigator's tree, its label and cells [`crate::Text`]s as the session builds them and strings once
/// rendered ([`NavTable::render`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NavNode<W = String> {
    /// The node's key, unique in the table and stable across revisions: the path of its segments from the top, a
    /// segment that its parent holds already numbered after it (`#2`, `#3`, in the order the rows are built). The
    /// window takes a row's identity from it.
    pub key: String,
    /// The index of the parent node, `None` at the top level.
    pub parent: Option<u32>,
    /// What the node is.
    pub role: NavRole,
    /// The printed label.
    pub label: W,
    /// The fixed badge words (`unplaced`, `frozen`, `spare`, `generated`, `QET grid`, `main`, `aux`, `external`).
    pub badges: Vec<String>,
    /// The records the row stands for.
    pub records: Vec<Uuid>,
    /// The records' kinds for edit routing, aligned with `records`; empty where the builder does not state them (not
    /// printed).
    pub kinds: Vec<String>,
    /// For a Potentials row of a potential known only at connection points (`kinds` is `cp`): the CP its record names
    /// (the function and the CP index), where a Name typed in table mode, with Type and Voltage if typed with it,
    /// defines it with `potential.define` (the G-PP triage's row 8, amending); `None` elsewhere
    /// (not printed). It carries no name: the definition takes the typed one.
    pub define_at: Option<(Uuid, u16)>,
    /// What draws them, main first, then page order, cell row, cell column, ID.
    pub drawables: Vec<Drawable>,
    /// What Enter opens.
    pub open: Option<NavOpen>,
    /// The cell texts, aligned with the table's columns, for a `table_row` node; empty otherwise.
    pub cells: Vec<W>,
    /// Whether the node is a row of the table.
    pub table_row: bool,
    /// The filter tokens: the rendered label's and cells', and those the session adds (a structure node's aliases),
    /// sorted and deduplicated ([`tokens`]).
    pub tokens: Vec<String>,
    /// The facets `(name, value)`; a flag has the value `""`.
    pub facets: Vec<(String, String)>,
    /// The natural keys of the rendered words: the label's, then one per cell. Before rendering, the label's key alone
    /// where the session ranks the node in its own order.
    pub natural: Vec<NaturalKey>,
}

/// The table of one navigator at one revision: its words [`crate::Text`]s as the session builds it
/// ([`Session::navigator_texts`]), strings once rendered ([`NavTable::render`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NavTable<W = String> {
    /// The navigator.
    pub kind: NavKind,
    /// The revision it was built at.
    pub rev: u64,
    /// The table's columns.
    pub columns: Vec<NavColumn<W>>,
    /// The nodes in tree order.
    pub nodes: Vec<NavNode<W>>,
    /// Every record and drawable ID to the nodes that carry it, in node order.
    pub index: BTreeMap<Uuid, Vec<u32>>,
}

impl NavTable<crate::Text> {
    /// The table in words: every heading, label and cell by `words` (the window's in its language, the command
    /// line's in English), with each node's filter tokens and natural keys of those words: a person filters and sorts
    /// what they see.
    pub fn render(self, words: impl Fn(&crate::Text) -> String) -> NavTable {
        let columns = (self.columns.into_iter())
            .map(|c| NavColumn {
                label: words(&c.label),
                key: c.key,
                derived: c.derived,
                key_column: c.key_column,
                route: c.route,
                strip_cell: c.strip_cell,
            })
            .collect();
        let nodes = (self.nodes.into_iter())
            .map(|n| {
                // A person filters and sorts what they see: the tokens and the sort keys of the rendered words.
                let label = words(&n.label);
                let cells: Vec<String> = n.cells.iter().map(&words).collect();
                let mut tokens = n.tokens;
                tokens.extend(node_tokens(&label, &cells));
                tokens.sort();
                tokens.dedup();
                let mut natural = n.natural;
                if natural.is_empty() {
                    natural.push(natural_key(&label));
                }
                natural.extend(cells.iter().map(|c| natural_key(c)));
                NavNode {
                    key: n.key,
                    parent: n.parent,
                    role: n.role,
                    label,
                    badges: n.badges,
                    records: n.records,
                    kinds: n.kinds,
                    define_at: n.define_at,
                    drawables: n.drawables,
                    open: n.open,
                    cells,
                    table_row: n.table_row,
                    tokens,
                    facets: n.facets,
                    natural,
                }
            })
            .collect();
        NavTable {
            kind: self.kind,
            rev: self.rev,
            columns,
            nodes,
            index: self.index,
        }
    }
}

/// A value of a navigator that no language changes (a tag, a folio, a list's cell as printed), as a text.
fn plain(value: impl Into<String>) -> crate::Text {
    crate::Text::bare(crate::TextArg::Plain(value.into()))
}

/// Values that no language changes, as texts ([`plain`]).
fn plains(values: Vec<String>) -> Vec<crate::Text> {
    values.into_iter().map(plain).collect()
}

/// The filter tokens of a text: case-folded and NFC, split at whitespace, at `- . _ : / + = ( ) ,` and
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

/// The tokens of a label and its cells.
fn node_tokens(label: &str, cells: &[String]) -> Vec<String> {
    std::iter::once(label)
        .chain(cells.iter().map(String::as_str))
        .flat_map(tokens)
        .collect()
}

/// The word of a device state (`used`, `spare`, `reserved`, `not_fitted`).
pub(super) fn state_word(s: &DeviceState) -> &'static str {
    match s {
        DeviceState::Spare => "spare",
        DeviceState::Reserved => "reserved",
        DeviceState::NotFitted => "not_fitted",
        _ => "used",
    }
}

/// A table under construction.
struct Builder {
    nodes: Vec<NavNode<crate::Text>>,
    /// The keys the table holds, each once ([`Builder::push`]).
    keys: BTreeSet<String>,
    /// Whether a measured number is written with a point whatever the project sets: the table the command line prints
    /// and the round-trip export reads back ([`Session::navigator_table`]); else as the project prints it.
    point: bool,
}

impl Builder {
    /// Appends a node and returns its index.
    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        parent: Option<u32>,
        role: NavRole,
        segment: &str,
        label: crate::Text,
        badges: Vec<String>,
        records: Vec<Uuid>,
        drawables: Vec<Drawable>,
        cells: Option<Vec<crate::Text>>,
        facets: Vec<(String, String)>,
    ) -> u32 {
        let path = match parent {
            Some(p) => format!("{}/{segment}", self.nodes[p as usize].key),
            None => segment.to_owned(),
        };
        // A key is the row's identity, in the table and in the window: a row whose segment another row of its parent
        // has already (two findings of one fingerprint: EW-FL-008's two open exhausts of one valve) is numbered after
        // it, so that no two rows share one.
        let mut key = path.clone();
        let mut n = 1;
        while !self.keys.insert(key.clone()) {
            n += 1;
            key = format!("{path}#{n}");
        }
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
            tokens: Vec::new(),
            natural: Vec::new(),
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
    fn finish(self, kind: NavKind, rev: u64) -> NavTable<crate::Text> {
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
    /// The table of navigator `kind` at the current revision: a plain function over `Q`,
    /// deterministic, reading memoised queries and `ew_reports::project_rows` only, for every kind of [`NavKind`].
    /// The Macros and Libraries tables also read [`Session::libraries`] as it is now: a library write
    /// changes them without a new revision, so the shell refetches them after one. The Messages table also reads the
    /// merge evidence (the manifest's `merged`, a git merge or rebase in progress, `--after-merge`): a
    /// change of that evidence without a new revision shows at the next revision.
    ///
    /// Its words are English and its measured numbers are written with a point whatever the project sets: the command
    /// line prints it and the round-trip export writes it and reads it back.
    pub fn navigator_table(&self, kind: NavKind) -> NavTable {
        self.texts_of(kind, true).render(ToString::to_string)
    }

    /// [`Session::navigator_table`] before its words are rendered: every heading, label and cell a [`crate::Text`],
    /// which the window renders in its language ([`NavTable::render`]); a measured number as the project prints it.
    pub fn navigator_texts(&self, kind: NavKind) -> NavTable<crate::Text> {
        self.texts_of(kind, false)
    }

    /// The table of `kind` in texts, a measured number written with a point when `point` ([`Builder::point`]).
    fn texts_of(&self, kind: NavKind, point: bool) -> NavTable<crate::Text> {
        let rev = self.world().rev.0;
        let mut b = Builder {
            nodes: Vec::new(),
            keys: BTreeSet::new(),
            point,
        };
        let q = self.query();
        match kind {
            NavKind::Pages => self.pages_table(&mut b),
            NavKind::Devices => devices_table(&q, &mut b),
            NavKind::Connections => connections_table(&q, &mut b),
            NavKind::Strips => strips_table(&q, &mut b),
            NavKind::Cables => cables_table(&q, &mut b),
            NavKind::Plc | NavKind::PlcGrid => plc_table(&q, &mut b),
            NavKind::Bus => bus_table(&q, &mut b),
            NavKind::Fluid => fluid_table(&q, &mut b),
            NavKind::Tubes => tubes_table(&q, &mut b),
            NavKind::Media => media_table(&q, &mut b),
            NavKind::Potentials => potentials_table(&q, &mut b),
            NavKind::Ips => ips_table(&q, &mut b),
            NavKind::Structure => structure_table(&q, &mut b),
            NavKind::Parts => parts_table(&q, &mut b),
            NavKind::Blocks => blocks_table(&q, &mut b),
            NavKind::Unplaced => unplaced_table(&q, &mut b),
            NavKind::Macros => self.macros_table(&q, &mut b),
            NavKind::Libraries => self.libraries_table(&q, &mut b),
            NavKind::Messages => {
                self.messages_table(&q, &mut b, self.online_scheme(), crate::checks::CheckMode::Online);
            }
        }
        b.finish(kind, rev)
    }
}

/// The columns of navigator `kind`'s table, their headings as texts: the headings of [`Session::navigator_texts`]
/// without building its rows (a heading pasted in another language).
pub fn navigator_columns(kind: NavKind) -> Vec<NavColumn<crate::Text>> {
    columns_of(kind)
}

/// The page order's folio and position of every drawn page.
pub(crate) fn folios(q: &Q) -> BTreeMap<Id<Page>, (usize, String, Uuid)> {
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

impl Drawable {
    /// The label of a representation row, and where a drawable sits (the Placed on column, a row menu's Go to):
    /// `<folio>-<cell>`, the folio alone without a cell.
    pub fn place(&self) -> String {
        if self.cell.is_empty() {
            self.folio.clone()
        } else {
            format!("{}-{}", self.folio, self.cell)
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The node helpers the providers share (each provider is its own file in `navigators/`)

/// A plain group node (no records, no cells): a potential, a strip heading, a section, an aspect, a manufacturer.
fn group(b: &mut Builder, parent: Option<u32>, role: NavRole, segment: &str, label: crate::Text) -> u32 {
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

/// The drawables of a device: the representations of its functions, main first, then page order, cell
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

/// Where a device without symbols of its own is drawn: a cable's cores (its wires), a strip's terminals; nothing for
/// any other device.
fn drawn_as_members(
    q: &Q,
    folios: &BTreeMap<Id<Page>, (usize, String, Uuid)>,
    dev: &Device,
) -> Vec<Drawable> {
    if dev.aspects.cable.is_some() {
        let cores: std::collections::BTreeSet<_> = q.idx().conductors_of_cable(dev.id).collect();
        let wires = q.connections().iter().filter_map(|(id, c)| match c.kind {
            ew_schema::ConnectionKind::Core(k) if cores.contains(&k) => Some(id),
            _ => None,
        });
        return sorted(wires.filter_map(|c| connection_drawable(q, folios, c)).collect());
    }
    if dev.aspects.strip.is_some() {
        let terminals = q
            .devices()
            .iter()
            .filter(|(_, t)| t.aspects.terminal.as_ref().is_some_and(|a| a.strip == dev.id))
            .map(|(id, _)| id);
        return terminals.flat_map(|t| device_drawables(q, folios, t)).collect();
    }
    Vec::new()
}

/// The drawable of a connection: its page, and the cell of its first end that is a CP drawn on that page (else of
/// its second end, else none). Every connection drawable counts as main.
fn connection_drawable(
    q: &Q,
    folios: &BTreeMap<Id<Page>, (usize, String, Uuid)>,
    c: Id<ew_schema::Connection>,
) -> Option<Keyed> {
    use ew_schema::Endpoint;
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
    cs: &[Id<ew_schema::Connection>],
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

/// The cells of row `i` of `set` as the list prints them without a language, except the columns `words`, whose cells
/// are the program's words (a device's kind, a terminal's state): those read in every language of the list texts.
fn report_cells(q: &Q, set: &ew_reports::RowSet, i: usize, words: &[&str]) -> Vec<crate::Text> {
    let worded: Vec<usize> = words.iter().filter_map(|k| set.column(k)).collect();
    let neutral = ew_reports::csv::row_texts(q, set, i);
    (neutral.into_iter().enumerate())
        .map(|(c, text)| match set.rows[i].cells.get(c) {
            Some(cell) if worded.contains(&c) => cell_words(q, cell),
            _ => plain(text),
        })
        .collect()
}

/// A list's cell in every language of the list texts, as a list printed in each prints it.
fn cell_words(q: &Q, cell: &ew_reports::Cell) -> crate::Text {
    let in_each = (ew_reports::catalogue().tags().into_iter()).filter_map(|tag| {
        let lang: ew_mltext::LangTag = tag.parse().ok()?;
        let opts = ew_reports::render::RenderOptions {
            lang: Some(lang.clone()),
            ..Default::default()
        };
        Some((lang, ew_reports::render::render(q, cell, &opts)))
    });
    crate::Text::bare(crate::TextArg::Ml(ew_mltext::MlText::from_langs(in_each)))
}

/// The words of the key `key` of the session's texts with a small first letter, in every language of its texts: a
/// choice's word as a cell reads it ("source", "bron").
fn lowered(key: &'static str) -> crate::Text {
    let catalogue = crate::catalogue();
    let text = key_text(key);
    let in_each = catalogue.tags().into_iter().filter_map(|tag| {
        let words = catalogue.text_or_english(&text, tag);
        let mut chars = words.chars();
        let lowered: String = chars.next().map(|c| c.to_lowercase().chain(chars).collect())?;
        Some((tag.parse().ok()?, lowered))
    });
    crate::Text::bare(crate::TextArg::Ml(ew_mltext::MlText::from_langs(in_each)))
}

/// The text of a key of the session's texts that the code names (`key!`).
fn key_text(key: &'static str) -> crate::Text {
    crate::Text::checked_by_macro(key, crate::text_english(key).unwrap_or("?"), Vec::new())
}

/// Words with a capital, unless their first word holds a digit ("3P-motor") or does not start with a lower-case
/// letter ("NO-contact", "PLC profile").
fn with_capital(words: &str) -> String {
    let first_word = words.split(char::is_whitespace).next().unwrap_or_default();
    let mut chars = words.chars();
    match chars.next() {
        Some(first) if first.is_lowercase() && !first_word.chars().any(|c| c.is_ascii_digit()) => {
            first.to_uppercase().chain(chars).collect()
        }
        _ => words.to_owned(),
    }
}

/// Yes or no, as a list prints them, in every language of the list texts.
fn yes_no(q: &Q, yes: bool) -> crate::Text {
    cell_words(q, &ew_reports::Cell::Bool(yes))
}

/// The cell of `key` among a row's cells as texts (an empty text when the set has no such column).
fn cell_text(set: &ew_reports::RowSet, cells: &[crate::Text], key: &str) -> crate::Text {
    set.column(key)
        .and_then(|c| cells.get(c))
        .cloned()
        .unwrap_or_else(|| plain(""))
}

/// The cell text of `key` in a row's texts.
fn cell_of<'a>(set: &ew_reports::RowSet, texts: &'a [String], key: &str) -> &'a str {
    set.column(key)
        .and_then(|c| texts.get(c))
        .map_or("", String::as_str)
}

/// The badge of a device row whose device no page draws (its tag shows `?` where the page goes,
/// and nothing said why); the window explains it on hover.
pub const NOT_DRAWN: &str = "not drawn";

/// The badge of a device row whose device is an earth connection (`ew_derive::layout::is_earth_connection`): drawn
/// with earth or ground symbols only and given no part, so no part is asked of it.
pub const EARTH_CONNECTION: &str = "earth connection";

/// The badge of a Messages rule group every finding of which has a quick fix of one kind: its Fix all.
pub const FIX_ALL: &str = "fix all";

#[cfg(test)]
mod tests {
    use super::columns::EDITABLE_COLUMNS;
    use super::messages::primary_drawables;
    use super::*;

    /// Two rows of one parent with one segment (two findings of one fingerprint) get keys of their own, the second
    /// numbered after the first, and a child of the second hangs under its key: the window takes a row's identity
    /// from its key.
    #[test]
    fn a_segment_given_twice_is_numbered_so_that_every_key_is_unique() {
        let mut b = Builder {
            nodes: Vec::new(),
            keys: BTreeSet::new(),
            point: false,
        };
        let push = |b: &mut Builder, parent, segment: &str| {
            let label = crate::Text::bare(crate::TextArg::Plain(segment.into()));
            b.push(
                parent,
                NavRole::Finding,
                segment,
                label,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                None,
                Vec::new(),
            )
        };
        let rule = push(&mut b, None, "rule:EW-FL-008");
        let first = push(&mut b, Some(rule), "finding:ab12");
        let second = push(&mut b, Some(rule), "finding:ab12");
        let child = push(&mut b, Some(second), "port");
        let keys: Vec<&str> = [first, second, child]
            .iter()
            .map(|i| b.nodes[*i as usize].key.as_str())
            .collect();
        assert_eq!(
            keys,
            [
                "rule:EW-FL-008/finding:ab12",
                "rule:EW-FL-008/finding:ab12#2",
                "rule:EW-FL-008/finding:ab12#2/port"
            ]
        );
    }

    /// A message that leads with its subject, or
    /// names it after a word (the layout rules' "Wire <wire> has no route", in either language), is shown without
    /// the subject a second time; another message, a longer tag that only starts like the subject and a subject
    /// glued to a word keep the prefix.
    #[test]
    fn a_message_naming_its_subject_shows_it_once() {
        let wire = "-G1:+ → -S1:1";
        for (subject, message, names) in [
            (
                "=P01+-201U1:DP",
                "=P01+-201U1:DP: the station address is TBD",
                true,
            ),
            ("-1K1", "-1K1 has no part", true),
            ("-1K1", "-1K10 has no part", false),
            ("-1K1", "the station address is TBD", false),
            (wire, "Wire -G1:+ → -S1:1 has no route: Route draws it.", true),
            (
                wire,
                "Draad -G1:+ → -S1:1 heeft geen route: Pagina routeren tekent hem.",
                true,
            ),
            ("folio 7", "A junction on folio 7 joins four wires.", true),
            ("-1K1", "the coil -1K1.", true),
            ("-1K1", "the coil -1K10 has no part", false),
            ("-1K1", "the coil x-1K1 has no part", false),
            ("", "Wire has no route", false),
        ] {
            assert_eq!(
                names_subject(subject, message),
                names,
                "{subject:?} in {message:?}"
            );
        }
    }

    #[test]
    fn tokens_split_at_separators_and_letter_digit_boundaries() {
        assert_eq!(tokens("-201U3"), ["201", "u", "3"]);
        assert_eq!(tokens("=P01 +J-E-2416"), ["p", "01", "j", "e", "2416"]);
        assert_eq!(tokens("Wire list (1),x_y"), ["wire", "list", "1", "x", "y"]);
        assert_eq!(tokens("  "), Vec::<String>::new());
        assert_eq!(tokens("Ä1"), ["ä", "1"]);
    }

    /// Two targets drawn first in the same cell; the primary is the one whose label prints first in natural
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

    /// kinds outside the contacts and coils read in plain words through the display mapping; the contacts and
    /// coils keep the generic rule. In Dutch the generic rule reads each category segment by its words, the CP
    /// designations after them kept.
    #[test]
    fn function_kinds_read_in_plain_words() {
        let def = |key: &str| {
            let item = ew_library::starter::item(key).expect(key);
            match ew_schema::LibDocument::from_json(item.json).expect(key) {
                ew_schema::LibDocument::Fndef(c) => c.item,
                _ => panic!("{key} is no FnDef"),
            }
        };
        let text = |key: &str, cps: &[&str]| {
            let cps: Vec<String> = cps.iter().map(|s| (*s).to_owned()).collect();
            kind_text(&def(key), &cps).expect(key)
        };
        assert_eq!(text("ew.fn.graphic", &[]), "Decoration");
        assert_eq!(text("ew.fn.ip", &["1"]), "Interruption point 1");
        assert_eq!(text("ew.fn.blackbox", &["13", "14", "24VDC"]), "Black box");
        assert_eq!(
            text("ew.fn.dp-connector", &["A1", "B1", "A2", "B2"]),
            "PROFIBUS connector A1-B1-A2-B2"
        );
        assert_eq!(text("ew.fn.mcb-1p", &["1", "2"]), "1P MCB 1-2");
        // The contacts and coils are unchanged.
        assert_eq!(text("ew.fn.contact-no", &["13", "14"]), "NO contact 13-14");
        assert_eq!(text("ew.fn.coil", &["A1", "A2"]), "Coil A1-A2");
        assert_eq!(
            text("ew.fn.contact-co", &["11", "12", "14"]),
            "Changeover contact 11-12-14",
            "sentence case"
        );
        let dutch = |key: &str, cps: &[&str]| crate::catalogue().text_or_english(&text(key, cps), "nl-NL");
        assert_eq!(
            dutch("ew.fn.contact-co", &["11", "12", "14"]),
            "Wisselcontact 11-12-14"
        );
        assert_eq!(dutch("ew.fn.contact-no", &["13", "14"]), "NO-contact 13-14");
        assert_eq!(dutch("ew.fn.coil", &["A1", "A2"]), "Spoel A1-A2");
        assert_eq!(dutch("ew.fn.silencer", &[]), "Geluiddemper");
        assert_eq!(dutch("ew.fn.valve-5-2-sol-spring", &["14"]), "5/2-ventiel 14");
        assert_eq!(
            dutch("ew.fn.mcb-2p", &["1", "2", "3", "4"]),
            "2P-installatieautomaat 1-2-3-4"
        );
        assert_eq!(dutch("ew.fn.ip-fluid", &[]), "Pneumatisch onderbrekingspunt");
        assert_eq!(
            dutch("ew.fn.cylinder-single-acting", &[]),
            "Enkelwerkende cilinder"
        );
        assert_eq!(dutch("ew.fn.regulator-gauge", &[]), "Manometerdrukregelaar");
        assert_eq!(dutch("ew.fn.ip", &["1"]), "Onderbrekingspunt 1");
        // One space between the words and the CP designations.
        assert_eq!(
            dutch("ew.fn.cylinder-double-acting-cushioned", &["A", "B"]),
            "Gedempte dubbelwerkende A-B"
        );
    }

    #[test]
    fn kinds_and_roles_have_their_words() {
        for k in NavKind::ALL {
            assert_eq!(NavKind::from_word(k.word()), Some(k));
        }
        assert_eq!(NavRole::StructureNode.word(), "structure_node");
    }

    /// Whether a value type is a scalar: Text, Ident, MlText, Bool, Int, Dec3, Quantity, Csa, Nm, Colour, Enum,
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

    /// TB1: each route of the editable-column table is an `EditRoute::FieldsSet` of `field_catalogue` with
    /// a scalar `FieldType`, and the catalogue's identifier flag (which table mode's policy reads) is set exactly for
    /// the identifier rows (the terminal designation, the potential's name and the structure node's code); a column of
    /// any other navigator has no route.
    #[test]
    fn the_editable_columns_are_scalar_fields_set_routes_of_the_catalogue() {
        use crate::output::{EditRoute, field_catalogue};
        let identifiers = [
            ("device", "terminal.designation"),
            ("potential_def", "name"),
            ("structure_node", "code"),
            // A module's slot (the catalogue's identifier flag, FR-UI-044 rule 2).
            ("device", "plc_module.slot"),
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
            // A cable's length source is JSON with its own typed editor (`parse_field_input`).
            // The PLC grid's address, target and slot have theirs too.
            let typed = [
                ("device", "cable.length"),
                ("channel", "address"),
                ("channel", "target"),
                ("device", "plc_module.slot"),
                ("function", "bus_port.address"),
                ("connection", "tube_part"),
            ]
            .contains(&(kind, path));
            // A channel's safety data is a structure: its cell says to edit it in the property grid.
            let in_grid = (kind, path) == ("channel", "safety");
            assert!(
                scalar(&spec.ty) || typed || in_grid,
                "{kind}.{path}: {}",
                spec.ty.word()
            );
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
            assert!(!col.derived);
        }
        for nav in NavKind::ALL {
            for c in columns_of(nav) {
                let listed = EDITABLE_COLUMNS
                    .iter()
                    .any(|(n, k, _, _)| *n == nav.source() && *k == c.key);
                if listed {
                    assert!(c.route.is_some(), "{nav:?}.{}", c.key);
                } else {
                    assert_eq!(c.route, None, "{nav:?}.{}", c.key);
                }
            }
        }
    }
}
