// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The two profiles of the sample project's company convention, as JSON text (WP-S2.01 part 2, FT-308): the colour
//! profile "DBU/WH 24 V" and the size profile "0.75/1.5". They are the values of the members `colour_profile` and
//! `size_profile` of `golden/J-E-2416.ew/project.jsonl` (the CC0 sample project, NFR-DOC-04), compact and with
//! sorted keys. The company-standard wizard starts from them (doc 16 section 9.2, steps 3 and 4), and with its
//! defaults reproduces the sample project's settings, which a test checks against the sample project itself.
//!
//! Beside them, the content the wizard ships, compiled in from `content/`: the frame and title-block forms (step 2,
//! FT-336), the colour profiles "EN 60204-1 machine" and "IEC 60445 installation" (step 3, FT-337) and the golden
//! layout of the report blocks (step 9, FT-341).

/// The colour profile "DBU/WH 24 V" (IEC 60757 codes with the company code DBU; basis: company convention).
pub const COLOUR_PROFILE_SAMPLE: &str = r##"{"basis":"company_convention","basis_note":{"en":"company convention; codes per IEC 60757, DBU is a company code (BU, dark shade)"},"codes":[{"base":"BU","code":"DBU","export_as":"DBU","shade":"dark"}],"key":"company.dbu-wh-24v","name":{"en":"DBU/WH 24 V"},"rows":[{"colour":["DBU"],"colour_name":{"en":"Dark Blue"},"function":{"en":"Positive"},"group":{"en":"24 VDC"},"potential_type":{"en":"L+"},"select":{"ac_dc":"dc","ptype":["l_plus"],"u_max":"120"}},{"colour":["DBU"],"colour_name":{"en":"Dark Blue"},"function":{"en":"Signal"},"group":{"en":"24 VDC"},"potential_type":{"en":"signal, 24 V DC"},"select":{"ac_dc":"dc","ptype":["signal"],"u_max":"120"}},{"colour":["WH"],"colour_name":{"en":"White"},"function":{"en":"0V"},"group":{"en":"24 VDC"},"potential_type":{"en":"M"},"select":{"ac_dc":"dc","ptype":["m"],"u_max":"120"}},{"colour":["BK"],"colour_name":{"en":"Black"},"function":{"en":"Live"},"group":{"en":"230 VAC"},"potential_type":{"en":"L"},"select":{"ac_dc":"ac","ptype":["l"],"u_max":"250","u_min":"50"}},{"colour":["BK"],"colour_name":{"en":"Black"},"function":{"en":"Signal"},"group":{"en":"230 VAC"},"potential_type":{"en":"signal, 230 V AC"},"select":{"ac_dc":"ac","ptype":["signal"],"u_max":"250","u_min":"50"}},{"colour":["BU"],"colour_name":{"en":"Blue"},"function":{"en":"Neutral"},"group":{"en":"230 VAC"},"potential_type":{"en":"N"},"select":{"ac_dc":"ac","ptype":["n"],"u_max":"250","u_min":"50"}},{"colour":["GN","YE"],"colour_name":{"en":"Yellow / Green"},"function":{"en":"Earth"},"group":{"en":"230 VAC"},"potential_type":{"en":"PE"},"select":{"ptype":["pe"]}},{"colour":["BK"],"colour_name":{"en":"Black"},"function":{"en":"Phase 1"},"group":{"en":"400 VAC"},"potential_type":{"en":"L1"},"select":{"ac_dc":"ac","ptype":["l1"],"u_max":"1000","u_min":"250"}},{"colour":["BK"],"colour_name":{"en":"Black"},"function":{"en":"Phase 2"},"group":{"en":"400 VAC"},"potential_type":{"en":"L2"},"select":{"ac_dc":"ac","ptype":["l2"],"u_max":"1000","u_min":"250"}},{"colour":["BK"],"colour_name":{"en":"Black"},"function":{"en":"Phase 3"},"group":{"en":"400 VAC"},"potential_type":{"en":"L3"},"select":{"ac_dc":"ac","ptype":["l3"],"u_max":"1000","u_min":"250"}},{"colour":["BU"],"colour_name":{"en":"Blue"},"function":{"en":"Neutral"},"group":{"en":"400 VAC"},"potential_type":{"en":"N"},"select":{"ac_dc":"ac","ptype":["n"],"u_max":"1000","u_min":"250"}},{"colour":["GN","YE"],"colour_name":{"en":"Yellow / Green"},"function":{"en":"Earth"},"group":{"en":"400 VAC"},"potential_type":{"en":"PE"},"select":{"ptype":["pe"]}},{"colour":["GY"],"colour_name":{"en":"Gray"},"function":{"en":"Positive"},"group":{"en":"Analogue Signals"},"potential_type":{"en":"signal +"},"select":{"analogue":true,"ptype":["l_plus"]}},{"colour":["WH"],"colour_name":{"en":"White"},"function":{"en":"Negative"},"group":{"en":"Analogue Signals"},"potential_type":{"en":"signal −"},"select":{"analogue":true,"ptype":["l_minus"]}},{"colour":["GN","YE"],"colour_name":{"en":"Yellow / Green"},"function":{"en":"Earth"},"group":{"en":"Analogue Signals"},"potential_type":{"en":"PE"},"select":{"ptype":["pe"]}}],"standards":{"en":"IEC60757 / EN60204-1"},"verified":true}"##;

/// The size profile "0.75/1.5": control circuits 0.75 mm2, mains circuits 1.5 mm2.
pub const SIZE_PROFILE_SAMPLE: &str = r##"{"bond_rule":{"class_from":"non_pe_side","colour_from":"pe"},"class_rules":[{"class":"pe","select":{"ptype":["pe","pen","fe"]}},{"class":"bus","select":{"discipline":"bus"}},{"class":"analogue","select":{"analogue":true}},{"class":"mains","select":{"ac_dc":"ac","u_min":"50"}},{"class":"control","select":{}}],"key":"company.0.75-1.5","name":{"en":"0.75/1.5"},"pe_class":"highest_at_ends","rows":[{"class":"control","codes":[{"code":"DBU","order":1},{"code":"GNYE","order":5},{"code":"WH","order":2}],"csa":"0.75"},{"class":"mains","codes":[{"code":"BK","order":3},{"code":"BU","order":4},{"code":"GNYE","order":5}],"csa":"1.5"}],"verified":true}"##;

/// The shipped frame and title-block forms of `content/forms` (the QElectroTech-compatible A4 landscape frames with
/// and without the grid, the cover frame, and the title block they name), in the `lib/` JSON form that
/// `library.embed` takes. The wizard embeds them into the basic project and names them in its frame defaults (doc
/// 16 section 9.2, step 2; FT-336); the sample project holds the same copies.
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

/// The shipped colour profiles of `content/profiles/colour` (FT-337; doc 16 section 9.2, step 3), by the choice of
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

/// The golden layout of the report blocks (doc 16 section 9.2, step 9; FT-341): report kind, first folio and page
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
