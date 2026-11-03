#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

# The Linux builds in the digest-pinned `ubuntu:22.04` container (WP-S6.06 D6; NFR-MAINT-09, doc 18 §5.3): the
# `c-only` job of ci.yml and the Linux job of package.yml. Runs inside the container as root, from the checkout
# (mounted at its path on the runner, so the path remaps and the release check see the runner's paths):
#
#   linux-container.sh download
#       With network. `apt-get update`, then downloads `gcc=$EW_GCC_VERSION` and `libc6-dev=$EW_LIBC6_DEV_VERSION`
#       (packaging/components.toml) from the image's own archive, which apt checks against its signed index, into the
#       mounted /var/cache/apt/archives and /var/lib/apt/lists. A version the archive no longer holds fails here.
#   linux-container.sh check | release
#       With no network (`docker run --network none`). Installs the two packages from the downloaded files
#       (`--no-download`), asserts that no C++ compiler is there (`g++`, `c++`, `clang++`), and runs as the runner's
#       uid and gid (EW_UID, EW_GID, through `setpriv`) with the host's toolchain (EW_TOOLCHAIN_BIN, mounted) on PATH:
#       - check: `cargo check --locked --offline -p elektrawatt -p ew` (the `c-only` job: every build script's C
#         compilation with a C compiler only);
#       - release: `ci/release-check.sh env`, `cargo build --release --locked --offline -p elektrawatt -p ew`,
#         `ci/release-check.sh scan` on both binaries (reports `release-check-<binary>.md`), and the highest `GLIBC_`
#         symbol version of both, which must be at most 2.35 (glibc of Ubuntu 22.04, the oldest Supported Linux).
#       The dependencies are fetched on the runner first (`cargo fetch --locked`), into the mounted cargo home.
set -euo pipefail

fail() {
  echo "::error::linux-container: $*" >&2
  exit 1
}

[ "$#" -eq 1 ] || fail "usage: linux-container.sh download | check | release"
mode="$1"
: "${EW_GCC_VERSION:?}" "${EW_LIBC6_DEV_VERSION:?}"
packages=("gcc=$EW_GCC_VERSION" "libc6-dev=$EW_LIBC6_DEV_VERSION")
apt_options=(-o Acquire::Retries=3 -o Acquire::http::Timeout=20 --no-install-recommends --yes)

if [ "$mode" = download ]; then
  apt-get "${apt_options[@]:0:4}" update
  DEBIAN_FRONTEND=noninteractive apt-get "${apt_options[@]}" install --download-only "${packages[@]}"
  exit 0
fi

case "$mode" in
  check | release) ;;
  *) fail "usage: linux-container.sh download | check | release" ;;
esac
: "${EW_UID:?}" "${EW_GID:?}" "${EW_TOOLCHAIN_BIN:?}"

DEBIAN_FRONTEND=noninteractive apt-get "${apt_options[@]}" install --no-download "${packages[@]}"
for compiler in g++ c++ clang++; do
  if command -v "$compiler" > /dev/null; then
    fail "a C++ compiler is in the container: $(command -v "$compiler") (NFR-MAINT-09: Rust and a C compiler only)"
  fi
done
echo "linux-container: no g++, c++ or clang++; C compiler: $(cc --version | sed -n 1p)"
dpkg-query --show gcc libc6-dev libc6

as_runner() {
  setpriv --reuid "$EW_UID" --regid "$EW_GID" --clear-groups \
    env HOME=/tmp PATH="$EW_TOOLCHAIN_BIN:$PATH" "$@"
}

if [ "$mode" = check ]; then
  as_runner cargo check --locked --offline -p elektrawatt -p ew
  exit 0
fi

# The release check's environment step runs in the shell that starts the build, so it sees what cargo inherits.
as_runner bash -c '
  set -euo pipefail
  bash ci/release-check.sh env
  cargo build --release --locked --offline -p elektrawatt -p ew
'
failed=0
for binary in elektrawatt ew; do
  as_runner bash ci/release-check.sh scan "target/release/$binary" target "release-check-$binary.md" || failed=1
  newest="$(objdump -T "target/release/$binary" | { grep -o 'GLIBC_[0-9.]*' || true; } | sed 's/GLIBC_//' | sort -uV | tail -n 1)"
  highest="$(printf '%s\n%s\n' "$newest" 2.35 | sort -V | tail -n 1)"
  echo "linux-container: the highest GLIBC_ version of $binary is $newest (at most 2.35)"
  if [ "$highest" != 2.35 ]; then
    echo "::error::linux-container: $binary needs GLIBC_$newest, newer than Ubuntu 22.04's 2.35" >&2
    failed=1
  fi
done
exit "$failed"
