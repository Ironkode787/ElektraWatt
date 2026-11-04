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

# The command line on the second copy (check exits 1 on Golden A's unwaived errors; apply and import qet exit 1 on a
# refused line or a failed model check, which are no network finding).
cli="$work/cli.ew"
flow ew-check "0 1" "$bin/ew" check "$cli"
flow ew-export-pdf "0" "$bin/ew" export pdf "$cli" --out "$work/cli.pdf"
flow ew-export-svg "0" "$bin/ew" export svg "$cli" --out "$work/svg" --pages 101
flow ew-export-bom "0" "$bin/ew" export bom "$cli" --out "$work/bom.csv"
flow ew-render "0" "$bin/ew" render "$cli" --page 103 --png --out "$work/103.png"
flow ew-sql "0" "$bin/ew" sql "$cli" "SELECT count(*) AS n FROM api_v1_device"
cp "$repo/ci/sandbox/fixture.qet" "$work/fixture.qet"
flow ew-import-qet "0 1" "$bin/ew" import qet "$work/fixture.qet" --content "$repo/content"
flow ew-print "0" "$bin/ew" print "$cli" --out "$work/print.pdf" --pages 101-103
flow ew-apply "0 1" "$bin/ew" apply "$cli" "$repo/golden/J-E-2416/variants/b2-renumber-pages.ewcmd"

if [ -s "$out/failed.txt" ]; then
  exit 1
fi
