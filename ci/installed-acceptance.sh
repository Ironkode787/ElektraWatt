#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

# AC-40's installed acceptance (WP-S6.06 D12, IN-12; doc 20 AC-40): the installed `ew`, without libraries (no
# EW_CONTENT_DIR and no --content), writes from the installed sample what the repository's acceptance files hold.
# package.yml runs it on Windows' installed tree and on Linux's staged tree, with write access to the tree denied
# (IN-10). Bash on both OSes (Git Bash on Windows), from any directory:
#
#   bash ci/installed-acceptance.sh <ew> <sample-dir> <expected-dir> <out-dir>
#
# - `ew export pdf` of the sample has the SHA-256 of <expected-dir>/pdf/sha256.txt;
# - `ew export report --kind <kind> --format csv` equals <expected-dir>/csv/<kind>.csv for every CSV there whose name is
#   a report kind of this `ew`; a name that `ew` refuses as an unknown report kind holds another export's rows (wires,
#   terminals, cables, bom) and is listed as such; both lists are printed;
# - `ew fmt --check` of the sample finds no file to rewrite (exit 0).
# <out-dir> must not exist; the outputs stay there. "Without system fonts" is proven where the system has none (the
# Linux containers of part 2b); elsewhere the PDF's digest, whose fonts are the bundled faces, is the same proof.
set -euo pipefail

fail() {
  echo "::error::installed-acceptance: $*" >&2
  exit 1
}

[ "$#" -eq 4 ] || fail "usage: ci/installed-acceptance.sh <ew> <sample-dir> <expected-dir> <out-dir>"
ew="$1"
sample="$2"
expected="$3"
out="$4"
unset EW_CONTENT_DIR
[ -d "$sample" ] || fail "the sample is not a directory: $sample"
[ -f "$expected/pdf/sha256.txt" ] || fail "missing input: $expected/pdf/sha256.txt"
mkdir "$out" || fail "the output directory exists or cannot be made: $out"

"$ew" export pdf "$sample" --out "$out/sample.pdf" || fail "ew export pdf of the installed sample failed"
want="$(tr -d '\r\n' < "$expected/pdf/sha256.txt")"
got="$(sha256sum < "$out/sample.pdf" | cut -d' ' -f1)"
[ "$got" = "$want" ] \
  || fail "the PDF of the installed sample has the SHA-256 $got, not the $want of $expected/pdf/sha256.txt"
echo "installed-acceptance: ew export pdf equals pdf/sha256.txt ($got)"

kinds=()
others=()
for file in "$expected"/csv/*.csv; do
  kind="$(basename "$file" .csv)"
  if "$ew" export report --kind "$kind" --format csv "$sample" --out "$out/$kind.csv" 2> "$out/$kind.err"; then
    cmp -s "$file" "$out/$kind.csv" \
      || fail "ew export report --kind $kind wrote $out/$kind.csv, which differs from $file"
    kinds+=("$kind")
  elif grep -qF "unknown report kind \`$kind\`" "$out/$kind.err"; then
    others+=("$kind")
  else
    cat "$out/$kind.err" >&2
    fail "ew export report --kind $kind failed"
  fi
done
[ "${#kinds[@]}" -gt 0 ] || fail "no CSV of $expected/csv is a report kind of this ew"
echo "installed-acceptance: ew export report --kind equals csv/<kind>.csv for ${#kinds[@]} kinds: ${kinds[*]}"
echo "installed-acceptance: the other CSVs hold other exports' rows: ${others[*]:-none}"

"$ew" fmt --check "$sample" || fail "ew fmt --check finds files of the installed sample to rewrite (or failed)"
echo "installed-acceptance: ew fmt --check finds no file to rewrite"
