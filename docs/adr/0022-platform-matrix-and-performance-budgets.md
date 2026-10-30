# ADR-0022: Platform matrix and performance budgets (S/M/L/XL) as CI-enforced requirements

**Status:** Accepted · amended 2026-10-30 (macOS not supported, decision log ID-880) · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the judge panel)

## Context

- **No sizing until G6.** No research note sized projects or named a platform baseline until the gap review G6. The
  only data points were EPLAN P8 Basic's 40-page cap and the reference project: 18 drawn folios and about 44 wires.
  The pilot's larger projects and their customers' multi-cabinet lines are unknown.
- **The engine decision needs numbers.** The derived-data engine starts cold and memoises only where profiling
  demands it (ADR-0002). The performance spike (gate G-SP; effort weeks 3–5 of the spine, 04 §15.1) makes that go/fallback decision, and it is meaningless without
  agreed sizes and budgets.
- **Rust platform tiers.** Tier 1 with host tools covers Windows 10+ (x64, ARM64), macOS 11+ on arm64 and Linux
  with glibc 2.17+. Intel macOS has been Tier 2 since its demotion. Since amendment 1, EW relies on the Windows and
  Linux rows only.
- **GPU-less hosts.** Panel builders work over RDP, Citrix or VDI without a GPU (G6, PQ-25). wgpu's DX12 backend
  handles WARP, and the always-built CPU renderer exists for exactly this case (ADR-0015).
- **Budgets drift.** Performance budgets that are written down but not enforced drift silently. A memo-invalidation
  bug can also hide inside a time budget while doing ten times too much work.

## Decision

1. **Platform tiers (v1):**

   | Platform | Tier |
   |---|---|
   | Windows 11 and Windows 10 22H2+ (x64) | Supported |
   | Ubuntu 22.04+ and Debian 12+ (x64), Wayland and X11 | Supported |
   | Windows ARM64, Linux arm64 | Best effort |
   | Other Linux distributions through Flatpak or AppImage | Community |
   | macOS on Apple silicon | Not supported (ID-880) |
   | macOS on Intel | Not supported (ID-880) |

   - **Supported** platforms are built and tested in CI, and a failure blocks the release.
   - **Best-effort** platforms are built for every release and fixed when reported.
   - **Not supported** platforms are neither built nor tested (amendment 1).
   - **Backends.** wgpu runs on DX12 and Vulkan. GL and GLES are untested.
   - **GPU-less fallback order:** DX12 WARP, then `ew-render-cpu`. The 3D view uses a software adapter where one
     exists (ADR-0016).
   - **The CLI**, including `ew mcp`, runs in a Linux container without a display or GPU.
2. **Baseline machines.**
   - **BM:** a 4-core 2018+ x64 CPU, 8 GB RAM (16 GB recommended at L/XL), an integrated GPU with DX12 or Vulkan,
     1920 × 1080 and a local SSD.
   - **BM-noGPU:** the same machine without a hardware adapter.
3. **Size classes.** These are the G6 bounds, with the XL device bound raised from 40,000 to 50,000 to cover the
   ARCH §6.7 estimate case XL-A ([02c](../design/02c-nonfunctional-requirements.md) §2.1). XL always means the class
   bound below, and XL-A the estimate case:

   | Class | Pages | Devices | Connections | CPs | Terminals | PLC channels | 3D placements |
   |---|---|---|---|---|---|---|---|
   | S | ≤ 50 | ≤ 300 | ≤ 1,500 | ≤ 3,750 | ≤ 120 | ≤ 60 | ≤ 200 |
   | M | ≤ 300 | ≤ 2,500 | ≤ 15,000 | ≤ 37,500 | ≤ 1,000 | ≤ 500 | ≤ 800 |
   | L | ≤ 1,500 | ≤ 12,000 | ≤ 70,000 | ≤ 175,000 | ≤ 4,800 | ≤ 2,400 | ≤ 3,000 |
   | XL | ≤ 5,000 | ≤ 50,000 | ≤ 250,000 | ≤ 625,000 | ≤ 20,000 | ≤ 10,000 | ≤ 10,000 |

   The golden project is S. XL is a stretch class for plant and OEM projects.
4. **Budgets are non-functional requirements, not estimates.** A selection, on BM:

   | Operation | S/M | L | XL | Requirement |
   |---|---|---|---|---|
   | Open to the first interactive page | ≤ 0.5 s / ≤ 2 s | ≤ 5 s | ≤ 20 s | NFR-PERF-03 |
   | Frame while editing (p99): apply ≤ 2 ms, derive ≤ 5 ms, render ≤ 5 ms | 16 ms | 16 ms | 16 ms | NFR-PERF-04 |
   | Commit of a move or wire gesture (p99) | ≤ 5 ms | ≤ 10 ms | ≤ 20 ms | NFR-PERF-05 |
   | Pan and zoom | dense A3 ≥ 60 fps; A4 ≥ 30 fps on BM-noGPU | | | NFR-PERF-06 |
   | Change touching many derived values (e.g. the 24 V DC colour in the wire profile; about 60k wires at XL) | ≤ 30 ms | ≤ 60 ms | ≤ 100 ms | NFR-PERF-07 |
   | Full check run | ≤ 2 s | ≤ 10 s | ≤ 60 s | NFR-PERF-09 |
   | Paginate every report block cold | ≤ 3 s | ≤ 15 s | ≤ 60 s | NFR-PERF-10 |
   | Full SQL projection build (job) | ≤ 1 s | ≤ 4 s | ≤ 10 s | NFR-PERF-14 |
   | PDF export | ≤ 50 ms per page; 500 pages ≤ 10 s | | | NFR-PERF-11 |
   | Incremental save | ≤ 200 ms | ≤ 500 ms | ≤ 2 s | NFR-PERF-12 |
   | Peak memory | ≤ 500 MB | ≤ 1.5 GB | ≤ 4 GB | NFR-PERF-13 |
   | Longest UI-thread occupancy | ≤ 100 ms; cancel effective ≤ 500 ms | | | NFR-PERF-19 |
   | 3D and routing (1.0) | `RouteWires` on 2,000 wires ≤ 5 s, one wire ≤ 50 ms; collision re-check of 500 placements ≤ 10 ms; 20 MB STEP import ≤ 30 s, cached reopen ≤ 1 s | | | NFR-PERF-16 … NFR-PERF-18 |

5. **Fixtures.** `ew-testkit` generates one synthetic project per class at its upper bound by replicating Golden B
   pages, wiring, strips, PLC stations and placements. It also generates **XL-A**, the ARCH §6.7 estimate case
   (2,000 pages, 50k devices, 200k connections), so that measurements can be compared with the estimates.
6. **The gate.** criterion 0.8.2 benchmarks run nightly on a dedicated BM-class runner over all fixtures. The
   benchmarks cover open, warm-up, move, draw wire, renumber folio, profile change, BOM, full check, 500-page PDF,
   save and projection build.
   - **Red gate.** Any budget miss, or a regression of more than 15 % against the stored baseline, turns the gate red.
     A red gate blocks release.
   - **Engine statistics** (memo hits, re-executions, cut-offs per edit) are asserted as **exact counts on every
     PR**. For example, a wire edit on folio 102 re-executes no query keyed to folio 203.
   - **Memory gate.** It covers peak RSS and memo bytes per query family.
   - **GPU-less jobs.** A DX12 WARP job and a CPU-renderer job run the kittest golden flows.
7. **Measurement rules.**
   - A single operation passes on the median of 10 runs, with no run above 1.5 × the budget.
   - A latency passes on its p99 over at least 1,000 samples of a scripted gesture sequence.
   - A budget changes only through a reviewed change to [02c](../design/02c-nonfunctional-requirements.md) that cites
     a measurement.

## Amendments

1. **macOS is not supported** (2026-10-30, project owner; decision log ID-880). macOS on Apple silicon (Supported
   before) and on Intel (Best effort before) become **Not supported**: no CI job, no artefact, no installer, no
   signing or notarisation, no Metal testing. The reasons: the two macOS CI jobs were the slowest of the pull-request
   set and caused the cycle-36 float-drift incident; notarisation, an Apple Developer ID, a Mac installer and Metal
   testing would add about 1.5–2.5 person-weeks to WP-S6.06 and WP-S6.07; the target users run Windows; determinism
   now holds across Windows and Linux. **The code stays portable:** the existing `cfg(target_os = "macos")` branches,
   the Apple-only dependency tables and the Cmd key display are kept, but EW writes no new macOS code and promises
   nothing for macOS; a contributor may build there at their own risk. The `cfg(target_os = "macos")` code kept for
   portability is dormant and unmaintained: CI no longer compiles it, so it may not build. macOS can return only
   through a new decision.
   The tier table, the backends, the baseline machine, the consequences and the alternatives follow this amendment.

## Consequences

**Positive**
- Performance and memory regressions are caught within a day. Memo-invalidation regressions are caught on the PR
  itself, through the exact statistics, before they show up as time.
- The performance spike (gate G-SP), the M1 spine exit (XL opens in ≤ 20 s; commit p99 ≤ 5 ms at M and ≤ 20 ms at XL) and the nightly
  gate use the same fixtures and numbers.
- RDP and VDI users are first-class from MVP rather than an afterthought.

**Negative**
- **Assumed sizes.** The XL numbers are assumptions. They must be recalibrated against the pilot user's three largest
  projects before the performance spike (gate G-SP) ([vision](../design/00-vision-and-scope.md) OQ5; G6 open question 1), and the
  budgets may move then.
- **A dedicated runner.** Stable timings need a dedicated bare-metal runner. On a public repository that runner must
  be isolated from secrets and must never run fork PRs
  ([18](../design/18-quality-security-release.md) OQ2).
- **No macOS** (amendment 1). Users on a Mac run EW only in a Windows or Linux virtual machine, or build it
  themselves without support.
- **CI cost.** It grows: per-platform suites (Windows and Linux), WARP and CPU jobs, nightly XL runs.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **Budgets defined after M2** | Too late. The engine and memoisation decisions of ADR-0002 are taken at the performance spike (gate G-SP) and need targets |
| **GPU-only rendering** | Fails on GPU-less RDP and VDI hosts, which pilot-type users run, and forces GPU runners into CI |
| **Timing-only gates** (no engine statistics) | Noisy on shared runners, and blind to memo bugs that recompute too much but still fit the budget at S |
| **Intel macOS as Supported** | It is Rust Tier 2, so EW could not promise more than the toolchain does (history: since amendment 1 no macOS is supported) |
| **Supporting Windows 8.1 or macOS before 11** | Outside Rust Tier 1 host support (Windows 10+, macOS 11+) (history: since amendment 1 no macOS is supported) |
| **Keeping macOS on Apple silicon Supported** (amendment 1) | Two slow CI jobs, notarisation, a Developer ID, a Mac installer and Metal testing for a platform the target users do not run |
| **Hosted runners for the gate** | Their variance exceeds the 15 % threshold, so hosted timings stay informational only |

## Related

- **ADRs:** [ADR-0002](0002-derived-data-engine-ew-memo.md), [ADR-0014](0014-gui-toolkit-egui.md), [ADR-0015](0015-display-list-renderers-pdf.md), [ADR-0016](0016-3d-renderer-wgpu-and-out-of-process-step-worker.md),
  [ADR-0023](0023-supply-chain-and-release-integrity.md).
- **Design:** [04 architecture](../design/04-architecture.md) §2, §6.7, §13, §15.1;
  [02c non-functional requirements](../design/02c-nonfunctional-requirements.md) §1.4, §2, §3;
  [18 quality, security and release](../design/18-quality-security-release.md) §4–§5;
  [gaps-and-corrections](../research/gaps-and-corrections.md) G6.
- **Requirements:** NFR-PERF-01 to NFR-PERF-20; NFR-PLAT-01 to NFR-PLAT-07; NFR-REL-14.
