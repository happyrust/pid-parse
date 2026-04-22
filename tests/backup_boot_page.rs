//! Integration test: locate page 9 inside the extracted MSDA body
//! and assert [`parse_boot_page`] recovers the SmartPlant fixture's
//! logical database name.

use pid_parse::backup::mdf_page::PAGE_SIZE;
use pid_parse::backup::{parse_boot_page, MdfPageCursor, PageType};
use std::path::Path;

const MSDA_PATH: &str = "test-file/backup-test/TEST02_p/extracted/Export.msda.bin";
/// Byte offset at which MDF pages start inside the extracted MSDA
/// body. Auto-detected by `pid_msda_probe` during stage 0+ and
/// observed to be 0x3F0 for the TEST02 fixture.
const MDF_BASE: usize = 0x3F0;

fn load_msda() -> Option<Vec<u8>> {
    let p = Path::new(MSDA_PATH);
    if !p.exists() {
        eprintln!("skipping: fixture {MSDA_PATH} not found");
        eprintln!(
            "run `cargo run --bin pid_backup_extract -- \
             test-file/backup-test/TEST02_p/Export.dmp --out \
             test-file/backup-test/TEST02_p/extracted/` first"
        );
        return None;
    }
    Some(std::fs::read(p).unwrap_or_else(|e| panic!("read {MSDA_PATH}: {e}")))
}

#[test]
fn real_msda_boot_page_reports_smartplant_database_name() {
    let Some(data) = load_msda() else {
        return;
    };

    // Walk pages until we hit a Boot Page (type 13). In the
    // TEST02 fixture it is page index 9 at offset 0x123F0; by
    // searching rather than hard-coding the index we stay robust
    // to future fixtures that may differ.
    let mut boot_page_bytes: Option<&[u8]> = None;
    for (_idx, offset, header) in MdfPageCursor::new(&data, MDF_BASE, PAGE_SIZE) {
        if header.page_type == PageType::BootPage {
            boot_page_bytes = Some(&data[offset..offset + PAGE_SIZE]);
            break;
        }
    }
    let page = boot_page_bytes.expect("TEST02 MSDA body must contain a Boot Page");

    let info = parse_boot_page(page).expect("Boot Page should parse");
    assert_eq!(
        info.database_name, "SP3DTrain_RDB_SCHEMA",
        "Boot Page DBINFO should report the SmartPlant database name"
    );

    eprintln!("Boot Page database_name = `{}`", info.database_name);
}
