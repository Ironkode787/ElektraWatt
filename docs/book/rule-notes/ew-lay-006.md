## What it checks

Two footprints that overlap with a positive area on one surface: two placements, a placement and a duct, or a rail and
a duct. One finding per pair, at most 64 per surface, then one summary finding. Touching edges do not overlap. This is
the 2D check; the 3D check comes later.

It also checks the closed door, where the enclosure part gives the room between the plate and the door (its usable
depth): a device or a duct of the plate that reaches further than that room, and a device of the plate and one of the
door that stand in front of each other when the door is closed and together take more than the room. A device without
a depth is not judged.

## Why

Two devices cannot occupy the same space on the plate, and the door must close over what stands on it.

## Standard

No standard: a project-hygiene rule.

## Example

In a new project with a layout, place two devices on one rail at the same position.

## How to fix

Move the placement on its rail or the plate with the move tool; for the closed door, move the device on the plate or in
the door so that they no longer stand in front of each other, or choose a part that is less deep. There is no quick
fix.

## Waiving

Waive it only when one footprint is larger than the real device and the devices fit.
