// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A copy of pages in the session (doc 06 §3.6, "A copy of pages"): [`Session::copy_pages`] composes `page.copy` from
//! the pages and the rule. Its fragment is the page macro's of the pages (`Session::macro_fragment`), widened by what a
//! second installation keeps together that a clipboard copy leaves out: the cables with their conductors and cores, the
//! connections no page draws (a strip's bridges), the bonds between copied ends and the report blocks embedded on the
//! pages. It refuses a copy whose interruption points would join the points they were copied from, and says what the
//! copy leaves on the pages that are not copied.

use std::collections::BTreeSet;

use ew_commands::commands::{
    CopyPages, Fragment, NodeSwap, Rename, TextNames, designation_moves, renamed, renamed_mltext, text_names,
};
use ew_derive::{IpPairingQ, Q, short_tag};
use ew_ids::{Id, Uuid};
use ew_schema::{
    Bond, Conductor, Connection, ConnectionKind, Device, Endpoint, Function, IpDirection, Kind, Page, Record,
    Representation, StructureNode,
};
use serde_json::Value;

use crate::{AnyCommand, CmdError, MacroError, MacroScope, Session};

/// Where the copies go.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CopyFolios {
    /// Each folio number plus this number.
    Offset(u32),
    /// After this page, with proposed folios.
    After(Uuid),
}

/// What [`Session::copy_pages`] copies and by which rule.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PageCopySpec {
    /// The pages, in page order.
    pub pages: Vec<Uuid>,
    /// The folios of the copies.
    pub folios: CopyFolios,
    /// Each structure node of the copied pages that the copy replaces, with the code of the copy's node.
    pub structure: Vec<(Uuid, String)>,
    /// The old and new texts of the pages' titles and descriptions, frozen tags and interruption-point names.
    pub rename: Vec<(String, String)>,
}

/// A composed copy of pages.
#[derive(Clone, PartialEq, Debug)]
pub struct PageCopyPlan {
    /// The `page.copy` command.
    pub command: AnyCommand,
    /// The devices, strips and cables also drawn on pages that are not copied, by their tags in natural order: their
    /// copies draw only what stands on the copied pages.
    pub drawn_elsewhere: Vec<String>,
    /// The frozen tags the rename leaves as they were where the copy's full designation would be its original's: the
    /// copy clears them and takes the scheme's tags.
    pub cleared_frozen: Vec<String>,
    /// The interruption-point names whose copies continue points outside the copy (a shared supply, an incoming
    /// supply of targets only).
    pub continued: Vec<String>,
    /// How many copied free texts take the copy's tag or the new name of what they name (`text_names`).
    pub texts_named: usize,
    /// The copied free texts left as they are because copied devices shared the tag they hold.
    pub texts_left: Vec<String>,
    /// The page titles the rename changes, each old and new, in page order.
    pub titles: Vec<(String, String)>,
    /// The bus devices of the copy (a valve terminal's bus node, a PLC station), by their tags in natural order: their
    /// copies join no bus and take no address of their own until a person gives them one.
    pub bus_devices: Vec<String>,
}

/// The refusal of `page.copy` at `at`.
fn refused(at: &str, text: crate::Text) -> CmdError {
    CmdError::invalid_params(<CopyPages as ew_commands::Command>::NAME, at, text)
}

/// The ID of a record value.
fn id_of(v: &Value) -> Option<Uuid> {
    v.get("id")?.as_str()?.parse().ok()
}

/// The IDs of a fragment's records of kind `kind`.
fn ids_of(fragment: &Fragment, kind: Kind) -> BTreeSet<Uuid> {
    fragment
        .records
        .iter()
        .filter(|r| r.get("$k").and_then(Value::as_str) == Some(kind.name()))
        .filter_map(id_of)
        .collect()
}

/// A record as a fragment line's value, without its import lineage.
fn line(r: Record) -> Value {
    let mut v = serde_json::to_value(r).expect("a record has a JSON form");
    if let Some(o) = v.as_object_mut() {
        o.remove("source_ref");
    }
    v
}

/// Natural order, then the text.
fn sorted(set: BTreeSet<String>) -> Vec<String> {
    let mut v: Vec<String> = set.into_iter().collect();
    v.sort_by(|a, b| ew_mltext::natural_cmp(a, b).then_with(|| a.cmp(b)));
    v
}

impl Session {
    /// `page.copy` of the pages `spec.pages` by the rule of `spec` (doc 06 §3.6, "A copy of pages"), with what the
    /// card says before it is applied. The fragment is the page macro's of the pages, widened by the cables (every
    /// conductor; the cores and bus lines on the pages keep their cable), the connections no page draws between copied
    /// ends, the bonds whose two ends are copied and the report blocks embedded on the pages.
    ///
    /// # Errors
    ///
    /// The refusal of `page.copy`: no page, one named twice or not a drawn page, pages of different grids; a code that
    /// is no name; interruption points that would join the points they were copied from (a name whose points all lie
    /// on the pages, or whose copied points include a source, that neither the rename nor the structure rule gives a
    /// pairing of its own; a name with no source in the project whose copied points are targets continues).
    pub fn copy_pages(&self, spec: &PageCopySpec) -> Result<PageCopyPlan, CmdError> {
        let q = self.query();
        if spec.pages.is_empty() {
            return Err(refused(
                "/fragment/pages",
                ew_commands::text!("refusal-page-copy-no-page"),
            ));
        }
        let mut pages: Vec<Id<Page>> = Vec::new();
        for (i, p) in spec.pages.iter().enumerate() {
            let at = format!("/fragment/pages/{i}");
            let Some(page) = q.pages().get(Id::from_uuid(*p)) else {
                return Err(refused(&at, ew_commands::text!("refusal-params-page-gone")));
            };
            if pages.contains(&page.id) {
                return Err(refused(&at, text!("refusal-page-copy-page-twice")));
            }
            if pages
                .first()
                .and_then(|f| q.pages().get(*f))
                .is_some_and(|f| f.grid != page.grid)
            {
                return Err(refused(&at, text!("refusal-page-copy-grids")));
            }
            pages.push(page.id);
        }
        let structure = spec
            .structure
            .iter()
            .enumerate()
            .map(|(i, (node, code))| {
                Ok(NodeSwap {
                    from: Id::from_uuid(*node),
                    to: code.trim().parse().map_err(|_| {
                        refused(
                            &format!("/structure/{i}/to"),
                            text!("refusal-page-copy-code", code.as_str()),
                        )
                    })?,
                })
            })
            .collect::<Result<Vec<_>, CmdError>>()?;
        let rename: Vec<Rename> = spec
            .rename
            .iter()
            .map(|(from, to)| Rename {
                from: from.clone(),
                to: to.clone(),
            })
            .collect();
        let (mut fragment, _) = self
            .macro_fragment(&MacroScope::Pages(spec.pages.clone()))
            .map_err(|e| match e {
                MacroError::Copy(_) => {
                    refused("/fragment/pages", ew_commands::text!("refusal-page-copy-no-page"))
                }
                other => CmdError::internal(other.to_string()),
            })?;
        widen(&q, &pages, &mut fragment);
        let (continued, joins) = ip_groups(&q, &fragment, &rename, &structure);
        if let Some(first) = joins.first() {
            return Err(refused(
                "/rename",
                text!("refusal-page-copy-ip-join", joins.join(", "), first.as_str()),
            ));
        }
        let swapped: BTreeSet<Id<StructureNode>> = structure.iter().map(|s| s.from).collect();
        let (drawn_elsewhere, cleared_frozen) = left_out(&q, &pages, &fragment, &rename, &swapped);
        // The texts that name what is copied: those naming one device or a renamed point change (a frozen tag kept
        // as it is prints as before); those naming devices that shared a tag are left.
        let kept_frozen = |d: &Uuid| {
            q.devices()
                .get(Id::from_uuid(*d))
                .and_then(|dev| dev.tag.frozen.as_ref().map(|f| f.text.as_str().to_owned()))
                .is_some_and(|f| {
                    renamed(&f, &rename) == f && designation_moves(&q, Id::from_uuid(*d), &swapped)
                })
        };
        let mut texts_named = 0;
        let mut texts_left = BTreeSet::new();
        for (item, names) in text_names(&q, &fragment, &rename) {
            match names {
                TextNames::Point(_) => texts_named += 1,
                TextNames::Devices(devices, _) if devices.len() == 1 => {
                    texts_named += usize::from(!kept_frozen(&devices[0]));
                }
                TextNames::Devices(..) => {
                    texts_left.extend(
                        q.page_items()
                            .get(Id::from_uuid(item))
                            .and_then(|i| match &i.kind {
                                ew_schema::PageItemKind::FreeText(t) => {
                                    t.text.as_neutral().map(str::to_owned)
                                }
                                _ => None,
                            }),
                    );
                }
            }
        }
        // The titles the rename changes, in the text the page shows first (its neutral text, else its first language).
        let shown = |t: &ew_mltext::MlText| t.neutral_or_first().unwrap_or_default().to_owned();
        let titles = pages
            .iter()
            .filter_map(|p| q.pages().get(*p))
            .map(|p| (shown(&p.title), shown(&renamed_mltext(&p.title, &rename))))
            .filter(|(old, new)| old != new)
            .collect();
        // The devices whose functions carry a bus port: the copy needs its bus line and its address (a persona run's
        // copied valve terminal joined the PLC and nothing said what it still lacked).
        let bus_devices = sorted(
            fragment
                .records
                .iter()
                .filter(|r| r.get("$k").and_then(Value::as_str) == Some(Kind::Function.name()))
                .filter(|r| r.get("bus_port").is_some_and(|p| !p.is_null()))
                .filter_map(|r| r.get("device")?.as_str()?.parse::<Uuid>().ok())
                .map(|d| short_tag(&q, Id::from_uuid(d)))
                .filter(|t| !t.is_empty())
                .collect(),
        );
        let (offset, after) = match spec.folios {
            CopyFolios::Offset(n) => (Some(n), None),
            CopyFolios::After(p) => (None, Some(Id::from_uuid(p))),
        };
        let command = AnyCommand::of(&CopyPages {
            fragment,
            offset,
            after,
            structure,
            rename,
        })?;
        Ok(PageCopyPlan {
            command,
            drawn_elsewhere,
            cleared_frozen,
            continued,
            texts_named,
            texts_left: sorted(texts_left),
            titles,
            bus_devices,
        })
    }
}

/// Adds to the page macro's fragment of `pages` what a copy of pages keeps together and a clipboard copy leaves out.
fn widen(q: &Q, pages: &[Id<Page>], fragment: &mut Fragment) {
    let on_pages: BTreeSet<Id<Page>> = pages.iter().copied().collect();
    let mut functions = ids_of(fragment, Kind::Function);
    let junctions = ids_of(fragment, Kind::Junction);
    let mut connections = ids_of(fragment, Kind::Connection);
    // The cables of the cores and bus lines on the pages, and of the cable symbols the copy left out.
    let cable_of = |c: &Connection| match &c.kind {
        ConnectionKind::Core(k) => q.table::<Conductor>().get(*k).map(|k| k.cable),
        ConnectionKind::Bus(line) => line.cable,
        _ => None,
    };
    let mut cables: BTreeSet<Id<Device>> = connections
        .iter()
        .filter_map(|c| q.connections().get(Id::from_uuid(*c)))
        .filter_map(|c| cable_of(c))
        .collect();
    for p in pages {
        for r in q.idx().reps_by_page(*p) {
            let Some(rep) = q.reps().get(r) else { continue };
            cables.extend(
                rep.functions
                    .iter()
                    .filter_map(|f| q.functions().get(*f)?.device)
                    .filter(|d| q.devices().get(*d).is_some_and(|d| d.aspects.cable.is_some())),
            );
        }
    }
    let devices = ids_of(fragment, Kind::Device);
    let mut added: Vec<Record> = Vec::new();
    for cable in &cables {
        let Some(dev) = q
            .devices()
            .get(*cable)
            .filter(|_| !devices.contains(&cable.as_uuid()))
        else {
            continue;
        };
        added.push(Record::Device((**dev).clone()));
        for f in q.idx().functions_of_device(*cable) {
            if let Some(func) = q.functions().get(f)
                && functions.insert(f.as_uuid())
            {
                added.push(Record::Function((**func).clone()));
            }
            for r in q.idx().reps_of_function(f) {
                if let Some(rep) = q.reps().get(r).filter(|r| on_pages.contains(&r.page)) {
                    added.push(Record::Representation((**rep).clone()));
                }
            }
        }
        for k in q.idx().conductors_of_cable(*cable) {
            if let Some(k) = q.table::<Conductor>().get(k) {
                added.push(Record::Conductor((**k).clone()));
            }
        }
    }
    let inside = |e: &Endpoint| match e {
        Endpoint::Cp(f, _) => functions.contains(&f.as_uuid()),
        Endpoint::Junction(j) => junctions.contains(&j.as_uuid()),
        Endpoint::Unknown(_) => false,
    };
    // The connections no page draws (a strip's bridges), between copied ends.
    for (id, c) in q.connections().iter() {
        if c.page.is_none() && inside(&c.a) && inside(&c.b) && connections.insert(id.as_uuid()) {
            added.push(Record::Connection((**c).clone()));
        }
    }
    // The bonds whose two ends are copied; their wire when it is copied too.
    for (_, b) in q.table::<Bond>().iter() {
        if inside(&b.a) && inside(&b.b) {
            let mut b: Bond = (**b).clone();
            b.via = b.via.filter(|c| connections.contains(&c.as_uuid()));
            added.push(Record::Bond(b));
        }
    }
    // The report blocks embedded on the pages.
    for (_, block) in q.report_blocks().iter() {
        if block
            .embedded
            .as_ref()
            .is_some_and(|e| on_pages.contains(&e.page))
        {
            added.push(Record::ReportBlock((**block).clone()));
        }
    }
    fragment.records.extend(added.into_iter().map(line));
    // The cores and bus lines of the copied cables are theirs again (a clipboard copy makes a core a plain wire).
    for v in &mut fragment.records {
        if v.get("$k").and_then(Value::as_str) != Some(Kind::Connection.name()) {
            continue;
        }
        let Some(c) = id_of(v).and_then(|id| q.connections().get(Id::from_uuid(id))) else {
            continue;
        };
        if cable_of(c).is_some_and(|k| cables.contains(&k))
            && let Ok(kind) = serde_json::to_value(&c.kind)
        {
            v["kind"] = kind;
        }
    }
}

/// The interruption-point names of the copy, by the project's pairing groups that have a point on the copied pages:
/// those whose copies continue points outside the copy, and those whose copies would join the points they were copied
/// from (a copied point is a source, or the group lies wholly on the pages, and neither the rename nor a swapped
/// structure node gives the copies a group of their own); each in natural order. A name with no source in the project
/// whose copied points are all targets continues whether or not the copy takes all its points: a supply that comes in
/// from outside the drawing feeds the copy too.
fn ip_groups(
    q: &Q,
    fragment: &Fragment,
    rename: &[Rename],
    structure: &[NodeSwap],
) -> (Vec<String>, Vec<String>) {
    let pairing = q.get::<IpPairingQ>(());
    let copied = ids_of(fragment, Kind::Function);
    let swapped: BTreeSet<Id<StructureNode>> = structure.iter().map(|s| s.from).collect();
    let mut continued = BTreeSet::new();
    let mut joins = BTreeSet::new();
    for g in &pairing.groups {
        let mine: Vec<Id<Function>> = g
            .members
            .iter()
            .copied()
            .filter(|f| copied.contains(&f.as_uuid()))
            .collect();
        if mine.is_empty() {
            continue;
        }
        let separated = renamed(&g.name, rename) != g.name || g.scope.is_some_and(|s| swapped.contains(&s));
        if separated {
            continue;
        }
        let is_source = |f: &Id<Function>| {
            pairing
                .ips
                .get(f)
                .is_some_and(|i| i.star_source || i.direction == IpDirection::Source)
        };
        let incoming = !g.members.iter().any(is_source)
            && mine.iter().all(|f| {
                pairing
                    .ips
                    .get(f)
                    .is_some_and(|i| i.direction == IpDirection::Target)
            });
        if !incoming && (mine.iter().any(is_source) || mine.len() == g.members.len()) {
            joins.insert(g.name.clone());
        } else {
            continued.insert(g.name.clone());
        }
    }
    (sorted(continued), sorted(joins))
}

/// What the copy leaves out, for the card: the devices drawn also on other pages (a function, a strip's terminal, a
/// cable's core), and the frozen tags the copy clears: the rename does not change them and no swapped node in
/// `swapped` gives the copy another full designation.
fn left_out(
    q: &Q,
    pages: &[Id<Page>],
    fragment: &Fragment,
    rename: &[Rename],
    swapped: &BTreeSet<Id<StructureNode>>,
) -> (Vec<String>, Vec<String>) {
    let on_pages: BTreeSet<Id<Page>> = pages.iter().copied().collect();
    let devices = ids_of(fragment, Kind::Device);
    let connections = ids_of(fragment, Kind::Connection);
    let elsewhere = |r: Id<Representation>| q.reps().get(r).is_some_and(|r| !on_pages.contains(&r.page));
    let mut drawn = BTreeSet::new();
    let mut cleared = BTreeSet::new();
    for d in &devices {
        let d: Id<Device> = Id::from_uuid(*d);
        let Some(dev) = q.devices().get(d) else { continue };
        let functions = q
            .idx()
            .functions_of_device(d)
            .any(|f| q.idx().reps_of_function(f).any(&elsewhere));
        let terminals = q
            .idx()
            .terminals_of_strip(d)
            .any(|t| !devices.contains(&t.as_uuid()));
        let cores = dev.aspects.cable.is_some()
            && q.connections().iter().any(|(id, c)| {
                !connections.contains(&id.as_uuid())
                    && matches!(&c.kind, ConnectionKind::Core(k)
                        if q.table::<Conductor>().get(*k).is_some_and(|k| k.cable == d))
            });
        if functions || terminals || cores {
            drawn.insert(short_tag(q, d));
        }
        if let Some(frozen) = &dev.tag.frozen
            && renamed(frozen.text.as_str(), rename) == frozen.text.as_str()
            && !designation_moves(q, d, swapped)
        {
            cleared.insert(frozen.text.as_str().to_owned());
        }
    }
    drawn.remove("");
    (sorted(drawn), sorted(cleared))
}
