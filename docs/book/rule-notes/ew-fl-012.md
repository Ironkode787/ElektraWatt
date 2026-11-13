## What it checks

In one fluid net: two ports of one valve joined, or a working port of a valve joined to a supply or exhaust port of
another device with no valve between them. One finding per conflict.

## Why

Joining port 2 to port 4 of a valve short-circuits the actuator; a working port on another device's supply port feeds
it backwards.

## Standard

No standard: a project-hygiene rule.

## Example

Draw a line from -201U3-Q1:2 to -201U3-Q1:4.

## How to fix

Draw each working line to a consumer. There is no quick fix.

## Waiving

Waive it for a deliberate cascade where one valve feeds another through its working port.
