## What it checks

Texts of a DXF drawing in an SHX font (an AutoCAD shape font such as `romans.shx` or `txt.shx`). SHX fonts are not
shipped with ElektraWatt, so the texts are drawn in a bundled font instead, through one substitution table.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out.

## Why

The bundled font has other letter widths than the SHX font, so a text can run longer or shorter than it did in the
original drawing and touch a line or another text.

## Standard

No standard: a project-hygiene rule.

## Example

Import a DXF drawing whose text style names `romans.shx`.

## How to fix

Check the texts of the imported page and move or shorten the ones that now touch something. There is no quick fix.

## Waiving

Waive the finding once you have checked the texts; the waiver records the review.
