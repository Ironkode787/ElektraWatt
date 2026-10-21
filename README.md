# ElektraWatt

ElektraWatt (EW) is an open-source, offline-first desktop application for electrical control-cabinet design, written
in Rust. It aims at the functionality of EPLAN Electric P8 class tools: schematics, devices and parts, terminals and
cables, PLC and fluid power, panel layout, reports, manufacturing outputs and checks, in a plain-text project format
that works with Git.

## Status

The design phase is complete as **draft v0.1**. The repository holds the design documents,
30 architecture decision records (25 Accepted, 5 Proposed), a work breakdown and a global plan. The first milestone
(M0) starts after the pre-M0 decisions listed in the [decision log](docs/plan/06-decision-log.md).

The cargo workspace with every crate skeleton of [architecture §3](docs/design/04-architecture.md#3-crate-and-workspace-layout),
the policy checks in `xtask` and the per-PR CI ([ci/README.md](ci/README.md)) now exist. There is **no functionality
yet**: the crates are empty.

## Building

The pinned toolchain in `rust-toolchain.toml` is installed automatically by rustup. Tests run with
[cargo-nextest](https://nexte.st).

```sh
cargo build --workspace --locked
cargo nextest run --workspace
cargo xtask ci      # fmt, clippy, nextest and the repository policy checks, as in CI
```

## Key decisions

| Topic | Decision |
|---|---|
| Language | Rust |
| Platforms | Cross-platform desktop (Windows, macOS, Linux), offline-first ([ADR-0022](docs/adr/0022-platform-matrix-and-performance-budgets.md)) |
| Team | 2 to 4 developers with AI assistance |
| v1 scope | Electric P8-class schematics, PLC and I/O, fluid power, 3D panel layout, manufacturing outputs, QET/EDZ/DXF import ([vision](docs/design/00-vision-and-scope.md)) |
| Application licence | GPL-3.0-or-later |
| Format crates | MIT OR Apache-2.0 |
| Symbols and templates | CC-BY-SA-4.0, with a design exception |
| Part data and examples | CC0 |
| Documentation | CC-BY-4.0 |

The licence set is planned and pending the legal gates ([ADR-0024](docs/adr/0024-licence-set-and-permissive-gpl-boundary.md),
[governance](docs/design/19-open-source-governance.md)). The licence texts are in `LICENSES/`, indexed by
[LICENSE.md](LICENSE.md); the plugin and design exceptions are pending (ADR-0025).

## Headline plan

Month 0 is November 2026. Dates come from [milestones.csv](docs/plan/milestones.csv), lean scope, 3 developers.

| | M1 spine exit | M-PP pilot preview | M2 MVP | M6 release 1.0 |
|---|---|---|---|---|
| Plan of record (external) | Feb 2028 | Dec 2028 | Mar 2030 | Mar 2032 |
| AI-assisted internal target | Nov 2027 | Jul 2028 | Jul 2029 | Feb 2031 |

The AI-assisted dates rest on weak evidence and are an internal target only. Details: [plan guide](docs/plan/README.md),
[global plan](docs/plan/01-global-plan.md).

## Where to start

- [Documentation map](docs/README.md): reading orders for decision makers, developers and domain experts.
- [ADR index](docs/adr/README.md) and [plan guide](docs/plan/README.md).
- [CLAUDE.md](CLAUDE.md) and [.claude/agents](.claude/agents): the model-routing setup for AI-assisted work (Opus for
  lead decisions, Sonnet for specified work, Haiku for mechanical tasks).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the workflow, the DCO, the AI-use policy and the confidentiality rules,
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md), [SECURITY.md](SECURITY.md) (private vulnerability reporting) and
[LICENSE.md](LICENSE.md) (the licence index). Roles, the RFC and ADR process and the AI-use policy are described in
the [open-source governance document](docs/design/19-open-source-governance.md). Comments on the documents and the
Proposed ADRs are welcome. The repository is public: do not include customer data, personal names or confidential
material in issues or pull requests.
