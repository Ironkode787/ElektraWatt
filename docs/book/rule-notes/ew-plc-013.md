## What it checks

Over the devices of a PLC station: a pin with the role FE on a PE potential without an FE–PE bond, and an L+ or M pin of
a supply group of the part that is not connected or lies on a potential of another type. It supersedes EW-POT-003 for
these pins.

## Why

An FE pin on PE carries fault current into the electronics' earth; an unconnected or swapped supply stops the card.

## Standard

- IEC 60204-1:2016, 8.4 (unverified)

## Example

On Golden A, the earth screws -201U1:3 and -202U1:3 are wired to PE.

## How to fix

Connect the pin to functional earth, or declare an FE–PE bond; feed each supply pin from its role. There is no quick fix.

## Waiving

Waive it only where the manufacturer allows FE on PE, with the source.
