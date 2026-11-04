## What it checks

The descriptions of the project's parts, checked for ratings and unit formatting. A rating is a number with a rating
unit, such as "10 A" or "24 V"; it belongs in the technical data. Unit formatting covers a number glued to its unit
("10A"), a unit in the wrong case ("50 hz") and a space inside a unit ("0.75 mm ²"); "24 VDC" and "230 VAC" are
accepted. Manufacturer spelling is not checked. One finding per part and kind of problem. A rule of the pack's 1.0
content, so it runs only where a scheme includes that content.

## Why

A description is a noun phrase that the parts list prints. Ratings in it drift from the typed data, and badly written
units are hard to search.

## Standard

No standard: a project-hygiene rule.

## Example

In a new project, use a part whose description reads "Contactor 24V 10A".

## How to fix

Write units with a space and in their proper case, such as "10 A" and "50 Hz". There is no quick fix yet.

## Waiving

Waive it for a description that quotes a manufacturer's type designation containing a rating.
