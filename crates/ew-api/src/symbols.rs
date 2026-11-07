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
    /// Stable human key. The item IDs are the draft's `identity`, not derived from the key.
    pub key: String,
    /// Semantic version; changing published content requires a new version.
    pub version: String,
    /// The library items this draft publishes. Absent: the first publication mints them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<DraftIdentity>,
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

/// The item IDs of a draft's library records, one per kind (ID-1463): UUIDv7s minted at the draft's first
/// publication, and kept only by a draft that comes from those items under their key. A draft whose key differs from
/// the key the target library holds for them publishes new items instead (ID-1474).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DraftIdentity {
    /// The symbol's item ID.
    #[schemars(with = "String")]
    pub symbol: Id<SymbolDef>,
    /// The owned function definition's item ID; absent until the draft publishes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub fndef: Option<Id<FnDef>>,
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
    /// The target library holds this key for another item of the kind: the draft does not come from it.
    KeyTaken {
        /// The symbol's key, or its owned function definition's.
        key: String,
        /// Whether the function definition's key is taken rather than the symbol's.
        function: bool,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input(s) => f.write_str(words(s).unwrap_or(s)),
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
            Self::KeyTaken { key, function: false } => write!(
                f,
                "The library already holds a different symbol with the key {key}. Choose another key."
            ),
            Self::KeyTaken { key, function: true } => write!(
                f,
                "The library already holds a different function definition with the key {key}. Choose another \
                 symbol key."
            ),
        }
    }
}
impl std::error::Error for Error {}

fn input(e: impl std::fmt::Display) -> Error {
    Error::Input(e.to_string())
}

/// The English words of a refusal key of the readers and importers, as the window's en-GB text has them, for every
/// caller that prints an [`Error`], the CLI among them (SM25#3); `None` for a text that is no such key.
fn words(key: &str) -> Option<&'static str> {
    Some(match key {
        "symbol-input-limit" => "The symbol file exceeds the supported size or item limit.",
        "symbol-input-version" => "This symbol authoring format is not supported.",
        "symbol-input-object" => "The symbol document is not valid.",
        "symbol-function-required" => "Assign a function definition to this symbol.",
        "symbol-function-changed" => {
            "The assigned function definition changed. Assign it again before saving."
        }
        "symbol-geometry-limit" => {
            "The symbol geometry exceeds the supported coordinate or complexity limit."
        }
        "symbol-import-stopped" => "The importer stopped on an internal error; nothing was imported.",
        "eplan-input-limit" => "The EPLAN input exceeds the supported size or complexity.",
        "eplan-archive-path" => "The EPLAN archive has unsafe or duplicate member names.",
        "eplan-archive-invalid" => "The EPLAN archive is damaged, encrypted or unsupported.",
        "eplan-archive-method" => "This archive compression method is unsupported.",
        "eplan-binary-unsupported" => {
            "This binary EPLAN library or macro is unsupported. Export a PXF XML macro or parts XML from EPLAN."
        }
        "eplan-xml-invalid" => "The EPLAN XML is malformed or contains forbidden declarations.",
        "eplan-geometry-invalid" => "The EPLAN drawing contains invalid coordinates.",
        "eplan-pxf-version" => "This EPLAN PXF version is unsupported; the reader supports revision 1.23.",
        "eplan-no-symbols" => "No supported symbol drawing or connection template was found.",
        "eplan-format-unsupported" => "This EPLAN document type is unsupported.",
        "eplan-slk-invalid" => "The symbol-library database is damaged or has an unsupported record layout.",
        "eplan-slk-version" => "This EPLAN symbol-library database version is not supported.",
        "eplan-slk-encoding" => {
            "This older SLK file uses an unsupported text encoding. UTF-16 and UTF-8 descriptors are supported."
        }
        "eplan-slk-companion" => {
            "Keep the SLK file beside its complete .sdb folder, then open it again. A required drawing table is missing."
        }
        "eplan-slk-record-unsupported" => {
            "The symbol-library database contains an unsupported record encoding."
        }
        _ => return None,
    })
}

/// What a symbol import returns through the front door: its drafts, or its own refusal.
struct Drafts(Result<Vec<SymbolDraft>, Error>);

impl crate::import::FrontDoorOutput for Drafts {}

/// One symbol import through WP-S3.09's front door under `Limits::element_definition()` (M25-p2#3): the sizes of
/// its inputs are checked before `run`, a size over the preset refused as `limit`, and a panic in `run` is refused
/// in words, so the window keeps its draft. `file` names the source.
fn front_door(
    file: &str,
    sizes: &[u64],
    limit: &str,
    run: impl FnOnce() -> Result<Vec<SymbolDraft>, Error>,
) -> Result<Vec<SymbolDraft>, Error> {
    use crate::import::{Limits, Refusal};
    match crate::import::front_door(file, sizes, &Limits::element_definition(), None, || {
        Ok(Drafts(run()))
    }) {
        Ok(Drafts(drafts)) => drafts,
        Err(Refusal::Import(_)) => Err(input(limit)),
        Err(_) => Err(input("symbol-import-stopped")),
    }
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
            d.identity = kept_identity(s, function, key);
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
    export_with(&ew_store::OsVfs, draft, path)
}

/// [`export`] through `vfs`: the temporary sibling is synced, moved by [`ew_store::Vfs::rename_noreplace`] (where
/// the file system has no hard links, by [`ew_store::Vfs::rename`] after the name is checked absent again, as the
/// project store does) and the folder synced. An existing destination is `symbol-export-exists`; any failure leaves
/// neither the temporary file nor a file of this call at `path`.
///
/// # Errors
/// As [`export`].
pub fn export_with(vfs: &dyn ew_store::Vfs, draft: &SymbolDraft, path: &Path) -> Result<(), Error> {
    use std::io::ErrorKind;
    let bytes = serde_json::to_vec_pretty(draft).map_err(input)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let id = Id::<()>::try_new_v7().map_err(input)?;
    let temporary = parent.join(format!(".symbol-{id}.tmp"));
    vfs.create_new(&temporary, &bytes).map_err(input)?;
    // Whether this call put the file at `path`: only that file is ever removed again.
    let mut ours = false;
    let placed = vfs
        .sync(&temporary)
        .and_then(|()| match vfs.rename_noreplace(&temporary, path) {
            Ok(()) => {
                ours = true;
                Ok(())
            }
            Err(e) if e.kind() == ErrorKind::Unsupported => match vfs.stat(path) {
                Ok(_) => Err(ErrorKind::AlreadyExists.into()),
                Err(e) if e.kind() == ErrorKind::NotFound => {
                    // A failed rename moved nothing.
                    vfs.rename(&temporary, path)?;
                    ours = true;
                    Ok(())
                }
                Err(e) => Err(e),
            },
            Err(e) if e.kind() == ErrorKind::AlreadyExists => Err(e),
            Err(e) => {
                // A move by link and unlink can fail after the link, which only succeeds where nothing was: then the file
                // there is ours.
                ours = vfs.read(path).is_ok_and(|held| held == bytes);
                Err(e)
            }
        });
    let placed = placed.and_then(|()| vfs.sync_dir(parent));
    let _ = vfs.remove_file(&temporary);
    match placed {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => Err(input("symbol-export-exists")),
        Err(e) => {
            // An unsynced name of a file this call placed: it goes too.
            if ours {
                let _ = vfs.remove_file(path);
                let _ = vfs.sync_dir(parent);
            }
            Err(input(e))
        }
    }
}

/// Preview graphics with text shaped identically to page output. Slot contents are illustrative except pin designations;
/// `slot_placeholder` names the other slots by their kind.
pub fn preview_display_list(
    draft: &SymbolDraft,
    geometry: &SymbolGeometry,
    styles: &TextStyleTable,
    slot_placeholder: &dyn Fn(&schema::SlotKind) -> String,
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
            TextSource::Slot(kind) => slot_placeholder(kind),
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
    /// A new native-grid draft with a private, empty black-box function definition. An empty key is allowed: the
    /// draft then cannot be published until it has one.
    ///
    /// # Errors
    /// Invalid text in the supplied name or key.
    pub fn new(key: &str, name: &str) -> Result<Self, Error> {
        let reference = reference(DRAFT_LIBRARY, &format!("{key}.function"), "1.0.0");
        let mut meta = json!({"licence": "CC-BY-SA-4.0", "sources": ["original"], "status": "draft"});
        if !key.is_empty() {
            meta["key"] = json!(key);
        }
        serde_json::from_value(json!({
            "format": FORMAT, "key": key, "version": "1.0.0",
            "symbol": {
                "meta": meta,
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
        // Bytes that are no text (an archive, say) are no authoring document: refused in words, not by the decoder.
        let text = std::str::from_utf8(bytes).map_err(|_| input("symbol-input-object"))?;
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
        if self.key.is_empty() {
            issue("symbol-key-required", "/key".into(), String::new());
        } else if self.key.len() > 128
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
        for (i, cp) in self.function.cps.iter().enumerate() {
            if matches!(cp.discipline, schema::Discipline::Unknown(_))
                || matches!(cp.potential_role, Some(schema::PotentialType::Unknown(_)))
                || matches!(cp.side, Some(schema::Side::Unknown(_)))
                || matches!(cp.port_role, Some(schema::FluidPortRole::Unknown(_)))
            {
                issue(
                    "symbol-function-semantics",
                    format!("/function/cps/{i}"),
                    cp.designation.to_string(),
                );
            }
        }
        for (i, cp) in self.symbol.cps.iter().enumerate() {
            if matches!(cp.discipline, schema::Discipline::Unknown(_)) {
                issue(
                    "symbol-function-semantics",
                    format!("/symbol/cps/{i}"),
                    cp.idx.to_string(),
                );
            }
        }
        // A member this reader does not know is refused before publication (§6), except the importers' extension keys.
        let mut members: Vec<(String, &schema::Extra)> = vec![
            ("/symbol".into(), &self.symbol.extra),
            ("/symbol/meta".into(), &self.symbol.meta.extra),
            ("/function".into(), &self.function.extra),
            ("/function/meta".into(), &self.function.meta.extra),
        ];
        let symbol_cps = self.symbol.cps.iter().map(|c| &c.extra);
        let function_cps = self.function.cps.iter().map(|c| &c.extra);
        members.extend(
            symbol_cps
                .enumerate()
                .map(|(i, e)| (format!("/symbol/cps/{i}"), e)),
        );
        members.extend(
            function_cps
                .enumerate()
                .map(|(i, e)| (format!("/function/cps/{i}"), e)),
        );
        // And of every nested object that publishes with them (M25-p4#5).
        for (i, p) in self.symbol.graphics.iter().enumerate() {
            let (tag, extra, stroke) = match p {
                Primitive::Line(x) => ("line", &x.extra, x.stroke.as_ref()),
                Primitive::Polyline(x) => ("polyline", &x.extra, x.stroke.as_ref()),
                Primitive::Rect(x) => ("rect", &x.extra, x.stroke.as_ref()),
                Primitive::Circle(x) => ("circle", &x.extra, x.stroke.as_ref()),
                Primitive::Ellipse(x) => ("ellipse", &x.extra, x.stroke.as_ref()),
                Primitive::Arc(x) => ("arc", &x.extra, x.stroke.as_ref()),
                Primitive::Polygon(x) => ("polygon", &x.extra, x.stroke.as_ref()),
                Primitive::Text(x) => ("text", &x.extra, None),
                Primitive::Image(x) => ("image", &x.extra, None),
                _ => continue,
            };
            members.push((format!("/symbol/graphics/{i}/{tag}"), extra));
            if let Some(s) = stroke {
                members.push((format!("/symbol/graphics/{i}/{tag}/stroke"), &s.extra));
            }
        }
        let slots = self.symbol.text_slots.iter().map(|s| &s.extra);
        members.extend(
            slots
                .enumerate()
                .map(|(i, e)| (format!("/symbol/text_slots/{i}"), e)),
        );
        let arrangements = self.symbol.arrangements.iter().map(|a| &a.extra);
        members.extend(
            arrangements
                .enumerate()
                .map(|(i, e)| (format!("/symbol/arrangements/{i}"), e)),
        );
        let links = self.symbol.link_points.iter().map(|l| &l.extra);
        members.extend(
            links
                .enumerate()
                .map(|(i, e)| (format!("/symbol/link_points/{i}"), e)),
        );
        for (i, link) in self.function.internal.iter().enumerate() {
            members.push((format!("/function/internal/{i}"), &link.extra));
            if let schema::InternalKind::PassThrough(spec) = &link.kind {
                members.push((format!("/function/internal/{i}/kind/pass_through"), &spec.extra));
            }
        }
        members.push(("/function/flags".into(), &self.function.flags.extra));
        if let Some(r) = &self.function_ref {
            members.push(("/function_ref".into(), &r.extra));
        }
        for (path, extra) in members {
            for name in extra
                .keys()
                .filter(|k| !k.starts_with("qet.") && k.as_str() != "source_ref")
            {
                let escaped = name.replace('~', "~0").replace('/', "~1");
                issue("symbol-unknown-member", format!("{path}/{escaped}"), name.clone());
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
        for (i, cp) in self.symbol.cps.iter().enumerate() {
            let path = format!("/symbol/cps/{i}");
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

    /// Materialise ordinary symbol/FnDef copies for a target library, without writing anything. A draft without an
    /// identity gets new item IDs on every call.
    ///
    /// # Errors
    /// Any save-blocking validation issue.
    pub fn documents(&self, library: Uuid) -> Result<Vec<LibDocument>, Error> {
        self.materialise(library, &self.publication_identity()?, &self.version)
    }

    /// This draft's identity, with a new UUIDv7 for each item it publishes that has none yet.
    fn publication_identity(&self) -> Result<DraftIdentity, Error> {
        let mut identity = match &self.identity {
            Some(identity) => identity.clone(),
            None => DraftIdentity {
                symbol: Id::try_new_v7().map_err(input)?,
                fndef: None,
            },
        };
        if self.function_ref.is_none() && identity.fndef.is_none() {
            identity.fndef = Some(Id::try_new_v7().map_err(input)?);
        }
        Ok(identity)
    }

    fn materialise(
        &self,
        library: Uuid,
        identity: &DraftIdentity,
        function_version: &str,
    ) -> Result<Vec<LibDocument>, Error> {
        let issues = self.validate();
        if !issues.is_empty() {
            return Err(Error::Invalid(issues));
        }
        let mut symbol = self.symbol.clone();
        symbol.meta.key = Some(self.key.parse().map_err(input)?);
        let mut docs = Vec::new();
        symbol.fndef = match (&self.function_ref, identity.fndef) {
            (Some(r), _) => r.clone(),
            (None, item) => {
                let item = item.ok_or_else(|| input("symbol-function-required"))?.as_uuid();
                let key = format!("{}.function", self.key);
                let mut function = self.function.clone();
                function.meta = symbol.meta.clone();
                function.meta.key = Some(key.parse().map_err(input)?);
                docs.push(copy("fndef", library, item, function_version, &function)?);
                LibRef {
                    library: Id::from_uuid(library),
                    item: Id::from_uuid(item),
                    version_req: format!("={function_version}"),
                    extra: Default::default(),
                }
            }
        };
        let item = identity.symbol.as_uuid();
        docs.push(copy("symbol", library, item, &self.version, &symbol)?);
        Ok(docs)
    }

    /// Publish through the library's existing lock and generation transaction. The first publication mints the
    /// draft's identity and keeps it in the draft. A key the library holds for another item is refused; the owned
    /// function definition keeps the version the library holds while its content is unchanged (ID-1463). An item of
    /// the identity that the library holds under another key is not renamed: the draft publishes a new item with a
    /// new identity, which it takes, and the report names it as added (ID-1474).
    ///
    /// # Errors
    /// Validation, unresolved assigned function, stale function, a key of another item, or the writer's
    /// lock/version/I/O errors. The draft and the library are then unchanged.
    pub fn publish(&mut self, registry: &mut Registry, library: Uuid) -> Result<WriteReport, Error> {
        self.check_function(registry)?;
        let mut identity = self.publication_identity()?;
        let function_key = format!("{}.function", self.key);
        let held = registry.documents(library, schema::Kind::Symbol);
        let held: Vec<_> = held
            .into_iter()
            .chain(registry.documents(library, schema::Kind::Fndef))
            .collect();
        // Whether the library holds `item` under a key other than `key`.
        let elsewhere = |item: Uuid, key: &str| {
            held.iter().any(|d| match d {
                LibDocument::Symbol(s) => s.item_id.as_uuid() == item && !own_key(&s.item.meta, key),
                LibDocument::Fndef(f) => f.item_id.as_uuid() == item && !own_key(&f.item.meta, key),
                _ => false,
            })
        };
        if elsewhere(identity.symbol.as_uuid(), &self.key) {
            identity = DraftIdentity {
                symbol: Id::try_new_v7().map_err(input)?,
                fndef: None,
            };
        }
        if identity
            .fndef
            .is_some_and(|f| elsewhere(f.as_uuid(), &function_key))
        {
            identity.fndef = None;
        }
        if self.function_ref.is_none() && identity.fndef.is_none() {
            identity.fndef = Some(Id::try_new_v7().map_err(input)?);
        }
        let mut docs = self.materialise(library, &identity, &self.version)?;
        let owned = identity.fndef.filter(|_| self.function_ref.is_none());
        let mut held_function = None;
        for doc in held {
            let (item, key, function, ours) = match &doc {
                LibDocument::Symbol(s) => (
                    s.item_id,
                    &s.item.meta.key,
                    false,
                    Some(identity.symbol.as_uuid()),
                ),
                LibDocument::Fndef(f) => (f.item_id, &f.item.meta.key, true, owned.map(|i| i.as_uuid())),
                _ => continue,
            };
            let wanted = if function { &function_key } else { &self.key };
            let same_key = key.as_ref().is_some_and(|k| k.as_str() == wanted);
            if ours.is_some() && Some(item.as_uuid()) != ours && same_key {
                return Err(Error::KeyTaken {
                    key: wanted.clone(),
                    function,
                });
            }
            if let LibDocument::Fndef(f) = doc
                && Some(item.as_uuid()) == ours
            {
                held_function = Some(f);
            }
        }
        let new_function = docs.iter().find_map(|d| match d {
            LibDocument::Fndef(f) => Some(&f.item),
            _ => None,
        });
        if let Some(held) =
            held_function.filter(|h| Some(&h.item) == new_function && h.version != self.version)
        {
            docs = self.materialise(library, &identity, &held.version)?;
        }
        let report = registry.write(library, &docs).map_err(Error::Library)?;
        self.identity = Some(identity);
        Ok(report)
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
        draft.identity = kept_identity(symbol, &function, key);
        draft.symbol = symbol.item.clone();
        draft.function = function.item;
        draft.symbol.meta.key = Some(key.parse().map_err(input)?);
        Ok(draft)
    }
}

/// The identity a draft keeps from the symbol it comes from: only under the symbol's own key, and its function
/// definition's only when that is the symbol's own one (ID-1463). Another key makes a new item.
fn kept_identity(symbol: &LibCopy<SymbolDef>, function: &LibCopy<FnDef>, key: &str) -> Option<DraftIdentity> {
    let own = |k: Option<&ew_mltext::Ident>, expected: &str| k.is_some_and(|k| k.as_str() == expected);
    own(symbol.item.meta.key.as_ref(), key).then(|| DraftIdentity {
        symbol: Id::from_uuid(symbol.item_id.as_uuid()),
        fndef: (function.library_id == symbol.library_id
            && own(function.item.meta.key.as_ref(), &format!("{key}.function")))
        .then(|| Id::from_uuid(function.item_id.as_uuid())),
    })
}

/// Whether an item's metadata names it `key`.
fn own_key(meta: &schema::ItemMeta, key: &str) -> bool {
    meta.key.as_ref().is_some_and(|k| k.as_str() == key)
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
    item_id: Uuid,
    version: &str,
    item: &T,
) -> Result<LibDocument, Error> {
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
    front_door(name, &[bytes.len() as u64], "symbol-input-limit", || {
        qet(bytes, name, key).map(|d| vec![d])
    })?
    .pop()
    .ok_or_else(|| input("symbol-input-object"))
}

fn qet(bytes: &[u8], name: &str, key: &str) -> Result<SymbolDraft, Error> {
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
        &Limits::element_definition(),
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

/// The key of a symbol imported from `path` without one (ID-1463: no shared default key): the file's name before its
/// first dot (the whole name when that is empty), each character other than an ASCII letter, a digit, `_` or `-`
/// replaced by `_`.
pub fn file_key(path: &Path) -> String {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let stem = name.split('.').next().filter(|s| !s.is_empty()).unwrap_or(&name);
    stem.chars()
        .take(100)
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
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
        // NFR-SEC-18 as ID-1461 amends it: the `.sdb` folder and its tables are read as themselves, never through a
        // link, whatever it points to.
        let link = |p: &Path| {
            std::fs::symlink_metadata(p)
                .map(|m| m.file_type().is_symlink())
                .map_err(|_| input("eplan-slk-companion"))
        };
        let folder = parent.join(directory);
        if link(&folder)? {
            return Err(input("eplan-archive-path"));
        }
        let root = folder.canonicalize().map_err(|_| input("eplan-slk-companion"))?;
        if root.parent() != Some(parent.as_path()) {
            return Err(input("eplan-archive-path"));
        }
        let mut files = std::collections::BTreeMap::new();
        let mut hash = Sha256::new();
        hash.update(&descriptor);
        let mut total = 0u64;
        let mut sizes = vec![descriptor.len() as u64];
        for name in ew_import_eplan::slk::FILES {
            let file = root.join(name);
            if link(&file)?
                || file
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
            sizes.push(bytes.len() as u64);
            files.insert(name.to_owned(), bytes);
        }
        let name = path.file_name().and_then(|e| e.to_str()).unwrap_or("symbol");
        let hash = format!("{:x}", hash.finalize());
        return front_door(name, &sizes, "eplan-input-limit", || {
            eplan_drafts(ew_import_eplan::slk::read(&files).map_err(input)?, key, &hash)
        });
    }
    let eplan = matches!(
        ext.as_str(),
        "edz" | "zip" | "7z" | "pxf" | "ema" | "ems" | "esl" | "slk" | "xml"
    );
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
        front_door(name, &[bytes.len() as u64], "symbol-input-limit", || {
            Ok(vec![SymbolDraft::parse(&bytes)?])
        })
    }
}

/// EPLAN input becomes reviewable drafts. Unresolved macro connections have an unknown direction,
/// so they cannot masquerade as verified electrical connection points.
///
/// # Errors
/// Any importer refusal or a recovered object outside the native format.
pub fn import_eplan(bytes: &[u8], name: &str, key: &str) -> Result<Vec<SymbolDraft>, Error> {
    use sha2::{Digest, Sha256};
    front_door(name, &[bytes.len() as u64], "eplan-input-limit", || {
        let hash = format!("{:x}", Sha256::digest(bytes));
        eplan_drafts(ew_import_eplan::read(bytes, name).map_err(input)?, key, &hash)
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    /// M25-p2#3: a panic inside a symbol import is the front door's refusal, in words.
    #[test]
    fn a_panic_in_a_symbol_import_is_refused_in_words() {
        let e = front_door("x.edz", &[1], "eplan-input-limit", || panic!("decoder fault")).unwrap_err();
        assert_eq!(
            e.to_string(),
            "The importer stopped on an internal error; nothing was imported."
        );
        let e = front_door("x.edz", &[17 * 1024 * 1024], "eplan-input-limit", || {
            Ok(Vec::new())
        })
        .unwrap_err();
        assert_eq!(
            e.to_string(),
            "The EPLAN input exceeds the supported size or complexity."
        );
    }

    /// SM25#3: the words are the window's en-GB texts of the same keys.
    #[test]
    fn the_words_are_the_windows_english() {
        let ftl = include_str!("../../ew-viewmodel/locales/en-GB/shell.ftl");
        let mut checked = 0;
        for (key, text) in ftl.lines().filter_map(|l| l.split_once(" = ")) {
            if let Some(w) = words(key) {
                assert_eq!(w, text, "{key}");
                checked += 1;
            }
        }
        assert!(checked > 0);
    }
}
