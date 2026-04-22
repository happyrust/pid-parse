//! Integration tests for the MSCI (SQL Configuration Information)
//! stream parser. Slices the real `Export.dmp` fixture at the
//! byte-range our MTF scan identified in stage 0 and verifies the
//! parser recovers the two SmartPlant TEST02 file records.

use pid_parse::backup::mtf::{MtfBlockCursor, MtfStreamCursor, MtfStreamKind};
use pid_parse::backup::{parse_msci, MsciFile};
use std::path::Path;

const EXPORT_DMP: &str = "test-file/backup-test/TEST02_p/Export.dmp";

fn load_fixture() -> Option<Vec<u8>> {
    let p = Path::new(EXPORT_DMP);
    if !p.exists() {
        eprintln!("skipping: fixture {EXPORT_DMP} not found");
        return None;
    }
    Some(std::fs::read(p).unwrap_or_else(|e| panic!("read {EXPORT_DMP}: {e}")))
}

/// Locate the first MSCI stream in `data` using the same walker
/// chain production callers would use (MtfBlockCursor →
/// MtfStreamCursor). Returns the stream's body byte slice.
fn locate_msci_body(data: &[u8]) -> Option<&[u8]> {
    for block in MtfBlockCursor::new(data) {
        let offset_to_first_event = u16::from_le_bytes([
            block.raw_common_header[8],
            block.raw_common_header[9],
        ]) as usize;
        let start = block.offset + offset_to_first_event;
        let end = block.offset + block.size;
        for stream in MtfStreamCursor::new(data, start, end) {
            if matches!(stream.kind, MtfStreamKind::SqlConfig) {
                return Some(&data[stream.body_offset..stream.body_end]);
            }
        }
    }
    None
}

#[test]
fn real_export_dmp_msci_parses_two_file_records() {
    let Some(data) = load_fixture() else {
        return;
    };
    let body = locate_msci_body(&data).expect("MTF layout must contain an MSCI stream");
    let config = parse_msci(body).expect("MSCI body from real fixture should parse");

    // The TEST02 backup set holds exactly one MDF + one LDF.
    assert_eq!(
        config.files.len(),
        2,
        "expected 2 file records; got {}: {:?}",
        config.files.len(),
        config.files
    );

    // Parser strips the leading 'H' marker that SQL Server emits
    // before every UTF-16LE string field in MSCI records.
    assert_eq!(
        config.files[0].logical_name, "SP3DTrain_RDB_SCHEMA_dat",
        "first record should surface the MDF logical name verbatim"
    );
    assert_eq!(
        config.files[1].logical_name, "SP3DTrain_RDB_SCHEMA_log",
        "second record should surface the LDF logical name verbatim"
    );
    assert_eq!(
        config.filegroup_name.as_deref(),
        Some("PRIMARY"),
        "SmartFile Group Info should report 'PRIMARY' for the fixture"
    );

    // Physical paths should point at the SQL Server data directory
    // for MSSQL 10.50 (SQL Server 2008 R2).
    for (idx, file) in config.files.iter().enumerate() {
        assert!(
            file.physical_path.contains(r"MSSQL10_50.MSSQLSERVER"),
            "record #{idx} physical path should contain MSSQL10_50 path stem; got `{}`",
            file.physical_path
        );
    }
    assert!(
        config.files[0].physical_path.ends_with(".mdf"),
        "first record's path should end with .mdf; got `{}`",
        config.files[0].physical_path
    );
    assert!(
        config.files[1].physical_path.ends_with(".ldf"),
        "second record's path should end with .ldf; got `{}`",
        config.files[1].physical_path
    );

    // Sanity echo for diagnostic runs with --nocapture.
    eprintln!("parsed MSCI config:");
    if let Some(fg) = &config.filegroup_name {
        eprintln!("  filegroup_name = `{fg}`");
    }
    for (i, file) in config.files.iter().enumerate() {
        let MsciFile {
            record_offset,
            logical_name,
            physical_path,
        } = file;
        eprintln!(
            "  #{i}  SFIN@0x{record_offset:06X}  logical=`{logical_name}`  path=`{physical_path}`"
        );
    }
}
