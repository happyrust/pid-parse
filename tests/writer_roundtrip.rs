//! In-memory CFB fixture tests for the writer layer.
//!
//! These tests build a tiny synthetic CFB on disk (via `tempfile`-free
//! `std::env::temp_dir`), parse it as a `PidPackage`, then re-emit through
//! `PidWriter` and assert the per-stream byte view round-trips. They run on
//! CI without needing any real `.pid` sample.

use pid_parse::package::PidPackage;
use pid_parse::writer::{
    MetadataUpdates, PidWriter, SheetChunkPatch, SheetPatch, StreamReplacement, WritePlan,
};
use pid_parse::PidParser;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

const DRAWING_PATH: &str = "/TaggedTxtData/Drawing";
const GENERAL_PATH: &str = "/TaggedTxtData/General";
const SHEET_PATH: &str = "/PlainSheet/Sheet1";
const UNKNOWN_PATH: &str = "/UnknownStorage/Blob";

static FIXTURE_COUNTER: AtomicU32 = AtomicU32::new(0);

fn unique_temp(name: &str) -> PathBuf {
    let n = FIXTURE_COUNTER.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("pid-parse-writer-{}-{}-{}.pid", pid, n, name))
}

fn drawing_xml() -> &'static [u8] {
    // Minimal but well-formed XML. The parser scans for SP_ tokens; if
    // none are present it just leaves drawing_meta empty, which is fine —
    // we only care about byte fidelity here.
    b"<?xml version=\"1.0\"?><Drawing><Tag SP_DRAWINGNUMBER=\"FX-001\"/></Drawing>"
}

fn general_xml() -> &'static [u8] {
    b"<?xml version=\"1.0\"?><General><FilePath>C:/fixture.pid</FilePath></General>"
}

/// Build a synthetic `.pid` file at `path` containing:
/// - `/TaggedTxtData/Drawing` (small XML)
/// - `/TaggedTxtData/General` (small XML)
/// - `/PlainSheet/Sheet1` (16 bytes 0..16)
/// - `/UnknownStorage/Blob` (32 random-but-deterministic bytes)
fn build_fixture_cfb(path: &PathBuf) {
    if path.exists() {
        std::fs::remove_file(path).expect("clean fixture path");
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("ensure tmp parent");
    }
    let mut cfb = ::cfb::create(path).expect("create fixture cfb");
    cfb.create_storage("/TaggedTxtData").unwrap();
    cfb.create_storage("/PlainSheet").unwrap();
    cfb.create_storage("/UnknownStorage").unwrap();

    let mut s = cfb.create_stream(DRAWING_PATH).unwrap();
    s.write_all(drawing_xml()).unwrap();
    drop(s);

    let mut s = cfb.create_stream(GENERAL_PATH).unwrap();
    s.write_all(general_xml()).unwrap();
    drop(s);

    let sheet_bytes: Vec<u8> = (0u8..16).collect();
    let mut s = cfb.create_stream(SHEET_PATH).unwrap();
    s.write_all(&sheet_bytes).unwrap();
    drop(s);

    let blob: Vec<u8> = (0u8..32).map(|i| i.wrapping_mul(7).wrapping_add(3)).collect();
    let mut s = cfb.create_stream(UNKNOWN_PATH).unwrap();
    s.write_all(&blob).unwrap();
    drop(s);

    cfb.flush().unwrap();
}

fn parse_pkg(path: &PathBuf) -> PidPackage {
    PidParser::new()
        .parse_package(path)
        .expect("parse_package on fixture")
}

#[test]
fn passthrough_roundtrip_preserves_streams() {
    let src = unique_temp("source");
    let dst = unique_temp("dest");
    build_fixture_cfb(&src);

    let pkg = parse_pkg(&src);
    PidWriter::write_to(&pkg, &WritePlan::default(), &dst).expect("write passthrough");

    let pkg2 = parse_pkg(&dst);

    let keys1: Vec<&String> = pkg.streams.keys().collect();
    let keys2: Vec<&String> = pkg2.streams.keys().collect();
    assert_eq!(keys1, keys2, "stream key set should match exactly");
    for key in keys1 {
        assert_eq!(
            pkg.streams[key].data, pkg2.streams[key].data,
            "stream {} bytes diverged after passthrough",
            key
        );
    }
}

#[test]
fn metadata_only_update_replaces_tagged_streams() {
    let src = unique_temp("md-src");
    let dst = unique_temp("md-dst");
    build_fixture_cfb(&src);

    let pkg = parse_pkg(&src);
    let new_drawing = "<?xml version=\"1.0\"?><Drawing>NEW</Drawing>".to_string();
    let plan = WritePlan {
        metadata_updates: MetadataUpdates {
            drawing_xml: Some(new_drawing.clone()),
            general_xml: None,
            ..Default::default()
        },
        ..Default::default()
    };
    PidWriter::write_to(&pkg, &plan, &dst).expect("write metadata update");

    let pkg2 = parse_pkg(&dst);
    assert_eq!(
        pkg2.streams[DRAWING_PATH].data,
        new_drawing.into_bytes(),
        "Drawing XML should reflect the replacement"
    );
    // Other streams stay byte-identical with the source.
    for path in [GENERAL_PATH, SHEET_PATH, UNKNOWN_PATH] {
        assert_eq!(
            pkg.streams[path].data, pkg2.streams[path].data,
            "untouched stream {} should round-trip verbatim",
            path
        );
    }
}

#[test]
fn stream_preservation_of_unknown_streams() {
    let src = unique_temp("unk-src");
    let dst = unique_temp("unk-dst");
    build_fixture_cfb(&src);

    let pkg = parse_pkg(&src);
    let plan = WritePlan {
        metadata_updates: MetadataUpdates {
            general_xml: Some("<?xml version=\"1.0\"?><G/>".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    PidWriter::write_to(&pkg, &plan, &dst).expect("write");
    let pkg2 = parse_pkg(&dst);
    assert_eq!(
        pkg.streams[UNKNOWN_PATH].data, pkg2.streams[UNKNOWN_PATH].data,
        "unknown blob must survive round-trip even with a metadata edit"
    );
}

#[test]
fn explicit_stream_replacement_overrides_metadata_layer() {
    let src = unique_temp("over-src");
    let dst = unique_temp("over-dst");
    build_fixture_cfb(&src);

    let pkg = parse_pkg(&src);
    let plan = WritePlan {
        metadata_updates: MetadataUpdates {
            drawing_xml: Some("<METADATA/>".into()),
            ..Default::default()
        },
        stream_replacements: vec![StreamReplacement {
            path: DRAWING_PATH.into(),
            new_data: b"<EXPLICIT/>".to_vec(),
        }],
        ..Default::default()
    };
    PidWriter::write_to(&pkg, &plan, &dst).expect("write");
    let pkg2 = parse_pkg(&dst);
    assert_eq!(
        pkg2.streams[DRAWING_PATH].data,
        b"<EXPLICIT/>",
        "explicit stream_replacements must run after metadata layer"
    );
}

#[test]
fn sheet_patch_byte_range() {
    let src = unique_temp("patch-src");
    let dst = unique_temp("patch-dst");
    build_fixture_cfb(&src);

    let pkg = parse_pkg(&src);
    let plan = WritePlan {
        sheet_patches: vec![SheetPatch {
            sheet_path: SHEET_PATH.into(),
            chunk_patches: vec![SheetChunkPatch {
                start: 4,
                end: 8,
                replacement: vec![0xAA; 4],
            }],
            experimental: true,
        }],
        ..Default::default()
    };
    PidWriter::write_to(&pkg, &plan, &dst).expect("write sheet patch");
    let pkg2 = parse_pkg(&dst);
    let bytes = &pkg2.streams[SHEET_PATH].data;
    assert_eq!(bytes.len(), 16, "sheet length should stay at 16");
    assert_eq!(&bytes[0..4], &[0u8, 1, 2, 3]);
    assert_eq!(&bytes[4..8], &[0xAA, 0xAA, 0xAA, 0xAA]);
    assert_eq!(&bytes[8..16], &[8u8, 9, 10, 11, 12, 13, 14, 15]);
}

#[test]
fn sheet_patch_out_of_range_errors() {
    let src = unique_temp("oob-src");
    let dst = unique_temp("oob-dst");
    build_fixture_cfb(&src);

    let pkg = parse_pkg(&src);
    let plan = WritePlan {
        sheet_patches: vec![SheetPatch {
            sheet_path: SHEET_PATH.into(),
            chunk_patches: vec![SheetChunkPatch {
                start: 4,
                end: 9999,
                replacement: vec![],
            }],
            experimental: true,
        }],
        ..Default::default()
    };
    let err = PidWriter::write_to(&pkg, &plan, &dst).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("sheet_patch") && msg.contains(SHEET_PATH),
        "expected sheet_patch context + path in error, got: {}",
        msg
    );
}

#[test]
fn missing_sheet_yields_missing_stream_error() {
    let src = unique_temp("ms-src");
    let dst = unique_temp("ms-dst");
    build_fixture_cfb(&src);

    let pkg = parse_pkg(&src);
    let plan = WritePlan {
        sheet_patches: vec![SheetPatch {
            sheet_path: "/Nope/NotThere".into(),
            chunk_patches: vec![],
            experimental: true,
        }],
        ..Default::default()
    };
    let err = PidWriter::write_to(&pkg, &plan, &dst).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("missing stream"),
        "expected MissingStream, got: {}",
        msg
    );
}
