# ElektraWatt

ElektraWatt (EW) is an open-source, offline-first desktop application for electrical control-cabinet design, written
in Rust. It aims at the functionality of EPLAN Electric P8 class tools: schematics, devices and parts, terminals and
cables, PLC and fluid power, panel layout, reports, manufacturing outputs and checks, in a plain-text project format
that works with Git.

## Status

Development runs in short AI-assisted cycles: one lead session plans each cycle, delegates the work to lanes, has
every lane reviewed independently and records the decisions. A cycle ends as a checkpoint, which pushes the working
branch, or as a delivery, which runs the full continuous-integration gate on Windows and Linux and then merges into
`main` by fast-forward; a delivery comes every second cycle. Forty-six cycles have been delivered to `main`, and the
forty-seventh lands as a checkpoint. Where that stands against the milestones of the
[global plan](docs/plan/01-global-plan.md):

| Milestone | State |
|---|---|
| M0 Foundations | Technical part delivered (workspace, CI on Linux and Windows, licensing and the licence-boundary check); the remaining gate items are the project owner's |
| M1 Spine exit | Delivered with follow-up tasks: the model, commands, derived data, checks, reports and the PDF of the reference project are produced headlessly; the gate record is in [docs/plan/gate-records](docs/plan/gate-records) |
| M-PP Pilot preview | Closed: gate G-PP was recorded as Go (decision log ID-777). The preview edits new projects in 2D with the starter symbol set |
| M2 MVP 0.1.0 | In progress, toward the pilot release 0.1.0: a first release on Windows and Linux, unsigned and with no network code (ID-1187). 26 work packages are in scope (the lean scope); signing, the update check and the steward's compliance processes are 1.0's |
| M3 to M6, 1.x | Not started |

What works today:

- **The window**, `elektrawatt`: opens a project with the page list, a zoomable canvas, navigators, table mode and a
  status bar; creates new projects through the first-run and company-standard wizards; docks panels into named
  workspaces; and edits: places devices and symbols, draws and routes wires with interruption points, edits
  properties, finds and replaces, copies and pastes, and undoes with a history. Every change is a recorded command.
- **The 2D panel layout**: enclosures, rails and mounting plates drawn as a panel page, with placing tools and the
  checks that go with them.
- **The `ew` command-line tool**: import of QElectroTech projects, checks with the pre-delivery scheme, a read-only SQL
  projection, cable list, bill-of-materials and table exports, PDF/A-2b and SVG export of every page, a print set,
  applying recorded command files, revisions and verification of the derived data. Run `cargo run -p ew -- help` for
  the full usage.
- **Outputs**: reports (parts list, terminal and cable lists, cover tables and back-links) drawn on report pages and
  written as PDF/A-2b and SVG.
- **The reference project** `golden/J-E-2416.ew` (36 pages, anonymised) with its expected outputs, and the acceptance
  runner `cargo xtask acceptance` that checks the acceptance criteria of the design against them.
- **Headless operation**: the window runs the same actions from a UI script without a display
  (`elektrawatt --headless --script <file>`), which is how the UI is tested.
- **Quality gates**: about 6,300 tests across about 50 crates and three binaries, with determinism lints (no wall
  clock, no environment reads and no hash-ordered collections in the model, derivation, format and export paths),
  property tests, a fuzzer and a golden-file oracle.

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

The main workflow runs on every pull request and on every push to `main`, and can be started by hand on any branch,
which is how a delivery is checked. It builds and tests on Linux and Windows, runs the UI headlessly in a container,
validates the exported PDF with veraPDF, fuzzes the changed targets for a minute and checks the supply chain
(`cargo deny`, `cargo vet`). Nightly runs fuzz for longer, run the memo oracle, property tests and sanitizers, build an
unsigned development CLI and report performance without gating it; coverage runs weekly. The jobs are described in
[ci/README.md](ci/README.md). The performance criteria are deliberately not measured during development (owner's
rule, decision log ID-512); they are addressed in the release phase.

## Key decisions

| Topic | Decision |
|---|---|
| Language | Rust |
| Platforms | Desktop for Windows and Linux (macOS is not supported), offline-first ([ADR-0022](docs/adr/0022-platform-matrix-and-performance-budgets.md)) |
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
checked on every build. All 32 architecture decision records are listed in the [ADR index](docs/adr/README.md); the
[decision log](docs/plan/06-decision-log.md) records every ruling taken during the cycles.

## Where to start

- [Documentation map](docs/README.md): reading orders for decision makers, developers and domain experts.
- [ADR index](docs/adr/README.md), [plan guide](docs/plan/README.md) and the
  [work breakdown](docs/plan/02-work-packages.md) with the follow-up tasks of every delivered package.
- The [reference project](docs/design/20-golden-project.md) and the [quality, security and release](docs/design/18-quality-security-release.md)
  document, which define what the tests and the CI prove.
- [CLAUDE.md](CLAUDE.md), [.claude/orchestration.md](.claude/orchestration.md) and [.claude/agents](.claude/agents):
  the setup for AI-assisted work (an orchestrating session with builder, reviewer, checker and lookup agents) and the
  testing policy.

## Branches

Work happens on a working branch. At a delivery, `main` is fast-forwarded to it after the full gate on Windows and
Linux, and both are pushed; at a checkpoint only the working branch is pushed. The branch of the running cycle is
named in the [decision log](docs/plan/06-decision-log.md); lanes work in their own worktrees and never push.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the workflow, the DCO, the AI-use policy and the confidentiality rules,
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md), [SECURITY.md](SECURITY.md) (private vulnerability reporting),
[PRIVACY.md](PRIVACY.md) (every data flow; there is no telemetry), [GOVERNANCE.md](GOVERNANCE.md) (who decides, and how a
contribution is reviewed), [ROADMAP.md](ROADMAP.md) (the milestones) and [LICENSE.md](LICENSE.md) (the licence index).
Roles, the RFC and ADR process and the AI-use policy are described in
the [open-source governance document](docs/design/19-open-source-governance.md). The repository is public: do not
include customer data, personal names or confidential material in issues or pull requests.
