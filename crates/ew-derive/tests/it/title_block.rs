// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Title-block values (doc 09 §2.6–§2.7, doc 20 §4.1, §4.2, §17.2) over the golden-shaped fixture, with the
//! anonymised placeholders of doc 20 §1.2 as literal strings. The full value sets of folio 101, folio 201 and
//! the divider of section 1 are `golden::title_block_value_sets_match_the_specification`.

use crate::common;

use common::{Fx, id, uid};
use ew_derive::{PageOrder, PageRef, Q, TbValue, TitleBlock, TitleBlockValues, profile_by_version};
use ew_mltext::MlText;
use serde_json::json;

const P01: u128 = 0x10;
const JE2416: u128 = 0x11;
const SEC1: u128 = 0x20;
const SEC2: u128 = 0x21;
const COVER: u128 = 0x301;
const PAGE3: u128 = 0x303;
const PAGE101: u128 = 0x401;
const PAGE102: u128 = 0x402;
const PAGE103: u128 = 0x403;
const PAGE201: u128 = 0x501;

fn ml(text: &str) -> MlText {
    serde_json::from_value(json!({ "en": text })).expect("multi-language text")
}

fn text(s: &str) -> TbValue {
    TbValue::Text(s.to_owned())
}

/// The golden-shaped fixture: doc 20 §4.1–§4.2 master data and parties, two sections, two revisions.
fn golden() -> Fx {
    let mut fx = Fx::new();
    fx.project_set(
        "master",
        json!({
            "description": {"en": "Expansion Control Cabinets"},
            "long_description": {"en": "Expansion Control Cabinets <end user>"},
            "document_title": {"en": "Electrical Definition"},
            "project_ref": "<project ref>",
            "drawing_id": "<drawing id>",
            "object_id": "J-E-2416",
            "manufacturing_year": 2025,
            "created": "2025-03-03",
            "edited": "2025-04-14",
        }),
    )
    .project_set(
        "supply",
        json!({"phases": 1, "neutral": true, "pe": true, "earthing": "tn_s", "u_nom": "230", "frequency": "50"}),
    )
    .project_set(
        "parties",
        json!([
            {"role": "customer", "name": {"en": "<customer>"}},
            {"role": "end_user", "name": {"en": "<end user>"},
             "address": {"street": "<address>", "postcode": "<postcode>", "city": "<city>", "country": "<country>"}},
            {"role": "panel_builder", "name": {"en": "<panel builder>"}},
            {"role": "engineer", "name": {"en": "<engineer>"}, "personal_data": true},
        ]),
    );
    fx.node(P01, "function", "P01", None)
        .node(JE2416, "location", "J-E-2416", None)
        .section(SEC1, 1, (100, 199), "Wiring Definition", true)
        .section(SEC2, 2, (200, 299), "Valve Manifold Definition", true);
    let both = [("function", P01), ("location", JE2416)];
    fx.page(COVER, 1, None, "title_page", &[], "Cover")
        .page(PAGE3, 3, None, "graphic", &[], "Notes")
        .page(
            PAGE101,
            101,
            Some(SEC1),
            "multi_line",
            &both,
            "Power Supply 1 - Main Current",
        )
        .page(PAGE102, 102, Some(SEC1), "multi_line", &both, "Power Supply 2")
        .page(PAGE103, 103, Some(SEC1), "multi_line", &both, "Distribution")
        .page(
            PAGE201,
            201,
            Some(SEC2),
            "fluid",
            &both,
            "Valve Manifold Overview 1",
        );
    fx.push(
        json!({"$k": "revision", "id": uid(0x601), "label": "V1.0", "date": "2025-03-03",
        "state": "released", "author": "engineer"}),
    )
    .push(
        json!({"$k": "revision", "id": uid(0x602), "label": "V2.0", "date": "2025-04-14",
        "state": "open", "author": "engineer"}),
    );
    fx
}

fn block(fx: &Fx, page: PageRef) -> TitleBlock {
    let m = fx.build();
    let q = Q::new(&m);
    (*q.get::<TitleBlockValues>(page)).clone()
}

/// Prev and Next at both ends of the page order (FR-PAG-013): (name, page, expected values, `None` = absent).
#[test]
fn prev_next_at_ends() {
    let fx = golden();
    let m = fx.build();
    let q = Q::new(&m);
    let last = q.get::<PageOrder>(()).entries.last().expect("pages").page;
    let cases = [
        (
            "the first page has no prev",
            PageRef::Drawn(id(COVER)),
            vec![
                ("page.folio", Some(text("1"))),
                ("page.prev", None),
                ("page.next", Some(text("3"))),
                ("doc.page_index", Some(text("1"))),
                // A page without a structure has no function, location or document kind.
                ("page.function", None),
                ("page.location", None),
                ("page.document_kind", None),
            ],
        ),
        (
            "the last page has no next",
            last,
            vec![("page.next", None), ("page.prev", Some(text("200")))],
        ),
    ];
    for (name, page, want) in cases {
        let tb = (*q.get::<TitleBlockValues>(page)).clone();
        for (path, value) in want {
            assert_eq!(tb.get(path), value.as_ref(), "case {name}: {path}");
        }
    }
}

#[test]
fn a_page_outside_the_order_has_no_page_or_doc_values() {
    let tb = block(&golden(), PageRef::Drawn(id(0xdead)));
    assert!(
        tb.values
            .keys()
            .all(|k| !k.starts_with("page.") && !k.starts_with("doc."))
    );
    // Project, party and revision values do not depend on the page.
    assert_eq!(tb.get("project.object"), Some(&text("J-E-2416")));
    assert_eq!(tb.get("revision.current.label"), Some(&text("V2.0")));
}

#[test]
fn description_override_wins() {
    let mut fx = golden();
    fx.record(PAGE102)["description_override"] = json!({"en": "Special Cabinet"});
    let tb = block(&fx, PageRef::Drawn(id(PAGE102)));
    assert_eq!(
        tb.get("page.description"),
        Some(&TbValue::Ml(ml("Special Cabinet")))
    );
    let other = block(&fx, PageRef::Drawn(id(PAGE103)));
    assert_eq!(
        other.get("page.description"),
        Some(&TbValue::Ml(ml("Expansion Control Cabinets")))
    );
}

#[test]
fn scale_prints_one_to_n() {
    let mut fx = golden();
    fx.record(PAGE103)["scale"] = json!({"one_to": 10});
    let tb = block(&fx, PageRef::Drawn(id(PAGE103)));
    assert_eq!(tb.get("page.scale"), Some(&text("1:10")));
}

#[test]
fn logo_and_notice_come_from_the_provisional_company_data() {
    let mut fx = golden();
    fx.project_set(
        "ew.company",
        json!({"logo": common::sha(7), "notice": {"en": "<notice>"}}),
    );
    let tb = block(&fx, PageRef::Drawn(id(PAGE101)));
    assert_eq!(
        tb.get("project.logo"),
        Some(&TbValue::Asset(ew_schema::AssetRef {
            sha256: common::sha(7).parse().expect("digest"),
            format: ew_schema::AssetFormat::Png,
            size: None,
        }))
    );
    assert_eq!(tb.get("project.notice"), Some(&TbValue::Ml(ml("<notice>"))));

    // FT-337 fix round: the bare-hash form takes the format and size of the asset the model holds.
    let sha: ew_schema::Sha256 = common::sha(7).parse().expect("digest");
    let held = ew_schema::AssetRef {
        sha256: sha,
        format: ew_schema::AssetFormat::Svg,
        size: Some((40, 10)),
    };
    let mut m = fx.build();
    m.put_asset(ew_model::Asset::new(held, &b"<svg/>"[..]));
    let q = Q::new(&m);
    let tb = (*q.get::<TitleBlockValues>(PageRef::Drawn(id(PAGE101)))).clone();
    assert_eq!(tb.get("project.logo"), Some(&TbValue::Asset(held)));

    // A malformed member leaves only its own path absent.
    fx.project_set(
        "ew.company",
        json!({"logo": "not a digest", "notice": {"en": "<notice>"}}),
    );
    let tb = block(&fx, PageRef::Drawn(id(PAGE101)));
    assert_eq!(tb.get("project.logo"), None);
    assert_eq!(tb.get("project.notice"), Some(&TbValue::Ml(ml("<notice>"))));

    // Without company data both are absent.
    let tb = block(&golden(), PageRef::Drawn(id(PAGE101)));
    assert_eq!(tb.get("project.logo"), None);
    assert_eq!(tb.get("project.notice"), None);
}

#[test]
fn supply_text_follows_the_supply_format() {
    let mut fx = golden();
    let supply = |fx: &Fx| {
        block(fx, PageRef::Drawn(id(PAGE101)))
            .get("project.supply")
            .cloned()
    };
    assert_eq!(supply(&fx), Some(text("1 x 230 VAC + N + PE, 50 Hz")));
    fx.project_set(
        "supply",
        json!({"phases": 3, "neutral": true, "pe": true, "u_nom": "400", "frequency": "50.5"}),
    );
    assert_eq!(supply(&fx), Some(text("3 x 400 VAC + N + PE, 50.5 Hz")));
    fx.project_set(
        "supply",
        json!({"phases": 1, "pe": true, "u_nom": "24", "frequency": "0"}),
    );
    assert_eq!(supply(&fx), Some(text("1 x 24 VDC + PE")));
    fx.project.as_object_mut().expect("object").remove("supply");
    assert_eq!(supply(&fx), None);
}

#[test]
fn quantities_are_not_bound_yet() {
    let mut fx = golden();
    fx.project_set("control_voltage", json!({"value": "24", "unit": "V"}))
        .project_set("rated_consumption", json!({"value": "6", "unit": "A"}));
    let tb = block(&fx, PageRef::Drawn(id(PAGE101)));
    assert_eq!(tb.get("project.control_voltage"), None);
    assert_eq!(tb.get("project.consumption"), None);
}

#[test]
fn the_current_revision_is_the_latest_and_a_checker_is_resolved() {
    let mut fx = golden();
    fx.record(0x602)["checker"] = json!("checker");
    // The checker party exists: its name is bound; the first party of a role wins.
    fx.project.as_object_mut().expect("object")["parties"]
        .as_array_mut()
        .expect("parties")
        .extend([
            json!({"role": "checker", "name": {"en": "<checker>"}}),
            json!({"role": "checker", "name": {"en": "<second checker>"}}),
        ]);
    let tb = block(&fx, PageRef::Drawn(id(PAGE101)));
    assert_eq!(
        tb.get("revision.current.checker.name"),
        Some(&TbValue::Ml(ml("<checker>")))
    );
    assert_eq!(tb.get("party.checker.name"), Some(&TbValue::Ml(ml("<checker>"))));

    // Equal dates: the natural order of the label decides (V10.0 after V9.0). Both are released: at most
    // one revision is open (EW-INV-016).
    let mut fx = Fx::new();
    fx.push(
        json!({"$k": "revision", "id": uid(1), "label": "V9.0", "date": "2025-01-01", "state": "released"}),
    )
    .push(
        json!({"$k": "revision", "id": uid(2), "label": "V10.0", "date": "2025-01-01", "state": "released"}),
    );
    let tb = block(&fx, PageRef::Drawn(id(1)));
    assert_eq!(tb.get("revision.current.label"), Some(&text("V10.0")));
}

/// The title block reads the typed revision rows (FT-165): relabelling the open revision re-derives the
/// memoised title block, and a second read of the same snapshot is a memo hit.
#[test]
#[allow(clippy::disallowed_methods)] // a raw edit of the snapshot, as a commit would make it
fn a_revision_edit_re_derives_the_memoised_title_block() {
    use ew_model::{AnyRow, Op};
    use ew_schema::{Kind, Revision};
    let model = golden().build();
    let store = ew_derive::MemoStore::new(&model);
    let page = PageRef::Drawn(id(PAGE101));
    let label = |store: &ew_derive::MemoStore| {
        store
            .query()
            .get::<TitleBlockValues>(page)
            .get("revision.current.label")
            .cloned()
    };
    assert_eq!(label(&store), Some(text("V2.0")));
    let key = (Kind::Revision, id::<()>(0x602));
    let before = model.row(&key).expect("the open revision");
    let mut rev: Revision = (**before.get::<Revision>().expect("a revision")).clone();
    rev.label = ew_mltext::Ident::from_input("V2.1").unwrap();
    let after = model
        .apply_unchecked(&[Op::update(before, AnyRow::of(rev))], model.rev().next())
        .expect("the edit applies");
    store.advance(&after, &[key]);
    assert_eq!(label(&store), Some(text("V2.1")));
    let hits = store.commit_stats().family("title_block").hits;
    assert_eq!(label(&store), Some(text("V2.1")));
    assert_eq!(store.commit_stats().family("title_block").hits, hits + 1);
}

#[test]
fn no_revision_means_no_revision_values() {
    let mut fx = golden();
    fx.records.retain(|r| r["$k"] != "revision");
    let tb = block(&fx, PageRef::Drawn(id(PAGE101)));
    assert!(tb.values.keys().all(|k| !k.starts_with("revision.")));
}

#[test]
fn only_profile_paths_appear() {
    let mut fx = golden();
    fx.project_set(
        "ew.company",
        json!({"logo": common::sha(7), "notice": {"en": "<notice>"}}),
    );
    let profile = profile_by_version("2027.1").expect("profile");
    for page in [PAGE101, PAGE201, COVER] {
        let tb = block(&fx, PageRef::Drawn(id(page)));
        assert!(!tb.values.is_empty());
        assert!(
            tb.values
                .keys()
                .all(|k| profile.title_block.paths.iter().any(|p| p == k)),
            "{:?}",
            tb.values.keys().collect::<Vec<_>>()
        );
    }
}

#[test]
fn an_empty_project_binds_nothing() {
    let tb = block(&Fx::new(), PageRef::Drawn(id(1)));
    assert!(tb.values.is_empty());
}

fn label_of(fx: &Fx) -> Option<TbValue> {
    block(fx, PageRef::Drawn(id(1)))
        .get("revision.current.label")
        .cloned()
}

/// Which revision is current: (name, revision records, the current label, the printed state when checked).
#[test]
fn current_revision_cases() {
    let rev = |n: u128, label: &str, date: Option<&str>, state: &str| {
        let mut r = json!({"$k": "revision", "id": uid(n), "label": label, "state": state});
        if let Some(d) = date {
            r["date"] = json!(d);
        }
        r
    };
    let v1 = || rev(1, "V1.0", Some("2025-03-03"), "released");
    let cases: Vec<(&str, Vec<serde_json::Value>, &str, Option<&str>)> = vec![
        (
            "an open undated revision beats a released dated one",
            vec![
                rev(1, "V2.0", Some("2025-04-14"), "released"),
                rev(2, "V3.0", None, "open"),
            ],
            "V3.0",
            None,
        ),
        (
            "a withdrawn revision is never current",
            vec![v1(), rev(2, "V2.0", Some("2025-05-01"), "withdrawn")],
            "V1.0",
            None,
        ),
        (
            "of two released revisions the latest is current",
            vec![v1(), rev(2, "V2.0", Some("2025-04-14"), "released")],
            "V2.0",
            None,
        ),
        // A withdrawal record leaves the revision it withdraws released (EW-INV-020), but the title block no
        // longer prints it: the latest revision that is not withdrawn is current.
        (
            "a revision named by a withdrawal record is never current",
            vec![
                v1(),
                rev(2, "V2.0", Some("2025-04-14"), "released"),
                json!({"$k": "revision", "id": uid(3), "label": "V2.0", "date": "2025-05-01",
                    "state": "withdrawn", "withdraws": uid(2)}),
            ],
            "V1.0",
            None,
        ),
        // A state of a newer format is kept verbatim by the typed record; it is not open, so it ranks with the
        // released revisions and prints as its text.
        (
            "a state of a newer format ranks with the released revisions",
            vec![v1(), rev(2, "V2.0", Some("2025-05-01"), "superseded")],
            "V2.0",
            Some("superseded"),
        ),
        (
            "an undated revision counts as newest within its state",
            vec![v1(), rev(2, "V0.9", None, "released")],
            "V0.9",
            None,
        ),
    ];
    for (name, records, label, state) in cases {
        let mut fx = Fx::new();
        for r in records {
            fx.push(r);
        }
        assert_eq!(label_of(&fx), Some(text(label)), "case {name}");
        if let Some(state) = state {
            let tb = block(&fx, PageRef::Drawn(id(1)));
            assert_eq!(
                tb.get("revision.current.state"),
                Some(&text(state)),
                "case {name}"
            );
        }
    }
}

/// FX37a: the revisions rank in the project's label order (`ew_derive::revision_label_key`): in the `letter` format
/// `AA` follows `Z` (natural order would put it first), and a label outside the format ranks below every label in it.
#[test]
fn the_current_revision_follows_the_project_label_order_past_z() {
    let rev = |n: u128, label: &str| json!({"$k": "revision", "id": uid(n), "label": label, "state": "released", "date": "2026-01-05"});
    let mut fx = Fx::new();
    fx.project_set("settings", json!({"revision_label": "letter"}));
    for (n, label) in [(1, "AA"), (2, "Z"), (3, "Y"), (4, "zz-imported")] {
        fx.push(rev(n, label));
    }
    assert_eq!(label_of(&fx), Some(text("AA")));
    // The same labels under the default format are all outside it: the natural order decides, as before.
    let mut fx = Fx::new();
    for (n, label) in [(1, "AA"), (2, "Z"), (3, "Y")] {
        fx.push(rev(n, label));
    }
    assert_eq!(label_of(&fx), Some(text("Z")));
}

#[test]
fn last_changed_in_follows_page_notes() {
    let mut fx = golden();
    // A note on a page that does not exist (a page deleted after the note) selects nothing.
    fx.record(0x602)["page_notes"] = json!([
        {"page": uid(PAGE101), "description": "reworked"},
        {"page": uid(PAGE102)},
        {"page": uid(0x9999)},
    ]);
    let last = |fx: &Fx, page| {
        block(fx, PageRef::Drawn(id(page)))
            .get("page.last_changed_in")
            .cloned()
    };
    assert_eq!(last(&fx, PAGE101), Some(text("V2.0")));
    assert_eq!(last(&fx, PAGE102), Some(text("V2.0")));
    assert_eq!(last(&fx, PAGE103), Some(text("V1.0")));

    // A released revision's `changed_pages` wins over its page notes: the typed release record's member
    // (WP-S1.18 part 1); an entry that is not a page ID is a schema violation of the line (ew-schema's tests).
    fx.record(0x601)["page_notes"] = json!([{"page": uid(PAGE103)}]);
    fx.record(0x601)["release"] = json!({"changed_pages": [uid(PAGE201)]});
    assert_eq!(last(&fx, PAGE201), Some(text("V1.0")));
    assert_eq!(last(&fx, PAGE103), Some(text("V1.0")));

    // Only drawn pages get the value; no revisions, no value.
    let tb = block(&fx, PageRef::Divider(id(SEC1)));
    assert_eq!(tb.get("page.last_changed_in"), None);
    fx.records.retain(|r| r["$k"] != "revision");
    assert_eq!(last(&fx, PAGE101), None);
}

#[test]
fn every_role_binds_its_address() {
    let mut fx = golden();
    let parties = fx.project.as_object_mut().expect("object")["parties"]
        .as_array_mut()
        .expect("parties");
    parties[0]["address"] = json!({"city": "<customer city>"});
    parties.push(json!({"role": "author", "name": {"en": "<author>"},
        "address": {"street": "<s>", "postcode": "<p>", "city": "<author city>", "country": "<c>"}}));
    let tb = block(&fx, PageRef::Drawn(id(PAGE101)));
    assert_eq!(
        tb.get("party.customer.address.city"),
        Some(&text("<customer city>"))
    );
    assert_eq!(tb.get("party.customer.address.street"), None);
    assert_eq!(tb.get("party.author.address.city"), Some(&text("<author city>")));
}
