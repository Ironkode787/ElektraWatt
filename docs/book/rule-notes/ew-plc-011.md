## What it checks

Over the modules of a PLC station: a module in a slot its part does not allow, more modules than the station's part
allows, and more input or output bytes than it allows. The limits come from the parts' PLC sections. Its prevent half
refuses the edit that breaks a limit.

## Why

A station refuses a configuration beyond its slots and I/O lengths.

## Standard

No standard: the limits are the device description's (GSD Max_Module, the maximum I/O lengths).

## Example

Put a module whose part allows slots 1 and 2 into slot 4.

## How to fix

Move the module, or remove modules. There is no quick fix.

## Waiving

Do not waive it: the PLC software refuses the configuration.
