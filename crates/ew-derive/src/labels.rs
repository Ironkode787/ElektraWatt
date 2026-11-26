// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `label_rows()`: every label of the project in the order the fitter applies them (doc 13 §5.3), one engine for every
//! label set: the wire markers of `wire_record_set()`, the terminal and strip markers, the cable markers, the device
//! labels, the tube markers of the tube list and the plates of the door. Each label names its marker part, the first
//! marker of the workshop stock that marks its set, and whether its text fits it.

use std::cmp::Reverse;
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::natural_key;
use ew_schema::{
    CarrierRef, Connection, Device, DeviceLabels, LabelMode, MarkTarget, MountKind, MountingSurface, Part,
    Placement,
};

use crate::fluid_rows::TubeRows;
use crate::layout::{CarrierGeometry, PlacementGeometry};
use crate::marking::{MarkerText, shorten};
use crate::q::Q;
use crate::query::{Policy, Query};
use crate::tag::short_tag;
use crate::wire_record::WireRecordSet;

/// The label sets (doc 13 §5.3), in the order the fitter applies them.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum LabelSet {
    /// The markers of the wire ends.
    WireMarkers,
    /// The markers of the terminals.
    TerminalMarkers,
    /// The markers of the strips.
    StripMarkers,
    /// The markers of the cables.
    CableMarkers,
    /// The labels of the devices.
    DeviceLabels,
    /// The markers of the tube ends.
    TubeMarkers,
    /// The plates of the devices in the door.
    Plates,
}

impl LabelSet {
    /// Every set, in order.
    pub const ALL: [LabelSet; 7] = [
        LabelSet::WireMarkers,
        LabelSet::TerminalMarkers,
        LabelSet::StripMarkers,
        LabelSet::CableMarkers,
        LabelSet::DeviceLabels,
        LabelSet::TubeMarkers,
        LabelSet::Plates,
    ];

    /// What a stock marker that prints this set marks (`sections.marker.marks`).
    pub fn marks(self) -> MarkTarget {
        match self {
            LabelSet::WireMarkers | LabelSet::TubeMarkers => MarkTarget::Wire,
            LabelSet::TerminalMarkers | LabelSet::StripMarkers => MarkTarget::Terminal,
            LabelSet::CableMarkers => MarkTarget::Cable,
            LabelSet::DeviceLabels | LabelSet::Plates => MarkTarget::Device,
        }
    }
}

/// The columns of the `labels` list (doc 13 §5.3), in its order: what a label file's export profile names by key.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LabelColumn {
    /// The label set.
    Set,
    /// What the label is for ("101.1 at -X1011:L1").
    For,
    /// The first line.
    Text1,
    /// The second line.
    Text2,
    /// The third line.
    Text3,
    /// How many the fitter prints.
    Copies,
    /// The marker part's number.
    MarkerPart,
    /// "does not fit its marker part".
    Note,
}

impl LabelColumn {
    /// Every column, in the list's order.
    pub const ALL: [LabelColumn; 8] = [
        LabelColumn::Set,
        LabelColumn::For,
        LabelColumn::Text1,
        LabelColumn::Text2,
        LabelColumn::Text3,
        LabelColumn::Copies,
        LabelColumn::MarkerPart,
        LabelColumn::Note,
    ];

    /// The column's key (`text_1`).
    pub fn key(self) -> &'static str {
        match self {
            LabelColumn::Set => "set",
            LabelColumn::For => "for",
            LabelColumn::Text1 => "text_1",
            LabelColumn::Text2 => "text_2",
            LabelColumn::Text3 => "text_3",
            LabelColumn::Copies => "copies",
            LabelColumn::MarkerPart => "marker_part",
            LabelColumn::Note => "note",
        }
    }

    /// The column whose key is `key`.
    pub fn parse(key: &str) -> Option<LabelColumn> {
        LabelColumn::ALL.into_iter().find(|c| c.key() == key)
    }
}

/// The generic export profile (doc 13 §5.4): the columns every label program that imports a table maps once, with
/// the headings the file prints; a project without a profile of its own uses it.
pub const GENERIC_PROFILE: [(LabelColumn, &str); 5] = [
    (LabelColumn::Text1, "Text 1"),
    (LabelColumn::Text2, "Text 2"),
    (LabelColumn::Text3, "Text 3"),
    (LabelColumn::Copies, "Copies"),
    (LabelColumn::For, "For"),
];

/// The project's export profile: its own, else the generic one.
pub fn label_profile(q: &Q) -> ew_schema::ExportProfile {
    q.project()
        .settings
        .workshop
        .label_profile
        .clone()
        .unwrap_or_else(|| ew_schema::ExportProfile {
            columns: GENERIC_PROFILE
                .iter()
                .map(|(c, heading)| ew_schema::ProfileColumn {
                    key: c.key().to_owned(),
                    heading: (*heading).to_owned(),
                    extra: ew_schema::Extra::default(),
                })
                .collect(),
            ..ew_schema::ExportProfile::default()
        })
}

/// What a label marks.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LabelTarget {
    /// The end (0 From, 1 To) of a wire (its ID).
    WireEnd(Id<Connection>, u8),
    /// A terminal, a strip, a cable or a device.
    Device(Id<Device>),
    /// One level of a terminal of several levels (doc 08 §6.3): the terminal and the level.
    TerminalLevel(Id<Device>, u8),
    /// The end (0 from, 1 to) of a tube (its ID).
    TubeEnd(Id<Connection>, u8),
}

impl LabelTarget {
    /// The device a label of a device or of a terminal's level marks.
    pub fn device(&self) -> Option<Id<Device>> {
        match self {
            LabelTarget::Device(d) | LabelTarget::TerminalLevel(d, _) => Some(*d),
            LabelTarget::WireEnd(..) | LabelTarget::TubeEnd(..) => None,
        }
    }
}

/// One label (doc 13 §2.4).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LabelRecord {
    /// Its set.
    pub set: LabelSet,
    /// What it marks.
    pub target: LabelTarget,
    /// Its lines, shortened to its marker part where it names lines and characters.
    pub lines: Vec<String>,
    /// How many the fitter prints.
    pub copies: u8,
    /// The stock's marker that marks its set.
    pub marker_part: Option<Id<Part>>,
    /// Whether its text fits the marker part.
    pub fits: bool,
}

/// `label_rows()`: see the module documentation. Policy Cold.
#[derive(Clone, Copy, Debug)]
pub struct LabelRows;

impl Query for LabelRows {
    type Key = ();
    type Value = Vec<LabelRecord>;
    const FAMILY: &'static str = "label_rows";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, _key: &()) -> Arc<Vec<LabelRecord>> {
        let workshop = &q.project().settings.workshop;
        let parts = q.parts();
        let marker = |set: LabelSet| {
            workshop.stock.iter().find_map(|r| {
                let m = parts.get(r.copy)?.item.sections.marker.as_ref()?;
                (m.marks == set.marks()).then_some((r.copy, m.lines, m.chars))
            })
        };
        let mut out = Vec::new();
        let mut push = |set: LabelSet, target: LabelTarget, made: Vec<String>, copies: u8| {
            let part = marker(set);
            let (lines, fits) = shorten(&made, part.and_then(|p| p.1), part.and_then(|p| p.2));
            out.push(LabelRecord {
                set,
                target,
                lines,
                copies,
                marker_part: part.map(|p| p.0),
                fits,
            });
        };
        // Wire markers: the wire list's order, the From end before the To end, as `wire_record_set()` marks them.
        for r in q.get::<WireRecordSet>(()).iter() {
            for (i, end) in r.ends.iter().enumerate() {
                if let MarkerText::Lines(lines) = &end.marker {
                    let i = u8::try_from(i).unwrap_or(0);
                    push(
                        LabelSet::WireMarkers,
                        LabelTarget::WireEnd(r.wire.id, i),
                        lines.clone(),
                        1,
                    );
                }
            }
        }
        // The devices by their tags, in natural order.
        let devices = q.devices();
        let mut by_tag: Vec<(Id<Device>, String)> = devices
            .iter()
            .map(|(id, _)| (id, short_tag(q, id)))
            .filter(|(_, t)| !t.is_empty())
            .collect();
        by_tag.sort_by_cached_key(|(id, t)| (natural_key(t), *id));
        let strips: Vec<&(Id<Device>, String)> = by_tag
            .iter()
            .filter(|(id, _)| devices.get(*id).is_some_and(|d| d.aspects.strip.is_some()))
            .collect();
        // Terminal markers: strips in natural tag order, terminals in mounting order, one per level of a terminal of
        // several; the designation (a level's own), and under it the function text where the strip prints function
        // texts (doc 08 §6.1).
        let lang = crate::print_language(q);
        for (strip, _) in &strips {
            let function_texts = devices
                .get(*strip)
                .and_then(|s| s.aspects.strip.as_ref())
                .is_some_and(|s| s.label_mode == LabelMode::FunctionText);
            let mut terminals: Vec<_> = q
                .idx()
                .terminals_of_strip(*strip)
                .filter_map(|t| devices.get(t).cloned())
                .collect();
            terminals.sort_by_key(|t| (t.aspects.terminal.as_ref().map_or(0, |a| a.seq), t.id));
            for t in terminals {
                let Some(term) = t.aspects.terminal.as_ref() else {
                    continue;
                };
                for level in crate::cps::terminal_levels(q, t.id) {
                    let mut lines = vec![term.level_designation(level).as_str().to_owned()];
                    let function = crate::text_in(&t.function_text, lang.as_ref());
                    if function_texts && !function.is_empty() {
                        lines.push(function.to_owned());
                    }
                    let target = level.map_or(LabelTarget::Device(t.id), |l| LabelTarget::TerminalLevel(t.id, l));
                    push(LabelSet::TerminalMarkers, target, lines, 1);
                }
            }
        }
        // Strip markers: the strip's tag.
        for (strip, tag) in &strips {
            push(
                LabelSet::StripMarkers,
                LabelTarget::Device(*strip),
                vec![tag.clone()],
                1,
            );
        }
        // Cable markers: the cable's tag, one per end.
        for (id, tag) in by_tag
            .iter()
            .filter(|(id, _)| devices.get(*id).is_some_and(|d| d.aspects.cable.is_some()))
        {
            push(
                LabelSet::CableMarkers,
                LabelTarget::Device(*id),
                vec![tag.clone()],
                2,
            );
        }
        // Device labels: a device with a part of its own that is no terminal, strip, cable or carrier.
        let carrier = |p: &ew_schema::PartRef| {
            parts.get(p.copy).is_some_and(|c| {
                let s = &c.item.sections;
                s.enclosure.is_some() || s.duct.is_some() || s.rail.is_some() || s.mounting_plate.is_some()
            })
        };
        let door = |d: &Device| {
            d.parts.iter().any(|u| {
                parts
                    .get(u.part.copy)
                    .and_then(|c| c.item.mech.as_ref())
                    .and_then(|m| m.mount_kind.as_ref())
                    .is_some_and(|k| *k == MountKind::DoorCutout)
            })
        };
        let copies = match workshop.device_labels {
            DeviceLabels::OnDeviceAndBeside => 2,
            DeviceLabels::OnDevice | DeviceLabels::Unknown(_) => 1,
        };
        // Placed devices in mounting order (per surface, carriers from the top, along each from the left), then the
        // others in natural tag order.
        let mut labelled: Vec<&(Id<Device>, String)> = by_tag.iter().collect();
        labelled.sort_by_cached_key(|(id, _)| mounting_key(q, *id).map_or((1, None), |k| (0, Some(k))));
        for (id, tag) in labelled.iter().copied() {
            let Some(d) = devices.get(*id) else { continue };
            let a = &d.aspects;
            if a.terminal.is_some() || a.strip.is_some() || a.cable.is_some() || d.parts.is_empty() {
                continue;
            }
            if d.parts.iter().all(|u| carrier(&u.part)) {
                continue;
            }
            push(
                LabelSet::DeviceLabels,
                LabelTarget::Device(*id),
                vec![tag.clone()],
                copies,
            );
        }
        // Tube markers: the tube's tag, else the far end as seen from this end; the tube list's order.
        for t in q.get::<TubeRows>(()).iter() {
            for (i, (_, other)) in [(&t.from, &t.to), (&t.to, &t.from)].into_iter().enumerate() {
                let text = if t.tag.is_empty() {
                    other.text.clone()
                } else {
                    t.tag.clone()
                };
                let i = u8::try_from(i).unwrap_or(0);
                push(
                    LabelSet::TubeMarkers,
                    LabelTarget::TubeEnd(t.id, i),
                    vec![text],
                    1,
                );
            }
        }
        // Plates: a device in the door by its function text in the project's language, in the door's mounting order.
        for (id, _) in labelled.iter().copied() {
            let Some(d) = devices.get(*id) else { continue };
            if !door(d) {
                continue;
            }
            let text = crate::text_in(&d.function_text, lang.as_ref()).to_owned();
            if !text.is_empty() {
                push(LabelSet::Plates, LabelTarget::Device(*id), vec![text], 1);
            }
        }
        Arc::new(out)
    }
}

/// Where a placed device stands in the mounting order: its first placement's surface, the top of its carrier (a
/// placement on the surface itself: of its footprint) from the top down, and the footprint's left edge. `None` for a
/// device without a placement whose geometry is known.
fn mounting_key(q: &Q, device: Id<Device>) -> Option<(Id<MountingSurface>, Reverse<i64>, i64)> {
    let (placement, geom) = q
        .idx()
        .placements_of_device(device)
        .filter_map(|p| {
            Some((
                q.table::<Placement>().get(p)?.clone(),
                q.get::<PlacementGeometry>(p).as_ref().clone()?,
            ))
        })
        .min_by_key(|(p, _)| p.id)?;
    let carrier_top = match placement.carrier {
        CarrierRef::Carrier(c) => q
            .get::<CarrierGeometry>(c)
            .as_ref()
            .as_ref()
            .and_then(|g| g.outline.iter().map(|r| r.max().y.0).max()),
        CarrierRef::Surface(_) | CarrierRef::Unknown(_) => None,
    };
    let top = carrier_top.unwrap_or(geom.footprint.max().y.0);
    Some((geom.surface, Reverse(top), geom.footprint.min().x.0))
}
