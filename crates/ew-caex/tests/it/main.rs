// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The tests of the exchange image's AML form.

mod caex215;
mod hostile;
mod round_trip;

use std::collections::BTreeMap;

use ew_caex::{
    AmlOptions, ApcVersion, ChannelType, ExchangeImage, IecTags, IoType, XAddr, XChannel, XInterface, XItem,
    XNetwork, XNode, XPort, XProject, XStation, XTag,
};

/// An image with one of everything the writer writes: a station with firmware and two modules (one with a
/// submodule), channels, tags with comments in two languages, a bus interface with a node and ports, and a subnet.
fn sample() -> ExchangeImage {
    let channel = |module: &str, n: u32| XChannel {
        id: format!("{module}-ch{n}"),
        name: format!("Channel_DO_{n}"),
        kind: ChannelType::Digital,
        io: IoType::Output,
        number: n,
        bits: 1,
    };
    let module = |id: &str, position: u32, start: u32| XItem {
        id: id.to_owned(),
        name: format!("-{id}"),
        position,
        type_name: Some("VMPA1-FB-EMG-8".to_owned()),
        order_number: Some("533361".to_owned()),
        addresses: vec![XAddr {
            start,
            bits: 8,
            io: IoType::Output,
        }],
        channels: (0..8).map(|n| channel(id, n)).collect(),
        iec: IecTags {
            designation: Some(format!("-{id}")),
            location: Some("+A1".to_owned()),
        },
        ..XItem::default()
    };
    let mut second = module("201U4", 2, 1);
    second.children.push(XItem {
        id: "201U4-sub".to_owned(),
        name: "Sub <1> & \"two\"".to_owned(),
        position: 1,
        built_in: true,
        firmware: Some("V1.2".to_owned()),
        addresses: vec![XAddr {
            start: 4,
            bits: 16,
            io: IoType::Input,
        }],
        ..XItem::default()
    });
    ExchangeImage {
        project: XProject {
            id: "cfg".to_owned(),
            name: "PLC1".to_owned(),
            tag_table_id: "cfg-tags".to_owned(),
            tag_table: "EW_IO".to_owned(),
        },
        stations: vec![XStation {
            id: "201U1".to_owned(),
            name: "-201U1".to_owned(),
            type_identifier: Some("OrderNumber:195740".to_owned()),
            firmware: Some("V2.0".to_owned()),
            iec: IecTags {
                designation: Some("-201U1".to_owned()),
                location: Some("+A1".to_owned()),
            },
            items: vec![module("201U3", 1, 0), second],
            interfaces: vec![XInterface {
                id: "if-x1".to_owned(),
                label: "X1".to_owned(),
                node: Some(XNode {
                    id: "node-x1".to_owned(),
                    bus: Some("Ethernet".to_owned()),
                    address: Some("192.168.0.10".to_owned()),
                    mask: Some("255.255.255.0".to_owned()),
                    router: None,
                    device_name: Some("cpx-1".to_owned()),
                    network: Some("net-1".to_owned()),
                }),
                ports: vec![
                    XPort {
                        id: "p1".to_owned(),
                        label: "P1R".to_owned(),
                    },
                    XPort {
                        id: "p2".to_owned(),
                        label: "P2R".to_owned(),
                    },
                ],
            }],
        }],
        tags: vec![
            XTag {
                id: "tag-1".to_owned(),
                name: "TA05_V_15".to_owned(),
                data_type: Some("Bool".to_owned()),
                io: Some(IoType::Output),
                address: Some("0.1".to_owned()),
                comment: BTreeMap::from([
                    ("en-GB".to_owned(), "valve 15 open".to_owned()),
                    ("nl-NL".to_owned(), "klep 15 open".to_owned()),
                ]),
                channel: Some("201U3-ch1".to_owned()),
            },
            XTag {
                id: "tag-2".to_owned(),
                name: "TA05_V_36".to_owned(),
                data_type: Some("Bool".to_owned()),
                io: Some(IoType::Output),
                address: Some("1.1".to_owned()),
                comment: BTreeMap::new(),
                channel: Some("201U4-ch1".to_owned()),
            },
        ],
        networks: vec![XNetwork {
            id: "net-1".to_owned(),
            name: "PN/IE_1".to_owned(),
            bus: Some("Ethernet".to_owned()),
        }],
    }
}

fn options(apc: ApcVersion) -> AmlOptions {
    AmlOptions {
        apc,
        file_name: "PLC1.aml".to_owned(),
        date: "2026-01-02".to_owned(),
        writer_version: "0.0.0".to_owned(),
        project_title: "Test".to_owned(),
        project_id: "project".to_owned(),
    }
}
