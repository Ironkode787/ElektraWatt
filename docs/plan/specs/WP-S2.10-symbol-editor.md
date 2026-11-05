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
| D7 | Save through the library writer, preserving its lock, writability, semantic-version and atomic-generation rules. New symbols have their own function definition; assigning an existing definition retains its semantics. Test placement uses the existing placement command with embedded copies and one undo transaction. |
| D8 | QET `.elmt` uses the existing bounded reader and keeps the QET grid, origin, unsupported-data report and source. A native-grid conversion is not implicit. |
| D9 | EPLAN scope includes EDZ and `.slk/.ema/.ems` as requested. Verify against manufacturer downloads kept outside the repository. Record supported input versions and feature losses. Binary macro/symbol decoding requires evidence; a factual-data fallback must be explicitly identified as a generated symbol, with original graphics unavailable. |
| D10 | UI follows doc 16a: drawing leads, task-ordered labelled tools, properties beside it, validation in words, all identifiers readable. Inspect at 1920×1080 and 1280×800. New text lives in en-GB and nl-NL. |

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

## 10. Implementation and review evidence

- `ew-api::symbols`: portable draft, common validation, schema, QET/EPLAN adapters, atomic new-file export,
  embedded/library forks, assigned-function consistency, canonical library publication. No format 0.1 changes.
- `ew-viewmodel::shell::symbol_editor`: independent bounded history, all editing actions, import candidate selection,
  function assignment, publication, scratch-page creation and placement using the destination page's insertion grid.
- `ew-ui::symbol_editor`: canvas tools, graphics selection/movement, numeric properties, pin mapping/direction,
  slots, anchors, linkage points, live A–H, import review and advanced JSON. Graphics use the production CPU renderer.
- `ew symbol template|schema|validate|import|publish`: data-only AI authoring, independent of an open project.
- `ew-import-eplan`: bounded 7z/ZIP and PXF 1.23 drawing subset, plus clearly marked generated parts outlines.
  Verified locally with the manufacturer's P30O EDZ download; source files remain private. The preserved source
  identity is a content hash, not the source's embedded workstation/user metadata.
- Focused acceptance: 18 tests passed (API authoring, GUI/headless shared actions, archive/XML boundaries,
  QET provenance/grid and the opt-in external EDZ sample). Larger lane-check and visual evidence are recorded
  in the hand-back after the final run. Performance: not measured (owner's rule).

### Exact limitations and remaining gates

Binary `.slk` and binary macros are refused. No native SLK sample or decoder is claimed. PXF supports lines,
rectangles, polylines, full circles and basic text; incomplete records, style changes and unresolved function
connections are reported. `.esl` is accepted only if it contains the same supported PXF macro structure;
symbol-library export structures are not verified. This is a partial fulfilment of the requested native-library
scope, not a full EPLAN compatibility claim. Imported electrical meaning still requires author review.

Proposed **FT-?**: implement verified native SLK/symbol-library export decoding and the remaining PXF primitives,
transforms and library-resolved connection positions, with redistributable synthetic fixtures.
Proposed **ID-?**: decide the five pinned decoder dependencies (`sevenz-turbo` 0.26.0, `lzma-turbo` 0.6.0,
`crc-fast` 1.10.0, `spin` 0.10.1, `zeroize` 1.9.1). `cargo-vet` has no approvals for them, and `cargo-deny` rejects
`lzma-turbo`'s GPL-3.0-or-later licence under the current external-dependency allowlist. No exemptions or licence
policy changes have been made. The GPL application graph uses Rust 1.97.1 for this importer; the workspace's
pinned compiler is 1.99.0 and the permissive crates' MSRV is unchanged. These checks block release approval.

The independent G-M2.5 blind-persona review and release promotion remain with the orchestrator. The generic
power-block fixture demonstrates authoring behavior; it is not a claim that the private reference drawing was
reproduced or that every G-M2.5 gate passed.
