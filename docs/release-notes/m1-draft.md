<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# ElektraWatt m1: release notes (draft)

**Status:** Skeleton (FT-299, G-M1-22); the lead fills it before the source tag `m1` is signed. **Part of:**
ElektraWatt release notes. Lines marked TODO are for the lead to complete; nothing else here is final.

> **This is a source tag, not a binary release.** The tag `m1` marks the source of the M1 spine exit. The nightly
> `ew` command-line builds of the workflow `cli-artefact.yml` are labelled "unsigned development build, not a
> release": they carry no version tag and no signature, run only on the CI runners that built them, and are
> not published. No installer, no signed binary and no download page exist for m1.

## 1. Scope

What the `m1` source tag contains (TODO: one paragraph from the delivered work packages of
[milestone M1](../plan/01-global-plan.md) and the [G-M1 readiness run](../plan/gate-records/G-M1-2026-10-readiness.md)).

- The model, the command layer and the project format of the spine (TODO: WP-S1.01 to WP-S1.04, WP-S1.09).
- The `ew` command-line interface and the headless session API (TODO: the command count and the CLI subcommands).
- The golden project built by commands (TODO: Golden A and the acceptance runner).
- TODO: what M1 does not contain (the desktop application beyond the first shell, the importers, PLC export,
  and every item of the plan that belongs to M2 or later).

## 2. Golden acceptance

`cargo xtask acceptance` over the headless M1 subset ([doc 20](../design/20-golden-project.md) §19 and the
acceptance runner of [doc 18](../design/18-quality-security-release.md) §3.10):

| Entries | Pass | Fail | Not built | Not applicable | Oracle mismatches |
|---|---|---|---|---|---|
| 94 | 88 | 0 | 0 | 6 | 0 |

TODO: refresh these counts from the report of the delivery commit (the committed headless report), name the six
`not_applicable` entries and why, and cite the report's commit.

## 3. Known defects

- **FT-316.** Undo of `library.embed` is refused when a row names an embedded item through a `LibRef`, so a
  golden build does not undo completely; the structural keep-alive is to become a delete-time prevent check
  (decision log ID-462). TODO: state the user-visible effect and the workaround.
- TODO: the other open follow-up tasks that a user can meet (from the plan's follow-up table, with their FT
  numbers), and the checks that are report-only (coverage, performance on hosted runners).

## 4. Unsigned development builds

TODO: repeat the warning in the words that go on the tag page. The content to keep:

- the nightly artefacts are named `ew-<os>-<arch>-unsigned-dev-<short sha>` and hold the `ew` program, a README
  and the licence text;
- they are built by CI in the release profile, are not signed or notarised (Windows and macOS may warn), and
  expire after 90 days;
- the source tag is signed by the project owner; no binary is signed or published for m1.

## 5. Build from source

TODO: the Rust toolchain version (`rust-toolchain.toml`), `cargo build -p ew --release --locked`, and the
verification commands of the readiness run.

## 6. Licences

TODO: the licence layers in one short paragraph ([doc 19](../design/19-open-source-governance.md) §2.1) and the
statement about the CC-BY-SA-4.0 symbols compiled into the binary (decision log ID-383, accepted by the owner as a
distribution fact on 2026-10-22, ID-454).
