// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The two profiles of the sample project's company convention, as JSON text: the colour
//! profile "DBU/WH 24 V" and the size profile "0.75/1.5". They are the values of the members `colour_profile` and
//! `size_profile` of `fixtures/sample/sample.ew/project.jsonl` (the CC0 sample project, NFR-DOC-04), compact and with
//! sorted keys, compiled in from `content/profiles/colour/company.dbu-wh-24v.json` and
//! `content/profiles/size/company.0.75-1.5.json`. The company-standard wizard starts from them (doc 16 section 9.2, steps 3 and 4), and with its
//! defaults reproduces the sample project's settings, which a test checks against the sample project itself.
//!
//! Beside them, the content the wizard ships, compiled in from `content/`: the frame and title-block forms (step 2), the colour profiles "EN 60204-1 machine" and "IEC 60445 installation" (step 3) and the standard
//! layout of the report blocks (step 9).

/// The colour profile "DBU/WH 24 V" (IEC 60757 codes with the company code DBU; basis: company convention).
pub const COLOUR_PROFILE_SAMPLE: &str =
    include_str!("../../../content/profiles/colour/company.dbu-wh-24v.json");

/// The size profile "0.75/1.5": control circuits 0.75 mm2, mains circuits 1.5 mm2.
pub const SIZE_PROFILE_SAMPLE: &str = include_str!("../../../content/profiles/size/company.0.75-1.5.json");

/// The shipped frame and title-block forms and their keys (one place: `ew_api::output`). The wizard embeds them into
/// the basic project and names them in its frame defaults (doc 16 section 9.2, step 2); the sample project holds the
/// same copies.
pub use ew_api::output::{A3_GRID_FRAME, GRID_FRAME, NOGRID_FRAME, SHIPPED_FORMS, TITLE_BLOCK};

/// The shipped colour profiles of `content/profiles/colour` (doc 16 section 9.2, step 3), by the choice of
/// the `colour` field: "EN 60204-1 machine" and "IEC 60445 installation". Each is a value of the settings member
/// `colour_profile`, the project's own table of doc 08 section 4.3 (cited by standard and clause, no standard text),
/// shipped with `verified: false`.
pub const SHIPPED_COLOUR_PROFILES: [(&str, &str); 2] = [
    (
        "en60204-machine",
        include_str!("../../../content/profiles/colour/ew.colour.en60204-1-machine.json"),
    ),
    (
        "iec60445-installation",
        include_str!("../../../content/profiles/colour/ew.colour.iec60445-installation.json"),
    ),
];

/// The standard layout of the report blocks (doc 16 section 9.2, step 9): report kind, first folio and page
/// title in English and in Dutch, which the block holds as one multi-language title so that its pages print it in the
/// project's language (doc 09 §3.8). The sample project's blocks are these, on the shipped frame without the grid; its
/// three terminal-diagram blocks 903 to 905 select one strip each, which a basic project has none of, so the basic
/// project holds one terminal-diagram block at 903 over every strip (its pages continue as 903.1, 903.2), and the
/// titles that name the strip or the device of a selection (`{strip}`, `{device}`) count pages instead (`{n}`).
pub const STANDARD_REPORT_LAYOUT: [(&str, u32, &str, &str); 9] = [
    ("toc", 2, "Table of Contents {n}", "Inhoudsopgave {n}"),
    ("parts_summary", 901, "Partlist {n}", "Stuklijst {n}"),
    (
        "terminal_diagram",
        903,
        "Terminal Diagram {n}",
        "Klemmenschema {n}",
    ),
    (
        "cable_overview_diagrams",
        906,
        "Cable Overview and Diagrams",
        "Kabeloverzicht en kabelschema's",
    ),
    ("wire_list", 907, "Wire List {n}", "Draadlijst {n}"),
    ("plc_overview", 908, "PLC Overview {n}", "PLC-overzicht {n}"),
    (
        "potential_overview",
        909,
        "Potential Overview",
        "Potentiaaloverzicht",
    ),
    (
        "structure_overview",
        910,
        "Device Tag Overview {n}",
        "Kenmerkenoverzicht {n}",
    ),
    ("revision_overview", 911, "Revision Overview", "Revisieoverzicht"),
];
