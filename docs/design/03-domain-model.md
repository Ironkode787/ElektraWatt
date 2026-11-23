# Domain model

**Status:** Draft v0.1 · **Date:** 2026-10-21 · **Part of:** ElektraWatt design document set

> **Anonymization note:** dates and the manufacturing year of the reference project in this document are synthetic placeholders, not the original values.

**Purpose.** This document defines the conceptual domain model shared by every ElektraWatt (EW) design document.
For each entity of an electrical control-cabinet project it gives the purpose, the key attributes with types and
units, the relationships, identity and lifecycle, the invariants, whether the concept is authored (stored, changed
only by commands) or derived (computed by a query, never stored), and its EPLAN Electric P8 and QElectroTech (QET)
equivalent. It refines §4 of the accepted [architecture](04-architecture.md) without changing its decisions: record
names are those of `ew-schema`, and every addition is marked (added). The reference drawing set (cabinet
J-E-2416) is the running example.

**Where each section lives.** The section numbers are the ones the code and the other documents cite.

| Section | File |
|---|---|
| [§1. Conventions](03-foundations.md#1-conventions) | [conventions, overview, values and identity](03-foundations.md) |
| [§2. Model overview](03-foundations.md#2-model-overview) | [conventions, overview, values and identity](03-foundations.md) |
| [§3. Value types: units, quantities, text](03-foundations.md#3-value-types-units-quantities-text) | [conventions, overview, values and identity](03-foundations.md) |
| [§4. Identity, lifecycle and invariants](03-foundations.md#4-identity-lifecycle-and-invariants) | [conventions, overview, values and identity](03-foundations.md) |
| [§5. Project and structure](03-project-pages.md#5-project-and-structure) | [project, structure and pages](03-project-pages.md) |
| [§6. Pages, frames and representations](03-project-pages.md#6-pages-frames-and-representations) | [project, structure and pages](03-project-pages.md) |
| [§7. Functions, devices and parts](03-devices-tags.md#7-functions-devices-and-parts) | [functions, devices, parts, tags and numbering](03-devices-tags.md) |
| [§8. Tags and numbering](03-devices-tags.md#8-tags-and-numbering) | [functions, devices, parts, tags and numbering](03-devices-tags.md) |
| [§9. Connectivity: connections, nets, signals, potentials](03-connectivity.md#9-connectivity-connections-nets-signals-potentials) | [connectivity, interruption points, terminals and cables](03-connectivity.md) |
| [§10. Interruption points and cross-references](03-connectivity.md#10-interruption-points-and-cross-references) | [connectivity, interruption points, terminals and cables](03-connectivity.md) |
| [§11. Terminals](03-connectivity.md#11-terminals) | [connectivity, interruption points, terminals and cables](03-connectivity.md) |
| [§12. Cables](03-connectivity.md#12-cables) | [connectivity, interruption points, terminals and cables](03-connectivity.md) |
| [§13. PLC](03-areas.md#13-plc) | [PLC, bus, fluid, layout, manufacturing, revisions and checks](03-areas.md) |
| [§14. Bus](03-areas.md#14-bus) | [PLC, bus, fluid, layout, manufacturing, revisions and checks](03-areas.md) |
| [§15. Fluid power](03-areas.md#15-fluid-power) | [PLC, bus, fluid, layout, manufacturing, revisions and checks](03-areas.md) |
| [§16. Panel layout and routing](03-areas.md#16-panel-layout-and-routing) | [PLC, bus, fluid, layout, manufacturing, revisions and checks](03-areas.md) |
| [§17. Manufacturing](03-areas.md#17-manufacturing) | [PLC, bus, fluid, layout, manufacturing, revisions and checks](03-areas.md) |
| [§18. Revisions and release](03-areas.md#18-revisions-and-release) | [PLC, bus, fluid, layout, manufacturing, revisions and checks](03-areas.md) |
| [§19. Checks](03-areas.md#19-checks) | [PLC, bus, fluid, layout, manufacturing, revisions and checks](03-areas.md) |
| [§20. The reference drawing and the sample project](03-areas.md#20-the-reference-drawing-and-the-sample-project) | [PLC, bus, fluid, layout, manufacturing, revisions and checks](03-areas.md) |
