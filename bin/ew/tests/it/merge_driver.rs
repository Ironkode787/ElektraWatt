// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.17 part 4b through the `ew` binary (D15, D29): `ew merge-driver` in a working tree that
//! holds a copy of Golden A, as git runs it. MD-5: the exits 0 and 1, the four-argument form, an internal error's 129
//! with `%A` byte-identical; MD-6: AC-41's driver half on W20 of page 102 and the manifest merge. Golden A is only
//! read and copied.

use std::path::{Path, PathBuf};
use std::process::Output;

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
/// Page 102 in the project, and in the working tree (the project in `proj/`).
const PAGE: &str = "pages/4aa43308-d0d3-57ee-be0c-d9383f429f50.jsonl";
const REPO_PAGE: &str = "proj/J-E-2416.ew/pages/4aa43308-d0d3-57ee-be0c-d9383f429f50.jsonl";
const W20_OVERRIDES: &str = r#""overrides":{"csa":"1.5","show_marker":true}"#;

/// A working tree with a copy of Golden A in `proj/J-E-2416.ew`, removed on drop.
struct Tree(PathBuf);

impl Tree {
    fn new() -> Tree {
        let root = ew_testdir::fresh_dir("ew-merge-driver");
        super::conventions::copy_dir(Path::new(GOLDEN_A), &root.join("proj/J-E-2416.ew"));
        Tree(root)
    }

    /// Writes the three sides as git's temporary files and runs `ew merge-driver` with `args_for`'s arguments.
    fn merge(
        &self,
        sides: [&str; 3],
        args_for: impl Fn(&Path, &Path, &Path) -> Vec<String>,
    ) -> (Output, String) {
        let [o, a, b] = ["o", "a", "b"].map(|n| self.0.join(format!(".merge_file_{n}")));
        for (path, text) in [(&o, sides[0]), (&a, sides[1]), (&b, sides[2])] {
            std::fs::write(path, text).unwrap();
        }
        let out = self.run(&args_for(&o, &a, &b));
        let merged = String::from_utf8(std::fs::read(&a).unwrap()).unwrap();
        (out, merged)
    }

    fn run(&self, args: &[String]) -> Output {
        std::process::Command::new(super::conventions::bin_exe())
            .arg("merge-driver")
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// The canonical five-argument form `%O %A %B %L %P` (`five`), else the older `%O %A %B %P`.
fn form(five: bool, path: &'static str) -> impl Fn(&Path, &Path, &Path) -> Vec<String> {
    move |o, a, b| {
        let mut args = vec![s(o), s(a), s(b)];
        if five {
            args.push("7".to_owned());
        }
        args.push(path.to_owned());
        args
    }
}

/// The exit code, stdout and stderr.
fn result(o: &Output) -> (Option<i32>, String, String) {
    (
        o.status.code(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

/// Golden A's page-102 file with W20's overrides replaced (as part 4a's `ac41_w20_*` tests).
fn page_with(overrides: &str) -> String {
    let base = std::fs::read_to_string(Path::new(GOLDEN_A).join(PAGE)).unwrap();
    let w20 = base
        .lines()
        .find(|l| l.contains("connections|W20"))
        .expect("W20")
        .to_owned();
    assert!(w20.contains(W20_OVERRIDES), "{w20}");
    base.replace(&w20, &w20.replace(W20_OVERRIDES, overrides))
}

/// W20's page of the common ancestor, of branch A (adds `colour` BU) and of branch B (sets `csa` "2.5").
fn clean_sides() -> (String, String, String) {
    (
        page_with(W20_OVERRIDES),
        page_with(r#""overrides":{"csa":"1.5","colour":["BU"],"show_marker":true}"#),
        page_with(r#""overrides":{"csa":"2.5","show_marker":true}"#),
    )
}

/// The page the two branches of [`clean_sides`] merge to.
fn merged_sides() -> String {
    page_with(r#""overrides":{"csa":"2.5","colour":["BU"],"show_marker":true}"#)
}

/// MD-5, MD-6: branch A adds W20's `colour` BU, branch B sets `csa` "2.5": exit 0 and the merged page, in both forms.
#[test]
fn md6_w20_merges_cleanly_to_csa_2_5_and_colour_bu_and_exits_0() {
    let t = Tree::new();
    let (base, ours, theirs) = clean_sides();
    let expected = merged_sides();
    for five in [true, false] {
        let (out, merged) = t.merge([&base, &ours, &theirs], form(five, REPO_PAGE));
        assert_eq!(result(&out), (Some(0), String::new(), String::new()));
        assert_eq!(merged, expected);
    }
    assert!(!t.0.join(".merge_file_a.tmp").exists());
}

/// MD-5, MD-6: the conflict variant (B also adds `colour` WH) writes one conflict record on `overrides.colour`, keeps
/// BU in place and exits 1, in both forms.
#[test]
fn md6_the_w20_conflict_variant_writes_one_conflict_record_and_exits_1() {
    let t = Tree::new();
    let (base, ours, _) = clean_sides();
    let theirs = page_with(r#""overrides":{"csa":"2.5","colour":["WH"],"show_marker":true}"#);
    let kept = merged_sides();
    // D14's member order.
    let conflict = r#"{"$k":"conflict","id":"1c5963b7-6c34-53d6-8fa6-9d02d4629711","kind":"connection","base":{"overrides":{}},"ours":{"overrides":{"colour":["BU"]}},"theirs":{"overrides":{"colour":["WH"]}}}"#;
    for five in [true, false] {
        let (out, merged) = t.merge([&base, &ours, &theirs], form(five, REPO_PAGE));
        assert_eq!(result(&out), (Some(1), String::new(), String::new()));
        assert_eq!(merged, format!("{kept}{conflict}\n"));
    }
}

/// MD-6: the manifest merge gives the larger generation plus one, `merged: true` and no `base`; the page both
/// branches changed is pending, a file one branch changed keeps that branch's hash.
#[test]
fn md6_the_manifest_merges_to_max_plus_one_merged_and_no_base() {
    let t = Tree::new();
    let base = std::fs::read_to_string(Path::new(GOLDEN_A).join("manifest.json")).unwrap();
    // A page's hash as the committed manifest holds it (it changes whenever Golden A's fixture is regenerated).
    let page_hash = base
        .lines()
        .find(|l| l.contains("\"pages/4aa43308-d0d3-57ee-be0c-d9383f429f50.jsonl\""))
        .and_then(|l| l.split('"').nth(3))
        .expect("the page in the manifest")
        .to_owned();
    let page_hash = page_hash.as_str();
    let device_hash = "sha256:e82c564a97ba572b71c077519883188930ab7b94f975b3567d2cea9f1dad8d66";
    let end = "\n  }\n}\n";
    assert!(base.contains("\"generation\": 1,") && base.contains(page_hash) && base.contains(device_hash));
    assert!(base.ends_with(end));
    let hash = |n: &str| format!("sha256:{}", n.repeat(64));
    let ours = base
        .replace("\"generation\": 1,", "\"generation\": 3,")
        .replace(page_hash, &hash("a"))
        .replace(device_hash, &hash("c"))
        .replace(end, &format!("\n  }},\n  \"base\": \"{}\"\n}}\n", hash("d")));
    let theirs = base
        .replace("\"generation\": 1,", "\"generation\": 2,")
        .replace(page_hash, &hash("b"))
        .replace(end, &format!("\n  }},\n  \"base\": \"{}\"\n}}\n", hash("e")));
    let expected = base
        .replace("\"generation\": 1,", "\"generation\": 4,")
        .replace(page_hash, "sha256:pending")
        .replace(device_hash, &hash("c"))
        .replace(end, "\n  },\n  \"merged\": true\n}\n");
    let (out, merged) = t.merge(
        [&base, &ours, &theirs],
        form(true, "proj/J-E-2416.ew/manifest.json"),
    );
    assert_eq!(result(&out), (Some(0), String::new(), String::new()));
    assert_eq!(merged, expected);
}

/// FT-1235 (RL-3): theirs' manifest names a format newer than this reader's: nothing is merged, `%A`
/// stays ours byte for byte, the driver says so and exits 1. Ours' newer format (the working tree's manifest) keeps a
/// page of that project as ours the same way.
#[test]
fn a_side_newer_than_the_reader_is_left_as_ours() {
    let t = Tree::new();
    let base = std::fs::read_to_string(Path::new(GOLDEN_A).join("manifest.json")).unwrap();
    let format = format!("\"format\": \"{}\"", ew_api::FORMAT);
    assert!(base.contains(&format));
    let ours = base.replace("\"generation\": 1,", "\"generation\": 2,");
    let theirs = base
        .replace("\"generation\": 1,", "\"generation\": 3,")
        .replace(&format, "\"format\": \"9.0\"");
    let manifest = "proj/J-E-2416.ew/manifest.json";
    let (out, merged) = t.merge([&base, &ours, &theirs], form(true, manifest));
    let said = |path: &str| {
        format!(
            "merge-driver: {path}: written by a newer ElektraWatt (format 9.0; this one reads {}); left as ours: \
             merge it with that version\n",
            ew_api::FORMAT
        )
    };
    assert_eq!(result(&out), (Some(1), String::new(), said(manifest)));
    assert_eq!(merged, ours);

    std::fs::write(t.0.join(manifest), &theirs).unwrap();
    let (base, ours, theirs) = clean_sides();
    let (out, merged) = t.merge([&base, &ours, &theirs], form(false, REPO_PAGE));
    assert_eq!(result(&out), (Some(1), String::new(), said(REPO_PAGE)));
    assert_eq!(merged, ours);
}

/// Theirs holds W20 one `$v` above this reader's version of a connection (1): nothing is merged
/// in the page, `%A` stays ours byte for byte, the driver says so and exits 1. Theirs holding a member this reader does
/// not know merges cleanly, the member kept.
#[test]
fn a_record_this_reader_cannot_read_leaves_the_file_as_ours() {
    let t = Tree::new();
    let id = r#""id":"1c5963b7-6c34-53d6-8fa6-9d02d4629711""#;
    let w20 = |page: String, from: &str, to: &str| -> String {
        let line = page.lines().find(|l| l.contains(id)).expect("W20").to_owned();
        page.replace(&line, &line.replacen(from, to, 1))
    };
    let (base, ours, theirs) = clean_sides();
    let newer = w20(theirs.clone(), id, &format!("{id},\"$v\":2"));
    assert_ne!(newer, theirs);
    let (out, merged) = t.merge([&base, &ours, &newer], form(true, REPO_PAGE));
    let said = format!(
        "merge-driver: {REPO_PAGE}: holds records this ElektraWatt cannot read (this one reads format {}); left as \
         ours: merge it with a newer version\n",
        ew_api::FORMAT
    );
    assert_eq!(result(&out), (Some(1), String::new(), said));
    assert_eq!(merged, ours);

    let end = r#""key":"connections|W20"}}"#;
    let later = w20(theirs, end, r#""key":"connections|W20"},"later":"kept"}"#);
    let expected = w20(merged_sides(), end, r#""key":"connections|W20"},"later":"kept"}"#);
    let (out, merged) = t.merge([&base, &ours, &later], form(true, REPO_PAGE));
    assert_eq!(result(&out), (Some(0), String::new(), String::new()));
    assert_eq!(merged, expected);
}

/// A library document whose theirs is one `$v` above this reader's version of a
/// manufacturer (1) is not merged either: `%A` stays ours byte for byte, the driver prints S3b and exits 1.
#[test]
fn a_library_document_this_reader_cannot_read_is_left_as_ours() {
    const DOC: &str = "proj/J-E-2416.ew/lib/manufacturers/221fd06e-3263-5cab-bbb9-27aee71c8d79.json";
    let t = Tree::new();
    let base = std::fs::read_to_string(t.0.join(DOC)).unwrap();
    let ours = base.replace(r#""name": "Siemens""#, r#""name": "Siemens AG""#);
    let theirs = base.replace(r#""$k": "manufacturer","#, r#""$k": "manufacturer", "$v": 2,"#);
    assert!(ours != base && theirs != base);
    let (out, merged) = t.merge([&base, &ours, &theirs], form(true, DOC));
    let said = format!(
        "merge-driver: {DOC}: holds records this ElektraWatt cannot read (this one reads format {}); left as ours: \
         merge it with a newer version\n",
        ew_api::FORMAT
    );
    assert_eq!(result(&out), (Some(1), String::new(), said));
    assert_eq!(merged, ours);
}

/// MD-5, ruling 6: a write that fails (the temporary file's name is taken by a folder) exits 129 and leaves `%A`
/// byte-identical, the folder as it was.
#[test]
fn md5_a_failed_write_exits_129_and_leaves_a_byte_identical() {
    let t = Tree::new();
    let (base, ours, theirs) = clean_sides();
    let tmp = t.0.join(".merge_file_a.tmp");
    std::fs::create_dir(&tmp).unwrap();
    let os_error = std::fs::File::create(&tmp).unwrap_err();
    let (out, merged) = t.merge([&base, &ours, &theirs], form(true, REPO_PAGE));
    assert_eq!(
        result(&out),
        (
            Some(129),
            String::new(),
            format!(
                "ew merge-driver: the merged {REPO_PAGE} cannot be written to {}: {os_error}; it is left as it was\n",
                t.0.join(".merge_file_a").display()
            )
        )
    );
    assert_eq!(merged, ours);
    assert!(tmp.is_dir());
}

/// MD-5, ruling 6: a side that cannot be read exits 129 and leaves `%A` byte-identical.
#[test]
fn md5_a_side_that_cannot_be_read_exits_129() {
    let t = Tree::new();
    let missing = t.0.join("gone");
    let os_error = std::fs::File::open(&missing).unwrap_err();
    let (out, merged) = t.merge(["o\n", "a\n", "b\n"], |o, a, _| {
        vec![s(o), s(a), s(&missing), "7".to_owned(), REPO_PAGE.to_owned()]
    });
    assert_eq!(
        result(&out),
        (
            Some(129),
            String::new(),
            format!(
                "ew merge-driver: {} cannot be read: {os_error}; {REPO_PAGE} is left as it was\n",
                missing.display()
            )
        )
    );
    assert_eq!(merged, "a\n");
}

/// The arguments are `%O %A %B %L %P` or `%O %A %B %P`: three are a usage error naming `<P>`, as the usage line shows
/// it, that exits 129 (git reads 2 as conflicts written; ADR-0012 amendment 2), and nothing is
/// written.
#[test]
fn three_arguments_are_a_usage_error() {
    let t = Tree::new();
    let (out, merged) = t.merge(["o\n", "a\n", "b\n"], |o, a, b| vec![s(o), s(a), s(b)]);
    assert_eq!(
        result(&out),
        (
            Some(129),
            String::new(),
            "error: the following required arguments were not provided:\n  <P>\n\nUsage: ew merge-driver <O> <A> <B> [L] <P>\n\nFor more information, try '--help'.\n".to_owned()
        )
    );
    assert_eq!(merged, "a\n");
}

/// Decision log ID-1273: a `%P` under a top-level folder whose name begins with `-` is a path, not an option: the
/// project in `-proj/` merges as in `proj/`.
#[test]
fn a_path_beginning_with_a_hyphen_merges() {
    let t = Tree::new();
    std::fs::rename(t.0.join("proj"), t.0.join("-proj")).unwrap();
    let (base, ours, theirs) = clean_sides();
    let expected = merged_sides();
    let path = "-proj/J-E-2416.ew/pages/4aa43308-d0d3-57ee-be0c-d9383f429f50.jsonl";
    let (out, merged) = t.merge([&base, &ours, &theirs], form(true, path));
    assert_eq!(result(&out), (Some(0), String::new(), String::new()));
    assert_eq!(merged, expected);
}
