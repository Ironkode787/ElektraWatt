## What it checks

A connection point with the role FE (functional earth) on a PE or PEN potential without an FE–PE bond at it. One
finding per connection point.

## Why

Functional earth serves a device's function, such as EMC filtering, not protection. Joining it to PE is often right,
but it should be a decision, declared as a bond, so that the PE circuit and the functional earth stay traceable.

## Standard

- IEC 60204-1:2016, 8.4 (unverified)

## Example

To see it in a new project, connect a device's FE pin to the PE potential with a plain wire.

## How to fix

Declare an FE–PE bond at the connection point with Draw ▸ Connect ▸ Bond, or connect the pin to a functional-earth
potential.

## Waiving

Waive it when the part's FE pin is a PE pin in all but name and the manufacturer says so; record the source in the
reason.
