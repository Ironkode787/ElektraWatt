## What it checks

Two halves. More than one PE potential in one location: one finding per location, naming the potentials. And a
terminal with rail bonding that has no location, in a project with two or more own locations; with at most one
location, such a terminal belongs to the main location and joins its PE rail.

## Why

Within one location every protective conductor should form one protective bonding circuit. Two PE potentials in one
place mean a PE connection is missing, so some exposed parts are not bonded.

## Standard

- IEC 60204-1:2016, 8.2.1 (unverified)

## Example

Golden A has none: the rails R1 to R3 join every PE connection point. In a copy, clear rail bonding on -X1032: the
location +J-E-2416 then holds two PE potentials.

## How to fix

Rail bonding is on the terminal's row in the strip editor. There is no quick fix.

## Waiving

Waive it only for a deliberately separate earth, such as a clean earth bar that is joined at one point elsewhere, and
name that point in the reason.
