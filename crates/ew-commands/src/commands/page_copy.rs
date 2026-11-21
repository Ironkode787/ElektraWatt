// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `page.copy` (doc 06 §3.6, "A copy of pages"): whole pages copied for a second installation in one transaction, by
//! the rule a person gives: new folios (an offset, or after a page), a structure node of the copy's own, renamed frozen
//! tags and interruption-point names. The pages arrive as a fragment with page records, which `Session::copy_pages`
//! builds from the page macro's fragment; `edit.paste`'s engine inserts it with the pages' positions kept.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::{Id, Uuid};
use ew_mltext::Ident;
use ew_model::LINKS;
use ew_schema::{Folio, FunctionKind, Kind, Open, Page, PageType, Record, Section, StructureNode};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::device::allowed_rep_types;
use super::page::{held_folio_problem, printed_folio, section_pages};
use super::paste::{Fragment, FragmentNode, Insertion, NewIds, at_path, occurrences};
use super::structure::CreateStructureNode;
use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::{IdentS, UuidS};
use crate::text::{Text, text};
use crate::tx::TxBuilder;

/// The record kinds the fragment of a copy of pages carries besides its page records: a page macro's, and a cable's
/// conductors, the bonds between copied ends and the report blocks embedded on copied pages.
pub const PAGE_COPY_KINDS: [Kind; 13] = [
    Kind::Device,
    Kind::Function,
    Kind::Representation,
    Kind::Connection,
    Kind::Junction,
    Kind::Conductor,
    Kind::Channel,
    Kind::PotentialDef,
    Kind::Bond,
    Kind::DeviceLink,
    Kind::PageItem,
    Kind::StructureBox,
    Kind::ReportBlock,
];

/// One structure node of the copied pages and the code of the copy's node beside it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NodeSwap {
    /// The node the copied pages, devices or boxes name.
    #[schemars(with = "UuidS")]
    pub from: Id<StructureNode>,
    /// The code of the copy's node: the node of that code with the same aspect and parent, created when there is
    /// none.
    #[schemars(with = "IdentS")]
    pub to: Ident,
}

/// One text replacement of the names of a copy.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Rename {
    /// The old text, not empty.
    pub from: String,
    /// The new text.
    pub to: String,
}

/// `text` with every occurrence of an old text of `rules` replaced by its new text: at each place the longest old text
/// that begins there, in one pass from the left, so a new text is never replaced again (`26=27` and `6=7` turn `26K6`
/// into `27K7`). An empty old text replaces nothing.
pub fn renamed(text: &str, rules: &[Rename]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        let hit = rules
            .iter()
            .filter(|r| !r.from.is_empty() && rest.starts_with(&r.from))
            .max_by_key(|r| r.from.len());
        match hit {
            Some(r) => {
                out.push_str(&r.to);
                rest = &rest[r.from.len()..];
            }
            None => {
                out.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    out
}

/// The prevent rules whose violation a copy of pages may carry, because a copy can only reproduce it (doc 06 §3.6,
/// "Refused in words"): a copied strip holds only copied terminals, with their designations, order and bridges
/// (EW-TERM-001 a terminal without a designation, EW-TERM-002 two terminals of one designation, EW-TERM-004 a bridge
/// group that breaks CI-06); a copied cable holds copied conductors on copied wires (EW-CORE-031 a core's colour,
/// EW-CORE-036 a core on two wires); a copied valve stands at its copied module's position (EW-FL-013). The check
/// reports each in the copy as in the source. Every other prevent rule judges a copy as any edit: a copy can make
/// their state anew (a source joining its original's name, EW-CORE-052; a manual PLC address, symbol or slot met
/// again in the project's one configuration project, EW-PLC-001, EW-PLC-007, EW-PLC-011; a name the rename makes
/// confusable, EW-RDS-009; a folio, EW-CORE-022, which the command refuses first).
pub const REPRODUCED: [&str; 6] = [
    "EW-TERM-001",
    "EW-TERM-002",
    "EW-TERM-004",
    "EW-CORE-031",
    "EW-CORE-036",
    "EW-FL-013",
];

/// Whether the copy of device `d` stands in a structure node of the copy's own: a node of `d`'s resolved structure
/// (its own slots, else its strip's or parent's, its structure box's or its page's, `ew_derive::StructureOf`) is one
/// the rule `swapped` replaces, so its full designation differs from its original's.
pub fn designation_moves(
    q: &ew_derive::Q,
    d: Id<ew_schema::Device>,
    swapped: &BTreeSet<Id<StructureNode>>,
) -> bool {
    q.get::<ew_derive::StructureOf>(d)
        .slots
        .values()
        .any(|s| swapped.contains(&s.node))
}

/// Whether the whole text `text` is the printed tag `tag` (`-X1`), and with its sign (`-X1`) or without it (`X1`).
pub fn names_tag(text: &str, tag: &str) -> Option<bool> {
    if tag.is_empty() {
        return None;
    }
    if text == tag {
        return Some(true);
    }
    let bare = tag.strip_prefix('-')?;
    (!bare.is_empty() && text == bare).then_some(false)
}

/// What a copied free text names by its whole text (doc 06 §3.6): copied devices by their printed tag before the
/// copy (several when they shared it), with the sign or without it; or a copied interruption point by a name the
/// rename changes, with the new name.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TextNames {
    /// The devices (source IDs) whose printed tag the text is, and whether it holds the tag's sign.
    Devices(Vec<Uuid>, bool),
    /// The interruption point's new name.
    Point(String),
}

/// The free texts of `fragment`'s records (source item IDs) whose whole text names a device of the fragment by its
/// printed tag on `q` (the source), or an interruption point of the fragment by a name `rename` changes; a bound
/// text, a text in several languages and one that holds a tag inside a longer text name nothing.
pub fn text_names(q: &ew_derive::Q, fragment: &Fragment, rename: &[Rename]) -> Vec<(Uuid, TextNames)> {
    let mut tags: BTreeMap<String, Vec<Uuid>> = BTreeMap::new();
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    let mut texts: Vec<(Uuid, String)> = Vec::new();
    for v in &fragment.records {
        match Record::from_value(v.clone()) {
            Ok(Record::Device(d)) => {
                let tag = ew_derive::short_tag(q, d.id);
                if !tag.is_empty() {
                    tags.entry(tag).or_default().push(d.id.as_uuid());
                }
            }
            Ok(Record::Function(f)) => {
                if let FunctionKind::Ip(ip) = &f.kind
                    && ip.external.is_none()
                {
                    let new = renamed(ip.name.as_str(), rename);
                    if new != ip.name.as_str() {
                        names.insert(ip.name.as_str().to_owned(), new);
                    }
                }
            }
            Ok(Record::PageItem(item)) => {
                if let ew_schema::PageItemKind::FreeText(t) = &item.kind
                    && t.bind.is_none()
                    && let Some(text) = t.text.as_neutral()
                {
                    texts.push((item.id.as_uuid(), text.trim().to_owned()));
                }
            }
            _ => {}
        }
    }
    texts
        .into_iter()
        .filter_map(|(item, text)| {
            let device = tags.iter().find_map(|(tag, devices)| {
                names_tag(&text, tag).map(|sign| TextNames::Devices(devices.clone(), sign))
            });
            device
                .or_else(|| names.get(&text).map(|n| TextNames::Point(n.clone())))
                .map(|named| (item, named))
        })
        .collect()
}

/// `page.copy` (`CopyPages`, doc 06 §3.6 "A copy of pages"): copies the pages of `fragment` (a fragment of this project
/// with page records, `Session::copy_pages`) in one transaction, one undo step.
///
/// **The rule.** The folios: `offset` adds a whole number to each page's folio number (a label that begins with the
/// number follows it: 6001.1 becomes 7001.1), in the section whose folio range holds the new number (a page of no
/// section stays in none); `after` proposes them after that page, as a page macro's (doc 06 §5.6). `structure` names
/// for a node of the copied records the code of the copy's node: the node of that code with the same aspect and parent,
/// created here when there is none; every structure slot of a copied page, device or box that names the node names
/// the copy's. `rename` replaces texts in every frozen tag and every interruption-point name but an external point's
/// ([`renamed`]); a frozen tag the rule leaves as it was is cleared where the copy's full designation would be its
/// original's ([`designation_moves`]), so the copy takes its scheme's tag there and keeps the frozen one in a node of
/// its own.
///
/// **Effects.** `edit.paste`'s effects with these differences: the pages' positions stay as the source holds them (a
/// symbol off the grid and a symbol type the page's kind would not take for a paste too); there is no bound on the
/// records; every record's ID is minted, a PLC channel's `plc_channel_id(new module, number)`; every device keeps its
/// tag counter, so its tag follows the numbering scheme from its new page (online numbering gives the next free
/// counter where two copies would meet on one page); the records keep a reference to a held library copy or an
/// external reference; a copied free text whose whole text names a copied device by its printed tag, or a copied
/// interruption point by a name the rename changes, takes the copy's tag or name ([`text_names`]). The prevent rules
/// of [`REPRODUCED`] do not refuse it: it can only reproduce their states.
///
/// **Rejected**, at the JSON Pointer named: `/fragment/project` a fragment of another project; `/fragment/pages` none;
/// `/fragment/pages/<i>` a page that takes no symbols, a generated page, or one of another grid; `/offset` both an
/// offset and `after`, or neither; `/after` a page that does not exist; `/offset` a new folio beyond the folio numbers,
/// outside every section's range where its page had a section, or one a page, a divider or a report block holds;
/// `/structure/<i>` a node the copied records do not name, one named twice, a node of an aspect this version does not
/// know, or a code that is the node's own; `/rename/<i>` an empty or repeated old text, or a frozen tag or name that
/// the rule leaves empty or malformed; and every refusal of `edit.paste` (`/fragment/…`). Cost: Bulk. Outcome: the first
/// new page.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CopyPages {
    /// The pages as a fragment with page records.
    pub fragment: Fragment,
    /// The number added to each folio number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
    /// The page after which the copies go, with proposed folios.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub after: Option<Id<Page>>,
    /// The copy's structure nodes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structure: Vec<NodeSwap>,
    /// The replacements in frozen tags and interruption-point names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rename: Vec<Rename>,
}

impl CopyPages {
    fn invalid(at: impl Into<String>, message: Text) -> CmdError {
        CmdError::invalid_params(Self::NAME, at, message)
    }

    /// The page records, typed.
    fn pages(&self) -> Result<Vec<Page>, CmdError> {
        let mut out = Vec::with_capacity(self.fragment.pages.len());
        for (i, v) in self.fragment.pages.iter().enumerate() {
            let at = format!("/fragment/pages/{i}");
            let Ok(Record::Page(p)) = Record::from_value(v.clone()) else {
                return Err(Self::invalid(at, text!("refusal-paste-page-record-kind")));
            };
            let generated = matches!(
                p.kind,
                PageType::Report | PageType::TableOfContents | PageType::SectionDivider
            );
            if generated || allowed_rep_types(&p.kind).is_some_and(<[_]>::is_empty) {
                return Err(Self::invalid(
                    at,
                    text!(
                        "refusal-page-copy-kind",
                        printed_folio(p.folio.number, p.folio.label.as_ref()),
                        crate::words::page_kind(&p.kind)
                    ),
                ));
            }
            out.push(p);
        }
        Ok(out)
    }

    /// The new folio of every page by its source ID (`offset`).
    fn offset_folios(
        tx: &TxBuilder<'_>,
        pages: &[Page],
        offset: u32,
    ) -> Result<BTreeMap<Uuid, Folio>, CmdError> {
        let sections = tx.iter::<Section>();
        let mut out = BTreeMap::new();
        let mut printed_new: BTreeSet<(Option<Id<Section>>, String)> = BTreeSet::new();
        for p in pages {
            let old = printed_folio(p.folio.number, p.folio.label.as_ref());
            let Some(number) = p.folio.number.checked_add(offset) else {
                return Err(Self::invalid(
                    "/offset",
                    text!("refusal-page-copy-folio-too-large", old, offset.to_string()),
                ));
            };
            // A label that begins with the folio number follows it (`6001.1` becomes `7001.1`); any other stays.
            let head = p.folio.number.to_string();
            let label = match &p.folio.label {
                Some(l) => match l.as_str().strip_prefix(&head) {
                    Some(rest) if !rest.starts_with(|c: char| c.is_ascii_digit()) => {
                        let text = format!("{number}{rest}");
                        Some(text.parse::<Ident>().map_err(|_| {
                            CmdError::internal(format!("folio label `{text}` is not an identifier"))
                        })?)
                    }
                    _ => Some(l.clone()),
                },
                None => None,
            };
            let printed = printed_folio(number, label.as_ref());
            let section = match p.folio.section {
                None => None,
                Some(_) => Some(
                    sections
                        .iter()
                        .find(|s| s.folio_range.0 <= number && number <= s.folio_range.1)
                        .ok_or_else(|| {
                            Self::invalid("/offset", text!("refusal-page-copy-no-section", printed.clone()))
                        })?,
                ),
            };
            let section_id = section.map(|s| s.id);
            let held = section_pages(tx, section_id)
                .into_iter()
                .any(|(_, f)| printed_folio(f.number, f.label.as_ref()) == printed);
            if held || !printed_new.insert((section_id, printed.clone())) {
                return Err(Self::invalid(
                    "/offset",
                    text!("refusal-page-copy-folio-held", printed),
                ));
            }
            if let Some(reason) = held_folio_problem(tx, section.map(|s| &**s), section_id, &printed) {
                return Err(Self::invalid("/offset", reason));
            }
            out.insert(
                p.id.as_uuid(),
                Folio {
                    section: section_id,
                    number,
                    label,
                    extra: ew_schema::Extra::new(),
                },
            );
        }
        Ok(out)
    }

    /// The copy's structure nodes: each swap's node found or created; every slot of the fragment's records and pages
    /// that names a swapped node names the copy's, and the fragment names the copy's node.
    fn swap_nodes(&self, tx: &mut TxBuilder<'_>, fragment: &mut Fragment) -> Result<(), CmdError> {
        let mut seen = BTreeSet::new();
        for (i, swap) in self.structure.iter().enumerate() {
            let at = format!("/structure/{i}");
            let named = fragment.nodes.iter().find(|n| n.id == swap.from).cloned();
            let (Some(named), Some(from)) = (named, tx.get::<StructureNode>(swap.from)) else {
                return Err(Self::invalid(at, text!("refusal-page-copy-node-not-named")));
            };
            if !seen.insert(swap.from) {
                return Err(Self::invalid(at, text!("refusal-page-copy-node-twice")));
            }
            if from.code == swap.to {
                return Err(Self::invalid(
                    at,
                    text!("refusal-page-copy-node-same", swap.to.as_str()),
                ));
            }
            let Open::Known(aspect) = from.aspect.clone() else {
                return Err(Self::invalid(at, text!("refusal-page-copy-node-aspect")));
            };
            let held = tx
                .iter::<StructureNode>()
                .iter()
                .find(|n| n.aspect == from.aspect && n.parent == from.parent && n.code == swap.to)
                .map(|n| n.id);
            let to = match held {
                Some(id) => id,
                None => CreateStructureNode {
                    aspect,
                    code: swap.to.clone(),
                    parent: from.parent,
                    aliases: Vec::new(),
                    description: ew_mltext::MlText::default(),
                }
                .run(tx)?
                .primary
                .map(|(_, id)| id.cast())
                .ok_or_else(|| CmdError::internal("structure.create_node made no node"))?,
            };
            for v in fragment.records.iter_mut().chain(fragment.pages.iter_mut()) {
                let Some(kind) = v.get("$k").and_then(Value::as_str).and_then(Kind::from_name) else {
                    continue;
                };
                for decl in LINKS
                    .iter()
                    .filter(|d| d.from == kind && d.to == Kind::StructureNode)
                {
                    for (path, node) in occurrences(v, decl.field) {
                        if node == swap.from.as_uuid()
                            && let Some(slot) = at_path(v, &path)
                        {
                            *slot = Value::from(to.as_uuid().hyphenated().to_string());
                        }
                    }
                }
            }
            if !fragment.nodes.iter().any(|n| n.id == to) {
                let mut path = named.path.clone();
                path.pop();
                path.push(swap.to.as_str().to_owned());
                fragment.nodes.push(FragmentNode {
                    id: to,
                    aspect: named.aspect.clone(),
                    path,
                    aliases: Vec::new(),
                });
            }
        }
        Ok(())
    }

    /// The rename: every frozen tag and every interruption-point name but an external point's, through [`renamed`];
    /// a frozen tag it leaves as it was is cleared where the copy's full designation would be its original's
    /// ([`designation_moves`] says where the structure rule gives it another).
    fn rename_names(&self, tx: &TxBuilder<'_>, fragment: &mut Fragment) -> Result<(), CmdError> {
        let q = ew_derive::Q::new(tx.base_model());
        let swapped: BTreeSet<Id<StructureNode>> = self.structure.iter().map(|s| s.from).collect();
        let mut froms = BTreeSet::new();
        for (i, r) in self.rename.iter().enumerate() {
            if r.from.is_empty() {
                return Err(Self::invalid(
                    format!("/rename/{i}/from"),
                    text!("refusal-page-copy-rename-empty"),
                ));
            }
            if !froms.insert(r.from.as_str()) {
                return Err(Self::invalid(
                    format!("/rename/{i}/from"),
                    text!("refusal-page-copy-rename-twice", r.from.as_str()),
                ));
            }
        }
        let name = |old: &str| -> Result<Ident, CmdError> {
            let new = renamed(old, &self.rename);
            new.parse::<Ident>().map_err(|_| {
                Self::invalid(
                    "/rename",
                    text!("refusal-page-copy-rename-not-a-name", old, new.as_str()),
                )
            })
        };
        for v in &mut fragment.records {
            let Ok(mut record) = Record::from_value(v.clone()) else {
                continue;
            };
            let changed = match &mut record {
                Record::Device(d) => match d.tag.frozen.as_mut() {
                    Some(frozen) => {
                        let new = name(frozen.text.as_str())?;
                        if new == frozen.text {
                            if !designation_moves(&q, d.id, &swapped) {
                                d.tag.frozen = None;
                            }
                        } else {
                            frozen.text = new;
                        }
                        true
                    }
                    None => false,
                },
                Record::Function(f) => match &mut f.kind {
                    FunctionKind::Ip(ip) if ip.external.is_none() && !self.rename.is_empty() => {
                        ip.name = name(ip.name.as_str())?;
                        true
                    }
                    _ => false,
                },
                _ => false,
            };
            if changed {
                *v = serde_json::to_value(&record).map_err(|e| CmdError::internal(e.to_string()))?;
            }
        }
        Ok(())
    }
}

impl CopyPages {
    /// The copied free texts that name a copied device or a renamed interruption point ([`text_names`]) take the
    /// copy's tag (with or without its sign, as the text had it) or the new name; a tag copied devices shared and
    /// their copies no longer share is left. The copies' tags are read after online numbering, on the transaction's
    /// planning snapshot. `members` maps the fragment's IDs to the copy's.
    fn retext(&self, tx: &mut TxBuilder<'_>, members: &[(Uuid, Uuid)]) -> Result<(), CmdError> {
        let named = text_names(&ew_derive::Q::new(tx.base_model()), &self.fragment, &self.rename);
        if named.is_empty() {
            return Ok(());
        }
        let new_of: BTreeMap<Uuid, Uuid> = members.iter().copied().collect();
        crate::normalise::Normaliser::run(&crate::normalise::OnlineNumbering, tx)?;
        let mut texts: Vec<(Uuid, String)> = Vec::new();
        {
            let q = ew_derive::Q::new(tx.planning_model()?);
            for (item, names) in named {
                let Some(&item) = new_of.get(&item) else { continue };
                let text = match names {
                    TextNames::Point(name) => Some(name),
                    TextNames::Devices(devices, sign) => {
                        let tags: BTreeSet<String> = devices
                            .iter()
                            .filter_map(|d| new_of.get(d))
                            .map(|d| ew_derive::short_tag(&q, Id::from_uuid(*d)))
                            .collect();
                        match (tags.len(), tags.into_iter().next()) {
                            (1, Some(tag)) if sign => Some(tag),
                            (1, Some(tag)) => Some(tag.strip_prefix('-').unwrap_or(&tag).to_owned()),
                            _ => None,
                        }
                    }
                };
                texts.extend(text.map(|t| (item, t)));
            }
        }
        for (item, text) in texts {
            tx.update::<ew_schema::PageItem>(Id::from_uuid(item), |i| {
                if let ew_schema::PageItemKind::FreeText(t) = &mut i.kind {
                    t.text = ew_mltext::MlText::neutral(&text);
                }
            })?;
        }
        Ok(())
    }
}

impl Command for CopyPages {
    const NAME: &'static str = "page.copy";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk().reproducing(&REPRODUCED)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.fragment.project.as_uuid() != tx.project().id.as_uuid() {
            return Err(Self::invalid(
                "/fragment/project",
                text!("refusal-page-copy-other-project"),
            ));
        }
        if self.fragment.pages.is_empty() {
            return Err(Self::invalid(
                "/fragment/pages",
                text!("refusal-page-copy-no-page"),
            ));
        }
        let pages = self.pages()?;
        let (target, folios) = match (self.offset, self.after) {
            (Some(_), Some(_)) => {
                return Err(Self::invalid(
                    "/offset",
                    text!("refusal-page-copy-offset-and-after"),
                ));
            }
            (None, None) => return Err(Self::invalid("/offset", text!("refusal-page-copy-no-folios"))),
            (None, Some(after)) => {
                if tx.get::<Page>(after).is_none() {
                    return Err(Self::invalid("/after", text!("refusal-params-page-gone")));
                }
                (after, None)
            }
            (Some(offset), None) => (pages[0].id, Some(Self::offset_folios(tx, &pages, offset)?)),
        };
        let mut fragment = self.fragment.clone();
        self.swap_nodes(tx, &mut fragment)?;
        self.rename_names(tx, &mut fragment)?;
        // Every device keeps its counter: its tag follows the scheme from its new page.
        let devices: BTreeSet<Uuid> = fragment
            .records
            .iter()
            .filter(|r| r.get("$k").and_then(Value::as_str) == Some(Kind::Device.name()))
            .filter_map(|r| r.get("id")?.as_str()?.parse().ok())
            .collect();
        let insertion = Insertion {
            name: Self::NAME,
            prefix: "/fragment",
            page: target,
            at: fragment.anchor,
            orient: ew_units::Orient2::R0,
            fragment: &fragment,
            kinds: &PAGE_COPY_KINDS,
            kinds_refusal: text!("refusal-page-copy-kinds"),
            ids: NewIds::Mint,
            adapt: &[],
            keep_frozen: true,
            keep_counter: devices,
            keep_refs: BTreeSet::new(),
            folios: folios.as_ref(),
            bounded: false,
        };
        let out = insertion.run(tx)?;
        self.retext(tx, &out.members)?;
        Ok(Outcome {
            primary: out.pages.first().map(|p| (Kind::Page, p.cast())),
            note: None,
            notices: out.notices,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(pairs: &[(&str, &str)]) -> Vec<Rename> {
        pairs
            .iter()
            .map(|(a, b)| Rename {
                from: (*a).to_owned(),
                to: (*b).to_owned(),
            })
            .collect()
    }

    #[test]
    fn the_longest_old_text_wins_and_nothing_is_replaced_twice() {
        let r = rules(&[("6", "7"), ("26", "27")]);
        assert_eq!(renamed("26K6", &r), "27K7");
        assert_eq!(renamed("-K1", &r), "-K1");
        // A new text that holds an old one is not replaced again.
        assert_eq!(renamed("6", &rules(&[("6", "66")])), "66");
        assert_eq!(renamed("24V", &rules(&[("24V", "24V.2")])), "24V.2");
        assert_eq!(renamed("Ü6", &rules(&[("6", "7")])), "Ü7");
    }
}
