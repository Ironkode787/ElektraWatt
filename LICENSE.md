<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC0-1.0
-->

# Licences

ElektraWatt is not under a single licence: each path of the repository has exactly one SPDX licence expression,
chosen by what the path contains. The authoritative map is
[docs/design/19-open-source-governance.md §2.1](docs/design/19-open-source-governance.md#21-licence-layers)
([ADR-0024](docs/adr/0024-licence-set-and-permissive-gpl-boundary.md)); it is held in machine form in
[`ci/licence-map.toml`](ci/licence-map.toml) and checked on every pull request, together with the SPDX header of
every file (`reuse lint`). The licence texts are in [`LICENSES/`](LICENSES). Where a path's row and a file's own
header differ, CI fails; the file header is what applies to that file.

| Layer | Content | Paths | SPDX expression |
|---|---|---|---|
| L1 | Application and GPL-layer crates, the binaries | `crates/ew-{model,rhai,derive,commands,checks,sqlview,library,store,text,reports,scene,render2d,render-cpu,export,print,plc,panel,render3d,mfg,import-eplan,import-dxf,import-catalog,import-tables,script,plugin-host,jobs,api,viewmodel,ui,testkit}/`, `bin/` | `GPL-3.0-or-later` (with the plugin exception once adopted, see below) |
| L2 | QElectroTech importer, files written by ElektraWatt | `crates/ew-import-qet/` | `GPL-3.0-or-later`, with no exception |
| L3 | QElectroTech importer, ported QElectroTech code (none yet) | `crates/ew-import-qet/src/qet_port/` | `GPL-2.0-or-later`, with the original notices kept |
| L4 | The permissive format, parser and SDK crates | `crates/ew-{units,ids,mltext,rds,schema,format,interop-api,dl,step-proto,memo,plcaddr,caex,devdesc,ewpp}/` | `MIT OR Apache-2.0` |
| L5 | JSON Schemas (from their first release) | `schemas/` | `MIT OR Apache-2.0` |
| L6 | Built-in rule packs, report packs and machine profiles: their data files (from their first release) | `packs/` | `MIT OR Apache-2.0` |
| L7 | Symbols, function definitions, frames, forms, templates, macros (from their first release) | `content/`; the symbol and macro repositories | `CC-BY-SA-4.0` with a design exception (see below) |
| L8 | Project-authored part facts, examples, tutorials, the golden fixture, the own test corpus (from their first release) | `examples/`, `golden/`, `fuzz/corpus/own/`; the parts repository | `CC0-1.0` |
| L9 | Documentation and the root prose files | `docs/`, `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CLAUDE.md`, `ci/README.md` | `CC-BY-4.0` |
| L9 | The Code of Conduct (Contributor Covenant 3.0, third-party text) | `CODE_OF_CONDUCT.md` | `CC-BY-SA-4.0`, the Covenant's own licence |
| L11 | Repository metadata and configuration | `LICENSE.md`, `REUSE.toml`, `Cargo.toml`, `Cargo.lock`, `deny.toml`, `about.toml`, `rust-toolchain.toml`, `rustfmt.toml`, `.gitignore`, `.gitattributes`, `.cargo/`, `.config/`, `.github/` (except `workflows/`), `supply-chain/`, `.claude/` | `CC0-1.0` |
| L12 | CI workflows, scripts and tools | `.github/workflows/`, `ci/`, `xtask/`, `docs/plan/tools/` | `MIT OR Apache-2.0` |

Rows L5 to L8 have no files yet; they are listed so that contributions land in the right row. Bundled third-party
assets (fonts, icons, data) keep their own licences on a separate allowlist and are listed in `NOTICE` when the
first one is added (19 §2.1 L13 to L15).

**Pending exceptions.** The plugin exception for the application layer and the design exception for symbols,
forms and templates are not adopted yet ([ADR-0025](docs/adr/0025-gpl-plugin-exception-wording.md), legal gate
LG-2). Until they are, GPL-layer files carry plain `GPL-3.0-or-later`, and pull requests from people outside the
maintainer team are not merged. When adopted, the texts are added to `LICENSES/` and applied to the headers of the
affected paths in one change.

**Contributions** are licensed under the licence of the path they change (inbound equals outbound), certified by
a Developer Certificate of Origin sign-off on every commit. There is no contributor licence agreement and no
copyright assignment. Every file carries the collective notice "The ElektraWatt contributors"; the git history is
the authorship record. See [CONTRIBUTING.md](CONTRIBUTING.md).

**Your work is yours.** Projects, PDFs, reports, `.ewpp` packages, machine files and as-built archives made with
ElektraWatt belong to you, including the symbol and form copies embedded in them; they are not subject to
ElektraWatt's copyleft.

**No substitute for verification.** ElektraWatt's checks do not replace engineering verification by a competent
person.
