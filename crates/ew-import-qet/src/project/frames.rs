// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A diagram's border and title block → a frame form and a title-block form of the project's QET import
//! library (doc 14 §4.9).
//!
//! - **Frame.** QET draws a diagram's border 5 px right of and below the scene origin (`Diagram::margin`),
//!   with a 20 px header band, then `cols × colsize` and `rows × rowsize`. The frame form keeps the scene
//!   origin as the page origin, so every imported position stays on the 10 px grid: its header band counts
//!   from the origin (25 px), its outer rectangle starts at the margin (`ew.frame.margin`), its columns are
//!   numbered from 1 as QET's default numbers them, and `displaycols`/`displayrows="false"` on both axes
//!   hides the bands (`ew.frame.show_grid`; one hidden band alone is reported, both bands show). The sheet is
//!   A4, landscape unless the drawing is taller than wide, and the page maps to it as QET's PDF export maps
//!   the scene: the border's corner at the sheet's corner, scaled to fit the drawing one pixel wider and
//!   higher than the border and title block.
//! - **Title block.** The diagram's template (`titleblocktemplate`, collection `embedded`) becomes a
//!   title-block form as wide as the border: columns in px, `tN%` of the width and `rN%` of the width left
//!   (rounded to whole px), rows in px; every cell's edges drawn once, the outline left to the frame. A field
//!   prints its label (`displaylabel`) and its value; the folio variables bind to the page (`%title`,
//!   `%folio`, `%folio-id`, `%folio-total`, `%previous-folio-num`, `%next-folio-num`), every other variable
//!   is filled with the diagram's value (its title-block attributes, then the project's title and
//!   properties, then the diagram's own properties), and variables naming a local path print nothing
//!   (reported). Font sizes become the styles `qet.<pt>` ([`styles`]). A logo stored as base64 (a PNG) or
//!   as XML (an SVG) becomes an image in its cell, fitted and centred ([`logo_image`]); one the asset rules
//!   refuse is reported. Without an embedded template the shipped `ew.titleblock.qet-a4` (QET's default)
//!   applies.
//! - Forms are content-addressed: equal forms are one copy.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_interop_api::{Code, Severity};
use ew_mltext::Ident;
use ew_schema::{
    Align, Bounds, FormCell, FormDef, FormKind, FrameColumns, FrameGrid, FrameRows, ImagePrim, ItemMeta,
    LibCopy, LibItem, Library, Overflow, Primitive, SheetSpec, SheetTransform, ShrinkSpec, TextStyleSpec,
};
use ew_units::{Grid, Nm, Pt2};
use serde_json::{Value, json};
use sha2::Digest;

use super::builtins::Builtins;
use super::cx::{Findings, Ids};
use super::items::ItemsOut;
use super::model::{QetDiagram, QetProject, TbCell, TbTemplate};
use crate::element::ITEM_VERSION;

/// The key of the shipped title block that reproduces QET's default template.
pub const QET_TITLEBLOCK_KEY: &str = "ew.titleblock.qet-a4";

/// QET's `Diagram::margin`: the border's distance from the scene origin, px.
const MARGIN_PX: i64 = 5;
/// QET's header bands (`rows_header_width`, `columns_header_height`), px.
const HEADER_PX: i64 = 20;
/// The frame and title-block line width: QET's 1 px pen.
const LINE_PX: i64 = 1;
/// A4, nm.
const A4: (i64, i64) = (297_000_000, 210_000_000);
/// The smallest printed cap height a shrinking title-block text keeps (QET compresses without limit).
const MIN_CAP: Nm = Nm(500_000);
/// The font size of a field without a usable one, pt (QET's default).
const DEFAULT_PT: u16 = 9;
/// The largest border QET values the importer takes: count and size each up to these.
const MAX_COUNT: i64 = 1_000;
const MAX_SIZE_PX: i64 = 10_000;

/// What the frames of a project's diagrams are.
#[derive(Default)]
pub struct Frames {
    /// The frame form of each diagram (by diagram index); `None` when none could be made.
    pub per_diagram: Vec<Option<Id<FormDef>>>,
    /// The new forms, frame and title-block, in first-use order, each once.
    pub forms: Vec<LibCopy<FormDef>>,
    /// The font sizes (pt) the title-block cells use.
    pub sizes: BTreeSet<u16>,
}

/// One QET px in nm: `round(px × 793 750 / 3)`.
fn px(n: i64) -> Nm {
    Grid::QET_1PX.at(n).unwrap_or(Nm::ZERO)
}

/// A step of `n` px as a rational grid.
fn px_grid(n: i64) -> Option<Grid> {
    Grid::new(
        u64::try_from(n).ok()?.checked_mul(Grid::QET_1PX.num())?,
        Grid::QET_1PX.den(),
    )
    .ok()
}

/// The style key of `pt` points: the import style of a regular QET font of that size
/// ([`crate::prims::text::style_key`]), so a title block and a drawing share their styles.
fn style_key(pt: u16) -> String {
    crate::prims::text::style_key(crate::decimal::Decimal::from_int(i64::from(pt)), false, false)
}

/// The text styles of `sizes`: `qet.<pt>` with the em height of `pt` points on QET's 96 dpi scene
/// ([`crate::prims::text::style_spec`]), for `ProjectSettings.text_styles`.
pub fn styles(sizes: &BTreeSet<u16>) -> BTreeMap<Ident, TextStyleSpec> {
    sizes
        .iter()
        .filter_map(|&pt| {
            let key = style_key(pt);
            Some((
                Ident::from_input(&key).ok()?,
                crate::prims::text::style_spec(&key)?,
            ))
        })
        .collect()
}

/// A library copy of the form `item` in the project's import library, content-addressed.
fn copy(ids: &Ids, library: Id<Library>, item: FormDef) -> LibCopy<FormDef> {
    let json = serde_json::to_string(&item).unwrap_or_default();
    let digest: [u8; 32] = sha2::Sha256::digest(json.as_bytes()).into();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    let item_id: Id<LibItem> = ids.id(&format!("qet:form:{hex}"));
    LibCopy {
        id: ew_ids::v5::lib_copy_id(library.as_uuid(), item_id.as_uuid()),
        v: 1,
        library_id: library,
        item_id,
        version: ITEM_VERSION.to_owned(),
        sha256: ew_schema::Sha256(digest),
        item,
    }
}

/// An empty form of `kind` on a `w × h` sheet.
fn form(kind: FormKind, w: Nm, h: Nm) -> FormDef {
    FormDef {
        kind,
        sheet: SheetSpec {
            width: w,
            height: h,
            extra: Default::default(),
        },
        page_to_sheet: None,
        grid: None,
        title_block: None,
        graphics: Vec::new(),
        cells: Vec::new(),
        data: None,
        continuation: None,
        meta: ItemMeta::default(),
        extra: Default::default(),
    }
}

/// A 1 px line from `a` to `b` (px).
fn line(a: (i64, i64), b: (i64, i64)) -> Option<Primitive> {
    let v = json!({"line": {
        "from": [px(a.0).0, px(a.1).0],
        "to": [px(b.0).0, px(b.1).0],
        "stroke": {"pen": {"width": px(LINE_PX).0}}
    }});
    serde_json::from_value(v).ok()
}

/// A border value: the integer in `1..=max`, else `default` (and `false`).
fn border_value(v: Option<i64>, default: i64, max: i64) -> (i64, bool) {
    match v {
        Some(n) if (1..=max).contains(&n) => (n, true),
        None => (default, true),
        Some(_) => (default, false),
    }
}

/// A border size in px: a whole number in `1..=MAX_SIZE_PX` (`60`, `60.0`), else `default` (and `false`
/// when one was written).
fn size(d: Option<crate::decimal::Decimal>, default: i64) -> (i64, bool) {
    let Some(d) = d else {
        return (default, true);
    };
    let s = d.to_plain_string();
    let v = s.strip_suffix(".0").unwrap_or(&s).parse::<i64>().ok();
    match v {
        Some(v) => border_value(Some(v), default, MAX_SIZE_PX),
        None => (default, false),
    }
}

/// Builds the frames of `project`'s diagrams; title-block logos join `assets`.
pub fn build(
    project: &QetProject,
    ids: &Ids,
    library: Id<Library>,
    builtins: &Builtins,
    used: &mut BTreeSet<String>,
    assets: &mut ItemsOut,
    findings: &mut Findings,
) -> Frames {
    let mut out = Frames::default();
    let mut seen: BTreeSet<Id<FormDef>> = BTreeSet::new();
    let mut reported: BTreeSet<String> = BTreeSet::new();
    let mut report_once = |findings: &mut Findings, message: String| {
        if reported.insert(message.clone()) {
            findings.file_level(Code::DROPPED_BY_DESIGN, Severity::Info, message);
        }
    };
    let templates: BTreeMap<&str, &TbTemplate> = project
        .templates
        .iter()
        .rev()
        .map(|t| (t.name.as_str(), t))
        .collect();
    let shipped = builtins.form(QET_TITLEBLOCK_KEY);
    for d in &project.diagrams {
        let b = &d.border;
        let (cols, ok1) = border_value(b.cols.map(i64::from), 17, MAX_COUNT);
        let (rows, ok2) = border_value(b.rows.map(i64::from), 8, MAX_COUNT);
        let (colsize, ok3) = size(b.colsize, 60);
        let (rowsize, ok4) = size(b.rowsize, 80);
        if !(ok1 && ok2 && ok3 && ok4) {
            report_once(
                findings,
                "a border with a count or size the importer does not take keeps QET's default of 17 columns of \
                 60 px and 8 rows of 80 px"
                    .to_owned(),
            );
        }
        let width = HEADER_PX + cols * colsize;
        let height = HEADER_PX + rows * rowsize;
        let attr = |n: &str| d.titleblock.iter().find(|(k, _)| k == n).map(|(_, v)| v.as_str());
        if attr("displayAt").is_some_and(|v| v != "bottom") {
            report_once(
                findings,
                "a title block QET draws at the right of the border is drawn below it".to_owned(),
            );
        }
        // The title block.
        let template = attr("titleblocktemplate")
            .filter(|n| !n.is_empty())
            .filter(|_| matches!(d.template_collection.as_str(), "" | "embedded"))
            .and_then(|n| templates.get(n).copied());
        if template.is_none() && attr("titleblocktemplate").is_some_and(|n| !n.is_empty()) {
            report_once(
                findings,
                format!(
                    "the title-block template `{}` is not in the project; QET's default title block is drawn",
                    attr("titleblocktemplate").unwrap_or_default()
                ),
            );
        }
        let tb = match template {
            Some(t) => {
                let ctx = context(project, d);
                let made = title_block(t, width, &ctx, &mut out.sizes, assets, &mut |m| {
                    report_once(findings, m)
                });
                made.map(|f| {
                    let c = copy(ids, library, f);
                    (c.id, c.item.sheet.height, Some(c))
                })
            }
            None => shipped.map(|c| {
                used.insert(QET_TITLEBLOCK_KEY.to_owned());
                (c.id, c.item.sheet.height, None)
            }),
        };
        let tb_h = tb.as_ref().map_or(Nm::ZERO, |t| t.1);
        let tb_id = tb.as_ref().map(|t| t.0);
        if let Some((id, _, Some(c))) = tb
            && seen.insert(id)
        {
            out.forms.push(c);
        }
        // The frame.
        let hidden = (b.display_cols == Some(false), b.display_rows == Some(false));
        if hidden.0 != hidden.1 {
            report_once(
                findings,
                "a border that hides only its column or only its row header shows both".to_owned(),
            );
        }
        let frame = frame(
            (cols, colsize, rows, rowsize),
            (width, height),
            tb_h,
            hidden.0 && hidden.1,
            tb_id,
        );
        let id = frame.map(|f| {
            let c = copy(ids, library, f);
            let id = c.id;
            if seen.insert(id) {
                out.forms.push(c);
            }
            id
        });
        out.per_diagram.push(id);
    }
    out
}

/// The frame form of a border (`cols, colsize, rows, rowsize` px) of `width × height` px with a title block
/// of `tb_h` below it.
fn frame(
    (cols, colsize, rows, rowsize): (i64, i64, i64, i64),
    (width, height): (i64, i64),
    tb_h: Nm,
    hide_bands: bool,
    title_block: Option<Id<FormDef>>,
) -> Option<FormDef> {
    // The drawing QET exports: the border and title block, one pixel wider and higher.
    let w = i128::from(px(width + 1).0);
    let h = i128::from(px(height + 1).0) + i128::from(tb_h.0);
    let (sw, sh) = if w >= h { A4 } else { (A4.1, A4.0) };
    let fit = |s: i64, d: i128| (i128::from(s) * 1_000_000_000) / d.max(1);
    let scale = fit(sw, w).min(fit(sh, h)).clamp(1, i128::from(u32::MAX));
    // The margin's exact length 5 × 793 750/3 nm, scaled and rounded.
    let m = (i128::from(MARGIN_PX) * i128::from(Grid::QET_1PX.num()) * scale
        + i128::from(Grid::QET_1PX.den()) * 500_000_000)
        / (i128::from(Grid::QET_1PX.den()) * 1_000_000_000);
    let offset = Nm(-i64::try_from(m).ok()?);
    let mut extra = serde_json::Map::new();
    extra.insert("ew.frame.line_width".into(), Value::from(px(LINE_PX).0));
    extra.insert("ew.frame.margin".into(), Value::from(px(MARGIN_PX).0));
    if hide_bands {
        extra.insert("ew.frame.show_grid".into(), Value::Bool(false));
    }
    let mut f = form(FormKind::Frame, Nm(sw), Nm(sh));
    f.page_to_sheet = Some(SheetTransform {
        scale_ppb: u32::try_from(scale).ok()?,
        offset: Pt2::new(offset, offset),
        extra: Default::default(),
    });
    f.grid = Some(FrameGrid {
        header_band: Some(px_grid(MARGIN_PX + HEADER_PX)?),
        columns: FrameColumns {
            count: u16::try_from(cols).ok()?,
            start_index: 1,
            width: px_grid(colsize)?,
            labels: Vec::new(),
            extra: Default::default(),
        },
        rows: FrameRows {
            count: u16::try_from(rows).ok()?,
            labels: Vec::new(),
            skip_io: false,
            height: px_grid(rowsize)?,
            extra: Default::default(),
        },
        extra: extra.into_iter().collect(),
    });
    f.title_block = title_block;
    Some(f)
}

/// What a QET variable becomes in a cell: a binding of the page or a literal value.
enum Var {
    /// An EW placeholder path (`page.folio`).
    Bound(&'static str),
    /// The diagram's value.
    Text(String),
}

/// The QET variables EW binds to the page, by name.
const BOUND: [(&str, &str); 6] = [
    ("title", "page.title"),
    ("folio", "page.folio"),
    ("folio-id", "doc.page_index"),
    ("folio-total", "doc.page_count"),
    ("previous-folio-num", "page.prev"),
    ("next-folio-num", "page.next"),
];

/// QET's variables that hold a local file path; they print nothing.
const PATHS: [&str; 3] = ["projectpath", "projectfilename", "savedfilepath"];

/// The variables of diagram `d`, by name; the first source that names one wins.
fn context(project: &QetProject, d: &QetDiagram) -> BTreeMap<String, Var> {
    let mut ctx: BTreeMap<String, Var> = BTreeMap::new();
    for (name, path) in BOUND {
        ctx.insert(name.to_owned(), Var::Bound(path));
    }
    for name in PATHS {
        ctx.insert(name.to_owned(), Var::Text(String::new()));
    }
    let mut add = |name: &str, value: &str| {
        ctx.entry(name.to_owned())
            .or_insert_with(|| Var::Text(value.to_owned()));
    };
    for (name, value) in &d.titleblock {
        // QET writes `date="null"` for a diagram without a date.
        add(
            name,
            if name == "date" && value == "null" {
                ""
            } else {
                value
            },
        );
    }
    add("projecttitle", project.title.as_deref().unwrap_or(""));
    for (name, value) in project.properties.iter().chain(&d.properties) {
        add(name, value);
    }
    ctx
}

/// A QET cell text with its variables replaced: an EW template (`{…}` placeholders, literal braces
/// doubled). `%{name}` names any variable; `%name` the longest known name at that point (QET replaces
/// names longest first); an unknown variable stays as written, as QET leaves it.
fn template(text: &str, ctx: &BTreeMap<String, Var>, report: &mut dyn FnMut(String)) -> String {
    let mut names: Vec<&str> = ctx.keys().map(String::as_str).collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    let mut out = String::new();
    let literal = |out: &mut String, s: &str| {
        for c in s.chars() {
            match c {
                '{' => out.push_str("{{"),
                '}' => out.push_str("}}"),
                c => out.push(c),
            }
        }
    };
    let mut emit = |out: &mut String, name: &str| match &ctx[name] {
        Var::Bound(path) => {
            out.push('{');
            out.push_str(path);
            out.push('}');
        }
        Var::Text(v) => {
            if PATHS.contains(&name) {
                report(format!(
                    "the title-block variable `%{name}` names a local file path and prints nothing"
                ));
            }
            literal(out, v);
        }
    };
    let mut rest = text;
    while let Some(at) = rest.find('%') {
        literal(&mut out, &rest[..at]);
        let after = &rest[at + 1..];
        if let Some(inner) = after.strip_prefix('{')
            && let Some(end) = inner.find('}')
            && ctx.contains_key(&inner[..end])
        {
            emit(&mut out, &inner[..end]);
            rest = &inner[end + 1..];
        } else if let Some(name) = names.iter().find(|n| after.starts_with(**n)) {
            emit(&mut out, name);
            rest = &after[name.len()..];
        } else {
            out.push('%');
            rest = after;
        }
    }
    literal(&mut out, rest);
    out
}

/// The widths in px of the template columns `specs` across `total` px: `Npx` or `N` absolute, `tN%` of
/// the total, `rN%` of what absolute and `t` columns leave; `None` for a part that does not parse.
fn column_widths(specs: &[String], total: i64) -> Vec<Option<i64>> {
    let pct = |s: &str| s.strip_suffix('%').and_then(|n| n.trim().parse::<i64>().ok());
    let abs = |s: &str| s.strip_suffix("px").unwrap_or(s).trim().parse::<i64>().ok();
    let round = |n: i64, p: i64| (n * p + 50).div_euclid(100);
    let fixed: i64 = specs
        .iter()
        .map(|s| match s.strip_prefix('t') {
            Some(t) => pct(t).map_or(0, |p| round(total, p)),
            None if s.starts_with('r') => 0,
            None => abs(s).unwrap_or(0),
        })
        .sum();
    let left = (total - fixed).max(0);
    specs
        .iter()
        .map(|s| {
            let w = if let Some(t) = s.strip_prefix('t') {
                pct(t).map(|p| round(total, p))
            } else if let Some(r) = s.strip_prefix('r') {
                pct(r).map(|p| round(left, p))
            } else {
                abs(s)
            };
            w.filter(|w| (0..=MAX_SIZE_PX * MAX_COUNT).contains(w))
        })
        .collect()
}

/// The title-block form of template `t` as wide as `width` px, filled from `ctx`.
fn title_block(
    t: &TbTemplate,
    width: i64,
    ctx: &BTreeMap<String, Var>,
    sizes: &mut BTreeSet<u16>,
    assets: &mut ItemsOut,
    report: &mut dyn FnMut(String),
) -> Option<FormDef> {
    let cols = column_widths(&t.cols, width);
    let rows: Vec<Option<i64>> = t
        .rows
        .iter()
        .map(|r| {
            r.strip_suffix("px")
                .unwrap_or(r)
                .trim()
                .parse::<i64>()
                .ok()
                .filter(|h| (0..=MAX_SIZE_PX).contains(h))
        })
        .collect();
    if cols.is_empty() || rows.is_empty() || cols.iter().chain(&rows).any(Option::is_none) {
        report(format!(
            "the grid of title-block template `{}` is not understood; QET's default title block is not drawn \
             in its place",
            t.name
        ));
        return None;
    }
    let edges = |v: &[Option<i64>]| -> Vec<i64> {
        std::iter::once(0)
            .chain(v.iter().scan(0, |acc, w| {
                *acc += w.unwrap_or(0);
                Some(*acc)
            }))
            .collect()
    };
    let (xs, ys) = (edges(&cols), edges(&rows));
    let (nc, nr) = (cols.len(), rows.len());
    let (tw, th) = (xs[nc], ys[nr]);
    // Cell regions: the template's cells, then every unit cell no region covers.
    let mut covered = vec![false; nc * nr];
    let mut regions: Vec<(usize, usize, usize, usize, Option<&TbCell>)> = Vec::new();
    for c in &t.cells {
        let (c0, r0) = (c.col as usize, c.row as usize);
        if c0 >= nc || r0 >= nr {
            report(format!(
                "a cell outside the grid of title-block template `{}` is not drawn",
                t.name
            ));
            continue;
        }
        let c1 = c0.saturating_add(c.colspan as usize).min(nc - 1);
        let r1 = r0.saturating_add(c.rowspan as usize).min(nr - 1);
        for r in r0..=r1 {
            for k in c0..=c1 {
                covered[r * nc + k] = true;
            }
        }
        regions.push((c0, r0, c1, r1, Some(c)));
    }
    for r in 0..nr {
        for k in 0..nc {
            if !covered[r * nc + k] {
                regions.push((k, r, k, r, None));
            }
        }
    }
    let mut f = form(FormKind::TitleBlock, px(tw), px(th));
    let mut segments: BTreeSet<((i64, i64), (i64, i64))> = BTreeSet::new();
    let mut logos: Vec<Primitive> = Vec::new();
    for &(c0, r0, c1, r1, cell) in &regions {
        let (x0, y0, x1, y1) = (xs[c0], ys[r0], xs[c1 + 1], ys[r1 + 1]);
        // The edges inside the outline (the frame draws the outline).
        if x0 > 0 {
            segments.insert(((x0, y0), (x0, y1)));
        }
        if x1 < tw {
            segments.insert(((x1, y0), (x1, y1)));
        }
        if y0 > 0 {
            segments.insert(((x0, y0), (x1, y0)));
        }
        if y1 < th {
            segments.insert(((x0, y1), (x1, y1)));
        }
        let Some(c) = cell else { continue };
        if let Some(logo) = &c.logo {
            match logo_image(t, logo, (x0, y0, x1, y1), assets) {
                Ok(image) => logos.push(image),
                Err(why) => report(format!(
                    "the title-block logo `{logo}` is not imported ({why}); its cell stays empty"
                )),
            }
            continue;
        }
        // QET prints `label: value` when the label is displayed.
        let mut value = template(&c.value, ctx, report);
        if c.display_label && !c.label.trim().is_empty() {
            value = format!("{}: {value}", template(c.label.trim(), ctx, report));
        }
        if value.trim().is_empty() {
            continue;
        }
        let pt = c
            .font_size
            .filter(|p| (2..=140).contains(p))
            .unwrap_or(DEFAULT_PT);
        sizes.insert(pt);
        let lines = value.lines().count();
        let mut extra = serde_json::Map::new();
        if lines > 1 {
            extra.insert("ew.cell.max_lines".into(), Value::from(lines));
        }
        let Ok(rect) = Bounds::new(Pt2::new(px(x0), px(y0)), Pt2::new(px(x1), px(y1))) else {
            continue;
        };
        f.cells.push(FormCell {
            rect,
            label: None,
            value: Some(value),
            style: Ident::from_input(&style_key(pt)).ok()?,
            align: align(&c.valign, &c.align),
            overflow: if c.hadjust {
                Overflow::ShrinkToFit(ShrinkSpec {
                    min_cap_height: MIN_CAP,
                    extra: Default::default(),
                })
            } else {
                Overflow::Error
            },
            visible_if: None,
            extra: extra.into_iter().collect(),
        });
    }
    f.graphics = segments
        .into_iter()
        .filter_map(|(a, b)| line(a, b))
        .chain(logos)
        .collect();
    Some(f)
}

/// The image of logo `name` of template `t` in the cell `(x0, y0, x1, y1)` px: its file (stored as base64, the
/// form QET writes a PNG in, or as XML, an SVG) taken into the batch's assets ([`ItemsOut::store_image`]),
/// drawn as large as the cell allows at the picture's own proportions and centred in it, as QET draws a logo.
fn logo_image(
    t: &TbTemplate,
    name: &str,
    (x0, y0, x1, y1): (i64, i64, i64, i64),
    assets: &mut ItemsOut,
) -> Result<Primitive, String> {
    let (_, storage, data) = t
        .logos
        .iter()
        .find(|(n, _, _)| n == name)
        .ok_or("the template has no such logo")?;
    let bytes = match storage.as_str() {
        "base64" => ew_schema::base64_decode(data).map_err(|e| e.to_string())?,
        "xml" => data.as_bytes().to_vec(),
        _ => return Err(format!("a logo stored as {storage} is not read")),
    };
    let (asset, size) = assets.store_image(bytes)?;
    let (w, h) = size.ok_or("the image has no size of its own")?;
    let (w, h) = (i128::from(w.max(1)), i128::from(h.max(1)));
    // The cell in nm, and the picture scaled to fit it.
    let (cx0, cy0, cx1, cy1) = (px(x0).0, px(y0).0, px(x1).0, px(y1).0);
    let (cw, ch) = (i128::from(cx1 - cx0), i128::from(cy1 - cy0));
    let (dw, dh) = if cw * h <= ch * w {
        (cw, cw * h / w)
    } else {
        (ch * w / h, ch)
    };
    let (dw, dh) = (
        i64::try_from(dw).map_err(|e| e.to_string())?,
        i64::try_from(dh).map_err(|e| e.to_string())?,
    );
    let (ox, oy) = (cx0 + (cx1 - cx0 - dw) / 2, cy0 + (cy1 - cy0 - dh) / 2);
    let rect = Bounds::new(Pt2::new(Nm(ox), Nm(oy)), Pt2::new(Nm(ox + dw), Nm(oy + dh)))
        .map_err(|e| e.to_string())?;
    Ok(Primitive::Image(ImagePrim {
        rect,
        asset,
        extra: Default::default(),
    }))
}

/// QET's `valign` and `align` as an EW alignment (QET's defaults: centre, left).
fn align(valign: &str, align: &str) -> Align {
    match (valign, align) {
        ("top", "center") => Align::TopCentre,
        ("top", "right") => Align::TopRight,
        ("top", _) => Align::TopLeft,
        ("bottom", "center") => Align::BottomCentre,
        ("bottom", "right") => Align::BottomRight,
        ("bottom", _) => Align::BottomLeft,
        (_, "center") => Align::Centre,
        (_, "right") => Align::MiddleRight,
        _ => Align::MiddleLeft,
    }
}
