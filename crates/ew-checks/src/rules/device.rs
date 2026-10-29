// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-CORE device and part rules of the 04x block (doc 15 §4.2; WP-S1.16 part 3): EW-CORE-041 to -045 and -048,
//! and EW-CORE-008's `form` half (FT-368). EW-CORE-047 is in [`core`](super::core).
//!
//! # Determinism (WP-S1.16 §10)
//!
//! Texts name devices by their derived tags and parts by their part numbers; lists inside a message are in natural
//! order of their printed text. IDs enter only targets and fingerprints.

use std::collections::BTreeMap;

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::natural_cmp;
use ew_derive::ew_schema::{AddrSpec, BomBasis, ContactKind, Device, FormDef, Kind, LibCopy, Page, Part};
use ew_derive::{ChannelsByModule, FrameSettings, Q, folio_label, manufacturer_name};

use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::core::DANGLING_REFERENCE;
use crate::rules::term::full_tag;

/// The part uses of a device with their copies, in printed order (part number in natural order, then the copy
/// ID); a use whose copy the project does not hold is left out (EW-CORE-008 reports a dangling reference).
fn parts_of(q: &Q, device: &Device) -> Vec<(Id<Part>, std::sync::Arc<LibCopy<Part>>)> {
    let parts = q.parts();
    let mut out: Vec<(Id<Part>, std::sync::Arc<LibCopy<Part>>)> = device
        .parts
        .iter()
        .filter_map(|u| parts.get(u.part.copy).map(|p| (u.part.copy, p.clone())))
        .collect();
    out.sort_by(|(ia, a), (ib, b)| {
        let (na, nb) = (a.item.part_number.as_str(), b.item.part_number.as_str());
        natural_cmp(na, nb)
            .then_with(|| na.cmp(nb))
            .then_with(|| ia.cmp(ib))
    });
    out.dedup_by_key(|(id, _)| *id);
    out
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-041

/// EW-CORE-041 "Device without part" (doc 15 §4.2): completeness, o r, warning, device.
pub const DEVICE_WITHOUT_PART: RuleMeta = RuleMeta {
    id: "EW-CORE-041",
    code: "102041",
    pack: "EW-CORE",
    title: "Device without part",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Device,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-CORE-041: a device without a part use (FR-CHK-024; D-04, EF-04, EF-19). The device half only: a device that
/// is not placed is EW-LAY-001's (WP-S4.01). The exemption of a device "supplied by others" (`supply_scope =
/// by_others`, FR-DEV-037, decision log ID-322) waits for the attribute (FT-209): until it exists the rule fires for
/// such devices too. Left out: a strip (its terminals carry the parts), a cable (its part is the cable part, and a
/// cable without one is reported by its length and core rules), a PLC configuration device and a device without
/// functions (nothing is drawn or wired, so nothing is ordered). One finding per device: target the device, no
/// discriminator, so the fingerprint holds only the device's ID (doc 15 §10.1: waiver W-1 survives a renumber).
#[derive(Clone, Copy, Debug)]
pub struct DeviceWithoutPart;

impl Rule for DeviceWithoutPart {
    fn meta(&self) -> &'static RuleMeta {
        &DEVICE_WITHOUT_PART
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(device) = *scope else {
            return Vec::new();
        };
        let Some(d) = q.devices().get(device) else {
            return Vec::new();
        };
        if !d.parts.is_empty() || d.aspects.strip.is_some() || d.aspects.cable.is_some() {
            return Vec::new();
        }
        if q.idx().functions_of_device(device).next().is_none() {
            return Vec::new();
        }
        let tag = full_tag(q, device, "device");
        vec![Finding::new(
            &DEVICE_WITHOUT_PART,
            [Target::new(Kind::Device, device)],
            "",
            tag.clone(),
            format!("{tag} has no part; assign one, or mark it as supplied by others"),
        )]
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-042

/// EW-CORE-042 "Incomplete part data" (doc 15 §4.2): completeness, o r, warning, device.
pub const INCOMPLETE_PART_DATA: RuleMeta = RuleMeta {
    id: "EW-CORE-042",
    code: "102042",
    pack: "EW-CORE",
    title: "Incomplete part data",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Device,
    supersedes: &[],
    release: Release::Mvp,
};

/// What a part copy lacks for EW-CORE-042, in a fixed order: `description`, `manufacturer`.
fn missing_part_data(part: &Part) -> Vec<&'static str> {
    let mut out = Vec::new();
    if part.description.is_empty() {
        out.push("description");
    }
    if manufacturer_name(&part.manufacturer).is_empty() {
        out.push("manufacturer");
    }
    out
}

/// EW-CORE-042: a part copy used by the device with an empty description or without a manufacturer (the copy's
/// manufacturer reference names none) (FR-CHK-024, D-04). One finding per device and part copy: targets the device and
/// the copy, discriminator `part-data`.
///
/// The catalogue's third condition, "no CP pattern", is not checked: Golden A's CPX modules (parts 195740 and 533361,
/// black boxes) carry no CP pattern, and doc 20 §18.1 expects no EW-CORE-042 finding (WP-S1.16 part 3 report: which
/// parts a CP pattern is due for is open).
#[derive(Clone, Copy, Debug)]
pub struct IncompletePartData;

impl Rule for IncompletePartData {
    fn meta(&self) -> &'static RuleMeta {
        &INCOMPLETE_PART_DATA
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(device) = *scope else {
            return Vec::new();
        };
        let Some(d) = q.devices().get(device) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (id, part) in parts_of(q, d) {
            let missing = missing_part_data(&part.item);
            if missing.is_empty() {
                continue;
            }
            let tag = full_tag(q, device, "device");
            let number = part.item.part_number.as_str();
            out.push(Finding::new(
                &INCOMPLETE_PART_DATA,
                [Target::new(Kind::Device, device), Target::new(Kind::Part, id)],
                "part-data",
                tag.clone(),
                format!("part {number} of {tag} has no {}", missing.join(", no ")),
            ));
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-043

/// EW-CORE-043 "Placeholder part" (doc 15 §4.2): completeness, o r, warning (release: error), device.
pub const PLACEHOLDER_PART: RuleMeta = RuleMeta {
    id: "EW-CORE-043",
    code: "102043",
    pack: "EW-CORE",
    title: "Placeholder part",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: Some(Severity::Error),
    scope: ScopeKind::Device,
    supersedes: &[],
    release: Release::Mvp,
};

/// The part number of a placeholder ("to be defined") part.
const PLACEHOLDER_NUMBER: &str = "TBD";

/// EW-CORE-043: a placeholder part: a part copy whose part number is `TBD` (FR-CHK-034, D-04, EF-17). The schema has
/// no typed placeholder marker on a part; Golden A's four placeholder parts carry the part number `TBD` (doc 20
/// §14.1, decision log ID-316, ID-317). One finding per placeholder copy, as EF-17 counts them: targets the copy and
/// every device that uses it, the fingerprint keyed on the copy, discriminator `placeholder`. It is reported in the
/// scope of the first of those devices by printed tag, so a further user of the part changes neither the count nor
/// the fingerprint. A warning online, an error in release mode (an open item that blocks release).
#[derive(Clone, Copy, Debug)]
pub struct PlaceholderPart;

impl Rule for PlaceholderPart {
    fn meta(&self) -> &'static RuleMeta {
        &PLACEHOLDER_PART
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(device) = *scope else {
            return Vec::new();
        };
        let Some(d) = q.devices().get(device) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (id, part) in parts_of(q, d) {
            if part.item.part_number.as_str() != PLACEHOLDER_NUMBER {
                continue;
            }
            // Every device that uses the copy, by printed tag.
            let mut users: Vec<(String, Id<Device>)> = q
                .devices()
                .iter()
                .filter(|(_, x)| x.parts.iter().any(|u| u.part.copy == id))
                .map(|(x, _)| (full_tag(q, x, "device"), x))
                .collect();
            users.sort_by(|a, b| {
                natural_cmp(&a.0, &b.0)
                    .then_with(|| a.0.cmp(&b.0))
                    .then_with(|| a.1.cmp(&b.1))
            });
            if users.first().map(|(_, x)| *x) != Some(device) {
                continue;
            }
            let what = part
                .item
                .description
                .as_neutral()
                .map(str::to_owned)
                .or_else(|| {
                    part.item
                        .description
                        .languages()
                        .find_map(|l| part.item.description.get(l).map(str::to_owned))
                })
                .unwrap_or_else(|| "a part".to_owned());
            let tags: Vec<&str> = users.iter().map(|(t, _)| t.as_str()).collect();
            out.push(
                Finding::new(
                    &PLACEHOLDER_PART,
                    std::iter::once(Target::new(Kind::Part, id))
                        .chain(users.iter().map(|(_, x)| Target::new(Kind::Device, *x))),
                    "placeholder",
                    format!("TBD {what}"),
                    format!(
                        "placeholder part \"{what}\" (part number TBD) is used by {}; choose the part before release",
                        tags.join(", ")
                    ),
                )
                .keyed_on(&[Target::new(Kind::Part, id)]),
            );
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-044

/// EW-CORE-044 "Open `Tbd` value" (doc 15 §4.2): completeness, o r, info (release: error), device.
pub const OPEN_TBD_VALUE: RuleMeta = RuleMeta {
    id: "EW-CORE-044",
    code: "102044",
    pack: "EW-CORE",
    title: "Open Tbd value",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Info,
    release_severity: Some(Severity::Error),
    scope: ScopeKind::Device,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-CORE-044: a typed `Tbd` value of the device (FR-CHK-034, D-15): a PLC module's input or output start and the
/// address of each of its channels (`AddrSpec::Tbd`). These are all the typed `Tbd` values of schema v0; a bus
/// address is not typed yet (`BusPort`, doc 03 §14), so EW-BUS-001's `Tbd` addresses are not reachable here. When
/// EW-PLC-015 (WP-S1.21) is built, its findings supersede these for the same targets (doc 15 §3.6). One finding per
/// value: the module (and the channel) as targets, discriminator `start.input`, `start.output` or `address`. An info
/// online, an error in release mode (an open item that blocks release).
#[derive(Clone, Copy, Debug)]
pub struct OpenTbdValue;

impl Rule for OpenTbdValue {
    fn meta(&self) -> &'static RuleMeta {
        &OPEN_TBD_VALUE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(device) = *scope else {
            return Vec::new();
        };
        let Some(module) = q.devices().get(device).and_then(|d| d.aspects.plc_module.clone()) else {
            return Vec::new();
        };
        let tag = full_tag(q, device, "module");
        let mut out = Vec::new();
        for (which, spec) in [("input", &module.start.input), ("output", &module.start.output)] {
            if *spec == AddrSpec::Tbd {
                out.push(Finding::new(
                    &OPEN_TBD_VALUE,
                    [Target::new(Kind::Device, device)],
                    format!("start.{which}"),
                    tag.clone(),
                    format!("{tag}: the {which} start address is TBD; enter it before release"),
                ));
            }
        }
        let channels = q.get::<ChannelsByModule>(());
        let table = q.table::<ew_derive::ew_schema::Channel>();
        for (number, id) in channels.of_module(device) {
            if table.get(id).is_some_and(|c| c.address == AddrSpec::Tbd) {
                out.push(Finding::new(
                    &OPEN_TBD_VALUE,
                    [Target::new(Kind::Device, device), Target::new(Kind::Channel, id)],
                    "address",
                    tag.clone(),
                    format!("{tag}: the address of channel {number} is TBD; enter it before release"),
                ));
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-045

/// EW-CORE-045 "Auxiliary contacts exceed the part" (doc 15 §4.2): conflict, o r, error, device.
pub const CONTACTS_EXCEED_PART: RuleMeta = RuleMeta {
    id: "EW-CORE-045",
    code: "102045",
    pack: "EW-CORE",
    title: "Auxiliary contacts exceed the part",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Device,
    supersedes: &[],
    release: Release::Mvp,
};

/// Counts of normally open, normally closed and change-over contacts.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
struct ContactCount {
    no: u32,
    nc: u32,
    co: u32,
}

impl ContactCount {
    fn add(&mut self, kind: &ContactKind, n: u32) {
        match kind {
            ContactKind::No => self.no += n,
            ContactKind::Nc => self.nc += n,
            ContactKind::Co => self.co += n,
            _ => {}
        }
    }

    fn text(self) -> String {
        format!("{} NO, {} NC, {} CO", self.no, self.nc, self.co)
    }

    /// Whether `need` fits these contacts: change-overs serve change-overs first, then any normally open or normally
    /// closed contact the part lacks.
    fn holds(self, need: ContactCount) -> bool {
        if need.co > self.co {
            return false;
        }
        let spare = self.co - need.co;
        let short = need.no.saturating_sub(self.no) + need.nc.saturating_sub(self.nc);
        short <= spare
    }
}

/// EW-CORE-045: the contact functions of a device (normally open, normally closed and change-over, by their function
/// definitions' `contact_kind`) exceed, or differ in kind from, the contacts of the device's parts (`Part.contacts`,
/// WP-S1.19 part 1; FR-CHK-036, STD XR-02). Every contact function of the device counts, the main one included,
/// since each uses a contact of the part; a part's change-over contact serves a normally open or closed one. The
/// contacts of every part use add up (an auxiliary contact block is a part use of the device); a device whose parts
/// carry no contact data is not judged. One finding per device: target the device, discriminator `contacts`.
#[derive(Clone, Copy, Debug)]
pub struct ContactsExceedPart;

impl Rule for ContactsExceedPart {
    fn meta(&self) -> &'static RuleMeta {
        &CONTACTS_EXCEED_PART
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(device) = *scope else {
            return Vec::new();
        };
        let Some(d) = q.devices().get(device) else {
            return Vec::new();
        };
        let parts = q.parts();
        let mut have = ContactCount::default();
        let mut has_data = false;
        for u in &d.parts {
            let Some(p) = parts.get(u.part.copy) else {
                continue;
            };
            if p.item.contacts.is_empty() {
                continue;
            }
            has_data = true;
            // A part use counts once per whole piece of its quantity, at least once.
            let pieces = u32::try_from(u.qty.0 / 1000).unwrap_or(u32::MAX).max(1);
            for c in &p.item.contacts {
                have.add(&c.kind, pieces);
            }
        }
        if !has_data {
            return Vec::new();
        }
        let mut need = ContactCount::default();
        for f in q.idx().functions_of_device(device) {
            let Some(def) = q.functions().get(f).and_then(|f| q.fndefs().get(f.def)) else {
                continue;
            };
            need.add(&def.item.contact_kind, 1);
        }
        if have.holds(need) {
            return Vec::new();
        }
        let tag = full_tag(q, device, "device");
        vec![Finding::new(
            &CONTACTS_EXCEED_PART,
            [Target::new(Kind::Device, device)],
            "contacts",
            tag.clone(),
            format!(
                "{tag} uses {} contacts; its parts have {}",
                need.text(),
                have.text()
            ),
        )]
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-048

/// EW-CORE-048 "Order codes do not cover the children" (doc 15 §4.2): conflict, o r, warning, device.
pub const ORDER_CODES_COVER: RuleMeta = RuleMeta {
    id: "EW-CORE-048",
    code: "102048",
    pack: "EW-CORE",
    title: "Order codes do not cover the children",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Device,
    supersedes: &[],
    release: Release::Mvp,
};

/// The descendants of `head` (children, their children, …), each once, in ID order; a cycle ends the walk.
fn descendants(q: &Q, head: Id<Device>) -> Vec<Id<Device>> {
    let mut out: Vec<Id<Device>> = Vec::new();
    let mut stack: Vec<Id<Device>> = vec![head];
    while let Some(d) = stack.pop() {
        for c in q.idx().device_children(d) {
            if c != head && !out.contains(&c) {
                out.push(c);
                stack.push(c);
            }
        }
    }
    out.sort();
    out
}

/// EW-CORE-048: an assembly with `bom_basis = assembly` whose order codes do not cover each of its descendants with
/// part uses exactly once (FR-DEV-020, D-36): only an order code with a configured part covers (doc 03 §7: one
/// without prints and covers nothing); covers are not transitive (decision log ID-371), so every descendant with own
/// part lines is named, and a device named by two order codes would print twice. The assembly device itself may be
/// covered, at most once. A descendant without part uses has no line to replace. One finding per device and
/// condition: targets the assembly and the device, discriminator `uncovered` or `covered-twice`. A cover outside the
/// assembly's subtree (FT-260) is not judged here.
#[derive(Clone, Copy, Debug)]
pub struct OrderCodesCover;

impl Rule for OrderCodesCover {
    fn meta(&self) -> &'static RuleMeta {
        &ORDER_CODES_COVER
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(head) = *scope else {
            return Vec::new();
        };
        let Some(assembly) = q.devices().get(head).and_then(|d| d.aspects.assembly.clone()) else {
            return Vec::new();
        };
        if assembly.bom_basis != BomBasis::Assembly {
            return Vec::new();
        }
        let mut covered: BTreeMap<Id<Device>, u32> = BTreeMap::new();
        for config in assembly.configs.iter().filter(|c| c.part.is_some()) {
            for d in config.covers.iter() {
                *covered.entry(*d).or_default() += 1;
            }
        }
        let tag = full_tag(q, head, "assembly");
        let mut rows: Vec<(String, Id<Device>, &'static str, String)> = Vec::new();
        let with_parts = |d: Id<Device>| q.devices().get(d).is_some_and(|x| !x.parts.is_empty());
        for d in std::iter::once(head).chain(descendants(q, head)) {
            let n = covered.get(&d).copied().unwrap_or(0);
            let child = full_tag(q, d, "device");
            if n > 1 {
                rows.push((
                    child.clone(),
                    d,
                    "covered-twice",
                    format!(
                        "{child} is covered by {n} order codes of {tag}; its parts would print {n} times"
                    ),
                ));
            } else if n == 0 && d != head && with_parts(d) {
                rows.push((
                    child.clone(),
                    d,
                    "uncovered",
                    format!(
                        "{child} is covered by no order code of {tag}; the assembly BOM lists its own parts beside the configured products"
                    ),
                ));
            }
        }
        rows.sort_by(|a, b| {
            natural_cmp(&a.0, &b.0)
                .then_with(|| a.0.cmp(&b.0))
                .then_with(|| a.1.cmp(&b.1))
        });
        rows.into_iter()
            .map(|(subject, d, disc, message)| {
                Finding::new(
                    &ORDER_CODES_COVER,
                    [Target::new(Kind::Device, head), Target::new(Kind::Device, d)],
                    disc,
                    subject,
                    message,
                )
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-008, the `form` half (FT-368)

/// EW-CORE-008 `form`: a page without a form of its own whose frame defaults (`settings.frames`, doc 03 §5.1) name a
/// form copy the project does not hold, so its frame falls through to the next rule (decision log ID-547 item 4,
/// FT-368). A page's own `form` is a checked link (`Model::from_records` refuses a dangling one); the frame defaults
/// are a settings member that is not. Reported per stored page, as the cell grid resolves it (`FrameGridOf`): the
/// entry of the page's type, then the default, whichever the resolution consults and finds missing. Target the page, discriminator `form`. An integrity rule of release mode: it runs in `ew check` and the
/// release gate, not online.
#[derive(Clone, Copy, Debug)]
pub struct DanglingFrameForm;

impl Rule for DanglingFrameForm {
    fn meta(&self) -> &'static RuleMeta {
        &DANGLING_REFERENCE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let settings = q.get::<FrameSettings>(());
        let forms = q.forms();
        let held = |id: Id<FormDef>| forms.contains(id);
        let mut pages: Vec<(Id<Page>, String)> = q
            .pages()
            .iter()
            .filter(|(_, p)| p.form.is_none())
            .map(|(id, p)| (id, folio_label(p)))
            .collect();
        pages.sort_by(|a, b| {
            natural_cmp(&a.1, &b.1)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| a.0.cmp(&b.0))
        });
        let mut out = Vec::new();
        for (id, folio) in pages {
            let Some(page) = q.pages().get(id) else {
                continue;
            };
            let type_name = serde_json::to_value(&page.kind)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default();
            let mut missing: Vec<String> = Vec::new();
            let entry = settings
                .per_page_type
                .iter()
                .find(|e| e.page_type == type_name)
                .and_then(|e| e.form);
            match entry {
                Some(f) if held(f) => continue,
                Some(f) => missing.push(format!("the frame for page type {type_name} names form copy {f}")),
                None => {}
            }
            if let Some(f) = settings.default
                && !held(f)
            {
                missing.push(format!("the default frame names form copy {f}"));
            }
            if missing.is_empty() {
                continue;
            }
            out.push(
                Finding::new(
                    &DANGLING_REFERENCE,
                    [Target::new(Kind::Page, id)],
                    "form",
                    format!("folio {folio}"),
                    format!(
                        "folio {folio}: {}, which the project does not hold; the page takes the next frame",
                        missing.join(", and ")
                    ),
                )
                .on_page(id),
            );
        }
        out
    }
}
