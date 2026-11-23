# Reports, forms and page output

**Status:** Draft v0.1 · **Date:** 2026-10-18 · **Part of:** ElektraWatt design document set

> **Anonymization note:** dates and the manufacturing year of the reference project in this document are synthetic placeholders, not the original values.

**Purpose.** This document specifies how ElektraWatt (EW) turns the model into documents. It covers the built-in
report kinds and user SQL reports, the declarative forms engine (report forms, frames, title blocks, cover pages),
report blocks whose pages are derived and join the page order, and the single display-list pipeline that feeds
PDF/A-2b, SVG, PNG, DXF and printing, plus XLSX/CSV tables from the same row source. It elaborates the REP and OUT
areas of [02a](02a-requirements-core.md) and the output side of I18N in [02b](02b-requirements-extended.md), within
the accepted [architecture](04-architecture.md) (§8, §10.2, §11; ADR-0015, ADR-0017, ADR-0018).

**Where each section lives.** The section numbers are the ones the code and the other documents cite.

| Section | File |
|---|---|
| [§1. Scope and context](09-reports-forms.md#1-scope-and-context) | [report blocks, rows, forms and pagination](09-reports-forms.md) |
| [§2. Concepts and data](09-reports-forms.md#2-concepts-and-data) | [report blocks, rows, forms and pagination](09-reports-forms.md) |
| [§3. Behaviour and rules](09-reports-forms.md#3-behaviour-and-rules) | [report blocks, rows, forms and pagination](09-reports-forms.md) |
| [§4. Page output pipeline](09-output.md#4-page-output-pipeline) | [the page output pipeline and exports](09-output.md) |
| [§5. UI and UX touchpoints](09-output.md#5-ui-and-ux-touchpoints) | [the page output pipeline and exports](09-output.md) |
| [§6. Outputs](09-output.md#6-outputs) | [the page output pipeline and exports](09-output.md) |
| [§7. Edge cases and pitfalls](09-output.md#7-edge-cases-and-pitfalls) | [the page output pipeline and exports](09-output.md) |
| [§8. The reference project](09-output.md#8-the-reference-project) | [the page output pipeline and exports](09-output.md) |
