# Backlog

Open work, one row each. A finished row is deleted; a new one is added under its milestone. The numbers are the
old follow-up numbers, kept so that a row's history can be found in git (`5516edba4`, `docs/plan/follow-ups.md`
and the work breakdown's Appendix B).

## M3

Rows that a work package of [m3.md](m3.md) takes. The lane checks each row first: a row that the code already
answers is deleted with a line in its report.



### The window at 1920 × 1080

- FT-1730 `ew-ui, ew-viewmodel, elektrawatt, xtask`: **High.** 1920 × 1080 is the main resolution and 2560 × 1440 its extension (the owner, 2026-11-13; doc 16a §2), but the code and its tests are anchored on 1280 × 800: the first window, the offscreen harness's default, the picture snapshots, the size rules that grow from 1280, the command bar's laws of fit and about forty test files. Re-anchor them on 1920 × 1080, let the size rules carry on to 2560 × 1440, undo what the 1280 fit forced, and keep one test that a smaller window stays usable. Before WP-S2.13.

### WP-S2.13: automation workspaces

- FT-1736 `ew-viewmodel, ew-ui`: File ▸ Import… on a drawing (WP-S3.19): the dialog's title stands indented past its buttons, and the unit choice shows the metre as "M" (the existing word `choice-m`).
- FT-1733 `ew-viewmodel, ew-ui`: What WP-S1.25 left to the window: the flexible fluid line's arc, autoconnect and a line-kind picker on fluid ports, and the device navigator's fluid actions (doc 11 §12).
- FT-1734 `content, ew-library`: The shipped terminal symbols (`ew.iec.terminal` and its QET twin) have no `PlcAddress` slot, so the setting `plc_address_at_terminals` (FT-1725, built) prints nothing; add the slot and look at the result on a page. The twin is in Golden A's tables: a golden change.
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
