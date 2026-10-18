# ADR-0021: Importers emit format-level records; QET first; EDZ as 7-Zip; DWG via acadrust plus ODA

**Status:** Accepted · amended 2026-10-18 · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the judge panel)

## Context

- **The pilot's migration path.** The pilot user draws with QElectroTech 0.100.0-dev, so QET import is how they
  migrate. Their legacy projects contain empty and duplicate terminal designations (D-11, D-12), hand-written
  wire markings ("2.5 BK", "1.5 DBU"), PLC addresses as free text ("Q0.1") and the Tahoma font.
- **Hostile input, reusable code.** Importers parse hostile input: XML bombs, zip bombs, path traversal and
  non-finite coordinates. QET itself suffered a CVE in project serialisation (CVE-2026-15037). Third parties (ERP,
  MES, other ECAD tools) should be able to reuse the import contract and record types, and write their own
  importers, without taking GPL code. EW's own importer crates are GPL (ARCH §3), and `ew-import-qet` must be,
  because it holds ported QET code.
- **Corrections from the research** ([gaps-and-corrections](../research/gaps-and-corrections.md)):
  - EDZ packages are 7-Zip, with a ZIP minority (C-1).
  - TIA Portal V21 writes CAEX 2.15 with AR APC 1.4.0, and its exports contain tags (C-2, C-3).
  - DWG has a pure-Rust route, acadrust (MPL-2.0), so LibreDWG FFI is not justified (C-4).
  - QET's `%f` (folio position) and `%F` (folio label) differ. The reference must use `%F-%l%c`, because "102-A1"
    would otherwise print "6-A1" (C-15).
- **Licences.** Porting QET code (GPL-2.0-or-later) is allowed by the owner's licence decision. Ported code cannot
  carry the plugin exception, though (C-8, ADR-0024).
- **Test corpus.** The QET example corpus has 24 `.qet` files (C-14).

## Decision

1. **Importers are pure functions from bytes to records.** The contract lives in the permissive `ew-interop-api`
   and depends only on `ew-schema`, `ew-ids`, `ew-units`, `ew-mltext` and `ew-rds`:

   ```rust
   pub trait Importer: Send + Sync {
       fn id(&self) -> &'static str;          // "qet", "qetz", "edz", "eplan-parts-xml", "dxf", "golden-csv"
       fn version(&self) -> &'static str;     // bumps on mapping changes; recorded in the import run
       fn sniff(&self, head: &[u8], path: &Path) -> Confidence;
       fn import(&self, src: &mut dyn Source, cx: &ImportCx) -> Result<ImportBatch, ImportError>;
   }
   pub struct ImportBatch { pub records: Vec<ew_schema::AnyRecord>,   // sorted by (kind, id); IDs final
                            pub assets: Vec<AssetBlob>,              // logos, images (amendment 1)
                            pub report: ImportReport, pub provenance: Vec<SourceRef> }
   ```

   The contract's rules:
   - **Purity.** An importer reads only `src`. It opens no other file, follows no external reference and makes no
     network call.
   - **Determinism.** The same bytes, options, hints and `as_of` date give a byte-identical batch.
   - **Hints, not the model.** The host passes plain-data `TargetHints` (identifier set, structure codes, colour
     aliases, font map). Importers never see `ew-model` (ARCH §3 rule 4).
   - **Division of work.** Batches must have no dangling references. Prevent-rule violations are emitted as found
     and reported later, and human decisions travel as `Proposal`s attached to report entries.
2. **Identity.** Imported records get `v5(namespace, source_hash, source_id)` IDs and keep a `SourceRef` (QET UUID,
   EDZ part number, DXF handle; ADR-0004). Re-importing the same file therefore updates records in place instead of
   duplicating them. Amendment 2 anchors the namespace on an import lineage.
3. **Applying an import.** `ApplyImport` turns a batch into a previewed transaction: rendered candidate pages plus
   entity and report diffs (ADR-0008).
   - Normalisers and validators run in **lenient** mode (ADR-0009).
   - The **migration report is the resulting check run**.
   - A bulk import into an empty project is a checkpoint, not an undo step.
4. **QET first (MVP).** `ew-import-qet` handles `.qet` file versions 0.3 to 0.200. It is the only crate that may
   hold ported QET code, in `src/qet_port/`, under GPL-2.0-or-later with the original notices. It is excluded from
   the plugin exception and unreachable from `ew-plugin-host`. It maps:

   | QET concept | EW model |
   |---|---|
   | Folio reports (next/previous report pairs) | Interruption points (folio 102: 24VDC-1 … 4 → 103-A1 …) |
   | Master and slave elements | Main and auxiliary functions of one device (ADR-0006) |
   | Element definitions | A per-project "QET import" symbol library on QET's own 10 px grid, `7_937_500/3` nm (ADR-0005, ADR-0027; FR-IMP-008) |
   | Terminal elements and QET 0.100+ strip data | Terminal devices in strips; empty and duplicate designations kept and reported (D-11, D-12; FR-IMP-013) |
   | Title-block templates; `graphics_table` items (TOC, nomenclature) | Frame forms; report blocks of the matching built-in kind (FR-IMP-015, FR-IMP-016) |
   | The project's own `<xrefs>` and `<report>` formulas | Read per project, never assumed |
   | Conductors | Connections, with nets and potentials rebuilt from connectivity |
   | Free texts recognised as data | Typed properties: "2.5 BK" → wire override, "Q0.1" → channel address (FR-IMP-017) |
   | Tahoma | The default drawing font, with the substitution recorded (ADR-0028) |

   The spine already contains a minimal QET element reader (weeks 2–5). The full importer is stream S3 work for M2,
   validated on the 24-file corpus and the pilot's projects. `.qetz` follows in 1.x (FR-IMP-024, Should). The host,
   not the importer, opens its `project.sqlite` member with `ew-library`'s hardened read-only opener and passes a
   neutral table image through `ImportCx`, so that the importer links no rusqlite (ARCH §3 rules 2 and 4;
   [14](../design/14-interop.md) OQ15).
5. **EPLAN data (1.0).**
   - **EDZ parts** through sevenz-rust2 0.24.0, plus zip for ZIP-format packages (FR-IMP-025, FR-IMP-026). EDZ
     import is part data only (`*.part.xml` article fields, function templates as pin lists), user-initiated and
     local, and shows the Data Portal terms notice. EW never bundles EDZ content.
   - **Other EPLAN sources:** EPLAN parts XML (FR-IMP-028), report spreadsheets through a column mapper
     (FR-IMP-029, Should), and PDF/DXF pages as locked underlays (FR-IMP-030, Should).
   - **Project XML** (`.epj`) is parked (Later, Could; FR-IMP-031): at most an experimental importer behind a flag.
     Binary `.elk` and `.edb` files are out of scope.
6. **DXF and DWG.**
   - **DXF (1.0)** R12–2018 through dxf 0.6.2, as a graphic page, into a page, or through the DXF-to-symbol wizard
     (FR-IMP-032, FR-IMP-034); SHX fonts map through a substitution table (FR-IMP-033).
   - **DWG, 1.0 route (Should):** a user-installed ODA File Converter, run as an external process (DWG → DXF;
     FR-IMP-036).
   - **DWG, 1.x route:** native reading through acadrust 0.6.4 (MPL-2.0) behind a feature flag, once a DWG corpus
     test passes (FR-IMP-037).
7. **AML (1.0, `ew-plc`).** EW writes CAEX 2.15 with AR APC 1.4.0 for TIA Portal and reads both 2.15 and 3.0
   (FR-PLC-029, FR-PLC-032).
8. **Limits and fuzzing.** Every importer enforces:
   - ≤ 100,000 archive entries and ≤ 2 GiB expanded;
   - nesting-depth limits;
   - no external XML entities;
   - rejection of `..`, absolute and drive paths.

   Each importer has a `cargo-fuzz` target (NFR-SEC-02, NFR-SEC-03).

## Amendments

Accepted on 2026-10-18 ([14](../design/14-interop.md) §2.1–§2.4, open questions 1, 2 and 15). The decision text above
is unchanged; these entries refine it.

1. **Assets in the batch** (2026-10-18). `ImportBatch` carries `assets: Vec<AssetBlob>`: content-addressed logos
   and images, written by the host to `assets/<sha256>.<ext>`.
2. **Lineage and import runs** (2026-10-18; ADR-0004 amendment 1). Imported records carry
   `source_ref{lineage, key}`, and their IDs are `v5(namespace, source_key)` with the namespace anchored on an
   `ImportLineage` (the QET project UUID, or the SHA-256 of the lineage's first imported file). `ImportLineage`
   records live in `project.jsonl`; each import persists an `ImportRun` record in `imports/<run-uuid>.jsonl`, so runs
   never conflict with project data (FR-IMP-040, FR-IMP-041). A source may also be imported as a copy into a new
   lineage (FR-IMP-045).
3. **Table images and proposals** (2026-10-18). `ew-interop-api` gains `TableImage` (table name → rows of typed
   cells), which the host fills through `ew-library`'s hardened read-only opener, so a QETZ importer reads its
   `project.sqlite` member without linking rusqlite. It also gains `Proposal`: format-level intent attached to a
   report entry, which `ApplyImport` turns into a command once the user accepts it.
4. **Underlays** (2026-10-18). From 1.0 a PDF or DXF page can be imported as an `Underlay` page item (FR-IMP-030).
5. **Plugin importers** (1.x) obtain record IDs only through the host's ID minting for a source key; the host
   re-mints and compares every ID and validates every record against its JSON Schema before `ApplyImport`
   (FR-EXT-037).

## Consequences

**Positive**
- **Isolation.** Importers can be fuzzed and unit-tested without a model. They can later move into a WASM sandbox or
  a separate process without changing their contract.
- **Reuse without GPL.** Third parties can reuse the contract and the permissive record types, and write their own
  importers, without GPL code.
- **Clean re-imports.** Re-importing yields meaningful diffs, and `ew diff` shows "3 folios changed" rather than a
  new project.
- **Legacy projects open.** Legacy defects become migration messages with quick fixes instead of refusals. NFR-UX-06
  sets one hour to resolve a reference-size migration report.
- **CI fixture.** The QET corpus becomes a CI fixture from the spine onwards.

**Negative**
- **Extra host plumbing.** Importers cannot ask the model questions, so the host must copy everything they need into
  `TargetHints`.
- **Memory.** Large batches are held in memory between parsing and `ApplyImport`.
- **QETZ.** The `project.sqlite` member needs a host-side opener and a neutral table image passed through `ImportCx`
  (amendment 3; [14](../design/14-interop.md) OQ15).
- **Per-project formulas.** Reading each project's own formulas multiplies the test matrix: xref, report label,
  folio and conductor formulas.
- **Two-licence crate.** `ew-import-qet` mixes GPL-2.0-or-later and GPL-3.0-or-later files and needs REUSE
  discipline.
- **External tool for DWG.** DWG in 1.0 depends on a proprietary external tool that the user installs and EW cannot
  ship. Without it, DWG files are refused with an explanation.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **Importers writing through the GPL `TxBuilder`** (proposal A as written) | They would be GPL-coupled and not reusable by third parties, could not be tested without a model, and a model invariant would turn a parse problem into a failed import |
| **LibreDWG through FFI** | GPL-3 C code in process, which C-4 finds not justified when a pure-Rust reader exists. Its write support is partial ([interop-formats](../research/interop-formats.md) §3.3) |
| **Assuming QET's default xref formula** (`%f-%l%c`) | Wrong for the reference project, which prints folio labels ("102-A1", not "6-A1") (C-15) |
| **A clean-room QET importer only** | Possible, but slower. Porting is permitted, and confining ported code to one crate keeps the plugin exception clean (C-8) |
| **Writing CAEX 3.0 for TIA** | TIA V21 still writes and expects 2.15 (C-3). 3.0 is read only |
| **Bundling EDZ or Data Portal content** | Database rights and the portal's terms forbid it (NFR-LIC-06) |

## Related

- **ADRs:** [ADR-0004](0004-identity-uuidv7-uuidv5.md), [ADR-0005](0005-value-types-nm-dec3-mltext.md), [ADR-0006](0006-function-device-representation-model.md), [ADR-0008](0008-transactions-undo-preview.md), [ADR-0009](0009-invariant-catalogue.md), [ADR-0019](0019-rule-engine-and-rule-packs.md),
  [ADR-0020](0020-extensibility-tiers-and-security-model.md),
  [ADR-0024](0024-licence-set-and-permissive-gpl-boundary.md), [ADR-0027](0027-native-symbol-grid-and-qet-grid-policy.md),
  [ADR-0028](0028-default-drawing-fonts.md).
- **Design:** [04 architecture](../design/04-architecture.md) §3, §10.6; [14 interop](../design/14-interop.md);
  [interop-formats](../research/interop-formats.md); [plc-bus-fluid](../research/plc-bus-fluid.md) §3.1.
- **Requirements:** FR-IMP-001 to FR-IMP-045 (in particular FR-IMP-001, FR-IMP-002, FR-IMP-003, FR-IMP-004, FR-IMP-005,
  FR-IMP-006, FR-IMP-008, FR-IMP-009, FR-IMP-010, FR-IMP-020, FR-IMP-024, FR-IMP-025, FR-IMP-032, FR-IMP-036,
  FR-IMP-037, FR-IMP-040, FR-IMP-041, FR-IMP-045); FR-EXT-037; FR-PLC-029, FR-PLC-032; NFR-SEC-02, NFR-SEC-03,
  NFR-SEC-18, NFR-LIC-05, NFR-LIC-06, NFR-UX-06.
