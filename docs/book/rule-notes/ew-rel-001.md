## What it checks

The four-eyes check of the release gate: no checker set, the checker is the author, or the sign-off is missing or does
not match the saved content. It runs only in the gate.

## Why

A released revision is checked by someone other than its author, and the sign-off binds the exact content that was
checked. Any edit after the sign-off needs a new one.

## Standard

No standard: a project-hygiene rule.

## Example

On Golden A the checker is not set.

## How to fix

The actions Set revision author… and Approve revision… do it. There is no quick fix.

## Waiving

Release-gate findings cannot be waived.
