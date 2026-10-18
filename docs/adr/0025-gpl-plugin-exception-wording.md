# ADR-0025: Wording of the GPL plugin exception

**Status:** Proposed · **Date:** 2026-10-17 · **Deciders:** project owner, with counsel

## Context

- **The open item.** The owner decided that the application is GPL-3.0-or-later "with a plugin exception, wording
  to be drafted by counsel" (ADR-0024). This ADR scopes that exception technically and proposes its shape. The
  legal text itself is counsel's.
- **Who can grant it.** GPL-3.0 §7 lets copyright holders grant additional permissions, and only them.
  - Ported QElectroTech code (GPL-2.0-or-later, confined to `ew-import-qet`) cannot carry the exception (C-8).
  - Under the DCO without a CLA, the exception cannot be added later to files that already carry external
    contributions without every contributor's consent. It must therefore be in the headers before the first
    external contribution is merged (governance gate LG-2).
  - GPL §7 also lets downstream distributors remove additional permissions from their copies.
- **The plugin model.** Plugins (ADR-0020, 1.x) are WebAssembly components loaded by wasmtime in the EW process.
  - They interact only through the WIT world `elektrawatt:plugin@1`.
  - They are built against the permissive `ew-plugin-wit` and `ew-plugin-sdk` crates.
  - They never link the UI toolkit or any GPL crate.
  - Whether loading a component in-process creates a combined work is legally uncertain without a written
    permission.
- **Other interfaces** ([17](../design/17-extensibility.md) §12):

  | Interface | Copyright reach (engineering view, not legal advice) | Exception needed |
  |---|---|---|
  | File format, JSON Schemas, permissive crates | none | no |
  | `ew` CLI as a process; `ew mcp` over stdio or loopback | normally separate works | no; state it for clarity |
  | WASM component through `elektrawatt:plugin@1` | uncertain | **yes** |
  | rhai scripts, SQL, rule packs, machine profiles, forms (inputs EW interprets) | generally not derivative | a clarifying sentence |
  | Python programs importing the `elektrawatt` wheel (GPL extension module) | plausibly a combined work | not covered by a WIT-only text |
  | Rust code linking `ew-api`, `ew-model` or other GPL crates | covered work | no exception; must be GPL-compatible |

- **Precedents.** The GCC Runtime Library Exception, the Classpath exception, the SPDX-listed
  `GPL-3.0-linking-exception`, and KiCad's library exception for content.
- **Identifier syntax.** Strict SPDX 2.3 allows only listed exception IDs after `WITH`. SPDX 3.0 adds `AdditionRef-`
  for custom additions (**verify**).

## Decision

**Pending.** Until this ADR is accepted:
- GPL-layer files carry `GPL-3.0-or-later` without an exception;
- pull requests from non-maintainers are labelled `blocked:LG-2` and are not merged;
- no plugin host ships. Plugins are a 1.x tier, so no user is affected yet.

## Recommendation

1. **Grant a GPL-3.0 §7 additional permission**, identified as `ElektraWatt-plugin-exception`, built on two defined
   terms:
   - **Plugin Interface:** the WIT package `elektrawatt:plugin`, at any version the project publishes;
   - **Independent Plugin:** a WebAssembly component that interacts with ElektraWatt only through the Plugin
     Interface and contains no ElektraWatt code other than generated bindings and the permissively licensed SDK.

   The permission allows conveying ElektraWatt combined with Independent Plugins, with the plugins under terms of
   their authors' choice, whatever their licence.
2. **Exclusions:**
   - modifications of ElektraWatt itself;
   - every file that does not carry the exception, in particular all of `ew-import-qet`;
   - code that links GPL crates directly.
3. **Clarifying statements** in the same text, so that companies can keep proprietary rule packs, reports and
   profiles:
   - **Separate programs.** Programs that interact through the CLI, MCP or the file format are separate works.
   - **Interpreted inputs and outputs.** Interpreted inputs (projects, rhai scripts, SQL, rule packs, machine
     profiles, forms) and outputs (projects, PDFs, reports, `.ewpp`, machine files; NFR-LIC-09) are not covered works
     merely because EW reads, runs or writes them.
4. **Python.** Ask counsel to extend the permission to programs that use only the documented Python API of the
   `elektrawatt` wheel, for parity with WASM plugins. If counsel advises against, document that distributed Python
   tools must be GPL-compatible. Private, undistributed use is unaffected either way.
5. **Placement.**
   - The canonical text is `LICENSES/LicenseRef-ElektraWatt-plugin-exception.txt` (REUSE layout), with a pointer from
     `LICENSE.md`.
   - It exists only once. This replaces the separate `COPYING.EXCEPTION` file of the initial ADR summary,
     following [19](../design/19-open-source-governance.md) OQ1.
   - Headers read `GPL-3.0-or-later WITH LicenseRef-ElektraWatt-plugin-exception`, or the `AdditionRef-` form. During
     M0, test both forms with reuse-tool 6.x and cargo-deny, and adopt the one both accept.
6. **Timing.** Adopt the exception together with the design-exception text for symbols and forms (ADR-0024), in one
   brief to counsel and one adoption event. This must happen before the first non-maintainer PR is merged (LG-2).
   The exception is applied in the same commit that adds the headers to every GPL-layer file.

## Consequences

If the recommendation is accepted:

**Positive**
- **Commercial plugins are legally clear.** Commercial and company-internal closed plugins, such as a vendor's
  machine exporter or a configurator check, can be distributed. This makes a plugin ecosystem plausible.
- **Proprietary content stays proprietary.** Companies can keep proprietary rule packs and report definitions
  without copyleft review.
- **The legal and technical boundaries coincide.** The boundary the permission draws is the WIT world, which the
  architecture already enforces: plugins cannot link GPL code, and `ew-plugin-host` cannot reach `ew-import-qet`.

**Negative**
- **Per-file statement.** The exception must be stated per file and maintained by REUSE tooling. A file added
  without it is outside the permission.
- **Ecosystem.** Some strict-copyleft contributors may object to enabling proprietary plugins.
- **Counsel and SPDX.** The definitions need counsel's review for EU law. The SPDX identifier form may change once
  tools support `AdditionRef-`.
- **Forks.** A fork may drop the permission. Plugins written against a fork without it would need GPL-compatible
  licences.

## Alternatives considered

| Alternative | Assessment |
|---|---|
| **No exception** (plugins must be GPL-compatible) | Legally simplest, but it rules out commercial vendor plugins and company-internal closed plugins, which the owner's decision intends to allow |
| **LGPL for the plugin SDK only** | Does not address the runtime combination with the GPL host. The SDK is already MIT OR Apache-2.0 |
| **Rely on "WASM is not linking"** without a written permission | Legally uncertain. Companies' legal departments would not accept it |
| **The SPDX-listed `GPL-3.0-linking-exception`** or a Classpath-style exception | Too broad: it would permit linking any code, including proprietary Rust products embedding `ew-api` statically, which defeats the purpose of choosing the GPL for the engine |
| **The GCC Runtime Library Exception** | Built for compiler runtimes and "eligible compilation processes". It does not fit a plugin host |
| **Adding the exception later, after contributions** | Impossible without every holder's consent (DCO, no CLA) |

**Decision needed from:** the project owner, with counsel. It is due before the first pull request from a
non-maintainer is merged (gate LG-2), and in any case before the plugin host is built in 1.x.

## Related

- **ADRs:** [ADR-0020](0020-extensibility-tiers-and-security-model.md), [ADR-0021](0021-importers-emit-format-level-records.md),
  [ADR-0024](0024-licence-set-and-permissive-gpl-boundary.md).
- **Design:** [17 extensibility](../design/17-extensibility.md) §8, §12, OQ4, OQ5;
  [19 open-source governance](../design/19-open-source-governance.md) §2.1, §3.2, §3.3, OQ1, OQ8;
  [04 architecture](../design/04-architecture.md) §3; [oss-landscape](../research/oss-landscape.md) §7.3;
  [gaps-and-corrections](../research/gaps-and-corrections.md) C-8.
- **Requirements:** NFR-LIC-02, NFR-LIC-05, NFR-LIC-09; FR-EXT-019, FR-EXT-020, FR-EXT-023.
