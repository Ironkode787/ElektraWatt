// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What an open and a save do with files and lines the writer does not own (review cycle 8): findings
//! beyond the cap still force a read-only session, quarantined lines are written back byte for byte, an
//! unlisted file at a path the save writes is moved aside instead of overwritten, and only the save
//! protocol's own temps are deleted.

// The writer's tests read the model untracked, ADR-0001 amendment 2.
#![allow(clippy::disallowed_methods)]

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;

use common::{apply, device_copy, fixture, mem_project, options, project_files, reopen};
use ew_model::{AnyRow, Op};
use ew_store::ew_format::ew_schema::Device;
use ew_store::ew_format::{FilePath, FindingKind, Limits};
use ew_store::mem::DirSync;
use ew_store::{Access, Changes, Opened, ReadOnlyReason, Store, join};

#[test]
fn a_finding_beyond_the_cap_still_forces_read_only_and_quarantined_bytes_survive() {
    // Review cycle 8, major 2: 1,001 duplicate lines fill the capped findings list, so the EW-CORE-005 of
    // the line after them (not UTF-8) is dropped from it; the session must still be read-only, and the
    // writer must put the line's bytes back, not U+FFFD.
    let (vfs, root) = mem_project(DirSync::Explicit);
    let files = fixture();
    let shard = files
        .keys()
        .find(|n| n.starts_with("devices/"))
        .expect("the fixture has shards")
        .clone();
    let mut bytes = files[&shard].clone();
    let first = bytes
        .split(|b| *b == b'\n')
        .next()
        .expect("a first line")
        .to_vec();
    for _ in 0..1_001 {
        bytes.extend_from_slice(&first);
        bytes.push(b'\n');
    }
    let broken: &[u8] = b"\xff\xfe{\"not\":\"utf-8\"}";
    bytes.extend_from_slice(broken);
    bytes.push(b'\n');
    vfs.put(&join(&root, &shard), &bytes);

    let Opened { store, model, report } = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert!(Limits::default().max_findings < 1_001);
    assert!(
        report.findings.iter().any(|f| f.kind == FindingKind::Truncated),
        "the loader's list is capped"
    );
    assert!(
        !report.findings.iter().any(|f| f.kind == FindingKind::JsonSyntax),
        "the syntax finding is one of those dropped"
    );
    assert_eq!(
        report.access,
        Access::ReadOnly(ReadOnlyReason::Integrity),
        "a dropped finding that forces read-only still does"
    );

    let rendered = store.render_all(&model).expect("renders");
    let written = &rendered[&FilePath::new(shard.as_str()).expect("a valid path")];
    let mut line = broken.to_vec();
    line.push(b'\n');
    assert!(
        written.windows(line.len()).any(|w| w == line.as_slice()),
        "the quarantined line is written back byte for byte"
    );
    assert!(
        !written.windows(3).any(|w| w == "\u{fffd}".as_bytes()),
        "no replacement character"
    );
}

#[test]
fn a_save_moves_an_unlisted_file_aside_instead_of_overwriting_it() {
    // Review cycle 8, major 3 (owner ruling): a colleague's `devices/<xx>.jsonl` that the manifest does not
    // list is EW-CORE-004 at open; a save that writes our own device into that shard moves the colleague's
    // file to `devices/<xx>.unlisted-2.jsonl` first and reports it, and the colleague's line survives.
    let (vfs, root) = mem_project(DirSync::Explicit);
    let files = fixture();
    let free = (0u64..=255)
        .find(|b| !files.contains_key(&format!("devices/{b:02x}.jsonl")))
        .expect("a shard the fixture does not use");
    let shard = format!("devices/{free:02x}.jsonl");
    let colleague = format!(
        "{{\"$k\":\"device\",\"id\":\"00000000-0000-7000-8000-0000000001{free:02x}\",\"kind\":\"a colleague's\"}}\n"
    );
    vfs.put(&join(&root, &shard), colleague.as_bytes());

    let Opened {
        mut store,
        model,
        report,
    } = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert_eq!(report.access, Access::Edit, "{:?}", report.findings);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.kind == FindingKind::Orphan && f.file.as_ref().is_some_and(|p| p.as_str() == shard)),
        "{:?}",
        report.findings
    );

    let template = model
        .table::<Device>()
        .iter()
        .map(|(_, d)| Arc::clone(d))
        .next()
        .expect("the fixture has devices");
    let ours = device_copy(&template, free);
    let mut keys = BTreeSet::new();
    let edited = apply(&model, &[Op::insert(AnyRow::of(ours))], &mut keys).expect("the device inserts");
    let saved = store
        .save(&edited, Changes::Keys(&keys))
        .expect("the save succeeds");
    assert_eq!(saved.generation, 2);

    let moved = FilePath::new(shard.as_str())
        .expect("a valid path")
        .unlisted(2, 0);
    assert_eq!(moved.as_str(), format!("devices/{free:02x}.unlisted-2.jsonl"));
    assert!(
        saved.findings.iter().any(|f| f.kind == FindingKind::Orphan
            && f.code() == Some("EW-CORE-004")
            && f.file.as_ref() == Some(&moved)
            && f.message.contains(&shard)),
        "{:?}",
        saved.findings
    );
    let on_disk = project_files(&vfs, &root);
    assert_eq!(
        on_disk.get(moved.as_str()).map(Vec::as_slice),
        Some(colleague.as_bytes()),
        "the colleague's file survives, byte for byte, under the unlisted name"
    );
    let ours_text = String::from_utf8(on_disk[&shard].clone()).expect("text");
    assert!(
        ours_text.contains(&format!("00000000-0000-7000-8000-{free:012x}")),
        "{ours_text}"
    );
    assert!(!ours_text.contains("a colleague's"), "{ours_text}");

    store.close();
    let again = reopen(&vfs, 2);
    assert_eq!(again.report.access, Access::Edit);
    assert!(again.model.content_eq(&edited));
    assert!(
        again
            .report
            .findings
            .iter()
            .any(|f| f.kind == FindingKind::Orphan && f.file.as_ref() == Some(&moved)),
        "every open reports the moved file: {:?}",
        again.report.findings
    );
}

#[test]
fn an_editing_open_deletes_only_the_save_protocols_temps() {
    // Review cycle 8, 5(f): `*.tmp-<n>` names that are not a temp of a writer-owned file or an intent belong
    // to someone else and stay.
    let (vfs, root) = mem_project(DirSync::Explicit);
    let shard = fixture()
        .keys()
        .find(|n| n.starts_with("devices/"))
        .expect("a shard")
        .clone();
    let ours = [format!("{shard}.tmp-9"), "manifest.json.tmp-1".to_owned()];
    let theirs = [
        "notes.txt.tmp-3",
        "rules/check.rhai.tmp-2",
        "assets/logo.svg.tmp-4",
    ];
    for name in ours.iter().map(String::as_str).chain(theirs) {
        vfs.put(&join(&root, name), b"x");
    }
    let opened = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert_eq!(opened.report.access, Access::Edit);
    let removed: BTreeSet<&str> = opened.report.removed.iter().map(String::as_str).collect();
    assert_eq!(removed, ours.iter().map(String::as_str).collect::<BTreeSet<_>>());
    let left = project_files(&vfs, &root);
    for name in theirs {
        assert!(left.contains_key(name), "{name} was deleted");
    }
}
