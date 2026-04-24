//! CLI: scan an extracted `Export.msda.bin` (the raw SQL Server
//! backup byte stream) and report MDF-page statistics.
//!
//! Stage-0 reconnaissance tool that feeds stage-1 MDF parsing: we
//! need to know where page 0 lives (the backup stream typically
//! starts with a header before the first MDF page), which page
//! types are present, and how many valid headers are recoverable.
//!
//! Usage:
//!
//! ```text
//! pid_msda_probe <MSDA body file> [--base OFFSET] [--stride N] [--sample N]
//! ```
//!
//! Flags:
//!
//! * `--base OFFSET` — byte offset at which to start walking pages.
//!   If omitted the probe auto-detects it by scanning for the first
//!   valid header.
//! * `--stride N` — stride between candidate pages. Defaults to the
//!   MDF page size (8192 bytes).
//! * `--sample N` — emit headers for the first N valid pages in
//!   text form. Default 20, use 0 to disable.
//!
//! Exit codes: 0 = walk completed, 1 = no valid pages found, 2 =
//! usage error.

use pid_parse::backup::mdf_page::{
    MdfPageCursor, MdfPageHeader, PageType, MIN_HEADER_BYTES, PAGE_SIZE,
};
use pid_parse::backup::text_scan::{find_ascii_run_containing, find_utf16le_run_containing};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
struct CliOptions {
    input: PathBuf,
    base: Option<usize>,
    stride: usize,
    sample: usize,
    /// One or more strings to search inside each page's body.
    /// Each needle is tried as both ASCII and UTF-16LE, so a single
    /// `--search` flag covers both SQL Server identifier columns
    /// (UTF-16LE) and row data that happens to be 8-bit encoded.
    search: Vec<String>,
}

fn print_usage() {
    eprintln!(
        "Usage: pid_msda_probe <msda.bin> [--base OFFSET] [--stride N] [--sample N]\n\
         \x20              [--search STRING]...\n\n\
         --base    byte offset at which MDF pages begin (auto-detected if omitted)\n\
         --stride  bytes between page headers (default 8192)\n\
         --sample  headers to print for diagnostics (default 20, 0 = none)\n\
         --search  search each page body for STRING (as both ASCII and UTF-16LE);\n\
         \x20        flag is repeatable"
    );
}

fn parse_args(args: &[String]) -> Result<CliOptions, String> {
    if args.len() < 2 {
        return Err("missing input path".into());
    }
    let input = PathBuf::from(&args[1]);
    let mut base: Option<usize> = None;
    let mut stride: usize = PAGE_SIZE;
    let mut sample: usize = 20;
    let mut search: Vec<String> = Vec::new();
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--base" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--base requires an offset".to_string())?;
                base = Some(parse_uint(value).map_err(|e| format!("--base: {e}"))?);
                i += 2;
            }
            "--stride" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--stride requires a number".to_string())?;
                stride = parse_uint(value).map_err(|e| format!("--stride: {e}"))?;
                if stride == 0 {
                    return Err("--stride must be positive".into());
                }
                i += 2;
            }
            "--sample" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--sample requires a number".to_string())?;
                sample = parse_uint(value).map_err(|e| format!("--sample: {e}"))?;
                i += 2;
            }
            "--search" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--search requires STRING".to_string())?;
                if value.is_empty() {
                    return Err("--search STRING must be non-empty".into());
                }
                search.push(value.clone());
                i += 2;
            }
            other => return Err(format!("unknown flag: {other}")),
        }
    }
    Ok(CliOptions {
        input,
        base,
        stride,
        sample,
        search,
    })
}

/// Parse an unsigned integer literal that may be written in decimal
/// (`4096`) or hex (`0x1000`).
fn parse_uint(s: &str) -> Result<usize, String> {
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        usize::from_str_radix(hex, 16).map_err(|e| format!("invalid hex `{s}`: {e}"))
    } else {
        s.parse::<usize>()
            .map_err(|e| format!("invalid decimal `{s}`: {e}"))
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_usage();
        std::process::exit(0);
    }
    let options = match parse_args(&args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("argument error: {e}");
            print_usage();
            std::process::exit(2);
        }
    };

    if let Err(e) = run(options) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(options: CliOptions) -> Result<(), String> {
    let data = std::fs::read(&options.input)
        .map_err(|e| format!("read {}: {e}", options.input.display()))?;

    let base = match options.base {
        Some(b) => b,
        None => auto_detect_base(&data, options.stride)?,
    };

    let total_candidates = (data.len().saturating_sub(base)) / options.stride;
    let mut valid_headers = 0usize;
    let mut by_type: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut samples: Vec<(usize, usize, MdfPageHeader)> = Vec::new();
    // Map: needle -> list of (page_index, page_offset, hit_kind, hit_offset_in_body)
    let mut search_hits: BTreeMap<String, Vec<SearchHit>> = options
        .search
        .iter()
        .map(|s| (s.clone(), Vec::new()))
        .collect();

    for (idx, offset, header) in MdfPageCursor::new(&data, base, options.stride) {
        valid_headers += 1;
        *by_type.entry(header.page_type.tag()).or_insert(0) += 1;
        if samples.len() < options.sample {
            samples.push((idx, offset, header));
        }

        if !options.search.is_empty() {
            let page_end = (offset + PAGE_SIZE).min(data.len());
            let body = &data[offset..page_end];
            for needle in &options.search {
                if let Some(hit) = find_ascii_run_containing(body, needle) {
                    search_hits
                        .entry(needle.clone())
                        .or_default()
                        .push(SearchHit {
                            page_index: idx,
                            page_offset: offset,
                            hit_kind: HitKind::Ascii,
                            body_offset: hit,
                        });
                }
                if let Some(hit) = find_utf16le_run_containing(body, needle) {
                    search_hits
                        .entry(needle.clone())
                        .or_default()
                        .push(SearchHit {
                            page_index: idx,
                            page_offset: offset,
                            hit_kind: HitKind::Utf16Le,
                            body_offset: hit,
                        });
                }
            }
        }
    }

    println!("Source:       {}", options.input.display());
    println!("Size:         {} bytes", data.len());
    println!("Page size:    {PAGE_SIZE} bytes");
    println!("Stride:       {} bytes", options.stride);
    println!("Base offset:  0x{base:08X} ({base})");
    println!("Candidates:   {total_candidates}");
    println!(
        "Valid pages:  {}  ({:.1}%)",
        valid_headers,
        percent(valid_headers, total_candidates)
    );

    println!();
    println!("== Page type distribution ==");
    for (tag, n) in &by_type {
        println!("  {tag:>9}  {n}");
    }

    if !samples.is_empty() {
        println!();
        println!("== Sample page headers (first {}) ==", samples.len());
        for (idx, offset, header) in &samples {
            println!(
                "  [{:>4}] @0x{:08X}  type={:>9} ({:>2})  slots={:>5}  free={:>5}  page_id=({},{})",
                idx,
                offset,
                header.page_type.tag(),
                page_type_raw(&header.page_type),
                header.slot_count,
                header.free_count,
                header.page_id.file_id,
                header.page_id.page_id,
            );
        }
    }

    if !search_hits.is_empty() {
        println!();
        println!("== Search results ==");
        for (needle, hits) in &search_hits {
            if hits.is_empty() {
                println!("  `{needle}` — no matches");
                continue;
            }
            println!("  `{needle}` — {} hit(s):", hits.len());
            for hit in hits.iter().take(20) {
                println!(
                    "    page #{:>4} @0x{:08X}  {:>7}  body+0x{:04X}",
                    hit.page_index,
                    hit.page_offset,
                    hit.hit_kind.label(),
                    hit.body_offset
                );
            }
            if hits.len() > 20 {
                println!("    ... ({} more)", hits.len() - 20);
            }
        }
    }

    if valid_headers == 0 {
        return Err("no valid MDF page headers found at the chosen base/stride".into());
    }
    Ok(())
}

/// A single needle-in-page match produced by the `--search` scan.
struct SearchHit {
    page_index: usize,
    page_offset: usize,
    hit_kind: HitKind,
    body_offset: usize,
}

/// Which string encoding produced the match; the CLI surfaces both
/// so reviewers can tell whether the hit came from raw row bytes
/// (typically ASCII) or column-name metadata (typically UTF-16LE).
enum HitKind {
    Ascii,
    Utf16Le,
}

impl HitKind {
    fn label(&self) -> &'static str {
        match self {
            Self::Ascii => "ASCII",
            Self::Utf16Le => "UTF-16LE",
        }
    }
}

/// Render a page-type back into the raw numeric value that stored
/// on disk. Used by the sample output for humans cross-referencing
/// with Paul Randal's page-type table.
fn page_type_raw(t: &PageType) -> u8 {
    match t {
        PageType::DataPage => 1,
        PageType::IndexPage => 2,
        PageType::TextMixPage => 3,
        PageType::TextTreePage => 4,
        PageType::SortPage => 7,
        PageType::Gam => 8,
        PageType::Sgam => 9,
        PageType::Iam => 10,
        PageType::Pfs => 11,
        PageType::BootPage => 13,
        PageType::FileHeaderPage => 14,
        PageType::DiffMap => 15,
        PageType::BulkChangeMap => 16,
        PageType::Reserved17 => 17,
        PageType::AllocUnit => 18,
        PageType::CompressedBackupPage => 19,
        PageType::FileGroupExtentMap => 20,
        PageType::ConflictLog => 21,
        PageType::Reserved22 => 22,
        PageType::Unknown(v) => *v,
    }
}

fn percent(num: usize, den: usize) -> f64 {
    if den == 0 {
        0.0
    } else {
        (num as f64) * 100.0 / (den as f64)
    }
}

/// Scan for the first offset on a 16-byte grid whose bytes parse as
/// a valid MDF page header. This recovers the backup stream's
/// "page 0" offset when the caller does not know it ahead of time.
///
/// Returns an error when the whole file yields no plausible header.
fn auto_detect_base(data: &[u8], stride: usize) -> Result<usize, String> {
    // Fine-grained scan at 16-byte granularity; the real base
    // offset we observed in the fixture is 0x143F0 (not a multiple
    // of 8192), so we cannot rely on page-aligned scanning here.
    const GRID: usize = 16;
    let max_candidate = data.len().saturating_sub(MIN_HEADER_BYTES);
    let mut best: Option<(usize, usize)> = None;
    let mut offset = 0;
    while offset <= max_candidate {
        if let Some(header) = MdfPageHeader::probe(&data[offset..]) {
            if header.header_version == 0x01 {
                // Stronger confidence: verify a second candidate
                // `stride` bytes further lands on another valid
                // header. This filters one-off coincidences in
                // the backup stream's header region.
                let next_offset = offset + stride;
                if next_offset + MIN_HEADER_BYTES <= data.len()
                    && MdfPageHeader::probe(&data[next_offset..]).is_some()
                {
                    best = Some((offset, 1));
                    break;
                }
            }
        }
        offset += GRID;
    }
    match best {
        Some((off, _)) => Ok(off),
        None => Err("could not auto-detect an MDF page base offset".into()),
    }
}
