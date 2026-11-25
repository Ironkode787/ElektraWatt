# Checks, revisions and collaboration

**Status:** Draft v0.7 · **Date:** 2026-10-25 · **Part of:** ElektraWatt design document set

> **Anonymization note:** dates and the manufacturing year of the reference project in this document are synthetic placeholders, not the original values.

**Purpose.** This document specifies how ElektraWatt (EW) finds errors, releases a checked drawing set and lets
several people work on one project without a server. It elaborates the CHK and REV areas of
[02a](02a-requirements-core.md) and the workflow part of COL in [02b](02b-requirements-extended.md). It covers the
rule engine and its modes, rule packs and schemes, waivers, quick fixes and messages, the one rule-ID scheme and the
**complete rule catalogue** (every rule any document names, one row per ID), managed revisions with a four-eyes
release, and the shared-drive and git workflows with semantic merge and visual page review. Storage mechanics (save
protocol, lock record, merge-driver internals, archive layout) stay in [05](05-project-format-and-persistence.md).

**Where each section lives.** The section numbers are the ones the code and the other documents cite.

| Section | File |
|---|---|
| [§1. Scope and boundaries](15-rule-engine.md#1-scope-and-boundaries) | [scope, rules and the rule engine](15-rule-engine.md) |
| [§2. Concepts and data](15-rule-engine.md#2-concepts-and-data) | [scope, rules and the rule engine](15-rule-engine.md) |
| [§3. Rule engine behaviour](15-rule-engine.md#3-rule-engine-behaviour) | [scope, rules and the rule engine](15-rule-engine.md) |
| [§4. Rule catalogue](15-rule-catalogue.md#4-rule-catalogue) | [the rule catalogue](15-rule-catalogue.md) |
| [§5. Revision management](15-revisions-collaboration.md#5-revision-management) | [revisions and collaboration](15-revisions-collaboration.md) |
| [§6. Collaboration](15-revisions-collaboration.md#6-collaboration) | [revisions and collaboration](15-revisions-collaboration.md) |
| [§7. UI and UX touchpoints](15-revisions-collaboration.md#7-ui-and-ux-touchpoints) | [revisions and collaboration](15-revisions-collaboration.md) |
| [§8. Outputs](15-revisions-collaboration.md#8-outputs) | [revisions and collaboration](15-revisions-collaboration.md) |
| [§9. Edge cases and pitfalls](15-revisions-collaboration.md#9-edge-cases-and-pitfalls) | [revisions and collaboration](15-revisions-collaboration.md) |
| [§10. The named findings](15-revisions-collaboration.md#10-the-named-findings) | [revisions and collaboration](15-revisions-collaboration.md) |
