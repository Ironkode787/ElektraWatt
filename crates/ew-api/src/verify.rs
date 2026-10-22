// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew verify-derived`: the memoised derived data against a cold recomputation (FR-EXT-005, NFR-REL-08,
//! doc 18 §3.4).
//!
//! [`sweep`] reads every `Memo` and `MemoLru` family of `ew_derive::FAMILIES`, `ew_reports::FAMILIES` (the
//! report rows, the generated pages and the tag classes) and [`crate::output::FAMILIES`] (the page views and the
//! per-layer page display lists of the export job) at every key of the snapshot through a memoising [`Q`] and
//! through a cold one, and lists the keys whose values differ.
//! [`verify_derived`] runs it over a session twice (the second pass reads mostly memo hits) and adds the
//! mismatches that the session's own oracle found while the sweep ran ([`MemoMode::Oracle`](crate::MemoMode)).
//! The acceptance runner calls it after each scripted edit of AC-16, the CLI after opening a project.
//!
//! The family list [`SWEPT`] is pinned against the three `FAMILIES` lists by `tests/verify.rs` (a promotion or
//! demotion of a family breaks that test).

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
    CpTargetsByPage, FrameLayer, LabelsLayer, PageBindingsOf, PageCpTargetsOf, PageEmbeddedOf, PageFrameOf,
    PageGridOf, PageRepsOf, PageSlotsOf, PageTableOf, SymbolsLayer, TablesLayer, WiresLayer,
};
use crate::{Mismatch, Session};

/// The families the sweep reads: the `Memo` and `MemoLru` families of `ew_derive::FAMILIES`,
/// `ew_reports::FAMILIES` and `ew_api::output::FAMILIES`, sorted.
pub const SWEPT: &[&str] = &[
    "assembly_break_marks",
    "bom_rows",
    "bond_index",
    "bond_paths",
    "bond_vias",
    "channel_addresses",
    "channel_states",
    "channel_targets",
    "channel_texts",
    "cp_flow_distance",
    "cp_targets_by_page",
    "def_points",
    "device_part_lines",
    "device_tag",
    "devices_by_structure",
    "drawn_page_order",
    "end_class",
    "flow_notes",
    "function_links",
    "ip_functions",
    "ip_info",
    "ip_notes",
    "ip_pairing",
    "module_order",
    "module_starts",
    "page_bindings",
    "page_cp_targets",
    "page_display_list_frame",
    "page_display_list_labels",
    "page_display_list_symbols",
    "page_display_list_tables",
    "page_display_list_wires",
    "page_embedded",
    "page_frame",
    "page_grid",
    "page_ids",
    "page_local_nets",
    "page_order",
    "page_reps",
    "page_slots",
    "page_table",
    "pageless_links",
    "physical_wires_of",
    "plc_rows",
    "potential_attrs",
    "potential_flow_bfs",
    "potential_name",
    "potential_partition",
    "profile_row_for",
    "rail_bonds",
    "rail_terminal",
    "rail_terminals",
    "report_block_pages",
    "report_rows",
    "settings_member",
    "solenoid_positions",
    "stitch_graph",
    "tag_class_members",
    "title_block",
    "toc_rows",
    "wire_default_props",
    "wire_numbers",
    "wire_profile_notes",
    "wire_profiles",
    "wire_props_of",
    "xref_text",
];

/// Every settings member (the keys of `settings_member`).
const ALL_MEMBERS: [Member; 8] = [
    Member::IpPairing,
    Member::ColourProfile,
    Member::SizeProfile,
    Member::WireDefault,
    Member::Numbering,
    Member::Frames,
    Member::XrefFormat,
    Member::DerivationProfile,
];

/// The keys of the snapshot a `Q` sees, for the families with a non-trivial key.
struct Keys {
    /// The report blocks; the flag marks a table of contents (its pages are keyed by their count).
    blocks: Vec<(Id<ReportBlock>, bool)>,
    cfgs: Vec<Id<PlcConfigProject>>,
    devices: Vec<Id<Device>>,
    functions: Vec<Id<Function>>,
    pages: Vec<Id<Page>>,
    reps: Vec<Id<Representation>>,
    page_refs: Vec<PageRef>,
    potentials: Vec<PotentialId>,
    /// The profile keys of the potentials, and the same attributes with each known circuit class.
    profile_keys: BTreeSet<ProfileKey>,
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
            devices: q.devices().ids().collect(),
            functions: q.functions().ids().collect(),
            pages: q.pages().ids().collect(),
            reps: q.reps().ids().collect(),
            page_refs: q.get::<PageOrder>(()).entries.iter().map(|e| e.page).collect(),
            potentials,
            profile_keys,
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
    check::<PageIds>(memo, cold, [()], o);
    check::<PagelessLinks>(memo, cold, [()], o);
    check::<RailBondsQ>(memo, cold, [()], o);
    check::<RailTerminalOf>(memo, cold, keys.devices.iter().copied(), o);
    check::<RailTerminals>(memo, cold, [()], o);
    check::<FlowNotes>(memo, cold, [()], o);
    check::<IpNotes>(memo, cold, [()], o);
    check::<PotentialAttrsOf>(memo, cold, keys.potentials.iter().copied(), o);
    check::<PotentialNameOf>(memo, cold, keys.potentials.iter().copied(), o);
    check::<ew_derive::ModuleOrder>(memo, cold, keys.cfgs.iter().copied(), o);
    check::<ew_derive::ModuleStarts>(memo, cold, keys.cfgs.iter().copied(), o);
    check::<ew_derive::ChannelAddresses>(memo, cold, keys.devices.iter().copied(), o);
    check::<ew_derive::ChannelStates>(memo, cold, keys.devices.iter().copied(), o);
    check::<ew_derive::ChannelTargets>(memo, cold, keys.devices.iter().copied(), o);
    check::<ew_derive::ChannelTexts>(memo, cold, keys.devices.iter().copied(), o);
    check::<ew_derive::SolenoidPositions>(memo, cold, keys.devices.iter().copied(), o);
    check::<ew_derive::PlcRows>(memo, cold, keys.devices.iter().copied(), o);
    check::<DevicePartLines>(memo, cold, keys.devices.iter().copied(), o);
    check::<DeviceTag>(memo, cold, keys.devices.iter().copied(), o);
    check::<DevicesByStructure>(memo, cold, [()], o);
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
    check::<SettingsMember>(memo, cold, ALL_MEMBERS, o);
    check::<StitchGraphQ>(memo, cold, [()], o);
    check::<TitleBlockValues>(memo, cold, keys.page_refs.iter().copied(), o);
    check::<TocRows>(memo, cold, [()], o);
    check::<WireDefaultProps>(memo, cold, [()], o);
    check::<WireNumbers>(memo, cold, [()], o);
    check::<WireProfileNotes>(memo, cold, [()], o);
    check::<WireProfilesQ>(memo, cold, [()], o);
    check::<WirePropsOf>(memo, cold, keys.potentials.iter().copied(), o);
    check::<XrefTextOf>(memo, cold, keys.reps.iter().copied(), o);
    // The page output (FT-59): the memoised parts of the page views and the page display list of every layer.
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
