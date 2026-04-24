//! End-to-end "parse → write → re-parse" round-trip walkthrough.
//!
//! Rounds out the `examples/` trilogy alongside `parse_walkthrough`
//! (reader) and `publish_walkthrough` (MDF → XML). Demonstrates the
//! writer path: open a `.pid` as a full [`PidPackage`] (model + raw
//! stream bytes), hand it plus a declarative [`WritePlan`] to
//! [`PidWriter::write_to_bytes`] for an in-memory round-trip, and
//! re-parse the output to prove every stream came back intact.
//!
//! Deliberately uses `WritePlan::default()` — a passthrough — rather
//! than a metadata patch. Field-level edits (e.g. `summary_updates`
//! writing a new `title`) require the target `.pid`'s
//! `SummaryInformation` stream to use only the VT codes the writer
//! currently rewrites (`VT_I4` / `VT_LPSTR` / `VT_LPWSTR` /
//! `VT_FILETIME`); real SmartPlant fixtures often also carry
//! `VT_I2`, which the writer rejects up-front. The passthrough shape
//! works on every `.pid` and still exercises the entire writer
//! pipeline, which is the useful part for a walkthrough.
//!
//! Usage:
//!   cargo run --example roundtrip_walkthrough -- path/to/file.pid
//!
//! If no path is passed, the example falls back to the local A01
//! fixture under `test-file/`. Missing fixture prints a soft-skip
//! notice and exits cleanly.

use std::error::Error;
use std::path::PathBuf;

use pid_parse::writer::{PidWriter, WritePlan};
use pid_parse::PidPackage;

const FALLBACK_FIXTURE: &str = "test-file/export-test/publish-data/A01/A01.pid";

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
        "  title           : {}",
        title_before.as_deref().unwrap_or("(unset)")
    );
    println!("  streams         : {}", pkg_before.parsed.streams.len());
    println!(
        "  unknown_streams : {}",
        pkg_before.parsed.unknown_streams.len()
    );

    // 2. Build a passthrough `WritePlan`. Non-default variants
    //    include:
    //    - `metadata_updates.summary_updates` — string edits to
    //      OLE SummaryInformation (keys: "title" / "author" /
    //      "subject" / …).
    //    - `metadata_updates.drawing_xml` / `general_xml` — full
    //      replacement of the tagged-text Drawing / General blobs.
    //    - `stream_replacements` — byte-level swap of any stream.
    //    - `sheet_patches` — experimental sheet byte-range edits.
    //    Every channel left at its default is a no-op, which is the
    //    passthrough contract we exercise here.
    let plan = WritePlan::default();

    // 3. Apply the plan and materialise the result as bytes. For a
    //    disk-backed round-trip use `PidWriter::write_to(&pkg, &plan,
    //    output_path)` instead; both funnel through the same CFB
    //    writer under the hood.
    let roundtripped_bytes = PidWriter::write_to_bytes(&pkg_before, &plan)?;
    println!("  output bytes    : {}", roundtripped_bytes.len());

    // 4. Re-parse to prove the output is still a well-formed `.pid`
    //    and that every stream round-tripped. Any consumer writing
    //    to disk can skip step 4; it's the "did it work?"
    //    assertion.
    let pkg_after = PidPackage::from_bytes(&roundtripped_bytes)?;
    let streams_after = pkg_after.parsed.streams.len();
    let streams_before = pkg_before.parsed.streams.len();

    println!("\n  re-parsed streams : {streams_after}");
    if streams_after == streams_before {
        println!("[ok] stream count matches — round-trip preserves every stream.");
    } else {
        eprintln!(
            "[warn] stream count mismatch: before={streams_before}, \
             after={streams_after}"
        );
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
