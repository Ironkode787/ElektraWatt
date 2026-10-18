// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Structure: node paths and the resolved structure of pages and devices (doc 03 §5, §8.1–§8.2 step 5,
//! FR-PRJ-019, FR-NUM-002).

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_ids::Id;
use ew_rds::{Aspect, Designation, RdsSyntax};
use ew_schema::{Device, Open, Page, Slot, StructureAssignment, StructureNode, TagScheme};

use crate::Q;
use crate::query::{Policy, Query};
use crate::tag::MainRep;

/// The aspects a device can be assigned to: every aspect except the product (the device tree itself) and
/// the document kind (pages only, excluded from device numbering; doc 03 §5).
pub const DEVICE_ASPECTS: [Aspect; 6] = [
    Aspect::FunctionalAssignment,
    Aspect::Function,
    Aspect::InstallationSite,
    Aspect::Location,
    Aspect::UserDefined,
    Aspect::Type,
];

/// The aspects a page can be assigned to: every aspect except the product.
pub const PAGE_ASPECTS: [Aspect; 7] = [
    Aspect::FunctionalAssignment,
    Aspect::Function,
    Aspect::InstallationSite,
    Aspect::Location,
    Aspect::DocumentKind,
    Aspect::UserDefined,
    Aspect::Type,
];

/// The slot of `aspect` in an assignment; `None` means inherit (absent).
pub fn slot(assign: &StructureAssignment, aspect: Aspect) -> Option<&Slot> {
    match aspect {
        Aspect::FunctionalAssignment => assign.functional_assignment.as_ref(),
        Aspect::Function => assign.function.as_ref(),
        Aspect::InstallationSite => assign.installation_site.as_ref(),
        Aspect::Location => assign.location.as_ref(),
        Aspect::DocumentKind => assign.document_kind.as_ref(),
        Aspect::UserDefined => assign.user_defined.as_ref(),
        Aspect::Type => assign.type_.as_ref(),
        Aspect::Product => None,
    }
}

/// The path of a structure node from its tree's root.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NodePath {
    /// The aspect of the tree; `None` for a node of an aspect this reader does not know.
    pub aspect: Option<Aspect>,
    /// The nodes, root first, ending with the node itself; empty for a missing node.
    pub nodes: Vec<Id<StructureNode>>,
    /// Their codes, root first: one designation level each.
    pub codes: Vec<String>,
}

/// `structure_path(node)`: the node's path from the root of its tree.
///
/// A parent cycle cannot exist (invariant STRUCTURE_ACYCLIC); the walk still stops at the first repeated
/// node, so a corrupt snapshot yields a finite path.
#[derive(Debug)]
pub struct StructurePath;

impl Query for StructurePath {
    type Key = Id<StructureNode>;
    type Value = NodePath;
    const FAMILY: &'static str = "structure_path";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Self::Key) -> Arc<NodePath> {
        let nodes = q.structure_nodes();
        let mut chain: Vec<(Id<StructureNode>, String)> = Vec::new();
        let mut aspect = None;
        let mut cur = Some(*key);
        while let Some(id) = cur {
            if chain.iter().any(|(seen, _)| *seen == id) {
                break;
            }
            let Some(node) = nodes.get(id) else { break };
            if chain.is_empty() {
                aspect = match &node.aspect {
                    Open::Known(a) => Some(*a),
                    Open::Unknown(_) => None,
                };
            }
            chain.push((id, node.code.as_str().to_owned()));
            cur = node.parent;
        }
        chain.reverse();
        Arc::new(NodePath {
            aspect,
            nodes: chain.iter().map(|(id, _)| *id).collect(),
            codes: chain.into_iter().map(|(_, c)| c).collect(),
        })
    }
}

/// Where a resolved slot came from (doc 03 §5 "Device resolution", FR-PRJ-019).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SlotSource {
    /// The record's own slot.
    Explicit,
    /// The parent device (child tags) or the strip (terminals).
    Parent,
    /// The page of the device's main representation.
    MainRepPage,
}

/// One resolved aspect.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ResolvedSlot {
    /// The node.
    pub node: Id<StructureNode>,
    /// The node's path codes, root first.
    pub codes: Vec<String>,
    /// Where it came from.
    pub source: SlotSource,
}

/// The resolved structure of a page or device: one node per assigned aspect, in canonical aspect order.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ResolvedStructure {
    /// The aspects with a node.
    pub slots: BTreeMap<Aspect, ResolvedSlot>,
}

impl ResolvedStructure {
    /// The node of an aspect.
    pub fn node(&self, aspect: Aspect) -> Option<Id<StructureNode>> {
        self.slots.get(&aspect).map(|s| s.node)
    }

    /// The designation of the aspects for which `keep` holds.
    pub fn designation(&self, mut keep: impl FnMut(Aspect) -> bool) -> Designation {
        let mut d = Designation::new();
        for (aspect, slot) in &self.slots {
            if !keep(*aspect) {
                continue;
            }
            for code in &slot.codes {
                // A stored code is a non-empty `Ident`, so `push` cannot fail.
                let _ = d.push(*aspect, code);
            }
        }
        d
    }

    /// The rendered structure (`=P01+J-E-2416`), every aspect.
    pub fn render(&self, syntax: &RdsSyntax) -> String {
        self.designation(|_| true).format(syntax)
    }

    /// One aspect rendered (`+J-E-2416`); empty when the aspect has no node.
    pub fn render_aspect(&self, aspect: Aspect, syntax: &RdsSyntax) -> String {
        self.designation(|a| a == aspect).format(syntax)
    }
}

fn resolved_slot(q: &Q, node: Id<StructureNode>, source: SlotSource) -> Option<ResolvedSlot> {
    // A slot naming a missing node, or a node of another aspect's tree, resolves to nothing (the
    // structural invariants rule both out in a valid snapshot).
    q.structure_nodes().get(node)?;
    let path = q.get::<StructurePath>(node);
    Some(ResolvedSlot {
        node,
        codes: path.codes.clone(),
        source,
    })
}

/// `page_structure(page)`: the page's own structure. Pages do not inherit: an absent, `none` or unknown
/// slot leaves the aspect empty.
#[derive(Debug)]
pub struct PageStructure;

impl Query for PageStructure {
    type Key = Id<Page>;
    type Value = ResolvedStructure;
    const FAMILY: &'static str = "page_structure";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Self::Key) -> Arc<ResolvedStructure> {
        let mut out = ResolvedStructure::default();
        let Some(page) = q.pages().get(*key) else {
            return Arc::new(out);
        };
        for aspect in PAGE_ASPECTS {
            if let Some(Slot::Node(n)) = slot(&page.structure, aspect) {
                if let Some(s) = resolved_slot(q, *n, SlotSource::Explicit) {
                    out.slots.insert(aspect, s);
                }
            }
        }
        Arc::new(out)
    }
}

/// The device a device inherits its structure from: the strip of a terminal, the parent of a child-tag
/// device.
pub(crate) fn structure_parent(dev: &Device) -> Option<Id<Device>> {
    if let Some(t) = &dev.aspects.terminal {
        return Some(t.strip);
    }
    if dev.tag.scheme == TagScheme::Child {
        return dev.parent;
    }
    None
}

/// `structure_of(device)`: the resolved structure of a device (doc 03 §8.2 step 5, FR-PRJ-019).
///
/// Per aspect of [`DEVICE_ASPECTS`]:
/// 1. the device's own slot: a node, or `none` (explicitly no node);
/// 2. absent (inherit) on a terminal or a child-tag device: the strip's or the parent's resolved slot, so
///    `-X1031:1` and `-201U3-Q1` are designations inside their parent's structure;
/// 3. absent on any other device: the slot of the page of its main representation;
/// 4. an unplaced device with a parent device: the parent's resolved slot; else no node.
///
/// Structure boxes around the main representation (FR-PRJ-020) are not in this format version; they
/// slot in between steps 1 and 3 when the record kind exists.
#[derive(Debug)]
pub struct StructureOf;

impl Query for StructureOf {
    type Key = Id<Device>;
    type Value = ResolvedStructure;
    const FAMILY: &'static str = "structure_of";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Self::Key) -> Arc<ResolvedStructure> {
        let mut out = ResolvedStructure::default();
        let Some(dev) = q.devices().get(*key) else {
            return Arc::new(out);
        };
        let inherit_parent = structure_parent(dev);
        let page = if inherit_parent.is_some() {
            None
        } else {
            q.get::<MainRep>(*key)
                .and_then(|r| q.reps().get(r).map(|r| r.page))
        };
        let parent = inherit_parent.or(if page.is_none() { dev.parent } else { None });
        let page_structure = page.map(|p| q.get::<PageStructure>(p));
        let parent_structure = parent.map(|p| q.get::<StructureOf>(p));
        for aspect in DEVICE_ASPECTS {
            let resolved = match slot(&dev.structure, aspect) {
                Some(Slot::Node(n)) => resolved_slot(q, *n, SlotSource::Explicit),
                Some(Slot::None) => None,
                // Absent or a newer form: inherit.
                Some(Slot::Unknown(_)) | None => {
                    if let Some(ps) = &page_structure {
                        ps.slots.get(&aspect).map(|s| ResolvedSlot {
                            source: SlotSource::MainRepPage,
                            ..s.clone()
                        })
                    } else if let Some(ps) = &parent_structure {
                        ps.slots.get(&aspect).map(|s| ResolvedSlot {
                            source: SlotSource::Parent,
                            ..s.clone()
                        })
                    } else {
                        None
                    }
                }
            };
            if let Some(s) = resolved {
                out.slots.insert(aspect, s);
            }
        }
        Arc::new(out)
    }
}

/// `devices_by_structure()`: every device under each structure node it resolves to (any aspect), devices
/// in ID order. A request-scoped derived index (no base index holds it).
#[derive(Debug)]
pub struct DevicesByStructure;

impl Query for DevicesByStructure {
    type Key = ();
    type Value = BTreeMap<Id<StructureNode>, Vec<Id<Device>>>;
    const FAMILY: &'static str = "devices_by_structure";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, _key: &()) -> Arc<Self::Value> {
        let mut out: BTreeMap<Id<StructureNode>, Vec<Id<Device>>> = BTreeMap::new();
        for id in q.devices().ids() {
            for slot in q.get::<StructureOf>(id).slots.values() {
                out.entry(slot.node).or_default().push(id);
            }
        }
        Arc::new(out)
    }
}
