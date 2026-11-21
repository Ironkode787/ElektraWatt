// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Checks of the builder DSL's verbs `assign_part`, `update_library` and `part_edit_copy` (`part.assign`,
//! `library.update` and `part.edit_copy`) against a session over the sample read in place (the committed directory
//! is never written). The acceptance tests with a library run in `ew-api` (`tests/library_ops.rs`).

use std::path::Path;

use ew_api::{AnyCommand, Cost, Session};
use ew_ids::{Id, Uuid};
use ew_schema::{Device, LibDocument, Part};
use ew_testkit::dsl::{Builder, CommandSink, Done};
use serde_json::{Value, json};

fn sample() -> Session {
    Session::open_read_only(Path::new(ew_testdir::SAMPLE), "test")
        .expect("the sample opens")
        .0
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
        .expect("a part of the sample");
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
    let mut s = sample();
    let mcb = part(&s, "EW-GEN-MCB-1");
    let psu = part(&s, "EW-GEN-PSU-1");
    let f1 = device_using(&s, copy_id(&mcb));
    let g1 = device_using(&s, copy_id(&psu));
    // A second part use of the PSU on the 1-pole breaker, then the breaker's part replaced by the PSU there: two
    // uses of one part.
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
        // A CP map that is not injective is refused at the repeated index (the PSU's CPs 3 and 4
        // are both `+`).
        let psu_copy = copy_id(&psu).as_uuid();
        let e = b
            .update_library(std::slice::from_ref(&next), &[(psu_copy, vec![(3, 3), (4, 3)])])
            .unwrap_err();
        assert!(e.message.contains("/cp_maps/0/map/1/1"), "{e}");
        let e = b
            .update_library(std::slice::from_ref(&next), &[(psu_copy, vec![(3, 3), (3, 4)])])
            .unwrap_err();
        assert!(e.message.contains("/cp_maps/0/map/1/0"), "{e}");
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

/// The DSL verb `part_edit_copy`: the sample's copies come from the starter library, so the 1-pole breaker's copy is
/// first forked (a project-local copy); the fork's edit is then in place (the same copy ID, the new description, the
/// hash of the new item), and a fork of it is refused at `/fork`.
#[test]
fn part_edit_copy_edits_a_project_local_copy_in_place() {
    let mut s = sample();
    let mcb = part(&s, "EW-GEN-MCB-1");
    let LibDocument::Part(c) = &mcb else {
        unreachable!()
    };
    let mut edited = serde_json::to_value(&c.item).unwrap();
    edited["description"] = json!("MCB 1P C4");
    let local = {
        let mut sink = Sink(&mut s);
        let mut b = Builder::new(&mut sink);
        b.part_edit_copy(c.id, edited.clone(), Some(Uuid::from_u128(7)), &[])
            .expect("the library copy forks")
    };
    assert_ne!(local, c.id, "the fork is a copy of its own");
    let mut edited = serde_json::to_value(&s.query().parts().get(local).unwrap().item).unwrap();
    edited["description"] = json!("MCB 1P C6");
    {
        let mut sink = Sink(&mut s);
        let mut b = Builder::new(&mut sink);
        let e = b
            .part_edit_copy(local, edited.clone(), Some(Uuid::from_u128(8)), &[])
            .unwrap_err();
        assert!(e.message.contains("/fork"), "{e}");
        assert_eq!(b.part_edit_copy(local, edited, None, &[]).unwrap(), local);
    }
    let q = s.query();
    let held = q.parts().get(local).unwrap();
    assert_eq!(held.item.description.as_neutral(), Some("MCB 1P C6"));
    assert_eq!(
        Some(held.sha256.0),
        ew_commands::commands::library_copy_hash(&LibDocument::Part((**held).clone()))
    );
}
