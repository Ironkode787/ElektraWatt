// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `Session` facade shared by every front-end: registers the SQL and plugin rule implementations at
//! session start and composes `.ewpp` export jobs.

#![forbid(unsafe_code)]
