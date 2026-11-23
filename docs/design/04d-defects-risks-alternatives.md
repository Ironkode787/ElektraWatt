# Architecture: the defects it prevents, the first build order, risks and alternatives

A chapter of the [architecture](04-architecture.md): its §14 to §17 and Appendix A, under the numbers the code and the other
documents cite. The crates, their order, where a thing lives, how a text reaches a person and what the shell
and the window own are in the architecture itself; read this chapter by the section you need.

---

## 14. How the architecture prevents the reference defects

**Classes:**
- **S, structural:** impossible by construction.
- **I, invariant:** prevent mode.
- **C, check:** online, offline or release.
- **A, assisted:** tools help, intent stays human.

| Defect | Class | Mechanism |
|---|---|---|
| D-01 stale -4W1 after renumber | S | Tags exist only as `device_tag`. BOM, layout and xrefs read the same query |
| D-02 layout T51 vs T55; free rectangles | S | `Placement3D` references a device, and panel fronts are derived from placements (`panel_front_reps`); labels come from `layout_label`. An orphan-graphic check covers the rest |
| D-03 mixed schemes, "+JE2416" | S + C | One canonical structure node; numbering rules per project; frozen tags explicit and reported |
| D-04 BOM incomplete, duplicated, no quantities | S | `bom_rows` aggregates parts on devices, assembly children, carriers and accessories; one strip is one row with a quantity |
| D-05 four revision formats | S | One `Revision` table and format; the title block reads it |
| D-06 description drift on 101 | S | Title block bound to the project property; a page override is explicit and flagged |
| D-07 no bookmarks, links or page labels | S | Outline from `page_order`, link annotations from xref link targets, `/PageLabels` from folios |
| D-08 free-text wire rule | S | `wire_props_of` with provenance; cover tables generated from the profile |
| D-09 missing reports and wire numbers | S | Built-in report pack (wire list, terminal, cable, PLC, potential overviews) as report blocks; connection numbering (`NumberSpec`) |
| D-10 overview 5 vs 4 valves | S + C | Overview representation generated from the assembly's children; hand-drawn-overview count check (REF-087) |
| D-11 missing terminal designation | I | Prevent rule (strict); lenient on import with a migration message |
| D-12 duplicate designations | I | Same, unique per strip |
| D-13 N switching (T55, S56) | C | `EW-POT-013` on derived potential types |
| D-14 "3-B0" | S | `rep_anchor_cell` from the symbol anchor policy; empty-cell check |
| D-15 address "TBD" | S + C | `AddrSpec::Tbd` is a typed open item; it blocks release |
| D-16 FE vs PE mixed | C | Potential types from part pin functions; FE ≠ PE rule |
| D-17 core colour "BR" | S + C | `ColourCode` enum plus profile codes; cores validated against the cable part |
| D-18 typos, manufacturer names, "50hz" | S + C | Manufacturer master data from the library; typed `Dec3` values with icu unit formatting; description lint |
| D-19 missing supply cable, lengths | C | External connection without a cable; missing lengths block release; lengths as committed computations |
| D-20 junction notation | S | `Junction` entities with style and sequence |
| D-21 PE continuity not drawn | C | PE bonding through carriers (rails, plate); offline continuity rule |
| D-22 0 V–PE merged by a wire | S | `Bond` links potentials without merging; type conflict without a bond is reported |
| D-23 unconnected 13-14 | C | Unconnected-CP rule unless marked `unused` |
| D-24 five IP names on one net | S | Canonical potential name with aliases |
| D-25 crossed supply pairs, one channel | C | Load summary per channel and potential; potential-pair report (user SQL example, §11) |
| D-26 odd-bit addresses | S + C | `channel_addresses` from the module start and the part address map; `EW-PLC-012` |
| D-27 untagged valves and process valves | S | Child devices of the assembly with derived tags; typed `process_tag` and field location |
| D-28 fluid model incomplete | C | `EW-FL` rules: supply on port 1, exhaust or silencer, tube spec |
| D-29 tag text sizes vary | S | Text styles bound to slot kind |
| D-30 customer vs end user | S | Typed party roles |
| D-31 3.70 pt text | C | Printed-height check at print scale |
| D-32 layout incomplete | C | `EW-LAY-001` unplaced devices; layout derived from devices |
| D-33 physical page labels, lost spaces | S | `/PageLabels`; glyph runs with real spaces |
| D-34 author = checker | I | Structural invariant on release |
| D-35 description misuse | S + C | Separate description and technical-data fields from the part; "rating in description" lint |
| D-36 order code vs components | S | `Assembly` aspect with config code; BOM in assembly and component modes |
| D-37 colour-profile basis | S + C | Profiles name their basis (standard vs company convention); rule |
| D-38 PE smaller than phase | C | PE sizing rule on derived cross-sections |
| D-39 IP ordering | A | Sort and arrange tool for interruption points |
| D-40 CPX block inside MPA section | A | Configurator-validation rule pack, as a plugin with vendor data |
| D-41 assembly tagged from two pages | S | Assembly-level tag with child numbering |
| D-42 strip drawn twice | S | One strip device with several segment representations; one BOM row |

**Reproducing the golden project (Golden A, until it was retired at 0.3.5).**
- **Tags.** S51, F55, T55, E55, S56, E56 and 1X1 are kept as frozen tags and listed in the numbering report. Every
  other tag is derived.
- **Frame and xrefs.** The QET-compatible frame form has 17 columns starting at 0 and rows A–H. The xref format is
  QET's `%F-%l%c` equivalent (C-15).
- **Profiles.** The house colour profile "DBU/WH 24 V" and the size profile (0.75 / 1.5 mm²) generate folio 1's
  tables.
- **Generated pages.** Section dividers 100, 200 and 900 and report folios 2, 901 and 903–911 are generated.
- **Expected check results.** `ew check --scheme pre-delivery` reports exactly the expected open items: D-13 unless
  corrected, D-26, the TBD address, missing parts on S56 and E56, and unplaced devices.

---

## 15. Spine build order and parallel streams

### 15.1 The spine: two lanes from day one

The critical path to the golden PDF is not only the model. It is also **symbols, page geometry and frame forms**
(feasibility judge). The spine therefore runs as two lanes that meet at `ew-schema` and the query catalogue.

**Effort, not calendar.** The first column below counts *effort weeks* of two full-time lanes. It fixes the order
and the dependencies of the spine's work, not dates. The calendar is the plan of record: the spine ends at milestone
M1, which the global plan dates in §5.1 and §7 and
milestones.csv (month 16 for three developers and the lean scope, because the same work
shares the team with the rest of S0, S1, S3, S6 and S7).

| Effort weeks | Lane 1: core (dev 1) | Lane 2: output (dev 2) |
|---|---|---|
| 0–2 | `ew-units`, `ew-ids`, `ew-mltext`, `ew-rds`; `ew-schema` v0 for the golden subset (co-owned) | `ew-dl`; symbol, function-definition and form record kinds in `ew-schema` |
| 2–5 | `ew-model` (tables, ops, base indexes, invariants); `ew-commands` (`TxBuilder`, `History`, normalisers, about 20 commands, preview fork) | Starter symbol set as data: about 40 IEC 60617 and ISO 1219 symbols used by J-E-2416. Minimal **QET element reader** (`.elmt` geometry and terminals embedded in `.qet`) into a per-project "QET import" library on its native grid |
| 3–5 | **Performance spike (gate G-SP in the plan, right after WP-S1.03):** synthetic L and XL replicated from the golden project; cold-query latency for move, wire, renumber and profile change; global connectivity; open time and memory. **Output:** the list of queries to memoise and the go/fallback decision (§6.1) | `ew-text` and `ew-scene` frames and forms: QET-compatible 17 × 8 frame, title block with bound fields, text stack and bundled fonts |
| 5–9 | `ew-derive` (structure, tags, anchors, xrefs, two-level nets, potentials, wire properties, page order, TOC, BOM, title block), cold first; `ew-memo` for the queries the performance spike selected; oracle mode | `ew-render-cpu` and PNG goldens; `ew-reports` pagination (TOC, BOM 901, terminal, wire); `ew import csv` golden bootstrap (`ew-import-tables`) |
| 6–9 | `ew-format` and `ew-store`: JSONL writer and reader, manifest, save protocol, journal, lock | krilla PDF: outline, links, named destinations, page labels, PDF/A-2b |
| 9–12 | `ew-sqlview` v0 and the acceptance runner (retired at 0.3.5); `ew query --explain`; perf gate in CI | Integration: `ew export pdf golden/`; visual and PDF structure tests |

**Spine exit (M1).** The exit criteria are the content of gate G-M1 (gates §5); they were met on the reference
project, which the sample project replaced at 0.3.5:
- `ew export pdf golden/J-E-2416.ew` produces the 18 folios plus report folios 903–911.
- §4.15 criteria 1–7 and 9 pass: pages, frame, xrefs, tags, BOM, terminals, wire data, PDF structure.
- The acceptance SQL/CSV pairs pass with the oracle on, still without a GUI.
- The XL synthetic project opens in ≤ 20 s, and commit p99 is ≤ 5 ms at M and ≤ 20 ms at XL for move and wire
  gestures.
- **Ownership:** `ew-memo` and the `Op` set have two named owners. Each lane's developer has reviewed the other's
  core crates.

### 15.2 Parallel streams after the spine

The query catalogue and `ew-schema` are the contract. UI and output developers code against `World`, the queries
and golden data, not mocks.

| Stream | Owns | First deliverables | 2 devs | 3 devs | 4 devs |
|---|---|---|---|---|---|
| **S1 Core and derivation** | Model breadth (cables, PLC, bus, fluid, assemblies, revisions), numbering, `ew-checks` and packs, merge driver and integrity, projection breadth, plugin and MCP read API | Full golden model; `ew check` with §4.15 criterion 8 | dev A | dev 1 | dev 1 |
| **S2 Interaction** | egui shell, `ew-render2d`, tools, navigators, property grid, keymaps, a11y, view-models | Read-only viewer first, then place, wire, move and undo on folios 101–106 | dev B | dev 2 | dev 2 |
| **S3 Documents and migration** | Report pack and form editor, SVG/DXF, **full QET importer** (folio reports → IPs, master/slave → main/aux, per-project `%F`/`%f` formulas, potentials from connectivity), EDZ parts, AML/TIA/PLCopen, manufacturing outputs and machine profiles, `.ewpp` | QET corpus (24 examples) and the pilot's projects; Komax CSV and wire list | dev A | dev 3 | dev 3 |
| **S4 Physical** | Layout model, STEP worker, `ew-render3d`, collision reconciler, routing, lengths, duct fill, 2D panel projection | Folio 902 driven by placements; wire lengths in the wire list | dev B, from the start of WP-S4.01 | dev 2, from the start of WP-S4.01 | dev 4 (joins at month 4 in the plan) |

**Milestones.** The content of each milestone is below; the dates are the plan's (see
global plan §3.4 and §5.1 and milestones.csv). The "plan of
record" column quotes month *n* counted from November 2026 for three developers and the lean scope; other team sizes
and the re-baseline at G-M1 are in the plan.

| Milestone | Plan of record | Content |
|---|---|---|
| M1 | month 16 | Spine exit: the headless golden PDF (§15.1) |
| M-PP (pilot preview, release 0.0.x) | month 26 | A slice of S1 and S2 for the pilot only: interactive 2D editing of new projects with the starter symbols, core reports, PDF and core online checks; unsigned build; gate G-PP |
| M2 (MVP) | month 41 | Interactive 2D editor MVP, QET import of the corpus (the plan's lean variant ships the project migration in the next 0.x, see [vision](00-vision-and-scope.md) §9.1), check packs and report pack, 2D panel front, usability test with two EPLAN users and one QET user, first release (0.1.0, unsigned) |
| M3 | month 47 | PLC, bus and fluid 1.0; QET migration; EDZ, AML, TIA tag and PLCopen exchange; DXF and DWG |
| M4 | month 58 | 3D panel, collision, routing and lengths; manufacturing outputs and machine profiles |
| M5 | after M4 | Fluid power (release 0.5.0) |
| M6 | month 59 | 1.0 feature complete (beta) |
| M7 (v1.0) | month 65 | Hardening, docs, format freeze, signed 1.0 |

**Rotating schema-steward role.** Every format change passes the steward rule (§7.6).

---

## 16. Risks, mitigations and non-goals

| # | Risk | Mitigation |
|---|---|---|
| 1 | **Stale memos** recreate the D-01 class invisibly | Untracked reads do not compile; cold mode by default; oracle in CI, debug and nightly; command fuzzer; `ew verify-derived`; `ew query --explain`; any query can be dropped back to cold with a one-line change |
| 2 | **Interactive latency at L/XL** (global connectivity, bulk edits) | Two-level nets with stable `PotentialId`s; backdating; bulk commands as preview jobs; performance spike (G-SP) early in the spine with a named fallback (§6.1); engine statistics asserted in the perf gate. Partly materialised at G-SP (2026-10-20, outcome Recycle): incremental connectivity, push invalidation and the reshaped connectivity queries (§6.1 to §6.4, ADR-0002 amendment 4) |
| 3 | **Memory at XL** (memos, projection, undo) | Granularity rules (§6.3); LRU and memory gate (specified for 1.0; not built); file-backed projection at XL; 256 MB undo cap |
| 4 | **`ew-memo` bus factor** | Small v1 engine (1.5–2.5 kLOC); generic crate with design notes; two owners by M1; most contributors write plain functions |
| 5 | **Derivation drift** across EW versions changes archived drawings | Pinned derivation profiles; golden tests per profile; as-built archive with PDF |
| 6 | **egui/wgpu churn and support periods** | `ew-ui` and `ew-viewmodel` isolation; quarterly lock-step upgrades; CPU renderer; a support-period policy decided with the CRA role (ADR-0030) |
| 7 | **Messy legacy imports** (D-11/D-12, custom xref formulas, Tahoma) | Record-level importers; lenient mode; migration report as a check run; per-project formula reading; font substitution recorded |
| 8 | **SQL projection becomes a fragile public contract or an attack surface** | Versioned `api_v1` tied to the format major; sandbox; trust prompt; fuzzing; SQL is never a write path |
| 9 | **Semantic merge conflicts** after git merges | Record- and field-level merge driver; in-file conflict records; post-merge integrity check with quick fixes; branch-safe "next free" counters |
| 10 | **Licence-boundary mistakes** (GPL code reaching permissive crates; ported QET code) | Closed permissive layer from the first commit; CI licence-boundary check; REUSE headers; DCO; QET-derived code only in `ew-import-qet` |
| 11 | **Symbols and forms on the golden critical path** | Spine lane 2 from the start; starter symbol set; QET element reader |
| 12 | **EPLAN users expect autoconnect semantics** | Autoconnect as an assistant that creates explicit connections with a ghost preview; usability test before M2 (ADR-0007) |

**What we explicitly do NOT build in v1:**
- **Core:**
  - no salsa, no ECS, no SQLite or other database as the live model, no stored derived data;
  - no parallel memo evaluation and no memo merge-back;
  - no CRDT or real-time co-editing (the op log stays CRDT-ready; Loro is the candidate).
- **UI:** no custom UI toolkit, no multi-row ribbon with contextual tabs (the single-row command bar of [ADR-0031](../adr/0031-design-language-and-task-ordered-command-bar.md) is not one), no free-form native undocking.
- **Plugins and scripting:** no native dylib plugins, no embedded CPython.
- **Geometry:** no B-rep kernel of our own, no routing in the interactive path.
- **Writes and semantics:** no SQL write path for anyone; no autoconnect as model semantics.
- **Formats:**
  - no EPLAN binary formats (`.elk`, `.edb`);
  - no LibreDWG FFI (acadrust plus the user-installed ODA converter);
  - no optional-content PDF layers;
  - no HPGL;
  - no TIA Openness in the core.
- **Network and data:** no network traffic without a user action, no telemetry, no bundled EDZ or Data Portal
  content.

---

## 17. Alternatives considered

| Proposal | Scores (feasibility / domain / longevity) | Why it lost |
|---|---|---|
| **C: relational, SQLite as the live model** | 7.5 / 7 / 7.5 = 22 | It tied with A and has the best debuggability and the best path for contributors who do not write Rust. It lost for four reasons:<br/>(1) It keeps **three representations of the truth**: text at rest, the SQLite working DB, and Rust scene caches or mirrors. Its own latency fallback (mirror hot `d_*` tables in Rust) converges on A.<br/>(2) Its asynchronous writer actor makes every interactive tool reason about uncommitted state.<br/>(3) **Its constraint story fails the primary migration path.** SQLite `UNIQUE` and `CHECK` constraints cannot be deferred, contrary to the proposal, and "staging with the same DDL" cannot ingest QET projects containing D-11 or D-12. The fix (relaxed constraints as configurable rules) removes the "40 invariants for free" advantage.<br/>(4) Graph and geometry derivations still rely on hand-written dirty-key mapping.<br/>Its distinctive strengths are **grafted as derived artefacts**: the read-only SQL projection with versioned views, SQL reports and rules, machine profiles as data, CSV and SQL acceptance tests, JSONL storage, hostile-database hardening, the schema-steward rule and the enumerated invariant catalogue |
| **D: retained UI plus document object model** | 5 / 6 / 6.5 = 17.5 | **Freshness rests on discipline, not construction**: a missed subscription is the D-01 class.<br/>Slint means about 12 dev-weeks (likely 20+) of DIY docking, trees, tables, property grid, gizmo and an accessibility harness. Its canvas interop is still an unstable feature re-keyed with every wgpu major, which undoes the stability argument.<br/>Its own S-expression format and parser is avoidable work. 100k-row Slint tables are unverified.<br/>**Grafted:** the always-built CPU canvas path, the toolkit-agnostic view-model layer, declarative plugin dialogs, the permissive display-list IR, commit-time normalisers, `Dec3`, link delete policies, read-tracking and field-mask tests, the command-sequence fuzzer, jobsets, `ew render --png`, the yearly format window and the supply-chain CI set |
| **B: ECS (bevy_ecs)** | 4 / 6.5 / 6 = 16.5 | The core depends on the bevy_ecs APIs churning right now (observers, lifecycle events, relationships, disabling components). Its escape hatch to hecs or slotmap would remove exactly the hooks its transactions and undo rely on.<br/>A World cannot be snapshotted, so previews are modal and offline checks run chunked on the main thread.<br/>Many-to-many relations need reified edges, and there is a dual Entity/Uid identity.<br/>A four-month all-hands spine ends with only three folios.<br/>**Grafted:** committed computations with input hashes, deterministic v5 IDs, ECS-free importer records, typed device aspects for multi-role devices, per-kind schema versions with unknown kinds preserved, namespaced plugin data, in-file conflict records, shuffle-order determinism tests and Fluent message keys |
| **Engine-level alternatives** | — | **salsa 0.28:** a write cancels or blocks in-flight readers (exports must run on their snapshot), its inputs would duplicate our tables, and it is still 0.x.<br/>**CRDT (Loro):** merges into semantically invalid states, with logs opaque to git.<br/>**A full hand-rolled incremental engine** (proposal A as written): replaced by the cut-down `ew-memo` |

---

## Appendix A. Review findings on proposal A and their resolution

| Finding (judge) | Resolution in this specification |
|---|---|
| Hand-rolled engine is the largest novel infrastructure on the critical path; parallel fork evaluation and memo merge-back are subtle (feasibility, domain) | Cold-first queries; `ew-memo` without parallel evaluation, sharded locks or merge-back; adopt-if-unchanged only (§2, §6.1–6.2) |
| Bus factor of one on the engine and the `Op` set (feasibility, longevity) | Two owners by M1; small generic engine; contributors write plain functions (§15.1, risk 4) |
| Salsa decision gate not clean (salsa cancels readers on write) (feasibility) | Salsa rejected by ADR. The performance spike compares cold vs memoised instead (§15.1) |
| Macro and rhai-in-query complexity (feasibility) | Tracked access by module privacy (`&Q` only), not a macro DSL; rhai in queries limited to pure formulas (§6.1, §10.3) |
| No PDF in the spine; the golden critical path is symbols and forms (feasibility) | Two-lane spine with symbols, forms and krilla PDF from the start; minimal QET element reader; CSV bootstrap (§15.1) |
| Unflagged or inconsistent versions (directories 6.0.0, rayon 1.12.0; "wasmtime 49.0.3 pinned to 48 LTS") (feasibility, longevity) | Marked **(verify)**; "the LTS line current at plugin launch, 48.x in development, 49.0.3 current" (§2, §3) |
| Memo memory at XL not shown; no granularity rule (domain) | Granularity rules, per-potential grouping, memory gate, explicit XL budget (§6.3, §6.7) |
| Revision diff: where the frozen model lives was vague (domain) | As-built archive with canonical text, loaded into a read-only `Model` for `diff_model` and `diff_query`; git tag (§7.8) |
| Report types are a fixed enum; no user reports (domain) | User SQL reports over the projection, plus built-in typed reports (§10.2, §11) |
| Parts on Device while terminals were functions carrying parts; inheritance chain lacked signal/net levels; D-09 missing from the defect table (domain) | Terminals are devices; parts attach only to devices; precedence override > net def > potential def > profile > default; D-09 mapped (§4.2, §6.4, §14) |
| Permissive `ew-format` and `ew-interop` depended on GPL crates (longevity) | Closed permissive layer (`ew-schema`, `ew-format`, `ew-interop-api`, …); importers emit records; CI boundary check (§3, §10.6) |
| No supply-chain or CRA story (longevity) | SBOM, cargo-deny/audit/vet/auditable, SLSA provenance, signing, REUSE, DCO; steward and CRA as an owner decision (§13, ADR-0030) |
| Pretty-printed JSON arrays conflict often in plain git (longevity) | JSONL, one record per line, ordered by the ID's random bits (§7.3) |
| Immediate-mode assumption with no notification layer for a toolkit swap (longevity) | `ew-viewmodel` with revision stamps over a change feed (§5.2, §9) |
| Invariant enforcement on legacy data (cross-check of C's flaw) | Invariants checked at transaction end; prevent rules configurable, lenient on import (§4.6) |
