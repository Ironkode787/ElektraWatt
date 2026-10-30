// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The strip editor's door below the shell (WP-S2.07 §4.2, §6.3 to §6.7; D4 to D8, D14): [`Session::strip_edit`]
//! compiles one strip gesture into one command, and [`Session::strip_edits`] several cell gestures into one.
//!
//! - **Group labels** (D8): setting the label L on terminals E of a strip rewrites `strip.groups`: every group that
//!   covers a member of E splits into the parts of its range without E's sequence numbers (each keeps its label and
//!   unknown members; a part covering no terminal is dropped); the other groups stay verbatim; for a non-empty L, each
//!   run of E consecutive in the strip's terminal order becomes a group L, merged with a directly adjacent part
//!   labelled L that has no unknown members; the list is sorted by `from_seq` (stable). An empty L only removes.
//! - **Bridge parts** (D6, D7): the part of the bridge group of each terminal is set on the group's carrier
//!   (`overrides.wire_part`), the part text resolved among the `accessory` parts; an empty text clears it.
//! - **Accessories** (D5, D7): one `fields.set` of `strip.accessories` per strip, kept sorted by `after_seq` (stable);
//!   an accessory's part text is resolved among the part kinds of its kind ([`accessory_part_kinds`]).
//! - **Insert and move** (D4, D14): one `terminal.insert` (the reference terminal's function definition and part uses,
//!   the next free whole-number designations) or one `terminal.move`.
//!
//! Several field edits are applied in order to working copies of the values they touch and compile to one
//! `fields.set` when every target gets the same (kind, path, value), else one `fields.set_many` with one entry per
//! (kind, path, value) in the order of first appearance. Every order is named (NFR-REL-07).

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::MlText;
use ew_derive::ew_schema::{AccessoryKind, Device, FnDef, LibDocument, TerminalGroup};
use ew_derive::{MainFunction, Q};
use serde_json::{Value, json};

use super::fields::{FieldChoice, ParseCx, PartCopyText, PartMiss, find_part, part_order};
use super::navigators::Uuid;
use super::properties::SheetCx;
use crate::{AnyCommand, Session};

/// The strip-editor columns whose cells compile through [`Session::strip_edit`] (§6.2).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum StripCell {
    /// A terminal's group label (D8).
    Group,
    /// A terminal's bridge part (D6).
    BridgePart,
    /// An accessory's part (D5).
    AccessoryPart,
}

/// One strip gesture (§4.2).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum StripEdit {
    /// The group label `label` on `terminals` (D8); `""` removes them from every group.
    GroupLabel {
        /// The terminal devices.
        terminals: Vec<Uuid>,
        /// The label.
        label: String,
    },
    /// The part of the bridges of `terminals` (D6, D7); `""` clears it.
    BridgePart {
        /// The terminal devices.
        terminals: Vec<Uuid>,
        /// The part text.
        part: String,
    },
    /// The part of accessory `index` of `strip` (D5, D7).
    AccessoryPart {
        /// The strip device.
        strip: Uuid,
        /// The index in `Strip.accessories`.
        index: u32,
        /// The part text.
        part: String,
    },
    /// A new accessory of `kind` after terminal sequence `after_seq` (after the accessories already there).
    AddAccessory {
        /// The strip device.
        strip: Uuid,
        /// The sequence number it follows.
        after_seq: u32,
        /// The kind's word (`end_plate`, `end_clamp`, `partition`, `marker`, `cover`).
        kind: String,
        /// The part text; `None`: the one project part copy of the kind's part kinds.
        part: Option<String>,
    },
    /// Removes accessories `indexes` of `strip`.
    RemoveAccessories {
        /// The strip device.
        strip: Uuid,
        /// The indexes in `Strip.accessories`.
        indexes: Vec<u32>,
    },
    /// `count` new terminals before `before` (absent: at the end), copying the reference terminal (D14).
    Insert {
        /// The strip device.
        strip: Uuid,
        /// The terminal to insert before.
        before: Option<Uuid>,
        /// How many terminals.
        count: u32,
        /// The designation (count 1 only).
        designation: Option<String>,
    },
    /// Moves `terminals` before `before` (absent: to the end) (D4).
    Move {
        /// The terminal devices.
        terminals: Vec<Uuid>,
        /// The terminal to move before.
        before: Option<Uuid>,
    },
}

/// Why a strip gesture does not compile.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum StripEditError {
    /// A reason key of §6.10 (`reason-no-accessory-part`, `table-no-bridge`).
    Refused(&'static str),
    /// A sentence, for the error line or the card.
    Invalid(String),
    /// Several part copies fit (D7): every one as a choice, its value the text `id:<uuid>` that resolves to it.
    Ambiguous(Vec<FieldChoice>),
}

/// The word an accessory row shows for its kind (D3).
pub(crate) fn accessory_word(kind: &AccessoryKind) -> &'static str {
    match kind {
        AccessoryKind::EndPlate => "End plate",
        AccessoryKind::EndClamp => "End bracket",
        AccessoryKind::Partition => "Partition plate",
        AccessoryKind::Marker => "Marker carrier",
        AccessoryKind::Cover => "Cover",
        AccessoryKind::Unknown(_) => "Accessory",
    }
}

/// The word an accessory row shows for the kind's parameter word (`end_clamp` → `End bracket`), for the shell, which
/// names no schema type.
pub fn accessory_kind_word(kind: &str) -> &'static str {
    match kind {
        "end_plate" => "End plate",
        "end_clamp" => "End bracket",
        "partition" => "Partition plate",
        "marker" => "Marker carrier",
        "cover" => "Cover",
        _ => "Accessory",
    }
}

/// The accessory kinds' words, in the order of the `kind` parameter.
pub const ACCESSORY_KINDS: [&str; 5] = ["end_plate", "end_clamp", "partition", "marker", "cover"];

/// The part kinds an accessory of kind `word` takes (D7): an end bracket `end_bracket`; a marker carrier `marker` and
/// `accessory`; every other accessory (and a bridge) `accessory`.
pub fn accessory_part_kinds(word: &str) -> &'static [&'static str] {
    match word {
        "end_clamp" => &["end_bracket"],
        "marker" => &["marker", "accessory"],
        _ => &["accessory"],
    }
}

/// The part kinds of a bridge's part (D7).
const BRIDGE_PART_KINDS: &[&str] = &["accessory"];

/// The edits of field values on their way to one command.
#[derive(Default)]
struct Work {
    /// Working copies of `strip.groups`, by strip, in first-touch order.
    groups: Vec<(Id<Device>, Vec<TerminalGroup>)>,
    /// Working copies of `strip.accessories` (JSON), by strip, in first-touch order.
    accessories: Vec<(Id<Device>, Vec<Value>)>,
    /// Carrier connection → its new `overrides.wire_part`, in first-touch order.
    carriers: Vec<(Uuid, Value)>,
    /// The order the touched values print in: (0 groups, 1 accessories, 2 carriers, index).
    order: Vec<(u8, usize)>,
}

/// One bridge group as the page description lists it (FT-621): its label `BR<n>` and its terminals in (`seq`, ID)
/// order, each as its device and its printed name `<strip>:<designation>`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BridgeRow {
    /// `BR<n>`, numbered as the terminal diagrams and the strip editor number them.
    pub label: String,
    /// The strip device.
    pub strip: Uuid,
    /// The terminals: (device, `-X1031:1`).
    pub terminals: Vec<(Uuid, String)>,
}

impl Session {
    /// Every bridge group of the project (FT-621), in `BR<n>` order (WP-S2.07 D6's `bridge_groups`).
    pub fn bridges(&self) -> Vec<BridgeRow> {
        let q = self.query();
        let tag = |d: Id<Device>| {
            q.get::<ew_derive::DeviceTag>(d)
                .as_ref()
                .as_ref()
                .map(ew_derive::Tag::short)
                .unwrap_or_default()
        };
        ew_reports::bridge_groups(&q)
            .into_iter()
            .map(|(label, strip, g)| {
                let strip_tag = tag(strip);
                let terminals = g
                    .terminals
                    .iter()
                    .map(|t| {
                        let designation = q
                            .devices()
                            .get(*t)
                            .and_then(|d| d.aspects.terminal.as_ref())
                            .map(|a| a.designation.as_str().to_owned())
                            .unwrap_or_default();
                        (t.as_uuid(), format!("{strip_tag}:{designation}"))
                    })
                    .collect();
                BridgeRow {
                    label,
                    strip: strip.as_uuid(),
                    terminals,
                }
            })
            .collect()
    }

    /// The key of the function definition of the function `record` (WP-S2.06 §6.7: the drop of an unplaced function
    /// row arms the Place tool with a symbol drawing that definition; the row names the function in words since cycle
    /// 37), else the definition's ID text; `None` when `record` is no function.
    pub fn function_fndef_key(&self, record: Uuid) -> Option<String> {
        let q = self.query();
        let f = q.functions().get(Id::from_uuid(record))?;
        Some(super::navigators::fndef_name(&q, f))
    }

    /// The main function of the device `record` and the key of its function definition (WP-S2.07 §6.8: an unplaced
    /// terminal row places its terminal's main function); `None` when `record` is no device or the device has no
    /// function. The key is empty for a definition without one.
    pub fn main_function(&self, record: Uuid) -> Option<(Uuid, String)> {
        let q = self.query();
        let device: Id<Device> = Id::from_uuid(record);
        q.devices().get(device)?;
        let main = (*q.get::<MainFunction>(device))?;
        let key = q
            .functions()
            .get(main)
            .and_then(|f| q.fndefs().get(f.def).cloned())
            .and_then(|d| d.item.meta.key.as_ref().map(|k| k.as_str().to_owned()))
            .unwrap_or_default();
        Some((main.as_uuid(), key))
    }

    /// The one command of a strip gesture (§4.2): the door through which the view model compiles every strip gesture.
    ///
    /// # Errors
    ///
    /// [`StripEditError`] when the gesture cannot apply.
    pub fn strip_edit(&self, edit: &StripEdit) -> Result<AnyCommand, StripEditError> {
        self.strip_edits(std::slice::from_ref(edit))
    }

    /// The one command of several cell gestures (§6.3): field edits applied in order, one `fields.set` or one
    /// `fields.set_many`. An `Insert` or a `Move` stands alone.
    ///
    /// # Errors
    ///
    /// [`StripEditError`] of the first edit that cannot apply; `Invalid` for no edit, or an `Insert` or `Move` among
    /// others.
    pub fn strip_edits(&self, edits: &[StripEdit]) -> Result<AnyCommand, StripEditError> {
        let q = self.query();
        match edits {
            [
                StripEdit::Insert {
                    strip,
                    before,
                    count,
                    designation,
                },
            ] => return insert(&q, *strip, *before, *count, designation.as_deref()),
            [StripEdit::Move { terminals, before }] => {
                let mut params =
                    json!({ "terminals": terminals.iter().map(Uuid::to_string).collect::<Vec<_>>() });
                if let Some(b) = before {
                    params["before"] = Value::String(b.to_string());
                }
                return Ok(AnyCommand {
                    name: "terminal.move".into(),
                    params,
                });
            }
            [] => return Err(StripEditError::Invalid("nothing to edit".into())),
            _ => {}
        }
        let cx = SheetCx::new(&q);
        let mut work = Work::default();
        for edit in edits {
            match edit {
                StripEdit::GroupLabel { terminals, label } => group_label(&q, &mut work, terminals, label)?,
                StripEdit::BridgePart { terminals, part } => {
                    bridge_part(&q, &cx, &mut work, terminals, part)?
                }
                StripEdit::AccessoryPart { strip, index, part } => {
                    let list = accessories_of(&q, &mut work, *strip)?;
                    let entry = list.get_mut(*index as usize).ok_or_else(|| {
                        StripEditError::Invalid(format!("the strip has no accessory {index}"))
                    })?;
                    let kind = entry
                        .get("kind")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned();
                    if part.trim().is_empty() {
                        return Err(StripEditError::Invalid("an accessory needs a part".into()));
                    }
                    let copy = resolve(&cx, part, accessory_part_kinds(&kind))?;
                    entry["part"] = json!({ "copy": copy });
                }
                StripEdit::AddAccessory {
                    strip,
                    after_seq,
                    kind,
                    part,
                } => {
                    if !ACCESSORY_KINDS.contains(&kind.as_str()) {
                        return Err(StripEditError::Invalid(format!(
                            "`{kind}` is not one of {}",
                            ACCESSORY_KINDS.join(", ")
                        )));
                    }
                    let kinds = accessory_part_kinds(kind);
                    let copy = match part {
                        Some(text) => resolve(&cx, text, kinds)?,
                        None => only_part(&cx, kinds)?,
                    };
                    let list = accessories_of(&q, &mut work, *strip)?;
                    let at = list
                        .iter()
                        .rposition(|a| {
                            a.get("after_seq").and_then(Value::as_u64).unwrap_or(0) <= u64::from(*after_seq)
                        })
                        .map_or(0, |i| i + 1);
                    list.insert(
                        at,
                        json!({"after_seq": after_seq, "kind": kind, "part": {"copy": copy}}),
                    );
                }
                StripEdit::RemoveAccessories { strip, indexes } => {
                    let list = accessories_of(&q, &mut work, *strip)?;
                    let n = list.len();
                    if let Some(bad) = indexes.iter().find(|i| **i as usize >= n) {
                        return Err(StripEditError::Invalid(format!(
                            "the strip has no accessory {bad}"
                        )));
                    }
                    let keep: Vec<Value> = list
                        .drain(..)
                        .enumerate()
                        .filter(|(i, _)| !indexes.iter().any(|x| *x as usize == *i))
                        .map(|(_, v)| v)
                        .collect();
                    *list = keep;
                }
                StripEdit::Insert { .. } | StripEdit::Move { .. } => {
                    return Err(StripEditError::Invalid(
                        "an insert or a move is a gesture of its own".into(),
                    ));
                }
            }
        }
        Ok(work.command())
    }
}

/// The copy of a part text among `kinds` (D7).
fn resolve(cx: &dyn ParseCx, text: &str, kinds: &[&str]) -> Result<String, StripEditError> {
    match find_part(cx, text, Some(kinds)) {
        Ok(id) => Ok(id),
        Err(PartMiss::Unknown(t)) => Err(StripEditError::Invalid(format!(
            "the project has no part `{t}` of kind {}",
            kinds.join(" or ")
        ))),
        Err(PartMiss::Invalid(m)) => Err(StripEditError::Invalid(m)),
        Err(PartMiss::Ambiguous(hits)) => Err(StripEditError::Ambiguous(choices(hits))),
    }
}

/// The choices of an ambiguous part (D7): the long display form, the value `id:<uuid>`.
fn choices(hits: Vec<PartCopyText>) -> Vec<FieldChoice> {
    hits.into_iter()
        .map(|c| FieldChoice {
            text: c.long(),
            value: Value::String(format!("id:{}", c.id)),
            note: None,
        })
        .collect()
}

/// The one project part copy of `kinds` (§6.5): none refuses `reason-no-accessory-part`, several are ambiguous.
fn only_part(cx: &dyn ParseCx, kinds: &[&str]) -> Result<String, StripEditError> {
    let mut fits: Vec<PartCopyText> = cx
        .part_copies()
        .into_iter()
        .filter(|c| kinds.contains(&c.kind.as_str()))
        .collect();
    fits.sort_by(part_order);
    match fits.len() {
        0 => Err(StripEditError::Refused("reason-no-accessory-part")),
        1 => Ok(fits.remove(0).id),
        _ => Err(StripEditError::Ambiguous(choices(fits))),
    }
}

/// The terminal aspect's strip and seq of a device.
fn terminal_of(q: &Q, t: Uuid) -> Option<(Id<Device>, u32)> {
    let d = q.devices().get(Id::from_uuid(t))?;
    let a = d.aspects.terminal.as_ref()?;
    Some((a.strip, a.seq))
}

/// The terminals of a strip in (seq, ID) order.
fn strip_terminals(q: &Q, strip: Id<Device>) -> Vec<(u32, Id<Device>)> {
    let mut v: Vec<(u32, Id<Device>)> = q
        .idx()
        .terminals_of_strip(strip)
        .filter_map(|t| Some((q.devices().get(t)?.aspects.terminal.as_ref()?.seq, t)))
        .collect();
    v.sort_unstable();
    v
}

/// The working copy of a strip's accessories (JSON), created from the stored list on first touch.
fn accessories_of<'w>(q: &Q, work: &'w mut Work, strip: Uuid) -> Result<&'w mut Vec<Value>, StripEditError> {
    let id: Id<Device> = Id::from_uuid(strip);
    if let Some(i) = work.accessories.iter().position(|(s, _)| *s == id) {
        return Ok(&mut work.accessories[i].1);
    }
    let aspect = q
        .devices()
        .get(id)
        .and_then(|d| d.aspects.strip.clone())
        .ok_or_else(|| StripEditError::Invalid("the device is not a terminal strip".into()))?;
    let list: Vec<Value> = aspect
        .accessories
        .iter()
        .filter_map(|a| serde_json::to_value(a).ok())
        .collect();
    work.order.push((1, work.accessories.len()));
    work.accessories.push((id, list));
    let last = work.accessories.len() - 1;
    Ok(&mut work.accessories[last].1)
}

/// D8 on the working copies: the label on the terminals, strip by strip in first-appearance order.
fn group_label(q: &Q, work: &mut Work, terminals: &[Uuid], label: &str) -> Result<(), StripEditError> {
    let mut by_strip: Vec<(Id<Device>, Vec<u32>)> = Vec::new();
    for t in terminals {
        let (strip, seq) =
            terminal_of(q, *t).ok_or_else(|| StripEditError::Invalid("the row is not a terminal".into()))?;
        match by_strip.iter_mut().find(|(s, _)| *s == strip) {
            Some((_, seqs)) => seqs.push(seq),
            None => by_strip.push((strip, vec![seq])),
        }
    }
    for (strip, mut set) in by_strip {
        set.sort_unstable();
        set.dedup();
        let at = match work.groups.iter().position(|(s, _)| *s == strip) {
            Some(i) => i,
            None => {
                let groups = q
                    .devices()
                    .get(strip)
                    .and_then(|d| d.aspects.strip.as_ref().map(|a| a.groups.clone()))
                    .ok_or_else(|| StripEditError::Invalid("the terminal's strip is not a strip".into()))?;
                work.order.push((0, work.groups.len()));
                work.groups.push((strip, groups));
                work.groups.len() - 1
            }
        };
        let order: Vec<u32> = strip_terminals(q, strip).into_iter().map(|(s, _)| s).collect();
        let groups = std::mem::take(&mut work.groups[at].1);
        work.groups[at].1 = relabel(groups, &order, &set, label);
    }
    Ok(())
}

/// D8 on one strip's groups: `order` the strip's terminal seqs in order, `set` the edited seqs (sorted).
pub(crate) fn relabel(
    groups: Vec<TerminalGroup>,
    order: &[u32],
    set: &[u32],
    label: &str,
) -> Vec<TerminalGroup> {
    let covers_terminal = |from: u32, to: u32| order.iter().any(|s| from <= *s && *s <= to);
    let mut out: Vec<TerminalGroup> = Vec::new();
    for g in groups {
        let hit: Vec<u32> = set
            .iter()
            .copied()
            .filter(|s| g.from_seq <= *s && *s <= g.to_seq)
            .collect();
        if hit.is_empty() {
            out.push(g);
            continue;
        }
        // The parts of [from, to] without the edited seqs.
        let mut start = g.from_seq;
        for s in hit.iter().copied().chain(std::iter::once(u32::MAX)) {
            let end = if s == u32::MAX {
                Some(g.to_seq)
            } else {
                s.checked_sub(1)
            };
            if let Some(end) = end
                && start <= end
                && end <= g.to_seq
                && (s != u32::MAX || start <= g.to_seq)
                && covers_terminal(start, end)
            {
                out.push(TerminalGroup {
                    from_seq: start,
                    to_seq: end,
                    label: g.label.clone(),
                    extra: g.extra.clone(),
                });
            }
            if s == u32::MAX {
                break;
            }
            match s.checked_add(1) {
                Some(n) => start = n,
                None => break,
            }
        }
    }
    if !label.is_empty() {
        // Runs of the edited terminals, consecutive in the strip's terminal order.
        let positions: Vec<usize> = set
            .iter()
            .filter_map(|s| order.iter().position(|x| x == s))
            .collect();
        let mut runs: Vec<(usize, usize)> = Vec::new();
        for p in positions {
            match runs.last_mut() {
                Some((_, end)) if *end + 1 == p => *end = p,
                _ => runs.push((p, p)),
            }
        }
        let is_l = |g: &TerminalGroup| g.label.as_neutral() == Some(label) && g.extra.is_empty();
        for (a, z) in runs {
            let mut from = order[a];
            let mut to = order[z];
            if a > 0 {
                let prev = order[a - 1];
                if let Some(i) = out.iter().position(|g| g.to_seq == prev && is_l(g)) {
                    from = out[i].from_seq;
                    out.remove(i);
                }
            }
            if let Some(next) = order.get(z + 1).copied()
                && let Some(i) = out.iter().position(|g| g.from_seq == next && is_l(g))
            {
                to = out[i].to_seq;
                out.remove(i);
            }
            out.push(TerminalGroup {
                from_seq: from,
                to_seq: to,
                label: MlText::neutral(label),
                extra: Default::default(),
            });
        }
    }
    out.sort_by_key(|g| g.from_seq);
    out
}

/// D6 on the working copies: the part of the bridge group of every terminal, on its carrier.
fn bridge_part(
    q: &Q,
    cx: &dyn ParseCx,
    work: &mut Work,
    terminals: &[Uuid],
    part: &str,
) -> Result<(), StripEditError> {
    let value = if part.trim().is_empty() {
        Value::Null
    } else {
        json!({ "copy": resolve(cx, part, BRIDGE_PART_KINDS)? })
    };
    let groups = ew_reports::bridge_groups(q);
    for t in terminals {
        let id: Id<Device> = Id::from_uuid(*t);
        let Some((_, _, g)) = groups.iter().find(|(_, _, g)| g.terminals.contains(&id)) else {
            return Err(StripEditError::Refused("table-no-bridge"));
        };
        let carrier = g.carrier.as_uuid();
        match work.carriers.iter_mut().find(|(c, _)| *c == carrier) {
            Some((_, v)) => *v = value.clone(),
            None => {
                work.order.push((2, work.carriers.len()));
                work.carriers.push((carrier, value.clone()));
            }
        }
    }
    Ok(())
}

impl Work {
    /// One `fields.set` when every target gets the same (kind, path, value), else one `fields.set_many`.
    fn command(self) -> AnyCommand {
        // (kind, path, value, target) in print order.
        let mut entries: Vec<(&str, &str, Value, String)> = Vec::new();
        for (what, i) in &self.order {
            match what {
                0 => {
                    let (strip, groups) = &self.groups[*i];
                    let value = serde_json::to_value(groups).unwrap_or(Value::Null);
                    let value = if value.as_array().is_some_and(Vec::is_empty) {
                        Value::Null
                    } else {
                        value
                    };
                    entries.push(("device", "strip.groups", value, strip.as_uuid().to_string()));
                }
                1 => {
                    let (strip, list) = &self.accessories[*i];
                    let mut list = list.clone();
                    // Kept sorted by `after_seq` (stable).
                    list.sort_by_key(|a| a.get("after_seq").and_then(Value::as_u64).unwrap_or(0));
                    let value = if list.is_empty() {
                        Value::Null
                    } else {
                        Value::Array(list)
                    };
                    entries.push(("device", "strip.accessories", value, strip.as_uuid().to_string()));
                }
                _ => {
                    let (carrier, value) = &self.carriers[*i];
                    entries.push((
                        "connection",
                        "overrides.wire_part",
                        value.clone(),
                        carrier.to_string(),
                    ));
                }
            }
        }
        let same = entries
            .windows(2)
            .all(|w| w[0].0 == w[1].0 && w[0].1 == w[1].1 && w[0].2 == w[1].2);
        if same && let Some((kind, path, value, _)) = entries.first() {
            let targets: Vec<&String> = entries.iter().map(|e| &e.3).collect();
            return AnyCommand {
                name: "fields.set".into(),
                params: json!({"kind": kind, "targets": targets, "path": path, "value": value}),
            };
        }
        let mut grouped: Vec<(&str, &str, Value, Vec<String>)> = Vec::new();
        for (kind, path, value, target) in entries {
            match grouped
                .iter_mut()
                .find(|(k, p, v, _)| *k == kind && *p == path && *v == value)
            {
                Some((_, _, _, ts)) => ts.push(target),
                None => grouped.push((kind, path, value, vec![target])),
            }
        }
        let edits: Vec<Value> = grouped
            .into_iter()
            .map(|(k, p, v, ts)| json!({"kind": k, "targets": ts, "path": p, "value": v}))
            .collect();
        AnyCommand {
            name: "fields.set_many".into(),
            params: json!({ "edits": edits }),
        }
    }
}

/// D14: `terminal.insert` before `before` (absent: at the end) with `count` terminals copying the reference terminal
/// (`before`, else the strip's last terminal).
fn insert(
    q: &Q,
    strip: Uuid,
    before: Option<Uuid>,
    count: u32,
    designation: Option<&str>,
) -> Result<AnyCommand, StripEditError> {
    let strip_id: Id<Device> = Id::from_uuid(strip);
    if !q
        .devices()
        .get(strip_id)
        .is_some_and(|d| d.aspects.strip.is_some())
    {
        return Err(StripEditError::Invalid(
            "the device is not a terminal strip".into(),
        ));
    }
    if count == 0 {
        return Err(StripEditError::Invalid("insert at least one terminal".into()));
    }
    if designation.is_some() && count != 1 {
        return Err(StripEditError::Invalid("a designation names one terminal".into()));
    }
    let terminals = strip_terminals(q, strip_id);
    let reference = before
        .map(Id::<Device>::from_uuid)
        .or_else(|| terminals.last().map(|(_, t)| *t));
    let fndef: Option<Id<FnDef>> = reference
        .and_then(|r| *q.get::<MainFunction>(r))
        .and_then(|f| q.functions().get(f).map(|f| f.def))
        .or_else(|| starter_terminal_def(q));
    let Some(fndef) = fndef else {
        return Err(StripEditError::Refused("reason-no-terminal-def"));
    };
    let parts: Vec<Value> = reference
        .and_then(|r| q.devices().get(r))
        .map(|d| {
            d.parts
                .iter()
                .filter_map(|u| serde_json::to_value(u).ok())
                .collect()
        })
        .unwrap_or_default();
    let names: Vec<String> = match designation {
        Some(d) => vec![d.to_owned()],
        None => {
            let highest = terminals
                .iter()
                .filter_map(|(_, t)| {
                    q.devices()
                        .get(*t)?
                        .aspects
                        .terminal
                        .as_ref()
                        .map(|a| a.designation.as_str().to_owned())
                })
                .filter(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
                .filter_map(|d| d.parse::<u64>().ok())
                .max()
                .unwrap_or(0);
            (1..=u64::from(count))
                .map(|k| (highest + k).to_string())
                .collect()
        }
    };
    let mut params = json!({
        "strip": strip.to_string(),
        "fndef": fndef.as_uuid().to_string(),
        "terminals": names.iter().map(|d| json!({ "designation": d })).collect::<Vec<_>>(),
    });
    if let Some(b) = before {
        params["before"] = Value::String(b.to_string());
    }
    if !parts.is_empty() {
        params["parts"] = Value::Array(parts);
    }
    Ok(AnyCommand {
        name: "terminal.insert".into(),
        params,
    })
}

/// The project's held copy of the starter `ew.fn.terminal`, if it embeds it.
fn starter_terminal_def(q: &Q) -> Option<Id<FnDef>> {
    let item = ew_library::starter::item("ew.fn.terminal")?;
    match LibDocument::from_json(item.json).ok()? {
        LibDocument::Fndef(c) if q.fndefs().contains(c.id) => Some(c.id),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(from: u32, to: u32, label: &str) -> TerminalGroup {
        TerminalGroup {
            from_seq: from,
            to_seq: to,
            label: MlText::neutral(label),
            extra: Default::default(),
        }
    }

    fn show(groups: &[TerminalGroup]) -> Vec<(u32, u32, String)> {
        groups
            .iter()
            .map(|g| {
                (
                    g.from_seq,
                    g.to_seq,
                    g.label.as_neutral().unwrap_or_default().to_owned(),
                )
            })
            .collect()
    }

    fn golden() -> Vec<TerminalGroup> {
        vec![
            g(1, 4, "Pneumatics"),
            g(5, 10, "Spare"),
            g(11, 14, "Pneumatics"),
            g(15, 20, "Spare"),
        ]
    }

    fn order() -> Vec<u32> {
        (1..=20).collect()
    }

    fn row(from: u32, to: u32, l: &str) -> (u32, u32, String) {
        (from, to, l.to_owned())
    }

    /// SE8: the three examples of D8 on -X1031's groups.
    #[test]
    fn relabel_splits_merges_and_clears() {
        let out = relabel(golden(), &order(), &[7, 8], "Reserve");
        assert_eq!(
            show(&out),
            [
                row(1, 4, "Pneumatics"),
                row(5, 6, "Spare"),
                row(7, 8, "Reserve"),
                row(9, 10, "Spare"),
                row(11, 14, "Pneumatics"),
                row(15, 20, "Spare"),
            ]
        );
        let out = relabel(golden(), &order(), &[5, 6, 7, 8, 9, 10], "Pneumatics");
        assert_eq!(show(&out), [row(1, 14, "Pneumatics"), row(15, 20, "Spare")]);
        let out = relabel(golden(), &order(), &[5, 6, 7, 8, 9, 10], "");
        assert_eq!(
            show(&out),
            [
                row(1, 4, "Pneumatics"),
                row(11, 14, "Pneumatics"),
                row(15, 20, "Spare")
            ]
        );
    }

    /// SE8: a group with an unknown member keeps it on each piece and is never absorbed.
    #[test]
    fn unknown_members_survive_on_each_piece() {
        let mut groups = golden();
        groups[1].extra.insert("x-note".into(), json!(1));
        let out = relabel(groups.clone(), &order(), &[7, 8], "Reserve");
        assert!(out[1].extra.contains_key("x-note") && out[3].extra.contains_key("x-note"));
        assert_eq!(
            (out[1].from_seq, out[1].to_seq, out[3].from_seq, out[3].to_seq),
            (5, 6, 9, 10)
        );
        // "Spare" on 11..14 next to the 5..10 piece with an unknown member: not absorbed into the new group.
        let out = relabel(groups, &order(), &[11, 12, 13, 14], "Spare");
        assert_eq!(
            show(&out),
            [row(1, 4, "Pneumatics"), row(5, 10, "Spare"), row(11, 20, "Spare")]
        );
        assert!(out[1].extra.contains_key("x-note"));
    }

    /// A piece without terminals is dropped; groups the edit does not touch stay verbatim.
    #[test]
    fn empty_pieces_drop_and_untouched_groups_stay() {
        let order = [1, 2, 3, 5];
        let out = relabel(vec![g(1, 5, "A"), g(7, 9, "B")], &order, &[3, 5], "");
        assert_eq!(show(&out), [row(1, 2, "A"), row(7, 9, "B")]);
    }
}
