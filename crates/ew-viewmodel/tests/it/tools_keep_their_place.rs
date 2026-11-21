// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1806: a tool on several tabs of the command bar keeps its place. Wire stood on Draw only, in its third group; a
//! person on a bus page with the PLC tab open found another button where Wire had been. Select and Wire, the tools
//! every drawn page is worked with, now lead Draw, PLC and Fluid alike.

use crate::common;

use common::Tmp;
use ew_viewmodel::command_bar::{Group, Tab, command_bar_of};
use ew_viewmodel::shell::{Mode, ShellModel};

/// Where each button of `tab`'s command row stands: (action, group position, position in the group).
fn places(s: &ShellModel, tab: Tab) -> Vec<(&'static str, usize, usize)> {
    command_bar_of(s, tab)
        .groups
        .iter()
        .enumerate()
        .flat_map(|(g, group)| {
            group
                .buttons
                .iter()
                .enumerate()
                .map(move |(b, button)| (button.id, g, b))
        })
        .collect()
}

#[test]
fn select_and_wire_stand_at_the_same_place_on_draw_plc_and_fluid() {
    let tmp = Tmp::new("tools-place");
    let s = tmp.sample_in(Mode::Headless, false, "101");
    let draw = places(&s, Tab::Draw);
    for id in ["tool.select", "tool.wire"] {
        let home = draw.iter().find(|p| p.0 == id).copied();
        assert!(home.is_some(), "{id} on Draw");
        for tab in [Tab::Plc, Tab::Fluid] {
            let there = places(&s, tab).into_iter().find(|p| p.0 == id);
            assert_eq!(there, home, "{id} on {tab:?}");
        }
    }
}

/// The rule, for every group a tab shares: it is the first group of its own tab and leads every tab it is shared
/// with, so each of its buttons has one place wherever it stands.
#[test]
fn a_shared_group_leads_every_tab_it_stands_on() {
    for group in Group::ALL {
        if group.shared_with().is_empty() {
            continue;
        }
        let home = group.tab();
        assert_eq!(home.groups().next(), Some(group), "{group:?} leads its own tab");
        for tab in group.shared_with() {
            assert_ne!(*tab, home, "{group:?}");
            let lead: Vec<Group> = tab.bar_groups().take_while(|g| g.tab() != *tab).collect();
            assert_eq!(lead.first(), Some(&group), "{group:?} leads {tab:?}");
        }
    }
}
