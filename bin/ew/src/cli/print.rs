// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew print`: the pages to a printer or a spool file.

use super::*;

/// `ew print`: the print set of the selection, written to `--out` or handed to the adapter.
pub(super) fn run_print(a: &PrintArgs) -> i32 {
    use ew_api::output::print::{Fit, PrintOptions};
    let Some(dir) = reader_dir(a.project.as_ref()) else {
        return 2;
    };
    let date = match source_date() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let (session, _) = match ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "print") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    let options = PrintOptions {
        pages: a.pages.clone(),
        paper: a.paper,
        fit: if a.fit { Fit::ToPaper } else { Fit::Actual },
        monochrome: a.mono,
        date,
        parallelism: Parallelism::threads(
            std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get),
        ),
    };
    let set = match session.print_set(&options) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let folios: Vec<String> = set.pages.iter().map(|p| p.label.clone()).collect();
    let scale = (0..set.pages.len()).map(|i| set.percent(i)).min().unwrap_or(100);
    let notices: Vec<String> = set.notices.iter().map(ToString::to_string).collect();
    for n in &notices {
        eprintln!("notice: {n}");
    }
    let result = |adapter: &str, out: Option<String>| results::PrintResult {
        schema: results::PRINT.to_owned(),
        adapter: adapter.to_owned(),
        printer: a.printer.clone().unwrap_or_default(),
        pages: folios.clone(),
        paper: a.paper.name().to_owned(),
        scale,
        notices: notices.clone(),
        out,
    };
    if let Some(out) = &a.out {
        if let Err(e) = write_atomically(out, &set.pdf) {
            eprintln!("error: {}: {e}", out.display());
            return 2;
        }
        if a.json {
            print!(
                "{}",
                results::to_text(&result("file", Some(out.display().to_string())))
            );
        }
        return 0;
    }
    // The job's title and the viewer's file stem: the project directory's name without its extension.
    let title = dir
        .file_stem()
        .map_or_else(|| "project".to_owned(), |n| n.to_string_lossy().into_owned());
    let job = ew_print::PrintJob {
        title: title.clone(),
        pdf: set.pdf,
        pages: set.pages,
        fonts: ew_api::output::font_bytes().clone(),
        printer: a.printer.clone(),
        monochrome: a.mono,
        cancel: ew_print::PrintCancel::default(),
    };
    let adapter: Box<dyn ew_print::PrintAdapter> = match a.via {
        PrintVia::Lp => Box::new(ew_print::Lp::new(std::env::temp_dir())),
        PrintVia::Viewer => Box::new(ew_print::Viewer::new(std::env::temp_dir(), &title)),
        // Ctrl+C aborts the document at the next band instead of ending the process mid-job.
        #[cfg(windows)]
        PrintVia::Windows => {
            ew_print::ctrl_c_cancels(&job.cancel);
            Box::new(ew_print::Windows::new())
        }
    };
    let outcome = match adapter.print(&job) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let file = match &outcome {
        ew_print::PrintOutcome::Opened { file } => Some(file.display().to_string()),
        ew_print::PrintOutcome::Spooled { .. } => None,
    };
    if a.json {
        print!("{}", results::to_text(&result(adapter.name(), file)));
    } else {
        match &outcome {
            ew_print::PrintOutcome::Spooled { printer, message } => {
                println!(
                    "{}",
                    spooled_line(folios.len(), printer.as_deref(), adapter.name())
                );
                if !message.is_empty() {
                    println!("{message}");
                }
            }
            ew_print::PrintOutcome::Opened { file } => println!(
                "{} pages opened in the PDF viewer from {}; print them from there",
                folios.len(),
                file.display()
            ),
        }
    }
    0
}

/// What `ew print` says when a spooler took `pages` pages: the printer (`None`: the default one) and the way, `lp` or
/// the Windows print spooler (`--via windows`).
pub(super) fn spooled_line(pages: usize, printer: Option<&str>, adapter: &str) -> String {
    let to = printer.map_or_else(|| "the default printer".to_owned(), |p| format!("printer {p}"));
    let through = if adapter == "lp" {
        "`lp`"
    } else {
        "the Windows print spooler"
    };
    format!("{pages} pages sent to {to} through {through}")
}
