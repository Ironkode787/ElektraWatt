## What it checks

A per-folio description kept as the page's own description override. The live rule is EW-CORE-023.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out. Where a live rule
reports the same condition on the same records, the report shows the live rule's row, with this code as its origin.

## Why

The migration report is the list of everything an import changed, guessed or left out, so that you can review the
result before you work on in EW. The page prints its own description instead of the project's.

## Standard

No standard: a project-hygiene rule.

## Example

Import a QElectroTech project whose folios have their own titles.

## How to fix

EW-CORE-023's quick fix "drop override" removes the override.

## Waiving

Waive the finding once you have reviewed it and accept what the importer did; the waiver records the review. Prefer
fixing where the hint offers a fix.
