// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The MVP rules of the EW-PLC pack (WP-S1.21 part 3, D10, PA-4, PA-5, PA-7) and EW-CORE-008's `duplicate_number`
//! half (FT-350), on a small valve terminal: a station with two eight-channel output modules (`-1U2`, `-1U3`) whose
//! part maps solenoid 14 of the valve at position p to channel 2(p − 1) and solenoid 12 to 2(p − 1) + 1, and one
//! valve `-1Q1` at position 1 of `-1U2`, solenoid 14 bound to channel 0 as the map says.

mod common;

use common::*;
use ew_derive::ew_ids::v5;
use ew_derive::ew_schema::{Device, Kind};
use serde_json::{Value, json};

const CFG: u128 = 0x50;
const STATION: u128 = 0x1100;
const M1: u128 = 0x1101;
const M2: u128 = 0x1102;
const VALVE: u128 = 0x1201;
const COIL: u128 = 0x1301;
const VALVE_DEF: u128 = 0x520;
const VALVE_SYM: u128 = 0x521;
const PART: u128 = 0x600;

/// The record ID text of channel `n` of module `module` (the v5 ID N1 gives it).
fn ch(module: u128, n: u16) -> String {
    v5::plc_channel_id::<Device, ()>(
        ew_derive::ew_ids::Id::from_uuid(ew_derive::ew_ids::Uuid::from_u128(module)),
        n,
    )
    .as_uuid()
    .to_string()
}

/// The fixture; `meta` is the part's `meta` (its confidence marks).
fn fixture_with(meta: &Value) -> Fx {
    let mut fx = Fx::base();
    fx.fndef_with(
        VALVE_DEF,
        VALVE_SYM,
        &[
            (0, "14", json!({"physical": false})),
            (1, "12", json!({"physical": false})),
        ],
        &json!([]),
        &json!({}),
    );
    fx.part(
        PART,
        "533361",
        &json!({
            "meta": meta,
            "kind": "device",
            "sections": {"plc": {
                "role": "module",
                "io": [{"dir": "out", "bits": 8}],
                "channels": {"count": 8, "kind": "do", "data_type": "Bool", "designation": "{n}"},
                "bit_of": "n",
                "unbound": "deactivated",
                "positions": {"count": 4, "map": [
                    {"solenoid": "14", "channel": "2p"},
                    {"solenoid": "12", "channel": "2p+1"},
                ]},
            }},
        }),
    );
    fx.push(json!({"$k": "plc_config_project", "id": us(CFG), "name": "PLC1", "profile": "siemens-tia-intl@2027.1"}));
    // drawn devices, so their tags are complete (`-1U2`, not `-?U2`)
    fx.drawn_device(STATION, "U", Some(1), P1);
    rec(&mut fx, &us(STATION))["plc_station"] = json!({"config_project": us(CFG), "role": "valve_terminal"});
    for (n, pos) in [(M1, 2), (M2, 3)] {
        fx.drawn_device(n, "U", Some(pos), P1);
        let r = rec(&mut fx, &us(n));
        r["parent"] = json!(us(STATION));
        r["position"] = json!(pos - 1);
        r["plc_module"] = json!({});
        fx.with_parts(n, &[PART]);
    }
    fx.device(VALVE, "Q", Some(1))
        .with("parent", json!(us(M1)))
        .with("position", json!(1));
    fx.push(json!({"$k": "function", "id": us(COIL), "device": us(VALVE), "def": us(VALVE_DEF)}));
    fx.push(json!({
        "$k": "representation", "id": us(COIL + 0x10_000), "functions": [us(COIL)], "page": us(P1),
        "symbol": us(VALVE_SYM), "pos": [0, 40_000_000],
    }));
    for module in [M1, M2] {
        for n in 0..8u16 {
            let mut c = json!({"$k": "channel", "id": ch(module, n), "module": us(module), "number": n, "kind": "do"});
            if (module, n) == (M1, 0) {
                c["binding"] = json!({"cp": [us(COIL), 0]});
            }
            fx.push(c);
        }
    }
    fx
}

fn fixture() -> Fx {
    fixture_with(&json!({}))
}

/// The record with ID text `id`.
fn rec<'a>(fx: &'a mut Fx, id: &str) -> &'a mut Value {
    fx.records
        .iter_mut()
        .find(|r| r["id"] == json!(id))
        .expect("the record")
}

const PLC_RULES: [&str; 7] = [
    "EW-PLC-001",
    "EW-PLC-002",
    "EW-PLC-003",
    "EW-PLC-007",
    "EW-PLC-008",
    "EW-PLC-012",
    "EW-PLC-015",
];

fn subjects(f: &[ew_checks::Finding]) -> Vec<(String, String)> {
    f.iter()
        .map(|f| (f.discriminator.clone(), f.subject.clone()))
        .collect()
}

#[test]
fn a_consistent_terminal_has_no_plc_finding() {
    let m = fixture().build();
    for rule in PLC_RULES {
        assert!(
            findings_of(&m, rule).is_empty(),
            "{rule}: {:#?}",
            findings_of(&m, rule)
        );
    }
    let q = ew_derive::Q::new(&m);
    let release = ew_checks::run(&q, &ew_checks::FULL, ew_checks::CheckMode::Release);
    let plc: Vec<&ew_checks::Finding> = release
        .findings
        .iter()
        .filter(|f| f.rule.starts_with("EW-PLC") || f.rule == "EW-CORE-008")
        .collect();
    assert!(plc.is_empty(), "{plc:#?}");
}

/// The fixture with channel `n` of -1U3 (`M2`) at the manual bit address `Q<byte>.<bit>`, for each `(n, byte, bit)`.
fn manual_bits(at: &[(u16, u32, u8)]) -> ew_model::Model {
    let mut fx = fixture();
    for &(n, byte, bit) in at {
        rec(&mut fx, &ch(M2, n))["address"] = json!({"manual": {"area": "q", "byte": byte, "bit": bit}});
    }
    fx.build()
}

/// EW-PLC-001: a manual address on another module's bit is one overlap with both channels as targets; a word at an
/// odd byte is an alignment warning beside its overlap group.
#[test]
fn ew_plc_001_reports_overlaps_and_word_alignment() {
    let f = findings_of(&manual_bits(&[(0, 0, 3)]), "EW-PLC-001");
    assert_eq!(
        subjects(&f),
        [("overlap".to_owned(), "-1U2:3".to_owned())],
        "{f:#?}"
    );
    assert_eq!(
        f[0].message,
        "addresses overlap in configuration project PLC1: -1U2:3 (Q0.3), -1U3:0 (Q0.3); give all but one of them \
         another address"
    );
    assert_eq!(f[0].targets.len(), 2);
    assert!(f[0].targets.iter().all(|t| t.kind == Kind::Channel));

    let mut fx = fixture();
    rec(&mut fx, &ch(M2, 0))["address"] = json!({"manual": {"area": "q", "size": "word", "byte": 1}});
    let f = findings_of(&fx.build(), "EW-PLC-001");
    let align: Vec<&ew_checks::Finding> = f.iter().filter(|f| f.discriminator == "alignment").collect();
    assert_eq!(align.len(), 1, "{f:#?}");
    assert_eq!(align[0].severity, ew_checks::Severity::Warning);
    assert_eq!(
        align[0].message,
        "-1U3:0: the address QW1 does not start at a multiple of 2 bytes, the profile's word alignment (an unverified \
         convention)"
    );
    // QW1 covers Q1.0 … Q2.7: one group of -1U3:0 and the seven other channels of -1U3 (FX38a: once, not seven pairs).
    let overlap: Vec<&ew_checks::Finding> = f.iter().filter(|f| f.discriminator == "overlap").collect();
    assert_eq!(overlap.len(), 1, "{f:#?}");
    assert_eq!(overlap[0].targets.len(), 8);
}

/// FX38a: three channels on one address are one finding with the three channels as targets.
#[test]
fn ew_plc_001_reports_a_group_of_three_once() {
    let f = findings_of(&manual_bits(&[(0, 0, 3), (1, 0, 3)]), "EW-PLC-001");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].targets.len(), 3);
    assert_eq!(f[0].subject, "-1U2:3");
    assert_eq!(
        f[0].message,
        "addresses overlap in configuration project PLC1: -1U2:3 (Q0.3), -1U3:0 (Q0.3), -1U3:1 (Q0.3); give all but \
         one of them another address"
    );
}

/// FX38a: two overlaps on different addresses are two findings.
#[test]
fn ew_plc_001_reports_separate_overlaps_apart() {
    let f = findings_of(&manual_bits(&[(0, 0, 3), (1, 0, 5)]), "EW-PLC-001");
    assert_eq!(
        subjects(&f),
        [
            ("overlap".to_owned(), "-1U2:3".to_owned()),
            ("overlap".to_owned(), "-1U2:5".to_owned())
        ],
        "{f:#?}"
    );
    assert!(f.iter().all(|f| f.targets.len() == 2), "{f:#?}");
}

/// FX38a: a group names five channels and counts the rest ("and 4 more channels"), in the rule text, the en-GB
/// pattern (equal to it, as FT-724 holds for EW-POT-001) and the nl-NL pattern; every member stays a target.
#[test]
fn ew_plc_001_names_five_channels_and_counts_the_rest() {
    let all: Vec<(u16, u32, u8)> = (0..8).map(|n| (n, 0, 3)).collect();
    let f = findings_of(&manual_bits(&all), "EW-PLC-001");
    assert_eq!(f.len(), 1, "{f:#?}");
    let f = &f[0];
    assert_eq!(f.targets.len(), 9);
    assert_eq!(f.message.matches("(Q0.3)").count(), 5, "{}", f.message);
    assert!(
        f.message
            .ends_with("; and 4 more channels; give all but one of them another address"),
        "{}",
        f.message
    );
    let nl = ew_checks::messages::render(f, "nl-NL");
    assert!(nl.contains("; en nog 4 kanalen; "), "{nl}");
    assert_eq!(nl.matches("(Q0.3)").count(), 5, "{nl}");
}

/// FX38a: EW-PLC-001's text is built in the rule and in the Fluent pattern; the en-GB pattern rendered from the
/// finding's arguments equals the rule's text for a pair, a group of three, the "and N more" cut and the alignment
/// warning, and the nl-NL pattern renders each.
#[test]
fn ew_plc_001_the_en_gb_pattern_equals_the_rule_text() {
    let all: Vec<(u16, u32, u8)> = (0..8).map(|n| (n, 0, 3)).collect();
    let mut word = fixture();
    rec(&mut word, &ch(M2, 0))["address"] = json!({"manual": {"area": "q", "size": "word", "byte": 1}});
    let models = [
        ("pair", manual_bits(&[(0, 0, 3)])),
        ("three", manual_bits(&[(0, 0, 3), (1, 0, 3)])),
        ("and N more", manual_bits(&all)),
        ("word", word.build()),
    ];
    for (case, m) in models {
        let f = findings_of(&m, "EW-PLC-001");
        assert!(!f.is_empty(), "{case}");
        for f in f {
            assert_eq!(
                ew_checks::messages::format(&f, "en-GB").as_deref(),
                Some(f.message.as_str()),
                "{case} {}",
                f.discriminator
            );
            let nl = ew_checks::messages::format(&f, "nl-NL").expect("an nl-NL pattern");
            assert_ne!(nl, f.message, "{case}: nl-NL is its own text");
        }
    }
}

/// EW-PLC-002: a manual address outside the module's byte is reported; one inside it is not.
#[test]
fn ew_plc_002_reports_an_address_outside_the_module() {
    let mut fx = fixture();
    rec(&mut fx, &ch(M2, 2))["address"] = json!({"manual": {"area": "q", "byte": 1, "bit": 5}});
    assert!(
        findings_of(&fx.build(), "EW-PLC-002").is_empty(),
        "Q1.5 is inside -1U3's byte"
    );
    rec(&mut fx, &ch(M2, 2))["address"] = json!({"manual": {"area": "q", "byte": 4, "bit": 5}});
    let f = findings_of(&fx.build(), "EW-PLC-002");
    assert_eq!(subjects(&f), [("range".to_owned(), "-1U3:2".to_owned())]);
    assert_eq!(
        f[0].message,
        "-1U3:2: the address Q4.5 is outside the I/O range of -1U3 (bytes 1 to 1 of area Q)"
    );
    rec(&mut fx, &ch(M2, 2))["address"] = json!({"manual": {"area": "i", "byte": 1, "bit": 5}});
    let f = findings_of(&fx.build(), "EW-PLC-002");
    assert_eq!(
        f[0].message,
        "-1U3:2: the address I1.5 is outside the I/O range of -1U3 (bytes 1 to 1 of area Q)"
    );
}

/// EW-PLC-003: a stored bit index beyond the unit width, on a channel and on a module start.
#[test]
fn ew_plc_003_reports_a_bit_beyond_the_unit() {
    let mut fx = fixture();
    rec(&mut fx, &ch(M2, 2))["address"] = json!({"manual": {"area": "q", "byte": 1, "bit": 9}});
    rec(&mut fx, &us(M1))["plc_module"] =
        json!({"start": {"output": {"manual": {"area": "q", "byte": 0, "bit": 8}}}});
    let f = findings_of(&fx.build(), "EW-PLC-003");
    assert_eq!(
        subjects(&f),
        [
            ("start.output".to_owned(), "-1U2".to_owned()),
            ("address".to_owned(), "-1U3:2".to_owned())
        ],
        "{f:#?}"
    );
    assert_eq!(
        f[0].message,
        "-1U2: the output start names bit 8, outside the 8 bits of a unit"
    );
    assert_eq!(
        f[1].message,
        "-1U3:2: the manual address names bit 9, outside the 8 bits of a unit"
    );
}

/// EW-PLC-007: one finding per duplicated symbol, every carrier a target; symbols compare exactly.
#[test]
fn ew_plc_007_reports_a_duplicate_symbol_once() {
    let mut fx = fixture();
    rec(&mut fx, &ch(M1, 3))["symbol"] = json!("Pump_On");
    rec(&mut fx, &ch(M2, 4))["symbol"] = json!("Pump_On");
    rec(&mut fx, &ch(M2, 5))["symbol"] = json!("pump_on");
    let f = findings_of(&fx.build(), "EW-PLC-007");
    assert_eq!(subjects(&f), [("symbol".to_owned(), "Pump_On".to_owned())]);
    assert_eq!(
        f[0].message,
        "the symbol Pump_On is used by 2 channels of configuration project PLC1: -1U2:3, -1U3:4; give each channel its \
         own symbol"
    );
    assert_eq!(f[0].targets.len(), 2);
}

/// EW-PLC-008: a used channel with an excluded address, and a module outside every station.
#[test]
fn ew_plc_008_reports_a_used_channel_without_an_address() {
    let mut fx = fixture();
    rec(&mut fx, &ch(M1, 0))["address"] = json!("excluded");
    let f = findings_of(&fx.build(), "EW-PLC-008");
    assert_eq!(subjects(&f), [("address".to_owned(), "-1U2:0".to_owned())]);
    assert_eq!(
        f[0].message,
        "-1U2:0 is used but has no address: it is excluded from addressing; give it an address"
    );
    // a spare channel without an address is no finding
    let mut fx = fixture();
    rec(&mut fx, &ch(M1, 4))["address"] = json!("excluded");
    assert!(findings_of(&fx.build(), "EW-PLC-008").is_empty());
}

/// EW-PLC-012 `binding` (PA-4): solenoid 14 of the valve at position 1 bound to channel 1 where the map says 0, with
/// the rebind quick fix; `manual`: a manual address that differs from the derived one.
#[test]
fn ew_plc_012_reports_a_binding_against_the_map_and_a_manual_deviation() {
    let mut fx = fixture();
    rec(&mut fx, &ch(M1, 0))
        .as_object_mut()
        .expect("object")
        .remove("binding");
    rec(&mut fx, &ch(M1, 1))["binding"] = json!({"cp": [us(COIL), 0]});
    let f = findings_of(&fx.build(), "EW-PLC-012");
    assert_eq!(
        subjects(&f),
        [("binding".to_owned(), "-1U2:1".to_owned())],
        "{f:#?}"
    );
    assert_eq!(
        f[0].message,
        "-1U2:1 (Q0.1) drives solenoid 14 of -1Q1, which the position map of part 533361 assigns to -1U2:0 (Q0.0); \
         rebind it to the map's channel, or correct the part's PLC section"
    );
    let mut kinds: Vec<Kind> = f[0].targets.iter().map(|t| t.kind).collect();
    kinds.sort();
    let mut want = vec![Kind::Channel, Kind::Function];
    want.sort();
    assert_eq!(kinds, want);
    let fix = f[0].fix.as_ref().expect("the rebind fix");
    assert_eq!(fix.command, "plc.bind_channel");
    assert_eq!(fix.params["channel"], json!(ch(M1, 0)));
    assert_eq!(
        fix.params["endpoint"],
        json!({"cp": {"function": us(COIL), "idx": 0}})
    );
    assert!(!f[0].unverified_data, "an unmarked part is verified data");

    let mut fx = fixture();
    rec(&mut fx, &ch(M2, 6))["address"] = json!({"manual": {"area": "q", "byte": 1, "bit": 7}});
    let f = findings_of(&fx.build(), "EW-PLC-012");
    assert_eq!(subjects(&f), [("manual".to_owned(), "-1U3:6".to_owned())]);
    assert_eq!(
        f[0].message,
        "-1U3:6: the manual address Q1.7 differs from the derived address Q1.6; set the address to auto, or keep the \
         deviation deliberately"
    );
    // a manual address equal to the derived one is no deviation
    rec(&mut fx, &ch(M2, 6))["address"] = json!({"manual": {"area": "q", "byte": 1, "bit": 6}});
    assert!(findings_of(&fx.build(), "EW-PLC-012").is_empty());
}

/// PA-5 (FT-323's PLC half): a part whose `plc` section is of confidence `test` marks EW-PLC-012's findings.
#[test]
fn ew_plc_012_marks_findings_on_a_test_section() {
    let mut fx = fixture_with(&json!({"confidence": {"plc": "test"}}));
    rec(&mut fx, &ch(M1, 0))
        .as_object_mut()
        .expect("object")
        .remove("binding");
    rec(&mut fx, &ch(M1, 1))["binding"] = json!({"cp": [us(COIL), 0]});
    let f = findings_of(&fx.build(), "EW-PLC-012");
    assert_eq!(f.len(), 1);
    assert!(f[0].unverified_data, "the finding rests on unverified data");
}

/// EW-PLC-015 (PA-4): a `Tbd` start and a `Tbd` channel address of a module in a configuration project, an info
/// online and an error in release; EW-CORE-044's findings for the same values are superseded, so each open value
/// prints once.
#[test]
fn ew_plc_015_reports_tbd_values_and_supersedes_ew_core_044() {
    let mut fx = fixture();
    rec(&mut fx, &us(M2))["plc_module"] = json!({"start": {"output": "tbd"}});
    rec(&mut fx, &ch(M1, 5))["address"] = json!("tbd");
    let m = fx.build();
    let f = findings_of(&m, "EW-PLC-015");
    assert_eq!(
        subjects(&f),
        [
            ("address".to_owned(), "-1U2:5".to_owned()),
            ("start.output".to_owned(), "-1U3".to_owned())
        ],
        "{f:#?}"
    );
    assert_eq!(
        f[0].message,
        "-1U2:5: the channel address is TBD; enter it before release"
    );
    assert_eq!(
        f[1].message,
        "-1U3: the output start address is TBD; enter it before release"
    );
    assert!(f.iter().all(|f| f.severity == ew_checks::Severity::Info));
    assert!(
        findings_of(&m, "EW-CORE-044").is_empty(),
        "superseded by EW-PLC-015"
    );

    let q = ew_derive::Q::new(&m);
    let release = ew_checks::run(&q, &ew_checks::FULL, ew_checks::CheckMode::Release);
    let tbd: Vec<&ew_checks::Finding> = release
        .findings
        .iter()
        .filter(|f| f.rule == "EW-PLC-015" || f.rule == "EW-CORE-044")
        .collect();
    assert_eq!(tbd.len(), 2, "{tbd:#?}");
    assert!(
        tbd.iter()
            .all(|f| f.rule == "EW-PLC-015" && f.severity == ew_checks::Severity::Error)
    );
}

/// A module outside every configuration project keeps EW-CORE-044's finding; EW-PLC-008 names the missing station.
#[test]
fn a_module_without_a_station_stays_ew_core_044s() {
    let mut fx = fixture();
    rec(&mut fx, &us(M2))
        .as_object_mut()
        .expect("object")
        .remove("parent");
    rec(&mut fx, &us(M2))["plc_module"] = json!({"start": {"output": "tbd"}});
    let m = fx.build();
    assert!(findings_of(&m, "EW-PLC-015").is_empty());
    assert_eq!(findings_of(&m, "EW-CORE-044").len(), 1);
}

/// EW-CORE-008 `duplicate_number` (FT-350, PA-7): a second channel numbered 2 on `-1U3` is reported; the v5 one is kept.
#[test]
fn ew_core_008_reports_a_duplicate_channel_number() {
    let mut fx = fixture();
    fx.push(json!({"$k": "channel", "id": us(0xC0), "module": us(M2), "number": 2, "kind": "do"}));
    // EW-CORE-008 is a release-mode rule
    let m = fx.build();
    let q = ew_derive::Q::new(&m);
    let f: Vec<ew_checks::Finding> = ew_checks::run(&q, &ew_checks::FULL, ew_checks::CheckMode::Release)
        .findings
        .into_iter()
        .filter(|f| f.rule == "EW-CORE-008")
        .collect();
    assert_eq!(
        subjects(&f),
        [("duplicate_number".to_owned(), "-1U3:2".to_owned())],
        "{f:#?}"
    );
    assert!(
        f[0].targets
            .iter()
            .any(|t| t.kind == Kind::Channel && t.id == uid(0xC0)),
        "the channel without the v5 ID is the one reported"
    );
    assert_eq!(
        f[0].message,
        "-1U3 holds a second channel numbered 2; the module keeps one channel per number, so this one prints nowhere"
    );
}
