# Connections, terminals and cables

**Status:** Draft v0.1 · **Date:** 2026-10-23 · **Part of:** ElektraWatt design document set

**Purpose.** This document specifies how ElektraWatt (EW) turns drawn connections into electrical meaning. It covers
connection points, connections and junctions, the three connectivity levels (net, signal, potential) with stable
identities, potential types, flow and bonds, the wire-property inheritance chain with its colour and size profiles,
and wire numbering. It then specifies terminal strips and cables, the check rules of this area and the row sources
behind the terminal, cable, wire and potential reports. It elaborates the CON, TRM and CBL areas and the
wire-numbering part of NUM in [02a](02a-requirements-core.md). It builds on the records of the
[domain model](03-domain-model.md) §9, §11 and §12 and follows the accepted [architecture](04-architecture.md)
(ADR-0001, ADR-0006, ADR-0007).

**Notation.** W1 … W44, J1 … J6 and BR1 … BR6 are the physical wires, junctions and bridges of the reference
drawing set's project, the running example. ▲ and ▲U-nn mark test inputs that the reference set does not determine. EF-nn and MF-nn are expected findings. PQ-nn are questions for the pilot user ([brainstorm](01-brainstorm.md)).

**Where each section lives.** The section numbers are the ones the code and the other documents cite.

| Section | File |
|---|---|
| [§1. Scope and position](08-connectivity-wires.md#1-scope-and-position) | [connectivity and wire properties](08-connectivity-wires.md) |
| [§2. Concepts and data](08-connectivity-wires.md#2-concepts-and-data) | [connectivity and wire properties](08-connectivity-wires.md) |
| [§3. Connectivity semantics](08-connectivity-wires.md#3-connectivity-semantics) | [connectivity and wire properties](08-connectivity-wires.md) |
| [§4. Wire properties](08-connectivity-wires.md#4-wire-properties) | [connectivity and wire properties](08-connectivity-wires.md) |
| [§5. Wire numbering](08-numbering-terminals-cables.md#5-wire-numbering) | [wire numbering, terminals, cables and checks](08-numbering-terminals-cables.md) |
| [§6. Terminal strips and terminals](08-numbering-terminals-cables.md#6-terminal-strips-and-terminals) | [wire numbering, terminals, cables and checks](08-numbering-terminals-cables.md) |
| [§7. Cables and conductors](08-numbering-terminals-cables.md#7-cables-and-conductors) | [wire numbering, terminals, cables and checks](08-numbering-terminals-cables.md) |
| [§8. Check rules](08-numbering-terminals-cables.md#8-check-rules) | [wire numbering, terminals, cables and checks](08-numbering-terminals-cables.md) |
| [§9. UI and UX touchpoints](08-numbering-terminals-cables.md#9-ui-and-ux-touchpoints) | [wire numbering, terminals, cables and checks](08-numbering-terminals-cables.md) |
| [§10. Outputs](08-numbering-terminals-cables.md#10-outputs) | [wire numbering, terminals, cables and checks](08-numbering-terminals-cables.md) |
| [§11. Performance and determinism](08-numbering-terminals-cables.md#11-performance-and-determinism) | [wire numbering, terminals, cables and checks](08-numbering-terminals-cables.md) |
| [§12. Edge cases and pitfalls](08-numbering-terminals-cables.md#12-edge-cases-and-pitfalls) | [wire numbering, terminals, cables and checks](08-numbering-terminals-cables.md) |
| [§13. Defects prevented](08-numbering-terminals-cables.md#13-defects-prevented) | [wire numbering, terminals, cables and checks](08-numbering-terminals-cables.md) |
