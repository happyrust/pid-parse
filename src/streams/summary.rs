use crate::error::PidError;
use crate::model::{PidDocument, SummaryInfo};
use std::collections::BTreeMap;
use std::io::Read;

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

/// Map standard SummaryInformation property IDs.
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

/// Map DocumentSummaryInformation property IDs into the same SummaryInfo.
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
        return format!("FILETIME({})", ft);
    }
    let secs = unix_100ns / 10_000_000;
    let nanos = (unix_100ns % 10_000_000) * 100;
    if let Some(dt) = chrono_free_format(secs, nanos as u32) {
        dt
    } else {
        format!("FILETIME({})", ft)
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
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, h, m, s
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
        _ => format!("Prop_{}", id),
    }
}

fn doc_summary_prop_name(id: u32) -> String {
    match id {
        1 => "CodePage".into(),
        2 => "Category".into(),
        14 => "Manager".into(),
        15 => "Company".into(),
        _ => format!("Prop_{}", id),
    }
}

fn u16_le(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([data[off], data[off + 1]])
}

fn u32_le(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}
