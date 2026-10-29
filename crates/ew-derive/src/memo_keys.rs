// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The inputs of the memo engine and the page-level keys of push invalidation (ADR-0002 amendment 4,
//! decision log ID-220 and ID-226; ARCH §6.2 "Additions after G-SP").
//!
//! # Two kinds of keys
//!
//! - **Verification keys.** A memo records what its query read exactly as `Q`'s tracked accessors report it:
//!   [`MemoInput::Read`] wraps the [`DepKey`] of `tracker.rs`, and [`stamp`] gives its revision stamp in a
//!   snapshot (the verifier contract of `tracker.rs`). A verification walk compares these stamps.
//! - **Registration keys** (coarse). The reverse-dependency index of `ew-memo` registers every read under the
//!   coarse key [`coarse`] gives in the snapshot the read observed: the page a row is drawn on
//!   ([`MemoInput::Page`]), an ID shard for rows without a page ([`MemoInput::Shard`]), an index-key shard for
//!   index buckets not keyed by page ([`MemoInput::IndexShard`]), and the read itself for whole tables,
//!   whole indexes, the project row and the reserved records. With row-level keys the index would hold about
//!   56 MB at XL (ID-226); page-level keys are required.
//!
//! # The contract between registration and touch
//!
//! A commit from `before` to `after` that put the rows `keys` reports [`touched`] inputs. They must include
//! the coarse key, **in the snapshot it was read from**, of every read whose content the commit changed
//! (`ew_memo::Db`: a stamp that changed with equal content need not be reported). Since a memo read a row or a
//! bucket in some earlier snapshot and the row was not changed in between, that snapshot agrees with `before`
//! on the row; so `touched` covers, for every put row, its keys in `before` and in `after`: the row's page or
//! shard, the coarse key of every index bucket the row feeds, the whole indexes it feeds, its table, and the
//! project row when the row is the project. The reserved records carry no stamps (their stamp is the model
//! revision, which every commit changes); they are touched only when they differ between `before` and
//! `after`, so a commit does not re-mark every memo that reads them (review cycle 9, finding 4, when the
//! title block still read the revisions as reserved records; they are typed since FT-165). A unit test
//! checks the contract against `ew-model`'s own stamps: every row, bucket, index and table whose stamp a
//! random commit changed is covered, and the reserved records are covered whenever they changed.

use ew_ids::Id;
use ew_memo::Durability;
use ew_model::{AnyKey, AnyRow, EndpointKey, Entity, IndexName, Model, Rev, RevIndex, Row};
use ew_schema::{
    AnyId, Bond, Conductor, Connection, Device, Endpoint, ExternalRef, FnDef, FormDef, Function,
    ImportLineage, Junction, Kind, LibCopy, Page, PageItem, Part, PotentialDef, ReportBlock, Representation,
    Section, StructureBox, StructureNode, SymbolDef,
};

use crate::potentials::PotentialId;
use crate::tracker::{DepKey, IxKey};

/// The number of ID shards per kind or index (a power of two).
pub(crate) const SHARDS: u16 = 4096;

/// A part of a producer's value that a query reads on its own (`ew_memo` part dependencies): the reader is
/// invalidated only when the producer reports that part changed, not on every change of the producer.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum PartKey {
    /// The topology of one potential in `potential_partition()`.
    Potential(PotentialId),
    /// The potential an endpoint belongs to in `potential_partition()`.
    Endpoint(EndpointKey),
    /// The flow entry of one potential in `potential_flow_bfs()`.
    Flow(PotentialId),
    /// The bond records in `potential_partition()` (`Partition::bonds`): which bonds exist, with their kind,
    /// via connection and path. Not the potentials at their ends: see [`PartKey::BondsOf`].
    Bonds,
    /// The bonds with an end on one potential in `potential_partition()`: a topology edit that changes the
    /// potential at a bond end declares the potentials before and after, not every bond reader.
    BondsOf(PotentialId),
    /// The pairing of one interruption point in `ip_pairing()`: its info (without the group index), its
    /// partners and the content of its group.
    Ip(Id<Function>),
    /// The whole value of a family read as a cut-off point ([`Q::get_value_part`](crate::Q)): a commit that
    /// marks the family does not mark the reader; only a re-execution that changes the value does. The
    /// family never declares parts, so every change of its value is a change of this part.
    Value,
    /// The derivation notes of a producer's value. A part reader replays the producer's notes, so every part
    /// read also depends on this part, and a producer that declares the parts it changed declares `Notes`
    /// when its notes changed (`memo_store`).
    Notes,
}

/// An input of the memo engine: the `ew_memo::Db::Input` of [`Q`](crate::Q).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum MemoInput {
    /// A tracked read, as `Q`'s accessors report it (a verification key).
    Read(DepKey),
    /// Registration key: the rows drawn on a page (representations, connections of the page, junctions) and
    /// the page-keyed index buckets (`reps_by_page`, `connections_by_page`, `junctions_by_page`).
    Page(Id<Page>),
    /// Registration key: the rows of a kind whose ID falls into a shard (rows without a page, absent rows).
    Shard(Kind, u16),
    /// Registration key: the buckets of an index not keyed by page whose key falls into a shard. An endpoint
    /// key `Cp(f, idx)` shards by `f`, so all CPs of a function share one shard.
    IndexShard(IndexName, u16),
    /// A part of a producer's value.
    Part(PartKey),
}

/// The shard of an ID: its 128 bits folded by XOR into 12 bits (deterministic; UUIDv7 IDs keep their random
/// low bits, so the fold spreads them).
pub(crate) fn shard(id: AnyId) -> u16 {
    let bits = id.as_uuid().as_u128();
    // 128 -> 64 -> 32 -> 16 bits, then the top nibble onto the low 12 so that every bit contributes.
    let b64 = (bits >> 64) as u64 ^ bits as u64;
    let b32 = (b64 >> 32) as u32 ^ b64 as u32;
    let b16 = (b32 >> 16) as u16 ^ b32 as u16;
    (b16 ^ (b16 >> 12)) & (SHARDS - 1)
}

/// Calls the generic function `$f::<T>(args)` for the entity type `T` of `$kind`, or evaluates `$other` for a
/// kind without a table (the project row, reserved kinds).
macro_rules! by_kind {
    ($kind:expr, $f:ident ( $($arg:expr),* ), $other:expr) => {
        match $kind {
            Kind::PlcConfigProject => $f::<ew_schema::PlcConfigProject>($($arg),*),
            Kind::ImportLineage => $f::<ImportLineage>($($arg),*),
            Kind::StructureNode => $f::<StructureNode>($($arg),*),
            Kind::Section => $f::<Section>($($arg),*),
            Kind::ExternalRef => $f::<ExternalRef>($($arg),*),
            Kind::Page => $f::<Page>($($arg),*),
            Kind::Device => $f::<Device>($($arg),*),
            Kind::Function => $f::<Function>($($arg),*),
            Kind::Representation => $f::<Representation>($($arg),*),
            Kind::Connection => $f::<Connection>($($arg),*),
            Kind::Junction => $f::<Junction>($($arg),*),
            Kind::Conductor => $f::<Conductor>($($arg),*),
            Kind::Channel => $f::<ew_schema::Channel>($($arg),*),
            Kind::PotentialDef => $f::<PotentialDef>($($arg),*),
            Kind::Bond => $f::<Bond>($($arg),*),
            Kind::DeviceLink => $f::<ew_schema::DeviceLink>($($arg),*),
            Kind::PageItem => $f::<PageItem>($($arg),*),
            Kind::StructureBox => $f::<StructureBox>($($arg),*),
            Kind::ReportBlock => $f::<ReportBlock>($($arg),*),
            Kind::MacroInstance => $f::<ew_schema::MacroInstance>($($arg),*),
            Kind::Revision => $f::<ew_schema::Revision>($($arg),*),
            Kind::Part => $f::<LibCopy<Part>>($($arg),*),
            Kind::Symbol => $f::<LibCopy<SymbolDef>>($($arg),*),
            Kind::Fndef => $f::<LibCopy<FnDef>>($($arg),*),
            Kind::Form => $f::<LibCopy<FormDef>>($($arg),*),
            Kind::Macro => $f::<LibCopy<ew_schema::MacroDef>>($($arg),*),
            Kind::Manufacturer => $f::<LibCopy<ew_schema::Manufacturer>>($($arg),*),
            _ => $other,
        }
    };
}

/// The page a row is drawn on: a representation's, a connection's (when it has one), a junction's, a page item's,
/// a structure box's and a macro instance's anchor page.
fn page_of(row: &AnyRow) -> Option<Id<Page>> {
    match row {
        AnyRow::Representation(r) => Some(r.page),
        AnyRow::Connection(c) => c.page,
        AnyRow::Junction(j) => Some(j.page),
        AnyRow::PageItem(i) => Some(i.page),
        AnyRow::StructureBox(b) => Some(b.page),
        AnyRow::MacroInstance(m) => Some(m.page),
        _ => None,
    }
}

/// The ID whose shard registers an endpoint key: the function of a CP, the junction.
fn endpoint_shard_id(e: &EndpointKey) -> AnyId {
    match e {
        EndpointKey::Cp(f, _) => f.cast(),
        EndpointKey::Junction(j) => j.cast(),
    }
}

/// The coarse registration key of `input` in `model`, the snapshot the read observed (see the module
/// documentation). `Page`, `Shard`, `IndexShard` and `Part` inputs are their own coarse key.
///
/// - `Read(Row((kind, id)))`: `Page(p)` for a present representation (its `page`), a present connection
///   with `page: Some(p)`, a present junction, page item or structure box (its `page`); `Shard(kind, shard(id))`
///   otherwise (other kinds, page-less connections, absent rows).
/// - `Read(IndexKey(name, key))`: `Page(p)` for `RepsByPage`, `ConnectionsByPage`, `JunctionsByPage`,
///   `PageItemsByPage` and `StructureBoxesByPage`;
///   `IndexShard(name, s)` otherwise, where `s` is the shard of the key's ID, of the function of a `Cp`
///   endpoint, or of the junction of a `Junction` endpoint.
/// - `Read(Table(_))`, `Read(Index(_))`, `Read(Project)`, `Read(Assets)`, `Read(Reserved)`: themselves.
pub(crate) fn coarse(model: &Model, input: &MemoInput) -> MemoInput {
    match input {
        MemoInput::Read(DepKey::Row(key)) => match model.row(key).as_ref().and_then(page_of) {
            Some(page) => MemoInput::Page(page),
            None => MemoInput::Shard(key.0, shard(key.1)),
        },
        MemoInput::Read(DepKey::IndexKey(name, key)) => match (name, key) {
            (
                IndexName::RepsByPage
                | IndexName::ConnectionsByPage
                | IndexName::JunctionsByPage
                | IndexName::PageItemsByPage
                | IndexName::StructureBoxesByPage,
                IxKey::Id(id),
            ) => MemoInput::Page(id.cast()),
            (name, IxKey::Id(id)) => MemoInput::IndexShard(*name, shard(*id)),
            (name, IxKey::Endpoint(e)) => MemoInput::IndexShard(*name, shard(endpoint_shard_id(e))),
        },
        other => *other,
    }
}

fn row_stamp<T: Entity>(model: &Model, id: AnyId) -> Rev {
    let table = model.table::<T>();
    table
        .row(&id.cast())
        .map_or_else(|| table.changed_at(), Row::changed_at)
}

fn table_stamp<T: Entity>(model: &Model) -> Rev {
    model.table::<T>().changed_at()
}

fn bucket_stamp<K: Ord + Clone, V: Ord + Clone>(ix: &RevIndex<K, V>, key: &K) -> Rev {
    ix.key_changed_at(key).unwrap_or(ix.changed_at())
}

/// The stamp of a bucket, as `IdxView::bucket` observes it. A key shape no accessor produces is the
/// model's revision (conservative, like the reserved records).
fn index_key_stamp(model: &Model, name: IndexName, key: IxKey) -> Rev {
    let ix = model.indexes();
    match (name, key) {
        (IndexName::FunctionsByDevice, IxKey::Id(id)) => bucket_stamp(ix.functions_by_device(), &id.cast()),
        (IndexName::RepsByPage, IxKey::Id(id)) => bucket_stamp(ix.reps_by_page(), &id.cast()),
        (IndexName::RepsByFunction, IxKey::Id(id)) => bucket_stamp(ix.reps_by_function(), &id.cast()),
        (IndexName::ConnectionsByPage, IxKey::Id(id)) => bucket_stamp(ix.connections_by_page(), &id.cast()),
        (IndexName::ConnectionsByEndpoint, IxKey::Endpoint(e)) => {
            bucket_stamp(ix.connections_by_endpoint(), &e)
        }
        (IndexName::JunctionsByPage, IxKey::Id(id)) => bucket_stamp(ix.junctions_by_page(), &id.cast()),
        (IndexName::TerminalsByStrip, IxKey::Id(id)) => bucket_stamp(ix.terminals_by_strip(), &id.cast()),
        (IndexName::ConductorsByCable, IxKey::Id(id)) => bucket_stamp(ix.conductors_by_cable(), &id.cast()),
        (IndexName::StructureChildren, IxKey::Id(id)) => bucket_stamp(ix.structure_children(), &id.cast()),
        (IndexName::DeviceChildren, IxKey::Id(id)) => bucket_stamp(ix.device_children(), &id.cast()),
        (IndexName::PagesBySection, IxKey::Id(id)) => bucket_stamp(ix.pages_by_section(), &id.cast()),
        (IndexName::PotentialDefsByEndpoint, IxKey::Endpoint(e)) => {
            bucket_stamp(ix.potential_defs_by_endpoint(), &e)
        }
        (IndexName::BondsByEndpoint, IxKey::Endpoint(e)) => bucket_stamp(ix.bonds_by_endpoint(), &e),
        (IndexName::PageItemsByPage, IxKey::Id(id)) => bucket_stamp(ix.page_items_by_page(), &id.cast()),
        (IndexName::StructureBoxesByPage, IxKey::Id(id)) => {
            bucket_stamp(ix.structure_boxes_by_page(), &id.cast())
        }
        _ => model.rev(),
    }
}

/// The stamp of a whole index, as `Q` observes it for `DepKey::Index`.
fn index_stamp(model: &Model, name: IndexName) -> Rev {
    let ix = model.indexes();
    match name {
        IndexName::FunctionsByDevice => ix.functions_by_device().changed_at(),
        IndexName::RepsByPage => ix.reps_by_page().changed_at(),
        IndexName::RepsByFunction => ix.reps_by_function().changed_at(),
        IndexName::ConnectionsByPage => ix.connections_by_page().changed_at(),
        IndexName::ConnectionsByEndpoint => ix.connections_by_endpoint().changed_at(),
        IndexName::JunctionsByPage => ix.junctions_by_page().changed_at(),
        IndexName::TerminalsByStrip => ix.terminals_by_strip().changed_at(),
        IndexName::ConductorsByCable => ix.conductors_by_cable().changed_at(),
        IndexName::StructureChildren => ix.structure_children().changed_at(),
        IndexName::DeviceChildren => ix.device_children().changed_at(),
        IndexName::PagesBySection => ix.pages_by_section().changed_at(),
        IndexName::PotentialDefsByEndpoint => ix.potential_defs_by_endpoint().changed_at(),
        IndexName::BondsByEndpoint => ix.bonds_by_endpoint().changed_at(),
        IndexName::PageItemsByPage => ix.page_items_by_page().changed_at(),
        IndexName::StructureBoxesByPage => ix.structure_boxes_by_page().changed_at(),
    }
}

/// The revision stamp of a tracked read in `model`: exactly what `Q`'s accessor observes for it (the
/// verifier contract of `tracker.rs`): a row's `changed_at` (the table's for an absent row), a bucket's
/// `changed_at` (the index's for an absent key), the table's, the index's, the project row's; the number of
/// assets held for the asset store; the model's revision for the reserved records.
pub(crate) fn stamp(model: &Model, dep: &DepKey) -> Rev {
    match dep {
        // A kind without a table has no read in `Q`: the model's revision is the conservative answer.
        DepKey::Row((kind, id)) => by_kind!(*kind, row_stamp(model, *id), model.rev()),
        DepKey::Table(kind) => by_kind!(*kind, table_stamp(model), model.rev()),
        DepKey::IndexKey(name, key) => index_key_stamp(model, *name, *key),
        DepKey::Index(name) => index_stamp(model, *name),
        DepKey::Project => model.project_row().changed_at(),
        DepKey::Assets => model.assets_stamp(),
        DepKey::Reserved => model.rev(),
    }
}

/// Whether a kind is an embedded library copy.
fn is_library(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Part | Kind::Symbol | Kind::Fndef | Kind::Form | Kind::Macro | Kind::Manufacturer
    )
}

/// The durability of a kind's rows and table.
fn kind_durability(kind: Kind) -> Durability {
    if kind == Kind::Project {
        Durability::Settings
    } else if is_library(kind) {
        Durability::Library
    } else {
        Durability::Drawing
    }
}

/// The durability of an input (ADR-0002 decision 4): the project row is `Settings`; rows and tables of the
/// embedded library copies (`Part`, `Symbol`, `Fndef`, `Form`, `Macro`, `Manufacturer`) are `Library`; everything else, the
/// registration keys, the asset store and parts included, is `Drawing` (a drawing commit brings assets in).
pub(crate) fn durability(input: &MemoInput) -> Durability {
    match input {
        MemoInput::Read(DepKey::Project) => Durability::Settings,
        MemoInput::Read(DepKey::Row((kind, _)) | DepKey::Table(kind)) => kind_durability(*kind),
        _ => Durability::Drawing,
    }
}

/// The inputs a commit from `before` to `after` touched, and the durability to advance the store with.
///
/// `keys` are the row keys the commit put (`Change::touched_keys`; the project row is `(Kind::Project, id)`).
/// For every key: `Read(Row(key))`, `Read(Table(kind))` (or `Read(Project)` for the project row),
/// `Shard(kind, shard(id))`, and in `before` and in `after` (where the row exists): the row's `Page`, and for
/// every named base index the row feeds, the bucket's coarse key and `Read(Index(name))`. `Read(Reserved)`
/// is included when the reserved records of `before` and `after` differ (their stamp changes with every
/// commit, their content almost never: no command puts them), and `Read(Assets)` when `after` holds assets
/// that `before` does not (a transaction brings its assets in without a row of the readers that depend on
/// them). The list is sorted and deduplicated.
///
/// The durability is the most durable level among the keys (`Drawing` for an empty list): `Settings` for the
/// project row, `Library` for a library copy. A kind's level never changes, so it covers the input before and
/// after the change (ADR-0002 amendment 3).
pub(crate) fn touched(before: &Model, after: &Model, keys: &[AnyKey]) -> (Vec<MemoInput>, Durability) {
    let mut out = Vec::new();
    if reserved_changed(before, after) {
        out.push(MemoInput::Read(DepKey::Reserved));
    }
    if stamp(before, &DepKey::Assets) != stamp(after, &DepKey::Assets) {
        out.push(MemoInput::Read(DepKey::Assets));
    }
    let mut level = Durability::Drawing;
    for key in keys {
        let (kind, id) = *key;
        level = level.max(kind_durability(kind));
        if kind == Kind::Project {
            out.push(MemoInput::Read(DepKey::Project));
        } else {
            out.push(MemoInput::Read(DepKey::Row(*key)));
            out.push(MemoInput::Read(DepKey::Table(kind)));
            out.push(MemoInput::Shard(kind, shard(id)));
        }
        for model in [before, after] {
            let Some(row) = model.row(key) else { continue };
            out.extend(page_of(&row).map(MemoInput::Page));
            for (name, k) in index_keys(&row) {
                out.push(coarse(model, &MemoInput::Read(DepKey::IndexKey(name, k))));
                out.push(MemoInput::Read(DepKey::Index(name)));
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    (out, level)
}

/// Whether the reserved records (what `Q::reserved` reads) differ between two snapshots: the same slice
/// (the usual case, shared by every commit) or equal content is no change.
fn reserved_changed(before: &Model, after: &Model) -> bool {
    let (a, b) = (before.unknown_records(), after.unknown_records());
    !(std::ptr::eq(a, b) || a == b)
}

fn by_id<T>(out: &mut Vec<(IndexName, IxKey)>, name: IndexName, id: Id<T>) {
    out.push((name, IxKey::Id(id.cast())));
}

fn at(out: &mut Vec<(IndexName, IxKey)>, name: IndexName, e: &Endpoint) {
    if let Some(key) = EndpointKey::of(e) {
        out.push((name, IxKey::Endpoint(key)));
    }
}

/// The named index keys a row feeds (the hand mirror of `ew-model`'s `LINKS` with an `index`, checked by
/// the unit tests against `ew-model`'s own bucket stamps).
fn index_keys(row: &AnyRow) -> Vec<(IndexName, IxKey)> {
    let mut out = Vec::new();
    match row {
        AnyRow::StructureNode(n) => {
            if let Some(parent) = n.parent {
                by_id(&mut out, IndexName::StructureChildren, parent);
            }
        }
        AnyRow::Page(p) => {
            if let Some(section) = p.folio.section {
                by_id(&mut out, IndexName::PagesBySection, section);
            }
        }
        AnyRow::Device(d) => {
            if let Some(parent) = d.parent {
                by_id(&mut out, IndexName::DeviceChildren, parent);
            }
            if let Some(terminal) = &d.aspects.terminal {
                by_id(&mut out, IndexName::TerminalsByStrip, terminal.strip);
            }
        }
        AnyRow::Function(f) => {
            if let Some(device) = f.device {
                by_id(&mut out, IndexName::FunctionsByDevice, device);
            }
        }
        AnyRow::Representation(r) => {
            for f in &r.functions {
                by_id(&mut out, IndexName::RepsByFunction, *f);
            }
            for entry in &r.pin_map {
                by_id(&mut out, IndexName::RepsByFunction, entry.function);
            }
            by_id(&mut out, IndexName::RepsByPage, r.page);
        }
        AnyRow::Connection(c) => {
            at(&mut out, IndexName::ConnectionsByEndpoint, &c.a);
            at(&mut out, IndexName::ConnectionsByEndpoint, &c.b);
            if let Some(page) = c.page {
                by_id(&mut out, IndexName::ConnectionsByPage, page);
            }
        }
        AnyRow::Junction(j) => by_id(&mut out, IndexName::JunctionsByPage, j.page),
        AnyRow::PageItem(i) => by_id(&mut out, IndexName::PageItemsByPage, i.page),
        AnyRow::StructureBox(b) => by_id(&mut out, IndexName::StructureBoxesByPage, b.page),
        AnyRow::Conductor(c) => by_id(&mut out, IndexName::ConductorsByCable, c.cable),
        AnyRow::PotentialDef(p) => at(&mut out, IndexName::PotentialDefsByEndpoint, &p.at),
        AnyRow::Bond(b) => {
            at(&mut out, IndexName::BondsByEndpoint, &b.a);
            at(&mut out, IndexName::BondsByEndpoint, &b.b);
        }
        _ => {}
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// The integration tests' fixture builder, shared with the unit tests below. The `path` is relative to this
/// file's directory (`src/`): a path attribute inside the inline `tests` module would be resolved against the
/// non-existent `src/memo_keys/tests/`, which Windows tolerates (lexical `..`) and Linux and macOS do not.
#[cfg(test)]
#[path = "../tests/common/mod.rs"]
pub(crate) mod test_common;

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::BTreeMap;
    use std::rc::Rc;
    use std::sync::Arc;

    use ew_model::{LINKS, Op};
    use ew_schema::Project;
    use proptest::collection::vec;
    use proptest::prelude::*;
    use proptest::test_runner::{
        Config, RngAlgorithm, TestCaseError, TestRng, TestRunner, contextualize_config,
    };
    use serde_json::{Value, json};

    use super::*;
    use crate::q::Q;
    use crate::tracker::RecordingTracker;

    use super::test_common::{Fx, id, uid};

    const FNDEF: u128 = 0x500;
    const SYM: u128 = 0x510;
    const NODE: u128 = 0x1000;
    const SECTION: u128 = 0x1100;
    const PAGE: u128 = 0x1200;
    const DEVICE: u128 = 0x1300;
    const FUNC: u128 = 0x1400;
    const REP: u128 = 0x1500;
    const CONN: u128 = 0x1600;
    const JUNC: u128 = 0x1700;
    const COND: u128 = 0x1800;
    const POT: u128 = 0x1900;
    const BOND: u128 = 0x1a00;
    const ABSENT: u128 = 0xdead;

    /// The kinds with a table, in the order of the `by_kind!` macro.
    const TABLE_KINDS: [Kind; 22] = [
        Kind::PlcConfigProject,
        Kind::ImportLineage,
        Kind::StructureNode,
        Kind::Section,
        Kind::ExternalRef,
        Kind::Page,
        Kind::Device,
        Kind::Function,
        Kind::Representation,
        Kind::Connection,
        Kind::Junction,
        Kind::Conductor,
        Kind::Channel,
        Kind::PotentialDef,
        Kind::Bond,
        Kind::DeviceLink,
        Kind::ReportBlock,
        Kind::Revision,
        Kind::Part,
        Kind::Symbol,
        Kind::Fndef,
        Kind::Form,
    ];

    const INDEXES: [IndexName; 15] = [
        IndexName::FunctionsByDevice,
        IndexName::RepsByPage,
        IndexName::RepsByFunction,
        IndexName::ConnectionsByPage,
        IndexName::ConnectionsByEndpoint,
        IndexName::JunctionsByPage,
        IndexName::TerminalsByStrip,
        IndexName::ConductorsByCable,
        IndexName::StructureChildren,
        IndexName::DeviceChildren,
        IndexName::PagesBySection,
        IndexName::PotentialDefsByEndpoint,
        IndexName::BondsByEndpoint,
        IndexName::PageItemsByPage,
        IndexName::StructureBoxesByPage,
    ];

    /// The `i`-th ID of a pool as UUID text.
    fn n(base: u128, i: u8) -> String {
        uid(base + u128::from(i))
    }

    /// An endpoint: a CP of function `i` for `sel < 3`, else junction `i`.
    fn ep(sel: u8, i: u8, cp: u8) -> Value {
        if sel < 3 {
            json!({"cp": [n(FUNC, i), u16::from(cp % 2)]})
        } else {
            json!({"junction": n(JUNC, i)})
        }
    }

    fn typed<T: Entity + serde::de::DeserializeOwned>(v: Value) -> AnyRow {
        AnyRow::of(serde_json::from_value::<T>(v.clone()).unwrap_or_else(|e| panic!("{v}: {e}")))
    }

    /// The row of kind selector `kind` (the order of `KIND_POOLS`) and pool index `idx`, its links chosen by
    /// `a`, `b` and `c`.
    fn make(kind: u8, idx: u8, a: u8, b: u8, c: u8) -> AnyRow {
        match kind {
            0 => {
                let mut v = json!({"id": n(NODE, idx), "aspect": "location", "code": format!("N{idx}")});
                if a < 4 {
                    v["parent"] = json!(n(NODE, b));
                }
                typed::<StructureNode>(v)
            }
            1 => typed::<Section>(json!({
                "id": n(SECTION, idx), "number": 1 + u16::from(a), "folio_range": [1, 9],
                "title": {"en": "s"}, "divider": false,
            })),
            2 => {
                let mut folio = json!({"number": 1 + u32::from(a)});
                if b < 4 {
                    folio["section"] = json!(n(SECTION, c));
                }
                typed::<Page>(json!({
                    "id": n(PAGE, idx), "folio": folio, "kind": "multi_line", "structure": {},
                    "title": {"en": "t"}, "grid": {"num": 4_000_000, "den": 1},
                }))
            }
            3 => {
                let mut v =
                    json!({"id": n(DEVICE, idx), "tag": {"letter": "K", "counter": 1 + u32::from(idx)}});
                if a < 2 {
                    v["parent"] = json!(n(DEVICE, b));
                }
                if b < 2 {
                    v["terminal"] = json!({"strip": n(DEVICE, c), "seq": 1, "designation": "1"});
                }
                typed::<Device>(v)
            }
            4 => {
                let mut v = json!({"id": n(FUNC, idx), "def": uid(FNDEF)});
                if a < 4 {
                    v["device"] = json!(n(DEVICE, b));
                }
                typed::<Function>(v)
            }
            5 => {
                let mut functions = vec![n(FUNC, a)];
                if b != a {
                    functions.push(n(FUNC, b));
                }
                let mut v = json!({
                    "id": n(REP, idx), "functions": functions, "page": n(PAGE, b), "symbol": uid(SYM),
                    "pos": [0, 0],
                });
                if c % 2 == 0 {
                    v["pin_map"] = json!([{"symbol_cp": 0, "function": n(FUNC, c), "idx": 0}]);
                }
                typed::<Representation>(v)
            }
            6 => {
                let mut v = json!({"id": n(CONN, idx), "a": ep(a, b, c), "b": ep(b, c, a)});
                if c < 4 {
                    v["page"] = json!(n(PAGE, a));
                }
                typed::<Connection>(v)
            }
            7 => {
                let sequence = if c < 3 { vec![n(CONN, b)] } else { vec![] };
                typed::<Junction>(json!({
                    "id": n(JUNC, idx), "page": n(PAGE, a), "at": [0, 0], "style": "dot", "sequence": sequence,
                }))
            }
            8 => typed::<Conductor>(json!({"id": n(COND, idx), "cable": n(DEVICE, a), "core": "1"})),
            9 => typed::<PotentialDef>(json!({"id": n(POT, idx), "at": ep(a, b, c), "name": "P"})),
            10 => {
                let mut v = json!({
                    "id": n(BOND, idx), "a": ep(a, b, c), "b": ep(b, c, a), "kind": "equipotential",
                });
                if c < 3 {
                    v["via"] = json!(n(CONN, a));
                }
                typed::<Bond>(v)
            }
            _ => unreachable!("kind selector"),
        }
    }

    /// The key kind and ID base of each selector of `make`.
    const KIND_POOLS: [(Kind, u128); 11] = [
        (Kind::StructureNode, NODE),
        (Kind::Section, SECTION),
        (Kind::Page, PAGE),
        (Kind::Device, DEVICE),
        (Kind::Function, FUNC),
        (Kind::Representation, REP),
        (Kind::Connection, CONN),
        (Kind::Junction, JUNC),
        (Kind::Conductor, COND),
        (Kind::PotentialDef, POT),
        (Kind::Bond, BOND),
    ];

    fn key_of(kind: u8, idx: u8) -> AnyKey {
        let (k, base) = KIND_POOLS[usize::from(kind)];
        (k, id::<()>(base + u128::from(idx)))
    }

    /// A model with a function definition and its symbol (rev 0), and a few rows of every kind that feeds a
    /// named index (rev 1; applied unchecked, links may dangle).
    fn base() -> Model {
        let mut fx = Fx::new();
        fx.fndef(FNDEF, ["Misc", "Misc", "Plain"], &[(0, "1"), (1, "2")]);
        fx.symbol(SYM, FNDEF, json!("origin"), &[(0, 0, 0, 0), (1, 1, 0, 1_000_000)]);
        let model = fx.build();
        let mut ops = Vec::new();
        for kind in 0..11u8 {
            for idx in 0..3u8 {
                ops.push(Op::insert(make(kind, idx, idx, idx + 1, idx + 2)));
            }
        }
        // Endpoints at junctions (`a >= 3`), and a page-less connection (`c >= 4`).
        ops.push(Op::insert(make(6, 3, 4, 4, 1)));
        ops.push(Op::insert(make(9, 3, 4, 1, 1)));
        ops.push(Op::insert(make(10, 3, 4, 4, 0)));
        model.apply_unchecked(&ops, Rev(1)).expect("seed rows apply")
    }

    fn keys_of_t<T: Entity>(m: &Model) -> Vec<AnyKey> {
        m.table::<T>().ids().map(|i| (T::KIND, i.cast())).collect()
    }

    fn all_keys(m: &Model) -> Vec<AnyKey> {
        let mut out = Vec::new();
        for kind in TABLE_KINDS {
            out.extend(by_kind!(kind, keys_of_t(m), Vec::new()));
        }
        out
    }

    macro_rules! id_buckets {
        ($out:ident, $name:expr, $ix:expr) => {
            $out.extend($ix.iter().map(|(k, _)| ($name, IxKey::Id(k.cast()))))
        };
    }

    macro_rules! endpoint_buckets {
        ($out:ident, $name:expr, $ix:expr) => {
            $out.extend($ix.iter().map(|(k, _)| ($name, IxKey::Endpoint(*k))))
        };
    }

    /// Every bucket key of the 15 named indexes.
    fn bucket_keys(m: &Model) -> Vec<(IndexName, IxKey)> {
        let ix = m.indexes();
        let mut out = Vec::new();
        id_buckets!(out, IndexName::FunctionsByDevice, ix.functions_by_device());
        id_buckets!(out, IndexName::RepsByPage, ix.reps_by_page());
        id_buckets!(out, IndexName::RepsByFunction, ix.reps_by_function());
        id_buckets!(out, IndexName::ConnectionsByPage, ix.connections_by_page());
        endpoint_buckets!(
            out,
            IndexName::ConnectionsByEndpoint,
            ix.connections_by_endpoint()
        );
        id_buckets!(out, IndexName::JunctionsByPage, ix.junctions_by_page());
        id_buckets!(out, IndexName::TerminalsByStrip, ix.terminals_by_strip());
        id_buckets!(out, IndexName::ConductorsByCable, ix.conductors_by_cable());
        id_buckets!(out, IndexName::StructureChildren, ix.structure_children());
        id_buckets!(out, IndexName::DeviceChildren, ix.device_children());
        id_buckets!(out, IndexName::PagesBySection, ix.pages_by_section());
        endpoint_buckets!(
            out,
            IndexName::PotentialDefsByEndpoint,
            ix.potential_defs_by_endpoint()
        );
        endpoint_buckets!(out, IndexName::BondsByEndpoint, ix.bonds_by_endpoint());
        id_buckets!(out, IndexName::PageItemsByPage, ix.page_items_by_page());
        id_buckets!(out, IndexName::StructureBoxesByPage, ix.structure_boxes_by_page());
        out
    }

    // -------------------------------------------------------------------------------------------------
    // shard, coarse, durability

    #[test]
    fn shard_is_deterministic_and_in_range() {
        for i in 0..10_000u128 {
            let a = id::<()>(i.wrapping_mul(0x9e37_79b9_7f4a_7c15_f39c_c060_5ced_c835));
            assert_eq!(shard(a), shard(a));
            assert!(shard(a) < SHARDS);
        }
        // Every one of the 128 bits contributes: flipping any single bit changes the shard.
        let bits = 0x0123_4567_89ab_cdef_0011_2233_4455_6677u128;
        for bit in 0..128 {
            assert_ne!(
                shard(id::<()>(bits)),
                shard(id::<()>(bits ^ (1u128 << bit))),
                "bit {bit} does not reach the shard"
            );
        }
    }

    #[test]
    fn coarse_keys_follow_the_contract() {
        let m = base();
        let rep = key_of(5, 0);
        let rep_page = n(PAGE, 1);
        assert_eq!(
            coarse(&m, &MemoInput::Read(DepKey::Row(rep))),
            MemoInput::Page(id::<Page>(PAGE + 1)),
            "a representation is registered on its page {rep_page}"
        );
        let paged = key_of(6, 0);
        assert_eq!(
            coarse(&m, &MemoInput::Read(DepKey::Row(paged))),
            MemoInput::Page(id::<Page>(PAGE))
        );
        let pageless = key_of(6, 2);
        assert_eq!(
            coarse(&m, &MemoInput::Read(DepKey::Row(pageless))),
            MemoInput::Shard(Kind::Connection, shard(pageless.1))
        );
        let junction = key_of(7, 1);
        assert_eq!(
            coarse(&m, &MemoInput::Read(DepKey::Row(junction))),
            MemoInput::Page(id::<Page>(PAGE + 1))
        );
        let absent = (Kind::Representation, id::<()>(ABSENT));
        assert_eq!(
            coarse(&m, &MemoInput::Read(DepKey::Row(absent))),
            MemoInput::Shard(Kind::Representation, shard(absent.1))
        );
        let device = key_of(3, 0);
        assert_eq!(
            coarse(&m, &MemoInput::Read(DepKey::Row(device))),
            MemoInput::Shard(Kind::Device, shard(device.1))
        );

        let page = id::<()>(PAGE);
        for name in [
            IndexName::RepsByPage,
            IndexName::ConnectionsByPage,
            IndexName::JunctionsByPage,
            IndexName::PageItemsByPage,
            IndexName::StructureBoxesByPage,
        ] {
            assert_eq!(
                coarse(&m, &MemoInput::Read(DepKey::IndexKey(name, IxKey::Id(page)))),
                MemoInput::Page(id::<Page>(PAGE))
            );
        }
        let dev = id::<()>(DEVICE);
        assert_eq!(
            coarse(
                &m,
                &MemoInput::Read(DepKey::IndexKey(IndexName::FunctionsByDevice, IxKey::Id(dev)))
            ),
            MemoInput::IndexShard(IndexName::FunctionsByDevice, shard(dev))
        );
        let f = id::<Function>(FUNC + 1);
        for idx in [0, 1, 7] {
            assert_eq!(
                coarse(
                    &m,
                    &MemoInput::Read(DepKey::IndexKey(
                        IndexName::ConnectionsByEndpoint,
                        IxKey::Endpoint(EndpointKey::Cp(f, idx))
                    ))
                ),
                MemoInput::IndexShard(IndexName::ConnectionsByEndpoint, shard(f.cast())),
                "all CPs of a function share a shard"
            );
        }
        let j = id::<Junction>(JUNC);
        assert_eq!(
            coarse(
                &m,
                &MemoInput::Read(DepKey::IndexKey(
                    IndexName::BondsByEndpoint,
                    IxKey::Endpoint(EndpointKey::Junction(j))
                ))
            ),
            MemoInput::IndexShard(IndexName::BondsByEndpoint, shard(j.cast()))
        );

        for own in [
            MemoInput::Read(DepKey::Table(Kind::Device)),
            MemoInput::Read(DepKey::Index(IndexName::RepsByPage)),
            MemoInput::Read(DepKey::Project),
            MemoInput::Read(DepKey::Reserved),
            MemoInput::Page(id(PAGE)),
            MemoInput::Shard(Kind::Device, 3),
            MemoInput::IndexShard(IndexName::BondsByEndpoint, 5),
        ] {
            assert_eq!(coarse(&m, &own), own);
        }
    }

    #[test]
    fn touched_durability() {
        let m = base();
        let project = (Kind::Project, m.project().id.cast::<()>());
        let fndef = (Kind::Fndef, id::<()>(FNDEF));
        let symbol = (Kind::Symbol, id::<()>(SYM));
        let connection = key_of(6, 0);

        let (list, level) = touched(&m, &m, &[project]);
        assert_eq!(level, Durability::Settings);
        assert!(list.contains(&MemoInput::Read(DepKey::Project)));
        assert!(!list.contains(&MemoInput::Read(DepKey::Row(project))));
        assert!(!list.contains(&MemoInput::Read(DepKey::Table(Kind::Project))));
        assert_eq!(touched(&m, &m, &[fndef]).1, Durability::Library);
        assert_eq!(touched(&m, &m, &[symbol]).1, Durability::Library);
        assert_eq!(touched(&m, &m, &[connection]).1, Durability::Drawing);
        assert_eq!(touched(&m, &m, &[project, fndef]).1, Durability::Library);
        assert_eq!(touched(&m, &m, &[project, connection]).1, Durability::Settings);

        let (list, level) = touched(&m, &m, &[]);
        assert_eq!(level, Durability::Drawing);
        assert_eq!(list, Vec::new(), "the reserved records did not change");

        assert_eq!(
            durability(&MemoInput::Read(DepKey::Project)),
            Durability::Settings
        );
        assert_eq!(
            durability(&MemoInput::Read(DepKey::Row(fndef))),
            Durability::Library
        );
        assert_eq!(
            durability(&MemoInput::Read(DepKey::Table(Kind::Form))),
            Durability::Library
        );
        assert_eq!(
            durability(&MemoInput::Read(DepKey::Row(connection))),
            Durability::Drawing
        );
        assert_eq!(durability(&MemoInput::Shard(Kind::Fndef, 1)), Durability::Drawing);
        assert_eq!(durability(&MemoInput::Page(id(PAGE))), Durability::Drawing);
        assert_eq!(
            durability(&MemoInput::Read(DepKey::Reserved)),
            Durability::Drawing
        );
        assert_eq!(
            durability(&MemoInput::Read(DepKey::Assets)),
            Durability::Drawing,
            "a drawing commit brings assets in"
        );
    }

    /// Cycle-29 review 4: an asset arrives without a row of the readers that depend on it (the file of an image item
    /// placed on another page), so a commit that grows the asset store touches `Read(Assets)` and nothing else, and
    /// its stamp changes with the insert only.
    #[test]
    fn touched_lists_the_asset_store_when_it_grew() {
        let before = base();
        let same = before.clone();
        assert_eq!(touched(&before, &same, &[]).0, Vec::new());
        let mut after = before.clone();
        let asset = ew_model::Asset::new(
            ew_schema::AssetRef {
                sha256: ew_schema::Sha256([7; 32]),
                format: ew_schema::AssetFormat::Svg,
                size: None,
            },
            vec![1u8, 2, 3],
        );
        after.put_asset(asset.clone());
        let (list, level) = touched(&before, &after, &[]);
        assert_eq!(list, vec![MemoInput::Read(DepKey::Assets)]);
        assert_eq!(level, Durability::Drawing);
        assert_ne!(stamp(&before, &DepKey::Assets), stamp(&after, &DepKey::Assets));
        let own = MemoInput::Read(DepKey::Assets);
        assert_eq!(coarse(&after, &own), own, "its own registration key");
        // Putting it again changes nothing.
        let mut again = after.clone();
        again.put_asset(asset);
        assert_eq!(touched(&after, &again, &[]).0, Vec::new());
    }

    // -------------------------------------------------------------------------------------------------
    // index_keys

    /// A row of kind `from` whose link `field` names `t`, and the index key it must feed.
    fn link_row(from: Kind, field: &str, t: u128) -> (AnyRow, IxKey) {
        let target = uid(t);
        let by_id = IxKey::Id(id::<()>(t));
        let other = json!({"cp": [uid(0x9999), 0]});
        // Endpoint links `a.cp[0]`, `b.junction` ...: the named side names `t`, the other one a CP.
        let sides = |field: &str| -> (Value, Value, IxKey) {
            let (named, key) = if field.ends_with("junction") {
                (json!({"junction": target}), EndpointKey::Junction(id(t)))
            } else {
                (json!({"cp": [target, 3]}), EndpointKey::Cp(id(t), 3))
            };
            if field.starts_with('b') {
                (other.clone(), named, IxKey::Endpoint(key))
            } else {
                (named, other.clone(), IxKey::Endpoint(key))
            }
        };
        match (from, field) {
            (Kind::StructureNode, "parent") => (
                typed::<StructureNode>(
                    json!({"id": uid(1), "aspect": "location", "code": "A", "parent": target}),
                ),
                by_id,
            ),
            (Kind::Page, "folio.section") => (
                typed::<Page>(json!({
                    "id": uid(1), "folio": {"number": 1, "section": target}, "kind": "multi_line",
                    "structure": {}, "title": {"en": "t"}, "grid": {"num": 4_000_000, "den": 1},
                })),
                by_id,
            ),
            (Kind::Device, "parent") => (
                typed::<Device>(json!({"id": uid(1), "tag": {"letter": "K"}, "parent": target})),
                by_id,
            ),
            (Kind::Device, "terminal.strip") => (
                typed::<Device>(json!({
                    "id": uid(1), "tag": {"letter": "X"},
                    "terminal": {"strip": target, "seq": 1, "designation": "1"},
                })),
                by_id,
            ),
            (Kind::Function, "device") => (
                typed::<Function>(json!({"id": uid(1), "def": uid(FNDEF), "device": target})),
                by_id,
            ),
            (Kind::Representation, "functions[]") => (
                typed::<Representation>(json!({
                    "id": uid(1), "functions": [target], "page": uid(2), "symbol": uid(SYM), "pos": [0, 0],
                })),
                by_id,
            ),
            (Kind::Representation, "pin_map[].function") => (
                typed::<Representation>(json!({
                    "id": uid(1), "functions": [], "page": uid(2), "symbol": uid(SYM), "pos": [0, 0],
                    "pin_map": [{"symbol_cp": 0, "function": target, "idx": 0}],
                })),
                by_id,
            ),
            (Kind::Representation, "page") => (
                typed::<Representation>(json!({
                    "id": uid(1), "functions": [], "page": target, "symbol": uid(SYM), "pos": [0, 0],
                })),
                by_id,
            ),
            (Kind::Connection, "page") => (
                typed::<Connection>(json!({
                    "id": uid(1), "a": other, "b": other, "page": target,
                })),
                by_id,
            ),
            (Kind::Connection, f @ ("a.cp[0]" | "a.junction" | "b.cp[0]" | "b.junction")) => {
                let (a, b, key) = sides(f);
                (typed::<Connection>(json!({"id": uid(1), "a": a, "b": b})), key)
            }
            (Kind::Junction, "page") => (
                typed::<Junction>(json!({
                    "id": uid(1), "page": target, "at": [0, 0], "style": "dot", "sequence": [],
                })),
                by_id,
            ),
            (Kind::Conductor, "cable") => (
                typed::<Conductor>(json!({"id": uid(1), "cable": target, "core": "1"})),
                by_id,
            ),
            (Kind::PotentialDef, f @ ("at.cp[0]" | "at.junction")) => {
                let (at, _, key) = sides(&format!("a.{}", &f[3..]));
                (
                    typed::<PotentialDef>(json!({"id": uid(1), "at": at, "name": "P"})),
                    key,
                )
            }
            (Kind::Bond, f @ ("a.cp[0]" | "a.junction" | "b.cp[0]" | "b.junction")) => {
                let (a, b, key) = sides(f);
                (
                    typed::<Bond>(json!({"id": uid(1), "a": a, "b": b, "kind": "equipotential"})),
                    key,
                )
            }
            (Kind::PageItem, "page") => (
                typed::<ew_schema::PageItem>(json!({
                    "id": uid(1), "page": target,
                    "kind": {"graphic": {"shape": {"line": {"a": [0, 0], "b": [1, 0]}}}},
                })),
                by_id,
            ),
            (Kind::StructureBox, "page") => (
                typed::<ew_schema::StructureBox>(json!({
                    "id": uid(1), "page": target, "outline": [[0, 0], [1, 0], [1, 1]], "style": "dotted",
                    "assigns": {},
                })),
                by_id,
            ),
            (kind, field) => panic!("link {kind:?}.{field} has no index_keys case in this test"),
        }
    }

    #[test]
    fn index_keys_cover_every_indexed_link() {
        let mut seen = Vec::new();
        for link in LINKS.iter().filter(|l| l.index.is_some()) {
            let name = link.index.expect("filtered");
            let (row, key) = link_row(link.from, link.field, 0x7777);
            let keys = index_keys(&row);
            assert!(
                keys.contains(&(name, key)),
                "{} is not mirrored by index_keys: {keys:?}",
                link.path()
            );
            seen.push(name);
        }
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen, INDEXES.to_vec(), "every named index is fed by some link");
    }

    #[test]
    fn index_keys_are_sorted_and_deduplicated() {
        // `functions` and `pin_map` name the same function and CP twice: one key each.
        let row = typed::<Representation>(json!({
            "id": uid(1), "functions": [uid(0x42)], "page": uid(2), "symbol": uid(SYM), "pos": [0, 0],
            "pin_map": [
                {"symbol_cp": 0, "function": uid(0x42), "idx": 0},
                {"symbol_cp": 1, "function": uid(0x42), "idx": 1},
            ],
        }));
        let keys = index_keys(&row);
        assert_eq!(
            keys,
            vec![
                (IndexName::RepsByPage, IxKey::Id(id(2))),
                (IndexName::RepsByFunction, IxKey::Id(id(0x42))),
            ]
        );
        let row = typed::<Connection>(json!({
            "id": uid(1), "a": {"cp": [uid(7), 0]}, "b": {"cp": [uid(7), 0]},
        }));
        assert_eq!(index_keys(&row).len(), 1);
    }

    // -------------------------------------------------------------------------------------------------
    // stamp agrees with Q

    fn probe_table<T: Entity>(q: &Q, ids: &[AnyId]) {
        let view = q.table::<T>();
        for i in ids {
            let _ = view.get(i.cast());
            let _ = view.contains(i.cast());
        }
        let _ = view.len();
        let _ = view.iter().count();
        let _ = view.ids().count();
    }

    /// Reads everything `Q` can read, present and absent, through a recording tracker, and checks that
    /// [`stamp`] reproduces every observed stamp.
    fn assert_stamps_agree(m: &Model) -> usize {
        let rec = Rc::new(RecordingTracker::new());
        let q = Q::with_tracker(m, Box::new(Rc::clone(&rec)));

        let keys = all_keys(m);
        for kind in TABLE_KINDS {
            let mut ids: Vec<AnyId> = keys.iter().filter(|k| k.0 == kind).map(|k| k.1).collect();
            ids.push(id(ABSENT));
            by_kind!(kind, probe_table(&q, &ids), ());
        }

        let ix = q.idx();
        for i in (0..6u8).map(u128::from).chain([ABSENT]) {
            let _ = ix.functions_of_device(id(DEVICE + i)).count();
            let _ = ix.reps_by_page(id(PAGE + i)).count();
            let _ = ix.reps_of_function(id(FUNC + i)).count();
            let _ = ix.connections_by_page(id(PAGE + i)).count();
            for cp in [0, 1] {
                let _ = ix.connections_at_cp(id(FUNC + i), cp).count();
                let _ = ix.connections_at(EndpointKey::Cp(id(FUNC + i), cp)).count();
                let _ = ix.potential_defs_at(EndpointKey::Cp(id(FUNC + i), cp)).count();
                let _ = ix.bonds_at(EndpointKey::Cp(id(FUNC + i), cp)).count();
            }
            let _ = ix.connected_cps_of_function(id(FUNC + i));
            let _ = ix.connections_at(EndpointKey::Junction(id(JUNC + i))).count();
            let _ = ix.potential_defs_at(EndpointKey::Junction(id(JUNC + i))).count();
            let _ = ix.bonds_at(EndpointKey::Junction(id(JUNC + i))).count();
            let _ = ix.junctions_by_page(id(PAGE + i)).count();
            let _ = ix.terminals_of_strip(id(DEVICE + i)).count();
            let _ = ix.conductors_of_cable(id(DEVICE + i)).count();
            let _ = ix.structure_children(id(NODE + i)).count();
            let _ = ix.device_children(id(DEVICE + i)).count();
            let _ = ix.pages_of_section(id(SECTION + i)).count();
        }
        let _ = q.project();
        let _ = q.profile();
        let _ = q.reserved(Kind::Waiver).count();
        let _ = q.logo_asset(&ew_schema::Sha256([7; 32]));

        let reads = rec.reads();
        for r in &reads {
            assert_eq!(
                stamp(m, &r.dep),
                r.observed,
                "stamp of {:?} in rev {}",
                r.dep,
                m.rev()
            );
        }
        for want in [
            |d: &DepKey| matches!(d, DepKey::Row(_)),
            |d: &DepKey| matches!(d, DepKey::Table(_)),
            |d: &DepKey| matches!(d, DepKey::IndexKey(..)),
            |d: &DepKey| matches!(d, DepKey::Index(_)),
            |d: &DepKey| matches!(d, DepKey::Project),
            |d: &DepKey| matches!(d, DepKey::Assets),
            |d: &DepKey| matches!(d, DepKey::Reserved),
        ] {
            assert!(
                reads.iter().any(|r| want(&r.dep)),
                "a kind of read was not exercised"
            );
        }
        reads.len()
    }

    #[test]
    fn stamp_agrees_with_q() {
        let m1 = base();
        assert_eq!(m1.rev(), Rev(1));
        assert_stamps_agree(&m1);

        // Rev 2: a representation moves page, a connection goes away, a bond and a conductor appear.
        let m1_row = |key| m1.row(&key).expect("seed row");
        let ops = vec![
            Op::update(m1_row(key_of(5, 0)), make(5, 0, 0, 2, 3)),
            Op::delete(m1_row(key_of(6, 0))),
            Op::update(m1_row(key_of(10, 3)), make(10, 3, 0, 1, 2)),
            Op::insert(make(8, 3, 3, 0, 0)),
        ];
        let m2 = m1.apply_unchecked(&ops, Rev(2)).expect("rev 2");
        assert_stamps_agree(&m2);

        // Rev 3: the project row changes, and a library copy is put again.
        let project_key = (Kind::Project, m2.project().id.cast::<()>());
        let AnyRow::Project(project) = m2.row(&project_key).expect("project row") else {
            panic!("project row is a project");
        };
        let mut v = serde_json::to_value(&*project).expect("project serialises");
        v["derivation_profile"] = json!("2027.2");
        let changed: Project = serde_json::from_value(v).expect("project parses");
        let symbol = m2.row(&(Kind::Symbol, id(SYM))).expect("symbol copy");
        let ops = vec![
            Op::update(AnyRow::Project(project), AnyRow::Project(Arc::new(changed))),
            Op::update(symbol.clone(), symbol),
            Op::insert(make(7, 3, 4, 0, 0)),
        ];
        let m3 = m2.apply_unchecked(&ops, Rev(3)).expect("rev 3");
        let observed = assert_stamps_agree(&m3);
        assert!(observed > 100, "the probe reads a lot ({observed})");

        // The stamps are not all the same: rows put at Rev(2) and Rev(3) carry their revision.
        assert_eq!(stamp(&m3, &DepKey::Project), Rev(3));
        assert_eq!(stamp(&m3, &DepKey::Row(key_of(5, 0))), Rev(2));
        assert_eq!(stamp(&m3, &DepKey::Row(key_of(5, 1))), Rev(1));
        assert_eq!(stamp(&m3, &DepKey::Reserved), Rev(3));
        // An absent row reads the table's stamp.
        assert_eq!(
            stamp(&m3, &DepKey::Row((Kind::Connection, id(ABSENT)))),
            stamp(&m3, &DepKey::Table(Kind::Connection))
        );
        // A kind without a table answers the model's revision.
        assert_eq!(stamp(&m3, &DepKey::Row((Kind::Waiver, id(ABSENT)))), Rev(3));
    }

    // -------------------------------------------------------------------------------------------------
    // touched covers every changed stamp

    /// One step of a generated batch: `kind` 0..=10 selects a `make` kind (`op == 0` deletes, else the row is
    /// inserted or replaced), 11 puts a library copy again, 12 changes the project row.
    #[derive(Clone, Debug)]
    struct Act {
        kind: u8,
        idx: u8,
        op: u8,
        a: u8,
        b: u8,
        c: u8,
    }

    fn act() -> impl Strategy<Value = Act> {
        (0u8..13, 0u8..4, 0u8..4, 0u8..6, 0u8..6, 0u8..6).prop_map(|(kind, idx, op, a, b, c)| Act {
            kind,
            idx,
            op,
            a,
            b,
            c,
        })
    }

    fn plan() -> impl Strategy<Value = Vec<Vec<Act>>> {
        vec(vec(act(), 1..5), 1..6)
    }

    /// The op of one action over the rows the batch has produced so far.
    fn op_of(model: &Model, overlay: &mut BTreeMap<AnyKey, Option<AnyRow>>, a: &Act) -> Option<Op> {
        let current = |overlay: &BTreeMap<AnyKey, Option<AnyRow>>, key: &AnyKey| {
            overlay.get(key).cloned().unwrap_or_else(|| model.row(key))
        };
        let op = match a.kind {
            0..=10 => {
                let key = key_of(a.kind, a.idx);
                let before = current(overlay, &key);
                if a.op == 0 {
                    before.map(Op::delete)
                } else {
                    let row = make(a.kind, a.idx, a.a, a.b, a.c);
                    Some(match before {
                        None => Op::insert(row),
                        Some(b) => Op::update(b, row),
                    })
                }
            }
            11 => {
                let key = if a.a % 2 == 0 {
                    (Kind::Symbol, id::<()>(SYM))
                } else {
                    (Kind::Fndef, id::<()>(FNDEF))
                };
                let row = current(overlay, &key)?;
                Some(Op::update(row.clone(), row))
            }
            _ => {
                let key = (Kind::Project, model.project().id.cast::<()>());
                let before = current(overlay, &key)?;
                let AnyRow::Project(p) = &before else { return None };
                let mut v = serde_json::to_value(&**p).expect("project serialises");
                v["derivation_profile"] = json!(if a.a % 2 == 0 { "2027.1" } else { "2027.2" });
                let after: Project = serde_json::from_value(v).expect("project parses");
                Some(Op::update(before, AnyRow::Project(Arc::new(after))))
            }
        };
        // A batch puts a key at most once: `touched` sees `before` and `after` only, not the states a key
        // passes through inside a batch (a link set and cleared again stamps its bucket without the net
        // rows feeding it; that stamp-only change is not pushed, which is sound because the content is
        // equal, and a verification walk compares the stamp anyway).
        let op = op.filter(|op| !overlay.contains_key(&op.key()));
        if let Some(op) = &op {
            overlay.insert(op.key(), op.after().cloned());
        }
        op
    }

    /// Checks the contract between registration and touch for every batch of `plan`; counts the changed
    /// stamps it saw in `changed`.
    fn check(plan: &[Vec<Act>], changed: &Cell<usize>) -> Result<(), TestCaseError> {
        let mut model = base();
        for batch in plan {
            let mut overlay = BTreeMap::new();
            let ops: Vec<Op> = batch
                .iter()
                .filter_map(|a| op_of(&model, &mut overlay, a))
                .collect();
            if ops.is_empty() {
                continue;
            }
            let after = model
                .apply_unchecked(&ops, model.rev().next())
                .expect("a batch of current rows applies");
            let keys: Vec<AnyKey> = ops.iter().map(Op::key).collect();
            let (list, _) = touched(&model, &after, &keys);
            prop_assert!(
                list.windows(2).all(|w| w[0] < w[1]),
                "the list is sorted and has no duplicates: {list:?}"
            );

            let mut deps = vec![DepKey::Project, DepKey::Assets, DepKey::Reserved];
            deps.extend(INDEXES.map(DepKey::Index));
            deps.extend(TABLE_KINDS.map(DepKey::Table));
            for m in [&model, &after] {
                deps.extend(all_keys(m).into_iter().map(DepKey::Row));
                deps.extend(bucket_keys(m).into_iter().map(|(n, k)| DepKey::IndexKey(n, k)));
            }
            for d in deps {
                // The reserved records' stamp is the model revision; their content is what counts (the
                // `ew_memo::Db` content rule), and no op changes it.
                if d == DepKey::Reserved {
                    prop_assert_eq!(
                        list.contains(&MemoInput::Read(DepKey::Reserved)),
                        model.unknown_records() != after.unknown_records()
                    );
                    continue;
                }
                if stamp(&model, &d) != stamp(&after, &d) {
                    changed.set(changed.get() + 1);
                    let c = coarse(&model, &MemoInput::Read(d));
                    prop_assert!(
                        list.contains(&c),
                        "{d:?} changed its stamp but its coarse key {c:?} is not touched; keys {keys:?}"
                    );
                }
            }
            model = after;
        }
        Ok(())
    }

    #[test]
    fn touched_covers_every_changed_stamp() {
        // `contextualize_config` applies the PROPTEST_* variables (the deep nightly's PROPTEST_CASES) that the
        // `proptest!` macro applies itself and a hand-built runner would otherwise ignore.
        let config = contextualize_config(Config {
            cases: 128,
            failure_persistence: None,
            ..Config::default()
        });
        let mut runner = TestRunner::new_with_rng(config, TestRng::deterministic_rng(RngAlgorithm::ChaCha));
        let changed = Cell::new(0usize);
        runner
            .run(&plan(), |plan| check(&plan, &changed))
            .expect("the contract holds");
        assert!(
            changed.get() > 1000,
            "the property is not vacuous ({})",
            changed.get()
        );
    }
}
