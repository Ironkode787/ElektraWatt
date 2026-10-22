# Introduction

ElektraWatt (EW) is an open-source, cross-platform desktop application for designing the electrical schematics of
control cabinets. This guide is its user documentation.

> **Status.** EW is in its foundations phase. What exists today is the project model, the command layer, the check
> engine and a command-line program, `ew`, that works on a project from the terminal ("headless"). The desktop
> application is not built yet, so this guide documents what you can do with `ew` now and grows with the product.
> Where a page describes something that is not built, it says so.

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
