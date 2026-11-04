## What it checks

On each rail, the placed members of an assembly or PLC station (the parent device and its children) out of their order
(the parent first, then by position), or with another device placed between two members. Unplaced members are skipped.
One finding per group and rail.

## Why

A station's modules sit side by side in the order of their positions; the bus behind them depends on it. A gap or a
wrong order cannot be built.

## Standard

No standard: a project-hygiene rule.

## Example

In a new project, place a PLC station's second module before its first on the rail.

## How to fix

Move the placements along the rail with the move tool. There is no quick fix.

## Waiving

Waive it for an assembly whose members may sit apart, such as separate power modules.
