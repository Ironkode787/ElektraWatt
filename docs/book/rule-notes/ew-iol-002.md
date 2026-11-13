## What it checks

Over the ports of an IO-Link master: a device whose process data is larger than the port window of the master's part,
and a cable at the port longer than the part's maximum (IO-Link: 20 m).

## Why

Process data beyond the window is cut off; a longer cable breaks the communication.

## Standard

No standard: a project-hygiene rule.

## Example

Connect an 88-bit device to a master whose window is 8 bytes.

## How to fix

Use a master with a larger window, or shorten the cable. There is no quick fix.

## Waiving

Do not waive it: the data or the link is lost.
