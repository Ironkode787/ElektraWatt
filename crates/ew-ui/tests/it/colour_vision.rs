// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The colour-vision test of WP-S2.08 D15 (acceptance check AX-6, NFR-A11Y-03, FR-UI-025; doc 16 §15.6): the four
//! highlight slots and the selection stay at least ΔE00 10 apart, pair by pair, as protanopes, deuteranopes and
//! tritanopes see them (Machado et al. 2009, severity 1.0); the CIEDE2000 code meets Sharma, Wu and Dalal's test pairs;
//! the slots' hatch angles are distinct, so a weak colour pair is still told apart by its pattern.
//!
//! The renders of folio 103 with 24VDC-1, 0V, PE and 24VDC-2 in slots 1 to 4 and of folio 102 with 24VDC-1 and 0V, at
//! 150 dpi through `ew-render-cpu` and under each simulation, are written for review with `EW_BLESS=1` under
//! `tests/expected/canvas/cvd/` and never compared (D15: not gated).

use crate::support;

use std::path::PathBuf;

use ew_ui::ew_viewmodel::highlight::{SLOT_ANGLES, SLOT_COLOURS};
use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
use ew_ui::ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::{Value, json};
use support::Tmp;
use support::colour::{Simulation, ciede2000, from_linear, lab, to_linear};

/// `canvas.selection`, `volt-600` (`ew-render2d`'s overlay accent, copied as `overlays.rs` does).
const SELECTION: [u8; 3] = [0x0B, 0x63, 0xCE];

/// The threshold of doc 16 §15.6 between any two colours shown together.
const LEAST_DELTA_E: f64 = 10.0;

/// Sharma, Wu and Dalal 2005, table 1: the 34 pairs (L*, a*, b*) and their ΔE00 at four decimals.
const SHARMA: [([f64; 3], [f64; 3], f64); 34] = [
    ([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485], 2.0425),
    ([50.0, 3.1571, -77.2803], [50.0, 0.0, -82.7485], 2.8615),
    ([50.0, 2.8361, -74.0200], [50.0, 0.0, -82.7485], 3.4412),
    ([50.0, -1.3802, -84.2814], [50.0, 0.0, -82.7485], 1.0000),
    ([50.0, -1.1848, -84.8006], [50.0, 0.0, -82.7485], 1.0000),
    ([50.0, -0.9009, -85.5211], [50.0, 0.0, -82.7485], 1.0000),
    ([50.0, 0.0, 0.0], [50.0, -1.0, 2.0], 2.3669),
    ([50.0, -1.0, 2.0], [50.0, 0.0, 0.0], 2.3669),
    ([50.0, 2.4900, -0.0010], [50.0, -2.4900, 0.0009], 7.1792),
    ([50.0, 2.4900, -0.0010], [50.0, -2.4900, 0.0010], 7.1792),
    ([50.0, 2.4900, -0.0010], [50.0, -2.4900, 0.0011], 7.2195),
    ([50.0, 2.4900, -0.0010], [50.0, -2.4900, 0.0012], 7.2195),
    ([50.0, -0.0010, 2.4900], [50.0, 0.0009, -2.4900], 4.8045),
    ([50.0, -0.0010, 2.4900], [50.0, 0.0010, -2.4900], 4.8045),
    ([50.0, -0.0010, 2.4900], [50.0, 0.0011, -2.4900], 4.7461),
    ([50.0, 2.5, 0.0], [50.0, 0.0, -2.5], 4.3065),
    ([50.0, 2.5, 0.0], [73.0, 25.0, -18.0], 27.1492),
    ([50.0, 2.5, 0.0], [61.0, -5.0, 29.0], 22.8977),
    ([50.0, 2.5, 0.0], [56.0, -27.0, -3.0], 31.9030),
    ([50.0, 2.5, 0.0], [58.0, 24.0, 15.0], 19.4535),
    ([50.0, 2.5, 0.0], [50.0, 3.1736, 0.5854], 1.0000),
    ([50.0, 2.5, 0.0], [50.0, 3.2972, 0.0], 1.0000),
    ([50.0, 2.5, 0.0], [50.0, 1.8634, 0.5757], 1.0000),
    ([50.0, 2.5, 0.0], [50.0, 3.2592, 0.3350], 1.0000),
    ([60.2574, -34.0099, 36.2677], [60.4626, -34.1751, 39.4387], 1.2644),
    ([63.0109, -31.0961, -5.8663], [62.8187, -29.7946, -4.0864], 1.2630),
    ([61.2901, 3.7196, -5.3901], [61.4292, 2.2480, -4.9620], 1.8731),
    ([35.0831, -44.1164, 3.7933], [35.0232, -40.0716, 1.5901], 1.8645),
    ([22.7233, 20.0904, -46.6940], [23.0331, 14.9730, -42.5619], 2.0373),
    ([36.4612, 47.8580, 18.3852], [36.2715, 50.5065, 21.2231], 1.4146),
    ([90.8027, -2.0831, 1.4410], [91.1528, -1.6435, 0.0447], 1.4441),
    ([90.9257, -0.5406, -0.9208], [88.6381, -0.8985, -0.7239], 1.5381),
    ([6.7747, -0.2908, -2.4247], [5.8714, -0.0985, -2.2286], 0.6377),
    ([2.0776, 0.0795, -1.1350], [0.9033, -0.0636, -0.5514], 0.9082),
];

#[test]
fn ciede2000_meets_the_test_pairs_of_sharma_wu_and_dalal() {
    let four = |v: f64| (v * 1e4).round() as i64;
    for (i, (one, two, expected)) in SHARMA.iter().enumerate() {
        assert_eq!(four(ciede2000(*one, *two)), four(*expected), "pair {}", i + 1);
        assert_eq!(four(ciede2000(*two, *one)), four(*expected), "pair {} reversed", i + 1);
    }
}

#[test]
fn srgb_round_trips_through_linear_light_and_white_is_neutral() {
    for c in 0..=255u8 {
        assert_eq!(from_linear(to_linear(c)), c, "{c}");
    }
    let [l, a, b] = lab([1.0; 3]);
    assert!((l - 100.0).abs() < 1e-9 && a.abs() < 1e-9 && b.abs() < 1e-9, "{l} {a} {b}");
}

/// The four slots and the selection, named as the report names them.
fn colours() -> Vec<(String, [u8; 3])> {
    SLOT_COLOURS
        .iter()
        .enumerate()
        .map(|(i, &(r, g, b))| (format!("slot {} #{r:02X}{g:02X}{b:02X}", i + 1), [r, g, b]))
        .chain([("selection #0B63CE".to_owned(), SELECTION)])
        .collect()
}

#[test]
fn d15_every_two_of_the_slots_and_the_selection_stay_apart_under_each_simulation() {
    let colours = colours();
    let mut table = Vec::new();
    let mut under = Vec::new();
    let mut least = f64::INFINITY;
    for sim in Simulation::ALL {
        let labs: Vec<[f64; 3]> = colours.iter().map(|(_, c)| lab(sim.linear(*c))).collect();
        for i in 0..colours.len() {
            for j in i + 1..colours.len() {
                let d = ciede2000(labs[i], labs[j]);
                let line = format!("{} | {} | {} | {d:.2}", sim.word(), colours[i].0, colours[j].0);
                least = least.min(d);
                if d < LEAST_DELTA_E {
                    under.push(line.clone());
                }
                table.push(line);
            }
        }
    }
    // The table for review (shown with `--no-capture`).
    println!("{}\nleast {least:.2}", table.join("\n"));
    assert!(under.is_empty(), "pairs under ΔE00 {LEAST_DELTA_E}: {under:#?}");
    // FR-UI-025: each slot is a hatch at its own angle, so the slots stay apart without their colours.
    let mut angles = SLOT_ANGLES.to_vec();
    angles.sort_unstable();
    angles.dedup();
    assert_eq!(angles.len(), SLOT_ANGLES.len(), "{SLOT_ANGLES:?}");
}

fn done(shell: &mut ShellModel, action: &str, params: Value) {
    assert_eq!(
        shell.dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

/// The resolution of the renders (doc 16 §15.6).
const DPI: f64 = 150.0;

/// A copy of Golden A open on `folio` in a headless shell whose pane is the sheet at 150 dpi (ppp 1, zoom 150/96).
fn at_150_dpi(tmp: &Tmp, folio: &str) -> ShellModel {
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    done(
        &mut shell,
        "file.open",
        json!({"dir": dir.to_string_lossy(), "read_only": false}),
    );
    done(&mut shell, "page.goto", json!({ "folio": folio }));
    let (w, h) = shell.canvas().unwrap().sheet();
    let px = |nm: i64| (nm as f64 / 25_400_000.0 * DPI).round();
    done(
        &mut shell,
        "view.canvas_resize",
        json!({"width": px(w), "height": px(h), "ppp": 1.0}),
    );
    done(&mut shell, "view.fit_page", json!({}));
    done(&mut shell, "view.zoom_to", json!({"percent": DPI / 96.0 * 100.0}));
    shell
}

/// Highlights the potentials `names` from the Potentials navigator, in slots 1, 2, … in that order.
fn highlight(shell: &mut ShellModel, names: &[&str]) {
    done(shell, "navigator.show", json!({"kind": "potentials"}));
    for name in names {
        let key = shell
            .nav(NavKind::Potentials)
            .and_then(|n| n.table())
            .and_then(|t| {
                t.nodes
                    .iter()
                    .find(|n| n.role == NavRole::Potential && n.label == *name)
                    .map(|n| n.key.clone())
            })
            .unwrap_or_else(|| panic!("no potential row {name}"));
        done(
            shell,
            "navigator.highlight",
            json!({"kind": "potentials", "key": key}),
        );
    }
}

// Program boundary of the test binary: the bless switch of the repository's tests (`EW_BLESS=1`).
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

#[test]
fn d15_the_highlight_renders_under_each_simulation_are_written_for_review() {
    let dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/expected/canvas/cvd"));
    for (folio, names) in [
        ("103", &["24VDC-1", "0V", "PE", "24VDC-2"][..]),
        ("102", &["24VDC-1", "0V"][..]),
    ] {
        let tmp = Tmp::new();
        let mut shell = at_150_dpi(&tmp, folio);
        highlight(&mut shell, names);
        let slots: Vec<(u8, &str)> = shell
            .highlights()
            .items()
            .iter()
            .map(|h| (h.slot, h.name.as_str()))
            .collect();
        let expected: Vec<(u8, &str)> = (0..).zip(names.iter().copied()).collect();
        assert_eq!(slots, expected, "{folio}: the potentials in slots 1 to {}", names.len());
        assert_eq!(
            shell.highlight_overlay(0).len(),
            names.len(),
            "{folio}: every highlight is drawn"
        );
        let (image, dpi) = ew_ui::canvas::snapshot_image(&shell, 0).expect("a snapshot");
        assert_eq!(dpi, 150, "{folio}");
        let (w, h) = shell.canvas().unwrap().sheet();
        let px = |nm: i64| (nm as f64 / 25_400_000.0 * DPI).round() as u32;
        assert_eq!((image.width, image.height), (px(w), px(h)), "{folio}: the sheet");
        for sim in Simulation::ALL {
            let mut seen = image.clone();
            sim.apply(&mut seen.pixels);
            assert_ne!(seen.pixels, image.pixels, "{folio} {}: the hues change", sim.word());
            if bless() {
                std::fs::create_dir_all(&dir).unwrap();
                let path = dir.join(format!("{folio}-{}.png", sim.word()));
                std::fs::write(&path, ew_render_cpu::encode_png(&seen, Some(150))).unwrap();
            }
        }
        shell.shutdown();
    }
}
