# ADR index

**Status:** Draft v0.1 · **Date:** 2026-10-18 · **Part of:** ElektraWatt document set

**Purpose.** This page lists every architecture decision record (ADR) of ElektraWatt (EW) with its status and the decision in one line. 25 ADRs are Accepted and 5 are Proposed. Status and date are read from each file; "amended 2026-10-18" means the Accepted record was revised in the consistency pass of that date; "2026-10-19" marks the amendments from the sixth implementation cycle.

| ADR | Title | Status | Date | Decision |
|---|---|---|---|---|
| [0001](0001-core-model-persistent-maps.md) | Core model: typed Rust records in persistent maps; everything printed is derived | Accepted (amended 2026-10-18, 2026-10-19) | 2026-10-17 | Typed Rust records in persistent maps are the single source of truth; everything printed is derived |
| [0002](0002-derived-data-engine-ew-memo.md) | Derived-data engine: cold-first queries plus a cut-down memo layer (`ew-memo`); no salsa | Accepted (amended 2026-10-18, 2026-10-19, 2026-10-20: G-SP outcome Recycle) | 2026-10-17 | Derived data via cold-first queries plus a cut-down memo layer (`ew-memo`); no salsa |
| [0003](0003-freshness-contract.md) | Freshness contract: classes I–III, frozen-on-purpose identifiers, committed computations, versioned derivation profiles | Accepted (amended 2026-10-19) | 2026-10-17 | Freshness classes I-III, frozen-on-purpose identifiers, committed computations, versioned derivation profiles |
| [0004](0004-identity-uuidv7-uuidv5.md) | Identity: UUIDv7 for authored records, UUIDv5 for instantiated, imported and generated ones | Accepted (amended 2026-10-18) | 2026-10-17 | UUIDv7 for authored records; UUIDv5 for instantiated, imported and generated ones |
| [0005](0005-value-types-nm-dec3-mltext.md) | Values: i64 nm coordinates, rational grids, `Dec3` quantities, `MlText` | Accepted (amended 2026-10-18) | 2026-10-17 | i64 nm coordinates, rational grids, `Dec3` quantities, `MlText` |
| [0006](0006-function-device-representation-model.md) | Function / Device / Representation model with typed device aspects | Accepted (amended 2026-10-18) | 2026-10-17 | Function / Device / Representation model with typed device aspects |
| [0007](0007-explicit-connections-and-autoconnect.md) | Connection model: explicit connections and junction sequences; autoconnect as an editing aid | Accepted (amended 2026-10-18) | 2026-10-17 | Explicit connections and junction sequences are the model; autoconnect is an editing aid |
| [0008](0008-transactions-undo-preview.md) | Transactions, undo/redo and preview-as-fork | Accepted (amended 2026-10-18) | 2026-10-17 | Command, Transaction, Op layers; undo/redo; preview as a fork of the model |
| [0009](0009-invariant-catalogue.md) | Invariant catalogue: structural vs configurable prevent rules; lenient import | Accepted (amended 2026-10-18, 2026-10-19) | 2026-10-17 | Structural invariants vs configurable prevent rules; lenient import |
| [0010](0010-project-storage-jsonl-directory.md) | Project storage: UUID-named JSONL directory, manifest-last save, external journal, lock file | Accepted (amended 2026-10-18, 2026-10-20) | 2026-10-17 | UUID-named JSONL directory, manifest-last save, external journal, lock file |
| [0011](0011-format-versioning-and-migration.md) | Format versioning, per-kind migration and format-change windows | Accepted (amended 2026-10-18) | 2026-10-17 | Two-part format version, per-kind migration, format-change windows |
| [0012](0012-git-integration.md) | Git integration: semantic merge driver, textconv, in-file conflict records, post-merge integrity | Accepted (amended 2026-10-18) | 2026-10-17 | Semantic merge driver, textconv, in-file conflict records, post-merge integrity check |
| [0013](0013-revisions-and-as-built-archive.md) | Revisions and the as-built archive outside the project directory | Accepted (amended 2026-10-18) | 2026-10-17 | One Revision table; as-built archive outside the project directory |
| [0014](0014-gui-toolkit-egui.md) | GUI toolkit: egui/eframe behind `ew-ui`, over a toolkit-agnostic `ew-viewmodel` | Accepted (amended 2026-10-18) | 2026-10-17 | egui/eframe behind `ew-ui`, over a toolkit-agnostic `ew-viewmodel` |
| [0015](0015-display-list-renderers-pdf.md) | One display-list IR; wgpu canvas plus an always-built CPU renderer; krilla PDF/A-2b; PDF as print master | Accepted (amended 2026-10-18) | 2026-10-17 | One display-list IR; wgpu canvas plus CPU renderer; krilla PDF/A-2b; PDF as print master |
| [0016](0016-3d-renderer-wgpu-and-out-of-process-step-worker.md) | 3D renderer on wgpu; out-of-process OCCT STEP worker via occt-sys | Accepted (amended 2026-10-18) | 2026-10-17 | Own wgpu 3D renderer; out-of-process OCCT STEP worker |
| [0017](0017-read-only-sql-projection-api-v1.md) | Read-only SQL projection with versioned `api_v1` views | Accepted (amended 2026-10-18) | 2026-10-17 | Read-only SQLite projection with versioned `api_v1` views |
| [0018](0018-reports-forms-engine-and-machine-profiles.md) | Reports and forms engine; report blocks as derived pages; machine-export profiles as data | Accepted (amended 2026-10-18) | 2026-10-17 | One reports/forms engine; report blocks as derived pages; machine profiles as data |
| [0019](0019-rule-engine-and-rule-packs.md) | Rule engine and rule packs | Accepted (amended 2026-10-18) | 2026-10-17 | Declarative rules in rule packs; severity, waivers, prevent/detect modes |
| [0020](0020-extensibility-tiers-and-security-model.md) | Extensibility tiers and security model (CLI, rhai, WASM on wasmtime LTS, MCP, Python) | Accepted (amended 2026-10-18) | 2026-10-17 | Extensibility tiers (CLI, rhai, WASM, MCP, Python) over one write path, with a security model |
| [0021](0021-importers-emit-format-level-records.md) | Importers emit format-level records; QET first; EDZ as 7-Zip; DWG via acadrust plus ODA | Accepted (amended 2026-10-18, 2026-10-19) | 2026-10-17 | Importers are pure functions from bytes to format-level records; QET first; EDZ as 7-Zip; DWG via acadrust plus ODA |
| [0022](0022-platform-matrix-and-performance-budgets.md) | Platform matrix and performance budgets (S/M/L/XL) as CI-enforced requirements | Accepted | 2026-10-17 | Platform tiers and S/M/L/XL performance budgets enforced in CI |
| [0023](0023-supply-chain-and-release-integrity.md) | Supply chain and release integrity | Accepted (amended 2026-10-18) | 2026-10-17 | Per-PR and release supply-chain controls: audits, SBOM, signing, reproducibility |
| [0024](0024-licence-set-and-permissive-gpl-boundary.md) | Licence set and the permissive/GPL crate boundary | Accepted (amended 2026-10-18) | 2026-10-17 | Licence map per path; permissive format crates, GPL application, enforced boundary |
| [0025](0025-gpl-plugin-exception-wording.md) | Wording of the GPL plugin exception | Proposed | 2026-10-17 | Wording of the GPL plugin exception (pending counsel) |
| [0026](0026-default-device-tag-identifier-set.md) | Default device-tag identifier set for new projects | Accepted (amended 2026-10-18) | 2026-10-17 | EN 81346-2:2009 single-letter identifier set as default for new projects |
| [0027](0027-native-symbol-grid-and-qet-grid-policy.md) | Native symbol and connection grid; policy for QET-grid pages | Proposed | 2026-10-17 | Native symbol and connection grid; policy for QET-grid pages (pending pilot input; the 4 mm default is in force since 2026-10-18) |
| [0028](0028-default-drawing-fonts.md) | Default drawing fonts | Proposed | 2026-10-17 | Default drawing fonts (pending pilot input; the Noto Sans default is in force since 2026-10-18) |
| [0029](0029-network-telemetry-crash-reporting-policy.md) | Network, telemetry and crash-reporting policy | Proposed | 2026-10-17 | Network, telemetry and crash-reporting policy (pending) |
| [0030](0030-legal-steward-cra-role-support-signing.md) | Legal steward, CRA role, support period and signing identity | Proposed | 2026-10-17 | Legal steward, CRA role, support period and signing identity (pending) |

## How to add an ADR

1. Copy the header of the latest ADR: title `# ADR-nnnn: <title>`, then the metadata line
   `**Status:** Proposed · **Date:** <date> · **Deciders:** <who>`.
2. Use the next free number and a short kebab-case file name: `docs/adr/nnnn-title.md`.
3. Write context (forces, constraints, requirement IDs), decision (numbered, checkable), alternatives considered,
   consequences, and the work packages and documents it touches.
4. Open it as Proposed; it becomes Accepted when the deciders named in it agree. Never rewrite an Accepted ADR
   silently: amend it with a dated note, or supersede it with a new ADR that links back.
5. Add a row to the table above, and to Part A or Part B of the [decision log](../plan/06-decision-log.md).
6. Run `python3 -I docs/plan/tools/doccheck.py` to check links and tables.

See [open-source governance](../design/19-open-source-governance.md) §3.4 for the RFC and ADR process.
