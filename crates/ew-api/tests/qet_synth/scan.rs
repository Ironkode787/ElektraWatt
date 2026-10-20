// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A tiny scanner for the XML the fixture builder writes: start and empty tags with their attributes and
//! the nesting, which is all the well-formedness checks of `tests/qet_import.rs` need. It is not a general
//! XML parser: it knows the builder's escaping (`&amp; &lt; &gt; &quot;`, no `>` or `"` inside a value) and
//! no comments, CDATA or DTDs.

use std::collections::BTreeMap;

/// One start tag or empty-element tag.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Tag {
    /// The element name.
    pub name: String,
    /// The attributes (values as written, still escaped).
    pub attrs: BTreeMap<String, String>,
    /// The index (in the scan's list) of the enclosing element.
    pub parent: Option<usize>,
}

impl Tag {
    /// The attribute `name`, or `""`.
    pub fn attr(&self, name: &str) -> &str {
        self.attrs.get(name).map_or("", String::as_str)
    }
}

/// The tags of `text` in document order.
///
/// # Errors
///
/// The position and reason when a tag is malformed or the nesting does not balance.
pub fn scan(text: &str) -> Result<Vec<Tag>, String> {
    let bytes = text.as_bytes();
    let mut tags: Vec<Tag> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        if text[i..].starts_with("<?") {
            let end = text[i..].find("?>").ok_or("unterminated `<?`")?;
            i += end + 2;
            continue;
        }
        if text[i..].starts_with("</") {
            let end = text[i..].find('>').ok_or("unterminated end tag")?;
            let name = text[i + 2..i + end].trim();
            let open = stack
                .pop()
                .ok_or_else(|| format!("end tag </{name}> without a start tag"))?;
            if tags[open].name != name {
                return Err(format!("</{name}> closes <{}>", tags[open].name));
            }
            i += end + 1;
            continue;
        }
        // A start or empty-element tag.
        let mut j = i + 1;
        while j < bytes.len() && !bytes[j].is_ascii_whitespace() && bytes[j] != b'/' && bytes[j] != b'>' {
            j += 1;
        }
        let name = text[i + 1..j].to_owned();
        if name.is_empty() {
            return Err(format!("empty tag name at byte {i}"));
        }
        let mut attrs = BTreeMap::new();
        let empty;
        loop {
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            match bytes.get(j) {
                Some(b'>') => {
                    empty = false;
                    j += 1;
                    break;
                }
                Some(b'/') => {
                    if bytes.get(j + 1) != Some(&b'>') {
                        return Err(format!("`/` not followed by `>` in <{name}>"));
                    }
                    empty = true;
                    j += 2;
                    break;
                }
                Some(_) => {
                    let start = j;
                    while j < bytes.len() && bytes[j] != b'=' && !bytes[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    let key = text[start..j].to_owned();
                    if bytes.get(j) != Some(&b'=') || bytes.get(j + 1) != Some(&b'"') {
                        return Err(format!("attribute `{key}` of <{name}> is not `=\"…\"`"));
                    }
                    let vstart = j + 2;
                    let vlen = text[vstart..]
                        .find('"')
                        .ok_or_else(|| format!("unterminated value of `{key}` in <{name}>"))?;
                    if attrs
                        .insert(key.clone(), text[vstart..vstart + vlen].to_owned())
                        .is_some()
                    {
                        return Err(format!("attribute `{key}` twice in <{name}>"));
                    }
                    j = vstart + vlen + 1;
                }
                None => return Err(format!("unterminated tag <{name}>")),
            }
        }
        tags.push(Tag {
            name,
            attrs,
            parent: stack.last().copied(),
        });
        if !empty {
            stack.push(tags.len() - 1);
        }
        i = j;
    }
    if let Some(open) = stack.last() {
        return Err(format!("<{}> is never closed", tags[*open].name));
    }
    Ok(tags)
}

/// A UUID as written (`{…}`, any case) in lower case without braces; `None` when it is not one.
pub fn norm_uuid(s: &str) -> Option<String> {
    let t = s.strip_prefix('{')?.strip_suffix('}')?;
    let b = t.as_bytes();
    let ok = b.len() == 36
        && b.iter().enumerate().all(|(i, c)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                *c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        });
    ok.then(|| t.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_nesting_and_attributes() {
        let tags = scan(r#"<?xml version="1.0"?><a x="1"><b/><c y="&amp;">t</c></a>"#).unwrap();
        let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["a", "b", "c"]);
        assert_eq!(tags[2].attr("y"), "&amp;");
        assert_eq!(tags[2].parent, Some(0));
        assert!(scan("<a><b></a>").is_err());
        assert!(scan("<a>").is_err());
    }
}
