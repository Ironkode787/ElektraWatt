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
- [CLI reference](cli/index.md): every subcommand of `ew`, with its flags, exit codes and `--json` result schema,
  from the CLI table.
- [Query reference](queries.md): every public query of `ew query`, with its key, example and output, from the query
  registry.
- [api_v1 reference](api-v1.md): every `api_v1` view of `ew sql`, with its columns, types and Preview marks.
- [Format reference](format/index.md): every record kind of the project format, with its file, rank and fields,
  from the format's JSON Schemas.

## Command-line program

The usage text of `ew` is part of the program: `ew --help` lists the subcommands and their options, and
`ew sql schema` prints the `api_v1` views that `ew sql` queries. The authoritative design catalogue, with the exit
codes of every subcommand, is
[doc 17 §3](https://github.com/Ironkode787/ElektraWatt/blob/main/docs/design/17-extensibility.md#3-the-ew-cli).

## Not generated yet

The documentation of the plugin interface (WIT, 1.x) is also meant to be generated (NFR-DOC-06). It is not part of
the book yet.
