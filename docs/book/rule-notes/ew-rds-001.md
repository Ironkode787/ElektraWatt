## What it checks

A device whose full designation, as EW derives it, does not parse under the strict reference designation grammar, with
the project's structure codes read as whole tokens. Left out: terminals, unnumbered tags (they still contain "?") and
frozen tags that EW-NUM-001 reports. One finding per device.

## Why

Other programs, labels and exports read a designation by its grammar: prefix signs for the aspects, then letter codes
and numbers. A designation that does not parse is read differently, or not at all, outside EW.

## Standard

- IEC 81346-1:2022, 6 (unverified)

## Example

Import a QElectroTech project with a device labelled "K 1", with a space in the tag.

## How to fix

Unfreezing the tag lets the numbering scheme derive it. There is no quick fix.

## Waiving

Waive it for a customer's designation that must stay as it is, and record the customer's rule in the reason.
