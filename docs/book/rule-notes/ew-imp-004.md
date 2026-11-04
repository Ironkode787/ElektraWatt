## What it checks

Content that the importer leaves out by design because a project has no counterpart for it.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out. Where a live rule
reports the same condition on the same records, the report shows the live rule's row, with this code as its origin.

## Why

The migration report is the list of everything an import changed, guessed or left out, so that you can review the
result before you work on in EW.

## Standard

No standard: a project-hygiene rule.

## Example

Import a QElectroTech project that holds content EW does not model.

## How to fix

Nothing to do; redraw the content if the project needs it.

## Waiving

Waive the finding once you have reviewed it and accept what the importer did; the waiver records the review. Prefer
fixing where the hint offers a fix.
