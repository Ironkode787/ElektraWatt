<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Release checklist: v0.3.5-rc.1

[The release checklist](../../checklist.md), copied and filled for the first candidate of 0.3.5. There is no CI
until 1.0 and the release workflow has an early alpha's size, so each item says what was run by hand and what was
not. An item that is not met is on record here for the owner's go; it is not hidden by a passing suite.

**Candidate:** `v0.3.5-rc.1` · **Commit:** the commit of `release/0.3` that the tag names · **Draft:** the draft
prerelease `v0.3.5-rc.1` on the repository's releases page · **Filled by:** the release manager ·
**Date:** 2026-11-22

| Item | Check | Signed by | Result and evidence |
|---|---|---|---|
| RC-01 | The threat model reviewed for this release | — | **Not done.** No review entry for 0.3.5. The release adds no network code. It adds one reader of a file from outside: the font file a project names as its typeface, read with skrifa, which was in the graph already. The readers it changes (the QElectroTech import, the EPLAN report and parts imports, the tables import) keep their bounds and their files in the hostile corpus, which passes; the imports that became jobs hand no session data to another thread (the second review read this). The exports now write a mark file into a folder they made and refuse to empty one without it |
| RC-02 | No open fuzz crash; at most 10 quarantined tests | The release manager | **Met for what runs by hand.** The deep run (`cargo nextest run --workspace --all-features --run-ignored only`) passed on the candidate's tree with the private corpus, 9 of 9. The whole workspace passed there, 7,774 tests. No fuzz campaign runs before 1.0; no test is quarantined. One random test that failed now and then since the texts of a turned symbol are placed anew was narrowed to what still holds, with its failing case kept as a seed |
| RC-03 | Advisory waivers dated; SQLite's bundled version clean | The release manager | **Met.** `deny.toml` and `.cargo/audit.toml` both have `ignore = []`, so there are no waivers. `cargo deny check advisories licenses bans` on the candidate's tree says "advisories ok, bans ok, licenses ok", after `url` went from 2.5.8 to 2.5.9 for RUSTSEC-2026-0592 (in the graph of the tests of `ew` only). SQLite's own list was not read again for this candidate; the bundled version is the one 0.3.0 shipped |
| RC-04 | `SECURITY.md` names the release and its support end | The release manager | **Met.** The supported-versions rows: "0.3.x — Yes, until the next minor release (0.4.0)", "0.2.x — No", "0.1.x — No"; within 0.3.x the latest release is the supported one, as the notes' section 1 says |
| RC-05 | The manual tests (RDP, screen readers, printers, network observation, the desktop installs, the crash offer) | — | **Not run for this candidate.** The installers, a printer and a screen reader are the owner's to try on a real desktop. The exported DXF pages were not opened in a CAD program by anyone |
| RC-06 | The translation status meets NFR-I18N-01; the guide is in every installer | The release manager | **Met for the window and the tutorials.** `cargo xtask book-po --check` passes on the candidate's tree. `ci/translation-status.py` exits 0: the window's texts are 100.0 % nl-NL (7765 of 7765), the rule messages 100.0 % (578 of 578), the guide's introduction and tutorials 100.0 % (238 of 238); the rest of the guide is English (280 of 1464 messages of the whole guide are translated). The guide's folder in the installers was not looked at by hand |
| RC-07 | The content audit of the bundled assets | The release manager | **Met by its test.** `ew-library`'s content audit passes on the shipped content (licences, sources, no vendor file, a row of `content/README.md` for every folder), with what this release adds: the current table of `content/profiles/current` (values of IEC/EN 60204-1:2016 Table 6, method B2, cited, cross-checked against published tables of the same standard and not against the standard's own text; no maker named), and the starter parts' Dutch descriptions |
| RC-08 | Profiles and format, CLI and API changes listed in the release notes | The release manager | **Met.** `docs/release-notes/0.3.5.md`, sections 4, 5 and 6 ("What changes in a project you already have"). No new derivation profile: until 1.0 a project takes the latest version's rules (ADR-0003 amendment 3). `cargo xtask api-guard` against 0.3.0: "0 breaking, 362 additive". One change an older build does not read as written, recorded in ADR-0011 amendment 8: an external reference may hold no location or no device, which 0.3.0 sets aside and writes back unchanged |
| RC-09 | The engineering-verification disclaimer in About, the guide and the report | The release manager | **Present in About's source and in the release notes' section 1.** Not screenshotted for this candidate |
| RC-10 | LG-1 on file | The owner | **Met.** The owner's statement on file for 0.1.0 stands: no employer is involved |
