#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

# FT-1267 (WP-S6.08 D6): the installed `elektrawatt` finds the installed user guide. The listings prove the folder is
# there; this proves the program opens it. package.yml runs it beside ci/installed-acceptance.sh on Windows' installed
# tree and on Linux's extraction of the AppImage, read-only. Bash on both OSes (Git Bash on Windows):
#
#   bash ci/installed-guide.sh <elektrawatt> <docs-dir> <out-dir>
#
# A headless run (`--headless --script`, a fresh `--config-dir`) asks for the guide's home page as Help ▸ Documentation
# does (`help.documentation`, topic `home`); the transcript's `doc` names the file the request opens below the guide's
# folder, which must be `introduction.html`, a file of <docs-dir> beside its `index.html`. When <docs-dir> holds the
# Dutch guide (`nl/`), the same request in a Dutch window must open `nl/introduction.html`, a file there too.
# <out-dir> must not exist; the script, the settings and the transcript stay there.
set -euo pipefail

fail() {
  echo "::error::installed-guide: $*" >&2
  exit 1
}

[ "$#" -eq 3 ] || fail "usage: ci/installed-guide.sh <elektrawatt> <docs-dir> <out-dir>"
exe="$1"
docs="$2"
out="$3"
unset EW_DOCS_DIR EW_CONFIG_DIR
[ -f "$docs/index.html" ] || fail "the installed guide has no index.html: $docs"
mkdir "$out" || fail "the output directory exists or cannot be made: $out"

nl=false
[ -d "$docs/nl" ] && nl=true
{
  echo '{"$schema":"ew.uiscript/1","label":"FT-1267: the installed user guide"}'
  echo '{"action":"help.documentation","params":{"topic":"home"}}'
  if [ "$nl" = true ]; then
    echo '{"action":"settings.set","params":{"key":"language","value":"nl"}}'
    echo '{"action":"help.documentation","params":{"topic":"home"}}'
  fi
} > "$out/guide.uiscript"
(cd "$out" && "$exe" --headless --script guide.uiscript --config-dir config > transcript.jsonl) \
  || fail "the headless run failed (transcript: $out/transcript.jsonl)"

docs_of() {
  grep '"action":"help.documentation"' "$out/transcript.jsonl" | grep -o '"doc":\(null\|"[^"]*"\)' | sed -n "$1p"
}
check() {
  local n="$1" want="$2"
  local got
  got="$(docs_of "$n")"
  [ "$got" = "\"doc\":\"$want\"" ] || fail "request $n opens ${got:-nothing}, not \"doc\":\"$want\" (the guide is not found)"
  [ -f "$docs/$want" ] || fail "request $n opens $want, which $docs does not hold"
  echo "installed-guide: help.documentation opens $want of the installed guide"
}
check 1 introduction.html
if [ "$nl" = true ]; then
  check 2 nl/introduction.html
fi
