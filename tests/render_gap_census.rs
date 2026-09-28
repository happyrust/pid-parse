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
    // Clean since 2026-09-24. The one refusal left here was a 22-member
    // DependencyObject its decoder's `group_kind_word <= 16` bound refused;
    // the word is the member count -- accepted records run 36 + 8k bytes and
    // that one is 36 + 8 x 22 -- so the bound became "room for k members" and
    // it decodes. No text is refused anywhere in the corpus either: reading
    // the native `igTextBox` sub-type layout decoded all 260 records of that
    // family, so every remaining refusal below is a zero-length polyline. See
    // `docs/analysis/2026-09-24-the-last-five-refusals.md`.
    ("DWG-0201GP06-01.pid", 0, 0),
    // 4 refused linestrings on /Sheet6: two coincident vertices at scope 3,
    // all on the switched-off HiddenObjects layer -- the gongyi drawing's
    // population C again, a correct refusal (same analysis). /Sheet6615's
    // rectangle was the one dropped record until `igRectangle2d` got its
    // decoder (2026-09-07); it decodes now, and emits nothing, being the
    // parent of the four edge lines that already do.
    ("DWG-0202GP06-01.pid", 4, 0),
    // 8 refused linestrings — population C (degenerate two-vertex), judged
    // a correct refusal.
    ("工艺管道及仪表流程-1.pid", 8, 0),
    // The one clean sheet in the corpus.
    ("D06.pid", 0, 0),
    // Clean since the sub-type layout landed: the 18 text records that used
    // to sit here were all sub-type 1 or 3, refused only because the decoder
    // looked for their count at sub-type 2's offset. The dropped record is
    // /JSite204/Sheet6's one 0x007B Group implementation; the two A2 border
    // rectangles that sat beside it decode since 2026-09-07.
    ("export-test/publish-data/A01/A01.pid", 0, 1),
];

/// `(fixture, graphic records the native reader skips)`, as `(type code,
/// count)` per stream in stream order — the third kind, beside the refused
/// and the undecoded, and disjoint from both.
///
/// A record whose type word carries `0x8000` is one `PSMSerializeIn` seeks
/// past before reading its oid. On this corpus every such record is an
/// earlier copy of a live record of the same oid at the same insertion —
/// the file retired it and kept the bytes — and until 2026-09-28 every
/// family decoded them and the projection drew them: 13 symbols on the
/// gongyi drawing two to four times over, an old pipe-run label under its
/// replacement (`examples/probe_run_conflicts.rs`, OCS plan
/// `2026-09-28-pid-import-next-steps.md` P-D12). They are not gaps and warn
/// nobody; they are pinned so a change in the skip rule cannot pass quietly.
type SkippedPerStream = &'static [(&'static str, u16, usize)];
const EXPECTED_SKIPPED: &[(&str, SkippedPerStream)] = &[
    // Five copies of the page frame, oid 947; the frame emitter already
    // deduplicated them by extent, so the page did not change.
    ("DWG-0201GP06-01.pid", &[("/Sheet6", 0x003D, 5)]),
    // The orphan storage's rectangle and its four edges -- the whole storage
    // is retired.
    (
        "DWG-0202GP06-01.pid",
        &[("/Sheet6615", 0x0018, 4), ("/Sheet6615", 0x0020, 1)],
    ),
    // 27 placement copies over 13 live oids (one of them three copies), and
    // label oid 6345's earlier text `250-LNG-57602- - `.
    (
        "工艺管道及仪表流程-1.pid",
        &[("/Sheet6", 0x004D, 1), ("/Sheet6", 0x00CE, 27)],
    ),
    ("D06.pid", &[]),
    (
        "export-test/publish-data/A01/A01.pid",
        &[("/JSite204/Sheet6", 0x0018, 4)],
    ),
];

#[test]
fn the_records_the_native_reader_skips_are_counted_apart() {
    let mut checked = 0usize;
    for (fixture, expected) in EXPECTED_SKIPPED {
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

        let skipped: Vec<(&str, u16, usize)> = geometry
            .skipped_graphic_records
            .iter()
            .map(|entry| (entry.stream_path.as_str(), entry.type_code, entry.count))
            .collect();
        assert_eq!(
            skipped, *expected,
            "{fixture}: graphic records carrying the native skip bit, per stream"
        );
        // Skipped is not refused and not dropped: no warning names them.
        for entry in &geometry.skipped_graphic_records {
            let code = format!("0x{:04X}", entry.type_code);
            assert!(
                !geometry.warnings.iter().any(|warning| {
                    warning.contains(&code)
                        && warning.contains(&entry.stream_path)
                        && warning.contains(&format!("{} record(s)", entry.count))
                }),
                "{fixture}: the {} skipped {code} record(s) in {} must not be warned about",
                entry.count,
                entry.stream_path
            );
        }
        // And no decoded record of any family carries the bit.
        for sheet in &parsed.sheet_streams {
            let Some(sheet_geometry) = sheet.geometry.as_ref() else {
                continue;
            };
            let flagged = sheet_geometry
                .decoded_igsymbols
                .iter()
                .map(|r| r.type_flags)
                .chain(sheet_geometry.decoded_iglines.iter().map(|r| r.type_flags))
                .chain(
                    sheet_geometry
                        .decoded_igtextboxes
                        .iter()
                        .map(|r| r.type_flags),
                )
                .chain(
                    sheet_geometry
                        .decoded_igsmartframes
                        .iter()
                        .map(|r| r.type_flags),
                )
                .filter(|flags| flags & 0b10 != 0)
                .count();
            assert_eq!(
                flagged, 0,
                "{fixture} {}: a decoded record carries the skip bit",
                sheet.path
            );
        }
    }
    if checked == 0 {
        eprintln!("skipping: no local fixtures available for the skipped-record census");
    }
}

/// `(fixture, decoded `igLine2d` records)` — the other side of the same
/// measurement. Retiring `aux_hi == 12` moved 88 records from the first table
/// to this one; a rule creeping back would show up here as a shortfall.
const EXPECTED_LINES: &[(&str, usize)] = &[
    ("DWG-0201GP06-01.pid", 24),
    // 46 until the type word's 0x8000 bit was honoured: `/Sheet6615`'s four
    // lines carry it, and the native reader seeks past such a record before
    // reading its oid (`parse_live_psm_header`); they are counted apart, in
    // `the_records_the_native_reader_skips_are_counted_apart`.
    ("DWG-0202GP06-01.pid", 42),
    ("工艺管道及仪表流程-1.pid", 218),
    ("D06.pid", 0),
    // 80 until the same bit: four of `/JSite204/Sheet6`'s lines carry it.
    ("export-test/publish-data/A01/A01.pid", 76),
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
    // DWG-0202 used to carry one of each — refused text on /Sheet6, an
    // undecodable rectangle on /Sheet6615 — so the two wordings met in one
    // file. The rectangle has a decoder now (it is the parent of four edge
    // lines and emits nothing), so on this fixture only refusals remain, and
    // the fixture's missing-decoder list must be empty: a reader who cannot
    // tell the two apart cannot tell "write a decoder" from "re-measure the
    // one you have", and the synthetic pair in `geometry.rs` keeps the two
    // wordings apart where both still occur.
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
    assert!(
        missing.is_empty() && geometry.dropped_graphic_records.is_empty(),
        "every graphic record of DWG-0202 has a decoder now; the /Sheet6615 rectangle was the \
         last without one: {missing:?}"
    );
    for warning in &refusals {
        assert!(
            !warning.contains("have no decoder"),
            "a refusal must not read as a missing decoder: {warning}"
        );
    }
}
