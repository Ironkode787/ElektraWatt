// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! DXF import (doc 14 §6, FR-IMP-032 … FR-IMP-035): ASCII and binary DXF R12 to 2018 read into a graphic page, the
//! drawing of a DXF underlay, symbols and part outlines, behind `ew-interop-api`.
//!
//! - **Reading** (own reader, [`probe`]): the group-code pairs of an ASCII or binary file (R12 one-byte codes, R13 and
//!   later two-byte codes), strings in Windows-1252 before AutoCAD 2007 and UTF-8 from then on, `\U+XXXX` escapes
//!   resolved; the header's unit, extents and `MIRRTEXT`; the LAYER, LTYPE, STYLE and BLOCK_RECORD tables; blocks,
//!   entities, layouts and image definitions.
//! - **Coverage** (doc 14 §6.1): LINE; LWPOLYLINE and POLYLINE with bulges and widths; ARC; CIRCLE; ELLIPSE; SPLINE;
//!   SOLID; TRACE; POINT; HATCH (solid fill, a pattern as its outline, EW-IMP-255); TEXT with its alignment; MTEXT
//!   with its formatting removed; INSERT nested, scaled, mirrored and as MINSERT arrays with ATTRIBs; constant ATTDEFs;
//!   DIMENSION and ACAD_TABLE through their anonymous blocks; LEADER; IMAGE as a frame with the file's name
//!   (EW-IMP-254); the model space and every paper-space layout, its viewports showing the model space clipped. 3D
//!   entities, proxies and other types are counted and reported (EW-IMP-253).
//! - **Units and frame** ([`convert`]): `$INSUNITS` or the user's [`Unit`] (EW-IMP-251 when the header names
//!   none), `round_half_even(v × unit_nm)` after a finiteness and range check, y flipped to the sheet's
//!   (`y_sheet = y_top − y`, `y_top` the top of the extents), angles as `Angle2`; curves flattened at 0.01 mm through
//!   `ew-dl`'s pure-Rust trigonometry, so the batch is the same on every platform.
//! - **Targets**: a graphic page ([`import_page`]: one page and its items, an item per shape keyed
//!   `dxf:<space>|<handle>`, an insert's shapes `…|<insert>/<handle>`, layers kept as the items' layers); the items a DXF
//!   underlay draws ([`convert`]); symbols ([`symbols`], the wizard's steps); a part's front outline ([`outline`]).
//! - **SHX** ([`SHX_SUBSTITUTES`]): an SHX font is drawn in a bundled family and reported once (EW-IMP-252).
//! - **Limits** (doc 14 §11): the input size and text length of the context's [`Limits`], at most 5 million entities,
//!   block nesting 32, 1 million shapes after expansion, 100,000 spline control points; coordinates finite and
//!   within the format's safe range after scaling.
//!
//! Pure (doc 14 §2.1): it reads only the bytes it is handed, never a file, the clock or the network. Depends on
//! `ew-interop-api` and the permissive `ew-dl`, `ew-units` and `ew-mltext` only, so it is `MIT OR Apache-2.0` like the
//! format crates it builds on.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod emit;
mod geom;
mod pairs;
mod parse;
mod symbol;
mod text;

use std::collections::{BTreeMap, BTreeSet};

pub use emit::{Converted, NO_UNIT, Probe, Space, Unit, convert, probe};
pub use ew_interop_api;
use ew_interop_api::ew_ids::{Id, v5};
use ew_interop_api::ew_schema::{
    Folio, ImportLineage, LineageAnchor, LineageCopy, Page, PageItem, PageType, Record, Section, Sha256,
};
use ew_interop_api::{
    ImportBatch, ImportContext, ImportError, ImportInput, ImportReport, LineageInfo, SourceInfo, SourceRef,
};
use ew_mltext::{Ident, MlText};
use ew_units::Grid;
pub use geom::{MAX_NESTING, MAX_PRIMITIVES, MAX_SPLINE_POINTS, TOLERANCE_NM};
pub use parse::MAX_ENTITIES;
pub use symbol::{BlockSymbol, SNAP_REPORT_NM, SymbolPin, SymbolSlot, outline, symbols};
pub use text::{DEFAULT_FAMILY, SHX_SUBSTITUTES, shx_substitute};

/// The importer ID: the `<importer>` of the lineage names (doc 14 §2.3).
pub const IMPORTER_ID: &str = "dxf";

/// The importer version, recorded in the import run; it changes with the mapping.
pub const IMPORTER_VERSION: &str = "1";

/// The source key of a lineage record.
pub const LINEAGE_KEY: &str = "dxf:lineage";

/// How a drawing is read: its unit when the header names none, the space, and hidden layers imported anyway.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DxfOptions {
    /// The unit when the header names none.
    pub unit: Option<Unit>,
    /// The model space or a layout.
    pub space: Space,
    /// Layers that are off or frozen but imported anyway, by name.
    pub show: BTreeSet<String>,
}

/// The page a drawing becomes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PageSpec {
    /// Its section.
    pub section: Option<Id<Section>>,
    /// Its folio number.
    pub number: u32,
    /// Its snap grid.
    pub grid: Grid,
    /// Its title (the file's name without the extension, by the hosts).
    pub title: String,
}

/// The lineage of an import into a project that already holds an import of the same file: a copy, its namespace from
/// the first import's (doc 14 §2.3 `Copy{of, n}`).
pub fn copy_lineage(sha256: [u8; 32], n: u32) -> (Uuid, LineageAnchor) {
    let first = first_namespace(sha256);
    let ns = v5::lineage_namespace(IMPORTER_ID, &v5::LineageAnchor::Copy { of: first, n })
        .unwrap_or_else(|_| unreachable!("\"dxf\" is a valid importer name"));
    (
        ns,
        LineageAnchor::Copy(LineageCopy {
            of: Id::from_uuid(first),
            n,
            extra: BTreeMap::new(),
        }),
    )
}

use ew_interop_api::ew_ids::Uuid;

/// The namespace of the first import of a file of SHA-256 `sha256`.
pub fn first_namespace(sha256: [u8; 32]) -> Uuid {
    v5::lineage_namespace(IMPORTER_ID, &v5::LineageAnchor::FirstImportSha256(sha256))
        .unwrap_or_else(|_| unreachable!("\"dxf\" is a valid importer name"))
}

/// A DXF drawing as a new graphic page of the project (doc 14 §6.2): the lineage (`ctx.lineage`, else a new one
/// anchored on the file's SHA-256, or as copy `copy` of it), the page and its items.
///
/// # Errors
///
/// As [`convert`]: no DXF, a limit, no unit.
pub fn import_page(
    input: &ImportInput,
    ctx: &ImportContext,
    options: &DxfOptions,
    page: &PageSpec,
    copy: Option<u32>,
) -> Result<ImportBatch, ImportError> {
    let limits = ctx.limits;
    if input.bytes().len() as u64 > limits.max_input_bytes {
        return Err(ImportError::LimitExceeded {
            what: "input size".to_owned(),
            limit: limits.max_input_bytes,
        });
    }
    let converted = convert(
        input.bytes(),
        &limits,
        options.unit,
        &options.space,
        &options.show,
        input.name(),
    )?;
    let version = converted.version.clone();
    let mut records = Vec::new();
    let lineage = match ctx.lineage {
        Some(l) => l,
        None => {
            let sha = input.sha256();
            let (namespace, anchor) = match copy {
                Some(n) => copy_lineage(sha, n),
                None => (
                    first_namespace(sha),
                    LineageAnchor::FirstImportSha256(Sha256(sha)),
                ),
            };
            let id: Id<ImportLineage> = v5::imported_id(namespace, LINEAGE_KEY);
            records.push(Record::ImportLineage(ImportLineage {
                id,
                v: 1,
                importer: Ident::from_input(IMPORTER_ID).unwrap_or_else(|_| unreachable!("an identifier")),
                source_name: input.name().to_owned(),
                anchor,
                namespace: Id::from_uuid(namespace),
                last_source_sha256: Some(Sha256(sha)),
                last_run: None,
                extra: BTreeMap::new(),
            }));
            LineageInfo { id, namespace }
        }
    };
    let page_key = format!("dxf:page|{}", options.space.key());
    let page_id: Id<Page> = v5::imported_id(lineage.namespace, &page_key);
    records.push(Record::Page(Page {
        id: page_id,
        v: 1,
        folio: Folio {
            section: page.section,
            number: page.number,
            label: None,
            extra: BTreeMap::new(),
        },
        kind: PageType::Graphic,
        structure: Default::default(),
        form: None,
        title: MlText::neutral(&page.title),
        description_override: None,
        scale: Default::default(),
        grid: page.grid,
        source_ref: Some(SourceRef {
            lineage: lineage.id,
            key: page_key,
            extra: BTreeMap::new(),
        }),
        extra: BTreeMap::new(),
    }));
    let mut items: Vec<PageItem> = converted
        .items
        .into_iter()
        .map(|(key, layer, kind)| PageItem {
            id: v5::imported_id(lineage.namespace, &key),
            v: 1,
            page: page_id,
            layer,
            kind,
            extra: BTreeMap::new(),
        })
        .collect();
    items.sort_by_key(|i| i.id);
    let n_items = items.len();
    records.extend(items.into_iter().map(Record::PageItem));
    let mut report = ImportReport::new(
        IMPORTER_ID,
        IMPORTER_VERSION,
        SourceInfo {
            name: input.name().to_owned(),
            sha256: input.sha256(),
            size: input.bytes().len() as u64,
            format: "dxf".to_owned(),
            format_version: Some(version),
            detail: None,
        },
    );
    for (kind, n) in [
        ("import_lineage", usize::from(ctx.lineage.is_none())),
        ("page", 1),
        ("page_item", n_items),
    ] {
        if n > 0 {
            report
                .stats
                .per_kind
                .insert(kind.to_owned(), u32::try_from(n).unwrap_or(u32::MAX));
        }
    }
    report.entries = converted.entries;
    Ok(ImportBatch {
        records,
        lib: Vec::new(),
        assets: Vec::new(),
        report,
    })
}
