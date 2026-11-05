// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `.ewui` script target (FT-1452, WP-S6.04 FP-2): bytes as a headless script (`ew.uiscript/1`) through the parser
//! the runner reads a script file with, `ew_viewmodel::script::parse`. The runner reads the file as UTF-8 and refuses
//! other bytes before the parser, so an input that is not UTF-8 ends there. Properties, whatever the bytes:
//!
//! - nothing panics: the parse and the text of its error;
//! - every step of a parsed script is on a line of its own, in order, with an object for its parameters;
//! - a parsed script written back (its header, then one `{"action", "params"}` line per step) parses to the same
//!   actions and parameters.
//!
//! The replay test runs the tutorial scripts of `docs/book/tutorial-scripts/` through [`check`] as its seeds.

use ew_viewmodel::script::{SCRIPT_SCHEMA, parse};
use serde_json::{Value, json};

/// What one input did.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Outcome {
    /// The bytes are UTF-8 and the script parsed.
    pub parsed: bool,
    /// The steps of the parsed script.
    pub steps: usize,
}

/// Runs one input.
///
/// # Panics
///
/// When a property of the module documentation fails.
pub fn check(input: &[u8]) -> Outcome {
    let Ok(text) = std::str::from_utf8(input) else {
        return Outcome::default();
    };
    let steps = match parse(text) {
        Ok(steps) => steps,
        Err(e) => {
            let _ = e.to_string();
            return Outcome::default();
        }
    };
    let mut last = 0;
    for step in &steps {
        assert!(step.line > last, "the steps are not on lines of their own, in order: {steps:?}");
        assert!(step.params.is_object(), "a step's parameters are no object: {step:?}");
        last = step.line;
    }
    let mut again = vec![json!({"$schema": SCRIPT_SCHEMA}).to_string()];
    again.extend(
        steps
            .iter()
            .map(|s| json!({"action": s.action, "params": s.params}).to_string()),
    );
    let reread = parse(&again.join("\n")).expect("a parsed script written back parses");
    let pairs = |s: &[ew_viewmodel::script::Step]| -> Vec<(String, Value)> {
        s.iter().map(|s| (s.action.clone(), s.params.clone())).collect()
    };
    assert_eq!(pairs(&reread), pairs(&steps), "a parsed script written back reads differently");
    Outcome {
        parsed: true,
        steps: steps.len(),
    }
}
