// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The DXF set's glue (doc 09 §4.9): which texts of a page's labels layer are the attributes of a symbol's insert and
//! under which names (one per kind of slot, few, stable and in words: a receiver reads them), and the symbols' names in
//! words that name their blocks. The writer is `ew_export::dxf`.

use std::collections::BTreeMap;

use ew_dl::ew_units::{Pt2, Rect};
use ew_derive::ew_ids::Uuid;
use ew_dl::{DisplayList, EntityRef, Item, ItemKind, Link};
use ew_scene::page::{PageView, SlotTextBox};
use ew_schema::SlotKind;

/// The attribute name of a text a slot of `kind` prints (doc 09 §4.9's table). No name begins with `PIN`, `CONN`, `TERM`
/// or `X`, which the DXF import reads as a connection point; `TAG` and `FUNCTION_TEXT` are what it reads as the tag
/// and the function text.
pub(super) fn attribute_name(kind: &SlotKind) -> String {
    match kind {
        SlotKind::Tag => "TAG".to_owned(),
        SlotKind::FunctionText => "FUNCTION_TEXT".to_owned(),
        SlotKind::TechData => "TECHNICAL_DATA".to_owned(),
        SlotKind::Manufacturer => "MANUFACTURER".to_owned(),
        SlotKind::PartNumber => "PART_NUMBER".to_owned(),
        SlotKind::TypeDesignation => "TYPE_DESIGNATION".to_owned(),
        SlotKind::CpDesignation(idx) => designation(*idx),
        SlotKind::Xref => "CROSS_REFERENCE".to_owned(),
        SlotKind::PlcAddress => "PLC_ADDRESS".to_owned(),
        SlotKind::ProcessTag => "PROCESS_TAG".to_owned(),
        SlotKind::UserProp(name) => format!("PROPERTY_{}", in_capitals(name.as_str())),
        SlotKind::ContactImage => "CONTACT_IMAGE".to_owned(),
        SlotKind::OrderCode => "ORDER_CODE".to_owned(),
        SlotKind::BusAddress => "BUS_ADDRESS".to_owned(),
        SlotKind::BusTermination => "BUS_TERMINATION".to_owned(),
        SlotKind::Unknown(_) => "TEXT".to_owned(),
    }
}

/// The attribute name of the designation of connection point `idx` (0-based): `DESIGNATION_<idx + 1>`.
fn designation(idx: u16) -> String {
    format!("DESIGNATION_{}", u32::from(idx) + 1)
}

/// A name with its letters a to z and its digits in capitals and every other character as `_`.
fn in_capitals(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// Whether an item is one line of text: a glyph run, or a group of the runs of one line.
fn is_line(item: &Item) -> bool {
    match &item.kind {
        ItemKind::Glyphs(_) => true,
        ItemKind::Group(g) => {
            !g.items.is_empty() && g.items.iter().all(|i| matches!(i.kind, ItemKind::Glyphs(_)))
        }
        _ => false,
    }
}

/// The concatenated text of a line.
fn line_text(item: &Item) -> String {
    match &item.kind {
        ItemKind::Glyphs(run) => run.text.clone(),
        ItemKind::Group(g) => g
            .items
            .iter()
            .filter_map(|i| match &i.kind {
                ItemKind::Glyphs(run) => Some(run.text.as_str()),
                _ => None,
            })
            .collect(),
        _ => String::new(),
    }
}

/// The bounds of one item (`DisplayList::bounds`: a run's layout box).
fn bounds(item: &Item) -> Option<Rect> {
    DisplayList {
        items: vec![item.clone()],
    }
    .bounds()
}

/// Whether `inner` lies within `outer`, a nanometre either way.
fn inside(outer: Rect, inner: Rect) -> bool {
    outer.min().x.0 - 1 <= inner.min().x.0
        && outer.min().y.0 - 1 <= inner.min().y.0
        && inner.max().x.0 <= outer.max().x.0 + 1
        && inner.max().y.0 <= outer.max().y.0 + 1
}

/// The labels layer `labels` (page coordinates) without the texts that are its representations' attributes, and those
/// texts by representation with their names, in print order (doc 09 §4.9): a line in the box of one of the
/// representation's slot texts where they print (`slot_texts`) is that slot's; a line that is a black box's pin text is
/// that connection point's designation; a line that is a terminal's strip tag is `STRIP`. A line without a link of its
/// own takes the link of the one item link of its representation that lies over it (a line of a contact list); over a
/// line of several items there is none.
pub(super) fn split_attributes(
    view: &PageView,
    slot_texts: &[SlotTextBox],
    labels: &DisplayList,
) -> (DisplayList, BTreeMap<EntityRef, Vec<(String, Item)>>) {
    let reps: BTreeMap<Uuid, _> = view.reps.iter().map(|r| (r.rep.id.as_uuid(), r)).collect();
    // The item links of the texts: invisible paths with a link source, by their entity.
    let mut links: BTreeMap<EntityRef, Vec<(Pt2, Link)>> = BTreeMap::new();
    for item in &labels.items {
        if let (ItemKind::Path(p), Some(e), Some(link @ Link::Source(_))) = (&item.kind, item.entity, item.link)
            && p.stroke.is_none()
            && p.fill.is_none()
            && let Some(b) = bounds(item)
        {
            let mid = |a: i64, b: i64| a + (b - a) / 2;
            let centre = Pt2::new(
                ew_dl::ew_units::Nm(mid(b.min().x.0, b.max().x.0)),
                ew_dl::ew_units::Nm(mid(b.min().y.0, b.max().y.0)),
            );
            links.entry(e).or_default().push((centre, link));
        }
    }
    let mut kept = Vec::with_capacity(labels.items.len());
    let mut attributes: BTreeMap<EntityRef, Vec<(String, Item)>> = BTreeMap::new();
    for item in &labels.items {
        let name = item
            .entity
            .filter(|_| is_line(item))
            .and_then(|e| Some((e, reps.get(&e.0)?, bounds(item)?)))
            .and_then(|(e, rv, b)| {
                let def = rv.symbol.as_ref()?;
                let text = line_text(item);
                let name = slot_texts
                    .iter()
                    .filter(|s| s.rep == e.0)
                    .find(|s| inside(s.rect, b))
                    .and_then(|s| def.text_slots.get(usize::from(s.slot)))
                    .map(|slot| attribute_name(&slot.kind))
                    .or_else(|| rv.pins.iter().find(|p| p.text == text).map(|p| designation(p.idx)))
                    .or_else(|| {
                        view.strip_tags
                            .iter()
                            .any(|t| t.rep == rv.rep.id && t.text == text)
                            .then(|| "STRIP".to_owned())
                    })?;
                Some((e, b, name))
            });
        match name {
            Some((e, b, name)) => {
                let mut line = item.clone();
                if line.link.is_none() {
                    let over: Vec<Link> = links
                        .get(&e)
                        .into_iter()
                        .flatten()
                        .filter(|(c, _)| b.contains(*c))
                        .map(|(_, l)| *l)
                        .collect();
                    if let [one] = over.as_slice() {
                        line.link = Some(*one);
                    }
                }
                attributes.entry(e).or_default().push((name, line));
            }
            None => kept.push(item.clone()),
        }
    }
    (DisplayList { items: kept }, attributes)
}

/// The name in words of each symbol the page's representations draw, in the view's language: the names of its blocks.
pub(super) fn symbol_names(view: &PageView, out: &mut BTreeMap<Uuid, String>) {
    for rv in &view.reps {
        if let Some(def) = &rv.symbol {
            out.entry(rv.rep.symbol.as_uuid())
                .or_insert_with(|| ew_scene::forms::ml_text(&def.name, view.language.as_ref()).to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A connection point counts from 1, and a user property's name keeps its letters and digits in capitals.
    #[test]
    fn a_designation_and_a_property_are_named_in_capitals() {
        assert_eq!(attribute_name(&SlotKind::CpDesignation(0)), "DESIGNATION_1");
        let prop = ew_mltext::Ident::from_input("rated-voltage").unwrap();
        assert_eq!(attribute_name(&SlotKind::UserProp(prop)), "PROPERTY_RATED_VOLTAGE");
    }
}
