use crate::api::ParseOptions;
use crate::error::PidError;
use crate::model::{EmbeddedStream, JSite, PidDocument};
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
            if s.as_bytes().get(drive_start).is_some_and(|b| b.is_ascii_alphabetic()) {
                return s[drive_start..].to_string();
            }
        }
    }
    s.to_string()
}

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
        let base = format!("/{}", name);
        let mut site = JSite {
            name: name.clone(),
            path: base.clone(),
            ..JSite::default()
        };

        let prop_path = format!("{}/JProperties", base);
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

        let ole_path = format!("{}/\u{1}Ole", base);
        if let Ok(mut s) = cfb.open_stream(&ole_path) {
            let mut data = Vec::new();
            s.read_to_end(&mut data)?;
            site.has_ole_stream = true;
            site.ole_links = crate::parsers::string_scan::scan_ascii_strings(&data, 64);
        }

        if options.keep_unknown_streams {
            let paths: Vec<PathBuf> = cfb
                .walk_storage(&base)?
                .filter(|entry| entry.is_stream())
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
