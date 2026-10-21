// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The public contract: the `api_v1_*` views and their columns (ADR-0017 decision 2, doc 17 §6.1).
//!
//! Each view is a plain projection of one internal table `t_<name>` with the same columns; users, rule packs,
//! MCP and the acceptance SQL see only the views. `api_v1` is **Preview** until the 1.0 freeze (ADR-0017
//! amendment 2): within v1 columns are only ever added.
//!
//! **Column conventions** (doc 17 §6.1): `id` is the entity UUID as lowercase text and references are
//! `<entity>_id`; `tag` is the list form of a device tag (`-101F1`, `-X1031:1`) and `tag_full` the complete
//! designation; multilingual text is `<name>` (the neutral text, else the first language) plus `<name>_ml`
//! (the JSON of all languages, read with `ew_ml`); quantities are exact decimal text with a unit column
//! where needed, lengths integer `_nm`; booleans 0/1; enums lowercase text, electrical codes in their printed
//! form (`L+`, `PE`, `DBU`); lists are JSON arrays as text; a value that is not derived yet is `NULL`.

/// The SQLite type of a column.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColType {
    /// `TEXT`.
    Text,
    /// `INTEGER`.
    Integer,
}

impl ColType {
    /// The SQL type name.
    pub fn sql(self) -> &'static str {
        match self {
            ColType::Text => "TEXT",
            ColType::Integer => "INTEGER",
        }
    }
}

/// One column of a view.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Column {
    /// The name.
    pub name: &'static str,
    /// The type.
    pub ty: ColType,
    /// What it holds.
    pub doc: &'static str,
}

/// One public view.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct View {
    /// The view name (`api_v1_device`).
    pub name: &'static str,
    /// What one row is.
    pub row: &'static str,
    /// The columns, in order.
    pub columns: &'static [Column],
}

impl View {
    /// The internal table behind the view (`t_device`).
    pub fn table(&self) -> String {
        format!("t_{}", self.name.trim_start_matches("api_v1_"))
    }
}

const fn t(name: &'static str, doc: &'static str) -> Column {
    Column {
        name,
        ty: ColType::Text,
        doc,
    }
}

const fn i(name: &'static str, doc: &'static str) -> Column {
    Column {
        name,
        ty: ColType::Integer,
        doc,
    }
}

/// `api_v1_meta`.
pub const META: View = View {
    name: "api_v1_meta",
    row: "the projection (one row)",
    columns: &[
        i("rev", "the model revision the projection was built from"),
        i(
            "generation",
            "the store generation the project was opened at; NULL for a model without a store",
        ),
        t("format", "the project format version (\"1\")"),
        t(
            "derivation_profile",
            "the derivation profile version of the project",
        ),
        t(
            "ew_version",
            "the version of the program that built the projection",
        ),
        t("languages", "the project languages, JSON array of BCP 47 tags"),
        t("as_of", "the date of the release context; NULL without one"),
        t(
            "api_status",
            "\"preview\": api_v1 is Preview until the 1.0 freeze (ADR-0017 amendment 2); columns may still be added",
        ),
        t(
            "preview_additions",
            "the views and columns beyond doc 17 §6.1, JSON array of \"view\" or \"view.column\" (PREVIEW_ADDITIONS)",
        ),
    ],
};

/// `api_v1_page`.
pub const PAGE: View = View {
    name: "api_v1_page",
    row: "page in page order, dividers and generated report pages included",
    columns: &[
        t(
            "id",
            "the page UUID; a divider page has its section's UUID, a generated page its v5 page ID",
        ),
        i("physical_index", "the 0-based position in the page order"),
        t("folio", "the printed folio label (\"101\", \"901.1\")"),
        i("number", "the folio number"),
        t(
            "label",
            "the stored folio label of an authored page (\"101a\"); NULL otherwise",
        ),
        t("section_id", "the section UUID; NULL for front matter"),
        t("kind", "the page type (schematic, section_divider, report, …)"),
        t("title", "the page title (TOC title), neutral text"),
        t("title_ml", "the page title, JSON of all languages"),
        t(
            "description",
            "the title-block description override of the page; NULL without one",
        ),
        i(
            "description_overridden",
            "1 when the page overrides the project description",
        ),
        t(
            "structure_function",
            "the function aspect of the page (\"=P01\"); NULL without one",
        ),
        t(
            "structure_location",
            "the location aspect of the page (\"+J-E-2416\"); NULL without one",
        ),
        t("prev_folio", "the folio of the previous page; NULL on the first"),
        t("next_folio", "the folio of the next page; NULL on the last"),
        i("generated", "1 for a generated report page"),
        t(
            "report_block_id",
            "the report block of a generated page; NULL otherwise",
        ),
    ],
};

/// `api_v1_device`.
pub const DEVICE: View = View {
    name: "api_v1_device",
    row: "device (strips, terminals and cables included)",
    columns: &[
        t("id", "the device UUID"),
        t(
            "tag",
            "the list form of the tag (\"-101F1\", \"-X1031:1\"); NULL for a device without one",
        ),
        t("tag_full", "the full designation (\"=P01+J-E-2416-101F1\")"),
        t("letter", "the letter code of the tag spec"),
        i("counter", "the stored counter; NULL when unnumbered"),
        i("frozen", "1 for a frozen tag"),
        t("kind", "the device kind (psu, terminal, …); NULL without one"),
        t("parent_id", "the parent device; NULL without one"),
        t(
            "structure_function",
            "the function aspect of the device (\"=P01\"); NULL without one",
        ),
        t(
            "structure_location",
            "the location aspect of the device (\"+J-E-2416\"); NULL without one",
        ),
        t(
            "function_text",
            "the function text, neutral text; NULL when empty",
        ),
        t(
            "function_text_ml",
            "the function text, JSON of all languages; NULL when empty",
        ),
        t("process_tag", "the process tag; NULL without one"),
        t(
            "state",
            "the lifecycle state as authored (used, spare, reserved, not_fitted)",
        ),
        t(
            "main_page_id",
            "the page of the main representation; NULL when not drawn",
        ),
        t(
            "main_folio",
            "the folio of the main representation; NULL when not drawn",
        ),
        t(
            "main_cell",
            "the frame cell of the main representation (\"B8\"); NULL without one",
        ),
        i("has_part", "1 when the device has a part"),
        t(
            "manufacturer",
            "the manufacturer of the main part; NULL without a part",
        ),
        t(
            "part_number",
            "the part number of the main part; NULL without a part",
        ),
        t(
            "tech_data",
            "the technical data as printed (\"6 A, C\"); NULL without any",
        ),
        t(
            "aspects",
            "the typed aspects, JSON array (\"strip\", \"terminal\", \"cable\")",
        ),
        i(
            "placed",
            "1 when placed in a panel layout; NULL until panel layout is derived (1.0)",
        ),
    ],
};

/// `api_v1_function`.
pub const FUNCTION: View = View {
    name: "api_v1_function",
    row: "function (interruption points included)",
    columns: &[
        t("id", "the function UUID"),
        t("device_id", "the device; NULL for an interruption point"),
        t(
            "device_tag",
            "the list form of the device's tag; NULL without a device",
        ),
        t("role", "main or aux"),
        t("kind", "normal, ip or black_box"),
        t("ip_name", "the name of an interruption point; NULL otherwise"),
        t("fndef", "the name of the function definition"),
        t(
            "category",
            "the category of the function definition; NULL without one",
        ),
        t(
            "page_id",
            "the page of its first representation in page order; NULL when not drawn",
        ),
        t("folio", "the folio of that representation; NULL when not drawn"),
        t("cell", "the frame cell of that representation; NULL without one"),
        t(
            "xref_text",
            "the printed cross-reference of that representation; NULL without one",
        ),
    ],
};

/// `api_v1_wire`.
pub const WIRE: View = View {
    name: "api_v1_wire",
    row: "physical wire or terminal bridge (doc 08 §3.8, §6.5)",
    columns: &[
        t(
            "id",
            "the wire UUID (its carrier connection); for a bridge the strip UUID and group (\"<uuid>/bridge/<n>\")",
        ),
        i(
            "seq",
            "the order of the wire list: wires in the numbering order of FR-NUM-028 (doc 08 §5.2), then bridges by label",
        ),
        t(
            "number",
            "the printed wire number (\"101.1\", `wire_numbers`; NULL for a cable core); a bridge's label \"BR1\"",
        ),
        t("kind", "wire or bridge"),
        t("from_end", "device, external or open"),
        t(
            "from_device_id",
            "the device at the From end; NULL for an external or open end",
        ),
        t("from_tag", "the list form of that device's tag"),
        t("from_cp", "the CP designation at the From end"),
        i(
            "from_level",
            "the terminal level at the From end; NULL without one",
        ),
        t(
            "from_clamp",
            "the terminal clamp at the From end (\"a\"); NULL without one",
        ),
        i(
            "from_side",
            "the terminal side at the From end (1 or 2); NULL without one",
        ),
        t(
            "from_ip",
            "the interruption point an external or open end ends at; NULL for a device end",
        ),
        t("to_end", "device, external or open"),
        t(
            "to_device_id",
            "the device at the To end; NULL for an external or open end",
        ),
        t("to_tag", "the list form of that device's tag"),
        t("to_cp", "the CP designation at the To end"),
        i("to_level", "the terminal level at the To end; NULL without one"),
        t("to_clamp", "the terminal clamp at the To end; NULL without one"),
        i(
            "to_side",
            "the terminal side at the To end (1 or 2); NULL without one",
        ),
        t(
            "to_ip",
            "the interruption point an external or open end ends at; NULL for a device end",
        ),
        t("potential_id", "the potential; NULL for a bond wire"),
        t(
            "potential_name",
            "the potential's name; \"A ⇄ B\" for a bond wire",
        ),
        t("csa", "the effective cross-section, exact decimal text"),
        t("csa_unit", "mm², AWG or kcmil"),
        t("colour", "the effective colour as printed (\"DBU\", \"GNYE\")"),
        t(
            "colour_iec",
            "the colour when every code is an IEC 60757 code; NULL otherwise",
        ),
        t("wire_part", "the part number of the wire part; NULL without one"),
        i(
            "length_nm",
            "the wire length; NULL until lengths are derived (1.0)",
        ),
        t(
            "length_source",
            "where the length comes from; NULL until lengths are derived (1.0)",
        ),
        t("cable_id", "the cable of a core wire; NULL otherwise"),
        t("cable_tag", "the list form of that cable's tag"),
        t("core", "the core name of a core wire (\"BN\")"),
        t("conductor_id", "the conductor of a core wire"),
        t("bond_id", "the bond a bond wire realises; NULL otherwise"),
        t(
            "ips",
            "the interruption points the wire passes, JSON array of names in path order",
        ),
        t(
            "junction_id",
            "the first junction the wire passes (doc 08 §3.2); NULL otherwise",
        ),
        t(
            "junction_style",
            "that junction's style: dot or bevel (\"junction\" for a newer style); NULL without a junction",
        ),
        t(
            "junction_wiring",
            "its effective wiring: star or chain (the style's default when unset); NULL without a junction",
        ),
        t(
            "junction_cell",
            "its folio and frame cell (\"101-C11\"; the folio alone outside the drawing area); NULL without a junction",
        ),
        t(
            "junction_cps",
            "star: the entering CP; chain: the wire's ends shared with another wire of the junction; JSON array of CP texts (\"F55:2\", \"-102G1:−#1\": `#n` where printed designations repeat)",
        ),
        t(
            "chain_cps",
            "the wire's ends on a CP that two or more drawn connections touch (a daisy chain, FR-CON-006), JSON array of CP texts",
        ),
        t(
            "bridge_kind",
            "plug-in, saddle, wire or inter-level for a bridge; NULL for a wire",
        ),
        t(
            "pages",
            "the folios of the wire's pages in page order, JSON array",
        ),
        t(
            "csa_source",
            "where the cross-section comes from: cdp, core, rule, bond, potential, project, built-in",
        ),
        t(
            "colour_source",
            "where the colour comes from: cdp, core, rule, potential, project, built-in",
        ),
        t("prov_csa", "the provenance of the cross-section in words"),
        t("prov_colour", "the provenance of the colour in words"),
    ],
};

/// `api_v1_terminal`.
pub const TERMINAL: View = View {
    name: "api_v1_terminal",
    row: "terminal",
    columns: &[
        t("id", "the terminal device UUID"),
        t("tag", "the list form of the terminal's tag (\"-X1031:1\")"),
        t("strip_id", "the strip"),
        t("strip_tag", "the list form of the strip's tag (\"-X1031\")"),
        t("designation", "the terminal designation (\"1\", \"PE\")"),
        t("printed", "the designation as printed on the terminal marker"),
        i("seq", "the position on the strip"),
        t(
            "function_text",
            "the terminal's function text (\"1+\"); NULL when empty",
        ),
        t(
            "strip_group",
            "the caption of the strip group holding the terminal (\"Pneumatics\"); NULL outside a group",
        ),
        t(
            "part_number",
            "the part number of the terminal's part; NULL without one",
        ),
        i("levels", "the number of levels"),
        t(
            "potential_id",
            "the potential of its first connected CP; NULL without one",
        ),
        t("potential_name", "that potential's name"),
        t(
            "potential_type",
            "that potential's type (\"L+\", \"PE\"); NULL without one",
        ),
        t(
            "state",
            "spare, reserved or not fitted as authored, else used (wired or bridged) or unused",
        ),
        t(
            "bridge",
            "the label of the bridge the terminal belongs to (\"BR1\"); NULL without one",
        ),
        i("rail_bonded", "1 when the terminal is bonded to the rail"),
        t(
            "page_id",
            "the page of its main representation; NULL when not drawn",
        ),
        t(
            "cell",
            "the frame cell of its main representation; NULL without one",
        ),
    ],
};

/// `api_v1_cable`.
pub const CABLE: View = View {
    name: "api_v1_cable",
    row: "cable",
    columns: &[
        t("id", "the cable device UUID"),
        t("tag", "the list form of the cable's tag (\"-106W1\")"),
        t("tag_full", "the full designation"),
        t(
            "part_number",
            "the part number of the cable's part; NULL without one",
        ),
        i("cores", "the number of conductors"),
        i("length_nm", "the manual length; NULL otherwise"),
        t("length_source", "manual, part or missing"),
        t(
            "from_location",
            "the location aspect of end A; NULL until derived",
        ),
        t("to_location", "the location aspect of end B; NULL until derived"),
        t(
            "end_a",
            "the devices at the From ends of its cores, a terminal standing for its strip, JSON array in natural order",
        ),
        t(
            "end_b",
            "the devices at the To ends of its cores, JSON array in natural order",
        ),
        i("external_end", "1 when a core wire ends at an external point"),
        i("shield", "1 when the cable has a shield conductor"),
        t(
            "pages",
            "the folios of the cable's representations in page order, JSON array",
        ),
    ],
};

/// `api_v1_conductor`.
pub const CONDUCTOR: View = View {
    name: "api_v1_conductor",
    row: "conductor of a cable",
    columns: &[
        t("id", "the conductor UUID"),
        t("cable_id", "the cable"),
        t("cable_tag", "the list form of the cable's tag"),
        t("core", "the core name (\"BN\", \"1\", \"SH\")"),
        i("pair", "the pair number; NULL without one"),
        t("colour", "the conductor's colour as printed; NULL without one"),
        t(
            "colour_iec",
            "the colour when every code is an IEC 60757 code; NULL otherwise",
        ),
        t(
            "csa",
            "the conductor's cross-section, exact decimal text; NULL without one",
        ),
        t("csa_unit", "mm², AWG or kcmil; NULL without a cross-section"),
        i("shield", "1 for a shield conductor"),
        t(
            "wire_id",
            "the physical wire running over the core; NULL without one",
        ),
    ],
};

/// `api_v1_potential`.
pub const POTENTIAL: View = View {
    name: "api_v1_potential",
    row: "potential (doc 08 §3)",
    columns: &[
        t(
            "id",
            "the potential ID: its minimum definition UUID, else its minimum member CP ID",
        ),
        t("name", "the canonical name"),
        t(
            "aliases",
            "the other interruption-point names on the potential, JSON array in natural order",
        ),
        t(
            "ptype",
            "the potential's own type (\"L+\", \"M\", \"PE\"); NULL without one",
        ),
        t(
            "voltage",
            "the potential's own voltage in V, exact decimal text; NULL without one",
        ),
        t("ac_dc", "ac or dc; NULL without one"),
        t(
            "circuit_class",
            "mains, control, analogue, bus or PE; NULL without one",
        ),
        t(
            "colour",
            "the profile's default colour for the potential: the effective colour of its first wire that takes it from the profile or a definition",
        ),
        t(
            "colour_iec",
            "that colour when every code is an IEC 60757 code; NULL otherwise",
        ),
        t(
            "default_csa",
            "the profile's default cross-section, exact decimal text",
        ),
        t("default_csa_unit", "mm², AWG or kcmil"),
        i("switched", "1 when functionally switched (doc 08 §3.6)"),
        t(
            "protected_by_device_id",
            "the device of the nearest upstream protective function; NULL without one",
        ),
        t("protected_by_tag", "that device's tag"),
        t(
            "protected_by_cp",
            "the CP of the protective function on this potential's side (\"Out1\"), when it is the direct upstream pair; NULL otherwise",
        ),
        t(
            "bond_ids",
            "the bonds joining this potential to another, JSON array of UUIDs",
        ),
        t(
            "bus_cable_ids",
            "the cables of the potential's bus lines (`Bus` connections with a cable, FR-CBL-008), JSON array of UUIDs; a bus line takes its colour and size from them (doc 20 §7.3 \"(cable)\")",
        ),
        i(
            "member_count",
            "device CPs (a terminal once) and external points on the potential (doc 20 §7.3)",
        ),
        t(
            "pages",
            "the folios of its wires' pages in page order, JSON array",
        ),
    ],
};

/// `api_v1_bond`.
pub const BOND: View = View {
    name: "api_v1_bond",
    row: "bond between two potentials (doc 08 §3.7)",
    columns: &[
        t("id", "the bond UUID"),
        t("label", "B1, B2, … in the partition's bond order"),
        t("kind", "the bond kind (pelv_0v_pe, …)"),
        t("a_potential_id", "the potential at end a"),
        t("a_potential_name", "its name"),
        t("b_potential_id", "the potential at end b"),
        t("b_potential_name", "its name"),
        t(
            "wire_id",
            "the physical wire realising the bond; NULL without one",
        ),
        t(
            "names",
            "the interruption-point names on the bond path, JSON array in natural order",
        ),
        t(
            "pages",
            "the folios of the bond wire's pages in page order, JSON array",
        ),
    ],
};

/// The columns of both BOM views, then the extra columns of one.
macro_rules! bom_columns {
    ($($extra:expr),* $(,)?) => {
        &[
    i("position", "the 1-based position within its group"),
    i("configured", "1 for a configured product (printed A1, A2, …)"),
    t("part_id", "the part copy"),
    t("variant", "the variant key; NULL without one"),
    t("manufacturer", "the manufacturer as printed"),
    t(
        "part_number",
        "the part number (the order code for a configured product)",
    ),
    t("type", "the type designation; NULL without one"),
    t("description", "the part description, neutral text"),
    t("description_ml", "the part description, JSON of all languages"),
    t("quantity", "the summed quantity, exact decimal text"),
    t("unit", "the order unit"),
    t(
        "tags",
        "the devices with this part, JSON array of list-form tags in natural order",
    ),
    t(
        "folios",
        "the folios of those devices' main representations, JSON array in page order",
    ),
            $($extra,)*
        ]
    };
}

/// `api_v1_bom_component`.
pub const BOM_COMPONENT: View = View {
    name: "api_v1_bom_component",
    row: "bill-of-materials row in component mode (doc 20 §14.1), in list order",
    columns: bom_columns!(),
};

/// `api_v1_bom_assembly`.
pub const BOM_ASSEMBLY: View = View {
    name: "api_v1_bom_assembly",
    row: "bill-of-materials row in assembly mode (doc 20 §14.2), in list order",
    columns: bom_columns!(
        t(
            "assembly_tag",
            "the list-form tag of the configured product's head device when the row has exactly one; NULL otherwise (not derived for several heads yet)",
        ),
        i(
            "level",
            "the level in the assembly tree; NULL until assembly levels are derived"
        ),
    ),
};

/// `api_v1_message`.
pub const MESSAGE: View = View {
    name: "api_v1_message",
    row: "finding of the projection's check run (open and waived)",
    columns: &[
        t(
            "fingerprint",
            "the 128-bit fingerprint, 32 lowercase hex digits (FR-CHK-044)",
        ),
        t("rule", "the rule ID (\"EW-CORE-033\")"),
        t("pack", "the rule pack"),
        t("pack_version", "the pack version; NULL until packs are versioned"),
        t(
            "severity",
            "info, warning or error (the scheme's effective severity)",
        ),
        t("mode", "the check mode of the run (release, online)"),
        t("state", "open or waived"),
        t(
            "target_ids",
            "the target records, JSON array of UUIDs, the primary target first",
        ),
        t("target_tags", "the subject as printed (a derived tag or a name)"),
        t("page_id", "the page the finding is shown on; NULL without one"),
        t("cell", "the frame cell; NULL until findings carry a position"),
        t("message", "the message in English"),
        t(
            "args",
            "the discriminator that tells findings over the same targets apart",
        ),
        t("waiver_id", "the waiver of a waived finding; NULL otherwise"),
    ],
};

/// Every public view, in name order.
pub const VIEWS: [View; 13] = [
    BOM_ASSEMBLY,
    BOM_COMPONENT,
    BOND,
    CABLE,
    CONDUCTOR,
    DEVICE,
    FUNCTION,
    MESSAGE,
    META,
    PAGE,
    POTENTIAL,
    TERMINAL,
    WIRE,
];

/// The DDL of the projection: one `t_*` table and one `api_v1_*` view per [`View`].
pub(crate) fn ddl() -> String {
    let mut sql = String::new();
    for v in VIEWS {
        let cols: Vec<String> = v
            .columns
            .iter()
            .map(|c| format!("{} {}", c.name, c.ty.sql()))
            .collect();
        sql.push_str(&format!("CREATE TABLE {} ({});\n", v.table(), cols.join(", ")));
        let names: Vec<&str> = v.columns.iter().map(|c| c.name).collect();
        sql.push_str(&format!(
            "CREATE VIEW {} AS SELECT {} FROM {};\n",
            v.name,
            names.join(", "),
            v.table()
        ));
    }
    sql
}

/// The status of the contract: `api_v1` is Preview until the 1.0 freeze (ADR-0017 amendment 2); the golden
/// acceptance SQL is its compatibility corpus.
pub const API_STATUS: &str = "preview";

/// The views and columns beyond the key columns of doc 17 §6.1 (owner ruling, cycle 13): a whole view as
/// `(view, &[])`, columns as `(view, &[columns])`.
pub const PREVIEW_ADDITIONS: &[(&str, &[&str])] = &[
    (
        "api_v1_bom_assembly",
        &["configured", "variant", "description_ml"],
    ),
    (
        "api_v1_bom_component",
        &["configured", "variant", "description_ml"],
    ),
    ("api_v1_bond", &[]),
    ("api_v1_cable", &["tag_full", "end_a", "end_b"]),
    ("api_v1_conductor", &["pair", "csa_unit"]),
    ("api_v1_device", &["function_text_ml"]),
    ("api_v1_function", &["ip_name"]),
    ("api_v1_meta", &["api_status", "preview_additions"]),
    ("api_v1_page", &["number", "section_id", "title_ml"]),
    (
        "api_v1_potential",
        &["colour_iec", "default_csa_unit", "bus_cable_ids"],
    ),
    (
        "api_v1_terminal",
        &["tag", "function_text", "strip_group", "bridge"],
    ),
    (
        "api_v1_wire",
        &[
            "seq",
            "from_end",
            "from_side",
            "from_ip",
            "to_end",
            "to_side",
            "to_ip",
            "conductor_id",
            "bond_id",
            "ips",
            "junction_id",
            "junction_style",
            "junction_wiring",
            "junction_cell",
            "junction_cps",
            "chain_cps",
            "bridge_kind",
            "csa_source",
            "colour_source",
        ],
    ),
];

/// [`PREVIEW_ADDITIONS`] as `view` and `view.column` names, in order.
pub fn preview_additions() -> Vec<String> {
    PREVIEW_ADDITIONS
        .iter()
        .flat_map(|(view, cols)| {
            if cols.is_empty() {
                vec![(*view).to_owned()]
            } else {
                cols.iter().map(|c| format!("{view}.{c}")).collect()
            }
        })
        .collect()
}

fn is_addition(view: &str, column: Option<&str>) -> bool {
    PREVIEW_ADDITIONS.iter().any(|(v, cols)| {
        *v == view
            && match column {
                None => cols.is_empty(),
                Some(c) => cols.is_empty() || cols.contains(&c),
            }
    })
}

/// The schema as text, one block per view (`ew sql schema`): the Preview status first, then every view with
/// its columns; the additions beyond doc 17 §6.1 are marked `[preview addition]`.
pub fn schema_text() -> String {
    let mut out = String::from(
        "api_v1 is Preview until the 1.0 freeze (ADR-0017 amendment 2): views and columns may still be added; \
         within v1 none is removed, renamed or retyped. Entries marked [preview addition] go beyond the \
         documented key columns (doc 17 §6.1).\n\n",
    );
    for v in VIEWS {
        let mark = if is_addition(v.name, None) {
            " [preview addition]"
        } else {
            ""
        };
        out.push_str(&format!("{} — one row per {}{mark}\n", v.name, v.row));
        for c in v.columns {
            let mark = if is_addition(v.name, Some(c.name)) && !is_addition(v.name, None) {
                " [preview addition]"
            } else {
                ""
            };
            out.push_str(&format!("  {:<24} {:<8} {}{mark}\n", c.name, c.ty.sql(), c.doc));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_are_sorted_unique_and_columns_unique() {
        let names: Vec<&str> = VIEWS.iter().map(|v| v.name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(names, sorted);
        for v in VIEWS {
            assert!(v.name.starts_with("api_v1_"));
            let mut cols: Vec<&str> = v.columns.iter().map(|c| c.name).collect();
            cols.sort_unstable();
            let n = cols.len();
            cols.dedup();
            assert_eq!(cols.len(), n, "duplicate column in {}", v.name);
        }
    }

    #[test]
    fn every_preview_addition_exists_and_the_schema_text_marks_it() {
        for (view, cols) in PREVIEW_ADDITIONS {
            let v = VIEWS
                .iter()
                .find(|v| v.name == *view)
                .unwrap_or_else(|| panic!("{view}"));
            for c in *cols {
                assert!(v.columns.iter().any(|x| x.name == *c), "{view}.{c}");
            }
        }
        let text = schema_text();
        assert!(text.starts_with("api_v1 is Preview until the 1.0 freeze"));
        assert!(text.contains(
            "api_v1_bond — one row per bond between two potentials (doc 08 §3.7) [preview addition]"
        ));
        assert!(
            text.lines()
                .any(|l| l.trim_start().starts_with("seq ") && l.ends_with("[preview addition]"))
        );
        assert!(preview_additions().contains(&"api_v1_wire.csa_source".to_owned()));
        assert!(preview_additions().contains(&"api_v1_bond".to_owned()));
    }

    #[test]
    fn the_mvp_views_of_fr_ext_007_exist() {
        for name in [
            "api_v1_device",
            "api_v1_function",
            "api_v1_page",
            "api_v1_wire",
            "api_v1_terminal",
            "api_v1_cable",
            "api_v1_conductor",
            "api_v1_potential",
            "api_v1_bom_component",
            "api_v1_bom_assembly",
            "api_v1_message",
        ] {
            assert!(VIEWS.iter().any(|v| v.name == name), "{name}");
        }
    }
}
