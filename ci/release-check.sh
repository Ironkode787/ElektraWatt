#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

# The release-build assertions of CI-R-03 for the Linux build of `elektrawatt` (FT-333; doc 18 §3.5, §5.2, §5.4;
# NFR-REL-07, NFR-SEC-20), run by the job `release-check` of `.github/workflows/cli-artefact.yml` before the release
# pipeline of WP-S6.07 exists. Two modes:
#
#   release-check.sh env
#       Run in the shell that starts the release build, right before `cargo build`. Fails unless PKG_CONFIG is
#       `false` and no variable that the pkg-config crate consults instead is set: `PKG_CONFIG_*` (the target-specific
#       `PKG_CONFIG_<target>` overrides, `PKG_CONFIG_PATH`, `PKG_CONFIG_LIBDIR`, `PKG_CONFIG_SYSROOT_DIR`, ...),
#       `HOST_PKG_CONFIG*` and `TARGET_PKG_CONFIG*`; and unless `ICU4X_DATA_DIR` is unset (§3.5). The names are read
#       from the environment this shell passes to its children, which is what cargo and the build scripts see, so a
#       name that is no valid shell identifier (`PKG_CONFIG_x86_64-unknown-linux-gnu`) is caught too.
#
#   release-check.sh scan <binary> <target-dir> <report.md>
#       After the build. Fails on any hit of
#       1. a build-host path in the binary (`strings -a`): `/home/`, `/usr/lib/x86_64-linux-gnu`, `/opt/`, and the
#          values of GITHUB_WORKSPACE, CARGO_HOME, RUSTUP_HOME and RUNNER_TEMP when set;
#       2. a hard link (`readelf -d` NEEDED) to libwayland*, libxkbcommon* or libX11*: these are dlopen'ed at run time;
#       3. a `Some(..)` library directory in x11-dl's generated `config.rs` under <target-dir>/release/build (with
#          PKG_CONFIG=false every entry is `None`), or no such file at all (the layout changed under the check).
#       Writes a Markdown report to <report.md> in every case, `ldd` output included for information.
#
# Locally (Linux): PKG_CONFIG=false ci/release-check.sh env && PKG_CONFIG=false cargo build -p elektrawatt --release
# --locked && ci/release-check.sh scan target/release/elektrawatt target report.md
set -euo pipefail

die() {
  echo "release-check: $*" >&2
  exit 1
}

# The names of the environment this shell exports, one per line (NUL-separated read: values may hold newlines).
env_names() {
  env -0 | while IFS= read -r -d '' kv; do printf '%s\n' "${kv%%=*}"; done
}

mode_env() {
  local failed=0
  if [ "${PKG_CONFIG-}" != "false" ]; then
    echo "release-check: PKG_CONFIG must be 'false', is '${PKG_CONFIG-<unset>}'" >&2
    failed=1
  fi
  local overrides
  overrides="$(env_names | grep -E '^(PKG_CONFIG_|HOST_PKG_CONFIG|TARGET_PKG_CONFIG)' || true)"
  if [ -n "$overrides" ]; then
    echo "release-check: pkg-config override variables are set (names only):" >&2
    printf '%s\n' "$overrides" | sed 's/^/  /' >&2
    failed=1
  fi
  # No `grep -q`: under pipefail its early exit would fail the pipeline through SIGPIPE and hide a hit.
  if [ -n "$(env_names | grep -x 'ICU4X_DATA_DIR' || true)" ]; then
    echo "release-check: ICU4X_DATA_DIR is set (doc 18 §3.5)" >&2
    failed=1
  fi
  [ "$failed" -eq 0 ] || exit 1
  echo "release-check: environment ok (PKG_CONFIG=false; no PKG_CONFIG_*, HOST_PKG_CONFIG*, TARGET_PKG_CONFIG*; no ICU4X_DATA_DIR)"
}

mode_scan() {
  [ "$#" -eq 3 ] || die "usage: release-check.sh scan <binary> <target-dir> <report.md>"
  local binary="$1" target_dir="$2" report="$3"
  [ -f "$binary" ] || die "no binary at $binary"
  for tool in strings readelf ldd; do
    command -v "$tool" > /dev/null || die "$tool not found (binutils, libc-bin)"
  done

  local failed=0
  strings_file="$(mktemp)"
  trap 'rm -f "$strings_file"' EXIT
  strings -a "$binary" > "$strings_file"

  local -a patterns=("/home/" "/usr/lib/x86_64-linux-gnu" "/opt/")
  local var
  for var in GITHUB_WORKSPACE CARGO_HOME RUSTUP_HOME RUNNER_TEMP; do
    if [ -n "${!var-}" ]; then
      patterns+=("${!var}")
    fi
  done

  {
    echo "# Release check of \`$(basename "$binary")\` (CI-R-03 before WP-S6.07; FT-333)"
    echo
    echo "- commit: ${GITHUB_SHA:-local}"
    echo "- binary: \`$binary\`, $(stat -c %s "$binary") bytes, sha256 \`$(sha256sum "$binary" | cut -d' ' -f1)\`"
    echo "- PKG_CONFIG at scan time: \`${PKG_CONFIG-<unset>}\`"
    echo "- RUSTFLAGS: \`${RUSTFLAGS-<unset>}\`"
    echo
    echo "## 1. Build-host paths in the binary (\`strings -a\`)"
    echo
    echo "| Pattern | Hits |"
    echo "|---|---|"
  } > "$report"
  local pattern hits
  local samples=""
  for pattern in "${patterns[@]}"; do
    hits="$(grep -cF -- "$pattern" "$strings_file" || true)"
    echo "| \`$pattern\` | $hits |" >> "$report"
    if [ "$hits" -gt 0 ]; then
      failed=1
      samples+=$'\n'"First hits of \`$pattern\`:"$'\n\n```\n'"$(grep -F -- "$pattern" "$strings_file" | head -n 20)"$'\n```\n'
    fi
  done
  printf '%s\n' "$samples" >> "$report"

  {
    echo "## 2. Hard-linked libraries (\`readelf -d\`, NEEDED)"
    echo
    echo '```'
    readelf -d "$binary" | grep -F '(NEEDED)' || echo "(none)"
    echo '```'
    echo
  } >> "$report"
  local forbidden
  forbidden="$(readelf -d "$binary" | grep -F '(NEEDED)' | grep -E '\[lib(wayland|xkbcommon|X11)' || true)"
  if [ -n "$forbidden" ]; then
    failed=1
    {
      echo "**Fail:** linked at load time, must be dlopen'ed:"
      echo
      echo '```'
      echo "$forbidden"
      echo '```'
      echo
    } >> "$report"
  else
    echo "No libwayland*, libxkbcommon* or libX11* is hard-linked." >> "$report"
    echo >> "$report"
  fi
  {
    echo "\`ldd\` (information):"
    echo
    echo '```'
    ldd "$binary" 2>&1 || true
    echo '```'
    echo
    echo "## 3. x11-dl's build-time library directories"
    echo
  } >> "$report"
  local -a configs
  mapfile -t configs < <(find "$target_dir/release/build" -path '*/x11-dl-*/out/config.rs' -type f 2> /dev/null | sort)
  if [ "${#configs[@]}" -eq 0 ]; then
    failed=1
    echo "**Fail:** no \`x11-dl-*/out/config.rs\` under \`$target_dir/release/build\`." >> "$report"
  else
    local config
    for config in "${configs[@]}"; do
      if grep -q 'Some(' "$config"; then
        failed=1
        {
          echo "**Fail:** \`${config#"$target_dir"/}\` carries build-host library directories:"
          echo
          echo '```'
          grep 'Some(' "$config"
          echo '```'
        } >> "$report"
      else
        echo "\`${config#"$target_dir"/}\`: every library directory is \`None\`." >> "$report"
      fi
    done
  fi

  {
    echo
    if [ "$failed" -eq 0 ]; then
      echo "**Verdict: pass.**"
    else
      echo "**Verdict: fail** (see the sections above)."
    fi
  } >> "$report"
  cat "$report"
  [ "$failed" -eq 0 ] || exit 1
}

case "${1-}" in
  env) shift; [ "$#" -eq 0 ] || die "usage: release-check.sh env"; mode_env ;;
  scan) shift; mode_scan "$@" ;;
  *) die "usage: release-check.sh env | scan <binary> <target-dir> <report.md>" ;;
esac
