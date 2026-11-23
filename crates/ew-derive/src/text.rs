// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! How a value of the domain is printed as a string that no language changes (doc 04 §3.4): one function per value.
//! Where two places print a value differently, both forms stand here side by side under names that say the
//! difference, and each caller keeps the form it had.

use ew_schema::{AddrValue, ColourCode, ColourSpec, Csa, PotentialType};
use ew_units::{Dec3, Nm};

use crate::bus::net_address_text;
use crate::plc::{ChannelAddr, Slot};

/// The printed text of a cross-section (`2.5`, `AWG18`, `250 kcmil`); `None` for a value of a newer format.
pub fn csa_text(c: &Csa) -> Option<String> {
    match c {
        Csa::Mm2(d) => Some(d.to_string()),
        Csa::Awg(n) => Some(format!("AWG{n}")),
        Csa::Kcmil(d) => Some(format!("{d} kcmil")),
        Csa::Unknown(_) => None,
    }
}

/// The printed colour code of a colour specification: its codes in their file spelling, concatenated (`BU`,
/// `GNYE`, `DBU`); `None` when a code has no text form (a value of a newer format).
pub fn colour_text(c: &ColourSpec) -> Option<String> {
    let mut out = String::new();
    for code in c.codes() {
        let v = serde_json::to_value(code).ok()?;
        out.push_str(v.as_str()?);
    }
    Some(out)
}

/// The codes of a colour joined without a separator (`GNYE`); a code of a newer format prints its raw JSON value. The
/// lists' and the SQL view's form ([`colour_text`] has none for such a code, [`colour_marked`] prints `?`).
pub fn colour_raw(c: &ColourSpec) -> String {
    c.codes()
        .iter()
        .map(|c| match c {
            ColourCode::Iec(i) => i.as_str().to_owned(),
            ColourCode::Company(i) | ColourCode::Foreign(i) => i.as_str().to_owned(),
            ColourCode::Unknown(v) => v.to_string(),
        })
        .collect()
}

/// The codes of a colour joined without a separator (`GNYE`, `BR`), each as [`colour_code_marked`]. The checks'
/// messages' form.
pub fn colour_marked(c: &ColourSpec) -> String {
    c.codes().iter().map(colour_code_marked).collect()
}

/// One colour code as written: the IEC or company code, a foreign code as stored, `?` for a code of a newer format.
pub fn colour_code_marked(c: &ColourCode) -> String {
    match c {
        ColourCode::Iec(c) => c.as_str().to_owned(),
        ColourCode::Company(c) | ColourCode::Foreign(c) => c.as_str().to_owned(),
        ColourCode::Unknown(_) => "?".to_owned(),
    }
}

/// A module's slot as the lists and the SQL view print it: a positional slot as its rank (`3`), else `<rack>.<slot>`
/// and `<rack>.<slot>.<subslot>`; `None` for a slot of a newer format.
pub fn slot_text(s: Slot) -> Option<String> {
    match s {
        Slot::Positional(n) => Some(n.to_string()),
        Slot::Rack {
            rack,
            slot,
            subslot: None,
        } => Some(format!("{rack}.{slot}")),
        Slot::Rack {
            rack,
            slot,
            subslot: Some(sub),
        } => Some(format!("{rack}.{slot}.{sub}")),
        Slot::Unresolved => None,
    }
}

/// A module's slot as the PLC grid prints it and its typed editor reads it back: [`slot_text`], but an authored slot
/// in rack 0 without a subslot as the slot alone (`3`, where the lists print `0.3`).
pub fn slot_short(s: Slot) -> Option<String> {
    match s {
        Slot::Rack {
            rack: 0,
            slot,
            subslot: None,
        } => Some(slot.to_string()),
        other => slot_text(other),
    }
}

/// A channel's derived address as printed: the profile's drawing form (`Q0.1`) or `TBD`; `None` for a channel without
/// one (excluded or not derivable), which each reader words its own way.
pub fn channel_address(a: &ChannelAddr) -> Option<&str> {
    match a {
        ChannelAddr::At { text, .. } => Some(text),
        ChannelAddr::Tbd => Some("TBD"),
        ChannelAddr::Excluded | ChannelAddr::Unresolved => None,
    }
}

/// A bus address as printed: `TBD`, or the address's text ([`net_address_text`]); `None` for a port without one or an
/// address of a newer format.
pub fn bus_address(a: &AddrValue) -> Option<String> {
    match a {
        AddrValue::Tbd => Some("TBD".to_owned()),
        AddrValue::Value(a) => net_address_text(a),
        AddrValue::NotApplicable | AddrValue::Unknown(_) => None,
    }
}

/// A length in nanometres as millimetres, exact, without trailing fraction zeros (`96`, `7.5`, `-0.000001`): the
/// layout's messages and fields.
pub fn mm_exact(nm: i128) -> String {
    let sign = if nm < 0 { "-" } else { "" };
    let a = nm.unsigned_abs();
    let (int, frac) = (a / 1_000_000, a % 1_000_000);
    if frac == 0 {
        format!("{sign}{int}")
    } else {
        let digits = format!("{frac:06}");
        format!("{sign}{int}.{}", digits.trim_end_matches('0'))
    }
}

/// A length in millimetres cut toward zero at the micrometre, whole millimetres plain, else three decimals at most
/// (`6`, `2.7`): the fluid media's sizes.
pub fn mm_micro(nm: Nm) -> String {
    Dec3(nm.0 / 1_000).to_string()
}

/// A length in millimetres with one decimal, rounded half away from zero (`40.0`, `-0.5`): the layout checks' points.
pub fn mm_tenth(nm: i128) -> String {
    const NM_PER_TENTH: u128 = 100_000;
    let tenths = (nm.unsigned_abs() + NM_PER_TENTH / 2) / NM_PER_TENTH;
    let sign = if nm < 0 && tenths > 0 { "-" } else { "" };
    format!("{sign}{}.{}", tenths / 10, tenths % 10)
}

/// [`mm_tenth`] without a trailing `.0` (`10`, `2.5`): the bracket checks' lengths.
pub fn mm_tenth_short(nm: i128) -> String {
    let text = mm_tenth(nm);
    match text.strip_suffix(".0") {
        Some(whole) => whole.to_owned(),
        None => text,
    }
}

/// A length in millimetres to one decimal through its floating-point value (ties as the float rounds them), a whole
/// number without the decimal (`45.5`, `120`; 1 nm is `0`): the commands' refusals that name a point.
pub fn mm_tenth_float(n: Nm) -> String {
    let s = format!("{:.1}", n.to_mm_f64());
    match s.strip_suffix(".0") {
        Some("-0") => "0".to_owned(),
        Some(whole) => whole.to_owned(),
        None => s,
    }
}

/// A length in millimetres with two decimals, rounded half up (`2.50`): the text-height check.
pub fn mm_hundredth(nm: i64) -> String {
    let hundredths = (nm + 5_000) / 10_000;
    format!("{}.{:02}", hundredths / 100, hundredths % 100)
}

/// A length in millimetres with three decimals, more when not exact (`40.000`, `0.0005`): the property fields.
pub fn mm_padded(nm: i64) -> String {
    let sign = if nm < 0 { "-" } else { "" };
    let abs = nm.unsigned_abs();
    let (mm, frac) = (abs / 1_000_000, abs % 1_000_000);
    let mut digits = format!("{frac:06}");
    while digits.len() > 3 && digits.ends_with('0') {
        digits.pop();
    }
    format!("{sign}{mm}.{digits}")
}

/// A length in millimetres cut at the micrometre: whole millimetres without decimals, else up to three decimals
/// without trailing zeros (`40`, `37.5`; a negative length under a micrometre `-0`): the panel page's dimensions.
pub fn mm_truncated(n: Nm) -> String {
    let (whole, frac) = (n.0 / 1_000_000, (n.0 % 1_000_000).abs());
    if frac == 0 {
        return whole.to_string();
    }
    let sign = if n.0 < 0 && whole == 0 { "-" } else { "" };
    let micro = format!("{:06}", frac);
    let dec = micro[..3].trim_end_matches('0');
    if dec.is_empty() {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{dec}")
    }
}

/// The code of an electrical potential type (`L1`, `PE`, `L+`, `M`); `None` for a signal, an undefined type and a type
/// of a newer format.
pub fn potential_type_code(t: &PotentialType) -> Option<&'static str> {
    Some(match t {
        PotentialType::L1 => "L1",
        PotentialType::L2 => "L2",
        PotentialType::L3 => "L3",
        PotentialType::L => "L",
        PotentialType::N => "N",
        PotentialType::Pe => "PE",
        PotentialType::Pen => "PEN",
        PotentialType::Fe => "FE",
        PotentialType::LPlus => "L+",
        PotentialType::LMinus => "L-",
        PotentialType::M => "M",
        PotentialType::Signal | PotentialType::Undefined | PotentialType::Unknown(_) => return None,
    })
}

/// A potential type as the checks' messages print it: its code, `signal`, and `undefined` for an undefined type and for
/// a type of a newer format.
pub fn potential_type_text(t: &PotentialType) -> &'static str {
    match t {
        PotentialType::Signal => "signal",
        other => potential_type_code(other).unwrap_or("undefined"),
    }
}

/// A potential type as the SQL view prints it: [`potential_type_text`], but `other` for a type of a newer format.
pub fn potential_type_sql(t: &PotentialType) -> &'static str {
    match t {
        PotentialType::Unknown(_) => "other",
        other => potential_type_text(other),
    }
}
