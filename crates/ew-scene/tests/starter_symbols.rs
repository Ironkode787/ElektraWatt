// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Lint of the starter symbol library (WP-S7.02): `content/symbols/*.json` (native 4 mm edition),
//! `content/symbols/qet10/*.json` (generated QET 10 px edition, key suffix `@qet10`) and
//! `content/fndefs/*.json`.
//!
//! Each test collects every problem and asserts once at its end, so one run lists everything. Every file
//! is read once and shared through a [`OnceLock`].
//!
//! # Sealing
//!
//! The envelope values `id`, `item_id`, `sha256` and the symbols' `fndef.item` are written by
//! `cargo xtask symbols-qet10` ("sealing"). A document whose `item_id` is the nil UUID is *unsealed*: its
//! `id` and `sha256` must then be placeholders too and the derived-ID checks are skipped. Set
//! `EW_REQUIRE_SEALED=1` to make an unsealed document a problem. The `sha256` value itself is verified by
//! `cargo xtask symbols-qet10 --check`, not here; this lint only rejects the all-zero placeholder on a
//! sealed document.
//!
//! # Snapshots
//!
//! The two inline snapshots at the end (`ew.iec.mcb-1pn` at R0, `ew.iec.contact-no` at M90) list cps, texts
//! and, per lowered item, kind, pen and geometry in integer nm. They are updated by
//! hand: cargo-insta may be unavailable, so after an intended change to a symbol or to `lower` run the
//! test, copy the received text into the `@r#"..."#` literal and review the diff.
//!
//! # Content blessing
//!
//! `EW_BLESS_CONTENT=1 cargo nextest run -p ew-scene --test starter_symbols bless_native_documents`
//! rewrites every native symbol and fndef file to the canonical text (member order and default omission
//! of `serde_json::to_string_pretty` over the typed document). It never touches envelope values and never
//! the generated `qet10` edition.

// Test target: bless and diagnostic environment flags (EW_*), which only choose whether a committed file is rewritten or a line is printed; the compared result never depends on them.
#![allow(clippy::disallowed_methods)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ew_ids::Uuid;
use ew_scene::symbol::{SymbolGeometry, TextSource, lower, resolve_anchor};
use ew_schema::{
    Align, AnchorPolicy, Colour, Dash, Dir4, FnDef, ItemMeta, ItemStatus, LibCopy, LibDocument, Pen,
    Primitive, Sha256, SlotKind, Stroke, SymbolCp, SymbolDef,
};
use ew_units::{Grid, Nm, Orient2, Pt2};

/// The interruption-point symbols: their xref slot keeps the default style of its kind.
const INTERRUPTION_POINTS: [&str; 2] = ["ew.gen.ip", "ew.gen.external-arrow"];

/// The keys of the native starter library; the qet10 edition has the same keys with `@qet10`.
const STARTER_KEYS: &[&str] = &[
    "ew.iec.contact-no",
    "ew.iec.contact-nc",
    "ew.iec.contact-co",
    "ew.iec.coil",
    "ew.iec.valve-coil",
    "ew.iec.fuse",
    "ew.iec.switch-1p",
    "ew.iec.switch-2p-manual",
    "ew.iec.mcb-1pn",
    "ew.iec.mcb-1p",
    "ew.iec.thermostat-nc",
    "ew.iec.position-switch-no",
    "ew.iec.pushbutton-no",
    "ew.iec.selector-switch-no",
    "ew.iec.lamp",
    "ew.iec.heater",
    "ew.iec.resistor",
    "ew.iec.motor-3ph",
    "ew.iec.transformer-1ph",
    "ew.iec.terminal",
    "ew.iec.terminal-pe",
    "ew.gen.strip-segment",
    "ew.iec.pe",
    "ew.iec.fe",
    "ew.iec.plug-pin",
    "ew.gen.blackbox",
    "ew.gen.ip",
    "ew.gen.external-arrow",
    "ew.gen.junction-dot",
    "ew.gen.junction-bevel",
    "ew.gen.cable-marker",
    "ew.gen.wire-marker",
    "ew.bus.dp-port",
    "ew.bus.dp-connector",
    "ew.ovw.module",
    "ew.ovw.valve-top",
    "ew.iso.valve-5-2-sol-spring",
    "ew.iso.cylinder-double-acting",
    "ew.iso.process-valve-2way",
];

/// The suffix of a generated QET 10 px key.
const QET_SUFFIX: &str = "@qet10";
/// The ElektraWatt builtin library (`library_id` of every starter document).
const LIBRARY: Uuid = Uuid::from_u128(0x7202fd8b_1024_4bf1_9e99_670772eaa589);
/// The version of every starter document.
const VERSION: &str = "0.1.0";
/// House pen width of the pen-independent tolerance of on-outline checks, in nm (native edition).
const OUTLINE_TOL: i64 = 1000;
/// The most text slots a symbol may have.
const MAX_SLOTS: usize = 12;
/// Graphic coordinates of the native edition are multiples of 0.5 mm.
const GRAPHIC_STEP: i64 = 500_000;
/// Text slot positions and widths of the native edition are multiples of 1 mm.
const SLOT_STEP: i64 = 1_000_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Edition {
    Native,
    Qet10,
}

/// One loaded document with its file name and text.
struct Doc<T> {
    /// Path relative to `content/`, with forward slashes.
    label: String,
    file: String,
    text: String,
    /// `serde_json::to_string_pretty` of the document plus a newline.
    canonical: String,
    key: String,
    meta: ItemMeta,
    copy: LibCopy<T>,
}

impl<T> Doc<T> {
    /// `file [key]`, for messages.
    fn who(&self) -> String {
        format!("{} [{}]", self.label, self.key)
    }
}

struct Lib {
    native: Vec<Doc<SymbolDef>>,
    qet10: Vec<Doc<SymbolDef>>,
    fndefs: Vec<Doc<FnDef>>,
    /// Problems found while loading.
    problems: Vec<String>,
}

fn content_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content")
}

/// Loads every `*.json` of `dir` as a strict `LibDocument`; names in `allowed_other` and anything else
/// that is not a `*.json` file are reported (the allowed ones silently skipped). A missing directory
/// gives nothing.
fn load_dir(
    dir: &Path,
    allowed_other: &[&str],
    problems: &mut Vec<String>,
) -> Vec<(PathBuf, String, LibDocument)> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<PathBuf> = read.filter_map(|e| e.ok().map(|e| e.path())).collect();
    names.sort();
    let mut out = Vec::new();
    for path in names {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if allowed_other.contains(&name.as_str()) {
            continue;
        }
        if !path.is_file() || !name.ends_with(".json") {
            problems.push(format!(
                "{}: unexpected entry (only *.json files are allowed)",
                path_label(&path)
            ));
            continue;
        }
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                problems.push(format!("{}: unreadable: {e}", path_label(&path)));
                continue;
            }
        };
        match LibDocument::from_json(&text) {
            Ok(doc) => out.push((path, text, doc)),
            Err(e) => problems.push(format!("{}: does not load strictly: {e}", path_label(&path))),
        }
    }
    out
}

/// The path from `content/` on, with forward slashes.
fn path_label(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    match s.find("content/") {
        Some(i) => s[i..].trim_start_matches("content/").to_owned(),
        None => s,
    }
}

/// Splits loaded documents into typed ones; a document of another kind is a problem.
fn typed<T>(
    docs: Vec<(PathBuf, String, LibDocument)>,
    expect: &str,
    pick: impl Fn(LibDocument) -> Result<(LibCopy<T>, ItemMeta), Option<ew_schema::Kind>>,
    problems: &mut Vec<String>,
) -> Vec<Doc<T>> {
    let mut out = Vec::new();
    for (path, text, doc) in docs {
        let label = path_label(&path);
        let canonical = match serde_json::to_string_pretty(&doc) {
            Ok(c) => c + "\n",
            Err(e) => {
                problems.push(format!("{label}: cannot be written back: {e}"));
                continue;
            }
        };
        if let Err(e) = doc.validate() {
            problems.push(format!("{label}: validate() fails: {e}"));
        }
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match pick(doc) {
            Ok((copy, meta)) => {
                let key = meta
                    .key
                    .as_ref()
                    .map_or_else(|| "<no key>".to_owned(), |k| k.as_str().to_owned());
                out.push(Doc {
                    label,
                    file,
                    text,
                    canonical,
                    key,
                    meta,
                    copy,
                });
            }
            Err(kind) => problems.push(format!("{label}: has kind {kind:?}, expected {expect}")),
        }
    }
    out
}

fn lib() -> &'static Lib {
    static LIB: OnceLock<Lib> = OnceLock::new();
    LIB.get_or_init(|| {
        let content = content_dir();
        let mut problems = Vec::new();
        for dir in ["symbols", "fndefs"] {
            if !content.join(dir).is_dir() {
                problems.push(format!("content/{dir} does not exist"));
            }
        }
        let native = load_dir(&content.join("symbols"), &["README.md", "qet10"], &mut problems);
        let qet10 = load_dir(&content.join("symbols/qet10"), &[], &mut problems);
        let fndefs = load_dir(&content.join("fndefs"), &[], &mut problems);
        let pick_symbol = |d: LibDocument| match d {
            LibDocument::Symbol(c) => {
                let meta = c.item.meta.clone();
                Ok((c, meta))
            }
            other => Err(other.kind()),
        };
        let pick_fndef = |d: LibDocument| match d {
            LibDocument::Fndef(c) => {
                let meta = c.item.meta.clone();
                Ok((c, meta))
            }
            other => Err(other.kind()),
        };
        Lib {
            native: typed(native, "symbol", pick_symbol, &mut problems),
            qet10: typed(qet10, "symbol", pick_symbol, &mut problems),
            fndefs: typed(fndefs, "fndef", pick_fndef, &mut problems),
            problems,
        }
    })
}

/// Asserts that `problems` is empty, listing all of them.
fn assert_clean(what: &str, problems: &[String]) {
    assert!(
        problems.is_empty(),
        "{what}: {} problem(s)\n  {}",
        problems.len(),
        problems.join("\n  ")
    );
}

fn symbols() -> impl Iterator<Item = (Edition, &'static Doc<SymbolDef>)> {
    let l = lib();
    l.native
        .iter()
        .map(|d| (Edition::Native, d))
        .chain(l.qet10.iter().map(|d| (Edition::Qet10, d)))
}

fn native_twins() -> BTreeMap<&'static str, &'static Doc<SymbolDef>> {
    lib().native.iter().map(|d| (d.key.as_str(), d)).collect()
}

fn require_sealed() -> bool {
    std::env::var("EW_REQUIRE_SEALED").as_deref() == Ok("1")
}

// ---------------------------------------------------------------------------------------------------------
// Geometry helpers

/// The QET 10 px edition value of a native nm value: `round(n × 7 937 500 / 12 000 000)`, ties toward zero.
/// The pen of the qet10 edition for a native house pen: the house pen in QET px (doc 20 §17.4; 1 px =
/// 7 937 500 / 30 nm, rounded): p013 0.5 px, p018 0.7, p026 1.0, p042 1.6, p052 2.0, p130 5.0.
fn qet_pen(p: &Pen) -> Option<Pen> {
    let w = match p {
        Pen::P013 => 132_292,
        Pen::P018 => 185_208,
        Pen::P026 => 264_583,
        Pen::P042 => 423_333,
        Pen::P052 => 529_167,
        Pen::P130 => 1_322_917,
        _ => return None,
    };
    Some(Pen::Width(Nm(w)))
}

fn qet_scale(n: i64) -> i64 {
    let num = i128::from(n) * 7_937_500;
    let den = 12_000_000_i128;
    let a = num.abs();
    let mut q = a / den;
    if 2 * (a % den) > den {
        q += 1;
    }
    i64::try_from(if num < 0 { -q } else { q }).expect("fits")
}

fn qet_pt(p: Pt2) -> Pt2 {
    Pt2::new(Nm(qet_scale(p.x.0)), Nm(qet_scale(p.y.0)))
}

fn on_grid(grid: Grid, x: Nm) -> bool {
    grid.at(grid.snap(x)) == Some(x)
}

fn pt_on_grid(grid: Grid, p: Pt2) -> bool {
    on_grid(grid, p.x) && on_grid(grid, p.y)
}

/// The kind tag and every coordinate (and length) of a primitive, in a fixed order.
fn coords(p: &Primitive) -> (&'static str, Vec<i64>) {
    let pt = |p: &Pt2| [p.x.0, p.y.0];
    let rect = |r: &ew_units::Rect| vec![r.min().x.0, r.min().y.0, r.max().x.0, r.max().y.0];
    match p {
        Primitive::Line(l) => ("line", [pt(&l.from), pt(&l.to)].concat()),
        Primitive::Polyline(l) => ("polyline", l.points.iter().flat_map(pt).collect()),
        Primitive::Rect(r) => {
            let mut v = rect(&r.rect.0);
            v.extend([r.rx.0, r.ry.0]);
            v.extend(r.shadow.map(|s| s.0));
            ("rect", v)
        }
        Primitive::Circle(c) => ("circle", vec![c.centre.x.0, c.centre.y.0, c.r.0]),
        Primitive::Ellipse(e) => ("ellipse", rect(&e.rect.0)),
        Primitive::Arc(a) => ("arc", rect(&a.rect.0)),
        Primitive::Polygon(g) => ("polygon", g.points.iter().flat_map(pt).collect()),
        Primitive::Text(t) => ("text", pt(&t.pos).to_vec()),
        Primitive::Image(i) => ("image", rect(&i.rect.0)),
        Primitive::Unknown(_) => ("unknown", Vec::new()),
    }
}

fn stroke_of(p: &Primitive) -> Option<&Stroke> {
    match p {
        Primitive::Line(x) => x.stroke.as_ref(),
        Primitive::Polyline(x) => x.stroke.as_ref(),
        Primitive::Rect(x) => x.stroke.as_ref(),
        Primitive::Circle(x) => x.stroke.as_ref(),
        Primitive::Ellipse(x) => x.stroke.as_ref(),
        Primitive::Arc(x) => x.stroke.as_ref(),
        Primitive::Polygon(x) => x.stroke.as_ref(),
        Primitive::Text(_) | Primitive::Image(_) | Primitive::Unknown(_) => None,
    }
}

fn fill_of(p: &Primitive) -> Option<&Colour> {
    match p {
        Primitive::Rect(x) => x.fill.as_ref(),
        Primitive::Circle(x) => x.fill.as_ref(),
        Primitive::Ellipse(x) => x.fill.as_ref(),
        Primitive::Polygon(x) => x.fill.as_ref(),
        _ => None,
    }
}

/// Main graphics, then those of every extra variant.
fn all_graphics(def: &SymbolDef) -> impl Iterator<Item = &Primitive> {
    def.graphics
        .iter()
        .chain(def.extra_variants.iter().flat_map(|v| v.graphics.iter()))
}

/// The main CPs, then those of every extra variant.
fn all_cps(def: &SymbolDef) -> impl Iterator<Item = &SymbolCp> {
    def.cps
        .iter()
        .chain(def.extra_variants.iter().flat_map(|v| v.cps.iter()))
}

/// Whether `p` is at a line end or polygon point, on a rectangle or ellipse outline, on a circle outline
/// or at the centre of a filled circle (the junction dot), within `tol` nm.
fn on_outline(p: Pt2, prims: &[Primitive], tol: i64) -> bool {
    let near = |a: Pt2| (a.x.0 - p.x.0).abs() <= tol && (a.y.0 - p.y.0).abs() <= tol;
    let (x, y) = (p.x.0 as f64, p.y.0 as f64);
    prims.iter().any(|prim| match prim {
        Primitive::Line(l) => near(l.from) || near(l.to),
        Primitive::Polyline(l) => l.points.iter().any(|&a| near(a)),
        Primitive::Polygon(g) => g.points.iter().any(|&a| near(a)),
        Primitive::Rect(r) => {
            let (a, b) = (r.rect.0.min(), r.rect.0.max());
            let within_x = a.x.0 - tol <= p.x.0 && p.x.0 <= b.x.0 + tol;
            let within_y = a.y.0 - tol <= p.y.0 && p.y.0 <= b.y.0 + tol;
            let on_x_edge = (p.x.0 - a.x.0).abs() <= tol || (p.x.0 - b.x.0).abs() <= tol;
            let on_y_edge = (p.y.0 - a.y.0).abs() <= tol || (p.y.0 - b.y.0).abs() <= tol;
            (on_x_edge && within_y) || (on_y_edge && within_x)
        }
        Primitive::Circle(c) => {
            let d = (x - c.centre.x.0 as f64).hypot(y - c.centre.y.0 as f64);
            (d - c.r.0 as f64).abs() <= tol as f64 || (c.fill.is_some() && near(c.centre))
        }
        Primitive::Ellipse(e) => on_ellipse(e.rect.0, x, y, tol),
        Primitive::Arc(a) => on_ellipse(a.rect.0, x, y, tol),
        Primitive::Text(_) | Primitive::Image(_) | Primitive::Unknown(_) => false,
    })
}

fn on_ellipse(r: ew_units::Rect, x: f64, y: f64, tol: i64) -> bool {
    let (a, b) = (r.width().0 as f64 / 2.0, r.height().0 as f64 / 2.0);
    if a <= 0.0 || b <= 0.0 {
        return false;
    }
    let cx = r.min().x.0 as f64 + a;
    let cy = r.min().y.0 as f64 + b;
    let v = (((x - cx) / a).powi(2) + ((y - cy) / b).powi(2)).sqrt();
    (v - 1.0).abs() * a.min(b) <= tol as f64
}

// ---------------------------------------------------------------------------------------------------------
// 1. LOAD

#[test]
fn documents_load_and_are_canonical() {
    let l = lib();
    let mut problems = l.problems.clone();
    for (ed, d) in symbols() {
        check_file(
            ed,
            &d.file,
            &d.who(),
            &d.key,
            &d.text,
            &d.canonical,
            &mut problems,
        );
    }
    for d in &l.fndefs {
        check_file(
            Edition::Native,
            &d.file,
            &d.who(),
            &d.key,
            &d.text,
            &d.canonical,
            &mut problems,
        );
    }
    assert_clean("library documents", &problems);
}

fn check_file(
    ed: Edition,
    file: &str,
    who: &str,
    key: &str,
    text: &str,
    canonical: &str,
    out: &mut Vec<String>,
) {
    if text != canonical {
        out.push(format!(
            "{who}: text is not canonical (run with EW_BLESS_CONTENT=1)"
        ));
    }
    if file != format!("{key}.json") {
        out.push(format!("{who}: file name must be `<meta.key>.json`"));
    }
    if ed == Edition::Qet10 && !key.ends_with(QET_SUFFIX) {
        out.push(format!("{who}: a qet10 key must end in `{QET_SUFFIX}`"));
    }
    if ed == Edition::Native && key.contains(QET_SUFFIX) {
        out.push(format!("{who}: a native key must not carry `{QET_SUFFIX}`"));
    }
}

// ---------------------------------------------------------------------------------------------------------
// 2. IDENTITY

#[test]
fn identity_and_hygiene() {
    let l = lib();
    let mut problems = Vec::new();
    for (_, d) in symbols() {
        check_identity(d, &d.meta, &mut problems);
        let def = &d.copy.item;
        if let Some(r) = &def.iec60617_ref {
            let s = r.as_str();
            let ok = s.len() == 6 && s.starts_with('S') && s[1..].bytes().all(|c| c.is_ascii_digit());
            if !ok {
                problems.push(format!(
                    "{}: iec60617_ref {s:?} is not `S` and five digits",
                    d.who()
                ));
            }
        }
        check_extra(&d.who(), &def.extra, &mut problems);
    }
    for d in &l.fndefs {
        check_identity(d, &d.meta, &mut problems);
        check_extra(&d.who(), &d.copy.item.extra, &mut problems);
    }
    let texts = symbols()
        .map(|(_, d)| (d.who(), &d.text))
        .chain(l.fndefs.iter().map(|d| (d.who(), &d.text)));
    for (who, text) in texts {
        if text.contains('\\') {
            problems.push(format!("{who}: contains a backslash"));
        }
        if text.contains("/Users/") {
            problems.push(format!("{who}: contains `/Users/`"));
        }
        if text.replace(QET_SUFFIX, "").contains('@') {
            problems.push(format!(
                "{who}: contains an `@` other than the `{QET_SUFFIX}` key suffix"
            ));
        }
    }
    assert_clean("identity and hygiene", &problems);
}

fn check_extra(who: &str, extra: &ew_schema::Extra, out: &mut Vec<String>) {
    for k in extra.keys().filter(|k| !k.starts_with("ew.")) {
        out.push(format!(
            "{who}: unknown member `{k}` (only `ew.` keys are allowed in extra)"
        ));
    }
}

fn check_identity<T>(d: &Doc<T>, meta: &ItemMeta, out: &mut Vec<String>) {
    let c = &d.copy;
    let who = d.who();
    if c.library_id.as_uuid() != LIBRARY {
        out.push(format!(
            "{who}: library_id is {}, expected {LIBRARY}",
            c.library_id
        ));
    }
    if c.version != VERSION {
        out.push(format!("{who}: version is {:?}, expected {VERSION:?}", c.version));
    }
    let item = c.item_id.as_uuid();
    if item.is_nil() {
        if !c.id.as_uuid().is_nil() || c.sha256 != Sha256([0; 32]) {
            out.push(format!(
                "{who}: partially sealed (item_id is the placeholder, id or sha256 is not)"
            ));
        }
        if require_sealed() {
            out.push(format!("{who}: unsealed (run `cargo xtask symbols-qet10`)"));
        }
    } else {
        if meta.key.is_some() {
            let want = Uuid::new_v5(&LIBRARY, d.key.as_bytes());
            if item != want {
                out.push(format!(
                    "{who}: item_id is {item}, expected v5(library, key) = {want}"
                ));
            }
        }
        let want_id = ew_ids::v5::lib_copy_id::<()>(LIBRARY, item).as_uuid();
        if c.id.as_uuid() != want_id {
            out.push(format!("{who}: id is {}, expected lib_copy_id = {want_id}", c.id));
        }
        // The hash itself is verified by `cargo xtask symbols-qet10 --check`.
        if c.sha256 == Sha256([0; 32]) {
            out.push(format!("{who}: sha256 is still the all-zero placeholder"));
        }
    }
    if meta.key.is_none() {
        out.push(format!("{who}: meta.key is missing"));
    }
    if meta.licence.as_deref() != Some("CC-BY-SA-4.0") {
        out.push(format!(
            "{who}: meta.licence is {:?}, expected \"CC-BY-SA-4.0\"",
            meta.licence
        ));
    }
    if meta.status != ItemStatus::Draft {
        out.push(format!("{who}: meta.status is {:?}, expected draft", meta.status));
    }
    if meta.sources.is_empty() {
        out.push(format!("{who}: meta.sources is empty"));
    }
}

// ---------------------------------------------------------------------------------------------------------
// 3. FNDEF REFERENCE

#[test]
fn fndef_references() {
    let l = lib();
    let mut problems = Vec::new();
    let by_key: BTreeMap<&str, &Doc<FnDef>> = l.fndefs.iter().map(|d| (d.key.as_str(), d)).collect();
    let mut referenced: BTreeSet<&str> = BTreeSet::new();
    let twins = native_twins();
    for (ed, d) in symbols() {
        let who = d.who();
        let def = &d.copy.item;
        let r = &def.fndef;
        if r.library.as_uuid() != LIBRARY {
            problems.push(format!(
                "{who}: fndef.library is {}, expected the builtin library",
                r.library
            ));
        }
        if r.version_req != "^0.1" {
            problems.push(format!(
                "{who}: fndef.version_req is {:?}, expected \"^0.1\"",
                r.version_req
            ));
        }
        let Some(key) = r.extra.get("key").and_then(|v| v.as_str()) else {
            problems.push(format!("{who}: fndef.extra[\"key\"] is not a string"));
            continue;
        };
        referenced.insert(key);
        let want = Uuid::new_v5(&LIBRARY, key.as_bytes());
        let item = r.item.as_uuid();
        if item.is_nil() {
            if !d.copy.item_id.as_uuid().is_nil() {
                problems.push(format!("{who}: fndef.item is the placeholder in a sealed symbol"));
            }
        } else if item != want {
            problems.push(format!(
                "{who}: fndef.item is {item}, expected v5(library, {key:?}) = {want}"
            ));
        }
        let Some(fd) = by_key.get(key) else {
            problems.push(format!("{who}: no fndef document with meta.key {key:?}"));
            continue;
        };
        let fn_item = fd.copy.item_id.as_uuid();
        if !item.is_nil() && !fn_item.is_nil() && fn_item != item {
            problems.push(format!(
                "{who}: fndef {key:?} has item_id {fn_item}, the reference says {item}"
            ));
        }
        for cp in all_cps(def) {
            if fd.copy.item.cps.get(&cp.fn_cp).is_none() {
                problems.push(format!(
                    "{who}: cp {} has fn_cp {}, not a CP of fndef {key:?}",
                    cp.idx, cp.fn_cp
                ));
            }
        }
        if ed == Edition::Qet10 {
            match d.key.strip_suffix(QET_SUFFIX).and_then(|k| twins.get(k)) {
                None => problems.push(format!("{who}: no native twin")),
                Some(n) if n.copy.item.fndef != *r => {
                    problems.push(format!("{who}: fndef reference differs from the native twin"));
                }
                Some(_) => {}
            }
        }
    }
    for fd in &l.fndefs {
        let who = fd.who();
        let def = &fd.copy.item;
        if !referenced.contains(fd.key.as_str()) {
            problems.push(format!("{who}: orphan, no symbol references it"));
        }
        let mut seen = BTreeSet::new();
        for cp in &def.cps {
            if !seen.insert(cp.idx) {
                problems.push(format!("{who}: duplicate CP idx {}", cp.idx));
            }
            if cp.designation.as_str().is_empty() {
                problems.push(format!("{who}: CP {} has an empty designation", cp.idx));
            }
        }
        for link in &def.internal {
            for end in [link.a, link.b] {
                if def.cps.get(&end).is_none() {
                    problems.push(format!(
                        "{who}: internal link {}-{} refers to missing CP {end}",
                        link.a, link.b
                    ));
                }
            }
        }
    }
    assert_clean("fndef references", &problems);
}

// ---------------------------------------------------------------------------------------------------------
// 4. GRID

#[test]
fn native_grid() {
    let mut problems = Vec::new();
    for d in &lib().native {
        let who = d.who();
        let def = &d.copy.item;
        if def.grid != Grid::MM4 {
            problems.push(format!("{who}: grid is {:?}, expected Grid::MM4", def.grid));
        }
        for cp in all_cps(def) {
            if !pt_on_grid(Grid::MM4, cp.pos) {
                problems.push(format!(
                    "{who}: cp {} at {:?} is off the 4 mm grid",
                    cp.idx, cp.pos
                ));
            }
        }
        for (i, prim) in all_graphics(def).enumerate() {
            let (tag, v) = coords(prim);
            if let Some(c) = v.iter().find(|c| *c % GRAPHIC_STEP != 0) {
                problems.push(format!(
                    "{who}: graphic {i} ({tag}) has coordinate {c}, not a multiple of 0.5 mm"
                ));
            }
        }
        for (i, s) in def.text_slots.iter().enumerate() {
            let vals = [s.pos.x.0, s.pos.y.0].into_iter().chain(s.max_width.map(|w| w.0));
            if let Some(c) = vals.into_iter().find(|c| c % SLOT_STEP != 0) {
                problems.push(format!(
                    "{who}: text slot {i} has value {c}, not a multiple of 1 mm"
                ));
            }
        }
        for lp in &def.link_points {
            if lp.pos.x.0 % GRAPHIC_STEP != 0 || lp.pos.y.0 % GRAPHIC_STEP != 0 {
                problems.push(format!(
                    "{who}: link point {} at {:?} is not a multiple of 0.5 mm",
                    lp.name, lp.pos
                ));
            }
        }
        if let AnchorPolicy::Named(p) = &def.anchor
            && !pt_on_grid(Grid::MM4, *p)
        {
            problems.push(format!("{who}: named anchor {p:?} is off the 4 mm grid"));
        }
    }
    assert_clean("native grid", &problems);
}

#[test]
fn qet10_grid_and_edition_equals_generation() {
    let mut problems = Vec::new();
    let twins = native_twins();
    for d in &lib().qet10 {
        let who = d.who();
        let q = &d.copy.item;
        if q.grid != Grid::QET_10PX {
            problems.push(format!("{who}: grid is {:?}, expected Grid::QET_10PX", q.grid));
        }
        for cp in all_cps(q) {
            if !pt_on_grid(Grid::QET_10PX, cp.pos) {
                problems.push(format!(
                    "{who}: cp {} at {:?} is off the QET 10 px grid",
                    cp.idx, cp.pos
                ));
            }
        }
        let Some(n) = d.key.strip_suffix(QET_SUFFIX).and_then(|k| twins.get(k)) else {
            problems.push(format!("{who}: no native twin"));
            continue;
        };
        compare_editions(&who, &n.copy.item, q, &mut problems);
    }
    assert_clean("qet10 edition", &problems);
}

/// The qet10 edition `q` must be the generation (scaling) of its native twin `n`.
fn compare_editions(who: &str, n: &SymbolDef, q: &SymbolDef, out: &mut Vec<String>) {
    let same = |what: &str, ok: bool, out: &mut Vec<String>| {
        if !ok {
            out.push(format!("{who}: {what} differs from the native twin"));
        }
    };
    same("name", n.name == q.name, out);
    same("keywords", n.keywords == q.keywords, out);
    same("rep_type", n.rep_type == q.rep_type, out);
    same("arrangements", n.arrangements == q.arrangements, out);
    same("iec60617_ref", n.iec60617_ref == q.iec60617_ref, out);
    same("iso1219_ref", n.iso1219_ref == q.iso1219_ref, out);
    compare_graphics(who, "graphics", &n.graphics, &q.graphics, out);
    compare_cps(who, "cps", n.cps.as_slice(), q.cps.as_slice(), out);
    if n.extra_variants.len() != q.extra_variants.len() {
        out.push(format!(
            "{who}: extra_variants count differs from the native twin"
        ));
    } else {
        for (i, (a, b)) in n.extra_variants.iter().zip(&q.extra_variants).enumerate() {
            same(&format!("extra_variants[{i}].kind"), a.kind == b.kind, out);
            compare_graphics(
                who,
                &format!("extra_variants[{i}].graphics"),
                &a.graphics,
                &b.graphics,
                out,
            );
            compare_cps(
                who,
                &format!("extra_variants[{i}].cps"),
                a.cps.as_slice(),
                b.cps.as_slice(),
                out,
            );
        }
    }
    if n.text_slots.len() != q.text_slots.len() {
        out.push(format!("{who}: text slot count differs from the native twin"));
    } else {
        for (i, (a, b)) in n.text_slots.iter().zip(&q.text_slots).enumerate() {
            same(
                &format!("text slot {i} kind/rot/align/visible"),
                a.kind == b.kind && a.rot == b.rot && a.align == b.align && a.visible == b.visible,
                out,
            );
            same(&format!("text slot {i} position"), qet_pt(a.pos) == b.pos, out);
            same(
                &format!("text slot {i} max_width"),
                a.max_width.map(|w| qet_scale(w.0)) == b.max_width.map(|w| w.0),
                out,
            );
        }
    }
    if n.link_points.len() != q.link_points.len() {
        out.push(format!("{who}: link point count differs from the native twin"));
    } else {
        for (a, b) in n.link_points.iter().zip(&q.link_points) {
            same(
                &format!("link point {}", a.name),
                a.name == b.name && qet_pt(a.pos) == b.pos,
                out,
            );
        }
    }
    let anchor_ok = match (&n.anchor, &q.anchor) {
        (AnchorPolicy::Named(a), AnchorPolicy::Named(b)) => qet_pt(*a) == *b,
        (a, b) => a == b,
    };
    same("anchor", anchor_ok, out);
}

fn compare_graphics(who: &str, what: &str, n: &[Primitive], q: &[Primitive], out: &mut Vec<String>) {
    if n.len() != q.len() {
        out.push(format!(
            "{who}: {what} has {} primitives, the native twin {}",
            q.len(),
            n.len()
        ));
        return;
    }
    for (i, (a, b)) in n.iter().zip(q).enumerate() {
        let ((ta, va), (tb, vb)) = (coords(a), coords(b));
        if ta != tb || va.len() != vb.len() {
            out.push(format!(
                "{who}: {what}[{i}] is a {tb} with {} values, the native twin a {ta} with {}",
                vb.len(),
                va.len()
            ));
            continue;
        }
        for (j, (x, y)) in va.iter().zip(&vb).enumerate() {
            if qet_scale(*x) != *y {
                out.push(format!(
                    "{who}: {what}[{i}] ({ta}) value {j} is {y}, expected {} (native {x})",
                    qet_scale(*x)
                ));
            }
        }
        let pens_ok = match (stroke_of(a), stroke_of(b)) {
            (None, None) => true,
            (Some(x), Some(y)) => {
                qet_pen(&x.pen).as_ref() == Some(&y.pen) && x.dash == y.dash && x.colour == y.colour
            }
            _ => false,
        };
        if !pens_ok {
            out.push(format!(
                "{who}: {what}[{i}] stroke is not the QET-px pen of the native twin's stroke"
            ));
        }
        if let (Primitive::Text(x), Primitive::Text(y)) = (a, b)
            && (x.text != y.text || x.align != y.align || x.angle != y.angle || x.style != y.style)
        {
            out.push(format!(
                "{who}: {what}[{i}] text content or alignment differs from the native twin"
            ));
        }
    }
}

fn compare_cps(who: &str, what: &str, n: &[SymbolCp], q: &[SymbolCp], out: &mut Vec<String>) {
    if n.len() != q.len() {
        out.push(format!(
            "{who}: {what} has {} CPs, the native twin {}",
            q.len(),
            n.len()
        ));
        return;
    }
    for (a, b) in n.iter().zip(q) {
        if a.idx != b.idx || a.fn_cp != b.fn_cp || a.dir != b.dir || a.discipline != b.discipline {
            out.push(format!(
                "{who}: {what} cp {} differs from the native twin in idx/fn_cp/dir/discipline",
                b.idx
            ));
        }
        if qet_pt(a.pos) != b.pos {
            out.push(format!(
                "{who}: {what} cp {} is at {:?}, expected {:?} (native {:?})",
                b.idx,
                b.pos,
                qet_pt(a.pos),
                a.pos
            ));
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// 5. TEXT SLOTS

#[test]
fn text_slots_and_arrangements() {
    let mut problems = Vec::new();
    for (_, d) in symbols() {
        let who = d.who();
        let def = &d.copy.item;
        let n = def.text_slots.len();
        if n > MAX_SLOTS {
            problems.push(format!("{who}: {n} text slots, at most {MAX_SLOTS} allowed"));
        }
        for (i, s) in def.text_slots.iter().enumerate() {
            if let SlotKind::CpDesignation(idx) = &s.kind
                && def.cps.get(idx).is_none()
            {
                problems.push(format!(
                    "{who}: text slot {i} designates CP {idx}, which does not exist"
                ));
            }
            if let Some(style) = &s.style
                && ew_text::builtin().get(style.as_str()).is_none()
            {
                problems.push(format!(
                    "{who}: text slot {i} names the unknown text style {:?}",
                    style.as_str()
                ));
            }
        }
        // Doc 06 §2.8 and doc 20 §17.5 bind `xref.device` to the cross-reference texts of devices (the aux text
        // "(…)" under the tag, the main list after it). The interruption-point symbols print their partner text
        // in `data.small`, the default style of the slot kind, and name no style (FT-84).
        let base = d.key.strip_suffix("@qet10").unwrap_or(&d.key);
        let want = if INTERRUPTION_POINTS.contains(&base) {
            None
        } else {
            Some("xref.device")
        };
        for (i, s) in def.text_slots.iter().enumerate() {
            if s.kind == SlotKind::Xref && s.style.as_ref().map(|n| n.as_str()) != want {
                problems.push(format!("{who}: xref slot {i} must name the style {want:?}"));
            }
        }
        let mut names = BTreeSet::new();
        for a in &def.arrangements {
            if !names.insert(a.name.as_str()) {
                problems.push(format!(
                    "{who}: arrangement name {:?} is not unique",
                    a.name.as_str()
                ));
            }
            if let Some(i) = a.slots.iter().find(|i| usize::from(**i) >= n) {
                problems.push(format!(
                    "{who}: arrangement {:?} lists slot {i}, only {n} slots exist",
                    a.name.as_str()
                ));
            }
        }
        if n == 0 {
            continue;
        }
        let find = |name: &str| def.arrangements.iter().find(|a| a.name.as_str() == name);
        match find("full") {
            None => problems.push(format!(
                "{who}: a symbol with text slots needs an arrangement \"full\""
            )),
            Some(a) => {
                let all: Vec<u16> = (0..n as u16).collect();
                if a.slots.as_slice() != all.as_slice() {
                    problems.push(format!(
                        "{who}: arrangement \"full\" must list all {n} slot indices"
                    ));
                }
            }
        }
        let tags: Vec<u16> = def
            .text_slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind == SlotKind::Tag)
            .map(|(i, _)| i as u16)
            .collect();
        if !tags.is_empty() {
            match find("tag") {
                None => problems.push(format!("{who}: a Tag slot needs an arrangement \"tag\"")),
                Some(a) if a.slots.as_slice() != tags.as_slice() => {
                    problems.push(format!(
                        "{who}: arrangement \"tag\" must contain exactly the Tag slot(s) {tags:?}"
                    ));
                }
                Some(_) => {}
            }
        }
    }
    assert_clean("text slots", &problems);
}

// ---------------------------------------------------------------------------------------------------------
// 6. GEOMETRY RULES

#[test]
fn geometry_rules() {
    let mut problems = Vec::new();
    for (ed, d) in symbols() {
        let who = d.who();
        let def = &d.copy.item;
        let tol = if ed == Edition::Native {
            OUTLINE_TOL
        } else {
            OUTLINE_TOL * 2 / 3
        };
        for (label, graphics, cps) in
            std::iter::once(("graphics", def.graphics.as_slice(), def.cps.as_slice())).chain(
                def.extra_variants
                    .iter()
                    .map(|v| ("extra variant", v.graphics.as_slice(), v.cps.as_slice())),
            )
        {
            if graphics.is_empty() {
                continue;
            }
            for cp in cps {
                if !on_outline(cp.pos, graphics, tol) {
                    problems.push(format!(
                        "{who}: cp {} at {:?} lies on no outline or line end of the {label} (doc 07 §3.2 rule 3)",
                        cp.idx, cp.pos
                    ));
                }
            }
        }
        for (i, prim) in all_graphics(def).enumerate() {
            if matches!(prim, Primitive::Image(_)) {
                problems.push(format!("{who}: graphic {i} is an Image"));
            }
            if matches!(prim, Primitive::Unknown(_)) {
                problems.push(format!("{who}: graphic {i} is of an unknown kind"));
            }
            if let Some(s) = stroke_of(prim) {
                let houses = [Pen::P013, Pen::P018, Pen::P026, Pen::P042, Pen::P052, Pen::P130];
                // The qet10 edition expresses the house pens in QET px (doc 20 §17.4).
                let house = match ed {
                    Edition::Native => houses.contains(&s.pen),
                    Edition::Qet10 => houses.iter().any(|h| qet_pen(h).as_ref() == Some(&s.pen)),
                };
                if !house {
                    problems.push(format!(
                        "{who}: graphic {i} uses the pen {:?}, not a house pen",
                        s.pen
                    ));
                }
                let dashable = [Pen::P013, Pen::P026];
                let dash_ok =
                    dashable.contains(&s.pen) || dashable.iter().any(|h| qet_pen(h).as_ref() == Some(&s.pen));
                if s.dash == Dash::Dash42 && !dash_ok {
                    problems.push(format!(
                        "{who}: graphic {i} dashes the pen {:?}; dash42 is for p013 and p026",
                        s.pen
                    ));
                }
                if matches!(s.dash, Dash::Unknown(_)) {
                    problems.push(format!("{who}: graphic {i} has an unknown dash"));
                }
                if s.colour != Colour::ByLayer && !is_white(&s.colour) {
                    problems.push(format!(
                        "{who}: graphic {i} has a stroke colour other than by_layer or white"
                    ));
                }
            }
            if let Some(f) = fill_of(prim)
                && *f != Colour::ByLayer
                && !is_white(f)
            {
                problems.push(format!(
                    "{who}: graphic {i} is filled with {f:?}, only by_layer or #ffffff are allowed"
                ));
            }
        }
    }
    assert_clean("geometry rules", &problems);
}

/// IEC 60417-5019 / -5018: protective earth is the earth sign inside a circle, functional earth is the plain
/// earth sign. Guards against the two symbols being swapped again.
#[test]
fn earth_symbols_pe_has_the_circle_and_fe_has_none() {
    let mut seen = BTreeSet::new();
    let mut problems = Vec::new();
    for (ed, d) in symbols() {
        let want = match d.key.strip_suffix(QET_SUFFIX).unwrap_or(&d.key) {
            "ew.iec.pe" => 1,
            "ew.iec.fe" => 0,
            _ => continue,
        };
        seen.insert(d.key.clone());
        let circles = all_graphics(&d.copy.item)
            .filter(|p| matches!(p, Primitive::Circle(_)))
            .count();
        if circles != want {
            problems.push(format!(
                "{} ({ed:?}): {circles} circle graphic(s), expected {want} (protective earth is the earth sign inside a circle, functional earth has none)",
                d.who()
            ));
        }
        // No text slot anchor of the protective earth may lie inside or on its circle (exact, in nm).
        for p in all_graphics(&d.copy.item) {
            let Primitive::Circle(c) = p else { continue };
            let r2 = i128::from(c.r.0) * i128::from(c.r.0);
            for (i, slot) in d.copy.item.text_slots.iter().enumerate() {
                let dx = i128::from(slot.pos.x.0) - i128::from(c.centre.x.0);
                let dy = i128::from(slot.pos.y.0) - i128::from(c.centre.y.0);
                if dx * dx + dy * dy <= r2 {
                    problems.push(format!(
                        "{} ({ed:?}): text slot {i} at {:?} lies inside or on the circle",
                        d.who(),
                        slot.pos
                    ));
                }
            }
        }
    }
    assert_eq!(seen.len(), 4, "pe and fe must exist in both editions: {seen:?}");
    assert_clean("earth symbols", &problems);
}

fn is_white(c: &Colour) -> bool {
    matches!(c, Colour::Rgb(rgb) if rgb.0 == [255, 255, 255])
}

// ---------------------------------------------------------------------------------------------------------
// 7. LOWER

/// `d` flipped left/right (the mirror before the turns).
fn flip(d: &Dir4) -> Dir4 {
    match d {
        Dir4::Left => Dir4::Right,
        Dir4::Right => Dir4::Left,
        other => other.clone(),
    }
}

/// `d` turned clockwise by `n` quarter turns.
fn turn(d: &Dir4, n: u8) -> Dir4 {
    let mut d = d.clone();
    for _ in 0..n {
        d = match d {
            Dir4::Up => Dir4::Right,
            Dir4::Right => Dir4::Down,
            Dir4::Down => Dir4::Left,
            Dir4::Left => Dir4::Up,
            other => other,
        };
    }
    d
}

#[test]
fn lower_in_all_variants() {
    let mut problems = Vec::new();
    for (_, d) in symbols() {
        let who = d.who();
        let def = &d.copy.item;
        let mut want: Vec<(u16, u16)> = def.cps.iter().map(|c| (c.idx, c.fn_cp)).collect();
        want.sort_unstable();
        let r0 = lower(def, Orient2::R0);
        for v in Orient2::ALL {
            let g = lower(def, v);
            let vl = v.letter();
            let mut got: Vec<(u16, u16)> = g.cps.iter().map(|c| (c.idx, c.fn_cp)).collect();
            got.sort_unstable();
            if got != want {
                problems.push(format!(
                    "{who} {vl}: placed (idx, fn_cp) multiset {got:?} differs from the definition {want:?}"
                ));
            }
            for (cp, base) in g.cps.iter().zip(&r0.cps) {
                let src = def.cps.get(&cp.idx).map(|c| c.pos);
                if src.map(|p| v.apply(p)) != Some(cp.pos) {
                    problems.push(format!(
                        "{who} {vl}: cp {} is at {:?}, not the variant image of its position",
                        cp.idx, cp.pos
                    ));
                }
                if !pt_on_grid(def.grid, cp.pos) {
                    problems.push(format!(
                        "{who} {vl}: cp {} at {:?} is off the symbol grid",
                        cp.idx, cp.pos
                    ));
                }
                let base_dir = if v.is_mirrored() {
                    flip(&base.dir)
                } else {
                    base.dir.clone()
                };
                let expect = turn(&base_dir, v.quarter_turns());
                if cp.dir != expect {
                    problems.push(format!(
                        "{who} {vl}: cp {} points {:?}, expected {expect:?}",
                        cp.idx, cp.dir
                    ));
                }
            }
            let anchor = v.apply(resolve_anchor(def));
            if g.anchor != anchor || !pt_on_grid(def.grid, g.anchor) {
                problems.push(format!(
                    "{who} {vl}: anchor {:?} is not the snapped, transformed anchor {anchor:?}",
                    g.anchor
                ));
            }
        }
    }
    assert_clean("lowering", &problems);
}

// ---------------------------------------------------------------------------------------------------------
// 8. KEY SET

#[test]
fn all_starter_keys_present() {
    let l = lib();
    let expect_native: BTreeSet<String> = STARTER_KEYS.iter().map(|k| (*k).to_owned()).collect();
    let expect_qet: BTreeSet<String> = STARTER_KEYS.iter().map(|k| format!("{k}{QET_SUFFIX}")).collect();
    let mut problems = Vec::new();
    for (what, expect, have) in [
        (
            "native",
            expect_native,
            l.native.iter().map(|d| d.key.clone()).collect::<BTreeSet<_>>(),
        ),
        (
            "qet10",
            expect_qet,
            l.qet10.iter().map(|d| d.key.clone()).collect::<BTreeSet<_>>(),
        ),
    ] {
        for k in expect.difference(&have) {
            problems.push(format!("{what}: missing {k}"));
        }
        for k in have.difference(&expect) {
            problems.push(format!("{what}: unexpected {k}"));
        }
    }
    assert_clean("starter key set", &problems);
}

// ---------------------------------------------------------------------------------------------------------
// 9. SNAPSHOTS

fn pt_s(p: Pt2) -> String {
    format!("({},{})", p.x.0, p.y.0)
}

fn render_variant(out: &mut String, v: Orient2, g: &SymbolGeometry) {
    writeln!(out, "{}", v.letter()).unwrap();
    for c in &g.cps {
        writeln!(out, "  cp {} fn {} {} {:?}", c.idx, c.fn_cp, pt_s(c.pos), c.dir).unwrap();
    }
    for t in &g.texts {
        let source = match &t.source {
            TextSource::Static(m) => format!("static {:?}", m.as_neutral().unwrap_or("?")),
            TextSource::Slot(k) => format!("slot {k:?}"),
        };
        let align: &Align = &t.align;
        writeln!(
            out,
            "  text {source} {} {} {align:?} visible={}",
            pt_s(t.pos),
            t.angle.0,
            t.visible
        )
        .unwrap();
    }
    writeln!(out, "  anchor {}", pt_s(g.anchor)).unwrap();
    writeln!(out, "  bounds {} {}", pt_s(g.bounds.min()), pt_s(g.bounds.max())).unwrap();
    writeln!(out, "  items {}", g.graphics.items.len()).unwrap();
    for (i, item) in g.graphics.items.iter().enumerate() {
        writeln!(out, "    {i} {}", render_item(item)).unwrap();
    }
}

/// One line per lowered item: the kind, the pen (width, dash, colour) and the geometry in integer nm.
/// Circles and arcs are lowered to cubic Beziers, so they appear as their `C` elements.
fn render_item(item: &ew_dl::Item) -> String {
    use ew_dl::{Brush, ItemKind, PathEl};
    let colour = |b: &Brush| {
        let Brush::Solid(c) = b;
        format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a)
    };
    match &item.kind {
        ItemKind::Path(p) => {
            let els: Vec<String> = p
                .path
                .els
                .iter()
                .map(|e| match e {
                    PathEl::MoveTo(a) => format!("M{}", pt_s(*a)),
                    PathEl::LineTo(a) => format!("L{}", pt_s(*a)),
                    PathEl::QuadTo(a, b) => format!("Q{}{}", pt_s(*a), pt_s(*b)),
                    PathEl::CubicTo(a, b, c) => format!("C{}{}{}", pt_s(*a), pt_s(*b), pt_s(*c)),
                    PathEl::ClosePath => "Z".to_owned(),
                })
                .collect();
            let pen = p.stroke.as_ref().map_or("none".to_owned(), |s| {
                let dash = s.dash.as_ref().map_or(String::new(), |d| {
                    let seg: Vec<String> = d.segments.iter().map(|n| n.0.to_string()).collect();
                    format!(" dash[{}]@{}", seg.join(","), d.offset.0)
                });
                format!("w{}{dash} {}", s.width.0, colour(&s.brush))
            });
            let fill = p.fill.as_ref().map_or("none".to_owned(), colour);
            format!("path pen {pen} fill {fill} {}", els.join(" "))
        }
        ItemKind::Glyphs(r) => format!("glyphs {:?} at {} size {}", r.text, pt_s(r.origin), r.size.0),
        ItemKind::Image(i) => format!("image {} {}", pt_s(i.rect.min()), pt_s(i.rect.max())),
        ItemKind::Instance(_) => "instance".to_owned(),
        ItemKind::Group(g) => format!("group of {}", g.items.len()),
    }
}

fn render_symbol(key: &str, v: Orient2) -> Option<String> {
    let d = lib().native.iter().find(|d| d.key == key)?;
    let mut text = String::new();
    render_variant(&mut text, v, &lower(&d.copy.item, v));
    Some(text)
}

#[test]
fn snapshot_mcb_1pn_r0() {
    let text = render_symbol("ew.iec.mcb-1pn", Orient2::R0).expect("ew.iec.mcb-1pn exists");
    insta::assert_snapshot!(text, @r#"
    A
      cp 0 fn 0 (0,0) Up
      cp 1 fn 1 (0,16000000) Down
      cp 2 fn 2 (12000000,0) Up
      cp 3 fn 3 (12000000,16000000) Down
      text slot Tag (-4000000,8000000) 0 MiddleRight visible=true
      text slot Manufacturer (15000000,3000000) 0 MiddleLeft visible=true
      text slot PartNumber (15000000,8000000) 0 MiddleLeft visible=true
      text slot TechData (15000000,13000000) 0 MiddleLeft visible=true
      text slot FunctionText (-4000000,12000000) 0 MiddleRight visible=false
      text slot CpDesignation(0) (1000000,2000000) 0 MiddleLeft visible=true
      text slot CpDesignation(1) (1000000,14000000) 0 MiddleLeft visible=true
      text slot CpDesignation(2) (13000000,2000000) 0 MiddleLeft visible=true
      text slot CpDesignation(3) (13000000,14000000) 0 MiddleLeft visible=true
      anchor (4000000,8000000)
      bounds (-4000000,0) (15000000,16000000)
      items 14
        0 path pen w260000 #000000ff fill none M(0,0) L(0,5000000)
        1 path pen w260000 #000000ff fill none M(0,16000000) L(0,12000000)
        2 path pen w260000 #000000ff fill none M(0,12000000) L(-2000000,5000000)
        3 path pen w260000 #000000ff fill none M(-1000000,4000000) L(1000000,6000000)
        4 path pen w260000 #000000ff fill none M(-1000000,6000000) L(1000000,4000000)
        5 path pen w260000 #000000ff fill none M(12000000,0) L(12000000,5000000)
        6 path pen w260000 #000000ff fill none M(12000000,16000000) L(12000000,12000000)
        7 path pen w260000 #000000ff fill none M(12000000,12000000) L(10000000,5000000)
        8 path pen w260000 #000000ff fill none M(11000000,4000000) L(13000000,6000000)
        9 path pen w260000 #000000ff fill none M(11000000,6000000) L(13000000,4000000)
        10 path pen w130000 dash[520000,260000]@0 #000000ff fill none M(-1000000,8500000) L(2000000,8500000)
        11 path pen w260000 #000000ff fill none M(2000000,7000000) L(2000000,10000000) L(4000000,10000000) L(4000000,7000000) Z
        12 path pen w130000 #000000ff fill none M(4000000,8500000) L(4500000,7500000) L(5500000,9500000) L(6000000,8500000)
        13 path pen w130000 dash[520000,260000]@0 #000000ff fill none M(6000000,8500000) L(11000000,8500000)
    "#);
}

#[test]
fn snapshot_contact_no_m90() {
    let Some(text) = render_symbol("ew.iec.contact-no", Orient2::M90) else {
        eprintln!("note: content/symbols/ew.iec.contact-no.json does not exist yet; snapshot skipped");
        return;
    };
    insta::assert_snapshot!(text, @r#"
    F
      cp 0 fn 0 (0,0) Right
      cp 1 fn 1 (-12000000,0) Left
      text slot Tag (-6000000,4000000) 90000 MiddleRight visible=true
      text slot Xref (-6000000,-4000000) 90000 MiddleLeft visible=true
      text slot CpDesignation(0) (-2000000,-1000000) 90000 MiddleLeft visible=true
      text slot CpDesignation(1) (-10000000,-1000000) 90000 MiddleLeft visible=true
      anchor (0,0)
      bounds (-12000000,-4000000) (0,4000000)
      items 3
        0 path pen w260000 #000000ff fill none M(0,0) L(-4000000,0)
        1 path pen w260000 #000000ff fill none M(-12000000,0) L(-8000000,0)
        2 path pen w260000 #000000ff fill none M(-8000000,0) L(-4000000,2000000)
    "#);
}

// ---------------------------------------------------------------------------------------------------------
// Content blessing

/// With `EW_BLESS_CONTENT=1`, rewrites every native symbol and fndef file to its canonical text; does
/// nothing otherwise. Envelope values are not touched.
#[test]
fn bless_native_documents() {
    if std::env::var("EW_BLESS_CONTENT").as_deref() != Ok("1") {
        return;
    }
    let content = content_dir();
    let mut changed = Vec::new();
    for dir in [content.join("symbols"), content.join("fndefs")] {
        let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "json"))
            .collect();
        paths.sort();
        for path in paths {
            let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let doc = LibDocument::from_json(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let canonical = serde_json::to_string_pretty(&doc).expect("serialisable") + "\n";
            if canonical != text {
                fs::write(&path, &canonical).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                changed.push(path_label(&path));
            }
        }
    }
    eprintln!("blessed {} file(s): {changed:?}", changed.len());
}
