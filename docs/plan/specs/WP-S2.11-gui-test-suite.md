# WP-S2.11: The GUI test suite as the per-PR gate (closed specification)

**Status:** amended 2026-11-05, cycle-52 design pass: §12's design note on one registered type for a form's closed choices (FT-1476, decision log ID-1434) · complete in cycle 51, built whole by one lane and its fix round FIX51p3 (decision log ID-1354,
ID-1375); the Linux pixel gate is proven by CI at the delivery, after the merge point's WSL re-bless; specified in cycle
50 (documents only), an M2 package built whole by one lane (ID-1281) in the eight steps
of §6, with one hand-back point (§11); about 2.6 pw by its steps, the PERT of 2.6 pw unchanged · **Date:** 2026-11-04
· **Work package:** [WP-S2.11](../02-work-packages.md#wp-s211-gui-test-suite-egui_kittest) · **Decision log:**
ID-1281, ID-1322, ID-1327 · **Part of:** ElektraWatt plan, specifications

**Purpose.** Most of the GUI suite exists: `crates/ew-ui/tests/` drives the window with egui_kittest without
rendering, runs an accessible-name walker after every settled frame, and holds a pixel gate of 61 chrome snapshots.
The parts of AC-28 are spread over three files, and none of them follows "102-A1" from 101-H16 and back. The walker
checks names, not role and state, and only on the screens a flow happens to reach. The pixel gate runs on Windows
(WARP) only. The acceptance runner reports per AC and per D-nn but not per requirement ID, so a requirement whose
criterion names the golden project and has no test is not listed as missing, and its G-M2 GUI entries cannot see the
kittest results. Seven follow-up rows are filed under this package. This document specifies only that gap, against the
code at `e89345a2` (§2.1). Its size is an estimate, not a commitment (ID-1210); every time is not measured (owner's
rule). Where this document and a design document differ, this document is the narrower and wins for the lane; the
design documents stay the target and are updated at the design pass (§14). A choice the lane meets that this document
does not settle is escalated, never guessed.

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [The texts](#5-the-texts)
6. [Build order](#6-build-order)
7. [Acceptance](#7-acceptance)
8. [Golden A's expected changes](#8-golden-as-expected-changes)
9. [Neighbours, files owned and what a lane may not decide](#9-neighbours-files-owned-and-what-a-lane-may-not-decide)
10. [Risks](#10-risks)
11. [Size, routing and the hand-back point](#11-size-routing-and-the-hand-back-point)
12. [Follow-ups, proposed](#12-follow-ups-proposed)
13. [Open questions](#13-open-questions)
14. [Documents to update](#14-documents-to-update)
15. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package:
- **AC-28.** AC-28's four steps run as one kittest flow on a copy of Golden A, with the accessibility check after every
  step.
- **Accessibility.** A census opens every surface of the window and checks every interactive node for its name, its
  role and its state. A new surface fails the census until it is listed.
- **Snapshots.** The pixel gate runs on Linux with a pinned Mesa lavapipe, as well as on Windows.
- **NFR-MAINT-11.** The acceptance runner lists every MVP requirement whose criterion names the golden project, with
  the AC entries and the tests that prove it, or `missing`. The gate run fills in pass or fail, the kittest results of
  the G-M2 GUI entries included.
- **The follow-up rows.** FT-363, FT-696, FT-701's rest, FT-771 and FT-772 are built and closed. FT-455 becomes a
  written manual check. FT-713 is ruled (§13 O2).

**Non-goals** (each with its package):
- No usability testing with people (WP-S2.12).
- No timing assertion of any kind (owner's rule): not AC-28's "every frame ≤ 16 ms" and not doc 18 §3.11's UI-thread
  watchdog (NFR-PERF-04, NFR-PERF-19). These are measured at milestones, never in this suite.
- No new test for a requirement row this package does not own: the runner lists such a row as `missing`, and its own
  package writes the test (FP-3).
- No change to the existing flows' assertions, to the Windows blessed images, or to WP-S2.08's colour-vision,
  contrast, pseudo-locale, keymap and 300 % tests. These exist and stay as they are (§2.1).
- No NVDA or Orca run here: the protocol is written here, and the run belongs to the release (RC-05, WP-S6.08).
- No new crate or tool beyond Mesa's lavapipe, which CI already installs; no change to `deny.toml`, `supply-chain/` or
  `Cargo.lock`.

## 2. Inputs and the code as it is

| Input | What the lane takes from it |
|---|---|
| [Quality](../../design/18-quality-security-release.md) §3.1 (row 10), §3.10, §3.11, §5.2 (CI-PR-02, CI-PR-03, CI-PR-07) | The UI layer's jobs; the runner's report; the job catalogue |
| [UX/UI](../../design/16-ux-ui.md) §10.5, §18.1 | The checks and their cadence; AC-28's walkthrough steps 3 and 4 |
| [Architecture](../../design/04-architecture.md) §13 | The UI row of the testing strategy |
| [Golden project](../../design/20-golden-project.md) §21 (AC-18, AC-28), §22 | The criteria and their gates; each AC row's requirement column |
| [Non-functional requirements](../../design/02c-nonfunctional-requirements.md) NFR-A11Y-02, NFR-MAINT-11 | §3 |
| ADR-0014 decision 6 and amendment 2; ADR-0031 (the snapshot features); WP-S2.17's D7 as amended | The walker over every focusable node; the snapshot path without kittest's render feature |
| [Gates](../03-gates.md) G-M2-02, G-M2-10 | The evidence the gate reads |
| FT-363, FT-455, FT-696, FT-701, FT-713, FT-771, FT-772 ([work breakdown](../02-work-packages.md), appendix B) | §4 D8 to D13 |

### 2.1 What exists at `e89345a2` (the base contract)

The lane relies on exactly these items. If one differs, the lane stops and reports it.

| Item | Where | Used for |
|---|---|---|
| The kittest suite: one test binary of 56 modules, without rendering; `egui_kittest` 0.37.0 as a dev-dependency with no default features | `crates/ew-ui/tests/it/main.rs:1-10`; `crates/ew-ui/Cargo.toml:58` | D1, D3 |
| The walker: `settle` runs a frame and `assert_named`, which checks that every visible node with an input role or the Focus action has a label or a placeholder; `assert_fits` beside it | `crates/ew-ui/tests/support/mod.rs:141-148`, `:185-230`, `:232-241` | D3 |
| AC-28's pieces: W29's chips, the colour through the editor and undo (PG5); a terminal joins -X1031 with the ghost's predicted tag (PL6); a link followed and back from folio 103; the link of folio 101 to 102-A1 found, not followed | `crates/ew-ui/tests/it/properties.rs:4-6`, `:83-84`; `crates/ew-ui/tests/it/place.rs:4-6`, `:66`; `crates/ew-ui/tests/it/viewer.rs:118-136`; `crates/ew-ui/tests/it/overlays.rs:577-607` | D1 |
| WP-S2.08's accessibility tests: the five core tasks from the keyboard (AX-5), colour vision (AX-6), the contrast lint (DL1), the pseudo-locale (LO-5), the keymaps, 300 % scale; the selection's live region (WP-S2.08 D13) | `crates/ew-ui/tests/it/keyboard_core.rs:4`, `colour_vision.rs:4`, `theme.rs:44`, `pseudo_locale.rs:4`, `keymaps.rs:4`, `scaling.rs:4`; `crates/ew-ui/src/canvas.rs:1417-1431` | §1 non-goals |
| The surfaces: `Dialog` (18 variants, one of them the creation forms `FormDialog` keyed by action), `PaneKind`, `NavKind` | `crates/ew-viewmodel/src/shell.rs:269-363`; `crates/ew-viewmodel/src/shell/creation.rs:221-235`; `crates/ew-viewmodel/src/layout.rs:53`; `crates/ew-api/src/output/navigators.rs:50` | D3 |
| The pixel gate: feature `snapshots`, compiled on Windows only, offscreen through `egui-wgpu` and `ew-render2d`'s headless device; a channel tolerance of 4 and at most 10 differing pixels; 61 blessed PNG files | `crates/ew-ui/Cargo.toml:20`; `crates/ew-ui/tests/snapshots.rs:1-23`; `crates/ew-ui/tests/offscreen/mod.rs:16`, `:49`, `:52`; `crates/ew-ui/tests/expected/snapshots/` | D4 |
| CI: `test-linux` and `test-linux-default` install `mesa-vulkan-drivers` from apt, unpinned, for the GPU parity test; `test-windows-3` runs the pixel gate on WARP and uploads its images on failure | `.github/workflows/ci.yml:177-183`, `:200-205`, `:316-321`, `:558-560`, `:591-598` | D4 |
| The acceptance runner: `cargo xtask acceptance` runs `ew-api`'s `acceptance` gate (M1) and writes the report, one row per AC and D-nn check; the committed headless report has 106 entries, and D-06's property-grid entry is `not_applicable` ("a G-M2 entry") | `xtask/src/main.rs:130-134`, `:647-690`; `crates/ew-api/tests/acceptance.rs:1-24`; `golden/J-E-2416/expected/acceptance_headless.md:3`, `:75` | D2 |
| The GP-named requirement rows: 163 rows of 02a, 02b and 02c whose acceptance column names GP, 144 of them MVP or "MVP / 1.0" (counted at the base) | `docs/design/02a-requirements-core.md`, `02b-requirements-extended.md`, `02c-nonfunctional-requirements.md` | D2 |
| FT-363: a `GpuError::Limits` from the canvas is recorded as the host's fault, which ends the GPU path for the session | `crates/ew-render2d/src/gpu/switch.rs:126-135`; `crates/ew-ui/src/gpu.rs:160-198`, `:217`, `:435` | D8 |
| FT-696: the conflict overlay draws the connections that the open EW-POT-001 findings name | `crates/ew-viewmodel/src/navigators/messages.rs:69-70`; `crates/ew-viewmodel/src/shell/messages.rs:130`; `crates/ew-ui/src/canvas.rs:354-364` | D9 |
| FT-701 and FT-772: the slot-text collision rules (pin texts are obstacles of cross-reference texts only) and the black-box caption stack (it avoids the box's CPs) | `crates/ew-scene/src/page.rs:80-98`, `:990`, `:1458` | D10, D11 |
| FT-771: `TableRep.tag_at` from the tag slot's default anchor | `crates/ew-api/src/output/editing.rs:1277`, `:1667` | D12 |
| RC-05, the release's manual tests | `docs/release/checklist.md:22` | D13 |

## 3. Requirements traced

| Requirement | State and what this package delivers | Acceptance |
|---|---|---|
| NFR-A11Y-02 | Built in part: names on the flows' screens and the live region (WP-S2.08); here name, role and state on every surface (D3), and the screen-reader protocol for RC-05 (D13) | GT-3, GT-9 |
| NFR-MAINT-11 (the rows naming GP) | Built for AC-nn and D-nn at M1 (WP-S6.01); here the per-requirement table with `missing`, and the G-M2 GUI entries from kittest (D2) | GT-2 |
| AC-28 | Built in three parts; here one flow with step 1 as written (D1); the frame bound is not measured (owner's rule) | GT-1 |
| AC-18 (GUI) | Built (WP-S2.09's Golden B in the window); here an entry of the gate run (D2) | GT-2 |
| FT-363, FT-696, FT-701, FT-771, FT-772 | §4 D8 to D12 | GT-5 to GT-8 |
| FT-455 | A manual check, written (D13) | GT-9 |

## 4. Decisions

Each is a lead ruling that the owner may reverse.

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **AC-28 as one flow.** A module `crates/ew-ui/tests/it/ac28.rs` runs, on one copy of Golden A: (1) folio 101, the cursor to the link at 101-H16 (Shift+F until the live region names "102-A1"), F, and folio 102 is shown with cell A1 inside the view; Alt+← returns to folio 101 with the earlier view exactly; (2) W29 selected through `select.set`, its chip line and colour line read whole ("0.75 mm² · potential 24VDC-1 · size profile '0.75/1.5' · class control"; "DBU · colour profile 'DBU/WH 24 V'"); (3) the colour changed through the editor, then undone, the wire's colour and the model's digest back to what they were; (4) a terminal added to -X1031 with the ghost's predicted tag shown before the click. `settle` (D3) runs after every step. The existing PG5, PL6 and viewer tests stay | One test that the gate and a reader can point at as AC-28; step 1 as AC-28 states it, which no test runs today |
| D2 | **The requirement table and the gate's GUI entries.** A hand-kept map, `golden/J-E-2416/acceptance/requirements.toml`, with the map marker `# map: acceptance-requirements: a GP-named requirement's tests go here, one [[requirement]] per row ID; a criterion proven outside ew-api's runner goes in [[criterion]].` Each `[[requirement]]` names a row ID and its tests as `<binary ID> <test path>`. Each `[[criterion]]` names an AC or D-nn check that `ew-api`'s runner cannot evaluate, with its tests: AC-18 (GUI), AC-28, D-06's property grid (PG9), NFR-A11Y-01 (AX-5), NFR-A11Y-02 (the census). `cargo xtask acceptance` gains `--gate <M1\|M2>` (M1 stays the default). xtask builds the table from the requirement rows of 02a to 02c whose acceptance names GP and whose release is MVP or "MVP / 1.0", from the AC rows of doc 20 §21 whose requirement column names them, and from the map. It writes `golden/J-E-2416/expected/acceptance_requirements.md`: one row per requirement, with its release, ACs, tests and status (`mapped` or `missing`). An xtask test compares the committed file with the generated one per PR, and checks that every mapped test exists (its function name in its binary's sources). The gate run runs the mapped tests in one nextest run, reads its JUnit file with a reader of a few lines (no new dependency), and writes `pass`, `fail` or `missing` into the report beside the AC table. `missing` does not block at 0.1.0 (§13 O3). The lane maps the rows whose existing tests cite the requirement ID and assert on Golden A; it writes no test for a row it does not own | NFR-MAINT-11's "pass/fail per requirement ID … a GP-named criterion without a test is listed as missing"; the GUI criteria live in another test binary than the runner, so the gate reads their results instead of re-implementing them; a static per-PR table keeps the map honest without running the suite twice |
| D3 | **The accessibility census.** `assert_named` grows into `assert_accessible`, which `settle` runs. It keeps R1 and adds the role and state rules, each checked on every visible, enabled, interactive node: (R1) a name, label or placeholder; (R2) a `CheckBox`, `Switch`, `RadioButton` or toggle button has its toggled state; (R3) a `ComboBox`, a node with a pop-up, or a `TreeItem` with children has its expanded state; (R4) an option, tab, row or tree item of a selectable container has its selected state; (R5) a text input or spin button has a value (empty allowed); (R6) a button whose action the registry reports unavailable is disabled, not hidden. A new module `crates/ew-ui/tests/it/census.rs` opens every surface on a copy of Golden A (folio 101, with a device and a wire selected, so that the cards show) and settles on each: every `Dialog`, `PaneKind` and `NavKind` variant, by a `match` with no wildcard arm, so a new variant does not compile until it is listed; every creation form, by the actions that build a `FormDialog` (a source check compares the census list with the `action:` members of those constructions); the six command-bar tabs and every menu of the action registry; the start page, the first-run steps and the ten wizard steps; and the Preview, History, Find and Messages panes with content. A widget that fails a rule is fixed in `ew-ui` in this package | NFR-A11Y-02's "100 % of interactive widgets … name, role and state" (G-M2-10), checked where the widgets are, not only where a flow passes; the exhaustive match makes the census grow with the window |
| D4 | **The pixel gate on Linux.** `snapshots.rs` compiles on Windows and Linux (`any(windows, target_os = "linux")`). The gate reads `tests/expected/snapshots/linux/<name>` on Linux when that file exists, else the shared file, with the same tolerance. Blessing on Linux writes a `linux/` file only for an image that differs beyond the tolerance from the shared one. Mesa is pinned: `ci/mesa-lavapipe.txt` holds the package version of `mesa-vulkan-drivers` (MIT; the version the lane installs from Ubuntu 24.04's snapshot archive at a fixed timestamp, **verify** that the runner's apt takes the snapshot), both Linux test jobs install exactly it, and a step fails with §5 U4 when `dpkg-query` reports another. The gate runs in `test-linux`'s all-features run, where `ew-ui` already is, and uploads its images on failure as `test-windows-3` does. CI-PR-07 is this run, not a job of its own | Doc 18 §3.11's two platforms, with one stable rasteriser per platform; override files keep the shared set the source and add no copies where the two rasterisers agree; a version check turns a moved mirror into a clear failure instead of a pixel diff |
| D5 | **The census's state on Golden A.** Every surface opens on a read-only copy of Golden A made by the suite's `Tmp` (never the committed directory), and the census commits nothing. The dialogs that need a state (the recovery dialog, the external-change dialog) get it through the shell's existing test hooks only | The census must not depend on a fixture of its own, and must not write Golden A |
| D6 | **The texts the suite asserts are whole** (`assert_eq!`); a census failure lists every failing node of a surface at once, so one run shows the whole list | The repository's rule; one run per fix round |
| D7 | **The suite stays without timing.** No assertion reads a clock; the bounded waits of `wait_for_page` stay a bound on progress, never a pass criterion | The owner's rule |
| D8 | **FT-363: a per-frame CPU fallback.** When the GPU canvas returns `GpuError::Limits` for a frame (a pane larger than the device allows), `GpuHost` gives no texture for that frame and records no fault; the CPU draws that frame, and the next frame tries the GPU again. Every other `GpuError` keeps today's path (the fault, the CPU canvas for the session, the banner) | A too-large pane is a property of one frame, not a broken device; the session keeps its GPU |
| D9 | **FT-696: potentials keyed by a connection point.** The overlay's input becomes, per open EW-POT-001 finding, the connections it names and, for each connection point it names, the connections that end at that point. A point with none is drawn as a ring of the overlay's pen and colour, one grid step across | The overlay then shows every potential a finding reaches; a lone point still shows where the conflict is |
| D10 | **FT-701's rest: pin texts as obstacles for technical data.** A black box's pin texts become fixed obstacles of rule 3 (technical-data texts) as they are of rule 2: a technical-data text that a pin text collides with drops below it as rule 2's texts do, at most three times. No other text moves | The row's open half; rule 2's mechanics, so no new placement logic |
| D11 | **FT-772: captions clear of stored wires.** When a black box's caption stack would cross a stored wire segment on its page, the whole stack moves left in grid steps, at most to the outline's left edge, until no caption box crosses one; when no position is free it stays where it was | The same move as the caption rule's left alignment; deterministic, with no search beyond the outline |
| D12 | **FT-771: `tag_at` where the tag is drawn.** `TableRep.tag_at` reports the tag's position after the scene's tag rule (ID-898), read from the same query that places it for drawing, so a hit test and a report agree with the page | One source for the position |
| D13 | **The manual checks, written.** `docs/release/manual-tests.md` (new; §13 O4) holds three protocols, each with its setup, steps, expected result and a result line: MT-1 NVDA on Windows and MT-2 Orca on Linux (find -102F1 on Golden A, read its part and its connections, open its page: NFR-A11Y-02's manual row); MT-3 the Linux clipboard (FT-455): copy in one instance and paste in another under Wayland and under X11, and paste from a text editor. WP-S6.08's RC-05 cites the file (§13 O4) | The manual rows get a protocol a tester can follow; the runs stay with the release |
| D14 | **Every item closed;** no decision changes an ADR. ADR-0014's amendment 2 is implemented as written (D3) | — |

## 5. The texts

Every text is asserted whole. No user text changes: these are test and CI outputs (en-GB only).

| # | Where | Text |
|---|---|---|
| U1 | `assert_accessible`, one line per failing node | `<surface>: <role> at <bounds> has no accessible name` (R1); `<surface>: <role> "<name>" has no <state> state` (R2 to R5, `<state>` one of `toggled`, `expanded`, `selected`, `value`); `<surface>: <role> "<name>" is enabled, but its action is unavailable` (R6) |
| U2 | The census's source check | `the action <action> builds a form that the census does not open` |
| U3 | xtask, the requirement map | `golden/J-E-2416/acceptance/requirements.toml: <id>: the test <binary> <path> does not exist`; `…: <id> is not a requirement whose acceptance names GP`; `…: <id> is listed twice` |
| U4 | CI, the Mesa pin | `mesa-vulkan-drivers is <got>, not the pinned <want> of ci/mesa-lavapipe.txt` |

## 6. Build order

One lane builds the eight steps in this order and commits at the end of each.

1. **AC-28** (D1): `ac28.rs`.
2. **The requirement table** (D2): the map with its marker, xtask's table and its per-PR test, the committed table,
   `--gate M2` and the GUI criteria; the AC-28 entry names step 1's test.
3. **The census** (D3, D5, D6): `assert_accessible`, `census.rs` and the source check, the widget fixes.
4. **The Linux gate** (D4): the cfg, the override rule, the Mesa pin and its check in both Linux jobs, the Linux images
   that differ (§13 O1).
5. **FT-363** (D8).
6. **FT-696 and FT-771** (D9, D12), with FT-696's frames at 1280 × 800 and 1920 × 1080.
7. **FT-701's rest and FT-772** (D10, D11), with frames of the moved texts on fixture pages at both sizes.
8. **The manual checks** (D13) and the release notes' entry.

## 7. Acceptance

- **GT-1** (AC-28) `ac28` passes: each step's assertion holds, its texts are asserted whole, and `settle` passes after
  every step; the view after Alt+← equals the view before F.
- **GT-2** (NFR-MAINT-11) The committed `acceptance_requirements.md` equals xtask's output, and lists every MVP
  GP-named row with its ACs and tests or `missing`. A fixture map with a test that does not exist, a row that names no
  GP, or a row listed twice fails with U3 whole. The delivery's `cargo xtask acceptance --gate M2` run, made by the
  orchestrator, reports AC-18 (GUI), AC-28, D-06, NFR-A11Y-01 and NFR-A11Y-02 as `pass` from their kittest tests, and
  every mapped requirement's status. No test pins the number of rows.
- **GT-3** (NFR-A11Y-02) `census` passes over every surface D3 lists. A unit test of `assert_accessible` on a fixture UI
  (a checkbox without a toggled state, a combo box without expanded, an unnamed button) fails with U1's lines whole.
  The source check fails with U2 on a fixture list without one form's action. Every flow's `settle` runs the six rules.
- **GT-4** (doc 18 §3.11) The delivery's `test-linux` run passes the pixel gate on lavapipe with the pinned Mesa;
  `test-windows-3` passes it on WARP with the shared images unchanged. The pin check is shown to fail on a wrong version
  (its script's `--test`).
- **GT-5** (FT-363) A canvas given a `Limits` error for one frame draws that frame on the CPU, records no fault and
  draws the next frame on the GPU; a `DeviceLost` still ends the GPU path with its banner.
- **GT-6** (FT-696, FT-771) An EW-POT-001 finding that names a connection point draws that point's connections in the
  overlay, and a point without connections draws its ring (frames at both sizes). `tag_at` equals the drawn tag's
  position on a fixture page where the tag rule moves the tag.
- **GT-7** (FT-701, FT-772) On fixture pages: a technical-data text drops below a pin text it would collide with, and a
  caption stack that would cross a stored wire moves left until it is clear, or stays where it was when no place is
  free. Golden A's PDF digest, PNGs and text CSVs are unchanged.
- **GT-8** The hand-back checks of the lane's package set pass, with `ew-ui`'s `snapshots` among them (fmt, settle,
  clippy, doc, nextest, policy, trailers).
- **GT-9** (NFR-A11Y-02 M, FT-455) `docs/release/manual-tests.md` holds MT-1 to MT-3 as D13 says, and `doccheck.py`
  passes on it; the runs are RC-05's at the release.

## 8. Golden A's expected changes

None. Steps 6 and 7 change how texts are placed, and Golden A's PDF digest, PNGs and text CSVs are expected to stay
byte-equal. A change to any of them is a stop: the lane reports the pages and the frames, and the orchestrator rules.
The two new files under `golden/J-E-2416/` (the map and the requirement table) change no existing file. The commit
that adds them carries the `Golden:` trailer of the brief's ID.

## 9. Neighbours, files owned and what a lane may not decide

### 9.1 Neighbours

| Neighbour | It owns | This package |
|---|---|---|
| WP-S6.01 | The acceptance runner and its M1 report | Adds the requirement table and the M2 GUI criteria beside it; the M1 report is unchanged |
| WP-S2.08 | The accessibility flows, the live region, colour vision, contrast, pseudo-locale, scale | Keeps them; `settle` now checks role and state on their screens too |
| WP-S2.17 | The pixel gate and its Windows images | Adds Linux to it without touching the shared images |
| WP-S2.02 | The GPU host | FT-363's fallback |
| WP-S6.08 | RC-05 and the G-M2 run | Gives RC-05 the protocols; its gate run uses `--gate M2` |
| WP-S2.12 | Usability rounds with people | FT-713's friction 3, if §13 O2 keeps option 1 |

### 9.2 Files owned

`crates/ew-ui/tests/` (`it/ac28.rs`, `it/census.rs`, `support/mod.rs`, `snapshots.rs`, `expected/snapshots/linux/`);
`crates/ew-ui/src/` (the widget fixes of D3, `gpu.rs` for D8, `canvas.rs` for D9);
`crates/ew-viewmodel/src/shell/messages.rs` and `navigators/messages.rs` (D9); `crates/ew-api/src/output/editing.rs`
(D12); `crates/ew-scene/src/page.rs` (D10, D11); `xtask/src/` (the table, `--gate`, its tests);
`golden/J-E-2416/acceptance/requirements.toml` and `golden/J-E-2416/expected/acceptance_requirements.md` (new);
`.github/workflows/ci.yml` (the two Linux jobs' Mesa steps and the upload); `ci/mesa-lavapipe.txt` and its check
script (new); `docs/release/manual-tests.md` (new); `docs/release-notes/unreleased.md`. Read-only: `golden/J-E-2416.ew`,
`crates/ew-ui/tests/expected/snapshots/*.png` (the shared set), `docs/release/checklist.md`, `deny.toml`,
`supply-chain/`, `Cargo.lock`, every other document.

### 9.3 What a lane may not decide

A lane stops and reports instead of choosing when:
1. Golden A's outputs change (§8), or a shared blessed image would have to be re-blessed;
2. a widget's fix under D3 would change what it shows or does, not only what it exposes, or the fixes pass about 0.3
   pw;
3. a requirement row seems to need a new test to be mapped, beyond D1 and D3;
4. the Mesa pin cannot be held by the snapshot archive, or Linux images cannot be blessed by §13 O1's way;
5. a text would differ from §5, or a new dependency seems needed.

The lane decides freely: the census's surface names and order; how a dialog is opened for the census (its action, or
the shell's test hook); the mapping of a requirement to existing tests; the JUnit reader's code; the snapshot
timestamp of the Mesa pin; and the fixture pages of steps 6 and 7.

## 10. Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 The role and state rules fail on many existing widgets at once | Medium · medium | They are NFR-A11Y-02's acceptance; §9.3 item 2 bounds the fixes, and the de-scope rule drops R6 after the two scene rows |
| R2 lavapipe and WARP differ on many images, so the Linux set is almost a second copy | Medium · low | Override files only where they differ (D4); a full copy is still one bless |
| R3 The snapshot archive's Mesa is withdrawn or apt cannot take a snapshot on the runner | Low · medium | §9.3 item 4; the fallback is the CI-PR-05 container pinned by digest, a ruling |
| R4 The requirement map invites mapping weak tests | Medium · low | Only tests that cite the row and assert on Golden A (D2); the reviewer reads the map |

## 11. Size, routing and the hand-back point

**Size.** About 2.6 pw: step 1 0.2, step 2 0.6, step 3 0.55, step 4 0.5, step 5 0.1, step 6 0.25, step 7 0.3, step 8
0.1. The PERT of 2.6 pw (O 1.5 · L 2.5 · P 4) stays. **De-scope rule:** when the lane is projected more than 20 %
over, these leave as follow-ups without a further ruling, in this order: FT-772 (D11), FT-701's rest (D10), rule R6 of
D3.

**Routing** (`.claude/orchestration.md` rule 0): one lane, **Opus at `medium`** (decided work with tests to run, a CI
change and two scene rules; no format change). WP-S2.04 and WP-S2.05 are complete.

**Hand-back point** (ID-1322). The lane hands back at the end of **step 4** when it then stands above about 550k
tokens; one that nears 800k anywhere commits and hands back. Its hand-off note then holds: the steps done with their
commits; the map's rows so far and the rows left `missing`; the surfaces of the census and the widgets fixed, with
those still failing; the Mesa version and timestamp pinned, and how the Linux images were blessed; the Linux override
files written; the test filters (`-p ew-ui --test it -E 'test(/^(ac28|census)::/)'`, `ew-ui`'s `snapshots` with
`--features snapshots`, `xtask`'s acceptance tests, `ew-scene`'s page tests, `ew-viewmodel`'s messages tests); and
every choice of §9.3's free list already made.

## 12. Follow-ups, proposed

The design pass opens the rows. FT-363, FT-696, FT-701, FT-771 and FT-772 close with steps 5 to 7, FT-455 with step 8.
FT-713 closes by §13 O2's ruling.
- **FP-1** NFR-UX-07's test: every registered tool cancels with Escape and leaves the revision unchanged (doc 16
  §10.5).
- **FP-2** A readable first zoom for a page at the 1280 × 800 floor, tried in WP-S2.12's R0 round (FT-713 friction 3,
  if §13 O2 keeps option 1).
- **FP-3** The `missing` rows of the requirement table, each to the package that owns its requirement.
- **FP-4** The UI-thread watchdog and AC-28's frame bound at the milestone performance run (NFR-PERF-04, NFR-PERF-19).

*Design note (2026-11-05, cycle-52 design pass; FT-1476, FT-1352; decision log ID-1407, ID-1434).* **One registered
type for a form's closed choices.** The census opens every form, and the choice-word test of `ew-viewmodel`
(`every_closed_choice_has_its_words_in_both_locales`, ID-1278 ruling 2) checks that each closed choice a form shows
through `strings::choice_text` has its `choice-<value>` text in en-GB and nl-NL. The test reads the action
signatures' `enum(…)` and `bool` parameters, the format's page types and `CLOSED_SETS`, a list kept by hand in
`shell.rs` beside its map marker, because a form field's closed choices are written field by field
(`choices: PAGE_TYPES`, `choices: BOOL_CHOICES`, …) with no list behind them; a new set a form uses and the list
forgets has no check (FT-1352 could not derive it, ID-1407). Three ways were weighed:
- **(a) A registered type (recommended).** A form field's `choices` becomes `&'static ClosedChoices`, a struct whose
  fields are private to one module, `strings::closed` (or `shell::choices`), so it can be built nowhere else. That
  module declares every set in one `closed_choices!` invocation, which defines each set as a `pub static` and the slice
  `ALL` from the same lines. A form can name only a set the macro defined, so `ALL` is complete by construction: no
  second list, no map marker. The choice-word test reads `ALL` in place of `CLOSED_SETS`, the census can assert that
  every choice a form shows comes from `ALL`, and the free-text fields keep an empty `choices`. Run-time lists (the
  project's locations, the enclosure parts in `options`) are open sets, outside the type. No new dependency (no
  `inventory` or `linkme`); permissive crates are not touched.
- **(b) A source scan.** A test greps the crate's source for `choices:` and resolves each constant's name. It is
  fragile (a re-export, a renamed constant or a computed slice escapes it) and keeps the list implicit.
- **(c) The census collects them.** The census, which opens every form on a Golden A copy, reads each open form's
  closed choices and checks their words. It needs no new type, but a choice set reached only in a state the census
  does not set up (a form behind a revision or a selection) is missed, and the check moves into the slower window
  suite.

The recommendation is (a), built in one FT batch lane: the type and the macro, every form field converted, the test
reading `ALL`, `CLOSED_SETS` and its `closed-sets` map marker removed, and a compile-time refusal of a choice list
built outside the module (FT-1521).

## 13. Open questions

- **O1** How the Linux images are blessed. Options: (1) the lane blesses them in WSL Ubuntu 24.04 with the pinned Mesa
  in a clone of its own, and commits only the `linux/` overrides; (2) the CI run uploads its images and the
  orchestrator commits them after a look; (3) Linux renders without a gate. Recommendation: (1); it keeps the bless in
  the lane's commit, and (2) needs a manual artefact download that the environment does not have.
- **O2** FT-713. Options: (1) frictions 12 and 17 are the reference drawing's own layout, which Golden A reproduces, so
  they close as such; friction 3 (texts unreadable at the 69 % fit zoom at the floor) goes to WP-S2.12's R0 round
  (FP-2); (2) open pages at a readable zoom now (a visible change of every first view). Recommendation: (1); a first
  view that hides part of the page is a usability question for people, not a test-suite change.
- **O3** The `missing` rows at 0.1.0. Options: (1) listed and non-blocking; G-M2 reads the list; (2) a `missing` row
  fails the M2 gate. Recommendation: (1), since NFR-MAINT-11's criterion asks for the list; whether the gate blocks on
  it is G-M2's ruling.
- **O4** `docs/release/manual-tests.md` beside WP-S6.08's checklist. Options: (1) this package writes the file (MT-1 to
  MT-3) and WP-S6.08's RC-05 cites it, with later manual protocols added there; (2) the protocols go into
  WP-S6.08's own release documents. Recommendation: (1); the protocols belong with the requirement they test.
- **O5** The size: about 2.6 pw, the PERT's. Recommendation: keep it.

## 14. Documents to update

At the design pass after the lane: doc 18 §3.11 (the census, the role and state rules, the Linux gate as built, no
watchdog in the suite), §5.2 (CI-PR-07 as the Linux pixel gate inside `test-linux`; the Mesa pin), §3.10 (the
requirement table and `--gate`); doc 16 §10.5 (the census in place of the walker over flows, the manual protocols'
file); doc 20 §22 (the G-M2 GUI entries from kittest); gates G-M2-02 (the frame bound as a milestone measurement) and
G-M2-10 (the census as its evidence); 02c NFR-MAINT-11 (the table's file); ADR-0014 amendment 2 (as built, with role
and state).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S2.11, WP-S6.01, WP-S2.08, WP-S2.17, WP-S2.02, WP-S6.08, WP-S2.12;
  FT-363, FT-455, FT-696, FT-701, FT-713, FT-771, FT-772.
- [Decisions](../decisions.md): ID-1210, ID-1281, ID-1322, ID-1327; [decision log](../06-decision-log.md): ID-866,
  ID-898.
- [Quality](../../design/18-quality-security-release.md) §3.1, §3.10, §3.11, §5.2;
  [UX/UI](../../design/16-ux-ui.md) §10.5, §18.1; [architecture](../../design/04-architecture.md) §13;
  [golden project](../../design/20-golden-project.md) §21, §22;
  [non-functional requirements](../../design/02c-nonfunctional-requirements.md) NFR-A11Y-02, NFR-MAINT-11.
- [ADR-0014](../../adr/0014-gui-toolkit-egui.md),
  [ADR-0031](../../adr/0031-design-language-and-task-ordered-command-bar.md);
  [gates](../03-gates.md) G-M2-02, G-M2-10.
- `crates/ew-ui/tests/`; `.github/workflows/ci.yml`; `docs/release/checklist.md` (RC-05).
