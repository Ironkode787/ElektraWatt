#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

# The pinned Mesa lavapipe of the Linux test jobs (WP-S2.11 D4): the version and the snapshot of ci/mesa-lavapipe.txt.
#
#   mesa-lavapipe.sh install
#       As root, on the runner. Adds Ubuntu 24.04's archive at the pin's snapshot time on snapshot.ubuntu.com as an apt
#       source (as ci/linux-container.sh reads it), with an apt preferences file that pins that origin (priority
#       1001), so that every package the install takes, lavapipe's LLVM among them, comes from the snapshot. Installs
#       exactly `mesa-vulkan-drivers=<version>`, checks it, and prints the installed Mesa and LLVM packages, each
#       with the source apt took it from.
#   mesa-lavapipe.sh check
#       Fails when `dpkg-query` reports another version of mesa-vulkan-drivers than the pin (or none).
#   mesa-lavapipe.sh '--test'
#       Shows that the check fails on a wrong version with its whole message, and passes on the pinned one.
set -euo pipefail

pin_file="$(cd "$(dirname "$0")" && pwd)/mesa-lavapipe.txt"
value() {
  sed -n "s/^$1=//p" "$pin_file"
}
want="$(value version)"
snapshot="$(value snapshot)"
[ -n "$want" ] || { echo "ci/mesa-lavapipe.txt names no version" >&2; exit 1; }
[[ "$snapshot" =~ ^[0-9]{8}T[0-9]{6}Z$ ]] || { echo "ci/mesa-lavapipe.txt: the snapshot is not a time such as 20261101T000000Z: $snapshot" >&2; exit 1; }

# The check of WP-S2.11 §5 U4: `$1` is the installed version.
same() {
  if [ "$1" != "$want" ]; then
    echo "mesa-vulkan-drivers is $1, not the pinned $want of ci/mesa-lavapipe.txt" >&2
    return 1
  fi
}

installed() {
  dpkg-query --show --showformat='${Version}' mesa-vulkan-drivers 2>/dev/null || echo "not installed"
}

case "${1:-}" in
  install)
    {
      echo "deb https://snapshot.ubuntu.com/ubuntu/$snapshot noble main universe"
      echo "deb https://snapshot.ubuntu.com/ubuntu/$snapshot noble-updates main universe"
      echo "deb https://snapshot.ubuntu.com/ubuntu/$snapshot noble-security main universe"
    } > /etc/apt/sources.list.d/mesa-lavapipe-snapshot.list
    {
      echo "Package: *"
      echo "Pin: origin \"snapshot.ubuntu.com\""
      echo "Pin-Priority: 1001"
    } > /etc/apt/preferences.d/mesa-lavapipe-snapshot.pref
    # FT-479: a stalled mirror connection becomes a retry of a few seconds.
    options=(-o Acquire::Retries=3 -o Acquire::http::Timeout=20 -o Acquire::https::Timeout=20)
    apt-get "${options[@]}" update -qq
    apt-get "${options[@]}" install -y -qq --no-install-recommends "mesa-vulkan-drivers=$want"
    same "$(installed)"
    echo "mesa-vulkan-drivers $want from the snapshot $snapshot"
    # ID-1375: the Mesa and LLVM that lavapipe runs on, each with the source apt took it from (its `***` line).
    packages="$({ dpkg-query --show --showformat='${db:Status-Abbrev} ${Package}\n' 'mesa-*' 'libllvm*' 2>/dev/null \
      || true; } | awk '$1 == "ii" { print $2 }')"
    for p in $packages; do
      apt-cache policy "$p"
    done
    ;;
  check)
    same "$(installed)"
    echo "mesa-vulkan-drivers is the pinned $want"
    ;;
  --test)
    if message="$(same "0.0-wrong" 2>&1)"; then
      echo "the check passed a wrong version" >&2
      exit 1
    fi
    expected="mesa-vulkan-drivers is 0.0-wrong, not the pinned $want of ci/mesa-lavapipe.txt"
    if [ "$message" != "$expected" ]; then
      echo "the check said \"$message\", not \"$expected\"" >&2
      exit 1
    fi
    same "$want"
    echo "the pin check fails on a wrong version and passes on $want"
    ;;
  *)
    echo "usage: mesa-lavapipe.sh install | check | --test" >&2
    exit 2
    ;;
esac
