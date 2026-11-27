// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The window's shares of the screen, beside the size tokens: how far a surface grows past its size token as the
//! window grows (doc 16a §2: the UI grows with the space it has), and the bounds of a pane's share. A share is the
//! same in every theme and density, so it is no token of the dump.

/// The share of the window a form dialog takes once that is wider than `size.dialog.form`
/// (the UI grows with the space it has).
pub(crate) const FORM_SHARE: f32 = 0.45;

/// The share of the window a small dialog that holds a list (the Show changes popup, the parts card) takes once that
/// is wider than `size.dialog.small`.
pub(crate) const SMALL_SHARE: f32 = 0.3;

/// The share of the window the print pre-dialog takes once that is wider than `size.dialog.preview` (1248 px at 1920).
pub(crate) const PREVIEW_SHARE: f32 = 0.65;

/// The share of the window's width a toast takes once that is wider than `size.toast` (doc 16a §2: the UI
/// grows with the space it has).
pub(crate) const TOAST_SHARE: f32 = 0.22;

// The share of the tab row the command search takes once that is wider than `size.search.width`.
pub(crate) const SEARCH_SHARE: f32 = 0.2;

/// The share of the window the command palette and the place picker take once that is wider than `size.palette`
/// (the UI grows with the space it has, never a width chosen for the floor).
pub(crate) const PALETTE_SHARE: f32 = 0.4;

/// The share of the window's height the result list takes once that is more than ten rows: ten rows at the
/// 800-px floor, more in a taller window.
pub(crate) const PART_LIST_SHARE: f32 = 0.35;

/// The share of the window's height the list may take before it scrolls (doc 16a §2: a list dialog fits its content
/// up to a share of the window, so it grows at 1920 × 1080).
pub(crate) const KEYMAP_LIST_SHARE: f32 = 0.5;

/// The share of a station view's pane the catalogue takes at its right, within [`CATALOGUE_MIN`] and
/// [`CATALOGUE_MAX`] and never more than [`CATALOGUE_MOST_SHARE`].
pub(crate) const CATALOGUE_SHARE: f32 = 0.26;

/// The catalogue's narrowest width.
pub(crate) const CATALOGUE_MIN: f32 = 280.0;

/// The catalogue's widest width.
pub(crate) const CATALOGUE_MAX: f32 = 460.0;

/// The most of a station view's pane the catalogue takes.
pub(crate) const CATALOGUE_MOST_SHARE: f32 = 0.5;

/// The share of the window's height a form's choice list opens to at most (every choice in view).
pub(crate) const FORM_LIST_HEIGHT_SHARE: f32 = 0.75;

/// The share of the window's height the list of a list dialog (the automation list, the templates) takes before it
/// scrolls.
pub(crate) const LIST_HEIGHT_SHARE: f32 = 0.45;

/// The share of the window's height the text styles' table takes (at 2560 × 1440 it shows more rows).
pub(crate) const TEXT_STYLES_HEIGHT_SHARE: f32 = 0.62;

/// The share of a text style's row its name column takes, at least `size.dialog.label`.
pub(crate) const TEXT_STYLE_NAME_SHARE: f32 = 0.34;

/// The share of the place picker's width its tree takes, within the left panel's bounds.
pub(crate) const PICKER_TREE_SHARE: f32 = 0.16;

/// The share of the place picker's width its detail takes, within the left panel's bounds.
pub(crate) const PICKER_DETAIL_SHARE: f32 = 0.2;

/// The share of the print dialog's room its progress line takes.
pub(crate) const PRINT_PROGRESS_SHARE: f32 = 0.6;

/// The share of the symbol editor's room a side panel takes at most; the drawing takes the rest.
pub(crate) const SYMBOL_PANEL_SHARE: f32 = 0.3;

/// The share of a part card its label column takes; the value stands beside it.
pub(crate) const PART_CARD_LABEL_SHARE: f32 = 0.45;

/// The share of the first-run screen's height above its column.
pub(crate) const FIRST_RUN_TOP_SHARE: f32 = 0.1;

/// The share of the start page's height above its title, at most 60 points.
pub(crate) const START_TOP_SHARE: f32 = 0.08;

/// The left column's width at the main resolution (doc 16a §2: 1920 × 1080 at 100 %) while it has its default share.
pub(crate) const LEFT_AT_MAIN: f32 = 480.0;

/// The share of every point a window is wider or narrower than the main resolution that the left column gains or
/// gives up while it has its default share: `size.panel.left_max` (640) from about 2380.
pub(crate) const LEFT_GROW: f32 = 0.35;

/// The most of the window's height the bottom dock grows to for its content while it keeps its
/// default share.
pub(crate) const BOTTOM_GROW: f32 = 0.4;

/// The most a row's keys column takes of the columns' room (it fits its widest chips below that).
pub(crate) const KEYS_SHARE: f32 = 0.25;

/// The most a row's "where" column takes of the columns' room (it fits its widest text below that).
pub(crate) const WHERE_SHARE: f32 = 0.35;

/// The share of the dialog's body the options column takes; the preview takes the rest.
pub(crate) const OPTIONS_SHARE: f32 = 0.36;

/// The preview's height at most, as a share of the window's height.
pub(crate) const PREVIEW_HEIGHT_SHARE: f32 = 0.56;

/// The share of the table's width each text column takes at least (EW term, EPLAN, QET; the definition takes the rest
/// after the actions, whose column is as wide as its widest button, so a button never wraps).
pub const TEXT_SHARE: f32 = 0.15;

/// The share a text column takes at most when its longest word needs more than [`crate::theme::shares::TEXT_SHARE`].
pub const TEXT_SHARE_MAX: f32 = 0.25;
