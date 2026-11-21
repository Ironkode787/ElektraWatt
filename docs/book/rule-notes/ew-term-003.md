## What it checks

A spare terminal (its state is Spare) that has a wire or a cable core on its strip's field side (side 2 unless the
strip says otherwise). Bridges, bus lines and internal links do not count as wires. One finding per terminal; online
only.

## Why

A spare is a terminal kept free for later use. Field wiring on it means either the terminal is in use and its state is
wrong, or the wire is drawn to the wrong terminal. A spare may be fed on its inner side.

## Standard

No standard: a project-hygiene rule.

## Example

In a copy of the sample project, set a terminal of -X1021 to Spare and draw a wire to it on the field side.

## How to fix

The terminal's state is on its row in the strip editor. There is no quick fix.

## Waiving

Waive it when the field wire is a deliberate reserve, for example a cable core parked on a spare; say so in the
waiver's reason.
