## What it checks

Entities of a DXF drawing that the import does not read: 3D faces and solids, regions, ACIS bodies, proxy objects of
other programs and every other type outside the drawing entities a page holds. The finding gives each type with its
count.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out.

## Why

What these entities showed is missing from the imported page, so the page may be incomplete.

## Standard

No standard: a project-hygiene rule.

## Example

Import a DXF drawing that holds a `3DSOLID`.

## How to fix

Compare the page with the original drawing and draw what is missing, or export the drawing as 2D lines from the
program that made it and import that. There is no quick fix.

## Waiving

Waive the finding once you have compared the page; the waiver records the review.
