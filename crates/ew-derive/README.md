<!-- SPDX-FileCopyrightText: 2026 The ElektraWatt contributors -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ew-derive

All domain queries of ElektraWatt and the tracked-read context `Q` (ARCH §6, ADR-0002, ADR-0003). A derived
value is a pure function `fn(q: &Q, key) -> Arc<V>` declared through the `Query` trait; `Q` exposes only tracked
accessors and never hands out the model. Phase 1 is cold: values are computed on demand and cached for the
lifetime of one `Q`; every read goes through a `Tracker` (a no-op now) so that `ew-memo` can record dependencies
later without changing query code. `FAMILIES` lists every query family with its initial policy (ARCH §6.4).

## Derivation profiles

The shipped derivation profiles are TOML data in `profiles/` (`2027.1.toml`): numbering scheme, cross-reference
format, title-block binding set, identifier set and the TOC layout. A project pins one through
`Project.derivation_profile`; the project's own settings win over the profile's defaults. A shipped profile is
never changed; an output-changing fix ships as a new profile (ADR-0003).

## Provisional shims

These read data whose record shape is not decided yet. Each is a shim, to be replaced when the owning work
package types the data in `ew-schema`:

| Shim | Reads | Replaced by |
|---|---|---|
| Numbering settings | `ProjectSettings.extra["numbering"]` in the JSON shape of doc 03 §5.1 | the typed `ProjectSettings.numbering` (WP-S1.11) |
| Frame defaults | `ProjectSettings.extra["frames"]` (doc 03 §5.1 `FrameDefaults`) | the typed member (WP-S3.02) |
| Company logo and notice | `Project.extra["ew.company"]` = `{"logo": "sha256:…", "notice": <MlText>}` for `project.logo` and `project.notice` | typed company data of the basic project (doc 07 §2.8) |
| Revisions | reserved `revision` records, read leniently (`label`, `date`, `state`, `author`, `checker` as party roles) | the typed `Revision` record (doc 03 §18) |

## Agreement with the command layer

`device_tag` takes the main representation by the rule of `ew_commands::numbering` (the smallest-ID
representation of the smallest-ID main function), so that tags are unique wherever the command layer's counters
are. The richer rule of doc 03 §8.2 step 4 must replace both rules together (WP-S1.11); `tests/agreement.rs`
pins the agreement.
