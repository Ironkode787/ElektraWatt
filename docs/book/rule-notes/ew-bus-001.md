## What it checks

Over the ports of a logical bus net: a station address still marked TBD (an open item that blocks release), and a
PROFIBUS address that two or more ports share. A port without an address is not checked. One finding per open address
and per shared address; an error in both modes.

## Why

Every station on a bus needs its own address. A missing or shared address stops the bus from starting.

## Standard

No standard: a project-hygiene rule.

## Example

On Golden A, the DP station address of -201U1 is TBD.

## How to fix

Set the address on the port's row in the property grid. There is no quick fix.

## Waiving

Do not waive it for a release; decide the address.
