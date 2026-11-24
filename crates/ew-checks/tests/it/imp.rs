// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The EW-IMP findings (doc 14 §2.5): the entries an importer recorded in the project's import runs, read back as
//! findings of their codes. How an entry and a live finding on the same records make one row is tested beside the
//! code, in `src/rules/imp.rs`.

use crate::common;

use common::{Fx, release_findings_of, us};
use ew_checks::Severity;
use serde_json::json;

const DEVICE: u128 = 0x301;
const LINEAGE: u128 = 0x900;
const LAST: u128 = 0x901;
const EARLIER: u128 = 0x902;

/// An import run `n` of the lineage with `entries`.
fn run(n: u128, entries: serde_json::Value) -> serde_json::Value {
    json!({
        "$k": "import_run", "id": us(n), "lineage": us(LINEAGE), "importer": "qet", "importer_version": "0.1.0",
        "source": {"name": "plant.qet", "sha256": format!("sha256:{}", "ab".repeat(32)), "size": 12,
                   "format": "qet-project"},
        "mode": "lenient", "entries": entries,
    })
}

/// Each entry of a lineage's last run is one finding of its code at its severity, its subject the entry's place (its
/// folio and cell, else its file); EW-IMP-103 on a device whose tag is still frozen carries the fix that unfreezes
/// it. An earlier run of the lineage is not read.
#[test]
fn the_entries_of_a_lineage_s_last_run_are_findings_of_their_codes() {
    let mut fx = Fx::base();
    fx.device(DEVICE, "K", Some(1));
    fx.rec_mut(&us(DEVICE))["tag"]["frozen"] = json!({"text": "K1", "sign": "hidden"});
    fx.push(json!({
        "$k": "import_lineage", "id": us(LINEAGE), "importer": "qet", "source_name": "plant",
        "anchor": {"fixed": "plant"}, "namespace": us(0xF000), "last_run": us(LAST),
    }));
    fx.push(run(
        LAST,
        json!([
            {"code": "EW-IMP-103", "severity": "error", "message": "label kept as a frozen tag",
             "location": {"file": "plant.qet", "folio": "101", "cell": "B2"},
             "targets": [{"kind": "device", "id": us(DEVICE)}]},
            {"code": "EW-IMP-006", "severity": "warning", "message": "font substituted",
             "location": {"file": "plant.qet"}},
        ]),
    ));
    fx.push(run(
        EARLIER,
        json!([{"code": "EW-IMP-006", "severity": "info", "message": "font substituted"}]),
    ));
    let m = fx.build();
    let kept = release_findings_of(&m, "EW-IMP-103");
    assert_eq!(kept.len(), 1, "{kept:#?}");
    assert_eq!(
        (kept[0].severity, kept[0].subject.as_str()),
        (Severity::Error, "folio 101, B2")
    );
    assert_eq!(
        kept[0].fix.as_ref().map(|f| f.label.to_string()).as_deref(),
        Some("Unfreeze the tag")
    );
    let font = release_findings_of(&m, "EW-IMP-006");
    assert_eq!(font.len(), 1, "the last run only: {font:#?}");
    assert_eq!(
        (font[0].severity, font[0].subject.as_str()),
        (Severity::Warning, "plant.qet")
    );
}
