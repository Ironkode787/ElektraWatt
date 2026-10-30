// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The content digest of a saved generation (doc 15 §5.4; WP-S1.18 D8, FR-REV-021): the value a checker's sign-off
//! binds to, so that any later content edit voids it.
//!
//! **Definition.** SHA-256 over the sorted `(path, sha256)` entries of the generation's manifest, without the files
//! that are not content ([`ew_schema::NOT_CONTENT_FILES`]: `revisions.jsonl` and `comments.jsonl`). The bytes hashed
//! are one line per entry, in byte order of the paths (the order of [`Manifest::files`]):
//!
//! ```text
//! <64 lower-case hex digits of the file's SHA-256><space><space><path><LF>
//! ```
//!
//! which is the line format of `sha256sum` (and of the archive's `SHA256SUMS`, doc 05 §14.2), so a third party can
//! recompute the digest from the manifest or from the files with standard tools. `.ew/` and the lock files are never in
//! the manifest. The function is pure: a function of one manifest, no file is read.

use std::fmt;
use std::fmt::Write as _;

use ew_schema::NOT_CONTENT_FILES;

use crate::manifest::{FileHash, Manifest, sha256};

/// A manifest whose content digest cannot be computed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ContentDigestError {
    /// A content file's hash is `sha256:pending` (a manifest the merge driver wrote, doc 05 §11.3): the hashes are
    /// not authoritative until the next save.
    Pending {
        /// The file.
        path: String,
    },
}

impl fmt::Display for ContentDigestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentDigestError::Pending { path } => write!(
                f,
                "the manifest's hash of {path} is pending (a merged project): save the project first"
            ),
        }
    }
}

impl std::error::Error for ContentDigestError {}

/// The bytes the content digest hashes: one `sha256sum` line per content file of `manifest`, in path order.
///
/// # Errors
///
/// [`ContentDigestError::Pending`] for a content file whose hash is pending.
pub fn content_lines(manifest: &Manifest) -> Result<String, ContentDigestError> {
    let mut out = String::new();
    for (path, hash) in &manifest.files {
        if NOT_CONTENT_FILES.contains(&path.as_str()) {
            continue;
        }
        let FileHash::Sha256(bytes) = hash else {
            return Err(ContentDigestError::Pending {
                path: path.as_str().to_owned(),
            });
        };
        for b in bytes {
            let _ = write!(out, "{b:02x}");
        }
        out.push_str("  ");
        out.push_str(path.as_str());
        out.push('\n');
    }
    Ok(out)
}

/// The content digest of the generation `manifest` describes (doc 15 §5.4; WP-S1.18 D8): SHA-256 over
/// [`content_lines`].
///
/// ```
/// use ew_format::{FileHash, FilePath, Manifest, content_digest, sha256};
///
/// let mut m = Manifest::new("0.1", "ElektraWatt 0.0.0", "7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37".parse().unwrap(), 3);
/// m.files.insert(FilePath::new("project.jsonl").unwrap(), FileHash::Sha256(sha256(b"p\n")));
/// let digest = content_digest(&m).unwrap();
/// // Revision records are not content: a sign-off written into them leaves the digest as it is.
/// m.files.insert(FilePath::new("revisions.jsonl").unwrap(), FileHash::Sha256(sha256(b"r\n")));
/// assert_eq!(content_digest(&m).unwrap(), digest);
/// ```
///
/// # Errors
///
/// [`ContentDigestError::Pending`] for a content file whose hash is pending.
pub fn content_digest(manifest: &Manifest) -> Result<[u8; 32], ContentDigestError> {
    content_lines(manifest).map(|lines| sha256(lines.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::FilePath;
    use proptest::prelude::*;

    fn manifest(files: &[(&str, &[u8])]) -> Manifest {
        let mut m = Manifest::new(
            "0.1",
            "ElektraWatt 0.0.0",
            "7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37".parse().unwrap(),
            1,
        );
        for (path, bytes) in files {
            m.files
                .insert(FilePath::new(*path).unwrap(), FileHash::Sha256(sha256(bytes)));
        }
        m
    }

    /// The bytes hashed are `sha256sum` lines in path order, without the files that are not content.
    #[test]
    fn the_lines_are_sha256sum_lines_of_the_content_files_in_path_order() {
        let m = manifest(&[
            ("revisions.jsonl", b"r\n"),
            ("project.jsonl", b"p\n"),
            ("comments.jsonl", b"c\n"),
            ("devices/a.jsonl", b"d\n"),
        ]);
        let hex = |b: &[u8]| -> String { sha256(b).iter().map(|x| format!("{x:02x}")).collect() };
        assert_eq!(
            content_lines(&m).unwrap(),
            format!(
                "{}  devices/a.jsonl\n{}  project.jsonl\n",
                hex(b"d\n"),
                hex(b"p\n")
            )
        );
        assert_eq!(
            content_digest(&m).unwrap(),
            sha256(content_lines(&m).unwrap().as_bytes())
        );
        // The empty project: the digest of no lines.
        assert_eq!(content_digest(&manifest(&[])).unwrap(), sha256(b""));
    }

    /// A pending hash (a merged manifest) has no digest; a pending hash of a file that is not content does not
    /// matter.
    #[test]
    fn a_pending_content_hash_has_no_digest() {
        let mut m = manifest(&[("project.jsonl", b"p\n")]);
        m.files
            .insert(FilePath::new("revisions.jsonl").unwrap(), FileHash::Pending);
        assert!(content_digest(&m).is_ok());
        m.files
            .insert(FilePath::new("devices/a.jsonl").unwrap(), FileHash::Pending);
        assert_eq!(
            content_digest(&m),
            Err(ContentDigestError::Pending {
                path: "devices/a.jsonl".to_owned()
            })
        );
    }

    /// The generation, the generator and the format stamps are not content.
    #[test]
    fn only_the_file_entries_count() {
        let a = manifest(&[("project.jsonl", b"p\n")]);
        let mut b = a.clone();
        b.generation = 9;
        b.generator = "ElektraWatt 9.9.9".to_owned();
        assert_eq!(content_digest(&a), content_digest(&b));
    }

    proptest! {
        /// Changing, adding or removing a content file changes the digest; changing the files that are not content
        /// never does (RG-3 at the format level: a comment or page-note edit does not void a sign-off).
        #[test]
        fn content_files_and_only_they_move_the_digest(
            names in proptest::collection::btree_set("[a-z]{1,8}", 1..6),
            seed in any::<u8>(),
            pick in any::<prop::sample::Index>(),
        ) {
            let files: Vec<(String, Vec<u8>)> = names
                .iter()
                .map(|n| (format!("devices/{n}.jsonl"), vec![seed, n.len() as u8]))
                .collect();
            let refs: Vec<(&str, &[u8])> = files.iter().map(|(p, b)| (p.as_str(), b.as_slice())).collect();
            let base = manifest(&refs);
            let digest = content_digest(&base).unwrap();

            let mut other = base.clone();
            for not_content in NOT_CONTENT_FILES {
                other.files.insert(
                    FilePath::new(not_content).unwrap(),
                    FileHash::Sha256(sha256(&[seed])),
                );
            }
            prop_assert_eq!(content_digest(&other).unwrap(), digest);

            let (path, bytes) = &files[pick.index(files.len())];
            let mut changed = base.clone();
            let mut edited = bytes.clone();
            edited.push(1);
            changed
                .files
                .insert(FilePath::new(path).unwrap(), FileHash::Sha256(sha256(&edited)));
            prop_assert_ne!(content_digest(&changed).unwrap(), digest);

            let mut removed = base.clone();
            removed.files.remove(&FilePath::new(path).unwrap());
            prop_assert_ne!(content_digest(&removed).unwrap(), digest);
        }
    }
}
