#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

# The AppImage's install tests (WP-S6.06 D11, IN-10 to IN-12, IN-14), on the AppImage just built by package.yml's
# linux job. Bash; `ew`, `elektrawatt` and the checks run from the AppImage, never from the staged tree.
#
#   install-test.sh container <image> <appimage> <version> <listing> <out-dir>
#       On the runner: runs `inside` in the container <image> (digest-pinned: ubuntu:22.04 and debian:12) with
#       `--network none`, as the runner's uid and gid (so `chmod -R a-w` binds), the checkout and the AppImage's
#       folder mounted read-only and <out-dir> writable, each at its runner path.
#   install-test.sh inside <appimage> <version> <listing> <out-dir> [resources]
#       IN-14: `<appimage> --appimage-extract-and-run --version` prints `elektrawatt <version>` and `... ew --version`
#       `ew <version>` (<version>: the workspace version the programs print, FT-1227); `--appimage-extract` gives
#       AppRun, elektrawatt.desktop and elektrawatt.png at the AppDir's root, the last two equal to the repository's,
#       and `usr/lib/elektrawatt/` whose `<sha256>  <path>` listing equals <listing> (packaging/stage.py's listing of
#       the staged tree: D3's). Then on that extraction, with `chmod -R a-w`: `AppRun --smoke` on the installed
#       sample (IN-10), with `resources` D13's installed-resources runs (IN-11; packaging/stage.py, so where Python
#       is: the runner, since the containers have none), AC-40's ci/installed-acceptance.sh with the AppImage's `ew`
#       dispatch (`AppRun ew`, IN-12), and the same listing afterwards (IN-10). <out-dir> must not exist.
set -euo pipefail

fail() {
  echo "::error::install-test: $*" >&2
  exit 1
}

repo="$(cd "$(dirname "$0")/../.." && pwd)"

# `<sha256>  <path>` of every file under $1, paths relative and in byte order (packaging/stage.py listing's form).
tree_listing() {
  (cd "$1" && find . -type f -print0 | LC_ALL=C sort -z | xargs -0 -r sha256sum | sed 's|  \./|  |')
}

inside() {
  [ "$#" -ge 4 ] || fail "usage: install-test.sh inside <appimage> <version> <listing> <out-dir> [resources]"
  local appimage="$1" version="$2" listing="$3" out="$4" resources="${5:-}"
  mkdir "$out" || fail "the output directory exists or cannot be made: $out"
  cd "$out"

  local got
  got="$("$appimage" --appimage-extract-and-run --version)"
  [ "$got" = "elektrawatt $version" ] || fail "--appimage-extract-and-run --version printed '$got', not 'elektrawatt $version'"
  got="$("$appimage" --appimage-extract-and-run ew --version)"
  [ "$got" = "ew $version" ] || fail "--appimage-extract-and-run ew --version printed '$got', not 'ew $version'"
  echo "install-test: IN-14: both programs print $version through --appimage-extract-and-run"

  "$appimage" --appimage-extract > /dev/null
  local appdir="$out/squashfs-root" root="$out/squashfs-root/usr/lib/elektrawatt"
  [ -x "$appdir/AppRun" ] || fail "no executable AppRun at the AppDir's root"
  cmp "$appdir/elektrawatt.desktop" "$repo/packaging/linux/elektrawatt.desktop" \
    || fail "the AppDir's elektrawatt.desktop is not packaging/linux/elektrawatt.desktop"
  cmp "$appdir/elektrawatt.png" "$repo/assets/icons/app/elektrawatt.png" \
    || fail "the AppDir's elektrawatt.png is not assets/icons/app/elektrawatt.png"
  tree_listing "$root" > "$out/listing-before.txt"
  diff "$listing" "$out/listing-before.txt" || fail "the extracted usr/lib/elektrawatt/ is not D3's tree (IN-14)"
  echo "install-test: IN-14: the desktop file and the icon at the root; usr/lib/elektrawatt/ is D3's tree ($(wc -l < "$listing") files)"

  # The runner removes its temporary folder after the job: give the write access back on every exit.
  trap 'chmod -R u+w "'"$appdir"'" 2> /dev/null || true' EXIT
  chmod -R a-w "$appdir"
  if touch "$root/write-probe" 2> /dev/null; then fail "the user can still write in $root"; fi
  "$appdir/AppRun" --smoke "$root/samples/J-E-2416.ew" || fail "AppRun --smoke on the installed sample failed (IN-10)"
  if [ "$resources" = resources ]; then
    python3 -I "$repo/packaging/stage.py" resources --tree "$root" --work "$out/resources"
  fi
  mkdir "$out/bin"
  printf '#!/bin/sh\nexec "%s/AppRun" ew "$@"\n' "$appdir" > "$out/bin/ew"
  chmod +x "$out/bin/ew"
  bash "$repo/ci/installed-acceptance.sh" "$out/bin/ew" "$root/samples/J-E-2416.ew" "$repo/golden/J-E-2416/expected" \
    "$out/acceptance"
  tree_listing "$root" > "$out/listing-after.txt"
  chmod -R u+w "$appdir"
  diff "$out/listing-before.txt" "$out/listing-after.txt" || fail "the programs changed the extracted tree (IN-10)"
  echo "install-test: IN-10 to IN-12 on the extraction, read-only${resources:+, with D13's resource runs}: passed"
}

container() {
  [ "$#" -eq 5 ] || fail "usage: install-test.sh container <image> <appimage> <version> <listing> <out-dir>"
  local image="$1" appimage="$2" version="$3" listing="$4" out="$5" attempt
  for attempt in 1 2 3; do
    docker pull "$image" && break
    [ "$attempt" -lt 3 ] || fail "docker pull of $image failed three times"
    sleep $((attempt * 15))
  done
  mkdir -p "$(dirname "$out")"
  docker run --rm --network none --user "$(id -u):$(id -g)" --env HOME=/tmp \
    --volume "$repo:$repo:ro" \
    --volume "$(dirname "$appimage"):$(dirname "$appimage"):ro" \
    --volume "$(dirname "$listing"):$(dirname "$listing"):ro" \
    --volume "$(dirname "$out"):$(dirname "$out")" \
    "$image" bash "$repo/packaging/linux/install-test.sh" inside "$appimage" "$version" "$listing" "$out"
}

[ "$#" -ge 1 ] || fail "usage: install-test.sh container | inside ..."
mode="$1"
shift
case "$mode" in
  inside) inside "$@" ;;
  container) container "$@" ;;
  *) fail "usage: install-test.sh container | inside ..." ;;
esac
