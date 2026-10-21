// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The functions and the collation the host registers on every consumer connection (doc 17 §6.2):
//! `ew_natural` (X2 < X10), `ew_ml(text, lang)`, and the date and time functions overridden so that they
//! reject `'now'` (results are deterministic, NFR-REL-07).
//!
//! Every function is flagged `SQLITE_DETERMINISTIC | SQLITE_INNOCUOUS`, as `trusted_schema = OFF` requires.

use std::sync::{Arc, Mutex};

use ew_derive::ew_mltext::{LangTag, MlText, natural_cmp};
use rusqlite::functions::{Context, FunctionFlags};
use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, Error};

/// The name of the natural-order collation.
pub const NATURAL: &str = "ew_natural";

/// The date and time functions that are overridden, with the number of leading arguments that must be
/// present so that the function does not fall back to `'now'` (`date()` is `date('now')`, `strftime('%Y')`
/// is `strftime('%Y', 'now')`).
pub(crate) const DATE_FUNCTIONS: [(&str, usize); 7] = [
    ("date", 1),
    ("time", 1),
    ("datetime", 1),
    ("julianday", 1),
    ("unixepoch", 1),
    ("strftime", 2),
    ("timediff", 2),
];

/// The niladic functions behind `CURRENT_DATE`, `CURRENT_TIME` and `CURRENT_TIMESTAMP`: always rejected.
pub(crate) const CURRENT_FUNCTIONS: [&str; 3] = ["current_date", "current_time", "current_timestamp"];

/// Arguments of a date function that read the clock or the host's time zone.
const CLOCK_ARGUMENTS: [&str; 3] = ["now", "localtime", "utc"];

fn flags() -> FunctionFlags {
    FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC | FunctionFlags::SQLITE_INNOCUOUS
}

fn user_error(msg: String) -> Error {
    Error::UserFunctionError(msg.into())
}

/// Registers the collation and the functions on `conn`.
pub(crate) fn register(conn: &Connection) -> rusqlite::Result<()> {
    conn.create_collation(NATURAL, natural_cmp)?;
    conn.create_scalar_function("ew_ml", 2, flags(), ew_ml)?;
    // The built-in date functions run on a private connection that holds no data; the overrides reject the
    // clock and the host time zone before they delegate, so the built-ins never see them.
    let aux = Arc::new(Mutex::new(Connection::open_in_memory()?));
    for (name, required) in DATE_FUNCTIONS {
        let aux = Arc::clone(&aux);
        conn.create_scalar_function(name, -1, flags(), move |ctx| {
            date_function(&aux, name, required, ctx)
        })?;
    }
    for name in CURRENT_FUNCTIONS {
        conn.create_scalar_function(name, 0, flags(), move |_| -> rusqlite::Result<SqlValue> {
            Err(user_error(format!(
                "{} reads the clock and is not available in the projection",
                name.to_uppercase()
            )))
        })?;
    }
    Ok(())
}

/// `ew_ml(<name>_ml, lang)`: the text of a multilingual value in `lang` (BCP 47, with the fallback of
/// [`MlText::text_for`]), else its language-neutral text, else its first language's text; `NULL` for a
/// `NULL` value. An argument that is not the JSON of a multilingual text is an error.
fn ew_ml(ctx: &Context<'_>) -> rusqlite::Result<Option<String>> {
    let Some(json) = ctx.get::<Option<String>>(0)? else {
        return Ok(None);
    };
    let lang = ctx.get::<Option<String>>(1)?;
    let ml: MlText = serde_json::from_str(&json)
        .map_err(|e| user_error(format!("ew_ml: not a multilingual text: {e}")))?;
    let tag = match lang {
        Some(l) => {
            Some(LangTag::parse_lenient(&l).map_err(|e| user_error(format!("ew_ml: language `{l}`: {e}")))?)
        }
        None => None,
    };
    let text = tag
        .as_ref()
        .and_then(|t| ml.text_for(t))
        .or_else(|| ml.as_neutral())
        .or_else(|| ml.languages().next().and_then(|l| ml.get(l)))
        .unwrap_or_default();
    Ok(Some(text.to_owned()))
}

fn date_function(
    aux: &Mutex<Connection>,
    name: &str,
    required: usize,
    ctx: &Context<'_>,
) -> rusqlite::Result<SqlValue> {
    if ctx.len() < required {
        return Err(user_error(format!(
            "{name}() without a time value reads the clock ('now'), which the projection does not allow"
        )));
    }
    let mut args = Vec::with_capacity(ctx.len());
    for i in 0..ctx.len() {
        let v: SqlValue = ctx.get(i)?;
        if let SqlValue::Text(t) = &v {
            let lower = t.trim().to_ascii_lowercase();
            if CLOCK_ARGUMENTS.contains(&lower.as_str()) {
                return Err(user_error(format!(
                    "{name}(): '{lower}' reads the clock or the host time zone, which the projection does not allow"
                )));
            }
        }
        args.push(v);
    }
    let placeholders = (1..=args.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("SELECT {name}({placeholders})");
    let aux = aux
        .lock()
        .map_err(|_| user_error(format!("{name}(): the date helper is poisoned")))?;
    aux.query_row(&sql, rusqlite::params_from_iter(args), |r| {
        r.get::<_, SqlValue>(0)
    })
}
