#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

# Parses every tracked script, never runs one (FT-1270, CI-PR-01 `lint`): `bash -n` over each `.sh` and the shipped
# `packaging/linux/AppRun`, and PowerShell's own parser (`[System.Management.Automation.Language.Parser]::ParseFile`)
# over each `.ps1`, so that a script that does not parse fails here, before the packaging jobs that run it. It fails
# closed: a failed listing, an empty one or a parser that throws fails the run (C50-p4#5).
#
#   bash ci/parse-scripts.sh      (from the repository root; needs git, bash and pwsh)
#
# Exit 0 when every script parses, 1 when one does not (each error printed with its file and line).
set -euo pipefail

# The listing in a file first, so that its exit status is checked (a process substitution's is not).
list="$(mktemp)"
trap 'rm -f "$list"' EXIT
git ls-files -z -- '*.sh' 'packaging/linux/AppRun' > "$list"
mapfile -d '' shells < "$list"
if [ "${#shells[@]}" -eq 0 ]; then
  echo "error: git lists no shell script to parse" >&2
  exit 1
fi

status=0
for script in "${shells[@]}"; do
  if ! bash -n "$script"; then
    echo "error: $script does not parse (bash -n)" >&2
    status=1
  fi
done

# The PowerShell scripts reach pwsh through the environment, one path per line: `-Command` takes no argument list.
EW_PS1="$(git ls-files -- '*.ps1')"
export EW_PS1
if [ -z "$EW_PS1" ]; then
  echo "error: git lists no PowerShell script to parse" >&2
  exit 1
fi
if ! pwsh -NoLogo -NoProfile -NonInteractive -Command '
  $ErrorActionPreference = "Stop"
  $failed = $false
  foreach ($script in ($env:EW_PS1 -split "`n" | Where-Object { $_ })) {
    try {
      $tokens = $null
      $errors = $null
      [void][System.Management.Automation.Language.Parser]::ParseFile((Resolve-Path $script).Path, [ref]$tokens, [ref]$errors)
      foreach ($e in $errors) {
        [Console]::Error.WriteLine("error: ${script}:$($e.Extent.StartLineNumber): $($e.Message)")
        $failed = $true
      }
    } catch {
      [Console]::Error.WriteLine("error: ${script}: the parser failed: $($_.Exception.Message)")
      $failed = $true
    }
  }
  if ($failed) { exit 1 }
'; then
  status=1
fi
exit "$status"
