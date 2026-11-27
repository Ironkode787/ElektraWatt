// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The workshop stock as Parts shows it (doc 07 §2.5 "The workshop stock", §5.10): the kinds a stock holds, where a part
//! stands in it, and the one `workshop.stock` of each change (a part added, taken out or moved to another place), one
//! undo step; and the stock a new project starts with (Use this stock for new projects).

use ew_commands::AnyCommand;
use ew_ids::{Id, Uuid, v5};
use ew_schema::{LibDocument, Part, PartKind, PartRef};
use serde_json::{Value, json};

use crate::Session;
use crate::part_offers::lib_ref;

/// The kinds a stock holds, in the order the Workshop stock view lists them: wires, ferrules and lugs, markers, cable
/// glands.
pub const STOCK_KINDS: [PartKind; 4] = [
    PartKind::Wire,
    PartKind::Ferrule,
    PartKind::Marker,
    PartKind::Gland,
];

/// Whether a part of `kind` may stand in the stock.
pub fn holds(kind: &PartKind) -> bool {
    STOCK_KINDS.contains(kind)
}

/// A change of the stock, as Parts makes it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StockChange {
    /// The part of the project's copy `copy` added after the stock; where the project does not hold it, the library
    /// item `source` is embedded with it.
    Add {
        /// The copy the part has, or will have, in the project.
        copy: Uuid,
        /// The library and the item it comes from, for a part the project does not hold yet.
        source: Option<(Uuid, Uuid)>,
    },
    /// The part of copy `copy` taken out of the stock.
    Remove(Uuid),
    /// The part of copy `copy` moved to place `to` among the stock's parts of its kind (1 is the first, which is taken
    /// first).
    Move {
        /// The copy.
        copy: Uuid,
        /// Its new place among its kind, from 1; a place past the last is the last.
        to: usize,
    },
}

impl Session {
    /// The project's workshop stock, its copies in its order of preference.
    pub fn stock(&self) -> Vec<Uuid> {
        let q = self.query();
        q.project()
            .settings
            .workshop
            .stock
            .iter()
            .map(|r| r.copy.as_uuid())
            .collect()
    }

    /// The kind of the project's copy `copy`, if it holds it.
    fn held_kind(&self, copy: Uuid) -> Option<PartKind> {
        self.query()
            .parts()
            .get(Id::<Part>::from_uuid(copy))
            .map(|c| c.item.kind.clone())
    }

    /// Where the project's copy `copy` stands in the stock: its place among the stock's parts of its kind (from 1) and
    /// how many of its kind the stock holds; `None` where it is not in the stock.
    pub fn stock_place(&self, copy: Uuid) -> Option<(usize, usize)> {
        let kind = self.held_kind(copy)?;
        let same: Vec<Uuid> = self
            .stock()
            .into_iter()
            .filter(|c| self.held_kind(*c).as_ref() == Some(&kind))
            .collect();
        let at = same.iter().position(|c| *c == copy)?;
        Some((at + 1, same.len()))
    }

    /// The `workshop.stock` that makes `change`, the cut increment kept; the command refuses what the stock cannot
    /// hold in its words.
    ///
    /// # Errors
    ///
    /// The refusal in words: a part added that is no wire, ferrule, marker or gland (`error-stock-kind`), or one the
    /// project does not hold whose library has it no more (`error-stock-gone`).
    pub fn stock_command(&self, change: StockChange) -> Result<AnyCommand, crate::Text> {
        let mut stock = self.stock();
        let mut copies: Vec<Value> = Vec::new();
        match change {
            StockChange::Add { copy, source } => {
                let kind = match (self.held_kind(copy), source) {
                    (Some(kind), _) => kind,
                    (None, Some((library, item))) => {
                        let r = lib_ref(library, item);
                        let docs = self
                            .closure_docs(std::slice::from_ref(&r))
                            .map_err(|_| text!("error-stock-gone"))?;
                        let kind = docs.iter().find_map(|d| match d {
                            LibDocument::Part(c) if c.item_id.as_uuid() == item => {
                                Some((c.item.kind.clone(), c.item.part_number.as_str().to_owned()))
                            }
                            _ => None,
                        });
                        let Some((kind, number)) = kind else {
                            return Err(text!("error-stock-gone"));
                        };
                        if !holds(&kind) {
                            return Err(text!("error-stock-kind", number));
                        }
                        copies = docs
                            .iter()
                            .map(|d| serde_json::to_value(d).expect("a library document has a JSON form"))
                            .collect();
                        stock.push(v5::lib_copy_id::<Part>(library, item).as_uuid());
                        return Ok(stock_cmd(&stock, copies));
                    }
                    (None, None) => return Err(text!("error-stock-gone")),
                };
                if !holds(&kind) {
                    let number = self
                        .query()
                        .parts()
                        .get(Id::<Part>::from_uuid(copy))
                        .map(|c| c.item.part_number.as_str().to_owned())
                        .unwrap_or_default();
                    return Err(text!("error-stock-kind", number));
                }
                stock.push(copy);
            }
            StockChange::Remove(copy) => stock.retain(|c| *c != copy),
            StockChange::Move { copy, to } => {
                let kind = self.held_kind(copy);
                stock.retain(|c| *c != copy);
                let same: Vec<usize> = (0..stock.len())
                    .filter(|i| self.held_kind(stock[*i]) == kind)
                    .collect();
                let at = match same.get(to.saturating_sub(1)) {
                    Some(i) => *i,
                    None => same.last().map_or(stock.len(), |i| i + 1),
                };
                stock.insert(at, copy);
            }
        }
        Ok(stock_cmd(&stock, copies))
    }

    /// The stock's parts as the library items they are copies of (Use this stock for new projects): `(library, item)`
    /// in the stock's order.
    pub fn stock_items(&self) -> Vec<(Uuid, Uuid)> {
        let q = self.query();
        self.stock()
            .into_iter()
            .filter_map(|c| q.parts().get(Id::<Part>::from_uuid(c)))
            .map(|c| (c.library_id.as_uuid(), c.item_id.as_uuid()))
            .collect()
    }

    /// The `workshop.stock` of a new project whose stock is the library items `items` (Use this stock for new
    /// projects), with the documents that embed them; an item that no longer resolves, or is no part the stock holds,
    /// is left out. `None` where none is left.
    pub(crate) fn items_stock_command(&self, items: &[(Uuid, Uuid)]) -> Option<AnyCommand> {
        let refs: Vec<_> = items
            .iter()
            .map(|(l, i)| lib_ref(*l, *i))
            .filter(
                |r| matches!(self.libraries().resolve(r), Ok(LibDocument::Part(c)) if holds(&c.item.kind)),
            )
            .collect();
        if refs.is_empty() {
            return None;
        }
        let docs = self.closure_docs(&refs).ok()?;
        let stock: Vec<Uuid> = refs
            .iter()
            .map(|r| v5::lib_copy_id::<Part>(r.library.as_uuid(), r.item.as_uuid()).as_uuid())
            .collect();
        let copies = docs
            .iter()
            .map(|d| serde_json::to_value(d).expect("a library document has a JSON form"))
            .collect();
        Some(stock_cmd(&stock, copies))
    }
}

/// `workshop.stock` of `stock` with the documents `copies`; the cut increment is kept.
fn stock_cmd(stock: &[Uuid], copies: Vec<Value>) -> AnyCommand {
    let stock: Vec<PartRef> = stock
        .iter()
        .map(|c| PartRef::new(Id::<Part>::from_uuid(*c)))
        .collect();
    AnyCommand {
        name: "workshop.stock".into(),
        params: json!({ "stock": stock, "copies": copies }),
    }
}
