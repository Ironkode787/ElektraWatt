<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Release checklist: v0.1.0-rc.3

[The release checklist](../../checklist.md), copied and filled for the candidate that became 0.1.0. CI was off for
this candidate (decision ID-1519), and the release workflow was cut to an early alpha's size (ID-1520). The owner gave
the go after their own hands-on test (ID-1521, ID-1522), with the items below that are not met on record.

**Candidate:** `v0.1.0-rc.3` · **Commit:** `135c0251b` · **Draft:** the draft prerelease `v0.1.0-rc.3` on the
repository's releases page · **Filled by:** the release manager (the orchestrator, decision log ID-1400) ·
**Date:** 2026-11-08

| Item | Check | Signed by | Result and evidence |
|---|---|---|---|
| RC-01 | The threat model reviewed for this release | — | **Not done.** No review entry for 0.1.0. The release has no network code (ID-1187). M2.5's proportionality review covers the security machinery (FT-1627) |
| RC-02 | No open fuzz crash; at most 10 quarantined tests | The release manager | **Not checked on this candidate.** CI is off (ID-1519), so no fuzz run reached rc.3. `fuzz/regressions/` holds 10 fixed cases, which the tests replay. The quarantined tests were not counted |
| RC-03 | Advisory waivers dated; SQLite's bundled version clean | The release manager | **Partly met.** `deny.toml` and `.cargo/audit.toml` both have `ignore = []`, so there are no waivers. SQLite's own vulnerability list was not checked |
| RC-04 | `SECURITY.md` names 0.1.0 and its support end | The release manager | **Met.** The supported-versions row: "0.1.x — Yes, until the next minor release (0.2.0)" |
| RC-05 | The manual tests (RDP, screen readers, printers, network observation, IN-15, PV-13) | — | **Not run.** The only manual test was the owner's own hands-on test of rc.3 on Windows (ID-1521). Its findings are due in M2.5-Fix |
| RC-06 | The translation status meets NFR-I18N-01; the guide is in every installer | The release manager | **Not met by one point.** `ci/translation-status.py` on `135c0251b`: the window's texts are 100.0 % nl-NL. The guide's Dutch translation leaves 4 of the 183 messages of the introduction and the tutorials untranslated (97.8 %), so the script exits 1 (FT-1661). The guide's folder in the installed tree was not checked separately |
| RC-07 | The content audit of the bundled assets | The release manager | **Not repeated for this candidate.** `NOTICE` and the third-party notices ship in every package |
| RC-08 | Profiles and format, CLI and API changes listed in the release notes | The release manager | **Met for a first release.** `docs/release-notes/0.1.0.md`; there is no earlier release for `api-guard` to compare against |
| RC-09 | The engineering-verification disclaimer in About, the check panel, the guide and the report | The release manager | **Present in About's source** (`crates/ew-viewmodel/src/about.rs`). Not screenshotted for this candidate |
| RC-10 | LG-1 on file | The owner | **Met.** The owner's statement that no employer is involved (ID-1401) |
