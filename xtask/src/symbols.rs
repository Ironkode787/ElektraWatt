// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `symbols-qet10`: seals the native starter library documents and generates the QET edition (WP-S7.02).
//!
//! The library documents under `content/` are pretty JSON with a placeholder envelope while being
//! authored. **Sealing** fills the envelope (copy id, library id, item id, version, content hash) and the
//! symbol's function-definition reference. The **QET edition** is a generated copy of every native symbol
//! on the QET 10 px grid: all lengths are scaled by `7_937_500 / 12_000_000` (the 4 mm grid step becomes
//! the QET step `7_937_500 / 3` nm) and the key gets the suffix `@qet10`.
//!
//! xtask must not depend on any workspace crate, so the documents are handled as JSON text with an
//! order-preserving value type ([`V`]); member order is never changed. The envelope, id derivations and
//! hash follow `ew-schema` and `ew-ids` and are pinned by tests there and here.

use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// The provisional system library id (same constant as `ew-import-qet`).
const BUILTIN_LIBRARY: Uuid = Uuid::from_u128(0x7202fd8b_1024_4bf1_9e99_670772eaa589);

/// The namespace of embedded library copies (`ew_ids::v5::NS_LIBCOPY`).
const NS_LIBCOPY: Uuid = Uuid::from_u128(0x04c1640f_8857_4dfc_9bff_e9baad47fb47);

/// The component separator of `ew_ids::v5::lib_copy_id`, U+001F.
const SEP: char = '\u{1F}';

/// The suffix of the QET edition's keys.
const QET_SUFFIX: &str = "@qet10";

/// The native grid step: 4 mm in nm.
const NATIVE_STEP: i64 = 4_000_000;

/// Envelope members in file order; everything else is the hashed item.
const ENVELOPE: [&str; 7] = ["$k", "id", "$v", "library_id", "item_id", "version", "sha256"];

/// An order-preserving JSON value. Floats are rejected: the library documents use integers only.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum V {
    /// `null`.
    Null,
    /// `true` / `false`.
    Bool(bool),
    /// An integer.
    Int(i64),
    /// A string.
    Str(String),
    /// An array.
    Arr(Vec<V>),
    /// An object; members in stored order, names unique.
    Obj(Vec<(String, V)>),
}

impl V {
    /// The member `name` of an object.
    fn get(&self, name: &str) -> Option<&V> {
        match self {
            V::Obj(members) => members.iter().find(|(n, _)| n == name).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The member `name` of an object, mutably.
    fn get_mut(&mut self, name: &str) -> Option<&mut V> {
        match self {
            V::Obj(members) => members.iter_mut().find(|(n, _)| n == name).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The string, if this is one.
    fn as_str(&self) -> Option<&str> {
        match self {
            V::Str(s) => Some(s),
            _ => None,
        }
    }
}

impl Serialize for V {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            V::Null => serializer.serialize_unit(),
            V::Bool(b) => serializer.serialize_bool(*b),
            V::Int(i) => serializer.serialize_i64(*i),
            V::Str(s) => serializer.serialize_str(s),
            V::Arr(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            V::Obj(members) => {
                let mut map = serializer.serialize_map(Some(members.len()))?;
                for (name, value) in members {
                    map.serialize_entry(name, value)?;
                }
                map.end()
            }
        }
    }
}

/// The serde visitor of [`V`].
struct VVisitor;

impl<'de> Visitor<'de> for VVisitor {
    type Value = V;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON value without floating-point numbers")
    }

    fn visit_unit<E: de::Error>(self) -> Result<V, E> {
        Ok(V::Null)
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<V, E> {
        Ok(V::Bool(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<V, E> {
        Ok(V::Int(v))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<V, E> {
        i64::try_from(v)
            .map(V::Int)
            .map_err(|_| E::custom(format!("integer {v} exceeds i64::MAX")))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<V, E> {
        Err(E::custom(format!(
            "floating-point number {v} is not allowed; use integers"
        )))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<V, E> {
        Ok(V::Str(v.to_owned()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<V, E> {
        Ok(V::Str(v))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<V, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(V::Arr(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<V, A::Error> {
        let mut members: Vec<(String, V)> = Vec::new();
        while let Some((name, value)) = map.next_entry::<String, V>()? {
            if members.iter().any(|(n, _)| *n == name) {
                return Err(de::Error::custom(format!("duplicate member `{name}`")));
            }
            members.push((name, value));
        }
        Ok(V::Obj(members))
    }
}

impl<'de> Deserialize<'de> for V {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<V, D::Error> {
        deserializer.deserialize_any(VVisitor)
    }
}

/// The compact JSON text of a value.
fn compact(v: &V) -> Result<String> {
    serde_json::to_string(v).context("serialising JSON")
}

/// The file text of a document: pretty JSON (2 spaces, LF) and a trailing newline.
fn file_text(v: &V) -> Result<String> {
    let mut text = serde_json::to_string_pretty(v).context("serialising JSON")?;
    text.push('\n');
    Ok(text)
}

/// Rounds `v * 7_937_500 / 12_000_000` to the nearest integer, ties toward zero. Negative values mirror
/// positive ones. For `v = k * 4_000_000` this equals `Grid::at(k)` of the QET grid.
pub(crate) fn scale(v: i64) -> i64 {
    const NUM: i128 = 7_937_500;
    const DEN: i128 = 12_000_000;
    let n = i128::from(v).abs() * NUM;
    let mut q = n / DEN;
    if 2 * (n % DEN) > DEN {
        q += 1;
    }
    let q = i64::try_from(q).expect("scaling shrinks the value");
    if v < 0 { -q } else { q }
}

/// The item id of a library key: `v5(BUILTIN_LIBRARY, key)`.
pub(crate) fn item_id(key: &str) -> Uuid {
    Uuid::new_v5(&BUILTIN_LIBRARY, key.as_bytes())
}

/// The copy id of an item of the built-in library: `v5(NS_LIBCOPY, library 0x1F item)`
/// (`ew_ids::v5::lib_copy_id`).
pub(crate) fn copy_id(item: Uuid) -> Uuid {
    let name = format!("{}{SEP}{}", BUILTIN_LIBRARY.hyphenated(), item.hyphenated());
    Uuid::new_v5(&NS_LIBCOPY, name.as_bytes())
}

/// A UUID as a lower-case hyphenated string value.
fn uuid_value(u: Uuid) -> V {
    V::Str(u.hyphenated().to_string())
}

/// The members of a document that make up the hashed item: everything but the envelope.
fn item_members(doc: &V) -> Result<Vec<(String, V)>> {
    let V::Obj(members) = doc else {
        bail!("the document is not a JSON object");
    };
    Ok(members
        .iter()
        .filter(|(n, _)| !ENVELOPE.contains(&n.as_str()))
        .cloned()
        .collect())
}

/// The `meta.key` of a document.
fn key_of(doc: &V) -> Result<&str> {
    doc.get("meta")
        .and_then(|m| m.get("key"))
        .and_then(V::as_str)
        .ok_or_else(|| anyhow!("missing string member `meta.key`"))
}

/// Seals a document: rewrites the envelope (`$k`, copy id, library id, item id, version, hash) and, for a
/// symbol, the function-definition reference. Item members keep their order.
pub(crate) fn seal(doc: &V) -> Result<V> {
    let kind = doc
        .get("$k")
        .and_then(V::as_str)
        .ok_or_else(|| anyhow!("missing string member `$k`"))?;
    if kind != "symbol" && kind != "fndef" {
        bail!("`$k` is `{kind}`, expected `symbol` or `fndef`");
    }
    let key = key_of(doc)?.to_owned();
    let version = match doc.get("version") {
        None => "0.1.0".to_owned(),
        Some(V::Str(s)) => s.clone(),
        Some(_) => bail!("`version` is not a string"),
    };
    let mut item = item_members(doc)?;
    if kind == "symbol" {
        let (_, fndef) = item
            .iter_mut()
            .find(|(n, _)| n == "fndef")
            .ok_or_else(|| anyhow!("symbol has no `fndef` member"))?;
        *fndef = seal_ref(fndef)?;
    }
    let item_doc = V::Obj(item);
    let digest = Sha256::digest(compact(&item_doc)?.as_bytes());
    let V::Obj(item) = item_doc else { unreachable!() };

    let item_uuid = item_id(&key);
    let mut out = vec![
        ("$k".to_owned(), V::Str(kind.to_owned())),
        ("id".to_owned(), uuid_value(copy_id(item_uuid))),
        ("library_id".to_owned(), uuid_value(BUILTIN_LIBRARY)),
        ("item_id".to_owned(), uuid_value(item_uuid)),
        ("version".to_owned(), V::Str(version)),
        ("sha256".to_owned(), V::Str(format!("sha256:{}", hex(&digest)))),
    ];
    out.extend(item);
    Ok(V::Obj(out))
}

/// Rewrites a symbol's `fndef` reference: library and item derive from `key`; `version_req` is kept.
fn seal_ref(r: &V) -> Result<V> {
    let key = r
        .get("key")
        .and_then(V::as_str)
        .ok_or_else(|| anyhow!("`fndef` has no string member `key`"))?;
    let version_req = r
        .get("version_req")
        .cloned()
        .ok_or_else(|| anyhow!("`fndef` has no member `version_req`"))?;
    Ok(V::Obj(vec![
        ("library".to_owned(), uuid_value(BUILTIN_LIBRARY)),
        ("item".to_owned(), uuid_value(item_id(key))),
        ("version_req".to_owned(), version_req),
        ("key".to_owned(), V::Str(key.to_owned())),
    ]))
}

/// Lower-case hex of a byte string.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// An in-place scaling step for one member.
type Scaler = fn(&mut V) -> Result<()>;

/// Scales an integer length in place.
fn len(v: &mut V) -> Result<()> {
    match v {
        V::Int(i) => {
            *i = scale(*i);
            Ok(())
        }
        other => bail!("expected an integer length, found {}", compact(other)?),
    }
}

/// Scales a point `[x, y]` in place.
fn pt(v: &mut V) -> Result<()> {
    match v {
        V::Arr(xy) if xy.len() == 2 => xy.iter_mut().try_for_each(len),
        other => bail!("expected a point [x, y], found {}", compact(other)?),
    }
}

/// Scales an array of points (a polyline, or a rectangle's two corners) in place.
fn pts(v: &mut V) -> Result<()> {
    match v {
        V::Arr(points) => points.iter_mut().try_for_each(pt),
        other => bail!("expected an array of points, found {}", compact(other)?),
    }
}

/// Applies `f` to member `name` of `obj` when present.
fn member(obj: &mut V, name: &str, f: fn(&mut V) -> Result<()>) -> Result<()> {
    match obj.get_mut(name) {
        Some(v) => f(v).with_context(|| format!("member `{name}`")),
        None => Ok(()),
    }
}

/// Scales one graphics primitive (`{"line": {...}}`). Strokes, fills, angles and text are not scaled.
fn scale_primitive(p: &mut V) -> Result<()> {
    let V::Obj(members) = p else {
        bail!("a primitive is not an object");
    };
    let [(name, body)] = members.as_mut_slice() else {
        bail!("a primitive must have exactly one member, the primitive name");
    };
    let fields: &[(&str, Scaler)] = match name.as_str() {
        "line" => &[("from", pt), ("to", pt)],
        "polyline" | "polygon" => &[("points", pts)],
        "rect" => &[("rect", pts), ("rx", len), ("ry", len), ("shadow", len)],
        "circle" => &[("centre", pt), ("r", len)],
        "ellipse" | "arc" | "image" => &[("rect", pts)],
        "text" => &[("pos", pt)],
        other => bail!("unknown primitive `{other}`"),
    };
    let name = name.clone();
    for (field, f) in fields {
        member(body, field, *f).with_context(|| format!("primitive `{name}`"))?;
    }
    Ok(())
}

/// Scales a connection point's `pos`, which must lie on the 4 mm grid.
fn scale_cp(cp: &mut V) -> Result<()> {
    let Some(pos) = cp.get_mut("pos") else {
        bail!("connection point has no `pos`");
    };
    let V::Arr(xy) = pos else {
        bail!("connection point `pos` is not an array");
    };
    for v in xy.iter() {
        match v {
            V::Int(i) if i % NATIVE_STEP == 0 => {}
            other => bail!(
                "connection point coordinate {} is not a multiple of {NATIVE_STEP} nm",
                compact(other)?
            ),
        }
    }
    pt(pos)
}

/// Scales every element of the array member `name` of `obj` with `f`.
fn each(obj: &mut V, name: &str, f: fn(&mut V) -> Result<()>) -> Result<()> {
    match obj.get_mut(name) {
        None => Ok(()),
        Some(V::Arr(items)) => items
            .iter_mut()
            .enumerate()
            .try_for_each(|(i, item)| f(item).with_context(|| format!("`{name}[{i}]`"))),
        Some(_) => bail!("`{name}` is not an array"),
    }
}

/// Scales a text slot's `pos` and `max_width`.
fn scale_slot(slot: &mut V) -> Result<()> {
    member(slot, "pos", pt)?;
    member(slot, "max_width", len)
}

/// Scales a link point's `pos`.
fn scale_named_point(p: &mut V) -> Result<()> {
    member(p, "pos", pt)
}

/// Scales an extra variant's graphics and connection points.
fn scale_variant(variant: &mut V) -> Result<()> {
    each(variant, "graphics", scale_primitive)?;
    each(variant, "cps", scale_cp)
}

/// Generates the QET edition of a native symbol document: the grid, the key and every length scaled.
pub(crate) fn qet10(native: &V) -> Result<V> {
    if native.get("$k").and_then(V::as_str) != Some("symbol") {
        bail!("only symbols have a QET edition");
    }
    let key = key_of(native)?.to_owned();
    let mut item = V::Obj(item_members(native)?);
    let native_grid = V::Obj(vec![
        ("num".to_owned(), V::Int(NATIVE_STEP)),
        ("den".to_owned(), V::Int(1)),
    ]);
    if item.get("grid") != Some(&native_grid) {
        bail!("the grid is not {{\"num\": {NATIVE_STEP}, \"den\": 1}}");
    }
    *item.get_mut("grid").expect("checked above") = V::Obj(vec![
        ("num".to_owned(), V::Int(7_937_500)),
        ("den".to_owned(), V::Int(3)),
    ]);
    *item
        .get_mut("meta")
        .and_then(|m| m.get_mut("key"))
        .expect("key_of succeeded") = V::Str(format!("{key}{QET_SUFFIX}"));

    each(&mut item, "graphics", scale_primitive)?;
    each(&mut item, "cps", scale_cp)?;
    each(&mut item, "text_slots", scale_slot)?;
    each(&mut item, "link_points", scale_named_point)?;
    each(&mut item, "extra_variants", scale_variant)?;
    if let Some(V::Obj(named)) = item.get_mut("anchor")
        && let [(name, point)] = named.as_mut_slice()
        && name == "named"
    {
        pt(point).context("`anchor`")?;
    }

    let V::Obj(mut members) = item else { unreachable!() };
    let mut doc = vec![("$k".to_owned(), V::Str("symbol".to_owned()))];
    if let Some(version) = native.get("version") {
        doc.push(("version".to_owned(), version.clone()));
    }
    doc.append(&mut members);
    seal(&V::Obj(doc))
}

/// A file the subcommand owns: its path, what it holds now (if it exists) and what it should hold.
struct Target {
    /// Path relative to the repository root, with `/` separators.
    rel: String,
    /// The current text; `None` when the file does not exist.
    current: Option<String>,
    /// The text sealing or generation produces.
    wanted: String,
}

/// The `*.json` files directly in `dir`, sorted by name.
fn json_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        if path.is_file() && path.extension().is_some_and(|e| e == "json") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// Reads and parses a library document.
fn parse_file(path: &Path) -> Result<V> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// The file name of `path`.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Computes every native and QET target, and the stale files of the QET directory.
fn plan(root: &Path) -> Result<(Vec<Target>, Vec<Target>, Vec<String>)> {
    let symbols_dir = root.join("content/symbols");
    if !symbols_dir.is_dir() {
        bail!(
            "content/symbols does not exist (looked in {})",
            symbols_dir.display()
        );
    }
    let fndefs_dir = root.join("content/fndefs");
    let qet_dir = symbols_dir.join("qet10");

    let mut native = Vec::new();
    let mut qet = Vec::new();
    let mut paths = json_files(&symbols_dir)?;
    if fndefs_dir.is_dir() {
        paths.extend(json_files(&fndefs_dir)?);
    }
    for path in paths {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let doc = parse_file(&path)?;
        let sealed = seal(&doc).with_context(|| format!("sealing {rel}"))?;
        let current = std::fs::read_to_string(&path).ok();
        if sealed.get("$k").and_then(V::as_str) == Some("symbol") {
            let key = key_of(&sealed)?.to_owned();
            let generated = qet10(&sealed).with_context(|| format!("generating the QET edition of {rel}"))?;
            let name = format!("{key}{QET_SUFFIX}.json");
            let qet_path = qet_dir.join(&name);
            qet.push(Target {
                rel: format!("content/symbols/qet10/{name}"),
                current: std::fs::read_to_string(&qet_path).ok(),
                wanted: file_text(&generated)?,
            });
        }
        native.push(Target {
            rel,
            current,
            wanted: file_text(&sealed)?,
        });
    }

    let mut stale = Vec::new();
    if qet_dir.is_dir() {
        for entry in std::fs::read_dir(&qet_dir).with_context(|| format!("reading {}", qet_dir.display()))? {
            let path = entry?.path();
            let name = file_name(&path);
            if !qet.iter().any(|t| t.rel.ends_with(&format!("/{name}"))) {
                stale.push(name);
            }
        }
        stale.sort();
    }
    Ok((native, qet, stale))
}

/// Seals the native documents under `content/symbols` and `content/fndefs` and generates the QET edition in
/// `content/symbols/qet10`. With `check`, nothing is written and every deviation is reported. Returns
/// whether the check passed (write mode always returns `true`).
pub(crate) fn run(root: &Path, check: bool) -> Result<bool> {
    let (native, qet, stale) = plan(root)?;
    if check {
        if native.iter().all(|t| !t.rel.starts_with("content/symbols/")) {
            eprintln!("symbols-qet10: no symbols in content/symbols");
            return Ok(false);
        }
        let mut problems = Vec::new();
        for t in &native {
            if t.current.as_deref() != Some(t.wanted.as_str()) {
                problems.push(format!(
                    "{} is not sealed; run `cargo xtask symbols-qet10`",
                    t.rel
                ));
            }
        }
        for t in &qet {
            match &t.current {
                None => problems.push(format!("{} is missing; run `cargo xtask symbols-qet10`", t.rel)),
                Some(text) if *text != t.wanted => {
                    problems.push(format!(
                        "{} is out of date; run `cargo xtask symbols-qet10`",
                        t.rel
                    ));
                }
                Some(_) => {}
            }
        }
        for name in &stale {
            problems.push(format!(
                "content/symbols/qet10/{name} is stale (no native symbol); run `cargo xtask symbols-qet10`"
            ));
        }
        if problems.is_empty() {
            println!("symbols-qet10: ok ({} native, {} qet10)", native.len(), qet.len());
            return Ok(true);
        }
        eprintln!("symbols-qet10: {} problem(s)", problems.len());
        for p in &problems {
            eprintln!("  - {p}");
        }
        return Ok(false);
    }

    let (mut sealed, mut generated, mut unchanged) = (0, 0, 0);
    for t in &native {
        if t.current.as_deref() == Some(t.wanted.as_str()) {
            unchanged += 1;
        } else {
            std::fs::write(root.join(&t.rel), &t.wanted).with_context(|| format!("writing {}", t.rel))?;
            sealed += 1;
        }
    }
    let qet_dir = root.join("content/symbols/qet10");
    std::fs::create_dir_all(&qet_dir).with_context(|| format!("creating {}", qet_dir.display()))?;
    for t in &qet {
        if t.current.as_deref() == Some(t.wanted.as_str()) {
            unchanged += 1;
        } else {
            std::fs::write(root.join(&t.rel), &t.wanted).with_context(|| format!("writing {}", t.rel))?;
            generated += 1;
        }
    }
    for name in &stale {
        std::fs::remove_file(qet_dir.join(name)).with_context(|| format!("removing stale {name}"))?;
    }
    println!(
        "symbols-qet10: sealed {sealed}, generated {generated}, removed {} stale, unchanged {unchanged}",
        stale.len()
    );
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A temporary repository root, removed on drop.
    struct Temp(PathBuf);

    impl Temp {
        fn new() -> Self {
            static N: AtomicU32 = AtomicU32::new(0);
            let dir = std::env::temp_dir().join(format!(
                "ew-xtask-symbols-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("content/symbols")).unwrap();
            std::fs::create_dir_all(dir.join("content/fndefs")).unwrap();
            Temp(dir)
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const SYMBOL: &str = r#"{
  "$k": "symbol", "id": "x", "library_id": "x", "item_id": "x", "version": "0.2.0", "sha256": "x",
  "meta": {"key": "ew.test.sym", "sources": ["ew.original"]},
  "fndef": {"library": "x", "item": "x", "version_req": "^0.1", "key": "ew.fn.test"},
  "grid": {"num": 4000000, "den": 1},
  "graphics": [
    {"line": {"from": [0, 0], "to": [0, 5000000], "stroke": {"pen": "p026"}}},
    {"polyline": {"points": [[4000000, 8500000], [-4500000, 7500000]], "stroke": {"pen": "p013"}}},
    {"rect": {"rect": [[2000000, 7000000], [4000000, 10000000]], "rx": 500000, "stroke": {"pen": "p026"}}},
    {"circle": {"centre": [1000000, 1000000], "r": 2000000}},
    {"text": {"pos": [-4000000, 0], "text": "Q", "angle": 90}}
  ],
  "cps": [{"idx": 0, "fn_cp": 0, "pos": [0, 16000000], "dir": "up"}],
  "text_slots": [{"kind": "tag", "pos": [-4000000, 8000000], "align": "middle_right", "max_width": 12000000}],
  "anchor": {"named": [4000000, 8000000]},
  "link_points": [{"name": "a", "pos": [-1000000, 8500000]}]
}"#;

    const FNDEF: &str = r#"{
  "$k": "fndef", "id": "x", "library_id": "x", "item_id": "x", "version": "0.1.0", "sha256": "x",
  "meta": {"key": "ew.fn.test"},
  "contact_kind": "pole"
}"#;

    fn parse(text: &str) -> V {
        serde_json::from_str(text).unwrap()
    }

    fn fixture(root: &Path) {
        std::fs::write(root.join("content/symbols/ew.test.sym.json"), SYMBOL).unwrap();
        std::fs::write(root.join("content/fndefs/ew.fn.test.json"), FNDEF).unwrap();
    }

    #[test]
    fn scale_values() {
        assert_eq!(scale(0), 0);
        assert_eq!(scale(4_000_000), 2_645_833);
        assert_eq!(scale(8_000_000), 5_291_667);
        assert_eq!(scale(500_000), 330_729);
        assert_eq!(scale(-4_000_000), -2_645_833);
        assert_eq!(scale(-500_000), -330_729);
    }

    #[test]
    fn scale_ties_toward_zero() {
        // 96 * 7_937_500 = 63.5 * 12_000_000 exactly.
        assert_eq!(96_i128 * 7_937_500 % 12_000_000, 6_000_000);
        assert_eq!(scale(96), 63);
        assert_eq!(scale(-96), -63);
        assert_eq!(scale(288), 190);
        assert_eq!(scale(-288), -190);
    }

    #[test]
    fn scale_matches_grid_reference() {
        for k in -50_i64..=50 {
            let n = i128::from(k) * 7_937_500;
            // Denominator 3 never ties, so round-half-up is the reference.
            let reference = (2 * n + 3).div_euclid(6);
            assert_eq!(i128::from(scale(k * 4_000_000)), reference, "k = {k}");
        }
    }

    #[test]
    fn uuid_v5_sanity() {
        assert_eq!(
            Uuid::new_v5(&Uuid::NAMESPACE_DNS, b"python.org").to_string(),
            "886313e1-3b8a-5372-9b90-0c9aee199e5d"
        );
        let item = item_id("ew.iec.mcb-1pn");
        assert_eq!(item.to_string(), "b1057f75-bba8-5767-8b57-58c39a3f88fe");
        assert_eq!(copy_id(item).to_string(), "64aba25b-8862-5a5f-b632-223294705dc7");
    }

    #[test]
    fn ordered_value_keeps_order() {
        let text = r#"{"b": 1, "a": [true, null, "s", {"z": -2, "y": 3}]}"#;
        let v = parse(text);
        assert_eq!(
            compact(&v).unwrap(),
            r#"{"b":1,"a":[true,null,"s",{"z":-2,"y":3}]}"#
        );
        let pretty = file_text(&v).unwrap();
        assert!(pretty.starts_with("{\n  \"b\": 1,\n  \"a\": [\n    true,"));
        assert!(pretty.ends_with("}\n"));
        assert_eq!(parse(&pretty), v);
    }

    #[test]
    fn ordered_value_rejects_floats_and_duplicates() {
        assert!(serde_json::from_str::<V>("[1.5]").is_err());
        assert!(serde_json::from_str::<V>("[18446744073709551615]").is_err());
        let err = serde_json::from_str::<V>(r#"{"a": 1, "a": 2}"#).unwrap_err();
        assert!(err.to_string().contains("duplicate member `a`"), "{err}");
    }

    #[test]
    fn seal_fills_envelope_and_fndef_ref() {
        let sealed = seal(&parse(SYMBOL)).unwrap();
        let V::Obj(members) = &sealed else { panic!() };
        let names: Vec<&str> = members.iter().take(8).map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            [
                "$k",
                "id",
                "library_id",
                "item_id",
                "version",
                "sha256",
                "meta",
                "fndef"
            ]
        );
        assert_eq!(sealed.get("version"), Some(&V::Str("0.2.0".to_owned())));
        assert_eq!(
            sealed.get("library_id"),
            Some(&V::Str(BUILTIN_LIBRARY.to_string()))
        );
        let fndef = sealed.get("fndef").unwrap();
        assert_eq!(fndef.get("item"), Some(&uuid_value(item_id("ew.fn.test"))));
        let V::Obj(ref_members) = fndef else { panic!() };
        let ref_names: Vec<&str> = ref_members.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(ref_names, ["library", "item", "version_req", "key"]);
        // The hash covers the item members only.
        let item = V::Obj(item_members(&sealed).unwrap());
        let digest = Sha256::digest(compact(&item).unwrap().as_bytes());
        assert_eq!(
            sealed.get("sha256"),
            Some(&V::Str(format!("sha256:{}", hex(&digest))))
        );
        assert_eq!(seal(&sealed).unwrap(), sealed);
    }

    #[test]
    fn seal_without_fndef_key_is_an_error() {
        let bad = SYMBOL.replace(r#", "key": "ew.fn.test""#, "");
        assert!(seal(&parse(&bad)).is_err());
    }

    #[test]
    fn write_is_idempotent_and_check_detects_edits() {
        let tmp = Temp::new();
        fixture(&tmp.0);
        assert!(!run(&tmp.0, true).unwrap());
        assert!(run(&tmp.0, false).unwrap());
        assert!(run(&tmp.0, true).unwrap());
        assert!(
            tmp.0
                .join("content/symbols/qet10/ew.test.sym@qet10.json")
                .is_file()
        );

        let before = std::fs::read_to_string(tmp.0.join("content/symbols/ew.test.sym.json")).unwrap();
        assert!(run(&tmp.0, false).unwrap());
        let after = std::fs::read_to_string(tmp.0.join("content/symbols/ew.test.sym.json")).unwrap();
        assert_eq!(before, after);

        let edited = before.replace("5000000", "5000001");
        assert_ne!(edited, before);
        std::fs::write(tmp.0.join("content/symbols/ew.test.sym.json"), edited).unwrap();
        assert!(!run(&tmp.0, true).unwrap());

        // Write mode reseals, removes stale qet10 files and repairs a missing one.
        std::fs::write(tmp.0.join("content/symbols/qet10/stale@qet10.json"), "{}").unwrap();
        assert!(!run(&tmp.0, true).unwrap());
        assert!(run(&tmp.0, false).unwrap());
        assert!(!tmp.0.join("content/symbols/qet10/stale@qet10.json").exists());
        assert!(run(&tmp.0, true).unwrap());
    }

    #[test]
    fn missing_directory_is_an_error_and_empty_fails_check() {
        let tmp = Temp::new();
        std::fs::remove_dir_all(tmp.0.join("content/symbols")).unwrap();
        let err = run(&tmp.0, true).unwrap_err();
        assert!(
            err.to_string().contains("content/symbols does not exist"),
            "{err}"
        );
        std::fs::create_dir_all(tmp.0.join("content/symbols")).unwrap();
        assert!(!run(&tmp.0, true).unwrap());
    }

    #[test]
    fn qet10_scales_lengths_only() {
        let q = qet10(&seal(&parse(SYMBOL)).unwrap()).unwrap();
        assert_eq!(key_of(&q).unwrap(), "ew.test.sym@qet10");
        assert_eq!(q.get("item_id"), Some(&uuid_value(item_id("ew.test.sym@qet10"))));
        assert_eq!(
            q.get("id"),
            Some(&uuid_value(copy_id(item_id("ew.test.sym@qet10"))))
        );
        assert_eq!(q.get("version"), Some(&V::Str("0.2.0".to_owned())));
        assert_eq!(
            compact(q.get("grid").unwrap()).unwrap(),
            r#"{"num":7937500,"den":3}"#
        );
        // The fndef reference is unchanged.
        assert_eq!(q.get("fndef"), seal(&parse(SYMBOL)).unwrap().get("fndef"));
        let V::Arr(graphics) = q.get("graphics").unwrap() else {
            panic!()
        };
        let g: Vec<String> = graphics.iter().map(|p| compact(p).unwrap()).collect();
        assert_eq!(
            g[0],
            r#"{"line":{"from":[0,0],"to":[0,3307292],"stroke":{"pen":"p026"}}}"#
        );
        assert_eq!(
            g[1],
            r#"{"polyline":{"points":[[2645833,5622396],[-2976562,4960937]],"stroke":{"pen":"p013"}}}"#
        );
        assert_eq!(
            g[2],
            r#"{"rect":{"rect":[[1322917,4630208],[2645833,6614583]],"rx":330729,"stroke":{"pen":"p026"}}}"#
        );
        assert_eq!(g[3], r#"{"circle":{"centre":[661458,661458],"r":1322917}}"#);
        assert_eq!(g[4], r#"{"text":{"pos":[-2645833,0],"text":"Q","angle":90}}"#);
        // CP at (0, 4 grid steps) lands at 4 * 7937500 / 3 rounded.
        assert_eq!(
            compact(&q.get("cps").unwrap().clone()).unwrap(),
            r#"[{"idx":0,"fn_cp":0,"pos":[0,10583333],"dir":"up"}]"#
        );
        assert_eq!(scale(16_000_000), 10_583_333);
        assert_eq!(
            compact(q.get("text_slots").unwrap()).unwrap(),
            r#"[{"kind":"tag","pos":[-2645833,5291667],"align":"middle_right","max_width":7937500}]"#
        );
        assert_eq!(
            compact(q.get("anchor").unwrap()).unwrap(),
            r#"{"named":[2645833,5291667]}"#
        );
        assert_eq!(
            compact(q.get("link_points").unwrap()).unwrap(),
            r#"[{"name":"a","pos":[-661458,5622396]}]"#
        );
    }

    #[test]
    fn qet10_rejects_off_grid_cp_grid_and_unknown_primitive() {
        let off_grid = SYMBOL.replace("[0, 16000000]", "[0, 16000001]");
        let err = qet10(&seal(&parse(&off_grid)).unwrap()).unwrap_err();
        assert!(format!("{err:#}").contains("not a multiple"), "{err:#}");

        let other_grid = SYMBOL.replace(r#""num": 4000000"#, r#""num": 5000000"#);
        assert!(qet10(&seal(&parse(&other_grid)).unwrap()).is_err());

        let unknown = SYMBOL.replace(r#""circle""#, r#""blob""#);
        let err = qet10(&seal(&parse(&unknown)).unwrap()).unwrap_err();
        assert!(format!("{err:#}").contains("unknown primitive `blob`"), "{err:#}");
    }

    #[test]
    fn committed_content_is_current() {
        assert!(run(&crate::workspace_root(), true).unwrap());
    }
}
