# Introduction

ElektraWatt (EW) is an open-source, cross-platform desktop application for designing the electrical schematics of
control cabinets. This guide is its user documentation.

> **Status.** EW is working toward its first release, the pilot release 0.1.0 for Windows and Linux. Two programs
> exist today: the desktop application, `elektrawatt`, which opens, edits, checks and prints a project, with
> navigators, table mode, a history and the 2D panel layout; and the command-line program, `ew`, which works on a
> project from the terminal ("headless"): it imports QElectroTech projects, checks, exports PDF/A-2b, SVG and tables,
> and prints. This guide grows with the product; where a page describes something that is not built, it says so. The
> milestones and where each stands are in the repository's
> [roadmap](https://github.com/Ironkode787/ElektraWatt/blob/main/ROADMAP.md).

## Checks and engineering verification

ElektraWatt's checks do not replace engineering verification by a competent person.

## How this guide is organised

The guide follows the [Diátaxis](https://diataxis.fr/) structure: four kinds of page, each for a different need.

| Section | Use it when you want to | Style |
|---|---|---|
| [Tutorials](tutorials/index.md) | learn by doing a complete task from start to finish | a guided lesson with a result at the end |
| [How-to guides](how-to/index.md) | solve a specific problem you already understand | short steps for one goal |
| [Reference](reference/index.md) | look up an exact fact: a command, a parameter, a rule | complete and dry; parts are generated from the code |
| [Explanation](explanation/index.md) | understand why EW works the way it does | background and design reasoning |

## Where to start

- **New to EW:** the tutorial [A first schematic page with potentials](tutorials/first-page.md), then
  [A terminal strip and a cable](tutorials/terminal-strip-and-cable.md) and
  [Reports and PDF](tutorials/reports-and-pdf.md). If you come from QElectroTech,
  [Importing a QElectroTech project](tutorials/import-qet.md) brings a project of yours over.
- **A task at hand:** the how-to guides, among them [Choose parts](how-to/choose-parts.md) and
  [Upgrade a project](how-to/upgrade-a-project.md).
- **A fact to look up:** the [rule reference](reference/rules/index.md), with a page per check rule; the window's
  [tools](reference/window/tools.md), [dialogs](reference/window/dialogs.md) and [panels](reference/window/panels.md);
  and the [release notes](reference/release-notes/index.md) of every release.
- **Coming from another program:** [EPLAN and EW](explanation/eplan-and-ew.md) and
  [QElectroTech and EW](explanation/qet-and-ew.md) relate the concepts you know to EW's.

## What the guide does not contain

The design documents, the architecture decision records and the work plan stay in the repository, next to the code
they describe. This guide links to them; it does not copy them. Start from the
[documentation map](https://github.com/Ironkode787/ElektraWatt/blob/main/docs/README.md).

## Offline use

The guide builds without network access and is meant to be bundled with the application, so that the help works
without a connection. Pages that link to the repository need a connection to follow those links.

## Sample data

Every example uses the anonymised golden project (`golden/J-E-2416`, licence CC0-1.0). It holds placeholders where a
real project would name a customer, an end user or a person, such as `<customer>` and `<drawing id>`. Do not put real
customer data into examples that you contribute to this guide.
