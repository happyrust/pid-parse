//! Orchestrator for `JSite*` top-level storages.
//!
//! For every `JSite*` storage the reader finds, decodes symbol /
//! local-symbol paths, scans `\x01Ole` / `\x01CompObj` sub-streams
//! for OLE links, and hands the property blob to
//! [`crate::parsers::jproperties`]. Output lives under
//! [`PidDocument::jsites`].

use crate::config::ParseOptions;
use crate::error::PidError;
use crate::model::{
    EmbeddedStream, JSite, JSiteNestedGeometry, JSiteSymbolInformation, PidDocument,
};
use std::collections::BTreeSet;
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

/// Read the symbol bodies out of one definition-cache storage's cluster
/// bytes: every drawable record, chain-gated, plus the `JSheet` oids that
/// the placements name. See [`JSiteNestedGeometry`].
///
/// Every family is admitted the same way -- the record has to start where
/// the stream's own chain says a record starts -- so a byte pattern inside
/// one record's payload cannot be read as another record, and the five
/// families cannot disagree about which bytes are whose. Connect points are
/// not read: they draw nothing.
fn decode_nested_geometry(data: &[u8]) -> JSiteNestedGeometry {
    use crate::parsers::sheet_records::{
        decode_igarc_at, decode_igbspcurve_at, decode_igcircle_at, decode_igline_at,
        decode_iglinestring_at, decode_igrectangle_at, decode_igtextbox_at, jsheet_oids,
        sheet_record_starts,
    };
    let mut out = JSiteNestedGeometry {
        sheets: jsheet_oids(data),
        ..JSiteNestedGeometry::default()
    };
    for at in sheet_record_starts(data) {
        if let Some(circle) = decode_igcircle_at(data, at) {
            out.circles.push(circle.into());
        } else if let Some(arc) = decode_igarc_at(data, at) {
            out.arcs.push(arc.into());
        } else if let Some(line) = decode_igline_at(data, at) {
            out.lines.push(line.into());
        } else if let Some(polyline) = decode_iglinestring_at(data, at) {
            out.polylines.push(polyline.into());
        } else if let Some(text) = decode_igtextbox_at(data, at) {
            out.texts.push(text.into());
        } else if let Some(rectangle) = decode_igrectangle_at(data, at) {
            out.rectangles.push(rectangle.into());
        } else if let Some(curve) = decode_igbspcurve_at(data, at) {
            out.bsplines.push(curve.into());
        }
    }
    out
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
            let curves = decode_nested_geometry(&data);
            if !curves.is_empty() {
                site.nested_geometry = Some(curves);
            }
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
