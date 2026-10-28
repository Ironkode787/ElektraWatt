// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The UI fonts (WP-S2.01 part 1, spec section 5.3): egui's built-in fonts are off (`default_fonts` is not a
//! feature of this build), and the bundled Noto Sans of `ew-text` becomes the proportional and monospace family.
//! The face bytes come from `ew_api::output::font_bytes()`, the same set the PDF export embeds, so the screen and
//! the paper use one font file (NFR-REL-11: no system fonts). WP-S2.17 part 1 adds Noto Sans SemiBold as the family
//! `semibold` for the chrome's emphasis (D8); it is not an export face.

use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};
use ew_viewmodel::ew_api::output::font_bytes;
use skrifa::string::StringId;
use skrifa::{FontRef, MetadataProvider};

/// The name of the regular face in the font definitions.
pub const REGULAR: &str = "NotoSans-Regular";

/// The name of the symbol face (Noto Sans Math), the last fallback for glyphs the regular face lacks.
pub const SYMBOLS: &str = "NotoSansMath-Regular";

/// The name of the SemiBold face (WP-S2.17 D8): UI emphasis only, never the drawings.
pub const SEMIBOLD: &str = "NotoSans-SemiBold";

/// The egui font family of the SemiBold face (`FontFamily::Name("semibold")`).
pub const SEMIBOLD_FAMILY: &str = "semibold";

/// The bundled Noto Sans SemiBold (`assets/fonts/NotoSans-SemiBold.ttf`, OFL-1.1; provenance in `NOTICE`). It is the
/// UI's own face, not one of the export's: the drawings keep Regular and Bold (D8).
const NOTO_SANS_SEMIBOLD: &[u8] = include_bytes!("../../../assets/fonts/NotoSans-SemiBold.ttf");

/// What a bundled face is, read from its own tables.
struct Face {
    family: String,
    weight: f32,
    data: FontData,
}

fn faces() -> Vec<Face> {
    font_bytes()
        .values()
        .filter_map(|bytes| {
            let font = FontRef::from_index(&bytes.data, bytes.index).ok()?;
            let family = font
                .localized_strings(StringId::FAMILY_NAME)
                .english_or_first()?
                .chars()
                .collect::<String>();
            Some(Face {
                family,
                weight: font.attributes().weight.value(),
                data: FontData::from_blob(Arc::new(Arc::clone(&bytes.data)), bytes.index),
            })
        })
        .collect()
}

/// The font definitions of the shell: Noto Sans Regular first, Noto Sans Math as the fallback, in both families.
pub fn definitions() -> FontDefinitions {
    let mut defs = FontDefinitions::empty();
    let faces = faces();
    let regular = faces
        .iter()
        .find(|f| f.family == "Noto Sans" && (f.weight - 400.0).abs() < 1.0);
    let symbols = faces.iter().find(|f| f.family.contains("Math"));
    let mut order: Vec<String> = Vec::new();
    for (name, face) in [(REGULAR, regular), (SYMBOLS, symbols)] {
        if let Some(face) = face {
            defs.font_data
                .insert(name.to_owned(), Arc::new(face.data.clone()));
            order.push(name.to_owned());
        }
    }
    defs.families.insert(FontFamily::Proportional, order.clone());
    defs.families.insert(FontFamily::Monospace, order.clone());
    defs.font_data.insert(
        SEMIBOLD.to_owned(),
        Arc::new(FontData::from_static(NOTO_SANS_SEMIBOLD)),
    );
    let mut semibold = vec![SEMIBOLD.to_owned()];
    semibold.extend(order);
    defs.families
        .insert(FontFamily::Name(SEMIBOLD_FAMILY.into()), semibold);
    defs
}

/// Installs the bundled fonts into `ctx` (takes effect with the next pass).
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(definitions());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_regular_and_the_symbol_face_are_found_in_the_bundle() {
        let defs = definitions();
        assert!(defs.font_data.contains_key(REGULAR), "Noto Sans Regular");
        assert!(defs.font_data.contains_key(SYMBOLS), "Noto Sans Math");
        let proportional = &defs.families[&FontFamily::Proportional];
        assert_eq!(proportional.first().map(String::as_str), Some(REGULAR));
        assert_eq!(defs.families[&FontFamily::Monospace], *proportional);
        // The bundled faces are the export's: the regular face is the one file of NotoSans-Regular.ttf.
        let regular = defs.font_data[REGULAR].bytes();
        assert!(font_bytes().values().any(|b| &*b.data == regular));
    }

    #[test]
    fn the_semibold_family_starts_with_the_semibold_face_and_falls_back_to_the_others() {
        let defs = definitions();
        let family = &defs.families[&FontFamily::Name(SEMIBOLD_FAMILY.into())];
        assert_eq!(family, &[SEMIBOLD, REGULAR, SYMBOLS]);
        let face = FontRef::new(defs.font_data[SEMIBOLD].bytes()).unwrap();
        assert_eq!(face.attributes().weight.value(), 600.0);
        let name: String = face
            .localized_strings(StringId::FAMILY_NAME)
            .english_or_first()
            .unwrap()
            .chars()
            .collect();
        assert_eq!(name, "Noto Sans SemiBold");
    }

    /// The SemiBold file is the one of `NOTICE` (ADR-0028 recommendation 6: bundled fonts are pinned by SHA-256).
    #[test]
    fn the_semibold_file_is_the_pinned_one() {
        use sha2::{Digest, Sha256};
        let digest: String = Sha256::digest(NOTO_SANS_SEMIBOLD)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            digest,
            "854409525d63d9f716d9ae3545a944e4b2af11abbee744e4f2b4a819eb386482"
        );
        assert_eq!(NOTO_SANS_SEMIBOLD.len(), 348_812);
    }
}
