// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! JPEG test fixtures built in the test from given quantised DCT coefficients (FT-406): no encoder, no floating point,
//! so the bytes are the same on every platform. The coefficients of each 8 × 8 block (DC and a few AC terms, in zigzag
//! order) come from a fixed formula over the component and the block position, so the IDCT, the upsampling and the
//! colour conversion of the decoder all do real work. One Huffman table pair serves every component: DC categories
//! 0–11 with 4-bit codes, the 162 AC symbols (EOB, ZRL, run 0–15 × size 1–10) with 8-bit codes; one quantisation
//! table of 2s.
//!
//! Also included by `ew-export`'s PDF tests (`#[path]`), so it is a plain module without crate-local imports.

#![allow(dead_code)]

/// The layout of a fixture.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layout {
    /// One component (greyscale).
    Grey,
    /// YCbCr, no subsampling.
    Ycbcr444,
    /// YCbCr, chroma halved both ways (Y sampling 2 × 2).
    Ycbcr420,
    /// Four components with an Adobe APP14 marker, transform 0: CMYK.
    Cmyk,
}

/// A baseline JPEG of `width × height` pixels (multiples of 16) in `layout`.
pub fn baseline(layout: Layout, width: u16, height: u16) -> Vec<u8> {
    build(layout, width, height, false)
}

/// The same coefficients as [`baseline`], written as a progressive JPEG (SOF2): one interleaved DC scan, then one AC
/// scan (1–63, no successive approximation) per component. It decodes to the same pixels as the baseline file.
pub fn progressive(layout: Layout, width: u16, height: u16) -> Vec<u8> {
    build(layout, width, height, true)
}

/// (horizontal, vertical) sampling factors per component.
fn sampling(layout: Layout) -> Vec<(u8, u8)> {
    match layout {
        Layout::Grey => vec![(1, 1)],
        Layout::Ycbcr444 => vec![(1, 1); 3],
        Layout::Ycbcr420 => vec![(2, 2), (1, 1), (1, 1)],
        Layout::Cmyk => vec![(1, 1); 4],
    }
}

/// The 64 quantised coefficients (zigzag order) of block (`bx`, `by`) of component `c`.
fn coefficients(c: usize, bx: usize, by: usize) -> [i32; 64] {
    let mut z = [0i32; 64];
    let seed = (c * 31 + bx * 17 + by * 7) as i32;
    z[0] = (seed * 13) % 160 - 80;
    for (k, slot) in z.iter_mut().enumerate().take(12).skip(1) {
        // Leave some zeros between the terms so that runs are coded too.
        if k % 3 != 0 {
            *slot = (seed + 5 * k as i32) % 23 - 11;
        }
    }
    z[20] = (seed % 7) - 3;
    z
}

struct Bits {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl Bits {
    fn new() -> Bits {
        Bits {
            out: Vec::new(),
            acc: 0,
            n: 0,
        }
    }

    fn put(&mut self, code: u32, len: u32) {
        for i in (0..len).rev() {
            self.acc = (self.acc << 1) | ((code >> i) & 1);
            self.n += 1;
            if self.n == 8 {
                let byte = self.acc as u8;
                self.out.push(byte);
                if byte == 0xff {
                    self.out.push(0);
                }
                self.acc = 0;
                self.n = 0;
            }
        }
    }

    fn finish(mut self) -> Vec<u8> {
        while self.n != 0 {
            self.put(1, 1);
        }
        self.out
    }
}

/// The magnitude category of a value and its extra bits (JPEG's one's-complement form for negatives).
fn category(v: i32) -> (u32, u32) {
    let size = 32 - v.unsigned_abs().leading_zeros();
    let bits = if v < 0 {
        (v + (1 << size) - 1) as u32
    } else {
        v as u32
    };
    (size, bits)
}

/// The AC symbols in table order: EOB, ZRL, then run × size.
fn ac_symbols() -> Vec<u8> {
    let mut s = vec![0x00, 0xf0];
    for run in 0..16u8 {
        for size in 1..=10u8 {
            s.push((run << 4) | size);
        }
    }
    s
}

fn dc(bits: &mut Bits, diff: i32) {
    let (size, extra) = category(diff);
    bits.put(size, 4);
    if size > 0 {
        bits.put(extra, size);
    }
}

fn ac(bits: &mut Bits, z: &[i32; 64], symbols: &[u8]) {
    let code = |sym: u8| symbols.iter().position(|&s| s == sym).unwrap() as u32;
    let mut run = 0;
    for &v in &z[1..] {
        if v == 0 {
            run += 1;
            continue;
        }
        while run > 15 {
            bits.put(code(0xf0), 8);
            run -= 16;
        }
        let (size, extra) = category(v);
        bits.put(code(((run as u8) << 4) | size as u8), 8);
        bits.put(extra, size);
        run = 0;
    }
    if run > 0 {
        bits.put(code(0x00), 8);
    }
}

fn segment(out: &mut Vec<u8>, marker: u8, payload: &[u8]) {
    out.extend_from_slice(&[0xff, marker]);
    out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(payload);
}

fn build(layout: Layout, width: u16, height: u16, progressive: bool) -> Vec<u8> {
    assert!(width.is_multiple_of(16) && height.is_multiple_of(16));
    let comps = sampling(layout);
    let (hmax, vmax) = (
        comps.iter().map(|c| c.0).max().unwrap() as usize,
        comps.iter().map(|c| c.1).max().unwrap() as usize,
    );
    let (mcux, mcuy) = (usize::from(width) / (8 * hmax), usize::from(height) / (8 * vmax));
    let symbols = ac_symbols();
    let mut out = vec![0xff, 0xd8];
    if layout == Layout::Cmyk {
        // Adobe APP14: version 100, flags 0 and 0, transform 0 (CMYK).
        segment(&mut out, 0xee, b"Adobe\x00\x64\x00\x00\x00\x00\x00");
    }
    let mut dqt = vec![0u8];
    dqt.extend([2u8; 64]);
    segment(&mut out, 0xdb, &dqt);
    let mut sof = vec![8];
    sof.extend_from_slice(&height.to_be_bytes());
    sof.extend_from_slice(&width.to_be_bytes());
    sof.push(comps.len() as u8);
    for (i, (h, v)) in comps.iter().enumerate() {
        sof.extend_from_slice(&[i as u8 + 1, (h << 4) | v, 0]);
    }
    segment(&mut out, if progressive { 0xc2 } else { 0xc0 }, &sof);
    let mut dht = vec![0x00];
    let mut counts = [0u8; 16];
    counts[3] = 12;
    dht.extend(counts);
    dht.extend(0..12u8);
    dht.push(0x10);
    let mut counts = [0u8; 16];
    counts[7] = symbols.len() as u8;
    dht.extend(counts);
    dht.extend(&symbols);
    segment(&mut out, 0xc4, &dht);
    // Interleaved scan over the MCUs: every component's blocks of the MCU in raster order.
    let interleaved = |bits: &mut Bits, with_ac: bool| {
        let mut pred = vec![0i32; comps.len()];
        for my in 0..mcuy {
            for mx in 0..mcux {
                for (c, &(h, v)) in comps.iter().enumerate() {
                    for by in 0..usize::from(v) {
                        for bx in 0..usize::from(h) {
                            let z = coefficients(c, mx * usize::from(h) + bx, my * usize::from(v) + by);
                            dc(bits, z[0] - pred[c]);
                            pred[c] = z[0];
                            if with_ac {
                                ac(bits, &z, &symbols);
                            }
                        }
                    }
                }
            }
        }
    };
    let sos = |out: &mut Vec<u8>, ids: &[u8], ss: u8, se: u8| {
        let mut p = vec![ids.len() as u8];
        for &id in ids {
            p.extend_from_slice(&[id, 0x00]);
        }
        p.extend_from_slice(&[ss, se, 0]);
        segment(out, 0xda, &p);
    };
    let ids: Vec<u8> = (1..=comps.len() as u8).collect();
    if progressive {
        sos(&mut out, &ids, 0, 0);
        let mut bits = Bits::new();
        interleaved(&mut bits, false);
        out.extend(bits.finish());
        // One AC scan per component, its blocks in the component's own raster order.
        for (c, &(h, v)) in comps.iter().enumerate() {
            sos(&mut out, &[c as u8 + 1], 1, 63);
            let mut bits = Bits::new();
            for by in 0..mcuy * usize::from(v) {
                for bx in 0..mcux * usize::from(h) {
                    ac(&mut bits, &coefficients(c, bx, by), &symbols);
                }
            }
            out.extend(bits.finish());
        }
    } else {
        sos(&mut out, &ids, 0, 63);
        let mut bits = Bits::new();
        interleaved(&mut bits, true);
        out.extend(bits.finish());
    }
    out.extend_from_slice(&[0xff, 0xd9]);
    out
}
