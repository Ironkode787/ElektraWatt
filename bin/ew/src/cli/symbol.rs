// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Project-independent symbol authoring: bounded input, structured validation, ordinary library publication.

use ew_api::symbols::{self, SymbolDraft};
use std::path::PathBuf;

use super::io::{Env, Io, outln};
use super::tree::{Leaf, group};
use crate::catalogue::{Exit, OK, USAGE_ETC, exit};

group! {
#[derive(Clone, PartialEq, Eq, Debug, clap::Subcommand)]
pub enum SymbolArgs {
    /// Print a new portable draft as JSON. Coordinates in files are integer nanometres.
    Template(TemplateArgs),
    /// Print the authoring JSON Schema.
    Schema(SchemaArgs),
    /// Print validation issues as JSON; exit 1 for an invalid draft, 2 for unreadable input.
    Validate(ValidateArgs),
    /// Print an array of editable drafts from QET .elmt, EPLAN EDZ/PXF/parts XML, or portable JSON.
    Import(ImportArgs),
    /// Publish a validated draft to a writable .ewlib; create the library when absent. Print whether the symbol was
    /// added, updated or unchanged, and the draft's identity: keep it in the draft to publish later versions.
    Publish(PublishArgs),
}
}

/// `ew symbol template`.
#[derive(Clone, PartialEq, Eq, Debug, clap::Args)]
pub struct TemplateArgs {
    key: String,
    name: String,
}

impl Leaf for TemplateArgs {
    const EXITS: &'static [Exit] = &[OK, USAGE_ETC];
}

/// `ew symbol schema`.
#[derive(Clone, PartialEq, Eq, Debug, clap::Args)]
pub struct SchemaArgs {}

impl Leaf for SchemaArgs {
    const EXITS: &'static [Exit] = &[OK, USAGE_ETC];
}

/// `ew symbol validate`.
#[derive(Clone, PartialEq, Eq, Debug, clap::Args)]
pub struct ValidateArgs {
    file: PathBuf,
}

impl Leaf for ValidateArgs {
    const EXITS: &'static [Exit] = &[OK, exit(1, "invalid draft"), USAGE_ETC];
}

/// `ew symbol import`.
#[derive(Clone, PartialEq, Eq, Debug, clap::Args)]
pub struct ImportArgs {
    file: PathBuf,
    /// The drafts' key; default: the file's name.
    #[arg(long)]
    key: Option<String>,
}

impl Leaf for ImportArgs {
    const EXITS: &'static [Exit] = &[OK, USAGE_ETC];
}

/// `ew symbol publish`.
#[derive(Clone, PartialEq, Eq, Debug, clap::Args)]
pub struct PublishArgs {
    file: PathBuf,
    #[arg(long)]
    library: PathBuf,
}

impl Leaf for PublishArgs {
    const EXITS: &'static [Exit] = &[OK, exit(1, "invalid draft"), USAGE_ETC];
}

pub(super) fn run(args: SymbolArgs, io: &mut Io<'_>) -> i32 {
    match execute(args, io.env) {
        Ok((value, code)) => {
            outln!(
                io,
                "{}",
                serde_json::to_string_pretty(&value).expect("JSON value")
            );
            code
        }
        Err(e) => {
            io.error(e);
            2
        }
    }
}

fn execute(args: SymbolArgs, env: &Env) -> Result<(serde_json::Value, i32), Box<dyn std::error::Error>> {
    use serde_json::json;
    Ok(match args {
        SymbolArgs::Template(TemplateArgs { key, name }) => {
            (serde_json::to_value(SymbolDraft::new(&key, &name)?)?, 0)
        }
        SymbolArgs::Schema(SchemaArgs {}) => (symbols::json_schema(), 0),
        SymbolArgs::Validate(ValidateArgs { file }) => {
            let d = SymbolDraft::parse(&symbols::read_file(&file)?)?;
            let issues = d.validate();
            let code = i32::from(!issues.is_empty());
            (
                json!({"format":"ew.symbol-validation/1","valid":issues.is_empty(),"issues":issues}),
                code,
            )
        }
        SymbolArgs::Import(ImportArgs { file, key }) => {
            let key = key.unwrap_or_else(|| symbols::file_key(&file));
            (serde_json::to_value(symbols::open(&file, &key)?)?, 0)
        }
        SymbolArgs::Publish(PublishArgs { file, library }) => {
            let mut d = SymbolDraft::parse(&symbols::read_file(&file)?)?;
            let issues = d.validate();
            if !issues.is_empty() {
                return Ok((
                    json!({"format":"ew.symbol-validation/1","valid":false,"issues":issues}),
                    1,
                ));
            }
            let mut tiers = env
                .tiers(vec![], vec![])
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
            let report = d.publish(&mut registry, id)?;
            let symbol = d.identity.as_ref().map(|i| i.symbol.as_uuid());
            let result = if report.added.iter().any(|i| Some(*i) == symbol) {
                "added"
            } else if report.updated.iter().any(|i| Some(*i) == symbol) {
                "updated"
            } else {
                "unchanged"
            };
            (
                json!({"format":"ew.symbol-publication/1","key":d.key,"version":d.version,"result":result,
                    "identity":d.identity}),
                0,
            )
        }
    })
}
