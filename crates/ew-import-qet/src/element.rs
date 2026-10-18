// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reading one element definition into library copies ([`read_element`]), and the importer of standalone
//! `.elmt` files ([`QetElementImporter`]).

use std::collections::BTreeMap;

use ew_ids::{Id, Uuid, v5};
use ew_interop_api::{
    Detection, ImportBatch, ImportContext, ImportError, ImportInput, ImportReport, Importer, InputNameError,
    Limits, LineageInfo, ReportEntry, SourceInfo, SourceRef, check_file_name,
};
use ew_mltext::{Ident, LangTag, MlText};
use ew_schema::{
    AnchorPolicy, CpDef, Discipline, FnDef, FnFlags, ImportLineage, ItemMeta, KeyedList, LibCopy,
    LibDocument, LibItem, LibRef, Library, LineageAnchor, Project, Record, RepType, Set, Sha256, SymbolCp,
    SymbolDef,
};
use ew_units::Grid;
use roxmltree::{Document, Node, ParsingOptions};
use serde::Serialize;
use serde_json::Value;
use sha2::Digest;

use crate::canon;
use crate::decimal::Decimal;
use crate::fndef::{self, Resolution};
use crate::prims::{self, Parsed, PrimCtx};

/// The semantic version of every imported library item: imports have no version of their own.
pub const ITEM_VERSION: &str = "1.0.0";

/// The source key of the `ImportLineage` record an import creates (`v5(namespace, "qet:lineage")`).
pub const LINEAGE_KEY: &str = "qet:lineage";

/// Where a definition comes from: the file name (never a path) and, for a definition embedded in a
/// project, its collection path (`embed://import/…`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ElementSource<'a> {
    file: &'a str,
    path: Option<&'a str>,
}

impl<'a> ElementSource<'a> {
    /// The source `file` name and, for a definition embedded in a project, its collection `path`
    /// (`embed://…`, not validated).
    ///
    /// # Errors
    ///
    /// [`InputNameError`] if `file` is empty or is a path (see [`check_file_name`]).
    pub fn new(file: &'a str, path: Option<&'a str>) -> Result<ElementSource<'a>, InputNameError> {
        check_file_name(file)?;
        Ok(ElementSource { file, path })
    }

    /// The source file name (never a path).
    pub fn file(&self) -> &'a str {
        self.file
    }

    /// The definition's collection path inside a project, if any.
    pub fn path(&self) -> Option<&'a str> {
        self.path
    }
}

/// One imported definition.
#[derive(Clone, PartialEq, Debug)]
pub struct ElementImport {
    /// The symbol.
    pub symbol: LibCopy<SymbolDef>,
    /// The generated function definition; `None` when the symbol references a built-in one
    /// (`next_report`, `previous_report` → `ew.gen.ip`).
    pub fndef: Option<LibCopy<FnDef>>,
    /// Findings, in order.
    pub entries: Vec<ReportEntry>,
    /// The definition's `version` attribute.
    pub version: Option<String>,
    /// The definition's `link_type` (default `simple`).
    pub link_type: String,
    /// The identity key `qet:eldef:<hex>`.
    pub key: String,
}

/// Parses `xml` with the limits: DTDs refused, at most `max_xml_nodes` nodes, element depth at most
/// `max_xml_depth`, attribute values at most `max_attribute_bytes`.
fn parse_xml<'i>(xml: &'i str, limits: &Limits) -> Result<Document<'i>, ImportError> {
    let opts = ParsingOptions {
        allow_dtd: false,
        nodes_limit: limits.max_xml_nodes,
        ..ParsingOptions::default()
    };
    let doc = Document::parse_with_options(xml, opts).map_err(|e| match e {
        roxmltree::Error::NodesLimitReached => ImportError::LimitExceeded {
            what: "XML node count".into(),
            limit: u64::from(limits.max_xml_nodes),
        },
        roxmltree::Error::DtdDetected => ImportError::Malformed {
            message: "a DOCTYPE is not accepted".into(),
            line: Some(1),
        },
        e => ImportError::Malformed {
            message: e.to_string(),
            line: Some(e.pos().row),
        },
    })?;
    // Depth and attribute sizes, iteratively.
    let mut stack = vec![(doc.root_element(), 1u32)];
    while let Some((node, depth)) = stack.pop() {
        if depth > limits.max_xml_depth {
            return Err(ImportError::LimitExceeded {
                what: "XML depth".into(),
                limit: u64::from(limits.max_xml_depth),
            });
        }
        if node
            .attributes()
            .any(|a| a.value().len() > limits.max_attribute_bytes)
        {
            return Err(ImportError::LimitExceeded {
                what: "XML attribute size".into(),
                limit: limits.max_attribute_bytes as u64,
            });
        }
        stack.extend(node.children().filter(Node::is_element).map(|c| (c, depth + 1)));
    }
    Ok(doc)
}

/// The child element of `node` named `name`.
fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

/// SHA-256 over the canonical compact JSON of a library item (doc 05 §13.2).
fn item_sha256<T: Serialize>(item: &T) -> Sha256 {
    let json = serde_json::to_string(item).unwrap_or_default();
    Sha256(sha2::Sha256::digest(json.as_bytes()).into())
}

/// A library copy of `item` with ID `item_id` in the project's QET import library, whose `library_id` is
/// the project ID (doc 05 §13.2).
fn lib_copy<T: Serialize>(library: Id<Library>, item_id: Id<LibItem>, item: T) -> LibCopy<T> {
    LibCopy {
        id: v5::lib_copy_id(library.as_uuid(), item_id.as_uuid()),
        v: 1,
        library_id: library,
        item_id,
        version: ITEM_VERSION.to_owned(),
        sha256: item_sha256(&item),
        item,
    }
}

/// Reads one element definition from `xml` into the lineage `lineage`, as library copies of the QET import
/// library of `project` (see the crate documentation for the mapping). The nesting depth and the attribute
/// counts are checked by a flat scan first ([`crate::xml::prescan`]) and the tree is parsed on a thread with
/// a stack sized for the depth ([`crate::xml`]).
///
/// # Errors
///
/// [`ImportError::Malformed`] for malformed XML or a DOCTYPE; [`ImportError::LimitExceeded`] for an exceeded
/// node, depth, attribute-count or attribute-size limit, or for a definition with more than 65,535
/// terminals (`what` is `"terminals per element definition"`; EW-IMP-001 for the file);
/// [`ImportError::Unsupported`] if the root is not `<definition type="element">`.
pub fn read_element(
    xml: &str,
    src: &ElementSource<'_>,
    lineage: &LineageInfo,
    project: Id<Project>,
    limits: &Limits,
) -> Result<ElementImport, ImportError> {
    let depth = crate::xml::prescan(xml, limits)?;
    let library: Id<Library> = Id::from_uuid(project.as_uuid());
    crate::xml::with_parser_stack(depth, || read_on_stack(xml, src, lineage, library, limits))
}

/// [`read_element`] after the pre-scan, on the parser thread.
fn read_on_stack(
    xml: &str,
    src: &ElementSource<'_>,
    lineage: &LineageInfo,
    library: Id<Library>,
    limits: &Limits,
) -> Result<ElementImport, ImportError> {
    let doc = parse_xml(xml, limits)?;
    let root = doc.root_element();
    if root.tag_name().name() != "definition" || root.attribute("type") != Some("element") {
        return Err(ImportError::Unsupported {
            message: format!(
                "root element <{}> is not an element definition <definition type=\"element\">",
                root.tag_name().name()
            ),
        });
    }
    let version = root.attribute("version").map(str::to_owned);
    let link_type = root.attribute("link_type").unwrap_or("simple").to_owned();
    let uuid = child(root, "uuid")
        .and_then(|n| n.attribute("uuid"))
        .map(str::to_owned);

    // Every finding goes through `ctx`: located by its line index and capped per definition.
    let mut ctx = PrimCtx::new(src.file(), src.path(), Decimal::ZERO, Decimal::ZERO);

    // Names.
    let mut names: Vec<(LangTag, String)> = Vec::new();
    let mut first_name: Option<String> = None;
    if let Some(n) = child(root, "names") {
        for name in n
            .children()
            .filter(|c| c.is_element() && c.tag_name().name() == "name")
        {
            // The text nodes around a comment join, and white space collapses, as in the canonical XML.
            let raw: String = name
                .children()
                .filter(Node::is_text)
                .filter_map(|c| c.text())
                .collect();
            let text = canon::collapse_whitespace(&raw);
            if first_name.is_none() && !text.is_empty() {
                first_name = Some(text.clone());
            }
            let lang = name.attribute("lang").unwrap_or("");
            match LangTag::parse_lenient(lang) {
                Ok(tag) => names.push((tag, text)),
                Err(_) => ctx.unsupported(
                    name,
                    format!("name with language {lang:?} dropped: not a language tag"),
                ),
            }
        }
    }

    // Kind informations.
    let mut kind: BTreeMap<String, String> = BTreeMap::new();
    if let Some(k) = child(root, "kindInformations") {
        for info in k
            .children()
            .filter(|c| c.is_element() && c.tag_name().name() == "kindInformation")
        {
            if let Some(name) = info.attribute("name") {
                kind.insert(name.to_owned(), info.text().unwrap_or("").trim().to_owned());
            }
        }
    }

    // Other children.
    let mut parsed = Parsed::default();
    for node in root.children().filter(Node::is_element) {
        match node.tag_name().name() {
            "description" => prims::parse_description(node, &mut ctx, &mut parsed),
            "uuid" | "names" | "kindInformations" | "informations" | "elementInformations" => {}
            other => ctx.unsupported(node, format!("unsupported element <{other}> dropped")),
        }
    }

    // CP indices are `u16`.
    let too_many_terminals = || ImportError::LimitExceeded {
        what: "terminals per element definition".into(),
        limit: u64::from(u16::MAX),
    };
    if u16::try_from(parsed.terminals.len()).is_err() {
        return Err(too_many_terminals());
    }

    // Identity.
    let digest = canon::identity_sha256(root);
    let key = canon::identity_key(&digest);
    let fndef_key = format!("{key}#fndef");
    let symbol_item: Id<LibItem> = v5::imported_id(lineage.namespace, &key);
    let fndef_item: Id<LibItem> = v5::imported_id(lineage.namespace, &fndef_key);
    let source_ref = |k: &str| {
        serde_json::to_value(SourceRef {
            lineage: lineage.id,
            key: k.to_owned(),
            extra: BTreeMap::new(),
        })
        .unwrap_or(Value::Null)
    };

    let mut sources = vec![format!("qet.file:{}", src.file())];
    if let Some(p) = src.path() {
        sources.push(format!("qet.path:{p}"));
    }
    if let Some(n) = &first_name {
        sources.push(format!("qet.name:{n}"));
    }
    if let Some(u) = &uuid {
        sources.push(format!("qet.uuid:{u}"));
    }
    if let Some(v) = &version {
        sources.push(format!("qet.version:{v}"));
    }
    sources.push(format!("qet.key:{key}"));
    let meta = ItemMeta {
        sources,
        foreign_terms: true,
        ..ItemMeta::default()
    };

    // Function semantics.
    let resolution = fndef::resolve(&link_type, &kind, src.path(), parsed.terminals.len());
    let (fndef_ref, fndef_copy) = match resolution {
        Resolution::Builtin(builtin) => {
            let item: Id<LibItem> = Id::from_uuid(Uuid::new_v5(&crate::BUILTIN_LIBRARY, builtin.as_bytes()));
            let mut extra = BTreeMap::new();
            extra.insert("key".to_owned(), Value::String(builtin.to_owned()));
            let r = LibRef {
                library: Id::from_uuid(crate::BUILTIN_LIBRARY),
                item,
                version_req: "^1".to_owned(),
                extra,
            };
            (r, None)
        }
        Resolution::Generated(g) => {
            if let Some((code, severity, message)) = g.finding {
                ctx.report(root, code, severity, message);
            }
            let cps = parsed
                .terminals
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let idx = u16::try_from(i).map_err(|_| too_many_terminals())?;
                    let mut extra = BTreeMap::new();
                    extra.insert("qet.type".to_owned(), Value::String(t.kind.clone()));
                    let designation = match Ident::from_input(&t.name) {
                        Ok(d) => d,
                        Err(_) => {
                            extra.insert("qet.name".to_owned(), Value::String(t.name.clone()));
                            Ident::from_input(&(i + 1).to_string())
                                .unwrap_or_else(|_| unreachable!("a decimal is an identifier"))
                        }
                    };
                    Ok(CpDef {
                        idx,
                        designation,
                        alias: None,
                        discipline: Discipline::Electrical,
                        potential_role: g.potential_role.clone(),
                        source_output: false,
                        optional: false,
                        level: None,
                        side: None,
                        clamp: None,
                        physical: true,
                        extra,
                    })
                })
                .collect::<Result<Vec<_>, ImportError>>()?;
            // Terminals are numbered by definition order, so a duplicate `idx` cannot occur; `Malformed` is the
            // closest importer error (ImportError has no internal-error variant).
            let cps = KeyedList::from_vec(cps).map_err(|e| ImportError::Malformed {
                line: None,
                message: format!("generated function definition has duplicate connection points: {e}"),
            })?;
            let mut extra = BTreeMap::new();
            if !kind.is_empty() {
                let obj = kind
                    .iter()
                    .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                    .collect::<serde_json::Map<_, _>>();
                extra.insert("qet.kind".to_owned(), Value::Object(obj));
            }
            extra.insert("source_ref".to_owned(), source_ref(&fndef_key));
            let def = FnDef {
                meta: meta.clone(),
                category: g.category,
                contact_kind: g.contact_kind,
                cps,
                internal: g.internal,
                flags: FnFlags::default(),
                extra,
            };
            let r = LibRef {
                library,
                item: fndef_item,
                version_req: "^1".to_owned(),
                extra: BTreeMap::new(),
            };
            (r, Some(lib_copy(library, fndef_item, def)))
        }
    };

    // Symbol.
    let cps = parsed
        .terminals
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let idx = u16::try_from(i).map_err(|_| too_many_terminals())?;
            let mut extra = BTreeMap::new();
            if let Some(u) = &t.uuid {
                extra.insert("qet.uuid".to_owned(), Value::String(u.clone()));
            }
            Ok(SymbolCp {
                idx,
                fn_cp: idx,
                pos: t.pos,
                dir: t.dir.clone(),
                discipline: Discipline::Electrical,
                extra,
            })
        })
        .collect::<Result<Vec<_>, ImportError>>()?;
    // Terminals are numbered by definition order, so a duplicate `idx` cannot occur; `Malformed` is the
    // closest importer error (ImportError has no internal-error variant).
    let cps = KeyedList::from_vec(cps).map_err(|e| ImportError::Malformed {
        line: None,
        message: format!("generated symbol has duplicate connection points: {e}"),
    })?;
    let mut extra = BTreeMap::new();
    extra.insert("qet.link_type".to_owned(), Value::String(link_type.clone()));
    extra.insert("source_ref".to_owned(), source_ref(&key));
    let symbol = SymbolDef {
        meta,
        name: MlText::from_langs(names),
        keywords: Set::default(),
        rep_type: if link_type == "thumbnail" {
            RepType::Graphic
        } else {
            RepType::MultiLine
        },
        fndef: fndef_ref,
        grid: Grid::QET_10PX,
        graphics: parsed.graphics,
        cps,
        text_slots: parsed.slots,
        arrangements: Vec::new(),
        anchor: AnchorPolicy::Origin,
        link_points: Vec::new(),
        extra_variants: Vec::new(),
        iec60617_ref: None,
        iso1219_ref: None,
        extra,
    };

    Ok(ElementImport {
        symbol: lib_copy(library, symbol_item, symbol),
        fndef: fndef_copy,
        entries: ctx.take_entries(),
        version,
        link_type,
        key,
    })
}

/// Whether the first start tag of `head` is `<definition>` with a `type` attribute of `element`. Skips a
/// BOM, the XML declaration and other processing instructions, comments, a DOCTYPE and white space; a head
/// that ends first gives `false`.
fn root_is_element_definition(head: &[u8]) -> bool {
    fn skip_space(mut b: &[u8]) -> &[u8] {
        while let [c, rest @ ..] = b {
            if !matches!(c, b' ' | b'\t' | b'\n' | b'\r') {
                break;
            }
            b = rest;
        }
        b
    }
    /// The bytes after the first `end` in `b`.
    fn after<'b>(b: &'b [u8], end: &[u8]) -> Option<&'b [u8]> {
        let at = b.windows(end.len()).position(|w| w == end)?;
        b.get(at + end.len()..)
    }
    let mut rest = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
    loop {
        rest = skip_space(rest);
        if let Some(r) = rest.strip_prefix(b"<?") {
            let Some(r) = after(r, b"?>") else { return false };
            rest = r;
        } else if let Some(r) = rest.strip_prefix(b"<!--") {
            let Some(r) = after(r, b"-->") else { return false };
            rest = r;
        } else if let Some(r) = rest.strip_prefix(b"<!") {
            // A DOCTYPE, with an optional internal subset in brackets.
            let mut depth = 0u32;
            let mut end = None;
            for (i, c) in r.iter().enumerate() {
                match c {
                    b'[' => depth += 1,
                    b']' => depth = depth.saturating_sub(1),
                    b'>' if depth == 0 => {
                        end = Some(i + 1);
                        break;
                    }
                    _ => {}
                }
            }
            let Some(r) = end.and_then(|e| r.get(e..)) else {
                return false;
            };
            rest = r;
        } else {
            break;
        }
    }
    let Some(mut tag) = rest.strip_prefix(b"<definition") else {
        return false;
    };
    if !matches!(tag.first(), Some(b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/')) {
        return false;
    }
    // The attributes of the start tag.
    loop {
        tag = skip_space(tag);
        let Some(&c) = tag.first() else { return false };
        if c == b'>' || c == b'/' {
            return false;
        }
        let name_len = tag
            .iter()
            .position(|c| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | b'=' | b'>' | b'/'))
            .unwrap_or(tag.len());
        let (attr, after_name) = tag.split_at(name_len);
        let Some(after_eq) = skip_space(after_name).strip_prefix(b"=") else {
            return false;
        };
        let value_start = skip_space(after_eq);
        let Some((&quote, value)) = value_start.split_first() else {
            return false;
        };
        if quote != b'"' && quote != b'\'' {
            return false;
        }
        let Some(len) = value.iter().position(|&c| c == quote) else {
            return false;
        };
        let (val, after_val) = value.split_at(len);
        if attr == b"type" && val == b"element" {
            return true;
        }
        tag = after_val.get(1..).unwrap_or_default();
    }
}

/// The importer of standalone `.elmt` element definition files.
///
/// A file imported without a target lineage starts a new one anchored on the file's SHA-256
/// (`FirstImportSha256`, importer `"qet"`), whose `ImportLineage` record (ID `v5(namespace, "qet:lineage")`)
/// is part of the batch.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct QetElementImporter;

impl Importer for QetElementImporter {
    fn id(&self) -> &'static str {
        crate::IMPORTER_ID
    }

    fn version(&self) -> &'static str {
        crate::IMPORTER_VERSION
    }

    /// `Certain` when the root element in the head is `<definition type="element">` (after an optional BOM,
    /// XML declaration, processing instructions, comments, DOCTYPE and white space), so that a project
    /// embedding definitions is not claimed; otherwise `Maybe(30)` for a name ending in `.elmt`, else `No`.
    /// A head that ends before the root start tag is complete is judged by the name alone.
    fn detect(&self, head: &[u8], name: &str) -> Detection {
        if root_is_element_definition(head) {
            Detection::Certain
        } else if name.to_ascii_lowercase().ends_with(".elmt") {
            Detection::Maybe(30)
        } else {
            Detection::No
        }
    }

    fn import(&self, input: &ImportInput, ctx: &mut ImportContext) -> Result<ImportBatch, ImportError> {
        let size = input.bytes().len() as u64;
        if size > ctx.limits.max_input_bytes {
            return Err(ImportError::LimitExceeded {
                what: "input size".into(),
                limit: ctx.limits.max_input_bytes,
            });
        }
        let bytes = input
            .bytes()
            .strip_prefix(b"\xEF\xBB\xBF")
            .unwrap_or(input.bytes());
        let xml = std::str::from_utf8(bytes).map_err(|e| ImportError::Unsupported {
            message: format!("not UTF-8 at byte {}", e.valid_up_to()),
        })?;

        let mut records = Vec::new();
        let lineage = match ctx.lineage {
            Some(l) => l,
            None => {
                let namespace = v5::lineage_namespace(
                    crate::IMPORTER_ID,
                    &v5::LineageAnchor::FirstImportSha256(input.sha256()),
                )
                .unwrap_or_else(|_| unreachable!("\"qet\" is a valid importer name"));
                let id: Id<ImportLineage> = v5::imported_id(namespace, LINEAGE_KEY);
                records.push(Record::ImportLineage(ImportLineage {
                    id,
                    v: 1,
                    importer: Ident::from_input(crate::IMPORTER_ID)
                        .unwrap_or_else(|_| unreachable!("\"qet\" is an identifier")),
                    source_name: input.name().to_owned(),
                    anchor: LineageAnchor::FirstImportSha256(Sha256(input.sha256())),
                    namespace: Id::from_uuid(namespace),
                    last_source_sha256: Some(Sha256(input.sha256())),
                    last_run: None,
                    extra: BTreeMap::new(),
                }));
                LineageInfo { id, namespace }
            }
        };

        let src = ElementSource::new(input.name(), None).map_err(|e| ImportError::Unsupported {
            message: e.to_string(),
        })?;
        let el = read_element(xml, &src, &lineage, ctx.project, &ctx.limits)?;

        let mut report = ImportReport::new(
            crate::IMPORTER_ID,
            crate::IMPORTER_VERSION,
            SourceInfo {
                name: input.name().to_owned(),
                sha256: input.sha256(),
                size,
                format: "qet-element".to_owned(),
                format_version: el.version.clone(),
                detail: Some(format!("link type {}", el.link_type)),
            },
        );
        for e in el.entries {
            report.push(e);
        }
        let mut lib = vec![LibDocument::Symbol(el.symbol)];
        if let Some(f) = el.fndef {
            lib.push(LibDocument::Fndef(f));
        }
        lib.sort_by_key(|d| match d {
            LibDocument::Fndef(c) => (0, c.id.as_uuid()),
            LibDocument::Symbol(c) => (1, c.id.as_uuid()),
            _ => (2, Uuid::nil()),
        });
        for r in &records {
            *report
                .stats
                .per_kind
                .entry(r.kind_name().unwrap_or("?").to_owned())
                .or_default() += 1;
        }
        for d in &lib {
            let k = d.kind().map_or("?", |k| k.name());
            *report.stats.per_kind.entry(k.to_owned()).or_default() += 1;
        }
        Ok(ImportBatch { records, lib, report })
    }
}
