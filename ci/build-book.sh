#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

# Builds the user guide (docs/book) offline into <dest> (WP-S6.06 D7; WP-S0.06, NFR-DOC-01, NFR-DOC-06). The one place
# of the mdBook pin: `docs.yml`'s `docs-book` job and `package.yml`'s `book` job call it, so the installers take the
# same offline book that `docs-book` checks.
#
#   bash ci/build-book.sh <dest>      (Linux, from the repository root; needs curl, sha256sum, unshare and sudo)
#
# 1. `cargo xtask docs-gen` writes the reference pages, which are built, not committed.
# 2. mdBook, the prebuilt Linux release archive of github.com/rust-lang/mdBook, is downloaded pinned by version and by
#    the SHA-256 that the release publishes for the archive, checked before anything is unpacked (resolved
#    2026-10-22). Update the version and the digest together. (The names are not MDBOOK_*: mdBook reads that prefix
#    as configuration overrides.)
# 3. The book is built in an empty network namespace, as the runner's user, so a plugin or theme that fetches fails
#    the build. ubuntu-24.04 restricts unprivileged user namespaces, so the namespace needs `unshare` and passwordless
#    sudo; their absence fails here with the reason, not as a documentation failure of the build.
set -euo pipefail

docs_mdbook_version="0.5.4"
docs_mdbook_sha256="3f28de05dafca9d0f2eab99c662116b0e37b89b1d96a08f8f430b9eeae958cd7"

fail() {
  echo "::error::build-book: $*" >&2
  exit 1
}

[ "$#" -eq 1 ] || fail "usage: ci/build-book.sh <dest>"
dest="$1"

cargo xtask docs-gen

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
archive="mdbook-v${docs_mdbook_version}-x86_64-unknown-linux-gnu.tar.gz"
curl --fail --silent --show-error --location --proto '=https' --tlsv1.2 \
  --output "$work/$archive" \
  "https://github.com/rust-lang/mdBook/releases/download/v${docs_mdbook_version}/${archive}"
echo "${docs_mdbook_sha256}  $work/$archive" | sha256sum --check --strict -
mkdir "$work/mdbook"
tar -xzf "$work/$archive" -C "$work/mdbook"
"$work/mdbook/mdbook" --version

command -v unshare > /dev/null || fail "unshare (util-linux) is not installed on this runner: the offline book build needs it"
sudo -n true || fail "passwordless sudo is not available on this runner: the offline book build needs it for unshare --net"
sudo unshare --net --setuid "$(id -u)" --setgid "$(id -g)" -- true \
  || fail "unshare --net failed on this runner: the offline book build needs a network namespace"

sudo unshare --net --setuid "$(id -u)" --setgid "$(id -g)" -- \
  "$work/mdbook/mdbook" build docs/book --dest-dir "$dest"
