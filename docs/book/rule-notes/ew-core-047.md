## What it checks

A device with functions none of which is drawn on any page in the schematic: every representation counts except panel
fronts, contact images and strip segments, and a graphic counts. Terminals, strips and devices without functions are
left out. A device with a part notes that the parts list still counts it. A warning online and an error in release
mode.

## Why

A device that the schematic does not show is either left over from a deletion or still to be drawn. Either way the
parts list counts something nobody can find on a page.

## Standard

No standard: a project-hygiene rule.

## Example

In a new project, place a device and delete its only symbol while the device keeps another role.

## How to fix

Place the device from the device navigator, or delete it there. There is no quick fix.

## Waiving

Do not waive it for a release; the device must be drawn or removed.
