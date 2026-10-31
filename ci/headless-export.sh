#!/bin/sh
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

# Runs INSIDE the headless container of CI-PR-05 (FT-295, ci/README.md): the container is a bare Linux image with no
# display, no GPU, no system fonts and no user directories of its own. The script first proves that the container is
# bare (a check that fails the job when the image or the run flags change under it), then runs the shipped
# `ew export pdf` over the golden project on one thread and on four, and `ew render --page 103 --png` (NFR-PLAT-04),
# with every user directory pointing at an empty directory and with the environment cleared. The host compares the
# two PDFs with `expected/pdf/sha256.txt` and the PNG with the digest of `expected/png/103.png`.
#
# The checks run as ROOT with CAP_DAC_READ_SEARCH (the job adds that capability and nothing else but the two it
# needs to drop privileges), so that the font search can read every directory of the image; the export then
# runs as the runner's uid and gid (EW_UID, EW_GID, set by the job) through `setpriv`, so that the PDFs in /out are
# the runner's. The font search also fails when `find` writes anything to stderr: an unreadable directory would
# otherwise be skipped silently and the proof would have a hole.
#
# Mounts (read-only except /out): /ew the `ew` binary, /project golden/J-E-2416.ew, /out the output directory.
# POSIX sh only: the image has no bash.
set -eu

fail() {
    echo "headless: $*" >&2
    exit 1
}

# No display.
[ -z "${DISPLAY:-}" ] || fail "DISPLAY is set"
[ -z "${WAYLAND_DISPLAY:-}" ] || fail "WAYLAND_DISPLAY is set"

# No GPU: no DRM render node, no vendor device node.
for node in /dev/dri /dev/nvidiactl /dev/nvidia0 /dev/kfd; do
    [ ! -e "$node" ] || fail "$node exists: the container sees a GPU"
done

# No system fonts: no font file anywhere on the image's own file systems (the bind mounts are other file
# systems), no font directory, no fontconfig.
[ -n "${EW_UID:-}" ] && [ -n "${EW_GID:-}" ] || fail "EW_UID and EW_GID must name the user of the export"
[ "$(id -u)" = 0 ] || fail "the checks must run as root, so that the font search reads every directory"
find_errors=/tmp/find-errors.txt
# `find` exits non-zero when it could not read a directory; its stderr is what counts, and it must be empty.
fonts="$(find / -xdev -type f \( -iname '*.ttf' -o -iname '*.otf' -o -iname '*.ttc' -o -iname '*.otc' \
    -o -iname '*.pfb' -o -iname '*.pfa' -o -iname '*.woff' -o -iname '*.woff2' -o -iname '*.pcf' \
    -o -iname '*.pcf.gz' \) 2>"$find_errors" || true)"
[ ! -s "$find_errors" ] || fail "the font search could not read everything: $(head -n 5 "$find_errors")"
[ -z "$fonts" ] || fail "font files on the image: $fonts"
for path in /usr/share/fonts /usr/local/share/fonts /etc/fonts /usr/bin/fc-list /usr/bin/fc-cache; do
    [ ! -e "$path" ] || fail "$path exists"
done

# Empty library and user directories (the application reads the user library and the starter content from them).
home=/tmp/empty-home
for dir in home appdata state data config; do
    mkdir -p "$home/$dir"
    chmod 0777 "$home/$dir"
done
for dir in "$home"/*; do
    [ -z "$(ls -A "$dir")" ] || fail "$dir is not empty"
done

echo "headless: bare container confirmed (no display, no GPU, no font file, empty user directories)"
command -v setpriv >/dev/null || fail "setpriv (util-linux) is not on the image"
setpriv --reuid "$EW_UID" --regid "$EW_GID" --clear-groups /ew --version

# The export runs as the runner's user, not as root. `env -i` clears everything else: no EW_CONTENT_DIR, no
# SOURCE_DATE_EPOCH, no locale.
run_ew() {
    setpriv --reuid "$EW_UID" --regid "$EW_GID" --clear-groups \
        env -i PATH=/usr/bin:/bin HOME="$home/home" USERPROFILE="$home/home" APPDATA="$home/appdata" \
        LOCALAPPDATA="$home/appdata" XDG_STATE_HOME="$home/state" XDG_DATA_HOME="$home/data" \
        XDG_CONFIG_HOME="$home/config" \
        /ew "$@"
}
for threads in 1 4; do
    run_ew export pdf /project --out "/out/golden-$threads.pdf" --threads "$threads"
done
# NFR-PLAT-04: one page as PNG on the CPU with the bundled fonts, at the golden PNGs' 150 dpi.
run_ew render /project --page 103 --png --out /out/103.png
ls -l /out
