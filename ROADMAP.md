<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Roadmap

This page lists the milestones of ElektraWatt by name and content, in order. It carries no dates. The milestone
that is running and the open work are in [docs/plan/](docs/plan/README.md).

## Milestones

| Milestone | Name | What it delivers |
|---|---|---|
| M0 | Kick-off and foundations | The workspace and CI on every supported platform, the licence files and the licence-boundary check, the pilot kick-off |
| M1 | Spine exit: headless golden PDF | `ew export pdf` writes the whole reference drawing set from the model, headless; the acceptance tests pass |
| M-PP | Pilot preview 0.0.x | Interactive 2D editing of new projects with the starter symbols, core reports and PDF; unsigned builds for the pilot only |
| M2 | MVP: pilot release 0.1.0 | The first release, for Windows and Linux: a per-user Windows setup, a Linux AppImage and the command-line archives, with the golden sample and the documentation inside. Interactive 2D editing of the reference set, QET imports and the pre-delivery check. It is unsigned and has no update check and no crash upload |
| M2.5 | Maintenance | No features: the code base and the documents cleaned up, the tests reviewed, and the over-design removed (the symbol editor moved into 0.1.0) |
| M2.5-Fix | A properly working program: release 0.2.0 | Every open fix, and every fix the maintenance turns up, until the program works as a whole in hands-on use |
| M3 | Automation breadth | New development only: PLC, bus and fluid models; exchange with TIA Portal and PLCopen tools; EDZ, DXF and DWG import; automation workspaces |
| M4 | Manufacturing and 2D/3D panel | The STEP worker, the 3D view, collision, routing and wire lengths; wire records, ferrules, labels and machine profiles; the `.ewpp` package |
| M5 | 1.0 feature complete (beta) | Every 1.0 feature merged; beta releases for the pilot and early adopters |
| M6 | 1.0 release | The format freeze and external acceptance. What the first release leaves for 1.0 comes in here: signed releases, the per-machine installer and the `.deb` package, the update check and the crash report with minidump, and the CRA processes |
| 1.x | Backlog | Work that follows 1.0 and is not yet split into packages |

## Where it stands

| Milestone | State |
|---|---|
| M0 to M2 | Closed: 0.1.0 was released on 2026-11-08 for Windows and Linux, unsigned and with no network code |
| M2.5 | Closed on 2026-11-11: the maintenance step ([plan and exit record](docs/plan/m2.5.md)) |
| M2.5-Fix | Closed: 0.2.0 was released on 2026-11-13, the fixes that make the program work well in the hand ([plan and log](docs/plan/m2.5-fix.md)) |
| M3 | Running since 2026-11-13: automation breadth, release 0.3.0 ([plan](docs/plan/m3.md)) |
| M4 to M6, 1.x | Not started |

What works today, in short: the desktop window opens, edits and checks a project, with navigators, table mode, a
history, the 2D panel layout and printing; the `ew` command-line tool imports QElectroTech projects, checks, exports
PDF/A-2b, SVG and tables (with the Excel round trip), prints and upgrades a project of an older format. 0.2.0 is an
early alpha that fixes what hands-on use of 0.1.0 found, with the right-click menu and forms for cables and terminals. The
[README](README.md#status) lists what is built.

## What is not on the roadmap

The non-goals of the project are in the [vision](docs/design/00-vision-and-scope.md#4-non-goals).

## Keeping this page right

This page changes with the plan, by a commit like any other file.
