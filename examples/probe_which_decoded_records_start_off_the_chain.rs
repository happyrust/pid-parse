//! Which records does a family decoder accept that its stream's own chain
//! does not start?
//!
//! A `Sheet*` stream is an 8-byte header followed by a chain of PSM records
//! (`u16` type word, `u32 bytes_to_follow`, payload, repeat —
//! `sheet_record_starts`), and so is a symbol cache's `/JSite*/PSMcluster0`.
//! A family whose scan slides a byte at a time accepts a record wherever its
//! own header and payload rules hold, so a byte pattern *inside* another
//! record's payload that happens to satisfy them is decoded as a record of
//! its own. Such a decode is a phantom: the native reader walks the chain
//! and never reads it. `GLine2d` lost its three corpus "records" to this in
//! August
//! (`docs/analysis/2026-08-10-gline2d-is-the-iso-page-ratio-not-a-record.md`),
//! which left the other families unmeasured; this probe measures all of
//! them at once.
//!
//! For every `Sheet*` stream of every drawing (`.pid`) and template (`.igr`,
//! on disk or inside a `RefData~*.zip`) under the directories given, it
//! holds each decoded range of each `SHEET_RECORD_FAMILIES` row against the
//! chain, and for every `/JSite*/PSMcluster0` the five decoders of the
//! symbol-information family (`streams/jsite.rs`). It prints:
//!
//! - per family, how many decodes start on the chain, how many start off
//!   it, and how many start on it and end somewhere else;
//! - for a family with decodes off the chain: the type word of the chain
//!   record each one sits inside, how far into that record's payload it
//!   starts, how many claim bytes past the end of the record they sit in,
//!   and the first twelve in full (a family with fewer than a hundred is
//!   listed whole);
//! - how many geometry entities `build_normalized_geometry` emitted from a
//!   byte range that starts off the chain, by record kind and confidence;
//! - the streams whose chain does not walk at all, and the header-only ones
//!   (8 bytes, record count 0).
//!
//! Run (PowerShell), with no argument for the corpora this machine keeps —
//! the 53-sheet plant backup, the templates in the `RefData~*.zip` beside
//! it, the repository's own `test-file`, and the eleven acceptance sheets:
//!
//! ```powershell
//! cargo run --example probe_which_decoded_records_start_off_the_chain
//! cargo run --example probe_which_decoded_records_start_off_the_chain -- <dir> [<dir> ...]
//! ```
//!
//! Read-only. A directory that is absent is reported and skipped. Exits
//! non-zero when any decode starts off the chain, so it doubles as a check.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_parse::model::SHEET_RECORD_FAMILIES;
use pid_parse::parsers::sheet_records::{
    decode_double_values, decode_flavor_holders, decode_standard_relations,
    decode_symbol_informations, decode_variables, parse_psm_header, sheet_record_starts,
    SHEET_STREAM_HEADER_LEN,
};
use pid_parse::{build_normalized_geometry, PidGeometryConfidence, PidPackage, PidParser};

/// One scan-based decoder of a symbol cache: its name, its type code and the
/// byte ranges it accepts in a stream.
type CacheFamily = (&'static str, u16, fn(&[u8]) -> Vec<core::ops::Range<usize>>);

/// The five decoders `streams/jsite.rs::decode_symbol_information_family`
/// runs over a `/JSite*/PSMcluster0`. The drawable records of a cache are
/// read on the chain already (`decode_nested_geometry`); these five are the
/// ones that go through `PsmRecordDecoder::scan`.
const CACHE_FAMILIES: [CacheFamily; 5] = [
    ("JSymbolInformation", 0x00BD, |data| {
        decode_symbol_informations(data)
            .into_iter()
            .map(|record| record.byte_range)
            .collect()
    }),
    ("DoubleValue", 0x00C7, |data| {
        decode_double_values(data)
            .into_iter()
            .map(|record| record.byte_range)
            .collect()
    }),
    ("Variables", 0x00EA, |data| {
        decode_variables(data)
            .into_iter()
            .map(|record| record.byte_range)
            .collect()
    }),
    ("StandardRelation", 0x006F, |data| {
        decode_standard_relations(data)
            .into_iter()
            .map(|record| record.byte_range)
            .collect()
    }),
    ("JFlavorHolder", 0x00ED, |data| {
        decode_flavor_holders(data)
            .into_iter()
            .map(|record| record.byte_range)
            .collect()
    }),
];

/// How many off-chain decodes of one family are printed in full when the
/// family has a hundred or more.
const LISTED_PER_FAMILY: usize = 12;

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// One document to walk: a label a reader can find it by, and its parse.
struct Document {
    label: String,
    package: PidPackage,
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_files(&path, out);
        } else if path.extension().is_some_and(|extension| {
            extension.eq_ignore_ascii_case("pid") || extension.eq_ignore_ascii_case("igr")
        }) {
            out.push(path);
        }
    }
}

#[cfg(feature = "refdata-archive")]
fn archive_templates(dir: &Path) -> Vec<Document> {
    use pid_parse::refdata_archive::{archive_members, read_archive_file, refdata_archives};
    let mut out = Vec::new();
    for archive in refdata_archives(dir) {
        let Ok(members) = archive_members(&archive, "igr") else {
            continue;
        };
        let zip = archive
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        for member in members {
            let label = format!("{zip}::{}", member.name());
            let Ok(bytes) = read_archive_file(&member) else {
                println!("  {label}: cannot be read");
                continue;
            };
            match PidPackage::from_bytes(&bytes) {
                Ok(package) => out.push(Document { label, package }),
                Err(error) => println!("  {label}: does not parse ({error})"),
            }
        }
    }
    out
}

#[cfg(not(feature = "refdata-archive"))]
fn archive_templates(_dir: &Path) -> Vec<Document> {
    println!("  (refdata-archive feature off: no RefData~*.zip read)");
    Vec::new()
}

/// Every drawing and template file under `dir`.
fn documents_under(dir: &Path) -> Vec<Document> {
    let mut files = Vec::new();
    collect_files(dir, &mut files);
    let mut out = Vec::new();
    for path in files {
        let label = path
            .strip_prefix(dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        match PidParser::new().parse_package(&path) {
            Ok(package) => out.push(Document { label, package }),
            Err(error) => println!("  {label}: does not parse ({error})"),
        }
    }
    out
}

/// What one family decoded, held against the chains of the streams it
/// decoded from.
#[derive(Default)]
struct FamilyCount {
    type_code: u16,
    /// Decodes that start where the chain starts a record and end where
    /// that record ends.
    on_chain: usize,
    /// Decodes that start where no chain record does.
    off_chain: usize,
    /// Decodes that start on the chain and end somewhere else.
    other_end: usize,
    /// Decodes in a stream whose chain does not walk, so there is no chain
    /// to hold them against.
    unchained: usize,
    /// Off-chain decodes per corpus.
    by_corpus: BTreeMap<String, usize>,
    /// Off-chain decodes by the type word of the chain record they sit in.
    hosts: BTreeMap<u16, usize>,
    /// Off-chain decodes by how far into that record's payload they start.
    payload_offsets: BTreeMap<usize, usize>,
    /// Off-chain decodes that claim bytes past the end of the record they
    /// sit in — which nothing stored inside a record can do.
    past_the_host: usize,
    /// Off-chain decodes, in full.
    lines: Vec<String>,
}

#[derive(Default)]
struct Report {
    documents: usize,
    /// `/JSite*/PSMcluster0` symbol caches walked, and those whose chain
    /// does not walk.
    caches: usize,
    streams: usize,
    walked: usize,
    header_only: Vec<String>,
    unwalked: Vec<String>,
    families: BTreeMap<&'static str, FamilyCount>,
    other_ends: Vec<String>,
    /// Entities emitted from a byte range that starts off the chain, by
    /// `(record kind, confidence)`.
    off_chain_entities: BTreeMap<(String, String), usize>,
    entities: usize,
}

impl Report {
    #[allow(clippy::too_many_arguments)]
    fn hold(
        &mut self,
        corpus: &str,
        label: &str,
        stream: &str,
        data: &[u8],
        chain: &BTreeMap<usize, usize>,
        family: &'static str,
        type_code: u16,
        range: &core::ops::Range<usize>,
    ) {
        let count = self.families.entry(family).or_default();
        count.type_code = type_code;
        if chain.is_empty() {
            count.unchained += 1;
            return;
        }
        if let Some(&end) = chain.get(&range.start) {
            if end == range.end {
                count.on_chain += 1;
            } else {
                count.other_end += 1;
                self.other_ends.push(format!(
                    "[{corpus}] {label} {stream} +{}: {family} claims to +{}, its chain record ends at +{end}",
                    range.start, range.end
                ));
            }
            return;
        }
        count.off_chain += 1;
        *count.by_corpus.entry(corpus.to_string()).or_default() += 1;
        let host = chain.range(..range.start).next_back();
        let place = match host {
            Some((&start, &end)) => {
                let type_word = parse_psm_header(data, start)
                    .map_or(0, |header| (header.type_flags << 14) | header.type_code);
                *count.hosts.entry(type_word).or_default() += 1;
                *count
                    .payload_offsets
                    .entry(range.start - start - 6)
                    .or_default() += 1;
                if range.end > end {
                    count.past_the_host += 1;
                }
                format!(
                    "inside the chain record at +{start}..+{end} (type word 0x{type_word:04X}, oid {}), {} bytes into its payload",
                    u32_at(data, start + 6).unwrap_or(0),
                    range.start - start - 6,
                )
            }
            None => "inside the 8-byte stream header".to_string(),
        };
        count.lines.push(format!(
            "[{corpus}] {label} {stream} +{}..+{}: {place}",
            range.start, range.end
        ));
    }
}

/// `start -> end` of every record on a stream's chain; empty when the chain
/// does not walk the stream exactly.
fn chain_of(data: &[u8]) -> BTreeMap<usize, usize> {
    sheet_record_starts(data)
        .into_iter()
        .filter_map(|at| {
            let header = parse_psm_header(data, at)?;
            Some((at, header.body_start + header.bytes_to_follow as usize))
        })
        .collect()
}

fn walk_document(report: &mut Report, corpus: &str, document: &Document) {
    report.documents += 1;
    let label = document.label.as_str();
    let mut chains: BTreeMap<&str, BTreeMap<usize, usize>> = BTreeMap::new();
    for (stream, raw) in &document.package.streams {
        let leaf = stream.rsplit('/').next().unwrap_or("");
        let data = &raw.data;
        if leaf == "PSMcluster0" && stream.starts_with("/JSite") {
            report.caches += 1;
            let chain = chain_of(data);
            if chain.is_empty() {
                report.unwalked.push(format!(
                    "[{corpus}] {label} {stream}: {} bytes (a symbol cache)",
                    data.len()
                ));
            }
            for (family, type_code, ranges) in CACHE_FAMILIES {
                for range in ranges(data) {
                    report.hold(
                        corpus, label, stream, data, &chain, family, type_code, &range,
                    );
                }
            }
            continue;
        }
        if !leaf.starts_with("Sheet") {
            continue;
        }
        report.streams += 1;
        let chain = chain_of(data);
        if !chain.is_empty() {
            report.walked += 1;
        } else {
            let line = format!(
                "[{corpus}] {label} {stream}: {} bytes, magic {:08X}, record count {}",
                data.len(),
                u32_at(data, 0).unwrap_or(0),
                u32_at(data, 4).unwrap_or(u32::MAX),
            );
            if data.len() == SHEET_STREAM_HEADER_LEN {
                report.header_only.push(line);
            } else {
                report.unwalked.push(line);
            }
        }
        for family in SHEET_RECORD_FAMILIES {
            for range in (family.decoded_ranges)(data) {
                report.hold(
                    corpus,
                    label,
                    stream,
                    data,
                    &chain,
                    family.name,
                    family.type_code,
                    &range,
                );
            }
        }
        if !chain.is_empty() {
            chains.insert(stream.as_str(), chain);
        }
    }

    // What the geometry projection made of them: an entity whose byte range
    // starts where no chain record does was built from bytes the native
    // reader never reads as a record.
    let geometry = build_normalized_geometry(&document.package.parsed);
    report.entities += geometry.entities.len();
    for entity in &geometry.entities {
        let (Some(stream), Some(range)) = (
            entity.source.stream_path.as_deref(),
            entity.source.byte_range,
        ) else {
            continue;
        };
        let Some(chain) = chains.get(stream) else {
            continue;
        };
        if chain.contains_key(&range.start) {
            continue;
        }
        let kind = entity
            .source
            .record_kind
            .map_or_else(|| "no record kind".to_string(), |kind| format!("{kind:?}"));
        let confidence = match entity.confidence {
            PidGeometryConfidence::Decoded => "decoded",
            PidGeometryConfidence::Inferred => "inferred",
            PidGeometryConfidence::ProbeOnly => "probe_only",
        };
        *report
            .off_chain_entities
            .entry((kind, confidence.to_string()))
            .or_default() += 1;
    }
}

fn main() {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    // `(label, directory, read the directory's files, read its archives)`.
    let corpora: Vec<(String, PathBuf, bool, bool)> = if args.is_empty() {
        let data_dir = std::env::var_os("PID_TEST_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"D:\work\plant-code\cad\pid-test-data"));
        let acceptance = std::env::var_os("PID_ACCEPTANCE_CORPUS")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"));
        vec![
            (
                "backup".to_string(),
                data_dir.join("backup-plant-data"),
                true,
                false,
            ),
            // The package directory holds the extracted backup too, which the
            // row above has walked; only its archives' templates are read.
            ("package".to_string(), data_dir, false, true),
            (
                "test-file".to_string(),
                Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file"),
                true,
                false,
            ),
            ("acceptance".to_string(), acceptance, true, false),
        ]
    } else {
        args.into_iter()
            .map(|dir| {
                (
                    dir.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    dir,
                    true,
                    true,
                )
            })
            .collect()
    };

    let mut report = Report::default();
    for (corpus, dir, files, archives) in &corpora {
        if !dir.is_dir() {
            println!("[{corpus}] {} is absent; skipped", dir.display());
            continue;
        }
        let mut documents = Vec::new();
        if *files {
            documents.extend(documents_under(dir));
        }
        if *archives {
            documents.extend(archive_templates(dir));
        }
        println!(
            "[{corpus}] {}: {} document(s)",
            dir.display(),
            documents.len()
        );
        for document in &documents {
            walk_document(&mut report, corpus, document);
        }
    }

    println!("\n=== streams ===");
    println!(
        "  {} document(s); {} Sheet stream(s): {} whose chain walks, {} header-only, the rest listed below; {} symbol cache(s)",
        report.documents,
        report.streams,
        report.walked,
        report.header_only.len(),
        report.caches
    );
    println!("\n=== header-only Sheet streams (8 bytes, no record at all) ===");
    if report.header_only.is_empty() {
        println!("  none");
    }
    for line in &report.header_only {
        println!("  {line}");
    }
    println!("\n=== streams whose chain does not walk ===");
    if report.unwalked.is_empty() {
        println!("  none");
    }
    for line in &report.unwalked {
        println!("  {line}");
    }

    println!("\n=== decoded records by family ===");
    println!(
        "  {:<20} {:<6} {:>9} {:>10} {:>10} {:>9}",
        "family", "code", "on-chain", "off-chain", "other-end", "no-chain"
    );
    let (mut on_chain, mut off_chain, mut other_end, mut unchained) = (0, 0, 0, 0);
    for (name, count) in &report.families {
        println!(
            "  {name:<20} 0x{:04X} {:>9} {:>10} {:>10} {:>9}",
            count.type_code, count.on_chain, count.off_chain, count.other_end, count.unchained
        );
        on_chain += count.on_chain;
        off_chain += count.off_chain;
        other_end += count.other_end;
        unchained += count.unchained;
    }
    println!(
        "  {:<20} {:<6} {on_chain:>9} {off_chain:>10} {other_end:>10} {unchained:>9}",
        "total", ""
    );

    println!("\n=== decodes that start off the chain, by family ===");
    if off_chain == 0 {
        println!("  none");
    }
    for (name, count) in &report.families {
        if count.off_chain == 0 {
            continue;
        }
        println!(
            "\n  {name}: {} off the chain, {} on it; {} of the off-chain decodes claim bytes past the end of the record they sit in",
            count.off_chain, count.on_chain, count.past_the_host
        );
        println!(
            "    by corpus: {}",
            count
                .by_corpus
                .iter()
                .map(|(corpus, n)| format!("{corpus} {n}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let mut hosts: Vec<(&u16, &usize)> = count.hosts.iter().collect();
        hosts.sort_by(|a, b| b.1.cmp(a.1));
        println!(
            "    inside a chain record of type word: {}",
            hosts
                .iter()
                .map(|(word, n)| format!("0x{word:04X} x{n}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let mut offsets: Vec<(&usize, &usize)> = count.payload_offsets.iter().collect();
        offsets.sort_by(|a, b| b.1.cmp(a.1));
        println!(
            "    bytes into that record's payload: {}{}",
            offsets
                .iter()
                .take(8)
                .map(|(offset, n)| format!("+{offset} x{n}"))
                .collect::<Vec<_>>()
                .join(", "),
            if offsets.len() > 8 {
                format!(", … {} more offsets", offsets.len() - 8)
            } else {
                String::new()
            }
        );
        let listed = if count.lines.len() < 100 {
            count.lines.len()
        } else {
            LISTED_PER_FAMILY
        };
        for line in count.lines.iter().take(listed) {
            println!("    {line}");
        }
        if count.lines.len() > listed {
            println!("    … {} more", count.lines.len() - listed);
        }
    }

    println!("\n=== decodes on the chain that end elsewhere ===");
    if report.other_ends.is_empty() {
        println!("  none");
    }
    for line in &report.other_ends {
        println!("  {line}");
    }

    println!(
        "\n=== entities built from a byte range that starts off the chain ({} entities in all) ===",
        report.entities
    );
    if report.off_chain_entities.is_empty() {
        println!("  none");
    }
    for ((kind, confidence), count) in &report.off_chain_entities {
        println!("  {count:>7} x record kind {kind}, {confidence}");
    }

    if off_chain > 0 {
        std::process::exit(1);
    }
}
