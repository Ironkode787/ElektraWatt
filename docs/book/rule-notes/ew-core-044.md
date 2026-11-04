## What it checks

A value of a device still marked TBD: a PLC module's input or output start, a channel address, or a bus port's station
address. A module inside a PLC configuration project reports through EW-PLC-015 and a port of a logical net through
EW-BUS-001, which replace this finding. One finding per value: info online, error in release mode.

## Why

TBD marks a value that is not decided yet. It may stay open while drawing, but the released documentation must state
it.

## Standard

No standard: a project-hygiene rule.

## Example

In a new project, set a PLC module that belongs to no configuration project to a TBD start address.

## How to fix

The value is on the module's or the port's row in the property grid. There is no quick fix.

## Waiving

Do not waive it for a release; decide the value.
