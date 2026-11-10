// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The colour science of the colour-vision test (doc 16 §15.6), in house: sRGB to linear light and
//! back (IEC 61966-2-1), the dichromacy simulations of Machado, Oliveira and Fernandes 2009 at severity 1.0 in linear
//! RGB, CIELAB against D65 and the CIEDE2000 colour difference (Sharma, Wu and Dalal 2005).

use std::collections::BTreeMap;

/// A dichromacy that the test simulates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Simulation {
    Protanopia,
    Deuteranopia,
    Tritanopia,
}

impl Simulation {
    /// The three simulations, in the order doc 16 §15.6 names them.
    pub const ALL: [Simulation; 3] = [
        Simulation::Protanopia,
        Simulation::Deuteranopia,
        Simulation::Tritanopia,
    ];

    /// The simulation's name in file names and reports.
    pub fn word(self) -> &'static str {
        match self {
            Simulation::Protanopia => "protanopia",
            Simulation::Deuteranopia => "deuteranopia",
            Simulation::Tritanopia => "tritanopia",
        }
    }

    /// Machado et al. 2009's matrix at severity 1.0, applied to linear RGB.
    pub fn matrix(self) -> [[f64; 3]; 3] {
        match self {
            Simulation::Protanopia => [
                [0.152286, 1.052583, -0.204868],
                [0.114503, 0.786281, 0.099216],
                [-0.003882, -0.048116, 1.051998],
            ],
            Simulation::Deuteranopia => [
                [0.367322, 0.860646, -0.227968],
                [0.280085, 0.672501, 0.047413],
                [-0.011820, 0.042940, 0.968881],
            ],
            Simulation::Tritanopia => [
                [1.255528, -0.076749, -0.178779],
                [-0.078411, 0.930809, 0.147602],
                [0.004733, 0.691367, 0.303900],
            ],
        }
    }

    /// The colour `rgb` (sRGB bytes) as the dichromat sees it, in linear RGB within [0, 1].
    pub fn linear(self, rgb: [u8; 3]) -> [f64; 3] {
        let m = self.matrix();
        let lin = rgb.map(to_linear);
        [0, 1, 2].map(|i| (m[i][0] * lin[0] + m[i][1] * lin[1] + m[i][2] * lin[2]).clamp(0.0, 1.0))
    }

    /// Applies the simulation to every pixel of RGBA bytes (alpha unchanged).
    pub fn apply(self, rgba: &mut [u8]) {
        let mut seen: BTreeMap<[u8; 3], [u8; 3]> = BTreeMap::new();
        for p in rgba.as_chunks_mut::<4>().0 {
            let rgb = [p[0], p[1], p[2]];
            let out = *seen
                .entry(rgb)
                .or_insert_with(|| self.linear(rgb).map(from_linear));
            p[..3].copy_from_slice(&out);
        }
    }
}

/// An sRGB byte as linear light (IEC 61966-2-1).
pub fn to_linear(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear light within [0, 1] as an sRGB byte (IEC 61966-2-1).
pub fn from_linear(v: f64) -> u8 {
    let c = if v <= 0.003_130_8 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (c * 255.0).round().clamp(0.0, 255.0) as u8
}

/// CIELAB (L*, a*, b*) of linear RGB, through XYZ with IEC 61966-2-1's matrix and its D65 white (the matrix's row
/// sums, so that white has a* = b* = 0).
pub fn lab(lin: [f64; 3]) -> [f64; 3] {
    const M: [[f64; 3]; 3] = [
        [0.4124, 0.3576, 0.1805],
        [0.2126, 0.7152, 0.0722],
        [0.0193, 0.1192, 0.9505],
    ];
    let xyz = M.map(|row| row[0] * lin[0] + row[1] * lin[1] + row[2] * lin[2]);
    let white = M.map(|row| row[0] + row[1] + row[2]);
    let d: f64 = 6.0 / 29.0;
    let f = |t: f64| {
        if t > d.powi(3) {
            t.cbrt()
        } else {
            t / (3.0 * d * d) + 4.0 / 29.0
        }
    };
    let [fx, fy, fz] = [0, 1, 2].map(|i| f(xyz[i] / white[i]));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// The CIEDE2000 colour difference of two CIELAB colours (kL = kC = kH = 1), as Sharma, Wu and Dalal 2005 give it.
pub fn ciede2000(one: [f64; 3], two: [f64; 3]) -> f64 {
    let [l1, a1, b1] = one;
    let [l2, a2, b2] = two;
    let pow7 = |c: f64| c.powi(7);
    let c_bar = (a1.hypot(b1) + a2.hypot(b2)) / 2.0;
    let g = 0.5 * (1.0 - (pow7(c_bar) / (pow7(c_bar) + pow7(25.0))).sqrt());
    let (a1p, a2p) = ((1.0 + g) * a1, (1.0 + g) * a2);
    let (c1p, c2p) = (a1p.hypot(b1), a2p.hypot(b2));
    let hue = |a: f64, b: f64| {
        if a == 0.0 && b == 0.0 {
            0.0
        } else {
            b.atan2(a).to_degrees().rem_euclid(360.0)
        }
    };
    let (h1p, h2p) = (hue(a1p, b1), hue(a2p, b2));
    let neutral = c1p * c2p == 0.0;
    let dl = l2 - l1;
    let dc = c2p - c1p;
    let dh = if neutral {
        0.0
    } else if h2p - h1p > 180.0 {
        h2p - h1p - 360.0
    } else if h2p - h1p < -180.0 {
        h2p - h1p + 360.0
    } else {
        h2p - h1p
    };
    let dh_big = 2.0 * (c1p * c2p).sqrt() * (dh / 2.0).to_radians().sin();
    let l_bar = (l1 + l2) / 2.0;
    let cp_bar = (c1p + c2p) / 2.0;
    let hp_bar = if neutral {
        h1p + h2p
    } else if (h1p - h2p).abs() <= 180.0 {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 360.0 {
        (h1p + h2p + 360.0) / 2.0
    } else {
        (h1p + h2p - 360.0) / 2.0
    };
    let cos = |deg: f64| deg.to_radians().cos();
    let t = 1.0 - 0.17 * cos(hp_bar - 30.0) + 0.24 * cos(2.0 * hp_bar) + 0.32 * cos(3.0 * hp_bar + 6.0)
        - 0.20 * cos(4.0 * hp_bar - 63.0);
    let d_theta = 30.0 * (-((hp_bar - 275.0) / 25.0).powi(2)).exp();
    let rc = 2.0 * (pow7(cp_bar) / (pow7(cp_bar) + pow7(25.0))).sqrt();
    let sl = 1.0 + 0.015 * (l_bar - 50.0).powi(2) / (20.0 + (l_bar - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * cp_bar;
    let sh = 1.0 + 0.015 * cp_bar * t;
    let rt = -(2.0 * d_theta).to_radians().sin() * rc;
    let (l, c, h) = (dl / sl, dc / sc, dh_big / sh);
    (l * l + c * c + h * h + rt * c * h).sqrt()
}
