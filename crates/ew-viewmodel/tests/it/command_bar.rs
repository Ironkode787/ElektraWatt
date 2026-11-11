// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The command surface's model: the registry's placements and the app menu, the describe
//! lines (deterministic), the Mirror split and the HUD's prompts.

use crate::common;
use ew_viewmodel::key;

use std::collections::BTreeMap;

use common::{Tmp, at, click, done, empty_point};
use ew_viewmodel::actions::{self, ACTIONS, MenuEntry, menu_model};
use ew_viewmodel::canvas::HOVER_SLOP_PT;
use ew_viewmodel::command_bar::{
    self, ButtonKind, CommandSize, Group, PENDING, Tab, command_bar_of, hud, next_step,
};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use ew_viewmodel::{help, strings};
use serde_json::json;

/// The actions that a key or the canvas runs and that have no place in the app menu (CB1's exceptions): the grid
/// cursor's click, the tool's step back, the model-command runner and the table editor's keys.
const KEY_ONLY: &[&str] = &[
    "canvas.activate",
    "tool.back",
    // The strip editor's row actions, reached through its row menu and keys.
    "strip.insert",
    "strip.delete_rows",
    "strip.move",
    "strip.unbridge",
    "command.run",
    "table.fill_down",
    "table.pattern_fill",
    "table.paste",
    "table.copy",
    "table.clear",
];

fn shell(tmp: &Tmp) -> ShellModel {
    tmp.shell_in(Mode::Headless, false, "101")
}

/// Every place an action takes in the app menu: the menu and the path of submenus and groups down to it.
fn menu_places(model: &ShellModel) -> BTreeMap<&'static str, Vec<String>> {
    fn walk(entry: &MenuEntry, path: &str, out: &mut BTreeMap<&'static str, Vec<String>>) {
        match entry {
            MenuEntry::Separator => {}
            MenuEntry::Item(i) => out.entry(i.id).or_default().push(path.to_owned()),
            MenuEntry::Submenu { label, items, .. } => {
                let here = format!("{path} > {label}");
                for i in items {
                    let places = out.entry(i.id).or_default();
                    // A submenu's items of one action with other parameters (Theme, Command tab) are one place.
                    if !places.contains(&here) {
                        places.push(here.clone());
                    }
                }
            }
            MenuEntry::Group { label, entries, .. } => {
                for e in entries {
                    walk(e, &format!("{path} > {label}"), out);
                }
            }
        }
    }
    let mut out = BTreeMap::new();
    for menu in menu_model(model) {
        for e in &menu.entries {
            walk(e, &menu.label, &mut out);
        }
    }
    out
}

/// CB1: every placed action has an icon key, a label and a docs anchor; the tabs and groups are in doc 16a §5.3's
/// order; Draw holds Select, Place, Connect, Annotate and Arrange; no place is taken twice; at most two Large per
/// group; every registry action appears exactly once in the app-menu tree.
#[test]
fn the_registry_places_every_command_once_and_the_app_menu_holds_every_action_once() {
    assert_eq!(
        Tab::ALL.map(Tab::word),
        ["project", "page", "draw", "check", "output", "view"]
    );
    let in_tab_order: Vec<Group> = Tab::ALL.into_iter().flat_map(Tab::groups).collect();
    assert_eq!(in_tab_order, Group::ALL, "the groups run tab by tab");
    assert_eq!(
        Tab::Draw.groups().collect::<Vec<_>>(),
        [
            Group::Select,
            Group::Place,
            Group::Connect,
            Group::PanelLayout,
            Group::Annotate,
            Group::Arrange
        ]
    );
    let mut places = Vec::new();
    for a in ACTIONS.iter() {
        let Some(p) = a.placement else { continue };
        assert_eq!(p.group.tab(), p.tab, "{}: the group's tab", a.id);
        let icon = a.icon.unwrap_or_else(|| panic!("{} has no icon key", a.id));
        assert!(!icon.is_empty(), "{}", a.id);
        assert!(strings::lookup(&a.label_key()).is_some(), "{} has no label", a.id);
        if let Some(key) = p.bar_label {
            assert!(
                strings::lookup(key.as_str()).is_some(),
                "{}: bar label {key:?}",
                a.id
            );
        }
        let topic = help::action_topic(a.id).unwrap_or_else(|| panic!("{} has no docs topic", a.id));
        assert!(help::anchor(topic).is_some(), "{}: topic {topic}", a.id);
        assert!(
            !a.headless_only && !a.hidden,
            "{} is placed but not in the window",
            a.id
        );
        assert!(
            !places.contains(&(p.group, p.order)),
            "{}: {:?} {} is taken twice",
            a.id,
            p.group,
            p.order
        );
        places.push((p.group, p.order));
    }
    for (group, order, name) in PENDING {
        assert!(
            !places.contains(&(*group, *order)),
            "the pending {name} shares {group:?} {order}"
        );
    }
    for group in Group::ALL {
        let large = command_bar::placed_in(group)
            .iter()
            .filter(|a| a.placement.is_some_and(|p| p.size == CommandSize::Large))
            .count();
        assert!(large <= 2, "{group:?} has {large} Large commands");
        assert!(
            strings::lookup(&group.label_key()).is_some(),
            "{group:?} has a name"
        );
    }
    for tab in Tab::ALL {
        assert!(strings::lookup(tab.label_key().as_str()).is_some());
        if let Some(id) = next_step(tab) {
            assert!(actions::find(id).is_some(), "{tab:?}'s next step {id}");
            assert!(strings::lookup(&format!("next-step-{}", tab.word())).is_some());
        }
    }

    let tmp = Tmp::new("cb1");
    let mut s = shell(&tmp);
    let found = menu_places(&s);
    let mut missing = Vec::new();
    for a in ACTIONS.iter().filter(|a| !a.headless_only && !a.hidden) {
        match found.get(a.id) {
            None => missing.push(a.id),
            Some(p) => assert_eq!(p.len(), 1, "{} appears in {p:?}", a.id),
        }
    }
    assert_eq!(missing, KEY_ONLY, "the actions outside the app menu");
    // The Window menu lists the open tabs as items of the hidden `tab.activate`.
    for id in found.keys().filter(|id| **id != "tab.activate") {
        let a = actions::find(id).unwrap();
        assert!(!a.headless_only && !a.hidden, "{id} is in the menu");
    }
    s.shutdown();
}

/// One name per thing (doc 16a P7): the Wire tool is "Wire" on its button, in its tooltip ("Wire (W)", the
/// registry label, which the menus and the palette carry too, and the key from the keymap).
#[test]
fn w42b_the_wire_tool_is_called_wire_on_its_button_and_in_its_tooltip() {
    let tmp = Tmp::new("w42b-wire");
    let s = shell(&tmp);
    let bar = command_bar_of(&s, Tab::Draw);
    let wire = bar
        .groups
        .iter()
        .flat_map(|g| g.buttons.iter())
        .find(|b| b.id == "tool.wire")
        .expect("the Wire button");
    let key = ew_viewmodel::keymap::display(&s.shortcut_text(actions::find("tool.wire").unwrap()));
    assert_eq!(
        (wire.bar_label.as_str(), format!("{} ({key})", wire.label)),
        ("Wire", format!("Wire ({key})"))
    );
}

/// CB8: the Draw tab's Mirror button opens its two directions.
#[test]
fn the_draw_tab_mirror_opens_its_two_directions() {
    let tmp = Tmp::new("cb8");
    let mut s = shell(&tmp);
    let bar = command_bar_of(&s, Tab::Draw);
    let mirror = bar
        .groups
        .iter()
        .flat_map(|g| &g.buttons)
        .find(|b| b.id == "tool.mirror_x")
        .unwrap();
    match &mirror.kind {
        ButtonKind::Split(items) => {
            assert_eq!(
                items.iter().map(|i| i.id).collect::<Vec<_>>(),
                ["tool.mirror_x", "tool.mirror_y"]
            );
        }
        other => panic!("Mirror is {other:?}"),
    }
    s.shutdown();
}

/// `view.command_tab` switches the bar's tab and `view.command_bar` collapses it, toggles without a parameter and
/// refuses a bad one; the describe text follows (D15) and is the same twice (CB6).
#[test]
fn the_command_tab_and_the_collapse_reach_the_describe_text() {
    let tmp = Tmp::new("cb6");
    let mut s = shell(&tmp);
    let text = s.describe();
    assert_eq!(text, s.describe(), "deterministic");
    let lines: Vec<&str> = text.lines().collect();
    let head = lines
        .iter()
        .position(|l| l.starts_with("commandbar "))
        .expect("the commandbar section");
    assert_eq!(lines[head], "commandbar tab \"Draw\"");
    assert!(
        lines[head + 1].starts_with("  group \"Select\" tool.select \"Select\""),
        "{}",
        lines[head + 1]
    );
    assert!(
        lines[head + 1].contains("(checked) enabled"),
        "{}",
        lines[head + 1]
    );
    let next = lines[head..]
        .iter()
        .find(|l| l.starts_with("  next step "))
        .expect("the next step");
    assert!(
        next.starts_with("  next step messages.check \"Check project\" [F7]"),
        "{next}"
    );
    assert!(text.contains("\nhud tool tool.select \"Select\" prompt \""));

    done(&mut s, "view.command_tab", json!({"tab": "view"}));
    assert_eq!(s.command_tab(), Tab::View);
    assert!(
        s.describe()
            .contains("\ncommandbar tab \"View\"\n  group \"Zoom\" view.fit_page")
    );
    assert!(matches!(
        s.dispatch("view.command_tab", &json!({"tab": "nowhere"})),
        Outcome::Failed { .. }
    ));
    assert_eq!(s.command_tab(), Tab::View);
    done(&mut s, "view.command_bar", json!({}));
    assert!(s.command_bar_collapsed());
    let text = s.describe();
    let head = text.lines().position(|l| l.starts_with("commandbar ")).unwrap();
    let after: Vec<&str> = text.lines().skip(head).take(2).collect();
    assert_eq!(after[0], "commandbar tab \"View\" collapsed");
    assert!(!after[1].starts_with("  group "), "{}", after[1]);
    done(&mut s, "view.command_bar", json!({"collapsed": false}));
    assert!(!s.command_bar_collapsed());
    done(&mut s, "view.command_bar", json!({"collapsed": true}));
    assert!(s.command_bar_collapsed());
    // The menu's Collapse item is checked while collapsed; the Command tab submenu checks the active tab.
    let menus = menu_model(&s);
    let view = menus.iter().find(|m| m.menu == actions::Menu::View).unwrap();
    let items = view.items();
    assert!(items.iter().any(|i| i.id == "view.command_bar" && i.checked));
    let tabs: Vec<(&str, bool)> = items
        .iter()
        .filter(|i| i.id == "view.command_tab")
        .map(|i| (i.label.as_str(), i.checked))
        .collect();
    assert_eq!(
        tabs,
        [
            ("Project", false),
            ("Page", false),
            ("Draw", false),
            ("Check", false),
            ("Output", false),
            ("View", true)
        ]
    );
    s.shutdown();
}

/// A new page shows with nothing selected, so its HUD does not read "1 selected".
#[test]
fn page_new_shows_the_new_page_with_nothing_selected() {
    let tmp = Tmp::new("ft1116");
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    let empty = empty_point(s.canvas().unwrap());
    let (_, p) = common::entity_points(s.canvas().unwrap())[0];
    click(&mut s, empty, json!({}));
    click(&mut s, p, json!({}));
    assert_eq!(s.selection().items.len(), 1);
    done(&mut s, "page.new", json!({"folio": "199"}));
    assert_eq!(s.current_folio(), Some("199"));
    assert!(s.selection().items.is_empty());
    assert_eq!(hud(&s).prompt, s.tool_hint());
    s.shutdown();
}

/// A page chosen in the page list selects its page record, which draws nothing on the canvas: the
/// canvas's HUD and status bar do not count it ("1 selected · Del deletes") and Edit ▸ Delete is not offered for it,
/// while the selection itself stays (the page list's Delete deletes the page). A device drawn on the page is counted
/// as before, also when it is selected by its tag (Delete deletes its representations on the page).
#[test]
fn a_page_chosen_in_the_page_list_is_not_counted_on_the_canvas() {
    use ew_viewmodel::navigator::RowKind;
    use ew_viewmodel::status::{StatusKind, bar};
    let tmp = Tmp::new("id1497-8");
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    let key = s
        .navigator_rows()
        .into_iter()
        .find(|r| matches!(r.kind, RowKind::Page(i) if s.page_rows()[i].folio == "101"))
        .expect("the row of 101")
        .key;
    done(
        &mut s,
        "navigator.select",
        json!({"kind": "pages", "keys": [key]}),
    );
    assert_eq!(s.selection().items.len(), 1, "the page record is selected");
    assert_eq!(s.canvas_selected(), 0);
    assert_eq!(hud(&s).prompt, s.tool_hint());
    assert!(!bar(&s, false).iter().any(|f| f.kind == StatusKind::Selection));
    let delete = actions::find("edit.delete_selection").unwrap();
    assert_eq!(
        (delete.enabled)(&s),
        Err(key!("reason-nothing-selected")),
        "Delete is not offered for the page record"
    );
    // A device selected by its tag and drawn on the page counts, as Delete takes it.
    done(&mut s, "select.set", json!({"refs": ["tag:-101F1"]}));
    assert_eq!(s.canvas_selected(), 1);
    assert_eq!((delete.enabled)(&s), Ok(()));
    let selected: Vec<String> = bar(&s, false)
        .into_iter()
        .filter(|f| f.kind == StatusKind::Selection)
        .map(|f| f.text)
        .collect();
    assert_eq!(selected, ["1 selected"]);
    assert_eq!(hud(&s).prompt, "1 selected \u{b7} Del deletes \u{b7} R rotates");
    // A click on a drawn item counts.
    let (_, p) = common::entity_points(s.canvas().unwrap())[0];
    click(&mut s, p, json!({}));
    assert_eq!(s.canvas_selected(), 1);
    assert!(bar(&s, false).iter().any(|f| f.kind == StatusKind::Selection));
    s.shutdown();
}

/// With Select and one selected object the prompt is "1 selected · Del deletes · R
/// rotates"; with the pointer over a pick stack of n objects it is "n objects overlap here · click again to select the
/// next".
#[test]
fn the_hud_prompts_the_selection_and_the_pick_stack() {
    let tmp = Tmp::new("cb5");
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    let h = hud(&s);
    assert_eq!(h.tool, "tool.select");
    assert_eq!(h.quick, ["tool.wire", "tool.place", "tool.text"]);
    assert_eq!(h.prompt, s.tool_hint());
    // One object selected, the pointer on empty sheet.
    let empty = empty_point(s.canvas().unwrap());
    let (_, p) = common::entity_points(s.canvas().unwrap())[0];
    click(&mut s, empty, json!({}));
    click(&mut s, p, json!({}));
    assert_eq!(s.selection().items.len(), 1);
    done(&mut s, "canvas.pointer", at(empty, json!({})));
    assert_eq!(hud(&s).prompt, "1 selected \u{b7} Del deletes \u{b7} R rotates");
    // At an overlap of two wires and a symbol (FT-500's point on folio 101 at actual size).
    done(&mut s, "view.zoom_actual", json!({}));
    let y = common::provisional_overlap_on_101(&mut s);
    let over = (150_000_000, y);
    done(&mut s, "canvas.pointer", at(over, json!({})));
    let c = s.canvas().unwrap();
    let n = ew_viewmodel::tools::pick_stack(
        c.page(),
        &c.page().snapper,
        over,
        HOVER_SLOP_PT / c.points_per_nm(),
    )
    .len();
    assert!(n >= 3, "the stack at the overlap: {n}");
    let h = hud(&s);
    assert_eq!(h.stack, n);
    assert_eq!(
        h.prompt,
        format!("{n} objects overlap here \u{b7} click again to select the next")
    );
    assert!(
        s.describe().contains(&format!(
            "\nhud tool tool.select \"Select\" prompt \"{n} objects overlap here"
        )),
        "the hud line"
    );
    s.shutdown();
}

/// Arming Place names the tool and, once a symbol is picked and the pointer on the sheet, the target and the keys.
#[test]
fn arming_place_names_the_tool_the_target_and_its_keys() {
    let tmp = Tmp::new("cb5-place");
    let mut s = tmp.shell_in(Mode::Headless, true, "103");
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1031", "designation": "30"}),
    );
    let snap = &s.canvas().unwrap().page().snapper;
    let p = snap.page_to_sheet((snap.page_of_index(70), snap.page_of_index(40)));
    done(&mut s, "canvas.pointer", at(p, json!({})));
    let h = hud(&s);
    assert_eq!(h.tool, "tool.place");
    assert_eq!(h.target.as_deref(), Some("-X1031:30"));
    assert!(
        h.prompt.ends_with("R rotates \u{b7} V variant \u{b7} Esc done"),
        "{}",
        h.prompt
    );
    assert!(h.prompt.contains("-X1031:30"), "{}", h.prompt);
    s.shutdown();
}

/// The Draw tab's Renumber reads "Renumber devices", and Edit route stands in the Connect group beside
/// Wire.
#[test]
fn the_draw_tab_names_renumber_devices_and_offers_edit_route_beside_wire() {
    let ids: Vec<&str> = command_bar::placed_in(Group::Connect)
        .iter()
        .map(|m| m.id)
        .collect();
    assert_eq!(ids[..2], ["tool.wire", "tool.route_edit"]);
    let tmp = Tmp::new("id1497-10");
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    let renumber = command_bar::action_button(&s, "devices.renumber").unwrap();
    assert_eq!(renumber.bar_label, "Renumber devices");
    let route = command_bar::action_button(&s, "tool.route_edit").unwrap();
    assert_eq!(route.bar_label, "Edit route");
    s.shutdown();
}
