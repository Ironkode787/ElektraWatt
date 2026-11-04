## What it checks

A value the importer could not map and kept as foreign or opaque text, such as a colour code it does not know.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out. Where a live rule
reports the same condition on the same records, the report shows the live rule's row, with this code as its origin.

## Why

The migration report is the list of everything an import changed, guessed or left out, so that you can review the
result before you work on in EW. A foreign value is kept but not understood: rules and reports cannot use it.

## Standard

No standard: a project-hygiene rule.

## Example

Import a QElectroTech project whose cores use a colour code that no alias table knows.

## How to fix

Where the importer knows a mapping, the finding carries it as its quick fix.

## Waiving

Waive the finding once you have reviewed it and accept what the importer did; the waiver records the review. Prefer
fixing where the hint offers a fix.
