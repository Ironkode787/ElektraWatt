// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Symbol authoring shared by the window and data-driven tools. Drafts are not project records.
//! Publishing uses the existing library writer; the project/library format is unchanged.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::Path;

use ew_derive::ew_ids::{Id, Uuid, v5};
pub use ew_dl as drawing;
pub use ew_mltext::{MlText, MlTextView};
pub use ew_scene::symbol::{SymbolGeometry, TextSource, lower};
pub use ew_schema as schema;
use ew_schema::{FnDef, LibCopy, LibDocument, LibRef, Sha256, SymbolDef};
pub use ew_text::TextStyleTable;
pub use ew_units as units;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::library::{LibraryError, Registry, WriteReport};

/// Authoring interchange version, independent of the released project format.
pub const FORMAT: &str = "ew.symbol/1";
/// Maximum authoring document size before parsing.
pub const MAX_BYTES: u64 = 4 * 1024 * 1024;
/// Maximum number of primitives or connection points in an editable symbol.
pub const MAX_ITEMS: usize = 4096;
/// Stable namespace for a draft's temporary references, never a user identity.
pub const DRAFT_LIBRARY: Uuid = Uuid::from_u128(0x791cacd0_f0e4_57af_b580_b364458b9452);

/// A portable draft. Unknown members of the underlying format objects survive editing.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SymbolDraft {
    /// Must be `ew.symbol/1`.
    pub format: String,
    /// Stable human key. The publisher derives IDs from this key.
    pub key: String,
    /// Semantic version; changing published content requires a new version.
    pub version: String,
    /// Drawing, pins, text and anchor in the existing format's integer nanometres.
    pub symbol: SymbolDef,
    /// Electrical semantics, including printed pin designations.
    pub function: FnDef,
    /// An assigned existing function definition. Absent: publish an owned definition with the symbol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function_ref: Option<LibRef>,
    /// Import limitations, retained in the portable draft for review.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub import_notes: Vec<String>,
    /// Detailed source-reader findings, preserved for review and automated correction.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub import_details: Vec<ImportDetail>,
    /// Explicit author acknowledgement of EPLAN geometry, function and source-term limitations.
    #[serde(default)]
    pub import_reviewed: bool,
}

/// A source-reader finding. The message retains the reader's original language.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportDetail {
    /// Stable source-reader diagnostic code.
    pub code: String,
    /// What was changed or omitted, in the reader's original wording.
    pub message: String,
    /// Source line when available.
    pub line: Option<u32>,
}

/// A validation message with a stable code, field path and localisable arguments.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct Issue {
    /// Localisation key; also the machine-readable code.
    pub code: &'static str,
    /// JSON pointer in the draft.
    pub path: String,
    /// Human-readable record name or offending value, never an opaque record ID.
    pub detail: String,
}

/// Authoring errors leave both draft and target unchanged.
#[derive(Debug)]
pub enum Error {
    /// Input is malformed, unsupported, or over a resource bound.
    Input(String),
    /// Publication refused by editor validation.
    Invalid(Vec<Issue>),
    /// The existing library writer refused the operation.
    Library(LibraryError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input(s) => f.write_str(s),
            Self::Invalid(v) => {
                for (i, issue) in v.iter().enumerate() {
                    if i != 0 {
                        f.write_str("; ")?;
                    }
                    write!(f, "{} at {}: {}", issue.code, issue.path, issue.detail)?;
                }
                Ok(())
            }
            Self::Library(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for Error {}

fn input(e: impl std::fmt::Display) -> Error {
    Error::Input(e.to_string())
}

impl crate::Session {
    /// Fork an embedded symbol (including a QET import), falling back to installed libraries.
    ///
    /// # Errors
    /// The source or its function definition is unavailable.
    pub fn symbol_draft(&self, source: &str, key: &str) -> Result<SymbolDraft, Error> {
        let q = self.query();
        if let Some((_, s)) = q.symbols().iter().find(|(id, s)| {
            id.to_string() == source || s.item.meta.key.as_ref().is_some_and(|k| k.as_str() == source)
        }) {
            let r = &s.item.fndef;
            let id = v5::lib_copy_id(r.library.as_uuid(), r.item.as_uuid());
            let function = q
                .fndefs()
                .get(id)
                .ok_or_else(|| input("symbol-function-required"))?;
            let mut d = SymbolDraft::new(key, key)?;
            d.symbol = s.item.clone();
            d.function = function.item.clone();
            d.symbol.meta.key = Some(key.parse().map_err(input)?);
            return Ok(d);
        }
        SymbolDraft::from_library(self.libraries(), source, key)
    }
}

impl SymbolDraft {
    /// Fork an installed symbol by its library/item address, or by its legacy library key.
    ///
    /// # Errors
    /// The symbol or its function definition is unavailable.
    pub fn from_library(registry: &Registry, source: &str, key: &str) -> Result<Self, Error> {
        let found = registry
            .libraries()
            .iter()
            .flat_map(|l| registry.documents(l.id, schema::Kind::Symbol))
            .find_map(|d| match d {
                LibDocument::Symbol(s)
                    if format!("library:{}:{}", s.library_id, s.item_id) == source
                        || s.item.meta.key.as_ref().is_some_and(|k| k.as_str() == source) =>
                {
                    Some(s)
                }
                _ => None,
            })
            .ok_or_else(|| input("symbol-input-object"))?;
        Self::fork(registry, &found, key)
    }
}

/// Atomically export a portable draft to a new file; existing files are never overwritten.
///
/// # Errors
/// Encoding, I/O, or an existing destination. A temporary sibling is removed on failure.
pub fn export(draft: &SymbolDraft, path: &Path) -> Result<(), Error> {
    use std::io::Write;
    let bytes = serde_json::to_vec_pretty(draft).map_err(input)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let id = Id::<()>::try_new_v7().map_err(input)?;
    let temporary = parent.join(format!(".symbol-{id}.tmp"));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(input)?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(input)?;
        drop(file);
        std::fs::hard_link(&temporary, path).map_err(input)
    })();
    let _ = std::fs::remove_file(&temporary);
    result
}

/// Preview graphics with text shaped identically to page output. Slot contents are illustrative except pin designations.
pub fn preview_display_list(
    draft: &SymbolDraft,
    geometry: &SymbolGeometry,
    styles: &TextStyleTable,
    slot_placeholder: &str,
) -> drawing::DisplayList {
    let mut list = geometry.graphics.clone();
    for text in &geometry.texts {
        let content = match &text.source {
            TextSource::Static(t) => match t.view() {
                MlTextView::Neutral(t) => t.to_owned(),
                MlTextView::Langs(map) => map
                    .iter()
                    .find(|(k, _)| k.as_str() == "en")
                    .or_else(|| map.iter().next())
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default(),
            },
            TextSource::Slot(schema::SlotKind::CpDesignation(idx)) => draft
                .symbol
                .cps
                .iter()
                .find(|c| c.idx == *idx)
                .and_then(|c| draft.function.cps.iter().find(|p| p.idx == c.fn_cp))
                .map(|p| p.designation.to_string())
                .unwrap_or_default(),
            TextSource::Slot(schema::SlotKind::Tag) => "-U1".into(),
            TextSource::Slot(_) => slot_placeholder.into(),
        };
        list.items.extend(
            ew_scene::page::symbol_preview_text(text, &content, ew_text::bundled(), styles)
                .list
                .items,
        );
    }
    list
}

impl SymbolDraft {
    /// A new native-grid draft with a private, empty black-box function definition.
    ///
    /// # Errors
    /// Invalid text in the supplied name or key.
    pub fn new(key: &str, name: &str) -> Result<Self, Error> {
        let reference = reference(DRAFT_LIBRARY, &format!("{key}.function"), "1.0.0");
        serde_json::from_value(json!({
            "format": FORMAT, "key": key, "version": "1.0.0",
            "symbol": {
                "meta": {"key": key, "licence": "CC-BY-SA-4.0", "sources": ["original"], "status": "draft"},
                "name": {"en": name}, "grid": {"num": 4000000, "den": 1},
                "fndef": reference, "anchor": "graphic_centre"
            },
            "function": {"category": ["Assembly", "BlackBox", "Generic"], "contact_kind": "black_box"}
        }))
        .map_err(input)
    }

    /// Read authoring JSON. Strict JSON parsing rejects duplicate members before typed loading.
    ///
    /// # Errors
    /// Size, depth, malformed JSON, unsupported envelope version, or invalid format values.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() as u64 > MAX_BYTES {
            return Err(input("symbol-input-limit"));
        }
        let text = std::str::from_utf8(bytes).map_err(input)?;
        let value = ew_schema::parse_value(text).map_err(input)?;
        let draft: Self = serde_json::from_value(value).map_err(input)?;
        if draft.format != FORMAT {
            return Err(input("symbol-input-version"));
        }
        if draft.symbol.graphics.len() > MAX_ITEMS
            || draft.symbol.cps.len() > MAX_ITEMS
            || draft.function.cps.len() > MAX_ITEMS
        {
            return Err(input("symbol-input-limit"));
        }
        Ok(draft)
    }

    /// All save-blocking issues, in stable field order. This does not modify the draft.
    pub fn validate(&self) -> Vec<Issue> {
        use ew_schema::{AnchorPolicy, Dir4, Primitive, SlotKind};
        use ew_units::Grid;
        let mut out = Vec::new();
        let mut issue = |code, path: String, detail: String| out.push(Issue { code, path, detail });
        if self.format != FORMAT {
            issue("symbol-input-version", "/format".into(), self.format.clone());
        }
        if self.key.is_empty()
            || self.key.len() > 128
            || !self
                .key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            issue("symbol-invalid-key", "/key".into(), self.key.clone());
        }
        let version: Vec<_> = self.version.split('.').collect();
        if version.len() != 3
            || version
                .iter()
                .any(|p| p.is_empty() || (p.len() > 1 && p.starts_with('0')) || p.parse::<u32>().is_err())
        {
            issue("symbol-invalid-version", "/version".into(), self.version.clone());
        }
        if self.symbol.name.is_empty() {
            issue("symbol-name-required", "/symbol/name".into(), self.key.clone());
        }
        if self
            .symbol
            .meta
            .licence
            .as_ref()
            .is_none_or(|s| s.trim().is_empty())
            || self.symbol.meta.sources.iter().all(|s| s.trim().is_empty())
        {
            issue(
                "symbol-provenance-required",
                "/symbol/meta".into(),
                self.key.clone(),
            );
        }
        let native = Grid::new(4_000_000, 1).expect("constant grid");
        if self.symbol.grid != native && self.symbol.grid != Grid::QET_10PX {
            issue("symbol-grid-unsupported", "/symbol/grid".into(), self.key.clone());
        }
        if self.symbol.graphics.len() > MAX_ITEMS
            || self.symbol.cps.len() > MAX_ITEMS
            || self.function.cps.len() > MAX_ITEMS
        {
            issue("symbol-input-limit", "/symbol".into(), self.key.clone());
        }
        if matches!(self.function.contact_kind, schema::ContactKind::Unknown(_)) {
            issue(
                "symbol-function-semantics",
                "/function/contact_kind".into(),
                self.key.clone(),
            );
        }
        for cp in self.function.cps.iter() {
            if matches!(cp.discipline, schema::Discipline::Unknown(_))
                || matches!(cp.potential_role, Some(schema::PotentialType::Unknown(_)))
                || matches!(cp.side, Some(schema::Side::Unknown(_)))
                || matches!(cp.port_role, Some(schema::FluidPortRole::Unknown(_)))
            {
                issue(
                    "symbol-function-semantics",
                    format!("/function/cps/{}", cp.idx),
                    cp.designation.to_string(),
                );
            }
        }
        for cp in self.symbol.cps.iter() {
            if matches!(cp.discipline, schema::Discipline::Unknown(_)) {
                issue(
                    "symbol-function-semantics",
                    format!("/symbol/cps/{}", cp.idx),
                    cp.idx.to_string(),
                );
            }
        }
        let mut mapped = BTreeSet::new();
        if self.import_notes.iter().any(|n| n.starts_with("eplan-")) && !self.import_reviewed {
            issue(
                "symbol-import-review",
                "/import_reviewed".into(),
                self.key.clone(),
            );
        }
        if self.symbol.graphics.is_empty() {
            issue(
                "symbol-graphics-required",
                "/symbol/graphics".into(),
                self.key.clone(),
            );
        }
        let grid = self.symbol.grid;
        for link in &self.function.internal {
            if link.a == link.b
                || ![link.a, link.b]
                    .iter()
                    .all(|i| self.function.cps.iter().any(|p| p.idx == *i))
                || matches!(link.kind, schema::InternalKind::Unknown(_))
            {
                issue(
                    "symbol-function-connections",
                    "/function/internal".into(),
                    self.key.clone(),
                );
            }
        }
        for cp in self.symbol.cps.iter() {
            let path = format!("/symbol/cps/{}", cp.idx);
            let function = self.function.cps.iter().find(|p| p.idx == cp.fn_cp);
            let label = function.map_or_else(|| cp.idx.to_string(), |p| p.designation.to_string());
            if grid.at(grid.snap(cp.pos.x)) != Some(cp.pos.x)
                || grid.at(grid.snap(cp.pos.y)) != Some(cp.pos.y)
            {
                issue("symbol-cp-off-grid", format!("{path}/pos"), label.clone());
            }
            if function.is_none_or(|p| !p.physical || p.discipline != cp.discipline)
                || !mapped.insert(cp.fn_cp)
            {
                issue("symbol-cp-mapping", format!("{path}/fn_cp"), label.clone());
            }
            if matches!(cp.dir, Dir4::Unknown(_)) {
                issue("symbol-cp-direction", format!("{path}/dir"), label);
            }
        }
        for (i, slot) in self.symbol.text_slots.iter().enumerate() {
            if matches!(slot.kind, SlotKind::Unknown(_)) {
                issue(
                    "symbol-slot-pin",
                    format!("/symbol/text_slots/{i}/kind"),
                    (i + 1).to_string(),
                );
            }
            if let SlotKind::CpDesignation(idx) = slot.kind
                && !self.symbol.cps.iter().any(|c| c.idx == idx)
            {
                issue(
                    "symbol-slot-pin",
                    format!("/symbol/text_slots/{i}"),
                    idx.to_string(),
                );
            }
        }
        if let AnchorPolicy::Slot(kind) = &self.symbol.anchor
            && !self.symbol.text_slots.iter().any(|s| &s.kind == kind)
        {
            issue("symbol-anchor-slot", "/symbol/anchor".into(), self.key.clone());
        }
        if matches!(self.symbol.anchor, AnchorPolicy::Unknown(_)) {
            issue("symbol-anchor-slot", "/symbol/anchor".into(), self.key.clone());
        }
        let mut names = BTreeSet::new();
        for point in &self.symbol.link_points {
            if !names.insert(point.name.as_str()) {
                issue(
                    "symbol-link-duplicate",
                    "/symbol/link_points".into(),
                    point.name.to_string(),
                );
            }
        }
        // Alternative drawing sets need their own mapping review; never silently publish an unedited variant.
        if !self.symbol.extra_variants.is_empty() {
            issue(
                "symbol-extra-variants",
                "/symbol/extra_variants".into(),
                self.key.clone(),
            );
        }
        for (i, p) in self.symbol.graphics.iter().enumerate() {
            let bad = match p {
                Primitive::Unknown(_) | Primitive::Image(_) => true,
                Primitive::Circle(c) => c.r.0 <= 0,
                Primitive::Rect(p) => {
                    p.rect.0.min().x == p.rect.0.max().x || p.rect.0.min().y == p.rect.0.max().y
                }
                Primitive::Ellipse(p) => {
                    p.rect.0.min().x == p.rect.0.max().x || p.rect.0.min().y == p.rect.0.max().y
                }
                Primitive::Arc(p) => {
                    p.sweep.0 == 0
                        || p.rect.0.min().x == p.rect.0.max().x
                        || p.rect.0.min().y == p.rect.0.max().y
                }
                Primitive::Polyline(p) => p.points.len() < 2,
                Primitive::Polygon(p) => p.points.len() < 3,
                _ => false,
            };
            if bad {
                issue(
                    "symbol-graphic-invalid",
                    format!("/symbol/graphics/{i}"),
                    (i + 1).to_string(),
                );
            }
        }
        // Bound arithmetic before scene lowering; do not traverse arbitrary preserved plugin metadata.
        let geometry = json!({"graphics": self.symbol.graphics, "cps": self.symbol.cps,
            "slots": self.symbol.text_slots, "links": self.symbol.link_points, "anchor": self.symbol.anchor});
        if !bounded(&geometry, 0) {
            issue("symbol-geometry-limit", "/symbol".into(), self.key.clone());
        }
        for arrangement in &self.symbol.arrangements {
            if arrangement
                .slots
                .iter()
                .any(|i| usize::from(*i) >= self.symbol.text_slots.len())
            {
                issue(
                    "symbol-slot-arrangement",
                    "/symbol/arrangements".into(),
                    arrangement.name.to_string(),
                );
            }
        }
        out
    }

    /// Preview uses exactly the placement renderer's variant transforms. Refuses unsafe bounds.
    ///
    /// # Errors
    /// Out-of-bounds geometry cannot be lowered safely.
    pub fn preview(&self, variant: u8) -> Result<SymbolGeometry, Error> {
        if self
            .validate()
            .iter()
            .any(|i| matches!(i.code, "symbol-geometry-limit" | "symbol-input-limit"))
        {
            return Err(input("symbol-geometry-limit"));
        }
        let orientations = [
            units::Orient2::R0,
            units::Orient2::R90,
            units::Orient2::R180,
            units::Orient2::R270,
            units::Orient2::M0,
            units::Orient2::M90,
            units::Orient2::M180,
            units::Orient2::M270,
        ];
        Ok(lower(&self.symbol, orientations[usize::from(variant % 8)]))
    }

    /// Materialise ordinary symbol/FnDef copies for a target library, without writing anything.
    ///
    /// # Errors
    /// Any save-blocking validation issue.
    pub fn documents(&self, library: Uuid) -> Result<Vec<LibDocument>, Error> {
        let issues = self.validate();
        if !issues.is_empty() {
            return Err(Error::Invalid(issues));
        }
        let mut symbol = self.symbol.clone();
        symbol.meta.key = Some(self.key.parse().map_err(input)?);
        let mut docs = Vec::new();
        symbol.fndef = if let Some(r) = &self.function_ref {
            r.clone()
        } else {
            let key = format!("{}.function", self.key);
            let mut function = self.function.clone();
            function.meta = symbol.meta.clone();
            function.meta.key = Some(key.parse().map_err(input)?);
            docs.push(copy("fndef", library, &key, &self.version, &function)?);
            reference(library, &key, &self.version)
        };
        docs.push(copy("symbol", library, &self.key, &self.version, &symbol)?);
        Ok(docs)
    }

    /// Publish through the library's existing lock and generation transaction.
    ///
    /// # Errors
    /// Validation, unresolved assigned function, stale function, or the writer's lock/version/I/O errors.
    pub fn publish(&self, registry: &mut Registry, library: Uuid) -> Result<WriteReport, Error> {
        self.check_function(registry)?;
        let docs = self.documents(library)?;
        registry.write(library, &docs).map_err(Error::Library)
    }

    /// Verify an assigned function before publishing or placing the draft.
    ///
    /// # Errors
    /// The function is unavailable or the portable snapshot differs from the referenced definition.
    pub fn check_function(&self, registry: &Registry) -> Result<(), Error> {
        if let Some(r) = &self.function_ref {
            match registry.resolve(r).map_err(Error::Library)? {
                LibDocument::Fndef(f) if f.item == self.function => {}
                _ => return Err(input("symbol-function-changed")),
            }
        }
        Ok(())
    }

    /// Fork a library symbol and its function into a portable, independent draft.
    ///
    /// # Errors
    /// Missing function definition or invalid new key.
    pub fn fork(registry: &Registry, symbol: &LibCopy<SymbolDef>, key: &str) -> Result<Self, Error> {
        let LibDocument::Fndef(function) = registry.resolve(&symbol.item.fndef).map_err(Error::Library)?
        else {
            return Err(input("symbol-function-required"));
        };
        let mut draft = Self::new(key, key)?;
        draft.symbol = symbol.item.clone();
        draft.function = function.item;
        draft.symbol.meta.key = Some(key.parse().map_err(input)?);
        Ok(draft)
    }
}

fn bounded(v: &Value, depth: usize) -> bool {
    if depth > 64 {
        return false;
    }
    match v {
        Value::Number(n) => n.as_i64().is_some_and(|n| n.unsigned_abs() <= 1_000_000_000_000),
        Value::Array(a) => a.len() <= MAX_ITEMS && a.iter().all(|v| bounded(v, depth + 1)),
        Value::Object(o) => o.len() <= MAX_ITEMS && o.values().all(|v| bounded(v, depth + 1)),
        Value::String(s) => s.len() <= 65_536,
        _ => true,
    }
}

fn reference(library: Uuid, key: &str, version: &str) -> LibRef {
    LibRef {
        library: Id::from_uuid(library),
        item: v5::imported_id(library, key),
        version_req: format!("={version}"),
        extra: Default::default(),
    }
}

fn copy<T: Serialize>(
    kind: &str,
    library: Uuid,
    key: &str,
    version: &str,
    item: &T,
) -> Result<LibDocument, Error> {
    let item_id = v5::imported_id::<()>(library, key).as_uuid();
    let mut value = serde_json::to_value(item).map_err(input)?;
    let map = value
        .as_object_mut()
        .ok_or_else(|| input("symbol-input-object"))?;
    map.extend(
        json!({"$k": kind, "id": v5::lib_copy_id::<()>(library, item_id),
        "library_id": library, "item_id": item_id, "version": version,
        "sha256": format!("sha256:{}", "0".repeat(64))})
        .as_object()
        .expect("object")
        .clone(),
    );
    let (mut doc, malformed) = LibDocument::load(value).map_err(input)?;
    if !malformed.is_empty() {
        return Err(input("symbol-input-object"));
    }
    let hash = ew_store::ew_format::library_copy_hash(&doc).ok_or_else(|| input("symbol-input-object"))?;
    match &mut doc {
        LibDocument::Symbol(s) => s.sha256 = Sha256(hash),
        LibDocument::Fndef(f) => f.sha256 = Sha256(hash),
        _ => return Err(input("symbol-input-object")),
    }
    Ok(doc)
}

/// Read only a regular file, with a bound enforced while reading as well as before it.
///
/// # Errors
/// A link, device, oversized input, or I/O failure.
pub fn read_file(path: &Path) -> Result<Vec<u8>, Error> {
    read_bounded_file(path, MAX_BYTES)
}

fn read_bounded_file(path: &Path, limit: u64) -> Result<Vec<u8>, Error> {
    let meta = std::fs::symlink_metadata(path).map_err(input)?;
    if !meta.file_type().is_file() || meta.len() > limit {
        return Err(input("symbol-input-limit"));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(input)?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(input)?;
    if bytes.len() as u64 > limit {
        return Err(input("symbol-input-limit"));
    }
    Ok(bytes)
}

/// JSON Schema for the authoring envelope, using the released types.
pub fn json_schema() -> Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(SymbolDraft)).expect("schema serialises");
    schema["properties"]["format"]["const"] = json!(FORMAT);
    schema["properties"]["key"]["pattern"] = json!("^[A-Za-z0-9._-]{1,128}$");
    schema["properties"]["version"]["pattern"] =
        json!("^(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)$");
    schema
}

/// Import a QElectroTech element as an independent editable draft, preserving its grid and provenance.
///
/// # Errors
/// Invalid or oversized XML, or an unresolved built-in function.
pub fn import_qet(bytes: &[u8], name: &str, key: &str) -> Result<SymbolDraft, Error> {
    use ew_import_qet::ew_interop_api::{Limits, LineageInfo};
    if bytes.len() as u64 > MAX_BYTES {
        return Err(input("symbol-input-limit"));
    }
    let xml = std::str::from_utf8(bytes).map_err(input)?;
    let source = ew_import_qet::ElementSource::new(name, None).map_err(input)?;
    let lineage = LineageInfo {
        id: Id::from_uuid(DRAFT_LIBRARY),
        namespace: DRAFT_LIBRARY,
    };
    let imported = ew_import_qet::read_element(
        xml,
        &source,
        &lineage,
        Id::from_uuid(DRAFT_LIBRARY),
        &Limits::default(),
    )
    .map_err(input)?;
    let function = match imported.fndef {
        Some(f) => f.item,
        None => match Registry::starter_only()
            .resolve(&imported.symbol.item.fndef)
            .map_err(Error::Library)?
        {
            LibDocument::Fndef(f) => f.item,
            _ => return Err(input("symbol-function-required")),
        },
    };
    let mut draft = SymbolDraft::new(key, key)?;
    draft.symbol = imported.symbol.item;
    draft.function = function;
    if !imported.entries.is_empty() {
        draft.import_notes.push("symbol-qet-review".into());
    }
    draft.import_details = imported
        .entries
        .into_iter()
        .map(|entry| ImportDetail {
            code: entry.code.to_string(),
            message: entry.message,
            line: entry.location.and_then(|location| location.line),
        })
        .collect();
    draft.symbol.meta.key = Some(key.parse().map_err(input)?);
    Ok(draft)
}

/// Open portable JSON, a QElectroTech element, or EPLAN input. Archive candidates stay separate.
///
/// # Errors
/// Unsupported input, parsing, resource limits or unsafe file type.
pub fn open(path: &Path, key: &str) -> Result<Vec<SymbolDraft>, Error> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext == "slk" {
        use sha2::{Digest, Sha256};
        let descriptor = read_bounded_file(path, 8192)?;
        let directory = ew_import_eplan::slk::directory(&descriptor).map_err(input)?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .canonicalize()
            .map_err(input)?;
        let root = parent
            .join(directory)
            .canonicalize()
            .map_err(|_| input("eplan-slk-companion"))?;
        if root.parent() != Some(parent.as_path()) {
            return Err(input("eplan-archive-path"));
        }
        let mut files = std::collections::BTreeMap::new();
        let mut hash = Sha256::new();
        hash.update(&descriptor);
        let mut total = 0u64;
        for name in ew_import_eplan::slk::FILES {
            let file = root.join(name);
            if file
                .canonicalize()
                .map_err(|_| input("eplan-slk-companion"))?
                .parent()
                != Some(root.as_path())
            {
                return Err(input("eplan-archive-path"));
            }
            let bytes = read_bounded_file(&file, ew_import_eplan::MAX_FILE)?;
            total += bytes.len() as u64;
            if total > ew_import_eplan::MAX_TOTAL {
                return Err(input("eplan-input-limit"));
            }
            hash.update(name.as_bytes());
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(&bytes);
            files.insert(name.to_owned(), bytes);
        }
        let imported = ew_import_eplan::slk::read(&files).map_err(input)?;
        return eplan_drafts(imported, key, &format!("{:x}", hash.finalize()));
    }
    let eplan = matches!(ext.as_str(), "edz" | "ema" | "ems" | "esl" | "slk" | "xml");
    let bytes = read_bounded_file(
        path,
        if eplan {
            ew_import_eplan::MAX_INPUT
        } else {
            MAX_BYTES
        },
    )?;
    let name = path.file_name().and_then(|e| e.to_str()).unwrap_or("symbol");
    if eplan {
        import_eplan(&bytes, name, key)
    } else if ext == "elmt" {
        Ok(vec![import_qet(&bytes, name, key)?])
    } else {
        Ok(vec![SymbolDraft::parse(&bytes)?])
    }
}

/// EPLAN input becomes reviewable drafts. Unresolved macro connections have an unknown direction,
/// so they cannot masquerade as verified electrical connection points.
///
/// # Errors
/// Any importer refusal or a recovered object outside the native format.
pub fn import_eplan(bytes: &[u8], name: &str, key: &str) -> Result<Vec<SymbolDraft>, Error> {
    use sha2::{Digest, Sha256};
    let hash = format!("{:x}", Sha256::digest(bytes));
    eplan_drafts(ew_import_eplan::read(bytes, name).map_err(input)?, key, &hash)
}

fn eplan_drafts(
    imported: Vec<ew_import_eplan::ImportedSymbol>,
    key: &str,
    hash: &str,
) -> Result<Vec<SymbolDraft>, Error> {
    imported.into_iter().enumerate().map(|(i, imported)| {
        let mut d = serde_json::to_value(SymbolDraft::new(&format!("{key}.{}", i+1), &imported.name)?).map_err(input)?;
        d["symbol"]["graphics"] = json!(imported.graphics);
        // No licence grant is inferred from a download. The author supplies the actual terms before publication.
        d["symbol"]["meta"]["licence"] = json!("");
        d["symbol"]["meta"]["foreign_terms"] = json!(true);
        d["symbol"]["meta"]["sources"] = json!([format!("eplan:sha256:{hash}")]);
        d["symbol"]["cps"] = json!(imported.pins.iter().enumerate().map(|(n,p)| json!({"idx":n+1,"fn_cp":n+1,"pos":p.position,"dir":p.direction.as_deref().unwrap_or("unassigned")})).collect::<Vec<_>>());
        d["function"]["cps"] = json!(imported.pins.iter().enumerate().map(|(n,p)| json!({"idx":n+1,"designation":p.designation})).collect::<Vec<_>>());
        d["import_notes"] = json!(imported.notes);
        SymbolDraft::parse(&serde_json::to_vec(&d).map_err(input)?)
    }).collect()
}
