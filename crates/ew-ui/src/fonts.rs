// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The UI fonts (WP-S2.01 part 1, spec section 5.3): egui's built-in fonts are off (`default_fonts` is not a
//! feature of this build), and the bundled Noto Sans of `ew-text` becomes the proportional and monospace family.
//! The face bytes come from `ew_api::output::font_bytes()`, the same set the PDF export embeds, so the screen and
//! the paper use one font file (NFR-REL-11: no system fonts).

use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};
use ew_viewmodel::ew_api::output::font_bytes;
use skrifa::string::StringId;
use skrifa::{FontRef, MetadataProvider};

/// The name of the regular face in the font definitions.
pub const REGULAR: &str = "NotoSans-Regular";

/// The name of the symbol face (Noto Sans Math), the last fallback for glyphs the regular face lacks.
pub const SYMBOLS: &str = "NotoSansMath-Regular";

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
    defs.families.insert(FontFamily::Monospace, order);
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
}
