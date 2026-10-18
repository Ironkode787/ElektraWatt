<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Contributing to ElektraWatt

Thank you for your interest in ElektraWatt. The project is in milestone M0: the repository holds the workspace
skeleton and the CI, and no functionality yet. This guide covers the workflow that applies today. It grows with the
code, and the roles and decision rules behind it are in the
[open-source governance document](docs/design/19-open-source-governance.md).

## Where to ask

- Open an issue with one of the templates: bug, feature, part data or symbol. Blank issues are disabled.
- Security reports never go into public issues, discussions or pull requests. See [SECURITY.md](SECURITY.md).

## Building and testing

```sh
cargo build --workspace --locked
cargo nextest run --workspace
cargo xtask ci
```

`cargo xtask ci` runs fmt, clippy, nextest and the policy checks, as CI does. [ci/README.md](ci/README.md) lists
the full per-PR CI set and how to reproduce each job locally.

The golden-project commands (`ew export pdf golden/J-E-2416.ew --out out/J-E-2416.pdf`, and the acceptance runner
`ew-testkit accept`) follow with M1.

## Branches, commits and pull requests

Work on a branch and open a pull request against `main`. Pull requests are squash-merged. The PR title becomes the
squash commit and the changelog line, so it is a conventional-commit title, for example `fix(xref): …` or
`feat(import-qet): …`.

## Developer Certificate of Origin

Every commit carries a sign-off trailer. `git commit -s` adds it:

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
The `Signed-off-by:` is always a human's.

A CI check (`cargo xtask check-dco`) rejects pull requests with unsigned commits. To fix a branch, run
`git commit --amend -s` for the last commit, or `git rebase --signoff main` for the whole branch.

## Licences: the licence of the path you touch

Inbound licence equals outbound licence per path: your contribution is licensed under the licence of the path it
changes. There is no CLA and no copyright assignment. The main rows:

| Paths | Licence |
|---|---|
| Application and GPL-layer crates, `bin/` | `GPL-3.0-or-later` (the plugin exception follows with ADR-0025) |
| `crates/ew-import-qet/` | `GPL-3.0-or-later`, no exception |
| `crates/ew-import-qet/src/qet_port/` (ported QET code only) | `GPL-2.0-or-later` |
| Permissive crates: `ew-units`, `ew-ids`, `ew-mltext`, `ew-rds`, `ew-schema`, `ew-format`, `ew-interop-api`, `ew-dl`, `ew-step-proto`, `ew-memo`, `ew-plcaddr`, `ew-caex`, `ew-devdesc`, `ew-ewpp` | `MIT OR Apache-2.0` |
| CI workflows, `ci/`, `xtask/` | `MIT OR Apache-2.0` |
| Documentation | `CC-BY-4.0` |
| Repository configuration | `CC0-1.0` |

[LICENSE.md](LICENSE.md) is the full index. The path-to-licence map is in
[governance §2.1](docs/design/19-open-source-governance.md#21-licence-layers).

- Every new file carries a header with the collective copyright notice "The ElektraWatt contributors", the year,
  and the SPDX licence identifier of its path. A file that cannot carry a header gets an entry in `REUSE.toml`.
  Headers name no person and no employer.
- Permissive crates may depend only on crates with permissive licences (the licence-boundary check, NFR-MAINT-02).
- Code copied or ported from another project is declared in the PR (provenance) and keeps its original notices
  (LR-05).

## Review

- Every PR needs one approving review by someone other than the author. CODEOWNERS routes the reviews. Critical
  paths get a second owner by M1.
- A change to the file format needs the schema steward's review.
- Licence, governance, format-major and API-break changes need an accepted RFC in `rfcs/`, referenced from the PR.
  The RFC process opens with the first binary release. Until then the decision is an ADR in `docs/adr/`.

The decision matrix is in
[governance §3.4](docs/design/19-open-source-governance.md#34-decision-making-rfcs-adrs-schema-steward-roadmap).

## Pull request checklist

The [pull request template](.github/PULL_REQUEST_TEMPLATE.md) asks you to confirm:

- a conventional-commit PR title;
- signed-off commits;
- provenance of any copied or ported code (LR-05);
- AI assistance, disclosed;
- whether the PR changes the file format, so the schema steward is requested;
- documentation and rule pages updated where behaviour changes;
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
4. **No laundering.** Asking a tool to reproduce another project's code is porting, and LR-05 applies. Output
   derived from QET code may only go into `qet_port/`.
5. **Agents and the repository.** No AI agent holds merge rights or pushes to protected branches. A PR written by
   an agent follows the same review rules as any other.

## Confidentiality

The repository is public. Never attach customer drawings, project files, screenshots with customer data or
personal data to an issue or PR. Use the placeholders `<customer>`, `<end user>`, `<project ref>`, `<drawing id>`
and `<engineer>`. Do not commit local filesystem paths.

A CI anonymisation guard (`cargo xtask check-anonymisation`) compares every committed word n-gram (n ≤ 3) with
hashes of confidential tokens that the maintainers hold (`ci/denylist.sha256`; NFR-PRIV-09). If it fires, replace
the token with a placeholder. Do not paste the token into the PR discussion.

## Dependencies

A new crates.io dependency must pass:

- `cargo deny check` (licence allowlist);
- `cargo vet`: each new crate gets an audit or an exemption, reviewed by a maintainer;
- if the crate has a build script, the build-script allowlist (`ci/build-scripts.allowlist`), after you have read
  its `build.rs`.

The `who` field of a cargo-vet audit is the contributor's GitHub handle, never a personal name or e-mail address.
Public audit sets are not imported for the same reason: they record auditors' names and addresses.

## Adding a rule, report, importer or machine profile

This guide follows with the first implementation of each (NFR-MAINT-08). Until then the design documents describe
them: [checks](docs/design/15-checks-revisions-collaboration.md), [interop](docs/design/14-interop.md) and
[manufacturing outputs](docs/design/13-manufacturing-outputs.md).

## Code of Conduct

Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md).
