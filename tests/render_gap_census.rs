//! Phase 40 gate: a Sheet record that does not reach the drawing is named.
//!
//! Phase 38 S2 made that true for records no decoder claims. It was not true
//! for the other half — a record whose family is wired and whose bytes that
//! family refused — because the census behind the warning tests the *type
//! code*, and a refused record's type code is one this crate decodes. Those
//! records fell out of the decoded output and out of the census both, and on
//! this corpus they outnumbered the named drops 141 to 5.
//!
//! Naming them is what got them re-measured, twice over. 88 of the 141 were
//! `igLine2d` refused on `aux_hi != 12` — a rule this crate invented for a PSM
//! envelope field the native reader discards — and retiring it left 53. Then
//! the census stopped letting a scan-based family's over-claim shadow a later
//! record's start, which surfaced 23 more (71), and retiring `igTextBox`'s
//! fixed-68 overhead decoded 33 records that had been refusals (44). The counts
//! below are the measurement, pinned. They are expected to move when a decoder
//! learns a shape it used to refuse; they are not expected to move quietly. See
//! `docs/analysis/2026-08-10-the-silent-bucket-is-refusals-not-unknowns.md`,
//! `docs/analysis/2026-08-11-remaining-header-is-the-psm-aux-field.md`,
//! `docs/analysis/2026-08-12-census-claims-are-starts-not-covers.md`,
//! `docs/analysis/2026-08-12-igtextbox-overhead-is-a-floor-not-a-constant.md`,
//! and `examples/probe_phase40_render_gap_census`.
//!
//! Fixtures soft-skip when absent, mirroring `tests/parse_real_files.rs`.

use pid_parse::{build_normalized_geometry, PidParser};

/// `(fixture, refused graphic records, undecoded graphic records)` — both
/// counted in records, not in `(stream, type code)` groups.
const EXPECTED: &[(&str, usize, usize)] = &[
    // The one refused DependencyObject. No text is refused anywhere in the
    // corpus any more: reading the native `igTextBox` sub-type layout
    // decoded all 260 records of that family, so every remaining refusal
    // below is a polyline or the 0x00FA.
    ("DWG-0201GP06-01.pid", 1, 0),
    // 4 refused linestrings on /Sheet6; /Sheet6615's one undecodable
    // rectangle is the dropped record.
    ("DWG-0202GP06-01.pid", 4, 1),
    // 8 refused linestrings — population C (degenerate two-vertex), judged
    // a correct refusal.
    ("工艺管道及仪表流程-1.pid", 8, 0),
    // The one clean sheet in the corpus.
    ("D06.pid", 0, 0),
    // Clean since the sub-type layout landed: the 18 text records that used
    // to sit here were all sub-type 1 or 3, refused only because the decoder
    // looked for their count at sub-type 2's offset.
    ("export-test/publish-data/A01/A01.pid", 0, 3),
];

/// `(fixture, decoded `igLine2d` records)` — the other side of the same
/// measurement. Retiring `aux_hi == 12` moved 88 records from the first table
/// to this one; a rule creeping back would show up here as a shortfall.
const EXPECTED_LINES: &[(&str, usize)] = &[
    ("DWG-0201GP06-01.pid", 24),
    ("DWG-0202GP06-01.pid", 46),
    ("工艺管道及仪表流程-1.pid", 218),
    ("D06.pid", 0),
    ("export-test/publish-data/A01/A01.pid", 80),
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
fn the_corpus_line_counts_include_what_the_retired_rule_refused() {
    let mut checked = 0usize;

    for (fixture, expected_lines) in EXPECTED_LINES {
        let path = format!("test-file/{fixture}");
        if !std::path::Path::new(&path).exists() {
            eprintln!("skipping: fixture {path} not found");
            continue;
        }
        let parsed = PidParser::new()
            .parse_file(&path)
            .unwrap_or_else(|err| panic!("fixture {path} should parse: {err}"));
        checked += 1;

        let lines: Vec<_> = parsed
            .sheet_streams
            .iter()
            .filter_map(|sheet| sheet.geometry.as_ref())
            .flat_map(|geometry| geometry.decoded_iglines.iter())
            .collect();

        assert_eq!(
            lines.len(),
            *expected_lines,
            "{fixture}: igLine2d count drifted"
        );
        // Every one of them is chain-resident by construction now; the field
        // the old rule tested is carried, not enforced.
        for line in &lines {
            assert!(
                matches!(line.aux_hi, 8 | 12 | 6996),
                "{fixture}: unexpected aux_hi {} on oid {} — a new population \
                 wants its own look, not a silent pass",
                line.aux_hi,
                line.oid
            );
        }
    }

    if checked == 0 {
        eprintln!("skipping: no local fixtures available for the line census");
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
    // DWG-0202 carries one of each — refused text on /Sheet6, an undecodable
    // rectangle on /Sheet6615 — so the two wordings meet in one file. A reader
    // who cannot tell them apart cannot tell "write a decoder" from
    // "re-measure the one you have".
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
