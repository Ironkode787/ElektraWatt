## What it checks

An object the importer inferred, such as a strip, a cable, a section or a junction that the source draws but does not
declare. The fallback strip that collects a folio's unlabelled terminals is one.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out. Where a live rule
reports the same condition on the same records, the report shows the live rule's row, with this code as its origin.

## Why

The migration report is the list of everything an import changed, guessed or left out, so that you can review the
result before you work on in EW. An inferred object may group things that belong apart.

## Standard

No standard: a project-hygiene rule.

## Example

Import a QElectroTech project whose terminals carry no strip label.

## How to fix

The migration report names the inferred object; select it to review it. There is no quick fix.

## Waiving

Waive the finding once you have reviewed it and accept what the importer did; the waiver records the review. Prefer
fixing where the hint offers a fix.
