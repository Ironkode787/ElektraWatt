// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Records named in words for a command's refusal (doc 16a): a refusal reaches the window's toast
//! (`error-command`), so it names a record by its tag, page folio or label, never by its ID. A record that does
//! not exist has no words: the refusal says so and its JSON Pointer names the parameter. The words are keyed:
//! a tag or a number is a plain argument, every word around it a [`Text`] in the window's language.

use ew_derive::layout::{CarrierLabel, CarrierLabelKind};
use ew_derive::{DeviceTag, Q};
use ew_schema::{
    Connection, Device, Function, FunctionKind, Junction, Kind, Page, PageType, RepType, Representation,
};

use crate::text::{Text, TextArg, text};
use crate::tx::TxBuilder;

/// A device's printed tag (`-102G1`): the derived short tag of the transaction's base model, else, for a device
/// the transaction made, its letter and counter (`-K1`; `-K` before online numbering gives it a counter).
fn tag_of(tx: &TxBuilder<'_>, id: ew_ids::Id<Device>, dev: &Device) -> String {
    let q = Q::new(tx.base_model());
    let short = q
        .get::<DeviceTag>(id)
        .as_ref()
        .as_ref()
        .map(ew_derive::Tag::short)
        .unwrap_or_default();
    if !short.is_empty() {
        return short;
    }
    match dev.tag.counter {
        Some(c) => format!("-{}{c}", dev.tag.letter.as_str()),
        None => format!("-{}", dev.tag.letter.as_str()),
    }
}

/// The tag of a device that exists.
fn existing_tag(tx: &TxBuilder<'_>, id: ew_ids::Id<Device>) -> Option<String> {
    tx.get::<Device>(id).map(|d| tag_of(tx, id, &d))
}

/// A device by its tag, or "a device that is not in the project".
pub(crate) fn device(tx: &TxBuilder<'_>, id: ew_ids::Id<Device>) -> TextArg {
    existing_tag(tx, id).map_or_else(|| text!("refusal-words-device-gone").into(), TextArg::Plain)
}

/// A function by its device's tag, or "a function without a device".
pub(crate) fn function(tx: &TxBuilder<'_>, id: ew_ids::Id<Function>) -> TextArg {
    tx.get::<Function>(id)
        .and_then(|f| f.device)
        .and_then(|d| existing_tag(tx, d))
        .map_or_else(
            || text!("refusal-words-function-no-device").into(),
            TextArg::Plain,
        )
}

/// A connection point by its device's tag and its printed designation (`-201U3-Q1:2`): the designation
/// `function_cps` derives on the transaction's base model, else the FnDef CP's, else the index.
pub(crate) fn cp(tx: &TxBuilder<'_>, f: ew_ids::Id<Function>, idx: u16) -> Text {
    let q = Q::new(tx.base_model());
    let derived = q
        .get::<ew_derive::FunctionCps>(f)
        .iter()
        .find(|c| c.idx == idx)
        .map(|c| c.designation.clone());
    let designation = derived
        .or_else(|| {
            let function = tx.get::<Function>(f)?;
            let def = tx.get::<ew_schema::LibCopy<ew_schema::FnDef>>(function.def)?;
            let cp = def.item.cps.iter().find(|c| c.idx == idx)?;
            Some(cp.designation.as_str().to_owned())
        })
        .unwrap_or_else(|| idx.to_string());
    text!("refusal-words-cp", function(tx, f), designation)
}

/// A channel as "channel 3 of -201U3" (its number and its module's tag), or "a channel that is not in the project".
pub(crate) fn channel(tx: &TxBuilder<'_>, id: ew_ids::Id<ew_schema::Channel>) -> Text {
    tx.get::<ew_schema::Channel>(id).map_or_else(
        || text!("refusal-words-channel-gone"),
        |c| {
            text!(
                "refusal-words-channel",
                c.number.to_string(),
                device(tx, c.module)
            )
        },
    )
}

/// The printed folio of a page (its label, else its number), or `None` when the page does not exist.
pub(crate) fn folio(tx: &TxBuilder<'_>, id: ew_ids::Id<Page>) -> Option<String> {
    tx.get::<Page>(id).map(|p| {
        p.folio
            .label
            .as_ref()
            .map_or_else(|| p.folio.number.to_string(), |l| l.as_str().to_owned())
    })
}

/// A page as "page 3", or "a page that is not in the project".
pub(crate) fn page(tx: &TxBuilder<'_>, id: ew_ids::Id<Page>) -> Text {
    folio(tx, id).map_or_else(
        || text!("refusal-words-page-gone"),
        |f| text!("refusal-words-page", f),
    )
}

/// A drawn symbol as "the symbol of -K1", or "a symbol" when it draws no function of a device.
pub(crate) fn symbol(tx: &TxBuilder<'_>, id: ew_ids::Id<Representation>) -> Text {
    let tag = tx
        .get::<Representation>(id)
        .and_then(|r| r.functions.first().copied())
        .and_then(|f| tx.get::<Function>(f))
        .and_then(|f| f.device)
        .and_then(|d| existing_tag(tx, d));
    tag.map_or_else(
        || text!("refusal-words-symbol-any"),
        |t| text!("refusal-words-symbol", t),
    )
}

/// A symbol copy by its item's name ("symbol Interruption point"), else its key, else "a symbol".
pub(crate) fn symbol_copy(tx: &TxBuilder<'_>, id: ew_ids::Id<ew_schema::SymbolDef>) -> TextArg {
    let words = tx
        .get::<ew_schema::LibCopy<ew_schema::SymbolDef>>(id)
        .and_then(|c| {
            let name = ew_derive::neutral(&c.item.name);
            if name.is_empty() {
                c.item.meta.key.as_ref().map(|k| TextArg::Plain(k.to_string()))
            } else {
                Some(text!("refusal-words-symbol-named", name).into())
            }
        });
    document_words(tx, Kind::Symbol, words)
}

/// Any record by the words of its kind (EW-IMP-010 and EW-IMP-011, the re-import card's rows): a device
/// by its tag, a function or an interruption point, a page, a symbol, a wire or a junction as above, else its kind with
/// its article (`a conductor`… `an enclosure`). Keyed, so the window shows it in its language.
pub(crate) fn record(tx: &TxBuilder<'_>, key: ew_model::AnyKey) -> TextArg {
    let (kind, id) = key;
    match kind {
        Kind::Device => device(tx, id.cast()),
        Kind::Function => interruption_point(tx, id.cast()),
        Kind::Page => page(tx, id.cast()).into(),
        Kind::Representation => symbol(tx, id.cast()).into(),
        Kind::Connection => wire(tx, id.cast()).into(),
        Kind::Junction => junction(tx, id.cast()).into(),
        other => kind_words(other),
    }
}

/// A record by its kind with its article only (`a device`… `an enclosure`), where no model is at hand.
pub(crate) fn kind_words(kind: Kind) -> TextArg {
    match kind {
        Kind::Device => text!("refusal-words-kind-device").into(),
        Kind::Function => text!("refusal-words-kind-function").into(),
        Kind::Page => text!("refusal-words-kind-page").into(),
        Kind::Representation => text!("refusal-words-symbol-any").into(),
        Kind::Connection => text!("refusal-words-wire-any").into(),
        Kind::Junction => text!("refusal-words-kind-junction").into(),
        Kind::Project => text!("refusal-words-kind-project").into(),
        Kind::PlcConfigProject => text!("refusal-words-kind-plc-config-project").into(),
        Kind::LogicalNet => text!("refusal-words-kind-logical-net").into(),
        Kind::PhysicalNet => text!("refusal-words-kind-physical-net").into(),
        Kind::Medium => text!("refusal-words-kind-medium").into(),
        Kind::ImportLineage => text!("refusal-words-kind-import-lineage").into(),
        Kind::StructureNode => text!("refusal-words-kind-structure-node").into(),
        Kind::Section => text!("refusal-words-kind-section").into(),
        Kind::ExternalRef => text!("refusal-words-kind-external-ref").into(),
        Kind::Conductor => text!("refusal-words-kind-conductor").into(),
        Kind::Channel => text!("refusal-words-kind-channel").into(),
        Kind::PotentialDef => text!("refusal-words-kind-potential-def").into(),
        Kind::NetDef => text!("refusal-words-kind-net-def").into(),
        Kind::MediumDef => text!("refusal-words-kind-medium-def").into(),
        Kind::Bond => text!("refusal-words-kind-bond").into(),
        Kind::DeviceLink => text!("refusal-words-kind-device-link").into(),
        Kind::PageItem => text!("refusal-words-kind-page-item").into(),
        Kind::StructureBox => text!("refusal-words-kind-structure-box").into(),
        Kind::ReportBlock => text!("refusal-words-kind-report-block").into(),
        Kind::MacroInstance => text!("refusal-words-kind-macro-instance").into(),
        Kind::PlaceholderGroup => text!("refusal-words-kind-placeholder-group").into(),
        Kind::LayoutSpace => text!("refusal-words-kind-layout-space").into(),
        Kind::Enclosure => text!("refusal-words-kind-enclosure").into(),
        Kind::MountingSurface => text!("refusal-words-kind-mounting-surface").into(),
        Kind::Carrier => text!("refusal-words-kind-carrier").into(),
        Kind::Placement => text!("refusal-words-kind-placement").into(),
        Kind::RouteOverride => text!("refusal-words-kind-route-override").into(),
        Kind::CommittedRoute => text!("refusal-words-kind-committed-route").into(),
        Kind::RoutingPath => text!("refusal-words-kind-routing-path").into(),
        Kind::SurfaceFeature => text!("refusal-words-kind-surface-feature").into(),
        Kind::Revision => text!("refusal-words-kind-revision").into(),
        Kind::Waiver => text!("refusal-words-kind-waiver").into(),
        Kind::Comment => text!("refusal-words-kind-comment").into(),
        Kind::Transmittal => text!("refusal-words-kind-transmittal").into(),
        Kind::ExchangeRecord => text!("refusal-words-kind-exchange-record").into(),
        Kind::ForeignId => text!("refusal-words-kind-foreign-id").into(),
        Kind::ImportRun => text!("refusal-words-kind-import-run").into(),
        Kind::Conflict => text!("refusal-words-kind-conflict").into(),
        Kind::Part => text!("refusal-words-kind-part").into(),
        Kind::Symbol => text!("refusal-words-kind-symbol").into(),
        Kind::Fndef => text!("refusal-words-kind-fndef").into(),
        Kind::Form => text!("refusal-words-kind-form").into(),
        Kind::Macro => text!("refusal-words-kind-macro").into(),
        Kind::Manufacturer => text!("refusal-words-kind-manufacturer").into(),
        Kind::PlcProfile => text!("refusal-words-kind-plc-profile").into(),
        Kind::MachineProfile => text!("refusal-words-kind-machine-profile").into(),
        Kind::LabelProfile => text!("refusal-words-kind-label-profile").into(),
        _ => text!("refusal-words-kind-other").into(),
    }
}

/// A library item by its name (a part's number, else the item's key, `ew.iec.mcb-1pn`), else its kind with its
/// article ("a part", as [`record`] names a library kind).
fn item(tx: &TxBuilder<'_>, kind: Kind, name: Option<String>) -> TextArg {
    name.map_or_else(
        || record(tx, (kind, ew_ids::Id::from_uuid(ew_ids::Uuid::nil()))),
        TextArg::Plain,
    )
}

/// A library copy by its item's name and its version ("2904602 at version 1.2.0"): the project holds no
/// library's name, so the item's names it.
pub(crate) fn copy(tx: &TxBuilder<'_>, kind: Kind, name: Option<String>, version: &str) -> Text {
    text!("refusal-words-copy", item(tx, kind, name), version)
}

/// A library document's kind, item words and version; `None` for a kind this build does not read. A document with
/// a name of its own is named by it with its kind's word ("manufacturer Phoenix Contact (PXC)", "symbol
/// Contactor"); else by a part's number, else the item's key.
fn document_parts(doc: &ew_schema::LibDocument) -> Option<(Kind, Option<TextArg>, &str)> {
    use ew_derive::neutral;
    use ew_schema::LibDocument as D;
    let key = |m: &ew_schema::ItemMeta| m.key.as_ref().map(|k| TextArg::Plain(k.to_string()));
    let named = |name: String, words: fn(String) -> Text, m: &ew_schema::ItemMeta| {
        if name.is_empty() {
            key(m)
        } else {
            Some(words(name).into())
        }
    };
    Some(match doc {
        D::Part(c) => (
            Kind::Part,
            Some(TextArg::Plain(c.item.part_number.to_string())),
            &c.version,
        ),
        D::Symbol(c) => (
            Kind::Symbol,
            named(
                neutral(&c.item.name),
                |n| text!("refusal-words-symbol-named", n),
                &c.item.meta,
            ),
            &c.version,
        ),
        D::Fndef(c) => (Kind::Fndef, key(&c.item.meta), &c.version),
        D::Form(c) => (Kind::Form, key(&c.item.meta), &c.version),
        D::Macro(c) => (
            Kind::Macro,
            named(
                neutral(&c.item.name),
                |n| text!("refusal-words-macro-named", n),
                &c.item.meta,
            ),
            &c.version,
        ),
        D::Manufacturer(c) => {
            let name = neutral(&c.item.name);
            let words = if name.is_empty() {
                key(&c.item.meta)
            } else {
                Some(text!("refusal-words-manufacturer-named", name, c.item.key.to_string()).into())
            };
            (Kind::Manufacturer, words, &c.version)
        }
        D::PlcProfile(c) => (
            Kind::PlcProfile,
            named(
                c.item.label.clone(),
                |n| text!("refusal-words-plc-profile-named", n),
                &c.item.meta,
            ),
            &c.version,
        ),
        D::Unknown(_) => return None,
    })
}

/// A library item by its words from [`document_parts`], else its kind with its article, as [`item`] names it.
fn document_words(tx: &TxBuilder<'_>, kind: Kind, words: Option<TextArg>) -> TextArg {
    words.unwrap_or_else(|| item(tx, kind, None))
}

/// A library document's copy, as [`copy`] names it; a document of a kind this build does not read as "an item".
pub(crate) fn document(tx: &TxBuilder<'_>, doc: &ew_schema::LibDocument) -> Text {
    document_parts(doc).map_or_else(
        || text!("refusal-words-kind-other"),
        |(kind, words, version)| text!("refusal-words-copy", document_words(tx, kind, words), version),
    )
}

/// A library document's item, as [`document`] names it without its version.
pub(crate) fn document_item(tx: &TxBuilder<'_>, doc: &ew_schema::LibDocument) -> TextArg {
    document_parts(doc).map_or_else(
        || text!("refusal-words-kind-other").into(),
        |(kind, words, _)| document_words(tx, kind, words),
    )
}

/// A damaged library copy in words, for the banner and the refusals of a project the open made
/// read-only: by its kind word and its own name or key as `doc` names it (the installed library's document of the
/// same copy, the project's own file being unreadable), else its kind with its article, and by the record of `model`
/// that names it, when one does ("the library copy of form A3 named by page 103").
pub fn damaged_copy(
    model: &ew_model::Model,
    kind: Kind,
    doc: Option<&ew_schema::LibDocument>,
    referrer: Option<ew_model::AnyKey>,
) -> Text {
    let mut ids = ew_ids::SystemSource;
    let tx = TxBuilder::new(model, &mut ids);
    let copy = match doc.and_then(document_parts) {
        Some((kind, Some(TextArg::Plain(name)), _)) => match kind {
            Kind::Part => text!("refusal-words-part-named", name).into(),
            Kind::Fndef => text!("refusal-words-fndef-named", name).into(),
            Kind::Form => text!("refusal-words-form-named", name).into(),
            Kind::Symbol => text!("refusal-words-symbol-named", name).into(),
            Kind::Macro => text!("refusal-words-macro-named", name).into(),
            Kind::Manufacturer => text!("refusal-words-manufacturer-keyed", name).into(),
            Kind::PlcProfile => text!("refusal-words-plc-profile-named", name).into(),
            _ => TextArg::Plain(name),
        },
        Some((_, Some(words), _)) => words,
        _ => item(&tx, kind, None),
    };
    match referrer {
        Some(key) => text!("refusal-words-damaged-copy", copy, record(&tx, key)),
        None => text!("refusal-words-library-copy", copy),
    }
}

/// Several texts as one, joined by commas (`a, b, c`), the way `comma_list` joins them; `None` for none: every
/// damaged copy a Save As leaves out.
pub fn comma_joined(items: Vec<Text>) -> Option<Text> {
    let mut rest = items.into_iter().rev();
    let last = rest.next()?;
    Some(rest.fold(last, |tail, item| text!("refusal-words-list-comma", item, tail)))
}

/// A carrier's label inside a sentence ("rail 1", "duct 3"; [`CarrierLabel::in_sentence`], keyed).
pub(crate) fn carrier_in_sentence(label: CarrierLabel) -> Text {
    match label.kind {
        CarrierLabelKind::Rail => text!("refusal-words-rail", label.n.to_string()),
        CarrierLabelKind::Duct => text!("refusal-words-duct", label.n.to_string()),
    }
}

/// A carrier's label as it stands alone ("Rail 1", "Duct 3"; [`CarrierLabel::label`], keyed).
pub(crate) fn carrier_label(label: CarrierLabel) -> Text {
    match label.kind {
        CarrierLabelKind::Rail => text!("refusal-words-rail-label", label.n.to_string()),
        CarrierLabelKind::Duct => text!("refusal-words-duct-label", label.n.to_string()),
    }
}

/// A length in millimetres to one decimal, a whole number without it ("45.5", "120"; 1 nm is "0").
fn mm(n: ew_units::Nm) -> String {
    let s = format!("{:.1}", n.to_mm_f64());
    match s.strip_suffix(".0") {
        Some("-0") => "0".to_owned(),
        Some(whole) => whole.to_owned(),
        None => s,
    }
}

/// A point of a page by its position and page ("x 120, y 45.5 mm on page 3").
pub(crate) fn point(tx: &TxBuilder<'_>, page_id: ew_ids::Id<Page>, p: ew_units::Pt2) -> Text {
    text!("refusal-words-point", mm(p.x), mm(p.y), page(tx, page_id))
}

/// A junction by its page and position ("the junction at x 120, y 45 mm on page 3"), or "a junction that is
/// not in the project".
pub(crate) fn junction(tx: &TxBuilder<'_>, id: ew_ids::Id<Junction>) -> Text {
    tx.get::<Junction>(id).map_or_else(
        || text!("refusal-words-junction-gone"),
        |j| text!("refusal-words-junction", point(tx, j.page, j.at)),
    )
}

/// An interruption point by its name ("interruption point 0V"); a function that is no interruption point by
/// its device's tag, as [`function`] names it.
pub(crate) fn interruption_point(tx: &TxBuilder<'_>, id: ew_ids::Id<Function>) -> TextArg {
    match tx.get::<Function>(id).map(|f| f.kind.clone()) {
        Some(FunctionKind::Ip(ip)) => text!("refusal-words-ip", ip.name.as_str()).into(),
        _ => function(tx, id),
    }
}

/// A position of a page without the page ("x 120, y 45.5 mm").
pub(crate) fn position(p: ew_units::Pt2) -> Text {
    text!("refusal-words-position", mm(p.x), mm(p.y))
}

/// A wire by the number the drawing prints for its physical wire ("wire 102.5", the derived `wire_numbers` label of
/// the base snapshot; a star's shared feed arm takes the first of its wires), else by its page ("a wire on page
/// 102"), else "a wire". A connection the transaction made has no printed number yet.
pub(crate) fn wire(tx: &TxBuilder<'_>, id: ew_ids::Id<Connection>) -> Text {
    let printed = || -> Option<String> {
        let c = tx
            .base_row(&(Kind::Connection, id.cast()))
            .and_then(|r| r.get::<Connection>().cloned())?;
        let q = Q::new(tx.base_model());
        let partition = q.get::<ew_derive::PotentialPartition>(());
        let potential = [&c.a, &c.b]
            .into_iter()
            .filter_map(ew_model::EndpointKey::of)
            .find_map(|k| partition.potential_at(k).map(|p| p.id))?;
        let wires = q.get::<ew_derive::PhysicalWiresOf>(potential);
        let w = wires.wires.iter().find(|w| w.constituents.contains(&id))?;
        let numbers = q.get::<ew_derive::WireNumbers>(());
        numbers.number(w.id).map(|n| n.label.clone())
    };
    if let Some(n) = printed() {
        return text!("refusal-words-wire", n);
    }
    match tx.get::<Connection>(id).and_then(|c| c.page) {
        Some(p) => text!("refusal-words-wire-on", page(tx, p)),
        None => text!("refusal-words-wire-any"),
    }
}

/// Items in a list joined by "; ", each in the reader's language; empty without items.
pub(crate) fn list(items: Vec<TextArg>) -> TextArg {
    joined(items, |a, b| text!("refusal-words-list", a, b))
}

/// Items in a list joined by ", ", each in the reader's language; empty without items.
pub(crate) fn comma_list(items: Vec<TextArg>) -> TextArg {
    joined(items, |a, b| text!("refusal-words-list-comma", a, b))
}

/// `items` folded from the right by `pair`: the first item, then the text of the rest.
fn joined(items: Vec<TextArg>, pair: impl Fn(TextArg, TextArg) -> Text) -> TextArg {
    let mut rest = items.into_iter().rev();
    let Some(last) = rest.next() else {
        return TextArg::Plain(String::new());
    };
    rest.fold(last, |tail, item| pair(item, tail).into())
}

/// A representation type in words (the window's own wording of the type choice): a type of a newer format
/// by its stored value.
pub(crate) fn rep_type(t: &RepType) -> TextArg {
    match t {
        RepType::MultiLine => text!("refusal-words-rep-type-multi-line").into(),
        RepType::SingleLine => text!("refusal-words-rep-type-single-line").into(),
        RepType::Overview => text!("refusal-words-rep-type-overview").into(),
        RepType::Fluid => text!("refusal-words-rep-type-fluid").into(),
        RepType::Topology => text!("refusal-words-rep-type-topology").into(),
        RepType::ContactImage => text!("refusal-words-rep-type-contact-image").into(),
        RepType::PanelFront => text!("refusal-words-rep-type-panel-front").into(),
        RepType::StripSegment => text!("refusal-words-rep-type-strip-segment").into(),
        RepType::Graphic => text!("refusal-words-rep-type-graphic").into(),
        RepType::Unknown(v) => TextArg::Plain(v.to_string()),
    }
}

/// A page of a kind, with its article ("a panel layout page", the window's own wording of the page type
/// choice): a kind of a newer format by its stored value.
pub(crate) fn page_kind(kind: &PageType) -> TextArg {
    match kind {
        PageType::TitlePage => text!("refusal-words-page-kind-title-page").into(),
        PageType::TableOfContents => text!("refusal-words-page-kind-table-of-contents").into(),
        PageType::SectionDivider => text!("refusal-words-page-kind-section-divider").into(),
        PageType::Graphic => text!("refusal-words-page-kind-graphic").into(),
        PageType::SingleLine => text!("refusal-words-page-kind-single-line").into(),
        PageType::MultiLine => text!("refusal-words-page-kind-multi-line").into(),
        PageType::Overview => text!("refusal-words-page-kind-overview").into(),
        PageType::BusTopology => text!("refusal-words-page-kind-bus-topology").into(),
        PageType::Fluid => text!("refusal-words-page-kind-fluid").into(),
        PageType::AssemblyOverview => text!("refusal-words-page-kind-assembly-overview").into(),
        PageType::PanelLayout => text!("refusal-words-page-kind-panel-layout").into(),
        PageType::ModelView => text!("refusal-words-page-kind-model-view").into(),
        PageType::ExternalDocument => text!("refusal-words-page-kind-external-document").into(),
        PageType::Report => text!("refusal-words-page-kind-report").into(),
        PageType::Unknown(v) => TextArg::Plain(v.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A record of a kind without words of its own is named by its keyed kind word, never by
    /// the kind's internal name.
    #[test]
    fn a_record_of_another_kind_is_named_by_its_keyed_kind_word() {
        let project = serde_json::from_str(
            r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
        )
        .expect("project");
        let model = ew_model::Model::new(project);
        let mut ids = ew_ids::SystemSource;
        let tx = TxBuilder::new(&model, &mut ids);
        let id = ew_ids::Id::from_uuid(ew_ids::Uuid::from_u128(1));
        for (kind, words) in [
            (Kind::Conductor, "a conductor"),
            (Kind::PotentialDef, "a potential definition"),
            (Kind::Enclosure, "an enclosure"),
        ] {
            let TextArg::Text(t) = record(&tx, (kind, id)) else {
                panic!("{kind} is keyed");
            };
            assert_eq!(t.to_string(), words);
            assert_eq!(
                t.key(),
                format!("refusal-words-kind-{}", kind.name().replace('_', "-"))
            );
        }
    }
}
