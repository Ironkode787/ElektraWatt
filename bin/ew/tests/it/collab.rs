// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.17 part 8 through the `ew` binary (D26 to D29, decision log ID-1284): `ew diff` (DG-1), its textconv in `git
//! log -p` (DG-2), `ew git setup` (DG-3, DG-4), `ew check --integrity` (DG-6) and AC-41 through the real git (DG-7).
//!
//! Git runs only in fresh temporary repositories (ruling 8): `-C <dir>`, every repository-selecting variable of
//! `ew_api::git::ISOLATE` removed, the global configuration a file of the test's own folder and the system one off, a
//! user set; a test skips with a message when git is absent. Golden A is only read and copied.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
/// Page 102.
const PAGE: &str = "pages/4aa43308-d0d3-57ee-be0c-d9383f429f50.jsonl";

/// A record's ID from Golden A by its source key (`connections|W20`, `devices|-102F2`).
fn golden_id(key: &str) -> String {
    let needle = format!("\"key\":\"{key}\"");
    for dir in ["pages", "devices"] {
        for e in std::fs::read_dir(Path::new(GOLDEN_A).join(dir)).unwrap() {
            let text = std::fs::read_to_string(e.unwrap().path()).unwrap();
            if let Some(line) = text.lines().find(|l| l.contains(&needle)) {
                let v: Value = serde_json::from_str(line).unwrap();
                return v["id"].as_str().unwrap().to_owned();
            }
        }
    }
    panic!("{key} is not in Golden A");
}

fn set(kind: &str, id: &str, path: &str, value: Value) -> ew_api::AnyCommand {
    ew_api::AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": kind, "targets": [id], "path": path, "value": value}),
    }
}

/// Runs `cmds` on the project `dir` in a session of this process and saves (an edit as the window makes it).
fn edit(dir: &Path, cmds: &[ew_api::AnyCommand]) {
    let mut store = ew_api::StoreOptions::new("ElektraWatt test");
    store.state_dir = None;
    let (mut s, _) = ew_api::Session::open(
        dir,
        store,
        ew_api::SessionOptions::default(),
        ew_api::Recovery::Keep,
    )
    .expect("the copy opens for editing");
    for c in cmds {
        s.execute(c).unwrap_or_else(|e| panic!("{c:?}: {e}"));
    }
    s.save().expect("the edit saves");
    s.close();
}

/// A fresh folder with a git repository `repo` holding a copy of Golden A as `J-E-2416.ew`, committed; its own global
/// git configuration. Removed on drop.
struct Repo(PathBuf);

impl Repo {
    /// `None` when git is absent (the test then skips).
    fn new(name: &str) -> Option<Repo> {
        let r = Repo(ew_testdir::fresh_dir(name));
        std::fs::create_dir_all(r.repo()).unwrap();
        std::fs::write(r.0.join("gitconfig"), "").unwrap();
        if !r.git(&["--version"]).status.success() {
            eprintln!("{name} skipped: no git executable on the path");
            return None;
        }
        for args in [
            &["init", "--quiet", "--initial-branch=main"][..],
            &["config", "user.name", "ElektraWatt test"],
            &["config", "user.email", "test@example.invalid"],
            &["config", "commit.gpgsign", "false"],
            &["config", "core.autocrlf", "false"],
        ] {
            assert!(r.git(args).status.success(), "git {args:?}");
        }
        super::conventions::copy_dir(Path::new(GOLDEN_A), &r.project());
        r.commit("Golden A");
        Some(r)
    }

    fn repo(&self) -> PathBuf {
        self.0.join("repo")
    }

    fn project(&self) -> PathBuf {
        self.repo().join("J-E-2416.ew")
    }

    /// The test's own environment for git and `ew`: no repository-selecting variable, the global configuration a file
    /// of this folder, no system configuration, no AppImage.
    fn env(&self, cmd: &mut Command) {
        for var in ew_api::git::ISOLATE {
            cmd.env_remove(var);
        }
        cmd.env_remove("APPIMAGE")
            .env_remove("APPDIR")
            .env("GIT_CONFIG_GLOBAL", self.0.join("gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .stdin(Stdio::null());
    }

    fn git(&self, args: &[&str]) -> Output {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(self.repo()).args(args);
        self.env(&mut cmd);
        cmd.output().unwrap()
    }

    fn ew<S: AsRef<OsStr>>(&self, args: &[S]) -> Output {
        let mut cmd = Command::new(super::conventions::bin_exe());
        cmd.args(args).current_dir(self.repo());
        self.env(&mut cmd);
        cmd.output().unwrap()
    }

    fn commit(&self, message: &str) {
        assert!(self.git(&["add", "--all"]).status.success());
        let o = self.git(&["commit", "--quiet", "-m", message]);
        assert!(
            o.status.success(),
            "commit: {}",
            String::from_utf8_lossy(&o.stderr)
        );
    }

    fn config(&self, key: &str) -> Option<String> {
        let o = self.git(&["config", "--get", key]);
        o.status
            .success()
            .then(|| String::from_utf8_lossy(&o.stdout).trim_end().to_owned())
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn result(o: &Output) -> (Option<i32>, String, String) {
    (
        o.status.code(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

/// The line `ew diff` prints for -101F1's part changed from Golden A's A9P52610 to 552286.
const PART_LINE: &str = "-101F1: parts [{\"part\":{\"copy\":\"A9P52610\"},\"qty\":\"1\"}] → [{\"part\":{\"copy\":\"552286\"},\"qty\":\"1\"}]\n";

/// -101F1 with the part copy of 552286 instead of A9P52610.
fn change_101f1(dir: &Path) {
    let device = golden_id("devices|-101F1");
    let parts = json!([{"part": {"copy": "07a6c371-1881-5acc-9ac3-0d773f0ddc16"}, "qty": "1"}]);
    edit(dir, &[set("device", &device, "parts", parts)]);
}

/// DG-1: `ew diff` between Golden A and a copy with -101F1's part changed prints that line (exit 0, and 1 with
/// `--exit-code`); the same between a directory and its `.ewz`, which are equal; and between `git:HEAD` and the working
/// copy in a temporary repository; `--json` validates against its result schema.
#[test]
fn dg1_ew_diff_prints_the_changed_part_with_the_tags_of_each_side() {
    let Some(r) = Repo::new("ew-dg1") else { return };
    let copy = r.0.join("copy.ew");
    super::conventions::copy_dir(Path::new(GOLDEN_A), &copy);
    change_101f1(&copy);
    let golden = Path::new(GOLDEN_A);
    let plain = r.ew(&[OsStr::new("diff"), golden.as_os_str(), copy.as_os_str()]);
    assert_eq!(result(&plain), (Some(0), PART_LINE.to_owned(), String::new()));
    let exit = r.ew(&[
        OsStr::new("diff"),
        golden.as_os_str(),
        copy.as_os_str(),
        OsStr::new("--exit-code"),
    ]);
    assert_eq!(result(&exit), (Some(1), PART_LINE.to_owned(), String::new()));
    let json = r.ew(&[
        OsStr::new("diff"),
        golden.as_os_str(),
        copy.as_os_str(),
        OsStr::new("--json"),
    ]);
    let typed: ew_api::cli_results::DiffResult =
        super::conventions::assert_result(&String::from_utf8_lossy(&json.stdout), ew_api::cli_results::DIFF);
    assert!(!typed.equal);
    assert_eq!(typed.changes.len(), 1);
    assert_eq!(typed.changes[0].label, "-101F1");

    // A directory and its `.ewz` hold the same records.
    let ewz = r.0.join("copy.ewz");
    let packed = r.ew(&[
        OsStr::new("export"),
        OsStr::new("ewz"),
        copy.as_os_str(),
        OsStr::new("--out"),
        ewz.as_os_str(),
    ]);
    assert_eq!(
        packed.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&packed.stderr)
    );
    let same = r.ew(&[
        OsStr::new("diff"),
        copy.as_os_str(),
        ewz.as_os_str(),
        OsStr::new("--exit-code"),
    ]);
    assert_eq!(result(&same), (Some(0), String::new(), String::new()));

    // `git:HEAD` against the working copy.
    change_101f1(&r.project());
    let head = r.ew(&["diff", "git:HEAD", "J-E-2416.ew", "--exit-code"]);
    assert_eq!(result(&head), (Some(1), PART_LINE.to_owned(), String::new()));
    let missing = r.ew(&["diff", "git:no-such-revision", "J-E-2416.ew"]);
    assert_eq!(
        result(&missing),
        (
            Some(2),
            String::new(),
            "error: git:no-such-revision: the revision holds no no-such-revision:J-E-2416.ew/manifest.json\n"
                .to_owned()
        )
    );
}

/// The `ew` under test as git's shell reads it.
fn quoted_exe() -> String {
    let exe = super::conventions::bin_exe().to_string_lossy().replace('\\', "/");
    format!("'{}'", exe.replace('\'', "'\\''"))
}

/// The four keys `ew git setup` writes for the `ew` under test.
fn expected_config() -> Vec<(&'static str, String)> {
    let exe = quoted_exe();
    vec![
        ("merge.elektrawatt.name", "ElektraWatt record merge".to_owned()),
        (
            "merge.elektrawatt.driver",
            format!("{exe} merge-driver %O %A %B %L %P"),
        ),
        ("diff.elektrawatt.textconv", format!("{exe} diff --textconv")),
        ("diff.elektrawatt.cachetextconv", "true".to_owned()),
    ]
}

/// The managed `.gitattributes` block of doc 05 §11.1 after a user's own line.
const ATTRIBUTES: &str = "*.png binary\n# >>> ElektraWatt (managed) >>>\n*.jsonl text eol=lf merge=elektrawatt \
diff=elektrawatt\nlib/**/*.json text eol=lf merge=elektrawatt diff=elektrawatt\nmanifest.json text eol=lf \
merge=elektrawatt\nrules/** text eol=lf\nreports/*.sql text eol=lf\nassets/* binary\n# <<< ElektraWatt (managed) <<<\n";

/// DG-3: without a terminal or `--yes`, `ew git setup` exits 2 and changes nothing; with `--yes` it writes the four
/// keys and the blocks, keeps a user's line outside the block, and is idempotent.
#[test]
fn dg3_git_setup_asks_writes_the_keys_and_the_blocks_and_is_idempotent() {
    let Some(r) = Repo::new("ew-dg3") else { return };
    std::fs::write(r.project().join(".gitattributes"), "*.png binary\n").unwrap();
    // C49-p9#6: a web app's manifest is no project folder.
    std::fs::create_dir_all(r.repo().join("web")).unwrap();
    std::fs::write(r.repo().join("web/manifest.json"), "{\"name\":\"app\"}\n").unwrap();
    let config_before = std::fs::read(r.repo().join(".git/config")).unwrap();
    let refused = r.ew(&["git", "setup"]);
    assert_eq!(
        result(&refused),
        (
            Some(2),
            String::new(),
            "error: `ew git setup` writes git's configuration only with your consent: run it on a terminal to be \
             asked, or pass --yes; nothing was changed\n"
                .to_owned()
        )
    );
    assert_eq!(
        std::fs::read(r.repo().join(".git/config")).unwrap(),
        config_before
    );
    assert_eq!(
        std::fs::read_to_string(r.project().join(".gitattributes")).unwrap(),
        "*.png binary\n"
    );
    assert!(!r.project().join(".gitignore").exists());

    let done = r.ew(&["git", "setup", "--yes"]);
    assert_eq!(
        result(&done),
        (
            Some(0),
            "the merge driver and the textconv are configured (4 keys)\nJ-E-2416.ew: .gitattributes and .gitignore \
             written\n"
                .to_owned(),
            String::new()
        )
    );
    for (key, value) in expected_config() {
        assert_eq!(r.config(key), Some(value), "{key}");
    }
    assert_eq!(
        std::fs::read_to_string(r.project().join(".gitattributes")).unwrap(),
        ATTRIBUTES
    );
    assert_eq!(
        std::fs::read_to_string(r.project().join(".gitignore")).unwrap(),
        "# >>> ElektraWatt (managed) >>>\n.ew/\n.~ew.lock\n.~ew.lock.stale-*\n.~ew.requests/\n*.tmp-*\n# <<< ElektraWatt \
         (managed) <<<\n"
    );
    let config = std::fs::read(r.repo().join(".git/config")).unwrap();
    let again = r.ew(&["git", "setup", "--yes"]);
    assert_eq!(
        result(&again),
        (
            Some(0),
            "the merge driver and the textconv are configured (4 keys)\nJ-E-2416.ew: the managed blocks are up to \
             date\n"
                .to_owned(),
            String::new()
        )
    );
    assert_eq!(
        std::fs::read(r.repo().join(".git/config")).unwrap(),
        config,
        "idempotent"
    );
    assert_eq!(
        std::fs::read_dir(r.repo().join("web")).unwrap().count(),
        1,
        "nothing written beside a web app's manifest"
    );
    assert_eq!(
        std::fs::read_to_string(r.project().join(".gitattributes")).unwrap(),
        ATTRIBUTES
    );
}

/// DG-4: `--hooks` installs the hook and refuses to replace a foreign one, writing nothing then.
#[test]
fn dg4_the_hook_is_installed_and_a_foreign_one_is_kept() {
    let Some(r) = Repo::new("ew-dg4") else { return };
    let hook = r.repo().join(".git/hooks/post-merge");
    let _ = std::fs::remove_file(&hook);
    let done = r.ew(&["git", "setup", "--yes", "--hooks"]);
    assert_eq!(
        done.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&done.stderr)
    );
    let exe = quoted_exe();
    let expected = format!(
        "#!/bin/sh\n# ElektraWatt (managed): the project-integrity check after a merge\nstatus=0\n{exe} check \
         --integrity --after-merge 'J-E-2416.ew' || status=1\nexit $status\n"
    );
    assert_eq!(std::fs::read_to_string(&hook).unwrap(), expected);
    std::fs::write(&hook, "#!/bin/sh\necho mine\n").unwrap();
    std::fs::remove_file(r.project().join(".gitignore")).unwrap();
    let refused = r.ew(&["git", "setup", "--yes", "--hooks"]);
    assert_eq!(
        result(&refused),
        (
            Some(2),
            String::new(),
            format!(
                "error: {} holds a post-merge hook that ElektraWatt did not write; it is kept (add `ew check \
                 --integrity --after-merge` to it by hand); nothing was changed\n",
                r.repo().join(".git").join("hooks").join("post-merge").display()
            )
        )
    );
    assert_eq!(std::fs::read_to_string(&hook).unwrap(), "#!/bin/sh\necho mine\n");
    assert!(!r.project().join(".gitignore").exists(), "nothing written");

    // C49-p9#9: `core.hooksPath` is where git runs hooks from, so the hook goes there.
    assert!(r.git(&["config", "core.hooksPath", "githooks"]).status.success());
    let done = r.ew(&["git", "setup", "--yes", "--hooks"]);
    assert_eq!(
        done.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&done.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(r.repo().join("githooks").join("post-merge")).unwrap(),
        expected
    );
    assert_eq!(std::fs::read_to_string(&hook).unwrap(), "#!/bin/sh\necho mine\n");

    // C49-p10b#5: on Windows a `core.hooksPath` inside the repository, written in another case, is inside it.
    if cfg!(windows) {
        let upper = r
            .repo()
            .join("casehooks")
            .to_string_lossy()
            .to_uppercase()
            .replace('\\', "/");
        assert!(r.git(&["config", "core.hooksPath", &upper]).status.success());
        let done = r.ew(&["git", "setup", "--yes", "--hooks"]);
        assert_eq!(
            done.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&done.stderr)
        );
        assert_eq!(
            std::fs::read_to_string(r.repo().join("casehooks").join("post-merge")).unwrap(),
            expected
        );
    }

    // C49-p10#4: a `core.hooksPath` outside the repository (as a global one is) serves other repositories too: refused.
    let shared = r.0.join("shared-hooks");
    let value = shared.to_string_lossy().replace('\\', "/");
    assert!(r.git(&["config", "core.hooksPath", &value]).status.success());
    let refused = r.ew(&["git", "setup", "--yes", "--hooks"]);
    assert_eq!(
        result(&refused),
        (
            Some(2),
            String::new(),
            format!(
                "error: git runs the hooks of this repository from {}, outside it (core.hooksPath), so other \
                 repositories run them too; run `ew git setup` without --hooks, and add `ew check --integrity \
                 --after-merge` to that post-merge hook by hand; nothing was changed\n",
                shared.display()
            )
        )
    );
    assert!(!shared.exists(), "nothing written");
}

/// C49-p9#5, C49-p10#2: run from an AppImage (`$APPIMAGE`, the executable under `$APPDIR`), `ew git setup` writes the
/// AppImage with `ew` as its first argument, never the executable's own path in the AppImage's transient mount; an
/// `ew` outside `$APPDIR` that inherited another AppImage's variables (an editor's terminal) writes its own path.
#[test]
fn git_setup_from_an_appimage_names_the_appimage() {
    let Some(r) = Repo::new("ew-dg3-appimage") else {
        return;
    };
    let image = r.0.join("Apps").join("Editor.AppImage");
    let exe = super::conventions::bin_exe();
    let quoted_image = format!("'{}'", image.to_string_lossy().replace('\\', "/"));
    let elsewhere = r.0.join("mount");
    for (appdir, command) in [
        (elsewhere.as_path(), quoted_exe()),
        (exe.parent().unwrap(), format!("{quoted_image} ew")),
    ] {
        let mut cmd = Command::new(&exe);
        cmd.args(["git", "setup", "--yes"]).current_dir(r.repo());
        r.env(&mut cmd);
        let done = cmd
            .env("APPIMAGE", &image)
            .env("APPDIR", appdir)
            .output()
            .unwrap();
        assert_eq!(
            done.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&done.stderr)
        );
        assert_eq!(
            r.config("merge.elektrawatt.driver"),
            Some(format!("{command} merge-driver %O %A %B %L %P"))
        );
        assert_eq!(
            r.config("diff.elektrawatt.textconv"),
            Some(format!("{command} diff --textconv"))
        );
    }
}

/// A copy of Golden A whose page 102 holds MD-6's conflict record on W20's colour, as part 5's CR-1 project.
fn cr1_project(dir: &Path) {
    const OVERRIDES: &str = r#""overrides":{"csa":"1.5","show_marker":true}"#;
    const CONFLICT: &str = r#"{"$k":"conflict","id":"1c5963b7-6c34-53d6-8fa6-9d02d4629711","kind":"connection","base":{"overrides":{}},"ours":{"overrides":{"colour":["BU"]}},"theirs":{"overrides":{"colour":["WH"]}}}"#;
    super::conventions::copy_dir(Path::new(GOLDEN_A), dir);
    let page = std::fs::read_to_string(dir.join(PAGE)).unwrap();
    let w20 = page
        .lines()
        .find(|l| l.contains("connections|W20"))
        .unwrap()
        .to_owned();
    let bu = w20.replace(
        OVERRIDES,
        r#""overrides":{"csa":"2.5","colour":["BU"],"show_marker":true}"#,
    );
    std::fs::write(dir.join(PAGE), format!("{}{CONFLICT}\n", page.replace(&w20, &bu))).unwrap();
}

/// DG-6: `ew check --integrity` exits 1 on CR-1's project for its conflict record and 0 on Golden A.
#[test]
fn dg6_the_integrity_check_fails_on_a_conflict_record_and_passes_golden_a() {
    let base = ew_testdir::fresh_dir("ew-dg6");
    let cr1 = base.join("cr1.ew");
    cr1_project(&cr1);
    let run = |dir: &Path| {
        Command::new(super::conventions::bin_exe())
            .args([OsStr::new("check"), OsStr::new("--integrity"), dir.as_os_str()])
            .current_dir(&base)
            .output()
            .unwrap()
    };
    let (code, out, err) = result(&run(&cr1));
    assert_eq!(code, Some(1), "{out}{err}");
    assert!(out.contains("EW-CORE-011"), "{out}");
    assert_eq!(err, "integrity: 1 errors, 0 warnings, 0 infos, 0 waived\n");
    let (code, out, err) = result(&run(Path::new(GOLDEN_A)));
    assert_eq!(
        (code, err.as_str()),
        (Some(0), "integrity: 0 errors, 0 warnings, 0 infos, 0 waived\n"),
        "{out}"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// The page-102 file of the project `dir`, as text.
fn page_102(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(PAGE)).unwrap()
}

/// W20's line of a page file.
fn w20_line(page: &str) -> Value {
    serde_json::from_str(page.lines().find(|l| l.contains("connections|W20")).unwrap()).unwrap()
}

/// DG-2 and DG-7 (AC-41): on a fresh repository holding a Golden A copy, `ew git setup --yes`, then the two branches of
/// D29: A sets -102F2's function text and adds W20's colour BU, B sets its cross-section 2.5. `git merge` exits 0
/// with csa 2.5 and BU, a merged manifest, and `ew check --integrity` exits 0; `git log -p` shows the textconv lines.
/// The conflict variant (B also adds WH) leaves one conflict record, the project opens, and `ew merge resolve --take
/// ours` makes it clean. Before setup, git's configuration has no driver (XW-7's status).
#[test]
fn dg7_ac41_merges_through_git_with_the_driver() {
    let Some(r) = Repo::new("ew-dg7") else { return };
    assert_eq!(
        r.config("merge.elektrawatt.driver"),
        None,
        "merge driver not configured"
    );
    assert_eq!(r.ew(&["git", "setup", "--yes"]).status.code(), Some(0));
    r.commit("ew git setup");
    let (w20, f2) = (golden_id("connections|W20"), golden_id("devices|-102F2"));
    let branch = |name: &str, from: &str, cmds: &[ew_api::AnyCommand]| {
        assert!(r.git(&["checkout", "--quiet", "-b", name, from]).status.success());
        edit(&r.project(), cmds);
        r.commit(name);
    };
    branch(
        "a",
        "main",
        &[
            set("device", &f2, "function_text", json!("Spare")),
            set("connection", &w20, "overrides.colour", json!(["BU"])),
        ],
    );
    branch(
        "b",
        "main",
        &[set("connection", &w20, "overrides.csa", json!("2.5"))],
    );
    branch(
        "b-wh",
        "main",
        &[
            set("connection", &w20, "overrides.csa", json!("2.5")),
            set("connection", &w20, "overrides.colour", json!(["WH"])),
        ],
    );

    // DG-2: the textconv lines of branch A's page change, labelled from the working copy.
    let log = r.git(&["log", "-p", "-1", "a", "--", &format!("J-E-2416.ew/{PAGE}")]);
    let log = String::from_utf8_lossy(&log.stdout).into_owned();
    assert!(r.git(&["checkout", "--quiet", "a"]).status.success());
    let (session, _) = ew_api::Session::open_read_only(&r.project(), "ElektraWatt test").unwrap();
    let (text, known) = ew_api::output::textconv(Some(&session), page_102(&r.project()).as_bytes());
    drop(session);
    let line = text
        .lines()
        .find(|l| l.contains("overrides.colour=[\"BU\"]"))
        .expect("W20's textconv line");
    assert!(known > 0 && !line.contains(&w20[..8]), "a derived label: {line}");
    assert!(
        log.lines().any(|l| l == format!("+{line}")),
        "git log -p shows {line}:\n{log}"
    );

    // DG-7: the clean merge.
    assert!(
        r.git(&["checkout", "--quiet", "-B", "merged", "a"])
            .status
            .success()
    );
    let merged = r.git(&["merge", "--no-edit", "b"]);
    assert_eq!(
        merged.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&merged.stderr)
    );
    let v = w20_line(&page_102(&r.project()));
    assert_eq!(
        v["overrides"],
        json!({"csa": "2.5", "colour": ["BU"], "show_marker": true})
    );
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(r.project().join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["merged"], json!(true));
    let integrity = r.ew(&["check", "--integrity", "J-E-2416.ew"]);
    assert_eq!(
        integrity.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&integrity.stdout)
    );

    // The conflict variant.
    assert!(
        r.git(&["checkout", "--quiet", "-B", "conflict", "a"])
            .status
            .success()
    );
    let conflicted = r.git(&["merge", "--no-edit", "b-wh"]);
    assert_eq!(
        conflicted.status.code(),
        Some(1),
        "git merge reports the conflict"
    );
    let page = page_102(&r.project());
    assert_eq!(
        page.matches("\"$k\":\"conflict\"").count(),
        1,
        "one conflict record"
    );
    assert_eq!(
        w20_line(&page)["overrides"]["colour"],
        json!(["BU"]),
        "ours in place"
    );
    let opened = r.ew(&["check", "--integrity", "J-E-2416.ew"]);
    assert_eq!(
        opened.status.code(),
        Some(1),
        "the project opens and reports the conflict"
    );
    let resolved = r.ew(&["merge", "resolve", "--take", "ours", "J-E-2416.ew"]);
    assert_eq!(
        result(&resolved),
        (
            Some(0),
            "1 conflict resolved, ours taken; saved\n".to_owned(),
            String::new()
        )
    );
    assert!(!page_102(&r.project()).contains("\"$k\":\"conflict\""));
    let clean = r.ew(&["check", "--integrity", "J-E-2416.ew"]);
    assert_eq!(
        clean.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&clean.stdout)
    );
}
