// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The command-sequence target (FT-272, FT-297, doc 18 §3.6): the seeded generator of the nightly command
//! fuzzer ([`crate::command_fuzz`], the modules of `crates/ew-testkit/tests/command_fuzz`) over the bytes of the
//! fuzzer's input.
//!
//! Input: a seed (eight bytes, little-endian), a step count (one byte, `1..=`[`MAX_STEPS`]) and the tape: every
//! draw of the generator is its seeded SplitMix64 output XOR the next eight bytes of the tape
//! ([`crate::command_fuzz`]'s `SplitMix64::with_tape`), so changing one tape word changes one draw (one command,
//! one target, one value) and libFuzzer's coverage feedback steers the sequence; a tape of zeros is the plain
//! seeded run of the nightly test with that seed. The run is the one of `fuzz_long` with a cheap cadence: the
//! structural check after every step that changed the model, the validity oracle of every command (a well-formed
//! step must commit, a malformed one must be refused and change nothing, the checks of what it created), the
//! save-and-reload round trip (a few percent of the steps) and the closing undo-all (the undo law: the initial model
//! comes back). Left to the nightly test: the memo sample (it reads the derived values cold in oracle mode, 1.5 to 8
//! seconds on the larger sequences, against the milliseconds of a step), `verify_derived`, the pre-delivery check, the
//! deep round trip and the CSV imports, so that the target runs a few inputs a second. Any violated oracle panics,
//! which the fuzzer reports as a crash.

use crate::command_fuzz::{Cadence, run_fuzz_tape};

/// The most steps of one input.
pub const MAX_STEPS: usize = 8;

/// The cadence of the target: no memo sample (the memo oracle belongs to the nightly test and to CI-N-02's full oracle
/// run), no sweeps, no imports.
pub fn cadence() -> Cadence {
    Cadence {
        sample_every: 0,
        check_every: 0,
        verify_every: 0,
        full_end: false,
        save_weight: 1,
        import_weight: 0,
        deep: false,
    }
}

/// A decoded input.
#[derive(Debug, PartialEq, Eq)]
pub struct Case<'a> {
    /// The seed of the run.
    pub seed: u64,
    /// The number of steps.
    pub steps: usize,
    /// The tape.
    pub tape: &'a [u8],
}

/// Decodes an input; `None` when it is shorter than the seed and the step count.
pub fn decode(input: &[u8]) -> Option<Case<'_>> {
    let (seed, rest) = input.split_first_chunk::<8>()?;
    let (&steps, tape) = rest.split_first()?;
    Some(Case {
        seed: u64::from_le_bytes(*seed),
        steps: 1 + usize::from(steps) % MAX_STEPS,
        tape,
    })
}

/// The inverse of [`decode`] for the seed corpus and a regression input.
pub fn encode(seed: u64, steps: usize, tape: &[u8]) -> Vec<u8> {
    let mut out = seed.to_le_bytes().to_vec();
    out.push(u8::try_from(steps.clamp(1, MAX_STEPS) - 1).expect("a small step count"));
    out.extend_from_slice(tape);
    out
}

/// Runs the sequence of one input; panics (with the seed, the step, the oracle and the command log) when an
/// oracle does not hold.
///
/// Returns the number of commands that committed (for the replay test; the fuzz target ignores it).
pub fn check(input: &[u8]) -> usize {
    let Some(case) = decode(input) else { return 0 };
    run_fuzz_tape(case.seed, case.tape, case.steps, &cadence()).committed()
}
