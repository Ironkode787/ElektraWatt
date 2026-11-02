<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Governance

This page says who decides what in ElektraWatt and how a contribution is reviewed. It is short on purpose: the project
is small, and the longer design of its governance is in the
[open-source governance document](docs/design/19-open-source-governance.md), which this page does not replace.

## Who decides

The project has one maintainer. The maintainer decides what is built, what is merged and what is released. There is no
board and no vote. Reviews and decisions are recorded in the open, so that anyone can see why a choice was made.

## Where decisions are written down

- **Architecture and policy decisions** are architecture decision records (ADRs) in [docs/adr/](docs/adr/README.md).
  A decision that the maintainer takes is recorded there, with its status, before the work that depends on it ships.
- **Planning and design decisions** are one-line entries in [docs/plan/decisions.md](docs/plan/decisions.md), with the
  older ones in the [decision log](docs/plan/06-decision-log.md). A commit that implements a decision names it in a
  `Decision:` trailer ([CONTRIBUTING.md](CONTRIBUTING.md), "Records: commit trailers").
- **Requirements** are in [docs/design/](docs/design/), and the plan, the milestones and the gates are in
  [docs/plan/](docs/plan/README.md).

## The RFC process is the ADR process

A change that the design document calls an RFC (a format major, a licence or a governance change, a break in the
command or CLI guarantees) is proposed and decided as an ADR. The proposal is a pull request that adds or amends the
ADR; it is merged when the maintainer accepts it, and the implementing change names it. There is no separate `rfcs/`
directory.

## How a contribution is reviewed

Contributions are pull requests. [CONTRIBUTING.md](CONTRIBUTING.md) says how to build and test, how to sign off a
commit (the Developer Certificate of Origin), which checks CI runs, and how reviews work ("Review"): every pull request
needs an approving review by someone other than its author, and `.github/CODEOWNERS` routes the reviews. Contributions
made with AI assistance follow the AI-use policy in the same file.

## Conduct, security and licences

- The [Code of Conduct](CODE_OF_CONDUCT.md) applies in every project space.
- Vulnerabilities are reported privately as described in [SECURITY.md](SECURITY.md).
- The licences of each path are in [LICENSE.md](LICENSE.md). The use policy for the project name, `TRADEMARKS.md`, is
  published with the 1.0 release.

## Changing this page

This page changes by a pull request that cites the decision behind it, like any other file.
