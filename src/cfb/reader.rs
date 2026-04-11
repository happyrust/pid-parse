use crate::api::ParseOptions;
use crate::error::PidError;
use crate::model::{PidDocument, StreamEntry};
use std::io::Read;
use std::path::Path;

pub fn parse_pid_file(path: &Path, options: &ParseOptions) -> Result<PidDocument, PidError> {
    let mut cfb = ::cfb::open(path)?;
    let tree = crate::cfb::tree::build_tree(&cfb, "/")?;
    let streams = collect_streams(&mut cfb, options)?;

    let mut doc = PidDocument {
        cfb_tree: tree,
        streams,
        ..PidDocument::default()
    };

    crate::streams::summary::parse_summary_streams(&mut cfb, &mut doc)?;

    if options.parse_xml {
        crate::streams::tagged_text::parse_tagged_text_streams(&mut cfb, &mut doc, options)?;
    }

    if options.parse_jsite_properties {
        crate::streams::jsite::parse_jsites(&mut cfb, &mut doc, options)?;
    }

    crate::streams::cluster::parse_clusters(&mut cfb, &mut doc, options)?;
    crate::streams::dynamic_attrs::parse_dynamic_attrs(&mut cfb, &mut doc, options)?;

    Ok(doc)
}

fn collect_streams<R: Read + std::io::Seek>(
    cfb: &mut ::cfb::CompoundFile<R>,
    options: &ParseOptions,
) -> Result<Vec<StreamEntry>, PidError> {
    let mut out = Vec::new();

    for entry in cfb.walk() {
        let entry = entry;
        if !entry.is_stream() {
            continue;
        }

        let path = entry.path().to_string_lossy().replace('\\', "/");
        let mut stream = cfb.open_stream(entry.path())?;
        let mut data = Vec::new();
        stream.read_to_end(&mut data)?;

        let preview_ascii = if options.scan_strings {
            crate::parsers::string_scan::scan_ascii_strings(&data, options.max_preview_strings)
        } else {
            vec![]
        };

        let magic_u32_le = data
            .get(0..4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));

        out.push(StreamEntry {
            path,
            size: data.len() as u64,
            preview_ascii,
            magic_u32_le,
        });
    }

    Ok(out)
}
