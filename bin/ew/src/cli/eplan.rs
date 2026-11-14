// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew import edz` and `ew import parts-xml` (doc 14 §5.2, §5.3, §13): the parts of an EDZ or an EPLAN parts XML into
//! a writable library, or the previewed update of a project's part copies from them.

use std::path::PathBuf;

use ew_api::eplan;

use super::{library::index_warning, library::open_libraries, open::project_dir};

/// The arguments of `ew import edz` and `ew import parts-xml`.
#[derive(clap::Args, Clone, PartialEq, Eq, Debug)]
pub struct ImportEplanPartsArgs {
    /// The EDZ (7-Zip or ZIP), the EPLAN parts XML or the part XML.
    #[arg(value_name = "file")]
    pub file: PathBuf,
    /// The writable library to write the parts into: its key or ID (`ew lib list`).
    #[arg(
        long,
        value_name = "library",
        allow_hyphen_values = true,
        overrides_with = "library",
        required_unless_present = "project",
        conflicts_with = "project"
    )]
    pub library: Option<String>,
    /// Update the part copies of this project from the file instead: prints what would change.
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        overrides_with = "project"
    )]
    pub project: Option<PathBuf>,
    /// With `--project`: commit the update as one undo step and save the project.
    #[arg(long, overrides_with = "apply", requires = "project")]
    pub apply: bool,
    /// The parts' import date, YYYY-MM-DD.
    #[arg(long, value_name = "date", allow_hyphen_values = true, overrides_with = "as_of", value_parser = super::tree::date)]
    pub as_of: Option<ew_api::catalog::Date>,
    /// With `--library`: write changed parts as the next patch version.
    #[arg(long, overrides_with = "bump", requires = "library")]
    pub bump: bool,
    /// With `--library`: take a stale library lock over.
    #[arg(long, overrides_with = "take_over", requires = "library")]
    pub take_over: bool,
    /// A company library to find the library and the manufacturers in (repeatable).
    #[arg(long, value_name = "dir", allow_hyphen_values = true)]
    pub company: Vec<PathBuf>,
    /// Your own function-definition rows (`fndef-map.toml`), over the shipped table.
    #[arg(
        long,
        value_name = "file",
        allow_hyphen_values = true,
        overrides_with = "fndef_map"
    )]
    pub fndef_map: Option<PathBuf>,
}

/// `ew import edz` (`edz`: the terms notice is shown first, EW-IMP-201) and `ew import parts-xml`.
pub(super) fn run_import_eplan_parts(a: &ImportEplanPartsArgs, edz: bool) -> i32 {
    let fndefs = match &a.fndef_map {
        Some(path) => match eplan::fndef_map_file(path) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("error: {e}");
                return 2;
            }
        },
        None => eplan::FndefMap::shipped(),
    };
    let input = match ew_api::catalog::read_input(&a.file, &ew_api::catalog::Limits::default()) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    if edz {
        eprintln!("{}", eplan::TERMS_NOTICE);
    }
    match &a.library {
        Some(library) => into_library(a, library, &input, &fndefs, edz),
        None => update_project(a, &input, &fndefs),
    }
}

/// The parts into the library `library`.
fn into_library(
    a: &ImportEplanPartsArgs,
    library: &str,
    input: &ew_api::catalog::ImportInput,
    fndefs: &eplan::FndefMap,
    edz: bool,
) -> i32 {
    let mut reg = match open_libraries(&a.company) {
        Ok(r) => r,
        Err(code) => return code,
    };
    let Some((id, key)) = reg
        .libraries()
        .into_iter()
        .find(|l| l.key == library || l.id.to_string() == library)
        .map(|l| (l.id, l.key))
    else {
        eprintln!("error: no library `{library}` (see `ew lib list`)");
        return 2;
    };
    match eplan::import_into_library(&mut reg, id, input, fndefs, (a.as_of, a.bump, a.take_over), edz) {
        Ok(done) => {
            super::print_catalog_rows(&done.import);
            let written = done.write.as_ref().map_or_else(
                || "nothing to write".to_owned(),
                |w| {
                    format!(
                        "written: {} added, {} updated, {} unchanged (generation {})",
                        w.added.len(),
                        w.updated.len(),
                        w.unchanged.len(),
                        w.generation
                    )
                },
            );
            println!(
                "imported {} into {key}: {}; {written}",
                input.name(),
                super::counts_text(&done.import)
            );
            if let Some(w) = done.write.as_ref().and_then(index_warning) {
                eprintln!("{w}");
            }
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            if matches!(
                e,
                ew_api::catalog::CatalogError::Library(ew_api::library::LibraryError::Stale { .. })
            ) {
                eprintln!("note: pass `--take-over` to take the stale lock over");
            }
            2
        }
    }
}

/// The part copies of the project, updated from the file: previewed, and with `--apply` committed and saved.
fn update_project(
    a: &ImportEplanPartsArgs,
    input: &ew_api::catalog::ImportInput,
    fndefs: &eplan::FndefMap,
) -> i32 {
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    let origin = ew_api::Origin::Cli {
        subcommand: "import edz".into(),
        confirmed: false,
    };
    let opened = if a.apply {
        super::open_for_writing(&dir, origin.clone())
    } else {
        ew_api::Session::open_read_only(&dir, env!("CARGO_PKG_VERSION"))
            .map(|(s, _)| s)
            .map_err(|e| {
                eprintln!("error: {}: {e}", dir.display());
                2
            })
    };
    let Ok(mut session) = opened else {
        return 2;
    };
    let code = update_session(a, &mut session, input, fndefs, origin);
    session.close();
    code
}

fn update_session(
    a: &ImportEplanPartsArgs,
    session: &mut ew_api::Session,
    input: &ew_api::catalog::ImportInput,
    fndefs: &eplan::FndefMap,
    origin: ew_api::Origin,
) -> i32 {
    let import = match session.eplan_parts(input, fndefs, a.as_of) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    super::print_catalog_rows(&import);
    let updates = match session.update_parts_from_import(&import) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    if let Some(plan) = &updates.plan {
        for c in &plan.copies {
            let fields: Vec<&str> = c.fields.iter().map(|f| f.member.as_str()).collect();
            println!(
                "copy {}: version {} -> {} ({:?}): changes {}",
                c.copy,
                c.from,
                c.to,
                c.class,
                fields.join(", ")
            );
        }
        if !plan.devices.is_empty() {
            println!("devices using the changed copies: {}", plan.devices.len());
        }
    }
    for (what, list) in [
        ("not in the project, left out", &updates.unmatched),
        ("carried by several copies, left alone", &updates.ambiguous),
    ] {
        if !list.is_empty() {
            println!("{what}: {}", list.join(", "));
        }
    }
    let Some(plan) = updates.plan else {
        println!(
            "{} of the file's parts are in the project; nothing differs, nothing to update",
            updates.matched.len()
        );
        return 0;
    };
    let n = plan.copies.len();
    if !a.apply {
        println!("{n} part copies would change; nothing was written (give `--apply` to update them)");
        return 0;
    }
    let result = session
        .preview_with(&plan.command, ew_api::Submit::from(origin))
        .and_then(|preview| session.commit(preview));
    if let Err(e) = result {
        eprintln!("error: {e}");
        return 2;
    }
    if let Err(e) = session
        .save()
        .map(|r| super::revision::warn_unreleased(r.unstable, &mut std::io::stderr()))
    {
        eprintln!("error: the save failed: {e:?}");
        return 2;
    }
    println!("{n} part copies updated in one step; saved");
    0
}
