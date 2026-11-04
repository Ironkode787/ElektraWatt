## What it checks

At a re-import: a record that the source no longer holds, kept here because you edited it in EW since the last import.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out. Where a live rule
reports the same condition on the same records, the report shows the live rule's row, with this code as its origin.

## Why

The migration report is the list of everything an import changed, guessed or left out, so that you can review the
result before you work on in EW. Your edit is kept, but the source has moved on.

## Standard

No standard: a project-hygiene rule.

## Example

Re-import a QElectroTech project after deleting an element there that you edited in EW.

## How to fix

Select the record from the migration report to review it. There is no quick fix.

## Waiving

Waive the finding once you have reviewed it and accept what the importer did; the waiver records the review. Prefer
fixing where the hint offers a fix.
