// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The property sheet's header (§6.2): what the selection is, its title and count, and a record's short title.
//!
//! Not here: the sections below the header (`sections.rs`); the selection's live description (`selection.rs`).

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::{MainRep, Q};
use ew_schema::{Device, Kind};
use serde_json::Value;

use super::context::SheetCx;
use super::selection::{connection_filter, wire_number_text};
use super::{
    FindingChip, Picked, SheetHeader, SheetSection, SheetWhat, device_tag_text, folio_of, kind_plural,
    lookup, main_location, ml_text, page_item_kind_words,
};
use crate::{Session, checks};

/// A short title of the record `id`: a device's tag, a page's folio and title, a node's path, a name.
pub(in crate::output) fn record_title(q: &Q, id: Uuid, source: Option<&str>) -> Option<String> {
    let (kind, json) = lookup(q, id)?;
    Some(match kind {
        Kind::Device => device_tag_text(q, Id::from_uuid(id)),
        Kind::Page => {
            let folio = folio_of(q, Id::from_uuid(id)).unwrap_or_default();
            let title = json.get("title").map(|t| ml_text(t, source)).unwrap_or_default();
            format!("{folio} {title}").trim().to_owned()
        }
        Kind::StructureNode => SheetCx::new(q).node_path(Id::from_uuid(id)).unwrap_or_default(),
        // A core by its cable's tag and its identification (`-1W1 BN`).
        Kind::Conductor => {
            let cable = json
                .get("cable")
                .and_then(Value::as_str)
                .and_then(|c| Uuid::parse_str(c).ok());
            let core = json.get("core").and_then(Value::as_str).unwrap_or_default();
            match cable {
                Some(c) => format!("{} {core}", device_tag_text(q, Id::from_uuid(c))),
                None => core.to_owned(),
            }
        }
        // A fluid line has no wire number; it is named by its line kind ("drain line").
        Kind::Connection => match json.pointer("/kind/fluid") {
            // A bus line is named by its cable (rc.5: it read `?`).
            None if json.pointer("/kind/bus").is_some() => json
                .pointer("/kind/bus/cable")
                .and_then(Value::as_str)
                .and_then(|c| Uuid::parse_str(c).ok())
                .map_or_else(|| "bus line".to_owned(), |c| device_tag_text(q, Id::from_uuid(c))),
            Some(fluid) => format!(
                "{} line",
                fluid
                    .get("line_kind")
                    .and_then(Value::as_str)
                    .unwrap_or("working")
            ),
            None => wire_number_text(q, Id::from_uuid(id)),
        },
        // A layout record by its device's tag, its label or its words.
        k if crate::output::layout_edit::LAYOUT_KINDS.contains(&k) => {
            crate::output::layout_edit::layout_title(q, k, id)?
        }
        // An external reference has no name; it reads as the IP tool prints it, `<device>:<connection>`
        // (the device alone without a connection).
        Kind::ExternalRef => {
            let part = |member: &str| json.get(member).and_then(Value::as_str).filter(|s| !s.is_empty());
            match (part("device"), part("connection")) {
                (Some(device), Some(connection)) => format!("{device}:{connection}"),
                (Some(device), None) => device.to_owned(),
                _ => kind.name().to_owned(),
            }
        }
        // G-PP triage row 12: a page item is named by its kind in words (`wire marker`, `free text`), not `page_item`.
        Kind::PageItem => json
            .get("kind")
            .and_then(page_item_kind_words)
            .unwrap_or_else(|| kind.name().to_owned()),
        _ => json
            .get("name")
            .or_else(|| json.get("label"))
            .or_else(|| json.get("code"))
            .or_else(|| json.get("title"))
            .map(|v| ml_text(v, source))
            .unwrap_or_else(|| kind.name().to_owned()),
    })
}

/// What `device` is, for the header ([`SheetWhat`]).
fn sheet_what(q: &Q, device: Id<Device>) -> SheetWhat {
    let symbol = (*q.get::<MainRep>(device))
        .and_then(|r| q.reps().get(r))
        .and_then(|r| q.symbols().get(r.symbol))
        .map(|c| c.item.name.clone())
        .filter(|n| !n.is_empty());
    SheetWhat {
        natures: crate::output::natures::device_natures(q, device),
        symbol,
        solenoids: crate::output::rack::solenoids(q, device).len(),
        terminal: crate::output::rack::terminal_kind(q, device),
        mark: false,
    }
}

/// The cable whose mark the page item `item` is (`ew_derive::cable_mark`), if it is one.
pub(super) fn marked_cable(q: &Q, item: Id<ew_schema::PageItem>) -> Option<Id<Device>> {
    let mark = ew_derive::cable_mark(q, q.page_items().get(item)?)?;
    let (first, _) = mark.cores.first()?;
    match &q.connections().get(*first)?.kind {
        ew_schema::ConnectionKind::Core(core) => q.conductors().get(*core).map(|c| c.cable),
        ew_schema::ConnectionKind::Bus(line) => line.cable,
        _ => None,
    }
}

impl Session {
    /// The header (§6.2).
    pub(super) fn header(
        &self,
        q: &Q,
        cx: &SheetCx<'_>,
        picked: &[Picked],
        sections: &[SheetSection],
    ) -> SheetHeader {
        let source = cx.source.as_deref();
        let mut kinds: BTreeMap<Kind, usize> = BTreeMap::new();
        for p in picked {
            *kinds.entry(p.kind).or_default() += 1;
        }
        let items = picked.len();
        // The device of a single device, function or representation, or the cable of a cable's mark.
        let device: Option<Id<Device>> = match picked {
            [one]
                if matches!(
                    one.kind,
                    Kind::Device | Kind::Function | Kind::Representation | Kind::PageItem
                ) =>
            {
                one.reach
                    .get(&Kind::Device)
                    .filter(|d| d.len() == 1)
                    .and_then(|d| d.iter().next())
                    .map(|u| Id::from_uuid(*u))
            }
            _ => None,
        };
        let title = match (picked, device) {
            ([_], Some(d)) => device_tag_text(q, d),
            // P-1: a generated page is titled by its own folio and title, not by its block's title format.
            ([one], None) if one.page_title.is_some() => one.page_title.clone().unwrap_or_default(),
            ([one], None) => {
                let id = one.reach.get(&one.kind).and_then(|s| s.iter().next()).copied();
                // A representation without a device (an interruption point's) is named by
                // its function's kind in the navigators' words ("Interruption point"), not by the raw kind word.
                let words = match one.reach.get(&Kind::Function) {
                    Some(fs) if one.kind == Kind::Representation && fs.len() == 1 => {
                        fs.iter().next().and_then(|f| {
                            q.functions()
                                .get(Id::from_uuid(*f))
                                .and_then(|f| crate::output::navigators::function_kind_words(q, f))
                        })
                    }
                    _ => None,
                };
                words
                    .map(str::to_owned)
                    .or_else(|| id.and_then(|id| record_title(q, id, source)))
                    .unwrap_or_else(|| one.kind.name().to_owned())
            }
            _ => match sections.first() {
                Some(s) => format!("{} {}", s.targets.len(), kind_plural(&s.kind)),
                None => format!("{items} items"),
            },
        };
        let subtitle = device.and_then(|d| {
            q.devices().get(d).map(|dev| {
                ml_text(
                    &serde_json::to_value(&dev.function_text).unwrap_or(Value::Null),
                    source,
                )
            })
        });
        let subtitle = subtitle.filter(|s| !s.is_empty());
        let location = device.and_then(|d| main_location(q, d));
        // The open findings of the online scheme (the project's `active_scheme`, else `drafting`) on the
        // sheet's records or pages.
        let mut targets: BTreeSet<Uuid> = BTreeSet::new();
        let mut pages: BTreeSet<Uuid> = BTreeSet::new();
        for s in sections {
            targets.extend(s.targets.iter().copied());
            if s.kind == "page" {
                pages.extend(s.targets.iter().copied());
            }
        }
        let mut findings: Vec<(String, String, FindingChip)> = Vec::new();
        let folios = crate::output::navigators::folios(q);
        {
            // With merge evidence, EW-CORE-013's groups.
            let report = self.check_over(q, self.online_scheme(), checks::CheckMode::Online);
            for f in report
                .findings
                .iter()
                .filter(|f| f.state == checks::FindingState::Open)
            {
                let on_target = f.targets.iter().any(|t| targets.contains(&t.id.as_uuid()));
                // A finding is on a page where Messages and the status bar count it: its own page, else the page of
                // its first drawn target (FT-1587: a page read "6 Pending" while the status bar said 9, the three
                // "has no part" findings of its devices having no page of their own).
                let on_page = !pages.is_empty()
                    && !on_target
                    && crate::output::navigators::finding_page(q, &folios, f)
                        .0
                        .is_some_and(|(_, p)| pages.contains(&p));
                if on_target || on_page {
                    findings.push((
                        f.rule.to_owned(),
                        f.subject.clone(),
                        FindingChip {
                            rule: f.rule.to_owned(),
                            severity: f.severity.name().to_owned(),
                            message: f.message.clone(),
                            pending_on: checks::rule(f.rule)
                                .filter(|r| r.meta().kind == checks::RuleKind::Completeness)
                                .and_then(|_| crate::output::navigators::finding_page(q, &folios, f).0)
                                .map(|(folio, _)| folio.to_lowercase()),
                        },
                    ));
                }
            }
        }
        findings.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        findings.dedup_by(|a, b| a.2 == b.2);
        // The persona re-test: a tube's chip opened Messages filtered by its heading "Tube (Working)", which no
        // finding prints; a tube is named there by its tag, else its two ends, an unnumbered wire by its ends.
        let filter = match picked {
            [one] if one.kind == Kind::Connection => one
                .reach
                .get(&Kind::Connection)
                .and_then(|s| s.iter().next())
                .and_then(|c| connection_filter(q, Id::from_uuid(*c))),
            _ => None,
        };
        SheetHeader {
            title,
            items,
            kinds: kinds.into_iter().map(|(k, n)| (k.name().to_owned(), n)).collect(),
            subtitle,
            location,
            findings: findings.into_iter().map(|(_, _, c)| c).collect(),
            filter,
            what: device.map(|d| SheetWhat {
                mark: picked.iter().all(|p| p.kind == Kind::PageItem),
                ..sheet_what(q, d)
            }),
        }
    }
}
