# Interoperability: import, export and migration

**Status:** Draft v0.3 · **Date:** 2026-10-21 · **Part of:** ElektraWatt design document set

**Purpose.** This document specifies how ElektraWatt (EW) reads and writes foreign formats: the permissive
importer and exporter contracts, the QElectroTech (QET) migration path in detail, EPLAN part and report data, DXF
and DWG, catalogue formats, and the spreadsheet round trip with stable IDs. It summarises the STEP and PLC exchange
formats owned by other documents, states the legal limits on third-party content, and defines size limits, fuzzing
and test corpora per format. It elaborates the IMP and EXP rows of [02b](02b-requirements-extended.md) §7–§8
within the accepted [architecture](04-architecture.md) (§3, §5, §10.6; ADR-0004, ADR-0009, ADR-0021, ADR-0024).
Format facts come from [interop-formats](../research/interop-formats.md), as corrected by
[gaps-and-corrections](../research/gaps-and-corrections.md) (C-1 EDZ container, C-4 DWG route, C-14 corpus size,
C-15 `%f` versus `%F`).

**Where each section lives.** The section numbers are the ones the code and the other documents cite.

| Section | File |
|---|---|
| [§1. Scope and context](14-contracts.md#1-scope-and-context) | [scope, contracts and the import pipeline](14-contracts.md) |
| [§2. Contracts and data](14-contracts.md#2-contracts-and-data) | [scope, contracts and the import pipeline](14-contracts.md) |
| [§3. The import pipeline](14-contracts.md#3-the-import-pipeline) | [scope, contracts and the import pipeline](14-contracts.md) |
| [§4. QElectroTech importer](14-qet-model.md#4-qelectrotech-importer) | [QElectroTech: geometry, elements, conductors and folios](14-qet-model.md) |
| [§4.1 Versions](14-qet-model.md#41-versions) | [QElectroTech: geometry, elements, conductors and folios](14-qet-model.md) |
| [§4.2 Geometry on the native rational grid](14-qet-model.md#42-geometry-on-the-native-rational-grid) | [QElectroTech: geometry, elements, conductors and folios](14-qet-model.md) |
| [§4.3 Element definitions → symbols and function definitions](14-qet-model.md#43-element-definitions--symbols-and-function-definitions) | [QElectroTech: geometry, elements, conductors and folios](14-qet-model.md) |
| [§4.4 Element instances → devices, functions, representations](14-qet-model.md#44-element-instances--devices-functions-representations) | [QElectroTech: geometry, elements, conductors and folios](14-qet-model.md) |
| [§4.5 Master and slave → main and auxiliary functions](14-qet-model.md#45-master-and-slave--main-and-auxiliary-functions) | [QElectroTech: geometry, elements, conductors and folios](14-qet-model.md) |
| [§4.6 Folio reports → interruption points](14-qet-model.md#46-folio-reports--interruption-points) | [QElectroTech: geometry, elements, conductors and folios](14-qet-model.md) |
| [§4.7 Conductors → connections](14-qet-model.md#47-conductors--connections) | [QElectroTech: geometry, elements, conductors and folios](14-qet-model.md) |
| [§4.8 Formulas and cross-reference formats](14-qet-model.md#48-formulas-and-cross-reference-formats) | [QElectroTech: geometry, elements, conductors and folios](14-qet-model.md) |
| [§4.9 Folios, sections and title blocks](14-qet-model.md#49-folios-sections-and-title-blocks) | [QElectroTech: geometry, elements, conductors and folios](14-qet-model.md) |
| [§4.10 Fonts and text styles](14-qet-content.md#410-fonts-and-text-styles) | [QElectroTech: texts, terminals, tables, the verifier and the symbol map](14-qet-content.md) |
| [§4.11 Strips, terminals and cables](14-qet-content.md#411-strips-terminals-and-cables) | [QElectroTech: texts, terminals, tables, the verifier and the symbol map](14-qet-content.md) |
| [§4.12 Text to data](14-qet-content.md#412-text-to-data) | [QElectroTech: texts, terminals, tables, the verifier and the symbol map](14-qet-content.md) |
| [§4.13 Tables, shapes, images and layout pages](14-qet-content.md#413-tables-shapes-images-and-layout-pages) | [QElectroTech: texts, terminals, tables, the verifier and the symbol map](14-qet-content.md) |
| [§4.14 QETZ (1.x)](14-qet-content.md#414-qetz-1x) | [QElectroTech: texts, terminals, tables, the verifier and the symbol map](14-qet-content.md) |
| [§4.15 Migration verifier and acceptance](14-qet-content.md#415-migration-verifier-and-acceptance) | [QElectroTech: texts, terminals, tables, the verifier and the symbol map](14-qet-content.md) |
| [§4.16 The project importer](14-qet-content.md#416-the-project-importer) | [QElectroTech: texts, terminals, tables, the verifier and the symbol map](14-qet-content.md) |
| [§4.17 The symbol map: elements bound to functions](14-qet-content.md#417-the-symbol-map-elements-bound-to-functions) | [QElectroTech: texts, terminals, tables, the verifier and the symbol map](14-qet-content.md) |
| [§5. EPLAN data](14-eplan-dxf-step.md#5-eplan-data) | [EPLAN data, DXF and DWG, STEP and PLC exchange](14-eplan-dxf-step.md) |
| [§6. DXF and DWG](14-eplan-dxf-step.md#6-dxf-and-dwg) | [EPLAN data, DXF and DWG, STEP and PLC exchange](14-eplan-dxf-step.md) |
| [§7. STEP, PLC and catalogue exchange](14-eplan-dxf-step.md#7-step-plc-and-catalogue-exchange) | [EPLAN data, DXF and DWG, STEP and PLC exchange](14-eplan-dxf-step.md) |
| [§8. Tables: XLSX and CSV with stable IDs](14-tables-limits.md#8-tables-xlsx-and-csv-with-stable-ids) | [tables, exports, legal limits, hostile input and corpora](14-tables-limits.md) |
| [§9. Other exports](14-tables-limits.md#9-other-exports) | [tables, exports, legal limits, hostile input and corpora](14-tables-limits.md) |
| [§10. Legal constraints](14-tables-limits.md#10-legal-constraints) | [tables, exports, legal limits, hostile input and corpora](14-tables-limits.md) |
| [§11. Hostile input: limits and fuzzing](14-tables-limits.md#11-hostile-input-limits-and-fuzzing) | [tables, exports, legal limits, hostile input and corpora](14-tables-limits.md) |
| [§12. Test corpora](14-tables-limits.md#12-test-corpora) | [tables, exports, legal limits, hostile input and corpora](14-tables-limits.md) |
| [§13. UI and UX touchpoints](14-tables-limits.md#13-ui-and-ux-touchpoints) | [tables, exports, legal limits, hostile input and corpora](14-tables-limits.md) |
| [§14. Edge cases and pitfalls](14-tables-limits.md#14-edge-cases-and-pitfalls) | [tables, exports, legal limits, hostile input and corpora](14-tables-limits.md) |
| [§15. The reference project](14-tables-limits.md#15-the-reference-project) | [tables, exports, legal limits, hostile input and corpora](14-tables-limits.md) |
