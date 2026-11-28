// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The place catalogue: what can be placed on a page, its symbols, parts, PLC parts, template functions,
//! bus interfaces and terminal strips, and the words their rows print.
//!
//! Not here: the `library.embed` parameters that place a catalogue row (`embeds.rs`); a symbol's shape
//! (`shape.rs`); the supply marks (`supply.rs`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use ew_derive::{DisplayedTag, PageOrder, PageRef, Q, short_tag};
use ew_ids::{Id, Uuid};
use ew_library::starter;
use ew_library::starter::StarterKind;
use ew_rds::IdentifierSet;
use ew_schema::{
    ConnectionKind, ContactKind, Device, FnDef, IpDirection, LibCopy, LibDocument, LibRef, LineKind, Page,
    PageType, Part, PartKind, RepType, SymbolDef,
};

use crate::Session;
use crate::output::join::fndef_copy;

/// What can be placed on a page: symbols and parts.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PlaceCatalogue {
    /// Embedded symbols and available library symbols on the page's stored grid, in key/address order.
    pub symbols: Vec<CatalogueSymbol>,
    /// The embedded part copies, then the libraries' parts with a function template the project does not hold,
    /// each group by manufacturer, then part number, then copy ID.
    pub parts: Vec<CataloguePart>,
    /// The libraries' PLC parts without a function template, which `parts` does not offer (nothing draws them):
    /// the station view lists them greyed with what to do.
    pub plc_without_functions: Vec<CataloguePart>,
    /// Every terminal strip of the project, by its tag as the page prints it: the strip chooser of a
    /// terminal placement.
    pub strips: Vec<CatalogueStrip>,
}

impl PlaceCatalogue {
    /// The row of the bus symbol `ew.gen.bus` the page draws a device with (doc 10 §2.7), on the page's grid.
    pub fn bus_symbol(&self) -> Option<&CatalogueSymbol> {
        self.symbols
            .iter()
            .find(|s| ew_schema::starter::edition_of(&s.key) == ew_schema::starter::GEN_BUS && s.allowed)
    }

    /// Whether choosing function `f` of part `p` draws it as a device box by the Black box tool: a black-box entry
    /// whose symbol row is the frame's (or whose FnDef is the generic black box). A symbol of its own drawn for the
    /// entry's FnDef (a power supply the user drew) draws it as any symbol does.
    pub fn drawn_as_box(&self, p: usize, f: usize) -> bool {
        let Some(f) = self.parts.get(p).and_then(|part| part.functions.get(f)) else {
            return false;
        };
        let Some(row) = f
            .symbol
            .as_ref()
            .and_then(|key| self.symbols.iter().find(|s| s.key == *key))
        else {
            return false;
        };
        f.black_box && (row.black_box || f.fndef_key == ew_schema::starter::FN_BLACKBOX)
    }
}

/// A terminal strip of the place catalogue: what a terminal placement adds to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CatalogueStrip {
    /// The strip device's ID.
    pub id: Uuid,
    /// The strip's tag as the page prints it (`-X1031`, `1X1`; `displayed_tag`).
    pub tag: String,
    /// The next free decimal designation: one more than the largest decimal designation of its terminals, `1`
    /// without one.
    pub next_designation: String,
    /// The sequence number `terminal.add` would give the next terminal: one more than the largest, `1` without one.
    pub next_seq: u32,
}

/// A symbol row of the place catalogue.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CatalogueSymbol {
    /// The symbol's key (`ew.iec.mcb-1p@qet10`); the copy's UUID text for a copy without a key.
    pub key: String,
    /// The copy ID when embedded; `None` for an unembedded starter or installed-library symbol.
    pub copy: Option<Uuid>,
    /// The key of the symbol's function definition (`ew.fn.mcb-1p`).
    pub fndef_key: String,
    /// The symbol's representation type in its JSON spelling (`multi_line`).
    pub rep_type: String,
    /// The letter code the project's identifier set maps the FnDef's category to (`F`), `None` without a mapping.
    pub letter: Option<String>,
    /// Whether the FnDef's contact kind is `black_box` (placed with the black box tool).
    pub black_box: bool,
    /// Whether the FnDef's first category is `Terminal` or `TerminalEarth` (placed as a terminal of a strip).
    pub terminal: bool,
    /// Whether the page's type allows the symbol's representation type, and the page's grid the symbol's.
    pub allowed: bool,
    /// A published library symbol on a connection grid other than the page's (SM25b m3): the symbol's grid and the
    /// page's, each as its `(num, den)` nanometres per step. The row is listed, not allowed, with this reason.
    pub grid_mismatch: Option<[(u64, u64); 2]>,
    /// The words the picker's filter matches besides the keys: the symbol's name in every language and its
    /// keywords.
    pub words: Vec<String>,
    /// The symbol's name in words (M39: the Place HUD names it): its neutral text, else its first language's; empty
    /// when the symbol has no name. A window shows [`CatalogueSymbol::name_text`] in its own language.
    pub name: String,
    /// The symbol's name in every language it has.
    pub name_text: ew_mltext::MlText,
    /// How the symbol gets onto a page.
    pub placed_by: PlacedBy,
    /// The first level of its FnDef's category (`ProtectionOvercurrent`, the browser's tree node); empty when the
    /// FnDef is missing.
    pub category: String,
    /// The discipline its connection points share (the browser says what a symbol is for); `None` when they
    /// differ or it has none.
    pub discipline: Option<ew_schema::Discipline>,
}

impl CatalogueSymbol {
    /// Whether it is an installed library's symbol not embedded yet: its key is its library address
    /// (`library:<library>:<item>`).
    pub fn from_library(&self) -> bool {
        self.copy.is_none() && self.key.starts_with("library:")
    }
}

/// How a symbol gets onto a page: what choosing it in the Insert symbol list does.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PlacedBy {
    /// As the representation of a device or a function: the Place tool, the black box tool, a terminal of a strip.
    #[default]
    Device,
    /// By the Interruption tool (the function's category is `InterruptionPoint`).
    InterruptionTool,
    /// By the Wire marker tool (the starter's `ew.gen.wire-marker`).
    MarkerTool,
    /// By the program, with what it belongs to: a junction's dot or bevel, a cable's marker, the arrow of an
    /// interruption point's external reference (the categories `Graphic` and `ExternalReference`).
    Program,
}

/// Whether a page of type `page_type` takes a symbol of representation type `rep_type`, both in their JSON
/// spellings (`fluid`, `multi_line`); an unknown word of either is not refused, as the command does not.
pub fn page_type_takes(page_type: &str, rep_type: &str) -> bool {
    let page: Option<PageType> = serde_json::from_value(serde_json::Value::String(page_type.to_owned())).ok();
    let rep: Option<RepType> = serde_json::from_value(serde_json::Value::String(rep_type.to_owned())).ok();
    match (
        page.as_ref().and_then(ew_commands::commands::allowed_rep_types),
        rep,
    ) {
        (Some(allowed), Some(rep)) => matches!(rep, RepType::Unknown(_)) || allowed.contains(&rep),
        _ => true,
    }
}

/// A part row of the place catalogue: an embedded part copy with a function template, or a library's part the
/// project does not hold yet, which choosing embeds.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CataloguePart {
    /// The part copy's ID (for a library part, the ID its copy will have: `v5(NS_LIBCOPY, library ‖ item)`).
    pub copy: Uuid,
    /// `(library, item)` of a library part the project does not hold; `None` for an embedded copy.
    pub library: Option<(Uuid, Uuid)>,
    /// The name of that library (the browser's library column); empty for an embedded copy.
    pub library_name: String,
    /// The part number.
    pub part_number: String,
    /// The part's description in words (the picker row leads with it): its neutral text, else its first
    /// language's; empty when it has none.
    pub description: String,
    /// The manufacturer's name (the library record's, held or not; [`Session::library_manufacturer`]).
    pub manufacturer: String,
    /// The part's ratings in words, its rated current, voltage and power in that order (`4 A · 230 V`), which tell
    /// two parts of one kind apart; empty when it has none.
    pub rating: String,
    /// The template's functions, in template order; empty for a held copy without a function template.
    pub functions: Vec<CatalogueTemplateFn>,
    /// The words the picker's filter matches besides the part number, the manufacturer and the FnDef keys
    /// ("relay" found nothing): the description in every language and the category.
    pub words: Vec<String>,
    /// The letter code the project's identifier set maps the part's own function category to (`G` for a power
    /// supply), `None` when the part has no category or the set no mapping: a black box placed from the part
    /// takes it.
    pub letter: Option<String>,
    /// The first level of the part's own category, else of its main function's FnDef category (the browser's tree
    /// node); empty when it has neither.
    pub category: String,
    /// On a bus-topology page, the part's interfaces ([`part_interfaces`]): a device of it is placed there with its
    /// bus symbol (`bus.place_ports`, doc 10 §2.7); empty on another page and for a part without them.
    pub interfaces: Vec<CatalogueInterface>,
    /// On a bus-topology page, the bus symbol of a part with interfaces as the page's grid draws it; `None` otherwise.
    pub bus_symbol: Option<BusSymbolShape>,
    /// Whether the part is a valve for a valve terminal (`ew_derive::fits_terminal`, no PLC section): the parts a
    /// terminal's empty position offers.
    pub terminal_valve: bool,
    /// Whether such a valve has a fluid symbol to stand in a valve position's cell, of any width: a position
    /// offers a valve without one with the reason, and does not take it.
    pub cell_symbol: bool,
    /// What the part is to a PLC ([`CataloguePlc`]): a station's head or a card; `None` for a part without a PLC
    /// section. The station view's catalogue lists the parts by it.
    pub plc: Option<CataloguePlc>,
    /// Whether the browser files the part under PLC and bus whatever its category says: a PLC card or a station's
    /// head, or a part with a bus interface. A valve terminal is filed as a valve terminal, with its plates and valves.
    pub plc_bus: bool,
    /// What the part brings with it (doc 07 §2.5), as the browser's detail lists it ("Comes with").
    pub accessories: Vec<CatalogueAccessory>,
    /// The part's variants: (key, name, number), in the part's order; empty for a part without variants.
    pub variants: Vec<(String, String, String)>,
}

/// One accessory of a catalogue part ([`CataloguePart::accessories`]), as the file spells its words.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CatalogueAccessory {
    /// Its kind (`end_plate`, `end_clamp`, …), empty without one.
    pub kind: String,
    /// The accessory's part number, empty when the project does not hold it yet.
    pub number: String,
    /// Its quantity.
    pub qty: String,
    /// Per what it counts (`unit`, `strip`, `rail_group`).
    pub per: String,
}

/// What a part is to a PLC, from its PLC and bus sections ([`CataloguePart::plc`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CataloguePlc {
    /// The kinds of station the part can head (`cpu`, `remote`, `valve_terminal`, `compact`: [`station_kinds`]); empty
    /// for a card, which goes into a station's slot.
    pub heads: Vec<&'static str>,
    /// What it gives the PLC (`8 DI`, [`crate::output::rack::gives_text`]); empty when its section names neither channels nor
    /// I/O ranges.
    pub gives: String,
    /// The slots a card may sit in; empty: any.
    pub allowed: Vec<u16>,
    /// The group the station view's catalogue lists it under.
    pub group: PlcGroup,
}

/// The groups of the station view's catalogue, in its order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum PlcGroup {
    /// A station's head that leads a bus or has no bus port: a PLC's CPU.
    Cpu,
    /// A station's head that follows on a bus: an interface module, a remote I/O head, a compact device.
    Interface,
    /// A valve terminal with a bus node.
    ValveTerminal,
    /// A card of digital inputs.
    DigitalIn,
    /// A card of digital outputs.
    DigitalOut,
    /// A card of analogue inputs or outputs.
    Analogue,
    /// Any other card (counters, IO-Link, supply, mixed).
    OtherCard,
    /// A valve for a valve terminal's position.
    Valve,
}

/// The kinds of station a part can head (the new station's "what it is"): `cpu` a part whose station leads a bus or
/// has no bus port (a PLC), `remote` one whose bus ports all follow (an interface module on a bus), `valve_terminal`
/// one that maps solenoids to valve positions, `compact` one with I/O of its own; a part may be more than one of them.
/// Empty for a part that is no station's head.
pub(crate) fn station_kinds(part: &Part) -> Vec<&'static str> {
    use ew_schema::{PlcRole, PortRole};
    let Some(plc) = part.sections.plc.as_ref().filter(|s| s.role == PlcRole::Station) else {
        return Vec::new();
    };
    let roles: Vec<&PortRole> = part
        .sections
        .bus
        .iter()
        .flat_map(|b| b.ports.iter().map(|x| &x.role))
        .collect();
    let leads = roles
        .iter()
        .any(|r| matches!(r, PortRole::Master | PortRole::Controller));
    let terminal = plc.positions.is_some();
    [
        ("cpu", !terminal && (roles.is_empty() || leads)),
        ("remote", !terminal && !roles.is_empty() && !leads),
        ("valve_terminal", terminal),
        ("compact", !terminal && !plc.io.is_empty()),
    ]
    .into_iter()
    .filter_map(|(word, fits)| fits.then_some(word))
    .collect()
}

/// What `part` is to a PLC ([`CataloguePart::plc`]).
pub(crate) fn catalogue_plc(part: &Part) -> Option<CataloguePlc> {
    use ew_schema::ChannelKind;
    let plc = part.sections.plc.as_ref()?;
    let heads = station_kinds(part);
    let group = if heads.contains(&"valve_terminal") {
        PlcGroup::ValveTerminal
    } else if heads.contains(&"cpu") {
        PlcGroup::Cpu
    } else if !heads.is_empty() {
        PlcGroup::Interface
    } else {
        // A card of two kinds of channels is a mixed card, among the other cards.
        match plc.channel_specs().as_slice() {
            [(_, c)] if c.kind == ChannelKind::Di => PlcGroup::DigitalIn,
            [(_, c)] if c.kind == ChannelKind::Do => PlcGroup::DigitalOut,
            [(_, c)] if matches!(c.kind, ChannelKind::Ai | ChannelKind::Ao) => PlcGroup::Analogue,
            _ => PlcGroup::OtherCard,
        }
    };
    Some(CataloguePlc {
        heads,
        gives: crate::output::rack::gives_text(plc),
        allowed: plc.allowed_slots.clone(),
        group,
    })
}

/// One function of a part's template in the place catalogue.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CatalogueTemplateFn {
    /// The 0-based position in the template (`device.place`'s `from_part.template`).
    pub index: u16,
    /// The key of the entry's function definition.
    pub fndef_key: String,
    /// The entry's function in words, as the Properties grid names a placed one: the FnDef's last
    /// category entry, the one before it lower-cased and the entry's CP designations (`NO contact 13-14`); the FnDef
    /// key when its category has fewer than two entries or the FnDef is missing; in every language of the session's
    /// texts.
    pub kind_text: crate::Text,
    /// Whether the entry is the part's main function.
    pub main: bool,
    /// The key of the first allowed symbol row that draws the entry's FnDef; `None` when no row does.
    pub symbol: Option<String>,
    /// Whether the entry's FnDef is a black box.
    pub black_box: bool,
    /// The CPs a function placed from the entry gets, in CP index order: each function CP index and the designation
    /// of the pattern entry it is drawn as (`ew_schema::TemplateFn::function_cps`, the rule `function_cps` derives,
    /// By position with the FnDef's CP indices when the entry names as many pattern entries as the FnDef has
    /// CPs, else the pattern entries with their own indices; the whole pattern when the entry names none).
    pub cps: Vec<(u16, String)>,
    /// The function CP indices whose FnDef point is a source's output (`source_output`: the power supply's + and −),
    /// which a black box placed from the entry lays along its bottom edge; empty when it has none.
    pub outputs: Vec<u16>,
    /// The part's device box when the entry is its main function (doc 07 §2.5): a click then places the box as the
    /// part lays it out; `None`: the box is drawn by two clicks.
    pub layout: Option<ew_schema::PartBox>,
}

/// The JSON spelling of a representation type (`multi_line`); `unknown` for a type of a newer format.
pub(super) fn rep_type_word(t: &RepType) -> String {
    match t {
        RepType::MultiLine => "multi_line",
        RepType::SingleLine => "single_line",
        RepType::Overview => "overview",
        RepType::Fluid => "fluid",
        RepType::Topology => "topology",
        RepType::ContactImage => "contact_image",
        RepType::PanelFront => "panel_front",
        RepType::StripSegment => "strip_segment",
        RepType::Graphic => "graphic",
        RepType::Unknown(_) => "unknown",
    }
    .to_owned()
}

/// The JSON key of a connection kind (`wire`, `core`, `bridge`, `bus`, `overview`, `internal`, `fluid`).
pub(super) fn connection_kind_word(k: &ConnectionKind) -> String {
    match k {
        ConnectionKind::Wire => "wire",
        ConnectionKind::Core(_) => "core",
        ConnectionKind::Bridge(_) => "bridge",
        ConnectionKind::Bus(_) => "bus",
        ConnectionKind::Overview => "overview",
        ConnectionKind::Internal => "internal",
        ConnectionKind::Fluid(_) => "fluid",
        ConnectionKind::Unknown(_) => "unknown",
    }
    .to_owned()
}

/// The JSON spelling of a fluid line's line kind (`unknown` for a kind of a newer format).
pub(super) fn line_kind_word(k: &LineKind) -> String {
    match k {
        LineKind::Working => "working",
        LineKind::Pilot => "pilot",
        LineKind::Exhaust => "exhaust",
        LineKind::Drain => "drain",
        LineKind::Flexible => "flexible",
        LineKind::Unknown(_) => "unknown",
    }
    .to_owned()
}

/// The JSON spelling of an IP direction.
pub fn ip_direction_word(d: &IpDirection) -> String {
    match d {
        IpDirection::Source => "source",
        IpDirection::Target => "target",
        IpDirection::Undirected => "undirected",
        IpDirection::Unknown(_) => "unknown",
    }
    .to_owned()
}

/// The starter documents, parsed once.
pub(super) fn starter_docs() -> &'static [(&'static str, StarterKind, LibDocument)] {
    static DOCS: OnceLock<Vec<(&'static str, StarterKind, LibDocument)>> = OnceLock::new();
    DOCS.get_or_init(|| {
        starter::items()
            .iter()
            .filter_map(|i| LibDocument::from_json(i.json).ok().map(|d| (i.key, i.kind, d)))
            .collect()
    })
}

/// The catalogue key of the starter symbol a part's bus port of `bus` that no symbol draws yet is placed with:
/// the DP port's (`ew.bus.dp-port`) for PROFIBUS DP, the PROFINET port's (`ew.bus.pn-port`) for PROFINET;
/// `None` for another bus system, which has no starter port symbol yet.
pub fn bus_port_symbol_key(bus: &ew_schema::BusSystem) -> Option<String> {
    let prefix = match bus {
        ew_schema::BusSystem::ProfibusDp => "ew.bus.dp-port@",
        ew_schema::BusSystem::Profinet => "ew.bus.pn-port@",
        _ => return None,
    };
    starter_docs().iter().find_map(|(key, _, d)| match d {
        LibDocument::Symbol(_) if key.starts_with(prefix) => Some((*key).to_owned()),
        _ => None,
    })
}

/// One interface of a part drawn on a bus-topology page (doc 10 §2.7, `bus.place_ports`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CatalogueInterface {
    /// The interface ("DP", "X1").
    pub interface: String,
    /// The port label of an interface listed port by port ("P1R").
    pub port_label: Option<String>,
    /// The interface in words: the interface, then the port label after a space ("X1", "X1 P1R").
    pub label: String,
    /// The copy ID of the starter FnDef a new port function of the interface takes: its bus's port (the DP port's,
    /// the PROFINET port's), the PROFINET port's for a bus without a starter port of its own.
    pub fndef: Uuid,
}

/// The interfaces of a part's bus section that its bus symbol draws (`ew_derive::bus_interfaces`: in list order, the
/// first of each (interface, port label) pair, one whose bus or role is of a newer format left out).
pub fn part_interfaces(part: &Part) -> Vec<CatalogueInterface> {
    let Some(bus) = &part.sections.bus else {
        return Vec::new();
    };
    ew_derive::bus_interfaces(part)
        .into_iter()
        .filter_map(|i| {
            let p = bus
                .ports
                .iter()
                .find(|p| p.interface == i.interface && p.port_label == i.port_label)?;
            let fndef = bus_port_symbol_key(&p.bus)
                .or_else(|| bus_port_symbol_key(&ew_schema::BusSystem::Profinet))
                .and_then(|k| starter_symbol(&k))
                .map(|s| fndef_copy(&s.item.fndef).as_uuid())?;
            Some(CatalogueInterface {
                interface: i.interface.as_str().to_owned(),
                port_label: i.port_label.as_ref().map(|l| l.as_str().to_owned()),
                label: i.words(),
                fndef,
            })
        })
        .collect()
}

/// A part's bus symbol as a page of `grid` draws it, for the tool that places it: its body and its sockets, relative to
/// its insertion point (doc 10 §2.7).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BusSymbolShape {
    /// The body `[x0, y0, x1, y1]`, in nm.
    pub body: [i64; 4],
    /// Each socket's point, in nm, outside the body.
    pub sockets: Vec<(i64, i64)>,
}

/// The bus symbol `part` draws on a page of `grid` (`ew_derive::bus_symbol`).
pub fn bus_symbol_shape(part: &Part, grid: ew_units::Grid) -> BusSymbolShape {
    let s = ew_derive::bus_symbol(part, grid);
    let body = s
        .graphics
        .iter()
        .find_map(|p| match p {
            ew_schema::Primitive::Rect(r) => {
                let (lo, hi) = (r.rect.0.min(), r.rect.0.max());
                Some([lo.x.0, lo.y.0, hi.x.0, hi.y.0])
            }
            _ => None,
        })
        .unwrap_or_default();
    BusSymbolShape {
        body,
        sockets: s.cps.iter().map(|c| (c.pos.x.0, c.pos.y.0)).collect(),
    }
}

/// The copy ID that the address `library:<library>:<item>` of an installed library's symbol (its catalogue key until it
/// is embedded) names once the project holds it: `v5(NS_LIBCOPY, library ‖ item)`; `None` for another text. So the
/// address names the symbol before and after its first placement.
pub fn address_copy(address: &str) -> Option<Uuid> {
    let (library, item) = address.strip_prefix("library:")?.split_once(':')?;
    let id: Id<SymbolDef> = ew_ids::v5::lib_copy_id(library.parse().ok()?, item.parse().ok()?);
    Some(id.as_uuid())
}

/// The starter symbol with this key.
pub(super) fn starter_symbol(key: &str) -> Option<&'static LibCopy<SymbolDef>> {
    starter_docs().iter().find_map(|(k, _, d)| match d {
        LibDocument::Symbol(c) if *k == key => Some(c),
        _ => None,
    })
}

/// The fluid symbol that draws FnDef copy `def` in a valve terminal's box (a valve in its cell, the terminal's
/// supply in its supply end), on `grid` when one is given: the project's of the lowest ID (the one `seat_reps` draws
/// with), else the starter's; with whether it is the starter's.
pub(super) fn box_symbol(
    q: &Q,
    def: Id<FnDef>,
    grid: Option<ew_units::Grid>,
) -> Option<(LibCopy<SymbolDef>, bool)> {
    let draws = |s: &SymbolDef| {
        s.rep_type == RepType::Fluid && grid.is_none_or(|g| s.grid == g) && fndef_copy(&s.fndef) == def
    };
    let held = q
        .symbols()
        .iter()
        .filter(|(_, s)| draws(&s.item))
        .min_by_key(|(id, _)| *id)
        .map(|(_, s)| ((**s).clone(), false));
    held.or_else(|| {
        starter_docs().iter().find_map(|(_, _, d)| match d {
            LibDocument::Symbol(c) if draws(&c.item) => Some((c.clone(), true)),
            _ => None,
        })
    })
}

/// The FnDef copy of a part's valve function: its first template entry on a FnDef of contact kind `valve`
/// (embedded, else the starter's); `None` for a part without one.
pub(super) fn part_valve_def(q: &Q, part: &Part) -> Option<Id<FnDef>> {
    part.function_template.iter().find_map(|t| {
        fndef_of(q, &t.fndef)
            .filter(|d| d.item.contact_kind == ContactKind::Valve)
            .map(|d| d.id)
    })
}

/// Whether a valve for a valve terminal of part `part` has a fluid symbol to stand in a valve position's cell
/// ([`CataloguePart::cell_symbol`]): one that draws its valve FnDef with graphics ([`box_symbol`], on any grid); a
/// symbol wider than a cell widens its cell (`ew_derive::SlotFrame::with_cells`).
fn fits_cell(q: &Q, part: &Part) -> bool {
    part_valve_def(q, part)
        .and_then(|d| box_symbol(q, d, None))
        .is_some_and(|(symbol, _)| ew_derive::graphics_x_extent(&symbol.item).is_some())
}

/// The starter FnDef copy with this copy ID.
pub(in crate::output) fn starter_fndef(id: Id<FnDef>) -> Option<&'static LibCopy<FnDef>> {
    starter_docs().iter().find_map(|(_, _, d)| match d {
        LibDocument::Fndef(c) if c.id == id => Some(c),
        _ => None,
    })
}

/// The key a `LibRef` carries (`"key"` member), else empty.
fn ref_key(r: &LibRef) -> String {
    r.extra
        .get("key")
        .and_then(|k| k.as_str())
        .unwrap_or_default()
        .to_owned()
}

/// An item key, else the copy's UUID text.
pub(super) fn key_or_id(key: Option<&ew_mltext::Ident>, id: Uuid) -> String {
    key.map_or_else(|| id.to_string(), |k| k.as_str().to_owned())
}

/// The FnDef of a symbol: the embedded copy, else the starter copy.
pub(super) fn fndef_of(q: &Q, r: &LibRef) -> Option<Arc<LibCopy<FnDef>>> {
    let id = fndef_copy(r);
    q.fndefs()
        .get(id)
        .cloned()
        .or_else(|| starter_fndef(id).map(|c| Arc::new(c.clone())))
}

/// The socket count of the interface a template entry on port function `def` stands for (doc 10 §2.7): the part's
/// first port of the bus `def` is filed under (a category word that begins the bus's stored name, as normaliser N5
/// pairs them), else its first port; `None` for another FnDef or an interface without a count.
fn entry_sockets(part: &Part, def: &FnDef) -> Option<u16> {
    if def.contact_kind != ContactKind::BusPort {
        return None;
    }
    let ports = &part.sections.bus.as_ref()?.ports;
    let words = serde_json::to_value(&def.category).unwrap_or_default();
    let filed = |p: &&ew_schema::PartBusPort| {
        let name = serde_json::to_value(&p.bus)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        words
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|w| w.as_str())
            .any(|w| !w.is_empty() && name.starts_with(&w.to_lowercase()))
    };
    ports.iter().find(filed).or(ports.first())?.ports
}

/// The browser's group of a part with neither a category nor a template, from what its kind and sections say: an
/// enclosure under `Enclosure`; a mounting plate, rail, duct or end bracket under `MountingAccessory`; a tube under
/// `FluidTube`; a bus cable and a bus plug (an accessory with a bus interface) under `BusCable`; another cable under
/// `CablePower`; a fitting under `FluidFitting`. `None` for any other part.
fn kind_group(part: &Part) -> Option<&'static str> {
    let bus = part.sections.bus.as_ref();
    Some(match part.kind {
        PartKind::Enclosure => "Enclosure",
        PartKind::MountingPlate | PartKind::Rail | PartKind::Duct | PartKind::EndBracket => {
            "MountingAccessory"
        }
        PartKind::Tube => "FluidTube",
        PartKind::Cable if bus.is_some_and(|b| !b.cable_for.is_empty()) => "BusCable",
        PartKind::Cable | PartKind::Wire => "CablePower",
        PartKind::Accessory if bus.is_some_and(|b| !b.ports.is_empty()) => "BusCable",
        PartKind::Accessory if part.sections.fluid.as_ref().is_some_and(|f| f.fitting.is_some()) => {
            "FluidFitting"
        }
        _ => return None,
    })
}

/// The catalogue row of part `part` with copy ID `copy` (the manufacturer left empty); a part without a function
/// template is a row without functions (a held copy is placed with a symbol the user chooses; a library part is not
/// offered, [`Session::place_catalogue`]). `library` names a library part the project does not hold: its template
/// FnDefs that the project does not embed are read from the starter.
fn catalogue_part(
    q: &Q,
    set: Option<&IdentifierSet>,
    rows: &[(CatalogueSymbol, Id<FnDef>)],
    copy: Uuid,
    part: &Part,
    library: Option<(Uuid, Uuid)>,
    page: &Page,
) -> Option<CataloguePart> {
    let mut main_category = None;
    let functions = part
        .function_template
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let def_id = fndef_copy(&t.fndef);
            let def = match library {
                Some(_) => fndef_of(q, &t.fndef),
                None => q.fndefs().get(def_id).cloned(),
            };
            if t.main && main_category.is_none() {
                main_category = def.as_ref().map(|d| d.item.category.0.as_str().to_owned());
            }
            // A black box on its own FnDef (the supply's, which declares its source outputs) is drawn by the black
            // box frame, whatever FnDef the frame's symbol names (read from the starter while not embedded).
            let read = def.clone().or_else(|| fndef_of(q, &t.fndef));
            // A port function of an interface with a socket count is drawn as a box with every socket (doc 10 §2.7).
            let sockets = read.as_ref().and_then(|d| entry_sockets(part, &d.item));
            let is_black_box = sockets.is_some()
                || read
                    .as_ref()
                    .is_some_and(|d| d.item.contact_kind == ContactKind::BlackBox);
            let outputs = read
                .map(|d| {
                    d.item
                        .cps
                        .iter()
                        .filter(|c| c.source_output)
                        .map(|c| c.idx)
                        .collect()
                })
                .unwrap_or_default();
            // The piece's own symbol where the page has it, else the first of its function (doc 07 §5.6).
            let of_function = rows
                .iter()
                .filter(|(r, f)| sockets.is_none() && *f == def_id && r.allowed)
                .map(|(r, _)| (r.copy.or_else(|| address_copy(&r.key)).unwrap_or_default(), r));
            let symbol = ew_derive::placed_symbol(t, of_function)
                .or_else(|| {
                    rows.iter()
                        .find(|(r, _)| is_black_box && r.black_box && r.allowed)
                        .map(|(r, _)| r)
                })
                .map(|r| r.key.clone());
            // The CPs a function placed from the entry gets: the rule `function_cps` derives.
            let pattern: Vec<u16> = part.cp_pattern.iter().map(|c| c.idx).collect();
            let def_cps: Vec<u16> = def
                .as_ref()
                .map(|d| d.item.cps.iter().map(|c| c.idx).collect())
                .unwrap_or_default();
            let cps = t
                .function_cps(&pattern, &def_cps)
                .into_iter()
                .map(|c| {
                    let d = part
                        .cp_pattern
                        .iter()
                        .find(|p| p.idx == c.pattern)
                        .map(|p| p.designation.as_str().to_owned())
                        .unwrap_or_default();
                    (c.idx, d)
                })
                .collect::<Vec<(u16, String)>>();
            let cps = match sockets {
                Some(n) => (0..n).map(|i| (i, format!("P{}", u32::from(i) + 1))).collect(),
                None => cps,
            };
            let fndef_key = def
                .as_ref()
                .and_then(|d| d.item.meta.key.as_ref().map(|k| k.as_str().to_owned()))
                .unwrap_or_else(|| ref_key(&t.fndef));
            let kind_text = def
                .as_ref()
                .and_then(|d| {
                    // The entry's designations, else the FnDef's (as a placed function's text falls back).
                    let mut designations: Vec<String> = cps.iter().map(|(_, s)| s.clone()).collect();
                    if designations.is_empty() {
                        let mut own: Vec<(u16, String)> = d
                            .item
                            .cps
                            .iter()
                            .map(|c| (c.idx, c.designation.as_str().to_owned()))
                            .collect();
                        own.sort();
                        designations = own.into_iter().map(|(_, s)| s).collect();
                    }
                    crate::output::navigators::kind_text(&d.item, &designations)
                })
                .unwrap_or_else(|| crate::Text::bare(crate::TextArg::Plain(fndef_key.clone())));
            CatalogueTemplateFn {
                index: u16::try_from(i).unwrap_or(u16::MAX),
                fndef_key,
                kind_text,
                main: t.main,
                symbol,
                black_box: def
                    .as_ref()
                    .is_some_and(|d| d.item.contact_kind == ContactKind::BlackBox),
                cps,
                outputs,
                // Whether or not the FnDef is embedded yet (as the CPs above).
                layout: part.box_.clone().filter(|_| t.main),
            }
        })
        .collect();
    let mut words = ml_texts(&part.description);
    if let Some((a, b, c)) = &part.category {
        words.extend([a, b, c].map(|w| w.as_str().to_owned()));
    }
    let letter = part.category.as_ref().and_then(|(category, group, _)| {
        set?.mapping(category.as_str(), Some(group.as_str()))
            .map(|m| m.letter().as_str().to_owned())
    });
    // A valve for a valve terminal: a valve without a coil of its own to wire, and no PLC part.
    let defs: Vec<_> = part
        .function_template
        .iter()
        .filter_map(|t| {
            q.fndefs()
                .get(fndef_copy(&t.fndef))
                .cloned()
                .or_else(|| fndef_of(q, &t.fndef))
        })
        .collect();
    // A valve made for a terminal names the solenoids the terminal drives (hand-operated valves fit no position).
    let terminal_valve = part.sections.plc.is_none()
        && part
            .sections
            .fluid
            .as_ref()
            .is_some_and(|f| !f.solenoids.is_empty())
        && ew_derive::fits_terminal(defs.iter().map(|d| &d.item));
    Some(CataloguePart {
        copy,
        library,
        library_name: String::new(),
        part_number: part.part_number.as_str().to_owned(),
        // In the project's language, else English, else the first, as the lists print a text.
        description: ew_derive::text_in(
            &part.description,
            q.project().languages.as_ref().map(|l| &l.source),
        )
        .to_owned(),
        manufacturer: String::new(),
        rating: rating_text(part),
        functions,
        words,
        letter,
        category: part
            .category
            .as_ref()
            .map(|(c, _, _)| c.as_str().to_owned())
            .or(main_category)
            .or_else(|| kind_group(part).map(str::to_owned))
            .unwrap_or_default(),
        interfaces: if page.kind == PageType::BusTopology {
            part_interfaces(part)
        } else {
            Vec::new()
        },
        bus_symbol: (page.kind == PageType::BusTopology && !part_interfaces(part).is_empty())
            .then(|| bus_symbol_shape(part, page.grid)),
        terminal_valve,
        cell_symbol: terminal_valve && fits_cell(q, part),
        plc_bus: part.sections.plc.as_ref().is_none_or(|p| p.positions.is_none())
            && (part.sections.plc.is_some()
                || part.sections.bus.as_ref().is_some_and(|b| !b.ports.is_empty())),
        plc: catalogue_plc(part),
        accessories: part
            .accessories
            .iter()
            .map(|a| CatalogueAccessory {
                kind: a
                    .kind
                    .as_ref()
                    .map(crate::part_editor::stored_word)
                    .unwrap_or_default(),
                number: q
                    .parts()
                    .get(ew_derive::accessory_part(q, a))
                    .map(|c| c.item.part_number.as_str().to_owned())
                    .unwrap_or_default(),
                qty: a.qty.to_string(),
                per: crate::part_editor::stored_word(&a.per),
            })
            .collect(),
        variants: part
            .variants
            .iter()
            .map(|v| {
                (
                    v.key.as_str().to_owned(),
                    v.name.neutral_or_first().unwrap_or(v.key.as_str()).to_owned(),
                    part.with_variant(Some(v.key.as_str()))
                        .part_number
                        .as_str()
                        .to_owned(),
                )
            })
            .collect(),
    })
}

/// A part's ratings in words ([`CataloguePart::rating`]): each of its rated current, voltage and power as
/// `<value> <unit>`, joined by ` · `.
fn rating_text(part: &Part) -> String {
    ["rated_current", "rated_voltage", "rated_power"]
        .iter()
        .filter_map(|k| {
            let t = part.tech_data.iter().find(|t| t.key.as_str() == *k)?;
            let v = serde_json::to_value(&t.value).ok()?;
            Some(format!(
                "{} {}",
                v.get("value")?.as_str()?,
                v.get("unit")?.as_str()?
            ))
        })
        .collect::<Vec<_>>()
        .join(" \u{b7} ")
}

/// Every text of `m`: the neutral one, else each language's.
fn ml_texts(m: &ew_mltext::MlText) -> Vec<String> {
    match m.as_neutral() {
        Some(t) => vec![t.to_owned()],
        None => m
            .languages()
            .filter_map(|l| m.get(l))
            .map(str::to_owned)
            .collect(),
    }
}

/// The letter the identifier set maps a FnDef's category to.
fn letter_of(set: Option<&IdentifierSet>, def: Option<&LibCopy<FnDef>>) -> Option<String> {
    category_letter(set?, &def?.item.category)
}

/// The letter the identifier set maps the function category `category` to: the one a new device of it takes.
fn category_letter(set: &IdentifierSet, category: &ew_schema::CategoryKey) -> Option<String> {
    let (category, group, _) = category;
    set.mapping(category.as_str(), Some(group.as_str()))
        .map(|m| m.letter().as_str().to_owned())
}

impl Session {
    /// The letter a new device of a function of category `category` takes, by the project's identifier set (with no
    /// project, the profile's): what a drawing's sample tag shows (doc 07 §5.6).
    pub fn device_letter(&self, category: &ew_schema::CategoryKey) -> Option<String> {
        let q = self.query();
        let set = IdentifierSet::by_key(&q.profile().identifier_set)?;
        category_letter(set, category)
    }
}

/// Whether a page type allows a representation type (`ew_commands`' rule: an unknown page or representation type
/// is not refused).
fn rep_type_allowed(page: &Page, rep_type: &RepType) -> bool {
    match ew_commands::commands::allowed_rep_types(&page.kind) {
        None => true,
        Some(allowed) => matches!(rep_type, RepType::Unknown(_)) || allowed.contains(rep_type),
    }
}

/// A catalogue symbol row of a symbol document.
fn symbol_row(
    q: &Q,
    set: Option<&IdentifierSet>,
    page: &Page,
    key: String,
    copy: Option<Uuid>,
    symbol: &SymbolDef,
) -> CatalogueSymbol {
    let def = fndef_of(q, &symbol.fndef);
    let fndef_key = def
        .as_ref()
        .and_then(|d| d.item.meta.key.as_ref().map(|k| k.as_str().to_owned()))
        .unwrap_or_else(|| ref_key(&symbol.fndef));
    // The category of the function says who puts the symbol on a page; of the program's own graphics the wire
    // marker has a tool.
    let placed_by = match def.as_ref().map(|d| d.item.category.0.as_str()) {
        Some("InterruptionPoint") => PlacedBy::InterruptionTool,
        Some("ExternalReference") => PlacedBy::Program,
        Some("Graphic") if ew_schema::starter::edition_of(&key) == ew_schema::starter::GEN_WIRE_MARKER => {
            PlacedBy::MarkerTool
        }
        Some("Graphic") => PlacedBy::Program,
        _ => PlacedBy::Device,
    };
    let disciplines: Vec<&ew_schema::Discipline> = match def.as_ref() {
        Some(d) => d.item.cps.iter().map(|c| &c.discipline).collect(),
        None => symbol.cps.iter().map(|c| &c.discipline).collect(),
    };
    let discipline = disciplines
        .first()
        .filter(|first| disciplines.iter().all(|d| d == *first))
        .map(|d| (*d).clone());
    let mut row = CatalogueSymbol {
        discipline,
        placed_by,
        category: def
            .as_ref()
            .map(|d| d.item.category.0.as_str().to_owned())
            .unwrap_or_default(),
        key,
        copy,
        fndef_key,
        rep_type: rep_type_word(&symbol.rep_type),
        letter: letter_of(set, def.as_deref()),
        // The bus symbol is drawn from a part's network ports, never as a box (doc 10 §2.7).
        black_box: !ew_derive::is_bus_copy(symbol)
            && symbol.meta.key.as_ref().is_some_and(|key| {
                starter_symbol(key.as_str()).is_some_and(|s| {
                    copy.is_none_or(|id| id == s.id.as_uuid())
                        && s.item.fndef.library == symbol.fndef.library
                        && s.item.fndef.item == symbol.fndef.item
                })
            })
            && def
                .as_ref()
                .is_some_and(|d| d.item.contact_kind == ContactKind::BlackBox),
        // A PE terminal (`TerminalEarth`) is placed into a strip like every terminal.
        terminal: def
            .as_ref()
            .is_some_and(|d| ew_rds::category_flags(d.item.category.0.as_str()).terminal),
        allowed: rep_type_allowed(page, &symbol.rep_type),
        grid_mismatch: None,
        name: ml_texts(&symbol.name).into_iter().next().unwrap_or_default(),
        name_text: symbol.name.clone(),
        words: ml_texts(&symbol.name)
            .into_iter()
            .chain(symbol.keywords.as_slice().iter().map(|k| k.as_str().to_owned()))
            .collect(),
    };
    // A device with ports is drawn on a bus-topology page as a box with every socket (doc 10 §2.7): the black-box
    // frame draws a topology representation there.
    if row.black_box && page.kind == PageType::BusTopology && !row.allowed {
        row.rep_type = rep_type_word(&RepType::Topology);
        row.allowed = true;
    }
    row
}

/// The drawn page labelled `folio`.
pub(super) fn drawn_page(q: &Q, folio: &str) -> Option<(PageRef, Id<Page>)> {
    let order = q.get::<PageOrder>(());
    let entry = order.by_folio(folio)?;
    match entry.page {
        PageRef::Drawn(p) => Some((entry.page, p)),
        _ => None,
    }
}

impl Session {
    /// The address `library:<library>:<item>` of the installed library's symbol whose key is `key` (`None` for
    /// another text): its catalogue key until it is embedded, after which its copy's key is `key`. So the key names
    /// the symbol before its first placement as after it, as [`address_copy`] lets the address do.
    pub fn library_symbol_address(&self, key: &str) -> Option<String> {
        self.libraries().libraries().into_iter().find_map(|library| {
            self.libraries()
                .documents(library.id, ew_schema::Kind::Symbol)
                .into_iter()
                .find_map(|doc| match doc {
                    LibDocument::Symbol(copy)
                        if copy.item.meta.key.as_ref().is_some_and(|k| k.as_str() == key) =>
                    {
                        Some(format!("library:{}:{}", copy.library_id, copy.item_id))
                    }
                    _ => None,
                })
        })
    }

    /// What can be placed on the page labelled `folio`; `None` for an unknown folio or a page
    /// that is not drawn (a generated page, a divider).
    ///
    /// `symbols` lists embedded symbols, unembedded starter symbols and installed library masters, filtered by
    /// their stored grid. Library masters have qualified addresses; an embedded copy remains version-pinned.
    /// Rows are ordered by key/address. `letter` is the identifier set's letter for the FnDef's category;
    /// `black_box` identifies the built-in resizable placeholder, `terminal` its terminal category, `allowed` whether
    /// the page's type allows the symbol's representation type (`allowed_rep_types`). `parts` lists every embedded
    /// part copy, with or without a function template, then every part of the libraries with a function
    /// template the project does not hold (the project's own copies first; their PLC parts without one go to
    /// `plc_without_functions`), each group by
    /// manufacturer name, part number, copy ID; a template function's `symbol` is the key of the first
    /// allowed symbol row that draws its FnDef.
    pub fn place_catalogue(&self, folio: &str) -> Option<PlaceCatalogue> {
        let q = self.query();
        let (_, page_id) = drawn_page(&q, folio)?;
        let page = q.pages().get(page_id)?.clone();
        let set = IdentifierSet::by_key(&q.profile().identifier_set);
        let mut rows: Vec<(CatalogueSymbol, Id<FnDef>)> = Vec::new();
        let mut embedded_keys: BTreeSet<String> = BTreeSet::new();
        // SM25b m3: the published library symbols, by copy ID, so that an embedded one on another grid is listed too.
        let published: BTreeSet<Uuid> = self
            .libraries()
            .libraries()
            .iter()
            .flat_map(|l| self.libraries().documents(l.id, ew_schema::Kind::Symbol))
            .filter_map(|d| match d {
                LibDocument::Symbol(s) => Some(s.id.as_uuid()),
                _ => None,
            })
            .filter(|id| {
                !starter_docs()
                    .iter()
                    .any(|(_, _, d)| matches!(d, LibDocument::Symbol(s) if s.id.as_uuid() == *id))
            })
            .collect();
        for (id, copy) in q.symbols().iter() {
            let key = key_or_id(copy.item.meta.key.as_ref(), id.as_uuid());
            embedded_keys.insert(key.clone());
            let other_grid = copy.item.grid != page.grid;
            if other_grid && !published.contains(&id.as_uuid()) {
                continue;
            }
            let mut row = symbol_row(&q, set, &page, key, Some(id.as_uuid()), &copy.item);
            if other_grid {
                row.grid_mismatch = Some([
                    (copy.item.grid.num(), copy.item.grid.den()),
                    (page.grid.num(), page.grid.den()),
                ]);
            }
            rows.push((row, fndef_copy(&copy.item.fndef)));
        }
        for (key, kind, doc) in starter_docs() {
            let (StarterKind::Symbol, LibDocument::Symbol(copy)) = (kind, doc) else {
                continue;
            };
            if embedded_keys.contains(*key) || copy.item.grid != page.grid || q.symbols().contains(copy.id) {
                continue;
            }
            let row = symbol_row(&q, set, &page, (*key).to_owned(), None, &copy.item);
            rows.push((row, fndef_copy(&copy.item.fndef)));
        }
        // Authored/imported library masters are offered independently of project copies.
        for library in self.libraries().libraries() {
            for doc in self.libraries().documents(library.id, ew_schema::Kind::Symbol) {
                let LibDocument::Symbol(copy) = doc else { continue };
                if q.symbols().contains(copy.id)
                    || starter_docs()
                        .iter()
                        .any(|(_, _, d)| matches!(d, LibDocument::Symbol(s) if s.id == copy.id))
                {
                    continue;
                }
                let address = format!("library:{}:{}", copy.library_id, copy.item_id);
                let mut row = symbol_row(&q, set, &page, address, None, &copy.item);
                // A published symbol of another grid is listed with both grids; Place puts its copy drawn on the page's
                // grid there. Whether the page's type takes it stays `allowed`.
                if copy.item.grid != page.grid {
                    row.grid_mismatch = Some([
                        (copy.item.grid.num(), copy.item.grid.den()),
                        (page.grid.num(), page.grid.den()),
                    ]);
                }
                let Ok(LibDocument::Fndef(def)) = self.libraries().resolve(&copy.item.fndef) else {
                    continue;
                };
                row.fndef_key = key_or_id(def.item.meta.key.as_ref(), def.id.as_uuid());
                row.letter = letter_of(set, Some(&def));
                row.terminal = ew_rds::category_flags(def.item.category.0.as_str()).terminal;
                row.category = def.item.category.0.as_str().to_owned();
                // A library master already has its authored outline and pins; do not redraw it as a box.
                row.black_box = false;
                row.name = format!("{} · {}", row.name, library.name);
                if let Some(key) = &copy.item.meta.key {
                    row.words.push(key.to_string());
                }
                row.words.push(library.name.clone());
                rows.push((row, fndef_copy(&copy.item.fndef)));
            }
        }
        rows.sort_by(|(a, _), (b, _)| {
            a.key
                .cmp(&b.key)
                .then_with(|| b.copy.is_some().cmp(&a.copy.is_some()))
        });
        // One order for the chooser, (maker, part number, copy), within the embedded copies and
        // within the libraries' parts the project does not hold, the embedded ones first.
        // An embedded part's maker is named from its own maker field as the Parts row names it
        // (`ew_derive::manufacturer_name`); a library part's, which the project does not hold, from its library
        // ([`Session::library_manufacturer`]).
        let mut parts: Vec<CataloguePart> = Vec::new();
        let mut without_functions: Vec<CataloguePart> = Vec::new();
        for (id, copy) in q.parts().iter() {
            // The parts a wire consumes (the workshop stock's wires, ferrules and markers) and a cable's gland are no
            // device to place.
            if copy.item.kind.in_stock() {
                continue;
            }
            if let Some(row) = catalogue_part(&q, set, &rows, id.as_uuid(), &copy.item, None, &page) {
                parts.push(CataloguePart {
                    manufacturer: ew_derive::manufacturer_name(&q, &copy.item.manufacturer),
                    ..row
                });
            }
        }
        // The libraries' parts the project does not hold.
        for c in self.library_parts() {
            if q.parts().contains(c.id)
                || (c.item.function_template.is_empty() && c.item.sections.plc.is_none())
            {
                continue;
            }
            let manufacturer = self.library_manufacturer(&c.item.manufacturer);
            let source = Some((c.library_id.as_uuid(), c.item_id.as_uuid()));
            let library_name = self
                .libraries()
                .libraries()
                .into_iter()
                .find(|l| l.id == c.library_id.as_uuid())
                .map(|l| l.name.clone())
                .unwrap_or_default();
            let Some(row) = catalogue_part(&q, set, &rows, c.id.as_uuid(), &c.item, source, &page) else {
                continue;
            };
            let row = CataloguePart {
                manufacturer,
                library_name,
                ..row
            };
            // A library part without a function template has nothing to draw it with: it is not offered, and a PLC
            // part of those is kept apart, so that the station view says why it is not.
            if !c.item.function_template.is_empty() {
                parts.push(row);
            } else if row.plc.is_some() {
                without_functions.push(row);
            }
        }
        // The project's own copies first, then the libraries' parts, each in that order.
        parts.sort_by(|a, b| {
            (a.library.is_some(), &a.manufacturer, &a.part_number, a.copy).cmp(&(
                b.library.is_some(),
                &b.manufacturer,
                &b.part_number,
                b.copy,
            ))
        });
        Some(PlaceCatalogue {
            symbols: rows.into_iter().map(|(r, _)| r).collect(),
            parts,
            plc_without_functions: without_functions,
            strips: strips_of(&q, page_id),
        })
    }
}

/// Every terminal strip of the project with its tag as `page` prints it and the next designation and sequence
/// number, by tag, ties by ID.
fn strips_of(q: &Q, page: Id<Page>) -> Vec<CatalogueStrip> {
    // The decimal designations in use (the next one is the lowest number free, so a gap an Undo
    // left is filled), the width of the widest zero-padded spelling (so `01` is followed by `02`) and the largest
    // sequence number of each strip's terminals.
    let mut used: BTreeMap<Id<Device>, (BTreeSet<u64>, usize, u32)> = BTreeMap::new();
    for (_, d) in q.devices().iter() {
        if let Some(t) = &d.aspects.terminal {
            let e = used.entry(t.strip).or_default();
            let designation = t.designation.as_str();
            if !designation.is_empty()
                && designation.bytes().all(|b| b.is_ascii_digit())
                && let Ok(n) = designation.parse::<u64>()
            {
                e.0.insert(n);
                if designation.len() > 1 && designation.starts_with('0') {
                    e.1 = e.1.max(designation.len());
                }
            }
            e.2 = e.2.max(t.seq);
        }
    }
    let mut out: Vec<CatalogueStrip> = q
        .devices()
        .iter()
        .filter(|(_, d)| d.aspects.strip.is_some())
        .map(|(id, _)| {
            let (taken, width, seq) = used.get(&id).map_or((None, 0, 0), |(t, w, s)| (Some(t), *w, *s));
            // The lowest number from 1 not in use: at most one step more than the strip has numbers.
            let mut next: u64 = 1;
            while next < u64::MAX && taken.is_some_and(|t| t.contains(&next)) {
                next += 1;
            }
            // The tag as the page prints it; a strip the page shows nothing of yet prints its short tag.
            let shown = q.get::<DisplayedTag>((id, page)).as_ref().clone();
            let tag = if shown.is_empty() { short_tag(q, id) } else { shown };
            CatalogueStrip {
                id: id.as_uuid(),
                tag,
                next_designation: format!("{next:0width$}"),
                next_seq: seq.saturating_add(1),
            }
        })
        .collect();
    out.sort_by(|a, b| a.tag.cmp(&b.tag).then(a.id.cmp(&b.id)));
    out
}

/// The gap between two pieces of a part's schematic picture: 8 mm.
const PIECE_GAP: ew_units::Nm = ew_units::Nm(8_000_000);

impl Session {
    /// The schematic picture of `part` (doc 07 §5.6): each piece of its function template drawn by its own symbol
    /// (`TemplateFn.symbol`, from the project or an installed library), else by a symbol of its function (the project's
    /// of the lowest ID, else the starter's), left to right in template order, as a page draws a symbol's lines; with
    /// the picture's width and height. `None` when no piece has a symbol.
    pub fn part_schematic(&self, part: &Part) -> Option<(ew_dl::DisplayList, (ew_units::Nm, ew_units::Nm))> {
        let q = self.query();
        let symbol_of = |entry: &ew_schema::TemplateFn| -> Option<SymbolDef> {
            let def = fndef_copy(&entry.fndef);
            let draws = |s: &SymbolDef| s.rep_type == RepType::MultiLine && fndef_copy(&s.fndef) == def;
            let own = entry
                .symbol
                .as_ref()
                .and_then(|r| match self.libraries().resolve(r) {
                    Ok(LibDocument::Symbol(c)) if draws(&c.item) => Some((c.id.as_uuid(), c.item)),
                    _ => None,
                });
            let mut held: Vec<(Uuid, SymbolDef)> = q
                .symbols()
                .iter()
                .filter(|(_, s)| draws(&s.item))
                .map(|(id, s)| (id.as_uuid(), s.item.clone()))
                .collect();
            held.sort_by_key(|(id, _)| *id);
            let starter = starter_docs().iter().filter_map(|(_, _, d)| match d {
                LibDocument::Symbol(c) if draws(&c.item) => Some((c.id.as_uuid(), c.item.clone())),
                _ => None,
            });
            ew_derive::placed_symbol(entry, own.into_iter().chain(held).chain(starter))
        };
        let mut list = ew_dl::DisplayList::new();
        let (mut x, mut height) = (0i64, 0i64);
        for entry in &part.function_template {
            let Some(def) = symbol_of(entry) else {
                continue;
            };
            let geometry = ew_scene::symbol::lower(&def, ew_units::Orient2::R0);
            let b = geometry.bounds;
            let (w, h) = (b.max().x.0 - b.min().x.0, b.max().y.0 - b.min().y.0);
            if x > 0 {
                x = x.saturating_add(PIECE_GAP.0);
            }
            let shift = ew_units::Pt2::new(ew_units::Nm(x - b.min().x.0), ew_units::Nm(-b.min().y.0));
            list.push(ew_dl::Item::group(
                Some(ew_dl::Transform::from_translation(shift)),
                None,
                geometry.graphics.items,
            ));
            x = x.saturating_add(w.max(1));
            height = height.max(h);
        }
        (x > 0).then(|| (list, (ew_units::Nm(x), ew_units::Nm(height.max(1)))))
    }

    /// The bus symbol of `part` as its sheet pictures it "On a bus page" (doc 10 §2.7, doc 07 §5.4): his own where the
    /// part names one (the project's copy, else the library's), else the program's on the grid of the project's first
    /// bus page (4 mm without one); `None` for a part without network ports.
    pub fn part_bus_symbol(&self, part: &Part) -> Option<PartBusSymbol> {
        if !ew_derive::has_bus_ports(part) {
            return None;
        }
        let q = self.query();
        let own = part.bus_symbol.as_ref().and_then(|r| {
            ew_derive::own_bus_symbol(&q, part)
                .cloned()
                .or_else(|| match self.libraries().resolve(r) {
                    Ok(LibDocument::Symbol(c)) => Some(c.item),
                    _ => None,
                })
        });
        let (missing, extra) = own
            .as_ref()
            .map(|s| ew_derive::socket_mismatch(s, part))
            .unwrap_or_default();
        let grid = q
            .pages()
            .iter()
            .find(|(_, p)| p.kind == PageType::BusTopology)
            .map_or(ew_units::Grid::MM4, |(_, p)| p.grid);
        let is_own = own.is_some();
        let symbol = own.unwrap_or_else(|| ew_derive::bus_symbol(part, grid));
        let geometry = ew_scene::symbol::lower(&symbol, ew_units::Orient2::R0);
        let b = geometry.bounds;
        let shift = ew_units::Pt2::new(ew_units::Nm(-b.min().x.0), ew_units::Nm(-b.min().y.0));
        // Its texts as a page prints them, shaped by the page's means: the interface names and socket labels, the part
        // in the header, the tag and each address by its placeholder (no device is drawn).
        let styles = crate::output::text_style_table(&q);
        let mut items = geometry.graphics.items;
        for t in &geometry.texts {
            use ew_scene::symbol::TextSource;
            let content = match &t.source {
                TextSource::Static(text) => ew_derive::text_in(text, None).to_owned(),
                TextSource::Slot(ew_schema::SlotKind::Tag) => "-?".to_owned(),
                TextSource::Slot(ew_schema::SlotKind::PartNumber) => crate::output::bus_part_words(&q, part),
                TextSource::Slot(ew_schema::SlotKind::BusAddress) => "TBD".to_owned(),
                TextSource::Slot(_) => String::new(),
            };
            let key = match (&t.style_key, &t.source) {
                (Some(key), _) => key.as_str(),
                (None, TextSource::Slot(kind)) => ew_scene::symbol::default_style_key(false, kind),
                (None, TextSource::Static(_)) => ew_scene::symbol::DEFAULT_TEXT_STYLE,
            };
            let content = match t.max_width {
                Some(room) => crate::output::cut_in_style(&q, key, &content, room),
                None => content,
            };
            items.extend(
                ew_scene::page::symbol_preview_text(t, key, &content, crate::output::fonts(&q), &styles)
                    .list
                    .items,
            );
        }
        let mut list = ew_dl::DisplayList::new();
        list.push(ew_dl::Item::group(
            Some(ew_dl::Transform::from_translation(shift)),
            None,
            items,
        ));
        let size = (
            ew_units::Nm((b.max().x.0 - b.min().x.0).max(1)),
            ew_units::Nm((b.max().y.0 - b.min().y.0).max(1)),
        );
        Some(PartBusSymbol {
            list,
            size,
            symbol,
            own: is_own,
            missing,
            extra,
        })
    }
}

/// A part's bus symbol as its sheet pictures it "On a bus page" ([`Session::part_bus_symbol`]).
#[derive(Clone, Debug)]
pub struct PartBusSymbol {
    /// The picture, as a page prints the symbol: its lines and its texts, a placeholder for each value of a device.
    pub list: ew_dl::DisplayList,
    /// Its width and height.
    pub size: (ew_units::Nm, ew_units::Nm),
    /// The symbol: what Make my own from it… opens as a drawing of his own.
    pub symbol: SymbolDef,
    /// Whether it is his own (`Part.bus_symbol`).
    pub own: bool,
    /// The sockets of the part's network ports his own does not draw, in words ("X1 P3").
    pub missing: Vec<String>,
    /// The sockets his own draws that the ports no longer have, in words.
    pub extra: Vec<String>,
}

/// The starter functions a piece of a part drawn with symbols can be (doc 07 §5.4, Drawn as): each starter FnDef with
/// connection points that is not a box, a bus port or a terminal, by its key, with its pins' designations, in the
/// starter's order.
pub(crate) fn starter_piece_fndefs() -> impl Iterator<Item = (&'static str, &'static LibCopy<FnDef>)> {
    use ew_schema::ContactKind;
    starter_docs().iter().filter_map(|(key, _, d)| match d {
        LibDocument::Fndef(c)
            if !c.item.cps.is_empty()
                && !matches!(
                    c.item.contact_kind,
                    ContactKind::BlackBox | ContactKind::BusPort | ContactKind::Terminal
                ) =>
        {
            Some((*key, c))
        }
        _ => None,
    })
}

/// Every starter FnDef with pins, in the starter's order, by its key: what a symbol can stand for (doc 07 §5.6), the
/// pieces' functions ([`starter_piece_fndefs`]) and the black boxes, bus ports and terminals with pins.
pub(crate) fn starter_pinned_fndefs() -> impl Iterator<Item = (&'static str, &'static LibCopy<FnDef>)> {
    starter_docs().iter().filter_map(|(key, _, d)| match d {
        LibDocument::Fndef(c) if !c.item.cps.is_empty() => Some((*key, c)),
        _ => None,
    })
}

/// The key of the starter FnDef that is item `item` (`ew.fn.coil`).
pub(crate) fn starter_fndef_key(item: Uuid) -> Option<&'static str> {
    starter_docs().iter().find_map(|(k, _, d)| match d {
        LibDocument::Fndef(c) if c.item_id.as_uuid() == item => Some(*k),
        _ => None,
    })
}

/// The starter FnDef of key `key` (`ew.fn.coil`).
pub(crate) fn starter_fndef_by_key(key: &str) -> Option<&'static LibCopy<FnDef>> {
    starter_docs().iter().find_map(|(k, _, d)| match d {
        LibDocument::Fndef(c) if *k == key => Some(c),
        _ => None,
    })
}

/// The key of the starter symbol that draws a piece of what `key` is (a starter function's key) on a schematic page
/// on the native grid: what Change it for this part… copies (doc 07 §5.6).
pub fn starter_piece_symbol(key: &str) -> Option<String> {
    let def = starter_fndef_by_key(key)?;
    starter_docs().iter().find_map(|(k, _, d)| match d {
        LibDocument::Symbol(c)
            if c.item.rep_type == RepType::MultiLine
                && fndef_copy(&c.item.fndef) == def.id
                && !k.ends_with(ew_schema::starter::QET10_EDITION) =>
        {
            Some((*k).to_owned())
        }
        _ => None,
    })
}

/// The name of the first starter symbol that draws starter FnDef `def`, on a page of any kind, in each language it
/// has: what a person calls the function (a piece's words, doc 07 §5.4).
pub(crate) fn starter_function_name(def: Id<FnDef>) -> Option<ew_mltext::MlText> {
    starter_docs().iter().find_map(|(_, _, d)| match d {
        LibDocument::Symbol(c) if fndef_copy(&c.item.fndef) == def => Some(c.item.name.clone()),
        _ => None,
    })
}

/// The name of the starter symbol that draws starter FnDef `def` on a schematic page, in the language `lang`.
pub(crate) fn starter_symbol_name(def: Id<FnDef>, lang: &str) -> Option<String> {
    starter_docs().iter().find_map(|(_, _, d)| match d {
        LibDocument::Symbol(c)
            if c.item.rep_type == RepType::MultiLine && fndef_copy(&c.item.fndef) == def =>
        {
            let lang: Option<ew_mltext::LangTag> = lang.parse().ok();
            lang.and_then(|l| c.item.name.text_for(&l))
                .or_else(|| c.item.name.neutral_or_first())
                .map(str::to_owned)
        }
        _ => None,
    })
}
