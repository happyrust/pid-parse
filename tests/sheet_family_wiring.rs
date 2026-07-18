//! M3-PR17 wiring-consistency gate: for every local fixture sheet, the
//! populated [`pid_parse::SheetGeometry`] family fields must agree with
//! a fresh re-decode of the same raw bytes through the
//! [`pid_parse::SHEET_RECORD_FAMILIES`] registry.
//!
//! This closes the historical silent gap where a decoder existed and
//! its cross-fixture ratchet (which re-decodes raw bytes directly)
//! stayed green, while the `streams/cluster.rs` wiring for the family
//! was missing — so parsed documents quietly lacked the records.
//! `attribute_fragments` sitting outside the old 12-conjunct empty
//! check is exactly the class of drift this test now surfaces.
//!
//! Fixtures soft-skip when absent, mirroring `tests/parse_real_files.rs`.

use pid_parse::{PidParser, SHEET_RECORD_FAMILIES};

const FIXTURES: &[&str] = &[
    "DWG-0201GP06-01.pid",
    "DWG-0202GP06-01.pid",
    "工艺管道及仪表流程-1.pid",
    "export-test/publish-data/A01/A01.pid",
    "D06.pid",
];

#[test]
fn sheet_geometry_family_fields_match_registry_redecode() {
    let mut checked_sheets = 0usize;
    let mut checked_fixtures = 0usize;

    for fixture in FIXTURES {
        let path = format!("test-file/{fixture}");
        if !std::path::Path::new(&path).exists() {
            eprintln!("skipping: fixture {path} not found");
            continue;
        }
        let pkg = PidParser::new()
            .parse_package(&path)
            .unwrap_or_else(|err| panic!("fixture {path} should parse: {err}"));
        checked_fixtures += 1;

        for sheet in &pkg.parsed.sheet_streams {
            let Some(raw) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            let bytes = raw.data.as_slice();
            checked_sheets += 1;

            match &sheet.geometry {
                Some(geometry) => {
                    for family in SHEET_RECORD_FAMILIES {
                        let wired = (family.record_count)(geometry);
                        let redecoded = (family.decoded_ranges)(bytes).len();
                        assert_eq!(
                            wired, redecoded,
                            "{fixture} {}: family {} has {wired} wired record(s) in \
                             SheetGeometry but {redecoded} record(s) on raw re-decode — \
                             cluster wiring and decoder disagree",
                            sheet.path, family.name
                        );
                    }
                }
                None => {
                    // No geometry evidence at all: every family must
                    // also re-decode to zero records, otherwise the
                    // cluster pass dropped a populated sheet.
                    for family in SHEET_RECORD_FAMILIES {
                        let redecoded = (family.decoded_ranges)(bytes).len();
                        assert_eq!(
                            redecoded, 0,
                            "{fixture} {}: sheet has no SheetGeometry but family {} \
                             re-decodes {redecoded} record(s) from raw bytes",
                            sheet.path, family.name
                        );
                    }
                }
            }
        }
    }

    if checked_fixtures == 0 {
        eprintln!("skipping: no local fixtures available for wiring consistency check");
        return;
    }
    eprintln!(
        "wiring consistency verified on {checked_sheets} sheet stream(s) across \
         {checked_fixtures} fixture(s) x {} families",
        SHEET_RECORD_FAMILIES.len()
    );
    assert!(
        checked_sheets > 0,
        "fixtures parsed but no sheet streams were checked"
    );
}
