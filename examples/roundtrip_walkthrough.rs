//! End-to-end "parse → declarative patch → write → re-parse" walkthrough.
//!
//! Rounds out the `examples/` trilogy alongside `parse_walkthrough`
//! (reader) and `publish_walkthrough` (MDF → XML). Demonstrates the
//! writer path: open a `.pid` as a full [`PidPackage`] (model + raw
//! stream bytes), build a [`WritePlan`] that patches the OLE
//! `SummaryInformation` title, hand both to [`PidWriter::write_to_bytes`]
//! for an in-memory round-trip, and re-parse the produced bytes to
//! prove the patch actually landed.
//!
//! Usage:
//!   cargo run --example roundtrip_walkthrough -- path/to/file.pid
//!
//! If no path is passed, the example falls back to the local A01
//! fixture under `test-file/`. Missing fixture prints a soft-skip
//! notice and exits cleanly.

use std::collections::BTreeMap;
use std::error::Error;
use std::path::PathBuf;

use pid_parse::writer::{MetadataUpdates, PidWriter, WritePlan};
use pid_parse::PidPackage;

const FALLBACK_FIXTURE: &str = "test-file/export-test/publish-data/A01/A01.pid";
const NEW_TITLE: &str = "pid-parse roundtrip_walkthrough demo";

fn main() -> Result<(), Box<dyn Error>> {
    let Some(path) = resolve_input() else {
        eprintln!(
            "roundtrip_walkthrough: no input given and `{FALLBACK_FIXTURE}` is missing — \
             skipping."
        );
        return Ok(());
    };

    println!("== roundtrip_walkthrough: {} ==", path.display());

    // 1. Parse into a full `PidPackage`. `from_path` is equivalent
    //    to `PidParser::new().parse_package(path)` and retains every
    //    raw stream alongside the decoded model — required for a
    //    faithful round-trip.
    let pkg_before = PidPackage::from_path(&path)?;
    let title_before = pkg_before
        .parsed
        .summary
        .as_ref()
        .and_then(|s| s.title.clone());
    println!(
        "  title before    : {}",
        title_before.as_deref().unwrap_or("(unset)")
    );
    println!("  streams         : {}", pkg_before.parsed.streams.len());

    // 2. Build a declarative patch. `summary_updates` keys are the
    //    symbolic `SummaryInformation` property names (`title`,
    //    `author`, `subject`, …); values are UTF-8 by default. Every
    //    other writer channel (`drawing_xml`, `stream_replacements`,
    //    `sheet_patches`) is left at its default — the writer skips
    //    no-op work automatically.
    let mut summary_updates: BTreeMap<String, String> = BTreeMap::new();
    summary_updates.insert("title".to_string(), NEW_TITLE.to_string());
    let plan = WritePlan {
        metadata_updates: MetadataUpdates {
            summary_updates,
            ..Default::default()
        },
        ..Default::default()
    };

    // 3. Apply the plan and materialise the result as bytes. For a
    //    disk-backed round-trip use `PidWriter::write_to(&pkg, &plan,
    //    output_path)` instead; both funnel through the same CFB
    //    writer under the hood.
    let patched_bytes = PidWriter::write_to_bytes(&pkg_before, &plan)?;
    println!("  patched bytes   : {}", patched_bytes.len());

    // 4. Re-parse to prove the patch actually landed and the
    //    resulting CFB is still a well-formed `.pid`. Any consumer
    //    can stop at step 3; step 4 is the "did it work?"
    //    assertion.
    let pkg_after = PidPackage::from_bytes(&patched_bytes)?;
    let title_after = pkg_after
        .parsed
        .summary
        .as_ref()
        .and_then(|s| s.title.clone());
    println!(
        "  title after     : {}",
        title_after.as_deref().unwrap_or("(unset)")
    );

    if title_after.as_deref() == Some(NEW_TITLE) {
        println!("\n[ok] summary title patch landed through the round-trip.");
    } else {
        eprintln!(
            "[warn] summary title did NOT land — got {title_after:?}; \
             expected {NEW_TITLE:?}"
        );
        std::process::exit(1);
    }

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
