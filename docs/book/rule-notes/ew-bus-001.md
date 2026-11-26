## What it checks

Over the ports of a logical bus net: a station address still marked TBD (an open item that blocks release), and a
PROFIBUS address that two or more ports share. A port without an address is not checked. One finding per open address
and per shared address; an error in both modes.

## Why

Every station on a bus needs its own address. A missing or shared address stops the bus from starting.

## Standard

No standard: a project-hygiene rule.

## Example

In a copy of the sample project, set the PROFIBUS address of the valve terminal -201K1's port on page 105 to TBD, or to
the address of the CPU -104A1's port.

## How to fix

Set the address on the port's row in the property grid. For an open PROFIBUS address the quick fix "Give it the next
free address" sets the first station address after the highest one the net holds that no port has (from 1 when the
net holds none; 0 stays the programming device's), within the range the port's part states. Two open ports are given
two addresses. A port whose range is full, or whose part states a range that holds no station address (its lowest
above its highest, or above 255), gets no quick fix: the finding stays, and the address is set by hand. Other buses have
no quick fix.

## Waiving

Do not waive it for a release; decide the address.
