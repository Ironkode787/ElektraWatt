// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The table schemas of `golden/J-E-2416/README.md`: file stem, required and optional columns.

use crate::table::TableSpec;

const fn spec(
    name: &'static str,
    required: &'static [&'static str],
    optional: &'static [&'static str],
) -> TableSpec {
    TableSpec {
        name,
        required,
        optional,
    }
}

/// `project.csv`.
pub(crate) const PROJECT: TableSpec = spec(
    "project",
    &["key", "derivation_profile"],
    &[
        "document_title",
        "description",
        "long_description",
        "project_ref",
        "drawing_id",
        "manufacturing_year",
        "created",
        "edited",
        "supply_phases",
        "supply_neutral",
        "supply_pe",
        "supply_earthing",
        "supply_u_nom",
        "supply_frequency",
        "supply_origin",
        "control_voltage",
        "rated_consumption",
        "identifier_set",
        "date_format",
        "decimal_separator",
        "numbering_scheme",
        "numbering_format",
        "numbering_strip_format",
        "numbering_child_separator",
    ],
);

/// `parties.csv`.
pub(crate) const PARTIES: TableSpec = spec(
    "parties",
    &["key", "role"],
    &["name", "street", "postcode", "city", "country", "personal_data"],
);

/// `structure.csv`.
pub(crate) const STRUCTURE: TableSpec = spec(
    "structure",
    &["key", "aspect", "code"],
    &["aliases", "parent", "description", "order"],
);

/// `external_refs.csv`.
pub(crate) const EXTERNAL_REFS: TableSpec = spec(
    "external_refs",
    &["key", "location", "device"],
    &["connection", "project"],
);

/// `sections.csv`.
pub(crate) const SECTIONS: TableSpec = spec(
    "sections",
    &["key", "number", "folio_from", "folio_to"],
    &["title", "divider"],
);

/// `pages.csv`.
pub(crate) const PAGES: TableSpec = spec(
    "pages",
    &["key", "number", "kind", "form", "grid"],
    &["label", "section", "title", "function", "location", "scale"],
);

/// `report_blocks.csv`.
pub(crate) const REPORT_BLOCKS: TableSpec = spec(
    "report_blocks",
    &["key", "kind", "form"],
    &[
        "section",
        "first_folio",
        "title",
        "continuation",
        "include",
        "embedded_page",
        "embedded_rect",
    ],
);

/// `parts.csv`.
pub(crate) const PARTS: TableSpec = spec(
    "parts",
    &["key", "manufacturer", "part_number"],
    &[
        "kind",
        "type_designation",
        "description",
        "unit",
        "test_data",
        "tech_data",
    ],
);

/// `part_cps.csv`.
pub(crate) const PART_CPS: TableSpec = spec(
    "part_cps",
    &["key", "part", "idx", "designation"],
    &["potential_role", "level", "side", "clamp"],
);

/// `part_templates.csv`.
pub(crate) const PART_TEMPLATES: TableSpec =
    spec("part_templates", &["key", "part", "n", "fndef", "cps"], &["main"]);

/// `cable_cores.csv`.
pub(crate) const CABLE_CORES: TableSpec = spec(
    "cable_cores",
    &["key", "part", "n", "core"],
    &["colour", "csa", "shield", "pair"],
);

/// `devices.csv`.
pub(crate) const DEVICES: TableSpec = spec(
    "devices",
    &["key", "letter"],
    &[
        "counter",
        "scheme",
        "frozen",
        "frozen_sign",
        "parent",
        "position",
        "function",
        "location",
        "kind",
        "parts",
        "function_text",
        "process_tag",
        "state",
        "strip_label_mode",
        "strip_groups",
        "strip_field_side",
        "cable",
        "cable_length",
        "tech_data",
        "plc_station",
        "plc_role",
        "plc_order",
        "plc_module",
    ],
);

/// `terminals.csv`.
pub(crate) const TERMINALS: TableSpec = spec(
    "terminals",
    &["key", "strip", "seq", "designation"],
    &[
        "function_text",
        "parts",
        "state",
        "rail_bonded",
        "function",
        "location",
    ],
);

/// `functions.csv`.
pub(crate) const FUNCTIONS: TableSpec = spec(
    "functions",
    &["key", "fndef"],
    &[
        "device",
        "role",
        "kind",
        "part",
        "template",
        "cp_aliases",
        "ip_name",
        "ip_direction",
        "ip_external",
    ],
);

/// `representations.csv`.
pub(crate) const REPRESENTATIONS: TableSpec = spec(
    "representations",
    &["key", "functions", "page", "symbol", "cell"],
    &["dx", "dy", "arrangement"],
);

/// `connections.csv`.
pub(crate) const CONNECTIONS: TableSpec = spec(
    "connections",
    &["key", "a", "b"],
    &[
        "wire",
        "page",
        "kind",
        "core",
        "bridge_group",
        "bridge_kind",
        "bridge_shaft",
        "csa",
        "colour",
        "show_marker",
        "cable",
    ],
);

/// `junctions.csv`.
pub(crate) const JUNCTIONS: TableSpec = spec(
    "junctions",
    &["key", "page", "cell", "style", "sequence"],
    &["dx", "dy", "wiring"],
);

/// `conductors.csv`.
pub(crate) const CONDUCTORS: TableSpec = spec(
    "conductors",
    &["key", "cable", "core"],
    &["colour", "csa", "shield", "pair"],
);

/// `potentials.csv`.
pub(crate) const POTENTIALS: TableSpec = spec(
    "potentials",
    &["key", "at"],
    &[
        "ptype",
        "voltage",
        "ac_dc",
        "frequency",
        "earthing",
        "circuit_class",
    ],
);

/// `bonds.csv`.
pub(crate) const BONDS: TableSpec = spec("bonds", &["key", "a", "b", "kind"], &["via"]);

/// `colour_profile.csv` (the colour profile of the project settings).
pub(crate) const COLOUR_PROFILE: TableSpec = spec(
    "colour_profile",
    &["key"],
    &[
        "voltage_class",
        "function",
        "potential_type",
        "colour",
        "code",
        "ptype",
        "ac_dc",
        "u_min",
        "u_max",
        "analogue",
    ],
);

/// `size_profile.csv` (the size profile of the project settings).
pub(crate) const SIZE_PROFILE: TableSpec = spec("size_profile", &["key"], &["control", "mains", "order"]);

/// `profiles.csv`: the header of the colour and the size profile.
pub(crate) const PROFILES: TableSpec = spec(
    "profiles",
    &["key", "profile", "name"],
    &[
        "basis",
        "basis_note",
        "standards",
        "dbu_code",
        "verified",
        "pe_class",
        "class_rules",
        "bond_rule",
    ],
);

/// `revisions.csv` (reserved `revision` records).
pub(crate) const REVISIONS: TableSpec = spec(
    "revisions",
    &["key"],
    &[
        "date",
        "description",
        "state",
        "author",
        "checker",
        "changed_pages",
    ],
);

/// `plc_config_projects.csv`.
pub(crate) const PLC_CONFIG_PROJECTS: TableSpec =
    spec("plc_config_projects", &["key", "name", "profile"], &["cpu"]);

/// `part_plc.csv` (the PLC section of a part copy).
pub(crate) const PART_PLC: TableSpec = spec(
    "part_plc",
    &["key", "role"],
    &[
        "io",
        "channel_count",
        "channel_kind",
        "data_type",
        "designation",
        "bit_of",
        "unbound",
    ],
);

/// `channels.csv`.
pub(crate) const CHANNELS: TableSpec = spec(
    "channels",
    &["key", "module", "number", "kind"],
    &["address", "state", "symbol", "comment", "binding", "target"],
);

/// `device_links.csv`.
pub(crate) const DEVICE_LINKS: TableSpec = spec("device_links", &["key", "from", "to", "kind"], &[]);

/// Every known table.
pub(crate) const ALL: [TableSpec; 28] = [
    PROJECT,
    PARTIES,
    STRUCTURE,
    EXTERNAL_REFS,
    SECTIONS,
    PAGES,
    REPORT_BLOCKS,
    PARTS,
    PART_CPS,
    PART_TEMPLATES,
    CABLE_CORES,
    DEVICES,
    TERMINALS,
    FUNCTIONS,
    REPRESENTATIONS,
    CONNECTIONS,
    JUNCTIONS,
    CONDUCTORS,
    POTENTIALS,
    BONDS,
    COLOUR_PROFILE,
    SIZE_PROFILE,
    PROFILES,
    REVISIONS,
    PLC_CONFIG_PROJECTS,
    PART_PLC,
    CHANNELS,
    DEVICE_LINKS,
];
