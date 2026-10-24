// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The steps of a run and the validity oracle each command step carries.
//!
//! A command step says what the model must do with it (a [`Verdict`]) and what must hold afterwards when it
//! committed (the [`Post`] checks):
//!
//! - [`Verdict::Accept`]: a well-formed step. The model must commit it (an `Ok`, whether or not it changed
//!   anything); a refusal is a finding, because the generator built the step from the live records and the
//!   command's own rules.
//! - [`Verdict::Refuse`]: a step the model must refuse (a nonexistent record, an off-grid position, a
//!   duplicate key): an `Ok` is a finding.
//! - [`Verdict::Either`]: the outcome depends on rules the generator does not model (prevent rules, the
//!   structural invariants); both are fine.
//!
//! Whatever the verdict, a refused step changes nothing (the revision does not move and the model equals the
//! model before), the refusal is of a kind a user can act on ([`CmdError::Internal`] is a bug), and a
//! committed step's primary record exists afterwards.

use ew_derive::Q;
use ew_schema::Kind;
use serde_json::{Value, json};

/// What the model must do with a step.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Verdict {
    Accept,
    Refuse,
    Either,
}

/// A table whose live-record count a check compares before and after a step.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Table {
    Pages,
    Sections,
    Nodes,
    ExternalRefs,
    Devices,
    Functions,
    Reps,
    Connections,
    Conductors,
    PotentialDefs,
    Bonds,
    ReportBlocks,
    DeviceLinks,
    PlcConfigProjects,
    Forms,
    Symbols,
    Fndefs,
    Parts,
    Revisions,
    PageItems,
    StructureBoxes,
}

impl Table {
    pub fn count(self, q: &Q) -> usize {
        match self {
            Table::Pages => q.pages().len(),
            Table::Sections => q.sections().len(),
            Table::Nodes => q.structure_nodes().len(),
            Table::ExternalRefs => q.external_refs().len(),
            Table::Devices => q.devices().len(),
            Table::Functions => q.functions().len(),
            Table::Reps => q.reps().len(),
            Table::Connections => q.connections().len(),
            Table::Conductors => q.conductors().len(),
            Table::PotentialDefs => q.potential_defs().len(),
            Table::Bonds => q.bonds().len(),
            Table::ReportBlocks => q.report_blocks().len(),
            Table::DeviceLinks => q.table::<ew_schema::DeviceLink>().len(),
            Table::PlcConfigProjects => q.table::<ew_schema::PlcConfigProject>().len(),
            Table::Forms => q.forms().len(),
            Table::Symbols => q.symbols().len(),
            Table::Fndefs => q.fndefs().len(),
            Table::Parts => q.parts().len(),
            Table::Revisions => q.table::<ew_schema::Revision>().len(),
            Table::PageItems => q.page_items().len(),
            Table::StructureBoxes => q.structure_boxes().len(),
        }
    }
}

/// A check function over the snapshots before and after a step.
pub type CheckFn = Box<dyn Fn(&Q, &Q) -> Result<(), String>>;

/// A check over the model before and after a committed step.
pub enum Post {
    /// The counts of live records change by exactly these deltas (a table not listed is not compared).
    Counts(Vec<(Table, isize)>),
    /// Any other property; the message is the finding.
    Check(CheckFn),
}

/// One step of a fuzz run.
pub enum Step {
    Undo,
    Redo,
    /// A save, a reload from disk and the comparison of the model and the derived families
    /// ([`super::roundtrip`]).
    SaveReload,
    /// A CSV table import, strict and lenient, over (possibly damaged) golden tables ([`super::import`]).
    Import(super::import::Damage),
    Cmd(CmdStep),
}

/// A command step.
pub struct CmdStep {
    pub name: &'static str,
    pub params: Value,
    pub verdict: Verdict,
    pub posts: Vec<Post>,
}

impl Step {
    /// An unchecked command step ([`Verdict::Either`], no checks).
    pub fn cmd(name: &'static str, params: Value) -> Step {
        Step::Cmd(CmdStep {
            name,
            params,
            verdict: Verdict::Either,
            posts: Vec::new(),
        })
    }

    pub fn verdict(self, verdict: Verdict) -> Step {
        match self {
            Step::Cmd(c) => Step::Cmd(CmdStep { verdict, ..c }),
            other => other,
        }
    }

    pub fn accept(self) -> Step {
        self.verdict(Verdict::Accept)
    }

    pub fn refuse(self) -> Step {
        self.verdict(Verdict::Refuse)
    }

    /// Adds a count check.
    pub fn counts(self, deltas: &[(Table, isize)]) -> Step {
        self.post(Post::Counts(deltas.to_vec()))
    }

    /// Adds a before/after check.
    pub fn check(self, f: impl Fn(&Q, &Q) -> Result<(), String> + 'static) -> Step {
        self.post(Post::Check(Box::new(f)))
    }

    fn post(self, post: Post) -> Step {
        match self {
            Step::Cmd(mut c) => {
                c.posts.push(post);
                Step::Cmd(c)
            }
            other => other,
        }
    }

    /// The command-log line of the step; the diagnostic steps are marker lines of the form `{"cmd":"#…"}`.
    pub fn line(&self) -> String {
        match self {
            Step::Undo => r##"{"cmd":"#undo"}"##.to_owned(),
            Step::Redo => r##"{"cmd":"#redo"}"##.to_owned(),
            Step::SaveReload => r##"{"cmd":"#save-reload"}"##.to_owned(),
            Step::Import(d) => json!({"cmd": "#import", "params": d.describe()}).to_string(),
            Step::Cmd(c) => json!({"cmd": c.name, "params": c.params}).to_string(),
        }
    }
}

/// Parses a JSON ID into a typed ID.
pub fn typed<T>(v: &Value) -> Option<ew_ids::Id<T>> {
    serde_json::from_value(v.clone()).ok()
}

/// Whether the record `key` exists in the snapshot `q` sees.
pub fn exists(q: &Q, key: (Kind, ew_schema::AnyId)) -> bool {
    let (kind, id) = key;
    match kind {
        Kind::StructureNode => q.structure_nodes().contains(id.cast()),
        Kind::Section => q.sections().contains(id.cast()),
        Kind::ExternalRef => q.external_refs().contains(id.cast()),
        Kind::Page => q.pages().contains(id.cast()),
        Kind::Device => q.devices().contains(id.cast()),
        Kind::Function => q.functions().contains(id.cast()),
        Kind::Representation => q.reps().contains(id.cast()),
        Kind::Connection => q.connections().contains(id.cast()),
        Kind::Junction => q.junctions().contains(id.cast()),
        Kind::Conductor => q.conductors().contains(id.cast()),
        Kind::PotentialDef => q.potential_defs().contains(id.cast()),
        Kind::Bond => q.bonds().contains(id.cast()),
        Kind::ReportBlock => q.report_blocks().contains(id.cast()),
        Kind::DeviceLink => q.table::<ew_schema::DeviceLink>().contains(id.cast()),
        Kind::PlcConfigProject => q.table::<ew_schema::PlcConfigProject>().contains(id.cast()),
        Kind::Channel => q.table::<ew_schema::Channel>().contains(id.cast()),
        Kind::Part => q.parts().contains(id.cast()),
        Kind::Symbol => q.symbols().contains(id.cast()),
        Kind::Fndef => q.fndefs().contains(id.cast()),
        Kind::Form => q.forms().contains(id.cast()),
        Kind::Revision => q.table::<ew_schema::Revision>().contains(id.cast()),
        Kind::PageItem => q.page_items().contains(id.cast()),
        Kind::StructureBox => q.structure_boxes().contains(id.cast()),
        // A kind no command creates (the project row, the reserved kinds): nothing to say.
        _ => true,
    }
}
