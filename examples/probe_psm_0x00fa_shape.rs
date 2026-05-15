//! Probe PSM type 0x00FA (250 hits cross-fixture) -- suspected
//! GraphicGroup / GraphicPersist records that relate geometry OIDs
//! to parent/group/reference lists.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

const PSM_HEADER_LEN: usize = 6;
const PSM_GRAPHIC_GROUP: u16 = 0x00FA;
const PSM_KNOWN_GEOMETRY_TYPES: &[u16] = &[
    0x3FE6, // SmartPlant GLine2d
    0x0030, // SmartPlant GArc2d
    0x0018, // igLine2d
    0x0084, // igLineString2d
    0x005E, // igPoint2d
    0x004D, // igTextBox
    0x00CE, // igSymbol2d
];

#[derive(Debug, Clone)]
struct PsmRecord {
    offset: usize,
    end: usize,
    type_code: u16,
    type_flags: u16,
    bytes_to_follow: u32,
    oid: Option<u32>,
}

#[derive(Debug, Default)]
struct GraphicGroupBucketStats {
    records: usize,
    records_with_geometry_oid_candidates: usize,
    geometry_oid_words: usize,
    geometry_offset_counts: BTreeMap<usize, usize>,
}

fn probe_fixture(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut cfb = CompoundFile::open(std::fs::File::open(path)?)?;
    let mut stream = cfb.open_stream("/Sheet6")?;
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes)?;
    println!(
        "\n=== {} /Sheet6 (size {}) ===",
        path.display(),
        bytes.len()
    );

    if bytes.len() < PSM_HEADER_LEN + 18 {
        return Ok(());
    }
    let psm_records = scan_psm_records(&bytes);
    let geometry_oids = known_geometry_oids(&psm_records);

    let mut hits = 0;
    let mut size_dist: BTreeMap<u32, usize> = BTreeMap::new();
    let mut subtype_dist: BTreeMap<u16, usize> = BTreeMap::new();
    let mut parent_dist: BTreeMap<u32, usize> = BTreeMap::new();
    let mut bucket_stats: BTreeMap<(u32, Option<u16>), GraphicGroupBucketStats> = BTreeMap::new();

    for record in psm_records
        .iter()
        .filter(|record| record.type_code == PSM_GRAPHIC_GROUP)
    {
        let payload = &bytes[record.offset + PSM_HEADER_LEN..record.end];
        let Some(oid) = read_u32(payload, 0) else {
            continue;
        };
        let Some(parent_ref) = read_u32(payload, 4) else {
            continue;
        };
        let sub_type_word = read_u16(payload, 16);

        hits += 1;
        *size_dist.entry(record.bytes_to_follow).or_insert(0) += 1;
        *parent_dist.entry(parent_ref).or_insert(0) += 1;
        if let Some(sub_type_word) = sub_type_word {
            *subtype_dist.entry(sub_type_word).or_insert(0) += 1;
        }

        let candidates = candidate_oid_words(payload, &geometry_oids);
        let stats = bucket_stats
            .entry((record.bytes_to_follow, sub_type_word))
            .or_default();
        stats.records += 1;
        let mut has_geometry_oid_candidate = false;
        for (pos, _, known_geometry) in &candidates {
            if !*known_geometry {
                continue;
            }
            has_geometry_oid_candidate = true;
            stats.geometry_oid_words += 1;
            *stats.geometry_offset_counts.entry(*pos).or_insert(0) += 1;
        }
        if has_geometry_oid_candidate {
            stats.records_with_geometry_oid_candidates += 1;
        }

        if hits <= 3 {
            println!(
                "  HIT[{}] @ 0x{:06x} type_flags={} bytes_to_follow={} oid={} parent_ref={} sub_type={}",
                hits,
                record.offset,
                record.type_flags,
                record.bytes_to_follow,
                oid,
                parent_ref,
                sub_type_word
                    .map(|value| format!("0x{value:04X}"))
                    .unwrap_or_else(|| "<missing>".to_string())
            );

            if let Some(prev) = adjacent_record_before(&psm_records, record.offset) {
                println!("           prev {}", format_record(prev));
            }
            if let Some(next) = adjacent_record_after(&psm_records, record.end) {
                println!("           next {}", format_record(next));
            }

            if candidates.is_empty() {
                println!("           candidate_oid_words: <none>");
            } else {
                println!("           candidate_oid_words:");
                for (pos, value, known_geometry) in candidates.iter().take(18) {
                    let marker = if *known_geometry { " geometry_oid" } else { "" };
                    println!("             +{pos:03}: {value}{marker}");
                }
            }

            let dump_len = payload.len().min(160);
            for chunk_start in (0..dump_len).step_by(16) {
                let chunk_end = (chunk_start + 16).min(dump_len);
                let raw: &[u8] = &payload[chunk_start..chunk_end];
                let hex: String = raw
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                let ascii: String = raw
                    .iter()
                    .map(|b| {
                        if (0x20..0x7F).contains(b) {
                            *b as char
                        } else {
                            '.'
                        }
                    })
                    .collect();
                println!("           +{:03}: {:<48} | {}", chunk_start, hex, ascii);
            }
        }
    }
    println!("  type 0x00FA: {} hits", hits);
    print_u32_distribution("bytes_to_follow", &size_dist, 10);
    print_u16_distribution("sub_type_word", &subtype_dist, 10);
    print_u32_distribution("parent_ref", &parent_dist, 10);
    print_graphic_group_bucket_summary(&bucket_stats, 12);
    Ok(())
}

fn scan_psm_records(data: &[u8]) -> Vec<PsmRecord> {
    if data.len() < PSM_HEADER_LEN {
        return Vec::new();
    }
    let mut records = Vec::new();
    for off in 0..=data.len() - PSM_HEADER_LEN {
        let type_word = u16::from_le_bytes([data[off], data[off + 1]]);
        let type_code = type_word & 0x3FFF;
        let type_flags = type_word >> 14;
        let bytes_to_follow =
            u32::from_le_bytes([data[off + 2], data[off + 3], data[off + 4], data[off + 5]]);
        if !(8..=100_000).contains(&bytes_to_follow) {
            continue;
        }
        let Some(payload_end) = off
            .checked_add(PSM_HEADER_LEN)
            .and_then(|start| start.checked_add(bytes_to_follow as usize))
        else {
            continue;
        };
        if payload_end > data.len() {
            continue;
        }
        let oid = read_u32(data, off + PSM_HEADER_LEN);
        records.push(PsmRecord {
            offset: off,
            end: payload_end,
            type_code,
            type_flags,
            bytes_to_follow,
            oid,
        });
    }
    records
}

fn known_geometry_oids(records: &[PsmRecord]) -> BTreeSet<u32> {
    records
        .iter()
        .filter(|record| PSM_KNOWN_GEOMETRY_TYPES.contains(&record.type_code))
        .filter_map(|record| record.oid)
        .filter(|oid| *oid > 16)
        .collect()
}

fn adjacent_record_before(records: &[PsmRecord], offset: usize) -> Option<&PsmRecord> {
    records
        .iter()
        .filter(|record| is_context_record(record.type_code))
        .filter(|record| record.end <= offset)
        .max_by_key(|record| record.end)
}

fn adjacent_record_after(records: &[PsmRecord], end: usize) -> Option<&PsmRecord> {
    records
        .iter()
        .filter(|record| is_context_record(record.type_code))
        .filter(|record| record.offset >= end)
        .min_by_key(|record| record.offset)
}

fn is_context_record(type_code: u16) -> bool {
    type_code == PSM_GRAPHIC_GROUP || PSM_KNOWN_GEOMETRY_TYPES.contains(&type_code)
}

fn candidate_oid_words(
    payload: &[u8],
    known_geometry_oids: &BTreeSet<u32>,
) -> Vec<(usize, u32, bool)> {
    let mut out = Vec::new();
    if payload.len() < 22 {
        return out;
    }
    for pos in (18..=payload.len() - 4).step_by(4) {
        let Some(value) = read_u32(payload, pos) else {
            continue;
        };
        if value == 0 || value > 10_000_000 {
            continue;
        }
        out.push((pos, value, known_geometry_oids.contains(&value)));
    }
    out
}

fn format_record(record: &PsmRecord) -> String {
    format!(
        "type=0x{:04X} flags={} range=0x{:06x}..0x{:06x} btf={} oid={}",
        record.type_code,
        record.type_flags,
        record.offset,
        record.end,
        record.bytes_to_follow,
        record
            .oid
            .map(|oid| oid.to_string())
            .unwrap_or_else(|| "<missing>".to_string())
    )
}

fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    let bytes = data.get(offset..offset.checked_add(2)?)?;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let bytes = data.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn print_u16_distribution(label: &str, dist: &BTreeMap<u16, usize>, limit: usize) {
    println!("  {label}:");
    let mut sorted: Vec<_> = dist.iter().collect();
    sorted.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    for (value, count) in sorted.iter().take(limit) {
        println!("    0x{value:04X}: {count} hits");
    }
}

fn print_u32_distribution(label: &str, dist: &BTreeMap<u32, usize>, limit: usize) {
    println!("  {label}:");
    let mut sorted: Vec<_> = dist.iter().collect();
    sorted.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    for (value, count) in sorted.iter().take(limit) {
        println!("    {value}: {count} hits");
    }
}

fn print_graphic_group_bucket_summary(
    stats: &BTreeMap<(u32, Option<u16>), GraphicGroupBucketStats>,
    limit: usize,
) {
    println!("  bucket geometry-OID candidate summary:");
    let mut sorted: Vec<_> = stats.iter().collect();
    sorted.sort_by_key(|((bytes_to_follow, sub_type), bucket)| {
        (
            std::cmp::Reverse(bucket.records),
            *bytes_to_follow,
            sub_type.unwrap_or(u16::MAX),
        )
    });
    for ((bytes_to_follow, sub_type), bucket) in sorted.iter().take(limit) {
        let sub_type = sub_type
            .map(|value| format!("0x{value:04X}"))
            .unwrap_or_else(|| "<missing>".to_string());
        let mut offsets: Vec<_> = bucket.geometry_offset_counts.iter().collect();
        offsets.sort_by_key(|(offset, count)| (std::cmp::Reverse(**count), **offset));
        let top_offsets = offsets
            .into_iter()
            .take(8)
            .map(|(offset, count)| format!("+{offset:03}:{count}"))
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "    btf={} sub_type={} records={} records_with_geom_oid={} geom_oid_words={} top_offsets=[{}]",
            bytes_to_follow,
            sub_type,
            bucket.records,
            bucket.records_with_geometry_oid_candidates,
            bucket.geometry_oid_words,
            top_offsets
        );
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for fixture in [
        "test-file/DWG-0201GP06-01.pid",
        "test-file/DWG-0202GP06-01.pid",
        "test-file/工艺管道及仪表流程-1.pid",
        "test-file/export-test/publish-data/A01/A01.pid",
    ] {
        let path = Path::new(fixture);
        if !path.exists() {
            continue;
        }
        probe_fixture(path)?;
    }
    Ok(())
}
