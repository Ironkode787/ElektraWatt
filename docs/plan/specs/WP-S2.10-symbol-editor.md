<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# WP-S2.10: symbol editor

Specification, 2026-11-05. Base: `ac9cc481`, after cycle 52. Branch: `feature/symbol-editor`.
Authority: ID-1402, ID-1405, ID-1435, ID-1445 and the owner's direct instruction to design and implement,
including an ECAD design standard, AI authoring and EPLAN/QElectroTech imports. One lane builds the whole package.
The owner's implementation instruction supersedes the handoff's intermediate specification-only hand-back.
Release promotion, independent review and G-M2.5 remain the orchestrator's work.

## 1. Goal and non-goals

Draw, edit, validate, preview, save and place reusable symbols. Manual and automated authors use the same model
and validation. Imported symbols enter an editable draft with their source and limitations visible.
The CPX power-block acceptance has six connection points, designated 1 through 5 and FE.

No project-format change, automatic conversion of QET grids, parametric families, library-wide lint catalogue,
3D model editor or release promotion. Importing a device's factual data must never be described as importing
its original symbol graphics. Unsupported binary records must be reported rather than silently omitted.

## 2. Inputs and code at the base

| Area | Existing implementation |
|---|---|
| Symbol data | `ew-schema/src/library.rs`: SymbolDef, Primitive, SymbolCp, TextSlot, AnchorPolicy, FnDef |
| Geometry | `ew-scene/src/symbol.rs`: deterministic lowering, all A–H orientations, readable texts and anchors |
| Libraries | `ew-library/src/registry.rs`: resolve, closure, create, write; manifest-last writes and version guards |
| Application boundary | `ew-api`: Session, library registry and import front door |
| GUI/headless | `ew-viewmodel`: ShellModel::dispatch and action registry; `ew-ui`: egui frame |
| QET | `ew-import-qet`: bounded XML element reader, graphics, terminals, generated function definition, provenance |
| EPLAN | `ew-import-eplan`: manifest and empty implementation; no functioning EDZ or macro reader |

## 3. Requirements traced

FR-SYM-017 (graphics, CPs, function definition, text slots, anchors, variants and live preview), FR-SYM-009
(4 mm connections, finer graphics/text grids, save refusal), FR-SYM-004 (stable CP identity in variants),
FR-SYM-005 (one function definition), FR-SYM-010 (explicit anchor), FR-SYM-014 (link points), FR-SYM-018
(import fork), FR-SYM-019 (licence/source), NFR-A11Y-02 and the G-M2.5 checklist.
The owner's additions require a written authoring standard, documented machine-readable authoring,
QET element import and investigation/implementation of EDZ and native EPLAN symbol/macro input.

## 4. Decisions

| # | Decision |
|---|---|
| D1 | A separate editor draft and undo history. Editing a library symbol never mutates already placed project copies. Closing/replacing an unsaved draft requires an explicit discard. |
| D2 | Preserve format 0.1. Authoring interchange is a separately versioned JSON envelope around existing SymbolDef/FnDef data. Publishing creates ordinary library copies with deterministic IDs and canonical hashes. No migration is required. |
| D3 | Apply [the symbol design standard](../../design/07a-symbol-design-standard.md). Native CPs on 4 mm, graphics on 0.5 mm and text on 1 mm. Numeric off-grid CP edits remain visible as errors and prevent publication; never silently move a pin. |
| D4 | Use the existing geometry lowering for previews. A–H are derived, never eight separately maintained drawings. Keep pin identities and readable text. GraphicCentre is the initial cross-reference anchor; insertion origin is distinct. |
| D5 | Validation is shared by GUI, headless authoring and import. Check finite bounded geometry, reference integrity, unique indices, CP mapping/discipline, the grid, slots, anchors, names and provenance. Errors block publishing; advice does not. This is editor validation, not WP-S7.04's library lint rules. |
| D6 | Expose documented `symbol.*` actions and authoring JSON for AI programs. Millimetre action parameters, templates and concrete examples avoid hand-managing IDs. Draft changes and validation are available without an external service. No generated code or remote URLs execute. |
| D7 | Save through the library writer, preserving its lock, writability, semantic-version and atomic-generation rules. New symbols have their own function definition; assigning an existing definition retains its semantics. Placement uses the existing command with embedded copies and one undo transaction. Creating a scratch page is a separate undoable operation; undo placement, then undo page creation. |
| D8 | QET `.elmt` uses the existing bounded reader and keeps the QET grid, origin, unsupported-data report and source. A native-grid conversion is not implicit. |
| D9 | EPLAN scope includes EDZ and `.slk/.ema/.ems` as requested. Verify against manufacturer downloads kept outside the repository. Record supported input versions and feature losses. Binary macro/symbol decoding requires evidence; a factual-data fallback must be explicitly identified as a generated symbol, with original graphics unavailable. |
| D10 | UI follows doc 16a: drawing leads, task-ordered labelled tools, properties beside it, validation in words, all identifiers readable. Use the main ribbon's small-button renderer: 16 px icons with labels, shared theme tokens and active states. Geometric glyphs use the existing EW 24-unit icon set; functions reuse existing icons. Inspect at 1920×1080 and 1280×800. New text lives in en-GB and nl-NL. |

## 5. Build order (one lane)

1. Design standard, this specification, plan and threat-model updates.
2. Draft model, shared validation, canonical publishing, import adapters and meaningful failure tests.
3. Toolkit-neutral editor actions/history, live preview and placement integration.
4. Editor canvas, property controls, import/export/save flows, keyboard access and bilingual text.
5. Authoring examples, tutorial/reference, GUI and headless acceptance, targeted checks and hand-back.

## 6. Acceptance

- A new power block with graphics and pins 1–5/FE can be edited, previewed in all orientations, saved, reopened
  and placed with its function definition. Save/placement uses no private reference data.
- A connection at 4.5 mm is refused with the offending pin in words; graphics at 0.5 mm remain valid.
- Undo/redo works across geometry, CP mapping and text edits. Failed parse/save does not destroy the draft.
- A–H preserves indices and directions; text is readable; off-grid imports remain inspectable but unpublishable.
- An AI-generated authoring file goes through exactly the editor's validation and can be published without UUID
  construction. Invalid/oversized/unrecognised input is refused before any library mutation.
- QET import preserves graphics, CP designations and grid. EPLAN tests distinguish full geometry from factual
  fallback and exercise malformed archives, traversal, size limits and unsupported macro versions.
- Existing format 0.1 files still load; no existing schema changes. Targeted package tests and one lane-check.
- G-M2.5's independent blind-persona round, release pipeline and G-M2 regressions remain release-gate evidence;
  they are not inferred from unit tests.

## 7. Neighbours, owned files and limits

Own new editor modules in ew-api, ew-viewmodel and ew-ui, their actions/locales/tests, EPLAN import implementation,
CLI authoring entry points where needed, and these design/guide/plan updates. Reuse existing library and placement
contracts. Do not change the project schema, Golden A, decisions.md, follow-ups.md, release branches or main.

## 8. Risks and questions

EPLAN binary compatibility and licensing are the largest uncertainty. Manufacturer files are local test evidence,
never committed or used as generated starter artwork. Preserve source terms. No claim of IEC certification or
full EPLAN compatibility. The owner selected both EDZ packages and native library/macro inputs and authorised
manufacturer downloads for compatibility tests. Other routine design decisions are D1–D10 above.

Proposed **ID-?**: adopt the ECAD authoring standard and common GUI/AI validator without changing format 0.1.
Proposed **ID-?**: accept the above import scope with truthful per-feature reports.
Proposed **FT-?**: any unsupported EPLAN binary versions found in validation, with evidence and exact limitation.
Only the orchestrator numbers these records. Dutch text added by this package is listed by the `symbol-` and
`action-symbol-` key families in the locale files for terminology review.

## 9. Documents and sources

Update doc 07, WP-S2.10 in plan 02, the threat model, the guide tutorial/reference and the authoring examples.
Sources: handoff WP-S2.10; ADR-0011/0027; docs 07/16/16a; the neighbouring specifications;
[EPLAN symbol variants](https://www.eplan.help/en-US/Infoportal/Content/Plattform/2026/Content/htm/symboleditorgui_h_mitsymbolvariantenarbeiten.htm),
[EPLAN symbol interchange](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/symboleditorgui_h_symboleimportierenexportieren.htm),
[KiCad library conventions](https://klc.kicad.org/symbol/s3/).

## 10. Implementation and initial verification evidence

- `ew-api::symbols`: portable draft, common validation, schema, QET/EPLAN adapters, atomic new-file export,
  embedded/library forks, assigned-function consistency, canonical library publication. No format 0.1 changes.
- `ew-viewmodel::shell::symbol_editor`: independent bounded history, all editing actions, import candidate selection,
  function assignment, publication, scratch pages using the symbol grid and placement only on a compatible page grid.
- `ew-ui::symbol_editor`: canvas tools, graphics selection/movement, numeric properties, pin mapping/direction,
  slots, anchors, linkage points, live A–H, import review and advanced JSON. Graphics use the production CPU renderer.
- `ew symbol template|schema|validate|import|publish`: data-only AI authoring, independent of an open project.
- `ew-import-eplan`: bounded 7z/ZIP and PXF 1.23 drawing subset, plus clearly marked generated parts outlines.
  Verified locally with the manufacturer's P30O EDZ download; source files remain private. The preserved source
  identity is a content hash, not the source's embedded workstation/user metadata.
- The affected-package suite exercised 3,540 tests (19 skipped): 3,538 passed in the final full run;
  the remaining coordinate-label and book-translation checks passed on their focused rerun after correction.
  This includes API authoring, GUI/headless actions, archive/XML boundaries, QET conversion-report round trips,
  pin-mapping preservation, repeated scratch-page creation and the opt-in manufacturer EDZ sample.
- Formatting, generated-file settlement, clippy, documentation/doctests and fuzz-target compilation passed.
  The remaining policy failures are exactly the dependency gates below. No snapshot baselines were changed.
- Offscreen scripts passed at 1280×800 and 1920×1080: six-pin drawing, A–H, pointer drawing, off-grid feedback,
  export, library save and embedded placement. Separate scripts passed for assigned functions, two successive
  scratch pages and the manufacturer's EDZ preview (33 editable candidates). Images/transcripts remain local.
  The manufacturer's source identity is SHA-256
  `01c7910f1625f15d6f284cf6fc7092de82b6cab848eabed7b274c959f60c2685`.
- Performance: not measured (owner's rule). Independent acceptance and release promotion remain pending.

### Exact limitations and remaining gates

Direct `.slk` import now reads its sibling `.sdb` drawing tables without EPLAN (SLK 1.2, FE53 tables,
UTF-16LE/UTF-8 descriptors). It reconstructs A-variant lines, axis-aligned rectangles, full circles and supported
connection-point records. Compressed records and Blob continuations are bounded and checked. Other primitives,
static text, fills/styles and unresolved connections remain explicit review findings. EPLAN B–H are not imported
as independently authored variants; EW derives them. Unknown database layouts and legacy descriptor encodings
are refused. A public library was inspected privately; no vendor content is redistributed.

Binary macros remain refused. PXF 1.23 supports lines, rectangles, polylines, full circles and basic text;
`.esl` is supported only for that PXF structure. Full EPLAN compatibility, native function semantics and complete
manufacturer-part import are not claimed.

Proposed **FT-?**: expand the verified SLK subset to remaining primitives/text/styles/encodings and independently
authored variants; implement symbol-library exports and library-resolved macro connections with synthetic fixtures.
Proposed **ID-?**: decide the five pinned decoder dependencies (`sevenz-turbo` 0.26.0, `lzma-turbo` 0.6.0,
`crc-fast` 1.10.0, `spin` 0.10.1, `zeroize` 1.9.1). `cargo-vet` has no approvals for them, and `cargo-deny` rejects
`lzma-turbo`'s GPL-3.0-or-later licence under the current external-dependency allowlist. The owner explicitly chose to keep release blocked pending dependency review. No dependency exemptions or licence
policy changes have been made. The GPL application graph uses Rust 1.97.1 for this importer; the workspace's
pinned compiler is 1.99.0 and the permissive crates' MSRV is unchanged. These checks block release approval.

The independent G-M2.5 blind-persona review and release promotion remain with the orchestrator. The generic
power-block fixture demonstrates authoring behavior; it is not a claim that the private reference drawing was
reproduced or that every G-M2.5 gate passed.

### Independent review corrections and UI revision

The independent review rejected the initial implementation for a neutral-text edit crash, incompatible-grid test
placement, and acceptance of unknown electrical semantics. Corrections preserve neutral/multilingual text,
reject mismatched page grids before mutation, give scratch pages the draft grid, and block unknown active function,
discipline, potential-role, side and fluid-port semantics from publication/placement.
Project Save confirmations now save the project explicitly. Displayed command availability uses the dispatch
predicate; status follows the symbol document. Preview glyphs use the page's font/style shaping pipeline.
The editor uses one contextual command bar and Drawing / Electrical data / Library panels, labelled fields,
named variant controls, numeric keyboard creation and a discard confirmation. Function type, potential roles,
source and optional-pin flags are visible without JSON. Printing a designation does not set electrical meaning.
The correction set has targeted regressions and offscreen checks at 1280×800 and 1920×1080; independent
re-review is still pending. The owner's dependency-review release block remains in force; no exemption was added.

## 11. Orchestrator brief audit (2026-11-05)

**Not complete.** This audit compares the implementation with the original WP-S2.10 handoff, doc 07 §5 and
[G-M2.5](../03-gates.md#55a-g-m25-symbol-editor-release-020-m25). It does not replace their acceptance criteria
with the smaller synthetic fixture in §6. It is an implementation audit, not an independent acceptance review.

| Requirement | Current evidence and remaining work |
|---|---|
| Graphics, native-grid CPs, slots, anchors, link points, live A–H | Implemented through shared authoring actions and the production renderer. The initial anchor is GraphicCentre. Unit/action tests and offscreen examples exist; exact reference acceptance remains below. |
| Doc 07 §5 FnDef panel with CP table and drag assignment | **Partial:** numeric `fn_cp` mapping and an existing-function library-key field work. Drag assignment is not implemented. A browsable function picker would also improve the current key-entry workflow. |
| Doc 07 §5 fork from QET import | **Partial:** `.elmt` opening and the shared `symbol.fork` action work. There is no visible editor command to fork an existing imported project/library symbol; complete that GUI route and its FnDef assignment flow. |
| Scratch test placement and lint feedback | Implemented. The page uses the symbol's grid; placement on an incompatible existing page is refused. Page creation and placement are two undo operations. Editor validation is included; WP-S7.04 library lint remains out of scope. |
| Doc 16a appearance and small shape/function icons | The editor now reuses the main ribbon's small-button renderer, icons, text size, tooltips and active state. Its contextual layout still needs independent visual acceptance at both required sizes. |
| G-M2.5-01: exact reference power block, saved, placed, wired and reported | **Pending:** the six-pin synthetic fixture is not the specified reference reproduction. Complete the exact picture-based flow, wiring/report checks and preview comparison in kittest and a built-binary script. Keep private reference data out of the public repository. |
| G-M2.5-02: off-grid refusal at save, in words; fine graphics saved | Validator/publication tests cover refusal and fine graphics; action tests cover the full error wording. **Partial gate evidence:** complete the explicit kittest save attempt at 4.5 mm and successful 0.5 mm graphics save. |
| G-M2.5-03: picture-only blind personas | **Pending** on the release candidate, including severity fixes and persona reruns. Author-operated screenshots do not satisfy it. |
| G-M2.5-04: release pipeline, advisories/fuzz/hostile inputs/network policy | **Blocked/pending:** the owner's dependency-review block remains. Targeted tests are not the end-to-end release dossier. |
| G-M2.5-05: tutorial/reference, offline F1, release notes | Guide pages and SUMMARY entries exist. This revision adds editor/action F1 routing and release-note entries. **Pending:** verify the final release's offline bundle and CI documentation gate. |
| G-M2.5-06: formats and migration | No changes to the 0.1 project/library format; no migration is introduced. **Pending:** the release fixture corpus on the 0.2.0 candidate. |
| G-M2.5-07: still-applicable G-M2 and AC-24 | **Pending** on the 0.2.0 candidate; no full regression-gate claim. |
| Owner additions: design standard, AI authoring, QET/EPLAN import | Standard, schema/template/validate/publish and QET element import are implemented. EDZ/PXF and direct SLK have the explicit supported subsets above. Native binary macros and full EPLAN electrical/part semantics remain unsupported. |

Remaining build order, within the same feature branch: finish drag pin mapping and the visible import-fork
workflow; execute the exact reference and grid-save acceptance flows; obtain independent re-review and the
picture-only persona round; verify offline documentation and the release fixture/regression/pipeline evidence.
Dependency review and release promotion stay with their authorised owners. Do not describe the package as
complete or merge it into the working/release branches before those decisions and gates.

Proposed **FT-?**: implement the missing drag assignment and visible imported-symbol fork workflow, retaining
the shared validation and headless actions. These are unmet brief items, not an approved descope.
