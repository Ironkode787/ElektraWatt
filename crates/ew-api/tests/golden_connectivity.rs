// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Connectivity acceptance on the golden model (WP-S1.07; doc 20 §7–§10, AC-09, AC-10, AC-12, NFR-REL-07).
//!
//! The model is the golden tables loaded through the CSV bootstrap (`tests/golden_common`), with the
//! colour and size profiles of doc 20 §7.1, §7.2 where a test says so. Tests:
//!
//! - `potential_partition_matches_section_7_3` (AC-10): the 18 potentials with names, aliases (natural order as
//!   returned), members and types.
//! - `potential_member_sets_match_section_7_3_exactly`: the exact member CPs of every potential.
//! - `potential_voltage_and_class_columns_match_section_7_3`: voltage, AC/DC and circuit class columns.
//! - `bond_b1_links_0v_and_pe` (§7.4): the one bond link.
//! - `bridges_join_their_potentials` (§8.3): the 19 bridge connections.
//! - `every_golden_wire_is_one_physical_wire` (AC-09): W1 to W44 with ends, constituents, potentials, core.
//! - `wire_sizes_and_colours_follow_section_8_2` (AC-09): effective cross-section and colour with source.
//! - `every_connection_has_an_effective_size_and_colour` (AC-09): with and without profiles, and the provenance
//!   level of every value (7 override, 6 core, 2 profile, 1 project default, 0 built in).
//! - `changing_the_24v_colour_updates_every_affected_wire` (AC-12): one profile edit, all affected wires.
//! - `flow_matches_doc_08_section_3_6`: distances, switched and protected potentials, loads (decision D4).
//! - `connectivity_does_not_depend_on_record_order` (NFR-REL-07): reversed and shuffled records.
//! - `no_connectivity_notes_on_the_golden_model`: no `Connectivity`, `MalformedShim`, `ProfileOverlap` or
//!   `DefaultWireProfile` note.

mod golden_common;

use std::collections::{BTreeMap, BTreeSet};

use ew_api::Session;
use ew_derive::{
    ConnWireProps, DeriveNote, EdgeKind, PhysicalWire, PhysicalWiresOf, PotentialAttrsOf, PotentialFlow,
    PotentialId, PotentialNameOf, PotentialPartition, PropSource, Q, SizeRule, StitchGraphQ, WirePropsOf,
};
use ew_import_tables::ew_interop_api::ImportBatch;
use ew_schema::{BondKind, CircuitClass, Connection, ConnectionKind, Endpoint, PotentialType, Record};
use golden_common::{batch, ids_by_key, session, session_with, session_with_profiles, source_key, table};

// ---------------------------------------------------------------------------------------------------------
// Expected values (doc 20 §7.3, §8.2)

/// (name, aliases, member devices, type). A member device stands for any of its functions.
type PotRow = (
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    PotentialType,
);

fn potential_rows() -> Vec<PotRow> {
    use PotentialType::{L, LPlus, M, N, Pe};
    let pe_aliases: &[&str] = &[
        "J-E-206-PE",
        "PE-1.1",
        "PE-1.2",
        "PE-1.3",
        "PE-1.4",
        "PE-AIR1",
        "PE-AIR2",
    ];
    vec![
        ("L", &["J-E-206-L"], &["J-E-206-L@101", "1X1:1", "S51"], L),
        ("N", &["J-E-206-N"], &["J-E-206-N@101", "1X1:2", "S51"], N),
        ("L-S", &[], &["S51", "-101F1", "F55"], L),
        ("N-S", &[], &["S51", "-101F1", "F55"], N),
        ("L-1", &[], &["-101F1", "-102G1"], L),
        ("N-1", &[], &["-101F1", "-102G1"], N),
        ("L-55", &[], &["F55", "E55", "E56"], L),
        ("N-55", &[], &["F55", "T55", "S56"], N),
        ("N-55T", &[], &["T55", "E55"], N),
        ("N-55S", &[], &["S56", "E56"], N),
        (
            "PE",
            pe_aliases,
            &[
                "1X1:PE", "-XPE", "-X1032:1", "-X1032:2", "-X1032:3", "-X1032:4", "-X1032:5", "E55",
                "-102G1", "-201U1", "-202U1",
            ],
            Pe,
        ),
        ("+24V", &[], &["-102G1", "-102F1"], LPlus),
        ("+24V-F", &[], &["-102F1", "-102F2"], LPlus),
        (
            "24VDC-1",
            &["24VDC-AIR1", "24VDC-AIR2", "24VDC-AIR3", "24VDC-AIR4"],
            &[
                "-102F2", "-X1031:1", "-X1031:2", "-X1031:3", "-X1031:4", "-201U1", "-202U1",
            ],
            LPlus,
        ),
        ("24VDC-2", &[], &["-102F2", "-X1031:5", "-X1031:6"], LPlus),
        ("24VDC-3", &[], &["-102F2", "-X1031:7", "-X1031:8"], LPlus),
        ("24VDC-4", &[], &["-102F2", "-X1031:9", "-X1031:10"], LPlus),
        (
            "0V",
            &["0VDC", "0VDC-AIR1", "0VDC-AIR2", "0VDC-AIR3", "0VDC-AIR4"],
            &[
                "-102G1",
                "-102F2",
                "-X1031:11",
                "-X1031:12",
                "-X1031:13",
                "-X1031:14",
                "-X1031:15",
                "-X1031:16",
                "-X1031:17",
                "-X1031:18",
                "-X1031:19",
                "-X1031:20",
                "-201U1",
                "-202U1",
            ],
            M,
        ),
    ]
}

/// One wire of §8.2: (number, potentials, csa, colour, csa source, colour source, constituents).
/// Sources: `cdp` is a connection override, `rule` and `profile` a profile row, `core` a conductor, `bond`
/// the bond rule.
type WireRow = (
    u32,
    &'static [&'static str],
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
);

#[rustfmt::skip]
fn wire_rows() -> Vec<WireRow> {
    vec![
        (1, &["L"], "1.5", "BK", "rule", "rule", &["W1"]),
        (2, &["N"], "1.5", "BU", "rule", "rule", &["W2"]),
        (3, &["PE"], "1.5", "GNYE", "rule", "rule", &["W3"]),
        (4, &["L"], "2.5", "BK", "cdp", "profile", &["W4"]),
        (5, &["N"], "2.5", "BU", "cdp", "profile", &["W5"]),
        (6, &["PE"], "1.5", "GNYE", "rule", "rule", &["W6"]),
        (7, &["L-S"], "2.5", "BK", "cdp", "profile", &["W7"]),
        (8, &["L-S"], "1.5", "BK", "rule", "rule", &["W8"]),
        (9, &["N-S"], "2.5", "BU", "cdp", "profile", &["W9"]),
        (10, &["N-S"], "1.5", "BU", "rule", "rule", &["W10"]),
        (11, &["L-1"], "1.5", "BK", "rule", "rule", &["W11", "W11.101"]),
        (12, &["N-1"], "1.5", "BU", "rule", "rule", &["W12", "W12.101"]),
        (13, &["L-55"], "1.5", "BK", "rule", "rule", &["W13", "J1.feed"]),
        (14, &["L-55"], "1.5", "BK", "rule", "rule", &["W14", "J1.feed"]),
        (15, &["N-55"], "1.5", "BU", "rule", "rule", &["W15", "J2.feed"]),
        (16, &["N-55"], "1.5", "BU", "rule", "rule", &["W16", "J2.feed"]),
        (17, &["N-55T"], "1.5", "BU", "rule", "rule", &["W17"]),
        (18, &["N-55S"], "1.5", "BU", "rule", "rule", &["W18"]),
        (19, &["PE"], "1.5", "GNYE", "rule", "rule", &["W19", "W19.101"]),
        (20, &["+24V"], "1.5", "DBU", "cdp", "profile", &["W20", "J3.1"]),
        (21, &["+24V"], "1.5", "DBU", "cdp", "profile", &["W20", "W21"]),
        (22, &["0V"], "1.5", "WH", "cdp", "profile", &["W22", "J5.feed"]),
        (23, &["0V"], "1.5", "WH", "cdp", "profile", &["W23", "J6.feed"]),
        (24, &["+24V-F"], "1.5", "DBU", "cdp", "profile", &["W24", "J4.1"]),
        (25, &["+24V-F"], "1.5", "DBU", "cdp", "profile", &["W24", "W25"]),
        (26, &["0V"], "0.75", "WH", "rule", "rule", &["J6.feed", "W26.102", "W26"]),
        (27, &["0V", "PE"], "0.75", "GNYE", "bond", "bond", &["J5.feed", "W27.102", "W27"]),
        (28, &["PE"], "1.5", "GNYE", "rule", "rule", &["W28", "W28.102"]),
        (29, &["24VDC-1"], "0.75", "DBU", "rule", "rule", &["W29", "W29.102"]),
        (30, &["24VDC-2"], "0.75", "DBU", "rule", "rule", &["W30", "W30.102"]),
        (31, &["24VDC-3"], "0.75", "DBU", "rule", "rule", &["W31", "W31.102"]),
        (32, &["24VDC-4"], "0.75", "DBU", "rule", "rule", &["W32", "W32.102"]),
        (33, &["PE"], "0.75", "GNYE", "rule", "rule", &["W33", "W33.106"]),
        (34, &["PE"], "0.75", "GNYE", "rule", "rule", &["W34", "W34.106"]),
        (35, &["0V"], "core", "BK", "core", "core", &["W35", "W35.103"]),
        (36, &["0V"], "core", "BU", "core", "core", &["W36", "W36.103"]),
        (37, &["PE"], "core", "GNYE", "core", "core", &["W37", "W37.106"]),
        (38, &["24VDC-1"], "core", "WH", "core", "core", &["W38", "W38.103"]),
        (39, &["24VDC-1"], "core", "BN", "core", "core", &["W39", "W39.103"]),
        (40, &["0V"], "core", "BK", "core", "core", &["W40", "W40.103"]),
        (41, &["0V"], "core", "BU", "core", "core", &["W41", "W41.103"]),
        (42, &["PE"], "core", "GNYE", "core", "core", &["W42", "W42.106"]),
        (43, &["24VDC-1"], "core", "WH", "core", "core", &["W43", "W43.103"]),
        (44, &["24VDC-1"], "core", "BN", "core", "core", &["W44", "W44.103"]),
    ]
}

// ---------------------------------------------------------------------------------------------------------
// Helpers

fn connections(batch: &ImportBatch) -> BTreeMap<String, Connection> {
    batch
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Connection(c) => Some((source_key(r)?.to_owned(), c.clone())),
            _ => None,
        })
        .collect()
}

/// The functions (IDs as text) of a member device: every non-IP function of the `functions.csv` rows
/// whose device is `device`, or the IP function itself for a name with `@`.
fn functions_of(ids: &BTreeMap<String, String>, device: &str) -> Vec<String> {
    if device.contains('@') {
        return vec![ids[&format!("functions|{device}")].clone()];
    }
    table("functions.csv")
        .into_iter()
        .filter(|(_, r)| r["device"] == device && r["kind"] != "ip")
        .map(|(k, _)| ids[&format!("functions|{k}")].clone())
        .collect()
}

/// The expected end CPs of a wire: the CP endpoints of its constituent connections that no other
/// constituent shares, interruption points paired by name (a paired point is internal; an unpaired one is
/// an external end).
fn expected_ends(
    ids: &BTreeMap<String, String>,
    conns: &BTreeMap<String, Connection>,
    constituents: &[&str],
) -> BTreeSet<(String, u16)> {
    let inv: BTreeMap<&String, &String> = ids.iter().map(|(k, v)| (v, k)).collect();
    // An interruption point counts by its name (the page suffix dropped), any other CP by itself.
    let name_of = |f: &str, idx: u16| match inv[&f.to_owned()].split_once('@') {
        Some((head, _)) => (head.to_owned(), 0),
        None => (f.to_owned(), idx),
    };
    let cps: Vec<(String, u16)> = constituents
        .iter()
        .flat_map(|c| {
            let conn = &conns[&format!("connections|{c}")];
            [&conn.a, &conn.b]
                .into_iter()
                .filter_map(|e| match e {
                    Endpoint::Cp(f, idx) => Some((f.to_string(), *idx)),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let mut count: BTreeMap<(String, u16), usize> = BTreeMap::new();
    for (f, idx) in &cps {
        *count.entry(name_of(f, *idx)).or_default() += 1;
    }
    cps.into_iter()
        .filter(|(f, idx)| count[&name_of(f, *idx)] == 1)
        .collect()
}

fn csa_text(csa: &ew_schema::Csa) -> String {
    match csa {
        ew_schema::Csa::Mm2(d) => serde_json::to_string(d).unwrap().trim_matches('"').to_owned(),
        other => format!("{other:?}"),
    }
}

/// `["GN","YE"]` as `GNYE`.
fn colour_text(c: &ew_schema::ColourSpec) -> String {
    serde_json::to_string(c)
        .unwrap()
        .replace(['[', ']', '"', ','], "")
}

/// Every physical wire of the model by carrier ID text (deduplicated over the potentials).
fn all_wires(q: &Q) -> BTreeMap<String, PhysicalWire> {
    let part = q.get::<PotentialPartition>(());
    let mut out: BTreeMap<String, PhysicalWire> = BTreeMap::new();
    for p in part.potentials() {
        for w in &q.get::<PhysicalWiresOf>(p.id).wires {
            out.entry(w.id.to_string()).or_insert_with(|| w.clone());
        }
    }
    out
}

/// The effective properties of a wire: colour text, colour source, csa text, csa source, class.
type Props = (String, PropSource, String, PropSource, Option<CircuitClass>);

fn all_props(q: &Q) -> BTreeMap<String, Props> {
    let part = q.get::<PotentialPartition>(());
    let mut out = BTreeMap::new();
    for p in part.potentials() {
        let map = q.get::<WirePropsOf>(p.id);
        for (id, e) in &map.wires {
            let colour = e.colour.as_ref().expect("a colour");
            let csa = e.csa.as_ref().expect("a cross-section");
            let entry = (
                colour_text(&colour.value),
                colour.source.clone(),
                csa_text(&csa.value),
                csa.source.clone(),
                e.class.clone(),
            );
            if let Some(prev) = out.insert(id.to_string(), entry.clone()) {
                assert_eq!(
                    prev, entry,
                    "wire {id} has different properties under different potentials"
                );
            }
        }
    }
    out
}

fn pot_id(q: &Q, name: &str) -> PotentialId {
    ew_derive::potential_named(q, name).unwrap_or_else(|| panic!("potential {name}"))
}

// ---------------------------------------------------------------------------------------------------------
// Tests

#[test]
fn potential_partition_matches_section_7_3() {
    let batch = batch();
    let s = session(&batch);
    let ids = ids_by_key(&batch);
    let q = Q::new(&s.world().model);
    let part = q.get::<PotentialPartition>(());
    // The DP potential is not loaded by the bootstrap (decision log ID-69).
    assert_eq!(part.potential_count(), 18);
    assert_eq!(potential_rows().len(), 18);
    for (name, aliases, members, ptype) in potential_rows() {
        let p = part
            .get(ew_derive::potential_named(&q, name).unwrap_or_else(|| panic!("potential {name}")))
            .unwrap();
        // Doc 08 §3.5: the aliases come in natural order, which is the order of the §7.3 cells; compared
        // as returned, without sorting.
        let want: Vec<String> = aliases.iter().map(|a| (*a).to_owned()).collect();
        assert_eq!(q.get::<PotentialNameOf>(p.id).aliases, want, "aliases of {name}");
        for m in members {
            let fns = functions_of(&ids, m);
            assert!(
                p.cps.iter().any(|c| fns.contains(&c.function.to_string())),
                "{name}: member {m} has no CP in the potential"
            );
        }
        // A potential without a definition type or a CP role (N-55S: a lamp CP has no role) takes its type
        // from the flow propagation (doc 08 §3.6); the partition alone leaves it unset.
        let got = q
            .get::<PotentialAttrsOf>(p.id)
            .ptype
            .as_ref()
            .map(|t| t.value.clone())
            .or_else(|| {
                q.get::<PotentialFlow>(p.id)
                    .as_ref()
                    .as_ref()
                    .and_then(|e| e.ptype.as_ref().map(|t| t.value.clone()))
            });
        assert_eq!(got, Some(ptype), "type of {name}");
    }
}

/// The member CPs of a potential as `device:CP designation` text, from the golden tables: the device of
/// the function row of `functions.csv`, the designation of the instantiated CP. Interruption points are
/// left out (their names are the aliases, checked apart); a terminal stands for all its clamp and shaft
/// CPs (`-X1031:3`), as the §7.3 cells list them.
fn member_text(q: &Q, ids: &BTreeMap<String, String>, p: &ew_derive::Potential) -> BTreeSet<String> {
    let inv: BTreeMap<&String, &String> = ids.iter().map(|(k, v)| (v, k)).collect();
    let functions = table("functions.csv");
    let terminals = table("terminals.csv");
    let mut out = BTreeSet::new();
    for c in &p.cps {
        let key = inv[&c.function.to_string()]
            .strip_prefix("functions|")
            .expect("a function key");
        let row = &functions[key];
        if row["kind"] == "ip" {
            continue;
        }
        let device = &row["device"];
        if terminals.contains_key(device) {
            out.insert(device.clone());
            continue;
        }
        let cps = q.get::<ew_derive::FunctionCps>(c.function);
        let cp = cps.iter().find(|x| x.idx == c.idx).expect("an instantiated CP");
        out.insert(format!("{device}:{}", cp.designation));
    }
    out
}

/// `strip:from` to `strip:to` terminals.
fn terminals(strip: &str, from: u32, to: u32) -> Vec<String> {
    (from..=to).map(|n| format!("{strip}:{n}")).collect()
}

/// The member sets of doc 20 §7.3 as `device:designation` text (`#n` repeat markers dropped: a repeated
/// designation is one text).
fn member_sets() -> Vec<(&'static str, Vec<String>)> {
    let own = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<String>>();
    let cat = |a: Vec<String>, b: Vec<String>| [a, b].concat();
    vec![
        ("L", own(&["1X1:1", "S51:2"])),
        ("N", own(&["1X1:2", "S51:4"])),
        ("L-S", own(&["S51:1", "-101F1:1", "F55:1"])),
        ("N-S", own(&["S51:3", "-101F1:N", "F55:N"])),
        ("L-1", own(&["-101F1:2", "-102G1:L"])),
        ("N-1", own(&["-101F1:N", "-102G1:N"])),
        ("L-55", own(&["F55:2", "E55:L", "E56:X2"])),
        ("N-55", own(&["F55:N", "T55:5", "S56:13"])),
        ("N-55T", own(&["T55:3", "E55:N"])),
        ("N-55S", own(&["S56:14", "E56:X1"])),
        (
            "PE",
            cat(
                own(&[
                    "1X1:PE",
                    "-XPE:PE",
                    "E55:PE",
                    "-102G1:PE",
                    "-201U1:3",
                    "-201U1:FE",
                    "-202U1:3",
                    "-202U1:FE",
                ]),
                terminals("-X1032", 1, 5),
            ),
        ),
        ("+24V", own(&["-102G1:+", "-102F1:1"])),
        ("+24V-F", own(&["-102F1:2", "-102F2:24VDC"])),
        (
            "24VDC-1",
            cat(
                own(&["-102F2:Out1", "-201U1:4", "-201U1:5", "-202U1:4", "-202U1:5"]),
                terminals("-X1031", 1, 4),
            ),
        ),
        ("24VDC-2", cat(own(&["-102F2:Out2"]), terminals("-X1031", 5, 6))),
        ("24VDC-3", cat(own(&["-102F2:Out3"]), terminals("-X1031", 7, 8))),
        ("24VDC-4", cat(own(&["-102F2:Out4"]), terminals("-X1031", 9, 10))),
        (
            "0V",
            cat(
                own(&[
                    "-102G1:\u{2212}",
                    "-102F2:GND",
                    "-201U1:1",
                    "-201U1:2",
                    "-202U1:1",
                    "-202U1:2",
                ]),
                terminals("-X1031", 11, 20),
            ),
        ),
    ]
}

#[test]
fn potential_member_sets_match_section_7_3_exactly() {
    let batch = batch();
    let s = session(&batch);
    let ids = ids_by_key(&batch);
    let q = Q::new(&s.world().model);
    let part = q.get::<PotentialPartition>(());
    let sets = member_sets();
    assert_eq!(sets.len(), 18);
    for (name, want) in sets {
        let p = part
            .get(ew_derive::potential_named(&q, name).unwrap_or_else(|| panic!("potential {name}")))
            .unwrap();
        let want: BTreeSet<String> = want.into_iter().collect();
        assert_eq!(member_text(&q, &ids, p), want, "members of {name}");
    }
}

/// The voltage and circuit-class columns of §7.3: (name, voltage text, AC/DC, circuit class). `None` is a
/// cell that states nothing: PE has no voltage, the spare 24VDC-2 to -4 have none (no source-output data in
/// the golden PSU), and the switched N-55T and N-55S state the voltage but no class.
type ClassRow = (
    &'static str,
    Option<&'static str>,
    Option<ew_schema::AcDc>,
    Option<CircuitClass>,
);

fn class_rows() -> Vec<ClassRow> {
    use ew_schema::AcDc::{Ac, Dc};
    let mains = |n| (n, Some("230"), Some(Ac), Some(CircuitClass::Mains));
    let control = |n| (n, Some("24"), Some(Dc), Some(CircuitClass::Control));
    vec![
        mains("L"),
        mains("N"),
        mains("L-S"),
        mains("N-S"),
        mains("L-1"),
        mains("N-1"),
        mains("L-55"),
        mains("N-55"),
        ("N-55T", Some("230"), Some(Ac), None),
        ("N-55S", Some("230"), Some(Ac), None),
        ("PE", None, None, None),
        control("+24V"),
        control("+24V-F"),
        control("24VDC-1"),
        ("24VDC-2", None, None, None),
        ("24VDC-3", None, None, None),
        ("24VDC-4", None, None, None),
        control("0V"),
    ]
}

/// Doc 20 §7.3, column "Type, voltage, class" and "Colour, default size". Voltage and AC/DC are the
/// potential's own (definition) value, else the `PotentialFlow` entry's (the switched N-55T and N-55S take
/// them from N-55 over the pass-through pair). The class is the potential's own (`circuit_class` of a
/// definition) where the cell states one. A cell without a class (switched, spare, PE) means "whatever the
/// default size column says": the class that `WirePropsOf` resolves for the potential's wires, which must
/// be mains for N-55T and N-55S (1.5 mm²), control for the spare 24 V potentials (0.75 mm²) and, for PE,
/// mains and control by the highest class at the wire ends ("GNYE, by class").
#[test]
fn potential_voltage_and_class_columns_match_section_7_3() {
    let batch = batch();
    let s = session_with_profiles(&batch);
    let q = Q::new(&s.world().model);
    let part = q.get::<PotentialPartition>(());
    assert_eq!(class_rows().len(), 18);
    for (name, voltage, ac_dc, class) in class_rows() {
        let p = part
            .get(ew_derive::potential_named(&q, name).unwrap_or_else(|| panic!("potential {name}")))
            .unwrap();
        let flow = q.get::<PotentialFlow>(p.id);
        let entry = flow.as_ref().as_ref().expect("a flow entry");
        let attrs = q.get::<PotentialAttrsOf>(p.id);
        let got_voltage = attrs
            .voltage
            .as_ref()
            .or(entry.voltage.as_ref())
            .map(|a| a.value.to_string());
        assert_eq!(got_voltage.as_deref(), voltage, "voltage of {name}");
        let got_env = attrs
            .env
            .as_ref()
            .or(entry.env.as_ref())
            .and_then(|a| a.value.ac_dc.clone());
        assert_eq!(got_env, ac_dc, "AC/DC of {name}");
        let own_class = attrs.circuit_class.as_ref().map(|a| a.value.clone());
        assert_eq!(own_class, class, "own circuit class of {name}");
        // The class the wires of the potential resolve (level-2 size rule).
        let resolved: BTreeSet<String> = q
            .get::<WirePropsOf>(p.id)
            .wires
            .values()
            .filter_map(|e| e.class.as_ref().map(|c| format!("{c:?}")))
            .collect();
        let text = |c: CircuitClass| format!("{c:?}");
        let want: BTreeSet<String> = match (name, class) {
            (_, Some(c)) => BTreeSet::from([text(c)]),
            ("PE", None) => BTreeSet::from([text(CircuitClass::Mains), text(CircuitClass::Control)]),
            ("N-55T" | "N-55S", None) => BTreeSet::from([text(CircuitClass::Mains)]),
            (_, None) => BTreeSet::from([text(CircuitClass::Control)]),
        };
        assert_eq!(resolved, want, "resolved classes of {name}");
    }
}

#[test]
fn bond_b1_links_0v_and_pe() {
    let batch = batch();
    let s = session(&batch);
    let ids = ids_by_key(&batch);
    let q = Q::new(&s.world().model);
    let part = q.get::<PotentialPartition>(());
    assert_eq!(part.bonds().len(), 1);
    let b = &part.bonds()[0];
    assert_eq!(b.kind, BondKind::Pelv0vPe);
    let sides: BTreeSet<String> = [b.a, b.b]
        .into_iter()
        .map(|p| {
            q.get::<PotentialNameOf>(p.expect("a bond side"))
                .name
                .text
                .clone()
        })
        .collect();
    assert_eq!(sides, BTreeSet::from(["0V".to_owned(), "PE".to_owned()]));
    assert_eq!(b.via.map(|v| v.to_string()), Some(ids["connections|W27"].clone()));
    assert_eq!(b.names, vec!["PE-0VDC".to_owned()]);
    assert!(
        part.potentials().all(|p| !q
            .get::<PotentialNameOf>(p.id)
            .aliases
            .iter()
            .any(|a| a == "PE-0VDC")),
        "PE-0VDC is a bond name, no alias"
    );
}

#[test]
fn bridges_join_their_potentials() {
    let batch = batch();
    let s = session(&batch);
    let ids = ids_by_key(&batch);
    let conns = connections(&batch);
    let q = Q::new(&s.world().model);
    let part = q.get::<PotentialPartition>(());
    let graph = q.get::<StitchGraphQ>(());
    let bridged: BTreeSet<String> = graph
        .edges
        .iter()
        .filter_map(|e| match &e.kind {
            EdgeKind::Bridge(c) => Some(c.to_string()),
            _ => None,
        })
        .collect();
    let want: BTreeSet<String> = ids
        .iter()
        .filter(|(k, _)| k.starts_with("connections|BR"))
        .map(|(_, v)| v.clone())
        .collect();
    assert_eq!(want.len(), 19);
    assert_eq!(bridged, want);
    for (key, conn) in conns.iter().filter(|(k, _)| k.starts_with("connections|BR")) {
        let group: usize = key["connections|BR".len()..]
            .split('.')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let pot = ["24VDC-1", "24VDC-2", "24VDC-3", "24VDC-4", "0V", "PE"][group - 1];
        let p = part.get(ew_derive::potential_named(&q, pot).unwrap()).unwrap();
        for e in [&conn.a, &conn.b] {
            let Endpoint::Cp(f, idx) = e else {
                panic!("{key}: not a CP end")
            };
            assert!(
                p.cps.iter().any(|c| c.function == *f && c.idx == *idx),
                "{key}: end {f}#{idx} is not in {pot}"
            );
        }
    }
}

#[test]
fn every_golden_wire_is_one_physical_wire() {
    let batch = batch();
    let s = session(&batch);
    let ids = ids_by_key(&batch);
    let conns = connections(&batch);
    let q = Q::new(&s.world().model);
    let wires = all_wires(&q);
    let want_ids: BTreeSet<String> = (1..=44)
        .map(|n| ids[&format!("connections|W{n}")].clone())
        .collect();
    assert_eq!(wires.keys().cloned().collect::<BTreeSet<_>>(), want_ids);
    let csv = table("connections.csv");
    for (n, pots, _, _, _, _, constituents) in wire_rows() {
        let key = format!("connections|W{n}");
        let w = &wires[&ids[&key]];
        let got: BTreeSet<String> = w.constituents.iter().map(|c| c.to_string()).collect();
        let want: BTreeSet<String> = constituents
            .iter()
            .map(|c| ids[&format!("connections|{c}")].clone())
            .collect();
        assert_eq!(got, want, "W{n} constituents");
        let got_ends: BTreeSet<(String, u16)> = w
            .ends
            .iter()
            .map(|e| (e.cp.function.to_string(), e.cp.idx))
            .collect();
        assert_eq!(got_ends, expected_ends(&ids, &conns, constituents), "W{n} ends");
        let mut got_pots: Vec<PotentialId> = w.potentials.clone();
        got_pots.sort();
        let mut want_pots: Vec<PotentialId> = pots.iter().map(|p| pot_id(&q, p)).collect();
        want_pots.sort();
        assert_eq!(got_pots, want_pots, "W{n} potentials");
        // The core: the conductor of the `core` column of the carrier's row.
        let core = &csv[&format!("W{n}")]["core"];
        let want_core = (!core.is_empty()).then(|| ids[&format!("conductors|{core}")].clone());
        assert_eq!(w.core.map(|c| c.to_string()), want_core, "W{n} core");
        if n == 27 {
            assert_eq!(w.bond.map(|b| b.to_string()), Some(ids["bonds|B1"].clone()));
        } else {
            assert!(w.bond.is_none(), "W{n} has no bond");
        }
    }
}

#[test]
fn wire_sizes_and_colours_follow_section_8_2() {
    let batch = batch();
    let s = session_with_profiles(&batch);
    let ids = ids_by_key(&batch);
    let q = Q::new(&s.world().model);
    let props = all_props(&q);
    for (n, _, csa, colour, csa_src, colour_src, _) in wire_rows() {
        let (got_colour, got_colour_src, got_csa, got_csa_src, class) =
            &props[&ids[&format!("connections|W{n}")]];
        // Known data gap: 24VDC-2 to 24VDC-4 carry no voltage (the golden PSU has no source-output data)
        // and flow does not reach them, so no colour profile row matches and the built-in default BU
        // applies (doc 20 §8.2 expects DBU).
        let gap = (30..=32).contains(&n);
        if gap {
            assert_eq!(got_colour, "BU", "W{n} colour (data gap)");
            assert!(
                matches!(got_colour_src, PropSource::BuiltIn { .. }),
                "W{n}: {got_colour_src:?}"
            );
            assert!(
                matches!(
                    got_csa_src,
                    PropSource::SizeRow {
                        class: CircuitClass::Control,
                        ..
                    }
                ),
                "W{n}: {got_csa_src:?}"
            );
        } else {
            assert_eq!(got_colour, colour, "W{n} colour");
            match colour_src {
                "rule" | "profile" | "bond" => {
                    assert!(
                        matches!(got_colour_src, PropSource::ColourRow { .. }),
                        "W{n}: {got_colour_src:?}"
                    )
                }
                "core" => {
                    assert!(
                        matches!(got_colour_src, PropSource::Conductor(_)),
                        "W{n}: {got_colour_src:?}"
                    )
                }
                other => panic!("{other}"),
            }
        }
        match csa_src {
            "cdp" => {
                assert_eq!(got_csa, csa, "W{n} csa");
                assert!(
                    matches!(got_csa_src, PropSource::Override(_)),
                    "W{n}: {got_csa_src:?}"
                );
            }
            "rule" => {
                assert_eq!(got_csa, csa, "W{n} csa");
                assert!(
                    matches!(got_csa_src, PropSource::SizeRow { .. }),
                    "W{n}: {got_csa_src:?}"
                );
            }
            "core" => assert!(
                matches!(got_csa_src, PropSource::SizeRow { .. }),
                "W{n}: {got_csa_src:?}"
            ),
            "bond" => {
                assert_eq!(got_csa, csa, "W{n} csa");
                assert!(
                    matches!(
                        got_csa_src,
                        PropSource::SizeRow {
                            rule: SizeRule::Bond,
                            ..
                        }
                    ),
                    "W{n}: {got_csa_src:?}"
                );
            }
            other => panic!("{other}"),
        }
        if matches!(n, 6 | 19 | 28 | 33 | 34) {
            assert!(
                matches!(
                    got_csa_src,
                    PropSource::SizeRow {
                        rule: SizeRule::PeHighestAtEnds,
                        ..
                    }
                ),
                "W{n}: {got_csa_src:?}"
            );
            let want = if n < 33 {
                CircuitClass::Mains
            } else {
                CircuitClass::Control
            };
            assert_eq!(*class, Some(want), "W{n} class");
        }
    }
}

/// Evaluates `ConnWireProps` of every Wire and Core connection and checks size and colour.
fn check_all_connections(s: &Session, batch: &ImportBatch) -> Q {
    let q = Q::new(&s.world().model);
    let mut checked = 0;
    for r in &batch.records {
        let Record::Connection(c) = r else { continue };
        if !matches!(c.kind, ConnectionKind::Wire | ConnectionKind::Core(_)) {
            continue;
        }
        let entries = q.get::<ConnWireProps>(c.id);
        assert!(!entries.is_empty(), "connection {} has no wire properties", c.id);
        for (w, e) in entries.iter() {
            assert!(e.csa.is_some(), "wire {w}: no csa");
            assert!(e.colour.is_some(), "wire {w}: no colour");
        }
        checked += 1;
    }
    assert!(checked > 44, "{checked} connections checked");
    q
}

/// Which sources a session has, for the expected provenance levels.
#[derive(Clone, Copy)]
struct Sources {
    /// The colour and size profiles of doc 20 §7.1, §7.2 (level 2).
    profiles: bool,
    /// A project default `wire_default` (level 1).
    default: bool,
}

/// Checks the provenance level of the size and the colour of every physical wire (doc 08 §4.1): 7 where a
/// constituent connection overrides the value, 6 where the core's conductor sets it, else 2 with profiles
/// (3 would be a definition's props, which the golden potentials do not have), else 1 with a project
/// default, else 0. The colour of the spare 24 V wires (no voltage: the documented data gap) matches no row
/// and falls through to level 1 or 0. Returns the (csa, colour) level of every wire by carrier ID.
fn check_levels(q: &Q, batch: &ImportBatch, sources: Sources) -> BTreeMap<String, (u8, u8)> {
    let conns: BTreeMap<String, Connection> = batch
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Connection(c) => Some((c.id.to_string(), c.clone())),
            _ => None,
        })
        .collect();
    let conductors: BTreeMap<String, ew_schema::Conductor> = batch
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Conductor(c) => Some((c.id.to_string(), c.clone())),
            _ => None,
        })
        .collect();
    let part = q.get::<PotentialPartition>(());
    let props = all_props(q);
    let mut out = BTreeMap::new();
    for (id, w) in all_wires(q) {
        let (_, colour_src, _, csa_src, _) = &props[&id];
        let overrides = |get: fn(&ew_schema::WireOverrides) -> bool| {
            w.constituents
                .iter()
                .any(|c| get(&conns[&c.to_string()].overrides))
        };
        let conductor = w.core.map(|c| &conductors[&c.to_string()]);
        let spare = w.potentials.iter().all(|p| {
            matches!(
                part.get(*p)
                    .map(|p| q.get::<PotentialNameOf>(p.id).name.text.clone())
                    .as_deref(),
                Some("24VDC-2" | "24VDC-3" | "24VDC-4")
            )
        });
        let fallback = |profile_row: bool| match (profile_row, sources) {
            (true, Sources { profiles: true, .. }) => 2,
            (_, Sources { default: true, .. }) => 1,
            _ => 0,
        };
        let want_csa = if overrides(|o| o.csa.is_some()) {
            7
        } else if conductor.is_some_and(|c| c.csa.is_some()) {
            6
        } else {
            fallback(true)
        };
        let want_colour = if overrides(|o| o.colour.is_some()) {
            7
        } else if conductor.is_some_and(|c| c.colour.is_some()) {
            6
        } else {
            fallback(!spare)
        };
        assert_eq!(csa_src.level(), want_csa, "csa level of wire {id}: {csa_src:?}");
        assert_eq!(
            colour_src.level(),
            want_colour,
            "colour level of wire {id}: {colour_src:?}"
        );
        out.insert(id, (want_csa, want_colour));
    }
    assert_eq!(out.len(), 44);
    out
}

#[test]
fn every_connection_has_an_effective_size_and_colour() {
    let batch = batch();
    let with = session_with_profiles(&batch);
    let mut q = check_all_connections(&with, &batch);
    let notes = q.diagnostics().notes();
    assert!(
        !notes
            .iter()
            .any(|n| matches!(n, DeriveNote::DefaultWireProfile { .. }))
    );
    // With profiles: the 10 wires of W4, W5, W7, W9 and W20 to W25 override the size (level 7), the 10 cores
    // set the colour (level 6), everything else comes from the profile rows (level 2; the 3 spare 24 V wires
    // match no colour row and take the built-in colour, level 0).
    let levels = check_levels(
        &q,
        &batch,
        Sources {
            profiles: true,
            default: false,
        },
    );
    assert_eq!(count_where(&levels, |l| l.0 == 7), 10);
    assert_eq!(count_where(&levels, |l| l.0 == 2), 34);
    assert_eq!(count_where(&levels, |l| l.1 == 6), 10);
    assert_eq!(count_where(&levels, |l| l.1 == 2), 31);
    assert_eq!(count_where(&levels, |l| l.1 == 0), 3);

    // Without profiles and without a default: overrides and cores as before, the rest is built in (level 0).
    let without = session(&batch);
    let mut q = check_all_connections(&without, &batch);
    let notes = q.diagnostics().notes();
    for member in ["colour_profile", "size_profile"] {
        assert!(
            notes
                .iter()
                .any(|n| matches!(n, DeriveNote::DefaultWireProfile { member: m } if *m == member)),
            "no DefaultWireProfile note for {member}: {notes:?}"
        );
    }
    let levels = check_levels(
        &q,
        &batch,
        Sources {
            profiles: false,
            default: false,
        },
    );
    assert_eq!(count_where(&levels, |l| l.0 == 0), 34);
    assert_eq!(count_where(&levels, |l| l.1 == 0), 34);

    // With a project default and no profiles the same wires take level 1.
    let defaulted = session_with(&batch, |p, _| {
        p.settings.extra.insert(
            "wire_default".into(),
            serde_json::json!({"csa": "1", "colour": ["GY"]}),
        );
    });
    let q = check_all_connections(&defaulted, &batch);
    let levels = check_levels(
        &q,
        &batch,
        Sources {
            profiles: false,
            default: true,
        },
    );
    assert_eq!(count_where(&levels, |l| l.0 == 1), 34);
    assert_eq!(count_where(&levels, |l| l.1 == 1), 34);
}

fn count_where(levels: &BTreeMap<String, (u8, u8)>, f: impl Fn(&(u8, u8)) -> bool) -> usize {
    levels.values().filter(|l| f(l)).count()
}

#[test]
fn changing_the_24v_colour_updates_every_affected_wire() {
    let batch = batch();
    let before_s = session_with_profiles(&batch);
    let before = all_props(&Q::new(&before_s.world().model));
    let after_s = session_with(&batch, |p, _| {
        let (mut colour, size) = golden_common::profiles_json();
        colour["rows"][0]["colour"] = serde_json::json!(["RD"]);
        p.settings.extra.insert("colour_profile".into(), colour);
        p.settings.extra.insert("size_profile".into(), size);
    });
    let after = all_props(&Q::new(&after_s.world().model));
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>()
    );
    let ids = ids_by_key(&batch);
    let mut changed = BTreeSet::new();
    for (w, b) in &before {
        let a = &after[w];
        if matches!(&b.1, PropSource::ColourRow { row: 0, .. }) {
            assert_eq!(a.0, "RD", "wire {w}");
            assert_eq!(a.1, b.1, "wire {w}: same row");
            changed.insert(w.clone());
        } else {
            assert_eq!(a, b, "wire {w} must not change");
        }
        // The cross-section never depends on the colour profile.
        assert_eq!((&a.2, &a.3), (&b.2, &b.3), "wire {w} csa");
    }
    // The 24 V wires of +24V (W20, W21), +24V-F (W24, W25) and 24VDC-1 (W29) took row 0.
    for n in [20, 21, 24, 25, 29] {
        assert!(
            changed.contains(&ids[&format!("connections|W{n}")]),
            "W{n} did not take row 0"
        );
    }
}

#[test]
fn flow_matches_doc_08_section_3_6() {
    let batch = batch();
    let s = session(&batch);
    let ids = ids_by_key(&batch);
    let q = Q::new(&s.world().model);
    let part = q.get::<PotentialPartition>(());
    let entry = |name: &str| {
        q.get::<PotentialFlow>(pot_id(&q, name))
            .as_ref()
            .clone()
            .unwrap_or_else(|| panic!("flow entry of {name}"))
    };
    for root in ["L", "N", "PE"] {
        assert_eq!(entry(root).distance, Some(0), "{root}");
    }
    let switched: BTreeSet<String> = part
        .potentials()
        .map(|p| q.get::<PotentialNameOf>(p.id).name.text.clone())
        .filter(|name| entry(name).switched)
        .collect();
    assert_eq!(switched, BTreeSet::from(["N-55T".to_owned(), "N-55S".to_owned()]));
    assert_eq!(
        entry("N-55T").switched_by.map(|f| f.to_string()),
        Some(ids["functions|T55"].clone())
    );
    assert_eq!(
        entry("N-55S").switched_by.map(|f| f.to_string()),
        Some(ids["functions|S56"].clone())
    );
    for (name, by) in [
        ("L-1", "-101F1"),
        ("N-1", "-101F1"),
        ("L-55", "F55"),
        ("N-55", "F55"),
        ("N-55T", "F55"),
        ("N-55S", "F55"),
    ] {
        assert_eq!(
            entry(name).protected_by.map(|f| f.to_string()),
            Some(ids[&format!("functions|{by}")].clone()),
            "{name} protected_by"
        );
    }
    // Loads (review decision D4): the consuming functions, never the feeding or a passing function.
    let device_of: BTreeMap<String, String> = table("functions.csv")
        .into_iter()
        .map(|(k, r)| (ids[&format!("functions|{k}")].clone(), r["device"].clone()))
        .collect();
    for (name, want) in [
        ("L-55", &["E55", "E56"][..]),
        ("N-1", &["-102G1"][..]),
        ("L-S", &[][..]),
        ("N-S", &[][..]),
        ("N-55T", &["E55"][..]),
        ("N-55S", &["E56"][..]),
    ] {
        let got: BTreeSet<String> = entry(name)
            .loads
            .iter()
            .map(|l| device_of[&l.function.to_string()].clone())
            .collect();
        let want: BTreeSet<String> = want.iter().map(|d| (*d).to_owned()).collect();
        assert_eq!(got, want, "loads of {name}");
    }
    // Documented gap: the golden PSU has no source-output data, so flow does not reach the 24 V potentials.
    for name in ["+24V", "+24V-F", "24VDC-1", "24VDC-2", "24VDC-3", "24VDC-4"] {
        assert_eq!(entry(name).distance, None, "{name}");
    }
}

/// The `Debug` text of every connectivity result of a session (profiles present).
fn digest(s: &Session) -> String {
    let q = Q::new(&s.world().model);
    let part = q.get::<PotentialPartition>(());
    let mut out = format!("{:?}\n", *part);
    for p in part.potentials() {
        out += &format!(
            "{:?}\n{:?}\n",
            *q.get::<PhysicalWiresOf>(p.id),
            *q.get::<WirePropsOf>(p.id)
        );
    }
    out
}

#[test]
fn connectivity_does_not_depend_on_record_order() {
    let batch = batch();
    let plain = digest(&session_with(&batch, |p, _| golden_common::with_profiles(p)));
    let reversed = digest(&session_with(&batch, |p, r| {
        golden_common::with_profiles(p);
        r.reverse();
    }));
    assert_eq!(plain, reversed, "reversed records");
    let shuffled = digest(&session_with(&batch, |p, r| {
        golden_common::with_profiles(p);
        // Fisher-Yates with a fixed-seed LCG.
        let mut state: u64 = 0x2545_f491_4f6c_dd1d;
        for i in (1..r.len()).rev() {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let j = ((state >> 33) as usize) % (i + 1);
            r.swap(i, j);
        }
    }));
    assert_eq!(plain, shuffled, "shuffled records");
}

#[test]
fn no_connectivity_notes_on_the_golden_model() {
    let batch = batch();
    let s = session_with_profiles(&batch);
    let mut q = check_all_connections(&s, &batch);
    let part = q.get::<PotentialPartition>(());
    for p in part.potentials() {
        let _ = q.get::<PotentialNameOf>(p.id);
        let _ = q.get::<PhysicalWiresOf>(p.id);
        let _ = q.get::<WirePropsOf>(p.id);
        let _ = q.get::<PotentialFlow>(p.id);
    }
    let notes = q.diagnostics().notes();
    // With the golden profiles injected, no profile or connectivity note at all: neither a malformed shim,
    // nor an overlapping colour row pair, nor a connectivity finding, nor a missing-profile default.
    let unwanted: Vec<&DeriveNote> = notes
        .iter()
        .filter(|n| {
            matches!(
                n,
                DeriveNote::Connectivity { .. }
                    | DeriveNote::MalformedShim { .. }
                    | DeriveNote::ProfileOverlap { .. }
                    | DeriveNote::DefaultWireProfile { .. }
            )
        })
        .collect();
    assert!(unwanted.is_empty(), "{unwanted:#?}");
}
