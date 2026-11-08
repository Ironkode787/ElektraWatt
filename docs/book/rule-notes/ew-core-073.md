## What it checks

A connection drawn on a page between two different points without a stored route. Such a wire is drawn as a dashed
straight line, on the canvas and in every export. One finding per wire, shown on the page.

## Why

An unrouted wire is not finished: its dashed line may cross symbols, texts and other wires, and the checks of the
drawing see no route for it.

## Standard

No standard: a project-hygiene rule.

## Example

Import a project whose source stored no route for a wire, or commit a wire with the Wire tool where no clear route
was found.

## How to fix

Use Route page, or select the wire and use Route selection; or draw its route with Edit route. There is no quick fix.

## Waiving

Waive it only while a page is a draft that is not released.
