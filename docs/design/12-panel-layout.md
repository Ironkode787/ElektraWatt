# Panel layout: 2D, 3D, routing, wire lengths and thermal

**Status:** Draft v0.1

**Purpose.** This document designs the physical side of an ElektraWatt (EW) project: layout spaces, enclosures and
mounting surfaces modelled at 1:1 in nanometres, rails and ducts as parametric carriers, placements bound to
schematic devices, 2D panel pages such as folio 902 as projections, the 3D scene with STEP models from the
out-of-process OCCT worker, collision and clearance checks, wire routing with duct fill, automatic wire lengths,
machining features, and the IEC 61439-1 / IEC TR 60890 thermal calculation. It elaborates the LAY and THM rows of
[02b](02b-requirements-extended.md) §4–§5 on the accepted [architecture](04-architecture.md) (ADR-0001, ADR-0003,
ADR-0016) and the entities of [domain model](03-domain-model.md) §16, and it states what the MVP, 1.0 and 1.x
releases contain.

**Where each section lives.** The section numbers are the ones the code and the other documents cite.

| Section | File |
|---|---|
| [§1. Scope and position](12-placement-pages.md#1-scope-and-position) | [placement, layout checks and 2D panel pages](12-placement-pages.md) |
| [§2. Concepts and data](12-placement-pages.md#2-concepts-and-data) | [placement, layout checks and 2D panel pages](12-placement-pages.md) |
| [§3. Placement behaviour and rules](12-placement-pages.md#3-placement-behaviour-and-rules) | [placement, layout checks and 2D panel pages](12-placement-pages.md) |
| [§4. Layout checks (EW-LAYOUT)](12-placement-pages.md#4-layout-checks-ew-layout) | [placement, layout checks and 2D panel pages](12-placement-pages.md) |
| [§5. 2D panel pages](12-placement-pages.md#5-2d-panel-pages) | [placement, layout checks and 2D panel pages](12-placement-pages.md) |
| [§6. 3D scene, STEP pipeline and panel macros](12-3d-routing-thermal.md#6-3d-scene-step-pipeline-and-panel-macros) | [3D, collision, routing, lengths, drilling and thermal](12-3d-routing-thermal.md) |
| [§7. Collision and clearance](12-3d-routing-thermal.md#7-collision-and-clearance) | [3D, collision, routing, lengths, drilling and thermal](12-3d-routing-thermal.md) |
| [§8. Routing](12-3d-routing-thermal.md#8-routing) | [3D, collision, routing, lengths, drilling and thermal](12-3d-routing-thermal.md) |
| [§9. Wire lengths](12-3d-routing-thermal.md#9-wire-lengths) | [3D, collision, routing, lengths, drilling and thermal](12-3d-routing-thermal.md) |
| [§10. Surface features, drilling and cut-outs](12-3d-routing-thermal.md#10-surface-features-drilling-and-cut-outs) | [3D, collision, routing, lengths, drilling and thermal](12-3d-routing-thermal.md) |
| [§11. Thermal calculation](12-3d-routing-thermal.md#11-thermal-calculation) | [3D, collision, routing, lengths, drilling and thermal](12-3d-routing-thermal.md) |
| [§12. UI and UX touchpoints](12-3d-routing-thermal.md#12-ui-and-ux-touchpoints) | [3D, collision, routing, lengths, drilling and thermal](12-3d-routing-thermal.md) |
| [§13. Performance budgets](12-3d-routing-thermal.md#13-performance-budgets) | [3D, collision, routing, lengths, drilling and thermal](12-3d-routing-thermal.md) |
| [§14. Outputs](12-3d-routing-thermal.md#14-outputs) | [3D, collision, routing, lengths, drilling and thermal](12-3d-routing-thermal.md) |
| [§15. Edge cases and pitfalls](12-3d-routing-thermal.md#15-edge-cases-and-pitfalls) | [3D, collision, routing, lengths, drilling and thermal](12-3d-routing-thermal.md) |
| [§16. The reference project](12-3d-routing-thermal.md#16-the-reference-project) | [3D, collision, routing, lengths, drilling and thermal](12-3d-routing-thermal.md) |

Conventions: ▲ marks a test value for an unknown of the
reference (U-nn) or a shop-policy default still to be confirmed by the pilot (PQ-22, PQ-24); "DT" is a
device tag; "CP" a connection point. Values marked *(unverified)* come from the research notes and ship as data,
never as hard-coded constants (FR-CHK-046). Source abbreviations: ARCH/arch =
[architecture](04-architecture.md); VIS = [vision](00-vision-and-scope.md); BS = [brainstorm](01-brainstorm.md);
PM = [panel-manufacturing](../research/panel-manufacturing.md); STD = [standards](../research/standards.md);
RD = [reference-drawing](../research/reference-drawing.md); EF = [eplan-features](../research/eplan-features.md);
PBF = [plc-bus-fluid](../research/plc-bus-fluid.md); C-nn and G-nn =
[gaps-and-corrections](../research/gaps-and-corrections.md).
