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

## How this guide is organised

The guide follows the [Diátaxis](https://diataxis.fr/) structure: four kinds of page, each for a different need.

| Section | Use it when you want to | Style |
|---|---|---|
| [Tutorials](tutorials/index.md) | learn by doing a complete task from start to finish | a guided lesson with a result at the end |
| [How-to guides](how-to/index.md) | solve a specific problem you already understand | short steps for one goal |
| [Reference](reference/index.md) | look up an exact fact: a command, a parameter, a rule | complete and dry; parts are generated from the code |
| [Explanation](explanation/index.md) | understand why EW works the way it does | background and design reasoning |

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
