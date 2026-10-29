# WP-S1.19: parts, libraries and macros (closed specification)

**Status:** specified in cycle 34 (documents only), an M2 package, in six parts: part 1 the typed part, manufacturer
and macro data within format 0.1 (1.2 pw, D5), part 2 the library tiers and the per-user search index in `ew-library`
(1.4 pw, D5), part 3 embedding by reference, `part.assign`, update from library and the manufacturer copies (1.3 pw,
D4), part 4 the CSV and XLSX parts import with column mapping and the basic-project backend (0.8 pw, D3), part 5 save
and insert macros with value sets and include flags (1.5 pw, D5), part 6 the macro and library panes (0.6 pw, D3);
part 1 first; parts 2 and 4 after it in any order; part 3 after part 2; part 5 after parts 1 and 3; part 6 last ·
**Date:** 2026-10-29 · **Work package:** [WP-S1.19](../02-work-packages.md#wp-s119-parts-libraries-and-macros) ·
**Decision log:** ID-783 · **Part of:** ElektraWatt plan, specifications

**Purpose.** This is the closed specification of the package that gives the MVP its catalogues: libraries at system,
company and user level as `.ewlib` text directories, one derived full-text index per user, embedding by reference with
the closure, assigning and replacing parts, the previewed update from a library, the parts import from CSV and XLSX
into a project or a library, window and page macros with value sets, and the basic-project backend. It also folds in
the open remainder of WP-S1.19a that these need (FT-24, FT-39, FT-168, FT-208, FT-463; D1). It is written against the
code at `357124dc` (§2.1, §2.2). Where this document and a design document differ, this document is the narrower and
wins for the lanes; the design documents stay the target of the whole package, and the documents named in §15.4 are
updated at the design pass after each part. A choice a lane meets that this document does not settle is escalated,
never guessed (`CLAUDE.md`, rule 2).

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [Crates, boundaries and the public items](#5-crates-boundaries-and-the-public-items)
6. [Part 1: typed part, manufacturer and macro data](#6-part-1-typed-part-manufacturer-and-macro-data)
7. [Part 2: library tiers and the search index](#7-part-2-library-tiers-and-the-search-index)
8. [Part 3: embedding, assignment and update from library](#8-part-3-embedding-assignment-and-update-from-library)
9. [Part 4: parts import and the basic-project backend](#9-part-4-parts-import-and-the-basic-project-backend)
10. [Part 5: macros](#10-part-5-macros)
11. [Part 6: the macro and library panes](#11-part-6-the-macro-and-library-panes)
12. [Determinism, the headless mode and the golden rule](#12-determinism-the-headless-mode-and-the-golden-rule)
13. [Files owned and what a lane may not decide](#13-files-owned-and-what-a-lane-may-not-decide)
14. [Risks](#14-risks)
15. [Size, order, routing and the documents to update](#15-size-order-routing-and-the-documents-to-update)
16. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package a pilot user keeps the company's parts and typicals in a company library on a
share or in git, finds a part by number, type or description in a few keystrokes, assigns it to devices in one undoable
step that embeds what it needs, sees which embedded copies are out of date and updates them through a preview, imports
a supplier's CSV or XLSX list with a column mapping, saves the PSU block of folio 102 as a window macro and the valve
manifold of folios 203 to 206 as a page macro with four value sets, and inserts them with new tags, rebound structure,
derived addresses and a conflict list before commit. A project still opens, renders and exports with no library
installed (NFR-REL-11). WP-S2.07 part 4 (the part selection dialog and the part editor), WP-S4.01 (part dimensions,
carrier kinds) and WP-S7.03 (the shipped starter parts and the basic project) build on the items of §5.2.

**Non-goals** (each is named with its package in §15.3):
- No accessory lists, part variants, ETIM or ECLASS classification, quality levels or consumable kinds (FR-PRT-003,
  -005, -018, -019, -021, -035; WP-S1.28, 1.0). Their members stay in `extra` (§6.1).
- No L4 commercial data, L5 3D or L6 3D connection points (1.0), no EDZ, EPLAN parts XML or BMEcat import (WP-S3.07,
  1.0 and 1.x), no distributor connectors (1.x).
- No placeholder re-apply, representation variants of macros, macro update, symbol macros, value-set CSV or rhai
  formulas in value sets (WP-S1.29, 1.0). No macro projects or macro boxes (FR-MAC-013, 1.0).
- No library linter, `ew lib render` or `ew lib pack` with the community review (FR-PRT-032, WP-S1.28, 1.0); no promote
  to company library and no editing of library items from inside a project (doc 07 §3.1, 1.0).
- No part selection dialog and no part editor (WP-S2.07 part 4); this package delivers the search and the commands
  that dialog calls.
- No pinning of palette copies and no purge of unused copies (FT-591).
- No change to any exported byte of Golden A, to AC-15's command log or to the acceptance result, except the
  re-blessed `lib/` files of part 3 (D10), whose exported outputs stay byte-equal.

## 2. Inputs and the code as it is

| Input | What the lanes take from it |
|---|---|
| [Symbols, parts and macros](../../design/07-symbols-parts-macros.md) §2.1 to §2.8, §3.1, §3.3, §3.5, §3.6, §3.8, §4.3, §4.5 to §4.7, §5, §9 | Library levels and identity, the `.ewlib` layout, the part record and data levels, the manufacturer master, macros, value sets and placeholder groups, the basic project, the embed closure, assignment, insertion, imports, licences, ranking, update classification, the insertion pipeline, the index build, the touchpoints and the golden typicals |
| [Project format](../../design/05-project-format-and-persistence.md) §2.2, §3.4, §3.6, §4, §5, §10.3, §13 | Kinds and ranks, `lib/` file naming and copy metadata, loader limits, the manifest, the save protocol, the additive rules, tiers and embedding |
| [Architecture](../../design/04-architecture.md) §3 (rule 2: only `ew-sqlview` and `ew-library` link rusqlite), §5.5 (fragments), §7.7 | Crate roles, the fragment format, catalogues |
| [Domain model](../../design/03-domain-model.md) §7 | `PartCopy`, `MacroDef`, `MacroInstance`, placeholder groups inside the macro |
| [Panel layout](../../design/12-panel-layout.md) §2.5 | The L3 fields and carrier sections WP-S4.01 reads |
| [Golden project](../../design/20-golden-project.md) §6.2, §9, §14, §20.2 | The PSU block, the valve typical and folios 203 to 206, the BOM rows, Golden C's step 5 |
| ADR-0004 amendment 2, ADR-0008, ADR-0010, ADR-0011 amendment 4, ADR-0021, ADR-0023, ADR-0024 | Copy identity, one command per gesture, the storage design reused for libraries, typing within format 0.1, importers' records, supply chain, the licence boundary |
| Requirements | The 27 rows of the package (§3) and the WP-S1.19a rows FR-PRT-002 and FR-PRT-016 where D1 folds them in; FT-24, FT-39, FT-166, FT-167, FT-168, FT-208, FT-262, FT-265, FT-274, FT-305, FT-323, FT-463, FT-487 |

### 2.1 What exists at `357124dc` (the base contract)

The lanes rely on exactly these items. If one differs, the lane stops and reports it.

| Item | Where | Used here for |
|---|---|---|
| `LibCopy<T>`, `ItemMeta` (`status`, `confidence`, `foreign_terms`), `Part` (L0, part of L1, `sections.cable_cores`, `sections.plc`), `PartKind` (15 kinds incl. `end_bracket`, `enclosure`, `duct`, `rail`, `configured`), `CpData` (idx, designation, role, level, side, clamp), `TemplateFn`, `OrderUnit`, `LibRef` | `crates/ew-schema/src/library.rs`, `value.rs` | The typed base part 1 extends |
| `LibDocument::{Part, Symbol, Fndef, Form, Unknown}`; the reserved kinds `macro`, `manufacturer` (library documents), `macro_instance` (rank 46, page file), `placeholder_group` (rank 47) | `crates/ew-schema/src/record.rs`, `kind.rs` | Part 1 implements `macro`, `manufacturer` and `macro_instance` (D2, D15) |
| `library.embed` (raw documents, copy identity, hash rule, project-local items refused, caller owns the closure) and `library_copy_hash` in `ew-commands` and `ew-format` with an agreement test | `crates/ew-commands/src/commands/library.rs`; `crates/ew-format/src/read.rs` | Embedding; part 3 adds the reference form (FT-274) |
| `parts.import` (project-local copies from the golden dialect, keys skipped, Bulk) and `parse_parts_tables` | `crates/ew-commands/src/commands/parts.rs`; `crates/ew-import-tables/src/parts.rs` | Part 4 widens the input, not the command |
| The compiled-in starter library (`ew_library::starter`, library ID `7202fd8b-1024-4bf1-9e99-670772eaa589`, items at 0.1.0 `draft`) and `BUILTIN_LIBRARY` in `ew-import-qet` | `crates/ew-library/src/starter.rs`; `crates/ew-import-qet/src/lib.rs` | The system library `ew-starter` (D4, FT-24) |
| `edit.paste` and `Fragment` (`#ew-fragment v1`, eight kinds, `FragmentNode` structure paths, 10,000 records) and `ew_api::parse_fragment` | `crates/ew-commands/src/commands/paste.rs` | The macro fragment (D12) |
| `Session::new_project` (built-in or directory basic project, `v5(new project, source)` IDs, library copies kept) | `crates/ew-api/src/new_project.rs` | The basic-project backend (§9.3) |
| The Parts row's chooser (`resolve_part`, `parse_part_uses`, `part_choices`) and WP-S2.07 D7's kind filter | `crates/ew-api/src/output/fields.rs` | Unchanged; part 3's search sits beside it |
| `manufacturer_name`, `main_part`, `tech_data_text` (first part use; manufacturer from `LibRef.extra["key"]`) | `crates/ew-derive/src/part_text.rs` | FT-39 and FT-208 (part 3) |
| The bulk card (`CardOrigin::Bulk`, Preview tab), the navigator framework and table mode | WP-S2.09 part 3, WP-S2.06 | Part 6's panes and every previewed command here |

### 2.2 Gaps found (why the parts are shaped as they are)

| Gap at `357124dc` | Consequence |
|---|---|
| `ew-library` holds only the compiled-in starter; nothing reads or writes a `.ewlib` directory, and no index exists | Part 2 builds the tiers, the reader, the writer and the index (D3 to D6) |
| No typed manufacturer copy: parts carry the manufacturer name in `LibRef.extra["key"]` (FT-39); Golden A has no `lib/manufacturers/` | Part 1 types the kind; part 3 migrates the importers, the golden emitter and Golden A (D10) |
| The L3 mechanical data, the carrier sections, contact data, alternatives, configured products and macro links are untyped (`extra`) | Part 1 types the MVP members (D2); WP-S4.01 and WP-S1.16 read them |
| `library.embed` takes raw documents and refuses an item held at any version; no command assigns a part with its closure; no update | Part 3: the reference form (FT-274), `part.assign`, `library.update` (D7 to D9) |
| `parts.import` reads only the golden tables' exact headers (FT-463) and only CSV | Part 4: column mapping and XLSX in `ew-import-catalog` (D11) |
| No macro kind, no insertion, no `MacroInstance` | Part 1 types them; part 5 builds save and insert (D12, D13) |
| `Project.basic_project` has no member (`new_project.rs` says so); saving a project as a basic project does not exist | Part 1 adds the member; part 4 records it and adds the save (D14) |

## 3. Requirements traced

| Requirement | What this package delivers | Part | Acceptance |
|---|---|---|---|
| FR-PRT-004 | `Part.alternatives` typed (`{part, kind}`); the import splits "A / B" into flagged candidates | 1, 4 | L1-2, IM-3 |
| FR-PRT-006 | `Part.configured` typed (`{order_code, components}`) beside `PartKind::Configured` | 1 | L1-2 |
| FR-PRT-007 | `function_template` as built; search reads it for the compatibility filter | 2 | IX-5 |
| FR-PRT-008 | `CpData` gains `clamp_type`, `csa_min`, `csa_max`, `max_conductors`, `twin_ferrule` | 1 | L1-2 |
| FR-PRT-010 | Typed sections: `plc`, `cable_cores` (as built), `enclosure`, `duct`, `rail`, `mounting_plate`; others stay in `sections.extra` | 1 | L1-2 |
| FR-PRT-011 | `Part.mech` typed (W×H×D, mount kind, rail offset, clearance, 2D front outline, mass) | 1 | L1-2 |
| FR-PRT-012 | Tiers, `.ewlib` reader and writer, per-user FTS5 index, rebuild, read-only shipped tiers, no SQLite on a share | 2 | IX-1 to IX-4, IX-8 |
| FR-PRT-014 | `library.update` previewed, field diff and affected devices | 3 | EM-5 to EM-7 |
| FR-PRT-023 | `part.assign` in mode `replace` over every device that uses a copy | 3 | EM-4 |
| FR-PRT-024 | Carrier kinds with kind-specific sections (duct inner size, rail profile, enclosure and plate dimensions) | 1 | L1-2 |
| FR-PRT-027 | `Part.macros` typed (`{rep_type, macro}`); WP-S4.01 consumes it | 1 | L1-2 |
| FR-PRT-028 | Libraries are canonical JSONL shards with a manifest written last | 2 | IX-2 |
| FR-PRT-031 | `ItemMeta.confidence` read through `part_confidence(copy, section)`; findings marked by the rules that read a non-verified section | 3 | EM-9 |
| FR-PRT-034 | Status filter in search (deprecated and withdrawn hidden by default); the library status of a copy offers the successor | 2, 3 | IX-6, EM-8 |
| FR-SYM-016 | `library.update` covers symbols, FnDefs and forms with the same classification | 3 | EM-6 |
| FR-MAC-001 | Save a selection as a window macro into a writable library | 5 | MC-1 |
| FR-MAC-002 | Save pages as a page macro; insertion creates pages with folio proposals | 5 | MC-2 |
| FR-MAC-003 | Insertion with new identities, rebound structure and derived tags | 5 | MC-3 |
| FR-MAC-004 | Variables bound to field paths, typed value sets | 5 | MC-4 |
| FR-MAC-005 | One undoable command after a previewed ghost with the conflict list | 5, 6 | MC-5, UI-2 |
| FR-MAC-006 | Macro preview: thumbnail and device list | 6 | UI-1 |
| FR-MAC-007 | Macros in versioned libraries with name, description, version, licence, thumbnail; embedded on use | 1, 5 | MC-1, MC-6 |
| FR-MAC-011 | Save as basic project, omitting pages, devices or both | 4 | BP-2 |
| FR-MAC-018 | A boolean variable includes or omits a placeholder group before the remap | 5 | MC-4 |
| FR-PRJ-009 | `Project.basic_project` recorded; library copies kept (as built) | 1, 4 | BP-1 |
| FR-PRJ-010 | The wizard backend (as built, FT-308) checked against folio 1 within ±0.2 mm | 4 | BP-3 |
| NFR-PERF-21 | The index and search built to the target; the generator and the `#[ignore]`d test written; **not measured (owner's rule)** | 2 | IX-9 |
| AC-32 (MVP part) | The 28 valve children of folios 203 to 206 reproduced by inserting the typical; the overviews 201 and 202 compared with EW-CORE-046 (WP-S1.16 part 3) | 5 | MC-4 |
| FR-PRT-002 (WP-S1.19a) | Manufacturer master copies with aliases and brands (D10) | 1, 3 | EM-3 |
| FR-PRT-016 (WP-S1.19a) | Column mapping for the CSV import (FT-463) | 4 | IM-1 |

## 4. Decisions

Each is a lead ruling that the owner may reverse. The design pass records them under ID-783 (and the as-built entries
after each part).

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **WP-S1.19a's open remainder is built here.** FT-24 (the system library constant), FT-39 (manufacturer copies), FT-168 (a part assignment command), FT-208 (the main part) and FT-463 (column mapping) are scope items of parts 1, 3 and 4, sized at **1.0 pw of WP-S1.19a's 2.6 pw PERT**; the specification therefore fills **6.8 pw** (5.8 + 1.0). WP-S1.19a closes with this package's part 4 | The catalogues need them, and two packages editing `part_text.rs` and `parts.import` in parallel would collide; the trade-off is a larger package |
| D2 | **Typing within format 0.1, once, in doc 07 §2.5's member order.** Part 1 types the MVP members of doc 07 §2.5 as optional members (`contacts`, `mech`, `alternatives`, `configured`, `macros`, the `CpData` clamp members and the carrier sections) and the reserved kinds `manufacturer`, `macro` and `macro_instance`, additive within the unreleased 0.1 under ADR-0011 amendment 4 (decision log ID-334), `schemas/0.1` re-blessed. Typed members are declared in doc 07 §2.5's order now, including the 1.0 members' places, which stay in `extra` until typed | A copy's `sha256` is taken over the item's compact JSON in declaration order, so every later re-ordering re-hashes existing copies; fixing the order before the first release costs nothing now. No Golden A copy uses a newly typed member, so no golden hash changes in part 1 |
| D3 | **`.ewlib` input and output live in `ew-library`** on `ew-format`'s pure line, manifest and hash functions, with their own lock file and manifest-last writer that follow doc 05 §5's protocol; no new crate, no new dependency edge (`ew-library` may use `ew-schema`, `ew-format` and rusqlite only) | The dependency rules forbid `ew-library` → `ew-store`; the save protocol is small enough to mirror, and one crate keeps the library code in one place. The trade-off is a second implementation of the protocol, kept honest by the same crash tests (§7.4) |
| D4 | **Tiers and where they live.** System: the compiled-in starter as the library `ew-starter` (its ID becomes `ew_interop_api::SYSTEM_LIBRARY`, FT-24) plus every `content/libraries/*.ewlib` of the installation; community: `<data dir>/packs/*.ewlib`, read-only; company: the directories listed in the settings key `libraries.company` (paths, in order); user: `<data dir>/libraries/*.ewlib`, writable. The data and cache directories follow `default_config_dir`'s pattern with `EW_DATA_DIR` and `EW_CACHE_DIR` overrides. System and community writes are refused at the registry (`LibraryError::ReadOnly`) | The four tiers of doc 07 §2.1 with one rule for writability; tests run with temporary directories through the overrides |
| D5 | **One index file per user, never authoritative.** `<cache dir>/library-index-v1.sqlite`, rusqlite bundled with FTS5 and the trigram tokenizer, the tables of doc 07 §4.3 without `part_prop` and `part_cp` (1.0) plus `item` and `item_text` for symbols, FnDefs, macros and manufacturers, a `library` table of `(library_id, generation, path)` and a `file` table of per-file hashes. Rebuilt per library when its manifest generation or a file hash changes (doc 07 §4.7), from scratch when the file is missing, unreadable or of another schema version. Opened only in the cache directory; never on a share | FR-PRT-012 as designed; a version in the file name lets a newer build start fresh instead of migrating a cache |
| D6 | **Search order.** The tiers of doc 07 §4.3 (exact `pn_norm`, prefix, trigram, bm25 with the column weights given there), then inside a tier: parts held by the project, then the caller's boost list (favourites and recently used, which the view state keeps, WP-S2.07 part 4), then natural order of part number, manufacturer key, library ID, item ID. Filters: kinds, category prefix, FnDef compatibility (same category's first two levels and contact kind, `part_fn`), library level, status (deprecated and withdrawn hidden unless asked) | Deterministic and testable without a clock; favourites stay a UI matter |
| D7 | **Commands take documents, never libraries.** `part.assign`, `library.update` and `macro.insert` carry the library documents they write, as `library.embed` does; `ew-api` resolves a `LibRef` with its closure from the tiers (the reference form of FT-274: a part's template FnDefs and manufacturer, a symbol's FnDef, a macro's symbols, FnDefs, parts and forms) and composes the command. A command log replays with `ew apply` on a machine with no library | ADR-0008's replay rule; the trade-off is larger command-log lines |
| D8 | **`part.assign` is the one assignment command** (FT-168). It embeds the missing copies of the closure (a held copy of the same version and hash is reused; another version is refused: "update from library first"), then sets the part uses of the devices in one of three modes: `append` (the default), `replace_all`, `replace {from}` (FR-PRT-023). It does not match template functions onto existing functions (doc 07 §3.3 steps 2 and 3 stay with part-first placement as built). **The main part (FT-208)** is the first part use whose copy's template has a `main` function with the device's main function's FnDef, else the first part use | One command per gesture and one undo step for the dialog of WP-S2.07 part 4; the template matching is a placement concern already served by part-first placement |
| D9 | **`library.update` is Bulk and always previewed.** For each copy: same hash → nothing; else the record diff is classified patch, minor or major by doc 07 §4.5 regardless of the declared version; a major change needs a CP map (by index if the designation is unchanged, else by designation), and a connected CP without a mapping refuses the command with the list; the copy file is rewritten in place (same copy ID). The card shows the class, the field diff, the affected devices and representations and the BOM rows that change. FR-SYM-016 uses the same command | One mechanism for parts, symbols, FnDefs and forms; the "keep both" choice of a cross-project paste (FT-396) stays with WP-S2.07 part 4 |
| D10 | **Manufacturer copies (FT-39).** `Manufacturer {key, name, aliases, brands}` is a library document (`lib/manufacturers/`); a part's `manufacturer` `LibRef` resolves to the embedded copy; `manufacturer_name` reads the copy's `name` and falls back to `extra["key"]` for files written before part 3. The importers and the golden emitter write manufacturer copies and stop writing `key`; **Golden A is re-blessed**: manufacturer copies added, the 29 part copies re-hashed, every exported output byte-equal (the test that proves it) | The design's master list (FR-PRT-002, D-18) replaces a stopgap that every reader had to know; the fallback keeps development files readable |
| D11 | **One general parts import.** `ew-import-catalog` reads CSV and XLSX (calamine, read-only, no formulas evaluated) through a column map (`member ← header`, FT-463), saved per source in `<config dir>/imports/<source key>.toml`; it dedupes on (manufacturer key, `pn_norm`), resolves manufacturer aliases through the master list it is given, and returns part documents with a report of new, updated and rejected rows (FR-PRT-016). Two targets: the project through `parts.import` (as built, project-local copies) and a writable library through `ew lib import parts` (a library write, §7.3). `ew-interop-api` re-exports `ew_schema`, so the importer names the record types without a new edge. The golden dialect stays in `ew-import-tables` | One mapping step for every user file; the golden bootstrap keeps its exact, tested reader |
| D12 | **A macro variant is a project fragment.** `MacroDef.variants[i].fragment` is the paste fragment of WP-S2.05 (`#ew-fragment v1`), widened for macros by the kinds `channel` and `device_link` and by the page records of a page macro; variables bind to field paths of the field catalogue (`SETTABLE`, WP-S2.05) and are checked when the macro is saved; value sets are typed; a placeholder group's `include` is a `Bool` variable; no rhai at MVP. Saving a macro is a **library write** (a Session operation, confirmed, not undoable, not a project command) | One fragment format for clipboard and macros, already parsed, validated and fuzzed |
| D13 | **`macro.insert` is one command**, `Interactive`, with the macro document and its closure as parameters, the target page (window macro) or the page after which new pages go (page macro), position, orientation, a value set name or explicit values, and `instance` (a UUID minted by the caller and written to the command log). New IDs are `v5(instance, local ID)`. It refuses only for a missing variant for the page's representation type, a grid mismatch, a value that does not coerce, or a held macro copy of another version; tag, address, IP-name and process-tag conflicts are checks that the preview lists (FR-MAC-005), never refusals | Replays reproduce every ID; conflicts are information the user decides on, as the card shows them |
| D14 | **The basic project is recorded and saved.** `Project.basic_project = {id, generation}` (the basic project's project ID and its manifest generation when copied; a basic project has no semantic version) is written by `Session::new_project` from a directory template; `Session::save_as_basic(dir, omit_pages, omit_devices)` writes a new directory of kind `Basic` with new IDs, keeping schemes, profiles, frames, sections, structure, report blocks and library copies | FR-PRJ-009 and FR-MAC-011 with the least new surface; a palette with pins is FT-591 |
| D15 | **`placeholder_group` stays reserved.** Groups live inside the `MacroDef` (doc 03 §7); a placed macro leaves only its `MacroInstance` | Doc 03's ruling; no project record is needed for MVP |
| D16 | **Parts 2 to 6 in the detail they need now.** Part 1 is closed to the field; parts 2 to 6 are closed to their public items, commands, decisions and acceptance; each part's lane may still raise a gap, which goes to a lead ruling, not to the lane | Part 1 dispatches first; the later parts are re-read against the delivered earlier parts at each dispatch, without reopening D1 to D15 |

No decision changes an ADR. ADR-0004 amendment 2 (copy identity), ADR-0008 (one command, undo, preview), ADR-0010 (the
storage design reused for libraries), ADR-0011 amendment 4 (typing within 0.1), ADR-0021 (importers emit records),
ADR-0023 (new dependencies with `cargo vet` and `cargo deny` entries) and ADR-0024 (the permissive crates gain only data
types and one re-export) are applied as written.

## 5. Crates, boundaries and the public items

### 5.1 Roles

1. **`ew-schema`** (permissive; part 1): the typed members and kinds of §6. No GPL dependency, no logic beyond serde
   and the schemas.
2. **`ew-format`** (permissive; part 1): reading and writing `lib/manufacturers/` and `lib/macros/`, `library_copy_hash`
   for the new kinds, `macro_instance` lines in page files.
3. **`ew-interop-api`** (permissive; parts 1, 4): `SYSTEM_LIBRARY` (FT-24) and `pub use ew_schema;` (D11).
4. **`ew-library`** (GPL; part 2): tiers, `.ewlib` reader and writer, the lock, the index, search; `ew lib` support.
5. **`ew-import-catalog`** (GPL; part 4): CSV and XLSX with the column map; depends on `ew-interop-api` only.
6. **`ew-commands`** (parts 3, 5): `part.assign`, `library.update`, `macro.insert`; `library.embed` accepts the new
   kinds; schemas, DSL verbs, fuzzer arms.
7. **`ew-derive`** (part 3): `manufacturer_name` on copies, `main_part` by D8, `part_confidence`.
8. **`ew-api`** (parts 2 to 5): `LibraryTiers` in `SessionOptions`, the resolver and closure, the command builders,
   search, library status, the update classification, macro save, `save_as_basic`, the `ew lib` and `ew import parts`
   CLI.
9. **`ew-viewmodel`, `ew-ui`** (part 6): the Macros and Libraries navigators, Insert ▸ Macro…, Save as macro…,
   Update… on the bulk card.

`ew-viewmodel` and `ew-ui` name no `ew-model` or `ew-schema` type (WP-S2.06 §4.1); no new package; no new dependency
edge between workspace crates. New external crates: `calamine` (part 4) in `ew-import-catalog`; rusqlite's existing
pin in `ew-library` (part 2, owner rule already allows it).

### 5.2 The public items (what later packages build on)

```rust
// ew-interop-api (part 1)
pub const SYSTEM_LIBRARY: Uuid;                       // 7202fd8b-1024-4bf1-9e99-670772eaa589, the starter's ID

// ew-library (part 2)
pub enum Tier { System, Community, Company, User }
pub struct LibraryTiers { pub system: Vec<PathBuf>, pub community: PathBuf, pub company: Vec<PathBuf>,
                          pub user: PathBuf, pub cache: PathBuf }
pub struct Registry;                                  // the opened tiers and the index
impl Registry {
    pub fn open(tiers: &LibraryTiers) -> Result<Registry, LibraryError>;   // reindexes what changed
    pub fn libraries(&self) -> Vec<LibraryInfo>;      // id, key, name, tier, version, licence, writable, index state
    pub fn resolve(&self, r: &LibRef) -> Result<LibDocument, LibraryError>;
    pub fn closure(&self, r: &LibRef) -> Result<Vec<LibDocument>, LibraryError>;   // item first, then closure order
    pub fn search(&self, q: &PartQuery) -> Vec<PartHit>;
    pub fn search_items(&self, q: &ItemQuery) -> Vec<ItemHit>;            // symbols, FnDefs, macros, manufacturers
    pub fn write(&mut self, library: Uuid, docs: &[LibDocument]) -> Result<WriteReport, LibraryError>;
    pub fn create(&mut self, dir: &Path, key: &str, name: &str, licence: &str) -> Result<Uuid, LibraryError>;
    pub fn rebuild_index(&mut self) -> Result<(), LibraryError>;
}
pub struct PartQuery { pub text: String, pub kinds: Vec<PartKind>, pub category: Vec<String>,
                       pub compatible_with: Option<FnCompat>, pub tiers: Vec<Tier>, pub include_inactive: bool,
                       pub held: Vec<(Uuid, Uuid)> /* (library, item) of project copies */,
                       pub boost: Vec<(Uuid, Uuid)>, pub limit: u32 }
pub struct PartHit { pub library: Uuid, pub item: Uuid, pub version: String, pub tier: Tier,
                     pub part_number: String, pub manufacturer: String, pub description: String,
                     pub kind: PartKind, pub status: ItemStatus, pub compatible: Option<bool>, pub rank_tier: u8 }
pub fn pn_norm(s: &str) -> String;
pub enum LibraryError { ReadOnly, NotFound, Locked, Format(String), Index(String), Io(String) }

// ew-api (parts 2 to 5)
impl Session {
    pub fn libraries(&self) -> &Registry;                                     // from SessionOptions.libraries
    pub fn part_search(&self, q: &PartQuery) -> Vec<PartHit>;                 // `held` filled from the project
    pub fn assign_part(&self, devices: &[Uuid], part: &LibRef, mode: AssignMode, qty: Dec3)
        -> Result<AnyCommand, ApiError>;                                      // part.assign with the closure
    pub fn library_status(&self) -> Vec<CopyStatus>;                          // FT-487's data
    pub fn update_from_library(&self, copies: &[Uuid], cp_maps: &[CpMap]) -> Result<UpdatePlan, ApiError>;
    pub fn save_macro(&mut self, target: Uuid, spec: &MacroSpec) -> Result<LibRef, ApiError>;  // library write
    pub fn insert_macro(&self, m: &LibRef, at: &InsertAt, values: &MacroValues, instance: Uuid)
        -> Result<AnyCommand, ApiError>;
    pub fn save_as_basic(&self, dir: &Path, omit_pages: bool, omit_devices: bool) -> Result<(), SaveError>;
}
pub enum AssignMode { Append, ReplaceAll, Replace { from: Uuid /* copy ID */ } }
pub enum CopyState { Current, UpdateAvailable { version: String, class: ChangeClass }, Deprecated { successor:
    Option<LibRef> }, SourceMissing, LocalFork }
pub enum ChangeClass { Patch, Minor, Major }
```

The exact field names may follow the crates' conventions; the items, their meaning, the one resolver (`Registry`) and
the rule that commands take documents (D7) may not change without a lead ruling. WP-S2.07 part 4 calls
`part_search`, `assign_part` and `update_from_library`; WP-S4.01 reads `Part.mech` and the carrier sections; WP-S7.03
fills `content/libraries/ew-starter-parts.ewlib` (§7.1) and a company basic project.

## 6. Part 1: typed part, manufacturer and macro data

**Scope.** D2, D15, FT-24, the schema half of FT-39 and of D14. Formats only: no command, no derivation and no UI
changes beyond what reading the new members needs. **1.2 pw, D5.**

### 6.1 Members and kinds

All new members are optional, default-omitted and keep unknown members in their own `extra` (the crate's
`de_extra` pattern). Units: lengths in `Nm` (nanometres, `i64`), cross-sections in `Dec3` mm², masses in grams
(`u32`).

| Type | Members (in this order) | Notes |
|---|---|---|
| `Part` | `meta, kind, manufacturer, part_number, type_designation, order_number, gtin, description, category, tech_data, free_props, unit, cp_pattern, function_template, contacts, mech, sections, alternatives, configured, macros, extra` | Doc 07 §2.5's order; `classification` (after `category`), `model3d`, `cp3d`, `drilling` (after `mech`), `accessories` (before `alternatives`), `variants` (after `configured`), `commercial`, `logistics`, `docs` (after `macros`) stay in `extra` until 1.0 |
| `ContactData` (new) | `kind: ContactKind` (the FnDef enum: NO, NC, changeover), `cps: Vec<Ident>` (designations, `["13", "14"]`), `seq: Option<u8>` (sequence digit), `special: Option<Ident>` | FR-PRT-009; EW-CORE-045 (WP-S1.16 part 3) reads it |
| `MechData` (new) | `width, height, depth: Option<Nm>`, `mount_kind: Option<MountKind>`, `rail_offset: Option<Vec2Nm>`, `clearance: Option<Clearance{top, bottom, left, right, front}>`, `front_2d: Vec<Vec<Pt2>>`, `mass: Option<u32>` | Doc 12 §2.5's L3 rows; `MountKind`: `ts35x7_5, ts35x15, ts32, ts15, screw, busbar_adapter, door_cutout, on_parent, none` with `Unknown(Value)` |
| `CpData` | as built, then `clamp_type: Option<ClampType>`, `csa_min`, `csa_max: Option<Dec3>`, `max_conductors: Option<u8>`, `twin_ferrule: bool` | FR-PRT-008; `ClampType`: `screw, spring, push_in, idc, ring_stud, plug` with `Unknown` |
| `PartSections` | `cable_cores, plc` (as built), then `enclosure: Option<EnclosureSection{width, height, depth, wall, plate: Option<LibRef>}>`, `duct: Option<DuctSection{inner_width, inner_height, slot_pitch}>`, `rail: Option<RailSection{profile: MountKind, stock_length}>`, `mounting_plate: Option<PlateSection{width, height, thickness}>`, `extra` | FR-PRT-024 and doc 12 §2.5's MVP rows |
| `AlternativeRef` (new) | `part: LibRef`, `kind: equivalent \| successor \| approved` | FR-PRT-004 |
| `Configured` (new) | `order_code: Ident`, `components: Vec<Component{part: LibRef, qty: Dec3}>` | FR-PRT-006 |
| `MacroLink` (new) | `rep_type: RepType`, `macro_ref: LibRef` (written `macro`) | FR-PRT-027 |
| `Manufacturer` (kind `manufacturer`) | `meta, key: Ident, name: MlText, aliases: Vec<Ident>, brands: Vec<Ident>, extra` | `LibCopy<Manufacturer>`, `lib/manufacturers/<copy id>.json` |
| `MacroDef` (kind `macro`) | `meta, name: MlText, description: MlText, kind: window \| symbol \| page, variants: Vec<MacroVariant{rep_type, key, fragment: Value}>, variables: Vec<VarDef{name, ty, bindings: Vec<Binding{record: Uuid /* fragment-local */, field: String}>, default: Option<Value>}>, groups: Vec<PlaceholderGroup{name, members: Vec<Uuid>, include: Option<String>}>, value_sets: Vec<ValueSet{name, values: BTreeMap<String, Value>}>, thumbnail: Option<Sha256>, extra` | `fragment` is kept as `Value` here (permissive crate); its parse is `ew-commands`' (D12). `VarType`: `text, ident, number, bool, state, part, process_tag, channel_binding, address, tag_counter, device_ref` |
| `MacroInstance` (kind `macro_instance`, rank 46, the anchor page's file) | `id, macro_copy: Id<…>` (written `macro`), `variant: Ident`, `value_set: Option<Ident>`, `values: BTreeMap<String, Value>`, `members: Vec<(Uuid, AnyId)>` (local → created), `page: Id<Page>` | Links: `page` cascade, `members` unlink (a deleted member leaves the instance), `macro` forbid on the copy |
| `Project` | gains `basic_project: Option<BasicProjectRef{id: Uuid, generation: u64}>` | D14 |
| `LibDocument` | gains `Manufacturer(LibCopy<Manufacturer>)` and `Macro(LibCopy<MacroDef>)` | `library.embed` accepts both (§6.2 item 5) |

### 6.2 Build list

1. `ew-schema`: the types of §6.1 with docs, `JsonSchema`, `Unknown(Value)` on every enum, round-trip and
   unknown-member tests; `Kind::Manufacturer`, `Kind::Macro`, `Kind::MacroInstance` turn `Implemented`; the link table
   entries of `MacroInstance` (§6.1); `schemas/0.1` regenerated and committed.
2. `ew-format`: `lib/manufacturers/` and `lib/macros/` read and written like the other library kinds;
   `macro_instance` lines in page files at rank 46; `library_copy_hash` for the two new document kinds.
3. `ew-commands`: `library_copy_hash` for the two kinds (the agreement test in `ew-api` extended); `library.embed`
   accepts `manufacturer` and `macro` documents with the same checks.
4. `ew-model`: the `MacroInstance` table with the invariants of its links (cascade, unlink, forbid), as the other
   page-owned kinds; no command creates one yet.
5. `ew-interop-api`: `SYSTEM_LIBRARY`; `ew-import-qet` and `ew-import-tables` use it and drop their own constants
   (FT-24); `BUILTIN_LIBRARY` stays as a deprecated alias for one cycle only if a test outside the owned files names
   it (else removed).
6. Tests (§6.3).

### 6.3 Acceptance of part 1

| # | Check | Where | Proves |
|---|---|---|---|
| L1-1 | Golden A opens, saves and exports byte-identically (`golden/**` unchanged, the golden output, acceptance and AC-15 tests unmodified and green on the lane's targeted runs) | `ew-format`, `ew-store`, `ew-api` tests | D2: no golden change |
| L1-2 | Each new member and kind round-trips through `ew-format` with a fixture copy: 2904602 with `mech` (W×H×D, `ts35x7_5`, rail offset), PT 2.5-TWIN with clamp members, a contactor with two contacts, an enclosure and a duct section, a configured product with components, an alternative, a macro link; unknown members of each type survive; the `sha256` of each fixture is the hash of its compact JSON | `ew-schema`, `ew-format` tests | §6.1 |
| L1-3 | A file written before part 1 that carries `mech` in `extra` (byte-wise key order) opens with the members typed, is reported not canonical (info) and is rewritten canonically at the next save; its stored `sha256` is reported by EW-CORE-014 when the canonical order differs (accepted within 0.1, ID-334) | `ew-format`, `ew-store` tests | ADR-0011 amendment 4 applied |
| L1-4 | A manufacturer copy and a macro copy embed with `library.embed` (copy identity, hash, duplicate refusal as built); a `macro_instance` line round-trips in a page file; deleting the page removes it; deleting a member unlinks it; the macro copy cannot be removed while an instance names it (EW-INV-021's rule) | `ew-commands`, `ew-model` tests | §6.1 links |
| L1-5 | `SYSTEM_LIBRARY` equals the starter documents' `library_id`; the QET importer's built-in references and the golden bootstrap are unchanged | `ew-interop-api`, `ew-import-qet`, `ew-import-tables` tests | FT-24 |
| L1-6 | Targeted checks for `ew-schema`, `ew-format`, `ew-interop-api`, `ew-model`, `ew-commands`, `ew-import-qet`, `ew-import-tables`, `ew-api`: nextest, clippy `-D warnings`, `cargo fmt --all --check`, `check-deps`, `check-features`, `check-raw-read`, `check-licence-map`, `reuse lint`, `docs-gen --check` | lane | §5.1 |

## 7. Part 2: library tiers and the search index

**Scope.** D3 to D6, FR-PRT-012, FR-PRT-028, FR-PRT-034's filter, NFR-PERF-21's build. **1.4 pw, D5.**

### 7.1 The `.ewlib` directory

As doc 07 §2.2: `library.json` written last (`id`, `key`, `name`, `version`, `licence`, `format`, `min_reader`,
`generation`, `depends`, `files` with their `sha256`), shards `parts/<xx>.jsonl`, `symbols/`, `fndefs/`, `forms/`,
`macros/`, `manufacturers.jsonl`, `assets/<sha256>.<ext>`, `REUSE.toml`, `LICENSES/`. A library record line is the
item's own document with `id` = the item ID, then `version`, then the item's members (doc 07 §2.1, ID-14); it carries
no `library_id` and no copy ID. The reader adds `library_id` from the manifest
when it hands a `LibDocument` out, with the copy ID `v5(NS_LIBCOPY, library ‖ item)` and the hash computed. Lines sort
by item ID inside a shard. The system library `ew-starter-parts` (key, ID `v5(SYSTEM_LIBRARY, "ew-starter-parts")`)
is the directory WP-S7.03 fills; part 2 ships it empty with its manifest.

### 7.2 Reading, the lock and writing

- **Reading** is the project loader's discipline: the limits of doc 05 §3.6, regular files only, no symbolic links
  followed, the manifest's file hashes checked; a file that fails is skipped with a `LibraryInfo` finding, and the
  library stays usable without it.
- **The lock** is `library.lock` beside the manifest, with the project lock's content and stale-lock rule (doc 05 §5).
  A writer that cannot take it gets `LibraryError::Locked`; readers never take it.
- **Writing** (`Registry::write`): take the lock; write every changed shard under a new name
  `<shard>.<generation>.jsonl` (temporary sibling, flush, rename); write `library.json` with the incremented
  generation and the new file names last, the same way; remove the files the old manifest named and the new one does
  not; release the lock. A crash before the manifest rename leaves the old manifest naming only old, intact files, so a
  reader sees one generation complete; unreferenced files left by a crash are removed by the next write. Published versions are immutable: a write of different content under
  an existing `(item, version)` is refused (doc 05 §13.1).
- **Read-only tiers** refuse every write (D4).

### 7.3 The index and search

The tables of D5; `pn_norm` and the ranking of D6; one SQLite transaction per library on reindex; `Registry::open`
reindexes what changed and reports per library `fresh`, `rebuilt` or `failed`; a corrupt or foreign index file is
deleted and rebuilt (never trusted, `trusted_schema=OFF`, defensive mode). `ew lib list`, `ew lib search <text>
[--kind] [--json]`, `ew lib index --rebuild`, `ew lib create <dir> --key --name --licence` and (part 4) `ew lib import
parts` are the CLI.

### 7.4 Acceptance of part 2 (outline closed to the checks)

| # | Check | Proves |
|---|---|---|
| IX-1 | A fixture company library with A9P52610: `search("A9P52610")` returns it first; `search("a9p 52610")`, `search("9P526")` (trigram) and `search("miniature circuit")` (bm25 over the description) return it | FR-PRT-012, D6 |
| IX-2 | A written library is canonical JSONL with one line per changed part in a git diff of two generations; the manifest is written last; killing the writer between shard and manifest (a fault-injection hook as `ew-store`'s) leaves the old generation readable | FR-PRT-028, D3 |
| IX-3 | Deleting the index file and reopening rebuilds it with identical search results for twenty fixed queries; a truncated index file is replaced; a changed shard reindexes only its library | FR-PRT-012, D5 |
| IX-4 | A write to the system or a community library is refused `ReadOnly`; a company library directory is never opened as SQLite (the index path is always under the cache directory, asserted) | FR-PRT-012, D4 |
| IX-5 | The compatibility filter: for a 1P+N MCB function, the MCB 1P+N fixture parts rank before the others, which come back with `compatible = false` | FR-PRT-015's acceptance, D6 |
| IX-6 | A deprecated part with a successor and a withdrawn part are hidden unless `include_inactive`; the successor is reported | FR-PRT-034 |
| IX-7 | Ties: two parts with equal score order by natural part number, manufacturer, library ID, item ID on two platforms | D6 |
| IX-8 | FTS5 and the trigram tokenizer are available in the bundled SQLite (the first test; if it fails, the lane stops) | D5 |
| IX-9 | `ew-testkit` generates a 500,000-part library; the `#[ignore]`d release-profile test of NFR-PERF-21 exists and compiles; **not run (owner's rule)** | NFR-PERF-21 |

## 8. Part 3: embedding, assignment and update from library

**Scope.** D7 to D10, FT-39, FT-166, FT-167's rest, FT-168, FT-208, FT-262, FT-265, FT-274, FT-323's marker,
FT-487's data. **1.3 pw, D4.**

- **The reference form of `library.embed` (FT-274)** lives in `ew-api` (`Session::embed_command(&[LibRef])`): it
  resolves each reference with its closure (D7) and builds the raw-document command as built. Golden C's step 4 line
  (FT-166, FT-167) becomes `library.embed` by reference in `c-fix.ewcmd`'s generator; the builder's copies (FT-278)
  stay as they are.
- **`part.assign { devices: [uuid], part: uuid /* copy ID */, copies: [doc], mode: append | replace_all | replace,
  from?: uuid, qty?: dec3 = 1 }`** (D8): `Interactive` below 50 devices, `Bulk` above; rejected: an empty device
  list, a device that does not exist, `part` neither held nor in `copies`, a copy in `copies` whose item the project
  holds at another version ("update from library first"), `replace` without `from` or with a device that does not use
  `from`, `qty` ≤ 0. Outcome: the first device. DSL verb and fuzzer arm.
- **`library.update { copies: [doc], cp_maps?: [{copy: uuid, map: [[old_idx, new_idx]]}] }`** (D9): `Bulk`; rejected:
  a copy the project does not hold, a downgrade (lower version), a major change with a connected CP left unmapped
  (the list in the error). Outcome: the first copy. The plan (`UpdatePlan`) gives the class, the field diff, the
  devices and representations touched and the BOM rows that change, for the card.
- **Manufacturer copies (D10, FT-39):** the emitters (`ew-import-tables`, the QET import's parts, `parts.import`'s
  callers) write manufacturer copies; `manufacturer_name` reads the copy; Golden A re-blessed with the byte-equal
  output test; EW-CORE-065's spelling half (FT-238) gets its master list (the rule is WP-S1.16's).
- **Main part (FT-208)** by D8 in `main_part`; folio 3's manufacturer slot reads the configured part of the order code
  (FT-265) and part 195740 gets its type designation in the golden data (FT-262), both with byte-checked folio 3
  output (a golden change the lead has delegated; listed in the lane's report).
- **Library status (FT-487's data):** `Session::library_status` per held copy: `Current`, `UpdateAvailable` with the
  class, `Deprecated` with the successor, `SourceMissing` (no installed library holds the item), `LocalFork`
  (`meta.source` is a fork).
- **Confidence (FR-PRT-031, FT-323's marker):** `ew-derive` `part_confidence(copy, section) -> Confidence`; a rule
  that reads a section with confidence other than `verified` sets `Finding.unverified_data`; the PLC and bus rules
  set it when WP-S1.21 builds them; the release report's list waits for WP-S1.18.

**Acceptance (outline closed to the checks):** EM-1 `embed_command` by reference embeds 2904602 with its manufacturer
and template FnDef in one command, replayable with no library; EM-2 every rejection of `part.assign` with its pointer;
EM-3 assigning A9P52610 to F55 from a fixture company library embeds the part and Schneider Electric's manufacturer
copy, sets the part use and prints "Schneider Electric", "A9P52610" and "C10" in the slots; EM-4 replacing A9P52610 by
another MCB across devices lists -101F1 and commits once (FR-PRT-023); EM-5 updating 2904602 (a patch: a description
change) shows the field diff and -102G1 before commit (FR-PRT-014); EM-6 a symbol update with a moved anchor is minor
and lists the changed cross-reference texts; a removed CP with a connection refuses with the list (FR-SYM-016, D9);
EM-7 a declared patch that removes a CP is classified major (doc 07 §4.5); EM-8 deprecating A9P52610 with a successor:
Golden A still opens unchanged, `library_status` offers the successor for -101F1 (FR-PRT-034); EM-9 a fixture rule
reading a `test` section marks its finding; EM-10 Golden A re-blessed (D10): every exported output and the acceptance
result unchanged, the `lib/` diff only manufacturer copies added and part copies re-hashed; EM-11 targeted checks as
L1-6 plus `ew-derive`, `ew-library`, `ew-testkit`.

## 9. Part 4: parts import and the basic-project backend

**Scope.** D11, D14, FT-463, FR-MAC-011, FR-PRJ-009, FR-PRJ-010's check. **0.8 pw, D3.**

- **`ew-import-catalog`:** `parse_parts(input: &ImportInput, map: &ColumnMap, master: &[Manufacturer]) ->
  PartsImport { parts: Vec<LibDocument>, manufacturers: Vec<LibDocument>, report: ImportReport }` for CSV (UTF-8 with
  or without a byte-order mark, comma or semicolon detected from the header line) and XLSX (first sheet unless named;
  values as displayed text; formulas not evaluated, their cached values read). The map's members: `manufacturer`,
  `part_number`, `type_designation`, `order_number`, `gtin`, `description` (one column per language), `kind`, `unit`,
  `tech_data.<key>`, `free_props.<key>`; unmapped columns are reported, not guessed. Rows: new, updated (same
  manufacturer and `pn_norm`, other content), unchanged, rejected (with the reason: no part number, unknown kind, a
  value that does not parse); "A / B" part texts split into two candidates flagged in the report, never one number
  (FR-PRT-004).
- **Targets:** File ▸ Import ▸ Parts table… gains the mapping step (the card of WP-S2.09 with a column table, the map
  saved per source key, D11), then `parts.import` as built; `ew import parts --map <file>`; `ew lib import parts <lib>
  <file> --map <file> --as-of <date>` writes into a writable library (§7.2) with the import date as a parameter (doc 07
  §3.6). An XLSX file larger than the loader limits of doc 05 §3.6 is refused before parsing.
- **Basic project (D14):** `new_project` records `basic_project`; `save_as_basic` writes the directory; FR-PRJ-010's
  acceptance is checked by a test that runs the wizard backend with the golden conventions and compares folio 1 and
  the title block with Golden A within ±0.2 mm (an existing equal test is cited instead of a new one).

**Acceptance (outline closed to the checks):** IM-1 a CSV with other headers imports through a saved map; IM-2 the same
list as XLSX gives the same parts and report; IM-3 "A / B" is two flagged candidates; IM-4 dedupe and alias
resolution ("Schneider" → Schneider Electric); IM-5 rejected rows with reasons; IM-6 into a library: a second import of
the changed file updates in place (same item IDs, ADR-0004 v5 of the source key) and refuses changed content under an
unchanged version unless `--bump`; BP-1 a project from a company basic project records `basic_project`; BP-2 Golden A
saved as basic without devices keeps frame, profiles, sections and structure and has no devices (FR-MAC-011); BP-3
folio 1 within ±0.2 mm; targeted checks as L1-6 for the changed crates.

## 10. Part 5: macros

**Scope.** D12, D13, FR-MAC-001 to -005, -007, -018, FT-305's command half. **1.5 pw, D5.**

- **Save as macro** (`Session::save_macro`): the selection (window) or the pages (page) become a fragment with local
  IDs, the closure's `LibRef`s, `PageRelative` structure for references equal to the source page's (doc 07 §3.5),
  frozen tags cleared unless marked keep, open ends dropped with their CPs listed, the insertion point snapped to the
  macro grid; variables are created from field paths (`make variable` on a property row in part 6, a `MacroSpec`
  here) and checked against the field catalogue; value sets typed by their variables; the result is written to a
  writable library at version 1.0.0 (a later save of the same key bumps the minor version) with the licence the
  library declares.
- **`macro.insert { copies: [doc], macro: uuid, page: uuid, at: pt2, orient, value_set?: ident, values?: object,
  instance: uuid }`** (D13), the pipeline of doc 07 §4.6 without rhai: pick the variant by the page's representation
  type; coerce the values (literals over the value set over the defaults); drop excluded groups with their connections
  (FR-MAC-018); remap IDs `v5(instance, local)`; rebind structure (`PageRelative` → the target page, codes → nodes by
  aspect and code or alias, unknown → listed, and refused only in strict mode); transform by `at` and `orient`; clear
  tag counters of derived tags; embed the closure (D8's version rule); remap channel bindings; write the
  `MacroInstance`; run the normalisers as a paste does; page macros create their pages after `page` with proposed
  folios (FR-MAC-002). Outcome: the first created device. DSL verb and fuzzer arm (FT-305: the macro step with a
  validity oracle).
- **Conflicts** (FR-MAC-005) are computed on the preview: duplicate frozen tags, IP names that would pair, PLC addresses
  already used (manual, and derived ones that collide), duplicate process tags, occupied CPs; the bulk card lists them
  before Apply.

**Acceptance (outline closed to the checks):** MC-1 the PSU block of folio 102 (-102G1, -102F1, -102F2, wires, IPs)
saves as one window macro with an insertion point into a fixture user library (FR-MAC-001); MC-2 a page macro inserted
after 206 proposes folio 207 (FR-MAC-002); MC-3 the PSU macro inserted on folio 105 yields -105G1, -105F1, -105F2 and
the card lists the IP pairings of L-1/N-1 and the second sources of 24VDC-1 … 4 (FR-MAC-003, golden §9.3); MC-4 the
valve-manifold page macro with value sets 1 to 4, inserted into Golden A with folios 203 to 206 removed, reproduces
their devices, channel bindings, derived addresses Q0.1 … Q6.7, process tags, "Spare" texts and the overview children
(the rows of `expected/rows` for those folios equal after the ID map; value set 4 inserts one module and no field
devices, no dangling connection) (FR-MAC-004, FR-MAC-018); MC-5 inserting the 203 typical twice with value set 1 lists
the eight address conflicts Q0.1, Q0.3 … Q1.7 before commit and commits as one undo step (FR-MAC-005); MC-6 the used
macro is embedded and `ew apply` of the log on a machine with no library reproduces the records (FR-MAC-007, D7);
MC-7 twice-run determinism of save and insert; targeted checks as L1-6 for the changed crates.

**Golden facts this part relies on:** the device tree of doc 20 §6.2 and the typicals of doc 07 §9.3. If inserting the
typical cannot reproduce a row of folios 203 to 206 because the macro model lacks a record kind, the lane stops and
reports the row (§13.2 item 4).

## 11. Part 6: the macro and library panes

**Scope.** FR-MAC-006, the UI half of FR-MAC-005 and FT-487. **0.6 pw, D3**, on WP-S2.06's navigator framework and
WP-S2.09's bulk card, in the design language of doc 16a.

- **Macros navigator** (`NavKind::Macros`, Insert ▸ Macro…): the macros of every tier and of the project, grouped by
  tier, filterable by text and kind; the detail shows the thumbnail (or "no preview"), the device list and the
  variables; Enter or a drag onto the canvas arms placement (the place tool's ghost as `edit.paste` uses it) and opens
  the value-set table (a value set row, or ad-hoc values); Apply commits `macro.insert` from the card with the conflict
  list.
- **Save as macro…** (Edit menu, a selection or the marked pages): name, kind, target library (writable ones only),
  insertion point, variables from the property sheet's `Make variable` row action, value-set table; the library write
  is confirmed and not undoable (D12).
- **Libraries navigator** (`NavKind::Libraries`, View): the installed libraries with tier, version, licence, writability
  and index state; the project's copies with their state (`CopyState`, FT-487's column and badge) and where they are
  used; Update… opens the bulk card of `library.update`.

**Acceptance (outline closed to the checks):** UI-1 the macro detail shows the PSU macro's thumbnail and device list
(FR-MAC-006); UI-2 a kittest flow and its `ew.uiscript/1` twin: insert the PSU macro on folio 105 from the navigator,
the card lists the pairings, Apply is one undo step; UI-3 Save as macro… writes into a user library and refuses a
system library; UI-4 the Libraries navigator shows Golden A's copies `SourceMissing` without libraries and
`UpdateAvailable` with a fixture library, and Update… previews; UI-5 the action, describe word and string counts, the
blind references re-blessed; UI-6 accessible names of the new rows (NFR-A11Y-01).

## 12. Determinism, the headless mode and the golden rule

1. **Commands carry documents** (D7): every command of this package replays with `ew apply` on a machine with no
   library installed; the tests run the replay.
2. **No clock, no randomness in a command:** import dates and macro instance IDs are parameters; the shell mints
   them and the command log records them.
3. **Tie-breaks are named:** search (D6), closure order (item first, then FnDefs, symbols, parts, forms, manufacturers,
   each by item ID), update CP mapping (index, then designation), macro remap (`v5(instance, local)`), value precedence
   (§10). None depends on a hash map's order or on time (NFR-REL-07).
4. **The index is never read by a command or a derivation**; a project's outputs depend on its embedded copies only
   (NFR-REL-11).
5. **Golden rule:** Golden A changes only where §8 says (D10, FT-262, FT-265), each with the byte-equal output test of
   the exports it does not mean to change, and the lane lists every changed file.

## 13. Files owned and what a lane may not decide

### 13.1 Files owned

**Part 1:** `crates/ew-schema/src/{library.rs,record.rs,kind.rs,project.rs,lib.rs}`, `crates/ew-schema/tests/**`,
`schemas/0.1/**`; `crates/ew-format/src/{read.rs,write.rs,layout.rs}`, `crates/ew-format/tests/**`;
`crates/ew-interop-api/src/lib.rs`; `crates/ew-import-qet/src/{lib.rs,element.rs}`,
`crates/ew-import-tables/src/{ids.rs,library.rs}`; `crates/ew-model/src/**` (the `MacroInstance` table only);
`crates/ew-commands/src/commands/library.rs`, `crates/ew-commands/tests/**`; `crates/ew-api/tests/**` (the hash
agreement test).
**Parts 2 to 6:** named here by crate (§5.1) and fixed by each part's dispatch brief; golden files only under §12 item 5.

Shared append-only files (registry lists, `params.rs`, fuzzer arms, DSL verbs, the strings table, `schemas/0.1`) may
be touched by other lanes; the second to integrate rebases onto the first. Read-only for every lane: other crates,
`golden/**` except where §8 names it, `ci/**` except the `cargo vet` and `deny` entries of a new dependency,
`.github/**`, `docs/**` except the generated reference pages. Each lane commits on its own branch and does not merge or
push.

### 13.2 What a lane may not decide

A lane stops and reports instead of choosing when:
1. a change would need a new workspace package, a new dependency edge in `ci/dep-rules.toml`, or a GPL type in a
   permissive crate;
2. a member or kind beyond §6.1 would be typed, a required member added, or a member's position differ from D2's
   order;
3. any exported byte of Golden A changes outside §8's list, or the acceptance result changes;
4. a golden fact of this document (the typicals, the PSU block, the BOM rows) is contradicted by the data, or the
   macro model cannot carry a record the typical needs;
5. a command beyond `part.assign`, `library.update`, `macro.insert` would be added, or a command would read a library,
   the index or the file system (D7);
6. FTS5 or the trigram tokenizer is missing from the bundled SQLite (IX-8), or a new external crate fails `cargo deny`
   or needs a licence outside the allowed set (ADR-0024);
7. a library write would be reachable from a command log replay, a script or an MCP session (library writes are
   front-end operations only);
8. the work of a later part is reached (D16).

The lane decides freely: private helper names, module layout inside the owned files, the exact Rust field names of
§5.2 (their meaning fixed), test fixtures, and the index's private SQL beyond D5's tables.

## 14. Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 The copy hash follows Rust member order, so a typing step re-hashes copies | Medium · medium | D2 fixes the order now; L1-3 covers old files; after the first format release a re-ordering is a minor format bump (ADR-0011 amendment 4) |
| R2 A second save protocol for libraries loses data on a crash | Low · high | D3's write order; IX-2's fault injection; D5 at Opus `high`/`xhigh` |
| R3 The index is slow or large on 500,000 parts | Medium · low | Not measured during development (owner's rule); the design follows doc 07 §4.3; release-phase measurement |
| R4 The valve typical needs a record kind the fragment does not carry (channels, device links, assembly children) | Medium · high | D12 widens the kinds; MC-4 is the oracle; §13.2 item 4 stops the lane early |
| R5 Golden A's re-bless (D10, FT-262, FT-265) hides an unintended output change | Low · high | The byte-equal tests of every export not meant to change; the lane lists every changed file |
| R6 XLSX parsing of hostile files | Low · medium | Size limits before parsing; calamine read-only without formulas; the importer is fuzzed with the CSV path |
| R7 The package overruns 6.8 pw | Medium · medium | The de-scope rule of §15.1 |

## 15. Size, order, routing and the documents to update

### 15.1 Size

WP-S1.19 keeps **5.8 pw PERT** (O 4 · L 5.5 · P 8.5); with D1's 1.0 pw of WP-S1.19a the parts fill **6.8 pw** with
**no reserve**: part 1 **1.2** (types and schemas 0.5, format and kinds 0.3, model table and embed 0.2, FT-24 and tests
0.2), part 2 **1.4** (tiers and reader 0.3, lock and writer 0.35, index and search 0.5, CLI 0.1, tests 0.15), part 3
**1.3** (resolver and closure 0.25, `part.assign` 0.3, `library.update` with classification 0.35, manufacturer copies
and the re-bless 0.2, status, main part, confidence 0.1, tests 0.1), part 4 **0.8** (import and map 0.45, targets and
CLI 0.15, basic project 0.15, tests 0.05), part 5 **1.5** (fragment widening and save 0.4, insert pipeline 0.6,
conflicts 0.2, typical oracle 0.2, fuzzer arm 0.1), part 6 **0.6**. The CSV is unchanged. **De-scope rule:** when a
part is projected (at its lane's midpoint report) or found (at its review) more than 20 % over its size, these items
leave the package as follow-ups without a further ruling, in this order: `ew lib create` (a library is then created by
copying a template directory), the `Deprecated` successor offer in `library_status`, XLSX sheet selection by name, the
Libraries navigator's where-used column; the lane reports which ones left.

### 15.2 Order and routing

Part 1 first (cycle 35 at the earliest; it may run in cycle 34's later wave if a slot is free). Parts 2 and 4 after
part 1, in any order or side by side; part 3 after part 2; part 5 after parts 1 and 3; part 6 after parts 2, 3 and 5.
**Routing** (Opus or Haiku only, ID-667, ID-700): part 1 to a lead-programmer lane, **Opus at `xhigh`** (file format);
part 2 **Opus at `high`** (data loss in the writer); part 3 **Opus at `high`** (core commands and a golden re-bless);
part 4 **Opus at `medium`**; part 5 **Opus at `high`** (the insertion pipeline); part 6 **Opus at `medium`**. Each part
is one lane plus the cycle's single independent review and its fix round. **Unblocks:** WP-S4.01 after part 1;
WP-S7.03 after parts 2 and 4; WP-S2.07 part 4 after part 3.

### 15.3 Follow-ups to record (numbered by the design passes)

1. Palette pins and `library.purge_unused` (doc 07 §2.8, §3.1): FT-591 (WP-S1.28).
2. Accessories, variants, classification, quality levels, consumables (WP-S1.28); EDZ, parts XML (WP-S3.07); L4 to L6
   (1.0).
3. Placeholder re-apply, representation variants, macro update, value-set CSV, rhai formulas (WP-S1.29).
4. The part selection dialog and part editor on part 3's items (WP-S2.07 part 4); FT-396's three-way paste choice
   there.
5. The release report's list of non-verified sections (WP-S1.18) on part 3's `part_confidence`.

### 15.4 Documents the design passes update

**Now (cycle 34):** plan 02 (WP-S1.19's status and this specification); decision log ID-783; FT-591. **After part
1:** doc 05 §2.2 (the implemented kinds), §3.5 (a part with `mech`), §13.2 (manufacturer copies); doc 07 §2.5 (as
built, the typed members), §2.7 (`MacroDef` as built); doc 03 §7. **After part 2:** doc 07 §2.2 and §4.7 (the index as
built), doc 05 §13.1; the user guide's library page. **After part 3:** doc 07 §3.1, §3.3 (`part.assign`), §4.5 (as
built); doc 06 §3.1 (the command count); doc 17 §2.1; doc 20 §9.2 (manufacturer copies). **After part 4:** doc 07 §3.6;
doc 14 (the parts import). **After part 5:** doc 07 §3.5, §4.6 (as built); doc 20 §9.3. **After part 6:** doc 16 §5 (the
Macros and Libraries navigators), doc 16a where a pattern is new; the traceability and the glossary (library tier,
copy state, macro instance).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S1.18, WP-S1.19a, WP-S1.19, WP-S1.28, WP-S1.29, WP-S2.05, WP-S2.06,
  WP-S2.07, WP-S2.09, WP-S3.07, WP-S4.01, WP-S7.03; Appendix B (FT-24, FT-39, FT-166, FT-167, FT-168, FT-208, FT-238,
  FT-262, FT-265, FT-274, FT-278, FT-305, FT-323, FT-396, FT-463, FT-487, FT-591).
- [Decision log](../06-decision-log.md): ID-14, ID-24, ID-206, ID-334, ID-397, ID-428, ID-512, ID-651, ID-667, ID-700,
  ID-733, ID-779, ID-782.
- [ADR-0004](../../adr/0004-identity-uuidv7-uuidv5.md), [ADR-0008](../../adr/0008-transactions-undo-preview.md),
  [ADR-0010](../../adr/0010-project-storage-jsonl-directory.md),
  [ADR-0011](../../adr/0011-format-versioning-and-migration.md),
  [ADR-0021](../../adr/0021-importers-emit-format-level-records.md),
  [ADR-0023](../../adr/0023-supply-chain-and-release-integrity.md),
  [ADR-0024](../../adr/0024-licence-set-and-permissive-gpl-boundary.md).
- [Symbols, parts and macros](../../design/07-symbols-parts-macros.md) §2 to §5, §9, §10;
  [project format](../../design/05-project-format-and-persistence.md) §2.2, §3, §5, §10.3, §13;
  [architecture](../../design/04-architecture.md) §3, §5.5, §7.7; [domain model](../../design/03-domain-model.md) §7;
  [panel layout](../../design/12-panel-layout.md) §2.5; [golden project](../../design/20-golden-project.md) §6.2, §9,
  §14, §20.2.
- Requirements: FR-PRT-002, FR-PRT-004, FR-PRT-006 to FR-PRT-008, FR-PRT-010 to FR-PRT-012, FR-PRT-014, FR-PRT-016,
  FR-PRT-023, FR-PRT-024, FR-PRT-027, FR-PRT-028, FR-PRT-031, FR-PRT-034, FR-SYM-016, FR-MAC-001 to FR-MAC-007,
  FR-MAC-011, FR-MAC-018, FR-PRJ-009, FR-PRJ-010, NFR-PERF-21, NFR-REL-07, NFR-REL-11.
