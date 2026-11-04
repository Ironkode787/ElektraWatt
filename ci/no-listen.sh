#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

# The listening-socket canary of WP-S6.07 D12 (d) (doc 18 CI-R-09; NFR-SEC-09's smoke-start half, G-M2-15), run by
# package.yml's linux job on the installed tree of the Linux install test: the AppImage's extraction that
# packaging/linux/install-test.sh leaves on the runner. The network sandbox (CI-N-06) is WP-S6.05's.
#
#   no-listen.sh <appdir> <out-dir>
#       Runs, under `strace -f -e trace=listen` (every thread and child process), `<appdir>/AppRun --smoke` on the
#       installed sample and `<appdir>/AppRun ew export pdf` of the same sample, writing each trace to <out-dir>. Fails
#       when a program fails, when a trace does not end with the program's exit, or when a trace records a `listen`
#       call; prints the traces' size either way. <out-dir> must not exist.
set -euo pipefail

fail() {
  echo "::error::no-listen: $*" >&2
  exit 1
}

[ "$#" -eq 2 ] || fail "usage: no-listen.sh <appdir> <out-dir>"
appdir="$1" out="$2"
sample="$appdir/usr/lib/elektrawatt/samples/J-E-2416.ew"
[ -x "$appdir/AppRun" ] || fail "no executable AppRun in $appdir"
[ -f "$sample" ] || fail "no installed sample at $sample"
command -v strace > /dev/null || fail "strace is not installed"
mkdir "$out" || fail "the output directory exists or cannot be made: $out"

strace -f -e trace=listen -o "$out/smoke.strace" "$appdir/AppRun" --smoke "$sample" \
  || fail "AppRun --smoke failed under strace"
strace -f -e trace=listen -o "$out/export.strace" "$appdir/AppRun" ew export pdf "$sample" --out "$out/J-E-2416.pdf" \
  || fail "AppRun ew export pdf failed under strace"

failed=0
for trace in "$out/smoke.strace" "$out/export.strace"; do
  # The traced program's own exit is the last line: the trace covers its whole run.
  last="$(tail -n 1 "$trace")"
  [[ "$last" == *"+++ exited with 0 +++" ]] || fail "$trace does not end with the program's exit: $last"
  listens="$(grep -E '^([0-9]+ +)?listen\(' "$trace" || true)"
  if [ -n "$listens" ]; then
    echo "::error::no-listen: $(basename "$trace") records a listen call:" >&2
    printf '%s\n' "$listens" >&2
    failed=1
  else
    echo "no-listen: $(basename "$trace"): no listen call ($(wc -l < "$trace") trace lines)"
  fi
done
exit "$failed"
