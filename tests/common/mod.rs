//! Shared helpers for the publish-related integration tests
//! (`tests/publish_*.rs`).
//!
//! Cargo treats every `tests/*.rs` file as an independent
//! integration-test binary and does NOT auto-link sibling
//! modules across binaries. We work around that with the
//! `tests/common/mod.rs` pattern — each test file that wants
//! the helpers writes `mod common;` and Cargo resolves the
//! reference to this directory's `mod.rs`.
//!
//! The `#[allow(dead_code)]` on the module body is required:
//! Cargo independently compiles `common` into every binary
//! that imports it, so any helper used by only a subset of
//! the tests would otherwise trip a dead-code warning in the
//! binaries that do not exercise it. The contract is that
//! every public item in this file is exercised by AT LEAST
//! ONE integration test in the workspace.

#![allow(dead_code)]

use pid_parse::publish::sqlite_load::open_readonly;
use pid_parse::publish::{load_drawing_graph, write_data_xml, PublishError};

// -----------------------------------------------------------------
// Fixture paths and identifiers
// -----------------------------------------------------------------

/// SQLite mirror produced by `tools/orca-mdf-probe` from the
/// TEST02 plant backup. Single source-of-truth path used by
/// every loader-side integration test.
pub const SQLITE_PATH: &str = "test-file/backup-test/TEST02_p/extracted/Export_v2.sqlite";

/// `T_Drawing.SP_ID` of the A01 drawing inside the TEST02
/// SQLite mirror — the only drawing the mirror contains as
/// of A30. New plant fixtures will need additional UID
/// constants.
pub const A01_DRAWING_UID: &str = "D9635C3C898840D1990B7E8BEE1D55DA";

/// Plant name passed to `write_data_xml(_)` when generating
/// XML in tests. Matches the TEST02 plant in the SQLite
/// mirror so any plant-level cross-checks line up.
pub const PLANT_NAME: &str = "TEST02";

/// Path to the SmartPlant-produced reference `_Data.xml` for
/// the A01 drawing. Bundled in `test-file/export-test/`.
pub const A01_REFERENCE_DATA_XML: &str =
    "test-file/export-test/publish-data/A01/A01_Data.xml";

/// Path to the SmartPlant-produced reference `_Data.xml` for
/// the DWG-0202GP06-01 drawing. Used by every cross-fixture
/// universality test (A24 / A27b) and by the A28 backlog
/// inventory.
pub const DWG_REFERENCE_DATA_XML: &str =
    "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01_Data.xml";

// -----------------------------------------------------------------
// Parity scope
// -----------------------------------------------------------------

/// PID tag varieties under fidelity parity (A23 / A24 / A27 /
/// A27b). Tags the writer does NOT yet emit
/// (PIDBranchPoint / PIDPipingBranchPoint backlog) are
/// deliberately out of scope; their fidelity inventory lives
/// in `publish_backlog_inventory.rs` instead.
///
/// Sorted alphabetically for stability across whitelists and
/// diff readers.
pub const TAGS_UNDER_PARITY: &[&str] = &[
    "PIDControlSystemFunction",
    "PIDDrawing",
    "PIDNote",
    "PIDNozzle",
    "PIDPipeline",
    "PIDPipingComponent",
    "PIDPipingConnector",
    "PIDPipingPort",
    "PIDProcessPoint",
    "PIDProcessVessel",
    "PIDRepresentation",
    "PIDSignalConnector",
    "PIDSignalPort",
];

// -----------------------------------------------------------------
// Fixture loaders
// -----------------------------------------------------------------

/// Generate `A01_Data.xml` through the real publish pipeline:
/// open the TEST02 SQLite mirror, load the drawing graph for
/// the A01 drawing UID, and emit the XML as a String. Returns
/// `None` when the SQLite fixture is missing (soft-skip on CI
/// workers without the SmartPlant backup bundle).
pub fn generate_a01_xml() -> Option<Result<String, PublishError>> {
    let sqlite_path = std::path::Path::new(SQLITE_PATH);
    if !sqlite_path.exists() {
        eprintln!("skipping: SQLite fixture {SQLITE_PATH} not found");
        return None;
    }
    let conn = match open_readonly(sqlite_path) {
        Ok(c) => c,
        Err(e) => return Some(Err(e)),
    };
    let drawing = match load_drawing_graph(&conn, A01_DRAWING_UID) {
        Ok(d) => d,
        Err(e) => return Some(Err(e)),
    };
    Some(write_data_xml(&drawing, PLANT_NAME))
}

/// Load the SmartPlant-produced reference A01 XML. Returns
/// `None` when the fixture is missing.
pub fn load_reference_a01_xml() -> Option<String> {
    load_reference_xml(A01_REFERENCE_DATA_XML)
}

/// Load the SmartPlant-produced reference DWG XML. Returns
/// `None` when the fixture is missing.
pub fn load_reference_dwg_xml() -> Option<String> {
    load_reference_xml(DWG_REFERENCE_DATA_XML)
}

/// Generic UTF-8 reader that prints a soft-skip notice when
/// the file is missing. Centralized so adding new reference
/// fixtures only requires a one-liner wrapper.
fn load_reference_xml(path: &str) -> Option<String> {
    let p = std::path::Path::new(path);
    if !p.exists() {
        eprintln!("skipping: reference fixture {path} not found");
        return None;
    }
    Some(std::fs::read_to_string(p).expect("reference should be utf-8"))
}
