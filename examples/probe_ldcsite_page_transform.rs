//! Dump root records around an `LdcSite` and its referrers while looking for
//! the page-space transform used by chain-bearing nested JSite geometry.

use std::io::Read;

use pid_parse::parsers::sheet_records::sheet_record_starts;

fn u16_le(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_le(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn main() {
    let path = std::env::args_os().nth(1).expect("PID path");
    let site: u32 = std::env::args()
        .nth(2)
        .expect("JSite id")
        .parse()
        .expect("numeric JSite id");
    let file = std::fs::File::open(&path).expect("open PID");
    let mut cfb = cfb::CompoundFile::open(file).expect("open CFB");
    let mut stream = cfb.open_stream("/PSMcluster0").expect("root PSMcluster0");
    let mut data = Vec::new();
    stream.read_to_end(&mut data).expect("read root chain");

    for at in sheet_record_starts(&data) {
        let Some(raw_type) = u16_le(&data, at) else { continue };
        let Some(length) = u32_le(&data, at + 2).map(|n| n as usize) else { continue };
        let Some(payload) = data.get(at + 6..at + 6 + length) else { continue };
        let oid = u32_le(payload, 0).unwrap_or_default();
        if oid != site && oid != site + 1 && (raw_type & 0x3fff) != 0x004f {
            continue;
        }
        let hex = payload.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ");
        let mut doubles = Vec::new();
        for offset in 0..=payload.len().saturating_sub(8) {
            let value = f64::from_le_bytes(payload[offset..offset + 8].try_into().unwrap());
            if value.is_finite() && value.abs() >= 1.0e-9 && value.abs() <= 10.0 {
                doubles.push(format!("+{offset}={value:.12}"));
            }
        }
        println!("at={at} oid={oid} type=0x{:04X} len={length}", raw_type & 0x3fff);
        println!("  hex: {hex}");
        println!("  f64: {}", doubles.join(" "));
    }

    let nested_name = format!("/JSite{site}/PSMcluster0");
    let mut nested = cfb.open_stream(&nested_name).expect("nested PSMcluster0");
    let mut bytes = Vec::new();
    nested.read_to_end(&mut bytes).expect("read nested chain");
    println!("nested={nested_name}");
    let starts = sheet_record_starts(&bytes);
    let mut parent_refs = std::collections::BTreeSet::new();
    let mut line_bounds = std::collections::BTreeMap::<u32, (f64, f64, f64, f64)>::new();
    for &at in &starts {
        let raw_type = u16_le(&bytes, at).unwrap();
        let type_code = raw_type & 0x3fff;
        if !matches!(type_code, 0x0018 | 0x0020 | 0x0059 | 0x005d | 0x0061) {
            continue;
        }
        let length = u32_le(&bytes, at + 2).unwrap() as usize;
        let payload = &bytes[at + 6..at + 6 + length];
        let oid = u32_le(payload, 0).unwrap_or_default();
        let parent = u32_le(payload, 4).unwrap_or_default();
        let layer = u32_le(payload, 8).unwrap_or_default();
        let index = u32_le(payload, 14).unwrap_or_default();
        parent_refs.insert(parent);
        let values = (18..=payload.len().saturating_sub(8))
            .step_by(8)
            .map(|offset| f64::from_le_bytes(payload[offset..offset + 8].try_into().unwrap()))
            .collect::<Vec<_>>();
        if type_code == 0x0018 && values.len() >= 4 {
            let bounds = line_bounds.entry(parent).or_insert((f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY));
            for (x, y) in [(values[0], values[1]), (values[2], values[3])] {
                bounds.0 = bounds.0.min(x); bounds.1 = bounds.1.min(y); bounds.2 = bounds.2.max(x); bounds.3 = bounds.3.max(y);
            }
        }
        println!("  oid={oid:<6} parent={parent:<6} layer={layer:<6} index={index:<6} type=0x{type_code:04X} len={length:<3} values={values:?}");
    }
    println!("parents={parent_refs:?}");
    let mut records = std::collections::BTreeMap::new();
    for &at in &starts {
        let raw_type = u16_le(&bytes, at).unwrap();
        let length = u32_le(&bytes, at + 2).unwrap() as usize;
        let payload = &bytes[at + 6..at + 6 + length];
        records.insert(u32_le(payload, 0).unwrap_or_default(), (raw_type & 0x3fff, length, payload.to_vec()));
    }
    for &at in &starts {
        let raw_type = u16_le(&bytes, at).unwrap();
        let length = u32_le(&bytes, at + 2).unwrap() as usize;
        let payload = &bytes[at + 6..at + 6 + length];
        let oid = u32_le(payload, 0).unwrap_or_default();
        if !parent_refs.contains(&oid) {
            continue;
        }
        let hex = payload.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ");
        let parent = u32_le(payload, 4).unwrap_or_default();
        println!("parent oid={oid} parent={parent} parent_record={:?} type=0x{:04X} len={length} hex={hex}", records.get(&parent).map(|r| (r.0, r.1)), raw_type & 0x3fff);
    }

    let doc = pid_parse::PidParser::new().parse_file(&path).expect("parse PID");
    let site_name = format!("JSite{site}");
    let info = doc.jsites.iter().find(|candidate| candidate.name == site_name)
        .and_then(|candidate| candidate.symbol_information.as_ref())
        .expect("symbol information");
    let parsed_site = doc.jsites.iter().find(|candidate| candidate.name == site_name).unwrap();
    println!("embedded sheets: {:?}", parsed_site.embedded_sheets.iter().map(|sheet| (sheet.oid, sheet.primitives.len())).collect::<Vec<_>>());
    let normalized = pid_parse::build_normalized_geometry(&doc);
    println!("normalized decoded={} embedded={}", normalized.entities.iter().filter(|e| e.confidence == pid_parse::PidGeometryConfidence::Decoded).count(), normalized.entities.iter().filter(|e| e.id.contains(":embedded:")).count());
    println!("symbol information:");
    for symbol in &info.symbol_informations {
        println!("  oid={} parent={} parent_record={:?} extents={:?} variables={:?}",
            symbol.oid, symbol.parent_ref, records.get(&symbol.parent_ref).map(|r| (r.0, r.1)), symbol.extents, symbol.variables);
    }
    println!("sheet parent chains:");
    for sheet in &parent_refs {
        let mut cursor = *sheet;
        print!("  {sheet}");
        for _ in 0..8 {
            let Some((kind, len, payload)) = records.get(&cursor) else { break };
            let next = u32_le(payload, 4).unwrap_or_default();
            print!(" -> {next}[0x{kind:04X}/{len}]");
            if next == 0 || next == cursor { break; }
            cursor = next;
        }
        println!();
    }
    println!("group transform candidates:");
    for sheet in &parent_refs {
        let (_, _, sheet_payload) = records.get(sheet).unwrap();
        let graphic = u32_le(sheet_payload, 4).unwrap();
        let Some((kind, len, payload)) = records.get(&graphic) else { continue };
        let words = (0..payload.len().saturating_sub(3)).step_by(4).map(|at| (at, u32_le(payload, at).unwrap())).collect::<Vec<_>>();
        let doubles = (0..payload.len().saturating_sub(7)).map(|at| (at, f64::from_le_bytes(payload[at..at+8].try_into().unwrap())))
            .filter(|(_, value)| value.is_finite() && value.abs() >= 1.0e-9 && value.abs() <= 10.0)
            .collect::<Vec<_>>();
        println!("  sheet={sheet} graphic={graphic} type=0x{kind:04X} len={len} words={words:?} doubles={doubles:?}");
    }
    let values = info.double_values.iter().map(|value| (value.oid, value.value))
        .collect::<std::collections::BTreeMap<_, _>>();
    println!("variable groups:");
    for group in &info.variable_groups {
        let resolved = group.members.iter().map(|member| (*member, values.get(member).copied()))
            .collect::<Vec<_>>();
        println!("  group {} => {resolved:?}", group.oid);
    }

    println!("space-map entries mentioning sheets, sheet parents or symbol information:");
    let interesting = parent_refs.iter().copied()
        .chain(info.symbol_informations.iter().map(|symbol| symbol.oid))
        .collect::<std::collections::BTreeSet<_>>();
    for (path, map) in &doc.psm_space_maps {
        if !path.contains(&site_name) { continue; }
        let leaf = path.rsplit(['/', '\\']).next().unwrap_or_default();
        let Some(hex) = leaf.strip_prefix("0x") else { continue };
        let Ok(address) = u32::from_str_radix(hex, 16) else { continue };
        let segment = address >> 13;
        for entry in &map.entries {
            let entry_oid = (segment << 13) | u32::from(entry.index);
            let members = entry.live_members().iter().map(|member| (member.value, member.tag)).collect::<Vec<_>>();
            if interesting.contains(&entry_oid) || members.iter().any(|(value, _)| interesting.contains(value)) {
                println!("  {path} entry={entry_oid} members={members:?}");
            }
        }
    }

    println!("top-level records matching nested group ids and igSymbol2d tails:");
    let placements = doc.sheet_streams.iter().flat_map(|sheet| sheet.geometry.iter().flat_map(|geometry| geometry.decoded_igsymbols.iter()))
        .map(|record| (record.oid, record)).collect::<std::collections::BTreeMap<_, _>>();
    let nested_group_ids = parent_refs.iter().copied().chain(parent_refs.iter().filter_map(|sheet| records.get(sheet).and_then(|r| u32_le(&r.2, 4))))
        .chain(info.symbol_informations.iter().flat_map(|symbol| [symbol.oid, symbol.parent_ref])).collect::<std::collections::BTreeSet<_>>();
    let stream_paths = cfb.walk().filter(cfb::Entry::is_stream).map(|entry| entry.path().to_path_buf())
        .filter(|path| path.file_name().is_some_and(|name| name.to_string_lossy().starts_with("Sheet"))).collect::<Vec<_>>();
    for stream_path in stream_paths {
        let mut stream = cfb.open_stream(&stream_path).unwrap();
        let mut sheet_bytes = Vec::new();
        stream.read_to_end(&mut sheet_bytes).unwrap();
        for at in sheet_record_starts(&sheet_bytes) {
            let raw_type = u16_le(&sheet_bytes, at).unwrap();
            let length = u32_le(&sheet_bytes, at + 2).unwrap() as usize;
            let payload = &sheet_bytes[at + 6..at + 6 + length];
            let oid = u32_le(payload, 0).unwrap_or_default();
            if nested_group_ids.contains(&oid) {
                println!("  MATCH path={} oid={oid} type=0x{:04X} len={length}", stream_path.display(), raw_type & 0x3fff);
            }
            if raw_type & 0x3fff != 0x00ce { continue; }
            let tail_at = payload.len().saturating_sub(32);
            let tail = payload[tail_at..].iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ");
            let words = (tail_at..payload.len().saturating_sub(3)).map(|off| (off, u32_le(payload, off).unwrap())).collect::<Vec<_>>();
            let direct = words.iter().filter(|(_, value)| interesting.contains(value)).copied().collect::<Vec<_>>();
            let decoded = placements.get(&oid).map(|record| (record.insertion_x, record.insertion_y, record.transform_00, record.transform_01, record.transform_10, record.transform_11, record.jsite_ref));
            let transformed = direct.first().and_then(|(_, sheet_ref)| line_bounds.get(sheet_ref)).and_then(|&(min_x,min_y,max_x,max_y)| placements.get(&oid).map(|r| {
                let transform = |x: f64, y: f64| (r.transform_00*x + r.transform_10*y + r.insertion_x, r.transform_01*x + r.transform_11*y + r.insertion_y);
                let points = [transform(min_x,min_y),transform(min_x,max_y),transform(max_x,min_y),transform(max_x,max_y)];
                points.into_iter().fold((f64::INFINITY,f64::INFINITY,f64::NEG_INFINITY,f64::NEG_INFINITY), |b,(x,y)| (b.0.min(x),b.1.min(y),b.2.max(x),b.3.max(y)))
            }));
            println!("  SYMBOL path={} oid={oid} len={length} tail@{tail_at}={tail} direct={direct:?} decoded={decoded:?} transformed_line_bounds={transformed:?}", stream_path.display());
        }
    }
}
