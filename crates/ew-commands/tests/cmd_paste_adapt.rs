// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.07 part 4b at the command layer (D22, FT-396; PS8's command half): `edit.paste`'s optional `adapt` reuses a
//! held library copy of another version instead of refusing ("Use the project's copy"), in one transaction and one undo
//! step; it refuses a major change from the pasted version to the held one, a copy listed twice, and a copy the
//! fragment does not carry at another version, each at `/adapt/<i>`; without `adapt` the old refusal holds; an empty
//! `adapt` is left out of the parameters, so an older paste re-serialises byte-equal.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{PasteFragment, library_copy_hash};
use ew_commands::{CmdError, Engine, Origin};
use ew_ids::{Id, v5};
use ew_model::Model;
use ew_schema::{Device, LibCopy, LibDocument, Part, Sha256};
use serde_json::{Value, json};

const LIB_Q: u128 = 0xd000;
const ITEM_Q: u128 = 0xd001;
const DEVICE: u128 = 0xd100;

fn with_hash(mut copy: LibCopy<Part>) -> LibCopy<Part> {
    let hash = library_copy_hash(&LibDocument::Part(copy.clone())).expect("a hash");
    copy.sha256 = Sha256(hash);
    copy
}

/// The part copy of [`LIB_Q`]/[`ITEM_Q`] at `version` with the CP designations `cps` and `description`.
fn version(version: &str, cps: &[&str], description: &str) -> LibCopy<Part> {
    let mut c = part(v5::lib_copy_id(uuid(LIB_Q), uuid(ITEM_Q)), "FX-MCB-1PN");
    c.library_id = Id::from_uuid(uuid(LIB_Q));
    c.item_id = Id::from_uuid(uuid(ITEM_Q));
    c.version = version.into();
    c.item.description = ew_mltext::MlText::neutral(description);
    c.item.cp_pattern = serde_json::from_value(
        cps.iter()
            .enumerate()
            .map(|(i, d)| json!({"idx": i, "designation": d}))
            .collect::<Value>(),
    )
    .expect("a CP pattern");
    with_hash(c)
}

/// The mini golden model holding `held`.
fn model(held: &LibCopy<Part>) -> Model {
    let (p, r, _) = mini_golden_parts();
    let mut lib = library();
    lib.push(LibDocument::Part(held.clone()));
    Model::from_records(p, r, lib).expect("valid")
}

/// `edit.paste` of one device whose part use names the copy, the fragment carrying `pasted`.
fn paste(pasted: &LibCopy<Part>, adapt: &[Id<()>]) -> PasteFragment {
    let fragment = json!({"format": "0.1", "project": uuid(0xd200).to_string(), "grid": [7_937_500, 3],
        "page_kind": "multi_line", "anchor": [0, 0],
        "records": [{"$k": "device", "id": uuid(DEVICE).to_string(), "tag": {"letter": "F", "counter": 9},
            "parts": [{"part": {"copy": pasted.id.to_string()}, "qty": "1"}]}],
        "copies": [serde_json::to_value(LibDocument::Part(pasted.clone())).unwrap()]});
    PasteFragment {
        page: PAGE_101,
        at: ew_units::Pt2::new(ew_units::Nm(0), ew_units::Nm(0)),
        fragment: serde_json::from_value(fragment).expect("a fragment"),
        adapt: adapt.to_vec(),
    }
}

fn refusal(e: &mut Engine, cmd: &PasteFragment) -> (String, String) {
    match e.execute(cmd).unwrap_err() {
        CmdError::InvalidParams { path, message, .. } => (path, message),
        err => panic!("{err:?}"),
    }
}

/// The part copies the pasted devices name (every device but the mini golden's).
fn pasted_uses(e: &Engine, before: &[Id<Device>]) -> Vec<Id<Part>> {
    e.model()
        .table::<Device>()
        .iter()
        .filter(|(id, _)| !before.contains(id))
        .flat_map(|(_, d)| d.parts.iter().map(|u| u.part.copy).collect::<Vec<_>>())
        .collect()
}

#[test]
fn adapt_reuses_the_held_copy_of_another_version_in_one_step() {
    let held = version("1.1.0", &["1", "2", "N", "N"], "MCB 1P+N C10 (new text)");
    let pasted = version("1.0.0", &["1", "2", "N", "N"], "MCB 1P+N C10");
    let mut e = engine(model(&held));
    let before = lines(e.model());
    let devices: Vec<Id<Device>> = e.model().table::<Device>().iter().map(|(id, _)| *id).collect();
    // Without adapt: the old refusal.
    let (path, message) = refusal(&mut e, &paste(&pasted, &[]));
    assert_eq!(path, "/fragment/copies/0");
    assert!(message.contains("update from library first"), "{message}");
    // With adapt: the held copy serves the pasted device; the held version stays.
    e.execute(&paste(&pasted, &[held.id.cast()])).expect("adapted");
    assert_eq!(pasted_uses(&e, &devices), [held.id]);
    let now = e
        .model()
        .table::<LibCopy<Part>>()
        .get(&held.id)
        .map(|c| (**c).clone());
    assert_eq!(now, Some(held.clone()), "the project's copy is unchanged");
    e.undo(&Origin::Gui).expect("one undo step");
    assert_eq!(lines(e.model()), before);
}

#[test]
fn adapt_refuses_a_major_change_twice_listed_and_unrelated_copies() {
    // The held version drops a CP the pasted records may name: major from the pasted version to the held one.
    let held = version("1.1.0", &["1", "2"], "MCB 1P C10");
    let pasted = version("1.0.0", &["1", "2", "N", "N"], "MCB 1P+N C10");
    let mut e = engine(model(&held));
    let (path, message) = refusal(&mut e, &paste(&pasted, &[held.id.cast()]));
    assert_eq!(path, "/adapt/0");
    // The whole refusal, one sentence with single spaces (FIX39a finding 4).
    assert_eq!(
        message,
        "the project's version of part 'FX-MCB-1PN' differs from the pasted one in a major way (CP 2 (N) disappears, \
         CP 3 (N) disappears); update instead"
    );
    // A copy listed twice.
    let ok = version("1.1.0", &["1", "2", "N", "N"], "new");
    let mut e = engine(model(&ok));
    let (path, _) = refusal(&mut e, &paste(&pasted, &[ok.id.cast(), ok.id.cast()]));
    assert_eq!(path, "/adapt/1");
    // A copy the fragment does not carry at another version.
    let other: Id<()> = Id::from_uuid(uuid(0xdead));
    let (path, message) = refusal(&mut e, &paste(&pasted, &[ok.id.cast(), other]));
    assert_eq!(path, "/adapt/1");
    assert!(message.contains("names no copy"), "{message}");
    // A held copy of the same hash is not "another version".
    let mut same = engine(model(&pasted));
    let (path, _) = refusal(&mut same, &paste(&pasted, &[pasted.id.cast()]));
    assert_eq!(path, "/adapt/0");
}

#[test]
fn an_empty_adapt_is_left_out_of_the_parameters() {
    let pasted = version("1.0.0", &["1", "2"], "x");
    let v = serde_json::to_value(paste(&pasted, &[])).unwrap();
    assert!(v.get("adapt").is_none(), "{v}");
    let back: PasteFragment = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(serde_json::to_value(&back).unwrap(), v);
    let with = serde_json::to_value(paste(&pasted, &[pasted.id.cast()])).unwrap();
    assert_eq!(with["adapt"], json!([pasted.id.to_string()]));
}
