//! Census every live `0x0063 igEllipse2d` / `0x007E igEllipticalArc2d`
//! record across the whole corpus and report the distribution of
//! `ratio` values past `1.0` -- the evidence behind dropping the
//! `ratio <= 1.0` cap from `ellipse_geometry` / `elliptical_arc_geometry`
//! (`src/parsers/sheet_records.rs`).
//!
//! Scans three sources:
//!
//! 1. every plain `.pid` / `.sym` file under the corpus roots;
//! 2. every `.sym` member of every `RefData~*.zip` found alongside a
//!    `backup-plant-data` directory;
//! 3. the repository's own `test-file/` fixtures.
//!
//! For each live record (chain-gated the same way the decoder is, but
//! without the `ratio` cap so records the shipped decoder used to
//! refuse are still counted), prints the file/member, stream, byte
//! offset and `ratio`. Ends with a summary: how many records have
//! `ratio > 1.0`, which distinct values, and whether any live outside
//! the two nozzle-label symbols the `2026-11-09` note documents.
//!
//! Read-only probe; run with
//! `cargo run -p pid-parse --example probe_ellipse_ratio_census --features refdata-archive`.
//! Soft-skips any root that does not exist on this machine.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{
    decode_igellipses, decode_igellipticalarcs, PSM_TYPE_CODE_IGELLIPSE2D,
    PSM_TYPE_CODE_IGELLIPTICALARC2D,
};

/// One over-the-cap record, named for where it was found.
struct Hit {
    source: String,
    stream: String,
    byte_offset: usize,
    type_code: u16,
    ratio: f64,
}

/// Decode every `igEllipse2d` / `igEllipticalArc2d` record in one
/// stream's bytes and push every one whose `ratio` exceeds `1.0` onto
/// `hits`, tagged with `source`/`stream` for reporting. Also returns
/// the total live-record count (both families) so the summary line
/// can report the denominator.
fn scan_stream(source: &str, stream: &str, data: &[u8], hits: &mut Vec<Hit>) -> usize {
    let ellipses = decode_igellipses(data);
    let arcs = decode_igellipticalarcs(data);
    let total = ellipses.len() + arcs.len();
    for e in &ellipses {
        if e.ratio > 1.0 {
            hits.push(Hit {
                source: source.to_string(),
                stream: stream.to_string(),
                byte_offset: e.byte_range.start,
                type_code: PSM_TYPE_CODE_IGELLIPSE2D,
                ratio: e.ratio,
            });
        }
    }
    for a in &arcs {
        if a.ratio > 1.0 {
            hits.push(Hit {
                source: source.to_string(),
                stream: stream.to_string(),
                byte_offset: a.byte_range.start,
                type_code: PSM_TYPE_CODE_IGELLIPTICALARC2D,
                ratio: a.ratio,
            });
        }
    }
    total
}

/// Parse `path` as a compound file and scan every `/Sheet*` stream,
/// accumulating into `hits`. Silently skips files that are not valid
/// compound files (non-`.pid`/`.sym` extensions slip in from a
/// directory walk and are not worth failing the census over).
fn scan_compound_file(path: &Path, hits: &mut Vec<Hit>) -> usize {
    let Ok(package) = pid_parse::PidParser::new().parse_package(path) else {
        return 0;
    };
    let source = path.display().to_string();
    let mut total = 0;
    for (stream_path, raw) in &package.streams {
        if stream_path
            .rsplit('/')
            .next()
            .unwrap_or("")
            .starts_with("Sheet")
        {
            total += scan_stream(&source, stream_path, &raw.data, hits);
        }
    }
    total
}

/// Recursively visit every `.pid`/`.sym` file under `dir`.
fn walk_dir(dir: &Path, hits: &mut Vec<Hit>, total: &mut usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            walk_dir(&path, hits, total);
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("pid") || ext.eq_ignore_ascii_case("sym"))
        {
            *total += scan_compound_file(&path, hits);
        }
    }
}

#[cfg(feature = "refdata-archive")]
fn scan_refdata_archives(dir: &Path, hits: &mut Vec<Hit>, total: &mut usize) {
    use pid_parse::refdata_archive::{archive_members, read_archive_file, refdata_archives};

    for archive in refdata_archives(dir) {
        let Ok(members) = archive_members(&archive, "sym") else {
            continue;
        };
        for member in members {
            let Ok(bytes) = read_archive_file(&member) else {
                continue;
            };
            let Ok(package) =
                pid_parse::PidParser::new().parse_package_from_reader(std::io::Cursor::new(bytes))
            else {
                continue;
            };
            let source = format!("{}::{}", archive.display(), member.name());
            for (stream_path, raw) in &package.streams {
                if stream_path
                    .rsplit('/')
                    .next()
                    .unwrap_or("")
                    .starts_with("Sheet")
                {
                    *total += scan_stream(&source, stream_path, &raw.data, hits);
                }
            }
        }
    }
}

#[cfg(not(feature = "refdata-archive"))]
fn scan_refdata_archives(_dir: &Path, _hits: &mut Vec<Hit>, _total: &mut usize) {
    println!("(refdata-archive feature disabled; skipping RefData~*.zip)");
}

fn main() {
    let mut hits = Vec::new();
    let mut total = 0usize;

    let roots: Vec<PathBuf> = [
        std::env::var_os("PID_TEST_DATA_DIR").map(PathBuf::from),
        Some(PathBuf::from(r"D:\work\plant-code\cad\pid-test-data")),
        Some(PathBuf::from(r"D:\work\cad\pid-test-data")),
        Some(PathBuf::from("test-file")),
    ]
    .into_iter()
    .flatten()
    .filter(|p| p.is_dir())
    .collect();

    let mut seen = BTreeSet::new();
    for root in &roots {
        let canon = root.canonicalize().unwrap_or_else(|_| root.clone());
        if !seen.insert(canon) {
            continue; // same directory reached through a different alias
        }
        println!("scanning {} ...", root.display());
        walk_dir(root, &mut hits, &mut total);
        scan_refdata_archives(root, &mut hits, &mut total);
    }

    if roots.is_empty() {
        println!("no corpus root found; set PID_TEST_DATA_DIR. soft-skip.");
        return;
    }

    println!(
        "\n{total} live 0x0063/0x007E records scanned, {} with ratio > 1.0:",
        hits.len()
    );
    for hit in &hits {
        println!(
            "  {} {} +{} type=0x{:04X} ratio={}",
            hit.source, hit.stream, hit.byte_offset, hit.type_code, hit.ratio
        );
    }

    let distinct: BTreeSet<String> = hits.iter().map(|h| format!("{:.4}", h.ratio)).collect();
    println!("\ndistinct ratio values (4 dp): {distinct:?}");

    let outside_nozzle_symbols = hits
        .iter()
        .filter(|h| !h.source.contains("Nozzle NPD") && !h.source.contains("Nozzle Rating"))
        .count();
    println!(
        "records with ratio > 1.0 outside the two nozzle-label symbols: {outside_nozzle_symbols}"
    );
}
