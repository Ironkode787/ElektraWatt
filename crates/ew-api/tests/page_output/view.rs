// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Test glue: the sample project over its records, the bundled fonts and the pages of the page order with their
//! views.
//!
//! The views are the product's: `ew_api::output` owns the glue rules (frame forms,
//! bindings, slot texts, wire markers, strip segments, report tables, embedded blocks) and builds one memoised
//! `page_display_list` family per layer. What stays here is test-only: the sample's records (with a record
//! shuffle for AC-14), and a symbol of `content/symbols` for the hostile page.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ew_api::output;
use ew_api::{Session, SessionOptions};
use ew_derive::{PageEntry, PageOrder, PageRef, Q};
use ew_dl::{Destination, FontKey};
use ew_render_cpu::FontBytes;
use ew_scene::page::{PageView, SymbolRef};
use ew_schema::{LibCopy, LibDocument};

pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn json_files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == ext))
        .collect();
    paths.sort();
    paths
}

/// The built-in content library: the QET symbols, the function definitions and the forms of `content/`.
pub fn library_docs() -> Vec<LibDocument> {
    let mut docs = Vec::new();
    for sub in ["symbols/qet10", "fndefs", "forms"] {
        for p in json_files(&repo().join("content").join(sub), "json") {
            docs.push(LibDocument::from_json(&std::fs::read_to_string(&p).unwrap()).unwrap());
        }
    }
    docs
}

/// The built-in symbol with `key` from `content/symbols/qet10`.
pub fn builtin_symbol(key: &str) -> Option<SymbolRef> {
    library_docs().into_iter().find_map(|d| match d {
        LibDocument::Symbol(LibCopy { id, item, .. })
            if item.meta.key.as_ref().is_some_and(|k| k.as_str() == key) =>
        {
            Some(SymbolRef {
                id,
                def: Arc::new(item),
            })
        }
        _ => None,
    })
}

/// The sample project over its records; with `shuffle`, the records are handed over in a permuted order (AC-14).
pub fn sample_session(shuffle: Option<u64>) -> Session {
    let (project, mut records, lib) = crate::common::sample_records();
    if let Some(seed) = shuffle {
        // A deterministic Fisher–Yates permutation (xorshift64).
        let mut s = seed | 1;
        for i in (1..records.len()).rev() {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            records.swap(i, (s % (i as u64 + 1)) as usize);
        }
    }
    Session::from_records(project, records, lib, SessionOptions::default())
        .expect("the sample passes the model's integrity check")
}

/// The bundled fonts by key, as the export job embeds them.
pub fn fonts() -> BTreeMap<FontKey, FontBytes> {
    output::font_bytes().clone()
}

/// One page of the page order with its view.
pub struct ViewedPage {
    /// The page-order entry.
    pub entry: PageEntry,
    /// The view (`ew_api::output::page_view`).
    pub view: PageView,
}

/// Every page of the page order with the product's view.
pub fn page_views(q: &Q) -> Vec<ViewedPage> {
    q.get::<PageOrder>(())
        .entries
        .iter()
        .map(|entry| ViewedPage {
            entry: entry.clone(),
            view: output::page_view(q, entry),
        })
        .collect()
}

/// The page's own destination.
pub fn destination(page: PageRef) -> Destination {
    output::page_destination(page)
}
