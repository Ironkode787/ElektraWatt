# A first headless run on the golden project

In this tutorial you use the `ew` command-line program on the golden project: you check the project, write its pages
as PDF and SVG, export the cable list and the bill of materials, and ask the project questions in SQL. Nothing needs
the desktop application, and nothing leaves your machine: `ew` never uses the network.

The golden project, `J-E-2416`, is an anonymised control-cabinet project that EW uses as its acceptance target. Where a
real project names a customer or a person, it holds a placeholder such as `<customer>`. You only read it here; none of
the steps changes it.

## Before you start

You need:

- a checkout of the ElektraWatt repository, which holds the project in `golden/`;
- the `ew` program, built from the checkout with `cargo build --release -p ew`. It is then
  `target/release/ew` (`target\release\ew.exe` on Windows). The examples write plain `ew`; put the program on your
  `PATH` or type its path.

Run every command from the root of the checkout. Check that `ew` works:

```console
$ ew --version
ew 0.0.0
```

Then make a directory for the results:

```console
$ mkdir out
```

## Where the project comes from

The folder `golden/J-E-2416/tables/` holds the project as plain CSV tables: one file per kind of record (devices,
terminals, connections, pages and so on). The importer of the `ew-import-tables` crate turns these tables into
project records, and the checked-in project `golden/J-E-2416.ew` is generated from the same tables. The
[golden README](https://github.com/Ironkode787/ElektraWatt/blob/main/golden/J-E-2416/README.md) describes the tables
and their format.

> **Not built yet.** The importer is a library today. The `ew import csv` subcommand that will run it from the terminal
> does not exist yet, so this tutorial starts from the checked-in project. When the subcommand exists, this step will
> become a command line.

A project is a directory, here `golden/J-E-2416.ew`. Every `ew` command that works on a project takes it as its first
argument.

## Step 1: check the project

`ew check` runs the check rules over the project and prints what it finds, one finding per line:

```console
$ ew check golden/J-E-2416.ew
error EW-CORE-033 003033 =P01+J-E-2416-3W1: =P01+J-E-2416-3W1 has no routed, manual or part length
info EW-NUM-001 017101 =P01+J-E-2416-E56: E56: frozen, deviates from scheme (rule-derived value 101E?)
info EW-NUM-001 017101 =P01+J-E-2416-F55: F55: frozen, deviates from scheme (rule-derived value 101F?)
...
1 errors, 0 warnings, 10 infos, 0 waived
```

Each line gives the severity, the rule ID, the rule's six-digit code, the thing the finding is about and a message. The
last line counts the findings. The golden project has one error and ten infos, and the error is on purpose: the
cable `-3W1` has no length, so a cable list cannot be complete.

Look at the exit code:

```console
$ echo $?
1
```

`ew` exits with 0 when the project has no unwaived errors, 1 when it has some and 2 when it could not run the check at
all (a project that cannot be opened, a wrong option). A script should treat 2 as a failure of the tooling and 1 as an
answer about the drawing. The exit codes of every subcommand are in
[doc 17 §3.2](https://github.com/Ironkode787/ElektraWatt/blob/main/docs/design/17-extensibility.md#32-exit-codes).

Each rule has a page in the reference: [`EW-CORE-033`](../reference/rules/ew-core.md#ew-core-033) is the cable without
a length, and [`EW-NUM-001`](../reference/rules/ew-iec81346.md#ew-num-001) is the information that a frozen device tag
differs from what the numbering scheme would give.

`ew check` takes a scheme and a mode. The default is the `pre-delivery` scheme in `release` mode; `--format json`
and `--format md` change the output, and `--out <file>` writes it to a file.

## Step 2: write the pages as PDF and SVG

`ew export pdf` writes every page of the project into one PDF/A-2b file, with an outline, page labels and
cross-reference links:

```console
$ ew export pdf golden/J-E-2416.ew --out out/J-E-2416.pdf
PDF written to out/J-E-2416.pdf: 36 pages, 92 links
112 page findings (unresolved wire ends, unknown styles, overflowing table texts)
```

The second line, on the standard error stream, counts remarks about the rendering of single pages. They do not stop
the export. The directory of `--out` must exist, which is why you made `out` first.

`ew export svg` writes one SVG file per page into a directory. Use `--pages` to select folios; this writes the pages
numbered 101 to 106 that exist:

```console
$ ew export svg golden/J-E-2416.ew --out out/svg --pages 101-106
SVG written to out/svg: 4 files
```

The directory now holds `101.svg`, `102.svg`, `103.svg` and `106.svg`. `ew` replaces the directory as a whole, so it
must hold SVG files only; a stale page of an earlier export cannot remain in it.

The same export always gives the same bytes. The only date in the output comes from the project, or from the
environment variable `SOURCE_DATE_EPOCH` when you set it, never from the clock of your machine; `--threads <n>`
changes the speed and never the result.

## Step 3: export the cable list and the bill of materials

Two more exports write the rows of a report as CSV:

```console
$ ew export cables golden/J-E-2416.ew --format csv --out out/cables.csv
export written to out/cables.csv
$ ew export bom golden/J-E-2416.ew --mode assembly --out out/bom-assembly.csv
export written to out/bom-assembly.csv
```

Open `out/cables.csv` in a text editor or a spreadsheet. It starts like this:

```text
row,Cable,Cable type,Core,Colour,mm²,From,To,Length
heading,Cable overview,,,,,,,
data,-3W1,6XV1830-0EH10,,,,,,length missing
data,-106W1,NEBU-G78W5-K-2-N-LE5,,,,"-X1031, -X1032",-201U1,from part
```

The cable `-3W1` says "length missing": it is the finding of step 1, seen from the other side. The cables `-106W1`
and `-106W2` take their length from their part.

`--mode` of `ew export bom` is `parts` (the default; every component part) or `assembly`, which replaces the parts a
configured product covers by one row for the product. Both write the rows of the project's own report page, so the
CSV equals what the drawing prints.

## Step 4: ask the project questions with SQL

`ew sql` runs one read-only `SELECT` over the project's SQL views, which are named `api_v1_...`:

```console
$ ew sql golden/J-E-2416.ew "SELECT count(*) FROM api_v1_terminal"
count(*)
28
1 rows
```

The result is CSV with a header line, on the standard output; the row count goes to the standard error stream. Ask for
the cables and where their lengths come from:

```console
$ ew sql golden/J-E-2416.ew "SELECT tag, cores, length_source FROM api_v1_cable ORDER BY tag"
tag,cores,length_source
-106W1,5,part
-106W2,5,part
-3W1,3,missing
3 rows
```

`ew sql schema` prints every view with its columns. Only `SELECT`, `WITH` and `VALUES` statements run; anything else, such
as `ATTACH`, is refused and the exit code is 2.

## What you have done

You checked a project and read the exit code, wrote its pages as PDF and SVG, exported a cable list and a bill of
materials, and queried the project in SQL, all from the terminal. The same commands run in a script or a CI job,
because they read the project without locking it and never need a display.

## Where to go next

- The [command reference](../reference/commands/index.md) lists the commands that change a project, and the
  [rule reference](../reference/rules/index.md) the rules that `ew check` runs.
- [How-to guides](../how-to/index.md) for single tasks, such as checking a project in a CI job, will follow this
  tutorial.
- The [explanation](../explanation/index.md) section links the design of the model and of the check engine.
