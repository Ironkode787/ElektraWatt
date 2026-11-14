// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EDZ part packages, EPLAN parts XML and a bare `part.xml` into [`PartDraft`]s (doc 14 §5.2, §5.3, FR-IMP-025 to
//! FR-IMP-028), under the importer contract of `ew-interop-api`.
//!
//! The reader is pure: it reads the bytes it is handed and nothing else. An EDZ (7-Zip or ZIP, told apart by its
//! content) is read under the archive limits of [`crate::archive`]; only its `*.part.xml` members are opened, the
//! others (pictures, 3D models) are counted and reported. The XML is streamed with `quick-xml` part by part, under
//! the XML limits of [`Limits`]; a DOCTYPE is refused. Every part becomes one [`PartDraft`] whose body is the
//! `lib/` JSON of a `Part`; `ew-import-catalog`'s `Placer` places it into a library.

use std::collections::BTreeMap;

use ew_interop_api::ew_ids::Uuid;
use ew_interop_api::{
    Code, ImportError, ImportInput, Limits, PartDraft, ReportEntry, SYSTEM_LIBRARY, Severity, SourceLocation,
};
use ew_mltext::{Ident, LangTag};
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::archive;

/// The importer's ID in the report and in a part's provenance.
pub const IMPORTER_ID: &str = "eplan";

/// The importer's version, recorded in the report; bumped on every mapping change.
pub const IMPORTER_VERSION: &str = "1";

/// The most report entries one read lists; the rest are counted (EW-IMP-001 in the stats, doc 14 §11).
const MAX_ENTRIES: usize = 1_000;

/// The most pins of one part, and of one file (a part's pins are JSON for the library: a hostile list of millions would
/// not fit the 256 MB of doc 14 §11).
const MAX_PINS: usize = 4_096;
const MAX_PINS_TOTAL: usize = 262_144;

/// The function definition of a template whose EPLAN definition is not mapped: EW's generic black box.
const GENERIC_FNDEF: &str = "ew.fn.blackbox";

/// The `version_req` of a reference to a starter function definition.
const BUILTIN_VERSION_REQ: &str = "^0.1";

// -------------------------------------------------------------------------------------------------------------------
// The function-definition table

/// One row of `fndef-map.toml`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    category: String,
    group: String,
    id: String,
    fndef: String,
    #[serde(default)]
    kind: Option<String>,
}

/// The file `fndef-map.toml`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapFile {
    #[serde(default)]
    map: Vec<Row>,
}

/// EPLAN's function definitions (category, group, id) and the EW function definition each stands for (doc 14 §5.2):
/// the table that ships ([`FndefMap::shipped`]) and, over it, the user's own rows ([`FndefMap::with_user`]).
#[derive(Clone, Debug)]
pub struct FndefMap {
    rows: BTreeMap<(String, String, String), (String, Option<String>)>,
}

impl FndefMap {
    /// The table that ships (`data/fndef-map.toml`).
    pub fn shipped() -> FndefMap {
        FndefMap::default()
            .with_user(include_str!("../data/fndef-map.toml"))
            .unwrap_or_else(|e| unreachable!("the shipped fndef-map.toml reads: {e}"))
    }

    /// This table with the rows of `toml` over it: a row for a definition that is mapped already replaces it.
    ///
    /// # Errors
    ///
    /// The text of the problem: the file is not TOML of the table's shape, or a row names no function definition.
    pub fn with_user(mut self, toml: &str) -> Result<FndefMap, String> {
        let file: MapFile = toml::from_str(toml).map_err(|e| e.to_string())?;
        for r in file.map {
            let (c, g, i) = (r.category.trim(), r.group.trim(), r.id.trim());
            if c.is_empty() || g.is_empty() || i.is_empty() || r.fndef.trim().is_empty() {
                return Err("every row needs a category, a group, an id and a fndef".to_owned());
            }
            self.rows.insert(
                (c.to_owned(), g.to_owned(), i.to_owned()),
                (r.fndef.trim().to_owned(), r.kind.map(|k| k.trim().to_owned())),
            );
        }
        Ok(self)
    }

    /// The EW function definition keys the table names, each once, in key order.
    pub fn fndef_keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.rows.values().map(|(k, _)| k.as_str()).collect();
        keys.sort_unstable();
        keys.dedup();
        keys
    }

    fn get(&self, key: &(String, String, String)) -> Option<&(String, Option<String>)> {
        self.rows.get(key)
    }
}

impl Default for FndefMap {
    /// An empty table: every template becomes a generic pin.
    fn default() -> FndefMap {
        FndefMap {
            rows: BTreeMap::new(),
        }
    }
}

// -------------------------------------------------------------------------------------------------------------------
// The XML, streamed

/// One `functiontemplate`.
#[derive(Default)]
struct Template {
    designations: Vec<String>,
    descriptions: Vec<String>,
    terminal_nrs: Vec<String>,
    key: Option<(String, String, String)>,
}

/// One `variant` (a part without one has a single, unnamed variant).
#[derive(Default)]
struct Variant {
    name: String,
    templates: Vec<Template>,
}

/// One `part` as the XML says it.
#[derive(Default)]
struct Article {
    /// The part's line in its file.
    line: u32,
    /// Its `P_ARTICLE_*` attributes.
    attrs: BTreeMap<String, String>,
    variants: Vec<Variant>,
    free: Vec<(String, String)>,
    accessories: Vec<(String, String, String)>,
    drilling: u32,
}

impl Article {
    fn attr(&self, name: &str) -> &str {
        self.attrs.get(name).map_or("", |v| v.trim())
    }
}

fn unreadable(why: impl std::fmt::Display) -> ImportError {
    ImportError::Malformed {
        message: why.to_string(),
        line: None,
    }
}

fn limit(what: &str, limit: u64) -> ImportError {
    ImportError::LimitExceeded {
        what: what.to_owned(),
        limit,
    }
}

/// The attributes of `e` by name, bounded by the per-element and per-document limits.
fn attributes(
    e: &BytesStart<'_>,
    limits: &Limits,
    total: &mut u64,
) -> Result<BTreeMap<String, String>, ImportError> {
    let mut out = BTreeMap::new();
    for (n, a) in e.attributes().enumerate() {
        if n as u64 >= u64::from(limits.max_xml_attributes_per_element) {
            return Err(limit(
                "XML attributes per element",
                u64::from(limits.max_xml_attributes_per_element),
            ));
        }
        *total += 1;
        if *total > u64::from(limits.max_xml_attributes) {
            return Err(limit("XML attributes", u64::from(limits.max_xml_attributes)));
        }
        let a = a.map_err(unreadable)?;
        let value = a.normalized_value(XmlVersion::Implicit1_0).map_err(unreadable)?;
        if value.len() > limits.max_attribute_bytes {
            return Err(limit("XML attribute value", limits.max_attribute_bytes as u64));
        }
        let name = String::from_utf8_lossy(a.key.local_name().as_ref()).into_owned();
        out.insert(name, value.into_owned());
    }
    Ok(out)
}

/// The values of a template attribute that lists one value per pin: split at line breaks and the pilcrow.
fn list(value: Option<&String>) -> Vec<String> {
    value
        .map(|v| {
            v.split(['\n', '\r', '¶'])
                .take(MAX_PINS + 1)
                .map(|s| s.trim().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// The function definition a template names: its three attributes, else `functiondefinition="c.g.i"`.
fn definition(attrs: &BTreeMap<String, String>) -> Option<(String, String, String)> {
    let get = |k: &str| {
        attrs
            .get(k)
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    };
    if let (Some(c), Some(g), Some(i)) = (
        get("functiondefcategory"),
        get("functiondefgroup"),
        get("functiondefid"),
    ) {
        return Some((c, g, i));
    }
    let text = get("functiondefinition")?;
    let mut parts = text.split(['.', '/', ',', ';']).map(str::trim);
    let key = (parts.next()?, parts.next()?, parts.next()?);
    (parts.next().is_none() && !key.0.is_empty() && !key.1.is_empty() && !key.2.is_empty())
        .then(|| (key.0.to_owned(), key.1.to_owned(), key.2.to_owned()))
}

/// The `length-unit` of the file as nanometres per unit.
fn unit_nm(unit: &str) -> Option<i64> {
    Some(match unit.trim().to_lowercase().as_str() {
        "" | "mm" => 1_000_000,
        "um" | "µm" => 1_000,
        "cm" => 10_000_000,
        "dm" => 100_000_000,
        "m" => 1_000_000_000,
        "in" | "inch" => 25_400_000,
        "ft" => 304_800_000,
        _ => return None,
    })
}

/// What [`stream`] finds.
enum Item {
    /// The root `partsmanagement` with its `length-unit` (empty when it has none).
    Management(String),
    /// A part, as it ends.
    Part(Article),
}

/// Streams the parts of `xml` (a `partsmanagement` or a bare `part`) to `on`, in file order.
fn stream(
    xml: &str,
    limits: &Limits,
    mut on: impl FnMut(Item) -> Result<(), ImportError>,
) -> Result<(), ImportError> {
    let mut reader = Reader::from_str(xml);
    let (mut depth, mut nodes, mut attrs_total) = (0u32, 0u64, 0u64);
    let (mut line, mut at) = (1u32, 0usize);
    let mut open: Option<Article> = None;
    let mut variant: Option<Variant> = None;
    let mut parts = 0u32;
    loop {
        let event = reader.read_event().map_err(unreadable)?;
        nodes += 1;
        if nodes > u64::from(limits.max_xml_nodes) {
            return Err(limit("XML nodes", u64::from(limits.max_xml_nodes)));
        }
        let pos = usize::try_from(reader.buffer_position())
            .unwrap_or(xml.len())
            .min(xml.len());
        let begin = xml.as_bytes()[at..pos].iter().filter(|&&b| b == b'\n').count();
        let here = line;
        line = line.saturating_add(u32::try_from(begin).unwrap_or(u32::MAX));
        at = pos;
        let (e, empty) = match event {
            Event::Start(e) => {
                depth += 1;
                if depth > limits.max_xml_depth {
                    return Err(limit("XML depth", u64::from(limits.max_xml_depth)));
                }
                (e, false)
            }
            Event::Empty(e) => (e, true),
            Event::End(e) => {
                depth = depth.saturating_sub(1);
                match e.local_name().as_ref() {
                    b"variant" => {
                        if let (Some(v), Some(p)) = (variant.take(), open.as_mut()) {
                            p.variants.push(v);
                        }
                    }
                    b"part" => {
                        if let Some(mut p) = open.take() {
                            if let Some(v) = variant.take() {
                                p.variants.push(v);
                            }
                            on(Item::Part(p))?;
                        }
                    }
                    _ => {}
                }
                continue;
            }
            Event::Text(t) => {
                if t.len() as u64 > u64::from(limits.max_text_bytes) {
                    return Err(limit("XML text", u64::from(limits.max_text_bytes)));
                }
                continue;
            }
            Event::CData(t) => {
                if t.len() as u64 > u64::from(limits.max_text_bytes) {
                    return Err(limit("XML text", u64::from(limits.max_text_bytes)));
                }
                continue;
            }
            Event::DocType(_) => return Err(unreadable("the file declares a DOCTYPE, and DTDs are refused")),
            Event::Eof => return Ok(()),
            _ => continue,
        };
        let name = e.local_name();
        match name.as_ref() {
            b"partsmanagement" => {
                let a = attributes(&e, limits, &mut attrs_total)?;
                on(Item::Management(
                    a.get("length-unit").cloned().unwrap_or_default(),
                ))?;
            }
            b"part" => {
                parts += 1;
                if parts > limits.max_table_rows {
                    return Err(limit("parts", u64::from(limits.max_table_rows)));
                }
                let article = Article {
                    line: here.max(1),
                    attrs: attributes(&e, limits, &mut attrs_total)?,
                    ..Article::default()
                };
                if empty {
                    on(Item::Part(article))?;
                } else {
                    open = Some(article);
                }
            }
            b"variant" if open.is_some() => {
                let a = attributes(&e, limits, &mut attrs_total)?;
                let v = Variant {
                    name: a.get("P_ARTICLE_VARIANT").cloned().unwrap_or_default(),
                    templates: Vec::new(),
                };
                if let Some(held) = variant.replace(v)
                    && let Some(p) = open.as_mut()
                {
                    p.variants.push(held);
                }
                if empty && let (Some(v), Some(p)) = (variant.take(), open.as_mut()) {
                    p.variants.push(v);
                }
            }
            b"functiontemplate" if open.is_some() => {
                let a = attributes(&e, limits, &mut attrs_total)?;
                let t = Template {
                    designations: list(a.get("connectionDesignation")),
                    descriptions: list(a.get("connectiondescription")),
                    terminal_nrs: list(a.get("terminalNr")),
                    key: definition(&a),
                };
                variant.get_or_insert_with(Variant::default).templates.push(t);
            }
            b"freeproperty" if open.is_some() => {
                let a = attributes(&e, limits, &mut attrs_total)?;
                if let (Some(p), Some(n)) = (open.as_mut(), a.get("name")) {
                    p.free
                        .push((n.clone(), a.get("value").cloned().unwrap_or_default()));
                }
            }
            b"accessory" if open.is_some() => {
                let a = attributes(&e, limits, &mut attrs_total)?;
                if let Some(p) = open.as_mut() {
                    let get = |k: &str| a.get(k).cloned().unwrap_or_default();
                    p.accessories.push((
                        get("P_ARTICLE_MANUFACTURER"),
                        get("P_ARTICLE_PARTNR"),
                        get("quantity"),
                    ));
                }
            }
            b"drillingPosition" if open.is_some() => {
                if let Some(p) = open.as_mut() {
                    p.drilling = p.drilling.saturating_add(1);
                }
            }
            _ => {}
        }
    }
}

// -------------------------------------------------------------------------------------------------------------------
// The mapping

/// A decimal text as nanometres with `per_unit` nanometres to the unit: `,` or `.` as the separator, at most 9 decimals,
/// no sign. `None` for anything else, or a value of 2^62 or more.
fn to_nm(text: &str, per_unit: i64) -> Option<i64> {
    let t = text.trim().replace(',', ".");
    let (int, frac) = t.split_once('.').unwrap_or((&t, ""));
    let digits = |p: &str| p.bytes().all(|b| b.is_ascii_digit());
    if (int.is_empty() && frac.is_empty()) || !digits(int) || !digits(frac) || frac.len() > 9 {
        return None;
    }
    let whole: i128 = if int.is_empty() { 0 } else { int.parse().ok()? };
    let fraction: i128 = if frac.is_empty() { 0 } else { frac.parse().ok()? };
    let scale = 10i128.pow(frac.len() as u32);
    let nm = whole.checked_mul(i128::from(per_unit))?
        + (fraction * i128::from(per_unit) * 2 + scale) / (2 * scale);
    i64::try_from(nm).ok().filter(|v| *v < 1 << 62)
}

/// The languages and texts of a multilingual article field, `de_DE@text;en_US@text;` (`;;` inside a text is a literal
/// `;`): a map of language tag to text, or the text itself when the field names no language.
fn ml_text(field: &str) -> (BTreeMap<String, String>, Option<String>, Vec<String>) {
    let field = field.trim();
    let mut langs = BTreeMap::new();
    let mut bad = Vec::new();
    if !field.contains('@') {
        return (langs, (!field.is_empty()).then(|| field.to_owned()), bad);
    }
    let mut segments: Vec<String> = vec![String::new()];
    let mut chars = field.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ';' if chars.peek() == Some(&';') => {
                chars.next();
                segments.last_mut().unwrap_or_else(|| unreachable!()).push(';');
            }
            ';' => segments.push(String::new()),
            c => segments.last_mut().unwrap_or_else(|| unreachable!()).push(c),
        }
    }
    for s in segments {
        let s = s.trim();
        if s.is_empty() {
            continue;
        }
        match s.split_once('@') {
            Some((lang, text)) => match LangTag::parse_lenient(&lang.trim().replace('_', "-")) {
                Ok(tag) if !text.trim().is_empty() => {
                    langs.insert(tag.as_str().to_owned(), text.trim().to_owned());
                }
                Ok(_) => {}
                Err(_) => bad.push(lang.trim().to_owned()),
            },
            None => bad.push(s.to_owned()),
        }
    }
    (langs, None, bad)
}

/// The report of one read, with the cap of [`MAX_ENTRIES`].
struct Notes {
    file: String,
    /// The pins kept so far, all parts together.
    pins: usize,
    entries: Vec<ReportEntry>,
    suppressed: BTreeMap<Code, u32>,
}

impl Notes {
    fn push(
        &mut self,
        code: Code,
        severity: Severity,
        message: String,
        member: Option<&str>,
        line: Option<u32>,
    ) {
        if self.entries.len() >= MAX_ENTRIES {
            *self.suppressed.entry(code).or_default() += 1;
            return;
        }
        self.entries
            .push(ReportEntry::new(code, severity, message).at(SourceLocation {
                file: self.file.clone(),
                member: member.map(str::to_owned),
                line,
                ..SourceLocation::default()
            }));
    }
}

/// The reference to the starter function definition `key`: `v5(SYSTEM_LIBRARY, key)`.
fn builtin(key: &str) -> Value {
    json!({
        "library": SYSTEM_LIBRARY.hyphenated().to_string(),
        "item": Uuid::new_v5(&SYSTEM_LIBRARY, key.as_bytes()).hyphenated().to_string(),
        "version_req": BUILTIN_VERSION_REQ,
        "key": key,
    })
}

/// What the file's `length-unit` is worth: nanometres to the unit.
struct Unit {
    per_unit: i64,
}

/// The pins of the main variant: CPs in template order, one template function per function definition.
fn pins(
    article: &Article,
    fndefs: &FndefMap,
    notes: &mut Notes,
    member: &str,
    body: &mut Map<String, Value>,
) {
    let Some(main) = article.variants.first() else {
        return;
    };
    // The function definitions in order of first use, each with the pins of its templates.
    /// The pins of one function definition: the definition, the part kind it makes, the pins with their extras.
    type Group = (String, Option<String>, Vec<(String, Map<String, Value>)>);
    let mut groups: Vec<Group> = Vec::new();
    let mut skipped = 0usize;
    for t in &main.templates {
        let (fndef, kind) = match t.key.as_ref().and_then(|k| fndefs.get(k)) {
            Some((f, k)) => (f.clone(), k.clone()),
            None => {
                let named = t
                    .key
                    .as_ref()
                    .map_or_else(|| "none".to_owned(), |k| format!("{}.{}.{}", k.0, k.1, k.2));
                notes.push(
                    Code::imp(202),
                    Severity::Warning,
                    format!(
                        "the function definition `{named}` of part `{}` is not in the function-definition table: its pins are generic",
                        article.attr("P_ARTICLE_PARTNR")
                    ),
                    Some(member),
                    Some(article.line),
                );
                (GENERIC_FNDEF.to_owned(), None)
            }
        };
        let slot = match groups.iter().position(|g| g.0 == fndef) {
            Some(i) => i,
            None => {
                groups.push((fndef, kind, Vec::new()));
                groups.len() - 1
            }
        };
        for (i, d) in t.designations.iter().enumerate() {
            if d.is_empty() {
                continue;
            }
            if notes.pins >= MAX_PINS_TOTAL || groups.iter().map(|g| g.2.len()).sum::<usize>() >= MAX_PINS {
                skipped += 1;
                continue;
            }
            notes.pins += 1;
            let mut extra = Map::new();
            if let Some(n) = t.terminal_nrs.get(i).filter(|n| !n.is_empty()) {
                extra.insert("eplan.terminal_nr".into(), json!(n));
            }
            if let Some(n) = t.descriptions.get(i).filter(|n| !n.is_empty()) {
                extra.insert("eplan.description".into(), json!(n));
            }
            groups[slot].2.push((d.clone(), extra));
        }
    }
    if skipped > 0 {
        notes.push(
            Code::LIMIT,
            Severity::Warning,
            format!(
                "part `{}` has more pins than one part or one file may have: {skipped} pins left out",
                article.attr("P_ARTICLE_PARTNR")
            ),
            Some(member),
            Some(article.line),
        );
    }
    let mut cps = Vec::new();
    let mut template = Vec::new();
    let mut kind = None;
    for (n, (fndef, k, mut group)) in groups.into_iter().enumerate() {
        // Natural order within a group of equal function definitions when its designations are numbered ("1", "2",
        // "10"); lettered pins (L, N, PE, +, −) keep the template's order.
        if group.iter().all(|(d, _)| d.chars().any(|c| c.is_ascii_digit())) {
            group.sort_by(|a, b| ew_mltext::natural_cmp(&a.0, &b.0));
        }
        if group.is_empty() {
            continue;
        }
        if kind.is_none() {
            kind = k;
        }
        let mut indices = Vec::new();
        for (designation, extra) in group {
            let idx = cps.len();
            indices.push(idx);
            let mut cp = Map::new();
            cp.insert("idx".into(), json!(idx));
            cp.insert("designation".into(), json!(designation));
            cp.extend(extra);
            cps.push(Value::Object(cp));
        }
        let mut f = Map::new();
        f.insert("fndef".into(), builtin(&fndef));
        if n == 0 || template.is_empty() {
            f.insert("main".into(), json!(true));
        }
        f.insert("cps".into(), json!(indices));
        template.push(Value::Object(f));
    }
    if cps.is_empty() {
        return;
    }
    body.insert("cp_pattern".into(), Value::Array(cps));
    body.insert("function_template".into(), Value::Array(template));
    if let Some(k) = kind.filter(|k| k != "device") {
        body.insert("kind".into(), json!(k));
    }
}

/// One article as a draft, or `None` when it has no part number or no manufacturer (the placer rejects it).
fn draft(
    article: &Article,
    fndefs: &FndefMap,
    unit: &Unit,
    notes: &mut Notes,
    member: &str,
    sources: &[String],
) -> PartDraft {
    let mut body = Map::new();
    let at = Some(article.line);
    let pn = article.attr("P_ARTICLE_PARTNR");
    let valid = |what: &str, v: &str, notes: &mut Notes| -> Option<Value> {
        if v.is_empty() {
            return None;
        }
        if Ident::from_input(v).is_err() {
            notes.push(
                Code::MALFORMED_ITEM,
                Severity::Warning,
                format!("the {what} `{v}` of part `{pn}` is not a valid identifier: left out"),
                Some(member),
                at,
            );
            return None;
        }
        Some(json!(v))
    };
    if let Some(v) = valid("type number", article.attr("P_ARTICLE_TYPENR"), notes) {
        body.insert("type_designation".into(), v);
    }
    if let Some(v) = valid("order number", article.attr("P_ARTICLE_ORDERNR"), notes) {
        body.insert("order_number".into(), v);
    }
    // The description: the first and the second line of the article, per language.
    let (l1, n1, b1) = ml_text(article.attr("P_ARTICLE_DESCR1"));
    let (l2, n2, b2) = ml_text(article.attr("P_ARTICLE_DESCR2"));
    for lang in b1.iter().chain(&b2) {
        notes.push(
            Code::MALFORMED_ITEM,
            Severity::Warning,
            format!("`{lang}` in the description of part `{pn}` is not a language and a text: left out"),
            Some(member),
            at,
        );
    }
    if !l1.is_empty() || !l2.is_empty() {
        let mut langs = Map::new();
        for tag in l1.keys().chain(l2.keys()) {
            let joined = [l1.get(tag), l2.get(tag)]
                .into_iter()
                .flatten()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            langs.insert(tag.clone(), json!(joined));
        }
        body.insert("description".into(), Value::Object(langs));
    } else if let Some(text) = [n1, n2].into_iter().flatten().reduce(|a, b| format!("{a}, {b}")) {
        body.insert("description".into(), json!(text));
    }
    // The size, in the file's unit.
    let mut mech = Map::new();
    for (attr, member_name) in [
        ("P_ARTICLE_WIDTH", "width"),
        ("P_ARTICLE_HEIGHT", "height"),
        ("P_ARTICLE_DEPTH", "depth"),
    ] {
        let v = article.attr(attr);
        if v.is_empty() {
            continue;
        }
        match to_nm(v, unit.per_unit) {
            Some(nm) => {
                mech.insert(member_name.into(), json!(nm));
            }
            None => notes.push(
                Code::MALFORMED_ITEM,
                Severity::Warning,
                format!("the {member_name} `{v}` of part `{pn}` is not a length: left out"),
                Some(member),
                at,
            ),
        }
    }
    if !mech.is_empty() {
        body.insert("mech".into(), Value::Object(mech));
    }
    pins(article, fndefs, notes, member, &mut body);
    // Free properties, keyed by name; a repeated name keeps its first value.
    let mut free: BTreeMap<String, &str> = BTreeMap::new();
    for (k, v) in &article.free {
        let k = k.trim();
        if Ident::from_input(k).is_err() {
            notes.push(
                Code::MALFORMED_ITEM,
                Severity::Warning,
                format!("the free property `{k}` of part `{pn}` has no usable name: left out"),
                Some(member),
                at,
            );
        } else if free.insert(k.to_owned(), v.trim()).is_some() {
            notes.push(
                Code::DROPPED_BY_DESIGN,
                Severity::Info,
                format!("the free property `{k}` of part `{pn}` is given twice: the last value is kept"),
                Some(member),
                at,
            );
        }
    }
    if !free.is_empty() {
        let mut props: Vec<(&String, &&str)> = free.iter().collect();
        props.sort_by(|a, b| Ident::from_input(a.0).ok().cmp(&Ident::from_input(b.0).ok()));
        body.insert(
            "free_props".into(),
            Value::Array(
                props
                    .into_iter()
                    .map(|(k, v)| json!({"key": k, "value": v}))
                    .collect(),
            ),
        );
    }
    // What the part model has no member for yet stays in `extra` under the importer's own names.
    if !article.accessories.is_empty() {
        body.insert(
            "eplan.accessories".into(),
            Value::Array(
                article
                    .accessories
                    .iter()
                    .map(|(m, p, q)| json!({"manufacturer": m, "part_number": p, "qty": q}))
                    .collect(),
            ),
        );
    }
    if article.variants.len() > 1 {
        body.insert(
            "eplan.variants".into(),
            Value::Array(
                article
                    .variants
                    .iter()
                    .skip(1)
                    .map(|v| {
                        let pins: Vec<&str> = v
                            .templates
                            .iter()
                            .flat_map(|t| t.designations.iter().map(String::as_str))
                            .filter(|d| !d.is_empty())
                            .collect();
                        json!({"name": v.name, "pins": pins})
                    })
                    .collect(),
            ),
        );
        notes.push(
            Code::DROPPED_BY_DESIGN,
            Severity::Info,
            format!(
                "part `{pn}` has {} variants: the first gives the pins, the others are kept as text under `eplan.variants`",
                article.variants.len()
            ),
            Some(member),
            at,
        );
    }
    if article.drilling > 0 {
        notes.push(
            Code::DROPPED_BY_DESIGN,
            Severity::Info,
            format!(
                "part `{pn}` has {} drilling positions: not imported yet (a part has no drilling pattern before the panel work)",
                article.drilling
            ),
            Some(member),
            at,
        );
    }
    if !article.attr("P_ARTICLE_PICTUREFILE").is_empty() {
        notes.push(
            Code::DROPPED_BY_DESIGN,
            Severity::Info,
            format!("the picture of part `{pn}` is not imported: a part holds no picture yet"),
            Some(member),
            at,
        );
    }
    let mut meta = Map::new();
    meta.insert("licence".into(), json!("LicenseRef-vendor-terms"));
    meta.insert("foreign_terms".into(), json!(true));
    PartDraft {
        line: article.line,
        part_number: pn.to_owned(),
        manufacturer: article.attr("P_ARTICLE_MANUFACTURER").to_owned(),
        body,
        meta,
        sources: sources.to_vec(),
    }
}

// -------------------------------------------------------------------------------------------------------------------
// The entry point

/// What a read of an EDZ or a parts XML gives.
#[derive(Clone, PartialEq, Debug)]
pub struct PartsRead {
    /// The detected format: `edz-7z`, `edz-zip`, `parts-xml` or `part-xml`.
    pub format: &'static str,
    /// One draft per part, in file order.
    pub drafts: Vec<PartDraft>,
    /// What the read found (EW-IMP-202 to -205 and the parts' own notes), at most 1,000.
    pub entries: Vec<ReportEntry>,
    /// The findings the cap left out, per code.
    pub suppressed: BTreeMap<Code, u32>,
}

fn hex(sha: [u8; 32]) -> String {
    use std::fmt::Write as _;
    sha.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// The error of an archive or a text of the old readers, as the contract's.
fn refusal(key: &str) -> ImportError {
    match key {
        "eplan-input-limit" => limit("archive size", crate::MAX_TOTAL),
        "eplan-archive-method" => ImportError::Unsupported {
            message: "the archive uses encryption or a compression method this reader does not read".into(),
        },
        "eplan-archive-path" => unreadable("the archive names a member by a path it must not use, or twice"),
        "eplan-binary-unsupported" => unreadable("a part file is not XML text"),
        _ => unreadable("the archive is damaged"),
    }
}

/// Reads the parts of an EDZ (7-Zip or ZIP), an EPLAN parts XML (`partsmanagement`) or a bare part XML, into drafts
/// (doc 14 §5.2, §5.3). The format is told by the content: the 7-Zip signature, a ZIP's `PK`, else XML. An EDZ's
/// `*.part.xml` members are read; the others are counted into one finding (EW-IMP-204 for `.e3d` 3D models). A ZIP
/// container is EW-IMP-203, a length unit other than mm EW-IMP-205 (every length is converted to nm), a function
/// definition the table does not map EW-IMP-202.
///
/// # Errors
///
/// [`ImportError::LimitExceeded`] over a limit of `limits` or of the archive reader (the EDZ at most 32 MiB, a part file
/// at most 4 MiB, 64 MiB unpacked); [`ImportError::Malformed`] for a damaged archive, XML that does not parse, a DOCTYPE,
/// an archive without part data or a file of another kind; [`ImportError::Unsupported`] for an encrypted archive or a
/// `length-unit` this reader does not know.
pub fn read_parts(input: &ImportInput, fndefs: &FndefMap, limits: &Limits) -> Result<PartsRead, ImportError> {
    let bytes = input.bytes();
    if bytes.len() as u64 > limits.max_input_bytes {
        return Err(limit("input size", limits.max_input_bytes));
    }
    let mut notes = Notes {
        file: input.name().to_owned(),
        entries: Vec::new(),
        suppressed: BTreeMap::new(),
        pins: 0,
    };
    let sha = hex(input.sha256());
    let seven = bytes.starts_with(b"7z\xbc\xaf\x27\x1c");
    let zip = bytes.starts_with(b"PK");
    let (members, format): (Vec<(String, String)>, &'static str) = if seven || zip {
        let (files, skipped) =
            archive::read_matching(bytes, &|n| n.to_ascii_lowercase().ends_with(".part.xml"))
                .map_err(|k| refusal(&k))?;
        if zip {
            notes.push(
                Code::imp(203),
                Severity::Info,
                "the EDZ is a ZIP archive, not a 7-Zip archive".into(),
                None,
                None,
            );
        }
        let models = skipped
            .iter()
            .filter(|n| n.to_ascii_lowercase().ends_with(".e3d"))
            .count();
        if models > 0 {
            notes.push(
                Code::imp(204),
                Severity::Info,
                format!("{models} 3D model files (.e3d) are ignored: the STEP model of the part is used"),
                None,
                None,
            );
        }
        if skipped.len() > models {
            notes.push(
                Code::DROPPED_BY_DESIGN,
                Severity::Info,
                format!(
                    "{} other members of the EDZ are not read (pictures and the like)",
                    skipped.len() - models
                ),
                None,
                None,
            );
        }
        if files.is_empty() {
            return Err(unreadable("the EDZ holds no *.part.xml part data"));
        }
        let mut members = Vec::new();
        for (name, data) in files {
            members.push((name, crate::text(&data).map_err(|k| refusal(&k))?.into_owned()));
        }
        (members, if seven { "edz-7z" } else { "edz-zip" })
    } else {
        let text = crate::text(bytes).map_err(|k| refusal(&k))?;
        let text = text.trim_start_matches('\u{feff}').to_owned();
        (vec![(String::new(), text)], "parts-xml")
    };
    let sources = vec![
        format!(
            "eplan.{}:{}",
            if format.starts_with("edz") {
                "edz"
            } else {
                "parts-xml"
            },
            input.name()
        ),
        format!("sha256:{sha}"),
    ];
    let mut drafts = Vec::new();
    let mut management = false;
    for (name, text) in &members {
        let member = (!name.is_empty()).then_some(name.as_str());
        let mut unit = Unit { per_unit: 1_000_000 };
        let before = drafts.len();
        stream(text, limits, |item| {
            match item {
                Item::Management(u) => {
                    management = true;
                    unit.per_unit = unit_nm(&u).ok_or_else(|| ImportError::Unsupported {
                        message: format!(
                            "the length unit `{}` is not known (mm, cm, dm, m, um, in, ft)",
                            u.trim()
                        ),
                    })?;
                    if unit.per_unit != 1_000_000 {
                        notes.push(
                            Code::imp(205),
                            Severity::Info,
                            format!("lengths in `{}` are converted to nanometres", u.trim()),
                            member,
                            None,
                        );
                    }
                }
                Item::Part(a) => drafts.push(draft(
                    &a,
                    fndefs,
                    &unit,
                    &mut notes,
                    member.unwrap_or(""),
                    &sources,
                )),
            }
            Ok(())
        })?;
        if drafts.len() == before {
            return Err(unreadable(match member {
                Some(m) => format!("{m} holds no part"),
                None => "the file holds no part".to_owned(),
            }));
        }
    }
    let format = if format == "parts-xml" && !management {
        "part-xml"
    } else {
        format
    };
    Ok(PartsRead {
        format,
        drafts,
        entries: notes.entries,
        suppressed: notes.suppressed,
    })
}
