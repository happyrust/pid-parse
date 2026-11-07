//! Does a sheet's `.pid` carry the border its plot would draw?
//!
//! A sheet wears its border as an OLE-linked frame: a page `igSmartFrame2d`
//! record (`0x003D`) in a `Sheet*` stream, whose payload `+156` names the
//! `JSite` storage holding the link; that storage's `\x01Ole` stream with the
//! path of the `.igr` (or of a `.pid` drawing template); the root
//! `PSMcluster0`'s `0x004C` OLE client site for it; and the one object the
//! frame's layer counts (`DrawingBorder` or `Notes` in this corpus).
//! `pid-plot` picks a template from the frame's page extent or from the
//! link's file name, so a sheet that carries none of these is plotted bare.
//!
//! This prints each piece for every sheet given, read off the bytes rather
//! than through the decoders: `0x003D` envelopes are found by a sliding scan,
//! which can over-count but never misses one. With `--dump <Export.dmp>` it
//! also prints the plant database's `T_DRAWING.TEMPLATE` for the sheet — the
//! drawing template the sheet was created from, which the database keeps
//! whether or not the sheet still holds a frame.
//!
//! ```text
//! cargo run --example probe_does_the_sheet_link_a_border -- [--dump <Export.dmp>] <a.pid>...
//! ```
//!
//! Over the 53 sheets of `pid-test-data\backup-plant-data`, 50 carry a linked
//! border and three carry none: `D井场`, `换热器CD` and
//! `凝析油一级闪蒸分离器B气相出口改造`.

use std::io::Read;
use std::path::{Path, PathBuf};

use cfb::CompoundFile;
use pid_parse::parsers::cluster_header::{decode_cluster_body_record_at, ClusterBodyRecordDecoded};
use pid_parse::publish::oracle_exp::read_backup_drawings;
use pid_parse::PidParser;

const IGSMARTFRAME: u16 = 0x003D;
/// Shortest payload that reaches a page frame's extent fields.
const FRAME_MIN_PAYLOAD: usize = 156;
const FRAME_MAX_PAYLOAD: usize = 4096;
const OLE_CLIENT_SITE: u16 = 0x004C;
const OLE_SITES_MANAGER: u16 = 0x0004;

fn read_stream(cfb: &mut CompoundFile<std::fs::File>, path: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    cfb.open_stream(path).ok()?.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn f64_at(data: &[u8], at: usize) -> Option<f64> {
    Some(f64::from_le_bytes(data.get(at..at + 8)?.try_into().ok()?))
}

/// Every offset whose envelope states `0x003D` with a payload long enough to
/// be a page frame and short enough to be one record.
fn frame_envelopes(data: &[u8]) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    for at in 0..data.len().saturating_sub(6) {
        let word = u16::from_le_bytes([data[at], data[at + 1]]);
        if word & 0x3FFF != IGSMARTFRAME {
            continue;
        }
        let Some(follow) = u32_at(data, at + 2).map(|v| v as usize) else {
            continue;
        };
        if (FRAME_MIN_PAYLOAD..=FRAME_MAX_PAYLOAD).contains(&follow)
            && at + 6 + follow <= data.len()
        {
            found.push((at, follow));
        }
    }
    found
}

/// Where the root `PSMcluster0` string table ends: its `{u32 index, u32 byte
/// length, UTF-16}` entries stop at the first word too large to be an index.
fn string_table_end(data: &[u8]) -> Option<usize> {
    for start in 16..data.len().min(256) {
        let mut at = start;
        let mut entries = 0;
        while let (Some(index), Some(len)) = (u32_at(data, at), u32_at(data, at + 4)) {
            let len = len as usize;
            if index == 0 || index > 64 || len > 512 || !len.is_multiple_of(2) {
                break;
            }
            let Some(text) = data.get(at + 8..at + 8 + len) else {
                break;
            };
            if !text
                .as_chunks::<2>()
                .0
                .iter()
                .all(|unit| (0x20..0x7F).contains(&unit[0]) && unit[1] == 0)
            {
                break;
            }
            at += 8 + len;
            entries += 1;
        }
        if entries >= 2 {
            return Some(at);
        }
    }
    None
}

/// The root `PSMcluster0` records after the string table. The chain has to
/// run exactly to the end of the stream; one that stops short reads as none.
fn root_records(data: &[u8]) -> Vec<ClusterBodyRecordDecoded> {
    let Some(mut at) = string_table_end(data) else {
        return Vec::new();
    };
    let mut records = Vec::new();
    while let Some(record) = decode_cluster_body_record_at(data, at) {
        at = record.byte_range.end;
        records.push(record);
        if at == data.len() {
            return records;
        }
    }
    Vec::new()
}

/// Printable ASCII runs and UTF-16LE runs that spell a path or a file name.
fn link_strings(data: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut ascii = Vec::new();
    for &byte in data.iter().chain(std::iter::once(&0)) {
        if (0x20..0x7F).contains(&byte) {
            ascii.push(byte);
        } else {
            if ascii.len() >= 8 {
                out.push(String::from_utf8_lossy(&ascii).into_owned());
            }
            ascii.clear();
        }
    }
    let units: Vec<u16> = data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect();
    let mut wide = Vec::new();
    for &unit in units.iter().chain(std::iter::once(&0)) {
        if (0x20..0x7F).contains(&unit) || (0x4E00..0xA000).contains(&unit) {
            wide.push(unit);
        } else {
            // Misaligned ASCII also reads as CJK units; a path has a
            // separator or an extension.
            let text = String::from_utf16_lossy(&wide);
            if wide.len() >= 8 && (text.contains('\\') || text.contains('.')) {
                out.push(format!("(utf-16) {text}"));
            }
            wide.clear();
        }
    }
    out
}

fn main() {
    let mut dump: Option<PathBuf> = None;
    let mut sheets: Vec<PathBuf> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--dump" {
            dump = args.next().map(PathBuf::from);
        } else {
            sheets.push(PathBuf::from(arg));
        }
    }
    if sheets.is_empty() {
        eprintln!("usage: probe_does_the_sheet_link_a_border [--dump <Export.dmp>] <a.pid>...");
        return;
    }
    let drawings = dump.as_deref().map(|path| {
        let bytes = std::fs::read(path).expect("read dump");
        read_backup_drawings(&bytes).expect("T_DRAWING")
    });
    for path in &sheets {
        report(path, drawings.as_deref());
    }
}

fn report(path: &Path, drawings: Option<&[pid_parse::publish::oracle_exp::BackupDrawing]>) {
    let file_name = path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    println!("== {file_name}");
    let doc = PidParser::new().parse_file(path).expect("parse");
    let mut cfb = CompoundFile::open(std::fs::File::open(path).expect("open")).expect("cfb");

    let mut frames = 0usize;
    let mut frame_layers: Vec<u32> = Vec::new();
    for sheet in &doc.sheet_streams {
        let decoded = sheet.geometry.as_ref().map_or(0, |geometry| {
            geometry
                .decoded_igsmartframes
                .iter()
                .filter(|frame| frame.page_extent_m().is_some())
                .count()
        });
        let bytes = read_stream(&mut cfb, &sheet.path).unwrap_or_default();
        let envelopes = frame_envelopes(&bytes);
        frames += envelopes.len();
        for layer in envelopes
            .iter()
            .filter_map(|(at, _)| u32_at(&bytes, at + 6 + 8))
        {
            if !frame_layers.contains(&layer) {
                frame_layers.push(layer);
            }
        }
        println!(
            "   {} {} B: {} 0x003D frame-sized envelopes; the decoder reads {decoded} page frames",
            sheet.path,
            bytes.len(),
            envelopes.len()
        );
        for (at, follow) in envelopes.iter().take(3) {
            let payload = &bytes[at + 6..at + 6 + follow];
            let mm = |offset| f64_at(payload, offset).map_or(f64::NAN, |m| m * 1000.0);
            println!(
                "     @{at} payload {follow}: oid {:?} layer {:?} content 0x{:08X} link 0x{:08X} \
                 extent {:.3} x {:.3} mm, +156 site {:?}",
                u32_at(payload, 0),
                u32_at(payload, 8),
                u32_at(payload, 14).unwrap_or(0),
                u32_at(payload, 26).unwrap_or(0),
                mm(76),
                mm(84),
                u32_at(payload, 156)
            );
        }
    }

    let list = read_stream(&mut cfb, "/JSitesList").unwrap_or_default();
    let listed: Vec<u32> = (8..list.len())
        .step_by(4)
        .filter_map(|at| u32_at(&list, at))
        .collect();
    let ole_sites: Vec<&pid_parse::model::JSite> = doc
        .jsites
        .iter()
        .filter(|site| site.has_ole_stream)
        .collect();
    println!(
        "   JSitesList: {} B, {:?} sites listed; JSite storages {}; with a \\x01Ole stream: {}",
        list.len(),
        u32_at(&list, 4),
        doc.jsites.len(),
        ole_sites.len()
    );
    let mut links = 0usize;
    for site in &ole_sites {
        let bytes = read_stream(&mut cfb, &format!("{}/\u{1}Ole", site.path)).unwrap_or_default();
        let strings = link_strings(&bytes);
        links += strings
            .iter()
            .filter(|text| {
                let lower = text.to_ascii_lowercase();
                lower.contains(".igr") || lower.contains(".pid")
            })
            .count();
        let number = site.name.trim_start_matches("JSite").parse::<u32>().ok();
        println!(
            "     {} \\x01Ole {} B, listed {}: {strings:?}",
            site.path,
            bytes.len(),
            number.is_some_and(|n| listed.contains(&n))
        );
    }

    let tally: Vec<String> = doc
        .sheet_layers
        .get("/")
        .into_iter()
        .flatten()
        .map(|layer| format!("{} {}", layer.name, layer.object_count))
        .collect();
    println!("   root layers, objects each: {}", tally.join(", "));
    for layer in doc
        .sheet_layers
        .get("/")
        .into_iter()
        .flatten()
        .filter(|layer| layer.name == "DrawingBorder" || frame_layers.contains(&layer.oid))
    {
        let role = if frame_layers.contains(&layer.oid) {
            "the frame's layer"
        } else {
            "layer"
        };
        println!(
            "   {role} {:?}: oid {} #{} objects {}",
            layer.name, layer.oid, layer.layer_number, layer.object_count
        );
    }

    let root = read_stream(&mut cfb, "/PSMcluster0").unwrap_or_default();
    let records = root_records(&root);
    let client_sites: Vec<Option<u32>> = records
        .iter()
        .filter(|record| record.type_code == OLE_CLIENT_SITE)
        .map(|record| u32_at(&record.raw_payload, 0))
        .collect();
    let manager: Vec<String> = records
        .iter()
        .filter(|record| record.type_code == OLE_SITES_MANAGER)
        .map(|record| {
            record
                .raw_payload
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect()
        })
        .collect();
    println!(
        "   root PSMcluster0: {} records; 0x004C OLE client sites {client_sites:?}; \
         0x0004 sites manager {manager:?}",
        records.len()
    );

    let summary_template = doc.summary.as_ref().and_then(|s| s.template.clone());
    let tagged_template = doc
        .drawing_meta
        .as_ref()
        .and_then(|meta| meta.tags.get("Template").cloned());
    println!(
        "   SummaryInformation template {summary_template:?}; TaggedTxtData Template {tagged_template:?}"
    );
    if let Some(drawings) = drawings {
        let row = drawings
            .iter()
            .find(|row| row.path.rsplit(['\\', '/']).next() == Some(file_name.as_str()));
        match row {
            Some(row) => println!("   T_DRAWING {}: TEMPLATE {:?}", row.path, row.template),
            None => println!("   T_DRAWING: no row names this file"),
        }
    }
    let verdict = if frames == 0 && ole_sites.is_empty() && client_sites.is_empty() {
        "no page frame, no OLE site, no link: the sheet carries no border"
    } else if links > 0 {
        "carries a linked border"
    } else {
        "carries part of a border"
    };
    println!("   => {verdict}");
}
