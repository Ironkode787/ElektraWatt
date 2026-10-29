// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Golden A checks of the builder DSL's verbs `assign_part` and `update_library` (WP-S1.19 part 3, `part.assign` and
//! `library.update`) against a session over path (a)'s batch (the committed directory is never opened). The
//! acceptance tests with a library run in `ew-api` (`tests/library_ops.rs`).

use std::path::Path;

use ew_api::{AnyCommand, Cost, Session, SessionOptions};
use ew_ids::{Id, Uuid};
use ew_schema::{Device, LibDocument, Part, Record};
use ew_testkit::golden::dsl::{Builder, CommandSink, Done};
use serde_json::{Value, json};

fn golden_a() -> Session {
    let batch = ew_testkit::golden::bootstrap(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("path (a) imports");
    let mut project = None;
    let mut records = Vec::new();
    for r in batch.records {
        match r {
            Record::Project(p) => project = Some(p),
            other => records.push(other),
        }
    }
    Session::from_records(
        project.expect("the project row"),
        records,
        batch.lib,
        SessionOptions::default(),
    )
    .expect("Golden A is a valid model")
}

/// The DSL's sink over a session: an interactive command runs directly, a bulk one through a preview.
struct Sink<'s>(&'s mut Session);

impl CommandSink for Sink<'_> {
    fn run(&mut self, name: &str, params: Value) -> Result<Done, String> {
        let cmd = AnyCommand {
            name: name.to_owned(),
            params,
        };
        let bulk = self.0.registry().get(name).map(|e| e.meta().cost) == Some(Cost::Bulk);
        let receipt = if bulk {
            let preview = self.0.preview(&cmd).map_err(|e| format!("{e:?}"))?;
            self.0.commit(preview)
        } else {
            self.0.execute(&cmd)
        };
        receipt
            .map(|r| Done {
                primary: r.outcome.primary,
                created: r.created,
            })
            .map_err(|e| format!("{e:?}"))
    }
}

fn part(s: &Session, number: &str) -> LibDocument {
    let q = s.query();
    let c = q
        .parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == number)
        .map(|(_, c)| (**c).clone())
        .expect("a golden part");
    LibDocument::Part(c)
}

fn copy_id(doc: &LibDocument) -> Id<Part> {
    match doc {
        LibDocument::Part(c) => c.id,
        _ => unreachable!(),
    }
}

fn device_using(s: &Session, part: Id<Part>) -> Id<Device> {
    s.query()
        .devices()
        .iter()
        .find(|(_, d)| d.parts.iter().any(|u| u.part.copy == part))
        .map(|(id, _)| id)
        .expect("a device uses the part")
}

#[test]
fn the_assign_and_update_verbs_change_part_uses_and_copies() {
    let mut s = golden_a();
    let mcb = part(&s, "A9P52610");
    let psu = part(&s, "2904602");
    let f1 = device_using(&s, copy_id(&mcb));
    let g1 = device_using(&s, copy_id(&psu));
    // A second part use of the PSU on -101F1, then the MCB replaced by the PSU there: two uses of one part.
    {
        let mut sink = Sink(&mut s);
        let mut b = Builder::new(&mut sink);
        b.assign_part(&[f1], copy_id(&psu), &[], "append", None, Some("2"))
            .unwrap();
        b.assign_part(&[f1], copy_id(&psu), &[], "replace", Some(copy_id(&mcb)), None)
            .unwrap();
    }
    let uses: Vec<(Id<Part>, String)> = s
        .query()
        .devices()
        .get(f1)
        .unwrap()
        .parts
        .iter()
        .map(|u| (u.part.copy, u.qty.to_string()))
        .collect();
    assert_eq!(
        uses,
        [(copy_id(&psu), "1".to_owned()), (copy_id(&psu), "2".to_owned())]
    );
    // The PSU's description changes in place: same copy ID, a new version.
    let mut next = serde_json::to_value(&psu).unwrap();
    next["version"] = json!("1.0.1");
    next["description"] = json!("Power supply, updated");
    let doc = LibDocument::from_value(next).unwrap();
    let hash = ew_commands::commands::library_copy_hash(&doc).unwrap();
    let mut next = serde_json::to_value(&doc).unwrap();
    next["sha256"] = json!(ew_schema::Sha256(hash).to_string());
    let next = LibDocument::from_value(next).unwrap();
    {
        let mut sink = Sink(&mut s);
        let mut b = Builder::new(&mut sink);
        // A mapping for a copy that is not updated is refused.
        let e = b
            .update_library(std::slice::from_ref(&next), &[(Uuid::nil(), Vec::new())])
            .unwrap_err();
        assert!(e.message.contains("/cp_maps/0/copy"), "{e}");
        b.update_library(&[next], &[]).unwrap();
    }
    let q = s.query();
    let held = q.parts().get(copy_id(&psu)).unwrap();
    assert_eq!(
        (held.version.as_str(), held.item.description.as_neutral()),
        ("1.0.1", Some("Power supply, updated"))
    );
    assert!(
        q.devices()
            .get(g1)
            .unwrap()
            .parts
            .iter()
            .any(|u| u.part.copy == copy_id(&psu))
    );
}
