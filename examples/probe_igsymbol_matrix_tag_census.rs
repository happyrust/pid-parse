//! Cross-tabulates the `igSymbol2d` matrix-tag's 3rd byte (`0xA7` canonical
//! vs `0x27`, seen only on `D04.pid`) against other payload fields, across
//! every fixture corpus this repo can reach.
//!
//! Walks the raw `/Sheet*` PSM record chain directly (not through the
//! `decode_igsymbols` decoder, so the `0x27` variant is visible even before
//! the decoder learns to accept it) and reports, per tag-byte value: hit
//! count, fixtures, `type_flags` seen, `bytes_to_follow` histogram, 7th
//! double (scale) distribution, `jsite_ref` range, on-sheet insertion rate.
//! See
//! `docs/analysis/2026-11-07-igsymbol2d-matrix-tag-third-byte-is-a-session-flag.md`
//! for the reading of the numbers this prints.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

const TYPE_IGSYMBOL2D: u16 = 0x00CE;
const PSM_HEADER_LEN: usize = 6;

fn read_f64(p: &[u8], at: usize) -> Option<f64> {
    let c = p.get(at..at + 8)?;
    Some(f64::from_le_bytes([
        c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7],
    ]))
}

fn read_u32(p: &[u8], at: usize) -> Option<u32> {
    let c = p.get(at..at + 4)?;
    Some(u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
}

/// Walk the chain-validated PSM record sequence of one stream, returning
/// every record whose type code is `0x00CE` regardless of tag-byte variant
/// (unlike the production decoder, which only accepts `0xA7`).
fn collect_igsymbol_candidates(bytes: &[u8]) -> Vec<(usize, u16, Vec<u8>)> {
    let mut out = Vec::new();
    let mut off = 0usize;
    while off + PSM_HEADER_LEN <= bytes.len() {
        let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
        let type_code = type_word & 0x3FFF;
        let btf = u32::from_le_bytes([
            bytes[off + 2],
            bytes[off + 3],
            bytes[off + 4],
            bytes[off + 5],
        ]) as usize;
        // chain-validated: btf must be sane and the record must end at
        // stream end or at another plausible header.
        if !(8..=100_000).contains(&btf) {
            off += 1;
            continue;
        }
        let start = off + PSM_HEADER_LEN;
        let end = start + btf;
        if end > bytes.len() {
            off += 1;
            continue;
        }
        let next_ok = end == bytes.len() || {
            if end + 6 <= bytes.len() {
                let next_word = u16::from_le_bytes([bytes[end], bytes[end + 1]]);
                let next_btf = u32::from_le_bytes([
                    bytes[end + 2],
                    bytes[end + 3],
                    bytes[end + 4],
                    bytes[end + 5],
                ]) as usize;
                (next_word & 0x3FFF != 0)
                    && (8..=100_000).contains(&next_btf)
                    && end + 6 + next_btf <= bytes.len() + 6
            } else {
                true
            }
        };
        if !next_ok {
            off += 1;
            continue;
        }
        if type_code == TYPE_IGSYMBOL2D {
            out.push((off, type_word, bytes[start..end].to_vec()));
        }
        off = end;
    }
    out
}

#[derive(Default)]
struct Bucket {
    count: usize,
    fixtures: BTreeMap<String, usize>,
    type_flags_seen: BTreeMap<u16, usize>,
    btf_seen: BTreeMap<u32, usize>,
    scale_min: f64,
    scale_max: f64,
    scales: Vec<f64>,
    on_sheet: usize,
    off_sheet: usize,
    jsite_min: u32,
    jsite_max: u32,
}

impl Bucket {
    fn new() -> Self {
        Bucket {
            scale_min: f64::INFINITY,
            scale_max: f64::NEG_INFINITY,
            jsite_min: u32::MAX,
            jsite_max: 0,
            ..Default::default()
        }
    }
}

fn gather_pid_files() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let repo = PathBuf::from(r"D:\work\plant-code\cad\pid-parse\test-file");
    roots.push(repo);
    if let Some(dir) = std::env::var_os("PID_TEST_DATA_DIR") {
        let dir = PathBuf::from(dir);
        roots.push(dir.join("backup-plant-data"));
        roots.push(dir.join("验收数据"));
    }
    for extra in [
        r"D:\work\cad\PID自研软件验证",
        r"D:\work\cad\pid-test-data\验收数据",
        r"D:\work\plant-code\cad\pid-test-data\backup-plant-data",
        r"D:\work\plant-code\cad\pid-test-data\验收数据",
    ] {
        roots.push(PathBuf::from(extra));
    }
    let mut files = Vec::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        collect_pid_recursive(&root, &mut files);
    }
    files.sort();
    files.dedup();
    files
}

fn collect_pid_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_pid_recursive(&path, out);
        } else if path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("pid"))
            .unwrap_or(false)
        {
            out.push(path);
        }
    }
}

fn main() {
    let files = gather_pid_files();
    println!("scanning {} .pid files", files.len());

    let mut buckets: BTreeMap<u8, Bucket> = BTreeMap::new();

    for path in &files {
        let fixture_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let mut cfb = match cfb::CompoundFile::open(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let sheet_paths: Vec<String> = cfb
            .walk()
            .filter(|e| e.is_stream())
            .map(|e| e.path().to_string_lossy().replace('\\', "/"))
            .filter(|p| p.contains("Sheet") && !p.contains("PSMcluster"))
            .collect();

        for sheet_path in sheet_paths {
            let cfb_path = sheet_path.trim_start_matches('/');
            let mut stream = match cfb.open_stream(cfb_path) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let mut bytes = Vec::new();
            if stream.read_to_end(&mut bytes).is_err() {
                continue;
            }
            for (off, type_word, payload) in collect_igsymbol_candidates(&bytes) {
                let type_flags = type_word >> 14;
                let btf = payload.len() as u32;
                // search both tag variants in first 64 bytes
                let search_end = payload.len().min(64);
                let tag_a7 = [0x02u8, 0x00, 0xA7, 0x50];
                let tag_27 = [0x02u8, 0x00, 0x27, 0x50];
                let mut tag_byte = None;
                let mut matrix_at = None;
                for (byte_val, tag) in [(0xA7u8, tag_a7), (0x27u8, tag_27)] {
                    if let Some(pos) = payload[..search_end].windows(4).position(|w| w == tag) {
                        tag_byte = Some(byte_val);
                        matrix_at = Some(pos + 4);
                        break;
                    }
                }
                let Some(tag_byte) = tag_byte else {
                    continue; // no recognizable tag at all; not in scope here
                };
                // Not `or_default()`: `Bucket::new` seeds `scale_min`/
                // `scale_max`/`jsite_min`/`jsite_max` to the identity values
                // a running min/max needs (`INFINITY`/`NEG_INFINITY`/`MAX`/0),
                // which `#[derive(Default)]` would zero instead.
                #[allow(clippy::unwrap_or_default)]
                let bucket = buckets.entry(tag_byte).or_insert_with(Bucket::new);
                bucket.count += 1;
                *bucket.fixtures.entry(fixture_name.clone()).or_insert(0) += 1;
                *bucket.type_flags_seen.entry(type_flags).or_insert(0) += 1;
                *bucket.btf_seen.entry(btf).or_insert(0) += 1;

                if let Some(matrix_at) = matrix_at {
                    let ins_x = read_f64(&payload, matrix_at + 32);
                    let ins_y = read_f64(&payload, matrix_at + 40);
                    let scale = read_f64(&payload, matrix_at + 48);
                    if let Some(s) = scale {
                        if s.is_finite() {
                            bucket.scales.push(s);
                            bucket.scale_min = bucket.scale_min.min(s);
                            bucket.scale_max = bucket.scale_max.max(s);
                        }
                    }
                    if let (Some(x), Some(y)) = (ins_x, ins_y) {
                        if (0.0..=1.3).contains(&x) && (0.0..=1.3).contains(&y) {
                            bucket.on_sheet += 1;
                        } else {
                            bucket.off_sheet += 1;
                        }
                    }
                    let tag_at = matrix_at - 4;
                    if tag_at >= 4 {
                        if let Some(jsite_ref) = read_u32(&payload, tag_at - 4) {
                            bucket.jsite_min = bucket.jsite_min.min(jsite_ref);
                            bucket.jsite_max = bucket.jsite_max.max(jsite_ref);
                        }
                    }
                }
                let _ = off;
            }
        }
    }

    for (tag_byte, bucket) in &buckets {
        println!("\n=== tag byte 0x{tag_byte:02X} ===");
        println!("count = {}", bucket.count);
        println!("fixtures: {:?}", bucket.fixtures);
        println!("type_flags_seen: {:?}", bucket.type_flags_seen);
        println!("bytes_to_follow histogram: {:?}", bucket.btf_seen);
        println!(
            "scale range: [{:.4}, {:.4}] (n={})",
            bucket.scale_min,
            bucket.scale_max,
            bucket.scales.len()
        );
        let mut distinct_scales: Vec<f64> = bucket.scales.clone();
        distinct_scales.sort_by(|a, b| a.partial_cmp(b).unwrap());
        distinct_scales.dedup();
        println!(
            "distinct scales (up to 20): {:?}",
            &distinct_scales[..distinct_scales.len().min(20)]
        );
        println!(
            "insertion on-sheet={} off-sheet={}",
            bucket.on_sheet, bucket.off_sheet
        );
        println!(
            "jsite_ref range: [{}, {}]",
            bucket.jsite_min, bucket.jsite_max
        );
    }
}
