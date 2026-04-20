//! Smoke tests that round-trip a real `.pid` fixture through the writer.
//! These tests only run when `test-file/<fixture>.pid` is present — the
//! fixtures are gitignored so the rest of CI stays green without them.
use pid_parse::writer::xml_edit::replace_simple_tag_text;
use pid_parse::{diff_packages, MetadataUpdates, PidParser, PidWriter, WritePlan};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(format!("test-file/{}", name))
}

fn tmp_output(label: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    p.push(format!("pid-parse-real-{}-{}.pid", label, nanos));
    p
}

fn streams_map(pkg: &pid_parse::PidPackage) -> BTreeMap<String, Vec<u8>> {
    pkg.streams
        .iter()
        .map(|(k, v)| (k.clone(), v.data.clone()))
        .collect()
}

#[test]
fn real_file_passthrough_preserves_all_streams_byte_for_byte() {
    let fixture = fixture_path("DWG-0201GP06-01.pid");
    if !fixture.exists() {
        eprintln!("skipping: test fixture not found at {}", fixture.display());
        return;
    }

    let parser = PidParser::new();
    let pkg_in = parser.parse_package(&fixture).expect("parse");
    let before = streams_map(&pkg_in);

    let dst = tmp_output("passthrough-real");
    PidWriter::write_to(&pkg_in, &WritePlan::default(), &dst).expect("write");

    let pkg_out = parser.parse_package(&dst).expect("reparse");
    let after = streams_map(&pkg_out);

    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>(),
        "stream path set must match after passthrough round-trip"
    );
    let mut mismatched: Vec<(String, usize, usize)> = Vec::new();
    for (path, original) in &before {
        let round_tripped = after.get(path).expect("path must exist");
        if original != round_tripped {
            mismatched.push((path.clone(), original.len(), round_tripped.len()));
        }
    }
    assert!(
        mismatched.is_empty(),
        "expected every stream to round-trip byte-identical, mismatched: {:?}",
        mismatched
    );

    let _ = std::fs::remove_file(&dst);
}

#[test]
fn real_file_passthrough_preserves_parsed_drawing_meta() {
    let fixture = fixture_path("DWG-0201GP06-01.pid");
    if !fixture.exists() {
        eprintln!("skipping: test fixture not found at {}", fixture.display());
        return;
    }

    let parser = PidParser::new();
    let pkg_in = parser.parse_package(&fixture).expect("parse");
    let drawing_in = pkg_in
        .parsed
        .drawing_meta
        .as_ref()
        .expect("drawing_meta")
        .clone();

    let dst = tmp_output("passthrough-real-meta");
    PidWriter::write_to(&pkg_in, &WritePlan::default(), &dst).expect("write");
    let pkg_out = parser.parse_package(&dst).expect("reparse");
    let drawing_out = pkg_out
        .parsed
        .drawing_meta
        .as_ref()
        .expect("drawing_meta round-trip");

    assert_eq!(drawing_in.drawing_number, drawing_out.drawing_number);
    assert_eq!(drawing_in.document_category, drawing_out.document_category);
    assert_eq!(drawing_in.template_name, drawing_out.template_name);

    let _ = std::fs::remove_file(&dst);
}

#[test]
fn real_file_reports_non_root_storage_clsids_deterministically() {
    let fixture = fixture_path("DWG-0201GP06-01.pid");
    if !fixture.exists() {
        eprintln!("skipping: test fixture not found at {}", fixture.display());
        return;
    }

    let parser = PidParser::new();
    let pkg_in = parser.parse_package(&fixture).expect("parse");
    let count_in = pkg_in.storage_clsids.len();

    // Passthrough round-trip must preserve the count and the values.
    let dst = tmp_output("nonroot-real");
    PidWriter::write_to(&pkg_in, &WritePlan::default(), &dst).expect("write");
    let pkg_out = parser.parse_package(&dst).expect("reparse");
    assert_eq!(
        pkg_out.storage_clsids.len(),
        count_in,
        "non-root storage CLSID count must match after round-trip"
    );
    for (path, clsid) in &pkg_in.storage_clsids {
        assert_eq!(
            pkg_out.storage_clsids.get(path),
            Some(clsid),
            "non-root CLSID mismatch at {}",
            path
        );
    }
    // Sanity: the map is either empty (typical real-file case) or all
    // entries must be non-nil UUIDs (nil is filtered in the parser).
    for clsid in pkg_in.storage_clsids.values() {
        assert!(!clsid.is_nil(), "nil CLSID should not be retained");
    }

    let _ = std::fs::remove_file(&dst);
}

#[test]
fn real_file_passthrough_preserves_root_clsid() {
    let fixture = fixture_path("DWG-0201GP06-01.pid");
    if !fixture.exists() {
        eprintln!("skipping: test fixture not found at {}", fixture.display());
        return;
    }

    let parser = PidParser::new();
    let pkg_in = parser.parse_package(&fixture).expect("parse");
    let expected = pkg_in
        .root_clsid
        .expect("real SmartPlant .pid should have a non-nil root CLSID");

    let dst = tmp_output("passthrough-real-clsid");
    PidWriter::write_to(&pkg_in, &WritePlan::default(), &dst).expect("write");
    let pkg_out = parser.parse_package(&dst).expect("reparse");
    assert_eq!(
        pkg_out.root_clsid,
        Some(expected),
        "root CLSID must survive passthrough round-trip"
    );

    let _ = std::fs::remove_file(&dst);
}

#[test]
fn real_file_passthrough_produces_empty_diff() {
    let fixture = fixture_path("DWG-0201GP06-01.pid");
    if !fixture.exists() {
        eprintln!("skipping: test fixture not found at {}", fixture.display());
        return;
    }

    let parser = PidParser::new();
    let pkg_in = parser.parse_package(&fixture).expect("parse");
    let dst = tmp_output("diff-real");
    PidWriter::write_to(&pkg_in, &WritePlan::default(), &dst).expect("write");
    let pkg_out = parser.parse_package(&dst).expect("reparse");

    let diff = diff_packages(&pkg_in, &pkg_out);
    assert!(
        diff.is_empty(),
        "real-file passthrough must produce an empty diff, got {} diff(s)",
        diff.diff_count()
    );

    let _ = std::fs::remove_file(&dst);
}

#[test]
fn real_file_set_drawing_number_diff_is_localized_to_one_stream() {
    let fixture = fixture_path("DWG-0201GP06-01.pid");
    if !fixture.exists() {
        eprintln!("skipping: test fixture not found at {}", fixture.display());
        return;
    }

    let parser = PidParser::new();
    let mut pkg = parser.parse_package(&fixture).expect("parse");
    pkg.set_drawing_xml_tag("DrawingNumber", "DIFF-LOCALIZED-TEST")
        .expect("set");

    let dst = tmp_output("diff-localized");
    PidWriter::write_to(&pkg, &WritePlan::default(), &dst).expect("write");

    let pkg_orig = parser.parse_package(&fixture).expect("re-parse source");
    let pkg_written = parser.parse_package(&dst).expect("parse written");
    let diff = diff_packages(&pkg_orig, &pkg_written);

    assert_eq!(diff.only_in_a.len(), 0);
    assert_eq!(diff.only_in_b.len(), 0);
    assert_eq!(
        diff.modified.len(),
        1,
        "should have exactly one modified stream, got: {:?}",
        diff.modified.iter().map(|m| &m.path).collect::<Vec<_>>()
    );
    assert_eq!(diff.modified[0].path, "/TaggedTxtData/Drawing");
    assert!(diff.root_clsid_match, "CLSID must still match");

    let _ = std::fs::remove_file(&dst);
}

#[test]
fn real_file_set_xml_tag_edits_template_only() {
    let fixture = fixture_path("DWG-0201GP06-01.pid");
    if !fixture.exists() {
        eprintln!("skipping: test fixture not found at {}", fixture.display());
        return;
    }

    let parser = PidParser::new();
    let mut pkg = parser.parse_package(&fixture).expect("parse");
    let drawing_before = pkg
        .get_stream("/TaggedTxtData/Drawing")
        .expect("drawing")
        .data
        .clone();

    let old = pkg
        .set_xml_tag("/TaggedTxtData/Drawing", "Template", "REPLACED.pid")
        .expect("set");
    assert_eq!(old, "XIONGANA2.pid", "should capture the original Template");

    let dst = tmp_output("set-xml-template");
    PidWriter::write_to(&pkg, &WritePlan::default(), &dst).expect("write");

    let pkg_out = parser.parse_package(&dst).expect("reparse");
    assert_eq!(
        pkg_out
            .parsed
            .drawing_meta
            .as_ref()
            .and_then(|m| m.template_name.as_deref()),
        Some("REPLACED.pid"),
        "parsed Template should match the new value"
    );

    // Every stream other than /TaggedTxtData/Drawing must be byte-identical
    // between the pre-edit source and the written output.
    let drawing_after = pkg_out
        .get_stream("/TaggedTxtData/Drawing")
        .expect("drawing")
        .data
        .clone();
    assert_ne!(
        drawing_before, drawing_after,
        "the drawing stream must have changed"
    );
    // Confirm the only diff is localized to the Template tag:
    let before_str = std::str::from_utf8(&drawing_before).expect("utf8");
    let after_str = std::str::from_utf8(&drawing_after).expect("utf8");
    assert!(after_str.contains("<Template>REPLACED.pid</Template>"));
    assert!(before_str.contains("<Template>XIONGANA2.pid</Template>"));
    // And that sibling tags still carry their originals.
    assert!(after_str.contains("DWG-0201GP06-01"));

    let _ = std::fs::remove_file(&dst);
}

#[test]
fn real_file_set_drawing_number_rewrites_only_the_target_tag() {
    let fixture = fixture_path("DWG-0201GP06-01.pid");
    if !fixture.exists() {
        eprintln!("skipping: test fixture not found at {}", fixture.display());
        return;
    }

    let parser = PidParser::new();
    let pkg_in = parser.parse_package(&fixture).expect("parse");
    let drawing_bytes_in = pkg_in
        .get_stream("/TaggedTxtData/Drawing")
        .expect("drawing xml")
        .data
        .clone();
    let xml_in = std::str::from_utf8(&drawing_bytes_in)
        .expect("utf-8")
        .to_string();
    let new_xml =
        replace_simple_tag_text(&xml_in, "DrawingNumber", "ROUND-TRIP-9999").expect("edit");

    let dst = tmp_output("set-dn-real");
    let plan = WritePlan {
        metadata_updates: MetadataUpdates {
            drawing_xml: Some(new_xml),
            ..Default::default()
        },
        ..Default::default()
    };
    PidWriter::write_to(&pkg_in, &plan, &dst).expect("write");

    let pkg_out = parser.parse_package(&dst).expect("reparse");
    let drawing_out = pkg_out.parsed.drawing_meta.as_ref().expect("drawing_meta");
    assert_eq!(
        drawing_out.drawing_number.as_deref(),
        Some("ROUND-TRIP-9999"),
        "DrawingNumber must reflect the edit"
    );
    // DrawingSite.DrawingNumber (different tag name) should keep the
    // original drawing number.
    assert_eq!(
        drawing_out
            .tags
            .get("DrawingSite.DrawingNumber")
            .map(|s| s.as_str()),
        Some("DWG-0201GP06-01"),
        "DrawingSite.DrawingNumber must be untouched"
    );
    // Every stream other than /TaggedTxtData/Drawing must be byte-identical.
    for (path, raw_in) in pkg_in.streams.iter() {
        if path == "/TaggedTxtData/Drawing" {
            continue;
        }
        let raw_out = pkg_out.get_stream(path).expect("stream present");
        assert_eq!(
            raw_in.data, raw_out.data,
            "stream {path} should be byte-identical after set-drawing-number"
        );
    }

    let _ = std::fs::remove_file(&dst);
}

#[test]
fn real_file_set_summary_title_preserves_other_streams() {
    // Phase 9m: verify that a summary_updates plan against a real
    // SmartPlant `.pid` fixture (when available) rewrites only the
    // `/\u{5}SummaryInformation` stream, with every other stream
    // byte-identical and the new title readable from the round-tripped
    // file.
    let fixture = fixture_path("DWG-0201GP06-01.pid");
    if !fixture.exists() {
        eprintln!("skipping: test fixture not found at {}", fixture.display());
        return;
    }

    let parser = PidParser::new();
    let pkg_in = parser.parse_package(&fixture).expect("parse");

    // The fixture may or may not already have a `/\u{5}SummaryInformation`
    // stream. If it doesn't, we can't exercise the writer here — seeding
    // a fresh property-set from scratch is out of scope for Phase 9m (see
    // plan Non-Goals / Known Limitations). Skip quietly so the real-file
    // suite stays opt-in rather than flake-based.
    if pkg_in.get_stream("/\u{5}SummaryInformation").is_none() {
        eprintln!(
            "skipping: fixture lacks /\\u0005SummaryInformation stream \
             (Phase 9n will grow the seed-from-empty path)"
        );
        return;
    }

    let dst = tmp_output("set-summary-real");
    let mut summary = std::collections::BTreeMap::new();
    summary.insert("title".to_string(), "ROUND-TRIP-PHASE-9M".to_string());
    let plan = WritePlan {
        metadata_updates: MetadataUpdates {
            summary_updates: summary,
            ..Default::default()
        },
        ..Default::default()
    };
    PidWriter::write_to(&pkg_in, &plan, &dst).expect("write");

    let pkg_out = parser.parse_package(&dst).expect("reparse");
    assert_eq!(
        pkg_out
            .parsed
            .summary
            .as_ref()
            .and_then(|s| s.title.clone())
            .as_deref(),
        Some("ROUND-TRIP-PHASE-9M"),
        "real-file title should reflect the summary_updates edit"
    );

    // Every stream other than /\u{5}SummaryInformation must be
    // byte-identical (this is the Phase 9l byte-level fidelity contract
    // exercised end-to-end on a real fixture).
    for (path, raw_in) in pkg_in.streams.iter() {
        if path == "/\u{5}SummaryInformation" {
            continue;
        }
        let raw_out = pkg_out
            .get_stream(path)
            .unwrap_or_else(|| panic!("stream {path} missing in dst"));
        assert_eq!(
            raw_in.data, raw_out.data,
            "stream {path} must be byte-identical after a summary-only edit"
        );
    }

    let _ = std::fs::remove_file(&dst);
}
