<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Contributing to ElektraWatt

Thank you for your interest in ElektraWatt. The project is an early alpha: 0.1.0 is released, and the work now is
to make it lean and to make it work well. The roles and decision rules are in the
[open-source governance document](docs/design/19-open-source-governance.md).

## Where to ask

- Open an issue with one of the templates: bug, feature, part data or symbol. Blank issues are disabled.
- Security reports never go into public issues, discussions or pull requests. See [SECURITY.md](SECURITY.md).

## The development loop

Three commands, run from the repository root:

```sh
cargo xtask check -p <crate> [-p <crate>]...   # format, lint and test what you changed, then the repository rules
cargo xtask gen                                 # regenerate the generated files
cargo xtask rules                               # the repository rules alone, in seconds
```

- **`check`** formats every Rust file, runs clippy with warnings as errors and the tests of the crates you name,
  with all features, and then the repository rules. Name the crates you changed and the crates that show the
  change (a command usually shows in `ew-api` and `ew-viewmodel`). Without `-p` it takes the whole workspace,
  about twelve minutes: do that before a release, not for every change.
- **`gen`** rewrites every file that is generated from code or data. Run it after you change a command, a check
  rule, a text key, a keymap, an icon or a library symbol, and commit what it changed with your change.
- **`rules`** checks the crate layers and the licence boundary, test-only features, raw reads of the model,
  confidential tokens, and that the generated files xtask writes are current. It builds nothing.

There is no continuous integration until 1.0. `check` is the check.

### Tests

- A test must not break because an unrelated feature added a command, a word, an action, a rule, a part, a row or a
  file. Do not write the total of a set that grows, a generated name or a path as a literal: read it from its
  source, or assert a relation (equal sets, a subset, "one per command", "not empty"). Counts of the test's own
  fixture are fine.
- A new integration test goes into its crate's one test binary, `tests/it/<name>.rs`. Touch `tests/it/main.rs`
  once after you add the file, since cargo does not see a new file in that folder until `main.rs` changes.
- No performance tests and no timing assertions during development.

### One place for every item

A command, an action, a rule, a text or an icon is declared in one place, and everything else follows from it at
build time or through `cargo xtask gen`. Do not add a list that has to be kept in step by hand; if a change seems to
need one, generate it or derive it instead.

## Decisions and open work

- Open work is a row of [`docs/plan/backlog.md`](docs/plan/backlog.md). A finished row is deleted.
- A decision that changes how something works is written into the document that describes it (a design document
  under `docs/design/` or an ADR under `docs/adr/`), so that the document states what is true now.
- The commit message says what changed and why. No trailer is required.

## Branches, commits and pull requests

Work on a branch and open a pull request against `main`. Pull requests are squash-merged. The PR title becomes the
squash commit and the changelog line, so it is a conventional-commit title, for example `fix(xref): …` or
`feat(import-qet): …`.

## Developer Certificate of Origin

Every commit of a contributor carries a sign-off trailer. `git commit -s` adds it:

```text
Signed-off-by: Your Name <address>
```

The sign-off certifies the [Developer Certificate of Origin 1.1](https://developercertificate.org). In short:

1. You wrote the contribution, or you otherwise have the right to submit it under the licence of the path it
   changes.
2. Or it is based on earlier work that carries a compatible open-source licence, and you may submit the modified
   work under that licence.
3. Or it was given to you by someone who certified one of the two points above, and you have not changed it.
4. You understand that the contribution and your sign-off are public and are kept in the project history.

The name must be one you are commonly known by. Anonymous contributions are not accepted.
The `Signed-off-by:` is always a human's. A maintainer checks the sign-offs at review; to fix a branch, run
`git commit --amend -s` for the last commit, or `git rebase --signoff main` for the whole branch.

## Licences: the licence of the path you touch

Inbound licence equals outbound licence per path: your contribution is licensed under the licence of the path it
changes. There is no CLA and no copyright assignment. The main rows:

| Paths | Licence |
|---|---|
| Application and GPL-layer crates, `bin/` | `GPL-3.0-or-later` (the plugin exception follows with ADR-0025) |
| `crates/ew-import-qet/` | `GPL-3.0-or-later`, no exception |
| `crates/ew-import-qet/src/qet_port/` (ported QET code only) | `GPL-2.0-or-later` |
| Permissive crates: the file format, identifiers, units and the other crates whose manifest says so | `MIT OR Apache-2.0` |
| Release workflows, `ci/`, `xtask/` | `MIT OR Apache-2.0` |
| Documentation | `CC-BY-4.0` |
| Repository configuration | `CC0-1.0` |

[LICENSE.md](LICENSE.md) is the full index. The path-to-licence map is in
[governance §2.1](docs/design/19-open-source-governance.md#21-licence-layers), and `cargo xtask rules` checks it.

- Every new file carries a header with the collective copyright notice "The ElektraWatt contributors", the year,
  and the SPDX licence identifier of its path. A file that cannot carry a header gets an entry in `REUSE.toml`.
  Headers name no person and no employer.
- Permissive crates may depend only on crates with permissive licences, and they build on the Rust version their
  manifest names.
- Code copied or ported from another project is declared in the PR (provenance) and keeps its original notices.

## Review

- Every PR needs one approving review by someone other than the author.
- A released file format changes only additively (ADR-0011): `cargo xtask api-guard` names any other change to
  `schemas/<format>/` and any change to its frozen copy `fixtures/format/<format>/`.
- Licence, governance, format-major and API-break changes are decided in an ADR in `docs/adr/`.

## Pull request checklist

The [pull request template](.github/PULL_REQUEST_TEMPLATE.md) asks you to confirm:

- a conventional-commit PR title;
- signed-off commits;
- provenance of any copied or ported code;
- AI assistance, disclosed;
- whether the PR changes the file format;
- documentation updated where behaviour changes;
- no customer data, personal data, confidential drawings or local paths.

## AI-use policy

The core team works with AI assistance. This policy regulates the use of AI tools; it does not ban it.

1. **Disclose.** A PR discloses AI assistance in the template. A commit may name the tool in a trailer such as
   `Assisted-by:` or `Co-Authored-By:`. The `Signed-off-by:` is always a human's.
2. **Own it.** The human who signs has read, understood and tested every line, and answers review questions
   personally. Discussion in issues and PRs is not AI-generated.
3. **Third-party and confidential content.** The QET element collection, EDZ and Data Portal content and standard
   texts are never used as training or fine-tuning data. They are never pasted into AI tools to generate library
   content. QET example projects are excluded from any data set used with AI tools. Confidential material, such as
   customer projects, goes into an AI tool only with its owner's permission and in a configuration that excludes
   it from training.
4. **No laundering.** Asking a tool to reproduce another project's code is porting, and the provenance rule
   applies. Output derived from QET code may only go into `qet_port/`.
5. **Agents and the repository.** No AI agent holds merge rights or pushes to protected branches. A PR written by
   an agent follows the same review rules as any other.

## Confidentiality

The repository is public. Never attach customer drawings, project files, screenshots with customer data or
personal data to an issue or PR. Use the placeholders `<customer>`, `<end user>`, `<project ref>`, `<drawing id>`
and `<engineer>`. Do not commit local filesystem paths.

`cargo xtask rules` compares every committed word n-gram (n ≤ 3) with hashes of confidential tokens that the
maintainers hold (`ci/denylist.sha256`). If it fires, replace the token with a placeholder. Do not paste the token
into the PR discussion.

## Dependencies

A new crates.io dependency must pass `cargo deny check` (licences, advisories and sources). Add one only when
writing the few lines yourself is clearly worse.

## Code of Conduct

Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md).
