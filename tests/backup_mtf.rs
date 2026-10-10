//! Integration tests for the MTF envelope parser.
//!
//! These tests poke at a real `SmartPlant` backup `Export.dmp` fixture
//! shipped under `test-file/backup-test/`. When the fixture is missing
//! (e.g. CI workers that do not carry large `SmartPlant` samples) the
//! tests skip cleanly — same pattern used by `parse_real_files.rs` /
//! `writer_real_files.rs`.

use pid_parse::backup::mtf::{
    detect_backup_stream_header_len, detect_logical_block_size, detect_non_mtf_dump_format,
    locate_sql_server_streams, mdf_bytes_of_dump, MtfBlockCursor, MtfBlockType, MtfError,
    MtfHeader, SqlServerDumpError, COMMON_BLOCK_HEADER_LEN,
};
use std::path::Path;
use std::process::Command;

const EXPORT_DMP: &str = "test-file/backup-test/TEST02_p/Export.dmp";
const ORACLE_EXPORT_DMP: &str = "test-file/backup-test/DWG-0202GP06-01_p/Export.dmp";

/// Read the first `bytes` bytes of a fixture, or return `None` when the
/// fixture is missing so tests can skip cleanly on CI.
fn read_head(path: &str, bytes: usize) -> Option<Vec<u8>> {
    let p = Path::new(path);
    if !p.exists() {
        eprintln!("skipping: fixture {path} not found");
        return None;
    }
    let data = std::fs::read(p).unwrap_or_else(|e| panic!("read {path}: {e}"));
    if data.len() < bytes {
        panic!("fixture {path} unexpectedly shorter than {bytes} bytes");
    }
    Some(data[..bytes].to_vec())
}

#[test]
fn real_export_dmp_starts_with_tape_descriptor() {
    let Some(head) = read_head(EXPORT_DMP, COMMON_BLOCK_HEADER_LEN) else {
        return;
    };

    let hdr = MtfHeader::probe(&head).unwrap_or_else(|e| {
        panic!(
            "Export.dmp should parse as MTF, got error {e:?}; first 16 bytes = {:02X?}",
            &head[..16]
        )
    });

    assert_eq!(
        hdr.block_type,
        MtfBlockType::Tape,
        "first MTF descriptor block must be TAPE"
    );
    assert_eq!(
        &hdr.raw_common_header[0..4],
        b"TAPE",
        "common header must echo the 'TAPE' magic"
    );
}

#[test]
fn real_export_dmp_exposes_common_header_bytes_verbatim() {
    // Smoke-test: the raw_common_header field should round-trip the
    // exact bytes we read from disk. Guards against silent trimming /
    // padding bugs in the probe helper.
    let Some(head) = read_head(EXPORT_DMP, COMMON_BLOCK_HEADER_LEN) else {
        return;
    };
    let hdr = MtfHeader::probe(&head).expect("header parses");
    assert_eq!(
        &hdr.raw_common_header[..],
        &head[..],
        "raw_common_header must mirror the on-disk bytes byte-for-byte"
    );
}

#[test]
fn real_export_dmp_logical_block_size_is_1024_for_sql_2008r2_fixture() {
    // We only have one real SmartPlant backup to exercise and its
    // SQL Server 2008 R2 vintage uses 1024-byte TAPE blocks followed
    // by an SFMB half-block. Hard-asserting that exact value catches
    // regressions in the detector's scan step or grid alignment.
    //
    // If a newer fixture comes along with 65536-byte blocks, loosen
    // this to the softer `assert!(matches!(..., 1024 | 65536))` shape
    // rather than deleting the test.
    let Some(head) = read_head(EXPORT_DMP, 4096) else {
        return;
    };
    let detected = detect_logical_block_size(&head)
        .expect("detector should locate the next descriptor within 4 KiB");
    assert_eq!(
        detected, 1024,
        "this fixture's TAPE block is expected to be 1024 bytes"
    );
}

#[test]
fn real_export_dmp_second_descriptor_is_sfmb_at_1024() {
    // Sanity cross-check for the detector: the second descriptor in
    // our SQL Server 2008 R2 fixture is a Soft Filemark Block half-
    // block at offset 1024. If that ever changes the detector test
    // above will also surface the discrepancy.
    let Some(head) = read_head(EXPORT_DMP, 2048) else {
        return;
    };
    let tag = [head[1024], head[1025], head[1026], head[1027]];
    let kind = MtfBlockType::from_bytes(tag);
    assert_eq!(
        kind,
        MtfBlockType::SoftFilemark,
        "expected SFMB at offset 1024, got tag bytes {tag:02X?}"
    );
}

#[test]
fn real_export_dmp_third_descriptor_is_sset_at_1536() {
    // After the half-block SFMB comes the Start-of-Set descriptor,
    // which kicks off the actual SQL Server backup stream.
    let Some(head) = read_head(EXPORT_DMP, 3072) else {
        return;
    };
    let tag = [head[1536], head[1537], head[1538], head[1539]];
    let kind = MtfBlockType::from_bytes(tag);
    assert_eq!(
        kind,
        MtfBlockType::StartOfSet,
        "expected SSET at offset 1536, got tag bytes {tag:02X?}"
    );
}

#[test]
fn real_export_dmp_cursor_yields_expected_prefix() {
    // Walk the first 64 KiB and confirm the cursor reports the
    // descriptor sequence we already verified with direct byte reads.
    // Beyond SSET lives the actual SQL Server backup stream (VOLB +
    // MQDA + large DBDB payload), which stage 0 treats as opaque but
    // the cursor should still iterate without panicking.
    let Some(head) = read_head(EXPORT_DMP, 65536) else {
        return;
    };
    let blocks: Vec<(MtfBlockType, usize, usize)> = MtfBlockCursor::new(&head)
        .take(4)
        .map(|b| (b.block_type, b.offset, b.size))
        .collect();

    assert!(
        blocks.len() >= 3,
        "cursor should yield at least TAPE + SFMB + SSET within 64 KiB"
    );
    assert_eq!(blocks[0].0, MtfBlockType::Tape);
    assert_eq!(blocks[0].1, 0);
    assert_eq!(blocks[0].2, 1024, "TAPE block spans offsets 0..1024");
    assert_eq!(blocks[1].0, MtfBlockType::SoftFilemark);
    assert_eq!(blocks[1].1, 1024);
    assert_eq!(blocks[1].2, 512, "SFMB is a half-block");
    assert_eq!(blocks[2].0, MtfBlockType::StartOfSet);
    assert_eq!(blocks[2].1, 1536);

    // Diagnostic dump for manual inspection when the upstream fixture
    // changes; does not fail the test if the 4th block is something
    // unexpected.
    eprintln!("first four blocks:");
    for (kind, offset, size) in &blocks {
        eprintln!(
            "  tag={} offset=0x{:06X} size={} B",
            kind.tag(),
            offset,
            size
        );
    }
}

#[test]
fn real_oracle_export_dmp_is_named_instead_of_probed_as_mtf() {
    let Some(head) = read_head(ORACLE_EXPORT_DMP, COMMON_BLOCK_HEADER_LEN) else {
        return;
    };

    assert!(matches!(
        MtfHeader::probe(&head),
        Err(MtfError::NotATapeStart { .. })
    ));
    let diag = detect_non_mtf_dump_format(&head).expect("an Oracle exp dump must be named");
    assert!(diag.contains("EXPORT:V12.01.00"), "{diag}");
}

#[test]
fn probe_and_extract_refuse_an_oracle_dump_with_the_same_message() {
    if !Path::new(ORACLE_EXPORT_DMP).exists() {
        eprintln!("skipping: fixture {ORACLE_EXPORT_DMP} not found");
        return;
    }

    let probe = Command::new(env!("CARGO_BIN_EXE_pid_backup_probe"))
        .arg(ORACLE_EXPORT_DMP)
        .output()
        .expect("run pid_backup_probe");
    let extract = Command::new(env!("CARGO_BIN_EXE_pid_backup_extract"))
        .arg(ORACLE_EXPORT_DMP)
        .arg("--out")
        .arg(std::env::temp_dir())
        .arg("--dry-run")
        .output()
        .expect("run pid_backup_extract");

    assert_eq!(probe.status.code(), Some(1));
    assert_eq!(extract.status.code(), Some(1));
    let probe_stderr = String::from_utf8_lossy(&probe.stderr);
    assert!(
        probe_stderr.contains("Oracle Database `exp` dump (EXPORT:V12.01.00)"),
        "{probe_stderr}"
    );
    assert_eq!(probe_stderr, String::from_utf8_lossy(&extract.stderr));

    // The library names the dump the same way, so a Backup Store built
    // from an Oracle backup can say why it decodes no rows.
    let data = std::fs::read(ORACLE_EXPORT_DMP).expect("read Oracle dump");
    let err = locate_sql_server_streams(&data).expect_err("an Oracle dump has no MTF streams");
    assert!(matches!(err, SqlServerDumpError::NotMtf(_)), "{err}");
    assert_eq!(
        format!("error: {err}\n"),
        probe_stderr,
        "the binaries print the library's wording"
    );
}

#[test]
fn mdf_bytes_of_dump_are_what_pid_backup_extract_as_mdf_writes() {
    const EXPORT_MDF: &str = "test-file/backup-test/TEST02_p/extracted/Export.mdf";
    if !Path::new(EXPORT_DMP).exists() || !Path::new(EXPORT_MDF).exists() {
        eprintln!("skipping: fixture {EXPORT_DMP} or {EXPORT_MDF} not found");
        return;
    }
    let dump = std::fs::read(EXPORT_DMP).expect("read Export.dmp");
    let extracted = std::fs::read(EXPORT_MDF).expect("read extracted Export.mdf");

    let streams = locate_sql_server_streams(&dump).expect("MSCI and MSDA streams");
    assert_eq!(0x0E10, streams.msci.body_offset);
    assert_eq!(3_304, streams.msci.body_len());
    assert_eq!(0x1E10, streams.msda.body_offset);
    assert_eq!(19_923_952, streams.msda.body_len());
    let msda = &dump[streams.msda.body_offset..streams.msda.body_end];
    assert_eq!(0x3F0, detect_backup_stream_header_len(msda));

    let mdf = mdf_bytes_of_dump(&dump).expect("MDF bytes");
    assert_eq!(19_922_944, mdf.len());
    assert_eq!(extracted.len(), mdf.len());
    assert!(
        extracted == mdf,
        "the extracted fixture is the dump's MDF byte for byte"
    );
}
