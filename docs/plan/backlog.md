# Backlog

Open work, one row each. A finished row is deleted; a new one is added under its milestone. The numbers are the
old follow-up numbers, kept so that a row's history can be found in git (`5516edba4`, `docs/plan/follow-ups.md`
and the work breakdown's Appendix B).

## M3

Rows that a work package of [m3.md](m3.md) takes. The lane checks each row first: a row that the code already
answers is deleted with a line in its report.

### WP-S1.24: PLC and bus 1.0

- FT-1724 `ew-schema, ew-derive`: The signal map of a bus port's part data (FR-BUS-023, Should): bus signals (A, B, DGND, VP, shield; TD±, RD±) to connector pins, so that a multi-line port representation resolves per-core connections and the core colours of the bus cable. WP-S1.24's first hand-back left it out.
- FT-1725 `ew-derive, ew-scene`: Target tracking's option "show the connected PLC address at terminals" (FR-PLC-018, doc 10 §3.5). WP-S1.24's first hand-back left it out.
- FT-323 `WP-S1.21`: The part-data confidence `ItemMeta.confidence` (verified, unverified, test) of the PLC and bus sections (doc 10 §2.6; part 533361's map is `test`, ▲U-23) and the "based on non-verified part data" …
- FT-80 `WP-S1.13`: The `DeviceLink` record kind (format change, FR-DEV-028) and with it the used-valve process-tag fallback of ID-167.

### WP-S1.25: fluid 1.0

- FT-1326 `WP-S7.03`: Starter parts for a generic valve-terminal module and valve, so that a new project draws a valve terminal without Golden A's parts (WP-S1.20 FP-1).
- FT-1569 `ew-api`: A project whose fluid function definition's copy is damaged and held by no library probably refuses every other copy's update as a misfit (read, not run); reproduce, and judge the misfit without the …
- FT-1576 `ew-commands`: `valve.add` with a part that has a PLC-module section adds a device that normaliser N0 attaches to the project's only station (WP-S1.21 D4) instead of the valve module; rule whether `valve.add` …
- FT-740 `ew-ui, ew-viewmodel, content/`: **Medium.** Golden A's valve coils have no drawn internal CPs, so FR-PLC-020's acceptance (channel 1 of -201U3 dragged onto -201U3-Q1's coil on folio 203) cannot be done by canvas pick; PG2 was tested on a …

### The main session

- FT-1728 `ew-checks`: The rules of release 1.0 that M3 builds (EW-PLC, EW-SAF, EW-PRC, EW-BUS, EW-IOL, EW-FL) run only in a scheme with the 1.0 content, and every shipped scheme runs the MVP content, so no user sees them. Shipped schemes run what is built; Golden A's expected findings become doc 20 §18.1 plus §18.2.

### WP-S2.13: automation workspaces

- FT-727 `ew-viewmodel, ew-ui`: EW-PLC-012's second quick fix, "open the part's PLC section" (doc 10 §3.10): the window opens the part copy's PLC section through the existing `part.edit_copy`; only "rebind to the map's channel" is offered today.
- FT-1726 `ew-viewmodel`: Output ▸ Reports ▸ Report block… offers a list kept by hand (`REPORT_KINDS` in `shell/creation.rs`), so the six PLC and bus lists of WP-S1.24 and the fluid lists of WP-S1.25 cannot be added from the window; offer every page-oriented report kind that has an engine, read from the kinds themselves.
- FT-738 `ew-api`: The PLC navigator's `safety` and `bus:<name>` facets (WP-S2.07 §8).
- FT-774 `ew-api, ew-commands`: Attach and bind from the PLC navigator is two undo steps (`assembly.attach`, then `plc.bind_channel`; ID-899): make it one step, as FT-656 did for placements.
- FT-737 `ew-api, ew-viewmodel`: **Medium.** The PLC grid's data-type, safety and signal-range columns and the rack column (WP-S2.07 §8, PBF §2.8), once WP-S1.21 models data type, safety and signal range and racks.
- FT-739 `ew-api, ew-viewmodel`: The PLC tree's rack level and the bus ports under their station (WP-S2.07 §8, doc 10 §6).
- FT-776 `ew-ui`: The PLC grid's Address header elides when the column is sorted (seen at cycle 39's wave-2 window check).
- FT-831 `ew-viewmodel, ew-ui`: A part's bus ports from the window: the generic CPU's `DP` port is typed only once the drafter places the DP port symbol joined to the CPU (N5, WP-S1.21 D16), a step a first-timer will not guess. …
- FT-856 `ew-viewmodel (field catalogue), ew-ui`: A bus port's interface, role and node address are not in the Properties grid, so the window cannot show or edit them there (W40b).
- FT-1672 `ew-viewmodel, ew-api`: A project with two or more CPUs cannot be built in the window: a new card joins a CPU by itself only while the project has one (D4), the notice says "attach the module to a station", and the window has no command for that (`assembly.attach` exists for `ew apply` only), so the PLC panel never lists the card. Build the attaching: the PLC panel lists the cards on no CPU, and a CPU row's menu attaches them ("Attach cards without a CPU").
- FT-434 `WP-S2.04`: `Bus` and `Fluid` connection kinds drawn by the Wire tool on bus-topology and fluid pages (doc 06 §2.2), with autoconnect proposals filtered by discipline once a table connection point carries one …
- FT-825 `ew-scene`: Canvas legibility: PLC addresses and card pin labels are tiny and turned sideways at the zoom a first-timer works at (rv-vis-3b t0026, t0046; rv-vis-3 t0003, t0004, t0011).

## Later

New development and polish for M3 and after, not scheduled: [backlog-later.md](backlog-later.md).
