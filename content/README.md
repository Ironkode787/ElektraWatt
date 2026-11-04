<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Shipped content

Every content set that ships with the application, as the release copies `content/` (RC-07's inventory, NFR-LIC-06,
NFR-LIC-07). Every library record carries its SPDX licence in `meta.licence` and its provenance in `meta.sources`; the
test `content_audit` of `ew-library` refuses a record without them, a licence other than its kind's (doc 07 §3.8) or
its file's row of `ci/licence-map.toml`, a part map expression that does not parse, and a folder of `content/`
without a row here. No set holds a vendor file or a maker's catalogue data.

| Folder | Kind | Licence | Source | Verified | Made by |
|---|---|---|---|---|---|
| `content/fndefs` | function definitions | CC-BY-SA-4.0 | the project's own (`ew.original`) | — | WP-S7.02 |
| `content/forms` | frames and the title block (QET-compatible) | CC-BY-SA-4.0 | the project's own, after doc 20 §17 | — | WP-S3.02 |
| `content/libraries/ew-starter-parts.ewlib` | generic starter parts and the manufacturer "Generic" | CC0-1.0 | the project's own (`ew.original`) | — | WP-S2.18 |
| `content/profiles/colour` | colour profiles (`colour_profile` values) | CC-BY-SA-4.0 | the project's own table of doc 08 §4.3 | `verified: false` | FT-337 |
| `content/symbols` | symbols, native edition | CC-BY-SA-4.0 | the project's own (`ew.original`) | — | WP-S7.02 |
| `content/symbols/qet10` | symbols, QET 10 px edition | CC-BY-SA-4.0 | generated from `content/symbols` | — | WP-S7.02 |

RC-07's evidence is this list, `NOTICE` and the run of `content_audit` on the release commit.
