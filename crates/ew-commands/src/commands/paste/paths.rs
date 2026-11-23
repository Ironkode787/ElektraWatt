// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The JSON paths of a fragment record's links: every reference at a link's field, the value at a path, and a
//! reference cleared by the link's rule.

use super::*;

/// One step of a concrete path into a record's JSON.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum Step {
    /// An object member.
    Key(String),
    /// An element of a list walked as `name[]`.
    Each(usize),
    /// A fixed element, `name[n]` (the function of a CP endpoint).
    At(usize),
}

/// One segment of a link's field path: the member name and `[]` (`Some(None)`) or `[n]` (`Some(Some(n))`).
pub(super) fn segments(field: &str) -> Vec<(&str, Option<Option<usize>>)> {
    field
        .split('.')
        .map(|s| match s.split_once('[') {
            None => (s, None),
            Some((name, rest)) => {
                let inner = rest.trim_end_matches(']');
                (name, Some(inner.parse::<usize>().ok()))
            }
        })
        .collect()
}

/// Every UUID string at `field` (a `LINKS` path) in `v`, with its concrete path.
pub(crate) fn occurrences(v: &Value, field: &str) -> Vec<(Vec<Step>, Uuid)> {
    fn walk(
        v: &Value,
        segs: &[(&str, Option<Option<usize>>)],
        path: &mut Vec<Step>,
        out: &mut Vec<(Vec<Step>, Uuid)>,
    ) {
        let Some(((name, index), rest)) = segs.split_first() else {
            if let Some(u) = v.as_str().and_then(|s| s.parse::<Id<()>>().ok()) {
                out.push((path.clone(), u.as_uuid()));
            }
            return;
        };
        let Some(child) = v.as_object().and_then(|o| o.get(*name)) else {
            return;
        };
        path.push(Step::Key((*name).to_owned()));
        match index {
            None => walk(child, rest, path, out),
            Some(None) => {
                for (i, c) in child.as_array().into_iter().flatten().enumerate() {
                    path.push(Step::Each(i));
                    walk(c, rest, path, out);
                    path.pop();
                }
            }
            Some(Some(n)) => {
                if let Some(c) = child.as_array().and_then(|a| a.get(*n)) {
                    path.push(Step::At(*n));
                    walk(c, rest, path, out);
                    path.pop();
                }
            }
        }
        path.pop();
    }
    let mut out = Vec::new();
    walk(v, &segments(field), &mut Vec::new(), &mut out);
    out
}

/// The value at `path`.
pub(crate) fn at_path<'v>(v: &'v mut Value, path: &[Step]) -> Option<&'v mut Value> {
    let mut cur = v;
    for s in path {
        cur = match s {
            Step::Key(k) => cur.as_object_mut()?.get_mut(k)?,
            Step::Each(i) | Step::At(i) => cur.as_array_mut()?.get_mut(*i)?,
        };
    }
    Some(cur)
}

/// Removes the member or element at `path`; `false` when there is none.
pub(super) fn remove_at(v: &mut Value, path: &[Step]) -> bool {
    let Some((last, parent)) = path.split_last() else {
        return false;
    };
    let Some(container) = at_path(v, parent) else {
        return false;
    };
    match last {
        Step::Key(k) => container.as_object_mut().and_then(|o| o.remove(k)).is_some(),
        Step::Each(i) | Step::At(i) => match container.as_array_mut() {
            Some(a) if *i < a.len() => {
                a.remove(*i);
                true
            }
            _ => false,
        },
    }
}

/// Whether `v` reads as a record.
pub(super) fn reads(v: &Value) -> bool {
    Record::from_value(v.clone()).is_ok()
}

/// Clears the reference at `path` of record `v`: the list element it is (`name[]`), else the
/// member holding it (a CP endpoint's `cp` for `name[n]`), else the object member directly around that member
/// when that object is not the record itself (`overrides.wire_part` for `overrides.wire_part.copy`); the first
/// removal after which the record still reads. `None`: the reference is mandatory.
pub(crate) fn cleared(v: &Value, path: &[Step]) -> Option<Value> {
    let target: &[Step] = match path.last() {
        Some(Step::At(_)) => &path[..path.len() - 1],
        _ => path,
    };
    let mut first = v.clone();
    if remove_at(&mut first, target) && reads(&first) {
        return Some(first);
    }
    // The object member around a member (never around a list element, never a member of the record itself).
    if let [.., Step::Key(_), Step::Key(_)] = target
        && target.len() >= 3
    {
        let around = &target[..target.len() - 1];
        let mut second = v.clone();
        if remove_at(&mut second, around) && reads(&second) {
            return Some(second);
        }
    }
    None
}

/// Whether a link is mandatory in a record, by the rule of [`cleared`] applied to `v` and the link's first
/// occurrence (`None` when the record does not hold the link).
#[cfg(test)]
pub(super) fn mandatory_in(decl: &LinkDecl, v: &Value) -> Option<bool> {
    let (path, _) = occurrences(v, decl.field).into_iter().next()?;
    Some(decl.policy == Policy::Cascade || cleared(v, &path).is_none())
}
