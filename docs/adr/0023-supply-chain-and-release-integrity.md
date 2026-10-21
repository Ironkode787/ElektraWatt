# ADR-0023: Supply chain and release integrity

**Status:** Accepted · amended 2026-10-18, 2026-10-21 · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the judge panel)

## Context

- **CRA timeline.** The EU Cyber Resilience Act (Regulation (EU) 2024/2847) has applied its reporting obligations
  for actively exploited vulnerabilities since 2026-09-11. All other obligations apply from 2027-12-11, before EW 1.0
  is expected (plan of record: January 2032, [plan/01](../plan/01-global-plan.md) §5.1) (G11). Whether EW ends up with a steward or a manufacturer behind it (ADR-0030), the
  evidence is the same: an SBOM, vulnerability handling, signed and attested artefacts.
- **Enterprise expectations.** Enterprise users and IT departments expect signed installers. Unsigned Windows
  binaries trigger SmartScreen, and unsigned macOS apps are blocked by Gatekeeper.
- **Concrete risks found in the research:**
  - one candidate STEP binding (cadrum) downloads prebuilt binaries at build time (C-5);
  - several niche crates are below any reasonable adoption threshold (C-9);
  - egui's inspection server can expose UI control on the network if compiled in;
  - a widely used serialisation crate, bincode, was declared unmaintained (RUSTSEC-2025-0141, 2025-12-16), which
    shows that advisories arrive for dependencies we cannot control. EW uses postcard and rkyv instead (ARCH §3).
- **Signing identity.** Artifact Signing (Authenticode) issues public-trust certificates to organisations in the EU,
  but not to individual EU developers (gaps §3.3).
- **Licence boundary.** It is enforced by the same supply-chain tooling: no GPL crate may enter the closure of a
  permissive crate (ADR-0024).

## Decision

1. **Per pull request.**

   | Control | Rule |
   |---|---|
   | Lockfile and toolchain | `Cargo.lock` committed; `--locked` builds; exact toolchain in `rust-toolchain.toml`, raised at most once per quarter (NFR-MAINT-03) |
   | cargo-deny 0.20.2 | **Licences:** the NFR-LIC-04 allowlist (MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, Unicode-3.0, MPL-2.0, CC0-1.0, OFL-1.1 for fonts, LGPL-2.1 only in `ew-step-worker`), plus `Apache-2.0 WITH LLVM-exception` for wasmtime and wit-bindgen (ADR-0024) and BSL-1.0 for transitive crates such as clipboard-win under arboard on Windows (**verify** with `cargo deny list` at M0). EW's own GPL workspace crates are exempted by per-crate exceptions and checked by the licence-map job instead. **Advisories:** vulnerabilities deny; "unmaintained" needs an owner decision within 90 days. **Bans:** duplicate versions, downloader build scripts. **Sources:** crates.io only, git sources allowlisted per crate |
   | cargo-audit 0.22.2 | RUSTSEC check of the lockfile |
   | cargo-vet 0.10.2 | Every new or updated crate audited or covered by imported audits. The exemption list is frozen at the first commit and burned down |
   | Build scripts | Every crate with a `custom-build` target must be on a reviewed allowlist. A job builds the workspace inside a network namespace without connectivity, so any build-time download fails |
   | Licence boundary | The closure of every permissive crate contains no GPL, LGPL or AGPL crate (NFR-MAINT-02) |
   | Dependency rules | The ARCH §3 rules over direct dependencies, including `ew-plugin-host` ↛ `ew-import-qet` |
   | Low-adoption crates | Test-only, or vendored with a named owner (C-9) |
   | CI hardening | Third-party actions pinned by commit SHA; read-only default tokens; self-hosted runners never run fork PRs (NFR-SEC-19) |
   | Hygiene | `reuse lint`, DCO sign-off check |

2. **Per release.** Releases are built by CI from a signed tag, in a protected `release` environment that needs a
   second maintainer's approval and uses short-lived OIDC credentials. Each release gets:
   - a **CycloneDX SBOM** (cargo-cyclonedx 0.5.9) per artefact, including non-Rust components: SQLite, OCCT and the
     bundled fonts;
   - `cargo-auditable` metadata embedded in every binary. No release ships with a known unpatched high or critical
     advisory (NFR-SEC-11);
   - third-party notices generated with cargo-about, covering crates, SQLite, OCCT and fonts, shipped in every
     artefact and shown in the About dialog, plus the LGPL source offer for OCCT (NFR-LIC-08);
   - **SLSA build provenance** through `actions/attest-build-provenance`, signed via Sigstore;
   - **Windows:** Authenticode through Artifact Signing under the steward's organisation identity, with RFC 3161
     timestamps on every `.exe`, `.dll`, `.msi` and setup file;
   - **macOS:** signing with hardened runtime and notarisation through rcodesign (apple-codesign 0.29.0), with
     Apple's `codesign` and `notarytool` as a fallback;
   - **All artefacts:** `SHA256SUMS`, signed with the project's ed25519 release key (minisign-compatible). Users
     without Sigstore access can verify offline.
3. **Assertions.**
   - Release binaries contain no egui inspection or egui_mcp code path, and open no listening socket unless the user
     starts `ew mcp` (NFR-SEC-09).
   - A network-sandbox job fails on any unexpected connection or DNS lookup (ADR-0029).
4. **Disclosure.** `SECURITY.md` provides:
   - private reporting through GitHub security advisories;
   - a supported-versions table (ADR-0030);
   - coordinated disclosure, with acknowledgement within 7 days.

   A threat model is kept in the repository and reviewed every minor release.
5. **Updates** are installed only on a user action, after signature verification. There is no silent auto-update
   (NFR-SEC-15).

## Amendments

Accepted on 2026-10-18. The decision text above is unchanged; these entries refine it.

1. **Builds before the signing identity exists** (2026-10-18; [18](../design/18-quality-security-release.md),
   [19](../design/19-open-source-governance.md) LG-4). Until LG-4 is met, the project publishes only unsigned
   nightlies carrying the banner **"unsigned development build"**. It creates no version tags (`vX.Y.Z`) and no
   signed or tagged release.
2. **The cargo-deny licence allowlist** (2026-10-18; NFR-LIC-04). This ADR and ADR-0024 use one list:
   `Apache-2.0 WITH LLVM-exception` is on it now; BSL-1.0 is added only if `cargo deny list` at M0 shows it; the
   osifont licence expression is added only if ADR-0028 bundles that font, with the exact SPDX expression verified
   with counsel. GPL-2.0-only, AGPL and non-OSI licences are denied.
3. **cargo-vet attribution; no imported audits** (2026-10-18, project owner (delegated, 2026-10-18);
   [19](../design/19-open-source-governance.md) §3.5, OQ18). The tool is cargo-vet 0.10.2. The `who` field of an
   audit names the contributor's GitHub handle, never a personal name or e-mail address. Decision 1's "covered by
   imported audits" is not used while the public-repository rule stands, because `imports.lock` would record other
   organisations' auditor names and e-mail addresses: every crate needs the project's own audit or a frozen
   exemption. The cost is more review time per dependency bump. Revisit when a steward exists.
4. **How exemptions are added** (2026-10-21, project owner (delegated); decision log ID-332;
   [18](../design/18-quality-security-release.md) §11). Decision 1's "frozen at the first commit" is not how the
   list works, and is replaced: the work package that adds or updates a crate adds one `[[exemptions.<crate>]]`
   entry per crate and exact version to `supply-chain/config.toml`, with the criteria its use needs
   (`safe-to-deploy` for code that ships, `safe-to-run` for build and test only), in the same `build(deps)` commit,
   whose message lists the exempted crates; the review of that commit checks the list, and `cargo vet` in CI fails
   for any crate with neither an audit nor an exemption. The list is burned down by the project's own audits in
   `supply-chain/audits.toml` (amendment 3), never by imported audit sets; an exemption is never widened to a new
   version without a new reviewed entry. Every exemption is audited or replaced before the first signed release,
   and until then a work package adds exemptions only for the crates it introduces (project owner (delegated),
   2026-10-21). Example: the cycle-13 rusqlite commit exempted cc, fallible-iterator,
   fallible-streaming-iterator, find-msvc-tools, libsqlite3-sys, pkg-config, rusqlite, shlex and vcpkg. Trade-off:
   no audit debt blocks a work package, but the debt is visible and grows with each dependency until audits catch up.

## Consequences

**Positive**
- Steward-grade evidence (and manufacturer-grade evidence, should a commercial edition ever exist) exists from the
  first signed release. Nothing has to be retrofitted before 2027-12-11.
- Dependency compromise, build-time downloads and licence-boundary mistakes are caught before merge, not at release.
- Users can verify any artefact offline: signed sums and an embedded release key.
- **Reproducibility.** Unsigned Linux binaries rebuilt independently from the tag should be bit-identical
  (NFR-SEC-20, Should, 1.0; [18](../design/18-quality-security-release.md) §16).

**Negative**
- **No signed release without an organisation.** Signing needs an organisation that holds the identity (ADR-0030).
  Until it exists, only unsigned nightlies labelled "unsigned development build" are published (amendment 1).
- **cargo-vet audits** cost reviewer time on every dependency bump, including the quarterly UI-stack upgrade.
- **Long OCCT build.** OCCT's C++ build has to be cached in a separate job (ADR-0016), and the build-script allowlist
  needs maintenance.
- **Key custody.** Custody of the release key is a standing responsibility: protected environment, yearly rotation
  with the next key embedded, and an incident runbook.
- **rcodesign.** Its last release dates from 2024-11-29, so the macOS fallback path must be kept working.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **Unsigned releases** | SmartScreen and Gatekeeper warnings train users to bypass security prompts, and there would be no integrity evidence for CRA purposes |
| **Ad-hoc dependency review** | Does not scale with the transitive trees of egui, wgpu, wasmtime and rusqlite, and leaves no audit trail |
| **cadrum-style prebuilt downloads** | Unverifiable binaries enter the build, and the SBOM cannot describe them (C-5) |
| **Sigstore-only verification** | Needs an online trust root. Offline and air-gapped shop-floor installations could not verify, so signed sums are kept alongside |
| **SignPath Foundation OSS signing** (used by QET) | Kept as a fallback. Not primary, because its 2026 eligibility is unverified and the certificate would not carry the steward's own identity (**verify**) |
| **Velopack delta updates** | Its signature-verification mechanism is unverified (G10). In v1, updates are full installers checked against the signed manifest |

## Related

- **ADRs:** [ADR-0014](0014-gui-toolkit-egui.md), [ADR-0016](0016-3d-renderer-wgpu-and-out-of-process-step-worker.md),
  [ADR-0020](0020-extensibility-tiers-and-security-model.md), [ADR-0024](0024-licence-set-and-permissive-gpl-boundary.md),
  [ADR-0029](0029-network-telemetry-crash-reporting-policy.md), [ADR-0030](0030-legal-steward-cra-role-support-signing.md).
- **Design:** [04 architecture](../design/04-architecture.md) §13;
  [18 quality, security and release](../design/18-quality-security-release.md) §5.4, §9, §10, §11, §13, §16;
  [19 open-source governance](../design/19-open-source-governance.md) §3.3;
  [gaps-and-corrections](../research/gaps-and-corrections.md) G10, G11, C-5, C-9.
- **Requirements:** NFR-SEC-01, NFR-SEC-09, NFR-SEC-10, NFR-SEC-11, NFR-SEC-12, NFR-SEC-13, NFR-SEC-14, NFR-SEC-15,
  NFR-SEC-19, NFR-SEC-20, NFR-MAINT-02, NFR-MAINT-03, NFR-MAINT-05, NFR-LIC-04, NFR-LIC-08, NFR-COMP-01.
