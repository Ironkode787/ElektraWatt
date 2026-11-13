## What it checks

A PROFIBUS station address outside the range of the port's part, else outside 0 to 125. The range is unverified data,
so the rule reports a warning.

## Why

A station address outside the range is not reachable on the bus.

## Standard

No standard: a project-hygiene rule.

## Example

Give a DP port the station address 126.

## How to fix

Enter an address inside the range on the port's row. There is no quick fix.

## Waiving

Waive it when the part's range is wider than the data says, with the source.
