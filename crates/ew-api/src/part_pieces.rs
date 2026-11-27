// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! On the schematic, as tables of the sheet (doc 07 §5.4): a part drawn with symbols is drawn as its **pieces** (a
//! coil, a main contact), each what it is (a starter function) with its pins, and a terminal as its **levels**, each
//! with its left and right clamp. The tables make the part's pins (the Connections table follows them) and, on Save,
//! its function template: a piece's row is a template entry of its starter function, a terminal's levels one entry of
//! the starter terminal with every pin, each pin at its level. What a part is drawn with is read by placing it
//! (`Feature::Placing`); the symbol a function is drawn with stays the starter's until the symbol has its place.

use ew_l10n::Text;
use ew_mltext::Ident;
use ew_schema::{Part, PartKind, TemplateFn};

use crate::Session;
use crate::part_box::PinRow;
use crate::part_editor::{PartDraftError, PartSheet, message};

/// A piece of a part drawn with symbols: what it is, as a starter function's key (`ew.fn.coil`), and its pins' names
/// as typed (`A1, A2`).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PieceRow {
    /// The starter function's key; empty until chosen.
    pub what: String,
    /// The pins' names, joined by commas.
    pub pins: String,
}

/// A terminal's level: the names of its left and its right clamp.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct LevelRow {
    /// The left clamp's pin.
    pub left: String,
    /// The right clamp's pin.
    pub right: String,
}

/// What a piece can be: a starter function by its key, in words.
#[derive(Clone, Debug)]
pub struct PieceKind {
    /// The starter function's key.
    pub key: &'static str,
    /// Its words (`Coil A1-A2`, `NO contact 13-14`).
    pub words: Text,
}

/// The most levels a terminal is made with here.
pub const MOST_LEVELS: usize = 3;

/// What a piece of a part can be: every starter function a piece is drawn with, in the starter's order.
pub fn piece_kinds() -> Vec<PieceKind> {
    crate::output::starter_piece_fndefs()
        .filter_map(|(key, c)| {
            let cps: Vec<String> = c
                .item
                .cps
                .iter()
                .map(|cp| cp.designation.as_str().to_owned())
                .collect();
            crate::output::navigators::kind_text(&c.item, &cps).map(|words| PieceKind { key, words })
        })
        .collect()
}

/// The name of the starter symbol that draws the piece `key` on a schematic page, in language `lang` (`en`).
pub fn piece_symbol(key: &str, lang: &str) -> Option<String> {
    let def = crate::output::starter_fndef_by_key(key)?;
    crate::output::starter_symbol_name(def.id, lang)
}

/// The names of `p`'s pins with the indexes `cps`, joined by commas.
fn names(p: &Part, cps: &[u16]) -> String {
    cps.iter()
        .filter_map(|i| p.cp_pattern.iter().find(|c| c.idx == *i))
        .map(|c| c.designation.as_str().to_owned())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The pieces part `p` is drawn as: each entry of its template but a box's, by its function's key.
pub(crate) fn pieces_of(p: &Part) -> Vec<PieceRow> {
    p.function_template
        .iter()
        .filter_map(|t| {
            let key = t.fndef.extra.get("key").and_then(|v| v.as_str())?;
            (key != ew_schema::starter::FN_BLACKBOX).then(|| PieceRow {
                what: key.to_owned(),
                pins: names(p, &t.cps),
            })
        })
        .collect()
}

/// The levels of terminal `p`: its pins by their level (a pin without one at the first), the first two of each.
pub(crate) fn levels_of(p: &Part) -> Vec<LevelRow> {
    if p.kind != PartKind::Terminal {
        return Vec::new();
    }
    let mut by_level: std::collections::BTreeMap<u8, Vec<String>> = std::collections::BTreeMap::new();
    for c in p.cp_pattern.iter() {
        by_level
            .entry(c.level.unwrap_or(1))
            .or_default()
            .push(c.designation.as_str().to_owned());
    }
    by_level
        .into_values()
        .map(|n| LevelRow {
            left: n.first().cloned().unwrap_or_default(),
            right: n.get(1).cloned().unwrap_or_default(),
        })
        .collect()
}

/// The names a pins' text gives (`A1, A2`): each trimmed, the empty ones left out.
fn split(text: &str) -> impl Iterator<Item = &str> {
    text.split([',', ' ']).map(str::trim).filter(|n| !n.is_empty())
}

impl PartSheet {
    /// Whether On the schematic asks for the part's pieces: a device drawn with symbols (its start, or the pieces it
    /// has), its pins not given by its channels or its ports.
    pub fn drawn_as_pieces(&self) -> bool {
        self.text("kind") == "device"
            && self.pins_from().is_none()
            && self.start.map_or(!self.pieces.is_empty(), |s| s.pieces)
    }

    /// Whether On the schematic asks for a terminal's levels.
    pub fn drawn_as_levels(&self) -> bool {
        self.text("kind") == "terminal"
    }

    /// The pins the pieces or the levels name, in their order, each once.
    fn named_pins(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut take = |n: &str| {
            if !out.iter().any(|o| o == n) {
                out.push(n.to_owned());
            }
        };
        if self.drawn_as_levels() {
            for l in &self.levels {
                for n in split(&l.left).chain(split(&l.right)) {
                    take(n);
                }
            }
        } else {
            for p in &self.pieces {
                for n in split(&p.pins) {
                    take(n);
                }
            }
        }
        out
    }

    /// The pins follow the pieces or the levels: a pin a row names is kept with its index and what it carries, a new
    /// name is a new pin, a pin no row names any more goes; the Connections table follows the pins.
    fn pins_from_rows(&mut self) {
        let named = self.named_pins();
        let mut next = self.pins.iter().map(|p| p.idx + 1).max().unwrap_or(0);
        let pins = named
            .into_iter()
            .map(|name| match self.pins.iter().find(|p| p.name == name) {
                Some(p) => p.clone(),
                None => {
                    next += 1;
                    PinRow {
                        idx: next - 1,
                        name,
                        role: String::new(),
                    }
                }
            })
            .collect();
        self.pins = pins;
        self.pins_to_table();
    }

    /// Adds an empty piece.
    pub fn add_piece(&mut self) {
        self.pieces.push(PieceRow::default());
    }

    /// Removes piece `i` (0-based) and the pins only it named; whether it had one.
    pub fn remove_piece(&mut self, i: usize) -> bool {
        if i >= self.pieces.len() {
            return false;
        }
        self.pieces.remove(i);
        self.pins_from_rows();
        true
    }

    /// A terminal of `n` levels (1 to [`MOST_LEVELS`]): a new level's clamps take the next free numbers.
    fn set_levels(&mut self, n: usize) {
        self.levels.truncate(n);
        while self.levels.len() < n {
            let taken = self.named_pins();
            let mut free = (1..).map(|k: u32| k.to_string()).filter(|k| !taken.contains(k));
            let left = free.next().unwrap_or_default();
            let right = free.next().unwrap_or_default();
            self.levels.push(LevelRow { left, right });
        }
        self.pins_from_rows();
    }
}

impl Session {
    /// A field of the pieces or the levels typed into the sheet: `pieces[i].what` (a starter function's key),
    /// `pieces[i].pins` (names joined by commas), `levels` (how many), `levels[i].left` and `levels[i].right`. `None`
    /// for another field.
    pub(crate) fn set_piece_field(
        &self,
        draft: &mut PartSheet,
        field: &str,
        text: &str,
    ) -> Option<Result<(), PartDraftError>> {
        let t = text.trim();
        if !(draft.new || draft.own)
            && (field == "levels" || field.starts_with("levels[") || field.starts_with("pieces["))
        {
            return Some(Err(PartDraftError::Refused(key!("error-part-field"))));
        }
        let bad = |why: &str| PartDraftError::Field(message(field, key!("error-part-value"), &[why]));
        let names_ok = |s: &str| -> Result<(), String> {
            split(s).try_for_each(|n| Ident::from_input(n).map(|_| ()).map_err(|e| e.to_string()))
        };
        let result = if field == "levels" {
            match t.parse::<usize>() {
                Ok(n @ 1..=MOST_LEVELS) => {
                    draft.set_levels(n);
                    Ok(())
                }
                _ => Err(bad(t)),
            }
        } else if let Some(rest) = field.strip_prefix("pieces[") {
            let (i, member) = rest.split_once("].")?;
            let i: usize = i.parse().ok()?;
            let row = draft.pieces.get_mut(i)?;
            match member {
                "what" if t.is_empty() || piece_kinds().iter().any(|k| k.key == t) => {
                    t.clone_into(&mut row.what);
                    Ok(())
                }
                "what" => Err(bad(t)),
                "pins" => match names_ok(t) {
                    Ok(()) => {
                        text.clone_into(&mut row.pins);
                        draft.pins_from_rows();
                        Ok(())
                    }
                    Err(why) => Err(bad(&why)),
                },
                _ => return None,
            }
        } else {
            let rest = field.strip_prefix("levels[")?;
            let (i, member) = rest.split_once("].")?;
            let i: usize = i.parse().ok()?;
            if let Err(why) = names_ok(t) {
                return Some(Err(bad(&why)));
            }
            let row = draft.levels.get_mut(i)?;
            match member {
                "left" => t.clone_into(&mut row.left),
                "right" => t.clone_into(&mut row.right),
                _ => return None,
            }
            draft.pins_from_rows();
            Ok(())
        };
        draft.messages.retain(|m| m.field != field);
        if let Err(PartDraftError::Field(m)) = &result {
            draft.messages.push(m.clone());
        }
        Some(result)
    }

    /// The template the pieces or the levels make, on Save: a piece's row is an entry of its starter function with
    /// its pins (the first the main one), a terminal's levels one entry of the starter terminal with every pin, each
    /// pin at its level. A sheet whose rows are as opened leaves the template as it was.
    pub(crate) fn apply_pieces(&self, draft: &PartSheet, p: &mut Part) -> Result<(), PartDraftError> {
        let idx_of = |p: &Part, name: &str| {
            p.cp_pattern
                .iter()
                .find(|c| c.designation.as_str() == name)
                .map(|c| c.idx)
        };
        if draft.drawn_as_levels() && !draft.levels.is_empty() && draft.levels != levels_of(&draft.original) {
            let def = crate::output::starter_fndef_by_key(ew_schema::starter::FN_TERMINAL)
                .ok_or(PartDraftError::Refused(key!("error-part-field")))?;
            let mut cps = Vec::new();
            for (level, row) in draft.levels.iter().enumerate() {
                for name in split(&row.left).chain(split(&row.right)) {
                    if let Some(idx) = idx_of(p, name) {
                        cps.push(idx);
                        if let Some(mut c) = p.cp_pattern.iter().find(|c| c.idx == idx).cloned() {
                            c.level = Some(u8::try_from(level + 1).unwrap_or(u8::MAX));
                            p.cp_pattern.insert(c);
                        }
                    }
                }
            }
            p.function_template = vec![TemplateFn {
                fndef: crate::part_new::fndef_ref(def),
                main: true,
                cps,
                extra: Default::default(),
            }];
            if p.category.is_none() {
                p.category = Some(def.item.category.clone());
            }
        } else if draft.drawn_as_pieces() && draft.pieces != pieces_of(&draft.original) {
            let mut template = Vec::new();
            for (i, row) in draft.pieces.iter().enumerate() {
                if row.what.is_empty() {
                    continue;
                }
                let def = crate::output::starter_fndef_by_key(&row.what).ok_or_else(|| {
                    PartDraftError::Field(message(
                        &format!("pieces[{i}].what"),
                        key!("error-part-value"),
                        &[&row.what],
                    ))
                })?;
                template.push(TemplateFn {
                    fndef: crate::part_new::fndef_ref(def),
                    main: template.is_empty(),
                    cps: split(&row.pins).filter_map(|n| idx_of(p, n)).collect(),
                    extra: Default::default(),
                });
                if p.category.is_none() {
                    p.category = Some(def.item.category.clone());
                }
            }
            p.function_template = template;
        }
        Ok(())
    }
}
