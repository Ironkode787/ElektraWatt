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
        (row.interface.as_str(), row.bus.as_str(), row.role.as_str(), row.ports.as_str()),
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
        part["description"].to_string().contains("Ethernet switch, unmanaged, 5 ports"),
        "{}",
        part["description"]
    );
    let port = &part["sections"]["bus"]["ports"][0];
    assert_eq!((port["role"].as_str(), port["ports"].as_u64()), (Some("passive"), Some(5)));
    let toast = d.shell_mut().toast().map(|t| t.text.clone());
    assert_eq!(toast.as_deref(), Some("Part SW-5 is in the project and in My parts."));
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
    let _ = d.shell_mut().dispatch("cursor.set", &json!({"x_mm": x, "y_mm": y}));
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
    let _ = d.shell_mut().dispatch("cursor.set", &json!({"x_mm": x, "y_mm": y}));
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
fn findings(d: &mut Driver, prefix: &str) -> Vec<String> {
    use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
    press(d, "Check project");
    let _ = d.shell_mut().dispatch("navigator.show", &json!({"kind": "messages"}));
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
/// boxes with every socket (P1 … P5, P1 and P2); the CPU's X1 is placed from its row. Bus lines from the CPU to the
/// switch and from the switch to each device make one PROFINET net of the CPU and both devices, which the switch
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
    assert_eq!(sockets, ["P1", "P2", "P3", "P4", "P5"], "the switch stands with its sockets");
    let a = place_box(&mut d, "EW-GEN-PND-1", (15, 45), (30, 52));
    let b = place_box(&mut d, "EW-GEN-PND-1", (65, 45), (80, 52));
    assert_eq!(a.len(), 2, "a device with two sockets: {a:?}");
    let cpu = place_box(&mut d, "EW-GEN-CPU-2", (40, 62), (60, 70));
    let tag = cpu[0].split(':').next().unwrap().to_owned();
    // The CPU's X1, as its row in the PLC navigator places it (Place port X1).
    let symbol = ew_ui::ew_viewmodel::ew_api::library::starter::items()
        .iter()
        .find(|i| i.key.starts_with("ew.bus.pn-port@"))
        .expect("the PROFINET port symbol")
        .key;
    let before = cp_labels(&mut d);
    assert_eq!(
        d.shell_mut().dispatch("place.pick", &json!({"symbol": symbol, "device": tag})),
        Outcome::Done
    );
    cursor_k(&mut d, (75, 66));
    let _ = d.shell_mut().dispatch("canvas.activate", &json!({}));
    d.input(&key("Escape"));
    d.settle();
    let x1: Vec<String> = cp_labels(&mut d).into_iter().filter(|l| !before.contains(l)).collect();
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
    // The PROFINET names in the Bus navigator's Address cells, one per node: the switch has none to give.
    let nodes: Vec<String> = rows
        .iter()
        .filter(|r| r.role == NavRole::BusPort.word() && !r.text.contains("passes the net through"))
        .map(|r| r.key.clone())
        .collect();
    assert_eq!(nodes.len(), 3, "the CPU and both devices are nodes");
    let _ = d.shell_mut().dispatch("navigator.table", &json!({"kind": "bus", "on": true}));
    d.settle();
    for (row, name) in nodes.iter().zip(["plc-1", "drive-1", "drive-2"]) {
        let set = json!({"kind": "bus", "column": "Address", "value": name, "rows": [row]});
        assert_eq!(d.shell_mut().dispatch("table.set", &set), Outcome::Done, "{row}");
        d.settle();
    }
    assert_eq!(findings(&mut d, "EW-BUS"), Vec::<String>::new(), "a correct star is silent");
}

/// Makes a part with one PROFINET interface of role device and `ports` sockets in the part editor (New part… in the
/// device browser), and saves it.
fn make_port_device(d: &mut Driver, number: &str, description: &str, ports: &str) {
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
    make_port_device(&mut d, "SW-8M", "Industrial Ethernet switch, managed, 8 ports", "8");
    make_port_device(&mut d, "DRV-1", "Frequency converter with PROFINET", "2");
    let page = json!({"folio": "2", "type": "bus_topology", "title": "Network"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    d.settle();
    let switch = place_box(&mut d, "SW-8M", (40, 15), (64, 22));
    assert_eq!(switch.len(), 8, "the switch stands with eight sockets: {switch:?}");
    let drive = place_box(&mut d, "DRV-1", (65, 45), (80, 52));
    let cpu = place_box(&mut d, "EW-GEN-CPU-2", (40, 62), (60, 70));
    let tag = cpu[0].split(':').next().unwrap().to_owned();
    let symbol = ew_ui::ew_viewmodel::ew_api::library::starter::items()
        .iter()
        .find(|i| i.key.starts_with("ew.bus.pn-port@"))
        .expect("the PROFINET port symbol")
        .key;
    let before = cp_labels(&mut d);
    let _ = d.shell_mut().dispatch("place.pick", &json!({"symbol": symbol, "device": tag}));
    cursor_k(&mut d, (75, 66));
    let _ = d.shell_mut().dispatch("canvas.activate", &json!({}));
    d.input(&key("Escape"));
    d.settle();
    let x1: Vec<String> = cp_labels(&mut d).into_iter().filter(|l| !before.contains(l)).collect();
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
    let nodes: Vec<String> = rows
        .iter()
        .filter(|r| r.role == NavRole::BusPort.word())
        .map(|r| r.key.clone())
        .collect();
    assert_eq!(nodes.len(), 3, "the CPU, the switch and the drive are its nodes");
    assert!(nodes.iter().all(|k| k.starts_with("nets:logical/")), "all in the net: {nodes:?}");
    let _ = d.shell_mut().dispatch("navigator.table", &json!({"kind": "bus", "on": true}));
    d.settle();
    for (row, name) in nodes.iter().zip(["plc-1", "switch-1", "drive-1"]) {
        let set = json!({"kind": "bus", "column": "Address", "value": name, "rows": [row]});
        assert_eq!(d.shell_mut().dispatch("table.set", &set), Outcome::Done, "{row}");
        d.settle();
    }
    assert_eq!(findings(&mut d, "EW-BUS"), Vec::<String>::new(), "named, the net is silent");
}
