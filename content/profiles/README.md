<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Shipped profiles

Profile data that ships with the application and that the company-standard wizard offers (doc 16 §9.2 step 3,
FT-337). Each file is one value of the project-settings member it names, as `project.settings.set` writes it.

| Path | Member | Profile |
|---|---|---|
| `colour/ew.colour.en60204-1-machine.json` | `colour_profile` | "EN 60204-1 machine" |
| `colour/ew.colour.iec60445-installation.json` | `colour_profile` | "IEC 60445 installation" |

**Provenance.** The rows are the project's own table of `docs/design/08-connections-terminals-cables.md` §4.3,
cited by standard and clause (NFR-COMP-05). No text, table or figure of a standard was copied; the group, function
and colour names are the project's own words, and the colour codes are the IEC 60757 letter codes. Both profiles
ship with `verified: false` until a reviewed change checks them against licensed copies of the standards; the rows
that doc 08 marks "(verify)" say so in their function text.

**What a selector cannot express.** A profile row selects by potential type, AC or DC, voltage range and analogue
flag, not by circuit class. EN 60204-1's red for an AC control line therefore applies to potentials typed `signal`
on AC; an AC control potential typed `L` takes the black line row (the built-in default, which knows the circuit
class, colours it red). The orange of excepted circuits has no selector and is not in the profile.
