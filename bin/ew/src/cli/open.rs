// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Finding and opening the project, for reading and for writing, and the words of the refusals.

use super::*;

/// The nearest ancestor of `start` (itself included) that holds `manifest.json` (doc 17 §3.1).
pub(super) fn find_project(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|d| d.join("manifest.json").is_file())
        .map(Path::to_path_buf)
}

/// The explicit project directory, else the one found from the current directory, for every subcommand but the readers
/// ([`reader_dir`]): an `.ewz` is refused with D9's text (WP-S1.17; it opens read-only).
pub(super) fn project_dir(explicit: Option<&PathBuf>) -> Option<PathBuf> {
    let dir = reader_dir(explicit)?;
    if ew_api::ewz::is_ewz(&ew_api::OsVfs, &dir) {
        eprintln!("{}", ewz_refusal(&dir));
        return None;
    }
    Some(dir)
}

/// The refusal of a subcommand that is not a reader, given an `.ewz` as its project (WP-S1.17 D9).
pub(super) fn ewz_refusal(path: &Path) -> String {
    format!("error: {}: {}", path.display(), ew_api::ewz::READ_ONLY)
}

/// The explicit project, else the directory found from the current directory, for a reader (`check`, `sql`, the row
/// and page exports, `export table`, `print`, `render`, `verify-derived`, `query`, `layout`, `fmt --check`, `upgrade
/// --check`): an `.ewz` is taken as it is and opened read-only from memory by `Session::open_reader` (WP-S1.17 D9, ID-1175 ruling
/// 5, ID-1230 ruling 5).
pub(super) fn reader_dir(explicit: Option<&PathBuf>) -> Option<PathBuf> {
    match explicit {
        Some(d) => Some(d.clone()),
        None => {
            let found = std::env::current_dir().ok().and_then(|cwd| find_project(&cwd));
            if found.is_none() {
                eprintln!("error: no project directory given and none found above the current directory");
            }
            found
        }
    }
}

/// Opens the project read-only and builds its SQL projection (exit code 2 on failure).
pub(super) fn open_projection(project: Option<&PathBuf>) -> Result<ew_api::sql::Projection, i32> {
    open_projection_and_session(project).map(|(projection, _)| projection)
}

/// [`open_projection`] with the session it was built from (the metadata of `ew sql --format xlsx`).
pub(super) fn open_projection_and_session(
    project: Option<&PathBuf>,
) -> Result<(ew_api::sql::Projection, ew_api::Session), i32> {
    let dir = reader_dir(project).ok_or(2)?;
    let (session, opened) = match ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "sql") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return Err(2);
        }
    };
    let report = session
        .check(&ew_api::checks::PRE_DELIVERY, CheckMode::Release)
        .with_integrity(ew_api::integrity_items(&opened));
    let projection = session.sql_projection(Some(&report)).map_err(|e| {
        eprintln!("error: {e}");
        2
    })?;
    Ok((projection, session))
}

/// Opens the project in `dir` for writing with `origin`: refused (exit 2) when it opens read-only or a crashed
/// session left unsaved changes in its journal, which a new save would hide (doc 05 section 8.3).
pub(super) fn open_for_writing(dir: &Path, origin: ew_api::Origin) -> Result<ew_api::Session, i32> {
    open_for_writing_with(dir, origin, ew_api::StoreOptions::new(env!("CARGO_PKG_VERSION")))
}

/// [`open_for_writing`] with the store options given.
pub(super) fn open_for_writing_with(
    dir: &Path,
    origin: ew_api::Origin,
    store: ew_api::StoreOptions,
) -> Result<ew_api::Session, i32> {
    open_for_writing_items(dir, origin, store).map(|(session, _)| session)
}

/// [`open_for_writing_with`] that also returns the integrity findings of opening (EW-CORE-001 … 019), which the release
/// gate joins as `ew check` does.
pub(super) fn open_for_writing_items(
    dir: &Path,
    origin: ew_api::Origin,
    store: ew_api::StoreOptions,
) -> Result<(ew_api::Session, Vec<ew_api::checks::IntegrityItem>), i32> {
    open_for_writing_from(dir, origin, store, None)
}

/// [`open_for_writing_items`] with the session's library tiers: `ew apply`'s, so that a `plc.set_profile` of a
/// library's address profile embeds it first (WP-S7.03 D8).
pub(super) fn open_for_writing_from(
    dir: &Path,
    origin: ew_api::Origin,
    store: ew_api::StoreOptions,
    libraries: Option<ew_api::library::LibraryTiers>,
) -> Result<(ew_api::Session, Vec<ew_api::checks::IntegrityItem>), i32> {
    let (session, opened) = open_editing_from(dir, origin, store, ew_api::Recovery::Keep, libraries)?;
    if opened.recovery.is_some() {
        eprintln!("{}", journal_refusal(dir));
        session.close();
        return Err(2);
    }
    if let Some(from) = &opened.migrated_from {
        eprintln!("error: {}", older_format(dir, from));
        session.close();
        return Err(2);
    }
    let items = ew_api::integrity_items(&opened);
    Ok((session, items))
}

/// Opens the project in `dir` for editing with `origin`, `recovery` saying what happens to a crashed session's
/// journal: refused (exit 2, [`read_only_refusal`]) when the open went read-only, which an editing open does silently
/// when another session holds the lock (ruling 2 of cycle 41: a writer never acts read-only). A repair open
/// ([`ew_api::StoreOptions::repair`]) that integrity findings made read-only is let through: `ew fmt --repair` names a
/// crashed session's journal first and then the finding (WP-S1.17, decision log ID-1185 rulings 3 and 4).
pub(super) fn open_editing(
    dir: &Path,
    origin: ew_api::Origin,
    store: ew_api::StoreOptions,
    recovery: ew_api::Recovery,
) -> Result<(ew_api::Session, ew_api::OpenReport), i32> {
    open_editing_from(dir, origin, store, recovery, None)
}

/// [`open_editing`] with the session's library tiers (`None`: the compiled-in libraries only).
pub(super) fn open_editing_from(
    dir: &Path,
    origin: ew_api::Origin,
    store: ew_api::StoreOptions,
    recovery: ew_api::Recovery,
    libraries: Option<ew_api::library::LibraryTiers>,
) -> Result<(ew_api::Session, ew_api::OpenReport), i32> {
    open_editing_to(dir, origin, store, recovery, libraries, &mut std::io::stderr())
}

/// [`open_editing_from`] with its refusals written to `errors` (`ew recover`'s injected writer, FT-1394).
pub(super) fn open_editing_to(
    dir: &Path,
    origin: ew_api::Origin,
    store: ew_api::StoreOptions,
    recovery: ew_api::Recovery,
    libraries: Option<ew_api::library::LibraryTiers>,
    errors: &mut dyn std::io::Write,
) -> Result<(ew_api::Session, ew_api::OpenReport), i32> {
    let repair = store.repair;
    let options = ew_api::SessionOptions {
        origin,
        libraries,
        ..ew_api::SessionOptions::default()
    };
    let (session, opened) = match ew_api::Session::open(dir, store, options, recovery) {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(errors, "error: {}: {e}", dir.display());
            return Err(2);
        }
    };
    match &opened.access {
        ew_api::Access::ReadOnly(ew_api::ReadOnlyReason::Integrity) if repair => {}
        ew_api::Access::ReadOnly(reason) => {
            // FT-1383 (ID-1418): a damaged library copy a device uses is named, with the way out.
            let refusal = match (reason, session.damaged_copy()) {
                (
                    ew_api::ReadOnlyReason::Integrity,
                    Some(ew_api::DamagedCopy {
                        device: Some(device),
                        part,
                        ..
                    }),
                ) => {
                    let copy = match part {
                        Some((part, library)) => format!("the library copy of part {part} from {library}"),
                        None => "the library copy of its part".to_owned(),
                    };
                    format!(
                        "error: {}: the project opens read-only: the device {device} uses {copy}, which is damaged; \
                         update the copy from its library first; nothing was written",
                        dir.display()
                    )
                }
                // FT-1500 (ID-1458): a copy no device names, by its own words and the record that names it.
                (ew_api::ReadOnlyReason::Integrity, Some(damaged)) => format!(
                    "error: {}: the project opens read-only: {} is damaged; update the copy from its library first; \
                     nothing was written",
                    dir.display(),
                    damaged.words
                ),
                _ => read_only_refusal(dir, reason),
            };
            let _ = writeln!(errors, "{refusal}");
            session.close();
            return Err(2);
        }
        ew_api::Access::Edit => {}
    }
    // FT-1503 (ID-1458): an image kept but not drawn is named as the window's open banner names it.
    for notice in undrawn_notices(&session) {
        let _ = writeln!(errors, "notice: {notice}");
    }
    Ok((session, opened))
}

/// The refusal of every writer (`apply`, `fmt`, `recover`, `import parts`, the human-only subcommands) whose open went
/// read-only (ruling 2 of cycle 41: `ew recover` refuses a held lock with `ew apply`'s text). The reason is in words,
/// those the window uses, with no session ID, process ID or `@` (FT-877; the ruling WP-S1.22 §10.2 rule 3 needs for a
/// changed text is ruling 1 of the cycle-42 FT batch).
pub(super) fn read_only_refusal(dir: &Path, reason: &ew_api::ReadOnlyReason) -> String {
    format!(
        "error: {}: the project opens read-only: {}; nothing was written",
        dir.display(),
        read_only_words(reason)
    )
}

/// A read-only reason in words. Text from the lock file and the system passes through [`printable`].
pub(super) fn read_only_words(reason: &ew_api::ReadOnlyReason) -> String {
    use ew_api::ReadOnlyReason as R;
    match reason {
        R::Requested => "it was opened read-only on request".to_owned(),
        R::LockedBy(info) => match &info.record {
            Some(r) => format!(
                "another session holds the project ({} on {}, since {})",
                printable(&r.user),
                printable(&r.host),
                printable(&r.started_at)
            ),
            None => "another session holds the project".to_owned(),
        },
        R::NotWritable(message) => format!("the project folder cannot be written ({})", printable(message)),
        R::Integrity => "it has integrity findings; `ew check` lists them".to_owned(),
        R::Format(why) => format!("it was {why}"),
        R::LockLost(_) => "the lock was taken over".to_owned(),
    }
}

/// Why a writer other than `ew upgrade` and `ew recover` does not write a project of an older format, which opens
/// migrated in memory (WP-S1.17 D11, decision log ID-1312): said before the unsaved-changes check and the save's
/// consent gate, naming the writer that converts it. `ew fmt --check` says it too.
pub(super) fn older_format(dir: &Path, from: &str) -> String {
    format!(
        "{}: the project is in the older format {from}; convert it with `ew upgrade <project>` first; nothing was \
         written",
        dir.display()
    )
}

/// The refusal of every writer (`apply`, `fmt`, `import parts`, the human-only subcommands) while a crashed session's
/// journal holds unsaved changes, which a new save would hide (doc 05 section 8.3). It names the command that replays
/// or discards them, `ew recover`, and no follow-up number (cycle 41 review, ruling 1 of its fix round).
pub(super) fn journal_refusal(dir: &Path) -> String {
    format!(
        "error: {}: a crashed session left unsaved changes in its journal; replay or discard them first with `ew \
         recover`; nothing was written",
        dir.display()
    )
}

/// `text` with its control characters and the Unicode bidirectional controls escaped: text from a project or state
/// file (a journal's command name, a finding about a file's content, a lock record's user) never reaches the terminal
/// as a control sequence, nor reorders the line it prints in (U+061C, U+200E, U+200F, U+202A to U+202E and U+2066 to
/// U+2069 print as `\u{…}`).
pub(super) fn printable(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() {
                c.escape_default().to_string()
            } else if is_bidi_control(c) {
                c.escape_unicode().to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// Whether `c` is a Unicode bidirectional control (the Bidi_Control property).
pub(super) fn is_bidi_control(c: char) -> bool {
    matches!(
        c,
        '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
    )
}
