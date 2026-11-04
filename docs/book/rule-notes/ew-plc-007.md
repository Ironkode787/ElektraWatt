## What it checks

A symbol that two or more channels of one PLC configuration project carry; symbols compare exactly. One finding per
symbol, naming its channels.

## Why

A symbol names one signal in the PLC program. Two channels with one symbol cannot both be exported to the PLC's tag
table.

## Standard

- IEC 61131-3:2013, 6.1.2 (unverified)

## Example

In a new project, give two channels the symbol Pump_On.

## How to fix

The symbol is on the channel's row. There is no quick fix.

## Waiving

Do not waive it: the tag table cannot hold both.
