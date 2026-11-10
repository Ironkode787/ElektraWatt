// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew check`, `ew checks set` and `ew verify`.

use super::*;

/// `ew project checks set`: `project.checks.set` from a confirmed CLI subcommand, the human origin the command's
/// exposure class `HumanOnly` accepts (ADR-0008 amendment 3, FT-336), after the confirmation typed on a controlling
/// terminal (doc 17 section 2.3, FR-EXT-030); without a terminal nothing is opened.
pub(super) fn run_checks_set(a: &ChecksSetArgs) -> i32 {
    use std::io::IsTerminal;
    // 1. The read-only part: every named scheme ships (doc 15 section 4.9); else exit 1 without asking.
    for name in [&a.active, &a.release].into_iter().flatten() {
        if scheme(name).is_none() {
            let names: Vec<&str> = ew_api::checks::SCHEMES.iter().map(|s| s.name).collect();
            eprintln!(
                "error: `{name}` is not a shipped check scheme; the schemes are: {}; nothing was changed",
                names.join(", ")
            );
            return 1;
        }
    }
    // 2. Without a controlling terminal (CI, a pipe, a script) a HumanOnly command is never submitted (doc 17
    //    section 2.3, FR-EXT-030); the project is not opened, so nothing is locked or journalled.
    if !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) {
        eprintln!(
            "error: HumanOnly: needs an interactive terminal (`project checks set` is confirmed by typing the \
             scheme name); nothing was changed"
        );
        return 2;
    }
    // 3. The change summary, then the typed confirmation: the scheme's name, exactly. There is no `--yes` flag.
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    if !confirm_typed(a, &mut input, &mut output) {
        eprintln!("error: not confirmed; nothing was changed");
        return 1;
    }
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    let origin = ew_api::Origin::Cli {
        subcommand: "project checks set".to_owned(),
        confirmed: true,
    };
    let mut session = match open_for_writing(&dir, origin) {
        Ok(s) => s,
        Err(code) => return code,
    };
    if let Err(e) = session.execute(&checks_set_command(a)) {
        eprintln!("error: {e}");
        session.close();
        return 1;
    }
    let code = match session.save() {
        Ok(saved) => {
            warn_unreleased(saved.unstable, &mut std::io::stderr());
            println!("check schemes set; saved");
            0
        }
        Err(e) => {
            eprintln!("error: the save failed: {e:?}");
            2
        }
    };
    session.close();
    code
}

/// The `project.checks.set` command of `a`: a scheme not given is absent (its default applies).
pub(super) fn checks_set_command(a: &ChecksSetArgs) -> ew_api::AnyCommand {
    let mut params = serde_json::Map::new();
    if let Some(s) = &a.active {
        params.insert("active_scheme".to_owned(), serde_json::Value::from(s.as_str()));
    }
    if let Some(s) = &a.release {
        params.insert("release_scheme".to_owned(), serde_json::Value::from(s.as_str()));
    }
    ew_api::AnyCommand {
        name: "project.checks.set".to_owned(),
        params: serde_json::Value::Object(params),
    }
}

/// The scheme name the person types to confirm `a`: the active scheme's, else the release scheme's.
pub(super) fn confirmation_word(a: &ChecksSetArgs) -> &str {
    a.active.as_deref().or(a.release.as_deref()).unwrap_or_default()
}

/// Prints the change summary of `a` to `output` and reads one line from `input`: `true` when it is exactly the
/// confirmation word ([`confirmation_word`]), surrounding white space aside (doc 17 section 2.3).
pub(super) fn confirm_typed(
    a: &ChecksSetArgs,
    input: &mut dyn std::io::BufRead,
    output: &mut dyn std::io::Write,
) -> bool {
    let word = confirmation_word(a);
    let line = |name: &str, scheme: &Option<String>, default: &str| match scheme {
        Some(s) => format!("  {name} scheme: {s}\n"),
        None => format!("  {name} scheme: back to the default ({default})\n"),
    };
    let summary = format!(
        "project checks set:\n{}{}The check scheme decides check outcomes: which rules the release gate runs and what \
         the checks report.\nType `{word}` to apply it, anything else to abort: ",
        line("active", &a.active, "drafting"),
        line("release", &a.release, "pre-delivery"),
    );
    if output
        .write_all(summary.as_bytes())
        .and_then(|()| output.flush())
        .is_err()
    {
        return false;
    }
    let mut answer = String::new();
    match input.read_line(&mut answer) {
        Ok(_) => answer.trim() == word,
        Err(_) => false,
    }
}

pub(super) fn run_verify_derived(a: &VerifyArgs) -> i32 {
    let Some(dir) = reader_dir(a.project.as_ref()) else {
        return 2;
    };
    let (session, _) = match ew_api::Session::open_reader_with(
        &dir,
        env!("CARGO_PKG_VERSION"),
        "verify-derived",
        ew_api::MemoMode::Oracle,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    let report = match ew_api::verify::verify_derived(&session) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    if a.json {
        print!(
            "{}",
            results::to_text(&results::VerifyDerivedResult {
                schema: results::VERIFY_DERIVED.to_owned(),
                families: report.keys.iter().map(|(f, n)| ((*f).to_owned(), *n)).collect(),
                keys: report.total_keys(),
                differences: report.differences.clone(),
                oracle: report
                    .oracle
                    .iter()
                    .map(|m| results::OracleMismatch {
                        family: m.family.to_owned(),
                        key: m.key.clone(),
                        memoised: m.memoised.clone(),
                        cold: m.cold.clone(),
                    })
                    .collect(),
            })
        );
        return i32::from(!report.is_clean());
    }
    println!(
        "verify-derived: {} families, {} keys, {} differences, {} oracle mismatches",
        report.keys.len(),
        report.total_keys(),
        report.differences.len(),
        report.oracle.len()
    );
    for d in &report.differences {
        println!("differs: {d}");
    }
    for m in &report.oracle {
        println!("oracle: {m:?}");
    }
    i32::from(!report.is_clean())
}

pub(super) fn run_check(a: &CheckArgs) -> i32 {
    let dir = match &a.project {
        Some(d) => d.clone(),
        None => {
            let found = std::env::current_dir().ok().and_then(|cwd| find_project(&cwd));
            match found {
                Some(d) => d,
                None => {
                    eprintln!("error: no project directory given and none found above the current directory");
                    return 2;
                }
            }
        }
    };
    let (mut session, opened) = match ew_api::Session::open_read_only(&dir, env!("CARGO_PKG_VERSION")) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    if a.integrity {
        if a.after_merge {
            session.assume_merged();
        }
        let result = report::localise(&session.check_integrity(&opened), &a.lang);
        return print_check(a, &result, &undrawn_notices(&session));
    }
    // `--scheme`, else the scheme the project names for the mode (FT-366), else the shipped default. A project naming a
    // scheme this build does not ship (a newer writer, a hand edit) gets the default and a notice; the exit status is
    // the check's (FT-405, decision log ID-602).
    let sch = match &a.scheme {
        Some(name) => scheme(name),
        None => {
            let named = session.project_check_scheme_name(a.mode);
            match named.as_deref().and_then(scheme) {
                Some(sch) => Some(sch),
                None => {
                    if let Some(name) = &named {
                        // The name comes from the project file: escaped so control characters cannot reach the
                        // terminal.
                        eprintln!(
                            "notice: project scheme '{}' is not shipped by this build; using pre-delivery",
                            name.escape_debug()
                        );
                    }
                    scheme("pre-delivery")
                }
            }
        }
    };
    let Some(sch) = sch else {
        eprintln!(
            "error: unknown scheme `{}`",
            a.scheme.as_deref().unwrap_or("pre-delivery")
        );
        return 2;
    };
    // The integrity findings of opening (EW-CORE-001 … 019) join the report: in release mode the "n r"
    // errors count and set the exit code (doc 15 §4.2).
    let result = report::localise(
        &session
            .check(sch, a.mode)
            .with_integrity(ew_api::integrity_items(&opened)),
        &a.lang,
    );
    print_check(a, &result, &undrawn_notices(&session))
}

/// The notices of the images the project keeps but no drawing shows, in the window's words (FT-1503, ID-1458).
pub(super) fn undrawn_notices(session: &ew_api::Session) -> Vec<String> {
    session
        .undrawn_image_notices()
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// Prints a check report as `ew check` does, to `--out` or stdout with the totals on stderr; its exit code. The
/// `notices` stand outside the findings and their counts (FT-1503): the JSON's `notices`, else a `notice:` line each on
/// stderr.
pub(super) fn print_check(a: &CheckArgs, result: &ew_api::checks::CheckReport, notices: &[String]) -> i32 {
    let result = result.clone();
    let text = match a.format {
        CheckFormat::Text => report::to_text(&result),
        CheckFormat::Markdown => report::to_markdown(&result),
        CheckFormat::Json => {
            let mut json = report::to_result(&result);
            json.notices = notices.to_vec();
            format!(
                "{:#}\n",
                serde_json::to_value(json).unwrap_or(serde_json::Value::Null)
            )
        }
    };
    if !matches!(a.format, CheckFormat::Json) {
        for notice in notices {
            eprintln!("notice: {notice}");
        }
    }
    match &a.out {
        Some(path) => match write_report(path, &text) {
            Ok(()) => println!("report written to {}", path.display()),
            Err(e) => {
                eprintln!("error: {}: {e}", path.display());
                return 2;
            }
        },
        None => {
            print!("{text}");
            let c = result.counts;
            eprintln!(
                "{}: {} errors, {} warnings, {} infos, {} waived",
                result.scheme, c.errors, c.warnings, c.infos, c.waived
            );
        }
    }
    result.exit_code()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(active: Option<&str>, release: Option<&str>) -> ChecksSetArgs {
        ChecksSetArgs {
            project: None,
            active: active.map(str::to_owned),
            release: release.map(str::to_owned),
        }
    }

    fn typed(a: &ChecksSetArgs, answer: &str) -> (bool, String) {
        let mut input = std::io::Cursor::new(answer.as_bytes().to_vec());
        let mut output = Vec::new();
        let ok = confirm_typed(a, &mut input, &mut output);
        (ok, String::from_utf8(output).unwrap())
    }

    #[test]
    fn the_typed_confirmation_is_the_scheme_name_exactly() {
        let a = args(Some("drafting"), Some("full"));
        let (ok, shown) = typed(&a, "drafting\n");
        assert!(ok);
        assert!(shown.contains("active scheme: drafting"), "{shown}");
        assert!(shown.contains("release scheme: full"), "{shown}");
        assert!(shown.contains("decides check outcomes"), "{shown}");
        assert!(shown.contains("Type `drafting`"), "{shown}");
        for wrong in ["", "\n", "y\n", "yes\n", "Drafting\n", "full\n", "drafting-x\n"] {
            assert!(!typed(&a, wrong).0, "{wrong:?} must abort");
        }
        // Only a release scheme: its name is the word, and the active one is said to return to its default.
        let b = args(None, Some("pre-delivery"));
        let (ok, shown) = typed(&b, "  pre-delivery \r\n");
        assert!(ok);
        assert!(
            shown.contains("active scheme: back to the default (drafting)"),
            "{shown}"
        );
    }

    #[test]
    fn the_command_carries_the_given_schemes_only() {
        let c = checks_set_command(&args(None, Some("full")));
        assert_eq!(c.name, "project.checks.set");
        assert_eq!(c.params, serde_json::json!({"release_scheme": "full"}));
    }

    #[test]
    fn the_subcommand_takes_no_yes_flag() {
        let parse = |a: &[&str]| parse(a.iter().map(|s| (*s).to_owned())).map_err(|e| e.render().to_string());
        assert!(parse(&["project", "checks", "set", "--release", "full", "--confirm"]).is_err());
        assert!(parse(&["project", "checks", "set", "--yes"]).is_err());
        assert!(
            parse(&["project", "checks", "set"]).is_err(),
            "a scheme is needed"
        );
        assert_eq!(
            parse(&["project", "checks", "set", "p.ew", "--active", "full"]),
            Ok(Command::ChecksSet(ChecksSetArgs {
                project: Some(PathBuf::from("p.ew")),
                active: Some("full".to_owned()),
                release: None,
            }))
        );
    }
}
