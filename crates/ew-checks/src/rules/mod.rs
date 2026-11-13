// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The shipped Rust rules, one module per pack family (doc 15 §4). Each rule is a unit struct with a
//! `RuleMeta` constant that copies its row of the catalogue (ID, code, pack, kind, modes, severities,
//! scope) and implements [`Rule`](crate::Rule).

pub mod bus;
// The EW-BUS rules of the pack's 1.0 content
pub mod bus_net;
pub mod cable;
pub mod core;
pub mod device;
// The EW-FL MVP rules
pub mod fluid;
pub mod imp;
// The EW-IOL rules
pub mod iolink;
// The EW-LAYOUT MVP rules
pub mod layout;
pub mod num;
pub mod page;
pub mod plc;
// The EW-PLC rules of the pack's 1.0 content
pub mod plc_wiring;
pub mod pot;
// EW-PRC-001
pub mod process;
pub mod rds;
pub mod rel;
// The EW-SAF rules
pub mod safety;
pub mod system;
pub mod term;
