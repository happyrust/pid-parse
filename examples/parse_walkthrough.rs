//! End-to-end "open a `.pid`, inspect, export to JSON" walkthrough.
//!
//! This is the companion to the crate-level `no_run` sketch in
//! `src/lib.rs`. It shows the reader pipeline in one file: parse a
//! file, enumerate streams, pull out the drawing / general metadata,
//! and serialise the full `PidDocument` as indented JSON.
//!
//! Usage:
//!   cargo run --example parse_walkthrough -- path/to/file.pid
//!
//! If no path is passed, the example falls back to the local A01
//! fixture under `test-file/`. Missing fixture prints a soft-skip
//! notice and exits cleanly — same pattern as the integration tests.

use std::error::Error;
use std::path::PathBuf;

use pid_parse::{ParseOptions, PidError, PidParser};

const FALLBACK_FIXTURE: &str = "test-file/export-test/publish-data/A01/A01.pid";

fn main() -> Result<(), Box<dyn Error>> {
    let path = resolve_input();
    let Some(path) = path else {
        eprintln!(
            "parse_walkthrough: no input given and `{FALLBACK_FIXTURE}` is missing — skipping."
        );
        return Ok(());
    };

    println!("== parse_walkthrough: {} ==", path.display());

    // Shrink the parse for a walkthrough; ParseOptions' defaults
    // decode everything (XML, JSite properties, unknown streams) —
    // for this demo we keep string scans but skip JSite properties to
    // show how to pick a cheaper decode.
    let options = ParseOptions {
        parse_jsite_properties: false,
        ..ParseOptions::default()
    };
    let doc = match PidParser::with_options(options).parse_file(&path) {
        Ok(d) => d,
        Err(PidError::Io(e)) => {
            eprintln!(
                "parse_walkthrough: io error opening {}: {e}",
                path.display()
            );
            return Ok(());
        }
        Err(other) => return Err(Box::new(other)),
    };

    println!("streams decoded: {}", doc.streams.len());
    println!("unknown streams: {}", doc.unknown_streams.len());
    println!("clusters       : {}", doc.clusters.len());
    println!("jsites         : {}", doc.jsites.len());

    if let Some(meta) = doc.drawing_meta.as_ref() {
        println!("\n-- DrawingMeta --");
        if let Some(n) = meta.drawing_number.as_deref() {
            println!("  drawing_number : {n}");
        }
        if let Some(c) = meta.document_category.as_deref() {
            println!("  category       : {c}");
        }
        if let Some(t) = meta.template_name.as_deref() {
            println!("  template       : {t}");
        }
        println!("  tag count      : {}", meta.tags.len());
    }

    if let Some(gm) = doc.general_meta.as_ref() {
        println!("\n-- GeneralMeta --");
        if let Some(fp) = gm.file_path.as_deref() {
            println!("  file_path : {fp}");
        }
        if let Some(fs) = gm.file_size.as_deref() {
            println!("  file_size : {fs}");
        }
    }

    // Full JSON dump. Pipe stdout to a file to persist the decoded
    // `PidDocument`; here we only announce the byte count so the
    // interactive output stays tidy. (This works end-to-end as of
    // the `SummaryPropertyValue` adjacently-tagged fix — prior to
    // that, serializing any `.pid` with user-defined summary
    // properties was broken.)
    let json = serde_json::to_string_pretty(&doc)?;
    println!(
        "\nPidDocument JSON: {} bytes (pipe to file to persist)",
        json.len()
    );

    Ok(())
}

fn resolve_input() -> Option<PathBuf> {
    let mut args = std::env::args().skip(1);
    if let Some(arg) = args.next() {
        return Some(PathBuf::from(arg));
    }
    let fallback = PathBuf::from(FALLBACK_FIXTURE);
    if fallback.exists() {
        Some(fallback)
    } else {
        None
    }
}
