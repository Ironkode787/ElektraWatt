// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Quat9` and `Pose3`: integer 3D poses.

use std::fmt;

use serde::de::{self, Error as _, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::json::SafeInt;
use crate::{Nm, UnitsError};

/// A unit quaternion `(w, x, y, z)` with components stored as integers in units of 10⁻⁹ (ADR-0005), so
/// files never carry floats.
///
/// A `Quat9` is always unit (within [`UNIT_TOLERANCE`](Quat9::UNIT_TOLERANCE)) and **sign-normalised**:
/// `w > 0`, or, if `w = 0`, the first non-zero component is positive (doc 05 §3.2). `q` and `−q` are the
/// same rotation, so this makes the encoding unique. [`new`](Quat9::new) normalises the sign;
/// deserialising rejects input that is not already normalised.
///
/// f64 and glam conversions are deferred to the crates that need them (`ew-panel`); they round each
/// component to the nearest 10⁻⁹ and then call [`new`](Quat9::new).
///
/// **JSON:** `[w, x, y, z]`.
///
/// ```
/// use ew_units::Quat9;
///
/// // A half turn about z, given with the "wrong" sign, is normalised.
/// let q = Quat9::new(0, 0, 0, -1_000_000_000).unwrap();
/// assert_eq!(q.components(), [0, 0, 0, 1_000_000_000]);
/// assert_eq!(serde_json::to_string(&q).unwrap(), "[0,0,0,1000000000]");
/// assert!(Quat9::new(1_000_000_000, 1_000_000_000, 0, 0).is_err());
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Quat9 {
    w: i64,
    x: i64,
    y: i64,
    z: i64,
}

impl Quat9 {
    /// The scale of a component: 1.0 is `1_000_000_000`.
    pub const SCALE: i64 = 1_000_000_000;

    /// The identity rotation `(1, 0, 0, 0)`.
    pub const IDENTITY: Quat9 = Quat9 {
        w: Quat9::SCALE,
        x: 0,
        y: 0,
        z: 0,
    };

    /// The accepted deviation of `w² + x² + y² + z²` from 10¹⁸.
    ///
    /// **Derivation.** Each stored component is `cᵢ = 10⁹·qᵢ + eᵢ` with `|eᵢ| ≤ ½` for an exact unit
    /// quaternion `q`. Then `Σcᵢ² − 10¹⁸ = 2·10⁹·Σqᵢeᵢ + Σeᵢ²`; by Cauchy–Schwarz `|Σqᵢeᵢ| ≤ ‖q‖·‖e‖ ≤ 1`, and
    /// `Σeᵢ² ≤ 1`, so the rounding of four components moves the squared norm by at most `2·10⁹ + 1`. A
    /// further `999_999` admits a source quaternion whose norm is off by a relative 5 × 10⁻¹³, far above the
    /// error of an f64 normalisation. Total: `2_001_000_000` (about 10⁻⁹ of the squared norm).
    pub const UNIT_TOLERANCE: i128 = 2_001_000_000;

    /// The unit quaternion `(w, x, y, z)` (in 10⁻⁹), sign-normalised.
    ///
    /// # Errors
    ///
    /// [`UnitsError::NotUnitQuaternion`] if the components fail [`is_unit`](Quat9::is_unit).
    pub fn new(w: i64, x: i64, y: i64, z: i64) -> Result<Quat9, UnitsError> {
        let c = [w, x, y, z];
        if !Quat9::is_unit(c) {
            return Err(UnitsError::NotUnitQuaternion);
        }
        let [w, x, y, z] = Quat9::sign_normalized(c);
        Ok(Quat9 { w, x, y, z })
    }

    /// Whether `|w² + x² + y² + z² − 10¹⁸| ≤ UNIT_TOLERANCE`, computed exactly.
    pub fn is_unit(c: [i64; 4]) -> bool {
        // A component beyond 2·10⁹ cannot belong to a unit quaternion; the bound also keeps the sum of
        // squares far inside i128.
        if c.iter().any(|v| v.unsigned_abs() > 2 * Quat9::SCALE as u64) {
            return false;
        }
        let sum: i128 = c.iter().map(|&v| i128::from(v) * i128::from(v)).sum();
        let one = i128::from(Quat9::SCALE) * i128::from(Quat9::SCALE);
        (sum - one).abs() <= Quat9::UNIT_TOLERANCE
    }

    /// Whether `w > 0`, or `w = 0` and the first non-zero of `x, y, z` is positive.
    pub fn is_sign_normalized(c: [i64; 4]) -> bool {
        c.iter().find(|&&v| v != 0).is_some_and(|&v| v > 0)
    }

    /// `c` or `−c`, whichever is sign-normalised (the all-zero input is returned unchanged). Negation
    /// wraps for `i64::MIN`, which no unit quaternion contains.
    pub fn sign_normalized(c: [i64; 4]) -> [i64; 4] {
        match c.iter().find(|&&v| v != 0) {
            Some(&v) if v < 0 => c.map(i64::wrapping_neg),
            _ => c,
        }
    }

    /// The scalar component `w`, in 10⁻⁹.
    pub const fn w(self) -> i64 {
        self.w
    }

    /// The `x` component, in 10⁻⁹.
    pub const fn x(self) -> i64 {
        self.x
    }

    /// The `y` component, in 10⁻⁹.
    pub const fn y(self) -> i64 {
        self.y
    }

    /// The `z` component, in 10⁻⁹.
    pub const fn z(self) -> i64 {
        self.z
    }

    /// `[w, x, y, z]`, in 10⁻⁹.
    pub const fn components(self) -> [i64; 4] {
        [self.w, self.x, self.y, self.z]
    }
}

impl Default for Quat9 {
    fn default() -> Quat9 {
        Quat9::IDENTITY
    }
}

impl Serialize for Quat9 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Components are bounded by 2·10⁹, far inside the JSON-safe range.
        self.components().map(SafeInt).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Quat9 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let c = <[SafeInt; 4]>::deserialize(deserializer)?.map(|v| v.0);
        if !Quat9::is_unit(c) {
            return Err(D::Error::custom(UnitsError::NotUnitQuaternion));
        }
        if !Quat9::is_sign_normalized(c) {
            return Err(D::Error::custom(UnitsError::NotSignNormalized));
        }
        let [w, x, y, z] = c;
        Ok(Quat9 { w, x, y, z })
    }
}

/// A rigid 3D pose: a translation of three [`Nm`] plus a rotation [`Quat9`] (ADR-0005, arch §7.3).
///
/// f64 mm (glam `DVec3`) appears only in 3D maths and f32 only in GPU buffers; neither is persisted.
/// Conversions live in the crates that need them (`ew-panel`).
///
/// **JSON:** `{"t":[x,y,z],"q":[w,x,y,z]}`, object form only (the array form `[t, q]` is rejected); both
/// members are required, none may repeat and no others are allowed.
///
/// ```
/// use ew_units::{Nm, Pose3, Quat9};
///
/// let pose = Pose3 { t: [Nm(0), Nm(500_000_000), Nm(0)], q: Quat9::IDENTITY };
/// let json = r#"{"t":[0,500000000,0],"q":[1000000000,0,0,0]}"#;
/// assert_eq!(serde_json::to_string(&pose).unwrap(), json);
/// assert_eq!(serde_json::from_str::<Pose3>(json).unwrap(), pose);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize)]
pub struct Pose3 {
    /// Translation `[x, y, z]` in nm.
    pub t: [Nm; 3],
    /// Rotation.
    pub q: Quat9,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "lowercase")]
enum PoseField {
    T,
    Q,
}

/// Deserialised by hand so that only the object form is accepted (a derived impl would also take `[t, q]`).
impl<'de> Deserialize<'de> for Pose3 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_struct("Pose3", &["t", "q"], PoseVisitor)
    }
}

struct PoseVisitor;

impl<'de> Visitor<'de> for PoseVisitor {
    type Value = Pose3;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(r#"an object {"t": [x, y, z], "q": [w, x, y, z]}"#)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Pose3, A::Error> {
        let (mut t, mut q) = (None, None);
        while let Some(field) = map.next_key()? {
            match field {
                PoseField::T if t.is_none() => t = Some(map.next_value()?),
                PoseField::Q if q.is_none() => q = Some(map.next_value()?),
                _ => return Err(de::Error::custom("duplicate pose member")),
            }
        }
        match (t, q) {
            (Some(t), Some(q)) => Ok(Pose3 { t, q }),
            (None, _) => Err(de::Error::missing_field("t")),
            (_, None) => Err(de::Error::missing_field("q")),
        }
    }
}

impl Pose3 {
    /// No translation, no rotation.
    pub const IDENTITY: Pose3 = Pose3 {
        t: [Nm::ZERO; 3],
        q: Quat9::IDENTITY,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MAX_SAFE_INTEGER;
    use proptest::prelude::*;

    const S: i64 = Quat9::SCALE;

    #[test]
    fn identity_and_defaults() {
        assert_eq!(Quat9::default(), Quat9::IDENTITY);
        assert_eq!(Pose3::default(), Pose3::IDENTITY);
        assert_eq!(Quat9::IDENTITY.components(), [S, 0, 0, 0]);
        assert!(Quat9::is_unit(Quat9::IDENTITY.components()));
    }

    #[test]
    fn sign_normalisation() {
        assert_eq!(Quat9::new(-S, 0, 0, 0).unwrap(), Quat9::IDENTITY);
        assert_eq!(Quat9::new(0, -S, 0, 0).unwrap().components(), [0, S, 0, 0]);
        assert_eq!(Quat9::new(0, 0, -S, 0).unwrap().components(), [0, 0, S, 0]);
        // w = 0: the first non-zero component decides, later signs follow.
        let h = 707_106_781;
        assert_eq!(Quat9::new(0, -h, h, 0).unwrap().components(), [0, h, -h, 0]);
        assert_eq!(Quat9::new(-h, 0, 0, h).unwrap().components(), [h, 0, 0, -h]);
        assert!(Quat9::is_sign_normalized([h, 0, 0, -h]));
        assert!(!Quat9::is_sign_normalized([0, 0, -S, 0]));
        assert!(!Quat9::is_sign_normalized([0, 0, 0, 0]));
        assert_eq!(Quat9::sign_normalized([0, 0, 0, 0]), [0, 0, 0, 0]);
    }

    #[test]
    fn unit_tolerance() {
        // cos 45°, sin 45° rounded: 707106781² · 2 = 999999999472363922, 527636078 below 10¹⁸.
        let h = 707_106_781;
        assert!(Quat9::is_unit([h, h, 0, 0]));
        // A quaternion off by a whole 10⁻⁶ in one component is rejected.
        assert!(!Quat9::is_unit([S - 1_000, 0, 0, 0]));
        assert!(!Quat9::is_unit([0, 0, 0, 0]));
        assert!(!Quat9::is_unit([i64::MIN, 0, 0, 0]));
        assert!(!Quat9::is_unit([i64::MAX, i64::MAX, i64::MAX, i64::MAX]));
        // The bound itself: 10¹⁸ ± UNIT_TOLERANCE.
        let edge = |w: i64| i128::from(w) * i128::from(w) - i128::from(S) * i128::from(S);
        assert!(edge(S + 1).abs() <= Quat9::UNIT_TOLERANCE);
        assert!(edge(S + 2).abs() > Quat9::UNIT_TOLERANCE);
        assert!(Quat9::is_unit([S + 1, 0, 0, 0]));
        assert!(!Quat9::is_unit([S + 2, 0, 0, 0]));
        assert_eq!(Quat9::new(S, S, 0, 0), Err(UnitsError::NotUnitQuaternion));
    }

    #[test]
    fn json_encoding() {
        let pose = Pose3 {
            t: [Nm(0), Nm(500_000_000), Nm(0)],
            q: Quat9::IDENTITY,
        };
        let json = r#"{"t":[0,500000000,0],"q":[1000000000,0,0,0]}"#;
        assert_eq!(serde_json::to_string(&pose).unwrap(), json);
        assert_eq!(serde_json::from_str::<Pose3>(json).unwrap(), pose);
        for bad in [
            r#"{"t":[0,0,0],"q":[-1000000000,0,0,0]}"#,
            r#"{"t":[0,0,0],"q":[0,0,-1000000000,0]}"#,
            r#"{"t":[0,0,0],"q":[1000000000,1000000000,0,0]}"#,
            r#"{"t":[0,0,0],"q":[0,0,0,0]}"#,
            r#"{"t":[0,0,0],"q":[1.0,0,0,0]}"#,
            r#"{"t":[0,0,0],"q":[1000000000,0,0]}"#,
            r#"{"t":[0,0],"q":[1000000000,0,0,0]}"#,
            r#"{"t":[0,0,0.5],"q":[1000000000,0,0,0]}"#,
            r#"{"t":[0,0,9007199254740992],"q":[1000000000,0,0,0]}"#,
            r#"{"t":[0,0,0]}"#,
            r#"{"q":[1000000000,0,0,0]}"#,
            r#"{"t":[0,0,0],"q":[1000000000,0,0,0],"r":1}"#,
            r#"[[0,0,0],[1000000000,0,0,0]]"#,
            r#"{"t":[0,0,0],"t":[0,0,0],"q":[1000000000,0,0,0]}"#,
        ] {
            assert!(serde_json::from_str::<Pose3>(bad).is_err(), "{bad}");
        }
        let far = Pose3 {
            t: [Nm(MAX_SAFE_INTEGER + 1), Nm(0), Nm(0)],
            q: Quat9::IDENTITY,
        };
        assert!(serde_json::to_string(&far).is_err());
    }

    /// Rounds a unit quaternion given as f64 to the stored form, as the deferred conversions will.
    fn quantise(q: [f64; 4]) -> [i64; 4] {
        q.map(|v| crate::arith::f64_to_i64(v * 1e9).unwrap())
    }

    fn unit_quats() -> impl Strategy<Value = Quat9> {
        (-1.0f64..1.0, -1.0f64..1.0, -1.0f64..1.0, -1.0f64..1.0).prop_filter_map(
            "degenerate",
            |(a, b, c, d)| {
                let n = (a * a + b * b + c * c + d * d).sqrt();
                if n < 1e-3 {
                    return None;
                }
                let [w, x, y, z] = quantise([a / n, b / n, c / n, d / n]);
                Quat9::new(w, x, y, z).ok()
            },
        )
    }

    proptest! {
        /// Every rounded f64 unit quaternion passes the tolerance (none is filtered out by `new`).
        #[test]
        fn rounded_unit_quaternions_are_unit(a in -1.0f64..1.0, b in -1.0f64..1.0, c in -1.0f64..1.0, d in -1.0f64..1.0) {
            let n = (a * a + b * b + c * c + d * d).sqrt();
            prop_assume!(n >= 1e-3);
            prop_assert!(Quat9::is_unit(quantise([a / n, b / n, c / n, d / n])));
        }

        #[test]
        fn new_is_sign_normalised_and_idempotent(q in unit_quats()) {
            prop_assert!(Quat9::is_sign_normalized(q.components()));
            let [w, x, y, z] = q.components();
            prop_assert_eq!(Quat9::new(w, x, y, z).unwrap(), q);
            prop_assert_eq!(Quat9::new(-w, -x, -y, -z).unwrap(), q);
        }

        #[test]
        fn pose_serde_round_trip(q in unit_quats(), t in prop::array::uniform3(-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER)) {
            let pose = Pose3 { t: t.map(Nm), q };
            let s = serde_json::to_string(&pose).unwrap();
            prop_assert_eq!(serde_json::from_str::<Pose3>(&s).unwrap(), pose);
        }
    }
}
