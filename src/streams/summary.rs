use crate::error::PidError;
use crate::model::{PidDocument, SummaryInfo, SummaryPropertyValue};
use std::collections::BTreeMap;
use std::io::Read;

/// FMTID of `DocumentSummaryInformation` section 2 (user-defined
/// property dictionary). [MS-OLEPS] lists this as
/// `{D5CDD505-2E9C-101B-9397-08002B2CF9AE}` with Data1/Data2/Data3 in
/// little-endian and Data4 in big-endian — the same wire layout that
/// `FMTID_DOC_SUMMARY_SECTION_2` in the writer uses.
const FMTID_DOC_SUMMARY_SECTION_2: [u8; 16] = [
    0x05, 0xD5, 0xCD, 0xD5, 0x9C, 0x2E, 0x1B, 0x10, 0x93, 0x97, 0x08, 0x00, 0x2B, 0x2C, 0xF9, 0xAE,
];

pub fn parse_summary_streams<R: Read + std::io::Seek>(
    cfb: &mut ::cfb::CompoundFile<R>,
    doc: &mut PidDocument,
) -> Result<(), PidError> {
    let mut info = SummaryInfo::default();
    let mut found = false;

    if let Ok(mut s) = cfb.open_stream("/\x05SummaryInformation") {
        let mut data = Vec::new();
        s.read_to_end(&mut data)?;
        if let Some(props) = parse_property_set(&data) {
            map_summary_props(&props, &mut info);
            found = true;
        }
    }

    if let Ok(mut s) = cfb.open_stream("/\x05DocumentSummaryInformation") {
        let mut data = Vec::new();
        s.read_to_end(&mut data)?;
        if let Some(props) = parse_property_set(&data) {
            map_doc_summary_props(&props, &mut info);
            found = true;
        }
        // Phase 10j: walk section 2 (user-defined dictionary) if present.
        // The stream may have 1 section (section 1 only, common case) or 2
        // sections (section 1 + user dict). Missing / malformed section 2
        // is a silent no-op — user_properties just stays empty.
        info.user_properties = parse_doc_summary_section_2(&data);
    }

    if found {
        doc.summary = Some(info);
    }

    Ok(())
}

#[derive(Debug)]
enum PropValue {
    Str(String),
    FileTime(i64),
    I4(i32),
}

fn parse_property_set(data: &[u8]) -> Option<BTreeMap<u32, PropValue>> {
    if data.len() < 28 {
        return None;
    }

    let byte_order = u16_le(data, 0);
    if byte_order != 0xFFFE {
        return None;
    }

    let num_sections = u32_le(data, 24) as usize;
    if num_sections == 0 || data.len() < 28 + num_sections * 20 {
        return None;
    }

    // First section: skip 16-byte FMTID, then read offset
    let section_offset = u32_le(data, 44) as usize;
    parse_section(data, section_offset)
}

fn parse_section(data: &[u8], offset: usize) -> Option<BTreeMap<u32, PropValue>> {
    if offset + 8 > data.len() {
        return None;
    }

    let _section_size = u32_le(data, offset);
    let num_props = u32_le(data, offset + 4) as usize;

    let mut props = BTreeMap::new();
    let id_list_start = offset + 8;

    for i in 0..num_props {
        let entry = id_list_start + i * 8;
        if entry + 8 > data.len() {
            break;
        }
        let prop_id = u32_le(data, entry);
        let prop_offset = u32_le(data, entry + 4) as usize;
        let abs_offset = offset + prop_offset;

        if let Some(val) = read_typed_value(data, abs_offset) {
            props.insert(prop_id, val);
        }
    }

    Some(props)
}

fn read_typed_value(data: &[u8], offset: usize) -> Option<PropValue> {
    if offset + 4 > data.len() {
        return None;
    }

    let vt = u32_le(data, offset) & 0xFFFF;
    let val_start = offset + 4;

    match vt {
        0x001E => {
            // VT_LPSTR (code page string)
            if val_start + 4 > data.len() {
                return None;
            }
            let len = u32_le(data, val_start) as usize;
            if val_start + 4 + len > data.len() {
                return None;
            }
            let bytes = &data[val_start + 4..val_start + 4 + len];
            let s = bytes
                .iter()
                .take_while(|&&b| b != 0)
                .map(|&b| b as char)
                .collect::<String>();
            Some(PropValue::Str(s.trim().to_string()))
        }
        0x001F => {
            // VT_LPWSTR (UTF-16LE)
            if val_start + 4 > data.len() {
                return None;
            }
            let char_count = u32_le(data, val_start) as usize;
            let byte_len = char_count * 2;
            if val_start + 4 + byte_len > data.len() {
                return None;
            }
            let words: Vec<u16> = (0..char_count)
                .map(|i| u16_le(data, val_start + 4 + i * 2))
                .take_while(|&w| w != 0)
                .collect();
            let s = String::from_utf16_lossy(&words);
            Some(PropValue::Str(s.trim().to_string()))
        }
        0x0040 => {
            // VT_FILETIME
            if val_start + 8 > data.len() {
                return None;
            }
            let lo = u32_le(data, val_start) as i64;
            let hi = u32_le(data, val_start + 4) as i64;
            let ft = (hi << 32) | lo;
            Some(PropValue::FileTime(ft))
        }
        0x0003 => {
            // VT_I4
            if val_start + 4 > data.len() {
                return None;
            }
            Some(PropValue::I4(u32_le(data, val_start) as i32))
        }
        _ => None,
    }
}

/// Map standard `SummaryInformation` property IDs.
fn map_summary_props(props: &BTreeMap<u32, PropValue>, info: &mut SummaryInfo) {
    // PID_TITLE = 2
    if let Some(PropValue::Str(v)) = props.get(&2) {
        if !v.is_empty() {
            info.title = Some(v.clone());
        }
    }
    // PID_TEMPLATE = 7
    if let Some(PropValue::Str(v)) = props.get(&7) {
        if !v.is_empty() {
            info.template = Some(v.clone());
        }
    }
    // PID_APPNAME = 18
    if let Some(PropValue::Str(v)) = props.get(&18) {
        if !v.is_empty() {
            info.creating_application = Some(v.clone());
        }
    }
    // PID_CREATE_DTM = 12
    if let Some(PropValue::FileTime(ft)) = props.get(&12) {
        info.created_time = Some(filetime_to_string(*ft));
    }
    // PID_LASTSAVE_DTM = 13
    if let Some(PropValue::FileTime(ft)) = props.get(&13) {
        info.modified_time = Some(filetime_to_string(*ft));
    }

    for (&id, val) in props {
        let label = summary_prop_name(id);
        match val {
            PropValue::Str(s) if !s.is_empty() => {
                info.raw.insert(label, s.clone());
            }
            PropValue::FileTime(ft) => {
                info.raw.insert(label, filetime_to_string(*ft));
            }
            PropValue::I4(n) => {
                info.raw.insert(label, n.to_string());
            }
            _ => {}
        }
    }
}

/// Map `DocumentSummaryInformation` property IDs into the same `SummaryInfo`.
fn map_doc_summary_props(props: &BTreeMap<u32, PropValue>, info: &mut SummaryInfo) {
    for (&id, val) in props {
        let label = format!("DocSummary.{}", doc_summary_prop_name(id));
        match val {
            PropValue::Str(s) if !s.is_empty() => {
                info.raw.insert(label, s.clone());
            }
            PropValue::FileTime(ft) => {
                info.raw.insert(label, filetime_to_string(*ft));
            }
            PropValue::I4(n) => {
                info.raw.insert(label, n.to_string());
            }
            _ => {}
        }
    }
}

fn filetime_to_string(ft: i64) -> String {
    if ft <= 0 {
        return "(empty)".to_string();
    }
    // FILETIME: 100ns intervals since 1601-01-01
    const EPOCH_DIFF: i64 = 116_444_736_000_000_000;
    let unix_100ns = ft - EPOCH_DIFF;
    if unix_100ns < 0 {
        return format!("FILETIME({ft})");
    }
    let secs = unix_100ns / 10_000_000;
    let nanos = (unix_100ns % 10_000_000) * 100;
    if let Some(dt) = chrono_free_format(secs, nanos as u32) {
        dt
    } else {
        format!("FILETIME({ft})")
    }
}

fn chrono_free_format(unix_secs: i64, _nanos: u32) -> Option<String> {
    let days = unix_secs / 86400;
    let day_secs = (unix_secs % 86400) as u32;
    let h = day_secs / 3600;
    let m = (day_secs % 3600) / 60;
    let s = day_secs % 60;

    let (year, month, day) = civil_from_days(days);
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z"
    ))
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn summary_prop_name(id: u32) -> String {
    match id {
        1 => "CodePage".into(),
        2 => "Title".into(),
        3 => "Subject".into(),
        4 => "Author".into(),
        5 => "Keywords".into(),
        6 => "Comments".into(),
        7 => "Template".into(),
        8 => "LastAuthor".into(),
        9 => "RevNumber".into(),
        12 => "CreateTime".into(),
        13 => "LastSaveTime".into(),
        14 => "PageCount".into(),
        15 => "WordCount".into(),
        18 => "AppName".into(),
        _ => format!("Prop_{id}"),
    }
}

fn doc_summary_prop_name(id: u32) -> String {
    match id {
        1 => "CodePage".into(),
        2 => "Category".into(),
        14 => "Manager".into(),
        15 => "Company".into(),
        _ => format!("Prop_{id}"),
    }
}

fn u16_le(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([data[off], data[off + 1]])
}

fn u32_le(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

// ---------------------------------------------------------------------
// Phase 10j: DocumentSummaryInformation section 2 (user dict) decoder.
// ---------------------------------------------------------------------

/// Walk the property-set header, locate section 2, verify its FMTID
/// matches the user-defined dictionary one, and decode its named
/// properties into a `name → SummaryPropertyValue` map.
///
/// Returns an empty map on any of: stream too short, `num_sections < 2`,
/// section 2 FMTID mismatch, malformed dictionary, or section bounds
/// off the end of the stream. Reader-side is defensive — fidelity
/// guarantees live in the writer layer's round-trip tests.
fn parse_doc_summary_section_2(data: &[u8]) -> BTreeMap<String, SummaryPropertyValue> {
    let empty = BTreeMap::new();
    if data.len() < 28 {
        return empty;
    }
    let byte_order = u16_le(data, 0);
    if byte_order != 0xFFFE {
        return empty;
    }
    let num_sections = u32_le(data, 24) as usize;
    if num_sections < 2 {
        return empty;
    }
    // Section headers are 20 bytes each (16B FMTID + 4B offset),
    // appended right after the 28-byte PropertySetStream prefix.
    let sec2_header_off = 28 + 20; // section 0 consumes 20B, section 1 starts at 48
    if data.len() < sec2_header_off + 20 {
        return empty;
    }
    let sec2_fmtid = &data[sec2_header_off..sec2_header_off + 16];
    if sec2_fmtid != FMTID_DOC_SUMMARY_SECTION_2 {
        return empty;
    }
    let sec2_offset = u32_le(data, sec2_header_off + 16) as usize;
    decode_user_dict_section(data, sec2_offset)
}

fn decode_user_dict_section(data: &[u8], offset: usize) -> BTreeMap<String, SummaryPropertyValue> {
    let mut out = BTreeMap::new();
    if offset + 8 > data.len() {
        return out;
    }
    let _section_size = u32_le(data, offset);
    let num_props = u32_le(data, offset + 4) as usize;
    // Build the prop_id → offset table first so we can resolve Dictionary
    // (PROPID 0) before walking the user props.
    let mut entries: Vec<(u32, usize)> = Vec::with_capacity(num_props);
    let id_list_start = offset + 8;
    for i in 0..num_props {
        let e = id_list_start + i * 8;
        if e + 8 > data.len() {
            return out;
        }
        let prop_id = u32_le(data, e);
        let prop_off = u32_le(data, e + 4) as usize;
        entries.push((prop_id, offset + prop_off));
    }

    // Phase 10j MVP: assume LPSTR dictionary (the overwhelmingly common
    // case in SmartPlant-produced .pid fixtures). Phase 10k may extend
    // to LPWSTR dictionaries based on the CodePage property (PROPID 1).
    let mut propid_to_name: BTreeMap<u32, String> = BTreeMap::new();
    if let Some((_, dict_abs)) = entries.iter().find(|(id, _)| *id == 0) {
        propid_to_name = parse_dictionary_lpstr(data, *dict_abs);
    }
    if propid_to_name.is_empty() {
        return out;
    }

    for (prop_id, abs_off) in &entries {
        // PROPID 0 (Dictionary) and PROPID 1 (CodePage) are metadata,
        // not user-visible properties.
        if *prop_id == 0 || *prop_id == 1 {
            continue;
        }
        let Some(name) = propid_to_name.get(prop_id).cloned() else {
            continue;
        };
        if let Some(val) = read_user_value(data, *abs_off) {
            out.insert(name, val);
        }
    }
    out
}

/// Parse the Dictionary property body (PROPID 0).
///
/// Wire layout: `[NumEntries: u32][DictionaryEntry × NumEntries]`.
/// Each `DictionaryEntry` is `[PropID: u32][Length: u32][Name bytes]`
/// with 4-byte alignment between entries. For the LPSTR variant used
/// here, `Length` is a byte count that includes the NUL terminator, and
/// `Name` is raw single-byte characters. Phase 10j reads these as UTF-8
/// (reasonable default since `SmartPlant` dict names are uniformly
/// ASCII); Phase 10k will honor the section's `CodePage` property.
fn parse_dictionary_lpstr(data: &[u8], offset: usize) -> BTreeMap<u32, String> {
    let mut out = BTreeMap::new();
    if offset + 4 > data.len() {
        return out;
    }
    let num_entries = u32_le(data, offset) as usize;
    let mut cursor = offset + 4;
    for _ in 0..num_entries {
        if cursor + 8 > data.len() {
            break;
        }
        let prop_id = u32_le(data, cursor);
        let len = u32_le(data, cursor + 4) as usize;
        let name_start = cursor + 8;
        if name_start + len > data.len() {
            break;
        }
        let raw = &data[name_start..name_start + len];
        // Strip NUL terminator(s) before decoding.
        let stripped: Vec<u8> = raw.iter().copied().take_while(|b| *b != 0).collect();
        let name = match String::from_utf8(stripped) {
            Ok(s) => s,
            Err(_) => return out, // defensive: non-UTF-8 dict, bail
        };
        out.insert(prop_id, name);
        // Advance past the name bytes. Dictionary entries are NOT
        // 4-byte-aligned in the DocumentSummary LPSTR variant per
        // [MS-OLEPS] §2.16 — they pack tight. Empirically, the byte
        // count is exact.
        cursor = name_start + len;
    }
    out
}

fn read_user_value(data: &[u8], offset: usize) -> Option<SummaryPropertyValue> {
    if offset + 4 > data.len() {
        return None;
    }
    let vt = u32_le(data, offset) & 0xFFFF;
    let val_start = offset + 4;
    match vt as u16 {
        0x001E => {
            // VT_LPSTR
            if val_start + 4 > data.len() {
                return None;
            }
            let len = u32_le(data, val_start) as usize;
            if val_start + 4 + len > data.len() {
                return None;
            }
            let bytes = &data[val_start + 4..val_start + 4 + len];
            let text: String = bytes
                .iter()
                .take_while(|&&b| b != 0)
                .map(|&b| b as char)
                .collect();
            Some(SummaryPropertyValue::Lpstr(text))
        }
        0x001F => {
            // VT_LPWSTR
            if val_start + 4 > data.len() {
                return None;
            }
            let char_count = u32_le(data, val_start) as usize;
            let byte_len = char_count * 2;
            if val_start + 4 + byte_len > data.len() {
                return None;
            }
            let words: Vec<u16> = (0..char_count)
                .map(|i| u16_le(data, val_start + 4 + i * 2))
                .take_while(|&w| w != 0)
                .collect();
            Some(SummaryPropertyValue::Lpwstr(String::from_utf16_lossy(
                &words,
            )))
        }
        0x0003 => {
            // VT_I4
            if val_start + 4 > data.len() {
                return None;
            }
            Some(SummaryPropertyValue::I4(u32_le(data, val_start) as i32))
        }
        0x000B => {
            // VT_BOOL — stored as i16 0x0000 (false) / 0xFFFF (true)
            if val_start + 2 > data.len() {
                return None;
            }
            Some(SummaryPropertyValue::Bool(u16_le(data, val_start) != 0))
        }
        0x0040 => {
            // VT_FILETIME
            if val_start + 8 > data.len() {
                return None;
            }
            let lo = u32_le(data, val_start) as u64;
            let hi = u32_le(data, val_start + 4) as u64;
            Some(SummaryPropertyValue::Filetime((hi << 32) | lo))
        }
        other => {
            // Unknown VT — preserve as Raw for audit / future-proofing.
            // Best-effort length: grab the rest of the value area up to
            // a small cap so we don't swallow the whole stream on a
            // malformed VT tag.
            let grab_len = (data.len() - val_start).min(64);
            Some(SummaryPropertyValue::Raw {
                vt: other,
                bytes: data[val_start..val_start + grab_len].to_vec(),
            })
        }
    }
}

#[cfg(test)]
mod section2_tests {
    use super::*;

    /// Build a minimal `DocumentSummaryInformation` stream with exactly
    /// two sections:
    /// - section 1 (standard): one `VT_LPWSTR` Category = "CAT"
    /// - section 2 (user dict): Dictionary with one entry
    ///   (PROPID 4 → "`SP_ProjectID`") plus one `VT_LPWSTR` value
    ///   "PROJ-001".
    fn sample_docsummary_bytes() -> Vec<u8> {
        // ---- section 1 (FMTID_DOC_SUMMARY) ----
        // Props: Category @ PROPID 2 = VT_LPWSTR "CAT"
        let cat_bytes = {
            let mut v: Vec<u8> = Vec::new();
            v.extend_from_slice(&0x001Fu32.to_le_bytes()); // VT_LPWSTR
            v.extend_from_slice(&4u32.to_le_bytes()); // char count incl NUL
            for c in "CAT\0".encode_utf16() {
                v.extend_from_slice(&c.to_le_bytes());
            }
            // pad to 4-byte boundary (4 code units × 2 = 8, already 8B, no pad)
            v
        };
        let sec1_header_len = 8 + 8; // size+count (8B) + 1 × 8-byte entry
        let sec1_data_off = sec1_header_len;
        let sec1: Vec<u8> = {
            let mut v = Vec::new();
            let total = sec1_header_len + cat_bytes.len();
            v.extend_from_slice(&(total as u32).to_le_bytes()); // section size
            v.extend_from_slice(&1u32.to_le_bytes()); // num props
            v.extend_from_slice(&2u32.to_le_bytes()); // PROPID 2 = Category
            v.extend_from_slice(&(sec1_data_off as u32).to_le_bytes());
            v.extend_from_slice(&cat_bytes);
            v
        };

        // ---- section 2 (FMTID_DOC_SUMMARY_SECTION_2) ----
        // Props:
        //   PROPID 0 = Dictionary entry (1 entry: PROPID 4 → "SP_ProjectID")
        //   PROPID 4 = VT_LPWSTR "PROJ-001"
        let dict_body = {
            let mut v = Vec::new();
            v.extend_from_slice(&1u32.to_le_bytes()); // 1 dict entry
                                                      // DictionaryEntry: PropID=4, Length=13 ("SP_ProjectID\0"), Name bytes
            v.extend_from_slice(&4u32.to_le_bytes());
            let name = b"SP_ProjectID\0";
            v.extend_from_slice(&(name.len() as u32).to_le_bytes());
            v.extend_from_slice(name);
            v
        };
        let proj_bytes = {
            let mut v: Vec<u8> = Vec::new();
            v.extend_from_slice(&0x001Fu32.to_le_bytes()); // VT_LPWSTR
            v.extend_from_slice(&9u32.to_le_bytes()); // "PROJ-001\0" = 9 code units
            for c in "PROJ-001\0".encode_utf16() {
                v.extend_from_slice(&c.to_le_bytes());
            }
            v
        };
        let sec2_prop_count = 2;
        let sec2_header_len = 8 + sec2_prop_count * 8;
        let sec2: Vec<u8> = {
            let mut v = Vec::new();
            // id+offset table uses offsets within the section (0-based at section start).
            let dict_off = sec2_header_len;
            let proj_off = dict_off + dict_body.len();
            let total = sec2_header_len + dict_body.len() + proj_bytes.len();
            v.extend_from_slice(&(total as u32).to_le_bytes()); // size
            v.extend_from_slice(&(sec2_prop_count as u32).to_le_bytes()); // 2 props
            v.extend_from_slice(&0u32.to_le_bytes()); // PROPID 0 = Dictionary
            v.extend_from_slice(&(dict_off as u32).to_le_bytes());
            v.extend_from_slice(&4u32.to_le_bytes()); // PROPID 4 = user prop
            v.extend_from_slice(&(proj_off as u32).to_le_bytes());
            v.extend_from_slice(&dict_body);
            v.extend_from_slice(&proj_bytes);
            v
        };

        // ---- PropertySetStream prefix (28B) + 2 × (16B FMTID + 4B offset) = 68B ----
        let mut stream: Vec<u8> = Vec::new();
        stream.extend_from_slice(&0xFFFEu16.to_le_bytes()); // ByteOrder
        stream.extend_from_slice(&0u16.to_le_bytes()); // Version
        stream.extend_from_slice(&0u32.to_le_bytes()); // SystemIdentifier
        stream.extend_from_slice(&[0u8; 16]); // CLSID (zero)
        stream.extend_from_slice(&2u32.to_le_bytes()); // NumPropertySets = 2

        // FMTID 0 = FMTID_DOC_SUMMARY (section 1)
        const FMTID_DOC_SUMMARY: [u8; 16] = [
            0x02, 0xD5, 0xCD, 0xD5, 0x9C, 0x2E, 0x1B, 0x10, 0x93, 0x97, 0x08, 0x00, 0x2B, 0x2C,
            0xF9, 0xAE,
        ];
        stream.extend_from_slice(&FMTID_DOC_SUMMARY);
        let sec1_offset = 68u32;
        stream.extend_from_slice(&sec1_offset.to_le_bytes());

        // FMTID 1 = FMTID_DOC_SUMMARY_SECTION_2
        stream.extend_from_slice(&FMTID_DOC_SUMMARY_SECTION_2);
        let sec2_offset = sec1_offset + sec1.len() as u32;
        stream.extend_from_slice(&sec2_offset.to_le_bytes());

        // Sections body
        stream.extend_from_slice(&sec1);
        stream.extend_from_slice(&sec2);
        stream
    }

    #[test]
    fn section_2_decodes_user_defined_lpwstr_property() {
        let data = sample_docsummary_bytes();
        let out = parse_doc_summary_section_2(&data);
        assert_eq!(out.len(), 1);
        match out.get("SP_ProjectID") {
            Some(SummaryPropertyValue::Lpwstr(v)) => assert_eq!(v, "PROJ-001"),
            other => panic!("unexpected value: {other:?}"),
        }
    }

    #[test]
    fn single_section_stream_yields_empty_user_properties() {
        let data = {
            // Reuse sample but manually patch NumPropertySets = 1 so the
            // second FMTID + offset are treated as absent (still present
            // in bytes, but we won't parse them).
            let mut v = sample_docsummary_bytes();
            // NumPropertySets is at offset 24, little-endian u32.
            v[24] = 1;
            v[25] = 0;
            v[26] = 0;
            v[27] = 0;
            v
        };
        let out = parse_doc_summary_section_2(&data);
        assert!(out.is_empty(), "num_sections=1 → no section 2 to decode");
    }

    #[test]
    fn section_2_with_wrong_fmtid_is_ignored() {
        let mut data = sample_docsummary_bytes();
        // Corrupt the section 2 FMTID first byte.
        data[48] ^= 0xFF;
        let out = parse_doc_summary_section_2(&data);
        assert!(out.is_empty(), "FMTID mismatch should return empty map");
    }

    #[test]
    fn unknown_vt_in_section_2_falls_through_to_raw_variant() {
        // Construct a minimal section 2 where the user prop has an
        // exotic VT (e.g. VT_UI8 = 0x0015) not explicitly modeled.
        // Expect `SummaryPropertyValue::Raw { vt, bytes }`.
        let raw_vt = 0x0015u16;

        let dict_body = {
            let mut v = Vec::new();
            v.extend_from_slice(&1u32.to_le_bytes());
            v.extend_from_slice(&7u32.to_le_bytes()); // PROPID 7
            let name = b"CustomUI8\0";
            v.extend_from_slice(&(name.len() as u32).to_le_bytes());
            v.extend_from_slice(name);
            v
        };
        let ui8_bytes = {
            let mut v = Vec::new();
            v.extend_from_slice(&(raw_vt as u32).to_le_bytes());
            v.extend_from_slice(&1u64.to_le_bytes()); // VT_UI8 payload
            v
        };
        let sec2_prop_count = 2;
        let sec2_header_len = 8 + sec2_prop_count * 8;
        let sec2: Vec<u8> = {
            let mut v = Vec::new();
            let dict_off = sec2_header_len;
            let raw_off = dict_off + dict_body.len();
            let total = sec2_header_len + dict_body.len() + ui8_bytes.len();
            v.extend_from_slice(&(total as u32).to_le_bytes());
            v.extend_from_slice(&(sec2_prop_count as u32).to_le_bytes());
            v.extend_from_slice(&0u32.to_le_bytes());
            v.extend_from_slice(&(dict_off as u32).to_le_bytes());
            v.extend_from_slice(&7u32.to_le_bytes());
            v.extend_from_slice(&(raw_off as u32).to_le_bytes());
            v.extend_from_slice(&dict_body);
            v.extend_from_slice(&ui8_bytes);
            v
        };
        let sec1 = {
            // Empty section 1 (just size=8, num_props=0).
            let mut v = Vec::new();
            v.extend_from_slice(&8u32.to_le_bytes());
            v.extend_from_slice(&0u32.to_le_bytes());
            v
        };
        let mut stream: Vec<u8> = Vec::new();
        stream.extend_from_slice(&0xFFFEu16.to_le_bytes());
        stream.extend_from_slice(&0u16.to_le_bytes());
        stream.extend_from_slice(&0u32.to_le_bytes());
        stream.extend_from_slice(&[0u8; 16]);
        stream.extend_from_slice(&2u32.to_le_bytes());

        const FMTID_DOC_SUMMARY: [u8; 16] = [
            0x02, 0xD5, 0xCD, 0xD5, 0x9C, 0x2E, 0x1B, 0x10, 0x93, 0x97, 0x08, 0x00, 0x2B, 0x2C,
            0xF9, 0xAE,
        ];
        stream.extend_from_slice(&FMTID_DOC_SUMMARY);
        let sec1_offset = 68u32;
        stream.extend_from_slice(&sec1_offset.to_le_bytes());
        stream.extend_from_slice(&FMTID_DOC_SUMMARY_SECTION_2);
        let sec2_offset = sec1_offset + sec1.len() as u32;
        stream.extend_from_slice(&sec2_offset.to_le_bytes());
        stream.extend_from_slice(&sec1);
        stream.extend_from_slice(&sec2);

        let out = parse_doc_summary_section_2(&stream);
        match out.get("CustomUI8") {
            Some(SummaryPropertyValue::Raw { vt, bytes }) => {
                assert_eq!(*vt, raw_vt);
                assert!(bytes.len() >= 8, "should capture the payload bytes");
            }
            other => panic!("unexpected value: {other:?}"),
        }
    }

    #[test]
    fn malformed_header_yields_empty_map() {
        let data = [0u8; 10]; // way too short
        assert!(parse_doc_summary_section_2(&data).is_empty());
    }
}
