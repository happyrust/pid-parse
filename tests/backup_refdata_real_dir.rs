//! Integration gate: scan the real `TEST02_p/` plant backup
//! folder and assert the production-shape `RefData~4~*` inventory
//! lines up exactly with what `src/backup/refdata.rs` reports.
//!
//! `crate::backup::refdata` already has unit-level coverage on
//! synthetic temp directories. This file complements those with a
//! shape-anchored check on the bundled `TEST02_p/` fixture so the
//! scanner stays honest about the four magic-byte families a real
//! `SmartPlant` plant backup actually carries (ZIP / CFB / XML /
//! ASCII-text). When the fixture is absent (the `test-file/`
//! tree is git-ignored — see `AGENTS.md`), the test soft-skips
//! with an explicit hint so the suite stays green for a fresh
//! checkout.
//!
//! Counts and per-id format / filename assertions are exact. Size is
//! checked against the current on-disk metadata so private and
//! sanitized fixtures can differ without hiding scanner regressions.

use std::path::Path;

use pid_parse::backup::{scan_refdata_dir, RefDataFormat};

/// Path to the bundled real-shape fixture. Lives under
/// `test-file/backup-test/TEST02_p/` along with `Manifest.txt`
/// and the `Export.dmp` MTF backup.
const FIXTURE_DIR: &str = "test-file/backup-test/TEST02_p";

/// Soft-skip hint emitted when the fixture is missing. Mirrors the
/// pattern used by `tests/backup_manifest_real_file.rs` so the
/// failure mode is consistent across the backup test suite.
const FIXTURE_MISSING_HINT: &str = "skipping: TEST02_p plant backup fixture not found at \
     `test-file/backup-test/TEST02_p` — \
     plant-restore Step 2 RefData scan coverage is NOT validated \
     on this run. Drop the backup folder under `test-file/` to \
     enable.";

#[test]
fn scan_real_test02_refdata_inventory_matches_on_disk_shape() {
    let dir = Path::new(FIXTURE_DIR);
    if !dir.exists() {
        eprintln!("{FIXTURE_MISSING_HINT}");
        return;
    }

    let entries = scan_refdata_dir(dir).expect("scan TEST02_p");

    // The bundled export ships exactly 10 RefData files, mixing
    // .zip and extension-less names.
    assert_eq!(entries.len(), 10, "RefData entry count");

    // All RefData files in this export carry schema 4.
    for e in &entries {
        assert_eq!(e.schema, 4, "{} should be schema 4", e.file_name);
    }

    // Sorted ascending by (schema, id) — `scan_refdata_dir`
    // guarantees this regardless of `read_dir`'s native order.
    let ids: Vec<u32> = entries.iter().map(|e| e.id).collect();
    assert_eq!(
        ids,
        vec![680, 681, 682, 683, 684, 685, 703, 709, 804, 809],
        "ids must be sorted ascending and match the bundled set",
    );

    // Per-id format + size assertions, indexed via a BTreeMap so
    // the failure messages stay clear even if the entry order
    // ever drifts.
    let by_id: std::collections::BTreeMap<u32, &pid_parse::backup::RefDataEntry> =
        entries.iter().map(|e| (e.id, e)).collect();

    let cases: &[(u32, RefDataFormat, &str)] = &[
        // The `SmartPlant` Rules CSV — `"Begin Rules",120,…`
        // sits right at the start of the file.
        (680, RefDataFormat::AsciiText, "RefData~4~680"),
        // The five `.zip` payloads.
        (681, RefDataFormat::Zip, "RefData~4~681.zip"),
        (682, RefDataFormat::Zip, "RefData~4~682.zip"),
        (684, RefDataFormat::Zip, "RefData~4~684.zip"),
        (685, RefDataFormat::Zip, "RefData~4~685.zip"),
        (804, RefDataFormat::Zip, "RefData~4~804.zip"),
        (809, RefDataFormat::Zip, "RefData~4~809.zip"),
        // OLE / CFB compound file (no extension).
        (683, RefDataFormat::Cfb, "RefData~4~683"),
        // ZIP without `.zip` suffix.
        (703, RefDataFormat::Zip, "RefData~4~703"),
        // `<ProjectInsulationSpecifications>` XML.
        (709, RefDataFormat::Xml, "RefData~4~709"),
    ];

    for (id, expected_format, expected_name) in cases {
        let entry = by_id
            .get(id)
            .unwrap_or_else(|| panic!("RefData id {id} missing"));
        let expected_size = dir
            .join(expected_name)
            .metadata()
            .unwrap_or_else(|err| panic!("metadata for {expected_name}: {err}"))
            .len();
        assert_eq!(entry.format, *expected_format, "format for id {id}");
        assert_eq!(entry.size, expected_size, "size for id {id}");
        assert_eq!(entry.file_name, *expected_name, "file_name for id {id}");
    }
}
