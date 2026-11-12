<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Release checklist: v0.2.0-rc.1

[The release checklist](../../checklist.md), copied and filled for the first candidate of 0.2.0. There is no CI
until 1.0 and the release workflow has an early alpha's size, so each item says what was run by hand and what was
not. An item that is not met is on record here for the owner's go; it is not hidden by a passing suite.

**Candidate:** `v0.2.0-rc.1` · **Commit:** the commit of `release/0.2` that the tag names · **Draft:** the draft
prerelease `v0.2.0-rc.1` on the repository's releases page · **Filled by:** the release manager ·
**Date:** 2026-11-12

| Item | Check | Signed by | Result and evidence |
|---|---|---|---|
| RC-01 | The threat model reviewed for this release | — | **Not done.** No review entry for 0.2.0. The release adds no network code and no new file kind. The QElectroTech importer reads more of a file than in 0.1.0 (title-block templates, logos, tables, free graphics), through the same bounded XML reader and the same image checks; the hostile corpus's QElectroTech files pass |
| RC-02 | No open fuzz crash; at most 10 quarantined tests | The release manager | **Met for what runs by hand.** The deep run (`cargo nextest run --workspace --all-features --run-ignored only`) passed on the release branch: it holds the long command fuzz, the oracle on every read, the acceptance gate and the long recordings. The hostile-corpus test passes in the whole-workspace check. No test is quarantined: every ignored test is a test of the deep run |
| RC-03 | Advisory waivers dated; SQLite's bundled version clean | The release manager | **Partly met.** `deny.toml` and `.cargo/audit.toml` both have `ignore = []`, so there are no waivers. No dependency changed since 0.1.0 (`Cargo.lock` differs in the workspace's own version only). SQLite's own vulnerability list was not checked |
| RC-04 | `SECURITY.md` names the release and its support end | The release manager | **Met.** The supported-versions rows: "0.2.x — Yes, until the next minor release (0.3.0)", "0.1.x — No" |
| RC-05 | The manual tests (RDP, screen readers, printers, network observation, the desktop installs, the crash offer) | — | **Not run.** The manual test of this release is the owner's own hands-on test of the candidate on Windows, which is its gate |
| RC-06 | The translation status meets NFR-I18N-01; the guide is in every installer | The release manager | **Met for the window and the tutorials.** `ci/translation-status.py` exits 0: the window's texts are 100.0 % nl-NL (5338 of 5338), the guide's introduction and tutorials 100.0 % (184 of 184); the rest of the guide is English (184 of 731 messages of the whole guide are translated). The guide's folder in the installed tree was not checked on this candidate |
| RC-07 | The content audit of the bundled assets | The release manager | **Met by its test.** `ew-library`'s content audit passes on the shipped content (licences, sources, no vendor file), with the new parts and the Dutch symbol names of this release. `NOTICE` and the third-party notices ship in every package; the notices of the AppImage's runtime are unchanged |
| RC-08 | Profiles and format, CLI and API changes listed in the release notes | The release manager | **Met.** `docs/release-notes/0.2.0.md`, sections 4 and 5. `cargo xtask api-guard` against 0.1.0: "0 breaking, 0 additive", so the file format is unchanged and nothing is migrated |
| RC-09 | The engineering-verification disclaimer in About, the check panel, the guide and the report | The release manager | **Present in About's source and in the release notes' section 1.** Not screenshotted for this candidate |
| RC-10 | LG-1 on file | The owner | **Met.** The owner's statement on file for 0.1.0 stands: no employer is involved |
