<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Release checklist: v0.3.0-rc.1

[The release checklist](../../checklist.md), copied and filled for the first candidate of 0.3.0. There is no CI
until 1.0 and the release workflow has an early alpha's size, so each item says what was run by hand and what was
not. An item that is not met is on record here for the owner's go; it is not hidden by a passing suite.

**Candidate:** `v0.3.0-rc.1` · **Commit:** the commit of `release/0.3` that the tag names · **Draft:** the draft
prerelease `v0.3.0-rc.1` on the repository's releases page · **Filled by:** the release manager ·
**Date:** 2026-11-16

| Item | Check | Signed by | Result and evidence |
|---|---|---|---|
| RC-01 | The threat model reviewed for this release | — | **Not done.** No review entry for 0.3.0. The release adds no network code. It adds readers of files from outside: EPLAN EDZ archives and parts XML, EPLAN report workbooks, DXF, AutomationML and the TIA tag workbook, and it runs a converter program that the user installs and names for DWG (in a private folder, removed on every way out). Each reader is bounded as the earlier ones are and has files in the hostile corpus, which passes; none was reviewed against the threat model |
| RC-02 | No open fuzz crash; at most 10 quarantined tests | The release manager | **Met for what runs by hand.** The deep run (`cargo nextest run --workspace --all-features --run-ignored only`) passed on the release branch with the private corpus: it holds the long command fuzz, the oracle on every read, the acceptance gate and the long recordings. The hostile-corpus test passes in the whole-workspace check. No test is quarantined: every ignored test is a test of the deep run |
| RC-03 | Advisory waivers dated; SQLite's bundled version clean | The release manager | **Met.** `deny.toml` and `.cargo/audit.toml` both have `ignore = []`, so there are no waivers. `cargo deny check advisories licenses bans` reported two advisories on the tip before the cut, RUSTSEC-2026-0459 (the SQLite that `libsqlite3-sys` 0.38.2 bundles) and RUSTSEC-2026-0486 (`rustix` 1.1.5); `rusqlite` went to 0.41.1 with `libsqlite3-sys` 0.39.1 and `rustix` to 1.1.6, and the check then says "advisories ok, bans ok, licenses ok". The two crates that use SQLite name Rust 1.89, which `rusqlite` 0.41 asks for |
| RC-04 | `SECURITY.md` names the release and its support end | The release manager | **Met.** The supported-versions rows: "0.3.x — Yes, until the next minor release (0.4.0)", "0.2.x — No", "0.1.x — No" |
| RC-05 | The manual tests (RDP, screen readers, printers, network observation, the desktop installs, the crash offer) | — | **Not run.** The manual test of this release is the owner's own hands-on test of the candidate on Windows, which is its gate. The window's pictures were looked at offscreen at 1920 × 1080 and 2560 × 1440 only |
| RC-06 | The translation status meets NFR-I18N-01; the guide is in every installer | The release manager | **Met for the window and the tutorials.** `ci/translation-status.py` exits 0: the window's texts are 100.0 % nl-NL (6126 of 6126), the rule messages 100.0 % (515 of 515), the guide's introduction and tutorials 100.0 % (138 of 138); the rest of the guide is English (183 of 873 messages of the whole guide are translated). The guide's folder in the installed tree was not checked on this candidate |
| RC-07 | The content audit of the bundled assets | The release manager | **Met by its test.** `ew-library`'s content audit passes on the shipped content (licences, sources, no vendor file), with the 33 ISO 1219 symbols, the overview modules and the starter parts of this release (`ew-starter-parts` 0.9.0: generic parts only, no manufacturer's part data). `NOTICE` and the third-party notices ship in every package; hayro (Apache-2.0 OR MIT) is new among them |
| RC-08 | Profiles and format, CLI and API changes listed in the release notes | The release manager | **Met.** `docs/release-notes/0.3.0.md`, sections 4 and 5. `cargo xtask api-guard` against 0.2.0: "0 breaking, 276 additive": the file format grew by optional members, enum values and record kinds only, a 0.2.0 build keeps what it does not know, and nothing is migrated |
| RC-09 | The engineering-verification disclaimer in About, the check panel, the guide and the report | The release manager | **Present in About's source and in the release notes' section 1.** Not screenshotted for this candidate |
| RC-10 | LG-1 on file | The owner | **Met.** The owner's statement on file for 0.1.0 stands: no employer is involved |
