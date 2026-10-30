// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.19 part 6 (§11): the tables of the Macros and Libraries navigators and the macro detail.
//!
//! **Libraries** (FT-487): the installed libraries grouped by tier ("System", "Community", "Company", "User"), each
//! with its version, licence, access and index state; then the group "In this project" (not a tier) with the
//! project's library copies, each with its state ([`crate::CopyState`] in words) and the devices that use it. A copy
//! row carries its state as a badge code (`update_available`, `source_missing`, `deprecated`, `local_fork`; a current
//! copy has none), which the shell prints in words.
//!
//! **Macros** (FR-MAC-006): the macros of every installed library grouped by tier, then the macro copies the project
//! holds under "In this project"; a row's key names the macro (`macro:<library>:<item>`, `held:<copy>`), which
//! [`Session::macro_detail`] reads.
//!
//! Both read [`Session::libraries`] as it is now, never a command: the tables are front-end views (D7).

use std::collections::BTreeSet;

use ew_derive::Q;
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_mltext::MlText;
use ew_schema::{Device, Kind, LibCopy, LibDocument, LibRef, MacroDef, MacroKind, Representation, Sha256};

use super::{Builder, NavColumn, NavRole, column, short_tag};
use crate::library::{IndexState, LibraryInfo, Tier};
use crate::{CopyState, Session};

/// The Macros navigator's columns (key, label); the first is the key column.
pub(super) const MACRO_COLUMNS: [(&str, &str); 6] = [
    ("name", "Name"),
    ("kind", "Kind"),
    ("version", "Version"),
    ("library", "Library"),
    ("devices", "Devices"),
    ("variables", "Variables"),
];

/// The Libraries navigator's columns (key, label); the first is the key column. A library row fills Tier, Licence
/// and Access and puts its index state under State; a copy row fills State and Used by.
pub(super) const LIBRARY_COLUMNS: [(&str, &str); 8] = [
    ("name", "Name"),
    ("kind", "Kind"),
    ("tier", "Tier"),
    ("version", "Version"),
    ("licence", "Licence"),
    ("access", "Access"),
    ("state", "State"),
    ("used_by", "Used by"),
];

/// The most device tags a copy's Used by cell names before "+<n>".
const USED_BY_SHOWN: usize = 4;

/// The label of the group of the project's own copies.
const IN_PROJECT: &str = "In this project";

/// The columns of a list (key, label).
pub(super) fn columns(list: &[(&str, &str)]) -> Vec<NavColumn> {
    list.iter()
        .enumerate()
        .map(|(i, (k, l))| column(k, l, i == 0, None))
        .collect()
}

/// A tier's name in words.
pub(crate) fn tier_word(t: Tier) -> &'static str {
    match t {
        Tier::System => "System",
        Tier::Community => "Community",
        Tier::Company => "Company",
        Tier::User => "User",
    }
}

/// A copy state's code (the badge and the `state` facet; empty for a current copy) and its words (the State cell).
pub(crate) fn copy_state(s: &CopyState) -> (&'static str, String) {
    match s {
        CopyState::Current => ("", "Current".to_owned()),
        CopyState::UpdateAvailable { version, .. } => {
            ("update_available", format!("Update available ({version})"))
        }
        CopyState::Deprecated { .. } => ("deprecated", "Deprecated".to_owned()),
        CopyState::SourceMissing => ("source_missing", "Source missing".to_owned()),
        CopyState::LocalFork => ("local_fork", "Local fork".to_owned()),
    }
}

/// A library item kind in words.
fn kind_word(k: Kind) -> &'static str {
    match k {
        Kind::Part => "Part",
        Kind::Symbol => "Symbol",
        Kind::Fndef => "Function definition",
        Kind::Form => "Form",
        Kind::Macro => "Macro",
        Kind::Manufacturer => "Manufacturer",
        _ => "Item",
    }
}

/// A macro kind in words.
fn macro_kind_word(k: &MacroKind) -> &'static str {
    match k {
        MacroKind::Window => "Window",
        MacroKind::Page => "Page",
        MacroKind::Symbol => "Symbol",
        MacroKind::Unknown(_) => "Other",
    }
}

/// A text in the neutral language, else its first.
fn text(t: &MlText) -> String {
    super::super::properties::neutral_or_first(t)
}

/// The words of a library's index state.
fn index_word(s: &IndexState) -> String {
    match s {
        IndexState::Fresh => "Index up to date".to_owned(),
        IndexState::Rebuilt => "Index rebuilt".to_owned(),
        IndexState::Failed(m) => format!("Index failed: {m}"),
    }
}

/// The name a held copy is listed by: a part's number, a symbol's or manufacturer's name, else its key.
fn copy_label(q: &Q, kind: Kind, copy: Uuid) -> String {
    fn key<T>(c: &LibCopy<T>, meta: impl Fn(&T) -> &ew_schema::ItemMeta) -> Option<String> {
        meta(&c.item).key.as_ref().map(ToString::to_string)
    }
    let label = match kind {
        Kind::Part => q
            .parts()
            .get(Id::from_uuid(copy))
            .map(|c| c.item.part_number.to_string()),
        Kind::Symbol => q.symbols().get(Id::from_uuid(copy)).and_then(|c| {
            let name = text(&c.item.name);
            if name.is_empty() {
                key(c, |i| &i.meta)
            } else {
                Some(name)
            }
        }),
        Kind::Fndef => q.fndefs().get(Id::from_uuid(copy)).and_then(|c| {
            let (a, b, d) = &c.item.category;
            key(c, |i| &i.meta).or_else(|| Some(format!("{a}.{b}.{d}")))
        }),
        Kind::Form => q
            .forms()
            .get(Id::from_uuid(copy))
            .and_then(|c| key(c, |i| &i.meta)),
        Kind::Manufacturer => q
            .manufacturers()
            .get(Id::from_uuid(copy))
            .map(|c| text(&c.item.name)),
        _ => None,
    };
    label
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| kind_word(kind).to_owned())
}

/// The devices of a macro's first variant as the detail lists them: `-<letter><counter>` and the function text, in
/// fragment order.
fn macro_devices(m: &MacroDef) -> Vec<String> {
    let Some(variant) = m.variants.first() else {
        return Vec::new();
    };
    let records = variant.fragment.get("records").and_then(|r| r.as_array());
    records
        .into_iter()
        .flatten()
        .filter(|r| r.get("$k").and_then(|k| k.as_str()) == Some("device"))
        .filter_map(|r| serde_json::from_value::<Device>(r.clone()).ok())
        .map(|d| {
            let mut tag = format!("-{}", d.tag.letter);
            if let Some(c) = d.tag.counter {
                tag.push_str(&c.to_string());
            }
            let function = text(&d.function_text);
            if function.is_empty() {
                tag
            } else {
                format!("{tag} {function}")
            }
        })
        .collect()
}

/// A macro's variables as the detail lists them: `<name> (<type>)`, with ` = <default>` when it has one.
fn macro_variables(m: &MacroDef) -> Vec<String> {
    m.variables
        .iter()
        .map(|v| {
            let ty = serde_json::to_value(&v.ty)
                .ok()
                .and_then(|t| t.as_str().map(str::to_owned))
                .unwrap_or_default();
            match &v.default {
                Some(d) => format!("{} ({ty}) = {d}", v.name),
                None => format!("{} ({ty})", v.name),
            }
        })
        .collect()
}

/// The extent of a macro's first variant about its anchor, `[x0, y0, x1, y1]` in nm: its representations' positions,
/// padded by 5 mm on every side; `None` without a representation.
fn macro_extent(m: &MacroDef) -> Option<[i64; 4]> {
    const PAD: i64 = 5_000_000;
    let variant = m.variants.first()?;
    let anchor = variant
        .fragment
        .get("anchor")
        .and_then(|a| serde_json::from_value::<ew_units::Pt2>(a.clone()).ok())
        .unwrap_or_default();
    let mut out: Option<[i64; 4]> = None;
    for r in variant
        .fragment
        .get("records")
        .and_then(|r| r.as_array())
        .into_iter()
        .flatten()
    {
        if r.get("$k").and_then(|k| k.as_str()) != Some("representation") {
            continue;
        }
        let Ok(rep) = serde_json::from_value::<Representation>(r.clone()) else {
            continue;
        };
        let x = rep.pos.x.0.saturating_sub(anchor.x.0);
        let y = rep.pos.y.0.saturating_sub(anchor.y.0);
        out = Some(match out {
            None => [x, y, x, y],
            Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x), y1.max(y)],
        });
    }
    out.map(|[x0, y0, x1, y1]| {
        [
            x0.saturating_sub(PAD),
            y0.saturating_sub(PAD),
            x1.saturating_add(PAD),
            y1.saturating_add(PAD),
        ]
    })
}

/// What the Macros navigator's detail shows of a macro (WP-S1.19 §11, FR-MAC-006).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MacroDetail {
    /// The name.
    pub name: String,
    /// The description.
    pub description: String,
    /// The kind in words (`Window`, `Page`).
    pub kind: &'static str,
    /// The version (`1.2.0`).
    pub version: String,
    /// The library's name, or "In this project" for a copy only the project holds.
    pub library: String,
    /// The thumbnail asset, `None` without one (the detail then says "No preview").
    pub thumbnail: Option<Sha256>,
    /// The devices: `-<letter><counter>` and the function text.
    pub devices: Vec<String>,
    /// The variables: `<name> (<type>)`, with ` = <default>`.
    pub variables: Vec<String>,
    /// The value sets' names.
    pub value_sets: Vec<String>,
    /// The macro in its library (`version_req` `*`), `None` for a copy no installed library holds: only a macro of an
    /// installed library is inserted (`Session::insert_macro` resolves it).
    pub reference: Option<LibRef>,
    /// The copy ID the macro has in a project.
    pub copy: Uuid,
    /// The ghost of the first variant about its anchor, `[x0, y0, x1, y1]` in nm ([`MacroDetail::extent`]'s rule:
    /// the representations' positions padded by 5 mm); `None` without a representation.
    pub extent: Option<[i64; 4]>,
}

impl MacroDetail {
    fn of(c: &LibCopy<MacroDef>, library: String, reference: Option<LibRef>) -> MacroDetail {
        MacroDetail {
            name: text(&c.item.name),
            description: text(&c.item.description),
            kind: macro_kind_word(&c.item.kind),
            version: c.version.clone(),
            library,
            thumbnail: c.item.thumbnail,
            devices: macro_devices(&c.item),
            variables: macro_variables(&c.item),
            value_sets: c.item.value_sets.iter().map(|v| v.name.to_string()).collect(),
            reference,
            copy: c.id.as_uuid(),
            extent: macro_extent(&c.item),
        }
    }
}

/// The installed libraries of a tier, in the registry's order.
fn of_tier(libs: &[LibraryInfo], tier: Tier) -> impl Iterator<Item = &LibraryInfo> {
    libs.iter().filter(move |l| l.tier == tier)
}

const TIERS: [Tier; 4] = [Tier::System, Tier::Community, Tier::Company, Tier::User];

impl Session {
    /// The macro a row key of the Macros navigator names (its last segment `macro:<library>:<item>` or
    /// `held:<copy>`): the installed library's document, or the project's copy; `None` for another key or a macro
    /// that is gone.
    pub fn macro_detail(&self, key: &str) -> Option<MacroDetail> {
        let last = key.rsplit('/').next()?;
        if let Some(rest) = last.strip_prefix("macro:") {
            let (library, item) = rest.split_once(':')?;
            let (library, item): (Uuid, Uuid) = (library.parse().ok()?, item.parse().ok()?);
            let info = self
                .libraries()
                .libraries()
                .into_iter()
                .find(|l| l.id == library)?;
            let doc = self
                .libraries()
                .documents(library, Kind::Macro)
                .into_iter()
                .find(|d| matches!(d, LibDocument::Macro(c) if c.item_id.as_uuid() == item))?;
            let LibDocument::Macro(c) = doc else { return None };
            let reference = LibRef {
                library: Id::from_uuid(library),
                item: Id::from_uuid(item),
                version_req: "*".to_owned(),
                extra: ew_schema::Extra::new(),
            };
            return Some(MacroDetail::of(&c, info.name, Some(reference)));
        }
        let copy: Uuid = last.strip_prefix("held:")?.parse().ok()?;
        let q = self.query();
        let c = q.table::<LibCopy<MacroDef>>().get(Id::from_uuid(copy))?.clone();
        Some(MacroDetail::of(&c, IN_PROJECT.to_owned(), None))
    }

    /// A new macro instance ID for an insertion of the macro copy `copy` (WP-S1.19 §12 item 2: the shell mints it and
    /// the command log records it): `v5(copy, "instance" ‖ n)` for the first `n` from the number of instances the
    /// project holds whose ID it does not hold. Equal projects give equal IDs, so a script inserts the same records.
    pub fn next_macro_instance(&self, copy: Uuid) -> Uuid {
        let q = self.query();
        let table = q.table::<ew_schema::MacroInstance>();
        let held: BTreeSet<Uuid> = table.iter().map(|(id, _)| id.as_uuid()).collect();
        (held.len()..)
            .map(|n| Uuid::new_v5(&copy, format!("instance\u{1f}{n}").as_bytes()))
            .find(|id| !held.contains(id))
            .unwrap_or_else(|| Uuid::new_v5(&copy, b"instance"))
    }

    /// Macros (WP-S1.19 §11): tier → macro of every installed library, then "In this project" → the macro copies the
    /// project holds that no listed row names.
    pub(super) fn macros_table(&self, q: &Q, b: &mut Builder) {
        let libs = self.libraries().libraries();
        let mut listed: BTreeSet<Uuid> = BTreeSet::new();
        for tier in TIERS {
            let mut rows: Vec<(LibraryInfo, LibCopy<MacroDef>)> = Vec::new();
            for lib in of_tier(&libs, tier) {
                for doc in self.libraries().documents(lib.id, Kind::Macro) {
                    if let LibDocument::Macro(c) = doc {
                        rows.push((lib.clone(), c));
                    }
                }
            }
            if rows.is_empty() {
                continue;
            }
            let word = tier_word(tier);
            let tnode = tier_group(b, tier);
            for (lib, c) in rows {
                listed.insert(c.id.as_uuid());
                let segment = format!("macro:{}:{}", lib.id, c.item_id.as_uuid());
                macro_row(b, tnode, &segment, &c, &lib.name, Some(word));
            }
        }
        let held: Vec<LibCopy<MacroDef>> = q
            .table::<LibCopy<MacroDef>>()
            .iter()
            .filter(|(id, _)| !listed.contains(&id.as_uuid()))
            .map(|(_, c)| (**c).clone())
            .collect();
        if held.is_empty() {
            return;
        }
        let gnode = project_group(b);
        for c in held {
            let segment = format!("held:{}", c.id.as_uuid());
            macro_row(b, gnode, &segment, &c, IN_PROJECT, None);
        }
    }

    /// Libraries (WP-S1.19 §11, FT-487): tier → library, then "In this project" → the project's library copies with
    /// their state ([`Session::library_status`]) and the devices that use them.
    pub(super) fn libraries_table(&self, q: &Q, b: &mut Builder) {
        let libs = self.libraries().libraries();
        for tier in TIERS {
            if of_tier(&libs, tier).next().is_none() {
                continue;
            }
            let tnode = tier_group(b, tier);
            for lib in of_tier(&libs, tier) {
                let access = if lib.writable { "Writable" } else { "Read-only" };
                let mut badges = Vec::new();
                if matches!(lib.index, IndexState::Failed(_)) {
                    badges.push("index_failed".to_owned());
                }
                let cells = vec![
                    lib.name.clone(),
                    "Library".to_owned(),
                    tier_word(tier).to_owned(),
                    lib.version.clone(),
                    lib.licence.clone(),
                    access.to_owned(),
                    index_word(&lib.index),
                    String::new(),
                ];
                let mut facets = vec![
                    ("kind".to_owned(), NavRole::Library.word().to_owned()),
                    ("tier".to_owned(), tier.name().to_owned()),
                ];
                if lib.writable {
                    facets.push(("writable".to_owned(), String::new()));
                }
                b.push(
                    Some(tnode),
                    NavRole::Library,
                    &format!("library:{}", lib.id),
                    lib.name.clone(),
                    badges,
                    Vec::new(),
                    Vec::new(),
                    Some(cells),
                    facets,
                );
            }
        }
        let status = self.library_status();
        if status.is_empty() {
            return;
        }
        let gnode = project_group(b);
        for s in status {
            let (code, words) = copy_state(&s.state);
            let label = copy_label(q, s.kind, s.copy);
            let mut tags: Vec<String> = s.devices.iter().map(|d| short_tag(q, *d)).collect();
            tags.retain(|t| !t.is_empty());
            let used_by = if tags.len() > USED_BY_SHOWN {
                format!(
                    "{} +{}",
                    tags[..USED_BY_SHOWN].join(", "),
                    tags.len() - USED_BY_SHOWN
                )
            } else {
                tags.join(", ")
            };
            let cells = vec![
                label.clone(),
                kind_word(s.kind).to_owned(),
                String::new(),
                s.version.clone(),
                String::new(),
                String::new(),
                words,
                used_by,
            ];
            let mut facets = vec![
                ("kind".to_owned(), NavRole::LibCopy.word().to_owned()),
                ("kind".to_owned(), s.kind.name().to_owned()),
            ];
            let badges = if code.is_empty() {
                facets.push(("state".to_owned(), "current".to_owned()));
                Vec::new()
            } else {
                facets.push(("state".to_owned(), code.to_owned()));
                vec![code.to_owned()]
            };
            let mut records = vec![s.copy];
            records.extend(s.devices.iter().map(|d| d.as_uuid()));
            b.push(
                Some(gnode),
                NavRole::LibCopy,
                &format!("copy:{}", s.copy),
                label,
                badges,
                records,
                Vec::new(),
                Some(cells),
                facets,
            );
        }
    }
}

/// A tier's group node.
fn tier_group(b: &mut Builder, tier: Tier) -> u32 {
    b.push(
        None,
        NavRole::Tier,
        &format!("tier:{}", tier.name()),
        tier_word(tier).to_owned(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
        vec![
            ("kind".to_owned(), NavRole::Tier.word().to_owned()),
            ("tier".to_owned(), tier.name().to_owned()),
        ],
    )
}

/// The group node of the project's own copies.
fn project_group(b: &mut Builder) -> u32 {
    b.push(
        None,
        NavRole::ProjectCopies,
        "project",
        IN_PROJECT.to_owned(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
        vec![("kind".to_owned(), NavRole::ProjectCopies.word().to_owned())],
    )
}

/// One macro row: its name, kind, version, library, device and variable counts; the facets `kind:macro`,
/// `kind:<window|page|symbol>` and `tier:<name>`.
fn macro_row(
    b: &mut Builder,
    parent: u32,
    segment: &str,
    c: &LibCopy<MacroDef>,
    library: &str,
    tier: Option<&str>,
) {
    let name = text(&c.item.name);
    let kind = macro_kind_word(&c.item.kind);
    let cells = vec![
        name.clone(),
        kind.to_owned(),
        c.version.clone(),
        library.to_owned(),
        macro_devices(&c.item).len().to_string(),
        c.item.variables.len().to_string(),
    ];
    let mut facets = vec![
        ("kind".to_owned(), NavRole::Macro.word().to_owned()),
        ("kind".to_owned(), kind.to_lowercase()),
    ];
    if let Some(t) = tier {
        facets.push(("tier".to_owned(), t.to_lowercase()));
    }
    b.push(
        Some(parent),
        NavRole::Macro,
        segment,
        name,
        Vec::new(),
        vec![c.id.as_uuid()],
        Vec::new(),
        Some(cells),
        facets,
    );
}
