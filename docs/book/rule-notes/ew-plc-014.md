## What it checks

The load current of the used outputs of a module against the ratings of its part: one output, a load supply group and the
whole module. A load comes from the target device's technical data: i_load, else power at voltage (24 V when none).
It runs in the offline and release modes.

## Why

An output above its rating trips its protection or ages the card.

## Standard

No standard: a project-hygiene rule.

## Example

Give a module part the channel rating 500 mA and drive a 1 A coil from it.

## How to fix

Spread the loads, or use a stronger output or module. There is no quick fix.

## Waiving

Waive it when the loads never switch together, with the reason.
