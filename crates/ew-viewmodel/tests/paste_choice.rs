// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.07 part 4b through the text interface (PS8's shell half; D22, FT-396): two copies of Golden A, P1 holding
//! FX-MCB-1PN 1.0.0 on F55 and P2 holding 1.0.1 (a description change, a patch), each in its own shell over one user
//! library "Fixture parts". The library is written by the parts import into a library (`catalog::import_into_library`),
//! the second version by the same import bumped, so the viewmodel's tests name no schema type. F55 copied in P1 and
//! pasted in P2 opens the paste choice card (`reason-paste-versions`): Use the project's copy is one `edit.paste` with
//! `adapt` and one undo step, Update the project's copy is disabled. Pasted the other way, Update the project's copy
//! opens `library.update`'s card and its Apply arms the paste again: two commands, two undo steps.

mod common;

use std::path::{Path, PathBuf};

use common::{Tmp, at, done, refused};
use ew_viewmodel::ew_api::catalog::{self, ColumnMap, Limits};
use ew_viewmodel::ew_api::library::{LibraryTiers, Registry};
use ew_viewmodel::properties::CardOrigin;
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::json;

fn tiers(root: &Path) -> LibraryTiers {
    LibraryTiers {
        system: Vec::new(),
        community: root.join("data").join("packs"),
        company: Vec::new(),
        user: root.join("data").join("libraries"),
        cache: root.join("cache"),
    }
}

/// The fixture's parts table: FX-MCB-1PN by the manufacturer Fixture, with `description`.
fn table(root: &Path, description: &str) -> PathBuf {
    let path = root.join("fixture-parts.csv");
    std::fs::write(
        &path,
        format!("Maker,Order code,Text,Category\nFixture,FX-MCB-1PN,{description},device\n"),
    )
    .unwrap();
    path
}

fn map() -> ColumnMap {
    ColumnMap::new(
        None,
        [
            ("manufacturer", "Maker"),
            ("part_number", "Order code"),
            ("description", "Text"),
            ("kind", "Category"),
        ],
    )
    .unwrap()
}

/// Writes FX-MCB-1PN into the user library "Fixture parts" (created on the first call): `bump` writes a changed
/// description as the next patch version.
fn publish(t: &LibraryTiers, root: &Path, description: &str, bump: bool) {
    let mut reg = Registry::open(t).unwrap();
    let lib = match reg.libraries().into_iter().find(|l| l.key == "fixture") {
        Some(l) => l.id,
        None => reg
            .create(
                &t.user.join("fixture.ewlib"),
                "fixture",
                "Fixture parts",
                "CC0-1.0",
            )
            .unwrap(),
    };
    let input = catalog::read_input(&table(root, description), &Limits::default()).unwrap();
    catalog::import_into_library(
        &mut reg,
        lib,
        &input,
        &map(),
        "2026-10-31".parse().ok(),
        bump,
        false,
    )
    .unwrap();
}

/// A headless shell with the tiers `t` and a writable copy of Golden A named `copy`, on folio 101.
fn shell_with(tmp: &Tmp, copy: &str, t: &LibraryTiers) -> ShellModel {
    let dir = tmp.0.join(copy);
    common::copy_dir(Path::new(common::GOLDEN_A), &dir);
    let config = ShellConfig::in_dir(&tmp.0.join(format!("config-{copy}")), "0.0.0-test")
        .with_libraries(Some(t.clone()));
    let mut shell = ShellModel::new(Mode::Headless, config);
    done(
        &mut shell,
        "file.open",
        json!({"dir": dir.to_string_lossy(), "read_only": false}),
    );
    done(&mut shell, "page.goto", json!({"folio": "101"}));
    shell
}

/// A shell whose F55 uses FX-MCB-1PN as the library holds it now (its first use replaced through the part dialog).
fn holding(tmp: &Tmp, copy: &str, t: &LibraryTiers) -> ShellModel {
    let mut s = shell_with(tmp, copy, t);
    done(&mut s, "select.set", json!({"refs": ["tag:F55"]}));
    done(&mut s, "part.choose", json!({"use": 0}));
    done(&mut s, "part.pick", json!({"part": "FX-MCB-1PN (Fixture)"}));
    s
}

/// F55's representation on the page shown, selected; its page position.
fn select_f55(s: &mut ShellModel) -> (i64, i64) {
    done(s, "select.set", json!({"refs": ["tag:F55"]}));
    let dev = s.selection().items[0].0;
    let rep = s
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .find(|r| r.device == Some(dev))
        .expect("F55 is drawn on 101")
        .clone();
    done(s, "select.set", json!({"ids": [rep.id.to_string()]}));
    rep.pos
}

/// F55 copied in `from`: the clipboard text and F55's position.
fn copy_f55(from: &mut ShellModel) -> (String, (i64, i64)) {
    let pos = select_f55(from);
    done(from, "edit.copy", json!({}));
    (from.clipboard().text().unwrap().to_owned(), pos)
}

/// The QET grid step in nm (rounded): a sheet offset that moves the snapped anchor by one step.
const STEP: i64 = 2_645_833;

/// Places the armed paste tool six grid steps below `pos` and commits.
fn place(s: &mut ShellModel, pos: (i64, i64)) {
    assert_eq!(s.tool().name(), "paste");
    let p = s.canvas().unwrap().page().snapper.page_to_sheet(pos);
    let target = (p.0, p.1 + 6 * STEP);
    done(s, "canvas.pointer", at(target, json!({})));
    done(s, "canvas.press", at(target, json!({})));
}

fn card_lines(s: &ShellModel) -> Vec<String> {
    s.describe()
        .lines()
        .map(str::trim_start)
        .filter(|l| l.starts_with("card ") || l.starts_with("conflict ") || l.starts_with("choice "))
        .map(str::to_owned)
        .collect()
}

fn commands(s: &ShellModel) -> Vec<String> {
    s.recorded_commands().iter().map(|c| c.cmd.clone()).collect()
}

/// The held FX-MCB-1PN as a copy of F55 carries it (the `$lib` line of its part): its copy ID and version. Copies F55
/// (the clipboard and the selection change).
fn held_mcb(s: &mut ShellModel) -> (String, String) {
    let (text, _) = copy_f55(s);
    text.lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| v.get("$lib").cloned())
        .find(|d| d["part_number"] == "FX-MCB-1PN")
        .map(|d| {
            (
                d["id"].as_str().unwrap().to_owned(),
                d["version"].as_str().unwrap().to_owned(),
            )
        })
        .expect("F55's fragment carries FX-MCB-1PN")
}

fn held_version(s: &mut ShellModel) -> String {
    held_mcb(s).1
}

#[test]
fn ps8_the_paste_choice_in_the_shell() {
    let tmp = Tmp::new("paste-choice");
    let t = tiers(&tmp.0);
    publish(&t, &tmp.0, "MCB 1P+N C10", false);
    let mut p1 = holding(&tmp, "p1", &t);
    publish(&t, &tmp.0, "MCB 1P+N C10 new text", true);
    let mut p2 = holding(&tmp, "p2", &t);
    assert_eq!(held_version(&mut p1), "1.0.0");
    assert_eq!(held_version(&mut p2), "1.0.1");
    let copy = held_mcb(&mut p2).0;

    // P1 → P2: the choice card; the pasted version is older.
    let (text, pos) = copy_f55(&mut p1);
    let steps = commands(&p2).len();
    assert_eq!(
        p2.dispatch("edit.paste_clip", &json!({ "text": text })),
        Outcome::Refused {
            reason_key: "reason-paste-versions"
        }
    );
    assert_ne!(p2.tool().name(), "paste", "no paste tool before the choice");
    // Cycle 39 (FIX39b-1): the prompt is short, since the card holds the choice; the HUD shows the same text.
    let prompt = "Another version of FX-MCB-1PN (Fixture) is in the project; choose in the card below";
    assert_eq!(p2.status_fields(false)[0], prompt);
    assert_eq!(ew_viewmodel::command_bar::hud(&p2).prompt, prompt);
    let card = p2.properties().card().expect("the choice card");
    assert!(matches!(card.origin, CardOrigin::PasteVersions(_)));
    let lines = card_lines(&p2);
    assert!(lines[0].starts_with("card failed \"edit.paste\" "), "{lines:?}");
    assert!(lines[0].ends_with(" paste"), "{lines:?}");
    assert_eq!(
        lines[1..],
        [
            "conflict \"FX-MCB-1PN (Fixture)\" project \"1.0.1\" fragment \"1.0.0\" patch",
            "choice adapt enabled",
            "choice update disabled: \"The pasted version is not newer than the project's\"",
        ]
    );
    // Cycle 39 (FIX39b-1): the card's row says the change size in words; the describe line keeps the class word.
    let CardOrigin::PasteVersions(choice) = &p2.properties().card().unwrap().origin else {
        unreachable!()
    };
    assert_eq!(
        ew_viewmodel::properties::PasteChoice::conflict_text(&choice.conflicts[0]),
        "FX-MCB-1PN (Fixture): this project 1.0.1, pasted 1.0.0, a data change"
    );
    // Apply is not a choice; Update is disabled with its reason.
    assert_eq!(
        refused(&mut p2, "preview.apply", json!({})),
        "reason-paste-versions"
    );
    assert_eq!(
        refused(&mut p2, "paste.resolve", json!({"choice": "update"})),
        "reason-paste-update-older"
    );
    // Use the project's copy: one edit.paste with adapt, one undo step.
    assert!(p2.preview_shown(), "the choice card opens the Preview tab");
    done(&mut p2, "paste.resolve", json!({"choice": "adapt"}));
    assert!(p2.properties().card().is_none(), "the card closes");
    // Cycle 39 (FIX39b-1): the tab the card opened closes with it.
    assert!(!p2.preview_shown(), "the Preview tab closes with the card");
    place(&mut p2, pos);
    assert_eq!(commands(&p2).len(), steps + 1, "one command");
    let last = p2.recorded_commands().last().unwrap().clone();
    assert_eq!(last.cmd, "edit.paste");
    assert_eq!(last.params["adapt"], json!([copy]));
    assert_eq!(held_version(&mut p2), "1.0.1", "the project's copy stays");
    let reps = p2.canvas().unwrap().page().tables.reps.len();
    done(&mut p2, "edit.undo", json!({}));
    assert_eq!(
        p2.canvas().unwrap().page().tables.reps.len(),
        reps - 1,
        "one undo step removes the paste"
    );

    // P2 → P1: the pasted version is newer; Update the project's copy, then the paste.
    let (text, pos) = copy_f55(&mut p2);
    let steps = commands(&p1).len();
    assert_eq!(
        p1.dispatch("edit.paste_clip", &json!({ "text": text })),
        Outcome::Refused {
            reason_key: "reason-paste-versions"
        }
    );
    let lines = card_lines(&p1);
    assert_eq!(
        lines[1..],
        [
            "conflict \"FX-MCB-1PN (Fixture)\" project \"1.0.0\" fragment \"1.0.1\" patch",
            "choice adapt enabled",
            "choice update enabled",
        ]
    );
    done(&mut p1, "paste.resolve", json!({"choice": "update"}));
    let card = p1.properties().card().expect("the update card");
    assert!(matches!(card.origin, CardOrigin::PasteUpdate { .. }));
    assert_eq!(card.preview.name, "library.update");
    assert_eq!(commands(&p1).len(), steps, "nothing committed before Apply");
    // Cycle 39 (FIX39b-1): the HUD's prompt follows the card: the update card's own, not the choice's.
    assert_eq!(
        ew_viewmodel::command_bar::hud(&p1).prompt,
        "Apply the update in the card below; the paste follows"
    );
    done(&mut p1, "preview.apply", json!({}));
    assert_eq!(held_version(&mut p1), "1.0.1");
    assert!(
        !p1.preview_shown(),
        "the tab the choice card opened closes after the update card's Apply"
    );
    assert!(
        !ew_viewmodel::command_bar::hud(&p1).prompt.contains("card below"),
        "the paste tool's own prompt after Apply"
    );
    place(&mut p1, pos);
    assert_eq!(
        commands(&p1)[steps..],
        ["library.update", "edit.paste"],
        "two commands"
    );
    assert!(
        p1.recorded_commands()
            .last()
            .unwrap()
            .params
            .get("adapt")
            .is_none()
    );
    done(&mut p1, "edit.undo", json!({}));
    assert_eq!(held_version(&mut p1), "1.0.1", "the paste is its own undo step");
    done(&mut p1, "edit.undo", json!({}));
    assert_eq!(held_version(&mut p1), "1.0.0", "two undo steps");
    // Cycle 39 (FIX39b-1): Cancel on the update card closes the tab the choice card opened and ends its prompt.
    assert_eq!(
        p1.dispatch("edit.paste_clip", &json!({ "text": text })),
        Outcome::Refused {
            reason_key: "reason-paste-versions"
        }
    );
    done(&mut p1, "paste.resolve", json!({"choice": "update"}));
    assert!(p1.preview_shown());
    done(&mut p1, "preview.cancel", json!({}));
    assert!(p1.properties().card().is_none());
    assert!(!p1.preview_shown(), "Cancel closes the tab");
    assert!(!ew_viewmodel::command_bar::hud(&p1).prompt.contains("card below"));
    // Without a choice card, paste.resolve is refused.
    assert_eq!(
        refused(&mut p1, "paste.resolve", json!({"choice": "adapt"})),
        "reason-no-paste-choice"
    );
    p1.shutdown();
    p2.shutdown();
}
