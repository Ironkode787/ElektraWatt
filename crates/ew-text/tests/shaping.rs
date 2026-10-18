// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shaping with the bundled fonts: fixed vectors, determinism, missing glyphs, line breaking, font-set
//! rules.

#![cfg(feature = "bundled-fonts")]

use std::time::Instant;

use ew_text::{
    Family, FontError, FontSet, FontSource, NOTO_SANS_BOLD_SHA256, NOTO_SANS_REGULAR_SHA256, Shaped,
    TextStyle, Weight, builtin, bundled, bundled_sources,
};
use ew_units::Nm;

fn style(key: &str) -> TextStyle {
    builtin().resolve(key).unwrap()
}

/// `id@x` per glyph (y is 0 on single lines), then the advance and the layout box's y extent.
fn vector(s: &Shaped) -> String {
    let g: Vec<String> = s
        .run
        .glyphs
        .iter()
        .map(|g| format!("{}@{}", g.id, g.x.0))
        .collect();
    format!(
        "{} | adv {} | y {}..{}",
        g.join(" "),
        s.advance().0,
        s.run.layout_box.min().y.0,
        s.run.layout_box.max().y.0
    )
}

#[test]
fn fixed_vectors() {
    let normal = style("text.normal");
    let small = style("data.small");
    let s = bundled().shape("J-E-2416", &normal, None);
    assert_eq!(s.run.font.sha256, NOTO_SANS_REGULAR_SHA256);
    assert_eq!(s.run.size, Nm(3_175_000));
    assert!(s.missing.is_empty());
    insta::assert_snapshot!(vector(&s), @"45@0 16@866775 40@1889125 16@3654425 21@4676775 23@6492875 20@8308975 25@10125075 | adv 11941175 | y -3394075..930275");
    let s = bundled().shape("Power Supply 1 - Main Current", &normal, None);
    insta::assert_snapshot!(vector(&s), @"51@0 82@1920875 90@3778250 72@6273800 85@8064500 3@9375775 54@10201275 88@11944350 83@13906500 83@15859125 79@17811750 92@18630900 3@20250150 20@21075650 3@22891750 16@23717250 3@24739600 48@25565100 68@28444825 76@30226000 81@31045150 3@33007300 38@33832800 88@35839400 85@37801550 85@39112825 72@40360600 81@42151300 87@44113450 | adv 45259625 | y -3394075..930275");
    let s = bundled().shape("0.75 mm²", &small, None);
    assert!(s.missing.is_empty());
    insta::assert_snapshot!(vector(&s), @"19@0 17@1362075 26@2000250 24@3362325 3@4724400 80@5343525 80@7569994 116@9796462 | adv 10629900 | y -2545556..697706");
    let s = bundled().shape("=P01+J-E-2416", &style("section.id"), None);
    assert_eq!(s.run.font.sha256, NOTO_SANS_BOLD_SHA256);
    insta::assert_snapshot!(vector(&s), @"32@0 51@5599641 19@11747500 20@17347141 14@22946783 45@28546424 16@31786776 40@34939022 16@40421188 21@43573434 23@49173075 20@54772717 25@60372358 | adv 65972000 | y -10465064..2868348");
}

#[test]
fn missing_glyphs_are_reported_once_in_order() {
    // "⇄" (and "∞") need the symbol fallback font of FR-OUT-027, which is not bundled yet.
    let s = bundled().shape("103-G6 ⇄ ∞ ⇄", &style("data.small"), None);
    assert_eq!(s.missing, vec!['⇄', '∞']);
    // They are drawn as .notdef (glyph 0); the other glyphs are real.
    let zeros = s.run.glyphs.iter().filter(|g| g.id == 0).count();
    assert_eq!(zeros, 3);
    assert!(!bundled().covers(&style("data.small"), '⇄'));
    assert!(bundled().covers(&style("data.small"), '²'));
    // Line breaks are not glyphs and not reported.
    assert!(bundled().shape("a\nb", &style("pin"), None).missing.is_empty());
}

#[test]
fn control_characters_are_reported_when_drawn_as_notdef() {
    for text in ["a\tb", "a\u{7}b"] {
        let s = bundled().shape(text, &style("data.small"), None);
        let ch = text.chars().nth(1).unwrap();
        assert_eq!(s.missing, vec![ch], "{text:?}");
        // The character is drawn as the visible .notdef glyph.
        assert!(s.run.glyphs.iter().any(|g| g.id == 0), "{text:?}");
    }
    // Line breaks end a line and are not reported.
    assert!(bundled().shape("a\r\nb", &style("pin"), None).missing.is_empty());
}

#[test]
fn default_ignorable_code_points_are_not_reported() {
    let st = style("data.small");
    for ch in [
        '\u{FE0F}',
        '\u{200B}',
        '\u{200D}',
        '\u{00AD}',
        '\u{FEFF}',
        '\u{2060}',
        '\u{E0100}',
    ] {
        let text = format!("a{ch}b");
        let s = bundled().shape(&text, &st, None);
        assert!(s.missing.is_empty(), "U+{:04X}", ch as u32);
    }
    // Real missing characters next to them are still reported.
    let s = bundled().shape("a\u{FE0F}\u{21C4}b", &st, None);
    assert_eq!(s.missing, vec!['\u{21C4}']);
}

#[test]
fn deterministic_across_runs_and_font_load_order() {
    let texts = [
        "J-E-2416",
        "Power Supply 1 - Main Current",
        "0.75 mm²",
        "Ünïcödé ÆØÅ ß €",
    ];
    let reversed: Vec<FontSource> = bundled_sources().into_iter().rev().collect();
    let other = FontSet::new(reversed).unwrap();
    let fresh = FontSet::new(bundled_sources()).unwrap();
    for t in texts {
        for key in ["text.normal", "section.id", "cp.small"] {
            let st = style(key);
            let a = bundled().shape(t, &st, None);
            assert_eq!(a, bundled().shape(t, &st, None), "{t} {key}: second run");
            assert_eq!(a, other.shape(t, &st, None), "{t} {key}: reversed load order");
            assert_eq!(a, fresh.shape(t, &st, None), "{t} {key}: fresh set");
        }
    }
    let keys = |s: &FontSet| s.faces().map(|f| (f.weight, f.key)).collect::<Vec<_>>();
    assert_eq!(keys(&other), keys(bundled()));
}

#[test]
fn positions_scale_linearly_with_size() {
    // Shaping happens in font units; nm positions are exact rescalings of the same integers.
    let a = bundled().shape("Prev: 100", &TextStyle::regular(Nm(1_000_000)), None);
    let b = bundled().shape("Prev: 100", &TextStyle::regular(Nm(2_000_000)), None);
    for (ga, gb) in a.run.glyphs.iter().zip(&b.run.glyphs) {
        assert_eq!(ga.id, gb.id);
        assert_eq!(ga.x.0 * 2, gb.x.0);
    }
    // At an em of 1000 nm (= units per em) every position is a whole font unit.
    let u = bundled().shape("Prev: 100", &TextStyle::regular(Nm(1_000)), None);
    assert_eq!(u.advance().0 * 1_000, a.advance().0);
}

#[test]
fn line_breaking_only_with_max_width() {
    let st = style("xref.device");
    let t = "Reproduction or disclosure of this drawing only with written permission of the owner.";
    let one = bundled().shape(t, &st, None);
    assert_eq!(one.lines.len(), 1);
    let w = Nm(one.advance().0 / 3);
    let wrapped = bundled().shape(t, &st, Some(w));
    assert!(wrapped.lines.len() >= 3, "{:?}", wrapped.lines);
    let pitch = bundled().vertical_metrics(&st).line_pitch;
    for (i, line) in wrapped.lines.iter().enumerate() {
        assert_eq!(line.baseline, Nm(pitch.0 * i as i64));
        // Greedy breaking keeps each line within the width, trailing spaces aside.
        let text = t[line.text.clone()].trim_end();
        let alone = bundled().shape(text, &st, None);
        assert!(alone.advance() <= w, "line {i} {text:?}");
        for g in &wrapped.run.glyphs[line.glyphs.clone()] {
            assert_eq!(g.y, line.baseline);
        }
    }
    // The glyphs of all lines together are those of the single line (no hyphenation, same shaping).
    let ids = |s: &Shaped| {
        s.run
            .glyphs
            .iter()
            .map(|g| g.id)
            .filter(|&id| id != 3)
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(&wrapped), ids(&one));
    assert_eq!(
        wrapped.run.layout_box.max().y,
        wrapped.lines.last().unwrap().baseline + bundled().vertical_metrics(&st).descent
    );
}

#[test]
fn metrics_of_the_default_face() {
    let f = bundled().face_for(&style("text.normal"));
    assert_eq!(f.family_name, "Noto Sans");
    assert_eq!(f.metrics.units_per_em, 1000);
    // Doc 20 §18.1 assumes a cap ratio of 0.714.
    assert_eq!(f.metrics.cap_height, 714);
    let m = bundled().vertical_metrics(&style("text.normal"));
    assert_eq!(m.cap_height, Nm(2_266_950));
}

#[test]
fn font_set_rules() {
    let regular = bundled_sources().remove(0);
    let wrong = FontSource::from_vec(vec![0, 1, 0, 0], Family::Default, Weight::Regular, false)
        .pinned(NOTO_SANS_REGULAR_SHA256);
    assert!(matches!(
        FontSet::new(vec![wrong]),
        Err(FontError::HashMismatch { .. })
    ));
    let junk = FontSource::from_vec(vec![1, 2, 3], Family::Default, Weight::Regular, false);
    assert!(matches!(
        FontSet::new(vec![junk]),
        Err(FontError::NotAFont { .. })
    ));
    assert_eq!(
        FontSet::new(vec![regular.clone(), regular.clone()]).unwrap_err(),
        FontError::Duplicate {
            family: Family::Default,
            weight: Weight::Regular,
            italic: false
        }
    );
    assert_eq!(FontSet::new(vec![]).unwrap_err(), FontError::Empty);
    // A set without Bold falls back to Regular for bold styles.
    let only = FontSet::new(vec![regular]).unwrap();
    let s = only.shape("X", &style("section.id"), None);
    assert_eq!(s.run.font.sha256, NOTO_SANS_REGULAR_SHA256);
}

#[test]
fn long_text_sanity() {
    let text: String = "Power Supply 1 - Main Current; "
        .chars()
        .cycle()
        .take(1_000)
        .collect();
    let start = Instant::now();
    let s = bundled().shape(&text, &style("text.normal"), None);
    let took = start.elapsed();
    assert_eq!(s.run.glyphs.len(), 1_000);
    // No budget is asserted (debug builds vary); the time is printed for inspection.
    println!("1000 characters shaped in {took:?}");
}
