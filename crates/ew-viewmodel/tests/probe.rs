// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool host and the preview protocol with tools that exist only here (WP-S2.03 part 2, spec §7.6; acceptance
//! checks T1, T2, T3, the probe's part of T9 and the preview words of T10). Built only with the dev-only feature
//! `testkit` (`required-features`), which `ShellModel::set_tool_for_tests` needs; `ci/dep-rules.toml` bans the feature
//! on every non-dev edge, so the binaries never contain the seam.
//!
//! **The probe** (§7.6): on a drag of a selected representation it shows the ghost of the selected entities moved by
//! the snapped grid offset, on `Dwell(80)` it previews `rep.move` by that offset, on release it commits the same
//! command, on Escape it cancels. WP-S2.04's move gesture follows its pattern.

mod common;

use std::cell::RefCell;
use std::rc::Rc;

use common::{Tmp, at, click, done, entity_points, state};
use ew_viewmodel::ew_api::AnyCommand;
use ew_viewmodel::ew_api::output::ew_dl::ew_units::Rect;
use ew_viewmodel::ew_api::output::ew_dl::{Destination, EntityRef};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use ew_viewmodel::tools::{
    Button, CanvasEvent, Overlay, OverlayItem, SelectionChange, Style, Token, Tool, ToolCx, ToolOutput,
};
use serde_json::{Value, json};

/// The probe's state.
#[derive(Clone, Debug)]
enum ProbeState {
    Idle,
    Dragging {
        from: (i64, i64),
        to: (i64, i64),
        rects: Vec<Rect>,
        reps: Vec<EntityRef>,
    },
}

/// The probe tool of spec §7.6.
struct Probe {
    state: ProbeState,
}

impl Probe {
    fn new() -> Probe {
        Probe {
            state: ProbeState::Idle,
        }
    }

    /// The grid offset of the drag: the sheet offset of the ghost and the page delta of `rep.move`, each the snapped
    /// grid points' difference in grid steps (`round(dk × g)` on the page).
    fn offset(cx: &ToolCx<'_>, from: (i64, i64), to: (i64, i64)) -> ((i64, i64), (i64, i64)) {
        let (a, b) = (cx.snapper.snap_grid(from), cx.snapper.snap_grid(to));
        let dk = (b.grid_index.0 - a.grid_index.0, b.grid_index.1 - a.grid_index.1);
        (
            (b.at_nm.0 - a.at_nm.0, b.at_nm.1 - a.at_nm.1),
            (cx.snapper.page_of_index(dk.0), cx.snapper.page_of_index(dk.1)),
        )
    }

    fn command(reps: &[EntityRef], delta: (i64, i64)) -> Box<AnyCommand> {
        Box::new(AnyCommand {
            name: "rep.move".into(),
            params: json!({
                "reps": reps.iter().map(|e| e.0.to_string()).collect::<Vec<_>>(),
                "delta": [delta.0, delta.1],
            }),
        })
    }
}

impl Tool for Probe {
    fn name(&self) -> &'static str {
        "probe"
    }

    fn state(&self) -> &'static str {
        match self.state {
            ProbeState::Idle => "idle",
            ProbeState::Dragging { .. } => "pressed",
        }
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        match (ev, &self.state) {
            (
                CanvasEvent::Press {
                    at,
                    button: Button::Primary,
                    ..
                },
                ProbeState::Idle,
            ) => match cx.entity_at(*at) {
                Some(e) if cx.selection.contains(e) => {
                    let reps = cx.selection.items.clone();
                    let rects = cx
                        .page
                        .entities
                        .iter()
                        .filter(|(x, _)| reps.contains(x))
                        .map(|(_, r)| *r)
                        .collect();
                    self.state = ProbeState::Dragging {
                        from: *at,
                        to: *at,
                        rects,
                        reps,
                    };
                    ToolOutput::None
                }
                _ => ToolOutput::None,
            },
            (
                CanvasEvent::PointerMove { at, .. },
                ProbeState::Dragging {
                    from, rects, reps, ..
                },
            ) => {
                let (sheet, _) = Probe::offset(cx, *from, *at);
                let rects = rects.clone();
                self.state = ProbeState::Dragging {
                    from: *from,
                    to: *at,
                    rects: rects.clone(),
                    reps: reps.clone(),
                };
                ToolOutput::Transient(Overlay::new(vec![OverlayItem::Ghost { rects, offset: sheet }]))
            }
            (CanvasEvent::Dwell { ms }, ProbeState::Dragging { from, to, reps, .. }) if *ms >= 80 => {
                let (_, delta) = Probe::offset(cx, *from, *to);
                if delta == (0, 0) {
                    ToolOutput::None
                } else {
                    ToolOutput::Preview(Probe::command(reps, delta))
                }
            }
            (CanvasEvent::Release { at, .. }, ProbeState::Dragging { from, reps, .. }) => {
                let (_, delta) = Probe::offset(cx, *from, *at);
                let reps = reps.clone();
                self.state = ProbeState::Idle;
                if delta == (0, 0) {
                    ToolOutput::Cancel
                } else {
                    ToolOutput::Commit(Probe::command(&reps, delta))
                }
            }
            (CanvasEvent::Escape, ProbeState::Dragging { .. }) => {
                self.state = ProbeState::Idle;
                ToolOutput::Cancel
            }
            _ => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> &'static str {
        "hint-select"
    }

    fn wants_hover(&self) -> bool {
        false
    }
}

/// A tool that answers every event with the next output its script holds (`None` when empty) and records the
/// events it got.
struct Scripted {
    outputs: Rc<RefCell<Vec<ToolOutput>>>,
    events: Rc<RefCell<Vec<CanvasEvent>>>,
    busy: Rc<RefCell<bool>>,
}

impl Tool for Scripted {
    fn name(&self) -> &'static str {
        "scripted"
    }

    fn state(&self) -> &'static str {
        if *self.busy.borrow() { "pressed" } else { "idle" }
    }

    fn on_event(&mut self, ev: &CanvasEvent, _: &ToolCx<'_>) -> ToolOutput {
        self.events.borrow_mut().push(ev.clone());
        if *ev == CanvasEvent::Escape {
            let was = std::mem::replace(&mut *self.busy.borrow_mut(), false);
            return if was { ToolOutput::Cancel } else { ToolOutput::None };
        }
        let mut outputs = self.outputs.borrow_mut();
        if outputs.is_empty() {
            ToolOutput::None
        } else {
            outputs.remove(0)
        }
    }

    fn status_hint(&self) -> &'static str {
        "hint-select"
    }

    fn wants_hover(&self) -> bool {
        true
    }
}

struct Handles {
    outputs: Rc<RefCell<Vec<ToolOutput>>>,
    events: Rc<RefCell<Vec<CanvasEvent>>>,
    busy: Rc<RefCell<bool>>,
}

fn scripted(shell: &mut ShellModel) -> Handles {
    let h = Handles {
        outputs: Rc::new(RefCell::new(Vec::new())),
        events: Rc::new(RefCell::new(Vec::new())),
        busy: Rc::new(RefCell::new(false)),
    };
    shell.set_tool_for_tests(Box::new(Scripted {
        outputs: Rc::clone(&h.outputs),
        events: Rc::clone(&h.events),
        busy: Rc::clone(&h.busy),
    }));
    h.events.borrow_mut().clear();
    h
}

/// Selects a representation of folio 102 whose drag the probe can preview (a `rep.move` of it is accepted), and
/// returns its point; the probe is then the active tool.
fn select_a_representation(shell: &mut ShellModel) -> (i64, i64) {
    let points = entity_points(shell.canvas().unwrap());
    for (_, p) in points.into_iter().take(40) {
        done(shell, "tool.select", json!({}));
        click(shell, p, json!({}));
        shell.set_tool_for_tests(Box::new(Probe::new()));
        done(shell, "canvas.press", at(p, json!({})));
        done(shell, "canvas.pointer", at((p.0 + 10_000_000, p.1), json!({})));
        done(shell, "canvas.dwell", json!({"ms": 80}));
        let ready = state(shell)["canvas"]["preview"]["state"] == "ready";
        done(shell, "ui.dismiss", json!({}));
        if ready {
            return p;
        }
    }
    panic!("no representation on 102 the probe could move");
}

#[test]
fn t1_every_output_of_the_host() {
    let tmp = Tmp::new("probe-t1");
    let mut shell = tmp.shell_in(Mode::Headless, true, "101");
    let h = scripted(&mut shell);
    let p = (100_000_000, 100_000_000);
    let poke = |shell: &mut ShellModel| shell.dispatch("canvas.press", &at(p, json!({})));
    // None: nothing.
    assert_eq!(poke(&mut shell), Outcome::Done);
    assert!(shell.tool_overlay().is_empty());
    // Transient: the overlay.
    let band = Overlay::new(vec![OverlayItem::Line {
        a: (0, 0),
        b: (10, 10),
        style: Style::solid(Token::Measure),
    }]);
    h.outputs.borrow_mut().push(ToolOutput::Transient(band.clone()));
    poke(&mut shell);
    assert_eq!(shell.tool_overlay(), &band);
    // Cancel: the overlay and the preview go; the revision stays.
    let rev = shell.revision();
    h.outputs.borrow_mut().push(ToolOutput::Cancel);
    poke(&mut shell);
    assert!(shell.tool_overlay().is_empty());
    assert_eq!(shell.revision(), rev);
    // Select: the selection changes and the overlay goes.
    let e = shell.canvas().unwrap().page().entities[0].0;
    h.outputs.borrow_mut().push(ToolOutput::Transient(band.clone()));
    h.outputs
        .borrow_mut()
        .push(ToolOutput::Select(SelectionChange::Replace(vec![e])));
    poke(&mut shell);
    poke(&mut shell);
    assert_eq!(shell.selection().items, vec![e]);
    assert!(shell.tool_overlay().is_empty());
    // Follow: the link's jump.
    let link = shell
        .canvas()
        .unwrap()
        .page()
        .links
        .iter()
        .find(|l| l.target.as_ref().is_some_and(|t| t.folio != "101"))
        .unwrap()
        .clone();
    h.outputs.borrow_mut().push(ToolOutput::Follow(link.dest));
    poke(&mut shell);
    assert_eq!(shell.current_folio(), Some(link.target.unwrap().folio.as_str()));
    done(&mut shell, "nav.back", json!({}));
    // An unknown destination follows nothing.
    h.outputs
        .borrow_mut()
        .push(ToolOutput::Follow(Destination::Page(Default::default())));
    assert!(matches!(
        poke(&mut shell),
        Outcome::Refused {
            reason_key: "reason-no-link"
        }
    ));
    // Preview: one `preview_with`, the state kept, a repeat ignored; Commit: a new revision headless through
    // `run_script`, and the overlay and preview go.
    let calls = shell.session_calls();
    let cmd = || {
        Box::new(AnyCommand {
            name: "page.create".into(),
            params: json!({"number": 777, "title": "Probe"}),
        })
    };
    h.outputs.borrow_mut().push(ToolOutput::Preview(cmd()));
    h.outputs.borrow_mut().push(ToolOutput::Preview(cmd()));
    poke(&mut shell);
    poke(&mut shell);
    assert_eq!(shell.session_calls().preview_with, calls.preview_with + 1);
    assert_eq!(state(&shell)["canvas"]["preview"]["command"], "page.create");
    assert_eq!(state(&shell)["canvas"]["preview"]["state"], "ready");
    let rev = shell.revision().unwrap();
    h.outputs.borrow_mut().push(ToolOutput::Commit(cmd()));
    assert_eq!(poke(&mut shell), Outcome::Done);
    assert_eq!(shell.revision(), Some(rev + 1));
    assert_eq!(shell.session_calls().run_script, calls.run_script + 1);
    assert!(shell.preview_state().is_none());
    assert!(shell.page_labels().iter().any(|f| f == "777"));
    shell.shutdown();
}

#[test]
fn t1_activation_focus_and_the_escape_chain() {
    let tmp = Tmp::new("probe-t1b");
    let mut shell = tmp.shell("101");
    // Activating a tool cancels the busy one.
    let h = scripted(&mut shell);
    *h.busy.borrow_mut() = true;
    done(&mut shell, "tool.measure", json!({}));
    assert_eq!(h.events.borrow().last(), Some(&CanvasEvent::Escape));
    assert_eq!(shell.tool().name(), "measure");
    // Focusing the other group escapes a busy tool.
    done(&mut shell, "view.side_by_side", json!({}));
    let h = scripted(&mut shell);
    *h.busy.borrow_mut() = true;
    done(&mut shell, "ui.focus", json!({"area": "canvas", "group": 0}));
    assert_eq!(h.events.borrow().as_slice(), &[CanvasEvent::Escape]);
    assert!(!*h.busy.borrow());
    // A press in the other group focuses it first (and escapes), then reaches the tool.
    *h.busy.borrow_mut() = true;
    h.events.borrow_mut().clear();
    done(
        &mut shell,
        "canvas.press",
        at((50_000_000, 50_000_000), json!({"group": 1})),
    );
    assert_eq!(h.events.borrow()[0], CanvasEvent::Escape);
    assert!(matches!(h.events.borrow()[1], CanvasEvent::Press { .. }));
    assert_eq!(shell.area().unwrap().focused, 1);
    // A tab switch escapes too.
    *h.busy.borrow_mut() = true;
    h.events.borrow_mut().clear();
    done(&mut shell, "tab.new", json!({}));
    assert_eq!(h.events.borrow().as_slice(), &[CanvasEvent::Escape]);
    // The Escape chain: a dialog first, then the link preview, then the tool, then Select, then the selection.
    done(&mut shell, "view.side_by_side", json!({}));
    let h = scripted(&mut shell);
    *h.busy.borrow_mut() = true;
    done(&mut shell, "help.about", json!({}));
    done(&mut shell, "ui.dismiss", json!({}));
    assert!(shell.dialog().is_none());
    assert!(h.events.borrow().is_empty(), "the dialog took Escape");
    let link = ew_viewmodel::canvas::rect_centre(shell.canvas().unwrap().page().links[0].rect);
    done(&mut shell, "canvas.pointer", at(link, json!({})));
    done(&mut shell, "canvas.dwell", json!({"ms": 300}));
    assert!(shell.link_preview().is_some());
    h.events.borrow_mut().clear();
    done(&mut shell, "ui.dismiss", json!({}));
    assert!(shell.link_preview().is_none());
    assert!(h.events.borrow().is_empty(), "the popup took Escape");
    done(&mut shell, "ui.dismiss", json!({}));
    assert_eq!(h.events.borrow().as_slice(), &[CanvasEvent::Escape]);
    assert_eq!(shell.tool().name(), "scripted", "a busy tool cancels and stays");
    done(&mut shell, "ui.dismiss", json!({}));
    assert_eq!(shell.tool().name(), "select", "an idle tool gives way to Select");
    let p = entity_points(shell.canvas().unwrap())[0].1;
    click(&mut shell, p, json!({}));
    assert!(!shell.selection().is_empty());
    done(&mut shell, "ui.dismiss", json!({}));
    assert!(
        shell.selection().is_empty(),
        "Select's Escape clears the selection"
    );
    shell.shutdown();
}

#[test]
fn t2_the_preview_protocol_headless() {
    let tmp = Tmp::new("probe-t2");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    let p = select_a_representation(&mut shell);
    let rev = shell.revision().unwrap();
    let calls = shell.session_calls();
    // The drag: the ghost, one preview per dwell that changes the command.
    done(&mut shell, "canvas.press", at(p, json!({})));
    let to = (p.0 + 10_000_000, p.1 + 5_000_000);
    done(&mut shell, "canvas.pointer", at(to, json!({})));
    assert!(matches!(shell.tool_overlay().items[0], OverlayItem::Ghost { .. }));
    done(&mut shell, "canvas.dwell", json!({"ms": 80}));
    assert_eq!(shell.session_calls().preview_with, calls.preview_with + 1);
    done(&mut shell, "canvas.dwell", json!({"ms": 80}));
    done(&mut shell, "canvas.dwell", json!({"ms": 300}));
    assert_eq!(
        shell.session_calls().preview_with,
        calls.preview_with + 1,
        "a repeat dwell runs none"
    );
    let preview = state(&shell)["canvas"]["preview"].clone();
    assert_eq!(preview["command"], "rep.move");
    assert_eq!(preview["state"], "ready");
    assert!(preview["tags"].is_u64() && preview["xrefs"].is_u64());
    let derived = shell.preview_state().unwrap().derived.clone();
    assert_eq!(preview["xrefs"], derived.xrefs.len());
    assert!(
        shell
            .describe()
            .contains("canvas preview ready \"rep.move\" tags ")
    );
    assert!(!shell.tool_overlay().is_empty(), "a preview keeps the overlay");
    // A commit by another script step makes it stale; the next dwell recomputes it.
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "page.create", "params": {"number": 778, "title": "Other"}}),
    );
    assert_eq!(state(&shell)["canvas"]["preview"]["state"], "stale");
    assert!(shell.describe().contains("canvas preview stale \"rep.move\""));
    done(&mut shell, "canvas.dwell", json!({"ms": 80}));
    assert_eq!(shell.session_calls().preview_with, calls.preview_with + 2);
    assert_eq!(state(&shell)["canvas"]["preview"]["state"], "ready");
    // Release commits through `run_script` (origin CmdLog): revision + 1, the record holds the line.
    let before = shell.revision().unwrap();
    assert_eq!(before, rev + 1);
    done(&mut shell, "canvas.release", at(to, json!({})));
    assert_eq!(shell.revision(), Some(before + 1));
    assert_eq!(shell.session_calls().run_script, calls.run_script + 2);
    assert_eq!(
        shell.session_calls().commit,
        0,
        "headless never commits a preview"
    );
    let last = shell.recorded_commands().last().unwrap();
    assert_eq!(last.cmd, "rep.move");
    assert!(ew_viewmodel::script::record_log(&shell).contains("\"cmd\":\"rep.move\""));
    assert!(shell.preview_state().is_none() && shell.tool_overlay().is_empty());
    // The page moved; undo restores it.
    let moved: Vec<Rect> = shell.selected_rects(0);
    done(&mut shell, "edit.undo", json!({}));
    let back: Vec<Rect> = shell.selected_rects(0);
    assert_ne!(moved, back);
    done(&mut shell, "edit.undo", json!({}));
    assert!(!shell.is_dirty());
    shell.shutdown();
}

#[test]
fn t2_the_window_commits_the_fresh_preview() {
    let tmp = Tmp::new("probe-t2w");
    let mut shell = tmp.shell_in(Mode::Gui, true, "102");
    let p = select_a_representation(&mut shell);
    let calls = shell.session_calls();
    let rev = shell.revision().unwrap();
    done(&mut shell, "canvas.press", at(p, json!({})));
    let to = (p.0 + 10_000_000, p.1);
    done(&mut shell, "canvas.pointer", at(to, json!({})));
    done(&mut shell, "canvas.dwell", json!({"ms": 80}));
    done(&mut shell, "canvas.release", at(to, json!({})));
    assert_eq!(shell.revision(), Some(rev + 1));
    let after = shell.session_calls();
    assert_eq!(
        after.commit,
        calls.commit + 1,
        "Session::commit of the fresh preview"
    );
    assert_eq!(after.execute_with, calls.execute_with);
    assert_eq!(after.preview_with, calls.preview_with + 1);
    // Without a dwell (no fresh preview) the window executes the command.
    done(
        &mut shell,
        "canvas.press",
        at((to.0 + 10_000_000, to.1), json!({})),
    );
    let _ = shell.selection();
    done(&mut shell, "edit.undo", json!({}));
    let p2 = select_a_representation(&mut shell);
    let calls = shell.session_calls();
    done(&mut shell, "canvas.press", at(p2, json!({})));
    done(
        &mut shell,
        "canvas.pointer",
        at((p2.0, p2.1 + 8_000_000), json!({})),
    );
    done(
        &mut shell,
        "canvas.release",
        at((p2.0, p2.1 + 8_000_000), json!({})),
    );
    assert_eq!(shell.session_calls().execute_with, calls.execute_with + 1);
    assert_eq!(shell.session_calls().commit, calls.commit);
    done(&mut shell, "edit.undo", json!({}));
    shell.shutdown();
}

#[test]
fn t2_a_human_only_command_is_refused_headless() {
    let tmp = Tmp::new("probe-t2h");
    let mut shell = tmp.shell_in(Mode::Headless, true, "101");
    let h = scripted(&mut shell);
    let rev = shell.revision();
    h.outputs
        .borrow_mut()
        .push(ToolOutput::Commit(Box::new(AnyCommand {
            name: "project.checks.set".into(),
            params: json!({"release_scheme": "full"}),
        })));
    let out = shell.dispatch("canvas.press", &at((100_000_000, 100_000_000), json!({})));
    let Outcome::Failed { message } = out else {
        panic!("{out:?}")
    };
    assert!(message.contains("project.checks.set"), "{message}");
    assert_eq!(shell.revision(), rev);
    assert!(shell.recorded_commands().is_empty());
    shell.shutdown();
}

#[test]
fn t3_escape_after_a_drag_changes_nothing() {
    let tmp = Tmp::new("probe-t3");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    let p = select_a_representation(&mut shell);
    let (rev, undo, dirty) = (shell.revision(), shell.can_undo(), shell.is_dirty());
    done(&mut shell, "canvas.press", at(p, json!({})));
    let to = (p.0 + 15_000_000, p.1 + 7_000_000);
    done(&mut shell, "canvas.pointer", at(to, json!({})));
    done(&mut shell, "canvas.dwell", json!({"ms": 80}));
    assert!(shell.preview_state().is_some());
    done(&mut shell, "ui.dismiss", json!({}));
    assert_eq!(shell.tool().state(), "idle");
    done(&mut shell, "canvas.release", at(to, json!({})));
    assert_eq!(
        (shell.revision(), shell.can_undo(), shell.is_dirty()),
        (rev, undo, dirty)
    );
    assert!(shell.preview_state().is_none());
    assert!(shell.tool_overlay().is_empty());
    assert_eq!(state(&shell)["canvas"]["preview"], Value::Null);
    assert_eq!(state(&shell)["canvas"]["overlay"], 0);
    shell.shutdown();
}

#[test]
fn t9_the_probe_cancels_with_escape_in_every_state_and_t10_the_preview_words() {
    let tmp = Tmp::new("probe-t9");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    let p = select_a_representation(&mut shell);
    let rev = shell.revision();
    // idle: None (the host replaces it with Select); pressed: Cancel.
    done(&mut shell, "canvas.press", at(p, json!({})));
    assert_eq!(shell.tool().state(), "pressed");
    done(&mut shell, "ui.dismiss", json!({}));
    assert_eq!(shell.tool().state(), "idle");
    done(&mut shell, "ui.dismiss", json!({}));
    assert_eq!(shell.tool().name(), "select");
    assert_eq!(shell.revision(), rev);
    // The preview words of the describe text: ready, stale, failed, tags, xrefs.
    let mut texts = Vec::new();
    shell.set_tool_for_tests(Box::new(Probe::new()));
    done(&mut shell, "canvas.press", at(p, json!({})));
    done(
        &mut shell,
        "canvas.pointer",
        at((p.0 + 10_000_000, p.1), json!({})),
    );
    done(&mut shell, "canvas.dwell", json!({"ms": 80}));
    texts.push(shell.describe());
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "page.create", "params": {"number": 779}}),
    );
    texts.push(shell.describe());
    done(&mut shell, "ui.dismiss", json!({}));
    let h = scripted(&mut shell);
    h.outputs
        .borrow_mut()
        .push(ToolOutput::Preview(Box::new(AnyCommand {
            name: "rep.move".into(),
            params: json!({"reps": ["00000000-0000-0000-0000-000000000001"], "delta": [0, 0]}),
        })));
    done(&mut shell, "canvas.dwell", json!({"ms": 80}));
    assert_eq!(state(&shell)["canvas"]["preview"]["state"], "failed");
    texts.push(shell.describe());
    // FT-422: the failed preview's message names the missing record by its alias, never its UUID.
    let failed = texts
        .last()
        .unwrap()
        .lines()
        .find(|l| l.starts_with("canvas preview failed "))
        .unwrap()
        .to_owned();
    assert_eq!(ew_viewmodel::alias::find_uuid(&failed), None, "{failed}");
    let n = shell
        .alias_number("00000000-0000-0000-0000-000000000001")
        .expect("the record's ID was printed");
    assert!(failed.contains(&format!("@{n}")), "{failed}");
    let words: std::collections::BTreeSet<&str> = texts
        .iter()
        .flat_map(|t| t.lines())
        .filter(|l| l.starts_with("canvas preview "))
        .flat_map(|l| l.split_whitespace())
        .filter(|w| !w.starts_with('"') && w.parse::<u64>().is_err())
        .collect();
    for w in ["ready", "stale", "failed", "tags", "xrefs"] {
        assert!(words.contains(w), "{w}: {words:?}");
        assert!(ew_viewmodel::describe::VOCABULARY.contains(&w));
    }
    shell.shutdown();
}
