// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The action table, section `automation`: the window's entries of the PLC, bus and fluid commands (WP-S2.13), each
//! building one declared command (`plc.*`, `bus.*`, `fluid.*`, `assembly.attach`) from the row or the selection it is
//! aimed at.

use super::super::*;

pub(in crate::actions) const ACTIONS: &[ActionMeta] = &[
    // A CPU row's Attach cards without a CPU (FT-1672): the cards that wait for a CPU become children of the station.
    with_params(
        "station:ref",
        needs_parameters(in_navigator(with_aliases(
            &[("station", ParamForm::Whole)],
            act(
                "plc.attach_cards",
                Command,
                Menu::Project,
                &[],
                writable,
                None,
                |s, p| s.act_plc_attach_cards(p),
            ),
        ))),
    ),
];
