## What it checks

A stored manual address whose bit index is not below the profile's unit width, such as I4.9 in an 8-bit unit, on a
channel or a module start. The window refuses such an address; a file or an import can hold one. Without a profile the
width is 8 bits.

## Why

A bit index beyond the unit's width does not exist on the PLC.

## Standard

- IEC 61131-3:2013, 6.5.5 (unverified)

## Example

Open a project file in which a channel's manual address is I4.9.

## How to fix

Enter the address again in the property grid; the window accepts only valid bits. There is no quick fix.

## Waiving

Do not waive it: the address does not exist.
