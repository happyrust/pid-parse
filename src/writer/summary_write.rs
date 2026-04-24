//! Phase 9l — `SummaryInformation` / `DocumentSummaryInformation` property-set
//! writer.
//!
//! This module parses an OLE property-set stream (as defined by
//! \[MS-OLEPS\]), lets the caller edit a curated set of string properties,
//! and re-emits the stream with **byte-level round-trip fidelity** for
//! properties that were not touched. Properties in unsupported types
//! (FILETIME, I4, etc.) are preserved verbatim — we never decode and
//! re-encode them.
//!
//! Scope (see `docs/plans/2026-04-21-phase-9l-summary-info-writer.md`):
//! - **Can** edit `VT_LPSTR` / `VT_LPWSTR` properties by symbolic name
//!   (e.g. `"title"`, `"author"`, `"comments"`, `"category"`, …).
//! - **Can** append a supported property that was not in the source.
//! - **Cannot** edit non-string properties (returns
//!   `ReadOnlyPropType`).
//! - **Cannot** delete properties (future).
//! - **Cannot** cross encoding boundaries: writing non-ASCII into an
//!   existing `VT_LPSTR` property returns `EncodingMismatch`.
//!
//! All errors are wrapped into `PidError::ParseFailure { context: "summary
//! writer", message }` so the public `PidError` surface is unchanged.

use crate::error::PidError;
use crate::package::PidPackage;
use crate::writer::plan::EncodedString;
use std::collections::BTreeMap;

pub const SUMMARY_INFO_PATH: &str = "/\u{5}SummaryInformation";
pub const DOC_SUMMARY_PATH: &str = "/\u{5}DocumentSummaryInformation";

/// Symbolic key → PROPID mapping for `/\x05SummaryInformation` section.
///
/// Kept in declaration order so error messages can print a stable known-keys
/// list. Only string-typed properties are mapped here.
const KEY_TO_SUMMARY_PROPID: &[(&str, u32)] = &[
    ("title", 2),
    ("subject", 3),
    ("author", 4),
    ("keywords", 5),
    ("comments", 6),
    ("template", 7),
    ("last_author", 8),
    ("rev_number", 9),
    ("app_name", 18),
];

/// Symbolic key → PROPID mapping for `/\x05DocumentSummaryInformation`
/// section 1. Section 2 (user-defined) is intentionally out of scope.
const KEY_TO_DOC_SUMMARY_PROPID: &[(&str, u32)] =
    &[("category", 2), ("manager", 14), ("company", 15)];

// VT codes from MS-OLEPS.
const VT_I4: u16 = 0x0003;
const VT_LPSTR: u16 = 0x001E;
const VT_LPWSTR: u16 = 0x001F;
const VT_FILETIME: u16 = 0x0040;

/// Which OLE stream a symbolic key targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SummaryStream {
    Summary,
    DocumentSummary,
}

impl SummaryStream {
    fn path(self) -> &'static str {
        match self {
            Self::Summary => SUMMARY_INFO_PATH,
            Self::DocumentSummary => DOC_SUMMARY_PATH,
        }
    }
}

/// Resolve a caller-supplied symbolic key to (target stream, PROPID).
fn resolve_key(key: &str) -> Option<(SummaryStream, u32)> {
    for (name, pid) in KEY_TO_SUMMARY_PROPID {
        if *name == key {
            return Some((SummaryStream::Summary, *pid));
        }
    }
    for (name, pid) in KEY_TO_DOC_SUMMARY_PROPID {
        if *name == key {
            return Some((SummaryStream::DocumentSummary, *pid));
        }
    }
    None
}

/// All symbolic keys known to the writer, for error-message help text.
fn all_known_keys() -> Vec<&'static str> {
    KEY_TO_SUMMARY_PROPID
        .iter()
        .chain(KEY_TO_DOC_SUMMARY_PROPID.iter())
        .map(|(name, _)| *name)
        .collect()
}

/// A single property inside a section. `raw_value` contains the full encoded
/// form (VT tag + typed bytes + 4-byte alignment padding), ready to be
/// concatenated into a section body.
#[derive(Debug, Clone)]
struct SummaryProp {
    prop_id: u32,
    vt: u16,
    raw_value: Vec<u8>,
}

/// A property-set section. Keeps props in original file order so edits do
/// not perturb unrelated properties' offsets in the rebuilt bytes.
#[derive(Debug, Clone)]
struct SummarySection {
    fmtid: [u8; 16],
    props: Vec<SummaryProp>,
}

/// Entire property-set stream (one or more sections). The stream `header`
/// is preserved verbatim from the source; section offsets are recomputed on
/// serialize.
#[derive(Debug, Clone)]
struct SummaryPropertySet {
    /// Bytes 0..=27 of the source stream (`byte_order`, version, `system_id`,
    /// `class_id`, `num_sections`). We hold these verbatim to avoid accidental
    /// re-encoding drift.
    header: [u8; 28],
    sections: Vec<SummarySection>,
}

impl SummaryPropertySet {
    fn parse(data: &[u8]) -> Result<Self, PidError> {
        if data.len() < 28 {
            return Err(malformed("property-set stream shorter than 28-byte header"));
        }
        let mut header = [0u8; 28];
        header.copy_from_slice(&data[..28]);
        if u16_le(&header, 0) != 0xFFFE {
            return Err(malformed(&format!(
                "bad byte-order mark at offset 0: 0x{:04X} (want 0xFFFE)",
                u16_le(&header, 0),
            )));
        }
        let num_sections = u32_le(&header, 24) as usize;
        if num_sections == 0 {
            return Err(malformed("property-set declares zero sections"));
        }
        let section_table_end = 28 + num_sections * 20;
        if data.len() < section_table_end {
            return Err(malformed(&format!(
                "section table ({num_sections} × 20 bytes) overflows stream of length {}",
                data.len(),
            )));
        }

        let mut sections = Vec::with_capacity(num_sections);
        for i in 0..num_sections {
            let entry_start = 28 + i * 20;
            let mut fmtid = [0u8; 16];
            fmtid.copy_from_slice(&data[entry_start..entry_start + 16]);
            let sec_offset = u32_le(data, entry_start + 16) as usize;
            let section = parse_section(data, sec_offset)?;
            sections.push(SummarySection {
                fmtid,
                props: section,
            });
        }

        Ok(Self { header, sections })
    }

    fn find_section_mut(&mut self, stream: SummaryStream) -> Option<&mut SummarySection> {
        let want = match stream {
            SummaryStream::Summary => FMTID_SUMMARY,
            SummaryStream::DocumentSummary => FMTID_DOC_SUMMARY,
        };
        self.sections.iter_mut().find(|s| s.fmtid == want)
    }

    fn serialize(&self) -> Vec<u8> {
        let num_sections = self.sections.len();
        let table_start = 28usize;
        let section_offsets_base = table_start + num_sections * 20;

        // First pass: serialize each section body into an owned buffer, so
        // we can compute its offset + size and then emit the header table.
        let mut bodies: Vec<Vec<u8>> = Vec::with_capacity(num_sections);
        let mut offsets: Vec<u32> = Vec::with_capacity(num_sections);
        let mut cursor = section_offsets_base;
        for section in &self.sections {
            let body = section.serialize_body();
            offsets.push(cursor as u32);
            cursor += body.len();
            bodies.push(body);
        }

        let mut out = Vec::with_capacity(cursor);
        out.extend_from_slice(&self.header);
        // Update num_sections defensively in case an upstream caller mutated
        // `self.sections.len()` between parse and serialize (future).
        let current = u32_le(&self.header, 24) as usize;
        if current != num_sections {
            let patched = (num_sections as u32).to_le_bytes();
            let dst_start = out.len() - 4;
            out[dst_start..dst_start + 4].copy_from_slice(&patched);
        }
        for (section, offset) in self.sections.iter().zip(offsets.iter()) {
            out.extend_from_slice(&section.fmtid);
            out.extend_from_slice(&offset.to_le_bytes());
        }
        for body in bodies {
            out.extend_from_slice(&body);
        }
        out
    }
}

impl SummarySection {
    fn serialize_body(&self) -> Vec<u8> {
        let num_props = self.props.len();
        let table_size = 8 + num_props * 8; // section header (8) + id/offset list
        let mut data_area = Vec::<u8>::new();
        let mut offsets: Vec<u32> = Vec::with_capacity(num_props);
        for prop in &self.props {
            // Each prop's offset is relative to the start of *the section*,
            // so we pre-reserve `table_size` worth of space before the data
            // area begins.
            let rel_offset = (table_size + data_area.len()) as u32;
            offsets.push(rel_offset);
            data_area.extend_from_slice(&prop.raw_value);
            // `raw_value` is already aligned to 4 bytes — enforced at
            // construction time. Guard with an assertion in debug builds.
            debug_assert!(
                prop.raw_value.len().is_multiple_of(4),
                "raw_value for prop {} not 4-byte aligned ({} bytes)",
                prop.prop_id,
                prop.raw_value.len(),
            );
        }

        let total_size = (table_size + data_area.len()) as u32;
        let mut body = Vec::with_capacity(total_size as usize);
        body.extend_from_slice(&total_size.to_le_bytes());
        body.extend_from_slice(&(num_props as u32).to_le_bytes());
        for (prop, offset) in self.props.iter().zip(offsets.iter()) {
            body.extend_from_slice(&prop.prop_id.to_le_bytes());
            body.extend_from_slice(&offset.to_le_bytes());
        }
        body.extend_from_slice(&data_area);
        body
    }

    /// Phase 9n: remove the property with the given `prop_id` from this
    /// section. Returns `true` if a prop was actually removed, `false` if
    /// no matching prop was found (silent no-op). Preserves the relative
    /// ordering of the remaining props.
    fn remove(&mut self, prop_id: u32) -> bool {
        let before = self.props.len();
        self.props.retain(|p| p.prop_id != prop_id);
        self.props.len() != before
    }

    /// Replace or insert a property value. `new_value` is stored as the
    /// Rust string; this function encodes it per the target VT (preserving
    /// the original VT if the prop already existed, else defaulting to
    /// `VT_LPWSTR`).
    ///
    /// Uses UTF-8 for `VT_LPSTR` (Phase 10g default). For explicit code
    /// page control, see [`SummaryPropertySet::set_string_with_encoding`]
    /// (Phase 10i).
    fn set_string(&mut self, prop_id: u32, new_value: &str) -> Result<(), PidError> {
        self.set_string_with_encoding(prop_id, new_value, encoding_rs::UTF_8)
    }

    /// Phase 10i (v0.8.0+): same as [`set_string`] but uses `encoding`
    /// for `VT_LPSTR` properties (ignored for `VT_LPWSTR`).
    fn set_string_with_encoding(
        &mut self,
        prop_id: u32,
        new_value: &str,
        encoding: &'static encoding_rs::Encoding,
    ) -> Result<(), PidError> {
        if let Some(idx) = self.props.iter().position(|p| p.prop_id == prop_id) {
            let existing_vt = self.props[idx].vt;
            let encoded = encode_string_with_encoding(existing_vt, new_value, encoding, prop_id)?;
            self.props[idx] = SummaryProp {
                prop_id,
                vt: existing_vt,
                raw_value: encoded,
            };
        } else {
            // New property: pick VT_LPWSTR by default — it round-trips any
            // Unicode input losslessly and most SmartPlant consumers read
            // both VT tags. For VT_LPWSTR the `encoding` argument is
            // ignored (UTF-16LE is unambiguous).
            let encoded = encode_string_with_encoding(VT_LPWSTR, new_value, encoding, prop_id)?;
            self.props.push(SummaryProp {
                prop_id,
                vt: VT_LPWSTR,
                raw_value: encoded,
            });
        }
        Ok(())
    }
}

/// Phase 9l / 10g entry: encode `value` into the wire layout for property
/// type `vt`. `VT_LPSTR` uses UTF-8 bytes (10g default).
///
/// For explicit code page control (CP1252 / GBK / `Shift_JIS`, etc.), see
/// [`encode_string_with_encoding`] (Phase 10i).
///
/// This UTF-8 convenience wrapper is retained for test symmetry with
/// Phase 10g. Production paths use [`SummaryPropertySet::set_string`] /
/// [`SummaryPropertySet::set_string_with_encoding`], which call
/// [`encode_string_with_encoding`] directly.
#[cfg(test)]
fn encode_string(vt: u16, value: &str, prop_id_for_err: u32) -> Result<Vec<u8>, PidError> {
    encode_string_with_encoding(vt, value, encoding_rs::UTF_8, prop_id_for_err)
}

/// Phase 10i (v0.8.0+): encode `value` into the wire layout for property
/// type `vt`, using `encoding` for `VT_LPSTR` single-byte output.
///
/// - `VT_LPSTR`: `value` is encoded via `encoding`; characters that cannot
///   be represented in the chosen encoding fail fast (no silent
///   mojibake / `?` substitution).
/// - `VT_LPWSTR`: `encoding` is ignored. UTF-16LE is unambiguous.
/// - Other VTs: returns `ReadOnlyPropType`-style error.
fn encode_string_with_encoding(
    vt: u16,
    value: &str,
    encoding: &'static encoding_rs::Encoding,
    prop_id_for_err: u32,
) -> Result<Vec<u8>, PidError> {
    match vt {
        VT_LPSTR => {
            // VT_LPSTR: 4-byte VT tag, 4-byte **byte** count (incl NUL
            // terminator), bytes, then 4-byte alignment padding.
            //
            // [MS-OLEPS] §2.11 defines VT_LPSTR as a NUL-terminated
            // single-byte string in the code page of the creating
            // platform. Phase 10g (v0.7.0+) defaults to UTF-8. Phase 10i
            // (v0.8.0+) supports explicit code page via `encoding`
            // (UTF-8 / windows-1252 / GBK / Shift_JIS / …).
            let (encoded_bytes, _used_encoding, had_errors) = encoding.encode(value);
            if had_errors {
                return Err(lossy_encode_err(prop_id_for_err, encoding.name(), value));
            }
            let mut bytes: Vec<u8> = encoded_bytes.into_owned();
            bytes.push(0);
            let char_count = bytes.len() as u32;
            let mut out = Vec::with_capacity(8 + bytes.len() + 3);
            out.extend_from_slice(&(VT_LPSTR as u32).to_le_bytes());
            out.extend_from_slice(&char_count.to_le_bytes());
            out.extend_from_slice(&bytes);
            while !out.len().is_multiple_of(4) {
                out.push(0);
            }
            Ok(out)
        }
        VT_LPWSTR => {
            // VT_LPWSTR: 4-byte VT tag, 4-byte *character* count (UTF-16
            // code units, incl NUL terminator), UTF-16LE bytes, padded to
            // a 4-byte boundary. `encoding` argument is ignored — UTF-16LE
            // is unambiguous.
            let _ = encoding;
            let mut units: Vec<u16> = value.encode_utf16().collect();
            units.push(0);
            let char_count = units.len() as u32;
            let mut out = Vec::with_capacity(8 + units.len() * 2 + 2);
            out.extend_from_slice(&(VT_LPWSTR as u32).to_le_bytes());
            out.extend_from_slice(&char_count.to_le_bytes());
            for unit in units {
                out.extend_from_slice(&unit.to_le_bytes());
            }
            while !out.len().is_multiple_of(4) {
                out.push(0);
            }
            Ok(out)
        }
        other => Err(enc_err(
            prop_id_for_err,
            other,
            "summary_updates can only target VT_LPSTR (0x001E) or \
             VT_LPWSTR (0x001F) properties in the current release",
        )),
    }
}

/// Phase 10i: resolve an `encoding_rs` label (case-insensitive; WHATWG
/// aliases like "cp1252" / "windows-1252" both accepted).
fn resolve_encoding(label: &str) -> Result<&'static encoding_rs::Encoding, PidError> {
    encoding_rs::Encoding::for_label(label.as_bytes()).ok_or_else(|| PidError::ParseFailure {
        context: "summary writer".into(),
        message: format!(
            "summary_updates_encoded has unknown encoding label '{label}'; \
             expected an encoding_rs label like 'UTF-8', 'windows-1252', \
             'GBK', or 'Shift_JIS'"
        ),
    })
}

fn lossy_encode_err(prop_id: u32, encoding_name: &str, value: &str) -> PidError {
    PidError::ParseFailure {
        context: "summary writer".into(),
        message: format!(
            "cannot encode value for PROPID {prop_id} as {encoding_name}: \
             input contains characters not representable in that code page \
             (input was {value:?}); try a wider encoding like 'UTF-8' or \
             target a VT_LPWSTR property"
        ),
    }
}

/// Parse one section starting at absolute `offset` within the stream bytes.
fn parse_section(data: &[u8], offset: usize) -> Result<Vec<SummaryProp>, PidError> {
    if offset + 8 > data.len() {
        return Err(malformed(&format!(
            "section header at offset {offset} overruns stream"
        )));
    }
    let _section_size = u32_le(data, offset);
    let num_props = u32_le(data, offset + 4) as usize;
    let id_list_start = offset + 8;
    let id_list_end = id_list_start + num_props * 8;
    if id_list_end > data.len() {
        return Err(malformed(&format!(
            "section prop id/offset list overruns stream (section@{offset}, {num_props} props)"
        )));
    }

    let mut props = Vec::with_capacity(num_props);
    for i in 0..num_props {
        let entry = id_list_start + i * 8;
        let prop_id = u32_le(data, entry);
        let prop_offset_rel = u32_le(data, entry + 4) as usize;
        let val_start = offset + prop_offset_rel;
        if val_start + 4 > data.len() {
            return Err(malformed(&format!(
                "prop {prop_id} typed-value header at stream offset {val_start} overruns"
            )));
        }
        let vt = (u32_le(data, val_start) & 0xFFFF) as u16;
        let value_size = typed_value_size(vt, data, val_start + 4)?;
        let total = 4 + value_size;
        // Include trailing padding so raw_value is 4-byte aligned — the
        // next prop's (or end-of-section's) offset must also be 4-byte
        // aligned per OLEPS, so any trailing bytes between `val_start +
        // total` and the next aligned offset are padding we need to keep.
        let aligned_total = total.div_ceil(4) * 4;
        if val_start + aligned_total > data.len() {
            return Err(malformed(&format!(
                "prop {prop_id} aligned value (len={aligned_total}) overruns stream"
            )));
        }
        let raw_value = data[val_start..val_start + aligned_total].to_vec();
        props.push(SummaryProp {
            prop_id,
            vt,
            raw_value,
        });
    }

    Ok(props)
}

/// Compute the number of bytes (sans 4-byte VT tag) the typed value at
/// `offset` occupies. Caller combines this with the VT tag size (4) and
/// any 4-byte alignment padding to determine how much raw bytes to keep.
fn typed_value_size(vt: u16, data: &[u8], offset: usize) -> Result<usize, PidError> {
    match vt {
        VT_I4 => Ok(4),
        VT_FILETIME => Ok(8),
        VT_LPSTR => {
            if offset + 4 > data.len() {
                return Err(malformed("VT_LPSTR char count overruns"));
            }
            let len = u32_le(data, offset) as usize;
            Ok(4 + len)
        }
        VT_LPWSTR => {
            if offset + 4 > data.len() {
                return Err(malformed("VT_LPWSTR char count overruns"));
            }
            let count = u32_le(data, offset) as usize;
            Ok(4 + count * 2)
        }
        other => {
            // Unsupported-but-present types: we preserve the raw bytes
            // verbatim, so we need to know their length. Without a full
            // VT type table we fall back to "read the rest of the section
            // up to the next 4-byte alignment or end-of-stream". Rather
            // than guess, reject the whole property-set — the caller can
            // always stream_replacements-blob the full bytes through
            // untouched.
            Err(malformed(&format!(
                "unsupported VT type 0x{other:04X} at offset {offset}; \
                 only VT_I4 / VT_LPSTR / VT_LPWSTR / VT_FILETIME are \
                 recognized"
            )))
        }
    }
}

fn u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn malformed(msg: &str) -> PidError {
    PidError::ParseFailure {
        context: "summary writer".into(),
        message: format!("malformed property-set: {msg}"),
    }
}

fn unknown_key(key: &str) -> PidError {
    let known = all_known_keys().join(", ");
    PidError::ParseFailure {
        context: "summary writer".into(),
        message: format!("summary_updates has unknown key '{key}'; known keys: {known}"),
    }
}

fn read_only_type(key: &str, prop_id: u32, vt: u16) -> PidError {
    PidError::ParseFailure {
        context: "summary writer".into(),
        message: format!(
            "summary_updates['{key}'] targets PROPID {prop_id} which is \
             encoded as VT 0x{vt:04X} in the source; only VT_LPSTR (0x001E) \
             and VT_LPWSTR (0x001F) are writable in Phase 9l"
        ),
    }
}

fn enc_err(prop_id: u32, vt: u16, reason: &str) -> PidError {
    PidError::ParseFailure {
        context: "summary writer".into(),
        message: format!("encode error for PROPID {prop_id} (VT 0x{vt:04X}): {reason}"),
    }
}

fn stream_not_found(path: &str) -> PidError {
    PidError::ParseFailure {
        context: "summary writer".into(),
        message: format!(
            "summary_updates targets stream '{path}' which does not exist \
             in the source package; use stream_replacements to seed it"
        ),
    }
}

/// Standard OLE FMTIDs (GUIDs in little-endian `Data1/Data2/Data3` +
/// big-endian `Data4[8]`, i.e. the on-disk byte order for a CFB-embedded
/// property-set stream).
const FMTID_SUMMARY: [u8; 16] = [
    0xE0, 0x85, 0x9F, 0xF2, 0xF9, 0x4F, 0x68, 0x10, 0xAB, 0x91, 0x08, 0x00, 0x2B, 0x27, 0xB3, 0xD9,
];
const FMTID_DOC_SUMMARY: [u8; 16] = [
    0x02, 0xD5, 0xCD, 0xD5, 0x9C, 0x2E, 0x1B, 0x10, 0x93, 0x97, 0x08, 0x00, 0x2B, 0x2C, 0xF9, 0xAE,
];

/// Public entry: apply the declared `summary_updates` map to `package`.
///
/// Passthrough (empty map) is a fast no-op. Any non-empty map triggers a
/// parse → mutate → serialize round-trip on each affected stream. Streams
/// that are not targeted by any key remain byte-for-byte identical.
pub fn apply_summary_updates(
    package: &mut PidPackage,
    updates: &BTreeMap<String, String>,
) -> Result<(), PidError> {
    if updates.is_empty() {
        return Ok(());
    }

    // Group resolved updates by target stream.
    let mut by_stream: BTreeMap<SummaryStream, Vec<(u32, String, String)>> = BTreeMap::new();
    for (key, value) in updates {
        match resolve_key(key) {
            Some((stream, prop_id)) => {
                by_stream
                    .entry(stream)
                    .or_default()
                    .push((prop_id, key.clone(), value.clone()));
            }
            None => return Err(unknown_key(key)),
        }
    }

    for (stream, edits) in by_stream {
        let path = stream.path();
        let bytes = match package.get_stream(path) {
            Some(raw) => raw.data.clone(),
            None => return Err(stream_not_found(path)),
        };
        let mut set = SummaryPropertySet::parse(&bytes)?;
        let section = set
            .find_section_mut(stream)
            .ok_or_else(|| PidError::ParseFailure {
                context: "summary writer".into(),
                message: format!(
                    "stream '{path}' parsed successfully but does not contain the \
                     expected FMTID section; the stream may be a user-defined \
                     property set unsupported in Phase 9l"
                ),
            })?;

        for (prop_id, key, new_value) in edits {
            // Guard: if the prop already exists and is a read-only VT,
            // reject before we touch anything.
            if let Some(existing) = section.props.iter().find(|p| p.prop_id == prop_id) {
                match existing.vt {
                    VT_LPSTR | VT_LPWSTR => { /* writable */ }
                    other => return Err(read_only_type(&key, prop_id, other)),
                }
            }
            section.set_string(prop_id, &new_value)?;
        }

        let new_bytes = set.serialize();
        package.replace_stream(path.to_string(), new_bytes);
    }

    Ok(())
}

/// Phase 9n: apply property deletions by symbolic key.
///
/// Empty `deletions` is a free no-op. Each key resolves to a
/// (stream, PROPID) pair via the same symbolic table as
/// [`apply_summary_updates`]. Unknown keys return `UnknownKey` up-front
/// (before any side effect). Keys that exist in the symbolic table but
/// are not currently present in the section are **silent no-ops**,
/// matching the `stream_replacements` convention that "delete what is not
/// there" never fails.
///
/// If the source package lacks the targeted property-set stream entirely,
/// returns `StreamNotFound` — consistent with `apply_summary_updates`.
/// Caller can silence this by pre-filtering keys whose stream may be
/// absent.
pub fn apply_summary_deletions(
    package: &mut PidPackage,
    deletions: &[String],
) -> Result<(), PidError> {
    if deletions.is_empty() {
        return Ok(());
    }

    let mut by_stream: BTreeMap<SummaryStream, Vec<u32>> = BTreeMap::new();
    for key in deletions {
        match resolve_key(key) {
            Some((stream, prop_id)) => {
                by_stream.entry(stream).or_default().push(prop_id);
            }
            None => return Err(unknown_key(key)),
        }
    }

    for (stream, prop_ids) in by_stream {
        let path = stream.path();
        let bytes = match package.get_stream(path) {
            Some(raw) => raw.data.clone(),
            None => return Err(stream_not_found(path)),
        };
        let mut set = SummaryPropertySet::parse(&bytes)?;
        let section = set
            .find_section_mut(stream)
            .ok_or_else(|| PidError::ParseFailure {
                context: "summary writer".into(),
                message: format!(
                    "stream '{path}' parsed successfully but does not contain the \
                     expected FMTID section; the stream may be a user-defined \
                     property set unsupported in Phase 9l"
                ),
            })?;

        let mut any_removed = false;
        for prop_id in prop_ids {
            if section.remove(prop_id) {
                any_removed = true;
            }
        }

        // Skip re-writing when no prop was actually removed: the stream
        // bytes are byte-equal with the source, so `replace_stream` would
        // only mark it `modified: true` without any observable effect.
        if any_removed {
            let new_bytes = set.serialize();
            package.replace_stream(path.to_string(), new_bytes);
        }
    }

    Ok(())
}

/// Phase 10i (v0.8.0+): apply `summary_updates_encoded` to `package`.
///
/// Works exactly like [`apply_summary_updates`], but each value carries an
/// explicit `encoding_rs` label used when the target prop is `VT_LPSTR`.
/// `VT_LPWSTR` targets ignore the encoding hint (UTF-16LE is
/// unambiguous). Lossy encoding (input characters not representable in
/// the chosen code page) fails fast with a clear error rather than
/// silently substituting replacement characters.
///
/// Passthrough (empty map) is a fast no-op.
pub fn apply_summary_updates_encoded(
    package: &mut PidPackage,
    updates: &BTreeMap<String, EncodedString>,
) -> Result<(), PidError> {
    if updates.is_empty() {
        return Ok(());
    }

    #[allow(clippy::type_complexity)]
    let mut by_stream: BTreeMap<
        SummaryStream,
        Vec<(u32, String, String, &'static encoding_rs::Encoding)>,
    > = BTreeMap::new();
    for (key, es) in updates {
        let (stream, prop_id) = resolve_key(key).ok_or_else(|| unknown_key(key))?;
        let encoding = resolve_encoding(&es.encoding)?;
        by_stream.entry(stream).or_default().push((
            prop_id,
            key.clone(),
            es.value.clone(),
            encoding,
        ));
    }

    for (stream, edits) in by_stream {
        let path = stream.path();
        let bytes = match package.get_stream(path) {
            Some(raw) => raw.data.clone(),
            None => return Err(stream_not_found(path)),
        };
        let mut set = SummaryPropertySet::parse(&bytes)?;
        let section = set
            .find_section_mut(stream)
            .ok_or_else(|| PidError::ParseFailure {
                context: "summary writer".into(),
                message: format!(
                    "stream '{path}' parsed successfully but does not contain the \
                     expected FMTID section; the stream may be a user-defined \
                     property set unsupported in Phase 9l"
                ),
            })?;

        for (prop_id, key, new_value, encoding) in edits {
            if let Some(existing) = section.props.iter().find(|p| p.prop_id == prop_id) {
                match existing.vt {
                    VT_LPSTR | VT_LPWSTR => { /* writable */ }
                    other => return Err(read_only_type(&key, prop_id, other)),
                }
            }
            section.set_string_with_encoding(prop_id, &new_value, encoding)?;
        }

        let new_bytes = set.serialize();
        package.replace_stream(path.to_string(), new_bytes);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid `SummaryInformation` stream carrying exactly
    /// three props: a `VT_LPSTR` title, a `VT_LPWSTR` author, and a `VT_FILETIME`
    /// `create_time`. Layout inside the section:
    ///
    /// ```text
    ///   [ section header 8B | id+offset table 3*8B | data area (props) ]
    ///     ^0                   ^8                    ^32
    /// ```
    fn sample_summary_bytes() -> Vec<u8> {
        const TABLE_BASE: usize = 8 + 3 * 8; // props start after the table

        // 1. Pack each prop's raw_value (VT tag + value + 4-byte padding)
        //    and remember its offset from the start of the section.
        let mut data_area: Vec<u8> = Vec::new();
        let mut offsets: Vec<u32> = Vec::with_capacity(3);

        // prop 1: VT_LPSTR title = "Old"
        offsets.push((TABLE_BASE + data_area.len()) as u32);
        data_area.extend_from_slice(&(VT_LPSTR as u32).to_le_bytes());
        let title_bytes = b"Old\0";
        data_area.extend_from_slice(&(title_bytes.len() as u32).to_le_bytes());
        data_area.extend_from_slice(title_bytes);
        while !data_area.len().is_multiple_of(4) {
            data_area.push(0);
        }

        // prop 2: VT_LPWSTR author = "Jane"
        offsets.push((TABLE_BASE + data_area.len()) as u32);
        data_area.extend_from_slice(&(VT_LPWSTR as u32).to_le_bytes());
        let author_units: Vec<u16> = "Jane".encode_utf16().chain(std::iter::once(0)).collect();
        data_area.extend_from_slice(&(author_units.len() as u32).to_le_bytes());
        for u in &author_units {
            data_area.extend_from_slice(&u.to_le_bytes());
        }
        while !data_area.len().is_multiple_of(4) {
            data_area.push(0);
        }

        // prop 3: VT_FILETIME create_time (any 8 bytes; arbitrary sentinel).
        offsets.push((TABLE_BASE + data_area.len()) as u32);
        data_area.extend_from_slice(&(VT_FILETIME as u32).to_le_bytes());
        data_area.extend_from_slice(&0x01DA_94C0_0000_0000u64.to_le_bytes());

        // 2. Assemble the section: header + id/offset table + data area.
        let section_size = (TABLE_BASE + data_area.len()) as u32;
        let mut section: Vec<u8> = Vec::with_capacity(section_size as usize);
        section.extend_from_slice(&section_size.to_le_bytes());
        section.extend_from_slice(&3u32.to_le_bytes()); // num_props
        let ids: [u32; 3] = [2, 4, 12]; // PID_TITLE, PID_AUTHOR, PID_CREATE_DTM
        for (id, off) in ids.iter().zip(offsets.iter()) {
            section.extend_from_slice(&id.to_le_bytes());
            section.extend_from_slice(&off.to_le_bytes());
        }
        section.extend_from_slice(&data_area);

        // 3. Stream wrapper: 28-byte header + 1 section entry (20B) + section.
        let mut stream: Vec<u8> = Vec::new();
        stream.extend_from_slice(&0xFFFEu16.to_le_bytes()); // byte_order
        stream.extend_from_slice(&0u16.to_le_bytes()); // version
        stream.extend_from_slice(&0u32.to_le_bytes()); // system_id
        stream.extend_from_slice(&[0u8; 16]); // class_id (nil)
        stream.extend_from_slice(&1u32.to_le_bytes()); // num_sections
        stream.extend_from_slice(&FMTID_SUMMARY);
        stream.extend_from_slice(&48u32.to_le_bytes()); // section offset = 28 + 1*20
        stream.extend_from_slice(&section);
        stream
    }

    use crate::model::PidDocument;
    use crate::package::{PidPackage, RawStream};

    fn package_with_summary(data: Vec<u8>) -> PidPackage {
        let mut streams = BTreeMap::new();
        streams.insert(
            SUMMARY_INFO_PATH.to_string(),
            RawStream {
                path: SUMMARY_INFO_PATH.to_string(),
                data,
                modified: false,
            },
        );
        PidPackage::new(None, streams, PidDocument::default())
    }

    #[test]
    fn parse_then_serialize_is_byte_identical_for_untouched_stream() {
        let bytes = sample_summary_bytes();
        let parsed = SummaryPropertySet::parse(&bytes).expect("parse");
        let re = parsed.serialize();
        assert_eq!(
            re, bytes,
            "untouched parse → serialize must round-trip byte for byte"
        );
    }

    #[test]
    fn apply_summary_updates_passthrough_empty_map_touches_nothing() {
        let bytes = sample_summary_bytes();
        let mut pkg = package_with_summary(bytes.clone());
        apply_summary_updates(&mut pkg, &BTreeMap::new()).expect("ok");
        let after = &pkg.get_stream(SUMMARY_INFO_PATH).unwrap().data;
        assert_eq!(after, &bytes);
    }

    #[test]
    fn apply_summary_updates_edits_title_and_preserves_filetime() {
        let bytes = sample_summary_bytes();
        let original_ft_slice = {
            let parsed = SummaryPropertySet::parse(&bytes).unwrap();
            parsed.sections[0]
                .props
                .iter()
                .find(|p| p.prop_id == 12)
                .unwrap()
                .raw_value
                .clone()
        };

        let mut pkg = package_with_summary(bytes);
        let mut updates = BTreeMap::new();
        updates.insert("title".to_string(), "New Pipeline Review".to_string());
        apply_summary_updates(&mut pkg, &updates).expect("apply");

        let after = pkg.get_stream(SUMMARY_INFO_PATH).unwrap().data.clone();
        let reparsed = SummaryPropertySet::parse(&after).expect("reparse");
        let section = &reparsed.sections[0];

        let title_prop = section.props.iter().find(|p| p.prop_id == 2).unwrap();
        assert_eq!(
            title_prop.vt, VT_LPSTR,
            "title's original VT_LPSTR preserved"
        );
        // Decode the title bytes
        let char_count = u32_le(&title_prop.raw_value, 4) as usize;
        let title_bytes = &title_prop.raw_value[8..8 + char_count - 1]; // drop NUL
        assert_eq!(title_bytes, b"New Pipeline Review");

        let ft_prop = section.props.iter().find(|p| p.prop_id == 12).unwrap();
        assert_eq!(
            ft_prop.raw_value, original_ft_slice,
            "FILETIME prop must be byte-for-byte preserved"
        );
    }

    #[test]
    fn apply_summary_updates_rejects_unknown_key() {
        let bytes = sample_summary_bytes();
        let mut pkg = package_with_summary(bytes);
        let mut updates = BTreeMap::new();
        updates.insert("not_a_real_key".to_string(), "whatever".to_string());
        let err = apply_summary_updates(&mut pkg, &updates).expect_err("should reject");
        let msg = format!("{err}");
        assert!(msg.contains("unknown key"), "got: {msg}");
        assert!(msg.contains("not_a_real_key"), "got: {msg}");
    }

    #[test]
    fn apply_summary_updates_adds_new_string_prop_when_absent() {
        let bytes = sample_summary_bytes();
        let mut pkg = package_with_summary(bytes);
        let mut updates = BTreeMap::new();
        updates.insert("subject".to_string(), "Q4 Review".to_string());
        apply_summary_updates(&mut pkg, &updates).expect("apply");
        let after = pkg.get_stream(SUMMARY_INFO_PATH).unwrap().data.clone();
        let reparsed = SummaryPropertySet::parse(&after).expect("reparse");
        let subj = reparsed.sections[0]
            .props
            .iter()
            .find(|p| p.prop_id == 3)
            .expect("subject prop appended");
        assert_eq!(subj.vt, VT_LPWSTR, "new props default to VT_LPWSTR");
        let count = u32_le(&subj.raw_value, 4) as usize;
        let utf16: Vec<u16> = (0..count - 1)
            .map(|i| u16_le(&subj.raw_value, 8 + i * 2))
            .collect();
        assert_eq!(String::from_utf16(&utf16).unwrap(), "Q4 Review");
    }

    #[test]
    fn apply_summary_updates_returns_stream_not_found_when_missing() {
        let pkg = PidPackage::new(None, BTreeMap::new(), PidDocument::default());
        let mut pkg = pkg;
        let mut updates = BTreeMap::new();
        updates.insert("title".to_string(), "X".to_string());
        let err = apply_summary_updates(&mut pkg, &updates).expect_err("should fail");
        let msg = format!("{err}");
        assert!(msg.contains("does not exist"), "got: {msg}");
    }

    #[test]
    fn encode_lpstr_accepts_utf8_non_ascii() {
        // Phase 10g (v0.7.0+): VT_LPSTR now accepts arbitrary UTF-8.
        // The raw encoding is just `value.as_bytes()` plus a NUL
        // terminator and 4-byte alignment padding, so the char_count
        // field counts bytes (not code points).
        let encoded = encode_string(VT_LPSTR, "中文 title", 2).expect("accept utf-8");
        assert!(encoded.len().is_multiple_of(4), "must be 4-byte aligned");
        let vt = u32_le(&encoded, 0);
        assert_eq!(vt, VT_LPSTR as u32);
        // char_count = UTF-8 byte length + 1 (NUL).
        let count = u32_le(&encoded, 4) as usize;
        assert_eq!(count, "中文 title".len() + 1);
        // Bytes after the char_count + VT tag should be the UTF-8
        // encoding followed by a NUL.
        let body = &encoded[8..8 + count - 1];
        assert_eq!(std::str::from_utf8(body).unwrap(), "中文 title");
        // Phase 10g guarantees round-trippability through the
        // property-set parser at the byte level too: patch the title
        // (PROPID 2, LPSTR) to a UTF-8 Chinese string, re-parse, and
        // assert the source VT_LPSTR was preserved AND the bytes
        // decode back to the same UTF-8 string.
        let mut updates = BTreeMap::new();
        updates.insert("title".to_string(), "公司 Co. 中文".to_string());
        let mut pkg = package_with_summary(sample_summary_bytes());
        apply_summary_updates(&mut pkg, &updates).expect("utf-8 LPSTR write");
        let after = pkg.get_stream(SUMMARY_INFO_PATH).unwrap().data.clone();
        let reparsed = SummaryPropertySet::parse(&after).expect("reparse");
        let title_prop = reparsed.sections[0]
            .props
            .iter()
            .find(|p| p.prop_id == 2)
            .expect("title prop present");
        assert_eq!(title_prop.vt, VT_LPSTR, "source VT_LPSTR preserved");
        let count = u32_le(&title_prop.raw_value, 4) as usize;
        let text_bytes = &title_prop.raw_value[8..8 + count - 1];
        assert_eq!(std::str::from_utf8(text_bytes).unwrap(), "公司 Co. 中文");
    }

    #[test]
    fn encode_lpwstr_accepts_unicode() {
        let encoded = encode_string(VT_LPWSTR, "公司名 Co.", 15).expect("ok");
        assert!(encoded.len().is_multiple_of(4), "must be 4-byte aligned");
        let vt = u32_le(&encoded, 0);
        assert_eq!(vt, VT_LPWSTR as u32);
    }

    // ---------------------------------------------------------------
    // Phase 9n: summary_deletions coverage.
    // ---------------------------------------------------------------

    #[test]
    fn apply_summary_deletions_removes_existing_prop() {
        let bytes = sample_summary_bytes();
        let mut pkg = package_with_summary(bytes);
        let deletions = vec!["title".to_string()];
        apply_summary_deletions(&mut pkg, &deletions).expect("delete");

        let after = pkg.get_stream(SUMMARY_INFO_PATH).unwrap().data.clone();
        let reparsed = SummaryPropertySet::parse(&after).expect("reparse");
        assert!(
            reparsed.sections[0].props.iter().all(|p| p.prop_id != 2),
            "PID_TITLE (=2) must be gone after delete"
        );
        // Author (4) and create_time (12) still present.
        assert!(reparsed.sections[0].props.iter().any(|p| p.prop_id == 4));
        assert!(reparsed.sections[0].props.iter().any(|p| p.prop_id == 12));
    }

    #[test]
    fn apply_summary_deletions_nonexistent_key_is_silent_noop() {
        let bytes = sample_summary_bytes();
        // `subject` (PROPID 3) is not present in sample_summary_bytes.
        let mut pkg = package_with_summary(bytes.clone());
        let deletions = vec!["subject".to_string()];
        apply_summary_deletions(&mut pkg, &deletions).expect("silent no-op");
        // Stream must be byte-identical because no prop was actually
        // removed — we optimize the rewrite path in that case.
        let after = &pkg.get_stream(SUMMARY_INFO_PATH).unwrap().data;
        assert_eq!(after, &bytes, "passthrough on silent no-op deletion");
    }

    #[test]
    fn apply_summary_deletions_unknown_key_returns_error() {
        let bytes = sample_summary_bytes();
        let mut pkg = package_with_summary(bytes);
        let deletions = vec!["bogus_summary_key".to_string()];
        let err = apply_summary_deletions(&mut pkg, &deletions).expect_err("reject");
        let msg = format!("{err}");
        assert!(msg.contains("unknown key"), "got: {msg}");
        assert!(msg.contains("bogus_summary_key"), "got: {msg}");
    }

    #[test]
    fn apply_summary_deletions_empty_is_zero_cost_noop() {
        let bytes = sample_summary_bytes();
        let mut pkg = package_with_summary(bytes.clone());
        apply_summary_deletions(&mut pkg, &[]).expect("ok");
        let after = &pkg.get_stream(SUMMARY_INFO_PATH).unwrap().data;
        assert_eq!(after, &bytes, "empty slice = byte-identical passthrough");
    }

    #[test]
    fn apply_summary_deletions_preserves_filetime_byte_for_byte() {
        // Delete title (LPSTR); assert create_time (FILETIME) is
        // byte-for-byte identical afterwards. This mirrors the Phase 9l
        // write-side fidelity guarantee but for the delete path.
        let bytes = sample_summary_bytes();
        let original_ft = SummaryPropertySet::parse(&bytes).unwrap().sections[0]
            .props
            .iter()
            .find(|p| p.prop_id == 12)
            .unwrap()
            .raw_value
            .clone();

        let mut pkg = package_with_summary(bytes);
        apply_summary_deletions(&mut pkg, &["title".to_string()]).expect("delete");
        let after = pkg.get_stream(SUMMARY_INFO_PATH).unwrap().data.clone();
        let re_ft = SummaryPropertySet::parse(&after).unwrap().sections[0]
            .props
            .iter()
            .find(|p| p.prop_id == 12)
            .expect("create_time must still exist")
            .raw_value
            .clone();
        assert_eq!(
            re_ft, original_ft,
            "FILETIME prop must survive an adjacent-prop deletion unchanged"
        );
    }

    // -----------------------------------------------------------------
    // Phase 10i: summary_updates_encoded (explicit code page) coverage.
    // -----------------------------------------------------------------

    #[test]
    fn encode_lpstr_with_cp1252_preserves_western_european_bytes() {
        // "Øresund" contains Ø (U+00D8) which in CP1252 is 0xD8 (single
        // byte), vs UTF-8's two bytes (0xC3, 0x98). Verify the single-byte
        // layout is what writer emits when encoding=windows-1252.
        let encoded =
            encode_string_with_encoding(VT_LPSTR, "Øresund", encoding_rs::WINDOWS_1252, 2)
                .expect("CP1252 accepts Ø");
        assert!(encoded.len().is_multiple_of(4));
        let vt = u32_le(&encoded, 0);
        assert_eq!(vt, VT_LPSTR as u32);
        let char_count = u32_le(&encoded, 4) as usize;
        let payload = &encoded[8..8 + char_count - 1]; // strip NUL terminator
        assert_eq!(payload[0], 0xD8, "Ø encodes as single byte 0xD8 in CP1252");
        assert_eq!(&payload[1..], b"resund", "ASCII subset preserved verbatim");
    }

    #[test]
    fn encode_lpstr_with_cp1252_rejects_chinese_characters_lossy() {
        // CP1252 cannot represent Chinese; writer must fail fast, not
        // silently emit "?" characters.
        let err = encode_string_with_encoding(VT_LPSTR, "公司", encoding_rs::WINDOWS_1252, 2)
            .expect_err("CP1252 should reject Chinese");
        let msg = format!("{err}");
        assert!(msg.contains("cannot encode"), "got: {msg}");
        assert!(
            msg.contains("windows-1252"),
            "encoding name in error: {msg}"
        );
        // Value repr helps debug which prop failed.
        assert!(msg.contains("公司"), "offending value in error: {msg}");
    }

    #[test]
    fn encode_lpstr_with_gbk_preserves_simplified_chinese_bytes() {
        // "公司" in GBK is 4 bytes (2 per character).
        let encoded = encode_string_with_encoding(VT_LPSTR, "公司", encoding_rs::GBK, 2)
            .expect("GBK accepts Simplified Chinese");
        let char_count = u32_le(&encoded, 4) as usize;
        let payload = &encoded[8..8 + char_count - 1];
        assert_eq!(
            payload.len(),
            4,
            "公司 encodes to 4 bytes in GBK (2 per character)"
        );
        // Round-trip sanity: decode via encoding_rs and compare.
        let (decoded, _, had_errors) = encoding_rs::GBK.decode(payload);
        assert!(!had_errors, "GBK round-trip must be lossless");
        assert_eq!(decoded, "公司");
    }

    #[test]
    fn encode_lpwstr_ignores_encoding_hint() {
        // VT_LPWSTR is UTF-16LE regardless of the `encoding` argument.
        // Pass windows-1252 and assert the body is still UTF-16LE of the
        // full Unicode string (no code-page narrowing).
        let encoded = encode_string_with_encoding(VT_LPWSTR, "公司", encoding_rs::WINDOWS_1252, 15)
            .expect("VT_LPWSTR ignores encoding");
        let vt = u32_le(&encoded, 0);
        assert_eq!(vt, VT_LPWSTR as u32);
        let char_count = u32_le(&encoded, 4) as usize;
        assert_eq!(char_count, 3, "2 Chinese chars + NUL = 3 code units");
        let unit0 = u16_le(&encoded, 8);
        let unit1 = u16_le(&encoded, 10);
        assert_eq!(unit0, '公' as u16);
        assert_eq!(unit1, '司' as u16);
    }

    #[test]
    fn resolve_encoding_accepts_standard_and_alias_labels() {
        assert_eq!(resolve_encoding("UTF-8").unwrap().name(), "UTF-8");
        assert_eq!(
            resolve_encoding("windows-1252").unwrap().name(),
            "windows-1252"
        );
        // Alias: encoding_rs accepts "cp1252" as shorthand.
        assert_eq!(resolve_encoding("cp1252").unwrap().name(), "windows-1252");
        // Case-insensitive.
        assert_eq!(resolve_encoding("GBK").unwrap().name(), "GBK");
        assert_eq!(resolve_encoding("gbk").unwrap().name(), "GBK");
    }

    #[test]
    fn resolve_encoding_rejects_unknown_labels() {
        let err = resolve_encoding("Klingon-1").expect_err("unknown label");
        let msg = format!("{err}");
        assert!(msg.contains("unknown encoding label"), "got: {msg}");
        assert!(msg.contains("Klingon-1"), "label echoed: {msg}");
    }

    #[test]
    fn apply_summary_updates_encoded_end_to_end_rewrites_lpstr_with_explicit_codepage() {
        // Prove the full pipeline: plan map → apply → re-parse sees the
        // CP1252 bytes and the source VT_LPSTR is preserved.
        let mut pkg = package_with_summary(sample_summary_bytes());
        let mut updates: BTreeMap<String, EncodedString> = BTreeMap::new();
        updates.insert(
            "title".to_string(),
            EncodedString::new("Ø Pipe", "windows-1252"),
        );
        apply_summary_updates_encoded(&mut pkg, &updates).expect("apply");

        let after = pkg.get_stream(SUMMARY_INFO_PATH).unwrap().data.clone();
        let reparsed = SummaryPropertySet::parse(&after).expect("reparse");
        let title_prop = reparsed.sections[0]
            .props
            .iter()
            .find(|p| p.prop_id == 2)
            .expect("title prop present");
        assert_eq!(title_prop.vt, VT_LPSTR, "source VT_LPSTR preserved");
        let count = u32_le(&title_prop.raw_value, 4) as usize;
        let body = &title_prop.raw_value[8..8 + count - 1];
        assert_eq!(body[0], 0xD8, "Ø is single byte 0xD8 in CP1252");
        assert_eq!(&body[1..], b" Pipe");
    }

    #[test]
    fn apply_summary_updates_encoded_rejects_lossy_input_clearly() {
        let mut pkg = package_with_summary(sample_summary_bytes());
        let mut updates: BTreeMap<String, EncodedString> = BTreeMap::new();
        updates.insert(
            "title".to_string(),
            EncodedString::new("公司", "windows-1252"),
        );
        let err = apply_summary_updates_encoded(&mut pkg, &updates).expect_err("lossy");
        let msg = format!("{err}");
        assert!(msg.contains("cannot encode"), "got: {msg}");
        // Source stream must be unchanged since the apply failed before
        // rewrite (well-behaved error path).
        let after = &pkg.get_stream(SUMMARY_INFO_PATH).unwrap().data;
        assert_eq!(
            after,
            &sample_summary_bytes(),
            "lossy-encoding failure should not mutate source bytes"
        );
    }

    #[test]
    fn apply_summary_updates_encoded_unknown_key_errors_without_side_effect() {
        let mut pkg = package_with_summary(sample_summary_bytes());
        let mut updates: BTreeMap<String, EncodedString> = BTreeMap::new();
        updates.insert(
            "not_a_real_key".to_string(),
            EncodedString::new("x", "UTF-8"),
        );
        let err = apply_summary_updates_encoded(&mut pkg, &updates).expect_err("unknown");
        let msg = format!("{err}");
        assert!(msg.contains("unknown key"), "got: {msg}");
    }

    #[test]
    fn apply_summary_updates_encoded_unknown_encoding_errors_cleanly() {
        let mut pkg = package_with_summary(sample_summary_bytes());
        let mut updates: BTreeMap<String, EncodedString> = BTreeMap::new();
        updates.insert("title".to_string(), EncodedString::new("x", "Klingon-1"));
        let err = apply_summary_updates_encoded(&mut pkg, &updates).expect_err("unknown enc");
        let msg = format!("{err}");
        assert!(msg.contains("unknown encoding label"), "got: {msg}");
    }

    #[test]
    fn apply_summary_updates_encoded_empty_is_zero_cost_noop() {
        let bytes = sample_summary_bytes();
        let mut pkg = package_with_summary(bytes.clone());
        apply_summary_updates_encoded(&mut pkg, &BTreeMap::new()).expect("noop");
        let after = &pkg.get_stream(SUMMARY_INFO_PATH).unwrap().data;
        assert_eq!(after, &bytes, "empty map = byte-identical passthrough");
    }
}
