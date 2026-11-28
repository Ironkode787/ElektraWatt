// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the symbol editor's side panels show: the outline of the symbol (its connection points, texts and shapes, and
//! what is left to do before it can be saved) and the Properties rows of the selection, or of the symbol itself with
//! nothing selected. `symbol.prop` sets a row, for every selected thing that has it.

use super::selection::{bounds, translate, value_mut, value_of};
use super::{Item, ROLES, SymbolEditor, name_words, role_words, text_style_words, words};
use crate::shell::ShellModel;
use crate::{key, strings};
use ew_api::symbols;
use serde_json::{Map, Value, json};

/// What a row of Properties holds; `None` inside is a value that differs among the selected things.
#[derive(Clone, PartialEq, Debug)]
pub enum RowValue {
    /// A text of one line.
    Text(Option<String>),
    /// A text that may hold line breaks.
    Lines(Option<String>),
    /// A length in millimetres.
    Mm(Option<f64>),
    /// An angle in degrees.
    Degrees(Option<f64>),
    /// One of `options`, each as (the word `symbol.prop` takes, its text).
    Choice {
        /// The chosen word.
        value: Option<String>,
        /// The words to choose from.
        options: Vec<(String, String)>,
    },
    /// On or off.
    Flag(Option<bool>),
}

impl RowValue {
    fn varies(&mut self) {
        match self {
            RowValue::Text(v) | RowValue::Lines(v) => *v = None,
            RowValue::Mm(v) | RowValue::Degrees(v) => *v = None,
            RowValue::Choice { value, .. } => *value = None,
            RowValue::Flag(v) => *v = None,
        }
    }
}

/// One row of the editor's Properties.
#[derive(Clone, PartialEq, Debug)]
pub struct SheetRow {
    /// The property `symbol.prop` sets.
    pub key: &'static str,
    /// The row's label.
    pub label: String,
    /// Its value.
    pub value: RowValue,
    /// Whether the row stands under More: a row only a rare case needs.
    pub more: bool,
    /// What the row is for, in a sentence (its tooltip).
    pub help: Option<String>,
    /// Why the value cannot be changed; `None` when it can.
    pub locked: Option<String>,
}

/// The editor's Properties: what is selected in words, and its rows.
#[derive(Clone, PartialEq, Debug)]
pub struct Sheet {
    /// What the rows belong to ("Connection point 1", "3 things", the symbol's name).
    pub title: String,
    /// What that is, in a line.
    pub line: String,
    /// The rows, the More rows last.
    pub rows: Vec<SheetRow>,
}

/// One thing of the outline.
#[derive(Clone, PartialEq, Debug)]
pub struct OutlineRow {
    /// The thing a click selects.
    pub item: Item,
    /// Its name.
    pub label: String,
    /// What it is for, beside the name.
    pub detail: String,
}

/// One list of the outline.
#[derive(Clone, PartialEq, Debug)]
pub struct OutlineGroup {
    /// The list: `cps`, `texts`, `shapes` or `marks`.
    pub key: &'static str,
    /// Its title.
    pub label: String,
    /// What it lists.
    pub rows: Vec<OutlineRow>,
    /// What it says while it lists nothing.
    pub empty: String,
}

/// One thing that keeps the symbol from being saved or used.
#[derive(Clone, PartialEq, Debug)]
pub struct Todo {
    /// What to do, in a sentence.
    pub text: String,
    /// The thing it is about, which a click selects; `None` for the symbol itself.
    pub item: Option<Item>,
}

/// The symbol as the left panel lists it.
#[derive(Clone, PartialEq, Debug)]
pub struct Outline {
    /// The symbol's name.
    pub title: String,
    /// What it is, in a panel builder's words.
    pub what: String,
    /// The lists: connection points, texts, shapes, and other marks when it has any.
    pub groups: Vec<OutlineGroup>,
    /// What keeps the symbol from being saved; empty: ready to save.
    pub todo: Vec<Todo>,
    /// The other drawings of an imported file, by name; a click replaces the draft with one.
    pub candidates: Vec<String>,
    /// The import's report, and whether the person has checked it; empty for a drawn symbol.
    pub import_notes: Vec<String>,
    /// Whether the import and its source terms were checked.
    pub import_reviewed: bool,
}

/// What it is's word for a device box with pins of its own: the symbol keeps a function of its own, its pins as drawn.
pub const BOX: &str = "box";

/// What it is's list: each kind as (its word, its words).
type Kinds = Vec<(String, String)>;

/// What a symbol can stand for, as the What it is list offers it, each as (its word, its words): a device box with pins
/// of its own ([`BOX`], what a first symbol of a fluid or bus page is), then every starter function with pins by its key
/// (`ew.fn.coil`), from the functions' one home ([`ew_api::part_pieces::symbol_kinds`], which the pieces table reads
/// too), whatever the discipline: a function chosen is the one the symbol draws, its pins standing for the function's.
pub fn what_kinds() -> Vec<(String, String)> {
    // Built once per language (every frame of the drawing reads it): the starter's functions do not change.
    static KINDS: std::sync::Mutex<Option<(u64, Kinds)>> = std::sync::Mutex::new(None);
    let generation = strings::generation();
    let mut kinds = KINDS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((g, list)) = kinds.as_ref()
        && *g == generation
    {
        return list.clone();
    }
    let mut out = vec![(BOX.to_owned(), label("symbol-what-box"))];
    out.extend(
        (ew_api::part_pieces::symbol_kinds().into_iter())
            .map(|k| (k.key.to_owned(), strings::render(&k.words))),
    );
    // After the box, by their words in the window's language.
    out[1..].sort_by(|a, b| a.1.cmp(&b.1));
    *kinds = Some((generation, out.clone()));
    out
}

fn label(key: &str) -> String {
    strings::text(strings::Key::run_time(key)).to_owned()
}

fn row(key: &'static str, label_key: &str, value: RowValue) -> SheetRow {
    SheetRow {
        key,
        label: label(label_key),
        value,
        more: false,
        help: strings::lookup(&format!("{label_key}-help")).map(str::to_owned),
        locked: None,
    }
}

fn choice(value: &str, options: Vec<(String, String)>) -> RowValue {
    RowValue::Choice {
        value: Some(value.to_owned()),
        options,
    }
}

fn mm(v: &Value) -> RowValue {
    RowValue::Mm(v.as_i64().map(|n| n as f64 / 1e6))
}

fn degrees(v: &Value) -> RowValue {
    RowValue::Degrees(Some(v.as_f64().unwrap_or(0.0) / 1000.0))
}

/// The text of a multilingual value in the window's language, else its English, else its first.
fn ml_words(v: &Value) -> String {
    serde_json::from_value::<symbols::MlText>(v.clone())
        .map(|t| name_words(&t))
        .unwrap_or_default()
}

/// Sets the text of a multilingual value, keeping the form its author gave it: a neutral text stays neutral, a text
/// in languages takes `text` in the window's language when it has it, else in its English, else in its first.
fn set_ml(v: &mut Value, text: &str) {
    let Ok(mut typed) = serde_json::from_value::<symbols::MlText>(v.clone()) else {
        *v = json!({ "en": text });
        return;
    };
    match typed.view() {
        symbols::MlTextView::Neutral(_) => typed = symbols::MlText::neutral(text),
        symbols::MlTextView::Langs(map) => {
            let language = strings::locale().split('-').next().unwrap_or("en");
            let tag = map
                .keys()
                .find(|k| k.as_str() == language)
                .or_else(|| map.keys().find(|k| k.as_str() == "en"))
                .or_else(|| map.keys().next())
                .cloned();
            match tag {
                Some(tag) => typed.set_lang(tag, text),
                None => typed = symbols::MlText::neutral(text),
            }
        }
    }
    *v = serde_json::to_value(typed).unwrap_or_default();
}

/// The twelve alignments of a text by (horizontal, vertical) position: left, centre, right; top, middle, bottom and the
/// first line's baseline (doc 09 §4.8).
const ALIGNS: [[&str; 4]; 3] = [
    ["top_left", "middle_left", "bottom_left", "baseline_left"],
    ["top_centre", "centre", "bottom_centre", "baseline_centre"],
    ["top_right", "middle_right", "bottom_right", "baseline_right"],
];
const ALIGN_H: [&str; 3] = ["left", "centre", "right"];
const ALIGN_V: [&str; 4] = ["top", "middle", "bottom", "baseline"];

/// The (horizontal, vertical) position of an alignment word; a word of a newer format reads as top left.
fn align_position(word: &str) -> (usize, usize) {
    (0..3)
        .flat_map(|h| (0..4).map(move |v| (h, v)))
        .find(|(h, v)| ALIGNS[*h][*v] == word)
        .unwrap_or((0, 0))
}

fn align_rows(v: &Value) -> [SheetRow; 2] {
    let (h, vertical) = align_position(v["align"].as_str().unwrap_or("top_left"));
    let options = |words: &[&str]| {
        words
            .iter()
            .map(|w| ((*w).to_owned(), label(&format!("symbol-align-{w}"))))
            .collect()
    };
    [
        row(
            "align_h",
            "symbol-align-horizontal",
            choice(ALIGN_H[h], options(&ALIGN_H)),
        ),
        row(
            "align_v",
            "symbol-align-vertical",
            choice(ALIGN_V[vertical], options(&ALIGN_V)),
        ),
    ]
}

fn set_align(v: &mut Value, horizontal: bool, word: &str) -> bool {
    let (mut h, mut vertical) = align_position(v["align"].as_str().unwrap_or("top_left"));
    let words: &[&str] = if horizontal { &ALIGN_H } else { &ALIGN_V };
    let Some(at) = words.iter().position(|w| *w == word) else {
        return false;
    };
    if horizontal {
        h = at;
    } else {
        vertical = at;
    }
    v["align"] = json!(ALIGNS[h][vertical]);
    true
}

/// The point an item is placed by: a point's own, a line's start, a box's top-left corner, a circle's centre, the
/// top-left of a path's bounds.
fn reference(v: &Value) -> Option<[i64; 2]> {
    let point = |v: &Value| Some([v.get(0)?.as_i64()?, v.get(1)?.as_i64()?]);
    if v.is_array() {
        return point(v);
    }
    if let Some(pos) = v.get("pos") {
        return point(pos);
    }
    let (kind, shape) = v.as_object()?.iter().next()?;
    match kind.as_str() {
        "line" => point(&shape["from"]),
        "circle" => point(&shape["centre"]),
        "text" => point(&shape["pos"]),
        _ => bounds(v).map(|b| [b[0], b[1]]),
    }
}

/// The kind of a graphic (`line`, `rect`, `text`, …) and its members.
fn graphic(v: &Value) -> Option<(&str, &Value)> {
    v.as_object()?.iter().next().map(|(k, v)| (k.as_str(), v))
}

/// The words of a graphic's kind ("Rectangle").
fn graphic_words(kind: &str) -> String {
    label(&format!(
        "symbol-{}",
        if kind == "rect" { "rectangle" } else { kind }
    ))
}

impl SymbolEditor {
    /// The designation of the function pin connection point `cp` draws, else the point's own number.
    fn cp_name(&self, cp: &symbols::schema::SymbolCp) -> String {
        // A bus symbol's socket by its interface, and its port where the interface has several ("X1 P2", "DP").
        if let Some(interface) = &cp.interface {
            let mut words = interface.to_string();
            if let Some(label) = &cp.port_label {
                words = format!("{words} {label}");
            }
            let same =
                |o: &&symbols::schema::SymbolCp| o.interface == cp.interface && o.port_label == cp.port_label;
            if self.draft.symbol.cps.iter().filter(same).count() > 1 {
                words = format!("{words} P{}", u32::from(cp.fn_cp) + 1);
            }
            return words;
        }
        self.draft
            .function
            .cps
            .get(&cp.fn_cp)
            .map_or_else(|| cp.idx.to_string(), |p| p.designation.to_string())
    }

    /// The name of `item` as the outline and Properties print it.
    pub fn item_name(&self, item: Item) -> String {
        let s = &self.draft.symbol;
        let numbered = |words: String, i: usize| {
            strings::format(key!("symbol-graphic-item"), &[&words, &(i + 1).to_string()])
        };
        match item {
            Item::Cp(i) => s
                .cps
                .iter()
                .as_slice()
                .get(i)
                .map_or_else(String::new, |cp| self.cp_name(cp)),
            Item::Slot(i) => s
                .text_slots
                .get(i)
                .map_or_else(String::new, |slot| self.slot_words(&slot.kind)),
            Item::Graphic(i) => {
                let v = s
                    .graphics
                    .get(i)
                    .and_then(|g| serde_json::to_value(g).ok())
                    .unwrap_or_default();
                match graphic(&v) {
                    Some(("text", t)) => {
                        strings::format(key!("symbol-text-quoted"), &[&ml_words(&t["text"])])
                    }
                    Some((kind, _)) => numbered(graphic_words(kind), i),
                    None => String::new(),
                }
            }
            Item::Link(i) => s.link_points.get(i).map_or_else(String::new, |l| {
                strings::format(key!("symbol-link-named"), &[l.name.as_str()])
            }),
            Item::Anchor => label("symbol-anchor"),
        }
    }

    /// What a text field writes, in words: the device's tag, a connection point's number.
    fn slot_words(&self, kind: &symbols::schema::SlotKind) -> String {
        use symbols::schema::SlotKind;
        match kind {
            SlotKind::Tag => label("symbol-tag"),
            SlotKind::FunctionText => label("symbol-function-text"),
            SlotKind::TechData => label("symbol-tech-data"),
            SlotKind::Xref => label("symbol-xref"),
            SlotKind::CpDesignation(idx) => {
                let name = self
                    .draft
                    .symbol
                    .cps
                    .get(idx)
                    .map_or_else(|| idx.to_string(), |cp| self.cp_name(cp));
                strings::format(key!("symbol-slot-number-of"), &[&name])
            }
            _ => label("symbol-slot"),
        }
    }

    /// What a text field can write, each as (the word `symbol.prop` and the Text field tool take, its text): the
    /// device's tag, its function text, its technical data, its cross-reference, and the number of each connection
    /// point (`cp:<number>`).
    pub fn slot_kinds(&self) -> Vec<(String, String)> {
        let mut kinds: Vec<(String, String)> = ["tag", "function_text", "tech_data", "xref"]
            .iter()
            .map(|k| ((*k).to_owned(), label(&format!("symbol-{}", k.replace('_', "-")))))
            .collect();
        kinds.extend(self.draft.symbol.cps.iter().map(|cp| {
            (
                format!("cp:{}", cp.idx),
                strings::format(key!("symbol-slot-number-of"), &[&self.cp_name(cp)]),
            )
        }));
        kinds
    }

    /// What the symbol is, as its [`what_kinds`] word: the starter function it draws, else [`BOX`] for a box of its own
    /// function; `None` for a function of its own of another type, or another library's.
    pub fn what(&self) -> Option<String> {
        match &self.draft.function_ref {
            Some(r) => ew_api::symbols::starter_function_key(r).map(str::to_owned),
            None => matches!(
                self.draft.function.contact_kind,
                symbols::schema::ContactKind::BlackBox
            )
            .then(|| BOX.to_owned()),
        }
    }

    /// What the symbol is, in a panel builder's words: a part's bus symbol by its part ("Bus symbol of CPL-PN-DP"), its
    /// [`what_kinds`] words, else its function type's own.
    fn what_words(&self) -> String {
        if let super::Drawing::BusSymbol(b) = &self.purpose {
            return strings::format(key!("symbol-what-bus"), &[&b.part]);
        }
        let kind = self
            .what()
            .and_then(|w| what_kinds().into_iter().find(|(k, _)| *k == w));
        match kind {
            Some((_, words)) => words,
            None => {
                let kind = serde_json::to_value(&self.draft.function.contact_kind).unwrap_or_default();
                label(&format!("symbol-kind-{}", kind.as_str().unwrap_or("unknown")))
            }
        }
    }

    /// The symbol as the left panel lists it.
    pub fn outline(&self) -> Outline {
        let s = &self.draft.symbol;
        let doc = serde_json::to_value(&self.draft).unwrap_or_default();
        let rows = |items: Vec<Item>| -> Vec<OutlineRow> {
            let names: Vec<String> = items.iter().map(|item| self.item_name(*item)).collect();
            items
                .into_iter()
                .enumerate()
                .map(|(i, item)| {
                    // Two things of one name (two tags, two pins both named N) are told apart by their place.
                    let twins = names.iter().filter(|n| **n == names[i]).count() > 1;
                    let label = if twins {
                        let nth = names[..=i].iter().filter(|n| **n == names[i]).count();
                        strings::format(key!("symbol-pin-ordinal"), &[&names[i], &nth.to_string()])
                    } else {
                        names[i].clone()
                    };
                    OutlineRow {
                        item,
                        label,
                        detail: self.item_detail(&doc, item),
                    }
                })
                .collect()
        };
        let is_text = |i: &usize| matches!(s.graphics.get(*i), Some(symbols::schema::Primitive::Text(_)));
        let graphics = 0..s.graphics.len();
        let mut texts: Vec<Item> = (0..s.text_slots.len()).map(Item::Slot).collect();
        texts.extend(graphics.clone().filter(is_text).map(Item::Graphic));
        let mut marks: Vec<Item> = (0..s.link_points.len()).map(Item::Link).collect();
        if matches!(s.anchor, symbols::schema::AnchorPolicy::Named(_)) {
            marks.push(Item::Anchor);
        }
        let bus = match &self.purpose {
            super::Drawing::BusSymbol(b) => Some(b),
            _ => None,
        };
        let mut groups = vec![
            OutlineGroup {
                key: "cps",
                label: label(if bus.is_some() {
                    "symbol-sockets"
                } else {
                    "symbol-pins"
                }),
                rows: rows((0..s.cps.len()).map(Item::Cp).collect()),
                empty: strings::text(key!("symbol-outline-empty-cps")).to_owned(),
            },
            OutlineGroup {
                key: "texts",
                label: label("symbol-texts"),
                rows: rows(texts),
                empty: strings::text(key!("symbol-outline-empty-texts")).to_owned(),
            },
            OutlineGroup {
                key: "shapes",
                label: label("symbol-shapes"),
                rows: rows(graphics.filter(|i| !is_text(i)).map(Item::Graphic).collect()),
                empty: strings::text(key!("symbol-outline-empty-shapes")).to_owned(),
            },
        ];
        if !marks.is_empty() {
            groups.push(OutlineGroup {
                key: "marks",
                label: label("symbol-marks"),
                rows: rows(marks),
                empty: strings::text(key!("symbol-outline-empty-marks")).to_owned(),
            });
        }
        let mut todo: Vec<Todo> = self
            .draft
            .validate()
            .iter()
            .map(|i| Todo {
                text: strings::format(strings::Key::run_time(i.code), &[&i.detail]),
                item: Item::of_pointer(&i.path).or_else(|| {
                    // An issue of a function pin is shown at the connection point that draws it.
                    let pin = i.path.strip_prefix("/function/cps/")?.parse::<usize>().ok()?;
                    let idx = self.draft.function.cps.iter().as_slice().get(pin)?.idx;
                    s.cps.iter().position(|c| c.fn_cp == idx).map(Item::Cp)
                }),
            })
            .collect();
        // A symbol's key is made from its name: a new drawing has none until he gives one.
        if self.unnamed {
            todo.insert(
                0,
                Todo {
                    text: label("symbol-todo-name"),
                    item: None,
                },
            );
        }
        if !self.draft.import_notes.is_empty() && !self.draft.import_reviewed {
            todo.push(Todo {
                text: label("symbol-import-review"),
                item: None,
            });
        }
        // His own bus symbol draws a socket for every network port of its part, and only those (doc 10 §2.7).
        if let Some(b) = bus {
            let (missing, extra) = ew_api::socket_mismatch(s, &b.ports);
            todo.extend(missing.iter().map(|m| Todo {
                text: strings::format(key!("symbol-todo-socket"), &[m]),
                item: None,
            }));
            todo.extend(extra.iter().map(|m| Todo {
                text: strings::format(key!("symbol-todo-socket-extra"), &[m]),
                item: None,
            }));
            if !ew_api::texts_clear(s) {
                todo.push(Todo {
                    text: label("symbol-bus-text-outside"),
                    item: None,
                });
            }
        }
        let mut import_notes: Vec<String> = self.draft.import_notes.iter().map(|n| label(n)).collect();
        import_notes.extend(self.draft.import_details.iter().map(|d| d.message.clone()));
        Outline {
            title: name_words(&s.name),
            what: strings::format(
                key!("symbol-what-line"),
                &[&self.what_words(), &strings::choice_text(self.discipline())],
            ),
            groups,
            todo,
            candidates: if self.candidates.len() > 1 {
                self.candidates
                    .iter()
                    .map(|d| name_words(&d.symbol.name))
                    .collect()
            } else {
                Vec::new()
            },
            import_notes,
            import_reviewed: self.draft.import_reviewed,
        }
    }

    /// What an outline row says beside the name: a connection point's potential, a shape's size.
    fn item_detail(&self, doc: &Value, item: Item) -> String {
        let side = |n: f64| strings::decimal(&trim_mm(n / 1e6));
        let size = |b: [i64; 4]| {
            strings::format(
                key!("symbol-size-mm"),
                &[&side((b[2] - b[0]) as f64), &side((b[3] - b[1]) as f64)],
            )
        };
        match item {
            // What the point carries, else the side its wire leaves on.
            Item::Cp(i) => self
                .draft
                .symbol
                .cps
                .iter()
                .as_slice()
                .get(i)
                .map(|cp| {
                    self.draft
                        .function
                        .cps
                        .get(&cp.fn_cp)
                        .and_then(|pin| serde_json::to_value(&pin.potential_role).ok())
                        .and_then(|v| v.as_str().map(role_words))
                        .unwrap_or_else(|| {
                            let dir = serde_json::to_value(&cp.dir).unwrap_or_default();
                            label(&format!("symbol-{}", dir.as_str().unwrap_or("direction")))
                        })
                })
                .unwrap_or_default(),
            // A line by its length, another shape by its size.
            Item::Graphic(_) => match value_of(doc, item).and_then(|v| Some((graphic(v)?.0, bounds(v)?))) {
                Some(("text", _)) | None => String::new(),
                Some(("line", b)) => strings::format(
                    key!("symbol-length-mm"),
                    &[&side(((b[2] - b[0]) as f64).hypot((b[3] - b[1]) as f64))],
                ),
                Some((_, b)) => size(b),
            },
            Item::Slot(_) | Item::Link(_) | Item::Anchor => String::new(),
        }
    }

    /// The rows of one thing.
    fn item_rows(&self, doc: &Value, item: Item, styles: &symbols::TextStyleTable) -> Vec<SheetRow> {
        let Some(v) = value_of(doc, item) else {
            return Vec::new();
        };
        let at = reference(v).unwrap_or([0, 0]);
        let place = |x: &str, y: &str| {
            [
                SheetRow {
                    label: x.to_owned(),
                    ..row("x", "symbol-x-mm", RowValue::Mm(Some(at[0] as f64 / 1e6)))
                },
                SheetRow {
                    label: y.to_owned(),
                    ..row("y", "symbol-y-mm", RowValue::Mm(Some(at[1] as f64 / 1e6)))
                },
            ]
        };
        let plain = || place(&label("symbol-x-mm"), &label("symbol-y-mm"));
        let style_row = |v: &Value| {
            let name = |key: &str| {
                let words = text_style_words(key);
                match styles.rows().iter().find(|r| r.key == key) {
                    Some(r) => strings::format(
                        key!("symbol-style-choice"),
                        &[
                            &words,
                            &strings::decimal(&format!("{:.2}", r.em.0 as f64 / 1_000_000.0)),
                        ],
                    ),
                    None => words,
                }
            };
            let mut options = vec![(String::new(), label("symbol-style-default"))];
            options.extend(styles.rows().iter().map(|r| (r.key.clone(), name(&r.key))));
            row(
                "style",
                "symbol-text-style",
                choice(v["style"].as_str().unwrap_or(""), options),
            )
        };
        let locked = self
            .draft
            .function_ref
            .is_some()
            .then(|| label("symbol-function-locked"));
        let mut rows = Vec::new();
        match item {
            Item::Cp(i) => {
                let Some(cp) = self.draft.symbol.cps.iter().as_slice().get(i) else {
                    return rows;
                };
                let pin = self.draft.function.cps.get(&cp.fn_cp);
                let pin_json = pin.and_then(|p| serde_json::to_value(p).ok()).unwrap_or_default();
                rows.push(SheetRow {
                    locked: locked.clone(),
                    ..row(
                        "name",
                        "symbol-designation",
                        RowValue::Text(Some(self.cp_name(cp))),
                    )
                });
                let mut roles = vec![("none".to_owned(), role_words(""))];
                roles.extend(ROLES.iter().map(|r| ((*r).to_owned(), role_words(r))));
                rows.push(SheetRow {
                    locked: locked.clone(),
                    ..row(
                        "role",
                        "symbol-potential-role",
                        choice(pin_json["potential_role"].as_str().unwrap_or("none"), roles),
                    )
                });
                let directions = ["up", "right", "down", "left"]
                    .iter()
                    .map(|d| ((*d).to_owned(), label(&format!("symbol-{d}"))))
                    .collect();
                rows.push(row(
                    "dir",
                    "symbol-direction",
                    choice(v["dir"].as_str().unwrap_or(""), directions),
                ));
                rows.extend(plain());
                let pins = self
                    .draft
                    .function
                    .cps
                    .iter()
                    .filter(|p| p.physical && p.discipline == cp.discipline)
                    .map(|p| (p.idx.to_string(), self.function_pin_name(p.idx)))
                    .collect();
                rows.push(SheetRow {
                    more: true,
                    ..row(
                        "pin",
                        "symbol-function-pin-row",
                        choice(&cp.fn_cp.to_string(), pins),
                    )
                });
                for (key, member, text) in [
                    ("source_output", "source_output", "symbol-source-output"),
                    ("optional", "optional", "symbol-optional-pin"),
                ] {
                    rows.push(SheetRow {
                        more: true,
                        locked: locked.clone(),
                        ..row(
                            key,
                            text,
                            RowValue::Flag(Some(pin_json[member].as_bool().unwrap_or(false))),
                        )
                    });
                }
                let disciplines = super::DISCIPLINES
                    .iter()
                    .map(|d| ((*d).to_owned(), strings::choice_text(d)))
                    .collect();
                rows.push(SheetRow {
                    more: true,
                    locked,
                    ..row(
                        "discipline",
                        "symbol-discipline",
                        choice(v["discipline"].as_str().unwrap_or("electrical"), disciplines),
                    )
                });
            }
            Item::Slot(_) => {
                let current = match &v["kind"] {
                    Value::String(s) => s.clone(),
                    kind => kind
                        .get("cp_designation")
                        .and_then(Value::as_u64)
                        .map_or_else(String::new, |n| format!("cp:{n}")),
                };
                rows.push(row(
                    "kind",
                    "symbol-slot-kind",
                    choice(&current, self.slot_kinds()),
                ));
                rows.push(style_row(v));
                rows.extend(plain());
                rows.extend(align_rows(v));
                rows.push(row("rot", "symbol-rotation", degrees(&v["rot"])));
                rows.push(row(
                    "visible",
                    "symbol-visible",
                    RowValue::Flag(Some(v["visible"].as_bool().unwrap_or(false))),
                ));
            }
            Item::Link(_) => {
                rows.push(row(
                    "name",
                    "symbol-name",
                    RowValue::Text(Some(v["name"].as_str().unwrap_or_default().to_owned())),
                ));
                rows.extend(plain());
            }
            Item::Anchor => rows.extend(plain()),
            Item::Graphic(_) => {
                let Some((kind, shape)) = graphic(v) else {
                    return rows;
                };
                let axis =
                    |part: &str, axis: &str| strings::format(key!("symbol-axis-mm"), &[&label(part), axis]);
                match kind {
                    "line" => {
                        rows.extend(place(&label("symbol-start-x"), &label("symbol-start-y")));
                        rows.push(row("x2", "symbol-end-x", mm(&shape["to"][0])));
                        rows.push(row("y2", "symbol-end-y", mm(&shape["to"][1])));
                    }
                    "circle" => {
                        rows.extend(place(
                            &axis("symbol-part-centre", "x"),
                            &axis("symbol-part-centre", "y"),
                        ));
                        rows.push(SheetRow {
                            label: strings::format(key!("symbol-mm"), &[&label("symbol-part-radius")]),
                            ..row("r", "symbol-part-radius", mm(&shape["r"]))
                        });
                    }
                    "text" => {
                        rows.push(row(
                            "text",
                            "symbol-text",
                            RowValue::Lines(Some(ml_words(&shape["text"]))),
                        ));
                        rows.push(style_row(shape));
                        rows.extend(plain());
                        rows.extend(align_rows(shape));
                        rows.push(row("rot", "symbol-rotation", degrees(&shape["angle"])));
                    }
                    "polyline" | "polygon" => rows.extend(plain()),
                    _ => {
                        rows.extend(plain());
                        let b = bounds(v).unwrap_or([0; 4]);
                        rows.push(row("w", "symbol-width-mm", mm(&json!(b[2] - b[0]))));
                        rows.push(row("h", "symbol-height-mm", mm(&json!(b[3] - b[1]))));
                    }
                }
                if kind == "arc" {
                    rows.push(row("start", "symbol-arc-start", degrees(&shape["start"])));
                    rows.push(row("sweep", "symbol-arc-sweep", degrees(&shape["sweep"])));
                }
                if matches!(kind, "rect" | "circle" | "ellipse" | "polygon") {
                    rows.push(row(
                        "fill",
                        "symbol-fill",
                        RowValue::Flag(Some(!shape["fill"].is_null())),
                    ));
                }
            }
        }
        rows
    }

    /// The name of function pin `idx`: its designation, with its place in the list when another pin has the same.
    fn function_pin_name(&self, idx: u16) -> String {
        let pins = &self.draft.function.cps;
        let Some(pin) = pins.get(&idx) else {
            return "—".to_owned();
        };
        if pins.iter().filter(|p| p.designation == pin.designation).count() > 1 {
            let ordinal = pins.iter().position(|p| p.idx == idx).unwrap_or_default() + 1;
            strings::format(
                key!("symbol-pin-ordinal"),
                &[pin.designation.as_str(), &ordinal.to_string()],
            )
        } else {
            pin.designation.to_string()
        }
    }

    /// The rows of the symbol itself, its short sheet (doc 07 §5.6): its name and what it is, in a panel builder's
    /// words; where it is listed and which function it stands for follow from what it is and are not asked.
    pub fn symbol_rows(&self) -> Vec<SheetRow> {
        let d = &self.draft;
        // Locked only for another library's function: a starter function is changed here.
        let mut locked =
            (d.function_ref.is_some() && self.what().is_none()).then(|| label("symbol-function-locked"));
        let mut what = what_kinds();
        // A part's bus symbol is that and nothing else: what it is names it and its part.
        if matches!(self.purpose, super::Drawing::BusSymbol(_)) {
            what = Vec::new();
            locked = Some(label("symbol-what-bus-locked"));
        }
        // A symbol of a function no entry is shows as what its type is, the list's last line.
        let current = match self.what().filter(|w| what.iter().any(|(k, _)| k == w)) {
            Some(w) => w,
            None => {
                what.push(("other".to_owned(), self.what_words()));
                "other".to_owned()
            }
        };
        let text = |s: &str| RowValue::Text(Some(s.to_owned()));
        vec![
            row(
                "name",
                "symbol-name",
                text(&if self.unnamed {
                    String::new()
                } else {
                    name_words(&d.symbol.name)
                }),
            ),
            SheetRow {
                locked,
                ..row("what", "symbol-what", choice(&current, what))
            },
        ]
    }

    /// The symbol's version and its terms, a group of the short sheet of their own under its things, closed until
    /// opened: the version Save gives it (read), the licence and the source he gives his own, which Make my own from it
    /// keeps from the source.
    pub fn terms_rows(&self) -> Vec<SheetRow> {
        let d = &self.draft;
        let text = |s: &str| RowValue::Text(Some(s.to_owned()));
        vec![
            SheetRow {
                locked: Some(label("symbol-version-given")),
                ..row("version", "symbol-version", text(&d.version))
            },
            row(
                "licence",
                "symbol-licence",
                text(d.symbol.meta.licence.as_deref().unwrap_or("")),
            ),
            row(
                "source",
                "symbol-source",
                text(d.symbol.meta.sources.first().map_or("", String::as_str)),
            ),
        ]
    }

    /// Where the symbol is listed, in words: the browsers' category of what it is.
    pub fn listed_under(&self) -> String {
        let f = serde_json::to_value(&self.draft.function).unwrap_or_default();
        crate::tools::category_words(f["category"][0].as_str().unwrap_or(""))
            .unwrap_or_default()
            .to_owned()
    }

    /// The drawing's Properties: the rows the selected things share; with nothing selected, none (the symbol's own
    /// stand on its short sheet).
    pub fn sheet(&self, styles: &symbols::TextStyleTable) -> Sheet {
        if self.selection.is_empty() {
            return Sheet {
                title: label("symbol-the-symbol"),
                line: label("symbol-nothing-selected"),
                rows: Vec::new(),
            };
        }
        let doc = serde_json::to_value(&self.draft).unwrap_or_default();
        let mut all = self
            .selection
            .iter()
            .map(|item| self.item_rows(&doc, *item, styles));
        let mut rows = all.next().unwrap_or_default();
        for other in all {
            rows.retain_mut(|r| {
                let Some(o) = other.iter().find(|o| o.key == r.key && o.label == r.label) else {
                    return false;
                };
                if o.value != r.value {
                    r.value.varies();
                }
                if r.locked.is_none() {
                    r.locked.clone_from(&o.locked);
                }
                true
            });
        }
        rows.sort_by_key(|r| r.more);
        let (title, line) = match self.selection[..] {
            // In the list a connection point is its designation; here it says what it is.
            [item @ Item::Cp(_)] => (
                strings::format(key!("symbol-mapping-target"), &[&self.item_name(item)]),
                self.kind_words(&doc, item),
            ),
            [item] => (self.item_name(item), self.kind_words(&doc, item)),
            _ => (
                strings::format(
                    key!("symbol-things-selected"),
                    &[&self.selection.len().to_string()],
                ),
                label("symbol-things-shared"),
            ),
        };
        Sheet { title, line, rows }
    }

    /// What `item` is, in a line under its name.
    fn kind_words(&self, doc: &Value, item: Item) -> String {
        match item {
            Item::Cp(_) => label("symbol-cp-line"),
            Item::Slot(_) => label("symbol-slot-line"),
            Item::Link(_) => label("symbol-link-line"),
            Item::Anchor => label("symbol-anchor-line"),
            Item::Graphic(_) => value_of(doc, item)
                .and_then(graphic)
                .map(|(kind, _)| {
                    label(if kind == "text" {
                        "symbol-text-line"
                    } else {
                        "symbol-shape-line"
                    })
                })
                .unwrap_or_default(),
        }
    }

    /// Sets property `key` of `item` in `doc`; whether the item has that property and `value` fits it.
    fn set_item(&self, doc: &mut Value, item: Item, key: &str, value: &Value) -> bool {
        let nm = |v: &Value| v.as_f64().map(|mm| (mm * 1e6).round() as i64);
        let milli = |v: &Value| v.as_f64().map(|d| (d * 1000.0).round() as i64);
        let own_function = self.draft.function_ref.is_none();
        let fn_cp = value_of(doc, item).map(|v| v["fn_cp"].clone());
        if let (Item::Cp(_), Some(fn_cp), "name" | "role" | "source_output" | "optional") =
            (item, &fn_cp, key)
        {
            if !own_function {
                return false;
            }
            let Some(pin) = doc["function"]["cps"]
                .as_array_mut()
                .and_then(|pins| pins.iter_mut().find(|p| p["idx"] == *fn_cp))
                .and_then(Value::as_object_mut)
            else {
                return false;
            };
            match (key, value) {
                ("name", Value::String(s)) if !s.trim().is_empty() => {
                    pin.insert("designation".into(), json!(s.trim()));
                }
                ("role", Value::String(s)) if s == "none" => {
                    pin.remove("potential_role");
                }
                ("role", Value::String(s)) if ROLES.contains(&s.as_str()) => {
                    pin.insert("potential_role".into(), json!(s));
                }
                ("source_output" | "optional", Value::Bool(on)) => {
                    pin.insert(key.into(), json!(on));
                }
                _ => return false,
            }
            return true;
        }
        let Some(v) = value_mut(doc, item) else {
            return false;
        };
        let at = reference(v).unwrap_or([0, 0]);
        match (key, value) {
            ("x", n) => nm(n).is_some_and(|x| {
                translate(v, x - at[0], 0);
                true
            }),
            ("y", n) => nm(n).is_some_and(|y| {
                translate(v, 0, y - at[1]);
                true
            }),
            ("dir", Value::String(s)) if matches!(item, Item::Cp(_)) => {
                v["dir"] = json!(s);
                true
            }
            ("name", Value::String(s)) if matches!(item, Item::Link(_)) => {
                v["name"] = json!(s);
                true
            }
            ("kind", Value::String(s)) if matches!(item, Item::Slot(_)) => {
                v["kind"] = match s.strip_prefix("cp:").map(str::parse::<u16>) {
                    Some(Ok(idx)) => json!({ "cp_designation": idx }),
                    Some(Err(_)) => return false,
                    None => json!(s),
                };
                true
            }
            ("visible", Value::Bool(on)) if matches!(item, Item::Slot(_)) => {
                v["visible"] = json!(on);
                true
            }
            _ => {
                // A text field's members are its own; a graphic's are its shape's.
                let (kind, target) = match item {
                    Item::Slot(_) => ("slot", v),
                    Item::Graphic(_) => match v.as_object_mut().and_then(|m| m.iter_mut().next()) {
                        Some((k, shape)) => (
                            [
                                "line", "circle", "text", "arc", "rect", "ellipse", "polygon", "image",
                            ]
                            .into_iter()
                            .find(|w| w == k)
                            .unwrap_or(""),
                            shape,
                        ),
                        None => return false,
                    },
                    _ => return false,
                };
                let texty = matches!(kind, "slot" | "text");
                match (key, value) {
                    ("style", Value::String(s)) if texty => {
                        if s.is_empty() {
                            if let Some(map) = target.as_object_mut() {
                                map.remove("style");
                            }
                        } else {
                            target["style"] = json!(s);
                        }
                        true
                    }
                    ("align_h", Value::String(s)) if texty => set_align(target, true, s),
                    ("align_v", Value::String(s)) if texty => set_align(target, false, s),
                    ("rot", n) if texty => milli(n).is_some_and(|a| {
                        target[if kind == "slot" { "rot" } else { "angle" }] = json!(a);
                        true
                    }),
                    ("text", Value::String(s)) if kind == "text" && !s.is_empty() => {
                        set_ml(&mut target["text"], s);
                        true
                    }
                    ("x2", n) if kind == "line" => nm(n).is_some_and(|x| {
                        target["to"][0] = json!(x);
                        true
                    }),
                    ("y2", n) if kind == "line" => nm(n).is_some_and(|y| {
                        target["to"][1] = json!(y);
                        true
                    }),
                    ("r", n) if kind == "circle" => nm(n).filter(|r| *r > 0).is_some_and(|r| {
                        target["r"] = json!(r);
                        true
                    }),
                    ("w" | "h", n) if matches!(kind, "rect" | "ellipse" | "arc" | "image") => {
                        let axis = usize::from(key == "h");
                        let from = target["rect"][0][axis].as_i64().unwrap_or(0);
                        nm(n).filter(|side| *side > 0).is_some_and(|side| {
                            target["rect"][1][axis] = json!(from + side);
                            true
                        })
                    }
                    ("start" | "sweep", n) if kind == "arc" => milli(n)
                        .filter(|a| (-360_000..=360_000).contains(a))
                        .is_some_and(|a| {
                            target[key] = json!(a);
                            true
                        }),
                    ("fill", Value::Bool(on))
                        if matches!(kind, "rect" | "circle" | "ellipse" | "polygon") =>
                    {
                        if *on {
                            target["fill"] = json!("by_layer");
                        } else if let Some(map) = target.as_object_mut() {
                            map.remove("fill");
                        }
                        true
                    }
                    _ => false,
                }
            }
        }
    }

    /// Sets field `key` of the symbol's short sheet in `doc` (its name, what it is) or the import's review; whether it
    /// is one and `value` fits it.
    fn set_symbol(&self, doc: &mut Value, key: &str, value: &Value) -> bool {
        let text = value.as_str();
        let own_function = self.draft.function_ref.is_none();
        match (key, text, value) {
            ("name", Some(s), _) if !s.trim().is_empty() => set_ml(&mut doc["symbol"]["name"], s.trim()),
            // A starter function: the symbol draws it, its pins standing for the function's. The box: a function of
            // its own again (no key), its pins kept.
            ("what", Some(word), _) if own_function || self.what().is_some() => {
                if word == BOX {
                    doc["function_ref"] = Value::Null;
                    // Its pins those drawn.
                    let drawn: Vec<Value> = (doc["symbol"]["cps"].as_array())
                        .map(|a| a.iter().map(|c| c["fn_cp"].clone()).collect())
                        .unwrap_or_default();
                    if let Some(pins) = doc["function"]["cps"].as_array_mut() {
                        pins.retain(|p| drawn.contains(&p["idx"]));
                    }
                    if let Some(function) = doc["function"].as_object_mut() {
                        function.remove("internal");
                    }
                    if let Some(meta) = doc["function"]["meta"].as_object_mut() {
                        meta.remove("key");
                    }
                    doc["function"]["contact_kind"] = json!("black_box");
                    doc["function"]["category"] = json!(["Assembly", "Generic", "Generic"]);
                } else {
                    let (Some(def), Some(r)) = (
                        ew_api::symbols::starter_function(word),
                        ew_api::symbols::piece_function(word),
                    ) else {
                        return false;
                    };
                    // The points drawn stand for its pins in order; one more than it has is To do's.
                    let pins: Vec<u16> = def.cps.iter().map(|c| c.idx).collect();
                    if let Some(points) = doc.pointer_mut("/symbol/cps").and_then(Value::as_array_mut) {
                        for (point, pin) in points.iter_mut().zip(&pins) {
                            point["fn_cp"] = json!(pin);
                        }
                    }
                    let (Ok(def), Ok(r)) = (serde_json::to_value(def), serde_json::to_value(r)) else {
                        return false;
                    };
                    doc["function"] = def;
                    doc["function_ref"] = r;
                }
            }
            ("licence", Some(s), _) => doc["symbol"]["meta"]["licence"] = json!(s.trim()),
            ("source", Some(s), _) => {
                doc["symbol"]["meta"]["sources"] = if s.trim().is_empty() {
                    json!([])
                } else {
                    json!([s.trim()])
                };
            }
            ("reviewed", _, Value::Bool(on)) => doc["import_reviewed"] = json!(on),
            _ => return false,
        }
        true
    }
}

/// A length in millimetres as a row prints it: up to three decimals, no trailing zeros.
pub fn trim_mm(mm: f64) -> String {
    let text = format!("{mm:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

impl ShellModel {
    /// The editor's Properties (`None` while the editor does not show).
    pub fn symbol_sheet(&self) -> Option<Sheet> {
        let editor = self.symbol_editor()?;
        Some(editor.sheet(&self.symbol_text_styles()))
    }

    /// `parts.field {field: symbol.<key>, value}` on the drawing: sets a field of the symbol's short sheet (`name`,
    /// `what`; `reviewed`, an import's review); one undo step. A value it does not take is refused in words.
    pub(crate) fn set_symbol_field(&mut self, key: &str, value: &str) -> Result<(), String> {
        let editor = self
            .symbol_editor
            .as_mut()
            .ok_or_else(|| words(key!("symbol-not-open")))?;
        if let Some(why) = &editor.locked {
            return Err(why.clone());
        }
        let value = match key {
            "reviewed" => Value::Bool(value == "true"),
            _ => Value::String(value.to_owned()),
        };
        let mut doc = serde_json::to_value(&editor.draft).map_err(|e| e.to_string())?;
        if !editor.set_symbol(&mut doc, key, &value) {
            return Err(words(key!("symbol-prop-invalid")));
        }
        if key == "what" {
            editor.kept_what = false;
        }
        if key == "name" {
            editor.unnamed = false;
        }
        editor.take(&doc)
    }

    /// `symbol.prop {key, value}`: sets a row of the drawing's Properties for every selected thing that has it; one
    /// undo step. A value the row does not take, or nothing selected, is refused in words.
    pub(super) fn symbol_prop(&mut self, p: &Map<String, Value>) -> Result<(), String> {
        let key = super::text(p, "key");
        let value = p.get("value").cloned().unwrap_or(Value::Null);
        let refused = || words(key!("symbol-prop-invalid"));
        let editor = self
            .symbol_editor
            .as_ref()
            .ok_or_else(|| words(key!("symbol-not-open")))?;
        let selection = editor.selection.clone();
        let param = |pairs: Value| pairs.as_object().cloned().unwrap_or_default();
        // The rows other operations set: a connection point's discipline and function pin.
        match (key, selection.as_slice()) {
            ("discipline" | "pin", items) if !items.is_empty() => {
                let (original, history) = (editor.draft.clone(), editor.undo.clone());
                for item in items {
                    let Item::Cp(i) = item else { return Err(refused()) };
                    let editor = self.symbol_editor.as_ref().ok_or_else(refused)?;
                    let idx = editor
                        .draft
                        .symbol
                        .cps
                        .iter()
                        .as_slice()
                        .get(*i)
                        .ok_or_else(refused)?
                        .idx;
                    let params = if key == "pin" {
                        let pin = value.as_str().and_then(|s| s.parse::<u16>().ok());
                        json!({ "cp": idx, "fn_cp": pin.ok_or_else(refused)? })
                    } else {
                        json!({ "cp": idx, "discipline": value })
                    };
                    self.symbol_operation(
                        if key == "pin" { "map_cp" } else { "discipline" },
                        &param(params),
                    )?;
                }
                // Several points changed are one step of the history.
                if let Some(editor) = &mut self.symbol_editor {
                    editor.undo = history;
                    if editor.draft != original {
                        let changed = std::mem::replace(&mut editor.draft, original);
                        editor.replace(changed);
                    }
                }
                return Ok(());
            }
            _ => {}
        }
        let editor = self
            .symbol_editor
            .as_mut()
            .ok_or_else(|| words(key!("symbol-not-open")))?;
        let mut doc = serde_json::to_value(&editor.draft).map_err(|e| e.to_string())?;
        if selection.is_empty() {
            return Err(refused());
        } else {
            let mut any = false;
            for item in &selection {
                // A point moved or given another edge takes the text that writes its number with it.
                let before = matches!(item, Item::Cp(_))
                    .then(|| super::selection::value_of(&doc, *item).cloned())
                    .flatten();
                let done = editor.set_item(&mut doc, *item, key, &value);
                if let (true, Some(before), "x" | "y" | "dir") = (done, &before, key) {
                    super::selection::cp_texts_follow(&mut doc, before, &selection);
                }
                any |= done;
            }
            if !any {
                return Err(refused());
            }
        }
        editor.take(&doc)?;
        // The style chosen for a text is the one the next text takes.
        if key == "style"
            && let Some(style) = value.as_str().filter(|s| !s.is_empty())
        {
            editor.text_style = Some(style.to_owned());
        }
        Ok(())
    }
}
