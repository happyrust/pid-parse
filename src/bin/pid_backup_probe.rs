//! CLI: scan a SmartPlant backup `Export.dmp` (an MTF envelope around
//! SQL Server backup pages) and print the block layout. Stage-0 tool
//! used to validate the [`pid_parse::backup::mtf`] parser on real
//! fixtures without committing large binaries to the repo.
//!
//! Usage:
//!
//! ```text
//! pid_backup_probe <path-to-Export.dmp> [--json]
//! ```
//!
//! Output is human-readable by default — one line per descriptor
//! block — or one JSON document summarizing the whole stream when
//! `--json` is present.
//!
//! Exit codes: 0 = walked to end-of-stream, 1 = I/O / parse error.

use pid_parse::backup::mtf::{
    detect_logical_block_size, MtfBlockCursor, MtfHeader, MtfStreamCursor, MtfStreamKind,
};
use pid_parse::backup::{parse_msci, MsciConfig};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, serde::Serialize)]
struct StreamRecord {
    tag: String,
    header_offset: usize,
    body_offset: usize,
    /// Effective body byte count, derived from the scan to the next
    /// stream tag (so it agrees with how much data extraction would
    /// produce).
    body_len: usize,
    /// Declared length word from the header, surfaced for
    /// diagnostics.
    declared_length: u64,
}

#[derive(Debug, serde::Serialize)]
struct BlockRecord {
    index: usize,
    tag: String,
    offset: usize,
    size: usize,
    streams: Vec<StreamRecord>,
}

#[derive(Debug, serde::Serialize)]
struct MsciFileSummary {
    logical_name: String,
    physical_path: String,
    record_offset: usize,
}

#[derive(Debug, serde::Serialize)]
struct MsciSummary {
    stream_body_offset: usize,
    stream_body_len: usize,
    filegroup_name: Option<String>,
    files: Vec<MsciFileSummary>,
}

#[derive(Debug, serde::Serialize)]
struct ProbeReport {
    source: PathBuf,
    file_size: u64,
    logical_block_size: Option<u32>,
    tape_header_attributes: u32,
    tape_header_os_id: u16,
    block_count: usize,
    counts_by_tag: BTreeMap<String, usize>,
    counts_by_stream_tag: BTreeMap<String, usize>,
    blocks: Vec<BlockRecord>,
    /// Present when an MSCI stream was found and parsed.
    #[serde(skip_serializing_if = "Option::is_none")]
    msci: Option<MsciSummary>,
}

fn print_usage() {
    eprintln!(
        "Usage: pid_backup_probe <Export.dmp> [--json] [--preview N]\n\n\
         --json     emit a single JSON document instead of text\n\
         --preview  cap the per-block list at N entries (default 20, use 0 for all)"
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 || args.iter().any(|a| a == "-h" || a == "--help") {
        print_usage();
        std::process::exit(if args.len() < 2 { 1 } else { 0 });
    }

    let input = PathBuf::from(&args[1]);
    let mut json_output = false;
    let mut preview: usize = 20;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => {
                json_output = true;
                i += 1;
            }
            "--preview" => {
                let value = match args.get(i + 1) {
                    Some(v) => v,
                    None => {
                        eprintln!("--preview requires a number");
                        std::process::exit(1);
                    }
                };
                preview = match value.parse() {
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("--preview: {e}");
                        std::process::exit(1);
                    }
                };
                i += 2;
            }
            other => {
                eprintln!("unknown flag: {other}");
                print_usage();
                std::process::exit(1);
            }
        }
    }

    let data = match std::fs::read(&input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("read {}: {e}", input.display());
            std::process::exit(1);
        }
    };

    let header = match MtfHeader::probe(&data) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("not an MTF stream: {e}");
            std::process::exit(1);
        }
    };

    let logical_block_size = detect_logical_block_size(&data);

    let mut blocks: Vec<BlockRecord> = Vec::new();
    let mut counts_by_tag: BTreeMap<String, usize> = BTreeMap::new();
    let mut counts_by_stream_tag: BTreeMap<String, usize> = BTreeMap::new();
    let mut msci_summary: Option<MsciSummary> = None;

    for (index, block) in MtfBlockCursor::new(&data).enumerate() {
        let tag = block.block_type.tag();
        *counts_by_tag.entry(tag.clone()).or_insert(0) += 1;

        // Scan streams inside this descriptor block. The first
        // stream header lives at offset `offset_to_first_event`
        // inside the DBLK (bytes 8..10 of the common header); stage 0
        // reads it directly rather than recomputing the per-kind
        // layout.
        let offset_to_first_event =
            u16::from_le_bytes([block.raw_common_header[8], block.raw_common_header[9]]) as usize;
        let dblk_body_start = block.offset + offset_to_first_event;
        let dblk_body_end = block.offset + block.size;
        let streams: Vec<StreamRecord> =
            MtfStreamCursor::new(&data, dblk_body_start, dblk_body_end)
                .map(|s| {
                    let stag = s.kind.tag();
                    *counts_by_stream_tag.entry(stag.clone()).or_insert(0) += 1;

                    // Parse the first MSCI stream we see into a
                    // summary for the report. Later MSCI streams
                    // (uncommon) are ignored in stage 0.
                    if matches!(s.kind, MtfStreamKind::SqlConfig) && msci_summary.is_none() {
                        let body_bytes = &data[s.body_offset..s.body_end];
                        if let Ok(cfg) = parse_msci(body_bytes) {
                            msci_summary =
                                Some(summarize_msci(&cfg, s.body_offset, body_bytes.len()));
                        }
                    }

                    StreamRecord {
                        tag: stag,
                        header_offset: s.header_offset,
                        body_offset: s.body_offset,
                        body_len: s.body_len(),
                        declared_length: s.declared_length,
                    }
                })
                .collect();

        blocks.push(BlockRecord {
            index,
            tag,
            offset: block.offset,
            size: block.size,
            streams,
        });
    }

    // Cap the per-block listing for the human-readable path; JSON
    // always gets the full list so downstream tools can filter.
    let display_blocks: Vec<&BlockRecord> = if preview == 0 || json_output {
        blocks.iter().collect()
    } else {
        blocks.iter().take(preview).collect()
    };

    if json_output {
        let report = ProbeReport {
            source: input.clone(),
            file_size: data.len() as u64,
            logical_block_size,
            tape_header_attributes: header.attributes,
            tape_header_os_id: header.os_id,
            block_count: blocks.len(),
            counts_by_tag: counts_by_tag.clone(),
            counts_by_stream_tag: counts_by_stream_tag.clone(),
            blocks,
            msci: msci_summary.clone(),
        };
        match serde_json::to_string_pretty(&report) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("JSON serialize: {e}");
                std::process::exit(1);
            }
        }
    } else {
        println!("Source: {}", input.display());
        println!("File size: {} bytes", data.len());
        match logical_block_size {
            Some(sz) => println!("Detected logical block size: {sz} bytes"),
            None => println!("Detected logical block size: (unknown — no second descriptor found)"),
        }
        println!(
            "TAPE attributes: 0x{:08X}  os_id: 0x{:04X}  os_version: 0x{:04X}",
            header.attributes, header.os_id, header.os_version
        );
        println!("Descriptor blocks: {}", blocks.len());
        println!();
        println!("== Block counts by tag ==");
        for (tag, n) in &counts_by_tag {
            println!("  {tag:>6}  {n}");
        }
        if !counts_by_stream_tag.is_empty() {
            println!();
            println!("== Stream counts by tag (inside DBLKs) ==");
            for (tag, n) in &counts_by_stream_tag {
                println!("  {tag:>6}  {n}");
            }
        }
        println!();
        println!("== Descriptor blocks (first {}) ==", display_blocks.len());
        for b in display_blocks {
            println!(
                "  [{:>4}] @0x{:08X}  {:>6}  size={} B  streams={}",
                b.index,
                b.offset,
                b.tag,
                b.size,
                b.streams.len()
            );
            for s in &b.streams {
                println!(
                    "          └─ {:>6}  header@0x{:08X}  body@0x{:08X}  body_len={} B  declared={} B",
                    s.tag, s.header_offset, s.body_offset, s.body_len, s.declared_length
                );
            }
        }
        if preview != 0 && blocks.len() > preview {
            println!(
                "  ... ({} more; rerun with --preview 0 for full list)",
                blocks.len() - preview
            );
        }

        if let Some(msci) = &msci_summary {
            println!();
            println!(
                "== MSCI configuration (stream body @0x{:08X}, {} B) ==",
                msci.stream_body_offset, msci.stream_body_len
            );
            if let Some(fg) = &msci.filegroup_name {
                println!("  filegroup: {fg}");
            }
            for f in &msci.files {
                println!(
                    "  file SFIN@0x{:06X}  logical=`{}`  path=`{}`",
                    f.record_offset, f.logical_name, f.physical_path
                );
            }
        }
    }
}

/// Shrink a parsed [`MsciConfig`] into the compact summary the probe
/// CLI emits. Kept out-of-band so the main control flow stays
/// readable.
fn summarize_msci(
    cfg: &MsciConfig,
    stream_body_offset: usize,
    stream_body_len: usize,
) -> MsciSummary {
    MsciSummary {
        stream_body_offset,
        stream_body_len,
        filegroup_name: cfg.filegroup_name.clone(),
        files: cfg
            .files
            .iter()
            .map(|f| MsciFileSummary {
                logical_name: f.logical_name.clone(),
                physical_path: f.physical_path.clone(),
                record_offset: f.record_offset,
            })
            .collect(),
    }
}

// Clone-derive for the inline summary struct used in the JSON output.
impl Clone for MsciSummary {
    fn clone(&self) -> Self {
        Self {
            stream_body_offset: self.stream_body_offset,
            stream_body_len: self.stream_body_len,
            filegroup_name: self.filegroup_name.clone(),
            files: self
                .files
                .iter()
                .map(|f| MsciFileSummary {
                    logical_name: f.logical_name.clone(),
                    physical_path: f.physical_path.clone(),
                    record_offset: f.record_offset,
                })
                .collect(),
        }
    }
}
