// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What is written is read back, and a CAEX 3.0 file as PLC software writes it is read.

use ew_caex::{ApcVersion, IoType, Limits, read_aml, write_aml};

use crate::{options, sample};

#[test]
fn a_written_image_is_read_back_as_it_was() {
    let image = sample();
    let aml = write_aml(&image, &options(ApcVersion::V1_4_0));
    let back = read_aml(aml.as_bytes(), &Limits::default()).expect("the writer's own file is read");
    assert_eq!(back, image);
}

#[test]
fn the_same_image_and_options_give_the_same_bytes() {
    let a = write_aml(&sample(), &options(ApcVersion::V1_4_0));
    let b = write_aml(&sample(), &options(ApcVersion::V1_4_0));
    assert_eq!(a, b);
    assert!(a.contains("2026-01-02T00:00:00"), "the date is the options' date");
}

#[test]
fn the_older_version_differs_in_the_declared_version_only() {
    let new = write_aml(&sample(), &options(ApcVersion::V1_4_0));
    let old = write_aml(&sample(), &options(ApcVersion::V1_2_0));
    assert!(new.contains("<Document DocumentIdentifier=\"AR APC\" Version=\"1.4.0\"/>"));
    assert!(old.contains("<Document DocumentIdentifier=\"AR APC\" Version=\"1.2.0\"/>"));
    assert_eq!(new.replace("1.4.0", "1.2.0"), old);
    let back = read_aml(old.as_bytes(), &Limits::default()).expect("the older version is read");
    assert_eq!(back, sample());
}

/// A CAEX 3.0 file in the shape PLC software writes: roles as `RoleRequirements`, the tag table below the CPU
/// item, links by the interfaces' own IDs, and elements this reader does not know.
const CAEX_3: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<CAEXFile SchemaVersion="3.0" FileName="station.aml" xmlns="http://www.dke.de/CAEX">
  <SuperiorStandardVersion>AutomationML 2.10</SuperiorStandardVersion>
  <SourceDocumentInformation OriginName="PLC software" OriginID="x" OriginVersion="21" LastWritingDateTime="2026-01-01T00:00:00"/>
  <InstanceHierarchy Name="Project">
    <InternalElement ID="p" Name="Project1">
      <InternalElement ID="folder" Name="Group">
        <InternalElement ID="d" Name="PLC_1">
          <Attribute Name="TypeIdentifier"><Value>System:Device.S71500</Value></Attribute>
          <InternalElement ID="rack" Name="Rail_0">
            <Attribute Name="PositionNumber"><Value>0</Value></Attribute>
            <InternalElement ID="cpu" Name="PLC_1">
              <Attribute Name="PositionNumber"><Value>1</Value></Attribute>
              <Attribute Name="TypeIdentifier"><Value>OrderNumber:6ES7 511-1AK02-0AB0</Value></Attribute>
              <Attribute Name="FirmwareVersion"><Value>V2.9</Value></Attribute>
              <Attribute Name="ProductDesignation IEC"><Value>-K01</Value></Attribute>
              <Attribute Name="Address">
                <Attribute Name="1">
                  <Attribute Name="StartAddress"><Value>4</Value></Attribute>
                  <Attribute Name="Length"><Value>16</Value></Attribute>
                  <Attribute Name="IoType"><Value>Input</Value></Attribute>
                </Attribute>
              </Attribute>
              <ExternalInterface ID="ch0" Name="Channel_DI_0" RefBaseClassPath="AutomationProjectConfigurationInterfaceClassLib/Channel">
                <Attribute Name="Type"><Value>Digital</Value></Attribute>
                <Attribute Name="IoType"><Value>Input</Value></Attribute>
                <Attribute Name="Number"><Value>0</Value></Attribute>
                <Attribute Name="Length"><Value>1</Value></Attribute>
              </ExternalInterface>
              <InternalElement ID="tags" Name="Default tag table">
                <ExternalInterface ID="t1" Name="Start" RefBaseClassPath="AutomationProjectConfigurationInterfaceClassLib/Tag">
                  <Attribute Name="DataType"><Value>Bool</Value></Attribute>
                  <Attribute Name="IoType"><Value>Input</Value></Attribute>
                  <Attribute Name="LogicalAddress"><Value>4.0</Value></Attribute>
                  <Attribute Name="Comment"><Value>start &amp; go</Value></Attribute>
                </ExternalInterface>
                <RoleRequirements RefBaseRoleClassPath="AutomationProjectConfigurationRoleClassLib/TagTable"/>
              </InternalElement>
              <InternalElement ID="if" Name="PROFINET interface_1">
                <InternalElement ID="node" Name="E1">
                  <Attribute Name="NetworkAddress"><Value>192.168.0.1</Value></Attribute>
                  <Attribute Name="SubnetMask"><Value>255.255.255.0</Value></Attribute>
                  <ExternalInterface ID="node-end" Name="LogicalEndPoint_Node" RefBaseClassPath="AutomationProjectConfigurationInterfaceClassLib/LogicalEndPoint"/>
                  <RoleRequirements RefBaseRoleClassPath="AutomationProjectConfigurationRoleClassLib/Node"/>
                </InternalElement>
                <InternalElement ID="port" Name="Port_1">
                  <Attribute Name="Label"><Value>P1 R</Value></Attribute>
                  <RoleRequirements RefBaseRoleClassPath="AutomationProjectConfigurationRoleClassLib/CommunicationPort"/>
                </InternalElement>
                <RoleRequirements RefBaseRoleClassPath="AutomationProjectConfigurationRoleClassLib/CommunicationInterface"/>
              </InternalElement>
              <RoleRequirements RefBaseRoleClassPath="AutomationProjectConfigurationRoleClassLib/DeviceItem"/>
            </InternalElement>
            <RoleRequirements RefBaseRoleClassPath="AutomationProjectConfigurationRoleClassLib/DeviceItem"/>
          </InternalElement>
          <RoleRequirements RefBaseRoleClassPath="AutomationProjectConfigurationRoleClassLib/Device"/>
        </InternalElement>
      </InternalElement>
      <InternalElement ID="net" Name="PN/IE_1">
        <ExternalInterface ID="net-end" Name="LogicalEndPoint_Subnet" RefBaseClassPath="AutomationProjectConfigurationInterfaceClassLib/LogicalEndPoint"/>
        <RoleRequirements RefBaseRoleClassPath="AutomationProjectConfigurationRoleClassLib/Subnet"/>
      </InternalElement>
      <InternalLink Name="Link To Tag_1" RefPartnerSideA="ch0" RefPartnerSideB="t1"/>
      <InternalLink Name="Link To Subnet_1" RefPartnerSideA="net-end" RefPartnerSideB="node-end"/>
      <RoleRequirements RefBaseRoleClassPath="AutomationProjectConfigurationRoleClassLib/AutomationProject"/>
    </InternalElement>
  </InstanceHierarchy>
</CAEXFile>"#;

#[test]
fn a_caex_3_file_of_plc_software_is_read() {
    let image = read_aml(CAEX_3.as_bytes(), &Limits::default()).expect("CAEX 3.0 is read");
    assert_eq!(image.project.name, "Project1");
    let station = &image.stations[0];
    assert_eq!(station.name, "PLC_1");
    assert_eq!(station.type_identifier.as_deref(), Some("System:Device.S71500"));
    let cpu = &station.items[0].children[0];
    assert_eq!(cpu.order_number.as_deref(), Some("6ES7 511-1AK02-0AB0"));
    assert_eq!(cpu.firmware.as_deref(), Some("V2.9"));
    assert_eq!(cpu.iec.designation.as_deref(), Some("-K01"));
    assert_eq!((cpu.addresses[0].start, cpu.addresses[0].bits), (4, 16));
    assert_eq!(cpu.addresses[0].io, IoType::Input);
    // the tag table under the CPU, its tag linked to the channel by the interfaces' own IDs
    assert_eq!(image.project.tag_table, "Default tag table");
    let tag = &image.tags[0];
    assert_eq!(tag.name, "Start");
    assert_eq!(tag.channel.as_deref(), Some("ch0"));
    assert_eq!(tag.comment.get("").map(String::as_str), Some("start & go"));
    // the interface below the CPU item belongs to the station, its node to the subnet
    let interface = &station.interfaces[0];
    assert_eq!(interface.label, "PROFINET interface_1");
    assert_eq!(interface.ports[0].label, "P1 R");
    let node = interface.node.as_ref().expect("the node");
    assert_eq!(node.address.as_deref(), Some("192.168.0.1"));
    assert_eq!(node.network.as_deref(), Some("net"));
    assert_eq!(image.networks[0].name, "PN/IE_1");
}
