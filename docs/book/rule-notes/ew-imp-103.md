## What it checks

A label that does not fit the numbering rule the importer inferred, kept as a frozen tag.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out. Where a live rule
reports the same condition on the same records, the report shows the live rule's row, with this code as its origin.

## Why

The migration report is the list of everything an import changed, guessed or left out, so that you can review the
result before you work on in EW. A frozen tag stays as it is when the project renumbers.

## Standard

No standard: a project-hygiene rule.

## Example

Import a QElectroTech project with one hand-written label among formula labels.

## How to fix

Unfreeze the tag with the quick fix in the Messages pane, or in the property grid, to take the scheme's value.

## Waiving

Waive the finding once you have reviewed it and accept what the importer did; the waiver records the review. Prefer
fixing where the hint offers a fix.
