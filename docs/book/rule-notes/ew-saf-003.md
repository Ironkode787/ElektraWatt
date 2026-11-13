## What it checks

An F-DI channel whose safety data names the module's test-pulse pin, while its sensor has no connection to that pin's
signal.

## Why

The module detects cross-circuits only when the sensor is fed from its test pulse.

## Standard

No standard: a project-hygiene rule.

## Example

Feed an emergency-stop button of an F-DI channel from 24 V instead of the test-pulse pin.

## How to fix

Wire the sensor's supply to the module's test-pulse pin. There is no quick fix.

## Waiving

Waive it only where the module does not need the test pulse, with the source.
