// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew merge resolve <project> --take ours|theirs [--kind <k>] [--field <path>]`: the conflict records a merge left,
//! resolved in one transaction of `merge.resolve` and saved.

use ew_api::{Conflict, ConflictBody};

use super::MergeResolveArgs;
use super::io::{Io, out, outln};
use super::save_failed;
use super::tree::{Leaf, TakeArg};
use crate::catalogue::{Exit, exit};

impl Leaf for MergeResolveArgs {
    const EXITS: &'static [Exit] = &[
        exit(0, "resolved and saved"),
        exit(
            1,
            "`merge.resolve` refused: theirs does not make a valid record, a file's lines do not read",
        ),
        exit(
            2,
            "usage (no conflict record selected), open, lock, a crashed session's journal and save errors",
        ),
    ];
    const RESULT: Option<&'static str> = Some(ew_api::cli_results::MergeResolveResult::SCHEMA);
}

/// Whether `conflict` is one `a` selects: its `kind` (a target's kind, or `file`) is `--kind`, and one of its members
/// is `--field`, or lies below it or above it (`overrides` and `overrides.colour` select each other).
fn selects(a: &MergeResolveArgs, conflict: &Conflict) -> bool {
    let kind = match &conflict.body {
        ConflictBody::Record { kind, .. } => kind.as_str(),
        ConflictBody::File { .. } => ew_api::FILE_CONFLICT_KIND,
    };
    let below = |outer: &str, inner: &str| {
        inner
            .strip_prefix(outer)
            .is_some_and(|rest| rest.starts_with('.'))
    };
    a.kind.as_deref().is_none_or(|k| k == kind)
        && a.field.as_deref().is_none_or(|f| {
            conflict
                .member_paths()
                .iter()
                .any(|p| p == f || below(f, p) || below(p, f))
        })
}

/// What `a` selected, in words, for the refusal when nothing matches.
fn selection(a: &MergeResolveArgs) -> String {
    match (&a.kind, &a.field) {
        (None, None) => "the project has no conflict records".to_owned(),
        (Some(k), None) => format!("no conflict record of the kind `{k}`"),
        (None, Some(f)) => format!("no conflict record on the member `{f}`"),
        (Some(k), Some(f)) => format!("no conflict record of the kind `{k}` on the member `{f}`"),
    }
}

/// `ew merge resolve`: exit 0 resolved and saved, 1 when `merge.resolve` refuses, 2 on usage (no conflict selected),
/// open, lock and save errors.
pub(super) fn run_merge_resolve(a: &MergeResolveArgs, io: &mut Io<'_>) -> i32 {
    let Some(dir) = super::project_dir(a.project.as_ref(), io) else {
        return 2;
    };
    let origin = ew_api::Origin::Cli {
        subcommand: "merge resolve".to_owned(),
        confirmed: false,
    };
    let mut session = match super::open_for_writing(&dir, origin, io) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let take = match a.take {
        TakeArg::Theirs => "theirs",
        TakeArg::Ours => "ours",
    };
    let choices: Vec<serde_json::Value> = session
        .query()
        .table::<Conflict>()
        .iter()
        .filter(|(_, c)| selects(a, c))
        .map(|(id, _)| serde_json::json!({"conflict": id.to_string(), "take": take}))
        .collect();
    if choices.is_empty() {
        io.error(format_args!("{}; nothing was changed", selection(a)));
        session.close();
        return 2;
    }
    let count = choices.len();
    let command = ew_api::AnyCommand {
        name: "merge.resolve".to_owned(),
        params: serde_json::json!({ "choices": choices }),
    };
    if let Err(e) = session.execute(&command) {
        io.error(e);
        session.close();
        return 1;
    }
    let code = match session
        .save()
        .inspect(|saved| super::warn_unreleased(saved.unstable, &mut *io.err))
    {
        Ok(_) if a.json => {
            let result = ew_api::cli_results::MergeResolveResult {
                schema: ew_api::cli_results::MergeResolveResult::SCHEMA.to_owned(),
                resolved: count,
                take: take.to_owned(),
            };
            out!(io, "{}", ew_api::cli_results::to_text(&result));
            0
        }
        Ok(_) => {
            let noun = if count == 1 { "conflict" } else { "conflicts" };
            outln!(io, "{count} {noun} resolved, {take} taken; saved");
            0
        }
        Err(e) => {
            io.error(save_failed(&e));
            2
        }
    };
    session.close();
    code
}
