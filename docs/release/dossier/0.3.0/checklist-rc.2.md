<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Release checklist: v0.3.0-rc.2

[The release checklist](../../checklist.md), copied and filled for the second candidate of 0.3.0. There is no CI
until 1.0 and the release workflow has an early alpha's size, so each item says what was run by hand and what was
not. An item that is not met is on record here for the owner's go; it is not hidden by a passing suite.

**Candidate:** `v0.3.0-rc.2` · **Commit:** the commit of `release/0.3` that the tag names · **Draft:** the draft
prerelease `v0.3.0-rc.2` on the repository's releases page · **Filled by:** the release manager ·
**Date:** 2026-11-16

| Item | Check | Signed by | Result and evidence |
|---|---|---|---|
| RC-01 | The threat model reviewed for this release | — | **Not done.** No review entry for 0.3.0. The release adds no network code. It adds readers of files from outside: EPLAN EDZ archives and parts XML, EPLAN report workbooks, DXF, AutomationML and the TIA tag workbook, and it runs a converter program that the user installs and names for DWG (in a private folder, removed on every way out). Each reader is bounded as the earlier ones are and has files in the hostile corpus, which passes; none was reviewed against the threat model. The fixes since the first candidate add no reader of outside files and no network code; a tutorial's practice project opens in a second instance of the program, started with the project's folder as its one argument |
| RC-02 | No open fuzz crash; at most 10 quarantined tests | The release manager | **Met for what runs by hand.** The deep run (`cargo nextest run --workspace --all-features --run-ignored only`) passed on the release branch with the private corpus: it holds the long command fuzz, the oracle on every read, the acceptance gate and the long recordings. The hostile-corpus test passes in the whole-workspace check. No test is quarantined: every ignored test is a test of the deep run |
| RC-03 | Advisory waivers dated; SQLite's bundled version clean | The release manager | **Met.** `deny.toml` and `.cargo/audit.toml` both have `ignore = []`, so there are no waivers. `cargo deny check advisories licenses bans` on the candidate's tree says "advisories ok, bans ok, licenses ok". No crate was added or changed since the first candidate (`Cargo.lock` differs only in the workspace-hack crate's own lines) |
| RC-04 | `SECURITY.md` names the release and its support end | The release manager | **Met.** The supported-versions rows: "0.3.x — Yes, until the next minor release (0.4.0)", "0.2.x — No", "0.1.x — No" |
| RC-05 | The manual tests (RDP, screen readers, printers, network observation, the desktop installs, the crash offer) | — | **Not run for this candidate.** The owner tested the first candidate by hand on Windows and found 29 points; this candidate answers them and waits for his test, which is the gate. The window's pictures were looked at offscreen at 1920 × 1080 and 2560 × 1440 only; seven picture-led persona runs on the fixes (four, then three re-tests after their fix lanes) did their jobs, the last three without a blocker |
| RC-06 | The translation status meets NFR-I18N-01; the guide is in every installer | The release manager | **Met for the window and the tutorials.** `cargo xtask book-po` was run on the candidate's tree and its `--check` passes. `ci/translation-status.py` exits 0: the window's texts are 100.0 % nl-NL (6656 of 6656), the rule messages 100.0 % (524 of 524), the guide's introduction and tutorials 100.0 % (223 of 223, with the three new tutorials); the rest of the guide is English (268 of 1095 messages of the whole guide are translated). The guide's folder in the installed tree was not checked on this candidate |
| RC-07 | The content audit of the bundled assets | The release manager | **Met by its test.** `ew-library`'s content audit passes on the shipped content (licences, sources, no vendor file), with the ISO 1219 symbols redrawn on a 12 mm module and the ones added since the first candidate, the overview modules and the starter parts of this release (`ew-starter-parts` 0.13.0: generic parts only, no manufacturer's part data). Tabler icons of the pinned release joined the vendored set. `NOTICE` and the third-party notices ship in every package; hayro (Apache-2.0 OR MIT) is new among them |
| RC-08 | Profiles and format, CLI and API changes listed in the release notes | The release manager | **Met.** `docs/release-notes/0.3.0.md`, sections 4 and 5. `cargo xtask api-guard` against 0.2.0: "0 breaking, 301 additive": the file format grew by optional members, enum values and record kinds only, a 0.2.0 build keeps what it does not know, and nothing is migrated |
| RC-09 | The engineering-verification disclaimer in About, the check panel, the guide and the report | The release manager | **Present in About's source and in the release notes' section 1.** Not screenshotted for this candidate |
| RC-10 | LG-1 on file | The owner | **Met.** The owner's statement on file for 0.1.0 stands: no employer is involved |
