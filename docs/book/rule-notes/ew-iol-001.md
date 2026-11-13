## What it checks

Over the ports of an IO-Link master: a class-B device on a class-A port, and an IO-Link device on a port that is not in
IO-Link mode.

## Why

A class-B device needs the port's second supply; a port in DI or DQ mode does not talk IO-Link.

## Standard

No standard: a project-hygiene rule.

## Example

Connect a class-B valve terminal to a class-A port.

## How to fix

Move the device to a class-B port, or set the port's mode to IO-Link. There is no quick fix.

## Waiving

Do not waive it: the device does not run.
