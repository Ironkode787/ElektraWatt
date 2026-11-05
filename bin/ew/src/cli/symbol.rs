// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Project-independent symbol authoring: bounded input, structured validation, ordinary library publication.

use ew_api::symbols::{self, SymbolDraft};
use std::path::PathBuf;

#[derive(Clone, PartialEq, Eq, Debug, clap::Subcommand)]
pub enum SymbolArgs {
    /// Print a new portable draft as JSON. Coordinates in files are integer nanometres.
    Template { key: String, name: String },
    /// Print the authoring JSON Schema.
    Schema,
    /// Print validation issues as JSON; exit 1 for an invalid draft, 2 for unreadable input.
    Validate { file: PathBuf },
    /// Print an array of editable drafts from QET .elmt, EPLAN EDZ/PXF/parts XML, or portable JSON.
    Import {
        file: PathBuf,
        #[arg(long, default_value = "user.import")]
        key: String,
    },
    /// Publish a validated draft to a writable .ewlib; create the library when absent.
    Publish {
        file: PathBuf,
        #[arg(long)]
        library: PathBuf,
    },
}

pub(super) fn run(args: SymbolArgs) -> i32 {
    match execute(args) {
        Ok((value, code)) => {
            println!("{}", serde_json::to_string_pretty(&value).expect("JSON value"));
            code
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}

fn execute(args: SymbolArgs) -> Result<(serde_json::Value, i32), Box<dyn std::error::Error>> {
    use serde_json::json;
    Ok(match args {
        SymbolArgs::Template { key, name } => (serde_json::to_value(SymbolDraft::new(&key, &name)?)?, 0),
        SymbolArgs::Schema => (symbols::json_schema(), 0),
        SymbolArgs::Validate { file } => {
            let d = SymbolDraft::parse(&symbols::read_file(&file)?)?;
            let issues = d.validate();
            let code = i32::from(!issues.is_empty());
            (
                json!({"format":"ew.symbol-validation/1","valid":issues.is_empty(),"issues":issues}),
                code,
            )
        }
        SymbolArgs::Import { file, key } => (serde_json::to_value(symbols::open(&file, &key)?)?, 0),
        SymbolArgs::Publish { file, library } => {
            let d = SymbolDraft::parse(&symbols::read_file(&file)?)?;
            let issues = d.validate();
            if !issues.is_empty() {
                return Ok((
                    json!({"format":"ew.symbol-validation/1","valid":false,"issues":issues}),
                    1,
                ));
            }
            let mut tiers = ew_api::library::LibraryTiers::for_user(vec![], vec![])
                .ok_or("No user library directory is configured")?;
            tiers.company.push(library.clone());
            let mut registry = ew_api::library::Registry::open(&tiers)?;
            d.check_function(&registry)?;
            let id = match registry
                .libraries()
                .into_iter()
                .find(|l| l.path.as_ref() == Some(&library))
            {
                Some(l) => l.id,
                None => registry.create(
                    &library,
                    "user-symbols",
                    "My symbols",
                    d.symbol.meta.licence.as_deref().unwrap_or(""),
                )?,
            };
            d.publish(&mut registry, id)?;
            (
                json!({"format":"ew.symbol-publication/1","key":d.key,"version":d.version}),
                0,
            )
        }
    })
}
