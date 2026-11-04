// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The command log target (WP-S6.04 D9; doc 17 §3.5, doc 18 §11.5 "`HumanOnly` replay"): the input as a `.ewcmd`
//! (lossy UTF-8) parsed by `CmdLog::parse` and run by `Session::run_script` on a new in-memory session of the golden
//! project. Properties: whatever the bytes, nothing panics; each line commits, or the replay stops at a refused or
//! failing line with the earlier lines committed; a log that names a `HumanOnly` command anywhere is refused before
//! its first line and changes nothing; a log refused before its first line changes nothing; and the model passes the
//! structural check afterwards.

use ew_api::{CmdLog, CmdLogError, Exposure};

use crate::golden;

/// What one input did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Outcome {
    /// Whether the text parsed as a command log.
    pub parsed: bool,
    /// The lines that committed.
    pub committed: usize,
}

/// Runs one input.
///
/// # Panics
///
/// When a `HumanOnly` log is not refused, a refused log changed the model, or the model fails the structural check.
pub fn check(input: &[u8]) -> Outcome {
    let Ok(log) = CmdLog::parse(&String::from_utf8_lossy(input)) else {
        return Outcome {
            parsed: false,
            committed: 0,
        };
    };
    let mut session = golden::session(1);
    let before = session.world();
    let human = log.commands().any(|c| {
        session
            .registry()
            .get(&c.name)
            .is_some_and(|e| e.meta().exposure == Exposure::HumanOnly)
    });
    let result = session.run_script(&log, "fuzz.ewcmd");
    if human {
        assert!(
            matches!(result, Err(CmdLogError::Refused { .. })),
            "a log with a HumanOnly command ran: {result:?}"
        );
    }
    if matches!(result, Err(CmdLogError::Refused { .. })) {
        assert!(
            session.world().model.content_eq(&before.model),
            "a refused log changed the model"
        );
    }
    if let Err(e) = session.world().model.integrity_check() {
        panic!("the model fails the structural check after the log: {e}");
    }
    Outcome {
        parsed: true,
        committed: match &result {
            Ok(receipts) => receipts.len(),
            Err(e) => e.committed(),
        },
    }
}
