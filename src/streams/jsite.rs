//! Orchestrator for `JSite*` top-level storages.
//!
//! For every `JSite*` storage the reader finds, decodes symbol /
//! local-symbol paths, scans `\x01Ole` / `\x01CompObj` sub-streams
//! for OLE links, and hands the property blob to
//! [`crate::parsers::jproperties`]. Output lives under
//! [`PidDocument::jsites`].

use crate::config::ParseOptions;
use crate::error::PidError;
use crate::model::{EmbeddedStream, JSite, JSiteEmbeddedPrimitive, JSiteEmbeddedSheet, JSiteSymbolInformation, PidDocument};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::PathBuf;

/// Strip garbage prefix before a UNC path (`\\`) or drive letter path (`X:\`).
fn extract_unc_or_path(s: &str) -> String {
    if let Some(pos) = s.find("\\\\") {
        return s[pos..].to_string();
    }
    if let Some(pos) = s.find(":\\") {
        if pos > 0 {
            let drive_start = pos - 1;
            if s.as_bytes()
                .get(drive_start)
                .is_some_and(u8::is_ascii_alphabetic)
            {
                return s[drive_start..].to_string();
            }
        }
    }
    s.to_string()
}

/// Run the four `PSMcluster0` decoders of the symbol-information /
/// expression family over one site's cluster bytes.
fn decode_symbol_information_family(data: &[u8]) -> JSiteSymbolInformation {
    use crate::parsers::sheet_records::{
        decode_double_values, decode_standard_relations, decode_symbol_informations,
        decode_variables,
    };
    JSiteSymbolInformation {
        symbol_informations: decode_symbol_informations(data)
            .into_iter()
            .map(Into::into)
            .collect(),
        double_values: decode_double_values(data)
            .into_iter()
            .map(Into::into)
            .collect(),
        variable_groups: decode_variables(data).into_iter().map(Into::into).collect(),
        relations: decode_standard_relations(data)
            .into_iter()
            .map(Into::into)
            .collect(),
    }
}

fn u16_le(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_le(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn f64_le(data: &[u8], at: usize) -> Option<f64> {
    Some(f64::from_le_bytes(data.get(at..at + 8)?.try_into().ok()?))
}

/// Recover the drawable children of every persisted `JSheet` in a JSite.
/// The same record decoders used for top-level Sheet streams provide the
/// common families; circle and arc use their native fixed layouts, which are
/// identical to the proven `.sym` layouts.
fn decode_embedded_sheets(data: &[u8]) -> Vec<JSiteEmbeddedSheet> {
    use crate::parsers::sheet_records::{
        decode_iglines, decode_iglinestrings, decode_igpoints, decode_igtextboxes,
        sheet_record_starts,
    };

    let mut sheets = BTreeMap::<u32, Vec<JSiteEmbeddedPrimitive>>::new();
    for at in sheet_record_starts(data) {
        let Some(type_code) = u16_le(data, at).map(|word| word & 0x3fff) else { continue };
        let Some(len) = u32_le(data, at + 2).map(|value| value as usize) else { continue };
        let Some(payload) = data.get(at + 6..at + 6 + len) else { continue };
        if type_code == 0x0114 {
            if let Some(oid) = u32_le(payload, 0) {
                sheets.entry(oid).or_default();
            }
        }
    }

    for record in decode_iglines(data) {
        if let Some(body) = sheets.get_mut(&record.parent_ref) {
            body.push(JSiteEmbeddedPrimitive::Line { oid: record.oid, start: record.start, end: record.end });
        }
    }
    for record in decode_iglinestrings(data) {
        if let Some(body) = sheets.get_mut(&record.parent_ref) {
            body.push(JSiteEmbeddedPrimitive::Polyline { oid: record.oid, points: record.vertices, closed: record.form == 2 });
        }
    }
    for record in decode_igpoints(data) {
        if let Some(body) = sheets.get_mut(&record.parent_ref) {
            body.push(JSiteEmbeddedPrimitive::Point { oid: record.oid, position: record.point });
        }
    }
    for record in decode_igtextboxes(data) {
        if let Some(body) = sheets.get_mut(&record.parent_ref) {
            body.push(JSiteEmbeddedPrimitive::Text {
                oid: record.oid,
                value: record.text,
                insertion: (record.trailing_double_1, record.trailing_double_2),
                rotation: record.rotation_rad,
            });
        }
    }

    for at in sheet_record_starts(data) {
        let Some(type_code) = u16_le(data, at).map(|word| word & 0x3fff) else { continue };
        if !matches!(type_code, 0x0059 | 0x0061) { continue }
        let Some(len) = u32_le(data, at + 2).map(|value| value as usize) else { continue };
        let Some(payload) = data.get(at + 6..at + 6 + len) else { continue };
        let (Some(oid), Some(parent_ref)) = (u32_le(payload, 0), u32_le(payload, 4)) else { continue };
        let Some(body) = sheets.get_mut(&parent_ref) else { continue };
        let (Some(x), Some(y), Some(radius)) = (f64_le(payload, 18), f64_le(payload, 26), f64_le(payload, 34)) else { continue };
        if ![x, y, radius].into_iter().all(f64::is_finite) || radius <= 0.0 { continue }
        if type_code == 0x0059 {
            body.push(JSiteEmbeddedPrimitive::Circle { oid, center: (x, y), radius });
        } else if let (Some(start_angle), Some(end_angle)) = (f64_le(payload, 42), f64_le(payload, 50)) {
            if start_angle.is_finite() && end_angle.is_finite() {
                body.push(JSiteEmbeddedPrimitive::Arc { oid, center: (x, y), radius, start_angle, end_angle });
            }
        }
    }

    sheets.into_iter().filter_map(|(oid, primitives)| (!primitives.is_empty()).then_some(JSiteEmbeddedSheet { oid, primitives })).collect()
}

/// Decode every top-level `JSite*` storage into
/// [`PidDocument::jsites`]. Honors
/// [`ParseOptions::parse_jsite_properties`].
pub fn parse_jsites<R: Read + std::io::Seek>(
    cfb: &mut ::cfb::CompoundFile<R>,
    doc: &mut PidDocument,
    options: &ParseOptions,
) -> Result<(), PidError> {
    let mut names = BTreeSet::new();
    for s in &doc.streams {
        if let Some(first) = s.path.split('/').find(|v| !v.is_empty()) {
            if first.starts_with("JSite") {
                names.insert(first.to_string());
            }
        }
    }

    for name in names {
        let base = format!("/{name}");
        let mut site = JSite {
            name: name.clone(),
            path: base.clone(),
            ..JSite::default()
        };

        let prop_path = format!("{base}/JProperties");
        if let Ok(mut s) = cfb.open_stream(&prop_path) {
            let mut data = Vec::new();
            s.read_to_end(&mut data)?;
            site.properties = crate::parsers::jproperties::parse_jproperties(&data);
            for value in &site.properties.strings {
                if value.ends_with(".sym") && site.symbol_path.is_none() {
                    let clean = extract_unc_or_path(value);
                    site.symbol_path = Some(clean.clone());
                    site.symbol_name = std::path::Path::new(&clean)
                        .file_name()
                        .map(|v| v.to_string_lossy().to_string());
                }
            }
        }

        let ole_path = format!("{base}/\u{1}Ole");
        if let Ok(mut s) = cfb.open_stream(&ole_path) {
            let mut data = Vec::new();
            s.read_to_end(&mut data)?;
            site.has_ole_stream = true;
            site.ole_links = crate::parsers::string_scan::scan_ascii_strings(&data, 64);
        }

        // The symbol-information / expression family lives in this site's
        // own cluster, never in a `Sheet*` stream, so it is decoded here
        // rather than through the sheet pipeline.
        let cluster_path = format!("{base}/PSMcluster0");
        if let Ok(mut s) = cfb.open_stream(&cluster_path) {
            let mut data = Vec::new();
            s.read_to_end(&mut data)?;
            let decoded = decode_symbol_information_family(&data);
            if !decoded.is_empty() {
                site.symbol_information = Some(decoded);
            }
            site.embedded_sheets = decode_embedded_sheets(&data);
        }

        if options.keep_unknown_streams {
            let paths: Vec<PathBuf> = cfb
                .walk_storage(&base)?
                .filter(cfb::Entry::is_stream)
                .map(|entry| entry.path().to_path_buf())
                .collect();

            for path_buf in paths {
                let path = path_buf.to_string_lossy().replace('\\', "/");
                if path == prop_path || path == ole_path {
                    continue;
                }
                let mut stream = cfb.open_stream(&path_buf)?;
                let mut data = Vec::new();
                stream.read_to_end(&mut data)?;
                let name = path.rsplit('/').next().unwrap_or("").to_string();
                site.raw_streams.push(EmbeddedStream {
                    name,
                    size: data.len() as u64,
                    preview_ascii: crate::parsers::string_scan::scan_ascii_strings(&data, 16),
                });
            }
        }

        doc.jsites.push(site);
    }

    Ok(())
}
