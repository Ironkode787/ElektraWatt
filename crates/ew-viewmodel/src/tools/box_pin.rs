// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Naming a pin of a drawn box in place (`box_pin`, rc.5: a box drawn without a part could not get pins). A box whose
//! pins are its own ([`TableBox::own_pins`](ew_api::output::TableBox::own_pins)) takes a pin where it stands: Add pin
//! (its right-click, Edit ▸ Add pin), a wire drawn to its edge where no pin is, or a double click on a pin to rename it
//! opens this tool on the pin's point, and the text popup asks for its name. Add pin without a point (the bar's) arms
//! the tool first: the new pin follows the pointer along the box's edge nearest it, and a click puts it there (rc.5: the
//! person did not know where a new pin lands).
//!
//! | State | Event | Next | Output |
//! |---|---|---|---|
//! | armed | `PointerMove`, `CursorMove` | armed (the pin on the free point of the outline nearest the point) | `Transient`: the pin's marker and "new pin" |
//! | armed | `Press` or `Activate` | typing | `Transient` |
//! | typing | `Text(s)` | typing (name = `s`, the field's line cleared) | `Transient`: the pin's marker and its name |
//! | typing | `Activate` or `Key(Finish)`, a name | typing, then finished | `Commit(box.reshape …)` with the pin named |
//! | typing | `Key(NextProposal)` (Tab) on a new pin, a name | typing on the next free point of its side, an empty name | `Commit(box.reshape …)` |
//! | typing | a blank name, or one with a space | typing | `None`; the line under the field says why ([`Tool::field_error`]) |
//! | armed, typing | `Escape` | finished | `Cancel` |
//!
//! Two pins of one box may have one name (two 24 V inputs), as a part's pins may (rc.5).
//!
//! Each name is one `box.reshape` with `cps`, one undo step. The host returns to Select once the tool is finished, or,
//! for a pin named for a wire, hands the Wire tool back to end the wire at the new pin (one undo step with it).

use crate::key;
use ew_api::AnyCommand;
use ew_api::output::ew_dl::EntityRef;

use super::black_box::{BoxCpAt, reshape_named};
use super::wire::PageGridMap;
use super::{
    Anchor, CanvasEvent, Ghost, MarkerKind, Overlay, OverlayItem, Tool, ToolCx, ToolKey, ToolOutput,
};

/// The pin name tool (see the module documentation).
pub struct PinNameTool {
    /// The box.
    rep: EntityRef,
    /// The pin named: its function CP index, point and direction.
    pin: BoxCpAt,
    /// Whether the pin is new (Add pin) rather than renamed.
    new: bool,
    /// The name typed so far.
    name: String,
    /// Whether Tab asked for the next pin of the side.
    next: bool,
    /// Whether a name was committed.
    named: bool,
    /// Whether the pin still follows the pointer (armed, before the click that puts it).
    armed: bool,
    /// Why the name typed cannot be taken, said under the field.
    error: Option<String>,
    /// Whether the host returns to Select (or to the wire).
    finished: bool,
    /// The Wire tool whose wire ends at the new pin, handed back by the host once it is named.
    pub(crate) resume: Option<Box<dyn Tool>>,
}

impl std::fmt::Debug for PinNameTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PinNameTool")
            .field("rep", &self.rep)
            .field("pin", &self.pin)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl PinNameTool {
    /// The tool naming the new pin `pin` of the box `rep`.
    pub fn add(rep: EntityRef, pin: BoxCpAt) -> PinNameTool {
        PinNameTool {
            rep,
            pin,
            new: true,
            name: String::new(),
            next: false,
            named: false,
            armed: false,
            error: None,
            finished: false,
            resume: None,
        }
    }

    /// The tool armed for a new pin of the box `rep`, which follows the pointer along the box's edge; `pin` where it
    /// stands first.
    pub fn arm(rep: EntityRef, pin: BoxCpAt) -> PinNameTool {
        PinNameTool {
            armed: true,
            ..PinNameTool::add(rep, pin)
        }
    }

    /// The tool renaming the pin `pin` of the box `rep`, named `name` now.
    pub fn rename(rep: EntityRef, pin: BoxCpAt, name: String) -> PinNameTool {
        PinNameTool {
            new: false,
            name,
            ..PinNameTool::add(rep, pin)
        }
    }

    /// Whether the pin is named (or the naming cancelled): the host returns to Select or to the wire.
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// The pin's point on the page.
    pub fn at(&self) -> (i64, i64) {
        self.pin.1
    }

    /// The box.
    pub fn rep(&self) -> EntityRef {
        self.rep
    }

    /// Whether a name was committed (the wire then ends at the pin).
    pub fn named(&self) -> bool {
        self.named
    }

    /// Whether Tab's commit asks for the next pin of the side.
    pub fn wants_next(&self) -> bool {
        self.next
    }

    /// The `box.reshape` that names the pin `name`, or the reason it cannot.
    fn command(&self, cx: &ToolCx<'_>) -> Result<Box<AnyCommand>, crate::strings::Key<'static>> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(key!("reason-pin-name-empty"));
        }
        if name.chars().any(char::is_whitespace) {
            return Err(key!("reason-pin-name-space"));
        }
        let Some(rep) = super::black_box::black_box_rep(cx, self.rep) else {
            return Err(key!("reason-one-black-box"));
        };
        let Some(b) = &rep.black_box else {
            return Err(key!("reason-one-black-box"));
        };
        let mut cps = b.cps.clone();
        if self.new {
            cps.push(self.pin);
        }
        let mut names: Vec<(u16, String)> = b
            .names
            .iter()
            .filter(|(i, _)| *i != self.pin.0)
            .cloned()
            .collect();
        names.push((self.pin.0, name.to_owned()));
        Ok(reshape_named(rep, &b.outline, Some(&cps), &names))
    }

    fn commit(&mut self, next: bool, cx: &ToolCx<'_>) -> ToolOutput {
        match self.command(cx) {
            Ok(cmd) => {
                self.next = next && self.new && self.resume.is_none();
                ToolOutput::Commit(cmd)
            }
            // Said under the field, which stays open to type on (rc.5: a banner covered it and took the typing).
            Err(reason) => {
                self.error = Some(crate::strings::text(reason).to_owned());
                ToolOutput::None
            }
        }
    }

    fn overlay(&self, cx: &ToolCx<'_>) -> Overlay {
        let at = cx.snapper.page_to_sheet(self.pin.1);
        let mut items = vec![OverlayItem::Marker {
            at,
            kind: MarkerKind::Cp,
        }];
        let label = if self.armed {
            crate::strings::text(key!("label-box-pin-new")).to_owned()
        } else {
            self.name.clone()
        };
        if !label.is_empty() {
            items.push(OverlayItem::Label {
                at,
                text: label,
                anchor: Anchor::Above,
            });
        }
        Overlay::new(items)
    }
}

/// The next free point after the pin `at` on its side of the box `outline` with the pins `cps` (Tab, rc.5): the nearest
/// grid point further along the side, else back along it, at least one step from both corners and on no pin; `None`
/// when the side has no such point.
pub fn next_on_side(
    grid: PageGridMap,
    outline: &[(i64, i64)],
    cps: &[BoxCpAt],
    at: (i64, i64),
) -> Option<(i64, i64)> {
    let n = outline.len();
    let side = (0..n)
        .map(|i| (outline[i], outline[(i + 1) % n]))
        .find(|(a, b)| super::black_box::on_outline(&[*a, *b], at))?;
    let (a, b) = side;
    let along_x = a.1 == b.1;
    let (lo, hi) = if along_x {
        (a.0.min(b.0), a.0.max(b.0))
    } else {
        (a.1.min(b.1), a.1.max(b.1))
    };
    let (k_lo, k_hi) = (grid.index(lo) + 1, grid.index(hi) - 1);
    let step = 1;
    let k = grid.index(if along_x { at.0 } else { at.1 });
    let point = |k: i64| {
        if along_x {
            (grid.coord(k), a.1)
        } else {
            (a.0, grid.coord(k))
        }
    };
    let free = |q: (i64, i64)| {
        !cps.iter()
            .any(|c| grid.index(c.1.0) == grid.index(q.0) && grid.index(c.1.1) == grid.index(q.1))
    };
    (1..=(k_hi - k_lo + 1))
        .flat_map(|m| [k + m * step, k - m * step])
        .filter(|k| (k_lo..=k_hi).contains(k))
        .map(point)
        .find(|q| free(*q))
}

impl Tool for PinNameTool {
    fn name(&self) -> &'static str {
        "box_pin"
    }

    fn state(&self) -> &'static str {
        if self.armed { "armed" } else { "typing" }
    }

    fn field_error(&self) -> Option<String> {
        self.error.clone()
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        match ev {
            CanvasEvent::PointerMove { at, .. } | CanvasEvent::CursorMove { at } if self.armed => {
                let p = cx.snapper.sheet_to_page(*at);
                if let Some(b) =
                    super::black_box::black_box_rep(cx, self.rep).and_then(|r| r.black_box.as_ref())
                {
                    let grid = PageGridMap::new(cx.page.tables.grid);
                    if let Some((q, dir)) =
                        super::black_box::pin_target(grid, &b.outline, &b.cps, self.pin.0, p)
                    {
                        self.pin = (self.pin.0, q, dir);
                    }
                }
                ToolOutput::Transient(self.overlay(cx))
            }
            CanvasEvent::Press { .. } | CanvasEvent::Activate if self.armed => {
                self.armed = false;
                ToolOutput::Transient(self.overlay(cx))
            }
            CanvasEvent::Text(text) => {
                self.name = text.clone();
                self.error = None;
                ToolOutput::Transient(self.overlay(cx))
            }
            CanvasEvent::Activate | CanvasEvent::Key(ToolKey::Finish) => self.commit(false, cx),
            CanvasEvent::Key(ToolKey::NextProposal) => self.commit(true, cx),
            CanvasEvent::Escape => {
                self.finished = true;
                ToolOutput::Cancel
            }
            CanvasEvent::PointerMove { .. } | CanvasEvent::CursorMove { .. } => {
                ToolOutput::Transient(self.overlay(cx))
            }
            _ => ToolOutput::None,
        }
    }

    fn on_commit(&mut self, done: bool) {
        if !done {
            self.next = false;
            return;
        }
        self.named = true;
        if !self.next {
            self.finished = true;
        }
    }

    fn status_hint(&self) -> super::Hint {
        if self.armed {
            super::Hint::plain(key!("hint-box-pin-armed"))
        } else if self.new && self.resume.is_none() {
            super::Hint::new(key!("hint-box-pin-add"), &["tool.next_proposal"])
        } else {
            super::Hint::plain(key!("hint-box-pin-name"))
        }
    }

    fn ghost(&self) -> Option<Ghost> {
        // Armed, nothing is typed yet: the status line alone says what a click does.
        if self.armed {
            return None;
        }
        Some(Ghost::Text {
            at_nm: Some(self.pin.1),
            text: self.name.clone(),
            bind: None,
        })
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

impl PinNameTool {
    /// After a Tab's commit: the next pin of the side, a free point and the smallest index the box does not use, with
    /// an empty name; finished where the side has no free point (the host says so). `cps` are the box's pins after the
    /// commit.
    pub fn go_next(
        &mut self,
        grid: PageGridMap,
        outline: &[(i64, i64)],
        cps: &[BoxCpAt],
    ) -> bool {
        if !self.next {
            return false;
        }
        self.next = false;
        let at = next_on_side(grid, outline, cps, self.pin.1);
        let idx = (0..=u16::MAX).find(|i| !cps.iter().any(|c| c.0 == *i));
        match (at, idx) {
            (Some(at), Some(idx)) => {
                self.pin = (idx, at, self.pin.2);
                self.name.clear();
                true
            }
            _ => {
                self.finished = true;
                false
            }
        }
    }
}
