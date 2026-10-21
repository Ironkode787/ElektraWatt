// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The shipped Rust rules, one module per pack family (doc 15 §4). Each rule is a unit struct with a
//! `RuleMeta` constant that copies its row of the catalogue (ID, code, pack, kind, modes, severities,
//! scope) and implements [`Rule`](crate::Rule).

pub mod core;
pub mod num;
pub mod pot;
pub mod rds;
pub mod term;
