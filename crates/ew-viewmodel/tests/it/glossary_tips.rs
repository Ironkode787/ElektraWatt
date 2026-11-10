// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The glossary pane (GT-1, FR-UI-045), the one-time tips (GT-2, GT-3, FR-UI-048) and
//! the wire-colour view (GT-4, FR-UI-047), on copies of Golden A. Every text a person reads is asserted whole.

use crate::common;

use std::path::Path;
use std::sync::Arc;

use common::{Tmp, at, done};
use ew_viewmodel::ew_api::Session;
use ew_viewmodel::ew_api::output::{NavKind, PageTables};
use ew_viewmodel::layout::PaneKind;
use ew_viewmodel::notifications::{NoteAction, NoteTone};
use ew_viewmodel::settings::Settings;
use ew_viewmodel::shell::{Focus, GlossaryRow, Mode, Outcome, ShellModel};
use ew_viewmodel::wire_colours::{DASH_NM, iec_rgb};
use serde_json::json;

/// W29 on folio 103 (`connections|W29`): 0.75 mm² DBU from the colour profile.
const W29: &str = "9eb2d865-fca4-59b1-87bf-c073d7190ceb";

/// W27, the bonded wire between PE and 0 V (GNYE).
const W27: &str = "wire:\"PE-0VDC\" -> \"-X1032:2/a\"";

/// A shell with a writable copy of Golden A on `folio`, its settings file written first with `settings` (TOML).
fn shell_with(tmp: &Tmp, settings: &str, folio: &str) -> ShellModel {
    let config = tmp.0.join("config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("settings.toml"), settings).unwrap();
    tmp.shell_in(Mode::Headless, true, folio)
}

/// The settings file as the shell left it.
fn settings_of(tmp: &Tmp) -> Settings {
    Settings::load(&tmp.0.join("config")).settings
}

fn tables(s: &ShellModel) -> Arc<PageTables> {
    Arc::clone(&s.canvas().unwrap().page().tables)
}

/// A click at page point `p` through the grid cursor.
fn click_at(s: &mut ShellModel, p: (i64, i64)) {
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(p);
    done(s, "cursor.set", at(q, json!({})));
    done(s, "canvas.activate", json!({}));
}

/// A Line from a grid point beside the first connection point of the page to that point (tip 1's habit).
fn line_onto_a_cp(s: &mut ShellModel) {
    let cp = tables(s).cps[0].at;
    let sn = &s.canvas().unwrap().page().snapper;
    let from = (cp.0 - 4 * sn.page_of_index(1) + 4 * sn.page_of_index(0), cp.1);
    done(s, "tool.graphic", json!({"shape": "line"}));
    click_at(s, from);
    click_at(s, cp);
    done(s, "tool.select", json!({}));
}

/// The lines of the describe text that start with `prefix`.
fn lines(s: &ShellModel, prefix: &str) -> Vec<String> {
    s.describe()
        .lines()
        .filter(|l| l.starts_with(prefix))
        .map(str::to_owned)
        .collect()
}

/// The tip toasts in the log, oldest first, as (text, action).
fn tips(s: &ShellModel) -> Vec<(String, Option<NoteAction>)> {
    let mut v: Vec<_> = s
        .notifications()
        .entries()
        .filter(|n| n.key == "tip")
        .map(|n| (n.text.clone(), n.action()))
        .collect();
    v.reverse();
    v
}

#[test]
fn gt1_folio_report_finds_the_interruption_point_whose_insert_arms_the_ip_tool_and_dutch_finds_it_too() {
    let tmp = Tmp::new("gt1-glossary");
    let mut s = tmp.shell_in(Mode::Headless, true, "102");
    assert!(!s.glossary_shown());
    done(&mut s, "help.glossary", json!({}));
    assert!(s.glossary_shown());
    assert!(s.layout().panes().contains(&PaneKind::Glossary));
    assert!(s.bottom_dock_shown(), "the pane is in the bottom dock");
    // Every term is listed before a search.
    assert_eq!(s.glossary_rows().len(), ew_viewmodel::glossary::terms().len());
    done(&mut s, "glossary.filter", json!({"text": "folio report"}));
    let ip = GlossaryRow {
        term: "Interruption point".to_owned(),
        eplan: "Interruption point".to_owned(),
        qet: "Folio report".to_owned(),
        definition:
            "Where a potential leaves one page and continues on another; pairs are found by name, star or \
                     chain."
                .to_owned(),
        actions: vec!["tool.ip".to_owned()],
    };
    let rows = s.glossary_rows();
    assert!(rows.contains(&ip), "{rows:#?}");
    let line = format!("glossary filter \"folio report\" rows {}", rows.len());
    assert_eq!(lines(&s, "glossary "), [line]);
    assert!(
        lines(&s, "  term ")
            .contains(&"  term \"Interruption point\" eplan \"Interruption point\" qet \"Folio report\" actions tool.ip".to_owned()),
        "{}",
        s.describe()
    );
    // Insert: the row's action arms the IP tool.
    let id = ew_viewmodel::actions::find(&ip.actions[0]).unwrap().id;
    done(&mut s, id, json!({}));
    assert_eq!(s.tool().name(), "ip");
    // A search for a word of no term lists nothing.
    done(&mut s, "glossary.filter", json!({"text": "zzqx"}));
    assert_eq!(s.glossary_rows(), []);
    // In Dutch the term reads in Dutch and its Dutch word finds the same row.
    done(&mut s, "view.language", json!({"value": "nl"}));
    done(&mut s, "glossary.filter", json!({"text": "onderbrekingspunt"}));
    let nl = GlossaryRow {
        term: "Onderbrekingspunt".to_owned(),
        definition:
            "Waar een potentiaal een pagina verlaat en op een andere doorloopt; paren worden op naam \
                     gevonden, als ster of keten."
                .to_owned(),
        ..ip.clone()
    };
    assert!(s.glossary_rows().contains(&nl), "{:#?}", s.glossary_rows());
    // The English words still find it in Dutch.
    done(&mut s, "glossary.filter", json!({"text": "folio rep"}));
    assert!(s.glossary_rows().contains(&nl));
    done(&mut s, "view.language", json!({"value": "en"}));
    // Help ▸ Glossary again closes it.
    done(&mut s, "help.glossary", json!({}));
    assert!(!s.glossary_shown());
    assert_eq!(lines(&s, "glossary "), Vec::<String>::new());
    s.shutdown();
}

#[test]
fn gt2_with_origin_qet_a_line_onto_a_connection_point_shows_tip_1_once_and_convert_undoes_it_and_arms_wire() {
    let tmp = Tmp::new("gt2-tip1");
    let mut s = shell_with(
        &tmp,
        "version = 1\nfirst_run_done = true\ncomes_from = \"qet\"\n",
        "102",
    );
    assert!(!s.can_undo());
    let items = tables(&s).items.len();
    line_onto_a_cp(&mut s);
    assert!(s.can_undo());
    assert_eq!(tables(&s).items.len(), items + 1, "the line is placed");
    assert_eq!(
        tips(&s),
        [(
            "Lines do not connect. Draw a wire instead.".to_owned(),
            Some(NoteAction::Convert)
        )]
    );
    let toast = s.toast().cloned().expect("the tip's toast");
    assert!(s.notification_valid(&toast));
    assert_eq!(toast.tone(), NoteTone::Info, "a tip informs");
    assert_eq!(
        ew_viewmodel::strings::text(NoteAction::Convert.label_key()),
        "Convert to connection"
    );
    assert_eq!(
        lines(&s, "toast "),
        ["toast \"Lines do not connect. Draw a wire instead.\" convert"]
    );
    assert_eq!(settings_of(&tmp).tips_seen, ["line_on_cp"]);
    // Convert to connection: the line is undone in one step and the Wire tool is armed.
    done(
        &mut s,
        "notifications.run",
        json!({"entry": toast.serial, "action": "convert"}),
    );
    assert!(!s.can_undo(), "one undo step takes the line away");
    assert_eq!(tables(&s).items.len(), items);
    assert_eq!(s.tool().name(), "wire");
    assert!(
        s.toast().is_none_or(|t| t.serial != toast.serial),
        "the toast goes"
    );
    done(&mut s, "tool.select", json!({}));
    // The second time nothing is shown.
    line_onto_a_cp(&mut s);
    assert_eq!(tables(&s).items.len(), items + 1);
    assert_eq!(tips(&s).len(), 1);
    done(&mut s, "file.save", json!({}));
    s.shutdown();
    // Nor in the next session: the tip is remembered in the settings file.
    let mut s = tmp.shell_in(Mode::Headless, true, "102");
    line_onto_a_cp(&mut s);
    assert_eq!(tips(&s), []);
    s.shutdown();
}

/// View ▸ Show tips as the View menu holds it: (label, checked).
fn tips_item(s: &ShellModel) -> (String, bool) {
    ew_viewmodel::actions::menu_model(s)
        .iter()
        .find_map(|m| m.items().into_iter().find(|i| i.id == "view.tips"))
        .map(|i| (i.label.clone(), i.checked))
        .expect("View ▸ Show tips")
}

/// View ▸ Show tips switches the one-time tips, saved to `tips` in `settings.toml`
/// (FR-UI-048), and its check shows the setting.
#[test]
fn view_show_tips_switches_the_tips_and_saves_the_setting() {
    let tmp = Tmp::new("ft1280-tips");
    let mut s = shell_with(
        &tmp,
        "version = 1\nfirst_run_done = true\ncomes_from = \"qet\"\n",
        "102",
    );
    assert_eq!(tips_item(&s), ("Show tips".to_owned(), true));
    done(&mut s, "view.tips", json!({"on": false}));
    assert_eq!(tips_item(&s), ("Show tips".to_owned(), false));
    assert!(!settings_of(&tmp).tips);
    line_onto_a_cp(&mut s);
    assert_eq!(tips(&s), [], "no tip while off");
    done(&mut s, "edit.undo", json!({}));
    // Without `on` it switches the other way.
    done(&mut s, "view.tips", json!({}));
    assert_eq!(tips_item(&s), ("Show tips".to_owned(), true));
    assert!(settings_of(&tmp).tips);
    line_onto_a_cp(&mut s);
    assert_eq!(tips(&s).len(), 1, "the tip again once on");
    // In Dutch too.
    done(&mut s, "view.language", json!({"value": "nl"}));
    assert_eq!(tips_item(&s), ("Tips tonen".to_owned(), true));
    s.shutdown();
}

/// Tip 2's refusal: the Tag row of two devices (one devices' tag is edited by freezing it).
fn refuse_a_derived_tag(s: &mut ShellModel) {
    done(s, "select.set", json!({"refs": ["tag:-102F1", "tag:-102F2"]}));
    assert_eq!(
        s.dispatch(
            "props.action",
            &json!({"row": "device/derived.tag", "action": "edit"})
        ),
        Outcome::Refused {
            reason_key: ew_viewmodel::key!("reason-row-computed")
        }
    );
}

/// A read-only session over Golden A, for the model's own reads.
fn golden() -> Session {
    Session::open_read_only(Path::new(common::GOLDEN_A), "0.0.0-test")
        .unwrap()
        .0
}

/// The folio of the first generated page and the ID of its report block.
fn generated_page(s: &ShellModel) -> (String, String) {
    let row = s
        .page_rows()
        .iter()
        .find(|r| r.generated)
        .expect("Golden A has generated pages");
    let (page, folio) = (row.id, row.folio.clone());
    let block = golden()
        .property_sheet(&[page])
        .sections
        .iter()
        .find(|x| x.kind == "report_block")
        .expect("the block's section")
        .targets[0];
    (folio, block.to_string())
}

/// Tip 3's refusal: a row of a generated page's sheet (the page shown with nothing selected).
fn refuse_a_generated_page(s: &mut ShellModel, folio: &str) -> String {
    done(s, "page.goto", json!({ "folio": folio }));
    done(s, "select.set", json!({"ids": []}));
    let sheet = s.properties().sheet().expect("the page's sheet").clone();
    let row = sheet
        .sections
        .iter()
        .find(|x| x.kind == "report_block")
        .and_then(|x| x.rows.iter().find(|r| !r.key.contains("/derived.")))
        .expect("a row of the block")
        .key
        .clone();
    let out = s.dispatch("props.action", &json!({"row": row, "action": "edit"}));
    assert!(matches!(out, Outcome::Failed { .. }), "{out:?}");
    row
}

/// A generated page's refusal names the page's title with its number, never the block's title format, in
/// both languages.
#[test]
fn a_generated_page_refuses_in_the_words_of_its_numbered_title() {
    let tmp = Tmp::new("ft1282-generated");
    let mut s = shell_with(&tmp, "version = 1\nfirst_run_done = true\n", "102");
    let (page, _) = generated_page(&s);
    for (language, text) in [
        (
            "en",
            "This row cannot be edited: Table of Contents 1 is generated by its report block.",
        ),
        (
            "nl",
            "Deze rij kan niet worden bewerkt: Table of Contents 1 wordt gegenereerd door het rapportblok.",
        ),
    ] {
        done(&mut s, "view.language", json!({ "value": language }));
        let row = refuse_a_generated_page(&mut s, &page);
        let out = s.dispatch("props.action", &json!({"row": row, "action": "edit"}));
        assert_eq!(
            out,
            Outcome::Failed {
                message: text.to_owned()
            },
            "{language}"
        );
    }
    s.shutdown();
}

#[test]
fn gt3_tips_2_and_3_appear_once_each_on_their_refusals_and_never_with_tips_off_or_another_origin() {
    // Off by the setting, and off for a user who does not come from EPLAN or QET.
    for (n, settings) in [
        "version = 1\nfirst_run_done = true\ncomes_from = \"eplan\"\ntips = false\n",
        "version = 1\nfirst_run_done = true\ncomes_from = \"kicad\"\n",
        "version = 1\nfirst_run_done = true\n",
    ]
    .into_iter()
    .enumerate()
    {
        let tmp = Tmp::new(&format!("gt3-off-{n}"));
        let mut s = shell_with(&tmp, settings, "102");
        line_onto_a_cp(&mut s);
        refuse_a_derived_tag(&mut s);
        let (page, _) = generated_page(&s);
        refuse_a_generated_page(&mut s, &page);
        assert_eq!(tips(&s), [], "{settings}");
        assert_eq!(settings_of(&tmp).tips_seen, Vec::<String>::new());
        s.shutdown();
    }
    // On, for a user from EPLAN; in the window, where Renumber devices… opens its dialog.
    let tmp = Tmp::new("gt3-on");
    let config = tmp.0.join("config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("settings.toml"),
        "version = 1\nfirst_run_done = true\ncomes_from = \"eplan\"\n",
    )
    .unwrap();
    let mut s = tmp.shell_in(Mode::Gui, true, "102");
    refuse_a_derived_tag(&mut s);
    refuse_a_derived_tag(&mut s);
    assert_eq!(
        tips(&s),
        [(
            "Tags are derived from the structure and the counter.".to_owned(),
            Some(NoteAction::Renumber)
        )]
    );
    assert_eq!(
        ew_viewmodel::strings::text(NoteAction::Renumber.label_key()),
        "Renumber devices…"
    );
    assert_eq!(
        lines(&s, "toast "),
        ["toast \"Tags are derived from the structure and the counter.\" renumber"]
    );
    // Renumber devices… opens its dialog.
    let serial = s.toast().unwrap().serial;
    done(
        &mut s,
        "notifications.run",
        json!({"entry": serial, "action": "renumber"}),
    );
    assert!(s.dialog().is_some(), "Renumber devices… is open");
    done(&mut s, "ui.dismiss", json!({}));
    // Tip 3 on a generated page; its action selects the report block in Properties.
    let (page, block) = generated_page(&s);
    refuse_a_generated_page(&mut s, &page);
    refuse_a_generated_page(&mut s, &page);
    assert_eq!(
        tips(&s)[1..],
        [(
            "This page is generated from a report block.".to_owned(),
            Some(NoteAction::ShowBlock)
        )]
    );
    assert_eq!(
        ew_viewmodel::strings::text(NoteAction::ShowBlock.label_key()),
        "Show report block"
    );
    assert_eq!(
        lines(&s, "toast ").last().map(String::as_str),
        Some("toast \"This page is generated from a report block.\" show_block")
    );
    let serial = s.toast().unwrap().serial;
    done(
        &mut s,
        "notifications.run",
        json!({"entry": serial, "action": "show_block"}),
    );
    assert_eq!(
        s.selection()
            .items
            .iter()
            .map(|e| e.0.to_string())
            .collect::<Vec<_>>(),
        [block]
    );
    assert_eq!(s.focus(), Some(Focus::Properties));
    let sheet = s.properties().sheet().expect("the block's sheet");
    assert_eq!(sheet.sections[0].kind, "report_block");
    assert_eq!(settings_of(&tmp).tips_seen, ["derived_tag", "generated_page"]);
    // Tip 1 is still to come: it is the only one shown now.
    done(&mut s, "page.goto", json!({"folio": "102"}));
    line_onto_a_cp(&mut s);
    assert_eq!(tips(&s).len(), 3);
    assert_eq!(
        settings_of(&tmp).tips_seen,
        ["derived_tag", "generated_page", "line_on_cp"]
    );
    s.shutdown();
}

/// A read-only project's refusal offers neither tip 2 nor tip 3 and leaves both unused.
#[test]
fn gt3_a_read_only_refusal_leaves_tips_2_and_3_unused() {
    let tmp = Tmp::new("gt3-read-only");
    let config = tmp.0.join("config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("settings.toml"),
        "version = 1\nfirst_run_done = true\ncomes_from = \"eplan\"\n",
    )
    .unwrap();
    let mut s = tmp.shell_in(Mode::Headless, false, "102");
    done(
        &mut s,
        "select.set",
        json!({"refs": ["tag:-102F1", "tag:-102F2"]}),
    );
    let read_only = Outcome::Refused {
        reason_key: ew_viewmodel::key!("reason-read-only"),
    };
    assert_eq!(
        s.dispatch(
            "props.action",
            &json!({"row": "device/derived.tag", "action": "edit"})
        ),
        read_only
    );
    let (folio, _) = generated_page(&s);
    done(&mut s, "page.goto", json!({ "folio": folio }));
    done(&mut s, "select.set", json!({"ids": []}));
    let row = s.properties().sheet().expect("the page's sheet").sections[0].rows[0]
        .key
        .clone();
    assert_eq!(
        s.dispatch("props.action", &json!({"row": row, "action": "edit"})),
        read_only
    );
    assert_eq!(tips(&s), []);
    assert_eq!(settings_of(&tmp).tips_seen, Vec::<String>::new());
    s.shutdown();
}

/// An editing tool refused on a generated page, and the Pages navigator's reorder and move of one, offer tip
/// 3 (once).
#[test]
fn gt3_a_tool_a_reorder_or_a_move_refused_on_a_generated_page_offers_tip_3() {
    let generated = "This page is generated from a report block.".to_owned();
    // The Wire tool on a generated page.
    let tmp = Tmp::new("gt3-tool");
    let mut s = shell_with(
        &tmp,
        "version = 1\nfirst_run_done = true\ncomes_from = \"qet\"\n",
        "102",
    );
    let (folio, block) = generated_page(&s);
    done(&mut s, "page.goto", json!({ "folio": folio }));
    assert_eq!(
        s.dispatch("tool.wire", &json!({})),
        Outcome::Refused {
            reason_key: ew_viewmodel::key!("reason-page-type-no-symbols-report")
        }
    );
    assert_eq!(tips(&s), [(generated.clone(), Some(NoteAction::ShowBlock))]);
    let serial = s.toast().unwrap().serial;
    done(
        &mut s,
        "notifications.run",
        json!({"entry": serial, "action": "show_block"}),
    );
    assert_eq!(
        s.selection()
            .items
            .iter()
            .map(|e| e.0.to_string())
            .collect::<Vec<_>>(),
        [block]
    );
    s.shutdown();
    // A reorder naming the generated page.
    let tmp = Tmp::new("gt3-reorder");
    let mut s = shell_with(
        &tmp,
        "version = 1\nfirst_run_done = true\ncomes_from = \"qet\"\n",
        "102",
    );
    let failed = Outcome::Failed {
        message: "A generated page has no folio of its own.".to_owned(),
    };
    assert_eq!(
        s.dispatch("pages.reorder", &json!({"pages": [folio], "end": true})),
        failed
    );
    assert_eq!(tips(&s), [(generated.clone(), Some(NoteAction::ShowBlock))]);
    s.shutdown();
    // A move of the cursor row, the generated page on show.
    let tmp = Tmp::new("gt3-move");
    let mut s = shell_with(
        &tmp,
        "version = 1\nfirst_run_done = true\ncomes_from = \"qet\"\n",
        "102",
    );
    done(&mut s, "page.goto", json!({ "folio": folio }));
    assert_eq!(s.dispatch("pages.move_up", &json!({})), failed);
    assert_eq!(tips(&s), [(generated, Some(NoteAction::ShowBlock))]);
    s.shutdown();
}

/// C49-p6b#5, C49-p8#9: the six Dutch choice words are the words the window already uses for those values.
#[test]
fn the_six_dutch_choice_words_equal_their_siblings() {
    use ew_viewmodel::strings::{self, Key};
    assert!(strings::set_locale("nl"));
    for (choice, sibling, word) in [
        (
            "choice-equipotential",
            "bond-kind-equipotential",
            "Potentiaalvereffening",
        ),
        ("choice-end-clamp", "choice-end-bracket", "Eindsteun"),
        ("choice-dt", "form-tag", "Kenmerk"),
        ("choice-chain", "junction-wiring-chain", "Keten"),
        ("choice-bevel", "junction-style-bevel", "Schuin"),
        (
            "choice-style-default",
            "junction-wiring-default",
            "Standaard van de stijl",
        ),
    ] {
        assert_eq!(strings::text(Key::run_time(choice)), word, "{choice}");
        assert_eq!(strings::text(Key::run_time(sibling)), word, "{sibling}");
    }
    assert!(strings::set_locale("en"));
}

#[test]
fn gt3_table_mode_refusals_on_a_tag_and_on_a_generated_page_offer_tips_2_and_3() {
    let tmp = Tmp::new("gt3-table");
    let mut s = shell_with(
        &tmp,
        "version = 1\nfirst_run_done = true\ncomes_from = \"qet\"\n",
        "102",
    );
    done(&mut s, "navigator.show", json!({"kind": "devices"}));
    done(&mut s, "navigator.table", json!({"kind": "devices", "on": true}));
    let table = golden().navigator_table(NavKind::Devices);
    let row = table.nodes.iter().find(|n| n.table_row).unwrap().key.clone();
    let column = table.columns.iter().find(|c| c.key_column).unwrap().key.clone();
    let out = s.dispatch(
        "table.edit",
        &json!({"kind": "devices", "row": row, "column": column}),
    );
    assert!(matches!(out, Outcome::Failed { .. }), "{out:?}");
    assert_eq!(
        tips(&s),
        [(
            "Tags are derived from the structure and the counter.".to_owned(),
            Some(NoteAction::Renumber)
        )]
    );
    done(&mut s, "navigator.table", json!({"kind": "pages", "on": true}));
    let table = golden().navigator_table(NavKind::Pages);
    let row = table
        .nodes
        .iter()
        .find(|n| n.table_row && n.badges.iter().any(|b| b == "generated"))
        .unwrap()
        .key
        .clone();
    let out = s.dispatch(
        "table.edit",
        &json!({"kind": "pages", "row": row, "column": "title"}),
    );
    assert_eq!(
        out,
        Outcome::Failed {
            message: "Title is read-only here: generated.".to_owned()
        }
    );
    assert_eq!(
        tips(&s)[1..],
        [(
            "This page is generated from a report block.".to_owned(),
            Some(NoteAction::ShowBlock)
        )]
    );
    s.shutdown();
}

#[test]
fn gt4_wire_colours_stroke_w29_in_dbu_and_stripe_w27_and_are_off_by_default() {
    let tmp = Tmp::new("gt4-view");
    let mut s = tmp.shell("103");
    assert!(!s.wire_colours_on());
    assert_eq!(s.wire_colour_overlay(0), []);
    assert_eq!(lines(&s, "canvas wire "), Vec::<String>::new());
    done(&mut s, "view.wire_colours", json!({}));
    assert!(s.wire_colours_on());
    let strokes = s.wire_colour_overlay(0);
    let w29 = strokes
        .iter()
        .find(|x| x.id.to_string() == W29)
        .expect("W29 is drawn on 103");
    assert_eq!(w29.code, "DBU");
    // Golden A's colour profile declares DBU as BU, dark: the colour comes from that, not the spelling.
    assert_eq!(w29.colours, [Some((0x11, 0x34, 0x76))]);
    assert!(!w29.cased);
    assert_eq!(
        lines(&s, "canvas wire "),
        [format!("canvas wire colours {}", strokes.len())]
    );
    // W27, on the page that draws it: green and yellow 2-mm dashes, alternating from green.
    done(&mut s, "select.set", json!({"refs": [W27]}));
    let w27 = s.selection().items[0].0;
    let folios: Vec<String> = s.page_rows().iter().map(|r| r.folio.clone()).collect();
    let folio = folios
        .into_iter()
        .find(|f| {
            done(&mut s, "page.goto", json!({ "folio": f }));
            tables(&s).connections.iter().any(|c| c.id == w27)
        })
        .expect("a page draws W27");
    let strokes = s.wire_colour_overlay(0);
    let stripes = strokes.iter().find(|x| x.id == w27).expect("W27 is stroked");
    assert_eq!(stripes.code, "GNYE", "on folio {folio}");
    assert_eq!(stripes.colours, [iec_rgb("GN"), iec_rgb("YE")]);
    assert_eq!(stripes.colours.len(), 2);
    let pieces = stripes.pieces();
    assert!(pieces.len() > 1);
    for (i, (a, b, c, _)) in pieces.iter().enumerate() {
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        assert!(len <= DASH_NM + 1.0, "{len}");
        if i == 0 {
            assert_eq!(*c, 0, "green first");
        }
    }
    // Off again: no stroke.
    done(&mut s, "view.wire_colours", json!({"on": false}));
    assert_eq!(s.wire_colour_overlay(0), []);
}

/// A script's `step` near `i64::MAX` with the cursor past row 0 stops at the last row (no overflow).
#[test]
fn ft1347_a_huge_glossary_step_stops_at_the_last_row() {
    let tmp = Tmp::new("ft1347-glossary");
    let mut s = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut s, "help.glossary", json!({}));
    let n = s.glossary_rows().len();
    done(&mut s, "glossary.row", json!({"step": 2}));
    assert_eq!(s.glossary_row(), Some(1));
    done(&mut s, "glossary.row", json!({"step": i64::MAX}));
    assert_eq!(s.glossary_row(), Some(n - 1));
}
