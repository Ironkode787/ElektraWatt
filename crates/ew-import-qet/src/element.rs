// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reading one element definition into library copies ([`read_element`]), and the importer of standalone
//! `.elmt` files ([`QetElementImporter`]).

use std::collections::BTreeMap;

use ew_ids::{Id, Uuid, v5};
use ew_interop_api::{
    Code, Detection, ImportBatch, ImportContext, ImportError, ImportInput, ImportReport, Importer, Limits,
    LineageInfo, ReportEntry, Severity, SourceInfo, SourceLocation, SourceRef,
};
use ew_mltext::{Ident, LangTag, MlText};
use ew_schema::{
    AnchorPolicy, CpDef, Discipline, FnDef, FnFlags, ImportLineage, ItemMeta, LibCopy, LibDocument, LibItem,
    LibRef, LineageAnchor, Record, RepType, Sha256, SymbolCp, SymbolDef,
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
    /// The source file name.
    pub file: &'a str,
    /// The definition's collection path inside a project, if any.
    pub path: Option<&'a str>,
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

/// A library copy of `item` with ID `item_id` in the QET import library.
fn lib_copy<T: Serialize>(item_id: Id<LibItem>, item: T) -> LibCopy<T> {
    LibCopy {
        id: v5::lib_copy_id(crate::QET_IMPORT_LIBRARY, item_id.as_uuid()),
        v: 1,
        library_id: Id::from_uuid(crate::QET_IMPORT_LIBRARY),
        item_id,
        version: ITEM_VERSION.to_owned(),
        sha256: item_sha256(&item),
        item,
    }
}

/// Reads one element definition from `xml` into the lineage `lineage` (see the crate documentation for the
/// mapping). The nesting depth is checked by a flat scan first and the tree is parsed on a thread with a
/// stack sized for it ([`crate::xml`]).
///
/// # Errors
///
/// [`ImportError::Malformed`] for malformed XML or a DOCTYPE; [`ImportError::LimitExceeded`] for an exceeded
/// node, depth or attribute limit; [`ImportError::Unsupported`] if the root is not
/// `<definition type="element">`.
pub fn read_element(
    xml: &str,
    src: &ElementSource<'_>,
    lineage: &LineageInfo,
    limits: &Limits,
) -> Result<ElementImport, ImportError> {
    let depth = crate::xml::max_depth(xml, limits.max_xml_depth).ok_or(ImportError::LimitExceeded {
        what: "XML depth".into(),
        limit: u64::from(limits.max_xml_depth),
    })?;
    crate::xml::with_parser_stack(depth, || read_on_stack(xml, src, lineage, limits))
}

/// [`read_element`] after the depth guard, on the parser thread.
fn read_on_stack(
    xml: &str,
    src: &ElementSource<'_>,
    lineage: &LineageInfo,
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

    let mut ctx = PrimCtx::new(src.file, src.path, Decimal::ZERO, Decimal::ZERO);
    let mut entries: Vec<ReportEntry> = Vec::new();
    let location = |node: Node<'_, '_>| SourceLocation {
        file: src.file.to_owned(),
        member: src.path.map(str::to_owned),
        item: Some(node.tag_name().name().to_owned()),
        line: Some(node.document().text_pos_at(node.range().start).row),
        ..SourceLocation::default()
    };

    // Names.
    let mut names: Vec<(LangTag, String)> = Vec::new();
    let mut first_name: Option<String> = None;
    if let Some(n) = child(root, "names") {
        for name in n
            .children()
            .filter(|c| c.is_element() && c.tag_name().name() == "name")
        {
            let text = name.text().unwrap_or("").trim().to_owned();
            if first_name.is_none() && !text.is_empty() {
                first_name = Some(text.clone());
            }
            let lang = name.attribute("lang").unwrap_or("");
            match LangTag::parse_lenient(lang) {
                Ok(tag) => names.push((tag, text)),
                Err(_) => entries.push(
                    ReportEntry::new(
                        Code::DROPPED_BY_DESIGN,
                        Severity::Info,
                        format!("name with language {lang:?} dropped: not a language tag"),
                    )
                    .at(location(name)),
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
            other => entries.push(
                ReportEntry::new(
                    Code::DROPPED_BY_DESIGN,
                    Severity::Info,
                    format!("unsupported element <{other}> dropped"),
                )
                .at(location(node)),
            ),
        }
    }
    entries.append(&mut ctx.entries);

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

    let mut sources = vec![format!("qet.file:{}", src.file)];
    if let Some(p) = src.path {
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
    let resolution = fndef::resolve(&link_type, &kind, src.path, parsed.terminals.len());
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
                entries.push(ReportEntry::new(code, severity, message).at(SourceLocation {
                    file: src.file.to_owned(),
                    member: src.path.map(str::to_owned),
                    item: Some("definition".to_owned()),
                    line: Some(root.document().text_pos_at(root.range().start).row),
                    ..SourceLocation::default()
                }));
            }
            let cps = parsed
                .terminals
                .iter()
                .enumerate()
                .map(|(i, t)| {
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
                    CpDef {
                        idx: i as u16,
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
                    }
                })
                .collect();
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
                library: Id::from_uuid(crate::QET_IMPORT_LIBRARY),
                item: fndef_item,
                version_req: "^1".to_owned(),
                extra: BTreeMap::new(),
            };
            (r, Some(lib_copy(fndef_item, def)))
        }
    };

    // Symbol.
    let cps = parsed
        .terminals
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut extra = BTreeMap::new();
            if let Some(u) = &t.uuid {
                extra.insert("qet.uuid".to_owned(), Value::String(u.clone()));
            }
            SymbolCp {
                idx: i as u16,
                fn_cp: i as u16,
                pos: t.pos,
                dir: t.dir.clone(),
                discipline: Discipline::Electrical,
                extra,
            }
        })
        .collect();
    let mut extra = BTreeMap::new();
    extra.insert("qet.link_type".to_owned(), Value::String(link_type.clone()));
    extra.insert("source_ref".to_owned(), source_ref(&key));
    let symbol = SymbolDef {
        meta,
        name: MlText::from_langs(names),
        keywords: Vec::new(),
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
        symbol: lib_copy(symbol_item, symbol),
        fndef: fndef_copy,
        entries,
        version,
        link_type,
        key,
    })
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

    /// `Certain` when the head contains `<definition` and `type="element"`; `Maybe(30)` for a name ending
    /// in `.elmt`; otherwise `No`.
    fn detect(&self, head: &[u8], name: &str) -> Detection {
        let has = |needle: &[u8]| head.windows(needle.len()).any(|w| w == needle);
        if has(b"<definition") && (has(b"type=\"element\"") || has(b"type='element'")) {
            Detection::Certain
        } else if name.to_ascii_lowercase().ends_with(".elmt") {
            Detection::Maybe(30)
        } else {
            Detection::No
        }
    }

    fn import(&self, input: &ImportInput, ctx: &mut ImportContext) -> Result<ImportBatch, ImportError> {
        let size = input.bytes.len() as u64;
        if size > ctx.limits.max_input_bytes {
            return Err(ImportError::LimitExceeded {
                what: "input size".into(),
                limit: ctx.limits.max_input_bytes,
            });
        }
        let bytes = input.bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&input.bytes);
        let xml = std::str::from_utf8(bytes).map_err(|e| ImportError::Unsupported {
            message: format!("not UTF-8 at byte {}", e.valid_up_to()),
        })?;

        let mut records = Vec::new();
        let lineage = match ctx.lineage {
            Some(l) => l,
            None => {
                let namespace = v5::lineage_namespace(
                    crate::IMPORTER_ID,
                    &v5::LineageAnchor::FirstImportSha256(input.sha256),
                )
                .unwrap_or_else(|_| unreachable!("\"qet\" is a valid importer name"));
                let id: Id<ImportLineage> = v5::imported_id(namespace, LINEAGE_KEY);
                records.push(Record::ImportLineage(ImportLineage {
                    id,
                    v: 1,
                    importer: Ident::from_input(crate::IMPORTER_ID)
                        .unwrap_or_else(|_| unreachable!("\"qet\" is an identifier")),
                    source_name: input.name.clone(),
                    anchor: LineageAnchor::FirstImportSha256(Sha256(input.sha256)),
                    namespace: Id::from_uuid(namespace),
                    last_source_sha256: Some(Sha256(input.sha256)),
                    last_run: None,
                    extra: BTreeMap::new(),
                }));
                LineageInfo { id, namespace }
            }
        };

        let src = ElementSource {
            file: &input.name,
            path: None,
        };
        let el = read_element(xml, &src, &lineage, &ctx.limits)?;

        let mut report = ImportReport::new(
            crate::IMPORTER_ID,
            crate::IMPORTER_VERSION,
            SourceInfo {
                name: input.name.clone(),
                sha256: input.sha256,
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
