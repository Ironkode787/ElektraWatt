// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cycle 39's wave-2 window-check findings (lane M39) at shell level, on Golden A: the palette's `:` refusal and the
//! title after a folio, the form's hint, Edit ▸ Delete with nothing selected, the part editor's kind words and the
//! Place HUD's symbol name.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::actions::{self, PaletteEntry};
use ew_viewmodel::shell::{Mode, ShellModel};
use ew_viewmodel::strings;
use serde_json::json;

fn query(shell: &mut ShellModel, q: &str) -> Vec<PaletteEntry> {
    if shell.palette().is_none() {
        done(shell, "tools.palette", json!({}));
    }
    shell.set_palette_query(q);
    shell.palette_entries()
}

/// `:999` lists nothing and says the page jump's refusal, not "No matching command"; an action query that matches
/// nothing still says "No matching command".
#[test]
fn a_missing_folio_says_no_page_has_it() {
    let tmp = Tmp::new("m39-folio");
    let mut shell = tmp.shell_in(Mode::Gui, true, "103");
    assert!(query(&mut shell, ":999").is_empty());
    assert_eq!(shell.palette_empty_text(), "No page has the folio 999.");
    assert!(query(&mut shell, "zzzzqqqq").is_empty());
    assert_eq!(shell.palette_empty_text(), strings::text("palette-no-match"));
}

/// `:102` shows the page's title after the folio, never the folio twice.
#[test]
fn a_folio_row_shows_the_page_title() {
    let tmp = Tmp::new("m39-title");
    let mut shell = tmp.shell_in(Mode::Gui, true, "103");
    let title = shell
        .page_rows()
        .iter()
        .find(|r| r.folio == "102")
        .map(|r| r.title.clone())
        .expect("Golden A has a page 102");
    assert!(!title.is_empty());
    let rows = query(&mut shell, ":102");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].label, "102");
    assert_eq!(rows[0].breadcrumb, title);
}

/// The palette form's hint text.
#[test]
fn the_form_hint_says_enter_and_escape() {
    assert_eq!(
        strings::text("palette-form-hint"),
        "Enter applies \u{b7} Escape goes back"
    );
}

/// Edit ▸ Delete deletes the canvas selection only: with nothing selected it is disabled with "Nothing is selected",
/// like Cut; with a selection it is enabled.
#[test]
fn delete_is_disabled_with_nothing_selected() {
    let tmp = Tmp::new("m39-delete");
    let mut shell = tmp.shell_in(Mode::Gui, true, "103");
    let meta = actions::find("edit.delete_selection").expect("the action");
    done(&mut shell, "select.set", json!({"refs": []}));
    assert_eq!((meta.enabled)(&shell), Err("reason-nothing-selected"));
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    done(&mut shell, "select.set", json!({"refs": ["tag:-102F1"]}));
    assert_eq!((meta.enabled)(&shell), Ok(()));
}

/// The part editor's kind words: every offered kind reads as a word, never with an underscore.
#[test]
fn part_kinds_read_as_words() {
    use ew_viewmodel::shell::part_dialog::{PART_KINDS, part_kind_word};
    assert_eq!(part_kind_word("device"), "Device");
    assert_eq!(part_kind_word("end_bracket"), "End bracket");
    assert_eq!(part_kind_word("mounting_plate"), "Mounting plate");
    assert_eq!(part_kind_word(""), "");
    assert!(PART_KINDS.iter().all(|k| !part_kind_word(k).contains('_')));
}

/// The Place tool's HUD names the symbol in words, not by its key.
#[test]
fn the_place_hud_names_the_symbol() {
    let tmp = Tmp::new("m39-hud");
    let mut shell = tmp.shell_in(Mode::Gui, true, "103");
    done(&mut shell, "tool.place", json!({}));
    done(
        &mut shell,
        "place.pick",
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "S"}),
    );
    let hud = ew_viewmodel::command_bar::hud(&shell);
    assert!(!hud.prompt.contains("ew.iec."), "{}", hud.prompt);
    assert!(!hud.prompt.contains("@qet10"), "{}", hud.prompt);
    // The symbol's name in words, from the catalogue cached while the tool is armed.
    assert!(hud.prompt.starts_with("Contact, normally open"), "{}", hud.prompt);
}

/// One pending rule for the Messages rows and the Properties chips: an open completeness finding on the editing
/// page, the folio compared case-folded; nothing is pending without an editing page.
#[test]
fn chips_and_rows_share_the_pending_rule() {
    use ew_viewmodel::ew_api::output::FindingChip;
    use ew_viewmodel::navigators::messages::{chip_pending, pending_on};
    let chip = |page: Option<&str>| FindingChip {
        rule: "EW-CORE-041".into(),
        severity: "warning".into(),
        message: "m".into(),
        pending_on: page.map(str::to_owned),
    };
    assert!(pending_on(true, Some("101a"), Some("101A")));
    assert!(!pending_on(false, Some("101"), Some("101")));
    assert!(!pending_on(true, Some("101"), None));
    assert!(chip_pending(&chip(Some("101a")), Some("101A")));
    assert!(!chip_pending(&chip(Some("102")), Some("101")));
    assert!(!chip_pending(&chip(None), Some("101")));
}
