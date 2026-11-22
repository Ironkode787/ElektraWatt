## What it checks

A wire whose cross-section was set by hand below the size the rating of its protecting device needs: the device that
protects the wire's potential (for a PE wire, the highest rating at its two ends), its rated current, and the current
table's least cross-section that carries it, the size the program proposes for a wire set by nothing. A size below the
table's first row (0.5 mm²) is reported where the rating is above what that row carries. A cable core takes its
cable's size and a bond wire no rating, so neither is checked. One finding per wire, online and at release.

## Why

A wire smaller than its protection allows can overheat before the device trips. The proposal from the rating is only
a proposal: a size set by hand wins over it, so nothing else warns when that size is too small.

## Standard

- IEC 60204-1:2016, Table 6 (installation method B2, PVC-insulated copper, 40 °C ambient, no derating)

## Example

On the sample's page 102 the wire from the C16 breaker -102F1 to -X1021:1 takes 2.5 mm². Set it to 0.5 mm² or
1.5 mm²: the rule reports it, naming the breaker, its 16 A and the proposed 2.5 mm².

## How to fix

Choose the quick fix "Take the proposed size": it removes the size set by hand where the wire takes the proposal
without it, else it sets the proposed size. A larger size, or a device of a lower rating, takes the finding away too.

## Waiving

This is a safety rule. Waive it only where the wire is protected otherwise or carries less by design (a derated
circuit, a separately fused branch); record why in the reason.
