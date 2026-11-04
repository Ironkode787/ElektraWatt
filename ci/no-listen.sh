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
#   no-listen.sh --test   # the trace check on fixture traces, one of them with a listen call (C50-p2#6)
set -euo pipefail

fail() {
  echo "::error::no-listen: $*" >&2
  exit 1
}

# Each trace ends with the traced program's exit and records no `listen` call; returns 1 when one records one.
check_traces() {
  local failed=0 trace last listens
  for trace in "$@"; do
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
  return "$failed"
}

if [ "$#" -eq 1 ] && [ "$1" = "--test" ]; then
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' EXIT
  # `strace -f -o` writes each line after its process ID.
  printf '%s\n' '4242  --- SIGCHLD {si_signo=SIGCHLD} ---' '4243  +++ exited with 0 +++' '4242  +++ exited with 0 +++' \
    > "$work/clean.strace"
  printf '%s\n' '4243  listen(7, 128)                    = 0' '4243  +++ exited with 0 +++' '4242  +++ exited with 0 +++' \
    > "$work/listen.strace"
  printf '%s\n' '4242  --- SIGKILL {si_signo=SIGKILL} ---' > "$work/cut.strace"
  out="$( (check_traces "$work/clean.strace") 2>&1)" || fail "test: the clean trace failed: $out"
  [ "$out" = "no-listen: clean.strace: no listen call (3 trace lines)" ] || fail "test: the clean trace printed: $out"
  if out="$( (check_traces "$work/clean.strace" "$work/listen.strace") 2>&1)"; then
    fail "test: the trace with a listen call passed: $out"
  fi
  want="no-listen: clean.strace: no listen call (3 trace lines)
::error::no-listen: listen.strace records a listen call:
4243  listen(7, 128)                    = 0"
  [ "$out" = "$want" ] || fail "test: the trace with a listen call printed: $out"
  if out="$( (check_traces "$work/cut.strace") 2>&1)"; then
    fail "test: the trace without the program's exit passed: $out"
  fi
  want="::error::no-listen: $work/cut.strace does not end with the program's exit: 4242  --- SIGKILL {si_signo=SIGKILL} ---"
  [ "$out" = "$want" ] || fail "test: the trace without the program's exit printed: $out"
  echo "no-listen: test passed (a clean trace, a listen call, a trace cut short)"
  exit 0
fi

[ "$#" -eq 2 ] || fail "usage: no-listen.sh <appdir> <out-dir>"
appdir="$1" out="$2"
sample="$appdir/usr/lib/elektrawatt/samples/J-E-2416.ew"
[ -x "$appdir/AppRun" ] || fail "no executable AppRun in $appdir"
# The sample project is a folder (WP-S6.06 D3: samples/J-E-2416.ew/).
[ -d "$sample" ] || fail "no installed sample project at $sample"
command -v strace > /dev/null || fail "strace is not installed"
mkdir "$out" || fail "the output directory exists or cannot be made: $out"

strace -f -e trace=listen -o "$out/smoke.strace" "$appdir/AppRun" --smoke "$sample" \
  || fail "AppRun --smoke failed under strace"
strace -f -e trace=listen -o "$out/export.strace" "$appdir/AppRun" ew export pdf "$sample" --out "$out/J-E-2416.pdf" \
  || fail "AppRun ew export pdf failed under strace"

check_traces "$out/smoke.strace" "$out/export.strace"
