# ADR-0014: GUI toolkit: egui/eframe behind `ew-ui`, over a toolkit-agnostic `ew-viewmodel`

**Status:** Accepted · amended 2026-10-18 · as-built notes 2026-10-22 (cycles 19 and 20) · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the architecture judge panel)

## Context

**What the shell must provide** ([UX/UI](../design/16-ux-ui.md)):
- docking with saved workspaces (Schematic, PLC, Fluid, Panel 3D, Manufacturing) and second-monitor viewports;
- navigator trees and tables virtualised to 100,000 rows (NFR-PERF-15);
- a schema-driven property grid with provenance ("0.75 mm² · from potential 24VDC-1 via profile 'DBU/WH 24 V'");
- **one GPU device** shared by the UI, the 2D canvas (ADR-0015) and the 3D panel view (ADR-0016);
- accessibility (UI Automation, NSAccessibility, AT-SPI) and keyboard-first operation (NFR-A11Y-01, NFR-A11Y-02);
- automated UI tests;
- Rust only, licence-compatible with GPL-3.0-or-later plus a plugin exception (ADR-0024, ADR-0025).

**Candidates** ([Rust ecosystem](../research/rust-ecosystem.md) §2, state 2026-10-17):

| Toolkit | Fit | Problems |
|---|---|---|
| egui/eframe 0.37 | Docking (egui_tiles 0.17.1), trees (egui_ltreeview 0.9.0), tables (egui_table 0.10.0), AccessKit by default, `egui_wgpu` paint callbacks on the app's wgpu 30 device, egui_kittest. Rerun builds a comparable 2D/3D data viewer on it | Breaking minors every 6–10 weeks; non-native look; immediate mode means the app owns all state; weaker text editing |
| Slint 1.18 | Stable 1.x API, company backing, native-styled widgets | No docking, no tree view (about 12–20 dev-weeks of DIY widgets); wgpu interop is an unstable feature renamed with every wgpu major; GPL-3.0-only or a royalty-free licence that forbids distributing an application that exposes its APIs |
| iced 0.14 | Elm architecture | Pinned to wgpu 27; no mainline AccessKit (unverified); self-described experimental |
| gpui | Excellent keyboard and text UX | No stable crates.io channel; no exposed GPU device for custom passes; weak accessibility |
| Tauri 2 | Mature, web UI | The UI would be TypeScript, not Rust; no shared wgpu device |

## Decision

1. **Toolkit.** egui/eframe 0.37 with egui_tiles, egui_table and egui_ltreeview; muda 0.21.2 for native menus and
   rfd 0.17.2 for native file dialogs.
2. **Confinement.** Only `ew-ui` and the `elektrawatt` binary know egui; only the renderers know wgpu
   ([architecture](../design/04-architecture.md) §3, rule 3; enforced by CI, NFR-MAINT-01). Amendment 3 adds the
   wiring viewer.
3. **View-model layer.** `ew-viewmodel` provides toolkit-agnostic panel models: navigator trees, table rows with their
   edit → command mapping, property-grid rows with provenance, message lists. Each is a plain Rust structure computed
   from queries and stamped with the revision it reflects. egui calls them every frame, which is cheap when memos hit
   (ADR-0002). A retained toolkit could subscribe to the change feed instead (FR-UI-037).
4. **Canvas tools** are state machines in `ew-ui` that return `Transient(overlay)`, `Preview(Command)` or
   `Commit(Command)` (ADR-0008); they never mutate the model.
5. **Upgrades.** egui, eframe, egui-wgpu, wgpu, winit and the egui_* crates are upgraded together, once a quarter, at
   a target of about one developer-day (NFR-MAINT-04). The toolchain is pinned in `rust-toolchain.toml` (≥ 1.95
   for egui 0.37) and raised with the same train (NFR-MAINT-03).
6. **Testing and accessibility.** egui_kittest flows run in CI and enforce accessible names. The egui
   `inspection` feature and `egui_mcp` are compiled **only** into test builds; CI asserts that release binaries do not
   contain the inspection code path, because it listens on a TCP port when enabled (NFR-SEC-09).

## Amendments

Accepted on 2026-10-18. The decision text above is unchanged; these entries refine it.

1. **Menus on Linux** (2026-10-18; [UX/UI](../design/16-ux-ui.md), open question 1). muda attaches menus only to GTK
   windows, which eframe/winit does not create (**verify**). EW therefore uses native menus through muda on macOS
   and Windows and draws the menu bar with egui on Linux. All menus are generated from one action registry.
   FR-UI-001 asks for native menus on macOS and Windows.
2. **Accessible-name coverage** (2026-10-18; same document, open question 6). egui_kittest fails only on unnamed
   *input* widgets, while NFR-A11Y-02 covers all interactive widgets. An `ew-ui` test helper therefore walks the
   AccessKit tree after every kittest flow and fails on any unnamed focusable node.
3. **Wiring viewer** (2026-10-18; [manufacturing outputs](../design/13-manufacturing-outputs.md),
   [architecture](../design/04-architecture.md) §3). The read-only `.ewpp` wiring viewer of 1.0 is built on
   `ew-ui` widgets. Rule 3 therefore reads: only `ew-ui`, `ew-viewer`, the app binary and the `ew-wiring-viewer`
   binary know egui; only the renderers know wgpu.

## As built (cycle 19)

A note, not an amendment: the decision is unchanged. WP-S2.01 part 1 (decision log ID-473 to ID-477) built the first
shell on this ADR: egui, eframe and egui_kittest `=0.37.0`, eframe on the wgpu renderer (decision 1's shared device)
without glow, persistence or egui's default fonts, rfd `=0.17.2` on the XDG portal. Three facts the decision text did
not foresee: AccessKit is a hard dependency of egui 0.37, not a feature, so it is always on; muda is deferred, so the
menu bar is drawn with egui on every OS until FT-310 (amendment 1 stays the target; decision log ID-446); and the
crates that name egui declare `rust-version = "1.95"` (decision 5). Only `ew-ui` and `elektrawatt` name egui or
eframe, and neither depends on wgpu directly (decision 2). The accessible-name walker of amendment 2 runs after every
kittest flow. The UI graph adds 296 crates, all cargo-vet exemptions until the lead's audit (FT-326).

## As built (cycle 20)

A note, not an amendment: the decision is unchanged. WP-S2.01 part 2 (decision log ID-486 to ID-493) added docking with
**egui_tiles 0.18.0, not the 0.17.1 of the context table**: 0.17.1 needs egui 0.36, and 0.18.0 is the release for egui
0.37, the version of decision 1 (ID-490). It is pinned `=0.18.0` with default features off, so no serde: the workspace
files are not the serialised egui_tiles tree but the toolkit-neutral `ew.workspace/1` that `ew-viewmodel` owns and
`ew-ui` converts (ID-489; [UX/UI](../design/16-ux-ui.md) §2.5), which keeps egui behind `ew-ui` as rule 3 asks. The
dock's splitter handles have no accessible name in 0.18.0, so amendment 2's walker skips exactly as many unnamed handles
as the dock has splitters until they are named (FT-338). The lead's audit (FT-326, decision log ID-498) audited the
native-boundary crates that FT-326 named or found them covered by an imported set; the rest of the graph, egui,
eframe, egui_tiles and the wgpu 30.0.1 front-end crate among it, stays exempt (385 exemptions at the integration tip).

## Consequences

### Positive

- Fastest delivery for a 2–4 person team: one language, one GPU device, existing docking, tables and trees.
- The Rerun pattern (egui shell plus own wgpu renderers) is proven at comparable scale.
- AccessKit names are enforced by tests from the first UI commit.
- A later chrome swap replaces only `ew-ui`; view-models, tools' commands and everything below stay.
- UI tests and AI-assisted UI work can drive the app in test builds without exposing anything in releases.

### Negative

- The look is not native; native menus (muda) and dialogs (rfd) mitigate this on macOS and Windows. On Linux the
  menu bar is drawn by egui (amendment 1).
- Quarterly lock-step upgrades are recurring work, and a skipped quarter makes the next one larger.
- Immediate mode puts all UI state in the application; view-models must be cheap to recompute every frame.
- Text editing in egui is weaker than in native toolkits; long multi-line texts (comments, function texts with
  several languages) need care.
- AccessKit's platform adapters do not yet support rich text or hypertext.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **Slint** | About 12–20 weeks of DIY docking, trees, tables and property grid; unstable wgpu interop; licence coupling: under GPL-3.0-only, Slint's code would carry no plugin exception (the problem C-8 describes for ported QET code), and the royalty-free terms forbid distributing an application that exposes Slint's APIs |
| **iced** | Lags wgpu by three majors; no mainline accessibility |
| **gpui** | No stable release channel; no custom GPU passes |
| **Tauri / Dioxus desktop** | The UI would not be Rust; no shared wgpu device for the canvases |
| **relm4/GTK4, cxx-qt** | Bind C/C++ toolkits; foreign look outside their home platform or a three-language codebase |
| **Own UI toolkit** | Out of scope for the team size |

## Related

- **ADRs:** [ADR-0002](0002-derived-data-engine-ew-memo.md), [ADR-0008](0008-transactions-undo-preview.md),
  [ADR-0015](0015-display-list-renderers-pdf.md), [ADR-0016](0016-3d-renderer-wgpu-and-out-of-process-step-worker.md), [ADR-0020](0020-extensibility-tiers-and-security-model.md), [ADR-0022](0022-platform-matrix-and-performance-budgets.md), [ADR-0023](0023-supply-chain-and-release-integrity.md), [ADR-0024](0024-licence-set-and-permissive-gpl-boundary.md).
- **Design:** [architecture](../design/04-architecture.md) §3, §9, §16 (risk 6); [UX/UI](../design/16-ux-ui.md);
  [quality, security and release](../design/18-quality-security-release.md) §6.
- **Requirements:** FR-UI-001, FR-UI-002, FR-UI-004, FR-UI-007, FR-UI-008, FR-UI-009, FR-UI-010, FR-UI-016,
  FR-UI-026, FR-UI-027, FR-UI-037, NFR-A11Y-01, NFR-A11Y-02, NFR-PERF-04, NFR-PERF-15, NFR-PLAT-01, NFR-MAINT-01,
  NFR-MAINT-03, NFR-MAINT-04, NFR-SEC-09.
