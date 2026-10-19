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
EW-CORE-018), a numbering scheme not applied, the default frame, a malformed shim, an overlapping colour-profile row pair, a query cycle. Notes are
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

- **Pairing default.** An absent `ip_pairing` setting means star mode with project scope. The star source is the
  point flagged `star_source`, else the single Source, else, with two or more Sources, the lowest-ID Source (noted),
  else the lowest-ID point. A scoped pairing (function or location) on a page without the aspect node pairs
  project-wide and is noted (EW-CORE-051).
- **No conductor edge.** `Core` connections and the IP pairing suffice; the drawing graph has no separate
  conductor edge.
- **Built-in defaults.** Without profiles the defaults of doc 08 §4.3 apply (decision log ID-150, the EN 60204-1
  column): an L-family potential of an AC control circuit is RD, and the cross-section is 1.5 mm² for Mains and
  0.75 mm² for every other class.
- **Aliases.** Doc 08 §3.5 governs over doc 20 §7.3: the aliases are every other distinct IP name of the potential in
  natural order (bond-path points excluded). §7.3 lists the golden aliases in that same order, so the golden
  partition matches both; the code follows §3.5.
- **PE rule.** A PE wire takes the highest circuit class at its two ends, ranked `Mains > Control > Analogue > Bus`;
  when no end has a ranked class the class is Control.
- **Cores.** The colour profile never recolours a core: its colour is the conductor's, else the project default or
  the built-in colour; its size is the conductor's, else the rules.
- **Loads (D4).** The loads of a potential are its consuming functions (coils, lamps, inputs); never a function with
  a pass-through pair touching the potential, a source feeding it, an interruption point or a terminal.
- **Hub arms (D1).** The carrier of a wire resolved at a hub is the connection of its arm that touches the end CP
  (doc 08 §3.8); the Target-IP rule applies to hub-free paths only.
- **Duplicated pins (D2).** Two CPs of one function with the same designation are one pin only on black boxes and on
  functions of part-pattern origin (the PSU's +#1/+#2 and −#1/−#2); never on terminals, never across levels.
- **Terminal levels (D3).** A terminal with a declared pass-through pair shorts the same-level CPs of each side and
  the pass-through links the sides; without one, all CPs of a level are one signal.
- **Rail bonds (D5).** Rail bonding joins only CPs with clamp rail foot or role PE or PEN, chained per location. A
  rail-bonded terminal without such a CP joins nothing (EW-TERM-005); one without a location is skipped (EW-POT-008).
- **External names (D6).** An external source IP may name an undefined potential: the name is the first definition,
  else the first non-external source IP, else the first external source IP, else the `<tag>:<designation>` fallback.
- **Conflicts.** Nothing merges silently: differing type, voltage, environment or class between definitions, and a
  defined type that contradicts a member role (FE on PE or PEN, PE or N on PEN are compatible), are EW-POT-001;
  the first definition in ID order wins.
- **Override provenance.** `PropSource::Override` names the constituent connection that carries the value
  (`PhysicalWire.override_sources`): the carrier for what it sets, else the first other constituent that sets it.
- **Bond cut.** Only the `via` connection is cut; the interruption points on the via path stay members of the
  potential they are wired to but name the bond, not the potential.

## Provisional shims (connectivity)

| Shim | Reads | Replaced by |
|---|---|---|
| Colour and size profiles, wire default | `ProjectSettings.extra["colour_profile"]`, `["size_profile"]`, `["wire_default"]` in the doc 03 §5.1 shapes; absent members fall back to the built-in defaults of the derivation profile (provenance `BuiltIn`) and are noted | the typed `ProjectSettings` members (doc 03 §5.1, decision log ID-17) |

## Cross-references (WP-S1.08)

`xref_text(rep)` (Memo by policy, cold in phase 1) prints the cross-reference of a representation from
`xref_relation(rep)` (cold) and `xref_format()` (cold); nothing is stored (FR-XRF-001, ADR-0003 class I).

| Representation (first function) | Relation | Printed (golden) |
|---|---|---|
| interruption point | its `stitch_graph` pair partners (star: the source lists every other point, each other point the source; chain: the one partner) | "102-A1", "101-H16" |
| external connection point | the `ExternalRef` | "+J-E-206 / JE-N1000-03" (no link) |
| main function on the main representation | the device's other functions' representations, then the other representations of the main function | "∞ 3-B2, 106-G3" |
| auxiliary, or another representation of the main function | the main representation | "(201-E1)" |
| contact image of the main function | one row per auxiliary: designations and cell | "13-14 /105.3, 21-22 /106.5" |

- Cells come from `rep_anchor_cell` (the symbol's anchor policy, never rendered geometry), so "3-B0" (D-14) cannot
  arise; a target without a cell prints "?" and is noted (EW-CORE-053).
- Targets are ordered by (page-order position, anchor row, anchor column, representation ID); lists use the
  profile's matrix (3 per row, 9 at most, then ", …").
- The format engine (`xref_format` module) reads `{page}` `{page_index}` `{row}` `{column}` `{function}`
  `{location}` `{list}` and the QET variables `%F` `%f` `%l` `%c` `%M` `%LM` (FR-XRF-003); structure prefixes
  follow `structure` (`when_different`, `always`; FR-XRF-004) and `same_page = "zone_only"` prints only the cell
  (FR-XRF-025).
- Not cross-referenced: strip segments, contact images of auxiliaries, an auxiliary drawn inside its own device's
  black box (doc 06 §2.7). A second main function (EW-INV-002) is treated as an auxiliary and noted.
- Each shown item is an `XrefLink` (byte range, target representation); the page output links a slot text to
  its first item, since a scene slot carries one link.

## Provisional shims (cross-references)

| Shim | Reads | Replaced by |
|---|---|---|
| Cross-reference format | `ProjectSettings.extra["xref_format"]` in the doc 03 §5.1 shape (a `name` member is ignored); absent: the pinned profile's `[xref]`; malformed: noted, the profile's | the typed `ProjectSettings.xref_format` (decision log ID-17) |

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
