// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The person's own parts, made in the window from nothing (FT-1766, FT-1767): New part… in the device browser opens
//! the part editor on a new part, whose bus interfaces are a form; Save puts the part into the project and into My
//! parts. The window's own path: clicks and keys on a new project at 1920 × 1080.

use crate::persona_round::{click, key, typed};
use crate::support::Tmp;

use ew_ui::ew_viewmodel::ew_api::library::LibraryTiers;
use ew_ui::ew_viewmodel::shell::{Dialog, Outcome};
use ew_ui::visual::Driver;
use serde_json::{Value, json};

/// A driver over a new project from the built-in template, with the person's own libraries under the test's folder
/// (My parts is made there).
pub(crate) fn new_project(tmp: &Tmp) -> Driver {
    let tiers = LibraryTiers::under(&tmp.0.join("data"), &tmp.0.join("cache"), Vec::new(), Vec::new());
    let mut d = Driver::in_folder(tmp.config().with_libraries(Some(tiers)), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let project = json!({"dir": tmp.0.join("p.ew"), "template": "builtin"});
    assert_eq!(d.shell_mut().dispatch("file.new", &project), Outcome::Done);
    d.settle();
    d
}

/// Clicks the last widget named `name` (a dialog is painted over the window, so its widgets come last: the dialog's
/// Save, not the quick-access one; its Description, not the page's) and settles.
pub(crate) fn press(d: &mut Driver, name: &str) {
    let at = d
        .labels()
        .into_iter()
        .rfind(|(l, _)| l == name)
        .unwrap_or_else(|| panic!("no widget named {name:?}"))
        .1;
    d.input(&click(at));
    d.settle();
}

/// Clicks the field named `name` and types `text` into it.
pub(crate) fn fill(d: &mut Driver, name: &str, text: &str) {
    press(d, name);
    d.input(&typed(text));
    d.settle();
}

/// The open part editor's draft.
fn draft(d: &mut Driver) -> ew_ui::ew_viewmodel::ew_api::PartDraft {
    match d.shell_mut().dialog() {
        Some(Dialog::PartEdit(e)) => e.draft.clone(),
        other => panic!("the part editor is open: {other:?}"),
    }
}

/// The open project's records as JSON, one per record.
pub(crate) fn records(d: &mut Driver) -> Vec<Value> {
    let world = d.shell_mut().world_for_tests().expect("a project");
    ew_commands::rows::model_lines(&world.model)
        .expect("every record has a JSON form")
        .into_iter()
        .map(|(_, line)| serde_json::from_str::<Value>(&line).unwrap_or(Value::Null))
        .collect()
}

/// The texts the page shown prints, groups followed.
pub(crate) fn page_texts(d: &mut Driver) -> Vec<String> {
    use ew_ui::ew_viewmodel::ew_api::output::ew_dl::{Item, ItemKind};
    fn walk(items: &[Item], out: &mut Vec<String>) {
        for item in items {
            match &item.kind {
                ItemKind::Glyphs(run) => out.push(run.text.clone()),
                ItemKind::Group(g) => walk(&g.items, out),
                _ => {}
            }
        }
    }
    let list = std::sync::Arc::clone(&d.shell_mut().canvas().expect("a page").page().list);
    let mut out = Vec::new();
    walk(&list.items, &mut out);
    out
}

/// Every file of a library folder, one after the other.
fn library_text(dir: &std::path::Path) -> String {
    let mut out = String::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for e in std::fs::read_dir(&at).into_iter().flatten().flatten() {
            if e.path().is_dir() {
                stack.push(e.path());
            } else {
                out.push_str(&std::fs::read_to_string(e.path()).unwrap_or_default());
            }
        }
    }
    out
}

/// An unmanaged switch with five ports, typed from its data sheet: New part… in the device browser, the identity, one
/// bus interface whose role is passive and whose port count is 5 (a wrong count is refused in words at the field),
/// Save. The project holds the part with its bus section, as My parts' part, and My parts holds it for the next
/// project.
#[test]
fn a_new_part_with_a_bus_interface_is_made_in_the_window_and_kept_in_my_parts() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    d.input(&key("Shift+I"));
    d.settle();
    press(&mut d, "New part…");
    assert!(draft(&mut d).new, "New part… opens the editor on a new part");
    fill(&mut d, "Manufacturer", "Acme");
    fill(&mut d, "Part number", "SW-5");
    fill(&mut d, "Description", "Ethernet switch, unmanaged, 5 ports");
    press(&mut d, "Add interface");
    assert_eq!(draft(&mut d).bus.len(), 1, "one interface row");
    press(&mut d, "Role 1");
    press(&mut d, "Passive");
    fill(&mut d, "Ports 1", "0");
    let said = d
        .texts()
        .into_iter()
        .any(|t| t.starts_with("Ports: \u{201c}0\u{201d} is no number of ports"));
    assert!(said, "a wrong port count is refused in words at its field");
    d.input(&key("Backspace"));
    d.input(&typed("5"));
    d.settle();
    let row = draft(&mut d).bus[0].clone();
    assert_eq!(
        (
            row.interface.as_str(),
            row.bus.as_str(),
            row.role.as_str(),
            row.ports.as_str()
        ),
        ("X1", "profinet", "passive", "5")
    );
    press(&mut d, "Save");
    let left = match d.shell_mut().dialog() {
        Some(Dialog::PartEdit(e)) => Some((e.error.clone(), e.draft.messages.clone())),
        _ => None,
    };
    assert_eq!(left, None, "Save closes the editor");
    let all = records(&mut d);
    let part = all
        .iter()
        .find(|v| v["part_number"] == "SW-5")
        .expect("the project holds the new part");
    assert!(
        part["description"]
            .to_string()
            .contains("Ethernet switch, unmanaged, 5 ports"),
        "{}",
        part["description"]
    );
    let port = &part["sections"]["bus"]["ports"][0];
    assert_eq!(
        (port["role"].as_str(), port["ports"].as_u64()),
        (Some("passive"), Some(5))
    );
    let toast = d.shell_mut().toast().map(|t| t.text.clone());
    assert_eq!(
        toast.as_deref(),
        Some("Part SW-5 is in the project and in My parts.")
    );
    let mine = tmp.0.join("data").join("libraries").join("user-parts.ewlib");
    assert!(
        library_text(&mine).contains("\"part_number\":\"SW-5\""),
        "My parts holds it for the next project"
    );
}

/// Moves the keyboard cursor to the point at grid indices `k` of the page shown (the window's keyboard path).
pub(crate) fn cursor_k(d: &mut Driver, k: (i64, i64)) {
    let (x, y) = {
        let c = d.shell_mut().canvas().expect("a page");
        let sn = &c.page().snapper;
        let s = sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1)));
        (s.0 as f64 / 1e6, s.1 as f64 / 1e6)
    };
    let _ = d
        .shell_mut()
        .dispatch("cursor.set", &json!({"x_mm": x, "y_mm": y}));
}

/// Moves the keyboard cursor onto the connection point labelled `label` (`-K1:P1`) and presses there.
pub(crate) fn press_cp(d: &mut Driver, label: &str) {
    let (x, y) = {
        let c = d.shell_mut().canvas().expect("a page");
        let at = c
            .page()
            .tables
            .cps
            .iter()
            .find(|cp| cp.label == label)
            .unwrap_or_else(|| panic!("no CP {label}"))
            .at;
        let s = c.page().snapper.page_to_sheet(at);
        (s.0 as f64 / 1e6, s.1 as f64 / 1e6)
    };
    let _ = d
        .shell_mut()
        .dispatch("cursor.set", &json!({"x_mm": x, "y_mm": y}));
    let _ = d.shell_mut().dispatch("canvas.activate", &json!({}));
    d.settle();
}

/// The labels of the connection points of the page shown.
pub(crate) fn cp_labels(d: &mut Driver) -> Vec<String> {
    let c = d.shell_mut().canvas().expect("a page");
    c.page().tables.cps.iter().map(|cp| cp.label.clone()).collect()
}

/// Places part `number` from the device browser as a box with the corners at grid indices `a` and `b`; returns the
/// labels of the connection points it brought.
pub(crate) fn place_box(d: &mut Driver, number: &str, a: (i64, i64), b: (i64, i64)) -> Vec<String> {
    let before = cp_labels(d);
    d.input(&key("Shift+I"));
    // The browser keeps its last filter: the field is set as typing into it sets it.
    let _ = d.shell_mut().dispatch("picker.filter", &json!({"text": number}));
    d.settle();
    d.input(&key("Enter"));
    d.settle();
    for k in [a, b] {
        cursor_k(d, k);
        let _ = d.shell_mut().dispatch("canvas.activate", &json!({}));
        d.settle();
    }
    d.input(&key("Escape"));
    d.input(&key("Escape"));
    d.settle();
    cp_labels(d).into_iter().filter(|l| !before.contains(l)).collect()
}

/// The texts of the findings of the rules whose code starts with `prefix`, after Check project.
pub(crate) fn findings(d: &mut Driver, prefix: &str) -> Vec<String> {
    use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
    press(d, "Check project");
    let _ = d
        .shell_mut()
        .dispatch("navigator.show", &json!({"kind": "messages"}));
    d.settle();
    d.shell_mut()
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .map(|t| {
            t.nodes
                .iter()
                .filter(|n| n.role == NavRole::Finding && n.cells[1].starts_with(prefix))
                .map(|n| format!("{}: {}", n.cells[1], n.cells[2]))
                .collect()
        })
        .unwrap_or_default()
}

/// FT-1767, a star on a new project: the CPU with PROFINET, the starter's unmanaged five-port switch and two generic
/// PROFINET devices, each placed from the device browser on a bus-topology page. The switch and the devices stand as
/// boxes with every socket (P1 … P5, P1 and P2), the CPU as its interfaces DP and X1. Bus lines from the CPU's X1 to
/// the switch and from the switch to each device make one PROFINET net of the CPU and both devices, which the switch
/// passes through (the Bus navigator lists it in the net), and no EW-BUS rule speaks.
#[test]
fn a_switch_joins_a_cpu_and_two_devices_in_one_net() {
    use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let page = json!({"folio": "2", "type": "bus_topology", "title": "Network"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    d.settle();
    let switch = place_box(&mut d, "EW-GEN-ESW-5", (40, 15), (60, 22));
    let sockets: Vec<&str> = switch.iter().filter_map(|l| l.split(':').nth(1)).collect();
    assert_eq!(
        sockets,
        ["P1", "P2", "P3", "P4", "P5"],
        "the switch stands with its sockets"
    );
    let a = place_box(&mut d, "EW-GEN-PND-1", (15, 45), (30, 52));
    let b = place_box(&mut d, "EW-GEN-PND-1", (65, 45), (80, 52));
    assert_eq!(a.len(), 2, "a device with two sockets: {a:?}");
    // The CPU stands as its interfaces: X1's one socket is its PROFINET port's P1.
    let cpu = place_box(&mut d, "EW-GEN-CPU-2", (40, 62), (50, 70));
    let x1: Vec<String> = cpu.iter().filter(|l| l.ends_with(":P1")).cloned().collect();
    // The bus lines: CPU to switch, switch to each device.
    d.input(&key("W"));
    d.settle();
    for (from, to) in [(&x1[0], &switch[0]), (&switch[1], &a[0]), (&switch[2], &b[0])] {
        press_cp(&mut d, from);
        press_cp(&mut d, to);
    }
    d.input(&key("Escape"));
    d.settle();
    let _ = d.shell_mut().dispatch("rail.show", &json!({"pane": "bus"}));
    d.settle();
    let rows = d.shell_mut().shown_rows(NavKind::Bus);
    let nets: Vec<&str> = rows
        .iter()
        .filter(|r| r.role == NavRole::Net.word())
        .map(|r| r.label.as_str())
        .collect();
    assert_eq!(nets, ["PROFINET"], "one net: {rows:?}");
    let ports: Vec<String> = rows
        .iter()
        .filter(|r| r.role == NavRole::BusPort.word())
        .map(|r| r.text.clone())
        .collect();
    assert!(
        ports.iter().any(|p| p.contains("passes the net through")),
        "the switch is in the net: {ports:?}"
    );
    // The PROFINET names in the Bus navigator's Address cells, one per node: the switch has none to give (the CPU's
    // DP, in no net, is listed apart).
    let nodes: Vec<String> = rows
        .iter()
        .filter(|r| r.role == NavRole::BusPort.word() && !r.text.contains("passes the net through"))
        .filter(|r| r.key.starts_with("nets:logical/"))
        .map(|r| r.key.clone())
        .collect();
    assert_eq!(nodes.len(), 3, "the CPU and both devices are nodes");
    let _ = d
        .shell_mut()
        .dispatch("navigator.table", &json!({"kind": "bus", "on": true}));
    d.settle();
    for (row, name) in nodes.iter().zip(["plc-1", "drive-1", "drive-2"]) {
        let set = json!({"kind": "bus", "column": "Address", "value": name, "rows": [row]});
        assert_eq!(d.shell_mut().dispatch("table.set", &set), Outcome::Done, "{row}");
        d.settle();
    }
    assert_eq!(
        findings(&mut d, "EW-BUS"),
        Vec::<String>::new(),
        "a correct star is silent"
    );
}

/// Makes a part with one PROFINET interface of role device and `ports` sockets in the part editor (New part… in the
/// device browser), and saves it.
pub(crate) fn make_port_device(d: &mut Driver, number: &str, description: &str, ports: &str) {
    d.input(&key("Shift+I"));
    d.settle();
    press(d, "New part…");
    fill(d, "Manufacturer", "Acme");
    fill(d, "Part number", number);
    fill(d, "Description", description);
    press(d, "Add interface");
    fill(d, "Ports 1", ports);
    press(d, "Save");
    assert!(d.shell_mut().dialog().is_none(), "{number} is saved");
}

/// FT-1767 from nothing: a managed switch such as a SCALANCE (eight sockets, one PROFINET name for the whole switch)
/// and a drive with two sockets, each typed in the part editor, placed as boxes with their sockets beside the CPU,
/// and joined CPU → switch → drive. The switch is a node of the one net (its row has an address, it passes nothing
/// without being in the net), the drive is listed with the others, and with the three names given no EW-BUS rule
/// speaks.
#[test]
fn a_managed_switch_and_a_drive_made_in_the_window_are_nodes_of_one_net() {
    use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    make_port_device(
        &mut d,
        "SW-8M",
        "Industrial Ethernet switch, managed, 8 ports",
        "8",
    );
    make_port_device(&mut d, "DRV-1", "Frequency converter with PROFINET", "2");
    let page = json!({"folio": "2", "type": "bus_topology", "title": "Network"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    d.settle();
    let switch = place_box(&mut d, "SW-8M", (40, 15), (64, 22));
    assert_eq!(
        switch.len(),
        8,
        "the switch stands with eight sockets: {switch:?}"
    );
    let drive = place_box(&mut d, "DRV-1", (65, 45), (80, 52));
    // The CPU stands as its interfaces DP and X1; X1's one socket is its PROFINET port's P1.
    let cpu = place_box(&mut d, "EW-GEN-CPU-2", (40, 62), (50, 70));
    let x1: Vec<String> = cpu.iter().filter(|l| l.ends_with(":P1")).cloned().collect();
    d.input(&key("W"));
    d.settle();
    for (from, to) in [(&x1[0], &switch[0]), (&switch[7], &drive[0])] {
        press_cp(&mut d, from);
        press_cp(&mut d, to);
    }
    d.input(&key("Escape"));
    d.input(&key("Escape"));
    d.settle();
    let _ = d.shell_mut().dispatch("rail.show", &json!({"pane": "bus"}));
    d.settle();
    let rows = d.shell_mut().shown_rows(NavKind::Bus);
    let nets = rows.iter().filter(|r| r.role == NavRole::Net.word()).count();
    assert_eq!(nets, 1, "one net");
    // The CPU's DP, in no net, is listed apart.
    let nodes: Vec<String> = rows
        .iter()
        .filter(|r| r.role == NavRole::BusPort.word() && r.key.starts_with("nets:logical/"))
        .map(|r| r.key.clone())
        .collect();
    assert_eq!(nodes.len(), 3, "the CPU, the switch and the drive are its nodes");
    // The boxes print their address as the port symbol does: open, it is TBD (the CPU's DP too).
    let open = page_texts(&mut d).into_iter().filter(|t| t == "Node TBD").count();
    assert_eq!(
        open, 4,
        "the CPU's X1 and DP, the switch and the drive print Node TBD"
    );
    let _ = d
        .shell_mut()
        .dispatch("navigator.table", &json!({"kind": "bus", "on": true}));
    d.settle();
    for (row, name) in nodes.iter().zip(["plc-1", "switch-1", "drive-1"]) {
        let set = json!({"kind": "bus", "column": "Address", "value": name, "rows": [row]});
        assert_eq!(d.shell_mut().dispatch("table.set", &set), Outcome::Done, "{row}");
        d.settle();
    }
    // Each box prints the name it was given, and the switch prints its one name.
    let texts = page_texts(&mut d);
    for name in ["plc-1", "switch-1", "drive-1"] {
        assert!(
            texts.iter().any(|t| t == &format!("Node {name}")),
            "{name}: {texts:?}"
        );
    }
    let open = texts.iter().filter(|t| *t == "Node TBD").count();
    assert_eq!(open, 1, "no address of the net stays open, only the CPU's DP");
    assert_eq!(
        findings(&mut d, "EW-BUS"),
        Vec::<String>::new(),
        "named, the net is silent"
    );
}

/// Makes the input card "DI 8 × 24 V" in the part editor: a PLC module of eight digital inputs whose pins the pattern
/// `{n+1}` gives (the preview says channel 0 → pin 1 … channel 7 → pin 8 before Save), allowed in slots `slots`.
pub(crate) fn make_card(d: &mut Driver, number: &str, slots: &str) {
    d.input(&key("Shift+I"));
    d.settle();
    press(d, "New part…");
    fill(d, "Manufacturer", "Acme");
    fill(d, "Part number", number);
    fill(d, "Description", "Digital input card, 8 × 24 V DC");
    press(d, "PLC role");
    press(d, "Module");
    press(d, "Channel kind");
    press(d, "DI");
    fill(d, "Channels", "8");
    let preview = d
        .texts()
        .into_iter()
        .any(|t| t == "Channel 0 → pin 1 … channel 7 → pin 8");
    assert!(preview, "the pin pattern's preview: {:?}", d.texts());
    fill(d, "Allowed slots", slots);
    press(d, "Save");
    let left = match d.shell_mut().dialog() {
        Some(Dialog::PartEdit(e)) => Some((e.error.clone(), e.draft.messages.clone())),
        _ => None,
    };
    assert_eq!(left, None, "{number} is saved");
}

/// Adds the module part numbered `number` to slot `slot` of station `station` (its tag) in its Rack….
fn rack_add(d: &mut Driver, station: &str, number: &str, slot: u64) {
    let open = d
        .shell_mut()
        .dispatch("plc.rack_editor", &json!({"station": format!("tag:{station}")}));
    assert_eq!(open, Outcome::Done);
    let part = d
        .shell_mut()
        .module_parts()
        .into_iter()
        .find(|m| m.text.contains(number))
        .unwrap_or_else(|| panic!("the rack offers {number}"))
        .copy;
    let chose = d
        .shell_mut()
        .dispatch("plc.rack_part", &json!({"part": part.to_string()}));
    assert_eq!(chose, Outcome::Done);
    assert_eq!(
        d.shell_mut().dispatch("plc.rack_add", &json!({"slot": slot})),
        Outcome::Done
    );
    d.input(&key("Escape"));
    d.settle();
}

/// The texts of the PLC navigator's rows.
pub(crate) fn plc_rows(d: &mut Driver) -> Vec<String> {
    use ew_ui::ew_viewmodel::navigators::NavKind;
    let _ = d.shell_mut().dispatch("rail.show", &json!({"pane": "plc"}));
    d.settle();
    d.shell_mut()
        .shown_rows(NavKind::Plc)
        .into_iter()
        .map(|r| r.text)
        .collect()
}

/// Job 4a: a person makes the card "DI 8 × 24 V" with eight inputs and a pin pattern, saves it, and adds it as a card
/// of the starter CPU by placing it beside the CPU from the device browser (it attaches to the project's one CPU): it
/// stands with its eight pins, and its eight channels stand in the PLC navigator with their addresses, bound to them.
#[test]
fn a_card_made_in_the_window_is_a_card_of_the_starter_cpu() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    make_card(&mut d, "DI8-24", "");
    let cpu = place_box(&mut d, "EW-GEN-CPU-1", (10, 10), (20, 18));
    assert_eq!(cpu.len(), 2, "the CPU with L+ and M");
    // The card placed beside it from the device browser attaches to the project's one CPU.
    let card = place_box(&mut d, "DI8-24", (30, 10), (44, 18));
    assert_eq!(card.len(), 8, "the card stands with its eight pins: {card:?}");
    let rows = plc_rows(&mut d);
    for channel in ["I0.0", "I0.7"] {
        assert!(rows.iter().any(|r| r.contains(channel)), "{channel}: {rows:?}");
    }
    let all = records(&mut d);
    // Channel n binds pin n + 1, CP index n of the card's pattern (the pins in channel order).
    let mut pins: Vec<(u64, u64)> = all
        .iter()
        .filter(|v| v.get("module").is_some() && v.get("number").is_some())
        .filter_map(|v| Some((v["number"].as_u64()?, v["binding"]["cp"][1].as_u64()?)))
        .collect();
    pins.sort_unstable();
    assert_eq!(
        pins,
        (0..8).map(|n| (n, n)).collect::<Vec<_>>(),
        "each channel binds its pin"
    );
}

/// A person's own input card and the starter CPU, input card and remote I/O head stand under PLC and bus in the device
/// browser: what a part is to a PLC decides, not the category its drawing as a box gave it (they stood under Other).
/// The valve terminal with a bus node stays with the valve terminals.
#[test]
fn plc_parts_stand_under_plc_and_bus_in_the_device_browser() {
    use ew_ui::ew_viewmodel::tools::{Node, Trade};
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    make_card(&mut d, "DI8-24", "");
    d.input(&key("Shift+I"));
    d.settle();
    let p = d.shell_mut().picker().expect("the device browser");
    let trade = |number: &str| {
        let part = p
            .catalogue
            .parts
            .iter()
            .find(|c| c.part_number == number)
            .unwrap_or_else(|| panic!("the browser lists {number}"));
        Node::of_part(part).trade()
    };
    for number in ["DI8-24", "EW-GEN-CPU-1", "EW-GEN-DI-1", "EW-GEN-RIO-1"] {
        assert_eq!(trade(number), Some(Trade::PlcBus), "{number}");
    }
    assert_eq!(trade("EW-GEN-VTM-1"), Some(Trade::Pneumatics));
    // The tree counts them there, and nothing of a PLC is left under Other.
    let other: Vec<&str> = p
        .catalogue
        .parts
        .iter()
        .filter(|c| c.plc.is_some() && Node::of_part(c) == Node::Other)
        .map(|c| c.part_number.as_str())
        .collect();
    assert_eq!(other, Vec::<&str>::new());
}

/// Makes a head station such as an IM 152 in the part editor (New part… in the device browser) and saves it: a PLC
/// station with the supply pins L+ and M, at most 32 modules and one PROFIBUS DP slave interface `DP`.
pub(crate) fn make_head_station(d: &mut Driver, number: &str) {
    d.input(&key("Shift+I"));
    d.settle();
    press(d, "New part…");
    fill(d, "Manufacturer", "Acme");
    fill(d, "Part number", number);
    fill(d, "Description", "Head station, PROFIBUS DP");
    press(d, "PLC role");
    press(d, "Station");
    fill(d, "Supply L+ pin", "L+");
    fill(d, "Supply M pin", "M");
    fill(d, "Most modules", "32");
    // The form keeps its height and scrolls (the persona run m3-p4): the bus interfaces are below the fold.
    d.input(&ew_ui::visual::Input::Scroll {
        at: (960, 600),
        dx: 0,
        dy: 20,
        mods: egui::Modifiers::NONE,
    });
    d.settle();
    press(d, "Add interface");
    press(d, "Interface 1");
    d.input(&key("Ctrl+A"));
    d.input(&typed("DP"));
    press(d, "Bus 1");
    press(d, "PROFIBUS DP");
    press(d, "Role 1");
    press(d, "Slave");
    press(d, "Save");
    assert!(d.shell_mut().dialog().is_none(), "the head station is saved");
}

/// A head station such as an IM 152, typed from its data sheet: a PLC station with a PROFIBUS DP slave interface,
/// its supply pins and at most 32 modules; a card allowed in slots 1 to 32 goes into its slot 1, and its DP port,
/// placed from the station's row on a bus-topology page, is a slave port in the Bus navigator. Nothing depends on
/// facts the program ships about a manufacturer's product.
#[test]
fn a_head_station_made_in_the_window_takes_its_cards_and_its_dp_port() {
    use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    make_head_station(&mut d, "HEAD-DP");
    make_card(&mut d, "DI8-24", "1-32");
    let head = place_box(&mut d, "HEAD-DP", (10, 10), (20, 18));
    let tag = head[0].split(':').next().unwrap().to_owned();
    assert_eq!(head.len(), 2, "the station's supply pins L+ and M: {head:?}");
    rack_add(&mut d, &tag, "DI8-24", 1);
    let rows = plc_rows(&mut d);
    assert!(
        rows.iter().any(|r| r.contains("head station")),
        "N0 makes it a head station: {rows:?}"
    );
    assert!(
        rows.iter().any(|r| r.contains("I0.0")),
        "its card's channels: {rows:?}"
    );
    // Its DP port, from its row (Place port DP), on a bus-topology page.
    let page = json!({"folio": "2", "type": "bus_topology", "title": "Network"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    d.settle();
    let symbol = ew_ui::ew_viewmodel::ew_api::library::starter::items()
        .iter()
        .find(|i| i.key.starts_with("ew.bus.dp-port@"))
        .expect("the DP port symbol")
        .key;
    let _ = d
        .shell_mut()
        .dispatch("place.pick", &json!({"symbol": symbol, "device": tag}));
    cursor_k(&mut d, (30, 30));
    let _ = d.shell_mut().dispatch("canvas.activate", &json!({}));
    d.input(&key("Escape"));
    let _ = d.shell_mut().dispatch("rail.show", &json!({"pane": "bus"}));
    d.settle();
    let port = d
        .shell_mut()
        .shown_rows(NavKind::Bus)
        .into_iter()
        .find(|r| r.role == NavRole::BusPort.word() && r.label == format!("{tag} DP"))
        .expect("the station's DP port in the Bus navigator");
    let _ = port;
    let all = records(&mut d);
    let slave = all
        .iter()
        .any(|v| v["bus_port"]["interface"] == "DP" && v["bus_port"]["role"] == "slave");
    assert!(slave, "the port is the part's slave interface");
}

/// Item 5, the way from the symbol editor: a symbol saved to a library offers Make a part for this symbol…, which
/// opens the part editor on a new part drawn with it; saved, the part's main function is the symbol's function
/// definition, so the device browser places the part with that symbol.
#[test]
fn the_symbol_editor_makes_a_part_for_its_saved_symbol() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    press(&mut d, "Symbol");
    d.input(&typed("coil, rect"));
    d.settle();
    press(&mut d, "Copy and edit");
    press(&mut d, "Save");
    let fndef = d
        .shell_mut()
        .symbol_editor()
        .and_then(|e| e.published().map(str::to_owned))
        .expect("the symbol is saved to a library");
    press(&mut d, "Draft");
    press(&mut d, "Make a part for this symbol…");
    let drawn = draft(&mut d).drawn_with;
    assert!(drawn.is_some(), "the new part names the symbol it is drawn with");
    fill(&mut d, "Manufacturer", "Acme");
    fill(&mut d, "Part number", "RLY-24");
    // Enter in a field saves, as Save does.
    d.input(&key("Enter"));
    d.settle();
    let left = match d.shell_mut().dialog() {
        Some(Dialog::PartEdit(e)) => Some((e.error.clone(), e.draft.messages.clone())),
        _ => None,
    };
    assert_eq!(left, None, "the part is saved");
    let part = records(&mut d)
        .into_iter()
        .find(|v| v["part_number"] == "RLY-24")
        .expect("the new part");
    let entry = &part["function_template"][0];
    assert_eq!(entry["main"], json!(true));
    assert!(entry["fndef"]["item"].is_string(), "{fndef}: {entry}");
}

/// Edit part… on the part numbered `number` (as the Edit menu runs it), then a pointer move, so that the window paints
/// the dialog it opened.
pub(crate) fn edit_part(d: &mut Driver, number: &str) {
    assert_eq!(
        d.shell_mut().dispatch("part.edit", &json!({"part": number})),
        Outcome::Done
    );
    d.input(&ew_ui::visual::Input::Hover { at: (5, 600) });
    d.settle();
}

/// A saved part of one's own is corrected (the follow-up of the lane): the switch typed with eight sockets where the
/// data sheet says six is placed, then Edit part… opens the same forms with what was saved; six sockets, Save, and the
/// card of the update lists the change before Apply. The part in My parts is its next version, the next placement
/// stands with six sockets, the form shows six again, and the note says what the device already drawn keeps.
#[test]
fn a_part_of_ones_own_is_corrected_and_the_next_placement_takes_it() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    make_port_device(&mut d, "SW-8", "Industrial Ethernet switch, managed", "8");
    let page = json!({"folio": "2", "type": "bus_topology", "title": "Network"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    d.settle();
    let first = place_box(&mut d, "SW-8", (40, 15), (64, 22));
    assert_eq!(first.len(), 8);
    edit_part(&mut d, "SW-8");
    let held = draft(&mut d);
    assert!(held.own, "a part of My parts is the person's own");
    assert_eq!(held.bus[0].ports, "8", "the form shows what was saved");
    let said = d
        .texts()
        .into_iter()
        .any(|t| t.starts_with("Devices that use this part: 1."));
    assert!(said, "the note says what the drawn device keeps");
    press(&mut d, "Ports 1");
    d.input(&key("Ctrl+A"));
    d.input(&typed("6"));
    d.settle();
    press(&mut d, "Save");
    assert!(
        d.shell_mut().properties().card().is_some(),
        "the update's card lists the change"
    );
    assert_eq!(d.shell_mut().dispatch("preview.apply", &json!({})), Outcome::Done);
    d.settle();
    let second = place_box(&mut d, "SW-8", (40, 40), (64, 47));
    assert_eq!(second.len(), 6, "the next placement has six sockets: {second:?}");
    edit_part(&mut d, "SW-8");
    assert_eq!(draft(&mut d).bus[0].ports, "6", "the form shows the correction");
    let mine = tmp.0.join("data").join("libraries").join("user-parts.ewlib");
    assert!(
        library_text(&mine).contains("\"ports\":6"),
        "My parts holds the next version"
    );
}

/// A starter part is not the person's to change: its editor says so, with the way out, and Copy as a part of your
/// own opens a new part with its interfaces, which Save keeps under a number of its own.
#[test]
fn a_starter_part_is_copied_as_a_part_of_ones_own() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let page = json!({"folio": "2", "type": "bus_topology", "title": "Network"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    d.settle();
    place_box(&mut d, "EW-GEN-ESW-5", (40, 15), (60, 22));
    edit_part(&mut d, "EW-GEN-ESW-5");
    assert!(!draft(&mut d).own);
    let said = d
        .texts()
        .into_iter()
        .any(|t| t.starts_with("This part belongs to ElektraWatt starter parts, which you cannot change"));
    assert!(said, "the reason in words");
    press(&mut d, "Copy as a part of your own…");
    let copy = draft(&mut d);
    assert!(copy.new, "a new part");
    assert_eq!(copy.bus[0].ports, "5", "with the starter part's interface");
    press(&mut d, "Part number");
    d.input(&key("Ctrl+A"));
    d.input(&typed("ESW-5-OWN"));
    d.settle();
    press(&mut d, "Save");
    assert!(d.shell_mut().dialog().is_none(), "saved under its own number");
    let part = records(&mut d)
        .into_iter()
        .find(|v| v["part_number"] == "ESW-5-OWN")
        .expect("the copy");
    assert_eq!(part["sections"]["bus"]["ports"][0]["ports"], json!(5));
}
