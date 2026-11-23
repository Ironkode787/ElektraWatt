// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `edit.paste`: pastes a project fragment (`#ew-fragment v1`, the
//! project-fragment format of architecture §5.5) onto a page in one transaction.
//!
//! The fragment arrives as the command's parameter, so a command log replays a paste exactly without a
//! clipboard and without parsing text; `ew_api::parse_fragment` turns the clipboard text into that parameter.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::{Id, Uuid};
use ew_model::{LINKS, LinkDecl, Policy};
use ew_schema::{
    Connection, Device, Endpoint, GraphicShape, Junction, Kind, LibDocument, LibRef, NumberSpec, Page,
    PageItemKind, PageType, Record, Representation, StructureNode, TagScheme,
};
use ew_units::{Angle2, Grid, Nm, Orient2, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::device::{allowed_rep_types, step_axis, step_route, steps_nm};
use super::library::check_copy;
use super::library_update::{ChangeClass, classify_update, row_document};
use crate::command::{Command, Outcome};
use crate::commands::edit::BULK_ABOVE;
use crate::error::CmdError;
use crate::geometry::{OutOfRange, point_in_range, point_on_grid, rep_cps_checked, sub};
use crate::meta::{CommandMeta, Cost};
use crate::proxy::{Pt2S, UuidS};
use crate::text::{Text, text};
use crate::tx::TxBuilder;

mod insertion;
mod motion;
mod paths;

use motion::*;

pub(crate) use insertion::{Insertion, NewIds};
pub(crate) use paths::{Step, at_path, cleared, occurrences};

/// The most records one fragment (and so one paste) may hold.
pub const PASTE_MAX_RECORDS: usize = 10_000;

/// The record kinds a fragment may carry.
pub const PASTE_KINDS: [Kind; 8] = [
    Kind::Device,
    Kind::Function,
    Kind::Representation,
    Kind::Connection,
    Kind::Junction,
    Kind::PotentialDef,
    Kind::PageItem,
    Kind::StructureBox,
];

/// One structure node a fragment's records name: its source ID, aspect, the codes of its
/// path from the root down and its aliases.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FragmentNode {
    /// The node's ID in the source project.
    #[schemars(with = "UuidS")]
    pub id: Id<StructureNode>,
    /// The aspect (`location`, `function`, …), as the node record writes it.
    pub aspect: String,
    /// The codes of the node's path, root first; the last is the node's own code.
    pub path: Vec<String>,
    /// The node's aliases.
    #[serde(default)]
    pub aliases: Vec<String>,
}

/// A project fragment as one JSON object: the `$fragment` header's known members, the records,
/// the library copies and the structure nodes. The ghost rectangles of the text form are not part of it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Fragment {
    /// The project format of the records (`0.1`).
    pub format: String,
    /// The source project's ID.
    #[schemars(with = "UuidS")]
    pub project: Id<()>,
    /// The source page's connection grid `[num, den]` in nanometres.
    pub grid: [u64; 2],
    /// The source page's type (`multi_line`).
    pub page_kind: String,
    /// The anchor: the minimum corner of the copied representations' bodies, the page items' bounds and the
    /// structure boxes' outlines, snapped down to the grid.
    #[schemars(with = "Pt2S")]
    pub anchor: Pt2,
    /// The records, canonical record objects (`$k` first), in fragment order.
    pub records: Vec<Value>,
    /// The library copies the records name, in the `lib/` JSON form `library.embed` takes.
    #[serde(default)]
    pub copies: Vec<Value>,
    /// The structure nodes the records name.
    #[serde(default)]
    pub nodes: Vec<FragmentNode>,
    /// The page records of a page macro (ADR-0011 amendment 5): `macro.insert` creates the pages after
    /// its target page with proposed folios; `edit.paste` refuses them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<Value>,
    /// The library references of the closure a macro's records name (ADR-0011 amendment 5), in place of
    /// `copies`: a library item holds no copies of other items, and the session resolves these into the documents
    /// `macro.insert` carries. A paste does not read them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lib: Vec<LibRef>,
}

/// `edit.paste`: pastes a project fragment onto `page` with its anchor at `at` (doc 06 §3.6, FR-DEV-029,
/// FR-UI-036, FR-UI-040), in one transaction (one undo step removes the records and the embedded copies).
///
/// **Rejected**, at the JSON Pointer named: `/page` a missing page, a generated page or a page type that takes no
/// symbols; `/fragment/format` another project format; `/fragment/grid` a page connection grid other than the
/// fragment's for a macro inserted with an orientation (a paste is fitted, effect 5); `/at` a point that is not a
/// grid point of the page; `/fragment/anchor` an anchor off the fragment's grid;
/// `/fragment/records` none or more than 10,000; `/fragment/records/<i>` a record that does not read, of a kind
/// outside device, function, representation, connection, junction, potential definition, page item and structure
/// box, of another schema version, listed twice, a representation off the grid (neither its position nor its first
/// connection point a grid point; a fitted symbol steps by its first pin) or of a representation type the page
/// does not take, or a mandatory reference that resolves to nothing; `/fragment/copies/<i>` a copy that
/// `library.embed` refuses, listed twice, or one the project holds in another version that `adapt` does not name;
/// `/fragment/nodes/<i>` a node listed twice or without a path; `/adapt/<i>` an ID listed twice, one that names no copy
/// of the fragment the project holds in another version, or one whose change from the pasted version to the held one
/// is major.
///
/// **Effects.** (1) A library copy the project holds with the same `sha256` is reused, and so is one that `adapt`
/// names (the held version stays); any other is embedded. (2)
/// Each structure node rebinds to the project's node of the same aspect and path, else to the one node of that
/// aspect whose aliases hold the leaf code; an unmatched node clears the slots naming it (the record inherits),
/// with a notice; no node is created. (3) Every record gets a minted ID, in (kind rank, fragment order), and loses
/// its import lineage (`source_ref`: a pasted record is new, not imported); the references are found from
/// `ew_model::LINKS`: one to a fragment record takes its new ID, one to a library copy stays (effect 1), a page
/// item's or a structure box's `page` and every other page reference take the target page, a wire marker's
/// connections take the pasted connections' IDs, a structure box's `assigns` rebinds as a device's slots do, and an
/// `external` reference stays only in its own project. (4) A reference that resolves to nothing is cleared when it
/// can be: a `Cascade` link never can; otherwise the member holding it, or the optional object directly around it
/// (never a member of the record itself other than the reference), is removed when the record still reads
/// without it, with a notice; else the paste is refused. A child device whose parent is cleared also loses its
/// `position`, and a `child` tag scheme. (5) Translation in grid steps: `Δk = index(at) − index(anchor)` per
/// axis; a grid coordinate becomes the grid point at its index plus `Δk`, any other moves by
/// `round_half_away(Δk × num / den)`; routes follow their ends as `rep.move`'s `steps` move them, and a page
/// item's points and a structure box's outline vertices move by the same rule. **Fitted**: a fragment of
/// another connection grid than the page's moves by `at − anchor` in nanometres, and then the representations (each by
/// its first connection point; a symbol whose own pin spacing does not fit the grid has its first pin on it) and the
/// junctions onto the page's grid together, each to its nearest grid point or on by whole steps where that would bring
/// it nearer what it faces than it stood (`geometry::fit`, doc 06 §8 item 3), each structure box's outline vertex to
/// its nearest grid point; page items keep their place relative to the rest (a representation's texts stand
/// relative to it), and every pasted connection loses its route, to be routed anew between the same ends (the window
/// does it in the same undo step). (6) Frozen tags
/// are cleared and wire numbers become unassigned; tag counters are cleared and the copies that take part in
/// page-based numbering get the next free counter of their main page's scope from online numbering, in the order of
/// their anchor cells (FR-NUM-028), so the tags follow the pasted positions and not the minted IDs. (7) A page item of a kind this
/// version does not know is left out before any ID is minted, and a wire marker whose connection list is empty
/// after effect 4 is not inserted, each with a notice. (8) A cut and its paste are a move: a device of the target
/// project that the fragment's symbols drew and no symbol draws now (Cut left it unplaced) comes back itself
/// (`Insertion::unplaced_devices`): it, its functions and the strip of such a terminal keep their IDs, parts, frozen
/// tags, counters, levels and data and are not inserted again; the pasted symbols, wires and bridges draw them. What
/// the cut left of them goes in the same transaction: the fragment's wires and junctions the project still holds (they
/// are pasted anew) and a wire drawn on another page than the target that ends at such a device, which a notice per
/// page counts. A wire on the target page that ends at such a device follows its new pin as a drag's wires do
/// (`rep.move`, `geometry::follow_end`).
///
/// Cost: `Bulk` above 500 records. Outcome: the first pasted representation; the notices name every cleared
/// reference and unmatched structure node.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PasteFragment {
    /// The target page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// The new position of the fragment's anchor, a grid point of the page.
    #[schemars(with = "Pt2S")]
    pub at: Pt2,
    /// The fragment.
    pub fragment: Fragment,
    /// The library copies (copy IDs) the project holds in another version that the paste uses as the project holds
    /// them instead of refusing (FT-396's "Use the project's copy"); each must name a copy of
    /// `fragment.copies` that the project holds with another hash, and the change from the pasted version to the held
    /// one must not be major (`classify_update`). Empty by default, and then left out of the parameters.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<UuidS>")]
    pub adapt: Vec<Id<()>>,
}

impl Command for PasteFragment {
    const NAME: &'static str = "edit.paste";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn cost(&self, _model: &ew_model::Model) -> Cost {
        if self.fragment.records.len() > BULK_ABOVE {
            Cost::Bulk
        } else {
            Cost::Interactive
        }
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if !self.fragment.pages.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/fragment/pages",
                text!("refusal-paste-page-macro"),
            ));
        }
        let insertion = Insertion {
            name: Self::NAME,
            prefix: "/fragment",
            page: self.page,
            at: self.at,
            orient: Orient2::R0,
            fragment: &self.fragment,
            kinds: &PASTE_KINDS,
            kinds_refusal: text!("refusal-paste-kinds"),
            ids: NewIds::Mint,
            adapt: &self.adapt,
            keep_frozen: false,
            keep_counter: BTreeSet::new(),
            keep_refs: BTreeSet::new(),
            keep_unplaced: true,
            folios: None,
            bounded: true,
        };
        let out = insertion.run(tx)?;
        Ok(Outcome {
            primary: out.first_rep,
            note: None,
            notices: out.notices,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::insertion::drop_lineage;
    use super::paths::{mandatory_in, reads};
    use super::*;
    use serde_json::json;

    const A: &str = "0190a000-0000-7000-8000-000000000001";
    const B: &str = "0190a000-0000-7000-8000-000000000002";
    const C: &str = "0190a000-0000-7000-8000-000000000003";
    const D: &str = "0190a000-0000-7000-8000-000000000004";
    const E: &str = "0190a000-0000-7000-8000-000000000005";
    const F: &str = "0190a000-0000-7000-8000-000000000006";

    /// One record of each pasted kind with every link of `LINKS` from that kind filled in.
    fn samples() -> Vec<Value> {
        vec![
            json!({"$k": "device", "id": A, "tag": {"letter": "K", "counter": 1}, "parent": B, "position": 1,
                "structure": {"functional_assignment": C, "function": C, "installation_site": C, "location": C,
                    "document_kind": C, "user_defined": C, "type": C},
                "parts": [{"part": {"copy": D}, "qty": "1"}],
                "source_ref": {"lineage": E, "key": "k"},
                "strip": {"accessories": [{"after_seq": 1, "kind": "end_plate", "part": {"copy": D}}]},
                "terminal": {"strip": B, "seq": 1, "designation": "1"},
                "assembly": {"configs": [{"order_code": "X", "part": {"copy": D}, "covers": [B]}]},
                "plc_station": {"config_project": F, "role": "cpu"}}),
            json!({"$k": "function", "id": A, "device": B, "def": D,
                "kind": {"ip": {"name": "L-1", "direction": "source", "external": F}},
                "cp_overrides": [{"idx": 0, "closure": {"kind": "plug", "part": {"copy": D}}}],
                "source_ref": {"lineage": E, "key": "k"}}),
            json!({"$k": "representation", "id": A, "functions": [B], "page": C, "symbol": D, "pos": [0, 0],
                "pin_map": [{"symbol_cp": 0, "function": B, "idx": 0}],
                "source_ref": {"lineage": E, "key": "k"}}),
            json!({"$k": "connection", "id": A, "a": {"cp": [B, 0]}, "b": {"junction": C}, "page": C,
                "kind": {"core": F}, "overrides": {"wire_part": {"copy": D}},
                "source_ref": {"lineage": E, "key": "k"}}),
            json!({"$k": "connection", "id": A, "a": {"junction": B}, "b": {"cp": [C, 0]},
                "kind": {"bus": {"cable": F, "plug_a": {"part": {"copy": D}}, "plug_b": {"part": {"copy": D}}}}}),
            json!({"$k": "junction", "id": A, "page": C, "at": [0, 0], "style": "dot", "sequence": [B, F],
                "fitting": {"copy": D}, "source_ref": {"lineage": E, "key": "k"}}),
            json!({"$k": "connection", "id": A, "a": {"cp": [B, 0]}, "b": {"cp": [C, 0]},
                "kind": {"fluid": {"tube": {"part": {"copy": D}}, "fitting_a": {"copy": D}, "fitting_b": {"copy": D}}}}),
            json!({"$k": "potential_def", "id": A, "at": {"cp": [B, 0]}, "name": "P", "props": {"wire_part": {"copy": D}},
                "source_ref": {"lineage": E, "key": "k"}}),
            json!({"$k": "potential_def", "id": A, "at": {"junction": B}, "name": "P"}),
            json!({"$k": "page_item", "id": A, "page": C,
                "kind": {"wire_marker": {"connections": [B], "at": [0, 0], "fields": ["csa"]}}}),
            json!({"$k": "structure_box", "id": A, "page": C,
                "outline": [[0, 0], [1, 0], [0, 1]], "style": "dotted",
                "assigns": {"functional_assignment": C, "function": C, "installation_site": C,
                    "location": C, "document_kind": C, "user_defined": C, "type": C},
                "external": F}),
            // A surface view (left out of a paste whole, but its surface link has its row).
            json!({"$k": "page_item", "id": A, "page": C,
                "kind": {"surface_view": {"surface": B, "datum_at": [0, 0]}}}),
            json!({"$k": "page_item", "id": A, "page": C,
                "kind": {"free_text": {"bind": "strip.tag", "of": B, "at": [0, 0]}}}),
            // last, as the tests above index the samples
            json!({"$k": "function", "id": A, "device": B, "def": D,
                "bus_port": {"interface": "DP", "bus": "profibus_dp", "role": "slave", "address": "tbd",
                    "logical_net": C, "physical_net": F}}),
        ]
    }

    /// The hand oracle of spec §8.4 effect 4, per link path: `Some(true)` mandatory, `None` dropped before anything
    /// resolves. Derived from doc 03: a representation's functions and symbol copy, a function's FnDef copy, a
    /// device's part copies, a connection's ends, a terminal's strip and a potential definition's endpoint are
    /// mandatory; a device's parent, a connection's `overrides.wire_part`, the `external` references are optional.
    /// The other rows are: the record's own page
    /// (retargeted, never unresolved), the structure slots (cleared: the record inherits), an import lineage (`None`:
    /// the paste drops `source_ref`, a pasted record is new, not imported), a strip accessory's part (mandatory like a part use), an assembly configuration's part and covers (optional), the PLC
    /// station's configuration project (mandatory: copy drops the aspect), a cable core or bus cable (mandatory and
    /// optional: copy makes a core a plain wire), a junction's sequence entries (optional), a function's device
    /// (mandatory: copy brings it), a wire marker's connections (optional: a marker left empty is dropped) and a
    /// structure box's assigns and external (optional, as a device's slots and an external reference).
    const ORACLE: &[(&str, Option<bool>)] = &[
        ("device.parent", Some(false)),
        ("device.parts[].part.copy", Some(true)),
        ("device.strip.accessories[].part.copy", Some(true)),
        ("device.structure.functional_assignment", Some(false)),
        ("device.structure.function", Some(false)),
        ("device.structure.installation_site", Some(false)),
        ("device.structure.location", Some(false)),
        ("device.structure.document_kind", Some(false)),
        ("device.structure.user_defined", Some(false)),
        ("device.structure.type", Some(false)),
        ("device.terminal.strip", Some(true)),
        ("device.assembly.configs[].part.copy", Some(false)),
        ("device.assembly.configs[].covers[]", Some(false)),
        ("device.plc_station.config_project", Some(true)),
        ("device.source_ref.lineage", None),
        ("function.device", Some(true)),
        ("function.def", Some(true)),
        ("function.kind.ip.external", Some(false)),
        // A closure's part is optional (the plug enters the bill of materials at 1.0).
        ("function.cp_overrides[].closure.part.copy", Some(false)),
        // A bus port's nets are optional (a port pasted where its net is missing is on no net).
        ("function.bus_port.logical_net", Some(false)),
        ("function.bus_port.physical_net", Some(false)),
        ("function.source_ref.lineage", None),
        ("representation.functions[]", Some(true)),
        ("representation.pin_map[].function", Some(true)),
        ("representation.page", Some(true)),
        ("representation.symbol", Some(true)),
        ("representation.source_ref.lineage", None),
        ("connection.a.cp[0]", Some(true)),
        ("connection.a.junction", Some(true)),
        ("connection.b.cp[0]", Some(true)),
        ("connection.b.junction", Some(true)),
        ("connection.page", Some(true)),
        ("connection.kind.core", Some(true)),
        ("connection.kind.bus.cable", Some(false)),
        ("connection.overrides.wire_part.copy", Some(false)),
        ("connection.source_ref.lineage", None),
        ("junction.page", Some(true)),
        ("junction.sequence[]", Some(false)),
        ("junction.source_ref.lineage", None),
        ("junction.fitting.copy", Some(true)),
        ("connection.kind.fluid.tube.part.copy", Some(false)),
        ("connection.kind.fluid.fitting_a.copy", Some(false)),
        ("connection.kind.fluid.fitting_b.copy", Some(false)),
        // A bus plug is its part: a plug pasted where its part is missing cannot stand, as a part use cannot.
        ("connection.kind.bus.plug_a.part.copy", Some(true)),
        ("connection.kind.bus.plug_b.part.copy", Some(true)),
        ("potential_def.at.cp[0]", Some(true)),
        ("potential_def.at.junction", Some(true)),
        ("potential_def.props.wire_part.copy", Some(false)),
        ("potential_def.source_ref.lineage", None),
        ("page_item.kind.wire_marker.connections[]", Some(false)),
        // A view without its surface shows nothing (the link cascades); a paste leaves views out.
        ("page_item.kind.surface_view.surface", Some(true)),
        // A text bound to a symbol prints the symbol's value and goes with it (the link cascades, doc 06 §3.8).
        ("page_item.kind.free_text.of", Some(true)),
        ("page_item.page", Some(true)),
        ("structure_box.assigns.document_kind", Some(false)),
        ("structure_box.assigns.functional_assignment", Some(false)),
        ("structure_box.assigns.function", Some(false)),
        ("structure_box.assigns.installation_site", Some(false)),
        ("structure_box.assigns.location", Some(false)),
        ("structure_box.assigns.type", Some(false)),
        ("structure_box.assigns.user_defined", Some(false)),
        ("structure_box.external", Some(false)),
        ("structure_box.page", Some(true)),
    ];

    /// CB3: the mechanical rule (the link's policy, then whether the record reads without the reference) agrees
    /// with the hand oracle for every link of `LINKS` from a pasted kind, and every such link has an oracle row.
    #[test]
    fn mandatory_and_optional_follow_the_oracle_for_every_link() {
        let samples = samples();
        for s in &samples {
            if let Err(e) = Record::from_value(s.clone()) {
                panic!("the sample reads: {e} {s}");
            }
        }
        let mut checked = BTreeSet::new();
        for decl in LINKS.iter().filter(|d| PASTE_KINDS.contains(&d.from)) {
            let path = decl.path();
            let expected = ORACLE
                .iter()
                .find(|(p, _)| *p == path)
                .unwrap_or_else(|| panic!("{path} has no oracle row"))
                .1;
            let holders: Vec<&Value> = samples
                .iter()
                .filter(|s| s["$k"] == decl.from.name() && !occurrences(s, decl.field).is_empty())
                .collect();
            assert!(!holders.is_empty(), "{path}: no sample holds it");
            for s in holders {
                match expected {
                    // Dropped before resolution: the paste never meets the reference.
                    None => {
                        let mut v = s.clone();
                        drop_lineage(&mut v);
                        assert!(occurrences(&v, decl.field).is_empty(), "{path} is dropped");
                        assert!(reads(&v), "{path}: the record reads without it");
                    }
                    Some(expected) => {
                        let g = mandatory_in(decl, s).unwrap();
                        assert_eq!(
                            g, expected,
                            "{path}: mandatory by the rule {g}, by the oracle {expected}"
                        );
                    }
                }
            }
            checked.insert(path);
        }
        assert_eq!(checked.len(), ORACLE.len(), "every oracle row is a link");
    }

    #[test]
    fn a_cleared_optional_object_leaves_its_siblings() {
        let v = &samples()[3];
        let (path, _) = occurrences(v, "overrides.wire_part.copy").remove(0);
        let out = cleared(v, &path).unwrap();
        assert_eq!(out["overrides"], json!({}));
        let (path, _) = occurrences(v, "source_ref.lineage").remove(0);
        assert!(cleared(v, &path).is_none());
    }

    #[test]
    fn list_elements_are_cleared_one_by_one() {
        let v = &samples()[5];
        let occ = occurrences(v, "sequence[]");
        assert_eq!(occ.len(), 2);
        let out = cleared(v, &occ[1].0).unwrap();
        assert_eq!(out["sequence"], json!([B]));
    }

    #[test]
    fn the_steps_of_a_coordinate_follow_rep_move() {
        let g = Grid::QET_10PX;
        // On the grid: the grid point at the index plus the steps; off it: the rounded length.
        let x = g.at(4).unwrap();
        assert_eq!(step_coord(g, x, 5).unwrap(), g.at(9).unwrap());
        assert_eq!(step_coord(g, Nm(1), 1).unwrap(), Nm(1 + 2_645_833));
    }
}
