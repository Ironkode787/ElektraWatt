// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew project assets purge`.

use super::*;

/// The word a person types to confirm `ew project assets purge`.
pub(super) const PURGE_WORD: &str = "purge";

/// `ew project assets purge`: the asset purge of doc 05 section 13.3 (`project.assets.purge`), a `HumanOnly`
/// command (owner ruling, 2026-10-23): the preview's list, then, after the word typed on a controlling terminal, the
/// purge of exactly the listed files. `--preview` needs no terminal; without one the purge itself exits 2 before the
/// project is opened.
pub(super) fn run_assets_purge(a: &AssetsPurgeArgs) -> i32 {
    use std::io::IsTerminal;
    let terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    run_assets_purge_with(
        a,
        terminal,
        &mut input,
        &mut output,
        ew_api::StoreOptions::new(env!("CARGO_PKG_VERSION")),
    )
}

/// [`run_assets_purge`] with the terminal test, the reader, the writer and the store options injected (unit tests).
pub(super) fn run_assets_purge_with(
    a: &AssetsPurgeArgs,
    terminal: bool,
    input: &mut dyn std::io::BufRead,
    output: &mut dyn std::io::Write,
    store: ew_api::StoreOptions,
) -> i32 {
    if !a.preview && !terminal {
        eprintln!(
            "error: HumanOnly: needs an interactive terminal (`project assets purge` is confirmed by typing \
             `{PURGE_WORD}`; `--preview` lists the files without one); nothing was removed"
        );
        return 2;
    }
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    // The preview is a reader: no lock, no roll-forward, no journal, nothing written, so it runs beside an open
    // editor. The purge opens for writing with the confirmed origin, used only after the word is typed.
    let opened = if a.preview {
        ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "project assets purge")
            .map(|(s, _)| s)
            .map_err(|e| {
                eprintln!("error: {}: {e}", dir.display());
                2
            })
    } else {
        let origin = ew_api::Origin::Cli {
            subcommand: "project assets purge".to_owned(),
            confirmed: true,
        };
        open_for_writing_with(&dir, origin, store)
    };
    let mut session = match opened {
        Ok(s) => s,
        Err(code) => return code,
    };
    let refused = |e: &ew_api::PurgeError| match e {
        ew_api::PurgeError::Io { .. } | ew_api::PurgeError::Save { .. } | ew_api::PurgeError::HumanOnly => 2,
        _ => 1,
    };
    let plan = match session.purge_preview() {
        Ok(plan) => plan,
        Err(e) => {
            eprintln!("error: {e}; nothing was removed");
            session.close();
            return refused(&e);
        }
    };
    let listing: String = plan
        .remove
        .iter()
        .map(|f| format!("remove {} ({} bytes)\n", f.path.as_str(), f.len))
        .collect();
    let verb = if a.preview { "would remove" } else { "removes" };
    let _ = output.write_all(listing.as_bytes());
    let _ = output.flush();
    eprintln!(
        "purge: {verb} {} asset file(s), {} bytes; {} kept (named)",
        plan.remove.len(),
        plan.bytes(),
        plan.kept
    );
    if a.preview || plan.remove.is_empty() {
        session.close();
        return 0;
    }
    let prompt = format!(
        "Deleted asset files do not come back by undo. Type `{PURGE_WORD}` to delete the {} file(s) above, anything \
         else to abort: ",
        plan.remove.len()
    );
    if !typed_word(input, output, &prompt, PURGE_WORD) {
        eprintln!("error: not confirmed; nothing was removed");
        session.close();
        return 1;
    }
    let code = match session.purge_assets(Some(&plan.hashes())) {
        Ok((done, report)) => {
            warn_unreleased(report.unstable, &mut std::io::stderr());
            let _ = writeln!(
                output,
                "removed {} asset file(s); generation {}",
                done.remove.len(),
                report.generation
            );
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            refused(&e)
        }
    };
    session.close();
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assets_purge_parses_its_directory_and_preview_flag() {
        let parse = |a: &[&str]| parse(a.iter().map(|s| (*s).to_owned())).map_err(|e| e.render().to_string());
        assert_eq!(
            parse(&["project", "assets", "purge"]),
            Ok(Command::AssetsPurge(AssetsPurgeArgs {
                project: None,
                preview: false,
            }))
        );
        assert_eq!(
            parse(&["project", "assets", "purge", "p.ew", "--preview"]),
            Ok(Command::AssetsPurge(AssetsPurgeArgs {
                project: Some(PathBuf::from("p.ew")),
                preview: true,
            }))
        );
        assert!(parse(&["project", "assets", "purge", "a", "b"]).is_err());
        assert!(parse(&["project", "assets", "purge", "--yes"]).is_err());
        assert!(parse(&["project", "assets"]).is_err());
    }

    /// A saved project in a fresh directory whose first logo's asset file nothing names any more (the logo was
    /// replaced and saved, and the session that did it is closed).
    fn project_with_an_unnamed_asset(tag: &str) -> PathBuf {
        let dir = ew_testdir::fresh_dir(&format!("ew-cli-purge-{tag}"));
        let project = serde_json::from_str(
            r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
        )
        .unwrap();
        let mut session =
            ew_api::Session::from_records(project, [], [], ew_api::SessionOptions::default()).unwrap();
        session.create(&dir, store()).unwrap();
        for logo in [
            &br#"<svg viewBox="0 0 8 2"><rect width="8" height="2"/></svg>"#[..],
            &br#"<svg viewBox="0 0 3 3"><circle r="1"/></svg>"#[..],
        ] {
            let cmd = ew_api::AnyCommand {
                name: "project.company.set".into(),
                params: serde_json::json!({ "logo": ew_api::base64_encode(logo) }),
            };
            let p = session.preview(&cmd).unwrap();
            session.commit(p).unwrap();
            session.save().unwrap();
        }
        session.close();
        dir
    }

    fn store() -> ew_api::StoreOptions {
        let mut o = ew_api::StoreOptions::new("test");
        o.state_dir = None;
        o
    }

    fn assets(dir: &Path) -> usize {
        std::fs::read_dir(dir.join("assets")).unwrap().count()
    }

    /// Runs the subcommand on `dir` with `terminal` and the typed `answer`; the exit code and what it printed.
    fn purge(dir: &Path, preview: bool, terminal: bool, answer: &str) -> (i32, String) {
        let args = AssetsPurgeArgs {
            project: Some(dir.to_path_buf()),
            preview,
        };
        let mut input = std::io::Cursor::new(answer.as_bytes().to_vec());
        let mut output = Vec::new();
        let code = run_assets_purge_with(&args, terminal, &mut input, &mut output, store());
        (code, String::from_utf8(output).unwrap())
    }

    /// Owner ruling (2026-10-23): the purge is human-only. Each path: the preview (no terminal needed, nothing
    /// changes), off a terminal (exit 2, nothing deleted), a wrong word (exit 1, nothing deleted), the typed word
    /// (exit 0, the listed file deleted after the list was shown).
    #[test]
    fn the_purge_needs_the_typed_word_on_a_terminal() {
        let dir = project_with_an_unnamed_asset("typed");
        assert_eq!(assets(&dir), 2);

        let (code, shown) = purge(&dir, true, false, "");
        assert_eq!(code, 0);
        assert!(
            shown.starts_with("remove assets/") && shown.lines().count() == 1,
            "{shown}"
        );
        assert_eq!(assets(&dir), 2, "the preview changes nothing");
        // The preview is a reader: it runs while an editor holds the project's lock, and leaves the lock alone.
        let (editor, _) = ew_api::Session::open(
            &dir,
            store(),
            ew_api::SessionOptions::default(),
            ew_api::Recovery::Keep,
        )
        .unwrap();
        let lock = std::fs::read(dir.join(".~ew.lock")).unwrap();
        let (code, shown) = purge(&dir, true, false, "");
        assert_eq!(code, 0, "{shown}");
        assert!(shown.starts_with("remove assets/"), "{shown}");
        assert_eq!(
            std::fs::read(dir.join(".~ew.lock")).unwrap(),
            lock,
            "the editor's lock is untouched"
        );
        editor.close();

        let (code, shown) = purge(&dir, false, false, "purge\n");
        assert_eq!(code, 2, "off a terminal");
        assert!(shown.is_empty(), "{shown}");
        assert_eq!(assets(&dir), 2);

        for wrong in ["", "\n", "y\n", "yes\n", "Purge\n", "purge it\n"] {
            let (code, shown) = purge(&dir, false, true, wrong);
            assert_eq!(code, 1, "{wrong:?} must abort");
            assert!(
                shown.contains("remove assets/"),
                "the list comes before the prompt: {shown}"
            );
            assert!(
                shown.contains("Type `purge` to delete the 1 file(s) above"),
                "{shown}"
            );
            assert_eq!(assets(&dir), 2, "{wrong:?}: nothing is deleted");
        }

        let (code, shown) = purge(&dir, false, true, "  purge \r\n");
        assert_eq!(code, 0, "{shown}");
        assert!(shown.contains("removed 1 asset file(s)"), "{shown}");
        assert_eq!(assets(&dir), 1);
        // Nothing left: no prompt.
        let (code, shown) = purge(&dir, false, true, "");
        assert_eq!(code, 0);
        assert!(!shown.contains("Type `purge`"), "{shown}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
