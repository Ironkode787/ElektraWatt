// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The part selection dialog's data. [`Session::part_offers`] merges the project's part
//! copies and the catalogue hits of [`Session::part_search`] into one list ordered for the target (the fit to its
//! function, the rank of the text, found by what the part is for before found by its words, held before not held, favourites and recently used parts, the catalogue's order,
//! the natural part number); [`Session::part_pick_command`] compiles a choice into the one command of its gesture
//! (`part.assign` for devices, the strip cell's `fields.set` with `copies` for a bridge or an accessory part).

use std::cmp::Ordering;
use std::collections::BTreeSet;

use ew_commands::commands::{AssignModeParam, AssignPart, AssignPlaceholder};
use ew_derive::Q;
use ew_derive::ew_ids::{Id, Uuid, v5};
use ew_schema::{Device, FnDef, ItemStatus, LibDocument, LibRef, Part, PartKind};
use ew_units::Dec3;

use crate::library::{FnCompat, PartQuery, Tier};
use crate::output::StripEdit;
use crate::{AnyCommand, ApiError, AssignMode, Session};

/// What the part selection dialog chooses a part for (D16).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PartTarget {
    /// The part uses of devices or terminals: one `part.assign` in `mode`.
    Devices {
        /// The devices.
        devices: Vec<Uuid>,
        /// How their part uses change.
        mode: AssignMode,
    },
    /// The bridge part of terminals (the strip editor's Bridge part cell, D6).
    BridgePart {
        /// The terminal devices.
        terminals: Vec<Uuid>,
    },
    /// The part of one accessory of a strip (the strip editor's accessory Part cell, D5).
    AccessoryPart {
        /// The strip device.
        strip: Uuid,
        /// The index in `Strip.accessories`.
        index: u32,
    },
}

/// One choice of the dialog.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum PartPick {
    /// A part copy the project holds (its copy ID).
    Held(Uuid),
    /// A catalogue part the project does not hold.
    Library {
        /// The library ID.
        library: Uuid,
        /// The item ID.
        item: Uuid,
    },
}

/// How an offer fits the target's function (D17).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Fit {
    /// The part's function template declares the function's FnDef (the same library and item, any version).
    SameFnDef,
    /// A template function has the function's first two category levels and contact kind.
    SameCategory,
    /// Neither, no template, or no function to fit.
    Other,
}

impl Fit {
    /// The describe word (`same-fndef`, `same-category`, `other`).
    pub fn word(self) -> &'static str {
        match self {
            Fit::SameFnDef => "same-fndef",
            Fit::SameCategory => "same-category",
            Fit::Other => "other",
        }
    }
}

/// Which offers the dialog lists (D17).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum OfferView {
    /// The project's copies and the catalogue.
    #[default]
    All,
    /// The project's copies only.
    Project,
    /// The favourites only.
    Favourites,
    /// The recently used parts only, most recent first.
    Recent,
}

impl OfferView {
    /// The views in the dialog's order.
    pub const ALL: [OfferView; 4] = [
        OfferView::All,
        OfferView::Project,
        OfferView::Favourites,
        OfferView::Recent,
    ];

    /// The parameter and describe word (`all`, `project`, `favourites`, `recent`).
    pub fn word(self) -> &'static str {
        match self {
            OfferView::All => "all",
            OfferView::Project => "project",
            OfferView::Favourites => "favourites",
            OfferView::Recent => "recent",
        }
    }

    /// The view of a word of [`OfferView::word`].
    pub fn from_word(word: &str) -> Option<OfferView> {
        OfferView::ALL.into_iter().find(|v| v.word() == word)
    }
}

/// The dialog's query (D17).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PartOfferQuery {
    /// The typed text: a part number in any spelling, or words of the type, manufacturer and description.
    pub text: String,
    /// Only these part kinds' words (`end_bracket`, `accessory`); all when empty.
    pub kinds: Vec<String>,
    /// A category prefix (the first level, then the second); all when empty.
    pub category: Vec<String>,
    /// The target's function, whose FnDef orders the offers by their fit.
    pub function: Option<Uuid>,
    /// Hide the offers that fit [`Fit::Other`] (only with a function).
    pub fits_only: bool,
    /// The view.
    pub view: OfferView,
    /// Also deprecated and withdrawn parts (FR-PRT-034).
    pub include_inactive: bool,
    /// `(library, item)` of the favourites (D18).
    pub favourites: Vec<(Uuid, Uuid)>,
    /// `(library, item)` of the recently used parts, most recent first (D18).
    pub recent: Vec<(Uuid, Uuid)>,
    /// What the text names a part as being for (FT-1845): first category levels (`SensingPosition`), which the
    /// caller reads from the text in the words of the categories; a part whose own category, or whose main
    /// function's FnDef category, starts with one is found by it.
    pub purpose_categories: Vec<String>,
    /// What the text names a part as being for: contact kinds' words (`no`); a part whose main function's FnDef has
    /// one is found by it.
    pub purpose_kinds: Vec<String>,
}

/// One row of the dialog's list.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PartOffer {
    /// What choosing the row picks.
    pub pick: PartPick,
    /// The part number, verbatim.
    pub part_number: String,
    /// The manufacturer's name.
    pub manufacturer: String,
    /// The description (the neutral text, else the first language).
    pub description: String,
    /// The part kind's word (`device`, `terminal`, `end_bracket`).
    pub kind: String,
    /// The fit to the target's function.
    pub fit: Fit,
    /// Whether the project holds the part.
    pub held: bool,
    /// The library the part comes from, as installed: (ID, name, tier); `None` for a project-local copy or a library
    /// that is not installed.
    pub library: Option<(Uuid, String, Tier)>,
    /// `(library, item)` of a library part (the favourites' and recent list's key); `None` for a project-local copy.
    pub item: Option<(Uuid, Uuid)>,
    /// The version.
    pub version: String,
    /// The status word (`released`, `draft`, `deprecated`, `withdrawn`).
    pub status: String,
    /// The successor's part number of a deprecated part (FR-PRT-034).
    pub successor: Option<String>,
    /// Whether the part is a favourite.
    pub favourite: bool,
    /// Whether the part was used recently.
    pub recent: bool,
}

/// The neutral text of a description, else its first language's.
pub(crate) fn text_of(t: &ew_mltext::MlText) -> String {
    if let Some(n) = t.as_neutral() {
        return n.to_owned();
    }
    t.languages()
        .next()
        .and_then(|l| t.get(l))
        .unwrap_or_default()
        .to_owned()
}

/// The word of a part kind (its JSON form).
pub(crate) fn kind_word(kind: &PartKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn status_word(s: &ItemStatus) -> &'static str {
    match s {
        ItemStatus::Draft => "draft",
        ItemStatus::Released => "released",
        ItemStatus::Deprecated(_) => "deprecated",
        ItemStatus::Withdrawn => "withdrawn",
        ItemStatus::Unknown(_) => "unknown",
    }
}

fn inactive(s: &ItemStatus) -> bool {
    matches!(s, ItemStatus::Deprecated(_) | ItemStatus::Withdrawn)
}

/// The rank of a held copy for the text (D17 (2)): 0 without text, 1 the normalised part number equals it, 2 starts
/// with it, 3 contains it (three characters or more), 4 every word is in the part number, type, manufacturer or
/// description (in every language), or the part is for what the text names (`purpose`); `None` when it does not
/// match.
fn held_rank(text: &str, part: &Part, manufacturer: &str, purpose: bool) -> Option<u8> {
    if text.is_empty() {
        return Some(0);
    }
    let n = crate::library::pn_norm(text);
    let pn = crate::library::pn_norm(part.part_number.as_str());
    if !n.is_empty() {
        if pn == n {
            return Some(1);
        }
        if pn.starts_with(&n) {
            return Some(2);
        }
        if n.chars().count() >= 3 && pn.contains(&n) {
            return Some(3);
        }
    }
    let descriptions: Vec<String> = part
        .description
        .languages()
        .filter_map(|l| part.description.get(l).map(str::to_owned))
        .chain(part.description.as_neutral().map(str::to_owned))
        .collect();
    let hay = format!(
        "{} {} {} {}",
        part.part_number.as_str(),
        part.type_designation.as_ref().map_or("", |t| t.as_str()),
        manufacturer,
        descriptions.join(" ")
    )
    .to_lowercase();
    (purpose || text.split_whitespace().all(|w| hay.contains(&w.to_lowercase()))).then_some(4)
}

fn category_matches(category: Option<&ew_schema::CategoryKey>, prefix: &[String]) -> bool {
    if prefix.is_empty() {
        return true;
    }
    let Some((a, b, c)) = category else {
        return false;
    };
    [a, b, c]
        .iter()
        .zip(prefix)
        .all(|(level, want)| level.as_str() == want.as_str())
        && prefix.len() <= 3
}

/// The order key of an offer (D17).
struct Key {
    recent_pos: usize,
    fit: Fit,
    rank: u8,
    /// Not found by what it is for (FT-1845): false leads its rank.
    not_purpose: bool,
    not_held: bool,
    boost: u8,
    score: usize,
    part_number: String,
    manufacturer: String,
    library: Uuid,
    item: Uuid,
}

fn key_order(a: &Key, b: &Key) -> Ordering {
    a.recent_pos
        .cmp(&b.recent_pos)
        .then(a.fit.cmp(&b.fit))
        .then(a.rank.cmp(&b.rank))
        .then(a.not_purpose.cmp(&b.not_purpose))
        .then(a.not_held.cmp(&b.not_held))
        .then(a.boost.cmp(&b.boost))
        .then(a.score.cmp(&b.score))
        .then_with(|| ew_mltext::natural_cmp(&a.part_number, &b.part_number))
        .then_with(|| ew_mltext::natural_cmp(&a.manufacturer, &b.manufacturer))
        .then(a.library.cmp(&b.library))
        .then(a.item.cmp(&b.item))
}

impl Session {
    /// The FnDef of a function as the fit rule reads it: its category's first two levels, contact kind and
    /// `(library, item)`.
    fn function_compat(&self, q: &Q, function: Uuid) -> Option<FnCompat> {
        let f = q.functions().get(Id::from_uuid(function))?;
        let def = q.fndefs().get(f.def)?;
        let (c0, c1, _) = &def.item.category;
        Some(FnCompat {
            category: [c0.as_str().to_owned(), c1.as_str().to_owned()],
            contact_kind: def.item.contact_kind.clone(),
            fndef: Some((def.library_id.as_uuid(), def.item_id.as_uuid())),
        })
    }

    /// The FnDef item a template reference names: the held copy, else the libraries'.
    fn template_fndef(&self, q: &Q, r: &LibRef) -> Option<FnDef> {
        let copy: Id<FnDef> = v5::lib_copy_id(r.library.as_uuid(), r.item.as_uuid());
        if let Some(c) = q.fndefs().get(copy) {
            return Some(c.item.clone());
        }
        match self.libraries().resolve(r) {
            Ok(LibDocument::Fndef(c)) => Some(c.item),
            _ => None,
        }
    }

    /// Whether a held part is for what the query names (FT-1845): its own category's first level, or its main
    /// template function's FnDef's first category level or contact kind, is among the query's purposes.
    fn held_purpose(&self, q: &Q, part: &Part, query: &PartOfferQuery) -> bool {
        if query.purpose_categories.is_empty() && query.purpose_kinds.is_empty() {
            return false;
        }
        let named = |c: &str| query.purpose_categories.iter().any(|p| p == c);
        if part.category.as_ref().is_some_and(|(c0, _, _)| named(c0.as_str())) {
            return true;
        }
        part.function_template
            .iter()
            .filter(|t| t.main)
            .filter_map(|t| self.template_fndef(q, &t.fndef))
            .any(|d| {
                let kind = serde_json::to_value(&d.contact_kind).unwrap_or_default();
                named(d.category.0.as_str()) || query.purpose_kinds.iter().any(|k| kind.as_str() == Some(k))
            })
    }

    /// The fit of a held part to a function (D17, by the rules the catalogue applies to its hits).
    fn held_fit(&self, q: &Q, part: &Part, compat: &FnCompat) -> Fit {
        let mut same_category = false;
        for t in &part.function_template {
            if compat.fndef == Some((t.fndef.library.as_uuid(), t.fndef.item.as_uuid())) {
                return Fit::SameFnDef;
            }
            if let Some(d) = self.template_fndef(q, &t.fndef) {
                let (c0, c1, _) = &d.category;
                same_category |= c0.as_str() == compat.category[0]
                    && c1.as_str() == compat.category[1]
                    && d.contact_kind == compat.contact_kind;
            }
        }
        if same_category {
            Fit::SameCategory
        } else {
            Fit::Other
        }
    }

    /// The part number of a successor reference, else the reference's item ID.
    fn successor_text(&self, status: &ItemStatus) -> Option<String> {
        let ItemStatus::Deprecated(d) = status else {
            return None;
        };
        let r = d.successor.as_ref()?;
        Some(match self.libraries().resolve(r) {
            Ok(LibDocument::Part(c)) => c.item.part_number.as_str().to_owned(),
            _ => r.item.as_uuid().to_string(),
        })
    }

    /// The layout kinds a part dialog for `devices` offers: an end bracket only when every
    /// device is a strip (its `aspects.strip` is set, with or without terminals); an enclosure and a mounting plate only
    /// when every device has no main function; a rail and a duct never (they are layout items, not part uses of a
    /// device). Every other device is offered none. Empty when `devices` is empty or one names no device.
    pub fn chooser_layout_kinds(&self, devices: &[Uuid]) -> Vec<String> {
        if devices.is_empty() {
            return Vec::new();
        }
        let q = self.query();
        let exists = |d: &Uuid| {
            let id: Id<Device> = Id::from_uuid(*d);
            q.devices().get(id).cloned()
        };
        let Some(found) = devices.iter().map(exists).collect::<Option<Vec<_>>>() else {
            return Vec::new();
        };
        let mut kinds: Vec<&str> = Vec::new();
        if found.iter().all(|d| d.aspects.strip.is_some()) {
            kinds.push("end_bracket");
        }
        if devices.iter().all(|d| self.main_function(*d).is_none()) {
            kinds.push("enclosure");
            kinds.push("mounting_plate");
        }
        kinds.into_iter().map(str::to_owned).collect()
    }

    /// The part selection dialog's list (D17): the project's part copies and the catalogue hits, one row per
    /// `(library, item)` (an item the project holds is listed once, as the held copy), filtered by the query and
    /// ordered for the target: the fit to the function (when the query names one), the rank of the text, held before
    /// not held, favourites then recently used parts, the catalogue's full-text order, the natural part number, the
    /// manufacturer, the library and item IDs. The view [`OfferView::Recent`] lists the recently used parts most
    /// recent first.
    pub fn part_offers(&self, q: &PartOfferQuery) -> Vec<PartOffer> {
        let query = self.query();
        let project = query.project().id.as_uuid();
        let compat = q.function.and_then(|f| self.function_compat(&query, f));
        let libs = self.libraries().libraries();
        let lib_of = |id: Uuid| {
            libs.iter()
                .find(|l| l.id == id)
                .map(|l| (l.id, l.name.clone(), l.tier))
        };
        let text = q.text.trim();
        let favourite = |k: Option<(Uuid, Uuid)>| k.is_some_and(|k| q.favourites.contains(&k));
        let recent_pos = |k: Option<(Uuid, Uuid)>| k.and_then(|k| q.recent.iter().position(|r| *r == k));
        let mut rows: Vec<(Key, PartOffer)> = Vec::new();
        let mut held_items: BTreeSet<(Uuid, Uuid)> = BTreeSet::new();
        for (id, c) in query.parts().iter() {
            held_items.insert((c.library_id.as_uuid(), c.item_id.as_uuid()));
            let p = &c.item;
            let kind = kind_word(&p.kind);
            if !q.kinds.is_empty() && !q.kinds.contains(&kind) {
                continue;
            }
            if !category_matches(p.category.as_ref(), &q.category) {
                continue;
            }
            if inactive(&p.meta.status) && !q.include_inactive {
                continue;
            }
            let manufacturer = ew_derive::manufacturer_name(&query, &p.manufacturer);
            let description = text_of(&p.description);
            let purpose = self.held_purpose(&query, p, q);
            let Some(rank) = held_rank(text, p, &manufacturer, purpose) else {
                continue;
            };
            let lib = c.library_id.as_uuid();
            let item = (lib != project).then_some((lib, c.item_id.as_uuid()));
            let fit = compat
                .as_ref()
                .map_or(Fit::Other, |cp| self.held_fit(&query, p, cp));
            let offer = PartOffer {
                pick: PartPick::Held(id.as_uuid()),
                part_number: p.part_number.as_str().to_owned(),
                manufacturer,
                description,
                kind,
                fit,
                held: true,
                library: item.and_then(|(l, _)| lib_of(l)),
                item,
                version: c.version.clone(),
                status: status_word(&p.meta.status).to_owned(),
                successor: self.successor_text(&p.meta.status),
                favourite: favourite(item),
                recent: recent_pos(item).is_some(),
            };
            rows.push((
                Key {
                    recent_pos: 0,
                    fit,
                    rank,
                    not_purpose: !purpose,
                    not_held: false,
                    boost: 0,
                    score: 0,
                    part_number: offer.part_number.clone(),
                    manufacturer: offer.manufacturer.clone(),
                    library: lib,
                    item: c.item_id.as_uuid(),
                },
                offer,
            ));
        }
        if q.view != OfferView::Project {
            let kinds: Vec<PartKind> = q
                .kinds
                .iter()
                .filter_map(|k| serde_json::from_value(serde_json::Value::String(k.clone())).ok())
                .collect();
            let mut boost: Vec<(Uuid, Uuid)> = q.favourites.iter().chain(&q.recent).copied().collect();
            boost.sort_unstable();
            boost.dedup();
            let hits = if !q.kinds.is_empty() && kinds.is_empty() {
                Vec::new()
            } else {
                self.part_search(&PartQuery {
                    text: text.to_owned(),
                    kinds,
                    category: q.category.clone(),
                    compatible_with: compat.clone(),
                    include_inactive: q.include_inactive,
                    boost,
                    purpose_categories: q.purpose_categories.clone(),
                    purpose_kinds: q
                        .purpose_kinds
                        .iter()
                        .filter_map(|k| serde_json::from_value(serde_json::Value::String(k.clone())).ok())
                        .collect(),
                    ..PartQuery::default()
                })
            };
            for (pos, h) in hits.into_iter().enumerate() {
                if held_items.contains(&(h.library, h.item)) {
                    continue;
                }
                let fit = match (h.same_fndef, h.compatible) {
                    (Some(true), _) => Fit::SameFnDef,
                    (_, Some(true)) => Fit::SameCategory,
                    _ => Fit::Other,
                };
                let item = Some((h.library, h.item));
                let fav = favourite(item);
                let rec = recent_pos(item).is_some();
                let offer = PartOffer {
                    pick: PartPick::Library {
                        library: h.library,
                        item: h.item,
                    },
                    part_number: h.part_number.clone(),
                    manufacturer: h.manufacturer.clone(),
                    description: h.description.clone(),
                    kind: kind_word(&h.kind),
                    fit,
                    held: false,
                    library: lib_of(h.library),
                    item,
                    version: h.version.clone(),
                    status: status_word(&h.status).to_owned(),
                    successor: self.successor_text(&h.status),
                    favourite: fav,
                    recent: rec,
                };
                rows.push((
                    Key {
                        recent_pos: 0,
                        fit,
                        rank: h.rank_tier,
                        not_purpose: !h.purpose,
                        not_held: true,
                        boost: if fav {
                            0
                        } else if rec {
                            1
                        } else {
                            2
                        },
                        score: if h.rank_tier == 4 { pos } else { 0 },
                        part_number: h.part_number,
                        manufacturer: h.manufacturer,
                        library: h.library,
                        item: h.item,
                    },
                    offer,
                ));
            }
        }
        rows.retain(|(_, o)| match q.view {
            OfferView::All => true,
            OfferView::Project => o.held,
            OfferView::Favourites => o.favourite,
            OfferView::Recent => o.recent,
        });
        if q.fits_only && compat.is_some() {
            rows.retain(|(_, o)| o.fit != Fit::Other);
        }
        if compat.is_none() {
            for (k, _) in &mut rows {
                k.fit = Fit::Other;
            }
        }
        if q.view == OfferView::Recent {
            for (k, o) in &mut rows {
                k.recent_pos = recent_pos(o.item).unwrap_or(usize::MAX);
            }
        }
        rows.sort_by(|a, b| key_order(&a.0, &b.0));
        rows.into_iter().map(|(_, o)| o).collect()
    }

    /// The part an offer picks: the held copy's item, else the library's.
    pub(crate) fn offer_part(&self, pick: &PartPick) -> Option<Part> {
        match pick {
            PartPick::Held(copy) => self
                .query()
                .parts()
                .get(Id::from_uuid(*copy))
                .map(|c| c.item.clone()),
            PartPick::Library { library, item } => {
                match self.libraries().resolve(&lib_ref(*library, *item)) {
                    Ok(LibDocument::Part(c)) => Some(c.item),
                    _ => None,
                }
            }
        }
    }

    /// The names of the functions a part's template declares (the dialog's detail line), in template order: each
    /// entry in words, as the Properties grid's Part function row reads it ("NO contact 13-14", "Coil A1-A2"), else
    /// its FnDef's key; an entry whose FnDef does not resolve has no name and is left out (FT-819: the line printed
    /// "functions ew.fn.mcb-1pn", or an item ID).
    pub fn part_offer_functions(&self, pick: &PartPick) -> Vec<String> {
        let q = self.query();
        self.offer_part(pick)
            .map(|p| {
                let pattern: Vec<u16> = p.cp_pattern.iter().map(|c| c.idx).collect();
                p.function_template
                    .iter()
                    .filter_map(|t| {
                        let def = self.template_fndef(&q, &t.fndef)?;
                        let def_cps: Vec<u16> = def.cps.iter().map(|c| c.idx).collect();
                        // The entry's CP designations, else the FnDef's, as the place catalogue reads them.
                        let mut cps: Vec<String> = t
                            .function_cps(&pattern, &def_cps)
                            .iter()
                            .filter_map(|c| p.cp_pattern.iter().find(|x| x.idx == c.pattern))
                            .map(|c| c.designation.as_str().to_owned())
                            .collect();
                        if cps.is_empty() {
                            let mut own: Vec<(u16, String)> = def
                                .cps
                                .iter()
                                .map(|c| (c.idx, c.designation.as_str().to_owned()))
                                .collect();
                            own.sort();
                            cps = own.into_iter().map(|(_, d)| d).collect();
                        }
                        crate::output::navigators::kind_text(&def, &cps)
                            .or_else(|| def.meta.key.map(|k| k.as_str().to_owned()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Type not known yet… of the part dialog (doc 07 §3.3): `part.placeholder` giving `devices` a placeholder part
    /// described by `description`, in the dialog's mode.
    ///
    /// # Errors
    ///
    /// [`ApiError::Cmd`] when the parameters have no JSON form.
    pub fn placeholder_command(
        &self,
        devices: &[Uuid],
        mode: AssignMode,
        description: &str,
    ) -> Result<AnyCommand, ApiError> {
        let (mode, from) = match mode {
            AssignMode::Append => (AssignModeParam::Append, None),
            AssignMode::ReplaceAll => (AssignModeParam::ReplaceAll, None),
            AssignMode::Replace { from } => (AssignModeParam::Replace, Some(Id::from_uuid(from))),
        };
        let cmd = AssignPlaceholder {
            devices: devices.iter().map(|d| Id::from_uuid(*d)).collect(),
            description: description.to_owned(),
            mode,
            from,
        };
        AnyCommand::of(&cmd).map_err(ApiError::Cmd)
    }

    /// The devices that carry the part copy `copy`, in ID order, when it is a placeholder (part number TBD, doc 07
    /// §3.3): a real part chosen for one of them replaces it on all; `None` for another part or a copy the project does
    /// not hold.
    pub fn placeholder_users(&self, copy: Uuid) -> Option<Vec<Uuid>> {
        let q = self.query();
        let id: Id<Part> = Id::from_uuid(copy);
        q.parts().get(id).filter(|c| c.item.is_placeholder())?;
        let mut users: Vec<Uuid> = q
            .devices()
            .iter()
            .filter(|(_, d)| d.parts.iter().any(|u| u.part.copy == id))
            .map(|(d, _)| d.as_uuid())
            .collect();
        users.sort_unstable();
        Some(users)
    }

    /// The one command of a choice (D16): for devices `part.assign` in the target's mode (a held copy without
    /// `copies`, a catalogue part with its closure); for a bridge or an accessory part the strip cell's `fields.set`
    /// (or `fields.set_many`) with the closure of a catalogue part in `copies`.
    ///
    /// # Errors
    ///
    /// [`ApiError::NotHeld`] for a held pick the project does not hold; [`ApiError::Library`],
    /// [`ApiError::OtherVersionHeld`] and [`ApiError::NotAPart`] as [`Session::assign_part`]; [`ApiError::Strip`]
    /// when the strip cell's compile refuses the part.
    pub fn part_pick_command(&self, target: &PartTarget, pick: &PartPick) -> Result<AnyCommand, ApiError> {
        let q = self.query();
        if let PartPick::Held(copy) = pick
            && !q.parts().contains(Id::from_uuid(*copy))
        {
            return Err(ApiError::NotHeld(*copy));
        }
        match (target, pick) {
            (PartTarget::Devices { devices, mode }, PartPick::Held(copy)) => {
                let (mode, from) = match mode {
                    AssignMode::Append => (AssignModeParam::Append, None),
                    AssignMode::ReplaceAll => (AssignModeParam::ReplaceAll, None),
                    AssignMode::Replace { from } => (AssignModeParam::Replace, Some(Id::from_uuid(*from))),
                };
                let cmd = AssignPart {
                    devices: devices.iter().map(|d| Id::from_uuid(*d)).collect(),
                    part: Id::from_uuid(*copy),
                    copies: Vec::new(),
                    mode,
                    from,
                    qty: None,
                };
                AnyCommand::of(&cmd).map_err(ApiError::Cmd)
            }
            (PartTarget::Devices { devices, mode }, PartPick::Library { library, item }) => {
                self.assign_part(devices, &lib_ref(*library, *item), *mode, Dec3::ONE)
            }
            (PartTarget::BridgePart { .. } | PartTarget::AccessoryPart { .. }, _) => {
                let (copy, extra, docs) = match pick {
                    PartPick::Held(copy) => (*copy, Vec::new(), Vec::new()),
                    PartPick::Library { library, item } => {
                        let r = lib_ref(*library, *item);
                        let doc = self.libraries().resolve(&r).map_err(|error| ApiError::Library {
                            reference: format!("{library}/{item}@*"),
                            error,
                        })?;
                        let LibDocument::Part(c) = doc else {
                            return Err(ApiError::NotAPart(
                                v5::lib_copy_id::<()>(*library, *item).as_uuid(),
                            ));
                        };
                        let docs = self.closure_docs(std::slice::from_ref(&r))?;
                        let copy = c.id.as_uuid();
                        let extra = vec![[
                            copy.hyphenated().to_string(),
                            c.item.part_number.as_str().to_owned(),
                            self.library_manufacturer(&c.item.manufacturer),
                            kind_word(&c.item.kind),
                            text_of(&c.item.description),
                        ]];
                        (copy, extra, docs)
                    }
                };
                let part = format!("id:{copy}");
                let edit = match target {
                    PartTarget::BridgePart { terminals } => StripEdit::BridgePart {
                        terminals: terminals.clone(),
                        part,
                    },
                    PartTarget::AccessoryPart { strip, index } => StripEdit::AccessoryPart {
                        strip: *strip,
                        index: *index,
                        part,
                    },
                    PartTarget::Devices { .. } => unreachable!("matched above"),
                };
                let mut cmd = self
                    .strip_edits_with(std::slice::from_ref(&edit), extra)
                    .map_err(ApiError::Strip)?;
                if !docs.is_empty() {
                    cmd.params["copies"] = serde_json::Value::Array(
                        docs.iter()
                            .map(|d| serde_json::to_value(d).expect("a library document has a JSON form"))
                            .collect(),
                    );
                }
                Ok(cmd)
            }
        }
    }
}

/// The reference to a library item at any version.
pub(crate) fn lib_ref(library: Uuid, item: Uuid) -> LibRef {
    LibRef {
        library: Id::from_uuid(library),
        item: Id::from_uuid(item),
        version_req: "*".to_owned(),
        extra: ew_schema::Extra::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_round_trip_their_words() {
        for v in OfferView::ALL {
            assert_eq!(OfferView::from_word(v.word()), Some(v));
        }
        assert_eq!(OfferView::from_word("everything"), None);
    }

    #[test]
    fn a_category_prefix_matches_by_level() {
        let id = |s: &str| ew_mltext::Ident::from_input(s).unwrap();
        let cat = (id("ProtectionOvercurrent"), id("MCB"), id("1P+N"));
        assert!(category_matches(Some(&cat), &[]));
        assert!(category_matches(Some(&cat), &["ProtectionOvercurrent".into()]));
        assert!(category_matches(
            Some(&cat),
            &["ProtectionOvercurrent".into(), "MCB".into()]
        ));
        assert!(!category_matches(Some(&cat), &["Switching".into()]));
        assert!(!category_matches(None, &["ProtectionOvercurrent".into()]));
    }
}
