## What it checks

A wire override (a value set on the wire itself) that equals the value the wire would take without it: its
cross-section, colour, wire part, insulation, function, shield or twist. One finding per wire and member; online only.

## Why

An override that changes nothing hides the project's real defaults: when the potential or the profile changes later,
this wire keeps the old value.

## Standard

No standard: a project-hygiene rule.

## Example

Golden A has none. In a new project, set a wire's cross-section to the value its potential already gives.

## How to fix

Clear the value on the wire's row in the property grid. There is no quick fix.

## Waiving

Waive it when the wire must keep its value even if the default changes later.
