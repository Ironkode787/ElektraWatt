// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The mount-kind compatibility table (doc 12 §3.3, WP-S4.01 D6): which rail profiles a part's mount kind fits.

use ew_schema::MountKind;

/// A rail profile (doc 12 §2.4: `profile: Ts35x7_5 | Ts35x15 | Ts32 | Ts15`), the rail kinds of [`MountKind`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Profile {
    /// Top-hat rail 35 × 7.5 mm.
    Ts35x7_5,
    /// Top-hat rail 35 × 15 mm.
    Ts35x15,
    /// G rail 32 mm.
    Ts32,
    /// Miniature top-hat rail 15 mm.
    Ts15,
}

impl Profile {
    /// The rail profile a mount kind names, or `None` for a kind that is not a rail profile (screw, busbar
    /// adapter, door cut-out, on parent, none, a newer value).
    pub fn of(kind: &MountKind) -> Option<Profile> {
        match kind {
            MountKind::Ts35x7_5 => Some(Profile::Ts35x7_5),
            MountKind::Ts35x15 => Some(Profile::Ts35x15),
            MountKind::Ts32 => Some(Profile::Ts32),
            MountKind::Ts15 => Some(Profile::Ts15),
            _ => None,
        }
    }

    /// The profile's stored name (the serde name of its [`MountKind`]).
    pub fn name(self) -> &'static str {
        match self {
            Profile::Ts35x7_5 => "ts35x7_5",
            Profile::Ts35x15 => "ts35x15",
            Profile::Ts32 => "ts32",
            Profile::Ts15 => "ts15",
        }
    }
}

/// The compatibility table (doc 12 §3.3, data): a device whose part mounts on the first profile fits a rail of each
/// profile of the second. TS 35 devices fit TS 35 × 7.5 and TS 35 × 15; TS 32 devices only TS 32 ("-102F1 (TS 35)
/// does not fit a TS 32 rail"); TS 15 devices only TS 15. Every other mount kind (screw, busbar adapter, door
/// cut-out, on parent, none) fits no rail.
pub const RAIL_FITS: [(Profile, &[Profile]); 4] = [
    (Profile::Ts35x7_5, &[Profile::Ts35x7_5, Profile::Ts35x15]),
    (Profile::Ts35x15, &[Profile::Ts35x7_5, Profile::Ts35x15]),
    (Profile::Ts32, &[Profile::Ts32]),
    (Profile::Ts15, &[Profile::Ts15]),
];

/// Whether a part fits a carrier.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fit {
    /// The table says it fits.
    Fits,
    /// The table says it does not fit.
    DoesNotFit,
    /// The part has no mount kind, or a mount kind or profile of a newer format: nothing to judge (an unknown
    /// footprint, D5).
    Unknown,
}

/// Whether a part of mount kind `mount` (`None`: the part has no `mech.mount_kind`) fits a rail of `profile`.
pub fn fits_rail(mount: Option<&MountKind>, profile: &MountKind) -> Fit {
    let (Some(mount), Some(rail)) = (mount, Profile::of(profile)) else {
        return Fit::Unknown;
    };
    if matches!(mount, MountKind::Unknown(_)) {
        return Fit::Unknown;
    }
    let fits = Profile::of(mount).is_some_and(|m| {
        RAIL_FITS
            .iter()
            .any(|(device, rails)| *device == m && rails.contains(&rail))
    });
    if fits { Fit::Fits } else { Fit::DoesNotFit }
}

/// Whether a part of mount kind `mount` goes on a mounting surface itself (doc 12 §3.3: screw-mounted devices go on
/// surfaces). EW-LAY-010 reports a rail device on a surface ("rail device not on a rail"); the commands accept it,
/// because `rehome_to_surface` creates exactly that state.
pub fn fits_surface(mount: Option<&MountKind>) -> Fit {
    match mount {
        None | Some(MountKind::Unknown(_)) => Fit::Unknown,
        Some(MountKind::Screw) => Fit::Fits,
        Some(_) => Fit::DoesNotFit,
    }
}
