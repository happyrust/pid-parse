//! Phase 40 gate: a Sheet record that does not reach the drawing is named.
//!
//! Phase 38 S2 made that true for records no decoder claims. It was not true
//! for the other half — a record whose family is wired and whose bytes that
//! family refused — because the census behind the warning tests the *type
//! code*, and a refused record's type code is one this crate decodes. Those
//! records fell out of the decoded output and out of the census both, and on
//! this corpus they outnumber the named drops 141 to 5.
//!
//! The counts below are the measurement, pinned. They are expected to move
//! when a decoder learns a shape it used to refuse; they are not expected to
//! move quietly. See
//! `docs/analysis/2026-08-10-the-silent-bucket-is-refusals-not-unknowns.md`
//! and `examples/probe_phase40_render_gap_census`.
//!
//! Fixtures soft-skip when absent, mirroring `tests/parse_real_files.rs`.

use pid_parse::{build_normalized_geometry, PidParser};

/// `(fixture, refused graphic records, undecoded graphic records)` — both
/// counted in records, not in `(stream, type code)` groups.
const EXPECTED: &[(&str, usize, usize)] = &[
    // Four text records in one shape its decoder refuses.
    ("DWG-0201GP06-01.pid", 4, 0),
    // Five refused text on /Sheet6, plus /Sheet6615 where four refused lines
    // and one undecodable rectangle are the entire stream: it draws nothing.
    ("DWG-0202GP06-01.pid", 9, 1),
    ("工艺管道及仪表流程-1.pid", 21, 0),
    // The one clean sheet in the corpus.
    ("D06.pid", 0, 0),
    // 80 refused lines and 18 refused text on /JSite204/Sheet6 — 88% of that
    // stream's records. It is the emptiest drawing in the corpus and this is
    // why.
    ("export-test/publish-data/A01/A01.pid", 98, 3),
];

#[test]
fn the_corpus_refusal_counts_are_the_measured_ones() {
    let mut checked = 0usize;

    for (fixture, expected_refused, expected_dropped) in EXPECTED {
        let path = format!("test-file/{fixture}");
        if !std::path::Path::new(&path).exists() {
            eprintln!("skipping: fixture {path} not found");
            continue;
        }
        let parsed = PidParser::new()
            .parse_file(&path)
            .unwrap_or_else(|err| panic!("fixture {path} should parse: {err}"));
        let geometry = build_normalized_geometry(&parsed);
        checked += 1;

        let refused: usize = geometry
            .refused_graphic_records
            .iter()
            .map(|entry| entry.count)
            .sum();
        let dropped: usize = geometry
            .dropped_graphic_records
            .iter()
            .map(|entry| entry.count)
            .sum();

        assert_eq!(
            refused, *expected_refused,
            "{fixture}: expected {expected_refused} refused graphic record(s), got {refused} \
             ({:?})",
            geometry.refused_graphic_records
        );
        assert_eq!(
            dropped, *expected_dropped,
            "{fixture}: expected {expected_dropped} undecodable graphic record(s), got {dropped} \
             ({:?})",
            geometry.dropped_graphic_records
        );
    }

    if checked == 0 {
        eprintln!("skipping: no local fixtures available for the render-gap census");
    }
}

#[test]
fn every_refused_graphic_record_is_named_in_a_warning() {
    // The structured list and the prose must not drift apart: a consumer
    // reading either one has to see the same missing content.
    let mut checked = 0usize;

    for (fixture, _, _) in EXPECTED {
        let path = format!("test-file/{fixture}");
        if !std::path::Path::new(&path).exists() {
            continue;
        }
        let parsed = PidParser::new()
            .parse_file(&path)
            .unwrap_or_else(|err| panic!("fixture {path} should parse: {err}"));
        let geometry = build_normalized_geometry(&parsed);

        for refused in &geometry.refused_graphic_records {
            checked += 1;
            let code = format!("0x{:04X}", refused.type_code);
            assert!(
                geometry.warnings.iter().any(|warning| {
                    warning.contains(&code)
                        && warning.contains(&refused.stream_path)
                        && warning.contains(&format!("{} record(s)", refused.count))
                        && warning.contains("refuses")
                }),
                "{fixture}: {} refused {code} record(s) in {} are not named in any warning: {:?}",
                refused.count,
                refused.stream_path,
                geometry.warnings
            );
        }
    }

    assert!(
        checked > 0,
        "no refused records were checked; the corpus should still contain them"
    );
}

#[test]
fn a_refusal_and_a_missing_decoder_do_not_read_alike() {
    // DWG-0202's /Sheet6615 carries one of each, so the two wordings meet in
    // one file. A reader who cannot tell them apart cannot tell "write a
    // decoder" from "re-measure the one you have".
    let path = "test-file/DWG-0202GP06-01.pid";
    if !std::path::Path::new(path).exists() {
        eprintln!("skipping: fixture {path} not found");
        return;
    }
    let parsed = PidParser::new()
        .parse_file(path)
        .expect("fixture should parse");
    let geometry = build_normalized_geometry(&parsed);

    let refusals: Vec<_> = geometry
        .warnings
        .iter()
        .filter(|warning| warning.contains("refuses"))
        .collect();
    let missing: Vec<_> = geometry
        .warnings
        .iter()
        .filter(|warning| warning.contains("have no decoder"))
        .collect();

    assert!(!refusals.is_empty(), "the refused lines and text are named");
    assert!(!missing.is_empty(), "the undecodable rectangle is named");
    for warning in &refusals {
        assert!(
            !warning.contains("have no decoder"),
            "a refusal must not read as a missing decoder: {warning}"
        );
    }
}
