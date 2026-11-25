// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page copy through the actions a person and a script use: Page ▸ Copy pages… (`pages.copy`) on a small project of its
//! own (a relay on 11 whose contact is on 12, a strip with terminals on both pages, a cable on 12), its form and its
//! card; Save as macro… of pages named by folio with nothing selected; Edit ▸ Paste in place; and a paste whose block
//! holds a text in the frame's header band, where the Text tool puts one.

use std::collections::BTreeSet;

use crate::common;

use common::{Tmp, done, grid_point, user_library};
use ew_api::output::{NavRole, Uuid};
use ew_viewmodel::key;
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::properties::CardOrigin;
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellConfig, ShellModel};
use ew_viewmodel::strings;
use serde_json::json;

const COIL: &str = "ew.iec.coil@qet10";
const CONTACT: &str = "ew.iec.contact-no@qet10";
const TERMINAL: &str = "ew.iec.terminal@qet10";

/// Places `pick` with its anchor at grid index `k` of the page on show, without autoconnect.
fn place(s: &mut ShellModel, pick: serde_json::Value, k: (i64, i64)) {
    done(s, "place.pick", pick);
    let p = grid_point(s, k);
    common::cursor(s, p);
    done(s, "canvas.activate", json!({"alt": true}));
    done(s, "tool.select", json!({}));
}

/// The page point of the connection point labelled `label` on the page on show.
fn cp(s: &ShellModel, label: &str) -> (i64, i64) {
    let t = common::tables(s);
    t.cps
        .iter()
        .find(|c| c.label == label)
        .unwrap_or_else(|| {
            let all: Vec<&str> = t.cps.iter().map(|c| c.label.as_str()).collect();
            panic!("no connection point {label} among {all:?}")
        })
        .at
}

/// A wire from the connection point `a` to `b` on the page on show, drawn with the Wire tool.
fn wire(s: &mut ShellModel, a: &str, b: &str) {
    let (a, b) = (cp(s, a), cp(s, b));
    done(s, "tool.wire", json!({}));
    common::cursor(s, a);
    done(s, "canvas.activate", json!({}));
    common::cursor(s, b);
    done(s, "canvas.activate", json!({}));
    done(s, "tool.select", json!({}));
}

/// The tag of the strip of the terminals the page on show draws (`-X111`).
fn strip_tag(s: &ShellModel) -> String {
    let t = common::tables(s);
    let terminal = t.reps.iter().find(|r| r.terminal).expect("a terminal").id;
    let label = &t.cps.iter().find(|c| c.rep == terminal).expect("its point").label;
    label.split(':').next().unwrap_or_default().to_owned()
}

/// The small project: on 11 a relay between the terminals 1 and 2 of a new strip; on 12 the relay's contact between the
/// strip's terminals 3 and 4, whose two wires are the cores of the cable -W1.
fn small_project(tmp: &Tmp) -> ShellModel {
    let mut s = tmp.empty_shell(Mode::Headless);
    crate::common::make_project(&mut s, &tmp.0.join("small.ew"), &[]);
    for (folio, title) in [("11", "Control"), ("12", "Outputs")] {
        done(
            &mut s,
            "page.new",
            json!({"folio": folio, "type": "multi_line", "title": title}),
        );
    }
    done(&mut s, "page.goto", json!({"folio": "11"}));
    place(&mut s, json!({"symbol": COIL, "letter": "K"}), (40, 30));
    // A new strip's tag takes its page and counter with its first terminal.
    done(&mut s, "strip.new", json!({"letter": "X"}));
    place(
        &mut s,
        json!({"symbol": TERMINAL, "strip": "-?X?", "terminal_designation": "1"}),
        (40, 15),
    );
    let x = strip_tag(&s);
    place(
        &mut s,
        json!({"symbol": TERMINAL, "strip": x, "terminal_designation": "2"}),
        (40, 45),
    );
    wire(&mut s, &format!("{x}:1/b"), "-11K1:A1");
    wire(&mut s, "-11K1:A2", &format!("{x}:2/a"));
    done(&mut s, "page.goto", json!({"folio": "12"}));
    place(&mut s, json!({"symbol": CONTACT, "device": "-11K1"}), (40, 30));
    for (d, k) in [("3", (40, 15)), ("4", (40, 45))] {
        place(
            &mut s,
            json!({"symbol": TERMINAL, "strip": x, "terminal_designation": d}),
            k,
        );
    }
    wire(&mut s, &format!("{x}:3/b"), "-11K1:13");
    wire(&mut s, "-11K1:14", &format!("{x}:4/a"));
    let wires: Vec<String> = common::tables(&s)
        .connections
        .iter()
        .map(|c| c.id.to_string())
        .collect();
    done(&mut s, "select.set", json!({ "ids": wires }));
    done(
        &mut s,
        "cable.from_wires",
        json!({"tag": "-W1", "type": "2x1.5", "cores": "BN BU"}),
    );
    s
}

/// The connection points' labels of page `folio` (shown for it).
fn labels(s: &mut ShellModel, folio: &str) -> BTreeSet<String> {
    done(s, "page.goto", json!({ "folio": folio }));
    common::tables(s).cps.iter().map(|c| c.label.clone()).collect()
}

/// The labels of an original page with the original folio `from` made `to` (`-11K1:13` on 22 is `-21K1:13`).
fn moved(labels: &BTreeSet<String>, from: &str, to: &str) -> BTreeSet<String> {
    labels.iter().map(|l| l.replacen(from, to, 1)).collect()
}

/// The device of the representation of page `folio` whose symbol is `symbol`.
fn device_of(s: &mut ShellModel, folio: &str, symbol: &str) -> Uuid {
    done(s, "page.goto", json!({ "folio": folio }));
    common::tables(s)
        .reps
        .iter()
        .find(|r| r.symbol_key == symbol)
        .and_then(|r| r.device)
        .unwrap_or_else(|| panic!("{symbol} on {folio}"))
}

/// The cables of the Cables navigator: (tag, its cores in row order).
fn cables(s: &mut ShellModel) -> Vec<(String, Vec<String>)> {
    done(s, "navigator.show", json!({"kind": "cables"}));
    let t = s.nav(NavKind::Cables).and_then(|n| n.table()).unwrap().clone();
    t.nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.role == NavRole::Cable)
        .map(|(i, n)| {
            let cores = t
                .nodes
                .iter()
                .filter(|c| c.role == NavRole::Conductor && c.parent == Some(i as u32))
                .map(|c| c.label.clone())
                .collect();
            (n.label.clone(), cores)
        })
        .collect()
}

/// The rule IDs of the findings of the Messages table after a check.
fn rules(s: &mut ShellModel) -> BTreeSet<String> {
    done(s, "navigator.show", json!({"kind": "messages"}));
    done(s, "messages.check", json!({}));
    let t = s
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .cloned()
        .expect("the Messages table");
    t.nodes
        .iter()
        .filter(|n| n.role == NavRole::Finding)
        .map(|n| n.cells[1].clone())
        .collect()
}

/// A script copies 11 and 12 with a folio offset of 10: 21 and 22 draw the copies at their places with the tags of
/// their new pages; the relay's coil on 21 and its contact on 22 are one new device; the strip's four terminals are
/// one new strip's over both copies; the two wires on 22 are cores of a new cable with the original's cores; no rule
/// finds on the copy what it does not find on the original; one undo step takes it away.
#[test]
fn a_script_copies_a_relay_a_strip_over_two_pages_and_a_cable_together() {
    let tmp = Tmp::new("ft1862-script");
    let mut s = small_project(&tmp);
    let (on11, on12) = (labels(&mut s, "11"), labels(&mut s, "12"));
    let relay = device_of(&mut s, "11", COIL);
    assert_eq!(
        device_of(&mut s, "12", CONTACT),
        relay,
        "the fixture: the contact is the relay's"
    );
    let before_cables = cables(&mut s);
    let before = rules(&mut s);
    done(&mut s, "pages.copy", json!({"pages": ["11", "12"], "offset": 10}));
    let card = s.properties().card().expect("the copy's card");
    assert!(matches!(card.origin, CardOrigin::PageCopy { .. }));
    assert_eq!(s.preview_summary().as_deref(), Some("Copy 2 pages: 11, 12"));
    done(&mut s, "preview.apply", json!({}));
    assert_eq!(common::toast(&s), "Copied 2 pages: 21, 22.");
    assert_eq!(labels(&mut s, "21"), moved(&on11, "11", "21"));
    assert_eq!(labels(&mut s, "22"), moved(&on12, "11", "21"));
    let copy = device_of(&mut s, "21", COIL);
    assert_ne!(copy, relay, "a new relay");
    assert_eq!(
        device_of(&mut s, "22", CONTACT),
        copy,
        "the copy's contact is the copy's relay's"
    );
    let core_kinds: Vec<String> = common::tables(&s)
        .connections
        .iter()
        .map(|c| c.kind.clone())
        .collect();
    assert_eq!(core_kinds, ["core", "core"], "the copy's wires on 22 are cores");
    let after_cables = cables(&mut s);
    let (_, cores) = &before_cables[0];
    assert!(
        after_cables.contains(&("-22W1".to_owned(), cores.clone())),
        "a new cable with the original's cores: {after_cables:?}"
    );
    let after = rules(&mut s);
    assert!(
        after.is_subset(&before),
        "new kinds of finding: {:?}",
        after.difference(&before).collect::<Vec<_>>()
    );
    done(&mut s, "edit.undo", json!({}));
    assert!(
        s.page_rows().iter().all(|r| r.folio != "21"),
        "one undo step takes the copy away"
    );
}

/// In the window Copy pages… on the page under the cursor opens its form with that page as After; submitted with an
/// offset it opens the card, which names the relay and the strip drawn also on 11, which is not copied; Apply copies
/// 12 as 32, whose relay and strip are new and take the tag of 32.
#[test]
fn the_form_takes_the_rule_and_the_card_names_what_stays_on_other_pages() {
    let tmp = Tmp::new("ft1862-form");
    let mut small = small_project(&tmp);
    let (on12, x) = (labels(&mut small, "12"), {
        done(&mut small, "page.goto", json!({"folio": "11"}));
        strip_tag(&small)
    });
    done(&mut small, "file.save", json!({}));
    small.shutdown();
    let mut s = ShellModel::new(
        Mode::Gui,
        ShellConfig::in_dir(&tmp.0.join("config-gui"), "0.0.0-test"),
    );
    done(
        &mut s,
        "file.open",
        json!({"dir": tmp.0.join("small.ew").to_string_lossy(), "read_only": false}),
    );
    done(&mut s, "page.goto", json!({"folio": "12"}));
    done(&mut s, "pages.copy", json!({}));
    let Some(Dialog::Form(f)) = s.dialog() else {
        panic!("the Copy pages form: {:?}", s.dialog());
    };
    assert_eq!(f.action, "pages.copy");
    let field = |n: &str| f.fields.iter().find(|x| x.name == n).map(|x| x.value.clone());
    assert_eq!(field("after").as_deref(), Some("12"));
    s.set_form_field("offset", "20");
    assert_eq!(s.submit_form(), Outcome::Done);
    assert!(s.dialog().is_none(), "the form closes on its card");
    let notices = s.preview_pane().notices().join("\n");
    assert!(
        notices.contains("-11K1") && notices.contains(&x),
        "the card names the relay and the strip: {notices}"
    );
    done(&mut s, "preview.apply", json!({}));
    assert_eq!(labels(&mut s, "32"), moved(&on12, "11", "32"));
    s.shutdown();
}

/// Save as macro… with pages named by folio and nothing selected or marked saves a page macro (it said "Nothing is
/// selected").
#[test]
fn save_as_macro_of_pages_by_folio_needs_no_selection() {
    let tmp = Tmp::new("ft1862-macro");
    let (t, _) = user_library(&tmp.0);
    let dir = tmp.sample_copy();
    common::copy_dir(std::path::Path::new(common::SAMPLE), &dir);
    let mut s = ShellModel::new(Mode::Gui, tmp.config().with_libraries(Some(t)));
    done(
        &mut s,
        "file.open",
        json!({"dir": dir.to_string_lossy(), "read_only": false}),
    );
    done(&mut s, "page.goto", json!({"folio": "101"}));
    done(&mut s, "select.set", json!({ "ids": [] }));
    assert!(s.navigator().marked().is_empty(), "no page marked");
    assert_eq!(
        s.dispatch(
            "macros.save",
            &json!({"name": "Supply page", "library": "Mine", "pages": ["101"], "confirm": true})
        ),
        Outcome::Done
    );
    s.shutdown();
}

/// Paste in place puts the block where it was copied: -102F1 copied on 102 and pasted in place on a new page of the
/// same frame stands where it stands on 102.
#[test]
fn paste_in_place_puts_the_block_where_it_was_copied() {
    let tmp = Tmp::new("ft1862-in-place");
    let mut s = tmp.sample_in(Mode::Gui, true, "102");
    let breaker = |s: &ShellModel| {
        let t = common::tables(s);
        let f1 = t
            .cps
            .iter()
            .find(|c| c.label == "-102F1:1")
            .expect("-102F1:1")
            .rep;
        t.reps.iter().find(|r| r.id == f1).map(|r| (r.id, r.pos)).unwrap()
    };
    let (rep, pos) = breaker(&s);
    done(&mut s, "select.set", json!({ "ids": [rep.to_string()] }));
    done(&mut s, "edit.copy", json!({}));
    done(
        &mut s,
        "page.new",
        json!({"folio": "106", "type": "multi_line", "title": "Copy"}),
    );
    done(&mut s, "page.goto", json!({"folio": "106"}));
    let rev = s.revision();
    done(&mut s, "edit.paste_in_place", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    assert_eq!(s.tool().name(), "select");
    let pasted: Vec<(i64, i64)> = common::tables(&s).reps.iter().map(|r| r.pos).collect();
    assert_eq!(pasted, [pos], "the copy stands where -102F1 stands on 102");
    s.shutdown();
}

/// A block whose free text lies in the frame's header band, where the Text tool put it, is pasted; a symbol put down
/// outside the drawing area is moved in to stand inside it.
#[test]
fn a_text_in_the_header_band_does_not_refuse_the_paste() {
    let tmp = Tmp::new("ft1862-band");
    let mut s = tmp.sample_in(Mode::Gui, true, "102");
    let area = ew_viewmodel::tools::ip::drawing_area(s.canvas().unwrap().page()).expect("a frame");
    let rep = {
        let t = common::tables(&s);
        t.cps
            .iter()
            .find(|c| c.label == "-102F1:1")
            .expect("-102F1:1")
            .rep
    };
    // The Text tool, half-way up the header band over the breaker.
    let at = (cp(&s, "-102F1:1").0, area[1] / 2);
    done(&mut s, "tool.text", json!({}));
    common::cursor(&mut s, at);
    done(&mut s, "canvas.activate", json!({}));
    done(&mut s, "text.input", json!({"text": "Band"}));
    done(&mut s, "canvas.activate", json!({}));
    done(&mut s, "tool.select", json!({}));
    let text = common::tables(&s)
        .items
        .iter()
        .find(|i| i.kind == "free_text" && i.bounds[1] < area[1])
        .map(|i| i.id)
        .expect("the text in the band");
    done(
        &mut s,
        "select.set",
        json!({ "ids": [rep.to_string(), text.to_string()] }),
    );
    done(&mut s, "edit.copy", json!({}));
    done(
        &mut s,
        "page.new",
        json!({"folio": "106", "type": "multi_line", "title": "Copy"}),
    );
    done(&mut s, "page.goto", json!({"folio": "106"}));
    assert_eq!(
        s.dispatch("edit.paste_in_place", &json!({})),
        Outcome::Done,
        "the text may stay in the band"
    );
    // The breaker put down at the sheet's corner, outside the drawing area, is moved in to stand inside it.
    done(&mut s, "page.goto", json!({"folio": "106"}));
    let before: Vec<_> = common::tables(&s).reps.iter().map(|r| r.id).collect();
    done(&mut s, "page.goto", json!({"folio": "102"}));
    done(&mut s, "select.set", json!({ "ids": [rep.to_string()] }));
    done(&mut s, "edit.copy", json!({}));
    done(&mut s, "page.goto", json!({"folio": "106"}));
    done(&mut s, "edit.paste_clip", json!({}));
    let corner = common::sheet(&s, (0, 0));
    done(&mut s, "canvas.pointer", common::at(corner, json!({})));
    assert_eq!(
        s.dispatch("canvas.press", &common::at(corner, json!({}))),
        Outcome::Done
    );
    let t = common::tables(&s);
    let new = t
        .reps
        .iter()
        .find(|r| !before.contains(&r.id))
        .expect("the pasted breaker");
    let b = new.body;
    assert!(
        area[0] < b[0].min(b[2]) && area[1] < b[1].min(b[3]),
        "inside the drawing area: {b:?} in {area:?}"
    );
    s.shutdown();
}

/// Writes `text` with the Text tool at grid index `k` of the page on show.
fn write(s: &mut ShellModel, text: &str, k: (i64, i64)) {
    done(s, "tool.text", json!({}));
    let p = grid_point(s, k);
    common::cursor(s, p);
    done(s, "canvas.activate", json!({}));
    done(s, "text.input", json!({ "text": text }));
    done(s, "canvas.activate", json!({}));
    done(s, "tool.select", json!({}));
}

/// The printed free texts of page `folio`, sorted.
fn texts(s: &mut ShellModel, folio: &str) -> Vec<String> {
    done(s, "page.goto", json!({ "folio": folio }));
    let mut out: Vec<String> = common::tables(s)
        .items
        .iter()
        .filter(|i| i.kind == "free_text")
        .filter_map(|i| i.text.clone())
        .collect();
    out.sort();
    out
}

/// D: free texts on 12 that name the strip by its tag ("-11X1") and the cable without its sign (its frozen "W1")
/// print the copy's names on 22: the strip's "-21X1", and the cable's tag on 22 without its sign (the copy's frozen
/// tag would print its original's, so it is cleared); a note that only holds the strip's tag is left as it is; the
/// card counts the two texts.
#[test]
fn free_texts_that_name_a_copied_strip_or_cable_take_the_copys_names() {
    let tmp = Tmp::new("ft1862-texts");
    let mut s = small_project(&tmp);
    done(&mut s, "page.goto", json!({"folio": "11"}));
    let x = strip_tag(&s);
    assert_eq!(x, "-11X1", "the fixture's strip");
    let cables_before = cables(&mut s);
    let w = cables_before[0].0.trim_start_matches('-').to_owned();
    done(&mut s, "page.goto", json!({"folio": "12"}));
    let note = format!("supply from {x}");
    for (text, k) in [
        (x.as_str(), (70, 15)),
        (w.as_str(), (70, 25)),
        (note.as_str(), (70, 35)),
    ] {
        write(&mut s, text, k);
    }
    let before = texts(&mut s, "12");
    done(&mut s, "pages.copy", json!({"pages": ["11", "12"], "offset": 10}));
    let notices = s.preview_pane().notices().join("\n");
    assert!(
        notices.contains("2 texts that name a copied device or interruption point"),
        "{notices}"
    );
    done(&mut s, "preview.apply", json!({}));
    let copy_cable = cables(&mut s)
        .into_iter()
        .map(|(tag, _)| tag)
        .find(|tag| !cables_before.iter().any(|(old, _)| old == tag))
        .expect("the copy's cable");
    assert_eq!(copy_cable, "-22W1", "the copy's cable takes the tag of 22");
    let mut expected = vec!["-21X1".to_owned(), "22W1".to_owned(), note.clone()];
    expected.sort();
    assert_eq!(texts(&mut s, "22"), expected);
    assert_eq!(texts(&mut s, "12"), before, "the originals print as before");
}

/// The form's refusals name its fields, never the script's parameters: an offset that is no whole number, a node
/// chosen without the copy's code, and a code typed without a node.
#[test]
fn the_forms_refusals_name_its_fields() {
    let tmp = Tmp::new("ft1862-form-words");
    let mut s = tmp.sample_in(Mode::Gui, true, "102");
    done(&mut s, "pages.copy", json!({}));
    let error = |s: &mut ShellModel| {
        assert_ne!(s.submit_form(), Outcome::Done);
        common::form(s).error.unwrap_or_default()
    };
    s.set_form_field("offset", "ten");
    assert_eq!(
        error(&mut s),
        strings::format(key!("error-page-copy-offset"), &["ten"])
    );
    s.set_form_field("offset", "");
    s.set_form_field("code", "CAB2");
    assert_eq!(error(&mut s), strings::text(key!("error-page-copy-node")));
    let node = common::form(&s)
        .fields
        .iter()
        .find(|f| f.name == "node")
        .and_then(|f| f.options.iter().find(|(id, _)| !id.is_empty()))
        .map(|(id, _)| id.clone())
        .expect("a structure node of the sample");
    s.set_form_field("node", &node);
    s.set_form_field("code", "");
    assert_eq!(error(&mut s), strings::text(key!("error-page-copy-code")));
    s.shutdown();
}

/// The form takes a folio offset or a folio to go after, never both: typing into one clears the other (a persona run
/// typed an offset beside the prefilled "after" and nothing said which won).
#[test]
fn the_offset_and_the_folio_after_clear_each_other() {
    let tmp = Tmp::new("copy-offset-or-after");
    let mut s = tmp.sample_in(Mode::Gui, true, "102");
    done(&mut s, "pages.copy", json!({}));
    let value = |s: &ShellModel, name: &str| {
        common::form(s)
            .fields
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.value.clone())
            .unwrap_or_default()
    };
    assert_eq!(value(&s, "after"), "102", "prefilled");
    s.set_form_field("offset", "10");
    assert_eq!(value(&s, "after"), "");
    s.set_form_field("after", "103");
    assert_eq!(value(&s, "offset"), "");
    assert_eq!(value(&s, "after"), "103");
    s.shutdown();
}

/// A refusal of the copy names it: "Copy pages: …".
#[test]
fn a_refused_copy_says_so() {
    let tmp = Tmp::new("ft1862-refused");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    let message = common::failed(
        &mut s,
        "pages.copy",
        json!({"pages": ["101", "102", "103"], "offset": 10}),
    );
    assert!(message.starts_with("Copy pages: "), "{message}");
}
