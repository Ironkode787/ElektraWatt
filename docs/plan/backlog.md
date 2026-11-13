# Backlog

Open work, one row each. A finished row is deleted; a new one is added under its milestone. The numbers are the
old follow-up numbers, kept so that a row's history can be found in git (`5516edba4`, `docs/plan/follow-ups.md`
and the work breakdown's Appendix B).

## M3

Rows that a work package of [m3.md](m3.md) takes. The lane checks each row first: a row that the code already
answers is deleted with a line in its report.

### WP-S1.24: PLC and bus 1.0

- FT-726 `ew-checks, ew-commands`: **Medium.** The prevent half of EW-PLC-001 and EW-PLC-007 under a strict scheme (WP-S1.21 D6 names EW-PLC-007's refusal; doc 10 §3.10's modes `prevent`); today both report online and in release mode (the …
- FT-727 `ew-checks, ew-ui`: EW-PLC-012's second quick fix, "open the part's PLC section" (doc 10 §3.10; WP-S1.21 D10); only "rebind to the map's channel" is built.
- FT-700 `ew-derive, ew-scene`: **Medium.** A `PlcAddress` slot on a channel's `target` function (a relay coil wired to a card output prints the channel address); a binding to the manual `target` does not count today.
- FT-725 `ew-commands, ew-derive`: **Medium.** `plc.readdress` and `plc.convert_manual_to_auto` (WP-S1.21 PA-2, de-scoped in part 3): both need A1 over the whole configuration project inside one transaction, which ADR-0008 amendment 4 (the …
- FT-830 `ew-commands`: `library.update` triggers neither N0 (PLC aspects) nor N5 (bus ports): a part copy keeps its ID across versions, so a PLC section or a bus port that a newer version adds reaches a device only through …
- FT-323 `WP-S1.21`: The part-data confidence `ItemMeta.confidence` (verified, unverified, test) of the PLC and bus sections (doc 10 §2.6; part 533361's map is `test`, ▲U-23) and the "based on non-verified part data" …
- FT-80 `WP-S1.13`: The `DeviceLink` record kind (format change, FR-DEV-028) and with it the used-valve process-tag fallback of ID-167.
- FT-841 `ew-commands (rep.join_device), ew-viewmodel`: Extend WP-S2.18 D2 so that a bus-port FnDef can join a device whose bus part has a port no function carries yet (today only a contact joins as an auxiliary function, so BU-7g's `rep.join_device` …

### WP-S1.25: fluid 1.0

- FT-1326 `WP-S7.03`: Starter parts for a generic valve-terminal module and valve, so that a new project draws a valve terminal without Golden A's parts (WP-S1.20 FP-1).
- FT-1569 `ew-api`: A project whose fluid function definition's copy is damaged and held by no library probably refuses every other copy's update as a misfit (read, not run); reproduce, and judge the misfit without the …
- FT-1576 `ew-commands`: `valve.add` with a part that has a PLC-module section adds a device that normaliser N0 attaches to the project's only station (WP-S1.21 D4) instead of the valve module; rule whether `valve.add` …
- FT-740 `ew-ui, ew-viewmodel, content/`: **Medium.** Golden A's valve coils have no drawn internal CPs, so FR-PLC-020's acceptance (channel 1 of -201U3 dragged onto -201U3-Q1's coil on folio 203) cannot be done by canvas pick; PG2 was tested on a …

### WP-S2.13: automation workspaces

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
