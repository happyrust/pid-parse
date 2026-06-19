//! Planning types for exporting a parsed `.pid` file as a directory bundle.
//!
//! The bundle exporter is intentionally staged. C1 defined the user-facing
//! plan; C2 adds the minimal writer; C4 emits decoded split views and
//! geometry files without promoting any parsed evidence between confidence
//! levels; C5 adds writer guidance files without adding any reverse-geometry
//! writer; C8/C9 add publish-input identity and deferred publish status
//! outputs; C10 adds an explicit helper that generates publish XML only when
//! callers opt into the MDF-backed publish pipeline.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

const BUNDLE_SCHEMA_VERSION: u32 = 1;

/// Declarative plan for a `.pid.bundle/` export.
///
/// The default plan follows the Phase 32 bundle contract: emit the manifest,
/// stream inventory, decoded document/split views, geometry summaries, audit
/// reports, and writer guidance; keep raw stream byte dumps and `MDF` publish
/// output opt-in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportBundlePlan {
    /// Version of the bundle directory schema written by this plan.
    #[serde(default = "default_bundle_schema_version")]
    pub bundle_schema_version: u32,
    /// Whether to write `raw/streams/*.bin` byte dumps.
    #[serde(default)]
    pub include_raw_stream_bytes: bool,
    /// Whether to write split convenience files under `decoded/` in addition
    /// to the full `decoded/document.json`.
    #[serde(default = "default_true")]
    pub include_decoded_split_views: bool,
    /// Whether to write `geometry/` outputs when normalized geometry is
    /// available.
    #[serde(default = "default_true")]
    pub include_geometry: bool,
    /// Whether to write `audit/` outputs such as coverage, byte audit, unknown
    /// streams, and confidence ledger files.
    #[serde(default = "default_true")]
    pub include_audit: bool,
    /// Whether to write `writer/` guidance files describing safe edit surfaces.
    #[serde(default = "default_true")]
    pub include_writer_guidance: bool,
    /// Optional `MDF`-backed publish subtree plan. `None` means no `publish/`
    /// directory is emitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish: Option<ExportBundlePublishPlan>,
}

/// Optional publish subtree configuration for [`ExportBundlePlan`].
///
/// Publish output is deliberately separate from `.pid` raw parsing. When this
/// is present, the eventual bundle manifest must record the `MDF` input hash so
/// consumers do not mistake publish XML for facts decoded from the `.pid`
/// container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportBundlePublishPlan {
    /// Path to the `Export.mdf` or legacy publish `SQLite` input.
    pub input_path: PathBuf,
    /// Optional drawing UID to pass to the publish pipeline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawing_uid: Option<String>,
    /// Optional plant name used by `_Data.xml` / `_Meta.xml` generation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plant: Option<String>,
    /// Optional publish style selector (`a01` / `dwg`) used by the publish
    /// pipeline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// Whether to include publish diff artifacts when a reference is supplied
    /// by a future exporter.
    #[serde(default)]
    pub include_diff: bool,
    /// Optional reference `_Data.xml` used to emit `publish/publish_diff.json`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_data_xml: Option<PathBuf>,
}

/// Manifest written at the root of a `.pid.bundle/` directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportBundleManifest {
    /// Version of the bundle directory schema.
    pub bundle_schema_version: u32,
    /// Tool metadata.
    pub tool: ExportBundleToolInfo,
    /// Source `.pid` identity.
    pub source: ExportBundleSourceInfo,
    /// Feature switches that shaped this export.
    pub features: ExportBundleFeatureInfo,
    /// Coarse counts for quick inspection.
    pub counts: ExportBundleCounts,
    /// Input file identities used to produce this bundle.
    pub inputs: ExportBundleInputs,
}

/// Tool metadata embedded in [`ExportBundleManifest`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportBundleToolInfo {
    /// Tool name.
    pub name: String,
    /// Crate version.
    pub version: String,
}

/// Source `.pid` identity embedded in [`ExportBundleManifest`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportBundleSourceInfo {
    /// Source path, if the package was parsed from a file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Source file size in bytes, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    /// Source modified timestamp as seconds since Unix epoch, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_unix_seconds: Option<u64>,
}

/// Input identities embedded in [`ExportBundleManifest`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportBundleInputs {
    /// Source `.pid` identity, if the package was parsed from a file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<ExportBundleInputIdentity>,
    /// Optional publish input identity. Present only when publish output is
    /// explicitly requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish_mdf: Option<ExportBundleInputIdentity>,
}

/// Stable file identity recorded in `manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportBundleInputIdentity {
    /// Input path as provided by the caller / source package.
    pub path: String,
    /// Lowercase hex SHA-256 digest of the input bytes.
    pub sha256: String,
    /// Input file size in bytes.
    pub size_bytes: u64,
    /// Input kind, for example `pid`, `mdf`, or `sqlite`.
    pub kind: String,
}

/// Feature switches embedded in [`ExportBundleManifest`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportBundleFeatureInfo {
    /// Whether raw stream `.bin` files were written.
    pub raw_stream_bytes: bool,
    /// Whether split decoded view files were requested.
    pub decoded_split_views: bool,
    /// Whether geometry files were requested.
    pub geometry: bool,
    /// Whether audit files were requested.
    pub audit: bool,
    /// Whether writer guidance files were requested.
    pub writer_guidance: bool,
    /// Whether the optional publish subtree was requested.
    pub publish_xml: bool,
}

/// Coarse bundle counts embedded in [`ExportBundleManifest`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportBundleCounts {
    /// Number of CFB streams in the source package.
    pub streams: usize,
    /// Number of decoded normalized geometry entities.
    pub decoded_geometry_entities: usize,
    /// Number of inferred/audit normalized geometry entities.
    pub audit_geometry_entities: usize,
    /// Number of probe-only normalized geometry entities.
    pub probe_geometry_entities: usize,
    /// Number of unknown streams reported by the parsed document.
    pub unknown_streams: usize,
}

/// Stream inventory written to `raw/streams.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportBundleStreamIndex {
    /// Stream entries in deterministic package order.
    pub streams: Vec<ExportBundleStreamEntry>,
}

/// One stream entry in [`ExportBundleStreamIndex`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportBundleStreamEntry {
    /// Normalized CFB stream path.
    pub path: String,
    /// Reversible, Windows-safe raw stream filename.
    pub escaped_filename: String,
    /// Stream size in bytes.
    pub size_bytes: usize,
    /// Whether the raw stream `.bin` file was emitted.
    pub raw_bytes_written: bool,
}

/// Confidence ledger written to `audit/confidence_ledger.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportBundleConfidenceLedger {
    /// Ledger entries describing the minimal C2 bundle outputs.
    pub entries: Vec<ExportBundleConfidenceEntry>,
}

/// One confidence-ledger row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportBundleConfidenceEntry {
    /// Source stream path, pseudo-path, or source domain represented by this row.
    pub source_path: String,
    /// Bundle-relative file path.
    pub bundle_path: String,
    /// Stream family, record family, or exported view family.
    pub family: String,
    /// Confidence class for the exported content.
    pub confidence: String,
    /// Evidence supporting this ledger row's classification.
    pub evidence: Vec<String>,
    /// Remaining blockers that prevent stronger parser or writer claims.
    pub blockers: Vec<String>,
    /// Human-facing summary of what the file contains.
    pub summary: String,
}

/// Status written to `publish/status.json` when publish output is requested.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportBundlePublishStatus {
    /// Requested drawing UID, if supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_drawing_uid: Option<String>,
    /// Requested plant name, if supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plant: Option<String>,
    /// Requested publish style (`a01` / `dwg`), if supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// Identity of the publish input, if readable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<ExportBundleInputIdentity>,
    /// Whether `publish/data.xml` was written.
    pub data_xml_written: bool,
    /// Whether `publish/meta.xml` was written.
    pub meta_xml_written: bool,
    /// Whether publish reference comparison was run.
    pub reference_comparison: ExportBundlePublishComparisonStatus,
    /// Skipped/error reason for the current publish subtree state.
    pub status: ExportBundlePublishRunStatus,
}

/// Reference comparison status embedded in [`ExportBundlePublishStatus`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportBundlePublishComparisonStatus {
    /// Machine-readable comparison state.
    pub state: String,
    /// Optional human-readable reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Run status embedded in [`ExportBundlePublishStatus`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportBundlePublishRunStatus {
    /// Machine-readable publish state.
    pub state: String,
    /// Human-readable reason.
    pub reason: String,
}

#[derive(Debug, Serialize)]
struct ExportBundlePublishDiffArtifact {
    reference_path: String,
    clean: bool,
    pid_tags: ExportBundlePublishDiffSummary,
    rel_defuids: ExportBundlePublishDiffSummary,
}

#[derive(Debug, Serialize)]
struct ExportBundlePublishDiffSummary {
    generated_total: usize,
    reference_total: usize,
    matching: usize,
    missing_from_generated: usize,
    extra_in_generated: usize,
    count_deltas: usize,
}

/// Export the Phase 32 bundle skeleton.
///
/// The mandatory core files are:
///
/// - `manifest.json`
/// - `raw/streams.json`
/// - optional `raw/streams/*.bin` when [`ExportBundlePlan::include_raw_stream_bytes`] is `true`
/// - `decoded/document.json`
/// - `audit/confidence_ledger.json`
///
/// C4 also writes decoded split views and geometry files when the corresponding
/// plan flags are enabled. C5 writes `writer/` guidance files. C9 writes a
/// deferred `publish/status.json` for publish opt-in plans, but this function
/// does not generate publish XML; callers must invoke
/// [`export_bundle_publish_xml`] explicitly for that.
pub fn export_bundle(
    package: &crate::PidPackage,
    plan: &ExportBundlePlan,
    out_dir: impl AsRef<std::path::Path>,
) -> Result<(), crate::PidError> {
    let out_dir = out_dir.as_ref();
    fs::create_dir_all(out_dir)?;

    let raw_dir = out_dir.join("raw");
    let decoded_dir = out_dir.join("decoded");
    let audit_dir = out_dir.join("audit");
    fs::create_dir_all(&raw_dir)?;
    fs::create_dir_all(&decoded_dir)?;
    fs::create_dir_all(&audit_dir)?;

    if plan.include_raw_stream_bytes {
        fs::create_dir_all(raw_dir.join("streams"))?;
    }

    let manifest = build_manifest(package, plan);
    write_json_pretty(&out_dir.join("manifest.json"), &manifest)?;

    let streams = build_stream_index(package, plan.include_raw_stream_bytes)?;
    if plan.include_raw_stream_bytes {
        write_raw_stream_files(package, &raw_dir)?;
    }
    write_json_pretty(&raw_dir.join("streams.json"), &streams)?;
    write_json_pretty(&decoded_dir.join("document.json"), &package.parsed)?;
    if plan.include_decoded_split_views {
        write_decoded_split_views(package, &decoded_dir)?;
    }
    if plan.include_geometry {
        write_geometry_files(package, &out_dir.join("geometry"))?;
    }
    if plan.include_writer_guidance {
        write_writer_guidance(package, &out_dir.join("writer"))?;
    }
    if plan.publish.is_some() {
        write_publish_status(plan, &out_dir.join("publish"))?;
    }
    write_json_pretty(
        &audit_dir.join("confidence_ledger.json"),
        &minimal_confidence_ledger(plan),
    )?;

    Ok(())
}

/// Generate `publish/data.xml`, `publish/meta.xml`, and a success
/// `publish/status.json` under a bundle publish directory.
///
/// This helper is the library-level bridge from `.pid.bundle/` export to the
/// existing MDF-backed publish pipeline. It requires an explicit
/// [`ExportBundlePublishPlan::drawing_uid`] and never derives publish XML from
/// `.pid` raw streams.
pub fn export_bundle_publish_xml(
    publish: &ExportBundlePublishPlan,
    publish_dir: impl AsRef<std::path::Path>,
) -> Result<ExportBundlePublishStatus, crate::PidError> {
    let Some(drawing_uid) = publish.drawing_uid.as_deref() else {
        return Err(bundle_publish_error(
            "drawing UID is required to generate bundle publish XML",
        ));
    };
    let publish_dir = publish_dir.as_ref();
    fs::create_dir_all(publish_dir)?;

    let conn = open_publish_input_as_sqlite(&publish.input_path)?;
    let mut drawing = crate::publish::load_drawing_graph(&conn, drawing_uid).map_err(|err| {
        bundle_publish_error(format!("load publish drawing `{drawing_uid}`: {err}"))
    })?;
    let style = parse_publish_style(publish.style.as_deref())?;
    drawing.style = style;
    let plant = publish.plant.as_deref().unwrap_or("P01");

    let data_xml = crate::publish::write_data_xml(&drawing, plant)
        .map_err(|err| bundle_publish_error(format!("render publish data XML: {err}")))?;
    let meta_xml = crate::publish::write_meta_xml(&drawing, plant)
        .map_err(|err| bundle_publish_error(format!("render publish meta XML: {err}")))?;
    fs::write(publish_dir.join("data.xml"), &data_xml)?;
    fs::write(publish_dir.join("meta.xml"), &meta_xml)?;

    let comparison = write_publish_diff_if_requested(publish, publish_dir, &data_xml)?;
    let status = ExportBundlePublishStatus {
        requested_drawing_uid: Some(drawing_uid.to_string()),
        plant: Some(plant.to_string()),
        style: Some(publish_style_label(style).to_string()),
        input: Some(input_identity(
            &publish.input_path,
            publish_input_kind(&publish.input_path),
        )?),
        data_xml_written: true,
        meta_xml_written: true,
        reference_comparison: comparison,
        status: ExportBundlePublishRunStatus {
            state: "written".to_string(),
            reason: "publish XML written via MDF-backed publish pipeline".to_string(),
        },
    };
    write_json_pretty(&publish_dir.join("status.json"), &status)?;
    Ok(status)
}

impl Default for ExportBundlePlan {
    fn default() -> Self {
        Self {
            bundle_schema_version: BUNDLE_SCHEMA_VERSION,
            include_raw_stream_bytes: false,
            include_decoded_split_views: true,
            include_geometry: true,
            include_audit: true,
            include_writer_guidance: true,
            publish: None,
        }
    }
}

impl ExportBundlePlan {
    /// Return the minimal contract files only.
    ///
    /// This keeps `manifest.json`, `raw/streams.json`,
    /// `decoded/document.json`, and `audit/confidence_ledger.json` in scope
    /// for the future exporter while disabling split views and optional
    /// geometry / writer guidance.
    pub fn minimal() -> Self {
        Self {
            include_decoded_split_views: false,
            include_geometry: false,
            include_audit: true,
            include_writer_guidance: false,
            ..Self::default()
        }
    }

    /// Enable opt-in raw stream byte dumps.
    pub fn with_raw_stream_bytes(mut self) -> Self {
        self.include_raw_stream_bytes = true;
        self
    }

    /// Attach an `MDF` / publish input plan, enabling the optional `publish/`
    /// subtree for the future exporter.
    pub fn with_publish(mut self, publish: ExportBundlePublishPlan) -> Self {
        self.publish = Some(publish);
        self
    }

    /// `true` when the future exporter should emit `raw/streams/*.bin` files.
    pub fn writes_raw_stream_bytes(&self) -> bool {
        self.include_raw_stream_bytes
    }

    /// `true` when the optional `publish/` subtree is requested.
    pub fn writes_publish_outputs(&self) -> bool {
        self.publish.is_some()
    }

    /// Parse a JSON export-bundle plan.
    pub fn from_json(json: &str) -> Result<Self, crate::error::PidError> {
        serde_json::from_str(json).map_err(|e| crate::error::PidError::ParseFailure {
            context: "ExportBundlePlan JSON".into(),
            message: e.to_string(),
        })
    }

    /// Serialize this export-bundle plan as compact JSON.
    pub fn to_json(&self) -> Result<String, crate::error::PidError> {
        serde_json::to_string(self).map_err(|e| crate::error::PidError::ParseFailure {
            context: "ExportBundlePlan serialization".into(),
            message: e.to_string(),
        })
    }

    /// Serialize this export-bundle plan as pretty-printed JSON.
    pub fn to_json_pretty(&self) -> Result<String, crate::error::PidError> {
        serde_json::to_string_pretty(self).map_err(|e| crate::error::PidError::ParseFailure {
            context: "ExportBundlePlan serialization".into(),
            message: e.to_string(),
        })
    }
}

fn build_manifest(package: &crate::PidPackage, plan: &ExportBundlePlan) -> ExportBundleManifest {
    let source = source_info(package);
    let geometry_counts = if plan.include_geometry {
        Some(geometry_counts(&crate::build_normalized_geometry(
            &package.parsed,
        )))
    } else {
        None
    };
    let (decoded_geometry_entities, audit_geometry_entities, probe_geometry_entities) =
        geometry_counts.unwrap_or_default();
    ExportBundleManifest {
        bundle_schema_version: plan.bundle_schema_version,
        tool: ExportBundleToolInfo {
            name: "pid-parse".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        source,
        features: ExportBundleFeatureInfo {
            raw_stream_bytes: plan.include_raw_stream_bytes,
            decoded_split_views: plan.include_decoded_split_views,
            geometry: plan.include_geometry,
            audit: plan.include_audit,
            writer_guidance: plan.include_writer_guidance,
            publish_xml: plan.publish.is_some(),
        },
        counts: ExportBundleCounts {
            streams: package.streams.len(),
            decoded_geometry_entities,
            audit_geometry_entities,
            probe_geometry_entities,
            unknown_streams: package.parsed.unknown_streams.len(),
        },
        inputs: build_inputs(package, plan),
    }
}

fn source_info(package: &crate::PidPackage) -> ExportBundleSourceInfo {
    let path = package
        .source_path
        .as_ref()
        .map(|path| path.display().to_string());
    let metadata = package
        .source_path
        .as_ref()
        .and_then(|path| fs::metadata(path).ok());
    let size_bytes = metadata.as_ref().map(std::fs::Metadata::len);
    let modified_unix_seconds = metadata
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs());

    ExportBundleSourceInfo {
        path,
        size_bytes,
        modified_unix_seconds,
    }
}

fn build_inputs(package: &crate::PidPackage, plan: &ExportBundlePlan) -> ExportBundleInputs {
    ExportBundleInputs {
        pid: package
            .source_path
            .as_ref()
            .and_then(|path| input_identity(path, "pid").ok()),
        publish_mdf: plan.publish.as_ref().and_then(|publish| {
            let kind = publish_input_kind(&publish.input_path);
            input_identity(&publish.input_path, kind).ok()
        }),
    }
}

fn input_identity(
    path: &std::path::Path,
    kind: impl Into<String>,
) -> Result<ExportBundleInputIdentity, crate::PidError> {
    let metadata = fs::metadata(path)?;
    Ok(ExportBundleInputIdentity {
        path: path.display().to_string(),
        sha256: sha256_file(path)?,
        size_bytes: metadata.len(),
        kind: kind.into(),
    })
}

fn publish_input_kind(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(std::ffi::OsStr::to_str) {
        Some(ext) if ext.eq_ignore_ascii_case("sqlite") => "sqlite",
        Some(ext) if ext.eq_ignore_ascii_case("db") => "sqlite",
        _ => "mdf",
    }
}

fn sha256_file(path: &std::path::Path) -> Result<String, crate::PidError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut hex, "{byte:02x}").map_err(|e| crate::PidError::ParseFailure {
            context: format!("sha256 hex: {}", path.display()),
            message: e.to_string(),
        })?;
    }
    Ok(hex)
}

fn build_stream_index(
    package: &crate::PidPackage,
    raw_bytes_written: bool,
) -> Result<ExportBundleStreamIndex, crate::PidError> {
    let mut streams = Vec::with_capacity(package.streams.len());
    for raw in package.streams.values() {
        streams.push(ExportBundleStreamEntry {
            path: raw.path.clone(),
            escaped_filename: escaped_stream_filename(&raw.path)?,
            size_bytes: raw.data.len(),
            raw_bytes_written,
        });
    }
    Ok(ExportBundleStreamIndex { streams })
}

fn write_raw_stream_files(
    package: &crate::PidPackage,
    raw_dir: &std::path::Path,
) -> Result<(), crate::PidError> {
    let stream_dir = raw_dir.join("streams");
    for raw in package.streams.values() {
        let filename = escaped_stream_filename(&raw.path)?;
        fs::write(stream_dir.join(filename), &raw.data)?;
    }
    Ok(())
}

fn write_decoded_split_views(
    package: &crate::PidPackage,
    decoded_dir: &std::path::Path,
) -> Result<(), crate::PidError> {
    let doc = &package.parsed;
    write_json_pretty(
        &decoded_dir.join("metadata.json"),
        &serde_json::json!({
            "summary": &doc.summary,
            "drawing_meta": &doc.drawing_meta,
            "general_meta": &doc.general_meta,
            "version_history": &doc.version_history,
            "doc_version2": &doc.doc_version2,
            "doc_version2_decoded": &doc.doc_version2_decoded,
            "app_object_registry": &doc.app_object_registry,
            "tagged_storages": &doc.tagged_storages,
        }),
    )?;
    write_json_pretty(
        &decoded_dir.join("sheets.json"),
        &serde_json::json!({
            "sheet_record_schema": &doc.sheet_record_schema,
            "sheet_streams": &doc.sheet_streams,
        }),
    )?;
    write_json_pretty(
        &decoded_dir.join("psm_tables.json"),
        &serde_json::json!({
            "psm_roots": &doc.psm_roots,
            "psm_cluster_table": &doc.psm_cluster_table,
            "psm_segment_table": &doc.psm_segment_table,
        }),
    )?;
    write_json_pretty(
        &decoded_dir.join("structure.json"),
        &serde_json::json!({
            "cfb_tree": &doc.cfb_tree,
            "streams": &doc.streams,
            "jsites": &doc.jsites,
            "clusters": &doc.clusters,
            "dynamic_attributes": &doc.dynamic_attributes,
            "unknown_streams": &doc.unknown_streams,
        }),
    )?;
    write_json_pretty(
        &decoded_dir.join("import_view.json"),
        &crate::build_import_view(doc),
    )?;
    if let Some(ref object_inventory) = doc.object_inventory {
        write_json_pretty(&decoded_dir.join("object_inventory.json"), object_inventory)?;
    }
    if let Some(ref object_graph) = doc.object_graph {
        write_json_pretty(&decoded_dir.join("object_graph.json"), object_graph)?;
    }
    if let Some(ref cross_reference) = doc.cross_reference {
        write_json_pretty(&decoded_dir.join("cross_reference.json"), cross_reference)?;
    }
    if let Some(ref layout) = doc.layout {
        write_json_pretty(&decoded_dir.join("layout.json"), layout)?;
    }
    Ok(())
}

fn write_geometry_files(
    package: &crate::PidPackage,
    geometry_dir: &std::path::Path,
) -> Result<(), crate::PidError> {
    fs::create_dir_all(geometry_dir)?;
    let geometry = crate::build_normalized_geometry(&package.parsed);
    let decoded_entities: Vec<_> = geometry
        .entities
        .iter()
        .filter(|entity| entity.confidence == crate::PidGeometryConfidence::Decoded)
        .cloned()
        .collect();
    let audit_entities: Vec<_> = geometry
        .entities
        .iter()
        .filter(|entity| entity.confidence == crate::PidGeometryConfidence::Inferred)
        .cloned()
        .collect();
    let probe_entities: Vec<_> = geometry
        .entities
        .iter()
        .filter(|entity| entity.confidence == crate::PidGeometryConfidence::ProbeOnly)
        .cloned()
        .collect();

    write_json_pretty(&geometry_dir.join("normalized_geometry.json"), &geometry)?;
    write_json_pretty(
        &geometry_dir.join("decoded_entities.json"),
        &decoded_entities,
    )?;
    write_json_pretty(&geometry_dir.join("audit_entities.json"), &audit_entities)?;
    write_json_pretty(&geometry_dir.join("probe_entities.json"), &probe_entities)?;
    Ok(())
}

fn geometry_counts(geometry: &crate::NormalizedPidGeometry) -> (usize, usize, usize) {
    let decoded = geometry
        .entities
        .iter()
        .filter(|entity| entity.confidence == crate::PidGeometryConfidence::Decoded)
        .count();
    let audit = geometry
        .entities
        .iter()
        .filter(|entity| entity.confidence == crate::PidGeometryConfidence::Inferred)
        .count();
    let probe = geometry
        .entities
        .iter()
        .filter(|entity| entity.confidence == crate::PidGeometryConfidence::ProbeOnly)
        .count();
    (decoded, audit, probe)
}

fn write_writer_guidance(
    package: &crate::PidPackage,
    writer_dir: &std::path::Path,
) -> Result<(), crate::PidError> {
    fs::create_dir_all(writer_dir)?;
    write_json_pretty(
        &writer_dir.join("round_trip_plan.json"),
        &serde_json::json!({
            "purpose": "Describe currently supported writer surfaces for this bundle.",
            "default_write_plan": crate::WritePlan::default(),
            "package_summary": {
                "streams": package.streams.len(),
                "root_clsid_preserved": package.root_clsid.is_some(),
                "storage_clsids": package.storage_clsids.len(),
                "storage_timestamps": package.storage_timestamps.len(),
                "state_bits": package.state_bits.len(),
            },
            "editable": [
                {
                    "surface": "TaggedTxtData/Drawing XML tag",
                    "method": "PidPackage::set_xml_tag",
                    "confidence": "Decoded",
                    "notes": "Mutates raw XML stream bytes only; reparse after writing for refreshed typed metadata."
                },
                {
                    "surface": "TaggedTxtData/General XML tag",
                    "method": "PidPackage::set_xml_tag",
                    "confidence": "Decoded",
                    "notes": "Mutates raw XML stream bytes only; reparse after writing for refreshed typed metadata."
                },
                {
                    "surface": "OLE SummaryInformation / DocumentSummaryInformation string properties",
                    "method": "WritePlan.metadata_updates.summary_updates",
                    "confidence": "Decoded",
                    "notes": "String properties only; non-string property values remain raw passthrough."
                },
                {
                    "surface": "Verbatim CFB stream replacement",
                    "method": "WritePlan.stream_replacements",
                    "confidence": "IdentifiedOnly",
                    "notes": "Whole-stream byte replacement only; caller owns byte validity, downstream compatibility, and reparse."
                },
                {
                    "surface": "Experimental Sheet byte patch",
                    "method": "WritePlan.sheet_patches with experimental=true",
                    "confidence": "IdentifiedOnly",
                    "notes": "Byte-range splice only; does not accept geometry, audit, or probe JSON as semantic input."
                }
            ],
            "read_only": [
                {
                    "surface": "geometry/decoded_entities.json",
                    "reason": "No source writer contract exists for regenerating Sheet bytes from bundle geometry."
                },
                {
                    "surface": "geometry/audit_entities.json",
                    "reason": "Typed audit and inferred entities are investigation outputs, not writer instructions."
                },
                {
                    "surface": "geometry/probe_entities.json",
                    "reason": "Probe-only evidence has not passed decoded promotion gates and is never writable."
                },
                {
                    "surface": "decoded/sheets.json",
                    "reason": "Sheet decoded/audit/probe records are export views; use explicit Sheet byte patches or stream replacements for byte-level experiments."
                },
                {
                    "surface": "decoded/structure.json",
                    "reason": "DA, JSite, PSM, unknown-stream, and object-graph surfaces remain read-only unless a future writer gate proves an exact edit."
                },
                {
                    "surface": "publish/*.xml",
                    "reason": "Publish XML is MDF-backed and separate from .pid raw parsing."
                }
            ],
            "forbidden_in_phase32": [
                "writing Sheet bytes from geometry/*.json",
                "using decoded/audit/probe bundle JSON as semantic write instructions",
                "promoting probe or inferred geometry to Decoded",
                "semantic write-back for Sheet geometry, probe entities, typed audit entities, Dynamic Attributes, JSite, PSM, or publish XML",
                "compacting unknown streams",
                "using MDF-backed publish XML as raw .pid decode or write evidence",
                "regenerating CFB tree without passthrough verification"
            ]
        }),
    )?;
    write_json_pretty(
        &writer_dir.join("diff_summary.json"),
        &serde_json::json!({
            "status": "not_run",
            "reason": "Bundle export does not perform a writer round-trip or package diff.",
            "recommended_verification": [
                "pid_inspect input.pid --round-trip out.pid --verify",
                "pid_writer_validate input.pid --json",
                "pid_writer_validate input.pid --apply-plan plan.json --out out.pid --keep --json",
                "pid_inspect input.pid --diff out.pid"
            ]
        }),
    )?;
    Ok(())
}

fn write_publish_status(
    plan: &ExportBundlePlan,
    publish_dir: &std::path::Path,
) -> Result<(), crate::PidError> {
    let Some(publish) = plan.publish.as_ref() else {
        return Ok(());
    };
    fs::create_dir_all(publish_dir)?;
    let input = {
        let kind = publish_input_kind(&publish.input_path);
        input_identity(&publish.input_path, kind).ok()
    };
    let reference_reason = if publish.include_diff {
        "publish XML generation is not implemented in bundle export yet; comparison is deferred"
    } else {
        "no publish reference comparison requested"
    };
    let status = ExportBundlePublishStatus {
        requested_drawing_uid: publish.drawing_uid.clone(),
        plant: publish.plant.clone(),
        style: publish.style.clone(),
        input,
        data_xml_written: false,
        meta_xml_written: false,
        reference_comparison: ExportBundlePublishComparisonStatus {
            state: "not_run".to_string(),
            reason: Some(reference_reason.to_string()),
        },
        status: ExportBundlePublishRunStatus {
            state: "skipped".to_string(),
            reason: "publish XML generation is not implemented in bundle export yet".to_string(),
        },
    };
    write_json_pretty(&publish_dir.join("status.json"), &status)
}

fn write_publish_diff_if_requested(
    publish: &ExportBundlePublishPlan,
    publish_dir: &std::path::Path,
    data_xml: &str,
) -> Result<ExportBundlePublishComparisonStatus, crate::PidError> {
    let Some(reference_path) = publish.reference_data_xml.as_ref() else {
        if publish.include_diff {
            return Ok(ExportBundlePublishComparisonStatus {
                state: "not_run".to_string(),
                reason: Some("reference data XML was not supplied".to_string()),
            });
        }
        return Ok(ExportBundlePublishComparisonStatus {
            state: "not_requested".to_string(),
            reason: None,
        });
    };

    let reference_xml = fs::read_to_string(reference_path)?;
    let tag_report = crate::publish::diff_publish_xml(data_xml, &reference_xml);
    let rel_report = crate::publish::diff_rel_defuids(data_xml, &reference_xml);
    let clean = tag_report.is_clean() && rel_report.is_clean();
    let artifact = ExportBundlePublishDiffArtifact {
        reference_path: reference_path.display().to_string(),
        clean,
        pid_tags: ExportBundlePublishDiffSummary {
            generated_total: tag_report.generated_total,
            reference_total: tag_report.reference_total,
            matching: tag_report.matching,
            missing_from_generated: tag_report.missing_from_generated,
            extra_in_generated: tag_report.extra_in_generated,
            count_deltas: tag_report.count_deltas,
        },
        rel_defuids: ExportBundlePublishDiffSummary {
            generated_total: rel_report.generated_total,
            reference_total: rel_report.reference_total,
            matching: rel_report.matching,
            missing_from_generated: rel_report.missing_from_generated,
            extra_in_generated: rel_report.extra_in_generated,
            count_deltas: rel_report.count_deltas,
        },
    };
    write_json_pretty(&publish_dir.join("publish_diff.json"), &artifact)?;

    Ok(ExportBundlePublishComparisonStatus {
        state: if clean { "clean" } else { "findings" }.to_string(),
        reason: Some(format!(
            "compared generated publish/data.xml against {}",
            reference_path.display()
        )),
    })
}

fn open_publish_input_as_sqlite(
    path: &std::path::Path,
) -> Result<rusqlite::Connection, crate::PidError> {
    if publish_input_kind(path) == "mdf" {
        crate::publish::open_mdf_as_sqlite(path)
            .map_err(|err| bundle_publish_error(format!("open MDF {}: {err}", path.display())))
    } else {
        crate::publish::sqlite_load::open_readonly(path)
            .map_err(|err| bundle_publish_error(format!("open SQLite {}: {err}", path.display())))
    }
}

fn parse_publish_style(
    style: Option<&str>,
) -> Result<crate::publish::PublishStyle, crate::PidError> {
    let Some(style) = style else {
        return Ok(crate::publish::PublishStyle::A01);
    };
    match style.to_ascii_lowercase().as_str() {
        "a01" => Ok(crate::publish::PublishStyle::A01),
        "dwg" => Ok(crate::publish::PublishStyle::Dwg),
        other => Err(bundle_publish_error(format!(
            "publish style accepts `a01` or `dwg`; got `{other}`"
        ))),
    }
}

fn publish_style_label(style: crate::publish::PublishStyle) -> &'static str {
    match style {
        crate::publish::PublishStyle::A01 => "a01",
        crate::publish::PublishStyle::Dwg => "dwg",
    }
}

fn bundle_publish_error(message: impl Into<String>) -> crate::PidError {
    crate::PidError::ParseFailure {
        context: "export_bundle publish".into(),
        message: message.into(),
    }
}

fn minimal_confidence_ledger(plan: &ExportBundlePlan) -> ExportBundleConfidenceLedger {
    let mut entries = vec![
        ExportBundleConfidenceEntry {
            source_path: "/".to_string(),
            bundle_path: "raw/streams.json".to_string(),
            family: "CFBF stream inventory".to_string(),
            confidence: "Decoded".to_string(),
            evidence: vec![
                "CFBF tree traversal".to_string(),
                "PidPackage raw stream index".to_string(),
                "reversible escaped filenames".to_string(),
            ],
            blockers: vec![],
            summary: "CFB stream inventory with reversible raw-stream filenames".to_string(),
        },
        ExportBundleConfidenceEntry {
            source_path: "/".to_string(),
            bundle_path: "decoded/document.json".to_string(),
            family: "PidDocument aggregate".to_string(),
            confidence: "IdentifiedOnly".to_string(),
            evidence: vec![
                "aggregate parser output".to_string(),
                "per-field confidence remains governed by the format atlas".to_string(),
            ],
            blockers: vec![
                "aggregate file intentionally contains decoded, audit, probe, identified-only, and unknown surfaces"
                    .to_string(),
            ],
            summary: "Full PidDocument aggregate; do not treat every child field as Decoded"
                .to_string(),
        },
        ExportBundleConfidenceEntry {
            source_path: "audit/confidence_ledger.json".to_string(),
            bundle_path: "audit/confidence_ledger.json".to_string(),
            family: "bundle confidence ledger".to_string(),
            confidence: "Decoded".to_string(),
            evidence: vec![
                "generated from ExportBundlePlan".to_string(),
                "canonical confidence vocabulary".to_string(),
            ],
            blockers: vec![],
            summary: "Bundle-level confidence ledger".to_string(),
        },
    ];
    if plan.include_decoded_split_views {
        entries.push(ExportBundleConfidenceEntry {
            source_path: "/decoded split views".to_string(),
            bundle_path: "decoded/*.json".to_string(),
            family: "PidDocument split projections".to_string(),
            confidence: "IdentifiedOnly".to_string(),
            evidence: vec![
                "copied from PidDocument fields without confidence promotion".to_string(),
                "atlas row for each underlying parser/model surface".to_string(),
            ],
            blockers: vec![
                "split files may contain lower-confidence child fields and must be read with atlas rows"
                    .to_string(),
            ],
            summary: "Optional split views copied from PidDocument fields without promotion"
                .to_string(),
        });
    }
    if plan.include_geometry {
        entries.extend([
            ExportBundleConfidenceEntry {
                source_path: "/Sheet*".to_string(),
                bundle_path: "geometry/normalized_geometry.json".to_string(),
                family: "normalized geometry aggregate".to_string(),
                confidence: "IdentifiedOnly".to_string(),
                evidence: vec![
                    "geometry projection preserves per-entity confidence".to_string(),
                    "decoded/audit/probe split files carry canonical classes".to_string(),
                ],
                blockers: vec![
                    "page transform, units, and some text/symbol placement semantics remain unavailable"
                        .to_string(),
                ],
                summary: "Normalized geometry projection preserving per-entity confidence"
                    .to_string(),
            },
            ExportBundleConfidenceEntry {
                source_path: "/Sheet* decoded PSM records".to_string(),
                bundle_path: "geometry/decoded_entities.json".to_string(),
                family:
                    "GLine2d, igLine2d, igLineString2d, igPoint2d, igTextBox, igSymbol2d"
                        .to_string(),
                confidence: "Decoded".to_string(),
                evidence: vec![
                    "typed Sheet PSM decoders".to_string(),
                    "cross-fixture ratchets".to_string(),
                    "byte-range provenance".to_string(),
                    "panic-safety coverage".to_string(),
                ],
                blockers: vec![
                    "semantic Sheet writer support is not proven from decoded geometry".to_string(),
                ],
                summary:
                    "Normalized geometry entities whose record layout and semantics are decoded"
                        .to_string(),
            },
            ExportBundleConfidenceEntry {
                source_path: "/Sheet* inferred and audit records".to_string(),
                bundle_path: "geometry/audit_entities.json".to_string(),
                family: "inferred geometry and typed audit records".to_string(),
                confidence: "TypedAudit".to_string(),
                evidence: vec![
                    "stable audit DTOs or inferred entities with provenance".to_string(),
                    "kept separate from decoded geometry".to_string(),
                ],
                blockers: vec![
                    "semantic field names, page transform, or native writer evidence remain missing"
                        .to_string(),
                ],
                summary:
                    "Inferred geometry entities with provenance, separated from decoded geometry"
                        .to_string(),
            },
            ExportBundleConfidenceEntry {
                source_path: "/Sheet* probe windows".to_string(),
                bundle_path: "geometry/probe_entities.json".to_string(),
                family: "probe geometry evidence".to_string(),
                confidence: "Probe".to_string(),
                evidence: vec![
                    "heuristic text, coordinate, or shape probes".to_string(),
                    "investigation-only output".to_string(),
                ],
                blockers: vec![
                    "probe evidence has not passed decoded promotion gates".to_string(),
                ],
                summary: "Probe-only geometry evidence, not renderable by default".to_string(),
            },
        ]);
    }
    if plan.include_writer_guidance {
        entries.push(ExportBundleConfidenceEntry {
            source_path: "writer policy".to_string(),
            bundle_path: "writer/*.json".to_string(),
            family: "writer guidance".to_string(),
            confidence: "Decoded".to_string(),
            evidence: vec![
                "explicit writer policy file".to_string(),
                "passthrough-first WritePlan boundary".to_string(),
            ],
            blockers: vec![
                "read confidence does not grant writer support for geometry, DA, JSite, or PSM semantics"
                    .to_string(),
            ],
            summary:
                "Writer guidance describing supported edit surfaces and read-only bundle views"
                    .to_string(),
        });
    }
    if plan.publish.is_some() {
        entries.push(ExportBundleConfidenceEntry {
            source_path: "MDF publish input".to_string(),
            bundle_path: "publish/status.json".to_string(),
            family: "MDF-backed publish status".to_string(),
            confidence: "Decoded".to_string(),
            evidence: vec![
                "explicit ExportBundlePublishPlan".to_string(),
                "publish input identity recorded separately from .pid source".to_string(),
            ],
            blockers: vec![
                "publish XML is adjunct MDF evidence, not raw .pid decode evidence".to_string(),
            ],
            summary: "Publish opt-in status; XML generation remains MDF-backed and deferred"
                .to_string(),
        });
    }
    if plan.include_raw_stream_bytes {
        entries.push(ExportBundleConfidenceEntry {
            source_path: "/raw CFB streams".to_string(),
            bundle_path: "raw/streams/*.bin".to_string(),
            family: "opt-in raw stream bytes".to_string(),
            confidence: "IdentifiedOnly".to_string(),
            evidence: vec![
                "raw byte passthrough".to_string(),
                "stream identity listed in raw/streams.json".to_string(),
            ],
            blockers: vec![
                "raw bytes are not decoded by emitting them".to_string(),
                "unknown stream semantics remain Unknown until an atlas row promotes them"
                    .to_string(),
            ],
            summary: "Opt-in raw CFB stream byte dumps".to_string(),
        });
    }
    ExportBundleConfidenceLedger { entries }
}

fn write_json_pretty<T: Serialize>(
    path: &std::path::Path,
    value: &T,
) -> Result<(), crate::PidError> {
    let json = serde_json::to_vec_pretty(value).map_err(|e| crate::PidError::ParseFailure {
        context: format!("serialize bundle JSON: {}", path.display()),
        message: e.to_string(),
    })?;
    let mut file = fs::File::create(path)?;
    file.write_all(&json)?;
    file.write_all(b"\n")?;
    Ok(())
}

fn escaped_stream_filename(path: &str) -> Result<String, crate::PidError> {
    let bytes = path.as_bytes();
    let mut escaped = String::with_capacity(bytes.len() * 2 + 4);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut escaped, "{byte:02x}").map_err(|e| crate::PidError::ParseFailure {
            context: "escape stream filename".into(),
            message: e.to_string(),
        })?;
    }
    escaped.push_str(".bin");
    Ok(escaped)
}

impl ExportBundlePublishPlan {
    /// Create a publish subtree plan for an `MDF` / legacy `SQLite` input.
    pub fn new(input_path: impl Into<PathBuf>) -> Self {
        Self {
            input_path: input_path.into(),
            drawing_uid: None,
            plant: None,
            style: None,
            include_diff: false,
            reference_data_xml: None,
        }
    }

    /// Set the drawing UID used by the publish pipeline.
    pub fn with_drawing_uid(mut self, drawing_uid: impl Into<String>) -> Self {
        self.drawing_uid = Some(drawing_uid.into());
        self
    }

    /// Set the plant name used by `_Data.xml` / `_Meta.xml` generation.
    pub fn with_plant(mut self, plant: impl Into<String>) -> Self {
        self.plant = Some(plant.into());
        self
    }

    /// Set the publish style (`a01` or `dwg`) used by the publish pipeline.
    pub fn with_style(mut self, style: impl Into<String>) -> Self {
        self.style = Some(style.into());
        self
    }

    /// Enable optional publish diff artifacts for the future exporter.
    pub fn with_diff(mut self) -> Self {
        self.include_diff = true;
        self
    }

    /// Set a reference `_Data.xml` path and enable publish diff output.
    pub fn with_reference_data_xml(mut self, reference_data_xml: impl Into<PathBuf>) -> Self {
        self.reference_data_xml = Some(reference_data_xml.into());
        self.include_diff = true;
        self
    }
}

fn default_bundle_schema_version() -> u32 {
    BUNDLE_SCHEMA_VERSION
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PidDocument;
    use crate::package::{PidPackage, RawStream};
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pid-parse-export-bundle-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        path
    }

    fn pkg_with_streams(streams: &[(&str, &[u8])]) -> PidPackage {
        let mut map = BTreeMap::new();
        for (path, data) in streams {
            map.insert(
                (*path).to_string(),
                RawStream {
                    path: (*path).to_string(),
                    data: (*data).to_vec(),
                    modified: false,
                },
            );
        }
        PidPackage::new(None, map, PidDocument::default())
    }

    fn pkg_with_source(source_path: PathBuf, streams: &[(&str, &[u8])]) -> PidPackage {
        let mut pkg = pkg_with_streams(streams);
        pkg.source_path = Some(source_path);
        pkg
    }

    fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> T {
        let bytes = std::fs::read(path).expect("read json");
        serde_json::from_slice(&bytes).expect("parse json")
    }

    #[test]
    fn default_plan_matches_safe_bundle_defaults() {
        let plan = ExportBundlePlan::default();

        assert_eq!(plan.bundle_schema_version, 1);
        assert!(!plan.writes_raw_stream_bytes());
        assert!(plan.include_decoded_split_views);
        assert!(plan.include_geometry);
        assert!(plan.include_audit);
        assert!(plan.include_writer_guidance);
        assert!(!plan.writes_publish_outputs());
    }

    #[test]
    fn minimal_plan_keeps_raw_and_publish_opt_in() {
        let plan = ExportBundlePlan::minimal();

        assert_eq!(plan.bundle_schema_version, 1);
        assert!(!plan.include_raw_stream_bytes);
        assert!(!plan.include_decoded_split_views);
        assert!(!plan.include_geometry);
        assert!(plan.include_audit);
        assert!(!plan.include_writer_guidance);
        assert!(plan.publish.is_none());
    }

    #[test]
    fn raw_stream_bytes_are_explicit_opt_in() {
        let plan = ExportBundlePlan::default().with_raw_stream_bytes();

        assert!(plan.writes_raw_stream_bytes());
        assert!(!ExportBundlePlan::default().writes_raw_stream_bytes());
    }

    #[test]
    fn publish_subtree_is_explicit_opt_in() {
        let publish = ExportBundlePublishPlan::new("Export.mdf")
            .with_drawing_uid("DRAWING-UID")
            .with_plant("TEST02")
            .with_diff();
        let plan = ExportBundlePlan::default().with_publish(publish);

        let publish = plan.publish.as_ref().expect("publish plan");
        assert!(plan.writes_publish_outputs());
        assert_eq!(publish.input_path, PathBuf::from("Export.mdf"));
        assert_eq!(publish.drawing_uid.as_deref(), Some("DRAWING-UID"));
        assert_eq!(publish.plant.as_deref(), Some("TEST02"));
        assert!(publish.include_diff);
    }

    #[test]
    fn missing_json_fields_use_safe_defaults() {
        let plan = ExportBundlePlan::from_json("{}").expect("parse default plan");

        assert_eq!(plan, ExportBundlePlan::default());
    }

    #[test]
    fn json_round_trip_preserves_publish_and_raw_options() {
        let original = ExportBundlePlan::minimal()
            .with_raw_stream_bytes()
            .with_publish(ExportBundlePublishPlan::new("Export.mdf").with_plant("TEST02"));

        let json = original.to_json().expect("serialize");
        let parsed = ExportBundlePlan::from_json(&json).expect("parse");

        assert_eq!(parsed, original);
        assert!(parsed.writes_raw_stream_bytes());
        assert!(parsed.writes_publish_outputs());
    }

    #[test]
    fn export_bundle_default_plan_writes_split_views_without_raw_bytes() {
        let package = pkg_with_streams(&[
            ("/TaggedTxtData/Drawing", b"<Drawing/>"),
            ("/JSite0/\x01Ole", &[0xAA, 0xBB, 0xCC]),
        ]);
        let out = tmp_dir("default-plan");

        export_bundle(&package, &ExportBundlePlan::default(), &out).expect("export");

        assert!(out.join("manifest.json").is_file());
        assert!(out.join("raw/streams.json").is_file());
        assert!(out.join("decoded/document.json").is_file());
        assert!(out.join("decoded/metadata.json").is_file());
        assert!(out.join("decoded/psm_tables.json").is_file());
        assert!(out.join("decoded/sheets.json").is_file());
        assert!(out.join("decoded/structure.json").is_file());
        assert!(out.join("decoded/import_view.json").is_file());
        assert!(out.join("geometry/normalized_geometry.json").is_file());
        assert!(out.join("geometry/decoded_entities.json").is_file());
        assert!(out.join("geometry/audit_entities.json").is_file());
        assert!(out.join("geometry/probe_entities.json").is_file());
        assert!(out.join("writer/round_trip_plan.json").is_file());
        assert!(out.join("writer/diff_summary.json").is_file());
        assert!(out.join("audit/confidence_ledger.json").is_file());
        assert!(!out.join("raw/streams").exists());

        let manifest: ExportBundleManifest = read_json(&out.join("manifest.json"));
        assert_eq!(manifest.bundle_schema_version, 1);
        assert_eq!(manifest.counts.streams, 2);
        assert_eq!(manifest.counts.decoded_geometry_entities, 0);
        assert_eq!(manifest.counts.audit_geometry_entities, 0);
        assert_eq!(manifest.counts.probe_geometry_entities, 0);
        assert!(!manifest.features.raw_stream_bytes);
        assert!(manifest.features.decoded_split_views);
        assert!(manifest.features.geometry);
        assert!(manifest.features.writer_guidance);

        let index: ExportBundleStreamIndex = read_json(&out.join("raw/streams.json"));
        assert_eq!(index.streams.len(), 2);
        assert!(index.streams.iter().all(|stream| !stream.raw_bytes_written));
        assert!(index
            .streams
            .iter()
            .any(|stream| stream.escaped_filename == "2f4a53697465302f014f6c65.bin"));

        let ledger: ExportBundleConfidenceLedger =
            read_json(&out.join("audit/confidence_ledger.json"));
        assert!(ledger.entries.iter().all(|entry| matches!(
            entry.confidence.as_str(),
            "Decoded" | "TypedAudit" | "Probe" | "IdentifiedOnly" | "Unknown"
        )));
        assert!(ledger.entries.iter().all(|entry| {
            !entry.source_path.is_empty() && !entry.family.is_empty() && !entry.evidence.is_empty()
        }));
        assert!(ledger
            .entries
            .iter()
            .any(|entry| entry.bundle_path == "decoded/document.json"));
        assert!(ledger.entries.iter().any(|entry| {
            entry.bundle_path == "decoded/document.json"
                && entry.confidence == "IdentifiedOnly"
                && entry
                    .blockers
                    .iter()
                    .any(|blocker| blocker.contains("aggregate"))
        }));
        assert!(ledger
            .entries
            .iter()
            .any(|entry| entry.bundle_path == "decoded/*.json"));
        assert!(ledger
            .entries
            .iter()
            .any(|entry| entry.bundle_path == "geometry/normalized_geometry.json"));
        assert!(ledger
            .entries
            .iter()
            .any(|entry| entry.bundle_path == "writer/*.json"));

        let writer_plan: serde_json::Value = read_json(&out.join("writer/round_trip_plan.json"));
        assert_eq!(
            writer_plan["default_write_plan"]["sheet_patches"],
            serde_json::json!([])
        );
        let read_only = writer_plan["read_only"]
            .as_array()
            .expect("read_only array");
        for expected in [
            "geometry/decoded_entities.json",
            "geometry/audit_entities.json",
            "geometry/probe_entities.json",
        ] {
            assert!(
                read_only
                    .iter()
                    .any(|surface| surface["surface"] == expected),
                "missing read-only surface {expected}: {read_only:?}"
            );
        }
        let editable = writer_plan["editable"].as_array().expect("editable array");
        for expected in [
            "TaggedTxtData/Drawing XML tag",
            "TaggedTxtData/General XML tag",
            "OLE SummaryInformation / DocumentSummaryInformation string properties",
            "Verbatim CFB stream replacement",
            "Experimental Sheet byte patch",
        ] {
            assert!(
                editable
                    .iter()
                    .any(|surface| surface["surface"] == expected),
                "missing writer-safe surface {expected}: {editable:?}"
            );
        }
        assert!(writer_plan["forbidden_in_phase32"]
            .as_array()
            .expect("forbidden array")
            .iter()
            .any(|operation| operation
                .as_str()
                .is_some_and(|text| text.contains("semantic write-back"))));
        let diff_summary: serde_json::Value = read_json(&out.join("writer/diff_summary.json"));
        assert_eq!(diff_summary["status"], "not_run");
        assert!(diff_summary["recommended_verification"]
            .as_array()
            .expect("recommended verification")
            .iter()
            .any(|command| command
                .as_str()
                .is_some_and(|text| text.contains("pid_writer_validate"))));
        let import_view: serde_json::Value = read_json(&out.join("decoded/import_view.json"));
        assert_eq!(import_view["title"], "Smart P&ID Import");
        assert!(import_view["objects"].as_array().is_some());

        std::fs::remove_dir_all(out).expect("cleanup");
    }

    #[test]
    fn export_bundle_minimal_plan_skips_split_views_and_geometry() {
        let package = pkg_with_streams(&[("/FlatStream", &[0x11, 0x22, 0x33])]);
        let out = tmp_dir("minimal-plan");

        export_bundle(&package, &ExportBundlePlan::minimal(), &out).expect("export");

        assert!(out.join("manifest.json").is_file());
        assert!(out.join("raw/streams.json").is_file());
        assert!(out.join("decoded/document.json").is_file());
        assert!(out.join("audit/confidence_ledger.json").is_file());
        assert!(!out.join("decoded/metadata.json").exists());
        assert!(!out.join("decoded/import_view.json").exists());
        assert!(!out.join("geometry").exists());
        assert!(!out.join("writer").exists());

        let manifest: ExportBundleManifest = read_json(&out.join("manifest.json"));
        assert!(!manifest.features.decoded_split_views);
        assert!(!manifest.features.geometry);
        assert!(!manifest.features.writer_guidance);
        assert_eq!(manifest.counts.decoded_geometry_entities, 0);
        assert_eq!(manifest.counts.audit_geometry_entities, 0);
        assert_eq!(manifest.counts.probe_geometry_entities, 0);

        let ledger: ExportBundleConfidenceLedger =
            read_json(&out.join("audit/confidence_ledger.json"));
        assert!(!ledger
            .entries
            .iter()
            .any(|entry| entry.bundle_path == "decoded/*.json"));
        assert!(!ledger
            .entries
            .iter()
            .any(|entry| entry.bundle_path.starts_with("geometry/")));
        assert!(!ledger
            .entries
            .iter()
            .any(|entry| entry.bundle_path.starts_with("writer/")));

        std::fs::remove_dir_all(out).expect("cleanup");
    }

    #[test]
    fn export_bundle_raw_stream_bytes_are_opt_in() {
        let package = pkg_with_streams(&[("/FlatStream", &[0x11, 0x22, 0x33])]);
        let out = tmp_dir("raw");
        let plan = ExportBundlePlan::minimal().with_raw_stream_bytes();

        export_bundle(&package, &plan, &out).expect("export");

        let index: ExportBundleStreamIndex = read_json(&out.join("raw/streams.json"));
        let stream = &index.streams[0];
        assert!(stream.raw_bytes_written);
        assert_eq!(stream.escaped_filename, "2f466c617453747265616d.bin");
        assert_eq!(
            std::fs::read(out.join("raw/streams").join(&stream.escaped_filename))
                .expect("raw bytes"),
            vec![0x11, 0x22, 0x33]
        );

        let ledger: ExportBundleConfidenceLedger =
            read_json(&out.join("audit/confidence_ledger.json"));
        assert!(ledger.entries.iter().any(|entry| {
            entry.bundle_path == "raw/streams/*.bin"
                && entry.confidence == "IdentifiedOnly"
                && entry.family == "opt-in raw stream bytes"
        }));

        std::fs::remove_dir_all(out).expect("cleanup");
    }

    #[test]
    fn manifest_records_pid_and_publish_input_identities() {
        let root = tmp_dir("input-identities");
        std::fs::create_dir_all(&root).expect("root dir");
        let pid_path = root.join("drawing.pid");
        let publish_path = root.join("Export.mdf");
        std::fs::write(&pid_path, b"pid-source").expect("pid source");
        std::fs::write(&publish_path, b"publish-source").expect("publish source");
        let package = pkg_with_source(pid_path.clone(), &[("/FlatStream", &[0x11])]);
        let plan = ExportBundlePlan::minimal()
            .with_publish(ExportBundlePublishPlan::new(publish_path.clone()));
        let out = root.join("bundle");

        export_bundle(&package, &plan, &out).expect("export");

        let manifest: ExportBundleManifest = read_json(&out.join("manifest.json"));
        let pid = manifest.inputs.pid.expect("pid identity");
        assert_eq!(pid.path, pid_path.display().to_string());
        assert_eq!(pid.size_bytes, 10);
        assert_eq!(pid.kind, "pid");
        assert_eq!(
            pid.sha256,
            "e02f7de2295a01688b6b5dfe40aa6a0680a61894023b6efd19606dc33819fc27"
        );
        let publish = manifest.inputs.publish_mdf.expect("publish identity");
        assert_eq!(publish.path, publish_path.display().to_string());
        assert_eq!(publish.size_bytes, 14);
        assert_eq!(publish.kind, "mdf");
        assert_eq!(
            publish.sha256,
            "811e3762b31fdc8f03c12d328d6df11c6eac4a5420b8689cf35ccaa409a089a7"
        );

        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn publish_status_json_records_deferred_xml_state() {
        let root = tmp_dir("publish-status");
        std::fs::create_dir_all(&root).expect("root dir");
        let publish_path = root.join("Export.mdf");
        std::fs::write(&publish_path, b"publish-source").expect("publish source");
        let package = pkg_with_streams(&[("/FlatStream", &[0x11])]);
        let plan = ExportBundlePlan::minimal().with_publish(
            ExportBundlePublishPlan::new(publish_path.clone())
                .with_drawing_uid("DRAWING-UID")
                .with_plant("TEST02")
                .with_style("a01")
                .with_diff(),
        );
        let out = root.join("bundle");

        export_bundle(&package, &plan, &out).expect("export");

        let status: ExportBundlePublishStatus = read_json(&out.join("publish/status.json"));
        assert_eq!(status.requested_drawing_uid.as_deref(), Some("DRAWING-UID"));
        assert_eq!(status.plant.as_deref(), Some("TEST02"));
        assert_eq!(status.style.as_deref(), Some("a01"));
        assert_eq!(status.input.as_ref().expect("input").kind, "mdf");
        assert!(!status.data_xml_written);
        assert!(!status.meta_xml_written);
        assert_eq!(status.reference_comparison.state, "not_run");
        assert_eq!(status.status.state, "skipped");
        assert!(status
            .status
            .reason
            .contains("publish XML generation is not implemented"));
        assert!(!out.join("publish/data.xml").exists());
        assert!(!out.join("publish/meta.xml").exists());

        let ledger: ExportBundleConfidenceLedger =
            read_json(&out.join("audit/confidence_ledger.json"));
        assert!(ledger
            .entries
            .iter()
            .any(|entry| entry.bundle_path == "publish/status.json"));

        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn export_bundle_publish_xml_requires_drawing_uid() {
        let root = tmp_dir("publish-missing-drawing");
        let publish = ExportBundlePublishPlan::new(root.join("missing.sqlite"));

        let err = export_bundle_publish_xml(&publish, root.join("publish"))
            .expect_err("drawing UID should be required before opening input");

        match err {
            crate::PidError::ParseFailure { context, message } => {
                assert_eq!(context, "export_bundle publish");
                assert!(message.contains("drawing UID is required"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn export_bundle_publish_xml_writes_data_meta_and_success_status() {
        let publish_path =
            PathBuf::from("test-file/backup-test/TEST02_p/extracted/Export_v2.sqlite");
        if !publish_path.exists() {
            eprintln!("skipping: fixture {} not found", publish_path.display());
            return;
        }
        let root = tmp_dir("publish-xml");
        let publish_dir = root.join("publish");
        let publish = ExportBundlePublishPlan::new(publish_path.clone())
            .with_drawing_uid("D9635C3C898840D1990B7E8BEE1D55DA")
            .with_plant("TEST02")
            .with_style("A01");

        let status = export_bundle_publish_xml(&publish, &publish_dir).expect("publish xml");

        assert!(publish_dir.join("data.xml").exists());
        assert!(publish_dir.join("meta.xml").exists());
        let data_xml = std::fs::read_to_string(publish_dir.join("data.xml")).expect("data xml");
        assert!(data_xml.contains("<PIDDrawing>"));
        assert!(data_xml.contains(r#"Plant="TEST02""#));
        let meta_xml = std::fs::read_to_string(publish_dir.join("meta.xml")).expect("meta xml");
        assert!(meta_xml.contains(r#"CompSchema="DocVersioningComponent""#));
        assert_eq!(
            status.requested_drawing_uid.as_deref(),
            publish.drawing_uid.as_deref()
        );
        assert_eq!(status.plant.as_deref(), Some("TEST02"));
        assert_eq!(status.style.as_deref(), Some("a01"));
        assert_eq!(status.input.as_ref().expect("input").kind, "sqlite");
        assert!(status.data_xml_written);
        assert!(status.meta_xml_written);
        assert_eq!(status.reference_comparison.state, "not_requested");
        assert_eq!(status.status.state, "written");

        let status_file: ExportBundlePublishStatus = read_json(&publish_dir.join("status.json"));
        assert_eq!(status_file, status);

        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn export_bundle_publish_xml_writes_reference_diff_artifact() {
        let publish_path =
            PathBuf::from("test-file/backup-test/TEST02_p/extracted/Export_v2.sqlite");
        let reference_path = PathBuf::from("test-file/export-test/publish-data/A01/A01_Data.xml");
        if !publish_path.exists() || !reference_path.exists() {
            eprintln!(
                "skipping: fixture {} or {} not found",
                publish_path.display(),
                reference_path.display()
            );
            return;
        }
        let root = tmp_dir("publish-diff");
        let publish_dir = root.join("publish");
        let publish = ExportBundlePublishPlan::new(publish_path)
            .with_drawing_uid("D9635C3C898840D1990B7E8BEE1D55DA")
            .with_plant("TEST02")
            .with_reference_data_xml(reference_path.clone());

        let status = export_bundle_publish_xml(&publish, &publish_dir).expect("publish xml");

        assert!(publish_dir.join("publish_diff.json").exists());
        let diff: serde_json::Value = read_json(&publish_dir.join("publish_diff.json"));
        assert_eq!(diff["reference_path"], reference_path.display().to_string());
        assert!(diff["pid_tags"]["generated_total"].as_u64().is_some());
        assert!(diff["rel_defuids"]["reference_total"].as_u64().is_some());
        assert!(matches!(
            status.reference_comparison.state.as_str(),
            "clean" | "findings"
        ));

        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn publish_input_kind_recognizes_legacy_sqlite_extensions() {
        assert_eq!(
            publish_input_kind(std::path::Path::new("Export.mdf")),
            "mdf"
        );
        assert_eq!(
            publish_input_kind(std::path::Path::new("Export_v2.sqlite")),
            "sqlite"
        );
        assert_eq!(
            publish_input_kind(std::path::Path::new("mirror.DB")),
            "sqlite"
        );
    }
}
