## What it checks

A DXF drawing whose header names no drawing unit (`$INSUNITS` absent or "unitless"): the import asked for the unit
and read the drawing in the one you chose.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out.

## Why

A coordinate of 100 is 100 mm, 100 inches or 100 metres depending on the unit. A wrong choice makes the drawing far
too large or too small on its page.

## Standard

No standard: a project-hygiene rule.

## Example

Import a DXF drawing whose header has no `$INSUNITS` and choose millimetres.

## How to fix

Check the drawing's size on its page. If it is wrong, delete the page and import the drawing again in the right
unit. There is no quick fix.

## Waiving

Waive the finding once you have checked the size; the waiver records the review.
