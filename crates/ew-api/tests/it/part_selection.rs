// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.07 part 4a below the shell: the part selection dialog's offers (PS1, PS2; D17), the command a choice
//! compiles to (PS4; D16) and the part editor's draft, refusals, in-place edit and fork (PS5 to PS7; D20, D21), over
//! Golden A and a project made by the wizard, with the fixture catalogue "Fixture parts" written into a temporary user
//! tier. No test pins the number of offers or of starter parts: they assert named rows and their order.

use crate::golden_common;

use std::path::{Path, PathBuf};

use ew_api::library::{LibraryTiers, Registry, starter};
use ew_api::output::{NavKind, NavRole};
use ew_api::{
    AnyCommand, ApiError, AssignMode, CopyState, Fit, NewProjectKind, OfferView, PartDraftError, PartOffer,
    PartOfferQuery, PartPick, PartTarget, ProjectTemplate, Session, SessionOptions,
};
use ew_derive::ew_ids::{Id, Uuid, v5};
use ew_schema::{Device, LibDocument, Part, Record};
use serde_json::{Value, json};

const ZERO: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tiers(root: &Path) -> LibraryTiers {
    LibraryTiers {
        system: Vec::new(),
        community: root.join("data").join("packs"),
        company: Vec::new(),
        user: root.join("data").join("libraries"),
        cache: root.join("cache"),
    }
}

/// The reference to a starter FnDef.
fn fndef(key: &str) -> Value {
    let LibDocument::Fndef(c) = LibDocument::from_json(starter::item(key).unwrap().json).unwrap() else {
        panic!("{key} is not a FnDef")
    };
    json!({"library": starter::LIBRARY.to_string(), "item": c.item_id.as_uuid().to_string(), "version_req": "^0.1"})
}

fn fndef_category(key: &str) -> Value {
    let LibDocument::Fndef(c) = LibDocument::from_json(starter::item(key).unwrap().json).unwrap() else {
        panic!("{key} is not a FnDef")
    };
    serde_json::to_value(&c.item.category).unwrap()
}

fn item(lib: Uuid, number: &str) -> Uuid {
    Uuid::new_v5(&lib, number.as_bytes())
}

/// A fixture part: number, kind, description, template FnDef, CP designations.
type FixturePart = (
    &'static str,
    Option<&'static str>,
    &'static str,
    Option<&'static str>,
    &'static [&'static str],
);

/// The fixture catalogue of §11.3: the manufacturer "Fixture" and six parts.
fn fixture_docs(lib: Uuid) -> Vec<LibDocument> {
    let mfr = item(lib, "manufacturer:Fixture");
    let mut docs = vec![
        LibDocument::from_value(json!({
            "$k": "manufacturer", "id": mfr.to_string(), "library_id": lib.to_string(),
            "item_id": mfr.to_string(), "version": "1.0.0", "sha256": ZERO,
            "key": "Fixture", "name": {"en": "Fixture"},
        }))
        .unwrap(),
    ];
    // (number, kind, description, template FnDef, CP designations)
    let parts: [FixturePart; 6] = [
        (
            "FX-MCB-1PN",
            None,
            "MCB 1P+N C10",
            Some("ew.fn.mcb-1pn"),
            &["1", "2", "N", "N"],
        ),
        ("FX-MCB-1P", None, "MCB 1P C10", Some("ew.fn.mcb-1p"), &["1", "2"]),
        (
            "FX-CON-1",
            None,
            "Contactor 9 A",
            Some("ew.fn.coil"),
            &["A1", "A2"],
        ),
        ("FX-ENDB-1", Some("end_bracket"), "End bracket", None, &[]),
        ("FX-BRG-1", Some("accessory"), "Plug-in bridge 2-pole", None, &[]),
        (
            "FX-MCB-OLD",
            None,
            "MCB 1P+N C10 old",
            Some("ew.fn.mcb-1pn"),
            &["1", "2", "N", "N"],
        ),
    ];
    for (number, kind, description, template, cps) in parts {
        let it = item(lib, number);
        let mut v = json!({
            "$k": "part", "id": it.to_string(), "library_id": lib.to_string(),
            "item_id": it.to_string(), "version": "1.0.0", "sha256": ZERO,
            "meta": {"key": number},
            "manufacturer": {"library": lib.to_string(), "item": mfr.to_string(), "version_req": "^1"},
            "part_number": number,
            "description": {"en": description},
            "cp_pattern": cps.iter().enumerate().map(|(i, d)| json!({"idx": i, "designation": d})).collect::<Vec<_>>(),
            "tech_data": [{"key": "rated_current", "value": {"value": "10", "unit": "A"}}],
        });
        if let Some(k) = kind {
            v["kind"] = json!(k);
        }
        if let Some(t) = template {
            v["category"] = fndef_category(t);
            v["function_template"] =
                json!([{"fndef": fndef(t), "main": true, "cps": (0..cps.len()).collect::<Vec<_>>()}]);
        }
        if number == "FX-MCB-OLD" {
            v["meta"]["status"] = json!({"deprecated": {"successor": {
                "library": lib.to_string(), "item": item(lib, "FX-MCB-1PN").to_string(), "version_req": "^1"}}});
        }
        docs.push(LibDocument::from_value(v).unwrap_or_else(|e| panic!("{number}: {e}")));
    }
    docs
}

/// A temporary user tier holding the fixture catalogue.
fn fixture(name: &str) -> (Dir, LibraryTiers, Uuid) {
    let root = Dir(ew_testdir::fresh_dir(name));
    let t = tiers(&root.0);
    let mut reg = Registry::open(&t).unwrap();
    let lib = reg
        .create(
            &root.0.join("data").join("libraries").join("fixture.ewlib"),
            "fixture",
            "Fixture parts",
            "CC0-1.0",
        )
        .unwrap();
    reg.write(lib, &fixture_docs(lib)).unwrap();
    (root, t, lib)
}

fn options(libraries: Option<LibraryTiers>) -> SessionOptions {
    SessionOptions {
        libraries,
        ..SessionOptions::default()
    }
}

/// Golden A with the libraries of `tiers`.
fn golden(tiers: Option<LibraryTiers>) -> Session {
    let batch = golden_common::batch();
    let mut project = None;
    let mut records = Vec::new();
    for r in &batch.records {
        match r {
            Record::Project(p) => project = Some(p.clone()),
            other => records.push(other.clone()),
        }
    }
    Session::from_records(project.unwrap(), records, batch.lib.clone(), options(tiers)).unwrap()
}

fn device(s: &Session, tag: &str) -> Uuid {
    golden_common::tags(s)
        .into_iter()
        .find(|(_, t)| t == tag)
        .unwrap_or_else(|| panic!("no device {tag}"))
        .0
        .as_uuid()
}

fn copy_of(s: &Session, number: &str) -> Uuid {
    s.query()
        .parts()
        .iter()
        .find(|(_, p)| p.item.part_number.as_str() == number)
        .unwrap_or_else(|| panic!("no copy {number}"))
        .0
        .as_uuid()
}

fn numbers(offers: &[PartOffer]) -> Vec<&str> {
    offers.iter().map(|o| o.part_number.as_str()).collect()
}

fn offer<'o>(offers: &'o [PartOffer], number: &str) -> &'o PartOffer {
    offers
        .iter()
        .find(|o| o.part_number == number)
        .unwrap_or_else(|| panic!("no offer {number}: {:?}", numbers(offers)))
}

fn lines(s: &Session) -> Vec<String> {
    ew_commands::rows::model_lines(&s.world().model)
        .unwrap()
        .into_iter()
        .map(|(_, l)| l)
        .collect()
}

fn commit(s: &mut Session, cmd: &AnyCommand) {
    let preview = s.preview(cmd).unwrap();
    s.commit(preview).unwrap();
}

fn part_uses(s: &Session, d: Uuid) -> Vec<Uuid> {
    s.query()
        .devices()
        .get(Id::<Device>::from_uuid(d))
        .unwrap()
        .parts
        .iter()
        .map(|u| u.part.copy.as_uuid())
        .collect()
}

/// F55's main function, whose FnDef the golden fact of PS1 names.
fn f55_function(s: &Session) -> Uuid {
    let (function, key) = s.main_function(device(s, "F55")).unwrap();
    assert_eq!(
        key, "ew.fn.mcb-1pn",
        "a golden fact: F55's FnDef is the starter's ew.fn.mcb-1pn"
    );
    function
}

/// PS1: F55's offers: the 1P+N parts (same FnDef), then the 1P parts (same category), then every other part, held
/// copies first; the deprecated part is absent; the Golden A copies with a template (terminals, black boxes) rank
/// `other`.
#[test]
fn ps1_f55_offers_the_same_function_first() {
    let (_dir, t, _) = fixture("ew-api-ps1");
    let s = golden(Some(t));
    let q = PartOfferQuery {
        function: Some(f55_function(&s)),
        ..PartOfferQuery::default()
    };
    let offers = s.part_offers(&q);
    let head: Vec<(&str, Fit)> = offers[..4]
        .iter()
        .map(|o| (o.part_number.as_str(), o.fit))
        .collect();
    assert_eq!(
        head,
        [
            ("EW-GEN-MCB-2", Fit::SameFnDef),
            ("FX-MCB-1PN", Fit::SameFnDef),
            ("EW-GEN-MCB-1", Fit::SameCategory),
            ("FX-MCB-1P", Fit::SameCategory),
        ]
    );
    let rest = &offers[4..];
    assert!(rest.iter().all(|o| o.fit == Fit::Other), "{:?}", numbers(rest));
    let first_not_held = rest.iter().position(|o| !o.held).unwrap();
    assert!(
        rest[first_not_held..].iter().all(|o| !o.held),
        "held copies first"
    );
    assert!(!numbers(&offers).contains(&"FX-MCB-OLD"));
    for number in ["PT 2.5-TWIN", "PT 2.5-TWIN-PE"] {
        let o = offer(&offers, number);
        assert_eq!((o.held, o.fit), (true, Fit::Other), "{number}");
    }
    // 5SY4204-7, F55's part, is project-local in Golden A: held, no library, other.
    let mcb = offer(&offers, "5SY4204-7");
    assert_eq!(
        (mcb.held, &mcb.library, mcb.item, mcb.fit),
        (true, &None, None, Fit::Other)
    );
    // The catalogue part names its library.
    let fx = offer(&offers, "FX-MCB-1PN");
    assert_eq!(fx.library.as_ref().map(|l| l.1.as_str()), Some("Fixture parts"));
    // Only parts that fit hides `other`.
    let fits = s.part_offers(&PartOfferQuery {
        fits_only: true,
        ..q.clone()
    });
    assert_eq!(numbers(&fits), numbers(&offers[..4]));
    // Without a function every row is `other`, in rank, held and part-number order.
    let plain = s.part_offers(&PartOfferQuery::default());
    assert!(plain.iter().all(|o| o.fit == Fit::Other));
    assert!(plain[0].held);
}

/// PS2: the text, the kind, the category, the deprecated parts, the views; a held library item is listed once.
#[test]
fn ps2_search_and_filters() {
    let (_dir, t, lib) = fixture("ew-api-ps2");
    let mut s = Session::new_project(
        ProjectTemplate::Builtin,
        NewProjectKind::Schematic,
        options(Some(t)),
    )
    .unwrap();
    let embed = s
        .embed_command(&[serde_json::from_value(json!({
            "library": lib.to_string(), "item": item(lib, "FX-MCB-1PN").to_string(), "version_req": "^1"}))
        .unwrap()])
        .unwrap();
    s.execute(&embed).unwrap();
    let find = |q: PartOfferQuery| s.part_offers(&q);
    let text = |t: &str| PartOfferQuery {
        text: t.into(),
        ..PartOfferQuery::default()
    };
    let exact = find(text("FX-MCB-1P"));
    assert_eq!(
        numbers(&exact)[..2],
        ["FX-MCB-1P", "FX-MCB-1PN"],
        "exact before prefix"
    );
    let words = find(PartOfferQuery {
        include_inactive: true,
        ..text("mcb 1p+n")
    });
    for n in ["FX-MCB-1PN", "FX-MCB-OLD"] {
        assert!(numbers(&words).contains(&n), "{n}: {:?}", numbers(&words));
    }
    let brackets = find(PartOfferQuery {
        kinds: vec!["end_bracket".into()],
        ..PartOfferQuery::default()
    });
    assert!(numbers(&brackets).contains(&"FX-ENDB-1"));
    assert!(
        brackets.iter().all(|o| o.kind == "end_bracket"),
        "{:?}",
        numbers(&brackets)
    );
    let mcbs = find(PartOfferQuery {
        category: vec!["ProtectionOvercurrent".into(), "MCB".into()],
        ..PartOfferQuery::default()
    });
    assert!(numbers(&mcbs).contains(&"FX-MCB-1P"));
    assert!(
        mcbs.iter().all(|o| o.part_number.contains("MCB")),
        "{:?}",
        numbers(&mcbs)
    );
    let plain = find(PartOfferQuery::default());
    assert!(!numbers(&plain).contains(&"FX-MCB-OLD"));
    let all = find(PartOfferQuery {
        include_inactive: true,
        ..PartOfferQuery::default()
    });
    let old = offer(&all, "FX-MCB-OLD");
    assert_eq!(
        (old.status.as_str(), old.successor.as_deref()),
        ("deprecated", Some("FX-MCB-1PN"))
    );
    let held = find(PartOfferQuery {
        view: OfferView::Project,
        ..PartOfferQuery::default()
    });
    assert!(!held.is_empty() && held.iter().all(|o| o.held));
    let once: Vec<&PartOffer> = plain.iter().filter(|o| o.part_number == "FX-MCB-1PN").collect();
    assert_eq!(once.len(), 1, "a held library item is listed once");
    assert!(once[0].held && matches!(once[0].pick, PartPick::Held(_)));
    // Favourites and recently used parts: listed by their views, and boosted in All among the not-held offers.
    let fav = (lib, item(lib, "FX-MCB-1P"));
    let recent = vec![(lib, item(lib, "FX-CON-1")), (lib, item(lib, "FX-ENDB-1"))];
    let q = PartOfferQuery {
        favourites: vec![fav],
        recent: recent.clone(),
        ..PartOfferQuery::default()
    };
    let boosted = find(q.clone());
    let first_catalogue = boosted.iter().position(|o| !o.held).unwrap();
    assert_eq!(
        boosted[first_catalogue].part_number, "FX-MCB-1P",
        "the favourite leads the catalogue"
    );
    assert!(boosted[first_catalogue].favourite);
    assert_eq!(
        numbers(&find(PartOfferQuery {
            view: OfferView::Favourites,
            ..q.clone()
        })),
        ["FX-MCB-1P"]
    );
    assert_eq!(
        numbers(&find(PartOfferQuery {
            view: OfferView::Recent,
            ..q
        })),
        ["FX-CON-1", "FX-ENDB-1"],
        "most recent first"
    );
}

/// PS4: a choice compiles to one command: `part.assign` (replace, append, a held copy without `copies`) and the bridge
/// part's `fields.set` with `copies`; each is one undo step.
#[test]
fn ps4_choosing_commits_one_command() {
    let (_dir, t, lib) = fixture("ew-api-ps4");
    let mut s = golden(Some(t));
    let f55 = device(&s, "F55");
    let mcb = copy_of(&s, "5SY4204-7");
    let offers = s.part_offers(&PartOfferQuery {
        function: Some(f55_function(&s)),
        ..PartOfferQuery::default()
    });
    let generic = offer(&offers, "EW-GEN-MCB-2").pick.clone();
    let replace = PartTarget::Devices {
        devices: vec![f55],
        mode: AssignMode::Replace { from: mcb },
    };
    let cmd = s.part_pick_command(&replace, &generic).unwrap();
    assert_eq!(cmd.name, "part.assign");
    assert_eq!(
        (&cmd.params["mode"], &cmd.params["from"]),
        (&json!("replace"), &json!(mcb.to_string()))
    );
    let copies = cmd.params["copies"].as_array().unwrap();
    let kinds: Vec<&str> = copies.iter().map(|c| c["$k"].as_str().unwrap()).collect();
    assert!(
        kinds.contains(&"part") && kinds.contains(&"manufacturer"),
        "{kinds:?}"
    );
    assert!(
        copies.iter().any(|c| c["key"] == "Generic"),
        "the manufacturer Generic travels"
    );
    let before = lines(&s);
    s.execute(&cmd).unwrap();
    let PartPick::Library { library, item: it } = generic else {
        panic!("a catalogue pick")
    };
    let new_copy = v5::lib_copy_id::<Part>(library, it).as_uuid();
    assert_eq!(part_uses(&s, f55), [new_copy]);
    s.undo(&ew_api::Origin::Gui).unwrap();
    assert_eq!(lines(&s), before, "one undo step back");
    // Add part… appends; a held copy travels without `copies`.
    let append = PartTarget::Devices {
        devices: vec![f55],
        mode: AssignMode::Append,
    };
    let appended = s.part_pick_command(&append, &generic).unwrap();
    assert!(
        appended.params.get("mode").is_none_or(|m| m == "append"),
        "{appended:?}"
    );
    let held = s.part_pick_command(&append, &PartPick::Held(mcb)).unwrap();
    assert_eq!(held.name, "part.assign");
    assert!(
        held.params
            .get("copies")
            .is_none_or(|c| c.as_array().is_some_and(Vec::is_empty))
    );
    assert!(matches!(
        s.part_pick_command(&append, &PartPick::Held(Uuid::nil())),
        Err(ApiError::NotHeld(_))
    ));
    // BR1's bridge part from the catalogue: one `fields.set` with `copies`, one undo step.
    let t1 = strip_terminal(&s, "-X1031", "1");
    let bridge = PartPick::Library {
        library: lib,
        item: item(lib, "FX-BRG-1"),
    };
    let cmd = s
        .part_pick_command(&PartTarget::BridgePart { terminals: vec![t1] }, &bridge)
        .unwrap();
    assert_eq!(
        (cmd.name.as_str(), &cmd.params["path"]),
        ("fields.set", &json!("overrides.wire_part"))
    );
    let embedded: Vec<&str> = cmd.params["copies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["$k"].as_str().unwrap())
        .collect();
    assert_eq!(embedded, ["part", "manufacturer"]);
    let before = lines(&s);
    s.execute(&cmd).unwrap();
    assert!(
        s.query()
            .parts()
            .iter()
            .any(|(_, p)| p.item.part_number.as_str() == "FX-BRG-1")
    );
    s.undo(&ew_api::Origin::Gui).unwrap();
    assert_eq!(lines(&s), before);
    // An MCB is not a bridge part.
    let wrong = PartPick::Library {
        library: lib,
        item: item(lib, "FX-MCB-1P"),
    };
    assert!(matches!(
        s.part_pick_command(&PartTarget::BridgePart { terminals: vec![t1] }, &wrong),
        Err(ApiError::Strip(_))
    ));
}

fn strip_terminal(s: &Session, strip: &str, designation: &str) -> Uuid {
    let t = s.navigator_table(NavKind::Strips);
    let at = t
        .nodes
        .iter()
        .position(|n| n.role == NavRole::Strip && n.label == strip)
        .unwrap();
    t.nodes
        .iter()
        .find(|n| {
            n.parent == Some(u32::try_from(at).unwrap())
                && n.role == NavRole::Terminal
                && n.label == designation
        })
        .unwrap()
        .records[0]
}

/// A project made by the wizard with the golden parts table imported (project-local copies).
fn wizard_with_parts() -> Session {
    let mut s =
        Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Schematic, options(None)).unwrap();
    let project = s.query().project().id.as_uuid();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416/tables/parts.csv");
    let parts = ew_api::import::read_parts_tables(&path, project).unwrap();
    commit(
        &mut s,
        &AnyCommand {
            name: ew_api::import::PARTS_IMPORT.into(),
            params: parts.params(),
        },
    );
    s
}

fn field_error(e: PartDraftError) -> (String, &'static str, Vec<String>) {
    match e {
        PartDraftError::Field(m) => (m.field, m.key, m.args),
        other => panic!("expected a field message, got {other:?}"),
    }
}

/// PS5: the draft refuses each case of D21 under its field and as the save's refusal; the history is unchanged.
#[test]
fn ps5_the_part_editor_refuses_with_a_message() {
    let s = wizard_with_parts();
    let copy = copy_of(&s, "5SY4204-7");
    let history = s.history().entries().len();
    let refused = |field: &str, text: &str| {
        let mut d = s.part_draft(copy).unwrap();
        let typed = s.set_draft_field(&mut d, field, text).unwrap_err();
        assert_eq!(d.text(field), text, "the text is kept");
        assert_eq!(
            d.message(field).cloned(),
            Some(match &typed {
                PartDraftError::Field(m) => m.clone(),
                PartDraftError::Refused(_) => panic!("a field"),
            })
        );
        let saved = s.part_edit_command(&d).unwrap_err();
        assert_eq!(saved, typed, "Save refuses with the field's message");
        field_error(typed)
    };
    assert_eq!(refused("manufacturer", "").1, "reason-part-no-manufacturer");
    assert_eq!(refused("manufacturer", "Nobody").1, "error-part-manufacturer");
    assert_eq!(refused("part_number", "  ").1, "reason-part-no-number");
    assert_eq!(refused("unit", "pcs").1, "reason-part-unit");
    let (field, key, args) = refused("tech:rated_current", "4 Amp");
    assert_eq!(
        (field.as_str(), key),
        ("tech:rated_current", "error-part-tech-value")
    );
    assert_eq!(args[0], "rated_current");
    assert!(args[1].contains("has no unit"), "{args:?}");
    // A key given twice (up to case).
    let mut d = s.part_draft(copy).unwrap();
    s.set_draft_field(&mut d, "tech:rated_current", "4 A").unwrap();
    let (_, key, _) = field_error(
        s.set_draft_field(&mut d, "tech:Rated_Current", "6 A")
            .unwrap_err(),
    );
    assert_eq!(key, "error-part-tech-twice");
    // Unchanged.
    let d = s.part_draft(copy).unwrap();
    assert_eq!(
        s.part_edit_command(&d).unwrap_err(),
        PartDraftError::Refused("reason-part-unchanged")
    );
    assert_eq!(s.history().entries().len(), history, "nothing committed");
    // A project-local copy is edited in place: no fork.
    assert!(!d.fork && d.library.is_none());
}

/// PS6: an edit in place: the plan's field diff, devices and BOM rows; one undo step; the copy ID, item ID and version
/// unchanged and the hash recomputed.
#[test]
fn ps6_an_edit_in_place_is_previewed() {
    let mut s = golden(None);
    let copy = copy_of(&s, "5SY4204-7");
    let f55 = device(&s, "F55");
    let held = s
        .query()
        .parts()
        .get(Id::from_uuid(copy))
        .map(|c| (**c).clone())
        .unwrap();
    let mut d = s.part_draft(copy).unwrap();
    assert_eq!(d.text("part_number"), "5SY4204-7");
    assert!(!d.fork, "Golden A's part copies are project-local");
    s.set_draft_field(&mut d, "description", "MCB 1P+N C4").unwrap();
    let plan = s.part_edit_plan(&d).unwrap();
    assert_eq!(plan.command.name, "part.edit_copy");
    assert_eq!(plan.fork, None);
    assert_eq!(
        plan.fields.iter().map(|f| f.member.as_str()).collect::<Vec<_>>(),
        ["description"]
    );
    assert!(plan.devices.iter().any(|x| x.as_uuid() == f55));
    // The BOM rows that change are 5SY4204-7's, where they print its description.
    assert!(!plan.bom_added.is_empty() && plan.bom_added.len() == plan.bom_removed.len());
    assert!(
        plan.bom_added
            .iter()
            .chain(&plan.bom_removed)
            .all(|r| r.contains("5SY4204-7")),
        "{:?} {:?}",
        plan.bom_removed,
        plan.bom_added
    );
    assert!(plan.bom_added.iter().all(|r| r.contains("MCB 1P+N C4")));
    let before = lines(&s);
    commit(&mut s, &plan.command);
    let after = s
        .query()
        .parts()
        .get(Id::from_uuid(copy))
        .map(|c| (**c).clone())
        .unwrap();
    assert_eq!(
        (after.id, after.item_id, &after.version),
        (held.id, held.item_id, &held.version)
    );
    assert_ne!(after.sha256, held.sha256);
    assert_eq!(
        Some(after.sha256.0),
        ew_commands::commands::library_copy_hash(&LibDocument::Part(after.clone()))
    );
    s.undo(&ew_api::Origin::Gui).unwrap();
    assert_eq!(lines(&s), before, "undo restores the copy");
}

/// PS7: a copy from a library forks: the fork carries `fork_of`, F55's part use names it, the original stays, the
/// Libraries status says Local fork; undo restores; a second edit of the fork is in place.
#[test]
fn ps7_a_library_copy_forks() {
    let mut s = golden(None);
    let f55 = device(&s, "F55");
    let mcb = copy_of(&s, "5SY4204-7");
    let generic = s
        .part_offers(&PartOfferQuery {
            text: "EW-GEN-MCB-2".into(),
            ..PartOfferQuery::default()
        })
        .into_iter()
        .find(|o| o.part_number == "EW-GEN-MCB-2")
        .unwrap()
        .pick;
    let cmd = s
        .part_pick_command(
            &PartTarget::Devices {
                devices: vec![f55],
                mode: AssignMode::Replace { from: mcb },
            },
            &generic,
        )
        .unwrap();
    s.execute(&cmd).unwrap();
    let original = copy_of(&s, "EW-GEN-MCB-2");
    let held = s
        .query()
        .parts()
        .get(Id::from_uuid(original))
        .map(|c| (**c).clone())
        .unwrap();
    let mut d = s.part_draft(original).unwrap();
    assert!(d.fork && d.library.is_some());
    s.set_draft_field(&mut d, "description", "MCB 1P+N C4").unwrap();
    let plan = s.part_edit_plan(&d).unwrap();
    let fork_item = s.next_fork_item(original);
    assert_eq!(plan.fork, Some(fork_item));
    let before = lines(&s);
    commit(&mut s, &plan.command);
    let project = s.query().project().id.as_uuid();
    let fork_copy = v5::lib_copy_id::<Part>(project, fork_item).as_uuid();
    let fork = s
        .query()
        .parts()
        .get(Id::from_uuid(fork_copy))
        .map(|c| (**c).clone())
        .unwrap();
    assert_eq!(fork.library_id.as_uuid(), project);
    assert_eq!(fork.version, held.version);
    assert_eq!(
        fork.item.meta.extra["source"]["fork_of"],
        json!({"library": held.library_id.as_uuid().to_string(), "item": held.item_id.as_uuid().to_string(),
               "version": held.version})
    );
    assert_eq!(part_uses(&s, f55), [fork_copy], "F55's part use names the fork");
    let still = s
        .query()
        .parts()
        .get(Id::from_uuid(original))
        .map(|c| (**c).clone())
        .unwrap();
    assert_eq!(still, held, "the original copy is unchanged");
    let status = s.library_status();
    let row = status
        .iter()
        .find(|r| r.copy == fork_copy)
        .expect("the fork is listed");
    assert_eq!(row.state, CopyState::LocalFork);
    // A second edit of the fork is in place.
    let mut d2 = s.part_draft(fork_copy).unwrap();
    assert!(!d2.fork);
    s.set_draft_field(&mut d2, "description", "MCB 1P+N C6").unwrap();
    let plan2 = s.part_edit_plan(&d2).unwrap();
    assert_eq!(plan2.fork, None);
    commit(&mut s, &plan2.command);
    assert_eq!(part_uses(&s, f55), [fork_copy]);
    s.undo(&ew_api::Origin::Gui).unwrap();
    s.undo(&ew_api::Origin::Gui).unwrap();
    assert_eq!(lines(&s), before, "undo restores");
    // The next fork skips a held fork item.
    commit(&mut s, &plan.command);
    assert_ne!(s.next_fork_item(original), fork_item);
}

// WP-S2.07 part 4b

/// FX-MCB-1PN of the fixture catalogue at `version`, its description `description` and its CP designations `cps`.
fn mcb_version(lib: Uuid, version: &str, description: &str, cps: &[&str]) -> LibDocument {
    let doc = fixture_docs(lib)
        .into_iter()
        .find(|d| matches!(d, LibDocument::Part(c) if c.item.part_number.as_str() == "FX-MCB-1PN"))
        .unwrap();
    let mut v = serde_json::to_value(&doc).unwrap();
    v["version"] = json!(version);
    v["description"] = json!({"en": description});
    v["cp_pattern"] = json!(
        cps.iter()
            .enumerate()
            .map(|(i, d)| json!({"idx": i, "designation": d}))
            .collect::<Vec<_>>()
    );
    v["function_template"][0]["cps"] = json!((0..cps.len()).collect::<Vec<_>>());
    LibDocument::from_value(v).unwrap()
}

/// Writes `doc` into the fixture library (the next version of its item).
fn publish(t: &LibraryTiers, lib: Uuid, doc: LibDocument) {
    let mut reg = Registry::open(t).unwrap();
    reg.write(lib, &[doc]).unwrap();
}

/// A copy of Golden A whose F55 uses FX-MCB-1PN as the fixture library holds it now (its first use replaced).
fn holding_mcb(t: &LibraryTiers, lib: Uuid) -> Session {
    let mut s = golden(Some(t.clone()));
    let f55 = device(&s, "F55");
    let from = copy_of(&s, "5SY4204-7");
    let cmd = s
        .part_pick_command(
            &PartTarget::Devices {
                devices: vec![f55],
                mode: AssignMode::Replace { from },
            },
            &PartPick::Library {
                library: lib,
                item: item(lib, "FX-MCB-1PN"),
            },
        )
        .unwrap();
    s.execute(&cmd).unwrap();
    s
}

/// F55's representations on folio 101 as a fragment (`edit.paste`'s `fragment` member).
fn f55_fragment(s: &Session) -> Value {
    let f55 = device(s, "F55");
    let reps: Vec<Uuid> = s
        .page_tables("101")
        .unwrap()
        .reps
        .iter()
        .filter(|r| r.device == Some(f55))
        .map(|r| r.id)
        .collect();
    assert!(!reps.is_empty(), "F55 is drawn on 101");
    let copied = s.copy_fragment(&reps).unwrap();
    ew_api::output::parse_fragment(&copied.text).unwrap().params
}

/// `edit.paste` of `fragment` on folio 101 at its own anchor, with `adapt`.
fn paste_cmd(s: &Session, fragment: &Value, adapt: &[Uuid]) -> AnyCommand {
    let page = s.page_tables("101").unwrap().page;
    let mut params = json!({"page": page.to_string(), "at": fragment["anchor"], "fragment": fragment});
    if !adapt.is_empty() {
        params["adapt"] = json!(adapt.iter().map(ToString::to_string).collect::<Vec<_>>());
    }
    AnyCommand {
        name: "edit.paste".into(),
        params,
    }
}

fn the_copy(s: &Session, copy: Uuid) -> (String, String) {
    let q = s.query();
    let c = q.parts().get(Id::<Part>::from_uuid(copy)).unwrap();
    (
        c.version.clone(),
        serde_json::to_value(&c.item.description).unwrap().to_string(),
    )
}

/// PS8 below the shell (D22, FT-396): P1 holds FX-MCB-1PN 1.0.0 on F55, P2 holds 1.1.0 (a description change, a
/// patch). F55 copied in P1 and pasted in P2: `paste_versions` lists the copy as a patch, not newer; `edit.paste`
/// refuses without `adapt` and with it reuses P2's copy (one undo step). Pasted the other way the pasted version is
/// newer: `paste_update` plans `library.update` from the fragment's document, and after it the paste needs no choice
/// (two commands, two undo steps; P1 holds 1.1.0). A version that drops a CP makes "Use the project's copy" major,
/// and `edit.paste` refuses that `adapt` at `/adapt/0`.
#[test]
fn ps8_the_paste_choice_below_the_shell() {
    let (_dir, t, lib) = fixture("ew-api-ps8");
    let copy = v5::lib_copy_id::<Part>(lib, item(lib, "FX-MCB-1PN")).as_uuid();
    let mut p1 = holding_mcb(&t, lib);
    publish(
        &t,
        lib,
        mcb_version(lib, "1.1.0", "MCB 1P+N C10, new text", &["1", "2", "N", "N"]),
    );
    let mut p2 = holding_mcb(&t, lib);
    assert_eq!(the_copy(&p1, copy).0, "1.0.0");
    assert_eq!(the_copy(&p2, copy).0, "1.1.0");

    // P1 → P2: the pasted version is older.
    let from_p1 = f55_fragment(&p1);
    let conflicts = p2.paste_versions(&from_p1);
    assert_eq!(conflicts.len(), 1, "{conflicts:?}");
    let c = &conflicts[0];
    assert_eq!(c.copy, copy);
    assert_eq!(c.title, "FX-MCB-1PN (Fixture)");
    assert_eq!((c.held.as_str(), c.pasted.as_str()), ("1.1.0", "1.0.0"));
    assert_eq!(c.class, ew_api::ChangeClass::Patch);
    assert!(!c.newer);
    assert!(
        p1.paste_versions(&from_p1).is_empty(),
        "no choice in the source project"
    );
    let cmd = paste_cmd(&p2, &from_p1, &[]);
    let refused = p2.preview(&cmd);
    assert!(
        refused.is_err_and(|e| e.to_string().contains("update from library first")),
        "the plain paste is refused"
    );
    let before = lines(&p2);
    let devices = p2.query().devices().len();
    let cmd = paste_cmd(&p2, &from_p1, &[copy]);
    commit(&mut p2, &cmd);
    let pasted: Vec<Uuid> = {
        let q = p2.query();
        q.devices()
            .iter()
            .filter(|(_, d)| d.parts.iter().any(|u| u.part.copy.as_uuid() == copy))
            .map(|(id, _)| id.as_uuid())
            .collect()
    };
    assert_eq!(p2.query().devices().len(), devices + 1);
    assert_eq!(pasted.len(), 2, "F55 and the pasted device use P2's copy");
    assert_eq!(the_copy(&p2, copy).0, "1.1.0", "the project's copy stays");
    p2.undo(&ew_api::Origin::Gui).unwrap();
    assert_eq!(lines(&p2), before, "one undo step");

    // P2 → P1: the pasted version is newer; Update the project's copy, then paste.
    let from_p2 = f55_fragment(&p2);
    let conflicts = p1.paste_versions(&from_p2);
    assert_eq!(conflicts.len(), 1);
    assert!(conflicts[0].newer);
    let before = lines(&p1);
    let plan = p1.paste_update(&from_p2).unwrap();
    assert_eq!(plan.command.name, "library.update");
    assert_eq!(plan.copies.len(), 1);
    assert_eq!(
        (plan.copies[0].from.as_str(), plan.copies[0].to.as_str()),
        ("1.0.0", "1.1.0")
    );
    commit(&mut p1, &plan.command);
    assert_eq!(the_copy(&p1, copy).0, "1.1.0");
    assert!(
        p1.paste_versions(&from_p2).is_empty(),
        "the paste needs no choice now"
    );
    let cmd = paste_cmd(&p1, &from_p2, &[]);
    commit(&mut p1, &cmd);
    p1.undo(&ew_api::Origin::Gui).unwrap();
    assert_eq!(the_copy(&p1, copy).0, "1.1.0", "the paste is its own undo step");
    p1.undo(&ew_api::Origin::Gui).unwrap();
    assert_eq!(lines(&p1), before, "two undo steps");
    // Nothing to update in a project that holds the pasted version.
    assert!(matches!(p2.paste_update(&from_p2), Err(ApiError::NothingToDo)));

    // A version that drops a CP: adapting P1's fragment to it is major.
    publish(&t, lib, mcb_version(lib, "1.2.0", "MCB 1P C10", &["1", "2"]));
    let mut p3 = holding_mcb(&t, lib);
    let conflicts = p3.paste_versions(&from_p1);
    assert_eq!(conflicts[0].class, ew_api::ChangeClass::Major, "{conflicts:?}");
    let cmd = paste_cmd(&p3, &from_p1, &[copy]);
    match p3.preview(&cmd) {
        Err(e) => assert!(e.to_string().contains("/adapt/0"), "{e}"),
        Ok(_) => panic!("a major adapt is refused"),
    }
}
