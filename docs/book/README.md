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
| `src/tutorials/` | lessons, each run as a test of the build (WP-S7.05 D10, D11): [a first schematic page](src/tutorials/first-page.md), [importing a QElectroTech project](src/tutorials/import-qet.md) and [a first headless run on the golden project](src/tutorials/headless-workflow.md); `files/` holds the files a tutorial starts from |
| `tutorial-scripts/` | the UI script of each window tutorial (`ew.uiscript/1`), run by `elektrawatt`'s `tutorials` test, which also checks the page's menu paths and keys against the window (T6) |
| `rule-notes/` | the hand-written notes of every shipped rule, one file per rule ID with six headings; `docs-gen` joins them into the rule pages and fails without them (WP-S7.05 D5) |
| `src/how-to/` | task guides: [using the application window](src/how-to/application-window.md), the target of F1 (no build bundles the built guide with the application yet; decision log ID-492) |
| `src/reference/` | exact facts; `commands/`, `rules/` and `release-notes/` are **generated**; `window/` holds the tools, dialogs and panels that F1 opens |
| `src/explanation/` | background; links the design documents |

## Build

The book builds offline: it uses no plugin, and nothing is fetched at build time. CI builds it with mdBook 0.5.4 (the
version and the checksum of the release archive are pinned in `.github/workflows/docs.yml`, job `docs-book`).

Run `cargo xtask docs-gen`, then `mdbook build docs/book`: the reference pages are built, not committed, so the
book does not build without them.

```console
$ cargo xtask docs-gen                          # builds the reference pages and updates src/SUMMARY.md
$ mdbook build docs/book                        # writes target/book
$ python3 -I ci/check-book-links.py target/book  # every link of the built pages resolves
```

`mdbook serve docs/book` previews the book with live reload.

## Generated pages

`cargo xtask docs-gen` writes `src/reference/commands/` (one page per command, from the parameter schemas in
`crates/ew-commands/schemas/`) and `src/reference/rules/` (one page per rule and an index page per rule pack, from the
rule catalogue snapshot `crates/ew-checks/catalogue/rules.json` and the notes of `rule-notes/`), the CLI, query,
`api_v1`, format, glossary and keymap references, and `src/reference/release-notes/` (one page per
`docs/release-notes/<X.Y.Z>.md`, newest first, with its relative links rewritten to the repository's address; the
drafts `unreleased.md` and `m1-draft.md` are not shown), and
regenerates the matching entries of `src/SUMMARY.md`. The pages are built, not committed: the root `.gitignore` names
them, and the contract files they render stay committed. `SUMMARY.md` is committed; `cargo xtask docs-gen --check`
fails when it differs from the sources, and it also runs in `cargo xtask ci`.

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
