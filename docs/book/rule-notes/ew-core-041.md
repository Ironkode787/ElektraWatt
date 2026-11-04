## What it checks

A device without a part. Left out: a strip (its terminals carry the parts), a cable (its length and core rules report
a missing cable part), a PLC configuration device and a device without functions. One finding per device; its
fingerprint holds only the device, so a waiver survives a renumber. A device supplied by others is still reported,
because the attribute that would exempt it is not built yet.

## Why

A device without a part cannot be ordered and is missing from the parts list.

## Standard

No standard: a project-hygiene rule.

## Example

On Golden A, S56 and E56 have no part.

## How to fix

Choose the part on the Parts row of the property grid; see the how-to on choosing parts. There is no quick fix.

## Waiving

Waive it for a device that others supply, until EW can mark it so; Golden A's -XPE is waived this way.
