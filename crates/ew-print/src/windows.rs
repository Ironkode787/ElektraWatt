// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Windows adapter (WP-S3.07 D12, rulings 1 to 9 of ID-1218): each placed page of the print set rendered by
//! `ew-render-cpu` in bands of [`BAND_ROWS`] device rows and sent to a printer through GDI.
//!
//! Without a printer named, the Windows print dialog (`PrintDlgExW`) chooses the printer and its settings; its owner is
//! a window of this process ([`Owner`], ID-1394 ruling 9), and its Cancel is [`PrintError::Cancelled`]. With a printer
//! named, its device context is opened with
//! its default settings and no dialog (`CreateDCW`). Then `StartDocW`, and per page: its paper and orientation asked
//! for by `ResetDCW` (D6's named sizes as `dmPaperSize` where Windows has one, else `dmPaperWidth` and
//! `dmPaperLength` in 0.1 mm; what the driver makes of the request is the driver's), `StartPage`, the bands rendered
//! at the lower of the device's horizontal and vertical resolutions (the placement's offset is from the paper's corner,
//! so the printable area's offset is subtracted) and each stretched by `StretchDIBits` to its device rectangle,
//! `EndPage`; then `EndDoc`. A band that would start below the device's paper is not sent, nor a band that is white
//! paper alone (FT-1158). The adapter's cancel signal ([`Windows::cancelled_by`], FT-1157) is read before each band:
//! once it is set, the document is aborted (`AbortDoc`) and the print is [`PrintError::Cancelled`]. With
//! [`PrintJob::monochrome`] the bands are drawn from the lists `ew_render_cpu::monochrome` gives, the greying the PDF
//! uses; the driver's colour setting is left as it is. A failure aborts the document. The print set's PDF is not used.
//!
//! The product's one place of `unsafe` (doc 18 §11.2, NFR-SEC-04; `ci/unsafe-allowlist.toml`): each block is one
//! system call, or one view of a buffer, with its `SAFETY:` comment; what a call allocates is owned by a guard that
//! frees it once ([`Dc`], [`Global`], [`Com`]).

#![allow(unsafe_code)]

use std::fmt;
use std::marker::PhantomData;
use std::mem::{align_of, offset_of, size_of};
use std::os::windows::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ::windows::Win32::Foundation::{ERROR_CANCELLED, ERROR_SUCCESS, GlobalFree, HGLOBAL, HWND, SetLastError};
use ::windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDCW, DEVMODEW, DEVMODEW_0_0, DIB_RGB_COLORS, DM_FORMNAME,
    DM_ORIENTATION, DM_PAPERLENGTH, DM_PAPERSIZE, DM_PAPERWIDTH, DM_SPECVERSION, DMORIENT_LANDSCAPE,
    DMORIENT_PORTRAIT, DMPAPER_A2, DMPAPER_A3, DMPAPER_A4, DMPAPER_A5, DMPAPER_LEGAL, DMPAPER_LETTER,
    DMPAPER_TABLOID, DeleteDC, GDI_ERROR, GET_DEVICE_CAPS_INDEX, GetDeviceCaps, HDC, LOGPIXELSX, LOGPIXELSY,
    PHYSICALHEIGHT, PHYSICALOFFSETX, PHYSICALOFFSETY, ResetDCW, SRCCOPY, StretchDIBits,
};
use ::windows::Win32::Storage::Xps::{AbortDoc, DOCINFOW, EndDoc, EndPage, StartDocW, StartPage};
use ::windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use ::windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use ::windows::Win32::UI::Controls::Dialogs::{
    DEVNAMES, PD_HIDEPRINTTOFILE, PD_NOCURRENTPAGE, PD_NOPAGENUMS, PD_NOSELECTION, PD_RESULT_PRINT,
    PD_RETURNDC, PD_USEDEVMODECOPIESANDCOLLATE, PRINTDLGEXW, PrintDlgExW, START_PAGE_GENERAL,
};
use ::windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GetForegroundWindow, GetWindowThreadProcessId, WS_EX_TOOLWINDOW, WS_POPUP,
};
use ::windows::core::{HRESULT, PCWSTR, w};
use ew_render_cpu::{PagePlacement, RenderOptions, RgbaImage, monochrome, pixel_size, render_placed_band};

use crate::{PdfPage, PrintAdapter, PrintError, PrintJob, PrintOutcome};

/// The device rows of one band (D12): about 10 MB of RGBA at 600 dpi on A3.
const BAND_ROWS: u32 = 256;

/// D6's named papers that Windows has a `DMPAPER_*` size for, by their short and long sides in nm, as
/// `ew_api::output::print::Paper` gives them; A1 and A0 have none and are asked for by their sides.
const NAMED_PAPERS: [(i64, i64, u32); 7] = [
    (148_000_000, 210_000_000, DMPAPER_A5),
    (210_000_000, 297_000_000, DMPAPER_A4),
    (297_000_000, 420_000_000, DMPAPER_A3),
    (420_000_000, 594_000_000, DMPAPER_A2),
    (215_900_000, 279_400_000, DMPAPER_LETTER),
    (215_900_000, 355_600_000, DMPAPER_LEGAL),
    (279_400_000, 431_800_000, DMPAPER_TABLOID),
];

/// The Windows adapter: the print dialog, or the printer [`PrintJob::printer`] names, then the pages through GDI.
#[derive(Clone, Debug, Default)]
pub struct Windows {
    /// The file the spooler writes the job to instead of the printer's port (DOCINFO's output); `None` but in WN-3's
    /// test.
    output: Option<PathBuf>,
    /// The cancel signal read before each band (FT-1157): set, the document is aborted.
    cancel: Arc<AtomicBool>,
}

impl Windows {
    /// The adapter as the binaries use it.
    pub fn new() -> Windows {
        Windows::default()
    }

    /// The adapter whose print ends, aborted, at the first band after `cancel` is set (FT-1157).
    pub fn cancelled_by(cancel: Arc<AtomicBool>) -> Windows {
        Windows {
            cancel,
            ..Windows::default()
        }
    }

    /// The adapter writing its jobs to `output` (ruling 7 of ID-1218: a print to "Microsoft Print to PDF" without its
    /// save dialog).
    #[cfg(test)]
    fn to_file(output: PathBuf) -> Windows {
        Windows {
            output: Some(output),
            ..Windows::default()
        }
    }
}

impl PrintAdapter for Windows {
    fn name(&self) -> &'static str {
        "windows"
    }

    fn print(&self, job: &PrintJob) -> Result<PrintOutcome, PrintError> {
        let (dc, mut mode, printer) = match &job.printer {
            Some(name) => (Dc::open(name)?, DevMode::blank(name), Some(name.clone())),
            None => dialog()?,
        };
        dc.start_doc(&job.title, self.output.as_deref())?;
        for page in &job.pages {
            if let Err(e) = send_page(&dc, &mut mode, page, job, &self.cancel) {
                dc.abort_doc();
                return Err(e);
            }
        }
        if !dc.end_doc() {
            return Err(failed(Step::EndDoc));
        }
        Ok(PrintOutcome::Spooled {
            printer,
            message: String::new(),
        })
    }
}

/// What failed, for [`PrintError::Windows`]: each step of a print in words.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step<'a> {
    /// The print dialog.
    Dialog,
    /// Opening the named printer.
    Open(&'a str),
    /// `StartDocW`.
    StartDoc,
    /// `ResetDCW` with the page's paper (its label).
    Paper(&'a str),
    /// The device's resolution.
    Resolution,
    /// `StartPage` (the page's label).
    StartPage(&'a str),
    /// Rendering a band of the page.
    Render(&'a str),
    /// `StretchDIBits` with a band of the page.
    Band(&'a str),
    /// `EndPage`.
    EndPage(&'a str),
    /// `EndDoc`.
    EndDoc,
}

impl fmt::Display for Step<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Step::Dialog => write!(f, "the Windows print dialog could not be shown"),
            Step::Open(printer) => write!(f, "the printer `{printer}` could not be opened"),
            Step::StartDoc => write!(f, "the printer did not start the document"),
            Step::Paper(page) => write!(f, "the printer did not take the paper of page {page}"),
            Step::Resolution => write!(f, "the printer reports no resolution"),
            Step::StartPage(page) => write!(f, "the printer did not start page {page}"),
            Step::Render(page) => write!(f, "page {page} could not be rendered"),
            Step::Band(page) => write!(f, "the printer did not take page {page}"),
            Step::EndPage(page) => write!(f, "the printer did not finish page {page}"),
            Step::EndDoc => write!(f, "the printer did not finish the document"),
        }
    }
}

/// `step` failed for `reason` (empty when the system gave none).
fn error(step: Step<'_>, reason: String) -> PrintError {
    PrintError::Windows {
        what: step.to_string(),
        reason,
    }
}

/// `step` failed; the reason is the thread's last error, empty when the call set none (each call below clears it
/// first).
fn failed(step: Step<'_>) -> PrintError {
    let code = HRESULT::from_thread();
    error(
        step,
        if code.is_ok() {
            String::new()
        } else {
            code.message()
        },
    )
}

/// `s` as a NUL-terminated wide string.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

/// One page: its paper asked for, then its bands between `StartPage` and `EndPage`, white ones not sent; `cancel` set
/// before a band is [`PrintError::Cancelled`].
fn send_page(
    dc: &Dc,
    mode: &mut DevMode,
    page: &PdfPage,
    job: &PrintJob,
    cancel: &AtomicBool,
) -> Result<(), PrintError> {
    let label = page.label.as_str();
    // An export page's paper is its sheet (the window's preview does the same).
    let mut placement = page.placement.clone().unwrap_or_else(|| PagePlacement {
        width: page.width,
        height: page.height,
        scale: (1, 1),
        offset: Default::default(),
        overlay: Default::default(),
    });
    let mono;
    let content = if job.monochrome {
        placement.overlay = monochrome(&placement.overlay);
        mono = monochrome(&page.content);
        &mono
    } else {
        &page.content
    };
    mode.ask_paper(placement.width.0, placement.height.0);
    if !dc.reset(mode) {
        return Err(failed(Step::Paper(label)));
    }
    let (x_dpi, y_dpi) = (dc.caps(LOGPIXELSX), dc.caps(LOGPIXELSY));
    let dpi = u32::try_from(x_dpi.min(y_dpi))
        .ok()
        .filter(|&d| d > 0)
        .ok_or_else(|| error(Step::Resolution, String::new()))?;
    let (left, top, device_height) = (
        dc.caps(PHYSICALOFFSETX),
        dc.caps(PHYSICALOFFSETY),
        dc.caps(PHYSICALHEIGHT),
    );
    let options = RenderOptions {
        dpi,
        ..RenderOptions::default()
    };
    let (columns, rows) = pixel_size(placement.paper(), dpi);
    if !dc.start_page() {
        return Err(failed(Step::StartPage(label)));
    }
    let mut first_row: u32 = 0;
    while u64::from(first_row) < rows {
        if cancel.load(Ordering::Relaxed) {
            return Err(PrintError::Cancelled);
        }
        let y = device(u64::from(first_row), y_dpi, dpi);
        if device_height > 0 && y >= device_height {
            break;
        }
        let (mut band, _) = render_placed_band(
            content, &placement, &options, first_row, BAND_ROWS, &job.fonts, None,
        )
        .map_err(|e| error(Step::Render(label), e.to_string()))?;
        if white(&band) {
            first_row += band.height;
            continue;
        }
        let bottom = device(u64::from(first_row) + u64::from(band.height), y_dpi, dpi);
        // RGBA to the B, G, R, unused order of a 32-bit BI_RGB bitmap; the background is opaque.
        for px in band.pixels.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
        }
        let rect = [-left, y - top, device(columns, x_dpi, dpi), bottom - y];
        if !dc.band(&band, rect) {
            return Err(failed(Step::Band(label)));
        }
        first_row += band.height;
    }
    if !dc.end_page() {
        return Err(failed(Step::EndPage(label)));
    }
    Ok(())
}

/// Whether `band` is white paper alone (every channel of every pixel at its full value), so sending it changes nothing
/// on the sheet (FT-1158).
fn white(band: &RgbaImage) -> bool {
    band.pixels.iter().all(|&c| c == u8::MAX)
}

/// `v` pixels at `dpi` in device pixels at `logical` per inch, rounded half up (the identity when they are equal).
fn device(v: u64, logical: i32, dpi: u32) -> i32 {
    let (logical, dpi) = (i128::from(logical), i128::from(dpi.max(1)));
    i32::try_from((i128::from(v) * logical + dpi / 2) / dpi).unwrap_or(i32::MAX)
}

/// The Windows print dialog (ruling 5 of ID-1218): the chosen printer's device context, its settings and its name
/// (`None` when the dialog's block holds none); Cancel is [`PrintError::Cancelled`].
fn dialog() -> Result<(Dc, DevMode, Option<String>), PrintError> {
    let _com = Com::init().map_err(|code| error(Step::Dialog, code.message()))?;
    let owner = Owner::new()?;
    let mut pd = PRINTDLGEXW {
        lStructSize: size_of::<PRINTDLGEXW>() as u32,
        hwndOwner: owner.0,
        // The page range was chosen before (the pre-dialog or `--pages`); the copies stay in the settings.
        Flags: PD_RETURNDC
            | PD_NOPAGENUMS
            | PD_NOSELECTION
            | PD_NOCURRENTPAGE
            | PD_HIDEPRINTTOFILE
            | PD_USEDEVMODECOPIESANDCOLLATE,
        nStartPage: START_PAGE_GENERAL,
        ..PRINTDLGEXW::default()
    };
    // SAFETY: `pd` is a PRINTDLGEXW of its own size (`lStructSize`) with every pointer member null (no page ranges,
    // template, callback or property pages; `hDevMode`, `hDevNames` and `hDC` null, so the dialog allocates them) and
    // `hwndOwner` a window handle; it is borrowed mutably for the call alone, and COM is initialised
    // apartment-threaded on this thread (`_com`).
    let shown = unsafe { PrintDlgExW(&mut pd) };
    // What the dialog allocated is freed on every way out, whatever it answered.
    let settings = Global(pd.hDevMode);
    let names = Global(pd.hDevNames);
    let dc = (!pd.hDC.is_invalid()).then_some(Dc(pd.hDC));
    shown.map_err(|e| error(Step::Dialog, e.message()))?;
    if pd.dwResultAction != PD_RESULT_PRINT {
        return Err(PrintError::Cancelled);
    }
    let unanswered = || error(Step::Dialog, "it gave no printer".to_owned());
    let dc = dc.ok_or_else(unanswered)?;
    let mode = settings
        .bytes()
        .and_then(|b| DevMode::from_bytes(&b))
        .ok_or_else(unanswered)?;
    let printer = names.bytes().and_then(|b| device_name(&b));
    Ok((dc, mode, printer))
}

/// The print dialog's owner (FT-1150, FT-1159; ID-1394 ruling 9): a window of this process, never another's. A modal
/// dialog disables its owner while it shows, and Windows lets a window of one process own a window of another, tying
/// their input together, so an `ew print` killed while its dialog showed would leave the terminal that ran it
/// disabled. When the foreground window is this process's (the main window the person chose Print… in), it is the
/// owner; otherwise (`ew print`, or a window that lost the foreground meanwhile) the owner is a hidden window of this
/// process, made for the dialog and destroyed after it. An elevated application is the same case: its own windows share
/// its integrity level, and a foreground window of another process, at whatever level, is never the owner (between
/// levels, User Interface Privilege Isolation blocks the messages of the lower to the higher).
struct Owner(HWND, bool);

impl Owner {
    /// The foreground window when it is this process's, else a hidden window of its own.
    fn new() -> Result<Owner, PrintError> {
        // SAFETY: `GetForegroundWindow` takes no argument and returns a handle, null when there is none.
        let foreground = unsafe { GetForegroundWindow() };
        if !foreground.is_invalid() && ours(window_process(foreground)) {
            return Ok(Owner(foreground, false));
        }
        // SAFETY: the class is the system's static `STATIC` (no instance needed), the strings are a static literal and
        // null, there is no parent, menu or creation data; the window is a zero-size popup that is never shown, and
        // this guard destroys it, on this thread (not `Send`).
        let hidden = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                w!("STATIC"),
                PCWSTR::null(),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                None,
                None,
            )
        };
        hidden
            .map(|h| Owner(h, true))
            .map_err(|e| error(Step::Dialog, e.message()))
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        if self.1 {
            // SAFETY: the window was created by this guard on this thread and nothing else destroys it.
            let _ = unsafe { DestroyWindow(self.0) };
        }
    }
}

/// The process of `window`; 0 when the handle names no window.
fn window_process(window: HWND) -> u32 {
    let mut process = 0;
    // SAFETY: `process` is a live `u32` the call writes the id to; an invalid handle only returns 0 and writes none.
    unsafe { GetWindowThreadProcessId(window, Some(&mut process)) };
    process
}

/// Whether `process` is this one (the owner rule of [`Owner`]).
fn ours(process: u32) -> bool {
    process == std::process::id()
}

/// The printer's name in a DEVNAMES block: the NUL-terminated wide string at its device offset (in characters from
/// the block's start).
fn device_name(bytes: &[u8]) -> Option<String> {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| u16::from_ne_bytes(c))
        .collect();
    let at = usize::from(*units.get(offset_of!(DEVNAMES, wDeviceOffset) / 2)?);
    let name = units.get(at..)?;
    let end = name.iter().position(|&u| u == 0)?;
    Some(String::from_utf16_lossy(&name[..end]))
}

/// COM initialised apartment-threaded on this thread for the print dialog, uninitialised when dropped. Not `Send`, so
/// it is dropped on the thread that initialised it.
struct Com(PhantomData<*const ()>);

impl Com {
    /// COM on this thread, or the system's refusal (a thread already in the multi-threaded apartment).
    fn init() -> Result<Com, HRESULT> {
        // SAFETY: the reserved argument is `None`, as required; a success (`S_OK`, or `S_FALSE` when this thread had
        // COM already) is balanced by the one `CoUninitialize` of the guard's drop.
        let code = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if code.is_ok() {
            Ok(Com(PhantomData))
        } else {
            Err(code)
        }
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        // SAFETY: the guard exists only after a successful `CoInitializeEx` on this thread, which it never leaves (not
        // `Send`), and is dropped once.
        unsafe { CoUninitialize() }
    }
}

/// A global memory block the print dialog allocated (its settings or its printer's names), freed when dropped.
struct Global(HGLOBAL);

impl Global {
    /// A copy of the block's bytes; `None` for a null handle or a block that does not lock.
    fn bytes(&self) -> Option<Vec<u8>> {
        if self.0.is_invalid() {
            return None;
        }
        // SAFETY: the handle is a block the dialog allocated, which only this guard frees (at its drop, after this
        // borrow); `GlobalLock` returns a pointer to its first byte (null on failure) that stays valid for
        // `GlobalSize` bytes until `GlobalUnlock`, and the bytes are copied out before the unlock.
        unsafe {
            let first = GlobalLock(self.0).cast::<u8>();
            if first.is_null() {
                return None;
            }
            let bytes = std::slice::from_raw_parts(first, GlobalSize(self.0)).to_vec();
            // It reports an error when the lock count reaches zero, which is the expected end.
            let _ = GlobalUnlock(self.0);
            Some(bytes)
        }
    }
}

impl Drop for Global {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: the dialog allocated the block for the caller to free, nothing else frees it, and it is not
            // locked (`bytes` unlocks before it returns).
            let _ = unsafe { GlobalFree(Some(self.0)) };
        }
    }
}

/// A printer's settings: a DEVMODEW, then `dmDriverExtra` bytes of its driver's, in a buffer of whole `u64`s (so
/// aligned for DEVMODEW) that is never shorter than one DEVMODEW.
struct DevMode(Vec<u64>);

const _: () = assert!(align_of::<DEVMODEW>() <= align_of::<u64>());

impl DevMode {
    /// Settings with the printer's name (truncated to the field) and their own size alone: a request with them asks
    /// for what they set and leaves the rest to the driver's defaults (a printer named, no dialog).
    fn blank(printer: &str) -> DevMode {
        let mut mode = DevMode(vec![0; size_of::<DEVMODEW>().div_ceil(8)]);
        let name: Vec<u8> = printer
            .encode_utf16()
            .take(31)
            .flat_map(u16::to_ne_bytes)
            .collect();
        mode.put(offset_of!(DEVMODEW, dmDeviceName), &name);
        mode.put(
            offset_of!(DEVMODEW, dmSpecVersion),
            &(DM_SPECVERSION.0 as u16).to_ne_bytes(),
        );
        mode.put(
            offset_of!(DEVMODEW, dmSize),
            &(size_of::<DEVMODEW>() as u16).to_ne_bytes(),
        );
        mode
    }

    /// The settings in `bytes` (the print dialog's block); `None` when the block is shorter than its own
    /// `dmSize + dmDriverExtra`, or its public part ends before the paper fields.
    fn from_bytes(bytes: &[u8]) -> Option<DevMode> {
        let field = |at: usize| {
            bytes
                .get(at..at + 2)
                .map(|b| usize::from(u16::from_ne_bytes([b[0], b[1]])))
        };
        let public = field(offset_of!(DEVMODEW, dmSize))?;
        let total = public + field(offset_of!(DEVMODEW, dmDriverExtra))?;
        if public < offset_of!(DEVMODEW, dmColor) || bytes.len() < total {
            return None;
        }
        let mut words = vec![0u64; total.max(size_of::<DEVMODEW>()).div_ceil(8)];
        for (word, chunk) in words.iter_mut().zip(bytes[..total].chunks(8)) {
            let mut b = [0u8; 8];
            b[..chunk.len()].copy_from_slice(chunk);
            *word = u64::from_ne_bytes(b);
        }
        Some(DevMode(words))
    }

    /// Writes `bytes` at the byte offset `at` of the settings (a DEVMODEW field at its `offset_of!` position, FT-1151:
    /// no view of the buffer as a DEVMODEW); every field lies inside the buffer, which is never shorter than one.
    fn put(&mut self, at: usize, bytes: &[u8]) {
        for (k, &b) in (at..).zip(bytes) {
            let mut word = self.0[k / 8].to_ne_bytes();
            word[k % 8] = b;
            self.0[k / 8] = u64::from_ne_bytes(word);
        }
    }

    /// The four bytes at the byte offset `at` of the settings.
    fn get4(&self, at: usize) -> [u8; 4] {
        std::array::from_fn(|i| self.0[(at + i) / 8].to_ne_bytes()[(at + i) % 8])
    }

    /// The settings as GDI reads them: the DEVMODEW, then its driver's bytes, all inside the buffer.
    fn as_ptr(&self) -> *const DEVMODEW {
        self.0.as_ptr().cast()
    }

    /// Asks for a paper of `width` × `height` nm (ruling 3 of ID-1218): D6's named sizes as `dmPaperSize` where
    /// Windows has one, else `dmPaperWidth` and `dmPaperLength` in 0.1 mm (the short and the long side); landscape
    /// when wider than high. The form name the dialog may have set is cleared, so a driver that ranks it first never
    /// prints on the dialog's form (ID-1226).
    fn ask_paper(&mut self, width: i64, height: i64) {
        let (short, long) = (width.min(height), width.max(height));
        let tenths = |nm: i64| i16::try_from((nm + 50_000) / 100_000).unwrap_or(i16::MAX);
        let named = NAMED_PAPERS
            .iter()
            .find(|p| (p.0, p.1) == (short, long))
            .map(|p| p.2);
        let orientation = if width > height {
            DMORIENT_LANDSCAPE
        } else {
            DMORIENT_PORTRAIT
        };
        let paper = |field: usize| offset_of!(DEVMODEW, Anonymous1) + field;
        let flags = offset_of!(DEVMODEW, dmFields);
        let mut fields = u32::from_ne_bytes(self.get4(flags)) & !DM_FORMNAME.0;
        self.put(
            paper(offset_of!(DEVMODEW_0_0, dmOrientation)),
            &(orientation as i16).to_ne_bytes(),
        );
        self.put(offset_of!(DEVMODEW, dmFormName), &[0; 64]);
        match named {
            Some(size) => {
                fields = (fields | DM_ORIENTATION.0 | DM_PAPERSIZE.0) & !(DM_PAPERLENGTH.0 | DM_PAPERWIDTH.0);
                self.put(
                    paper(offset_of!(DEVMODEW_0_0, dmPaperSize)),
                    &(size as i16).to_ne_bytes(),
                );
            }
            None => {
                fields = (fields | DM_ORIENTATION.0 | DM_PAPERLENGTH.0 | DM_PAPERWIDTH.0) & !DM_PAPERSIZE.0;
                self.put(paper(offset_of!(DEVMODEW_0_0, dmPaperSize)), &0i16.to_ne_bytes());
                self.put(
                    paper(offset_of!(DEVMODEW_0_0, dmPaperWidth)),
                    &tenths(short).to_ne_bytes(),
                );
                self.put(
                    paper(offset_of!(DEVMODEW_0_0, dmPaperLength)),
                    &tenths(long).to_ne_bytes(),
                );
            }
        }
        self.put(flags, &fields.to_ne_bytes());
    }
}

/// A printer's device context, deleted when dropped.
struct Dc(HDC);

impl Dc {
    /// The device context of the printer `name` with its default settings (no dialog).
    fn open(name: &str) -> Result<Dc, PrintError> {
        let device = wide(name);
        // SAFETY: the driver is a static NUL-terminated literal, `device` is NUL-terminated and outlives the call, the
        // port is null and no settings are passed (the printer's defaults); `SetLastError` only clears the thread's
        // last error first.
        let dc = unsafe {
            SetLastError(ERROR_SUCCESS);
            CreateDCW(w!("WINSPOOL"), PCWSTR(device.as_ptr()), PCWSTR::null(), None)
        };
        if dc.is_invalid() {
            return Err(failed(Step::Open(name)));
        }
        Ok(Dc(dc))
    }

    /// A capability of the device (`GetDeviceCaps`).
    fn caps(&self, index: GET_DEVICE_CAPS_INDEX) -> i32 {
        // SAFETY: the device context is live (this guard deletes it, at its drop); the index is one of GDI's.
        unsafe { GetDeviceCaps(Some(self.0), index) }
    }

    /// Starts the document `title`, written to `output` instead of the printer's port when given (`StartDocW`); the
    /// person's cancel of the driver's own prompt (a file name) is [`PrintError::Cancelled`].
    fn start_doc(&self, title: &str, output: Option<&Path>) -> Result<(), PrintError> {
        let title = wide(title);
        let output: Option<Vec<u16>> = output.map(|p| p.as_os_str().encode_wide().chain([0]).collect());
        let info = DOCINFOW {
            cbSize: size_of::<DOCINFOW>() as i32,
            lpszDocName: PCWSTR(title.as_ptr()),
            lpszOutput: output.as_ref().map_or(PCWSTR::null(), |o| PCWSTR(o.as_ptr())),
            ..DOCINFOW::default()
        };
        // SAFETY: the device context is live; `info` is a DOCINFOW of its own size whose strings are NUL-terminated
        // wide strings (made just above) that outlive the call, and whose data type is null; `SetLastError` only
        // clears the thread's last error first.
        let job = unsafe {
            SetLastError(ERROR_SUCCESS);
            StartDocW(self.0, &info)
        };
        if job > 0 {
            return Ok(());
        }
        if HRESULT::from_thread() == HRESULT::from_win32(ERROR_CANCELLED.0) {
            return Err(PrintError::Cancelled);
        }
        Err(failed(Step::StartDoc))
    }

    /// Asks for the settings `mode` between pages (`ResetDCW`).
    fn reset(&self, mode: &DevMode) -> bool {
        // SAFETY: the device context is live and not inside a page; `mode` points to a DEVMODEW followed by its
        // `dmDriverExtra` bytes, all inside its buffer (`DevMode`), which outlives the call; `SetLastError` only clears
        // the thread's last error first.
        unsafe {
            SetLastError(ERROR_SUCCESS);
            !ResetDCW(self.0, mode.as_ptr()).is_invalid()
        }
    }

    /// Starts a page (`StartPage`).
    fn start_page(&self) -> bool {
        // SAFETY: the device context is live and inside a document; `SetLastError` only clears the last error first.
        unsafe {
            SetLastError(ERROR_SUCCESS);
            StartPage(self.0) > 0
        }
    }

    /// Sends `band` (B, G, R, unused per pixel, top row first) stretched over the device rectangle `[x, y, width,
    /// height]` (`StretchDIBits`); `false` when GDI refuses it or the pixels do not fill the band.
    fn band(&self, band: &RgbaImage, rect: [i32; 4]) -> bool {
        let (Ok(columns), Ok(rows)) = (i32::try_from(band.width), i32::try_from(band.height)) else {
            return false;
        };
        let size = (band.width as usize)
            .checked_mul(band.height as usize)
            .and_then(|n| n.checked_mul(4));
        if size != Some(band.pixels.len()) {
            return false;
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: columns,
                // Negative: the rows run top-down, as the band's do.
                biHeight: -rows,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..BITMAPINFOHEADER::default()
            },
            ..BITMAPINFO::default()
        };
        // SAFETY: the device context is live and inside a page; `info` describes a top-down 32-bit BI_RGB bitmap of
        // `columns` × `rows` pixels, whose bytes `band.pixels` holds exactly (checked above) and keeps alive across the
        // call; with 32 bits per pixel and DIB_RGB_COLORS no colour table is read; `SetLastError` only clears the
        // thread's last error first.
        let lines = unsafe {
            SetLastError(ERROR_SUCCESS);
            StretchDIBits(
                self.0,
                rect[0],
                rect[1],
                rect[2],
                rect[3],
                0,
                0,
                columns,
                rows,
                Some(band.pixels.as_ptr().cast()),
                &info,
                DIB_RGB_COLORS,
                SRCCOPY,
            )
        };
        lines != 0 && lines != GDI_ERROR
    }

    /// Ends a page (`EndPage`).
    fn end_page(&self) -> bool {
        // SAFETY: the device context is live and inside a page; `SetLastError` only clears the last error first.
        unsafe {
            SetLastError(ERROR_SUCCESS);
            EndPage(self.0) > 0
        }
    }

    /// Ends the document (`EndDoc`), which hands the job to the spooler.
    fn end_doc(&self) -> bool {
        // SAFETY: the device context is live and inside a document; `SetLastError` only clears the last error first.
        unsafe {
            SetLastError(ERROR_SUCCESS);
            EndDoc(self.0) > 0
        }
    }

    /// Abandons the document after a failure (`AbortDoc`); its own failure leaves nothing more to do.
    fn abort_doc(&self) {
        // SAFETY: the device context is live and inside a document.
        let _ = unsafe { AbortDoc(self.0) };
    }
}

impl Drop for Dc {
    fn drop(&mut self) {
        // SAFETY: the device context was created for this guard (`CreateDCW` or the dialog's `PD_RETURNDC`), nothing
        // else deletes it, and it is deleted once.
        let _ = unsafe { DeleteDC(self.0) };
    }
}

#[cfg(test)]
mod tests;
