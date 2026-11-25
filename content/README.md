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

| Folder | Kind | Licence | Source | Verified |
|---|---|---|---|---|
| `content/fndefs` | function definitions | CC-BY-SA-4.0 | the project's own (`ew.original`) | — |
| `content/forms` | frames and the title block (QET-compatible) | CC-BY-SA-4.0 | the project's own, after doc 20 §17 | — |
| `content/libraries/ew-sample-parts.ewlib` | the sample project's catalogue parts and their manufacturers | CC0-1.0 | Golden A's part copies (`golden.table:…`) | `meta.confidence`: `test` or `unverified` per section, never `verified` |
| `content/libraries/ew-starter-parts.ewlib` | generic starter parts and the manufacturer "Generic" | CC0-1.0 | the project's own (`ew.original`) | — |
| `content/plc-profiles` | address profiles (`plc_profile`), compiled into `ew-derive` | CC-BY-SA-4.0 | the project's own data of doc 10 §2.5 | allocation `verified: false` |
| `content/profiles/colour` | colour profiles (`colour_profile` values) | CC-BY-SA-4.0 | the project's own table of doc 08 §4.3; "DBU/WH 24 V": the sample project's convention | `verified: false`; "DBU/WH 24 V" `verified: true` |
| `content/profiles/current` | current tables (a conductor's current by cross-section), compiled into `ew-derive` | CC-BY-SA-4.0 | the values of IEC/EN 60204-1:2016 Table 6, method B2, PVC-insulated copper, 40 °C; cited, not copied text | cross-checked against manufacturers' published tables of the same standard, not against the standard's own text |
| `content/profiles/size` | size profiles (`size_profile` values) | CC-BY-SA-4.0 | "0.75/1.5": the sample project's convention | `verified: true` |
| `content/symbols` | symbols, native edition | CC-BY-SA-4.0 | the project's own (`ew.original`) | — |
| `content/symbols/qet10` | symbols, QET 10 px edition | CC-BY-SA-4.0 | generated from `content/symbols` | — |

RC-07's evidence is this list, `NOTICE` and the run of `content_audit` on the release commit.
