#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# The flows of the network sandbox (WP-S6.05 D4, CI-N-06), run by the deep nightly's job `network-sandbox` as the runner
# user inside a network namespace whose only interface is loopback, up. Each flow runs under
#   strace -f -e trace=socket,connect,bind,listen,sendto,sendmsg,sendmmsg,openat -o <out>/<flow>.strace
# with its output in <out>/<flow>.log; a flow whose exit code is not one it may end with is listed in <out>/failed.txt.
# ci/sandbox-check.py scans the traces afterwards, outside the namespace.
#
#   ci/sandbox/run.sh <repo> <release-dir> <kittest-binary> <work> <out>
#
# <release-dir> holds the release `elektrawatt` and `ew`, <work> two copies of Golden A (`gp.ew` for the window flows,
# `cli.ew` for the command line) and is the working directory of every flow.
set -uo pipefail

repo=$1
bin=$2
kittest=$3
work=$4
out=$5
mkdir -p "$out"
: >"$out/failed.txt"
cd "$work" || exit 2

# The user directories of every flow inside the work directory (C50-p3#7): whatever `sudo` left in the environment, no
# flow reads or writes root's home or the runner's own settings, state or caches, and a flow that may end with exit 1
# cannot pass on a refused write there.
export HOME="$work/home" XDG_CONFIG_HOME="$work/home/config" XDG_STATE_HOME="$work/home/state" \
  XDG_DATA_HOME="$work/home/data" XDG_CACHE_HOME="$work/home/cache"
mkdir -p "$XDG_CONFIG_HOME" "$XDG_STATE_HOME" "$XDG_DATA_HOME" "$XDG_CACHE_HOME"
unset EW_CONFIG_DIR EW_DATA_DIR EW_CACHE_DIR

# flow <name> "<exit codes it may end with>" <command>...
flow() {
  local name=$1 ok=$2 code=0
  shift 2
  strace -f -qq -e trace=socket,connect,bind,listen,sendto,sendmsg,sendmmsg,openat -o "$out/$name.strace" "$@" \
    >"$out/$name.log" 2>&1 || code=$?
  case " $ok " in
    *" $code "*) echo "ok    $name (exit $code)" ;;
    *)
      echo "FAIL  $name (exit $code; see $name.log)"
      echo "$name" >>"$out/failed.txt"
      ;;
  esac
}

# The window's binary: the smoke run (read-only) and a headless session (first run skipped, open, an edit, check,
# export PDF, print to file, save, quit).
flow smoke "0" "$bin/elektrawatt" --smoke "$work/gp.ew"
flow headless "0" "$bin/elektrawatt" --headless --script "$repo/ci/sandbox/flows.ewui" --config-dir "$work/config"

# The window's flows through the kittest harness: the tests ci/sandbox/kittest-filter.txt names.
mapfile -t tests < <(grep -v '^[[:space:]]*\(#\|$\)' "$repo/ci/sandbox/kittest-filter.txt")
flow kittest "0" "$kittest" --exact "${tests[@]}"
# A renamed test matches no name and runs nothing with exit 0 (C50-p3#7): every name must have run and passed.
if ! grep -q "^test result: ok\. ${#tests[@]} passed;" "$out/kittest.log"; then
  echo "FAIL  kittest (not all ${#tests[@]} named tests ran and passed; see kittest.log)"
  grep -qx kittest "$out/failed.txt" || echo kittest >>"$out/failed.txt"
fi

# The command line on the second copy (check exits 1 on Golden A's unwaived errors; apply and import qet exit 1 on a
# refused line or a failed model check, which are no network finding).
cli="$work/cli.ew"
flow ew-check "0 1" "$bin/ew" check "$cli"
flow ew-export-pdf "0" "$bin/ew" export pdf "$cli" --out "$work/cli.pdf"
flow ew-export-svg "0" "$bin/ew" export svg "$cli" --out "$work/svg" --pages 101
flow ew-export-bom "0" "$bin/ew" export bom "$cli" --out "$work/bom.csv"
flow ew-render "0" "$bin/ew" render "$cli" --page 103 --png --out "$work/103.png"
flow ew-sql "0" "$bin/ew" sql "$cli" "SELECT count(*) AS n FROM api_v1_device"
# The import flows (WP-S6.04 D14, NFR-SEC-18): a sibling file beside each fixture, which the second fixture names by an
# external entity and an `xi:include` (refused, exit 2). <out>/<flow>.import names the flow's source, the work folder,
# the repository and the user directories for ci/sandbox-check.py's rule `foreign-file`.
cp "$repo/ci/sandbox/fixture.qet" "$work/fixture.qet"
cp "$repo/ci/sandbox/fixture-hostile.qet" "$work/fixture-hostile.qet"
cp "$repo/ci/sandbox/sibling.txt" "$work/sibling.txt"
printf 'source %s\nwork %s\nrepo %s\nallow %s\n' "$work/fixture.qet" "$work" "$repo" "$HOME" >"$out/ew-import-qet.import"
flow ew-import-qet "0 1" "$bin/ew" import qet "$work/fixture.qet" --content "$repo/content"
printf 'source %s\nwork %s\nrepo %s\nallow %s\n' "$work/fixture-hostile.qet" "$work" "$repo" "$HOME" \
  >"$out/ew-import-qet-hostile.import"
flow ew-import-qet-hostile "2" "$bin/ew" import qet "$work/fixture-hostile.qet" --content "$repo/content"
flow ew-print "0" "$bin/ew" print "$cli" --out "$work/print.pdf" --pages 101-103
flow ew-apply "0 1" "$bin/ew" apply "$cli" "$repo/golden/J-E-2416/variants/b2-renumber-pages.ewcmd"

if [ -s "$out/failed.txt" ]; then
  exit 1
fi
