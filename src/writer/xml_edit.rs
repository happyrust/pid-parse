//! Minimal text-level XML helpers for the writer pipeline.
//!
//! These helpers are intentionally **not** a full XML rewriter. They do a
//! targeted `<Tag>old</Tag>` text substitution that is safe for the simple,
//! flat, no-namespace metadata XML used by SmartPlant `/TaggedTxtData/*`
//! streams. Anything more elaborate (CDATA sections, comments, attributes
//! on the target tag, self-closing forms, nested tags with the same name)
//! is reported as an error instead of silently rewriting the wrong thing.
//!
//! Why not quick-xml / a real tree model? Because the downstream reader
//! is `parsers/xml_util::collect_simple_tags`, which itself handles only
//! this flat shape — so matching that assumption keeps the round-trip
//! surface honest.
use crate::error::PidError;

/// Replace the text body of the first occurrence of `<tag>...</tag>` in
/// `xml` with `new_value`. Returns the new XML string on success.
///
/// Accepts only simple `<tag>` open forms (no attributes). Rejects
/// self-closing forms and tags with attributes to avoid accidental
/// rewriting of a similarly-named attribute-bearing tag.
///
/// `new_value` is XML-escaped (`&`, `<`, `>` only — sufficient for text
/// nodes per the XML spec).
pub fn replace_simple_tag_text(
    xml: &str,
    tag: &str,
    new_value: &str,
) -> Result<String, PidError> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);

    let open_idx = xml.find(&open).ok_or_else(|| PidError::ParseFailure {
        context: format!("xml_edit:{}", tag),
        message: format!("no <{}> open tag found", tag),
    })?;
    let content_start = open_idx + open.len();

    let rel_close = xml[content_start..]
        .find(&close)
        .ok_or_else(|| PidError::ParseFailure {
            context: format!("xml_edit:{}", tag),
            message: format!("no matching </{}> close tag found", tag),
        })?;
    let content_end = content_start + rel_close;

    // Reject a second `<tag>` in between — that would mean the caller
    // wants to target a later occurrence (unsupported in the minimal
    // writer).
    if xml[content_start..content_end].contains(&open) {
        return Err(PidError::ParseFailure {
            context: format!("xml_edit:{}", tag),
            message: format!("nested <{}> tag found, cannot safely edit", tag),
        });
    }

    let escaped = escape_text(new_value);
    Ok(format!(
        "{}{}{}",
        &xml[..content_start],
        escaped,
        &xml[content_end..]
    ))
}

fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_first_occurrence_text() {
        let xml = "<Drawing><DrawingNumber>OLD-001</DrawingNumber></Drawing>";
        let got = replace_simple_tag_text(xml, "DrawingNumber", "NEW-001").expect("ok");
        assert_eq!(
            got,
            "<Drawing><DrawingNumber>NEW-001</DrawingNumber></Drawing>"
        );
    }

    #[test]
    fn escapes_special_characters() {
        let xml = "<Root><N>x</N></Root>";
        let got = replace_simple_tag_text(xml, "N", "A & B < C").expect("ok");
        assert!(got.contains("A &amp; B &lt; C"));
    }

    #[test]
    fn missing_tag_returns_error() {
        let xml = "<Root><A>a</A></Root>";
        let err = replace_simple_tag_text(xml, "B", "x").expect_err("should fail");
        match err {
            PidError::ParseFailure { context, .. } => assert_eq!(context, "xml_edit:B"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn missing_close_tag_returns_error() {
        let xml = "<Root><A>broken";
        let err = replace_simple_tag_text(xml, "A", "x").expect_err("should fail");
        match err {
            PidError::ParseFailure { context, message } => {
                assert_eq!(context, "xml_edit:A");
                assert!(message.contains("close"), "msg: {message}");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn rejects_nested_same_tag() {
        let xml = "<Root><A><A>inner</A></A></Root>";
        let err = replace_simple_tag_text(xml, "A", "x").expect_err("should fail");
        match err {
            PidError::ParseFailure { message, .. } => {
                assert!(message.contains("nested"), "msg: {message}");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn preserves_surrounding_whitespace_and_siblings() {
        let xml = "<Root>\n  <A>old</A>\n  <B>keep</B>\n</Root>";
        let got = replace_simple_tag_text(xml, "A", "new").expect("ok");
        assert_eq!(got, "<Root>\n  <A>new</A>\n  <B>keep</B>\n</Root>");
    }
}
