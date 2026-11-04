## What it checks

A channel address outside its module's input or output range: another area, or bytes outside the module's start plus
the width of the part's range in that direction. Only a module whose start is an address is checked. One finding per
channel.

## Why

A channel addressed outside its module reads or writes another module's bits.

## Standard

- IEC 61131-3:2013, 6.5.5 (unverified)

## Example

In a new project, give a channel of an 8-bit output module at Q0 the manual address Q4.0.

## How to fix

The module's start is in its properties. There is no quick fix.

## Waiving

Do not waive it: the address belongs to another module.
