// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shaping with the bundled fonts: fixed vectors, determinism, missing glyphs, line breaking, font-set
//! rules.

#![cfg(feature = "bundled-fonts")]

use std::time::Instant;

use ew_text::{
    Family, FontError, FontSet, FontSource, NOTO_SANS_BOLD_SHA256, NOTO_SANS_MATH_REGULAR_SHA256,
    NOTO_SANS_REGULAR_SHA256, Shaped, TextStyle, Weight, builtin, bundled, bundled_sources,
};
use ew_units::Nm;
use skrifa::MetadataProvider;
use skrifa::raw::types::NameId;

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
fn symbols_shape_with_the_math_fallback_face() {
    let st = style("data.small");
    let regular = bundled().face_for(&st).key;
    let math = bundled()
        .faces()
        .find(|f| f.family == Family::Symbols)
        .expect("the bundled set has the symbol fallback")
        .key;
    assert_eq!(math.sha256, NOTO_SANS_MATH_REGULAR_SHA256);
    // "∞", "⇄" and "−" are not in Noto Sans; the math face has them.
    for ch in ['\u{221E}', '\u{21C4}', '\u{2212}'] {
        assert!(!bundled().covers(&st, ch), "U+{:04X}", ch as u32);
        assert!(bundled().has_glyph(&st, ch), "U+{:04X}", ch as u32);
    }
    let s = bundled().shape("\u{221E} 3-B2, 106-G3", &st, None);
    assert!(s.missing.is_empty(), "{:?}", s.missing);
    // The run keeps the style's face; the "∞" is a real glyph of the math face, the rest stays Noto Sans.
    assert_eq!(s.run.font, regular);
    assert!(s.run.glyphs.iter().all(|g| g.id != 0));
    assert_eq!(s.faces.len(), 2, "{:?}", s.faces);
    assert_eq!((s.faces[0].font, s.faces[0].glyphs.clone()), (math, 0..1));
    assert_eq!(&s.run.text[s.faces[0].text.clone()], "\u{221E}");
    assert_eq!(s.faces[1].font, regular);
    assert_eq!(s.faces[1].glyphs, 1..s.run.glyphs.len());
    assert_eq!(&s.run.text[s.faces[1].text.clone()], " 3-B2, 106-G3");
    // The fallback glyph advances the pen like any other: the next glyph starts after the "∞".
    assert!(s.run.glyphs[1].x > s.run.glyphs[0].x);
    // Text the style's face covers entirely is one span of that face, unchanged by the fallback.
    let plain = bundled().shape("J-E-2416", &st, None);
    assert_eq!(plain.faces.len(), 1);
    assert_eq!(plain.faces[0].font, regular);
    assert_eq!(plain.faces[0].glyphs, 0..plain.run.glyphs.len());
    // A bold style keeps the bold face for what it has and falls back for the rest.
    let bold = bundled().shape("A\u{21C4}B", &style("section.id"), None);
    assert_eq!(bold.run.font.sha256, NOTO_SANS_BOLD_SHA256);
    assert_eq!(bold.faces.len(), 3);
    assert_eq!(bold.faces[1].font, math);
    assert!(bold.missing.is_empty());
    // Spans never cross a line.
    let wrapped = bundled().shape("\u{221E} \u{221E}\n\u{221E}", &st, None);
    assert_eq!(wrapped.lines.len(), 2);
    for line in &wrapped.lines {
        let covered: usize = wrapped
            .faces
            .iter()
            .filter(|f| f.glyphs.start >= line.glyphs.start && f.glyphs.end <= line.glyphs.end)
            .map(|f| f.glyphs.len())
            .sum();
        assert_eq!(covered, line.glyphs.len());
    }
}

#[test]
fn missing_glyphs_are_reported_once_in_order() {
    // "⇄" and "∞" have glyphs in the symbol fallback; characters neither face has are still reported.
    let st = style("data.small");
    let s = bundled().shape("103-G6 ⇄ ∞ ⇄", &st, None);
    assert!(s.missing.is_empty());
    assert!(s.run.glyphs.iter().all(|g| g.id != 0));
    // U+E000 (private use) and U+0378 (unassigned) are in neither face.
    let s = bundled().shape("103-G6 \u{E000} ∞ \u{378} \u{E000}", &st, None);
    assert_eq!(s.missing, vec!['\u{E000}', '\u{378}']);
    // They are drawn as .notdef (glyph 0) of the style's face; the other glyphs are real.
    let zeros = s.run.glyphs.iter().filter(|g| g.id == 0).count();
    assert_eq!(zeros, 3);
    for span in s.faces.iter().filter(|f| f.font != s.run.font) {
        assert!(s.run.glyphs[span.glyphs.clone()].iter().all(|g| g.id != 0));
    }
    assert!(!bundled().has_glyph(&st, '\u{E000}'));
    assert!(bundled().covers(&st, '²'));
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
    let s = bundled().shape("a\u{FE0F}\u{E000}b", &st, None);
    assert_eq!(s.missing, vec!['\u{E000}']);
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

#[test]
fn math_face_identity() {
    let f = bundled()
        .faces()
        .find(|f| f.family == Family::Symbols)
        .expect("the bundled set has the symbol fallback");
    assert_eq!(f.family_name, "Noto Sans Math");
    assert_eq!(f.version, "Version 3.000");
    assert_eq!(f.weight, Weight::Regular);
    // The name table entries NOTICE and REUSE.toml reproduce, and the characters FR-OUT-027 needs.
    let data = bundled().face_data(f.key).unwrap();
    assert_eq!(data.len(), 657_440);
    let font = skrifa::FontRef::new(data).unwrap();
    let name = |id| {
        font.localized_strings(id)
            .english_or_first()
            .map(|s| s.chars().collect::<String>())
    };
    assert_eq!(
        name(NameId::COPYRIGHT_NOTICE).as_deref(),
        Some("Copyright 2022 Google LLC. All Rights Reserved.")
    );
    assert_eq!(
        name(NameId::TRADEMARK).as_deref(),
        Some("Noto is a trademark of Google Inc.")
    );
    assert_eq!(name(NameId::SUBFAMILY_NAME).as_deref(), Some("Regular"));
    let charmap = font.charmap();
    for ch in ['\u{221E}', '\u{2212}', '\u{21C4}'] {
        assert!(charmap.map(ch).is_some(), "U+{:04X}", ch as u32);
    }
    // Same units per em as Noto Sans: fallback glyph positions stay whole font units (crate docs, step 2).
    assert_eq!(
        f.metrics.units_per_em,
        bundled().face_for(&style("text.normal")).metrics.units_per_em
    );
}
