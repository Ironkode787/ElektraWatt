## What it checks

The two ends of a QElectroTech folio report whose names differ, or a pairing that needs chain mode.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out. Where a live rule
reports the same condition on the same records, the report shows the live rule's row, with this code as its origin.

## Why

The migration report is the list of everything an import changed, guessed or left out, so that you can review the
result before you work on in EW. EW pairs interruption points by name.

## Standard

No standard: a project-hygiene rule.

## Example

Import a QElectroTech project in which a folio report and its partner carry different labels.

## How to fix

The name and the pairing mode are on the interruption point's row in the property grid. There is no quick fix.

## Waiving

Waive the finding once you have reviewed it and accept what the importer did; the waiver records the review. Prefer
fixing where the hint offers a fix.
