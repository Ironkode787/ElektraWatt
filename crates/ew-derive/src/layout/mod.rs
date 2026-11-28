// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The layout's pure geometry (doc 12 §2.1–§2.4, §3.3): carrier paths, the mount-kind compatibility
//! table and the pose validator, shared by the `layout.*` commands (`ew-commands`) and part 5's EW-LAY-010.
//!
//! Everything here is a plain function over record values, in integer nanometres and exact: carriers are
//! axis-parallel at MVP, so every length is a sum of coordinate differences (i128, no square root, no float), and a
//! rotation is a whole number of quarter turns about the surface normal `w`.
//!
//! | Item | What it answers |
//! |---|---|
//! | [`check_path`], [`path_length`], [`RailAxis`] | Whether a carrier path is axis-parallel, its length, a rail's axis |
//! | [`Profile`], [`RAIL_FITS`], [`fits_rail`], [`fits_surface`] | The mount-kind compatibility table (doc 12 §3.3) |
//! | [`Quarter`], [`check_pose`], [`PoseOn`] | The degrees of freedom of a pose per carrier kind (doc 12 §2.3) |
//! | [`clip_offset`], [`rehome_pose`] | The surface pose equivalent to a rail pose (doc 12 §3.6, `rehome_to_surface`) |
//! | [`plate_frame`] | The frame and usable rectangle of a plate centred in its enclosure |
//! | [`surfaces`], [`Face`], [`surface_frame`], [`surface_datum`] | The plate and the six faces of a cabinet, the frame and sheet of a face made from its part, the corner a surface is measured from (doc 12 §2.8) |
//! | [`Mountable`] | `mountable(dev)`: the mountable predicate as built (D8, doc 12 §2.7) |
//! | [`Unplaced`] | `unplaced(space)`: the mountable devices of the space's location without a placement |
//! | [`PlacementGeometry`] | `placement_geometry(p)`: the footprint and outline in the surface frame |
//! | [`RailOccupancy`] | `rail_occupancy(carrier)`: the sorted intervals along a rail |
//! | [`StripLayout`] | `strip_layout(strip)`: a strip's terminals and the accessories standing in it, in mounting order |
//! | [`LayoutLabel`] | `layout_label((p, page, mode))`: the label text (D10) |
//! | [`CarrierGeometry`] | `carrier_geometry(carrier)`: the axis check, length, axis and outline of a carrier |
//! | [`layout_legend`], [`LegendRow`] | `layout_legend(surface)`: the legend rows of a surface (doc 12 §5.4) |
//! | [`page_surface`], [`surface_pages`] | The surface a panel-layout page shows, and the pages that show a surface |
//! | [`carrier_order`], [`metres_of_nm`] | The legend order of carriers and a length in metres exact to the millimetre |
//! | [`carrier_labels`], [`CarrierLabel`] | "Rail n" and "Duct n" of a surface's carriers |
//! | [`RailItem`], [`min_gap`], [`runs`], [`brackets`], [`free_length`] | The rail model: items, minimum gaps, runs of strips and their end brackets |
//! | [`resolve`], [`SnapMode`] | Where a dropped item lands in free, push or swap mode and which neighbours move |
//! | [`rail_items`], [`rail_extent`] | A rail's items from `rail_occupancy`; what a device takes along a rail |
//! | [`GroupAccessories`], [`RailBrackets`] | `group_accessories(carrier)`: the derived end brackets of a rail and their count per strip (FR-LAY-060) |
//! | [`member_order`], [`member_slot`] | The member order of an assembly, shared with EW-LAY-016, and where a member is pushed in |
//! | [`next_fit`] | Where a fill places an item: the first fit at or after a floor |
//! | [`PinSpots`] | `pin_spots(p)`: where a placed device's pins stand, their way out and height (doc 12 §8.1) |
//! | [`DuctGraph`], [`plan_routes`], [`measure`], [`route_status`] | The ducts as a graph, Route's pass, a stored way's run and status (doc 12 §8) |
//! | [`DuctFill`], [`WIRE_OD`], [`INNER_SHARE`] | `duct_fill(duct)`: the fill of a duct's pieces (doc 12 §8.6) |
//! | [`reserve`], [`fill_limit`], [`door_loop`], [`routed_length`] | The shop's numbers of doc 12 §9 and a routed wire's length |
//!
//! Frames (doc 12 §2.1): a surface's `u` grows to the right and `v` upwards; a rail's pose `t = (s, 0, 0)` is the part's
//! clip point at distance `s` from `path[0]` along the axis; a surface pose `t = (u, v, 0)` is the part origin, the
//! bottom-left corner of its footprint (x along the rail, y up the plate); a part's `rail_offset` is the clip point
//! in the part frame (absent: the footprint's centre).

// The rail items and the derived end brackets
mod accessories;
// The member order of an assembly
mod assembly;
mod carrier;
// How full a duct is (doc 12 §8.6)
mod fill;
mod fit;
// The cable glands of a cabinet (doc 12 §3.9)
mod glands;
// The holes of a surface (doc 12-3d §10)
mod holes;
// The layout legend and the surface of a panel page
mod legend;
// Where a placed device's pins stand (doc 12 §8.1)
mod pins;
mod pose;
// Where a device stands: per rail, per surface, per device (doc 13 §6.1)
mod mounting;
// The layout queries (doc 12 §2.6)
mod queries;
// The surfaces of a cabinet (doc 12 §2.8)
mod surfaces;
// The rail model and the mode resolution
mod snap;
// The template for drilling of a surface (doc 13 §9.1)
mod template;
// The ways of a cabinet (doc 12 §8, §9)
mod ways;

pub use accessories::{Bracket, GroupAccessories, RailBrackets, rail_items};
pub use assembly::{member_order, member_slot};
pub use carrier::{PathError, RailAxis, check_path, path_length};
pub use fill::{
    DuctFill, DuctFillV, FillPiece, INNER_SHARE, InnerFrom, WIRE_OD, inner_area, wire_area, wire_od,
};
pub use fit::{Fit, Profile, RAIL_FITS, fits_rail, fits_surface};
pub use glands::{
    CableGland, CableGlands, NoGland, cable_outer_diameter, cable_part_use, gland_nut, gland_pitch,
    gland_row, gland_section, gland_snap, hole_nut,
};
pub use holes::{
    Feature, HOLE_SIZES, HoleSizes, Made, Owner, SizeFamily, SurfaceFeatures, carrier_fixings, choice_for,
    chosen, duct_fixing_default, fixings, hole_size, lost_choices, made, part_holes, placement_holes,
    rail_fixing_default,
};
pub use legend::{
    CarrierLabel, CarrierLabelKind, CarrierOrder, LegendItem, LegendRow, cabinet_name, carrier_label,
    carrier_labels, carrier_length, carrier_order, carriers_in_order, layout_legend, legend_layout_page,
    location_text, metres_of_nm, page_surface, surface_pages,
};
pub(crate) use mounting::from_rail_start;
pub use mounting::{
    DevicePlace, Mounted, OnSurface, RailAssembly, RailStart, device_places, rail_assembly, surface_items,
};
pub use pins::{PinSpot, PinSpots, PinSpotsV, drawn_face, in_door, pin_face, placement_of_function, reach};
pub use pose::{PoseError, PoseOn, Quarter, check_pose, clip_offset, plate_frame, rehome_pose};
pub use queries::{
    CarrierGeom, CarrierGeometry, Exempt, LabelMode, LayoutLabel, MountAs, Mountability, Mountable, Occupied,
    PlacementGeom, PlacementGeometry, RailExtent, RailOccupancy, StripItem, StripItemKind, StripLayout,
    UNKNOWN_SIDE, Unplaced, fill_candidates, is_earth_connection, is_earth_point, places_strip, primary_part,
    rail_extent, space_of_location, unlocated, unlocated_drawn,
};
pub(crate) use queries::{shown_location, strip_terminals};
pub use snap::{
    BracketRole, BracketSpan, RailItem, Resolution, SnapError, SnapMode, brackets, cascade, cascade_mirrored,
    end_gap, free_length, min_gap, next_fit, resolve, runs, start_gap,
};
pub use surfaces::{
    Face, behind_door, door_count, door_hinge, door_index, door_left, enclosure_section, face_of, face_size,
    from_datum, sheet_size, surface_datum, surface_frame, surface_page, surfaces, to_enclosure,
};
pub use template::{
    Paper, TEMPLATE_BAND, TEMPLATE_CROSS, TEMPLATE_MARGIN, TEMPLATE_OVERLAP, TEMPLATE_PAST, TemplateSheets,
    TemplateTile, drawable, hole_boxes, template_sheets,
};
pub use ways::{
    CabinetWire, DOOR_LOOP, DuctGraph, DuctGraphV, DuctLine, EndPlace, Entry, FILL_LIMIT, Joint, LaidSpaces,
    NoWay, Planned, PlannedWire, RESERVE, RoutePlan, RouteScope, RouteStatus, StaleWhy, Way, cabinet_page,
    cabinet_wire, cabinet_wires, door_loop, end_place, entries, fill_limit, mark_ends, measure, plan_routes,
    plate_of_space, reserve, route_status, routed_length, space_of_wire, stored_way, way_points, way_surface,
};

#[cfg(test)]
mod tests;
