// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! [`LibraryError`]: why a registry operation did not happen.

use std::fmt;
use std::time::SystemTime;

use ew_ids::Uuid;

/// Why a registry operation did not happen (WP-S1.19 §5.2).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LibraryError {
    /// A write to a system or community library, or a library created inside a read-only tier (D4).
    ReadOnly,
    /// No installed library holds the library, the item, or the item at a version the reference accepts.
    NotFound,
    /// Another live session holds the library's lock (`library.lock`, doc 05 §7.1).
    Locked {
        /// The holder's OS user.
        holder: String,
        /// The holder's host.
        host: String,
        /// When the holder took the lock, RFC 3339 as the lock record states it.
        since: String,
    },
    /// The library's lock is stale (doc 05 §7.4: older than `lock.stale_after`, or a dead process on this host).
    /// A takeover needs the user's confirmation: the front end then calls
    /// [`Registry::take_over_stale`](crate::Registry::take_over_stale).
    Stale {
        /// The holder's OS user (empty for an unreadable lock record).
        holder: String,
        /// The holder's host (empty for an unreadable lock record).
        host: String,
        /// The lock file's modification time, the holder's last heartbeat.
        last_seen: Option<SystemTime>,
    },
    /// A write of other content under a published `(item, version)` (doc 05 §13.1: versions are immutable).
    Immutable {
        /// The item.
        item: Uuid,
        /// The version the library holds with other content.
        version: String,
    },
    /// A library directory or document that cannot be used, with the reason.
    Format(String),
    /// The search index failed, with SQLite's message.
    Index(String),
    /// A file-system error, with its message.
    Io(String),
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LibraryError::ReadOnly => {
                f.write_str("the library is read-only (system and community libraries)")
            }
            LibraryError::NotFound => f.write_str("no installed library holds it"),
            LibraryError::Locked { holder, host, since } => {
                write!(f, "the library is locked by {holder}@{host} since {since}")
            }
            LibraryError::Stale { holder, host, .. } => write!(
                f,
                "the library's lock of {holder}@{host} is stale; taking it over needs confirmation"
            ),
            LibraryError::Immutable { item, version } => write!(
                f,
                "item {item} is published at version {version} with other content; give the change a new version"
            ),
            LibraryError::Format(m) => write!(f, "library format: {m}"),
            LibraryError::Index(m) => write!(f, "library index: {m}"),
            LibraryError::Io(m) => write!(f, "library I/O: {m}"),
        }
    }
}

impl std::error::Error for LibraryError {}

impl From<std::io::Error> for LibraryError {
    fn from(e: std::io::Error) -> LibraryError {
        LibraryError::Io(e.to_string())
    }
}

impl From<rusqlite::Error> for LibraryError {
    fn from(e: rusqlite::Error) -> LibraryError {
        LibraryError::Index(e.to_string())
    }
}
