// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S3.07 part 3 (D10, rulings 2 to 5 of ID-1180): the print pre-dialog and its print job.
//!
//! **The dialog.** `file.print` (Output ▸ Print…, Ctrl+P) opens [`Dialog::Print`] with its options, which its own
//! parameters may set at once: the Range (All, Current page, Sections with their check list, Pages with a text field),
//! the Paper (Sheet or a size), Scale to fit and Monochrome. `print.set` changes options, `print.page` moves the
//! preview, and `print.send` hands the built set to the window ([`ShellModel::take_print_request`]): Print… through
//! the system's way to print, Open in PDF viewer through the viewer. Escape and Cancel cancel a running build and
//! close the dialog, printing nothing.
//!
//! **The job.** Each option change cancels the running build and starts one for the new options: the print set of
//! the current snapshot ([`ew_api::Session::print_set_job`], [`crate::jobs::JobKind::Print`]) on its own thread, so
//! the window never waits on it and an edit made meanwhile is applied at once; the set prints the revision it was
//! built at. The window polls it as it polls the export jobs ([`ShellModel::poll_jobs`]); the headless mode waits for
//! it unless a step says `wait: false`. With no page chosen (no section ticked, an empty Pages field) no job runs:
//! the preview shows the empty state. The build writes no file, so a cancel leaves none.
//!
//! **Printing.** The window hands the request to its platform, which prints on a worker thread and reports a
//! [`PrintEnd`]; [`ShellModel::print_done`] shows its one toast (printed, D11's fallback notice, or the failure).

use std::sync::Arc;

use ew_api::ew_jobs::{Job, JobEnd, JobOptions, Progress};
use ew_api::output::PageSelection;
use ew_api::output::print::{Fit, Paper, PrintOptions, PrintSet};
use serde_json::{Map, Value};

use super::jobs::bool_param;
use super::{Dialog, Mode, Outcome, ShellModel, failed, string_param};
use crate::jobs::JobKind;
use crate::key;
use crate::strings;
use crate::strings::Key;

/// The Range of the pre-dialog (ruling 4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PrintRange {
    /// Every page.
    All,
    /// The current page (`PageSelection::Folios` of its folio, ruling 2).
    Current,
    /// The ticked sections, each with its divider.
    Sections,
    /// The pages typed in the field, as `--pages` takes them.
    Pages,
}

impl PrintRange {
    /// Every range, in the radio group's order.
    pub const ALL: [PrintRange; 4] = [
        PrintRange::All,
        PrintRange::Current,
        PrintRange::Sections,
        PrintRange::Pages,
    ];

    /// The word `print.set {range}` takes and the describe text prints.
    pub fn word(self) -> &'static str {
        match self {
            PrintRange::All => "all",
            PrintRange::Current => "current",
            PrintRange::Sections => "sections",
            PrintRange::Pages => "pages",
        }
    }

    /// The radio button's label.
    pub fn label_key(self) -> Key<'static> {
        match self {
            PrintRange::All => key!("print-range-all"),
            PrintRange::Current => key!("print-range-current"),
            PrintRange::Sections => key!("print-range-sections"),
            PrintRange::Pages => key!("print-range-pages"),
        }
    }

    fn parse(word: &str) -> Option<PrintRange> {
        PrintRange::ALL.into_iter().find(|r| r.word() == word)
    }
}

/// One row of the Sections check list.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PrintSection {
    /// The section's number (`1`).
    pub number: u16,
    /// Its title.
    pub title: String,
    /// Whether it is ticked.
    pub ticked: bool,
}

impl PrintSection {
    /// The row's label: "Section 1: Wiring Definition".
    pub fn label(&self) -> String {
        strings::format(
            key!("print-section-row"),
            &[&self.number.to_string(), &self.title],
        )
    }
}

/// Where the pre-dialog's print set stands.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PrintBuild {
    /// Its job runs ([`ShellModel::print_progress`]).
    Building,
    /// It is built ([`ShellModel::print_set`]).
    Built {
        /// Its pages.
        pages: usize,
    },
    /// No page is chosen: §6.17's empty state.
    Empty,
    /// The build failed, with the cause (no path).
    Failed(String),
}

/// The print pre-dialog's state (D10, ruling 4).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PrintDialog {
    /// The Range.
    pub range: PrintRange,
    /// The project's sections, by number, with their ticks.
    pub sections: Vec<PrintSection>,
    /// The Pages field's text.
    pub pages: String,
    /// The refusal of the Pages field's text, shown under the field.
    pub pages_error: Option<String>,
    /// The current page's folio when the dialog opened.
    pub current: Option<String>,
    /// The Paper.
    pub paper: Paper,
    /// Scale to fit.
    pub fit: bool,
    /// Monochrome.
    pub monochrome: bool,
    /// The preview's page (0-based) in the built set.
    pub page: usize,
    /// Where the set stands.
    pub build: PrintBuild,
    /// The number of the build the state shows; the window's preview image is tagged with it.
    pub generation: u64,
}

impl PrintDialog {
    /// The selection of the options; `None` when no page is chosen.
    fn selection(&self) -> Option<PageSelection> {
        match self.range {
            PrintRange::All => Some(PageSelection::All),
            PrintRange::Current => self
                .current
                .as_ref()
                .map(|f| PageSelection::Folios(vec![f.clone()])),
            PrintRange::Sections => {
                let ticked: Vec<String> = self
                    .sections
                    .iter()
                    .filter(|s| s.ticked)
                    .map(|s| s.number.to_string())
                    .collect();
                (!ticked.is_empty()).then_some(PageSelection::Sections(ticked))
            }
            PrintRange::Pages if self.pages.trim().is_empty() => None,
            PrintRange::Pages => PageSelection::parse(&self.pages).ok(),
        }
    }

    /// The print options of the dialog for `selection`.
    fn options(&self, selection: PageSelection) -> PrintOptions {
        PrintOptions {
            pages: selection,
            paper: self.paper,
            fit: if self.fit { Fit::ToPaper } else { Fit::Actual },
            monochrome: self.monochrome,
            ..PrintOptions::default()
        }
    }

    /// The Paper choice's words: "Sheet size" or the paper's name.
    pub fn paper_text(paper: Paper) -> String {
        match paper {
            Paper::Sheet => strings::text(key!("print-paper-sheet")).to_owned(),
            other => other.name().to_owned(),
        }
    }
}

/// How the window is to print a built set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PrintVia {
    /// Print…: the system's way to print (on Linux the portal, then `lp`, then the viewer).
    Printer,
    /// Open in PDF viewer.
    Viewer,
}

/// A built print set for the window to print (ruling 3), taken once.
#[derive(Clone, Debug)]
pub struct PrintRequest {
    /// The set: its PDF and its placed pages.
    pub set: Arc<PrintSet>,
    /// The job's title in the spooler and the viewer's file stem: the project's name.
    pub title: String,
    /// Print… or Open in PDF viewer.
    pub via: PrintVia,
    /// Whether the set is drawn in black and grey.
    pub monochrome: bool,
}

/// How a print the window started ended (the platform's report, ruling 3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PrintEnd {
    /// Sent to a printer (the system's print dialog, or `lp`).
    Printed,
    /// Opened in the PDF viewer.
    Opened,
    /// Sent to the default printer through `lp` after the system's print dialog failed (D11's notice).
    FellBackToPrinter,
    /// Opened in the PDF viewer after the ways to print before it failed (D11's notice).
    FellBackToViewer,
    /// The person cancelled the system's print dialog: no toast.
    Cancelled,
    /// Every way failed; the last cause in words.
    Failed(String),
    /// The window has no way to print (the platform's default).
    Unavailable,
}

/// The print job and what it built (the dialog's state is [`Dialog::Print`]).
#[derive(Default)]
pub(crate) struct PrintState {
    /// The running build: its generation and its job.
    job: Option<(u64, Job<PrintSet>)>,
    /// The built set of a generation.
    set: Option<(u64, Arc<PrintSet>)>,
    /// The last generation started.
    generation: u64,
    /// The request the window takes.
    request: Option<PrintRequest>,
    /// The title of the print the window runs, for its toast.
    printing: Option<String>,
}

impl std::fmt::Debug for PrintState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrintState")
            .field("generation", &self.generation)
            .field("building", &self.job.is_some())
            .field("built", &self.set.as_ref().map(|(g, _)| *g))
            .finish_non_exhaustive()
    }
}

/// A `print.page {to}` step.
fn page_step(p: &Map<String, Value>) -> Result<Option<i64>, Outcome> {
    match string_param(p, "to")? {
        None => Ok(None),
        Some("next") => Ok(Some(1)),
        Some("previous") => Ok(Some(-1)),
        Some(_) => Err(failed(key!("error-bad-param"), &["to"])),
    }
}

impl ShellModel {
    /// The print pre-dialog, when it is open.
    fn print_dialog_mut(&mut self) -> Option<&mut PrintDialog> {
        match &mut self.dialog {
            Some(Dialog::Print(d)) => Some(d),
            _ => None,
        }
    }

    /// The print pre-dialog's state, when it is open.
    pub fn print_dialog(&self) -> Option<&PrintDialog> {
        match &self.dialog {
            Some(Dialog::Print(d)) => Some(d),
            _ => None,
        }
    }

    /// The built print set the dialog shows, when it is built.
    pub fn print_set(&self) -> Option<&Arc<PrintSet>> {
        let d = self.print_dialog()?;
        self.print
            .set
            .as_ref()
            .filter(|(g, _)| *g == d.generation)
            .map(|(_, s)| s)
    }

    /// The progress of the running build, when one runs.
    pub fn print_progress(&self) -> Option<Progress> {
        self.print.job.as_ref().map(|(_, j)| j.progress())
    }

    /// Whether the window must wait for the print build (a visual run): one runs and no test gate holds it.
    pub fn print_waits(&self) -> bool {
        self.print.job.is_some() && self.jobs.hooks.gate().is_none()
    }

    /// The print request the window hands to its platform, taken once.
    pub fn take_print_request(&mut self) -> Option<PrintRequest> {
        let request = self.print.request.take()?;
        self.print.printing = Some(request.title.clone());
        Some(request)
    }

    /// The end of the print the window ran: its one toast (none for a cancel).
    pub fn print_done(&mut self, end: PrintEnd) {
        let title = self.print.printing.take().unwrap_or_default();
        let text = match end {
            PrintEnd::Printed => strings::format(key!("toast-print-printed"), &[&title]),
            PrintEnd::Opened => strings::format(key!("toast-print-opened"), &[&title]),
            PrintEnd::FellBackToPrinter => strings::format(key!("toast-print-fallback-printer"), &[&title]),
            PrintEnd::FellBackToViewer => strings::format(key!("toast-print-fallback-viewer"), &[&title]),
            PrintEnd::Cancelled => return,
            PrintEnd::Failed(cause) => {
                // The text ends its own sentence: a cause's closing full stop would double it.
                strings::format(
                    key!("toast-print-failed"),
                    &[cause.strip_suffix('.').unwrap_or(&cause)],
                )
            }
            PrintEnd::Unavailable => strings::text(key!("print-unavailable")).to_owned(),
        };
        self.notes.push(text, "print", None);
    }

    /// Cancels the running build and forgets the built set (the dialog closes).
    pub(super) fn drop_print(&mut self) {
        if let Some((_, job)) = self.print.job.take() {
            job.cancel();
        }
        self.print.set = None;
    }

    /// Cancels the running build and waits for its end (the shell's end: no thread outlives it).
    pub(super) fn stop_print(&mut self) {
        if let Some((_, job)) = self.print.job.take() {
            job.cancel();
            let _ = job.wait();
        }
        self.print.set = None;
    }

    /// `file.print {range?, sections?, pages?, paper?, fit?, mono?, wait?}` (Output ▸ Print…, Ctrl+P): opens the
    /// print pre-dialog with these options and starts its build.
    pub(crate) fn act_file_print(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: key!("reason-no-project"),
            };
        };
        let sections = ew_api::output::print::sections(&project.session.query())
            .into_iter()
            .map(|(number, title)| PrintSection {
                number,
                title,
                ticked: false,
            })
            .collect();
        let current = self.current_folio().map(str::to_owned);
        self.drop_print();
        self.open_dialog(Dialog::Print(Box::new(PrintDialog {
            range: PrintRange::All,
            sections,
            pages: String::new(),
            pages_error: None,
            current,
            paper: Paper::Sheet,
            fit: false,
            monochrome: false,
            page: 0,
            build: PrintBuild::Empty,
            generation: 0,
        })));
        self.act_print_set(p)
    }

    /// `print.set {range?, sections?, pages?, paper?, fit?, mono?, wait?}`: changes the pre-dialog's options; the
    /// running build is cancelled and one starts for the new options. `sections` is the ticked numbers separated by
    /// `,` (an empty text ticks none).
    pub(crate) fn act_print_set(&mut self, p: &Map<String, Value>) -> Outcome {
        let range = match string_param(p, "range") {
            Err(e) => return e,
            Ok(None) => None,
            Ok(Some(w)) => match PrintRange::parse(w) {
                Some(r) => Some(r),
                None => return failed(key!("error-bad-param"), &["range"]),
            },
        };
        let paper = match string_param(p, "paper") {
            Err(e) => return e,
            Ok(None) => None,
            Ok(Some(name)) => match Paper::parse(name) {
                Ok(paper) => Some(paper),
                Err(e) => {
                    return Outcome::Failed {
                        message: e.to_string(),
                    };
                }
            },
        };
        let (sections, pages) = match (string_param(p, "sections"), string_param(p, "pages")) {
            (Ok(s), Ok(t)) => (s.map(str::to_owned), t.map(str::to_owned)),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let wait = match bool_param(p, "wait", true) {
            Ok(w) => w,
            Err(e) => return e,
        };
        let Some(d) = self.print_dialog_mut() else {
            return Outcome::Refused {
                reason_key: key!("reason-no-print-dialog"),
            };
        };
        let flags = [("fit", d.fit), ("mono", d.monochrome)].map(|(name, now)| bool_param(p, name, now));
        let [fit, mono] = match flags {
            [Ok(f), Ok(m)] => [f, m],
            [Err(e), _] | [_, Err(e)] => return e,
        };
        if let Some(numbers) = &sections {
            let wanted: Vec<&str> = numbers
                .split(',')
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .collect();
            let known: Vec<String> = d.sections.iter().map(|s| s.number.to_string()).collect();
            if let Some(unknown) = wanted.iter().find(|n| !known.iter().any(|k| k == *n)) {
                return Outcome::Failed {
                    message: if known.is_empty() {
                        format!("no section `{unknown}` (the project has no sections)")
                    } else {
                        format!("no section `{unknown}` (the sections are {})", known.join(", "))
                    },
                };
            }
            for s in &mut d.sections {
                s.ticked = wanted.contains(&s.number.to_string().as_str());
            }
        }
        if let Some(r) = range {
            d.range = r;
        }
        if let Some(text) = pages {
            d.pages = text;
        }
        if let Some(paper) = paper {
            d.paper = paper;
        }
        d.fit = fit;
        d.monochrome = mono;
        self.start_print_build();
        if self.mode == Mode::Headless && wait {
            self.wait_print();
        }
        Outcome::Done
    }

    /// `print.page {to?: next|previous, page?}`: the preview's page, `page` counted from 1, clamped to the set.
    pub(crate) fn act_print_page(&mut self, p: &Map<String, Value>) -> Outcome {
        let step = match page_step(p) {
            Ok(s) => s,
            Err(e) => return e,
        };
        let page = match p.get("page") {
            None => None,
            Some(v) => match v.as_u64().filter(|n| *n >= 1) {
                Some(n) => Some(usize::try_from(n - 1).unwrap_or(usize::MAX)),
                None => return failed(key!("error-bad-param"), &["page"]),
            },
        };
        let pages = self.print_set().map_or(0, |s| s.pages.len());
        let Some(d) = self.print_dialog_mut() else {
            return Outcome::Refused {
                reason_key: key!("reason-no-print-dialog"),
            };
        };
        let last = pages.saturating_sub(1);
        let mut at = page.unwrap_or(d.page);
        match step {
            Some(1) => at = at.saturating_add(1),
            Some(_) => at = at.saturating_sub(1),
            None => {}
        }
        d.page = at.min(last);
        Outcome::Done
    }

    /// `print.send {via?: printer|viewer}`: hands the built set to the window (Print… or Open in PDF viewer) and
    /// closes the dialog.
    pub(crate) fn act_print_send(&mut self, p: &Map<String, Value>) -> Outcome {
        let via = match string_param(p, "via") {
            Err(e) => return e,
            Ok(None | Some("printer")) => PrintVia::Printer,
            Ok(Some("viewer")) => PrintVia::Viewer,
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["via"]),
        };
        let Some(set) = self.print_set().cloned() else {
            return Outcome::Refused {
                reason_key: key!("reason-print-not-built"),
            };
        };
        let monochrome = self.print_dialog().is_some_and(|d| d.monochrome);
        // As `ew print`: the project directory's name without its extension.
        let name = self.project_name().unwrap_or("project");
        let title = std::path::Path::new(name)
            .file_stem()
            .map_or_else(|| name.to_owned(), |s| s.to_string_lossy().into_owned());
        self.print.request = Some(PrintRequest {
            set,
            title,
            via,
            monochrome,
        });
        self.drop_print();
        self.dialog = None;
        Outcome::Done
    }

    /// Cancels the running build and starts one for the dialog's options (none when no page is chosen).
    fn start_print_build(&mut self) {
        if let Some((_, job)) = self.print.job.take() {
            job.cancel();
        }
        self.print.set = None;
        self.print.generation += 1;
        let generation = self.print.generation;
        let options = JobOptions {
            name: JobKind::Print.word().to_owned(),
            waker: self.jobs.waker.clone(),
            gate: self.jobs.hooks.gate(),
        };
        let Some(d) = self.print_dialog() else { return };
        let selection = d.selection();
        let parse_error = match (d.range, &selection) {
            // FT-1124 (ID-1225): the refusal in words, not the engine's.
            (PrintRange::Pages, None) if !d.pages.trim().is_empty() => PageSelection::parse(&d.pages)
                .err()
                .map(|_| strings::text(key!("print-pages-error")).to_owned()),
            _ => None,
        };
        let job = match (&self.project, selection) {
            (Some(project), Some(selection)) => {
                let job = project.session.print_set_job(&d.options(selection));
                Some(Job::spawn(options, move |ctl| job.run(ctl)))
            }
            _ => None,
        };
        let build = match job {
            Some(Ok(job)) => {
                self.print.job = Some((generation, job));
                PrintBuild::Building
            }
            Some(Err(_)) => PrintBuild::Failed(strings::text(key!("error-no-thread")).to_owned()),
            None => PrintBuild::Empty,
        };
        if let Some(d) = self.print_dialog_mut() {
            d.generation = generation;
            d.page = 0;
            d.pages_error = parse_error;
            d.build = build;
        }
    }

    /// Waits for the running build and takes its end.
    pub fn wait_print(&mut self) {
        if let Some((generation, job)) = self.print.job.take() {
            let end = job.wait();
            self.finish_print(generation, end);
        }
    }

    /// Takes the end of a finished build (the window every frame, the script runner before every step); a build
    /// whose dialog has closed is cancelled.
    pub(super) fn poll_print(&mut self) {
        if self.print_dialog().is_none() {
            self.drop_print();
            return;
        }
        let ended = match &mut self.print.job {
            Some((generation, job)) => job.try_end().map(|end| (*generation, end)),
            None => None,
        };
        if let Some((generation, end)) = ended {
            self.print.job = None;
            self.finish_print(generation, end);
        }
    }

    /// The end of build `generation`: the dialog's state, when it still shows that build.
    fn finish_print(&mut self, generation: u64, end: JobEnd<PrintSet>) {
        // FT-1124 (ID-1225): a selection the page order refuses is the Pages text's refusal, shown once under the field
        // in words while the preview shows the empty state; any other failure shows in the preview only. The engine
        // words (the CLI's) are told by `OutputError::Selection`'s own prefix.
        let selection = ew_api::output::OutputError::Selection(String::new()).to_string();
        let pages = self.print_dialog().is_some_and(|d| d.range == PrintRange::Pages);
        let (build, set, field_error) = match end {
            JobEnd::Done(set) => (
                PrintBuild::Built {
                    pages: set.pages.len(),
                },
                Some(Arc::new(set)),
                None,
            ),
            JobEnd::Cancelled => (PrintBuild::Empty, None, None),
            JobEnd::Failed(cause) if pages && cause.starts_with(&selection) => (
                PrintBuild::Empty,
                None,
                Some(strings::text(key!("print-pages-error")).to_owned()),
            ),
            JobEnd::Failed(cause) => (PrintBuild::Failed(cause), None, None),
            JobEnd::Panicked => (
                PrintBuild::Failed(strings::text(key!("job-cause-internal")).to_owned()),
                None,
                None,
            ),
        };
        let Some(d) = self.print_dialog_mut() else { return };
        if d.generation != generation {
            return;
        }
        // A Pages text the page order refuses shows its refusal under the field.
        if d.range == PrintRange::Pages {
            d.pages_error = field_error;
        }
        d.build = build;
        d.page = 0;
        self.print.set = set.map(|s| (generation, s));
    }

    /// The preview's caption: "1 of 5".
    pub fn print_page_text(&self) -> Option<String> {
        let d = self.print_dialog()?;
        let set = self.print_set()?;
        Some(strings::format(
            key!("print-page-of"),
            &[&(d.page + 1).to_string(), &set.pages.len().to_string()],
        ))
    }

    /// The progress line of a running build (§6.19): "Building the print set… 40 %", or the writing phase's line.
    pub fn print_progress_text(&self) -> Option<String> {
        let p = self.print_progress()?;
        Some(if p.total == 0 && p.phase == "writing" {
            strings::text(key!("print-writing")).to_owned()
        } else {
            let percent = (u64::from(p.done) * 100)
                .checked_div(u64::from(p.total))
                .unwrap_or(0);
            strings::format(key!("print-building"), &[&percent.to_string()])
        })
    }

    /// Why Print… and Open in PDF viewer are disabled now; `None` once the set is built.
    pub fn print_disabled_reason(&self) -> Option<String> {
        let d = self.print_dialog()?;
        match &d.build {
            PrintBuild::Built { .. } if self.print_set().is_some() => None,
            PrintBuild::Empty => Some(strings::text(key!("reason-print-no-pages")).to_owned()),
            PrintBuild::Failed(_) => Some(strings::text(key!("reason-print-failed")).to_owned()),
            _ => Some(strings::text(key!("reason-print-not-built")).to_owned()),
        }
    }
}
