// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The status-bar model (WP-S2.01 part 1, spec section 5.3; WP-S2.02 part 1, spec section 6.5; UX/UI doc 16
//! section 3.6).
//!
//! Left to right: the hint (the active tool's status hint, or the hovered link's target while the tool shows the
//! hover; WP-S2.03 §7.2), the page as `<folio> (<n> of <total>)`,
//! the cursor `102-H15 · x 255.0 y 172.5 mm` (the cell where one exists, then sheet coordinates: the pointer over
//! the focused canvas, else the grid cursor, else left out; WP-S2.03 §6.5, FT-345), the zoom `156 %`, the grid (`QET 10 px`, a pitch
//! such as `4 mm`, or `Grid off`), the save state (`*` unsaved, `saved`, or `read-only: <reason>`), the canvas
//! backend (`CPU`, `WARP` or `GPU <backend>`), the UI language, `Notifications <n>` while the notifications log is not
//! empty, and while a job runs the job field (WP-S2.09 part 1). Without a project the page and save fields are left
//! out; without a canvas the cursor, zoom and grid. These text fields stay the transcripts' `status` member.
//!
//! The window's status bar and the describe line follow WP-S2.17 D12 instead ([`bar`]): the state at the left, then
//! the findings, the job, the selection, the cursor, the grid, the zoom, the save state, the renderer and the
//! notifications, each with its emphasis.

use crate::canvas::{GridLabel, grid_label, mm1, zoom_percent_text};
use crate::shell::{AccessMode, ShellModel};
use crate::strings;

/// The grid field of `model`'s canvas: the label, or "Grid off" when it is off or the page has none.
pub fn grid_text(model: &ShellModel) -> String {
    let grid = model
        .canvas()
        .and_then(|c| c.page().grid)
        .filter(|_| model.grid_on());
    match grid.map(|g| grid_label(&g)) {
        None => strings::text("status-grid-off").to_owned(),
        Some(GridLabel::Qet10) => strings::text("status-grid-qet").to_owned(),
        Some(GridLabel::Mm(n)) => strings::format("status-grid-mm", &[&n]),
    }
}

/// The accessible texts of a canvas (WP-S2.03 §6.9, doc 16 §10.1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CanvasNodeText {
    /// The node's name: `Page <folio>, <title>`.
    pub name: String,
    /// Its description: `<hint>, zoom <zoom>`, then `, cursor <cell> x <x> y <y> mm` while the grid cursor shows.
    pub description: String,
    /// The polite live region's text: `<cell>, x <x> y <y> mm`, then `, link to <label>` when a link is at the
    /// cursor; empty while no cursor shows (the region stays present, so a screen reader keeps it).
    pub live: String,
}

/// The accessible texts of group `group`'s canvas; `None` without a canvas there.
pub fn canvas_node(model: &ShellModel, group: usize) -> Option<CanvasNodeText> {
    let c = model.canvas_of(group)?;
    let page = c.page();
    let mut description = strings::format(
        "canvas-description",
        &[
            &hint_of(model, c),
            &strings::format("status-zoom", &[&zoom_percent_text(c.zoom())]),
        ],
    );
    let live = c
        .cursor()
        .map(|(x, y)| {
            let cell = page.cell_text((x, y));
            description.push_str(&match &cell {
                Some(cell) => strings::format("canvas-cursor-description", &[cell, &mm1(x), &mm1(y)]),
                None => strings::format("canvas-cursor-description-no-cell", &[&mm1(x), &mm1(y)]),
            });
            let mut live = match &cell {
                Some(cell) => strings::format("canvas-cursor-live", &[cell, &mm1(x), &mm1(y)]),
                None => strings::format("status-cursor", &[&mm1(x), &mm1(y)]),
            };
            if let Some(label) = c.cursor_link().and_then(crate::canvas::PageLink::label) {
                live.push_str(&strings::format("canvas-cursor-live-link", &[&label]));
            }
            if let Some(cp) = c.cursor_cp() {
                live.push_str(&strings::format("canvas-cursor-live-cp", &[&cp.label]));
            }
            live
        })
        .unwrap_or_default();
    // A tool's state change or a selection change is said until the cursor moves again (WP-S2.03 §7.2, §7.6); it is
    // the focused group's.
    let focused = model.area().map(|a| a.focused) == Some(group);
    let live = match model.tool_announcement() {
        Some(text) if focused => text.to_owned(),
        _ => live,
    };
    Some(CanvasNodeText {
        name: strings::format("canvas-name", &[&page.folio, &page.title]),
        description,
        live,
    })
}

/// The hint of one canvas: the hovered link's target while the active tool shows the hover, else the tool's status
/// hint (WP-S2.03 §6.4, §7.2).
fn hint_of(model: &ShellModel, c: &crate::canvas::CanvasModel) -> String {
    match c.hovered_link().filter(|_| model.tool().wants_hover()) {
        Some(link) => match link.label() {
            Some(label) => strings::format("status-hint-link", &[&label]),
            None => strings::text("status-hint-link-external").to_owned(),
        },
        None => match model.tool_notice() {
            Some(notice) => notice.to_owned(),
            None => model.tool_hint().to_owned(),
        },
    }
}

/// The hint field: the active tool's status hint (`Click to select; drag for a rubber band`), or the hovered link's
/// `Click or F: go to <label>` (WP-S2.03 §6.4, §7.2).
pub fn hint(model: &ShellModel) -> String {
    match model.canvas() {
        Some(c) => hint_of(model, c),
        None => model.tool_hint().to_owned(),
    }
}

/// The status-bar fields of `model` (WP-S2.09 §6.4, §6.5): after the language, `Notifications <n>` while the log is
/// not empty, and last, while a job runs and `hide_job` is false, the job field: `PDF export <p> %` in the window,
/// `PDF export` headless (no progress number in a transcript, §9 item 2).
pub fn fields(model: &ShellModel, hide_job: bool) -> Vec<String> {
    let mut out = Vec::new();
    match model.project_name() {
        None => out.push(strings::text("status-hint-start").to_owned()),
        Some(_) => {
            out.push(hint(model));
            if let Some(folio) = model.current_folio() {
                out.push(strings::format(
                    "status-page",
                    &[
                        folio,
                        &(model.page_index() + 1).to_string(),
                        &model.page_count().to_string(),
                    ],
                ));
            }
            if let Some(canvas) = model.canvas() {
                if let Some((x, y)) = model.shown_point() {
                    out.push(match canvas.page().cell_text((x, y)) {
                        Some(cell) => strings::format("status-cursor-cell", &[&cell, &mm1(x), &mm1(y)]),
                        None => strings::format("status-cursor", &[&mm1(x), &mm1(y)]),
                    });
                }
                out.push(strings::format(
                    "status-zoom",
                    &[&zoom_percent_text(canvas.zoom())],
                ));
                out.push(grid_text(model));
            }
            out.push(match model.access() {
                Some(AccessMode::ReadOnly { reason_key }) => {
                    strings::format("status-read-only", &[strings::text(reason_key)])
                }
                _ if model.is_dirty() => strings::text("status-unsaved").to_owned(),
                _ => strings::text("status-saved").to_owned(),
            });
        }
    }
    out.push(model.canvas_backend().label());
    out.push(strings::text("status-language").to_owned());
    let log = model.notifications();
    if !log.is_empty() {
        out.push(strings::format("status-notifications", &[&log.len().to_string()]));
    }
    if !hide_job && let Some(job) = model.jobs().newest_running() {
        out.push(job_field(model, job));
    }
    out
}

// WP-S2.17 part 3 (D12; doc 16a §5.8)
/// What a field of the status bar shows (D12's fields, in their order).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum StatusKind {
    /// The state at the left: `Ready`.
    State,
    /// The findings of the open Messages pane, when any count is not zero; a click opens Messages.
    Findings,
    /// The running job, 200 ms after it started; a click opens Jobs.
    Job,
    /// The selection's size, when not empty.
    Selection,
    /// The pointer or the grid cursor: the cell, then the sheet coordinates.
    Cursor,
    /// The grid (`QET 10 px`, `4 mm`, `Grid off`).
    Grid,
    /// The zoom.
    Zoom,
    /// Saved, unsaved changes or read-only.
    Save,
    /// The canvas renderer.
    Renderer,
    /// The notifications log's size, when not empty; a click opens the log.
    Notifications,
}

impl StatusKind {
    /// The field's word (`state`, `findings`): the describe line's order and the window's node names are the enum's.
    pub fn word(self) -> &'static str {
        match self {
            StatusKind::State => "state",
            StatusKind::Findings => "findings",
            StatusKind::Job => "job",
            StatusKind::Selection => "selection",
            StatusKind::Cursor => "cursor",
            StatusKind::Grid => "grid",
            StatusKind::Zoom => "zoom",
            StatusKind::Save => "save",
            StatusKind::Renderer => "renderer",
            StatusKind::Notifications => "notifications",
        }
    }
}

/// How a field is emphasised (doc 16a §5.8): an exception stands out, the rest is quiet.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    /// Secondary text.
    Normal,
    /// The error colour, SemiBold (findings with an error).
    Error,
    /// The warning colour (findings with warnings only, read-only, a CPU or WARP renderer).
    Warning,
    /// The success colour (Saved).
    Success,
    /// Primary text (unsaved changes).
    Strong,
    /// Tertiary text (the GPU renderer).
    Muted,
}

/// One field of the status bar.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StatusField {
    /// Which field.
    pub kind: StatusKind,
    /// Its text.
    pub text: String,
    /// Its emphasis.
    pub tone: Tone,
    /// Its tooltip (why the canvas draws on the CPU, why the project is read-only); empty for none.
    pub tip: String,
}

/// The findings field's text and tone: `1 error · 2 warnings`, the worst severity's tone; `None` when every count is
/// zero. FT-710: the counts are [`crate::navigators::messages::MessageCounts::open`]'s (the project's open findings without the
/// pending and the waived ones, as the check's toast and the Messages tab's chip count them), and the pending ones
/// follow as their own part (`6 warnings · 2 pending on this page`), so leaving the page, which ends them, is
/// explained.
fn findings(model: &ShellModel) -> Option<(String, Tone)> {
    findings_text(model.messages_counts()?)
}

/// The findings field of counts `c` (see [`bar`]'s findings).
pub fn findings_text(c: crate::navigators::messages::MessageCounts) -> Option<(String, Tone)> {
    let part = |n: usize, one: &str, many: &str| match n {
        0 => None,
        1 => Some(strings::text(one).to_owned()),
        n => Some(strings::format(many, &[&n.to_string()])),
    };
    let parts: Vec<String> = [
        part(c.errors, "status-findings-error", "status-findings-errors"),
        part(c.warnings, "status-findings-warning", "status-findings-warnings"),
        part(c.infos, "status-findings-info", "status-findings-infos"),
        part(
            c.pending,
            "status-findings-pending-one",
            "status-findings-pending",
        ),
    ]
    .into_iter()
    .flatten()
    .collect();
    if parts.is_empty() {
        return None;
    }
    let tone = if c.errors > 0 {
        Tone::Error
    } else if c.warnings > 0 {
        Tone::Warning
    } else {
        Tone::Normal
    };
    Some((parts.join(" · "), tone))
}

/// The status bar of `model` (D12; doc 16a §5.8): the state at the left, then in this order the findings, the job,
/// the selection, the cursor, the grid, the zoom, the save state, the renderer and the notifications, each only when
/// it applies. The tool's hint is the HUD's (part 2); the page and the language fields left the bar. `hide_job` leaves
/// the job field out (a job younger than 200 ms).
pub fn bar(model: &ShellModel, hide_job: bool) -> Vec<StatusField> {
    let field = |kind, text: String, tone| StatusField {
        kind,
        text,
        tone,
        tip: String::new(),
    };
    let mut out = vec![field(
        StatusKind::State,
        strings::text("status-ready").to_owned(),
        Tone::Normal,
    )];
    if model.project_name().is_some() {
        if let Some((text, tone)) = findings(model) {
            out.push(field(StatusKind::Findings, text, tone));
        }
        if !hide_job && let Some(job) = model.jobs().newest_running() {
            out.push(field(StatusKind::Job, job_field(model, job), Tone::Normal));
        }
        let selected = model.selection().items.len();
        if selected > 0 {
            out.push(field(
                StatusKind::Selection,
                strings::format("status-selected", &[&selected.to_string()]),
                Tone::Normal,
            ));
        }
        if let Some(canvas) = model.canvas() {
            if let Some((x, y)) = model.shown_point() {
                let text = match canvas.page().cell_text((x, y)) {
                    Some(cell) => strings::format("status-cursor-cell", &[&cell, &mm1(x), &mm1(y)]),
                    None => strings::format("status-cursor", &[&mm1(x), &mm1(y)]),
                };
                out.push(field(StatusKind::Cursor, text, Tone::Normal));
            }
            out.push(field(StatusKind::Grid, grid_text(model), Tone::Normal));
            out.push(field(
                StatusKind::Zoom,
                strings::format("status-zoom", &[&zoom_percent_text(canvas.zoom())]),
                Tone::Normal,
            ));
        }
        out.push(match model.access() {
            Some(AccessMode::ReadOnly { reason_key }) => StatusField {
                tip: strings::text(reason_key).to_owned(),
                ..field(
                    StatusKind::Save,
                    strings::text("status-bar-read-only").to_owned(),
                    Tone::Warning,
                )
            },
            _ if model.is_dirty() => field(
                StatusKind::Save,
                strings::text("status-bar-unsaved").to_owned(),
                Tone::Strong,
            ),
            _ => field(
                StatusKind::Save,
                strings::text("status-bar-saved").to_owned(),
                Tone::Success,
            ),
        });
    }
    let renderer = model.canvas_backend().label();
    let tone = if renderer.starts_with("GPU") {
        Tone::Muted
    } else {
        Tone::Warning
    };
    let tip = if tone == Tone::Warning {
        strings::text("status-renderer-why").to_owned()
    } else {
        String::new()
    };
    out.push(StatusField {
        tip,
        ..field(StatusKind::Renderer, renderer, tone)
    });
    let log = model.notifications();
    if !log.is_empty() {
        out.push(field(
            StatusKind::Notifications,
            strings::format("status-notifications", &[&log.len().to_string()]),
            Tone::Normal,
        ));
    }
    out
}

/// The job field of a running job: its kind's label and, in the window, the per cent done (`done × 100 / total`,
/// integer; the phase text while indeterminate).
pub fn job_field(model: &ShellModel, job: &crate::jobs::JobEntry) -> String {
    let label = strings::text(job.kind.label_key());
    if model.mode() == crate::shell::Mode::Headless {
        return label.to_owned();
    }
    match job.progress() {
        Some(p) if p.total > 0 => {
            let pct = u64::from(p.done) * 100 / u64::from(p.total);
            strings::format("status-job", &[label, &pct.to_string()])
        }
        Some(p) => strings::format("status-job-phase", &[label, &phase_text(p.phase)]),
        None => label.to_owned(),
    }
}

/// The phase word of a job's progress as text (`building` reads "building pages").
pub fn phase_text(phase: &str) -> String {
    match phase {
        "building" => strings::text("job-phase-building").to_owned(),
        "drawing" => strings::text("job-phase-drawing").to_owned(),
        "writing" => strings::text("job-phase-writing").to_owned(),
        other => other.to_owned(),
    }
}
