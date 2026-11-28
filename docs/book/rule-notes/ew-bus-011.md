## What it checks

A part whose own bus symbol does not draw a socket of its network ports, or draws one they no longer have.

## Why

The program never redraws a bus symbol of your own. When the part's network ports change, a socket the symbol lacks
has no place for its bus line, and one it has too many is a socket the device does not have.

## Standard

No standard: a project-hygiene rule.

## Example

Make your own bus symbol of a coupler whose interface X1 has two ports, then give X1 three ports in Parts: the
symbol does not draw X1 P3.

## How to fix

Open the bus symbol (the quick fix) and draw the socket, or take out the one the ports no longer have. **Use the
program's symbol again** on the part's sheet, under **On a bus page**, drops your own symbol.

## Waiving

Do not waive it: a device of the part has a socket no drawing shows.
