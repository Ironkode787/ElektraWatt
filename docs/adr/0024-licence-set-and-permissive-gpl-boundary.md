# ADR-0024: Licence set and the permissive/GPL crate boundary

**Status:** Accepted · amended 2026-10-18 · **Date:** 2026-10-17 · **Deciders:** project owner (licence set, decided 2026-10-17); core
team (crate boundary)

## Context

- **The owner's licence decision (2026-10-17, binding):**
  - the application is GPL-3.0-or-later with a plugin exception, whose wording counsel drafts (ADR-0025);
  - reusable format, parser and SDK crates are MIT OR Apache-2.0;
  - symbols and templates are CC-BY-SA-4.0 with a KiCad-style design exception;
  - project-authored part data and examples are CC0;
  - documentation is CC-BY-4.0;
  - contributions come in under the DCO, with no CLA.

  This ADR records that decision and adds the architectural boundary that makes it workable.
- **Why the boundary must be fixed on day one.** Under the DCO without a CLA, the project holds no rights beyond the
  licences. Moving code from a GPL crate into a permissive one later needs every copyright holder's consent. For the
  same reason, the plugin exception cannot be added later to files that already carry external contributions.
- **What the judges found.** In proposals A and C, crates labelled permissive depended on GPL crates, so the
  "permissive" format layer was not actually usable without GPL code.
- **QET code.** QElectroTech is GPL-2.0-or-later. Its code can be ported into GPL-3.0-or-later works, but the plugin
  exception cannot be granted for it, because only the copyright holders can grant additional permissions (C-8).
- **Dependency licences:**
  - imbl is MPL-2.0 and notify is CC0;
  - wasmtime is Apache-2.0 WITH LLVM-exception;
  - acadrust is MPL-2.0;
  - OCCT is LGPL-2.1, linked only into `ew-step-worker`;
  - Noto Sans is OFL-1.1.
- **Code of conduct.** Contributor Covenant 3.0 is the current version (C-13).
- **Employer ownership.** Dutch law (Auteurswet art. 7) makes an employer the author of works its employees create in
  their job (verify with counsel), so employer statements matter.

## Decision

1. **Licence map.** Each repository path has exactly one SPDX expression, held in machine form in
   `ci/licence-map.toml` and in `REUSE.toml`:

   | Content | SPDX expression |
   |---|---|
   | Application and GPL-layer crates, binaries `elektrawatt`, `ew`, `ew-step-worker` | `GPL-3.0-or-later WITH <plugin exception>` (ADR-0025). Until it is adopted: `GPL-3.0-or-later` |
   | `ew-import-qet`: files written by EW | `GPL-3.0-or-later`, with no exception |
   | `ew-import-qet/src/qet_port/`: ported QET code | `GPL-2.0-or-later`, with the original QET notices kept verbatim, listed in `NOTICE` |
   | Permissive layer crates; JSON Schemas; CI scripts | `MIT OR Apache-2.0` |
   | Built-in rule packs, report packs and machine profiles: their TOML, rhai, SQL and Fluent data files | `MIT OR Apache-2.0` (they are code), so company packs derived from them need no GPL compliance. The Rust rule implementations inside `ew-checks` stay in the GPL layer: they are `GPL-3.0-or-later`, like the rest of the application layer |
   | Symbols, function definitions, frames, forms, page and report templates, macros | `CC-BY-SA-4.0 WITH <design exception>` |
   | Project-authored part facts, examples, tutorials, the golden J-E-2416 fixture, the own test corpus | `CC0-1.0` |
   | Documentation: design docs, ADRs, mdBook, rule pages | `CC-BY-4.0` |
   | Bundled third-party assets (fonts, icons) | Their own licences, on a separate allowlist: OFL-1.1 for Noto Sans; osifont's GPL-with-font-exception only if ADR-0028 bundles it |

   The design exception is the SPDX-listed `KiCad-libraries-exception` used verbatim, if counsel confirms that it
   covers control-cabinet projects, machine files and as-built archives. Otherwise it is a project-specific text.
   The full path-to-licence map is [19](../design/19-open-source-governance.md) §2.1 (amendment 2).
2. **A closed permissive layer from the first commit.**
   - **Crates:** `ew-units`, `ew-ids`, `ew-mltext`, `ew-rds`, `ew-schema` (every persisted record type), `ew-format`,
     `ew-interop-api`, `ew-dl`, `ew-step-proto`, `ew-memo`, plus the planned `ew-plugin-wit` and `ew-plugin-sdk`
     and, by amendment 1, `ew-plcaddr`, `ew-caex`, `ew-devdesc` and `ew-ewpp`.
   - **Allowed licences** in their dependency closure (normal, build and dev): MIT, Apache-2.0, BSD, ISC, Zlib,
     Unicode-3.0, MPL-2.0 and CC0-1.0. No GPL, LGPL or AGPL crate is allowed (NFR-MAINT-02). Amendment 3 adds
     `Apache-2.0 WITH LLVM-exception`.
   - **Planned permissive code is written there from day one.** Code destined for a permissive crate, such as the
     CAEX model and the device-description parsers (`ew-caex`, `ew-devdesc`), starts in that crate.
3. **The GPL layer adds behaviour, never a second data definition.** `ew-model` wraps `ew-schema` record structs in
   tables and adds ops, indexes, invariants and derivation (ARCH §3 rule 5). Third parties can therefore read, write,
   validate and import EW projects with the permissive crates alone.
4. **QET-derived code is confined.**
   - Ported QET code lives only in `ew-import-qet`, and only `ew-api` depends on that crate.
   - `ew-plugin-host` and the permissive crates never reach it (NFR-LIC-05).
   - The crate is excluded from the plugin exception.
5. **Outputs are free of EW's copyleft.** Projects, PDFs, reports, `.ewpp` packages, machine files and as-built
   archives made with EW belong to the user. That includes the symbol and form copies embedded in them, through the
   design exception (NFR-LIC-09). `LICENSE.md`, the About dialog and the documentation say so in the same words.
6. **Mechanics.**
   - REUSE/SPDX headers on every file, with every licence text in `LICENSES/`.
   - The collective copyright notice "The ElektraWatt contributors" (NFR-LIC-10). Files with third-party
     material keep their original notices.
   - DCO 1.1 `Signed-off-by` on every commit, checked per PR. An AI tool is never the signer.
   - Contributor Covenant 3.0.
   - A per-PR licence-map check and licence-boundary check (ADR-0023).
7. **The golden fixture** is published as CC0 only with every public-repository redaction rule applied, and after
   the pilot panel builder's written consent to publish the anonymised technical content (governance gate LG-5).

## Amendments

Accepted on 2026-10-18. The decision text above is unchanged; these entries refine it.

1. **Further permissive crates** (2026-10-18; [architecture](../design/04-architecture.md) §3;
   [10](../design/10-plc-and-bus.md), [13](../design/13-manufacturing-outputs.md)). The closed permissive layer
   also holds `ew-plcaddr` (PLC addresses, address profiles, the grammar engine and bit intervals), `ew-caex` (the
   CAEX 2.15/3.0 model), `ew-devdesc` (GSD, GSDML and IODD; ESI and EDS at 1.x) and `ew-ewpp` (the `.ewpp`
   record types, JSON Schemas, reader, writer and package diff). Each depends only on permissive crates, so the
   boundary is unchanged; the licence-boundary check covers them.
2. **The full path map** (2026-10-18; [19](../design/19-open-source-governance.md) §2.1). This ADR keeps the
   licence set and the layer rules. The authoritative path-to-licence map is 19 §2.1, enforced by
   `ci/licence-map.toml`; it adds, by reference, the layers for repository metadata and configuration (L11), CI
   workflows, scripts and tools (L12, `MIT OR Apache-2.0`) and logo and wordmark artwork (L13, `CC-BY-4.0`, with
   the marks governed separately by `TRADEMARKS.md`).
3. **One licence allowlist** (2026-10-18; NFR-LIC-04, NFR-MAINT-02). This ADR and ADR-0023 use the same list.
   `Apache-2.0 WITH LLVM-exception` is allowed now, also in the permissive closure (wit-bindgen, wasmtime);
   BSL-1.0 is added only if `cargo deny list` at M0 shows it; the osifont licence expression is added only if
   ADR-0028 bundles that font, with the exact SPDX expression verified with counsel.

   *As built (cycle 28):* native clipboard support reaches clipboard-win 5.4.3 and error-code 3.4.0, both
   BSL-1.0. Their reviewed licence declarations/text satisfy NFR-LIC-04’s existing graph-triggered admission;
   `deny.toml`, `about.toml` and `fuzz/deny.toml` admit BSL-1.0 consistently so generated dependency notices
   include its text. The delivery correction updates only clipboard-win from withdrawn 5.4.1 to non-yanked
   5.4.3, retaining the upstream HTML security fix. The L4 licence-boundary allowlist remains unchanged;
   no application-source changes are made.
4. **The Code of Conduct keeps its own licence** (2026-10-18; [19](../design/19-open-source-governance.md) §2.1 L9,
   §2.3; `LICENSE.md`). Decision 6's Contributor Covenant 3.0 is third-party text licensed `CC-BY-SA-4.0` by its
   steward (verified against the published text), so `CODE_OF_CONDUCT.md` keeps that licence and the Covenant's
   copyright line and is the one exception to decision 1's documentation row (`CC-BY-4.0`). `LICENSES/` holds the
   CC-BY-SA-4.0 text.
5. **Direct edges to the foundation crates** (2026-10-18, core team; [architecture](../design/04-architecture.md) §3
   rule 6). GPL crates may depend directly on the permissive foundation crates `ew-units`, `ew-ids`, `ew-mltext`,
   `ew-rds`, `ew-schema` and `ew-dl` instead of reaching their types through re-exports. The boundary is unchanged,
   because every such edge points from the GPL layer into the permissive one. Each edge is drawn in the architecture
   graph, which is the source, and listed in `ci/dep-rules.toml`, which `cargo xtask check-deps` enforces; the two
   change in the same pull request. Decision 4 is untouched: only `ew-api` depends on `ew-import-qet`.

## Consequences

**Positive**
- **An open format.** Anyone (ERP, MES, machine vendors, other ECAD tools) can read, write, validate and import EW
  projects without GPL code. The open format is the strategic asset.
- **Porting is allowed.** QET code can be ported legally, which speeds up the importer that the pilot's migration
  depends on.
- **Protection against closed forks.** Copyleft prevents a closed fork of the application. The plugin exception
  (ADR-0025) still allows proprietary plugins, such as a vendor exporter.
- **Enforced in CI.** The boundary is checked on every PR, so it cannot erode silently.

**Negative**
- **Upfront design.** The permissive layer must be designed carefully up front. A type needed by third parties that
  was first written in a GPL crate cannot simply be moved.
- **No dual licensing.** Dual licensing and open-core are closed off by construction (no CLA). This is intended.
- **Forks may drop the exception.** GPL §7 lets downstream distributors remove additional permissions from their
  copies. The project's own releases keep it.
- **REUSE overhead.** Every file needs a header or a `REUSE.toml` entry, and the mixed-licence `ew-import-qet` needs
  special care.
- **Employer statements.** Every employer whose staff contribute on company time must sign a statement licensing
  that work (governance gate LG-1). The pilot panel builder's statement is the first.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **Fully permissive application** (MIT OR Apache-2.0 everywhere) | Declined by the owner. No porting of GPL code from QET, KiCad or LibrePCB, and a vendor could ship a closed fork |
| **A CLA** | Friction and contributor distrust. It would mainly serve dual licensing, which is not intended. Declined by the owner in favour of the DCO |
| **Deciding the boundary after the first external contributions** | Every later move needs every holder's consent, which is the trap this ADR avoids |
| **MPL-2.0 or EUPL-1.2 for the application** | Neither can absorb GPL code, so no QET porting |
| **LGPL-3.0 for the application** | Awkward with Rust's static linking and monomorphisation |
| **AGPL-3.0 for the application** | Adds nothing for an offline desktop application, and many companies ban it internally. It stays reserved for optional server components (Later) |
| **CC-BY-4.0 or CC0 for symbols** | Considered in the research (C-7). The owner chose share-alike with a design exception, following KiCad's proven precedent |

## Related

- **ADRs:** [ADR-0001](0001-core-model-persistent-maps.md) (schema records), [ADR-0021](0021-importers-emit-format-level-records.md),
  [ADR-0023](0023-supply-chain-and-release-integrity.md), [ADR-0025](0025-gpl-plugin-exception-wording.md),
  [ADR-0028](0028-default-drawing-fonts.md), [ADR-0030](0030-legal-steward-cra-role-support-signing.md).
- **Design:** [04 architecture](../design/04-architecture.md) §3; [19 open-source governance](../design/19-open-source-governance.md)
  §2–§3 (§2.1 path map), §5, OQ12; [17 extensibility](../design/17-extensibility.md) §12;
  [15 checks](../design/15-checks-revisions-collaboration.md) OQ16;
  [oss-landscape](../research/oss-landscape.md) §7; [gaps-and-corrections](../research/gaps-and-corrections.md) C-7,
  C-8, C-13.
- **Requirements:** NFR-LIC-01 to NFR-LIC-14; NFR-MAINT-01, NFR-MAINT-02; NFR-DOC-04; FR-EXT-023, FR-EXT-036.
