// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew revision approve|release|withdraw`.

use super::*;

/// The subcommand name of a `ew revision` action, as the origin of its session names it.
pub(super) fn revision_subcommand(action: &RevisionAction) -> &'static str {
    match action {
        RevisionAction::Approve { .. } => "revision approve",
        RevisionAction::Release { .. } => "revision release",
        RevisionAction::Withdraw { .. } => "revision withdraw",
    }
}

/// Today's date (UTC) from the system clock: the date of a sign-off. The CLI is the front-end that reads the clock;
/// the command takes the date as a parameter (WP-S1.16 §10 rule 2).
pub(super) fn today() -> ew_api::catalog::Date {
    use ew_api::Clock as _;
    let seconds = ew_api::SystemClock
        .now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
    date_from_epoch(seconds)
        .and_then(|d| ew_api::catalog::Date::new(d.year, d.month, d.day).ok())
        .unwrap_or_else(|| ew_api::catalog::Date::new(1980, 1, 1).expect("a valid date"))
}

/// `ew revision approve|release|withdraw` (WP-S1.18 part 3, D10): the `HumanOnly` commands of the release workflow,
/// each submitted from the confirmed origin only after the person types the revision label on a controlling terminal
/// (doc 15 section 5.4, doc 17 section 2.3); without a terminal nothing is opened (exit 2).
pub(super) fn run_revision(a: &RevisionArgs) -> i32 {
    use std::io::IsTerminal;
    let terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    run_revision_with(
        a,
        terminal,
        &mut input,
        &mut output,
        ew_api::StoreOptions::new(env!("CARGO_PKG_VERSION")),
        today(),
        &ew_api::git::GitExe::default(),
    )
}

/// One finding as the report text prints it: `<severity> <rule> <code> <subject>: <message>`.
pub(super) fn finding_line(f: &ew_api::checks::Finding) -> String {
    format!(
        "{} {} {} {}: {}\n",
        f.severity, f.rule, f.code, f.subject, f.message
    )
}

/// Prints `prompt` to `output` and reads one line from `input`: `true` when it is exactly `word`, surrounding white
/// space aside.
pub(super) fn typed_word(
    input: &mut dyn std::io::BufRead,
    output: &mut dyn std::io::Write,
    prompt: &str,
    word: &str,
) -> bool {
    let mut answer = String::new();
    output
        .write_all(prompt.as_bytes())
        .and_then(|()| output.flush())
        .is_ok()
        && input.read_line(&mut answer).is_ok()
        && answer.trim() == word
}

/// [`run_revision`] with the terminal test, the reader, the writer, the store options, the date and the git port
/// injected: the confirmation hook of the tests (doc 18 section 3.15's `test-confirm`, realised as the injected
/// terminal; the production binary reads the real terminal and exits 2 without one, and runs the `git` executable).
pub(super) fn run_revision_with(
    a: &RevisionArgs,
    terminal: bool,
    input: &mut dyn std::io::BufRead,
    output: &mut dyn std::io::Write,
    store: ew_api::StoreOptions,
    today: ew_api::catalog::Date,
    git: &dyn ew_api::git::Git,
) -> i32 {
    let subcommand = revision_subcommand(&a.action);
    if !terminal {
        eprintln!(
            "error: HumanOnly: needs an interactive terminal (`{subcommand}` is confirmed by typing the revision \
             label); nothing was changed"
        );
        return 2;
    }
    let checker = match &a.action {
        RevisionAction::Approve { role } => {
            match serde_json::from_value::<ew_api::PartyRole>(serde_json::Value::String(role.clone())) {
                Ok(r) if !matches!(r, ew_api::PartyRole::Unknown(_)) => Some(r),
                _ => {
                    eprintln!(
                        "error: `{role}` is not a party role (customer, end_user, panel_builder, author, checker, \
                         engineer); nothing was changed"
                    );
                    return 2;
                }
            }
        }
        _ => None,
    };
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    let origin = ew_api::Origin::Cli {
        subcommand: subcommand.to_owned(),
        confirmed: true,
    };
    let (mut session, items) = match open_for_writing_items(&dir, origin, store) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let code = match (&a.action, checker) {
        (RevisionAction::Approve { .. }, Some(checker)) => {
            approve_revision(&mut session, &items, &checker, input, output, today)
        }
        (RevisionAction::Release { label, archive_dir }, _) => {
            let release = Release {
                label,
                archive_dir: archive_dir.as_deref(),
                git,
                today,
            };
            release_revision(&mut session, &items, &release, input, output)
        }
        (RevisionAction::Withdraw { label, reason }, _) => {
            withdraw_revision(&mut session, label, reason, input, output)
        }
        (RevisionAction::Approve { .. }, None) => 2,
    };
    // The release's housekeeping (FT-814): a run record that could not be removed, as the project opened or a run
    // was settled.
    for trouble in session.take_release_troubles() {
        eprintln!("warning: {trouble}");
    }
    session.close();
    code
}

/// The warning after a save that wrote an unreleased format (FT-1154, WP-S6.08 D3).
pub(crate) const UNRELEASED_SAVE: &str = "warning: saved in a format that is not released yet; released versions of \
                                          ElektraWatt open this project read-only";

/// After a save: [`UNRELEASED_SAVE`] on `errors` when the manifest it wrote is marked `unstable` (FT-1154); the exit
/// code stays the save's.
pub(crate) fn warn_unreleased(unstable: bool, errors: &mut dyn std::io::Write) {
    if unstable {
        let _ = writeln!(errors, "{UNRELEASED_SAVE}");
    }
}

/// Saves after a confirmed revision command: exit 0 with `done` printed, 2 when the save fails.
pub(super) fn save_revision(
    session: &mut ew_api::Session,
    output: &mut dyn std::io::Write,
    done: &str,
) -> i32 {
    match session.save() {
        Ok(report) => {
            warn_unreleased(report.unstable, &mut std::io::stderr());
            let _ = writeln!(output, "{done}; saved (generation {})", report.generation);
            0
        }
        Err(e) => {
            eprintln!("error: the save failed: {e:?}");
            2
        }
    }
}

/// `ew revision approve --as <role>`: the gate on the saved generation, the safety-kind waivers to acknowledge, the
/// typed label, then `revision.approve` through [`ew_api::Session::approve`] and a save.
pub(super) fn approve_revision(
    session: &mut ew_api::Session,
    items: &[ew_api::checks::IntegrityItem],
    checker: &ew_api::PartyRole,
    input: &mut dyn std::io::BufRead,
    output: &mut dyn std::io::Write,
    today: ew_api::catalog::Date,
) -> i32 {
    let Some(label) = session.open_revision_label() else {
        eprintln!("error: the project has no open revision; nothing was approved");
        return 1;
    };
    let gate = match session.release_gate(&label, items) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("error: {e}; nothing was approved");
            return 1;
        }
    };
    let blocked = gate.approval_blockers();
    if !blocked.is_empty() {
        let listing: String = blocked.iter().map(|f| finding_line(f)).collect();
        let _ = output.write_all(listing.as_bytes());
        eprintln!(
            "error: the release gate has {} unwaived error(s) besides {}; nothing was approved",
            blocked.len(),
            ew_api::FOUR_EYES_RULE
        );
        return 1;
    }
    let role = ew_api::role_name(checker);
    let mut summary = format!(
        "approve revision {label} as {role} (author: {}): generation {}, content digest {}, scheme {}\n",
        gate.author
            .as_ref()
            .map_or_else(|| "not set".to_owned(), ew_api::role_name),
        gate.generation,
        gate.content_digest,
        gate.report.scheme
    );
    for w in &gate.safety_waivers {
        let waived = gate.report.findings.iter().find(|f| {
            matches!(&f.state, ew_api::checks::FindingState::Waived { waiver } if *waiver == w.to_string())
        });
        let what = waived.map_or_else(String::new, |f| format!(": {} {}", f.rule, f.subject));
        summary.push_str(&format!("  safety waiver {w}{what}\n"));
    }
    let acknowledge = if gate.safety_waivers.is_empty() {
        String::new()
    } else {
        format!(
            " and acknowledge the {} safety waiver(s) above",
            gate.safety_waivers.len()
        )
    };
    let prompt = format!(
        "{summary}Type the revision label `{label}` to approve it{acknowledge}, anything else to abort: "
    );
    if !typed_word(input, output, &prompt, &label) {
        eprintln!("error: not confirmed; nothing was changed");
        return 1;
    }
    let approval = ew_api::Approval {
        label: label.clone(),
        checker: checker.clone(),
        date: today,
        waivers_ack: gate.safety_waivers.clone(),
    };
    if let Err(e) = session.approve(&approval, items) {
        if let ew_api::ApproveError::Blocked(list) = &e {
            let listing: String = list.iter().map(finding_line).collect();
            let _ = output.write_all(listing.as_bytes());
        }
        eprintln!("error: {e}; nothing was approved");
        return 1;
    }
    save_revision(session, output, &format!("approved revision {label} as {role}"))
}

/// What `ew revision release` was asked: the label, the archive root, the git port and the release date.
pub(super) struct Release<'a> {
    label: &'a str,
    archive_dir: Option<&'a Path>,
    git: &'a dyn ew_api::git::Git,
    today: ew_api::catalog::Date,
}

impl Release<'_> {
    /// The release options; `git_consent` is the typed confirmation, whose prompt names the git steps.
    fn options(&self, git_consent: bool) -> ew_api::ReleaseOptions<'_> {
        ew_api::ReleaseOptions {
            archive_root: self.archive_dir,
            git: self.git,
            git_consent,
            parallelism: Parallelism::threads(
                std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get),
            ),
        }
    }
}

/// The exit code of a release error: 1 for a refusal before anything was written, 2 when writing failed.
pub(super) fn release_exit(e: &ew_api::ReleaseError) -> i32 {
    use ew_api::ReleaseError as E;
    match e {
        E::Save(_)
        | E::Generation(_)
        | E::Output(_)
        | E::Archive(_)
        | E::Write(_)
        | E::Stamp(_)
        | E::Unsettled(_)
        | E::Undecided(_)
        | E::Io { .. } => 2,
        _ => 1,
    }
}

/// `ew revision release <label>` (doc 05 section 14.1; WP-S1.18 part 4): an incomplete release of the label is settled
/// first ([`settle_release`]); else the plan (the gate on the saved generation, with its blockers when it refuses; the
/// archive; the git worktree and tag), the typed label, then [`ew_api::Session::release`], which saves.
pub(super) fn release_revision(
    session: &mut ew_api::Session,
    items: &[ew_api::checks::IntegrityItem],
    release: &Release<'_>,
    input: &mut dyn std::io::BufRead,
    output: &mut dyn std::io::Write,
) -> i32 {
    let label = release.label;
    match session.incomplete_release(release.archive_dir) {
        Ok(Some(found)) if found.label == label => {
            return settle_release(session, items, release, &found, input, output);
        }
        Ok(_) => {}
        Err(e) => {
            eprintln!("error: {e}; nothing was released");
            return release_exit(&e);
        }
    }
    let plan = match session.release_plan(label, items, &release.options(false)) {
        Ok(plan) => plan,
        Err(ew_api::ReleaseError::Blocked(blockers)) => {
            let listing: String = blockers.iter().map(finding_line).collect();
            let _ = output.write_all(listing.as_bytes());
            eprintln!(
                "error: release of {label} refused: the gate has {} blocker(s); nothing was released",
                blockers.len()
            );
            return 1;
        }
        Err(e) => {
            eprintln!("error: {e}; nothing was released");
            return release_exit(&e);
        }
    };
    let mut summary = format!(
        "release revision {label}: generation {}, content digest {}, scheme {}\n  archive: {}\n",
        plan.gate.generation,
        plan.gate.content_digest,
        plan.gate.report.scheme,
        plan.archive.display()
    );
    if let Some((worktree, tag)) = &plan.git {
        summary.push_str(&format!(
            "  git: commits the project directory in the worktree {} (\"Release {label}\") and creates the tag {tag}\n",
            worktree.display()
        ));
        if plan.archive_in_worktree {
            summary.push_str(&format!(
                "warning: the archive root lies inside the git worktree {}; archives are never added to git, so name a \
                 root outside it with --archive-dir <dir>\n",
                worktree.display()
            ));
        }
    }
    let prompt = format!(
        "{summary}A released revision is immutable. Type the revision label `{label}` to release it, anything else to \
         abort: "
    );
    if !typed_word(input, output, &prompt, label) {
        eprintln!("error: not confirmed; nothing was changed");
        return 1;
    }
    let ctx = ew_api::ReleaseContext {
        label: label.to_owned(),
        date: release.today,
    };
    match session.release(&ctx, items, &release.options(true)) {
        Ok(done) => {
            released_lines(output, &done);
            0
        }
        Err(e) => {
            if let ew_api::ReleaseError::Blocked(list) = &e {
                let listing: String = list.iter().map(finding_line).collect();
                let _ = output.write_all(listing.as_bytes());
            }
            eprintln!("error: {e}");
            release_exit(&e)
        }
    }
}

/// What a completed release prints.
pub(super) fn released_lines(output: &mut dyn std::io::Write, done: &ew_api::Released) {
    let _ = writeln!(
        output,
        "released revision {}: archive {} ({}); saved (generation {})",
        done.label,
        done.archive.display(),
        done.sha256,
        done.stamped
    );
    if let Some((_, tag)) = &done.git {
        let _ = writeln!(
            output,
            "tagged {tag}; generation {} is left for your next commit",
            done.stamped
        );
    }
    if done.save_incomplete {
        let _ = writeln!(
            output,
            "note: the save of generation {} completes when the project is next opened",
            done.stamped
        );
    }
    warn_unreleased(done.unstable, &mut std::io::stderr());
}

/// An archive of the open revision's label that a crashed release left (doc 05 section 14.1 "On failure"): the
/// person types `complete` (when it can be completed) to stamp it, or `discard` (when this project directory's own run
/// provably wrote it, FT-814) to remove it and its tag. An archive that is not this project's (another project's or
/// revision's, or one that names no project) is offered neither: it is kept, and the release is refused; so is one that
/// can neither be completed nor provably discarded.
pub(super) fn settle_release(
    session: &mut ew_api::Session,
    items: &[ew_api::checks::IntegrityItem],
    release: &Release<'_>,
    found: &ew_api::IncompleteRelease,
    input: &mut dyn std::io::BufRead,
    output: &mut dyn std::io::Write,
) -> i32 {
    let label = &found.label;
    if let Some(why) = &found.foreign {
        eprintln!(
            "error: release of {label} refused: {why}; it is kept. Name another archive folder with --archive-dir \
             <dir>; nothing was changed"
        );
        return 1;
    }
    if let (Some(refusal), Some(unproven)) = (&found.refusal, &found.unproven) {
        eprintln!(
            "error: release of {label} refused: {unproven}, and it cannot be completed ({refusal}); it is kept. Remove \
             the file by hand if you know it is a leftover, or name another archive folder with --archive-dir <dir>; \
             nothing was changed"
        );
        return 1;
    }
    let mut summary = format!(
        "incomplete release {label}: the archive {} ({}) is in place, but the revision is still open\n",
        found.archive.display(),
        found.sha256
    );
    let words = match (&found.refusal, &found.unproven) {
        (Some(why), _) => {
            summary.push_str(&format!("  it cannot be completed: {why}\n"));
            "`discard` to remove it (and the tag of the label, if this folder's crashed release created it)"
        }
        (None, Some(why)) => {
            summary.push_str(&format!(
                "  it is not offered for discarding: {why}; remove the file by hand if you know it is a leftover\n"
            ));
            "`complete` to stamp the revision released with it (committing and tagging when the project lies in a git \
             worktree)"
        }
        (None, None) => {
            "`complete` to stamp the revision released with it (committing and tagging when the project lies in a git \
             worktree), `discard` to remove it (and the tag of the label, if this folder's crashed release created it)"
        }
    };
    let prompt = format!("{summary}Type {words}, anything else to abort: ");
    let mut answer = String::new();
    let read = output
        .write_all(prompt.as_bytes())
        .and_then(|()| output.flush())
        .is_ok()
        && input.read_line(&mut answer).is_ok();
    match (read, answer.trim(), &found.refusal) {
        (true, "complete", None) => match session.complete_release(items, &release.options(true)) {
            Ok(done) => {
                released_lines(output, &done);
                0
            }
            Err(e) => {
                eprintln!("error: {e}");
                release_exit(&e)
            }
        },
        (true, "discard", _) => match session.discard_incomplete_release(&release.options(true)) {
            Ok(done) => {
                let tag = done
                    .tag_removed
                    .map_or_else(String::new, |t| format!(" and the tag {t}"));
                let _ = writeln!(
                    output,
                    "discarded the incomplete release {label}: removed {}{tag}; the revision is still open",
                    done.archive.display()
                );
                if let Some(t) = done.tag_kept {
                    let _ = writeln!(
                        output,
                        "note: the git tag {t} was not made by this folder's crashed release, so it was kept; a new \
                         release of {label} needs it gone"
                    );
                }
                0
            }
            Err(e) => {
                eprintln!("error: {e}");
                release_exit(&e)
            }
        },
        _ => {
            eprintln!("error: not confirmed; nothing was changed");
            1
        }
    }
}

/// `ew revision withdraw <label> --reason <text>`: the typed label, then `revision.withdraw` of the released revision
/// and a save.
pub(super) fn withdraw_revision(
    session: &mut ew_api::Session,
    label: &str,
    reason: &str,
    input: &mut dyn std::io::BufRead,
    output: &mut dyn std::io::Write,
) -> i32 {
    let Some(id) = session.released_revision(label) else {
        eprintln!("error: the project has no released revision {label}; nothing was withdrawn");
        return 1;
    };
    let prompt = format!(
        "withdraw revision {label}: {reason}\nA withdrawal stays in the revision history. Type the revision label \
         `{label}` to withdraw it, anything else to abort: "
    );
    if !typed_word(input, output, &prompt, label) {
        eprintln!("error: not confirmed; nothing was changed");
        return 1;
    }
    let cmd = ew_api::AnyCommand {
        name: "revision.withdraw".to_owned(),
        params: serde_json::json!({"revision": id.to_string(), "reason": reason}),
    };
    if let Err(e) = session.execute(&cmd) {
        eprintln!("error: {e}; nothing was withdrawn");
        return 1;
    }
    save_revision(session, output, &format!("withdrew revision {label}"))
}

#[cfg(test)]
mod tests {
    //! `ew revision approve|release|withdraw` (WP-S1.18 part 3, D10) with the injected terminal: the runs work on a
    //! copy of Golden A in a fresh directory; nothing under `golden/` is opened for writing.
    use super::*;

    fn parse(a: &[&str]) -> Result<Command, String> {
        super::parse(a.iter().map(|s| (*s).to_owned())).map_err(|e| e.render().to_string())
    }

    #[test]
    fn revision_subcommands_parse_their_label_role_reason_and_directory() {
        assert_eq!(
            parse(&["revision", "approve", "--as", "checker"]),
            Ok(Command::Revision(RevisionArgs {
                action: RevisionAction::Approve {
                    role: "checker".into()
                },
                project: None,
            }))
        );
        assert_eq!(
            parse(&["revision", "release", "V2.0", "p.ew"]),
            Ok(Command::Revision(RevisionArgs {
                action: RevisionAction::Release {
                    label: "V2.0".into(),
                    archive_dir: None
                },
                project: Some(PathBuf::from("p.ew")),
            }))
        );
        assert_eq!(
            parse(&["revision", "release", "V2.0", "--archive-dir", "out/archives"]),
            Ok(Command::Revision(RevisionArgs {
                action: RevisionAction::Release {
                    label: "V2.0".into(),
                    archive_dir: Some(PathBuf::from("out/archives"))
                },
                project: None,
            }))
        );
        assert_eq!(
            parse(&["revision", "withdraw", "V1.0", "--reason", "wrong terminals"]),
            Ok(Command::Revision(RevisionArgs {
                action: RevisionAction::Withdraw {
                    label: "V1.0".into(),
                    reason: "wrong terminals".into()
                },
                project: None,
            }))
        );
        for bad in [
            &["revision"][..],
            &["revision", "sign"],
            &["revision", "approve"],
            &["revision", "approve", "--as"],
            &["revision", "approve", "--as", "checker", "--yes"],
            &["revision", "release"],
            &["revision", "release", "V2.0", "a", "b"],
            &["revision", "release", "V2.0", "--as", "checker"],
            &["revision", "release", "V2.0", "--archive-dir"],
            &["revision", "approve", "--as", "checker", "--archive-dir", "a"],
            &["revision", "withdraw", "V1.0"],
            &["revision", "withdraw", "--reason", "r"],
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }

    fn golden() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew")
    }

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap() {
            let e = e.unwrap();
            let target = to.join(e.file_name());
            if e.path().is_dir() {
                copy_dir(&e.path(), &target);
            } else {
                std::fs::copy(e.path(), &target).unwrap();
            }
        }
    }

    /// A copy of Golden A in a fresh directory.
    fn golden_copy(tag: &str) -> PathBuf {
        let dir = ew_testdir::fresh_dir(&format!("ew-cli-revision-{tag}"));
        copy_dir(&golden(), &dir);
        dir
    }

    fn store() -> ew_api::StoreOptions {
        let mut o = ew_api::StoreOptions::new("test");
        o.state_dir = None;
        o
    }

    fn date() -> ew_api::catalog::Date {
        ew_api::catalog::Date::new(2025, 6, 2).unwrap()
    }

    /// The bytes of every file under `dir`, by relative path: "nothing was written" compares two of these.
    fn files(dir: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
        fn walk(root: &Path, dir: &Path, out: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    walk(root, &p, out);
                } else {
                    out.insert(
                        p.strip_prefix(root).unwrap().to_path_buf(),
                        std::fs::read(&p).unwrap(),
                    );
                }
            }
        }
        let mut out = std::collections::BTreeMap::new();
        walk(dir, dir, &mut out);
        out
    }

    /// The git port of the tests: the copies in the temporary directory lie in no worktree, and no test looks.
    struct NoGit;

    impl ew_api::git::Git for NoGit {
        fn worktree(&self, _: &dyn ew_api::Vfs, _: &Path) -> std::io::Result<Option<PathBuf>> {
            Ok(None)
        }
        fn available(&self) -> Result<(), ew_api::git::GitError> {
            Err(ew_api::git::GitError::Missing)
        }
        fn commit_dir(&self, _: &Path, _: &str) -> Result<ew_api::git::Committed, ew_api::git::GitError> {
            Err(ew_api::git::GitError::Missing)
        }
        fn tag_message(&self, _: &Path, _: &str) -> Result<Option<String>, ew_api::git::GitError> {
            Err(ew_api::git::GitError::Missing)
        }
        fn tag_exists(&self, _: &Path, _: &str) -> Result<bool, ew_api::git::GitError> {
            Err(ew_api::git::GitError::Missing)
        }
        fn tag(&self, _: &Path, _: &str, _: &str) -> Result<(), ew_api::git::GitError> {
            Err(ew_api::git::GitError::Missing)
        }
        fn delete_tag(&self, _: &Path, _: &str) -> Result<(), ew_api::git::GitError> {
            Err(ew_api::git::GitError::Missing)
        }
    }

    /// Runs `ew revision <action> <dir>` with `terminal` and the typed `answer`; the exit code and what it printed.
    fn run(dir: &Path, action: RevisionAction, terminal: bool, answer: &str) -> (i32, String) {
        run_in(dir, action, terminal, answer, None)
    }

    /// [`run`] with the state directory `state` (the release's run records, FT-814).
    fn run_in(
        dir: &Path,
        action: RevisionAction,
        terminal: bool,
        answer: &str,
        state: Option<&Path>,
    ) -> (i32, String) {
        let args = RevisionArgs {
            action,
            project: Some(dir.to_path_buf()),
        };
        let mut input = std::io::Cursor::new(answer.as_bytes().to_vec());
        let mut output = Vec::new();
        let mut store = store();
        store.state_dir = state.map(Path::to_path_buf);
        let code = run_revision_with(&args, terminal, &mut input, &mut output, store, date(), &NoGit);
        (code, String::from_utf8(output).unwrap())
    }

    fn approve(role: &str) -> RevisionAction {
        RevisionAction::Approve { role: role.into() }
    }

    fn release(label: &str) -> RevisionAction {
        RevisionAction::Release {
            label: label.into(),
            archive_dir: None,
        }
    }

    fn release_into(label: &str, root: &Path) -> RevisionAction {
        RevisionAction::Release {
            label: label.into(),
            archive_dir: Some(root.to_path_buf()),
        }
    }

    fn withdraw(label: &str) -> RevisionAction {
        RevisionAction::Withdraw {
            label: label.into(),
            reason: "the terminal plan was wrong".into(),
        }
    }

    /// Waives every error of the release gate besides EW-REL-001 from the GUI and saves.
    fn waive_the_gate(dir: &Path) {
        let (mut s, report) = ew_api::Session::open(
            dir,
            store(),
            ew_api::SessionOptions::default(),
            ew_api::Recovery::Keep,
        )
        .unwrap();
        let items = ew_api::integrity_items(&report);
        let gate = s.release_gate("V2.0", &items).unwrap();
        let waive: Vec<(String, String)> = gate
            .approval_blockers()
            .iter()
            .map(|f| (f.rule.to_owned(), f.fingerprint.to_string()))
            .collect();
        for (rule, fingerprint) in waive {
            let cmd = ew_api::AnyCommand {
                name: "check.waive".into(),
                params: serde_json::json!({"rule": rule, "fingerprint": fingerprint,
                    "reason": "accepted for the test release", "author": "engineer", "date": "2025-06-01"}),
            };
            s.execute_with(&cmd, ew_api::Submit::from(ew_api::Origin::Gui))
                .unwrap();
        }
        s.save().unwrap();
        s.close();
    }

    /// Without a terminal each subcommand exits 2 and writes nothing; a role that is no party role exits 2.
    #[test]
    fn revision_commands_need_a_terminal_and_a_party_role() {
        let dir = golden_copy("terminal");
        let before = files(&dir);
        for action in [approve("checker"), release("V2.0"), withdraw("V1.0")] {
            let (code, shown) = run(&dir, action.clone(), false, "V2.0\nV1.0\n");
            assert_eq!(code, 2, "{action:?}");
            assert_eq!(shown, "", "{action:?}: no prompt");
        }
        let (code, shown) = run(&dir, approve("boss"), true, "V2.0\n");
        assert_eq!((code, shown.as_str()), (2, ""));
        assert_eq!(files(&dir), before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// RG-4/D15 through the CLI: the release of Golden A is refused with the gate's blockers listed (the release
    /// scheme's errors and EW-REL-001's checker), and the approval is refused while those errors are open.
    #[test]
    fn the_gate_refuses_release_and_approval_of_golden_a_listing_the_blockers() {
        let dir = golden_copy("refused");
        let before = files(&dir);
        let (code, shown) = run(&dir, release("V2.0"), true, "");
        assert_eq!(code, 1, "{shown}");
        assert!(
            shown
                .lines()
                .any(|l| l.starts_with("error EW-REL-001 ") && l.contains("checker not set")),
            "{shown}"
        );
        assert!(shown.lines().any(|l| l.starts_with("error EW-POT-")), "{shown}");
        assert!(shown.lines().all(|l| l.starts_with("error ")), "{shown}");
        let (code, listed) = run(&dir, approve("checker"), true, "V2.0\n");
        assert_eq!(code, 1, "{listed}");
        assert!(!listed.contains("EW-REL-001"), "{listed}");
        assert!(!listed.contains("Type the revision label"), "no prompt: {listed}");
        assert_eq!(listed.lines().count() + 1, shown.lines().count());
        // A label that is not the open revision's.
        let (code, _) = run(&dir, release("V1.0"), true, "");
        assert_eq!(code, 1);
        assert_eq!(files(&dir), before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// RG-1/RG-3 and RA-1 through the CLI: the typed label approves (the safety waivers listed first), the author's
    /// role and a wrong label do not; the release shows its plan and needs the typed label, then writes the archive
    /// (with Golden A's report blocks as CSV) and stamps the revision; a released revision is withdrawn on its typed
    /// label.
    #[test]
    fn approve_release_and_withdraw_on_the_typed_label() {
        let dir = golden_copy("approve");
        let root = dir.with_extension("archives");
        let _ = std::fs::remove_dir_all(&root);
        waive_the_gate(&dir);
        let before = files(&dir);
        for wrong in ["", "\n", "v2.0\n", "V2\n", "yes\n", "V1.0\n"] {
            let (code, shown) = run(&dir, approve("checker"), true, wrong);
            assert_eq!(code, 1, "{wrong:?}");
            assert!(
                shown.contains("Type the revision label `V2.0` to approve it and acknowledge the"),
                "{shown}"
            );
            assert!(shown.contains("  safety waiver "), "{shown}");
        }
        // The author's party may not check (Golden A's V2.0 is authored by the engineer).
        let (code, _) = run(&dir, approve("engineer"), true, "V2.0\n");
        assert_eq!(code, 1);
        assert_eq!(files(&dir), before);

        let (code, shown) = run(&dir, approve("checker"), true, " V2.0 \r\n");
        assert_eq!(code, 0, "{shown}");
        assert!(
            shown.contains("approved revision V2.0 as checker; saved"),
            "{shown}"
        );
        let revisions = std::fs::read_to_string(dir.join("revisions.jsonl")).unwrap();
        assert!(revisions.contains("\"sign_off\""), "{revisions}");
        assert!(revisions.contains("\"checker\":\"checker\""), "{revisions}");

        // The gate passes now: the plan and the prompt; a wrong label writes nothing.
        let approved = files(&dir);
        let (code, shown) = run(&dir, release_into("V2.0", &root), true, "V1.0\n");
        assert_eq!(code, 1, "{shown}");
        assert!(shown.starts_with("release revision V2.0: generation "), "{shown}");
        assert!(shown.contains("  archive: "), "{shown}");
        assert!(!shown.contains("  git: "), "outside a worktree: {shown}");
        assert!(
            shown.contains("Type the revision label `V2.0` to release it"),
            "{shown}"
        );
        assert_eq!(files(&dir), approved);
        assert!(!root.exists());
        // An archive root inside the project is refused.
        let (code, _) = run(&dir, release_into("V2.0", &dir.join("archives")), true, "V2.0\n");
        assert_eq!(code, 1);
        assert_eq!(files(&dir), approved);

        let (code, shown) = run(&dir, release_into("V2.0", &root), true, "V2.0\n");
        assert_eq!(code, 0, "{shown}");
        assert!(shown.contains("released revision V2.0: archive "), "{shown}");
        let archive = std::fs::read(root.join("V2.0.ewarchive")).expect("the archive");
        let limits = ew_api::zip::ZipLimits {
            max_ratio: u64::MAX,
            ..ew_api::zip::ZipLimits::default()
        };
        let entries = ew_api::archive::read(&archive, &limits).expect("the archive verifies");
        assert!(
            entries
                .keys()
                .any(|k| k.starts_with("reports/") && k.ends_with(".csv")),
            "{:?}",
            entries.keys()
        );
        assert!(
            entries.keys().any(|k| k.starts_with("pdf/")),
            "{:?}",
            entries.keys()
        );
        let revisions = std::fs::read_to_string(dir.join("revisions.jsonl")).unwrap();
        assert!(revisions.contains("\"V2.0.ewarchive\""), "{revisions}");
        // Released once: a second release is refused.
        let (code, _) = run(&dir, release_into("V2.0", &root), true, "V2.0\n");
        assert_eq!(code, 1);
        let released = files(&dir);

        // Withdraw: a wrong label, then the typed label.
        let (code, shown) = run(&dir, withdraw("V1.0"), true, "V2.0\n");
        assert_eq!(code, 1);
        assert!(
            shown.contains("Type the revision label `V1.0` to withdraw it"),
            "{shown}"
        );
        assert_eq!(files(&dir), released);
        let (code, shown) = run(&dir, withdraw("V1.0"), true, "V1.0\n");
        assert_eq!(code, 0, "{shown}");
        let revisions = std::fs::read_to_string(dir.join("revisions.jsonl")).unwrap();
        assert!(revisions.contains("the terminal plan was wrong"), "{revisions}");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// RA-3 through the CLI: an archive of the open revision's label that no record names (what a crashed release
    /// leaves: here the archive of an identical copy) is offered before a new release; `complete` stamps the revision
    /// with it, and a damaged one can only be discarded. FT-814: a copy of the project finds the other copy's archive
    /// in the shared archive folder, but `discard` is not offered without this folder's run record, and typed it
    /// removes nothing; with the record it is offered and removes the archive and the record.
    #[test]
    fn an_incomplete_release_is_completed_or_discarded() {
        let a = golden_copy("incomplete-a");
        waive_the_gate(&a);
        let (code, shown) = run(&a, approve("checker"), true, "V2.0\n");
        assert_eq!(code, 0, "{shown}");
        let b = ew_testdir::fresh_dir("ew-cli-revision-incomplete-b");
        copy_dir(&a, &b);
        let root_a = a.with_extension("archives");
        let root_b = b.with_extension("archives");
        let state = a.with_extension("state");
        for r in [&root_a, &root_b, &state] {
            let _ = std::fs::remove_dir_all(r);
        }
        let (code, shown) = run_in(&a, release_into("V2.0", &root_a), true, "V2.0\n", Some(&state));
        assert_eq!(code, 0, "{shown}");

        // B, a copy of A, on A's archive folder with the same state directory: `discard` is not offered, and typed it
        // removes nothing (FT-814).
        let archived = std::fs::read(root_a.join("V2.0.ewarchive")).unwrap();
        let before_b = files(&b);
        let (code, shown) = run_in(&b, release_into("V2.0", &root_a), true, "discard\n", Some(&state));
        assert_eq!(code, 1, "{shown}");
        assert!(
            shown.contains(
                "  it is not offered for discarding: the archive V2.0.ewarchive in that folder is not provably a \
                 leftover of this project folder's own release (no release from this folder recorded writing it, and a \
                 copy of the folder names the same project); remove the file by hand if you know it is a leftover\n"
            ),
            "{shown}"
        );
        assert!(shown.contains("Type `complete` to stamp"), "{shown}");
        assert!(!shown.contains("`discard`"), "{shown}");
        assert_eq!(std::fs::read(root_a.join("V2.0.ewarchive")).unwrap(), archived);
        assert_eq!(files(&b), before_b);

        std::fs::create_dir_all(&root_b).unwrap();
        std::fs::copy(root_a.join("V2.0.ewarchive"), root_b.join("V2.0.ewarchive")).unwrap();

        // Aborted: nothing changes.
        let before = files(&b);
        let (code, shown) = run(&b, release_into("V2.0", &root_b), true, "V2.0\n");
        assert_eq!(code, 1, "{shown}");
        assert!(
            shown.starts_with("incomplete release V2.0: the archive "),
            "{shown}"
        );
        assert!(shown.contains("Type `complete` to stamp"), "{shown}");
        assert_eq!(files(&b), before);
        // With a run record of B's folder for the archive (what B's own crashed run leaves), `discard` is offered and
        // removes the archive and the record.
        let record = run_record_of(&b, &root_b, &state);
        let (code, shown) = run_in(&b, release_into("V2.0", &root_b), true, "discard\n", Some(&state));
        assert_eq!(code, 0, "{shown}");
        assert!(shown.contains(", `discard` to remove it"), "{shown}");
        assert!(
            shown.contains("discarded the incomplete release V2.0: removed "),
            "{shown}"
        );
        assert!(!root_b.join("V2.0.ewarchive").exists());
        assert!(!record.exists(), "the run's record is removed");
        assert_eq!(files(&b), before_b);
        std::fs::copy(root_a.join("V2.0.ewarchive"), root_b.join("V2.0.ewarchive")).unwrap();

        // Completed with the archive's own facts.
        let (code, shown) = run(&b, release_into("V2.0", &root_b), true, "complete\n");
        assert_eq!(code, 0, "{shown}");
        assert!(shown.contains("released revision V2.0: archive "), "{shown}");
        assert_eq!(
            std::fs::read(b.join("revisions.jsonl")).unwrap(),
            std::fs::read(a.join("revisions.jsonl")).unwrap(),
            "the same release record as the uninterrupted release"
        );

        // A damaged archive names no project: neither completed nor discarded, but kept, and nothing is offered.
        let c = ew_testdir::fresh_dir("ew-cli-revision-incomplete-c");
        copy_dir(&golden(), &c);
        let root_c = c.with_extension("archives");
        let _ = std::fs::remove_dir_all(&root_c);
        std::fs::create_dir_all(&root_c).unwrap();
        std::fs::write(root_c.join("V2.0.ewarchive"), b"not an archive").unwrap();
        for answer in ["complete\n", "discard\n"] {
            let (code, shown) = run(&c, release_into("V2.0", &root_c), true, answer);
            assert_eq!(code, 1, "{shown}");
            assert!(!shown.contains("Type `discard`"), "{shown}");
            assert_eq!(
                std::fs::read(root_c.join("V2.0.ewarchive")).unwrap(),
                b"not an archive",
                "{answer}: the damaged archive is kept"
            );
        }
        for d in [&a, &b, &c, &root_a, &root_b, &root_c, &state] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    /// Writes the run record a release of the project in `dir` leaves in `state` once it has written the archive of the
    /// open revision in `root` (FT-814), as if that run had crashed right after; returns the record's file.
    fn run_record_of(dir: &Path, root: &Path, state: &Path) -> PathBuf {
        let mut options = store();
        options.state_dir = Some(state.to_path_buf());
        let (session, _) = ew_api::Session::open(
            dir,
            options,
            ew_api::SessionOptions::default(),
            ew_api::Recovery::Keep,
        )
        .unwrap();
        let found = session.incomplete_release(Some(root)).unwrap().unwrap();
        let store = session.store().unwrap();
        let vfs = &**store.vfs();
        let modified = std::fs::metadata(&found.archive).unwrap().modified().unwrap();
        let record = ew_api::archive::RunRecord {
            project_dir: ew_api::archive::path_text(vfs, store.root()),
            archive: ew_api::archive::path_text(vfs, &found.archive),
            project: session.query().project().id.to_string(),
            revision: found.revision.to_string(),
            label: found.label.clone(),
            generation: found.facts.as_ref().unwrap().1,
            sha256: found.sha256.to_string(),
            modified: ew_api::archive::record_time(modified),
            run: "the-crashed-run".to_owned(),
        };
        let path = ew_api::archive::write_run_record(vfs, state, &record).unwrap();
        session.close();
        path
    }
}
