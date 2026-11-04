## What it checks

An asset file, such as an image, whose content does not match the hash in its file name.

## Why

The file name is how records name the asset. A file that does not match it is not the asset the records mean, so a
page would print the wrong picture.

## Standard

No standard: a project-hygiene rule.

## Example

To see it, change one byte of an asset file of a project that holds one and open the project.

## How to fix

Import the asset again, or restore the file from git, and delete the damaged file. There is no quick fix.

## Waiving

An integrity finding cannot be waived: the stored project itself is wrong, and it is repaired, never accepted.
