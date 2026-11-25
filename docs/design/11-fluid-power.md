# Fluid power

**Status:** Draft v0.1 · **Date:** 2026-10-18 · **Part of:** ElektraWatt design document set

**Purpose.** This document specifies how ElektraWatt (EW) represents and checks fluid power: pneumatics in v1 and
hydraulics later. Fluid power runs on the same connection graph as the electrical schematic. It uses typed ports, valve
switching models and media that propagate the way potentials do. The document covers devices that are fluid and
electrical at once: solenoid valves, and valve terminals that are also PLC stations. It also covers the generated
valve-terminal overview pages, process tags and target tracking, tubes and their reports, the QET pneumatic import
mapping, and the EW-FL rule pack. It elaborates the FLU area of [02b](02b-requirements-extended.md). It builds on
[domain model](03-domain-model.md) §7 and §15, and follows the accepted [architecture](04-architecture.md)
(ADR-0001, ADR-0006, ADR-0007).

**Where each section lives.** The section numbers are the ones the code and the other documents cite.

| Section | File |
|---|---|
| [§1. Scope and position](11-concepts-rules.md#1-scope-and-position) | [concepts, fluid and electrical devices, rules](11-concepts-rules.md) |
| [§2. Concepts and data](11-concepts-rules.md#2-concepts-and-data) | [concepts, fluid and electrical devices, rules](11-concepts-rules.md) |
| [§3. Devices that are fluid and electrical](11-concepts-rules.md#3-devices-that-are-fluid-and-electrical) | [concepts, fluid and electrical devices, rules](11-concepts-rules.md) |
| [§4. Behaviour and rules](11-concepts-rules.md#4-behaviour-and-rules) | [concepts, fluid and electrical devices, rules](11-concepts-rules.md) |
| [§5. Algorithms](11-algorithms-outputs.md#5-algorithms) | [algorithms, checks, the window, outputs and import](11-algorithms-outputs.md) |
| [§6. Check rules: the EW-FL pack](11-algorithms-outputs.md#6-check-rules-the-ew-fl-pack) | [algorithms, checks, the window, outputs and import](11-algorithms-outputs.md) |
| [§7. UI and UX touchpoints](11-algorithms-outputs.md#7-ui-and-ux-touchpoints) | [algorithms, checks, the window, outputs and import](11-algorithms-outputs.md) |
| [§8. Outputs](11-algorithms-outputs.md#8-outputs) | [algorithms, checks, the window, outputs and import](11-algorithms-outputs.md) |
| [§9. QET pneumatic import](11-algorithms-outputs.md#9-qet-pneumatic-import) | [algorithms, checks, the window, outputs and import](11-algorithms-outputs.md) |
| [§10. Edge cases and pitfalls](11-algorithms-outputs.md#10-edge-cases-and-pitfalls) | [algorithms, checks, the window, outputs and import](11-algorithms-outputs.md) |
| [§11. The valve terminal as data](11-algorithms-outputs.md#11-the-valve-terminal-as-data) | [algorithms, checks, the window, outputs and import](11-algorithms-outputs.md) |
| [§12. Scope by release](11-algorithms-outputs.md#12-scope-by-release) | [algorithms, checks, the window, outputs and import](11-algorithms-outputs.md) |
