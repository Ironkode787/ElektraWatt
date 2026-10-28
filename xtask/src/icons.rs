// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gen-icons`: the icon table of `ew-ui` (WP-S2.17 part 6, D9; doc 16a §4.11).
//!
//! Reads the vendored Tabler Icons outline SVGs (`assets/icons/tabler/`, MIT) and the project's own glyphs
//! (`assets/icons/ew/`, GPL-3.0-or-later, keyed `ew-<stem>`), parses every `d` attribute with
//! `kurbo::BezPath::from_svg` (arcs become cubics, nothing is flattened) and writes
//! `crates/ew-ui/src/icons/generated.rs`: one `IconId` variant per icon in `snake_case`, the `&'static str` keys,
//! and the path segments as static data, sorted by key. Coordinates are rounded to 1/1000 of the 24-unit grid so
//! that the bytes do not depend on the platform's trigonometry.
//!
//! The generator accepts only `svg` root elements and `path` children (comments and an XML declaration are
//! skipped). It refuses any other element and any `fill` other than `none`. Tabler's invisible bounding box
//! (`<path stroke="none" d="M0 0h24v24H0z" fill="none"/>`) is the one path it drops. `--check` changes nothing
//! and fails when the committed file differs, like `docs-gen --check`.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result, bail};
use kurbo::{BezPath, PathEl};

/// The vendored Tabler Icons 3.50.0 outline SVGs.
const TABLER_DIR: &str = "assets/icons/tabler";
/// The project's own glyphs.
const EW_DIR: &str = "assets/icons/ew";
/// The generated table, relative to the workspace root.
pub(crate) const OUTPUT: &str = "crates/ew-ui/src/icons/generated.rs";
/// Tabler's invisible 24 × 24 bounding box, dropped.
const BOUNDING_BOX: &str = "M0 0h24v24H0z";

/// One icon: its key and its parsed path.
struct Icon {
    key: String,
    path: BezPath,
}

/// An element of an SVG file: its name and attributes.
struct Element {
    name: String,
    attrs: Vec<(String, String)>,
}

impl Element {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// The opening and empty element tags of `text` in order (closing tags, comments and declarations skipped).
fn elements(text: &str) -> Result<Vec<Element>> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        rest = &rest[start..];
        if let Some(after) = rest.strip_prefix("<!--") {
            let end = after.find("-->").context("unterminated comment")?;
            rest = &after[end + 3..];
            continue;
        }
        let end = rest.find('>').context("unterminated tag")?;
        let tag = &rest[1..end];
        rest = &rest[end + 1..];
        if tag.starts_with('/') || tag.starts_with('?') {
            continue;
        }
        if tag.starts_with('!') {
            bail!("unsupported markup `<{tag}>`");
        }
        let tag = tag.trim_end_matches('/').trim();
        let name_end = tag.find(char::is_whitespace).unwrap_or(tag.len());
        let name = tag[..name_end].to_owned();
        let mut attrs = Vec::new();
        let mut a = tag[name_end..].trim_start();
        while !a.is_empty() {
            let eq = a
                .find('=')
                .with_context(|| format!("attribute without value in <{name}>"))?;
            let key = a[..eq].trim().to_owned();
            let v = a[eq + 1..].trim_start();
            let quote = v.chars().next().context("missing attribute value")?;
            if quote != '"' && quote != '\'' {
                bail!("unquoted attribute `{key}` in <{name}>");
            }
            let close = v[1..].find(quote).context("unterminated attribute value")?;
            attrs.push((key, v[1..=close].to_owned()));
            a = v[close + 2..].trim_start();
        }
        out.push(Element { name, attrs });
    }
    Ok(out)
}

/// Parses one SVG file into a path, enforcing the stroke-only rules.
fn parse_svg(text: &str) -> Result<BezPath> {
    let elements = elements(text)?;
    let mut path = BezPath::new();
    for (i, e) in elements.iter().enumerate() {
        if let Some(fill) = e.attr("fill")
            && fill != "none"
        {
            bail!("<{}> has fill=\"{fill}\"; icons are stroke-only", e.name);
        }
        if e.attr("style").is_some() {
            bail!("<{}> has a style attribute; icons are plain stroke paths", e.name);
        }
        match (i, e.name.as_str()) {
            (0, "svg") => {
                if e.attr("viewBox") != Some("0 0 24 24") {
                    bail!("the viewBox must be `0 0 24 24`");
                }
            }
            (_, "path") if i > 0 => {
                let d = e.attr("d").context("<path> without d")?;
                if e.attr("stroke") == Some("none") {
                    if d == BOUNDING_BOX {
                        continue;
                    }
                    bail!("<path> with stroke=\"none\" other than the bounding box");
                }
                let parsed = BezPath::from_svg(d).map_err(|err| anyhow::anyhow!("d=\"{d}\": {err}"))?;
                path.extend(parsed.elements().iter().copied());
            }
            (_, other) => bail!("element <{other}> is not allowed; icons hold only <path> elements"),
        }
    }
    if path.elements().is_empty() {
        bail!("no path");
    }
    Ok(path)
}

/// The icons of one directory, keyed `<prefix><stem>`.
fn read_dir(root: &Path, dir: &str, prefix: &str, icons: &mut Vec<Icon>) -> Result<()> {
    let full = root.join(dir);
    let entries = std::fs::read_dir(&full).with_context(|| format!("reading {}", full.display()))?;
    for entry in entries {
        let p = entry?.path();
        let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if p.extension().and_then(|e| e.to_str()) != Some("svg") {
            continue;
        }
        if !stem
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            bail!("{dir}/{stem}.svg: the name must be lower-case kebab-case");
        }
        let text = std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
        let path = parse_svg(&text).with_context(|| format!("{dir}/{stem}.svg"))?;
        icons.push(Icon {
            key: format!("{prefix}{stem}"),
            path,
        });
    }
    Ok(())
}

/// A coordinate rounded to 1/1000 and printed as an `f32` literal.
fn num(v: f64) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    let r = if r == 0.0 { 0.0 } else { r };
    let s = format!("{r}");
    if s.contains('.') { s } else { format!("{s}.0") }
}

/// The text of the generated table for `icons` (sorted here).
fn render(mut icons: Vec<Icon>) -> Result<String> {
    icons.sort_by(|a, b| a.key.cmp(&b.key));
    for w in icons.windows(2) {
        if w[0].key == w[1].key {
            bail!("duplicate icon key `{}`", w[0].key);
        }
    }
    let mut s = String::new();
    // The SPDX tags are split so that `reuse lint` does not read them as this file's own.
    s.push_str(concat!(
        "// SPDX",
        "-FileCopyrightText: Copyright (c) 2020-2026 Paweł Kuna\n",
        "// SPDX",
        "-FileCopyrightText: 2026 The ElektraWatt contributors\n",
        "// SPDX",
        "-License-Identifier: GPL-3.0-or-later\n",
    ));
    s.push_str(
        "\n\
         // Generated by `cargo xtask gen-icons` from assets/icons/tabler/ (Tabler Icons 3.50.0, MIT) and\n\
         // assets/icons/ew/ (GPL-3.0-or-later); do not edit. `cargo xtask gen-icons --check` fails when it is stale.\n\
         \n\
         #[allow(unused_imports, reason = \"the table uses the segment kinds its SVGs need\")]\n\
         use super::Seg::{self, C, L, M, Q, Z};\n\n",
    );
    let n = icons.len();
    writeln!(
        s,
        "/// The number of icons in the table.\npub const COUNT: usize = {n};\n"
    )?;
    s.push_str(
        "/// One icon of the table, named by its key in `snake_case` (`ew_` for the project's own glyphs).\n\
         #[allow(non_camel_case_types, missing_docs)]\n\
         #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]\n\
         pub enum IconId {\n",
    );
    for icon in &icons {
        writeln!(s, "    {},", icon.key.replace('-', "_"))?;
    }
    s.push_str(
        "}\n\nimpl IconId {\n    /// Every icon, in key order.\n    pub const ALL: [IconId; COUNT] = [\n",
    );
    for icon in &icons {
        writeln!(s, "        IconId::{},", icon.key.replace('-', "_"))?;
    }
    s.push_str(
        "    ];\n}\n\n/// The keys, in the order of [`IconId::ALL`].\npub const KEYS: [&str; COUNT] = [\n",
    );
    for icon in &icons {
        writeln!(s, "    {:?},", icon.key)?;
    }
    s.push_str("];\n\n/// The path segments on the 24-unit grid, in the order of [`IconId::ALL`].\npub(super) static PATHS: [&[Seg]; COUNT] = [\n");
    for icon in &icons {
        writeln!(s, "    // {}", icon.key)?;
        s.push_str("    &[\n");
        for el in icon.path.elements() {
            let line = match *el {
                PathEl::MoveTo(p) => format!("M({}, {})", num(p.x), num(p.y)),
                PathEl::LineTo(p) => format!("L({}, {})", num(p.x), num(p.y)),
                PathEl::QuadTo(a, p) => {
                    format!("Q({}, {}, {}, {})", num(a.x), num(a.y), num(p.x), num(p.y))
                }
                PathEl::CurveTo(a, b, p) => format!(
                    "C({}, {}, {}, {}, {}, {})",
                    num(a.x),
                    num(a.y),
                    num(b.x),
                    num(b.y),
                    num(p.x),
                    num(p.y)
                ),
                PathEl::ClosePath => "Z".to_owned(),
            };
            writeln!(s, "        {line},")?;
        }
        s.push_str("    ],\n");
    }
    s.push_str("];\n");
    Ok(s)
}

/// The wanted text of the generated table for the repository at `root`.
pub(crate) fn generate(root: &Path) -> Result<String> {
    let mut icons = Vec::new();
    read_dir(root, TABLER_DIR, "", &mut icons)?;
    read_dir(root, EW_DIR, "ew-", &mut icons)?;
    render(icons)
}

/// The violations of `gen-icons --check` for the repository at `root`.
pub(crate) fn violations(root: &Path) -> Result<Vec<String>> {
    let wanted = generate(root)?;
    let have = std::fs::read_to_string(root.join(OUTPUT))
        .ok()
        .map(|t| t.replace("\r\n", "\n"));
    Ok(if have.as_deref() == Some(wanted.as_str()) {
        Vec::new()
    } else {
        vec![format!("{OUTPUT} differs; run `cargo xtask gen-icons`")]
    })
}

/// `gen-icons [--check]` for the repository at `root`.
pub(crate) fn run(root: &Path, check: bool) -> Result<bool> {
    if check {
        let v = violations(root)?;
        if v.is_empty() {
            println!("gen-icons: ok");
            return Ok(true);
        }
        eprintln!("gen-icons: {} problem(s)", v.len());
        for line in &v {
            eprintln!("  - {line}");
        }
        return Ok(false);
    }
    let wanted = generate(root)?;
    let path = root.join(OUTPUT);
    std::fs::write(&path, wanted).with_context(|| format!("writing {}", path.display()))?;
    println!("gen-icons: wrote {OUTPUT}");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str =
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor">"#;

    fn svg(body: &str) -> String {
        format!("{HEAD}{body}</svg>")
    }

    fn icon(key: &str, body: &str) -> Icon {
        Icon {
            key: key.to_owned(),
            path: parse_svg(&svg(body)).unwrap(),
        }
    }

    #[test]
    fn a_path_icon_parses_and_drops_the_bounding_box() {
        let p = parse_svg(&svg(
            r#"<path stroke="none" d="M0 0h24v24H0z" fill="none"/><path d="M4 4l16 16" />"#,
        ))
        .unwrap();
        assert_eq!(p.elements().len(), 2);
    }

    #[test]
    fn a_non_path_element_is_refused() {
        let err = parse_svg(&svg(r#"<circle cx="12" cy="12" r="3"/>"#)).unwrap_err();
        assert!(format!("{err:#}").contains("<circle> is not allowed"), "{err:#}");
        let err = parse_svg(&svg(r#"<g><path d="M1 1h2"/></g>"#)).unwrap_err();
        assert!(format!("{err:#}").contains("<g> is not allowed"), "{err:#}");
    }

    #[test]
    fn a_fill_is_refused() {
        let err = parse_svg(&svg(r#"<path d="M4 4h4v4z" fill="currentColor"/>"#)).unwrap_err();
        assert!(format!("{err:#}").contains("stroke-only"), "{err:#}");
        let err = parse_svg(r#"<svg viewBox="0 0 24 24" fill="black"><path d="M1 1h2"/></svg>"#).unwrap_err();
        assert!(format!("{err:#}").contains("stroke-only"), "{err:#}");
    }

    #[test]
    fn arcs_become_cubics_and_rounding_is_fixed() {
        let p = parse_svg(&svg(r#"<path d="M10 10a2 2 0 1 0 4 0"/>"#)).unwrap();
        assert!(p.elements().iter().any(|e| matches!(e, PathEl::CurveTo(..))));
        assert_eq!(num(1.0), "1.0");
        assert_eq!(num(-0.0001), "0.0");
        assert_eq!(num(2.33333), "2.333");
    }

    /// IC1: the output does not depend on the input order and is the same bytes twice.
    #[test]
    fn the_output_is_deterministic_and_sorted() {
        let a = render(vec![
            icon("b-two", r#"<path d="M1 1h2"/>"#),
            icon("a-one", r#"<path d="M2 2v2"/>"#),
        ])
        .unwrap();
        let b = render(vec![
            icon("a-one", r#"<path d="M2 2v2"/>"#),
            icon("b-two", r#"<path d="M1 1h2"/>"#),
        ])
        .unwrap();
        assert_eq!(a, b);
        assert!(a.find("a_one,").unwrap() < a.find("b_two,").unwrap());
        assert!(
            render(vec![
                icon("x", r#"<path d="M1 1h2"/>"#),
                icon("x", r#"<path d="M1 1h2"/>"#)
            ])
            .is_err()
        );
    }

    fn root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    /// IC1: generating twice gives the same bytes, and the committed table is current.
    #[test]
    fn the_committed_table_is_current() {
        let root = root();
        assert_eq!(generate(&root).unwrap(), generate(&root).unwrap());
        assert_eq!(violations(&root).unwrap(), Vec::<String>::new());
    }

    /// IC1: `--check` fails on a stale table (a copy of the inputs with one icon changed).
    #[test]
    fn the_check_fails_on_a_stale_table() {
        let dir = std::env::temp_dir().join(format!("ew-gen-icons-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for sub in [TABLER_DIR, EW_DIR, "crates/ew-ui/src/icons"] {
            std::fs::create_dir_all(dir.join(sub)).unwrap();
        }
        std::fs::write(dir.join(TABLER_DIR).join("a.svg"), svg(r#"<path d="M1 1h2"/>"#)).unwrap();
        std::fs::write(dir.join(EW_DIR).join("b.svg"), svg(r#"<path d="M2 2v2"/>"#)).unwrap();
        assert!(run(&dir, false).unwrap());
        assert!(violations(&dir).unwrap().is_empty());
        assert!(run(&dir, true).unwrap());
        std::fs::write(dir.join(TABLER_DIR).join("a.svg"), svg(r#"<path d="M1 1h3"/>"#)).unwrap();
        assert_eq!(violations(&dir).unwrap().len(), 1);
        assert!(!run(&dir, true).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
