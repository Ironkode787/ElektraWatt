## What it checks

A wire end's marker whose text does not fit the wire marker of the workshop stock, its characters per line or its
lines, after the marker is shortened (the product sign dropped, a long line broken after its last `:`, `-` or `+`). One
finding per end.

## Why

A marker that does not fit is printed small, cut off by the label program, or not at all; a cut marker names a wrong
place. The program never cuts a character itself.

## Standard

No standard: a project-hygiene rule.

## Example

Keep a wire marker of one line of 10 characters in the stock: the marker `+CAB-104A1-A2:L+` at a field end does not
fit.

## How to fix

Choose a marker part of the stock with longer or more lines, set the end's marker by hand, or mark the wires by their
numbers (Project properties ▸ Workshop ▸ Marker text). There is no quick fix.

## Waiving

Waive it where the label program prints the text on a larger marker by itself.
