## What it checks

Two halves. Protective-earth (PE) networks in one location that do not meet: one finding per location, naming the
networks. And a terminal joined to the PE rail that has no location, in a project with two or more own locations; with
at most one location, such a terminal belongs to the main location and joins its PE rail.

## Why

Within one location every protective conductor should form one protective bonding circuit. Two PE potentials in one
place mean a PE connection is missing, so some exposed parts are not bonded.

## Standard

- IEC 60204-1:2016, 8.2.1 (unverified)

## Example

In a copy of the sample project, delete the wire from -X1011:PE to -X1031:PE and clear rail bonding on -X1031:PE: the
location `+CAB` then holds two PE networks.

## How to fix

Put the PE terminals of the location on one bonded rail (Rail bonding is on the terminal's row in the strip editor), or
draw a PE wire from each network to the PE rail. There is no quick fix.

## Waiving

Waive it only for a deliberately separate earth, such as a clean earth bar that is joined at one point elsewhere, and
name that point in the reason.
