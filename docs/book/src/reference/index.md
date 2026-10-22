# Reference

The reference states exact facts about EW and nothing else: what a command takes, what a rule checks, what a program
prints. Look here when you know what you are doing and need the detail. To learn, use a
[tutorial](../tutorials/index.md); to understand the design, read the [explanation](../explanation/index.md).

## Generated reference

These pages are generated from the code by `cargo xtask docs-gen` and checked in CI, so they cannot fall behind it
(NFR-DOC-06). Do not edit them by hand.

- [Command reference](commands/index.md): every registered command, with its parameters, types, constraints and exit
  codes, from the parameter schemas of the command registry.
- [Rule reference](rules/index.md): every shipped check rule, one page per rule pack, from the rule registry.

## Command-line program

The usage text of `ew` is part of the program: `ew --help` lists the subcommands and their options, and
`ew sql schema` prints the `api_v1` views that `ew sql` queries. The authoritative design catalogue, with the exit
codes of every subcommand, is
[doc 17 §3](https://github.com/Ironkode787/ElektraWatt/blob/main/docs/design/17-extensibility.md#3-the-ew-cli).

## Not generated yet

The file-format specification with its JSON Schemas, the `api_v1` view reference, the CLI reference and the keymap
sheets are also meant to be generated (NFR-DOC-06). They are not part of the book yet.
