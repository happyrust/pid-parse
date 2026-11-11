//! The matchers of four retired record families, on the test side.
//!
//! `GLine2d` (`0x3FE6`), `JStyleOverride` as a sheet family (`0x0030`),
//! `SubRecord0x0010` and `AttributeFragment` (`0x0010`) were record families
//! of `pid_parse::parsers::sheet_records` until 2026-11-11. No chain of any
//! stream holds a record any of them reads: `0x3FE6` is not a type code, no
//! stream holds a `0x0010` record, and the `0x0030` records are in
//! `StyleCluster`, where `style_link` reads them by their own layout. What
//! the four reported were fields of other records, found by trying a matcher
//! at every byte offset of a sheet
//! (`docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
//! §3, §12, §17.2, §18).
//!
//! The library no longer exports the matchers: an entry point that reads
//! untrusted bytes and matches nothing a file holds is not one a caller
//! should find. They are kept here, as they were, for the tests and probes
//! that say what the old hits were — each hit is held to the record it sat
//! in, and that needs the rule that made the hit. A test includes this file
//! with `#[path = "support/retired_matchers.rs"]`, a probe with
//! `#[path = "../tests/support/retired_matchers.rs"]`; `tests/retired_matchers.rs`
//! holds the matchers' own rules.
//!
//! The doc comments below are the ones the items carried in the library,
//! history included; where one says "this module" or links an item by a
//! bare name, read `pid_parse::parsers::sheet_records`.
#![allow(dead_code)]

use pid_parse::parsers::sheet_records::{
    parse_live_psm_header, PsmHeader, PsmRecordDecoder, PSM_ENVELOPE_LEN,
    PSM_TYPE_CODE_JSTYLE_OVERRIDE,
};

// ---------------------------------------------------------------------------
// `GLine2d` (`0x3FE6`): constants
// ---------------------------------------------------------------------------

/// Byte length of the record header the two retired matchers read
/// ([`decode_primitive_line_at`], [`decode_jstyle_override_at`]).
///
/// Layout reverse-engineered from `radsrvitem.dll`
/// `PSMSerializeOut` (0x56491E80) / `PSMSerializeIn` (0x564915E0)
/// in
/// `docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md`:
///
/// ```text
/// 0..2    uint16_le  type_code         // 14-bit, top 2 bits flags
/// 2..6    uint32_le  bytes_to_follow   // payload length excl. these 6
/// 6..10   uint32_le  oid               // object identifier
/// 10..18  8 bytes    aux               // type-specific aux payload prefix
/// ```
pub const PSM_RECORD_HEADER_LEN: usize = 18;

/// Byte length of one in-memory `GLine2d` payload:
/// 6 × `f64` LE = 48 bytes.
///
/// Layout reverse-engineered from `radsrvitem.dll`
/// `GLine2d::Validate` (`sub_56524C50` @ 0x56524C50). Documented in
/// `docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md`.
pub const GLINE2D_PAYLOAD_LEN: usize = 48;

/// PSM type code the `GLine2d` `PrimitiveLine` decoder accepts.
///
/// **Retracted as a corpus identification, 2026-08-10.** The original
/// claim was that every record whose 18-byte PSM header is followed by 48
/// bytes matching the `GLine2d` validation rules carries
/// `type_code == 0x3FE6`. That is a tautology: the candidate set was
/// *defined* by those rules. Two independent tests both say no —
/// `radsrvitem.dll`'s authoritative type→class enumeration
/// (`sub_56448F70`) covers `0x06`..`0x117` and has no `0x3FE6`, and none
/// of the corpus's three former "records" sits on a record boundary. Each
/// sits 160 bytes inside an `igSmartFrame2d`, on the top two bytes of its
/// `1/√2` page-ratio `f64`. See
/// `docs/analysis/2026-08-10-gline2d-is-the-iso-page-ratio-not-a-record.md`.
///
/// The class is real — `radsrvitem.dll` has a `GLine2d::Validate`
/// (`sub_56524C50`) and the six-double parametric layout is its — but it
/// is a class of the geometry library, not one the reader serializes
/// records of, and `0x3FE6` is not its type code or anyone's: it is past
/// [`PSM_TYPE_CODE_LAST`], and a chain walk stops on a word no record
/// opens with ([`psm_type_word_opens_a_record`]).
///
/// **Retired as a sheet family, 2026-11-11.** Gated on chain membership
/// from 2026-08-10, the family decoded nothing on any corpus; with the
/// class-table tripwire it could decode nothing on any stream. The
/// registry row, the `SheetGeometry` field, the model DTO, the emitter
/// and the stream entry point are gone. The layout is still read at an
/// offset by [`decode_primitive_line_at`].
pub const PSM_TYPE_CODE_GLINE2D: u16 = 0x3FE6;

/// Unit-vector tolerance used when accepting a candidate `GLine2d`
/// record. The IDA-decoded `GLine2d::Validate` uses a strict
/// tolerance (`sub_56472D30()` returns ~1e-9 for normalized 2D),
/// but real `SmartPlant` writes can have direction vectors rounded
/// through coordinate transforms; 1e-3 catches every fixture record
/// we have without bringing in obvious false positives.
const GLINE2D_UNIT_VECTOR_TOLERANCE: f64 = 1e-3;

/// Sentinel beyond which an origin / parameter / direction value
/// is treated as garbage (uninitialized memory, out-of-domain).
const GLINE2D_COORDINATE_DOMAIN_LIMIT: f64 = 1e9;

/// One decoded PSM `GLine2d` `PrimitiveLine` record.
///
/// Phase 14 anti-promotion guarantee: this DTO carries the **raw
/// parametric** geometry (`origin + t * direction` for
/// `t ∈ [param_start, param_end]`), not Cartesian `start`/`end`
/// endpoints, and exposes a [`Self::byte_range`] covering the full
/// PSM record. Producing
/// [`crate::geometry::PidGeometryConfidence::Decoded`] is the
/// responsibility of `geometry.rs`; this module only decodes
/// bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetPrimitiveLineDecoded {
    /// Byte range covering the **entire** PSM record (header +
    /// inner payload + any trailing attribute bytes the writer
    /// included in `bytes_to_follow`).
    pub byte_range: std::ops::Range<usize>,
    /// PSM 14-bit type code from the record header. Always
    /// [`PSM_TYPE_CODE_GLINE2D`] for records this decoder emits.
    pub type_code: u16,
    /// Top 2 bits of the PSM type word (record-level flags).
    /// Documented as the `0x8000` / `0x4000` bits in
    /// `PSMSerializeIn`'s skip-record branch.
    pub type_flags: u16,
    /// `bytes_to_follow` field from the PSM header. The decoder
    /// uses this as the trailing edge of [`Self::byte_range`].
    pub bytes_to_follow: u32,
    /// `oid` field from the PSM header (the object identifier
    /// `SmartPlant` assigns to this geometry).
    pub oid: u32,
    /// Local-space origin: `point(t) = origin + t * direction`.
    pub origin: (f64, f64),
    /// Unit direction vector. `sqrt(direction_x^2 + direction_y^2)`
    /// is guaranteed within 1e-3 of 1.0 at decode time (a private
    /// `GLINE2D_UNIT_VECTOR_TOLERANCE` constant in this module).
    pub direction: (f64, f64),
    /// Parameter range start. `param_start < param_end` is
    /// guaranteed at decode time.
    pub param_start: f64,
    /// Parameter range end.
    pub param_end: f64,
}

impl SheetPrimitiveLineDecoded {
    /// Cartesian endpoint A computed from the parametric form
    /// (`origin + param_start * direction`).
    pub fn endpoint_a(&self) -> (f64, f64) {
        (
            self.origin.0 + self.param_start * self.direction.0,
            self.origin.1 + self.param_start * self.direction.1,
        )
    }

    /// Cartesian endpoint B computed from the parametric form
    /// (`origin + param_end * direction`).
    pub fn endpoint_b(&self) -> (f64, f64) {
        (
            self.origin.0 + self.param_end * self.direction.0,
            self.origin.1 + self.param_end * self.direction.1,
        )
    }
}

/// Try to read the `GLine2d` layout at `offset` in `data`: the matcher of
/// a family retired on 2026-11-11, kept for the test that says what its
/// hits were.
///
/// The bytes at `offset` are accepted when all of these hold:
///
/// 1. the type code (header bytes 0..2 LE, masked to 14 bits) is
///    [`PSM_TYPE_CODE_GLINE2D`];
/// 2. `bytes_to_follow` (header bytes 2..6 LE) is `>=` 48 and fits within
///    the remaining stream;
/// 3. all six payload `f64`s are finite and within `|x| <= 1e9`;
/// 4. `(direction.x, direction.y)` has unit length within `1e-3` and is
///    not the zero vector;
/// 5. `param_start < param_end` strictly.
///
/// The five are satisfiable by accident: a `[1 0; 0 1]` placement matrix
/// reads as a unit direction plus an ordered parameter pair, and `1/√2` —
/// the ISO A-series page ratio every `igSmartFrame2d` carries — is an
/// `f64` whose top two bytes are literally `E6 3F`. That is all this
/// matcher ever matched: tried at every offset of the corpora's sheets it
/// accepts seven runs of bytes, each 160 bytes into a page frame
/// (`docs/analysis/2026-08-10-gline2d-is-the-iso-page-ratio-not-a-record.md`).
/// No stream entry point walks it: `0x3FE6` is past
/// [`PSM_TYPE_CODE_LAST`], a chain walk stops on such a word
/// ([`psm_type_word_opens_a_record`]), and so no chain can hand it a
/// record. Bounds-checked: `offset >= data.len()` or a truncated tail
/// returns `None`.
///
/// Thin wrapper over [`GLine2dDecoder::decode_at`].
pub fn decode_primitive_line_at(data: &[u8], offset: usize) -> Option<SheetPrimitiveLineDecoded> {
    GLine2dDecoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for the `SmartPlant` extended
/// `GLine2d` family (PSM type `0x3FE6`).
///
/// The family deviation from the standard IGDS record shape lives
/// here, not in the shared seam: `GLine2d` uses the extended
/// [`PSM_RECORD_HEADER_LEN`] (18-byte) header carrying `oid` at
/// header bytes 6..10 (standard families carry `oid` in their payload
/// sub-header), and its `bytes_to_follow` may exceed the 48-byte
/// geometry payload — the trailing attribute bytes are included in
/// the decoded [`SheetPrimitiveLineDecoded::byte_range`] so
/// [`PsmRecordDecoder::scan`] jumps the whole record.
pub struct GLine2dDecoder;

impl PsmRecordDecoder for GLine2dDecoder {
    type Record = SheetPrimitiveLineDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_GLINE2D
    }

    fn min_record_len(&self) -> usize {
        PSM_RECORD_HEADER_LEN + GLINE2D_PAYLOAD_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetPrimitiveLineDecoded> {
        let envelope = parse_live_psm_header(data, offset)?;
        if envelope.type_code != PSM_TYPE_CODE_GLINE2D {
            return None;
        }
        let header_end = offset.checked_add(PSM_RECORD_HEADER_LEN)?;
        let payload_end = header_end.checked_add(GLINE2D_PAYLOAD_LEN)?;
        if payload_end > data.len() {
            return None;
        }
        let bytes_to_follow_usize = envelope.bytes_to_follow as usize;
        // bytes_to_follow must cover at least the 48-byte payload.
        if bytes_to_follow_usize < GLINE2D_PAYLOAD_LEN {
            return None;
        }
        // The full record (envelope + bytes_to_follow trailer) must fit.
        let record_end = offset.checked_add(PSM_ENVELOPE_LEN + bytes_to_follow_usize)?;
        if record_end > data.len() {
            return None;
        }
        decode_primitive_line_payload(data, offset, &envelope, header_end, payload_end, record_end)
    }

    fn advance_of(&self, record: &SheetPrimitiveLineDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// Family-specific payload validation for `GLine2d` (the 18-byte
/// extended header's `oid` plus the six-double parametric payload).
fn decode_primitive_line_payload(
    data: &[u8],
    offset: usize,
    envelope: &PsmHeader,
    header_end: usize,
    payload_end: usize,
    record_end: usize,
) -> Option<SheetPrimitiveLineDecoded> {
    let type_code = envelope.type_code;
    let type_flags = envelope.type_flags;
    let bytes_to_follow = envelope.bytes_to_follow;

    let header = data.get(offset..header_end)?;
    let oid = u32::from_le_bytes([header[6], header[7], header[8], header[9]]);

    // 6-double GLine2d payload at offset + 18.
    let payload = data.get(header_end..payload_end)?;
    let mut d = [0f64; 6];
    for (i, slot) in d.iter_mut().enumerate() {
        let chunk = payload.get(i * 8..i * 8 + 8)?;
        *slot = f64::from_le_bytes([
            chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
        ]);
    }
    if !d.iter().all(|x| x.is_finite()) {
        return None;
    }
    if d.iter().any(|x| x.abs() > GLINE2D_COORDINATE_DOMAIN_LIMIT) {
        return None;
    }
    let dir_x = d[2];
    let dir_y = d[3];
    if dir_x.abs() < 1e-12 && dir_y.abs() < 1e-12 {
        return None;
    }
    let unit_err = ((dir_x * dir_x + dir_y * dir_y).sqrt() - 1.0).abs();
    if unit_err > GLINE2D_UNIT_VECTOR_TOLERANCE {
        return None;
    }
    let param_start = d[4];
    let param_end = d[5];
    if param_start >= param_end {
        return None;
    }

    Some(SheetPrimitiveLineDecoded {
        byte_range: offset..record_end,
        type_code,
        type_flags,
        bytes_to_follow,
        oid,
        origin: (d[0], d[1]),
        direction: (dir_x, dir_y),
        param_start,
        param_end,
    })
}

// ---------------------------------------------------------------------------
// Phase 16 Slice D: PSM-encoded `JStyleOverride` decoder (PSM type `0x0030`)
//
// **Re-identification of Phase 14 §6.1 future-slice.**
//
// The PSM type code `0x0030` does **not** map to IGDS `GArc2d`. IDA
// reverse engineering (see `docs/analysis/2026-05-16-jstyleoverride-v3-fields.md`)
// proves that:
//
// 1. The PSM lookup table at `radsrvitem.dll!dword_5667B068[48]` resolves
//    `type_code == 0x0030` to CLSID
//    `{47FCC338-2D0F-11D0-A1FF-080036A1CF02}`.
// 2. The RAD framework CLSID registry in `JUTIL.dll` (file offset
//    `0x35680`) maps that CLSID to `style.dll` class **"JSL Override
//    Style"** (RAD friendly name).
// 3. The implementing C++ class name is **`JStyleOverride`** (from
//    `style.dll` RTTI), with inheritance chain `JStyleOverride →
//    JStyleR2d → JStyleBase`.
// 4. The on-disk serialization for the SmartPlant fixture set is the
//    **Version 3** path (`style.dll!sub_1000F030`), which writes exactly
//    13 `IOContext::DoIO` calls totalling **64 bytes**, matching the
//    fixture PSM payload exactly.
//
// Authoritative Version-3 byte layout:
//
// ```text
// disk +0..3   u32 (host this+22)
// disk +4..7   u32 (host this+24)
// disk +8..11  u32 (host this+25)
// disk +12..15 u32 (host this+38)
// disk +16..23 f64 (host this+26)
// disk +24..31 f64 (host this+28) — `rotation_angle` candidate
// disk +32..39 f64 (host this+30)
// disk +40..47 f64 (host this+34)
// disk +48..51 u32 (host this+32)
// disk +52..55 u32 (host this+47)
// disk +56..59 u32 (host this+48)
// disk +60..61 u16 (host this+36)
// disk +62..63 u16 (host byte+146)
// ```
//
// Phase 17 removed the historical Phase 14 `PrimitiveArc` compatibility
// decoder. PSM type `0x0030` is now represented solely by this
// `JStyleOverride` decoder on the parser surface.

/// Inner PSM payload length emitted by `JStyleOverride`
/// `IJPersistImp::Save` Version-3 path (`style.dll!sub_1000F030`).
/// Fixture PSM records always have `bytes_to_follow >= 64`; any
/// surplus belongs to the optional attribute / linkage tail
/// (`bytes_to_follow - 64`), which is *not* part of the `JStyleOverride`
/// payload itself.
pub const JSTYLE_OVERRIDE_PAYLOAD_LEN: usize = 64;
/// Minimum `bytes_to_follow` a `JStyleOverride` record can ship with
/// (= the 64-byte Version-3 payload alone). Larger values are valid
/// (fixture commonly ships 128 / 145 / 224 / 384 etc.) — the surplus
/// is the optional attribute tail.
pub const JSTYLE_OVERRIDE_MIN_BYTES_TO_FOLLOW: u32 = 64;
/// Upper bound on `bytes_to_follow` — guards against header-noise
/// hits. Fixture maximum observed is 384.
pub const JSTYLE_OVERRIDE_MAX_BYTES_TO_FOLLOW: u32 = 4096;
/// Coordinate-style f64 fields (`+16..23`, `+24..31`, `+32..39`,
/// `+40..47`) must satisfy `|x| <= JSTYLE_OVERRIDE_FIELD_DOMAIN_LIMIT`
/// to discard adversarial / out-of-range noise. The fixture-observed
/// maximum across all 4 doubles is ~10 (rotation angle `2π`), but the
/// limit is set generously to avoid rejecting future records that
/// might encode larger magnitudes (e.g. world-space coordinates).
pub const JSTYLE_OVERRIDE_FIELD_DOMAIN_LIMIT: f64 = 1.0e6;

/// One decoded PSM `0x0030` `JStyleOverride` record (RAD `style.dll`
/// CLSID `{47FCC338-2D0F-11D0-A1FF-080036A1CF02}`,
/// IDA-confirmed Version-3 IO path
/// `style.dll!sub_1000F030`).
///
/// All 13 fields written by the Save / Load Version-3 path are
/// exposed verbatim (4×u32 prefix + 4×f64 mid + 3×u32 + 2×u16). The
/// surplus attribute tail (`bytes_to_follow > 64`) is retained as
/// `raw_attribute_tail` for audit; its internal layout (plant tag,
/// linkage references, `1.0` markers, etc.) is documented as
/// hypothesis in
/// `docs/analysis/2026-05-15-garc2d-packed-int-tail.md` §11 but is
/// not part of the stable contract.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetJStyleOverrideDecoded {
    /// Byte range covering the full PSM record (header + payload +
    /// any attribute tail captured by `bytes_to_follow`).
    pub byte_range: std::ops::Range<usize>,
    /// PSM 14-bit type code. Always `0x0030`.
    pub type_code: u16,
    /// Top 2 bits of the PSM type word (record-level flags).
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header. `>= 64`. Values above
    /// 64 indicate an optional attribute tail.
    pub bytes_to_follow: u32,
    /// `oid` from the PSM header.
    pub oid: u32,
    /// Version-3 disk field at payload `+0..3` (host `this+22`).
    pub field_a_u32: u32,
    /// Version-3 disk field at payload `+4..7` (host `this+24`).
    pub field_b_u32: u32,
    /// Version-3 disk field at payload `+8..11` (host `this+25`).
    pub field_c_u32: u32,
    /// Version-3 disk field at payload `+12..15` (host `this+38`).
    pub field_d_u32: u32,
    /// Version-3 disk field at payload `+16..23` (host `this+26`),
    /// f64.
    pub field_1_f64: f64,
    /// Version-3 disk field at payload `+24..31` (host `this+28`),
    /// f64. **Rotation-angle candidate** — cross-fixture probe
    /// shows values cluster around `{0, π/2, 3π/2, 2π}`.
    pub field_2_f64: f64,
    /// Version-3 disk field at payload `+32..39` (host `this+30`),
    /// f64.
    pub field_3_f64: f64,
    /// Version-3 disk field at payload `+40..47` (host `this+34`),
    /// f64.
    pub field_4_f64: f64,
    /// Version-3 disk field at payload `+48..51` (host `this+32`).
    pub field_e_u32: u32,
    /// Version-3 disk field at payload `+52..55` (host `this+47`).
    pub field_f_u32: u32,
    /// Version-3 disk field at payload `+56..59` (host `this+48`).
    pub field_g_u32: u32,
    /// Version-3 disk field at payload `+60..61` (host `this+36`).
    pub field_h_u16: u16,
    /// Version-3 disk field at payload `+62..63` (host byte 146).
    pub field_i_u16: u16,
    /// Raw attribute tail bytes (`bytes_to_follow - 64`). Retained
    /// for audit only; internal layout is hypothesis pending more
    /// fixture coverage.
    pub raw_attribute_tail: Vec<u8>,
}

/// [`PsmRecordDecoder`] adapter for the RAD `JStyleOverride` family
/// (PSM type `0x0030`, `style.dll` Version-3 IO — **not** an arc; see
/// the Phase 16 reclassification note on
/// [`PSM_TYPE_CODE_JSTYLE_OVERRIDE`]).
///
/// Like `GLine2d`, this family uses the extended 18-byte
/// [`PSM_RECORD_HEADER_LEN`] header (`oid` at header bytes 6..10) and
/// carries a variable attribute tail inside `bytes_to_follow`; both
/// deviations live on this adapter.
///
/// **Retired as a sheet family, 2026-11-11.** No sheet chain of any corpus
/// carries this type code, so the family had no record to decode. The
/// 13 671 "records" it used to report — and emit as position-less
/// `ProbeOnly` entities — were what a byte-sliding walk made of the block
/// every `0x00FA` `DependencyObject` ends with, `<type u16> <selector u16>
/// <len u16>` followed by `len` bytes: a 48-byte block states its length
/// as `30 00`, which reads as this type word, and the block's first four
/// bytes then read as `bytes_to_follow`. All of them ran past the end of
/// the record they sat in
/// (`docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
/// §3.2, §12). The class is real and so is its type code; its records live
/// in a `StyleCluster`, where `style_link` reads them by their own
/// 90-byte layout.
///
/// The registry row, the `SheetGeometry` field, the model DTO, the emitter
/// and the stream entry point are gone. This matcher and
/// [`decode_jstyle_override_at`] stay, for one reason: the tests and
/// probes that re-find what the old scan matched hold each hit to the
/// record it sat in, and need the rule that made the hit.
pub struct JStyleOverrideDecoder;

impl PsmRecordDecoder for JStyleOverrideDecoder {
    type Record = SheetJStyleOverrideDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_JSTYLE_OVERRIDE
    }

    fn min_record_len(&self) -> usize {
        PSM_RECORD_HEADER_LEN + JSTYLE_OVERRIDE_PAYLOAD_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetJStyleOverrideDecoded> {
        let envelope = parse_live_psm_header(data, offset)?;
        if envelope.type_code != PSM_TYPE_CODE_JSTYLE_OVERRIDE {
            return None;
        }
        if envelope.type_flags != 0 {
            return None;
        }
        if !(JSTYLE_OVERRIDE_MIN_BYTES_TO_FOLLOW..=JSTYLE_OVERRIDE_MAX_BYTES_TO_FOLLOW)
            .contains(&envelope.bytes_to_follow)
        {
            return None;
        }
        let btf_usize = envelope.bytes_to_follow as usize;
        let record_end = offset.checked_add(PSM_ENVELOPE_LEN + btf_usize)?;
        if record_end > data.len() {
            return None;
        }
        let header_end = offset.checked_add(PSM_RECORD_HEADER_LEN)?;
        let payload_end = header_end.checked_add(JSTYLE_OVERRIDE_PAYLOAD_LEN)?;
        if payload_end > data.len() {
            return None;
        }
        decode_jstyle_override_payload(data, offset, &envelope, header_end, payload_end, record_end)
    }

    fn advance_of(&self, record: &SheetJStyleOverrideDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// Try to decode one PSM `0x0030` `JStyleOverride` record at `offset`.
///
/// Validation (only fields with IDA + cross-fixture probe evidence):
///
/// 1. PSM 14-bit type code == `0x0030` and type flags are zero.
/// 2. `bytes_to_follow` ∈ `[JSTYLE_OVERRIDE_MIN_BYTES_TO_FOLLOW,
///    JSTYLE_OVERRIDE_MAX_BYTES_TO_FOLLOW]`.
/// 3. The record body fits in `data`.
/// 4. All four payload f64 fields are finite and within
///    `|x| <= JSTYLE_OVERRIDE_FIELD_DOMAIN_LIMIT`.
///
/// This authoritative decoder does **not** apply the historical
/// `axis_a.y ≈ 0` constraint from the removed Phase 14 `PrimitiveArc`
/// compatibility path, which rejected ~51% of real `JStyleOverride`
/// records.
///
/// Thin wrapper over [`JStyleOverrideDecoder::decode_at`].
pub fn decode_jstyle_override_at(data: &[u8], offset: usize) -> Option<SheetJStyleOverrideDecoded> {
    JStyleOverrideDecoder.decode_at(data, offset)
}

/// Family-specific payload extraction for `JStyleOverride` (the
/// 18-byte extended header's `oid`, the 64-byte Version-3 field
/// block, and the raw attribute tail).
fn decode_jstyle_override_payload(
    data: &[u8],
    offset: usize,
    envelope: &PsmHeader,
    header_end: usize,
    payload_end: usize,
    record_end: usize,
) -> Option<SheetJStyleOverrideDecoded> {
    let type_code = envelope.type_code;
    let type_flags = envelope.type_flags;
    let bytes_to_follow = envelope.bytes_to_follow;

    let header = data.get(offset..header_end)?;
    let oid = u32::from_le_bytes([header[6], header[7], header[8], header[9]]);

    let payload = data.get(header_end..payload_end)?;

    let field_a_u32 = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let field_b_u32 = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
    let field_c_u32 = u32::from_le_bytes([payload[8], payload[9], payload[10], payload[11]]);
    let field_d_u32 = u32::from_le_bytes([payload[12], payload[13], payload[14], payload[15]]);

    let field_1_f64 = f64::from_le_bytes([
        payload[16],
        payload[17],
        payload[18],
        payload[19],
        payload[20],
        payload[21],
        payload[22],
        payload[23],
    ]);
    let field_2_f64 = f64::from_le_bytes([
        payload[24],
        payload[25],
        payload[26],
        payload[27],
        payload[28],
        payload[29],
        payload[30],
        payload[31],
    ]);
    let field_3_f64 = f64::from_le_bytes([
        payload[32],
        payload[33],
        payload[34],
        payload[35],
        payload[36],
        payload[37],
        payload[38],
        payload[39],
    ]);
    let field_4_f64 = f64::from_le_bytes([
        payload[40],
        payload[41],
        payload[42],
        payload[43],
        payload[44],
        payload[45],
        payload[46],
        payload[47],
    ]);

    if !field_1_f64.is_finite()
        || !field_2_f64.is_finite()
        || !field_3_f64.is_finite()
        || !field_4_f64.is_finite()
    {
        return None;
    }
    if field_1_f64.abs() > JSTYLE_OVERRIDE_FIELD_DOMAIN_LIMIT
        || field_2_f64.abs() > JSTYLE_OVERRIDE_FIELD_DOMAIN_LIMIT
        || field_3_f64.abs() > JSTYLE_OVERRIDE_FIELD_DOMAIN_LIMIT
        || field_4_f64.abs() > JSTYLE_OVERRIDE_FIELD_DOMAIN_LIMIT
    {
        return None;
    }

    let field_e_u32 = u32::from_le_bytes([payload[48], payload[49], payload[50], payload[51]]);
    let field_f_u32 = u32::from_le_bytes([payload[52], payload[53], payload[54], payload[55]]);
    let field_g_u32 = u32::from_le_bytes([payload[56], payload[57], payload[58], payload[59]]);
    let field_h_u16 = u16::from_le_bytes([payload[60], payload[61]]);
    let field_i_u16 = u16::from_le_bytes([payload[62], payload[63]]);

    let raw_attribute_tail = data.get(payload_end..record_end)?.to_vec();

    Some(SheetJStyleOverrideDecoded {
        byte_range: offset..record_end,
        type_code,
        type_flags,
        bytes_to_follow,
        oid,
        field_a_u32,
        field_b_u32,
        field_c_u32,
        field_d_u32,
        field_1_f64,
        field_2_f64,
        field_3_f64,
        field_4_f64,
        field_e_u32,
        field_f_u32,
        field_g_u32,
        field_h_u16,
        field_i_u16,
        raw_attribute_tail,
    })
}

// ---------------------------------------------------------------------------
// Phase 18: PSM 0x0010 sub-record family — audit-only decoder
//
// PSM type code 0x0010 is the most prevalent (638 cross-fixture hits)
// yet-unexplained record type after the Phase 14-17 typed decoders. probe
// evidence (`examples/probe_psm_0x0010_shape.rs`) shows highly polymorphic
// payload shapes — sizes range from ~13 bytes to ≥99 bytes, with multiple
// sub-kinds inside (some carry IEEE 754 doubles, others carry leading
// discriminators such as `02 00 01 00`). They are referenced from
// JStyleOverride `+38..41` (`referenced_oid_a`) and `+56..59`
// (`referenced_oid_c`); see
// `docs/analysis/2026-05-15-garc2d-packed-int-tail.md` §11.
//
// Until IDA reverse engineering confirms the class identity and the
// sub-kind discriminator, this decoder follows the Phase 15 DependencyObject
// audit-only template: stable 6-byte PSM header (`type_word + bytes_to_follow`,
// **not** the 18-byte IGDS header used by Phase 14 typed primitives) +
// raw payload + full provenance. NO sub-kind field naming, NO
// `PidGraphicKind` emission. Sub-kind decoders will land in a future phase.
//
// 2026-11-10: the paragraphs above describe hits of a byte-sliding walk,
// and what they describe is not a record family. Held against each
// stream's record chain, none of the 66 190 hits across the corpora starts
// a record: 99% sit on the `+12` of a graphic record, where the `u16`
// sub-type word is `0x0010` and the `u32` after it — read above as
// `bytes_to_follow` — is that record's style id. The "polymorphic payload
// shapes" are the tails of whichever record hosted the hit; the "leading
// discriminator `02 00 01 00`" is what an `igTextBox` stores at `+18`. No
// sheet chain carries a `0x0010` record, so the decoder below, which now
// reads the chain like every other family, returns nothing on every
// drawing measured. See
// `docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
// §3.1.

/// PSM type code the `0x0010` "sub-record" decoder accepts.
///
/// First categorized in
/// `docs/plans/2026-05-14-phase14-decoder-suite-final-summary.md` §6.3 as
/// "embedded sub-records / attribute fragments inside other record types"
/// on the strength of 638 hits of a byte-sliding walk over four fixtures.
/// Embedded they were — each one was a field of the record it sat in — and
/// no `Sheet*` chain holds a record of this type code.
pub const PSM_TYPE_CODE_SUB_RECORD_0X0010: u16 = 0x0010;

/// Minimum `bytes_to_follow` accepted by the conservative 0x0010 audit
/// decoder. Cross-fixture probe baseline shows the smallest observed
/// payload is 13 bytes; the threshold is set to 8 to leave headroom
/// while still rejecting trivially-small noise hits.
pub const SUB_RECORD_0X0010_MIN_BYTES_TO_FOLLOW: u32 = 8;

/// Upper bound on `bytes_to_follow` for 0x0010 records. Probe maximum
/// observed across the four-fixture set is ~99 bytes; the cap leaves
/// room for variant growth while rejecting wide-scan false positives
/// whose `bytes_to_follow` accidentally reads as a giant value.
pub const SUB_RECORD_0X0010_MAX_BYTES_TO_FOLLOW: u32 = 100_000;

/// One decoded PSM `0x0010` sub-record (audit-only).
///
/// This DTO exposes only the stable 6-byte PSM header and the variable
/// payload as raw bytes. The payload structure is **highly polymorphic**
/// (size 13..99+, multiple sub-kinds with different discriminators), so
/// per-field naming is deferred until IDA reverse engineering confirms
/// the class identity and sub-kind layout. The decoder is intentionally
/// permissive: it admits every byte sequence whose header satisfies
/// `type_code == 0x0010` and `bytes_to_follow` fits within the stream.
/// Some hits may be embedded fragments inside larger parent records
/// rather than standalone 0x0010 records — the audit collection
/// preserves both, leaving sub-kind disambiguation to a future phase.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetSubRecord0x0010Decoded {
    /// Byte range covering the full PSM record (6-byte header + payload).
    pub byte_range: std::ops::Range<usize>,
    /// PSM 14-bit type code. Always
    /// [`PSM_TYPE_CODE_SUB_RECORD_0X0010`] (`0x0010`).
    pub type_code: u16,
    /// Top 2 bits of the PSM type word (record-level flags).
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header. Equals `raw_payload.len()`
    /// by construction.
    pub bytes_to_follow: u32,
    /// Raw payload bytes (length = `bytes_to_follow`). Sub-kind
    /// discrimination and per-field decoding are deferred to a
    /// future phase.
    pub raw_payload: Vec<u8>,
    /// Audit-only Phase 19 field: `payload[0..2]` as little-endian
    /// `u16`. `None` when `raw_payload.len() < 2`.
    ///
    /// This is **not** a named sub-kind discriminator — Phase 19 probe
    /// (`examples/probe_psm_0x0010_sub_kind.rs`) showed that while
    /// `leading_word == 0x0002` accounts for ~28% of records and many
    /// size buckets are mono-valued at this position, several large
    /// buckets (size 31, 70, 13, 16, 43) are heterogeneous at +0 and
    /// likely encode coordinates or OIDs in the leading bytes rather
    /// than a sub-kind tag. The field name therefore describes only
    /// the byte position, not any semantic role; promotion to a typed
    /// `sub_kind` field is deferred pending IDA confirmation.
    pub leading_word: Option<u16>,
}

/// Try to decode a single PSM `0x0010` sub-record starting at `offset`
/// in `data`. Returns `None` when any validation rule of
/// [`SubRecord0x0010Decoder`] fails. Bounds-checked: passing
/// `offset >= data.len()` or a truncated tail returns `None`.
///
/// Thin wrapper over [`SubRecord0x0010Decoder::decode_at`].
pub fn decode_sub_record_0x0010_at(
    data: &[u8],
    offset: usize,
) -> Option<SheetSubRecord0x0010Decoded> {
    SubRecord0x0010Decoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for what was the polymorphic `0x0010`
/// sub-record family: it accepts the 6-byte PSM header when
/// `type_code == 0x0010` and `bytes_to_follow` satisfies the
/// `[SUB_RECORD_0X0010_MIN_BYTES_TO_FOLLOW,
/// SUB_RECORD_0X0010_MAX_BYTES_TO_FOLLOW]` envelope. Conservative and
/// panic-free.
///
/// **Retired as a sheet family, 2026-11-11.** No chain of any stream in
/// the corpora carries this type code. The 66 190 "sub-records" the family
/// used to report were what a byte-sliding walk made of a graphic record's
/// own payload: `+12` of an `igLine2d`, `igPoint2d`, `igTextBox`,
/// `igLineString2d` … is its sub-type word, which is `0x0010` on most of
/// them, and the `u32` after it is the record's style id, which the walk
/// read as `bytes_to_follow`. More than half ran past the end of the
/// record they sat in
/// (`docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
/// §3.1, §12). What each "sub-record" held is a field of its host, which
/// the host's own family decodes.
///
/// The registry row, the `SheetGeometry` field, the model DTO, the emitter
/// and the stream entry point are gone. This matcher and
/// [`decode_sub_record_0x0010_at`] stay for the tests and probes that
/// re-find what the old scan matched.
pub struct SubRecord0x0010Decoder;

impl PsmRecordDecoder for SubRecord0x0010Decoder {
    type Record = SheetSubRecord0x0010Decoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_SUB_RECORD_0X0010
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN.saturating_add(SUB_RECORD_0X0010_MIN_BYTES_TO_FOLLOW as usize)
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetSubRecord0x0010Decoded> {
        let header = parse_live_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_SUB_RECORD_0X0010 {
            return None;
        }
        if !(SUB_RECORD_0X0010_MIN_BYTES_TO_FOLLOW..=SUB_RECORD_0X0010_MAX_BYTES_TO_FOLLOW)
            .contains(&header.bytes_to_follow)
        {
            return None;
        }
        let btf = header.bytes_to_follow as usize;
        let payload_end = header.body_start.checked_add(btf)?;
        if payload_end > data.len() {
            return None;
        }
        let raw_payload = data.get(header.body_start..payload_end)?.to_vec();
        let leading_word = raw_payload
            .get(0..2)
            .map(|slot| u16::from_le_bytes([slot[0], slot[1]]));
        Some(SheetSubRecord0x0010Decoded {
            byte_range: offset..payload_end,
            type_code: header.type_code,
            type_flags: header.type_flags,
            bytes_to_follow: header.bytes_to_follow,
            raw_payload,
            leading_word,
        })
    }

    fn advance_of(&self, record: &SheetSubRecord0x0010Decoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

// Phase 26: PSM 0x0010 attribute-fragment decoder (additive, audit-only)
//
// 2026-05-31 analysis proved 0x0010 records carry SmartPlant attribute
// text (instrument tags / line numbers / nominal sizes / drawing refs /
// annotation labels) as length-prefixed UTF-16LE after a fixed
// `marker(4) + aux(8)` prefix. See
// `docs/analysis/2026-05-31-psm-0x0010-ida-recheck-plan.md`. This decoder
// is strictly additive: the raw Phase 18 `SubRecord0x0010Decoder`
// path is unchanged, and no `PidGraphicKind` is emitted.

/// One length-prefixed UTF-16LE string extracted from a PSM `0x0010`
/// attribute fragment (Phase 26).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedAttributeString {
    /// Offset (within the record payload) of the `u16` length word.
    pub len_offset: usize,
    /// Character count from the `u16` length prefix.
    pub char_count: u16,
    /// Decoded UTF-16LE text. Records whose tail is not clean UTF-16LE
    /// are rejected wholesale, so this is always valid Unicode.
    pub text: String,
}

/// A PSM `0x0010` record decoded as a `SmartPlant` **attribute fragment**
/// (Phase 26).
///
/// Carries engineering attribute text — instrument tags, line numbers,
/// nominal sizes, drawing references, annotation labels — as one or more
/// length-prefixed UTF-16LE strings after a fixed `marker(4) + aux(8)`
/// prefix. This is an **additive, audit-only** view that coexists with
/// the raw [`SheetSubRecord0x0010Decoded`] from Phase 18 (unchanged) and
/// emits no geometry kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetAttributeFragmentDecoded {
    /// Byte range covering the full PSM record (6-byte header + payload).
    pub byte_range: std::ops::Range<usize>,
    /// PSM 14-bit type code. Always
    /// [`PSM_TYPE_CODE_SUB_RECORD_0X0010`] (`0x0010`).
    pub type_code: u16,
    /// `payload[0..4]` as little-endian `u32`. A repeating **type
    /// marker** (frequently `0x00010002`), **not** a unique object id.
    pub marker: u32,
    /// `payload[4..12]` raw. 8-byte header/aux whose per-field semantics
    /// remain pending IDA confirmation; preserved verbatim for audit.
    pub aux: [u8; 8],
    /// Length-prefixed UTF-16LE strings parsed from `payload[12..]`.
    /// Non-empty by construction.
    pub strings: Vec<DecodedAttributeString>,
}

/// Payload offset where the first length-prefixed string begins
/// (`marker(4) + aux(8)`).
pub const ATTRIBUTE_FRAGMENT_STRING_START: usize = 12;

/// Upper bound on a single string's `u16` character count, guarding
/// against a bogus length prefix.
pub const ATTRIBUTE_FRAGMENT_MAX_CHAR_COUNT: u16 = 4096;

/// Try to decode a single PSM `0x0010` attribute fragment at `offset`.
/// Returns `None` when the header fails validation, the payload is too
/// short for `marker(4)+aux(8)+len`, or no clean UTF-16LE string is
/// found. Bounds-checked and panic-free.
///
/// Thin wrapper over [`AttributeFragmentDecoder::decode_at`].
pub fn decode_attribute_fragment_at(
    data: &[u8],
    offset: usize,
) -> Option<SheetAttributeFragmentDecoded> {
    AttributeFragmentDecoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for the Phase 26 attribute-fragment
/// **view** of the `0x0010` family (audit-only, additive): same PSM
/// envelope as [`SubRecord0x0010Decoder`], but the payload must parse
/// as `marker(4) + aux(8) + [u16 len + UTF-16LE]*` with at least one
/// clean non-empty string. Two adapters deliberately share one type
/// code here — the raw Phase 18 collection and this typed view coexist
/// by design.
///
/// **Retired as a sheet family, 2026-11-11**, with
/// [`SubRecord0x0010Decoder`]: the same envelope, so the same absence.
/// The 12 531 "fragments" this used to report were, all but 33, the
/// inside of a text box from its `+12` on — sub-type word, style id, then
/// the box's own length-prefixed UTF-16 text, which is what the strings
/// were; 303 of those boxes were superseded copies the native reader
/// skips. The text of a label is `igTextBox`'s to decode, and it does.
/// This matcher and [`decode_attribute_fragment_at`] stay for the tests
/// and probes that re-find what the old scan matched.
pub struct AttributeFragmentDecoder;

impl PsmRecordDecoder for AttributeFragmentDecoder {
    type Record = SheetAttributeFragmentDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_SUB_RECORD_0X0010
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN.saturating_add(SUB_RECORD_0X0010_MIN_BYTES_TO_FOLLOW as usize)
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetAttributeFragmentDecoded> {
        let header = parse_live_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_SUB_RECORD_0X0010 {
            return None;
        }
        if !(SUB_RECORD_0X0010_MIN_BYTES_TO_FOLLOW..=SUB_RECORD_0X0010_MAX_BYTES_TO_FOLLOW)
            .contains(&header.bytes_to_follow)
        {
            return None;
        }
        let btf = header.bytes_to_follow as usize;
        let payload_end = header.body_start.checked_add(btf)?;
        if payload_end > data.len() {
            return None;
        }
        decode_attribute_fragment_payload(data, offset, &header, payload_end)
    }

    fn advance_of(&self, record: &SheetAttributeFragmentDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// Family-specific payload validation for attribute fragments (the
/// `marker + aux + length-prefixed UTF-16LE strings` grammar).
fn decode_attribute_fragment_payload(
    data: &[u8],
    offset: usize,
    header: &PsmHeader,
    payload_end: usize,
) -> Option<SheetAttributeFragmentDecoded> {
    let type_code = header.type_code;

    let payload = data.get(header.body_start..payload_end)?;
    if payload.len() < ATTRIBUTE_FRAGMENT_STRING_START + 2 {
        return None;
    }
    let marker = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let mut aux = [0u8; 8];
    aux.copy_from_slice(payload.get(4..12)?);

    let mut strings = Vec::new();
    let mut pos = ATTRIBUTE_FRAGMENT_STRING_START;
    while pos + 2 <= payload.len() {
        let char_count = u16::from_le_bytes([payload[pos], payload[pos + 1]]);
        if char_count == 0 || char_count > ATTRIBUTE_FRAGMENT_MAX_CHAR_COUNT {
            break;
        }
        let text_start = pos + 2;
        let byte_len = (char_count as usize).checked_mul(2)?;
        let text_end = text_start.checked_add(byte_len)?;
        if text_end > payload.len() {
            break;
        }
        let Some(raw) = payload.get(text_start..text_end) else {
            break;
        };
        let Some(text) = decode_attribute_utf16le(raw) else {
            break;
        };
        if text.chars().all(char::is_whitespace) {
            break;
        }
        strings.push(DecodedAttributeString {
            len_offset: pos,
            char_count,
            text,
        });
        pos = text_end;
    }

    if strings.is_empty() {
        return None;
    }
    Some(SheetAttributeFragmentDecoded {
        byte_range: offset..payload_end,
        type_code,
        marker,
        aux,
        strings,
    })
}

/// Decode `bytes` as UTF-16LE, returning `None` on odd length, any
/// unpaired surrogate, or any control character other than space/tab.
fn decode_attribute_utf16le(bytes: &[u8]) -> Option<String> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let units = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes([c[0], c[1]]));
    let mut out = String::new();
    for unit in char::decode_utf16(units) {
        let ch = unit.ok()?;
        if ch.is_control() && ch != ' ' && ch != '\t' {
            return None;
        }
        out.push(ch);
    }
    Some(out)
}
