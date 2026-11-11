// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The two profiles of the sample project's company convention, as JSON text: the colour
//! profile "DBU/WH 24 V" and the size profile "0.75/1.5". They are the values of the members `colour_profile` and
//! `size_profile` of `golden/J-E-2416.ew/project.jsonl` (the CC0 sample project, NFR-DOC-04), compact and with
//! sorted keys, compiled in from `content/profiles/colour/company.dbu-wh-24v.json` and
//! `content/profiles/size/company.0.75-1.5.json`. The company-standard wizard starts from them (doc 16 section 9.2, steps 3 and 4), and with its
//! defaults reproduces the sample project's settings, which a test checks against the sample project itself.
//!
//! Beside them, the content the wizard ships, compiled in from `content/`: the frame and title-block forms (step 2,
//! FT-336), the colour profiles "EN 60204-1 machine" and "IEC 60445 installation" (step 3) and the golden
//! layout of the report blocks (step 9).

/// The colour profile "DBU/WH 24 V" (IEC 60757 codes with the company code DBU; basis: company convention).
pub const COLOUR_PROFILE_SAMPLE: &str =
    include_str!("../../../content/profiles/colour/company.dbu-wh-24v.json");

/// The size profile "0.75/1.5": control circuits 0.75 mm2, mains circuits 1.5 mm2.
pub const SIZE_PROFILE_SAMPLE: &str = include_str!("../../../content/profiles/size/company.0.75-1.5.json");

/// The shipped frame and title-block forms of `content/forms` (the QElectroTech-compatible A4 landscape frames with
/// and without the grid, the cover frame, and the title block they name), in the `lib/` JSON form that
/// `library.embed` takes. The wizard embeds them into the basic project and names them in its frame defaults (doc
/// 16 section 9.2, step 2); the sample project holds the same copies.
pub const SHIPPED_FORMS: [&str; 4] = [
    include_str!("../../../content/forms/ew.frame.qet-a4-landscape-cover.json"),
    include_str!("../../../content/forms/ew.frame.qet-a4-landscape-grid.json"),
    include_str!("../../../content/forms/ew.frame.qet-a4-landscape-nogrid.json"),
    include_str!("../../../content/forms/ew.titleblock.qet-a4.json"),
];

/// The key of the shipped cover frame.
pub const COVER_FRAME: &str = "ew.frame.qet-a4-landscape-cover";
/// The key of the shipped frame with the column and row grid.
pub const GRID_FRAME: &str = "ew.frame.qet-a4-landscape-grid";
/// The key of the shipped frame without the grid.
pub const NOGRID_FRAME: &str = "ew.frame.qet-a4-landscape-nogrid";

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

/// The key of the shipped title block, which the frames name (the closure of a frame form).
pub const TITLE_BLOCK: &str = "ew.titleblock.qet-a4";

/// The golden layout of the report blocks (doc 16 section 9.2, step 9): report kind, first folio and page
/// title. The sample project's blocks are these, on the shipped frame without the grid; its three terminal-diagram
/// blocks 903 to 905 select one strip each, which a basic project has none of, so the basic project holds one
/// terminal-diagram block at 903 over every strip (its pages continue as 903.1, 903.2), and the titles that name
/// the strip or the device of a selection (`{strip}`, `{device}`) count pages instead (`{n}`).
pub const GOLDEN_REPORT_LAYOUT: [(&str, u32, &str); 9] = [
    ("toc", 2, "Table of Contents {n}"),
    ("parts_summary", 901, "Partlist {n}"),
    ("terminal_diagram", 903, "Terminal Diagram {n}"),
    ("cable_overview_diagrams", 906, "Cable Overview and Diagrams"),
    ("wire_list", 907, "Wire List {n}"),
    ("plc_overview", 908, "PLC Overview {n}"),
    ("potential_overview", 909, "Potential Overview"),
    ("structure_overview", 910, "Device Tag Overview {n}"),
    ("revision_overview", 911, "Revision Overview"),
];
