// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! FT-418: real exporters' SVG files get the verdicts the lead ruled.

use std::path::{Path, PathBuf};

use ew_schema::{AssetError, AssetFormat, SvgHazard, sniff, svg_hazard};

/// The corpus files, relative to `tests/fixtures/svg-exporters/`, and their ruled verdicts (`None`: accepted).
const CORPUS: &[(&str, Option<SvgHazard>)] = &[
    ("affinity/action-potential-schematic.svg", None),
    ("affinity/water-analogy.svg", None),
    ("coreldraw/alefbet.svg", None),
    ("coreldraw/aeroplane-sketch.svg", None),
    ("drawio/rl-series-circuit.svg", None),
    (
        "drawio/music-classes-html-labels.svg",
        Some(SvgHazard::ForeignObject),
    ),
    ("illustrator/ephedrine-enantiomers.svg", None),
    ("illustrator/keyboard-109.svg", None),
    ("illustrator/euro-edge-inscription.svg", None),
    ("illustrator/sign-2-4mr.svg", None),
    ("illustrator/calendar-2015.svg", None),
    ("illustrator/four-bit-counter.svg", None),
    ("illustrator/dot-pattern.svg", None),
    ("illustrator/animal-cell-hex-colours.svg", None),
    ("illustrator/beetle-range-css-hex.svg", None),
    (
        "illustrator/arpanet-preserve-editing.svg",
        Some(SvgHazard::Entity),
    ),
    (
        "illustrator/bmi-chart-preserve-editing.svg",
        Some(SvgHazard::Entity),
    ),
    ("inkscape/torus-knot-7-9.svg", None),
    ("inkscape/torus-knot-8-9.svg", None),
    ("inkscape/binomial-cube.svg", None),
    ("inkscape/insignia-gradient-chains.svg", None),
    ("inkscape/ten-pointed-stars.svg", None),
    ("inkscape/three-phase-rectifier.svg", None),
    ("inkscape/treaty-map-55-groups.svg", None),
    ("libreoffice/rgb-colour-wheel.svg", None),
    ("visio/choke-diagram.svg", None),
    ("visio/spaghetti-markers.svg", None),
];

/// The corpus folder.
fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/svg-exporters")
}

/// The bytes of one corpus file.
fn read(path: &str) -> Vec<u8> {
    std::fs::read(corpus_root().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn every_exporter_file_gets_its_ruled_verdict() {
    let mut mismatches = Vec::new();
    for (path, expected) in CORPUS {
        let bytes = read(path);
        let sniffed = sniff(&bytes);
        let hazard = svg_hazard(&bytes);
        let ok = match expected {
            None => matches!(sniffed, Ok((AssetFormat::Svg, _))) && hazard.is_none(),
            Some(want) => {
                matches!(sniffed, Err(AssetError::UnsafeSvg(got)) if got == *want) && hazard == Some(*want)
            }
        };
        if !ok {
            mismatches.push(format!(
                "{path}: expected {expected:?}, got sniff {sniffed:?} and svg_hazard {hazard:?}"
            ));
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

#[test]
fn the_corpus_holds_exactly_the_ruled_files() {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
        let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        for entry in entries {
            let path = entry.unwrap_or_else(|e| panic!("{}: {e}", dir.display())).path();
            if path.is_dir() {
                walk(&path, root, out);
            } else if path.extension().is_some_and(|ext| ext == "svg") {
                let relative = path.strip_prefix(root).unwrap();
                out.push(
                    relative
                        .components()
                        .map(|part| part.as_os_str().to_string_lossy())
                        .collect::<Vec<_>>()
                        .join("/"),
                );
            }
        }
    }
    let root = corpus_root();
    let mut found = Vec::new();
    walk(&root, &root, &mut found);
    found.sort();
    let mut expected: Vec<String> = CORPUS.iter().map(|(path, _)| (*path).to_owned()).collect();
    expected.sort();
    assert_eq!(found, expected);
}

#[test]
fn sources_lists_every_corpus_file() {
    let sources = std::fs::read_to_string(corpus_root().join("SOURCES.md"))
        .unwrap_or_else(|e| panic!("SOURCES.md: {e}"));
    let missing: Vec<&str> = CORPUS
        .iter()
        .filter(|(path, _)| !sources.contains(&format!("`{path}`")))
        .map(|(path, _)| *path)
        .collect();
    assert!(missing.is_empty(), "SOURCES.md does not name: {missing:?}");
}

#[test]
fn refused_exporter_files_say_how_to_export_again() {
    for (path, expected) in CORPUS {
        let Some(hazard) = expected else { continue };
        let Err(error) = sniff(&read(path)) else {
            panic!("{path}: accepted");
        };
        let text = error.to_string();
        assert!(
            text.contains(hazard.advice()),
            "{path}: the advice is missing: {text}"
        );
        assert!(
            text.contains("plain SVG") || text.contains("Plain SVG"),
            "{path}: the advice does not name a plain SVG: {text}"
        );
    }
}
