## What it checks

A module start or channel address of a module in a PLC configuration project that is still TBD. One finding per open
value: info online, error in release mode, an open item that blocks release. It replaces EW-CORE-044's finding for the
same value.

## Why

TBD marks an address that is not decided yet. The released documentation and the PLC export must state it.

## Standard

No standard: a project-hygiene rule.

## Example

In a new project, set a module's start address to TBD.

## How to fix

The address is on the module's or the channel's row. There is no quick fix.

## Waiving

Do not waive it for a release; decide the address.
