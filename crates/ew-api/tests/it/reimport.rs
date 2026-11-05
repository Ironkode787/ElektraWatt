// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S3.09 step 6: a QET file re-imported into its lineage (D10 to D12, IC-12). The same file again is unchanged and
//! writes nothing; a variant with one more terminal after an edit here adds the terminal and its wire and keeps the
//! edit; a record changed on both sides is kept with EW-IMP-011, a record removed in the source is kept with
//! EW-IMP-010; the IDs of unchanged source keys stay; a file of another project is refused (T7).

use std::path::Path;

use ew_api::import::{Detected, ReimportClass, ReimportError, StarterLibrary, import_qet_with};
use ew_api::{AnyCommand, Origin, Session, SessionOptions};
use ew_schema::Record;
use serde_json::json;

use crate::qet_lenient::{El, SIMPLE, TERMINAL, el, folio, qet, uuid};

const FILE: &str = "r.qet";

fn library() -> StarterLibrary {
    StarterLibrary::from_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content"))
        .expect("the starter content")
}

fn k1(label: &'static str) -> El {
    el(SIMPLE, uuid(1), 100, 100).label(label)
}

fn k2() -> El {
    el(SIMPLE, uuid(2), 200, 100).label("-K2")
}

/// Folio 1 with -K1 and -K2 and a wire between them.
fn base_text() -> String {
    qet(
        r#"version="0.100""#,
        &[folio("1", "Base", vec![k1("-K1"), k2()]).wire((&uuid(1), SIMPLE, 1), (&uuid(2), SIMPLE, 0))],
    )
}

/// The base with one more terminal, -X1, wired to -K2.
fn variant_text() -> String {
    qet(
        r#"version="0.100""#,
        &[folio(
            "1",
            "Base",
            vec![k1("-K1"), k2(), el(TERMINAL, uuid(3), 300, 100).label("-X1")],
        )
        .wire((&uuid(1), SIMPLE, 1), (&uuid(2), SIMPLE, 0))
        .wire((&uuid(2), SIMPLE, 1), (&uuid(3), TERMINAL, 0))],
    )
}

fn imported(text: &str) -> Session {
    let options = SessionOptions {
        ids: Some(ew_testids::seeded_ids(5)),
        ..SessionOptions::default()
    };
    import_qet_with(FILE, text.as_bytes().to_vec(), &library(), options)
        .expect("the file imports")
        .session
        .expect("import.apply commits the batch")
}

fn device(s: &Session, tag: &str) -> ew_schema::Device {
    let q = s.query();
    q.table::<ew_schema::Device>()
        .iter()
        .map(|(_, d)| (**d).clone())
        .find(|d| {
            q.get::<ew_derive::DeviceTag>(d.id)
                .as_ref()
                .as_ref()
                .map(ew_derive::Tag::short)
                .as_deref()
                == Some(tag)
        })
        .unwrap_or_else(|| panic!("{tag} is in the project"))
}

fn text_en(t: &ew_mltext::MlText) -> Option<&str> {
    t.get(&"en".parse().expect("a language tag"))
}

fn edit_function_text(s: &mut Session, tag: &str, text: &str) {
    let id = device(s, tag).id;
    s.execute(&AnyCommand {
        name: "fields.set".into(),
        params: json!({ "kind": "device", "targets": [id.to_string()], "path": "function_text", "value": {"en": text} }),
    })
    .expect("fields.set function_text");
}

fn model(s: &Session) -> ew_model::Model {
    s.world().model
}

#[test]
fn the_same_file_again_is_unchanged_and_writes_nothing() {
    let mut s = imported(&base_text());
    let before = model(&s);
    let before_rev = s.world().rev;
    let detected = s
        .detect_qet_lineage(FILE, base_text().into_bytes(), &library())
        .expect("its own lineage");
    assert!(matches!(detected, Detected::Lineage(_)), "{detected:?}");
    assert_eq!(detected.question(FILE), None);
    let r = s
        .reimport_qet(FILE, base_text().into_bytes(), &library(), detected.lineage())
        .expect("the re-import is planned");
    let records = r
        .batch
        .records
        .iter()
        .filter(|r| !matches!(r, Record::Project(_)))
        .count();
    assert!(records > 0);
    assert_eq!(
        r.summary(),
        format!(
            "Re-import of r.qet: 0 added, 0 changed in the source, 0 kept with your changes, 0 changed on both sides and kept, 0 removed in the source and kept, {records} unchanged."
        )
    );
    // The IDs of the unchanged source keys are the project's.
    for record in r
        .batch
        .records
        .iter()
        .filter(|r| !matches!(r, Record::Project(_)))
    {
        let id = record.id().expect("an ID");
        assert!(
            r.plan.rows.iter().any(|row| row.key.1.as_uuid() == id),
            "{record:?}"
        );
    }
    assert!(s.apply_reimport(r).expect("nothing to apply").is_none());
    assert!(model(&s).content_eq(&before), "nothing was written");
    assert_eq!(s.world().rev, before_rev);
}

/// ID-1374 (C51-p2#10): a file of the project's own lineage is imported once: the batch the detection made is the
/// batch of an import into the lineage, so a job runs the importer once; a proposed lineage gives no batch.
#[test]
fn the_detection_of_the_own_lineage_gives_the_re_import_batch() {
    use ew_api::import::{detect_qet_reimport, reimport_qet_batch};
    let s = imported(&base_text());
    let lineages = s.qet_lineages().expect("the lineages");
    let (detected, batch) =
        detect_qet_reimport(&lineages, FILE, base_text().into_bytes(), &library()).expect("its own lineage");
    assert!(matches!(detected, Detected::Lineage(_)), "{detected:?}");
    let into = reimport_qet_batch(
        &lineages,
        FILE,
        base_text().into_bytes(),
        &library(),
        detected.lineage(),
    )
    .expect("imported into the lineage");
    assert_eq!(batch, Some(into));
    let (proposed, none) =
        detect_qet_reimport(&lineages, FILE, variant_text().into_bytes(), &library()).expect("proposed");
    assert!(matches!(proposed, Detected::Proposed { .. }), "{proposed:?}");
    assert_eq!(none, None);
}

#[test]
fn a_variant_adds_its_terminal_and_keeps_the_edit_here() {
    let mut s = imported(&base_text());
    edit_function_text(&mut s, "-K1", "Main contactor");
    let detected = s
        .detect_qet_lineage(FILE, variant_text().into_bytes(), &library())
        .expect("the earlier import is proposed");
    let Detected::Proposed { percent, .. } = &detected else {
        panic!("a file without a project UUID is matched by its source keys: {detected:?}");
    };
    assert_eq!(
        detected.question(FILE),
        Some(format!(
            "r.qet matches the earlier import of r.qet ({percent} % of its items). Re-import it into that import?"
        ))
    );
    let r = s
        .reimport_qet(FILE, variant_text().into_bytes(), &library(), detected.lineage())
        .expect("the re-import is planned");
    let classes: Vec<(ReimportClass, String)> = r
        .plan
        .rows
        .iter()
        .filter(|row| row.class != ReimportClass::Unchanged)
        .map(|row| (row.class, row.record.to_string()))
        .collect();
    assert_eq!(r.plan.count(ReimportClass::SourceChanged), 0, "{classes:?}");
    assert_eq!(r.plan.count(ReimportClass::Conflict), 0, "{classes:?}");
    assert_eq!(r.plan.count(ReimportClass::Removed), 0, "{classes:?}");
    assert_eq!(
        classes
            .iter()
            .filter(|(c, _)| *c == ReimportClass::EwChanged)
            .collect::<Vec<_>>(),
        [&(ReimportClass::EwChanged, "-K1".to_owned())]
    );
    let added: Vec<ew_api::import::ReimportRow> = r
        .plan
        .rows
        .iter()
        .filter(|row| row.class == ReimportClass::Added)
        .cloned()
        .collect();
    let kinds: std::collections::BTreeSet<&str> = added.iter().map(|row| row.key.0.name()).collect();
    assert!(kinds.contains("connection"), "{classes:?}");
    assert!(
        added.iter().any(|row| row.record.to_string() == "-X1"),
        "{classes:?}"
    );
    let before = model(&s);
    s.apply_reimport(r).expect("applied").expect("a transaction");
    assert_eq!(
        text_en(&device(&s, "-K1").function_text),
        Some("Main contactor"),
        "the edit is kept"
    );
    let x1 = device(&s, "-X1");
    assert!(added.iter().any(|row| row.key.1.as_uuid() == x1.id.as_uuid()));
    // Right after the re-import, the same file again finds every record unchanged but the edit (R3).
    let again = s
        .reimport_qet(FILE, variant_text().into_bytes(), &library(), detected.lineage())
        .expect("planned again");
    let changed: Vec<(ReimportClass, String)> = again
        .plan
        .rows
        .iter()
        .filter(|row| row.class != ReimportClass::Unchanged)
        .map(|row| (row.class, row.record.to_string()))
        .collect();
    assert_eq!(changed, [(ReimportClass::EwChanged, "-K1".to_owned())]);
    // Apply is one undo step.
    s.undo(&Origin::Gui).expect("the re-import is undoable");
    assert!(model(&s).content_eq(&before), "undo restores the project");
}

#[test]
fn a_change_on_both_sides_and_a_removal_are_kept_and_listed() {
    let mut s = imported(&base_text());
    edit_function_text(&mut s, "-K1", "Main contactor");
    // The source renames -K1 and no longer has -K2.
    let text = qet(r#"version="0.100""#, &[folio("1", "Base", vec![k1("-K3")])]);
    let detected = s
        .detect_qet_lineage(FILE, text.clone().into_bytes(), &library())
        .expect("the earlier import is proposed");
    let r = s
        .reimport_qet(FILE, text.into_bytes(), &library(), detected.lineage())
        .expect("the re-import is planned");
    let conflict = r
        .plan
        .rows
        .iter()
        .find(|row| row.class == ReimportClass::Conflict && row.key.0.name() == "device")
        .expect("-K1 changed on both sides");
    assert_eq!(conflict.record.to_string(), "-K1");
    let paths: Vec<&str> = conflict.members.iter().map(|m| m.path.as_str()).collect();
    assert!(paths.contains(&"function_text"), "{paths:?}");
    let removed: Vec<String> = r
        .plan
        .rows
        .iter()
        .filter(|row| row.class == ReimportClass::Removed)
        .map(|row| row.record.to_string())
        .collect();
    assert!(removed.contains(&"-K2".to_owned()), "{removed:?}");
    s.apply_reimport(r).expect("applied").expect("a transaction");
    assert_eq!(text_en(&device(&s, "-K1").function_text), Some("Main contactor"));
    device(&s, "-K2");
    let report = s.check(&ew_api::checks::MIGRATION, ew_api::checks::CheckMode::Release);
    let messages = |rule: &str| -> Vec<String> {
        report
            .findings
            .iter()
            .filter(|f| f.rule == rule)
            .map(|f| f.message.clone())
            .collect()
    };
    assert!(
        messages("EW-IMP-011")
            .contains(&"-K1 was changed both in r.qet and here; this project's version is kept.".to_owned()),
        "{:?}",
        messages("EW-IMP-011")
    );
    assert!(
        messages("EW-IMP-010").contains(
            &"-K2 was removed in r.qet; it is kept here. Delete it if it is no longer needed.".to_owned()
        ),
        "{:?}",
        messages("EW-IMP-010")
    );
}

/// ID-1392 (FT-1387): a conflict row's member values in words: a multi-language text as its text, a record ID by the
/// record's title, an absent value as `-`.
#[test]
fn member_values_read_in_words() {
    let s = imported(&base_text());
    let k1 = device(&s, "-K1").id.to_string();
    assert_eq!(
        s.member_words("device", "function_text", Some(&json!({"en": "Main contactor"}))),
        "Main contactor"
    );
    assert_eq!(
        s.member_words("connection", "from.device", Some(&json!(k1))),
        "-K1"
    );
    assert_eq!(s.member_words("device", "function_text", None), "-");
}

/// ID-1392 (FT-1384): after a re-import that keeps a record removed in the source, the same file again writes
/// nothing: no run, no undo step, no change.
#[test]
fn the_same_file_after_a_removal_writes_nothing() {
    let mut s = imported(&base_text());
    let text = qet(r#"version="0.100""#, &[folio("1", "Base", vec![k1("-K1")])]);
    let reimport = |s: &mut Session| {
        let detected = s
            .detect_qet_lineage(FILE, text.clone().into_bytes(), &library())
            .expect("the earlier import is proposed");
        s.reimport_qet(FILE, text.clone().into_bytes(), &library(), detected.lineage())
            .expect("the re-import is planned")
    };
    let r = reimport(&mut s);
    assert!(r.plan.count(ReimportClass::Removed) > 0);
    s.apply_reimport(r).expect("applied").expect("a transaction");
    let (before, before_rev) = (model(&s), s.world().rev);
    let r = reimport(&mut s);
    assert_eq!(r.plan.count(ReimportClass::Removed), 0);
    assert!(!r.plan.writes());
    assert!(s.apply_reimport(r).expect("nothing to apply").is_none());
    assert!(model(&s).content_eq(&before), "nothing was written");
    assert_eq!(s.world().rev, before_rev);
    device(&s, "-K2");
}

/// ID-1389 (W51 finding C): a device the source renames is listed by the tag it has here, not by the source's new
/// label.
#[test]
fn a_device_changed_in_the_source_is_named_by_its_tag_here() {
    let mut s = imported(&base_text());
    let text = qet(
        r#"version="0.100""#,
        &[folio("1", "Base", vec![k1("-K3"), k2()]).wire((&uuid(1), SIMPLE, 1), (&uuid(2), SIMPLE, 0))],
    );
    let detected = s
        .detect_qet_lineage(FILE, text.clone().into_bytes(), &library())
        .expect("the earlier import is proposed");
    let r = s
        .reimport_qet(FILE, text.into_bytes(), &library(), detected.lineage())
        .expect("the re-import is planned");
    let changed: Vec<String> = r
        .plan
        .rows
        .iter()
        .filter(|row| row.class == ReimportClass::SourceChanged && row.key.0.name() == "device")
        .map(|row| row.record.to_string())
        .collect();
    assert_eq!(changed, ["-K1"]);
}

#[test]
fn a_file_of_another_project_is_refused() {
    let s = imported(&base_text());
    let other = qet(
        r#"version="0.100""#,
        &[folio(
            "1",
            "Other",
            vec![el(SIMPLE, uuid(9), 100, 100).label("-Q9")],
        )],
    );
    let err = s
        .detect_qet_lineage("other.qet", other.into_bytes(), &library())
        .expect_err("no lineage matches");
    assert!(matches!(err, ReimportError::NotAnImport { .. }));
    assert_eq!(
        err.to_string(),
        "other.qet is not an earlier import of this project; import it into a new project instead."
    );
}
