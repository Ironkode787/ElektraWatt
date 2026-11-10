#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

# Builds the user guide (docs/book) offline into <dest> (WP-S6.06 D7; WP-S0.06, NFR-DOC-01, NFR-DOC-06). The one user
# of the mdBook pin: `docs.yml`'s `docs-book` job and `package.yml`'s `book` job call it, so the installers take the
# same offline book that `docs-book` checks.
#
#   bash ci/build-book.sh <dest>      (Linux, from the repository root; needs curl, sha256sum, python3, cargo, unshare,
#                                      sudo)
#   bash ci/build-book.sh --pot       (Linux, from the repository root; refreshes docs/book/po/messages.pot)
#
# 1. `cargo xtask docs-gen` writes the reference pages, which are built, not committed.
# 2. mdBook, the prebuilt Linux release archive of github.com/rust-lang/mdBook, is downloaded pinned by version and by
#    the SHA-256 that the release publishes for the archive, checked before anything is unpacked (resolved
#    2026-10-22). The pin is the `mdbook` component of packaging/components.toml (FT-1224), read through
#    `packaging/stage.py pin`; `mdbook --version` must print its version. (The names are not MDBOOK_*: mdBook reads
#    that prefix as configuration overrides.)
# 3. mdbook-i18n-helpers (WP-S7.05 D14), the `mdbook-i18n-helpers` component, is downloaded as its crate, checked by its
#    SHA-256 before it is unpacked, and built with `cargo install --locked`; it gives `mdbook-gettext`, the one
#    preprocessor of the Dutch build, and `mdbook-xgettext`, which makes the PO template.
# 4. The book is built in an empty network namespace, as the runner's user, so a plugin or theme that fetches fails
#    the build. ubuntu-24.04 restricts unprivileged user namespaces, so the namespace needs `unshare` and passwordless
#    sudo; their absence fails here with the reason, not as a documentation failure of the build. The English book goes
#    to <dest>, then the Dutch one (`book.language = nl`, docs/book/po/nl.po) to <dest>/nl.
# 5. The PO template is made from a copy of the book without its generated pages, so no generated text enters it; a
#    committed docs/book/po/messages.pot whose messages differ is a warning here (`--pot` refreshes it, before the
#    translation is worked on).
set -euo pipefail

docs_mdbook_version="$(python3 -I packaging/stage.py pin mdbook)"
docs_mdbook_url="$(python3 -I packaging/stage.py pin mdbook --field url)"
docs_mdbook_sha256="$(python3 -I packaging/stage.py pin mdbook --field sha256)"
docs_i18n_version="$(python3 -I packaging/stage.py pin mdbook-i18n-helpers)"
docs_i18n_url="$(python3 -I packaging/stage.py pin mdbook-i18n-helpers --field url)"
docs_i18n_sha256="$(python3 -I packaging/stage.py pin mdbook-i18n-helpers --field sha256)"
docs_template="docs/book/po/messages.pot"

fail() {
  echo "::error::build-book: $*" >&2
  exit 1
}

[ "$#" -eq 1 ] || fail "usage: ci/build-book.sh <dest> | ci/build-book.sh --pot"
dest="$1"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
archive="mdbook-v${docs_mdbook_version}-x86_64-unknown-linux-gnu.tar.gz"
curl --fail --silent --show-error --location --proto '=https' --tlsv1.2 \
  --output "$work/$archive" \
  "$docs_mdbook_url"
echo "${docs_mdbook_sha256}  $work/$archive" | sha256sum --check --strict -
mkdir "$work/mdbook"
tar -xzf "$work/$archive" -C "$work/mdbook"
printed="$("$work/mdbook/mdbook" --version)"
echo "$printed"
[ "$printed" = "mdbook v${docs_mdbook_version}" ] \
  || fail "the downloaded mdBook prints '$printed', not 'mdbook v${docs_mdbook_version}' (packaging/components.toml)"

crate="mdbook-i18n-helpers-${docs_i18n_version}.crate"
curl --fail --silent --show-error --location --proto '=https' --tlsv1.2 \
  --output "$work/$crate" \
  "$docs_i18n_url"
echo "${docs_i18n_sha256}  $work/$crate" | sha256sum --check --strict -
mkdir "$work/i18n-src"
tar -xzf "$work/$crate" -C "$work/i18n-src"
# Outside the workspace, so its cargo configuration and toolchain file do not apply to the tool's build.
(cd "$work" && cargo install --quiet --locked --path "i18n-src/mdbook-i18n-helpers-${docs_i18n_version}" \
  --root "$work/i18n" --target-dir "$work/i18n-target")
gettext="$work/i18n/bin/mdbook-gettext"
xgettext="$work/i18n/bin/mdbook-xgettext"
[ -x "$gettext" ] && [ -x "$xgettext" ] \
  || fail "cargo install of mdbook-i18n-helpers ${docs_i18n_version} gave no mdbook-gettext and mdbook-xgettext"

# D14: the PO template of the hand-written pages and the table of contents, into <dir>/messages.pot: the book is copied
# without the generated pages (the paths of the .gitignore block of docs_gen.rs) and without their lines in
# SUMMARY.md, then read by mdbook-xgettext.
pot_template() {
  local out="$1" copy="$work/pot-book" pattern="" path
  rm -rf "$copy"
  cp -r docs/book "$copy"
  while read -r path; do
    rm -rf "${copy:?}/${path#/docs/book/}"
    pattern="${pattern}|\\]\\(${path#/docs/book/src/}"
  done < <(grep '^/docs/book/src/reference/' .gitignore)
  grep -Ev "${pattern#|}" "$copy/src/SUMMARY.md" > "$copy/src/SUMMARY.kept"
  mv "$copy/src/SUMMARY.kept" "$copy/src/SUMMARY.md"
  MDBOOK_OUTPUT="{\"xgettext\": {\"command\": \"$xgettext\"}}" "$work/mdbook/mdbook" build "$copy" --dest-dir "$out"
  [ -s "$out/messages.pot" ] || fail "mdbook-xgettext wrote no $out/messages.pot"
}

if [ "$dest" = "--pot" ]; then
  pot_template "$work/pot"
  cp "$work/pot/messages.pot" "$docs_template"
  echo "build-book: wrote $docs_template"
  exit 0
fi

cargo xtask docs-gen

command -v unshare > /dev/null || fail "unshare (util-linux) is not installed on this runner: the offline book build needs it"
sudo -n true || fail "passwordless sudo is not available on this runner: the offline book build needs it for unshare --net"
sudo unshare --net --setuid "$(id -u)" --setgid "$(id -g)" -- true \
  || fail "unshare --net failed on this runner: the offline book build needs a network namespace"

# WP-S7.05 D2: the book is versioned per release; every page's title names the workspace version
# (`[workspace.package].version` of Cargo.toml), through mdBook's own configuration override.
docs_version="$(python3 -I -c 'import sys, tomllib; print(tomllib.load(open(sys.argv[1], "rb"))["workspace"]["package"]["version"])' Cargo.toml)"
docs_title="ElektraWatt User Guide ${docs_version}"
docs_title_nl="ElektraWatt Gebruikershandleiding ${docs_version}"

sudo unshare --net --setuid "$(id -u)" --setgid "$(id -g)" -- \
  env MDBOOK_BOOK__TITLE="$docs_title" "$work/mdbook/mdbook" build docs/book --dest-dir "$dest"

# D14: the Dutch book, through the one preprocessor mdbook-gettext and docs/book/po/nl.po; a message nl.po does not
# translate, and every generated page, shows in English.
sudo unshare --net --setuid "$(id -u)" --setgid "$(id -g)" -- \
  env MDBOOK_BOOK__TITLE="$docs_title_nl" MDBOOK_BOOK__LANGUAGE=nl \
  MDBOOK_PREPROCESSOR__GETTEXT="{\"after\": [\"links\"], \"command\": \"$gettext\"}" \
  "$work/mdbook/mdbook" build docs/book --dest-dir "$dest/nl"

# DC-4: the built book's index page carries the version in its title, the Dutch one too.
grep -qF "${docs_title}</title>" "$dest/index.html" \
  || fail "$dest/index.html does not carry the title '${docs_title}' (WP-S7.05 D2)"
grep -qF "${docs_title_nl}</title>" "$dest/nl/index.html" \
  || fail "$dest/nl/index.html does not carry the title '${docs_title_nl}' (WP-S7.05 D2, D14)"

# D14: a stale template is a warning; WP-S6.08's RC-06 refreshes it.
pot_template "$work/pot"
if ! stale="$(python3 -I ci/translation-status.py --stale "$work/pot/messages.pot" "$docs_template")"; then
  echo "::warning::build-book: $stale"
fi
