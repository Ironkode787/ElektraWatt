## What it checks

A wire or cable core whose colour is exactly the code BU (blue) on a potential that is not N, in a project that has an
N potential. A company code such as DBU (dark blue) is not judged, and neither is the colour a wire takes from the
program's built-in table when the project chose none. One finding per wire.

## Why

Light blue marks the neutral. Where a project has a neutral, a blue conductor that is not the neutral can be mistaken
for it.

## Standard

- IEC 60204-1:2016, 13.2.3

## Example

In a copy of the sample project, give a 24 V wire the colour BU in the property grid.

## How to fix

For a DC control circuit, a company colour such as DBU keeps the blue look without the code BU. There is no quick fix.

## Waiving

Waive it for a cable whose core colours are fixed by the manufacturer and the documentation marks the core's use.
