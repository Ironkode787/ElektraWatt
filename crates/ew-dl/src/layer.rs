// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Layer`: the display-list layers of `page_display_list(page, layer, lang)`.

/// One display-list layer (ADR-0015 decision 1 and amendment 2; doc 09 §4.1; doc 12 §5.2). Each layer is
/// its own memoised query, so a wire edit never rebuilds the title block.
///
/// Schematic and report pages use [`Layer::SCHEMATIC`]; panel-layout pages use [`Layer::PANEL`].
///
/// ```
/// use ew_dl::Layer;
///
/// assert_eq!(Layer::Wires.name(), "wires");
/// assert!(!Layer::Highlights.is_exported());
/// assert!(Layer::PANEL.contains(&Layer::Frame));
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Layer {
    /// Frame form, grid labels, title block.
    Frame,
    /// Symbol representations.
    Symbols,
    /// Connections.
    Wires,
    /// Derived texts: tags, cross-references, wire labels.
    Labels,
    /// Embedded blocks and report data areas.
    Tables,
    /// Revision clouds and marks (1.0).
    Revision,
    /// Screen-only selection and check highlights; never exported.
    Highlights,
    /// Panel layout: mounting surface.
    Surface,
    /// Panel layout: carriers (rails, ducts, busbars).
    Carriers,
    /// Panel layout: placed devices.
    Placements,
    /// Panel layout: machining features (holes, threads, cut-outs, slots).
    Features,
    /// Panel layout: dimensions.
    Dims,
}

impl Layer {
    /// Every layer, in declaration order.
    pub const ALL: [Layer; 12] = [
        Layer::Frame,
        Layer::Symbols,
        Layer::Wires,
        Layer::Labels,
        Layer::Tables,
        Layer::Revision,
        Layer::Highlights,
        Layer::Surface,
        Layer::Carriers,
        Layer::Placements,
        Layer::Features,
        Layer::Dims,
    ];

    /// The layers of schematic and report pages (ADR-0015 amendment 2), in the order the amendment lists
    /// them.
    pub const SCHEMATIC: [Layer; 7] = [
        Layer::Frame,
        Layer::Symbols,
        Layer::Wires,
        Layer::Labels,
        Layer::Tables,
        Layer::Revision,
        Layer::Highlights,
    ];

    /// The layers of panel-layout pages (ADR-0015 amendment 2), in the order the amendment lists them.
    pub const PANEL: [Layer; 9] = [
        Layer::Surface,
        Layer::Carriers,
        Layer::Placements,
        Layer::Features,
        Layer::Dims,
        Layer::Frame,
        Layer::Labels,
        Layer::Tables,
        Layer::Highlights,
    ];

    /// Whether the layer reaches exports (PDF, SVG, PNG, DXF, print). Only [`Layer::Highlights`] is screen
    /// only (doc 09 §4.1).
    pub const fn is_exported(self) -> bool {
        !matches!(self, Layer::Highlights)
    }

    /// A stable lower-case name for query keys, logs and snapshots. Never changes once released.
    pub const fn name(self) -> &'static str {
        match self {
            Layer::Frame => "frame",
            Layer::Symbols => "symbols",
            Layer::Wires => "wires",
            Layer::Labels => "labels",
            Layer::Tables => "tables",
            Layer::Revision => "revision",
            Layer::Highlights => "highlights",
            Layer::Surface => "surface",
            Layer::Carriers => "carriers",
            Layer::Placements => "placements",
            Layer::Features => "features",
            Layer::Dims => "dims",
        }
    }

    /// The layer with this [`name`](Layer::name).
    pub fn from_name(name: &str) -> Option<Layer> {
        Layer::ALL.into_iter().find(|l| l.name() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_flags() {
        for l in Layer::ALL {
            assert_eq!(l.is_exported(), l != Layer::Highlights, "{l:?}");
        }
    }

    #[test]
    fn names_are_unique_and_round_trip() {
        let names: Vec<&str> = Layer::ALL.iter().map(|l| l.name()).collect();
        assert_eq!(
            names,
            [
                "frame",
                "symbols",
                "wires",
                "labels",
                "tables",
                "revision",
                "highlights",
                "surface",
                "carriers",
                "placements",
                "features",
                "dims"
            ]
        );
        for l in Layer::ALL {
            assert_eq!(Layer::from_name(l.name()), Some(l));
        }
        assert_eq!(Layer::from_name("Frame"), None);
    }

    #[test]
    fn page_kind_sets() {
        assert!(Layer::SCHEMATIC.iter().all(|l| Layer::ALL.contains(l)));
        assert!(!Layer::SCHEMATIC.contains(&Layer::Dims));
        assert!(!Layer::PANEL.contains(&Layer::Symbols));
        assert!(!Layer::PANEL.contains(&Layer::Wires));
        assert!(!Layer::PANEL.contains(&Layer::Revision));
        assert!(Layer::PANEL.contains(&Layer::Highlights));
    }
}
