# Symbols, parts and macros

**Status:** Draft v0.1 · **Date:** 2026-10-21 · **Part of:** ElektraWatt design document set

**Purpose.** This document specifies ElektraWatt's (EW) master data: what a part is and the one place where a
person meets it, symbols and function definitions (FnDefs), macros with value sets, basic projects (company
templates), and the libraries and governance that hold them. It refines the [architecture](04-architecture.md)
(§4.2 library copies, §5.5 macros, §7.7 catalogues) and the [domain model](03-domain-model.md) (§6 `SymbolDef`, §7
`FnDef`, `PartCopy`, `MacroDef`), and elaborates the SYM, PRT and MAC rows of [02a](02a-requirements-core.md). Its
examples are the sample project's (`fixtures/sample/`) and invented parts of a maker called Example.

How a symbol is drawn is specified in [07a: Symbol design standard](07a-symbol-design-standard.md).

**Where each section lives.** The section numbers are the ones the code and the other documents cite.

| Section | File |
|---|---|
| [§1. Scope and position in the architecture](07-concepts.md#1-scope-and-position-in-the-architecture) | [symbols, function definitions, parts and macros as data](07-concepts.md) |
| [§2. Concepts and data](07-concepts.md#2-concepts-and-data) | [symbols, function definitions, parts and macros as data](07-concepts.md) |
| [§3. Behaviour and rules](07-behaviour-libraries.md#3-behaviour-and-rules) | [behaviour, algorithms and library governance](07-behaviour-libraries.md) |
| [§4. Algorithms](07-behaviour-libraries.md#4-algorithms) | [behaviour, algorithms and library governance](07-behaviour-libraries.md) |
| [§5. UI and UX touchpoints](07-place.md#5-ui-and-ux-touchpoints) | [Parts, the place for parts and symbols, and the other windows of master data](07-place.md) |
| [§6. Library governance and the community library](07-behaviour-libraries.md#6-library-governance-and-the-community-library) | [behaviour, algorithms and library governance](07-behaviour-libraries.md) |
| [§7. Outputs](07-behaviour-libraries.md#7-outputs) | [behaviour, algorithms and library governance](07-behaviour-libraries.md) |
| [§8. Edge cases and pitfalls](07-behaviour-libraries.md#8-edge-cases-and-pitfalls) | [behaviour, algorithms and library governance](07-behaviour-libraries.md) |
| [§9. The starter set](07-behaviour-libraries.md#9-the-starter-set) | [behaviour, algorithms and library governance](07-behaviour-libraries.md) |
