# ADR-0006: Function / Device / Representation model with typed device aspects

**Status:** Accepted · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the architecture judge panel)

## Context

EPLAN distinguishes three things that QET conflates in one element: the **device** (a physical item with a part,
e.g. MCB -101F1 with Schneider A9P52610), its **functions** (main and auxiliary: a coil and its contacts, a
protective device and its signal contact) and their **representations** (multi-line, overview, fluid, bus topology,
panel front). The reference drawing set needs all three:

| Need | Reference evidence |
|---|---|
| One device drawn on several pages, linked by xrefs | -201U1 on folios 3, 201 and 106 with "(201-E1)"; module details referenced as "∞ 203-B4" |
| One device with several roles at once | -201U1 (Festo CPX FB13) is a PLC station, a Profibus node and a pneumatic assembly, with 7/8" power, DP and pneumatic connection points in one device (REF-098, folios 3, 106, 201–206) |
| An overview generated from the assembly | Folio 202 shows 5 valves on -202U2, the detail page and BOM 4 (D-10) |
| One strip drawn in several segments | X1031 drawn twice on folio 103, its BOM row duplicated (D-42) |
| One assembly identity | The manifold tagged from two pages, -201U1 … U9 and -202U1 … U3 (D-41); 28 valves without tags (D-27) |
| Layout bound to devices | Folio 902's T51 versus the schematic's T55 (D-02) |
| A complete BOM from one path | Missing -106W2, -201U4 … U9 and the 571334 valves; X1031 listed twice (D-04) |

Proposal A as written modelled terminals as functions that carry parts, which creates a second BOM path. Device
subclasses cannot express a device that is a PLC station, a bus node and an assembly at the same time.

## Decision

1. **Device.**
   `Device { tag: TagSpec, kind, parent: Option<Id<Device>>, parts: SmallVec<PartUse{part, qty}>, function_text: MlText, process_tag, tech_data, state, aspects: DeviceAspects, extra }`.
   **Parts attach only to devices**, with several part uses per device.
2. **Typed aspects instead of subtypes:**
   ```rust
   pub struct DeviceAspects {
       pub strip: Option<Strip>,            pub terminal: Option<Terminal>,
       pub cable: Option<Cable>,            pub assembly: Option<Assembly>,   // config code, BOM basis (D-36)
       pub plc_station: Option<PlcStation>, pub plc_module: Option<PlcModule>,
       pub bus_node: Option<BusNode>,       pub valve_terminal: Option<ValveTerminal>,
   }
   ```
3. **Function.** `Function { device, def: FnDefRef, role: Main | Aux, cp_overrides }`. Its connection points are
   instantiated from the function definition or the part's pin pattern (IDs per ADR-0004). Each device has exactly
   one main function (structural invariant, ADR-0009). Interruption points are functions of kind `Ip`.
4. **Representation.** `Representation { functions, page, symbol, variant, pos, orient, rep_type, pin_map, text_slots }`
   draws one or more functions on one page. Types: multi-line, overview, fluid, bus topology, panel front and strip
   segment; the [domain model](../design/03-domain-model.md) §6 adds single-line, contact image and graphic
   (FR-XRF-016). **An unplaced function has no representation**; "unplaced" is never stored.
5. **Terminals are devices**, as in EPLAN: a strip is a device with aspect `Strip`, a terminal a device with aspect
   `Terminal { strip, seq, designation, levels }`. Levels and clamps are CPs carrying level and side, which covers
   PT 2.5-TWIN terminals.
6. **PLC channels** are entities `Channel { module, number, kind, data_type, address, state, symbol, comment, binding: Option<Endpoint> }`
   bound to function endpoints. **Layout placements** reference a device and optionally one function
   (`Placement3D { device, function, carrier, pose, locked }`) and store no tag text.

**Worked example: the valve terminal on folios 201–206.**
- -201U1 is one device carrying `PlcStation` (role valve terminal) + `BusNode` + `Assembly` (order codes
  51E-F13GCQP-D+H2GG and 32P-VGL-R-…) + `ValveTerminal` (pilot supply; [fluid power](../design/11-fluid-power.md),
  [PLC and bus](../design/10-plc-and-bus.md)).
- The sub-bases -201U3 … -201U8 are child devices carrying `PlcModule`.
- Each valve is a child device of its sub-base, with a coil function bound to a `Channel` (Q0.1 …) and a pneumatic
  function whose CPs are ports 1/2/3/4/5.
- The overview on folios 201 and 202 is a representation generated from the assembly's children in physical order.

## Consequences

### Positive

- The BOM aggregates a single path, `bom_rows(mode)` over parts on devices, assembly children and carriers. One strip
  with several segments is one row with a quantity (D-04, D-42), in assembly or component mode (D-36).
- The overview cannot disagree with the details, because it is generated from the assembly's children (D-10).
- Child devices get derived tags such as -201U3-QM1 from their parent (D-27, D-41).
- Layout labels come from the device a placement references (D-02).
- Xrefs between representations of one device (main/aux, overview/detail) are relations, not texts (FR-XRF-015,
  FR-XRF-016).
- The valve terminal's dual nature is explicit, so the PLC, bus and fluid views show one identity.

### Negative

- The aspect set is fixed in `ew-schema`. A new built-in role is a schema change (a format minor, ADR-0011). Plugin
  roles go into namespaced `plugin.<id>.*` data.
- Some aspect combinations are meaningless (terminal plus cable). The invariant catalogue needs
  aspect-compatibility rules.
- Treating terminals as devices makes up to 20,000 of the 50,000 devices of the XL class terminals
  ([non-functional requirements](../design/02c-nonfunctional-requirements.md) §2.1). The device table and its
  shards grow accordingly.
- QET users must learn the device/function split; the QET importer maps master/slave elements to main and auxiliary
  functions (FR-IMP-009).

## Pending amendments

These refinements come from the [domain model](../design/03-domain-model.md) (§7 and open questions 5 and 6). They
do not change the decision above.
1. **Interruption-point functions without a device.** An interruption point such as 24VDC-1 belongs to no physical
   device. *Recommendation:* make `Function.device` optional, with the structural invariant "None if and only if
   the kind is `Ip`", and add `kind: Normal | Ip | BlackBox` to `Function`, as the domain model §7 does.
2. **Where the valve-terminal `Assembly` aspect lives.** *Recommendation:* keep it on -201U1 for the golden
   project, with `AssemblyConfig.covers` mapping the order codes to the children, and allow a part-less assembly
   device for valve terminals without an integrated bus node ([PLC and bus](../design/10-plc-and-bus.md)).

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **Device subclasses** (one Rust type per kind) | Cannot combine roles; the valve terminal would need three linked entities whose tags can drift (the D-41 class) |
| **Terminals as functions carrying parts** (proposal A as written) | Two BOM paths, two places for part data |
| **EAV property bags for built-in data** | No type checking; the schema-driven property grid and the JSON Schemas lose types; slower queries |
| **ECS components** (proposal B) | Rejected with the substrate in ADR-0001 |

## Related

- **ADRs:** [ADR-0001](0001-core-model-persistent-maps.md), [ADR-0004](0004-identity-uuidv7-uuidv5.md),
  [ADR-0007](0007-explicit-connections-and-autoconnect.md), [ADR-0009](0009-invariant-catalogue.md), [ADR-0018](0018-reports-forms-engine-and-machine-profiles.md).
- **Design:** [architecture](../design/04-architecture.md) §4.2, §4.3; [domain model](../design/03-domain-model.md)
  §7, §11–§13; [connections, terminals and cables](../design/08-connections-terminals-cables.md) §6;
  [PLC and bus](../design/10-plc-and-bus.md); [panel layout](../design/12-panel-layout.md).
- **Requirements:** FR-DEV-001, FR-DEV-002, FR-DEV-003, FR-DEV-004, FR-DEV-005, FR-DEV-015, FR-DEV-019,
  FR-DEV-020, FR-DEV-021, FR-DEV-022, FR-DEV-023, FR-DEV-024, FR-NUM-013, FR-TRM-001, FR-TRM-004, FR-TRM-011,
  FR-PLC-004, FR-LAY-006, FR-REP-005, FR-REP-006, FR-XRF-015, FR-XRF-016, FR-IMP-009.
