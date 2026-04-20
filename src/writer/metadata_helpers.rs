//! Surgical edits on the SmartPlant `/TaggedTxtData/Drawing` and
//! `/TaggedTxtData/General` XML payloads.
//!
//! Why byte-level instead of a full XML re-write?
//!
//! - SmartPlant readers can be picky about whitespace, attribute quote
//!   style, and namespace ordering. A `quick_xml::Reader` → `Writer`
//!   round-trip would normalize all of that and introduce visual diffs
//!   for every save, even when the user edited a single field.
//! - The two streams these helpers target use a flat schema: simple
//!   `<Tag attr="value"/>` lines for Drawing, and short `<Element>text</Element>`
//!   lines for General. A targeted regex-style scan is enough.
//! - Failure modes are explicit (attribute not found, duplicates, etc.)
//!   so callers get actionable errors instead of "succeeded but didn't
//!   actually change anything".
//!
//! All public helpers return `Result<String, MetadataEditError>`. The
//! returned `String` is always the original `xml` with the targeted
//! region spliced; every byte outside that region is preserved verbatim.

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MetadataEditError {
    #[error("attribute `{attr}` not found in XML")]
    AttributeNotFound { attr: String },
    #[error(
        "attribute `{attr}` appears {count} times; refusing to edit ambiguously (callers should narrow the target first)"
    )]
    DuplicateAttribute { attr: String, count: usize },
    #[error("attribute `{attr}` value is not properly quoted")]
    UnterminatedAttribute { attr: String },
    #[error("element `{element}` not found in XML")]
    ElementNotFound { element: String },
    #[error("element `{element}` appears {count} times; refusing to edit ambiguously")]
    DuplicateElement { element: String, count: usize },
    #[error("element `{element}` is malformed (missing closing tag or invalid structure)")]
    MalformedElement { element: String },
}

/// Replace the value of an XML attribute, leaving every other byte
/// (whitespace, quote style, sibling attributes, comments) untouched.
///
/// The attribute is found via a `name="value"` byte scan that requires
/// the previous byte to be ASCII whitespace (so `MY_ATTR` doesn't match
/// inside `EXTRA_MY_ATTR`). `new_value` is XML-escaped before insertion
/// — callers pass plain strings, the helper handles `&`, `<`, `>`, `"`.
///
/// Errors:
/// - [`MetadataEditError::AttributeNotFound`] if no `attr="…"` occurs.
/// - [`MetadataEditError::DuplicateAttribute`] if two or more matches
///   are found (caller should resolve the ambiguity).
/// - [`MetadataEditError::UnterminatedAttribute`] if a match starts but
///   the closing `"` is missing.
pub fn set_drawing_attribute(
    xml: &str,
    attr: &str,
    new_value: &str,
) -> Result<String, MetadataEditError> {
    let matches = find_attribute_value_ranges(xml, attr);
    match matches.len() {
        0 => Err(MetadataEditError::AttributeNotFound { attr: attr.into() }),
        1 => {
            let (start, end) = matches[0];
            let mut out = String::with_capacity(xml.len() + new_value.len());
            out.push_str(&xml[..start]);
            out.push_str(&xml_escape_attribute_value(new_value));
            out.push_str(&xml[end..]);
            Ok(out)
        }
        n => Err(MetadataEditError::DuplicateAttribute {
            attr: attr.into(),
            count: n,
        }),
    }
}

/// Convenience over [`set_drawing_attribute`]: replaces
/// `SP_DRAWINGNUMBER`. Equivalent to
/// `set_drawing_attribute(xml, "SP_DRAWINGNUMBER", value)`.
pub fn set_drawing_number(xml: &str, value: &str) -> Result<String, MetadataEditError> {
    set_drawing_attribute(xml, "SP_DRAWINGNUMBER", value)
}

/// Read-only counterpart of [`set_drawing_attribute`]: return the raw
/// (un-unescaped) attribute value if `attr` appears exactly once,
/// otherwise `None`.
///
/// "Exactly once" mirrors the writer's policy — duplicate matches make
/// edits ambiguous, and silently returning the first hit would mask
/// that ambiguity from the read side too. Callers that need the
/// "duplicate found" detail should treat `None` plus a follow-up
/// `set_drawing_attribute` error as the canonical signal.
pub fn get_drawing_attribute(xml: &str, attr: &str) -> Option<String> {
    let matches = find_attribute_value_ranges(xml, attr);
    if matches.len() == 1 {
        let (start, end) = matches[0];
        Some(xml[start..end].to_string())
    } else {
        None
    }
}

/// Read-only counterpart of [`set_element_text`]: return the raw
/// (un-unescaped) text content of `<element>…</element>` if the element
/// appears exactly once with a regular open/close pair, otherwise
/// `None`.
///
/// Self-closing tags (`<element/>`) and any malformed structure return
/// `None` rather than an error — this read API is meant to be a
/// best-effort lookup; callers that need diagnostic detail should fall
/// through to [`set_element_text`] which surfaces the typed
/// [`MetadataEditError`].
pub fn get_general_element_text(xml: &str, element: &str) -> Option<String> {
    let matches = find_element_text_ranges(xml, element).ok()?;
    if matches.len() == 1 {
        let (start, end) = matches[0];
        Some(xml[start..end].to_string())
    } else {
        None
    }
}

/// Bulk read: every `(attr_name, attr_value)` pair that appears on any
/// open tag in the document, in source order. Duplicates are kept; the
/// writer rejects edits to ambiguous attrs but readers should see
/// everything.
///
/// Skips XML processing instructions (`<?…?>`), comments (`<!--…-->`),
/// CDATA (`<![…]]>`), and closing tags (`</…>`). Does not deep-parse
/// CDATA bodies or DTD content; raw byte scan keeps the helper aligned
/// with the rest of the module.
pub fn list_drawing_attributes(xml: &str) -> Vec<(String, String)> {
    let bytes = xml.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        // Skip PI / comment / CDATA / closing-tag prefixes.
        if let Some(skip_to) = skip_special_tag(bytes, i) {
            i = skip_to;
            continue;
        }
        if i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            // closing tag — find '>' and move past
            i += 2;
            while i < bytes.len() && bytes[i] != b'>' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
            continue;
        }
        // Open tag: skip the tag name, then scan attributes until `>` or `/>`.
        i += 1;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && bytes[i] != b'>'
            && bytes[i] != b'/'
        {
            i += 1;
        }
        // Now i is at whitespace, '>', or '/'. Scan attributes.
        while i < bytes.len() && bytes[i] != b'>' {
            if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'>' {
                break;
            }
            if bytes[i].is_ascii_whitespace() {
                i += 1;
                continue;
            }
            // Read attribute name: chars until '=' or whitespace
            let name_start = i;
            while i < bytes.len()
                && bytes[i] != b'='
                && !bytes[i].is_ascii_whitespace()
                && bytes[i] != b'>'
                && bytes[i] != b'/'
            {
                i += 1;
            }
            let name_end = i;
            // Skip whitespace + '='
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= bytes.len() || bytes[i] != b'=' {
                // Attribute without value (rare in PID); skip.
                continue;
            }
            i += 1; // past '='
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= bytes.len() || bytes[i] != b'"' {
                // Single-quoted or unquoted attrs: not in our scope; skip.
                continue;
            }
            i += 1; // past opening '"'
            let value_start = i;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }
            if i >= bytes.len() {
                break;
            }
            let value_end = i;
            i += 1; // past closing '"'
            let name = std::str::from_utf8(&bytes[name_start..name_end])
                .unwrap_or("")
                .to_string();
            let value = std::str::from_utf8(&bytes[value_start..value_end])
                .unwrap_or("")
                .to_string();
            if !name.is_empty() {
                out.push((name, value));
            }
        }
        // Step past `>` or `/>`
        if i < bytes.len() && bytes[i] == b'/' {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'>' {
            i += 1;
        }
    }
    out
}

/// Bulk read: every `<element>text</element>` pair whose inner text
/// contains no nested elements. Self-closing tags are skipped; elements
/// with mixed content (text + child elements) are walked into so leaf
/// children still appear (e.g. `<General><FilePath>x</FilePath></General>`
/// returns `[("FilePath", "x")]` and skips the outer `General`).
/// Returned in source order.
///
/// Same skip rules for PI / comment / CDATA prefixes as
/// [`list_drawing_attributes`].
pub fn list_general_elements(xml: &str) -> Vec<(String, String)> {
    let bytes = xml.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        if let Some(skip_to) = skip_special_tag(bytes, i) {
            i = skip_to;
            continue;
        }
        if i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            // Closing tag — just step past `>` so the outer loop can
            // continue scanning siblings.
            i += 2;
            while i < bytes.len() && bytes[i] != b'>' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
            continue;
        }
        // Open tag — extract name.
        let name_start = i + 1;
        let mut j = name_start;
        while j < bytes.len()
            && !bytes[j].is_ascii_whitespace()
            && bytes[j] != b'>'
            && bytes[j] != b'/'
        {
            j += 1;
        }
        let name_end = j;
        let name = match std::str::from_utf8(&bytes[name_start..name_end]) {
            Ok(s) if !s.is_empty() => s.to_string(),
            _ => {
                i += 1;
                continue;
            }
        };
        // Find end of open tag, deciding self-closing vs. paired.
        while j < bytes.len() && bytes[j] != b'>' {
            j += 1;
        }
        if j >= bytes.len() {
            break;
        }
        let self_closing = j > name_end && bytes[j - 1] == b'/';
        let after_open = j + 1; // position past '>'
        if self_closing {
            i = after_open;
            continue;
        }
        // Peek the inner content up to the next `<`. If that `<` opens
        // exactly `</name>`, this is a leaf-text element — record it
        // and jump past the close tag. Otherwise we let the outer loop
        // walk into the children (i.e. step past only the open tag).
        let mut k = after_open;
        while k < bytes.len() && bytes[k] != b'<' {
            k += 1;
        }
        if k >= bytes.len() {
            break;
        }
        let close_marker = format!("</{}", name);
        if bytes[k..].starts_with(close_marker.as_bytes()) {
            let after_marker = k + close_marker.len();
            let mut m = after_marker;
            while m < bytes.len() && bytes[m].is_ascii_whitespace() {
                m += 1;
            }
            if m < bytes.len() && bytes[m] == b'>' {
                let text = std::str::from_utf8(&bytes[after_open..k])
                    .unwrap_or("")
                    .to_string();
                out.push((name, text));
                i = m + 1;
                continue;
            }
        }
        // Either nested children or unmatched close — descend by just
        // skipping the open tag.
        i = after_open;
    }
    out
}

/// Detect and skip XML processing instructions, comments, and CDATA
/// markers. Returns the new cursor position past the special tag, or
/// `None` if no special tag started at `i`.
fn skip_special_tag(bytes: &[u8], i: usize) -> Option<usize> {
    if i + 1 >= bytes.len() || bytes[i] != b'<' {
        return None;
    }
    let next = bytes[i + 1];
    if next == b'?' {
        // <?xml … ?> — find ?>
        let mut j = i + 2;
        while j + 1 < bytes.len() && !(bytes[j] == b'?' && bytes[j + 1] == b'>') {
            j += 1;
        }
        return Some(if j + 2 <= bytes.len() {
            j + 2
        } else {
            bytes.len()
        });
    }
    if next == b'!' {
        // <!-- … --> or <![CDATA[…]]> or <!DOCTYPE …>
        if bytes[i..].starts_with(b"<!--") {
            let mut j = i + 4;
            while j + 2 < bytes.len() && &bytes[j..j + 3] != b"-->" {
                j += 1;
            }
            return Some(if j + 3 <= bytes.len() {
                j + 3
            } else {
                bytes.len()
            });
        }
        if bytes[i..].starts_with(b"<![CDATA[") {
            let mut j = i + 9;
            while j + 2 < bytes.len() && &bytes[j..j + 3] != b"]]>" {
                j += 1;
            }
            return Some(if j + 3 <= bytes.len() {
                j + 3
            } else {
                bytes.len()
            });
        }
        // Generic <! … > (DOCTYPE etc.)
        let mut j = i + 2;
        while j < bytes.len() && bytes[j] != b'>' {
            j += 1;
        }
        return Some(if j < bytes.len() { j + 1 } else { bytes.len() });
    }
    None
}

/// Replace the text content of an element, e.g. turn
/// `<FilePath>C:/old.pid</FilePath>` into
/// `<FilePath>D:/new.pid</FilePath>`. Self-closing elements
/// (`<FilePath/>`) are detected and rejected as
/// [`MetadataEditError::MalformedElement`] because they have no text
/// region to replace.
///
/// `new_text` is XML-escaped before insertion (handles `<`, `>`, `&`).
pub fn set_element_text(
    xml: &str,
    element: &str,
    new_text: &str,
) -> Result<String, MetadataEditError> {
    let matches = find_element_text_ranges(xml, element)?;
    match matches.len() {
        0 => Err(MetadataEditError::ElementNotFound {
            element: element.into(),
        }),
        1 => {
            let (start, end) = matches[0];
            let mut out = String::with_capacity(xml.len() + new_text.len());
            out.push_str(&xml[..start]);
            out.push_str(&xml_escape_text(new_text));
            out.push_str(&xml[end..]);
            Ok(out)
        }
        n => Err(MetadataEditError::DuplicateElement {
            element: element.into(),
            count: n,
        }),
    }
}

/// Convenience: try `<FilePath>` first (the canonical SmartPlant tag),
/// then `<Path>` as fallback (some older General XMLs use that name —
/// see `parsers/general_xml.rs`).
pub fn set_general_file_path(xml: &str, value: &str) -> Result<String, MetadataEditError> {
    match set_element_text(xml, "FilePath", value) {
        Ok(out) => Ok(out),
        Err(MetadataEditError::ElementNotFound { .. }) => set_element_text(xml, "Path", value),
        Err(other) => Err(other),
    }
}

// ── internals ────────────────────────────────────────────────────────────

/// Scan `xml` for occurrences of `attr="…"` and return the
/// `(value_start, value_end)` byte ranges (within the original string)
/// for each match. The bounds point at the *value* slice, NOT including
/// the surrounding double quotes.
fn find_attribute_value_ranges(xml: &str, attr: &str) -> Vec<(usize, usize)> {
    let bytes = xml.as_bytes();
    let attr_bytes = attr.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;

    while i + attr_bytes.len() < bytes.len() {
        let candidate = &bytes[i..];
        if candidate.starts_with(attr_bytes) {
            // The byte just before `i` must be either start-of-buffer or
            // ASCII whitespace, so we don't match the tail of a longer
            // identifier (e.g. `MY_ATTR` inside `EXTRA_MY_ATTR`).
            let left_ok = i == 0 || bytes[i - 1].is_ascii_whitespace();
            // The byte just after the name must be `=` or whitespace
            // followed by `=`.
            let after = i + attr_bytes.len();
            let right_ok = after < bytes.len()
                && (bytes[after] == b'='
                    || (bytes[after].is_ascii_whitespace()
                        && find_next_equals(bytes, after).is_some()));
            if left_ok && right_ok {
                if let Some(eq) = find_next_equals(bytes, after) {
                    if let Some(quote) = find_next_quote(bytes, eq + 1) {
                        if let Some(end_quote) = find_unescaped_quote(bytes, quote + 1) {
                            out.push((quote + 1, end_quote));
                            i = end_quote + 1;
                            continue;
                        }
                    }
                }
            }
        }
        i += 1;
    }
    out
}

fn find_next_equals(bytes: &[u8], from: usize) -> Option<usize> {
    let mut j = from;
    while j < bytes.len() && bytes[j].is_ascii_whitespace() {
        j += 1;
    }
    if j < bytes.len() && bytes[j] == b'=' {
        Some(j)
    } else {
        None
    }
}

fn find_next_quote(bytes: &[u8], from: usize) -> Option<usize> {
    let mut j = from;
    while j < bytes.len() && bytes[j].is_ascii_whitespace() {
        j += 1;
    }
    if j < bytes.len() && bytes[j] == b'"' {
        Some(j)
    } else {
        None
    }
}

/// Find the next unescaped `"` after `from`. SmartPlant XML uses XML
/// entity escapes (`&quot;`) inside attribute values, never a literal
/// `\"` C-style escape, so a plain byte scan is correct.
fn find_unescaped_quote(bytes: &[u8], from: usize) -> Option<usize> {
    let mut j = from;
    while j < bytes.len() {
        if bytes[j] == b'"' {
            return Some(j);
        }
        j += 1;
    }
    None
}

/// Locate `<element …>text</element>` regions; return the byte range of
/// the text content (without the surrounding tags). Self-closing
/// `<element/>` triggers `MalformedElement` because there's nothing to
/// rewrite.
fn find_element_text_ranges(
    xml: &str,
    element: &str,
) -> Result<Vec<(usize, usize)>, MetadataEditError> {
    let bytes = xml.as_bytes();
    let open_prefix = format!("<{}", element);
    let open_bytes = open_prefix.as_bytes();
    let close_tag = format!("</{}>", element);
    let close_bytes = close_tag.as_bytes();

    let mut out = Vec::new();
    let mut i = 0;
    while i + open_bytes.len() < bytes.len() {
        if bytes[i..].starts_with(open_bytes) {
            // The byte just after `<element` must be `>`, whitespace,
            // or `/` — otherwise this matches `<elementX>` etc.
            let after = i + open_bytes.len();
            if after >= bytes.len() {
                break;
            }
            let next = bytes[after];
            if next == b'>' {
                let text_start = after + 1;
                let close_at =
                    find_subsequence(bytes, close_bytes, text_start).ok_or_else(|| {
                        MetadataEditError::MalformedElement {
                            element: element.into(),
                        }
                    })?;
                out.push((text_start, close_at));
                i = close_at + close_bytes.len();
                continue;
            }
            if next.is_ascii_whitespace() {
                // Skip attributes until `>` or `/>`.
                let mut j = after;
                while j < bytes.len()
                    && bytes[j] != b'>'
                    && !(bytes[j] == b'/' && j + 1 < bytes.len() && bytes[j + 1] == b'>')
                {
                    j += 1;
                }
                if j >= bytes.len() {
                    return Err(MetadataEditError::MalformedElement {
                        element: element.into(),
                    });
                }
                if bytes[j] == b'/' {
                    return Err(MetadataEditError::MalformedElement {
                        element: element.into(),
                    });
                }
                let text_start = j + 1;
                let close_at =
                    find_subsequence(bytes, close_bytes, text_start).ok_or_else(|| {
                        MetadataEditError::MalformedElement {
                            element: element.into(),
                        }
                    })?;
                out.push((text_start, close_at));
                i = close_at + close_bytes.len();
                continue;
            }
            if next == b'/' {
                return Err(MetadataEditError::MalformedElement {
                    element: element.into(),
                });
            }
        }
        i += 1;
    }
    Ok(out)
}

fn find_subsequence(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from >= haystack.len() {
        return None;
    }
    let mut i = from;
    while i + needle.len() <= haystack.len() {
        if &haystack[i..i + needle.len()] == needle {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn xml_escape_attribute_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            // Apostrophe is fine inside double-quoted attributes but we
            // escape it for safety in case a tool re-quotes the source.
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

fn xml_escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_drawing_number_replaces_value_in_simple_tag() {
        let xml = r#"<?xml version="1.0"?><Drawing><Tag SP_DRAWINGNUMBER="OLD-001"/></Drawing>"#;
        let out = set_drawing_number(xml, "NEW-007").unwrap();
        assert_eq!(
            out,
            r#"<?xml version="1.0"?><Drawing><Tag SP_DRAWINGNUMBER="NEW-007"/></Drawing>"#
        );
    }

    #[test]
    fn set_drawing_attribute_preserves_other_attributes_and_whitespace() {
        let xml = r#"<Tag SP_FOO = "x"   SP_DRAWINGNUMBER = "OLD"  SP_BAR="y"/>"#;
        let out = set_drawing_attribute(xml, "SP_DRAWINGNUMBER", "NEW").unwrap();
        assert_eq!(
            out,
            r#"<Tag SP_FOO = "x"   SP_DRAWINGNUMBER = "NEW"  SP_BAR="y"/>"#
        );
    }

    #[test]
    fn set_drawing_attribute_does_not_match_suffix_of_longer_name() {
        let xml = r#"<Tag MY_DRAWINGNUMBER="A" SP_DRAWINGNUMBER="B"/>"#;
        let out = set_drawing_attribute(xml, "SP_DRAWINGNUMBER", "Z").unwrap();
        assert_eq!(out, r#"<Tag MY_DRAWINGNUMBER="A" SP_DRAWINGNUMBER="Z"/>"#);
    }

    #[test]
    fn set_drawing_attribute_returns_not_found_when_missing() {
        let xml = r#"<Tag SP_OTHER="x"/>"#;
        let err = set_drawing_attribute(xml, "SP_DRAWINGNUMBER", "v").unwrap_err();
        assert_eq!(
            err,
            MetadataEditError::AttributeNotFound {
                attr: "SP_DRAWINGNUMBER".into()
            }
        );
    }

    #[test]
    fn set_drawing_attribute_returns_duplicate_when_appearing_twice() {
        let xml = r#"<Tag SP_X="1"/><Tag SP_X="2"/>"#;
        let err = set_drawing_attribute(xml, "SP_X", "z").unwrap_err();
        assert_eq!(
            err,
            MetadataEditError::DuplicateAttribute {
                attr: "SP_X".into(),
                count: 2,
            }
        );
    }

    #[test]
    fn set_drawing_attribute_xml_escapes_special_chars_in_value() {
        let xml = r#"<Tag SP_NAME="OLD"/>"#;
        let out = set_drawing_attribute(xml, "SP_NAME", r#"A&B<C>"D'"#).unwrap();
        assert_eq!(out, r#"<Tag SP_NAME="A&amp;B&lt;C&gt;&quot;D&apos;"/>"#);
    }

    #[test]
    fn set_drawing_attribute_supports_empty_replacement_value() {
        let xml = r#"<Tag SP_X="something"/>"#;
        let out = set_drawing_attribute(xml, "SP_X", "").unwrap();
        assert_eq!(out, r#"<Tag SP_X=""/>"#);
    }

    #[test]
    fn set_drawing_attribute_handles_unicode_value() {
        let xml = r#"<Tag SP_TITLE="OLD"/>"#;
        let out = set_drawing_attribute(xml, "SP_TITLE", "中文图号 №7").unwrap();
        assert_eq!(out, r#"<Tag SP_TITLE="中文图号 №7"/>"#);
    }

    #[test]
    fn set_element_text_replaces_simple_text_content() {
        let xml = r#"<General><FilePath>C:/old.pid</FilePath></General>"#;
        let out = set_element_text(xml, "FilePath", "D:/new.pid").unwrap();
        assert_eq!(out, r#"<General><FilePath>D:/new.pid</FilePath></General>"#);
    }

    #[test]
    fn set_element_text_handles_element_with_attributes() {
        let xml = r#"<General><FilePath kind="abs">C:/old</FilePath></General>"#;
        let out = set_element_text(xml, "FilePath", "C:/new").unwrap();
        assert_eq!(
            out,
            r#"<General><FilePath kind="abs">C:/new</FilePath></General>"#
        );
    }

    #[test]
    fn set_element_text_rejects_self_closing_tag() {
        let xml = r#"<General><FilePath/></General>"#;
        let err = set_element_text(xml, "FilePath", "C:/x").unwrap_err();
        assert_eq!(
            err,
            MetadataEditError::MalformedElement {
                element: "FilePath".into()
            }
        );
    }

    #[test]
    fn set_element_text_returns_not_found_when_missing() {
        let xml = r#"<General><Other>x</Other></General>"#;
        let err = set_element_text(xml, "FilePath", "v").unwrap_err();
        assert_eq!(
            err,
            MetadataEditError::ElementNotFound {
                element: "FilePath".into()
            }
        );
    }

    #[test]
    fn set_element_text_xml_escapes_special_chars() {
        let xml = r#"<E>OLD</E>"#;
        let out = set_element_text(xml, "E", "A&B<C>").unwrap();
        assert_eq!(out, r#"<E>A&amp;B&lt;C&gt;</E>"#);
    }

    #[test]
    fn set_element_text_does_not_match_longer_element_name() {
        // `<FilePathExt>` must not match `FilePath` lookup.
        let xml = r#"<General><FilePathExt>X</FilePathExt><FilePath>Y</FilePath></General>"#;
        let out = set_element_text(xml, "FilePath", "Z").unwrap();
        assert_eq!(
            out,
            r#"<General><FilePathExt>X</FilePathExt><FilePath>Z</FilePath></General>"#
        );
    }

    #[test]
    fn set_general_file_path_falls_back_from_filepath_to_path() {
        let xml = r#"<General><Path>old</Path></General>"#;
        let out = set_general_file_path(xml, "new").unwrap();
        assert_eq!(out, r#"<General><Path>new</Path></General>"#);
    }

    #[test]
    fn set_general_file_path_prefers_filepath_when_both_exist() {
        // Spec: `FilePath` is canonical, `Path` is fallback. If
        // `FilePath` appears, edit it and leave `Path` alone.
        let xml = r#"<General><Path>P</Path><FilePath>F</FilePath></General>"#;
        let out = set_general_file_path(xml, "new").unwrap();
        assert_eq!(
            out,
            r#"<General><Path>P</Path><FilePath>new</FilePath></General>"#
        );
    }

    #[test]
    fn get_drawing_attribute_returns_value_for_single_match() {
        let xml = r#"<Tag SP_DRAWINGNUMBER="DWG-007"/>"#;
        assert_eq!(
            get_drawing_attribute(xml, "SP_DRAWINGNUMBER").as_deref(),
            Some("DWG-007")
        );
    }

    #[test]
    fn get_drawing_attribute_returns_none_when_missing() {
        let xml = r#"<Tag SP_OTHER="x"/>"#;
        assert_eq!(get_drawing_attribute(xml, "SP_DRAWINGNUMBER"), None);
    }

    #[test]
    fn get_drawing_attribute_returns_none_when_duplicate() {
        let xml = r#"<Tag SP_X="A"/><Tag SP_X="B"/>"#;
        assert_eq!(
            get_drawing_attribute(xml, "SP_X"),
            None,
            "duplicates should mirror the writer's ambiguity rejection"
        );
    }

    #[test]
    fn get_general_element_text_returns_text_for_single_match() {
        let xml = r#"<General><FilePath>C:/x.pid</FilePath></General>"#;
        assert_eq!(
            get_general_element_text(xml, "FilePath").as_deref(),
            Some("C:/x.pid")
        );
    }

    #[test]
    fn get_general_element_text_returns_none_when_missing() {
        let xml = r#"<General><Other>x</Other></General>"#;
        assert_eq!(get_general_element_text(xml, "FilePath"), None);
    }

    #[test]
    fn get_general_element_text_returns_none_for_self_closing() {
        let xml = r#"<General><FilePath/></General>"#;
        assert_eq!(
            get_general_element_text(xml, "FilePath"),
            None,
            "self-closing has no text region; should return None instead of erroring"
        );
    }

    #[test]
    fn list_drawing_attributes_returns_pairs_in_document_order() {
        let xml = r#"<?xml version="1.0"?>
<!--header comment-->
<Drawing>
  <Tag SP_DRAWINGNUMBER="FX-001" SP_PROJECTNUMBER="PRJ-A"/>
  <Tag SP_REVISION="2"/>
</Drawing>"#;
        let pairs = list_drawing_attributes(xml);
        assert_eq!(
            pairs,
            vec![
                ("SP_DRAWINGNUMBER".to_string(), "FX-001".to_string()),
                ("SP_PROJECTNUMBER".to_string(), "PRJ-A".to_string()),
                ("SP_REVISION".to_string(), "2".to_string()),
            ]
        );
    }

    #[test]
    fn list_drawing_attributes_returns_empty_for_empty_xml() {
        assert!(list_drawing_attributes("").is_empty());
        assert!(list_drawing_attributes("<Drawing/>").is_empty());
    }

    #[test]
    fn list_general_elements_returns_pairs_for_simple_xml() {
        let xml = r#"<?xml version="1.0"?>
<General>
  <FilePath>C:/x.pid</FilePath>
  <FileSize>2048</FileSize>
  <Author>OLD</Author>
</General>"#;
        let pairs = list_general_elements(xml);
        // Note: <General> itself is also an open/close pair, but its
        // inner contains nested children → skipped.
        assert_eq!(
            pairs,
            vec![
                ("FilePath".to_string(), "C:/x.pid".to_string()),
                ("FileSize".to_string(), "2048".to_string()),
                ("Author".to_string(), "OLD".to_string()),
            ]
        );
    }

    #[test]
    fn list_general_elements_skips_self_closing() {
        let xml = r#"<General><A>hello</A><B/><C>world</C></General>"#;
        let pairs = list_general_elements(xml);
        assert_eq!(
            pairs,
            vec![
                ("A".to_string(), "hello".to_string()),
                ("C".to_string(), "world".to_string()),
            ]
        );
    }

    #[test]
    fn list_general_elements_skips_elements_with_nested_children() {
        let xml = r#"<Root><Outer><Inner>x</Inner></Outer><Plain>y</Plain></Root>"#;
        let pairs = list_general_elements(xml);
        // Outer has a child element → skipped. Inner and Plain remain.
        assert_eq!(
            pairs,
            vec![
                ("Inner".to_string(), "x".to_string()),
                ("Plain".to_string(), "y".to_string()),
            ]
        );
    }

    #[test]
    fn empty_xml_yields_not_found_for_attribute_lookup() {
        let err = set_drawing_attribute("", "SP_X", "v").unwrap_err();
        assert_eq!(
            err,
            MetadataEditError::AttributeNotFound {
                attr: "SP_X".into()
            }
        );
    }

    #[test]
    fn empty_xml_yields_not_found_for_element_lookup() {
        let err = set_element_text("", "X", "v").unwrap_err();
        assert_eq!(
            err,
            MetadataEditError::ElementNotFound {
                element: "X".into()
            }
        );
    }
}
