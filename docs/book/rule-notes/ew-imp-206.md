## What it checks

A column of an EPLAN report export (a device, terminal or connection list) that the report's column mapping does not
name. Its values were not imported.

It is an import finding: the importer recorded it in the project's import run, and it appears in that import's
migration report, which the `migration` and `full` schemes show; `pre-delivery` leaves it out.

## Why

EPLAN's report schemes are the customer's own, so a column the mapping does not know may hold data the project needs.

## Standard

No standard: a project-hygiene rule.

## Example

A terminal diagram with a column "Remark" and a mapping of strip, terminal and level only.

## How to fix

Map the column in the column mapper and import the report again; a newer export of the same list updates the project
in place. There is no quick fix.

## Waiving

Waive the finding when the column is of no use to the project; the waiver records the decision.
