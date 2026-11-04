#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

# The Linux builds in the digest-pinned `ubuntu:22.04` container (WP-S6.06 D6; NFR-MAINT-09, doc 18 §5.3): the
# `c-only` job of ci.yml and the Linux jobs of package.yml (x86_64, and D16's aarch64 in the arm64 image). Runs inside
# the container as root, from the checkout (mounted at its path on the runner, so the path remaps and the release
# check see the runner's paths):
#
#   linux-container.sh download
#       With network. `apt-get update`, then downloads `gcc=$EW_GCC_VERSION` and `libc6-dev=$EW_LIBC6_DEV_VERSION`
#       (packaging/components.toml), and `squashfs-tools=$EW_SQUASHFS_TOOLS_VERSION` when that is set (the x86_64 job
#       of package.yml, for the AppImage), into the mounted /var/cache/apt/archives and /var/lib/apt/lists, which apt
#       checks against its signed index. The snapshot's index holds every architecture, so the arm64 image reads the
#       same source. The archive is Ubuntu's at the time EW_APT_SNAPSHOT (`[apt]` of
#       packaging/components.toml, FT-1222) on snapshot.ubuntu.com, so it holds the pinned versions for good. The
#       snapshot is served over HTTPS only, and the image has no CA certificates: the runner's bundle is mounted at
#       /etc/ssl/certs/ca-certificates.crt. `packaging/stage.py debs` then compares the downloaded files with the list.
#   linux-container.sh check | release | cli
#       With no network (`docker run --network none`). Installs the two packages from the downloaded files
#       (`--no-download`), asserts that no C++ compiler is there (`g++`, `c++`, `clang++`), and runs as the runner's
#       uid and gid (EW_UID, EW_GID, through `setpriv`) with the host's toolchain (EW_TOOLCHAIN_BIN, mounted) on PATH:
#       - check: `cargo check --locked --offline -p elektrawatt -p ew` (the `c-only` job: every build script's C
#         compilation with a C compiler only);
#       - release: `ci/release-check.sh env`, `cargo build --release --locked --offline -p elektrawatt -p ew`,
#         `ci/release-check.sh scan` on both binaries (reports `release-check-<binary>.md`), and the highest `GLIBC_`
#         symbol version of both, which must be at most 2.35 (glibc of Ubuntu 22.04, the oldest Supported Linux); it
#         writes the container's `dpkg-query --show` to `container-packages.txt` for the job summary (FT-1223);
#       - cli: the same for `ew` alone (D16's aarch64 CLI archive).
#       The dependencies are fetched on the runner first (`cargo fetch --locked`), into the mounted cargo home.
#   linux-container.sh appimage
#       With no network. Installs squashfs-tools from the downloaded files, writes `mksquashfs -version`'s first line
#       to `squashfs-tools-version.txt` for the job's report, and as the runner's uid and gid writes the squashfs image
#       of the AppDir EW_APPDIR to EW_SQUASHFS: `mksquashfs <AppDir> <image> -root-owned -noappend -comp zstd`. zstd:
#       the pinned runtime links libzstd and zlib into its squashfuse (its Makefile), so it reads zstd, which
#       decompresses faster than gzip at a smaller size (D10).
#   Every mode reads the same apt source, the snapshot, since the downloaded lists are named after it.
set -euo pipefail

fail() {
  echo "::error::linux-container: $*" >&2
  exit 1
}

[ "$#" -eq 1 ] || fail "usage: linux-container.sh download | check | release | cli | appimage"
mode="$1"
: "${EW_GCC_VERSION:?}" "${EW_LIBC6_DEV_VERSION:?}" "${EW_APT_SNAPSHOT:?}"
[[ "$EW_APT_SNAPSHOT" =~ ^[0-9]{8}T[0-9]{6}Z$ ]] \
  || fail "EW_APT_SNAPSHOT is not a time such as 20261102T000000Z: $EW_APT_SNAPSHOT"
packages=("gcc=$EW_GCC_VERSION" "libc6-dev=$EW_LIBC6_DEV_VERSION")
apt_options=(-o Acquire::Retries=3 -o Acquire::http::Timeout=20 --no-install-recommends --yes)

rm -f /etc/apt/sources.list.d/*
# The image's /etc/apt/apt.conf.d/docker-clean deletes /var/cache/apt/archives/*.deb after every dpkg run (and after
# `apt-get update`). The archives are the runner's folder, mounted into each run, and checked once by `stage.py debs`:
# with the hook, the release run's install of gcc emptied it, and the AppImage run found no squashfs-tools or liblzo2-2
# to install offline (package.yml run 6). Each run is a fresh container, so dropping the hook touches nothing else.
rm -f /etc/apt/apt.conf.d/docker-clean
{
  echo "deb https://snapshot.ubuntu.com/ubuntu/$EW_APT_SNAPSHOT jammy main"
  echo "deb https://snapshot.ubuntu.com/ubuntu/$EW_APT_SNAPSHOT jammy-updates main"
} > /etc/apt/sources.list

if [ "$mode" = download ]; then
  ca=/etc/ssl/certs/ca-certificates.crt
  [ -s "$ca" ] || fail "no CA bundle at $ca: mount the runner's, read-only (the snapshot is served over HTTPS only)"
  apt-get "${apt_options[@]:0:4}" -o Acquire::https::CaInfo="$ca" update
  DEBIAN_FRONTEND=noninteractive apt-get "${apt_options[@]}" -o Acquire::https::CaInfo="$ca" \
    install --download-only "${packages[@]}" ${EW_SQUASHFS_TOOLS_VERSION:+"squashfs-tools=$EW_SQUASHFS_TOOLS_VERSION"}
  exit 0
fi

as_runner() {
  setpriv --reuid "$EW_UID" --regid "$EW_GID" --clear-groups \
    env HOME=/tmp PATH="${EW_TOOLCHAIN_BIN:+$EW_TOOLCHAIN_BIN:}$PATH" "$@"
}

case "$mode" in
  check | release | cli) ;;
  appimage)
    : "${EW_UID:?}" "${EW_GID:?}" "${EW_SQUASHFS_TOOLS_VERSION:?}" "${EW_APPDIR:?}" "${EW_SQUASHFS:?}"
    DEBIAN_FRONTEND=noninteractive apt-get "${apt_options[@]}" install --no-download \
      "squashfs-tools=$EW_SQUASHFS_TOOLS_VERSION"
    mksquashfs -version | sed -n 1p | as_runner tee squashfs-tools-version.txt
    as_runner mksquashfs "$EW_APPDIR" "$EW_SQUASHFS" -root-owned -noappend -comp zstd
    exit 0
    ;;
  *) fail "usage: linux-container.sh download | check | release | cli | appimage" ;;
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

if [ "$mode" = check ]; then
  as_runner cargo check --locked --offline -p elektrawatt -p ew
  exit 0
fi

binaries=(elektrawatt ew)
cli_scan=""
if [ "$mode" = cli ]; then
  binaries=(ew)
  cli_scan=--cli
fi
# The release check's environment step runs in the shell that starts the build, so it sees what cargo inherits.
as_runner bash -c '
  set -euo pipefail
  bash ci/release-check.sh env
  cargo build --release --locked --offline "${@/#/--package=}"
' build "${binaries[@]}"
dpkg-query --show | as_runner tee container-packages.txt > /dev/null
failed=0
for binary in "${binaries[@]}"; do
  # `ew` alone (cli) has no x11-dl, which only the window's program pulls in: the scan's section 3 does not apply.
  as_runner bash ci/release-check.sh scan ${cli_scan:+"$cli_scan"} "target/release/$binary" target \
    "release-check-$binary.md" || failed=1
  newest="$(objdump -T "target/release/$binary" | { grep -o 'GLIBC_[0-9.]*' || true; } | sed 's/GLIBC_//' | sort -uV | tail -n 1)"
  highest="$(printf '%s\n%s\n' "$newest" 2.35 | sort -V | tail -n 1)"
  echo "linux-container: the highest GLIBC_ version of $binary is $newest (at most 2.35)"
  if [ "$highest" != 2.35 ]; then
    echo "::error::linux-container: $binary needs GLIBC_$newest, newer than Ubuntu 22.04's 2.35" >&2
    failed=1
  fi
done
exit "$failed"
