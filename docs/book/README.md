# The ElektraWatt user guide (mdBook)

This directory holds the user documentation of ElektraWatt as an [mdBook](https://rust-lang.github.io/mdBook/)
(NFR-DOC-01, WP-S0.06). The design documents, the decision records and the plan stay in `docs/design`, `docs/adr` and
`docs/plan`; the book links to them and does not copy them.

## Layout

The book follows the [Diátaxis](https://diataxis.fr/) structure. Each section has a landing page that says what
belongs there.

| Path | Content |
|---|---|
| `book.toml` | the mdBook configuration (title, language, offline HTML output) |
| `src/SUMMARY.md` | the table of contents; the entries below `reference/commands/index.md` and `reference/rules/index.md` are generated |
| `src/introduction.md` | what the guide is and how it is organised |
| `src/tutorials/` | lessons: [a first headless run on the golden project](src/tutorials/headless-workflow.md) |
| `src/how-to/` | task guides (none yet) |
| `src/reference/` | exact facts; `commands/` and `rules/` are **generated** |
| `src/explanation/` | background; links the design documents |

## Build

The book builds offline: it uses no plugin, and nothing is fetched at build time. CI builds it with mdBook 0.5.4 (the
version and the checksum of the release archive are pinned in `.github/workflows/docs.yml`, job `docs-book`).

```console
$ cargo xtask docs-gen --check                  # the generated pages are current
$ mdbook build docs/book                        # writes target/book
$ python3 -I ci/check-book-links.py target/book  # every link of the built pages resolves
```

`mdbook serve docs/book` previews the book with live reload.

## Generated pages

`cargo xtask docs-gen` writes `src/reference/commands/` (one page per command, from the parameter schemas in
`crates/ew-commands/schemas/`) and `src/reference/rules/` (one page per rule pack, from the rule catalogue snapshot
`crates/ew-checks/catalogue/rules.json`), and regenerates the matching entries of `src/SUMMARY.md`. The pages are
committed. `cargo xtask docs-gen --check` fails when one differs from its source; it also runs in `cargo xtask ci`.

When a command or a rule changes, regenerate the sources first, then the pages:

```console
$ EW_COMMANDS_BLESS=1 cargo test -p ew-commands --test schemas
$ EW_CHECKS_BLESS=1 cargo test -p ew-checks --test catalogue
$ cargo xtask docs-gen
```

## Writing pages

- Examples use the anonymised golden project only. Never put a customer, end-user, person, project or drawing
  identifier into a page; use the placeholders of the golden project (`<customer>`, `<drawing id>`).
- Link design documents with their GitHub address (`https://github.com/Ironkode787/ElektraWatt/blob/main/docs/...`),
  because the built book does not contain them. `ci/check-book-links.py` checks that such a link names an existing
  file and heading. Link pages of the book with relative `.md` paths.
- Say plainly when something is not built yet.

## Translation

The source language is English (`language = "en"` in `book.toml`). Translations are planned through
mdbook-i18n-helpers (PO files) and Weblate (NFR-I18N-07: Dutch tutorials at the MVP, German by 1.0). Neither is set up
yet: the Weblate project needs a maintainer's account, and mdbook-i18n-helpers is added to the build when a pinned
version has been verified to build offline with this mdBook version.
