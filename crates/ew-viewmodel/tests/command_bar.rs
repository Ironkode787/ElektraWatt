// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.17 part 2: the command surface's model. CB1 (the registry's placements and the app menu), CB6 (the describe
//! lines, deterministic), CB8 (the Draw tab equals the mockup's command row) and the HUD's prompts (CB5's model side,
//! FT-521).

mod common;

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
    // WP-S2.07 part 1 (§6.9): the strip editor's row actions, reached through its row menu and keys.
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
fn cb1_the_registry_places_every_command_once_and_the_app_menu_holds_every_action_once() {
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
        assert!(strings::lookup(a.label_key).is_some(), "{} has no label", a.id);
        if let Some(key) = p.bar_label {
            assert!(strings::lookup(key).is_some(), "{}: bar label {key}", a.id);
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
        assert!(strings::lookup(tab.label_key()).is_some());
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
    // The Window menu lists the open tabs as items of the hidden `tab.activate` (WP-S2.03).
    for id in found.keys().filter(|id| **id != "tab.activate") {
        let a = actions::find(id).unwrap();
        assert!(!a.headless_only && !a.hidden, "{id} is in the menu");
    }
    s.shutdown();
}

/// One command of the mockup's command row: its group, label and size.
type MockupCommand = (String, String, CommandSize);

/// The Draw tab's commands of the mockup's command row: (group, label, size) in order, and the next step's label.
fn mockup_draw() -> (Vec<MockupCommand>, String, Vec<String>, String) {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/design/assets/design-language-mockup.html"
    );
    let html = std::fs::read_to_string(path).expect("the mockup");
    let row_start = html.find("<div class=\"cmdrow\"").expect("the command row");
    let row_end = row_start + html[row_start..].find("</header>").expect("the bar's end");
    let row = &html[row_start..row_end];
    // The label of a button is its text after the last </svg>.
    let label_of = |button: &str| -> String {
        let body = &button[..button.find("</button>").unwrap()];
        let text = body.rsplit("</svg>").next().unwrap();
        text.trim().to_owned()
    };
    let mut commands = Vec::new();
    let mut group = String::new();
    let mut rest = row;
    let mut next = String::new();
    while let Some(i) = rest.find('<') {
        rest = &rest[i..];
        if let Some(r) = rest.strip_prefix("<div class=\"group\" aria-label=\"") {
            group = r[..r.find('"').unwrap()].to_owned();
        } else if rest.starts_with("<button class=\"cmd") {
            let class = &rest[15..rest[15..].find('"').unwrap() + 15];
            let size = if class.split(' ').any(|c| c == "small") {
                CommandSize::Small
            } else {
                CommandSize::Large
            };
            commands.push((group.clone(), label_of(rest), size));
        } else if rest.starts_with("<button class=\"primary") {
            next = label_of(rest);
        }
        rest = &rest[1..];
    }
    let tabs_start = html.find("role=\"tablist\"").unwrap();
    let tabs_end = tabs_start + html[tabs_start..].find("</nav>").unwrap();
    let tabs = html[tabs_start..tabs_end]
        .split("role=\"tab\"")
        .skip(1)
        .map(|t| {
            let t = &t[t.find('>').unwrap() + 1..];
            t[..t.find('<').unwrap()].trim().to_owned()
        })
        .collect();
    let app_start = html.find("<button class=\"appbtn").unwrap();
    let app = &html[app_start..app_start + html[app_start..].find("</button>").unwrap()];
    let title = app.split("title=\"").nth(1).unwrap();
    let title = title[..title.find('"').unwrap()].to_owned();
    let name = if app.contains(">ElektraWatt") {
        "ElektraWatt".to_owned()
    } else {
        String::new()
    };
    (commands, next, tabs, format!("{name}|{title}"))
}

/// CB8: the Draw tab's buttons parsed from the mockup's command row (group, label, Large or Small, in order) equal
/// `command_bar` for Draw, a mockup command whose action is not in the registry listed by name in `PENDING` and
/// skipped; the next step reads "Check project"; the six tab labels equal the mockup's; the app button is named
/// "ElektraWatt" and its accessible name is "Menu".
#[test]
fn cb8_the_draw_tab_equals_the_mockups_command_row() {
    let (mockup, next, tabs, app) = mockup_draw();
    assert!(mockup.len() >= 10, "parsed {mockup:?}");
    let pending: Vec<&str> = PENDING.iter().map(|(_, _, n)| *n).collect();
    let expected: Vec<MockupCommand> = mockup
        .into_iter()
        .filter(|(_, label, _)| !pending.contains(&label.as_str()))
        .collect();
    for name in ["Terminal", "Potential", "Align"] {
        assert!(pending.contains(&name), "{name} is pending");
    }
    let tmp = Tmp::new("cb8");
    let mut s = shell(&tmp);
    let bar = command_bar_of(&s, Tab::Draw);
    let built: Vec<(String, String, CommandSize)> = bar
        .groups
        .iter()
        .flat_map(|g| {
            g.buttons
                .iter()
                .map(|b| (g.label.clone(), b.bar_label.clone(), b.size))
        })
        .collect();
    assert_eq!(built, expected);
    assert_eq!(
        bar.next.as_ref().map(|b| b.bar_label.as_str()),
        Some(next.as_str())
    );
    assert_eq!(bar.next.as_ref().map(|b| b.id), Some("messages.check"));
    let labels: Vec<String> = bar.tabs.iter().map(|(_, l)| l.clone()).collect();
    assert_eq!(labels, tabs);
    assert_eq!(
        app,
        format!(
            "{}|{} (Alt)",
            strings::text("commandbar-app"),
            strings::text("commandbar-menu")
        )
    );
    // The Select tool is on in the bar.
    assert!(bar.groups[0].buttons[0].checked);
    // Mirror opens its two directions.
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
fn cb6_the_command_tab_and_the_collapse_reach_the_describe_text() {
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

/// CB5's model side and FT-521: with Select and one selected object the prompt is "1 selected · Del deletes · R
/// rotates"; with the pointer over a pick stack of n objects it is "n here · click again for the next".
#[test]
fn cb5_ft521_the_hud_prompts_the_selection_and_the_pick_stack() {
    let tmp = Tmp::new("cb5");
    let mut s = shell(&tmp);
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
    // FT-521: at an overlap of two wires and a symbol (FT-500's point on folio 101 at actual size).
    done(&mut s, "view.zoom_actual", json!({}));
    let page = s.canvas().unwrap().page();
    let y = page
        .snapper
        .page_to_sheet(page.tables.connections[0].polyline[0])
        .1;
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
    assert_eq!(h.prompt, format!("{n} here \u{b7} click again for the next"));
    assert!(
        s.describe()
            .contains(&format!("\nhud tool tool.select \"Select\" prompt \"{n} here")),
        "the hud line"
    );
    s.shutdown();
}

/// Arming Place names the tool and, once a symbol is picked and the pointer on the sheet, the target and the keys.
#[test]
fn cb5_arming_place_names_the_tool_the_target_and_its_keys() {
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
