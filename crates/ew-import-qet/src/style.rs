// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The QET style mini-language (doc 14 §4.3): the `style` attribute of element primitives, such as
//! `line-style:dashed;line-weight:thin;filling:none;color:black`.
//!
//! # Syntax
//!
//! Segments are separated by `;`; each segment is `key:value`, both trimmed of ASCII white space. Empty
//! segments are ignored. A segment without `:` is unsupported as `(segment, "")`. When a key repeats, the
//! last segment wins. Keys and values are matched exactly (QET writes them in lower case).
//!
//! # Mapping
//!
//! | Key | Value | Result |
//! |---|---|---|
//! | `line-style` | `normal` (default) | [`Dash::Solid`] |
//! | | `dashed` | [`Dash::Dash42`] |
//! | | `dotted` | [`Dash::Dot12`] |
//! | | `dashdotted` | [`Dash::DashDot`] |
//! | `line-weight` | `none` | no stroke (`stroke = None`) |
//! | | `thin` (0.5 px) | [`Pen::P013`] (0.13 mm) |
//! | | `normal` (1 px, default) | [`Pen::P026`] (0.26 mm) |
//! | | `hight` (2 px; historical spelling, kept by QET) | [`Pen::P052`] (0.52 mm) |
//! | | `eleve` (5 px; historical spelling) | [`Pen::P130`] (1.30 mm) |
//! | `color` | `black` (default) | [`Colour::ByLayer`] |
//! | | a name of the colour table below, or `#rrggbb` | [`Colour::Rgb`] |
//! | `filling` | `none` (default) | no fill (`fill = None`) |
//! | | `black` | [`Colour::ByLayer`] |
//! | | a name of the colour table below, or `#rrggbb` | [`Colour::Rgb`] |
//! | | `hor`, `ver`, `bdiag`, `fdiag` (hatch patterns) | unsupported; no fill |
//! | `antialias` | any | ignored silently: rendering quality is the renderer's choice, nothing is lost |
//!
//! The pen widths follow the house widths of doc 20 §17.4, whose "native px" column gives 0.5, 1.0, 2.0
//! and 5.0 px for the 0.13, 0.26, 0.52 and 1.30 mm pens.
//!
//! **Colour table** (Qt's named colours for the Qt names, SVG colours for the others):
//!
//! | Name | Rgb |
//! |---|---|
//! | `white` | `#ffffff` |
//! | `red` | `#ff0000` |
//! | `green` | `#00ff00` |
//! | `blue` | `#0000ff` |
//! | `gray` | `#a0a0a4` |
//! | `lightgray` | `#c0c0c0` |
//! | `brown` | `#a52a2a` |
//! | `yellow` | `#ffff00` |
//! | `cyan` | `#00ffff` |
//! | `magenta` | `#ff00ff` |
//! | `orange` | `#ffa500` |
//! | `purple` | `#800080` |
//!
//! `#rrggbb` accepts hex digits in either case and stores them in lower case.
//!
//! **Unsupported** keys and values keep the default of their key (solid, 0.26 mm, by layer, no fill) and are
//! listed in [`StyleSpec::unsupported`] in source order; the caller reports each one (EW-IMP-004) and keeps
//! it in the primitive's `extra` under `qet.style.<key>`. The table is extensible: add rows, never change
//! one.

use std::collections::BTreeMap;

use ew_schema::{Colour, Dash, Pen, Rgb, Stroke};

/// The parsed `style` attribute of one primitive.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StyleSpec {
    /// The stroke; `None` for `line-weight:none`. Its `extra` is empty.
    pub stroke: Option<Stroke>,
    /// The fill; `None` for `filling:none`, an absent `filling` or an unsupported one.
    pub fill: Option<Colour>,
    /// Unsupported `(key, value)` pairs in source order (a repeated key is listed once per occurrence).
    pub unsupported: Vec<(String, String)>,
}

impl Default for StyleSpec {
    /// The style of an absent or empty attribute: a solid 0.26 mm stroke by layer, no fill.
    fn default() -> StyleSpec {
        StyleSpec {
            stroke: Some(Stroke {
                pen: Pen::P026,
                dash: Dash::Solid,
                colour: Colour::ByLayer,
                extra: BTreeMap::new(),
            }),
            fill: None,
            unsupported: vec![],
        }
    }
}

/// Parses a `style` attribute by the table of the module documentation. Never fails: anything it cannot map
/// is listed in [`StyleSpec::unsupported`].
pub fn parse_style(style: &str) -> StyleSpec {
    let mut pen = Pen::P026;
    let mut dash = Dash::Solid;
    let mut colour = Colour::ByLayer;
    let mut stroked = true;
    let mut fill: Option<Colour> = None;
    let mut unsupported: Vec<(String, String)> = Vec::new();
    let trim = |s: &str| s.trim_matches(|c: char| c.is_ascii_whitespace()).to_string();

    for raw in style.split(';') {
        let segment = trim(raw);
        if segment.is_empty() {
            continue;
        }
        let Some((raw_key, raw_value)) = segment.split_once(':') else {
            unsupported.push((segment.clone(), String::new()));
            continue;
        };
        let key = trim(raw_key);
        let value = trim(raw_value);
        match key.as_str() {
            "line-style" => match value.as_str() {
                "normal" => dash = Dash::Solid,
                "dashed" => dash = Dash::Dash42,
                "dotted" => dash = Dash::Dot12,
                "dashdotted" => dash = Dash::DashDot,
                _ => {
                    dash = Dash::Solid;
                    unsupported.push((key.clone(), value.clone()));
                }
            },
            "line-weight" => match value.as_str() {
                "none" => stroked = false,
                "thin" => {
                    pen = Pen::P013;
                    stroked = true;
                }
                "normal" => {
                    pen = Pen::P026;
                    stroked = true;
                }
                "hight" => {
                    pen = Pen::P052;
                    stroked = true;
                }
                "eleve" => {
                    pen = Pen::P130;
                    stroked = true;
                }
                _ => {
                    pen = Pen::P026;
                    stroked = true;
                    unsupported.push((key.clone(), value.clone()));
                }
            },
            "color" => match parse_colour(&value) {
                Some(c) => colour = c,
                None => {
                    colour = Colour::ByLayer;
                    unsupported.push((key.clone(), value.clone()));
                }
            },
            "filling" => match value.as_str() {
                "none" => fill = None,
                "hor" | "ver" | "bdiag" | "fdiag" => {
                    fill = None;
                    unsupported.push((key.clone(), value.clone()));
                }
                _ => match parse_colour(&value) {
                    Some(c) => fill = Some(c),
                    None => {
                        fill = None;
                        unsupported.push((key.clone(), value.clone()));
                    }
                },
            },
            "antialias" => {}
            _ => unsupported.push((key.clone(), value.clone())),
        }
    }

    let stroke = if stroked {
        Some(Stroke {
            pen,
            dash,
            colour,
            extra: BTreeMap::new(),
        })
    } else {
        None
    };
    StyleSpec {
        stroke,
        fill,
        unsupported,
    }
}

/// The colour of a `color` or `filling` value: `black` → [`Colour::ByLayer`], a name of the colour table or
/// `#rrggbb` → [`Colour::Rgb`]; `None` for anything else (including `none`, which the callers handle first).
/// Text primitives use it for their `color` attribute.
pub fn parse_colour(value: &str) -> Option<Colour> {
    const NAMED: [(&str, [u8; 3]); 12] = [
        ("white", [0xff, 0xff, 0xff]),
        ("red", [0xff, 0x00, 0x00]),
        ("green", [0x00, 0xff, 0x00]),
        ("blue", [0x00, 0x00, 0xff]),
        ("gray", [0xa0, 0xa0, 0xa4]),
        ("lightgray", [0xc0, 0xc0, 0xc0]),
        ("brown", [0xa5, 0x2a, 0x2a]),
        ("yellow", [0xff, 0xff, 0x00]),
        ("cyan", [0x00, 0xff, 0xff]),
        ("magenta", [0xff, 0x00, 0xff]),
        ("orange", [0xff, 0xa5, 0x00]),
        ("purple", [0x80, 0x00, 0x80]),
    ];
    if value == "black" {
        return Some(Colour::ByLayer);
    }
    if let Some((_, rgb)) = NAMED.iter().find(|(name, _)| *name == value) {
        return Some(Colour::Rgb(Rgb(*rgb)));
    }
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some(Colour::Rgb(Rgb([byte(0)?, byte(2)?, byte(4)?])))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(r: u8, g: u8, b: u8) -> Colour {
        Colour::Rgb(Rgb([r, g, b]))
    }

    fn pair(key: &str, value: &str) -> (String, String) {
        (key.to_string(), value.to_string())
    }

    fn stroke_of(spec: &StyleSpec) -> &Stroke {
        spec.stroke.as_ref().expect("a stroke")
    }

    const TABLE: [(&str, [u8; 3]); 12] = [
        ("white", [0xff, 0xff, 0xff]),
        ("red", [0xff, 0x00, 0x00]),
        ("green", [0x00, 0xff, 0x00]),
        ("blue", [0x00, 0x00, 0xff]),
        ("gray", [0xa0, 0xa0, 0xa4]),
        ("lightgray", [0xc0, 0xc0, 0xc0]),
        ("brown", [0xa5, 0x2a, 0x2a]),
        ("yellow", [0xff, 0xff, 0x00]),
        ("cyan", [0x00, 0xff, 0xff]),
        ("magenta", [0xff, 0x00, 0xff]),
        ("orange", [0xff, 0xa5, 0x00]),
        ("purple", [0x80, 0x00, 0x80]),
    ];

    #[test]
    fn defaults() {
        assert_eq!(parse_style(""), StyleSpec::default());
        assert_eq!(parse_style(" ; ;"), StyleSpec::default());
    }

    #[test]
    fn line_styles() {
        for (value, dash) in [
            ("normal", Dash::Solid),
            ("dashed", Dash::Dash42),
            ("dotted", Dash::Dot12),
            ("dashdotted", Dash::DashDot),
        ] {
            let spec = parse_style(&format!("line-style:{value}"));
            assert_eq!(stroke_of(&spec).dash, dash, "{value}");
            assert!(spec.unsupported.is_empty());
        }
        let spec = parse_style("line-style:wavy");
        assert_eq!(stroke_of(&spec).dash, Dash::Solid);
        assert_eq!(spec.unsupported, vec![pair("line-style", "wavy")]);
    }

    #[test]
    fn line_weights() {
        assert_eq!(parse_style("line-weight:none").stroke, None);
        for (value, pen) in [
            ("thin", Pen::P013),
            ("normal", Pen::P026),
            ("hight", Pen::P052),
            ("eleve", Pen::P130),
        ] {
            let spec = parse_style(&format!("line-weight:{value}"));
            assert_eq!(stroke_of(&spec).pen, pen, "{value}");
            assert!(spec.unsupported.is_empty());
        }
        let spec = parse_style("line-weight:none;line-weight:thin");
        assert_eq!(stroke_of(&spec).pen, Pen::P013);
        let spec = parse_style("line-weight:heavy");
        assert_eq!(stroke_of(&spec).pen, Pen::P026);
        assert_eq!(spec.unsupported, vec![pair("line-weight", "heavy")]);
    }

    #[test]
    fn colours() {
        assert_eq!(stroke_of(&parse_style("color:black")).colour, Colour::ByLayer);
        for (name, [r, g, b]) in TABLE {
            let spec = parse_style(&format!("color:{name}"));
            assert_eq!(stroke_of(&spec).colour, rgb(r, g, b), "color:{name}");
            assert!(spec.unsupported.is_empty());
            let spec = parse_style(&format!("filling:{name}"));
            assert_eq!(spec.fill, Some(rgb(r, g, b)), "filling:{name}");
            assert!(spec.unsupported.is_empty());
        }
        let spec = parse_style("color:#A0b0C0");
        assert_eq!(stroke_of(&spec).colour, rgb(0xa0, 0xb0, 0xc0));
        let spec = parse_style("color:pink");
        assert_eq!(stroke_of(&spec).colour, Colour::ByLayer);
        assert_eq!(spec.unsupported, vec![pair("color", "pink")]);
    }

    #[test]
    fn fills() {
        let spec = parse_style("filling:none");
        assert_eq!(spec.fill, None);
        assert!(spec.unsupported.is_empty());
        assert_eq!(parse_style("filling:black").fill, Some(Colour::ByLayer));
        for hatch in ["hor", "ver", "bdiag", "fdiag"] {
            let spec = parse_style(&format!("filling:{hatch}"));
            assert_eq!(spec.fill, None);
            assert_eq!(spec.unsupported, vec![pair("filling", hatch)]);
        }
    }

    #[test]
    fn typical_qet_style() {
        let spec = parse_style("line-style:dashed;line-weight:thin;filling:white;color:red");
        let stroke = stroke_of(&spec);
        assert_eq!(stroke.pen, Pen::P013);
        assert_eq!(stroke.dash, Dash::Dash42);
        assert_eq!(stroke.colour, rgb(0xff, 0x00, 0x00));
        assert_eq!(spec.fill, Some(rgb(0xff, 0xff, 0xff)));
        assert!(spec.unsupported.is_empty());
    }

    #[test]
    fn antialias_and_unknown_keys() {
        let spec = parse_style("antialias:true;font:x;bogus");
        assert_eq!(spec.stroke, StyleSpec::default().stroke);
        assert_eq!(spec.fill, None);
        assert_eq!(spec.unsupported, vec![pair("font", "x"), pair("bogus", "")]);
    }

    #[test]
    fn whitespace() {
        let spec = parse_style(" line-style : dotted ; color: blue ");
        let stroke = stroke_of(&spec);
        assert_eq!(stroke.dash, Dash::Dot12);
        assert_eq!(stroke.colour, rgb(0x00, 0x00, 0xff));
        assert!(spec.unsupported.is_empty());
    }
}
