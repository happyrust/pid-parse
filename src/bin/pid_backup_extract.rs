//! CLI: extract the SQL Server backup payload (MSDA stream body)
//! and the configuration metadata (MSCI stream) from a SmartPlant
//! `Export.dmp` into standalone files.
//!
//! Stage 0 treats the extracted `*.msda.bin` as the raw SQL Server
//! backup byte stream — it is NOT yet a directly-restorable `.mdf`
//! file, because SQL Server stores the database in an
//! extent-sparse format internally. Decoding that format into
//! individual MDF pages is Stage 1's job; extracting the contiguous
//! bytes here gives downstream tooling (our future page parser, or
//! external tools like OrcaMDF) a clean starting point.
//!
//! Usage:
//!
//! ```text
//! pid_backup_extract <Export.dmp> --out <dir>
//! pid_backup_extract <Export.dmp> --out <dir> --dry-run
//! ```
//!
//! Outputs, written to the target directory (created if missing):
//!
//! * `<stem>.msci.json` — parsed MSCI configuration (filegroup +
//!   per-file records)
//! * `<stem>.msci.bin` — raw MSCI stream body (for inspection)
//! * `<stem>.msda.bin` — raw MSDA stream body (the SQL backup
//!   stream)
//!
//! `<stem>` is the input file's stem (e.g. `Export`).
//!
//! Exit codes: 0 = success, 1 = I/O / parse / format error, 2 =
//! usage error.

use pid_parse::backup::mtf::{MtfBlockCursor, MtfStream, MtfStreamCursor, MtfStreamKind};
use pid_parse::backup::{parse_msci, MsciConfig};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct CliOptions {
    input: PathBuf,
    out_dir: PathBuf,
    dry_run: bool,
    /// Also write a `<stem>.mdf` file that skips the SQL-Server
    /// backup-stream leading header (default 1008 bytes) so tools
    /// like OrcaMDF can open the extracted bytes as an MDF. The
    /// leading-header size is auto-detected by finding the first
    /// valid MDF page header on a 16-byte grid, falling back to
    /// the 0x3F0 value observed in SQL Server 2008 R2 fixtures.
    emit_mdf: bool,
}

fn print_usage() {
    eprintln!(
        "Usage: pid_backup_extract <Export.dmp> --out <dir> [--dry-run] [--as-mdf]\n\n\
         Extracts the SQL Server MSDA + MSCI streams from a SmartPlant\n\
         Export.dmp into individual files for offline analysis.\n\n\
         --dry-run     parse everything and report planned outputs\n\
         \x20             without writing any files.\n\
         --as-mdf      additionally emit `<stem>.mdf` — the MSDA body\n\
         \x20             stripped of its leading backup-stream header\n\
         \x20             so external tools (OrcaMDF, Stellar, etc.)\n\
         \x20             can open it as an MDF file."
    );
}

fn parse_args(args: &[String]) -> Result<CliOptions, String> {
    if args.len() < 2 {
        return Err("missing input path".into());
    }
    let input = PathBuf::from(&args[1]);
    let mut out_dir: Option<PathBuf> = None;
    let mut dry_run = false;
    let mut emit_mdf = false;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--out requires a directory path".to_string())?;
                out_dir = Some(PathBuf::from(value));
                i += 2;
            }
            "--dry-run" => {
                dry_run = true;
                i += 1;
            }
            "--as-mdf" => {
                emit_mdf = true;
                i += 1;
            }
            other => return Err(format!("unknown flag: {other}")),
        }
    }
    let out_dir = out_dir.ok_or_else(|| "--out <dir> is required".to_string())?;
    Ok(CliOptions {
        input,
        out_dir,
        dry_run,
        emit_mdf,
    })
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

    // Sanity-check that the file is MTF-shaped before we start
    // writing anything.
    pid_parse::backup::mtf::MtfHeader::probe(&data)
        .map_err(|e| format!("input does not start with an MTF TAPE descriptor: {e}"))?;

    // Walk the stream tree once, collecting the first MSDA + MSCI.
    let mut msci: Option<LocatedStream<'_>> = None;
    let mut msda: Option<LocatedStream<'_>> = None;
    for block in MtfBlockCursor::new(&data) {
        let offset_to_first_event =
            u16::from_le_bytes([block.raw_common_header[8], block.raw_common_header[9]]) as usize;
        let start = block.offset + offset_to_first_event;
        let end = block.offset + block.size;
        for stream in MtfStreamCursor::new(&data, start, end) {
            // Snapshot the byte range first so we can reference the
            // bytes before moving `stream` into the LocatedStream.
            let body_slice = &data[stream.body_offset..stream.body_end];
            match stream.kind {
                MtfStreamKind::SqlConfig if msci.is_none() => {
                    msci = Some(LocatedStream {
                        stream,
                        body: body_slice,
                    });
                }
                MtfStreamKind::SqlData if msda.is_none() => {
                    msda = Some(LocatedStream {
                        stream,
                        body: body_slice,
                    });
                }
                _ => {}
            }
        }
    }

    let msci = msci.ok_or_else(|| "no MSCI stream found in input".to_string())?;
    let msda = msda.ok_or_else(|| "no MSDA stream found in input".to_string())?;

    let config = parse_msci(msci.body).map_err(|e| format!("MSCI parse: {e}"))?;

    let stem = options
        .input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "backup".to_string());
    let msci_json_path = options.out_dir.join(format!("{stem}.msci.json"));
    let msci_bin_path = options.out_dir.join(format!("{stem}.msci.bin"));
    let msda_bin_path = options.out_dir.join(format!("{stem}.msda.bin"));
    let mdf_path = options.out_dir.join(format!("{stem}.mdf"));

    // If the caller requested an MDF rip, auto-detect the leading
    // backup-stream header so we know where the first MDF page
    // starts. Falls back to the canonical 0x3F0 value observed in
    // SQL Server 2008 R2 fixtures when auto-detect returns None.
    let mdf_header_len = if options.emit_mdf {
        Some(detect_backup_stream_header_len(msda.body))
    } else {
        None
    };

    println!("Input:       {}", options.input.display());
    println!(
        "MSCI body:   {} bytes @0x{:08X}",
        msci.body.len(),
        msci.stream.body_offset
    );
    println!(
        "MSDA body:   {} bytes @0x{:08X}",
        msda.body.len(),
        msda.stream.body_offset
    );
    println!();
    println!("MSCI summary:");
    if let Some(fg) = &config.filegroup_name {
        println!("  filegroup: {fg}");
    }
    for f in &config.files {
        println!("  file  logical=`{}`", f.logical_name);
        println!("        physical=`{}`", f.physical_path);
        println!("        SFIN record @0x{:06X}", f.record_offset);
    }
    if let Some(off) = mdf_header_len {
        println!();
        println!(
            "Detected backup-stream header length: {off} bytes (MDF pages start at MSDA+0x{off:X})"
        );
    }
    println!();
    println!("Planned outputs:");
    println!("  {}", msci_json_path.display());
    println!("  {}", msci_bin_path.display());
    println!("  {}  ({} bytes)", msda_bin_path.display(), msda.body.len());
    if let Some(off) = mdf_header_len {
        let mdf_len = msda.body.len().saturating_sub(off);
        println!("  {}  ({} bytes, MDF)", mdf_path.display(), mdf_len);
    }

    if options.dry_run {
        println!();
        println!("Dry run — no files written.");
        return Ok(());
    }

    std::fs::create_dir_all(&options.out_dir)
        .map_err(|e| format!("create {}: {e}", options.out_dir.display()))?;

    std::fs::write(&msci_bin_path, msci.body)
        .map_err(|e| format!("write {}: {e}", msci_bin_path.display()))?;
    std::fs::write(&msda_bin_path, msda.body)
        .map_err(|e| format!("write {}: {e}", msda_bin_path.display()))?;

    let json = serde_json::to_string_pretty(&MsciJsonShape::from(&config))
        .map_err(|e| format!("serialize MSCI JSON: {e}"))?;
    std::fs::write(&msci_json_path, json)
        .map_err(|e| format!("write {}: {e}", msci_json_path.display()))?;

    if let Some(off) = mdf_header_len {
        let mdf_bytes = &msda.body[off..];
        std::fs::write(&mdf_path, mdf_bytes)
            .map_err(|e| format!("write {}: {e}", mdf_path.display()))?;
    }

    println!();
    println!("Wrote:");
    println!("  {}", msci_json_path.display());
    println!("  {}", msci_bin_path.display());
    println!("  {}  ({} bytes)", msda_bin_path.display(), msda.body.len());
    if let Some(off) = mdf_header_len {
        let mdf_len = msda.body.len().saturating_sub(off);
        println!(
            "  {}  ({} bytes, MDF for OrcaMDF)",
            mdf_path.display(),
            mdf_len
        );
    }
    Ok(())
}

/// Locate the byte offset at which the first MDF page starts
/// inside the MSDA body. Walks a 16-byte grid looking for a
/// header whose `m_headerVersion == 0x01` and whose `m_type` is a
/// canonical page type (1..=22), validating the hit by checking
/// that the offset `stride` bytes further also lands on a valid
/// page header.
///
/// Falls back to the canonical 0x3F0 offset observed in SQL
/// Server 2008 R2 SmartPlant fixtures if the scan yields nothing.
fn detect_backup_stream_header_len(msda_body: &[u8]) -> usize {
    use pid_parse::backup::mdf_page::{MdfPageHeader, MIN_HEADER_BYTES, PAGE_SIZE};

    const GRID: usize = 16;
    // Stop scanning at 8 MiB — enough to skip any plausible
    // backup-stream header without chewing through a 19 MB input
    // on corrupt fixtures.
    const SCAN_LIMIT: usize = 8 * 1024 * 1024;
    let limit = msda_body.len().min(SCAN_LIMIT);
    let mut offset = 0usize;
    while offset + MIN_HEADER_BYTES <= limit {
        if MdfPageHeader::probe(&msda_body[offset..]).is_some() {
            let next = offset + PAGE_SIZE;
            if next + MIN_HEADER_BYTES <= msda_body.len()
                && MdfPageHeader::probe(&msda_body[next..]).is_some()
            {
                return offset;
            }
        }
        offset += GRID;
    }
    // Fallback: SQL Server 2008 R2 fixture default.
    0x3F0
}

/// Small wrapper around [`MtfStream`] that also carries the body byte
/// slice. Avoids repeated `&data[s.body_offset..s.body_end]` calls
/// in the walk loop.
struct LocatedStream<'a> {
    stream: MtfStream,
    body: &'a [u8],
}

/// JSON-friendly shape for [`MsciConfig`]. Keeping it in this binary
/// avoids adding `Serialize` to the library type (stage 0 library
/// stays framework-agnostic), while still letting the CLI ship
/// meaningful output.
#[derive(serde::Serialize)]
struct MsciJsonShape<'a> {
    filegroup_name: Option<&'a str>,
    files: Vec<MsciJsonFile<'a>>,
    records: &'a [usize],
}

#[derive(serde::Serialize)]
struct MsciJsonFile<'a> {
    logical_name: &'a str,
    physical_path: &'a str,
    record_offset: usize,
}

impl<'a> From<&'a MsciConfig> for MsciJsonShape<'a> {
    fn from(cfg: &'a MsciConfig) -> Self {
        Self {
            filegroup_name: cfg.filegroup_name.as_deref(),
            files: cfg
                .files
                .iter()
                .map(|f| MsciJsonFile {
                    logical_name: &f.logical_name,
                    physical_path: &f.physical_path,
                    record_offset: f.record_offset,
                })
                .collect(),
            records: &cfg.records,
        }
    }
}

/// Help the compiler see that `Path::join` is available without a
/// spurious "unused import" if the signature shape shifts in future
/// refactors. Stage 0 does not actually call it here, but keeping
/// the import explicit documents intent.
#[allow(dead_code)]
fn _imports_sentinel(p: &Path) {
    let _ = p.join("_");
}
