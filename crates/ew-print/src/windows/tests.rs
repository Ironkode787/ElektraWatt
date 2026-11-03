// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Windows adapter without a dialog (WP-S3.07 WN-3's automated half, ruling 7 of ID-1218): its words, its paper
//! requests, the dialog's printer name, and four placed pages printed to "Microsoft Print to PDF".

use std::collections::BTreeMap;

use ::windows::Win32::Foundation::ERROR_INVALID_PRINTER_NAME;
use ::windows::Win32::Graphics::Gdi::{DEVMODE_FIELD_FLAGS, DEVMODEW_0_0};
use ew_render_cpu::ew_dl::ew_units::{Nm, Pt2, Rect};
use ew_render_cpu::ew_dl::{Brush, Destination, DisplayList, Item, Path as DlPath, Pen, Stroke};

use super::*;

/// The printer every Windows 10 and 11 installation has, which writes PDF files.
const PDF_PRINTER: &str = "Microsoft Print to PDF";

/// A fresh empty directory under the system's temporary folder, removed on drop.
struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Dir {
        let dir = std::env::temp_dir().join(format!("ew-print-windows-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Dir(dir)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn job(printer: &str, pages: Vec<PdfPage>) -> PrintJob {
    PrintJob {
        title: "J-E-2416 section 1".to_owned(),
        pdf: Vec::new(),
        pages,
        printer: Some(printer.to_owned()),
        monochrome: false,
        fonts: BTreeMap::new(),
    }
}

/// Every step's words, whole, and the error with and without the system's reason.
#[test]
fn the_steps_read_as_words() {
    let steps = [
        (Step::Dialog, "the Windows print dialog could not be shown"),
        (Step::Open("office"), "the printer `office` could not be opened"),
        (Step::StartDoc, "the printer did not start the document"),
        (
            Step::Paper("103"),
            "the printer did not take the paper of page 103",
        ),
        (Step::Resolution, "the printer reports no resolution"),
        (Step::StartPage("103"), "the printer did not start page 103"),
        (Step::Render("103"), "page 103 could not be rendered"),
        (Step::Band("103"), "the printer did not take page 103"),
        (Step::EndPage("103"), "the printer did not finish page 103"),
        (Step::EndDoc, "the printer did not finish the document"),
    ];
    for (step, words) in steps {
        assert_eq!(step.to_string(), words);
    }
    assert_eq!(
        error(Step::Dialog, "it gave no printer".to_owned()).to_string(),
        "the Windows print dialog could not be shown: it gave no printer"
    );
    assert_eq!(
        error(Step::Resolution, String::new()).to_string(),
        "the printer reports no resolution"
    );
}

/// A printer that does not exist is refused in words with the system's reason, before anything is spooled.
#[test]
fn a_printer_that_does_not_exist_is_refused_in_words() {
    let name = "ew-print-test-no-such-printer";
    assert_eq!(
        Windows::new().print(&job(name, Vec::new())),
        Err(PrintError::Windows {
            what: format!("the printer `{name}` could not be opened"),
            reason: HRESULT::from_win32(ERROR_INVALID_PRINTER_NAME.0).message(),
        })
    );
}

/// The settings' bytes.
fn bytes(mode: &DevMode) -> Vec<u8> {
    mode.0.iter().flat_map(|w| w.to_ne_bytes()).collect()
}

/// The paper fields of the settings: the field flags, then orientation, size, length and width.
fn paper(mode: &DevMode) -> (u32, [i16; 4]) {
    let b = bytes(mode);
    let at = |o: usize| i16::from_ne_bytes([b[o], b[o + 1]]);
    let union = offset_of!(DEVMODEW, Anonymous1);
    let flags = offset_of!(DEVMODEW, dmFields);
    (
        u32::from_ne_bytes([b[flags], b[flags + 1], b[flags + 2], b[flags + 3]]),
        [
            at(union + offset_of!(DEVMODEW_0_0, dmOrientation)),
            at(union + offset_of!(DEVMODEW_0_0, dmPaperSize)),
            at(union + offset_of!(DEVMODEW_0_0, dmPaperLength)),
            at(union + offset_of!(DEVMODEW_0_0, dmPaperWidth)),
        ],
    )
}

/// Ruling 3 of ID-1218: a named size of D6 is asked for by its Windows paper and the orientation, A1 (no Windows
/// paper) by its sides in 0.1 mm; the settings keep their own size and the printer's name, and a block shorter than
/// its own size is refused.
#[test]
fn the_paper_request_names_a_known_size_or_gives_its_sides() {
    let mut mode = DevMode::blank(PDF_PRINTER);
    let dm = mode.public();
    assert_eq!(dm.dmSize as usize, size_of::<DEVMODEW>());
    assert_eq!(dm.dmDriverExtra, 0);
    let name: Vec<u16> = PDF_PRINTER.encode_utf16().collect();
    assert_eq!(dm.dmDeviceName[..name.len()], name[..]);
    assert_eq!(dm.dmDeviceName[name.len()], 0);
    let fields = |f: &[DEVMODE_FIELD_FLAGS]| f.iter().fold(0, |a, b| a | b.0);
    // A4 landscape on A5 landscape: the Windows A5 paper, turned.
    mode.ask_paper(210_000_000, 148_000_000);
    assert_eq!(
        paper(&mode),
        (
            fields(&[DM_ORIENTATION, DM_PAPERSIZE]),
            [DMORIENT_LANDSCAPE as i16, DMPAPER_A5 as i16, 0, 0]
        )
    );
    // US Letter upright.
    mode.ask_paper(215_900_000, 279_400_000);
    assert_eq!(
        paper(&mode),
        (
            fields(&[DM_ORIENTATION, DM_PAPERSIZE]),
            [DMORIENT_PORTRAIT as i16, DMPAPER_LETTER as i16, 0, 0]
        )
    );
    // A1 upright: no Windows paper, so its sides; the size field leaves the request.
    mode.ask_paper(594_000_000, 841_000_000);
    assert_eq!(
        paper(&mode),
        (
            fields(&[DM_ORIENTATION, DM_PAPERLENGTH, DM_PAPERWIDTH]),
            [DMORIENT_PORTRAIT as i16, 0, 8410, 5940]
        )
    );
    // The dialog's block: read back whole; shorter than its own size, refused.
    let block = bytes(&mode);
    assert_eq!(DevMode::from_bytes(&block).map(|m| paper(&m)), Some(paper(&mode)));
    assert!(DevMode::from_bytes(&block[..size_of::<DEVMODEW>() - 1]).is_none());
    assert!(DevMode::from_bytes(&block[..10]).is_none());
}

/// The printer's name in the dialog's DEVNAMES block; a block whose name has no end gives none.
#[test]
fn the_dialog_names_its_printer() {
    // The header (4 characters), then "winspool", the printer and the port, each NUL-terminated.
    let mut units: Vec<u16> = vec![4, 13, 20, 0];
    for s in ["winspool", "Office", "Ne00:"] {
        units.extend(s.encode_utf16().chain([0]));
    }
    let block: Vec<u8> = units.iter().flat_map(|u| u.to_ne_bytes()).collect();
    assert_eq!(device_name(&block).as_deref(), Some("Office"));
    assert_eq!(device_name(&block[..30]), None);
    assert_eq!(device_name(&block[..4]), None);
    // Device pixels: equal resolutions keep the rows, a finer vertical one stretches them, halves round up.
    assert_eq!(device(256, 600, 600), 256);
    assert_eq!(device(256, 1200, 600), 512);
    assert_eq!(device(1, 300, 200), 2);
}

/// A4 landscape sheet `n` fitted to A5 landscape (148/210, centred) with a stand-in for the stamp.
fn placed_page(n: usize) -> PdfPage {
    let mm = |x: i64, y: i64| Pt2::new(Nm::from_mm(x), Nm::from_mm(y));
    let mut items = vec![
        Item::path(
            DlPath::rect(Rect::from_points(mm(10, 10), mm(287, 200))),
            Some(Stroke::pen(Pen::P052)),
            None,
        ),
        Item::path(
            DlPath::line(mm(10, 10), mm(287, 200)),
            Some(Stroke::pen(Pen::P026)),
            None,
        ),
    ];
    // One block per page number, so the pages differ.
    for k in 0..n {
        let x = 30 + 40 * k as i64;
        items.push(Item::path(
            DlPath::rect(Rect::from_points(mm(x, 40), mm(x + 30, 90))),
            None,
            Some(Brush::BLACK),
        ));
    }
    PdfPage {
        destination: Destination::Page(Default::default()),
        label: format!("10{n}"),
        title: format!("10{n} Test page"),
        section: Some("Section 1".to_owned()),
        width: Nm::from_mm(297),
        height: Nm::from_mm(210),
        content: DisplayList { items },
        placement: Some(PagePlacement {
            width: Nm::from_mm(210),
            height: Nm::from_mm(148),
            scale: (148, 210),
            offset: Pt2::new(Nm(342_857), Nm(0)),
            overlay: DisplayList {
                items: vec![Item::path(
                    DlPath::rect(Rect::from_points(mm(180, 141), mm(205, 144))),
                    None,
                    Some(Brush::BLACK),
                )],
            },
        }),
    }
}

/// The page objects of a PDF (`/Type /Page`, not `/Pages`).
fn page_objects(pdf: &[u8]) -> usize {
    [&b"/Type /Page"[..], b"/Type/Page"]
        .iter()
        .map(|pat| {
            pdf.windows(pat.len())
                .enumerate()
                .filter(|(i, w)| w == pat && pdf.get(i + pat.len()) != Some(&b's'))
                .count()
        })
        .sum()
}

/// WN-3's automated half (ruling 7 of ID-1218): four placed pages printed through the adapter to "Microsoft Print to
/// PDF" with DOCINFO's output file set, so no dialog opens; the spooler writes a PDF of four pages once the job has
/// run. When that printer's device context cannot be created, a skip line names the reason and the test passes.
#[test]
fn four_placed_pages_print_to_microsoft_print_to_pdf() {
    let dir = Dir::new("wn3");
    let out = dir.0.join("wn3.pdf");
    let pages: Vec<PdfPage> = (1..=4).map(placed_page).collect();
    match Windows::to_file(out.clone()).print(&job(PDF_PRINTER, pages)) {
        Err(PrintError::Windows { what, reason }) if what == Step::Open(PDF_PRINTER).to_string() => {
            println!("skipped: {what}: {reason}");
            return;
        }
        printed => assert_eq!(
            printed,
            Ok(PrintOutcome::Spooled {
                printer: Some(PDF_PRINTER.to_owned()),
                message: String::new(),
            })
        ),
    }
    // The spooler writes the file after `EndDoc`, when the job runs: waited for up to two minutes.
    let mut pdf = Vec::new();
    for _ in 0..600 {
        pdf = std::fs::read(&out).unwrap_or_default();
        if pdf.trim_ascii_end().ends_with(b"%%EOF") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    assert!(
        pdf.trim_ascii_end().ends_with(b"%%EOF"),
        "no PDF at {}",
        out.display()
    );
    // What the driver made of the A5 landscape request, for the manual half of WN-3.
    let boxes: Vec<String> = pdf
        .windows(9)
        .enumerate()
        .filter(|(_, w)| w == b"/MediaBox")
        .filter_map(|(i, _)| {
            let rest = &pdf[i..];
            let end = rest.iter().position(|&b| b == b']')?;
            Some(String::from_utf8_lossy(&rest[..=end]).into_owned())
        })
        .collect();
    println!("{PDF_PRINTER} wrote {} bytes; {boxes:?}", pdf.len());
    assert_eq!(page_objects(&pdf), 4);
}
