//! Trace-only walker for OLE PropertySetStream-formatted streams used by
//! `\x05SummaryInformation` and `\x05DocumentSummaryInformation`.
//!
//! Lives alongside the semantic decoder in [`crate::streams::summary`];
//! the two walk the same bytes independently so byte-audit accounting
//! stays cleanly separated from the business model.
//!
//! The reference layout follows [MS-OLEPS]:
//!
//! ```text
//! PropertySetStream (28-byte prefix):
//!     0x00  u16 LE   ByteOrder = 0xFFFE
//!     0x02  u16 LE   Version = 0
//!     0x04  u32 LE   SystemIdentifier
//!     0x08  16B      CLSID (zero in PropertySetStream)
//!     0x18  u32 LE   NumPropertySets
//!
//! Per section header (20 bytes, repeated NumPropertySets times):
//!     +0x00  16B      FMTID
//!     +0x10  u32 LE   section offset (relative to stream start)
//!
//! Section body @ section_offset:
//!     +0x00  u32 LE   section size
//!     +0x04  u32 LE   num_props
//!     +0x08  N × 8B   PROPID + offset table
//!                        (offset is relative to section start)
//!     ...    typed values pointed to by the offset table
//!
//! Typed value @ section_offset + prop_offset:
//!     +0x00  u32 LE   VT type tag (low 16 bits)
//!     followed by VT-specific payload (see read paths below)
//! ```
//!
//! `DocumentSummaryInformation` may carry a second section whose FMTID
//! is `{D5CDD505-2E9C-101B-9397-08002B2CF9AE}` and whose PROPID 0
//! holds a Dictionary of `propid → name` mappings. The walker decodes
//! this dictionary in full so user-defined string/integer properties
//! land entirely inside `consumed_ranges`.

use crate::byte_audit::{ByteRange, ParserTraceBuilder, TraceConfidence};

/// FMTID for `DocumentSummaryInformation` section 2 (user-defined
/// dictionary). Mirrors `streams::summary::FMTID_DOC_SUMMARY_SECTION_2`
/// — kept duplicated here so this trace-only walker has no `pub` /
/// cross-module dependency on the orchestration layer.
const FMTID_DOC_SUMMARY_SECTION_2: [u8; 16] = [
    0x05, 0xD5, 0xCD, 0xD5, 0x9C, 0x2E, 0x1B, 0x10, 0x93, 0x97, 0x08, 0x00, 0x2B, 0x2C, 0xF9, 0xAE,
];

/// `VT_LPSTR` — single-byte (code-page) string.
const VT_LPSTR: u32 = 0x0000_001E;
/// `VT_LPWSTR` — UTF-16LE string.
const VT_LPWSTR: u32 = 0x0000_001F;
/// `VT_FILETIME` — 8-byte FILETIME.
const VT_FILETIME: u32 = 0x0000_0040;
/// `VT_I4` — signed 32-bit integer.
const VT_I4: u32 = 0x0000_0003;
/// `VT_BOOL` — 16-bit boolean (0x0000 / 0xFFFF).
const VT_BOOL: u32 = 0x0000_000B;

/// Lightweight summary returned by the walker. Not used by the live
/// pipeline (which goes through [`crate::streams::summary`]); exposed
/// so the unit tests can assert the walker really visited every section
/// and prop without being coupled to the semantic model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryPropertySetTrace {
    /// `NumPropertySets` field copied verbatim from the prefix.
    pub num_sections: u32,
    /// How many sections the walker actually descended into. Should
    /// equal `num_sections` on well-formed streams; can be smaller when
    /// a section's offset / size is malformed.
    pub sections_walked: usize,
    /// How many `(PROPID, offset)` table entries the walker visited
    /// across all sections. Counts dictionary entries as one prop
    /// (PROPID 0) — the dictionary's internal records are tracked by
    /// `dict_entries_walked` instead.
    pub props_walked: usize,
    /// How many dictionary entries were decoded (only non-zero for
    /// `DocumentSummaryInformation` section 2 streams).
    pub dict_entries_walked: usize,
}

/// Phase 12b-1e trace-aware walker for OLE `PropertySetStream` streams.
///
/// Trace schema:
/// - `[0..28]` `PropertySetStream` prefix — `Decoded`.
/// - per `[28 + 20*i .. 28 + 20*i + 20]` section header — `Decoded`
///   (16B FMTID + 4B section offset).
/// - per section body @ `section_offset`:
///   - `[..+8]` section size + `num_props` — `Decoded`.
///   - `[+8 .. +8 + num_props*8]` `(PROPID, offset)` table — `Decoded`.
///   - per prop value @ `section_offset + prop_offset`:
///     - `VT_LPSTR` / `VT_LPWSTR` / `VT_I4` / `VT_BOOL` /
///       `VT_FILETIME` — full value (tag + length field + payload)
///       consumed `Decoded`.
///     - Dictionary (only when section FMTID matches user-dict and
///       `PROPID` == 0): `num_entries` header + every entry's
///       `(PROPID, length, name bytes)` consumed `Decoded`.
///     - Unknown VT — only the 4-byte VT tag consumed `Probed`; the
///       payload remains leftover so the audit surfaces the unmapped
///       region clearly.
///
/// Returns `Some(SummaryPropertySetTrace)` whenever the walker advances
/// past the 28-byte prefix; `None` for streams shorter than the prefix
/// or with a wrong byte-order marker. Inside the body, malformed
/// section / property pointers short-circuit individual descents
/// without unwinding the consume calls already issued — the leftover
/// region therefore pinpoints exactly where the walker stopped.
pub fn parse_summary_property_set_with_trace(
    data: &[u8],
    trace: &mut ParserTraceBuilder,
) -> Option<SummaryPropertySetTrace> {
    if data.len() < 28 {
        return None;
    }
    let byte_order = u16_le(data, 0)?;
    if byte_order != 0xFFFE {
        return None;
    }
    trace.consume(ByteRange::new(0, 28), TraceConfidence::Decoded);
    let num_sections = u32_le(data, 24)?;
    let mut sections_walked = 0usize;
    let mut props_walked = 0usize;
    let mut dict_entries_walked = 0usize;
    let section_headers_end = 28usize.checked_add(
        (num_sections as usize)
            .checked_mul(20)
            .filter(|n| *n <= data.len())?,
    )?;
    if section_headers_end > data.len() {
        return Some(SummaryPropertySetTrace {
            num_sections,
            sections_walked,
            props_walked,
            dict_entries_walked,
        });
    }

    for s in 0..num_sections as usize {
        let header_start = 28 + s * 20;
        let header_end = header_start + 20;
        let fmtid_start = header_start;
        let offset_field = header_start + 16;
        trace.consume(
            ByteRange::new(header_start as u64, header_end as u64),
            TraceConfidence::Decoded,
        );
        let Some(section_offset) = u32_le(data, offset_field).map(|v| v as usize) else {
            continue;
        };
        let Some(fmtid) = data.get(fmtid_start..fmtid_start + 16) else {
            continue;
        };
        let is_user_dict = fmtid == FMTID_DOC_SUMMARY_SECTION_2;
        if let Some((walked, dict_count)) = walk_section(data, section_offset, is_user_dict, trace)
        {
            sections_walked += 1;
            props_walked += walked;
            dict_entries_walked += dict_count;
        }
    }

    Some(SummaryPropertySetTrace {
        num_sections,
        sections_walked,
        props_walked,
        dict_entries_walked,
    })
}

fn walk_section(
    data: &[u8],
    section_offset: usize,
    is_user_dict: bool,
    trace: &mut ParserTraceBuilder,
) -> Option<(usize, usize)> {
    let body_header_end = section_offset.checked_add(8)?;
    if body_header_end > data.len() {
        return None;
    }
    trace.consume(
        ByteRange::new(section_offset as u64, body_header_end as u64),
        TraceConfidence::Decoded,
    );
    let num_props = u32_le(data, section_offset + 4)? as usize;
    let id_list_start = section_offset + 8;
    let id_list_end = id_list_start.checked_add(num_props.checked_mul(8)?)?;
    if id_list_end > data.len() {
        return None;
    }
    trace.consume(
        ByteRange::new(id_list_start as u64, id_list_end as u64),
        TraceConfidence::Decoded,
    );

    let mut dict_entries = 0usize;
    for i in 0..num_props {
        let entry = id_list_start + i * 8;
        let Some(prop_id) = u32_le(data, entry) else {
            break;
        };
        let Some(prop_offset) = u32_le(data, entry + 4).map(|v| v as usize) else {
            break;
        };
        let Some(abs_offset) = section_offset.checked_add(prop_offset) else {
            break;
        };
        if is_user_dict && prop_id == 0 {
            dict_entries += walk_dictionary(data, abs_offset, trace);
        } else {
            walk_typed_value(data, abs_offset, trace);
        }
    }
    Some((num_props, dict_entries))
}

fn walk_typed_value(data: &[u8], offset: usize, trace: &mut ParserTraceBuilder) {
    let Some(tag_end) = offset.checked_add(4).filter(|e| *e <= data.len()) else {
        return;
    };
    let Some(vt) = u32_le(data, offset).map(|v| v & 0x0000_FFFF) else {
        return;
    };
    let val_start = tag_end;
    match vt {
        VT_LPSTR => {
            let Some(len_field_end) = val_start.checked_add(4).filter(|e| *e <= data.len()) else {
                return;
            };
            let Some(len) = u32_le(data, val_start).map(|v| v as usize) else {
                return;
            };
            let Some(payload_end) = len_field_end.checked_add(len).filter(|e| *e <= data.len())
            else {
                return;
            };
            trace.consume(
                ByteRange::new(offset as u64, payload_end as u64),
                TraceConfidence::Decoded,
            );
        }
        VT_LPWSTR => {
            let Some(len_field_end) = val_start.checked_add(4).filter(|e| *e <= data.len()) else {
                return;
            };
            let Some(cc) = u32_le(data, val_start).map(|v| v as usize) else {
                return;
            };
            let Some(payload_bytes) = cc.checked_mul(2) else {
                return;
            };
            let Some(payload_end) = len_field_end
                .checked_add(payload_bytes)
                .filter(|e| *e <= data.len())
            else {
                return;
            };
            trace.consume(
                ByteRange::new(offset as u64, payload_end as u64),
                TraceConfidence::Decoded,
            );
        }
        VT_I4 => {
            let Some(end) = val_start.checked_add(4).filter(|e| *e <= data.len()) else {
                return;
            };
            trace.consume(
                ByteRange::new(offset as u64, end as u64),
                TraceConfidence::Decoded,
            );
        }
        VT_BOOL => {
            let Some(end) = val_start.checked_add(2).filter(|e| *e <= data.len()) else {
                return;
            };
            trace.consume(
                ByteRange::new(offset as u64, end as u64),
                TraceConfidence::Decoded,
            );
        }
        VT_FILETIME => {
            let Some(end) = val_start.checked_add(8).filter(|e| *e <= data.len()) else {
                return;
            };
            trace.consume(
                ByteRange::new(offset as u64, end as u64),
                TraceConfidence::Decoded,
            );
        }
        _ => {
            // Unknown VT — only the 4B tag is recoverable; payload
            // remains leftover for audit.
            trace.consume(
                ByteRange::new(offset as u64, tag_end as u64),
                TraceConfidence::Probed,
            );
        }
    }
}

fn walk_dictionary(data: &[u8], offset: usize, trace: &mut ParserTraceBuilder) -> usize {
    let Some(num_entries_end) = offset.checked_add(4).filter(|e| *e <= data.len()) else {
        return 0;
    };
    let Some(num_entries) = u32_le(data, offset).map(|v| v as usize) else {
        return 0;
    };
    trace.consume(
        ByteRange::new(offset as u64, num_entries_end as u64),
        TraceConfidence::Decoded,
    );
    let mut cursor = num_entries_end;
    let mut walked = 0usize;
    for _ in 0..num_entries {
        let Some(header_end) = cursor.checked_add(8).filter(|e| *e <= data.len()) else {
            return walked;
        };
        trace.consume(
            ByteRange::new(cursor as u64, header_end as u64),
            TraceConfidence::Decoded,
        );
        let Some(len) = u32_le(data, cursor + 4).map(|v| v as usize) else {
            return walked;
        };
        let name_start = header_end;
        let Some(name_end) = name_start.checked_add(len).filter(|e| *e <= data.len()) else {
            return walked;
        };
        trace.consume(
            ByteRange::new(name_start as u64, name_end as u64),
            TraceConfidence::Decoded,
        );
        cursor = name_end;
        walked += 1;
    }
    walked
}

fn u16_le(data: &[u8], offset: usize) -> Option<u16> {
    let slice = data.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

fn u32_le(data: &[u8], offset: usize) -> Option<u32> {
    let slice = data.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FMTID_SUMMARY: [u8; 16] = [
        0xE0, 0x85, 0x9F, 0xF2, 0xF9, 0x4F, 0x68, 0x10, 0xAB, 0x91, 0x08, 0x00, 0x2B, 0x27, 0xB3,
        0xD9,
    ];
    const FMTID_DOC_SUMMARY_SECTION_1: [u8; 16] = [
        0x02, 0xD5, 0xCD, 0xD5, 0x9C, 0x2E, 0x1B, 0x10, 0x93, 0x97, 0x08, 0x00, 0x2B, 0x2C, 0xF9,
        0xAE,
    ];

    fn write_property_set_prefix(out: &mut Vec<u8>, num_sections: u32) {
        out.extend_from_slice(&0xFFFEu16.to_le_bytes()); // ByteOrder
        out.extend_from_slice(&0u16.to_le_bytes()); // Version
        out.extend_from_slice(&0u32.to_le_bytes()); // SystemIdentifier
        out.extend_from_slice(&[0u8; 16]); // CLSID
        out.extend_from_slice(&num_sections.to_le_bytes());
    }

    fn vt_lpwstr(text: &str) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&VT_LPWSTR.to_le_bytes());
        let mut units: Vec<u16> = text.encode_utf16().collect();
        units.push(0);
        v.extend_from_slice(&(units.len() as u32).to_le_bytes());
        for u in units {
            v.extend_from_slice(&u.to_le_bytes());
        }
        v
    }

    fn vt_lpstr(text: &str) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&VT_LPSTR.to_le_bytes());
        let mut bytes: Vec<u8> = text.bytes().collect();
        bytes.push(0);
        v.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        v.extend_from_slice(&bytes);
        v
    }

    fn vt_filetime(value: u64) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&VT_FILETIME.to_le_bytes());
        v.extend_from_slice(&value.to_le_bytes());
        v
    }

    fn vt_i4(value: i32) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&VT_I4.to_le_bytes());
        v.extend_from_slice(&value.to_le_bytes());
        v
    }

    fn vt_bool(value: bool) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&VT_BOOL.to_le_bytes());
        let raw: u16 = if value { 0xFFFF } else { 0x0000 };
        v.extend_from_slice(&raw.to_le_bytes());
        v
    }

    /// Build a single-section `PropertySetStream` containing the given
    /// `(prop_id, value_bytes)` pairs.
    fn build_summary(props: &[(u32, Vec<u8>)]) -> Vec<u8> {
        let header_len = 8 + props.len() * 8;
        let mut entries = Vec::new();
        let mut payloads: Vec<Vec<u8>> = Vec::new();
        let mut cursor = header_len;
        for (prop_id, payload) in props {
            entries.push((*prop_id, cursor as u32));
            cursor += payload.len();
            payloads.push(payload.clone());
        }
        let total = cursor;

        let mut section = Vec::new();
        section.extend_from_slice(&(total as u32).to_le_bytes());
        section.extend_from_slice(&(props.len() as u32).to_le_bytes());
        for (id, off) in entries {
            section.extend_from_slice(&id.to_le_bytes());
            section.extend_from_slice(&off.to_le_bytes());
        }
        for p in payloads {
            section.extend_from_slice(&p);
        }

        let mut stream = Vec::new();
        write_property_set_prefix(&mut stream, 1);
        stream.extend_from_slice(&FMTID_SUMMARY);
        let section_offset: u32 = 28 + 20;
        stream.extend_from_slice(&section_offset.to_le_bytes());
        stream.extend_from_slice(&section);
        stream
    }

    #[test]
    fn rejects_streams_shorter_than_property_set_prefix() {
        let mut b = ParserTraceBuilder::new("parse_summary_property_set");
        let r = parse_summary_property_set_with_trace(&[0; 16], &mut b);
        assert!(r.is_none());
        let trace = b.build("/\u{5}SummaryInformation", 16);
        assert_eq!(trace.consumed_bytes(), 0);
    }

    #[test]
    fn rejects_wrong_byte_order_marker() {
        let mut data = vec![0u8; 28];
        data[0] = 0xFE;
        data[1] = 0xFE; // not 0xFFFE LE
        let mut b = ParserTraceBuilder::new("parse_summary_property_set");
        let r = parse_summary_property_set_with_trace(&data, &mut b);
        assert!(r.is_none());
        let trace = b.build("/\u{5}SummaryInformation", data.len() as u64);
        assert_eq!(trace.consumed_bytes(), 0);
    }

    #[test]
    fn walks_single_lpwstr_property_with_full_byte_coverage() {
        let data = build_summary(&[(2, vt_lpwstr("Demo Title"))]);
        let mut b = ParserTraceBuilder::new("parse_summary_property_set");
        let summary =
            parse_summary_property_set_with_trace(&data, &mut b).expect("walker succeeds");
        assert_eq!(summary.num_sections, 1);
        assert_eq!(summary.sections_walked, 1);
        assert_eq!(summary.props_walked, 1);
        assert_eq!(summary.dict_entries_walked, 0);

        let trace = b.build("/\u{5}SummaryInformation", data.len() as u64);
        assert_eq!(
            trace.consumed_bytes(),
            data.len() as u64,
            "every byte of fixture must be claimed"
        );
        assert!(trace.leftover_ranges.is_empty());
    }

    #[test]
    fn walks_mixed_value_types_decoded_only() {
        let data = build_summary(&[
            (2, vt_lpwstr("Title")),
            (3, vt_lpstr("Subject")),
            (12, vt_filetime(0x1FED_CBA9_8765_4321)),
            (15, vt_i4(-7)),
        ]);
        let mut b = ParserTraceBuilder::new("parse_summary_property_set");
        let summary =
            parse_summary_property_set_with_trace(&data, &mut b).expect("walker succeeds");
        assert_eq!(summary.props_walked, 4);

        let trace = b.build("/\u{5}SummaryInformation", data.len() as u64);
        assert_eq!(trace.consumed_bytes(), data.len() as u64);

        // All ranges should be Decoded; no Probed bucket because every
        // VT is recognised.
        let probed = trace
            .ranges_by_confidence
            .get(&TraceConfidence::Probed)
            .cloned()
            .unwrap_or_default();
        assert!(probed.is_empty(), "no Probed ranges expected: {probed:?}");
    }

    #[test]
    fn unknown_vt_consumes_only_tag_as_probed() {
        // Build a synthetic single-prop section pointing at a VT_UI8
        // (0x0015) value 8 bytes long. Walker must claim only the
        // 4-byte tag and leave the 8-byte payload as leftover.
        let payload_bytes = {
            let mut v: Vec<u8> = Vec::new();
            v.extend_from_slice(&0x0015u32.to_le_bytes()); // VT_UI8
            v.extend_from_slice(&[0xAA; 8]);
            v
        };
        let header_len = 8 + 8; // size + count + 1 entry
        let mut section = Vec::new();
        let total = header_len + payload_bytes.len();
        section.extend_from_slice(&(total as u32).to_le_bytes());
        section.extend_from_slice(&1u32.to_le_bytes());
        section.extend_from_slice(&7u32.to_le_bytes()); // PROPID 7
        section.extend_from_slice(&(header_len as u32).to_le_bytes());
        section.extend_from_slice(&payload_bytes);

        let mut stream = Vec::new();
        write_property_set_prefix(&mut stream, 1);
        stream.extend_from_slice(&FMTID_SUMMARY);
        let section_offset: u32 = 28 + 20;
        stream.extend_from_slice(&section_offset.to_le_bytes());
        stream.extend_from_slice(&section);

        let mut b = ParserTraceBuilder::new("parse_summary_property_set");
        let summary =
            parse_summary_property_set_with_trace(&stream, &mut b).expect("walker succeeds");
        assert_eq!(summary.props_walked, 1);

        let trace = b.build("/\u{5}SummaryInformation", stream.len() as u64);
        // Decoded buckets cover prefix (28) + section header (20) +
        // section size+count (8) + id_list (8) + VT tag is Probed →
        // total Decoded = 64. Probed bucket = VT tag (4). Leftover =
        // 8-byte payload.
        let decoded_total: u64 = trace
            .ranges_by_confidence
            .get(&TraceConfidence::Decoded)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(ByteRange::len)
            .sum();
        let probed_total: u64 = trace
            .ranges_by_confidence
            .get(&TraceConfidence::Probed)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(ByteRange::len)
            .sum();
        assert_eq!(probed_total, 4, "only 4-byte VT tag claimed as Probed");
        assert_eq!(decoded_total, stream.len() as u64 - 8 - 4);
        assert_eq!(trace.leftover_bytes(), 8, "8-byte VT_UI8 payload leftover");
    }

    #[test]
    fn boolean_property_consumes_two_byte_payload() {
        let data = build_summary(&[(23, vt_bool(true))]);
        let mut b = ParserTraceBuilder::new("parse_summary_property_set");
        parse_summary_property_set_with_trace(&data, &mut b).expect("walker succeeds");
        let trace = b.build("/\u{5}SummaryInformation", data.len() as u64);
        assert_eq!(trace.consumed_bytes(), data.len() as u64);
    }

    #[test]
    fn walks_two_section_doc_summary_with_user_dictionary_full_coverage() {
        // Section 1: standard DocSummary, one VT_LPWSTR Category.
        let category = vt_lpwstr("Demo Category");
        let sec1_header_len = 8 + 8;
        let sec1_total = sec1_header_len + category.len();
        let mut sec1 = Vec::new();
        sec1.extend_from_slice(&(sec1_total as u32).to_le_bytes());
        sec1.extend_from_slice(&1u32.to_le_bytes());
        sec1.extend_from_slice(&2u32.to_le_bytes()); // PROPID 2 = Category
        sec1.extend_from_slice(&(sec1_header_len as u32).to_le_bytes());
        sec1.extend_from_slice(&category);

        // Section 2: user dictionary with one entry.
        let dict_body = {
            let mut v = Vec::new();
            v.extend_from_slice(&1u32.to_le_bytes()); // num entries
            v.extend_from_slice(&4u32.to_le_bytes()); // PROPID 4
            let name = b"SP_ProjectID\0";
            v.extend_from_slice(&(name.len() as u32).to_le_bytes());
            v.extend_from_slice(name);
            v
        };
        let value_bytes = vt_lpwstr("PROJ-001");
        let sec2_header_len = 8 + 16; // size + count + 2 entries
        let dict_off = sec2_header_len;
        let value_off = dict_off + dict_body.len();
        let sec2_total = value_off + value_bytes.len();
        let mut sec2 = Vec::new();
        sec2.extend_from_slice(&(sec2_total as u32).to_le_bytes());
        sec2.extend_from_slice(&2u32.to_le_bytes());
        sec2.extend_from_slice(&0u32.to_le_bytes()); // PROPID 0 = Dictionary
        sec2.extend_from_slice(&(dict_off as u32).to_le_bytes());
        sec2.extend_from_slice(&4u32.to_le_bytes()); // PROPID 4 = user value
        sec2.extend_from_slice(&(value_off as u32).to_le_bytes());
        sec2.extend_from_slice(&dict_body);
        sec2.extend_from_slice(&value_bytes);

        // Build full stream.
        let mut stream = Vec::new();
        write_property_set_prefix(&mut stream, 2);
        stream.extend_from_slice(&FMTID_DOC_SUMMARY_SECTION_1);
        let sec1_offset = 28u32 + 20 * 2;
        stream.extend_from_slice(&sec1_offset.to_le_bytes());
        stream.extend_from_slice(&FMTID_DOC_SUMMARY_SECTION_2);
        let sec2_offset = sec1_offset + sec1.len() as u32;
        stream.extend_from_slice(&sec2_offset.to_le_bytes());
        stream.extend_from_slice(&sec1);
        stream.extend_from_slice(&sec2);

        let mut b = ParserTraceBuilder::new("parse_summary_property_set");
        let summary =
            parse_summary_property_set_with_trace(&stream, &mut b).expect("walker succeeds");
        assert_eq!(summary.num_sections, 2);
        assert_eq!(summary.sections_walked, 2);
        assert_eq!(
            summary.props_walked, 3,
            "section 1 has 1 prop + section 2 has 2 props"
        );
        assert_eq!(summary.dict_entries_walked, 1);

        let trace = b.build("/\u{5}DocumentSummaryInformation", stream.len() as u64);
        assert_eq!(
            trace.consumed_bytes(),
            stream.len() as u64,
            "every byte must be Decoded for this fixture"
        );
        assert!(trace.leftover_ranges.is_empty());
    }

    #[test]
    fn truncated_section_body_short_circuits_without_panic() {
        // Standard 1-section header points past the stream end. Walker
        // must not panic; it should consume the prefix + section header
        // and leave the rest as leftover.
        let mut stream = Vec::new();
        write_property_set_prefix(&mut stream, 1);
        stream.extend_from_slice(&FMTID_SUMMARY);
        let bogus_offset: u32 = 1_000_000;
        stream.extend_from_slice(&bogus_offset.to_le_bytes());

        let mut b = ParserTraceBuilder::new("parse_summary_property_set");
        let summary =
            parse_summary_property_set_with_trace(&stream, &mut b).expect("walker still returns");
        assert_eq!(summary.num_sections, 1);
        assert_eq!(summary.sections_walked, 0, "section body could not be read");

        let trace = b.build("/\u{5}SummaryInformation", stream.len() as u64);
        // Prefix (28) + section header (20) = 48 Decoded, remaining 0.
        assert_eq!(trace.consumed_bytes(), 48);
        assert_eq!(trace.leftover_bytes(), stream.len() as u64 - 48);
    }
}
