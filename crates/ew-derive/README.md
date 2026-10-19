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

## Derivation notes

A `DeriveNote` reports a fallback or a problem met while deriving: an unknown profile (an error, integrity code
EW-CORE-018), a numbering scheme not applied, the default frame, a malformed shim, a query cycle. Notes are
request-scoped: a set in canonical order, read by the owner of the `Q` through `Q::diagnostics()` (not reachable
from a query body). `ew-api`'s `Session` surfaces them as findings. An unknown pinned profile is never silent
(ADR-0003): display uses the newest shipped profile, the note is an error, and profile-dependent commands are
refused in WP-S1.11 (not yet implemented).

## Provisional shims

These read data whose record shape is not decided yet. Each is a shim, to be replaced when the owning work
package types the data in `ew-schema`:

| Shim | Reads | Replaced by |
|---|---|---|
| Numbering settings | `ProjectSettings.extra["numbering"]` in the JSON shape of doc 03 §5.1 | the typed `ProjectSettings.numbering` (WP-S1.11) |
| Frame defaults | `ProjectSettings.extra["frames"]` (doc 03 §5.1 `FrameDefaults`); a malformed member falls back to the default and is noted; with no frame configured the built-in QET-compatible A4 frame geometry (17 × 8 cells) applies and is noted | the typed member (WP-S3.02) |
| Company logo and notice | `Project.extra["ew.company"]` = `{"logo": "sha256:…", "notice": <MlText>}` for `project.logo` and `project.notice` | typed company data of the basic project (doc 07 §2.8) |
| Revisions | reserved `revision` records, read leniently (`label`, `date`, `state`, `author`, `checker` as party roles); also `page_notes` and `release.changed_pages` for `page.last_changed_in` (the no-digest branch of doc 15 §5.6) | the typed `Revision` record (doc 03 §18) |

## Connectivity (WP-S1.07)

`page_local_nets(page)` → `stitch_graph()` → `potential_partition()` (doc 08 §3.3) give nets, signals and
potentials; `physical_wires_of(potential)`, `potential_flow(potential)` and `wire_props_of(potential)` build on
them, and `wire_props(conn)` is a cold lookup into the per-potential map (ARCH §6.3). Readings this crate applies
where the documents leave the data open:

- **Pairing default.** An absent `ip_pairing` setting means star mode with project scope.
- **Terminal level rule.** A terminal function whose FnDef declares no pass-through pair joins all its CPs of one
  level: a template-origin terminal takes its CP indices from the part pattern, which the FnDef links do not name.
- **Duplicated pins.** Two CPs of one function with the same printed designation are one signal unless a declared
  pass-through pair links them (the black-box PSU's +#1/+#2 and −#1/−#2).
- **Rail bonds.** The rail-bonded terminals of one location are joined through their rail-foot CPs (else their
  smallest CP in the graph).
- **Bond cut.** Only the `via` connection is cut; the interruption points on the via path stay members of the
  potential they are wired to but name the bond, not the potential.

## Provisional shims (connectivity)

| Shim | Reads | Replaced by |
|---|---|---|
| Colour and size profiles, wire default | `ProjectSettings.extra["colour_profile"]`, `["size_profile"]`, `["wire_default"]` in the doc 03 §5.1 shapes; absent members fall back to the built-in defaults of the derivation profile (provenance `BuiltIn`) and are noted | the typed `ProjectSettings` members (doc 03 §5.1, decision log ID-17) |

## Agreement with the command layer

`device_tag` takes the main representation by the rule of `ew_commands::numbering` (the smallest-ID
representation of the smallest-ID main function), so that tags are unique wherever the command layer's counters
are. The richer rule of doc 03 §8.2 step 4 must replace both rules together (WP-S1.11); `tests/agreement.rs`
pins the agreement.

The main representation excludes contact images in both layers (doc 03 §5.1). Only schemes the command layer
numbers are applied (counter scope `page`, format and rules containing `{page}`, `{letter}` and `{counter}`); a
scheme the command layer does not number falls back to the profile's default scheme, with a note, until WP-S1.11.
An unknown named key still resolves to the project's default scheme, else the profile's.

## Limits

- `function_cps` detects template origin only through a part template naming the function's FnDef copy; else the
  FnDef's connection points apply.
- `RdsSyntax::default()` is used for every printed designation until the project's designation syntax setting
  exists (FR-PRJ-016); `Tag::displayed` likewise.
