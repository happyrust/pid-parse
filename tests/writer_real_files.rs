//! Conditional smoke test: round-trip a real `.pid` fixture through the
//! writer layer if `test-file/DWG-0201GP06-01.pid` is present locally. CI
//! runs without `test-file/` (gitignored) — the test prints a notice and
//! short-circuits in that case, matching the convention in
//! `parse_real_files.rs`.

use pid_parse::writer::{PidWriter, WritePlan};
use pid_parse::PidParser;
use std::path::PathBuf;

#[test]
fn real_file_passthrough_roundtrip_preserves_streams() {
    let src = PathBuf::from("test-file/DWG-0201GP06-01.pid");
    if !src.exists() {
        eprintln!(
            "skipping {} round-trip: fixture not present (test-file/ is gitignored)",
            src.display()
        );
        return;
    }

    let parser = PidParser::new();
    let pkg = parser.parse_package(&src).expect("parse_package on real fixture");

    // Write to a sibling tmp path (avoids touching test-file/ itself).
    let dst = std::env::temp_dir().join(format!(
        "pid-parse-real-roundtrip-{}.pid",
        std::process::id()
    ));
    if dst.exists() {
        std::fs::remove_file(&dst).ok();
    }
    PidWriter::write_to(&pkg, &WritePlan::default(), &dst)
        .expect("re-emit real fixture as passthrough");

    let pkg2 = parser
        .parse_package(&dst)
        .expect("re-parse round-tripped real fixture");

    let keys1: Vec<&String> = pkg.streams.keys().collect();
    let keys2: Vec<&String> = pkg2.streams.keys().collect();
    assert_eq!(keys1, keys2, "real fixture stream key set must match");
    for key in keys1 {
        assert_eq!(
            pkg.streams[key].data, pkg2.streams[key].data,
            "real fixture stream {} bytes diverged after passthrough",
            key
        );
    }

    // Best-effort cleanup; ignore failures (e.g. AV holding the handle).
    let _ = std::fs::remove_file(&dst);
}
