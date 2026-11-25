// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! How a value of the domain is printed as a string that no language changes (doc 04 §3.4): one function per value.
//! Where two places print a value differently, both forms stand here side by side under names that say the
//! difference, and each caller keeps the form it had. The word for a kind of the domain is a [`Text`] by key of this
//! crate's text files (`texts/<language>/words.ftl`, architecture §18), which the checks, the lists and the session
//! print; [`in_languages`] renders one in every language of the files as the multi-language text the lists print.

use std::sync::OnceLock;

use ew_l10n::render::{Language, OnError};
use ew_l10n::{Text, TextArg};
use ew_mltext::MlText;
use ew_schema::{AddrValue, ColourCode, ColourSpec, Csa, FluidPortRole, Kind, LineKind, PotentialType};
use ew_units::{Dec3, Nm};

use crate::bus::net_address_text;
use crate::plc::{ChannelAddr, Slot};

/// The printed text of a cross-section on a wire marker (`2.5`, `AWG18`, `AWG2/0`, `250 kcmil`); `None` for a value of a
/// newer format.
pub fn csa_text(c: &Csa) -> Option<String> {
    match c {
        Csa::Mm2(d) => Some(d.to_string()),
        Csa::Awg(n) => Some(format!("AWG{}", awg_gauge(*n))),
        Csa::Kcmil(d) => Some(format!("{d} kcmil")),
        Csa::Unknown(_) => None,
    }
}

/// A cross-section with its unit as the lists print it (`2.5 mm²`, `AWG18`, `AWG2/0`, `250 kcmil`): Properties' and the
/// checks' messages' form, where [`csa_text`] is a wire marker's, without the unit of square millimetres; `None` for a
/// value of a newer format.
pub fn csa_with_unit(c: &Csa) -> Option<String> {
    match c {
        Csa::Mm2(_) => csa_text(c).map(|t| format!("{t} mm²")),
        other => csa_text(other),
    }
}

/// An AWG gauge as the wire trade writes it: `18`, and from 1/0 up the zeros (`1/0` for 0, `2/0` for −1).
pub fn awg_gauge(n: i8) -> String {
    if n <= 0 {
        format!("{}/0", 1 - i16::from(n))
    } else {
        n.to_string()
    }
}

/// The printed colour of a colour specification: its codes as written, concatenated (`BU`, `GNYE`, `DBU`), each as
/// [`colour_code_text`].
pub fn colour_text(c: &ColourSpec) -> String {
    c.codes().iter().map(colour_code_text).collect()
}

/// One colour code as written: the IEC or company code, a foreign code as stored, `?` for a code of a newer format.
pub fn colour_code_text(c: &ColourCode) -> String {
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
/// one (excluded or not derivable), which a list and a grid leave empty and a sentence words as
/// [`channel_address_words`] does.
pub fn channel_address(a: &ChannelAddr) -> Option<&str> {
    match a {
        ChannelAddr::At { text, .. } => Some(text),
        ChannelAddr::Tbd => Some("TBD"),
        ChannelAddr::Excluded | ChannelAddr::Unresolved => None,
    }
}

/// A channel's derived address in a sentence, in the reader's language: its printed form ([`channel_address`]), else
/// "no address" (a channel without an entry, excluded or not derivable).
pub fn channel_address_words(a: Option<&ChannelAddr>) -> TextArg {
    match a.and_then(channel_address) {
        Some(printed) => printed.into(),
        None => text!("channel-no-address").into(),
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

/// A potential type as the checks' messages and the SQL view print it: its code, `signal`, `undefined` for an undefined
/// type and `other` for a type of a newer format.
pub fn potential_type_text(t: &PotentialType) -> &'static str {
    match t {
        PotentialType::Signal => "signal",
        PotentialType::Unknown(_) => "other",
        other => potential_type_code(other).unwrap_or("undefined"),
    }
}

/// The languages of this crate's text files.
const LANGUAGES: &[&str] = &["en-GB", "nl-NL"];

/// A port's role in words ("working").
pub fn port_role(role: &FluidPortRole) -> Text {
    match role {
        FluidPortRole::Supply1 => text!("port-role-supply"),
        FluidPortRole::Working2 | FluidPortRole::Working4 => text!("port-role-working"),
        FluidPortRole::Exhaust3 | FluidPortRole::Exhaust5 => text!("port-role-exhaust"),
        FluidPortRole::Pilot10 | FluidPortRole::Pilot12 | FluidPortRole::Pilot14 => text!("port-role-pilot"),
        FluidPortRole::PilotExhaust82 | FluidPortRole::PilotExhaust84 => text!("port-role-pilot-exhaust"),
        FluidPortRole::ExternalPilot => text!("port-role-external-pilot"),
        FluidPortRole::Consumer => text!("port-role-consumer"),
        FluidPortRole::Sensing => text!("port-role-sensing"),
        FluidPortRole::Generic | FluidPortRole::Unknown(_) => text!("port-role-other"),
    }
}

/// A line kind in words ("pilot").
pub fn line_kind(kind: &LineKind) -> Text {
    match kind {
        LineKind::Working => text!("line-kind-working"),
        LineKind::Pilot => text!("line-kind-pilot"),
        LineKind::Exhaust => text!("line-kind-exhaust"),
        LineKind::Drain => text!("line-kind-drain"),
        LineKind::Flexible => text!("line-kind-flexible"),
        LineKind::Unknown(_) => text!("line-kind-other"),
    }
}

/// A tube by its two ends (`-201U3-Q1:2 → -203Q1:A`).
pub fn tube_ends(from: impl Into<TextArg>, to: impl Into<TextArg>) -> Text {
    let (from, to): (TextArg, TextArg) = (from.into(), to.into());
    text!("tube-ends", from, to)
}

/// A junction by the frame cell it stands in ("the junction in C11").
pub fn junction_in(cell: impl Into<TextArg>) -> Text {
    let cell: TextArg = cell.into();
    text!("junction-in", cell)
}

/// A junction as the end of a tube ("tee").
pub fn tube_end_junction() -> Text {
    text!("tube-end-junction")
}

/// The vacuum a vacuum generator makes from the medium `name` ("vacuum (from compressed air 6 bar)").
pub fn medium_vacuum(name: &MlText) -> Text {
    text!("medium-vacuum", TextArg::Ml(name.clone()))
}

/// The air a valve vents ("exhaust").
pub fn medium_exhaust() -> Text {
    text!("medium-exhaust")
}

/// A PLC channel by its number and its module's tag ("channel 1 of -201U3").
pub fn channel_of_module(number: impl Into<TextArg>, module: impl Into<TextArg>) -> Text {
    let (number, module): (TextArg, TextArg) = (number.into(), module.into());
    text!("channel-of-module", number, module)
}

/// A record by its kind with its article only (`a device`… `an enclosure`, a representation `a symbol`, a connection `a
/// wire`), for a sentence, where no model is at hand: the one table of a record kind's words ([`record_words`]). A kind
/// that no record can be yet (a reserved kind) and a kind of a newer format are `an item` ([`record_other`]).
pub fn record(kind: Kind) -> Text {
    let [with_article, _] = record_words(kind);
    with_article
}

/// A record kind's name, the bare noun (`device`, `structure node`, a representation `symbol`): what a cell, a column,
/// a heading or a label shows, with the capital it needs ([`record`]'s word without its article).
pub fn record_name(kind: Kind) -> Text {
    let [_, name] = record_words(kind);
    name
}

/// The two forms of a record kind's words, with its article and its name: the one table ([`record`], [`record_name`]).
fn record_words(kind: Kind) -> [Text; 2] {
    match kind {
        Kind::Device => [text!("record-device"), text!("record-device-name")],
        Kind::Function => [text!("record-function"), text!("record-function-name")],
        Kind::Page => [text!("record-page"), text!("record-page-name")],
        Kind::Junction => [text!("record-junction"), text!("record-junction-name")],
        Kind::Representation => [
            text!("record-representation"),
            text!("record-representation-name"),
        ],
        Kind::Connection => [text!("record-connection"), text!("record-connection-name")],
        Kind::Project => [text!("record-project"), text!("record-project-name")],
        Kind::PlcConfigProject => [
            text!("record-plc-config-project"),
            text!("record-plc-config-project-name"),
        ],
        Kind::LogicalNet => [text!("record-logical-net"), text!("record-logical-net-name")],
        Kind::PhysicalNet => [text!("record-physical-net"), text!("record-physical-net-name")],
        Kind::Medium => [text!("record-medium"), text!("record-medium-name")],
        Kind::ImportLineage => [
            text!("record-import-lineage"),
            text!("record-import-lineage-name"),
        ],
        Kind::StructureNode => [
            text!("record-structure-node"),
            text!("record-structure-node-name"),
        ],
        Kind::Section => [text!("record-section"), text!("record-section-name")],
        Kind::ExternalRef => [text!("record-external-ref"), text!("record-external-ref-name")],
        Kind::Conductor => [text!("record-conductor"), text!("record-conductor-name")],
        Kind::Channel => [text!("record-channel"), text!("record-channel-name")],
        Kind::PotentialDef => [text!("record-potential-def"), text!("record-potential-def-name")],
        Kind::MediumDef => [text!("record-medium-def"), text!("record-medium-def-name")],
        Kind::Bond => [text!("record-bond"), text!("record-bond-name")],
        Kind::DeviceLink => [text!("record-device-link"), text!("record-device-link-name")],
        Kind::PageItem => [text!("record-page-item"), text!("record-page-item-name")],
        Kind::StructureBox => [text!("record-structure-box"), text!("record-structure-box-name")],
        Kind::ReportBlock => [text!("record-report-block"), text!("record-report-block-name")],
        Kind::MacroInstance => [
            text!("record-macro-instance"),
            text!("record-macro-instance-name"),
        ],
        Kind::LayoutSpace => [text!("record-layout-space"), text!("record-layout-space-name")],
        Kind::Enclosure => [text!("record-enclosure"), text!("record-enclosure-name")],
        Kind::MountingSurface => [
            text!("record-mounting-surface"),
            text!("record-mounting-surface-name"),
        ],
        Kind::Carrier => [text!("record-carrier"), text!("record-carrier-name")],
        Kind::Placement => [text!("record-placement"), text!("record-placement-name")],
        Kind::Revision => [text!("record-revision"), text!("record-revision-name")],
        Kind::Waiver => [text!("record-waiver"), text!("record-waiver-name")],
        Kind::ExchangeRecord => [
            text!("record-exchange-record"),
            text!("record-exchange-record-name"),
        ],
        Kind::ForeignId => [text!("record-foreign-id"), text!("record-foreign-id-name")],
        Kind::ImportRun => [text!("record-import-run"), text!("record-import-run-name")],
        Kind::Conflict => [text!("record-conflict"), text!("record-conflict-name")],
        Kind::Part => [text!("record-part"), text!("record-part-name")],
        Kind::Symbol => [text!("record-symbol"), text!("record-symbol-name")],
        Kind::Fndef => [text!("record-fndef"), text!("record-fndef-name")],
        Kind::Form => [text!("record-form"), text!("record-form-name")],
        Kind::Macro => [text!("record-macro"), text!("record-macro-name")],
        Kind::Manufacturer => [text!("record-manufacturer"), text!("record-manufacturer-name")],
        Kind::PlcProfile => [text!("record-plc-profile"), text!("record-plc-profile-name")],
        _ => [text!("record-other"), text!("record-other-name")],
    }
}

/// A record of a kind this build does not word ("an item").
pub fn record_other() -> Text {
    text!("record-other")
}

/// The category of an open item: a value still to be decided, a device without a part, a device not placed, a
/// connection point not connected, a cable without a length.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpenItem {
    /// "TBD value".
    TbdValue,
    /// "Device without part".
    DeviceWithoutPart,
    /// "Unplaced device".
    UnplacedDevice,
    /// "Unconnected connection point".
    UnconnectedCp,
    /// "Missing length".
    MissingLength,
}

/// The category of an open item, as the list of open items prints it ("TBD value").
pub fn open_item(category: OpenItem) -> Text {
    match category {
        OpenItem::TbdValue => text!("open-item-tbd-value"),
        OpenItem::DeviceWithoutPart => text!("open-item-device-without-part"),
        OpenItem::UnplacedDevice => text!("open-item-unplaced-device"),
        OpenItem::UnconnectedCp => text!("open-item-unconnected-cp"),
        OpenItem::MissingLength => text!("open-item-missing-length"),
    }
}

/// `text` in every language of this crate's text files, as the lists print a word of the program in the project's
/// language.
pub fn in_languages(text: &Text) -> MlText {
    static BUNDLES: OnceLock<Vec<Language>> = OnceLock::new();
    let bundles = BUNDLES.get_or_init(|| {
        LANGUAGES
            .iter()
            .map(|tag| {
                let sources = crate::SOURCES.iter().filter(|(l, _)| l == tag).map(|(_, s)| *s);
                Language::new(tag, sources, &[], false)
            })
            .collect()
    });
    MlText::from_langs(LANGUAGES.iter().zip(bundles).map(|(tag, language)| {
        let words = language
            .text(text, OnError::Keep)
            .unwrap_or_else(|| text.to_string());
        (tag.parse().expect("a language tag"), words)
    }))
}
