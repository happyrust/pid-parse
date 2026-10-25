//! `FreeFormAttrSet` (`0x0089`) rows of the top-level record chains.
//!
//! Every live `0x0089` record of the five drawings in
//! `D:\work\cad\PID自研软件验证` holds exactly one attribute set
//! (`docs/analysis/2026-10-10-a-label-value-reaches-its-item-through-its-text-box.md`):
//!
//! * payload `+0` is the row's own oid and `+12` the oid of the record it
//!   describes;
//! * at `+20` a set header -- `u16 tag 0x0000, u16 length, 0x00`, then the
//!   NUL-terminated ASCII class (`P&IDAttributes`, `SymbolAttr`,
//!   `_TagSequenceNo`, `_SmartText`, ...) -- whose length word covers exactly
//!   those bytes;
//! * back to back after it, the set's fields, each `u16 tag, u16 length` over
//!   the whole field: `0x8010` text (a kind byte, a NUL-terminated ASCII name,
//!   then a value whose only NUL ends the field, UTF-8 or else GBK), `0x0003`
//!   integer (a NUL-terminated name and an `i32`) and `0x000B` boolean (a
//!   NUL-terminated name and a `u16`, zero for false).
//!
//! The fields end at the first frame that is none of these. A row whose
//! header does not frame exactly is not decoded.

/// Payload offset of the oid of the record a row describes.
pub const ATTRIBUTE_ROW_DESCRIBED_OFFSET: usize = 12;
/// Payload offset of a row's set header.
pub const ATTRIBUTE_SET_OFFSET: usize = 20;

const SET_HEADER_TAG: u16 = 0x0000;
const TEXT_TAG: u16 = 0x8010;
const INTEGER_TAG: u16 = 0x0003;
const BOOLEAN_TAG: u16 = 0x000B;
/// Longest field or class name accepted.
const MAX_NAME_LEN: usize = 64;

/// One decoded `FreeFormAttrSet` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeRow {
    /// The row's own oid (payload `+0`).
    pub oid: u32,
    /// The oid of the record the row describes (payload `+12`).
    pub described: u32,
    /// The set's class name, e.g. `P&IDAttributes`.
    pub class: String,
    /// The set's fields in on-disk order.
    pub fields: Vec<AttributeField>,
}

/// One field of an attribute set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeField {
    /// The field name, e.g. `ModelID`.
    pub name: String,
    /// The field value.
    pub value: AttributeValue,
}

/// The value of an attribute-set field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeValue {
    /// `0x8010`: text, decoded as UTF-8 or else GBK.
    Text(String),
    /// `0x0003`: a little-endian `i32`.
    Integer(i32),
    /// `0x000B`: a little-endian `u16`, zero for false.
    Boolean(bool),
}

impl AttributeRow {
    /// The first text field called `name`.
    pub fn text(&self, name: &str) -> Option<&str> {
        self.fields.iter().find_map(|field| match &field.value {
            AttributeValue::Text(text) if field.name == name => Some(text.as_str()),
            _ => None,
        })
    }
}

/// Decode a `0x0089` row payload, or `None` when its set header does not
/// frame exactly at [`ATTRIBUTE_SET_OFFSET`].
pub fn decode_attribute_row(payload: &[u8]) -> Option<AttributeRow> {
    let oid = u32_at(payload, 0)?;
    let described = u32_at(payload, ATTRIBUTE_ROW_DESCRIBED_OFFSET)?;
    let (tag, end) = frame_at(payload, ATTRIBUTE_SET_OFFSET)?;
    if tag != SET_HEADER_TAG || *payload.get(ATTRIBUTE_SET_OFFSET + 4)? != 0 {
        return None;
    }
    let (class, after_class) = ascii_name(payload, ATTRIBUTE_SET_OFFSET + 5, end)?;
    if after_class != end {
        return None;
    }
    let mut fields = Vec::new();
    let mut at = end;
    while let Some((field, next)) = field_at(payload, at) {
        fields.push(field);
        at = next;
    }
    Some(AttributeRow {
        oid,
        described,
        class,
        fields,
    })
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    let bytes = data.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    let bytes = data.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

/// The tag of the frame at `at` and the offset its length word ends it at.
fn frame_at(data: &[u8], at: usize) -> Option<(u16, usize)> {
    let tag = u16_at(data, at)?;
    let length = usize::from(u16_at(data, at.checked_add(2)?)?);
    let end = at.checked_add(length)?;
    (length >= 6 && end <= data.len()).then_some((tag, end))
}

/// A NUL-terminated printable-ASCII name starting at `at` and ending before
/// `end`, and the offset just past its NUL.
fn ascii_name(data: &[u8], at: usize, end: usize) -> Option<(String, usize)> {
    let slice = data.get(at..end)?;
    let nul = slice.iter().position(|&byte| byte == 0)?;
    let name = &slice[..nul];
    if name.is_empty()
        || name.len() > MAX_NAME_LEN
        || !name.iter().all(|&byte| (0x20..0x7F).contains(&byte))
    {
        return None;
    }
    Some((String::from_utf8_lossy(name).into_owned(), at + nul + 1))
}

fn text_of(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => encoding_rs::GBK.decode(bytes).0.into_owned(),
    }
}

/// The field framed at `at` and the offset after it.
fn field_at(data: &[u8], at: usize) -> Option<(AttributeField, usize)> {
    let (tag, end) = frame_at(data, at)?;
    let (name, value) = match tag {
        TEXT_TAG => {
            let (name, value_at) = ascii_name(data, at + 5, end)?;
            let (last, body) = data.get(value_at..end)?.split_last()?;
            if *last != 0 || body.contains(&0) {
                return None;
            }
            (name, AttributeValue::Text(text_of(body)))
        }
        INTEGER_TAG => {
            let (name, value_at) = ascii_name(data, at + 4, end)?;
            if value_at + 4 != end {
                return None;
            }
            (
                name,
                AttributeValue::Integer(u32_at(data, value_at)? as i32),
            )
        }
        BOOLEAN_TAG => {
            let (name, value_at) = ascii_name(data, at + 4, end)?;
            if value_at + 2 != end {
                return None;
            }
            (name, AttributeValue::Boolean(u16_at(data, value_at)? != 0))
        }
        _ => return None,
    };
    Some((AttributeField { name, value }, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(tag: u16, body: &[u8]) -> Vec<u8> {
        let mut out = tag.to_le_bytes().to_vec();
        out.extend_from_slice(&((body.len() + 4) as u16).to_le_bytes());
        out.extend_from_slice(body);
        out
    }

    fn header(class: &str) -> Vec<u8> {
        let mut body = vec![0u8];
        body.extend_from_slice(class.as_bytes());
        body.push(0);
        frame(SET_HEADER_TAG, &body)
    }

    fn text(name: &str, value: &[u8]) -> Vec<u8> {
        let mut body = vec![1u8];
        body.extend_from_slice(name.as_bytes());
        body.push(0);
        body.extend_from_slice(value);
        body.push(0);
        frame(TEXT_TAG, &body)
    }

    fn integer(name: &str, value: i32) -> Vec<u8> {
        let mut body = name.as_bytes().to_vec();
        body.push(0);
        body.extend_from_slice(&value.to_le_bytes());
        frame(INTEGER_TAG, &body)
    }

    fn boolean(name: &str, value: u16) -> Vec<u8> {
        let mut body = name.as_bytes().to_vec();
        body.push(0);
        body.extend_from_slice(&value.to_le_bytes());
        frame(BOOLEAN_TAG, &body)
    }

    fn row(oid: u32, described: u32, parts: &[Vec<u8>]) -> Vec<u8> {
        let mut out = oid.to_le_bytes().to_vec();
        out.extend_from_slice(&[0; 8]);
        out.extend_from_slice(&described.to_le_bytes());
        out.extend_from_slice(&[0xFF, 0xFF, 0x04, 0x01]);
        for part in parts {
            out.extend_from_slice(part);
        }
        out
    }

    fn canonical() -> Vec<u8> {
        row(
            3061,
            3770,
            &[
                header("P&IDAttributes"),
                text("DrawingItemType", b"Connector"),
                text("ModelID", b"1460257C153F46D9ACB21E3A5FA31ADB"),
                integer("Flag", 1),
            ],
        )
    }

    #[test]
    fn a_row_names_itself_what_it_describes_its_class_and_its_fields() {
        let decoded = decode_attribute_row(&canonical()).expect("decodes");
        assert_eq!(decoded.oid, 3061);
        assert_eq!(decoded.described, 3770);
        assert_eq!(decoded.class, "P&IDAttributes");
        assert_eq!(decoded.text("DrawingItemType"), Some("Connector"));
        assert_eq!(
            decoded.text("ModelID"),
            Some("1460257C153F46D9ACB21E3A5FA31ADB")
        );
        assert_eq!(
            decoded.fields[2],
            AttributeField {
                name: "Flag".to_string(),
                value: AttributeValue::Integer(1),
            }
        );
        assert_eq!(decoded.text("Flag"), None);
    }

    #[test]
    fn a_boolean_field_is_zero_for_false() {
        let payload = row(
            1,
            2,
            &[
                header("_TagSequenceNo"),
                boolean("PropertyDrivesValue", 1),
                boolean("UseShortText", 0),
            ],
        );
        let decoded = decode_attribute_row(&payload).expect("decodes");
        assert_eq!(decoded.fields[0].value, AttributeValue::Boolean(true));
        assert_eq!(decoded.fields[1].value, AttributeValue::Boolean(false));
    }

    #[test]
    fn a_text_that_is_not_utf8_is_read_as_gbk() {
        let (gbk, _, _) = encoding_rs::GBK.encode("集注站");
        let payload = row(1, 2, &[header("_Name"), text("ItemValue", &gbk)]);
        let decoded = decode_attribute_row(&payload).expect("decodes");
        assert_eq!(decoded.text("ItemValue"), Some("集注站"));
    }

    #[test]
    fn a_header_elsewhere_than_plus_20_is_not_a_row() {
        let mut payload = canonical();
        payload.insert(ATTRIBUTE_SET_OFFSET, 0);
        assert_eq!(decode_attribute_row(&payload), None);
    }

    #[test]
    fn a_header_whose_length_overruns_its_name_is_not_a_row() {
        let mut payload = canonical();
        let length_at = ATTRIBUTE_SET_OFFSET + 2;
        let length = u16_at(&payload, length_at).unwrap() + 1;
        payload[length_at..length_at + 2].copy_from_slice(&length.to_le_bytes());
        assert_eq!(decode_attribute_row(&payload), None);
    }

    #[test]
    fn a_class_name_outside_printable_ascii_is_not_a_row() {
        let payload = row(1, 2, &[header("P&ID\u{7F}")]);
        assert_eq!(decode_attribute_row(&payload), None);
    }

    #[test]
    fn a_payload_too_short_for_the_header_is_not_a_row() {
        assert_eq!(
            decode_attribute_row(&canonical()[..ATTRIBUTE_SET_OFFSET + 3]),
            None
        );
        assert_eq!(decode_attribute_row(&[]), None);
    }

    #[test]
    fn the_fields_end_at_the_first_frame_of_another_tag() {
        let mut payload = canonical();
        payload.extend_from_slice(&frame(0x0042, b"Unknown\0\0\0"));
        payload.extend_from_slice(&text("After", b"x"));
        let decoded = decode_attribute_row(&payload).expect("decodes");
        assert_eq!(decoded.fields.len(), 3);
        assert_eq!(decoded.text("After"), None);
    }

    #[test]
    fn a_text_with_a_nul_inside_ends_the_fields() {
        let payload = row(
            1,
            2,
            &[
                header("_SmartText"),
                text("Before", b"a"),
                text("FormatString", b"a\0b"),
            ],
        );
        let decoded = decode_attribute_row(&payload).expect("decodes");
        assert_eq!(decoded.fields.len(), 1);
    }

    #[test]
    fn an_integer_framed_longer_than_four_bytes_ends_the_fields() {
        let mut body = b"Flag\0".to_vec();
        body.extend_from_slice(&[1, 0, 0, 0, 0]);
        let payload = row(1, 2, &[header("P&IDAttributes"), frame(INTEGER_TAG, &body)]);
        let decoded = decode_attribute_row(&payload).expect("decodes");
        assert!(decoded.fields.is_empty());
    }

    #[test]
    fn every_truncation_of_a_row_decodes_or_refuses_without_panicking() {
        let payload = canonical();
        for end in 0..=payload.len() {
            let _ = decode_attribute_row(&payload[..end]);
        }
        let decoded = decode_attribute_row(&payload[..payload.len() - 1]).expect("header intact");
        assert_eq!(decoded.fields.len(), 2);
    }
}
