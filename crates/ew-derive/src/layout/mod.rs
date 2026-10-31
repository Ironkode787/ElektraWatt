// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The layout's pure geometry (WP-S4.01 D5, doc 12 §2.1–§2.4, §3.3): carrier paths, the mount-kind compatibility
//! table and the pose validator, shared by the `layout.*` commands (`ew-commands`) and part 5's EW-LAY-010.
//!
//! Everything here is a plain function over record values, in integer nanometres and exact: carriers are
//! axis-parallel at MVP, so every length is a sum of coordinate differences (i128, no square root, no float), and a
//! rotation is a whole number of quarter turns about the surface normal `w`.
//!
//! | Item | What it answers |
//! |---|---|
//! | [`check_path`], [`path_length`], [`RailAxis`] | Whether a carrier path is axis-parallel (D5), its length, a rail's axis |
//! | [`Profile`], [`RAIL_FITS`], [`fits_rail`], [`fits_surface`] | The mount-kind compatibility table (doc 12 §3.3) |
//! | [`Quarter`], [`check_pose`], [`PoseOn`] | The degrees of freedom of a pose per carrier kind (doc 12 §2.3) |
//! | [`clip_offset`], [`rehome_pose`] | The surface pose equivalent to a rail pose (doc 12 §3.6, `rehome_to_surface`) |
//! | [`plate_frame`] | The frame and usable rectangle of a plate centred in its enclosure (D5) |
//! | [`Mountable`] | `mountable(dev)`: the mountable predicate as built (D8, doc 12 §2.7) |
//! | [`Unplaced`] | `unplaced(space)`: the mountable devices of the space's location without a placement |
//! | [`PlacementGeometry`] | `placement_geometry(p)`: the footprint and outline in the surface frame (D5) |
//! | [`RailOccupancy`] | `rail_occupancy(carrier)`: the sorted intervals along a rail |
//! | [`LayoutLabel`] | `layout_label((p, page, mode))`: the label text (D10) |
//! | [`CarrierGeometry`] | `carrier_geometry(carrier)`: the axis check, length, axis and outline of a carrier |
//! | [`layout_legend`], [`LegendRow`] | `layout_legend(surface)`: the legend rows of a surface (D11, doc 12 §5.4) |
//! | [`page_surface`], [`surface_pages`] | The surface a panel-layout page shows, and the pages that show a surface (D9) |
//! | [`carrier_order`], [`metres_of_nm`] | The legend order of carriers and a length in metres exact to the millimetre (D11) |
//!
//! Frames (doc 12 §2.1): a surface's `u` grows to the right and `v` upwards; a rail's pose `t = (s, 0, 0)` is the part's
//! clip point at distance `s` from `path[0]` along the axis; a surface pose `t = (u, v, 0)` is the part origin, the
//! bottom-left corner of its footprint (x along the rail, y up the plate); a part's `rail_offset` is the clip point
//! in the part frame (absent: the footprint's centre).

mod carrier;
mod fit;
// WP-S4.01 part 4: the layout legend and the surface of a panel page (D11)
mod legend;
mod pose;
// WP-S4.01 part 2: the layout queries (doc 12 §2.6)
mod queries;

pub use carrier::{PathError, RailAxis, check_path, path_length};
pub use fit::{Fit, Profile, RAIL_FITS, fits_rail, fits_surface};
pub use legend::{
    CarrierOrder, LegendItem, LegendRow, carrier_length, carrier_order, carriers_in_order, layout_legend,
    metres_of_nm, page_surface, surface_pages,
};
pub use pose::{PoseError, PoseOn, Quarter, check_pose, clip_offset, mm, plate_frame, rehome_pose};
pub use queries::{
    CarrierGeom, CarrierGeometry, Exempt, LabelMode, LayoutLabel, MountAs, Mountability, Mountable, Occupied,
    PlacementGeom, PlacementGeometry, RailOccupancy, UNKNOWN_SIDE, Unplaced, primary_part, space_of_location,
};

#[cfg(test)]
mod tests;
