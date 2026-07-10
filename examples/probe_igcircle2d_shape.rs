//! Phase 35-A evidence probe for PSM `0x0059 igCircle2d` records.
//!
//! This is intentionally positional: it reports byte ranges, source domains,
//! layout buckets, neighbors, and f64 candidates without assigning geometry
//! semantics. Usage:
//! `cargo run --quiet --example probe_igcircle2d_shape -- [--dump-records] <file-or-dir>`

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{Read, Seek, SeekFrom};
use std::ops::Range;
use std::path::{Path, PathBuf};

const PSM_HEADER_LEN: usize = 6;
const MIN_BYTES_TO_FOLLOW: usize = 8;
const MAX_BYTES_TO_FOLLOW: usize = 100_000;
const IGCIRCLE2D_TYPE_CODE: u16 = 0x0059;
const CFB_SIGNATURE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RecordMeta {
    start: usize,
    end: usize,
    raw_type: u16,
    type_code: u16,
    btf: usize,
}

impl RecordMeta {
    fn payload_range(self) -> Range<usize> {
        self.start + PSM_HEADER_LEN..self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SourceDomain {
    SymbolSheet,
    NestedPidJsite,
    Other,
}

impl fmt::Display for SourceDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::SymbolSheet => "symbol_sheet",
            Self::NestedPidJsite => "nested_pid_jsite",
            Self::Other => "other",
        };
        f.write_str(name)
    }
}

#[derive(Debug, Default)]
struct F64Stats {
    seen: usize,
    finite: usize,
    min: Option<f64>,
    max: Option<f64>,
    distinct_bits: BTreeSet<u64>,
}

impl F64Stats {
    fn observe(&mut self, value: f64) {
        self.seen += 1;
        self.distinct_bits.insert(value.to_bits());
        if value.is_finite() {
            self.finite += 1;
            self.min = Some(self.min.map_or(value, |current| current.min(value)));
            self.max = Some(self.max.map_or(value, |current| current.max(value)));
        }
    }
}

#[derive(Debug, Default)]
struct BucketStats {
    records: usize,
    raw_types: BTreeMap<u16, usize>,
    neighbors: BTreeMap<(Option<u16>, Option<u16>), usize>,
    terminal_bytes: BTreeMap<u8, usize>,
    f64_candidates: BTreeMap<usize, F64Stats>,
}

impl BucketStats {
    fn observe(
        &mut self,
        raw_type: u16,
        previous_type: Option<u16>,
        next_type: Option<u16>,
        payload: &[u8],
    ) {
        self.records += 1;
        *self.raw_types.entry(raw_type).or_insert(0) += 1;
        *self
            .neighbors
            .entry((previous_type, next_type))
            .or_insert(0) += 1;
        if let Some(&terminal_byte) = payload.last() {
            *self.terminal_bytes.entry(terminal_byte).or_insert(0) += 1;
        }

        if payload.len() < 8 {
            return;
        }
        for offset in 0..=payload.len() - 8 {
            self.f64_candidates
                .entry(offset)
                .or_default()
                .observe(f64_at(&payload[offset..offset + 8]));
        }
    }
}

#[derive(Debug)]
struct RecordDump {
    file: PathBuf,
    stream_path: String,
    source_domain: SourceDomain,
    record: RecordMeta,
    previous: Option<RecordMeta>,
    next: Option<RecordMeta>,
    payload: Vec<u8>,
}

#[derive(Debug, Default)]
struct ScanSummary {
    files_scanned: usize,
    cfb_containers: usize,
    non_cfb_files: usize,
    streams_scanned: usize,
    circle_records: usize,
    source_totals: BTreeMap<SourceDomain, usize>,
    buckets: BTreeMap<(SourceDomain, usize), BucketStats>,
    dumps: Vec<RecordDump>,
}

fn record_at(bytes: &[u8], start: usize) -> Option<RecordMeta> {
    let header_end = start.checked_add(PSM_HEADER_LEN)?;
    let header = bytes.get(start..header_end)?;
    let raw_type = u16::from_le_bytes([header[0], header[1]]);
    let type_code = raw_type & 0x3FFF;
    if type_code == 0 {
        return None;
    }
    let btf = u32::from_le_bytes([header[2], header[3], header[4], header[5]]) as usize;
    if !(MIN_BYTES_TO_FOLLOW..=MAX_BYTES_TO_FOLLOW).contains(&btf) {
        return None;
    }
    let end = header_end.checked_add(btf)?;
    if end > bytes.len() {
        return None;
    }
    Some(RecordMeta {
        start,
        end,
        raw_type,
        type_code,
        btf,
    })
}

fn chain_to_stream_end(bytes: &[u8], start: usize) -> Option<Vec<RecordMeta>> {
    let mut records = Vec::new();
    let mut offset = start;
    loop {
        let record = record_at(bytes, offset)?;
        records.push(record);
        if record.end == bytes.len() {
            return Some(records);
        }
        offset = record.end;
    }
}

fn walk_records(bytes: &[u8]) -> Vec<RecordMeta> {
    // ponytail: worst-case malformed streams are O(n^2); memoize suffix validity only if corpus runtime becomes material.
    let mut start = 0usize;
    while start + PSM_HEADER_LEN <= bytes.len() {
        if let Some(records) = chain_to_stream_end(bytes, start) {
            return records;
        }
        start += 1;
    }
    Vec::new()
}

fn f64_at(bytes: &[u8]) -> f64 {
    let Ok(raw) = <[u8; 8]>::try_from(bytes) else {
        return f64::NAN;
    };
    f64::from_le_bytes(raw)
}

fn has_cfb_signature(bytes: &[u8]) -> bool {
    bytes == CFB_SIGNATURE
}

fn classify_source(file: &Path, stream_path: &str) -> SourceDomain {
    let normalized = stream_path.replace('\\', "/");
    let extension = file.extension().and_then(|value| value.to_str());
    let is_symbol = extension.is_some_and(|value| value.eq_ignore_ascii_case("sym"));
    let is_pid = extension.is_some_and(|value| value.eq_ignore_ascii_case("pid"));
    let has_sheet_component = normalized
        .split('/')
        .any(|component| component.starts_with("Sheet"));
    if is_symbol && has_sheet_component {
        SourceDomain::SymbolSheet
    } else if is_pid
        && normalized
            .split('/')
            .any(|component| component.starts_with("JSite"))
        && normalized.ends_with("/PSMcluster0")
    {
        SourceDomain::NestedPidJsite
    } else {
        SourceDomain::Other
    }
}

fn collect_files(path: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if path.is_file() {
        files.push(path.to_path_buf());
        return Ok(());
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        if child.is_dir() {
            collect_files(&child, files)?;
        } else {
            files.push(child);
        }
    }
    Ok(())
}

fn scan(root: &Path, dump_records: bool) -> Result<ScanSummary, Box<dyn std::error::Error>> {
    let mut summary = ScanSummary::default();
    let mut files = Vec::new();
    collect_files(root, &mut files)?;
    files.sort();

    for file in files {
        summary.files_scanned += 1;
        let mut handle = std::fs::File::open(&file).map_err(|error| {
            std::io::Error::new(
                error.kind(),
                format!("failed to open {}: {error}", file.display()),
            )
        })?;
        let mut signature = [0; 8];
        let signature_len = handle.read(&mut signature).map_err(|error| {
            std::io::Error::new(
                error.kind(),
                format!("failed to read {}: {error}", file.display()),
            )
        })?;
        if !has_cfb_signature(&signature[..signature_len]) {
            summary.non_cfb_files += 1;
            continue;
        }
        handle.seek(SeekFrom::Start(0)).map_err(|error| {
            std::io::Error::new(
                error.kind(),
                format!("failed to rewind {}: {error}", file.display()),
            )
        })?;
        let mut cfb = cfb::CompoundFile::open(handle).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("failed to parse CFB {}: {error}", file.display()),
            )
        })?;
        summary.cfb_containers += 1;
        let stream_paths: Vec<_> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|entry| entry.path().to_path_buf())
            .collect();

        for stream_path in stream_paths {
            let mut stream = cfb.open_stream(&stream_path).map_err(|error| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "failed to open stream {} in {}: {error}",
                        stream_path.display(),
                        file.display()
                    ),
                )
            })?;
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).map_err(|error| {
                std::io::Error::new(
                    error.kind(),
                    format!(
                        "failed to read stream {} in {}: {error}",
                        stream_path.display(),
                        file.display()
                    ),
                )
            })?;
            summary.streams_scanned += 1;
            let records = walk_records(&bytes);
            let stream_name = stream_path.to_string_lossy().into_owned();
            let source_domain = classify_source(&file, &stream_name);

            for (index, record) in records.iter().copied().enumerate() {
                if record.type_code != IGCIRCLE2D_TYPE_CODE {
                    continue;
                }
                let previous = index.checked_sub(1).map(|i| records[i]);
                let next = records.get(index + 1).copied();
                let payload = &bytes[record.payload_range()];
                summary.circle_records += 1;
                *summary.source_totals.entry(source_domain).or_insert(0) += 1;
                summary
                    .buckets
                    .entry((source_domain, record.btf))
                    .or_default()
                    .observe(
                        record.raw_type,
                        previous.map(|value| value.type_code),
                        next.map(|value| value.type_code),
                        payload,
                    );
                if dump_records {
                    summary.dumps.push(RecordDump {
                        file: file.clone(),
                        stream_path: stream_name.clone(),
                        source_domain,
                        record,
                        previous,
                        next,
                        payload: payload.to_vec(),
                    });
                }
            }
        }
    }
    Ok(summary)
}

fn type_and_range(record: Option<RecordMeta>) -> String {
    record.map_or_else(
        || "none".to_owned(),
        |record| {
            format!(
                "0x{:04X}@[{}..{})",
                record.type_code, record.start, record.end
            )
        },
    )
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_f64(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".to_owned(), |value| format!("{value:.17e}"))
}

fn print_summary(summary: &ScanSummary) {
    for dump in &summary.dumps {
        println!(
            "RECORD file={} stream={} domain={} range=[{}..{}) raw_type=0x{:04X} flags=0x{:04X} btf={} prev={} next={}",
            dump.file.display(),
            dump.stream_path,
            dump.source_domain,
            dump.record.start,
            dump.record.end,
            dump.record.raw_type,
            dump.record.raw_type & 0xC000,
            dump.record.btf,
            type_and_range(dump.previous),
            type_and_range(dump.next),
        );
        println!("  payload: {}", hex(&dump.payload));
        if dump.payload.len() >= 8 {
            for offset in 0..=dump.payload.len() - 8 {
                let value = f64_at(&dump.payload[offset..offset + 8]);
                println!(
                    "  f64@+{offset:03}: bits=0x{:016X} value={value:.17e} finite={}",
                    value.to_bits(),
                    value.is_finite()
                );
            }
        }
    }

    println!("==================== SUMMARY ====================");
    println!(
        "files scanned: {}, valid CFB containers: {}, non-CFB files: {}, streams scanned: {}",
        summary.files_scanned,
        summary.cfb_containers,
        summary.non_cfb_files,
        summary.streams_scanned
    );
    println!("0x0059 records: {}", summary.circle_records);
    println!("source domains:");
    for (domain, count) in &summary.source_totals {
        println!("  {domain}: {count}");
    }
    println!("layout buckets:");
    for ((domain, btf), bucket) in &summary.buckets {
        println!("  domain={domain} btf={btf} records={}", bucket.records);
        for (raw_type, count) in &bucket.raw_types {
            println!(
                "    raw_type=0x{raw_type:04X} flags=0x{:04X} count={count}",
                raw_type & 0xC000
            );
        }
        for ((previous, next), count) in &bucket.neighbors {
            println!(
                "    neighbors prev={} next={} count={count}",
                previous.map_or_else(|| "none".to_owned(), |value| format!("0x{value:04X}")),
                next.map_or_else(|| "none".to_owned(), |value| format!("0x{value:04X}")),
            );
        }
        for (value, count) in &bucket.terminal_bytes {
            println!("    terminal_byte=0x{value:02X} count={count}");
        }
        for (offset, stats) in &bucket.f64_candidates {
            println!(
                "    f64@+{offset:03} seen={} finite={}/{} distinct_bits={} min={} max={}",
                stats.seen,
                stats.finite,
                stats.seen,
                stats.distinct_bits.len(),
                format_f64(stats.min),
                format_f64(stats.max),
            );
        }
    }
}

fn parse_args() -> Result<(PathBuf, bool), String> {
    let mut root = None;
    let mut dump_records = false;
    for arg in std::env::args().skip(1) {
        if arg == "--dump-records" {
            dump_records = true;
        } else if arg.starts_with('-') {
            return Err(format!("unknown option: {arg}"));
        } else if root.replace(PathBuf::from(arg)).is_some() {
            return Err("expected one file-or-dir argument".to_owned());
        }
    }
    let root = root
        .ok_or_else(|| "usage: probe_igcircle2d_shape [--dump-records] <file-or-dir>".to_owned())?;
    if !root.exists() {
        return Err(format!("input does not exist: {}", root.display()));
    }
    Ok((root, dump_records))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (root, dump_records) = parse_args()
        .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message))?;
    print_summary(&scan(&root, dump_records)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(raw_type: u16, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(6 + payload.len());
        out.extend_from_slice(&raw_type.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn canonical_chain_finds_target_and_neighbors() {
        let mut bytes = record(0x0018, &[0; 8]);
        bytes.extend(record(0x4059, &[0; 8]));
        bytes.extend(record(0x0084, &[0; 8]));

        let records = walk_records(&bytes);

        assert_eq!(records.len(), 3);
        assert_eq!(records[1].type_code, IGCIRCLE2D_TYPE_CODE);
        assert_eq!(records[0].type_code, 0x0018);
        assert_eq!(records[2].type_code, 0x0084);
    }

    #[test]
    fn truncated_header_is_not_counted() {
        assert!(walk_records(&[0x59, 0x00, 0x08]).is_empty());
    }

    #[test]
    fn oversized_btf_is_not_counted() {
        let mut bytes = vec![0x59, 0x00];
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());

        assert!(walk_records(&bytes).is_empty());
    }

    #[test]
    fn false_neighbor_does_not_admit_record() {
        let mut bytes = record(0x0059, &[0; 8]);
        bytes.extend_from_slice(&[0; 6]);

        assert!(walk_records(&bytes).is_empty());
    }

    #[test]
    fn plausible_neighbor_without_complete_chain_is_rejected() {
        let mut bytes = record(0x0059, &[0; 8]);
        bytes.extend(record(0x0018, &[0; 8]));
        bytes.extend_from_slice(&[0; 6]);

        assert!(walk_records(&bytes).is_empty());
    }

    #[test]
    fn adversarial_lengths_do_not_panic() {
        for len in 0..128 {
            let bytes = vec![0xFF; len];
            assert!(std::panic::catch_unwind(|| walk_records(&bytes)).is_ok());
        }
    }

    #[test]
    fn non_finite_f64_is_reported() {
        let bytes = record(0x0059, &f64::NAN.to_le_bytes());
        let records = walk_records(&bytes);
        let mut stats = F64Stats::default();

        stats.observe(f64_at(&bytes[records[0].payload_range()]));

        assert_eq!(stats.seen, 1);
        assert_eq!(stats.finite, 0);
        assert_eq!(stats.distinct_bits.len(), 1);
    }

    #[test]
    fn layout_bucket_tracks_terminal_byte() {
        let mut stats = BucketStats::default();
        stats.observe(0x0059, None, None, &[0, 0, 0, 0, 0, 0, 0, 1]);

        assert_eq!(stats.terminal_bytes.get(&1), Some(&1));
    }

    #[test]
    fn nested_jsite_domain_requires_pid_container() {
        assert_eq!(
            classify_source(Path::new("fixture.pid"), "/JSite12\\PSMcluster0"),
            SourceDomain::NestedPidJsite
        );
        assert_eq!(
            classify_source(Path::new("fixture.sym"), "/JSite12\\PSMcluster0"),
            SourceDomain::Other
        );
        assert_eq!(
            classify_source(Path::new("fixture.sym"), "/Sheet6"),
            SourceDomain::SymbolSheet
        );
    }

    #[test]
    fn cfb_signature_requires_the_full_magic() {
        assert!(has_cfb_signature(&CFB_SIGNATURE));
        assert!(!has_cfb_signature(&CFB_SIGNATURE[..7]));
        assert!(!has_cfb_signature(&[0; 8]));
    }
}
