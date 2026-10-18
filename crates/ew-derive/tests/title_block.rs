// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Title-block values (doc 09 §2.6–§2.7, doc 20 §4.1, §4.2, §17.2): full value sets of the golden-shaped
//! fixture, with the anonymised placeholders of doc 20 §1.2 as literal strings.

mod common;

use std::collections::BTreeMap;

use common::{Fx, id, uid};
use ew_derive::{PageOrder, PageRef, Q, TbValue, TitleBlock, TitleBlockValues, profile_by_version};
use ew_mltext::MlText;
use ew_schema::Date;
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

fn date(y: u16, m: u8, d: u8) -> TbValue {
    TbValue::Date(Date::new(y, m, d).expect("date"))
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

fn expected(pairs: &[(&str, TbValue)]) -> BTreeMap<String, TbValue> {
    pairs.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect()
}

/// The values every page of the golden fixture shares.
fn shared() -> Vec<(&'static str, TbValue)> {
    vec![
        (
            "project.description",
            TbValue::Ml(ml("Expansion Control Cabinets")),
        ),
        (
            "project.long_description",
            TbValue::Ml(ml("Expansion Control Cabinets <end user>")),
        ),
        ("project.document_title", TbValue::Ml(ml("Electrical Definition"))),
        ("project.reference", text("<project ref>")),
        ("project.drawing_id", text("<drawing id>")),
        ("project.object", text("J-E-2416")),
        ("project.created", date(2025, 3, 3)),
        ("project.edited", date(2025, 4, 14)),
        ("project.supply", text("1 x 230 VAC + N + PE, 50 Hz")),
        ("revision.current.label", text("V2.0")),
        ("revision.current.state", text("open")),
        ("revision.current.date", date(2025, 4, 14)),
        ("revision.current.author.name", TbValue::Ml(ml("<engineer>"))),
        ("party.customer.name", TbValue::Ml(ml("<customer>"))),
        ("party.end_user.name", TbValue::Ml(ml("<end user>"))),
        ("party.end_user.address.street", text("<address>")),
        ("party.end_user.address.postcode", text("<postcode>")),
        ("party.end_user.address.city", text("<city>")),
        ("party.end_user.address.country", text("<country>")),
        ("party.panel_builder.name", TbValue::Ml(ml("<panel builder>"))),
        ("party.engineer.name", TbValue::Ml(ml("<engineer>"))),
    ]
}

#[test]
fn folio_101_full_value_set() {
    let fx = golden();
    let mut want = shared();
    want.extend([
        ("page.folio", text("101")),
        ("page.prev", text("100")),
        ("page.next", text("102")),
        ("page.title", TbValue::Ml(ml("Power Supply 1 - Main Current"))),
        ("page.description", TbValue::Ml(ml("Expansion Control Cabinets"))),
        ("page.function", text("=P01")),
        ("page.location", text("+J-E-2416")),
        ("doc.page_index", text("4")),
        ("doc.page_count", text("8")),
    ]);
    let tb = block(&fx, PageRef::Drawn(id(PAGE101)));
    assert_eq!(tb.values, expected(&want));
    // The checker is unset (doc 20 U-03): the path is absent, not empty.
    assert_eq!(tb.get("revision.current.checker.name"), None);
    assert_eq!(tb.get("page.scale"), None);
}

#[test]
fn folio_201_full_value_set() {
    let fx = golden();
    let mut want = shared();
    want.extend([
        ("page.folio", text("201")),
        ("page.prev", text("200")),
        ("page.title", TbValue::Ml(ml("Valve Manifold Overview 1"))),
        ("page.description", TbValue::Ml(ml("Expansion Control Cabinets"))),
        ("page.function", text("=P01")),
        ("page.location", text("+J-E-2416")),
        ("doc.page_index", text("8")),
        ("doc.page_count", text("8")),
    ]);
    let tb = block(&fx, PageRef::Drawn(id(PAGE201)));
    // The last page has no next page (FR-PAG-013).
    assert_eq!(tb.values, expected(&want));
}

#[test]
fn divider_of_section_1_full_value_set() {
    let fx = golden();
    let mut want = shared();
    want.extend([
        ("page.folio", text("100")),
        ("page.prev", text("3")),
        ("page.next", text("101")),
        (
            "page.title",
            TbValue::Ml(ml(
                "=P01+J-E-2416 - Section 1 - Page 100 - 199 - Wiring Definition",
            )),
        ),
        // Dividers print the project description (doc 09 §2.7).
        ("page.description", TbValue::Ml(ml("Expansion Control Cabinets"))),
        ("page.function", text("=P01")),
        ("page.location", text("+J-E-2416")),
        ("doc.page_index", text("3")),
        ("doc.page_count", text("8")),
    ]);
    let tb = block(&fx, PageRef::Divider(id(SEC1)));
    assert_eq!(tb.values, expected(&want));
}

#[test]
fn first_page_has_no_prev() {
    let tb = block(&golden(), PageRef::Drawn(id(COVER)));
    assert_eq!(tb.get("page.folio"), Some(&text("1")));
    assert_eq!(tb.get("page.prev"), None);
    assert_eq!(tb.get("page.next"), Some(&text("3")));
    assert_eq!(tb.get("doc.page_index"), Some(&text("1")));
    // A page without a structure has no function, location or document kind.
    assert_eq!(tb.get("page.function"), None);
    assert_eq!(tb.get("page.location"), None);
    assert_eq!(tb.get("page.document_kind"), None);
}

#[test]
fn last_page_has_no_next() {
    let fx = golden();
    let m = fx.build();
    let q = Q::new(&m);
    let order = q.get::<PageOrder>(());
    let last = order.entries.last().expect("pages").page;
    let tb = (*q.get::<TitleBlockValues>(last)).clone();
    assert_eq!(tb.get("page.next"), None);
    assert_eq!(tb.get("page.prev"), Some(&text("200")));
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
        Some(&TbValue::Asset(common::sha(7).parse().expect("digest")))
    );
    assert_eq!(tb.get("project.notice"), Some(&TbValue::Ml(ml("<notice>"))));

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

    // Equal dates: the natural order of the label decides (V10.0 after V9.0).
    let mut fx = Fx::new();
    fx.push(json!({"$k": "revision", "id": uid(1), "label": "V9.0", "date": "2025-01-01", "state": "open"}))
        .push(
            json!({"$k": "revision", "id": uid(2), "label": "V10.0", "date": "2025-01-01", "state": "open"}),
        );
    let tb = block(&fx, PageRef::Drawn(id(1)));
    assert_eq!(tb.get("revision.current.label"), Some(&text("V10.0")));
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
