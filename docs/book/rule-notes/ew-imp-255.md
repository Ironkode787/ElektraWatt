## What it checks

Hatches of a DXF drawing with a pattern (lines, crosses, bricks). A solid hatch is imported as a filled area; a
patterned one keeps only its outline. The finding gives each pattern with its count.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out.

## Why

The pattern itself is not drawn, so an area that was shaded in the original is empty on the page.

## Standard

No standard: a project-hygiene rule.

## Example

Import a DXF drawing with an `ANSI31` hatch.

## How to fix

Where the shading matters, draw it on the page. There is no quick fix.

## Waiving

Waive the finding once you have checked the page; the waiver records the review.
