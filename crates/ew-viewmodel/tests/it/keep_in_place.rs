// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Keep in place (doc 06a §2.4) through the shell on a temporary copy of the sample: the canvas context menu over a
//! selection of symbols offers it, keeps every selected symbol in one undo step and says so with their tags, shows its
//! check while they are kept, and lets them go again.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::command_bar::{MenuSpot, canvas_menu};
use ew_viewmodel::key;
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::json;

/// Whether each of `reps` is kept in place, as the page tables say.
fn kept(shell: &ShellModel, reps: &[ew_api::output::Uuid]) -> Vec<bool> {
    let page = shell.canvas().unwrap().page();
    reps.iter()
        .map(|id| page.tables.reps.iter().find(|r| r.id == *id).unwrap().locked)
        .collect()
}

/// The menu's Keep in place entry: whether it is enabled and checked.
fn entry(shell: &ShellModel) -> (bool, bool) {
    let item = canvas_menu(shell, MenuSpot::Object, None)
        .into_iter()
        .find(|i| i.id == "rep.keep_in_place")
        .expect("Keep in place over a symbol");
    assert_eq!(item.label, "Keep in place");
    (item.enabled, item.checked)
}

#[test]
fn keep_in_place_keeps_the_selected_symbols_in_one_step_and_lets_them_go() {
    let tmp = Tmp::new("keep-in-place");
    let mut shell = tmp.sample_in(Mode::Headless, true, "101");
    // Two symbols of two devices that are no terminals, with their tags as their connection points print them.
    let page = shell.canvas().unwrap().page().clone();
    let mut reps = Vec::new();
    for r in page
        .tables
        .reps
        .iter()
        .filter(|r| r.device.is_some() && !r.terminal)
    {
        let Some(cp) = page.tables.cps.iter().find(|c| r.functions.contains(&c.function)) else {
            continue;
        };
        let tag = cp.label.split(':').next().unwrap().to_owned();
        if reps.iter().all(|(_, d, _)| *d != r.device) {
            reps.push((r.id, r.device, tag));
        }
        if reps.len() == 2 {
            break;
        }
    }
    let ids: Vec<_> = reps.iter().map(|(id, _, _)| *id).collect();
    let tags: Vec<String> = reps.iter().map(|(_, _, t)| t.clone()).collect();
    assert_eq!(kept(&shell, &ids), [false, false], "The sample keeps nothing");
    let selected: Vec<String> = ids.iter().map(ToString::to_string).collect();
    done(&mut shell, "select.set", json!({ "ids": selected }));
    assert_eq!(entry(&shell), (true, false));

    done(&mut shell, "rep.keep_in_place", json!({}));
    assert_eq!(kept(&shell, &ids), [true, true]);
    assert_eq!(entry(&shell), (true, true), "checked while both are kept");
    let toast = shell.toast().map(|t| t.text.clone()).unwrap_or_default();
    assert!(toast.starts_with("Kept in place: "), "{toast}");
    for tag in &tags {
        assert!(toast.contains(tag.as_str()), "{toast} names {tag}");
    }

    // One undo step takes both back.
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(kept(&shell, &ids), [false, false]);
    done(&mut shell, "edit.redo", json!({}));
    assert_eq!(kept(&shell, &ids), [true, true]);

    // With both kept it lets them go.
    done(&mut shell, "select.set", json!({ "ids": selected }));
    done(&mut shell, "rep.keep_in_place", json!({}));
    assert_eq!(kept(&shell, &ids), [false, false]);
    let toast = shell.toast().map(|t| t.text.clone()).unwrap_or_default();
    assert!(toast.starts_with("No longer kept in place: "), "{toast}");
}

/// Without a symbol in the selection the entry is not there to run: the action refuses in words.
#[test]
fn keep_in_place_without_a_symbol_is_refused() {
    let tmp = Tmp::new("keep-in-place-none");
    let mut shell = tmp.sample_in(Mode::Headless, true, "101");
    assert_eq!(
        shell.dispatch("rep.keep_in_place", &json!({})),
        Outcome::Refused {
            reason_key: key!("reason-keep-in-place-no-symbol")
        }
    );
}
