// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Import… of Parts, the doors (doc 07 §3.6, §5.9), as the headless action `parts.import`: a maker's table of his own
//! parts mapped to the declared fields as the set's first step, the set in the list as This import (new, the same,
//! refused with why), the map changed and the set following, Save writing My parts and the map for its source, with or
//! without a project; File ▸ Import… of a table going on to Parts; a refused table; the window's file dialog. The
//! picker lists a project's parts by maker and number.

use crate::common;

use std::path::{Path, PathBuf};

use common::{Tmp, done, refused};
use ew_api::Session;
use ew_api::import::read_parts_tables;
use ew_viewmodel::ew_api::library::LibraryTiers;
use ew_viewmodel::shell::{Mode, Outcome, Prompt, ShellConfig, ShellModel};
use ew_viewmodel::tools::PickerRow;
use serde_json::json;

const VERSION: &str = "0.0.0-test";

fn table_parts() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/tables/parts.csv")
}

/// A shell whose person has a user folder under `tmp` (My parts is made there on the first Save).
fn with_my_parts(tmp: &Tmp, mode: Mode) -> ShellModel {
    let mine = LibraryTiers::under(&tmp.0.join("data"), &tmp.0.join("cache"), Vec::new(), Vec::new());
    ShellModel::new(mode, tmp.config().with_libraries(Some(mine)))
}

/// A maker's table of his own parts: a cabinet, a terminal, a row without a part number.
const TABLE: &str = "Maker;Article;Kind;Description;W;H;D;Price
Example;ENC-8-10-3;enclosure;Wall enclosure 800 x 1000 x 300 mm;800;1000;300;412.00
Example;FT-4;terminal;Feed-through terminal 4 mm2, screw;6.2;60;45;1.10
Example;;rail;Rail 35 x 15 mm, 2 m;;;;9.00
";

fn table(tmp: &Tmp) -> PathBuf {
    let file = tmp.0.join("example-parts.csv");
    std::fs::write(&file, TABLE).unwrap();
    file
}

/// The lines of the describe text that start with `prefix` (after their indent).
fn lines_with(shell: &ShellModel, prefix: &str) -> Vec<String> {
    shell
        .describe()
        .lines()
        .map(str::trim_start)
        .filter(|l| l.starts_with(prefix))
        .map(str::to_owned)
        .collect()
}

/// A table goes into Parts as a set: its columns pre-filled where a header names a field, the price not imported;
/// the sizes mapped, the set follows; Save writes the new parts into My parts and saves the map for the source.
#[test]
fn a_maker_s_table_comes_into_parts_as_a_set_and_save_writes_my_parts() {
    let tmp = Tmp::new("parts-set-flow");
    let file = table(&tmp);
    let mut shell = with_my_parts(&tmp, Mode::Headless);
    common::make_project(&mut shell, &tmp.0.join("proj.ew"), &[]);
    done(
        &mut shell,
        "parts.import",
        json!({"path": file.to_string_lossy()}),
    );
    assert_eq!(shell.screen(), ew_viewmodel::shell::Screen::Parts);
    let place = shell.parts_place().expect("Parts");
    assert_eq!(place.scope, ew_viewmodel::ew_api::parts::PartScope::Import);
    // Maker, Kind and Description name their fields; Article, W, H and D are his to map.
    let columns = lines_with(&shell, "column ");
    assert!(
        columns.contains(&"column \"Maker\" \"Example\" manufacturer".to_owned()),
        "{columns:?}"
    );
    assert!(
        columns.contains(&"column \"Price\" \"412.00\" -".to_owned()),
        "{columns:?}"
    );
    assert_eq!(
        lines_with(&shell, "refused \""),
        ["refused \"Map a column to Identity \u{25b8} Part number\""]
    );
    for (column, member) in [
        ("Article", "part_number"),
        ("W", "width"),
        ("H", "height"),
        ("D", "depth"),
    ] {
        done(
            &mut shell,
            "parts.map",
            json!({"column": column, "member": member}),
        );
    }
    assert!(lines_with(&shell, "refused \"").is_empty());
    assert_eq!(
        lines_with(&shell, "set \""),
        ["set \"example-parts.csv\" new 2 changed 0 same 0 refused 1"]
    );
    let sentences = lines_with(&shell, "\"Row ");
    assert_eq!(
        sentences,
        [
            "\"Row 4 is refused: it has no part number. Give it one in the file and read it again, or leave it out.\""
        ]
    );
    // The cabinet's row shows its outer size as it will be.
    done(&mut shell, "parts.open", json!({"row": 0}));
    assert!(
        lines_with(&shell, "as-it-will-be sections.enclosure.width")
            .contains(&"as-it-will-be sections.enclosure.width \"800 mm\"".to_owned()),
        "{}",
        shell.describe()
    );
    // A field that is no choice of the column is refused.
    let out = shell.dispatch("parts.map", &json!({"column": "Price", "member": "price"}));
    assert!(matches!(out, Outcome::Failed { .. }), "{out:?}");
    done(&mut shell, "parts.save", json!({}));
    assert!(shell.import_set().is_none(), "the set went into My parts");
    assert_eq!(
        common::state(&shell)["toast"]["text"],
        "example-parts.csv saved to My parts: 2 new, 0 changed."
    );
    done(
        &mut shell,
        "parts.find",
        json!({"text": "ENC-8-10-3", "scope": "mine"}),
    );
    assert!(
        shell
            .parts_place()
            .unwrap()
            .found
            .iter()
            .any(|f| f.offer.part_number == "ENC-8-10-3")
    );
    // Nothing went into the project: a project takes a part when a device uses it.
    assert!(!shell.can_undo());
    // The map is saved for the source and fills the next table of it: the same parts, now the same.
    let map =
        ew_api::catalog::load_column_map(&tmp.0.join("config"), "example-parts").expect("the map is saved");
    assert_eq!(map.header(&ew_api::catalog::Member::field("width")), Some("W"));
    done(
        &mut shell,
        "parts.import",
        json!({"path": file.to_string_lossy()}),
    );
    assert_eq!(
        lines_with(&shell, "set \""),
        ["set \"example-parts.csv\" new 0 changed 0 same 2 refused 1"]
    );
    assert_eq!(
        refused(&mut shell, "parts.save", json!({})),
        "reason-set-nothing-new"
    );
    done(&mut shell, "parts.discard", json!({}));
    assert!(shell.import_set().is_none());
    shell.shutdown();
}

/// With no project the place over the libraries alone takes the table; a read-only project shows the set and refuses
/// its Save in words.
#[test]
fn a_set_is_saved_without_a_project_and_refused_on_a_read_only_one() {
    let tmp = Tmp::new("parts-set-alone");
    let file = table(&tmp);
    let mut shell = with_my_parts(&tmp, Mode::Headless);
    done(
        &mut shell,
        "parts.import",
        json!({"path": file.to_string_lossy()}),
    );
    assert!(shell.parts_without_project());
    done(
        &mut shell,
        "parts.map",
        json!({"column": "Article", "member": "part_number"}),
    );
    done(&mut shell, "parts.save", json!({}));
    assert!(!shell.has_project(), "no project is made");
    shell.shutdown();

    let tmp = Tmp::new("parts-set-read-only");
    let file = table(&tmp);
    let mut shell = tmp.sample_shell("101");
    done(
        &mut shell,
        "parts.import",
        json!({"path": file.to_string_lossy()}),
    );
    done(
        &mut shell,
        "parts.map",
        json!({"column": "Article", "member": "part_number"}),
    );
    assert_eq!(refused(&mut shell, "parts.save", json!({})), "reason-read-only");
    shell.shutdown();
}

/// File ▸ Import… of a table that names its maker and number goes on to Parts.
#[test]
fn file_import_of_a_parts_table_goes_on_to_parts() {
    let tmp = Tmp::new("parts-set-file-import");
    let file = tmp.0.join("list.csv");
    std::fs::write(&file, "Maker;Part number\nExample;FT-4\n").unwrap();
    let mut shell = with_my_parts(&tmp, Mode::Headless);
    common::make_project(&mut shell, &tmp.0.join("proj.ew"), &[]);
    done(&mut shell, "file.import", json!({"file": file.to_string_lossy()}));
    assert_eq!(shell.screen(), ew_viewmodel::shell::Screen::Parts);
    assert_eq!(
        lines_with(&shell, "set \""),
        ["set \"list.csv\" new 1 changed 0 same 0 refused 0"]
    );
    shell.shutdown();
}

/// A table that cannot be read says why, with its pointer, and opens no set.
#[test]
fn a_bad_table_is_refused_with_its_pointer_and_opens_no_set() {
    let tmp = Tmp::new("parts-bad");
    let file = tmp.0.join("parts.csv");
    // A table without a header row cannot be mapped.
    std::fs::write(&file, "\n\n").unwrap();
    let mut shell = with_my_parts(&tmp, Mode::Headless);
    common::make_project(&mut shell, &tmp.0.join("proj.ew"), &[]);
    let out = shell.dispatch("parts.import", &json!({"path": file.to_string_lossy()}));
    let Outcome::Failed { message } = out else {
        panic!("{out:?}");
    };
    assert!(message.starts_with("The table is not read:"), "{message}");
    assert!(message.contains("header row"), "{message}");
    assert!(shell.import_set().is_none(), "no set");
    shell.shutdown();
}

/// The window asks for the file in its dialog; a script names it.
#[test]
fn the_window_asks_for_the_file_and_headless_needs_the_parameter() {
    let tmp = Tmp::new("parts-prompt");
    let mut headless_shell = with_my_parts(&tmp, Mode::Headless);
    let out = headless_shell.dispatch("parts.import", &json!({}));
    assert!(matches!(out, Outcome::Failed { .. }), "{out:?}");
    headless_shell.shutdown();

    let tmp = Tmp::new("parts-prompt-gui");
    let mut gui = ShellModel::new(Mode::Gui, ShellConfig::in_dir(&tmp.0.join("config"), VERSION));
    done(&mut gui, "parts.open", json!({}));
    done(&mut gui, "parts.import", json!({}));
    let prompt = gui.take_prompt().expect("a prompt for the window");
    assert_eq!(prompt.action, "parts.import");
    assert_eq!(prompt.prompt, Prompt::OpenPartsTable);
    assert_eq!(prompt.param, "path");
    // On the Symbols shelf Import… asks for a symbol file.
    done(&mut gui, "parts.find", json!({"shelf": "symbols"}));
    done(&mut gui, "parts.import", json!({}));
    let prompt = gui.take_prompt().expect("a prompt for the window");
    assert_eq!(prompt.prompt, Prompt::OpenSymbol);
    gui.shutdown();
}

/// The picker lists the project's own parts among the libraries', by maker, in part-number order within one.
#[test]
fn the_picker_lists_the_project_s_parts_by_manufacturer_in_part_number_order() {
    let tmp = Tmp::new("parts-picker");
    let mut shell = with_my_parts(&tmp, Mode::Headless);
    let dir = tmp.0.join("proj.ew");
    common::make_project(&mut shell, &dir, &[]);
    done(&mut shell, "file.save", json!({}));
    let (session, _) = Session::open_read_only(&dir, VERSION).unwrap();
    let read = read_parts_tables(&table_parts(), &session).unwrap();
    drop(session);
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "parts.import", "params": read.params()}),
    );
    done(&mut shell, "tool.place_part", json!({}));
    let picker = shell.picker().expect("the picker is open");
    let rows: Vec<(&str, &str, bool)> = picker
        .rows
        .iter()
        .filter_map(|r| match r {
            PickerRow::Part(p) => {
                let part = &picker.catalogue.parts[*p];
                Some((
                    part.manufacturer.as_str(),
                    part.part_number.as_str(),
                    part.library.is_some(),
                ))
            }
            _ => None,
        })
        .collect();
    let keys: Vec<(&str, &str)> = rows.iter().map(|(m, n, _)| (*m, *n)).collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(keys, sorted, "manufacturer, then part-number order");
    assert!(
        rows.iter().any(|r| !r.2) && rows.iter().any(|r| r.2),
        "both groups listed"
    );
    shell.shutdown();
}

/// A family of cables from a table: the column Cores (mapped by its name) gives each cable that many numbered cores,
/// and once saved the Cable tool offers it.
#[test]
fn a_cores_column_gives_a_cable_its_cores_and_the_cable_tool_offers_it() {
    let tmp = Tmp::new("parts-set-cores-vm");
    let file = tmp.0.join("cables.csv");
    std::fs::write(
        &file,
        "Maker;Article;Kind;Description;Cores
Example;CY-7x1.5;cable;Control cable, 7 cores 1.5 mm2;7
",
    )
    .unwrap();
    let mut shell = with_my_parts(&tmp, Mode::Gui);
    common::make_project(&mut shell, &tmp.0.join("proj.ew"), &[]);
    done(
        &mut shell,
        "parts.import",
        json!({"path": file.to_string_lossy()}),
    );
    let columns = lines_with(&shell, "column ");
    assert!(
        columns.contains(&"column \"Cores\" \"7\" cable.cores".to_owned()),
        "{columns:?}"
    );
    done(
        &mut shell,
        "parts.map",
        json!({"column": "Article", "member": "part_number"}),
    );
    assert_eq!(
        lines_with(&shell, "set \""),
        ["set \"cables.csv\" new 1 changed 0 same 0 refused 0"]
    );
    done(&mut shell, "parts.save", json!({}));
    done(&mut shell, "parts.back", json!({}));
    done(&mut shell, "cable.new", json!({}));
    let offered = common::form(&shell).fields.iter().any(|f| {
        f.options
            .iter()
            .any(|(v, w)| v.contains("CY-7x1.5") || w.contains("CY-7x1.5"))
    });
    assert!(offered, "{:?}", common::form(&shell).fields);
    shell.shutdown();
}

/// A description's column names its language in the window's language.
#[test]
fn a_description_s_language_is_named_in_the_window_s_language() {
    use ew_viewmodel::shell::PartsMap;
    assert_eq!(
        PartsMap::member_text("description.nl", "Text NL"),
        "Identity \u{25b8} Description, Dutch"
    );
    assert!(ew_viewmodel::strings::set_locale("nl"));
    let dutch = PartsMap::member_text("description.en", "Text EN");
    assert!(ew_viewmodel::strings::set_locale("en"));
    assert!(dutch.ends_with(", Engels"), "{dutch}");
}

/// The column map is saved with the set it read: a Save whose write fails leaves no map for the source (review finding
/// 14 of lane 3), and the set stays to be saved again.
#[test]
fn a_set_whose_write_fails_saves_no_map() {
    let tmp = Tmp::new("parts-set-write-fails");
    let file = table(&tmp);
    let mut shell = with_my_parts(&tmp, Mode::Headless);
    common::make_project(&mut shell, &tmp.0.join("proj.ew"), &[]);
    // My parts is made by a part saved on its sheet; then its folder's manifest is spoilt, so its next write fails.
    done(&mut shell, "parts.new", json!({"start": "terminal"}));
    for (f, v) in [("manufacturer", "Example"), ("part_number", "DL-2.5")] {
        done(&mut shell, "parts.field", json!({"field": f, "value": v}));
    }
    done(&mut shell, "parts.save", json!({}));
    let my_parts = tmp.0.join("data").join("libraries").join("user-parts.ewlib");
    std::fs::write(my_parts.join("library.json"), "not a manifest").unwrap();
    done(
        &mut shell,
        "parts.import",
        json!({"path": file.to_string_lossy()}),
    );
    done(
        &mut shell,
        "parts.map",
        json!({"column": "Article", "member": "part_number"}),
    );
    common::failed(&mut shell, "parts.save", json!({}));
    assert!(
        ew_api::catalog::load_column_map(&tmp.0.join("config"), "example-parts").is_err(),
        "no map is saved for a set that was not written"
    );
    assert!(shell.import_set().is_some(), "the set stays");
    shell.shutdown();
}

/// The parts are written, then the map of the columns cannot be kept (its folder is a file): the toast says both, the
/// set leaves the list for My parts as on any Save, and the parts are found there.
#[test]
fn a_map_that_cannot_be_kept_after_the_parts_were_saved_is_said_beside_them() {
    let tmp = Tmp::new("parts-set-map-fails");
    let file = table(&tmp);
    let mut shell = with_my_parts(&tmp, Mode::Headless);
    common::make_project(&mut shell, &tmp.0.join("proj.ew"), &[]);
    let config = tmp.0.join("config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join(ew_api::catalog::MAPS_DIR), "not a folder").unwrap();
    done(
        &mut shell,
        "parts.import",
        json!({"path": file.to_string_lossy()}),
    );
    done(
        &mut shell,
        "parts.map",
        json!({"column": "Article", "member": "part_number"}),
    );
    done(&mut shell, "parts.save", json!({}));
    assert!(shell.import_set().is_none(), "the set went into My parts");
    let toast = common::state(&shell)["toast"]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(
        toast.starts_with(
            "example-parts.csv saved to My parts: 2 new, 0 changed. Its map of the columns was not kept"
        ),
        "{toast}"
    );
    assert_eq!(
        shell.parts_place().map(|p| p.scope),
        Some(ew_viewmodel::ew_api::parts::PartScope::Mine)
    );
    shell.shutdown();
}
