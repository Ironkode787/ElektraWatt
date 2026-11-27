// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The parts a symbol draws (doc 07 §5.6): the parts a page draws with a symbol, which its sheet lists (Parts drawn
//! with it), and Use it for every part with this piece…, which gives a piece's own symbol to every part of his own with
//! a piece of that function and that many pins. One function finds the parts of a piece ([`Session::parts_drawing`]);
//! both read it.

use ew_ids::Uuid;
use ew_schema::{Kind, LibCopy, LibDocument, LibRef, Part};

use crate::Session;
use crate::library_ops::UpdatePlan;

/// A part a symbol draws, or a piece of it does.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DrawnPart {
    /// Its number.
    pub number: String,
    /// Its maker's name, as the library or the project names it.
    pub maker: String,
    /// The library that holds it (`None`: only the project's).
    pub library: Option<Uuid>,
    /// Its item in that library, or its copy in the project.
    pub item: Uuid,
    /// Whether a library he can write holds it: his own, which Use it for every part changes.
    pub mine: bool,
    /// Whether the project holds a copy of it.
    pub held: bool,
    /// The template entries the symbol (or a symbol of that piece) draws.
    pub entries: Vec<usize>,
}

/// Whether `a` and `b` name the same library item.
fn same(a: &LibRef, b: &LibRef) -> bool {
    a.library == b.library && a.item == b.item
}

impl Session {
    /// The parts of the installed libraries and of the project with a template entry that `wanted` picks, each once (a
    /// library's part the project holds is its library row, marked held): what a symbol's sheet lists and what Use it
    /// for every part changes.
    pub fn parts_drawing(&self, wanted: &dyn Fn(&ew_schema::TemplateFn) -> bool) -> Vec<DrawnPart> {
        let q = self.query();
        let registry = self.libraries();
        let maker = |p: &Part| {
            p.manufacturer
                .extra
                .get("key")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let entries = |p: &Part| -> Vec<usize> {
            (p.function_template.iter().enumerate())
                .filter(|(_, t)| wanted(t))
                .map(|(i, _)| i)
                .collect()
        };
        let mut out: Vec<DrawnPart> = Vec::new();
        for library in registry.libraries() {
            for doc in registry.documents(library.id, Kind::Part) {
                let LibDocument::Part(c) = doc else { continue };
                let found = entries(&c.item);
                if found.is_empty() {
                    continue;
                }
                let held = q
                    .parts()
                    .iter()
                    .any(|(_, h)| h.library_id == c.library_id && h.item_id == c.item_id);
                out.push(DrawnPart {
                    number: c.item.part_number.as_str().to_owned(),
                    maker: maker(&c.item),
                    library: Some(c.library_id.as_uuid()),
                    item: c.item_id.as_uuid(),
                    mine: library.writable,
                    held,
                    entries: found,
                });
            }
        }
        for (id, c) in q.parts().iter() {
            let in_library = out
                .iter()
                .any(|d| d.library == Some(c.library_id.as_uuid()) && d.item == c.item_id.as_uuid());
            let found = entries(&c.item);
            if in_library || found.is_empty() {
                continue;
            }
            out.push(DrawnPart {
                number: c.item.part_number.as_str().to_owned(),
                maker: maker(&c.item),
                library: None,
                item: id.as_uuid(),
                mine: false,
                held: true,
                entries: found,
            });
        }
        out.sort_by(|a, b| (&a.number, &a.maker).cmp(&(&b.number, &b.maker)));
        out
    }

    /// The parts a page draws with symbol `symbol` (Parts drawn with it, doc 07 §5.6): those with a piece that names
    /// it, and those with a piece of its function that names no symbol of its own, which a page draws with the
    /// function's symbols.
    pub fn parts_drawn_with(&self, symbol: Option<&LibRef>, fndef: &LibRef) -> Vec<DrawnPart> {
        self.parts_drawing(&|t| match &t.symbol {
            Some(s) => symbol.is_some_and(|x| same(s, x)),
            None => same(&t.fndef, fndef),
        })
    }

    /// The parts Use it for every part with this piece… changes: his own (a library he can write holds them), not
    /// `except` (the part being drawn, a library item), with a piece of function `fndef` and `pins` pins that
    /// `symbol` does not draw yet. Never a starter part; a project's copy only through the library's update, which
    /// its card names.
    pub fn parts_with_piece(
        &self,
        fndef: &LibRef,
        pins: usize,
        symbol: &LibRef,
        except: Option<Uuid>,
    ) -> Vec<DrawnPart> {
        let mut out = self.parts_drawing(&|t| {
            same(&t.fndef, fndef) && t.cps.len() == pins && t.symbol.as_ref().is_none_or(|s| !same(s, symbol))
        });
        out.retain(|d| d.mine && Some(d.item) != except);
        out
    }

    /// Use it for every part with this piece…: each part of [`Session::parts_with_piece`] written into its library as
    /// its next version, its pieces of that function and pin count naming `symbol`; and the `library.update` that
    /// brings the project's copies of them to it, in one undo step (`None` when the project holds none of them).
    ///
    /// # Errors
    ///
    /// A library that cannot be written, in its words.
    pub fn use_symbol_for_pieces(
        &mut self,
        fndef: &LibRef,
        pins: usize,
        symbol: &LibRef,
        except: Option<Uuid>,
    ) -> Result<Option<UpdatePlan>, String> {
        let parts = self.parts_with_piece(fndef, pins, symbol, except);
        let mut held: Vec<LibDocument> = Vec::new();
        for d in &parts {
            let Some(library) = d.library else { continue };
            let current = self
                .libraries()
                .documents(library, Kind::Part)
                .into_iter()
                .find_map(|doc| match doc {
                    LibDocument::Part(c) if c.item_id.as_uuid() == d.item => Some(c),
                    _ => None,
                });
            let Some(c) = current else { continue };
            let mut item = c.item.clone();
            for i in &d.entries {
                if let Some(t) = item.function_template.get_mut(*i) {
                    t.symbol = Some(symbol.clone());
                }
            }
            let version = ew_schema::next_patch(&c.version).ok_or_else(|| c.version.clone())?;
            let copy = crate::part_new::finish(LibCopy { version, item, ..c }, LibDocument::Part);
            let doc = LibDocument::Part(copy);
            self.libraries_mut()
                .write(library, std::slice::from_ref(&doc))
                .map_err(|e| e.to_string())?;
            if d.held {
                held.push(doc);
            }
        }
        if held.is_empty() {
            return Ok(None);
        }
        self.update_from_documents(&held, &[])
            .map(Some)
            .map_err(|e| e.to_string())
    }
}
