## What it checks

An F-module without an F-destination address, with one outside the range of its device description, or with one that
another F-module of the project uses.

## Why

PROFIsafe addresses a safety module by its F-destination address; two modules with one address stop the safety program.

## Standard

- IEC 61784-3-3 (unverified)

## Example

Give two F-modules the F-destination address 100.

## How to fix

Enter a unique address inside the range on the module's row. There is no quick fix.

## Waiving

Do not waive it: the safety program cannot address the modules.
