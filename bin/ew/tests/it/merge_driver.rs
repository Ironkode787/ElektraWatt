// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Through the `ew` binary: `ew merge-driver` in a working tree that holds a copy of the sample, as git runs it. The
//! exits 0 and 1, the four-argument form, an internal error's 129 with `%A` byte-identical; AC-41's driver half on
//! the wire of page 103 given an override (the shield of -103W1, 2.5 mm²) and the manifest merge.

use std::ffi::OsStr;
use std::path::Path;
use std::process::Output;

use crate::support::{EwTmp as _, SAMPLE, Tmp, page_file};

/// The shield wire's overrides in the common ancestor of these merges (the sample's shield has none).
pub(crate) const SHIELD_OVERRIDES: &str = r#""overrides":{"csa":"2.5"}"#;

/// Page 103 in the project.
pub(crate) fn page() -> String {
    page_file(Path::new(SAMPLE), 103)
}

/// Page 103 in the working tree (the project in `proj/`).
fn repo_page() -> String {
    format!("proj/sample.ew/{}", page())
}

/// The sample's page-103 file and its shield wire's line.
pub(crate) fn shield_line() -> (String, String) {
    let base = std::fs::read_to_string(Path::new(SAMPLE).join(page())).unwrap();
    let id = format!(r#""id":"{}""#, shield_id());
    let line = base
        .lines()
        .find(|l| l.contains(&id))
        .expect("the shield wire")
        .to_owned();
    (base, line)
}

/// The shield wire's ID, found by its ends: the terminal -X1031:SH and the motor -103M1.
pub(crate) fn shield_id() -> String {
    static ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ID.get_or_init(|| {
        let t = Tmp::new("ew-shield-id");
        let query = "SELECT id FROM api_v1_wire WHERE (from_tag = '-X1031:SH' AND to_tag = '-103M1') \
                     OR (from_tag = '-103M1' AND to_tag = '-X1031:SH')";
        let o = t.ew(&[OsStr::new("sql"), OsStr::new(SAMPLE), OsStr::new(query)]);
        let out = String::from_utf8_lossy(&o.stdout).into_owned();
        match out.lines().collect::<Vec<_>>()[..] {
            ["id", id] => id.to_owned(),
            _ => panic!("the shield wire: {out}"),
        }
    })
    .clone()
}

/// A working tree with a copy of the sample in `proj/sample.ew`, removed on drop.
struct Tree(Tmp);

impl Tree {
    fn new() -> Tree {
        let root = Tmp::new("ew-merge-driver");
        crate::support::copy_dir(Path::new(SAMPLE), &root.path("proj/sample.ew"));
        Tree(root)
    }

    /// Writes the three sides as git's temporary files and runs `ew merge-driver` with `args_for`'s arguments.
    fn merge(
        &self,
        sides: [&str; 3],
        args_for: impl Fn(&Path, &Path, &Path) -> Vec<String>,
    ) -> (Output, String) {
        let [o, a, b] = ["o", "a", "b"].map(|n| self.0.path(&format!(".merge_file_{n}")));
        for (path, text) in [(&o, sides[0]), (&a, sides[1]), (&b, sides[2])] {
            std::fs::write(path, text).unwrap();
        }
        let out = self.run(&args_for(&o, &a, &b));
        let merged = String::from_utf8(std::fs::read(&a).unwrap()).unwrap();
        (out, merged)
    }

    fn run(&self, args: &[String]) -> Output {
        self.0.ew_command(&["merge-driver"]).args(args).output().unwrap()
    }
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// The canonical five-argument form `%O %A %B %L %P` (`five`), else the older `%O %A %B %P`.
fn form(five: bool, path: &str) -> impl Fn(&Path, &Path, &Path) -> Vec<String> {
    let path = path.to_owned();
    move |o, a, b| {
        let mut args = vec![s(o), s(a), s(b)];
        if five {
            args.push("7".to_owned());
        }
        args.push(path.clone());
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

/// The sample's page-103 file with the shield wire given `overrides` (as part 4a's `ac41_w20_*` tests): its last
/// member, where the canonical writer puts it.
pub(crate) fn page_with(overrides: &str) -> String {
    let (base, line) = shield_line();
    let open = line.strip_suffix('}').expect("a JSON line");
    base.replace(&line, &format!("{open},{overrides}}}"))
}

/// The shield wire's page of the common ancestor, of branch A (adds `colour` BU) and of branch B (sets `csa` "4").
fn clean_sides() -> (String, String, String) {
    (
        page_with(SHIELD_OVERRIDES),
        page_with(r#""overrides":{"csa":"2.5","colour":["BU"]}"#),
        page_with(r#""overrides":{"csa":"4"}"#),
    )
}

/// The page the two branches of [`clean_sides`] merge to.
fn merged_sides() -> String {
    page_with(r#""overrides":{"csa":"4","colour":["BU"]}"#)
}

/// MD-5, MD-6: branch A adds the shield wire's `colour` BU, branch B sets `csa` "4": exit 0 and the merged page, in
/// both forms.
#[test]
fn a_wire_merges_cleanly_to_csa_4_and_colour_bu_and_exits_0() {
    let t = Tree::new();
    let (base, ours, theirs) = clean_sides();
    let expected = merged_sides();
    for five in [true, false] {
        let (out, merged) = t.merge([&base, &ours, &theirs], form(five, &repo_page()));
        assert_eq!(result(&out), (Some(0), String::new(), String::new()));
        assert_eq!(merged, expected);
    }
    // No temporary file is left beside the sides.
    let names: Vec<String> = std::fs::read_dir(t.0.dir())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.iter().all(|n| !n.contains(".tmp")), "{names:?}");
}

/// The conflict record on the shield wire's colour, ours BU and theirs WH, in D14's member order: the merge driver's
/// result of the conflict variant.
pub(crate) fn colour_conflict() -> String {
    format!(
        r#"{{"$k":"conflict","id":"{}","kind":"connection","base":{{"overrides":{{}}}},"ours":{{"overrides":{{"colour":["BU"]}}}},"theirs":{{"overrides":{{"colour":["WH"]}}}}}}"#,
        shield_id()
    )
}

/// MD-5, MD-6: the conflict variant (B also adds `colour` WH) writes one conflict record on `overrides.colour`, keeps
/// BU in place and exits 1, in both forms.
#[test]
fn the_conflict_variant_writes_one_conflict_record_and_exits_1() {
    let t = Tree::new();
    let (base, ours, _) = clean_sides();
    let theirs = page_with(r#""overrides":{"csa":"4","colour":["WH"]}"#);
    let kept = merged_sides();
    let conflict = colour_conflict();
    for five in [true, false] {
        let (out, merged) = t.merge([&base, &ours, &theirs], form(five, &repo_page()));
        assert_eq!(result(&out), (Some(1), String::new(), String::new()));
        assert_eq!(merged, format!("{kept}{conflict}\n"));
    }
}

/// MD-6: the manifest merge gives the larger generation plus one, `merged: true` and no `base`; the page both
/// branches changed is pending, a file one branch changed keeps that branch's hash.
#[test]
fn the_manifest_merges_to_max_plus_one_merged_and_no_base() {
    let t = Tree::new();
    // The manifest as ours and theirs saw it: generation 1 and no `base` (the sample's own generation is that of its
    // script's saves).
    let committed = std::fs::read_to_string(Path::new(SAMPLE).join("manifest.json")).unwrap();
    let generation = committed
        .lines()
        .find(|l| l.contains("\"generation\": "))
        .unwrap()
        .to_owned();
    let mut base = committed.replace(&generation, "  \"generation\": 1,");
    let end = "\n  }\n}\n";
    if let Some(i) = base.find("\n  },\n  \"base\": ") {
        base.replace_range(i.., end);
    }
    // A page's hash and a device file's hash as the committed manifest holds them (they change whenever the
    // sample is regenerated).
    let hash_of = |prefix: &str| {
        base.lines()
            .find(|l| l.trim_start().starts_with(prefix))
            .and_then(|l| l.split('"').nth(3))
            .expect("the file in the manifest")
            .to_owned()
    };
    let page_hash = hash_of(&format!("\"{}\"", page()));
    let device_hash = hash_of("\"devices/");
    let (page_hash, device_hash) = (page_hash.as_str(), device_hash.as_str());
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
        form(true, "proj/sample.ew/manifest.json"),
    );
    assert_eq!(result(&out), (Some(0), String::new(), String::new()));
    assert_eq!(merged, expected);
}

/// Theirs' manifest names a format newer than this reader's: nothing is merged, `%A`
/// stays ours byte for byte, the driver says so and exits 1. Ours' newer format (the working tree's manifest) keeps a
/// page of that project as ours the same way.
#[test]
fn a_side_newer_than_the_reader_is_left_as_ours() {
    let t = Tree::new();
    let base = std::fs::read_to_string(Path::new(SAMPLE).join("manifest.json")).unwrap();
    let format = format!("\"format\": \"{}\"", ew_api::FORMAT);
    assert!(base.contains(&format));
    let generation = base
        .lines()
        .find(|l| l.contains("\"generation\": "))
        .unwrap()
        .to_owned();
    let ours = base.replace(&generation, "  \"generation\": 90,");
    let theirs = base
        .replace(&generation, "  \"generation\": 91,")
        .replace(&format, "\"format\": \"9.0\"");
    let manifest = "proj/sample.ew/manifest.json";
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

    std::fs::write(t.0.path(manifest), &theirs).unwrap();
    let (base, ours, theirs) = clean_sides();
    let (out, merged) = t.merge([&base, &ours, &theirs], form(false, &repo_page()));
    assert_eq!(result(&out), (Some(1), String::new(), said(&repo_page())));
    assert_eq!(merged, ours);
}

/// Theirs holds the shield wire one `$v` above this reader's version of a connection (1): nothing is merged in the
/// page, `%A` stays ours byte for byte, the driver says so and exits 1. Theirs holding a member this reader does not
/// know merges cleanly, the member kept.
#[test]
fn a_record_this_reader_cannot_read_leaves_the_file_as_ours() {
    let t = Tree::new();
    let id = format!(r#""id":"{}""#, shield_id());
    let shield = |page: String, change: &dyn Fn(&str) -> String| -> String {
        let line = page
            .lines()
            .find(|l| l.contains(&id))
            .expect("the shield wire")
            .to_owned();
        page.replace(&line, &change(&line))
    };
    let (base, ours, theirs) = clean_sides();
    let newer = shield(theirs.clone(), &|l| l.replacen(&id, &format!("{id},\"$v\":2"), 1));
    assert_ne!(newer, theirs);
    let repo_page = repo_page();
    let (out, merged) = t.merge([&base, &ours, &newer], form(true, &repo_page));
    let said = format!(
        "merge-driver: {repo_page}: holds records this ElektraWatt cannot read (this one reads format {}); left as \
         ours: merge it with a newer version\n",
        ew_api::FORMAT
    );
    assert_eq!(result(&out), (Some(1), String::new(), said));
    assert_eq!(merged, ours);

    let later = |l: &str| format!("{},\"later\":\"kept\"}}", l.strip_suffix('}').unwrap());
    let expected = shield(merged_sides(), &later);
    let later = shield(theirs, &later);
    let (out, merged) = t.merge([&base, &ours, &later], form(true, &repo_page));
    assert_eq!(result(&out), (Some(0), String::new(), String::new()));
    assert_eq!(merged, expected);
}

/// A library document whose theirs is one `$v` above this reader's version of a
/// manufacturer (1) is not merged either: `%A` stays ours byte for byte, the driver prints S3b and exits 1.
#[test]
fn a_library_document_this_reader_cannot_read_is_left_as_ours() {
    let t = Tree::new();
    // The sample's one manufacturer, Generic.
    let file = std::fs::read_dir(t.0.path("proj/sample.ew/lib/manufacturers"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .file_name();
    let doc = format!("proj/sample.ew/lib/manufacturers/{}", file.to_string_lossy());
    let doc = doc.as_str();
    let base = std::fs::read_to_string(t.0.path(doc)).unwrap();
    let ours = base.replace(
        r#""name": {"en": "Generic"}"#,
        r#""name": {"en": "Generic parts"}"#,
    );
    let theirs = base.replace(r#""$k": "manufacturer","#, r#""$k": "manufacturer", "$v": 2,"#);
    assert!(ours != base && theirs != base);
    let (out, merged) = t.merge([&base, &ours, &theirs], form(true, doc));
    let said = format!(
        "merge-driver: {doc}: holds records this ElektraWatt cannot read (this one reads format {}); left as ours: \
         merge it with a newer version\n",
        ew_api::FORMAT
    );
    assert_eq!(result(&out), (Some(1), String::new(), said));
    assert_eq!(merged, ours);
}

/// A write that fails (`%A`'s name is so long that no temporary file beside it can be named) exits 129 and leaves
/// `%A` byte-identical.
#[test]
fn a_failed_write_exits_129_and_leaves_a_byte_identical() {
    let t = Tree::new();
    let (base, ours, theirs) = clean_sides();
    // 250 characters: a file's name, but not with a temporary file's suffix (a name holds at most 255).
    let name = format!(".merge_file_{}", "a".repeat(238));
    let [o, a, b] = [".merge_file_o", name.as_str(), ".merge_file_b"].map(|n| t.0.path(n));
    for (path, text) in [(&o, &base), (&a, &ours), (&b, &theirs)] {
        std::fs::write(path, text).unwrap();
    }
    let os_error = std::fs::File::create(t.0.path(&format!("{name}.tmp-0"))).unwrap_err();
    let repo_page = repo_page();
    let out = t.run(&form(true, &repo_page)(&o, &a, &b));
    assert_eq!(
        result(&out),
        (
            Some(129),
            String::new(),
            format!(
                "ew merge-driver: the merged {repo_page} cannot be written to {}: {os_error}; it is left as it was\n",
                a.display()
            )
        )
    );
    assert_eq!(std::fs::read_to_string(&a).unwrap(), ours);
}

/// A side that cannot be read exits 129 and leaves `%A` byte-identical.
#[test]
fn a_side_that_cannot_be_read_exits_129() {
    let t = Tree::new();
    let missing = t.0.path("gone");
    let os_error = std::fs::File::open(&missing).unwrap_err();
    let repo_page = repo_page();
    let (out, merged) = t.merge(["o\n", "a\n", "b\n"], |o, a, _| {
        vec![s(o), s(a), s(&missing), "7".to_owned(), repo_page.clone()]
    });
    assert_eq!(
        result(&out),
        (
            Some(129),
            String::new(),
            format!(
                "ew merge-driver: {} cannot be read: {os_error}; {repo_page} is left as it was\n",
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

/// A `%P` under a top-level folder whose name begins with `-` is a path, not an option: the
/// project in `-proj/` merges as in `proj/`.
#[test]
fn a_path_beginning_with_a_hyphen_merges() {
    let t = Tree::new();
    std::fs::rename(t.0.path("proj"), t.0.path("-proj")).unwrap();
    let (base, ours, theirs) = clean_sides();
    let expected = merged_sides();
    let path = format!("-{}", repo_page());
    let (out, merged) = t.merge([&base, &ours, &theirs], form(true, &path));
    assert_eq!(result(&out), (Some(0), String::new(), String::new()));
    assert_eq!(merged, expected);
}
