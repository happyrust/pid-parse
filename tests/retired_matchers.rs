//! The rules of the four retired matchers, held where the matchers now live.
//!
//! These are the unit tests the matchers had in
//! `pid_parse::parsers::sheet_records` until the matchers left the library
//! (`tests/support/retired_matchers.rs` says why they are kept at all). They
//! hold what each matcher accepts and refuses at an offset, and — walked on a
//! chain through the library's `PsmRecordDecoder::scan` — what a stream hands
//! them: nothing for `0x3FE6`, a type word the chain walk stops on, and
//! nothing for a record that carries the `0x4000` flag.

#[path = "support/retired_matchers.rs"]
mod retired_matchers;

use pid_parse::parsers::sheet_records::{
    parse_psm_header, PsmRecordDecoder, SheetChain, PSM_ENVELOPE_LEN, PSM_TYPE_CODE_IGLINE2D,
    SHEET_STREAM_HEADER_LEN, SHEET_STREAM_MAGIC,
};
use retired_matchers::{
    decode_attribute_fragment_at, decode_primitive_line_at, decode_sub_record_0x0010_at,
    AttributeFragmentDecoder, GLine2dDecoder, SheetAttributeFragmentDecoded,
    SheetPrimitiveLineDecoded, SheetSubRecord0x0010Decoded, SubRecord0x0010Decoder,
    GLINE2D_PAYLOAD_LEN, PSM_RECORD_HEADER_LEN, PSM_TYPE_CODE_GLINE2D,
    PSM_TYPE_CODE_SUB_RECORD_0X0010, SUB_RECORD_0X0010_MAX_BYTES_TO_FOLLOW,
};

/// `records` — one record or a run of them, end to end — behind the 8-byte
/// header a chain stream opens with: the magic, and the number of records
/// among them the native reader reads.
fn chain_stream(records: &[u8]) -> Vec<u8> {
    let mut stream = vec![0u8; SHEET_STREAM_HEADER_LEN];
    stream.extend_from_slice(records);
    let live = u32::try_from(SheetChain::walk(&stream).live_records).unwrap_or(u32::MAX);
    stream[..4].copy_from_slice(&SHEET_STREAM_MAGIC.to_le_bytes());
    stream[4..8].copy_from_slice(&live.to_le_bytes());
    stream
}

/// Wrap synthetic records into a `Sheet*` stream: the 8-byte stream header,
/// then the records nose to tail. The walk does not inspect the header
/// bytes, only their length.
fn synthetic_sheet_stream(records: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![0u8; SHEET_STREAM_HEADER_LEN];
    for record in records {
        out.extend_from_slice(record);
    }
    out
}

// -----------------------------------------------------------------
// Phase 14 Slice D: PSM GLine2d decoder tests
// -----------------------------------------------------------------

/// Bytes a `GLine2d` record declares after the 6-byte envelope when it
/// carries nothing but its own body: the 12 header bytes the envelope
/// does not cover (`oid` + aux) plus the 48-byte payload.
///
/// Declaring 48 here instead would make the record claim to end 12 bytes
/// before its own payload does — harmless to a scan, but it is not a
/// record a chain walk can step over, so the fixture would no longer
/// model a real stream.
const SYNTHETIC_GLINE2D_BYTES_TO_FOLLOW: usize =
    PSM_RECORD_HEADER_LEN - PSM_ENVELOPE_LEN + GLINE2D_PAYLOAD_LEN;

/// Build a single synthetic PSM `GLine2d` record:
/// 18-byte header (type=0x3FE6, oid=`oid`) + 6×f64 `GLine2d` payload
/// (origin, direction, params).
fn build_synthetic_gline2d_record(
    oid: u32,
    origin: (f64, f64),
    direction: (f64, f64),
    param_start: f64,
    param_end: f64,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(PSM_RECORD_HEADER_LEN + GLINE2D_PAYLOAD_LEN);
    // type_code (14-bit) at bits 0..14 of the LE u16, top 2 bits = 0 flags.
    let type_word: u16 = PSM_TYPE_CODE_GLINE2D;
    out.extend_from_slice(&type_word.to_le_bytes());
    out.extend_from_slice(&(SYNTHETIC_GLINE2D_BYTES_TO_FOLLOW as u32).to_le_bytes());
    // oid
    out.extend_from_slice(&oid.to_le_bytes());
    // 8-byte aux (set to fixed pattern for inspection).
    out.extend_from_slice(&[0u8; 8]);
    // 6 doubles
    for v in [
        origin.0,
        origin.1,
        direction.0,
        direction.1,
        param_start,
        param_end,
    ] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// The matchers of three retired families walked on a chain, as the
/// stream entry points they no longer have walked them. The unit tests
/// below hold the matchers' own rules, which the evidence tests rest
/// on; nothing outside tests walks them.
fn decode_primitive_lines(data: &[u8]) -> Vec<SheetPrimitiveLineDecoded> {
    GLine2dDecoder.scan(data)
}

/// The same for the `0x0010` sub-record view.
fn decode_sub_records_0x0010(data: &[u8]) -> Vec<SheetSubRecord0x0010Decoded> {
    SubRecord0x0010Decoder.scan(data)
}

/// The same for the attribute-fragment view.
fn decode_attribute_fragments(data: &[u8]) -> Vec<SheetAttributeFragmentDecoded> {
    AttributeFragmentDecoder.scan(data)
}

/// The `GLine2d` records of a synthetic stream, read where the test put
/// them: each record's own length leads to the next.
///
/// `0x3FE6` is past the last slot of the native reader's class table,
/// so a chain walk stops on such a record
/// ([`psm_type_word_opens_a_record`]) and the matcher walked on a chain
/// returns nothing for any stream. The payload rules are the class's
/// own and are still held, through the entry point that reads at an
/// offset.
fn gline2d_records_where_they_lie(stream: &[u8]) -> Vec<SheetPrimitiveLineDecoded> {
    let mut out = Vec::new();
    let mut at = SHEET_STREAM_HEADER_LEN;
    while let Some(header) = parse_psm_header(stream, at) {
        let end = header.body_start + header.bytes_to_follow as usize;
        if header.bytes_to_follow == 0 || end > stream.len() {
            break;
        }
        out.extend(decode_primitive_line_at(stream, at));
        at = end;
    }
    out
}

#[test]
fn primitive_line_decodes_canonical_synthetic_record() {
    // Canonical synthetic line: origin at (0,0), unit horizontal
    // direction, params [0, 1.0]. Endpoints A=(0,0) B=(1,0).
    let record = build_synthetic_gline2d_record(42, (0.0, 0.0), (1.0, 0.0), 0.0, 1.0);
    let stream = synthetic_sheet_stream(&[record]);
    assert!(
        decode_primitive_lines(&stream).is_empty(),
        "no chain holds a record of a type code past the class table"
    );
    let decoded = gline2d_records_where_they_lie(&stream);
    assert_eq!(decoded.len(), 1, "expected exactly one decoded line");
    let line = &decoded[0];
    assert_eq!(line.type_code, PSM_TYPE_CODE_GLINE2D);
    assert_eq!(line.type_flags, 0);
    assert_eq!(
        line.bytes_to_follow as usize,
        SYNTHETIC_GLINE2D_BYTES_TO_FOLLOW
    );
    assert_eq!(line.oid, 42);
    assert_eq!(line.origin, (0.0, 0.0));
    assert!((line.direction.0 - 1.0).abs() < 1e-12);
    assert!(line.direction.1.abs() < 1e-12);
    assert_eq!(line.param_start, 0.0);
    assert_eq!(line.param_end, 1.0);
    assert_eq!(line.byte_range.start, SHEET_STREAM_HEADER_LEN);
    // byte_range covers the envelope plus everything it declares.
    assert_eq!(
        line.byte_range.end,
        SHEET_STREAM_HEADER_LEN + PSM_ENVELOPE_LEN + SYNTHETIC_GLINE2D_BYTES_TO_FOLLOW
    );
    // Endpoint helpers.
    assert_eq!(line.endpoint_a(), (0.0, 0.0));
    let (bx, by) = line.endpoint_b();
    assert!((bx - 1.0).abs() < 1e-12);
    assert!(by.abs() < 1e-12);
}

#[test]
fn primitive_line_rejects_wrong_type_code() {
    // Same bytes but with a non-GLine2d type code.
    let mut record = build_synthetic_gline2d_record(1, (0.0, 0.0), (1.0, 0.0), 0.0, 1.0);
    // Overwrite type with 0x1234 (not the GLine2d type).
    record[0] = 0x34;
    record[1] = 0x12;
    let decoded = gline2d_records_where_they_lie(&synthetic_sheet_stream(&[record]));
    assert!(decoded.is_empty(), "wrong type_code must be rejected");
}

#[test]
fn primitive_line_rejects_non_unit_direction() {
    // direction = (2.0, 0.0): length 2, not unit.
    let record = build_synthetic_gline2d_record(1, (0.0, 0.0), (2.0, 0.0), 0.0, 1.0);
    assert!(
        gline2d_records_where_they_lie(&synthetic_sheet_stream(&[record])).is_empty(),
        "non-unit direction vector must be rejected"
    );
}

#[test]
fn primitive_line_rejects_zero_direction() {
    // direction = (0.0, 0.0): zero vector.
    let record = build_synthetic_gline2d_record(1, (0.0, 0.0), (0.0, 0.0), 0.0, 1.0);
    assert!(
        gline2d_records_where_they_lie(&synthetic_sheet_stream(&[record])).is_empty(),
        "zero direction vector must be rejected"
    );
}

#[test]
fn primitive_line_rejects_reversed_param_range() {
    // param_start == param_end.
    let record = build_synthetic_gline2d_record(1, (0.0, 0.0), (1.0, 0.0), 1.0, 1.0);
    assert!(
        gline2d_records_where_they_lie(&synthetic_sheet_stream(&[record])).is_empty(),
        "param_start >= param_end must be rejected"
    );
    // param_start > param_end.
    let record = build_synthetic_gline2d_record(1, (0.0, 0.0), (1.0, 0.0), 1.5, 0.5);
    assert!(
        gline2d_records_where_they_lie(&synthetic_sheet_stream(&[record])).is_empty(),
        "reversed param range must be rejected"
    );
}

#[test]
fn primitive_line_rejects_nan_coordinate() {
    let mut record = build_synthetic_gline2d_record(1, (0.0, 0.0), (1.0, 0.0), 0.0, 1.0);
    // Overwrite origin.x with NaN bytes (any non-finite).
    let nan_bytes = f64::NAN.to_le_bytes();
    let origin_off = PSM_RECORD_HEADER_LEN;
    record[origin_off..origin_off + 8].copy_from_slice(&nan_bytes);
    assert!(
        gline2d_records_where_they_lie(&synthetic_sheet_stream(&[record])).is_empty(),
        "NaN coordinate must be rejected"
    );
}

/// A payload that satisfies every `GLine2d` rule is still not a record
/// when it sits inside another one.
///
/// This is the corpus failure in miniature: three "records" that passed
/// all five payload rules while living 160 bytes inside an
/// `igSmartFrame2d`. Chain membership is what rejects them, so it needs
/// a test that fails if the gate is ever removed.
#[test]
fn primitive_line_inside_another_records_payload_is_not_a_record() {
    let hidden = build_synthetic_gline2d_record(99, (0.0, 0.0), (1.0, 0.0), 0.0, 1.0);
    // One record of some other family whose payload happens to contain a
    // perfectly well-formed GLine2d.
    let mut host = Vec::new();
    host.extend_from_slice(&PSM_TYPE_CODE_IGLINE2D.to_le_bytes());
    host.extend_from_slice(&(hidden.len() as u32).to_le_bytes());
    host.extend_from_slice(&hidden);

    let stream = synthetic_sheet_stream(&[host]);
    assert!(
        decode_primitive_line_at(&stream, SHEET_STREAM_HEADER_LEN + 6).is_some(),
        "the buried bytes do satisfy every payload rule; without that the \
         test would prove nothing"
    );
    assert!(
        decode_primitive_lines(&stream).is_empty(),
        "a payload-shaped match inside another record is not a record"
    );
}

#[test]
fn primitive_line_decoder_is_panic_safe_on_truncated_input() {
    // Build a complete stream, then truncate at various sizes.
    let record = build_synthetic_gline2d_record(1, (0.0, 0.0), (1.0, 0.0), 0.0, 1.0);
    let stream = synthetic_sheet_stream(&[record]);
    for trunc_len in 0..stream.len() {
        // Must not panic, must return empty / no decoded line.
        let decoded = decode_primitive_lines(&stream[..trunc_len]);
        assert!(
            decoded.is_empty(),
            "truncated input of length {trunc_len} must not decode anything"
        );
    }
    // Empty input also fine.
    assert!(decode_primitive_lines(&[]).is_empty());
}

#[test]
fn primitive_line_decoder_is_panic_safe_on_random_noise() {
    // Adversarial deterministic noise: incrementing bytes.
    let noise: Vec<u8> = (0..4096).map(|i| (i & 0xFF) as u8).collect();
    // Just running without panic is the test; whatever it decodes
    // is acceptable (must be `Vec`, not panic).
    let _decoded = decode_primitive_lines(&noise);
    // All-zeros: nothing valid.
    assert!(decode_primitive_lines(&vec![0u8; 4096]).is_empty());
    // All-0xFF: nothing valid (type_code = 0x3FFF != GLine2d).
    assert!(decode_primitive_lines(&vec![0xFFu8; 4096]).is_empty());
}

#[test]
fn primitive_line_decodes_two_back_to_back_records() {
    let data = synthetic_sheet_stream(&[
        build_synthetic_gline2d_record(7, (0.10, 0.10), (1.0, 0.0), 0.0, 0.5),
        build_synthetic_gline2d_record(8, (0.20, 0.30), (0.0, 1.0), 0.0, 0.8),
    ]);
    let decoded = gline2d_records_where_they_lie(&data);
    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded[0].oid, 7);
    assert_eq!(decoded[1].oid, 8);
    // The two records must have non-overlapping byte ranges.
    assert!(decoded[0].byte_range.end <= decoded[1].byte_range.start);
}

#[test]
fn primitive_line_byte_range_covers_full_record_when_attribute_tail_present() {
    // Build a record whose bytes_to_follow covers its 12 uncovered
    // header bytes, the 48-byte payload, and a 200-byte attribute tail.
    const TAIL_LEN: usize = 200;
    const BYTES_TO_FOLLOW: usize = SYNTHETIC_GLINE2D_BYTES_TO_FOLLOW + TAIL_LEN;
    let mut record = Vec::new();
    let type_word: u16 = PSM_TYPE_CODE_GLINE2D;
    record.extend_from_slice(&type_word.to_le_bytes());
    record.extend_from_slice(&(BYTES_TO_FOLLOW as u32).to_le_bytes());
    // oid
    record.extend_from_slice(&123u32.to_le_bytes());
    // 8-byte aux
    record.extend_from_slice(&[0u8; 8]);
    // GLine2d payload
    for v in [0.0f64, 0.0, 1.0, 0.0, 0.0, 1.0] {
        record.extend_from_slice(&v.to_le_bytes());
    }
    record.extend_from_slice(&[0xAB; TAIL_LEN]);

    let decoded = gline2d_records_where_they_lie(&synthetic_sheet_stream(&[record]));
    assert_eq!(decoded.len(), 1);
    let line = &decoded[0];
    assert_eq!(line.byte_range.start, SHEET_STREAM_HEADER_LEN);
    // byte_range covers the envelope plus everything it declares, tail
    // included — not just the geometry payload.
    assert_eq!(
        line.byte_range.end,
        SHEET_STREAM_HEADER_LEN + PSM_ENVELOPE_LEN + BYTES_TO_FOLLOW
    );
    assert_eq!(line.bytes_to_follow as usize, BYTES_TO_FOLLOW);
}

// -----------------------------------------------------------------
// Phase 26: PSM 0x0010 attribute-fragment decoder tests
// -----------------------------------------------------------------

fn build_attribute_fragment(marker: u32, aux: [u8; 8], strings: &[&str]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&marker.to_le_bytes());
    payload.extend_from_slice(&aux);
    for s in strings {
        let units: Vec<u16> = s.encode_utf16().collect();
        payload.extend_from_slice(&(units.len() as u16).to_le_bytes());
        for u in units {
            payload.extend_from_slice(&u.to_le_bytes());
        }
    }
    let mut out = Vec::new();
    out.extend_from_slice(&PSM_TYPE_CODE_SUB_RECORD_0X0010.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&payload);
    out
}

#[test]
fn attribute_fragment_decodes_single_string() {
    let rec = build_attribute_fragment(
        0x0001_0002,
        [3, 0, 1, 0, 0x44, 0, 0, 0],
        &["ODOIL020150 MM"],
    );
    let decoded = decode_attribute_fragments(&chain_stream(&rec));
    assert_eq!(decoded.len(), 1);
    let f = &decoded[0];
    assert_eq!(f.type_code, PSM_TYPE_CODE_SUB_RECORD_0X0010);
    assert_eq!(f.marker, 0x0001_0002);
    assert_eq!(f.aux, [3, 0, 1, 0, 0x44, 0, 0, 0]);
    assert_eq!(f.strings.len(), 1);
    assert_eq!(f.strings[0].char_count, 14);
    assert_eq!(f.strings[0].text, "ODOIL020150 MM");
}

#[test]
fn attribute_fragment_decodes_cjk_string() {
    let rec = build_attribute_fragment(0x0001_0002, [0; 8], &["设计温度"]);
    let decoded = decode_attribute_fragments(&chain_stream(&rec));
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].strings[0].text, "设计温度");
    assert_eq!(decoded[0].strings[0].char_count, 4);
}

#[test]
fn attribute_fragment_decodes_multiple_strings() {
    let rec = build_attribute_fragment(1, [0; 8], &["A3", "DN80"]);
    let decoded = decode_attribute_fragments(&chain_stream(&rec));
    assert_eq!(decoded.len(), 1);
    let texts: Vec<&str> = decoded[0].strings.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(texts, vec!["A3", "DN80"]);
}

#[test]
fn attribute_fragment_rejects_control_char_string() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&1u32.to_le_bytes());
    payload.extend_from_slice(&[0u8; 8]);
    payload.extend_from_slice(&2u16.to_le_bytes());
    // U+0001 U+0002 — control chars, must reject the whole record.
    payload.extend_from_slice(&[0x01, 0x00, 0x02, 0x00]);
    let mut rec = Vec::new();
    rec.extend_from_slice(&PSM_TYPE_CODE_SUB_RECORD_0X0010.to_le_bytes());
    rec.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    rec.extend_from_slice(&payload);
    assert!(decode_attribute_fragment_at(&rec, 0).is_none());
}

#[test]
fn attribute_fragment_skips_truncated_length_prefix() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&1u32.to_le_bytes());
    payload.extend_from_slice(&[0u8; 8]);
    payload.extend_from_slice(&100u16.to_le_bytes());
    payload.extend_from_slice(&[0x41, 0x00]); // only 1 char available, len says 100
    let mut rec = Vec::new();
    rec.extend_from_slice(&PSM_TYPE_CODE_SUB_RECORD_0X0010.to_le_bytes());
    rec.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    rec.extend_from_slice(&payload);
    assert!(decode_attribute_fragment_at(&rec, 0).is_none());
}

#[test]
fn attribute_fragment_rejects_wrong_type_code() {
    let mut rec = build_attribute_fragment(1, [0; 8], &["A3"]);
    rec[0] = 0x18; // igLine2d type code
    assert!(decode_attribute_fragment_at(&rec, 0).is_none());
}

#[test]
fn attribute_fragment_rejects_too_short_payload() {
    // payload < marker(4)+aux(8)+len(2) = 14
    let payload = vec![0u8; 10];
    let mut rec = Vec::new();
    rec.extend_from_slice(&PSM_TYPE_CODE_SUB_RECORD_0X0010.to_le_bytes());
    rec.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    rec.extend_from_slice(&payload);
    assert!(decode_attribute_fragment_at(&rec, 0).is_none());
}

#[test]
fn attribute_fragment_at_returns_some_for_valid_offset() {
    let rec = build_attribute_fragment(0x0001_0002, [0; 8], &["DN80"]);
    let f = decode_attribute_fragment_at(&rec, 0).expect("valid offset must decode");
    assert_eq!(f.strings[0].text, "DN80");
    assert_eq!(f.byte_range, 0..rec.len());
}

#[test]
fn attribute_fragment_decoder_is_panic_safe() {
    let rec = build_attribute_fragment(
        0x0001_0002,
        [1, 2, 3, 4, 5, 6, 7, 8],
        &["ODOIL020150 MM", "DN80"],
    );
    for trunc in 0..rec.len() {
        assert!(decode_attribute_fragment_at(&rec[..trunc], 0).is_none());
        let _ = decode_attribute_fragments(&rec[..trunc]);
    }
    let noise: Vec<u8> = (0..4096u32).map(|i| (i & 0xFF) as u8).collect();
    let _ = decode_attribute_fragments(&noise);
    let _ = decode_attribute_fragments(&vec![0xFFu8; 2048]);
}

// Phase 18: PSM 0x0010 sub-record family audit-only decoder tests
// -----------------------------------------------------------------

fn build_synthetic_sub_record_0x0010(type_flags: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(6 + payload.len());
    let type_word = PSM_TYPE_CODE_SUB_RECORD_0X0010 | ((type_flags & 0x3) << 14);
    out.extend_from_slice(&type_word.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

#[test]
fn sub_record_0x0010_decodes_canonical_payload() {
    let payload: Vec<u8> = (0..16u8).collect();
    let record = build_synthetic_sub_record_0x0010(0, &payload);
    let decoded = decode_sub_records_0x0010(&chain_stream(&record));
    assert_eq!(decoded.len(), 1);
    let rec = &decoded[0];
    assert_eq!(
        rec.byte_range,
        SHEET_STREAM_HEADER_LEN..SHEET_STREAM_HEADER_LEN + 6 + payload.len()
    );
    assert_eq!(rec.type_code, PSM_TYPE_CODE_SUB_RECORD_0X0010);
    assert_eq!(rec.type_flags, 0);
    assert_eq!(rec.bytes_to_follow as usize, payload.len());
    assert_eq!(rec.raw_payload, payload);
    // Phase 19: leading_word = u16::from_le_bytes([payload[0],
    // payload[1]]) = LE(0x00, 0x01) = 0x0100.
    assert_eq!(rec.leading_word, Some(0x0100));
}

#[test]
fn sub_record_0x0010_decoded_at_returns_some_for_valid_offset() {
    let payload = vec![0xAAu8; 32];
    let record = build_synthetic_sub_record_0x0010(0, &payload);
    let rec = decode_sub_record_0x0010_at(&record, 0).expect("valid offset must decode");
    assert_eq!(rec.bytes_to_follow, 32);
    assert_eq!(rec.raw_payload.len(), 32);
}

#[test]
fn sub_record_0x0010_rejects_wrong_type_code() {
    let payload = vec![0u8; 16];
    let mut record = build_synthetic_sub_record_0x0010(0, &payload);
    // Flip type_code to 0x0018 (igLine2d).
    record[0] = 0x18;
    record[1] = 0x00;
    assert!(decode_sub_record_0x0010_at(&record, 0).is_none());
}

#[test]
fn sub_record_0x0010_rejects_zero_bytes_to_follow() {
    let payload = vec![];
    let record = build_synthetic_sub_record_0x0010(0, &payload);
    // bytes_to_follow = 0 < SUB_RECORD_0X0010_MIN_BYTES_TO_FOLLOW (8).
    assert!(decode_sub_record_0x0010_at(&record, 0).is_none());
}

#[test]
fn sub_record_0x0010_rejects_below_minimum_bytes_to_follow() {
    // bytes_to_follow = 7 < min (8). Must build a 13-byte buffer
    // so the header fits but payload is too small to be accepted.
    let mut record = Vec::with_capacity(6 + 7);
    record.extend_from_slice(&PSM_TYPE_CODE_SUB_RECORD_0X0010.to_le_bytes());
    record.extend_from_slice(&7u32.to_le_bytes());
    record.extend_from_slice(&[0u8; 7]);
    assert!(decode_sub_record_0x0010_at(&record, 0).is_none());
}

#[test]
fn sub_record_0x0010_rejects_oversized_bytes_to_follow() {
    // Encode a fake bytes_to_follow well above the cap; we only
    // need the header (6 bytes) — payload bounds will reject the
    // record anyway because data.len() is small.
    let mut record = Vec::with_capacity(6);
    record.extend_from_slice(&PSM_TYPE_CODE_SUB_RECORD_0X0010.to_le_bytes());
    record.extend_from_slice(&(SUB_RECORD_0X0010_MAX_BYTES_TO_FOLLOW + 1).to_le_bytes());
    let mut padded = record.clone();
    padded.extend(std::iter::repeat_n(0u8, 16));
    assert!(decode_sub_record_0x0010_at(&padded, 0).is_none());
}

#[test]
fn sub_record_0x0010_rejects_truncated_payload() {
    let payload = vec![0u8; 16];
    let record = build_synthetic_sub_record_0x0010(0, &payload);
    // Drop the last 4 payload bytes; bytes_to_follow header still
    // says 16 but only 12 are available.
    let truncated = &record[..record.len() - 4];
    assert!(decode_sub_record_0x0010_at(truncated, 0).is_none());
    assert!(decode_sub_records_0x0010(&chain_stream(truncated)).is_empty());
}

#[test]
fn sub_record_0x0010_returns_empty_for_short_or_empty_input() {
    assert!(decode_sub_records_0x0010(&[]).is_empty());
    // 13 bytes = exactly header (6) + min payload (8) - 1.
    assert!(decode_sub_records_0x0010(&[0u8; 13]).is_empty());
}

#[test]
fn sub_record_0x0010_decoder_is_panic_safe_on_short_input() {
    let payload = vec![0xCCu8; 16];
    let record = build_synthetic_sub_record_0x0010(0, &payload);
    for trunc_len in 0..record.len() {
        assert!(decode_sub_record_0x0010_at(&record[..trunc_len], 0).is_none());
        // Should not panic; truncated inputs return an empty vec.
        let _ = decode_sub_records_0x0010(&record[..trunc_len]);
    }
}

#[test]
fn sub_record_0x0010_decoder_is_panic_safe_on_random_noise() {
    let noise: Vec<u8> = (0..4096).map(|i| (i & 0xFF) as u8).collect();
    let _ = decode_sub_records_0x0010(&noise);
    assert!(decode_sub_records_0x0010(&vec![0u8; 4096]).is_empty());
    // All-0xFF stream never matches type_code 0x0010 (low 14 bits
    // would be 0x3FFF, not 0x0010).
    assert!(decode_sub_records_0x0010(&vec![0xFFu8; 4096]).is_empty());
}

#[test]
fn sub_record_0x0010_decodes_two_back_to_back_records() {
    let payload_a = vec![0xAAu8; 16];
    let payload_b = vec![0xBBu8; 24];
    let mut data = build_synthetic_sub_record_0x0010(0, &payload_a);
    data.extend_from_slice(&build_synthetic_sub_record_0x0010(0, &payload_b));
    let decoded = decode_sub_records_0x0010(&chain_stream(&data));
    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded[0].raw_payload, payload_a);
    assert_eq!(decoded[1].raw_payload, payload_b);
    assert!(decoded[0].byte_range.end <= decoded[1].byte_range.start);
}

#[test]
fn sub_record_0x0010_preserves_type_flags() {
    // The lower flag bit (type word 0x4000) rides through the envelope
    // as stored. No chain walk reads a record that carries it, so it is
    // read here where it lies.
    let payload = vec![0u8; 16];
    let record = build_synthetic_sub_record_0x0010(0b01, &payload);
    assert!(decode_sub_records_0x0010(&chain_stream(&record)).is_empty());
    let decoded = decode_sub_record_0x0010_at(&record, 0).expect("reads at an offset");
    assert_eq!(decoded.type_flags, 0b01);
    assert_eq!(decoded.type_code, PSM_TYPE_CODE_SUB_RECORD_0X0010);
}

// -----------------------------------------------------------------
// Phase 19: PSM 0x0010 leading_word audit field
// -----------------------------------------------------------------

#[test]
fn sub_record_0x0010_leading_word_matches_first_two_payload_bytes_le() {
    // Payload starts with the Phase-19-probe-confirmed dominant
    // leading word 0x0002 (28% of cross-fixture records).
    let mut payload = vec![0x02u8, 0x00];
    payload.extend(std::iter::repeat_n(0xAAu8, 14));
    let record = build_synthetic_sub_record_0x0010(0, &payload);
    let decoded = decode_sub_records_0x0010(&chain_stream(&record));
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].leading_word, Some(0x0002));

    // Same exercise with 0x0003 (the second-most-common leading
    // word per Phase 19 probe, 3.6% of records).
    let mut payload = vec![0x03u8, 0x00];
    payload.extend(std::iter::repeat_n(0xBBu8, 14));
    let record = build_synthetic_sub_record_0x0010(0, &payload);
    let decoded = decode_sub_records_0x0010(&chain_stream(&record));
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].leading_word, Some(0x0003));

    // Non-trivial byte-order check: payload starts with 0x1C 0x4E
    // → LE u16 = 0x4E1C (one of the size-16 bucket dominant words).
    let mut payload = vec![0x1Cu8, 0x4E];
    payload.extend(std::iter::repeat_n(0xCCu8, 14));
    let record = build_synthetic_sub_record_0x0010(0, &payload);
    let decoded = decode_sub_records_0x0010(&chain_stream(&record));
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].leading_word, Some(0x4E1C));
}

#[test]
fn sub_record_0x0010_leading_word_is_none_for_sub_two_byte_payload() {
    // The decoder enforces bytes_to_follow >= 8, so the only way
    // to observe `leading_word == None` is to construct the DTO
    // manually with a < 2 byte raw_payload. This tests that the
    // schema honors that contract (and protects against a future
    // refactor that drops the Option<>).
    let dto = SheetSubRecord0x0010Decoded {
        byte_range: 0..6,
        type_code: PSM_TYPE_CODE_SUB_RECORD_0X0010,
        type_flags: 0,
        bytes_to_follow: 0,
        raw_payload: Vec::new(),
        leading_word: None,
    };
    assert_eq!(dto.leading_word, None);
    let dto_one = SheetSubRecord0x0010Decoded {
        leading_word: None,
        raw_payload: vec![0x42],
        bytes_to_follow: 1,
        ..dto.clone()
    };
    assert_eq!(dto_one.leading_word, None);
}
