// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew lib`: the libraries and their index.

use super::*;

// The program boundary: `EW_CONTENT_DIR` only names the directory of the starter content, as `--content` does.
#[allow(
    clippy::disallowed_methods,
    reason = "the environment is read at the program boundary"
)]
pub(super) fn content_dir() -> Option<PathBuf> {
    std::env::var_os("EW_CONTENT_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Opens the tiers of `ew lib` (exit code 2 on failure).
pub(super) fn open_libraries(company: &[PathBuf]) -> Result<ew_api::library::Registry, i32> {
    let system: Vec<PathBuf> = content_dir().map(|d| d.join("libraries")).into_iter().collect();
    let Some(tiers) = ew_api::library::LibraryTiers::for_user(system, company.to_vec()) else {
        eprintln!("error: no data and cache directories (set EW_DATA_DIR and EW_CACHE_DIR)");
        return Err(2);
    };
    ew_api::library::Registry::open(&tiers).map_err(|e| {
        eprintln!("error: the libraries cannot be opened: {e}");
        2
    })
}

pub(super) fn index_state(s: &ew_api::library::IndexState) -> String {
    match s {
        ew_api::library::IndexState::Fresh => "fresh".to_owned(),
        ew_api::library::IndexState::Rebuilt => "rebuilt".to_owned(),
        ew_api::library::IndexState::Failed(m) => format!("failed: {m}"),
    }
}

pub(super) fn run_lib(a: &LibArgs) -> i32 {
    use ew_api::library::{ItemQuery, PartQuery};
    let mut reg = match open_libraries(&a.company) {
        Ok(r) => r,
        Err(code) => return code,
    };
    let libs = reg.libraries();
    let keys: std::collections::BTreeMap<String, String> =
        libs.iter().map(|l| (l.id.to_string(), l.key.clone())).collect();
    let key_of = |id: &dyn std::fmt::Display| {
        let id = id.to_string();
        keys.get(&id).cloned().unwrap_or(id)
    };
    match &a.action {
        LibAction::List => {
            if a.json {
                let libraries = libs
                    .iter()
                    .map(|l| results::LibraryRow {
                        id: l.id.to_string(),
                        key: l.key.clone(),
                        name: l.name.clone(),
                        tier: l.tier.name().to_owned(),
                        version: l.version.clone(),
                        licence: l.licence.clone(),
                        writable: l.writable,
                        generation: l.generation,
                        index: index_state(&l.index),
                        path: l.path.as_ref().map(|p| p.display().to_string()),
                        // WP-S7.03 T2: an address profile that does not render is left out of the profiles at hand.
                        findings: l
                            .findings
                            .iter()
                            .cloned()
                            .chain(ew_api::unreadable_profiles(&reg, l.id))
                            .collect(),
                    })
                    .collect();
                print!(
                    "{}",
                    results::to_text(&results::LibListResult {
                        schema: results::LIB_LIST.to_owned(),
                        libraries,
                    })
                );
            } else {
                for l in &libs {
                    let path = l
                        .path
                        .as_ref()
                        .map_or_else(|| "(compiled in)".to_owned(), |p| p.display().to_string());
                    println!(
                        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{path}",
                        l.tier.name(),
                        l.key,
                        l.version,
                        l.name,
                        l.id,
                        if l.writable { "rw" } else { "ro" },
                        index_state(&l.index)
                    );
                    for f in l
                        .findings
                        .iter()
                        .cloned()
                        .chain(ew_api::unreadable_profiles(&reg, l.id))
                    {
                        println!("  {f}");
                    }
                }
            }
            for f in reg.findings() {
                eprintln!("warning: {f}");
            }
            0
        }
        LibAction::Search {
            text,
            kinds,
            all,
            limit,
        } => {
            let kinds: Vec<_> = kinds
                .iter()
                .filter_map(|k| ew_api::library::item_kind(k))
                .collect();
            let parts = kinds.is_empty() || kinds.iter().any(|k| k.name() == "part");
            let others: Vec<_> = kinds.into_iter().filter(|k| k.name() != "part").collect();
            let mut rows = Vec::new();
            if parts {
                let q = PartQuery {
                    text: text.clone(),
                    include_inactive: *all,
                    limit: *limit,
                    ..PartQuery::default()
                };
                let hits = match reg.try_search(&q) {
                    Ok(h) => h,
                    Err(e) => {
                        eprintln!("error: {e}");
                        return 2;
                    }
                };
                for h in hits {
                    rows.push((
                        results::LibHit::Part(results::PartHitRow {
                            kind: "part".to_owned(),
                            library: h.library.to_string(),
                            library_key: key_of(&h.library),
                            item: h.item.to_string(),
                            version: h.version.clone(),
                            tier: h.tier.name().to_owned(),
                            part_number: h.part_number.clone(),
                            manufacturer: h.manufacturer.clone(),
                            description: h.description.clone(),
                            part_kind: json_text(serde_json::to_value(&h.kind)),
                            status: serde_json::to_value(&h.status).unwrap_or_default(),
                            rank_tier: h.rank_tier,
                        }),
                        format!(
                            "part\t{}\t{}\t{}\t{}\t{}/{}@{}",
                            h.part_number,
                            h.manufacturer,
                            h.description,
                            json_text(serde_json::to_value(&h.status)),
                            key_of(&h.library),
                            h.item,
                            h.version
                        ),
                    ));
                }
            }
            if !others.is_empty() {
                let q = ItemQuery {
                    text: text.clone(),
                    kinds: others,
                    include_inactive: *all,
                    limit: *limit,
                    ..ItemQuery::default()
                };
                for h in reg.search_items(&q) {
                    rows.push((
                        results::LibHit::Item(results::ItemHitRow {
                            kind: h.kind.name().to_owned(),
                            library: h.library.to_string(),
                            library_key: key_of(&h.library),
                            item: h.item.to_string(),
                            version: h.version.clone(),
                            tier: h.tier.name().to_owned(),
                            key: h.key.clone(),
                            name: h.name.clone(),
                            status: serde_json::to_value(&h.status).unwrap_or_default(),
                            rank_tier: h.rank_tier,
                        }),
                        format!(
                            "{}\t{}\t{}\t{}\t{}/{}@{}",
                            h.kind.name(),
                            h.key,
                            h.name,
                            json_text(serde_json::to_value(&h.status)),
                            key_of(&h.library),
                            h.item,
                            h.version
                        ),
                    ));
                }
            }
            if a.json {
                print!(
                    "{}",
                    results::to_text(&results::LibSearchResult {
                        schema: results::LIB_SEARCH.to_owned(),
                        hits: rows.into_iter().map(|(j, _)| j).collect(),
                    })
                );
            } else {
                if rows.is_empty() {
                    eprintln!("nothing found");
                }
                for (_, line) in rows {
                    println!("{line}");
                }
            }
            0
        }
        LibAction::IndexRebuild => {
            if let Err(e) = reg.rebuild_index() {
                eprintln!("error: the index cannot be rebuilt: {e}");
                return 2;
            }
            match reg.index_counts() {
                Ok((parts, items)) => {
                    if a.json {
                        print!(
                            "{}",
                            results::to_text(&results::LibIndexResult {
                                schema: results::LIB_INDEX.to_owned(),
                                parts,
                                items,
                            })
                        );
                    } else {
                        println!("index rebuilt: {parts} parts, {items} other items");
                    }
                    0
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    2
                }
            }
        }
        LibAction::Create {
            dir,
            key,
            name,
            licence,
        } => match reg.create(dir, key, name, licence) {
            Ok(id) => {
                let listed = reg.libraries().iter().any(|l| l.id == id);
                if a.json {
                    print!(
                        "{}",
                        results::to_text(&results::LibCreateResult {
                            schema: results::LIB_CREATE.to_owned(),
                            key: key.clone(),
                            id: id.to_string(),
                            dir: dir.display().to_string(),
                            listed,
                        })
                    );
                } else {
                    println!("created {key} ({id}) in {}", dir.display());
                }
                if !listed {
                    eprintln!(
                        "note: {} lies in no library tier; pass it as `--company` to use it",
                        dir.display()
                    );
                }
                0
            }
            Err(e) => {
                eprintln!("error: {e}");
                2
            }
        },
        LibAction::ImportParts {
            library,
            file,
            map,
            as_of,
            bump,
            take_over,
        } => {
            let Some(target) = libs
                .iter()
                .find(|l| l.key == *library || l.id.to_string() == *library)
                .map(|l| (l.id, l.key.clone()))
            else {
                eprintln!("error: no library `{library}` (see `ew lib list`)");
                return 2;
            };
            let (map, input) = match read_map_and_table(map, file) {
                Ok(r) => r,
                Err(code) => return code,
            };
            match ew_api::catalog::import_into_library(
                &mut reg,
                target.0,
                &input,
                &map,
                Some(*as_of),
                *bump,
                *take_over,
            ) {
                Ok(done) => {
                    if a.json {
                        print!(
                            "{}",
                            results::to_text(&results::LibImportPartsResult {
                                schema: results::LIB_IMPORT_PARTS.to_owned(),
                                library: target.1.clone(),
                                source: input.name().to_string(),
                                rows: row_counts(&done.import),
                                report: report_rows(&done.import),
                                written: done.write.as_ref().map(|w| results::LibWrite {
                                    added: w.added.len(),
                                    updated: w.updated.len(),
                                    unchanged: w.unchanged.len(),
                                    generation: w.generation,
                                }),
                            })
                        );
                        if let Some(w) = done.write.as_ref().and_then(index_warning) {
                            eprintln!("{w}");
                        }
                        return 0;
                    }
                    print_catalog_rows(&done.import);
                    let written = done.write.as_ref().map_or_else(
                        || "nothing to write".to_owned(),
                        |w| {
                            format!(
                                "written: {} added, {} updated, {} unchanged (generation {})",
                                w.added.len(),
                                w.updated.len(),
                                w.unchanged.len(),
                                w.generation
                            )
                        },
                    );
                    println!(
                        "imported {} into {}: {}; {written}",
                        input.name(),
                        target.1,
                        counts_text(&done.import)
                    );
                    if let Some(w) = done.write.as_ref().and_then(index_warning) {
                        eprintln!("{w}");
                    }
                    0
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    if matches!(
                        e,
                        ew_api::catalog::CatalogError::Library(ew_api::library::LibraryError::Stale { .. })
                    ) {
                        eprintln!("note: pass `--take-over` to take the stale lock over");
                    }
                    2
                }
            }
        }
    }
}

/// The warning for a library write whose search index failed to take the committed write (FT-665): the write
/// stands, and the index is brought up to date on the next open or write.
pub(super) fn index_warning(w: &ew_api::library::WriteReport) -> Option<String> {
    w.index_error.as_deref().map(|e| {
        format!("warning: the library's search index was not updated ({e}); it is rebuilt on the next open or write")
    })
}
