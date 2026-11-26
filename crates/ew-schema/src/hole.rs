// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A hole in a surface of a cabinet (doc 12 §2.5): a part's drilling pattern (`MechData.holes`) and a hole by hand
//! (`SurfaceFeature.hole`) name it the same way, and [`Hole::parse`] is the one reader of its text form for the part
//! editor's cells, the Add hole form and the command line. The size a named hole is made at is a table of
//! `ew_derive::layout` (doc 12-3d §10.2), never stored here.
//!
//! ```
//! use ew_schema::{Cutout, Hole, HoleKind, Screw};
//!
//! assert_eq!(Hole::parse(HoleKind::Cutout, "22 mm"), Ok(Hole::Cutout(Cutout::D22)));
//! assert_eq!(Hole::parse(HoleKind::Fixing, "m5"), Ok(Hole::Fixing(Screw::M5)));
//! assert_eq!(serde_json::to_string(&Hole::Fixing(Screw::M5)).unwrap(), r#"{"fixing":"m5"}"#);
//! assert_eq!(Hole::Cutout(Cutout::D22).to_string(), "22 mm");
//! ```

use std::fmt;

use ew_units::{Dec3, Nm};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::proxy::NmS;
use crate::track::open_enum;

/// A hole (doc 12 §2.5), tagged by its kind: `{"round": {"d": …}}`, `{"fixing": "m5"}`, `{"cutout": "d22"}`,
/// `{"gland": "m20"}` or `{"rect": {"w": …, "h": …, "r": …}}`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "Hole")]
pub enum Hole {
    /// A plain hole of a diameter.
    Round {
        /// The diameter.
        #[schemars(with = "NmS")]
        d: Nm,
    },
    /// A hole for a screw that fixes the part: the shop's setting and the surface decide how it is made.
    Fixing(Screw),
    /// The round cut-out of an operator of a class.
    Cutout(Cutout),
    /// The hole of a metric cable gland.
    Gland(Gland),
    /// A rectangular cut-out, with a corner radius.
    Rect {
        /// The width.
        #[schemars(with = "NmS")]
        w: Nm,
        /// The height.
        #[schemars(with = "NmS")]
        h: Nm,
        /// The corner radius; absent: square corners.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(with = "Option<NmS>")]
        r: Option<Nm>,
    },
    /// A form of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The screw of a fixing hole.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "Screw")]
pub enum Screw {
    /// M3.
    M3,
    /// M4.
    M4,
    /// M5.
    M5,
    /// M6.
    M6,
    /// M8.
    M8,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The class of a round cut-out (an operator's mounting hole).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "Cutout")]
pub enum Cutout {
    /// 16 mm.
    D16,
    /// 22 mm.
    D22,
    /// 30 mm.
    D30,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The thread of a metric cable gland.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "Gland")]
pub enum Gland {
    /// M12.
    M12,
    /// M16.
    M16,
    /// M20.
    M20,
    /// M25.
    M25,
    /// M32.
    M32,
    /// M40.
    M40,
    /// M50.
    M50,
    /// M63.
    M63,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

open_enum!(Hole, Screw, Cutout, Gland);

/// The kind of a [`Hole`], as a form asks for it before its size.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum HoleKind {
    /// [`Hole::Round`].
    Round,
    /// [`Hole::Fixing`].
    Fixing,
    /// [`Hole::Cutout`].
    Cutout,
    /// [`Hole::Gland`].
    Gland,
    /// [`Hole::Rect`].
    Rect,
}

impl HoleKind {
    /// Every kind, in the order a form offers them.
    pub const ALL: [HoleKind; 5] = [
        HoleKind::Round,
        HoleKind::Fixing,
        HoleKind::Cutout,
        HoleKind::Gland,
        HoleKind::Rect,
    ];

    /// The kind's word, the member that names it in a [`Hole`]'s stored form (`round`, `fixing`, `cutout`, `gland`,
    /// `rect`).
    pub fn word(self) -> &'static str {
        match self {
            HoleKind::Round => "round",
            HoleKind::Fixing => "fixing",
            HoleKind::Cutout => "cutout",
            HoleKind::Gland => "gland",
            HoleKind::Rect => "rect",
        }
    }

    /// The kind of the word `word` ([`HoleKind::word`]).
    pub fn of_word(word: &str) -> Option<HoleKind> {
        HoleKind::ALL.into_iter().find(|k| k.word() == word)
    }
}

/// Why a hole's text does not parse ([`Hole::parse`]): a typed fact the window and the command line put in words.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum HoleParseError {
    /// No number where a length was asked for.
    NoNumber,
    /// The text names no screw size of [`Screw`].
    NotAScrew {
        /// The text as typed.
        text: String,
    },
    /// The text names no cut-out of [`Cutout`].
    NotACutout {
        /// The text as typed.
        text: String,
    },
    /// The text names no gland thread of [`Gland`].
    NotAGland {
        /// The text as typed.
        text: String,
    },
    /// The text is no rectangle `w × h`, with `R r` after it.
    NotRect {
        /// The text as typed.
        text: String,
    },
    /// A length that is zero or less.
    NotPositive,
}

impl fmt::Display for HoleParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HoleParseError::NoNumber => f.write_str("no number"),
            HoleParseError::NotAScrew { text } => {
                write!(f, "`{text}` is no screw size: type M3, M4, M5, M6 or M8")
            }
            HoleParseError::NotACutout { text } => write!(f, "`{text}` is no cut-out: type 16, 22 or 30 mm"),
            HoleParseError::NotAGland { text } => {
                write!(
                    f,
                    "`{text}` is no gland thread: type M12, M16, M20, M25, M32, M40, M50 or M63"
                )
            }
            HoleParseError::NotRect { text } => {
                write!(f, "`{text}` is no rectangle: type width × height, R radius")
            }
            HoleParseError::NotPositive => f.write_str("a length must be greater than 0"),
        }
    }
}

impl std::error::Error for HoleParseError {}

impl Screw {
    /// Every screw size, smallest first.
    pub const ALL: [Screw; 5] = [Screw::M3, Screw::M4, Screw::M5, Screw::M6, Screw::M8];

    /// The nominal diameter in millimetres; `None` for a value of a newer format.
    pub fn mm(&self) -> Option<u32> {
        Some(match self {
            Screw::M3 => 3,
            Screw::M4 => 4,
            Screw::M5 => 5,
            Screw::M6 => 6,
            Screw::M8 => 8,
            Screw::Unknown(_) => return None,
        })
    }
}

impl Cutout {
    /// Every cut-out class, smallest first.
    pub const ALL: [Cutout; 3] = [Cutout::D16, Cutout::D22, Cutout::D30];

    /// The class in millimetres; `None` for a value of a newer format.
    pub fn mm(&self) -> Option<u32> {
        Some(match self {
            Cutout::D16 => 16,
            Cutout::D22 => 22,
            Cutout::D30 => 30,
            Cutout::Unknown(_) => return None,
        })
    }
}

impl Gland {
    /// Every gland thread, smallest first.
    pub const ALL: [Gland; 8] = [
        Gland::M12,
        Gland::M16,
        Gland::M20,
        Gland::M25,
        Gland::M32,
        Gland::M40,
        Gland::M50,
        Gland::M63,
    ];

    /// The thread's nominal diameter in millimetres; `None` for a value of a newer format.
    pub fn mm(&self) -> Option<u32> {
        Some(match self {
            Gland::M12 => 12,
            Gland::M16 => 16,
            Gland::M20 => 20,
            Gland::M25 => 25,
            Gland::M32 => 32,
            Gland::M40 => 40,
            Gland::M50 => 50,
            Gland::M63 => 63,
            Gland::Unknown(_) => return None,
        })
    }
}

impl Hole {
    /// The kind; `None` for a form of a newer format.
    pub fn kind(&self) -> Option<HoleKind> {
        Some(match self {
            Hole::Round { .. } => HoleKind::Round,
            Hole::Fixing(_) => HoleKind::Fixing,
            Hole::Cutout(_) => HoleKind::Cutout,
            Hole::Gland(_) => HoleKind::Gland,
            Hole::Rect { .. } => HoleKind::Rect,
            Hole::Unknown(_) => return None,
        })
    }

    /// Reads the text form of a hole of kind `kind`: a round hole "Ø 4.5", "4.5" or "4.5 mm"; a fixing "M5"; a
    /// cut-out "22 mm", "22" or "Ø 22"; a gland "M20"; a rectangle "92 × 92 R 2", "92x92" or "92 x 92, R 2". Lengths
    /// are millimetres, with a point or a comma, to the micrometre.
    ///
    /// # Errors
    ///
    /// The [`HoleParseError`] that says what is wrong with the text.
    pub fn parse(kind: HoleKind, text: &str) -> Result<Hole, HoleParseError> {
        let t = text.trim();
        match kind {
            HoleKind::Round => Ok(Hole::Round { d: length(t)? }),
            HoleKind::Fixing => metric(t, &Screw::ALL, Screw::mm)
                .map(Hole::Fixing)
                .ok_or_else(|| HoleParseError::NotAScrew { text: t.to_owned() }),
            HoleKind::Gland => metric(t, &Gland::ALL, Gland::mm)
                .map(Hole::Gland)
                .ok_or_else(|| HoleParseError::NotAGland { text: t.to_owned() }),
            HoleKind::Cutout => {
                let not = || HoleParseError::NotACutout { text: t.to_owned() };
                let d = length(t).map_err(|_| not())?;
                Cutout::ALL
                    .into_iter()
                    .find(|c| c.mm().is_some_and(|mm| Nm::from_mm(i64::from(mm)) == d))
                    .map(Hole::Cutout)
                    .ok_or_else(not)
            }
            HoleKind::Rect => {
                let not = || HoleParseError::NotRect { text: t.to_owned() };
                let lower = t.to_lowercase();
                let (size, radius) = match lower.split_once('r') {
                    Some((size, radius)) => (size.trim().trim_end_matches(',').trim(), Some(radius)),
                    None => (lower.as_str(), None),
                };
                let (w, h) = size.split_once(['×', 'x', '*']).ok_or_else(not)?;
                let w = length(w).map_err(|e| {
                    if e == HoleParseError::NotPositive {
                        e
                    } else {
                        not()
                    }
                })?;
                let h = length(h).map_err(|e| {
                    if e == HoleParseError::NotPositive {
                        e
                    } else {
                        not()
                    }
                })?;
                let r = match radius {
                    Some(r) => Some(length(r).map_err(|e| {
                        if e == HoleParseError::NotPositive {
                            e
                        } else {
                            not()
                        }
                    })?),
                    None => None,
                };
                Ok(Hole::Rect { w, h, r })
            }
        }
    }
}

/// The text form [`Hole::parse`] reads back: "Ø 4.5", "M5", "22 mm", "M20", "92 × 92 R 2"; a form of a newer
/// format prints its stored JSON.
impl fmt::Display for Hole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let metric = |f: &mut fmt::Formatter<'_>, mm: Option<u32>, v: &dyn fmt::Debug| match mm {
            Some(mm) => write!(f, "M{mm}"),
            None => write!(f, "{v:?}"),
        };
        match self {
            Hole::Round { d } => write!(f, "Ø {}", mm_text(*d)),
            Hole::Fixing(s) => metric(f, s.mm(), s),
            Hole::Gland(g) => metric(f, g.mm(), g),
            Hole::Cutout(c) => match c.mm() {
                Some(mm) => write!(f, "{mm} mm"),
                None => write!(f, "{c:?}"),
            },
            Hole::Rect { w, h, r } => {
                write!(f, "{} × {}", mm_text(*w), mm_text(*h))?;
                match r {
                    Some(r) => write!(f, " R {}", mm_text(*r)),
                    None => Ok(()),
                }
            }
            Hole::Unknown(v) => write!(f, "{v}"),
        }
    }
}

/// A length in millimetres as the text form writes it: to the micrometre, without trailing zeros ("4.5", "22").
pub fn mm_text(n: Nm) -> String {
    let um = n.0 / 1_000;
    let (sign, um) = if um < 0 { ("-", -um) } else { ("", um) };
    let (whole, frac) = (um / 1_000, um % 1_000);
    if frac == 0 {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{}", format!("{frac:03}").trim_end_matches('0'))
    }
}

/// A positive length in millimetres: an optional "Ø" before it and "mm" after it.
fn length(text: &str) -> Result<Nm, HoleParseError> {
    let t = text.trim().trim_start_matches(['Ø', 'ø', '⌀']).trim();
    let t = t.strip_suffix("mm").unwrap_or(t).trim();
    let d = Dec3::parse_lenient(t).map_err(|_| HoleParseError::NoNumber)?;
    let nm = d.0.checked_mul(1_000).map(Nm).ok_or(HoleParseError::NoNumber)?;
    if nm <= Nm::ZERO {
        Err(HoleParseError::NotPositive)
    } else {
        Ok(nm)
    }
}

/// The member of `all` whose metric size is the text's "M<n>".
fn metric<T: Clone>(text: &str, all: &[T], mm: impl Fn(&T) -> Option<u32>) -> Option<T> {
    let n: u32 = text.strip_prefix(['M', 'm'])?.trim().parse().ok()?;
    all.iter().find(|v| mm(v) == Some(n)).cloned()
}
