// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The exchange image (doc 10 §4.2): the content of a PLC exchange file as plain data.
//!
//! Every object carries the ID the file gives it (`id`): an export writes the project's own IDs there, so that a
//! re-import finds its objects again (FR-PLC-056); an import keeps whatever the PLC software wrote, and the
//! application decides whether that is one of its own IDs or a foreign one.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What one exchange file says about one configuration project.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExchangeImage {
    /// The configuration project.
    pub project: XProject,
    /// The stations, in the file's order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stations: Vec<XStation>,
    /// The tags (symbols), in the file's order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<XTag>,
    /// The subnets the bus nodes are on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub networks: Vec<XNetwork>,
}

/// The configuration project of an image (AR APC `AutomationProject`).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct XProject {
    /// The ID in the file.
    pub id: String,
    /// The name ("PLC1").
    pub name: String,
    /// The ID of the tag table in the file.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tag_table_id: String,
    /// The name of the tag table the tags are in ("EW_IO").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tag_table: String,
}

/// The IEC designations an object carries (`ProductDesignation IEC`, `LocationIdentifier IEC`).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IecTags {
    /// The product designation ("-201U1").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub designation: Option<String>,
    /// The location ("+J-E-2416").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

impl IecTags {
    /// Whether neither designation is set.
    pub fn is_empty(&self) -> bool {
        self.designation.is_none() && self.location.is_none()
    }
}

/// A station (AR APC `Device`).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct XStation {
    /// The ID in the file.
    pub id: String,
    /// The name.
    pub name: String,
    /// The type identifier (`OrderNumber:195740`, `System:Device.S71500`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_identifier: Option<String>,
    /// The firmware version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firmware: Option<String>,
    /// The IEC designations.
    #[serde(default, skip_serializing_if = "IecTags::is_empty")]
    pub iec: IecTags,
    /// The racks, modules and submodules, in position order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<XItem>,
    /// The bus interfaces.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interfaces: Vec<XInterface>,
}

/// A rack, module or submodule (AR APC `DeviceItem`).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct XItem {
    /// The ID in the file.
    pub id: String,
    /// The name.
    pub name: String,
    /// The position among its siblings (`PositionNumber`): the slot of a module.
    pub position: u32,
    /// The type name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    /// The order number (the `OrderNumber:` type identifier without its prefix).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order_number: Option<String>,
    /// The firmware version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firmware: Option<String>,
    /// Whether the item is built into its parent.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub built_in: bool,
    /// The I/O ranges, at most one per direction as the application writes them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub addresses: Vec<XAddr>,
    /// The channels, in number order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<XChannel>,
    /// The items below this one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<XItem>,
    /// The IEC designations.
    #[serde(default, skip_serializing_if = "IecTags::is_empty")]
    pub iec: IecTags,
}

/// The direction of an I/O range, a channel or a tag.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IoType {
    /// An input.
    Input,
    /// An output.
    Output,
}

impl IoType {
    /// The AR APC word (`Input`, `Output`).
    pub fn word(self) -> &'static str {
        match self {
            IoType::Input => "Input",
            IoType::Output => "Output",
        }
    }

    /// The direction of an AR APC word, read without regard to case.
    pub fn from_word(word: &str) -> Option<IoType> {
        match word.trim().to_ascii_lowercase().as_str() {
            "input" => Some(IoType::Input),
            "output" => Some(IoType::Output),
            _ => None,
        }
    }
}

/// One I/O range of an item (an entry of the AR APC `Address` list).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct XAddr {
    /// The start byte.
    pub start: u32,
    /// The length in bits.
    pub bits: u32,
    /// The direction.
    pub io: IoType,
}

/// Whether a channel is digital or analogue.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChannelType {
    /// A digital channel.
    Digital,
    /// An analogue channel.
    Analog,
}

impl ChannelType {
    /// The AR APC word (`Digital`, `Analog`).
    pub fn word(self) -> &'static str {
        match self {
            ChannelType::Digital => "Digital",
            ChannelType::Analog => "Analog",
        }
    }
}

/// A channel of an item (AR APC `Channel` interface).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct XChannel {
    /// The ID in the file.
    pub id: String,
    /// The name (`Channel_DO_1`).
    pub name: String,
    /// Digital or analogue.
    pub kind: ChannelType,
    /// The direction.
    pub io: IoType,
    /// The 0-based number.
    pub number: u32,
    /// The length in bits.
    pub bits: u32,
}

/// A tag (AR APC `Tag` interface in a `TagTable`; a row of a tag table).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct XTag {
    /// The ID in the file.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    /// The name: the symbol.
    pub name: String,
    /// The data type ("Bool").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_type: Option<String>,
    /// The direction; the address has no area letter of its own in AML.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io: Option<IoType>,
    /// The logical address as the file prints it (`0.1` in AML, `%Q0.1` in a tag table).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// The comment by language tag; the empty key is a comment without a language.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub comment: BTreeMap<String, String>,
    /// The ID of the channel the tag is linked to (an `InternalLink`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
}

/// A bus interface of a station (AR APC `CommunicationInterface`).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct XInterface {
    /// The ID in the file.
    pub id: String,
    /// The label ("DP", "X1").
    pub label: String,
    /// The node: the interface's address on its subnet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<XNode>,
    /// The ports.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<XPort>,
}

/// The node of an interface (AR APC `Node`).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct XNode {
    /// The ID in the file.
    pub id: String,
    /// The bus type (`Profibus`, `Ethernet`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bus: Option<String>,
    /// The network address: a PROFIBUS station address or an IPv4 address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// The subnet mask.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<String>,
    /// The router (gateway) address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub router: Option<String>,
    /// The PROFINET device name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
    /// The ID of the subnet the node is on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
}

/// A port of an interface (AR APC `CommunicationPort`).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct XPort {
    /// The ID in the file.
    pub id: String,
    /// The label ("P1R").
    pub label: String,
}

/// A subnet (AR APC `Subnet`).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct XNetwork {
    /// The ID in the file.
    pub id: String,
    /// The name.
    pub name: String,
    /// The bus type (`Profibus`, `Ethernet`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bus: Option<String>,
}
