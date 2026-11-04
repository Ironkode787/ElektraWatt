## What it checks

A label stored in the source that differs from the result of the source's own label formula. The derived tag wins.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out. Where a live rule
reports the same condition on the same records, the report shows the live rule's row, with this code as its origin.

## Why

The migration report is the list of everything an import changed, guessed or left out, so that you can review the
result before you work on in EW. QElectroTech can store a label that its formula would no longer give, after a
renumber.

## Standard

No standard: a project-hygiene rule.

## Example

Import a QElectroTech project in which an element's stored label is stale against its formula.

## How to fix

To keep the stored label, freeze the tag in the property grid. There is no quick fix.

## Waiving

Waive the finding once you have reviewed it and accept what the importer did; the waiver records the review. Prefer
fixing where the hint offers a fix.
