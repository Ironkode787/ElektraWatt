# ElektraWatt

An open-source, cross-platform desktop application for designing and documenting electrical control cabinets —
schematics, PLC/I-O, fluid power, 2D/3D panel layout and manufacturing outputs — with EPLAN-class engineering
depth. Written in Rust.

> **Status: design phase.** There is no code yet. This repository currently holds the design documentation and the
> research behind it.

## Key decisions so far

| Topic | Decision |
|---|---|
| Language | Rust |
| Platform | Cross-platform desktop (Windows, macOS, Linux), offline-first, projects as files on disk |
| Team | 2–4 developers |
| v1 scope | Schematics (EPLAN Electric P8 class), PLC / I-O integration, manufacturing outputs, 3D panel layout, import from QElectroTech, EPLAN (EDZ parts) and DXF |
| Licence (planned) | Application GPL-3.0-or-later; reusable format/SDK crates MIT OR Apache-2.0 |
| Architecture | Typed Rust model in persistent maps; everything printed is a derived query; egui shell with own wgpu renderers; JSON Lines project directory — see [architecture](docs/design/04-architecture.md) |

## Documentation

- [Architecture specification](docs/design/04-architecture.md)
- [Research notes](docs/research/) — EPLAN feature inventory, standards, Rust ecosystem, interop formats,
  open-source landscape, reference-drawing analysis, panel & manufacturing, PLC/bus/fluid, gaps & corrections
- [Architecture proposals](docs/research/architecture-proposals/) — the four competing proposals evaluated by the
  judge panel

The remaining design documents (vision, requirements, domain model, feature designs, ADRs) and the global plan
(milestones, work packages, gates) are being written and will land in `docs/`.
