// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! End-to-end tests of `ew fmt`, its repair and `ew recover` on a damaged or crashed project.

#[allow(clippy::wildcard_imports)]
use crate::cli::*;
#[allow(clippy::wildcard_imports)]
use crate::support::*;
use std::path::PathBuf;
use std::process::Output;

/// FM-3: `ew fmt` on a project with a crashed session's journal exits 2, pointing at `ew recover`, and writes nothing,
/// though a file is out of canonical form; the journal stays.
#[test]
fn fmt_refuses_a_project_with_a_crashed_session_journal() {
    let t = Tmp::new("ew-cli-test");
    let project = sample_copy(&t);
    swap_first_lines(&project, &device_file(&project));
    crashed_session(&t, &project, &[new_page(7777)]);
    let before = tree(&project);
    let journals = journal_files(&t);
    assert_eq!(journals.len(), 1);
    let o = t.ew(&["fmt".as_ref(), project.as_os_str()]);
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: a crashed session left unsaved changes in its journal; replay or discard them first with \
                 `ew recover`; nothing was written\n",
                project.display()
            )
        )
    );
    assert!(tree(&project) == before, "nothing is written");
    assert!(journal_files(&t) == journals, "the journal stays");
}

/// The first page file of `project` (`pages/<id>.jsonl`).
fn first_page_file(project: &std::path::Path) -> String {
    let first = tree(&project.join("pages"))
        .into_keys()
        .next()
        .expect("a page file");
    format!("pages/{first}")
}

/// `bytes` with `lines` inserted after their first line.
fn after_the_first_line(bytes: &[u8], lines: &[&[u8]]) -> Vec<u8> {
    let first = bytes.iter().position(|b| *b == b'\n').expect("a first line") + 1;
    let mut out = bytes[..first].to_vec();
    for line in lines {
        out.extend_from_slice(line);
        out.push(b'\n');
    }
    out.extend_from_slice(&bytes[first..]);
    out
}

/// Writes `bytes` to the project file `rel` with their hash in the manifest, as a save of those bytes writes it: lines
/// that came with the generation, not an edit outside ElektraWatt since the last save.
pub(crate) fn put_as_saved(project: &std::path::Path, rel: &str, bytes: &[u8]) {
    let hash = |b: &[u8]| format!("sha256:{}", sha256_hex(b));
    let (old, new) = (hash(&std::fs::read(project.join(rel)).unwrap()), hash(bytes));
    let manifest = std::fs::read_to_string(project.join("manifest.json")).unwrap();
    assert_eq!(manifest.matches(&old).count(), 1, "the manifest lists {rel} once");
    std::fs::write(project.join("manifest.json"), manifest.replace(&old, &new)).unwrap();
    std::fs::write(project.join(rel), bytes).unwrap();
}

/// The generation of the sample's copies: that of its script's saves.
pub(crate) fn sample_generation() -> u64 {
    generation(std::path::Path::new(SAMPLE))
}

/// The backup `ew fmt --repair` writes for `project` at `generation`: beside the project folder, named
/// `<project folder>.before-repair-g<generation>.ewz`, as the file system names it.
fn backup_of(project: &std::path::Path, generation: u64) -> PathBuf {
    let canonical = std::fs::canonicalize(project).unwrap();
    let text = canonical.to_string_lossy().into_owned();
    let plain = PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text));
    let name = format!(
        "{}.before-repair-g{generation}.ewz",
        plain.file_name().unwrap().to_string_lossy()
    );
    plain.parent().unwrap().join(name)
}

/// The names of the files beside `project` that start with its folder's name and `.before-repair`.
fn beside_the_project(project: &std::path::Path) -> Vec<String> {
    let prefix = format!("{}.before-repair", project.file_name().unwrap().to_string_lossy());
    let mut names: Vec<String> = std::fs::read_dir(project.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(&prefix))
        .collect();
    names.sort();
    names
}

/// EZ-5: `ew fmt --repair` on a page file of the sample's copy with one broken line
/// prints the line (its text cut at 80 characters) and the backup, writes the backup `.ewz` beside the project folder
/// (generation 1 as it was, the line included) and a project without the line, which opens editable and canonical; a
/// second run has nothing to repair and writes nothing; run on the project as it was, it reuses the backup.
#[test]
fn fmt_repair_discards_a_broken_line_after_a_backup() {
    let t = Tmp::new("ew-cli-test");
    let project = sample_copy(&t);
    let rel = first_page_file(&project);
    let original = std::fs::read(project.join(&rel)).unwrap();
    let broken = format!("{{\"$k\":\"page\",\"title\":\"{}\"", "x".repeat(100));
    let bytes = after_the_first_line(&original, &[broken.as_bytes()]);
    put_as_saved(&project, &rel, &bytes);
    let saved = t.0.join("as-it-was.ew");
    copy_dir(&project, &saved);
    let before = tree(&project);
    let fmt = |extra: &[&str]| ew_in(&t, &["fmt".as_ref(), project.as_os_str()], extra);
    let run = |o: &Output| (o.status.code(), stdout(o), stderr(o));
    let backup = backup_of(&project, sample_generation());
    let shown: String = broken.chars().take(80).collect();
    let discarded = format!("discarded {rel}:2: {shown}\u{2026}\n");
    assert_eq!(
        run(&fmt(&["--repair"])),
        (
            Some(0),
            format!(
                "{discarded}backup {} (generation {})\n",
                backup.display(),
                sample_generation()
            ),
            format!(
                "Repaired: discarded 1 line; saved generation {}\n",
                sample_generation() + 1
            )
        )
    );
    let after = tree(&project);
    assert_eq!(after[&rel], original, "the page file without the line");
    let changed: Vec<&String> = after.keys().filter(|k| after.get(*k) != before.get(*k)).collect();
    assert_eq!(
        changed,
        ["manifest.json", &rel],
        "only the manifest and the page file"
    );
    let packed = ew_api::ewz::read(&std::fs::read(&backup).unwrap()).unwrap();
    assert_eq!(packed.generation(), sample_generation());
    assert_eq!(packed.entries[&rel], bytes, "the backup holds the broken line");
    assert_eq!(
        beside_the_project(&project),
        [format!("sample.ew.before-repair-g{}.ewz", sample_generation())],
        "no temporary file is left"
    );
    assert_eq!(
        run(&fmt(&["--check"])),
        (
            Some(0),
            String::new(),
            "fmt: every file is in canonical form\n".to_owned()
        )
    );

    let now = tree(&project);
    assert_eq!(
        run(&fmt(&["--repair"])),
        (Some(0), String::new(), "Nothing to repair\n".to_owned())
    );
    assert!(tree(&project) == now, "nothing is written");

    std::fs::remove_dir_all(&project).unwrap();
    copy_dir(&saved, &project);
    assert_eq!(
        run(&fmt(&["--repair"])),
        (
            Some(0),
            format!(
                "{discarded}backup {} (generation {}, already there and reused)\n",
                backup.display(),
                sample_generation()
            ),
            format!(
                "Repaired: discarded 1 line; saved generation {}\n",
                sample_generation() + 1
            )
        )
    );
    assert_eq!(tree(&project)[&rel], original);
    assert!(journal_files(&t).is_empty(), "the repair's journal is closed");
}

/// EZ-5 on a hand-edited copy of the sample: the broken line written by hand,
/// the manifest untouched (an edit outside ElektraWatt since the last save), and the same with the page file's hash
/// pending in a merged manifest. `ew export ewz` refuses the project, which is no verified generation; `ew fmt
/// --repair` writes the backup of the files as found, saying how many differ from the generation as saved, then the
/// project without the line, which opens editable and canonical; `ew extract` makes a project of the backup, its
/// manifest verified and hashed to the files, every listed file as it was before the repair.
#[test]
fn fmt_repair_backs_up_a_hand_edited_project_as_found() {
    const NOT_JSON: &[u8] = br#"{"$k":"page","id":"#;
    let hash = |b: &[u8]| format!("sha256:{}", sha256_hex(b));
    let run = |o: &Output| (o.status.code(), stdout(o), stderr(o));
    for merged in [false, true] {
        let t = Tmp::new("ew-cli-test");
        let project = sample_copy(&t);
        let rel = first_page_file(&project);
        let original = std::fs::read(project.join(&rel)).unwrap();
        let bytes = after_the_first_line(&original, &[NOT_JSON]);
        std::fs::write(project.join(&rel), &bytes).unwrap();
        let listed = if merged {
            let manifest = std::fs::read_to_string(project.join("manifest.json")).unwrap();
            assert_eq!(manifest.matches(&hash(&original)).count(), 1);
            // A merged manifest has no `base`: the sample's saved one has.
            let mut pending = manifest.replace(&hash(&original), "sha256:pending");
            let end = pending.find("\n  },\n  \"base\": ").expect("the manifest's base");
            pending.replace_range(end.., "\n  },\n  \"merged\": true\n}\n");
            assert!(pending.ends_with("\"merged\": true\n}\n"));
            std::fs::write(project.join("manifest.json"), pending).unwrap();
            "sha256:pending".to_owned()
        } else {
            hash(&original)
        };
        let before = tree(&project);
        let mut found = before.clone();
        let manifest = String::from_utf8(before["manifest.json"].clone()).unwrap();
        found.insert(
            "manifest.json".to_owned(),
            manifest.replace(&listed, &hash(&bytes)).into_bytes(),
        );

        let out = t.0.join("export.ewz");
        let export = ew_in(
            &t,
            &[
                "export".as_ref(),
                "ewz".as_ref(),
                project.as_os_str(),
                "--out".as_ref(),
                out.as_os_str(),
            ],
            &[],
        );
        let why = if merged {
            format!("the manifest's hash of {rel} is pending (a merged project); save it first")
        } else {
            format!("{rel} changed on disk since it was saved (an external change)")
        };
        assert_eq!(
            run(&export),
            (
                Some(2),
                String::new(),
                format!("error: {}: {why}\n", project.display())
            )
        );
        assert!(!out.exists(), "the export writes nothing");

        let backup = backup_of(&project, sample_generation());
        let repair = ew_in(
            &t,
            &["fmt".as_ref(), project.as_os_str(), "--repair".as_ref()],
            &[],
        );
        assert_eq!(
            run(&repair),
            (
                Some(0),
                format!(
                    "discarded {rel}:2: {}\nbackup {} (generation {}, as found: 1 file edited outside EW)\n",
                    String::from_utf8_lossy(NOT_JSON),
                    backup.display(),
                    sample_generation()
                ),
                format!(
                    "Repaired: discarded 1 line; saved generation {}\n",
                    sample_generation() + 1
                )
            ),
            "merged: {merged}"
        );
        let after = tree(&project);
        assert_eq!(after[&rel], original, "the page file without the line");
        let changed: Vec<&String> = after.keys().filter(|k| after.get(*k) != before.get(*k)).collect();
        assert_eq!(
            changed,
            ["manifest.json", &rel],
            "only the manifest and the page file"
        );
        assert_eq!(
            run(&ew_in(
                &t,
                &["fmt".as_ref(), project.as_os_str(), "--check".as_ref()],
                &[]
            )),
            (
                Some(0),
                String::new(),
                "fmt: every file is in canonical form\n".to_owned()
            )
        );

        let restored = t.0.join("restored.ew");
        let extract = ew_in(
            &t,
            &[
                "extract".as_ref(),
                backup.as_os_str(),
                "--into".as_ref(),
                restored.as_os_str(),
            ],
            &[],
        );
        assert_eq!(
            run(&extract),
            (
                Some(0),
                format!(
                    "extracted into {}: generation {}, {} files\n",
                    restored.display(),
                    sample_generation(),
                    found.len()
                ),
                String::new()
            )
        );
        assert!(
            tree(&restored) == found,
            "every listed file as it was before the repair, the manifest hashed to them"
        );
    }
}

/// The refusals come before anything is written, each named, exit 2, in this order: an `.ewz`; a
/// crashed session's journal (here before a conflict marker the repair cannot discard); a finding other than a broken
/// line that forces read-only (EW-CORE-006), also one beyond the findings cap; a project whose backup cannot be packed
/// (no manifest; a page file edited outside ElektraWatt since the last save is packed as found); a backup name
/// holding another file. And `--repair` with `--check` is refused by the parser.
#[test]
fn fmt_repair_refusals_come_first_and_write_nothing() {
    const NOT_JSON: &[u8] = br#"{"$k":"page","id":"#;
    const MARKER: &[u8] = b"<<<<<<< HEAD";
    let run = |o: &Output| (o.status.code(), stdout(o), stderr(o));
    let repair = |t: &Tmp, p: &std::path::Path, extra: &[&str]| {
        ew_in(t, &["fmt".as_ref(), p.as_os_str(), "--repair".as_ref()], extra)
    };
    // The project of a fresh temporary directory, its first page file holding `lines` after its first line, written as
    // a save writes them (`saved`) or by hand.
    let project = |lines: &[&[u8]], saved: bool| {
        let t = Tmp::new("ew-cli-test");
        let p = tiny_project(&t);
        let rel = first_page_file(&p);
        let bytes = after_the_first_line(&std::fs::read(p.join(&rel)).unwrap(), lines);
        if saved {
            put_as_saved(&p, &rel, &bytes);
        } else {
            std::fs::write(p.join(&rel), &bytes).unwrap();
        }
        (t, p, rel)
    };

    // An `.ewz`.
    let t = Tmp::new("ew-cli-test");
    let ewz = t.0.join("x.ewz");
    std::fs::write(&ewz, b"not read").unwrap();
    assert_eq!(
        run(&repair(&t, &ewz, &[])),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: an .ewz opens read-only; extract it first\n",
                ewz.display()
            )
        )
    );

    // A crashed session's journal, before the conflict marker.
    let (t, p, _) = project(&[], true);
    crashed_session(&t, &p, &[new_page(7777)]);
    let rel = first_page_file(&p);
    let bytes = after_the_first_line(&std::fs::read(p.join(&rel)).unwrap(), &[NOT_JSON, MARKER]);
    std::fs::write(p.join(&rel), &bytes).unwrap();
    let (before, journals) = (tree(&p), journal_files(&t));
    assert_eq!(
        run(&repair(&t, &p, &[])),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: a crashed session left unsaved changes in its journal; replay or discard them first with \
                 `ew recover`; nothing was written\n",
                p.display()
            )
        )
    );
    assert!(
        tree(&p) == before && journal_files(&t) == journals,
        "nothing is written; the journal stays"
    );

    // A conflict marker (EW-CORE-006) beside a broken line.
    let (t, p, rel) = project(&[NOT_JSON, MARKER], false);
    let before = tree(&p);
    assert_eq!(
        run(&repair(&t, &p, &[])),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: EW-CORE-006 at {rel}:3 forces read-only, and `ew fmt --repair` discards only lines that \
                 are not JSON or not UTF-8 (EW-CORE-005); nothing was written; resolve it first (`ew check` lists the \
                 findings), then run `ew fmt --repair` again\n",
                p.display()
            )
        )
    );
    assert!(tree(&p) == before, "nothing is written");

    // The marker after 1,001 broken lines: its finding is beyond the cap of the open's list, and still refuses.
    let mut lines = vec![NOT_JSON; 1_001];
    lines.push(MARKER);
    let (t, p, _) = project(&lines, false);
    let before = tree(&p);
    assert_eq!(
        run(&repair(&t, &p, &[])),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: a finding beyond those the open lists forces read-only, and `ew fmt --repair` discards \
                 only lines that are not JSON or not UTF-8 (EW-CORE-005); nothing was written; resolve it first (`ew \
                 check` lists the findings), then run `ew fmt --repair` again\n",
                p.display()
            )
        )
    );
    assert!(tree(&p) == before, "nothing is written");

    // A project without a manifest: its files cannot be packed as found.
    let (t, p, _) = project(&[NOT_JSON], false);
    std::fs::remove_file(p.join("manifest.json")).unwrap();
    let before = tree(&p);
    assert_eq!(
        run(&repair(&t, &p, &[])),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: the backup cannot be packed: the project has no manifest.json; nothing was written; keep a \
                 copy of the project folder and correct the broken lines by hand\n",
                p.display()
            )
        )
    );
    assert!(tree(&p) == before, "nothing is written");
    assert_eq!(beside_the_project(&p), Vec::<String>::new());

    // A file at the backup's name that is not this generation's backup.
    let (t, p, _) = project(&[NOT_JSON], true);
    let backup = backup_of(&p, 1);
    std::fs::write(&backup, b"another file").unwrap();
    let before = tree(&p);
    assert_eq!(
        run(&repair(&t, &p, &[])),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: {} already exists and does not hold this generation's backup; nothing was written; move it \
                 out of the folder, then run `ew fmt --repair` again\n",
                p.display(),
                backup.display()
            )
        )
    );
    assert!(tree(&p) == before, "nothing is written");
    assert_eq!(std::fs::read(&backup).unwrap(), b"another file");

    // `--repair` with `--check`: the parser's refusal, before any open.
    assert_eq!(
        run(&repair(&t, &p, &["--check"])),
        (
            Some(2),
            String::new(),
            "error: the argument '--repair' cannot be used with '--check'\n\nUsage: ew fmt --repair <project-dir>\n\n\
             For more information, try '--help'.\n"
                .to_owned()
        )
    );
    drop(t);
}

/// FM-4: `ew recover` prints the journal's summary and changes nothing; `--discard` off a terminal exits 2
/// before the project is opened (the typed paths are unit tests with the injected terminal, `cli/project_files.rs`);
/// `--replay` saves the journal's transactions and removes it.
#[test]
fn recover_summarises_and_replays_a_crashed_session_journal() {
    let t = Tmp::new("ew-cli-test");
    let project = sample_copy(&t);
    crashed_session(&t, &project, &[new_page(7777), new_page(7778)]);
    let before = tree(&project);
    let journals = journal_files(&t);
    let recover = |extra: &[&str]| ew_in(&t, &["recover".as_ref(), project.as_os_str()], extra);
    let run = |o: &Output| (o.status.code(), stdout(o), stderr(o));
    let summary = "a crashed session left 2 unsaved transactions in its journal:\n  1. page.create\n  2. page.create\nthe \
                   project is unchanged since that session last opened or saved it\n";
    assert_eq!(
        run(&recover(&[])),
        (
            Some(0),
            format!(
                "{summary}`ew recover --replay` replays and saves them; `ew recover --discard` deletes them\n"
            ),
            String::new()
        )
    );
    assert!(
        tree(&project) == before && journal_files(&t) == journals,
        "the summary changes nothing"
    );

    assert_eq!(
        run(&recover(&["--discard"])),
        (
            Some(2),
            String::new(),
            "error: HumanOnly: needs an interactive terminal (`recover --discard` is confirmed by typing `discard`; \
             `ew recover` prints the summary without one); nothing was discarded\n"
                .to_owned()
        )
    );
    assert!(
        tree(&project) == before && journal_files(&t) == journals,
        "off a terminal nothing changes"
    );

    assert_eq!(
        run(&recover(&["--replay"])),
        (
            Some(0),
            format!(
                "{summary}replayed 2 transactions of the crashed session's journal; saved generation {}\n",
                sample_generation() + 1
            ),
            String::new()
        )
    );
    assert!(journal_files(&t).is_empty(), "the journal is removed");
    let pages = saved_pages(&project);
    assert!(
        pages.contains(&"7777".to_owned()) && pages.contains(&"7778".to_owned()),
        "{pages:?}"
    );
    assert_eq!(
        run(&recover(&[])),
        (
            Some(0),
            "no journal of a crashed session: nothing to recover\n".to_owned(),
            String::new()
        )
    );
}

/// Replaying a segment whose base is not the open generation behaves exactly as the existing
/// code does for that case today (refused or applied). A crashed session set page 1's title; another session set the
/// same title and saved; `ew recover --replay` is refused (exit 1) and keeps the journal and the project unchanged.
#[test]
fn recover_refuses_a_replay_that_collides_with_a_later_save() {
    let t = Tmp::new("ew-cli-test");
    let project = sample_copy(&t);
    // A page's ID: its file name (`pages/<id>.jsonl`).
    let page = std::fs::read_dir(project.join("pages"))
        .unwrap()
        .map(|e| {
            let name = e.unwrap().file_name().to_string_lossy().into_owned();
            name.trim_end_matches(".jsonl").to_owned()
        })
        .min()
        .expect("a page");
    let title = |text: &str| ew_api::AnyCommand {
        name: "fields.set".into(),
        params: serde_json::json!({"kind": "page", "targets": [page], "path": "title", "value": {"en": text}}),
    };
    crashed_session(&t, &project, &[title("from the crashed session")]);
    let mut store = ew_api::StoreOptions::new("test");
    store.state_dir = None;
    let (mut other, _) = ew_api::Session::open(
        &project,
        store,
        ew_api::SessionOptions::default(),
        ew_api::Recovery::Keep,
    )
    .unwrap();
    other.execute(&title("from a later session")).unwrap();
    other.save().unwrap();
    other.close();
    let before = tree(&project);
    let journals = journal_files(&t);

    let o = t.ew(&["recover".as_ref(), project.as_os_str(), "--replay".as_ref()]);
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(1),
            "a crashed session left 1 unsaved transaction in its journal:\n  1. fields.set\nthe project was saved by \
             another session since; a replay is refused when it meets a field changed in that save\n"
                .to_owned(),
            "error: the replay is refused: 1 field of the journal changed on disk too; the journal is kept and \
             nothing was saved (`ew recover --discard` deletes it)\n"
                .to_owned()
        )
    );
    assert!(tree(&project) == before, "nothing is saved");
    assert!(journal_files(&t) == journals, "the journal is kept, unchanged");
}

/// `ew recover` checks that it got edit access; if not, it refuses with exit 2 and the same
/// text `ew apply` uses for a held lock; it never acts read-only. While this test process holds the project's lock,
/// `ew recover` and `ew apply` exit 2 with the same text, `ew apply`'s as built: the read-only reason a second open
/// sees, with the lock's holder (FR-COL-006); the crashed session's journal stays. The holder's user name
/// holds a right-to-left override (U+202E), which the refusal prints escaped.
#[test]
fn recover_and_apply_refuse_a_held_lock_with_the_same_text() {
    let t = Tmp::new("ew-cli-test");
    let project = sample_copy(&t);
    crashed_session(&t, &project, &[new_page(7777)]);
    let journals = journal_files(&t);
    let mut store = ew_api::StoreOptions::new("test");
    store.state_dir = None;
    store.identity.user = "ops\u{202E}fdp.exe".to_owned();
    let (holder, opened) = ew_api::Session::open(
        &project,
        store,
        ew_api::SessionOptions::default(),
        ew_api::Recovery::Keep,
    )
    .unwrap();
    assert_eq!(opened.access, ew_api::Access::Edit);
    // A second open in this process sees the lock as the binary does (the holder is alive and has another session).
    let mut second = ew_api::StoreOptions::new("test");
    second.state_dir = None;
    let (reader, seen) = ew_api::Session::open(
        &project,
        second,
        ew_api::SessionOptions::default(),
        ew_api::Recovery::Keep,
    )
    .unwrap();
    let ew_api::Access::ReadOnly(ew_api::ReadOnlyReason::LockedBy(info)) = &seen.access else {
        panic!("the second open is read-only by the lock: {:?}", seen.access);
    };
    assert!(
        info.record
            .as_ref()
            .is_some_and(|r| !r.user.is_empty() && !r.host.is_empty()),
        "the reason names the holder: {info:?}"
    );
    // "another session holds the project" plus " (<user> on <host>, since
    // <time>)" when the lock record is readable (no `@`, no session or process ID).
    let holder_record = info.record.as_ref().unwrap();
    assert_eq!(holder_record.user, "ops\u{202E}fdp.exe");
    let expected = format!(
        "error: {}: the project opens read-only: another session holds the project (ops\\u{{202e}}fdp.exe on {}, since \
         {}); nothing was written\n",
        project.display(),
        holder_record.host,
        holder_record.started_at
    );
    let words = expected.split_once("read-only: ").unwrap().1;
    assert!(
        !words.contains(&holder_record.session) && !words.contains('@'),
        "{words}"
    );
    reader.close();
    let before = tree(&project);
    let log = renumber_log(&t);
    for args in [
        vec!["recover".as_ref(), project.as_os_str()],
        vec!["recover".as_ref(), project.as_os_str(), "--replay".as_ref()],
        vec!["apply".as_ref(), project.as_os_str(), log.as_os_str()],
    ] {
        let o = t.ew(&args);
        assert_eq!(
            (o.status.code(), stdout(&o), stderr(&o)),
            (Some(2), String::new(), expected.clone()),
            "{args:?}"
        );
    }
    assert!(tree(&project) == before, "nothing is written");
    assert!(journal_files(&t) == journals, "the journal stays");
    holder.close();
}
