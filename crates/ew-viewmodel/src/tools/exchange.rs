// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The change-symbol tool (`exchange`; WP-S2.04 §7.5, FR-SYM-022): one selected representation shown with another
//! symbol, committed with `rep.exchange_symbol`.
//!
//! State `armed`. The overlay is the new shape's outline at the representation's position and orientation with a
//! `cp` marker per CP it draws and a dashed rectangle around each function CP that would become unmapped. When the
//! new symbol draws the function's own definition the map is by index, so the unmapped CPs are known at once; for
//! another definition the map is by designation, which the command derives: the tool marks the CPs the preview's
//! notices or refusal name (a dwell previews). `Dwell` previews, `Activate` or a press commits
//! `rep.exchange_symbol {rep, symbol}`, `Escape` answers `None` (the host returns to Select).

use std::sync::Arc;

use ew_api::AnyCommand;
use ew_api::geometry::orient_point;
use ew_api::output::SymbolShape;
use ew_api::output::ew_dl::EntityRef;
use ew_api::output::ew_dl::ew_units::{Nm, Pt2, Rect};
use serde_json::json;

use super::{
    Button, CanvasEvent, Ghost, MarkerKind, Overlay, OverlayItem, PreviewInfo, Style, Token, Tool, ToolCx,
    ToolOutput,
};

/// Half the side of the dashed rectangle around an unmapped CP, sheet nm.
const UNMAPPED_HALF_NM: i64 = 1_000_000;

/// One function CP the representation draws now: index, designation, sheet point.
#[derive(Clone, Debug)]
struct DrawnCp {
    idx: u16,
    designation: String,
    at: (i64, i64),
}

/// The change-symbol tool.
#[derive(Clone, Debug)]
pub struct ExchangeTool {
    rep: EntityRef,
    shape: Arc<SymbolShape>,
    /// The outline and markers of the new shape, sheet nm.
    base: Vec<OverlayItem>,
    /// The function CPs the representation draws now (its first function's).
    drawn: Vec<DrawnCp>,
    /// Whether the new symbol draws the function's own definition (the map is by index).
    same_def: bool,
    /// The function CP indices left unmapped (known by index, or named by the preview).
    unmapped: Vec<u16>,
}

/// The function CP indices a message names (`function CP 3 …`), in order.
fn named_cps(text: &str) -> Vec<u16> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("function CP ") {
        rest = &rest[i + "function CP ".len()..];
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if let Ok(n) = digits.parse::<u16>()
            && !out.contains(&n)
        {
            out.push(n);
        }
    }
    out
}

impl ExchangeTool {
    /// The tool for representation `rep` of the page of `cx` and the new symbol's `shape`; `same_def` when the new
    /// symbol draws the representation's function definition.
    pub fn new(rep: EntityRef, shape: Arc<SymbolShape>, same_def: bool, cx: &ToolCx<'_>) -> ExchangeTool {
        let tables = &cx.page.tables;
        let row = tables.reps.iter().find(|r| r.id == rep.0);
        let mut base = Vec::new();
        let mut drawn = Vec::new();
        if let Some(r) = row {
            base.push(OverlayItem::Paths {
                paths: shape.outline.clone().into(),
                transform: (r.orient, r.pos),
            });
            for cp in &shape.cps {
                let o = orient_point(r.orient, cp.at);
                base.push(OverlayItem::Marker {
                    at: cx.snapper.page_to_sheet((r.pos.0 + o.0, r.pos.1 + o.1)),
                    kind: MarkerKind::Cp,
                });
            }
            if let Some(f) = r.functions.first() {
                drawn = tables
                    .cps
                    .iter()
                    .filter(|c| c.rep == r.id && c.function == *f)
                    .map(|c| DrawnCp {
                        idx: c.idx,
                        designation: c
                            .label
                            .rsplit_once(':')
                            .map_or(c.label.as_str(), |(_, d)| d)
                            .to_owned(),
                        at: cx.snapper.page_to_sheet(c.at),
                    })
                    .collect();
            }
        }
        let unmapped = if same_def {
            let mapped: Vec<u16> = shape.cps.iter().map(|c| c.fn_cp).collect();
            drawn
                .iter()
                .filter(|c| !mapped.contains(&c.idx))
                .map(|c| c.idx)
                .collect()
        } else {
            Vec::new()
        };
        ExchangeTool {
            rep,
            shape,
            base,
            drawn,
            same_def,
            unmapped,
        }
    }

    /// The overlay (see the module documentation).
    pub fn overlay(&self) -> Overlay {
        let mut items = self.base.clone();
        for c in self.drawn.iter().filter(|c| self.unmapped.contains(&c.idx)) {
            let s = c.at;
            items.push(OverlayItem::Rect {
                r: Rect::from_points(
                    Pt2::new(Nm(s.0 - UNMAPPED_HALF_NM), Nm(s.1 - UNMAPPED_HALF_NM)),
                    Pt2::new(Nm(s.0 + UNMAPPED_HALF_NM), Nm(s.1 + UNMAPPED_HALF_NM)),
                ),
                style: Style::dashed(Token::Ghost),
            });
        }
        Overlay::new(items)
    }

    fn command(&self) -> Box<AnyCommand> {
        Box::new(AnyCommand {
            name: "rep.exchange_symbol".into(),
            params: json!({"rep": self.rep.0.to_string(), "symbol": self.shape.copy.to_string()}),
        })
    }
}

impl Tool for ExchangeTool {
    fn name(&self) -> &'static str {
        "exchange"
    }

    fn state(&self) -> &'static str {
        "armed"
    }

    fn on_event(&mut self, ev: &CanvasEvent, _cx: &ToolCx<'_>) -> ToolOutput {
        match ev {
            CanvasEvent::PointerMove { .. } | CanvasEvent::CursorMove { .. } => {
                ToolOutput::Transient(self.overlay())
            }
            CanvasEvent::Dwell { ms } if *ms >= super::PREVIEW_DWELL_MS => {
                ToolOutput::Preview(self.command())
            }
            CanvasEvent::Activate
            | CanvasEvent::Press {
                button: Button::Primary,
                ..
            } => ToolOutput::Commit(self.command()),
            _ => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> &'static str {
        "hint-exchange"
    }

    fn wants_hover(&self) -> bool {
        false
    }

    fn ghost(&self) -> Option<Ghost> {
        Some(Ghost::Exchange {
            symbol: self.shape.key.clone(),
            unmapped: self
                .drawn
                .iter()
                .filter(|c| self.unmapped.contains(&c.idx))
                .map(|c| c.designation.clone())
                .collect(),
        })
    }

    fn on_preview(&mut self, info: &PreviewInfo) -> Option<Overlay> {
        if self.same_def {
            return None;
        }
        let mut named: Vec<u16> = Vec::new();
        for text in info.notices.iter().chain(info.message.iter()) {
            for n in named_cps(text) {
                if !named.contains(&n) {
                    named.push(n);
                }
            }
        }
        self.unmapped = named;
        Some(self.overlay())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cps_a_message_names_are_read_in_order() {
        assert_eq!(
            named_cps("function CP 1 is connected on this page; function CP 3 is no longer drawn"),
            vec![1, 3]
        );
        assert!(named_cps("nothing").is_empty());
    }
}
