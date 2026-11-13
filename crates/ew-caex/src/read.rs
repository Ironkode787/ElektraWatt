// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The AML reader (doc 10 §4.4, doc 14 §11): CAEX 2.15 and 3.0 files with AR APC content into an
//! [`ExchangeImage`].
//!
//! The reader is pure (doc 14 §2.1): it reads the bytes it is handed and nothing else. A hostile file is refused
//! before it costs much: the input size, the element depth, the number of nodes, the attributes per element and per
//! document, the length of an attribute value and of a text are bounded by [`Limits`], and a DOCTYPE is refused, so
//! there are no entities to expand. A file that is well-formed but holds no AR APC project is
//! [`ImportError::Unsupported`]; a number that is no number is [`ImportError::Malformed`] and names the element.
//!
//! What is read: the first `InternalElement` with the role `AutomationProject` and, below it at any depth, `Device`
//! elements (stations) with their `DeviceItem`s, `CommunicationInterface`s with a `Node` and `CommunicationPort`s,
//! `TagTable`s (also below a CPU item or in a tag folder) and `Subnet`s. A role is the last segment of an element's
//! `SupportedRoleClass` or `RoleRequirements` path. An `InternalLink` names its sides as `<element ID>:<interface
//! name>` (CAEX 2.15) or by the interface's own ID (CAEX 3.0); a link between a channel and a tag gives the tag its
//! channel, one between a node and a subnet gives the node its network. Everything else is skipped.

use std::collections::BTreeMap;

use ew_interop_api::{ImportError, Limits};
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::image::{
    ChannelType, ExchangeImage, IecTags, IoType, XAddr, XChannel, XInterface, XItem, XNetwork, XNode, XPort,
    XProject, XStation, XTag,
};

/// The image of the AML file `bytes`.
///
/// # Errors
///
/// [`ImportError::LimitExceeded`] for a file beyond `limits`; [`ImportError::Malformed`] for a file that is not
/// well-formed XML, declares a DOCTYPE or holds a number that is none; [`ImportError::Unsupported`] for a file that is
/// not CAEX 2.15 or 3.0 or holds no AR APC project.
pub fn read_aml(bytes: &[u8], limits: &Limits) -> Result<ExchangeImage, ImportError> {
    if bytes.len() as u64 > limits.max_input_bytes {
        return Err(over("input size", limits.max_input_bytes));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| malformed("the file is not UTF-8"))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let root = parse(text, limits)?;
    if root.name != "CAEXFile" {
        return Err(unsupported(format!(
            "the root element is `{}`, not `CAEXFile`: this is no AutomationML file",
            root.name
        )));
    }
    let version = root.attr("SchemaVersion").unwrap_or("");
    if !(version == "2.15" || version.starts_with("3.")) {
        return Err(unsupported(format!(
            "the file is CAEX `{version}`; CAEX 2.15 and 3.0 are read"
        )));
    }
    let project = root
        .children
        .iter()
        .filter(|c| c.name == "InstanceHierarchy")
        .find_map(|h| find_role(h, "AutomationProject"))
        .ok_or_else(|| {
            unsupported("the file holds no AR APC project (an element with the role `AutomationProject`)")
        })?;

    let mut image = ExchangeImage {
        project: XProject {
            id: project.id().to_owned(),
            name: project.attr("Name").unwrap_or("").to_owned(),
            ..XProject::default()
        },
        ..ExchangeImage::default()
    };
    let mut ends = Ends::default();
    for child in project.elements() {
        project_child(child, &mut image, &mut ends)?;
    }
    let mut links = Vec::new();
    collect_links(&root, &mut links);
    for (a, b) in links {
        let (Some(a), Some(b)) = (ends.resolve(a), ends.resolve(b)) else {
            continue;
        };
        for (x, y) in [(a, b), (b, a)] {
            match (x, y) {
                (End::Channel(c), End::Tag(t)) => {
                    if let Some(tag) = image.tags.get_mut(*t) {
                        tag.channel = Some(c.clone());
                    }
                }
                (End::Node(s, i), End::Subnet(n)) => {
                    let node = image
                        .stations
                        .get_mut(*s)
                        .and_then(|st| st.interfaces.get_mut(*i))
                        .and_then(|f| f.node.as_mut());
                    if let Some(node) = node {
                        node.network = Some(n.clone());
                    }
                }
                _ => {}
            }
        }
    }
    Ok(image)
}

// ---------------------------------------------------------------------------------------------------------
// The elements of the image

/// What an interface is, for the links: the channel's ID, the tag's index, the node's station and interface index,
/// the subnet's ID.
enum End {
    Channel(String),
    Tag(usize),
    Node(usize, usize),
    Subnet(String),
}

/// The interfaces a link can name: by `(element ID, interface name)` and by the interface's own ID.
#[derive(Default)]
struct Ends {
    by_owner: BTreeMap<(String, String), usize>,
    by_id: BTreeMap<String, usize>,
    ends: Vec<End>,
}

impl Ends {
    fn add(&mut self, owner: &str, interface: &El, end: End) {
        let i = self.ends.len();
        self.ends.push(end);
        let name = interface.attr("Name").unwrap_or("");
        self.by_owner.entry((owner.to_owned(), name.to_owned())).or_insert(i);
        if !interface.id().is_empty() {
            self.by_id.entry(interface.id().to_owned()).or_insert(i);
        }
    }

    fn resolve(&self, side: &str) -> Option<&End> {
        let i = self.by_id.get(side).or_else(|| {
            let (owner, name) = side.rsplit_once(':')?;
            self.by_owner.get(&(owner.to_owned(), name.to_owned()))
        })?;
        self.ends.get(*i)
    }
}

fn collect_links<'a>(el: &'a El, out: &mut Vec<(&'a str, &'a str)>) {
    for c in &el.children {
        if c.name == "InternalLink" {
            if let (Some(a), Some(b)) = (c.attr("RefPartnerSideA"), c.attr("RefPartnerSideB")) {
                out.push((a, b));
            }
        } else {
            collect_links(c, out);
        }
    }
}

/// The first element at or below `el` with the role `role`.
fn find_role<'a>(el: &'a El, role: &str) -> Option<&'a El> {
    el.elements()
        .find_map(|c| if c.role() == Some(role) { Some(c) } else { find_role(c, role) })
}

fn project_child(el: &El, image: &mut ExchangeImage, ends: &mut Ends) -> Result<(), ImportError> {
    match el.role() {
        Some("Device") => {
            let index = image.stations.len();
            let mut station = XStation {
                id: el.id().to_owned(),
                name: el.attr("Name").unwrap_or("").to_owned(),
                type_identifier: el.value("TypeIdentifier"),
                firmware: el.value("FirmwareVersion"),
                iec: iec(el),
                ..XStation::default()
            };
            for child in el.elements() {
                station_child(child, index, &mut station, image, ends)?;
            }
            image.stations.push(station);
        }
        Some("TagTable") => tag_table(el, image, ends),
        Some("Subnet") => {
            image.networks.push(XNetwork {
                id: el.id().to_owned(),
                name: el.attr("Name").unwrap_or("").to_owned(),
                bus: el.value("Type"),
            });
            for i in el.interfaces() {
                ends.add(el.id(), i, End::Subnet(el.id().to_owned()));
            }
        }
        // a folder or a group: what it holds belongs to the project
        _ => {
            for child in el.elements() {
                project_child(child, image, ends)?;
            }
        }
    }
    Ok(())
}

/// An element below a station: an item, an interface, or a tag table under the CPU.
fn station_child(
    el: &El,
    index: usize,
    station: &mut XStation,
    image: &mut ExchangeImage,
    ends: &mut Ends,
) -> Result<(), ImportError> {
    match el.role() {
        Some("CommunicationInterface") => {
            station.interfaces.push(interface(el, index, station.interfaces.len(), ends));
        }
        Some("TagTable") => tag_table(el, image, ends),
        Some("DeviceItem") => {
            let item = item(el, index, station, image, ends)?;
            station.items.push(item);
        }
        _ => {
            for child in el.elements() {
                station_child(child, index, station, image, ends)?;
            }
        }
    }
    Ok(())
}

fn item(
    el: &El,
    index: usize,
    station: &mut XStation,
    image: &mut ExchangeImage,
    ends: &mut Ends,
) -> Result<XItem, ImportError> {
    let type_identifier = el.value("TypeIdentifier");
    let mut out = XItem {
        id: el.id().to_owned(),
        name: el.attr("Name").unwrap_or("").to_owned(),
        position: el.number("PositionNumber")?.unwrap_or(0),
        type_name: el.value("TypeName"),
        order_number: type_identifier
            .as_deref()
            .and_then(|t| t.strip_prefix("OrderNumber:"))
            .map(str::to_owned),
        firmware: el.value("FirmwareVersion"),
        built_in: el
            .value("BuiltIn")
            .is_some_and(|b| b.eq_ignore_ascii_case("true")),
        iec: iec(el),
        ..XItem::default()
    };
    if let Some(list) = el.attribute("Address") {
        for entry in list.children.iter().filter(|c| c.name == "Attribute") {
            let (Some(start), Some(bits), Some(io)) = (
                entry.number("StartAddress")?,
                entry.number("Length")?,
                entry.value("IoType").as_deref().and_then(IoType::from_word),
            ) else {
                continue;
            };
            out.addresses.push(XAddr { start, bits, io });
        }
    }
    for i in el.interfaces() {
        if i.class() != Some("Channel") {
            continue;
        }
        let Some(io) = i.value("IoType").as_deref().and_then(IoType::from_word) else {
            continue;
        };
        let channel = XChannel {
            id: i.id().to_owned(),
            name: i.attr("Name").unwrap_or("").to_owned(),
            kind: match i.value("Type") {
                Some(t) if t.eq_ignore_ascii_case("analog") => ChannelType::Analog,
                _ => ChannelType::Digital,
            },
            io,
            number: i.number("Number")?.unwrap_or(0),
            bits: i.number("Length")?.unwrap_or(1),
        };
        ends.add(el.id(), i, End::Channel(channel.id.clone()));
        out.channels.push(channel);
    }
    for child in el.elements() {
        match child.role() {
            Some("CommunicationInterface") => {
                station
                    .interfaces
                    .push(interface(child, index, station.interfaces.len(), ends));
            }
            Some("TagTable") => tag_table(child, image, ends),
            Some("DeviceItem") => out.children.push(item(child, index, station, image, ends)?),
            _ => {
                // a tag folder under a CPU, or an element this reader does not know
                let mut tables = Vec::new();
                collect_role(child, "TagTable", &mut tables);
                for t in tables {
                    tag_table(t, image, ends);
                }
            }
        }
    }
    Ok(out)
}

fn collect_role<'a>(el: &'a El, role: &str, out: &mut Vec<&'a El>) {
    if el.role() == Some(role) {
        out.push(el);
        return;
    }
    for c in el.elements() {
        collect_role(c, role, out);
    }
}

fn interface(el: &El, station: usize, index: usize, ends: &mut Ends) -> XInterface {
    let mut out = XInterface {
        id: el.id().to_owned(),
        label: el
            .value("Label")
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| el.attr("Name").unwrap_or("").to_owned()),
        ..XInterface::default()
    };
    for child in el.elements() {
        match child.role() {
            Some("Node") if out.node.is_none() => {
                out.node = Some(XNode {
                    id: child.id().to_owned(),
                    bus: child.value("Type"),
                    address: child.value("NetworkAddress"),
                    mask: child.value("SubnetMask"),
                    router: child.value("RouterAddress"),
                    device_name: child.value("PnDeviceName"),
                    network: None,
                });
                for i in child.interfaces() {
                    ends.add(child.id(), i, End::Node(station, index));
                }
            }
            Some("CommunicationPort") => out.ports.push(XPort {
                id: child.id().to_owned(),
                label: child
                    .value("Label")
                    .filter(|l| !l.is_empty())
                    .unwrap_or_else(|| child.attr("Name").unwrap_or("").to_owned()),
            }),
            _ => {}
        }
    }
    out
}

fn tag_table(el: &El, image: &mut ExchangeImage, ends: &mut Ends) {
    if image.project.tag_table_id.is_empty() {
        image.project.tag_table_id = el.id().to_owned();
        image.project.tag_table = el.attr("Name").unwrap_or("").to_owned();
    }
    for i in el.interfaces() {
        if i.class() != Some("Tag") {
            continue;
        }
        let mut comment = BTreeMap::new();
        if let Some(c) = i.attribute("Comment") {
            if let Some(plain) = c.child_text("Value").filter(|t| !t.is_empty()) {
                comment.insert(String::new(), plain.to_owned());
            }
            for lang in c.children.iter().filter(|a| a.name == "Attribute") {
                let Some(tag) = lang.attr("Name").and_then(|n| n.strip_prefix("aml-lang=")) else {
                    continue;
                };
                if let Some(text) = lang.child_text("Value").filter(|t| !t.is_empty()) {
                    comment.insert(tag.to_owned(), text.to_owned());
                }
            }
        }
        ends.add(el.id(), i, End::Tag(image.tags.len()));
        image.tags.push(XTag {
            id: i.id().to_owned(),
            name: i.attr("Name").unwrap_or("").to_owned(),
            data_type: i.value("DataType"),
            io: i.value("IoType").as_deref().and_then(IoType::from_word),
            address: i.value("LogicalAddress"),
            comment,
            channel: None,
        });
    }
}

fn iec(el: &El) -> IecTags {
    IecTags {
        designation: el.value("ProductDesignation IEC"),
        location: el.value("LocationIdentifier IEC"),
    }
}

// ---------------------------------------------------------------------------------------------------------
// The document

/// One element of the document: its name without a prefix, its XML attributes, its child elements and its text.
#[derive(Default)]
struct El {
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<El>,
    text: String,
}

impl El {
    /// The XML attribute `name`.
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// The CAEX `ID`, empty when the element has none.
    fn id(&self) -> &str {
        self.attr("ID").unwrap_or("")
    }

    /// The child `InternalElement`s.
    fn elements(&self) -> impl Iterator<Item = &El> {
        self.children.iter().filter(|c| c.name == "InternalElement")
    }

    /// The child `ExternalInterface`s.
    fn interfaces(&self) -> impl Iterator<Item = &El> {
        self.children.iter().filter(|c| c.name == "ExternalInterface")
    }

    /// The role: the last segment of the first `SupportedRoleClass` or `RoleRequirements` path.
    fn role(&self) -> Option<&str> {
        self.children.iter().find_map(|c| {
            let path = match c.name.as_str() {
                "SupportedRoleClass" => c.attr("RefRoleClassPath"),
                "RoleRequirements" => c.attr("RefBaseRoleClassPath"),
                _ => None,
            }?;
            path.rsplit('/').next()
        })
    }

    /// The interface class: the last segment of `RefBaseClassPath`.
    fn class(&self) -> Option<&str> {
        self.attr("RefBaseClassPath")?.rsplit('/').next()
    }

    /// The CAEX attribute `name`.
    fn attribute(&self, name: &str) -> Option<&El> {
        self.children
            .iter()
            .find(|c| c.name == "Attribute" && c.attr("Name") == Some(name))
    }

    /// The text of the child element `name`.
    fn child_text(&self, name: &str) -> Option<&str> {
        self.children
            .iter()
            .find(|c| c.name == name)
            .map(|c| c.text.trim())
    }

    /// The value of the CAEX attribute `name`; `None` when it is absent or empty.
    fn value(&self, name: &str) -> Option<String> {
        self.attribute(name)?
            .child_text("Value")
            .filter(|t| !t.is_empty())
            .map(str::to_owned)
    }

    /// The value of the CAEX attribute `name` as a number.
    fn number(&self, name: &str) -> Result<Option<u32>, ImportError> {
        let Some(text) = self.value(name) else {
            return Ok(None);
        };
        text.parse().map(Some).map_err(|_| {
            malformed(format!(
                "the attribute `{name}` of `{}` is `{text}`, not a whole number",
                self.attr("Name").unwrap_or("")
            ))
        })
    }
}

/// The document of `text`, bounded by `limits`.
fn parse(text: &str, limits: &Limits) -> Result<El, ImportError> {
    let mut reader = Reader::from_str(text);
    // the open elements, the document's holder first
    let mut stack: Vec<El> = vec![El::default()];
    let mut nodes: u64 = 0;
    let mut attributes: u64 = 0;
    loop {
        let event = reader.read_event().map_err(|e| malformed(e.to_string()))?;
        match event {
            Event::Start(e) => {
                if stack.len() as u64 > u64::from(limits.max_xml_depth) {
                    return Err(over("XML depth", u64::from(limits.max_xml_depth)));
                }
                stack.push(element(&e, limits, &mut nodes, &mut attributes)?);
            }
            Event::Empty(e) => {
                let el = element(&e, limits, &mut nodes, &mut attributes)?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(el);
                }
            }
            Event::End(_) => {
                // quick-xml refuses an end tag that matches no start, so the holder is never closed here
                if stack.len() > 1 {
                    if let (Some(el), Some(parent)) = (stack.pop(), stack.last_mut()) {
                        parent.children.push(el);
                    }
                }
            }
            Event::Text(t) => {
                let t = t.xml10_content().map_err(|e| malformed(e.to_string()))?;
                push_text(&mut stack, &t, limits)?;
            }
            Event::CData(t) => {
                let t = t.decode().map_err(|e| malformed(e.to_string()))?;
                push_text(&mut stack, &t, limits)?;
            }
            Event::GeneralRef(r) => {
                let c = match r.resolve_char_ref().map_err(|e| malformed(e.to_string()))? {
                    Some(c) => c,
                    None => match &*r {
                        b"lt" => '<',
                        b"gt" => '>',
                        b"amp" => '&',
                        b"apos" => '\'',
                        b"quot" => '"',
                        other => {
                            return Err(malformed(format!(
                                "an unknown entity `&{};`",
                                String::from_utf8_lossy(other)
                            )));
                        }
                    },
                };
                push_text(&mut stack, c.encode_utf8(&mut [0; 4]), limits)?;
            }
            Event::DocType(_) => {
                return Err(malformed("the file declares a DOCTYPE, and DTDs are refused"));
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if stack.len() != 1 {
        return Err(malformed("the file ends inside an element"));
    }
    let mut holder = stack.pop().unwrap_or_default();
    match holder.children.len() {
        1 => Ok(holder.children.remove(0)),
        0 => Err(malformed("the file holds no element")),
        _ => Err(malformed("the file holds more than one root element")),
    }
}

fn element(
    e: &BytesStart<'_>,
    limits: &Limits,
    nodes: &mut u64,
    attributes: &mut u64,
) -> Result<El, ImportError> {
    *nodes += 1;
    if *nodes > u64::from(limits.max_xml_nodes) {
        return Err(over("XML node", u64::from(limits.max_xml_nodes)));
    }
    let mut el = El {
        name: String::from_utf8_lossy(e.local_name().as_ref()).into_owned(),
        ..El::default()
    };
    for a in e.attributes() {
        let a = a.map_err(|e| malformed(e.to_string()))?;
        *attributes += 1;
        if el.attrs.len() as u64 >= u64::from(limits.max_xml_attributes_per_element) {
            return Err(over(
                "attributes per XML element",
                u64::from(limits.max_xml_attributes_per_element),
            ));
        }
        if *attributes > u64::from(limits.max_xml_attributes) {
            return Err(over("XML attribute", u64::from(limits.max_xml_attributes)));
        }
        if a.value.len() > limits.max_attribute_bytes {
            return Err(over("XML attribute value", limits.max_attribute_bytes as u64));
        }
        let value = a
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|e| malformed(e.to_string()))?;
        // a namespace declaration and a prefixed attribute (`xsi:…`) say nothing about the content
        if a.key.prefix().is_some() || a.key.as_ref() == b"xmlns" {
            continue;
        }
        el.attrs.push((
            String::from_utf8_lossy(a.key.local_name().as_ref()).into_owned(),
            value.into_owned(),
        ));
    }
    Ok(el)
}

fn push_text(stack: &mut [El], text: &str, limits: &Limits) -> Result<(), ImportError> {
    let Some(el) = stack.last_mut() else {
        return Ok(());
    };
    if (el.text.len() + text.len()) as u64 > u64::from(limits.max_text_bytes) {
        return Err(over("XML text", u64::from(limits.max_text_bytes)));
    }
    el.text.push_str(text);
    Ok(())
}

fn over(what: &str, limit: u64) -> ImportError {
    ImportError::LimitExceeded {
        what: what.to_owned(),
        limit,
    }
}

fn malformed(message: impl Into<String>) -> ImportError {
    ImportError::Malformed {
        message: message.into(),
        line: None,
    }
}

fn unsupported(message: impl Into<String>) -> ImportError {
    ImportError::Unsupported {
        message: message.into(),
    }
}
