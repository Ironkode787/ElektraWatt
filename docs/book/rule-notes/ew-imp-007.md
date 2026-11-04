## What it checks

A value the importer mapped through an alias table: a manufacturer name, a colour code or a structure code written
another way in the source.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out. Where a live rule
reports the same condition on the same records, the report shows the live rule's row, with this code as its origin.

## Why

The migration report is the list of everything an import changed, guessed or left out, so that you can review the
result before you work on in EW. A wrong alias puts a wrong value in the project.

## Standard

No standard: a project-hygiene rule.

## Example

Import a QElectroTech project whose cores use the colour code BR, which maps to BN.

## How to fix

The mapped value is on the record's row in the property grid. There is no quick fix.

## Waiving

Waive the finding once you have reviewed it and accept what the importer did; the waiver records the review. Prefer
fixing where the hint offers a fix.
