// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The asset store of a [`Model`](crate::Model) (doc 05 §13.3, FT-337): content-addressed image bytes that records
//! name through an [`AssetRef`].
//!
//! Assets are not rows and not ops. Their bytes are immutable and named by their SHA-256, so putting an asset
//! the model holds already changes nothing, and holding one that no record names is harmless: the model keeps
//! every asset it was given for its lifetime, which covers "while any record or undo entry references them" (doc
//! 05 §13.3). A transaction carries the assets its ops name ([`Model::put_asset`] is called with the ops'
//! candidate); the store loads them at open and writes the named ones at save.

use std::sync::Arc;

use ew_schema::{AssetRef, Sha256};

/// One asset: its reference (hash, format, intrinsic size) and its bytes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Asset {
    reference: AssetRef,
    bytes: Arc<[u8]>,
}

impl Asset {
    /// An asset from its bytes and the reference the asset store made for them ([`ew_schema::sniff`] and the
    /// SHA-256 of `bytes`). The caller computed `reference.sha256` over `bytes`: the model does not hash.
    pub fn new(reference: AssetRef, bytes: impl Into<Arc<[u8]>>) -> Asset {
        Asset {
            reference,
            bytes: bytes.into(),
        }
    }

    /// The reference.
    pub fn reference(&self) -> &AssetRef {
        &self.reference
    }

    /// The content hash.
    pub fn sha256(&self) -> &Sha256 {
        &self.reference.sha256
    }

    /// The encoded file.
    pub fn bytes(&self) -> &Arc<[u8]> {
        &self.bytes
    }
}
