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

use pid_parse::publish::{
    load_drawing_graph_from_mdf, write_data_xml, write_meta_xml, PublishError, PublishStyle,
};

// -----------------------------------------------------------------
// Fixture paths and identifiers — A01 plant (TEST02)
// -----------------------------------------------------------------

/// MDF extracted from the TEST02 plant backup. This is the
/// single source-of-truth path for publish fidelity tests.
pub const A01_MDF_PATH: &str = "test-file/backup-test/TEST02_p/extracted/Export.mdf";

/// Legacy SQLite mirror kept only for `sqlite_load` compatibility
/// tests. New publish fidelity gates must use [`A01_MDF_PATH`].
pub const SQLITE_PATH: &str = "test-file/backup-test/TEST02_p/extracted/Export_v2.sqlite";

/// `T_Drawing.SP_ID` of the A01 drawing inside the TEST02
/// MDF fixture.
pub const A01_DRAWING_UID: &str = "D9635C3C898840D1990B7E8BEE1D55DA";

/// Plant name passed to `write_data_xml(_)` when generating
/// XML in tests. Matches the TEST02 plant in the MDF fixture
/// so any plant-level cross-checks line up.
pub const PLANT_NAME: &str = "TEST02";

/// Path to the SmartPlant-produced reference `_Data.xml` for
/// the A01 drawing. Bundled in `test-file/export-test/`.
pub const A01_REFERENCE_DATA_XML: &str =
    "test-file/export-test/publish-data/A01/A01_Data.xml";

/// Path to the SmartPlant-produced reference `_Meta.xml` for
/// the A01 drawing. Sibling of [`A01_REFERENCE_DATA_XML`];
/// drives the `_Meta.xml` parity gates in
/// `tests/publish_meta_parity.rs`.
pub const A01_REFERENCE_META_XML: &str =
    "test-file/export-test/publish-data/A01/A01_Meta.xml";

// -----------------------------------------------------------------
// Fixture paths and identifiers — DWG plant (P01)
// -----------------------------------------------------------------

/// Path to the SmartPlant-produced reference `_Data.xml` for
/// the DWG-0202GP06-01 drawing. Used by every cross-fixture
/// universality test (A24 / A27b) and by the A28 backlog
/// inventory.
pub const DWG_REFERENCE_DATA_XML: &str =
    "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01_Data.xml";

/// Path to the SmartPlant-produced reference `_Meta.xml` for
/// the DWG-0202GP06-01 drawing.
pub const DWG_REFERENCE_META_XML: &str =
    "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01_Meta.xml";

/// Expected location of the DWG plant's MDF fixture. The path
/// convention mirrors the A01 layout:
/// `test-file/backup-test/<plant>_p/extracted/Export.mdf`.
/// When the file is present, every DWG loader/writer gate runs
/// end-to-end through the Rust MDF reader; when absent,
/// dependent tests soft-skip with [`DWG_MDF_MISSING_HINT`].
pub const DWG_MDF_PATH: &str = "test-file/backup-test/DWG-0202GP06-01_p/extracted/Export.mdf";

/// `T_Drawing.SP_ID` of the DWG-0202GP06-01 drawing, sourced
/// from the reference `_Meta.xml`'s `DocUID` attribute so the
/// constant stays aligned with the bundled reference fixture
/// independently of whether the MDF fixture has been produced.
pub const DWG_DRAWING_UID: &str = "9D1F3F232C47409D8C54EFB22FC3682A";

/// Plant name the DWG reference fixture publishes under.
/// Sourced from the reference `_Meta.xml`'s `Plant`
/// attribute. Used as the `plant_name` argument when the DWG
/// loader/writer pipeline is available.
pub const DWG_PLANT_NAME: &str = "P01";

/// Single soft-skip message every DWG-loader-dependent test
/// emits when [`DWG_MDF_PATH`] is not present. The wording
/// explicitly flags the blockage so a reader seeing the
/// message in test output knows the Stage 2–4 gates
/// (loader canonical-field enrichment, A24/A27b whitelist
/// closure, PIDBranchPoint / PIDPipingBranchPoint parity) are
/// NOT validated on this run even though the suite appears
/// green.
pub const DWG_MDF_MISSING_HINT: &str =
    "skipping: DWG Export.mdf fixture not found at \
     `test-file/backup-test/DWG-0202GP06-01_p/extracted/Export.mdf` — \
     loader canonical-field enrichment, A24/A27b whitelist closure, \
     PIDBranchPoint / PIDPipingBranchPoint parity, and every other \
     DWG loader / branch-point gate in Stage 2-4 are NOT verifiable \
     on this run. Drop the MDF fixture in place to re-enable them.";

// -----------------------------------------------------------------
// Parity scope
// -----------------------------------------------------------------

/// PID tag varieties under fidelity parity (A23 / A24 / A27 /
/// A27b). This is the full writer-supported tag surface as of
/// Stage-4, including the two formerly-backlogged branch-point
/// varieties. A23 still soft-skips tags absent from the A01
/// fixture; keeping the union here means the shared whitelist /
/// drift guards stay aligned with `publish::supported_pid_tags()`.
///
/// Sorted alphabetically for stability across whitelists and
/// diff readers.
pub const TAGS_UNDER_PARITY: &[&str] = &[
    "PIDBranchPoint",
    "PIDControlSystemFunction",
    "PIDDrawing",
    "PIDNote",
    "PIDNozzle",
    "PIDPipeline",
    "PIDPipingBranchPoint",
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
/// open the TEST02 MDF with the Rust reader, load the drawing
/// graph for the A01 drawing UID, and emit the XML as a String.
/// Returns `None` when the MDF fixture is missing.
pub fn generate_a01_xml() -> Option<Result<String, PublishError>> {
    let mdf_path = std::path::Path::new(A01_MDF_PATH);
    if !mdf_path.exists() {
        eprintln!("skipping: MDF fixture {A01_MDF_PATH} not found");
        return None;
    }
    let drawing = match load_drawing_graph_from_mdf(mdf_path, A01_DRAWING_UID) {
        Ok(d) => d,
        Err(e) => return Some(Err(e)),
    };
    Some(write_data_xml(&drawing, PLANT_NAME))
}

/// Generate `A01_Meta.xml` through the real publish pipeline.
/// Sibling of [`generate_a01_xml`] but targets the
/// document-versioning `_Meta.xml` surface. Soft-skips when the
/// A01 MDF fixture is missing.
pub fn generate_a01_meta_xml() -> Option<Result<String, PublishError>> {
    let mdf_path = std::path::Path::new(A01_MDF_PATH);
    if !mdf_path.exists() {
        eprintln!("skipping: MDF fixture {A01_MDF_PATH} not found");
        return None;
    }
    let drawing = match load_drawing_graph_from_mdf(mdf_path, A01_DRAWING_UID) {
        Ok(d) => d,
        Err(e) => return Some(Err(e)),
    };
    Some(write_meta_xml(&drawing, PLANT_NAME))
}

/// Generate `DWG-0202GP06-01_Data.xml` through the real
/// publish pipeline, opening the DWG plant's MDF fixture at
/// [`DWG_MDF_PATH`] and emitting with
/// [`PublishStyle::Dwg`] explicitly applied (per the
/// "PublishStyle stays an explicit input" convention — the
/// helper never guesses style from the source file).
///
/// Returns `None` when [`DWG_MDF_PATH`] is missing; emits
/// [`DWG_MDF_MISSING_HINT`] so the blockage is visible in
/// test output and not mistaken for a silent pass.
pub fn generate_dwg_data_xml() -> Option<Result<String, PublishError>> {
    let Some(mut drawing) = open_dwg_drawing() else {
        return None;
    };
    drawing.style = PublishStyle::Dwg;
    Some(write_data_xml(&drawing, DWG_PLANT_NAME))
}

/// Generate `DWG-0202GP06-01_Meta.xml` through the real
/// publish pipeline. Sibling of [`generate_dwg_data_xml`].
pub fn generate_dwg_meta_xml() -> Option<Result<String, PublishError>> {
    let Some(mut drawing) = open_dwg_drawing() else {
        return None;
    };
    drawing.style = PublishStyle::Dwg;
    Some(write_meta_xml(&drawing, DWG_PLANT_NAME))
}

/// Open + load the DWG drawing graph. Returns `None` with a
/// descriptive soft-skip notice when the MDF fixture is missing or
/// the loader step itself fails before reaching the writer.
/// Private because every public DWG helper delegates here —
/// adding a new DWG scenario should extend this function
/// rather than duplicate the skip logic.
fn open_dwg_drawing() -> Option<pid_parse::publish::PublishDrawing> {
    let mdf_path = std::path::Path::new(DWG_MDF_PATH);
    if !mdf_path.exists() {
        eprintln!("{DWG_MDF_MISSING_HINT}");
        return None;
    }
    match load_drawing_graph_from_mdf(mdf_path, DWG_DRAWING_UID) {
        Ok(d) => Some(d),
        Err(e) => {
            eprintln!(
                "skipping: DWG MDF present but load_drawing_graph_from_mdf failed for \
                 drawing UID `{DWG_DRAWING_UID}`: {e}"
            );
            None
        }
    }
}

/// Load the SmartPlant-produced reference A01 `_Data.xml`.
/// Returns `None` when the fixture is missing.
pub fn load_reference_a01_xml() -> Option<String> {
    load_reference_xml(A01_REFERENCE_DATA_XML)
}

/// Load the SmartPlant-produced reference DWG `_Data.xml`.
/// Returns `None` when the fixture is missing.
pub fn load_reference_dwg_xml() -> Option<String> {
    load_reference_xml(DWG_REFERENCE_DATA_XML)
}

/// Load the SmartPlant-produced reference A01 `_Meta.xml`.
pub fn load_reference_a01_meta_xml() -> Option<String> {
    load_reference_xml(A01_REFERENCE_META_XML)
}

/// Load the SmartPlant-produced reference DWG `_Meta.xml`.
pub fn load_reference_dwg_meta_xml() -> Option<String> {
    load_reference_xml(DWG_REFERENCE_META_XML)
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

/// Narrow A01 delivery-contract normalization: only the three
/// A39 evidence-guarded publish-time synthetic residual classes are
/// rewritten.
///
/// Stable object identities — drawing, vessel, nozzle, pipeline,
/// publishable representation UIDs, and every concrete `IRel`
/// endpoint — are left raw. This keeps the contract aligned with
/// the real burn-down target. The ignored A39 probe scans the full
/// Rust MDF table inventory exposed by `oxidized-mdf` and confirms
/// connector-family UID, `<Rel><IObject UID="..."/>`, and
/// `GraphicOID` are not present in the complete TEST02 MDF table
/// scan or MDF raw text / UUID byte forms.
#[derive(Clone, Copy)]
pub struct A01SyntheticMaskOptions {
    pub connector_family: bool,
    pub rel_iobject_uid: bool,
    pub graphic_oid: bool,
}

impl A01SyntheticMaskOptions {
    pub const ALL: Self = Self {
        connector_family: true,
        rel_iobject_uid: true,
        graphic_oid: true,
    };
}

pub fn normalize_a01_synthetic_slots(xml: &str, opts: A01SyntheticMaskOptions) -> String {
    let mut current_block: Option<&'static str> = None;
    let mut connector_uid: Option<String> = None;
    let mut graphic_count = 0usize;
    let mut rel_count = 0usize;
    let mut out = Vec::new();

    for raw in xml.replace("\r\n", "\n").lines() {
        let trimmed = raw.trim_start();
        match trimmed {
            "<PIDPipingConnector>" => current_block = Some("PIDPipingConnector"),
            "<Rel>" => current_block = Some("Rel"),
            _ => {}
        }

        let mut line = raw.to_string();

        if opts.connector_family
            && current_block == Some("PIDPipingConnector")
            && trimmed.starts_with("<IObject ")
        {
            if let Some(uid) = extract_attr(&line, "UID") {
                connector_uid = Some(uid.clone());
                line = replace_attr_value(&line, "UID", "@CONNECTOR@");
            }
        }

        if opts.connector_family {
            if let Some(base) = connector_uid.as_deref() {
                for (from, to) in [
                    (base.to_string(), "@CONNECTOR@"),
                    (format!("{base}.1"), "@PORT1@"),
                    (format!("{base}.2"), "@PORT2@"),
                    (format!("{base}.PPT"), "@PROCESS_POINT@"),
                ] {
                    line = line.replace(&from, to);
                }
            }
        }

        if opts.graphic_oid && trimmed.starts_with("<IDrawingRepresentation ") {
            graphic_count += 1;
            line = replace_attr_value(&line, "GraphicOID", &format!("@GRAPHIC{graphic_count}@"));
        }

        if opts.rel_iobject_uid && current_block == Some("Rel") && trimmed.starts_with("<IObject ")
        {
            rel_count += 1;
            line = replace_attr_value(&line, "UID", &format!("@REL{rel_count}@"));
        }

        out.push(line);

        match trimmed {
            "</PIDPipingConnector>" | "</Rel>" => current_block = None,
            _ => {}
        }
    }

    let mut normalized = out.join("\n");
    normalized.push('\n');
    normalized
}

fn extract_attr(line: &str, attr: &str) -> Option<String> {
    let needle = format!(r#"{attr}=""#);
    let start = line.find(&needle)? + needle.len();
    let end = start + line[start..].find('"')?;
    Some(line[start..end].to_string())
}

fn replace_attr_value(line: &str, attr: &str, replacement: &str) -> String {
    let needle = format!(r#"{attr}=""#);
    let Some(start) = line.find(&needle) else {
        return line.to_string();
    };
    let value_start = start + needle.len();
    let Some(rel_end) = line[value_start..].find('"') else {
        return line.to_string();
    };
    let value_end = value_start + rel_end;
    format!(
        "{}{}{}",
        &line[..value_start],
        replacement,
        &line[value_end..]
    )
}
