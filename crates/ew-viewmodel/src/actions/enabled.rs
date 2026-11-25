// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The actions' enable checks: each `fn(&ShellModel) -> Result<(), Key>` that an entry of the action table names as
//! its `enabled` or `enabled_when_named`, the reason key of a refusal in its error, and [`enabled_with`], which
//! decides with the parameters of a call and holds the one rule that is not in the rows (the symbol editor's).
//!
//! Not here: the table that names the checks is `actions/table/`; the menus built from it in `actions/menu.rs`.

use serde_json::Value;

use super::{ActionKind, ActionMeta};
use crate::key;
use crate::shell::ShellModel;
use crate::strings::Key;

pub(super) fn always(_: &ShellModel) -> Result<(), Key<'static>> {
    Ok(())
}

pub(super) fn project_open(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.has_project() {
        Ok(())
    } else {
        Err(key!("reason-no-project"))
    }
}

pub(super) fn writable(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if m.is_writable() {
        Ok(())
    } else {
        Err(key!("reason-read-only"))
    }
}

/// The project can be written and shows a page (Title block fields…).
pub(super) fn writable_page(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    has_pages(m)
}

/// The project can be written and shows a page of a list (List columns…, Rows of your own…).
pub(super) fn writable_list(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.shows_list() {
        Ok(())
    } else {
        Err(key!("reason-no-list-page"))
    }
}

/// The project can be written and shows a page of a list, or a legend is selected on its page (List columns…).
pub(super) fn writable_columns(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.shows_list() || m.selected_legend().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-list-or-legend"))
    }
}

/// The selection is one valve terminal (Generate overview… without a row), else why not in words.
pub(super) fn valve_terminal_selected(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    m.selected_valve_terminal().map(|_| ())
}

/// The project can be written and the cursor of a station's view stands on a module or a valve (the PLC tab's
/// Replace, Remove and Move), else why not in words.
pub(super) fn station_module(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    m.station_module()
}

/// The project can be written, and the module or the head under the cursor of a station's view is drawn on no page
/// yet (Draw on a page).
pub(super) fn station_module_undrawn(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    match m.station_taken()? {
        _ if m.station_draft().is_some() => Err(key!("reason-rack-draft")),
        true => Err(key!("reason-rack-drawn")),
        false if on_valve_position(m) => Err(key!("reason-rack-valve-draw")),
        false => Ok(()),
    }
}

/// The project can be written, and the module or the head under the cursor of a made station's view is drawn on a page
/// (Move to page); a valve is drawn in its terminal's cell.
pub(super) fn station_module_movable(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    match m.station_taken()? {
        _ if m.station_draft().is_some() => Err(key!("reason-rack-draft")),
        _ if on_valve_position(m) => Err(key!("reason-rack-valve-draw")),
        true => Ok(()),
        false => Err(key!("reason-nav-not-placed")),
    }
}

/// The project can be written and, in a station's view, the page the view stands on takes the device under its
/// cursor (Place here); a navigator's row names its device and says the same itself.
pub(super) fn station_module_placeable(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.station_tab().is_none() {
        return Ok(());
    }
    let device = m.station_device().ok_or(key!("reason-rack-no-module"))?;
    match m.place_here(device) {
        Some(Ok(_)) => Ok(()),
        Some(Err(reason)) => Err(reason),
        None => Err(key!("reason-place-drawn")),
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
pub(super) fn station_module_slot(m: &ShellModel) -> Result<(), Key<'static>> {
    station_module(m)?;
    if on_valve_position(m) {
        return Err(key!("reason-rack-position"));
    }
    Ok(())
}

/// The cursor of a station's view stands on a module that is drawn on a page (Go to page).
pub(super) fn station_module_drawn(m: &ShellModel) -> Result<(), Key<'static>> {
    match m.station_taken()? {
        true => Ok(()),
        false => Err(key!("reason-nav-not-placed")),
    }
}

/// The project can be written and the selection is one multipin valve terminal (Draw the plug).
pub(super) fn multipin_terminal_selected(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    m.selected_multipin_terminal().map(|_| ())
}

/// The project can be written and a selected valve has open working ports (Plug ports).
pub(super) fn open_ports_selected(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    m.selected_open_ports().map(|_| ())
}

/// The project can be written and has an open revision (Approve, Release, Set author).
pub(super) fn open_revision(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.open_revision_label().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-open-revision"))
    }
}

/// The project can be written and has a released revision that is not withdrawn (Withdraw).
pub(super) fn released_revision(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.released_labels().is_empty() {
        Err(key!("reason-no-released-revision"))
    } else {
        Ok(())
    }
}

/// An incomplete release was found when the project opened and is not settled yet.
pub(super) fn incomplete_release(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.has_incomplete_release() {
        Ok(())
    } else {
        Err(key!("reason-no-incomplete-release"))
    }
}

/// The project has a parts list to open (a `parts_summary` or `parts_list` report block with pages).
pub(super) fn has_parts_list(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if m.parts_list_folio().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-parts-list"))
    }
}

pub(super) fn has_pages(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if m.page_count() > 0 {
        Ok(())
    } else {
        Err(key!("reason-no-pages"))
    }
}

/// Export table… and Import table…: a project with a navigator shown as its table, the
/// one the actions read (`ShellModel::table_file_nav`); the Messages pane passes, and its action refuses as
/// `navigator.table` does.
pub(super) fn table_shown(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    m.table_file_nav().map(|_| ())
}

/// The history panel's row moves and Show changes: a project with the panel open.
pub(super) fn history_open(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if m.history_shown() {
        Ok(())
    } else {
        Err(key!("reason-history-closed"))
    }
}

/// Undo to here: a writable project with the panel open.
pub(super) fn history_writable(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    history_open(m)
}

pub(super) fn can_undo(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.symbol_editor().is_none() {
        writable(m)?;
    }
    // Something in flight on the pointer is put away by Undo even before the first step.
    if m.can_undo() || (m.symbol_editor().is_none() && m.tool_in_flight()) {
        Ok(())
    } else {
        Err(key!("reason-nothing-to-undo"))
    }
}

pub(super) fn can_redo(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.symbol_editor().is_none() {
        writable(m)?;
    }
    if m.can_redo() || (m.symbol_editor().is_none() && m.tool_in_flight()) {
        Ok(())
    } else {
        Err(key!("reason-nothing-to-redo"))
    }
}

pub(super) fn has_next(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    if m.page_index() + 1 < m.page_count() {
        Ok(())
    } else {
        Err(key!("reason-no-next-page"))
    }
}

pub(super) fn has_previous(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    if m.page_index() > 0 {
        Ok(())
    } else {
        Err(key!("reason-no-previous-page"))
    }
}

pub(super) fn has_recent(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.recent_count() > 0 {
        Ok(())
    } else {
        Err(key!("reason-no-recent"))
    }
}

pub(super) fn page_shown(m: &ShellModel) -> Result<(), Key<'static>> {
    // The wizard's preview page is a page on show too (`view.snapshot` writes it).
    if m.page_count() > 0 || m.wizard().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-page"))
    }
}

pub(super) fn can_zoom_in(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    match m.canvas() {
        Some(c) if c.zoom() >= crate::canvas::ZOOM_MAX => Err(key!("reason-zoom-max")),
        _ => Ok(()),
    }
}

pub(super) fn can_zoom_out(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    match m.canvas() {
        Some(c) if c.zoom() <= crate::canvas::ZOOM_MIN => Err(key!("reason-zoom-min")),
        _ => Ok(()),
    }
}

pub(super) fn can_add_tab(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.tab_count() >= crate::area::MAX_TABS => Err(key!("reason-tabs-full")),
        _ => Ok(()),
    }
}

pub(super) fn can_close_tab(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    match m.area() {
        // A station's view closes even as the last tab: its tab shows its page again.
        Some(a) if a.tab_count() <= 1 && a.current().station.is_none() => Err(key!("reason-last-tab")),
        _ => Ok(()),
    }
}

pub(super) fn can_cycle_tabs(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.focused_group().tabs.len() <= 1 => Err(key!("reason-one-tab")),
        _ => Ok(()),
    }
}

pub(super) fn two_groups(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.side_by_side() => Ok(()),
        _ => Err(key!("reason-one-group")),
    }
}

pub(super) fn can_go_back(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.current().history.back_len() > 0 => Ok(()),
        _ => Err(key!("reason-history-empty")),
    }
}

pub(super) fn can_go_forward(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.current().history.forward_len() > 0 => Ok(()),
        _ => Err(key!("reason-history-empty")),
    }
}

/// A panel-layout page of a project that can be changed is on show (the reason: "Open a panel layout page").
pub(crate) fn panel_page(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    writable(m)?;
    match m.page_rows().get(m.page_index()).map(|r| &r.kind) {
        Some(ew_api::PageType::PanelLayout) => Ok(()),
        _ => Err(key!("reason-no-panel-page")),
    }
}

/// A panel-layout page with a surface view is on show in a project that can be changed (Draw ▸ Layout is
/// disabled elsewhere with "Open a panel layout page; make one with New page…", and on a panel page that shows no
/// enclosure yet with how to show one).
pub(crate) fn panel_view(m: &ShellModel) -> Result<(), Key<'static>> {
    panel_page(m)?;
    if m.panel_tables().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-panel-view"))
    }
}

/// A placement is selected on a panel page that can be changed.
/// Add legend: a panel page that shows a surface whose layout has no legend yet, on the page or as its page (where
/// the room beside the layout does not hold it whole, the legend gets its page).
pub(super) fn legend_addable(m: &ShellModel) -> Result<(), Key<'static>> {
    panel_page(m)?;
    match m.legend_offer() {
        ew_api::output::LegendOffer::At(_) | ew_api::output::LegendOffer::NoRoom => Ok(()),
        ew_api::output::LegendOffer::Exists(_) => Err(key!("reason-legend-exists")),
        ew_api::output::LegendOffer::Paged => Err(key!("reason-legend-paged")),
        ew_api::output::LegendOffer::NoLayout => Err(key!("reason-legend-no-layout")),
    }
}

/// Fit the layout to the sheet: a panel page with a view and no legend box beside the layout, whose room the plate
/// would take (doc 12 §5.4: the box goes to its own page first).
pub(super) fn sheet_fittable(m: &ShellModel) -> Result<(), Key<'static>> {
    panel_view(m)?;
    match m.legend_offer() {
        ew_api::output::LegendOffer::Exists(_) => Err(key!("reason-layout-fit-legend")),
        _ => Ok(()),
    }
}

/// Move the legend to its own page: a panel page that holds a legend box.
pub(super) fn legend_movable(m: &ShellModel) -> Result<(), Key<'static>> {
    panel_page(m)?;
    match m.legend_offer() {
        ew_api::output::LegendOffer::Exists(_) => Ok(()),
        _ => Err(key!("reason-legend-no-box")),
    }
}

/// Hide text: one symbol selected on a writable page.
pub(super) fn one_rep_selected(m: &ShellModel) -> Result<(), Key<'static>> {
    writable_page(m)?;
    if m.selected_rep_texts().is_some() {
        Ok(())
    } else {
        Err(key!("reason-text-one-symbol"))
    }
}

/// Put the text back: a selected symbol has a text moved or turned.
pub(super) fn texts_moved(m: &ShellModel) -> Result<(), Key<'static>> {
    writable_page(m)?;
    if m.selected_reps_with_moved_texts().is_empty() {
        Err(key!("reason-texts-not-moved"))
    } else {
        Ok(())
    }
}

/// Keep in place: the selection holds a symbol of a drawn page.
pub(super) fn reps_selected(m: &ShellModel) -> Result<(), Key<'static>> {
    writable_page(m)?;
    if m.selected_rep_rows().is_empty() {
        Err(key!("reason-keep-in-place-no-symbol"))
    } else {
        Ok(())
    }
}

/// Function text over its path (doc 06 §3.8): the selection is one symbol of a device.
pub(super) fn one_device_symbol(m: &ShellModel) -> Result<(), Key<'static>> {
    writable_page(m)?;
    match m.selected_rep_rows().as_slice() {
        [one] if one.device.is_some() => Ok(()),
        _ => Err(key!("reason-one-device-symbol")),
    }
}

/// Name the strip here (doc 06 §3.8): the selection is one terminal's symbol.
pub(super) fn one_terminal_symbol(m: &ShellModel) -> Result<(), Key<'static>> {
    writable_page(m)?;
    match m.selected_rep_rows().as_slice() {
        [one] if one.terminal => Ok(()),
        _ => Err(key!("reason-one-terminal-symbol")),
    }
}

/// Reset label position: the selected placement's label was moved.
pub(super) fn label_moved(m: &ShellModel) -> Result<(), Key<'static>> {
    panel_selection(m)?;
    if m.selected_label_moved() {
        Ok(())
    } else {
        Err(key!("reason-label-not-moved"))
    }
}

/// Draw ▸ Layout ▸ Plate: a panel layout page is on show and the project has a mounting plate to show.
pub(super) fn plate_choosable(m: &ShellModel) -> Result<(), Key<'static>> {
    panel_page(m)?;
    if m.has_plates() {
        Ok(())
    } else {
        Err(key!("reason-no-plate"))
    }
}

/// Reset label positions: a label of the panel page shown was moved.
pub(super) fn labels_moved(m: &ShellModel) -> Result<(), Key<'static>> {
    panel_page(m)?;
    if m.moved_labels().is_empty() {
        Err(key!("reason-labels-not-moved"))
    } else {
        Ok(())
    }
}

pub(super) fn panel_selection(m: &ShellModel) -> Result<(), Key<'static>> {
    panel_page(m)?;
    if m.selected_placements() == 0 {
        Err(key!("reason-no-placement"))
    } else {
        Ok(())
    }
}

/// Why the page on show takes no symbols or wires, by its type, in the words of its kind where one key exists for it
///  and the generic text otherwise; `None` for a page that takes them. One mapping for the refusal of the
/// editing tools and the canvas HUD's prompt.
pub(crate) fn page_type_reason(m: &ShellModel) -> Option<Key<'static>> {
    match m.page_rows().get(m.page_index()).map(|r| &r.kind) {
        Some(ew_api::PageType::SectionDivider) => Some(key!("reason-page-type-no-symbols-divider")),
        Some(ew_api::PageType::TitlePage) => Some(key!("reason-page-type-no-symbols-title")),
        Some(ew_api::PageType::TableOfContents) => Some(key!("reason-page-type-no-symbols-contents")),
        Some(ew_api::PageType::Report) => Some(key!("reason-page-type-no-symbols-report")),
        Some(ew_api::PageType::PanelLayout) => Some(key!("reason-page-type-no-symbols-panel")),
        Some(ew_api::PageType::ModelView | ew_api::PageType::ExternalDocument) => {
            Some(key!("reason-page-type-no-symbols"))
        }
        _ => None,
    }
}

/// A drawn page of a project that can be changed is on show (the editing tools).
pub(crate) fn editable_page(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    writable(m)?;
    // The page types without wire or symbol tools (doc 06 §2.2: title page, panel layout, model view, external
    // document; the generated table of contents, dividers and reports are not drawn pages at all).
    // The refusal names the page's kind in words where one key exists for it, the generic text otherwise.
    if let Some(reason) = page_type_reason(m) {
        return Err(reason);
    }
    match m.canvas() {
        Some(c) if !c.page().tables.page.is_nil() => Ok(()),
        _ => Err(key!("reason-not-editable")),
    }
}

/// A drawn page of a writable project for Paste (window check, item 3), of a kind that takes a paste: a kind that takes
/// none names itself in the reason (a generated report has no canvas at all, and says so rather than the general one).
pub(super) fn paste_page(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    writable(m)?;
    if let Some(reason) = m.paste_page_refusal() {
        return Err(reason);
    }
    match m.canvas() {
        Some(c) if !c.page().tables.page.is_nil() => Ok(()),
        _ => Err(key!("reason-not-editable")),
    }
}

/// An editable page whose type has a connection kind the Wire tool draws.
pub(super) fn wire_page(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    m.wire_kind().map(|_| ())
}

/// An editable bus-topology page that does not draw a station or bus device of the project yet (Put a station on this
/// page…).
pub(super) fn put_station_page(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if m.page_rows().get(m.page_index()).map(|r| &r.kind) != Some(&ew_api::PageType::BusTopology) {
        return Err(key!("reason-put-station-bus-page"));
    }
    if m.stations_off_page(None).is_empty() {
        return Err(key!("reason-no-station-off-page"));
    }
    Ok(())
}

/// An editable fluid page (Draw ▸ Fluid line…); elsewhere (another page, a station's view) the reason names the
/// fluid page (in a station's view the Tube's hover spoke of symbols on drawn pages).
pub(super) fn fluid_page(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    match m.wire_kind() {
        Ok(Some("fluid")) => editable_page(m),
        _ => Err(key!("reason-not-fluid-page")),
    }
}

/// An editable page with two or more terminals of one row selected (Spacing…).
pub(super) fn terminal_row(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if m.terminal_row_selected() {
        Ok(())
    } else {
        Err(key!("reason-terminals-spacing"))
    }
}

/// An editable page whose selection gives a bond its two ends: exactly one selected connection, a wire and no tube
/// (Draw ▸ Bond). The headless form that names the two ends (`a` and `b`) needs only the editable page
/// ([`enabled_with`]).
pub(super) fn bond_ends(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if !m.one_connection_selected() {
        Err(key!("reason-bond-selection"))
    } else if m.fluid_line_selected() {
        Err(key!("reason-bond-fluid"))
    } else {
        Ok(())
    }
}

/// An editable page whose selection names one point a potential is defined at (Draw ▸ Define potential…).
pub(super) fn potential_point(m: &ShellModel) -> Result<(), Key<'static>> {
    // A Potentials row's form defines at the row's point, whatever page is on show.
    if m.potential_pinned() {
        return writable(m);
    }
    editable_page(m)?;
    m.potential_point().map(|_| ())
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
) -> Result<(), Key<'static>> {
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
            return Err(key!("symbol-editor-active"));
        }
    }
    match meta.enabled_when_named {
        Some((params, enabled)) if params.iter().any(|k| p.contains_key(*k)) => enabled(m),
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
    pub fn enabled_now(&self, m: &ShellModel) -> Result<(), Key<'static>> {
        enabled_with(self, m, &serde_json::Map::new())
    }
}

/// An editable page with exactly one junction selected (Edit ▸ Junction sequence…; window check).
pub(super) fn one_junction(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if m.one_junction_selected() {
        Ok(())
    } else {
        Err(key!("reason-one-junction"))
    }
}

/// An editable page with exactly one interruption point selected (Edit ▸ Continue on page…).
pub(super) fn one_ip(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if m.selected_ip_count() == 1 {
        Ok(())
    } else {
        Err(key!("reason-select-one-ip"))
    }
}

/// A writable project with interruption points selected (Supply comes in here…): without them the command search
/// says how to make them ("supply" found it and nothing said what it needs).
pub(super) fn ips_selected(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.selected_ip_count() >= 1 {
        Ok(())
    } else {
        Err(key!("reason-supply-in-select"))
    }
}

/// An editable page with at least two interruption points selected (Edit ▸ Arrange interruption points).
pub(super) fn two_ips(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if m.selected_ip_count() >= 2 {
        Ok(())
    } else {
        Err(key!("reason-select-ips"))
    }
}

/// An editable page with exactly one black box selected (Edit ▸ Add connection point, the grips).
pub(super) fn one_box(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if m.one_black_box_selected() {
        Ok(())
    } else {
        Err(key!("reason-one-black-box"))
    }
}

/// An editable page with one box of a PLC card in the older form selected (Draw the card in one row).
pub(super) fn older_card(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    m.older_card().map(|_| ())
}

/// An editable page with one box of a PLC card selected (Terminal wires to card side).
pub(super) fn card_box(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    m.card_box(key!("reason-not-card-box")).map(|_| ())
}

/// An editable page with exactly one unbound free text selected (Edit ▸ Edit text).
pub(super) fn one_free_text(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    m.text_edit_target().map(|_| ())
}

/// An editable page whose selection is two or more terminals of one strip not all bridged yet (Edit ▸ Bridge
/// terminals).
pub(super) fn two_strip_terminals(m: &ShellModel) -> Result<(), Key<'static>> {
    // A writable project, not an editable page (the selection may be records).
    writable(m)?;
    m.bridge_command().map(|_| ())
}

/// The strip editor's gestures: a writable project with the Terminal strips navigator open and focused
/// (headless: open).
pub(super) fn strips_editor(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    m.strip_editor_shown()
}

/// Swap cores: a writable project with the Cables navigator open and focused (headless: open).
pub(super) fn cables_editor(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    m.cables_shown()
}

/// The junction sequence editor is open on a project that can be changed.
pub(super) fn junction_editor_open(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.junction_editor().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-junction-editor"))
    }
}

/// A project with a page is open: `page.describe` reads a page.
pub(super) fn page_describable(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if m.page_count() > 0 {
        Ok(())
    } else {
        Err(key!("reason-no-page"))
    }
}

/// The Place tool is active and its symbol can be an auxiliary function, or the Select tool holds one placed symbol of
/// a device (J offers the Join chooser).
pub(super) fn place_joinable(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if m.place_joinable() || m.join_rep().is_some() {
        Ok(())
    } else if m.tool().name() == "place" {
        Err(key!("reason-join-not-auxiliary"))
    } else {
        Err(key!("reason-join-select"))
    }
}

/// The Join chooser is open.
pub(super) fn chooser_open(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.chooser().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-chooser"))
    }
}

/// The parts import's column mapping step is open on a writable project.
pub(super) fn parts_map_open(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.parts_map().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-parts-map"))
    }
}

/// The picker is open.
pub(super) fn picker_open(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.picker().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-picker"))
    }
}

pub(super) fn has_link_preview(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    if m.link_preview().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-preview"))
    }
}

pub(super) fn can_discard_journal(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.can_discard_journal() {
        Ok(())
    } else {
        Err(key!("reason-no-journal"))
    }
}

/// Update from library needs the damaged-copy banner that offers it.
pub(super) fn damaged_update_offered(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.damaged_update_offered() {
        Ok(())
    } else {
        Err(key!("reason-no-damaged-copy-update"))
    }
}

/// The recovery dialog's answers need the dialog.
pub(super) fn recovery_open(m: &ShellModel) -> Result<(), Key<'static>> {
    if matches!(m.dialog(), Some(crate::shell::Dialog::Recovery { .. })) {
        Ok(())
    } else {
        Err(key!("reason-no-recovery"))
    }
}

pub(super) fn wizard_open(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.wizard().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-wizard"))
    }
}

pub(super) fn first_run_open(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.first_run().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-first-run"))
    }
}

/// The property grid's actions need a project; the grid's own controls also need the pane.
pub(super) fn grid_shown(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if m.properties_shown() {
        Ok(())
    } else {
        Err(key!("reason-no-sheet"))
    }
}

/// A grid action that may commit: a writable project with the pane shown.
pub(super) fn grid_writable(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    grid_shown(m)
}

/// The find pane's own controls need the pane.
pub(super) fn find_shown(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if m.find_shown() {
        Ok(())
    } else {
        Err(key!("reason-no-find"))
    }
}

/// A replace: a writable project with the find pane shown.
pub(super) fn find_writable(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    find_shown(m)
}

/// The card's buttons: the card sits in the property grid or the find pane, so one of them is shown; an import card
///  is a window of its own and needs neither.
pub(super) fn card_shown(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    // The Preview tab shows every card: a table card is in its navigator.
    if m.properties_shown()
        || m.find_shown()
        || m.import_card_open()
        || m.preview_shown()
        || m.table_gesture().is_some()
    {
        Ok(())
    } else {
        Err(key!("reason-no-sheet"))
    }
}

/// Apply on the card: a writable project with the property grid or the find pane shown, or an import card open.
pub(super) fn card_writable(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    card_shown(m)
}

/// `navigator.apply`: a writable project with the Pages pane's reorder card, the Messages pane's
/// quick-fix card or a navigator's table card on show.
pub(super) fn reorder_card(m: &ShellModel) -> Result<(), Key<'static>> {
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
        Err(key!("reason-no-card"))
    }
}

/// `table.resolve` and `table.drop`: a writable project with a table card on show.
pub(super) fn table_card(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.table_gesture().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-card"))
    }
}

/// View ▸ Clear highlights: a highlight is shown.
pub(super) fn has_highlights(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if m.highlights().is_empty() {
        Err(key!("reason-no-highlights"))
    } else {
        Ok(())
    }
}

/// The page reorder actions: a writable project with pages.
pub(super) fn pages_writable(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    has_pages(m)
}

/// Copy and Copy tag list: a page is shown and something is selected (a read-only project copies too).
pub(super) fn selection_shown(m: &ShellModel) -> Result<(), Key<'static>> {
    page_shown(m)?;
    if m.selection().is_empty() {
        Err(key!("reason-nothing-selected"))
    } else {
        Ok(())
    }
}

/// Edit ▸ Delete (M39): an editable page with something selected; it deletes the canvas selection only (a navigator's
/// rows have their own Delete, `table.clear`), so with nothing selected it is disabled for that reason, as Cut is.
pub(super) fn editable_selection(m: &ShellModel) -> Result<(), Key<'static>> {
    // In a station's view Delete takes out what stands under the cursor: a module, a valve.
    if m.station_tab().is_some() {
        writable(m)?;
        return m.station_taken().map(|_| ());
    }
    // Delete on a panel page deletes the selected placements and carriers.
    if panel_page(m).is_ok() {
        return if m.selection().is_empty() {
            Err(key!("reason-nothing-selected"))
        } else {
            Ok(())
        };
    }
    editable_page(m)?;
    // A selection Delete has nothing of on this page (a page chosen in the page list) is not
    // offered here.
    if m.selection().is_empty() || !m.delete_has_target() {
        Err(key!("reason-nothing-selected"))
    } else {
        Ok(())
    }
}

/// Save as macro…: a page on show with something selected (a window macro) or pages
/// marked in the Pages navigator (a page macro).
pub(super) fn macro_scope_shown(m: &ShellModel) -> Result<(), Key<'static>> {
    page_shown(m)?;
    if m.selection().is_empty() && m.navigator().marked().is_empty() {
        Err(key!("reason-nothing-selected"))
    } else {
        Ok(())
    }
}

/// Cut and Duplicate: an editable page with something selected.
pub(super) fn selection_editable(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    selection_shown(m)
}

/// An editable page whose selection is shown and holds no tube: a cable's cores are wires (Cable…).
pub(super) fn cable_wires(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    // Nothing selected says what to select.
    match selection_shown(m) {
        Err(k) if k == key!("reason-nothing-selected") => return Err(key!("reason-cable-select-wires")),
        other => other?,
    }
    if m.fluid_line_selected() {
        Err(key!("reason-cable-fluid"))
    } else {
        Ok(())
    }
}

/// The keymap editor's controls need the keyboard shortcuts dialog.
pub(super) fn keymap_editor_open(m: &ShellModel) -> Result<(), Key<'static>> {
    if matches!(m.dialog(), Some(crate::shell::Dialog::Shortcuts)) {
        Ok(())
    } else {
        Err(key!("reason-no-keymap-editor"))
    }
}

/// The paste choice card's choices need the card and a writable project.
pub(super) fn paste_choice_open(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if matches!(
        m.properties().card().map(|c| &c.origin),
        Some(crate::properties::CardOrigin::PasteVersions(_))
    ) {
        Ok(())
    } else {
        Err(key!("reason-no-paste-choice"))
    }
}

/// The print pre-dialog's own steps.
pub(super) fn print_dialog_open(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if m.print_dialog().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-print-dialog"))
    }
}

/// The part selection dialog's own actions need the dialog.
pub(super) fn part_dialog_open(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if matches!(m.dialog(), Some(crate::shell::Dialog::PartChoose(_))) {
        Ok(())
    } else {
        Err(key!("reason-no-part-dialog"))
    }
}

/// The part editor's own actions need the editor and a writable project.
pub(super) fn part_editor_open(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if matches!(m.dialog(), Some(crate::shell::Dialog::PartEdit(_))) {
        Ok(())
    } else {
        Err(key!("reason-no-part-editor"))
    }
}
