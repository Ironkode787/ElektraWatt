// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew verify-derived`: the memoised derived data against a cold recomputation (FR-EXT-005, NFR-REL-08,
//! doc 18 §3.4).
//!
//! [`sweep`] reads the memoised families of `ew-derive`, of `ew-reports` (the report rows, the generated pages
//! and the tag classes), of `ew-checks` (every rule's findings in every scope) and of the page output (the page views and the per-layer page display lists of the
//! export job) at every key of the snapshot through a memoising [`Q`] and through a cold one, and lists the keys
//! whose values differ. Its report names every family it read ([`Sweep::keys`]).
//! [`verify_derived`] runs it over a session twice (the second pass reads mostly memo hits) and adds the
//! mismatches that the session's own oracle found while the sweep ran ([`MemoMode::Oracle`](crate::MemoMode)).
//! The acceptance runner calls it after each scripted edit of AC-16, the CLI after opening a project.
//!
//! `tests/it/memo_oracle.rs` (`every_family_the_store_memoises_is_swept`) checks after a workload over the golden
//! project that every family holding memos in the session's store is among the families the sweep read.

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::ew_ids::Id;
use ew_derive::{
    BomMode, BomRows, BondIndex, BondPathsQ, BondVias, DefPoints, DevicePartLines, DeviceTag,
    DevicesByStructure, DrawnPageOrder, EndClass, EndScope, FlowBfs, FlowNotes, FunctionLinksOf, Member,
    PageIds, PageLocalNets, PageOrder, PageRef, PagelessLinks, PhysicalWiresOf, PotentialAttrsOf,
    PotentialId, PotentialNameOf, PotentialPartition, ProfileKey, ProfileRowFor, Q, Query, RailBondsQ,
    RailTerminalOf, RailTerminals, SettingsMember, StitchGraphQ, TitleBlockValues, TocRows, WireDefaultProps,
    WireNumbers, WireProfileNotes, WireProfilesQ, WirePropsOf, XrefTextOf,
};
use ew_derive::{IpFunctions, IpInfoOf, IpNotes, IpPairingQ};
use ew_reports::{BlockPages, BlockRows, TagClassMembers};
use ew_schema::{
    CircuitClass, Device, Function, Page, PlcConfigProject, ReportBlock, ReportKind, ReportSource,
    Representation,
};

use crate::output::{
    CarriersLayer, CpTargetsByPage, FrameLayer, LabelsLayer, PageBindingsOf, PageCpTargetsOf, PageDigest,
    PageEmbeddedOf, PageFrameOf, PageGridOf, PageRepsOf, PageSlotsOf, PageSurfaceView, PageTableOf,
    PlacementsLayer, ShapedText, SurfaceLayer, SymbolsLayer, TablesLayer, WiresLayer,
};
use crate::{Mismatch, Session};

/// The keys of the snapshot a `Q` sees, for the families with a non-trivial key.
struct Keys {
    /// The report blocks; the flag marks a table of contents (its pages are keyed by their count).
    blocks: Vec<(Id<ReportBlock>, bool)>,
    cfgs: Vec<Id<PlcConfigProject>>,
    /// The logical and physical bus nets.
    lnets: Vec<Id<ew_schema::LogicalNet>>,
    pnets: Vec<Id<ew_schema::PhysicalNet>>,
    devices: Vec<Id<Device>>,
    functions: Vec<Id<Function>>,
    pages: Vec<Id<Page>>,
    reps: Vec<Id<Representation>>,
    page_refs: Vec<PageRef>,
    potentials: Vec<PotentialId>,
    /// The profile keys of the potentials, and the same attributes with each known circuit class.
    profile_keys: BTreeSet<ProfileKey>,
    /// The layout spaces, carriers and placements.
    spaces: Vec<Id<ew_schema::LayoutSpace>>,
    carriers: Vec<Id<ew_schema::Carrier>>,
    placements: Vec<Id<ew_schema::Placement>>,
    /// The fluid lines.
    fluid_lines: Vec<Id<ew_schema::Connection>>,
}

impl Keys {
    fn of(q: &Q) -> Keys {
        let potentials: Vec<PotentialId> = q
            .get::<PotentialPartition>(())
            .potentials()
            .map(|p| p.id)
            .collect();
        let mut profile_keys = BTreeSet::new();
        for p in &potentials {
            let key = ProfileKey::of_potential(q, *p);
            for class in [
                CircuitClass::Mains,
                CircuitClass::Control,
                CircuitClass::Analogue,
                CircuitClass::Bus,
                CircuitClass::Pe,
            ] {
                profile_keys.insert(ProfileKey::new(&key.attrs(), Some(class)));
            }
            profile_keys.insert(key);
        }
        Keys {
            blocks: q
                .report_blocks()
                .iter()
                .map(|(id, b)| (id, matches!(b.source, ReportSource::Kind(ReportKind::Toc))))
                .collect(),
            cfgs: q.table::<PlcConfigProject>().ids().collect(),
            lnets: q.table::<ew_schema::LogicalNet>().ids().collect(),
            pnets: q.table::<ew_schema::PhysicalNet>().ids().collect(),
            devices: q.devices().ids().collect(),
            functions: q.functions().ids().collect(),
            pages: q.pages().ids().collect(),
            reps: q.reps().ids().collect(),
            page_refs: q.get::<PageOrder>(()).entries.iter().map(|e| e.page).collect(),
            potentials,
            profile_keys,
            spaces: q.table::<ew_schema::LayoutSpace>().ids().collect(),
            carriers: q.table::<ew_schema::Carrier>().ids().collect(),
            placements: q.table::<ew_schema::Placement>().ids().collect(),
            fluid_lines: q
                .connections()
                .iter()
                .filter(|(_, c)| matches!(c.kind, ew_schema::ConnectionKind::Fluid(_)))
                .map(|(id, _)| id)
                .collect(),
        }
    }
}

/// The result of one [`sweep`].
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Sweep {
    /// The number of keys read per family.
    pub keys: BTreeMap<&'static str, usize>,
    /// One line per key whose memoised value differs from the cold one: `family(key)`.
    pub differences: Vec<String>,
}

/// Reads `F` at every key through `memo` and, when given, `cold`, and records the keys whose values differ.
fn check<F: Query>(memo: &Q, cold: Option<&Q>, keys: impl IntoIterator<Item = F::Key>, out: &mut Sweep) {
    let mut n = 0;
    for key in keys {
        let m = memo.get::<F>(key.clone());
        if let Some(cold) = cold {
            let c = cold.get::<F>(key.clone());
            if *m != *c {
                out.differences.push(format!("{}({key:?})", F::FAMILY));
            }
        }
        n += 1;
    }
    out.keys.insert(F::FAMILY, n);
}

/// Reads every family at every key of the snapshot through `memo` and compares with `cold`. The keys come
/// from `cold` when there is one, else from `memo`; without `cold` the sweep only fills the memo store.
pub fn sweep(memo: &Q, cold: Option<&Q>) -> Sweep {
    let keys = Keys::of(cold.unwrap_or(memo));
    let mut out = Sweep::default();
    let o = &mut out;
    check::<BlockRows>(memo, cold, keys.blocks.iter().map(|(id, _)| *id), o);
    // The pages of a table of contents are keyed by their count, which `page_order` settles: 1 to 3 cover it.
    check::<BlockPages>(
        memo,
        cold,
        keys.blocks.iter().flat_map(|(id, toc)| {
            if *toc {
                vec![(*id, 1), (*id, 2), (*id, 3)]
            } else {
                vec![(*id, 0)]
            }
        }),
        o,
    );
    check::<TagClassMembers>(memo, cold, [()], o);
    check::<BomRows>(memo, cold, [BomMode::Component, BomMode::Assembly], o);
    check::<ew_derive::AssemblyBreakMarks>(memo, cold, [()], o);
    check::<BondIndex>(memo, cold, [()], o);
    check::<BondPathsQ>(memo, cold, [()], o);
    check::<BondVias>(memo, cold, [()], o);
    check::<DefPoints>(memo, cold, [()], o);
    check::<FunctionLinksOf>(memo, cold, keys.functions.iter().copied(), o);
    check::<IpFunctions>(memo, cold, [()], o);
    check::<IpInfoOf>(memo, cold, keys.functions.iter().copied(), o);
    check::<IpPairingQ>(memo, cold, [()], o);
    // An interruption point's structure and the structure box around a representation.
    check::<ew_derive::IpStructure>(memo, cold, keys.functions.iter().copied(), o);
    check::<ew_derive::RepStructureBox>(memo, cold, keys.reps.iter().copied(), o);
    check::<PageIds>(memo, cold, [()], o);
    check::<PagelessLinks>(memo, cold, [()], o);
    check::<RailBondsQ>(memo, cold, [()], o);
    check::<RailTerminalOf>(memo, cold, keys.devices.iter().copied(), o);
    check::<RailTerminals>(memo, cold, [()], o);
    check::<FlowNotes>(memo, cold, [()], o);
    check::<ew_derive::FluidNets>(memo, cold, [()], o);
    check::<ew_derive::ValveSlots>(memo, cold, keys.devices.iter().copied(), o);
    check::<ew_derive::Galleries>(memo, cold, keys.devices.iter().copied(), o);
    check::<ew_derive::FluidTubes>(memo, cold, [()], o);
    check::<ew_derive::MediumZones>(memo, cold, [()], o);
    check::<ew_derive::TubePropsOf>(memo, cold, keys.fluid_lines.iter().copied(), o);
    check::<ew_derive::TubeRows>(memo, cold, [()], o);
    check::<ew_derive::ValveTerminalRows>(memo, cold, [()], o);
    check::<ew_derive::MediumRows>(memo, cold, [()], o);
    check::<IpNotes>(memo, cold, [()], o);
    check::<PotentialAttrsOf>(memo, cold, keys.potentials.iter().copied(), o);
    check::<PotentialNameOf>(memo, cold, keys.potentials.iter().copied(), o);
    check::<ew_derive::ModuleOrder>(memo, cold, keys.cfgs.iter().copied(), o);
    check::<ew_derive::ModuleStarts>(memo, cold, keys.cfgs.iter().copied(), o);
    // The per-module start (D9) and the address index (EW-PLC-001).
    check::<ew_derive::ModuleStartOf>(memo, cold, keys.devices.iter().copied(), o);
    check::<ew_derive::AddressIndex>(memo, cold, keys.cfgs.iter().copied(), o);
    check::<ew_derive::ChannelAddresses>(memo, cold, keys.devices.iter().copied(), o);
    check::<ew_derive::ChannelStates>(memo, cold, keys.devices.iter().copied(), o);
    check::<ew_derive::ChannelTargets>(memo, cold, keys.devices.iter().copied(), o);
    check::<ew_derive::ChannelTexts>(memo, cold, keys.devices.iter().copied(), o);
    // The channel indexes.
    check::<ew_derive::ChannelsByModule>(memo, cold, [()], o);
    check::<ew_derive::ChannelsByCp>(memo, cold, [()], o);
    check::<ew_derive::ModuleChannelIds>(memo, cold, keys.devices.iter().copied(), o);
    check::<ew_derive::SolenoidPositions>(memo, cold, keys.devices.iter().copied(), o);
    // The bus segments and the members of a logical net.
    check::<ew_derive::BusSegments>(memo, cold, keys.pnets.iter().copied(), o);
    check::<ew_derive::NetMembersOf>(memo, cold, keys.lnets.iter().copied(), o);
    check::<ew_derive::PlcRows>(memo, cold, keys.devices.iter().copied(), o);
    // The printed PLC addresses, and the targets they print on (FT-700).
    check::<ew_derive::ChannelsByTarget>(memo, cold, [()], o);
    check::<ew_derive::PlcAddressText>(memo, cold, keys.functions.iter().copied(), o);
    check::<DevicePartLines>(memo, cold, keys.devices.iter().copied(), o);
    check::<DeviceTag>(memo, cold, keys.devices.iter().copied(), o);
    check::<DevicesByStructure>(memo, cold, [()], o);
    // The drawing settings (crossings as hops).
    check::<ew_derive::DrawingSettingsQ>(memo, cold, [()], o);
    check::<DrawnPageOrder>(memo, cold, [()], o);
    check::<EndClass>(
        memo,
        cold,
        keys.devices
            .iter()
            .flat_map(|d| [EndScope::Device(*d), EndScope::Strip(*d)]),
        o,
    );
    check::<PageLocalNets>(memo, cold, keys.pages.iter().copied(), o);
    check::<PageOrder>(memo, cold, [()], o);
    check::<PhysicalWiresOf>(memo, cold, keys.potentials.iter().copied(), o);
    check::<FlowBfs>(memo, cold, [()], o);
    check::<ew_derive::CpFlowDistanceOf>(memo, cold, keys.potentials.iter().copied(), o);
    check::<PotentialPartition>(memo, cold, [()], o);
    check::<ProfileRowFor>(memo, cold, keys.profile_keys.iter().cloned(), o);
    check::<SettingsMember>(memo, cold, Member::ALL, o);
    check::<StitchGraphQ>(memo, cold, [()], o);
    // ADR-0032: the project's effective text-style table.
    check::<ew_derive::TextStyles>(memo, cold, [()], o);
    check::<ew_derive::StripBridges>(memo, cold, keys.devices.iter().copied(), o);
    check::<TitleBlockValues>(memo, cold, keys.page_refs.iter().copied(), o);
    check::<TocRows>(memo, cold, [()], o);
    check::<WireDefaultProps>(memo, cold, [()], o);
    check::<WireNumbers>(memo, cold, [()], o);
    check::<WireProfileNotes>(memo, cold, [()], o);
    check::<WireProfilesQ>(memo, cold, [()], o);
    check::<WirePropsOf>(memo, cold, keys.potentials.iter().copied(), o);
    check::<XrefTextOf>(memo, cold, keys.reps.iter().copied(), o);
    // The layout queries; a label in both modes on every page (the abbreviated one reads the page).
    check::<ew_derive::layout::Unplaced>(memo, cold, keys.spaces.iter().copied(), o);
    check::<ew_derive::layout::RailOccupancy>(memo, cold, keys.carriers.iter().copied(), o);
    // The derived end brackets of every carrier.
    check::<ew_derive::layout::GroupAccessories>(memo, cold, keys.carriers.iter().copied(), o);
    check::<ew_derive::layout::PlacementGeometry>(memo, cold, keys.placements.iter().copied(), o);
    check::<ew_derive::layout::LayoutLabel>(
        memo,
        cold,
        keys.placements.iter().flat_map(|p| {
            keys.pages.iter().flat_map(move |page| {
                [
                    ew_derive::layout::LabelMode::Abbreviated,
                    ew_derive::layout::LabelMode::Full,
                ]
                .map(|mode| (*p, *page, mode))
            })
        }),
        o,
    );
    // The page output: the memoised parts of the page views and the page display list of every layer.
    check::<PageFrameOf>(memo, cold, keys.page_refs.iter().copied(), o);
    check::<PageGridOf>(memo, cold, keys.page_refs.iter().copied(), o);
    check::<PageRepsOf>(memo, cold, keys.pages.iter().copied(), o);
    check::<PageSlotsOf>(memo, cold, keys.pages.iter().copied(), o);
    check::<PageCpTargetsOf>(memo, cold, keys.pages.iter().copied(), o);
    check::<CpTargetsByPage>(memo, cold, [()], o);
    check::<PageBindingsOf>(memo, cold, keys.page_refs.iter().copied(), o);
    check::<PageTableOf>(memo, cold, keys.page_refs.iter().copied(), o);
    check::<PageEmbeddedOf>(memo, cold, keys.pages.iter().copied(), o);
    check::<FrameLayer>(memo, cold, keys.page_refs.iter().copied(), o);
    check::<SymbolsLayer>(memo, cold, keys.page_refs.iter().copied(), o);
    check::<WiresLayer>(memo, cold, keys.page_refs.iter().copied(), o);
    check::<LabelsLayer>(memo, cold, keys.page_refs.iter().copied(), o);
    check::<TablesLayer>(memo, cold, keys.page_refs.iter().copied(), o);
    // The surface view of a panel-layout page, its three layers and its shaped labels.
    check::<PageSurfaceView>(memo, cold, keys.pages.iter().copied(), o);
    check::<SurfaceLayer>(memo, cold, keys.page_refs.iter().copied(), o);
    check::<CarriersLayer>(memo, cold, keys.page_refs.iter().copied(), o);
    check::<PlacementsLayer>(memo, cold, keys.page_refs.iter().copied(), o);
    let shaped: BTreeSet<(String, ew_text::TextStyle)> = keys
        .pages
        .iter()
        .flat_map(|p| crate::output::panel::label_keys(cold.unwrap_or(memo), *p))
        .collect();
    check::<ShapedText>(memo, cold, shaped, o);
    check::<PageDigest>(memo, cold, keys.page_refs.iter().copied(), o);
    // The memoised findings of every shipped rule in every scope of its kind (`ew-checks`).
    let q = cold.unwrap_or(memo);
    let mut scopes = BTreeMap::new();
    let checks: Vec<(&'static str, ew_checks::Scope)> = ew_checks::rules()
        .iter()
        .flat_map(|r| {
            let meta = r.meta();
            let all = scopes
                .entry(meta.scope)
                .or_insert_with(|| ew_checks::scopes(q, meta.scope));
            all.iter().map(|s| (meta.id, *s)).collect::<Vec<_>>()
        })
        .collect();
    check::<ew_checks::CheckOf>(memo, cold, checks, o);
    out
}

/// What [`verify_derived`] found.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct VerifyReport {
    /// The number of keys read per family (one pass).
    pub keys: BTreeMap<&'static str, usize>,
    /// The keys whose memoised value differs from the cold value, over both passes (deduplicated).
    pub differences: Vec<String>,
    /// What the session's own oracle found while the sweeps ran (empty unless the session runs in
    /// [`MemoMode::Oracle`](crate::MemoMode)).
    pub oracle: Vec<Mismatch>,
}

impl VerifyReport {
    /// Whether nothing differs: `ew verify-derived` exits 0.
    pub fn is_clean(&self) -> bool {
        self.differences.is_empty() && self.oracle.is_empty()
    }

    /// The number of keys read in one pass.
    pub fn total_keys(&self) -> usize {
        self.keys.values().sum()
    }
}

/// Why a verification could not run.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VerifyError {
    /// The session does not memoise ([`MemoMode::Off`](crate::MemoMode)): the memoised and the cold read are the
    /// same computation, so a comparison would always be clean.
    MemoOff,
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifyError::MemoOff => {
                f.write_str("the session does not memoise (memo mode off): there is nothing to verify")
            }
        }
    }
}

impl std::error::Error for VerifyError {}

/// Recomputes every derived value of `session`'s current snapshot cold and compares it with the memoised
/// value, key by key, in two passes through the session's memo store (the second reads the memo hits). With a
/// session in oracle mode, the oracle's own mismatches since the last call are included.
///
/// # Errors
///
/// [`VerifyError::MemoOff`] for a session without a memo store.
pub fn verify_derived(session: &Session) -> Result<VerifyReport, VerifyError> {
    if session.memo().is_none() {
        return Err(VerifyError::MemoOff);
    }
    let cold = session.query_over(&session.world().model);
    let mut report = VerifyReport::default();
    for pass in 0..2 {
        let memo = session.query();
        let result = sweep(&memo, Some(&cold));
        if pass == 0 {
            report.keys = result.keys;
        }
        for d in result.differences {
            if !report.differences.contains(&d) {
                report.differences.push(d);
            }
        }
    }
    report.oracle = session.take_oracle_mismatches();
    Ok(report)
}
