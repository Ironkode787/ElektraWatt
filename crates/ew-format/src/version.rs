// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reader decisions of doc 05 §10.1 (ADR-0011).

use std::cmp::Ordering;

use crate::manifest::Manifest;

/// What a reader may do with a project, from its manifest's stamps (doc 05 §10.1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Access {
    /// Read and write; a save writes this reader's format.
    ReadWrite,
    /// Read and write with the banner "written by a newer ElektraWatt; unknown data preserved"; the stamp
    /// stays the file's format (same major, newer minor, `min_reader` ≤ reader).
    ReadWriteNewer,
    /// Read only, with the reason.
    ReadOnly(&'static str),
    /// Migrate in memory first (an older major); the migration chain is WP-S1.17.
    Migrate,
    /// Do not open: nothing may be written, nothing is read.
    Refuse(String),
}

/// A `major.minor` format.
fn parse(format: &str) -> Option<(u32, u32)> {
    let (major, minor) = format.split_once('.')?;
    let num = |s: &str| {
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) && (s == "0" || !s.starts_with('0')))
            .then(|| s.parse().ok())
            .flatten()
    };
    Some((num(major)?, num(minor)?))
}

/// Whether two formats are compatible as "same major" (doc 05 §10.1: before 1.0 every step counts as
/// breaking, so `0.x` and `0.y` are different majors).
fn same_major(a: (u32, u32), b: (u32, u32)) -> bool {
    a.0 == b.0 && (a.0 > 0 || a.1 == b.1)
}

/// The reader decision for a manifest read by a reader of format `reader` (doc 05 §10.1, ADR-0011):
///
/// | Condition | Access |
/// |---|---|
/// | unparseable `format` or `min_reader`, or `min_reader` > reader | [`Access::Refuse`] |
/// | `unstable: true` and `release_build` | [`Access::ReadOnly`] |
/// | same major, file ≤ reader | [`Access::ReadWrite`] |
/// | same major, file > reader | [`Access::ReadWriteNewer`] |
/// | newer major | [`Access::ReadOnly`] |
/// | older major | [`Access::Migrate`] |
///
/// Before 1.0 every step is a major: `0.2` read by a `0.1` reader is a newer major.
///
/// ```
/// use ew_format::{Access, Manifest, reader_access};
///
/// let mut m = Manifest::new("0.1", "test", "8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a".parse().unwrap(), 1);
/// assert_eq!(reader_access(&m, "0.1", true), Access::ReadWrite);
/// m.format = "0.2".into();
/// assert!(matches!(reader_access(&m, "0.1", true), Access::ReadOnly(_)));
/// m.format = "1.3".into();
/// m.min_reader = "1.0".into();
/// assert_eq!(reader_access(&m, "1.1", true), Access::ReadWriteNewer);
/// m.min_reader = "1.2".into();
/// assert!(matches!(reader_access(&m, "1.1", true), Access::Refuse(_)));
/// ```
pub fn reader_access(manifest: &Manifest, reader: &str, release_build: bool) -> Access {
    let (Some(r), Some(f), Some(m)) = (
        parse(reader),
        parse(&manifest.format),
        parse(&manifest.min_reader),
    ) else {
        return Access::Refuse(format!(
            "unreadable format stamps \"{}\" / \"{}\"",
            manifest.format, manifest.min_reader
        ));
    };
    if m > r {
        return Access::Refuse(format!(
            "the project needs a reader of format {} or newer; this one reads {reader}",
            manifest.min_reader
        ));
    }
    if manifest.unstable && release_build {
        return Access::ReadOnly("written by a development build with an unreleased format");
    }
    if same_major(f, r) {
        return match f.cmp(&r) {
            Ordering::Greater => Access::ReadWriteNewer,
            _ => Access::ReadWrite,
        };
    }
    if f > r {
        Access::ReadOnly("written by a newer major format")
    } else {
        Access::Migrate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_parse_strictly() {
        assert_eq!(parse("0.1"), Some((0, 1)));
        assert_eq!(parse("10.20"), Some((10, 20)));
        for bad in ["", "1", "1.", ".1", "01.1", "1.01", "1.2.3", "a.b", "-1.0"] {
            assert_eq!(parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn before_one_every_step_breaks() {
        assert!(!same_major((0, 1), (0, 2)));
        assert!(same_major((0, 1), (0, 1)));
        assert!(same_major((1, 0), (1, 4)));
        let mut m = Manifest::new(
            "0.0",
            "t",
            "8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a".parse().unwrap(),
            1,
        );
        assert_eq!(reader_access(&m, "0.1", true), Access::Migrate);
        m.format = "0.1".into();
        m.min_reader = "0.1".into();
        m.unstable = true;
        assert!(matches!(reader_access(&m, "0.1", true), Access::ReadOnly(_)));
        assert_eq!(reader_access(&m, "0.1", false), Access::ReadWrite);
    }
}
