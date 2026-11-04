## What it checks

Spelling variants of one structure code in the source, such as +JE2416 and +J-E-2416. The live rule is EW-RDS-010.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out. Where a live rule
reports the same condition on the same records, the report shows the live rule's row, with this code as its origin.

## Why

The migration report is the list of everything an import changed, guessed or left out, so that you can review the
result before you work on in EW. Two spellings split one location in two.

## Standard

No standard: a project-hygiene rule.

## Example

Import a QElectroTech project that writes one location in two ways.

## How to fix

Move the devices and pages of one node to the other, then delete the empty node. There is no quick fix.

## Waiving

Waive the finding once you have reviewed it and accept what the importer did; the waiver records the review. Prefer
fixing where the hint offers a fix.
