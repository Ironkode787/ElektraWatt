// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! How a value of the domain is printed as a string that no language changes (doc 04 §3.4): one function per value.
//! Where two places print a value differently, both forms stand here side by side under names that say the
//! difference, and each caller keeps the form it had. The word for a kind of the domain is a [`Text`] by key of this
//! crate's text files (`texts/<language>/words.ftl`, architecture §18), which the checks, the lists and the session
//! print; [`in_languages`] renders one in every language of the files as the multi-language text the lists print.

use ew_l10n::{Text, TextArg};
use ew_mltext::MlText;
use ew_schema::{
    AddrValue, ColourCode, ColourSpec, Csa, FluidPortRole, Kind, LineKind, PotentialType, TagSpec,
};
use ew_units::{Dec3, Nm};

use crate::bus::net_address_text;
use crate::plc::{ChannelAddr, Slot};

/// A device's tag by its own letter and counter (`-K1`; `-K` before online numbering gives it a counter): how a
/// notice names a device the derivation has not tagged.
pub fn letter_counter_tag(tag: &TagSpec) -> String {
    match tag.counter {
        Some(c) => format!("-{}{c}", tag.letter.as_str()),
        None => format!("-{}", tag.letter.as_str()),
    }
}

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
    let tenths = ew_units::arith::div_round_ties_away(nm, 100_000);
    let sign = if tenths < 0 { "-" } else { "" };
    let tenths = tenths.unsigned_abs();
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

/// A length in millimetres with two decimals, rounded half away from zero (`2.50`): the text-height check.
pub fn mm_hundredth(nm: i64) -> String {
    let hundredths = ew_units::arith::div_round_ties_away(i128::from(nm), 10_000);
    let sign = if hundredths < 0 { "-" } else { "" };
    let hundredths = hundredths.unsigned_abs();
    format!("{sign}{}.{:02}", hundredths / 100, hundredths % 100)
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

/// A potential type in words, as the lists print it and the checks name it: its code (`L1`, `PE`, the same in every
/// language), `signal`, `undefined` for an undefined type and `other` for a type of a newer format. The SQL view prints
/// its English.
pub fn potential_type(t: &PotentialType) -> Text {
    if let Some(code) = potential_type_code(t) {
        return text!("potential-type-code", code);
    }
    match t {
        PotentialType::Signal => text!("potential-type-signal"),
        PotentialType::Undefined => text!("potential-type-undefined"),
        _ => text!("potential-type-other"),
    }
}

/// A circuit class in words, as the lists print it and the checks name it (`mains`, `PE`); `other` for a class of a newer
/// format. The SQL view prints its English.
pub fn circuit_class(c: &ew_schema::CircuitClass) -> Text {
    use ew_schema::CircuitClass as C;
    match c {
        C::Mains => text!("circuit-class-mains"),
        C::Control => text!("circuit-class-control"),
        C::Analogue => text!("circuit-class-analogue"),
        C::Bus => text!("circuit-class-bus"),
        C::Pe => text!("circuit-class-pe"),
        C::Unknown(_) => text!("circuit-class-other"),
    }
}

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

/// A channel kind in words (`DO`, `AI`, `counter`, `multi-function`): a signal code is the same in every language.
pub fn channel_kind(k: &ew_schema::ChannelKind) -> Text {
    use ew_schema::ChannelKind as K;
    match k {
        K::Di => text!("channel-kind-di"),
        K::Do => text!("channel-kind-do"),
        K::Ai => text!("channel-kind-ai"),
        K::Ao => text!("channel-kind-ao"),
        K::Counter => text!("channel-kind-counter"),
        K::IoLinkPort => text!("channel-kind-io-link"),
        K::PowerSupply => text!("channel-kind-supply"),
        K::MultiFunction => text!("channel-kind-multi-function"),
        K::Unknown(_) => text!("channel-kind-other"),
    }
}

/// A text style's name as the Text styles page lists it, the symbol editor offers it and EW-CORE-061 names it: a built-in
/// style by its name (`text-style-tag-main`, "Device tag"), a style a QElectroTech import made (`qet.<size>[.b][.i]`) as
/// imported text of its point size, any other style a project adds as a text style of this project. Never the key.
pub fn text_style_name(key: &str) -> Text {
    if let Some(rest) = key.strip_prefix("qet.") {
        let rest = rest.strip_suffix(".i").unwrap_or(rest);
        return match rest.strip_suffix(".b") {
            Some(size) => text!("text-style-imported-bold", size.to_owned()),
            None => text!("text-style-imported", rest.to_owned()),
        };
    }
    match key {
        "tag.main" => text!("text-style-tag-main"),
        "data.small" => text!("text-style-data-small"),
        "cp.small" => text!("text-style-cp-small"),
        "pin" => text!("text-style-pin"),
        "xref.device" => text!("text-style-xref-device"),
        "marker" => text!("text-style-marker"),
        "terminal" => text!("text-style-terminal"),
        "text.normal" => text!("text-style-text-normal"),
        "field.label" => text!("text-style-field-label"),
        "box.title" => text!("text-style-box-title"),
        "cover.table" => text!("text-style-cover-table"),
        "cover.title" => text!("text-style-cover-title"),
        "cover.subtitle" => text!("text-style-cover-subtitle"),
        "section.id" => text!("text-style-section-id"),
        "section.subtitle" => text!("text-style-section-subtitle"),
        _ => text!("text-style-own"),
    }
}

/// A PLC channel by its number and its module's tag ("channel 1 of -201U3").
pub fn channel_of_module(number: impl Into<TextArg>, module: impl Into<TextArg>) -> Text {
    let (number, module): (TextArg, TextArg) = (number.into(), module.into());
    text!("channel-of-module", number, module)
}

/// A record by its kind with its article only (`a device`… `an enclosure`, a representation `a symbol`, a connection `a
/// connection`, which can be a wire, a cable core, a bridge, a bus line or a tube), for a sentence, where no model is at
/// hand: the one table of a record kind's words ([`record_words`]). A kind of a newer format is `an item`
/// ([`record_other`]).
pub fn record(kind: Kind) -> Text {
    let [with_article, _, _] = record_words(Some(kind));
    with_article
}

/// A record kind's name, the bare noun (`device`, `structure node`, a representation `symbol`): what a cell, a column,
/// a heading or a label shows, with the capital it needs ([`record`]'s word without its article).
pub fn record_name(kind: Kind) -> Text {
    let [_, name, _] = record_words(Some(kind));
    name
}

/// A record kind's name in the plural (`devices`, `structure nodes`): what a heading of many shows, with the capital it
/// needs.
pub fn record_names(kind: Kind) -> Text {
    let [_, _, names] = record_words(Some(kind));
    names
}

/// The words of the record kind named `name` (`potential_def`), as [`record_words`] holds them; a name no kind has is
/// an item.
pub fn record_words_named(name: &str) -> [Text; 3] {
    record_words(Kind::from_name(name))
}

/// The three forms of a record kind's words, with its article, its name and its plural: the one table ([`record`],
/// [`record_name`], [`record_names`]).
fn record_words(kind: Option<Kind>) -> [Text; 3] {
    match kind {
        Some(Kind::Project) => [
            text!("record-project"),
            text!("record-project-name"),
            text!("record-project-names"),
        ],
        Some(Kind::PlcConfigProject) => [
            text!("record-plc-config-project"),
            text!("record-plc-config-project-name"),
            text!("record-plc-config-project-names"),
        ],
        Some(Kind::LogicalNet) => [
            text!("record-logical-net"),
            text!("record-logical-net-name"),
            text!("record-logical-net-names"),
        ],
        Some(Kind::PhysicalNet) => [
            text!("record-physical-net"),
            text!("record-physical-net-name"),
            text!("record-physical-net-names"),
        ],
        Some(Kind::Medium) => [
            text!("record-medium"),
            text!("record-medium-name"),
            text!("record-medium-names"),
        ],
        Some(Kind::ImportLineage) => [
            text!("record-import-lineage"),
            text!("record-import-lineage-name"),
            text!("record-import-lineage-names"),
        ],
        Some(Kind::StructureNode) => [
            text!("record-structure-node"),
            text!("record-structure-node-name"),
            text!("record-structure-node-names"),
        ],
        Some(Kind::Section) => [
            text!("record-section"),
            text!("record-section-name"),
            text!("record-section-names"),
        ],
        Some(Kind::ExternalRef) => [
            text!("record-external-ref"),
            text!("record-external-ref-name"),
            text!("record-external-ref-names"),
        ],
        Some(Kind::Page) => [
            text!("record-page"),
            text!("record-page-name"),
            text!("record-page-names"),
        ],
        Some(Kind::Device) => [
            text!("record-device"),
            text!("record-device-name"),
            text!("record-device-names"),
        ],
        Some(Kind::Function) => [
            text!("record-function"),
            text!("record-function-name"),
            text!("record-function-names"),
        ],
        Some(Kind::Representation) => [
            text!("record-representation"),
            text!("record-representation-name"),
            text!("record-representation-names"),
        ],
        Some(Kind::Connection) => [
            text!("record-connection"),
            text!("record-connection-name"),
            text!("record-connection-names"),
        ],
        Some(Kind::Junction) => [
            text!("record-junction"),
            text!("record-junction-name"),
            text!("record-junction-names"),
        ],
        Some(Kind::Conductor) => [
            text!("record-conductor"),
            text!("record-conductor-name"),
            text!("record-conductor-names"),
        ],
        Some(Kind::Channel) => [
            text!("record-channel"),
            text!("record-channel-name"),
            text!("record-channel-names"),
        ],
        Some(Kind::PotentialDef) => [
            text!("record-potential-def"),
            text!("record-potential-def-name"),
            text!("record-potential-def-names"),
        ],
        Some(Kind::NetDef) => [
            text!("record-net-def"),
            text!("record-net-def-name"),
            text!("record-net-def-names"),
        ],
        Some(Kind::MediumDef) => [
            text!("record-medium-def"),
            text!("record-medium-def-name"),
            text!("record-medium-def-names"),
        ],
        Some(Kind::Bond) => [
            text!("record-bond"),
            text!("record-bond-name"),
            text!("record-bond-names"),
        ],
        Some(Kind::DeviceLink) => [
            text!("record-device-link"),
            text!("record-device-link-name"),
            text!("record-device-link-names"),
        ],
        Some(Kind::PageItem) => [
            text!("record-page-item"),
            text!("record-page-item-name"),
            text!("record-page-item-names"),
        ],
        Some(Kind::StructureBox) => [
            text!("record-structure-box"),
            text!("record-structure-box-name"),
            text!("record-structure-box-names"),
        ],
        Some(Kind::ReportBlock) => [
            text!("record-report-block"),
            text!("record-report-block-name"),
            text!("record-report-block-names"),
        ],
        Some(Kind::MacroInstance) => [
            text!("record-macro-instance"),
            text!("record-macro-instance-name"),
            text!("record-macro-instance-names"),
        ],
        Some(Kind::PlaceholderGroup) => [
            text!("record-placeholder-group"),
            text!("record-placeholder-group-name"),
            text!("record-placeholder-group-names"),
        ],
        Some(Kind::LayoutSpace) => [
            text!("record-layout-space"),
            text!("record-layout-space-name"),
            text!("record-layout-space-names"),
        ],
        Some(Kind::Enclosure) => [
            text!("record-enclosure"),
            text!("record-enclosure-name"),
            text!("record-enclosure-names"),
        ],
        Some(Kind::MountingSurface) => [
            text!("record-mounting-surface"),
            text!("record-mounting-surface-name"),
            text!("record-mounting-surface-names"),
        ],
        Some(Kind::Carrier) => [
            text!("record-carrier"),
            text!("record-carrier-name"),
            text!("record-carrier-names"),
        ],
        Some(Kind::Placement) => [
            text!("record-placement"),
            text!("record-placement-name"),
            text!("record-placement-names"),
        ],
        Some(Kind::RouteOverride) => [
            text!("record-route-override"),
            text!("record-route-override-name"),
            text!("record-route-override-names"),
        ],
        Some(Kind::CommittedRoute) => [
            text!("record-committed-route"),
            text!("record-committed-route-name"),
            text!("record-committed-route-names"),
        ],
        Some(Kind::RoutingPath) => [
            text!("record-routing-path"),
            text!("record-routing-path-name"),
            text!("record-routing-path-names"),
        ],
        Some(Kind::SurfaceFeature) => [
            text!("record-surface-feature"),
            text!("record-surface-feature-name"),
            text!("record-surface-feature-names"),
        ],
        Some(Kind::Revision) => [
            text!("record-revision"),
            text!("record-revision-name"),
            text!("record-revision-names"),
        ],
        Some(Kind::Waiver) => [
            text!("record-waiver"),
            text!("record-waiver-name"),
            text!("record-waiver-names"),
        ],
        Some(Kind::Comment) => [
            text!("record-comment"),
            text!("record-comment-name"),
            text!("record-comment-names"),
        ],
        Some(Kind::Transmittal) => [
            text!("record-transmittal"),
            text!("record-transmittal-name"),
            text!("record-transmittal-names"),
        ],
        Some(Kind::ExchangeRecord) => [
            text!("record-exchange-record"),
            text!("record-exchange-record-name"),
            text!("record-exchange-record-names"),
        ],
        Some(Kind::ForeignId) => [
            text!("record-foreign-id"),
            text!("record-foreign-id-name"),
            text!("record-foreign-id-names"),
        ],
        Some(Kind::ImportRun) => [
            text!("record-import-run"),
            text!("record-import-run-name"),
            text!("record-import-run-names"),
        ],
        Some(Kind::Conflict) => [
            text!("record-conflict"),
            text!("record-conflict-name"),
            text!("record-conflict-names"),
        ],
        Some(Kind::Part) => [
            text!("record-part"),
            text!("record-part-name"),
            text!("record-part-names"),
        ],
        Some(Kind::Symbol) => [
            text!("record-symbol"),
            text!("record-symbol-name"),
            text!("record-symbol-names"),
        ],
        Some(Kind::Fndef) => [
            text!("record-fndef"),
            text!("record-fndef-name"),
            text!("record-fndef-names"),
        ],
        Some(Kind::Form) => [
            text!("record-form"),
            text!("record-form-name"),
            text!("record-form-names"),
        ],
        Some(Kind::Macro) => [
            text!("record-macro"),
            text!("record-macro-name"),
            text!("record-macro-names"),
        ],
        Some(Kind::Manufacturer) => [
            text!("record-manufacturer"),
            text!("record-manufacturer-name"),
            text!("record-manufacturer-names"),
        ],
        Some(Kind::PlcProfile) => [
            text!("record-plc-profile"),
            text!("record-plc-profile-name"),
            text!("record-plc-profile-names"),
        ],
        Some(Kind::MachineProfile) => [
            text!("record-machine-profile"),
            text!("record-machine-profile-name"),
            text!("record-machine-profile-names"),
        ],
        Some(Kind::LabelProfile) => [
            text!("record-label-profile"),
            text!("record-label-profile-name"),
            text!("record-label-profile-names"),
        ],
        _ => [
            text!("record-other"),
            text!("record-other-name"),
            text!("record-other-names"),
        ],
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
    crate::catalogue().in_languages_kept(text)
}
