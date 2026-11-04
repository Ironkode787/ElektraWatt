## What it checks

Junctions of one potential that disagree on which arm feeds them: the arm between two junctions feeds both or neither,
or a source feeds a star that has no hub. One finding per such point; online only.

## Why

EW turns the drawn junctions into physical wires from the feed outward. When the feed is ambiguous, the wire list
cannot tell which terminal gets which wire.

## Standard

No standard: a project-hygiene rule.

## Example

In a new project, draw two junctions on one wire and feed the line from both sides.

## How to fix

The wiring order (Chain or Star) is on the junction's row in the property grid. There is no quick fix.

## Waiving

Waive it only when the derived wires are what you want and the drawing cannot be made unambiguous.
