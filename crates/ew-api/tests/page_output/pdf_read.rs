// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A minimal PDF reader for the acceptance harness (doc 18 §3.9; FR-OUT-008, AC-01, AC-06, AC-13): it reads
//! back the PDF the project itself writes (krilla 0.8, ew-export) and returns, per page, the **text layer**
//! (content streams decoded through the fonts' ToUnicode maps) and the **stroked line segments** in printed
//! mm.
//!
//! Test code only: it never ships, and it reads bytes, not the model (the raw-read policy of `ew-model` is
//! not involved). *As built (cycle 14):* doc 18 §3.9 names hayro's parser for the frame check; this
//! in-repo reader covers the subset our writer produces without a new dependency (open ruling of the
//! cycle-14 report). What it supports, and refuses with an error otherwise:
//!
//! - **File:** classic cross-reference tables and indirect objects read in file order (no object streams,
//!   no encryption, no incremental-update semantics beyond "the later object wins"); streams with
//!   `/FlateDecode` or no filter.
//! - **Pages:** the page tree from `/Root /Pages` in `/Kids` order; `/Resources` and `/MediaBox` inherited.
//! - **Content:** `q Q cm w`, path construction `m l c v y h re`, painting `S s B B* b b* f F f* n`, clipping
//!   `W W*`; text `BT ET Tf Tm Td TD T* TL Tc Tw Tz Ts Tr Tj TJ ' "`; marked content `BDC BMC EMC` with
//!   `/ActualText` replacing the shown text; Form XObjects through `Do`. Colour and state operators are
//!   skipped. Inline images (`BI`) are an error.
//! - **Text:** one run per `BT … ET` (krilla writes one glyph run per text object); every code is mapped
//!   through the font's ToUnicode CMap (`bfchar`, `bfrange` with an offset or an array), the code length
//!   from its code-space range; a code without a mapping becomes U+FFFD, so a lost character is visible.
//! - **Segments:** every straight segment of a stroked path (`l`, `h`, `re`), mapped through the CTM to
//!   page space and then to mm with y down from the top of the media box; curves are not segments.

use std::collections::BTreeMap;
use std::io::Read;

/// 1 pt in mm.
const MM_PER_PT: f64 = 25.4 / 72.0;

/// A PDF object.
#[derive(Clone, Debug, PartialEq)]
pub enum Obj {
    Null,
    Bool(bool),
    Int(i64),
    Real(f64),
    Name(Vec<u8>),
    Str(Vec<u8>),
    Array(Vec<Obj>),
    Dict(BTreeMap<Vec<u8>, Obj>),
    Ref(u32),
    /// A stream: its dictionary and its raw (still encoded) bytes.
    Stream(BTreeMap<Vec<u8>, Obj>, Vec<u8>),
    /// A content-stream operator (only inside content streams).
    Op(Vec<u8>),
}

impl Obj {
    fn num(&self) -> Option<f64> {
        match self {
            Obj::Int(i) => Some(*i as f64),
            Obj::Real(r) => Some(*r),
            _ => None,
        }
    }

    fn dict(&self) -> Option<&BTreeMap<Vec<u8>, Obj>> {
        match self {
            Obj::Dict(d) | Obj::Stream(d, _) => Some(d),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Lexer and object parser

fn is_white(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'\x0c' | 0)
}

fn is_delim(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

struct Lexer<'a> {
    b: &'a [u8],
    pos: usize,
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Int(i64),
    Real(f64),
    Name(Vec<u8>),
    Str(Vec<u8>),
    ArrOpen,
    ArrClose,
    DictOpen,
    DictClose,
    Word(Vec<u8>),
}

impl<'a> Lexer<'a> {
    fn new(b: &'a [u8]) -> Lexer<'a> {
        Lexer { b, pos: 0 }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.b.len() {
            let c = self.b[self.pos];
            if is_white(c) {
                self.pos += 1;
            } else if c == b'%' {
                while self.pos < self.b.len() && !matches!(self.b[self.pos], b'\r' | b'\n') {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn next(&mut self) -> Result<Option<Tok>, String> {
        self.skip_ws();
        let Some(&c) = self.b.get(self.pos) else {
            return Ok(None);
        };
        let start = self.pos;
        Ok(Some(match c {
            b'[' => {
                self.pos += 1;
                Tok::ArrOpen
            }
            b']' => {
                self.pos += 1;
                Tok::ArrClose
            }
            b'<' if self.b.get(self.pos + 1) == Some(&b'<') => {
                self.pos += 2;
                Tok::DictOpen
            }
            b'>' if self.b.get(self.pos + 1) == Some(&b'>') => {
                self.pos += 2;
                Tok::DictClose
            }
            b'<' => Tok::Str(self.hex()?),
            b'(' => Tok::Str(self.literal()?),
            b'/' => {
                self.pos += 1;
                let mut name = Vec::new();
                while let Some(&c) = self.b.get(self.pos) {
                    if is_white(c) || is_delim(c) {
                        break;
                    }
                    if c == b'#' && self.pos + 2 < self.b.len() {
                        let h = std::str::from_utf8(&self.b[self.pos + 1..self.pos + 3]).unwrap_or("");
                        if let Ok(v) = u8::from_str_radix(h, 16) {
                            name.push(v);
                            self.pos += 3;
                            continue;
                        }
                    }
                    name.push(c);
                    self.pos += 1;
                }
                Tok::Name(name)
            }
            b')' | b'>' | b'{' | b'}' => return Err(format!("unexpected `{}` at {start}", c as char)),
            _ => {
                while let Some(&c) = self.b.get(self.pos) {
                    if is_white(c) || is_delim(c) {
                        break;
                    }
                    self.pos += 1;
                }
                let word = &self.b[start..self.pos];
                number(word).unwrap_or_else(|| Tok::Word(word.to_vec()))
            }
        }))
    }

    fn hex(&mut self) -> Result<Vec<u8>, String> {
        self.pos += 1;
        let mut digits = Vec::new();
        loop {
            let Some(&c) = self.b.get(self.pos) else {
                return Err("unterminated hex string".into());
            };
            self.pos += 1;
            match c {
                b'>' => break,
                c if is_white(c) => {}
                c if c.is_ascii_hexdigit() => digits.push(c),
                c => return Err(format!("bad hex digit `{}`", c as char)),
            }
        }
        if digits.len() % 2 == 1 {
            digits.push(b'0');
        }
        Ok(digits
            .chunks(2)
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect())
    }

    fn literal(&mut self) -> Result<Vec<u8>, String> {
        self.pos += 1;
        let mut out = Vec::new();
        let mut depth = 1;
        loop {
            let Some(&c) = self.b.get(self.pos) else {
                return Err("unterminated literal string".into());
            };
            self.pos += 1;
            match c {
                b'(' => {
                    depth += 1;
                    out.push(c);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    out.push(c);
                }
                b'\\' => {
                    let Some(&e) = self.b.get(self.pos) else {
                        return Err("unterminated escape".into());
                    };
                    self.pos += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'\r' => {
                            if self.b.get(self.pos) == Some(&b'\n') {
                                self.pos += 1;
                            }
                        }
                        b'\n' => {}
                        b'0'..=b'7' => {
                            let mut v = u32::from(e - b'0');
                            for _ in 0..2 {
                                match self.b.get(self.pos) {
                                    Some(&d @ b'0'..=b'7') => {
                                        v = v * 8 + u32::from(d - b'0');
                                        self.pos += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push((v & 0xff) as u8);
                        }
                        other => out.push(other),
                    }
                }
                c => out.push(c),
            }
        }
        Ok(out)
    }
}

fn number(w: &[u8]) -> Option<Tok> {
    let s = std::str::from_utf8(w).ok()?;
    let body = s.strip_prefix(['+', '-']).unwrap_or(s);
    if body.is_empty() || !body.bytes().all(|c| c.is_ascii_digit() || c == b'.') {
        return None;
    }
    if body.contains('.') {
        let v: f64 = if body == "." {
            return None;
        } else {
            let t = if s.ends_with('.') {
                format!("{s}0")
            } else {
                s.to_owned()
            };
            let t = t.replacen("-.", "-0.", 1).replacen("+.", "0.", 1);
            let t = if t.starts_with('.') { format!("0{t}") } else { t };
            t.parse().ok()?
        };
        Some(Tok::Real(v))
    } else {
        s.parse().ok().map(Tok::Int)
    }
}

/// Parses objects from tokens; `ops` turns unknown words into [`Obj::Op`] (content streams).
struct Parser<'a> {
    lx: Lexer<'a>,
    ops: bool,
}

impl<'a> Parser<'a> {
    fn new(b: &'a [u8], ops: bool) -> Parser<'a> {
        Parser {
            lx: Lexer::new(b),
            ops,
        }
    }

    /// The next object, or `None` at the end.
    fn obj(&mut self) -> Result<Option<Obj>, String> {
        let Some(t) = self.lx.next()? else {
            return Ok(None);
        };
        self.parse_tok(t).map(Some)
    }

    fn parse_tok(&mut self, t: Tok) -> Result<Obj, String> {
        Ok(match t {
            Tok::Int(i) => {
                if !self.ops && i >= 0 {
                    // `n g R`: look ahead two tokens.
                    let save = self.lx.pos;
                    if let (Ok(Some(Tok::Int(_))), Ok(Some(Tok::Word(w)))) = (self.lx.next(), self.lx.next())
                        && w == b"R"
                    {
                        return Ok(Obj::Ref(u32::try_from(i).map_err(|_| "object number too large")?));
                    }
                    self.lx.pos = save;
                }
                Obj::Int(i)
            }
            Tok::Real(r) => Obj::Real(r),
            Tok::Name(n) => Obj::Name(n),
            Tok::Str(s) => Obj::Str(s),
            Tok::ArrOpen => {
                let mut items = Vec::new();
                loop {
                    match self.lx.next()? {
                        None => return Err("unterminated array".into()),
                        Some(Tok::ArrClose) => break,
                        Some(t) => items.push(self.parse_tok(t)?),
                    }
                }
                Obj::Array(items)
            }
            Tok::DictOpen => {
                let mut d = BTreeMap::new();
                loop {
                    match self.lx.next()? {
                        None => return Err("unterminated dictionary".into()),
                        Some(Tok::DictClose) => break,
                        Some(Tok::Name(k)) => {
                            let t = self.lx.next()?.ok_or("dictionary value missing")?;
                            let v = self.parse_tok(t)?;
                            d.insert(k, v);
                        }
                        Some(t) => return Err(format!("dictionary key expected, found {t:?}")),
                    }
                }
                Obj::Dict(d)
            }
            Tok::ArrClose | Tok::DictClose => return Err("unbalanced `]` or `>>`".into()),
            Tok::Word(w) => match w.as_slice() {
                b"true" => Obj::Bool(true),
                b"false" => Obj::Bool(false),
                b"null" => Obj::Null,
                _ if self.ops => Obj::Op(w),
                _ => return Err(format!("unexpected keyword `{}`", String::from_utf8_lossy(&w))),
            },
        })
    }
}

// ---------------------------------------------------------------------------------------------------------
// File

/// A parsed PDF: its indirect objects and its trailer.
pub struct PdfFile {
    objects: BTreeMap<u32, Obj>,
    trailer: BTreeMap<Vec<u8>, Obj>,
}

impl PdfFile {
    /// Reads every indirect object in file order (see the module documentation for what is supported).
    pub fn parse(bytes: &[u8]) -> Result<PdfFile, String> {
        if !bytes.starts_with(b"%PDF-") {
            return Err("not a PDF (no %PDF- header)".into());
        }
        let mut p = Parser::new(bytes, false);
        let mut objects = BTreeMap::new();
        let mut trailer = BTreeMap::new();
        while let Some(t) = p.lx.next()? {
            match t {
                Tok::Word(w) if w == b"xref" => {
                    // Skip the table: everything up to the trailer.
                    loop {
                        match p.lx.next()? {
                            Some(Tok::Word(w)) if w == b"trailer" => break,
                            Some(_) => {}
                            None => return Err("xref without trailer".into()),
                        }
                    }
                    match p.obj()? {
                        Some(Obj::Dict(d)) => trailer.extend(d),
                        _ => return Err("trailer is not a dictionary".into()),
                    }
                }
                Tok::Word(w) if w == b"startxref" => {
                    p.lx.next()?;
                }
                Tok::Int(num) => {
                    let gen_ok = matches!(p.lx.next()?, Some(Tok::Int(_)));
                    let obj_ok = matches!(p.lx.next()?, Some(Tok::Word(w)) if w == b"obj");
                    if !gen_ok || !obj_ok {
                        return Err(format!("object {num}: `n g obj` expected"));
                    }
                    let value = p.obj()?.ok_or("object body missing")?;
                    let save = p.lx.pos;
                    let value = match (value, p.lx.next()?) {
                        (Obj::Dict(d), Some(Tok::Word(w))) if w == b"stream" => {
                            let data = stream_data(bytes, &mut p.lx, &d)?;
                            Obj::Stream(d, data)
                        }
                        (v, _) => {
                            p.lx.pos = save;
                            v
                        }
                    };
                    match p.lx.next()? {
                        Some(Tok::Word(w)) if w == b"endobj" => {}
                        other => return Err(format!("object {num}: endobj expected, found {other:?}")),
                    }
                    objects.insert(u32::try_from(num).map_err(|_| "bad object number")?, value);
                }
                other => return Err(format!("unexpected token {other:?} at file level")),
            }
        }
        Ok(PdfFile { objects, trailer })
    }

    /// `o` with references followed.
    fn get<'b>(&'b self, mut o: &'b Obj) -> &'b Obj {
        for _ in 0..32 {
            match o {
                Obj::Ref(n) => o = self.objects.get(n).unwrap_or(&Obj::Null),
                _ => return o,
            }
        }
        &Obj::Null
    }

    fn key<'b>(&'b self, d: &'b BTreeMap<Vec<u8>, Obj>, k: &str) -> &'b Obj {
        self.get(d.get(k.as_bytes()).unwrap_or(&Obj::Null))
    }

    /// The decoded bytes of a stream object.
    fn decode(&self, o: &Obj) -> Result<Vec<u8>, String> {
        let Obj::Stream(d, raw) = self.get(o) else {
            return Err("a stream was expected".into());
        };
        let filters: Vec<Vec<u8>> = match self.key(d, "Filter") {
            Obj::Null => Vec::new(),
            Obj::Name(n) => vec![n.clone()],
            Obj::Array(a) => a
                .iter()
                .filter_map(|f| match self.get(f) {
                    Obj::Name(n) => Some(n.clone()),
                    _ => None,
                })
                .collect(),
            other => return Err(format!("bad /Filter {other:?}")),
        };
        let mut data = raw.clone();
        for f in filters {
            if f != b"FlateDecode" {
                return Err(format!("unsupported filter /{}", String::from_utf8_lossy(&f)));
            }
            let mut out = Vec::new();
            flate2::read::ZlibDecoder::new(data.as_slice())
                .read_to_end(&mut out)
                .map_err(|e| format!("flate: {e}"))?;
            data = out;
        }
        Ok(data)
    }

    /// The pages in page-tree order, each with its text layer and stroked segments.
    pub fn pages(&self) -> Result<Vec<PageRead>, String> {
        let root = self.key(&self.trailer, "Root").dict().ok_or("no /Root")?;
        let tree = self.key(root, "Pages");
        let mut leaves = Vec::new();
        self.collect(tree, None, None, &mut leaves, 0)?;
        let mut fonts = FontCache::default();
        leaves
            .into_iter()
            .map(|(page, resources, media)| {
                let media = media.ok_or("a page without /MediaBox")?;
                let mut gs = Interp {
                    file: self,
                    fonts: &mut fonts,
                    media,
                    out: PageRead {
                        width_mm: (media[2] - media[0]) * MM_PER_PT,
                        height_mm: (media[3] - media[1]) * MM_PER_PT,
                        runs: Vec::new(),
                        segments: Vec::new(),
                    },
                };
                let contents: Vec<&Obj> = match self.key(page, "Contents") {
                    Obj::Array(a) => a.iter().collect(),
                    Obj::Null => Vec::new(),
                    _ => vec![page.get(b"Contents".as_slice()).unwrap()],
                };
                let mut data = Vec::new();
                for c in contents {
                    data.extend(self.decode(c)?);
                    data.push(b'\n');
                }
                gs.run(&data, resources, IDENTITY, 0)?;
                Ok(gs.out)
            })
            .collect()
    }

    #[allow(clippy::type_complexity)]
    fn collect<'b>(
        &'b self,
        node: &'b Obj,
        resources: Option<&'b BTreeMap<Vec<u8>, Obj>>,
        media: Option<[f64; 4]>,
        out: &mut Vec<(
            &'b BTreeMap<Vec<u8>, Obj>,
            Option<&'b BTreeMap<Vec<u8>, Obj>>,
            Option<[f64; 4]>,
        )>,
        depth: usize,
    ) -> Result<(), String> {
        if depth > 64 {
            return Err("page tree too deep".into());
        }
        let d = self
            .get(node)
            .dict()
            .ok_or("a page-tree node is not a dictionary")?;
        let resources = self.key(d, "Resources").dict().or(resources);
        let media = match self.key(d, "MediaBox") {
            Obj::Array(a) if a.len() == 4 => {
                let v: Vec<f64> = a.iter().filter_map(|x| self.get(x).num()).collect();
                (v.len() == 4).then(|| [v[0], v[1], v[2], v[3]]).or(media)
            }
            _ => media,
        };
        match self.key(d, "Type") {
            Obj::Name(n) if n == b"Pages" => {
                let Obj::Array(kids) = self.key(d, "Kids") else {
                    return Err("/Pages without /Kids".into());
                };
                for k in kids {
                    self.collect(k, resources, media, out, depth + 1)?;
                }
                Ok(())
            }
            _ => {
                out.push((d, resources, media));
                Ok(())
            }
        }
    }
}

/// The bytes of a stream that starts after the `stream` keyword: `/Length` when it is direct and lands on
/// `endstream`, else up to the next `endstream`.
fn stream_data(bytes: &[u8], lx: &mut Lexer<'_>, d: &BTreeMap<Vec<u8>, Obj>) -> Result<Vec<u8>, String> {
    let mut start = lx.pos;
    if bytes.get(start) == Some(&b'\r') {
        start += 1;
    }
    if bytes.get(start) == Some(&b'\n') {
        start += 1;
    }
    let by_length = match d.get(b"Length".as_slice()) {
        Some(Obj::Int(n)) if *n >= 0 => {
            let end = start + usize::try_from(*n).unwrap_or(usize::MAX / 2);
            let rest = bytes.get(end..).unwrap_or(&[]);
            let skip = rest.iter().take_while(|c| is_white(**c)).count();
            rest[skip..]
                .starts_with(b"endstream")
                .then_some((end, end + skip))
        }
        _ => None,
    };
    let (end, kw) = match by_length {
        Some(e) => e,
        None => {
            let at = bytes[start..]
                .windows(9)
                .position(|w| w == b"endstream")
                .ok_or("stream without endstream")?
                + start;
            let mut end = at;
            while end > start && matches!(bytes[end - 1], b'\r' | b'\n') {
                end -= 1;
            }
            (end, at)
        }
    };
    lx.pos = kw + b"endstream".len();
    Ok(bytes[start..end].to_vec())
}

// ---------------------------------------------------------------------------------------------------------
// Content streams

/// One page as read back.
#[derive(Clone, Debug, Default)]
pub struct PageRead {
    /// The media box width in mm.
    pub width_mm: f64,
    /// The media box height in mm.
    pub height_mm: f64,
    /// The text layer: one entry per text object, in content order.
    pub runs: Vec<TextRun>,
    /// The stroked straight segments.
    pub segments: Vec<Segment>,
}

/// The text of one text object (`BT … ET`) and where it starts.
#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    /// The extracted text.
    pub text: String,
    /// The start of the first shown glyph in mm, y down.
    pub origin: (f64, f64),
}

/// A stroked straight segment in mm, y down from the top of the page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub a: (f64, f64),
    pub b: (f64, f64),
    /// The stroke width in mm (the line width scaled by the CTM).
    pub width: f64,
}

type Matrix = [f64; 6];

const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `m` then `n` (row-vector convention of the PDF specification: `p' = p × m × n`).
fn mul(m: Matrix, n: Matrix) -> Matrix {
    [
        m[0] * n[0] + m[1] * n[2],
        m[0] * n[1] + m[1] * n[3],
        m[2] * n[0] + m[3] * n[2],
        m[2] * n[1] + m[3] * n[3],
        m[4] * n[0] + m[5] * n[2] + n[4],
        m[4] * n[1] + m[5] * n[3] + n[5],
    ]
}

fn apply(m: Matrix, x: f64, y: f64) -> (f64, f64) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

/// A font's code length and ToUnicode map.
#[derive(Clone, Debug, Default)]
struct Cmap {
    code_len: usize,
    map: BTreeMap<u32, String>,
}

#[derive(Default)]
struct FontCache {
    by_object: BTreeMap<u32, Cmap>,
}

#[derive(Clone)]
struct State {
    ctm: Matrix,
    width: f64,
}

struct Interp<'a> {
    file: &'a PdfFile,
    fonts: &'a mut FontCache,
    media: [f64; 4],
    out: PageRead,
}

/// The mutable text and path state of one content stream.
struct Run {
    stack: Vec<State>,
    gs: State,
    path: Vec<((f64, f64), (f64, f64))>,
    start: (f64, f64),
    cur: (f64, f64),
    tm: Matrix,
    tlm: Matrix,
    leading: f64,
    font: Option<Cmap>,
    text: Option<(String, Option<(f64, f64)>)>,
    /// Marked-content stack: `Some(actual text)` for a span that replaces its shown text.
    marked: Vec<Option<String>>,
}

impl Interp<'_> {
    fn to_mm(&self, p: (f64, f64)) -> (f64, f64) {
        (
            (p.0 - self.media[0]) * MM_PER_PT,
            (self.media[3] - p.1) * MM_PER_PT,
        )
    }

    fn run(
        &mut self,
        data: &[u8],
        resources: Option<&BTreeMap<Vec<u8>, Obj>>,
        ctm: Matrix,
        depth: usize,
    ) -> Result<(), String> {
        if depth > 8 {
            return Err("form XObjects nested too deep".into());
        }
        let mut st = Run {
            stack: Vec::new(),
            gs: State { ctm, width: 1.0 },
            path: Vec::new(),
            start: (0.0, 0.0),
            cur: (0.0, 0.0),
            tm: IDENTITY,
            tlm: IDENTITY,
            leading: 0.0,
            font: None,
            text: None,
            marked: Vec::new(),
        };
        let mut p = Parser::new(data, true);
        let mut args: Vec<Obj> = Vec::new();
        while let Some(o) = p.obj()? {
            let Obj::Op(op) = o else {
                args.push(o);
                continue;
            };
            let n = |i: usize| -> Result<f64, String> {
                args.get(i)
                    .and_then(Obj::num)
                    .ok_or_else(|| format!("operator `{}`: number {i} missing", String::from_utf8_lossy(&op)))
            };
            match op.as_slice() {
                b"q" => st.stack.push(st.gs.clone()),
                b"Q" => st.gs = st.stack.pop().ok_or("unbalanced Q")?,
                b"cm" => {
                    let m = [n(0)?, n(1)?, n(2)?, n(3)?, n(4)?, n(5)?];
                    st.gs.ctm = mul(m, st.gs.ctm);
                }
                b"w" => st.gs.width = n(0)?,
                b"m" => {
                    st.cur = apply(st.gs.ctm, n(0)?, n(1)?);
                    st.start = st.cur;
                }
                b"l" => {
                    let q = apply(st.gs.ctm, n(0)?, n(1)?);
                    st.path.push((st.cur, q));
                    st.cur = q;
                }
                b"c" => st.cur = apply(st.gs.ctm, n(4)?, n(5)?),
                b"v" | b"y" => st.cur = apply(st.gs.ctm, n(2)?, n(3)?),
                b"h" => {
                    if st.cur != st.start {
                        st.path.push((st.cur, st.start));
                    }
                    st.cur = st.start;
                }
                b"re" => {
                    let (x, y, w, h) = (n(0)?, n(1)?, n(2)?, n(3)?);
                    let c =
                        [(x, y), (x + w, y), (x + w, y + h), (x, y + h)].map(|(a, b)| apply(st.gs.ctm, a, b));
                    for i in 0..4 {
                        st.path.push((c[i], c[(i + 1) % 4]));
                    }
                    st.start = c[0];
                    st.cur = c[0];
                }
                b"S" | b"s" | b"B" | b"B*" | b"b" | b"b*" => {
                    if matches!(op.as_slice(), b"s" | b"b" | b"b*") && st.cur != st.start {
                        st.path.push((st.cur, st.start));
                    }
                    let m = st.gs.ctm;
                    let scale = (m[0] * m[3] - m[1] * m[2]).abs().sqrt();
                    let width = st.gs.width * scale * MM_PER_PT;
                    for (a, b) in std::mem::take(&mut st.path) {
                        let seg = Segment {
                            a: self.to_mm(a),
                            b: self.to_mm(b),
                            width,
                        };
                        self.out.segments.push(seg);
                    }
                }
                b"f" | b"F" | b"f*" | b"n" => st.path.clear(),
                b"W" | b"W*" => {}
                b"BT" => {
                    st.tm = IDENTITY;
                    st.tlm = IDENTITY;
                    st.text = Some((String::new(), None));
                }
                b"ET" => {
                    let (text, origin) = st.text.take().ok_or("ET without BT")?;
                    self.out.runs.push(TextRun {
                        text,
                        origin: origin.unwrap_or_default(),
                    });
                }
                b"Tf" => {
                    let Some(Obj::Name(name)) = args.first() else {
                        return Err("Tf without a font name".into());
                    };
                    st.font = Some(self.font(resources, name)?);
                }
                b"Tm" => {
                    st.tm = [n(0)?, n(1)?, n(2)?, n(3)?, n(4)?, n(5)?];
                    st.tlm = st.tm;
                }
                b"Td" | b"TD" => {
                    if op == b"TD" {
                        st.leading = -n(1)?;
                    }
                    st.tlm = mul([1.0, 0.0, 0.0, 1.0, n(0)?, n(1)?], st.tlm);
                    st.tm = st.tlm;
                }
                b"TL" => st.leading = n(0)?,
                b"T*" => {
                    st.tlm = mul([1.0, 0.0, 0.0, 1.0, 0.0, -st.leading], st.tlm);
                    st.tm = st.tlm;
                }
                b"Tc" | b"Tw" | b"Tz" | b"Ts" | b"Tr" => {}
                b"Tj" | b"'" | b"\"" => {
                    if op != b"Tj" {
                        st.tlm = mul([1.0, 0.0, 0.0, 1.0, 0.0, -st.leading], st.tlm);
                        st.tm = st.tlm;
                    }
                    if let Some(Obj::Str(s)) = args.last() {
                        self.show(&mut st, s)?;
                    }
                }
                b"TJ" => {
                    if let Some(Obj::Array(a)) = args.first() {
                        for item in a {
                            if let Obj::Str(s) = item {
                                self.show(&mut st, s)?;
                            }
                        }
                    }
                }
                b"BMC" => st.marked.push(None),
                b"BDC" => {
                    let props = match args.get(1) {
                        Some(Obj::Dict(d)) => Some(d.clone()),
                        Some(Obj::Name(name)) => resources
                            .and_then(|r| self.file.key(r, "Properties").dict())
                            .and_then(|p| self.file.key(p, &String::from_utf8_lossy(name)).dict())
                            .cloned(),
                        _ => None,
                    };
                    let actual = props.as_ref().and_then(|d| match self.file.key(d, "ActualText") {
                        Obj::Str(s) => Some(text_string(s)),
                        _ => None,
                    });
                    st.marked.push(actual);
                }
                b"EMC" => {
                    if let Some(Some(actual)) = st.marked.pop() {
                        let origin = self.origin(&st);
                        let (text, o) = st.text.get_or_insert_with(|| (String::new(), None));
                        o.get_or_insert(origin);
                        text.push_str(&actual);
                    }
                }
                b"Do" => {
                    let Some(Obj::Name(name)) = args.first() else {
                        return Err("Do without a name".into());
                    };
                    let xobjects = resources
                        .and_then(|r| self.file.key(r, "XObject").dict())
                        .ok_or("Do without /XObject resources")?;
                    let x = xobjects
                        .get(name)
                        .ok_or_else(|| format!("XObject /{} missing", String::from_utf8_lossy(name)))?;
                    let Obj::Stream(d, _) = self.file.get(x) else {
                        return Err("an XObject is not a stream".into());
                    };
                    if matches!(self.file.key(d, "Subtype"), Obj::Name(s) if s == b"Form") {
                        let m = match self.file.key(d, "Matrix") {
                            Obj::Array(a) if a.len() == 6 => {
                                let v: Vec<f64> = a.iter().filter_map(|x| self.file.get(x).num()).collect();
                                [v[0], v[1], v[2], v[3], v[4], v[5]]
                            }
                            _ => IDENTITY,
                        };
                        let inner = self.file.decode(x)?;
                        let res = self.file.key(d, "Resources").dict().or(resources);
                        self.run(&inner, res, mul(m, st.gs.ctm), depth + 1)?;
                    }
                }
                b"BI" | b"ID" | b"EI" => return Err("inline images are not supported".into()),
                // Colour, state and compatibility operators do not affect text or geometry here.
                _ => {}
            }
            args.clear();
        }
        Ok(())
    }

    fn origin(&self, st: &Run) -> (f64, f64) {
        self.to_mm(apply(mul(st.tm, st.gs.ctm), 0.0, 0.0))
    }

    fn show(&mut self, st: &mut Run, s: &[u8]) -> Result<(), String> {
        let origin = self.origin(st);
        let cmap = st.font.as_ref().ok_or("text shown without a font")?;
        let replaced = st.marked.iter().any(Option::is_some);
        let (text, o) = st.text.as_mut().ok_or("text shown outside BT … ET")?;
        o.get_or_insert(origin);
        if replaced {
            return Ok(());
        }
        let len = cmap.code_len.max(1);
        for code in s.chunks(len) {
            let v = code.iter().fold(0u32, |acc, b| (acc << 8) | u32::from(*b));
            match cmap.map.get(&v) {
                Some(t) => text.push_str(t),
                None => text.push('\u{FFFD}'),
            }
        }
        Ok(())
    }

    fn font(&mut self, resources: Option<&BTreeMap<Vec<u8>, Obj>>, name: &[u8]) -> Result<Cmap, String> {
        let fonts = resources
            .and_then(|r| self.file.key(r, "Font").dict())
            .ok_or("Tf without /Font resources")?;
        let r = fonts
            .get(name)
            .ok_or_else(|| format!("font /{} missing", String::from_utf8_lossy(name)))?;
        let key = match r {
            Obj::Ref(n) => Some(*n),
            _ => None,
        };
        if let Some(c) = key.and_then(|k| self.fonts.by_object.get(&k)) {
            return Ok(c.clone());
        }
        let d = self.file.get(r).dict().ok_or("a font is not a dictionary")?;
        let type0 = matches!(self.file.key(d, "Subtype"), Obj::Name(s) if s == b"Type0");
        let mut cmap = match d.get(b"ToUnicode".as_slice()) {
            Some(t) => parse_cmap(&self.file.decode(t)?)?,
            None => Cmap::default(),
        };
        if cmap.code_len == 0 {
            cmap.code_len = if type0 { 2 } else { 1 };
        }
        if let Some(k) = key {
            self.fonts.by_object.insert(k, cmap.clone());
        }
        Ok(cmap)
    }
}

/// A PDF text string: UTF-16BE with a byte-order mark, else PDFDocEncoding (taken as Latin-1).
fn text_string(s: &[u8]) -> String {
    if s.starts_with(&[0xfe, 0xff]) {
        utf16(&s[2..])
    } else {
        s.iter().map(|&b| char::from(b)).collect()
    }
}

fn utf16(s: &[u8]) -> String {
    let units: Vec<u16> = s
        .chunks(2)
        .map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)]))
        .collect();
    String::from_utf16_lossy(&units)
}

/// A ToUnicode CMap: `codespacerange` (the code length), `bfchar` and `bfrange`.
fn parse_cmap(data: &[u8]) -> Result<Cmap, String> {
    let mut lx = Lexer::new(data);
    let mut toks = Vec::new();
    while let Some(t) = lx.next()? {
        toks.push(t);
    }
    let mut cmap = Cmap::default();
    let code = |b: &[u8]| b.iter().fold(0u32, |acc, x| (acc << 8) | u32::from(*x));
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            Tok::Word(w) if w == b"begincodespacerange" => {
                if let Some(Tok::Str(lo)) = toks.get(i + 1)
                    && cmap.code_len == 0
                {
                    cmap.code_len = lo.len();
                }
                i += 1;
            }
            Tok::Word(w) if w == b"beginbfchar" => {
                i += 1;
                while let (Some(Tok::Str(src)), Some(Tok::Str(dst))) = (toks.get(i), toks.get(i + 1)) {
                    cmap.map.insert(code(src), utf16(dst));
                    i += 2;
                }
            }
            Tok::Word(w) if w == b"beginbfrange" => {
                i += 1;
                while let (Some(Tok::Str(lo)), Some(Tok::Str(hi))) = (toks.get(i), toks.get(i + 1)) {
                    let (lo, hi) = (code(lo), code(hi));
                    if hi < lo || hi - lo > 0xffff {
                        return Err("bad bfrange".into());
                    }
                    match toks.get(i + 2) {
                        Some(Tok::Str(dst)) => {
                            let mut units: Vec<u16> = dst
                                .chunks(2)
                                .map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)]))
                                .collect();
                            for c in lo..=hi {
                                cmap.map.insert(c, String::from_utf16_lossy(&units));
                                if let Some(last) = units.last_mut() {
                                    *last = last.wrapping_add(1);
                                }
                            }
                            i += 3;
                        }
                        Some(Tok::ArrOpen) => {
                            let mut j = i + 3;
                            let mut c = lo;
                            while let Some(Tok::Str(dst)) = toks.get(j) {
                                cmap.map.insert(c, utf16(dst));
                                c += 1;
                                j += 1;
                            }
                            if toks.get(j) != Some(&Tok::ArrClose) {
                                return Err("bad bfrange array".into());
                            }
                            i = j + 1;
                        }
                        _ => return Err("bad bfrange destination".into()),
                    }
                }
            }
            _ => i += 1,
        }
    }
    Ok(cmap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_strings_and_names_lex() {
        let mut lx = Lexer::new(b"-.5 4. +3 (a\\(b\\)c\\101) <00 4> /A#20B [ ] << >>");
        let mut toks = Vec::new();
        while let Some(t) = lx.next().unwrap() {
            toks.push(t);
        }
        assert_eq!(
            toks,
            vec![
                Tok::Real(-0.5),
                Tok::Real(4.0),
                Tok::Int(3),
                Tok::Str(b"a(b)cA".to_vec()),
                Tok::Str(vec![0, 0x40]),
                Tok::Name(b"A B".to_vec()),
                Tok::ArrOpen,
                Tok::ArrClose,
                Tok::DictOpen,
                Tok::DictClose,
            ]
        );
    }

    #[test]
    fn a_cmap_maps_chars_ranges_and_arrays() {
        let c = parse_cmap(
            b"1 begincodespacerange <0000> <FFFF> endcodespacerange 2 beginbfchar <0001> <0053> <0002> <0020> endbfchar 1 beginbfrange <0003> <0005> <0041> endbfrange 1 beginbfrange <0006> <0007> [<221E> <D835DC00>] endbfrange",
        )
        .unwrap();
        assert_eq!(c.code_len, 2);
        let t: String = (1..=7).map(|k| c.map[&k].clone()).collect();
        assert_eq!(t, "S ABC\u{221e}\u{1d400}");
    }
}
