// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The written file against the content model of CAEX 2.15 (IEC 62424:2008 Annex C), written from the standard:
//! the published schema file is not in the repository, since its licence for redistribution is not established
//! (doc 10, Open question 16). For every element the writer emits: its children come in the order of the schema's
//! sequence, it carries the attributes the schema requires and AutomationML adds (a unique `ID`), and every link
//! names an interface that exists.

use std::collections::BTreeSet;

use ew_caex::{ApcVersion, write_aml};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::{options, sample};

/// The header every CAEX object may start with.
const HEADER: [&str; 5] = ["Description", "Version", "Revision", "Copyright", "AdditionalInformation"];

/// An element's content model: the children after the header in schema order, and its required attributes.
fn model(element: &str) -> Option<(&'static [&'static str], &'static [&'static str])> {
    Some(match element {
        "CAEXFile" => (
            &[
                "ExternalReference",
                "InstanceHierarchy",
                "InterfaceClassLib",
                "RoleClassLib",
                "SystemUnitClassLib",
            ],
            &["FileName", "SchemaVersion"],
        ),
        "InstanceHierarchy" => (&["InternalElement"], &["Name"]),
        "InternalElement" => (
            &[
                "Attribute",
                "ExternalInterface",
                "InternalElement",
                "SupportedRoleClass",
                "InternalLink",
                "RoleRequirements",
                "MappingObject",
            ],
            &["Name", "ID"],
        ),
        "ExternalInterface" => (&["Attribute"], &["Name", "ID", "RefBaseClassPath"]),
        "Attribute" => (
            &["DefaultValue", "Value", "RefSemantic", "Constraint", "Attribute"],
            &["Name"],
        ),
        "SupportedRoleClass" => (&["MappingObject"], &["RefRoleClassPath"]),
        "InternalLink" => (&[], &["Name", "RefPartnerSideA", "RefPartnerSideB"]),
        _ => return None,
    })
}

struct Open {
    name: String,
    /// The rank of the last child seen: header children are 0, the model's children 1 + their index.
    rank: usize,
}

fn attr(e: &BytesStart<'_>, name: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == name.as_bytes())
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

fn check(aml: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let mut reader = Reader::from_str(aml);
    let mut stack: Vec<Open> = Vec::new();
    let mut ids = BTreeSet::new();
    // (owner ID, interface name) of every interface, and the sides of every link
    let mut interfaces = BTreeSet::new();
    let mut sides = Vec::new();
    let mut owner: Vec<String> = Vec::new();
    loop {
        let (e, empty) = match reader.read_event().expect("well-formed XML") {
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::End(e) => {
                let closed = stack.pop().expect("an open element");
                if closed.name == "InternalElement" {
                    owner.pop();
                }
                assert_eq!(closed.name.as_bytes(), e.name().as_ref());
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
        // the element in its parent's sequence
        if let Some(parent) = stack.last_mut() {
            if let Some((children, _)) = model(&parent.name) {
                let rank = if HEADER.contains(&name.as_str()) {
                    Some(0)
                } else {
                    children.iter().position(|c| *c == name).map(|i| i + 1)
                };
                match rank {
                    None => problems.push(format!("`{name}` is no child of `{}`", parent.name)),
                    Some(r) if r < parent.rank => problems.push(format!(
                        "`{name}` comes after a later child of `{}`: the schema's sequence is broken",
                        parent.name
                    )),
                    Some(r) => parent.rank = r,
                }
            }
        }
        // its own attributes
        if let Some((_, required)) = model(&name) {
            for a in required {
                if attr(&e, a).is_none_or(|v| v.is_empty()) {
                    problems.push(format!("`{name}` has no `{a}`"));
                }
            }
        }
        match name.as_str() {
            "CAEXFile" => {
                if attr(&e, "SchemaVersion").as_deref() != Some("2.15") {
                    problems.push("the schema version is not 2.15".to_owned());
                }
            }
            "InternalElement" | "ExternalInterface" => {
                let id = attr(&e, "ID").unwrap_or_default();
                if !ids.insert(id.clone()) {
                    problems.push(format!("the ID `{id}` is used twice"));
                }
                if name == "ExternalInterface" {
                    interfaces.insert((
                        owner.last().cloned().unwrap_or_default(),
                        attr(&e, "Name").unwrap_or_default(),
                    ));
                } else if !empty {
                    owner.push(id);
                }
            }
            "InternalLink" => {
                for side in ["RefPartnerSideA", "RefPartnerSideB"] {
                    sides.push(attr(&e, side).unwrap_or_default());
                }
            }
            _ => {}
        }
        if !empty {
            stack.push(Open { name, rank: 0 });
        }
    }
    for side in sides {
        let named = side
            .rsplit_once(':')
            .is_some_and(|(o, n)| interfaces.contains(&(o.to_owned(), n.to_owned())));
        if !named {
            problems.push(format!("the link side `{side}` names no interface"));
        }
    }
    problems
}

#[test]
fn the_written_file_follows_the_caex_2_15_content_model() {
    for (version, _) in ApcVersion::all() {
        let aml = write_aml(&sample(), &options(*version));
        assert_eq!(check(&aml), Vec::<String>::new());
        assert!(aml.contains("AutomationMLVersion=\"2.0\""));
    }
}

#[test]
fn the_check_finds_a_broken_sequence_and_a_dangling_link() {
    let aml = write_aml(&sample(), &options(ApcVersion::V1_4_0));
    // a role before the attributes of its element breaks the sequence
    let role = "<SupportedRoleClass RefRoleClassPath=\"AutomationProjectConfigurationRoleClassLib/Device\"/>";
    let moved = aml.replacen(role, "", 1).replacen(
        "<InternalElement ID=\"201U1\" Name=\"-201U1\">",
        &format!("<InternalElement ID=\"201U1\" Name=\"-201U1\">{role}"),
        1,
    );
    assert!(check(&moved).iter().any(|p| p.contains("sequence")));
    let dangling = aml.replacen("201U3:Channel_DO_1", "201U3:Channel_DO_9", 1);
    assert!(check(&dangling).iter().any(|p| p.contains("names no interface")));
}
