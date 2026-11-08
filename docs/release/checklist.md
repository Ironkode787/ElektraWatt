<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Release checklist (0.1.x)

The items a pipeline cannot prove (doc 18 §7.4, RC-01 to RC-10, in their 0.1.0 form; WP-S6.07 D14, §7). This file
is the template: for each candidate, WP-S6.08 copies it into the release dossier (GP-11) and fills the last two
columns. The procedure around it is [the release procedure](README.md). RC-11, the Accessibility Conformance Report,
is 1.0's. The release workflow runs no CI, no SBOM and no install test at 0.1.x (ID-1520): RC-02 reads the last
fuzz run there is, and RC-05's desktop installs are the one install test.

**Candidate:** `v<X.Y.Z>-rc.<n>` · **Commit:** `<sha>` · **Draft:** `<release page>` · **Filled by:** `<maintainer>` ·
**Date:** `<YYYY-MM-DD>`

| Item | Check | Evidence asked for | Signed by | Result and evidence |
|---|---|---|---|---|
| RC-01 | The threat model (WP-S6.04) is reviewed for the parsers and extension points added since its last review (NFR-SEC-01); the review also finds no analytics, install identifier, feature counter, AI or translation call (NFR-PRIV-02, NFR-PRIV-08; WP-S6.05 D18) | The threat model's review entry for this release, with the parsers and extension points it covers | The maintainer who reviewed it | |
| RC-02 | No open fuzz crash: `fuzz/regressions/` and the findings of the last deep nightly that ran (CI is off until 1.0, ID-1519); at most 10 quarantined tests (doc 18 §3.14) | The last deep nightly's run, or "none since CI stopped"; the count of quarantined tests | The release manager | |
| RC-03 | Every advisory waiver in `deny.toml` and `.cargo/audit.toml` has a reason, an owner and an expiry within 90 days of the release; SQLite's bundled version (libsqlite3-sys's bundled `sqlite3.h`) has no open vulnerability on SQLite's own list (NFR-SEC-11; WP-S6.07 D11) | The two files' `ignore` entries, each with its expiry; the SQLite version and the date the list was read | The release manager | |
| RC-04 | `SECURITY.md`'s supported-versions table names 0.1.0 and its support end (NFR-COMP-03) | The table's row | The release manager | |
| RC-05 | The manual tests: RDP without GPU, NVDA and Orca, two printers, Windows network observation, WP-S6.06's four desktop installs (its IN-15), and WP-S6.05's PV-13 on Windows: a window with a dirty project ended in Task Manager offers the project at the next start, and `icacls` shows the state folder's per-user ACL (doc 18 §12.2; WP-S6.05 D10) | The test reports, one per test, with the build they ran on | The tester of each | |
| RC-06 | The translation status meets NFR-I18N-01; the guide is built and in every installer | The translation report; the guide's folder in the installed tree (WP-S6.06's listing) | The release manager | |
| RC-07 | The content audit of the bundled assets: licences, provenance, no standards text, no Tahoma (NFR-LIC-06, NFR-COMP-05) | `NOTICE` and the audit's notes | The release manager | |
| RC-08 | Derivation profiles, format changes and CLI or API changes are listed in the release notes | `docs/release-notes/<X.Y.Z>.md`; `api-guard`'s report against the previous release | The release manager | |
| RC-09 | The engineering-verification disclaimer is in About, the check panel, the guide and the release report (NFR-COMP-04) | A screenshot of each window place; the guide's page; the report | The release manager | |
| RC-10 | LG-1, the employer copyright statement, is on file (doc 19 §3.3; ID-1203); LG-4 and the steward checklist are 1.0's | The statement, or the owner's statement that no employer's staff contributed (WP-S6.07 §13 O3) | The owner | |

Who is the release manager at 0.1.0 is the owner's choice (WP-S6.07 §13 O4; the recommendation: the owner, the one
maintainer). A failed item is a blocker: it is fixed on `main` and a new candidate is cut (the procedure's §8).
