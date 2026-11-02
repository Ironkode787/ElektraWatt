<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Roadmap

This page lists the milestones of ElektraWatt by name and content, in order. It carries no dates: the dates of the
plan are a forecast that moves with the work, and the current forecast is in [docs/plan/](docs/plan/README.md). The
full definition of each milestone, with its exit criteria and the gate that closes it, is in the
[work breakdown](docs/plan/02-work-packages.md#22-milestones) and in the [gates](docs/plan/03-gates.md). The reasons
behind the order are in the [global plan](docs/plan/01-global-plan.md).

## Milestones

| Milestone | Name | What it delivers |
|---|---|---|
| M0 | Kick-off and foundations | The workspace and CI on every supported platform, the licence files and the licence-boundary check, the pilot kick-off |
| M1 | Spine exit: headless golden PDF | `ew export pdf` writes the whole reference drawing set from the model, headless; the acceptance tests pass |
| M-PP | Pilot preview 0.0.x | Interactive 2D editing of new projects with the starter symbols, core reports and PDF; unsigned builds for the pilot only |
| M2 | MVP: pilot release 0.1.0 | The first release, for Windows and Linux: a per-user Windows setup, a Linux AppImage and the command-line archives, with the golden sample and the documentation inside. Interactive 2D editing of the reference set, QET imports and the pre-delivery check. It is unsigned and has no update check and no crash upload |
| M3 | Automation breadth | PLC, bus and fluid models; exchange with TIA Portal and PLCopen tools; EDZ, DXF and DWG import; automation workspaces |
| M4 | Manufacturing and 2D/3D panel | The STEP worker, the 3D view, collision, routing and wire lengths; wire records, ferrules, labels and machine profiles; the `.ewpp` package |
| M5 | 1.0 feature complete (beta) | Every 1.0 feature merged; beta releases for the pilot and early adopters |
| M6 | 1.0 release | The format freeze and external acceptance. What the first release leaves for 1.0 comes in here: signed releases, the per-machine installer and the `.deb` package, the update check and the crash report with minidump, and the CRA processes |
| 1.x | Backlog | Work that follows 1.0 and is not yet split into packages |

## What is not on the roadmap

The non-goals of the project are in the [vision](docs/design/00-vision-and-scope.md#4-non-goals).

## Keeping this page right

This page is reviewed with the plan, at least once a quarter, and changes by a pull request like any other file.
