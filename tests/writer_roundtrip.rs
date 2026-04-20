//! Integration tests for `pid_parse::writer`.
//!
//! These tests build a minimal CFB container in a temp file (no external
//! fixtures required), round-trip it through [`PidWriter`], and verify
//! stream-level byte preservation plus metadata / sheet-patch semantics.
use pid_parse::writer::{SheetChunkPatch, SheetPatch};
use pid_parse::{MetadataUpdates, PidParser, PidWriter, Uuid, WritePlan};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

/// Minimal but structurally valid XML bodies that pass the parser's
/// `parse_tagged_text_streams` step without surprises. The parser ignores
/// unknown tags, so any well-formed XML works.
const DRAWING_XML: &str = "<Drawing><DrawingNumber>DEMO-0001</DrawingNumber></Drawing>";
const GENERAL_XML: &str = "<General><FilePath>C:\\demo\\demo.pid</FilePath></General>";

/// Generate a unique temp-file path so tests can run in parallel without
/// stomping each other.
fn unique_tmp(label: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    p.push(format!("pid-parse-it-{}-{}.pid", label, nanos));
    p
}

/// Build a fixture CFB with a handful of streams covering the cases we
/// care about: a metadata XML stream, a nested "unknown" storage, and a
/// sheet-like binary stream.
fn build_fixture_cfb(path: &std::path::Path) {
    let file = std::fs::File::create(path).expect("create fixture");
    let mut cfb = ::cfb::CompoundFile::create(file).expect("cfb create");

    cfb.create_storage_all("/TaggedTxtData").expect("storage");
    cfb.create_storage_all("/UnknownStorage").expect("storage");
    cfb.create_storage_all("/PlainSheet").expect("storage");

    let mut s = cfb.create_stream("/TaggedTxtData/Drawing").expect("stream");
    s.write_all(DRAWING_XML.as_bytes()).expect("write");
    drop(s);

    let mut s = cfb.create_stream("/TaggedTxtData/General").expect("stream");
    s.write_all(GENERAL_XML.as_bytes()).expect("write");
    drop(s);

    let mut s = cfb.create_stream("/UnknownStorage/Blob").expect("stream");
    s.write_all(&(0u8..=31).collect::<Vec<_>>()).expect("write");
    drop(s);

    // A 16-byte sheet-like stream used by the sheet_patch tests.
    let mut s = cfb.create_stream("/PlainSheet/Sheet1").expect("stream");
    s.write_all(&(0u8..16).collect::<Vec<_>>()).expect("write");
    drop(s);

    cfb.flush().expect("flush");
}

fn streams_map(pkg: &pid_parse::PidPackage) -> BTreeMap<String, Vec<u8>> {
    pkg.streams
        .iter()
        .map(|(k, v)| (k.clone(), v.data.clone()))
        .collect()
}

#[test]
fn passthrough_roundtrip_preserves_every_stream() {
    let src = unique_tmp("passthrough-src");
    let dst = unique_tmp("passthrough-dst");
    build_fixture_cfb(&src);

    let parser = PidParser::new();
    let pkg_in = parser.parse_package(&src).expect("parse");
    let before = streams_map(&pkg_in);

    PidWriter::write_to(&pkg_in, &WritePlan::default(), &dst).expect("write");

    let pkg_out = parser.parse_package(&dst).expect("reparse");
    let after = streams_map(&pkg_out);
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>(),
        "stream path set must be preserved"
    );
    for (p, data) in &before {
        assert_eq!(
            data,
            after.get(p).expect("path must exist after round-trip"),
            "bytes differ for stream {}",
            p
        );
    }

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn metadata_only_update_replaces_tagged_streams_and_keeps_others() {
    let src = unique_tmp("metadata-src");
    let dst = unique_tmp("metadata-dst");
    build_fixture_cfb(&src);

    let parser = PidParser::new();
    let pkg_in = parser.parse_package(&src).expect("parse");
    let before = streams_map(&pkg_in);

    let new_drawing = "<Drawing><DrawingNumber>NEW-9999</DrawingNumber></Drawing>";
    let plan = WritePlan {
        metadata_updates: MetadataUpdates {
            drawing_xml: Some(new_drawing.to_string()),
            ..Default::default()
        },
        ..WritePlan::default()
    };
    PidWriter::write_to(&pkg_in, &plan, &dst).expect("write");

    let pkg_out = parser.parse_package(&dst).expect("reparse");
    let after = streams_map(&pkg_out);

    assert_eq!(
        after.get("/TaggedTxtData/Drawing").expect("stream"),
        new_drawing.as_bytes(),
        "drawing xml should be the new content"
    );
    // Every other stream must keep its original bytes.
    for (p, original) in &before {
        if p == "/TaggedTxtData/Drawing" {
            continue;
        }
        assert_eq!(
            after.get(p).expect("path must still exist"),
            original,
            "stream {} should be untouched",
            p
        );
    }

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn unknown_streams_are_preserved_through_passthrough_with_metadata() {
    let src = unique_tmp("unknown-src");
    let dst = unique_tmp("unknown-dst");
    build_fixture_cfb(&src);

    let parser = PidParser::new();
    let pkg_in = parser.parse_package(&src).expect("parse");
    let blob_before = pkg_in
        .get_stream("/UnknownStorage/Blob")
        .expect("blob")
        .data
        .clone();

    // Apply an update unrelated to the unknown blob — it must still survive.
    let plan = WritePlan::metadata_only(Some(DRAWING_XML.to_string()), None);
    PidWriter::write_to(&pkg_in, &plan, &dst).expect("write");

    let pkg_out = parser.parse_package(&dst).expect("reparse");
    let blob_after = pkg_out
        .get_stream("/UnknownStorage/Blob")
        .expect("blob")
        .data
        .clone();
    assert_eq!(
        blob_before, blob_after,
        "unknown blob must be byte-identical"
    );

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn sheet_patch_replaces_byte_range_and_preserves_length() {
    let src = unique_tmp("sheet-src");
    let dst = unique_tmp("sheet-dst");
    build_fixture_cfb(&src);

    let parser = PidParser::new();
    let pkg_in = parser.parse_package(&src).expect("parse");

    let plan = WritePlan {
        sheet_patches: vec![SheetPatch {
            sheet_path: "/PlainSheet/Sheet1".to_string(),
            chunk_patches: vec![SheetChunkPatch {
                start: 4,
                end: 8,
                replacement: vec![0xAA; 4],
            }],
            experimental: true,
        }],
        ..WritePlan::default()
    };
    PidWriter::write_to(&pkg_in, &plan, &dst).expect("write");

    let pkg_out = parser.parse_package(&dst).expect("reparse");
    let sheet = pkg_out
        .get_stream("/PlainSheet/Sheet1")
        .expect("sheet stream");
    assert_eq!(sheet.data.len(), 16, "total length should be preserved");
    assert_eq!(&sheet.data[0..4], &[0, 1, 2, 3]);
    assert_eq!(&sheet.data[4..8], &[0xAA; 4]);
    assert_eq!(&sheet.data[8..16], &(8u8..16).collect::<Vec<_>>());

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn sheet_patch_out_of_range_is_rejected() {
    let src = unique_tmp("sheet-err-src");
    let dst = unique_tmp("sheet-err-dst");
    build_fixture_cfb(&src);

    let parser = PidParser::new();
    let pkg_in = parser.parse_package(&src).expect("parse");

    let plan = WritePlan {
        sheet_patches: vec![SheetPatch {
            sheet_path: "/PlainSheet/Sheet1".to_string(),
            chunk_patches: vec![SheetChunkPatch {
                start: 10,
                end: 99,
                replacement: vec![0xFF; 4],
            }],
            experimental: true,
        }],
        ..WritePlan::default()
    };
    match PidWriter::write_to(&pkg_in, &plan, &dst) {
        Err(pid_parse::PidError::ParseFailure { context, .. }) => {
            assert!(
                context.contains("sheet_patch"),
                "context should mark the sheet_patch source: {context}"
            );
        }
        other => panic!("expected ParseFailure, got {other:?}"),
    }

    let _ = std::fs::remove_file(&src);
    // dst may or may not exist depending on when the error was raised.
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn non_root_storage_clsid_round_trips() {
    // Build a fixture, stamp a non-root storage CLSID, parse+write+reparse,
    // and assert the CLSID survives.
    let src = unique_tmp("nonroot-clsid-src");
    let dst = unique_tmp("nonroot-clsid-dst");
    build_fixture_cfb(&src);

    {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&src)
            .expect("open rw");
        let mut cfb = ::cfb::CompoundFile::open(file).expect("open cfb");
        let clsid = Uuid::parse_str("F29F85E0-4FF9-1068-AB91-08002B27B3D9").unwrap();
        cfb.set_storage_clsid("/UnknownStorage", clsid)
            .expect("set");
        cfb.flush().expect("flush");
    }

    let parser = PidParser::new();
    let pkg_in = parser.parse_package(&src).expect("parse");
    let expected = Uuid::parse_str("F29F85E0-4FF9-1068-AB91-08002B27B3D9").unwrap();
    assert_eq!(
        pkg_in.storage_clsids.get("/UnknownStorage"),
        Some(&expected),
        "source should capture the non-root CLSID"
    );

    PidWriter::write_to(&pkg_in, &WritePlan::default(), &dst).expect("write");
    let pkg_out = parser.parse_package(&dst).expect("reparse");
    assert_eq!(
        pkg_out.storage_clsids.get("/UnknownStorage"),
        Some(&expected),
        "non-root CLSID should survive round-trip"
    );

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn fixture_without_clsid_reports_none() {
    // cfb::create leaves the root CLSID as the nil UUID, which the parser
    // normalizes to `None`. Regression guard for the nil-check in reader.
    let src = unique_tmp("nil-clsid-src");
    build_fixture_cfb(&src);
    let parser = PidParser::new();
    let pkg = parser.parse_package(&src).expect("parse");
    assert!(
        pkg.root_clsid.is_none(),
        "nil UUID should be normalized to None, got {:?}",
        pkg.root_clsid
    );
    let _ = std::fs::remove_file(&src);
}

#[test]
fn root_clsid_round_trips_when_source_has_one() {
    // Build a fixture, stamp a deliberate CLSID on its root, parse+write+reparse.
    let src = unique_tmp("clsid-src");
    let dst = unique_tmp("clsid-dst");
    build_fixture_cfb(&src);

    // Stamp the root CLSID by opening the fixture a second time.
    {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&src)
            .expect("open rw");
        let mut cfb = ::cfb::CompoundFile::open(file).expect("open cfb");
        let clsid = Uuid::parse_str("00020906-0000-0000-C000-000000000046").expect("uuid");
        cfb.set_storage_clsid("/", clsid).expect("set clsid");
        cfb.flush().expect("flush");
    }

    let parser = PidParser::new();
    let pkg_in = parser.parse_package(&src).expect("parse");
    let expected = Uuid::parse_str("00020906-0000-0000-C000-000000000046").unwrap();
    assert_eq!(
        pkg_in.root_clsid,
        Some(expected),
        "source CLSID should be captured"
    );

    PidWriter::write_to(&pkg_in, &WritePlan::default(), &dst).expect("write");
    let pkg_out = parser.parse_package(&dst).expect("reparse");
    assert_eq!(
        pkg_out.root_clsid,
        Some(expected),
        "destination CLSID should be preserved"
    );

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn original_package_is_not_mutated_by_write() {
    let src = unique_tmp("mut-src");
    let dst = unique_tmp("mut-dst");
    build_fixture_cfb(&src);

    let parser = PidParser::new();
    let pkg = parser.parse_package(&src).expect("parse");
    let snapshot = streams_map(&pkg);

    let plan = WritePlan::metadata_only(
        Some("<Drawing><DrawingNumber>X</DrawingNumber></Drawing>".to_string()),
        None,
    );
    PidWriter::write_to(&pkg, &plan, &dst).expect("write");

    // The writer must have worked on a clone.
    assert_eq!(streams_map(&pkg), snapshot, "pkg must be unchanged");

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

/// Build a minimal but valid `/\u{5}SummaryInformation` property-set stream
/// with a single VT_LPSTR title property. Mirrors the fixture helper in
/// `writer::summary_write::tests` but lives here so the integration layer
/// stays self-contained.
fn minimal_summary_info_bytes(title_ascii: &str) -> Vec<u8> {
    const FMTID_SUMMARY: [u8; 16] = [
        0xE0, 0x85, 0x9F, 0xF2, 0xF9, 0x4F, 0x68, 0x10, 0xAB, 0x91, 0x08, 0x00, 0x2B, 0x27, 0xB3,
        0xD9,
    ];
    const VT_LPSTR: u16 = 0x001E;
    // title value: NUL-terminated ASCII bytes.
    let mut title_bytes: Vec<u8> = title_ascii.as_bytes().to_vec();
    title_bytes.push(0);
    // Typed-value bytes: VT tag (4) + char count (4) + bytes + 4-byte pad.
    let mut typed_value = Vec::new();
    typed_value.extend_from_slice(&(VT_LPSTR as u32).to_le_bytes());
    typed_value.extend_from_slice(&(title_bytes.len() as u32).to_le_bytes());
    typed_value.extend_from_slice(&title_bytes);
    while !typed_value.len().is_multiple_of(4) {
        typed_value.push(0);
    }

    // Section layout: 8-byte header + 1 prop id/offset entry (8 bytes) + data.
    let table_size: usize = 8 + 8;
    let prop_offset = table_size as u32;
    let section_size = (table_size + typed_value.len()) as u32;
    let mut section = Vec::new();
    section.extend_from_slice(&section_size.to_le_bytes());
    section.extend_from_slice(&1u32.to_le_bytes()); // num_props
    section.extend_from_slice(&2u32.to_le_bytes()); // PID_TITLE = 2
    section.extend_from_slice(&prop_offset.to_le_bytes());
    section.extend_from_slice(&typed_value);

    // Stream wrapper: 28-byte header + 1 section entry (20 bytes) + section.
    let mut stream = Vec::new();
    stream.extend_from_slice(&0xFFFEu16.to_le_bytes()); // byte order
    stream.extend_from_slice(&0u16.to_le_bytes()); // version
    stream.extend_from_slice(&0u32.to_le_bytes()); // system_id
    stream.extend_from_slice(&[0u8; 16]); // class_id (nil)
    stream.extend_from_slice(&1u32.to_le_bytes()); // num_sections
    stream.extend_from_slice(&FMTID_SUMMARY);
    stream.extend_from_slice(&48u32.to_le_bytes()); // section offset = 28 + 1*20
    stream.extend_from_slice(&section);
    stream
}

#[test]
fn summary_updates_rewrite_title_end_to_end_through_pid_writer() {
    // Add a /\x05SummaryInformation stream to the fixture, then round-trip
    // through PidWriter with a `summary_updates` plan and assert the new
    // title is visible in the reparsed document.
    let src = unique_tmp("summary-src");
    let dst = unique_tmp("summary-dst");
    build_fixture_cfb(&src);

    {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&src)
            .expect("open rw");
        let mut cfb = ::cfb::CompoundFile::open(file).expect("open cfb");
        let mut s = cfb
            .create_stream("/\u{5}SummaryInformation")
            .expect("create summary stream");
        s.write_all(&minimal_summary_info_bytes("Original Title"))
            .expect("write summary");
        drop(s);
        cfb.flush().expect("flush");
    }

    let parser = PidParser::new();
    let pkg = parser.parse_package(&src).expect("parse");
    assert_eq!(
        pkg.parsed.summary.as_ref().and_then(|s| s.title.clone()),
        Some("Original Title".into()),
        "reader should pick up the pre-write title",
    );

    let mut summary_updates = BTreeMap::new();
    summary_updates.insert("title".to_string(), "Rewritten by Phase 9l".to_string());
    let plan = WritePlan {
        metadata_updates: MetadataUpdates {
            summary_updates,
            ..Default::default()
        },
        ..Default::default()
    };

    PidWriter::write_to(&pkg, &plan, &dst).expect("write");

    let pkg_after = parser.parse_package(&dst).expect("reparse");
    assert_eq!(
        pkg_after
            .parsed
            .summary
            .as_ref()
            .and_then(|s| s.title.clone()),
        Some("Rewritten by Phase 9l".into()),
        "writer should have updated the property-set title",
    );

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn summary_updates_unknown_key_fails_writer_with_clear_error() {
    let src = unique_tmp("summary-bad-src");
    let dst = unique_tmp("summary-bad-dst");
    build_fixture_cfb(&src);
    {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&src)
            .expect("open rw");
        let mut cfb = ::cfb::CompoundFile::open(file).expect("open cfb");
        let mut s = cfb
            .create_stream("/\u{5}SummaryInformation")
            .expect("create summary stream");
        s.write_all(&minimal_summary_info_bytes("whatever"))
            .expect("write summary");
        drop(s);
        cfb.flush().expect("flush");
    }

    let parser = PidParser::new();
    let pkg = parser.parse_package(&src).expect("parse");

    let mut summary_updates = BTreeMap::new();
    summary_updates.insert("made_up_key".to_string(), "x".to_string());
    let plan = WritePlan {
        metadata_updates: MetadataUpdates {
            summary_updates,
            ..Default::default()
        },
        ..Default::default()
    };

    let err = PidWriter::write_to(&pkg, &plan, &dst).expect_err("must reject");
    let msg = format!("{err}");
    assert!(msg.contains("unknown key"), "got: {msg}");
    assert!(msg.contains("made_up_key"), "got: {msg}");

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}
