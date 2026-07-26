//! Phase 36-B: does the `.sym` record-chain reading hold across the corpus?
//!
//! `probe_sym_container.rs` read two symbols byte by byte and found that a
//! `.sym` `Sheet*` stream is a plain PSM record chain starting at offset 8 --
//! the cluster header's `stream_type` / `body_len` fields are in fact the
//! first record's own 6-byte envelope. On those two files every chain landed
//! exactly on the last byte of the stream.
//!
//! Two files is an anecdote. This probe replays the walk over the whole
//! reference library (`test-file/symbols-full/`, unpacked from the RefData
//! backup zips) and reports:
//!
//! 1. how many symbols parse and how many chains land exactly on the end --
//!    an off-by-anything means the record layout is not what it looks like;
//! 2. which PSM type codes appear, with their payload sizes, so the decoder
//!    covers the shapes that actually occur rather than the three the two
//!    sample files happened to use;
//! 3. the coordinate envelope, which says whether symbol bodies really are
//!    drawn around their own origin in metres.
//!
//! Read-only: no parser, schema, or model change.

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_parse::PidParser;

const CLUSTER_MAGIC: u32 = 0x6C90_F544;
/// Record chains start here: the 8 bytes before are the stream magic and the
/// record count, and the "header" fields a cluster reader would find at 8..16
/// are the first record's envelope.
const CHAIN_START: usize = 8;
const PSM_ENVELOPE_LEN: usize = 6;
/// Payload prefix ahead of the f64 run: oid, parent, a u32, a u16 sub-type,
/// and one more u32 -- read positionally, not yet named.
const PAYLOAD_HEADER_LEN: usize = 18;

fn symbols_root() -> PathBuf {
    let full = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test-file")
        .join("symbols-full");
    if full.is_dir() {
        return full;
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test-file")
        .join("symbols")
}

fn collect_syms(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_syms(&path, out);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("sym"))
        {
            out.push(path);
        }
    }
}

fn u16_le(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2)
        .and_then(|s| <[u8; 2]>::try_from(s).ok())
        .map(u16::from_le_bytes)
}

fn u32_le(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .and_then(|s| <[u8; 4]>::try_from(s).ok())
        .map(u32::from_le_bytes)
}

fn f64_le(b: &[u8], at: usize) -> Option<f64> {
    b.get(at..at + 8)
        .and_then(|s| <[u8; 8]>::try_from(s).ok())
        .map(f64::from_le_bytes)
}

#[derive(Default)]
struct TypeStat {
    count: usize,
    payload_lens: BTreeMap<usize, usize>,
}

#[derive(Default)]
struct Corpus {
    files: usize,
    parse_failed: usize,
    sheets: usize,
    chains_clean: usize,
    chains_short: usize,
    chains_overrun: usize,
    types: BTreeMap<u16, TypeStat>,
    coord_min: f64,
    coord_max: f64,
    coord_samples: usize,
}

fn walk(bytes: &[u8], corpus: &mut Corpus) {
    let mut at = CHAIN_START;
    let mut overrun = false;
    while at + PSM_ENVELOPE_LEN <= bytes.len() {
        let (Some(raw_type), Some(btf)) = (u16_le(bytes, at), u32_le(bytes, at + 2)) else {
            break;
        };
        if raw_type == 0 {
            break;
        }
        let body = at + PSM_ENVELOPE_LEN;
        let Some(end) = body.checked_add(btf as usize) else {
            overrun = true;
            break;
        };
        if end > bytes.len() {
            overrun = true;
            break;
        }
        let type_code = raw_type & 0x0FFF;
        let entry = corpus.types.entry(type_code).or_default();
        entry.count += 1;
        *entry.payload_lens.entry(btf as usize).or_default() += 1;

        // Everything after the positional 18-byte header is an f64 run on the
        // shapes read so far; sampling it says whether a symbol body really
        // sits around its own origin.
        let payload = &bytes[body..end];
        if payload.len() > PAYLOAD_HEADER_LEN {
            let mut off = PAYLOAD_HEADER_LEN;
            while off + 8 <= payload.len() {
                if let Some(v) = f64_le(payload, off) {
                    if v.is_finite() && v.abs() < 1.0e6 {
                        corpus.coord_min = corpus.coord_min.min(v);
                        corpus.coord_max = corpus.coord_max.max(v);
                        corpus.coord_samples += 1;
                    }
                }
                off += 8;
            }
        }
        at = end;
    }
    if overrun {
        corpus.chains_overrun += 1;
    } else if at == bytes.len() {
        corpus.chains_clean += 1;
    } else {
        corpus.chains_short += 1;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = symbols_root();
    let mut syms = Vec::new();
    collect_syms(&root, &mut syms);
    syms.sort();
    if syms.is_empty() {
        println!("no .sym fixtures under {}; skipping", root.display());
        return Ok(());
    }
    println!("scanning {} symbols under {}", syms.len(), root.display());

    let mut corpus = Corpus {
        coord_min: f64::INFINITY,
        coord_max: f64::NEG_INFINITY,
        ..Corpus::default()
    };
    let mut dirty: Vec<String> = Vec::new();

    for sym in &syms {
        corpus.files += 1;
        let Ok(pkg) = PidParser::new().parse_package(sym) else {
            corpus.parse_failed += 1;
            continue;
        };
        for (path, raw) in &pkg.streams {
            let is_sheet = path
                .rsplit('/')
                .next()
                .is_some_and(|n| n.starts_with("Sheet"));
            if !is_sheet || raw.data.len() <= CHAIN_START {
                continue;
            }
            if u32_le(&raw.data, 0) != Some(CLUSTER_MAGIC) {
                continue;
            }
            corpus.sheets += 1;
            let before = (corpus.chains_clean, corpus.chains_overrun);
            walk(&raw.data, &mut corpus);
            if corpus.chains_clean == before.0 && dirty.len() < 12 {
                let name = sym.strip_prefix(&root).unwrap_or(sym.as_path());
                let how = if corpus.chains_overrun > before.1 {
                    "overrun"
                } else {
                    "short"
                };
                dirty.push(format!("{} {path} ({how})", name.display()));
            }
        }
    }

    println!("\n-- chain integrity --");
    println!("  files          : {}", corpus.files);
    println!("  parse failed   : {}", corpus.parse_failed);
    println!("  sheet streams  : {}", corpus.sheets);
    println!(
        "  chain lands exactly on stream end : {} / {} ({:.1}%)",
        corpus.chains_clean,
        corpus.sheets,
        100.0 * corpus.chains_clean as f64 / corpus.sheets.max(1) as f64
    );
    println!("  stopped early  : {}", corpus.chains_short);
    println!("  overran        : {}", corpus.chains_overrun);
    if !dirty.is_empty() {
        println!("  first non-clean streams:");
        for d in &dirty {
            println!("    {d}");
        }
    }

    println!("\n-- PSM type codes in symbol sheets --");
    let total: usize = corpus.types.values().map(|s| s.count).sum();
    let mut rows: Vec<(&u16, &TypeStat)> = corpus.types.iter().collect();
    rows.sort_by_key(|(_, stat)| Reverse(stat.count));
    for (code, stat) in rows {
        let lens: Vec<String> = stat
            .payload_lens
            .iter()
            .map(|(len, n)| format!("{len}x{n}"))
            .take(6)
            .collect();
        println!(
            "  {code:#06x}  {:>7} records ({:>5.1}%)  payload_len: {}{}",
            stat.count,
            100.0 * stat.count as f64 / total.max(1) as f64,
            lens.join(" "),
            if stat.payload_lens.len() > 6 {
                format!(" (+{} more)", stat.payload_lens.len() - 6)
            } else {
                String::new()
            }
        );
    }

    println!("\n-- coordinate envelope (f64s after the 18-byte payload header) --");
    println!(
        "  samples {}  min {:.6}  max {:.6}",
        corpus.coord_samples, corpus.coord_min, corpus.coord_max
    );

    Ok(())
}
