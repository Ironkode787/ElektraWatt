// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The steps of a run and the validity oracle each command step carries.
//!
//! The command decides whether a step is accepted; the fuzzer asserts invariants. A command step says
//! whether the arm built it to be invalid (a [`Verdict`]) and what must hold afterwards when it committed (the
//! [`Post`] checks):
//!
//! - [`Verdict::Refuse`]: the arm built the step to be invalid in a way no command rule decides: an ID that names
//!   no record (a fresh UUID, a deleted record) or a record of another kind (the generic `tx.require` refuses
//!   both), or parameters that fail the registry's schema check or parse. An `Ok` is a finding.
//! - [`Verdict::Either`]: every other step. Whether a command's own rules accept it is the command's to decide;
//!   a verdict that restated a rule would be a second copy of it.
//!
//! Whatever the verdict, a refused step changes nothing (the revision does not move and the model equals the
//! model before), the refusal is of a kind a user can act on (`CmdError::Internal` is a bug), a committed step's
//! primary record exists afterwards, and a committed, undoable step obeys the undo law.

use ew_derive::Q;
use ew_schema::Kind;
use serde_json::{Value, json};

/// Whether the arm built a step to be invalid (see the module).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Verdict {
    /// Invalid in a way no command rule decides: the model must refuse it.
    Refuse,
    /// The command decides.
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
    LogicalNets,
    PhysicalNets,
    Forms,
    Symbols,
    Fndefs,
    Parts,
    Revisions,
    PageItems,
    StructureBoxes,
    Waivers,
    LayoutSpaces,
    Enclosures,
    Surfaces,
    Carriers,
    Placements,
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
            Table::LogicalNets => q.table::<ew_schema::LogicalNet>().len(),
            Table::PhysicalNets => q.table::<ew_schema::PhysicalNet>().len(),
            Table::Forms => q.forms().len(),
            Table::Symbols => q.symbols().len(),
            Table::Fndefs => q.fndefs().len(),
            Table::Parts => q.parts().len(),
            Table::Revisions => q.table::<ew_schema::Revision>().len(),
            Table::PageItems => q.page_items().len(),
            Table::StructureBoxes => q.structure_boxes().len(),
            Table::Waivers => q.table::<ew_schema::Waiver>().len(),
            Table::LayoutSpaces => q.table::<ew_schema::LayoutSpace>().len(),
            Table::Enclosures => q.table::<ew_schema::Enclosure>().len(),
            Table::Surfaces => q.table::<ew_schema::MountingSurface>().len(),
            Table::Carriers => q.table::<ew_schema::Carrier>().len(),
            Table::Placements => q.table::<ew_schema::Placement>().len(),
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
    /// Where the arm gave a [`Verdict::Refuse`] (`#[track_caller]`), for the failure text of a refusal that is not
    /// generic.
    pub site: Option<&'static std::panic::Location<'static>>,
}

impl Step {
    /// An unchecked command step ([`Verdict::Either`], no checks).
    pub fn cmd(name: &'static str, params: Value) -> Step {
        Step::Cmd(CmdStep {
            name,
            params,
            verdict: Verdict::Either,
            posts: Vec::new(),
            site: None,
        })
    }

    pub fn verdict(self, verdict: Verdict) -> Step {
        match self {
            Step::Cmd(c) => Step::Cmd(CmdStep { verdict, ..c }),
            other => other,
        }
    }

    /// Marks the step as invalid in a way no command rule decides (see the module); the call site is kept for the
    /// failure text.
    #[track_caller]
    pub fn refuse(self) -> Step {
        let site = std::panic::Location::caller();
        match self.verdict(Verdict::Refuse) {
            Step::Cmd(c) => Step::Cmd(CmdStep {
                site: Some(site),
                ..c
            }),
            other => other,
        }
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
        Kind::LogicalNet => q.table::<ew_schema::LogicalNet>().contains(id.cast()),
        Kind::PhysicalNet => q.table::<ew_schema::PhysicalNet>().contains(id.cast()),
        Kind::PlcConfigProject => q.table::<ew_schema::PlcConfigProject>().contains(id.cast()),
        Kind::Channel => q.table::<ew_schema::Channel>().contains(id.cast()),
        Kind::Part => q.parts().contains(id.cast()),
        Kind::Symbol => q.symbols().contains(id.cast()),
        Kind::Fndef => q.fndefs().contains(id.cast()),
        Kind::Form => q.forms().contains(id.cast()),
        Kind::Revision => q.table::<ew_schema::Revision>().contains(id.cast()),
        Kind::PageItem => q.page_items().contains(id.cast()),
        Kind::StructureBox => q.structure_boxes().contains(id.cast()),
        Kind::LayoutSpace => q.table::<ew_schema::LayoutSpace>().contains(id.cast()),
        Kind::Enclosure => q.table::<ew_schema::Enclosure>().contains(id.cast()),
        Kind::MountingSurface => q.table::<ew_schema::MountingSurface>().contains(id.cast()),
        Kind::Carrier => q.table::<ew_schema::Carrier>().contains(id.cast()),
        Kind::Placement => q.table::<ew_schema::Placement>().contains(id.cast()),
        // A kind no command creates (the project row, the reserved kinds): nothing to say.
        _ => true,
    }
}
