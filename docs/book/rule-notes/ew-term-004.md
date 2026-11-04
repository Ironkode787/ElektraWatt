## What it checks

A bridge group of a strip that is not valid: its terminals are not neighbours in the strip's mounting order, it runs
over more than one bridge shaft, one of its bridges reaches another strip or a device that is not a terminal, or it
overlaps another group on a shaft they share. One finding per group with every reason. An edit that makes a group
invalid is refused; an invalid group in a file or an import is reported.

## Why

A plug-in bridge joins terminals that sit next to each other on one shaft. A group that skips a terminal or crosses a
strip cannot be built with a bridge and would be wired wrongly.

## Standard

No standard: a project-hygiene rule.

## Example

The window refuses such a bridge. To see the finding, open a project file in which a bridge joins terminals 1 and 3 of
a strip while terminal 2 sits between them.

## How to fix

Use wires for terminals that a bridge cannot join. There is no quick fix.

## Waiving

Waive it only when the bridge is a wire jumper that the documentation shows as a bridge on purpose.
