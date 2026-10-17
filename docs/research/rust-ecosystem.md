# The Rust ecosystem for a CAD-class desktop ECAD application (state: 2026-10-17)

Research note for **ElektraWatt**. It covers GUI toolkits, 2D/3D rendering, geometry kernels, file formats, persistence, undo and collaboration, scripting, app plumbing and testing. Each area is evaluated against what an EPLAN-class cabinet-design tool needs. The note ends with three candidate stacks, concrete recommendations and open questions.

---

## 0. Method, data provenance and caveats

- **Versions, release dates, licenses, MSRV and download counts** come straight from the crates.io registry API, queried on **2026-10-17**. "90d" means crates.io "recent downloads" (last 90 days). Crate names link to their crates.io page.
- **Repository health** (archived status, README status statements, changelogs) comes from GitHub pages, READMEs and CHANGELOG files fetched the same day.
- **Search budget ran out:** The shared web-search budget ran out partway through. The remaining facts come from direct fetches of primary sources, or from my own knowledge. Claims that could not be checked against a source today are marked **(unverified)**.
- **2026 crate flood:** Many brand-new crates appeared on crates.io in 2026, often apparently AI-generated, with very low download counts. Examples are a dozen STEP and DWG crates published August–October 2026. They are listed as **watch-list**, not **adopt**.
- **Rust toolchain:** By the 6-week cadence, current stable is about Rust 1.99 (computed). Several key crates need recent compilers: egui 0.37 needs MSRV 1.95, Bevy 0.20 needs 1.97.1, wasmtime 49 needs 1.96.

---

## 1. What ElektraWatt needs from the stack

| Requirement | Driver (brief / reference folio) | Consequence for the stack |
|---|---|---|
| Schematic canvas, hundreds of pages, tens of thousands of objects, IEC page frame with column 0..16 / row A..H grid | every folio; cross-refs such as "102-A1", "103-G6" (folios 101–106) | GPU canvas with a cached scene per page, a spatial index for hit-testing, zoom-independent hairlines and crisp text |
| Screen output identical to PDF output (title block, fonts, cross-ref labels) | title block on every page; defects 5–7 (inconsistent fields) | **One** display list and **one** text-shaping stack feeding both the screen and PDF backends; fonts bundled and embedded |
| Large tabular editors: part list, wire / terminal / cable lists, PLC I/O lists (Q0.1…Q6.7) | folio 901 BOM; folios 203–206 PLC addresses; defect 9 | Virtualised tables of 100k+ rows with sort, filter, inline edit and multi-select |
| Navigators: page tree by `=`/`+` structure, device tree, terminal strips | folio 2 TOC; IEC 81346 aspects `=P01`, `+J-E-2416` | Lazy tree view for 10k+ nodes |
| Property editing (wire "1.5 DBU", device part data, PLC address) | folios 101–106 | Property grid driven by a schema |
| Docking and multi-monitor (schematic on one screen; navigator, 3D and lists on another) | EPLAN workflow | Docking with tabs and splits, ideally native undocking into separate windows |
| 3D panel layout with STEP models of real parts (Schneider NSYPLA1074G enclosure, Phoenix 2904602 PSU, Festo CPX), rails and ducts, collision, routing, wire lengths | folio 902; brief v1 item 3 | STEP import and tessellation, instanced rendering, collision queries, graph routing |
| Reports and PDF: TOC, BOM, terminal / cable / wire lists, PDF/A, clickable cross-refs | folios 2, 901; defect 9 | A PDF writer with PDF/A, links, named destinations and outlines; a report layout engine |
| Imports: QElectroTech `.qet` (XML), DXF/DWG, EPLAN (XML / EDZ archives) | brief v1 item 4 | XML, zip, DXF and DWG readers |
| PLC exchange: AML (IEC 62424, XML), CSV, Excel | brief v1 item 1 | quick-xml, csv, xlsx crates |
| Machine exports (Komax, Rittal), user rules | brief v1 item 2 | Scripting and plugin API |
| Open-source licensing | brief | Every dependency must be license-compatible with the project license (not yet chosen) |

---

## 2. GUI toolkits

### 2.1 Snapshot (crates.io data, 2026-10-17)

| Toolkit | Latest stable (date) | Pre-release | License | Downloads total / 90d | Backing | Maturity statement |
|---|---|---|---|---|---|---|
| [egui](https://crates.io/crates/egui) / [eframe](https://crates.io/crates/eframe) | 0.37.0 (2026-10-15) | – | MIT OR Apache-2.0 | 26.7M / 6.8M | Rerun (company) + community | pre-1.0. Releases: 0.34 on 2026-03-26, 0.35 on 06-25, 0.36 on 08-05, 0.37 on 10-15 ([CHANGELOG](https://github.com/emilk/egui/blob/main/CHANGELOG.md)) |
| [iced](https://crates.io/crates/iced) | 0.14.0 (2025-12-07) | – | MIT | 3.0M / 0.67M | Héctor Ramón; Kraken sponsors | README: "Iced is currently experimental software" ([repo](https://github.com/iced-rs/iced)) |
| [slint](https://crates.io/crates/slint) | 1.18.1 (2026-09-21) | 1.19 unreleased | GPL-3.0-only OR Slint Royalty-free-2.0 OR Slint Software-3.0 | 2.05M / 0.71M | SixtyFPS GmbH | 1.x API stable since 1.0.0 (2023-04-03). Minor release about every 2 months ([CHANGELOG](https://github.com/slint-ui/slint/blob/master/CHANGELOG.md)) |
| [gpui](https://crates.io/crates/gpui) (official) | 0.2.2 (2025-10-22) | – | Apache-2.0 | 0.37M / 0.21M | Zed Industries | README: pre-1.0, "often be breaking changes" ([crates/gpui](https://github.com/zed-industries/zed/tree/main/crates/gpui)) |
| ↳ [gpui-pre](https://crates.io/crates/gpui-pre), [gpui-ce](https://crates.io/crates/gpui-ce), [gpui-fast](https://crates.io/crates/gpui-fast), [gpui-kit](https://crates.io/crates/gpui-kit) | 0.3.9 / 0.2.2 / 0.1.8 / 0.7.3 (Aug–Oct 2026) | – | Apache-2.0 | small | community, Longbridge | snapshot republish of `zed@93d7456`, community fork, layout fork, component kit |
| [dioxus](https://crates.io/crates/dioxus) | 0.7.10 (2026-07-30) | 0.8.0-alpha.3 (2026-10-16) | MIT OR Apache-2.0 | 3.2M / 1.2M | DioxusLabs (company) | Desktop uses a webview. The native renderer is "experimental" ([README](https://github.com/DioxusLabs/dioxus)) |
| ↳ [blitz-dom](https://crates.io/crates/blitz-dom) | 0.2.4 (2025-10-22) | 0.3.0-beta.4 | MIT OR Apache-2.0 | 0.29M / 0.07M | DioxusLabs | "currently in a beta state … many bugs and missing features" ([repo](https://github.com/DioxusLabs/blitz)) |
| [tauri](https://crates.io/crates/tauri) | 2.12.4 (2026-10-14) | 3.0.0-alpha.5 (2026-10-14), alpha line since 2026-09-13 | Apache-2.0 OR MIT | 37.1M / 14.7M | Tauri programme / CrabNebula | 2.0.0 stable since 2024-10-02 ([releases](https://github.com/tauri-apps/tauri/releases)) |
| [floem](https://crates.io/crates/floem) | 0.2.0 (2024-11-14) | – | MIT | 44k / 19k | Lapce | "still maturing" ([repo](https://github.com/lapce/floem)) |
| [xilem](https://crates.io/crates/xilem) / [masonry](https://crates.io/crates/masonry) | 0.4.0 (2025-10-29) | – | Apache-2.0 | 12k / 2.5k | Linebender | "experimental" ([repo](https://github.com/linebender/xilem)) |
| [makepad-widgets](https://crates.io/crates/makepad-widgets) | 1.0.0 (2025-05-13) | – | MIT OR Apache-2.0 | 20k / 1.4k | Makepad BV | own DSL and ecosystem ([repo](https://github.com/makepad/makepad)) |
| [freya](https://crates.io/crates/freya) | 0.4.3 (2026-08-30) | 0.5.0-rc.10 | MIT | 45k / 9.5k | essentially one maintainer | Skia-based, own reactive core since 0.4 ([repo](https://github.com/marc2332/freya)) |
| [relm4](https://crates.io/crates/relm4) + [gtk4](https://crates.io/crates/gtk4) | 0.11.0 (2026-04-08) / 0.11.5 (2026-09-20) | – | Apache-2.0 OR MIT / MIT (GTK itself LGPL-2.1+) | 1.3M / 0.76M; 5.0M / 1.7M | gtk-rs | mature bindings |
| [cxx-qt](https://crates.io/crates/cxx-qt) | 0.10.0 (2026-08-24) | – | MIT OR Apache-2.0 (Qt: LGPL-3.0 / GPL / commercial) | 130k / 55k | KDAB | "early development and the API changes frequently" ([repo](https://github.com/KDAB/cxx-qt)) |

Shared plumbing used by most of these: [winit](https://crates.io/crates/winit) 0.30.13 (2026-03-02; 0.31.0-beta.3), [accesskit](https://crates.io/crates/accesskit) 0.25.1 (2026-09-25), [muda](https://crates.io/crates/muda) 0.21.2 (native menus), [wgpu](https://crates.io/crates/wgpu) 30.0.1.

### 2.2 Capability matrix for ElektraWatt's needs

"DIY" means the application must build it.

| | Docking | Tree view (10k+ nodes) | Table (100k rows) | Property grid | Menus / shortcuts | IME / text input | Accessibility | Multi-window | Custom wgpu canvas | Native look |
|---|---|---|---|---|---|---|---|---|---|---|
| **egui** | [egui_dock](https://crates.io/crates/egui_dock) 0.21.1; [egui_tiles](https://crates.io/crates/egui_tiles) 0.17.1 (Rerun) | [egui_ltreeview](https://crates.io/crates/egui_ltreeview) 0.9.0 | `egui_extras::TableBuilder` virtual rows; [egui_table](https://crates.io/crates/egui_table) 0.10.0 ("millions of rows") | DIY on `Grid`, or [egui-probe](https://crates.io/crates/egui-probe) 0.13 | In-window menus, `KeyboardShortcut`. Native macOS menu bar only via muda (DIY) | IME overhaul in 0.35, more in 0.36/0.37 | AccessKit is a default eframe feature, with large progress in 0.34–0.37 | multi-viewport (native OS windows) | **Excellent**: `egui_wgpu` paint callbacks share the app's wgpu 30 device | No |
| **iced** | `pane_grid` (splits, drag; no tabs) | DIY | `table` widget new in 0.14; virtualisation DIY (unverified) | DIY | Menus via [iced_aw](https://crates.io/crates/iced_aw) 0.14.1 | IME added in 0.14 | None in mainline; not in 0.14 notes (unverified) | yes (since 0.12) | `shader` widget, but iced_wgpu 0.14 pins **wgpu 27** | No |
| **Slint** | none (DIY) | none (DIY over `ListView`) | `StandardTableView`, model-driven (virtualisation at 100k unverified) | DIY | `MenuBar` (native on macOS), `ContextMenuArea`, `KeyBinding` / `@keys` since 1.16 | yes | AccessKit via winit; native via Qt backend | yes | `unstable-wgpu-30`: texture import and rendering notifiers. The feature is renamed with every wgpu major | Partial (fluent / cupertino / material / qt styles) |
| **gpui (+gpui-kit)** | gpui-kit dock: resizable, tabs, nested splits, serializable | not confirmed | gpui-kit virtualised tables, "hundreds of thousands of rows" | DIY | **Best-in-class** actions and keymaps; native macOS menus | strong (Zed) | Weak. The official crate has no AccessKit dependency; gpui-ce adds accesskit 0.24 | yes | **No**: renderer is internal; custom GPU passes are not exposed (unverified) | No |
| **Tauri / Dioxus-desktop** (webview) | JS libraries (dockview, golden-layout) | JS | JS grids (AG Grid, TanStack) | JS | muda native menus | browser-grade | browser-grade | yes | WebGL2/WebGPU inside the webview only; no shared wgpu device | No (web) |
| **cxx-qt (QML)** | QML only; no Rust QWidgets bindings → no `QDockWidget` | QML TreeView | QML TableView | DIY | Qt | Qt | Qt (mature) | yes | via C++ / Qt RHI only | Yes |
| **relm4 / GTK4** | DIY | `GtkListView` + `TreeExpander` | `GtkColumnView` | DIY | GMenu | yes | AT-SPI (good on Linux, weaker elsewhere) | yes | `GtkGLArea` (OpenGL), no wgpu | GNOME only |

### 2.3 Notes per toolkit

**egui / eframe 0.37 (recommended shell).**
- **Rendering:** eframe's default features are `accesskit`, `wgpu`, `wayland`, `x11`, `system_fonts` (crates.io feature list). It depends on `wgpu ^30` and `winit ^0.30.13`.
- **Text:**
  - 0.34 replaced ab_glyph with **skrifa + vello_cpu** glyph rasterisation, adding hinting and variable fonts.
  - 0.35 switched shaping to **harfrust** for kerning and ligatures, and overhauled IME.
  - 0.37 adds a system font provider via fontique, so any system glyph renders, including CJK and colour emoji. Relevant for multilingual projects.
  - ([releases](https://github.com/emilk/egui/releases))
- **Accessibility (0.37):** `WidgetType` is replaced by `accesskit::Role`. Adds accessible names, better menu/modal/panel trees, and invisible widgets pruned from the tree. **egui_kittest now fails tests when input widgets lack an accessible name** ([CHANGELOG](https://github.com/emilk/egui/blob/main/CHANGELOG.md)).
- **Theming:** 0.35 added "classes" and 0.37 an experimental `StyleProvider` theme plugin system.
- **AI tooling:** 0.35 added an **inspection protocol plus `egui_mcp`**, which lets an AI agent read the AccessKit tree of a running app and drive it. This is directly useful for a 2–4 person team working with AI assistance.
- **Production reference:** Rerun builds a large-data 2D/3D viewer on egui plus its own wgpu renderer. It also maintains egui_tiles and egui_table.
- **Weaknesses:**
  - Immediate mode means the application owns all state.
  - Layout is single-pass, so some auto-sizing quirks remain.
  - The look is not native.
  - Every minor release is breaking (budget about a day per upgrade, estimate).
  - egui_dock tabs undock into in-app egui windows; native OS-window undocking is not documented ([egui_dock README](https://github.com/Adanos020/egui_dock)).
  - egui_dock splits support only two children per node; egui_tiles supports more.

**iced 0.14.**
- 0.14 (2025-12-07) shipped reactive rendering, time-travel debugging, headless and end-to-end testing, IME, `table` / `grid` widgets, hot reloading and the `comet` debugger ([releases](https://github.com/iced-rs/iced/releases)).
- Concerns:
  - About 10 months without a release.
  - Pinned to wgpu 27 and cosmic-text 0.15 while the rest of the ecosystem is on wgpu 30.
  - No mainline AccessKit (unverified).
  - The README calls it experimental.
- The Elm architecture maps nicely onto command-based undo, but large dynamic property forms get verbose.
- Users include Halloy and Icebreaker (README), with Kraken as sponsor. The COSMIC desktop uses a fork, libcosmic (unverified).

**Slint 1.18.**
- **Strengths:**
  - Declarative `.slint` markup with live preview.
  - Stable 1.x API.
  - Company backing.
  - Native-styled widget sets.
  - Recent large-UI performance work: slow focus moving in windows "with a very large number of elements" and an exponential visibility check were both fixed in 1.18 ([CHANGELOG](https://github.com/slint-ui/slint/blob/master/CHANGELOG.md)).
  - Keyboard shortcuts (`KeyBinding`, `MenuItem.shortcut`) since 1.16.
  - Experimental **Vello renderer** (`renderer-vello`) in 1.18.
  - An `mcp` cargo feature exists in 1.18.1 (details unverified).
- **wgpu interop is explicitly unstable:** the feature went `unstable-wgpu-26` → `27` → `28` → `29` → `30` within about 12 months. Each step was a new module (`slint::wgpu_30`, `BackendSelector::require_wgpu_30()`, `Image::to_wgpu_30_texture()`) and the old feature was removed.
- **No docking and no tree view:** you build them.
- **Licensing** ([royalty-free 2.0 text](https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md); [terms overview](https://slint.dev/terms-and-conditions)):
  - The GPL-3.0-only option means the distributed ElektraWatt binary must be GPL-3.0 compatible. Apache-2.0, MPL-2.0 and LGPL dependencies are fine; GPL-2.0-only dependencies are not.
  - Royalty-free 2.0 covers desktop, mobile and web, **excludes embedded systems**, and requires the `AboutSlint` widget or an attribution badge on the download page.
  - Royalty-free 2.0 also forbids distributing "an Application that exposes the Software's APIs". **A plugin API that lets plugins build Slint UI would conflict.**
  - Royalty-free 2.0 is not OSI-approved. Distributions use the GPL route; Debian has Slint in its [NEW queue](https://dfsg-new-queue.debian.org/reviews/rust-i-slint-common).

**gpui (Zed).**
- **Platforms** (README):
  - macOS: Metal.
  - Windows: Win32 windowing and DirectWrite text.
  - Linux / FreeBSD: `wayland` and/or `x11` features.
- The Linux renderer moved from Blade to **wgpu** ([PR #46758](https://github.com/zed-industries/zed/pull/46758), January 2026). macOS and Windows keep their native renderers ([HN discussion](https://news.ycombinator.com/item?id=47002825)).
- **Distribution problem:**
  - The official crates.io release is a year old (0.2.2, 2025-10-22, still on blade-graphics 0.7).
  - The README suggests `version = "*"` for a `gpui_platform` crate that is not on crates.io.
  - The ecosystem has split into republishes and forks: `gpui-pre` snapshots, `gpui-ce`, `gpui-fast`, `gpui-unofficial`.
- **gpui-kit 0.7.3** (formerly gpui-component, Longbridge; [repo](https://github.com/longbridge/gpui-component)):
  - 75+ components, including a serializable dock layout, virtualised tables, virtual lists and a 200k-line code editor.
  - "Has powered Longbridge Pro from day one."
  - It pins `gpui-pre =0.3.9`.
- gpui is excellent for text-heavy, keyboard-driven tools. But it lacks a stable release channel, an exposed GPU device for custom wgpu or 3D rendering, and mature accessibility. **That rules it out for ElektraWatt today.**

**Dioxus 0.7 / Blitz.**
- 0.7.0 (2025-10-31) brought sub-second Rust hot-patching.
- Desktop is a wry webview. "Native" is Blitz: Stylo CSS, Taffy layout, Parley text and Vello via AnyRender, with AccessKit in `blitz-shell`. Blitz is self-described as beta and not production-ready.
- 0.8 is in alpha (2026-10-16).
- With a webview, Dioxus inherits Tauri's trade-offs. Blitz is too young for a CAD shell.

**Tauri 2 / 3-alpha.**
- 2.x is stable and heavily used: 14.7M downloads in 90 days.
- The 3.0 alpha line (since 2026-09-13) adds **`tauri-runtime-cef`**, a Chromium Embedded Framework runtime (CEF 154.5.0 as of 2026-10-14), including windowless rendering with an app-provided render handler ([releases](https://github.com/tauri-apps/tauri/releases)). This could remove WebKitGTK/WKWebView inconsistencies at the cost of shipping Chromium (size not measured).
- The UI would be TypeScript, which conflicts with the decision that "the application is Rust" unless the canvas is Rust compiled to WASM inside the webview.

**Floem, Xilem/Masonry, Makepad, Freya.**
- None is suitable for a 5-year CAD codebase in 2026.
- Floem: no crate release since 2024-11; no accessibility mention.
- Xilem: experimental. `masonry 0.4` still depends on vello 0.6 and parley 0.6, while current versions are 0.11 and 0.12.
- Makepad: own DSL; 1.4k downloads in 90 days.
- Freya: single maintainer; no table or tree widget.
- Xilem is worth watching because its building blocks (vello, parley, kurbo, AccessKit) are the ones ElektraWatt would use anyway.

**relm4 / gtk4-rs and cxx-qt.**
- Both bind mature C toolkits.
- GTK4 on Windows and macOS needs a bundled GTK runtime and looks foreign (unverified, widely reported). It offers only OpenGL interop.
- cxx-qt has no Rust QWidgets bindings, needs a C++ / CMake / Qt 6 SDK toolchain, and leads to a three-language codebase (Rust, C++, QML).
- Both conflict with the "Rust-native" decision except as fallbacks.

### 2.4 Reference projects

| Project | Toolkit | Lesson for ElektraWatt |
|---|---|---|
| **LibrePCB 2.0** (2026-01-28) ([release](https://librepcb.org/blog/2026-01-28_release_2.0.0/)) | Slint (C++ API) | See the bullets below this table |
| **Zed** | gpui | Shows gpui's performance ceiling for text and editor UIs. It has no 3D and no user-supplied GPU passes |
| **Rerun** | egui + own wgpu renderer (`re_renderer`), egui_tiles, egui_table, `re_ui` 0.38.1 | The closest analogue to ElektraWatt: a pure-Rust, multi-view, large-data desktop app with 2D and 3D views embedded in an egui shell |
| COSMIC desktop | iced fork (libcosmic) (unverified) | iced works at scale but needed a fork (a11y, widgets) |

LibrePCB 2.0 details:
- The new single-window UI was "built from scratch" with Slint, replacing 5 separate windows.
- Legacy Qt dialogs still run on the shared Qt event loop, which allowed a step-by-step migration ([roadmap](https://librepcb.org/blog/2024-10-17_roadmap_2.0/)).
- **The board and schematic canvas is not Slint primitives.** `SlintGraphicsView::render(GraphicsScene, w, h) → slint::Image` paints with QPainter, and a later `setUseOpenGl` path renders into an offscreen OpenGL FBO ([dev docs](https://developers.librepcb.org/d9/d0c/classlibrepcb_1_1editor_1_1_slint_graphics_view.html), [header](https://developers.librepcb.org/dc/df0/slintgraphicsview_8h_source.html)).
- 2.0.1 (2026-02-22) was mostly bug fixes and HiDPI work ([2.0.1](https://librepcb.org/blog/2026-02-22_release_2.0.1/)).
- Lesson: the toolkit provides the chrome; **the CAD canvas is always the app's own renderer.**

---

## 3. 2D canvas rendering, text and hit-testing

### 3.1 Vector renderers

| Crate | Version (date) | License | Kind | Status / notes |
|---|---|---|---|---|
| [wgpu](https://crates.io/crates/wgpu) | 30.0.1 (2026-08-22) | MIT OR Apache-2.0 | GPU abstraction (Vulkan / Metal / DX12 / GL / WebGPU) | Breaking majors about **quarterly**: v27 on 2025-10-01, v28 on 2025-12-17, v29 on 2026-03-18, v30 on 2026-07-01 ([CHANGELOG](https://github.com/gfx-rs/wgpu/blob/trunk/CHANGELOG.md)). Foundation for egui, Bevy, vello, glyphon, kiss3d |
| [vello](https://crates.io/crates/vello) | 0.11.0 (2026-10-02) | Apache-2.0 OR MIT | GPU-compute 2D renderer | README: "remains an experimental implementation for compute-capable GPUs" ([repo](https://github.com/linebender/vello)) |
| [vello_cpu](https://crates.io/crates/vello_cpu) | 0.3.0 (2026-10-02) | Apache-2.0 OR MIT | CPU, SIMD, multithreaded | README: "currently overall more mature". egui uses it for glyphs. Good for thumbnails, export and visual tests |
| [vello_hybrid](https://crates.io/crates/vello_hybrid) → [vello_gpu](https://crates.io/crates/vello_gpu) | 0.2.0 (wgpu 29) → 0.3.0 (2026-10-02, wgpu 30) | Apache-2.0 OR MIT | CPU path processing, GPU raster; WebGL2 or wgpu backend without compute shaders | "intended to become the primary renderer for production GPU use cases as it matures". [vello_api](https://crates.io/crates/vello_api) 0.0.7 is a "preview" ([docs](https://docs.rs/crate/vello_api/latest)) |
| [femtovg](https://crates.io/crates/femtovg) | 0.27.0 (2026-08-31) | MIT OR Apache-2.0 | NanoVG-style GPU (OpenGL ES, wgpu) | Slint's default renderer; includes text shaping and path hit-testing ([repo](https://github.com/femtovg/femtovg)) |
| [tiny-skia](https://crates.io/crates/tiny-skia) | 0.12.0 (2026-02-02) | BSD-3-Clause | CPU Skia subset | Linebender-maintained. **No text.** 20–100% slower than Skia on x86-64. About 200 KiB binary ([repo](https://github.com/linebender/tiny-skia)) |
| [lyon](https://crates.io/crates/lyon) / [lyon_tessellation](https://crates.io/crates/lyon_tessellation) | 1.0.19 / 1.0.22 (2026-09-06) | MIT OR Apache-2.0 | Fill and stroke tessellation to triangles | Stable 1.x API. Ideal for tessellating symbol definitions once and drawing them instanced |
| [kurbo](https://crates.io/crates/kurbo) / [peniko](https://crates.io/crates/peniko) | 0.13.1 / 0.6.1 | Apache-2.0 OR MIT | Curve geometry (BezPath, arcs, nearest-point, area / winding) / brush and colour types | The geometry vocabulary of Linebender crates |
| [skia-safe](https://crates.io/crates/skia-safe) | 0.153.3 (2026-09-04) | MIT (Skia BSD-3) | Full Skia bindings incl. GPU and PDF backends | Downloads or builds a large native Skia binary. Used by Freya and Slint (renderer-skia) |
| [anyrender](https://crates.io/crates/anyrender) | 0.14.0 (2026-10-03) | MIT OR Apache-2.0 | Renderer-agnostic draw API (vello / skia / cpu) | Used by Blitz and Floem; an interesting pattern but young |

### 3.2 Text shaping and layout

| Crate | Version (date) | License | Role |
|---|---|---|---|
| [parley](https://crates.io/crates/parley) | 0.12.0 (2026-10-09) | Apache-2.0 OR MIT | Rich text layout (line breaking, bidi, styles). Uses [fontique](https://crates.io/crates/fontique) 0.12.0 for font enumeration and fallback |
| [cosmic-text](https://crates.io/crates/cosmic-text) | 0.19.0 (2026-04-22) | MIT OR Apache-2.0 | Shaping and layout plus editing buffer. Depends on fontdb 0.23, harfrust 0.5, skrifa 0.40, unicode-bidi. Used by iced and glyphon |
| [glyphon](https://crates.io/crates/glyphon) | 0.12.0 (2026-07-09) | MIT OR Apache-2.0 OR Zlib | wgpu 30 text renderer (glyph atlas) on cosmic-text 0.19 |
| [harfrust](https://crates.io/crates/harfrust) | 0.14.0 (2026-10-04) | MIT | HarfBuzz-compatible shaper. Successor of [rustybuzz](https://crates.io/crates/rustybuzz) (last release 0.20.1, 2024-11-12). Used by egui (epaint) and cosmic-text |
| [skrifa](https://crates.io/crates/skrifa) | 0.48.0 (2026-10-03) | MIT OR Apache-2.0 | Font parsing, scaling and hinting (Google Fonts "fontations"). Used by egui, krilla and Slint |
| [swash](https://crates.io/crates/swash) | 0.2.10 (2026-07-17) | Apache-2.0 OR MIT | Glyph scaling and rasterisation |
| [fontdb](https://crates.io/crates/fontdb) | 0.24.0 (2026-07-29) | MIT | System font database (resvg ecosystem) |
| [ttf-parser](https://crates.io/crates/ttf-parser) | 0.25.1 (2024-11-29) | MIT OR Apache-2.0 | Older parser, being superseded by skrifa / read-fonts |

For ElektraWatt, use **parley** (or cosmic-text) for layout, with bundled drawing fonts. Draw glyphs on the GPU from a glyph atlas, and embed exactly the same font files into PDFs through krilla's subsetter. This guarantees identical metrics on screen and on paper, which matters for title blocks and cross-reference labels.

### 3.3 Spatial index and hit-testing

- **Spatial index:** [rstar](https://crates.io/crates/rstar) 0.13.0 (2026-05-24; MIT OR Apache-2.0; 49M downloads). An R*-tree with bulk load, insert / remove, envelope and nearest-neighbour queries. Use one tree per page (AABBs of symbols, wires, texts), plus a project-level index for global search. Exact picking then uses kurbo (`ParamCurveNearest::nearest`, `Shape::contains`, stroke-distance checks).
- **Alternative index:** [parry2d](https://crates.io/crates/parry2d) 0.31.1 has BVH and shape queries.
- **2D boolean and polygon operations** (cabinet layout cut-outs, duct fill areas):
  - [geo](https://crates.io/crates/geo) 0.33.1 (MIT OR Apache-2.0).
  - [i_overlay](https://crates.io/crates/i_overlay) 9.1.0 (2026-10-14).
  - [clipper2](https://crates.io/crates/clipper2) 0.6.0, which I believe wraps the C++ Clipper2 (unverified).
  - [spade](https://crates.io/crates/spade) 2.15.1 (constrained Delaunay triangulation).
  - [earcutr](https://crates.io/crates/earcutr) 0.5.0 (ISC).

### 3.4 Recommended canvas architecture

1. **Display-list IR, generated by pure functions from the model** and memoised per symbol definition and per page. Items are `Path(kurbo::BezPath, stroke, brush)`, `GlyphRun`, `Image` and `Group(transform, children)`. Each item carries stable entity IDs for picking and highlighting.
2. **Screen backend: an own wgpu renderer** drawn inside the GUI via `egui_wgpu` callbacks.
   - Tessellate symbol geometry once per symbol definition with lyon.
   - Draw placements with instancing (per-instance transform and colour).
   - Render lines and wires as screen-space-width segments expanded in the vertex shader, so zooming never re-tessellates.
   - Render text from a glyph atlas.
   - A full A4 schematic page has a few thousand primitives; the whole project only needs to stream through the GPU for thumbnails or overview.
3. **PDF backend:** krilla (§6).
4. **SVG and DXF export** from the same IR.
5. **CPU raster backend:** vello_cpu or tiny-skia, for page thumbnails, print rasterisation and golden-image tests.
6. **Optional GPU vector backend:** vello or vello_gpu could replace (2) once vello_gpu is called production-ready. Keep the IR backend-agnostic, in the style of anyrender.

---

## 4. 3D (panel layout)

| Option | Version (date) | License | Assessment |
|---|---|---|---|
| **wgpu directly** (own renderer) | 30.0.1 | MIT OR Apache-2.0 | **Recommended.** Cabinet scenes are modest: one enclosure, about 100–500 devices, rails, ducts, a few thousand wires. Requirements: instancing, edge lines, transparency, picking and ortho / perspective views. This is roughly a few thousand lines of code, embeddable in an egui panel next to the 2D canvas and sharing the same device |
| [bevy](https://crates.io/crates/bevy) | 0.20.0 (2026-10-08) | MIT OR Apache-2.0 | README: "still in the early stages", with breaking releases "approximately once every 3 months": 0.18 on 2026-01-13, 0.19 on 2026-06-19, 0.20 on 2026-10-08 ([README](https://github.com/bevyengine/bevy)). MSRV 1.97.1, wgpu 30, accesskit 0.24. Wants to own the app loop and window. [bevy_egui](https://crates.io/crates/bevy_egui) 0.43.1, [bevy_picking](https://crates.io/crates/bevy_picking) 0.20.0 and [bevy_panorbit_camera](https://crates.io/crates/bevy_panorbit_camera) 0.35.1 exist. As an embedded renderer it brings a lot of churn for little gain |
| [three-d](https://crates.io/crates/three-d) | 0.19.0 (2026-04-17) | MIT | OpenGL 3.3 / WebGL2 (glow 0.17). Can run inside eframe's glow backend. Pre-1.0 with "regular breaking changes" ([repo](https://github.com/asny/three-d)). Using it would force eframe off its wgpu default |
| [rend3](https://crates.io/crates/rend3) | 0.3.0 (2022-02-12) | MIT OR Apache-2.0 OR Zlib | **Do not use.** Repository "archived by the owner on Jun 7, 2025", "MAINTENANCE MODE" ([repo](https://github.com/BVE-Reborn/rend3)) |
| [kiss3d](https://crates.io/crates/kiss3d) | 0.47.0 (2026-10-04) | BSD-3-Clause | Now on wgpu 30. "Not designed to be feature-complete or fast". Good for debug visualisation of routing graphs |
| [fyrox](https://crates.io/crates/fyrox) | 1.0.1 (2026-03-28) | MIT | Full game engine with editor (1.0.0 on 2026-03-24). Wrong shape for an ECAD app |
| [transform-gizmo-egui](https://crates.io/crates/transform-gizmo-egui) | 0.11.0 (2026-08-17) | MIT OR Apache-2.0 | Move / rotate gizmo for egui. Useful for placing devices on rails |

**Picking.** Use a CPU ray cast against parry3d BVHs of the tessellated meshes. It is deterministic, has no GPU readback stall, and the same structure serves collision. A GPU ID buffer (R32Uint target) is an option for hover feedback.

**Camera.** CAD conventions: orbit about the picked point, zoom to cursor, pan, standard views (front / top / iso) and an ortho toggle. This is about 300 lines of code; bevy_panorbit_camera is a good reference.

**Instancing.** Repeated parts, such as dozens of Phoenix PT 2.5 terminals on `+JE2416-X1031` (folio 103), share one mesh plus an instance buffer.

**Mesh processing.**
- [meshopt](https://crates.io/crates/meshopt) 0.7.0 (2026-10-17): vertex cache optimisation and simplification for level of detail (LOD).
- [gltf](https://crates.io/crates/gltf) 1.4.1: stable but last released 2024-05-10. Use it as a cache and exchange format.
- [stl_io](https://crates.io/crates/stl_io) 0.11.0 and [tobj](https://crates.io/crates/tobj) 4.0.5 for STL and OBJ.

---

## 5. Geometry and CAD kernels, STEP, collision, routing

### 5.1 B-rep / STEP options

| Option | Version (date) | License | Status and fit |
|---|---|---|---|
| **OCCT via [occt-sys](https://crates.io/crates/occt-sys) + [opencascade](https://crates.io/crates/opencascade)** (bschwind/opencascade-rs) | occt-sys 7.8.1 (2026-08-22); opencascade 0.3.0 (2026-08-24) | LGPL-2.1 (OCCT) | occt-sys statically builds OCCT 7.8.1 from source with CMake and a C++ compiler; a system OCCT build is faster. The wrapper uses cxx. README: "currently a major work in progress"; STEP/STL partly working ([repo](https://github.com/bschwind/opencascade-rs)) |
| **[cadrum](https://crates.io/crates/cadrum)** (lzpel) | 0.9.0 (2026-10-10) | MIT wrapper; OCCT LGPL-2.1 | Downloads **prebuilt static OCCT 8.0.1** for Linux x86_64 / aarch64, Windows MSVC / GNU, macOS arm64 / x86_64 and wasm32. Reads and writes STEP and BRep; booleans; meshes to STL and GLB. Young project: 67 stars, about 8.5k downloads ([repo](https://github.com/lzpel/cadrum)) |
| OCCT upstream | 8.0.0 final, then 8.0.1 | LGPL-2.1 + Open CASCADE exception (exception wording unverified) | 8.0 moved to C++17 with more than 500 changes since 7.9.0 ([announcement](https://dev.opencascade.org/content/open-cascade-technology-800-release)). KiCad 10.0.1 still reports OCC 7.6.3 ([KiCad forum](https://forum.kicad.info/t/open-cascade-technology-version-8-0-0-to-the-public/68991)). The industry-proven STEP AP203/AP214/AP242 reader, including assemblies and colours via XCAF |
| **truck** ([truck-stepio](https://crates.io/crates/truck-stepio), [truck-meshalgo](https://crates.io/crates/truck-meshalgo), [truck-shapeops](https://crates.io/crates/truck-shapeops)) | last crates.io release 0.3.0 / 0.4.0 (**2024-09-20**) | Apache-2.0 | The git master is active. The unreleased CHANGELOG lists "Read assembly from step file", "Write assembly to step file", a new `truck-assembly`, fillets and a `truck-drafting` 2D crate ([CHANGELOG](https://github.com/ricosjp/truck/blob/master/CHANGELOG.md)). Robustness on real vendor STEP is undocumented. **Pure-Rust option to evaluate, not to depend on yet** |
| [monstertruck](https://crates.io/crates/monstertruck) | 0.4.1 (2026-09-19) | Apache-2.0 | Fork and rename of truck: B-rep, NURBS, meshing, STEP export example ([docs](https://docs.rs/monstertruck)). 510 downloads |
| [ruststep](https://crates.io/crates/ruststep) / [espr](https://crates.io/crates/espr) | 0.4.0 (2024-09-20) | Apache-2.0 | EXPRESS compiler and Part 21 parser underlying truck-stepio |
| **Fornjot** ([fj](https://crates.io/crates/fj) 0.49.0, 2024-03-21) | – | 0BSD | **Shut down.** "This project has been shut down. Its goals were never reached." Repository archived 2026-06-19 ([repo](https://github.com/hannobraun/fornjot)) |
| foxtrot (Formlabs) | not on crates.io | MIT OR Apache-2.0 | "A fast, experimental STEP file viewer". AP214-generated `step` crate that "take[s] a very long time to compile", own `cdt` and `nurbs` crates. "May not work for your models". No paid maintenance ([repo](https://github.com/Formlabs/foxtrot)). Reference code only |
| [csgrs](https://crates.io/crates/csgrs) | 0.20.1 (2025-07-24) | MIT | Mesh CSG on BSP trees. Useful for generated geometry (duct cut-outs, mounting-plate holes), not for STEP |
| 2026 newcomers (watch-list) | – | various | [cadmpeg-codec-step](https://crates.io/crates/cadmpeg-codec-step) 0.6.1 (Apache-2.0; Part 21 read/write AP203/214/242), [step-io](https://crates.io/crates/step-io) 0.2.5, oxideav-step 0.0.1, stepq 0.6.0, [openbim-step](https://crates.io/crates/openbim-step) 0.11.0 (**AGPL-3.0**). All under 4k downloads and months old |

**Conclusion.** For vendor STEP files (Rittal, Phoenix, Siemens, Festo; usually AP214 or AP242 with assemblies), only OCCT is reliable today. Run it **out-of-process** in a `ew-step-worker` executable:

- crashes and pathological files cannot take down the editor;
- the LGPL obligations stay clean and visible;
- the long C++ build is isolated, and cadrum's prebuilt binaries remove it entirely;
- a pure-Rust worker (truck or monstertruck) can later be swapped in behind the same IPC contract.

The worker outputs:

- the assembly tree and part metadata;
- tessellated meshes at two LODs;
- B-rep feature edges for CAD-style outlines;
- bounding boxes and convex hulls for collision;
- mounting reference frames (DIN-rail clip points, where derivable).

Results are cached by content hash.

### 5.2 Collision, math, routing

- **[parry3d](https://crates.io/crates/parry3d) 0.31.1** (2026-09-18; Apache-2.0):
  - Now glam-based via [glamx](https://crates.io/crates/glamx) 0.3.
  - BVH-accelerated `TriMesh` and `Compound` queries.
  - Since 0.31, query results report which sub-shape (triangle or part) was hit.
  - Deformable meshes with BVH refit; rayon-parallel contact manifolds; rkyv and serde support ([CHANGELOG](https://github.com/dimforge/parry/blob/master/CHANGELOG.md)).
  - Use it for broadphase AABB / OBB checks, exact `intersection_test` between device meshes, `distance` queries for clearance rules, and ray casts for picking.
  - Note 0.31.0 was yanked; use 0.31.1.
- **[rapier3d](https://crates.io/crates/rapier3d) 0.36.1** (2026-10-10): a physics engine. Not needed, since ElektraWatt has no dynamics.
- **Linear algebra:**
  - [glam](https://crates.io/crates/glam) 0.34.1: f32/f64 SIMD, used by Bevy and parry. Use `DVec3` / `DAffine3` for model coordinates in mm and f32 for GPU buffers.
  - [nalgebra](https://crates.io/crates/nalgebra) 0.35.0: only if a generic solver is needed.
- **Wire routing.** No Rust crate addresses cabinet wire routing (unverified, but nothing surfaced on crates.io).
  - Build a routing graph: nodes at duct entries / exits and junctions, rail ends and device connection points; edges carry length and duct-fill capacity.
  - Run shortest paths with [petgraph](https://crates.io/crates/petgraph) 0.8.3 (MIT OR Apache-2.0) or [pathfinding](https://crates.io/crates/pathfinding) 4.16.0 (A*, Dijkstra, Yen k-shortest for alternatives).
  - Then add bend allowances and service loops, and check duct fill (cross-section sum against duct cross-section).
  - This yields the automatic wire lengths needed for the Komax export.

---

## 6. 2D and document formats

### 6.1 DXF / DWG / SVG

| Crate | Version (date) | License | Notes |
|---|---|---|---|
| [dxf](https://crates.io/crates/dxf) (ixmilia/dxf-rs) | 0.6.2 (2026-10-03) | MIT | Reads and writes DXF (and DXB). Ships spec references R10–2018. Integration tests need the ODA File Converter on Windows ([repo](https://github.com/ixmilia/dxf-rs)). The default choice for DXF import / export |
| [acadrust](https://crates.io/crates/acadrust) | 0.6.4 (2026-10-11) | **MPL-2.0** | Pure Rust, reads and writes **DWG and DXF R12 (AC1009) to R2018+ (AC1032)**. AC1009 fidelity incomplete. 48 entity types, ACIS SAT/SAB, CI on three operating systems plus wasm32 ([repo](https://github.com/hakanaktt/acadrust)). First release 2026-02-07, 37k downloads. **Most promising DWG option; needs a corpus evaluation** |
| [cadkit-dwg](https://crates.io/crates/cadkit-dwg) | 0.3.3 (2026-10-16) | MIT OR Apache-2.0 | Clean-room DWG reader R14–2018. 252 downloads, 2 weeks old |
| [libredwg-sys](https://crates.io/crates/libredwg-sys) (+ uncad) | 0.12.0 (2026-10-07) | **GPL-3.0-or-later** | Vendored LibreDWG with local patches. Usable only if ElektraWatt is GPL-3-compatible |
| ODA Drawings SDK (Teigha) | – | proprietary | Incompatible with distributing an OSS product. Users can convert DWG to DXF themselves with the free ODA File Converter |
| [usvg](https://crates.io/crates/usvg) / [resvg](https://crates.io/crates/resvg) | 0.48.1 (2026-08-02) | Apache-2.0 OR MIT | Parse and render SVG, e.g. imported vendor symbols, icons, logos in title blocks |
| [svg](https://crates.io/crates/svg) | 0.18.0 (2024-09-27) | Apache-2.0 OR MIT | Simple writer. Writing SVG from the display list by hand is also trivial |

### 6.2 PDF writing

| Crate | Version (date) | License | Notes |
|---|---|---|---|
| **[krilla](https://crates.io/crates/krilla)** | 0.8.2 (2026-06-04) | MIT OR Apache-2.0 | See the feature list below this table |
| [pdf-writer](https://crates.io/crates/pdf-writer) | 0.15.0 (2026-05-27) | MIT OR Apache-2.0 | Low-level, step-by-step PDF writer (krilla's base) |
| [printpdf](https://crates.io/crates/printpdf) | 0.12.8 (2026-09-05) | MIT | Higher-level page API. Less rigorous standards validation than krilla (unverified) |
| **[typst](https://crates.io/crates/typst)** / [typst-pdf](https://crates.io/crates/typst-pdf) | 0.15.1 (2026-07-17) | Apache-2.0 | See the feature list below this table |
| [typst-as-lib](https://crates.io/crates/typst-as-lib) | 0.17.0 (2026-10-07) | MIT | Convenience wrapper for embedding Typst |

krilla ([repo](https://github.com/LaurenzV/krilla)):
- Builds on pdf-writer and supports PDF 1.4–2.0.
- **PDF/A-1, -2, -3 and -4 and PDF/UA-1** with tagged PDF.
- Outlines, page labels, annotations, **links, named destinations** and metadata.
- CFF/TTF subsetting and colour fonts; SVG via krilla-svg.
- No `unsafe`.
- 90+ snapshot tests and 210+ visual tests across six viewers.
- **Recommended drawing PDF backend.**

Typst ([releases](https://github.com/typst/typst/releases)):
- PDF export has been built on krilla since 0.14 (2025-10-24).
- All PDF/A variants from 1b to 4e, opt-in PDF/UA-1, and tagged output by default.
- **Embeds PDFs as images via hayro.**
- 0.15 made floating-point results consistent across platforms, giving reproducible output.
- 0.14.2 fixed a WASM-plugin runtime memory bug.
- Strong for free-form documents. EPLAN-style forms are fixed-grid reports, better served by an own form engine.

### 6.3 PDF reading and import

| Crate | Version (date) | License | Notes |
|---|---|---|---|
| [hayro](https://crates.io/crates/hayro) (+ hayro-syntax / -interpret / -svg) | 0.8.0 (2026-10-04) | Apache-2.0 OR MIT | Pure-Rust PDF interpreter and renderer to bitmap or SVG. Tested against 1,400+ PDFs from the PDFBox and pdf.js corpora. Missing: knockout groups and non-embedded CID fonts. "Performance has not been a focus" ([repo](https://github.com/LaurenzV/hayro)). Uses: render legacy PDFs and datasheets as page underlays, PDF thumbnails, and **regression-testing our own PDF output** |
| [lopdf](https://crates.io/crates/lopdf) | 0.45.0 (2026-09-08) | MIT | Object-level read / modify / write: merge, metadata, bookmarks, form fields |
| [pdfium-render](https://crates.io/crates/pdfium-render) | 0.9.4 (2026-09-06) | MIT OR Apache-2.0 | Bindings to Google PDFium. Needs the PDFium shared library shipped alongside (BSD-3 / Apache-2.0) (unverified detail). The most robust renderer, but a native dependency |
| [pdf](https://crates.io/crates/pdf) | 0.10.0 (2026-03-02) | MIT | Older pure-Rust reader |

### 6.4 Tabular, XML and archive formats

- **Spreadsheets and CSV:**
  - [calamine](https://crates.io/crates/calamine) 0.36.1: reads xlsx / xls / ods.
  - [rust_xlsxwriter](https://crates.io/crates/rust_xlsxwriter) 0.100.0 (2026-10-17; MIT OR Apache-2.0): writes formatted xlsx with tables, autofilters and formulas. Ideal for wire lists and PLC lists for customers.
  - [umya-spreadsheet](https://crates.io/crates/umya-spreadsheet) 3.1.1: read-modify-write.
  - [csv](https://crates.io/crates/csv) 1.4.0.
- **XML:**
  - [quick-xml](https://crates.io/crates/quick-xml) 0.42.0 (MIT): streaming plus serde; best for large AML (CAEX) files and EPLAN XML.
  - [roxmltree](https://crates.io/crates/roxmltree) 0.21.1: fast read-only DOM; convenient for QET `.qet` / `.elmt` element files.
  - [xmltree](https://crates.io/crates/xmltree) 0.12.0.
- **Archives:** [zip](https://crates.io/crates/zip) 9.0.4 (2026-10-16; MIT) for EDZ archives and AMLX (OPC) packages, both zip-based (unverified internal layout), and for an optional single-file project container.

---

## 7. Persistence and serialization

| Crate | Version (date) | License | Assessment |
|---|---|---|---|
| [rusqlite](https://crates.io/crates/rusqlite) | 0.40.2 (2026-08-08) | MIT | **Recommended for parts and catalogue DB, caches and indices.** The `bundled` feature compiles SQLite in. Features include `session` (changesets), `backup`, `functions`, `vtab`, `serialize` and `modern_sqlite`. FTS5 is available through bundled SQLite |
| [sqlx](https://crates.io/crates/sqlx) | 0.9.0 (2026-05-21) | MIT OR Apache-2.0 | Async and compile-time checked queries. Overkill for an offline desktop app |
| [libsql](https://crates.io/crates/libsql) | 0.10.0 (2026-10-13) | MIT | Turso fork; its replication features are irrelevant for v1 |
| [redb](https://crates.io/crates/redb) | 4.4.0 (2026-10-13) | MIT OR Apache-2.0 | Pure-Rust ACID key-value store. Note the major bumps: 3.0 on 2025-08-09 and 4.0 on 2026-04-02 |
| [sled](https://crates.io/crates/sled) | 0.34.7 (2021-09-12); 1.0.0-alpha.124 (2024-10) | MIT/Apache-2.0 | **Avoid**: stagnant alpha |
| [fjall](https://crates.io/crates/fjall) | 3.1.12 (2026-10-03) | MIT OR Apache-2.0 | LSM key-value store; not needed |
| [serde](https://crates.io/crates/serde) 1.0.229 / [serde_json](https://crates.io/crates/serde_json) 1.0.154 / [ron](https://crates.io/crates/ron) 0.12.2 / [toml](https://crates.io/crates/toml) 1.1.8 (TOML 1.1) / [kdl](https://crates.io/crates/kdl) 6.7.1 | – | permissive | Text formats for the project files |
| [rkyv](https://crates.io/crates/rkyv) 0.8.18 / [postcard](https://crates.io/crates/postcard) 1.1.3 / [wincode](https://crates.io/crates/wincode) 0.6.3 / [bitcode](https://crates.io/crates/bitcode) 0.6.9 | – | permissive | Binary caches. rkyv is zero-copy and suits tessellation caches |
| [bincode](https://crates.io/crates/bincode) | 3.0.0 (2025-12-16) | MIT | **Unmaintained.** 3.0.0 contains only a compile error. "Due to a doxxing and harassment incident, development on bincode has ceased". The README suggests wincode, postcard or rkyv ([readme](https://github.com/bincode-org/bincode)) |
| IDs: [uuid](https://crates.io/crates/uuid) 1.28.0 (v7), [ulid](https://crates.io/crates/ulid) 3.0.0 | – | permissive | Stable entity IDs |

**Recommended project storage (git-friendly by design):**

- **Directory layout:** A project is a directory with one text file per page, per device group or structure node, and per local symbol or part. Example layout: `project.json`, `pages/101.page.json`, `devices/…`, `cables/…`.
- **Serialization rules:**
  - Deterministic: sorted keys, `BTreeMap` / `IndexMap` with defined order, no timestamps inside entities.
  - Coordinates stored as integers (µm) or fixed decimals, so float noise never produces diffs.
  - Entities identified by UUIDv7 or ULID, never by tag. Tags like `-101F1` are derived and editable without breaking references, which addresses defects 1 and 2.
- **Format choice:** RON maps Rust enums naturally. JSON has the widest tooling (jq, diff viewers, JSON Schema for third parties). TOML is awkward for large nested arrays.
- **Transport:** A single-file `.ewz` is a zip of the same directory, for e-mail and transport.
- **SQLite only for derived or large data:** parts library (EDZ / ECLASS / ETIM imports, FTS5 search), search indices, tessellation caches. Everything there can be rebuilt from text plus imports.

---

## 8. Undo, incremental recomputation, collaboration

**Undo and command pattern.**
- [undo](https://crates.io/crates/undo) 0.52.0 (2025-03-08; MIT OR Apache-2.0): Record and History with branches and merging.
- Most CAD apps write their own `Command { apply, revert, merge }` with transactions. ElektraWatt needs one transaction to span model edits and derived-index updates, e.g. "renumber devices on page 103".

**Persistent (immutable) data structures.**
- [imbl](https://crates.io/crates/imbl) 7.0.2 (2026-09-09; MPL-2.0+) is the maintained fork of [im](https://crates.io/crates/im) (15.1.0, last release 2022-04-29).
- [rpds](https://crates.io/crates/rpds) 1.2.1 (MIT).
- With structural sharing, the model root is O(1) to clone. That enables:
  - snapshot-based undo / redo;
  - background threads (report generation, PDF export of 500 pages, STEP worker results) working on a consistent snapshot while the user keeps editing;
  - cheap "preview" of destructive operations.

**Incremental derived data.**
- [salsa](https://crates.io/crates/salsa) 0.28.5 (2026-09-24; Apache-2.0 OR MIT) is rust-analyzer's incremental query engine.
- Model derived artefacts as memoised queries rather than stored data: device tags, cross-reference labels ("102-A1"), potential propagation (wire colour and size from potential), TOC, BOM with quantity aggregation, terminal / cable / wire lists.
- This structurally prevents defects 1, 2, 4, 6, 7 and 8, because nothing derived can go stale.
- A simpler hand-rolled dependency-tracked cache is also viable; salsa's API churn is a cost (0.x).

**CRDTs.**

| Crate | Version (date) | License | Data model | Fit |
|---|---|---|---|---|
| [automerge](https://crates.io/crates/automerge) (+ [autosurgeon](https://crates.io/crates/autosurgeon) 0.14.0) | 0.12.0 (2026-09-16) | MIT | JSON-like: maps, lists, text, counters. "Automerge 3" cut memory about 10×. The Rust API is "low-level", and autosurgeon is recommended for struct mapping ([repo](https://github.com/automerge/automerge)) | OK for documents; the struct-mapping layer adds friction |
| [yrs](https://crates.io/crates/yrs) | 0.28.0 (2026-09-17) | MIT | Yjs port: Map, Array, Text, XML | Strongest ecosystem when the UI is web-based |
| [loro](https://crates.io/crates/loro) | 1.16.2 (2026-09-21) | MIT | Map (LWW), List, **MovableList, movable Tree**, Text / rich text; time travel; shallow snapshots; Eg-walker ([repo](https://github.com/loro-dev/loro)) | **Best structural fit**: the page / structure tree as a movable tree, entity properties as maps |
| [diamond-types](https://crates.io/crates/diamond-types) | 1.0.0 (2022-08-25) | ISC | Text-only | Not applicable |

The engineering model is a graph with referential and semantic invariants: unique tags per structure, connections to existing terminals, one main function per device. A CRDT merge can produce states that are well-formed but semantically invalid, so a validation and repair pass is needed regardless. CRDT logs are also binary and opaque to git.

**Recommendation:** no CRDT in v1. Make every edit an explicit, serialisable operation on stable IDs, which also feeds undo and a macro / scripting log. Offer git-based collaboration with a semantic 3-way merge driver at entity level. Layer Loro on top only if real-time co-editing becomes a requirement.

---

## 9. Scripting and plugins

| Crate | Version (date) | License | Assessment |
|---|---|---|---|
| [rhai](https://crates.io/crates/rhai) | 1.26.1 (2026-09-10) | MIT OR Apache-2.0 | Pure Rust, embeddable, sandboxed by default (no I/O unless registered), with operation and call-depth limits. **Ideal for user rules**: tag format templates, wire property rules by potential, report field expressions, title-block field formulas |
| [mlua](https://crates.io/crates/mlua) | 0.12.2 (2026-10-03) | MIT | Lua 5.1–5.4, LuaJIT and **Luau** (sandbox mode). Vendored C build. Faster than rhai; a familiar language for some users |
| [pyo3](https://crates.io/crates/pyo3) | 0.30.1 (2026-10-14) | MIT OR Apache-2.0 | Embedding CPython means shipping a Python runtime and dealing with ABI and versions. **Better in reverse**: publish the headless ElektraWatt core as a Python wheel (pyo3 / maturin) for automation, as an alternative to an in-app Python console |
| [wasmtime](https://crates.io/crates/wasmtime) | 49.0.3 (2026-10-14) | Apache-2.0 WITH LLVM-exception | Monthly majors. **LTS: every 12th release gets 24 months of security fixes** ([BA article](https://bytecodealliance.org/articles/wasmtime-lts)). 36.0.0 (2025-08-20) is supported to 2027-08-20; 48.0.0 (2026-08-20) is the next LTS by that rule (inferred). Component Model plus WIT via [wit-bindgen](https://crates.io/crates/wit-bindgen) 0.63.0. **Recommended plugin host** |
| [wasmer](https://crates.io/crates/wasmer) | 7.5.0 (2026-10-01) | MIT | Alternative runtime; weaker Component Model story (unverified) |
| [extism](https://crates.io/crates/extism) | 1.41.0 (2026-10-09) | BSD-3-Clause | Simpler plugin framework on wasmtime, with host and plugin SDKs ([repo](https://github.com/extism/extism)). Byte-buffer interfaces, so less typed than WIT |
| [rquickjs](https://crates.io/crates/rquickjs) 0.14.0 / [boa_engine](https://crates.io/crates/boa_engine) 0.22.0 | – | MIT / Unlicense OR MIT | Embedded JS; no strong reason for ECAD |
| [abi_stable](https://crates.io/crates/abi_stable) 0.11.3 (2023-10-12) / [stabby](https://crates.io/crates/stabby) 72.1.16 / [libloading](https://crates.io/crates/libloading) 0.9.0 | – | MIT/Apache; EPL-2.0 OR Apache-2.0; ISC | Native dylib plugins with a stable ABI. abi_stable is stale. Unsafe, per-platform builds, crash-prone: **avoid** |

**Plugin architecture:**
- **Tier 1 (v1):** rhai for rules and expressions.
- **Tier 2 (v1.x):** WASM components on the wasmtime LTS line. A WIT world exposes read-only model queries, operation submission, and file I/O through capability handles. Use it for importers and exporters: Komax, Rittal, TIA Portal / Codesys AML / CSV variants, customer-specific reports.
- **Tier 3:** headless core as a Python wheel and CLI for batch automation.

---

## 10. Application plumbing

### 10.1 Overview

| Concern | Crates (version, date, license) | Notes |
|---|---|---|
| File dialogs | [rfd](https://crates.io/crates/rfd) 0.17.2 (2026-01-12, MIT); [native-dialog](https://crates.io/crates/native-dialog) 0.9.7 | rfd uses native dialogs on Windows / macOS and xdg-desktop-portal or GTK on Linux |
| Clipboard | [arboard](https://crates.io/crates/arboard) 3.7.0 (2026-10-15, MIT OR Apache-2.0) | Text and images. Vector clipboard (EMF on Windows, PDF on macOS) for pasting schematic snippets into Word / Excel is DIY via `windows` / `objc2` |
| Native menus, tray, hotkeys | [muda](https://crates.io/crates/muda) 0.21.2, [tray-icon](https://crates.io/crates/tray-icon) 0.26.1, [global-hotkey](https://crates.io/crates/global-hotkey) 0.9.0 | From the Tauri team; work with raw winit windows |
| Notifications | [notify-rust](https://crates.io/crates/notify-rust) 4.18.2 | Background job finished (PDF export, STEP import) |
| File watching | [notify](https://crates.io/crates/notify) 8.2.0 (9.0.0-rc.5) (CC0-1.0) | Detect external edits to project files after a git pull, and changes to library folders |
| i18n | [fluent](https://crates.io/crates/fluent) 0.17.0 / [fluent-bundle](https://crates.io/crates/fluent-bundle) 0.16.0 (May 2025), [fluent-templates](https://crates.io/crates/fluent-templates) 0.15.1, [i18n-embed](https://crates.io/crates/i18n-embed) 0.16.0; [rust-i18n](https://crates.io/crates/rust-i18n) 4.2.4; [icu](https://crates.io/crates/icu) 2.3.1 (Unicode-3.0); [sys-locale](https://crates.io/crates/sys-locale) 0.3.2 | Fluent handles plurals and grammatical cases (NL / DE / FR UI). Use ICU4X for number and date formatting and collation in reports. Keep *project-content* multilinguality (EPLAN-style multi-language texts) separate from UI i18n |
| Logging | [tracing](https://crates.io/crates/tracing) 0.1.44, [tracing-subscriber](https://crates.io/crates/tracing-subscriber) 0.3.23, [tracing-appender](https://crates.io/crates/tracing-appender) 0.2.5 | Rolling file logs plus in-app log panel |
| Crash reporting | [sentry](https://crates.io/crates/sentry) 0.49.3 (MIT), [sentry-rust-minidump](https://crates.io/crates/sentry-rust-minidump) 0.18.1, [crash-handler](https://crates.io/crates/crash-handler) 0.8.1 + [minidumper](https://crates.io/crates/minidumper) 0.11.0 (Embark; out-of-process minidumps) | Self-hosted Sentry or GlitchTip for an OSS project; opt-in only (GDPR) |
| Auto-update | [velopack](https://crates.io/crates/velopack) 1.2.161 (MIT; installers plus delta updates, Win / macOS / Linux); [self_update](https://crates.io/crates/self_update) 1.3.1; [axoupdater](https://crates.io/crates/axoupdater) 0.10.2 (cargo-dist); cargo-packager-updater | Velopack is the most complete; offline-first means updates must be optional |
| Packaging | [cargo-dist](https://crates.io/crates/cargo-dist) 0.32.0 (2026-05-22); [cargo-packager](https://crates.io/crates/cargo-packager) 0.11.8 (2025-11-27; DMG / .app, deb, AppImage, pacman, NSIS, MSI / WiX) ([repo](https://github.com/crabnebula-dev/cargo-packager)); [cargo-bundle](https://crates.io/crates/cargo-bundle) 0.12.0 (2026-09-20); [cargo-wix](https://crates.io/crates/cargo-wix) 0.3.9 (2025-03-13) | See the dist note below this table |
| Code signing | [apple-codesign](https://crates.io/crates/apple-codesign) 0.29.0 (rcodesign; MPL-2.0; last release 2024-11-29): sign and notarize macOS apps from Linux CI. Windows Authenticode via Azure Artifact (Trusted) Signing, or the free SignPath Foundation programme for OSS (both unverified for 2026 eligibility). Linux: Flathub and AppImage need no paid certificates | Apple Developer ID costs USD 99/yr (unverified). Decide who owns the signing identity (the pilot panel builder vs a project foundation) |
| Cross builds | [cargo-zigbuild](https://crates.io/crates/cargo-zigbuild) 0.23.4 (glibc targeting); [cross](https://crates.io/crates/cross) 0.2.5 (2023, stale) | Prefer native runners per OS |

Packaging note: dist lost its original company backing in 2025 and continued with community releases ([blog](https://www.mistys-internet.website/blog/blog/2025/07/20/dist-0-dot-28-dot-1-is-out)); its 2026 maintenance status is unconfirmed.

### 10.2 Printing (deep dive)

No crate in October 2026 offers a cross-platform print dialog plus page rendering.

| Platform | Viable path | Crates |
|---|---|---|
| Linux | xdg-desktop-portal **Print** portal: `PreparePrint` shows the desktop's print dialog, then `Print` takes a PDF file descriptor. Works in Flatpak. Alternative: CUPS directly (CUPS accepts PDF) | [ashpd](https://crates.io/crates/ashpd) 0.13.13 (MIT); [printers](https://crates.io/crates/printers) 2.3.0 (CUPS / winspool listing, printing files and bytes, jobs; optional Ghostscript; no dialog) ([repo](https://github.com/talesluna/rust-printers)); [cpdb-rs](https://crates.io/crates/cpdb-rs) 0.1.1 (OpenPrinting Common Print Dialog Backends; 58 downloads) |
| macOS | Load the generated PDF into `PDFDocument`, then `printOperation(for:scalingMode:autoRotate:)`, then run it modally. This gives the native print panel with preview | [objc2-pdf-kit](https://crates.io/crates/objc2-pdf-kit) 0.3.2, [objc2-app-kit](https://crates.io/crates/objc2-app-kit) 0.3.2 |
| Windows | `PrintDlgEx` returns an HDC; render pages either as vectors (display list to GDI or Direct2D) or as rasters (vello_cpu at printer DPI, **banded**). Alternative: the XPS Print / Print Document Package API with XPS generated from the display list | [windows](https://crates.io/crates/windows) 0.62.2 |

Rasterising a full A3 page at 600 dpi gives 7016 × 9921 px, about 278 MB in RGBA. That is why rasterisation must be banded, or use 1-bit or 300 dpi.

**Recommendation: "PDF is the print master."** Printing means generating the PDF (krilla), then handing it to a small per-OS adapter: Linux portal / CUPS, macOS PDFKit, and on Windows a GDI path that renders from the display list. A "print via system PDF viewer" fallback is always available.

### 10.3 CI on GitHub Actions

- **Runners:** Windows (windows-2025), macOS arm64 (macos-15) and Linux (ubuntu-24.04, plus arm64). Runner labels are unverified; check the current image list when setting up.
- **Tools:**
  - `Swatinem/rust-cache`.
  - [cargo-nextest](https://crates.io/crates/cargo-nextest) 0.9.148.
  - [cargo-llvm-cov](https://crates.io/crates/cargo-llvm-cov) 0.9.1.
  - [cargo-deny](https://crates.io/crates/cargo-deny) 0.20.2, which enforces the **license allowlist** matching the chosen project license, plus advisories such as RUSTSEC unmaintained notices (e.g. bincode).
  - [cargo-about](https://crates.io/crates/cargo-about) 0.9.2, which generates the third-party notices required for MIT, BSD, Apache and OFL fonts.
  - [cargo-auditable](https://crates.io/crates/cargo-auditable) 0.7.7, which embeds the dependency list in binaries.
  - [cargo-insta](https://crates.io/crates/cargo-insta) 1.49.0.
- **Build caching:** The OCCT build dominates build time (unverified, likely tens of minutes from source). Use cadrum's prebuilt OCCT or cache `occt-sys` artefacts, and build the STEP worker in a separate job.
- **Toolchain pinning:** Pin with `rust-toolchain.toml` (≥ 1.95 for egui 0.37). Upgrade wgpu, egui and winit in lock-step about once a quarter.

---

## 11. Testing

| Purpose | Tools | ElektraWatt use |
|---|---|---|
| Snapshot tests | [insta](https://crates.io/crates/insta) 1.49.0 (Apache-2.0) | Model dumps after QET import; BOM / TOC / terminal / wire list outputs (CSV / JSON); AML export; DXF export text |
| Property tests | [proptest](https://crates.io/crates/proptest) 1.12.0 | Renumbering keeps references and updates BOM (defect 1); cross-ref resolution invariants; save → load is the identity; hit-test result lies within the bounding box; routing graph lengths are at least the Euclidean distance |
| Benchmarks | [criterion](https://crates.io/crates/criterion) 0.8.2; [divan](https://crates.io/crates/divan) 0.1.21 (last release 2025-04-10) | Page scene build, full-project PDF export (target: 500 pages in seconds), search, STEP cache load |
| Fuzzing | [cargo-fuzz](https://crates.io/crates/cargo-fuzz) 0.13.2 (libFuzzer, nightly) | One fuzz target per importer: QET XML, DXF, DWG, EPLAN XML, EDZ zip, AML, STEP Part 21 lexer, own project format. ClusterFuzzLite or OSS-Fuzz once public (unverified eligibility) |
| Visual regression (pages) | CPU backend ([vello_cpu](https://crates.io/crates/vello_cpu) 0.3.0 or [tiny-skia](https://crates.io/crates/tiny-skia) 0.12.0) to PNG, diffed with [dify](https://crates.io/crates/dify) 0.8.0 (used by egui_kittest) or [image-compare](https://crates.io/crates/image-compare) 0.5.0 (SSIM) | Golden PNGs per symbol and per page of a reference project. CPU rendering is deterministic across CI machines |
| Visual regression (PDF) | krilla output rasterised with [hayro](https://crates.io/crates/hayro), then image diff; veraPDF (external Java CLI) for PDF/A validation | Catches font, title-block and cross-ref regressions in the actual deliverable |
| UI tests | [egui_kittest](https://crates.io/crates/egui_kittest) 0.37.0 / [kittest](https://crates.io/crates/kittest) 0.5.0 | AccessKit-driven widget queries, image snapshots, MP4 recording of tests (0.37), accessible-name enforcement. `egui_mcp` for AI-agent exploratory testing |
| Reference corpus | The user's <drawing id> drawing set (and its `.qet` sources) | End-to-end: import → regenerate PDF → assert TOC, BOM and tag consistency. Encodes defects 1–9 as failing-then-passing tests |

---

## 12. License compatibility (ecosystem view)

| Component(s) | License | In a permissive (MIT / Apache-2.0) ElektraWatt | In a GPL-3.0-or-later ElektraWatt |
|---|---|---|---|
| egui, wgpu, vello, parley, kurbo, lyon, krilla, typst, hayro, rusqlite, serde, parry, glam, rhai, wasmtime, rfd, arboard … | MIT / Apache-2.0 / BSD / Zlib / ISC | yes | yes (Apache-2.0 is GPL-3-compatible) |
| Slint | GPL-3.0-only / Royalty-free-2.0 / commercial | Only via royalty-free 2.0: not OSI-approved, attribution required, **no API exposure**, no embedded use | yes (the distributed app is effectively GPL-3.0) |
| OCCT (occt-sys, opencascade, cadrum's OCCT) | LGPL-2.1 (+ exception) | yes (out-of-process or relinkable; OSS source availability satisfies relinking) | yes |
| LibreDWG (libredwg-sys) | GPL-3.0-or-later | **no** | yes |
| acadrust, imbl, apple-codesign | MPL-2.0 (file-level copyleft) | yes | yes |
| openbim-step | AGPL-3.0-or-later | no | possible (GPL-3 §13 allows combination) but undesirable |
| Qt via cxx-qt | LGPL-3.0 | yes (dynamic linking) | yes |
| stabby | EPL-2.0 OR Apache-2.0 | yes (Apache) | yes (Apache) |
| Bundled fonts | typically OFL-1.1 | yes (with notice) | yes |

**Ecosystem-level observation.** A permissive core (MIT / Apache-2.0) maximises contributor and corporate reuse but closes off Slint (OSI-clean) and LibreDWG. GPL-3.0-or-later keeps every option open and matches the ECAD / EDA open-source norm: KiCad (GPL-3+), LibrePCB (GPL-3), QElectroTech (GPL-2+) (unverified for QET). A common split is a GPL-3.0-or-later application with permissively licensed format crates (QET / DXF / AML / EDZ readers) to attract external contributors. The final choice belongs to the architecture decision.

---

## 13. Candidate stacks

### Stack A: "Rerun-style" pure Rust, egui shell plus own wgpu renderers (recommended)

| Layer | Choice |
|---|---|
| Shell / UI | eframe / egui 0.37 (wgpu 30, winit 0.30, AccessKit); egui_tiles 0.17 or egui_dock 0.21; egui_table 0.10 / egui_extras; egui_ltreeview 0.9; own property grid; muda for the native macOS menu bar; rfd; arboard |
| 2D canvas | Own wgpu renderer via egui_wgpu callbacks; lyon tessellation, instancing, SDF lines; parley + glyph atlas; kurbo geometry; rstar index |
| 3D | Own wgpu renderer in an egui panel; parry3d (collision, picking); meshopt; transform-gizmo-egui; petgraph / pathfinding routing |
| CAD | `ew-step-worker` process with OCCT 8 via cadrum (or occt-sys 7.8); GLB / rkyv cache |
| Output | krilla (PDF/A-3b, links, outlines); own SVG / DXF writers; dxf crate; rust_xlsxwriter; hayro for import and tests |
| Data | Text project directory (serde JSON / RON, UUIDv7); rusqlite parts DB with FTS5; zip container; imbl snapshots; salsa-style derived queries |
| Extensibility | rhai (v1); wasmtime component plugins on the LTS line (v1.x); headless core wheel via pyo3 |
| Plumbing | tracing, sentry (opt-in), velopack, cargo-packager / cargo-dist, fluent + icu |

Strengths:
- A single wgpu device shared by UI, 2D and 3D, with no texture copies or cross-toolkit glue.
- Fully Rust-native and permissively licensed, so it imposes no constraint on the project license.
- Proven pattern (Rerun).
- Fastest iteration, and immediate mode suits schema-driven property panels.
- Excellent testability (kittest, egui_mcp).
- Strong recent accessibility progress.

Weaknesses:
- Non-native look.
- Docking undocks only to in-app windows (native undocking DIY or via egui viewports).
- Pre-1.0 churn every 6–10 weeks.
- More DIY widgets (property grid, advanced tables).
- Text-editing ergonomics below native.

### Stack B: Slint chrome plus own wgpu canvas

| Layer | Choice |
|---|---|
| Shell / UI | Slint 1.18 (winit backend, `renderer-femtovg-wgpu` or skia-wgpu, `unstable-wgpu-30`); `.slint` markup with live preview; MenuBar, KeyBinding, StandardTableView; own dock and tree components |
| Canvas / 3D | Same own wgpu renderers as Stack A, rendered into wgpu textures and shown via `Image::try_from(wgpu::Texture)` / rendering notifiers, or LibrePCB-style images |
| Everything else | As Stack A |
| License | Project effectively **GPL-3.0** (Slint GPL route), or Slint royalty-free 2.0 with its attribution and API-exposure limits |

Strengths:
- Declarative UI, which designers and AI find easy to generate.
- Stable 1.x API with company support.
- Native-styled widgets, native menus and shortcuts.
- LibrePCB precedent in EDA.

Weaknesses:
- No docking or tree widgets.
- Canvas interop is labelled *unstable* and re-keyed to every wgpu major.
- Two languages (Rust plus `.slint`).
- Schema-generated property UIs are clumsier than in immediate mode.
- License constraints, including on any plugin API exposing UI.

### Stack C: Tauri 2 plus web UI plus Rust core

| Layer | Choice |
|---|---|
| Shell / UI | Tauri 2.12 (watch the 3.x CEF runtime); TypeScript UI (Solid / React) with dockview, AG Grid Community or TanStack Table, a JS tree |
| Canvas | WebGL2 / WebGPU in the webview: either a Rust → WASM renderer (wgpu on WebGPU / WebGL2) inside the page, or JS (PixiJS / custom) |
| 3D | three.js (or Rust → WASM wgpu) in the webview; meshes from the Rust core via IPC or shared buffers |
| Core | Rust library: model, rules, salsa queries, krilla PDF, importers, OCCT worker, SQLite; exposed as Tauri commands |

Strengths:
- The richest widget ecosystem (docking, grids, trees, property editors).
- Best IME and accessibility (browser engine).
- Largest contributor pool.
- Tauri 2 is stable and very widely deployed.

Weaknesses:
- **The UI is not Rust**, which contradicts the user's decision unless the canvas and 3D are WASM-Rust.
- Three webview engines (WebView2, WKWebView, WebKitGTK); WebKitGTK WebGL performance is weak (unverified, widely reported). A CEF runtime would fix this at the cost of size.
- IPC serialisation overhead for scenes with 10k+ objects and for meshes.
- Two rendering paths (web canvas vs Rust PDF) risk screen / PDF mismatch, unless the WASM display-list renderer is shared.

### Trade-off summary

| Criterion | A: egui | B: Slint | C: Tauri |
|---|---|---|---|
| Matches "written in Rust" decision | Full | Full (plus `.slint` DSL) | Partial (TS UI) |
| Canvas performance and control | Highest (shared device) | High (texture interop, unstable API) | Medium–high (WASM / WebGPU), IPC-bound |
| 3D integration | Native, same device | Via textures | Via webview |
| Docking / trees / tables out of the box | Good (crates) | Weak (DIY) | Best |
| Native look and feel | Low | Medium | Low–medium |
| Accessibility | Good and improving | Good | Best |
| License freedom | Any license | GPL-3.0 or RF-2.0 restrictions | Any license |
| Upgrade churn | High (6–10 week breaking minors; quarterly wgpu) | Low–medium (stable 1.x, unstable wgpu feature) | Low for Tauri 2; JS ecosystem churn |
| AI-assisted development fit | High (egui_mcp, kittest) | High (DSL, `mcp` feature) | High (web) |
| Overall risk for a 2–4 person team | **Lowest** | Medium | Medium–high (two stacks) |

Not recommended now, to re-evaluate in 2027:
- **gpui:** no stable release channel; no custom GPU passes for 3D; weak accessibility.
- **iced:** wgpu lag, no mainline a11y, slow release cadence.
- **Dioxus native / Blitz:** beta.
- **Xilem / Masonry:** experimental.
- **Floem, Makepad, Freya:** adoption and bus-factor risk.
- **GTK4 / Qt:** not Rust-native, heavy toolchains.

---

## Implications for ElektraWatt

1. **Adopt Stack A (egui 0.37 + own wgpu 2D/3D renderers)** as the baseline. Wrap panels behind a thin internal `ew_ui` layer so that a later move of the chrome (e.g. to Slint or Xilem) never touches the canvas, 3D or model crates.
2. **One display-list IR, many backends.**
   - Backends: screen (wgpu), PDF (krilla), SVG, DXF (dxf crate) and PNG (vello_cpu).
   - One text stack (parley or cosmic-text, harfrust, skrifa) with **bundled fonts**, embedded in PDFs.
   - Title blocks, cross-ref labels and reports then look the same everywhere, which fixes the class of defects 5–7.
3. **Canvas implementation rules:**
   - Tessellate symbol definitions once (lyon) and instance them.
   - Draw lines as screen-space-width shader primitives.
   - Use a per-page rstar index and exact kurbo picking.
   - Never re-tessellate on zoom.
   - Target: 60 fps on integrated GPUs for a dense A3 page; full-project PDF export of 500 pages in under 10 s (targets to be validated).
4. **3D = own renderer on the same wgpu device.**
   - Model maths in glam f64 (mm).
   - parry3d 0.31.1 for collision, clearance and picking.
   - Routing graph with petgraph / pathfinding, duct-fill and length calculation.
   - **Exclude rend3 (archived 2025-06-07) and Fornjot (shut down 2026-06-19)**; do not embed Bevy.
5. **STEP via OCCT, out-of-process.**
   - Start with cadrum (prebuilt static OCCT 8.0.1, MIT wrapper); occt-sys / opencascade-rs is the fallback.
   - Define a stable IPC / cache contract: assembly tree, LOD meshes, feature edges, hulls, mounting frames.
   - Keep a pure-Rust evaluation track (truck master or monstertruck) behind the same contract.
6. **PDF strategy:**
   - krilla for all drawing output, targeting **PDF/A-3b** with embedded machine-readable project data (AML / CSV / JSON).
   - A named destination per cross-reference target, so "102-A1" is clickable, and outlines generated from the TOC.
   - hayro for importing PDFs as underlays and for PDF regression tests.
   - Typst only for free-form documents; reports use an own form-based engine on the display list.
7. **Import / export crates:**
   - dxf 0.6.2 for DXF.
   - Evaluate acadrust (MPL-2.0) for DWG on a real corpus before committing. LibreDWG only if the project is GPL-3-compatible.
   - quick-xml for AML and EPLAN XML, roxmltree for QET, zip for EDZ / AMLX.
   - calamine and rust_xlsxwriter for Excel, csv for CSV.
8. **Persistence:**
   - A git-friendly **text project directory** with deterministic serialisation, integer coordinates and UUIDv7 IDs. Tags are derived properties, never keys.
   - SQLite (rusqlite, bundled, FTS5) only for parts catalogues, indices and caches.
   - A zip `.ewz` container for transport.
   - **Never bincode** (unmaintained since 2025-12); use rkyv or postcard for caches.
9. **Model core:**
   - Immutable snapshots (imbl) plus explicit, serialisable operations for undo / redo, macros and background jobs.
   - Derived data (tags, cross-refs, potentials and wire properties, TOC, BOM with quantities, terminal / cable / wire / PLC lists) as memoised queries (salsa or hand-rolled), never stored. This structurally eliminates defects 1, 2, 4, 6, 7 and 8.
10. **Collaboration:** no CRDT in v1. Make the operation log and IDs CRDT-ready, ship a semantic git merge driver, and keep Loro as the candidate if real-time co-editing is ever required.
11. **Extensibility:**
    - rhai for rules and expressions in v1.
    - wasmtime component plugins on an LTS line (48.x, assuming LTS per the every-12th rule) for importers / exporters in v1.x.
    - The headless core as a Python wheel (pyo3) instead of an embedded interpreter.
    - No native dylib plugins.
12. **Printing:** treat PDF as the print master. Implement three small adapters (ashpd Print portal or CUPS on Linux, PDFKit `NSPrintOperation` on macOS, `PrintDlgEx` + banded GDI rendering on Windows), plus a "open in system PDF viewer" fallback.
13. **Distribution:**
    - cargo-packager (or cargo-dist) for MSI / NSIS, DMG and AppImage / deb; Flathub for Linux.
    - velopack for optional delta updates.
    - rcodesign for macOS notarisation in CI.
    - Decide the Windows signing route (SignPath OSS vs Azure Artifact Signing) and who owns the identity.
14. **Quality gates in CI from day one:**
    - nextest, insta, proptest.
    - cargo-fuzz targets per importer.
    - CPU-rendered golden-page diffs (dify), hayro-based PDF diffs plus veraPDF validation.
    - egui_kittest UI tests with accessible-name enforcement.
    - cargo-deny license policy matching the chosen project license; cargo-about notices.
15. **Version hygiene:**
    - Pin the toolchain (≥ 1.95). Upgrade egui, wgpu and winit together once a quarter.
    - Avoid crates that pin old wgpu majors (iced 0.14 is on wgpu 27; vello_hybrid 0.2 on wgpu 29; masonry 0.4 on vello 0.6).
    - Treat 2026-vintage crates with under 10k downloads as watch-list only.
16. **The ecosystem does not provide these; ElektraWatt must build them:**
    - CAD canvas and symbol editor.
    - Docking polish (native undock), property grid.
    - Form-based report engine and the PDF cross-ref link model.
    - Print adapters.
    - Cabinet routing and duct-fill logic.
    - EPLAN, QET and EDZ importers, AML / TIA exchange.
    - DWG robustness testing.

   Plan staffing for these explicitly.
17. **License recommendation (ecosystem input):** GPL-3.0-or-later for the application keeps Slint, LibreDWG and OCCT available and follows ECAD / EDA open-source practice. Publish reusable format crates under Apache-2.0 OR MIT to attract external contributors. A fully permissive project is also feasible with Stack A, at the cost of LibreDWG and OSI-clean Slint.

## Open questions

1. **Project license:** GPL-3.0-or-later or permissive? This gates Slint (Stack B), LibreDWG and the plugin-API wording.
2. **Accessibility requirement:** Is screen-reader accessibility a v1 requirement, given EU accessibility legislation and corporate procurement policies? It changes how much weight egui's progress vs Tauri's browser-grade a11y should carry.
3. **Native look:** Is a non-native, custom look acceptable to panel-builder users coming from EPLAN's Windows-native UI (ribbon, docking)?
4. **DWG scope:** Is DWG import a v1 must, or is DXF plus "convert with ODA File Converter" acceptable? If DWG is required, acadrust needs a corpus test (R2000–R2018 files from Rittal, Siemens and customer archives).
5. **STEP scope:** Is exact B-rep geometry required in v1, e.g. mounting-plate drilling and cut-out data for Rittal Perforex or machining, or are tessellated meshes enough? Exact geometry pulls OCCT deeper, possibly in-process.
6. **Performance targets:** What are the target project size and hardware (largest projects in pages and objects; integrated GPUs; **Citrix / RDP / VDI sessions without a GPU**)? The last needs a validated CPU fallback (wgpu on WARP / llvmpipe, or vello_cpu) and measured performance.
7. **Collaboration:** Will real-time multi-user editing ever be needed, or is git-based / file-based collaboration (with locking) enough? This decides whether to reserve architecture for Loro.
8. **Plugin languages:** Which do users expect? EPLAN's API is .NET, so some users may want C#. WASM components can host several languages; C# support via componentize-dotnet is unverified.
9. **Code signing:** Who holds the identities (the pilot panel builder or a project foundation)? Is a free OSS signing programme acceptable for corporate customers' IT policies?
10. **Upgrade budget:** Can the team absorb quarterly breaking upgrades (egui, wgpu, salsa, wasmtime monthly / LTS)? Or should it prefer fewer, stabler dependencies even at the cost of more DIY?
11. **Facts to re-check** (marked unverified above):
    - iced mainline accessibility status.
    - Whether gpui exposes custom GPU passes.
    - Slint `StandardTableView` behaviour at 100k rows.
    - Exact OCCT LGPL exception wording and the static-linking stance.
    - Current GitHub Actions runner labels.
    - 2026 eligibility for SignPath / Azure Artifact Signing.
    - EDZ / AMLX archive internals.
