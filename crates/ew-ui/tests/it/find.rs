// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The find pane in the window without rendering (WP-S2.05 part 2, acceptance checks FR3 in kittest and the window
//! half of FR6's exact counts): Ctrl+H opens find and replace with the text field focused, typing makes no search,
//! Enter runs it once, the 20 results are a named list box, Ctrl+Enter previews the replace in the card and a second
//! Ctrl+Enter applies it (one undo step), Ctrl+Z undoes it; Escape closes the card, then gives the focus back to the
//! canvas; the accessible-name walker after every step. Keyboard only.

use crate::support;

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::{Focus, Outcome};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

fn on_golden(tmp: &Tmp, folio: &str) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({ "folio": folio }));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    h
}

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action}"
    );
}

fn results(h: &Harness<'_, App>) -> usize {
    h.state()
        .shell()
        .find_state()
        .results()
        .map_or(0, |r| r.hits.len())
}

#[test]
fn fr3_find_and_replace_from_the_keyboard() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "204");
    // Ctrl+H: find and replace below the page view, the focus in the pane and its text field.
    h.key_press_modifiers(Modifiers::COMMAND, Key::H);
    settle(&mut h);
    assert!(h.state().shell().find_shown());
    assert_eq!(h.state().shell().focus(), Some(Focus::Find));
    assert!(h.state().shell().find_state().replace_mode());
    assert!(
        h.get_all_by_role(egui::Role::Group)
            .any(|n| n.accesskit_node().label().as_deref() == Some("Find and replace")),
        "the pane is a named group"
    );
    // The scope through the pane's own actions (its checkboxes and filter field are named; see the walker).
    dispatch(&mut h, "find.fields", json!({"fields": ["function_text"]}));
    dispatch(&mut h, "find.folios", json!({"folios": ["204", "205", "206"]}));
    settle(&mut h);
    // Typing makes no search.
    let calls = h.state().shell().session_calls();
    let field = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Find"))
        .expect("the Find field");
    field.type_text("Spare");
    settle(&mut h);
    assert_eq!(h.state().shell().find_state().text(), "Spare");
    assert_eq!(
        h.state().shell().session_calls().find,
        calls.find,
        "typing makes no call"
    );
    // Tab to the replacement, type it, Enter runs the search: one call.
    h.key_press(Key::Tab);
    settle(&mut h);
    let field = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Replace with"))
        .expect("the replacement field");
    field.type_text("Reserve");
    settle(&mut h);
    assert_eq!(h.state().shell().find_state().replacement(), "Reserve");
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(
        h.state().shell().session_calls().find,
        calls.find + 1,
        "Enter runs it once"
    );
    assert_eq!(results(&h), 20);
    let options: Vec<String> = h
        .get_all_by_role(egui::Role::ListBoxOption)
        .filter_map(|n| n.accesskit_node().label())
        .filter(|l| l.contains("Function text"))
        .collect();
    // FT-433: the list box holds the 20; only the rows on show are built.
    assert!(
        h.get_all_by_role(egui::Role::ListBox)
            .any(|n| n.accesskit_node().label().as_deref() == Some("20 results")),
        "the list box of the 20 results"
    );
    assert!((1..=20).contains(&options.len()), "{options:?}");
    assert!(
        options
            .iter()
            .all(|o| o.contains("Spare -> Reserve") && !o.contains("[[")),
        "the names are plain text (FT-433): {options:?}"
    );
    // Down moves the cursor (no call).
    h.key_press(Key::ArrowDown);
    settle(&mut h);
    assert_eq!(h.state().shell().find_state().row(), Some(1));
    // Ctrl+Enter: the card; Ctrl+Enter again: Apply, one undo step.
    let steps = h.state().shell().recorded_commands().len();
    h.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    settle(&mut h);
    let card = h.state().shell().properties().card().expect("the replace card");
    assert_eq!(card.changes, 20);
    assert!(
        h.get_all_by_role(egui::Role::Button)
            .any(|n| n.accesskit_node().label().as_deref() == Some("Apply")),
        "the card's Apply"
    );
    h.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    settle(&mut h);
    assert!(h.state().shell().properties().card().is_none());
    assert_eq!(h.state().shell().recorded_commands().len(), steps + 1);
    assert_eq!(
        h.state().shell().find_state().note(),
        Some("Replaced 20; run again to refresh")
    );
    // Ctrl+Z undoes the replace.
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    settle(&mut h);
    dispatch(&mut h, "find.run", json!({}));
    settle(&mut h);
    assert_eq!(results(&h), 20, "undo restores all 20");
    // Escape: the card first, then the focus back to the canvas.
    h.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    settle(&mut h);
    assert!(h.state().shell().properties().card().is_some());
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(h.state().shell().properties().card().is_none());
    assert_eq!(h.state().shell().focus(), Some(Focus::Find));
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(matches!(h.state().shell().focus(), Some(Focus::Canvas(_))));
}

/// Ctrl+Enter with a text field focused does what it does with none: the first builds the replace card, the second
/// applies it (it must not rebuild the card).
#[test]
fn fr3_ctrl_enter_in_a_focused_field_builds_then_applies() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "204");
    h.key_press_modifiers(Modifiers::COMMAND, Key::H);
    settle(&mut h);
    dispatch(&mut h, "find.fields", json!({"fields": ["function_text"]}));
    dispatch(&mut h, "find.folios", json!({"folios": ["204", "205", "206"]}));
    dispatch(&mut h, "find.replacement", json!({"text": "Reserve"}));
    settle(&mut h);
    let field = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Find"))
        .expect("the Find field");
    field.type_text("Spare");
    settle(&mut h);
    assert!(h.ctx.text_edit_focused(), "the field keeps the focus");
    let steps = h.state().shell().recorded_commands().len();
    h.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    settle(&mut h);
    let card = h
        .state()
        .shell()
        .properties()
        .card()
        .expect("the first builds the card");
    assert_eq!(card.changes, 20);
    assert_eq!(
        h.state().shell().recorded_commands().len(),
        steps,
        "nothing applied yet"
    );
    let calls = h.state().shell().session_calls();
    h.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    settle(&mut h);
    assert!(
        h.state().shell().properties().card().is_none(),
        "the second applies it"
    );
    assert_eq!(h.state().shell().recorded_commands().len(), steps + 1);
    assert_eq!(
        h.state().shell().session_calls().find,
        calls.find,
        "applying a fresh card runs no search"
    );
}

/// Types `text` as keyboard input into whatever has the focus (no accessibility action that moves the focus).
fn type_keys(h: &mut Harness<'_, App>, text: &str) {
    h.event(egui::Event::Text(text.to_owned()));
    settle(h);
}

/// Tab in the search field moves the focus to "Replace with" and Shift+Tab back (keyboard parity, WP-S2.05 part 2).
#[test]
fn fr3_tab_and_shift_tab_move_between_the_find_fields() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "204");
    h.key_press_modifiers(Modifiers::COMMAND, Key::H);
    settle(&mut h);
    type_keys(&mut h, "Spare");
    h.key_press(Key::Tab);
    settle(&mut h);
    type_keys(&mut h, "Reserve");
    let f = h.state().shell().find_state();
    assert_eq!(f.text(), "Spare");
    assert_eq!(f.replacement(), "Reserve");
    h.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
    settle(&mut h);
    type_keys(&mut h, "X");
    let f = h.state().shell().find_state();
    assert_eq!(f.text(), "SpareX");
    assert_eq!(f.replacement(), "Reserve");
}

/// A click into the search field while the shell's focus is on the canvas hands the focus to the pane, so Tab moves
/// to "Replace with" and does not reach the canvas's `tool.next_proposal` (a chord of the canvas context).
#[test]
fn fr3_tab_after_a_click_into_the_field_stays_in_the_pane() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "204");
    h.key_press_modifiers(Modifiers::COMMAND, Key::H);
    settle(&mut h);
    // Leave the field (egui's focus on a button), then give the canvas the shell's focus: the canvas takes egui's.
    h.get_all_by_role(egui::Role::Button)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Find"))
        .expect("the Find button")
        .focus();
    settle(&mut h);
    dispatch(&mut h, "ui.focus", json!({"area": "canvas", "group": 0}));
    settle(&mut h);
    assert_eq!(h.state().shell().focus(), Some(Focus::Canvas(0)));
    let field = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Find"))
        .expect("the Find field");
    field.click();
    settle(&mut h);
    assert_eq!(h.state().shell().focus(), Some(Focus::Find));
    type_keys(&mut h, "Spare");
    h.key_press(Key::Tab);
    settle(&mut h);
    type_keys(&mut h, "Reserve");
    let f = h.state().shell().find_state();
    assert_eq!(f.text(), "Spare");
    assert_eq!(f.replacement(), "Reserve");
}

/// The result options of the list (the built rows).
fn options<'t>(h: &'t Harness<'_, App>) -> Vec<egui_kittest::Node<'t>> {
    h.get_all_by_role(egui::Role::ListBoxOption).collect()
}

/// AX-7 (FT-433): a result's accessible name is its line in plain text and its description the same line with the
/// matches marked; a long list builds only the rows on show, and the cursor's row is built wherever it moves.
#[test]
fn ax7_results_are_named_in_plain_text_and_only_the_rows_on_show_are_built() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "204");
    h.key_press_modifiers(Modifiers::COMMAND, Key::F);
    settle(&mut h);
    dispatch(&mut h, "find.fields", json!({"fields": ["function_text"]}));
    dispatch(&mut h, "find.folios", json!({"folios": ["204", "205", "206"]}));
    dispatch(&mut h, "find.text", json!({"text": "Spare"}));
    dispatch(&mut h, "find.run", json!({}));
    settle(&mut h);
    let hit = h.state().shell().find_state().results().unwrap().hits[0].clone();
    let marked = ew_ui::ew_viewmodel::find::marked(&hit);
    assert!(!hit.before.contains("[[") && marked.contains("[["), "{marked}");
    let lead = format!(
        "{} · {} · Function text · ",
        hit.cell.as_deref().or(hit.folio.as_deref()).unwrap(),
        hit.subject
    );
    let first = &options(&h)[0];
    assert_eq!(
        first.accesskit_node().label().as_deref(),
        Some(format!("{lead}{}", hit.before).as_str())
    );
    assert_eq!(
        first.accesskit_node().description().as_deref(),
        Some(format!("{lead}{marked}").as_str())
    );
    // Every field, every folio: more results than rows on show.
    dispatch(
        &mut h,
        "find.fields",
        json!({"fields": ["function_text", "tag", "property", "text", "part_number"]}),
    );
    dispatch(&mut h, "find.folios", json!({"folios": []}));
    dispatch(&mut h, "find.text", json!({"text": "-"}));
    dispatch(&mut h, "find.run", json!({}));
    settle(&mut h);
    let n = results(&h);
    let built = options(&h).len();
    assert!(built > 0 && built < n, "{built} rows built of {n}");
    // The cursor on the last result: the list scrolls to it and builds its row, the last one.
    dispatch(&mut h, "find.row", json!({ "to": n }));
    settle(&mut h);
    let last = h.state().shell().find_state().results().unwrap().hits[n - 1].clone();
    let built: Vec<String> = options(&h)
        .iter()
        .filter_map(|o| o.accesskit_node().label())
        .collect();
    assert_eq!(
        built.last(),
        Some(&ew_ui::find::result_line(&last, false)),
        "{built:#?}"
    );
    assert!(built.len() < n);
    // The cursor's option is selected for AccessKit, the others are not.
    let selected: Vec<Option<bool>> = options(&h)
        .iter()
        .map(|o| o.accesskit_node().is_selected())
        .collect();
    let mut expected = vec![Some(false); selected.len()];
    *expected.last_mut().unwrap() = Some(true);
    assert_eq!(selected, expected);
}

/// C49-p5#1: a result longer than the pane and a result with a line break keep one line's height (the row truncates),
/// so the cursor, moved past them, sits inside the list at the 1280 × 800 floor and at 1920 × 1080; its name and
/// description keep the whole line with the replacement, its tooltip shows that line (C49-p5b#1), and it alone is
/// selected for AccessKit.
#[test]
fn long_and_two_line_results_keep_the_cursor_inside_the_list() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "204");
    h.key_press_modifiers(Modifiers::COMMAND, Key::H);
    settle(&mut h);
    let pane_name = ew_ui::ew_viewmodel::strings::text(ew_ui::ew_viewmodel::key!("pane-find")).to_owned();
    // One line of body text in a button's padding: the list's row height.
    let style = h.ctx.style_of(h.ctx.theme());
    let line = h
        .ctx
        .fonts_mut(|f| f.row_height(&egui::TextStyle::Body.resolve(&style)));
    let one_line = (line + 2.0 * style.spacing.button_padding.y).max(style.spacing.interact_size.y);
    let long = "a replacement that runs past the right edge of the pane ".repeat(6);
    for size in [egui::Vec2::new(1280.0, 800.0), egui::Vec2::new(1920.0, 1080.0)] {
        h.set_size(size);
        settle(&mut h);
        for replacement in [long.trim_end(), "first line\nsecond line"] {
            dispatch(&mut h, "find.fields", json!({"fields": ["function_text"]}));
            dispatch(&mut h, "find.folios", json!({"folios": []}));
            dispatch(&mut h, "find.text", json!({"text": "e"}));
            dispatch(&mut h, "find.replacement", json!({ "text": replacement }));
            dispatch(&mut h, "find.run", json!({}));
            settle(&mut h);
            let hits = h.state().shell().find_state().results().unwrap().hits.clone();
            let replaced = hits.iter().filter(|hit| hit.after.is_some()).count();
            assert!(replaced > 12, "{replaced} of {} results replaced", hits.len());
            for to in [12, hits.len()] {
                let at = format!("{size:?}, {replacement:?}, row {to}");
                dispatch(&mut h, "find.row", json!({ "to": to }));
                settle(&mut h);
                let options = options(&h);
                let cursor: Vec<_> = options
                    .iter()
                    .filter(|o| o.accesskit_node().is_selected() == Some(true))
                    .collect();
                assert_eq!(cursor.len(), 1, "{at}");
                assert!(options.iter().all(|o| o.accesskit_node().is_selected().is_some()));
                let heights: Vec<f32> = options.iter().map(|o| o.rect().height()).collect();
                assert!(
                    heights.iter().all(|&height| height <= one_line + 0.5),
                    "{at}: rows of {heights:?} for one line of {one_line}"
                );
                // A replaced hit, cut on screen: its name and description carry the whole line and the replacement.
                let hit = &hits[to - 1];
                assert!(hit.after.is_some(), "{at}: a replaced hit");
                let whole = ew_ui::find::result_line(hit, false);
                assert!(whole.contains(replacement), "{at}: {whole}");
                let node = cursor[0].accesskit_node();
                assert_eq!(node.label().as_deref(), Some(whole.as_str()));
                assert_eq!(node.description(), Some(ew_ui::find::result_line(hit, true)));
                let mut list = cursor[0].parent();
                while let Some(n) = &list
                    && n.accesskit_node().role() != egui::Role::ListBox
                {
                    list = n.parent();
                }
                let list = list.expect("the result list").rect();
                let pane = h
                    .get_all_by_role(egui::Role::Group)
                    .find(|n| n.accesskit_node().label().as_deref() == Some(pane_name.as_str()))
                    .expect("the find pane")
                    .rect();
                // The list runs from its top to the pane's bottom.
                let row = cursor[0].rect();
                assert!(
                    row.top() >= list.top() && row.bottom() <= pane.bottom(),
                    "{at}: the cursor's row {row:?} outside the list from {} to {}",
                    list.top(),
                    pane.bottom()
                );
                // Its tooltip: the whole line.
                cursor[0].hover();
                settle(&mut h);
                settle(&mut h);
                assert!(
                    h.query_all_by_label(&whole)
                        .any(|n| n.accesskit_node().role() == egui::Role::Label),
                    "{at}: the tooltip with the whole line"
                );
            }
        }
    }
}

/// FT-1343 (FT-1260's first edge): an action without a form that fails when the palette runs it raises a banner the
/// window reports as that action, so the action's next success clears it.
#[test]
fn a_palette_run_that_fails_is_cleared_by_the_actions_next_success() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "204");
    dispatch(&mut h, "find.open_replace", json!({}));
    dispatch(&mut h, "find.fields", json!({"fields": ["function_text"]}));
    dispatch(&mut h, "find.option", json!({"option": "regex", "on": true}));
    dispatch(&mut h, "find.text", json!({"text": "("}));
    dispatch(&mut h, "find.replacement", json!({"text": "Reserve"}));
    settle(&mut h);
    h.state_mut().dispatch_from_window("tools.palette", &json!({}));
    settle(&mut h);
    h.state_mut().shell_mut().set_palette_query("Replace all");
    settle(&mut h);
    let palette = h.state().shell().palette().expect("the palette");
    assert_eq!(
        h.state().shell().palette_entries()[palette.selected].id,
        "find.replace_all"
    );
    h.key_press(Key::Enter);
    settle(&mut h);
    assert!(h.state().shell().palette().is_none());
    assert_eq!(
        h.state().shell().banner(),
        // FT-1414: in words, never the parser's own text.
        Some(
            "The regular expression cannot be used: check its brackets, repeats and escapes, or make it simpler"
        ),
        "the failure is a banner"
    );
    dispatch(&mut h, "find.text", json!({"text": "Spare"}));
    h.state_mut().dispatch_from_window("find.replace_all", &json!({}));
    settle(&mut h);
    assert_eq!(h.state().shell().banner(), None, "its success clears it");
    h.state_mut().shell_mut().shutdown();
}

/// W49.1-J: the folio filter takes the rest of its row (a row of its own where the rest cannot show its placeholder),
/// so its placeholder shows whole in Dutch at the 1280 × 800 floor and at 1920 × 1080 and the field reaches the pane's
/// right edge; every node fits at both sizes.
#[test]
fn the_folio_filter_takes_the_width_it_has_at_both_sizes() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "204");
    dispatch(&mut h, "view.language", json!({ "value": "nl" }));
    h.key_press_modifiers(Modifiers::COMMAND, Key::H);
    settle(&mut h);
    let name = ew_ui::ew_viewmodel::strings::text(ew_ui::ew_viewmodel::key!("find-folios")).to_owned();
    let pane_name = ew_ui::ew_viewmodel::strings::text(ew_ui::ew_viewmodel::key!("pane-find")).to_owned();
    for size in [egui::Vec2::new(1280.0, 800.0), egui::Vec2::new(1920.0, 1080.0)] {
        h.set_size(size);
        settle(&mut h);
        settle(&mut h);
        support::assert_fits(&h);
        let field = h
            .get_all_by_role(egui::Role::TextInput)
            .find(|n| n.accesskit_node().label().as_deref() == Some(name.as_str()))
            .expect("the folio field")
            .rect();
        let pane = h
            .get_all_by_role(egui::Role::Group)
            .find(|n| n.accesskit_node().label().as_deref() == Some(pane_name.as_str()))
            .expect("the find pane")
            .rect();
        let style = h.ctx.style_of(h.ctx.theme());
        let font = egui::TextStyle::Body.resolve(&style);
        let hint = h.ctx.fonts_mut(|f| {
            f.layout_no_wrap(name.clone(), font, egui::Color32::BLACK)
                .size()
                .x
        });
        // C49-p5#6: the field's frame keeps a button's padding each side (egui's text edit style).
        let margin = 2.0 * style.spacing.button_padding.x;
        assert!(
            field.width() >= hint + margin,
            "{size:?}: a field of {} for a placeholder of {hint} and a margin of {margin}",
            field.width()
        );
        // W49.2-F5: the rows keep the header's inset, `space.4`, at the right as at the left.
        let inset = ew_ui::theme::current(&h.ctx).space_4;
        assert!(
            (pane.right() - inset - field.right()).abs() <= 2.0,
            "{size:?}: the field ends at {} in a pane ending at {} with an inset of {inset}",
            field.right(),
            pane.right()
        );
    }
}

/// W49.2-F4 and F5: in replace mode the Find and Replace fields share what their row has beside the buttons, so a
/// long replacement shows more of itself at 1920 × 1080 than at the floor (doc 16a §2: a floor fix scales, never caps);
/// the fields and the buttons stay on one row ending at the pane's right inset, and the field rows start at the
/// header's inset, `space.4` (16a §4.9). Every node fits at both sizes.
#[test]
fn w49_2_f4_the_find_fields_take_their_rows_width_at_the_headers_inset() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "204");
    h.key_press_modifiers(Modifiers::COMMAND, Key::H);
    settle(&mut h);
    dispatch(&mut h, "find.text", json!({ "text": "e" }));
    dispatch(
        &mut h,
        "find.replacement",
        json!({ "text": "a replacement that runs past the edge ".repeat(8) }),
    );
    let label = |k| ew_ui::ew_viewmodel::strings::text(k).to_owned();
    let find = label(ew_ui::ew_viewmodel::key!("find-text"));
    let replacement = label(ew_ui::ew_viewmodel::key!("find-replacement"));
    let close = label(ew_ui::ew_viewmodel::key!("find-close"));
    let pane_name = label(ew_ui::ew_viewmodel::key!("pane-find"));
    let inset = ew_ui::theme::current(&h.ctx).space_4;
    let mut widths = Vec::new();
    for size in [egui::Vec2::new(1280.0, 800.0), egui::Vec2::new(1920.0, 1080.0)] {
        h.set_size(size);
        settle(&mut h);
        settle(&mut h);
        support::assert_fits(&h);
        let input = |name: &str| {
            h.get_all_by_role(egui::Role::TextInput)
                .find(|n| n.accesskit_node().label().as_deref() == Some(name))
                .unwrap_or_else(|| panic!("the field {name}"))
                .rect()
        };
        let (find, replacement) = (input(&find), input(&replacement));
        let close = h
            .get_all_by_role(egui::Role::Button)
            .find(|n| n.accesskit_node().label().as_deref() == Some(close.as_str()))
            .expect("Close")
            .rect();
        let pane = h
            .get_all_by_role(egui::Role::Group)
            .find(|n| n.accesskit_node().label().as_deref() == Some(pane_name.as_str()))
            .expect("the find pane")
            .rect();
        assert!(
            (find.left() - pane.left() - inset).abs() <= 1.0,
            "{size:?}: the row starts at {} in a pane starting at {} with an inset of {inset}",
            find.left(),
            pane.left()
        );
        assert!(
            (find.width() - replacement.width()).abs() <= 1.0,
            "{size:?}: {find:?} and {replacement:?} share the row"
        );
        assert!(
            (close.center().y - find.center().y).abs() <= 1.0,
            "{size:?}: Close {close:?} on the fields' row {find:?}"
        );
        assert!(
            (pane.right() - inset - close.right()).abs() <= 2.0,
            "{size:?}: the row ends at {} in a pane ending at {} with an inset of {inset}",
            close.right(),
            pane.right()
        );
        widths.push(replacement.width());
    }
    assert!(
        widths[1] > widths[0] + 100.0,
        "the replacement field grows with the window: {widths:?}"
    );
}

/// FT-1288: in a 1280 × 720 window the find pane's empty state shows its two lines whole inside the pane (its icon
/// gives way), in Find and in Find and replace, and still shows its icon at 1920 × 1080.
#[test]
fn the_empty_state_shows_whole_in_a_short_window() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "204");
    let text = |k| ew_ui::ew_viewmodel::strings::text(k).to_owned();
    let pane_name = text(ew_ui::ew_viewmodel::key!("pane-find"));
    let lines = [
        text(ew_ui::ew_viewmodel::key!("empty-find-start")),
        text(ew_ui::ew_viewmodel::key!("empty-find-start-text")),
    ];
    for (size, chord) in [
        (egui::Vec2::new(1280.0, 720.0), Key::F),
        (egui::Vec2::new(1280.0, 720.0), Key::H),
        (egui::Vec2::new(1920.0, 1080.0), Key::H),
    ] {
        h.set_size(size);
        settle(&mut h);
        h.key_press_modifiers(Modifiers::COMMAND, chord);
        settle(&mut h);
        settle(&mut h);
        let pane = h
            .get_all_by_role(egui::Role::Group)
            .find(|n| n.accesskit_node().label().as_deref() == Some(pane_name.as_str()))
            .expect("the find pane")
            .rect();
        for line in &lines {
            let r = h.get_by_role_and_label(egui::Role::Label, line).rect();
            assert!(
                pane.contains_rect(r),
                "{size:?} {chord:?}: {line} {r:?} inside {pane:?}"
            );
        }
        let _ = h.state_mut().shell_mut().dispatch("find.close", &json!({}));
        settle(&mut h);
    }
}
