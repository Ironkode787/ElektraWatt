// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.08 part 2 (keymaps, D6 to D8) headless: KM-1 the preset files equal what the registry generates and the
//! KiCad-like preset binds W, E, A and G on the canvas; KM-2 the user's `keymap.toml` replaces the preset's bindings of
//! the actions it names, keeps unknown actions as unused, and a file with a conflict is refused with a banner while the
//! preset applies; KM-3 the editor records, refuses a conflict, lists shadowing, resets and exports a file that imports
//! back equal; KM-4 the reachability of both presets on QWERTY-US, QWERTZ-DE and AZERTY-FR; KM-5 Ctrl+Shift+N is
//! Renumber devices…; KM-6 the keymap sheet. `EW_BLESS=1` rewrites the generated files:
//!
//! ```text
//! EW_BLESS=1 cargo nextest run -p ew-viewmodel --test keymaps
//! ```

mod common;

use std::path::{Path, PathBuf};

use common::{Tmp, bless, done, refused};
use ew_viewmodel::actions::{self, Context};
use ew_viewmodel::keymap::{self, Chord, Finding, Keymap, Layout, Preset, Severity};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::json;

const BLESS_HINT: &str = "EW_BLESS=1 cargo nextest run -p ew-viewmodel --test keymaps";

fn keymaps_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("keymaps")
}

/// The committed file `name` under `keymaps/` equals `text`; `EW_BLESS=1` rewrites it.
fn assert_file(name: &str, text: &str) {
    let path = keymaps_dir().join(name);
    if bless() {
        std::fs::create_dir_all(keymaps_dir()).unwrap();
        std::fs::write(&path, text).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("keymaps/{name} is missing; run {BLESS_HINT}"))
        .replace("\r\n", "\n");
    assert!(committed == text, "keymaps/{name} is stale; run {BLESS_HINT}");
}

/// The text of a preset's file: the licence header, then the keymap.
fn preset_file(preset: Preset) -> String {
    format!(
        "# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors\n# SPDX-License-Identifier: GPL-3.0-or-later\n\n\
         # The {} keymap preset (WP-S2.08 D6), generated from the action registry by ew-viewmodel's `keymaps` test.\n\
         # Do not edit: change the registry (or the preset's rule in keymap.rs) and run {BLESS_HINT}.\n\n{}",
        preset.name(),
        preset.keymap().to_toml()
    )
}

fn chord(text: &str) -> Chord {
    Chord::parse(text).unwrap()
}

/// KM-1: `keymaps/ew.toml` equals what the registry generates, `keymaps/kicad.toml` parses into the KiCad-like preset.
#[test]
fn the_preset_files_equal_the_registry() {
    for preset in Preset::ALL {
        let text = preset_file(preset);
        assert_file(&format!("{}.toml", preset.word()), &text);
        let parsed = Keymap::parse(&text).unwrap();
        assert_eq!(parsed, preset.keymap(), "{preset:?}");
        assert_eq!(parsed.name, preset.name());
        assert!(parsed.unused().is_empty());
    }
    // The file writes punctuation by physical key (D6): `]` is `phys:BracketRight`.
    let ew = std::fs::read_to_string(keymaps_dir().join("ew.toml")).unwrap();
    assert!(
        ew.contains("keys = \"phys:BracketRight\"\naction = \"cp.next\""),
        "{ew}"
    );
}

/// KM-1 (FR-UI-013): with the KiCad-like preset W starts a wire, E focuses Properties, A starts Place and G Move on
/// the canvas; M, R, W, E, X, Y, T and Home are equal in both presets.
#[test]
fn the_kicad_preset_binds_w_e_a_and_g_on_the_canvas() {
    let tmp = Tmp::new("keymap-kicad");
    let mut shell = shell_in(&tmp.0.join("config"));
    shell.start_first_run_if_needed();
    done(
        &mut shell,
        "first_run.set",
        json!({"field": "keymap", "value": "kicad"}),
    );
    assert_eq!(shell.keymap_preset(), Preset::Kicad);
    let bound = shell.bindings();
    let has = |keys: &str, action: &str| {
        bound
            .iter()
            .any(|b| b.chord == chord(keys) && b.action == action && b.context == Context::Canvas)
    };
    for (keys, action) in [
        ("W", "tool.wire"),
        ("E", "props.focus"),
        ("A", "tool.place"),
        ("G", "tool.move"),
        ("M", "tool.move"),
    ] {
        assert!(has(keys, action), "{keys} {action}");
    }
    assert!(!has("I", "tool.place"), "A replaces I");
    for keys in ["M", "R", "W", "E", "X", "Y", "T", "Home"] {
        let of = |p: Preset| {
            keymap::effective(p, None)
                .into_iter()
                .filter(|b| b.chord == chord(keys))
                .map(|b| (b.action, b.context))
                .collect::<Vec<_>>()
        };
        assert_eq!(of(Preset::Ew), of(Preset::Kicad), "{keys}");
        assert!(!of(Preset::Ew).is_empty(), "{keys} is bound");
    }
    // The menus show the preset's key: Place reads A.
    let place = actions::find("tool.place").unwrap();
    assert_eq!(shell.shortcut_text(place), "A");
}

fn shell_in(dir: &Path) -> ShellModel {
    ShellModel::new(Mode::Gui, ShellConfig::in_dir(dir, "0.0.0-test"))
}

/// KM-2: the user's file replaces the preset's binding of `tool.wire`, keeps an unknown action as unused, and a file
/// with two equal chords in one context is refused with a banner that names them; the preset applies.
#[test]
fn the_user_keymap_replaces_keeps_unknown_actions_and_a_conflict_is_refused() {
    let tmp = Tmp::new("keymap-user");
    let dir = tmp.0.join("config");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(keymap::USER_FILE),
        "[meta]\nname = \"Mine\"\nversion = 1\n\n[[bind]]\nkeys = \"Ctrl+Shift+W\"\naction = \"tool.wire\"\n\
         context = \"canvas\"\n\n[[bind]]\nkeys = \"Ctrl+J\"\naction = \"future.action\"\ncontext = \"global\"\n",
    )
    .unwrap();
    let shell = shell_in(&dir);
    assert_eq!(shell.banner(), None);
    let wire = actions::find("tool.wire").unwrap();
    assert_eq!(shell.shortcuts_of(wire), ["Ctrl+Shift+W"]);
    let bound = shell.bindings();
    assert!(
        bound
            .iter()
            .any(|b| b.action == "tool.wire" && b.chord == chord("Ctrl+Shift+W"))
    );
    assert!(
        !bound
            .iter()
            .any(|b| b.action == "tool.wire" && b.chord == chord("W"))
    );
    let unused = shell.user_keymap().unwrap().unused();
    assert_eq!(unused.len(), 1);
    assert_eq!(unused[0].action, "future.action");

    // Two equal chords in one context: refused, the banner names the chord, the file is kept aside.
    std::fs::write(
        dir.join(keymap::USER_FILE),
        "[meta]\nname = \"Bad\"\nversion = 1\n\n[[bind]]\nkeys = \"Ctrl+J\"\naction = \"file.open\"\ncontext = \"global\"\n\n\
         [[bind]]\nkeys = \"Ctrl+J\"\naction = \"file.save\"\ncontext = \"global\"\n",
    )
    .unwrap();
    let shell = shell_in(&dir);
    let banner = shell.banner().expect("a banner").to_owned();
    assert!(
        banner.contains("Ctrl+J is bound twice in the global context"),
        "{banner}"
    );
    assert!(banner.contains("keymap.toml.bad-1"), "{banner}");
    assert!(dir.join("keymap.toml.bad-1").exists());
    assert!(shell.user_keymap().is_none(), "the preset applies");
    let open = actions::find("file.open").unwrap();
    assert_eq!(shell.shortcuts_of(open), ["Ctrl+O"]);

    // A file that does not parse is refused the same way.
    std::fs::write(
        dir.join(keymap::USER_FILE),
        "[meta]\nversion = 1\n[[bind]]\nkeys = \"Ctrl+Nope\"\naction = \"file.open\"\ncontext = \"global\"\n",
    )
    .unwrap();
    let shell = shell_in(&dir);
    assert!(
        shell.banner().is_some_and(|b| b.contains("Ctrl+Nope")),
        "{:?}",
        shell.banner()
    );
}

/// KM-3: the editor records a chord, shows a conflict as an error and a canvas-over-global binding as shadowing,
/// resets one action, and exports a file that imports back equal.
#[test]
fn the_editor_records_refuses_shadows_resets_and_round_trips() {
    let tmp = Tmp::new("keymap-editor");
    let dir = tmp.0.join("config");
    let mut shell = shell_in(&dir);
    // The editor's actions need the dialog.
    assert_eq!(
        refused(&mut shell, "keymap.search", json!({"text": "wire"})),
        "reason-no-keymap-editor"
    );
    done(&mut shell, "help.shortcuts", json!({}));
    done(&mut shell, "keymap.search", json!({"text": "wire"}));
    assert!(shell.keymap_rows().iter().all(|r| keymap::row_matches(r, "wire")));
    assert!(shell.keymap_rows().iter().any(|r| r.action == "tool.wire"));

    // Record: Escape ends a recording and leaves the dialog open.
    done(&mut shell, "keymap.record_start", json!({"action": "tool.wire"}));
    assert_eq!(shell.keymap_editor().recording, Some("tool.wire"));
    done(&mut shell, "ui.dismiss", json!({}));
    assert_eq!(shell.keymap_editor().recording, None);
    assert!(shell.keymap_editor_open());

    // A recorded chord over a global one is shadowing: set, and listed as an info on the row.
    done(&mut shell, "keymap.record_start", json!({"action": "tool.wire"}));
    done(
        &mut shell,
        "keymap.record",
        json!({"action": "tool.wire", "keys": "Ctrl+Shift+W"}),
    );
    assert_eq!(shell.keymap_editor().recording, None);
    let row = shell
        .keymap_rows()
        .into_iter()
        .find(|r| r.action == "tool.wire")
        .unwrap();
    assert_eq!(row.chords, ["Ctrl+Shift+W"]);
    assert!(row.custom);
    assert_eq!(
        row.notes,
        ["Ctrl+Shift+W in the canvas context shadows Close project"]
    );
    assert!(dir.join(keymap::USER_FILE).exists(), "written at once");

    // A conflict is an error: refused, listed, nothing changes.
    let reason = refused(
        &mut shell,
        "keymap.record",
        json!({"action": "tool.wire", "keys": "M"}),
    );
    assert_eq!(reason, "reason-keymap-conflict");
    let refusal = shell.keymap_editor().refusal.clone().unwrap();
    assert_eq!(refusal.action, "tool.wire");
    assert_eq!(refusal.chord, "M");
    assert_eq!(
        refusal.errors,
        ["M is bound twice in the canvas context: Move, Connection"]
    );
    let wire = actions::find("tool.wire").unwrap();
    assert_eq!(shell.shortcuts_of(wire), ["Ctrl+Shift+W"]);
    // Ctrl+Alt and the reserved chords are errors too.
    refused(
        &mut shell,
        "keymap.record",
        json!({"action": "tool.wire", "keys": "Ctrl+Alt+W"}),
    );
    refused(
        &mut shell,
        "keymap.record",
        json!({"action": "tool.wire", "keys": "Alt+F4"}),
    );

    // Export, change, import: the bindings come back equal.
    let before = shell.bindings();
    let file = tmp.0.join("exported.toml");
    done(
        &mut shell,
        "keymap.export",
        json!({"path": file.to_string_lossy()}),
    );
    done(
        &mut shell,
        "keymap.record",
        json!({"action": "file.open", "keys": "Ctrl+Shift+O"}),
    );
    assert_ne!(shell.bindings(), before);
    done(
        &mut shell,
        "keymap.import",
        json!({"path": file.to_string_lossy()}),
    );
    assert_eq!(shell.bindings(), before);
    assert_eq!(
        keymap::Keymap::parse(&std::fs::read_to_string(&file).unwrap()).unwrap(),
        *shell.user_keymap().unwrap()
    );

    // Reset one action: the preset's binding is back, the others stay.
    done(
        &mut shell,
        "keymap.record",
        json!({"action": "file.open", "keys": "Ctrl+Shift+O"}),
    );
    done(&mut shell, "keymap.reset", json!({"action": "tool.wire"}));
    assert_eq!(shell.shortcuts_of(wire), ["W"]);
    let open = actions::find("file.open").unwrap();
    assert_eq!(shell.shortcuts_of(open), ["Ctrl+Shift+O"]);
    // The written file is what a new shell loads.
    let again = shell_in(&dir);
    assert_eq!(again.bindings(), shell.bindings());

    // An import with a conflict fails and changes nothing.
    let bad = tmp.0.join("bad.toml");
    std::fs::write(
        &bad,
        "[meta]\nname = \"Bad\"\nversion = 1\n[[bind]]\nkeys = \"W\"\naction = \"tool.move\"\ncontext = \"canvas\"\n",
    )
    .unwrap();
    let outcome = shell.dispatch("keymap.import", &json!({"path": bad.to_string_lossy()}));
    assert!(
        matches!(&outcome, ew_viewmodel::shell::Outcome::Failed { message } if message.contains("W is bound twice")),
        "{outcome:?}"
    );
    assert_eq!(shell.shortcuts_of(open), ["Ctrl+Shift+O"]);
}

/// The preset bindings that D8's reachability rule flags today, listed for the orchestrator before any binding changes
/// (spec §11.2 item 3, R4): each with its proposed fix. Logical `Plus`, `Equals` and `0` need Shift on some layouts.
const PENDING_D8: &[(&str, &str, &str)] = &[
    // Ctrl+Plus types Shift+= on US and AZERTY (zoom in also has Ctrl+Equals, which types there).
    ("needs-shift", "Ctrl+Plus", "QWERTY-US"),
    // `=` is Shift+0 on QWERTZ (zoom in also has Ctrl+Plus, which types there).
    ("needs-shift", "Ctrl+Equals", "QWERTZ-DE"),
    ("needs-shift", "Ctrl+Plus", "AZERTY-FR"),
    // `0` is Shift+à on AZERTY, and Ctrl+0 is zoom to 100 %'s only binding: proposed fix `Ctrl+phys:Digit0`.
    ("needs-shift", "Ctrl+0", "AZERTY-FR"),
    ("unreachable", "view.zoom_actual", "AZERTY-FR"),
];

fn pending_word(f: &Finding) -> (String, String, String) {
    match f {
        Finding::NeedsShift { chord, layout, .. } => {
            ("needs-shift".to_owned(), chord.text(), layout.name().to_owned())
        }
        Finding::Unreachable { action, layout } => (
            "unreachable".to_owned(),
            (*action).to_owned(),
            layout.name().to_owned(),
        ),
        other => panic!("not a reachability finding: {other:?}"),
    }
}

/// KM-4 (NFR-I18N-06): for each preset and each layout every bound action has a typable binding, apart from the
/// listed pending bindings; neither preset has a conflict, a Ctrl+Alt chord or a reserved chord.
#[test]
fn every_preset_action_is_reachable_on_the_three_layouts() {
    for preset in Preset::ALL {
        let bound = keymap::effective(preset, None);
        let errors: Vec<Finding> = keymap::conflicts(&bound)
            .into_iter()
            .filter(|f| f.severity() == Severity::Error)
            .collect();
        assert!(errors.is_empty(), "{preset:?}: {errors:?}");
        let found: Vec<(String, String, String)> =
            keymap::reachability(&bound).iter().map(pending_word).collect();
        let expected: Vec<(String, String, String)> = PENDING_D8
            .iter()
            .map(|(a, b, c)| ((*a).to_owned(), (*b).to_owned(), (*c).to_owned()))
            .collect();
        assert_eq!(found, expected, "{preset:?}");
    }
    // The three layouts are the ones of NFR-I18N-06.
    assert_eq!(
        Layout::ALL.map(Layout::name),
        ["QWERTY-US", "QWERTZ-DE", "AZERTY-FR"]
    );
    // The Ctrl+Alt ban and the reserved chords refuse a user keymap.
    for keys in ["Ctrl+Alt+P", "Ctrl+Alt+F5", "Alt+F4", "Alt+Tab", "Win+D"] {
        let user = Keymap::parse(&format!(
            "[meta]\nname = \"x\"\nversion = 1\n[[bind]]\nkeys = \"{keys}\"\naction = \"file.open\"\ncontext = \"global\"\n"
        ))
        .unwrap();
        assert_eq!(keymap::user_errors(Preset::Ew, &user).len(), 1, "{keys}");
    }
}

/// KM-5 (D7): Ctrl+Shift+N is Renumber devices… in the EW preset (FR-UI-012's "renum" shows it).
#[test]
fn ctrl_shift_n_is_renumber_devices() {
    let meta = actions::find("devices.renumber").unwrap();
    assert_eq!(meta.shortcut, ["Ctrl+Shift+N"]);
    let tmp = Tmp::new("keymap-km5");
    let shell = shell_in(&tmp.0.join("config"));
    assert_eq!(shell.shortcut_text(meta), "Ctrl+Shift+N");
    let bound = keymap::effective(Preset::Ew, None);
    let on = bound
        .iter()
        .filter(|b| b.chord == chord("Ctrl+Shift+N"))
        .map(|b| b.action)
        .collect::<Vec<_>>();
    assert_eq!(on, ["devices.renumber"]);
}

/// KM-6 (NFR-DOC-06): the keymap sheet `keymaps/sheet.json` equals what the presets generate, and holds each bound
/// action once with its bindings per preset.
#[test]
fn the_keymap_sheet_equals_the_presets() {
    let text = keymap::sheet_json();
    assert_file("sheet.json", &text);
    let sheet: serde_json::Value = serde_json::from_str(&text).unwrap();
    let place = sheet["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["action"] == "tool.place")
        .unwrap();
    assert_eq!(place["bindings"]["ew"], json!(["I"]));
    assert_eq!(place["bindings"]["kicad"], json!(["A"]));
    assert_eq!(place["context"], "canvas");
    assert_eq!(place["label_key"], "action-tool-place");
}
