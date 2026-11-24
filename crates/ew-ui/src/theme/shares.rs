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

// The share of the tab row the command search takes once that is wider than `size.search.width` (FT-798).
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

/// The share of the window's height the part editor's form takes once that is more than sixteen rows (doc 16a §2).
pub(crate) const PART_FORM_SHARE: f32 = 0.6;

/// The share of the symbol editor's canvas the start card takes once that is wider than `size.dialog.form`: a third
/// of a wide window, so the lists of parts and symbols use the room of 2560 points.
pub(crate) const SYMBOL_CARD_SHARE: f32 = 1.0 / 3.0;

/// The share of a station view's pane the catalogue takes at its right, within [`CATALOGUE_MIN`] and
/// [`CATALOGUE_MAX`] and never more than [`CATALOGUE_MOST_SHARE`].
pub(crate) const CATALOGUE_SHARE: f32 = 0.26;

/// The catalogue's narrowest width.
pub(crate) const CATALOGUE_MIN: f32 = 280.0;

/// The catalogue's widest width.
pub(crate) const CATALOGUE_MAX: f32 = 460.0;

/// The most of a station view's pane the catalogue takes.
pub(crate) const CATALOGUE_MOST_SHARE: f32 = 0.5;
