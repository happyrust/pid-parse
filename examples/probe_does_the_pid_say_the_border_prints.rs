//! Does anything a `.pid` stores tell a sheet whose plot leaves its linked
//! border out from one whose plot draws it?
//!
//! `集注站 凝析油闪蒸单元 一级闪蒸分离器A` and `集注站 烃水露点控制单元（一）
//! 低温分离器A` link the same A1 template as `…分离器B` and the three `空冷器`
//! sheets, yet their reference plots carry no frame and no template text while
//! the other four carry both. A field the vendor honours would have to differ
//! between the two groups, so this probe reads every place the frame's display
//! could hang on and says, field by field, whether it separates them:
//!
//! * the page `igSmartFrame2d` record in full, and the `JSheetLayer` record of
//!   the layer its payload `+8` names, with that layer's display and locate
//!   bits in the sheet's `0x0057` view filter set and the set's four further
//!   bitmaps;
//! * the second filter set class `0x0060`;
//! * every stream of each `JSite` storage that holds a `\x01Ole` stream, and
//!   the storage's stream list (no `\x01CompObj`, `\x03ObjInfo` or cached
//!   presentation in this corpus);
//! * the OLE client-site (`0x004C`) and sites-manager (`0x0004`) records;
//! * the dynamic-attribute records whose owner (payload `+12`, the trailer's
//!   `field_x`) is the frame, with each record's own id (payload `+0`) masked;
//! * the document singletons of the root `PSMcluster0` (`0x0002`, `0x0036`,
//!   `0x0037`, `0x0042`, `0x0076`, `0x0086`, `0x0088`) and the small
//!   top-level streams.
//!
//! A field *separates* when every bare sheet holds one value no framed sheet
//! holds. The type names come from `tools/psm_type_clsid.py`.
//!
//! ```text
//! cargo run --example probe_does_the_pid_say_the_border_prints -- \
//!     --bare <a.pid>... --framed <b.pid>...
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use cfb::CompoundFile;
use pid_parse::parsers::cluster_header::{
    decode_cluster_body_record_at, decode_unclustered_da_body_records, ClusterBodyRecordDecoded,
};
use pid_parse::parsers::view_filter_sets::PSM_TYPE_CODE_VIEW_FILTER_SET;
use pid_parse::PidParser;

/// Document-level `PSMcluster0` classes read whole.
const SINGLETONS: [(u16, &str); 9] = [
    (0x0002, "INGR Persist Preference DEX"),
    (0x0004, "OLE subsystem sites manager"),
    (0x0036, "JViewSet"),
    (0x0037, "BackDrop"),
    (0x0042, "JSheetLayerManager"),
    (0x004C, "OLE subsystem client site"),
    (0x0060, "Top ViewFilterSet (0x0060)"),
    (0x0076, "SheetView"),
    (0x0086, "JSheets"),
];
const SHEET_LAYER_GROUP: u16 = 0x0088;
const SHEET_LAYER: u16 = 0x0081;
const TOP_LEVEL_STREAMS: [&str; 11] = [
    "/\u{5}SummaryInformation",
    "/\u{5}DocumentSummaryInformation",
    "/AppObject",
    "/DocVersion2",
    "/DocVersion3",
    "/PSMroots",
    "/PSMclustertable",
    "/PSMsegmenttable",
    "/JTaggedTxtStgList",
    "/Dynamic Attributes Metadata",
    "/JSitesList",
];

fn read_stream(cfb: &mut CompoundFile<std::fs::File>, path: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    cfb.open_stream(path).ok()?.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

/// Where the root `PSMcluster0` string table ends: its `{u32 index, u32 byte
/// length, UTF-16}` entries (`SiteObjects`, `PreferenceSet`, `Sheets`, an
/// empty fourth) stop at the first word too large to be an index.
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

/// The root `PSMcluster0` records: the chain after the string table, which
/// must run exactly to the end of the stream. Starting anywhere earlier is
/// wrong in a way that can look right — `Sheets`' last unit `73 00` and the
/// next entry read as a `0x0073` record, and on `空冷器C` a start at offset 32
/// swallows the first real records and still chains to the end.
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

/// The four bitmaps after display and locate, with their length prefixes.
fn further_bitmaps(payload: &[u8]) -> Option<Vec<u8>> {
    let mut at = 38usize;
    let mut out = Vec::new();
    for index in 0..6 {
        if *payload.get(at)? != 0xFF {
            return None;
        }
        let len = usize::from(u16::from_le_bytes(
            payload.get(at + 1..at + 3)?.try_into().ok()?,
        ));
        let end = at + 3 + len;
        if index >= 2 {
            out.extend_from_slice(payload.get(at..end)?);
        }
        at = end;
    }
    Some(out)
}

fn fields_of(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let doc = PidParser::new().parse_file(path).expect("parse");
    let mut cfb = CompoundFile::open(std::fs::File::open(path).expect("open")).expect("cfb");

    let mut frame_oids = Vec::new();
    let mut layer_refs = Vec::new();
    let mut frame_bytes = Vec::new();
    for sheet in &doc.sheet_streams {
        let Some(geometry) = &sheet.geometry else {
            continue;
        };
        let frames: Vec<_> = geometry
            .decoded_igsmartframes
            .iter()
            .filter(|frame| frame.page_extent_m().is_some())
            .collect();
        if frames.is_empty() {
            continue;
        }
        let bytes = read_stream(&mut cfb, &sheet.path).expect("sheet stream");
        for frame in frames {
            frame_bytes.extend_from_slice(&bytes[frame.byte_start..frame.byte_end]);
            frame_oids.push(frame.oid);
            layer_refs.push(frame.sheet_layer_ref);
        }
    }
    out.insert("igSmartFrame2d page frame record".into(), frame_bytes);

    let root = read_stream(&mut cfb, "/PSMcluster0").unwrap_or_default();
    let records = root_records(&root);
    if records.is_empty() {
        eprintln!(
            "{}: the root PSMcluster0 chain did not close",
            path.display()
        );
    }
    let mut layer_record = Vec::new();
    for record in records.iter().filter(|r| r.type_code == SHEET_LAYER) {
        if u32_at(&record.raw_payload, 0).is_some_and(|oid| layer_refs.contains(&oid)) {
            layer_record.extend_from_slice(&record.raw_payload);
        }
    }
    out.insert(
        "JSheetLayer record of the frame's layer".into(),
        layer_record,
    );

    let mut bits = Vec::new();
    for layer in doc.sheet_layers.get("/").into_iter().flatten() {
        if layer_refs.contains(&layer.oid) {
            bits.extend_from_slice(
                format!(
                    "{} #{} displayed={:?} locatable={:?}",
                    layer.name, layer.layer_number, layer.displayed, layer.locatable
                )
                .as_bytes(),
            );
        }
    }
    out.insert("frame layer display / locate bits (0x0057)".into(), bits);

    let mut further = Vec::new();
    for record in records
        .iter()
        .filter(|r| r.type_code == PSM_TYPE_CODE_VIEW_FILTER_SET)
    {
        further.extend(further_bitmaps(&record.raw_payload).unwrap_or_default());
    }
    out.insert("0x0057 bitmaps 3..6".into(), further);

    for (type_code, name) in SINGLETONS {
        let mut value = Vec::new();
        for record in records.iter().filter(|r| r.type_code == type_code) {
            value.extend_from_slice(&record.raw_payload);
        }
        out.insert(format!("0x{type_code:04X} {name}"), value);
    }
    let group: Vec<u8> = records
        .iter()
        .filter(|r| r.type_code == SHEET_LAYER_GROUP)
        .flat_map(|r| r.raw_payload.clone())
        .collect();
    out.insert(format!("0x{SHEET_LAYER_GROUP:04X} JSheetLayerGroup"), group);

    let ole_sites: Vec<String> = cfb
        .walk()
        .filter(|entry| entry.is_stream() && entry.name() == "\u{1}Ole")
        .filter_map(|entry| {
            entry
                .path()
                .parent()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
        })
        .collect();
    let mut site_listing = Vec::new();
    for site in &ole_sites {
        let mut names: Vec<String> = cfb
            .read_storage(site)
            .map(|entries| entries.map(|e| e.name().to_string()).collect())
            .unwrap_or_default();
        names.sort();
        let shown: Vec<String> = names
            .iter()
            .map(|n| n.escape_default().to_string())
            .collect();
        site_listing.extend_from_slice(format!("{site}: {}", shown.join(",")).as_bytes());
        for (name, shown) in names.iter().zip(&shown) {
            if let Some(bytes) = read_stream(&mut cfb, &format!("{site}/{name}")) {
                out.insert(format!("OLE site stream {shown}"), bytes);
            }
        }
    }
    out.insert("OLE site storage listing".into(), site_listing);

    let da = read_stream(&mut cfb, "/Unclustered Dynamic Attributes").unwrap_or_default();
    let mut owned = Vec::new();
    let da_records = decode_unclustered_da_body_records(&da);
    if da_records.is_empty() {
        eprintln!(
            "{}: the dynamic-attribute chain did not close",
            path.display()
        );
    }
    for record in da_records {
        if u32_at(&record.raw_payload, 12).is_some_and(|owner| frame_oids.contains(&owner)) {
            owned.extend_from_slice(&record.raw_payload[4..]);
        }
    }
    out.insert(
        "dynamic attributes owned by the frame (own id masked)".into(),
        owned,
    );

    for stream in TOP_LEVEL_STREAMS {
        out.insert(
            format!("stream {}", stream.escape_default()),
            read_stream(&mut cfb, stream).unwrap_or_default(),
        );
    }
    out
}

fn short(value: &[u8]) -> String {
    if !value.is_empty() && value.len() <= 96 && value.iter().all(|b| (0x20..0x7F).contains(b)) {
        return String::from_utf8_lossy(value).into_owned();
    }
    if value.len() <= 12 {
        return value.iter().map(|b| format!("{b:02x}")).collect();
    }
    let mut hash: u32 = 0x811c_9dc5;
    for byte in value {
        hash = (hash ^ u32::from(*byte)).wrapping_mul(0x0100_0193);
    }
    format!("{}B:{hash:08x}", value.len())
}

fn main() {
    let mut bare: Vec<PathBuf> = Vec::new();
    let mut framed: Vec<PathBuf> = Vec::new();
    let mut into_bare = true;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--bare" => into_bare = true,
            "--framed" => into_bare = false,
            _ if into_bare => bare.push(PathBuf::from(arg)),
            _ => framed.push(PathBuf::from(arg)),
        }
    }
    if bare.is_empty() || framed.is_empty() {
        eprintln!(
            "usage: probe_does_the_pid_say_the_border_prints --bare <a.pid>... --framed <b.pid>..."
        );
        return;
    }
    let sheets: Vec<(String, bool, BTreeMap<String, Vec<u8>>)> = bare
        .iter()
        .map(|p| (p, true))
        .chain(framed.iter().map(|p| (p, false)))
        .enumerate()
        .map(|(index, (path, is_bare))| {
            let label = format!("{}{}", if is_bare { 'b' } else { 'f' }, index + 1);
            println!(
                "{label} = {}",
                path.file_stem().unwrap_or_default().to_string_lossy()
            );
            (label, is_bare, fields_of(path))
        })
        .collect();

    let names: Vec<String> = sheets
        .iter()
        .flat_map(|(_, _, fields)| fields.keys().cloned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut separating = 0usize;
    for name in &names {
        let values: Vec<&[u8]> = sheets
            .iter()
            .map(|(_, _, fields)| fields.get(name).map_or(&[][..], Vec::as_slice))
            .collect();
        let bare_values: Vec<&[u8]> = sheets
            .iter()
            .zip(&values)
            .filter(|((_, is_bare, _), _)| *is_bare)
            .map(|(_, value)| *value)
            .collect();
        let framed_values: Vec<&[u8]> = sheets
            .iter()
            .zip(&values)
            .filter(|((_, is_bare, _), _)| !*is_bare)
            .map(|(_, value)| *value)
            .collect();
        let verdict = if values.iter().all(|v| *v == values[0]) {
            "same in every sheet"
        } else if bare_values.iter().all(|v| *v == bare_values[0])
            && !framed_values.contains(&bare_values[0])
        {
            separating += 1;
            "SEPARATES the groups"
        } else {
            "varies, does not separate"
        };
        println!("\n{name}: {verdict}");
        if verdict == "same in every sheet" {
            println!("  all {}", short(values[0]));
            continue;
        }
        for ((label, _, _), value) in sheets.iter().zip(&values) {
            println!("  {label} {}", short(value));
        }
    }
    println!(
        "\n{} fields over {} bare and {} framed sheets: {separating} separate the groups",
        names.len(),
        bare.len(),
        framed.len()
    );
}
