# ADR-0007: Connection model: explicit connections and junction sequences; autoconnect as an editing aid

**Status:** Accepted, to be validated with two EPLAN users and one QET user before M2 · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the architecture judge panel)

## Context

Two connection models exist in the ECAD tools EW's users come from
([gaps and corrections](../research/gaps-and-corrections.md) G5):

| Model | Used by | Strengths | Weaknesses |
|---|---|---|---|
| **Autoconnect**: connections generated from aligned connection points and connection symbols (angles, T-nodes with a target direction) | EPLAN | Fast drawing of regular ladder diagrams; wiring order via the T-node direction | Moving a symbol can silently re-route logic; logic depends on geometry; complex to reimplement faithfully |
| **Explicit connections**: wire objects between terminals, with auto-routed or edited paths | QET, KiCad | Robust to moves; direct QET import; a simple anchor for wire IDs | Slower to draw ladders without assistance |

The reference project adds concrete needs:
- **T-junction sequences.** Folios 101–103 contain T-junctions whose connection sequence (which device terminal each
  wire goes to) matters for terminal plans and wire lists, for example the 24 V distribution onto the +JE2416-X1031
  terminals on folio 103 (REF-034).
- **Mixed junction notation.** Dots on folio 101 (F55 outputs) and 103, bevelled corners on 101, 102 and 103 that are
  side effects of overlapping conductors (D-20).
- **Stable connection identity.** Wire numbers (FR-NUM-019, FR-NUM-023), connection definition points carrying "1.5
  DBU" overrides (FR-CON-023), ferrules and machine export (Komax) all attach to a connection and must survive moves
  and renumbering.
- **Migration.** The pilot user's QET projects store explicit conductors; EPLAN-trained users expect autoconnect.

## Decision

1. **Explicit logical connections are the model.**
   `Connection { a: Endpoint, b: Endpoint, page, route: SmallVec<Pt2>, kind, overrides: WireOverrides, number: NumberSpec }`
   with `Endpoint = Cp { function, idx } | Junction(id)` and
   `kind = Wire | Core(Id<Conductor>) | Bridge | Fluid | Bus | Overview | Internal`.
2. **Junctions are entities:** `Junction { page, at, style: Dot | Bevel, sequence: SmallVec<Id<Connection>> }`. The
   sequence is the explicit T-node order; the style is a display choice, not a topology difference.
3. **Autoconnect is a normaliser, not semantics.** When a placement aligns CPs, the commit pipeline's normaliser
   (ADR-0008) **creates** explicit connection ops in the same transaction. The tool shows them as a ghost before the
   mouse is released. After commit they are ordinary connections.
4. **Moves never change connectivity** (FR-CON-004). Routes follow their endpoints; a reconnect is an explicit
   command.
5. **Insertion onto a wire** splits the connection and keeps its properties on both parts (FR-CON-008).
6. **Overview connections are excluded from nets.** Potentials and nets are derived from wire, core, bridge and
   internal connections, interruption-point pairs, terminal internal links and plug-in jumpers (FR-CON-009).
   Bonds link potentials without merging them (D-22).
7. **Several connections on one CP** keep a defined order (daisy-chaining, FR-CON-006): the junction sequence where
   a junction exists, otherwise ascending connection ID.

## Consequences

### Positive

- Moving a symbol never silently rewires logic.
- QET conductors map one to one onto connections (FR-IMP-012).
- Wire IDs are stable for numbering, ferrules and machine export.
- Junction order is data, so terminal plans and wire lists are deterministic.
- Live highlighting during a drag can use page-local nets, because topology does not change while dragging.

### Negative

- Drawing regular ladders depends on good autoconnect UX: the ghost preview, autoconnect on placement, and an
  explicit "connect aligned points" command for moves.
- EW semantics differ from EPLAN's in edge cases (a symbol dropped onto an existing autoconnect line, T-node
  direction). The differences must be documented in the EPLAN ↔ EW glossary (NFR-DOC-05).
- EPLAN macros and future EPLAN imports that rely on autoconnect need a conversion pass into explicit connections.
- Each connection stores its route, which costs bytes per wire compared with geometry-derived lines.

## Validation before M2

The moderated usability test of NFR-UX-01 (two EPLAN users, one QET user) must answer:
1. Do the EPLAN users complete the core tasks unassisted (≥ 80 %, NFR-UX-01), and does the QET user draw the
   folio 102 circuit (-102G1, -102F1, -102F2 with wires) and export a PDF within 30 minutes (NFR-UX-04)?
2. **Autoconnect on move:** should moving a symbol onto aligned CPs propose connections by default, or only with a
   modifier? The schematic editor currently proposes on placement by default and on move only with `C`
   ([schematic editor](../design/06-schematic-editor.md), open question 5).
3. Do the EW signal and potential boundaries match what EPLAN users expect
   ([domain model](../design/03-domain-model.md), open question 2)?

If the test fails, the remedy is better autoconnect UX (defaults, modifiers, a re-autoconnect command), not
autoconnect as model semantics. Changing the model would need a new ADR that supersedes this one.

## Pending amendments

These refinements come from the [domain model](../design/03-domain-model.md) (§9 and open questions 3 and 7). They
do not change the decision above.
1. **Optional `Connection.page`.** Bridges defined in the strip editor and internal links (channel to coil) are not
   drawn. *Recommendation:* make `page` an `Option`; such rows live in the shard of the device that owns endpoint
   `a` ([project format](../design/05-project-format-and-persistence.md) §2.2).
2. **Junction wiring.** The domain model refines the junction to `style: Dot | Bevel{orient}` plus
   `wiring: Chain | Star`. *Recommendation:* adopt both, with Dot junctions defaulting to Star from the first
   sequence member and Bevel junctions to Chain (the golden reading of folio 101); confirm with the pilot user and
   with QET imports, where only geometry is available.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **Autoconnect as model semantics** (EPLAN) | Logic depends on geometry; moves silently rewire; a faithful reimplementation is costly; wire IDs would be unstable |
| **Explicit storage plus automatic re-autoconnect after every move** | Still rewires silently; offered only as an explicit command or modifier |
| **Connections without junction entities** (order implied by geometry) | Cannot express the T-node order of REF-034 or normalise the mixed notation of D-20 |

## Related

- **ADRs:** [ADR-0001](0001-core-model-persistent-maps.md), [ADR-0004](0004-identity-uuidv7-uuidv5.md),
  [ADR-0006](0006-function-device-representation-model.md), [ADR-0008](0008-transactions-undo-preview.md), [ADR-0021](0021-importers-emit-format-level-records.md).
- **Design:** [architecture](../design/04-architecture.md) §4.2, §5.2, §16 (risk 12);
  [schematic editor](../design/06-schematic-editor.md); [connections, terminals and
  cables](../design/08-connections-terminals-cables.md) §2–§3; [UX/UI](../design/16-ux-ui.md) §19.
- **Requirements:** FR-CON-001, FR-CON-002, FR-CON-003, FR-CON-004, FR-CON-005, FR-CON-006, FR-CON-007,
  FR-CON-008, FR-CON-009, FR-CON-023, FR-NUM-019, FR-NUM-023, FR-UI-016, FR-UI-017, FR-IMP-012, NFR-UX-01,
  NFR-UX-04, NFR-DOC-05.
