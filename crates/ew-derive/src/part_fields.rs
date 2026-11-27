// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The declaration of a part's fields (doc 07 §2.5, §5.4, §5.7): every member of a part, with its word, the group of
//! the part's sheet it stands in, which parts have it, how a person meets it, and what reads it and does without it.
//! It is the one list of a part's members: the place for parts shows the fields of [`fields_of`], says what a part
//! lacks by [`lacks`], and a feature asks [`lacking`] instead of testing members itself. A field's kind of value is the
//! part schema's; a member added to the schema fails the guard's test in `ew-api` until it has its row here.
//!
//! A path names a member as the part's JSON form does: `mech.width`, a list's item member `cp_pattern[].csa_min`, a
//! list of values `meta.sources[]`. A reference to another part (`sections.enclosure.plate`) is one field. A named
//! technical value is `tech_data.<key>`: a key of the part's technical data that the program computes with.

use ew_l10n::{Text, TextArg};
use ew_schema::{MountKind, Part, PartKind};
use serde_json::Value;

/// The group of a part's sheet a field stands in, in the sheet's order (doc 07 §5.4): the kinds' own groups stand
/// between Connections and Technical data.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Group {
    /// The maker, the number, the kind, the description.
    Identity,
    /// How it is drawn on a schematic page: its pieces, its pins, its box.
    Schematic,
    /// Its size, how it mounts, its front and its holes.
    Cabinet,
    /// The clamps of its pins.
    Connections,
    /// A cabinet's own data.
    Enclosure,
    /// A mounting plate's own data.
    Plate,
    /// A rail's own data.
    Rail,
    /// A wiring duct's own data.
    Duct,
    /// A cable's cores and size.
    Cable,
    /// A wire's own data.
    Wire,
    /// A ferrule's or a lug's own data.
    Ferrule,
    /// A marker's own data.
    Marker,
    /// A cable gland's own data.
    Gland,
    /// A valve's coil and ports.
    Valve,
    /// A tube's own data.
    Tube,
    /// What the part is in the PLC.
    Plc,
    /// Its network ports.
    Ports,
    /// The values the program computes with, and free rows.
    Technical,
    /// What it brings and the forms it is also sold in.
    GoesWith,
    /// Where it came from.
    About,
}

impl Group {
    /// The group's word.
    pub fn word(self) -> &'static str {
        match self {
            Group::Identity => key!("part-group-identity"),
            Group::Schematic => key!("part-group-schematic"),
            Group::Cabinet => key!("part-group-cabinet"),
            Group::Connections => key!("part-group-connections"),
            Group::Enclosure => key!("part-group-enclosure"),
            Group::Plate => key!("part-group-plate"),
            Group::Rail => key!("part-group-rail"),
            Group::Duct => key!("part-group-duct"),
            Group::Cable => key!("part-group-cable"),
            Group::Wire => key!("part-group-wire"),
            Group::Ferrule => key!("part-group-ferrule"),
            Group::Marker => key!("part-group-marker"),
            Group::Gland => key!("part-group-gland"),
            Group::Valve => key!("part-group-valve"),
            Group::Tube => key!("part-group-tube"),
            Group::Plc => key!("part-group-plc"),
            Group::Ports => key!("part-group-ports"),
            Group::Technical => key!("part-group-technical"),
            Group::GoesWith => key!("part-group-goes-with"),
            Group::About => key!("part-group-about"),
        }
    }

    /// The group's name in a command or a script (`cabinet`, `goes_with`).
    pub fn name(self) -> &'static str {
        match self {
            Group::Identity => "identity",
            Group::Schematic => "schematic",
            Group::Cabinet => "cabinet",
            Group::Connections => "connections",
            Group::Enclosure => "enclosure",
            Group::Plate => "plate",
            Group::Rail => "rail",
            Group::Duct => "duct",
            Group::Cable => "cable",
            Group::Wire => "wire",
            Group::Ferrule => "ferrule",
            Group::Marker => "marker",
            Group::Gland => "gland",
            Group::Valve => "valve",
            Group::Tube => "tube",
            Group::Plc => "plc",
            Group::Ports => "ports",
            Group::Technical => "technical",
            Group::GoesWith => "goes_with",
            Group::About => "about",
        }
    }

    /// The group named `name` ([`Group::name`]) among those a field stands in.
    pub fn named(name: &str) -> Option<Group> {
        PART_FIELDS.iter().map(|f| f.group).find(|g| g.name() == name)
    }

    /// The group the field at `path` stands in: what a check's Edit part… names to open the sheet where the person
    /// enters what it lacks.
    pub fn of(path: &str) -> Option<Group> {
        PART_FIELDS.iter().find(|f| f.path == path).map(|f| f.group)
    }
}

/// Which parts have a field, or which parts a reader reads it of: a predicate over the part.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Has {
    /// Every part.
    Every,
    /// The parts of these kinds.
    Kinds(&'static [PartKind]),
    /// A part that stands in a cabinet with a size of its own: every kind but those a wire consumes, a cable, a tube,
    /// a cabinet, a plate, a rail and a gland, whose sizes are their own groups'.
    Mounted,
    /// A mounted part that is placed on a rail or a surface by itself: a mounted part but a duct and an accessory.
    Placed,
    /// A part that needs holes made for it: it mounts in a cut-out of a door or a wall, or is screwed on.
    Cutout,
    /// A part whose devices have pins: a device, a terminal, a busbar, an accessory, a configured product.
    Pinned,
    /// A part that has pins now.
    HasPins,
    /// A part that is drawn on a schematic page: a device, a terminal, a configured product.
    Drawn,
    /// A part with a pneumatic section (a valve, a valve terminal, a regulator).
    Fluid,
    /// A part with solenoids in its pneumatic section: a solenoid valve.
    Solenoids,
    /// A part with a PLC section: a module or a station.
    Plc,
}

impl Has {
    /// Whether part `p` has it.
    pub fn holds(self, p: &Part) -> bool {
        use PartKind as K;
        match self {
            Has::Every => true,
            Has::Kinds(kinds) => kinds.contains(&p.kind),
            Has::Mounted => !matches!(
                p.kind,
                K::Wire
                    | K::Ferrule
                    | K::Marker
                    | K::Cable
                    | K::Tube
                    | K::Enclosure
                    | K::MountingPlate
                    | K::Rail
                    | K::Gland
            ),
            // An accessory (a plug, a bridge, an end plate) is placed with what it goes with, never on its own.
            Has::Placed => Has::Mounted.holds(p) && !matches!(p.kind, K::Duct | K::Accessory),
            Has::Cutout => matches!(
                p.mech.as_ref().and_then(|m| m.mount_kind.as_ref()),
                Some(MountKind::DoorCutout | MountKind::Screw)
            ),
            Has::Pinned => matches!(
                p.kind,
                K::Device | K::Terminal | K::Busbar | K::Accessory | K::Configured
            ),
            Has::HasPins => !p.cp_pattern.is_empty(),
            Has::Drawn => matches!(p.kind, K::Device | K::Terminal | K::Configured),
            Has::Fluid => p.sections.fluid.is_some(),
            Has::Solenoids => p.sections.fluid.as_ref().is_some_and(|f| !f.solenoids.is_empty()),
            Has::Plc => p.sections.plc.is_some(),
        }
    }
}

/// A form of the sheet that writes a member together with others: a table or a drawing of its own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Form {
    /// The maker, typed by name and offered from the makers known.
    Maker,
    /// Drawn as: the pieces with their pins and symbols.
    Pieces,
    /// The pins, and a terminal's levels and clamps.
    PinGrid,
    /// The device box.
    Box,
    /// A cable's cores.
    Cores,
    /// The network ports.
    Ports,
    /// The PLC channels.
    Channels,
    /// The holes of the front.
    Holes,
    /// The free rows of technical data.
    Technical,
}

/// How a person meets a field.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Entry {
    /// One value typed or chosen; its kind of value is the schema's.
    Field,
    /// A column of the table over the list member named.
    Column(&'static str),
    /// A member that a form writes.
    Form(Form),
    /// Derived or brought, shown and not typed.
    Shown,
    /// Nothing reads it: not asked for, shown under Also stored when it has a value; why, by key.
    Stored(&'static str),
}

/// What a value must be beyond what the schema's type says.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Limit {
    /// Nothing more.
    None,
    /// A length above zero (a size; a clearance may be zero).
    AboveZero,
}

/// What a feature does without a field.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Without {
    /// It cannot use the part.
    Refused,
    /// A check passes the part by, or a list leaves a cell empty.
    Skipped,
    /// A default is taken, and the sentence names it.
    Assumed,
    /// It is only printed or shown; nothing is said while it is empty, and the sentence says what it is for.
    Printed,
}

/// A feature of the program that reads a part.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Feature {
    /// The parts list.
    PartsList,
    /// Choosing a part for a device.
    PartChoice,
    /// Finding a part.
    Search,
    /// Where a part came from.
    About,
    /// Placing its pieces on a schematic page.
    Placing,
    /// The contact image under a coil.
    ContactImage,
    /// The potentials its pins carry.
    Potentials,
    /// The terminal diagram's levels.
    Terminals,
    /// Its footprint on the cabinet layout.
    Footprint,
    /// Whether it fits a rail.
    RailFit,
    /// What is kept free round it.
    Clearance,
    /// The doors and walls it stands in.
    Surfaces,
    /// What it takes behind a door.
    DoorReach,
    /// The hole table and the drawings for drilling.
    HoleTable,
    /// The part card.
    PartCard,
    /// The cabinet's outline and its faces.
    Cabinet,
    /// The cabinet's plate.
    PlateFrame,
    /// Drawing a duct.
    DuctSize,
    /// The treatment of the wire ends.
    WireEnds,
    /// The check that a wire fits its clamp.
    ClampFit,
    /// The ferrule a wire end gets.
    FerruleChoice,
    /// The labels a marker takes.
    MarkerFit,
    /// The wire part a wire matches.
    StockMatch,
    /// Drawing a cable with its cores.
    CableCores,
    /// The gland a cable gets.
    GlandChoice,
    /// The network pages.
    Bus,
    /// The PLC configuration.
    Plc,
    /// The load on a PLC module's outputs.
    OutputLoad,
    /// Sizing the wires a protective device protects.
    WireSizing,
    /// The load a PLC output drives.
    SupplyLoad,
    /// The assembly list.
    AssemblyList,
    /// What a part brings with it.
    Accessories,
    /// The pneumatic pages.
    Fluid,
    /// The check of a tube against its ports and its pressure.
    TubeFit,
    /// The check of a coil against its output.
    CoilCheck,
    /// How full a wiring duct is (EW-LAY-007).
    DuctFill,
    /// The way of a wire from a pin (which edge it leaves by).
    WireWay,
}

impl Feature {
    /// The feature's word (Ready for's row).
    pub fn word(self) -> &'static str {
        match self {
            Feature::PartsList => key!("part-feature-parts-list"),
            Feature::PartChoice => key!("part-feature-part-choice"),
            Feature::Search => key!("part-feature-search"),
            Feature::About => key!("part-feature-about"),
            Feature::Placing => key!("part-feature-placing"),
            Feature::ContactImage => key!("part-feature-contact-image"),
            Feature::Potentials => key!("part-feature-potentials"),
            Feature::Terminals => key!("part-feature-terminals"),
            Feature::Footprint => key!("part-feature-footprint"),
            Feature::RailFit => key!("part-feature-rail-fit"),
            Feature::Clearance => key!("part-feature-clearance"),
            Feature::Surfaces => key!("part-feature-surfaces"),
            Feature::DoorReach => key!("part-feature-door-reach"),
            Feature::HoleTable => key!("part-feature-hole-table"),
            Feature::PartCard => key!("part-feature-part-card"),
            Feature::Cabinet => key!("part-feature-cabinet"),
            Feature::PlateFrame => key!("part-feature-plate-frame"),
            Feature::DuctSize => key!("part-feature-duct-size"),
            Feature::WireEnds => key!("part-feature-wire-ends"),
            Feature::ClampFit => key!("part-feature-clamp-fit"),
            Feature::FerruleChoice => key!("part-feature-ferrule-choice"),
            Feature::MarkerFit => key!("part-feature-marker-fit"),
            Feature::StockMatch => key!("part-feature-stock-match"),
            Feature::CableCores => key!("part-feature-cable-cores"),
            Feature::GlandChoice => key!("part-feature-gland-choice"),
            Feature::Bus => key!("part-feature-bus"),
            Feature::Plc => key!("part-feature-plc"),
            Feature::OutputLoad => key!("part-feature-output-load"),
            Feature::WireSizing => key!("part-feature-wire-sizing"),
            Feature::SupplyLoad => key!("part-feature-supply-load"),
            Feature::AssemblyList => key!("part-feature-assembly-list"),
            Feature::Accessories => key!("part-feature-accessories"),
            Feature::Fluid => key!("part-feature-fluid"),
            Feature::TubeFit => key!("part-feature-tube-fit"),
            Feature::CoilCheck => key!("part-feature-coil-check"),
            Feature::DuctFill => key!("part-feature-duct-fill"),
            Feature::WireWay => key!("part-feature-wire-way"),
        }
    }
}

/// A feature that reads a field: what it does without it and the sentence that says so (or, for [`Without::Printed`],
/// what the field is for), and which parts it reads it of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Reader {
    /// The feature.
    pub feature: Feature,
    /// What it does without the field.
    pub without: Without,
    /// Its sentence, by key; `{ $p0 }`, where the sentence has it, names the fields lacking.
    pub says: &'static str,
    /// The parts it reads the field of (beside the field's own [`PartField::has`]).
    pub on: Has,
}

/// One field of a part.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PartField {
    /// The member's path.
    pub path: &'static str,
    /// Its word, by key.
    pub word: &'static str,
    /// The group of the sheet it stands in.
    pub group: Group,
    /// Which parts have it.
    pub has: Has,
    /// How a person meets it.
    pub entry: Entry,
    /// What its value must be beyond its type.
    pub limit: Limit,
    /// What reads it; empty only for [`Entry::Stored`].
    pub readers: &'static [Reader],
}

/// What a part lacks for one feature: the reader's sentence and the fields it lacks, in the declaration's order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Lack {
    /// The feature.
    pub feature: Feature,
    /// What it does without them.
    pub without: Without,
    /// Its sentence, by key.
    pub says: &'static str,
    /// The fields lacking.
    pub fields: Vec<&'static PartField>,
}

impl Lack {
    /// The sentence, the fields lacking joined in the reader's language where it names them.
    pub fn text(&self) -> Text {
        let words = TextArg::List(ew_l10n::List::new(
            self.fields
                .iter()
                .map(|f| TextArg::Text(word_in(f.word)))
                .collect(),
            text!("part-fields-between"),
            text!("part-fields-and"),
        ));
        sentence(self.says, words)
    }
}

/// The text of a declared key (a word, a group's, a feature's): checked when the declaration was built.
pub fn word(key: &'static str) -> Text {
    Text::checked_by_macro(key, crate::text_english(key).unwrap_or("?"), Vec::new())
}

/// The word of a field as a sentence takes it (`width`, not `Width`): its key's `-in` form, which the text files give
/// beside every field's label, else the label.
pub fn word_in(key: &'static str) -> Text {
    let wanted = format!("{key}-in");
    match crate::KEYS.iter().find(|k| **k == wanted) {
        Some(k) => word(k),
        None => word(key),
    }
}

/// The sentence `says` of a reader, `fields` its argument where its English takes one.
fn sentence(says: &'static str, fields: TextArg) -> Text {
    let english = crate::text_english(says).unwrap_or("?");
    let args = if ew_l10n::arity(english) > 0 {
        vec![fields]
    } else {
        Vec::new()
    };
    Text::checked_by_macro(says, english, args)
}

/// A reader of `feature` that does `without` and says `says`, of every part that has the field.
const fn r(feature: Feature, without: Without, says: &'static str) -> Reader {
    Reader {
        feature,
        without,
        says,
        on: Has::Every,
    }
}

/// A reader as [`r`], of the parts `on` only.
const fn r_on(feature: Feature, without: Without, says: &'static str, on: Has) -> Reader {
    Reader {
        feature,
        without,
        says,
        on,
    }
}

/// A field.
const fn f(
    path: &'static str,
    word: &'static str,
    group: Group,
    has: Has,
    entry: Entry,
    readers: &'static [Reader],
) -> PartField {
    PartField {
        path,
        word,
        group,
        has,
        entry,
        limit: Limit::None,
        readers,
    }
}

/// A length that must be above zero.
const fn size(
    path: &'static str,
    word: &'static str,
    group: Group,
    has: Has,
    readers: &'static [Reader],
) -> PartField {
    PartField {
        path,
        word,
        group,
        has,
        entry: Entry::Field,
        limit: Limit::AboveZero,
        readers,
    }
}

/// A member nothing reads: not asked for.
const fn stored(
    path: &'static str,
    word: &'static str,
    group: Group,
    has: Has,
    why: &'static str,
) -> PartField {
    PartField {
        path,
        word,
        group,
        has,
        entry: Entry::Stored(why),
        limit: Limit::None,
        readers: &[],
    }
}

use Entry::{Column, Field, Form as F, Shown};
use Feature as Ft;
use Group as G;
use Without::{Assumed, Printed, Refused, Skipped};

const LIST: &[Reader] = &[r(Ft::PartsList, Printed, key!("part-says-printed-list"))];
const IDENTITY: &[Reader] = &[r(Ft::PartsList, Refused, key!("part-says-identity"))];
const ABOUT: &[Reader] = &[r(Ft::About, Printed, key!("part-says-about"))];
const PIECES: &[Reader] = &[r(Ft::Placing, Refused, key!("part-says-pieces"))];
const CONTACTS: &[Reader] = &[r(Ft::ContactImage, Printed, key!("part-says-contacts"))];
const PINS: &[Reader] = &[r(Ft::Placing, Printed, key!("part-says-pins"))];
const LEVELS: &[Reader] = &[r(Ft::Terminals, Printed, key!("part-says-levels"))];
const BOX: &[Reader] = &[r(Ft::Placing, Printed, key!("part-says-box"))];
const SIZE: &[Reader] = &[
    r_on(Ft::Footprint, Assumed, key!("part-says-footprint"), Has::Placed),
    r_on(
        Ft::DuctSize,
        Assumed,
        key!("part-says-duct-size"),
        Has::Kinds(&[PartKind::Duct]),
    ),
];
const CLEARANCE: &[Reader] = &[r(Ft::Clearance, Printed, key!("part-says-clearance"))];
const FRONT: &[Reader] = &[r(Ft::Surfaces, Printed, key!("part-says-front"))];
const HOLES: &[Reader] = &[r_on(Ft::HoleTable, Skipped, key!("part-says-holes"), Has::Cutout)];
const CLAMP_RANGE: &[Reader] = &[r_on(
    Ft::ClampFit,
    Skipped,
    key!("part-says-clamp-range"),
    Has::HasPins,
)];
const CABINET: &[Reader] = &[r(Ft::Cabinet, Skipped, key!("part-says-cabinet"))];
const DOORS: &[Reader] = &[r(Ft::Surfaces, Assumed, key!("part-says-doors"))];
const PLATE: &[Reader] = &[r(Ft::PlateFrame, Refused, key!("part-says-plate-size"))];
const CORES: &[Reader] = &[r(Ft::CableCores, Refused, key!("part-says-cores"))];
const CORE_DATA: &[Reader] = &[r(Ft::CableCores, Printed, key!("part-says-core-data"))];
const WIRE: &[Reader] = &[r(Ft::StockMatch, Refused, key!("part-says-wire"))];
const FERRULE: &[Reader] = &[r(Ft::FerruleChoice, Refused, key!("part-says-ferrule"))];
const FERRULE_DATA: &[Reader] = &[r(Ft::WireEnds, Printed, key!("part-says-ferrule-data"))];
const MARKER_SIZE: &[Reader] = &[r(Ft::MarkerFit, Skipped, key!("part-says-marker-size"))];
const GLAND: &[Reader] = &[r(Ft::GlandChoice, Refused, key!("part-says-gland"))];
/// The coil check reads the coil of a valve with solenoids (EW-FL-009), not of a cylinder or a regulator.
const COIL: &[Reader] = &[r_on(
    Ft::CoilCheck,
    Skipped,
    key!("part-says-coil"),
    Has::Solenoids,
)];
const FLUID: &[Reader] = &[r(Ft::Fluid, Printed, key!("part-says-fluid"))];
const TUBE: &[Reader] = &[r(Ft::TubeFit, Skipped, key!("part-says-tube"))];
const TUBE_DATA: &[Reader] = &[r(Ft::TubeFit, Printed, key!("part-says-tube-data"))];
const PLC: &[Reader] = &[r(Ft::Plc, Printed, key!("part-says-plc"))];
/// The output load check reads the ratings of a PLC module or station, not of every device.
const RATINGS: &[Reader] = &[r_on(Ft::OutputLoad, Skipped, key!("part-says-ratings"), Has::Plc)];
const BUS: &[Reader] = &[r(Ft::Bus, Printed, key!("part-says-bus"))];
const TECH: &[Reader] = &[r(Ft::PartsList, Printed, key!("part-says-tech"))];
const LOAD: &[Reader] = &[r(Ft::SupplyLoad, Printed, key!("part-says-load"))];
const ACCESSORIES: &[Reader] = &[r(Ft::Accessories, Printed, key!("part-says-accessories"))];
const VARIANTS: &[Reader] = &[r(Ft::PartsList, Printed, key!("part-says-variants"))];
const CONFIGURED: &[Reader] = &[r(Ft::PartsList, Printed, key!("part-says-configured"))];

const NOT_READ: &str = key!("part-stored-not-read");
const NOT_YET: &str = key!("part-stored-not-yet");

const ENCLOSURE: Has = Has::Kinds(&[PartKind::Enclosure]);
const PLATE_KIND: Has = Has::Kinds(&[PartKind::MountingPlate]);
const RAIL: Has = Has::Kinds(&[PartKind::Rail]);
const DUCT: Has = Has::Kinds(&[PartKind::Duct]);
const CABLE: Has = Has::Kinds(&[PartKind::Cable]);
const WIRE_KIND: Has = Has::Kinds(&[PartKind::Wire]);
const FERRULE_KIND: Has = Has::Kinds(&[PartKind::Ferrule]);
const MARKER: Has = Has::Kinds(&[PartKind::Marker]);
const GLAND_KIND: Has = Has::Kinds(&[PartKind::Gland]);
const TUBE_KIND: Has = Has::Kinds(&[PartKind::Tube]);
const TERMINAL: Has = Has::Kinds(&[PartKind::Terminal]);
const DEVICE: Has = Has::Kinds(&[PartKind::Device]);
const BUS_KINDS: Has = Has::Kinds(&[PartKind::Device, PartKind::Accessory]);
const CONFIGURED_KIND: Has = Has::Kinds(&[PartKind::Configured]);

/// Every field of a part, group by group in the sheet's order.
pub static PART_FIELDS: &[PartField] = &[
    // Identity
    f(
        "kind",
        key!("part-field-kind"),
        G::Identity,
        Has::Every,
        Field,
        &[r(Ft::PartChoice, Printed, key!("part-says-kind"))],
    ),
    f(
        "manufacturer",
        key!("part-field-manufacturer"),
        G::Identity,
        Has::Every,
        F(Form::Maker),
        IDENTITY,
    ),
    f(
        "part_number",
        key!("part-field-part-number"),
        G::Identity,
        Has::Every,
        Field,
        IDENTITY,
    ),
    f(
        "type_designation",
        key!("part-field-type-designation"),
        G::Identity,
        Has::Every,
        Field,
        LIST,
    ),
    f(
        "order_number",
        key!("part-field-order-number"),
        G::Identity,
        Has::Every,
        Field,
        LIST,
    ),
    f(
        "gtin",
        key!("part-field-gtin"),
        G::Identity,
        Has::Every,
        Field,
        &[r(Ft::Search, Printed, key!("part-says-gtin"))],
    ),
    f(
        "description",
        key!("part-field-description"),
        G::Identity,
        Has::Every,
        Field,
        LIST,
    ),
    f(
        "category",
        key!("part-field-category"),
        G::Identity,
        Has::Every,
        Field,
        &[r(Ft::PartChoice, Printed, key!("part-says-category"))],
    ),
    f(
        "unit",
        key!("part-field-unit"),
        G::Identity,
        Has::Every,
        Field,
        LIST,
    ),
    // On the schematic
    f(
        "function_template[].fndef",
        key!("part-field-function-template-fndef"),
        G::Schematic,
        Has::Drawn,
        F(Form::Pieces),
        PIECES,
    ),
    f(
        "function_template[].main",
        key!("part-field-function-template-main"),
        G::Schematic,
        Has::Drawn,
        F(Form::Pieces),
        // A flag: absent is "not the main piece", never a lack.
        &[r(Ft::Placing, Printed, key!("part-says-main"))],
    ),
    f(
        "function_template[].cps[]",
        key!("part-field-function-template-cps"),
        G::Schematic,
        Has::Drawn,
        F(Form::Pieces),
        // A piece without pins (a cylinder's, an air service unit's) is placed as it is: nothing lacks.
        PINS,
    ),
    f(
        "contacts[].kind",
        key!("part-field-contacts-kind"),
        G::Schematic,
        Has::Drawn,
        F(Form::Pieces),
        CONTACTS,
    ),
    f(
        "contacts[].cps[]",
        key!("part-field-contacts-cps"),
        G::Schematic,
        Has::Drawn,
        F(Form::Pieces),
        CONTACTS,
    ),
    f(
        "contacts[].seq",
        key!("part-field-contacts-seq"),
        G::Schematic,
        Has::Drawn,
        F(Form::Pieces),
        CONTACTS,
    ),
    f(
        "contacts[].special",
        key!("part-field-contacts-special"),
        G::Schematic,
        Has::Drawn,
        F(Form::Pieces),
        CONTACTS,
    ),
    f(
        "cp_pattern[].idx",
        key!("part-field-cp-pattern-idx"),
        G::Schematic,
        Has::Pinned,
        Shown,
        PINS,
    ),
    f(
        "cp_pattern[].designation",
        key!("part-field-cp-pattern-designation"),
        G::Schematic,
        Has::Pinned,
        F(Form::PinGrid),
        PINS,
    ),
    f(
        "cp_pattern[].potential_role",
        key!("part-field-cp-pattern-potential-role"),
        G::Schematic,
        Has::Pinned,
        F(Form::PinGrid),
        &[r(Ft::Potentials, Printed, key!("part-says-carries"))],
    ),
    f(
        "cp_pattern[].level",
        key!("part-field-cp-pattern-level"),
        G::Schematic,
        TERMINAL,
        F(Form::PinGrid),
        LEVELS,
    ),
    f(
        "cp_pattern[].side",
        key!("part-field-cp-pattern-side"),
        G::Schematic,
        TERMINAL,
        F(Form::PinGrid),
        LEVELS,
    ),
    f(
        "cp_pattern[].clamp",
        key!("part-field-cp-pattern-clamp"),
        G::Schematic,
        TERMINAL,
        F(Form::PinGrid),
        LEVELS,
    ),
    f(
        "box.width",
        key!("part-field-box-width"),
        G::Schematic,
        Has::Drawn,
        F(Form::Box),
        BOX,
    ),
    f(
        "box.height",
        key!("part-field-box-height"),
        G::Schematic,
        Has::Drawn,
        F(Form::Box),
        BOX,
    ),
    f(
        "box.pins[].idx",
        key!("part-field-box-pins-idx"),
        G::Schematic,
        Has::Drawn,
        F(Form::Box),
        BOX,
    ),
    f(
        "box.pins[].side",
        key!("part-field-box-pins-side"),
        G::Schematic,
        Has::Drawn,
        F(Form::Box),
        BOX,
    ),
    f(
        "box.pins[].at",
        key!("part-field-box-pins-at"),
        G::Schematic,
        Has::Drawn,
        F(Form::Box),
        BOX,
    ),
    // In the cabinet
    size(
        "mech.width",
        key!("part-field-mech-width"),
        G::Cabinet,
        Has::Mounted,
        SIZE,
    ),
    size(
        "mech.height",
        key!("part-field-mech-height"),
        G::Cabinet,
        Has::Mounted,
        SIZE,
    ),
    size(
        "mech.depth",
        key!("part-field-mech-depth"),
        G::Cabinet,
        Has::Placed,
        &[r_on(Ft::DoorReach, Skipped, key!("part-says-depth"), Has::Placed)],
    ),
    f(
        "mech.mount_kind",
        key!("part-field-mech-mount-kind"),
        G::Cabinet,
        Has::Placed,
        Field,
        &[r(Ft::RailFit, Skipped, key!("part-says-mount"))],
    ),
    f(
        "mech.mass",
        key!("part-field-mech-mass"),
        G::Cabinet,
        Has::Placed,
        Field,
        &[r(Ft::PartCard, Printed, key!("part-says-mass"))],
    ),
    f(
        "mech.clearance.top",
        key!("part-field-mech-clearance-top"),
        G::Cabinet,
        Has::Placed,
        Field,
        CLEARANCE,
    ),
    f(
        "mech.clearance.bottom",
        key!("part-field-mech-clearance-bottom"),
        G::Cabinet,
        Has::Placed,
        Field,
        CLEARANCE,
    ),
    f(
        "mech.clearance.left",
        key!("part-field-mech-clearance-left"),
        G::Cabinet,
        Has::Placed,
        Field,
        CLEARANCE,
    ),
    f(
        "mech.clearance.right",
        key!("part-field-mech-clearance-right"),
        G::Cabinet,
        Has::Placed,
        Field,
        CLEARANCE,
    ),
    size(
        "mech.clearance.front",
        key!("part-field-mech-clearance-front"),
        G::Cabinet,
        Has::Placed,
        &[r(Ft::DoorReach, Assumed, key!("part-says-clearance-front"))],
    ),
    f(
        "mech.rail_offset",
        key!("part-field-mech-rail-offset"),
        G::Cabinet,
        Has::Placed,
        Shown,
        &[r(Ft::Footprint, Printed, key!("part-says-rail-line"))],
    ),
    f(
        "mech.front_shape",
        key!("part-field-mech-front-shape"),
        G::Cabinet,
        Has::Placed,
        Field,
        FRONT,
    ),
    f(
        "mech.front_2d[][]",
        key!("part-field-mech-front-2d"),
        G::Cabinet,
        Has::Placed,
        Shown,
        FRONT,
    ),
    f(
        "mech.holes[].x",
        key!("part-field-mech-holes-x"),
        G::Cabinet,
        Has::Placed,
        F(Form::Holes),
        HOLES,
    ),
    f(
        "mech.holes[].y",
        key!("part-field-mech-holes-y"),
        G::Cabinet,
        Has::Placed,
        F(Form::Holes),
        HOLES,
    ),
    f(
        "mech.holes[].hole",
        key!("part-field-mech-holes-hole"),
        G::Cabinet,
        Has::Placed,
        F(Form::Holes),
        HOLES,
    ),
    // Connections
    f(
        "cp_pattern[].clamp_type",
        key!("part-field-cp-pattern-clamp-type"),
        G::Connections,
        Has::Pinned,
        Column("cp_pattern"),
        &[r_on(
            Ft::WireEnds,
            Skipped,
            key!("part-says-clamp-type"),
            Has::HasPins,
        )],
    ),
    f(
        "cp_pattern[].csa_min",
        key!("part-field-cp-pattern-csa-min"),
        G::Connections,
        Has::Pinned,
        Column("cp_pattern"),
        CLAMP_RANGE,
    ),
    f(
        "cp_pattern[].csa_max",
        key!("part-field-cp-pattern-csa-max"),
        G::Connections,
        Has::Pinned,
        Column("cp_pattern"),
        CLAMP_RANGE,
    ),
    f(
        "cp_pattern[].max_conductors",
        key!("part-field-cp-pattern-max-conductors"),
        G::Connections,
        Has::Pinned,
        Column("cp_pattern"),
        &[r_on(
            Ft::ClampFit,
            Skipped,
            key!("part-says-clamp-wires"),
            Has::HasPins,
        )],
    ),
    f(
        "cp_pattern[].twin_ferrule",
        key!("part-field-cp-pattern-twin-ferrule"),
        G::Connections,
        Has::Pinned,
        Column("cp_pattern"),
        &[r(Ft::WireEnds, Printed, key!("part-says-twin"))],
    ),
    f(
        "cp_pattern[].face",
        key!("part-field-cp-pattern-face"),
        G::Connections,
        Has::Pinned,
        Column("cp_pattern"),
        &[r_on(Ft::WireWay, Assumed, key!("part-says-face"), Has::HasPins)],
    ),
    // Enclosure
    size(
        "sections.enclosure.width",
        key!("part-field-enclosure-width"),
        G::Enclosure,
        ENCLOSURE,
        CABINET,
    ),
    size(
        "sections.enclosure.height",
        key!("part-field-enclosure-height"),
        G::Enclosure,
        ENCLOSURE,
        CABINET,
    ),
    size(
        "sections.enclosure.depth",
        key!("part-field-enclosure-depth"),
        G::Enclosure,
        ENCLOSURE,
        &[r(Ft::Surfaces, Skipped, key!("part-says-cabinet-depth"))],
    ),
    f(
        "sections.enclosure.plate",
        key!("part-field-enclosure-plate"),
        G::Enclosure,
        ENCLOSURE,
        Field,
        &[r(Ft::PlateFrame, Refused, key!("part-says-plate"))],
    ),
    f(
        "sections.enclosure.doors[].hinge",
        key!("part-field-enclosure-doors-hinge"),
        G::Enclosure,
        ENCLOSURE,
        Column("sections.enclosure.doors"),
        DOORS,
    ),
    f(
        "sections.enclosure.doors[].width",
        key!("part-field-enclosure-doors-width"),
        G::Enclosure,
        ENCLOSURE,
        Column("sections.enclosure.doors"),
        &[r(Ft::Surfaces, Printed, key!("part-says-front"))],
    ),
    f(
        "sections.enclosure.edge_margin",
        key!("part-field-enclosure-edge-margin"),
        G::Enclosure,
        ENCLOSURE,
        Field,
        &[r(Ft::Surfaces, Assumed, key!("part-says-edge-margin"))],
    ),
    size(
        "sections.enclosure.usable_depth",
        key!("part-field-enclosure-usable-depth"),
        G::Enclosure,
        ENCLOSURE,
        &[r(Ft::DoorReach, Skipped, key!("part-says-usable-depth"))],
    ),
    stored(
        "sections.enclosure.wall",
        key!("part-field-enclosure-wall"),
        G::Enclosure,
        ENCLOSURE,
        NOT_READ,
    ),
    // Mounting plate
    size(
        "sections.mounting_plate.width",
        key!("part-field-plate-width"),
        G::Plate,
        PLATE_KIND,
        PLATE,
    ),
    size(
        "sections.mounting_plate.height",
        key!("part-field-plate-height"),
        G::Plate,
        PLATE_KIND,
        PLATE,
    ),
    stored(
        "sections.mounting_plate.thickness",
        key!("part-field-plate-thickness"),
        G::Plate,
        PLATE_KIND,
        NOT_READ,
    ),
    // Rail
    f(
        "sections.rail.profile",
        key!("part-field-rail-profile"),
        G::Rail,
        RAIL,
        Field,
        &[r(Ft::RailFit, Refused, key!("part-says-rail-profile"))],
    ),
    stored(
        "sections.rail.stock_length",
        key!("part-field-rail-stock-length"),
        G::Rail,
        RAIL,
        NOT_READ,
    ),
    // Wiring duct
    size(
        "sections.duct.inner_width",
        key!("part-field-duct-inner-width"),
        G::Duct,
        DUCT,
        &[r(Ft::DuctFill, Assumed, key!("part-says-duct-inner"))],
    ),
    size(
        "sections.duct.inner_height",
        key!("part-field-duct-inner-height"),
        G::Duct,
        DUCT,
        &[r(Ft::DuctFill, Assumed, key!("part-says-duct-inner"))],
    ),
    stored(
        "sections.duct.slot_pitch",
        key!("part-field-duct-slot-pitch"),
        G::Duct,
        DUCT,
        NOT_READ,
    ),
    // Cable
    f(
        "sections.cable_cores[].core",
        key!("part-field-cable-cores-core"),
        G::Cable,
        CABLE,
        F(Form::Cores),
        CORES,
    ),
    f(
        "sections.cable_cores[].colour",
        key!("part-field-cable-cores-colour"),
        G::Cable,
        CABLE,
        F(Form::Cores),
        CORE_DATA,
    ),
    f(
        "sections.cable_cores[].csa",
        key!("part-field-cable-cores-csa"),
        G::Cable,
        CABLE,
        F(Form::Cores),
        CORE_DATA,
    ),
    f(
        "sections.cable_cores[].shield",
        key!("part-field-cable-cores-shield"),
        G::Cable,
        CABLE,
        F(Form::Cores),
        CORE_DATA,
    ),
    f(
        "sections.cable_cores[].pair",
        key!("part-field-cable-cores-pair"),
        G::Cable,
        CABLE,
        F(Form::Cores),
        CORE_DATA,
    ),
    size(
        "sections.cable.outer_diameter",
        key!("part-field-cable-outer-diameter"),
        G::Cable,
        CABLE,
        &[r(Ft::GlandChoice, Skipped, key!("part-says-cable-diameter"))],
    ),
    f(
        "sections.bus.cable_for[]",
        key!("part-field-bus-cable-for"),
        G::Cable,
        CABLE,
        Field,
        &[r(Ft::Bus, Printed, key!("part-says-cable-for"))],
    ),
    // Wire
    f(
        "sections.wire.csa",
        key!("part-field-wire-csa"),
        G::Wire,
        WIRE_KIND,
        Field,
        WIRE,
    ),
    f(
        "sections.wire.colour",
        key!("part-field-wire-colour"),
        G::Wire,
        WIRE_KIND,
        Field,
        WIRE,
    ),
    f(
        "sections.wire.class",
        key!("part-field-wire-class"),
        G::Wire,
        WIRE_KIND,
        Field,
        &[r(Ft::WireEnds, Assumed, key!("part-says-wire-class"))],
    ),
    size(
        "sections.wire.outer_diameter",
        key!("part-field-wire-outer-diameter"),
        G::Wire,
        WIRE_KIND,
        &[r(Ft::DuctFill, Assumed, key!("part-says-wire-diameter"))],
    ),
    // Ferrule or lug
    f(
        "sections.ferrule.csa_min",
        key!("part-field-ferrule-csa-min"),
        G::Ferrule,
        FERRULE_KIND,
        Field,
        FERRULE,
    ),
    f(
        "sections.ferrule.csa_max",
        key!("part-field-ferrule-csa-max"),
        G::Ferrule,
        FERRULE_KIND,
        Field,
        FERRULE,
    ),
    f(
        "sections.ferrule.twin",
        key!("part-field-ferrule-twin"),
        G::Ferrule,
        FERRULE_KIND,
        Field,
        FERRULE_DATA,
    ),
    size(
        "sections.ferrule.sleeve",
        key!("part-field-ferrule-sleeve"),
        G::Ferrule,
        FERRULE_KIND,
        &[r(Ft::WireEnds, Skipped, key!("part-says-sleeve"))],
    ),
    f(
        "sections.ferrule.colour",
        key!("part-field-ferrule-colour"),
        G::Ferrule,
        FERRULE_KIND,
        Field,
        FERRULE_DATA,
    ),
    f(
        "sections.ferrule.stud",
        key!("part-field-ferrule-stud"),
        G::Ferrule,
        FERRULE_KIND,
        Field,
        FERRULE_DATA,
    ),
    stored(
        "sections.ferrule.insulated",
        key!("part-field-ferrule-insulated"),
        G::Ferrule,
        FERRULE_KIND,
        NOT_READ,
    ),
    // Marker
    f(
        "sections.marker.marks",
        key!("part-field-marker-marks"),
        G::Marker,
        MARKER,
        Field,
        &[r(Ft::MarkerFit, Refused, key!("part-says-marker"))],
    ),
    f(
        "sections.marker.lines",
        key!("part-field-marker-lines"),
        G::Marker,
        MARKER,
        Field,
        MARKER_SIZE,
    ),
    f(
        "sections.marker.chars",
        key!("part-field-marker-chars"),
        G::Marker,
        MARKER,
        Field,
        MARKER_SIZE,
    ),
    // Cable gland
    f(
        "sections.gland.thread",
        key!("part-field-gland-thread"),
        G::Gland,
        GLAND_KIND,
        Field,
        GLAND,
    ),
    size(
        "sections.gland.clamp_min",
        key!("part-field-gland-clamp-min"),
        G::Gland,
        GLAND_KIND,
        GLAND,
    ),
    size(
        "sections.gland.clamp_max",
        key!("part-field-gland-clamp-max"),
        G::Gland,
        GLAND_KIND,
        GLAND,
    ),
    size(
        "sections.gland.nut",
        key!("part-field-gland-nut"),
        G::Gland,
        GLAND_KIND,
        &[r(Ft::GlandChoice, Assumed, key!("part-says-gland-nut"))],
    ),
    // Valve
    f(
        "sections.fluid.coil.voltage",
        key!("part-field-fluid-coil-voltage"),
        G::Valve,
        Has::Fluid,
        Field,
        COIL,
    ),
    f(
        "sections.fluid.coil.env",
        key!("part-field-fluid-coil-env"),
        G::Valve,
        Has::Fluid,
        Field,
        COIL,
    ),
    stored(
        "sections.fluid.coil.power_w",
        key!("part-field-fluid-coil-power"),
        G::Valve,
        Has::Fluid,
        NOT_YET,
    ),
    f(
        "sections.fluid.ports[].designation",
        key!("part-field-fluid-ports-designation"),
        G::Valve,
        Has::Fluid,
        Column("sections.fluid.ports"),
        FLUID,
    ),
    f(
        "sections.fluid.ports[].position",
        key!("part-field-fluid-ports-position"),
        G::Valve,
        Has::Fluid,
        Column("sections.fluid.ports"),
        FLUID,
    ),
    f(
        "sections.fluid.ports[].push_in_od",
        key!("part-field-fluid-ports-push-in"),
        G::Valve,
        Has::Fluid,
        Column("sections.fluid.ports"),
        FLUID,
    ),
    f(
        "sections.fluid.ports[].qn",
        key!("part-field-fluid-ports-qn"),
        G::Valve,
        Has::Fluid,
        Column("sections.fluid.ports"),
        FLUID,
    ),
    f(
        "sections.fluid.ports[].thread",
        key!("part-field-fluid-ports-thread"),
        G::Valve,
        Has::Fluid,
        Column("sections.fluid.ports"),
        FLUID,
    ),
    f(
        "sections.fluid.solenoids[]",
        key!("part-field-fluid-solenoids"),
        G::Valve,
        Has::Fluid,
        Shown,
        FLUID,
    ),
    f(
        "sections.fluid.positions",
        key!("part-field-fluid-positions"),
        G::Valve,
        Has::Fluid,
        Shown,
        FLUID,
    ),
    f(
        "sections.fluid.pins[].pin",
        key!("part-field-fluid-pins-pin"),
        G::Valve,
        Has::Fluid,
        Shown,
        FLUID,
    ),
    f(
        "sections.fluid.pins[].solenoid",
        key!("part-field-fluid-pins-solenoid"),
        G::Valve,
        Has::Fluid,
        Shown,
        FLUID,
    ),
    f(
        "sections.fluid.galleries[].gallery",
        key!("part-field-fluid-galleries-gallery"),
        G::Valve,
        Has::Fluid,
        Shown,
        FLUID,
    ),
    f(
        "sections.fluid.galleries[].separates",
        key!("part-field-fluid-galleries-separates"),
        G::Valve,
        Has::Fluid,
        Shown,
        FLUID,
    ),
    f(
        "sections.fluid.galleries[].tap",
        key!("part-field-fluid-galleries-tap"),
        G::Valve,
        Has::Fluid,
        Shown,
        FLUID,
    ),
    f(
        "sections.fluid.transform.regulator.outlet",
        key!("part-field-fluid-transform-outlet"),
        G::Valve,
        Has::Fluid,
        Shown,
        FLUID,
    ),
    f(
        "sections.fluid.transform.regulator.p_set",
        key!("part-field-fluid-transform-pressure"),
        G::Valve,
        Has::Fluid,
        Shown,
        FLUID,
    ),
    f(
        "sections.fluid.transform.ejector.vacuum",
        key!("part-field-fluid-transform-vacuum"),
        G::Valve,
        Has::Fluid,
        Shown,
        FLUID,
    ),
    stored(
        "sections.fluid.fitting.port_thread",
        key!("part-field-fluid-fitting-thread"),
        G::Valve,
        Has::Fluid,
        NOT_READ,
    ),
    stored(
        "sections.fluid.fitting.tube_od",
        key!("part-field-fluid-fitting-tube"),
        G::Valve,
        Has::Fluid,
        NOT_READ,
    ),
    // Tube
    size(
        "sections.fluid.tube.od",
        key!("part-field-tube-od"),
        G::Tube,
        TUBE_KIND,
        TUBE,
    ),
    f(
        "sections.fluid.tube.p_rating",
        key!("part-field-tube-pressure"),
        G::Tube,
        TUBE_KIND,
        Field,
        TUBE,
    ),
    size(
        "sections.fluid.tube.id",
        key!("part-field-tube-id"),
        G::Tube,
        TUBE_KIND,
        TUBE_DATA,
    ),
    f(
        "sections.fluid.tube.material",
        key!("part-field-tube-material"),
        G::Tube,
        TUBE_KIND,
        Field,
        TUBE_DATA,
    ),
    f(
        "sections.fluid.tube.colour",
        key!("part-field-tube-colour"),
        G::Tube,
        TUBE_KIND,
        Field,
        TUBE_DATA,
    ),
    size(
        "sections.fluid.tube.min_bend_radius",
        key!("part-field-tube-bend"),
        G::Tube,
        TUBE_KIND,
        TUBE_DATA,
    ),
    f(
        "sections.fluid.tube.t_range.min",
        key!("part-field-tube-t-min"),
        G::Tube,
        TUBE_KIND,
        Shown,
        TUBE_DATA,
    ),
    f(
        "sections.fluid.tube.t_range.max",
        key!("part-field-tube-t-max"),
        G::Tube,
        TUBE_KIND,
        Shown,
        TUBE_DATA,
    ),
    // PLC
    f(
        "sections.plc.role",
        key!("part-field-plc-role"),
        G::Plc,
        DEVICE,
        F(Form::Channels),
        PLC,
    ),
    f(
        "sections.plc.channels.kind",
        key!("part-field-plc-channels-kind"),
        G::Plc,
        DEVICE,
        F(Form::Channels),
        PLC,
    ),
    f(
        "sections.plc.channels.count",
        key!("part-field-plc-channels-count"),
        G::Plc,
        DEVICE,
        F(Form::Channels),
        PLC,
    ),
    f(
        "sections.plc.channels.data_type",
        key!("part-field-plc-channels-type"),
        G::Plc,
        DEVICE,
        F(Form::Channels),
        PLC,
    ),
    f(
        "sections.plc.channels.designation",
        key!("part-field-plc-channels-pins"),
        G::Plc,
        DEVICE,
        F(Form::Channels),
        PLC,
    ),
    f(
        "sections.plc.more_channels[].kind",
        key!("part-field-plc-more-kind"),
        G::Plc,
        DEVICE,
        F(Form::Channels),
        PLC,
    ),
    f(
        "sections.plc.more_channels[].count",
        key!("part-field-plc-more-count"),
        G::Plc,
        DEVICE,
        F(Form::Channels),
        PLC,
    ),
    f(
        "sections.plc.more_channels[].data_type",
        key!("part-field-plc-more-type"),
        G::Plc,
        DEVICE,
        F(Form::Channels),
        PLC,
    ),
    f(
        "sections.plc.more_channels[].designation",
        key!("part-field-plc-more-pins"),
        G::Plc,
        DEVICE,
        F(Form::Channels),
        PLC,
    ),
    f(
        "sections.plc.io[].dir",
        key!("part-field-plc-io-dir"),
        G::Plc,
        DEVICE,
        F(Form::Channels),
        PLC,
    ),
    f(
        "sections.plc.io[].class",
        key!("part-field-plc-io-class"),
        G::Plc,
        DEVICE,
        F(Form::Channels),
        PLC,
    ),
    f(
        "sections.plc.io[].bits",
        key!("part-field-plc-io-bits"),
        G::Plc,
        DEVICE,
        F(Form::Channels),
        PLC,
    ),
    f(
        "sections.plc.supply_groups[].kind",
        key!("part-field-plc-supply-kind"),
        G::Plc,
        DEVICE,
        F(Form::Channels),
        PLC,
    ),
    f(
        "sections.plc.supply_groups[].feeds",
        key!("part-field-plc-supply-feeds"),
        G::Plc,
        DEVICE,
        F(Form::Channels),
        PLC,
    ),
    f(
        "sections.plc.supply_groups[].cps.l_plus",
        key!("part-field-plc-supply-plus"),
        G::Plc,
        DEVICE,
        F(Form::Channels),
        PLC,
    ),
    f(
        "sections.plc.supply_groups[].cps.m",
        key!("part-field-plc-supply-m"),
        G::Plc,
        DEVICE,
        F(Form::Channels),
        PLC,
    ),
    f(
        "sections.plc.allowed_slots[]",
        key!("part-field-plc-allowed-slots"),
        G::Plc,
        DEVICE,
        F(Form::Channels),
        PLC,
    ),
    f(
        "sections.plc.limits.max_modules",
        key!("part-field-plc-max-modules"),
        G::Plc,
        DEVICE,
        F(Form::Channels),
        PLC,
    ),
    f(
        "sections.plc.limits.max_input_bytes",
        key!("part-field-plc-max-inputs"),
        G::Plc,
        DEVICE,
        Field,
        PLC,
    ),
    f(
        "sections.plc.limits.max_output_bytes",
        key!("part-field-plc-max-outputs"),
        G::Plc,
        DEVICE,
        Field,
        PLC,
    ),
    f(
        "sections.plc.ratings.channel_ma",
        key!("part-field-plc-channel-ma"),
        G::Plc,
        DEVICE,
        Field,
        RATINGS,
    ),
    f(
        "sections.plc.ratings.group_ma",
        key!("part-field-plc-group-ma"),
        G::Plc,
        DEVICE,
        Field,
        RATINGS,
    ),
    f(
        "sections.plc.ratings.module_ma",
        key!("part-field-plc-module-ma"),
        G::Plc,
        DEVICE,
        Field,
        RATINGS,
    ),
    f(
        "sections.plc.iolink.class",
        key!("part-field-plc-iolink-class"),
        G::Plc,
        DEVICE,
        Shown,
        PLC,
    ),
    f(
        "sections.plc.iolink.input_bytes",
        key!("part-field-plc-iolink-inputs"),
        G::Plc,
        DEVICE,
        Shown,
        PLC,
    ),
    f(
        "sections.plc.iolink.output_bytes",
        key!("part-field-plc-iolink-outputs"),
        G::Plc,
        DEVICE,
        Shown,
        PLC,
    ),
    f(
        "sections.plc.iolink.max_cable_m",
        key!("part-field-plc-iolink-cable"),
        G::Plc,
        DEVICE,
        Shown,
        PLC,
    ),
    f(
        "sections.plc.positions.count",
        key!("part-field-plc-positions"),
        G::Plc,
        DEVICE,
        Shown,
        PLC,
    ),
    f(
        "sections.plc.positions.map[].channel",
        key!("part-field-plc-positions-channel"),
        G::Plc,
        DEVICE,
        Shown,
        PLC,
    ),
    f(
        "sections.plc.positions.map[].solenoid",
        key!("part-field-plc-positions-solenoid"),
        G::Plc,
        DEVICE,
        Shown,
        PLC,
    ),
    f(
        "sections.plc.unbound",
        key!("part-field-plc-unbound"),
        G::Plc,
        DEVICE,
        Shown,
        PLC,
    ),
    f(
        "sections.plc.bit_of",
        key!("part-field-plc-bit-of"),
        G::Plc,
        DEVICE,
        Shown,
        PLC,
    ),
    f(
        "sections.plc.pin_of",
        key!("part-field-plc-pin-of"),
        G::Plc,
        DEVICE,
        Shown,
        PLC,
    ),
    // Network ports
    f(
        "sections.bus.ports[].interface",
        key!("part-field-bus-interface"),
        G::Ports,
        BUS_KINDS,
        F(Form::Ports),
        BUS,
    ),
    f(
        "sections.bus.ports[].port_label",
        key!("part-field-bus-port-label"),
        G::Ports,
        BUS_KINDS,
        F(Form::Ports),
        BUS,
    ),
    f(
        "sections.bus.ports[].bus",
        key!("part-field-bus-system"),
        G::Ports,
        BUS_KINDS,
        F(Form::Ports),
        BUS,
    ),
    f(
        "sections.bus.ports[].role",
        key!("part-field-bus-role"),
        G::Ports,
        BUS_KINDS,
        F(Form::Ports),
        BUS,
    ),
    f(
        "sections.bus.ports[].ports",
        key!("part-field-bus-ports"),
        G::Ports,
        BUS_KINDS,
        F(Form::Ports),
        BUS,
    ),
    f(
        "sections.bus.ports[].termination",
        key!("part-field-bus-termination"),
        G::Ports,
        BUS_KINDS,
        F(Form::Ports),
        BUS,
    ),
    f(
        "sections.bus.ports[].connector",
        key!("part-field-bus-connector"),
        G::Ports,
        BUS_KINDS,
        Shown,
        BUS,
    ),
    f(
        "sections.bus.ports[].rates[]",
        key!("part-field-bus-rates"),
        G::Ports,
        BUS_KINDS,
        Shown,
        BUS,
    ),
    f(
        "sections.bus.ports[].address_range.min",
        key!("part-field-bus-address-min"),
        G::Ports,
        BUS_KINDS,
        Shown,
        BUS,
    ),
    f(
        "sections.bus.ports[].address_range.max",
        key!("part-field-bus-address-max"),
        G::Ports,
        BUS_KINDS,
        Shown,
        BUS,
    ),
    f(
        "sections.bus.ports[].iolink.vendor_id",
        key!("part-field-bus-iolink-vendor"),
        G::Ports,
        BUS_KINDS,
        Shown,
        BUS,
    ),
    f(
        "sections.bus.ports[].iolink.device_id",
        key!("part-field-bus-iolink-device"),
        G::Ports,
        BUS_KINDS,
        Shown,
        BUS,
    ),
    f(
        "sections.bus.ports[].iolink.class",
        key!("part-field-bus-iolink-class"),
        G::Ports,
        BUS_KINDS,
        Shown,
        BUS,
    ),
    f(
        "sections.bus.ports[].iolink.data_type",
        key!("part-field-bus-iolink-type"),
        G::Ports,
        BUS_KINDS,
        Shown,
        BUS,
    ),
    f(
        "sections.bus.ports[].iolink.input_bits",
        key!("part-field-bus-iolink-inputs"),
        G::Ports,
        BUS_KINDS,
        Shown,
        BUS,
    ),
    f(
        "sections.bus.ports[].iolink.output_bits",
        key!("part-field-bus-iolink-outputs"),
        G::Ports,
        BUS_KINDS,
        Shown,
        BUS,
    ),
    f(
        "sections.bus.ports[].signals",
        key!("part-field-bus-signals"),
        G::Ports,
        BUS_KINDS,
        Shown,
        BUS,
    ),
    // Technical data: the free rows, and the values the program computes with by their keys
    f(
        "tech_data[].key",
        key!("part-field-tech-key"),
        G::Technical,
        Has::Every,
        F(Form::Technical),
        TECH,
    ),
    f(
        "tech_data[].value",
        key!("part-field-tech-value"),
        G::Technical,
        Has::Every,
        F(Form::Technical),
        TECH,
    ),
    f(
        "tech_data.rated_current",
        key!("part-field-tech-rated-current"),
        G::Technical,
        DEVICE,
        Field,
        &[r(Ft::WireSizing, Printed, key!("part-says-rated-current"))],
    ),
    f(
        "tech_data.i_load",
        key!("part-field-tech-load-current"),
        G::Technical,
        DEVICE,
        Field,
        LOAD,
    ),
    f(
        "tech_data.power",
        key!("part-field-tech-power"),
        G::Technical,
        DEVICE,
        Field,
        LOAD,
    ),
    f(
        "tech_data.voltage",
        key!("part-field-tech-voltage"),
        G::Technical,
        DEVICE,
        Field,
        LOAD,
    ),
    f(
        "tech_data.colour",
        key!("part-field-tech-colour"),
        G::Technical,
        TERMINAL,
        Field,
        &[r(Ft::AssemblyList, Printed, key!("part-says-terminal-colour"))],
    ),
    // Goes with it
    f(
        "accessories[].part",
        key!("part-field-accessories-part"),
        G::GoesWith,
        Has::Mounted,
        Column("accessories"),
        ACCESSORIES,
    ),
    f(
        "accessories[].kind",
        key!("part-field-accessories-kind"),
        G::GoesWith,
        Has::Mounted,
        Column("accessories"),
        ACCESSORIES,
    ),
    f(
        "accessories[].qty",
        key!("part-field-accessories-qty"),
        G::GoesWith,
        Has::Mounted,
        Column("accessories"),
        ACCESSORIES,
    ),
    f(
        "accessories[].per",
        key!("part-field-accessories-per"),
        G::GoesWith,
        Has::Mounted,
        Column("accessories"),
        ACCESSORIES,
    ),
    f(
        "accessories[].optional",
        key!("part-field-accessories-optional"),
        G::GoesWith,
        Has::Mounted,
        Column("accessories"),
        ACCESSORIES,
    ),
    f(
        "variants[].key",
        key!("part-field-variants-key"),
        G::GoesWith,
        Has::Mounted,
        Column("variants"),
        VARIANTS,
    ),
    f(
        "variants[].name",
        key!("part-field-variants-name"),
        G::GoesWith,
        Has::Mounted,
        Column("variants"),
        VARIANTS,
    ),
    f(
        "variants[].part_number",
        key!("part-field-variants-part-number"),
        G::GoesWith,
        Has::Mounted,
        Column("variants"),
        VARIANTS,
    ),
    f(
        "variants[].order_number",
        key!("part-field-variants-order-number"),
        G::GoesWith,
        Has::Mounted,
        Column("variants"),
        VARIANTS,
    ),
    f(
        "variants[].type_designation",
        key!("part-field-variants-type"),
        G::GoesWith,
        Has::Mounted,
        Column("variants"),
        VARIANTS,
    ),
    f(
        "variants[].description",
        key!("part-field-variants-description"),
        G::GoesWith,
        Has::Mounted,
        Shown,
        VARIANTS,
    ),
    stored(
        "variants[].gtin",
        key!("part-field-variants-gtin"),
        G::GoesWith,
        Has::Mounted,
        NOT_READ,
    ),
    f(
        "variants[].tech_data[].key",
        key!("part-field-variants-tech-key"),
        G::GoesWith,
        Has::Mounted,
        Shown,
        VARIANTS,
    ),
    f(
        "variants[].tech_data[].value",
        key!("part-field-variants-tech-value"),
        G::GoesWith,
        Has::Mounted,
        Shown,
        VARIANTS,
    ),
    f(
        "configured.order_code",
        key!("part-field-configured-order-code"),
        G::GoesWith,
        CONFIGURED_KIND,
        Field,
        CONFIGURED,
    ),
    f(
        "configured.components[].part",
        key!("part-field-configured-part"),
        G::GoesWith,
        CONFIGURED_KIND,
        Column("configured.components"),
        CONFIGURED,
    ),
    f(
        "configured.components[].qty",
        key!("part-field-configured-qty"),
        G::GoesWith,
        CONFIGURED_KIND,
        Column("configured.components"),
        CONFIGURED,
    ),
    // About
    f(
        "meta.key",
        key!("part-field-meta-key"),
        G::About,
        Has::Every,
        Shown,
        &[r(Ft::Search, Printed, key!("part-says-key"))],
    ),
    f(
        "meta.licence",
        key!("part-field-meta-licence"),
        G::About,
        Has::Every,
        Shown,
        ABOUT,
    ),
    f(
        "meta.sources[]",
        key!("part-field-meta-sources"),
        G::About,
        Has::Every,
        Shown,
        ABOUT,
    ),
    f(
        "meta.as_of",
        key!("part-field-meta-as-of"),
        G::About,
        Has::Every,
        Shown,
        ABOUT,
    ),
    f(
        "meta.confidence",
        key!("part-field-meta-confidence"),
        G::About,
        Has::Every,
        Shown,
        ABOUT,
    ),
    f(
        "meta.status",
        key!("part-field-meta-status"),
        G::About,
        Has::Every,
        Shown,
        &[r(Ft::PartChoice, Printed, key!("part-says-status"))],
    ),
    f(
        "meta.foreign_terms",
        key!("part-field-meta-foreign-terms"),
        G::About,
        Has::Every,
        Shown,
        ABOUT,
    ),
    stored(
        "free_props[].key",
        key!("part-field-free-props-key"),
        G::About,
        Has::Every,
        key!("part-stored-import"),
    ),
    stored(
        "free_props[].value",
        key!("part-field-free-props-value"),
        G::About,
        Has::Every,
        key!("part-stored-import"),
    ),
    stored(
        "alternatives[].kind",
        key!("part-field-alternatives-kind"),
        G::About,
        Has::Every,
        NOT_READ,
    ),
    stored(
        "alternatives[].part",
        key!("part-field-alternatives-part"),
        G::About,
        Has::Every,
        NOT_READ,
    ),
    stored(
        "macros[].rep_type",
        key!("part-field-macros-rep-type"),
        G::About,
        Has::Every,
        NOT_READ,
    ),
    stored(
        "macros[].macro",
        key!("part-field-macros-macro"),
        G::About,
        Has::Every,
        NOT_READ,
    ),
];

/// The field at `path` (a list's item member as `[]`: `cp_pattern[].csa_min`), if one is declared.
pub fn field(path: &str) -> Option<&'static PartField> {
    PART_FIELDS.iter().find(|f| f.path == path)
}

/// The fields part `p` has, in the sheet's order: group by group, each group's in the declaration's order.
pub fn fields_of(p: &Part) -> Vec<&'static PartField> {
    let mut out: Vec<&'static PartField> = PART_FIELDS.iter().filter(|f| f.has.holds(p)).collect();
    out.sort_by_key(|f| f.group);
    out
}

/// What part `p` lacks: every reader that is not only printed, of a field the part has and lacks, the fields merged per
/// reader's sentence, in the order of the sheet.
pub fn lacks(p: &Part) -> Vec<Lack> {
    let json = serde_json::to_value(p).unwrap_or(Value::Null);
    let mut out: Vec<Lack> = Vec::new();
    for field in fields_of(p) {
        if matches!(field.entry, Entry::Stored(_)) || present(&json, field.path) {
            continue;
        }
        for reader in field.readers {
            if reader.without == Without::Printed || !reader.on.holds(p) {
                continue;
            }
            match out
                .iter_mut()
                .find(|l| l.feature == reader.feature && l.says == reader.says)
            {
                Some(l) => l.fields.push(field),
                None => out.push(Lack {
                    feature: reader.feature,
                    without: reader.without,
                    says: reader.says,
                    fields: vec![field],
                }),
            }
        }
    }
    out
}

/// The fields part `p` lacks that `feature` reads: the gate a query, a rule or a command asks.
pub fn lacking(p: &Part, feature: Feature) -> Vec<&'static PartField> {
    lacks(p)
        .into_iter()
        .filter(|l| l.feature == feature)
        .flat_map(|l| l.fields)
        .collect()
}

/// A part's missing datum that an outcome comes from (doc 07 §5.7): a finding or a refusal carries it, and the window
/// offers Edit part… for it, which opens Parts on the part at the field.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PartLack {
    /// The part's copy in the project.
    pub copy: ew_ids::Id<Part>,
    /// The field it lacks first.
    pub field: &'static str,
}

impl PartLack {
    /// What part `p` (the project's copy `copy`) lacks first that `feature` reads; `None` where it lacks nothing.
    pub fn of(copy: ew_ids::Id<Part>, p: &Part, feature: Feature) -> Option<PartLack> {
        let field = lacking(p, feature).first()?.path;
        Some(PartLack { copy, field })
    }
}

/// How one feature stands for a part (Ready for, doc 07 §5.7).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Readiness {
    /// Nothing it reads is missing: the fields it reads that the part gives, in the sheet's order.
    Ready(Vec<&'static PartField>),
    /// What it lacks, one entry per reader's sentence.
    Lacks(Vec<Lack>),
    /// It reads fields of this kind of part, but none of this part: why, as the readers' [`Reader::on`].
    NotHere(Has),
}

/// A feature and how it stands for a part.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Ready {
    /// The feature.
    pub feature: Feature,
    /// How it stands.
    pub state: Readiness,
}

impl Ready {
    /// Its line under the feature: what it reads that is given, why it does not apply, or each lack's sentence.
    pub fn lines(&self) -> Vec<Text> {
        match &self.state {
            Readiness::Ready(given) if given.is_empty() => vec![text!("part-ready-nothing")],
            Readiness::Ready(given) => vec![Text::checked_by_macro(
                key!("part-ready-given"),
                crate::text_english("part-ready-given").unwrap_or("?"),
                vec![TextArg::List(ew_l10n::List::new(
                    given.iter().map(|f| TextArg::Text(word_in(f.word))).collect(),
                    text!("part-fields-between"),
                    text!("part-fields-and"),
                ))],
            )],
            Readiness::Lacks(lacks) => lacks.iter().map(Lack::text).collect(),
            Readiness::NotHere(has) => vec![word(has.not_here())],
        }
    }
}

impl Has {
    /// The sentence that says a reader does not read this part: it is not of the parts the predicate names.
    pub fn not_here(self) -> &'static str {
        match self {
            Has::Every | Has::Kinds(_) => key!("part-not-here-kind"),
            Has::Mounted => key!("part-not-here-mounted"),
            Has::Placed => key!("part-not-here-placed"),
            Has::Cutout => key!("part-not-here-cutout"),
            Has::Pinned | Has::HasPins => key!("part-not-here-pins"),
            Has::Drawn => key!("part-not-here-drawn"),
            Has::Fluid => key!("part-not-here-fluid"),
            Has::Solenoids => key!("part-not-here-solenoids"),
            Has::Plc => key!("part-not-here-plc"),
        }
    }
}

/// Every feature that reads a field part `p` has, in the order of the sheet, one entry each: what it lacks, else what
/// it reads that is given, else why it reads nothing of this part.
pub fn ready_for(p: &Part) -> Vec<Ready> {
    let json = serde_json::to_value(p).unwrap_or(Value::Null);
    let mut out: Vec<Ready> = Vec::new();
    for field in fields_of(p) {
        if matches!(field.entry, Entry::Stored(_)) {
            continue;
        }
        let given = present(&json, field.path);
        for reader in field.readers {
            let at = match out.iter().position(|r| r.feature == reader.feature) {
                Some(i) => i,
                None => {
                    out.push(Ready {
                        feature: reader.feature,
                        state: Readiness::NotHere(reader.on),
                    });
                    out.len() - 1
                }
            };
            if !reader.on.holds(p) {
                continue;
            }
            let state = &mut out[at].state;
            if matches!(state, Readiness::NotHere(_)) {
                *state = Readiness::Ready(Vec::new());
            }
            if let Readiness::Ready(fields) = state
                && given
                && !fields.contains(&field)
            {
                fields.push(field);
            }
        }
    }
    for lack in lacks(p) {
        if let Some(r) = out.iter_mut().find(|r| r.feature == lack.feature) {
            match &mut r.state {
                Readiness::Lacks(l) => l.push(lack),
                state => *state = Readiness::Lacks(vec![lack]),
            }
        }
    }
    out
}

/// Whether the member at `path` has a value in `json`: a list's item member in every item of a list that has items, a
/// list of values when it has one, a named technical value when the technical data has its key.
fn present(json: &Value, path: &str) -> bool {
    if let Some(key) = path.strip_prefix("tech_data.") {
        return json.get("tech_data").and_then(Value::as_array).is_some_and(|a| {
            a.iter()
                .any(|t| t.get("key").and_then(Value::as_str) == Some(key))
        });
    }
    fn at(v: &Value, members: &[&str]) -> bool {
        let Some((first, rest)) = members.split_first() else {
            return !v.is_null();
        };
        match first.strip_suffix("[]") {
            Some(list) => {
                let items = if list.is_empty() {
                    v.as_array()
                } else {
                    v.get(list).and_then(Value::as_array)
                };
                items.is_some_and(|a| !a.is_empty() && a.iter().all(|i| at(i, rest)))
            }
            None => v.get(*first).is_some_and(|v| at(v, rest)),
        }
    }
    // `a[][]` is the list `a`, each item a list: its segments `a[]` and `[]`.
    let mut segments: Vec<String> = Vec::new();
    for s in path.split('.') {
        let name = s.trim_end_matches("[]");
        match s.matches("[]").count() {
            0 => segments.push(name.to_owned()),
            n => {
                segments.push(format!("{name}[]"));
                segments.extend((1..n).map(|_| "[]".to_owned()));
            }
        }
    }
    let segments: Vec<&str> = segments.iter().map(String::as_str).collect();
    at(json, &segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A part of kind `kind` read from its JSON form with the members `more`.
    fn part(kind: &str, more: Value) -> Part {
        let mut v = serde_json::json!({
            "kind": kind,
            "manufacturer": {"library": "00000000-0000-0000-0000-000000000001", "item": "00000000-0000-0000-0000-000000000002", "version_req": "*"},
            "part_number": "P-1",
        });
        if let (Some(o), Value::Object(m)) = (v.as_object_mut(), more) {
            o.extend(m);
        }
        serde_json::from_value(v).expect("a part")
    }

    /// A field a person meets names what reads it, and a field nothing reads is stored and names no reader: a member
    /// added to the declaration says where it is used or why it is not asked for.
    #[test]
    fn every_field_a_person_enters_names_a_reader() {
        for f in PART_FIELDS {
            let stored = matches!(f.entry, Entry::Stored(_));
            assert_eq!(f.readers.is_empty(), stored, "{}", f.path);
        }
    }

    /// The reference page Parts ▸ Fields (NFR-DOC-06): the declaration in English, group by group, written where
    /// `EW_REFERENCE_DIR` asks for the reference, for `cargo xtask docs-gen`'s `reference/part-fields.md`.
    // The variable only chooses whether the generated file is also written out.
    #[allow(clippy::disallowed_methods)]
    #[test]
    fn the_part_fields_reference() {
        let mut groups: Vec<Group> = PART_FIELDS.iter().map(|f| f.group).collect();
        groups.sort();
        groups.dedup();
        let entry = |e: Entry| match e {
            Entry::Field => "typed".to_owned(),
            Entry::Column(list) => format!("a column of {list}"),
            Entry::Form(form) => format!("{form:?}").to_lowercase(),
            Entry::Shown => "shown".to_owned(),
            Entry::Stored(why) => format!("not asked: {}", word(why)),
        };
        let groups: Vec<Value> = groups
            .into_iter()
            .map(|g| {
                let fields: Vec<Value> = PART_FIELDS
                    .iter()
                    .filter(|f| f.group == g)
                    .map(|f| {
                        let readers: Vec<Value> = f
                            .readers
                            .iter()
                            .map(|r| {
                                serde_json::json!({
                                    "feature": word(r.feature.word()).to_string(),
                                    "without": format!("{:?}", r.without).to_lowercase(),
                                    "says": sentence(r.says, TextArg::Text(word(f.word))).to_string(),
                                })
                            })
                            .collect();
                        serde_json::json!({
                            "path": f.path,
                            "word": word(f.word).to_string(),
                            "entry": entry(f.entry),
                            "above_zero": f.limit == Limit::AboveZero,
                            "readers": readers,
                        })
                    })
                    .collect();
                serde_json::json!({"group": word(g.word()).to_string(), "fields": fields})
            })
            .collect();
        let text = serde_json::to_string_pretty(&serde_json::json!({ "groups": groups })).expect("JSON");
        if let Some(dir) = std::env::var_os("EW_REFERENCE_DIR").map(std::path::PathBuf::from) {
            std::fs::create_dir_all(&dir).expect("the reference folder");
            std::fs::write(dir.join("part-fields.json"), &text).expect("part-fields.json");
        }
        assert!(text.contains("\"Mounting plate\""), "{text}");
    }

    #[test]
    fn a_cabinet_without_its_size_says_the_sentence_once_with_both_fields() {
        let cabinet = part(
            "enclosure",
            serde_json::json!({"sections": {"enclosure": {"depth": 300_000_000}}}),
        );
        let lacks = lacks(&cabinet);
        let cabinet: Vec<&Lack> = lacks.iter().filter(|l| l.feature == Feature::Cabinet).collect();
        assert_eq!(cabinet.len(), 1, "{lacks:?}");
        let paths: Vec<&str> = cabinet[0].fields.iter().map(|f| f.path).collect();
        assert_eq!(paths, ["sections.enclosure.width", "sections.enclosure.height"]);
        assert_eq!(
            cabinet[0].text().to_string(),
            "Without width and height the cabinet's outline, its doors and its walls have no size on the layout."
        );
    }

    #[test]
    fn a_rail_without_a_profile_lacks_it_for_the_rail_fit() {
        let rail = part("rail", serde_json::json!({}));
        let lacking: Vec<&str> = lacking(&rail, Feature::RailFit).iter().map(|f| f.path).collect();
        assert_eq!(lacking, ["sections.rail.profile"]);
        let given = part(
            "rail",
            serde_json::json!({"sections": {"rail": {"profile": "ts35x7_5"}}}),
        );
        assert!(lacking_is_empty(&given, Feature::RailFit));
    }

    fn lacking_is_empty(p: &Part, feature: Feature) -> bool {
        lacking(p, feature).is_empty()
    }

    #[test]
    fn a_ferrule_has_no_field_in_the_cabinet() {
        let ferrule = part("ferrule", serde_json::json!({}));
        let fields = fields_of(&ferrule);
        assert!(fields.iter().all(|f| f.group != Group::Cabinet), "{fields:?}");
        assert!(fields.iter().any(|f| f.group == Group::Ferrule));
    }

    #[test]
    fn a_list_member_is_present_when_every_item_has_it() {
        let json = serde_json::json!({"cp_pattern": [{"csa_min": "1"}, {}], "meta": {"sources": ["x"]}});
        assert!(!present(&json, "cp_pattern[].csa_min"));
        assert!(present(&json, "meta.sources[]"));
        assert!(
            !present(&json, "mech.holes[].x"),
            "an empty list lacks its members"
        );
        let outline = serde_json::json!({"mech": {"front_2d": [[{"x": 0, "y": 0}]]}});
        assert!(present(&outline, "mech.front_2d[][]"));
    }
}
