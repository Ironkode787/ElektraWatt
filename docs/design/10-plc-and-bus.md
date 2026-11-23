# PLC, I/O and fieldbus

**Status:** Draft v0.7 · **Date:** 2026-10-31 · **Part of:** ElektraWatt design document set

**Purpose.** This document specifies how ElektraWatt (EW) models PLC configuration projects, stations, modules,
channels and fieldbus networks. It defines how EW derives addresses, symbols, function texts and targets instead of
storing them as text, how it exchanges I/O data with TIA Portal, CODESYS and spreadsheets, and which rules check the
result. It elaborates the PLC, BUS and PLC-exchange EXP rows of [02b](02b-requirements-extended.md) on top of the
[domain model](03-domain-model.md) §13–§14 and the accepted [architecture](04-architecture.md) (ADR-0001, ADR-0003,
ADR-0006). The running example and acceptance case is the reference project's Festo valve terminal -201U1 on
folios 3, 106 and 201–206.

**Where each section lives.** The section numbers are the ones the code and the other documents cite.

| Section | File |
|---|---|
| [§1. Scope and boundaries](10-plc-data.md#1-scope-and-boundaries) | [the PLC data](10-plc-data.md) |
| [§2. Concepts and data](10-plc-data.md#2-concepts-and-data) | [the PLC data](10-plc-data.md) |
| [§2.1 Overview](10-plc-data.md#21-overview) | [the PLC data](10-plc-data.md) |
| [§2.2 Configuration project](10-plc-data.md#22-configuration-project) | [the PLC data](10-plc-data.md) |
| [§2.3 Stations and modules](10-plc-data.md#23-stations-and-modules) | [the PLC data](10-plc-data.md) |
| [§2.4 Channels](10-plc-data.md#24-channels) | [the PLC data](10-plc-data.md) |
| [§2.5 Addresses and address profiles](10-plc-data.md#25-addresses-and-address-profiles) | [the PLC data](10-plc-data.md) |
| [§2.6 PLC and bus part data](10-plc-data.md#26-plc-and-bus-part-data) | [the PLC data](10-plc-data.md) |
| [§2.7 Bus model](10-bus.md#27-bus-model) | [the bus](10-bus.md) |
| [§2.8 Device types from device descriptions](10-plc-data.md#28-device-types-from-device-descriptions) | [the PLC data](10-plc-data.md) |
| [§2.9 Safety data](10-plc-data.md#29-safety-data) | [the PLC data](10-plc-data.md) |
| [§2.10 Derived queries](10-plc-data.md#210-derived-queries) | [the PLC data](10-plc-data.md) |
| [§3. Behaviour and rules](10-plc-behaviour.md#3-behaviour-and-rules) | [the PLC behaviour and rules](10-plc-behaviour.md) |
| [§3.1 Address derivation](10-plc-behaviour.md#31-address-derivation) | [the PLC behaviour and rules](10-plc-behaviour.md) |
| [§3.2 Channel states (A3)](10-plc-behaviour.md#32-channel-states-a3) | [the PLC behaviour and rules](10-plc-behaviour.md) |
| [§3.3 Binding and normalisers](10-plc-behaviour.md#33-binding-and-normalisers) | [the PLC behaviour and rules](10-plc-behaviour.md) |
| [§3.4 Symbols, designations and function texts](10-plc-behaviour.md#34-symbols-designations-and-function-texts) | [the PLC behaviour and rules](10-plc-behaviour.md) |
| [§3.5 Target tracking (A4)](10-plc-behaviour.md#35-target-tracking-a4) | [the PLC behaviour and rules](10-plc-behaviour.md) |
| [§3.6 Representations](10-plc-behaviour.md#36-representations) | [the PLC behaviour and rules](10-plc-behaviour.md) |
| [§3.7 Valve terminals](10-plc-behaviour.md#37-valve-terminals) | [the PLC behaviour and rules](10-plc-behaviour.md) |
| [§3.8 Bus topology, addresses and termination](10-bus.md#38-bus-topology-addresses-and-termination) | [the bus](10-bus.md) |
| [§3.9 Commands, freezing and ownership](10-plc-behaviour.md#39-commands-freezing-and-ownership) | [the PLC behaviour and rules](10-plc-behaviour.md) |
| [§3.10 Check rules](10-plc-behaviour.md#310-check-rules) | [the PLC behaviour and rules](10-plc-behaviour.md) |
| [§4. Exchange with PLC software and spreadsheets](10-exchange.md#4-exchange-with-plc-software-and-spreadsheets) | [exchange, outputs and the command line](10-exchange.md) |
| [§5. Outputs: report row sources](10-exchange.md#5-outputs-report-row-sources) | [exchange, outputs and the command line](10-exchange.md) |
| [§6. UI and UX touchpoints](10-window.md#6-ui-and-ux-touchpoints) | [the window](10-window.md) |
| [§7. CLI and automation](10-exchange.md#7-cli-and-automation) | [exchange, outputs and the command line](10-exchange.md) |
| [§8. Edge cases and pitfalls](10-plc-behaviour.md#8-edge-cases-and-pitfalls) | [the PLC behaviour and rules](10-plc-behaviour.md) |
| [§9. The reference project](10-plc-behaviour.md#9-the-reference-project) | [the PLC behaviour and rules](10-plc-behaviour.md) |
