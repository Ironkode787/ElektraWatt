# Use the command line on the sample project

Here you use the `ew` command-line program on the sample project: you check the project, write its pages as PDF and
SVG, export the cable list and the bill of materials, and ask the project questions in SQL. Nothing needs the desktop
application, and nothing leaves your machine: `ew` never uses the network.

The sample project is a small conveyor drive with a pneumatic clamp: a 400 V supply, a 24 V DC control circuit, a
motor starter, a PLC with its cards on a PROFIBUS DP line, a valve terminal with two cylinders, the panel layout and
the generated reports. Where a real project names a customer or a person, it holds a placeholder such as
`Sample Customer`. You only read it here; none of the steps changes it.

## Before you start

You need:

- a checkout of the ElektraWatt repository, which holds the project in `fixtures/sample/`;
- the `ew` program, built from the checkout with `cargo build --release -p ew`. It is then
  `target/release/ew` (`target\release\ew.exe` on Windows). The examples write plain `ew`; put the program on your
  `PATH` or type its path.

Run every command from the root of the checkout. Check that `ew` works; it prints its name and its version:

```console
$ ew --version
```

Then make a directory for the results:

```console
$ mkdir out
```

## Where the project comes from

EW makes the sample project itself. The script `fixtures/sample/sample.uiscript` holds the window's own actions in
order, from the new-project wizard to the rails of the panel layout, as a user chooses them; `cargo xtask gen` runs it and writes the
project `fixtures/sample/sample.ew`, and a test checks that the checked-in project is what the script makes.

A project is a directory, here `fixtures/sample/sample.ew`. Every `ew` command that works on a project takes it as its
first argument.

## Step 1: check the project

`ew check` runs the check rules over the project and prints what it finds, one finding per line:

```console
$ ew check fixtures/sample/sample.ew
0 errors, 0 warnings, 0 infos, 0 waived
pre-delivery: 0 errors, 0 warnings, 0 infos, 0 waived
```

The sample project is clean, so only the counts print. In a project with findings each one is a line before them,
for example a tag that is frozen and differs from what the numbering scheme would give:

```text
info EW-NUM-001 017101 +CAB-K1: K1: frozen, deviates from scheme (rule-derived value 101K1)
```

Each line gives the severity, the rule ID, the rule's six-digit code, the thing the finding is about and a message. The
last line counts the findings, and the standard error stream repeats the count with the scheme's name.

Look at the exit code:

```console
$ echo $?
0
```

`ew` exits with 0 when the project has no unwaived errors, 1 when it has some and 2 when it could not run the check at
all (a project that cannot be opened, a wrong option). A script should treat 2 as a failure of the tooling and 1 as an
answer about the drawing. The exit codes of every subcommand are in
[doc 17 §3.2](https://github.com/Ironkode787/ElektraWatt/blob/main/docs/design/17-extensibility.md#32-exit-codes).

Each rule has a page in the reference: [`EW-NUM-001`](../reference/rules/ew-num-001.md) is the information that a
frozen device tag differs from what the numbering scheme would give, and [`EW-CORE-033`](../reference/rules/ew-core-033.md)
is the error of a cable without a length, which the sample does not have.

`ew check` takes a scheme and a mode. The default is the `pre-delivery` scheme in `release` mode; `--format json`
and `--format md` change the output, and `--out <file>` writes it to a file.

## Step 2: write the pages as PDF and SVG

`ew export pdf` writes every page of the project into one PDF/A-2b file, with an outline, page labels and
cross-reference links:

```console
$ ew export pdf fixtures/sample/sample.ew --out out/sample.pdf
PDF written to out/sample.pdf: 35 pages, 203 links
260 page findings: 260 empty fields in a frame or title block
```

The second line, on the standard error stream, counts remarks about the rendering of single pages and says what
they are: here, fields of the title blocks that the project leaves empty. They do not stop the export. The directory of
`--out` must exist, which is why you made `out` first.

`ew export svg` writes one SVG file per page into a directory. Use `--pages` to select folios; this writes the drawn
pages 101 to 105:

```console
$ ew export svg fixtures/sample/sample.ew --out out/svg --pages 101-105
SVG written to out/svg: 5 files
35 page findings: 35 empty fields in a frame or title block
```

The directory now holds `101.svg` to `105.svg`. Both ends of a range are folio labels of the project. `ew` replaces
the directory as a whole, so it must be empty or one an earlier SVG export wrote (the file `.elektrawatt-export` in
it says so); a stale page of an earlier export cannot remain in it, and a folder of your own files is refused.

The same export always gives the same bytes. The only date in the output comes from the project, or from the
environment variable `SOURCE_DATE_EPOCH` when you set it, never from the clock of your machine; `--threads <n>`
changes the speed and never the result.

## Step 3: export the cable list and the bill of materials

Two more exports write the rows of a report as CSV:

```console
$ ew export cables fixtures/sample/sample.ew --format csv --out out/cables.csv
export written to out/cables.csv
$ ew export bom fixtures/sample/sample.ew --mode assembly --out out/bom-assembly.csv
export written to out/bom-assembly.csv
```

Open `out/cables.csv` in a text editor or a spreadsheet. It starts like this:

```text
row,Cable,Cable type,Core,Colour,mm²,From,To,Length
heading,Cable overview,,,,,,,
data,-101W1,EW-GEN-CBL-17,,,,,-X1011,10 m
data,-103W1,EW-GEN-CBL-25,,,,-X1031,-103M1,5 m
data,-105W1,EW-GEN-CBL-2,,,,-104A1,-201K1,8 m
```

The supply cable `-101W1` comes in from outside the cabinet, so it has no From end; the motor cable `-103W1` runs from
the strip `-X1031` to the motor, and the bus cable `-105W1` from the PLC to the valve terminal. Each has the length
typed in its Properties. Below these rows come the cores of each cable, one row per core.

`--mode` of `ew export bom` is `parts` (the default; every component part) or `assembly`, which replaces the parts a
configured product covers by one row for the product. Both write the rows of the project's own report page, so the
CSV equals what the drawing prints. A project without that page (a new project from the built-in template) refuses
them and names the command that writes the rows of the whole project instead, such as
`ew export report --kind cable_overview_diagrams`.

## Step 4: ask the project questions with SQL

`ew sql` runs one read-only `SELECT` over the project's SQL views, which are named `api_v1_...`:

```console
$ ew sql fixtures/sample/sample.ew "SELECT count(*) FROM api_v1_terminal"
count(*)
18
1 rows
```

The result is CSV with a header line, on the standard output; the row count goes to the standard error stream. Ask for
the cables and where their lengths come from:

```console
$ ew sql fixtures/sample/sample.ew "SELECT tag, cores, length_source FROM api_v1_cable ORDER BY tag"
tag,cores,length_source
-101W1,5,manual
-103W1,5,manual
-105W1,3,manual
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
- More [how-to guides](index.md) for single tasks, such as checking a project in a CI job, will follow this one.
- The [explanation](../explanation/index.md) section links the design of the model and of the check engine.
