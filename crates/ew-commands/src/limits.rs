// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The size limits of an untrusted parameter document ([`check_limits`]): its bytes, its nesting, the length of an
//! array and of a string. They run before the parameters are read into the command struct, whose serde reading
//! refuses a wrong parameter with the JSON Pointer of the offending value (doc 17 §2.1).

use std::io;

use ew_l10n::Text;
use serde_json::Value;

/// The most bytes a parameter document may have, measured as compact JSON.
pub const MAX_PARAM_BYTES: usize = 4 * 1024 * 1024;
/// The deepest nesting of arrays and objects a parameter document may have: a scalar root is depth 0,
/// `[]` is depth 1.
pub const MAX_PARAM_DEPTH: usize = 64;
/// The most items any one array of a parameter document may have.
pub const MAX_PARAM_ITEMS: usize = 1_048_576;
/// The most UTF-8 bytes any one string (object keys included) of a parameter document may have.
pub const MAX_PARAM_STRING: usize = 1024 * 1024;

/// The first limit a parameter document passes: where and what.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LimitViolation {
    /// The JSON Pointer of the offending value in the document (`""` for the root, `/route/1/0`).
    pub path: String,
    /// What is wrong, by key.
    pub text: Text,
}

/// Checks the size limits of a parameter document ([`MAX_PARAM_DEPTH`], [`MAX_PARAM_ITEMS`],
/// [`MAX_PARAM_STRING`], then [`MAX_PARAM_BYTES`]), reporting the first violation with the JSON Pointer of
/// the offending value (`""` for the byte length of the whole document). Exactly at a bound is accepted.
///
/// One walk bounded to depth `MAX_PARAM_DEPTH + 1` (it cannot overflow the stack), then a serialisation
/// that stops as soon as the compact length exceeds the bound, so a huge document is never fully rendered.
pub fn check_limits(instance: &Value) -> Result<(), LimitViolation> {
    walk_limits(instance, &Path::Root, 0)?;
    let mut counter = Counting::default();
    if serde_json::to_writer(&mut counter, instance).is_err() || counter.bytes > MAX_PARAM_BYTES {
        return Err(violation(
            &Path::Root,
            text!("refusal-limit-document", MAX_PARAM_BYTES.to_string()),
        ));
    }
    Ok(())
}

/// `level` is the number of arrays and objects around `v`, not counting `v`.
fn walk_limits(v: &Value, path: &Path<'_>, level: usize) -> Result<(), LimitViolation> {
    match v {
        Value::String(s) if s.len() > MAX_PARAM_STRING => Err(violation(
            path,
            text!(
                "refusal-limit-string",
                s.len().to_string(),
                MAX_PARAM_STRING.to_string()
            ),
        )),
        Value::Array(items) => {
            let level = level + 1;
            if level > MAX_PARAM_DEPTH {
                return Err(too_deep(path));
            }
            if items.len() > MAX_PARAM_ITEMS {
                return Err(violation(
                    path,
                    text!(
                        "refusal-limit-items",
                        items.len().to_string(),
                        MAX_PARAM_ITEMS.to_string()
                    ),
                ));
            }
            for (i, item) in items.iter().enumerate() {
                walk_limits(item, &Path::Idx(path, i), level)?;
            }
            Ok(())
        }
        Value::Object(members) => {
            let level = level + 1;
            if level > MAX_PARAM_DEPTH {
                return Err(too_deep(path));
            }
            for (key, value) in members {
                let at = Path::Key(path, key);
                if key.len() > MAX_PARAM_STRING {
                    return Err(violation(
                        &at,
                        text!(
                            "refusal-limit-key",
                            key.len().to_string(),
                            MAX_PARAM_STRING.to_string()
                        ),
                    ));
                }
                walk_limits(value, &at, level)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn too_deep(path: &Path<'_>) -> LimitViolation {
    violation(path, text!("refusal-limit-depth", MAX_PARAM_DEPTH.to_string()))
}

/// An `io::Write` that counts bytes and fails once the count exceeds [`MAX_PARAM_BYTES`].
#[derive(Default)]
struct Counting {
    bytes: usize,
}

impl io::Write for Counting {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes = self.bytes.saturating_add(buf.len());
        if self.bytes > MAX_PARAM_BYTES {
            return Err(io::Error::other("document too long"));
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Where a value sits in the instance, as a chain of borrowed steps: building a child costs nothing, the
/// JSON Pointer is only written for the violation that is returned.
#[derive(Clone, Copy)]
enum Path<'a> {
    Root,
    Key(&'a Path<'a>, &'a str),
    Idx(&'a Path<'a>, usize),
}

impl Path<'_> {
    /// The JSON Pointer (`~` is `~0`, `/` is `~1`).
    fn pointer(&self) -> String {
        let mut steps = Vec::new();
        let mut at = self;
        loop {
            match at {
                Path::Root => break,
                Path::Key(parent, key) => {
                    steps.push(Err(*key));
                    at = parent;
                }
                Path::Idx(parent, i) => {
                    steps.push(Ok(*i));
                    at = parent;
                }
            }
        }
        let mut out = String::new();
        for step in steps.into_iter().rev() {
            out.push('/');
            match step {
                Ok(i) => out.push_str(&i.to_string()),
                Err(key) => {
                    for c in key.chars() {
                        match c {
                            '~' => out.push_str("~0"),
                            '/' => out.push_str("~1"),
                            c => out.push(c),
                        }
                    }
                }
            }
        }
        out
    }
}

fn violation(path: &Path<'_>, text: Text) -> LimitViolation {
    LimitViolation {
        path: path.pointer(),
        text,
    }
}
