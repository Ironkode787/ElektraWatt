## What it checks

A duct whose fullest piece, with the ways Route stored, is over the project's duct fill limit (Project properties ▸
Workshop, 60 % unless set). A wire's area comes from its wire part's outer diameter, else from a table of default
diameters by cross-section; the duct's inner area from its part, else 85 % of its nominal size.

## Why

A duct packed past its fill takes no more wires and lets no heat out; its cover does not close.

## Standard

No standard: a project-hygiene rule.

## Example

Set the duct fill limit to 1 % and run Route on the panel page.

## How to fix

Choose a wider duct, add a second one beside it and run Route again, or raise the fill limit where the shop allows
it. There is no quick fix.

## Waiving

Waive it when the duct's real inner size is larger than its part says.
