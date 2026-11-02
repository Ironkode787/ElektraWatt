// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The print set (WP-S3.07 part 2, D5 to D7; decision log ID-1114): the export's pages placed on a paper, stamped
//! when scaled, and written by the export's PDF writer with the export's options and fonts (ADR-0015: the PDF is the
//! print master). `ew-print`'s adapters hand the bytes to the OS; the binaries build their `PrintJob` from a
//! [`PrintSet`]'s fields.
//!
//! **One geometry (D7).** A print page is the export page unchanged ([`PdfPage::content`], item for item) with a
//! [`PagePlacement`]: the paper's size, the uniform scale as an exact ratio, the offset, and an overlay in paper
//! coordinates that holds the stamp and nothing else. Every renderer draws it by one rule: the content at the target
//! resolution times the scale, at the offset, then the overlay at the paper's resolution.
//!
//! **The paper and the fit (D6).** [`Paper::Sheet`] prints each page on its own sheet size; a named size is turned
//! like each page (landscape for a sheet wider than high). [`Fit::Actual`] prints at 100 % with the sheet's top-left
//! corner at the paper's: a sheet larger than the paper is clipped from its top-left and the set's notices name the
//! page ([`PrintNotice::Clipped`]). [`Fit::ToPaper`] scales each page uniformly to the whole paper,
//! `min(paper width / sheet width, paper height / sheet height)`, never above 100 %, and centres it; below 100 % the
//! stamp `scaled <n> %` (`n` rounded half up) is drawn in the house font, black, 2 mm cap height, its baseline 4 mm
//! above the paper's bottom edge and its end 5 mm from the right edge.
//!
//! **The document ID.** A set whose every page lies on its own sheet unscaled with no stamp is the export's
//! document, byte for byte; any other set adds ` print <paper> <fit>` to the export's document ID text.

use std::fmt;
use std::sync::Arc;

use ew_derive::Q;
use ew_dl::ew_units::{Nm, Pt2};
use ew_dl::{DisplayList, Item};
use ew_export::{Parallelism, PdfPage, PdfReport};
use ew_text::TextStyle;

pub use ew_export::PagePlacement;

use super::{
    ExportAssets, ExportOptions, Halt, Hooks, NoHooks, OutputError, PageFindingAt, PageSelection, PdfDate,
    export_pages_hooked, font_bytes,
};
use crate::Session;

/// The paper a print set is printed on (WP-S3.07 D6).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum Paper {
    /// Each page on its own sheet size (the default).
    #[default]
    Sheet,
    /// ISO A5, 148 × 210 mm.
    A5,
    /// ISO A4, 210 × 297 mm.
    A4,
    /// ISO A3, 297 × 420 mm.
    A3,
    /// ISO A2, 420 × 594 mm.
    A2,
    /// ISO A1, 594 × 841 mm.
    A1,
    /// ISO A0, 841 × 1189 mm.
    A0,
    /// US Letter, 8.5 × 11 in.
    Letter,
    /// US Legal, 8.5 × 14 in.
    Legal,
    /// US Tabloid, 11 × 17 in.
    Tabloid,
}

impl Paper {
    /// Every paper, in the order the refusal lists them.
    pub const ALL: [Paper; 10] = [
        Paper::Sheet,
        Paper::A5,
        Paper::A4,
        Paper::A3,
        Paper::A2,
        Paper::A1,
        Paper::A0,
        Paper::Letter,
        Paper::Legal,
        Paper::Tabloid,
    ];

    /// The paper's name as `--paper` takes it: `sheet`, `A5`, …, `Tabloid`.
    pub fn name(self) -> &'static str {
        match self {
            Paper::Sheet => "sheet",
            Paper::A5 => "A5",
            Paper::A4 => "A4",
            Paper::A3 => "A3",
            Paper::A2 => "A2",
            Paper::A1 => "A1",
            Paper::A0 => "A0",
            Paper::Letter => "Letter",
            Paper::Legal => "Legal",
            Paper::Tabloid => "Tabloid",
        }
    }

    /// The paper named `name`, ignoring ASCII case.
    ///
    /// # Errors
    ///
    /// [`PrintError::UnknownPaper`] for any other name.
    pub fn parse(name: &str) -> Result<Paper, PrintError> {
        Paper::ALL
            .into_iter()
            .find(|p| p.name().eq_ignore_ascii_case(name.trim()))
            .ok_or_else(|| PrintError::UnknownPaper(name.to_owned()))
    }

    /// The short and the long side; `None` for [`Paper::Sheet`].
    pub fn sides(self) -> Option<(Nm, Nm)> {
        let mm = |a: i64, b: i64| Some((Nm::from_mm(a), Nm::from_mm(b)));
        // 1 in = 25.4 mm = 25 400 000 nm.
        let inch = |a10: i64, b10: i64| Some((Nm(a10 * 2_540_000), Nm(b10 * 2_540_000)));
        match self {
            Paper::Sheet => None,
            Paper::A5 => mm(148, 210),
            Paper::A4 => mm(210, 297),
            Paper::A3 => mm(297, 420),
            Paper::A2 => mm(420, 594),
            Paper::A1 => mm(594, 841),
            Paper::A0 => mm(841, 1189),
            Paper::Letter => inch(85, 110),
            Paper::Legal => inch(85, 140),
            Paper::Tabloid => inch(110, 170),
        }
    }
}

impl fmt::Display for Paper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// How a page meets the paper (WP-S3.07 D6).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Fit {
    /// At 100 %, the sheet's top-left corner at the paper's; a larger sheet is clipped (the default).
    #[default]
    Actual,
    /// Scaled uniformly to the whole paper and centred, never enlarged (`--fit`).
    ToPaper,
}

/// Why no print set was built.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PrintError {
    /// A paper name that is not one of [`Paper::ALL`].
    UnknownPaper(String),
}

impl fmt::Display for PrintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrintError::UnknownPaper(name) => {
                let names: Vec<&str> = Paper::ALL.iter().map(|p| p.name()).collect();
                write!(f, "unknown paper `{name}` ({})", names.join(", "))
            }
        }
    }
}

impl std::error::Error for PrintError {}

/// What a print set is built from (WP-S3.07 D6).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PrintOptions {
    /// The pages (`--pages`, `--sections`).
    pub pages: PageSelection,
    /// The paper (`--paper`).
    pub paper: Paper,
    /// The fit (`--fit`).
    pub fit: Fit,
    /// Draw in black and grey (`--mono`, FR-OUT-011).
    pub monochrome: bool,
    /// The creation date (`SOURCE_DATE_EPOCH` in the CLI); `None`: the model's.
    pub date: Option<PdfDate>,
    /// The threads of the page phase; changes speed only, never bytes.
    pub parallelism: Parallelism,
}

/// Something the person should know about a print set.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PrintNotice {
    /// A page larger than the paper, printed at 100 % and clipped from its top-left corner.
    Clipped {
        /// The page's folio label.
        folio: String,
        /// The paper.
        paper: Paper,
    },
}

impl fmt::Display for PrintNotice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrintNotice::Clipped { folio, paper } => write!(
                f,
                "page {folio} is larger than {paper}: it is printed at 100 % and clipped from its top-left corner"
            ),
        }
    }
}

/// A built print set (WP-S3.07 D5).
#[derive(Clone, PartialEq, Debug)]
pub struct PrintSet {
    /// The print pages, in page order: each export page with its [`PagePlacement`].
    pub pages: Vec<PdfPage>,
    /// The PDF of the set (the print master).
    pub pdf: Vec<u8>,
    /// Per page, the stamp it carries ("scaled 70 %"), `None` for a page printed at 100 %.
    pub stamps: Vec<Option<String>>,
    /// The notices (clipped pages), in page order.
    pub notices: Vec<PrintNotice>,
    /// The findings of the page scenes.
    pub findings: Vec<PageFindingAt>,
    /// The writer's report.
    pub report: PdfReport,
}

impl PrintSet {
    /// The scale of the page at `index` in whole percent, rounded half up (100 for an unscaled page).
    pub fn percent(&self, index: usize) -> u32 {
        self.pages
            .get(index)
            .and_then(|p| p.placement.as_ref())
            .map_or(100, |p| percent(p.scale))
    }
}

/// `num / den` in whole percent, rounded half up.
fn percent((num, den): (u64, u64)) -> u32 {
    let (num, den) = (u128::from(num), u128::from(den.max(1)));
    u32::try_from((200 * num + den) / (2 * den)).unwrap_or(u32::MAX)
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

/// The placement of a `width` × `height` sheet on `paper` with `fit`, and whether the sheet is clipped (D6).
pub fn placement(width: Nm, height: Nm, paper: Paper, fit: Fit) -> (PagePlacement, bool) {
    let (pw, ph) = match paper.sides() {
        None => (width, height),
        Some((short, long)) if width > height => (long, short),
        Some((short, long)) => (short, long),
    };
    let positive = |n: Nm| u64::try_from(n.0).ok().filter(|v| *v > 0);
    let scale = match (fit, positive(width), positive(height), positive(pw), positive(ph)) {
        (Fit::ToPaper, Some(sw), Some(sh), Some(pw), Some(ph)) => {
            // min(pw / sw, ph / sh), compared exactly; never enlarged.
            let (num, den) = if u128::from(pw) * u128::from(sh) <= u128::from(ph) * u128::from(sw) {
                (pw, sw)
            } else {
                (ph, sh)
            };
            if num >= den {
                (1, 1)
            } else {
                let g = gcd(num, den);
                (num / g, den / g)
            }
        }
        _ => (1, 1),
    };
    let scaled = |n: Nm| {
        let v = i128::from(n.0) * i128::from(scale.0) / i128::from(scale.1);
        Nm(i64::try_from(v).unwrap_or(i64::MAX))
    };
    let offset = match fit {
        Fit::Actual => Pt2::ORIGIN,
        Fit::ToPaper => Pt2::new(
            Nm((pw.0 - scaled(width).0) / 2),
            Nm((ph.0 - scaled(height).0) / 2),
        ),
    };
    let clipped = fit == Fit::Actual && (width > pw || height > ph);
    let overlay = if scale.0 < scale.1 {
        DisplayList {
            items: vec![stamp(&stamp_text(scale), pw, ph)],
        }
    } else {
        DisplayList::default()
    };
    (
        PagePlacement {
            width: pw,
            height: ph,
            scale,
            offset,
            overlay,
        },
        clipped,
    )
}

/// The stamp's text for a scale: "scaled 70 %".
pub fn stamp_text(scale: (u64, u64)) -> String {
    format!("scaled {} %", percent(scale))
}

/// The stamp's cap height.
pub const STAMP_CAP_HEIGHT: Nm = Nm(2_000_000);
/// The stamp's baseline above the paper's bottom edge.
pub const STAMP_BASELINE: Nm = Nm(4_000_000);
/// The stamp's end from the paper's right edge.
pub const STAMP_END: Nm = Nm(5_000_000);

/// The stamp's glyph run on a `pw` × `ph` paper: the house font, black, [`STAMP_CAP_HEIGHT`], its baseline
/// [`STAMP_BASELINE`] above the bottom edge and its end [`STAMP_END`] from the right edge.
fn stamp(text: &str, pw: Nm, ph: Nm) -> Item {
    let fonts = ew_text::bundled();
    // The em height whose cap height is 2 mm, from the face's cap height at a 1 mm em.
    let unit = TextStyle::regular(Nm::from_mm(1));
    let cap = fonts.vertical_metrics(&unit).cap_height.0.max(1);
    let em = i128::from(STAMP_CAP_HEIGHT.0) * i128::from(Nm::from_mm(1).0) / i128::from(cap);
    let style = unit.with_size(Nm(i64::try_from(em).unwrap_or(STAMP_CAP_HEIGHT.0)));
    let shaped = fonts.shape(text, &style, None);
    let mut run = (*shaped.run).clone();
    run.origin = Pt2::new(pw - STAMP_END - shaped.advance(), ph - STAMP_BASELINE);
    Item::glyphs(Arc::new(run))
}

/// The print set of the pages `options` selects over `q` (D5): the export's pages, placed and stamped, written by
/// [`ew_export::pdf`] with the export's options and the bundled fonts.
///
/// # Errors
///
/// [`OutputError`] for a selection the page order does not have or pages the writer refuses.
pub fn print_set_with(q: &Q, options: &PrintOptions, assets: &ExportAssets) -> Result<PrintSet, OutputError> {
    print_set_hooked(q, options, assets, &NoHooks).map_err(|h| match h {
        Halt::Output(e) => e,
        // No hook cancels.
        Halt::Cancelled => OutputError::Selection("the print set was cancelled".to_owned()),
    })
}

/// The sections a print set may name ([`PageSelection::Sections`], the window's Range ▸ Sections list): each
/// section's number and neutral title, by number, each number once.
pub fn sections(q: &Q) -> Vec<(u16, String)> {
    let mut all: Vec<(u16, String)> = q
        .sections()
        .iter()
        .map(|(_, s)| (s.number, super::view::neutral(&s.title)))
        .collect();
    all.sort();
    all.dedup_by_key(|s| s.0);
    all
}

/// [`print_set_with`] with `hooks` (the print job, ID-1180 ruling 2): the export's phases with their reports and
/// checkpoints, then `report(0, 0, "writing")` before the placement and the writer.
pub(crate) fn print_set_hooked<H: Hooks>(
    q: &Q,
    options: &PrintOptions,
    assets: &ExportAssets,
    hooks: &H,
) -> Result<PrintSet, Halt> {
    let export = export_pages_hooked(
        q,
        &ExportOptions {
            pages: options.pages.clone(),
            parallelism: options.parallelism,
            monochrome: options.monochrome,
            date: options.date,
        },
        assets,
        hooks,
    )?;
    hooks.report(0, 0, "writing");
    let mut set = print_pages(export.pages, export.options, options.paper, options.fit)?;
    set.findings = export.findings;
    Ok(set)
}

/// The print set of export `pages` written with their export `options` (D5 to D7): each page placed on `paper` with
/// `fit` and stamped when scaled, then written by [`ew_export::pdf`] with the bundled fonts; no findings.
///
/// # Errors
///
/// [`OutputError::Pdf`] when the writer refuses the pages.
pub fn print_pages(
    mut pages: Vec<PdfPage>,
    options: ew_export::PdfOptions,
    paper: Paper,
    fit: Fit,
) -> Result<PrintSet, OutputError> {
    let mut stamps = Vec::with_capacity(pages.len());
    let mut notices = Vec::new();
    let mut as_exported = true;
    for page in &mut pages {
        let (placement, clipped) = placement(page.width, page.height, paper, fit);
        if clipped {
            notices.push(PrintNotice::Clipped {
                folio: page.label.clone(),
                paper,
            });
        }
        stamps.push((placement.scale.0 < placement.scale.1).then(|| stamp_text(placement.scale)));
        as_exported &= placement.is_identity()
            && placement.overlay.items.is_empty()
            && (placement.width, placement.height) == (page.width, page.height);
        page.placement = Some(placement);
    }
    let mut pdf_options = options;
    if !as_exported {
        let fit = match fit {
            Fit::Actual => "actual",
            Fit::ToPaper => "fit",
        };
        pdf_options.document_id = format!("{} print {paper} {fit}", pdf_options.document_id);
    }
    let out = ew_export::pdf(&pages, &pdf_options, font_bytes(), None).map_err(OutputError::Pdf)?;
    Ok(PrintSet {
        pages,
        pdf: out.bytes,
        stamps,
        notices,
        findings: Vec::new(),
        report: out.report,
    })
}

impl Session {
    /// The print set of the current snapshot ([`print_set_with`]) through the session's memo store.
    ///
    /// # Errors
    ///
    /// [`OutputError`] for a selection the page order does not have or pages the writer refuses.
    pub fn print_set(&self, options: &PrintOptions) -> Result<PrintSet, OutputError> {
        print_set_with(&self.query(), options, &self.export_assets())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn papers_parse_by_name_and_refuse_others_in_words() {
        assert_eq!(Paper::parse("a5"), Ok(Paper::A5));
        assert_eq!(Paper::parse("Sheet"), Ok(Paper::Sheet));
        assert_eq!(Paper::parse("tabloid"), Ok(Paper::Tabloid));
        assert_eq!(
            Paper::parse("A7").unwrap_err().to_string(),
            "unknown paper `A7` (sheet, A5, A4, A3, A2, A1, A0, Letter, Legal, Tabloid)"
        );
    }

    #[test]
    fn percent_rounds_half_up() {
        assert_eq!(percent((74, 105)), 70);
        assert_eq!(percent((70, 99)), 71);
        assert_eq!(percent((1, 200)), 1);
        assert_eq!(percent((1, 8)), 13);
        assert_eq!(percent((1, 1)), 100);
    }

    /// PR-1's and PR-2's factors: an A4 landscape sheet on A5 is min(210/297, 148/210) = 74/105 (70.48 %), an A3
    /// landscape sheet on A4 is 70/99 (70.71 %), both centred; A4 on A4 is 100 % with no stamp.
    #[test]
    fn the_fit_is_the_smaller_ratio_exactly_and_centred() {
        let a4 = (Nm::from_mm(297), Nm::from_mm(210));
        let (p, clipped) = placement(a4.0, a4.1, Paper::A5, Fit::ToPaper);
        assert!(!clipped);
        assert_eq!((p.width, p.height), (Nm::from_mm(210), Nm::from_mm(148)));
        assert_eq!(p.scale, (74, 105));
        // 297 mm × 74 / 105 = 209.314… mm, so 0.343 mm each side; the height fills the paper.
        assert_eq!(p.offset, Pt2::new(Nm(342_857), Nm(0)));
        assert_eq!(stamp_text(p.scale), "scaled 70 %");
        assert_eq!(p.overlay.items.len(), 1);
        let (p, _) = placement(Nm::from_mm(420), Nm::from_mm(297), Paper::A4, Fit::ToPaper);
        assert_eq!(p.scale, (70, 99));
        assert_eq!(stamp_text(p.scale), "scaled 71 %");
        let (p, _) = placement(a4.0, a4.1, Paper::A4, Fit::ToPaper);
        assert_eq!((p.scale, p.offset), ((1, 1), Pt2::ORIGIN));
        assert!(p.overlay.items.is_empty());
        // Never enlarged: an A4 sheet on A3 stays at 100 %, centred.
        let (p, _) = placement(a4.0, a4.1, Paper::A3, Fit::ToPaper);
        assert_eq!(p.scale, (1, 1));
        assert_eq!(p.offset, Pt2::new(Nm(61_500_000), Nm(43_500_000)));
        assert!(p.overlay.items.is_empty());
    }

    /// Actual size: the sheet at the paper's top-left corner, a larger sheet clipped; a portrait sheet turns the paper.
    #[test]
    fn actual_size_clips_a_larger_sheet_and_turns_the_paper_with_the_sheet() {
        let (p, clipped) = placement(Nm::from_mm(420), Nm::from_mm(297), Paper::A4, Fit::Actual);
        assert!(clipped);
        assert_eq!((p.scale, p.offset), ((1, 1), Pt2::ORIGIN));
        assert_eq!((p.width, p.height), (Nm::from_mm(297), Nm::from_mm(210)));
        assert!(p.overlay.items.is_empty());
        let (p, clipped) = placement(Nm::from_mm(210), Nm::from_mm(297), Paper::A4, Fit::Actual);
        assert!(!clipped);
        assert_eq!((p.width, p.height), (Nm::from_mm(210), Nm::from_mm(297)));
        let (p, clipped) = placement(Nm::from_mm(420), Nm::from_mm(297), Paper::Sheet, Fit::Actual);
        assert!(!clipped && p.is_identity());
        assert_eq!((p.width, p.height), (Nm::from_mm(420), Nm::from_mm(297)));
    }

    /// PR-2 in miniature (Golden A has no A3 sheet): an A3 landscape page printed on A4 with the fit carries
    /// "scaled 71 %", without it is clipped and the notice names the page; its content stays the page's item for item,
    /// and on its own sheet at actual size the set is the export's PDF byte for byte.
    #[test]
    fn an_a3_page_on_a4_is_scaled_to_71_percent_or_clipped_with_a_notice() {
        let mm = |x: i64, y: i64| Pt2::new(Nm::from_mm(x), Nm::from_mm(y));
        let frame = Item::path(
            ew_dl::Path::rect(ew_dl::ew_units::Rect::from_points(mm(10, 10), mm(410, 287))),
            Some(ew_dl::Stroke::pen(ew_dl::Pen::P026)),
            None,
        );
        let page = PdfPage {
            destination: ew_dl::Destination::Page(ew_derive::ew_ids::Uuid::from_u128(902)),
            label: "902".to_owned(),
            title: "902 Cabinet".to_owned(),
            section: None,
            width: Nm::from_mm(420),
            height: Nm::from_mm(297),
            content: DisplayList { items: vec![frame] },
            placement: None,
        };
        let options = ew_export::PdfOptions::new(PdfDate::ymd(2026, 1, 1));
        let fitted = print_pages(vec![page.clone()], options.clone(), Paper::A4, Fit::ToPaper).unwrap();
        assert_eq!(fitted.stamps, [Some("scaled 71 %".to_owned())]);
        assert!(fitted.notices.is_empty());
        assert_eq!(fitted.percent(0), 71);
        assert_eq!(fitted.pages[0].content, page.content);
        let clipped = print_pages(vec![page.clone()], options.clone(), Paper::A4, Fit::Actual).unwrap();
        assert_eq!(clipped.stamps, [None]);
        let notices: Vec<String> = clipped.notices.iter().map(ToString::to_string).collect();
        assert_eq!(
            notices,
            ["page 902 is larger than A4: it is printed at 100 % and clipped from its top-left corner"]
        );
        let sheet = print_pages(vec![page.clone()], options.clone(), Paper::Sheet, Fit::Actual).unwrap();
        let export = ew_export::pdf(&[page], &options, font_bytes(), None).unwrap();
        assert!(
            sheet.pdf == export.bytes,
            "a set on its own sheets is the export's PDF"
        );
        assert!(fitted.pdf != export.bytes && clipped.pdf != export.bytes);
    }

    /// The stamp: its baseline 4 mm above the bottom edge, its end 5 mm from the right edge, 2 mm cap height.
    #[test]
    fn the_stamp_sits_at_the_bottom_right_of_the_paper() {
        let (p, _) = placement(Nm::from_mm(297), Nm::from_mm(210), Paper::A5, Fit::ToPaper);
        let ew_dl::ItemKind::Glyphs(run) = &p.overlay.items[0].kind else {
            panic!("the stamp is a glyph run")
        };
        assert_eq!(run.text, "scaled 70 %");
        assert_eq!(run.origin.y, Nm::from_mm(148 - 4));
        let end = run.origin.x + run.layout_box.max().x;
        assert_eq!(end, Nm::from_mm(210 - 5));
        let cap = ew_text::bundled()
            .vertical_metrics(&TextStyle::regular(run.size))
            .cap_height;
        assert!((cap.0 - STAMP_CAP_HEIGHT.0).abs() <= 1, "{cap:?}");
    }
}
