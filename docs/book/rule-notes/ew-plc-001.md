## What it checks

Channel addresses of one PLC configuration project whose bit ranges overlap in one area, such as IW0 against I1.3: one
finding per group of overlapping channels, naming up to five with their addresses. Also a word or longer address at a
byte that is not a multiple of the profile's word alignment, as a warning. Automatic addresses never overlap, so a
finding involves a manual address or a part map.

## Why

Two channels on one address read or write the same bits: the program cannot tell the signals apart, and an output may
drive the wrong load.

## Standard

- IEC 61131-3:2013, 6.5.5 (unverified)

## Example

In a new project, give two channels of one configuration project the manual address I0.0.

## How to fix

When a group's members change, a waiver of the group no longer matches. There is no quick fix.

## Waiving

Waive it only for channels that share an address on purpose, such as a word and its bits read twice.
