#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""The CycloneDX SBOM of each release artefact and the non-crate part of its notices (WP-S6.07 D9; doc 18 CI-R-07;
NFR-SEC-13, NFR-LIC-08, FT-04), run by the OS jobs of .github/workflows/package.yml.

  python3 -I ci/sbom.py cyclonedx --target <triple> --bin <name>... --out <dir>
  python3 -I ci/sbom.py shipped --target <triple> --bin <name>... [--root <name>] --out <tree.txt>
  python3 -I ci/sbom.py merge --artefact <file name> --version <v> --metadata <json> --tree <tree.txt>
                              --cdx <json>... [--component <name>...] --out <artefact>.cdx.json
  python3 -I ci/sbom.py complete --sbom <artefact>.cdx.json --metadata <json> --tree <tree.txt> --bin <name>...
  python3 -I ci/sbom.py notices --sbom <artefact>.cdx.json... --metadata <json> --notices <THIRD-PARTY-NOTICES.txt>
  python3 -I ci/sbom.py --test                                          # its offline unit test

`cyclonedx` writes `cargo metadata --filter-platform <triple>` to <dir>/metadata.json and runs cargo-cyclonedx (JSON,
CycloneDX 1.5, every dependency, one SBOM per binary of the workspace for <triple>); it moves the SBOM of each `--bin`
to <dir>/<name>.cdx.json and deletes the other files it wrote beside the workspace's manifests, so the checkout stays
clean. `shipped` writes the artefact's shipped set (ID-1318): `cargo tree -e normal,build -p <bins> --target <triple>
--prefix none --format "{p}"`, cargo's own feature resolution, which leaves out the optional dependencies that
`cargo metadata` resolves when only a weak feature (`dep?/feature`) names them; with `--root <name>`, the part of that
joint graph that the binary <name> reaches, since the artefact's <name> is built in the one joint invocation whose
features cargo unifies (C50-p2#2). `merge` joins the binaries' SBOMs of
one artefact into one (one component per purl, the binaries as application components, the dependency graph's union),
keeps only the crate components of the shipped set and prints one line per component it drops, gives each `bom-ref`
of a workspace crate its path relative to the workspace (no checkout path), and adds the non-crate components: each file block of a
"Bundled ..." section of NOTICE (the fonts, with family, version, licence, copyright, source and SHA-256), a
section's single component where it has no file block (the Tabler icons, with their version), SQLite at the version of
the `sqlite3.h` that libsqlite3-sys bundles (found through its manifest path in <json>, when the binaries' graph holds
it with its `bundled` feature), and the components of packaging/components.toml that the artefact holds (`--component`:
NSIS's stub in the setup, the type-2 runtime in the AppImage). `complete` fails on any package of the shipped set that
the SBOM lacks, and on any package of the shipped set outside the binaries' closure in <json> (normal and build
dependencies), naming each. `notices` appends a "Non-crate components" section
to the cargo-about output <THIRD-PARTY-NOTICES.txt> (each non-crate component of the SBOMs with its licence, then the
NOTICE section it comes from or that names it, SQLite's dedication included) and fails, naming each, on any component
of the SBOMs that is not one of the workspace's own crates and that the file does not name as `- <name> <version>`, and
on any non-crate component without a NOTICE section whose packaging/components.toml entry gives no `no_notice`, the
reason it needs none (C50-p2#3). A components.toml entry's `shipped_licence`, where it has one, is the licence of the
part that ships, its `licence` that of the whole download.
NOTICE, packaging/components.toml and the bundled header are the one source of each non-crate component; the
repository is the one this script lives in. Standard library only.
"""
import argparse
import contextlib
import datetime
import io
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest
import urllib.parse
import urllib.request
import uuid

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
NOTICE = "NOTICE"
COMPONENTS = os.path.join("packaging", "components.toml")
SECTION = "elektrawatt:notice-section"
NON_CRATE = "Non-crate components"
RULE = "-" * 114


class Refusal(Exception):
    """A failed check, with the message the script prints."""


def run(command, cwd=None):
    """Runs `command`: its standard output as text, or a Refusal that names it and carries its standard error."""
    done = subprocess.run(command, cwd=cwd, capture_output=True, text=True, check=False)
    if done.returncode != 0:
        raise Refusal(f"`{' '.join(command)}` failed (exit {done.returncode}):\n{done.stderr.strip()}")
    return done.stdout


def read_json(path):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def read_tree(path):
    """The shipped set of the `shipped` file at `path`."""
    with open(path, encoding="utf-8") as f:
        return shipped_set(f.read())


def write_atomic(path, text):
    """Writes `text` to `path` through a temporary file in its folder, renamed over it."""
    folder = os.path.dirname(os.path.abspath(path))
    fd, partial = tempfile.mkstemp(dir=folder, prefix=".sbom-")
    try:
        with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as f:
            f.write(text)
        os.replace(partial, path)
    finally:
        if os.path.exists(partial):
            os.remove(partial)


# ---------------------------------------------------------------------------------------------------------------
# cargo metadata


def bin_package(metadata, name):
    """The id of the workspace package that has the binary target `name`."""
    packages = {p["id"]: p for p in metadata["packages"]}
    for member in metadata["workspace_members"]:
        if any(t["name"] == name and "bin" in t["kind"] for t in packages[member]["targets"]):
            return member
    raise Refusal(f"no workspace package has the binary {name}")


def normal_closure(metadata, bins, kinds=(None,)):
    """The ids of the packages that the binaries `bins` reach through normal dependencies (with `kinds` `(None,
    "build")`, through build dependencies too; the platform filter is the metadata's own, `--filter-platform`), their
    own packages included."""
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    seen = set()
    stack = [bin_package(metadata, b) for b in bins]
    while stack:
        node = stack.pop()
        if node in seen:
            continue
        seen.add(node)
        for dep in nodes[node]["deps"]:
            if any(kind["kind"] in kinds for kind in dep["dep_kinds"]):
                stack.append(dep["pkg"])
    return seen


def shipped_set(text):
    """The (name, version) pairs of `cargo tree --prefix none --format "{p}"`'s lines (`<name> v<version>[ ...]`)."""
    found = set()
    for line in text.splitlines():
        if not line.strip():
            continue
        match = re.match(r"(\S+) v(\S+)", line)
        if not match:
            raise Refusal(f"not a line of cargo tree's {{p}} format: {line}")
        found.add((match.group(1), match.group(2)))
    return found


def root_block(text, root):
    """The (name, version) pairs that the root `root` reaches in `cargo tree --prefix depth --format "{p}"` of several
    roots. cargo shows a package's dependencies once in the whole output and marks each later showing `(*)`, so the
    dependencies of each package are taken from wherever they are shown."""
    deps, roots, path = {}, [], []
    for line in text.splitlines():
        if not line.strip():
            continue
        match = re.match(r"(\d+)(\S+) v(\S+)", line)
        if not match or int(match.group(1)) > len(path):
            raise Refusal(f"not a line of cargo tree's depth-prefixed {{p}} format: {line}")
        depth, key = int(match.group(1)), (match.group(2), match.group(3))
        del path[depth:]
        if path:
            deps[path[-1]].add(key)
        else:
            roots.append(key)
        deps.setdefault(key, set())
        path.append(key)
    start = [key for key in roots if key[0] == root]
    if not start:
        raise Refusal(f"cargo tree shows no root {root}")
    found, stack = set(), start
    while stack:
        key = stack.pop()
        if key not in found:
            found.add(key)
            stack.extend(deps[key])
    return found


def relative_ref(ref, root):
    """A `path+file://` package id (cargo's id of a workspace crate) with its path relative to the workspace root
    `root`, as `workspace/<path>#<fragment>`; any other reference as it is."""
    if not ref or not ref.startswith("path+file://"):
        return ref
    location, _, fragment = ref[len("path+"):].partition("#")
    path = urllib.request.url2pathname(urllib.parse.urlparse(location).path)
    relative = os.path.relpath(os.path.normpath(path), os.path.normpath(root)).replace(os.sep, "/")
    return f"workspace/{relative}#{fragment}"


def workspace_crates(metadata):
    """(name, version) of each workspace member: ElektraWatt's own crates, not third-party components."""
    packages = {p["id"]: p for p in metadata["packages"]}
    return {(packages[m]["name"], packages[m]["version"]) for m in metadata["workspace_members"]}


# ---------------------------------------------------------------------------------------------------------------
# The non-crate components


def notice_sections(text):
    """NOTICE's sections, in order: (title, body), a title being a line underlined by `-` (three or more)."""
    lines = text.splitlines()
    heads = [i for i in range(len(lines) - 1)
             if lines[i].strip() and re.fullmatch(r"-{3,}", lines[i + 1].strip())]
    sections = []
    for k, head in enumerate(heads):
        end = heads[k + 1] if k + 1 < len(heads) else len(lines)
        body = "\n".join(lines[head + 2:end]).strip("\n")
        sections.append((lines[head].strip(), body))
    return sections


def licence(expression):
    """A CycloneDX licence entry: an SPDX id, or an expression."""
    if re.fullmatch(r"[A-Za-z0-9.+-]+", expression):
        return [{"license": {"id": expression}}]
    return [{"expression": expression}]


def file_component(ref, name, version, spdx, keys, title):
    """A component of a NOTICE section: its keys `Copyright`, `Source` and `SHA-256` (the first word of each)."""
    component = {"type": "file", "bom-ref": ref, "name": name, "version": version, "licenses": licence(spdx)}
    if keys.get("Copyright"):
        component["copyright"] = keys["Copyright"]
    if keys.get("SHA-256"):
        component["hashes"] = [{"alg": "SHA-256", "content": keys["SHA-256"].split()[0]}]
    if keys.get("Source"):
        component["externalReferences"] = [{"type": "distribution", "url": keys["Source"].split()[0]}]
    component["properties"] = [{"name": SECTION, "value": title}]
    return component


def notice_components(text):
    """The components of NOTICE's "Bundled ..." sections: one per file block (a path line, then indented `Key: value`
    lines), or, without file blocks, the one the section's first sentence names (`<name>, version <v>,`). The licence
    is the section's `LICENSES/<id>.txt`."""
    components = []
    for title, body in notice_sections(text):
        if not title.startswith("Bundled "):
            continue
        spdx = re.search(r"LICENSES/([A-Za-z0-9.+-]+)\.txt", body)
        if not spdx:
            raise Refusal(f"NOTICE's section \"{title}\" names no LICENSES/<id>.txt")
        found = []
        lines = body.splitlines()
        for i, line in enumerate(lines):
            if re.fullmatch(r"assets/\S+", line):
                keys = {}
                for item in lines[i + 1:]:
                    pair = re.fullmatch(r"\s+([A-Za-z0-9-]+):\s+(.*)", item)
                    if not pair:
                        break
                    keys[pair.group(1)] = pair.group(2).strip()
                version = re.sub(r"^Version\s+", "", keys.get("Version", ""))
                if not keys.get("Family") or not version:
                    raise Refusal(f"NOTICE's block {line} has no Family or Version")
                found.append(file_component(f"notice:{line}", keys["Family"], version, spdx.group(1), keys, title))
        if not found:
            named = re.match(r"([^,\n]+), version ([^,\s]+),", body)
            if not named:
                raise Refusal(f"NOTICE's section \"{title}\" names no component")
            keys = dict(re.findall(r"^\s+([A-Za-z0-9-]+):\s+(.*)$", body, re.M))
            found.append(file_component(f"notice:{named.group(1)}", named.group(1), named.group(2), spdx.group(1),
                                        keys, title))
        components.extend(found)
    return components


def section_naming(sections, word):
    """The title of the first NOTICE section whose title holds `word`, or None."""
    return next((title for title, _ in sections if word in title), None)


def sqlite_component(metadata, closure, sections):
    """SQLite at the version of the sqlite3.h that libsqlite3-sys bundles, when the closure holds that crate with its
    `bundled` feature; else None."""
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    for node in sorted(closure):
        package = packages[node]
        if package["name"] != "libsqlite3-sys" or "bundled" not in nodes[node]["features"]:
            continue
        header = os.path.join(os.path.dirname(package["manifest_path"]), "sqlite3", "sqlite3.h")
        try:
            with open(header, encoding="utf-8", errors="replace") as f:
                version = re.search(r'^#define SQLITE_VERSION\s+"([^"]+)"', f.read(), re.M)
        except OSError:
            version = None
        if not version:
            raise Refusal(f"no SQLITE_VERSION in {header}, the header libsqlite3-sys {package['version']} bundles")
        title = section_naming(sections, "SQLite")
        if title is None:
            raise Refusal("NOTICE has no section naming SQLite, whose dedication the notices carry")
        return {
            "type": "library", "bom-ref": "sqlite", "name": "SQLite", "version": version.group(1),
            "licenses": licence("blessing"), "purl": f"pkg:generic/sqlite@{version.group(1)}",
            "description": f"bundled by libsqlite3-sys {package['version']} (sqlite3/sqlite3.h) and compiled into it",
            "properties": [{"name": SECTION, "value": title}],
        }
    return None


def packaging_component(repo, name, sections):
    """The component `name` of packaging/components.toml."""
    with open(os.path.join(repo, COMPONENTS), "rb") as f:
        table = tomllib.load(f)
    for entry in table["component"]:
        if entry["name"] == name:
            component = {"type": "application", "bom-ref": f"component:{name}", "name": name,
                         "version": entry["version"],
                         "licenses": licence(entry.get("shipped_licence", entry["licence"])),
                         "description": entry["used_for"],
                         "externalReferences": [{"type": "distribution", "url": entry["url"]}]}
            if entry["sha256"]:
                component["hashes"] = [{"alg": "SHA-256", "content": entry["sha256"]}]
            title = section_naming(sections, f"`{name}`")
            component["properties"] = [{"name": SECTION, "value": title or ""}]
            return component
    raise Refusal(f"no component {name} in {COMPONENTS}")


# ---------------------------------------------------------------------------------------------------------------
# The commands


def cyclonedx(target, bins, out, repo=REPO, run=run):
    """cargo metadata and cargo-cyclonedx for `target`; the SBOM of each of `bins` moved to <out>/<bin>.cdx.json."""
    os.makedirs(out, exist_ok=True)
    text = run(["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", target], cwd=repo)
    write_atomic(os.path.join(out, "metadata.json"), text)
    metadata = json.loads(text)

    def written():
        found = set()
        for folder, dirs, files in os.walk(repo):
            dirs[:] = [d for d in dirs if d not in ("target", ".git")]
            found.update(os.path.normpath(os.path.join(folder, f)) for f in files if f.endswith(".cdx.json"))
        return found

    before = written()
    run(["cargo", "cyclonedx", "--format", "json", "--spec-version", "1.5", "--describe", "binaries", "--all",
         "--target", target, "--quiet"], cwd=repo)
    new = written() - before
    try:
        for name in bins:
            folder = os.path.dirname(next(p["manifest_path"] for p in metadata["packages"]
                                          if p["id"] == bin_package(metadata, name)))
            produced = os.path.normpath(os.path.join(folder, f"{name}_bin.cdx.json"))
            if produced not in new:
                raise Refusal(f"cargo-cyclonedx wrote no {produced}")
            os.replace(produced, os.path.join(out, f"{name}.cdx.json"))
            new.discard(produced)
    finally:
        for path in new:
            os.remove(path)


def shipped(target, bins, out, root=None, repo=REPO, run=run):
    """ID-1318: writes `cargo tree -e normal,build -p <bins> --target <target> --prefix none --format "{p}"` to
    `out`; with `root`, the packages that the binary `root` reaches in that graph, one `<name> v<version>` a line."""
    packages = [a for b in bins for a in ("-p", b)]
    text = run(["cargo", "tree", "--locked", "-e", "normal,build", *packages, "--target", target, "--prefix",
                "depth" if root else "none", "--format", "{p}"], cwd=repo)
    if root:
        text = "".join(f"{name} v{version}\n" for name, version in sorted(root_block(text, root)))
    write_atomic(out, text)
    return shipped_set(text)


def merge(artefact, version, cdx_files, metadata, shipped, components=(), repo=REPO):
    """D9: one SBOM for the artefact from its binaries' SBOMs, its crate components those of the shipped set `shipped`
    ((name, version) pairs, ID-1318), with the non-crate components."""
    with open(os.path.join(repo, NOTICE), encoding="utf-8") as f:
        notice = f.read()
    sections = notice_sections(notice)
    merged, refs, graph, bins, tools, properties = [], set(), {}, [], [], []
    for path in cdx_files:
        bom = read_json(path)
        if bom.get("bomFormat") != "CycloneDX" or bom.get("specVersion") != "1.5":
            raise Refusal(f"{path} is no CycloneDX 1.5 document")
        top = dict(bom["metadata"]["component"])
        top["type"] = "application"
        bins.append(top["name"])
        tools = tools or bom["metadata"].get("tools", [])
        properties = properties or bom["metadata"].get("properties", [])
        for component in [top] + bom.get("components", []):
            key = component.get("purl") or component.get("bom-ref")
            if key not in refs:
                refs.add(key)
                merged.append(component)
        for dependency in bom.get("dependencies", []):
            graph.setdefault(dependency["ref"], set()).update(dependency.get("dependsOn", []))
    # ID-1318: what ships; cargo metadata's resolve also holds the optional dependencies of weak features.
    dropped = sorted((c for c in merged if (c.get("name"), c.get("version")) not in shipped),
                     key=lambda c: (c.get("name"), c.get("version")))
    for component in dropped:
        print(f"sbom: {artefact}: dropped {component.get('name')} {component.get('version')}, which cargo tree does "
              f"not list for {' and '.join(bins)}")
    gone = {c.get("bom-ref") for c in dropped}
    merged = [c for c in merged if c.get("bom-ref") not in gone]
    graph = {ref: on - gone for ref, on in graph.items() if ref not in gone}
    # No checkout path: a workspace crate's bom-ref is its path relative to the workspace.
    root = metadata["workspace_root"]
    for component in merged:
        if "bom-ref" in component:
            component["bom-ref"] = relative_ref(component["bom-ref"], root)
    graph = {relative_ref(ref, root): {relative_ref(r, root) for r in on} for ref, on in graph.items()}
    closure = normal_closure(metadata, bins)
    extra = notice_components(notice)
    sqlite = sqlite_component(metadata, closure, sections)
    if sqlite:
        extra.append(sqlite)
    extra.extend(packaging_component(repo, name, sections) for name in components)
    merged.extend(extra)
    return {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": f"urn:uuid:{uuid.uuid4()}",
        "version": 1,
        "metadata": {
            "timestamp": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "tools": with_tool(tools),
            "component": {"type": "application", "bom-ref": artefact, "name": artefact, "version": version},
            "properties": list(properties),
        },
        "components": merged,
        "dependencies": [{"ref": ref, "dependsOn": sorted(on)} for ref, on in sorted(graph.items())],
    }


def with_tool(tools):
    """The input SBOMs' tools with this script added, in either of CycloneDX 1.5's forms (a list, or an object with
    `components`)."""
    if isinstance(tools, dict):
        listed = dict(tools)
        listed["components"] = list(tools.get("components", [])) + [
            {"type": "application", "group": "ElektraWatt", "name": "ci/sbom.py"}]
        return listed
    return list(tools) + [{"vendor": "ElektraWatt", "name": "ci/sbom.py"}]


def complete(sbom, metadata, shipped, bins, name="the SBOM"):
    """D9, ID-1318: every package of the shipped set is a component of the SBOM, and every one lies in the binaries'
    closure of normal and build dependencies in cargo metadata."""
    packages = {p["id"]: p for p in metadata["packages"]}
    have = {(c.get("name"), c.get("version")) for c in sbom.get("components", [])}
    missing = sorted(shipped - have)
    if missing:
        lines = "".join(f"\n  {n} {v}" for n, v in missing)
        raise Refusal(f"{name} lacks {len(missing)} package(s) that cargo tree lists for {' and '.join(bins)}:{lines}")
    closure = {(packages[i]["name"], packages[i]["version"]) for i in normal_closure(metadata, bins, (None, "build"))}
    outside = sorted(shipped - closure)
    if outside:
        lines = "".join(f"\n  {n} {v}" for n, v in outside)
        raise Refusal(f"cargo tree lists {len(outside)} package(s) for {' and '.join(bins)} outside their closure in "
                      f"cargo metadata:{lines}")
    return len(have)


def notices(text, sboms, metadata, repo=REPO):
    """D9, FT-04: the cargo-about output `text` with the "Non-crate components" section; refuses a component of the
    SBOMs that it does not name."""
    if NON_CRATE in text:
        raise Refusal(f"the notices hold a \"{NON_CRATE}\" section already")
    with open(os.path.join(repo, NOTICE), encoding="utf-8") as f:
        sections = notice_sections(f.read())
    components, seen = [], set()
    for sbom in sboms:
        for component in sbom.get("components", []):
            section = next((p["value"] for p in component.get("properties", []) if p["name"] == SECTION), None)
            key = (component["name"], component["version"])
            if section is not None and key not in seen:
                seen.add(key)
                components.append((component, section))

    def line(component):
        entry = component["licenses"][0]
        spdx = entry.get("expression") or entry["license"].get("id") or entry["license"].get("name")
        return f"- {component['name']} {component['version']} ({spdx})\n"

    out = [text.rstrip("\n"), "\n\n", RULE, "\n", NON_CRATE, "\n", RULE, "\n\n",
           "These components of ElektraWatt's releases are not Rust crates. Each is listed with its licence, then its "
           "notice as the repository's NOTICE file states it.\n"]
    for title, body in sections:
        listed = [c for c, s in components if s == title]
        if listed:
            out.append("\n")
            out.extend(line(c) for c in listed)
            out.append(f"\n{title}\n{'-' * len(title)}\n\n{body}\n")
    unsectioned = [c for c, s in components if s not in {t for t, _ in sections}]
    if unsectioned:
        with open(os.path.join(repo, COMPONENTS), "rb") as f:
            reasons = {e["name"]: e.get("no_notice") for e in tomllib.load(f)["component"]}
        bare = sorted({(c["name"], c["version"]) for c in unsectioned if not reasons.get(c["name"])})
        if bare:
            names = "".join(f"\n  {n} {v}" for n, v in bare)
            raise Refusal(f"NOTICE has no section for {len(bare)} component(s) of the SBOM, and "
                          f"packaging/components.toml gives no reason they need none:{names}")
        out.append("\n")
        for component in unsectioned:
            out.extend([line(component), f"  No notice: {reasons[component['name']]}\n"])
    result = "".join(out)
    own = workspace_crates(metadata)
    missing = sorted({(c["name"], c["version"]) for sbom in sboms for c in sbom.get("components", [])
                      if (c["name"], c["version"]) not in own
                      and not re.search(rf"^- {re.escape(c['name'])} {re.escape(c['version'])}(?: \(|$)", result,
                                        re.M)})
    if missing:
        names = "".join(f"\n  {n} {v}" for n, v in missing)
        raise Refusal(f"the notices do not name {len(missing)} component(s) of the SBOM:{names}")
    return result


def main(argv):
    parser = argparse.ArgumentParser(prog="sbom.py", description=__doc__.split("\n")[0])
    parser.add_argument("--test", action="store_true", help="run the offline unit test")
    sub = parser.add_subparsers(dest="command")
    p = sub.add_parser("cyclonedx", help="cargo metadata and the binaries' SBOMs of cargo-cyclonedx")
    p.add_argument("--target", required=True)
    p.add_argument("--bin", action="append", required=True)
    p.add_argument("--out", required=True)
    p = sub.add_parser("shipped", help="the shipped set of the binaries (cargo tree)")
    p.add_argument("--target", required=True)
    p.add_argument("--bin", action="append", required=True)
    p.add_argument("--root")
    p.add_argument("--out", required=True)
    p = sub.add_parser("merge", help="one artefact's SBOM with the non-crate components")
    p.add_argument("--artefact", required=True)
    p.add_argument("--version", required=True)
    p.add_argument("--metadata", required=True)
    p.add_argument("--tree", required=True)
    p.add_argument("--cdx", action="append", required=True)
    p.add_argument("--component", action="append", default=[])
    p.add_argument("--out", required=True)
    p = sub.add_parser("complete", help="the SBOM against the shipped set and cargo metadata")
    p.add_argument("--sbom", required=True)
    p.add_argument("--metadata", required=True)
    p.add_argument("--tree", required=True)
    p.add_argument("--bin", action="append", required=True)
    p = sub.add_parser("notices", help="append the non-crate components to the cargo-about notices")
    p.add_argument("--sbom", action="append", required=True)
    p.add_argument("--metadata", required=True)
    p.add_argument("--notices", required=True)
    args = parser.parse_args(argv)
    if args.test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(SbomTest)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    try:
        if args.command == "cyclonedx":
            cyclonedx(args.target, args.bin, args.out)
            print(f"sbom: cargo-cyclonedx's SBOMs of {', '.join(args.bin)} for {args.target} in {args.out}")
        elif args.command == "shipped":
            count = len(shipped(args.target, args.bin, args.out, args.root))
            built = f"{args.root}, built with {', '.join(args.bin)}," if args.root else ", ".join(args.bin)
            print(f"sbom: {args.out}: {count} packages that cargo tree lists for {built} on {args.target}")
        elif args.command == "merge":
            bom = merge(args.artefact, args.version, args.cdx, read_json(args.metadata), read_tree(args.tree),
                        args.component)
            write_atomic(args.out, json.dumps(bom, indent=2, ensure_ascii=False) + "\n")
            extra = [c["name"] for c in bom["components"] if any(p["name"] == SECTION for p in c.get("properties", []))]
            print(f"sbom: {args.out}: {len(bom['components'])} components; non-crate: {', '.join(extra)}")
        elif args.command == "complete":
            count = complete(read_json(args.sbom), read_json(args.metadata), read_tree(args.tree), args.bin, args.sbom)
            print(f"sbom: {args.sbom} holds every package cargo tree lists for {', '.join(args.bin)}, each in cargo "
                  f"metadata's closure ({count} components)")
        elif args.command == "notices":
            with open(args.notices, encoding="utf-8") as f:
                text = f.read()
            result = notices(text, [read_json(s) for s in args.sbom], read_json(args.metadata))
            write_atomic(args.notices, result)
            print(f"sbom: {args.notices} names every component of {', '.join(args.sbom)}")
        else:
            parser.error("give a command or --test")
    except Refusal as refusal:
        print(f"sbom: {refusal}", file=sys.stderr)
        return 1
    return 0


# ---------------------------------------------------------------------------------------------------------------
# The offline unit test

FIXTURE_NOTICE = """SPDX-License-Identifier: CC0-1.0

ElektraWatt third-party notices
================================

Intro.

Bundled fonts (assets/fonts/, ADR-0028)
---------------------------------------

The fonts are licensed under the OFL; the licence text is LICENSES/OFL-1.1.txt.

assets/fonts/A-Regular.ttf
  Family:    A Sans Regular
  Version:   Version 2.003
  Copyright: Copyright 2015 A Inc.
  Source:    https://example.org/a.ttf
  SHA-256:   aaaa

assets/fonts/B-Math.ttf
  Family:    B Math Regular
  Version:   Version 3.000
  Source:    https://example.org/b.zip (release asset), file b.ttf inside
  SHA-256:   bbbb

Bundled icons (assets/icons/x/)
-------------------------------

X Icons, version 1.2.3, licensed under the MIT License (LICENSES/MIT.txt).

  Copyright: Copyright (c) 2020 X
  Source:    https://example.org/x.tgz (the package)
  SHA-256:   cccc (the package archive)

The runtime (packaging/components.toml `runtime`)
--------------------------------------------------

The runtime's notice.

SQLite (bundled by libsqlite3-sys)
----------------------------------

  May you do good and not evil.
"""

FIXTURE_COMPONENTS = """[[component]]
name = "nsis"
version = "3.13"
licence = "Zlib AND CPL-1.0"
shipped_licence = "Zlib"
no_notice = "zlib asks for none in a binary."
url = "https://example.org/nsis.zip"
sha256 = "dddd"
checked_by = "x"
used_for = "the setup's stub"

[[component]]
name = "runtime"
version = "20251108"
licence = "MIT"
url = "https://example.org/runtime"
sha256 = ""
checked_by = "x"
used_for = "the AppImage's runtime"
"""


class SbomTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp(prefix="sbom-test-")
        self.repo = os.path.join(self.dir, "repo")
        os.makedirs(os.path.join(self.repo, "packaging"))
        with open(os.path.join(self.repo, NOTICE), "w", encoding="utf-8") as f:
            f.write(FIXTURE_NOTICE)
        with open(os.path.join(self.repo, COMPONENTS), "w", encoding="utf-8") as f:
            f.write(FIXTURE_COMPONENTS)
        sqlite = os.path.join(self.dir, "libsqlite3-sys", "sqlite3")
        os.makedirs(sqlite)
        with open(os.path.join(sqlite, "sqlite3.h"), "w", encoding="utf-8") as f:
            f.write('/*\n** May you do good and not evil.\n*/\n#define SQLITE_VERSION        "3.53.2"\n')
        # The window's binary is a workspace crate: cargo's id holds its absolute path.
        self.app_id = "path+" + pathlib.Path(os.path.join(self.dir, "ws", "bin", "app")).as_uri() + "#0.0.0"
        self.metadata = self.fixture_metadata()
        self.shipped = shipped_set(self.SHIPPED)
        self.cdx = {"app": self.write("app.cdx.json",
                                      self.fixture_cdx("app", ["serde", "libsqlite3-sys", "cc", "egui_glow"])),
                    "cli": self.write("cli.cdx.json", self.fixture_cdx("cli", ["serde"]))}

    def tearDown(self):
        shutil.rmtree(self.dir, ignore_errors=True)

    def write(self, name, data):
        path = os.path.join(self.dir, name)
        with open(path, "w", encoding="utf-8") as f:
            json.dump(data, f)
        return path

    def fixture_metadata(self):
        def package(name, version, manifest="Cargo.toml", bin=False):
            pid = self.app_id if name == "app" else f"registry+x#{name}@{version}"
            kind = ["bin"] if bin else ["lib"]
            return {"id": pid, "name": name, "version": version, "manifest_path": manifest,
                    "targets": [{"name": name, "kind": kind}]}

        def dep(name, version, kind=None):
            return {"pkg": f"registry+x#{name}@{version}", "dep_kinds": [{"kind": kind, "target": None}]}

        manifest = os.path.join(self.dir, "libsqlite3-sys", "Cargo.toml")
        packages = [package("app", "0.0.0", bin=True), package("cli", "0.0.0", bin=True),
                    package("serde", "1.0.0"), package("libsqlite3-sys", "0.38.2", manifest),
                    package("cc", "1.2.0"), package("proptest", "1.0.0"), package("ew-model", "0.0.0")]
        nodes = [
            {"id": self.app_id, "features": [],
             "deps": [dep("serde", "1.0.0"), dep("libsqlite3-sys", "0.38.2"), dep("proptest", "1.0.0", "dev"),
                      dep("ew-model", "0.0.0")]},
            {"id": "registry+x#cli@0.0.0", "features": [], "deps": [dep("serde", "1.0.0")]},
            {"id": "registry+x#serde@1.0.0", "features": [], "deps": []},
            {"id": "registry+x#libsqlite3-sys@0.38.2", "features": ["bundled"], "deps": [dep("cc", "1.2.0", "build")]},
            {"id": "registry+x#cc@1.2.0", "features": [], "deps": []},
            {"id": "registry+x#proptest@1.0.0", "features": [], "deps": []},
            {"id": "registry+x#ew-model@0.0.0", "features": [], "deps": []},
        ]
        members = [self.app_id, "registry+x#cli@0.0.0", "registry+x#ew-model@0.0.0"]
        return {"packages": packages, "workspace_members": members, "resolve": {"nodes": nodes},
                "workspace_root": os.path.join(self.dir, "ws")}

    # The shipped set of `cargo tree --prefix none --format "{p}"` for app and cli: no egui_glow, which only a weak
    # feature names (ID-1318).
    SHIPPED = ("app v0.0.0 (/ws/bin/app)\nserde v1.0.0\nlibsqlite3-sys v0.38.2\ncc v1.2.0\new-model v0.0.0\n\n"
               "cli v0.0.0 (/ws/bin/cli)\nserde v1.0.0 (*)\n")

    def fixture_cdx(self, name, deps):
        versions = {"serde": "1.0.0", "libsqlite3-sys": "0.38.2", "cc": "1.2.0", "ew-model": "0.0.0",
                    "egui_glow": "0.37.0"}
        deps = deps + (["ew-model"] if name == "app" else [])
        top = self.app_id if name == "app" else f"registry+x#{name}@0.0.0"
        components = [{"type": "library", "bom-ref": f"registry+x#{d}@{versions[d]}", "name": d,
                       "version": versions[d], "purl": f"pkg:cargo/{d}@{versions[d]}",
                       "licenses": [{"expression": "MIT"}]} for d in deps]
        return {"bomFormat": "CycloneDX", "specVersion": "1.5", "version": 1,
                "metadata": {"tools": [{"vendor": "CycloneDX", "name": "cargo-cyclonedx", "version": "0.5.9"}],
                             "component": {"type": "application", "bom-ref": top,
                                           "name": name, "version": "0.0.0", "purl": f"pkg:cargo/{name}@0.0.0"},
                             "properties": [{"name": "cdx:rustc:sbom:target:triple", "value": "x86_64-unknown-linux-gnu"}]},
                "components": components,
                "dependencies": [{"ref": top, "dependsOn": [f"registry+x#{d}@{versions[d]}" for d in deps]}]}

    def merged(self, cdx=("app", "cli"), components=("nsis",)):
        printed = io.StringIO()
        with contextlib.redirect_stdout(printed):
            bom = merge("ElektraWatt-0.1.0-x64-user-setup.exe", "0.1.0", [self.cdx[c] for c in cdx], self.metadata,
                        self.shipped, components, self.repo)
        self.printed = printed.getvalue()
        return bom

    def test_merge_keeps_the_shipped_set_and_names_each_dropped_crate(self):
        bom = self.merged()
        self.assertNotIn("egui_glow", self.by_name(bom))
        self.assertEqual(self.printed, "sbom: ElektraWatt-0.1.0-x64-user-setup.exe: dropped egui_glow 0.37.0, which "
                                       "cargo tree does not list for app and cli\n")
        refs = {d["ref"] for d in bom["dependencies"]} | {r for d in bom["dependencies"] for r in d["dependsOn"]}
        self.assertNotIn("registry+x#egui_glow@0.37.0", refs)
        self.assertEqual(shipped_set(self.SHIPPED), {("app", "0.0.0"), ("serde", "1.0.0"), ("libsqlite3-sys", "0.38.2"),
                                                     ("cc", "1.2.0"), ("ew-model", "0.0.0"), ("cli", "0.0.0")})

    # `cargo tree --prefix depth --format "{p}"` of app and cli, built together (C50-p2#2): cli's own block shows
    # serde without its dependency (`(*)`), and cli reaches what app's block shows under serde, not libsqlite3-sys.
    JOINT = ("0app v0.0.0 (/ws/bin/app)\n1serde v1.0.0\n2serde_core v1.0.0\n1libsqlite3-sys v0.38.2\n2cc v1.2.0\n"
             "\n0cli v0.0.0 (/ws/bin/cli)\n1serde v1.0.0 (*)\n1cc v1.2.0 (*)\n")

    def test_the_cli_tree_is_what_cli_reaches_in_the_joint_graph(self):
        self.assertEqual(root_block(self.JOINT, "cli"), {("cli", "0.0.0"), ("serde", "1.0.0"),
                                                         ("serde_core", "1.0.0"), ("cc", "1.2.0")})
        self.assertEqual(len(root_block(self.JOINT, "app")), 5)
        commands = []

        def fake(command, cwd=None):
            commands.append(command)
            return self.JOINT

        out = os.path.join(self.dir, "cli.tree")
        self.assertEqual(shipped("x86_64-unknown-linux-gnu", ["app", "cli"], out, "cli", self.repo, fake),
                         root_block(self.JOINT, "cli"))
        self.assertEqual(commands, [["cargo", "tree", "--locked", "-e", "normal,build", "-p", "app", "-p", "cli",
                                     "--target", "x86_64-unknown-linux-gnu", "--prefix", "depth", "--format", "{p}"]])
        with open(out, encoding="utf-8") as f:
            self.assertEqual(f.read(), "cc v1.2.0\ncli v0.0.0\nserde v1.0.0\nserde_core v1.0.0\n")
        with self.assertRaises(Refusal) as refused:
            root_block(self.JOINT, "ew")
        self.assertEqual(str(refused.exception), "cargo tree shows no root ew")
        with self.assertRaises(Refusal) as refused:
            root_block("0app v0.0.0\n2serde v1.0.0\n", "app")
        self.assertEqual(str(refused.exception), "not a line of cargo tree's depth-prefixed {p} format: 2serde v1.0.0")

    def test_no_bom_ref_holds_a_checkout_path(self):
        bom = self.merged()
        refs = [c["bom-ref"] for c in bom["components"] if "bom-ref" in c]
        refs += [d["ref"] for d in bom["dependencies"]] + [r for d in bom["dependencies"] for r in d["dependsOn"]]
        self.assertIn("workspace/bin/app#0.0.0", refs)
        for ref in refs:
            self.assertNotIn(pathlib.Path(self.dir).as_uri()[len("file://"):], ref)
            self.assertNotIn(self.dir, ref)
            self.assertFalse(ref.startswith("path+file:"), ref)

    def test_a_shipped_package_outside_the_closure_fails_naming_it(self):
        bom = self.merged()
        bom["components"].append({"type": "library", "name": "zlib-rs", "version": "0.6.8"})
        with self.assertRaises(Refusal) as refused:
            complete(bom, self.metadata, self.shipped | {("zlib-rs", "0.6.8")}, ["app", "cli"])
        self.assertEqual(str(refused.exception), "cargo tree lists 1 package(s) for app and cli outside their closure "
                                                 "in cargo metadata:\n  zlib-rs 0.6.8")

    def by_name(self, bom):
        return {c["name"]: c for c in bom["components"]}

    def test_the_merged_sbom_holds_the_non_crate_components_with_versions_and_licences(self):
        bom = self.merged()
        self.assertEqual(bom["metadata"]["component"]["name"], "ElektraWatt-0.1.0-x64-user-setup.exe")
        names = self.by_name(bom)
        # One component per purl: serde, in both binaries' SBOMs, once.
        self.assertEqual([c["name"] for c in bom["components"]].count("serde"), 1)
        self.assertEqual(names["app"]["type"], "application")
        self.assertEqual(names["cli"]["type"], "application")
        font = names["A Sans Regular"]
        self.assertEqual((font["version"], font["licenses"], font["hashes"], font["copyright"]),
                         ("2.003", [{"license": {"id": "OFL-1.1"}}], [{"alg": "SHA-256", "content": "aaaa"}],
                          "Copyright 2015 A Inc."))
        self.assertEqual(names["B Math Regular"]["externalReferences"],
                         [{"type": "distribution", "url": "https://example.org/b.zip"}])
        icons = names["X Icons"]
        self.assertEqual((icons["version"], icons["licenses"], icons["hashes"][0]["content"]),
                         ("1.2.3", [{"license": {"id": "MIT"}}], "cccc"))
        sqlite = names["SQLite"]
        self.assertEqual((sqlite["version"], sqlite["licenses"]), ("3.53.2", [{"license": {"id": "blessing"}}]))
        nsis = names["nsis"]
        self.assertEqual((nsis["version"], nsis["licenses"], nsis["hashes"][0]["content"]),
                         ("3.13", [{"license": {"id": "Zlib"}}], "dddd"))
        self.assertNotIn("runtime", names)
        self.assertEqual(self.by_name(self.merged(components=("runtime",)))["runtime"]["licenses"],
                         [{"license": {"id": "MIT"}}])
        # The CLI alone does not reach libsqlite3-sys: no SQLite.
        self.assertNotIn("SQLite", self.by_name(self.merged(cdx=("cli",), components=())))

    def test_a_package_of_the_shipped_set_missing_from_the_sbom_fails_naming_it(self):
        bom = self.merged()
        self.assertEqual(complete(bom, self.metadata, self.shipped, ["app", "cli"]), len(bom["components"]))
        bom["components"] = [c for c in bom["components"] if c["name"] != "libsqlite3-sys"]
        with self.assertRaises(Refusal) as refused:
            complete(bom, self.metadata, self.shipped, ["app", "cli"])
        self.assertEqual(str(refused.exception), "the SBOM lacks 1 package(s) that cargo tree lists for app and cli:"
                                                 "\n  libsqlite3-sys 0.38.2")

    CARGO_ABOUT = ("ElektraWatt third-party notices\n===============================\n\nUsed by:\n"
                   "- serde 1.0.0 (https://example.org/serde)\n- libsqlite3-sys 0.38.2\n- cc 1.2.0\n\nMIT text\n")

    def test_the_notices_gain_the_non_crate_section_and_name_every_component(self):
        boms = [self.merged(), self.merged(cdx=("cli",), components=("runtime",))]
        text = notices(self.CARGO_ABOUT, boms, self.metadata, self.repo)
        tail = text[len(self.CARGO_ABOUT.rstrip("\n")):]
        self.assertTrue(tail.startswith(f"\n\n{RULE}\n{NON_CRATE}\n{RULE}\n\n"), tail)
        self.assertIn("\n- A Sans Regular 2.003 (OFL-1.1)\n- B Math Regular 3.000 (OFL-1.1)\n\n"
                      "Bundled fonts (assets/fonts/, ADR-0028)\n---------------------------------------\n\n"
                      "The fonts are licensed under the OFL", tail)
        self.assertIn("\n- X Icons 1.2.3 (MIT)\n\nBundled icons", tail)
        self.assertIn("\n- SQLite 3.53.2 (blessing)\n\nSQLite (bundled by libsqlite3-sys)\n", tail)
        self.assertIn("  May you do good and not evil.\n", tail)
        self.assertIn("\n- runtime 20251108 (MIT)\n\nThe runtime (packaging/components.toml `runtime`)\n", tail)
        self.assertTrue(tail.endswith("\n\n- nsis 3.13 (Zlib)\n  No notice: zlib asks for none in a binary.\n"), tail)
        with self.assertRaises(Refusal) as refused:
            notices(text, boms, self.metadata, self.repo)
        self.assertEqual(str(refused.exception), 'the notices hold a "Non-crate components" section already')

    def test_a_non_crate_component_without_a_section_or_a_reason_fails_naming_it(self):
        bom = self.merged()
        with open(os.path.join(self.repo, COMPONENTS), "w", encoding="utf-8") as f:
            f.write(FIXTURE_COMPONENTS.replace('no_notice = "zlib asks for none in a binary."\n', ""))
        with self.assertRaises(Refusal) as refused:
            notices(self.CARGO_ABOUT, [bom], self.metadata, self.repo)
        self.assertEqual(str(refused.exception), "NOTICE has no section for 1 component(s) of the SBOM, and "
                                                 "packaging/components.toml gives no reason they need none:\n"
                                                 "  nsis 3.13")

    def test_a_component_the_notices_do_not_name_fails_naming_it(self):
        without_cc = self.CARGO_ABOUT.replace("- cc 1.2.0\n", "")
        with self.assertRaises(Refusal) as refused:
            notices(without_cc, [self.merged()], self.metadata, self.repo)
        self.assertEqual(str(refused.exception), "the notices do not name 1 component(s) of the SBOM:\n  cc 1.2.0")

    def test_the_cyclonedx_step_keeps_the_binaries_sboms_and_cleans_the_checkout(self):
        repo = os.path.join(self.dir, "ws")
        for folder in ("bin/app", "bin/cli", "xtask", "target/x"):
            os.makedirs(os.path.join(repo, folder))
        metadata = self.fixture_metadata()
        for package, folder in (("app", "bin/app"), ("cli", "bin/cli")):
            entry = next(p for p in metadata["packages"] if p["name"] == package)
            entry["manifest_path"] = os.path.join(repo, folder, "Cargo.toml")
        with open(os.path.join(repo, "target", "x", "kept.cdx.json"), "w", encoding="utf-8") as f:
            f.write("{}")

        def fake(command, cwd=None):
            if command[1] == "metadata":
                return json.dumps(metadata)
            for folder in ("bin/app/app_bin.cdx.json", "bin/cli/cli_bin.cdx.json", "xtask/xtask_bin.cdx.json"):
                with open(os.path.join(repo, folder), "w", encoding="utf-8") as f:
                    f.write(folder)
            return ""

        out = os.path.join(self.dir, "out")
        cyclonedx("x86_64-unknown-linux-gnu", ["app"], out, repo, fake)
        self.assertEqual(sorted(os.listdir(out)), ["app.cdx.json", "metadata.json"])
        for gone in ("bin/app/app_bin.cdx.json", "bin/cli/cli_bin.cdx.json", "xtask/xtask_bin.cdx.json"):
            self.assertFalse(os.path.exists(os.path.join(repo, gone)), gone)
        self.assertTrue(os.path.exists(os.path.join(repo, "target", "x", "kept.cdx.json")))

    def test_the_repositorys_notice_gives_its_components(self):
        with open(os.path.join(REPO, NOTICE), encoding="utf-8") as f:
            text = f.read()
        components = {c["name"]: c for c in notice_components(text)}
        for font in ("Noto Sans Regular", "Noto Sans Bold", "Noto Sans SemiBold", "Noto Sans Math Regular"):
            self.assertEqual(components[font]["licenses"], [{"license": {"id": "OFL-1.1"}}], font)
            self.assertRegex(components[font]["hashes"][0]["content"], r"^[0-9a-f]{64}$")
        self.assertEqual(components["Tabler Icons"]["licenses"], [{"license": {"id": "MIT"}}])
        self.assertRegex(components["Tabler Icons"]["version"], r"^\d+\.\d+\.\d+$")
        sections = notice_sections(text)
        sqlite = section_naming(sections, "SQLite")
        self.assertIsNotNone(sqlite)
        body = dict(sections)[sqlite]
        self.assertIn("May you do good and not evil.", body)
        self.assertIsNotNone(section_naming(sections, "`appimage-type2-runtime`"))
        # The packaging components that the artefacts hold are in packaging/components.toml.
        for name in ("nsis", "appimage-type2-runtime"):
            self.assertEqual(packaging_component(REPO, name, sections)["name"], name)
        # C50-p2#3: NSIS's stub ships under zlib alone, which asks for no notice; the runtime has its section.
        nsis = packaging_component(REPO, "nsis", sections)
        self.assertEqual((nsis["licenses"], nsis["properties"][0]["value"]), ([{"license": {"id": "Zlib"}}], ""))
        with open(os.path.join(REPO, COMPONENTS), "rb") as f:
            entry = next(e for e in tomllib.load(f)["component"] if e["name"] == "nsis")
        self.assertTrue(entry["no_notice"])


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
