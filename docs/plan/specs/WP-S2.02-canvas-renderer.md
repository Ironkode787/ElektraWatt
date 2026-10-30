# WP-S2.02: the 2D canvas renderer and its GPU-less fallback (closed specification)

**Status:** specified for cycle 21 (part 1, one lead-programmer lane, D4; part 2 after part 1's delivery, one
lead-programmer lane, D4; Opus review of each). **Part 1 delivered, as built (cycle 21; decision log ID-517 to
ID-522):** A1 to A15 passed (§10), with the lane's rulings 1 to 7 and the review's fix round amended into §4.2, §6.1,
§6.2, §6.3, §6.6 and §9. **WP-S2.02 is complete: part 2 delivered, as built (cycle 22; decision log ID-539 to
ID-544):** B1 to B9 passed (§10), with the lane's rulings (ID-540), the review's fix round (ID-542) and the review's
design items (ID-544) amended into §6.3, §7 and §8 · **Date:** 2026-10-22 (amended 2026-10-22, cycle-21 and cycle-22
design passes) · **Work package:**
[WP-S2.02](../02-work-packages.md#wp-s202-2d-canvas-renderer-ew-render2d-and-gpu-less-fallback) · **Decision log:**
ID-505 to ID-511, ID-514, ID-517 to ID-522, ID-539 to ID-544 · **Part of:** ElektraWatt plan, specifications

**Purpose.** This is the closed specification of the canvas: the page view of the shell
([WP-S2.01](WP-S2.01-application-shell.md)) becomes a canvas that shows the current page at any zoom with pan, the
grid, the page bounds, a hover highlight and the cursor in the status bar, drawn by `ew-render2d` on the GPU where one
is usable and by the CPU renderer everywhere else, measured frame by frame, and fully drivable without a display. Where
this document and a design document differ, this document is the narrower and wins for the lanes; the design documents
stay the target of the whole work package, and the documents named in §14 are updated at the design pass after each
part. A choice a lane meets that this document does not settle is escalated, never guessed (`CLAUDE.md`, rule 2).

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs](#2-inputs)
3. [The decisions of this specification](#3-the-decisions-of-this-specification)
4. [Crates and boundaries](#4-crates-and-boundaries)
5. [Dependencies and the supply chain](#5-dependencies-and-the-supply-chain)
6. [Part 1: the canvas on the CPU tile path](#6-part-1-the-canvas-on-the-cpu-tile-path)
7. [Part 2: the GPU path and the switch rule](#7-part-2-the-gpu-path-and-the-switch-rule)
8. [Determinism](#8-determinism)
9. [Frame-time instrumentation](#9-frame-time-instrumentation)
10. [Acceptance checks](#10-acceptance-checks)
11. [Files owned](#11-files-owned)
12. [Out of scope](#12-out-of-scope)
13. [Review checklist for the Opus reviewer](#13-review-checklist-for-the-opus-reviewer)
14. [Size, routing and the documents to update](#14-size-routing-and-the-documents-to-update)
15. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the work package the project view's centre pane is a canvas: the current page at any zoom from
5 % to 5000 % with pan, the page's connection grid, the page bounds, a hover highlight on the entity under the pointer,
and the cursor position, zoom and grid in the status bar. It runs at the frame budgets of NFR-PERF-04 and NFR-PERF-06
on the GPU (DX12, Metal, Vulkan, WARP) through `ew-render2d`, and on the CPU renderer where no usable adapter exists
or `--no-gpu` is given, with the active backend in the status bar (FR-UI-028, NFR-PLAT-02, NFR-PLAT-03). Every canvas
action is a registry action on `ShellModel::dispatch`, so `elektrawatt --headless --script` and the blind drafter can
zoom, pan, point and read back what the canvas shows (decision log ID-443).

**Non-goals** (each is named with its package in §12):
- **No editing gestures**: no tool state machines, drags, ghosts, snapping or keyboard grid cursor (WP-S2.03,
  WP-S2.04).
- **No selection semantics**: a click selects nothing. The hover outline is a placeholder over display-list bounding
  boxes; the overlay can draw a selection style, but only WP-S2.03's selection model feeds it.
- **No printing** (WP-S3.07) and no change to any exported byte: PDF, SVG and the golden PNGs stay as they are.
- No tabs, side by side, per-page viewports, jump history, go to cell or the cell in the status bar (WP-S2.03,
  FT-345); no themes, keymap presets or high-contrast work (WP-S2.08); no image snapshots or watchdog (WP-S2.11).

## 2. Inputs

| Input | What the lanes take from it |
|---|---|
| [WP-S2.01's specification](WP-S2.01-application-shell.md) and its as-built status | The shell model, the registry, the headless mode (`ew.uiscript/1`, `ew.uitranscript/1`, `ui.describe`), the rules of its §3 (through `Session` only, no `unsafe`, no unordered output); the page view this package replaces |
| [Architecture](../../design/04-architecture.md) §3 (rules 3, 6, 7), §8.1, §8.2, §9 | The crate table (`ew-render2d`, `ew-render-cpu`, `ew-dl`, `ew-scene`, `ew-text`), the one IR, the GPU and CPU paths, LOD, the shared wgpu device |
| [ADR-0014](../../adr/0014-gui-toolkit-egui.md), [ADR-0015](../../adr/0015-display-list-renderers-pdf.md) (decisions 1 to 3, amendments 6, 7, 9), [ADR-0022](../../adr/0022-platform-matrix-and-performance-budgets.md) | One wgpu device; the IR and its renderers; PNG determinism per build target; the GPU-less fallback order (WARP, then the CPU renderer); the measurement rules |
| [UX/UI](../../design/16-ux-ui.md) §1 (rules 1 to 3), §2.3, §3.6, §3.9, §10.1, §10.3, §10.4, §11.1, §11.2, §14, §17 (items 7, 8) | View actions never touch the model; repaint on demand; the status-bar fields; the canvas's AccessKit node; the hover and selection styles; print-true sheet colours; mixed-DPI rule |
| [Schematic editor](../../design/06-schematic-editor.md) §6 | Zoom 5 % to 5000 % in √2 steps around the cursor; pan with the middle button, Space-drag or Ctrl+arrows; fit page |
| [Reports and output](../../design/09-reports-forms-output.md) §4.3, §4.4 | The CPU raster is deterministic per build target; the golden PNGs at 150 dpi |
| [Quality](../../design/18-quality-security-release.md) §3.5, §3.11, §3.16, §3.17, §4.2, §4.3 | Determinism; UI tests; the cross-platform source rules; what the blind drafter drives; the `frame.gesture` and `render.pan` families and the runner rules |
| Requirements | FR-UI-027 (DPI scaling part), FR-UI-028, FR-UI-043 (the MVP part), NFR-PERF-04, NFR-PERF-06, NFR-PLAT-02, NFR-PLAT-03 (the WBS row), with FR-UI-034, NFR-PERF-19, NFR-PERF-20, NFR-A11Y-02 and NFR-A11Y-03 where the canvas touches them |
| Code | `crates/ew-ui/src/page_view.rs` and `latest.rs` (the raster worker), `crates/ew-render-cpu/src/render.rs` (`render`, `pixel_size`), `crates/ew-dl/src/item.rs` (`DisplayList::bounds`), `crates/ew-api/src/output/mod.rs` (`export_pages`, `font_bytes`), `crates/ew-viewmodel/src/{shell,actions,status,describe,script,strings}.rs`, `crates/ew-api/tests/page_output` (the doc 20 tolerance), `xtask/src/perf.rs` (the gate's bench line format) |

The WBS lists WP-S3.01 (the display list) as the package's dependency; it is in place as built (the export job's sheet
display list per page, ADR-0015 amendment 7). WP-S2.01 and WP-S1.08 are WP-S2.03's dependencies, not this one's.

## 3. The decisions of this specification

Each is a lead ruling that the owner may reverse; the decision log carries the full text.

| # | Decision | Reason and trade-off | Log |
|---|---|---|---|
| D1 | **Two parts.** Part 1 (2.4 pw): the canvas, its view model and actions, and the CPU tile path, which is the fallback and the backend everywhere until part 2. Part 2 (2.3 pw): the GPU path, the switch rule, FT-328 | The fallback is required anyway (FR-UI-028), and everything around the backend (viewport, input, overlays, status, headless, instrumentation) is shared; building it first gives R0 and WP-S2.03 a zoomable viewer one part earlier and gives the GPU path a measured baseline | ID-505 |
| D2 | **The GPU path's form: tessellation, not a textured quad of the CPU raster, and the first step is the CPU tiles.** Part 1's canvas is a tile pyramid of `ew-render-cpu` rasters shown as egui textures (drawn by egui's own wgpu renderer). Part 2's `ew-render2d` tessellates the display list with lyon (once per symbol and per glyph, cached by content), draws instanced with a depth-ordered paint order and screen-space minimum stroke widths into an offscreen 4× MSAA texture, and shows that texture as an egui native texture | A textured quad of the CPU raster cannot pan and zoom a dense A3 page at 60 fps on re-raster alone, so it is the loading and fallback state, not the GPU path. An offscreen texture instead of the paint callback of ADR-0015 decision 2 gives the canvas its own MSAA, one display path for both backends (an egui image), and the same code in the live view and the headless render-to-texture test. Glyphs as tessellated outlines instead of an atlas give one instanced pipeline and no atlas management at any zoom. Budget: render ≤ 5 ms p99 per frame of NFR-PERF-04 on BM; part 2 is 2.3 pw. ADR-0015 recorded the change as a pending amendment, applied as amendment 10 when part 2 was built (cycle 22, ID-539) | ID-506 |
| D3 | **The switch rule.** CPU canvas when `--no-gpu` is given, the setting asks for it, no wgpu render state exists (headless, kittest), the adapter is a software adapter other than DX12 WARP, or the GPU canvas fails to initialise or loses its device; GPU canvas otherwise, WARP included. A host with no adapter at all cannot open the window: `elektrawatt` exits 3 with a message naming `--headless`, `ew` and, on Linux, Mesa's software drivers | It follows NFR-PLAT-03's order (WARP, then the CPU renderer); egui's own chrome needs an adapter, so the CPU canvas helps where an adapter exists but is unusable for the canvas, not where none exists | ID-507 |
| D4 | **Determinism.** The CPU path is the golden reference and stays byte-identical per build target; the canvas snapshot (`canvas.snapshot`) is always a CPU image. The GPU path is excluded from golden comparisons; a render-to-texture parity test compares it with the CPU canvas under a one-pixel-neighbourhood tolerance (§8) | MSAA and analytic coverage never agree pixel for pixel, and the GPU clamps hairlines to one pixel by design; a neighbourhood test still catches missing, misplaced or miscoloured geometry | ID-508 |
| D5 | **Instrumentation.** A frame-statistics ring buffer (1,024 frames, p50, p99 and maximum per phase), an engine-statistics hook asserted as exact counts on every PR (pan and zoom re-execute no query), and a `canvas` bench that the nightly gate reports and never gates until a milestone run blesses its CPU families | Timings are runner-dependent and gated only on BM at milestones (doc 18 §4.3, the testing policy); counts are exact anywhere | ID-509 |
| D6 | **Boundaries.** `ew-render2d` gets the workspace edge to `ew-render-cpu` (it owns both backends); four public items are added below the shell (§4.2); `ew-ui` adds `egui-wgpu` as a direct dependency in part 2 | The canvas renderer owning both backends keeps `ew-ui` a host; each addition is a read accessor or a re-export of a type already in a public signature | ID-510 |
| D7 | **The canvas in the headless mode and `ui.describe`.** New registry actions, a `canvas` member at the end of the transcript state, and `canvas` lines in `ui.describe` with eleven new vocabulary words (ID-474 extended) | What the headless mode cannot do, the shell does not do (ID-443); the blind drafter must be able to zoom, pan, point and read back without a screenshot | ID-511 |

## 4. Crates and boundaries

### 4.1 Roles

| Crate | Layer | Content after part 1 | After part 2 | Workspace edges |
|---|---|---|---|---|
| `ew-viewmodel` | GPL | `canvas`: the toolkit-neutral canvas model in `ShellModel` (viewport, zoom ladder, fit, pan clamp, pointer, hover pick, grid state, backend label, statistics toggle); the new actions, status fields, transcript member and describe lines (§6) | The `canvas_backend` setting | `ew-api` only (unchanged) |
| `ew-render2d` | GPL | The canvas renderer, toolkit-neutral (no egui): `view` (the view transform in physical pixels), `tiles` (level choice, tile planning, the LRU cache policy), `pool` (the raster worker pool over `ew-render-cpu`), `overlay` (page bounds, grid dots, highlight outlines as screen-space primitives, and their compositing into an `RgbaImage` for the headless snapshot), `stats` (frame statistics and the one clock) | `gpu`: the wgpu renderer (§7), `pub use wgpu;`, the adapter classification and the switch rule as a pure function | `ew-dl`, **`ew-render-cpu` (new edge)** |
| `ew-render-cpu` | GPL | Adds `render_region` and `ItemBounds` (§4.2); `render` unchanged | — | `ew-dl` (unchanged) |
| `ew-dl` | permissive | Adds `DisplayList::entity_bounds` (§4.2) | — | unchanged |
| `ew-api` | GPL | Adds `Session::page_grid` and `output::PageGrid`, and re-exports `ew_dl` from `output` (§4.2) | — | unchanged |
| `ew-ui` | GPL | `canvas`: the egui host of the canvas (input mapping, texture upload, painting, the AccessKit node, the statistics overlay), replacing `page_view` in the project view; the headless `canvas.snapshot`; the `canvas` bench | The GPU host: the native texture, the backend selection at start, the failure banner | unchanged (`ew-viewmodel`, `ew-render2d`, `ew-render-cpu`, `ew-render3d`) |
| `elektrawatt` | GPL | Unchanged except the tests | `--no-gpu`, the adapter selector, exit code 3 | unchanged |

**Rules the lanes keep** (WP-S2.01's §3 rules stand; decision log ID-444, ID-510):
1. **Through `Session` only.** `ew-viewmodel` and `ew-ui` reach the model only through `Session` methods; no
   `ew-model` name, no `raw-read`; `cargo xtask check-raw-read` stays green.
2. **One new workspace edge**, `ew-render2d → ew-render-cpu`, in `ci/dep-rules.toml` (`"ew-render2d" = ["ew-dl",
   "ew-render-cpu"]`); no other edge. Display-list types are named through `ew_api::output::ew_dl` in `ew-viewmodel`
   and `ew-ui` (architecture §3 rule 7).
3. **Only the public items of §4.2** are added below the shell; the lane writes them first, in their own commit, with
   tests. Any other missing item stops the lane.
4. **Toolkit isolation.** `ew-render2d` names no egui type; only `ew-ui` and `elektrawatt` name egui, eframe and
   egui-wgpu; only `ew-render2d` depends on wgpu directly (the `[[external_owner]]` rules of `ci/dep-rules.toml`).
   `ew-ui` names wgpu types only as `ew_render2d::wgpu::…`.
5. **No `unsafe`.** `#![forbid(unsafe_code)]` in every crate root stays; vertex data is written with
   `to_le_bytes` into `Vec<u8>` (all supported targets are little-endian), so no `bytemuck` derive is used.
6. **No unordered output.** Transcripts, describe text, tile queues and caches iterate `Vec`, `BTreeMap` or
   `BTreeSet`; the cache's LRU order is a frame counter, never a hash order.
7. **One clock.** Frame timing reads `std::time::Instant` in one function, `ew_render2d::stats::now`, allowed with
   `#[allow(clippy::disallowed_methods)]` and a justification line, and listed in `ci/disallowed-methods-allow.toml`;
   no time value reaches a transcript, a describe line or a file.

### 4.2 The public items added below the shell

| Crate | Item | Contract |
|---|---|---|
| `ew-render-cpu` | `pub struct RegionOptions { pub dpi: u32, pub x: i64, pub y: i64, pub width: u32, pub height: u32, pub background: Color, pub surround: Color }` and `pub fn render_region(list: &DisplayList, bounds: Option<&ItemBounds>, sheet: SheetSize, options: &RegionOptions, fonts: &dyn FontResolver, geometry: Option<&dyn GeometryResolver>) -> Result<(RgbaImage, RenderReport), RenderError>` | Output pixel `(i, j)` is pixel `(x + i, y + j)` of the full-sheet raster at `dpi` where that pixel lies inside `pixel_size(sheet, dpi)`, and `surround` elsewhere; inside the sheet the background is `background`. `width` and `height` in `1..=4096`, else `RenderError::Size`. The full rectangle at `x = y = 0` gives the bytes of `render` exactly. Same single-threaded `Level::baseline()` pipeline as `render` |
| `ew-render-cpu` | `pub struct ItemBounds` with `pub fn of(list: &DisplayList, geometry: Option<&dyn GeometryResolver>) -> ItemBounds` | The bounds of each top-level item, inflated by its reach: half its widest stroke times the miter limit (a miter join reaches that far) and twice the em size of a glyph run (outlines reach beyond their layout box), plus 2 device pixels at the region's dpi when used; `render_region` skips a top-level item whose inflated bounds miss the region. Culling never changes a pixel (A3). *As built (ruling 4, ID-518): the reach is wider than this row first said (half the stroke plus 2 pixels), so that no join or glyph outline can be culled* |
| `ew-dl` | `pub fn DisplayList::entity_bounds(&self) -> Vec<(EntityRef, Rect)>` | For every item at any depth whose `entity` is set: its control-point bounds under the enclosing transforms, by the rules of `DisplayList::bounds`; in paint order (depth first, pre-order); items without a point are left out |
| `ew-api` | `pub use ew_dl;` in `ew_api::output`; `pub struct PageGrid { pub num_nm: i64, pub den: i64, pub sheet_scale_ppb: u32, pub sheet_offset_nm: (i64, i64) }` in `output`; `pub fn Session::page_grid(&self, folio: &str) -> Option<PageGrid>` | `PageGrid` is the page's connection grid `Page.grid` (ADR-0005, ADR-0027) as a rational pitch in nm, with the page-to-sheet map of the page's frame: the scale in parts per 10⁹ (`1_000_000_000` without a map) and where the page origin lands on the sheet (`(0, 0)` without a map); `None` when no page has that printed folio label. *As built (ruling 1, ID-518): the map was added because a QET frame scales and shifts the page onto the sheet, and without it the grid dots drift off the connection points; the overlay draws the dots at `offset + scale × k × pitch`* |

## 5. Dependencies and the supply chain

**Part 1 adds no external crate.** `ew-ui` adds `tracing = { version = "0.1.44", default-features = false, features =
["std"] }` (already in the graph through `ew-memo`) for the frame spans of §9; `Cargo.lock` gains no package. The lane
checks this with `cargo tree -p elektrawatt` before and after (the package count is unchanged) and says so in the
commit message.

**Part 2** (decision log ID-510):

| Crate | Dependency | Spec | New packages | Why |
|---|---|---|---|---|
| `ew-render2d` | `wgpu` | `=30.0.1` (the version egui-wgpu 0.37.0 locks), `default-features = false`, `features = ["wgsl"]`, plus per target `dx12` (Windows), `metal` (macOS), `vulkan` and `gles` (Linux; `gles` keeps `wayland-sys/dlopen`, the `required_features` rule) | none | The renderer; one wgpu instance in the graph, so its types are egui-wgpu's |
| `ew-render2d` | `lyon_tessellation` | `=1.0.22` (architecture §3), default features | expected: `lyon_tessellation`, `lyon_path`, `lyon_geom`, `float_next_after` (arrayvec, euclid and num-traits are locked already) | Fill and stroke tessellation |
| `ew-render2d` | `kurbo` | `0.13.1` (locked, through `ew-dl`) | none | Dashes and flattening before tessellation |
| `ew-render2d` | `skrifa` | `0.44.0` (locked) | none | Glyph outlines, unhinted, from the bundled fonts only |
| `ew-render2d` | `pollster` | the version already in the `elektrawatt` graph (`cargo tree -i pollster`) | none | Blocking on adapter and device requests in the headless device of tests and the bench |
| `ew-ui` | `egui-wgpu` | `=0.37.0`, `default-features = false` (locked through eframe) | none | Registering the canvas texture with the renderer of eframe's render state |

**What part 2 adds to the policy files, in one `build(deps)` commit:** `Cargo.lock`; one `[[exemptions.<crate>]]` per
new package and version in `supply-chain/config.toml`, `safe-to-deploy` (all are in the shipped graph), listed in the
commit message, with no `[[audits]]` entry (audits are the lead's, ID-4, ID-332); `ci/build-scripts.allowlist` for any
new build script after reading it; no `deny.toml` change (lyon is MIT OR Apache-2.0; any other licence stops the lane).
Checks: `cargo deny check`, `cargo vet --locked`, `cargo xtask check-deps`, `check-features`, `check-build-scripts`,
`check-licence-map`, `python -m reuse lint`. ID-476's distribution hold is unaffected; the lyon crates join the audit
list of FT-326.

**FT-328 is part 2's.** The wgpu backend features of the application are trimmed per operating system (DX12 with WARP
on Windows, Metal on macOS, Vulkan and GL on Linux), the release binary size and the clean build time of `elektrawatt`
are measured before and after and reported, and `check-features` stays green.

*As built (part 2, cycle 22; decision log ID-539, ID-543).* **FT-328 is closed.** The trim is eframe's
`wgpu_no_default_features` with the backends named per operating system (DX12 and WARP on Windows, Metal on macOS,
Vulkan and GL on Linux with the guard of ID-476). The Windows release binary measured 53,900,288 B before part 2,
54,407,168 B with the GPU path and every backend, and 52,502,528 B after the trim (-1.8 MiB); the clean build time is
not measured (owner's rule). **Supply chain:** `lyon_tessellation` 1.0.22, `lyon_path` 1.0.19, `lyon_geom` 1.0.20 (MIT
OR Apache-2.0) and `float_next_after` 1.0.0 (MIT) entered the graph, none with a build script, the allowlist unchanged.
They are four dated `safe-to-deploy` exemptions with no audit written, which is the form doc 18 §5.4 and ID-332 ask for
a lane; the audits that burn them down are FT-326's (the lyon family joins it) and come before any distributed binary
(ID-476's hold). `termcolor` and `wgpu-core-deps-wasm` left the graph, so the exemptions went from 384 to 387.

## 6. Part 1: the canvas on the CPU tile path

### 6.1 The canvas model (`ew-viewmodel`)

`ShellModel` gains a `canvas` field of a plain-Rust `CanvasModel`, present whenever a page is shown:

| Field | Type | Meaning |
|---|---|---|
| `sheet` | `(i64, i64)` nm | The page's width and height (from `page_content`) |
| `centre` | `(i64, i64)` nm | The sheet point at the pane's centre; clamped to the sheet rectangle `[0, w] × [0, h]` |
| `zoom` | `f64` | The factor of paper size (1.0 = 100 %); 100 % shows 1 mm of paper as 96/25.4 logical points (CSS convention), so the page keeps its physical size across monitors (doc 16 §17 item 7). *Ruling (ID-521):* the points meant here exclude the UI scale: 100 % is 96/25.4 physical pixels per paper millimetre times the operating system's scale factor, and the UI scale enlarges only the chrome. Part 1 as built used egui's pixels per point, which include the UI scale; part 2 divides by it (FT-349, *as built in cycle 22:* `D = zoom × 96 × ppp / ui_scale`, 1.0 headless, so no transcript or golden changed) |
| `pane` | `(f64, f64)` points, `ppp: f64` | The pane size and pixels per point; headless default 1200 × 800 at 1.0 |
| `pointer` | `Option<(i64, i64)>` nm | The sheet point under the pointer, `None` when the pointer is outside the pane |
| `grid` | `Option<PageGrid>`, `grid_on: bool` | The page's grid from `Session::page_grid`, and whether it is drawn (default on) |
| `hover` | `Option<(Uuid, Rect)>` | The hover pick (below), recomputed when the pointer, zoom or scene changes |
| `backend` | `CanvasBackend` | `Cpu`, `Warp` or `Gpu(<backend name>)`; set by the host (`ShellModel::set_canvas_backend`, the one host-state setter: the backend is a fact about the host, not a user action), always `Cpu` headless |
| `statistics` | `bool` | The frame-statistics overlay (§9) |

**Zoom ladder** (doc 06 §6): the values `2^(k/2)` for `k = −8 … 11` (6.25 % to 4525.5 %), with 5 % and 5000 % as the
clamp ends; odd `k` is computed as `SQRT_2 × 2^((k−1)/2)` by multiplication of constants, never `powf`. Zoom in goes to
the smallest ladder value above `zoom × (1 + 1e-9)`, else 5000 %; zoom out to the largest below `zoom × (1 − 1e-9)`,
else 5 %. Continuous zoom (pinch, `view.zoom_to`) may leave the ladder; every zoom is clamped to `[0.05, 50]`.

**Fit page** (Home, also on every page change): `zoom = min((pane_w − 32) / sheet_w_pt, (pane_h − 32) / sheet_h_pt)`
clamped, `centre` = the sheet's centre (integer halves, toward zero). Per-page viewports are WP-S2.03's.

**Zoom about an anchor** `a` (sheet nm): `centre' = a − (a − centre) × zoom / zoom'`, each axis rounded toward zero to
nm, then clamped; while the clamp is inactive the anchor keeps its screen position to within 1 nm. **Pan** by
`(dx, dy)` points moves the centre by `−d / points_per_nm`, rounded toward zero, then clamped. **Resize** keeps the
centre and the zoom; a change of `ppp` keeps the zoom (paper units). *As built (ruling 2, ID-518):* a **fitted** view
(after Home, a page change or the first frame) refits on every resize until the first zoom or pan, so a window that is
resized right after opening still shows the whole page; after a zoom or pan, resize keeps the centre and the zoom.

**Hover pick** (placeholder until WP-S2.03's `page_spatial_index`): from `entity_bounds` of the page's display list,
computed once per (revision, folio) and kept in the model; the entries whose rectangle, inflated by 4 points at the
current zoom, contains the pointer; the one with the smallest uninflated area (`i128`) wins, ties to the later in paint
order.

### 6.2 Actions and input

New registry actions (View menu unless hidden; label keys and English texts in `ew_viewmodel::strings`):

| ID | Menu | Shortcut | Enabled when | Params (headless) |
|---|---|---|---|---|
| `view.zoom_in` | View | Ctrl+Plus, Ctrl+Equals | a page is shown and zoom < 5000 % (`reason-zoom-max`) | — |
| `view.zoom_out` | View | Ctrl+Minus | a page is shown and zoom > 5 % (`reason-zoom-min`) | — |
| `view.zoom_actual` | View | Ctrl+0 | a page is shown | — (100 %) |
| `view.fit_page` | View | Home | a page is shown (now fits for real) | — |
| `view.pan_left`, `view.pan_right`, `view.pan_up`, `view.pan_down` | View ▸ Pan | Ctrl+Arrow Left, Right, Up, Down | a page is shown | — (one eighth of the pane) |
| `view.grid` | View | — | a page is shown; checked while drawn | — |
| `view.frame_stats` | View | — | always; checked while shown | — |
| `view.zoom_to` | hidden | — | a page is shown | `percent`, optional `x_mm`, `y_mm` (the anchor; default the centre) |
| `view.pan` | hidden | — | a page is shown | `dx`, `dy` (points) |
| `view.canvas_resize` | hidden | — | always | `width`, `height` (points), `ppp`; at most `MAX_PANE_POINTS` (16,384) points and `MAX_PANE_PIXELS` (8,192) physical pixels per side and `ppp` ≤ `MAX_PPP` (8), else `error-bad-param` (fix round, ID-519) |
| `canvas.pointer` | hidden | — | a page is shown | `x_mm`, `y_mm`, both or neither (neither: the pointer left) |
| `canvas.snapshot` | headless only | — | a page is shown | `path` |

The reason key for "no page shown" is the existing one of `page.first` if it fits, else a new `reason-no-page-shown`.
egui's own keyboard zoom (`zoom_with_keyboard`) is switched off, so Ctrl+Plus, Ctrl+Minus and Ctrl+0 reach the canvas
and never the UI scale (which stays `view.ui_scale`).

*As built (ruling 3, ID-518; review fix 5, ID-519).* Dispatching an action normally clears the banner; the canvas's
view actions (the zooms, `view.fit_page`, the pans, `view.grid`, `view.frame_stats`, `view.zoom_to`, `view.pan`,
`view.canvas_resize` and `canvas.pointer`) leave it standing, because moving the pointer or resizing the window must
not dismiss what the banner says. The window host bounds its own resize request with the same limits
(`bounded_pane`), so an oversized window never makes a failing resize every frame.

**Pointer input** (mapped by `ew-ui` onto the actions above, so the GUI and a script take the same path):

| Input | Effect |
|---|---|
| Mouse wheel in line or page units | One ladder step per whole line (accumulated, sign of y) around the pointer (`view.zoom_to` with the anchor) |
| Scroll in point units (trackpads) | Pan by the delta (`view.pan`) |
| Pinch or egui's zoom delta | Continuous zoom by the factor around the pointer |
| Middle-button drag; Space held with a left-button drag | Pan by the drag delta |
| Pointer move over the pane; pointer leaving it | `canvas.pointer` with the sheet point; without parameters |
| Click | Nothing (no selection, §1) |

No zoom is animated (doc 16 §10.4's reduce motion is the only behaviour in this package). Keyboard routing is WP-S2.01's
(a focused text field, then a popup or dialog, then the shortcuts).

### 6.3 The CPU tile path (`ew-render2d` over `ew-render-cpu`)

**Scene.** For the current (revision, folio) the host keeps one `Arc<ExportPages>` from `ShellModel::page_content`
(fetched on the UI thread, once per key; FT-346 measures whether a cold fetch can exceed NFR-PERF-19) and one
`ItemBounds` built on a worker. A new revision or folio is a new scene; the previous scene's tiles stay visible until
the new ones arrive for the same folio, and are never mixed with them in one frame for a different folio.

**Levels.** The display resolution is `D = zoom × 96 × ppp` dpi. Pyramid levels are `L_k = 6 × 2^k` dpi for
`k = 0 … 12` (6 to 24,576 dpi); the target level is the smallest `L_k ≥ D`, or `L_12` above it, so a level tile is
only ever scaled down, by at most 2. When the view has not changed for 150 ms the planner is **settled**: it wants the
exact resolution `round(D)` instead, and draws with the view origin rounded to whole physical pixels, so an idle canvas
is pixel-exact; level tiles stay cached as the fallback.

**Tiles.** 256 × 256 physical pixels of the full-sheet raster at a resolution, keyed by `(scene, dpi, tx, ty)`, rendered
by `render_region` with the scene's `ItemBounds`. Each frame the planner returns, in priority order: (1) the tiles of the
preview level `max(0, k − 3)` covering the pane (few and fast, so something always shows), (2) the visible tiles at the
wanted resolution, centre out (by the squared distance of the tile centre to the pane centre, ties by `(ty, tx)`), (3)
the ring of tiles around the visible set. Drawing paints the preview tiles first and finer tiles over them; a tile's
screen rectangle has edges rounded to physical pixels so neighbours share their edges (no seams).

**Pool and cache.** `clamp(available_parallelism − 1, 1, 4)` worker threads; a job not in the newest plan is dropped
before it starts (a generation counter); a running job finishes and is cached if its scene is current. At most 8 tile
uploads per frame (2 MiB); a pending tile requests a repaint when it lands. The cache holds at most 256 MiB of tile
textures, evicts least recently used by frame counter, and never evicts a tile drawn in the current frame.

*As built (the review's fix round, ID-519).* The pool marks a tile delivered before it sends it, and a submission skips
delivered tiles under the same lock, so a tile that finishes between the host's receive and its next submission is
never queued again; the host counts `skipped_cached`, `requested_while_cached` and `rastered_while_cached` (A9). A
previous-revision fallback tile on the last layer leaves the canvas not ready (`is_ready()` and `page_view_ready()`
false) until the new revision's tiles are in, and counts a fallback frame in the frame statistics. With no canvas on
screen (no project, the wizard, or the pane hidden behind another tab) the host submits an empty plan and drops the
cached scenes, so the pool does no work and no textures are held; a hidden pane's tiles are rastered again when its tab
returns (a budgeted retention is FT-356, *delivered with part 2 in cycle 22:* a hidden tab keeps its tiles within a
quarter of the cache and frees its GPU targets; WP-S2.03 part 1 extends that budget to its tabs, ID-535). `TilePool::recv_timeout` (50 ms) is the blocking receive the bench uses. A
commit on another page makes a new scene, because the scene key is the revision; keying it by a digest of the page's
content is part of FT-347. *As built (the review's fix round of part 2, ID-542):* the scene key is (session, revision,
folio), not (revision, folio): the revision counter restarts with every `Project`, so two projects could share a key. The
session is a per-shell epoch bumped at each of the three places that create a `Project` (a reload is a new session),
carried in `CanvasPage::session`, and used by the shell's canvas and error keys, the keep-the-view rule, the CPU tile
scene with its stand-in, and the GPU request, stand-in and accept. `CanvasHost::release` clears the frame, the request,
the shown scene and the canvas's scene, so a project switch can never show the previous project's page as ready; a seam
test pins both.

**Wizard preview.** The wizard's live preview keeps WP-S2.01's fitted image (ID-445); `page_view.rs` remains for it
only, and `view.snapshot` and `--smoke` keep their full-page path unchanged.

### 6.4 Overlays: page bounds, grid and highlight

`ew-render2d::overlay` turns the view into screen-space primitives in physical pixels, which `ew-ui` paints as egui
shapes over the tiles (one mesh for the grid dots) and the headless snapshot composites with integer pixel fills:

| Overlay | Rule |
|---|---|
| Surround | Outside the sheet, the theme's `canvas_surround` |
| Page bounds | A 1-pixel outline of the sheet rectangle in the theme's `border` |
| Grid | Dots at the page grid's points inside the sheet, 1 physical pixel (2 at `ppp ≥ 2`), in `grid_dot`; when the spacing is below 8 points the pitch doubles until it is not, so every dot stays on a grid point; none when `grid_on` is off or the page has no grid |
| Hover | The hovered entity's rectangle outset by 2 points, 1-pixel dashed outline (4 on, 4 off) in `hover` #0072B2 (doc 16 §10.3) |
| Selected (API only) | 2-pixel solid outline in `selection` #0072B2; drawn when the caller passes rectangles, which nothing does in this package |

The theme tokens of these overlays are constants in one module of `ew-render2d` for the light and dark themes
(provisional values: surround #D4D4D4 and #2B2B2B, border #8C8C8C and #6E6E6E, grid dot #9E9E9E in both); WP-S2.08
replaces them with the theme system. The sheet is white with its authored colours in every theme (FR-UI-043, doc 16
§11.2). The frame-statistics overlay (§9) is text in the canvas's top-right corner.

### 6.5 Status bar and accessibility

**Status bar** (doc 16 §3.6; left to right): the hint, the page, **the cursor** `x 255.0 y 172.5 mm` (sheet
coordinates, origin top-left, y down, one decimal; left out while the pointer is outside the pane; the cell is FT-345),
**the zoom** `156 %` (an integer at 10 % and above, one decimal below), **the grid** (`QET 10 px` for the grid
`7_937_500/3` nm, otherwise the pitch in mm with at most three decimals, such as `4 mm`; `Grid off` when off), the save
state, **the backend** (`CPU`, `WARP` or `GPU <backend>` such as `GPU Vulkan`), the language, the export label.

**AccessKit** (doc 16 §10.1; NFR-A11Y-02): the canvas is one focusable node with role `Canvas`, the name
`Page <folio>, <page title>` and the description `<hint>, zoom <zoom>`; the accessible-name walker passes with it.
Primitives are not exposed (the navigators are the accessible representation, FR-UI-026).

### 6.6 The headless mode

**Transcript.** The step `state` gains, after `status`, the member `canvas`: `null` while no page is shown, else an
object with members in this order: `folio` (string), `zoom` (percent rounded to 0.1, a number), `view_nm` (the sheet
rectangle the pane shows, `[x0, y0, x1, y1]`, integers, may extend beyond the sheet), `grid` (`[num_nm, den]` or
`null` when off or absent), `pointer_nm` (`[x, y]` or `null`), `hover` (the entity's UUID as a string, or `null`),
`backend` (the label), `statistics` (boolean). No time value appears. This is a reviewed transcript change: every
expected state in the existing headless tests gains the member.

**`ui.describe`** gains these lines after the page-view line, in this order (millimetres with three decimals from
integer nm, the zoom with one decimal):

```text
canvas zoom 156.3 % view 0.000 0.000 297.000 210.000 mm
canvas grid on "QET 10 px"                                   (or: canvas grid off)
canvas pointer 255.000 172.500 mm                            (or: canvas pointer none)
canvas hover "<uuid>" box 250.100 170.000 260.300 175.000 mm (or: canvas hover none)
canvas backend "CPU"
canvas statistics shown                                      (only while the overlay is on)
```

*As built since cycle 26 (FT-422, decision log ID-630):* the hovered entity is printed as its alias, unquoted
(`canvas hover @4 box …`), and the transcript's `hover` member holds the alias (`"@4"`), never the UUID.

`VOCABULARY` gains `canvas`, `zoom`, `view`, `grid`, `pointer`, `hover`, `box`, `mm`, `none`, `backend` and
`statistics` (ID-474, ID-493: a reviewed vocabulary change; the grid label and the backend label are quoted user text,
so backend names never enter the vocabulary). FT-314's harness help lists them.

**`canvas.snapshot`** writes the pane as the canvas shows it, at `pane × ppp` pixels: `render_region` at
`dpi = round(D)` with the view origin rounded to whole pixels, `background` white and `surround` from the theme of the
settings (light when the theme is `system`), then the overlays of §6.4 composited by `ew-render2d`; encoded with
`encode_png(&image, Some(dpi))`. It never uses tiles or a GPU, so it is the same bytes on every run of one build
target. `view.snapshot` (the whole page at a dpi) is unchanged. *As built (ruling 6, ID-518):* a pane larger than 4096
pixels on a side is rendered as `render_region` parts of at most 4096 pixels and stitched; with the bounds of
`view.canvas_resize` (§6.2) a snapshot is at most 8,192 × 8,192 pixels (256 MiB).

## 7. Part 2: the GPU path and the switch rule

### 7.1 The renderer (`ew_render2d::gpu`)

- **Scene preparation on a worker** (no UI-thread tessellation): the display list is walked once; **every group with
  a transform or a clip is a mesh context** (ruling of the part-2 lane, accepted by the review, ID-540): an instance of
  a mesh keyed by a 128-bit content key (two SipHash passes of the group's items with distinct prefixes; no equality
  check); every glyph is an instance of a mesh keyed by (font key, glyph ID),
  tessellated from its skrifa outline in font units with a tolerance of em/2000 (ID-540); the remaining paths form the
  page's static mesh. Meshes are cached
  across scenes, so a second preparation of the same page tessellates nothing (B3). While a scene prepares, the
  canvas shows the CPU tiles of §6.3 (the loading state). *As built:* the scene is keyed (session, revision, folio)
  (§6.3); the previous revision of the same folio stands in while the new one prepares, and the canvas is not ready
  until the new scene is (consistent with part 1's readiness rule). Content-keyed meshes are what part 2 delivers of
  FT-347: re-preparing after a commit elsewhere tessellates nothing.
- **Coordinates.** Page coordinates as `f32` micrometres (exact to 0.06 µm up to 1 m); mesh-local coordinates relative
  to the mesh origin.
- **Fills** with lyon's fill tessellator, non-zero rule, tolerance 1 µm. **Strokes** with lyon's stroke tessellator at
  their width, caps, joins and the miter limit of `ew-dl`, dashes split first with kurbo; each vertex carries its
  normal scaled by the half-width, and the vertex shader widens any stroke to at least 1 physical pixel
  (architecture §8.2's screen-space lines).
- **Paint order** by depth: every drawn item gets its depth from its paint index (later is nearer); opaque items are
  batched in any order with a depth test; items whose brush alpha is below 255 are drawn last, in paint order, with the
  depth test and without depth writes. *Ruling (ID-541):* translucent primitives are **exact in paint order between
  instances and approximate inside one mesh**, where a mesh's own primitives keep the mesh's order; §8 item 3 bounds
  the difference.
- **LOD**: a glyph run whose em height on screen is below 2 pixels is drawn as its layout box filled with its brush at
  alpha 0.35 (the "bar"); images the CPU path skips (no bytes, SVG, undecodable) are skipped and counted (*as built,
  cycle 30*; before FT-480 every image was skipped); a group clip becomes a per-instance clip
  rectangle in page coordinates tested in the fragment shader (exact for multiples of 90°, the bounding box otherwise).
- **Raster images** (*as built, cycle 30, FT-480; decision log ID-702*). Each image item takes one paint index. The
  preparation worker decodes PNG and JPEG with `ew_render_cpu::decode_image`, premultiplies the texels and caches them
  by SHA-256 in the `MeshCache` under its budget. When a side exceeds the device's `max_texture_dimension_2d`, it
  downscales by an area average: the longer side becomes the limit, the shorter `round(shorter × limit / longer)`, at
  least 1. A drawn image is the unit square over its rectangle with the context's clip, drawn by a second pair of
  pipelines over the same vertex and instance layout, view uniform, MSAA and depth format. Group 1 holds the texture
  and a bilinear clamp-to-edge sampler without mipmaps, which is the CPU path's `ImageSampler::default()`. Texels are
  premultiplied `Rgba8Unorm` and blend as premultiplied colour. Opaque images go in the opaque pass; images with alpha
  go in the translucent pass in paint order. Textures are cached by SHA-256 within `GPU_TEXTURE_BYTES` (256 MiB), least
  recently used first and never the current scene's; an evicted texture is uploaded again on demand, and one beyond the
  device limit is refused and counted, not a fault. Parity against the CPU raster: mean channel difference 0.27 (bound
  0.35), no unmatched ink. Since FT-480 every page is drawn by the switch rule's backend (§7.2); the CPU routing of
  raster pages of cycle 29 (`has_raster_images`, `GpuHost::draws_page`) is removed.
- **Target.** One WGSL file. The canvas renders into its own 4× multisampled colour and depth attachments, resolved
  into a texture of the pane's physical size, which `ew-ui` registers once per size as an egui native texture through
  egui-wgpu's renderer and paints as an image under the overlays of §6.4 (the overlays stay one code path for both
  backends). The texture format is the one at which a filled rectangle reads back its authored sRGB bytes within ±1
  through egui-wgpu's sampling (B2). *As built:* `Rgba8Unorm`; egui-wgpu requires it for native textures and samples it
  as gamma-space colour, so authored sRGB bytes blend in gamma space like the CPU u8 pipeline (#0072B2 reads back
  within ±1); the filter is nearest, one texel per physical pixel.
- **Frame.** In `App::update`, before egui paints: when the view or the scene changed, encode and submit one render;
  idle frames submit nothing (repaint on demand, doc 16 §1 rule 3).
- **Mapped buffers** (from the cycle-21 audit of wgpu 30.0.1): wgpu-core 30.0.1's safe `Buffer::destroy` and device
  loss release a buffer without checking for mapped views still alive, so a view kept across `unmap`, `destroy` or a
  device loss reads or writes freed memory. **No mapped view may outlive `unmap`, `destroy` or device loss**: a mapped
  range is only ever held inside the scope that maps it (the map callback or the block that awaits it, as egui-wgpu
  does), copied out there, and dropped before `unmap`; this applies to the readback buffers of the render-to-texture
  tests and the bench as well as to any upload buffer. The renderer uses `Queue::write_buffer` and
  `Queue::write_texture` for uploads, so it maps no buffer outside the readback path. *As built:* the only mapped view
  is in `GpuCanvas::read_back`, held inside the scope that maps it.
- **Errors and limits** (*as built*). The GPU canvas records a device loss or an uncaptured error instead of wgpu's
  default panic. `GpuError::Limits` (a pane larger than the device allows) ends the GPU path for the session with the
  banner (ID-544); a per-frame CPU fallback, with `GpuHost::render` returning `None` without recording a fault, is
  FT-363. A lyon panic is caught with `catch_unwind` and meets FT-310's panic hook (a note on FT-310, ID-544). The
  handlers installed on eframe's shared device outlive the canvas, so they log through `tracing` and never swallow
  egui-wgpu's own errors (ID-542).

### 7.2 The switch rule

Evaluated at start and on a failure, in this order; the result is set with `ShellModel::set_canvas_backend`:

| # | Condition | Canvas | Status label |
|---|---|---|---|
| 1 | `--no-gpu` | CPU; eframe's adapter selector picks the first adapter whose device type is CPU (none: the default adapter, with a banner) | `CPU` |
| 2 | Setting `canvas_backend = "cpu"` (View ▸ Use the CPU renderer for the canvas, a checked item, effective at once) | CPU | `CPU` |
| 3 | No wgpu render state (headless, kittest) | CPU | `CPU` |
| 4 | Adapter device type discrete, integrated, virtual or other | GPU | `GPU <backend>` |
| 5 | Adapter device type CPU on DX12 (WARP) | GPU | `WARP` |
| 6 | Adapter device type CPU on another backend (llvmpipe, lavapipe and the like) | CPU | `CPU` |
| 7 | GPU initialisation fails (pipeline creation or a validation error caught in an error scope) | CPU, with a one-time banner naming the error | `CPU` |
| 8 | Device lost or an uncaptured error after start | CPU for the rest of the session, with a one-time banner. *Note (ID-544):* there is one shared device (ADR-0014), so a real device loss also ends egui's window; this row helps for uncaptured errors only, and recovery is FT-310 and WP-S2.11 | `CPU` |

The rule is one pure function `ew_render2d::gpu::choose(flags, setting, adapter: Option<AdapterSummary>, init:
Result<(), GpuError>) -> CanvasBackend`, table-tested (B1). A host with no adapter at all fails in eframe before the
first frame: `elektrawatt` catches the error, writes one message to stderr (it names `elektrawatt --headless`, `ew`,
and on Linux the Mesa software drivers) and exits with code 3; the headless and smoke modes never need an adapter.

*As built (part 2, cycle 22; decision log ID-539, ID-542).* The status labels are `GPU DX12`, `GPU Vulkan`, `GPU Metal`,
`GPU GL`, `WARP` and `CPU`; the setting is `canvas_backend` (`auto` by default, or `cpu`; `settings.set` accepts it);
`--no-gpu` gives the CPU canvas and a software adapter for the window (none: the default adapter, with a banner); an
initialisation failure shows the one banner "The GPU canvas could not start (…)". **The exit-3 rule of the review's fix
round:** exit code 3 follows an adapter-request or `--no-gpu` selector failure, or any other wgpu error when a fresh
instance's enumeration counts no adapter (otherwise 1, as for every non-wgpu error). A `create_surface` failure with no
Vulkan loader or EGL had exited 1; the fresh instance skips the backends it cannot start instead of panicking. **A
recorded gap (FT-364):** the count honours `WGPU_BACKEND` while eframe's own instance does not.

## 8. Determinism

1. **The CPU path is the reference.** `render` is unchanged; the golden PNG tests of `crates/ew-api/tests/page_output`
   and the headless `view.snapshot` test keep their bytes. `render_region` of the full rectangle equals `render` byte
   for byte (A3).
2. **The CPU canvas** is byte-deterministic per build target: `canvas.snapshot` on equal inputs gives equal bytes (A7),
   and its sheet area matches the matching crop of the 150 dpi golden PNG within doc 20's tolerance (at most 0.5 % of
   pixels differing by more than 16) when the zoom gives `D = 150` (A6). Tiles stitched at one resolution match `render`
   at that resolution within the same tolerance (A3); they are not required to be byte-identical, because a tile's
   coordinates are rounded at a finer magnitude than the full raster's.
3. **The GPU path is excluded from golden comparisons.** It is checked against the CPU canvas by a render-to-texture
   parity test (B2): a "clearly ink" pixel (some channel at least 128 away from white) of one image is matched when a
   pixel of the other image within its 3 × 3 neighbourhood has every channel within 128 of it; at most 1 % of the
   clearly-ink pixels of each image may be unmatched. It runs on folio 103 at `D = 150` and on the dense A3 page of §9.
   *As built (the review's fix round, ID-542, ID-541):* the neighbourhood rule alone is lenient, so B2 adds a
   **mean-channel-difference bound** over the whole image (folio 103: measured 0.60, bound 0.75; dense A3: measured
   6.36, bound 8.0) and an **edge-position check** (an edge at 60.2 or 40.3 px lands on column 60 and row 40 as in the CPU
   raster, ±0), and samples #0072B2 at the first fully covered pixel inside each edge. The dense page is overlapping
   copies of folio 103, so its larger difference is overlap plus the approximate order of translucent primitives
   inside one mesh (§7.1); the folio-103 bound is the sensitive check, and the dense-page bound only guards against gross
   error.
4. **Where it runs.** The parity test needs an adapter: on Windows it must find one (DX12 WARP), and a Windows run
   without one stops the lane for a lead ruling; elsewhere it prints `skipped: no adapter` and passes. *As built (ID-542):*
   `EW_REQUIRE_GPU_ADAPTER=1` makes a missing adapter fail the GPU tests (a reviewed allow site of the environment lint,
   doc 18 §3.5), and the CI jobs with an adapter set it: Linux installs Mesa's lavapipe (`mesa-vulkan-drivers`, commits
   d229734 and 3012b0e) and so runs B2, Windows uses WARP, macOS Metal; the headless container job keeps
   `skipped: no adapter`, which is printed only without the flag (doc 18 §4.2, §5.2). No GPU image
   appears in a transcript, a describe text or a golden.

## 9. Frame-time instrumentation

**Phases** of one GUI frame, measured inside `App::update` through `ew_render2d::stats::now` and recorded as `tracing`
spans (`frame`, `frame.input`, `frame.scene`, `frame.render`, `frame.paint`):

| Phase | Covers |
|---|---|
| `input` | Mapping the frame's input to actions and dispatching them |
| `scene` | Fetching or reusing the page's display list and its bounds (zero on a hit) |
| `render` | Tile planning, cache lookups and texture uploads (CPU), or preparation upload, encoding and submission (GPU) |
| `paint` | Laying out the canvas pane and building the overlay shapes |
| `interval` | The time since the previous frame (frame rate); egui's own tessellation and presentation happen after `update` and are covered only by it |

**`FrameStats`** (`ew-render2d::stats`): a ring buffer of the last 1,024 frames with the phases above, the tiles
rastered and uploaded, and the memo counters' deltas (hits, re-executions) from `Session::memo()`'s total statistics,
read through a `ShellModel` getter; summaries are nearest-rank p50, p99 and maximum per phase. View ▸ Frame statistics
shows fps, the p99 of the frame and of each phase, pending tiles and the backend in the canvas corner. The budgets it is
read against are NFR-PERF-04 (frame ≤ 16 ms p99, render ≤ 5 ms) and NFR-PERF-06 (≥ 60 fps dense A3 on the GPU, ≥ 30 fps
A4 on the CPU renderer).

**Engine-statistics hook, exact counts on every PR** (NFR-PERF-20): A9 asserts that pan, zoom, pointer and resize steps
re-execute no query and fetch no display list after the first frame of a page.

**The `canvas` bench** (`crates/ew-ui/benches/canvas.rs`, `harness = false`): headless, without egui, on the same
`ew-render2d` planner, pool and (part 2) GPU renderer the GUI uses. Scenes: Golden A folio 103 (a temporary copy) and a
dense A3 page made by translating copies of folio 103's display list onto an A3 landscape sheet until at least 5,000
paths and glyph runs are drawn. Gesture script: 1,000 frames per scene (400 pan steps of 40 points, 400 zoom steps
around moving anchors that cross levels, 200 pointer moves). It prints a title line `canvas benchmarks`, a header
`class=GP generator_version=0 mode=quick|full` and the gate's `bench <family> median=… p99=…` lines for the families
`frame.gesture.pan`, `frame.gesture.zoom` (UI-thread time per frame) with per-phase families
`frame.gesture.pan.<phase>` and `frame.gesture.zoom.<phase>`, `render.pan.cpu` and `render.zoom.cpu` (time until every
visible tile of the frame's wanted resolution is cached, with the pool) and, in part 2, `render.pan.gpu` and
`render.zoom.gpu` (encode, submit and wait per frame on the adapter found; a line `skipped: no adapter` otherwise).
`cargo xtask perf-gate` runs it once per gate run (not per class); its values are report-only: new families never fail
the gate, the CPU families enter the baseline only by a bless at a milestone run on the BM runner, and the GPU families
never enter the hosted runners' baseline. Lanes build the bench and test the gate's parsing of a captured sample; they
do not run it (testing policy), and their reports say "not measured (milestone rule)".

*As built (ruling 5, ID-518; ID-512, ID-520).* The dense-A3 scene's families carry the suffix `.dense_a3`
(`frame.gesture.pan.dense_a3`, `render.pan.cpu.dense_a3` and so on), and pointer frames count into their own family
`frame.gesture.point` (with its per-phase families), not into pan or zoom. Since the owner's ruling of 2026-10-22 no
performance measurement runs during development: the first measurement and the bless of the CPU families are a
release-phase item (FT-355), and reports say "not measured (owner's rule)". The bench waits for a frame's tiles at most
120 s (`TILE_WAIT_LIMIT`); a crash or that limit running out fails the nightly `perf.yml` as an infrastructure failure,
like any bench that does not run, although the values are report-only (ID-520).

## 10. Acceptance checks

All tests are ordinary nextest tests in the existing three test jobs (CI-PR-02, -03, -04), without workflow changes,
on temporary copies of Golden A with `EW_CONFIG_DIR` set to a temporary directory.

**Part 1**

| # | Check | Where | Proves | Result (cycle 21) |
|---|---|---|---|---|
| A1 | Canvas model: the ladder values and the clamp ends; zoom in and out from an off-ladder fit; the anchor keeps its screen position within 1 nm while the clamp is inactive; fit page centres with the 16-point margins; the pan clamp; resize and `ppp` changes keep centre and zoom; the transcript numbers' formatting | `ew-viewmodel` unit tests | §6.1 | Passed (`ew-viewmodel` unit tests) |
| A2 | Registry: the new IDs with label keys and English texts; shortcuts unique, none Ctrl+Alt; hidden actions without menu entries; refusal reasons at the zoom ends | `ew-viewmodel` tests (WP-S2.01's A1 extended) | §6.2 | Passed |
| A3 | `render_region`: the full rectangle equals `render` byte for byte (a synthetic list and folio 103); a 3 × 3 stitch of 256-pixel regions at 192 dpi matches `render` at 192 dpi within doc 20's tolerance, reporting the largest channel difference; pixels outside the sheet are `surround`; rendering with and without `ItemBounds` gives equal bytes; `render`'s existing tests and the page-output goldens unchanged | `ew-render-cpu` tests; `cargo nextest run -p ew-api --test page_output` | §4.2, §8 | Passed: the full rectangle byte-equal to `render`; the stitch on folio 103 at 150 and 192 dpi within tolerance, largest channel difference 1; `page_output` goldens unchanged |
| A4 | Tile planner and cache: level choice and the top clamp; the visible set covers pane ∩ sheet exactly; centre-out order with its tie rule; the preview level; settled exact resolution after 150 ms (with an injected clock); shared edges, no gaps; LRU within the budget never evicting a tile of the current frame; stale jobs dropped by generation | `ew-render2d` unit tests (synthetic path-only lists) | §6.3 | Passed, with the fix round's test of the delivered-tile race (ID-519) |
| A5 | `entity_bounds` exact under nested transforms (0°, 90°, mirrored); on folio 103, for every entity rectangle, a pointer at its centre hovers an entity whose rectangle contains the point and whose area is not larger | `ew-dl`, `ew-viewmodel` tests | §4.2, §6.1 | Passed |
| A6 | Headless canvas through the built binary: open, `page.goto` 103, `view.canvas_resize` 1200 × 800 at 1.0, `view.zoom_to` 156.25 %, `canvas.snapshot`: the sheet area matches the crop of `golden/J-E-2416/expected/png/103.png` within doc 20's tolerance; the surround and the page bounds are where the view says; grid dots lie at the QET 10 px pitch; with the theme `dark` the sheet pixels are byte-identical to the light run and the surround differs (FR-UI-043's MVP part) | `bin/elektrawatt/tests/headless.rs` | §6.4, §6.6, §8 | Passed: the sheet crop matches the golden PNG with no differing pixel at the report's precision |
| A7 | Determinism: the canvas script twice gives byte-identical transcripts and snapshots | `bin/elektrawatt/tests/headless.rs` (`the_transcript_is_deterministic`); `canvas.rs` T7 (snapshot bytes) | §8 | Passed |
| A8 | Transcript and describe: the `canvas` member and its order; the describe lines; `canvas.pointer` at an entity's centre gives that entity in `hover` and its `canvas hover` line; the vocabulary test passes with the eleven words; existing expected states updated | `ew-viewmodel` tests; `bin/elektrawatt/tests/headless.rs` | §6.6, D7 | Passed (eleven words; the expected states updated) |
| A9 | Engine statistics, exact: after the first frame of folio 103, 200 steps of pan, zoom, pointer and resize give 0 memo re-executions, 0 further `page_content` calls, and no tile requested twice while cached | `ew-ui` test | §9, NFR-PERF-20 | Passed after the fix round: `skipped_cached` > 0, `requested_while_cached` and `rastered_while_cached` 0, with a `view.canvas_resize` step (ID-519) |
| A10 | kittest without rendering: Ctrl+Plus changes the zoom field and leaves the UI scale; Home fits; Ctrl+Arrow Right moves the view; a line-unit wheel event zooms around the pointer (its sheet point unchanged); a middle-button drag pans; View ▸ Grid shows `Grid off`; the cursor field follows the pointer; the canvas node has role `Canvas` and its name; the accessible-name walker passes after each step | `ew-ui` tests | §6.2, §6.5 | Passed |
| A11 | `FrameStats`: nearest-rank p50 and p99 and the ring's wrap; after N kittest frames the statistics hold N samples; no time is asserted | `ew-render2d`, `ew-ui` tests | §9 | Passed (no time asserted) |
| A12 | The bench compiles (`cargo bench -p ew-ui --bench canvas --no-run`); `xtask`'s gate parses a captured sample of its output (a fixture), runs it once per gate and treats its families as report-only | `xtask` unit tests | §9 | Passed: the bench compiles and the gate parses a captured sample; the bench was not run (owner's rule) |
| A13 | Policy: `cargo xtask check-deps` (the one new edge), `check-features`, `check-raw-read`, `check-licence-map`, `check-disallowed-methods` (the one clock site listed), `check-ordered-collections`; `python -m reuse lint`; no new package in `Cargo.lock`; `unsafe` appears only in `forbid` attributes | lane | §4, §5 | Passed: one new edge, no new package (535 packages) |
| A14 | Targeted tests and lints of the changed crates (`ew-dl`, `ew-render-cpu`, `ew-render2d`, `ew-api`, `ew-viewmodel`, `ew-ui`, `elektrawatt`, `xtask`): `cargo nextest run -p …` with and without `--all-features`, `cargo clippy -p … --all-targets --all-features -- -D warnings`, `cargo fmt --all --check`, `cargo doc -p ew-render2d -p ew-viewmodel -p ew-ui --no-deps` with `-D warnings` | lane | testing policy | Passed: 493 targeted tests (294 with default features), clippy, fmt and rustdoc clean |
| A15 | Manual on the lane's OS, reported and not gated: pan and zoom feel on folio 103, the settled crispness, the statistics overlay's numbers as observed | lane | — | Passed by the orchestrator after integration, on Windows in a debug build on a copy of the golden fixture (ID-522); the overlay's numbers are not recorded |

**Part 2**

| # | Check | Where | Proves | Result (cycle 22) |
|---|---|---|---|---|
| B1 | The switch rule's table, every row of §7.2, as a pure-function test; the adapter selector of `--no-gpu` on synthetic adapter lists | `ew-render2d`, `elektrawatt` tests | §7.2 | Passed |
| B2 | Render-to-texture on a headless device: the parity rule of §8 item 3 on folio 103 at `D = 150` and on the dense A3 page; a filled #0072B2 rectangle reads back its bytes within ±1; on Windows an adapter must be found | `ew-ui` tests | §7.1, §8 | Passed on the hardware DX12 adapter and on WARP, for folio 103 and the dense A3 page: 0 unmatched clearly-ink pixels, with the mean-channel-difference bound (0.60 against 0.75; 6.36 against 8.0), the edge-position check and #0072B2 within ±1 of the fix round (§8, ID-541, ID-542); on CI Linux with lavapipe under `EW_REQUIRE_GPU_ADAPTER=1`, the headless container `skipped: no adapter` |
| B3 | Caches, exact counts: a second preparation of folio 103 tessellates no mesh; N placements of one symbol give one mesh and N instances; repeated glyphs one mesh each | `ew-render2d` tests | §7.1 | Passed |
| B4 | LOD: at 10 % zoom the bar count equals the runs below 2 pixels; a 0.01 mm line leaves ink in the readback | `ew-render2d` tests on the headless device | §7.1 | Passed |
| B5 | An injected initialisation failure and an injected device loss switch the canvas to CPU with one banner each, and the status shows `CPU` | `ew-ui` tests | §7.2 | Passed |
| B6 | Exit code 3 and its message when the adapter request fails (an injected failure); `--no-gpu` gives the CPU canvas | `elektrawatt` tests | §7.2 | Passed |
| B7 | FT-328: the per-OS backend features, the binary size and the clean build time before and after, reported; `check-features` green | lane | §5 | Delivered, FT-328 closed: 53,900,288 B before, 54,407,168 B with the GPU path untrimmed, 52,502,528 B trimmed (-1.8 MiB); the clean build time is not measured (owner's rule); `check-features` green (§5) |
| B8 | Supply chain per §5; every mapped buffer range in the code lives inside its mapping scope (review grep for `get_mapped_range`) | lane; reviewer | §5, §7.1 | As built (ID-543): four lyon-family exemptions with a dated note, `termcolor` pruned, 384 to 387; the only mapped view is in `GpuCanvas::read_back` (§7.1) |
| B9 | Part 1's A1 to A14 still pass; the kittest flows run on the CPU canvas (no render state); the bench's GPU families compile; targeted tests and lints as A14 | lane | — | Passed: A1 to A14 hold; 248 tests in the four crates with and without all-features (after the fix round); page output and acceptance 36 tests; `golden/` untouched |

*Manual check of part 2 (A15's form, by the lane on Windows, reported and not gated):* the canvas with the status bar
`GPU DX12` and with `--no-gpu` (`CPU`) is identical at fit, at 283 % and after a pan; no lock was left behind. The
Linux look (macOS is not supported, ID-880), a real device loss, exit 3 on a host without an adapter, the MSAA resolve on GL hardware and
`--no-gpu` choosing llvmpipe are manual-only and stay an owner item (ID-544).

The workspace suite, the acceptance runner and the performance runs are not run by the lanes (testing policy); the
integrating session runs `cargo xtask ci` at the cycle delivery.

## 11. Files owned

**Part 1:** `crates/ew-dl/src/item.rs` and its tests (`entity_bounds`; MIT OR Apache-2.0 headers);
`crates/ew-render-cpu/src/{render.rs,lib.rs}` and tests (`render_region`, `ItemBounds`); `crates/ew-api/src/output/mod.rs`
and a test (`pub use ew_dl`, `PageGrid`, `Session::page_grid`); `crates/ew-render2d/**` (Cargo.toml: the
`ew-render-cpu` dependency and the description "The 2D canvas renderer: the CPU tile path and the wgpu path over one
display list"; new modules `view`, `tiles`, `pool`, `overlay`, `stats`); `crates/ew-viewmodel/src/**` and tests (new
`canvas.rs`; `actions`, `shell`, `status`, `strings`, `describe`, `script`); `crates/ew-ui/**` (new `canvas` module
replacing `page_view` in the project view, `headless.rs` for `canvas.snapshot`, `benches/canvas.rs`, Cargo.toml for
`tracing` and the bench); `bin/elektrawatt/tests/**`; `ci/dep-rules.toml` (the one edge);
`ci/disallowed-methods-allow.toml` (the clock site); `xtask/src/perf.rs` (the third bench). **Part 2:** additionally
`crates/ew-render2d/src/gpu/**` (with the WGSL file), `bin/elektrawatt/src/**`, `Cargo.lock`,
`supply-chain/config.toml` (exemptions only), `ci/build-scripts.allowlist`.

Read-only for the lanes: every other crate, `ci/licence-map.toml`, `deny.toml`, `.github/**`, `docs/**` (the design
pass updates the documents of §14). Each lane commits on its own branch and does not merge or push.

## 12. Out of scope

| Item | Where it goes |
|---|---|
| Selection, exact hit-testing over `page_spatial_index`, tool state machines, transient overlays and previews, snap indicators, the keyboard grid cursor, fit selection | WP-S2.03 |
| Tabs, side by side, per-page viewports in `view.json`, jump history, back and forward, xref jumps and hover previews, go to cell | WP-S2.03 |
| The cursor's cell in the status bar (`102-H15`) with the `Session` cell lookup that go to cell also needs | FT-345 (with WP-S2.03) |
| The page display list off the UI thread for a cold page | FT-346 (with FT-251) |
| The tile-scene digest and per-layer GPU buffers kept across a commit (NFR-PERF-20's PR criterion); the content-keyed meshes of FT-347 are delivered by part 2 | FT-347, the rest (with WP-S2.04) |
| Optional mipmaps for the level tiles: egui-wgpu 0.37 makes one mip level and ignores `mipmap_mode`, so mipmaps would need a CPU half level, and the GPU draws motion wherever an adapter is usable; deferred | FT-354, re-dated to "if a visual defect is reported" |
| A per-frame CPU fallback for `GpuError::Limits` instead of ending the GPU path for the session | FT-363 |
| The exit-3 adapter count honours `WGPU_BACKEND` while eframe's instance does not | FT-364 |
| Recovery from a real device loss on the shared device; lyon panics meeting the panic hook | FT-310, WP-S2.11 (ID-544) |
| Placement, wiring and drawing gestures; drag overlays | WP-S2.04 |
| Themes and their tokens, high contrast, minimum font size, UI scale to 300 %, keymap presets (the wheel and trackpad mapping may differ per preset) | WP-S2.08 |
| Image snapshots, the watchdog, the WARP and lavapipe snapshot jobs | WP-S2.11 |
| The WARP and CPU-renderer CI jobs running the kittest golden flows (ADR-0022) | WP-S6.06 |
| Printing | WP-S3.07 |
| The dark-sheet view transform (FR-UI-043, 1.0) and Describe page (FR-UI-046) | 1.0 |
| The 3D view and its software-adapter rule | WP-S4.04 |

## 13. Review checklist for the Opus reviewer

1. **Boundaries.** One new workspace edge only; the four items of §4.2 and no other public item below the shell;
   `ew-render2d` free of egui; wgpu named only through `ew_render2d::wgpu` outside `ew-render2d`; no `ew-model` name.
2. **One path.** Every zoom, pan, pointer and resize in the GUI ends in `ShellModel::dispatch`; the only other
   `ShellModel` mutation added is `set_canvas_backend`.
3. **References kept.** `render` and every golden byte unchanged; A3's byte equality holds; `view.snapshot` and
   `--smoke` unchanged.
4. **UI thread.** No raster or tessellation on the UI thread; at most 8 uploads per frame; stale jobs dropped; idle
   frames submit nothing.
5. **Determinism and privacy.** Transcripts and describe text carry no time, path or process ID; A7 passes; the
   clock is read in one listed function; no hash-order iteration reaches an output or a queue.
6. **GPU safety.** No mapped buffer view outlives `unmap`, `destroy` or device loss: every `get_mapped_range` result
   is used and dropped inside the scope that mapped it (the map callback or the block that awaited it), including the
   texture readback buffers of the tests and the bench; uploads use `write_buffer` and `write_texture`; no `unsafe`,
   no `bytemuck` derive.
7. **The switch rule** matches §7.2 row for row; every failure path ends on the CPU canvas with one banner; exit code 3
   is only for a missing adapter (an adapter-request or selector failure, or a fresh enumeration that counts none).
8. **Accessibility.** The canvas node's role and name; the walker passes; no meaning in colour alone in the overlays
   (the hover is dashed, a selection solid).
9. **Supply chain** (part 2). The dependency specs of §5; exemptions match the new lock entries; no audit entry by the
   lane; FT-328's numbers reported.
10. **Instrumentation.** The phases of §9; A9's counts exact; the bench report-only and never run by the lane; perf
    claims marked "not measured (owner's rule)" (ID-512).
11. **Scope.** Nothing of §12 half-done; deferred items named with their FT in the lane report.

## 14. Size, routing and the documents to update

**Size.** WP-S2.02 keeps 4.7 pw PERT (O 3 · L 4.5 · P 7); the CSV is unchanged. **Part 1, 2.4 pw:** the canvas model
and actions with the headless and describe changes 0.5, the four public items 0.2, the tile planner, pool, cache and
overlays 0.7, the egui host with input, status and AccessKit 0.4, instrumentation, the bench and the gate 0.3, the tests
0.3. **Part 2, 2.3 pw:** the wgpu renderer (pipelines, tessellation and caches, glyph outlines, LOD, clips) 1.3, the
host integration, the switch rule and failure handling 0.4, the parity and cache tests 0.3, FT-328 and the supply-chain
work 0.3.

**Routing.** Both parts are performance-critical rendering across several crates with public items (`CLAUDE.md`: the
lead-programmer's), so each goes to one lead-programmer lane at D4 (`opus`, `high`), with an Opus review. Part 2 starts
after part 1's delivery, so its GPU renderer is measured against a working CPU canvas.

**Documents the design passes update** (named here because D2, D6 and D7 change accepted text): ADR-0015 decision 2
(the pending amendment recorded there), architecture §3 (the `ew-render2d` row and edge), §8.2 (the offscreen texture,
glyph meshes, the tile path) and §9; doc 16 §3.6 (the zoom field), §3.9 (the canvas as built) and §10.1; doc 18 §3.17
(what the drafter can read back) and §4.2 (the bench families); ADR-0014's as-built note (egui-wgpu as a direct
dependency). *Done at the cycle-22 design pass (ID-555):* ADR-0015 amendment 10, ADR-0014's as-built note, architecture
§3, §5.4, §8.2 and §9, this specification, doc 16 and doc 18.

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S2.02, WP-S2.03, WP-S2.04, WP-S2.08, WP-S2.11, WP-S3.01, WP-S3.07,
  WP-S4.04, WP-S6.06; Appendix B (FT-251, FT-314, FT-326, FT-328, FT-345 to FT-347).
- [Decision log](../06-decision-log.md): ID-4, ID-332, ID-443, ID-444, ID-445, ID-474, ID-476, ID-493, ID-503, ID-505 to
  ID-511.
- [ADR-0005](../../adr/0005-value-types-nm-dec3-mltext.md), [ADR-0014](../../adr/0014-gui-toolkit-egui.md),
  [ADR-0015](../../adr/0015-display-list-renderers-pdf.md), [ADR-0022](../../adr/0022-platform-matrix-and-performance-budgets.md),
  [ADR-0027](../../adr/0027-native-symbol-grid-and-qet-grid-policy.md); [architecture](../../design/04-architecture.md)
  §3, §8, §9; [UX/UI](../../design/16-ux-ui.md) §1 to §3, §10, §11, §14, §17; [schematic editor](../../design/06-schematic-editor.md)
  §6; [reports and output](../../design/09-reports-forms-output.md) §4.3, §4.4;
  [quality](../../design/18-quality-security-release.md) §3.5, §3.11, §3.16, §3.17, §4.2, §4.3.
- Requirements: FR-UI-026, FR-UI-027, FR-UI-028, FR-UI-034, FR-UI-043, FR-UI-046, NFR-PERF-04, NFR-PERF-06,
  NFR-PERF-19, NFR-PERF-20, NFR-PLAT-02, NFR-PLAT-03, NFR-A11Y-02, NFR-A11Y-03.
