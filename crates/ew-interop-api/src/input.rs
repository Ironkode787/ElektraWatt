// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `ImportInput`: one source file as bytes, with its name and hash.

use std::fmt;
use std::sync::Arc;

use sha2::{Digest, Sha256};

/// One source file: its name (a file name, never a path: imports never learn where a file lives, doc 14
/// §2.1, NFR-PRIV), its bytes and their SHA-256. The fields are private so that the name rule and the hash
/// cannot be bypassed; read them through the accessors.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImportInput {
    name: String,
    bytes: Arc<[u8]>,
    sha256: [u8; 32],
}

/// Why [`ImportInput::new`] or [`check_file_name`] refused a name.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum InputNameError {
    /// The name is empty.
    Empty,
    /// The name contains `/`, `\`, `:` or a NUL byte: it is a path, not a file name.
    NotAFileName,
}

impl fmt::Display for InputNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InputNameError::Empty => f.write_str("file name is empty"),
            InputNameError::NotAFileName => {
                f.write_str("not a file name (contains a path separator, ':' or NUL)")
            }
        }
    }
}

impl std::error::Error for InputNameError {}

/// Checks that `name` is a file name: not empty and without `/`, `\`, `:` or a NUL byte.
///
/// # Errors
///
/// [`InputNameError::Empty`] for an empty name; [`InputNameError::NotAFileName`] for a path.
pub fn check_file_name(name: &str) -> Result<(), InputNameError> {
    if name.is_empty() {
        return Err(InputNameError::Empty);
    }
    if name.contains(['/', '\\', ':', '\0']) {
        return Err(InputNameError::NotAFileName);
    }
    Ok(())
}

impl ImportInput {
    /// The input named `name` with `bytes`; computes the SHA-256.
    ///
    /// # Errors
    ///
    /// [`InputNameError`] if `name` is empty or is a path (contains `/`, `\`, `:` or NUL).
    pub fn new(name: &str, bytes: impl Into<Arc<[u8]>>) -> Result<ImportInput, InputNameError> {
        check_file_name(name)?;
        let bytes: Arc<[u8]> = bytes.into();
        let sha256: [u8; 32] = Sha256::digest(&bytes[..]).into();
        Ok(ImportInput {
            name: name.to_owned(),
            bytes,
            sha256,
        })
    }

    /// The file name without any directory, such as `contact.elmt`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The file's bytes.
    pub fn bytes(&self) -> &Arc<[u8]> {
        &self.bytes
    }

    /// SHA-256 of [`bytes`](Self::bytes).
    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
}
