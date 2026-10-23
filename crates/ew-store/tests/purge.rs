// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The asset purge (doc 05 §13.3, FT-380): it removes only listed asset files whose hash no project file mentions
//! (kept unknown lines and foreign files included) and that the caller does not keep (the undo entries), through the
//! save protocol's removal list; it is refused in read-only sessions, with quarantined lines, with unsaved journal
//! steps or an unsaved model, with a crashed session's journal and after an external change; a kill at every call of
//! the purge's save leaves the old or the new generation, never a named asset without its bytes.

// The writer's tests read the model untracked, ADR-0001 amendment 2.
#![allow(clippy::disallowed_methods)]

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;

use common::{apply, edit, mem_project, options, par_map, project_files, reopen, root, try_reopen};
use ew_model::{AnyKey, AnyRow, Asset, Model, Op};
use ew_store::ew_format::ew_schema::{AssetRef, COMPANY_MEMBER, Sha256, sniff};
use ew_store::ew_format::{Manifest, Severity, sha256};
use ew_store::mem::{DirSync, Fault, MemVfs};
use ew_store::{Access, Changes, PurgeError, Store, join};

const MODES: [DirSync; 2] = [DirSync::Explicit, DirSync::Implicit];

/// A placeholder logo of `w × h` (no company's artwork): each size is another asset.
fn logo(w: u32) -> Asset {
    let bytes = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} 20"><rect width="{w}" height="20"/></svg>"#)
        .into_bytes();
    let (format, size) = sniff(&bytes).expect("an SVG");
    Asset::new(
        AssetRef {
            sha256: Sha256(sha256(&bytes)),
            format,
            size,
        },
        bytes.as_slice(),
    )
}

/// `model` with the asset held and named as the company logo; the keys the edit touched.
fn with_logo(model: &Model, asset: &Asset) -> (Model, BTreeSet<AnyKey>) {
    let before = Arc::clone(model.project());
    let mut after = (*before).clone();
    after.extra.insert(
        COMPANY_MEMBER.to_owned(),
        serde_json::json!({ "logo": serde_json::to_value(asset.reference()).unwrap() }),
    );
    let mut keys = BTreeSet::new();
    let mut next = apply(
        model,
        &[Op::update(
            AnyRow::Project(before),
            AnyRow::Project(Arc::new(after)),
        )],
        &mut keys,
    )
    .expect("the edit applies");
    next.put_asset(asset.clone());
    (next, keys)
}

fn hex(asset: &Asset) -> String {
    asset.sha256().to_string()["sha256:".len()..].to_owned()
}

fn manifest(vfs: &MemVfs) -> Manifest {
    let files = project_files(vfs, &root());
    Manifest::parse(&files["manifest.json"], &Default::default()).expect("a manifest")
}

/// The golden project with the logo saved as A, then C, then B (pid 1, closed): three listed asset files, B named.
/// Then a foreign file mentions C and a kept unknown-kind line in `structure.jsonl` mentions D, a fourth asset saved
/// before A.
fn four_assets(mode: DirSync) -> (Arc<MemVfs>, [Asset; 4]) {
    let (vfs, root) = mem_project(mode);
    let o = Store::open(&root, options(&vfs, 1)).expect("the golden project opens");
    let (mut store, mut model) = (o.store, o.model);
    let assets = [logo(10), logo(20), logo(30), logo(40)];
    let [a, b, c, d] = &assets;
    for asset in [d, a, c, b] {
        let (next, keys) = with_logo(&model, asset);
        store.save(&next, Changes::Keys(&keys)).expect("the save");
        model = next;
    }
    store.close();
    vfs.put(
        &join(&root, "notes.txt"),
        format!("the old logo was SHA256:{}\n", hex(c).to_uppercase()).as_bytes(),
    );
    let files = project_files(&vfs, &root);
    let mut structure = files["structure.jsonl"].clone();
    structure.extend_from_slice(
        format!(
            "{{\"$k\":\"x_future\",\"id\":\"0f0e0d0c-0b0a-5908-8706-050403020100\",\"asset\":\"sha256:{}\"}}\n",
            hex(d)
        )
        .as_bytes(),
    );
    vfs.put(&join(&root, "structure.jsonl"), &structure);
    (vfs, assets)
}

#[test]
fn a_purge_removes_only_the_asset_files_nothing_names() {
    let (vfs, [a, b, c, d]) = four_assets(DirSync::Explicit);
    let o = reopen(&vfs, 2);
    assert_eq!(o.report.access, Access::Edit);
    let mut store = o.store;
    for asset in [&a, &b, &c, &d] {
        assert!(project_files(&vfs, &root()).contains_key(&asset.reference().path()));
    }
    let plan = store.purge_plan(&BTreeSet::new()).expect("a plan");
    let removed: Vec<String> = plan.remove.iter().map(|f| f.path.as_str().to_owned()).collect();
    assert_eq!(
        removed,
        [a.reference().path()],
        "only A: B is named, C and D are mentioned"
    );
    assert_eq!(plan.kept, 3);
    assert_eq!(plan.remove[0].len, a.bytes().len() as u64);
    // A hash the undo entries hold is kept.
    let keep: BTreeSet<Sha256> = [*a.sha256()].into();
    assert!(store.purge_plan(&keep).expect("a plan").remove.is_empty());

    let (done, report) = store.purge(&o.model, &BTreeSet::new(), None).expect("the purge");
    assert_eq!(done, plan);
    assert_eq!(
        report
            .removed
            .iter()
            .map(|p| p.as_str().to_owned())
            .collect::<Vec<_>>(),
        removed
    );
    assert!(!project_files(&vfs, &root()).contains_key(&a.reference().path()));
    assert!(
        !manifest(&vfs)
            .files
            .keys()
            .any(|p| p.as_str() == a.reference().path())
    );
    // Nothing is left to purge, and a second purge writes nothing.
    assert!(store.purge_plan(&BTreeSet::new()).unwrap().remove.is_empty());
    let (_, again) = store.purge(&o.model, &BTreeSet::new(), None).unwrap();
    assert!(!again.manifest_written);
    store.close();
    let o = reopen(&vfs, 3);
    assert!(
        !o.report.findings.iter().any(|f| f.severity() > Severity::Info),
        "{:?}",
        o.report.findings
    );
    assert_eq!(o.model.asset(b.sha256()), Some(&b));
    assert!(o.model.asset(a.sha256()).is_none());
}

#[test]
fn a_purge_of_listed_hashes_removes_exactly_those_or_nothing() {
    let (vfs, [a, b, _, _]) = four_assets(DirSync::Explicit);
    let o = reopen(&vfs, 2);
    let mut store = o.store;
    // B is named: listing it makes the whole purge stale, and A stays.
    let listed: BTreeSet<Sha256> = [*a.sha256(), *b.sha256()].into();
    match store.purge(&o.model, &BTreeSet::new(), Some(&listed)) {
        Err(PurgeError::Stale(stale)) => assert_eq!(stale, [*b.sha256()]),
        other => panic!("expected Stale, got {other:?}"),
    }
    assert!(project_files(&vfs, &root()).contains_key(&a.reference().path()));
    let listed: BTreeSet<Sha256> = [*a.sha256()].into();
    let (plan, _) = store
        .purge(&o.model, &BTreeSet::new(), Some(&listed))
        .expect("the purge");
    assert_eq!(plan.hashes(), listed);
    assert!(!project_files(&vfs, &root()).contains_key(&a.reference().path()));
}

#[test]
fn a_purge_is_refused_unless_the_session_is_saved_and_clean() {
    let (vfs, _) = four_assets(DirSync::Explicit);
    // Read-only: another session holds the lock.
    let held = reopen(&vfs, 2);
    let mut other = Store::open(&root(), options(&vfs, 3)).expect("opens read-only");
    assert!(matches!(other.report.access, Access::ReadOnly(_)));
    // The plan is a preview: it works read-only; the purge does not.
    assert_eq!(other.store.purge_plan(&BTreeSet::new()).unwrap().remove.len(), 1);
    assert!(matches!(
        other.store.purge(&other.model, &BTreeSet::new(), None),
        Err(PurgeError::ReadOnly(_))
    ));
    drop(other);
    let mut store = held.store;
    // An unsaved model, and then its journalled step.
    let (edited, keys) = edit(&held.model);
    assert!(matches!(
        store.purge(&edited, &BTreeSet::new(), None),
        Err(PurgeError::Unsaved)
    ));
    let keys: Vec<AnyKey> = keys.into_iter().collect();
    store
        .journal_commit(&held.model, &edited, &keys, "fields.set", "test")
        .expect("journalled");
    assert!(matches!(
        store.purge_plan(&BTreeSet::new()),
        Err(PurgeError::Unsaved)
    ));
    // A crashed session's journal blocks the next session.
    store.crash();
    vfs.restart();
    let o = try_reopen(&vfs, 4).expect("reopens");
    assert!(o.store.recovery().is_some());
    assert!(matches!(
        o.store.purge_plan(&BTreeSet::new()),
        Err(PurgeError::Recovery)
    ));
    let mut store = o.store;
    store.discard_recovery();
    assert!(store.purge_plan(&BTreeSet::new()).is_ok());
    // An external change of the manifest.
    let files = project_files(&vfs, &root());
    let mut bytes = files["manifest.json"].clone();
    bytes.push(b'\n');
    vfs.put(&join(&root(), "manifest.json"), &bytes);
    assert!(matches!(
        store.purge_plan(&BTreeSet::new()),
        Err(PurgeError::ExternalChange)
    ));
}

#[test]
fn a_purge_is_refused_while_lines_are_quarantined() {
    let (vfs, _) = four_assets(DirSync::Explicit);
    let files = project_files(&vfs, &root());
    let mut structure = files["structure.jsonl"].clone();
    structure.extend_from_slice(b"{\"$k\":\"structure_node\",\"id\":\"not a uuid\"}\n");
    vfs.put(&join(&root(), "structure.jsonl"), &structure);
    let o = reopen(&vfs, 2);
    assert_eq!(o.report.access, Access::Edit, "{:?}", o.report.findings);
    match o.store.purge_plan(&BTreeSet::new()) {
        Err(PurgeError::Quarantined(files)) => {
            assert_eq!(
                files.iter().map(|f| f.as_str()).collect::<Vec<_>>(),
                ["structure.jsonl"]
            );
        }
        other => panic!("expected Quarantined, got {other:?}"),
    }
}

/// A kill at every call of the purge's save, in both directory-sync modes: the project reopens at the old generation
/// with A listed and on disk, or at the new one without A (its file deleted by the roll-forward or the journal's
/// removals); B, the named logo, is always loaded with its bytes; no finding above info.
#[test]
fn a_kill_at_every_call_of_a_purge_leaves_the_old_or_the_new_generation() {
    for mode in MODES {
        let setup = || {
            let (vfs, assets) = four_assets(mode);
            let o = reopen(&vfs, 2);
            (vfs, o.store, o.model, assets)
        };
        let (vfs, mut store, model, _) = setup();
        let g = store.generation();
        vfs.set_trace(true);
        vfs.reset_calls();
        store
            .purge(&model, &BTreeSet::new(), None)
            .expect("the dry-run purge");
        let n = vfs.calls();
        assert!(n > 0);
        let failures: Vec<String> = par_map((1..=n).collect(), |k| {
            let (vfs, mut store, model, [a, b, _, _]) = setup();
            vfs.reset_calls();
            vfs.set_fault(Fault::Kill { at: k });
            let _ = store.purge(&model, &BTreeSet::new(), None);
            store.crash();
            vfs.restart();
            let o = match try_reopen(&vfs, 5) {
                Ok(o) => o,
                Err(e) => return vec![format!("{mode:?} k={k}: {e}")],
            };
            let mut out = Vec::new();
            let files = project_files(&vfs, &root());
            let listed = manifest(&vfs)
                .files
                .keys()
                .any(|p| p.as_str() == a.reference().path());
            let on_disk = files.contains_key(&a.reference().path());
            match o.report.generation {
                x if x == g => {
                    if !(listed && on_disk) {
                        out.push(format!(
                            "{mode:?} k={k}: at g, A listed {listed}, on disk {on_disk}"
                        ));
                    }
                }
                x if x == g + 1 => {
                    if listed || on_disk {
                        out.push(format!(
                            "{mode:?} k={k}: at g+1, A listed {listed}, on disk {on_disk}"
                        ));
                    }
                }
                x => out.push(format!("{mode:?} k={k}: generation {x}")),
            }
            if o.model.asset(b.sha256()) != Some(&b) {
                out.push(format!("{mode:?} k={k}: the named logo lost its bytes"));
            }
            if let Some(f) = o.report.findings.iter().find(|f| f.severity() > Severity::Info) {
                out.push(format!("{mode:?} k={k}: {f:?}"));
            }
            out
        })
        .into_iter()
        .flatten()
        .collect();
        assert!(
            failures.is_empty(),
            "{} failures:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

/// `text` with every character of `hex` spelled as a JSON escape `\u00XX`.
fn escaped(hex: &str) -> String {
    hex.chars().map(|c| format!("\\u{:04x}", c as u32)).collect()
}

/// Review fix 2: a record line may spell a hash with JSON escapes (the loader accepts it and rewrites it at the
/// next save); the text scan of the file misses it, so the purge also reads the model: the named logo (B) and an
/// unknown member's escaped hash (A, in a structure node) both keep their files.
#[test]
fn a_hash_spelled_with_json_escapes_keeps_its_asset() {
    let (vfs, [a, b, _, _]) = four_assets(DirSync::Explicit);
    let files = project_files(&vfs, &root());
    let project = String::from_utf8(files["project.jsonl"].clone()).unwrap();
    let plain = format!("sha256:{}", hex(&b));
    assert!(project.contains(&plain));
    let project = project.replace(&plain, &format!("sha256:{}", escaped(&hex(&b))));
    vfs.put(&join(&root(), "project.jsonl"), project.as_bytes());
    let structure = String::from_utf8(files["structure.jsonl"].clone()).unwrap();
    let first = structure.lines().next().unwrap();
    let tagged = first.replacen(
        '{',
        &format!("{{\"x.future\":\"sha256:{}\",", escaped(&hex(&a))),
        1,
    );
    let structure = structure.replacen(first, &tagged, 1);
    vfs.put(&join(&root(), "structure.jsonl"), structure.as_bytes());
    let o = reopen(&vfs, 2);
    assert_eq!(o.report.access, Access::Edit, "{:?}", o.report.findings);
    assert_eq!(
        o.model.referenced_assets().keys().copied().collect::<Vec<_>>(),
        [*b.sha256()],
        "the loader decodes the escaped logo"
    );
    let plan = o.store.purge_plan(&BTreeSet::new()).unwrap();
    assert!(plan.remove.is_empty(), "nothing goes: {plan:?}");
}

/// Review fix 3: an asset file rewritten after the open (the manifest unchanged) no longer holds the asset of its
/// name; the purge re-hashes it and leaves it alone.
#[test]
fn an_asset_file_rewritten_behind_the_manifest_is_left_alone() {
    let (vfs, [a, _, _, _]) = four_assets(DirSync::Explicit);
    let o = reopen(&vfs, 2);
    let path = a.reference().path();
    assert_eq!(
        o.store.purge_plan(&BTreeSet::new()).unwrap().remove[0]
            .path
            .as_str(),
        path
    );
    vfs.put(
        &join(&root(), &path),
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
    );
    let plan = o.store.purge_plan(&BTreeSet::new()).unwrap();
    assert!(plan.remove.is_empty(), "{plan:?}");
    assert_eq!(plan.kept, 4);
}

/// Review fix 9: files are scanned in chunks; a mention that straddles two chunks of a large foreign file still keeps
/// its asset, and a mention further on in it too.
#[test]
fn a_mention_across_a_chunk_boundary_of_a_large_file_keeps_its_asset() {
    let (vfs, [a, _, c, _]) = four_assets(DirSync::Explicit);
    let chunk = 1 << 20;
    let mut big = vec![b' '; chunk - 30];
    big.extend_from_slice(hex(&c).as_bytes());
    big.resize(3 * chunk + 17, b'.');
    big.extend_from_slice(hex(&a).as_bytes());
    big.push(b'\n');
    vfs.put(&join(&root(), "notes.txt"), &big);
    let o = reopen(&vfs, 2);
    let plan = o.store.purge_plan(&BTreeSet::new()).unwrap();
    assert!(plan.remove.is_empty(), "{plan:?}");
    // Without the mentions, both go.
    vfs.put(&join(&root(), "notes.txt"), &vec![b'x'; 3 * chunk]);
    let plan = o.store.purge_plan(&BTreeSet::new()).unwrap();
    let mut gone: Vec<String> = plan.remove.iter().map(|f| f.path.as_str().to_owned()).collect();
    gone.sort();
    let mut want = vec![a.reference().path(), c.reference().path()];
    want.sort();
    assert_eq!(gone, want);
}
