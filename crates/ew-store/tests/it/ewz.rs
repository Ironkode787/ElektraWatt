// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.17 part 2, `.ewz` transport (D8, D9; decision log ID-1175): EZ-1 (a deterministic pack of one verified
//! generation that extracts to its source), EZ-3 (the extract, all or nothing) and EZ-4 (the reader's refusals); and the
//! repair's pack of the files as found (decision log ID-1186).

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use ew_store::ew_format::{hash_text, sha256};
use ew_store::ewz::{self, AsFound, Ewz, EwzError, ExtractError, PackError};
use ew_store::mem::{DirSync, Fault, MemVfs};
use ew_store::zip::{self, Method, ZipLimits};
use ew_store::{OpenMode, OsVfs, Store, StoreOptions, Vfs};

use crate::common::{fixture, small_fixture};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

/// The git files EZ-1 adds to its copy of Golden A (Golden A has none, ID-1175 ruling 8).
fn git_files() -> BTreeMap<String, Vec<u8>> {
    BTreeMap::from([
        (
            ".gitattributes".to_owned(),
            b"*.jsonl merge=elektrawatt\n".to_vec(),
        ),
        (".gitignore".to_owned(), b".~ew.lock\n.ew/\n".to_vec()),
    ])
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), &target).unwrap();
        }
    }
}

/// Every file below `dir`, by `/`-separated relative path.
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let path = e.unwrap().path();
            if path.is_dir() {
                walk(&path, base, out);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// A read-only store of the project at `dir` on the real file system (no lock, no journal).
fn open_read_only(dir: &Path) -> Store {
    let mut o = StoreOptions::new("ElektraWatt test");
    o.mode = OpenMode::ReadOnly;
    o.state_dir = None;
    Store::open(dir, o).expect("the project opens").store
}

/// The `.ewz` of a project given as its files (`manifest.json` among them) and the git files.
fn pack_files(files: &BTreeMap<String, Vec<u8>>, git: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let mut listed = files.clone();
    let manifest = listed.remove("manifest.json").expect("a manifest");
    ewz::pack(&manifest, &listed, git).expect("the project packs")
}

/// EZ-1 (FR-COL-009's acceptance): a copy of Golden A with git files packs twice, from two opens, to the same bytes;
/// `mimetype` is the first entry, stored, `manifest.json` the second, the rest in byte order; a stray, a foreign
/// file, `.ew/`, a lock, a temp and `.git/` change nothing; extracting gives a directory byte-equal to the source.
#[test]
fn ez1_golden_a_packs_deterministically_and_extracts_to_its_source() {
    let base = ew_testdir::fresh_dir("ew-ewz-ez1");
    let source = base.join("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &source);
    for (name, data) in git_files() {
        std::fs::write(source.join(name), data).unwrap();
    }
    let expected = tree(&source);
    let first = ewz::pack_store(&open_read_only(&source)).expect("Golden A packs");
    let second = ewz::pack_store(&open_read_only(&source)).expect("Golden A packs again");
    assert_eq!(
        first, second,
        "two packs of the same generation are byte-identical"
    );
    assert_eq!(first.generation, 1);
    assert_eq!(first.files, expected.len());

    // Nothing but the generation and the git files enters.
    std::fs::write(source.join("README.txt"), b"a foreign file").unwrap();
    std::fs::write(source.join("pages").join("copy of a page.jsonl"), b"{}\n").unwrap();
    std::fs::create_dir_all(source.join(".ew")).unwrap();
    std::fs::write(source.join(".ew").join("cache"), b"x").unwrap();
    std::fs::create_dir_all(source.join(".git")).unwrap();
    std::fs::write(source.join(".git").join("config"), b"[core]\n").unwrap();
    std::fs::write(source.join("project.jsonl.tmp-9"), b"a temp").unwrap();
    std::fs::write(source.join(".~ew.lock.stale-1"), b"a stale lock").unwrap();
    let third = ewz::pack_store(&open_read_only(&source)).expect("Golden A packs with clutter");
    assert_eq!(
        third, first,
        "strays, foreign files, .ew/, temps, locks and .git/ never enter"
    );

    let entries = zip::read(&first.bytes, &ZipLimits::default()).unwrap();
    assert_eq!(
        (
            entries[0].name.as_str(),
            entries[0].method,
            entries[0].data.as_slice()
        ),
        (
            "mimetype",
            Method::Stored,
            b"application/vnd.elektrawatt.project+zip".as_slice()
        )
    );
    assert_eq!(
        &first.bytes[30..38],
        b"mimetype",
        "the first local header names mimetype"
    );
    assert_eq!(entries[1].name, "manifest.json");
    let rest: Vec<&str> = entries[2..].iter().map(|e| e.name.as_str()).collect();
    let mut sorted = rest.clone();
    sorted.sort_unstable();
    assert_eq!(rest, sorted, "the rest in byte order");
    assert!(entries[1..].iter().all(|e| e.method == Method::Deflated));

    let target = base.join("out").join("J-E-2416.ew");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    let archive = base.join("J-E-2416.ewz");
    std::fs::write(&archive, &first.bytes).unwrap();
    let read = ewz::read_file(&OsVfs, &archive).expect("the archive reads");
    ewz::extract(&OsVfs, &read, &target).expect("the archive extracts");
    assert_eq!(
        tree(&target),
        expected,
        "the extracted directory is byte-equal to the source"
    );
    assert_eq!(
        std::fs::read_dir(target.parent().unwrap()).unwrap().count(),
        1,
        "no staging directory is left"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// The repair's pack as found (WP-S1.17, decision log ID-1186): on a copy of Golden A with git files, whose files match
/// its manifest, it is `pack_store`'s archive byte for byte and counts no edited file; after a file is edited outside
/// ElektraWatt, `pack_store` still refuses the generation, and the pack as found holds the file as it is, the manifest
/// hashed to it, the other files and the git files as they are, and counts it.
#[test]
fn the_pack_as_found_is_the_verified_pack_until_a_file_is_edited() {
    let base = ew_testdir::fresh_dir("ew-ewz-as-found");
    let source = base.join("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &source);
    for (name, data) in git_files() {
        std::fs::write(source.join(name), data).unwrap();
    }
    let store = open_read_only(&source);
    let verified = ewz::pack_store(&store).expect("Golden A packs");
    assert_eq!(
        ewz::pack_as_found(&store).expect("Golden A packs as found"),
        AsFound {
            packed: verified,
            edited: 0
        }
    );

    let rel = "devices/02.jsonl";
    let before = std::fs::read(source.join(rel)).unwrap();
    let mut edited = before.clone();
    edited.push(b'\n');
    std::fs::write(source.join(rel), &edited).unwrap();
    let store = open_read_only(&source);
    assert_eq!(
        ewz::pack_store(&store).unwrap_err().to_string(),
        "devices/02.jsonl changed on disk since it was saved (an external change)"
    );
    let found = ewz::pack_as_found(&store).expect("Golden A packs as found after the edit");
    let mut expected = tree(&source);
    let manifest = String::from_utf8(expected["manifest.json"].clone())
        .unwrap()
        .replace(&hash_text(&sha256(&before)), &hash_text(&sha256(&edited)));
    expected.insert("manifest.json".to_owned(), manifest.into_bytes());
    assert_eq!(
        (found.packed.generation, found.packed.files, found.edited),
        (1, expected.len(), 1)
    );
    let read = ewz::read(&found.packed.bytes).expect("the pack as found verifies");
    assert_eq!(
        read.entries, expected,
        "the files as found, the manifest hashed to them"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// The ruling-3 bound inside `pack`: a project whose archive the reader would refuse is not packed, in words.
#[test]
fn a_project_the_reader_would_refuse_is_not_packed() {
    let mut listed = small_fixture();
    let manifest = listed.remove("manifest.json").unwrap();
    let git = git_files();
    // A git file of another name.
    let other = BTreeMap::from([("notes.txt".to_owned(), b"x".to_vec())]);
    let e = ewz::pack(&manifest, &listed, &other).unwrap_err();
    assert_eq!(
        e.to_string(),
        "this project cannot be packed into an .ewz (`notes.txt` is not a git file); share the project folder instead"
    );
    // A file the manifest does not list, passed as a project file, is what the reader refuses after the write.
    let mut more = listed.clone();
    more.insert("rules/extra.txt".to_owned(), b"x".to_vec());
    let e = ewz::pack(&manifest, &more, &git).unwrap_err();
    assert!(matches!(e, PackError::Refused(_)));
    assert_eq!(
        e.to_string(),
        "this project cannot be packed into an .ewz (the entry `rules/extra.txt` is not a file the manifest lists); \
         share the project folder instead"
    );
}

/// A verified `.ewz` of the small fixture.
fn small_ewz() -> Ewz {
    ewz::read(&pack_files(&small_fixture(), &git_files())).expect("the small archive reads")
}

/// A file system holding the directory `/w`, durable.
fn workdir() -> MemVfs {
    let vfs = MemVfs::new(DirSync::Explicit);
    vfs.put_dir(Path::new("/w"));
    vfs
}

/// The names directly in `/w`.
fn names(vfs: &MemVfs) -> Vec<String> {
    vfs.list(Path::new("/w"))
        .unwrap()
        .into_iter()
        .map(|(n, _)| n)
        .collect()
}

/// EZ-3: extracting onto an existing directory or file is refused before anything is written; an existing staging
/// directory is refused and not touched.
#[test]
fn ez3_an_existing_target_is_refused_and_nothing_is_written() {
    let archive = small_ewz();
    let target = Path::new("/w/p.ew");
    let vfs = workdir();
    vfs.put_dir(target);
    let e = ewz::extract(&vfs, &archive, target).unwrap_err();
    assert!(matches!(e, ExtractError::Exists(_)));
    assert_eq!(
        e.to_string(),
        format!(
            "{} already exists: an .ewz is extracted into a new folder only; nothing was written",
            target.display()
        )
    );
    assert_eq!(names(&vfs), ["p.ew"]);
    assert!(vfs.files(target).is_empty());

    let vfs = workdir();
    vfs.put(target, b"a file");
    assert!(matches!(
        ewz::extract(&vfs, &archive, target),
        Err(ExtractError::Exists(_))
    ));
    assert_eq!(
        vfs.files(Path::new("/w")),
        BTreeMap::from([("p.ew".to_owned(), b"a file".to_vec())])
    );

    let vfs = workdir();
    vfs.put(Path::new("/w/p.ew.tmp-ewz/mine"), b"another extract's");
    let e = ewz::extract(&vfs, &archive, target).unwrap_err();
    assert_eq!(
        e.to_string(),
        format!(
            "{} already exists (another extract, or an interrupted one): remove it and try again; nothing was written",
            Path::new("/w/p.ew.tmp-ewz").display()
        )
    );
    assert_eq!(
        vfs.files(Path::new("/w")),
        BTreeMap::from([("p.ew.tmp-ewz/mine".to_owned(), b"another extract's".to_vec())])
    );
}

/// EZ-3: a failure injected through the `Vfs` at each call of an extract leaves no target and removes the staging
/// directory; without a fault the extract writes exactly the archive's files, durably (a power loss right after it
/// keeps them).
#[test]
fn ez3_a_failure_at_each_call_leaves_no_target_and_no_staging() {
    let archive = small_ewz();
    let target = Path::new("/w/p.ew");
    let vfs = workdir();
    ewz::extract(&vfs, &archive, target).expect("the extract succeeds");
    let total = vfs.calls();
    assert_eq!(vfs.files(target), archive.entries);
    assert_eq!(names(&vfs), ["p.ew"]);
    vfs.set_fault(Fault::PowerLoss {
        at: total + 1,
        keep: 0,
    });
    assert!(vfs.stat(target).is_err());
    vfs.restart();
    assert_eq!(
        vfs.files(target),
        archive.entries,
        "durable when the extract returns"
    );

    let mut kinds = std::collections::BTreeSet::new();
    for at in 1..=total {
        let vfs = workdir();
        vfs.set_trace(true);
        vfs.set_fault(Fault::Error {
            at,
            kind: io::ErrorKind::Other,
        });
        let e = ewz::extract(&vfs, &archive, target).expect_err("an injected failure fails the extract");
        let call = vfs.trace()[usize::try_from(at - 1).unwrap()].clone();
        kinds.insert(call.split(' ').nth(1).unwrap().to_owned());
        assert!(
            names(&vfs).is_empty(),
            "call {call}: {:?} left behind ({e})",
            names(&vfs)
        );
    }
    assert_eq!(
        kinds.into_iter().collect::<Vec<_>>(),
        [
            "create_dir",
            "create_new",
            "read",
            "rename",
            "stat",
            "sync",
            "sync_dir"
        ],
        "every kind of call of the extract was failed once"
    );

    // One failure's text, whole.
    let vfs = workdir();
    vfs.set_fault(Fault::Error {
        at: 2,
        kind: io::ErrorKind::Other,
    });
    assert_eq!(
        ewz::extract(&vfs, &archive, target).unwrap_err().to_string(),
        format!(
            "{}: injected error; nothing was extracted",
            Path::new("/w/p.ew.tmp-ewz").display()
        )
    );
}

/// FT-1108: the parent's sync fails and the move back fails too (the process is killed at the sync): the complete
/// target stays, and the error says so instead of "nothing was extracted".
#[test]
fn ez3_a_failed_sync_and_a_failed_move_back_report_the_target_that_stays() {
    let archive = small_ewz();
    let target = Path::new("/w/p.ew");
    let vfs = workdir();
    ewz::extract(&vfs, &archive, target).expect("the extract succeeds");
    let total = vfs.calls();
    let vfs = workdir();
    vfs.set_fault(Fault::Kill { at: total });
    let e = ewz::extract(&vfs, &archive, target).unwrap_err();
    assert!(matches!(e, ExtractError::NotDurable { .. }), "{e:?}");
    assert_eq!(
        e.to_string(),
        format!(
            "{}: killed; {} was extracted whole and stays, but it may not survive a power failure",
            Path::new("/w").display(),
            target.display()
        )
    );
    vfs.restart();
    assert_eq!(vfs.files(target), archive.entries);
    assert_eq!(names(&vfs), ["p.ew"]);
}

/// The `MemVfs` learnt directory renames (ID-1175 ruling 4): the subtree moves, an empty target directory is replaced,
/// a non-empty one or a file is refused, and a rename into itself is refused.
#[test]
fn a_directory_rename_moves_its_subtree() {
    let vfs = MemVfs::new(DirSync::Explicit);
    vfs.put(Path::new("/w/a/x/1"), b"1");
    vfs.put(Path::new("/w/a/2"), b"2");
    vfs.put_dir(Path::new("/w/a/empty"));
    vfs.rename(Path::new("/w/a"), Path::new("/w/b")).unwrap();
    assert_eq!(
        vfs.files(Path::new("/w")),
        BTreeMap::from([
            ("b/2".to_owned(), b"2".to_vec()),
            ("b/x/1".to_owned(), b"1".to_vec())
        ])
    );
    assert_eq!(names(&vfs), ["b"]);
    assert!(vfs.stat(Path::new("/w/b/empty")).is_ok());
    vfs.put_dir(Path::new("/w/c"));
    vfs.rename(Path::new("/w/b"), Path::new("/w/c")).unwrap();
    assert_eq!(names(&vfs), ["c"]);
    vfs.put(Path::new("/w/d/f"), b"f");
    assert_eq!(
        vfs.rename(Path::new("/w/c"), Path::new("/w/d"))
            .unwrap_err()
            .kind(),
        io::ErrorKind::DirectoryNotEmpty
    );
    assert_eq!(
        vfs.rename(Path::new("/w/c"), Path::new("/w/d/f"))
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotADirectory
    );
    assert_eq!(
        vfs.rename(Path::new("/w/c"), Path::new("/w/c/x/y"))
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
    assert_eq!(
        vfs.rename_noreplace(Path::new("/w/c"), Path::new("/w/e"))
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput,
        "a rename without replacement moves files only"
    );
    // A rename is durable through its parent's sync.
    vfs.sync_dir(Path::new("/w")).unwrap();
    vfs.set_fault(Fault::PowerLoss {
        at: vfs.calls() + 1,
        keep: 0,
    });
    assert!(vfs.stat(Path::new("/w")).is_err());
    vfs.restart();
    assert_eq!(names(&vfs), ["c", "d"]);
    assert_eq!(vfs.read(Path::new("/w/c/x/1")).unwrap(), b"1");
}

/// `create_dir` is exclusive.
#[test]
fn create_dir_is_exclusive() {
    let vfs = workdir();
    vfs.create_dir(Path::new("/w/a")).unwrap();
    assert_eq!(
        vfs.create_dir(Path::new("/w/a")).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    vfs.put(Path::new("/w/f"), b"f");
    assert_eq!(
        vfs.create_dir(Path::new("/w/f")).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(
        vfs.create_dir(Path::new("/w/no/b")).unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
    let base = ew_testdir::fresh_dir("ew-ewz-create-dir");
    OsVfs.create_dir(&base.join("a")).unwrap();
    assert_eq!(
        OsVfs.create_dir(&base.join("a")).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// `bytes` with every occurrence of `from` replaced by `to` (the same length): a name in both of its headers.
fn patched(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
    assert_eq!(from.len(), to.len());
    let mut out = bytes.to_vec();
    let mut hits = 0;
    let mut i = 0;
    while i + from.len() <= out.len() {
        if &out[i..i + from.len()] == from.as_bytes() {
            out[i..i + from.len()].copy_from_slice(to.as_bytes());
            hits += 1;
            i += from.len();
        } else {
            i += 1;
        }
    }
    assert_eq!(hits, 2, "the name occurs in its local and its central header");
    out
}

/// The small fixture's entries as the container takes them (`mimetype` and `manifest.json` first), with `extra`
/// inserted in byte order and `change` applied to the list.
fn small_list(extra: &[(&str, &[u8])]) -> Vec<(String, Method, Vec<u8>)> {
    let mut files = small_fixture();
    let manifest = files.remove("manifest.json").unwrap();
    for (n, d) in extra {
        files.insert((*n).to_owned(), d.to_vec());
    }
    let mut list = vec![
        (
            "mimetype".to_owned(),
            Method::Stored,
            ewz::MIMETYPE.as_bytes().to_vec(),
        ),
        ("manifest.json".to_owned(), Method::Deflated, manifest),
    ];
    list.extend(files.into_iter().map(|(n, d)| (n, Method::Deflated, d)));
    list
}

fn written(list: &[(String, Method, Vec<u8>)]) -> Vec<u8> {
    let entries: Vec<(&str, Method, &[u8])> = list
        .iter()
        .map(|(n, m, d)| (n.as_str(), *m, d.as_slice()))
        .collect();
    zip::write(&entries).unwrap()
}

fn refusal(bytes: &[u8]) -> String {
    let e = ewz::read(bytes).expect_err("the archive is refused");
    e.to_string()
}

/// Deflatable data whose compressed size `c` is at least 327 bytes and whose size is exactly `201 · c`: a 201 : 1 entry
/// above the 64 KiB floor (ID-1175 ruling 3).
fn ratio_201() -> Vec<u8> {
    let noise: Vec<u8> = (0..420u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    let mut zeros = 80_000usize;
    for _ in 0..64 {
        let mut data = noise.clone();
        data.resize(noise.len() + zeros, 0);
        let one = zip::write(&[("x", Method::Deflated, data.as_slice())]).unwrap();
        // The compressed size of the only entry: the local header's bytes 18..22.
        let c = u32::from_le_bytes([one[18], one[19], one[20], one[21]]) as usize;
        if data.len() == 201 * c {
            assert!(c >= 327, "{c}");
            return data;
        }
        zeros = 201 * c - noise.len();
    }
    panic!("no exact 201 : 1 size found");
}

/// EZ-4: archives with `..`, a backslash, a duplicate name ignoring case, an entry outside D9's set, a hash mismatch, a
/// missing `mimetype` or a 201 : 1 entry are refused by the reader, each in words; so are an entry out of order, a
/// stored entry, a listed file missing, a manifest that lists a path outside the project's files, and a Windows
/// device name.
#[test]
fn ez4_hostile_archives_are_refused_unread() {
    let base = small_list(&[]);
    assert!(ewz::read(&written(&base)).is_ok(), "the unaltered archive reads");

    let dotdot = written(&small_list(&[("rules/ab/x", b"x")]));
    assert_eq!(
        refusal(&patched(&dotdot, "rules/ab/x", "rules/../x")),
        "the entry name `rules/../x` is not allowed: an empty, `.` or `..` segment"
    );
    let backslash = written(&small_list(&[("rules/a_b", b"x")]));
    assert_eq!(
        refusal(&patched(&backslash, "rules/a_b", "rules\\a_b")),
        "the entry name `rules\\a_b` is not allowed: a backslash"
    );
    let twice = written(&small_list(&[("rules/a", b"x"), ("rules/b", b"y")]));
    assert_eq!(
        refusal(&patched(&twice, "rules/b", "RULES/A")),
        "the entry name `RULES/A` occurs twice"
    );
    assert_eq!(
        refusal(&written(&small_list(&[("rules/extra.txt", b"x")]))),
        "the entry `rules/extra.txt` is not a file the manifest lists"
    );
    assert_eq!(
        refusal(&written(&small_list(&[("README.txt", b"x")]))),
        "the entry `README.txt` is not a file the manifest lists"
    );
    let mut changed = small_list(&[]);
    let project = changed.iter_mut().find(|(n, _, _)| n == "project.jsonl").unwrap();
    project.2.push(b'\n');
    assert_eq!(
        refusal(&written(&changed)),
        "the entry `project.jsonl` does not have the manifest's hash"
    );
    let mut no_mimetype = small_list(&[]);
    no_mimetype.remove(0);
    assert_eq!(
        refusal(&written(&no_mimetype)),
        "the first entry is not `mimetype`, stored, holding `application/vnd.elektrawatt.project+zip`"
    );
    let mut deflated_mimetype = small_list(&[]);
    deflated_mimetype[0].1 = Method::Deflated;
    assert_eq!(
        refusal(&written(&deflated_mimetype)),
        "the first entry is not `mimetype`, stored, holding `application/vnd.elektrawatt.project+zip`"
    );
    let bomb = ratio_201();
    assert_eq!(
        refusal(&written(&small_list(&[("rules/bomb", bomb.as_slice())]))),
        "the entry `rules/bomb` exceeds the limit on the compression ratio"
    );

    let mut swapped = small_list(&[]);
    let n = swapped.len();
    swapped.swap(n - 1, n - 2);
    let (last, before) = (swapped[n - 1].0.clone(), swapped[n - 2].0.clone());
    assert_eq!(
        refusal(&written(&swapped)),
        format!("the entry `{last}` is not in byte order after `{before}`")
    );
    let mut stored = small_list(&[]);
    stored[3].1 = Method::Stored;
    assert_eq!(
        refusal(&written(&stored)),
        format!("the entry `{}` is not deflated", stored[3].0)
    );
    let mut missing = small_list(&[]);
    let gone = missing.remove(4).0;
    assert_eq!(
        refusal(&written(&missing)),
        format!("`{gone}` is listed in the manifest but not in the archive")
    );
    let mut no_manifest = small_list(&[]);
    no_manifest.remove(1);
    assert_eq!(
        refusal(&written(&no_manifest)),
        "the second entry is not `manifest.json`, deflated"
    );
    let device = written(&small_list(&[("rules/xyz", b"x")]));
    assert_eq!(
        refusal(&patched(&device, "rules/xyz", "rules/aux")),
        "the entry name `rules/aux` is not allowed: a Windows device name"
    );

    // A manifest that lists a git hook: refused before any entry is taken.
    let mut hook = small_list(&[]);
    let text = String::from_utf8(hook[1].2.clone()).unwrap();
    let hooked = text.replacen(
        "\"files\": {",
        "\"files\": {\n    \".git/hooks/post-checkout\": \"sha256:0000000000000000000000000000000000000000000000000000000000000000\",",
        1,
    );
    assert_ne!(hooked, text, "the manifest's files member: {text}");
    hook[1].2 = hooked.into_bytes();
    assert_eq!(
        refusal(&written(&hook)),
        "the manifest lists `.git/hooks/post-checkout`, which is not a project file"
    );
}

/// The reader takes the default bounds only: an `.ewz` larger than 2 GiB is refused before it is read, and the bytes of
/// a file that is not a ZIP are refused in the container's words.
#[test]
fn a_file_that_is_no_archive_is_refused() {
    let vfs = workdir();
    vfs.put(Path::new("/w/x.ewz"), b"not a zip");
    let e = ewz::read_file(&vfs, Path::new("/w/x.ewz")).unwrap_err();
    assert!(matches!(e, EwzError::Zip(_)));
    assert_eq!(
        e.to_string(),
        "not an archive of the expected form: shorter than an end record"
    );
    assert!(ewz::is_ewz(&vfs, Path::new("/w/x.ewz")));
    assert!(
        ewz::is_ewz(&vfs, Path::new("/w/X.EWZ")),
        "a missing file of that name is an .ewz too"
    );
    vfs.put_dir(Path::new("/w/d.ewz"));
    assert!(!ewz::is_ewz(&vfs, Path::new("/w/d.ewz")), "a directory is not");
    assert!(!ewz::is_ewz(&vfs, Path::new("/w/p.ew")));
}

/// The golden fixture's own archive in memory reads to its files (the reader's happy path on Golden A's bytes).
#[test]
fn golden_a_in_memory_reads_back_to_its_files() {
    let bytes = pack_files(&fixture(), &BTreeMap::new());
    let read = ewz::read(&bytes).unwrap();
    assert_eq!(read.entries, fixture());
    let vfs = workdir();
    ewz::extract(&vfs, &read, &PathBuf::from("/w/J-E-2416.ew")).unwrap();
    assert_eq!(vfs.files(Path::new("/w/J-E-2416.ew")), fixture());
}
