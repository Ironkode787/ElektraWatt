# ElektraWatt

ElektraWatt (EW) is an open-source, offline-first desktop application for electrical control-cabinet design, written
in Rust. It aims at the functionality of EPLAN Electric P8 class tools: schematics, devices and parts, terminals and
cables, PLC and fluid power, panel layout, reports, manufacturing outputs and checks, in a plain-text project format
that works with Git.

## Status

Development runs in short AI-assisted cycles: one lead session plans each cycle, delegates the work to lanes, has
every lane reviewed independently, records the decisions, runs the full gate on Windows and Linux and merges the
cycle into `main` by fast-forward. Twenty-two cycles have been delivered. Where that stands against the milestones
of the [global plan](docs/plan/01-global-plan.md):

| Milestone | State |
|---|---|
| M0 Foundations | Technical part delivered (workspace, CI on three operating systems, licensing and the licence-boundary check); the remaining gate items are the project owner's |
| M1 Spine exit | Delivered with follow-up tasks: the model, commands, derived data, checks, reports and the PDF of the reference project are produced headlessly; the gate record is in [docs/plan/gate-records](docs/plan/gate-records) |
| M-PP Pilot preview | In progress: the application shell (new project, the company-standard wizard, docking, help) and the 2D canvas (CPU tile path and GPU path) are delivered; the viewer with page navigation, tabs and the tool framework is specified and being built; editing tools follow |
| M2 MVP and later | Not started |

What works today:

- **The `ew` command-line tool**: import of QElectroTech projects, check schemes, a read-only SQL projection, cable
  list and bill-of-materials exports, PDF/A-2b and SVG export of every page, applying recorded command files,
  verification of the derived data. Run `cargo run -p ew -- help` for the full usage.
- **The `elektrawatt` desktop application**: opens a project as a viewer with the page list, a zoomable canvas with
  the connection grid and a status bar, creates new projects through the first-run and company-standard wizards,
  docks panels into named workspaces, and runs the same actions from a UI script without a window
  (`elektrawatt --headless --script <file>`), which is how the UI is tested.
- **The reference project** `golden/J-E-2416.ew` (36 pages, anonymised) with its expected outputs, and the acceptance
  runner `cargo xtask acceptance` that checks the acceptance criteria of the design against them.
- About 3,000 tests across 45 crates and three binaries, with determinism lints (no wall clock, no environment reads
  and no hash-ordered collections in the model, derivation, format and export paths), property tests, a fuzzer and a
  golden-file oracle.

The calendar months in the plan documents are a reference for a human team; progress is tracked in cycles in
[global plan §3.8](docs/plan/01-global-plan.md#38-cycle-based-progress-view-plan-of-record-since-2026-10-20).

## Building and running

The pinned toolchain in `rust-toolchain.toml` is installed automatically by rustup. Tests run with
[cargo-nextest](https://nexte.st).

```sh
cargo build --workspace --locked
cargo nextest run --workspace
cargo xtask ci      # fmt, clippy, nextest, doctests and the repository policy checks, as in CI
```

To look at the reference project, work on a copy of it, because the application and the tool keep a lock and a
journal beside an open project:

```sh
cp -r golden/J-E-2416.ew /tmp/J-E-2416.ew
cargo run -p ew -- export pdf /tmp/J-E-2416.ew --out /tmp/J-E-2416.pdf
cargo run -p elektrawatt -- /tmp/J-E-2416.ew
```

The window needs a graphics adapter; without one it exits with code 3 and names the headless mode. `--no-gpu` draws
the canvas on the CPU. The user guide is built from [docs/book](docs/book) with mdBook.

## Continuous integration

Every pull request builds and tests on Linux, Windows and macOS, runs the UI headlessly in a container, validates the
exported PDF with veraPDF, fuzzes the changed targets for a minute and checks the supply chain (`cargo deny`,
`cargo vet`). Nightly runs fuzz for longer, run the memo oracle, property tests and sanitizers, build an unsigned
development CLI and report performance without gating it; coverage runs weekly. The jobs are described in
[ci/README.md](ci/README.md). The performance criteria are deliberately not measured during development (owner's
rule, decision log ID-512); they are addressed in the release phase.

## Key decisions

| Topic | Decision |
|---|---|
| Language | Rust |
| Platforms | Cross-platform desktop (Windows, macOS, Linux), offline-first ([ADR-0022](docs/adr/0022-platform-matrix-and-performance-budgets.md)) |
| Project format | Plain-text JSONL records in a directory, lossless on open, Git-friendly ([ADR-0010](docs/adr/0010-project-storage-jsonl-directory.md)) |
| UI toolkit | egui on wgpu with a CPU fallback for the canvas ([ADR-0014](docs/adr/0014-gui-toolkit-egui.md), [ADR-0015](docs/adr/0015-display-list-renderers-pdf.md)) |
| v1 scope | Electric P8-class schematics, PLC and I/O, fluid power, 3D panel layout, manufacturing outputs, QET/EDZ/DXF import ([vision](docs/design/00-vision-and-scope.md)) |
| Application licence | GPL-3.0-or-later |
| Format crates | MIT OR Apache-2.0 |
| Symbols and templates | CC-BY-SA-4.0, with a design exception |
| Part data and examples | CC0 |
| Documentation | CC-BY-4.0 |

The licence texts are in `LICENSES/`, indexed by [LICENSE.md](LICENSE.md); the licence set and the boundary between
the permissive and the GPL crates are in [ADR-0024](docs/adr/0024-licence-set-and-permissive-gpl-boundary.md) and
checked on every build. All 30 architecture decision records are listed in the [ADR index](docs/adr/README.md); the
[decision log](docs/plan/06-decision-log.md) records every ruling taken during the cycles.

## Where to start

- [Documentation map](docs/README.md): reading orders for decision makers, developers and domain experts.
- [ADR index](docs/adr/README.md), [plan guide](docs/plan/README.md) and the
  [work breakdown](docs/plan/02-work-packages.md) with the follow-up tasks of every delivered package.
- The [reference project](docs/design/20-golden-project.md) and the [quality, security and release](docs/design/18-quality-security-release.md)
  document, which define what the tests and the CI prove.
- [CLAUDE.md](CLAUDE.md) and [.claude/agents](.claude/agents): the model-routing setup for AI-assisted work (Opus for
  lead decisions, Sonnet for specified work, Haiku for mechanical tasks) and the testing policy.

## Branches

`main` holds the delivered cycles, each merged by fast-forward after the full gate on Windows and Linux. The
integration branch of the running cycle is named in the [decision log](docs/plan/06-decision-log.md); lanes work in
their own worktrees and never push.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the workflow, the DCO, the AI-use policy and the confidentiality rules,
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md), [SECURITY.md](SECURITY.md) (private vulnerability reporting) and
[LICENSE.md](LICENSE.md) (the licence index). Roles, the RFC and ADR process and the AI-use policy are described in
the [open-source governance document](docs/design/19-open-source-governance.md). The repository is public: do not
include customer data, personal names or confidential material in issues or pull requests.
