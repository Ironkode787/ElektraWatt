// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The shipped Rust rules, one module per pack family (doc 15 §4). Each rule is a unit struct with a
//! `RuleMeta` constant that copies its row of the catalogue (ID, code, pack, kind, modes, severities,
//! scope) and implements [`Rule`](crate::Rule).

pub mod bus;
pub mod cable;
pub mod core;
pub mod device;
pub mod num;
pub mod page;
pub mod plc;
pub mod pot;
pub mod rds;
pub mod rel;
pub mod system;
pub mod term;
