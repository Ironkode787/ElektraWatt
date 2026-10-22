# WP-S2.01 part 1: the application shell (closed specification)

**Status:** Delivered (part 2, cycle 20, from the part-2 list of §9 and its follow-up tasks: FT-307, FT-308, FT-309, FT-311, FT-312, FT-313's journal discard and FT-327, with FT-310 deferred; deviations as built in decision log ID-486 to ID-493: egui_tiles 0.18.0, the toolkit-neutral workspace file, the wizard writing only existing commands, the empty-directory rule, the `SaveError` re-export) · Delivered (part 1, cycle 19; superseded by the code and the work package's status line; deviations as built in decision log ID-473 to ID-477: AccessKit always on, `rust-version` 1.95, the optional `read_only` and `query` parameters, `ui.dismiss`, the journal kept on a dirty exit, headless configuration isolation) · specified for cycle 19 (one programmer lane, D3; Opus review) · **Date:** 2026-10-22 · **Work package:**
[WP-S2.01](../02-work-packages.md#wp-s201-application-shell-first-run-and-company-standard-wizard-ui) · **Decision
log:** ID-440 to ID-449 · **Part of:** ElektraWatt plan, specifications

**Purpose.** This is the closed specification that the cycle-19 lane executes without further design judgment: the
first UI step of M-PP, an egui shell that opens, shows, undoes, saves and exports projects through the `Session` of
`ew-api` only, and that CI and the blind drafter can drive without a display. Where this document and a design
document differ, this document is the narrower and wins for the lane; the design documents stay the target of the
whole work package. A choice the lane meets that this document does not settle is escalated, never guessed
(`CLAUDE.md`, rule 2).

**Why it lives here.** Closed specifications go to `docs/plan/specs/`, one file per work package or part, named
`<WP-ID>-<slug>.md` (decision log ID-441). A specification is handed down once, executed by one lane and then
superseded by the code and the work package's status line; like the gate records in `docs/plan/gate-records/` it is a dated
document beside the standing ones, not a section of the WBS, which stays one table per work package.

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs](#2-inputs)
3. [Crates and boundaries](#3-crates-and-boundaries)
4. [Dependencies and the supply chain](#4-dependencies-and-the-supply-chain)
5. [What the shell contains at the end of part 1](#5-what-the-shell-contains-at-the-end-of-part-1)
6. [The headless mode](#6-the-headless-mode)
7. [Acceptance checks](#7-acceptance-checks)
8. [Files owned](#8-files-owned)
9. [Out of scope](#9-out-of-scope)
10. [Review checklist for the Opus reviewer](#10-review-checklist-for-the-opus-reviewer)
11. [Size](#11-size)
12. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of part 1, `elektrawatt` starts to a start page, opens an existing project directory through
`ew_api::Session::open`, shows its pages read-only (the CPU renderer's image of one page at a time), moves between
pages, undoes and redoes, saves, saves as, exports the PDF, closes and quits; every one of these is an action of one
toolkit-neutral registry, reachable from the menu bar, the command palette and a keyboard shortcut; and
`elektrawatt --headless --script <file>` performs the same actions on the same code path with no window, no GPU and
no display, writing a deterministic transcript.

**Non-goals of part 1** (each is a follow-up task of
[Appendix B](../02-work-packages.md#appendix-b-follow-up-tasks-from-the-implementation-cycles) or another work
package, §9): creating projects, the first-run flow and the company-standard wizard, docking and named workspaces,
native menus, F1 and the complete About dialog, the GPU canvas, zoom and pan, any editing tool, the property grid,
navigators, the message panel, jobs with progress and cancel, keymap files, localisation.

## 2. Inputs

| Input | What the lane takes from it |
|---|---|
| [ADR-0014](../../adr/0014-gui-toolkit-egui.md) and its amendments | egui/eframe 0.37 behind `ew-ui` over `ew-viewmodel`; the accessible-name walker (amendment 2); inspection and `egui_mcp` only in test builds |
| [Architecture](../../design/04-architecture.md) §3 (rules 1, 3, 6, 7), §9, §10.1 | The crate table, the dependency rules, the UI shell table, the smoke flag `elektrawatt --smoke <project>` |
| [UX/UI](../../design/16-ux-ui.md) §1, §2.1, §2.4, §2.5, §3.2, §3.6, §3.7, §4.1 to §4.3 | The client rule (the UI writes only through `Session`), the action registry, settings, menus, status bar, start page, Escape and input routing, the palette |
| [Schematic editor](../../design/06-schematic-editor.md) §6 | The shortcuts of §5.4 (Page Down and Page Up, Ctrl+G, Home, Escape, undo and redo) |
| [Quality](../../design/18-quality-security-release.md) §3.11, §3.15, §3.16, §5.2, §11.2 | UI tests, the no-bypass rule for scripted origins, the cross-platform source rules, CI-PR-05 and CI-PR-07, the `unsafe` policy |
| `crates/ew-api` (`src/lib.rs`, `src/output/`, `src/cmdlog.rs`) | `Session::open`, `open_read_only`, `create`, `save`, `close`, `undo`, `redo`, `history`, `is_dirty`, `page_labels`, `export_pages`, `export_pdf`, `run_script`, `CmdLog::parse`, `output::font_bytes`, `StoreOptions::new`, `Recovery` |
| `bin/ew/src/cli.rs` (`ew apply`) and `bin/ew/tests/cli.rs` | The open pattern (store options, read-only and recovery handling) and the test pattern (a temporary copy of Golden A) |
| `crates/ew-api/tests/page_output/golden_files.rs` (`page_png`) | How a page becomes the 150 dpi golden PNG, and how the golden PNGs are compared |
| `ci/dep-rules.toml`, `ci/licence-map.toml`, `ci/build-scripts.allowlist`, `deny.toml`, `supply-chain/config.toml` | The checks the new dependencies must pass (§4) |

## 3. Crates and boundaries

No crate is created: the three crates exist as skeletons since WP-S0.01, with `license = "GPL-3.0-or-later"`, the
REUSE header in every file, `rust-version = "1.92"`, membership through the workspace glob and their rows in
`ci/licence-map.toml` (`crates/ew-viewmodel/**`, `crates/ew-ui/**`, `bin/**`) and `ci/dep-rules.toml`. Part 1 fills
them and changes none of these rows. Every new file carries the two-line SPDX header of its neighbours
(`GPL-3.0-or-later`); test fixtures written by the lane carry it too or an entry in `REUSE.toml`.

| Crate | Layer | Part 1 content | Workspace edges (unchanged) |
|---|---|---|---|
| `ew-viewmodel` | GPL | The toolkit-neutral **action registry** (§5.2), the **shell model** `ShellModel` (all application state and `dispatch`), the settings file, the UI-script parser and runner core (§6), the page-list and status-bar models. No egui, no wgpu, no winit, no renderer | `ew-api` only |
| `ew-ui` | GPL | The egui frame over `ShellModel`: menu bar, start page, page view, status bar, palette, About, dialogs; key routing; the page raster through `ew-render-cpu` on a worker thread; the headless runner `ew_ui::headless` (which adds the raster to the core runner); the bundled UI fonts; the accessible-name walker for tests | `ew-viewmodel`, `ew-render-cpu` (and the existing skeleton edges to `ew-render2d`, `ew-render3d`, unused in part 1) |
| `elektrawatt` (bin) | GPL | `main`: argument parsing (`--headless`, `--script`, `--record`, `--smoke`, an optional project path), the eframe window, rfd file dialogs, exit codes | `ew-ui`, `ew-print` (unused in part 1) |

**Rules the lane must keep** (decision log ID-444):
1. **Through `Session` only.** The shell holds `Option<ew_api::Session>` inside `ShellModel` and reaches the model
   only through `Session` methods and queries. It names no `ew-model` type, enables no `raw-read` feature and calls
   no raw-read item; `cargo xtask check-raw-read` stays green.
2. **No new workspace edge** in `ci/dep-rules.toml`. Types of crates below `ew-api` are named through re-exports
   (architecture §3 rule 7): `ew-viewmodel` adds `pub use ew_api;` at its root, and `ew-ui` names `Session` and the
   output types as `ew_viewmodel::ew_api::…`. A field of a foreign type is used without naming the type where
   possible (for example `SheetSize { width: page.width, height: page.height }`). If a type is still unreachable,
   the lane stops and reports: an edge is an architecture change.
3. **No new public item in `ew-api`.** Part 1 needs none. If the lane finds one missing, it stops and reports (a
   public API is lead-programmer work).
4. **Only `ew-ui` and `elektrawatt` name egui or eframe** (rule 3; the `[[external_owner]]` rule of
   `ci/dep-rules.toml` already enforces it). Neither depends on `wgpu` directly.
5. **No `unsafe`.** Every crate root keeps `#![forbid(unsafe_code)]`; the workspace lint stays `forbid`. A feature
   that needs `unsafe` in our code (the muda attach on Windows) is out of scope (§9).
6. **No unordered output.** Transcripts, settings files, the describe text and menus iterate `Vec`, `BTreeMap` or
   `BTreeSet`, never a `HashMap` or `HashSet` (doc 18 §3.5).

## 4. Dependencies and the supply chain

**Versions** (exact, as [architecture](../../design/04-architecture.md) §3 pins them; decision log ID-447):

| Crate | Dependency | Spec | Why |
|---|---|---|---|
| `ew-ui` | `egui` | `=0.37.0`, `default-features = false`, `features = ["accesskit"]` | The widgets. `default_fonts` is off: egui's default fonts include the Ubuntu font under its own licence, which `deny.toml` does not allow; the UI uses the bundled Noto Sans of `ew-text` through `ew_api::output::font_bytes()` |
| `ew-ui` (dev) | `egui_kittest` | `=0.37.0`, default features off (no snapshot or wgpu feature) | Interaction tests and the accessible-name walker, without rendering, on all three OSes (doc 18 §3.11) |
| `ew-viewmodel` | `serde_json` | `1.0.154` (already locked) | UI-script lines, command parameters, the transcript |
| `ew-viewmodel` | `toml` | `1.1.8` (already locked), default features only | `settings.toml`; a `toml::Table` keeps unknown keys, sorted |
| `elektrawatt` | `eframe` | `=0.37.0`, `default-features = false`, `features = ["accesskit", "wgpu", "wayland", "x11"]` | The window on the wgpu renderer, so that WP-S2.02 paints into the same device (ADR-0014 decision 1); no `glow`, no `persistence`, no `default_fonts` |
| `elektrawatt` | `rfd` | `=0.17.2`, `default-features = false`, the XDG-portal backend on Linux (no GTK) | Open and Save As dialogs. The lane picks the portal's async-runtime feature that adds the fewest crates to `Cargo.lock` and names it in the commit message |

Not added in part 1: `egui_tiles`, `egui_table`, `egui_ltreeview`, `transform-gizmo-egui`, `muda`, `arboard`,
`egui-wgpu` as a direct dependency, `fluent-bundle`. The `inspection` feature of egui and the `egui_mcp` crate are
banned outside tests (`check-features`); part 1 uses neither, not even in tests.

**What the lane adds to the policy files, in the same `build(deps)` commit:**
1. `Cargo.lock`, generated by cargo; `--locked` builds pass afterwards.
2. `supply-chain/config.toml`: one `[[exemptions.<crate>]]` per new crate and exact version, `safe-to-deploy` for
   everything in the graph of `elektrawatt`, `safe-to-run` for crates only `egui_kittest` brings (decision log
   ID-332). The commit message lists the exempted crates. The lane writes **no** `[[audits]]` entry and imports no
   audit set: audits are the lead's (ID-4, ID-332), and the Opus reviewer checks the exemption list.
3. `ci/build-scripts.allowlist`: each new package with a build script, after reading its `build.rs`, with a comment
   line saying what it does (no network, no download, writes only under `OUT_DIR`). A build script that downloads or
   reaches the network stops the lane (C-5).
4. `deny.toml`: no change, except `BSL-1.0` in `[licenses].allow` if and only if `cargo deny check licenses` names it
   (the clipboard crates of `egui-winit` on Windows use it; it is OSI-approved and GPL-compatible; pre-approved by
   ID-447). Any other new licence, and any `LicenseRef-*`, stops the lane.
5. No system package on any CI image. If a `-sys` crate needs a system library through `pkg-config`, the lane stops
   and reports; it does not edit the workflows.

**Checks the lane runs for the dependency set** (its change is the dependency set, so these are the targeted policy
checks of the testing policy): `cargo deny check`, `cargo vet --locked`, `cargo xtask check-deps`,
`check-features`, `check-build-scripts`, `check-licence-map`, `python -m reuse lint`.

## 5. What the shell contains at the end of part 1

### 5.1 The shell model

`ew_viewmodel::shell::ShellModel` owns all application state: the settings, the optional open project (its
`Session`, directory, access mode, open report summary), the current page index, the last error or banner, the
palette state, and the record of committed commands (§6.4). Its one mutating entry point is
`dispatch(&mut self, action: &ActionId, params: &serde_json::Value) -> Outcome`; `Outcome` is `Done`,
`Refused { reason_key }` (a disabled action, with the reason of §5.2) or `Failed { message }`. `ew-ui` calls
`dispatch` for every menu item, palette entry, shortcut and dialog result, and reads its getters to paint. The
headless runner calls the same `dispatch` (§6). No other code path changes `ShellModel` or the `Session`.

**Project lifecycle** (every step through `ew-api`):

| Situation | Behaviour |
|---|---|
| Open | `Session::open(dir, StoreOptions::new(version), SessionOptions { origin: Origin::Gui, .. }, Recovery::Keep)`. The window title becomes `<directory name> - ElektraWatt`, with `*` before it while `is_dirty()` |
| Lock held by another session | `Access::ReadOnly(reason)`: the project opens read-only; a banner names the reason; Save, Undo and Redo are disabled with the reason key `reason-read-only` |
| A leftover journal (`OpenReport::recovery` is set) | The editing session is closed again and the project re-opens through `Session::open_read_only`, with the banner "This project has unsaved changes from a session that ended unexpectedly. It is open read-only; recovery comes with a later version." Recovery itself is FT-313 |
| Integrity findings in the open report | A banner with their count; their list waits for the message panel (WP-S2.09) |
| Close or Quit while dirty | GUI: a dialog Save / Discard / Cancel. Headless: the step fails unless `params.discard` is `true` |
| Save | `Session::save`; a `SaveError` becomes a banner and leaves the project dirty |
| Save As | `Session::create(dir, StoreOptions::new(version))` into an empty directory chosen with rfd (headless: `params.dir`) |
| Close | `Session::close`; back to the start page |
| Export PDF | `Session::export_pdf(&ExportOptions::default())` and a write of the bytes to the chosen path. Synchronous in part 1, with the label "Exporting PDF..." in the status bar; the job form with progress and cancel is FT-313. The bytes equal those of `ew export pdf` on the same project with `SOURCE_DATE_EPOCH` unset |
| Undo, Redo | `Session::undo`, `Session::redo`; enabled from `Session::history()`, disabled with `reason-nothing-to-undo` / `reason-nothing-to-redo` |

**Settings** (FR-UI-033, FR-UI-049): `settings.toml` with `version = 1`, `recent_projects` (at most 10, most recent
first, no duplicates, paths that no longer exist kept until opened and then removed with a banner) and `ui_scale`
(one of 1.0, 1.25, 1.5, 2.0). The directory is `%APPDATA%\ElektraWatt` on Windows,
`~/Library/Application Support/ElektraWatt` on macOS and `$XDG_CONFIG_HOME/elektrawatt` (else `~/.config/elektrawatt`)
elsewhere, overridden by the environment variable `EW_CONFIG_DIR` (tests always set it). Unknown keys are kept and
written back; an unreadable file yields defaults, is copied to `settings.toml.bad-<n>` (the lowest free `n`) and
raises a banner. Nothing is written into a project directory except by `save` and `create`.

### 5.2 The action registry

`ew_viewmodel::actions` defines the registry of [UX/UI](../../design/16-ux-ui.md) §2.4 in toolkit-neutral form
(decision log ID-444): `ActionMeta { id: &'static str, kind: View | Command, label_key: &'static str, category,
shortcut: &'static [&'static str], enabled: fn(&ShellModel) -> Result<(), &'static str> }`. The reason of a failed
predicate is a string key; English texts for all keys live in one module `ew_viewmodel::strings` (a `match` from key
to `&'static str`), so that WP-S2.08 replaces it with Fluent without touching call sites. No user-facing literal
appears elsewhere in `ew-viewmodel` or `ew-ui`.

The part-1 actions and their default bindings (the "EW" preset of doc 16 §4.4 and doc 06 §6; on macOS `Ctrl`
reads `Cmd`, done by `ew-ui`):

| ID | Menu | Shortcut | Enabled when | Params (headless) |
|---|---|---|---|---|
| `file.open` | File | Ctrl+O | always | `dir` |
| `file.open_recent` | File ▸ Open recent | — | the list is not empty | `index` |
| `file.save` | File | Ctrl+S | a writable project is open | — |
| `file.save_as` | File | Ctrl+Shift+S | a project is open | `dir` |
| `file.export_pdf` | File ▸ Export | Ctrl+Shift+E | a project with at least one page is open | `path` |
| `file.close` | File | Ctrl+W | a project is open | `discard` (bool, optional) |
| `app.quit` | File | Ctrl+Q | always | `discard` (bool, optional) |
| `edit.undo` | Edit | Ctrl+Z | something to undo | — |
| `edit.redo` | Edit | Ctrl+Shift+Z, Ctrl+Y | something to redo | — |
| `page.next`, `page.previous` | Page | Page Down, Page Up | a project is open and a next / previous page exists | — |
| `page.first`, `page.last` | Page | — | a project with pages is open | — |
| `page.goto` | Page | Ctrl+G | a project with pages is open | `folio` (a printed label of `page_labels()`) |
| `view.fit_page` | View | Home | a page is shown (part 1 always fits; the action exists for the binding) | — |
| `view.ui_scale` | View ▸ UI scale | — | always | `scale` |
| `tools.palette` | Tools | Ctrl+K, Ctrl+Shift+P | always | — |
| `help.shortcuts` | Help | — | always | — |
| `help.about` | Help | — | always | — |
| `command.run` | — (palette and headless only) | — | a writable project is open | `cmd`, `params` (§6.3) |

Headless-only actions (no menu entry): `view.snapshot` (`path`, `dpi`), `ui.describe` (§6.2).

Every model command of `Session::registry()` also appears in the palette, labelled by its name and disabled with
the reason key `reason-parameters-later` in the GUI: parameter forms are WP-S2.08. They run only through
`command.run` in the headless mode in part 1.

### 5.3 The window

**Menu bar.** Drawn with egui on all three operating systems in part 1 (decision log ID-446), generated from the
registry: File, Edit, View, Page, Tools, Help, in that order, each entry with its accelerator text and greyed when
disabled (its reason in the tooltip). Native menus through muda are FT-310.

**Start page** (doc 16 §3.7, part-1 subset): Open..., the recent projects (each a button), and the line "Update check:
off" (there is no update check; nothing on the page opens a connection, NFR-PRIV-01). New project, Open sample
project, Import QET project, Create company standard, tutorials and the quick switches are FT-307, FT-308 and
FT-312. The start page appears before any project is touched; no work that could take time runs before the first
frame (NFR-PERF-02; the 1.5 s budget is measured only at the milestone run).

**Project view.** A fixed layout of egui panels (docking is FT-309): left, the **page list** (the printed folio
labels of `Session::page_labels()`, the current one selected; a click is `page.goto`); centre, the **page view**;
bottom, the **status bar**. The page list is a plain list, not the page navigator of WP-S2.03.

**Page view** (decision log ID-445). The current page is shown read-only as the CPU renderer's image, fitted to the
pane with its aspect ratio kept:
1. On the UI thread, `Session::export_pages(&ExportOptions { pages: PageSelection::Folios(vec![label]), ..default })`
   gives the page's sheet display list through the session's memo store (phase 1 must stay on the session's thread;
   `Session` is not `Send`).
2. A worker thread (`std::thread`) runs `ew_render_cpu::render(&page.content, sheet, &options, fonts, None)` with
   `fonts = ew_api::output::font_bytes()` and sends the `RgbaImage` back over a channel, tagged with
   `(rev, folio, dpi)`. The UI uploads it as an egui texture; a result whose tag no longer matches the current
   revision, page or dpi is dropped.
3. The display dpi is the largest multiple of 12 between 36 and 200 at which the sheet fits the pane at the
   current `pixels_per_point`, so a resize re-rasters only when it crosses a step.
4. Until the image arrives the pane shows "Rendering page <folio>..."; a render error shows its message there.
5. If a type of step 2 is not `Send`, the lane stops and reports (no raster on the UI thread as a workaround).

**Status bar** (doc 16 §3.6, part-1 fields, left to right): the hint "Read-only view" (tools come with WP-S2.03);
the page as `<folio> (<n> of <total>)`; the save state (`*` unsaved, `saved`, or `read-only: <reason>`); the render
backend of the page view, `CPU`; the UI language `EN`; while an export runs, `Exporting PDF...`.

**Palette** (FR-UI-012, part-1 subset). Ctrl+K opens a popup with a text field over all registry actions and the
registered model commands; matching is case-insensitive substring on the label and the ID; disabled entries are
listed last with their reason; Enter runs the selected enabled entry; Escape closes it. Ranking, aliases, prefixes
and parameter forms are WP-S2.08.

**Keyboard** (doc 16 §4.2). Routing order: a focused text field, then an open popup or dialog, then the shell's
shortcuts. Escape closes the topmost popup or dialog and otherwise does nothing (no tool exists; NFR-UX-07). No
binding uses Ctrl+Alt+letter (NFR-I18N-06).

**About** (FR-UI-038, part-1 subset): the version, "Page view: CPU renderer", "Update check: off. Crash reports: none",
the licence `GPL-3.0-or-later` and the repository URL from `CARGO_PKG_REPOSITORY`. The third-party notices, the
disclaimer and the support status are FT-311.

**Fonts and scale.** At start the UI installs the bundled Noto Sans face from `ew_api::output::font_bytes()` as the
egui proportional and monospace family; `ui_scale` sets `pixels_per_point` relative to the native value.

**Accessibility** (NFR-A11Y-02, ADR-0014 amendment 2). Every interactive widget has an accessible name from its label
key; icon-only widgets are not used in part 1.

## 6. The headless mode

**The rule** (decision log ID-443). Every UI action is an action of the registry dispatched on `ShellModel`; every
model change is a `Session` command or undo/redo step; the GUI and the headless mode share `dispatch`. So CI, the
acceptance runner and the blind drafter (WP-S6.13) exercise the shell without a display, and anything the headless
mode cannot do, the shell does not do.

### 6.1 Invocation

| Invocation | Effect | Exit code |
|---|---|---|
| `elektrawatt` [`<project-dir>`] | The window; with a path, `file.open` on it after the first frame | 0 on quit |
| `elektrawatt --headless --script <file>` | Runs the UI script; the transcript goes to stdout | 0 all steps done; 1 a step refused or failed (the run stops there); 2 usage error, unreadable or malformed script, or an unknown action ID (checked before the first step runs) |
| `elektrawatt --headless --script -` | The same, reading the script from stdin line by line and flushing one transcript line per step, so a driver can run a live session | as above |
| `--record <file.ewcmd>` (with `--headless`) | After the run, writes every committed `command.run` step as an `ew.cmdlog/1` command log (header label `recorded by elektrawatt <version>`), replayable by `ew apply` | unchanged |
| `elektrawatt --smoke <project-dir>` | Opens the project read-only, renders its first page at 36 dpi through the page-view path, exits (architecture §10.1) | 0 or 1 |

The headless mode creates no window, no wgpu instance and no event loop, and needs no display server: it does not
initialise eframe or winit. It sets the session origin `Origin::Gui` like the GUI; model commands of a script run
with origin `Origin::CmdLog` (§6.3).

### 6.2 The script and the transcript

**Script** (`ew.uiscript/1`, JSON Lines). The first non-empty line is the header
`{"$schema":"ew.uiscript/1","label":"..."}`; every further line is `{"action":"<id>","params":{...}}` with `params`
optional (an empty object). Unknown members are a parse error (exit 2), like `ew.cmdlog/1`.

```text
{"$schema":"ew.uiscript/1","label":"smoke"}
{"action":"file.open","params":{"dir":"<temporary copy of golden/J-E-2416.ew>"}}
{"action":"page.goto","params":{"folio":"103"}}
{"action":"view.snapshot","params":{"path":"103.png","dpi":150}}
{"action":"command.run","params":{"cmd":"page.create","params":{"number":999,"title":"Smoke"}}}
{"action":"edit.undo"}
{"action":"file.close","params":{"discard":true}}
```

**Transcript** (`ew.uitranscript/1`, JSON Lines on stdout). A header `{"$schema":"ew.uitranscript/1","version":"<app
version>"}`, then one object per step: `{"step":<1-based>,"action":"<id>","outcome":"done"|"refused"|"failed",
"reason":<key or null>,"message":<text or null>,"state":{...}}`. `state` is the shell's observable state after the
step, with members in this order: `title`, `project` (directory name or null; never a full path), `access`
(`"edit"`, `"read-only"` or null), `dirty`, `page` (folio or null), `page_index`, `page_count`, `can_undo`,
`can_redo`, `banner` (text or null), `status` (the status-bar fields as an array of strings). No time stamps, no
absolute paths, no process IDs: two runs of one script on equal inputs give equal bytes.

`ui.describe` adds a member `describe` to its step: the visible UI as text, one line per element, in paint order (the
menu entries with enabled state and reason, the start page or the page list, the status bar, an open dialog or the
palette with its entries). It is what an agent reads instead of a screenshot. `view.snapshot` writes the current page
as a PNG at the given dpi (default 150) through the same render path as the page view, with
`ew_render_cpu::encode_png(&image, Some(dpi))`.

### 6.3 Model commands in a script

`command.run` with `{"cmd": "<name>", "params": {...}}` builds a one-line `ew.cmdlog/1` text, parses it with
`CmdLog::parse` and runs it with `Session::run_script(&log, "<script file name>")`. So the line gets the command
log's reference resolution (`{"ref":"folio:103"}` and the others of `cmdlog.rs`), its interactive or preview-and-commit
handling, one undo step per line and the origin `Origin::CmdLog`, under which `ew-commands` refuses every
`HumanOnly` command (doc 18 §3.15: no confirmation path for scripts). The shell adds no other way to submit a command.

### 6.4 Recording

The shell keeps, per open project, the list of `command.run` steps that committed, minus those taken back by undo
and plus those restored by redo, in commit order. `--record` writes that list; the recorded log replayed by
`ew apply` on a fresh copy of the same project gives the same `page_labels()` and the same saved files as the run
(the smoke test of §7 checks the labels).

## 7. Acceptance checks

All tests are ordinary nextest tests of the three crates and run in the existing three test jobs of `ci.yml`
(CI-PR-02, -03, -04) without workflow changes; none needs a display or a GPU. Tests open temporary copies of Golden
A (as `bin/ew/tests/cli.rs` does) and set `EW_CONFIG_DIR` to a temporary directory; nothing writes into the
repository.

| # | Check | Where | Proves |
|---|---|---|---|
| A1 | Registry invariants: IDs unique; every entry has a label key with an English text in `strings`; shortcuts unique within the shell context; no Ctrl+Alt+letter binding; every menu entry is a registry entry | `ew-viewmodel` unit tests | §5.2 |
| A2 | Shell-model transitions on a copy of Golden A: start → open → the page list equals `Session::page_labels()` → next, previous, first, last and goto with their bounds and refusal reasons → undo and redo enabled states → save clears `dirty` → close returns to the start page | `ew-viewmodel` tests | §5.1 |
| A3 | Read-only cases: a second `ShellModel` opening a project the first holds is read-only with Save refused (`reason-read-only`); a project with a leftover journal opens read-only with the banner | `ew-viewmodel` tests | §5.1 |
| A4 | Settings: round trip with an unknown key kept; a corrupted file gives defaults, `settings.toml.bad-1` and a banner; the recent list is capped at 10, deduplicated, most recent first | `ew-viewmodel` tests | FR-UI-049 |
| A5 | Script parser: header missing or foreign, unknown member, unknown action ID → exit 2 before any step runs; a refused step → exit 1 and the run stops | `ew-viewmodel` tests | §6.1, §6.2 |
| A6 | Headless smoke: the script of §6.2 (plus `file.save` after a committed `command.run`, a reopen and `file.export_pdf`) through the built binary: exit 0; the transcript's states as expected; the 150 dpi snapshot of folio 103 has the size of `golden/J-E-2416/expected/png/103.png` and at most 0.5 % of its pixels differ by more than 16 (the rule of `the_png_candidates_match_within_the_doc_20_tolerance` in `crates/ew-api/tests/page_output.rs`); the PDF exported before any edit has the SHA-256 of `golden/J-E-2416/expected/pdf/sha256.txt`; after `command.run` of `page.create`, `file.save`, `file.close` and a second `file.open`, `page_labels()` contains the new page's folio label; `--record` gives a log that `CmdLog::parse` reads back to the committed commands | `bin/elektrawatt/tests/headless.rs` | §6; the GUI export equals the CLI export (CI-PR-05's comparison, FR-OUT-022) |
| A7 | Determinism: the smoke script run twice gives byte-identical transcripts | `bin/elektrawatt/tests/headless.rs` | §3 rule 6 |
| A8 | `--smoke` on a copy of Golden A exits 0; on a missing directory exits 1 | `bin/elektrawatt/tests/headless.rs` | architecture §10.1 |
| A9 | No scripted bypass: `command.run` of a `HumanOnly` command (any registered one) fails with the exposure error and changes nothing | `ew-viewmodel` test | §6.3 |
| A10 | kittest flows without rendering: the start page; Ctrl+K opens the palette, typing "save" lists Save disabled with its reason when no project is open, Escape closes it; with a project open, Page Down changes the page in the status bar; Help ▸ About opens and closes | `ew-ui` tests | §5.3 |
| A11 | Accessible-name walker after every step of A10: no focusable AccessKit node without a name (ADR-0014 amendment 2) | `ew-ui` test helper | NFR-A11Y-02 |
| A12 | Policy: `cargo xtask check-deps`, `check-features`, `check-raw-read`, `check-build-scripts`, `check-licence-map`; `python -m reuse lint`; `cargo deny check`; `cargo vet --locked`; `grep -rn "unsafe" crates/ew-viewmodel crates/ew-ui bin/elektrawatt` finds only the `forbid` attributes; no `HashMap`/`HashSet` in the three crates' non-test code | lane, before the commit | §3, §4 |
| A13 | Targeted tests and lints (testing policy of `CLAUDE.md`): `cargo nextest run -p ew-viewmodel -p ew-ui -p elektrawatt` with and without `--all-features`; `cargo clippy -p ew-viewmodel -p ew-ui -p elektrawatt --all-targets --all-features -- -D warnings`; `cargo fmt --all --check`; `cargo doc -p ew-viewmodel -p ew-ui --no-deps` with `-D warnings` | lane | — |
| A14 | CI time: the lane reports the clean `cargo build -p elektrawatt` time and the test time of the three crates on its machine; the integrating session reads the per-job times of the first CI run after integration from the CI-time trend and records them in the cycle's design pass (decision log ID-448). Performance budgets (NFR-PERF-02, NFR-PERF-19) are "not measured (milestone rule)" | lane; integrating session | NFR-MAINT-05 |
| A15 | Manual, on the lane's own OS only, and reported, not gated: the window opens, a copy of Golden A opens from the dialog, pages switch, Save works | lane | — |

The workspace suite, the acceptance runner and the performance tests are not run by the lane (testing policy); the
integrating session runs `cargo xtask ci` at the cycle delivery, on Windows and in the local Linux environment
(doc 18 §3.16).

## 8. Files owned

| Path | Change |
|---|---|
| `crates/ew-viewmodel/Cargo.toml`, `crates/ew-viewmodel/src/**`, `crates/ew-viewmodel/tests/**` | New modules `actions`, `shell`, `settings`, `script`, `strings`, `page_list`, `status`; `pub use ew_api;` |
| `crates/ew-ui/Cargo.toml`, `crates/ew-ui/src/**`, `crates/ew-ui/tests/**` | New modules `app` (the frame), `menu`, `start_page`, `page_view` (raster worker), `status_bar`, `palette`, `about`, `fonts`, `headless`, `a11y` (test helper, `#[cfg(test)]` or a dev-only module) |
| `bin/elektrawatt/Cargo.toml`, `bin/elektrawatt/src/**`, `bin/elektrawatt/tests/**` | `main.rs` (arguments, eframe, rfd, exit codes) |
| `Cargo.lock` | Regenerated |
| `supply-chain/config.toml` | Exemptions only (§4) |
| `ci/build-scripts.allowlist` | New build scripts after review (§4) |
| `deny.toml` | Only `BSL-1.0`, only if needed (§4) |

Read-only for the lane: `crates/ew-api/**` and every other crate, `ci/dep-rules.toml`, `ci/licence-map.toml`,
`.github/**`, `docs/**` (the design pass after the review updates the documents). The lane commits on its own branch
and does not merge or push.

## 9. Out of scope

| Item | Where it goes |
|---|---|
| New project from the built-in or a company basic project, with the `ew-api` entry point it needs | FT-307 (WP-S2.01 part 2; the API by lead-programmer) |
| First run (doc 16 §9.1) and the company-standard wizard steps 1–9 and 11 (§9.2) | FT-308 (WP-S2.01 part 2) |
| Docking with egui_tiles, named workspaces and the workspace files (FR-UI-002) | FT-309 (WP-S2.01 part 2) |
| Native menus through muda on macOS and Windows (FR-UI-001; ADR-0014 amendment 1), with the `unsafe` decision for the Windows attach | FT-310 (WP-S2.01 part 2; a lead decision first) |
| F1 to documentation anchors and tooltip "learn more" links (FR-UI-031); the complete About dialog (FR-UI-038) | FT-311 (WP-S2.01 part 2) |
| Start page: open sample project (NFR-DOC-04), import QET project, create company standard, tutorials, quick switches | FT-312 (WP-S2.01 part 2) |
| Export as a job with progress and cancel off the UI thread (FR-UI-024, NFR-PERF-19); the crash-recovery dialog (doc 05 §8.3) | FT-313 (WP-S2.09) |
| The blind-drafter harness driving the headless mode | FT-314 (WP-S6.13) |
| The GPU canvas, the GPU-less fallback chain and the render-backend logic of FR-UI-028 | WP-S2.02 |
| Zoom, pan, tabs, side by side, xref jumps, the page navigator, the tool framework, editing gestures of any kind | WP-S2.03, WP-S2.04 |
| The property grid with provenance (FR-UI-009; D-06's grid entry, gated at G-M2 by ID-422) | WP-S2.05 |
| Navigators and tables | WP-S2.06 |
| Palette ranking, aliases, prefixes and parameter forms; keymap files and presets; Fluent localisation; themes | WP-S2.08 |
| Message panel, history panel, jobs panel, notifications | WP-S2.09 |
| Printing | WP-S3.07 |
| Image snapshots, the CI-PR-07 job, the full flow suite | WP-S2.11 |
| Clipboard | WP-S2.05 |

## 10. Review checklist for the Opus reviewer

1. **Boundaries.** No new edge in `ci/dep-rules.toml`; no new public item in `ew-api`; no `ew-model` name, no
   `raw-read`; egui and eframe only in `ew-ui` and `elektrawatt`; `ew-viewmodel` compiles without egui.
2. **One path.** Every menu item, shortcut, palette entry and dialog result ends in `ShellModel::dispatch`; the
   headless runner uses the same function; there is no second mutation path (grep for `Session` method calls
   outside `shell.rs`).
3. **No bypass.** Model commands enter only through `command.run` → `Session::run_script`; A9 passes; the GUI cannot
   run a command without parameters it does not have.
4. **UI thread.** The raster runs on the worker; stale results are dropped by tag; the export is the one documented
   synchronous exception (FT-313).
5. **Determinism and privacy.** Transcripts carry no time, no absolute path, no process ID; A7 passes; no `HashMap`
   order reaches any output; the settings file is the only file written outside a project and the chosen export path.
6. **Supply chain.** The dependency specs equal §4; `default_fonts` is off; no inspection feature, no `egui_mcp`; the
   exemption list matches the new lock entries and its criteria are right (deploy versus run); every new build
   script was read and its allowlist line says what it does; `deny.toml` changed at most by `BSL-1.0`; no audit entry
   was written by the lane.
7. **Safety.** `#![forbid(unsafe_code)]` in all three crate roots; no `unsafe` anywhere in them.
8. **Accessibility.** A11 passes; no unnamed interactive widget; Escape closes the topmost popup.
9. **Strings.** No user-facing literal outside `ew_viewmodel::strings`.
10. **Tests.** A1 to A13 present and green on the lane's OS; A14's numbers reported; the cross-platform source rules
    of doc 18 §3.16 kept (no `#[path]` in an inline module, no wall-clock assertions).
11. **Scope.** Nothing of §9 slipped in half-done; deferred items are named in the lane report with their FT.

## 11. Size

WP-S2.01 keeps its estimate of 5.8 pw PERT (O 4 · L 5.5 · P 8.5); the CSV is unchanged. Part 1 is about **2.4 pw**
of it (the registry and shell model 0.6, the egui frame with the page view and palette 0.8, the headless runner and
recording 0.4, the dependency and policy work 0.3, the tests 0.3). Part 2 (FT-307 to FT-312) is the remaining
**3.4 pw**: the wizard and first run about 2.0, docking and workspaces 0.6, native menus, F1, About and the start-page
items 0.8. FT-313 and FT-314 are absorbed by WP-S2.09 and WP-S6.13. Difficulty for routing: D3 (several files and
three crates inside a decided design; the boundary and supply-chain rules are fixed here).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S2.01 to WP-S2.11, WP-S3.07, WP-S6.13; Appendix B (FT-306 to FT-314).
- [Decision log](../06-decision-log.md): ID-4, ID-332, ID-341, ID-381, ID-421, ID-422, ID-440 to ID-449.
- [ADR-0014](../../adr/0014-gui-toolkit-egui.md); [architecture](../../design/04-architecture.md) §3, §9, §10.1;
  [UX/UI](../../design/16-ux-ui.md) §1 to §4; [schematic editor](../../design/06-schematic-editor.md) §6;
  [quality](../../design/18-quality-security-release.md) §3.5, §3.11, §3.15, §3.16, §5.2, §11.2.
- Requirements: FR-UI-001, FR-UI-002, FR-UI-012, FR-UI-024, FR-UI-028, FR-UI-030, FR-UI-031, FR-UI-033, FR-UI-034,
  FR-UI-038, FR-UI-049, FR-OUT-022, NFR-PERF-02, NFR-PERF-19, NFR-PRIV-01, NFR-A11Y-02, NFR-UX-07, NFR-I18N-06,
  NFR-MAINT-05, NFR-DOC-04.
