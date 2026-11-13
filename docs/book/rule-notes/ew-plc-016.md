## What it checks

A PLC configuration project that differs from its last exchange with the PLC software: a station, a module, a
channel, a tag or a bus node with another value than the exported or imported file had, one added since, or one that
is gone. One finding per configuration project; it names the exchange and its date, counts the differing objects and
names the first three.

## Why

The PLC software works from the file it was given. A symbol renamed or a module added afterwards is not in the PLC
program until the configuration project is exported again.

## Standard

No standard: a project-hygiene rule.

## Example

Export a configuration project with `ew export plc-aml`, then rename the symbol of a channel.

## How to fix

Export the configuration project again, or import the file of the PLC software; `ew plc status` lists every
differing object with its fields. There is no quick fix.

## Waiving

Waive it while the PLC side is not due for an update, with the reason.
