## What it checks

An item the importer skipped because it was malformed or refused: a number that is not a number, a value out of range,
a node that does not parse.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out. Where a live rule
reports the same condition on the same records, the report shows the live rule's row, with this code as its origin.

## Why

The migration report is the list of everything an import changed, guessed or left out, so that you can review the
result before you work on in EW. A skipped item is missing from the project.

## Standard

No standard: a project-hygiene rule.

## Example

Import a QElectroTech project in which an element's position is not a number.

## How to fix

Import the corrected source again. There is no quick fix.

## Waiving

Waive the finding once you have reviewed it and accept what the importer did; the waiver records the review. Prefer
fixing where the hint offers a fix.
