// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The actions' enable checks: each `fn(&ShellModel) -> Result<(), Key>` that an entry of the action table names as
//! its `enabled` or `enabled_when_named`, the reason key of a refusal in its error, and [`enabled_with`], which
//! decides with the parameters of a call and holds the one rule that is not in the rows (the symbol editor's).
//!
//! Not here: the table that names the checks is `actions/table/`; the menus built from it in `actions/menu.rs`.

use serde_json::{Map, Value};

use super::{ActionKind, ActionMeta};
use crate::key;
use crate::shell::ShellModel;
use crate::strings;
use ew_api::Text;

pub(super) fn always(_: &ShellModel) -> Result<(), Text> {
    Ok(())
}

pub(super) fn project_open(m: &ShellModel) -> Result<(), Text> {
    if m.has_project() {
        Ok(())
    } else {
        Err(key!("reason-no-project").into())
    }
}

/// An open project outside the template wizard: the left rail's entries by their keys.
pub(super) fn rail_entries(m: &ShellModel) -> Result<(), Text> {
    project_open(m)?;
    if m.wizard().is_some() {
        return Err(key!("reason-dialog-open").into());
    }
    Ok(())
}

pub(super) fn writable(m: &ShellModel) -> Result<(), Text> {
    project_open(m)?;
    if m.is_writable() {
        Ok(())
    } else {
        Err(key!("reason-read-only").into())
    }
}

/// The project can be written and shows a page (Title block fields…).
pub(super) fn writable_page(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    has_pages(m)
}

/// The project can be written and shows a page of a list (List columns…, Rows of your own…).
pub(super) fn writable_list(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    if m.shows_list() {
        Ok(())
    } else {
        Err(key!("reason-no-list-page").into())
    }
}

/// The project can be written and shows a page of a list, or a legend is selected on its page (List columns…).
pub(super) fn writable_columns(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    if m.shows_list() || m.selected_legend().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-list-or-legend").into())
    }
}

/// The selection is one valve terminal (Generate overview… without a row), else why not in words.
pub(super) fn valve_terminal_selected(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    m.selected_valve_terminal().map(|_| ()).map_err(Into::into)
}

/// The project can be written and the cursor of a station's view stands on a module or a valve (the PLC tab's
/// Replace, Remove and Move), else why not in words.
pub(super) fn station_module(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    m.station_module().map_err(Into::into)
}

/// The project can be written, and the module or the head under the cursor of a station's view is drawn on no page
/// yet (Draw on a page).
pub(super) fn station_module_undrawn(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    match m.station_taken()? {
        _ if m.station_draft().is_some() => Err(key!("reason-rack-draft").into()),
        true => Err(key!("reason-rack-drawn").into()),
        false if on_valve_position(m) => Err(key!("reason-rack-valve-draw").into()),
        false => Ok(()),
    }
}

/// The project can be written, and the module or the head under the cursor of a made station's view is drawn on a page
/// (Move to page); a valve is drawn in its terminal's cell.
pub(super) fn station_module_movable(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    match m.station_taken()? {
        _ if m.station_draft().is_some() => Err(key!("reason-rack-draft").into()),
        _ if on_valve_position(m) => Err(key!("reason-rack-valve-draw").into()),
        true => Ok(()),
        false => Err(key!("reason-nav-not-placed").into()),
    }
}

/// The project can be written and, in a station's view, the page the view stands on takes the device under its
/// cursor (Place here); a navigator's row names its device and says the same itself.
pub(super) fn station_module_placeable(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    if m.station_tab().is_none() {
        return Ok(());
    }
    let device = m.station_device().ok_or(key!("reason-rack-no-module"))?;
    match m.place_here(device) {
        Some(Ok(_)) => Ok(()),
        Some(Err(reason)) => Err(reason.into()),
        None => Err(key!("reason-place-drawn").into()),
    }
}

/// Whether the cursor of a station's view stands on a valve terminal's position.
pub(super) fn on_valve_position(m: &ShellModel) -> bool {
    m.station_tab()
        .and_then(|t| t.cursor)
        .is_some_and(|c| matches!(c, crate::area::StationCursor::Position { .. }))
}

/// [`station_module`], and the cursor stands on a slot, not on a valve position, which takes no move (Move left,
/// Move right).
pub(super) fn station_module_slot(m: &ShellModel) -> Result<(), Text> {
    station_module(m)?;
    if on_valve_position(m) {
        return Err(key!("reason-rack-position").into());
    }
    Ok(())
}

/// The cursor of a station's view stands on a module that is drawn on a page (Go to page).
pub(super) fn station_module_drawn(m: &ShellModel) -> Result<(), Text> {
    match m.station_taken()? {
        true => Ok(()),
        false => Err(key!("reason-nav-not-placed").into()),
    }
}

/// The project can be written and the selection is one multipin valve terminal (Draw the plug).
pub(super) fn multipin_terminal_selected(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    m.selected_multipin_terminal().map(|_| ()).map_err(Into::into)
}

/// The project can be written and a selected valve has open working ports (Plug ports).
pub(super) fn open_ports_selected(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    m.selected_open_ports().map(|_| ()).map_err(Into::into)
}

/// The project can be written and has an open revision (Approve, Release, Set author).
pub(super) fn open_revision(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    if m.open_revision_label().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-open-revision").into())
    }
}

/// The project can be written and has a released revision that is not withdrawn (Withdraw).
pub(super) fn released_revision(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    if m.released_labels().is_empty() {
        Err(key!("reason-no-released-revision").into())
    } else {
        Ok(())
    }
}

/// An incomplete release was found when the project opened and is not settled yet.
pub(super) fn incomplete_release(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    if m.has_incomplete_release() {
        Ok(())
    } else {
        Err(key!("reason-no-incomplete-release").into())
    }
}

/// The project has a parts list to open (a `parts_summary` or `parts_list` report block with pages).
pub(super) fn has_parts_list(m: &ShellModel) -> Result<(), Text> {
    project_open(m)?;
    if m.parts_list_folio().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-parts-list").into())
    }
}

pub(super) fn has_pages(m: &ShellModel) -> Result<(), Text> {
    project_open(m)?;
    if m.page_count() > 0 {
        Ok(())
    } else {
        Err(key!("reason-no-pages").into())
    }
}

/// Export table… and Import table…: a project with a navigator shown as its table, the
/// one the actions read (`ShellModel::table_file_nav`); the Messages pane passes, and its action refuses as
/// `navigator.table` does.
pub(super) fn table_shown(m: &ShellModel) -> Result<(), Text> {
    project_open(m)?;
    m.table_file_nav().map(|_| ()).map_err(Into::into)
}

/// The history panel's row moves and Show changes: a project with the panel open.
pub(super) fn history_open(m: &ShellModel) -> Result<(), Text> {
    project_open(m)?;
    if m.history_shown() {
        Ok(())
    } else {
        Err(key!("reason-history-closed").into())
    }
}

/// Undo to here: a writable project with the panel open.
pub(super) fn history_writable(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    history_open(m)
}

pub(super) fn can_undo(m: &ShellModel) -> Result<(), Text> {
    if m.symbol_editor().is_none() {
        writable(m)?;
    }
    // Something in flight on the pointer is put away by Undo even before the first step.
    if m.can_undo() || (m.symbol_editor().is_none() && m.tool_in_flight()) {
        Ok(())
    } else {
        Err(key!("reason-nothing-to-undo").into())
    }
}

pub(super) fn can_redo(m: &ShellModel) -> Result<(), Text> {
    if m.symbol_editor().is_none() {
        writable(m)?;
    }
    if m.can_redo() || (m.symbol_editor().is_none() && m.tool_in_flight()) {
        Ok(())
    } else {
        Err(key!("reason-nothing-to-redo").into())
    }
}

pub(super) fn has_next(m: &ShellModel) -> Result<(), Text> {
    has_pages(m)?;
    if m.page_index() + 1 < m.page_count() {
        Ok(())
    } else {
        Err(key!("reason-no-next-page").into())
    }
}

pub(super) fn has_previous(m: &ShellModel) -> Result<(), Text> {
    has_pages(m)?;
    if m.page_index() > 0 {
        Ok(())
    } else {
        Err(key!("reason-no-previous-page").into())
    }
}

pub(super) fn has_recent(m: &ShellModel) -> Result<(), Text> {
    if m.recent_count() > 0 {
        Ok(())
    } else {
        Err(key!("reason-no-recent").into())
    }
}

pub(super) fn page_shown(m: &ShellModel) -> Result<(), Text> {
    // The wizard's preview page is a page on show too (`view.snapshot` writes it).
    if m.page_count() > 0 || m.wizard().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-page").into())
    }
}

pub(super) fn can_zoom_in(m: &ShellModel) -> Result<(), Text> {
    has_pages(m)?;
    match m.canvas() {
        Some(c) if c.zoom() >= crate::canvas::ZOOM_MAX => Err(key!("reason-zoom-max").into()),
        _ => Ok(()),
    }
}

pub(super) fn can_zoom_out(m: &ShellModel) -> Result<(), Text> {
    has_pages(m)?;
    match m.canvas() {
        Some(c) if c.zoom() <= crate::canvas::ZOOM_MIN => Err(key!("reason-zoom-min").into()),
        _ => Ok(()),
    }
}

pub(super) fn can_add_tab(m: &ShellModel) -> Result<(), Text> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.tab_count() >= crate::area::MAX_TABS => Err(key!("reason-tabs-full").into()),
        _ => Ok(()),
    }
}

pub(super) fn can_close_tab(m: &ShellModel) -> Result<(), Text> {
    has_pages(m)?;
    match m.area() {
        // A station's view closes even as the last tab: its tab shows its page again.
        Some(a) if a.tab_count() <= 1 && a.current().station.is_none() => Err(key!("reason-last-tab").into()),
        _ => Ok(()),
    }
}

pub(super) fn can_cycle_tabs(m: &ShellModel) -> Result<(), Text> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.focused_group().tabs.len() <= 1 => Err(key!("reason-one-tab").into()),
        _ => Ok(()),
    }
}

pub(super) fn two_groups(m: &ShellModel) -> Result<(), Text> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.side_by_side() => Ok(()),
        _ => Err(key!("reason-one-group").into()),
    }
}

pub(super) fn can_go_back(m: &ShellModel) -> Result<(), Text> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.current().history.back_len() > 0 => Ok(()),
        _ => Err(key!("reason-history-empty").into()),
    }
}

/// Back in the tab group `group` (a tab strip's own Back): its active tab has somewhere to go back to.
pub(super) fn back_in_group(m: &ShellModel, p: &Map<String, Value>) -> Result<(), Text> {
    has_pages(m)?;
    let group = p
        .get("group")
        .and_then(Value::as_u64)
        .and_then(|g| usize::try_from(g).ok());
    match (m.area(), group) {
        (Some(a), Some(g))
            if a.groups
                .get(g)
                .is_some_and(|t| t.active_tab().history.back_len() > 0) =>
        {
            Ok(())
        }
        (Some(_), Some(_)) => Err(key!("reason-history-empty").into()),
        _ => can_go_back(m),
    }
}

/// The sample project is installed (File ▸ Open sample).
pub(super) fn sample_present(m: &ShellModel) -> Result<(), Text> {
    if m.has_sample() {
        Ok(())
    } else {
        Err(key!("error-no-sample").into())
    }
}

pub(super) fn can_go_forward(m: &ShellModel) -> Result<(), Text> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.current().history.forward_len() > 0 => Ok(()),
        _ => Err(key!("reason-history-empty").into()),
    }
}

/// A panel-layout page of a project that can be changed is on show (the reason: "Open a panel layout page").
pub(crate) fn panel_page(m: &ShellModel) -> Result<(), Text> {
    has_pages(m)?;
    writable(m)?;
    match m.page_rows().get(m.page_index()).map(|r| &r.kind) {
        Some(ew_api::PageType::PanelLayout) => Ok(()),
        _ => Err(key!("reason-no-panel-page").into()),
    }
}

/// A panel-layout page with a surface view is on show in a project that can be changed (Draw ▸ Layout is
/// disabled elsewhere with "Open a panel layout page; make one with New page…", and on a panel page that shows no
/// enclosure yet with how to show one).
pub(crate) fn panel_view(m: &ShellModel) -> Result<(), Text> {
    panel_page(m)?;
    if m.panel_tables().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-panel-view").into())
    }
}

/// A placement is selected on a panel page that can be changed.
/// Add legend: a panel page that shows a surface whose layout has no legend yet, on the page or as its page (where
/// the room beside the layout does not hold it whole, the legend gets its page).
pub(super) fn legend_addable(m: &ShellModel) -> Result<(), Text> {
    panel_page(m)?;
    match m.legend_offer() {
        ew_api::output::LegendOffer::At(_) | ew_api::output::LegendOffer::NoRoom => Ok(()),
        ew_api::output::LegendOffer::Exists(_) => Err(key!("reason-legend-exists").into()),
        ew_api::output::LegendOffer::Paged => Err(key!("reason-legend-paged").into()),
        ew_api::output::LegendOffer::NoLayout => Err(key!("reason-legend-no-layout").into()),
    }
}

/// Fit the layout to the sheet: a panel page with a view and no legend box beside the layout, whose room the plate
/// would take (doc 12 §5.4: the box goes to its own page first).
pub(super) fn sheet_fittable(m: &ShellModel) -> Result<(), Text> {
    panel_view(m)?;
    match m.legend_offer() {
        ew_api::output::LegendOffer::Exists(_) => Err(key!("reason-layout-fit-legend").into()),
        _ => Ok(()),
    }
}

/// Move the legend to its own page: a panel page that holds a legend box.
pub(super) fn legend_movable(m: &ShellModel) -> Result<(), Text> {
    panel_page(m)?;
    match m.legend_offer() {
        ew_api::output::LegendOffer::Exists(_) => Ok(()),
        _ => Err(key!("reason-legend-no-box").into()),
    }
}

/// Hide text: one symbol selected on a writable page.
pub(super) fn one_rep_selected(m: &ShellModel) -> Result<(), Text> {
    writable_page(m)?;
    if m.selected_rep_texts().is_some() {
        Ok(())
    } else {
        Err(key!("reason-text-one-symbol").into())
    }
}

/// Put the text back: a selected symbol has a text moved or turned.
pub(super) fn texts_moved(m: &ShellModel) -> Result<(), Text> {
    writable_page(m)?;
    if m.selected_reps_with_moved_texts().is_empty() {
        Err(key!("reason-texts-not-moved").into())
    } else {
        Ok(())
    }
}

/// Keep in place: the selection holds a symbol of a drawn page.
pub(super) fn reps_selected(m: &ShellModel) -> Result<(), Text> {
    writable_page(m)?;
    if m.selected_rep_rows().is_empty() {
        Err(key!("reason-keep-in-place-no-symbol").into())
    } else {
        Ok(())
    }
}

/// Function text over its path (doc 06 §3.8): the selection is one symbol of a device.
pub(super) fn one_device_symbol(m: &ShellModel) -> Result<(), Text> {
    writable_page(m)?;
    match m.selected_rep_rows().as_slice() {
        [one] if one.device.is_some() => Ok(()),
        _ => Err(key!("reason-one-device-symbol").into()),
    }
}

/// Name the strip here (doc 06 §3.8): the selection is one terminal's symbol.
pub(super) fn one_terminal_symbol(m: &ShellModel) -> Result<(), Text> {
    writable_page(m)?;
    match m.selected_rep_rows().as_slice() {
        [one] if one.terminal => Ok(()),
        _ => Err(key!("reason-one-terminal-symbol").into()),
    }
}

/// Reset label position: the selected placement's label was moved.
pub(super) fn label_moved(m: &ShellModel) -> Result<(), Text> {
    panel_selection(m)?;
    if m.selected_label_moved() {
        Ok(())
    } else {
        Err(key!("reason-label-not-moved").into())
    }
}

/// Draw ▸ Layout ▸ Plate: a panel layout page is on show and the project has a mounting plate to show.
pub(super) fn plate_choosable(m: &ShellModel) -> Result<(), Text> {
    panel_page(m)?;
    if m.has_plates() {
        Ok(())
    } else {
        Err(key!("reason-no-plate").into())
    }
}

/// Add hole here…: a panel page is shown in a project that can be changed.
pub(super) fn panel_page_shown(m: &ShellModel) -> Result<(), Text> {
    has_pages(m)?;
    writable(m)?;
    if m.panel_tables().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-panel-page").into())
    }
}

/// Drilling…: a surface of the project has holes.
pub(super) fn has_holes(m: &ShellModel) -> Result<(), Text> {
    has_pages(m)?;
    if m.has_holes() {
        Ok(())
    } else {
        Err(key!("reason-no-holes").into())
    }
}

/// Surface…: a project that can be changed has a cabinet (an enclosure brings its plate).
pub(super) fn surfaces_offered(m: &ShellModel) -> Result<(), Text> {
    has_pages(m)?;
    writable(m)?;
    if m.has_plates() {
        Ok(())
    } else {
        Err(key!("reason-no-plate").into())
    }
}

/// Reset label positions: a label of the panel page shown was moved.
pub(super) fn labels_moved(m: &ShellModel) -> Result<(), Text> {
    panel_page(m)?;
    if m.moved_labels().is_empty() {
        Err(key!("reason-labels-not-moved").into())
    } else {
        Ok(())
    }
}

pub(super) fn panel_selection(m: &ShellModel) -> Result<(), Text> {
    panel_page(m)?;
    if m.selected_placements() == 0 {
        Err(key!("reason-no-placement").into())
    } else {
        Ok(())
    }
}

/// Why the page on show takes no symbols or wires, by its type, its kind in the commands' words (a panel layout
/// page says where its devices come from instead); `None` for a page that takes them. One mapping for the refusal of
/// the editing tools and the canvas HUD's prompt.
pub(crate) fn page_type_reason(m: &ShellModel) -> Option<Text> {
    use ew_api::PageType;
    let kind = &m.page_rows().get(m.page_index())?.kind;
    match kind {
        PageType::PanelLayout => Some(key!("reason-page-type-no-symbols-panel").into()),
        PageType::SectionDivider
        | PageType::TitlePage
        | PageType::TableOfContents
        | PageType::Report
        | PageType::ModelView
        | PageType::ExternalDocument => Some(strings::keyed(
            key!("reason-page-type-no-symbols"),
            vec![ew_api::page_kind_words(kind)],
        )),
        _ => None,
    }
}

/// Route's page (doc 12 §12.5): a page that shows a drawing, or a panel page that shows a cabinet's plate.
pub(crate) fn route_page(m: &ShellModel) -> Result<(), Text> {
    let cabinet = || {
        m.current_folio().is_some_and(|f| {
            m.project_session()
                .is_some_and(|s| s.cabinet_of_folio(f).is_some())
        })
    };
    match editable_page(m) {
        Ok(()) => Ok(()),
        Err(_) if writable(m).is_ok() && cabinet() => Ok(()),
        Err(_) if writable(m).is_ok() && page_type_reason(m).is_some() => {
            Err(key!("reason-route-page").into())
        }
        Err(e) => Err(e),
    }
}

/// A drawn page of a project that can be changed is on show (the editing tools).
pub(crate) fn editable_page(m: &ShellModel) -> Result<(), Text> {
    has_pages(m)?;
    writable(m)?;
    // The page types without wire or symbol tools (doc 06 §2.2: title page, panel layout, model view, external
    // document; the generated table of contents, dividers and reports are not drawn pages at all).
    // The refusal names the page's kind in words.
    if let Some(reason) = page_type_reason(m) {
        return Err(reason);
    }
    match m.canvas() {
        Some(c) if !c.page().tables.page.is_nil() => Ok(()),
        _ => Err(key!("reason-not-editable").into()),
    }
}

/// A drawn page of a writable project for Paste (window check, item 3), of a kind that takes a paste: a kind that takes
/// none names itself in the reason (a generated report has no canvas at all, and says so rather than the general one).
pub(super) fn paste_page(m: &ShellModel) -> Result<(), Text> {
    has_pages(m)?;
    writable(m)?;
    if let Some(reason) = m.paste_page_refusal() {
        return Err(reason);
    }
    match m.canvas() {
        Some(c) if !c.page().tables.page.is_nil() => Ok(()),
        _ => Err(key!("reason-not-editable").into()),
    }
}

/// An editable page whose type has a connection kind the Wire tool draws.
pub(super) fn wire_page(m: &ShellModel) -> Result<(), Text> {
    editable_page(m)?;
    m.wire_kind().map(|_| ()).map_err(Into::into)
}

/// An editable bus-topology page that does not draw a station or bus device of the project yet (Put a station on this
/// page…).
pub(super) fn put_station_page(m: &ShellModel) -> Result<(), Text> {
    editable_page(m)?;
    if m.page_rows().get(m.page_index()).map(|r| &r.kind) != Some(&ew_api::PageType::BusTopology) {
        return Err(key!("reason-put-station-bus-page").into());
    }
    if m.stations_off_page(None).is_empty() {
        return Err(key!("reason-no-station-off-page").into());
    }
    Ok(())
}

/// An editable fluid page (Draw ▸ Fluid line…); elsewhere (another page, a station's view) the reason names the
/// fluid page (in a station's view the Tube's hover spoke of symbols on drawn pages).
pub(super) fn fluid_page(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    match m.wire_kind() {
        Ok(Some("fluid")) => editable_page(m),
        _ => Err(key!("reason-not-fluid-page").into()),
    }
}

/// An editable page with two or more terminals of one row selected (Spacing…).
pub(super) fn terminal_row(m: &ShellModel) -> Result<(), Text> {
    editable_page(m)?;
    if m.terminal_row_selected() {
        Ok(())
    } else {
        Err(key!("reason-terminals-spacing").into())
    }
}

/// An editable page whose selection gives a bond its two ends: exactly one selected connection, a wire and no tube
/// (Draw ▸ Bond). The headless form that names the two ends (`a` and `b`) needs only the editable page
/// ([`enabled_with`]).
pub(super) fn bond_ends(m: &ShellModel) -> Result<(), Text> {
    editable_page(m)?;
    if !m.one_connection_selected() {
        Err(key!("reason-bond-selection").into())
    } else if m.fluid_line_selected() {
        Err(key!("reason-bond-fluid").into())
    } else {
        Ok(())
    }
}

/// An editable page whose selection names one point a potential is defined at (Draw ▸ Define potential…).
pub(super) fn potential_point(m: &ShellModel) -> Result<(), Text> {
    // A Potentials row's form defines at the row's point, whatever page is on show.
    if m.potential_pinned() {
        return writable(m);
    }
    editable_page(m)?;
    m.potential_point().map(|_| ()).map_err(Into::into)
}

/// Whether the action `meta` can run now with the parameters `p`: its `enabled`, or its `enabled_when_named` where the
/// parameters name what `enabled` reads from the selection. The menus and the palette, which send no such parameters,
/// read `enabled` alone.
///
/// The one decision that is not in the action's row is the symbol editor's: while it shows, the tools and every
/// command that is neither the editor's (`symbol.`) nor a file's (`file.`) are refused, but for the rows marked
/// `in_symbol_editor`, and Save goes whatever the project's state, because it saves the symbol. It is written here
/// once by the IDs' prefixes because it holds for whole families of actions, several hundred rows, and for every
/// action added to them.
///
/// # Errors
///
/// The reason key of the refusal.
pub fn enabled_with(
    meta: &ActionMeta,
    m: &ShellModel,
    p: &serde_json::Map<String, Value>,
) -> Result<(), Text> {
    if m.symbol_editor().is_some() {
        if meta.id == "file.save" {
            return Ok(());
        }
        if meta.id.starts_with("tool.")
            || (meta.kind == ActionKind::Command
                && !meta.id.starts_with("symbol.")
                && !meta.id.starts_with("file.")
                && !meta.in_symbol_editor)
        {
            return Err(key!("symbol-editor-active").into());
        }
    }
    match meta.enabled_when_named {
        Some((params, enabled)) if params.iter().any(|k| p.contains_key(*k)) => enabled(m, p),
        _ => (meta.enabled)(m),
    }
}

impl ActionMeta {
    /// Whether the action can run now from a place that sends no parameters (a menu, the command search, a pane's
    /// button): [`enabled_with`] without parameters, so that it is greyed where the dispatch would refuse it (the
    /// symbol editor's rule included).
    ///
    /// # Errors
    ///
    /// The reason key of the refusal.
    pub fn enabled_now(&self, m: &ShellModel) -> Result<(), Text> {
        enabled_with(self, m, &serde_json::Map::new())
    }
}

/// A command tab (`tab`): a tab that needs a project has one ([`crate::command_bar::tab_reason`]).
pub(super) fn tab_named(m: &ShellModel, p: &Map<String, Value>) -> Result<(), Text> {
    let tab = p
        .get("tab")
        .and_then(Value::as_str)
        .and_then(crate::command_bar::Tab::from_word);
    match tab {
        Some(tab) if crate::command_bar::tab_reason(m, tab).is_some() => {
            Err(key!("reason-tab-needs-project").into())
        }
        _ => Ok(()),
    }
}

/// Update… of the Libraries pane: while its table is on show, a copy it would update has an update (the marked rows,
/// else the cursor's, else every copy); without the table the handler counts.
pub(super) fn libraries_updatable(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    let shown = m
        .nav(ew_api::output::NavKind::Libraries)
        .is_some_and(|n| n.table().is_some());
    if shown && m.libraries_update_count() == 0 {
        Err(key!("reason-no-updates").into())
    } else {
        Ok(())
    }
}

// The checks of a call that names its record (`ActionMeta::enabled_when_named`): what a row's menu sends, so that the
// row's entry is greyed where the dispatch would refuse it, with the same reason.

/// The record the parameter `name` names by its UUID; `None` for another value (an alias or a tag, which the handler
/// resolves and checks itself).
fn named_record(p: &Map<String, Value>, name: &str) -> Option<ew_api::output::Uuid> {
    p.get(name)?.as_str()?.parse().ok()
}

/// A row's Cable… (`cable`): the page can be changed and the cable has wires to take.
pub(super) fn cable_named(m: &ShellModel, p: &Map<String, Value>) -> Result<(), Text> {
    writable_page(m)?;
    match (m.project_session(), named_record(p, "cable")) {
        (Some(s), Some(cable)) if s.cable_wires(cable).is_empty() => Err(key!("reason-cable-no-wire").into()),
        _ => Ok(()),
    }
}

/// Place port (`device` and `port`): an editable bus-topology page, where a port is the box of its interface.
pub(super) fn port_named(m: &ShellModel, _: &Map<String, Value>) -> Result<(), Text> {
    editable_page(m)?;
    if m.page_rows().get(m.page_index()).map(|r| &r.kind) == Some(&ew_api::PageType::BusTopology) {
        Ok(())
    } else {
        Err(key!("reason-port-bus-page").into())
    }
}

/// Attach cards to a station (`station`): a card waits for a CPU.
pub(super) fn cards_named(m: &ShellModel, _: &Map<String, Value>) -> Result<(), Text> {
    writable(m)?;
    if m.project_session()
        .is_some_and(|s| s.plc_cards_without_cpu().is_empty())
    {
        Err(key!("reason-no-cards-without-cpu").into())
    } else {
        Ok(())
    }
}

/// Add valve (`module`): the module has a valve part to add.
pub(super) fn valve_part_named(m: &ShellModel, p: &Map<String, Value>) -> Result<(), Text> {
    writable(m)?;
    match (m.project_session(), named_record(p, "module")) {
        (Some(s), Some(module)) if s.fluid_menu(module).valve_part.is_none() => {
            Err(key!("reason-valve-no-part").into())
        }
        _ => Ok(()),
    }
}

/// Go to a thing by its drawing (`record`): the record is drawn.
pub(super) fn drawn_named(m: &ShellModel, p: &Map<String, Value>) -> Result<(), Text> {
    has_pages(m)?;
    match (m.project_session(), named_record(p, "record")) {
        (Some(s), Some(record)) if s.first_drawing(record).is_none() => {
            Err(key!("reason-nav-not-placed").into())
        }
        _ => Ok(()),
    }
}

/// Add to a PLC (`device`): a station names its PLC, a device with passive bus ports only takes no part.
pub(super) fn plc_device_named(m: &ShellModel, p: &Map<String, Value>) -> Result<(), Text> {
    writable(m)?;
    let offer = m
        .project_session()
        .zip(named_record(p, "device"))
        .map(|(s, device)| s.plc_offer(device));
    match offer {
        Some(ew_api::output::PlcOffer::Station(name)) => {
            Err(strings::keyed(key!("reason-plc-station-of"), vec![name.into()]))
        }
        Some(ew_api::output::PlcOffer::Passive) => Err(key!("reason-plc-passive").into()),
        _ => Ok(()),
    }
}

/// Choose part… for a strip (`strip`): a terminal of the strip has no part yet.
pub(super) fn strip_parts_named(m: &ShellModel, p: &Map<String, Value>) -> Result<(), Text> {
    writable(m)?;
    match named_record(p, "strip") {
        Some(strip) if m.strip_terminals_without_part(strip).is_empty() => {
            Err(key!("reason-strip-all-parts").into())
        }
        _ => Ok(()),
    }
}

/// An editable page with exactly one junction selected (Edit ▸ Junction sequence…; window check).
pub(super) fn one_junction(m: &ShellModel) -> Result<(), Text> {
    editable_page(m)?;
    if m.one_junction_selected() {
        Ok(())
    } else {
        Err(key!("reason-one-junction").into())
    }
}

/// An editable page with exactly one interruption point selected (Edit ▸ Continue on page…).
pub(super) fn one_ip(m: &ShellModel) -> Result<(), Text> {
    editable_page(m)?;
    if m.selected_ip_count() == 1 {
        Ok(())
    } else {
        Err(key!("reason-select-one-ip").into())
    }
}

/// A writable project with interruption points selected (Supply comes in here…): without them the command search
/// says how to make them ("supply" found it and nothing said what it needs).
pub(super) fn ips_selected(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    if m.selected_ip_count() >= 1 {
        Ok(())
    } else {
        Err(key!("reason-supply-in-select").into())
    }
}

/// An editable page with at least two interruption points selected (Edit ▸ Arrange interruption points).
pub(super) fn two_ips(m: &ShellModel) -> Result<(), Text> {
    editable_page(m)?;
    if m.selected_ip_count() >= 2 {
        Ok(())
    } else {
        Err(key!("reason-select-ips").into())
    }
}

/// An editable page with exactly one black box selected (Edit ▸ Add connection point, the grips).
pub(super) fn one_box(m: &ShellModel) -> Result<(), Text> {
    editable_page(m)?;
    m.selected_box().map(|_| ()).map_err(Into::into)
}

/// An editable page with one box of a PLC card in the older form selected (Draw the card in one row).
pub(super) fn older_card(m: &ShellModel) -> Result<(), Text> {
    editable_page(m)?;
    m.older_card().map(|_| ()).map_err(Into::into)
}

/// An editable page with one box of a PLC card selected (Terminal wires to card side).
pub(super) fn card_box(m: &ShellModel) -> Result<(), Text> {
    editable_page(m)?;
    m.card_box(key!("reason-not-card-box"))
        .map(|_| ())
        .map_err(Into::into)
}

/// An editable page with exactly one unbound free text selected (Edit ▸ Edit text).
pub(super) fn one_free_text(m: &ShellModel) -> Result<(), Text> {
    editable_page(m)?;
    m.text_edit_target().map(|_| ()).map_err(Into::into)
}

/// An editable page whose selection is two or more terminals of one strip not all bridged yet (Edit ▸ Bridge
/// terminals).
pub(super) fn two_strip_terminals(m: &ShellModel) -> Result<(), Text> {
    // A writable project, not an editable page (the selection may be records).
    writable(m)?;
    m.bridge_command().map(|_| ()).map_err(Into::into)
}

/// The strip editor's gestures: a writable project with the Terminal strips navigator open and focused
/// (headless: open).
pub(super) fn strips_editor(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    m.strip_editor_shown().map_err(Into::into)
}

/// Swap cores: a writable project with the Cables navigator open and focused (headless: open).
pub(super) fn cables_editor(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    m.cables_shown().map_err(Into::into)
}

/// The junction sequence editor is open on a project that can be changed.
pub(super) fn junction_editor_open(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    if m.junction_editor().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-junction-editor").into())
    }
}

/// A project with a page is open: `page.describe` reads a page.
pub(super) fn page_describable(m: &ShellModel) -> Result<(), Text> {
    project_open(m)?;
    if m.page_count() > 0 {
        Ok(())
    } else {
        Err(key!("reason-no-page").into())
    }
}

/// The Place tool is active and its symbol can be an auxiliary function, or the Select tool holds one placed symbol of
/// a device (J offers the Join chooser).
pub(super) fn place_joinable(m: &ShellModel) -> Result<(), Text> {
    editable_page(m)?;
    if m.place_joinable() || m.join_rep().is_some() {
        Ok(())
    } else if m.tool().name() == "place" {
        Err(key!("reason-join-not-auxiliary").into())
    } else {
        Err(key!("reason-join-select").into())
    }
}

/// The Join chooser is open.
pub(super) fn chooser_open(m: &ShellModel) -> Result<(), Text> {
    if m.chooser().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-chooser").into())
    }
}

/// A column mapping is open: a table's in Parts (the set's first step), else the EPLAN report's card.
pub(super) fn mapping_open(m: &ShellModel) -> Result<(), Text> {
    if m.import_set().is_some_and(|s| s.map.is_some()) {
        return Ok(());
    }
    parts_map_open(m)
}

/// The EPLAN report's column mapping card is open on a writable project.
pub(super) fn parts_map_open(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    if m.parts_map().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-parts-map").into())
    }
}

/// The picker is open.
pub(super) fn picker_open(m: &ShellModel) -> Result<(), Text> {
    if m.picker().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-picker").into())
    }
}

pub(super) fn has_link_preview(m: &ShellModel) -> Result<(), Text> {
    has_pages(m)?;
    if m.link_preview().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-preview").into())
    }
}

pub(super) fn can_discard_journal(m: &ShellModel) -> Result<(), Text> {
    if m.can_discard_journal() {
        Ok(())
    } else {
        Err(key!("reason-no-journal").into())
    }
}

/// Update from library needs the damaged-copy banner that offers it.
pub(super) fn damaged_update_offered(m: &ShellModel) -> Result<(), Text> {
    if m.damaged_update_offered() {
        Ok(())
    } else {
        Err(key!("reason-no-damaged-copy-update").into())
    }
}

/// The recovery dialog's answers need the dialog.
pub(super) fn recovery_open(m: &ShellModel) -> Result<(), Text> {
    if matches!(m.dialog(), Some(crate::shell::Dialog::Recovery { .. })) {
        Ok(())
    } else {
        Err(key!("reason-no-recovery").into())
    }
}

pub(super) fn wizard_open(m: &ShellModel) -> Result<(), Text> {
    if m.wizard().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-wizard").into())
    }
}

pub(super) fn first_run_open(m: &ShellModel) -> Result<(), Text> {
    if m.first_run().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-first-run").into())
    }
}

/// The property grid's actions need a project; the grid's own controls also need the pane.
pub(super) fn grid_shown(m: &ShellModel) -> Result<(), Text> {
    project_open(m)?;
    if m.properties_shown() {
        Ok(())
    } else {
        Err(key!("reason-no-sheet").into())
    }
}

/// A grid action that may commit: a writable project with the pane shown.
pub(super) fn grid_writable(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    grid_shown(m)
}

/// The find pane's own controls need the pane.
pub(super) fn find_shown(m: &ShellModel) -> Result<(), Text> {
    project_open(m)?;
    if m.find_shown() {
        Ok(())
    } else {
        Err(key!("reason-no-find").into())
    }
}

/// A replace: a writable project with the find pane shown.
pub(super) fn find_writable(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    find_shown(m)
}

/// The card's buttons: the card sits in the property grid or the find pane, so one of them is shown; an import card
///  is a window of its own and needs neither.
pub(super) fn card_shown(m: &ShellModel) -> Result<(), Text> {
    project_open(m)?;
    // The Preview tab shows every card: a table card is in its navigator.
    if m.properties_shown()
        || m.find_shown()
        || m.preview_shown()
        || m.table_gesture().is_some()
    {
        Ok(())
    } else {
        Err(key!("reason-no-sheet").into())
    }
}

/// Apply on the card: a writable project with the property grid or the find pane shown, or an import card open.
pub(super) fn card_writable(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    card_shown(m)
}

/// `navigator.apply`: a writable project with the Pages pane's reorder card, the Messages pane's
/// quick-fix card or a navigator's table card on show.
pub(super) fn reorder_card(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    if m.properties().card().is_some_and(|c| {
        matches!(
            c.origin,
            crate::properties::CardOrigin::Reorder { .. }
                | crate::properties::CardOrigin::QuickFix { .. }
                | crate::properties::CardOrigin::Table { .. }
        )
    }) {
        Ok(())
    } else {
        Err(key!("reason-no-card").into())
    }
}

/// `table.resolve` and `table.drop`: a writable project with a table card on show.
pub(super) fn table_card(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    if m.table_gesture().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-card").into())
    }
}

/// View ▸ Clear highlights: a highlight is shown.
pub(super) fn has_highlights(m: &ShellModel) -> Result<(), Text> {
    project_open(m)?;
    if m.highlights().is_empty() {
        Err(key!("reason-no-highlights").into())
    } else {
        Ok(())
    }
}

/// The page reorder actions: a writable project with pages.
pub(super) fn pages_writable(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    has_pages(m)
}

/// Copy and Copy tag list: a page is shown and something is selected (a read-only project copies too).
pub(super) fn selection_shown(m: &ShellModel) -> Result<(), Text> {
    page_shown(m)?;
    if m.selection().is_empty() {
        Err(key!("reason-nothing-selected").into())
    } else {
        Ok(())
    }
}

/// Edit ▸ Delete (M39): an editable page with something selected; it deletes the canvas selection only (a navigator's
/// rows have their own Delete, `table.clear`), so with nothing selected it is disabled for that reason, as Cut is.
pub(super) fn editable_selection(m: &ShellModel) -> Result<(), Text> {
    // In a station's view Delete takes out what stands under the cursor: a module, a valve.
    if m.station_tab().is_some() {
        writable(m)?;
        return m.station_taken().map(|_| ()).map_err(Into::into);
    }
    // Delete on a panel page deletes the selected placements and carriers.
    if panel_page(m).is_ok() {
        return if m.selection().is_empty() {
            Err(key!("reason-nothing-selected").into())
        } else {
            Ok(())
        };
    }
    editable_page(m)?;
    // A selection Delete has nothing of on this page (a page chosen in the page list) is not
    // offered here.
    if m.selection().is_empty() || !m.delete_has_target() {
        Err(key!("reason-nothing-selected").into())
    } else {
        Ok(())
    }
}

/// Save as macro…: a page on show with something selected (a window macro) or pages
/// marked in the Pages navigator (a page macro).
pub(super) fn macro_scope_shown(m: &ShellModel) -> Result<(), Text> {
    page_shown(m)?;
    if m.selection().is_empty() && m.navigator().marked().is_empty() {
        Err(key!("reason-nothing-selected").into())
    } else {
        Ok(())
    }
}

/// Cut and Duplicate: an editable page with something selected.
pub(super) fn selection_editable(m: &ShellModel) -> Result<(), Text> {
    editable_page(m)?;
    selection_shown(m)
}

/// Move to page: an editable page with symbols among its selection; with none, it says to select them (a move takes
/// the selected symbols only).
pub(super) fn symbols_selected(m: &ShellModel) -> Result<(), Text> {
    editable_page(m)?;
    let symbols = m
        .canvas()
        .is_some_and(|c| !crate::tools::transform::selected_reps(m.selection(), c.page()).is_empty());
    if symbols {
        Ok(())
    } else {
        Err(key!("reason-move-select-symbols").into())
    }
}

/// An editable page whose selection is shown and holds no tube: a cable's cores are wires (Cable…).
pub(super) fn cable_wires(m: &ShellModel) -> Result<(), Text> {
    editable_page(m)?;
    // Nothing selected says what to select.
    match selection_shown(m) {
        Err(k) if k == key!("reason-nothing-selected") => {
            return Err(key!("reason-cable-select-wires").into());
        }
        other => other?,
    }
    if m.fluid_line_selected() {
        Err(key!("reason-cable-fluid").into())
    } else {
        Ok(())
    }
}

/// The keymap editor's controls need the keyboard shortcuts dialog.
pub(super) fn keymap_editor_open(m: &ShellModel) -> Result<(), Text> {
    if matches!(m.dialog(), Some(crate::shell::Dialog::Shortcuts)) {
        Ok(())
    } else {
        Err(key!("reason-no-keymap-editor").into())
    }
}

/// The paste choice card's choices need the card and a writable project.
pub(super) fn paste_choice_open(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    if matches!(
        m.properties().card().map(|c| &c.origin),
        Some(crate::properties::CardOrigin::PasteVersions(_))
    ) {
        Ok(())
    } else {
        Err(key!("reason-no-paste-choice").into())
    }
}

/// The print pre-dialog's own steps.
pub(super) fn print_dialog_open(m: &ShellModel) -> Result<(), Text> {
    project_open(m)?;
    if m.print_dialog().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-print-dialog").into())
    }
}

/// The part selection dialog's own actions need the dialog.
pub(super) fn part_dialog_open(m: &ShellModel) -> Result<(), Text> {
    project_open(m)?;
    if matches!(m.dialog(), Some(crate::shell::Dialog::PartChoose(_))) {
        Ok(())
    } else {
        Err(key!("reason-no-part-dialog").into())
    }
}

/// Parts is shown (doc 07 §5).
pub(super) fn parts_shown(m: &ShellModel) -> Result<(), Text> {
    if m.parts_place().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-parts-place").into())
    }
}

/// Parts' drawing: while Parts shows, and from a page whose project he can write, where a placed box's Arrange its
/// box… and the device browser's Arrange… open Parts on that part's box first.
pub(super) fn parts_drawing(m: &ShellModel) -> Result<(), Text> {
    parts_shown(m).or_else(|_| writable(m))
}

/// New part…: Parts over the libraries alone (no project), else a writable project.
pub(super) fn parts_writable(m: &ShellModel) -> Result<(), Text> {
    if m.parts_without_project() {
        Ok(())
    } else {
        writable(m)
    }
}

/// The open sheet of Parts, over the libraries alone or in a writable project.
fn open_sheet(m: &ShellModel) -> Result<&crate::shell::parts_place::OpenSheet, Text> {
    parts_writable(m)?;
    m.parts_place()
        .and_then(|p| p.sheet.as_ref())
        .ok_or_else(|| key!("reason-no-part-open").into())
}

/// The open part is the person's to change: his own, or new.
pub(super) fn part_editable(m: &ShellModel) -> Result<(), Text> {
    if open_sheet(m)?.editable() {
        Ok(())
    } else {
        Err(key!("reason-part-not-yours").into())
    }
}

/// A field may be typed: on the drawing, the symbol's short sheet (refused in words where the symbol is not his); else
/// the open part's, where it is his to change.
pub(super) fn field_editable(m: &ShellModel) -> Result<(), Text> {
    if m.symbol_editor().is_some() {
        return Ok(());
    }
    part_editable(m)
}

/// Save: the open part is his to change and was changed (a new part always is); on the drawing, the symbol is his
/// (what it has left to do, Save says above the drawing).
pub(super) fn part_savable(m: &ShellModel) -> Result<(), Text> {
    if let Some(editor) = m.symbol_editor() {
        return match &editor.locked {
            Some(why) => Err(ew_api::Text::bare(ew_api::TextArg::Plain(why.clone()))),
            None => Ok(()),
        };
    }
    // An import's set: Save writes its new and changed parts into My parts.
    if let Some(set) = m.parts_place().filter(|p| p.set_shown()).and_then(|p| p.set.as_deref()) {
        parts_writable(m)?;
        return if set.to_save() > 0 {
            Ok(())
        } else {
            Err(key!("reason-set-nothing-new").into())
        };
    }
    part_editable(m)?;
    let open = open_sheet(m)?;
    if open.changed || open.sheet.new {
        Ok(())
    } else {
        Err(key!("reason-part-not-changed").into())
    }
}

/// Discard: the open part was changed since it was opened or saved; on the drawing, the symbol.
pub(super) fn part_changed(m: &ShellModel) -> Result<(), Text> {
    if let Some(editor) = m.symbol_editor() {
        return if editor.dirty() {
            Ok(())
        } else {
            Err(key!("reason-part-not-changed").into())
        };
    }
    // An import's set: Discard drops it.
    if m.parts_place().is_some_and(|p| p.set_shown()) {
        return Ok(());
    }
    let open = open_sheet(m)?;
    if open.changed || open.sheet.new {
        Ok(())
    } else {
        Err(key!("reason-part-not-changed").into())
    }
}

/// Make my own from it: the open part is not his; on the drawing, the symbol.
pub(super) fn part_not_mine(m: &ShellModel) -> Result<(), Text> {
    if let Some(editor) = m.symbol_editor() {
        return match editor.locked {
            Some(_) => Ok(()),
            None => Err(key!("reason-symbol-is-yours").into()),
        };
    }
    match open_sheet(m)?.lives {
        ew_api::parts::Lives::NotMine { .. } => Ok(()),
        _ => Err(key!("reason-part-own-already").into()),
    }
}

/// Keep in my parts: the open part is only in this project.
pub(super) fn part_only_here(m: &ShellModel) -> Result<(), Text> {
    match open_sheet(m)?.lives {
        ew_api::parts::Lives::OnlyHere if !m.parts_without_project() => Ok(()),
        _ => Err(key!("reason-part-keep").into()),
    }
}

/// Remove…: the open part is in My parts, or only in this project and used by no device.
pub(super) fn part_removable(m: &ShellModel) -> Result<(), Text> {
    let open = open_sheet(m)?;
    match open.lives {
        ew_api::parts::Lives::Mine => Ok(()),
        // With no project nothing is taken out of one.
        ew_api::parts::Lives::OnlyHere if m.parts_without_project() => {
            Err(key!("reason-part-not-own").into())
        }
        ew_api::parts::Lives::OnlyHere if open.sheet.users == 0 => Ok(()),
        ew_api::parts::Lives::OnlyHere => Err(strings::keyed(
            key!("reason-part-remove-used"),
            vec![ew_api::TextArg::Number(
                i64::try_from(open.sheet.users).unwrap_or(i64::MAX),
            )],
        )),
        _ => Err(key!("reason-part-not-own").into()),
    }
}

/// Edit frame: the project can be written and a page is in view whose frame the project holds ("Open a page: a frame
/// is changed on a page that prints it").
pub(super) fn frame_editable(m: &ShellModel) -> Result<(), Text> {
    writable(m)?;
    if m.canvas().is_none() {
        return Err(key!("reason-frame-open-page").into());
    }
    if m.frame_facts().is_some() {
        Ok(())
    } else {
        Err(key!("reason-frame-none").into())
    }
}
