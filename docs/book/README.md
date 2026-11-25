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
| `src/tutorials/` | lessons, each run as a test of the build (WP-S7.05 D10, D11): [a first schematic page](src/tutorials/first-page.md), [a terminal strip and a cable](src/tutorials/terminal-strip-and-cable.md), [reports and PDF](src/tutorials/reports-and-pdf.md), [importing a QElectroTech project](src/tutorials/import-qet.md), [a six-pin power block](src/tutorials/symbol-editor.md), [a PLC with its modules](src/tutorials/plc-station.md), [a bus network](src/tutorials/bus-network.md) and [a pneumatic circuit](src/tutorials/pneumatic-circuit.md); `files/` holds the files a tutorial starts from, `images/` its pictures. The command line's how-to, [use the command line on the sample project](src/how-to/headless-workflow.md), runs as a test too |
| `po/` | the PO template `messages.pot` of the hand-written pages and the table of contents, and `nl.po`, the Dutch translation (WP-S7.05 D14) |
| `tutorial-scripts/` | the UI script of each window tutorial (`ew.uiscript/1`), run by `elektrawatt`'s `tutorials` test, which also checks the page's menu paths and keys against the window (T6) |
| `rule-notes/` | the hand-written notes of every shipped rule, one file per rule ID with six headings; `docs-gen` joins them into the rule pages and fails without them (WP-S7.05 D5) |
| `src/how-to/` | task guides: [using the application window](src/how-to/application-window.md), the target of F1 (no build bundles the built guide with the application yet; decision log ID-492) |
| `src/reference/` | exact facts; `commands/`, `rules/` and `release-notes/` are **generated**; `window/` holds the tools, dialogs and panels that F1 opens |
| `src/explanation/` | background; links the design documents |

## Build

The book builds offline: nothing is fetched at build time, and the only plugin is the pinned `mdbook-gettext` of the
Dutch build. CI builds it with `ci/build-book.sh` and mdBook 0.5.4 (the versions and checksums of mdBook and of
mdbook-i18n-helpers are pinned in `packaging/components.toml`), in the `docs-book` job of `.github/workflows/docs.yml`.

Run `cargo xtask docs-gen`, then `mdbook build docs/book`: the reference pages are built, not committed, so the
book does not build without them.

```console
$ cargo xtask docs-gen                          # builds the reference pages and updates src/SUMMARY.md
$ mdbook build docs/book                        # writes target/book
```

`mdbook serve docs/book` previews the book with live reload.

## Generated pages

`cargo xtask docs-gen` writes `src/reference/commands/` (one page per command, from the parameter schemas of the
reference, `target/reference/commands/`) and `src/reference/rules/` (one page per rule and an index page per rule
pack, from the rule catalogue of the reference, `target/reference/rules.json`, and the notes of `rule-notes/`), the CLI, query,
`api_v1`, format, glossary and keymap references, and `src/reference/release-notes/` (one page per
`docs/release-notes/<X.Y.Z>.md`, newest first, with its relative links rewritten to the repository's address; the
drafts `unreleased.md` and `m1-draft.md` are not shown), and
regenerates the matching entries of `src/SUMMARY.md`. The pages are built, not committed: the root `.gitignore` names
them, and the contract files they render stay committed. `SUMMARY.md` is committed; `cargo xtask docs-gen --check`
writes no page and fails when it differs from the sources, and `cargo xtask check` of the whole workspace runs it as a
step.

When a command or a rule changes, regenerate the reference and the pages:

```console
$ cargo xtask docs-gen
```

## Writing pages

- Examples use the sample project only (`fixtures/sample/`, made by its script). Never put a customer, end-user,
  person, project or drawing identifier into a page; use the sample's placeholders (`Sample Customer`).
- Link design documents with their GitHub address (`https://github.com/Ironkode787/ElektraWatt/blob/main/docs/...`),
  because the built book does not contain them. Link pages of the book with relative `.md` paths.
- Say plainly when something is not built yet.
- A picture (`src/*/images/`) is a frame of the window painted offscreen at 1920 × 1080 from the page's tutorial
  script with frame lines added (`elektrawatt --headless --script <s> --visual --frames <dir> --window 1920x1080
  --ppp 1`; the frame lines are described in `crates/ew-ui/src/visual.rs`), cut to the part it shows, with a text
  alternative. Paint it again when the window it shows changes.

## Translation

The source language is English (`language = "en"` in `book.toml`). The Dutch guide (NFR-I18N-07, WP-S7.05 D14) is
built from the same pages through mdbook-i18n-helpers 0.4.0 (pinned in `packaging/components.toml`):

- `po/messages.pot` holds the messages of the hand-written pages and the table of contents, made from a copy of the
  book without the generated pages, so no generated text is translated. It is generated: `cargo xtask book-po` writes it
  from the pages (and `cargo xtask book-po --check` says whether it is stale). Never edit it. It is made when the
  guide is translated and before a release, not with every change of a page: `cargo xtask gen` and
  `cargo xtask rules` leave it and the translations alone, so that branches do not all rewrite them.
- `po/nl.po` translates the introduction, the tutorials page and the tutorials; every other message, and every
  generated page, shows in English. `cargo xtask book-po` merges it with the template: a message whose English is
  unchanged keeps its Dutch; one whose English changed takes its old Dutch marked `#, fuzzy`, with the old English
  under `#|` (the book shows such a message in English until it is looked at); a translation whose message is gone
  stays at the end under `#~`. So to translate after pages changed: run `cargo xtask book-po`, then in `nl.po` bring each fuzzy
  entry's `msgstr` up to date and delete its `#, fuzzy` and `#|` lines. Only the `msgstr` texts are written by hand
  (no Weblate at 0.1.0, ID-1331). Keep the window's Dutch words (the terminology table at the head of
  `crates/ew-viewmodel/texts/nl-NL/actions.ftl`) and its Dutch menu paths, which `elektrawatt`'s `tutorials` test
  checks.
- `ci/build-book.sh` builds the Dutch book into `nl/` of the built book (`MDBOOK_BOOK__LANGUAGE=nl`, the
  `mdbook-gettext` preprocessor set from the environment, so `mdbook build docs/book` alone needs no plugin). F1 in a
  window whose language is Dutch opens the Dutch page when the installed book has it.
- `cargo xtask rules` fails when `nl.po` leaves a message of the introduction or a tutorial untranslated (its rule
  `translations`); the Dutch window's every key is a test of `ew-viewmodel` (`every_shipped_locale_has_every_key`).
