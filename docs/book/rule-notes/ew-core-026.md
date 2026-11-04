## What it checks

A strip, cable or PLC station that no report block of its family covers: a terminal diagram for a strip, the cable
overview and diagrams for a cable, a PLC overview for a station. A block covers the devices of its manual selection,
else every device of its family, less its exclusions. One finding per uncovered device: info online, warning in
release mode.

## Why

A delivery is expected to document every strip, cable and station. One that no report covers is missing from the
documentation set.

## Standard

No standard: a project-hygiene rule.

## Example

In a copy of Golden A, exclude a strip from the terminal-diagram block's selection.

## How to fix

A report block's selection is in its properties. There is no quick fix.

## Waiving

Waive it for a device that is documented elsewhere, for example by its manufacturer's own diagram.
