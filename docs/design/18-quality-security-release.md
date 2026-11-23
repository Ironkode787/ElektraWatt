# Quality engineering, security and release

**Status:** Draft v0.1

> **State during the alpha.** This document specifies the quality system for 1.0. Today the check is the development
> loop of [`CONTRIBUTING.md`](../../CONTRIBUTING.md) ("The development loop"): `cargo xtask check`, `gen` and `rules`.
> There is no CI until 1.0: `.github/workflows/` holds `release.yml` and `package.yml`, run for a release. What exists
> today: the determinism lints and repository rules (§3.5), the command-sequence fuzzer, the oracle test, snapshot and
> visual golden tests, the sample project (§3.10) and the UI tests (§3.4 to §3.11), the hostile corpus (§3.12), the persona
> harness `cargo xtask blind` (§3.17), `cargo deny`, `#![forbid(unsafe_code)]` (§10, §11.2), the release and package
> workflows with `packaging/` (§7.4, §8, §9.2), `SECURITY.md`, `PRIVACY.md` and the threat model (§11, §12). The rest is
> the target for 1.0: the CI catalogue (§5), the performance and memory gate (§4), the cargo-fuzz targets, `cargo vet`,
> signing and the update check. A cadence of "PR", "nightly" or "release" in a table is that target. No performance
> claim is measured (§4.3).

**Purpose.** This document specifies how ElektraWatt (EW) proves its correctness, performance and security, and
how it is built, signed, shipped and supported. It turns the architecture's "prove it in CI" principle
([architecture](04-architecture.md) §1.2 (10), §13) and the quality targets of
[02c](02c-nonfunctional-requirements.md) into test layers, gates, CI jobs, release rules, packaging, signing,
supply-chain controls, a threat model, a privacy design and an EU Cyber Resilience Act (CRA) position. Other design
documents refer to "the CI job in 18": this document owns those pipelines.

**Where each section lives.** The section numbers are the ones the code and the other documents cite.

| Section | File |
|---|---|
| [§1. Scope and boundaries](18-tests-ci.md#1-scope-and-boundaries) | [scope, test strategy, performance and CI](18-tests-ci.md) |
| [§2. Concepts and data](18-tests-ci.md#2-concepts-and-data) | [scope, test strategy, performance and CI](18-tests-ci.md) |
| [§3. Test strategy](18-tests-ci.md#3-test-strategy) | [scope, test strategy, performance and CI](18-tests-ci.md) |
| [§4. Performance and memory gate](18-tests-ci.md#4-performance-and-memory-gate) | [scope, test strategy, performance and CI](18-tests-ci.md) |
| [§5. CI pipeline](18-tests-ci.md#5-ci-pipeline) | [scope, test strategy, performance and CI](18-tests-ci.md) |
| [§6. Toolchain, MSRV and the upgrade train](18-release-security.md#6-toolchain-msrv-and-the-upgrade-train) | [toolchain, release, signing, security and privacy](18-release-security.md) |
| [§7. Release process](18-release-security.md#7-release-process) | [toolchain, release, signing, security and privacy](18-release-security.md) |
| [§8. Packaging per OS](18-release-security.md#8-packaging-per-os) | [toolchain, release, signing, security and privacy](18-release-security.md) |
| [§9. Signing, provenance and updates](18-release-security.md#9-signing-provenance-and-updates) | [toolchain, release, signing, security and privacy](18-release-security.md) |
| [§10. Supply chain](18-release-security.md#10-supply-chain) | [toolchain, release, signing, security and privacy](18-release-security.md) |
| [§11. Security process](18-release-security.md#11-security-process) | [toolchain, release, signing, security and privacy](18-release-security.md) |
| [§12. Privacy](18-release-security.md#12-privacy) | [toolchain, release, signing, security and privacy](18-release-security.md) |
| [§13. EU Cyber Resilience Act readiness](18-release-security.md#13-eu-cyber-resilience-act-readiness) | [toolchain, release, signing, security and privacy](18-release-security.md) |
| [§14. UI/UX touchpoints](18-release-security.md#14-uiux-touchpoints) | [toolchain, release, signing, security and privacy](18-release-security.md) |
| [§15. Outputs](18-release-security.md#15-outputs) | [toolchain, release, signing, security and privacy](18-release-security.md) |
| [§16. Edge cases and pitfalls](18-release-security.md#16-edge-cases-and-pitfalls) | [toolchain, release, signing, security and privacy](18-release-security.md) |
| [§17. The reference project in the quality system](18-release-security.md#17-the-reference-project-in-the-quality-system) | [toolchain, release, signing, security and privacy](18-release-security.md) |
