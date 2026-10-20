// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reading the `.qet` DOM into [`QetProject`] (STUB: implemented by the reader work item).

use ew_interop_api::{ImportError, Limits};

use super::cx::Findings;
use super::model::QetProject;

/// Reads the project in `xml` into owned data. Runs on the parser thread, after [`crate::xml::prescan`].
///
/// # Errors
///
/// [`ImportError::Malformed`] for malformed XML or a DOCTYPE, [`ImportError::LimitExceeded`] for an
/// exceeded limit, [`ImportError::Unsupported`] if the root is not `<project>`.
pub fn read_project(xml: &str, findings: &mut Findings, limits: &Limits) -> Result<QetProject, ImportError> {
    let _ = (xml, findings, limits);
    Ok(QetProject::default())
}
