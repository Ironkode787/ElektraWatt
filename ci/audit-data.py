#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""The canary of the shipped binaries' embedded dependency list (WP-S6.07 D12 (b); doc 18 CI-R-09; NFR-SEC-13),
run by the OS jobs of .github/workflows/package.yml on every binary that an artefact ships.

  python3 -I ci/audit-data.py <binary>...      # checks each binary
  python3 -I ci/audit-data.py --test           # its offline unit test (minimal ELF and PE images built in memory)

`cargo auditable build` embeds the binary's dependency list as zlib-compressed JSON in a section named `.dep-v0`
(ELF and PE alike). For each binary this script reads that section, fails when it is missing ("no cargo-auditable data
in <file>"), and fails when the list names a test-only crate, `egui_mcp`, `ew-testids` or `ew-testkit` ("<file>
contains the test-only crate <name>"); else it prints how many packages the list holds. ELF (64-bit, little-endian)
and PE images are read; anything else is refused. A binary that fails is named in one line, without a traceback, and
the next one is checked; the exit status is 1 when any failed (C50-p2#7). Standard library only.
"""
import argparse
import contextlib
import io
import json
import os
import shutil
import struct
import sys
import tempfile
import unittest
import zlib

SECTION = b".dep-v0"
BANNED = ("egui_mcp", "ew-testids", "ew-testkit")


class Refusal(Exception):
    """A failed check, with the message the script prints."""


def elf_section(data, name):
    """The bytes of the section `name` of a 64-bit little-endian ELF image, or None."""
    if data[4] != 2 or data[5] != 1:
        raise Refusal("not a 64-bit little-endian ELF image")
    shoff = struct.unpack_from("<Q", data, 0x28)[0]
    shentsize, shnum, shstrndx = struct.unpack_from("<HHH", data, 0x3A)
    headers = [struct.unpack_from("<IIQQQQ", data, shoff + i * shentsize) for i in range(shnum)]
    strtab = headers[shstrndx]
    for sh_name, _type, _flags, _addr, offset, size in headers:
        start = strtab[4] + sh_name
        if data[start:data.index(b"\0", start)] == name:
            return data[offset:offset + size]
    return None


def pe_section(data, name):
    """The bytes of the section `name` (at most 8 bytes) of a PE image, or None."""
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe:pe + 4] != b"PE\0\0":
        raise Refusal("an MZ image without the PE signature")
    count = struct.unpack_from("<H", data, pe + 6)[0]
    optional = struct.unpack_from("<H", data, pe + 20)[0]
    table = pe + 24 + optional
    for i in range(count):
        raw_name, virtual_size, _address, raw_size, raw_offset = struct.unpack_from("<8sIIII", data, table + 40 * i)
        if raw_name.rstrip(b"\0") == name:
            return data[raw_offset:raw_offset + min(virtual_size or raw_size, raw_size)]
    return None


def packages(path):
    """The names of the packages of the binary's embedded dependency list."""
    with open(path, "rb") as f:
        data = f.read()
    try:
        if data[:4] == b"\x7fELF":
            section = elf_section(data, SECTION)
        elif data[:2] == b"MZ":
            section = pe_section(data, SECTION)
        else:
            raise Refusal("neither an ELF nor a PE image")
    except (struct.error, ValueError, IndexError, OverflowError) as error:
        raise Refusal(f"{path} is no readable image: {error}") from error
    except Refusal as refusal:
        raise Refusal(f"{path}: {refusal}") from refusal
    if section is None:
        raise Refusal(f"no cargo-auditable data in {path}")
    try:
        text = zlib.decompressobj().decompress(section)
        listed = json.loads(text)["packages"]
        if not isinstance(listed, list):
            raise TypeError(f"`packages` is a {type(listed).__name__}, not a list")
        return [p["name"] for p in listed]
    except (zlib.error, ValueError, KeyError, TypeError, RecursionError) as error:
        raise Refusal(f"the cargo-auditable data of {path} is unreadable: {error!r}") from error


def check(path):
    """D12 (b): the binary carries its dependency list and names no test-only crate. Returns the package count."""
    names = packages(path)
    for name in BANNED:
        if name in names:
            raise Refusal(f"{path} contains the test-only crate {name}")
    return len(names)


def main(argv):
    parser = argparse.ArgumentParser(prog="audit-data.py", description=__doc__.split("\n")[0])
    parser.add_argument("--test", action="store_true", help="run the offline unit test")
    parser.add_argument("binaries", nargs="*")
    args = parser.parse_args(argv)
    if args.test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(AuditDataTest)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    if not args.binaries:
        parser.error("give the binaries, or --test")
    failed = 0
    for path in args.binaries:
        try:
            count = check(path)
            print(f"audit-data: {path}: {count} packages in its cargo-auditable data, no test-only crate")
        except Refusal as refusal:
            print(f"audit-data: {refusal}", file=sys.stderr)
            failed = 1
    return failed


def dep_list(*names):
    """A compressed dependency list as cargo-auditable writes it."""
    listed = [{"name": n, "version": "1.0.0", "source": "crates.io"} for n in names]
    listed[0]["root"] = True
    return zlib.compress(json.dumps({"packages": listed}).encode(), 7)


def elf_image(sections):
    """A minimal 64-bit little-endian ELF image with the sections `sections` (name, bytes) and a string table."""
    names = b"\0" + b"".join(n + b"\0" for n, _ in sections) + b".shstrtab\0"
    body = b"".join(d for _, d in sections) + names
    shoff = 64 + len(body)
    headers = [b"\0" * 64]
    offset, name_at = 64, 1
    for name, payload in sections + [(b".shstrtab", names)]:
        headers.append(struct.pack("<IIQQQQIIQQ", name_at, 1, 0, 0, offset, len(payload), 0, 0, 1, 0))
        offset += len(payload)
        name_at += len(name) + 1
    header = bytearray(64)
    header[:6] = b"\x7fELF\x02\x01"
    struct.pack_into("<Q", header, 0x28, shoff)
    struct.pack_into("<HHH", header, 0x3A, 64, len(headers), len(headers) - 1)
    return bytes(header) + body + b"".join(headers)


def pe_image(sections):
    """A minimal PE image (DOS header, signature, COFF header, no optional header) with the sections `sections`."""
    table = 0x40 + 4 + 20
    offset = table + 40 * len(sections)
    head = bytearray(table)
    head[:2] = b"MZ"
    struct.pack_into("<I", head, 0x3C, 0x40)
    head[0x40:0x44] = b"PE\0\0"
    struct.pack_into("<HH", head, 0x44, 0x8664, len(sections))
    entries, data = b"", b""
    for name, payload in sections:
        padded = payload + b"\0" * (-len(payload) % 512)
        entries += struct.pack("<8sIIIIIIHHI", name, len(payload), 0x1000, len(padded), offset + len(data), 0, 0, 0, 0, 0)
        data += padded
    return bytes(head) + entries + data


class AuditDataTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp(prefix="audit-data-test-")

    def tearDown(self):
        shutil.rmtree(self.dir, ignore_errors=True)

    def image(self, name, data):
        path = os.path.join(self.dir, name)
        with open(path, "wb") as f:
            f.write(data)
        return path

    def refused(self, path):
        with self.assertRaises(Refusal) as refused:
            check(path)
        return str(refused.exception)

    def test_the_package_list_is_read_from_an_elf_and_a_pe_image(self):
        listed = dep_list("elektrawatt", "serde", "egui")
        elf = self.image("elektrawatt", elf_image([(b".text", b"\x90" * 16), (SECTION, listed)]))
        pe = self.image("elektrawatt.exe", pe_image([(b".text", b"\x90" * 16), (SECTION, listed)]))
        for path in (elf, pe):
            self.assertEqual(packages(path), ["elektrawatt", "serde", "egui"], path)
            self.assertEqual(check(path), 3, path)

    def test_an_image_without_the_section_fails(self):
        for name, data in (("ew", elf_image([(b".text", b"\x90")])), ("ew.exe", pe_image([(b".text", b"\x90")]))):
            path = self.image(name, data)
            self.assertEqual(self.refused(path), f"no cargo-auditable data in {path}")

    def test_an_image_naming_a_test_only_crate_fails(self):
        for banned in BANNED:
            for name, build in (("ew", elf_image), ("ew.exe", pe_image)):
                path = self.image(name, build([(SECTION, dep_list("ew", "serde", banned))]))
                self.assertEqual(self.refused(path), f"{path} contains the test-only crate {banned}")

    def test_a_malformed_image_or_list_is_refused_in_one_line(self):
        # An ELF section table offset past what an index can hold (the mutation probe's OverflowError).
        elf = bytearray(elf_image([(SECTION, dep_list("ew"))]))
        struct.pack_into("<Q", elf, 0x28, 2**63)
        path = self.image("ew", bytes(elf))
        self.assertEqual(self.refused(path), f"{path} is no readable image: Python int too large to convert to C "
                                             "ssize_t")
        for listed, error in ((5, "TypeError('`packages` is a int, not a list')"),
                              ({"name": "ew"}, "TypeError('`packages` is a dict, not a list')"),
                              ("ew", "TypeError('`packages` is a str, not a list')"),
                              ([{"version": "1.0.0"}], "KeyError('name')"),
                              (["ew"], 'TypeError("string indices must be integers, not \'str\'")')):
            path = self.image("ew", elf_image([(SECTION, zlib.compress(json.dumps({"packages": listed}).encode()))]))
            self.assertEqual(self.refused(path), f"the cargo-auditable data of {path} is unreadable: {error}", listed)

    def test_every_binary_is_checked_and_a_failure_fails_the_run(self):
        bad = bytearray(elf_image([(SECTION, dep_list("ew"))]))
        struct.pack_into("<Q", bad, 0x28, 2**63)
        bad = self.image("bad", bytes(bad))
        listless = self.image("listless", elf_image([(SECTION, zlib.compress(b'{"packages": 5}'))]))
        good = self.image("good", elf_image([(SECTION, dep_list("ew", "serde"))]))
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            self.assertEqual(main([bad, listless, good]), 1)
        self.assertEqual(err.getvalue(), f"audit-data: {bad} is no readable image: Python int too large to convert "
                                         f"to C ssize_t\naudit-data: the cargo-auditable data of {listless} is "
                                         "unreadable: TypeError('`packages` is a int, not a list')\n")
        self.assertEqual(out.getvalue(), f"audit-data: {good}: 2 packages in its cargo-auditable data, no test-only "
                                         "crate\n")

    def test_other_files_are_refused(self):
        path = self.image("notes.txt", b"plain text")
        self.assertEqual(self.refused(path), f"{path}: neither an ELF nor a PE image")
        path = self.image("ew", elf_image([(SECTION, b"not zlib")]))
        self.assertTrue(self.refused(path).startswith(f"the cargo-auditable data of {path} is unreadable: "))


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
