// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The four importer targets (WP-S3.09 D15, IC-17): every importer that exists, called through the front door
//! (`ew_api::import::front_door`) as a host calls it.
//!
//! - [`qet_project`]: the bytes as one `.qet`, imported into an empty project by `import.apply` (`import_qet`);
//! - [`qet_element`]: the bytes as one `.elmt` under `Limits::element_definition()`;
//! - [`golden_tables`]: up to eight named tables of the golden dialect, then the bootstrap's own path
//!   (`session_from_batch`);
//! - [`parts_table`]: the bytes as a CSV or XLSX supplier table with a fixed column map.
//!
//! Properties, whatever the bytes: no importer panics (the front door's catch would hide one, so a
//! [`Refusal::Panicked`] fails the target); every emitted record and library document reads again through
//! `ew-schema`'s strict reader from its own line with no malformed variant and validates; a batch keeps the importer
//! contract (`check_contract`); where a model is made (`qet_project`, `golden_tables`), it is made or refused with a
//! report, and a made model passes the structural check.

use std::sync::OnceLock;

use ew_api::SessionOptions;
use ew_api::import::{
    ImportBatch, ImportMode, Imported, Refusal, StarterLibrary, check_contract, front_door, import_qet,
    import_tables, session_from_batch,
};
use ew_import_qet::ew_interop_api::{ImportContext, ImportInput, Importer, Limits};
use ew_schema::{LibDocument, Record};

/// What one input did.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Outcome {
    /// The importer accepted the input (a batch or a parts import).
    pub imported: bool,
    /// Records and library documents it emitted.
    pub emitted: usize,
    /// A model was made from it (`qet_project`, `golden_tables`).
    pub committed: bool,
}

/// The starter content an import resolves its built-in keys in, read once.
fn library() -> &'static StarterLibrary {
    static LIB: OnceLock<StarterLibrary> = OnceLock::new();
    LIB.get_or_init(|| {
        StarterLibrary::from_dir(&crate::golden::repo().join("content")).unwrap_or_else(|_| StarterLibrary::empty())
    })
}

/// Fails on an importer panic that the front door caught.
fn no_panic(r: &Refusal) {
    assert!(
        !matches!(r, Refusal::Panicked { .. }),
        "the importer panicked: {r}"
    );
}

/// A record's own line reads again strictly, with no malformed variant, and validates.
fn rereads(record: &Record) {
    let line = serde_json::to_value(record).expect("an emitted record has a JSON form");
    let (again, malformed) = Record::load(line).expect("an emitted record reads again");
    assert!(malformed.is_empty(), "{malformed:?}");
    again.validate().expect("an emitted record validates");
}

/// A library document's JSON reads again strictly, with no malformed variant, and validates.
fn doc_rereads(doc: &LibDocument) {
    let value = serde_json::to_value(doc).expect("an emitted document has a JSON form");
    let (again, malformed) = LibDocument::load(value).expect("an emitted document reads again");
    assert!(malformed.is_empty(), "{malformed:?}");
    again.validate().expect("an emitted document validates");
}

/// The properties of a batch: its records and documents read again, and it keeps the contract.
fn batch(b: &ImportBatch) -> usize {
    b.records.iter().for_each(rereads);
    b.lib.iter().for_each(doc_rereads);
    check_contract(b, None).expect("the front door returns batches that keep the contract");
    b.records.len() + b.lib.len()
}

/// The model of an import passes the structural check when it was made.
fn committed(session: &Result<ew_api::Session, String>) -> bool {
    match session {
        Ok(s) => {
            s.world()
                .model
                .integrity_check()
                .expect("a committed import passes the structural check");
            true
        }
        Err(_) => false,
    }
}

/// `qet_project`: the bytes as `fuzz.qet`.
pub fn qet_project(data: &[u8]) -> Outcome {
    match import_qet("fuzz.qet", data.to_vec(), library()) {
        Ok(Imported { batch: b, session, .. }) => Outcome {
            imported: true,
            emitted: batch(&b),
            committed: committed(&session),
        },
        Err(r) => {
            no_panic(&r);
            Outcome::default()
        }
    }
}

/// `qet_element`: the bytes as `fuzz.elmt` under `Limits::element_definition()`.
pub fn qet_element(data: &[u8]) -> Outcome {
    let Ok(input) = ImportInput::new("fuzz.elmt", data.to_vec()) else {
        return Outcome::default();
    };
    let mut ctx = ImportContext::new(ew_ids::Id::from_uuid(ew_ids::Uuid::nil()));
    ctx.limits = Limits::element_definition();
    let limits = ctx.limits;
    let importer = ew_import_qet::QetElementImporter;
    let sizes = [input.bytes().len() as u64];
    match front_door("fuzz.elmt", &sizes, &limits, None, || importer.import(&input, &mut ctx)) {
        Ok(b) => Outcome {
            imported: true,
            emitted: batch(&b),
            committed: false,
        },
        Err(r) => {
            no_panic(&r);
            Outcome::default()
        }
    }
}

/// The most tables of one [`golden_tables`] input.
pub const MAX_TABLES: usize = 8;

/// The tables of a [`golden_tables`] input: the first byte's low three bits give the count less one, its high bit the
/// lenient mode; each table is a selector byte (an index into the dialect's table names, modulo their number), a 2-byte
/// little-endian length and its bytes (a length past the end takes the rest; the last table takes the rest).
pub fn decode_tables(data: &[u8]) -> (ImportMode, Vec<(String, Vec<u8>)>) {
    let names: Vec<String> = ew_import_tables::table_names().collect();
    let Some((&head, mut rest)) = data.split_first() else {
        return (ImportMode::Strict, Vec::new());
    };
    let mode = if head & 0x80 != 0 {
        ImportMode::Lenient
    } else {
        ImportMode::Strict
    };
    let count = usize::from(head & 0x07) + 1;
    let mut tables = Vec::new();
    for i in 0..count {
        let Some((&selector, after)) = rest.split_first() else {
            break;
        };
        let name = names[usize::from(selector) % names.len()].clone();
        let (bytes, next) = if i + 1 == count {
            (after, &after[after.len()..])
        } else {
            let (len, body) = after.split_at(after.len().min(2));
            let mut le = [0u8; 2];
            le[..len.len()].copy_from_slice(len);
            body.split_at(usize::from(u16::from_le_bytes(le)).min(body.len()))
        };
        if !tables.iter().any(|(n, _)| *n == name) {
            tables.push((name, bytes.to_vec()));
        }
        rest = next;
    }
    (mode, tables)
}

/// Encodes tables as [`decode_tables`] reads them (`names` are indexes into the dialect's table names).
pub fn encode_tables(lenient: bool, tables: &[(u8, &[u8])]) -> Vec<u8> {
    let count = tables.len().clamp(1, MAX_TABLES);
    let mut out = vec![u8::try_from(count - 1).unwrap_or(7) | if lenient { 0x80 } else { 0 }];
    for (i, (selector, bytes)) in tables.iter().take(count).enumerate() {
        out.push(*selector);
        if i + 1 < count {
            out.extend_from_slice(&u16::try_from(bytes.len()).unwrap_or(u16::MAX).to_le_bytes());
        }
        out.extend_from_slice(bytes);
    }
    out
}

/// The index of the dialect's table `name` (`parts.csv`), for [`encode_tables`].
pub fn table_index(name: &str) -> u8 {
    ew_import_tables::table_names()
        .position(|n| n == name)
        .and_then(|i| u8::try_from(i).ok())
        .unwrap_or_else(|| panic!("no table {name}"))
}

/// `golden_tables`: the tables of [`decode_tables`] through `import_tables`, then `session_from_batch`.
pub fn golden_tables(data: &[u8]) -> Outcome {
    let (mode, tables) = decode_tables(data);
    let mut inputs = Vec::new();
    for (name, bytes) in tables {
        match ImportInput::new(&name, bytes) {
            Ok(i) => inputs.push(i),
            Err(_) => return Outcome::default(),
        }
    }
    match import_tables(&inputs, library(), mode) {
        Ok(b) => {
            let emitted = batch(&b);
            Outcome {
                imported: true,
                emitted,
                committed: committed(&session_from_batch(&b, SessionOptions::default())),
            }
        }
        Err(ew_api::import::TablesError::Refused(r)) => {
            no_panic(&r);
            Outcome::default()
        }
        Err(_) => Outcome::default(),
    }
}

/// The fixed column map of [`parts_table`]: the headers `Manufacturer`, `Part number`, `Type`, `Order number`,
/// `GTIN`, `Description`, `Kind` and `Unit`.
pub fn parts_map() -> ew_api::catalog::ColumnMap {
    ew_api::catalog::ColumnMap::new(
        None,
        [
            ("manufacturer", "Manufacturer"),
            ("part_number", "Part number"),
            ("type_designation", "Type"),
            ("order_number", "Order number"),
            ("gtin", "GTIN"),
            ("description", "Description"),
            ("kind", "Kind"),
            ("unit", "Unit"),
        ],
    )
    .expect("the fixed map is valid")
}

/// `parts_table`: the bytes as `fuzz.csv` (a workbook when they are one) through `parse_parts` into a new library.
pub fn parts_table(data: &[u8]) -> Outcome {
    use ew_api::catalog::{CatalogTarget, parse_parts};
    let Ok(input) = ImportInput::new("fuzz.csv", data.to_vec()) else {
        return Outcome::default();
    };
    let map = parts_map();
    let target = CatalogTarget::new(ew_ids::Uuid::from_u128(0x0190_d8a4_6d3e_7c41_8f00_0000_0000_0001));
    let sizes = [input.bytes().len() as u64];
    match front_door("fuzz.csv", &sizes, &target.limits, None, || {
        parse_parts(&input, &map, &[], &target)
    }) {
        Ok(p) => {
            p.parts.iter().chain(&p.manufacturers).for_each(doc_rereads);
            Outcome {
                imported: true,
                emitted: p.parts.len() + p.manufacturers.len(),
                committed: false,
            }
        }
        Err(r) => {
            no_panic(&r);
            Outcome::default()
        }
    }
}
