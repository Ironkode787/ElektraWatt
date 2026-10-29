// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The built-in text-style table of doc 20 §17.5: style key → QET px → native pt → em height in nm.
//!
//! Native pt = QET px × 0.75. The em height in nm is `round(px × 793 750 / 3)`, because 1 px = 25.4/96 mm = 0.75 pt.
//! The printed pt of the doc (× 0.986282578) is a consequence of the frame's page-to-sheet transform, not a second
//! size. Project-level text styles (model data, FR-SYM-013) arrive with WP-S1.12 and will override or extend this
//! table. The golden table has no italic style.

use ew_units::{Grid, Nm};

use crate::{Family, TextStyle, Weight};

/// The style key used when a key is unknown or absent (`text.normal`, doc 20 §17.5).
pub const DEFAULT_STYLE_KEY: &str = "text.normal";

/// One row of a [`TextStyleTable`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct StyleDef {
    /// The style key (`text.normal`).
    pub key: &'static str,
    /// The size in QET pixels (1 px = 25.4/96 mm; native pt = px × 0.75).
    pub qet_px: u16,
    /// The weight.
    pub weight: Weight,
}

impl StyleDef {
    /// The native size in milli-points: `qet_px × 750`.
    pub const fn native_milli_pt(&self) -> u32 {
        self.qet_px as u32 * 750
    }

    /// The em height in nm: `round(qet_px × 793_750 / 3)`, ties toward zero ([`Grid::QET_1PX`]).
    pub fn em(&self) -> Nm {
        Grid::QET_1PX.at(i64::from(self.qet_px)).unwrap_or(Nm::ZERO)
    }

    /// The text style: default family, [`em`](StyleDef::em), this weight, upright.
    pub fn text_style(&self) -> TextStyle {
        TextStyle {
            family: Family::Default,
            size: self.em(),
            weight: self.weight,
            italic: false,
        }
    }
}

/// A table of text styles.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TextStyleTable {
    rows: &'static [StyleDef],
}

const ROWS: &[StyleDef] = &[
    StyleDef {
        key: "cover.title",
        qet_px: 48,
        weight: Weight::Regular,
    },
    StyleDef {
        key: "section.id",
        qet_px: 37,
        weight: Weight::Bold,
    },
    StyleDef {
        key: "section.subtitle",
        qet_px: 29,
        weight: Weight::Regular,
    },
    StyleDef {
        key: "cover.subtitle",
        qet_px: 27,
        weight: Weight::Regular,
    },
    StyleDef {
        key: "box.title",
        qet_px: 21,
        weight: Weight::Regular,
    },
    StyleDef {
        key: "cover.table",
        qet_px: 16,
        weight: Weight::Regular,
    },
    StyleDef {
        key: "tag.main",
        qet_px: 13,
        weight: Weight::Regular,
    },
    StyleDef {
        key: "text.normal",
        qet_px: 12,
        weight: Weight::Regular,
    },
    StyleDef {
        key: "field.label",
        qet_px: 11,
        weight: Weight::Regular,
    },
    StyleDef {
        key: "data.small",
        qet_px: 9,
        weight: Weight::Regular,
    },
    StyleDef {
        key: "pin",
        qet_px: 8,
        weight: Weight::Regular,
    },
    StyleDef {
        key: "xref.device",
        qet_px: 7,
        weight: Weight::Regular,
    },
    StyleDef {
        key: "cp.small",
        qet_px: 5,
        weight: Weight::Regular,
    },
    StyleDef {
        key: "marker",
        qet_px: 5,
        weight: Weight::Regular,
    },
];

static BUILTIN: TextStyleTable = TextStyleTable { rows: ROWS };

/// The built-in table.
///
/// ```
/// use ew_text::{Weight, builtin};
/// use ew_units::Nm;
///
/// let style = builtin().resolve("text.normal").unwrap();
/// assert_eq!(style.size, Nm(3_175_000));
/// assert_eq!(style.weight, Weight::Regular);
/// assert!(builtin().resolve("nope").is_none());
/// ```
pub fn builtin() -> &'static TextStyleTable {
    &BUILTIN
}

impl TextStyleTable {
    /// The rows in table order.
    pub fn rows(&self) -> &'static [StyleDef] {
        self.rows
    }

    /// The row of `key`.
    pub fn get(&self, key: &str) -> Option<&'static StyleDef> {
        self.rows.iter().find(|r| r.key == key)
    }

    /// The text style of `key`.
    pub fn resolve(&self, key: &str) -> Option<TextStyle> {
        self.get(key).map(StyleDef::text_style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYS: [&str; 14] = [
        "cover.title",
        "section.id",
        "section.subtitle",
        "cover.subtitle",
        "box.title",
        "cover.table",
        "tag.main",
        "text.normal",
        "field.label",
        "data.small",
        "pin",
        "xref.device",
        "cp.small",
        "marker",
    ];
    const NATIVE_MILLI_PT: [u32; 14] = [
        36000, 27750, 21750, 20250, 15750, 12000, 9750, 9000, 8250, 6750, 6000, 5250, 3750, 3750,
    ];
    const EM_NM: [i64; 14] = [
        12_700_000, 9_789_583, 7_672_917, 7_143_750, 5_556_250, 4_233_333, 3_439_583, 3_175_000, 2_910_417,
        2_381_250, 2_116_667, 1_852_083, 1_322_917, 1_322_917,
    ];
    const PRINTED_MILLI_PT: [i128; 14] = [
        35510, 27370, 21450, 19970, 15530, 11840, 9620, 8880, 8140, 6660, 5920, 5180, 3700, 3700,
    ];
    const PRINTED_EM_NM: [i128; 14] = [
        12_530_000, 9_660_000, 7_570_000, 7_050_000, 5_480_000, 4_180_000, 3_390_000, 3_130_000, 2_870_000,
        2_350_000, 2_090_000, 1_830_000, 1_300_000, 1_300_000,
    ];
    const PRINT_FACTOR: i128 = 986_282_578;

    /// `value × factor / 10^9` rounded half up to a multiple of `step`; asserts the result is not a tie.
    fn printed(value: i128, step: i128) -> i128 {
        let scaled = value * PRINT_FACTOR;
        let unit = step * 1_000_000_000;
        assert_ne!((scaled % unit) * 2, unit, "tie for {value}");
        (scaled + unit / 2).div_euclid(unit) * step
    }

    /// The rows, their keys and order, and every row against doc 20 §17.5, one row per case: only `section.id` is bold; the native size in milli-pt,
    /// the em, the printed size (rounded to 0.01 pt) and the printed em are those of the doc. Doc 20 gives the
    /// printed em rounded to 0.01 mm (decision log ID-38), so the exact printed em (em × 0.986282578, truncated
    /// to nm) must lie within half a step, 0.005 mm, of the doc value.
    #[test]
    fn rows_keys_and_order() {
        let rows = builtin().rows();
        assert_eq!(rows.len(), 14);
        let keys: Vec<&str> = rows.iter().map(|r| r.key).collect();
        assert_eq!(keys, KEYS);
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 14);
        assert!(builtin().get(DEFAULT_STYLE_KEY).is_some());
        assert!(builtin().resolve(DEFAULT_STYLE_KEY).is_some());
        assert!(builtin().get("unknown").is_none());
        assert!(builtin().resolve("unknown").is_none());
        for (i, r) in rows.iter().enumerate() {
            let case = r.key;
            let bold = if r.key == "section.id" {
                Weight::Bold
            } else {
                Weight::Regular
            };
            assert_eq!(r.weight, bold, "case {case}: weight");
            assert_eq!(
                r.native_milli_pt(),
                NATIVE_MILLI_PT[i],
                "case {case}: native milli-pt"
            );
            assert_eq!(r.em(), Nm(EM_NM[i]), "case {case}: em");
            let printed_pt = printed(i128::from(r.native_milli_pt()), 10);
            assert_eq!(printed_pt, PRINTED_MILLI_PT[i], "case {case}: printed milli-pt");
            let exact = (i128::from(r.em().0) * PRINT_FACTOR) / 1_000_000_000;
            let want = PRINTED_EM_NM[i];
            assert!(
                (exact - want).abs() <= 5_000,
                "case {case}: printed em {exact} vs {want}"
            );
        }
    }

    #[test]
    fn resolve_builds_upright_default_family_style() {
        for r in builtin().rows() {
            assert_eq!(
                builtin().resolve(r.key),
                Some(TextStyle {
                    family: Family::Default,
                    size: r.em(),
                    weight: r.weight,
                    italic: false,
                }),
                "{}",
                r.key
            );
        }
    }
}
