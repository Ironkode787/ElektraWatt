// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `fields.set` (`SetFields`): the property grid's command, also over a multi-selection (doc 06 §3.1, §3.7); and
//! `fields.set_many` (`SetManyFields`, WP-S2.05 part 2): one value per target set, the command of find and replace and
//! of "freeze" on several devices, one transaction of `fields.set` steps.

use std::collections::BTreeSet;

use ew_model::Entity;
use ew_schema::{
    AddrSpec, AddrValue, AnyId, BusMedium, BusSystem, Channel, ChannelState, Conductor, Connection, Device,
    DeviceLink, DeviceLinkKind, Endpoint, ExternalRef, Function, FunctionKind, IpDirection, Kind, LogicalNet,
    NetAddress, OnOff, Page, PageItem, PageItemKind, PageType, PhysicalNet, PlcArea, PlcConfigProject,
    PlcSize, PortRole, PotentialDef, ReportBlock, Representation, Revision, Section, StructureBox,
    StructureNode,
};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::command::{Command, Outcome};
use crate::commands::common::KindName;
use crate::error::CmdError;
use crate::meta::{CommandMeta, Cost};
use crate::proxy::UuidS;
use crate::text::{Text, TextArg, text};
use crate::tx::TxBuilder;

/// The settable field paths per kind: dotted JSON member paths of the record's serde form (device aspects
/// are flattened, so `strip.label_mode` is the strip aspect's member). Identity, links that commands own
/// (`device`, `page`, `functions`, endpoints, `sequence`, `cable`, `strip`), positions and
/// `source_ref`/`extra` are not settable here; they have their own commands (`assembly.attach` sets
/// `parent` and `position`).
///
/// The device aspects `assembly`, `plc_station` and `plc_module` are set as a whole (a JSON object adds
/// or replaces the aspect, `null` removes it) or member by member once they exist (FT-169, FT-270).
/// Adding a `plc_module` (or changing the module's parts) instantiates its channels from the part data
/// (normaliser N1, `channel_instantiation`). An assembly configuration's `covers` names the assembly
/// device itself or its descendants only (doc 03 §7); any other device is rejected.
///
/// A page item's members (WP-S2.04 part 4a) are settable within its own variant only (`kind.free_text.text` on a
/// free text, refused at `/path` on a graphic), never `kind` as a whole or a wire marker's `connections` (a group is
/// changed by deleting and placing it); after the value is read back, the checks of `item.place`, `marker.place` and
/// `box.structure` apply (degenerate shapes, an unknown `bind`, an empty text without `bind`, the angles, fewer than
/// three outline vertices, a node of the wrong aspect, a missing external reference), refused at `/value`. An edit of
/// a wire marker writes a clean list: entries naming a connection that does not exist or is not on its page (a file
/// of an older build) are dropped.
///
/// A revision's members are settable while it is open; a released or withdrawn revision is immutable
/// (FR-REV-009) and refused at `/targets/<i>`. Its `state` is not settable (Open → Released is
/// `revision.release`), nor are its roles (WP-S1.18 D4, FT-402): `author` and `checker` are refused at `/path` from
/// every origin, because the author is set by `revision.set_author` and the checker by approval. A `page_notes`
/// value names existing pages, one note per page (EW-INV-019), and its notes take no member other than `page` and
/// `description`. A `label` runs EW-REL-004's prevent half as `revision.create` does for an open revision
/// (`CmdError::Prevented`): it is in the project's label format and follows every other revision's label.
///
/// An interruption point's `name`, `direction`, `sort` and `star_source` are settable on its function as
/// `kind.ip.<member>` (FT-399: renaming an interruption point from the grid); on any other function they are refused
/// at `/path`. Its `external` keeps `ip.place`'s route. A star source is a source point and an external point takes no
/// star (ID-1146): judged on each point whose direction or star the call sets, after all of the call's changes (so a
/// `fields.set_many` that clears the star and sets the direction to target passes), a starred point with another
/// direction or a starred external point refuses the whole call, naming the first such point.
///
/// The project row (kind `project`, its one ID as the target) takes `master.description`, the project
/// description that title blocks and bound free texts print (FT-447, FR-SYM-024); the other project members
/// keep their own commands.
///
/// The PLC kinds (WP-S1.21 D6, FT-226): a channel's `designation`, `address`, `state`, `symbol`, `comment` and
/// `target`; a configuration project's `name` (unique, as `plc.config_project.create` requires; its profile
/// changes only through its own command); a device link's `kind` (its devices are chosen by `device.link`). A
/// channel's `address` takes `auto`, `excluded`, `tbd` or an address text in the notation of the channel's
/// configuration project's profile (the shipped default profile while the module has no station), stored as a
/// `manual` address: `"I4.0"`; a text that does not parse is refused at `/value` with the profile's example
/// (`expected an address like Q0.1`). The stored JSON form is accepted as well. A value of a newer format
/// (an unknown state, area, size or link kind) is refused at `/value`: an edit writes only what this build knows.
///
/// The bus model (WP-S1.21 D11): a function's `bus_port` is set as a whole (a JSON object adds or replaces the
/// port, `null` removes it) or member by member once it exists (a member of a function without a port is refused
/// at `/path`); a logical net's `name`, `bus`, `master` and `rate`; a physical net's `name`, `medium` and
/// `order_hint`. Net names are unique per kind, as `bus.logical_net.create` and `bus.physical_net.create` require.
/// The nets a port names, a master's function and the functions of an `order_hint` must exist; a master is a CP. A
/// value of a newer format (a bus system, role, address form, termination or medium this build does not know) is
/// refused at `/value`.
pub const SETTABLE: &[(KindName, &[&str])] = &[
    (
        KindName::Device,
        &[
            "tag.letter",
            "tag.counter",
            "tag.frozen",
            "tag.scheme",
            "kind",
            "parts",
            "function_text",
            "process_tag",
            "tech_data",
            "state",
            "structure",
            "strip.label_mode",
            "strip.designation_scheme",
            "strip.accessories",
            "strip.tag_print",
            "strip.groups",
            "strip.field_side",
            "terminal.seq",
            "terminal.designation",
            "terminal.levels",
            "terminal.rail_bonded",
            "cable.length",
            "cable.core_id_mode",
            "cable.pre_assembled",
            "assembly",
            "assembly.configs",
            "assembly.bom_basis",
            "plc_station",
            "plc_station.config_project",
            "plc_station.role",
            "plc_station.order",
            "plc_module",
            "plc_module.slot",
            "plc_module.start",
            "plc_module.part_use",
        ],
    ),
    (
        KindName::Function,
        &[
            "cp_overrides",
            "kind.ip.name",
            "kind.ip.direction",
            "kind.ip.sort",
            "kind.ip.star_source",
            "bus_port",
            "bus_port.interface",
            "bus_port.port_label",
            "bus_port.bus",
            "bus_port.role",
            "bus_port.address",
            "bus_port.termination",
            "bus_port.logical_net",
            "bus_port.physical_net",
        ],
    ),
    (
        KindName::Page,
        &[
            "title",
            "description_override",
            "structure",
            "form",
            "scale",
            "grid",
            "kind",
        ],
    ),
    (KindName::Representation, &["text_slots", "arrangement"]),
    (
        KindName::Connection,
        &[
            "kind",
            // WP-S1.20 D18: the line kind of a fluid line (`kind.fluid.line_kind`), refused on another connection.
            "line_kind",
            "number",
            "overrides.csa",
            "overrides.colour",
            "overrides.wire_part",
            "overrides.insulation",
            "overrides.function",
            "overrides.flags",
            "overrides.show_marker",
            "overrides.length",
        ],
    ),
    (KindName::Conductor, &["core", "colour", "csa", "shield", "pair"]),
    (
        KindName::PotentialDef,
        &["name", "ptype", "voltage", "env", "circuit_class", "props"],
    ),
    (
        KindName::StructureNode,
        &["code", "aliases", "description", "order"],
    ),
    (KindName::Section, &["title", "divider", "folio_range"]),
    (KindName::Project, &["master.description"]),
    (
        KindName::ReportBlock,
        &[
            "title",
            "first_folio",
            "sort",
            "group",
            "continuation",
            "empty",
            "manual",
        ],
    ),
    (KindName::ExternalRef, &["project", "device", "connection"]),
    (
        KindName::Revision,
        &["label", "date", "description", "page_notes"],
    ),
    (
        KindName::PageItem,
        &[
            "layer",
            "kind.graphic.shape",
            "kind.graphic.stroke",
            "kind.graphic.dash",
            "kind.graphic.fill",
            "kind.free_text.text",
            "kind.free_text.bind",
            "kind.free_text.at",
            "kind.free_text.angle",
            "kind.free_text.style",
            "kind.free_text.align",
            "kind.wire_marker.at",
            "kind.wire_marker.angle",
        ],
    ),
    (
        KindName::StructureBox,
        &["outline", "style", "caption", "assigns", "external"],
    ),
    (
        KindName::Channel,
        &["designation", "address", "state", "symbol", "comment", "target"],
    ),
    (KindName::PlcConfigProject, &["name"]),
    (KindName::DeviceLink, &["kind"]),
    (KindName::LogicalNet, &["name", "bus", "master", "rate"]),
    (KindName::PhysicalNet, &["name", "medium", "order_hint"]),
];

/// The aspect members of a device: a path below one of them needs the aspect to exist.
pub const ASPECTS: &[&str] = &[
    "strip",
    "terminal",
    "cable",
    "assembly",
    "plc_station",
    "plc_module",
];

/// `fields.set` (`SetFields`): sets one field on one or more records of one kind.
///
/// For each target in order: the record's serde JSON form gets `value` at `path` (a JSON `null` removes the
/// member, which resets it to its default); intermediate objects are created, except that a path below a
/// device aspect ([`ASPECTS`]) requires the aspect to exist; the JSON is read back into the record type
/// and put. Rejected with [`CmdError::InvalidParams`] at `/path`: a path not in [`SETTABLE`] for the kind;
/// at `/value`: a value the record type does not accept (the deserialisation error as message), and a
/// connection's `overrides.colour` or a conductor's `colour` with a code that is neither IEC 60757 nor a company
/// code of the project's colour profile (FR-CON-019, `marker.place`'s check and text; FT-451, FT-476). Targets
/// must exist and be distinct; there is at least one. Invariants and prevent rules apply at the end of the
/// transaction, so a swap of two terminal designations over two commands is not possible here, but one
/// `fields.set` that leaves a duplicate is caught later by prevent rules (ADR-0009). Outcome: none.
/// Coalesce typing in one cell with a key (FR-UI-009). A page whose kind moves away from `panel_layout` loses its
/// surface views in the same transaction (FT-869; `fields.set_many` judges the page's final kind).
///
/// `copies` (WP-S2.07 D16, as `device.place` since FT-656): library documents (`lib/` JSON form) that the value names
/// and the project lacks (a catalogue part chosen for a strip's bridge or accessory), embedded first, in the same
/// transaction, as `part.assign` embeds its closure: each is checked as `library.embed` checks it, at `/copies/i`; a
/// copy the project holds with the same hash is reused, one it holds with another hash (another version of the item)
/// is refused ("update from library first").
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetFields {
    /// The kind of every target.
    pub kind: KindName,
    /// The records.
    #[schemars(with = "Vec<UuidS>")]
    pub targets: Vec<AnyId>,
    /// The dotted field path.
    pub path: String,
    /// The new value in the field's JSON encoding; `null` resets it.
    pub value: Value,
    /// The library documents to embed first (`lib/` JSON form): what the value names that the project lacks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
}

impl Command for SetFields {
    const NAME: &'static str = "fields.set";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        crate::commands::assign::embed_closure(Self::NAME, tx, &self.copies)?;
        self.apply(tx)?;
        refuse_bad_stars(tx, std::slice::from_ref(self))?;
        if self.kind == KindName::Page {
            drop_left_surface_views(tx, &self.targets)?;
        }
        Ok(Outcome::default())
    }
}

/// FT-869 (ID-1080): a page of `targets` whose kind this command moves away from `panel_layout` (judged on the
/// page's kind at the end of the command against the base) loses its surface views in the same transaction, so the
/// preview lists them and one undo restores them. A page without a view, or one that ends as a panel layout, keeps
/// everything.
fn drop_left_surface_views(tx: &mut TxBuilder<'_>, targets: &[AnyId]) -> Result<(), CmdError> {
    let mut views = Vec::new();
    for t in targets {
        let page: ew_ids::Id<Page> = t.cast();
        let was_layout = tx
            .base_row(&(Kind::Page, *t))
            .and_then(|r| r.get::<Page>().map(|p| p.kind == PageType::PanelLayout))
            .unwrap_or(false);
        let is_layout = tx
            .get::<Page>(page)
            .is_some_and(|p| p.kind == PageType::PanelLayout);
        if !was_layout || is_layout {
            continue;
        }
        for id in tx.page_items_on_page(page) {
            if tx
                .get::<PageItem>(id)
                .is_some_and(|i| matches!(i.kind, PageItemKind::SurfaceView(_)))
            {
                views.push((Kind::PageItem, id.cast()));
            }
        }
    }
    tx.delete_many(&views)
}

impl SetFields {
    /// The edit of [`SetFields`] on `tx`, with `fields.set`'s pointers (`fields.set_many` prefixes them).
    fn apply(&self, tx: &mut TxBuilder<'_>) -> Result<(), CmdError> {
        if self.targets.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/targets",
                text!("refusal-params-no-target"),
            ));
        }
        let mut seen = BTreeSet::new();
        for (i, t) in self.targets.iter().enumerate() {
            if !seen.insert(*t) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/targets/{i}"),
                    text!("refusal-params-target-twice"),
                ));
            }
        }
        if self.kind == KindName::Revision && matches!(self.path.as_str(), "author" | "checker") {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/path",
                text!("refusal-fields-revision-party", self.path.to_string()),
            ));
        }
        let allowed = SETTABLE
            .iter()
            .find(|(k, _)| *k == self.kind)
            .map_or(&[][..], |(_, paths)| *paths);
        if !allowed.contains(&self.path.as_str()) {
            let kind = self.kind.kind();
            let message = if allowed.is_empty() {
                text!(
                    "refusal-fields-not-settable-kind",
                    self.path.clone(),
                    kind.to_string()
                )
            } else {
                text!(
                    "refusal-fields-not-settable",
                    self.path.clone(),
                    kind.to_string(),
                    allowed.join(", ")
                )
            };
            return Err(CmdError::invalid_params(Self::NAME, "/path", message));
        }
        let (path, value) = (self.path.as_str(), &self.value);
        if self.kind == KindName::Revision {
            for (i, t) in self.targets.iter().enumerate() {
                crate::commands::revision::refuse_closed_revision(
                    tx,
                    Self::NAME,
                    &format!("/targets/{i}"),
                    t.cast(),
                )?;
            }
        }
        for t in &self.targets {
            match self.kind {
                KindName::Device => set_one::<Device>(tx, *t, path, value)?,
                KindName::Function => {
                    refuse_not_ip(tx, *t, path)?;
                    refuse_no_port(tx, *t, path)?;
                    set_one::<Function>(tx, *t, path, value)?;
                    if path == "bus_port" || path.starts_with("bus_port.") {
                        check_port(tx, t.cast())?;
                    }
                    if path == "kind.ip.direction" {
                        refuse_unknown_direction(tx, *t)?;
                    }
                }
                KindName::Page => {
                    set_one::<Page>(tx, *t, path, value)?;
                    if path == "scale" {
                        refuse_bad_scale(tx, *t)?;
                    }
                }
                KindName::Representation => set_one::<Representation>(tx, *t, path, value)?,
                KindName::Connection if path == "line_kind" => set_line_kind(tx, t.cast(), value)?,
                KindName::Connection => {
                    set_one::<Connection>(tx, *t, path, value)?;
                    if path == "overrides.colour" {
                        refuse_unknown_colour(tx, t.cast())?;
                    }
                }
                KindName::Conductor => {
                    set_one::<Conductor>(tx, *t, path, value)?;
                    if path == "colour" {
                        refuse_unknown_conductor_colour(tx, t.cast())?;
                    }
                }
                KindName::PotentialDef => set_one::<PotentialDef>(tx, *t, path, value)?,
                KindName::StructureNode => set_one::<StructureNode>(tx, *t, path, value)?,
                KindName::Section => {
                    set_one::<Section>(tx, *t, path, value)?;
                    // FT-1510 (WP-S6.08 D27): a divider onto a folio a page or a page-oriented report block of the
                    // section holds is refused, as `page.create` and `report_block.create` keep the divider's free.
                    if path == "divider" || path == "folio_range" {
                        refuse_held_divider(tx, t.cast())?;
                    }
                }
                KindName::ReportBlock => {
                    set_one::<ReportBlock>(tx, *t, path, value)?;
                    // ID-1417 (C52-p2#1): a first folio or a continuation whose generated pages a page of the section
                    // already holds is refused, as `report_block.create` refuses it.
                    if path == "first_folio" || path == "continuation" {
                        let b = tx.require::<ReportBlock>(t.cast())?;
                        if let (None, Some(first)) = (&b.embedded, b.first_folio) {
                            let held = crate::commands::structure::held_folio(
                                tx,
                                b.section,
                                first,
                                &b.continuation,
                                Some(b.id),
                            );
                            if let Some(held) = held {
                                return tx.reject(text!("refusal-page-create-exists", held));
                            }
                        }
                    }
                }
                KindName::ExternalRef => set_one::<ExternalRef>(tx, *t, path, value)?,
                KindName::Revision => {
                    if path == "page_notes" {
                        crate::commands::revision::refuse_open_page_notes(Self::NAME, value)?;
                    }
                    if path == "label" {
                        crate::commands::revision::refuse_label_edit_out_of_order(tx, t.cast(), value)?;
                    }
                    set_one::<Revision>(tx, *t, path, value)?;
                    if path == "page_notes" {
                        let notes = tx.require::<Revision>(t.cast())?.page_notes.clone();
                        crate::commands::revision::check_page_notes(tx, Self::NAME, "/value", &notes)?;
                    }
                }
                KindName::PageItem => {
                    refuse_other_variant(tx, *t, path)?;
                    set_one::<PageItem>(tx, *t, path, value)?;
                    let mut item = (*tx.require::<PageItem>(t.cast())?).clone();
                    if let Some(p) = crate::commands::items::item_problem(&item) {
                        return Err(CmdError::invalid_params(Self::NAME, "/value", p));
                    }
                    let before = item.clone();
                    crate::commands::items::clean_marker(tx, &mut item);
                    if item != before {
                        tx.put(item);
                    }
                }
                KindName::StructureBox => {
                    set_one::<StructureBox>(tx, *t, path, value)?;
                    let b = tx.require::<StructureBox>(t.cast())?;
                    crate::commands::structure_box::check_box(tx, Self::NAME, "", &b).map_err(
                        |e| match e {
                            CmdError::InvalidParams { text, .. } => {
                                CmdError::invalid_params(Self::NAME, "/value", *text)
                            }
                            other => other,
                        },
                    )?;
                }
                KindName::Project => set_project_member(tx, *t, path, value)?,
                KindName::Channel => {
                    let value = if path == "address" {
                        channel_address_value(tx, t.cast(), value)?
                    } else {
                        value.clone()
                    };
                    set_one::<Channel>(tx, *t, path, &value)?;
                    refuse_unknown_channel_member(tx, t.cast())?;
                }
                KindName::PlcConfigProject => {
                    set_one::<PlcConfigProject>(tx, *t, path, value)?;
                    refuse_duplicate_config_name(tx, t.cast())?;
                }
                KindName::DeviceLink => {
                    set_one::<DeviceLink>(tx, *t, path, value)?;
                    let l = tx.require::<DeviceLink>(t.cast())?;
                    if let DeviceLinkKind::Unknown(v) = &l.kind {
                        return Err(CmdError::invalid_params(
                            Self::NAME,
                            "/value",
                            text!("refusal-fields-link-kind-unknown", v.to_string()),
                        ));
                    }
                }
                KindName::LogicalNet => {
                    set_one::<LogicalNet>(tx, *t, path, value)?;
                    check_logical_net(tx, t.cast())?;
                }
                KindName::PhysicalNet => {
                    set_one::<PhysicalNet>(tx, *t, path, value)?;
                    check_physical_net(tx, t.cast())?;
                }
                // Junctions, bonds and the layout kinds (WP-S4.02 D7) have no settable fields: rejected above.
                KindName::Junction
                | KindName::Bond
                | KindName::LayoutSpace
                | KindName::Enclosure
                | KindName::MountingSurface
                | KindName::Carrier
                | KindName::Placement => {
                    return Err(CmdError::internal("fields.set: kind without settable fields"));
                }
            }
        }
        if self.kind == KindName::Device && (path == "assembly" || path.starts_with("assembly.")) {
            for t in &self.targets {
                refuse_foreign_covers(tx, t.cast())?;
            }
        }
        Ok(())
    }
}

/// [`set_one`] for the project row, which is not a table entity: the one target must be the project's ID (else not
/// found); the member is set on the record's JSON form and read back, refused at `/value` as [`set_one`] refuses.
fn set_project_member(
    tx: &mut TxBuilder<'_>,
    target: AnyId,
    path: &str,
    value: &Value,
) -> Result<(), CmdError> {
    let project = tx.project();
    if project.id.cast::<()>() != target {
        return Err(CmdError::NotFound {
            key: (Kind::Project, target),
        });
    }
    let mut json = serde_json::to_value(&*project)
        .map_err(|e| CmdError::internal(format!("fields.set: cannot serialise the project ({e})")))?;
    let segments: Vec<&str> = path.split('.').collect();
    if !set_member(&mut json, &segments, value) {
        return Err(CmdError::invalid_params(
            SetFields::NAME,
            "/path",
            text!("refusal-fields-below-non-object", path.to_string()),
        ));
    }
    let next: ew_schema::Project = serde_json::from_value(json).map_err(|e| {
        CmdError::invalid_params(
            SetFields::NAME,
            "/value",
            text!("refusal-params-detail", e.to_string()),
        )
    })?;
    tx.set_project(next)
}

/// FR-CON-019 for `fields.set` on a connection's `overrides.colour` (FT-451): the colour as read back must be IEC
/// 60757 codes or company codes of the project's colour profile, the same check and text as `marker.place`'s;
/// refused at `/value`. A removed colour (`null`) passes.
fn refuse_unknown_colour(tx: &TxBuilder<'_>, conn: ew_ids::Id<Connection>) -> Result<(), CmdError> {
    let c = tx.require::<Connection>(conn)?;
    if let Some(colour) = &c.overrides.colour {
        let company = crate::commands::items::company_codes(tx);
        if let Some(p) = crate::commands::items::colour_problem(&company, colour) {
            return Err(CmdError::invalid_params(SetFields::NAME, "/value", p));
        }
    }
    Ok(())
}

/// `fields.set` on a section's `divider` or `folio_range` (FT-1510, WP-S6.08 D27): with a divider, the range's first
/// folio, which the divider page prints, may not be printed by a page of the section or be the first folio of one of
/// its page-oriented report blocks.
fn refuse_held_divider(tx: &TxBuilder<'_>, section: ew_ids::Id<Section>) -> Result<(), CmdError> {
    let s = tx.require::<Section>(section)?;
    if !s.divider {
        return Ok(());
    }
    let first = s.folio_range.0;
    let head = first.to_string();
    let page = crate::commands::page::section_pages(tx, Some(s.id))
        .into_iter()
        .any(|(_, f)| crate::commands::page::printed_folio(f.number, f.label.as_ref()) == head);
    if page {
        return tx.reject(text!(
            "refusal-fields-section-divider-page",
            head,
            s.number.to_string()
        ));
    }
    let report = tx
        .iter::<ReportBlock>()
        .iter()
        .any(|b| b.section == Some(s.id) && b.embedded.is_none() && b.first_folio == Some(first));
    if report {
        return tx.reject(text!(
            "refusal-fields-section-divider-report",
            head,
            s.number.to_string()
        ));
    }
    Ok(())
}

/// `fields.set` on a page's `scale` (FT-1065): the scale as read back must be one `page.surface_view.set` takes, with
/// its text ([`crate::commands::page::scale_problem`]); refused at `/value`.
fn refuse_bad_scale(tx: &TxBuilder<'_>, page: AnyId) -> Result<(), CmdError> {
    let p = tx.require::<Page>(page.cast())?;
    match crate::commands::page::scale_problem(&p.scale) {
        Some(problem) => Err(CmdError::invalid_params(SetFields::NAME, "/value", problem)),
        None => Ok(()),
    }
}

/// [`refuse_unknown_colour`] for a conductor's `colour` (FT-476): the same check and text, refused at `/value`.
fn refuse_unknown_conductor_colour(
    tx: &TxBuilder<'_>,
    conductor: ew_ids::Id<Conductor>,
) -> Result<(), CmdError> {
    let c = tx.require::<Conductor>(conductor)?;
    if let Some(colour) = &c.colour {
        let company = crate::commands::items::company_codes(tx);
        if let Some(p) = crate::commands::items::colour_problem(&company, colour) {
            return Err(CmdError::invalid_params(SetFields::NAME, "/value", p));
        }
    }
    Ok(())
}

// WP-S1.21 D6: the PLC kinds ------------------------------------------------------------------------------------

/// The value `fields.set` writes for a channel's `address`: the words `auto`, `excluded` and `tbd` (in any ASCII
/// case, trimmed) as they are; any other text parsed by the profile of the channel's configuration project into a
/// `manual` address, refused at `/value` with the profile's example when it does not parse; a JSON value other
/// than a string unchanged (the stored form, or `null` for `auto`).
fn channel_address_value(
    tx: &TxBuilder<'_>,
    channel: ew_ids::Id<Channel>,
    value: &Value,
) -> Result<Value, CmdError> {
    let Value::String(text) = value else {
        return Ok(value.clone());
    };
    let word = text.trim().to_ascii_lowercase();
    if matches!(word.as_str(), "auto" | "excluded" | "tbd") {
        return Ok(Value::String(word));
    }
    let c = tx.require::<Channel>(channel)?;
    let profile = channel_profile(tx, c.module);
    let address = ew_derive::parse_plc_address(&profiles_in(tx), &profile, text).map_err(|m| {
        CmdError::invalid_params(SetFields::NAME, "/value", text!("refusal-params-detail", m))
    })?;
    serde_json::to_value(AddrSpec::Manual(address))
        .map_err(|e| CmdError::internal(format!("fields.set: cannot serialise an address ({e})")))
}

/// The address profiles at hand in the transaction's model (WP-S7.03 D8): the built-ins, then the project's copies.
pub(crate) fn profiles_in(tx: &TxBuilder<'_>) -> Vec<std::sync::Arc<ew_derive::ew_plcaddr::AddressProfile>> {
    let copies = tx.iter::<ew_schema::LibCopy<ew_schema::PlcProfileDef>>();
    ew_derive::profiles_at_hand(copies.iter().map(|c| &**c))
}

/// The profile reference that reads a channel's address texts: the profile of the configuration project of the
/// module's station (the module device itself or its nearest ancestor with `plc_station`), else the shipped
/// default profile (a module without a station, or a station whose configuration project is gone).
pub(crate) fn channel_profile(tx: &TxBuilder<'_>, module: ew_ids::Id<Device>) -> String {
    let mut at = Some(module);
    let mut steps = 0usize;
    while let Some(d) = at.and_then(|d| tx.get::<Device>(d)) {
        if let Some(station) = &d.aspects.plc_station {
            return tx
                .get::<PlcConfigProject>(station.config_project)
                .map_or_else(ew_derive::default_profile, |c| c.profile.to_string());
        }
        steps += 1;
        if steps > 4096 {
            break; // a parent cycle that the invariants report (EW-INV-009)
        }
        at = d.parent;
    }
    ew_derive::default_profile()
}

/// A channel member of a newer format as read back (an unknown state, an address form, area or size this build
/// does not know): refused at `/value`, since an edit writes only what this build can judge.
fn refuse_unknown_channel_member(tx: &TxBuilder<'_>, channel: ew_ids::Id<Channel>) -> Result<(), CmdError> {
    let c = tx.require::<Channel>(channel)?;
    let unknown_address = match &c.address {
        AddrSpec::Unknown(_) => true,
        AddrSpec::Manual(a) => matches!(a.area, PlcArea::Unknown(_)) || matches!(a.size, PlcSize::Unknown(_)),
        AddrSpec::Auto | AddrSpec::Excluded | AddrSpec::Tbd => false,
    };
    if unknown_address {
        return Err(CmdError::invalid_params(
            SetFields::NAME,
            "/value",
            text!("refusal-fields-address-unknown"),
        ));
    }
    if let Some(ChannelState::Unknown(v)) = &c.state {
        return Err(CmdError::invalid_params(
            SetFields::NAME,
            "/value",
            text!("refusal-fields-channel-state-unknown", v.to_string()),
        ));
    }
    Ok(())
}

/// A configuration project's name as `plc.config_project.create` keeps it: unique among the configuration
/// projects (compared exactly), else rejected.
fn refuse_duplicate_config_name(
    tx: &TxBuilder<'_>,
    cfg: ew_ids::Id<PlcConfigProject>,
) -> Result<(), CmdError> {
    let c = tx.require::<PlcConfigProject>(cfg)?;
    if tx
        .iter::<PlcConfigProject>()
        .into_iter()
        .any(|o| o.id != cfg && o.name == c.name)
    {
        return tx.reject(text!("refusal-plc-project-exists", c.name.to_string()));
    }
    Ok(())
}

// WP-S2.05 part 2 ------------------------------------------------------------------------------------------------

/// The most entries one `fields.set_many` takes.
pub const SET_MANY_MAX_EDITS: usize = 10_000;

/// `fields.set_many` (`SetManyFields`, WP-S2.05 part 2; FR-DEV-032, FR-UI-020): sets one value per (kind, targets,
/// path) entry, the entries in the given order, in **one** transaction (one undo step).
///
/// Each entry is applied exactly as `fields.set` with those parameters would be: the same [`SETTABLE`] paths, the same
/// per-kind checks (closed revisions, aspects, assembly covers, the tag-collision refusal) and the same rejections,
/// their JSON Pointer prefixed by `/edits/<i>`. The caller computes every value (find and replace computes the
/// replaced texts, "freeze" each device's own product designation): the command searches nothing and reads no derived
/// value, so a command log replays it exactly. Rejected with [`CmdError::InvalidParams`] at `/edits`: no entry, or
/// more than 10,000 entries; at `/edits/<j>/targets/<k>`: a (target, path) pair that an earlier entry already names.
/// Cost: `Bulk` when the entries name more than 500 targets in total (`BULK_ABOVE`, as `edit.delete`),
/// else interactive. Outcome: none.
///
/// `copies` (WP-S2.07 D16): library documents the values name that the project lacks, embedded first as
/// `fields.set`'s `copies` (refused at `/copies/i`); an entry carries none of its own (refused at `/edits/<i>/copies`).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetManyFields {
    /// The entries, each the parameters of one `fields.set` (1 to 10,000), without `copies`.
    pub edits: Vec<SetFields>,
    /// The library documents to embed first (`lib/` JSON form): what the values name that the project lacks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
}

impl Command for SetManyFields {
    const NAME: &'static str = "fields.set_many";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn cost(&self, _model: &ew_model::Model) -> Cost {
        let targets: usize = self.edits.iter().map(|e| e.targets.len()).sum();
        if targets > crate::commands::BULK_ABOVE {
            Cost::Bulk
        } else {
            Cost::Interactive
        }
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.edits.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/edits",
                text!("refusal-params-no-entry"),
            ));
        }
        if self.edits.len() > SET_MANY_MAX_EDITS {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/edits",
                text!(
                    "refusal-fields-set-many-too-many",
                    self.edits.len().to_string(),
                    SET_MANY_MAX_EDITS.to_string()
                ),
            ));
        }
        // One (target, path) pair per command: a later entry naming it again is refused before anything is applied.
        let mut pairs: BTreeSet<(AnyId, &str)> = BTreeSet::new();
        for (j, e) in self.edits.iter().enumerate() {
            let mut own = BTreeSet::new();
            for (k, t) in e.targets.iter().enumerate() {
                // A target twice within one entry is `fields.set`'s own refusal, at its own pointer below.
                if own.insert(*t) && !pairs.insert((*t, e.path.as_str())) {
                    return Err(CmdError::invalid_params(
                        Self::NAME,
                        format!("/edits/{j}/targets/{k}"),
                        text!("refusal-fields-set-many-pair-twice", e.path.to_string()),
                    ));
                }
            }
        }
        if let Some(i) = self.edits.iter().position(|e| !e.copies.is_empty()) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                format!("/edits/{i}/copies"),
                text!("refusal-fields-copies-in-entry"),
            ));
        }
        crate::commands::assign::embed_closure(Self::NAME, tx, &self.copies)?;
        for (i, e) in self.edits.iter().enumerate() {
            e.apply(tx).map_err(|err| match err {
                CmdError::InvalidParams { path, text, .. } => {
                    CmdError::invalid_params(Self::NAME, format!("/edits/{i}{path}"), *text)
                }
                other => other,
            })?;
        }
        refuse_bad_stars(tx, &self.edits)?;
        // FT-869: judged on each page's final kind; a batch that sets the kind away and back is refused (one entry per
        // target and path), so the view stays.
        let pages: BTreeSet<AnyId> = self
            .edits
            .iter()
            .filter(|e| e.kind == KindName::Page)
            .flat_map(|e| e.targets.iter().copied())
            .collect();
        drop_left_surface_views(tx, &pages.into_iter().collect::<Vec<_>>())?;
        Ok(Outcome::default())
    }
}

/// Doc 03 §7: an assembly configuration covers the assembly device itself or its descendants, each named
/// explicitly. A covered device outside that subtree that **this edit adds** (absent from every
/// configuration of the base row) is refused at `/value`; a cover that was out of scope already (a child
/// moved away by `assembly.attach`) is a check finding (decision log ID-371, FT-260), never a refusal, so an
/// unrelated assembly edit such as `assembly.bom_basis` still commits.
fn refuse_foreign_covers(tx: &TxBuilder<'_>, device: ew_ids::Id<Device>) -> Result<(), CmdError> {
    let Some(dev) = tx.get::<Device>(device) else {
        return Ok(());
    };
    let Some(assembly) = &dev.aspects.assembly else {
        return Ok(());
    };
    let before: std::collections::BTreeSet<ew_ids::Id<Device>> = tx
        .base_row(&(Kind::Device, device.cast()))
        .and_then(|r| r.get::<Device>().cloned())
        .and_then(|d| d.aspects.assembly.clone())
        .map(|a| a.configs.iter().flat_map(|c| c.covers.iter().copied()).collect())
        .unwrap_or_default();
    for config in &assembly.configs {
        for covered in config.covers.iter().filter(|c| !before.contains(*c)) {
            // walk up from the covered device; the chain is acyclic (EW-INV-009), bounded anyway
            let mut at = Some(*covered);
            let mut steps = 0usize;
            let inside = loop {
                match at {
                    Some(d) if d == device => break true,
                    Some(d) if steps <= 4096 => {
                        at = tx.get::<Device>(d).and_then(|x| x.parent);
                        steps += 1;
                    }
                    _ => break false,
                }
            };
            if !inside {
                return Err(CmdError::invalid_params(
                    SetFields::NAME,
                    "/value",
                    text!(
                        "refusal-fields-config-covers-other",
                        config.order_code.to_string(),
                        crate::words::device(tx, *covered),
                        crate::words::device(tx, device)
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// A page-item path names a variant (`kind.<variant>.<member>`): it must be the record's own (WP-S2.04 §5.2).
fn refuse_other_variant(tx: &TxBuilder<'_>, target: AnyId, path: &str) -> Result<(), CmdError> {
    let Some(variant) = path.strip_prefix("kind.").and_then(|r| r.split('.').next()) else {
        return Ok(());
    };
    let item = tx.require::<PageItem>(target.cast())?;
    let own = item.kind.name().unwrap_or("");
    if own != variant {
        return Err(CmdError::invalid_params(
            SetFields::NAME,
            "/path",
            text!(
                "refusal-fields-item-variant",
                own.to_string(),
                variant.to_string(),
                path.to_string()
            ),
        ));
    }
    Ok(())
}

/// An interruption point's members (`kind.ip.*`, FT-399) on a function that is not one: refused at `/path`.
fn refuse_not_ip(tx: &TxBuilder<'_>, target: AnyId, path: &str) -> Result<(), CmdError> {
    if !path.starts_with("kind.ip.") {
        return Ok(());
    }
    let f = tx.require::<Function>(target.cast())?;
    if !matches!(f.kind, FunctionKind::Ip(_)) {
        return Err(CmdError::invalid_params(
            SetFields::NAME,
            "/path",
            text!(
                "refusal-fields-not-ip",
                crate::words::function(tx, target.cast()),
                path.to_string()
            ),
        ));
    }
    Ok(())
}

// WP-S1.21 D11: the bus kinds ------------------------------------------------------------------------------------

/// A member of a function's `bus_port` needs the port to exist (as a device aspect's members do): refused at `/path`.
fn refuse_no_port(tx: &TxBuilder<'_>, target: AnyId, path: &str) -> Result<(), CmdError> {
    if !path.starts_with("bus_port.") {
        return Ok(());
    }
    let f = tx.require::<Function>(target.cast())?;
    if f.bus_port.is_none() {
        // "function -Q1 has no …", or "a function without a device has no …": the fallback is its own subject.
        let subject = match crate::words::function(tx, target.cast()) {
            TextArg::Plain(tag) => text!("refusal-fields-function-named", tag).into(),
            other => other,
        };
        return Err(CmdError::invalid_params(
            SetFields::NAME,
            "/path",
            text!("refusal-fields-no-port", subject),
        ));
    }
    Ok(())
}

/// The refusal of a value of a newer format at `/value`: an edit writes only what this build knows.
fn newer(text: Text) -> CmdError {
    CmdError::invalid_params(SetFields::NAME, "/value", text)
}

/// A function's bus port as read back: every enum value known to this build, and the nets it names exist.
fn check_port(tx: &TxBuilder<'_>, function: ew_ids::Id<Function>) -> Result<(), CmdError> {
    let f = tx.require::<Function>(function)?;
    let Some(port) = &f.bus_port else {
        return Ok(());
    };
    if let BusSystem::Unknown(v) = &port.bus {
        return Err(crate::commands::bus::unknown_bus(SetFields::NAME, "/value", v));
    }
    if let PortRole::Unknown(v) = &port.role {
        return Err(newer(text!("refusal-fields-port-role-unknown", v.to_string())));
    }
    if let AddrValue::Unknown(v) | AddrValue::Value(NetAddress::Unknown(v)) = &port.address {
        return Err(newer(text!("refusal-fields-bus-address-unknown", v.to_string())));
    }
    if let Some(OnOff::Unknown(v)) = &port.termination {
        return Err(newer(text!("refusal-fields-termination-unknown", v.to_string())));
    }
    if let Some(n) = port.logical_net {
        tx.require::<LogicalNet>(n)?;
    }
    if let Some(n) = port.physical_net {
        tx.require::<PhysicalNet>(n)?;
    }
    Ok(())
}

/// A logical net as read back: a bus system this build knows, a unique name, a rate above 0, and a master that is
/// the CP of an existing function.
fn check_logical_net(tx: &TxBuilder<'_>, net: ew_ids::Id<LogicalNet>) -> Result<(), CmdError> {
    let n = tx.require::<LogicalNet>(net)?;
    if let BusSystem::Unknown(v) = &n.bus {
        return Err(crate::commands::bus::unknown_bus(SetFields::NAME, "/value", v));
    }
    if n.rate == Some(ew_schema::BusRate(0)) {
        return Err(CmdError::invalid_params(
            SetFields::NAME,
            "/value",
            text!("refusal-fields-zero-rate"),
        ));
    }
    match &n.master {
        None => {}
        Some(Endpoint::Cp(f, _)) => {
            tx.require::<Function>(*f)?;
        }
        Some(_) => {
            return Err(CmdError::invalid_params(
                SetFields::NAME,
                "/value",
                text!("refusal-fields-master-cp"),
            ));
        }
    }
    crate::commands::bus::refuse_duplicate_logical_name(tx, Some(net), &n.name)
}

/// A physical net as read back: a medium this build knows, a unique name, and an `order_hint` of existing
/// functions.
fn check_physical_net(tx: &TxBuilder<'_>, net: ew_ids::Id<PhysicalNet>) -> Result<(), CmdError> {
    let n = tx.require::<PhysicalNet>(net)?;
    if let BusMedium::Unknown(v) = &n.medium {
        return Err(newer(text!("refusal-bus-medium-unknown", v.to_string())));
    }
    for f in &n.order_hint {
        tx.require::<Function>(*f)?;
    }
    crate::commands::bus::refuse_duplicate_physical_name(tx, Some(net), &n.name)
}

/// An interruption point's direction as read back must be one this build knows (`IpDirection` keeps a newer word
/// verbatim for files, never for an edit): refused at `/value`.
fn refuse_unknown_direction(tx: &TxBuilder<'_>, target: AnyId) -> Result<(), CmdError> {
    let f = tx.require::<Function>(target.cast())?;
    if let FunctionKind::Ip(ip) = &f.kind
        && let IpDirection::Unknown(v) = &ip.direction
    {
        return Err(CmdError::invalid_params(
            SetFields::NAME,
            "/value",
            text!("refusal-fields-direction-unknown", v.to_string()),
        ));
    }
    Ok(())
}

/// ID-1146: after all of a call's entries, each interruption point whose direction or star an entry sets must be an
/// unstarred point or a starred source that is not external; the first that is not, in entry and target order,
/// refuses the whole call, named in words ("interruption point L-1: a star source is a source point; …").
fn refuse_bad_stars(tx: &TxBuilder<'_>, edits: &[SetFields]) -> Result<(), CmdError> {
    for e in edits {
        if e.kind != KindName::Function
            || !matches!(e.path.as_str(), "kind.ip.direction" | "kind.ip.star_source")
        {
            continue;
        }
        for t in &e.targets {
            let f = tx.require::<Function>(t.cast())?;
            if let FunctionKind::Ip(ip) = &f.kind
                && let Some(problem) = crate::commands::connection::star_problem(
                    &ip.direction,
                    ip.star_source,
                    ip.external.is_some(),
                )
            {
                return tx.reject(text!(
                    "refusal-fields-ip-star",
                    crate::words::interruption_point(tx, t.cast()),
                    problem
                ));
            }
        }
    }
    Ok(())
}

/// Sets `value` at the dotted `path` of one record: serialise, edit the JSON, read it back, put.
/// `fields.set` path `line_kind` of a connection (WP-S1.20 D18): the line kind of a fluid line, one of `working`,
/// `pilot`, `exhaust`, `drain` and `flexible`. Refused on a connection that is no fluid line (§5 T11).
fn set_line_kind(tx: &mut TxBuilder<'_>, id: ew_ids::Id<Connection>, value: &Value) -> Result<(), CmdError> {
    let c = tx.require::<Connection>(id)?;
    if !matches!(c.kind, ew_schema::ConnectionKind::Fluid(_)) {
        return tx.reject(text!("refusal-fields-not-fluid-line", crate::words::wire(tx, id)));
    }
    let kind = match serde_json::from_value::<ew_schema::LineKind>(value.clone()) {
        Ok(ew_schema::LineKind::Unknown(_)) | Err(_) => {
            return Err(CmdError::invalid_params(
                SetFields::NAME,
                "/value",
                text!("refusal-fields-line-kind"),
            ));
        }
        Ok(kind) => kind,
    };
    tx.update::<Connection>(id, |c| {
        if let ew_schema::ConnectionKind::Fluid(f) = &mut c.kind {
            f.line_kind = kind;
        }
    })
}

fn set_one<T: Entity + Serialize + DeserializeOwned>(
    tx: &mut TxBuilder<'_>,
    target: AnyId,
    path: &str,
    value: &Value,
) -> Result<(), CmdError> {
    let id = target.cast::<<T as Entity>::Marker>();
    let record = tx.require::<T>(id)?;
    let mut json = serde_json::to_value(&*record)
        .map_err(|e| CmdError::internal(format!("fields.set: cannot serialise the record ({e})")))?;
    let segments: Vec<&str> = path.split('.').collect();
    if T::KIND == Kind::Device
        && segments.len() > 1
        && ASPECTS.contains(&segments[0])
        && !json.get(segments[0]).is_some_and(Value::is_object)
    {
        return Err(CmdError::invalid_params(
            SetFields::NAME,
            "/path",
            text!(
                "refusal-fields-no-aspect",
                segments[0].to_string(),
                path.to_string()
            ),
        ));
    }
    if !set_member(&mut json, &segments, value) {
        return Err(CmdError::invalid_params(
            SetFields::NAME,
            "/path",
            text!("refusal-fields-below-non-object", path.to_string()),
        ));
    }
    let next: T = serde_json::from_value(json).map_err(|e| {
        CmdError::invalid_params(
            SetFields::NAME,
            "/value",
            text!("refusal-params-detail", e.to_string()),
        )
    })?;
    tx.update::<T>(id, |r| *r = next)?;
    if T::KIND == Kind::Device && matches!(path, "tag.counter" | "tag.letter" | "tag.scheme") {
        refuse_tag_collision(tx, target.cast())?;
    }
    Ok(())
}

/// D2: a counter or letter set through `fields.set` that makes the device's page-based tag collide on its
/// page is refused with the next free counter as the remedy, never replaced silently (automatic
/// renumbering, FR-NUM-029, applies only to new devices, moves and page changes). A cable on no page is checked in
/// the project's scope, in words that name no page (FT-1406, ID-1418). A tag that the edit leaves unchanged, another
/// unplaced device and a device outside page-based numbering are not checked.
fn refuse_tag_collision(tx: &TxBuilder<'_>, device: ew_ids::Id<Device>) -> Result<(), CmdError> {
    use crate::numbering::{CounterScope, devices_in_scope, main_page, next_free_counter, numbered};
    let Some(dev) = tx.get::<Device>(device) else {
        return Ok(());
    };
    let unchanged = tx
        .base_row(&(Kind::Device, device.cast()))
        .and_then(|r| r.get::<Device>().map(|b| b.tag == dev.tag))
        .unwrap_or(false);
    let Some(counter) = dev.tag.counter else {
        return Ok(());
    };
    if unchanged || !numbered(&dev) {
        return Ok(());
    }
    let page = main_page(tx, device);
    let scope = match page {
        Some(page) => CounterScope::Page(page),
        None if dev.aspects.cable.is_some() => CounterScope::Project,
        None => return Ok(()),
    };
    let taken = devices_in_scope(tx, &dev.tag.letter, scope)
        .into_iter()
        .any(|(other, c)| other != device && c == Some(counter));
    if !taken {
        return Ok(());
    }
    let free = next_free_counter(tx, &dev.tag.letter, scope, Some(device));
    let Some(page) = page else {
        return tx.reject(text!(
            "refusal-fields-counter-used-project",
            counter.to_string(),
            dev.tag.letter.to_string(),
            free.to_string()
        ));
    };
    tx.reject(text!(
        "refusal-fields-counter-used",
        counter.to_string(),
        dev.tag.letter.to_string(),
        crate::words::page(tx, page),
        free.to_string()
    ))
}

/// Sets (or, for `null`, removes) the member at `segments` below `root`, creating intermediate objects
/// when setting. `false` if an intermediate value exists and is not an object.
pub(crate) fn set_member(root: &mut Value, segments: &[&str], value: &Value) -> bool {
    let Some((last, parents)) = segments.split_last() else {
        return false;
    };
    let remove = value.is_null();
    let mut cur = root;
    for seg in parents {
        let Some(obj) = cur.as_object_mut() else {
            return false;
        };
        if remove && !obj.contains_key(*seg) {
            return true;
        }
        cur = obj
            .entry((*seg).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }
    let Some(obj) = cur.as_object_mut() else {
        return false;
    };
    if remove {
        obj.remove(*last);
    } else {
        obj.insert((*last).to_string(), value.clone());
    }
    true
}
