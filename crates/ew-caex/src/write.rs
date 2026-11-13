// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The AML writer (doc 10 §4.3): an [`ExchangeImage`] as AutomationML AR APC on CAEX 2.15.
//!
//! The bytes depend on the image and the options only (FR-EXP-006): elements and attributes are written in the
//! image's order and in the order of the CAEX 2.15 content model (attributes, interfaces, child elements, the role,
//! then the links), the writer header takes its date from the options and never from the clock, and the file names
//! no person. The class libraries are referenced by path and not embedded (FR-PLC-029).

use std::fmt::Write as _;

use crate::image::{ExchangeImage, IecTags, XInterface, XItem, XStation};

/// The AR APC version a file declares (FR-PLC-029, FR-PLC-030).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum ApcVersion {
    /// AR APC 1.4.0 (the default).
    #[default]
    V1_4_0,
    /// AR APC 1.2.0, for older PLC software.
    V1_2_0,
}

impl ApcVersion {
    /// The version text of the file's `Document` element.
    pub fn text(self) -> &'static str {
        VERSIONS[self as usize].1
    }

    /// The version of a version text (`1.4.0`, `1.2.0`).
    pub fn parse(text: &str) -> Option<ApcVersion> {
        VERSIONS.iter().find(|(_, t)| *t == text.trim()).map(|(v, _)| *v)
    }

    /// Every version, with its text.
    pub fn all() -> &'static [(ApcVersion, &'static str)] {
        &VERSIONS
    }
}

/// The version table: what differs between the AR APC versions the writer offers. The element set is the same in
/// both; which attributes an older importer refuses is not verified (doc 10 §4.3).
const VERSIONS: [(ApcVersion, &str); 2] = [(ApcVersion::V1_4_0, "1.4.0"), (ApcVersion::V1_2_0, "1.2.0")];

/// What a written file says beyond its image.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct AmlOptions {
    /// The AR APC version.
    pub apc: ApcVersion,
    /// The file name the root element declares (`PLC1.aml`).
    pub file_name: String,
    /// The export's document date, `YYYY-MM-DD` (the writer header's `LastWritingDateTime`, at midnight).
    pub date: String,
    /// The version of the writing program.
    pub writer_version: String,
    /// The project's title.
    pub project_title: String,
    /// The project's ID.
    pub project_id: String,
}

const ROLES: &str = "AutomationProjectConfigurationRoleClassLib";
const INTERFACES: &str = "AutomationProjectConfigurationInterfaceClassLib";

/// `image` as an AML file.
pub fn write_aml(image: &ExchangeImage, options: &AmlOptions) -> String {
    let mut w = Xml::default();
    w.out.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    w.open(
        "CAEXFile",
        &[
            ("FileName", &options.file_name),
            ("SchemaVersion", "2.15"),
            ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
            ("xsi:noNamespaceSchemaLocation", "CAEX_ClassModel_V2.15.xsd"),
        ],
    );
    w.empty("AdditionalInformation", &[("AutomationMLVersion", "2.0")]);
    w.open("AdditionalInformation", &[]);
    w.open("WriterHeader", &[]);
    w.text("WriterName", "ElektraWatt");
    w.text("WriterID", "ElektraWatt");
    w.text("WriterVendor", "The ElektraWatt contributors");
    w.text("WriterVendorURL", "https://github.com/Ironkode787/ElektraWatt");
    w.text("WriterVersion", &options.writer_version);
    w.text("WriterRelease", &options.writer_version);
    w.text("LastWritingDateTime", &format!("{}T00:00:00", options.date));
    w.text("WriterProjectTitle", &options.project_title);
    w.text("WriterProjectID", &options.project_id);
    w.close("WriterHeader");
    w.close("AdditionalInformation");
    w.open("AdditionalInformation", &[]);
    w.empty(
        "Document",
        &[("DocumentIdentifier", "AR APC"), ("Version", options.apc.text())],
    );
    w.close("AdditionalInformation");

    w.open("InstanceHierarchy", &[("Name", &image.project.name)]);
    w.open(
        "InternalElement",
        &[("ID", &image.project.id), ("Name", &image.project.name)],
    );
    for station in &image.stations {
        station_element(&mut w, station);
    }
    if !image.tags.is_empty() {
        w.open(
            "InternalElement",
            &[("ID", &image.project.tag_table_id), ("Name", &image.project.tag_table)],
        );
        for tag in &image.tags {
            w.open(
                "ExternalInterface",
                &[
                    ("ID", &tag.id),
                    ("Name", &tag.name),
                    ("RefBaseClassPath", &format!("{INTERFACES}/Tag")),
                ],
            );
            if let Some(t) = &tag.data_type {
                w.attribute("DataType", "xs:string", t);
            }
            if let Some(io) = tag.io {
                w.attribute("IoType", "xs:string", io.word());
            }
            if let Some(a) = &tag.address {
                w.attribute("LogicalAddress", "xs:string", a);
            }
            if !tag.comment.is_empty() {
                w.open("Attribute", &[("Name", "Comment"), ("AttributeDataType", "xs:string")]);
                if let Some(plain) = tag.comment.get("") {
                    w.text("Value", plain);
                }
                for (lang, text) in tag.comment.iter().filter(|(l, _)| !l.is_empty()) {
                    w.attribute(&format!("aml-lang={lang}"), "xs:string", text);
                }
                w.close("Attribute");
            }
            w.close("ExternalInterface");
        }
        w.role("TagTable");
        w.close("InternalElement");
    }
    for net in &image.networks {
        w.open("InternalElement", &[("ID", &net.id), ("Name", &net.name)]);
        if let Some(bus) = &net.bus {
            w.attribute("Type", "xs:string", bus);
        }
        w.empty(
            "ExternalInterface",
            &[
                ("ID", &format!("{}.end", net.id)),
                ("Name", SUBNET_END),
                ("RefBaseClassPath", &format!("{INTERFACES}/LogicalEndPoint")),
            ],
        );
        w.role("Subnet");
        w.close("InternalElement");
    }
    w.role("AutomationProject");
    let mut n = 0usize;
    for tag in &image.tags {
        let Some((owner, name)) = tag.channel.as_deref().and_then(|c| channel_owner(image, c)) else {
            continue;
        };
        n += 1;
        w.empty(
            "InternalLink",
            &[
                ("Name", &format!("Link To Tag_{n}")),
                ("RefPartnerSideA", &format!("{owner}:{name}")),
                ("RefPartnerSideB", &format!("{}:{}", image.project.tag_table_id, tag.name)),
            ],
        );
    }
    let mut n = 0usize;
    for node in image
        .stations
        .iter()
        .flat_map(|s| &s.interfaces)
        .filter_map(|i| i.node.as_ref())
    {
        let Some(net) = node
            .network
            .as_deref()
            .filter(|id| image.networks.iter().any(|n| n.id == *id))
        else {
            continue;
        };
        n += 1;
        w.empty(
            "InternalLink",
            &[
                ("Name", &format!("Link To Subnet_{n}")),
                ("RefPartnerSideA", &format!("{}:{NODE_END}", node.id)),
                ("RefPartnerSideB", &format!("{net}:{SUBNET_END}")),
            ],
        );
    }
    w.close("InternalElement");
    w.close("InstanceHierarchy");
    w.close("CAEXFile");
    w.out
}

/// The interface name of a node's end of its link to a subnet.
pub(crate) const NODE_END: &str = "LogicalEndPoint_Node";
/// The interface name of a subnet's end.
pub(crate) const SUBNET_END: &str = "LogicalEndPoint_Subnet";

/// The item that holds channel `id` and the channel's name: the two halves of a link's side.
fn channel_owner<'a>(image: &'a ExchangeImage, id: &str) -> Option<(&'a str, &'a str)> {
    fn find<'a>(items: &'a [XItem], id: &str) -> Option<(&'a str, &'a str)> {
        items.iter().find_map(|i| {
            i.channels
                .iter()
                .find(|c| c.id == id)
                .map(|c| (i.id.as_str(), c.name.as_str()))
                .or_else(|| find(&i.children, id))
        })
    }
    image.stations.iter().find_map(|s| find(&s.items, id))
}

fn station_element(w: &mut Xml, station: &XStation) {
    w.open("InternalElement", &[("ID", &station.id), ("Name", &station.name)]);
    if let Some(t) = &station.type_identifier {
        w.attribute("TypeIdentifier", "xs:string", t);
    }
    if let Some(f) = &station.firmware {
        w.attribute("FirmwareVersion", "xs:string", f);
    }
    iec_attributes(w, &station.iec);
    for item in &station.items {
        item_element(w, item);
    }
    for interface in &station.interfaces {
        interface_element(w, interface);
    }
    w.role("Device");
    w.close("InternalElement");
}

fn iec_attributes(w: &mut Xml, iec: &IecTags) {
    if let Some(d) = &iec.designation {
        w.attribute("ProductDesignation IEC", "xs:string", d);
    }
    if let Some(l) = &iec.location {
        w.attribute("LocationIdentifier IEC", "xs:string", l);
    }
}

fn item_element(w: &mut Xml, item: &XItem) {
    w.open("InternalElement", &[("ID", &item.id), ("Name", &item.name)]);
    w.attribute("PositionNumber", "xs:int", &item.position.to_string());
    if let Some(t) = &item.type_name {
        w.attribute("TypeName", "xs:string", t);
    }
    if let Some(o) = &item.order_number {
        w.attribute("TypeIdentifier", "xs:string", &format!("OrderNumber:{o}"));
    }
    if let Some(f) = &item.firmware {
        w.attribute("FirmwareVersion", "xs:string", f);
    }
    w.attribute(
        "BuiltIn",
        "xs:boolean",
        if item.built_in { "true" } else { "false" },
    );
    if !item.addresses.is_empty() {
        w.open("Attribute", &[("Name", "Address")]);
        for (i, a) in item.addresses.iter().enumerate() {
            w.open("Attribute", &[("Name", &(i + 1).to_string())]);
            w.attribute("StartAddress", "xs:int", &a.start.to_string());
            w.attribute("Length", "xs:int", &a.bits.to_string());
            w.attribute("IoType", "xs:string", a.io.word());
            w.close("Attribute");
        }
        w.close("Attribute");
    }
    iec_attributes(w, &item.iec);
    for c in &item.channels {
        w.open(
            "ExternalInterface",
            &[
                ("ID", &c.id),
                ("Name", &c.name),
                ("RefBaseClassPath", &format!("{INTERFACES}/Channel")),
            ],
        );
        w.attribute("Type", "xs:string", c.kind.word());
        w.attribute("IoType", "xs:string", c.io.word());
        w.attribute("Number", "xs:int", &c.number.to_string());
        w.attribute("Length", "xs:int", &c.bits.to_string());
        w.close("ExternalInterface");
    }
    for child in &item.children {
        item_element(w, child);
    }
    w.role("DeviceItem");
    w.close("InternalElement");
}

fn interface_element(w: &mut Xml, interface: &XInterface) {
    w.open(
        "InternalElement",
        &[("ID", &interface.id), ("Name", &interface.label)],
    );
    w.attribute("Label", "xs:string", &interface.label);
    if let Some(node) = &interface.node {
        w.open("InternalElement", &[("ID", &node.id), ("Name", "Node")]);
        for (name, value) in [
            ("Type", &node.bus),
            ("NetworkAddress", &node.address),
            ("SubnetMask", &node.mask),
            ("RouterAddress", &node.router),
            ("PnDeviceName", &node.device_name),
        ] {
            if let Some(v) = value {
                w.attribute(name, "xs:string", v);
            }
        }
        if node.network.is_some() {
            w.empty(
                "ExternalInterface",
                &[
                    ("ID", &format!("{}.end", node.id)),
                    ("Name", NODE_END),
                    ("RefBaseClassPath", &format!("{INTERFACES}/LogicalEndPoint")),
                ],
            );
        }
        w.role("Node");
        w.close("InternalElement");
    }
    for port in &interface.ports {
        w.open("InternalElement", &[("ID", &port.id), ("Name", &port.label)]);
        w.attribute("Label", "xs:string", &port.label);
        w.role("CommunicationPort");
        w.close("InternalElement");
    }
    w.role("CommunicationInterface");
    w.close("InternalElement");
}

/// An indented XML text under construction.
#[derive(Default)]
struct Xml {
    out: String,
    depth: usize,
}

impl Xml {
    fn start(&mut self, name: &str, attrs: &[(&str, &str)]) {
        for _ in 0..self.depth {
            self.out.push_str("  ");
        }
        let _ = write!(self.out, "<{name}");
        for (k, v) in attrs {
            let _ = write!(self.out, " {k}=\"{}\"", escape(v));
        }
    }

    fn open(&mut self, name: &str, attrs: &[(&str, &str)]) {
        self.start(name, attrs);
        self.out.push_str(">\n");
        self.depth += 1;
    }

    fn empty(&mut self, name: &str, attrs: &[(&str, &str)]) {
        self.start(name, attrs);
        self.out.push_str("/>\n");
    }

    fn close(&mut self, name: &str) {
        self.depth = self.depth.saturating_sub(1);
        for _ in 0..self.depth {
            self.out.push_str("  ");
        }
        let _ = writeln!(self.out, "</{name}>");
    }

    fn text(&mut self, name: &str, text: &str) {
        self.start(name, &[]);
        let _ = writeln!(self.out, ">{}</{name}>", escape(text));
    }

    /// A CAEX attribute with its value.
    fn attribute(&mut self, name: &str, data_type: &str, value: &str) {
        self.open("Attribute", &[("Name", name), ("AttributeDataType", data_type)]);
        self.text("Value", value);
        self.close("Attribute");
    }

    fn role(&mut self, role: &str) {
        self.empty(
            "SupportedRoleClass",
            &[("RefRoleClassPath", &format!("{ROLES}/{role}"))],
        );
    }
}

/// `text` for an attribute value or character data: the five XML entities, and a character XML 1.0 cannot hold
/// (a control character other than tab, LF and CR; U+FFFE, U+FFFF) left out. Tab, LF and CR are written as character
/// references, so that an attribute value keeps them.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' | '\r' => {
                let _ = write!(out, "&#{};", c as u32);
            }
            c if (c as u32) < 0x20 || c == '\u{fffe}' || c == '\u{ffff}' => {}
            c => out.push(c),
        }
    }
    out
}
