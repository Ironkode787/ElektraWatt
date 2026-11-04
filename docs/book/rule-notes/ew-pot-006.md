## What it checks

A protective-earth wire whose cross-section is below what the PE sizing table asks for its associated line conductors:
at a terminal end, the line wires on the same side of the same strip; at a device end, the line wires on the device's
connection points. The largest line cross-section S gives the minimum: equal to S up to 16 mm², then 16 mm² up to 35
mm², then S/2. Line types are L, L1, L2 and L3. Cross-sections in AWG or kcmil are not compared. One finding per PE
wire, at the end with the largest requirement. The sizing table ships unverified, and every finding says so.

## Why

A protective conductor must carry the fault current until the protective device trips. An undersized PE conductor can
overheat or open before that, and then exposed parts stay live.

## Standard

- IEC 60204-1:2016, 8.2.2 (unverified; the sizing table ships with verified = false)
- IEC 60364-5-54:2011, 543.1.2 (unverified)

## Example

Golden A's W6 is 1.5 mm² at 1X1, next to the 2.5 mm² line wire W4.

## How to fix

Check the result against the standard yourself: the sizing table is not verified yet. There is no quick fix.

## Waiving

This is a safety rule. Waive it only after a sizing calculation, for example by the adiabatic method, shows the
smaller cross-section is enough; record the calculation in the reason.
