# Non-functional requirements

**Status:** Draft v0.2 · **Date:** 2026-10-18 · **Part of:** ElektraWatt design document set

**Purpose.** This catalogue states the quality requirements that ElektraWatt (EW) must meet: performance per
project-size class, platforms, data safety, security, privacy, accessibility, internationalisation,
maintainability, licensing, documentation, regulatory readiness and usability. Every requirement is measurable and
names where it is verified. The targets come from the G6 budgets adopted by ADR-0022, the accepted
[architecture](04-architecture.md) and the success criteria of [vision and scope](00-vision-and-scope.md) §3.2.
Where this file sets a number found in none of those sources, the row says "new target". Functional requirements
are in [02a](02a-requirements-core.md) and [02b](02b-requirements-extended.md).

**Contents**
1. [Conventions](#1-conventions)
2. [PERF: performance](#2-perf-performance)
3. [PLAT: platforms](#3-plat-platforms)
4. [REL: data safety and reliability](#4-rel-data-safety-and-reliability)
5. [SEC: security](#5-sec-security)
6. [PRIV: privacy](#6-priv-privacy)
7. [A11Y: accessibility](#7-a11y-accessibility)
8. [I18N: internationalisation](#8-i18n-internationalisation)
9. [MAINT: maintainability](#9-maint-maintainability)
10. [LIC: licensing](#10-lic-licensing)
11. [DOC: documentation](#11-doc-documentation)
12. [COMP: regulatory readiness](#12-comp-regulatory-readiness)
13. [UX: learnability and usability](#13-ux-learnability-and-usability)
14. [Requirements covered](#requirements-covered)
15. [Open questions](#open-questions)

---

## 1. Conventions

### 1.1 Table columns

The columns are those of [02a](02a-requirements-core.md) §1.1, with three differences:
- **ID** is `NFR-<AREA>-<nn>`. IDs are stable, and a withdrawn row keeps its ID with the text "Withdrawn".
- **Release** "MVP / 1.0" in a PERF row means: the S and M budgets apply from MVP, the L and XL budgets from 1.0
  (VIS SC-5). In other rows it means the part named first is MVP and the rest 1.0.
- **Acceptance criterion** starts with a verification code (§1.2). "GP" is the golden project, the reference
  project held as an EW model, cabinet J-E-2416 ([reference analysis](../research/reference-drawing.md) §4).

### 1.2 Verification codes

| Code | Where | Cadence | A failure blocks |
|---|---|---|---|
| **PR** | CI on every pull request | Every PR | The merge |
| **N** | Nightly CI; timings on a dedicated performance runner | Daily | The next release |
| **R** | Release pipeline | Every release | Publishing |
| **M** | Manual test protocol (screen readers, RDP, printers, installers) | Every minor release | The release |
| **S** | Moderated user study or pilot log | Before M2 and at 1.0 | Milestone exit |
| **A** | Review or audit (legal, security, content) | Per release, or as stated | The release |

### 1.3 Sources

The abbreviations of [02a](02a-requirements-core.md) §1.2 apply (REF-nnn, D-nn, GP, EPL, STD, IOF, PMF, PBF, G-n,
C-n, ARCH, VIS, ADR-00nn). This file adds:
- **RE §n**: [rust-ecosystem.md](../research/rust-ecosystem.md);
- **OSS §n**: [oss-landscape.md](../research/oss-landscape.md);
- **SC-n, K-n, AS-n**: success criteria, constraints and assumptions in VIS §3.2 and §10;
- **BS IDEA-nnn**: [brainstorm](01-brainstorm.md).

### 1.4 Baseline machines and measurement rules

| Name | Definition |
|---|---|
| **BM** | 4-core 2018+ x64 CPU or Apple M1, 8 GB RAM (16 GB recommended for L/XL), integrated GPU with DX12, Vulkan or Metal, 1920 × 1080, local SSD (G6) |
| **BM-noGPU** | BM without a usable hardware adapter, as in RDP, VDI or Citrix sessions: DX12 WARP or the CPU renderer (`ew-render-cpu`) |

- **Build and data.** Release build, default settings, the synthetic fixture of the class at its upper bound
  (§2.1), and a warm OS file cache unless a row says "cold".
- **Statistics.** A single operation passes on the median of 10 runs, with no run above 1.5 × budget. A latency
  passes on its p99 over at least 1,000 samples of a scripted gesture sequence.
- **Estimates and requirements.** The figures of ARCH §6.7 are estimates. The budgets here are requirements. The performance
  spike (gate G-SP, [global plan](../plan/01-global-plan.md) §5.1) measures them. A budget changes only by a reviewed change to this file that cites the measurement.

---

## 2. PERF: performance

### 2.1 Project-size classes

The classes are assumptions (AS-2), to be recalibrated against the pilot user's three largest projects before
the performance spike (G-SP; VIS open question 5). The page, device, connection and placement bounds are those of G6. The XL
device bound is raised from 40,000 to 50,000 to cover the ARCH §6.7 estimate case. The other columns use the
ARCH §6.7 ratios: functions = 3 × devices, CPs = 2.5 × connections, terminals = 0.4 × devices, channels = 0.2 ×
devices.

| Class | Pages | Devices | Functions | Connections | CPs | Terminals | PLC channels | 3D placements | Example |
|---|---|---|---|---|---|---|---|---|---|
| **S** | ≤ 50 | ≤ 300 | ≤ 900 | ≤ 1,500 | ≤ 3,750 | ≤ 120 | ≤ 60 | ≤ 200 | GP: 18 drawn folios plus generated folios 903–911, under 100 connections |
| **M** | ≤ 300 | ≤ 2,500 | ≤ 7,500 | ≤ 15,000 | ≤ 37,500 | ≤ 1,000 | ≤ 500 | ≤ 800 | Typical machine panel |
| **L** | ≤ 1,500 | ≤ 12,000 | ≤ 36,000 | ≤ 70,000 | ≤ 175,000 | ≤ 4,800 | ≤ 2,400 | ≤ 3,000 | Multi-cabinet line |
| **XL** | ≤ 5,000 | ≤ 50,000 | ≤ 150,000 | ≤ 250,000 | ≤ 625,000 | ≤ 20,000 | ≤ 10,000 | ≤ 10,000 | Plant or OEM project (stretch) |

**Fixtures.** `ew-testkit` generates one fixture per class at its upper bound, by replicating Golden B pages,
wiring, strips, PLC stations and placements until the class counts are reached. It also generates **XL-A**, the
ARCH §6.7 case (2,000 pages, 50k devices, 200k connections, about 500k CPs), so that measurements can be compared
with the estimates. At XL-A the project is about 250 MB of JSONL in about 2,300 files, plus one file per embedded
library item. XL always means the class bound of the table above, and XL-A the estimate case.

### 2.2 Requirements

| ID | Requirement | Prio | Release | Source | Acceptance criterion |
|---|---|---|---|---|---|
| NFR-PERF-01 | The budgets of this section shall hold on BM for every project up to the upper bound of its class (§2.1). | Must | MVP / 1.0 | G6; ADR-0022; SC-5 | **N:** every budget below is measured on its class fixture and the gate of NFR-PERF-20 passes. |
| NFR-PERF-02 | Start-up to a usable window (start page, no project) shall take ≤ 1.5 s and open no network connection. | Must | MVP | G6, G12 | **N:** median of 10 cold starts ≤ 1.5 s; the network sandbox (NFR-PRIV-01) records nothing. |
| NFR-PERF-03 | Opening a project shall reach the first interactive page in S ≤ 0.5 s, M ≤ 2 s, L ≤ 5 s and XL ≤ 20 s. Warm-up runs in the background, and editing is possible before it ends. | Must | MVP / 1.0 | G6; ARCH §6.7, §15.1 | **N:** time from the open command to the first frame in which a symbol on the first page is selectable; an edit made during warm-up meets NFR-PERF-05. |
| NFR-PERF-04 | While placing, moving, connecting, dragging, panning and zooming, frame time shall be ≤ 16 ms at p99 in every class: apply and validation ≤ 2 ms, derived updates of visible views ≤ 5 ms, render ≤ 5 ms. | Must | MVP / 1.0 | ARCH §2; G6 | **N:** a scripted gesture sequence on folios 101–106, replicated in each fixture, logs ≥ 1,000 frames through `tracing`/puffin; total and per-phase p99 within budget. |
| NFR-PERF-05 | Committing a move or wire gesture, including the global potential update, shall take p99 S/M ≤ 5 ms, L ≤ 10 ms (new target) and XL ≤ 20 ms. Live highlighting during a drag uses page-local nets only. | Must | MVP / 1.0 | ARCH §6.7, §15.1 (M1 exit) | **N:** 1,000 wires drawn and 1,000 symbols moved across cells per fixture. |
| NFR-PERF-06 | Panning and zooming a dense A3 page (about 5,000 primitives) shall run at ≥ 60 fps on BM, and an A4 page at ≥ 30 fps on BM-noGPU. | Must | MVP | G6; ARCH §8.2 | **N:** a synthetic dense page and GP folio 103 on the GPU, DX12 WARP and CPU-renderer jobs. |
| NFR-PERF-07 | A change touching many derived values shall update all visible views within S/M ≤ 30 ms, L ≤ 60 ms and XL ≤ 100 ms. Example: the colour of "24 V DC" in the wire profile, about 60k wires at XL. Renumbering folio 103 → 104 shall commit in ≤ 5 ms plus its preview diff. | Must | MVP / 1.0 | ARCH §6.7 | **N:** profile edit per fixture; engine statistics show that only 24 V potentials re-executed. |
| NFR-PERF-08 | A `Cost::Bulk` command shall show progress within 200 ms. Its preview shall be ready within S ≤ 0.5 s, M ≤ 1 s, L ≤ 3 s and XL ≤ 10 s, measured on a project-wide renumber, while editing stays possible (new targets). | Should | MVP / 1.0 | ARCH §5.4 | **N:** `RenumberDevices{scope: project}` per fixture, with a concurrent scripted edit meeting NFR-PERF-04. |
| NFR-PERF-09 | A full check run (all shipped packs, online and offline rules) shall finish in S/M ≤ 2 s, L ≤ 10 s and XL ≤ 60 s. Each online rule shall stay within 5 ms per commit on the M fixture; a rule over that budget shall be demoted to deferred and logged (FR-CHK-013). | Must | MVP / 1.0 | G6; ARCH §12; ADR-0019; FR-CHK-013 | **N:** `ew check --scheme pre-delivery` per fixture, and per-rule online timings on the M fixture; **PR:** a deliberately slow test rule is demoted. |
| NFR-PERF-10 | Paginating every report block cold (TOC, BOM in both modes, terminal, cable, wire, PLC and potential reports) shall finish in S/M ≤ 3 s, L ≤ 15 s and XL ≤ 60 s. | Must | MVP / 1.0 | G6; ARCH §11 | **N:** per fixture; GP folios 903–911 within 1 s. |
| NFR-PERF-11 | PDF export shall average ≤ 50 ms per page, export 500 pages in ≤ 10 s on a snapshot while editing continues, and `ew export pdf` of GP shall finish in ≤ 3 s end to end. | Must | MVP | G6; RE §11; ARCH §2 | **N:** a 500-page slice of the L fixture with a concurrent scripted edit meeting NFR-PERF-04; GP timed through the CLI. |
| NFR-PERF-12 | An incremental save shall finish in S/M ≤ 200 ms, L ≤ 500 ms and XL ≤ 2 s on the writer thread without blocking the UI. A full save (Save As) at XL shall take ≤ 15 s (new target). | Must | MVP / 1.0 | G6; ARCH §7.4 | **N:** saves after 1, 10 and 100 commits; frame p99 during the save meets NFR-PERF-04. |
| NFR-PERF-13 | Peak resident memory after open, warm-up, full check and PDF export shall be S/M ≤ 500 MB, L ≤ 1.5 GB and XL ≤ 4 GB. At XL: memos ≤ 300 MB, undo ≤ 256 MB, and the SQL projection ≤ 500 MB, file-backed. | Must | MVP / 1.0 | G6; ARCH §6.3, §6.7 | **N:** memory gate on peak RSS and on memo bytes per query family. |
| NFR-PERF-14 | A full SQL projection build shall take S/M ≤ 1 s, L ≤ 4 s and XL ≤ 10 s as a job; refreshing one table after a commit at M ≤ 100 ms. | Should | MVP / 1.0 | ARCH §6.7, §10.2 | **N:** timed per fixture. |
| NFR-PERF-15 | Navigators and tables of 100,000 rows shall scroll within NFR-PERF-04, filter within 100 ms per keystroke, and project-wide find shall answer within 1 s at XL. | Must | MVP | ARCH §9; FR-UI-007, FR-UI-020 | **N:** the device navigator of the XL fixture; finding "A9F78116". |
| NFR-PERF-16 | `RouteWires` on a 2,000-wire cabinet shall finish in ≤ 5 s, and re-routing one wire in ≤ 50 ms. | Must | 1.0 | G6; PMF §6.7 | **N:** one cabinet of the L fixture with 2,000 routed wires. |
| NFR-PERF-17 | A full 3D collision re-check of 500 placements shall finish within 10 ms after a drag ends, with box and hull feedback inside the 16 ms frame while dragging. The 3D view of 10,000 instanced placements shall render at ≥ 30 fps on BM (new target). | Must | 1.0 | PMF §6.4; ARCH §2, §8.5 | **N:** the layout of the XL fixture. |
| NFR-PERF-18 | Importing a 20 MB STEP file in `ew-step-worker` shall take ≤ 30 s, and reopening it from the cache ≤ 1 s. | Should | 1.0 | G6; ARCH §8.6 | **N:** a licence-clean, generated 20 MB STEP file. |
| NFR-PERF-19 | No operation shall occupy the UI thread for more than 100 ms. Longer work runs as a job with progress and cooperative cancel, which takes effect within 500 ms (new target). | Must | MVP | ARCH §2; FR-UI-024 | **PR:** a `tracing` watchdog fails any test with a UI-thread span over 100 ms; cancel latency asserted for PDF export and a full check. |
| NFR-PERF-20 | A **performance gate** shall run nightly on a dedicated runner over all fixtures. It covers criterion benchmarks for open, warm-up, move, draw wire, renumber folio, profile change, BOM, full check, 500-page PDF, save and projection build, plus the memory gate. Any budget miss, or a regression > 15 % against the stored baseline, turns it red, and a red gate blocks release. Per-edit engine statistics (memo hits, re-executions, cut-offs) are asserted as exact counts on every PR. | Must | MVP | ARCH §6.2, §13; ADR-0022 | **N/R:** a seeded 20 % slowdown in `potential_partition` turns the gate red, and the release pipeline refuses that commit. **PR:** a wire edit on folio 102 re-executes no query keyed to folio 203 and keeps the GPU buffers of unchanged display-list layers. |
| NFR-PERF-21 | Part search shall return its first 50 results within 100 ms over a 500,000-part index, and a full rebuild of a 500,000-part index shall finish within 60 s on BM, as a job off the UI thread. | Should | MVP | 07 §4.3, §4.7; NFR-PERF-19 | **N:** a synthetic 500,000-part library generated by `ew-testkit`, measured in the gate of NFR-PERF-20. |
| NFR-PERF-22 | AML export and the AML import preview of 10,000 PLC channels shall each finish within 10 s on BM. Both run on a snapshot (fork) while editing continues. | Should | 1.0 | G6; ADR-0022; 10 §4, §8 | **N:** the XL fixture (10,000 channels), with a concurrent scripted edit meeting NFR-PERF-04. |
| NFR-PERF-23 | A thermal recalculation of a layout space with 500 placements shall finish within 200 ms as a job on BM. | Should | 1.0 | 12 §13 | **N:** the largest layout space of the XL fixture, in the gate of NFR-PERF-20. |
| NFR-PERF-24 | Exporting the `.ewpp` of a 2,000-wire cabinet (L fixture) shall take ≤ 20 s on BM, as a job that never holds the UI thread longer than NFR-PERF-19 allows. Re-evaluating the wire records of one potential after a commit shall take ≤ 2 ms at M. | Should | 1.0 | ARCH §6.3; NFR-PERF-19; 13 §8 | **N:** one cabinet of the L fixture in the gate of NFR-PERF-20; engine statistics show that a wire edit re-executes the wire records of one potential only. |
| NFR-PERF-25 | On BM, importing a reference-size `.qet` shall reach the import preview within 2 s, a 3.5 MB corpus file within 5 s and an M-class QET project within 30 s, and the batch import of the 24 corpus files shall finish within 2 min. | Should | MVP | G6; IOF §10; 14 §4, §12 | **N:** the QET corpus and a synthetic M-class `.qet` written by `ew-testkit`, in the gate of NFR-PERF-20. |

---

## 3. PLAT: platforms

**Tiers.** *Supported* platforms are built and tested in CI, and a failure blocks the release. *Best-effort*
platforms are built for every release and fixed when reported, but a failure does not block the release.

| Platform | Tier |
|---|---|
| Windows 11 and Windows 10 22H2+ (x64) | Supported |
| macOS on Apple silicon | Supported |
| Ubuntu 22.04+ and Debian 12+ (x64), Wayland and X11 | Supported |
| Windows ARM64, macOS on Intel (Rust Tier 2), Linux arm64 | Best effort |
| Other Linux distributions through Flatpak or AppImage | Community |

| ID | Requirement | Prio | Release | Source | Acceptance criterion |
|---|---|---|---|---|---|
| NFR-PLAT-01 | EW shall ship the same feature set on every Supported platform of the tier table. Platform-specific code is limited to adapters: printing, menus, dialogs, clipboard and signing. | Must | MVP | ADR-0022; ARCH §2; K-4 | **PR:** the full test suite runs on Windows, macOS arm64 and Ubuntu; **R:** an artefact exists for every Supported and Best-effort row. |
| NFR-PLAT-02 | wgpu shall run on DX12 (Windows), Metal (macOS) and Vulkan (Linux; Windows fallback). OpenGL and GLES are best effort and untested. | Must | MVP | G6; ADR-0015 | **PR:** the GPU smoke test runs per backend on the CI runners that provide one. |
| NFR-PLAT-03 | Without a hardware adapter (RDP, VDI, Citrix), EW shall fall back to DX12 WARP, then to the CPU renderer. Every 2D function stays available within NFR-PERF-06, and the status bar names the backend. The 3D view uses a software adapter where one exists. Otherwise the 3D view is disabled with an explanation, while the 2D panel projection (folio 902 type) stays available. | Must | MVP / 1.0 | G6; ARCH §8.2; ADR-0015; FR-UI-028 | **N:** the WARP job and the CPU-renderer job run the kittest golden flows; **M:** an RDP session without a GPU completes the five core tasks of NFR-UX-01. |
| NFR-PLAT-04 | The `ew` CLI, including `ew mcp`, shall run on every Supported platform and in a Linux container without a display or GPU. | Must | MVP | ARCH §1.2 (7); FR-EXT-001 | **PR:** a container job runs `ew export pdf` and `ew render --page 103 --png` on GP. |
| NFR-PLAT-05 | Projects shall work on NTFS, APFS (case-insensitive and case-sensitive), ext4, btrfs and SMB 2.1/3 shares. Project-internal paths are UUID-based lower-case ASCII, at most 120 characters below the project root. Projects whose own path contains spaces or non-ASCII characters must also work. | Must | MVP | G1; ARCH §7.1 | **PR:** GP saved and reopened on a case-sensitive APFS image and on a Samba share; the longest internal path ≤ 120 characters. |
| NFR-PLAT-06 | Releases shall provide offline installers containing fonts, symbols, starter libraries and the documentation: a Windows MSI (per machine, silent install, policy keys) and a per-user installer that needs no administrator rights; a signed and notarised macOS DMG; a Linux AppImage and `.deb`. Flathub follows (Should, 1.0). | Must | MVP | RE §10.1; OSS §6.3; G12 | **R:** installation on clean VMs without network; the installed application opens the GP sample. |
| NFR-PLAT-07 | EW shall run without administrator or root rights. It writes only to user-chosen project locations and to the OS config, state and cache directories, never to its installation directory. | Must | MVP | G1; ARCH §7.4 | **PR:** the test suite passes with a read-only installation directory. |
| NFR-PLAT-08 | The update check, the update mirror URL, crash upload, `ew mcp` availability, plugin installation and developer mode shall be enforceable by administrator policy: Windows registry policy keys, macOS managed preferences and a Linux system policy file. A policy overrides the user settings and is shown as managed. | Should | 1.0 | G12; NFR-PRIV-03; 18 §9.3 | **M:** with the update-check policy set to off, the settings toggle is disabled and the network sandbox records no request. |

---

## 4. REL: data safety and reliability

| ID | Requirement | Prio | Release | Source | Acceptance criterion |
|---|---|---|---|---|---|
| NFR-REL-01 | An interrupted save (process kill or power loss at any point) shall leave the project openable at either the previous or the new generation, decided by the manifest hashes. A mixed generation is never accepted silently. | Must | MVP | G1; ADR-0010; ARCH §7.4; FR-COL-003, FR-COL-004 | **N:** a fault-injection harness kills the writer at 1,000 random points (temp write, `sync_all`, rename, manifest) on ext4, NTFS and APFS; every reopen yields generation n or n+1, or reports the integrity mismatch. |
| NFR-REL-02 | After an application or OS crash, at most the last 1 s of committed transactions may be lost. Recovery shall offer a journal replay onto the last good generation, backed by three rotating recovery snapshots written every 2 minutes while the project is dirty. | Must | MVP | ARCH §7.4; FR-COL-005 | **N:** `kill -9` at random times while a script commits 10 edits/s; the replayed model equals the live model at kill time, minus at most 1 s of commits. |
| NFR-REL-03 | A failed save (disk full, permission denied, SMB disconnect) shall leave the previous generation intact, keep the in-memory model and journal, keep the project dirty and report cause and remedy. No manifest is written for an incomplete set. | Must | MVP | G1 | **PR:** ENOSPC and EACCES injected at each write step; GP still opens at the previous generation. |
| NFR-REL-04 | Opening and saving an unchanged project shall be byte-identical. Unknown fields and unknown record kinds shall be preserved verbatim, and no save may drop data that EW cannot interpret. | Must | MVP | ADR-0011; ARCH §7.6; FR-COL-010 | **PR:** `load(save(m)) = m`, byte-stable on GP and the QET corpus; a fixture with an unknown `"$k"` survives a save. |
| NFR-REL-05 | Format compatibility rules: (a) every released format fixture opens in every later build, and migrations are never deleted; (b) a reader opens newer minors of its major with a warning; (c) newer majors are refused or opened read-only; (d) after 1.0, majors change only in a yearly window, and development builds that write an unreleased format warn. | Must | MVP / 1.0 | ADR-0011; FR-COL-010, FR-COL-011 | **PR:** the fixture-corpus job; a format change without migration, fixture, schema and docs fails (schema-steward rule). |
| NFR-REL-06 | Before the first save that rewrites a project in a newer format major (before 1.0: a newer format), EW shall ask for confirmation and write an `.ewz` of the original generation next to the project (new). | Should | MVP | ADR-0011; FR-COL-029 | **PR:** saving an older fixture leaves an `.ewz` that reopens to the original bytes. |
| NFR-REL-07 | For the same model, settings, EW version and derivation profile, every output shall be byte-identical across Windows, macOS and Linux, thread counts and record load order. This covers project files (JSONL), PDF, SVG, XLSX/CSV reports, `.ewpp`, machine and PLC exports, routing, lengths, ferrule selection, bundle sequencing and check findings. PNG output is identical within the dify tolerance across platforms (ADR-0015); if the performance spike (G-SP) finds a deterministic scalar mode of the CPU renderer, `ew render` and the CI goldens use it and PNG is byte-identical again on the same platform. No output carries wall-clock timestamps, random IDs or absolute paths: dates come from the model or from an explicit export setting. | Must | MVP / 1.0 | ARCH §1.2 (8), §13; PMF §11; FR-OUT-019; ADR-0015 | **PR:** shuffle-order and serial-versus-parallel digests; **N:** cross-platform comparison of GP output digests, and of GP PNG renders within the dify tolerance. |
| NFR-REL-08 | Derived data shall be correct by test. Memoised results shall equal a cold recomputation. These properties shall hold: `undo(all) ∘ apply(cmds) = id`, journal replay = live state, preview + commit = execute, and renumbering keeps all references. | Must | MVP | ARCH §6.8, §13; ADR-0002, ADR-0008 | **PR:** oracle on GP plus short proptest runs; **N:** full oracle and the command-sequence fuzzer; `ew verify-derived` reports zero differences on every fixture. |
| NFR-REL-09 | A project pinned to a derivation profile shall print byte-identical tags, cross-references and wire numbers in every later EW release. Shipped profiles are never removed and keep their golden tests. | Must | MVP | ADR-0003; ARCH §6.6; FR-PRJ-011 | **PR:** golden outputs per shipped profile. |
| NFR-REL-10 | An as-built archive shall be self-contained: canonical text, embedded libraries, PDF/A-2b, `.ewpp`, SHA-256 sums, and the JSON Schemas and format specification of its format version. It shall open read-only in every EW release for at least 10 years after its creation, and its PDF and JSONL shall stay interpretable without EW. | Must | 1.0 | G2; ADR-0013; ARCH §7.8; Regulation (EU) 2023/1230 retention (article unverified) | **R:** the GP V1.0 archive passes `sha256sum -c` and veraPDF PDF/A-2b, and validates against the included schemas with a permissive-only reader; every release's archive joins the fixture corpus. |
| NFR-REL-11 | A project shall open, render, check and export without network access, without its source libraries and without system fonts. | Must | MVP | G2; ARCH §7.7 | **PR:** GP in a sandbox with empty library directories and no system fonts yields the golden PDF digest. |
| NFR-REL-12 | A crash, hang or limit violation in an importer, rhai script, SQL query, `ew-step-worker` or plugin shall leave the model unchanged and the session running. The failure is reported and a worker is restarted. | Must | MVP | ARCH §2, §8.6, §10 | **PR:** a hostile STEP input that kills the worker and a non-terminating rhai script both leave the model digest unchanged. |
| NFR-REL-13 | On a panic or crash, EW shall flush the journal, write a local minidump and log, and offer recovery at the next start. | Must | MVP | ADR-0029; RE §10.1 | **PR:** a forced panic test yields a minidump, a log and a recovery offer. |
| NFR-REL-14 | An 8-hour scripted editing session on the M fixture shall show no crash, memory growth of at most 10 % after warm-up, and no frame-budget degradation (new target). | Should | 1.0 | ARCH §5.3 | **N** (weekly): soak job. |
| NFR-REL-15 | A release shall be atomic: the as-built archive is written and verified, the revision stamped and, in a git repository, the tag created; otherwise the revision stays Open with no archive or tag referenced. | Must | MVP | ADR-0013; FR-REV-006; 05 §14.1 | **N:** killing `ew revision release` at random points on Golden A-R leaves V2.0 either Open, or Released with an archive that verifies. |
| NFR-REL-16 | For any sequence of committed transactions, incremental saves shall produce files byte-identical to a full save of the final model. | Must | MVP | ARCH §7.4; 05 §2.2, §5.1 | **PR:** proptest command sequences on GP compare the digests of incremental and full saves after every step; **N:** the same on the M fixture. |
| NFR-REL-17 | When a later EW release updates a bundled drawing font, it shall ship the update as a new font version and keep the previous file available. A project keeps the font version its derivation profile names, so reopened projects keep identical line breaks; moving to the new version is the opt-in, previewed profile upgrade of FR-PRJ-011. | Should | 1.0 | ADR-0003; ADR-0028 (proposed); NFR-REL-07, NFR-REL-09 | **PR:** opening Golden A with a build that bundles a newer version of the default font gives the same PDF bytes until the profile is upgraded. |

---

## 5. SEC: security

The threat model is G10 as adopted in ARCH §10.6. Received projects, import files, plugins, scripts and AI agents
are untrusted.

| ID | Requirement | Prio | Release | Source | Acceptance criterion |
|---|---|---|---|---|---|
| NFR-SEC-01 | A threat model shall be maintained in the repository and reviewed every minor release. Every new parser or extension point adds an entry with its controls. | Should | MVP | G10; ARCH §10.6 | **A:** release-checklist item. |
| NFR-SEC-02 | Every parser of external bytes shall enforce limits: ≤ 100,000 archive entries, ≤ 2 GiB expanded, nesting depth, no external XML entities, no `..`, absolute or drive paths, NaN and overflow guards. Parsers never execute content. This covers the JSONL loader and migration chain, `.ewz`, QET/QETZ, EDZ (7-Zip and ZIP), parts XML, DXF, DWG, AML, PLCopen, GSD/GSDML, IODD, ESI, EDS, CSV/XLSX and STEP IPC, and, as they ship (1.0 and later), the `.ewpp` package readers (wiring viewer, `ew diff`, as-built) and the machine-feedback and status-log parsers. | Must | MVP / 1.0 | G10; FR-IMP-005; ARCH §10.6; 13 §7.5, §8.4 | **PR:** a hostile corpus (zip bomb, billion-laughs XML, path-traversal entry, NaN coordinate) is rejected within 1 s with ≤ 256 MB extra RSS; at 1.0, a fuzzed `.ewpp` with a `..` entry and a 3 GiB member is rejected without extraction. |
| NFR-SEC-03 | Each parser of NFR-SEC-02, plus the merge driver and the SQL sandbox, shall have a cargo-fuzz target. It runs ≥ 60 s per changed target on PRs and ≥ 30 min per target nightly (new targets). Every crash becomes a regression test, and no release ships with an open fuzz crash. | Must | MVP | ARCH §13; G10 | **PR/N/R:** fuzz job reports; release checklist. |
| NFR-SEC-04 | Every crate shall `#![forbid(unsafe_code)]` except an allowlist (renderers, the app shell's platform glue, `ew-step-worker`). Test-only crates that never ship in release artefacts (`ew-testkit`, `fuzz/`) may use `unsafe` when they are listed in the xtask allowlist. Each `unsafe` block carries a `SAFETY` comment reviewed by a second maintainer. C and C++ code is limited to bundled SQLite in process and OCCT out of process (new). | Should | MVP | K-2; ARCH §3, §8.6; 18 §11.2 | **PR:** a lint compares `unsafe` use per crate with the allowlist. |
| NFR-SEC-05 | Opening, viewing, checking or exporting a received project shall not run its automation scripts, macros, SQL reports, SQL or rhai rule packs, or plugin invocations until the user trusts the project. Trust is remembered by project hash and signer, built-in packs are trusted, and the untrusted state is visible. | Must | MVP | G10; ARCH §10.2, §10.6; FR-EXT-014 | **PR:** a fixture with a file-writing script and an SQL report: open and `ew export pdf` create no file and show the trust banner. |
| NFR-SEC-06 | rhai shall have no I/O functions registered and run with operation, call-depth and string-size limits. SQL shall run on a `query_only` connection, one statement per call, with an authorizer that denies `ATTACH`/`DETACH`, every `PRAGMA`, DDL, DML, transactions, `ANALYZE`, `REINDEX`, direct `t_*` reads, `load_extension` and non-allowlisted functions. Run time is bounded by a progress-handler deadline; `SQLITE_LIMIT_*` limits apply, and the VDBE-op limit counts only as a prepare-time program-size cap (enforcement **verify**). The row caps are those of ADR-0017. Foreign SQLite files are opened read-only, with `trusted_schema=OFF` and defensive mode. | Must | MVP | ARCH §7.7, §10.2, §10.3; ADR-0017 | **PR:** a hostile corpus (ATTACH, a `PRAGMA` read, two statements in one call, recursive-CTE bomb, trigger-laden QETZ database) is refused or stopped at the deadline. |
| NFR-SEC-07 | Plugins shall run with: a capability manifest approved at install; WASI pre-opened directories only; no network unless granted; fuel or epoch deadlines and `StoreLimits` per call; signed manifests, unsigned plugins only in an explicit developer mode; and the plugin's ID in the undo history. | Must | 1.x | ARCH §10.4; ADR-0020; FR-EXT-020 | **PR:** a test plugin that reads outside its pre-open, loops forever and allocates 4 GiB is trapped each time, with the model unchanged. |
| NFR-SEC-08 | `ew mcp` shall never start implicitly. It listens on stdio or loopback only, never on another interface, and loopback requires a per-session random token. Writes are ask-first and journalled with `origin=mcp`, and access is limited to the opened project. | Must | MVP / 1.0 | ARCH §10.5; ADR-0020; FR-EXT-016, FR-EXT-017, FR-EXT-026 | **PR:** binding to a non-loopback address is impossible; a write without approval or a request without the token is refused. |
| NFR-SEC-09 | Release binaries shall contain no egui inspection, egui_mcp or other remote-control endpoint. They open no listening socket unless the user starts `ew mcp`. | Must | MVP | G10; ARCH §9, §13 | **R:** a binary assertion, plus a socket scan in the network sandbox. |
| NFR-SEC-10 | Supply-chain controls: a committed `Cargo.lock` and a pinned toolchain; `cargo-deny` (licences, advisories, bans, sources: crates.io unless allowlisted) and `cargo-audit` per PR; a `cargo-vet` audit per new or updated crate; no build script that downloads binaries; low-adoption crates test-only or vendored with an owner. | Must | MVP | ADR-0023; G10; C-5, C-9 | **PR:** a PR adding an unaudited crate or an unlisted git dependency fails. |
| NFR-SEC-11 | No release shall ship with a known unpatched high or critical advisory in its dependency closure. Lower severities need a waiver that expires within 90 days (new). | Must | MVP | SC-10; ADR-0023 | **R:** `cargo-audit` and an SBOM scan at release. |
| NFR-SEC-12 | Releases shall be built by CI from a signed tag. Windows binaries and installers are Authenticode-signed (Artifact Signing, organisation identity). macOS builds are signed and notarised (rcodesign). Every artefact carries a SHA-256 sum and SLSA build provenance. | Must | MVP | ADR-0023, ADR-0030 | **R:** signature checks and `gh attestation verify` pass for every artefact. |
| NFR-SEC-13 | Every release artefact shall have a CycloneDX SBOM that includes the non-Rust components (SQLite, OCCT, fonts). `cargo-auditable` metadata is embedded in every binary. | Must | MVP | ADR-0023; G10, G11 | **R:** SBOM present and complete against `cargo metadata`. |
| NFR-SEC-14 | `SECURITY.md` shall offer private reporting (GitHub security advisories), a supported-versions table and coordinated disclosure, with acknowledgement within 7 days and a fix or mitigation for critical issues within 30 days (new targets). A CVE is requested per fixed vulnerability. | Must | MVP | G10, G11; ADR-0023 | **A:** a disclosure tabletop exercise before 1.0. |
| NFR-SEC-15 | An update shall be downloaded and installed only on a user action and only after verification. The update manifest and `SHA256SUMS` are signed with an ed25519 release key whose public half (current and next key) is embedded in the binary. EW verifies the manifest before it shows any update notice and verifies every downloaded file against it. There is no silent auto-update. | Must | MVP | G10, G12; ADR-0029; 18 §9.2 | **PR:** a manifest with a bad signature produces no notice; **R:** a tampered update package is refused, and `SHA256SUMS.sig` verifies with the published key. |
| NFR-SEC-16 | The project loader shall read only regular files inside the project directory, follow no symbolic links or junctions, and never read trust decisions (NFR-SEC-05) from the project directory. | Must | MVP | G10; 05 §3.6, §13.4 | **PR:** a fixture whose page file is a symbolic link to a file outside the project opens with INT-03 and reads nothing outside the project. |
| NFR-SEC-17 | Spreadsheet exports shall not produce cells that a spreadsheet evaluates as formulas: XLSX writes all text as string cells, and the Excel CSV presets guard a leading `=`, `+`, `-`, `@`, tab or CR. | Should | MVP | OWASP CSV injection; FR-EXP-003; 09 §4.6 | **PR:** a function text "=HYPERLINK(…)" exports inert in XLSX and in the `excel-nl` CSV preset; the `rfc4180` preset keeps it verbatim. |
| NFR-SEC-18 | Importers shall read only the bytes of the selected source: no external XML entities, XInclude, DXF IMAGE file references, SVG external references or sibling files, and no network access. | Must | MVP | G10; NFR-SEC-02; 14 §2, §11 | **PR:** a DXF whose IMAGEDEF points to a local file and an SVG logo referencing a URL import with placeholders, and a file-access trace shows no other file opened. |
| NFR-SEC-19 | CI shall pin third-party actions by commit SHA and use read-only default tokens. Release and signing jobs run in a protected environment that needs a second maintainer's approval and uses short-lived (OIDC) credentials. Self-hosted runners never run pull requests from forks. | Must | MVP | G10; ADR-0023; 18 §5.4 | **PR:** a workflow lint fails an unpinned action or a write-all token; **A:** the environment settings are reviewed every minor release. |
| NFR-SEC-20 | Unsigned Linux x64 binaries of `ew` and `elektrawatt` shall rebuild bit-identically from the release tag in an independent CI job. | Should | 1.0 | ADR-0023; 18 §16 | **R:** the SHA-256 values of the two builds are equal. |
| NFR-SEC-21 | `ew-step-worker` shall run under an OS sandbox (Linux seccomp and namespaces, a Windows restricted token or AppContainer, a macOS sandbox profile) with no network access, reading only its input and writing only its cache directory. | Could | 1.x | G10; ADR-0016; 18 §11.1 (TM-08) | **PR:** a test worker build that opens a socket or writes outside the cache is killed by the sandbox. |

---

## 6. PRIV: privacy

The default is "offline-first, zero by default" (ADR-0029, proposed; owner decides the crash endpoint).

| ID | Requirement | Prio | Release | Source | Acceptance criterion |
|---|---|---|---|---|---|
| NFR-PRIV-01 | EW shall open no network connection, DNS lookups included, without an explicit user action. Permitted user-initiated flows: the update check if opted in, a crash upload per report if opted in, a help link opened in the browser, a plugin holding a granted network capability, and `ew mcp` on loopback. Start, open, edit, save, import, export, render, print, check and offline help open no connection. | Must | MVP | G12; ADR-0029; ARCH §16; SC-10 | **N:** the network-sandbox job runs CLI and kittest flows (open GP, edit, check, export PDF, import QET, print to file) and fails on any attempt. |
| NFR-PRIV-02 | EW shall contain no usage analytics, no install or user identifier and no feature counters. | Must | MVP | G12; ADR-0029 | **A:** code review; **N:** network sandbox. |
| NFR-PRIV-03 | The update check shall be opt-in at first run and off by default in MSI deployments (policy key). It sends only product version, OS and architecture, at most once a day. | Should | MVP | ADR-0029 | **N:** the captured request carries only those fields. |
| NFR-PRIV-04 | Crash reports shall be local (minidump plus log) and inspectable and copyable by the user. Upload is opt-in per report, to the endpoint decided under ADR-0029. An upload contains only a client-side stack walk (module, build ID, offset), allowlisted metadata and the panic message scrubbed of project content, paths, titles and user and host names; it never contains a raw minidump or memory. The user previews the exact payload before sending. | Must | MVP / 1.0 | ADR-0029; G12; 18 §12.3 | **PR:** a forced crash with GP open yields an upload payload with no minidump bytes and no "J-E-2416", path or user name. |
| NFR-PRIV-05 | Journal, recovery snapshots, caches and logs shall live in per-user OS directories with user-only permissions. The journal is removed on a clean close, and leftovers of crashed sessions older than 30 days are pruned. The default log level records no project text or personal data (new). | Should | MVP | ARCH §7.4 | **PR:** permission checks (0600/0700 on Linux and macOS; user-only ACL on Windows). |
| NFR-PRIV-06 | Person fields (engineer, checker, party names and addresses) shall be marked as personal data in `ew-schema`. Exports shall offer to omit them from page text, reports and XMP. Embedding the native project into a PDF requires a confidentiality confirmation. | Should | MVP | G12, G15; FR-PRJ-024; FR-OUT-003, FR-OUT-009, FR-OUT-025 | **PR:** an "omit personal data" export of GP contains no `<engineer>` value in its text or XMP. |
| NFR-PRIV-07 | `PRIVACY.md` shall list every data flow with its content and endpoint and is shown in the About dialog. Any change to it requires an ADR. | Must | MVP | ADR-0029; G12; FR-UI-038 | **A:** release checklist. |
| NFR-PRIV-08 | EW shall send no project data to any AI or translation service. Agents connect through MCP by the user's choice, and machine translation is available only through a plugin with the user's own key. | Must | MVP | VIS §4; G7, G12; FR-I18N-015 | **N:** network sandbox; **A:** dependency review. |
| NFR-PRIV-09 | CI shall reject any committed file that contains a word n-gram (n ≤ 3) whose SHA-256 is on the maintainers' confidential-token list. Only the hashes are committed. | Must | MVP | K-7; NFR-DOC-04; 20 §3.5 | **PR:** committing a fixture with a listed token fails CI; AC-39 passes. |
| NFR-PRIV-10 | Usability-test participants shall consent in writing, on the template of 19 §3.7, before any recording. Recordings and session logs stay with the moderator and are deleted 90 days after the report; only anonymised findings are published. | Should | MVP | 19 §3.7; 16 OQ 9; NFR-UX-01; GDPR art. 6(1)(a) | **S:** a signed consent for every NFR-UX-01 and NFR-UX-09 session; **A:** the deletion log is reviewed at milestone exit. |

---

## 7. A11Y: accessibility

| ID | Requirement | Prio | Release | Source | Acceptance criterion |
|---|---|---|---|---|---|
| NFR-A11Y-01 | Every command shall be reachable from the keyboard (palette and keymap), and the canvas fully operable (FR-UI-015). Focus is visible and in logical order, nothing traps the keyboard, and Escape closes every dialog. | Must | MVP | G8; ARCH §9; SC-12 | **PR:** kittest runs the five core tasks (place, wire, terminal strip, renumber, export PDF) on GP without pointer events. |
| NFR-A11Y-02 | 100 % of interactive widgets shall expose name, role and state through AccessKit (UI Automation, NSAccessibility, AT-SPI). The selected object is announced as a live region, e.g. "-102G1, power supply 24 V DC, 7 connection points, 9 wires". Navigators and tables are the accessible representation of the schematic (FR-UI-026). | Must | MVP | G8; ARCH §9 | **PR:** kittest name enforcement; **M:** with NVDA, VoiceOver and Orca, find -102F1, read its part and connections, and open its page. |
| NFR-A11Y-03 | No meaning shall be carried by colour alone (FR-UI-025). Highlight and severity palettes stay distinguishable under protanopia, deuteranopia and tritanopia simulation, with ≥ 3:1 contrast against the canvas. | Must | MVP | G8; WCAG 2.1 SC 1.4.1, 1.4.11 | **PR:** CVD-simulated renders of folio 102 with 24VDC-1 highlighted and the message panel open. |
| NFR-A11Y-04 | UI text contrast shall be ≥ 4.5:1 in the light and dark themes and ≥ 7:1 in the high-contrast theme. The UI scales from 100 % to 300 % without clipped text in core dialogs (FR-UI-027). Animations can be disabled, and no interaction has a time limit. | Must | MVP | G8; WCAG 2.1 SC 1.4.3, 1.4.6 | **PR:** a contrast lint over theme tokens; kittest snapshots at 300 %. |
| NFR-A11Y-05 | Optional tagged PDF (PDF/UA-1, ISO 14289-1) shall carry a logical reading order (page content by structure, then title block), alternative text for symbols from their tag and function text, and report tables tagged as tables. XLSX and CSV serve as accessible alternatives. | Should | 1.0 | G8; FR-OUT-012 | **R:** the tagged GP PDF passes a PDF/UA-1 checker (veraPDF profile, **verify**). |
| NFR-A11Y-06 | The HTML user documentation shall show no critical WCAG 2.1 AA violation in automated checks, and every screenshot and diagram has a text alternative. | Should | 1.0 | G8, G13 | **N:** an automated accessibility scan of the built docs. |

---

## 8. I18N: internationalisation

Project-data multilinguality (`MlText`) is functional and specified in FR-I18N-001 … FR-I18N-016. The rows below
are quality targets.

| ID | Requirement | Prio | Release | Source | Acceptance criterion |
|---|---|---|---|---|---|
| NFR-I18N-01 | UI locales: English and Dutch at MVP, German by 1.0 (FR-UI-032). A locale is listed as supported only with ≥ 95 % of strings and 100 % of core-flow strings translated; others are labelled partial (new threshold). | Must | MVP / 1.0 | VIS OQ8, SC-13; ARCH §9 | **R:** the Weblate statistics gate. |
| NFR-I18N-02 | Every user-facing string (UI, check messages, built-in report headings, human-readable CLI output) shall come from a Fluent key. A pseudo-localised build (+35 % length, accented characters) passes the core kittest flows without truncation or overlap. | Must | MVP | ARCH §9, §12; G7 | **PR:** a string-literal lint on `ew-ui` and `ew-checks`; the pseudo-locale kittest job. |
| NFR-I18N-03 | Project files, derived values and outputs shall not depend on the OS locale, time zone or keyboard layout. Number and date formats depend only on project settings and the display language (FR-I18N-009). | Must | MVP | G7; ARCH §1.2 (8) | **N:** GP files and PDF byte-identical under OS locales en-US, nl-NL, de-DE and tr-TR and time zones UTC−10 and UTC+14. |
| NFR-I18N-04 | Project texts shall support full Unicode (NFC), with the identifier hygiene of FR-I18N-011 and natural sort (FR-I18N-010), in projects whose paths contain non-ASCII characters and spaces. | Must | MVP | G1, G7 | **PR:** GP opens, saves and exports from a directory named `Projecten Ü ñ 测试`. |
| NFR-I18N-05 | A project with four display languages shall open within NFR-PERF-03, and each additional per-language PDF export shall cost no more than NFR-PERF-11 (new). | Should | 1.0 | G7; ARCH §4.5 | **N:** the M fixture with four languages. |
| NFR-I18N-06 | Default shortcuts shall be reachable on QWERTY, QWERTZ and AZERTY layouts (new). | Should | MVP | G5 | **PR:** a keymap conflict and reachability test per layout. |
| NFR-I18N-07 | Documentation shall be translated through mdbook-i18n-helpers (PO) and Weblate: English complete at every release, Dutch tutorials at MVP, German by 1.0. | Should | MVP / 1.0 | G13; OSS §8.5 | **R:** translation status report attached to the release. |

---

## 9. MAINT: maintainability

| ID | Requirement | Prio | Release | Source | Acceptance criterion |
|---|---|---|---|---|---|
| NFR-MAINT-01 | CI shall enforce the crate rules of ARCH §3: `ew-model` tables are `pub(crate)` and its `raw-read` feature is enabled only by `ew-derive`, `ew-commands` and `ew-store`; only `ew-sqlview` and `ew-library` link rusqlite; only `ew-ui`, `ew-viewer`, the app binary and the `ew-wiring-viewer` binary depend on egui or an egui integration crate; only the renderers depend on wgpu; importers depend only on `ew-interop-api`, their parsers and permissive helpers; no crate depends on a crate above it in the §3 graph. | Must | MVP | ARCH §3 | **PR:** a dependency-rule check over `cargo metadata` fails a seeded violation. |
| NFR-MAINT-02 | The dependency closure of every permissive crate, counting normal, build and dev dependencies, shall contain only MIT, Apache-2.0 (also WITH LLVM-exception), BSD, ISC, Zlib, Unicode-3.0, MPL-2.0 or CC0-1.0 crates and no GPL, LGPL or AGPL crate, from the first commit. Permissive crates test with permissive test helpers only. | Must | MVP | ADR-0024; ARCH §3, §13 | **PR:** a GPL dependency seeded into `ew-format`, also as a dev dependency, fails the licence-boundary check. |
| NFR-MAINT-03 | The application shall build with the toolchain pinned in `rust-toolchain.toml` (≥ 1.95 for egui 0.37), raised at most once per quarter together with the UI-stack upgrade. Published permissive crates declare a `rust-version` at least 6 months old and are built at that MSRV in CI; raising it is a minor version bump (new). GPL application crates need only the pinned toolchain and declare the version their dependencies require (decision log ID-141). | Must | MVP | RE §0, §10.3 | **PR:** the MSRV job builds the permissive crates with their declared `rust-version`. |
| NFR-MAINT-04 | egui, eframe, egui-wgpu, wgpu, winit and the egui_* crates shall be upgraded in lock-step once per quarter, at a target of about one developer-day. The workspace is never more than two egui minors or one wgpu major behind. wasmtime stays on an LTS line and moves to the next one before the current line leaves support. | Must | MVP | ARCH §9; ADR-0014, ADR-0020 | **A:** a quarterly upgrade issue; **N:** a dependency-age report that flags the lag limits. |
| NFR-MAINT-05 | Per-PR CI shall run fmt, clippy `-D warnings`, nextest, insta, short proptest runs, the oracle on GP, kittest, fuzz smoke, the SQL compile test, the schema and fixture check, `reuse lint` and the DCO check, within 30 min wall time (new target). | Must | MVP | ARCH §13; OSS §8.1 | **PR:** the pipeline definition; a monthly CI duration report. |
| NFR-MAINT-06 | Core crates (the permissive layer, `ew-model`, `ew-derive`, `ew-commands`, `ew-store`) shall reach ≥ 80 % line coverage (new target). Each invariant of `ew-model/invariants.md` has its own test, and the field-mask exhaustiveness test guards the index declarations. | Should | MVP | ARCH §4.1, §4.6 | **PR:** cargo-llvm-cov report; a new link field without an index declaration fails. |
| NFR-MAINT-07 | `ew-memo`, the `Op` set, `ew-format` and `ew-derive` shall each have two named owners in `CODEOWNERS` by M1, and the schema-steward role rotates. | Must | MVP | ARCH §15.1; ADR-0002, ADR-0011 | **A:** M1 exit review. |
| NFR-MAINT-08 | Permissive crates shall `#![deny(missing_docs)]`. rustdoc is built and link-checked in CI. Architecture-level changes come with an ADR. `CONTRIBUTING.md` explains how to add a rule, report, importer and machine profile, and the invariant catalogue and `ew-memo` design notes stay current. | Must | MVP | ARCH §4.6, §6.2, §7.6; OSS §8.2 | **PR:** rustdoc and docs build without warnings. |
| NFR-MAINT-09 | The application shall build with Rust and a C compiler (bundled SQLite) only. A C++ toolchain is needed only for `ew-step-worker`, which builds in a separate CI job with cached OCCT artefacts. | Should | MVP | RE §10.3; ARCH §8.6 | **PR:** the app job runs on an image without a C++ compiler. |
| NFR-MAINT-10 | **API compatibility guard.** Snapshots of the command and query registries, the CLI help and JSON output schemas, the `api_v1` columns, the WIT package and the MCP tool list shall be checked on every PR. A breaking change fails unless it carries an `api-break` label, an ADR reference and a release note. | Must | MVP | 17 §11; 18 §5.2 (CI-PR-14); NFR-DOC-06 | **PR:** renaming a column of `api_v1_wire` fails the job. |
| NFR-MAINT-11 | Every requirement whose acceptance criterion names GP shall have an automated test in the golden acceptance suite. The acceptance runner reports pass/fail per requirement ID, per AC-nn of [20](20-golden-project.md) and per D-nn regression test, and each milestone gate evaluates its AC subset (20 §22). | Must | MVP | ARCH §13; 18 §3.10; 20 §22 | **PR:** the M1 job prints a table of AC-01 … AC-20 and AC-39 results; a GP-named criterion without a test is listed as missing. |
| NFR-MAINT-12 | Crates that produce persisted or exported bytes shall forbid, by lint, unordered hash collections, wall-clock and random APIs, and environment reads outside the settings module. Persisted records have no floating-point fields. | Must | MVP | NFR-REL-07; ARCH §1.2 (8), §4.5; 18 §3.5 | **PR:** a seeded `HashMap` iteration in `ew-reports` fails clippy. |
| NFR-MAINT-13 | PR CI shall not retry tests. A nondeterministic test is quarantined within one working day with an issue and an owner and keeps running nightly; more than 10 quarantined tests block the next release. | Should | MVP | 18 §3.14 | **A:** release checklist item RC-02. |

---

## 10. LIC: licensing

| Content | Licence (owner decision, ADR-0024) |
|---|---|
| Application and GPL-layer crates | GPL-3.0-or-later with the plugin exception (wording pending, ADR-0025) |
| Permissive crates; built-in rule packs, report packs and machine profiles | MIT OR Apache-2.0 |
| Symbols, frames, forms, page and report templates | CC-BY-SA-4.0 with a KiCad-style design exception |
| Project-authored part data, examples, the GP fixture | CC0-1.0 |
| Documentation | CC-BY-4.0 |

| ID | Requirement | Prio | Release | Source | Acceptance criterion |
|---|---|---|---|---|---|
| NFR-LIC-01 | Every repository path shall carry the licence of the table above, declared in `REUSE.toml` or in file headers. | Must | MVP | ADR-0024 | **PR:** `reuse lint` checks the path-to-licence map. |
| NFR-LIC-02 | Every file shall carry an SPDX licence identifier and copyright (NFR-LIC-10), and `LICENSES/` shall hold every licence text, including the plugin and design exceptions (NFR-LIC-12). | Must | MVP | OSS §7.3; ADR-0024 | **PR:** `reuse lint` passes. |
| NFR-LIC-03 | Every commit shall carry a DCO `Signed-off-by`; there is no CLA. AI-assisted contributions are disclosed, and the human signer owns them. | Must | MVP | ADR-0024; OSS §8.1 | **PR:** the DCO check rejects unsigned commits. |
| NFR-LIC-04 | The application's dependency licences shall be allowlisted in `cargo-deny`: MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2/3-Clause, ISC, Zlib, Unicode-3.0, MPL-2.0, CC0-1.0, OFL-1.1 (fonts) and LGPL-2.1 (OCCT, in `ew-step-worker` only). BSL-1.0 is added if `cargo deny list` at M0 shows it, and the osifont licence expression only if ADR-0028 bundles that font (exact SPDX expression to be verified with counsel). GPL-2.0-only, AGPL and non-OSI licences are denied. ADR-0023 and ADR-0024 use the same list. | Must | MVP | OSS §7.3; ADR-0023, ADR-0024 | **PR:** a seeded GPL-2.0-only crate fails. |
| NFR-LIC-05 | QET-derived code shall live only in `ew-import-qet`, keep its original copyright notices and GPL-2.0-or-later headers, be listed in `NOTICE` and be excluded from the plugin exception. Plugins never link it. | Must | MVP | ADR-0024; C-8 | **PR:** a REUSE rule on `ew-import-qet`; the dependency check blocks `ew-plugin-host` → `ew-import-qet`. |
| NFR-LIC-06 | EW shall bundle no third-party engineering content without an explicit redistribution licence, and record provenance and licence per imported record. Never shipped: IEC 60617 database content (graphics, names or dumps; own drawings only, with an optional S-number reference); ECLASS dictionary content (only user-supplied IRDIs); EDZ or Data Portal content (EDZ import is user-initiated, local and part data only); the QET element collection (imported elements stay in the project's "QET import" library); vendor STEP, DXF and device-description files; Tahoma; licensed standard texts, figures and threshold tables. Third-party content is never used for model training. | Must | MVP | STD §4, §10, §14; IOF §1.12, §9; PMF §6.5; OSS §2.6; G14 | **PR:** the library linter; **A:** a release content audit of every bundled asset. |
| NFR-LIC-07 | Every shipped symbol, part, form and macro record shall carry an SPDX licence and a source field. | Must | MVP | OSS §7.3; BS IDEA-090 | **PR:** the library CI rejects records without them. |
| NFR-LIC-08 | Third-party notices (crates, SQLite, OCCT, fonts) shall be generated with cargo-about, shipped in every artefact and shown in the About dialog. The LGPL obligations for OCCT are met: source offer and a replaceable worker binary. | Must | MVP | RE §10.3; C-5, C-6; FR-UI-038 | **R:** notices present and complete against the SBOM. |
| NFR-LIC-09 | Licence texts and documentation shall state that projects, PDFs, reports and machine files made with EW, including embedded symbol and form copies, are not subject to EW's copyleft. | Must | MVP | ADR-0024; OSS §7.3 | **A:** legal review before the first release. |
| NFR-LIC-10 | Every file's `SPDX-FileCopyrightText` shall be the collective notice "The ElektraWatt contributors" with the year. Files containing third-party material keep their original notices verbatim above it. Headers name no employer or person. | Should | MVP | ADR-0024; K-7; 19 §2.3 | **PR:** the licence-map check rejects a header without the collective notice; files ported from QET keep their original notices. |
| NFR-LIC-11 | Before the first binary release, the steward shall hold written copyright statements from every employer whose staff contributed, starting with the pilot panel builder. The statements cover all contributions since the first commit under the licences of §10, both exceptions and the CC0 publication of the anonymised GP fixture. | Must | MVP | 19 §3.3 (LG-1, LG-5); OSS §7.4 | **A:** the steward's legal checklist; the release pipeline requires the checklist item. |
| NFR-LIC-12 | The plugin exception (ADR-0025) and the design exception shall be in `LICENSES/` and applied in the SPDX headers of the affected paths before the first pull request from a non-maintainer is merged. | Must | MVP | ADR-0025; C-8; 17 §12; 19 §2.1 | **PR:** a check blocks merging a non-maintainer PR while either exception text is missing. |
| NFR-LIC-13 | "ElektraWatt" shall pass a trademark clearance search in Nice classes 9 and 42 before the first binary release. A word mark shall be filed in the steward's name by 1.0, and `TRADEMARKS.md` shall publish the use policy. | Should | MVP / 1.0 | 19 §3.3 (LG-3); OSS §7.5 | **A:** the clearance report is on file; the policy is published with the first binary release. |
| NFR-LIC-14 | A rights holder's complaint against shipped content shall be acknowledged within 7 days. If it is plausible, the content is withheld from the next release of the affected pack or application, the affected pack version is yanked from the index, and the decision is recorded. | Should | 1.0 | NFR-LIC-06; 07 §6; 19 §3.6 | **A:** a takedown tabletop exercise before 1.0. |

---

## 11. DOC: documentation

| ID | Requirement | Prio | Release | Source | Acceptance criterion |
|---|---|---|---|---|---|
| NFR-DOC-01 | User documentation shall be an mdBook in the Diátaxis structure (tutorials, how-to, reference, explanation). It is versioned per release, built and link-checked per PR and bundled offline with the application. Every tool, dialog and panel has an F1 anchor (FR-UI-031). | Must | MVP | G13 | **PR:** every registered tool and dialog ID resolves to an existing anchor. |
| NFR-DOC-02 | Every shipped rule ID (e.g. EW-POT-013, EW-PLC-012, EW-L011) shall have a page: what it checks, why, the standard reference by number and clause (no copied text), a GP example, quick fixes and waiver guidance. F1 on a message opens it. | Must | MVP | G13; ARCH §12; ADR-0019; FR-CHK-015 | **PR:** CI fails if a rule lacks its page or its `docs` reference does not resolve in the bundled docs. |
| NFR-DOC-03 | Tutorials shall exist for: (1) a first schematic page with potentials; (2) a terminal strip and a cable; (4) reports and PDF; (6) importing a QET project and fixing the migration report, at MVP. At 1.0 they are joined by (3) a PLC card and addressing and (5) 2D/3D layout and the wire list. | Must | MVP / 1.0 | G13 | **PR:** every tutorial runs as a scripted CI test. |
| NFR-DOC-04 | Every release shall ship the anonymised GP (CC0) as sample project, tutorial basis and CI fixture. It contains no customer, end-user, project-reference, drawing-id or person identifier. | Must | MVP | VIS §11 (proposal); SC-1; ADR-0024; K-7 | **R:** the sample opens offline and passes its acceptance SQL/CSV; party and person fields hold placeholders, and the guard of NFR-PRIV-09 passes. |
| NFR-DOC-05 | The documentation shall include EPLAN ↔ EW and QET ↔ EW concept glossaries: function, device, part, CDP, interruption point, basic project, form, message management; element, master/slave, folio report, conductor. | Must | MVP | G13 | **A:** reviewed in the usability test (NFR-UX-01). |
| NFR-DOC-06 | Reference documentation shall be generated from code and fail CI when stale: the command catalogue with parameter JSON Schemas, the CLI reference, the file-format specification with JSON Schemas, the `api_v1` view reference, the WIT docs, the rule catalogue and keymap sheets. | Must | MVP | ARCH §7.6, §10; FR-EXT-015 | **PR:** regeneration produces no diff. |
| NFR-DOC-07 | Every release shall have release and migration notes: format changes, new derivation profiles, CLI and API changes and upgrade steps. | Must | MVP | ADR-0011; ADR-0003 | **R:** a release cannot be published without the notes. |
| NFR-DOC-08 | The repository shall contain `LICENSE.md`, `GOVERNANCE.md`, `CONTRIBUTING.md` (with the AI-use policy), `CODE_OF_CONDUCT.md` (Contributor Covenant 3.0), `SECURITY.md`, `PRIVACY.md`, `TRADEMARKS.md` and `ROADMAP.md`, linked from the README. `ROADMAP.md` is reviewed at least quarterly. | Must | MVP | C-13; OSS §8; 19 §2.4 | **PR:** a file-presence check; **R:** the release checklist confirms a roadmap review within the last 100 days. |
| NFR-DOC-09 | Changes to a format major, the `api_v1` view set, the WIT world major, CLI or command stability guarantees, the data-model ontology, licences or governance shall have an accepted RFC in `rfcs/` after a final-comment period of at least 7 days (14 for licences and governance), and the implementing PR references it. | Should | MVP | ARCH §7.6; OSS §8.2; 19 §3.4 | **PR:** a PR labelled `format-major`, `api-break` or `licence` without an RFC reference fails. |

---

## 12. COMP: regulatory readiness

Dates follow G11: CRA reporting duties apply from 2026-09-11, the other obligations from 2027-12-11. Whether a legal
steward exists and what role it takes is ADR-0030 (proposed).

| ID | Requirement | Prio | Release | Source | Acceptance criterion |
|---|---|---|---|---|---|
| NFR-COMP-01 | Before the first public release, the project shall have steward-grade CRA processes: a cybersecurity policy, vulnerability handling (NFR-SEC-14), SBOMs (NFR-SEC-13), a procedure for notifying actively exploited vulnerabilities and severe incidents, and a contact for market-surveillance authorities. | Must | MVP | G11; ADR-0023, ADR-0030 | **A:** a steward checklist reviewed with counsel. |
| NFR-COMP-02 | No paid offering (support, LTS build) shall be sold under the ElektraWatt name until the manufacturer obligations are budgeted and in place: technical documentation, the Annex I requirements, a support period normally ≥ 5 years, an EU declaration of conformity and CE self-assessment. Personal-data processing is never a condition of use. | Must | MVP | G11; ADR-0030 | **A:** governance review before any commercial announcement. |
| NFR-COMP-03 | `SECURITY.md` shall publish the support period: the latest minor gets all fixes; each minor gets security and data-loss fixes for 6 months after its successor ships. An LTS line exists only together with a commercial decision. | Must | MVP | ADR-0030 (proposed); OSS §8.3; 18 §7.5 | **A:** the table matches the published releases. |
| NFR-COMP-04 | The UI (About, release dialog, check panel), the documentation, the licence texts and every release report shall state that EW's checks do not replace engineering verification. Release reports list rule-pack versions and waivers. Thermal reports, PE-continuity lists and verification checklists state their method and data sources and that they do not replace verification by a competent person. | Must | MVP / 1.0 | ADR-0030; G11 (PLD 2024/2853); VIS §11, 02b proposals; FR-UI-038 | **PR:** a text assertion on the GP release report; kittest checks the About and check panels. |
| NFR-COMP-05 | Standards shall be cited by number and clause, never reproduced as copyrighted text, tables or figures. Rule constants from unverified sources ship with a `verified` flag. | Must | MVP | STD §0, §14; 02b proposal (FR-CHK) | **A:** the release content audit (NFR-LIC-06). |
| NFR-COMP-06 | At 1.0, an Accessibility Conformance Report against EN 301 549 shall be published, covering clause 11 (software) and clause 12 (documentation) and listing known gaps, such as canvas graphics that are conveyed through the navigators. | Should | 1.0 | G8 | **A:** the report is published with 1.0. |
| NFR-COMP-07 | The steward shall publish a yearly funding statement listing income by source and its use, so that non-monetisation under the CRA can be demonstrated. An income source other than donations, grants or cost reimbursement needs the review of NFR-COMP-02 first. | Should | MVP | G11; ADR-0030; 19 §3.9 | **A:** the statement is published with the first release after each calendar year. |
| NFR-COMP-08 | Before the first binary release, the source repository, the content repositories and the release environment shall belong to a GitHub organisation controlled by the steward, with at least two owners, and no release secret shall depend on a personal account. | Should | MVP | ADR-0023, ADR-0030; 19 §3.3 (LG-4) | **A:** the steward checklist (ST-08) lists the organisation owners; **R:** the release workflow runs only in the organisation's protected `release` environment. |

---

## 13. UX: learnability and usability

| ID | Requirement | Prio | Release | Source | Acceptance criterion |
|---|---|---|---|---|---|
| NFR-UX-01 | Before M2, a moderated usability test with at least two EPLAN users and one QET user shall reach ≥ 80 % unassisted completion of the core tasks (place, wire, terminal strip, renumber, export PDF) on GP. It is repeated at 1.0 with at least five users, adding a layout task and a PLC task. Severe findings are fixed before milestone exit. | Must | MVP / 1.0 | SC-9; VIS §11 (proposal); ADR-0007 | **S:** protocol and results in [UX](16-ux-ui.md); the formative round of NFR-UX-09 precedes the M2 test. |
| NFR-UX-02 | The System Usability Scale score shall be ≥ 68 at M2 and ≥ 75 at 1.0 (new targets). | Should | MVP / 1.0 | — | **S:** SUS questionnaire in the NFR-UX-01 sessions. |
| NFR-UX-03 | EPLAN and QET vocabulary shall work as command-palette aliases (e.g. "folio report", "master/slave", "plot frame"), the EPLAN-like keymap is validated by two EPLAN users at 1.0, and UI terms follow IEC 81346 and EPLAN usage (device, function, CDP, interruption point). | Must | MVP / 1.0 | G5, G13; FR-UI-012, FR-UI-014 | **PR:** palette tests for each glossary term of NFR-DOC-05; **S:** keymap confirmation. |
| NFR-UX-04 | Starting from the company basic project and without documentation, a QET user shall draw the folio 102 circuit (-102G1, -102F1, -102F2 with wires) and export a PDF within 30 minutes (new target). | Should | MVP | G13; FR-UI-030 | **S:** timed task in the NFR-UX-01 sessions. |
| NFR-UX-05 | The pilot user shall complete three consecutive new production projects in EW without QET, with no hand-maintained report page, no slower than their own QET baseline on a reference-size project. | Must | MVP | SC-3 | **S:** pilot log and timing; the clock starts at G-M2 and the criterion closes at G-M3 ([gates](../plan/03-gates.md#83-success-criteria-by-gate) §8.3). |
| NFR-UX-06 | Importing a reference-size QET project of the pilot user and resolving its migration report to zero errors shall take the pilot user at most 1 hour (new target). | Should | MVP | SC-4; FR-IMP-003 | **S:** pilot log; in the plan's lean variant the migration ships after the MVP and SC-4 closes at G-M3 (vision §9.1). |
| NFR-UX-07 | Every edit shall be undoable or flagged as a marker line in the history; destructive non-undoable operations (library writes outside the project, overwriting an existing file, purging library copies, the empty-project import checkpoint) are confirmed (ARCH §5.3). Every bulk command is previewable, no modal dialog blocks editing while a job runs, Escape cancels every tool, and every message states cause and remedy and carries its rule ID. | Must | MVP | ARCH §5.3, §5.4; ADR-0008; FR-UI-022, FR-UI-023 | **PR:** kittest cancels each registered tool with Escape; exporting the GP PDF to a new file asks nothing and adds a marker line, while exporting over an existing file asks once; a lint checks that every Fluent message of a rule has a remedy key. |
| NFR-UX-08 | GP shall reproduce from the shipped defaults (EN 81346-2:2009 letter codes, QET-compatible frame, xref format) and the pilot's company basic project, with no further settings edits. | Must | MVP | ADR-0026; G13; FR-PRJ-009 | **PR:** the acceptance runner builds GP from the defaults and the basic project only. |
| NFR-UX-09 | A formative usability round (R0) with at least two participants shall run as soon as the viewer and the place and wire tools exist, which the plan of record reaches with the pilot preview (M-PP; checkpoint CP-05 in [gates](../plan/03-gates.md#25-checkpoints-between-gates) §2.5); its findings are triaged before the R1 round and the M2 test. | Should | MVP | SC-9; ADR-0007; 16 §19.2 (round R0) | **S:** the round's report exists before the M2 test of NFR-UX-01 is scheduled. |

---

## Requirements covered

This catalogue **defines** NFR-PERF-01 … NFR-PERF-25, NFR-PLAT-01 … NFR-PLAT-08, NFR-REL-01 … NFR-REL-17,
NFR-SEC-01 … NFR-SEC-21, NFR-PRIV-01 … NFR-PRIV-10, NFR-A11Y-01 … NFR-A11Y-06, NFR-I18N-01 … NFR-I18N-07,
NFR-MAINT-01 … NFR-MAINT-13, NFR-LIC-01 … NFR-LIC-14, NFR-DOC-01 … NFR-DOC-09, NFR-COMP-01 … NFR-COMP-08 and
NFR-UX-01 … NFR-UX-09 (147 rows; counts below).

It sets the quality targets of these functional requirements: FR-PRJ-009, FR-PRJ-011, FR-PRJ-024, FR-OUT-002,
FR-OUT-003, FR-OUT-009, FR-OUT-012, FR-OUT-019, FR-OUT-025, FR-CHK-013, FR-CHK-015, FR-REV-006, FR-UI-007,
FR-UI-012, FR-UI-014, FR-UI-015, FR-UI-020, FR-UI-022 … FR-UI-028, FR-UI-030 … FR-UI-032, FR-UI-038, FR-EXP-003,
FR-EXT-001, FR-EXT-014 … FR-EXT-017, FR-EXT-020, FR-EXT-026, FR-COL-003 … FR-COL-005, FR-COL-010, FR-COL-011,
FR-COL-029, FR-IMP-003, FR-IMP-005, FR-I18N-009 … FR-I18N-011, FR-I18N-015.

| Area | Elaborated in |
|---|---|
| PERF, MAINT, SEC, LIC (mechanics), COMP; PLAT-08, PRIV-04 | [18-quality-security-release](18-quality-security-release.md) |
| REL (save, recovery, format, archive), PRIV-05, SEC-16 | [05-project-format-and-persistence](05-project-format-and-persistence.md), [15-checks-revisions-collaboration](15-checks-revisions-collaboration.md) |
| A11Y, UX, I18N (UI) | [16-ux-ui](16-ux-ui.md) |
| SEC-05 … SEC-08, MAINT-10 | [17-extensibility](17-extensibility.md) |
| LIC, DOC, COMP-01 … COMP-03, COMP-07, COMP-08, PRIV-10 | [19-open-source-governance](19-open-source-governance.md) |
| PERF fixtures, DOC-04, UX-08, PRIV-09, MAINT-11 | [20-golden-project](20-golden-project.md) |
| PERF-16 … PERF-18, PERF-23 | [12-panel-layout](12-panel-layout.md) |
| PERF-21 | [07-symbols-parts-macros](07-symbols-parts-macros.md) |
| PERF-22 | [10-plc-and-bus](10-plc-and-bus.md) |
| PERF-24, SEC-02 (package readers) | [13-manufacturing-outputs](13-manufacturing-outputs.md) |
| PERF-25, SEC-18 | [14-interop](14-interop.md) |
| SEC-17 | [09-reports-forms-output](09-reports-forms-output.md) |
| REL-17 | [ADR-0028](../adr/0028-default-drawing-fonts.md) |

### Counts

| Area | Rows | Must | Should | Could | MVP | MVP / 1.0 | 1.0 | 1.x |
|---|---|---|---|---|---|---|---|---|
| PERF | 25 | 17 | 8 | 0 | 8 | 11 | 6 | 0 |
| PLAT | 8 | 7 | 1 | 0 | 6 | 1 | 1 | 0 |
| REL | 17 | 14 | 3 | 0 | 12 | 2 | 3 | 0 |
| SEC | 21 | 16 | 4 | 1 | 16 | 2 | 1 | 2 |
| PRIV | 10 | 6 | 4 | 0 | 9 | 1 | 0 | 0 |
| A11Y | 6 | 4 | 2 | 0 | 4 | 0 | 2 | 0 |
| I18N | 7 | 4 | 3 | 0 | 4 | 2 | 1 | 0 |
| MAINT | 13 | 10 | 3 | 0 | 13 | 0 | 0 | 0 |
| LIC | 14 | 11 | 3 | 0 | 12 | 1 | 1 | 0 |
| DOC | 9 | 8 | 1 | 0 | 8 | 1 | 0 | 0 |
| COMP | 8 | 5 | 3 | 0 | 6 | 1 | 1 | 0 |
| UX | 9 | 5 | 4 | 0 | 6 | 3 | 0 | 0 |
| **Total** | **147** | **107** | **39** | **1** | **104** | **25** | **16** | **2** |

"MVP / 1.0" rows apply from MVP for S and M projects, or for the part named first (§1.1). No row is "Later".

### Adopted proposals

The provisional IDs proposed upstream map to these rows. "Merged" means the proposal refined an existing row.

| Source | Proposal → row |
|---|---|
| [VIS](00-vision-and-scope.md) §11 | NFR-DOC-nn (golden sample) → NFR-DOC-04; NFR-COMP-nn (verification disclaimer) → NFR-COMP-04; NFR-UX-nn (usability test) → NFR-UX-01 |
| [02a](02a-requirements-core.md) | NFR-REL-nn (atomic release) → NFR-REL-15; NFR-PERF-nn (online rule budget) → NFR-PERF-09 (merged); NFR-MAINT-nn (automated test per GP criterion) → NFR-MAINT-11 |
| [02b](02b-requirements-extended.md) | NFR-PERF-nn (routing) → NFR-PERF-16; NFR-PERF-nn (collision) → NFR-PERF-17; NFR-PERF-nn (STEP) → NFR-PERF-18; NFR-REL-nn (determinism) → NFR-REL-07; NFR-SEC-nn (fuzzing) → NFR-SEC-03; NFR-LIC-nn (bundled content) → NFR-LIC-06; NFR-COMP-nn (thermal and verification disclaimers) → NFR-COMP-04 |
| [05](05-project-format-and-persistence.md) | NFR-REL-nn (incremental save equals full save) → NFR-REL-16; NFR-REL-nn (atomic release) → NFR-REL-15, shared with 02a; NFR-SEC-nn (loader reads regular project files only) → NFR-SEC-16 |
| [07](07-symbols-parts-macros.md) | NFR-PERF-nn (part search) → NFR-PERF-21 |
| [09](09-reports-forms-output.md) | P09-12 → NFR-SEC-17 |
| [10](10-plc-and-bus.md) | NFR-PERF-nn (j) → NFR-PERF-22 |
| [12](12-panel-layout.md) | NFR-PERF-nn (thermal recalculation) → NFR-PERF-23 |
| [13](13-manufacturing-outputs.md) | P13-9 → NFR-PERF-24; P13-10 → NFR-SEC-02 (merged; NFR-SEC-03 covers every parser of NFR-SEC-02) |
| [14](14-interop.md) | NFR-SEC-nn (importers read only the selected source) → NFR-SEC-18; NFR-PERF-nn (QET import times) → NFR-PERF-25 |
| [16](16-ux-ui.md) | P16-10 → NFR-UX-09 |
| [17](17-extensibility.md) | NFR-MAINT-10 → same number |
| [18](18-quality-security-release.md) | P18-1 → NFR-SEC-15 (merged); P18-2 → NFR-SEC-19; P18-3 → NFR-SEC-20; P18-4 → NFR-PRIV-04 (merged); P18-5 → NFR-MAINT-12; P18-6 → NFR-MAINT-13; P18-9 → NFR-SEC-21; P18-10 → NFR-PLAT-08 |
| [19](19-open-source-governance.md) | P19-1 → NFR-LIC-10; P19-2 → NFR-LIC-11; P19-3 → NFR-LIC-12; P19-4 → NFR-LIC-13; P19-5 → NFR-DOC-08; P19-6 → NFR-DOC-09; P19-7 → NFR-PRIV-10; P19-8 → NFR-COMP-07; P19-9 → NFR-LIC-14; P19-10 → NFR-COMP-08 |
| [20](20-golden-project.md) | NFR-PRIV-nn (anonymisation guard) → NFR-PRIV-09; NFR-MAINT-nn (acceptance runner) → NFR-MAINT-11, shared with 02a |
| [ADR-0028](../adr/0028-default-drawing-fonts.md) (recommendation 6) | NFR-REL-nn (pinned font versions) → NFR-REL-17 |

### Rows proposed to 02a and 02b

This file's own proposals for the functional catalogues were adopted as follows: FR-OUT-nnn (omit personal data) →
FR-OUT-025; FR-REV-nnn (schemas and format specification in the archive) → not added, because NFR-REL-10 covers
it; FR-CHK-nnn (rule `docs` reference) → FR-CHK-015 (existing); FR-COL-nnn (backup before a format-major rewrite)
→ FR-COL-029; FR-EXT-nnn (MCP loopback token) → FR-EXT-026; FR-UI-nnn (About dialog) → FR-UI-038, which also
carries the support status proposed by 18 [P18-8].

---

## Open questions

1. **XL bounds versus the architecture's estimate case.** G6 bounds XL at 40,000 devices, while ARCH §6.7 estimates
   for 50,000 devices on 2,000 pages and 200,000 connections. This file takes the larger value of each source
   (5,000 pages, 50,000 devices, 250,000 connections), so the XL fixture is about 25 % above the estimate case.
   *Recommendation:* gate on the class bound, keep XL-A for comparison with the estimates, and recalibrate L and XL
   from the pilot's largest projects before the performance spike (G-SP). Lower the bounds rather than relax the budgets.
2. **Pure formulas in untrusted projects.** rhai formulas inside queries (tag and wire-number formats, title-block
   formulas) are scripts, and without them an untrusted project cannot render. ARCH §10.6 says embedded scripts never
   auto-run. *Recommendation:* evaluate pure formulas in untrusted projects, because they run with no I/O, under
   operation limits and through tracked reads only (ARCH §10.3). Let trust gate automation scripts, SQL, rule packs
   and plugins. Record the distinction in ADR-0020.
3. **Windows 10.** Mainstream support for Windows 10 22H2 has ended, and extended updates are time-limited
   (**verify** dates), but shop-floor PCs often run it or an LTSC edition (G6; gaps-and-corrections open question 2).
   *Recommendation:* keep Windows 10 22H2 and LTSC 2021 Supported through 1.0, then review against the pilot's fleet.
4. **macOS minimum version.** Rust's Tier 1 floor is macOS 11, which Apple no longer patches. *Recommendation:*
   support the three most recent macOS major versions at each EW release, matching Apple's security-update window.
5. **GPU-less Linux and 3D without an adapter.** wgpu on llvmpipe or lavapipe is unverified in wgpu's CI, and the
   CPU renderer is 2D only. *Recommendation:* guarantee 2D through the CPU renderer, treat software Vulkan as best
   effort for 3D, add a lavapipe CI job, and ask the pilot whether RDP users need the 3D view at all.
6. **Per-PR performance signal.** The architecture gates timings nightly, so a regression is found a day late.
   *Recommendation:* keep timings nightly on the dedicated runner, where they are not noisy, and assert the exact
   engine-statistics counts per PR (NFR-PERF-20). Counts do not depend on runner speed.
7. **Targets marked "new target".** These are L commit p99, bulk-preview, full-save, cancel, soak, fuzz durations,
   advisory waivers, disclosure times, coverage, CI wall time, SUS and task times. *Recommendation:* confirm the
   performance ones at the performance spike (G-SP), the usability ones at the M2 study and the security ones with the steward
   (ADR-0030). Then remove the "new target" marks.
8. **Pending owner decisions.** NFR-PRIV-04 (crash upload) depends on ADR-0029, and NFR-COMP-01 … 03,
   NFR-COMP-07, NFR-COMP-08, NFR-LIC-11, NFR-LIC-13 (the steward's role) and NFR-SEC-12 (signing identity) depend
   on ADR-0030. NFR-LIC-12 needs the exception wording of ADR-0025, and NFR-REL-17 assumes the bundled fonts of
   ADR-0028. All four ADRs are proposed. *Recommendation:* ship local-only crash reports at MVP, and decide
   ADR-0025 and ADR-0030 before the first non-maintainer PR and the first signed release, as VIS open question 9
   proposes.
9. **PDF/UA validation tool.** That veraPDF validates PDF/UA-1 is general knowledge, not verified in the research
   notes. *Recommendation:* verify it during the spine, and fall back to the PDF Association's PAC checker as a
   manual **M** test if needed.
10. **EN 301 549 demand.** The European Accessibility Act most likely does not cover B2B ECAD (G8), so the report of
    NFR-COMP-06 matters only for public-sector tenders. *Recommendation:* keep it at 1.0 Should, and promote it to
    Must only if a public-sector user appears.
