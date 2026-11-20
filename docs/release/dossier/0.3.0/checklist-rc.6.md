<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Release checklist: v0.3.0-rc.6

[The release checklist](../../checklist.md), copied and filled for the sixth candidate of 0.3.0. There is no CI
until 1.0 and the release workflow has an early alpha's size, so each item says what was run by hand and what was
not. An item that is not met is on record here for the owner's go; it is not hidden by a passing suite.

**Candidate:** `v0.3.0-rc.6` · **Commit:** the commit of `release/0.3` that the tag names · **Draft:** the draft
prerelease `v0.3.0-rc.6` on the repository's releases page · **Filled by:** the release manager ·
**Date:** 2026-11-20

| Item | Check | Signed by | Result and evidence |
|---|---|---|---|
| RC-01 | The threat model reviewed for this release | — | **Not done.** No review entry for 0.3.0. The release adds no network code. It adds readers of files from outside: EPLAN EDZ archives and parts XML, EPLAN report workbooks, DXF, AutomationML and the TIA tag workbook, and it runs a converter program that the user installs and names for DWG (in a private folder, removed on every way out). Each reader is bounded as the earlier ones are and has files in the hostile corpus, which passes; none was reviewed against the threat model. The changes since the second candidate, those of the fifth among them, add no reader of outside files and no network code; three fonts are bundled as files; F1 opens a page of the installed guide in the system's browser, as the Guide buttons did |
| RC-02 | No open fuzz crash; at most 10 quarantined tests | The release manager | **Met for what runs by hand.** The deep run (`cargo nextest run --workspace --all-features --run-ignored only`) passed on the candidate's tree with the private corpus, 18 of 18. The whole workspace passed there, 7,635 tests. No fuzz campaign runs before 1.0; no test is quarantined |
| RC-03 | Advisory waivers dated; SQLite's bundled version clean | The release manager | **Met.** `deny.toml` and `.cargo/audit.toml` both have `ignore = []`, so there are no waivers. `cargo deny check advisories licenses bans` on the candidate's tree says "advisories ok, bans ok, licenses ok". No crate was added or changed since the second candidate (`Cargo.lock` is as it was); the three fonts added are files under the SIL Open Font License 1.1, recorded in `NOTICE` and `REUSE.toml` |
| RC-04 | `SECURITY.md` names the release and its support end | The release manager | **Met.** The supported-versions rows: "0.3.x — Yes, until the next minor release (0.4.0)", "0.2.x — No", "0.1.x — No" |
| RC-05 | The manual tests (RDP, screen readers, printers, network observation, the desktop installs, the crash offer) | — | **Not run for this candidate.** The fifth candidate was not tested by hand: the owner asked for the redesign of the router first and judged its pages from pictures while it was built. The installers, a printer and a screen reader are his to try on a real desktop |
| RC-06 | The translation status meets NFR-I18N-01; the guide is in every installer | The release manager | **Met for the window and the tutorials.** `cargo xtask book-po` was run on the candidate's tree and its `--check` passes. `ci/translation-status.py` exits 0: the window's texts are 100.0 % nl-NL (7396 of 7396), the rule messages 100.0 % (578 of 578), the guide's introduction and tutorials 100.0 % (230 of 230); the rest of the guide is English (276 of 1448 messages of the whole guide are translated). The guide's folder in the installed tree was not checked on this candidate |
| RC-07 | The content audit of the bundled assets | The release manager | **Met by its test.** `ew-library`'s content audit passes on the shipped content (licences, sources, no vendor file), with the symbols and function definitions added since the second candidate (a two-pole breaker, a three-pole switch, a residual-current device, a normally closed push button, an emergency stop; an address place per solenoid on the valves with two) and the starter parts of this release (`ew-starter-parts` 0.18.0: generic parts only, no manufacturer's part data; since the fourth candidate generic bus cables and plugs for the other buses, and the two earth symbols named for what each is for). The bundled fonts Noto Sans Condensed, Noto Sans Mono and Routed Gothic 1.0.0 (OFL-1.1) are recorded with their hashes in `NOTICE`; one Tabler icon of the pinned release joined the vendored set. `NOTICE` and the third-party notices ship in every package; hayro (Apache-2.0 OR MIT) is new among them |
| RC-08 | Profiles and format, CLI and API changes listed in the release notes | The release manager | **Met.** `docs/release-notes/0.3.0.md`, sections 4 and 5. `cargo xtask api-guard` against 0.2.0: "0 breaking, 324 additive": the file format grew by optional members, enum values and record kinds only, a 0.2.0 build keeps what it does not know, and nothing is migrated |
| RC-09 | The engineering-verification disclaimer in About, the check panel, the guide and the report | The release manager | **Present in About's source and in the release notes' section 1.** Not screenshotted for this candidate |
| RC-10 | LG-1 on file | The owner | **Met.** The owner's statement on file for 0.1.0 stands: no employer is involved |
