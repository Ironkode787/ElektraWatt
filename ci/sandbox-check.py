#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""The verdict of the network sandbox (WP-S6.05 D4, D5; doc 18 section 12.2, CI-N-06; NFR-PRIV-01, NFR-SEC-09).

  python3 -I ci/sandbox-check.py scan <flow>.strace...      the syscalls of each flow, traced with strace -f
  python3 -I ci/sandbox-check.py canary <binary>...         the strings of each release binary
  python3 -I ci/sandbox-check.py --test                     its offline unit test

`scan` reads the `strace -f -e trace=socket,connect,bind,listen,sendto,sendmsg,sendmmsg,openat` output of each flow
(the flow is the file name without `.strace`) and fails on:

  inet-socket    a `socket` of AF_INET or AF_INET6
  inet-traffic   a `connect`, `sendto`, `sendmsg` or `sendmmsg` to an AF_INET or AF_INET6 address
  resolver       a `connect` to the resolver's socket (/run/systemd/resolve/*, /run/nscd/*, /var/run/nscd/*)
  listen         any `listen`
  inet-bind      a `bind` of AF_INET or AF_INET6
  lookup-file    an `openat` of /etc/resolv.conf, /etc/hosts or /etc/gai.conf (the files a name lookup reads, in
                 place of doc 18's LD_PRELOAD shim)

Other AF_UNIX traffic (D-Bus) and AF_NETLINK pass. There is no allowlist: 0.1.0 has no `ew mcp`. `canary` fails when a
binary holds the bytes `EW_DEBUG_PANIC` (the forced panic of a debug build, WP-S6.05 D9) or `egui_mcp` (the test bridge,
CI-PR-09). Each failure prints the flow or file, the rule and the syscall line; the exit code is 1 on any failure, 2 on
a usage error. Standard library only.
"""
import os
import re
import sys
import tempfile
import unittest

# The syscall of a line: after strace -f's pid and an optional time, up to its "(" (a resumed call names it inside
# "<... name resumed>").
CALL = re.compile(r"^(?:\[pid\s+)?\d*\]?\s*(?:\d+(?:\.\d+)?\s+)?(?:<\.\.\.\s+)?([a-z_0-9]+)(?:\(|\s+resumed>)")
INET = re.compile(r"\bAF_INET6?\b")
INET_ADDRESS = re.compile(r"sa_family=AF_INET6?\b")
UNIX_PATH = re.compile(r'sun_path="([^"]*)"')
OPENED = re.compile(r'openat\([^,]*,\s*"([^"]*)"')

RESOLVER_SOCKETS = ("/run/systemd/resolve/", "/run/nscd/", "/var/run/nscd/")
LOOKUP_FILES = ("/etc/resolv.conf", "/etc/hosts", "/etc/gai.conf")
CANARIES = (b"EW_DEBUG_PANIC", b"egui_mcp")


def findings(line):
    """The rules `line` (one strace line) breaks, in order."""
    m = CALL.match(line)
    if not m:
        return []
    call = m.group(1)
    rules = []
    if call == "socket" and INET.search(line.split(",", 1)[0]):
        rules.append("inet-socket")
    if call in ("connect", "sendto", "sendmsg", "sendmmsg") and INET_ADDRESS.search(line):
        rules.append("inet-traffic")
    if call == "connect":
        path = UNIX_PATH.search(line)
        if path and path.group(1).startswith(RESOLVER_SOCKETS):
            rules.append("resolver")
    if call == "listen":
        rules.append("listen")
    if call == "bind" and INET_ADDRESS.search(line):
        rules.append("inet-bind")
    if call == "openat":
        opened = OPENED.search(line)
        if opened and opened.group(1) in LOOKUP_FILES:
            rules.append("lookup-file")
    return rules


def scan_text(flow, text):
    """The failure lines of one flow's trace."""
    out = []
    for line in text.splitlines():
        for rule in findings(line):
            out.append(f"{flow}: {rule}: {line.strip()}")
    return out


def scan(paths):
    out = []
    for path in paths:
        flow = os.path.basename(path)
        if flow.endswith(".strace"):
            flow = flow[: -len(".strace")]
        with open(path, encoding="utf-8", errors="replace") as f:
            out.extend(scan_text(flow, f.read()))
    return out


def canary(paths):
    out = []
    for path in paths:
        with open(path, "rb") as f:
            data = f.read()
        for needle in CANARIES:
            if needle in data:
                out.append(f"{path}: canary: holds the bytes {needle.decode()}")
    return out


def main(argv):
    if argv == ["--test"]:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(Tests)
        result = unittest.TextTestRunner(verbosity=1).run(suite)
        return 0 if result.wasSuccessful() else 1
    if len(argv) < 2 or argv[0] not in ("scan", "canary"):
        print(__doc__.strip(), file=sys.stderr)
        return 2
    failures = scan(argv[1:]) if argv[0] == "scan" else canary(argv[1:])
    for f in failures:
        print(f)
    if failures:
        print(f"sandbox-check {argv[0]}: {len(failures)} finding(s)", file=sys.stderr)
        return 1
    print(f"sandbox-check {argv[0]}: {len(argv) - 1} file(s), no finding")
    return 0


class Tests(unittest.TestCase):
    def test_each_network_syscall_fails_with_its_flow_line_and_rule(self):
        trace = "\n".join(
            [
                '4321 socket(AF_INET, SOCK_STREAM|SOCK_CLOEXEC, IPPROTO_TCP) = 3',
                '4321 connect(4, {sa_family=AF_UNIX, sun_path="/run/systemd/resolve/io.systemd.Resolve"}, 42) = 0',
                '4321 openat(AT_FDCWD, "/etc/resolv.conf", O_RDONLY|O_CLOEXEC) = 5',
                '4322 listen(3, 128) = 0',
                '4322 bind(3, {sa_family=AF_INET, sin_port=htons(0), sin_addr=inet_addr("127.0.0.1")}, 16) = 0',
            ]
        )
        self.assertEqual(
            scan_text("headless", trace),
            [
                "headless: inet-socket: 4321 socket(AF_INET, SOCK_STREAM|SOCK_CLOEXEC, IPPROTO_TCP) = 3",
                'headless: resolver: 4321 connect(4, {sa_family=AF_UNIX, sun_path="/run/systemd/resolve/io.systemd.Resolve"}, 42) = 0',
                'headless: lookup-file: 4321 openat(AT_FDCWD, "/etc/resolv.conf", O_RDONLY|O_CLOEXEC) = 5',
                "headless: listen: 4322 listen(3, 128) = 0",
                'headless: inet-bind: 4322 bind(3, {sa_family=AF_INET, sin_port=htons(0), sin_addr=inet_addr("127.0.0.1")}, 16) = 0',
            ],
        )

    def test_inet6_traffic_and_a_resumed_socket_fail(self):
        trace = "\n".join(
            [
                '77 sendto(3, "x", 1, 0, {sa_family=AF_INET6, sin6_port=htons(53)}, 28) = 1',
                '77 <... socket resumed>) = 3',
                '77 socket(AF_INET6, SOCK_DGRAM <unfinished ...>',
            ]
        )
        self.assertEqual(
            [f.split(": ")[1] for f in scan_text("ew-sql", trace)],
            ["inet-traffic", "inet-socket"],
        )

    def test_dbus_and_netlink_and_other_files_pass(self):
        trace = "\n".join(
            [
                '9 socket(AF_UNIX, SOCK_STREAM|SOCK_CLOEXEC, 0) = 3',
                '9 connect(3, {sa_family=AF_UNIX, sun_path="/run/user/1001/bus"}, 110) = 0',
                '9 sendmsg(3, {msg_name=NULL, msg_namelen=0, msg_iov=[{iov_base="\\0AUTH", iov_len=6}]}, MSG_NOSIGNAL) = 6',
                '9 socket(AF_NETLINK, SOCK_RAW|SOCK_CLOEXEC, NETLINK_ROUTE) = 4',
                '9 bind(4, {sa_family=AF_NETLINK, nl_pid=0, nl_groups=00000000}, 12) = 0',
                '9 openat(AT_FDCWD, "/etc/hostname", O_RDONLY) = 5',
                '9 openat(AT_FDCWD, "/home/runner/work/gp.ew/manifest.json", O_RDONLY|O_CLOEXEC) = 6',
                '9 +++ exited with 0 +++',
            ]
        )
        self.assertEqual(scan_text("smoke", trace), [])

    def test_the_canary_fails_on_the_trigger_and_passes_without_it(self):
        with tempfile.TemporaryDirectory() as d:
            debug = os.path.join(d, "debug")
            release = os.path.join(d, "release")
            with open(debug, "wb") as f:
                f.write(b"\x7fELF\0\0forced panic (EW_DEBUG_PANIC)\0")
            with open(release, "wb") as f:
                f.write(b"\x7fELF\0\0EW_CONFIG_DIR\0EW_DEBUG\0")
            self.assertEqual(canary([debug]), [f"{debug}: canary: holds the bytes EW_DEBUG_PANIC"])
            self.assertEqual(canary([release]), [])

    def test_the_flow_is_the_file_name(self):
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "ew-check.strace")
            with open(path, "w", encoding="utf-8") as f:
                f.write("1 listen(3, 1) = 0\n")
            self.assertEqual(scan([path]), ["ew-check: listen: 1 listen(3, 1) = 0"])


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
