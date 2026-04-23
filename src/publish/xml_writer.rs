//! Publish-Data XML writer — DTO → SmartPlant-compatible XML.
//!
//! Stage-1 A3: emit the structural skeleton of a Publish Data
//! document — `Container` root, the drawing metadata node, every
//! representation, and a DefUID-classified relationship list. The
//! output resembles the SPPID reference format closely enough that
//! downstream validators should accept it, but business-specific
//! interface nodes (`<PIDProcessVessel>` etc.) are NOT yet
//! populated — those land in A4 once the loader pulls
//! T_Equipment / T_Vessel / T_Nozzle / T_PipeRun.
//!
//! ## Format guarantees
//!
//! * UTF-8 output, indented for human inspection (two-space
//!   indent, trailing newline). SmartPlant accepts compact and
//!   indented forms alike.
//! * Strings go through XML entity escaping so names / paths with
//!   `&`, `<`, quotes, CR/LF round-trip cleanly.
//! * Unknown optional fields render as empty attribute values
//!   (`Description=""`) — matches how SPPID itself emits
//!   blank-but-present attributes.

use std::collections::HashMap;
use std::fmt::Write;

use super::model::{
    PublishDrawing, PublishError, PublishObject, PublishRelationship, PublishRepresentation,
};

/// Software-version / schema-version / tooling constants that the
/// SmartPlant reference implementation stamps onto every Publish
/// Data `<Container>`. Hard-coded here because they are not
/// carried by any backup table — the values are part of SmartPlant
/// 2014 R1's output contract.
const CONTAINER_COMP_SCHEMA: &str = "PIDComponent";
/// `_Meta.xml` switches the schema marker to advertise it as the
/// document-versioning sibling of the main data document. Reference
/// SPPID exports keep `Scope="Data"` for both files, so only the
/// schema label changes.
const CONTAINER_META_COMP_SCHEMA: &str = "DocVersioningComponent";
const CONTAINER_SCOPE: &str = "Data";
const CONTAINER_SOFTWARE_VERSION: &str = "10.00.31.0023";
const CONTAINER_SCHEMA_VERSION: &str = "04.02.17.01";
const CONTAINER_TOOL_ID: &str = "SMARTPLANTPID";
const CONTAINER_TOOL_SIGNATURE: &str = "AAAD";
const CONTAINER_SDECIMAL: &str = ".";

/// Default `<IDocumentVersion DocRevision="..."/>` attribute when a
/// drawing has not been versioned in the source backup. Reference
/// exports ship `"0"` for unrevised drawings.
const META_DEFAULT_DOC_REVISION: &str = "0";
/// Default `<IDocumentVersion DocVersion="..."/>` attribute. Same
/// rationale — reference exports use `"1"` for first-time emits.
const META_DEFAULT_DOC_VERSION: &str = "1";

/// Number of `<PIDSignalPort>` children SmartPlant always derives
/// from a single InstrFunction / Instrument row. Pinned at 8
/// because the DWG-0202GP06-01 reference fixture emits exactly
/// `<instr>.1` through `<instr>.8` for each of its two
/// InstrFunction objects (16 ports total = the A15 backlog
/// count). Future fixtures that exhibit a different cardinality
/// will let this constant become a per-object derivation rule.
const INSTR_DERIVED_SIGNAL_PORT_COUNT: u8 = 8;

/// Emit a full `_Data.xml` document for the given drawing into a
/// string buffer. `plant_name` is a user-supplied value (e.g. the
/// SmartPlant plant identifier from MSCI / Manifest); stage-1
/// exposes it as an input because SPPID encodes it in the
/// `<Container Plant="...">` attribute.
pub fn write_data_xml(drawing: &PublishDrawing, plant_name: &str) -> Result<String, PublishError> {
    let mut buf = String::with_capacity(4096);
    writeln!(buf, r#"<?xml version ="1.0" encoding="UTF-8"?>"#).map_err(fmt_err)?;
    write_container_open(&mut buf, drawing, plant_name)?;
    write_pid_drawing(&mut buf, drawing)?;
    write_business_objects(&mut buf, drawing)?;
    write_representations(&mut buf, drawing)?;
    write_relationships(&mut buf, drawing)?;
    writeln!(buf, " </Container>").map_err(fmt_err)?;
    Ok(buf)
}

/// Emit a `_Meta.xml` document for the given drawing. SmartPlant's
/// reference Publish-Data export ships the document-versioning
/// envelope as a sibling file alongside `<DrawingName>_Data.xml`.
/// Compared with `write_data_xml` the structural shape is fixed and
/// minimal — three nodes (DocumentVersion / DocumentRevision /
/// File) and three `<Rel>` rows wiring them together.
///
/// The meta document carries no business attributes. Its sole
/// inputs are the drawing's `drawing_uid`, `drawing_name`, and
/// (optionally) `date_created`. Inner UIDs (the ones stamped onto
/// the version / revision / file / rel nodes) are derived
/// deterministically from `drawing_uid` via [`derive_meta_uid`] so
/// successive re-emits of the same drawing produce byte-identical
/// XML — a property tests rely on heavily.
///
/// `plant_name` is reused unchanged from the data document; it
/// shows up as the `<Container Plant="...">` attribute.
pub fn write_meta_xml(drawing: &PublishDrawing, plant_name: &str) -> Result<String, PublishError> {
    let mut buf = String::with_capacity(1024);
    writeln!(buf, r#"<?xml version ="1.0" encoding="UTF-8"?>"#).map_err(fmt_err)?;
    write_meta_container_open(&mut buf, drawing, plant_name)?;

    let version_uid = derive_meta_uid(&drawing.drawing_uid, "version");
    let revision_uid = derive_meta_uid(&drawing.drawing_uid, "revision");
    let file_uid = derive_meta_uid(&drawing.drawing_uid, "file");
    let rel_versioned_uid = derive_meta_uid(&drawing.drawing_uid, "rel/versioned-doc");
    let rel_revised_uid = derive_meta_uid(&drawing.drawing_uid, "rel/revised-document");
    let rel_file_uid = derive_meta_uid(&drawing.drawing_uid, "rel/file-composition");

    write_meta_document_version(&mut buf, drawing, &version_uid)?;
    write_meta_rel(
        &mut buf,
        &rel_versioned_uid,
        &drawing.drawing_uid,
        &version_uid,
        "VersionedDoc",
    )?;
    write_meta_document_revision(&mut buf, drawing, &revision_uid)?;
    write_meta_rel(
        &mut buf,
        &rel_revised_uid,
        &revision_uid,
        &drawing.drawing_uid,
        "RevisedDocument",
    )?;
    write_meta_file(&mut buf, drawing, &file_uid)?;
    write_meta_rel(
        &mut buf,
        &rel_file_uid,
        &file_uid,
        &version_uid,
        "FileComposition",
    )?;

    writeln!(buf, "  </Container>").map_err(fmt_err)?;
    Ok(buf)
}

/// Emit every `PublishObject` as the corresponding SmartPlant XML
/// tag (`<PIDProcessVessel>` / `<PIDNozzle>` / ...). Stage-1
/// handles the four item types the TEST02 A01 fixture exercises
/// (Vessel, Nozzle, PipeRun, PipingPoint); A11 extends the
/// dispatcher to Note / ItemNote (→ `<PIDNote>`) and Instrument /
/// InstrFunction (→ `<PIDControlSystemFunction>`), modeled on the
/// DWG-0202GP06-01 reference fixture. Unknown types still fall
/// through with a generic `<PIDItem>` wrapper so the writer stays
/// total.
fn write_business_objects(buf: &mut String, drawing: &PublishDrawing) -> Result<(), PublishError> {
    for obj in &drawing.objects {
        match obj.item_type_name.as_str() {
            "Vessel" => write_process_vessel(buf, obj, drawing)?,
            "Nozzle" => write_nozzle(buf, obj, drawing)?,
            // PipeRun maps to the logical pipeline + its physical
            // connector + the connector's two derived piping ports
            // and one derived process point. SmartPlant's exporter
            // emits all five tags from a single PipeRun row in this
            // exact order; we mirror that to stay compatible with
            // the SemanticDiffReport contract.
            "PipeRun" => {
                write_pipeline(buf, obj)?;
                write_piping_connector(buf, obj)?;
                write_derived_connector_endpoints(buf, obj)?;
            }
            // PipingPoint as a top-level object is now treated as a
            // generic placeholder. T_PipingPoint rows never reach
            // here (the loader stopped injecting them in A13); this
            // arm exists for forward-compat in case a future fixture
            // surfaces a true standalone PIDPipingPort.
            "PipingPoint" => write_piping_port(buf, obj)?,
            // A11: Note / ItemNote → `<PIDNote>`. The reference
            // DWG fixture ships 11 of these — annotation labels
            // hung on the drawing canvas.
            "Note" | "ItemNote" => write_item_note(buf, obj)?,
            // A11+A16: Instrument / InstrFunction →
            // `<PIDControlSystemFunction>` plus the eight derived
            // `<PIDSignalPort>` children SmartPlant always
            // synthesizes (UIDs `<instr>.{1..8}`). The DWG fixture
            // ships 2 InstrFunction rows × 8 derived ports = 16
            // PIDSignalPort entries, matching the A15 backlog
            // count exactly.
            "Instrument" | "InstrFunction" => {
                write_control_system_function(buf, obj)?;
                write_derived_instr_signal_ports(buf, obj, INSTR_DERIVED_SIGNAL_PORT_COUNT)?;
            }
            // TODO(A12+): Exchanger / Mechanical have business
            // subtables registered in `subtables_for_item_type` but
            // no dedicated SmartPlant tag observed in the TEST02 +
            // DWG-0202GP06-01 reference fixtures. They fall through
            // to the generic placeholder until a fixture surfaces
            // their canonical XML shape.
            other => write_generic_object(buf, obj, other)?,
        }
    }
    Ok(())
}

/// Derive a human-readable description from the object's first
/// symbol-bearing representation, e.g.
/// `\Equipment\Vessels\Horizontal Drums\Horizontal Drum.sym`
/// → `"Horizontal Drum"`. Returns `None` when no `.sym` rep is
/// attached; callers fall back to whichever SPPID code column
/// they already have.
fn derive_type_description_from_symbol(
    drawing: &PublishDrawing,
    object_uid: &str,
) -> Option<String> {
    for rep in &drawing.representations {
        if rep.model_item_uid.as_deref() != Some(object_uid) {
            continue;
        }
        let Some(path) = &rep.symbol_path else {
            continue;
        };
        if path.is_empty() {
            continue;
        }
        let last = path.rsplit('\\').next().unwrap_or(path);
        let stem = last.strip_suffix(".sym").unwrap_or(last);
        if !stem.is_empty() {
            return Some(stem.to_string());
        }
    }
    None
}

/// Resolve a business-field value (e.g. `EquipmentType = "0"`) to
/// its codelist display text when the drawing's [`CodelistIndex`]
/// carries a mapping for `attribute_name`. Empty / missing values
/// short-circuit to `None` so the writer never burns a codelist
/// lookup on rows without the attribute set.
fn resolve_codelist_field(
    drawing: &PublishDrawing,
    obj: &PublishObject,
    attribute_name: &str,
) -> Option<String> {
    let raw = obj.fields.get(attribute_name)?;
    if raw.is_empty() {
        return None;
    }
    drawing
        .codelist
        .lookup_by_attribute(attribute_name, raw)
        .map(str::to_string)
}

fn write_process_vessel(
    buf: &mut String,
    obj: &PublishObject,
    drawing: &PublishDrawing,
) -> Result<(), PublishError> {
    let item_tag = obj
        .fields
        .get("ItemTag")
        .cloned()
        .unwrap_or_else(|| format_equipment_tag(obj));
    let description = obj.description.as_deref().unwrap_or("");
    // Three-tier fallback for the SmartPlant `EqTypeDescription`
    // attribute. The codelist lookup is authoritative — it is what
    // SmartPlant itself uses to render the enum — so it wins when
    // the metadata catalog ships the mapping. Drawing fixtures
    // produced without a codelist catalog fall back to parsing the
    // symbol path (`Horizontal Drum.sym` → `"Horizontal Drum"`),
    // and finally to the raw `EquipmentType` enum ID so the
    // attribute is never silently blank.
    let eq_type_description = resolve_codelist_field(drawing, obj, "EquipmentType")
        .or_else(|| derive_type_description_from_symbol(drawing, &obj.uid))
        .unwrap_or_else(|| obj.fields.get("EquipmentType").cloned().unwrap_or_default());
    writeln!(buf, "   <PIDProcessVessel>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IObject UID="{}" ItemTag="{}" Description="{}"/>"#,
        escape_attr(&obj.uid),
        escape_attr(&item_tag),
        escape_attr(description),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IPIDProcessVesselOcc/>").map_err(fmt_err)?;
    writeln!(buf, "      <IProcessVesselOcc/>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IEquipment EqTypeDescription="{}"/>"#,
        escape_attr(&eq_type_description)
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IEquipmentOcc/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPBSItem/>").map_err(fmt_err)?;
    writeln!(buf, "      <IProcessEquipment/>").map_err(fmt_err)?;
    writeln!(buf, "      <IProcessVessel/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPIDProcessVessel/>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IPIDTypical IsTypical="{}"/>"#,
        obj.is_typical.as_deref().map(map_bool).unwrap_or("False")
    )
    .map_err(fmt_err)?;
    writeln!(buf, "   </PIDProcessVessel>").map_err(fmt_err)
}

fn write_nozzle(
    buf: &mut String,
    obj: &PublishObject,
    drawing: &PublishDrawing,
) -> Result<(), PublishError> {
    let nominal_diameter = obj
        .fields
        .get("NominalDiameter")
        .cloned()
        .map(|v| format_diameter(&v))
        .unwrap_or_default();
    let piping_materials_class = obj
        .fields
        .get("PipingMaterialsClass")
        .cloned()
        .unwrap_or_default();
    // Three-tier fallback for `ProcEqpCompTypeDescription`, in
    // order of authority:
    //   1. SmartPlant codelist on T_Nozzle.NozzleType
    //      (e.g. "0" → "Flanged Nozzle")
    //   2. Symbol path stem (`Flanged Nozzle.sym` → "Flanged Nozzle")
    //   3. Hard-coded fallback "Flanged Nozzle" so the attribute is
    //      never blank — matches the SmartPlant default for the
    //      overwhelming majority of nozzle rows.
    let proc_eq_comp_description = resolve_codelist_field(drawing, obj, "NozzleType")
        .or_else(|| derive_type_description_from_symbol(drawing, &obj.uid))
        .unwrap_or_else(|| "Flanged Nozzle".to_string());
    writeln!(buf, "   <PIDNozzle>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IObject UID="{}"/>"#,
        escape_attr(&obj.uid)
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IPipingPortComposition/>").map_err(fmt_err)?;
    writeln!(buf, "      <INozzleOcc/>").map_err(fmt_err)?;
    writeln!(buf, "      <INozzle/>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IEquipmentComponent ProcEqpCompTypeDescription="{}"/>"#,
        escape_attr(&proc_eq_comp_description)
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IEquipmentComponentOcc/>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IPipeCrossSectionItem NominalDiameter="{}"/>"#,
        escape_attr(&nominal_diameter)
    )
    .map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IPipingSpecifiedItem PipingMaterialsClass="{}"/>"#,
        escape_attr(&piping_materials_class)
    )
    .map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IPIDTypical IsTypical="{}"/>"#,
        obj.is_typical.as_deref().map(map_bool).unwrap_or("False")
    )
    .map_err(fmt_err)?;
    writeln!(buf, "   </PIDNozzle>").map_err(fmt_err)
}

/// Resolve the `ItemTag` attribute for a pipeline-like object (a
/// PipeRun row that drives both `<PIDPipeline>` and
/// `<PIDPipingConnector>`). Order of preference:
///
/// 1. `obj.fields["ItemTag"]` — populated by the loader from
///    `T_PlantItem.ItemTag`. This is the canonical tag SmartPlant
///    itself stores (e.g. `"A010102102-PH"` in the TEST02 fixture)
///    and is what Publish Data XML consumers expect.
/// 2. Legacy synthesized form `PH-{seq}-{dia}-{class}` — kept as a
///    fallback so drawings without a T_PlantItem row still emit a
///    non-opaque identifier. Matches pre-A8 behaviour for anything
///    that lacks the catalog link.
/// 3. The raw `obj.uid` — last-ditch choice when neither an
///    `ItemTag` nor a `TagSequenceNo` is available; ensures the
///    attribute is never blank.
fn resolve_pipe_item_tag(obj: &PublishObject) -> String {
    if let Some(tag) = obj.fields.get("ItemTag") {
        if !tag.is_empty() {
            return tag.clone();
        }
    }
    let tag_sequence = obj.fields.get("TagSequenceNo").map(String::as_str).unwrap_or("");
    if !tag_sequence.is_empty() {
        let piping_materials_class = obj
            .fields
            .get("PipingMaterialsClass")
            .map(String::as_str)
            .unwrap_or("");
        let nominal_diameter = obj
            .fields
            .get("NominalDiameter")
            .map(|v| format_diameter(v))
            .unwrap_or_default();
        return format!(
            "PH-{tag_sequence}-{nominal_diameter}-{piping_materials_class}"
        );
    }
    obj.uid.clone()
}

fn write_pipeline(buf: &mut String, obj: &PublishObject) -> Result<(), PublishError> {
    let item_tag = resolve_pipe_item_tag(obj);
    writeln!(buf, "   <PIDPipeline>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IObject UID="{}" ItemTag="{}"/>"#,
        escape_attr(&obj.uid),
        escape_attr(&item_tag),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IPipeline/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPipingConnectorComposition/>").map_err(fmt_err)?;
    writeln!(buf, "      <IFluidSystem/>").map_err(fmt_err)?;
    writeln!(buf, "      <IExpandableThing/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPIDTypical/>").map_err(fmt_err)?;
    writeln!(buf, "   </PIDPipeline>").map_err(fmt_err)
}

fn write_piping_connector(buf: &mut String, obj: &PublishObject) -> Result<(), PublishError> {
    let tag_sequence = obj.fields.get("TagSequenceNo").cloned().unwrap_or_default();
    let piping_materials_class = obj
        .fields
        .get("PipingMaterialsClass")
        .cloned()
        .unwrap_or_default();
    let nominal_diameter = obj
        .fields
        .get("NominalDiameter")
        .cloned()
        .map(|v| format_diameter(&v))
        .unwrap_or_default();
    // The connector inherits its ItemTag from the pipeline it is
    // the physical half of — SmartPlant renders them identically.
    let item_tag = resolve_pipe_item_tag(obj);
    // PipingConnector UID derived from the PipeRun UID so it
    // remains stable across runs. SPPID treats this as a
    // composition relationship inside the drawing.
    let connector_uid = format!("{}-CNX", obj.uid);
    writeln!(buf, "   <PIDPipingConnector>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IObject UID="{}" ItemTag="{}"/>"#,
        escape_attr(&connector_uid),
        escape_attr(&item_tag),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IConnector/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPipingConnector/>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <INamedPipingConnector PipingConnectorPrefix="" PipingConnectorSeqNo="{}" PipingConnectorSuff=""/>"#,
        escape_attr(&tag_sequence)
    )
    .map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IPipeCrossSectionItem NominalDiameter="{}"/>"#,
        escape_attr(&nominal_diameter)
    )
    .map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IPipingSpecifiedItem PipingMaterialsClass="{}"/>"#,
        escape_attr(&piping_materials_class)
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IPIDTypical IsTypical=\"False\"/>").map_err(fmt_err)?;
    writeln!(buf, "   </PIDPipingConnector>").map_err(fmt_err)
}

/// Emit the three virtual nodes SmartPlant always derives from a
/// PipingConnector: two `<PIDPipingPort>` children (suffixed `.1`
/// and `.2`, both inheriting the parent connector's nominal
/// diameter) plus one `<PIDProcessPoint>` (suffixed `.PPT`).
///
/// These nodes never appear as their own SQLite rows — they are
/// SmartPlant client-side composition members rendered by the
/// exporter at publish time. The base UID is the same `<piperun>-CNX`
/// the connector carries, so the resulting `_Data.xml` cross-refs
/// (`PipingPortComposition`, `ProcessPointCollection`,
/// `PipingEnd1Conn`, `PipingEnd2Conn`) line up with the reference
/// fixture's UID conventions.
fn write_derived_connector_endpoints(
    buf: &mut String,
    obj: &PublishObject,
) -> Result<(), PublishError> {
    let connector_uid = format!("{}-CNX", obj.uid);
    let nominal_diameter = obj
        .fields
        .get("NominalDiameter")
        .cloned()
        .map(|v| format_diameter(&v))
        .unwrap_or_default();

    for port_index in [1u8, 2u8] {
        let port_uid = format!("{connector_uid}.{port_index}");
        writeln!(buf, "   <PIDPipingPort>").map_err(fmt_err)?;
        writeln!(
            buf,
            r#"      <IObject UID="{}" Name="{}"/>"#,
            escape_attr(&port_uid),
            port_index,
        )
        .map_err(fmt_err)?;
        writeln!(buf, "      <IConnection/>").map_err(fmt_err)?;
        writeln!(buf, "      <IPipingPort/>").map_err(fmt_err)?;
        writeln!(buf, "      <IPipingConnection/>").map_err(fmt_err)?;
        writeln!(buf, "      <IPort/>").map_err(fmt_err)?;
        writeln!(
            buf,
            r#"      <IPipeCrossSectionItem NominalDiameter="{}"/>"#,
            escape_attr(&nominal_diameter),
        )
        .map_err(fmt_err)?;
        writeln!(buf, "   </PIDPipingPort>").map_err(fmt_err)?;
    }

    let process_point_uid = format!("{connector_uid}.PPT");
    writeln!(buf, "   <PIDProcessPoint>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IObject UID="{}"/>"#,
        escape_attr(&process_point_uid),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IProcessPoint/>").map_err(fmt_err)?;
    writeln!(buf, "      <IFacilityPoint/>").map_err(fmt_err)?;
    writeln!(buf, "      <IProcessPointCaseComposition/>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IBulkMaxProcessPoint PhaseTemperatureMax="" PressureMax=""/>"#,
    )
    .map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IBulkMinProcessPoint PhaseTemperatureMin="" PressureMin=""/>"#,
    )
    .map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IBulkNormProcessPoint MolecularWeightNorm="" CpCvRatioNorm=""/>"#,
    )
    .map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IBulkBaseProcessPoint PhaseTemperatureBase="" PressureBase=""/>"#,
    )
    .map_err(fmt_err)?;
    writeln!(buf, "   </PIDProcessPoint>").map_err(fmt_err)
}

/// Emit `count` `<PIDSignalPort>` children SmartPlant derives
/// from a single InstrFunction / Instrument row.
///
/// Each port carries:
/// * `IObject UID="<instr>.N" Name="N"` — index-suffixed UID matching
///   the SmartPlant `<instr>.{1..count}` pattern (verified against
///   `DWG-0202GP06-01_Data.xml`).
/// * Five empty interface tags (`IConnection`, `ISignalConnection`,
///   `ISignalPort`, `IPipingConnection`, `IFacilityPoint`) — exact
///   shape of the reference fixture.
/// * `IPIDTypical` (no `IsTypical` attribute, matching the
///   reference; Vessel/Nozzle's typical attribute is omitted here).
///
/// Counts of zero render nothing but stay well-formed.
fn write_derived_instr_signal_ports(
    buf: &mut String,
    obj: &PublishObject,
    count: u8,
) -> Result<(), PublishError> {
    for index in 1..=count {
        let port_uid = format!("{}.{index}", obj.uid);
        writeln!(buf, "   <PIDSignalPort>").map_err(fmt_err)?;
        writeln!(
            buf,
            r#"      <IObject UID="{}" Name="{}"/>"#,
            escape_attr(&port_uid),
            index,
        )
        .map_err(fmt_err)?;
        writeln!(buf, "      <IConnection/>").map_err(fmt_err)?;
        writeln!(buf, "      <ISignalConnection/>").map_err(fmt_err)?;
        writeln!(buf, "      <ISignalPort/>").map_err(fmt_err)?;
        writeln!(buf, "      <IPipingConnection/>").map_err(fmt_err)?;
        writeln!(buf, "      <IFacilityPoint/>").map_err(fmt_err)?;
        writeln!(buf, "      <IPIDTypical/>").map_err(fmt_err)?;
        writeln!(buf, "   </PIDSignalPort>").map_err(fmt_err)?;
    }
    Ok(())
}

fn write_piping_port(buf: &mut String, obj: &PublishObject) -> Result<(), PublishError> {
    let nominal_diameter = obj
        .fields
        .get("NominalDiameter")
        .cloned()
        .map(|v| format_diameter(&v))
        .unwrap_or_default();
    writeln!(buf, "   <PIDPipingPort>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IObject UID="{}"/>"#,
        escape_attr(&obj.uid)
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IConnection/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPipingPort/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPipingConnection/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPort/>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IPipeCrossSectionItem NominalDiameter="{}"/>"#,
        escape_attr(&nominal_diameter)
    )
    .map_err(fmt_err)?;
    writeln!(buf, "   </PIDPipingPort>").map_err(fmt_err)
}

/// Emit a `<PIDNote>` block for a Note / ItemNote object.
///
/// The reference DWG fixture (`DWG-0202GP06-01_Data.xml`) shows
/// the canonical shape:
/// ```text
/// <PIDNote>
///    <IObject UID="..."/>
///    <IDrawingItem/>
///    <IPBSNote/>
///    <INote NoteText="量液孔"/>
///    <IDocumentItem/>
/// </PIDNote>
/// ```
///
/// `NoteText` is sourced from the standard
/// `T_ModelItem.Description` column when present (this is where the
/// SmartPlant client puts the note body for plain annotation rows).
/// Fixtures whose business subtable has stamped a dedicated
/// `NoteText` field win over `Description`. When neither is
/// present the attribute renders empty rather than fabricating a
/// placeholder.
fn write_item_note(buf: &mut String, obj: &PublishObject) -> Result<(), PublishError> {
    let note_text = obj
        .fields
        .get("NoteText")
        .cloned()
        .or_else(|| obj.description.clone())
        .unwrap_or_default();
    writeln!(buf, "   <PIDNote>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IObject UID="{}"/>"#,
        escape_attr(&obj.uid)
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IDrawingItem/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPBSNote/>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <INote NoteText="{}"/>"#,
        escape_attr(&note_text)
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IDocumentItem/>").map_err(fmt_err)?;
    writeln!(buf, "   </PIDNote>").map_err(fmt_err)
}

/// Emit a `<PIDControlSystemFunction>` block for an Instrument /
/// InstrFunction object.
///
/// The reference DWG fixture shows the canonical shape:
/// ```text
/// <PIDControlSystemFunction>
///    <IObject UID="..." Name="LIA-060201"/>
///    <IPBSItem ConstructionStatus="@NewConstruction" .../>
///    <IControlSystemFunction/>
///    <IDrawingItem/>
///    <ISignalPortComposition/>
///    <IInstrument/>
///    <IPlannedMatl/>
///    <ILoopMember/>
///    <IDocumentItem/>
///    <INoteCollection/>
///    <IExpandableThing/>
///    <INamedInstrument InstrFuncModifier="" InstrLoopSuffix=""
///       InstrTagPrefix="" InstrTagSequenceNo="060201"
///       InstrTagSuffix="" MeasuredVariable="LIA"/>
/// </PIDControlSystemFunction>
/// ```
///
/// The friendly `Name` attribute is built as `<MeasuredVariable>-<TagSequenceNo>`
/// from `obj.fields["MeasuredVariableCode"]` + `obj.fields["TagSequenceNo"]`
/// (both columns live on `T_Instrument`, attached to InstrFunction
/// rows via `subtables_for_item_type`). When either piece is
/// missing the writer falls back to the bare `MeasuredVariable`
/// (or the bare sequence) so the human-readable label is still
/// non-empty whenever any signal is available.
fn write_control_system_function(
    buf: &mut String,
    obj: &PublishObject,
) -> Result<(), PublishError> {
    let measured_variable = obj
        .fields
        .get("MeasuredVariableCode")
        .cloned()
        .unwrap_or_default();
    let tag_sequence_no = obj
        .fields
        .get("TagSequenceNo")
        .cloned()
        .unwrap_or_default();
    let tag_prefix = obj.fields.get("TagPrefix").cloned().unwrap_or_default();
    let tag_suffix = obj.fields.get("TagSuffix").cloned().unwrap_or_default();
    let loop_suffix = obj
        .fields
        .get("LoopTagSuffix")
        .cloned()
        .unwrap_or_default();
    let func_modifier = obj
        .fields
        .get("InstrumentTypeModifier")
        .cloned()
        .unwrap_or_default();

    let name = match (measured_variable.is_empty(), tag_sequence_no.is_empty()) {
        (false, false) => format!("{measured_variable}-{tag_sequence_no}"),
        (false, true) => measured_variable.clone(),
        (true, false) => tag_sequence_no.clone(),
        (true, true) => String::new(),
    };

    writeln!(buf, "   <PIDControlSystemFunction>").map_err(fmt_err)?;
    if name.is_empty() {
        // Empty `Name=""` would be uglier than omitting it; reference
        // fixtures always have a populated Name, so the empty case is
        // a defensive fallback only.
        writeln!(
            buf,
            r#"      <IObject UID="{}"/>"#,
            escape_attr(&obj.uid),
        )
        .map_err(fmt_err)?;
    } else {
        writeln!(
            buf,
            r#"      <IObject UID="{}" Name="{}"/>"#,
            escape_attr(&obj.uid),
            escape_attr(&name),
        )
        .map_err(fmt_err)?;
    }
    writeln!(buf, "      <IPBSItem/>").map_err(fmt_err)?;
    writeln!(buf, "      <IControlSystemFunction/>").map_err(fmt_err)?;
    writeln!(buf, "      <IDrawingItem/>").map_err(fmt_err)?;
    writeln!(buf, "      <ISignalPortComposition/>").map_err(fmt_err)?;
    writeln!(buf, "      <IInstrument/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPlannedMatl/>").map_err(fmt_err)?;
    writeln!(buf, "      <ILoopMember/>").map_err(fmt_err)?;
    writeln!(buf, "      <IDocumentItem/>").map_err(fmt_err)?;
    writeln!(buf, "      <INoteCollection/>").map_err(fmt_err)?;
    writeln!(buf, "      <IExpandableThing/>").map_err(fmt_err)?;
    writeln!(
        buf,
        concat!(
            r#"      <INamedInstrument InstrFuncModifier="{}" InstrLoopSuffix="{}" "#,
            r#"InstrTagPrefix="{}" InstrTagSequenceNo="{}" InstrTagSuffix="{}" "#,
            r#"MeasuredVariable="{}"/>"#,
        ),
        escape_attr(&func_modifier),
        escape_attr(&loop_suffix),
        escape_attr(&tag_prefix),
        escape_attr(&tag_sequence_no),
        escape_attr(&tag_suffix),
        escape_attr(&measured_variable),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "   </PIDControlSystemFunction>").map_err(fmt_err)
}

fn write_generic_object(
    buf: &mut String,
    obj: &PublishObject,
    item_type_name: &str,
) -> Result<(), PublishError> {
    writeln!(
        buf,
        r#"   <!-- Unsupported item type `{}`: emitting generic placeholder -->"#,
        escape_attr(item_type_name),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "   <PIDItem>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IObject UID="{}"/>"#,
        escape_attr(&obj.uid),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "   </PIDItem>").map_err(fmt_err)
}

/// Render the SmartPlant composite equipment tag ("TagPrefix
/// TagSequenceNo") from a vessel / equipment row's business
/// fields. Returns an empty string when neither field is present.
fn format_equipment_tag(obj: &PublishObject) -> String {
    let prefix = obj
        .fields
        .get("TagPrefix")
        .map(|s| s.as_str())
        .unwrap_or("");
    let seq = obj
        .fields
        .get("TagSequenceNo")
        .map(|s| s.as_str())
        .unwrap_or("");
    if prefix.is_empty() && seq.is_empty() {
        String::new()
    } else {
        format!("{prefix} {seq}").trim().to_string()
    }
}

/// Append a `" mm"` suffix to a bare numeric diameter so the XML
/// matches SmartPlant's canonical "250 mm" form. If the value
/// already carries a unit we leave it alone.
fn format_diameter(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.chars().any(|c| c.is_ascii_alphabetic()) {
        return trimmed.to_string();
    }
    format!("{trimmed} mm")
}

/// Map a SPPID boolean string ("1" / "0" / "") to the XML form
/// SmartPlant uses ("True" / "False").
fn map_bool(value: &str) -> &'static str {
    match value.trim() {
        "1" | "True" | "true" => "True",
        _ => "False",
    }
}

/// Convert a `std::fmt::Error` into a [`PublishError`] so the
/// writer's `?`-operator chain stays uniform with the SQLite
/// loader's.
fn fmt_err(err: std::fmt::Error) -> PublishError {
    PublishError::Sqlite(format!("format: {err}"))
}

fn write_container_open(
    buf: &mut String,
    drawing: &PublishDrawing,
    plant_name: &str,
) -> Result<(), PublishError> {
    writeln!(
        buf,
        concat!(
            r#"<Container CompSchema="{}" Scope="{}" SoftwareVersion="{}" "#,
            r#"IsValidated="False" SchemaVersion="{}" LoginUser="" LoginPWD="" "#,
            r#"Plant="{}" Project="" DocUID="{}" DocName="{}" Version="" "#,
            r#"ToolID="{}" ToolSignature="{}" SDECIMAL="{}">"#
        ),
        CONTAINER_COMP_SCHEMA,
        CONTAINER_SCOPE,
        CONTAINER_SOFTWARE_VERSION,
        CONTAINER_SCHEMA_VERSION,
        escape_attr(plant_name),
        escape_attr(&drawing.drawing_uid),
        escape_attr(&drawing.drawing_name),
        CONTAINER_TOOL_ID,
        CONTAINER_TOOL_SIGNATURE,
        CONTAINER_SDECIMAL,
    )
    .map_err(fmt_err)
}

/// `_Meta.xml` flavor of the container header. Identical wire shape
/// to [`write_container_open`] but stamps `CompSchema=
/// "DocVersioningComponent"` so SmartPlant routes the document to
/// the document-versioning loader instead of the data loader.
/// `LoginUser` / `LoginPWD` attributes are intentionally omitted to
/// match the reference exports byte-for-byte.
fn write_meta_container_open(
    buf: &mut String,
    drawing: &PublishDrawing,
    plant_name: &str,
) -> Result<(), PublishError> {
    writeln!(
        buf,
        concat!(
            r#"<Container CompSchema="{}" Scope="{}" SoftwareVersion="{}" "#,
            r#"IsValidated="False" SchemaVersion="{}" Plant="{}" Project="" "#,
            r#"DocUID="{}" DocName="{}" Version="" ToolID="{}" "#,
            r#"ToolSignature="{}" SDECIMAL="{}">"#
        ),
        CONTAINER_META_COMP_SCHEMA,
        CONTAINER_SCOPE,
        CONTAINER_SOFTWARE_VERSION,
        CONTAINER_SCHEMA_VERSION,
        escape_attr(plant_name),
        escape_attr(&drawing.drawing_uid),
        escape_attr(&drawing.drawing_name),
        CONTAINER_TOOL_ID,
        CONTAINER_TOOL_SIGNATURE,
        CONTAINER_SDECIMAL,
    )
    .map_err(fmt_err)
}

/// Emit `<DocumentVersion>` block with the deterministic
/// `version_uid`, the drawing's friendly name (`"<name> Version"`),
/// and a `DocVersionDate` parsed from `drawing.date_created`. When
/// no date is available the attribute renders empty rather than
/// fabricating a fake one — downstream tooling can treat the empty
/// string as "unknown".
fn write_meta_document_version(
    buf: &mut String,
    drawing: &PublishDrawing,
    version_uid: &str,
) -> Result<(), PublishError> {
    let version_date = drawing
        .date_created
        .as_deref()
        .map(format_meta_date)
        .unwrap_or_default();
    writeln!(buf, "   <DocumentVersion>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IObject UID="{}" Name="{} Version"/>"#,
        escape_attr(version_uid),
        escape_attr(&drawing.drawing_name),
    )
    .map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IDocumentVersion DocRevision="{}" DocVersionDate="{}" DocVersion="{}"/>"#,
        META_DEFAULT_DOC_REVISION,
        escape_attr(&version_date),
        META_DEFAULT_DOC_VERSION,
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IFileComposition/>").map_err(fmt_err)?;
    writeln!(buf, "   </DocumentVersion>").map_err(fmt_err)
}

/// Emit `<DocumentRevision>` with the deterministic `revision_uid`.
/// `MajorRev_ForRevise` defaults to `"0"` and `MinorRev_ForRevise`
/// stays empty; both match every reference fixture and there is no
/// SQLite column carrying the values yet.
fn write_meta_document_revision(
    buf: &mut String,
    drawing: &PublishDrawing,
    revision_uid: &str,
) -> Result<(), PublishError> {
    writeln!(buf, "   <DocumentRevision>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IObject UID="{}" Name="{} Revision"/>"#,
        escape_attr(revision_uid),
        escape_attr(&drawing.drawing_name),
    )
    .map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IDocumentRevision MajorRev_ForRevise="0" MinorRev_ForRevise=""/>"#,
    )
    .map_err(fmt_err)?;
    writeln!(buf, "   </DocumentRevision>").map_err(fmt_err)
}

/// Emit `<File>` with the deterministic `file_uid`. The file name
/// is `<drawing_name>.pid` to mirror the on-disk artifact; the
/// `IFile FilePath=""` attribute stays empty because the original
/// SPPID export stamps the local filesystem path of the operator's
/// machine — a value we have no way of recovering and that, when
/// missing, downstream consumers tolerate.
fn write_meta_file(
    buf: &mut String,
    drawing: &PublishDrawing,
    file_uid: &str,
) -> Result<(), PublishError> {
    writeln!(buf, "   <File>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IObject UID="{}" Name="{}.pid" Description=""/>"#,
        escape_attr(file_uid),
        escape_attr(&drawing.drawing_name),
    )
    .map_err(fmt_err)?;
    writeln!(buf, r#"      <IFile FilePath=""/>"#).map_err(fmt_err)?;
    writeln!(buf, "   </File>").map_err(fmt_err)
}

/// Emit a single `<Rel>` row. `def_uid` is the SmartPlant
/// relationship classifier (`"VersionedDoc"`, `"RevisedDocument"`,
/// `"FileComposition"`); the meta document only ever uses these
/// three so the helper does not need to be more general.
fn write_meta_rel(
    buf: &mut String,
    rel_uid: &str,
    uid1: &str,
    uid2: &str,
    def_uid: &str,
) -> Result<(), PublishError> {
    writeln!(buf, "   <Rel>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IObject UID="{}"/>"#,
        escape_attr(rel_uid),
    )
    .map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IRel UID1="{}" UID2="{}" DefUID="{}"/>"#,
        escape_attr(uid1),
        escape_attr(uid2),
        escape_attr(def_uid),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "   </Rel>").map_err(fmt_err)
}

/// Derive a deterministic 32-hex-character SmartPlant UID from
/// `(seed, role)` via UUID v5 (SHA-1 over the OID namespace). The
/// `seed` is typically the drawing's `T_Drawing.SP_ID`; the `role`
/// disambiguates the per-document children (`"version"`,
/// `"revision"`, `"file"`, `"rel/<def-uid>"`). The output is the
/// uppercase 32-char hex form SmartPlant uses for every SP_ID.
///
/// Determinism is the whole point: the writer can be invoked twice
/// and both runs produce byte-identical `_Meta.xml`, so test
/// fixtures can be golden-compared and CI can detect any drift.
pub(crate) fn derive_meta_uid(seed: &str, role: &str) -> String {
    let payload = format!("{seed}/{role}");
    let uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, payload.as_bytes());
    uuid.simple().to_string().to_uppercase()
}

/// Normalize a SmartPlant `DateCreated` string into the
/// `YYYY/MM/DD` form reference exports use for `DocVersionDate`.
///
/// OrcaMDF surfaces the value as the SQL Server raw render — for
/// example `"2026/4/20 10:32:46"`. We zero-pad the month and day
/// and drop the time component, matching the reference fixtures'
/// `"2026/04/20"`. Any input that does not parse as
/// `YYYY/M/D[ ...]` is returned verbatim so callers retain enough
/// debug context to spot unsupported formats.
fn format_meta_date(raw: &str) -> String {
    let date_part = raw.split_whitespace().next().unwrap_or(raw);
    let mut parts = date_part.split('/');
    let (Some(y), Some(m), Some(d), None) = (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return raw.to_string();
    };
    let (Ok(y_n), Ok(m_n), Ok(d_n)) = (y.parse::<u32>(), m.parse::<u32>(), d.parse::<u32>()) else {
        return raw.to_string();
    };
    format!("{y_n:04}/{m_n:02}/{d_n:02}")
}

fn write_pid_drawing(buf: &mut String, drawing: &PublishDrawing) -> Result<(), PublishError> {
    writeln!(buf, "   <PIDDrawing>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IObject UID="{}" Name="{}" Description=""/>"#,
        escape_attr(&drawing.drawing_uid),
        escape_attr(&drawing.drawing_name),
    )
    .map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IDocument DocCategory="P&amp;ID Documents" DocTitle="{}" DocType="P&amp;ID" DocSubtype=""/>"#,
        escape_attr(&drawing.drawing_name),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IDocVersionComposition/>").map_err(fmt_err)?;
    writeln!(buf, "      <IDwgRepresentationComposition/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPIDDrawing/>").map_err(fmt_err)?;
    writeln!(buf, "      <ISchematicDwg/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPBSItem/>").map_err(fmt_err)?;
    writeln!(buf, "   </PIDDrawing>").map_err(fmt_err)
}

/// True when a representation row deserves a `<PIDRepresentation>`
/// element in the published XML. SmartPlant's exporter skips the
/// pure annotation / label rows — those whose `T_Representation.SP_ModelItemID`
/// is NULL or empty (typically `\Equipment\Labels - ...` and
/// `\Piping\Labels - ...` symbols). Mirroring that filter is what
/// keeps the A01 diff in lockstep with the reference fixture.
///
/// The check is centralized so `write_representations` and the
/// derived `<Rel>` emitters share the exact same predicate; the
/// derived `DwgRepresentationComposition` rel was already
/// naturally filtered (its source IS `model_item_uid`), but the
/// `DrawingItems` rel was not — A14 brings them into alignment.
fn representation_is_publishable(rep: &PublishRepresentation) -> bool {
    matches!(rep.model_item_uid.as_deref(), Some(uid) if !uid.is_empty())
}

fn write_representations(buf: &mut String, drawing: &PublishDrawing) -> Result<(), PublishError> {
    for rep in &drawing.representations {
        if !representation_is_publishable(rep) {
            continue;
        }
        writeln!(buf, "   <PIDRepresentation>").map_err(fmt_err)?;
        writeln!(
            buf,
            r#"      <IObject UID="{}"/>"#,
            escape_attr(&rep.uid)
        )
        .map_err(fmt_err)?;
        match rep.graphic_oid {
            Some(oid) => writeln!(
                buf,
                r#"      <IDrawingRepresentation GraphicOID="{oid}"/>"#
            )
            .map_err(fmt_err)?,
            None => writeln!(buf, r#"      <IDrawingRepresentation/>"#).map_err(fmt_err)?,
        }
        writeln!(buf, "   </PIDRepresentation>").map_err(fmt_err)?;
    }
    Ok(())
}

fn write_relationships(buf: &mut String, drawing: &PublishDrawing) -> Result<(), PublishError> {
    // Emit the three classes of `<Rel>` nodes in the order SPPID
    // uses: (1) ModelItem → Representation, (2) Drawing →
    // Representation, (3) T_Relationship rows (semantically
    // classified). That ordering matches the reference
    // DWG-0202GP06-01_Data.xml layout.

    // Build a lookup from UID → ItemTypeName so we can infer
    // DefUID for T_Relationship rows. Covers both model items
    // and representations (representations do not carry a SPPID
    // item type, but surfacing them as "Representation" lets
    // the classifier still pick a reasonable DefUID).
    let mut type_by_uid: HashMap<&str, &str> = HashMap::new();
    for obj in &drawing.objects {
        type_by_uid.insert(obj.uid.as_str(), obj.item_type_name.as_str());
    }
    for rep in &drawing.representations {
        type_by_uid.insert(rep.uid.as_str(), "Representation");
    }

    // --- Derived: ModelItem → Representation (DwgRepresentationComposition)
    // Naturally filtered to publishable reps (the rel's source IS
    // `model_item_uid`, so a pure annotation row with no model
    // item never produces one). Keep the inline check anyway so a
    // future loader change that accidentally injects `Some("")`
    // does not silently produce a malformed rel.
    for rep in &drawing.representations {
        if !representation_is_publishable(rep) {
            continue;
        }
        let model_item_uid = rep
            .model_item_uid
            .as_deref()
            .expect("publishable rep has model_item_uid");
        write_rel(
            buf,
            &format!("DRC-{}-{}", model_item_uid, rep.uid),
            model_item_uid,
            &rep.uid,
            "DwgRepresentationComposition",
        )?;
    }

    // --- Derived: Drawing → Representation (DrawingItems)
    // A14: only emit `DrawingItems` for representations that survive
    // the A14 publishability filter. Otherwise we would generate a
    // rel pointing at a `<PIDRepresentation>` we never wrote — a
    // dangling reference SmartPlant validators reject.
    for rep in &drawing.representations {
        if !representation_is_publishable(rep) {
            continue;
        }
        write_rel(
            buf,
            &format!("DRI-{}-{}", drawing.drawing_uid, rep.uid),
            &drawing.drawing_uid,
            &rep.uid,
            "DrawingItems",
        )?;
    }

    // --- From T_Relationship, classified by endpoint item types
    for rel in &drawing.relationships {
        let uid1 = rel.source_uid.as_deref().unwrap_or("");
        let uid2 = rel.target_uid.as_deref().unwrap_or("");
        let def_uid = classify_relationship(rel, &type_by_uid);
        let prefix = defuid_prefix(&def_uid);
        let rel_uid = format!("{prefix}-{uid1}-{uid2}");
        write_rel(buf, &rel_uid, uid1, uid2, &def_uid)?;
    }
    Ok(())
}

/// Emit a single `<Rel>` node with the given pre-composed UIDs.
fn write_rel(
    buf: &mut String,
    rel_uid: &str,
    uid1: &str,
    uid2: &str,
    def_uid: &str,
) -> Result<(), PublishError> {
    writeln!(buf, "   <Rel>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IObject UID="{}"/>"#,
        escape_attr(rel_uid)
    )
    .map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IRel UID1="{}" UID2="{}" DefUID="{}"/>"#,
        escape_attr(uid1),
        escape_attr(uid2),
        escape_attr(def_uid),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "   </Rel>").map_err(fmt_err)
}

/// Pick the SPPID DefUID for a T_Relationship row given a lookup
/// of endpoint ItemTypeNames. Stage-1 covers the combinations
/// observed in TEST02 A01; anything unknown falls back to the
/// generic `"Relationship"` so the writer stays total.
fn classify_relationship(
    rel: &PublishRelationship,
    type_by_uid: &HashMap<&str, &str>,
) -> String {
    let src_type = rel
        .source_uid
        .as_deref()
        .and_then(|u| type_by_uid.get(u).copied())
        .unwrap_or("");
    let tgt_type = rel
        .target_uid
        .as_deref()
        .and_then(|u| type_by_uid.get(u).copied())
        .unwrap_or("");
    match (src_type, tgt_type) {
        // Nozzle attached to a vessel → equipment-component composition.
        ("Nozzle", "Vessel") | ("Vessel", "Nozzle") => "EquipmentComponentComposition".into(),
        // Piping endpoint tying a connector / pipe to an equipment
        // face. When the rel already targets a Representation, we
        // leave it classified by the model layer that produced it.
        ("PipeRun", "Nozzle") | ("Nozzle", "PipeRun") => "PipingEnd1Conn".into(),
        // Connector → Pipeline composition.
        ("PipeRun", "Pipeline") | ("Pipeline", "PipeRun") => "PipingConnectors".into(),
        // Two representations related at the drawing level — treat
        // as a generic DwgRepresentationComposition.
        ("Representation", "Representation") => "DwgRepresentationComposition".into(),
        // Any other combination keeps the generic marker. Higher
        // layers can override once they ship richer item types.
        _ => "Relationship".into(),
    }
}

/// Prefix used when composing the `<Rel><IObject UID="...">`
/// value from UID1 / UID2. Matches the SPPID reference convention:
/// `DRC-` / `DRI-` / `EQC-` / `PCN-` / `PE1-` / `PE2-` /
/// `PPC-` / `PTF-` / `SPC-` / `PRP-`.
fn defuid_prefix(def_uid: &str) -> &'static str {
    match def_uid {
        "DwgRepresentationComposition" => "DRC",
        "DrawingItems" => "DRI",
        "EquipmentComponentComposition" => "EQC",
        "PipingConnectors" => "PCN",
        "PipingEnd1Conn" => "PE1",
        "PipingEnd2Conn" => "PE2",
        "PipingPortComposition" => "PPC",
        "PipingTapOrFitting" => "PTF",
        "SignalPortComposition" => "SPC",
        "ProcessPointCollection" => "PRP",
        _ => "REL",
    }
}

/// XML attribute-value escape. SmartPlant uses double-quote
/// delimiters so we only need to escape the five canonical
/// entities plus CR/LF (which SPPID stores verbatim as
/// `&#13;&#10;` inside attribute values).
fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\r' => out.push_str("&#13;"),
            '\n' => out.push_str("&#10;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::publish::model::{PublishObject, PublishRelationship, PublishRepresentation};

    fn example_drawing() -> PublishDrawing {
        let mut d = PublishDrawing::new("D9635C3C898840D1990B7E8BEE1D55DA", "A01");
        d.template = Some("A2-W-New.pid".into());
        d.path = Some("\\01\\01\\A01.pid".into());
        d.date_created = Some("2026/4/20 10:32:46".into());
        d.objects = vec![
            PublishObject {
                uid: "185EF98B03E844158E3BD8E82806E6CF".into(),
                item_type_name: "PipeRun".into(),
                ..PublishObject::default()
            },
            PublishObject {
                uid: "7465E81219DB49B492BDF60A055AA391".into(),
                item_type_name: "Nozzle".into(),
                ..PublishObject::default()
            },
            PublishObject {
                uid: "C57494A1B154442C9DF0F4BA713E88EC".into(),
                item_type_name: "Vessel".into(),
                ..PublishObject::default()
            },
        ];
        d.representations = vec![
            PublishRepresentation {
                uid: "CA8A0A9DD1784E3BB6913445CE3F6375".into(),
                drawing_uid: d.drawing_uid.clone(),
                model_item_uid: Some("C57494A1B154442C9DF0F4BA713E88EC".into()),
                graphic_oid: Some(184),
                symbol_path: Some(r"\Equipment\Vessels\Horizontal Drums\Horizontal Drum.sym".into()),
                representation_type: Some(13),
            },
            PublishRepresentation {
                uid: "C33E5BD9B9CC4287B244A925A7A1F29B".into(),
                drawing_uid: d.drawing_uid.clone(),
                model_item_uid: Some("7465E81219DB49B492BDF60A055AA391".into()),
                graphic_oid: Some(51),
                symbol_path: Some(r"\Equipment Components\Nozzles\Flanged Nozzle.sym".into()),
                representation_type: Some(13),
            },
        ];
        d.relationships = vec![PublishRelationship {
            uid: "50B7DAA7B182478D8EE5D1F4E6CD3FA5".into(),
            drawing_uid: d.drawing_uid.clone(),
            source_uid: Some("C33E5BD9B9CC4287B244A925A7A1F29B".into()),
            target_uid: Some("CA8A0A9DD1784E3BB6913445CE3F6375".into()),
            graphic_oid: Some(42),
            item1_location: Some(-1),
            item2_location: Some(-3),
            is_binary: Some(2),
        }];
        d
    }

    #[test]
    fn xml_opens_with_container_element_and_hard_coded_constants() {
        let out = write_data_xml(&example_drawing(), "TEST02").expect("write");
        assert!(out.starts_with("<?xml version =\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(out.contains("CompSchema=\"PIDComponent\""));
        assert!(out.contains("SoftwareVersion=\"10.00.31.0023\""));
        assert!(out.contains("Plant=\"TEST02\""));
        assert!(out.contains("DocUID=\"D9635C3C898840D1990B7E8BEE1D55DA\""));
        assert!(out.contains("DocName=\"A01\""));
        assert!(out.trim_end().ends_with("</Container>"));
    }

    #[test]
    fn xml_renders_drawing_node_with_escape() {
        let out = write_data_xml(&example_drawing(), "TEST02").expect("write");
        assert!(out.contains("<PIDDrawing>"));
        assert!(out.contains("<IObject UID=\"D9635C3C898840D1990B7E8BEE1D55DA\" Name=\"A01\""));
        assert!(out.contains("DocCategory=\"P&amp;ID Documents\""));
        assert!(out.contains("DocType=\"P&amp;ID\""));
    }

    #[test]
    fn xml_renders_every_representation() {
        let out = write_data_xml(&example_drawing(), "TEST02").expect("write");
        // Two representations in the fixture — both should be
        // emitted, complete with their GraphicOIDs.
        assert!(out.contains(r#"<IObject UID="CA8A0A9DD1784E3BB6913445CE3F6375"/>"#));
        assert!(out.contains(r#"<IDrawingRepresentation GraphicOID="184"/>"#));
        assert!(out.contains(r#"<IObject UID="C33E5BD9B9CC4287B244A925A7A1F29B"/>"#));
        assert!(out.contains(r#"<IDrawingRepresentation GraphicOID="51"/>"#));
    }

    #[test]
    fn xml_emits_derived_drawing_and_model_item_rels() {
        let out = write_data_xml(&example_drawing(), "TEST02").expect("write");
        // DwgRepresentationComposition — ModelItem → Rep
        assert!(
            out.contains(r#"<IObject UID="DRC-C57494A1B154442C9DF0F4BA713E88EC-CA8A0A9DD1784E3BB6913445CE3F6375"/>"#),
            "expected a DRC- prefixed rel for Vessel model item → its representation; full output:\n{out}"
        );
        assert!(out.contains(
            r#"<IRel UID1="C57494A1B154442C9DF0F4BA713E88EC" UID2="CA8A0A9DD1784E3BB6913445CE3F6375" DefUID="DwgRepresentationComposition"/>"#
        ));
        // DrawingItems — Drawing → Rep
        assert!(
            out.contains(r#"<IObject UID="DRI-D9635C3C898840D1990B7E8BEE1D55DA-CA8A0A9DD1784E3BB6913445CE3F6375"/>"#),
            "expected a DRI- prefixed rel for Drawing → Vessel rep"
        );
        assert!(out.contains(
            r#"<IRel UID1="D9635C3C898840D1990B7E8BEE1D55DA" UID2="CA8A0A9DD1784E3BB6913445CE3F6375" DefUID="DrawingItems"/>"#
        ));
    }

    #[test]
    fn xml_classifies_rel_endpoints_to_concrete_defuid() {
        // The example drawing's T_Relationship row ties the
        // Nozzle representation to the Vessel representation —
        // two Representations on the same drawing. The writer
        // classifies that pair as `DwgRepresentationComposition`
        // and uses the DRC- prefix on the composite UID.
        let out = write_data_xml(&example_drawing(), "TEST02").expect("write");
        assert!(
            out.contains(r#"<IObject UID="DRC-C33E5BD9B9CC4287B244A925A7A1F29B-CA8A0A9DD1784E3BB6913445CE3F6375"/>"#),
            "expected a DRC- prefixed rel from the T_Relationship row (Rep→Rep); out:\n{out}"
        );
    }

    #[test]
    fn xml_escapes_chinese_title_and_crlf() {
        let mut d = example_drawing();
        d.drawing_name = "安3集气站\r\n排污单元".into();
        let out = write_data_xml(&d, "P01").expect("write");
        // CR/LF must become numeric character references so the
        // attribute stays well-formed.
        assert!(out.contains("Name=\"安3集气站&#13;&#10;排污单元\""));
    }

    #[test]
    fn escape_attr_handles_xml_specials() {
        assert_eq!(escape_attr("a & b"), "a &amp; b");
        assert_eq!(escape_attr("<x>"), "&lt;x&gt;");
        assert_eq!(escape_attr("it's \"ok\""), "it&apos;s &quot;ok&quot;");
        assert_eq!(escape_attr("line1\r\nline2"), "line1&#13;&#10;line2");
    }

    #[test]
    fn vessel_eq_type_uses_codelist_lookup_when_available() {
        // When the drawing ships a codelist entry for
        // EquipmentType = "0", the writer MUST prefer the codelist
        // text over the symbol-path stem. This mirrors SmartPlant's
        // own rendering (the enum display text is the source of
        // truth; the symbol path is a UI convention).
        let mut d = example_drawing();
        // Seed the vessel with an EquipmentType code so the codelist
        // path has something to resolve.
        let vessel = d
            .objects
            .iter_mut()
            .find(|o| o.item_type_name == "Vessel")
            .expect("vessel in fixture");
        vessel.fields.insert("EquipmentType".into(), "3".into());
        // Register EquipmentType → codelist 28 → "3" = "Reactor".
        d.codelist.insert_attribute_mapping("EquipmentType", "28");
        d.codelist.insert_entry("28", "3", "Reactor");

        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IEquipment EqTypeDescription="Reactor"/>"#),
            "codelist-resolved description should win over the symbol-path stem; out:\n{out}"
        );
        // The symbol path stem ("Horizontal Drum") must NOT appear
        // as the EqType description — it's still in the XML as part
        // of the rep's FileName chain, but not on <IEquipment>.
        assert!(
            !out.contains(r#"EqTypeDescription="Horizontal Drum""#),
            "codelist lookup should beat symbol-path fallback; out:\n{out}"
        );
    }

    #[test]
    fn vessel_eq_type_falls_back_to_symbol_path_when_codelist_empty() {
        // No codelist metadata loaded → the writer's second-tier
        // fallback (symbol-path stem) must still produce the human
        // name so legacy fixtures keep working.
        let d = example_drawing();
        assert!(d.codelist.is_empty(), "fixture ships with empty codelist");
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IEquipment EqTypeDescription="Horizontal Drum"/>"#),
            "symbol path should still surface when codelist is empty; out:\n{out}"
        );
    }

    #[test]
    fn vessel_eq_type_falls_back_to_raw_code_when_no_symbol_and_no_codelist() {
        // Neither a codelist mapping nor a symbol path — the writer
        // must still emit something, and the raw EquipmentType code
        // is the last-ditch choice. (A blank attribute would silently
        // hide data, which is strictly worse than an opaque code.)
        let mut d = PublishDrawing::new("UID-V", "V");
        d.objects = vec![PublishObject {
            uid: "V1".into(),
            item_type_name: "Vessel".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("EquipmentType".into(), "7".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IEquipment EqTypeDescription="7"/>"#),
            "raw EquipmentType code must still land when both preferred \
             lookups miss; out:\n{out}"
        );
    }

    #[test]
    fn nozzle_proc_eq_comp_uses_codelist_lookup_when_available() {
        // Same three-tier fallback as the vessel, but keyed on
        // `NozzleType`. When the catalog ships the mapping the
        // writer must prefer it.
        let mut d = example_drawing();
        let nozzle = d
            .objects
            .iter_mut()
            .find(|o| o.item_type_name == "Nozzle")
            .expect("nozzle in fixture");
        nozzle.fields.insert("NozzleType".into(), "2".into());
        d.codelist.insert_attribute_mapping("NozzleType", "12");
        d.codelist.insert_entry("12", "2", "Pressurized Nozzle");

        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"ProcEqpCompTypeDescription="Pressurized Nozzle""#),
            "codelist-resolved nozzle description should win; out:\n{out}"
        );
    }

    #[test]
    fn nozzle_proc_eq_comp_keeps_default_when_nothing_else_available() {
        // No codelist, no symbol path — the default `"Flanged Nozzle"`
        // still lands so every nozzle has a non-empty description.
        let mut d = PublishDrawing::new("UID-N", "N");
        d.objects = vec![PublishObject {
            uid: "NZ1".into(),
            item_type_name: "Nozzle".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"ProcEqpCompTypeDescription="Flanged Nozzle""#),
            "hard-coded `Flanged Nozzle` fallback still fires; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_uses_plantitem_itemtag_when_available() {
        // A8: When T_PlantItem supplies an ItemTag (e.g. SmartPlant's
        // canonical "A010102102-PH" form), the writer MUST use it
        // verbatim rather than re-deriving a "PH-…" placeholder
        // from pipe-run columns. Same rule applies to the connector
        // that SmartPlant renders as the physical half of the
        // pipeline — both end up with identical tags.
        let mut d = PublishDrawing::new("UID-A01", "A01");
        d.objects = vec![PublishObject {
            uid: "PIPE-UID".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("ItemTag".into(), "A010102102-PH".into());
                m.insert("TagSequenceNo".into(), "0102102".into());
                m.insert("NominalDiameter".into(), "250".into());
                m.insert("PipingMaterialsClass".into(), "B5".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        // Pipeline + Connector share the SmartPlant-canonical tag.
        assert!(
            out.contains(r#"ItemTag="A010102102-PH""#),
            "expected canonical ItemTag from T_PlantItem; out:\n{out}"
        );
        // The pre-A8 synthesized form must NOT appear anywhere once
        // the catalog-driven tag is available.
        assert!(
            !out.contains("PH-0102102-250"),
            "synthesized PH- form should be suppressed when T_PlantItem has an ItemTag; out:\n{out}"
        );
        // Exactly two occurrences — once for <PIDPipeline> and once
        // for <PIDPipingConnector>.
        let occurrences = out.matches(r#"ItemTag="A010102102-PH""#).count();
        assert_eq!(
            occurrences, 2,
            "pipeline + connector should both carry the PlantItem ItemTag; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_synthesizes_tag_when_plantitem_itemtag_absent() {
        // No `ItemTag` key in obj.fields → the legacy `PH-…`
        // synthesis path should still fire so drawings without
        // T_PlantItem data remain readable.
        let mut d = PublishDrawing::new("UID-A01", "A01");
        d.objects = vec![PublishObject {
            uid: "PIPE-UID".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("TagSequenceNo".into(), "0102102".into());
                m.insert("NominalDiameter".into(), "250".into());
                m.insert("PipingMaterialsClass".into(), "B5".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"ItemTag="PH-0102102-250 mm-B5""#),
            "synthesized PH- tag should fire when T_PlantItem.ItemTag is missing; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_empty_itemtag_treated_as_absent_and_falls_back() {
        // A T_PlantItem row that is present but with an EMPTY ItemTag
        // (SmartPlant's legal "tag not yet assigned" state) must not
        // overrule the synthesized fallback — otherwise the XML would
        // emit `ItemTag=""` which is less useful than a synthesized
        // placeholder.
        let mut d = PublishDrawing::new("UID-A01", "A01");
        d.objects = vec![PublishObject {
            uid: "PIPE-UID".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("ItemTag".into(), "".into());
                m.insert("TagSequenceNo".into(), "0102102".into());
                m.insert("NominalDiameter".into(), "250".into());
                m.insert("PipingMaterialsClass".into(), "B5".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"ItemTag="PH-0102102-250 mm-B5""#),
            "empty PlantItem ItemTag should fall through to synthesized form; out:\n{out}"
        );
        assert!(
            !out.contains(r#"ItemTag="""#),
            "writer must never emit an empty ItemTag attribute; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_without_any_tag_info_falls_back_to_uid() {
        // Final fallback tier — no ItemTag, no TagSequenceNo —
        // emit the raw UID so the attribute is at least uniquely
        // identifying even if it's not human-readable.
        let mut d = PublishDrawing::new("UID-A01", "A01");
        d.objects = vec![PublishObject {
            uid: "BARE-UID".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"ItemTag="BARE-UID""#),
            "bare UID should surface when no ItemTag / TagSequenceNo present; out:\n{out}"
        );
    }

    #[test]
    fn piping_point_emits_pid_piping_port_tag_with_nominal_diameter() {
        // A9: PipingPoint objects synthesized from T_PipingPoint
        // rows must render as <PIDPipingPort> with the nominal
        // diameter carried through.
        let mut d = PublishDrawing::new("UID-A01", "A01");
        d.objects = vec![PublishObject {
            uid: "PP-UID".into(),
            item_type_name: "PipingPoint".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("NominalDiameter".into(), "250".into());
                m.insert("SP_PlantItemID".into(), "NOZZLE1".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains("<PIDPipingPort>"),
            "PipingPoint should render as <PIDPipingPort>; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IObject UID="PP-UID"/>"#),
            "IObject should carry the PipingPoint UID; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IPipeCrossSectionItem NominalDiameter="250 mm"/>"#),
            "NominalDiameter should round-trip with the `mm` unit; out:\n{out}"
        );
        assert!(
            out.contains("</PIDPipingPort>"),
            "PIDPipingPort must close; out:\n{out}"
        );
    }

    #[test]
    fn empty_drawing_still_produces_well_formed_xml() {
        let d = PublishDrawing::new("UID-EMPTY", "NoName");
        let out = write_data_xml(&d, "Plant1").expect("write");
        assert!(out.contains("<PIDDrawing>"));
        assert!(out.contains("</PIDDrawing>"));
        // No representations or rels — but the container still
        // closes and the document is valid.
        assert!(out.trim_end().ends_with("</Container>"));
    }

    // -----------------------------------------------------------------
    // A10.2 — _Meta.xml writer
    // -----------------------------------------------------------------

    #[test]
    fn meta_xml_renders_doc_versioning_container_header() {
        let d = PublishDrawing::new("D9635C3C898840D1990B7E8BEE1D55DA", "A01");
        let out = write_meta_xml(&d, "TEST02").expect("write meta");
        assert!(
            out.contains(r#"CompSchema="DocVersioningComponent""#),
            "meta document must advertise the DocVersioningComponent schema; out:\n{out}"
        );
        assert!(
            out.contains(r#"Plant="TEST02""#),
            "Plant attribute must round-trip; out:\n{out}"
        );
        assert!(
            out.contains(r#"DocUID="D9635C3C898840D1990B7E8BEE1D55DA""#),
            "DocUID must equal drawing_uid; out:\n{out}"
        );
        assert!(
            out.contains(r#"DocName="A01""#),
            "DocName must equal drawing_name; out:\n{out}"
        );
        assert!(
            out.trim_end().ends_with("</Container>"),
            "container must close; out:\n{out}"
        );
    }

    #[test]
    fn meta_xml_emits_three_main_blocks_and_three_rels_in_order() {
        let d = PublishDrawing::new("UID-ABC", "DRAW1");
        let out = write_meta_xml(&d, "P01").expect("write meta");

        let pos_version = out.find("<DocumentVersion>").expect("DocumentVersion present");
        let pos_revision = out.find("<DocumentRevision>").expect("DocumentRevision present");
        let pos_file = out.find("<File>").expect("File present");

        assert!(
            pos_version < pos_revision && pos_revision < pos_file,
            "blocks must be ordered DocumentVersion < DocumentRevision < File; out:\n{out}"
        );

        let rel_count = out.matches("<Rel>").count();
        assert_eq!(rel_count, 3, "meta document carries exactly three Rel rows; out:\n{out}");

        for def_uid in ["VersionedDoc", "RevisedDocument", "FileComposition"] {
            assert!(
                out.contains(&format!(r#"DefUID="{def_uid}""#)),
                "DefUID `{def_uid}` must appear; out:\n{out}"
            );
        }
    }

    #[test]
    fn meta_xml_uids_are_deterministic_across_runs() {
        let d = PublishDrawing::new("UID-ABC", "DRAW1");
        let out_a = write_meta_xml(&d, "P01").expect("write");
        let out_b = write_meta_xml(&d, "P01").expect("write again");
        assert_eq!(
            out_a, out_b,
            "deterministic UID derivation must produce byte-identical meta XML"
        );
    }

    #[test]
    fn meta_xml_uses_drawing_name_for_version_revision_and_file_node() {
        let d = PublishDrawing::new("UID-1", "TANK-01");
        let out = write_meta_xml(&d, "P01").expect("write");
        assert!(
            out.contains(r#"Name="TANK-01 Version""#),
            "DocumentVersion IObject Name should embed drawing name; out:\n{out}"
        );
        assert!(
            out.contains(r#"Name="TANK-01 Revision""#),
            "DocumentRevision IObject Name should embed drawing name; out:\n{out}"
        );
        assert!(
            out.contains(r#"Name="TANK-01.pid""#),
            "File IObject Name should be `<drawing>.pid`; out:\n{out}"
        );
    }

    #[test]
    fn meta_xml_normalizes_date_created_to_yyyy_mm_dd() {
        let mut d = PublishDrawing::new("UID-1", "A01");
        d.date_created = Some("2026/4/20 10:32:46".into());
        let out = write_meta_xml(&d, "P01").expect("write");
        assert!(
            out.contains(r#"DocVersionDate="2026/04/20""#),
            "OrcaMDF raw date should zero-pad to YYYY/MM/DD; out:\n{out}"
        );
        assert!(
            !out.contains(r#"DocVersionDate="2026/4/20 10:32:46""#),
            "raw timestamp must not appear verbatim; out:\n{out}"
        );
    }

    #[test]
    fn meta_xml_handles_missing_date_with_empty_attribute() {
        let d = PublishDrawing::new("UID-1", "A01");
        let out = write_meta_xml(&d, "P01").expect("write");
        assert!(
            out.contains(r#"DocVersionDate="""#),
            "missing date_created should surface as DocVersionDate=\"\"; out:\n{out}"
        );
    }

    #[test]
    fn meta_xml_rel_uid1_uid2_match_expected_topology() {
        let d = PublishDrawing::new("DUID", "A01");
        let out = write_meta_xml(&d, "P01").expect("write");

        let version_uid = derive_meta_uid("DUID", "version");
        let revision_uid = derive_meta_uid("DUID", "revision");
        let file_uid = derive_meta_uid("DUID", "file");

        // Drawing -> Version (VersionedDoc)
        assert!(
            out.contains(&format!(
                r#"UID1="DUID" UID2="{version_uid}" DefUID="VersionedDoc""#
            )),
            "VersionedDoc rel must wire drawing -> version; out:\n{out}"
        );
        // Revision -> Drawing (RevisedDocument)
        assert!(
            out.contains(&format!(
                r#"UID1="{revision_uid}" UID2="DUID" DefUID="RevisedDocument""#
            )),
            "RevisedDocument rel must wire revision -> drawing; out:\n{out}"
        );
        // File -> Version (FileComposition)
        assert!(
            out.contains(&format!(
                r#"UID1="{file_uid}" UID2="{version_uid}" DefUID="FileComposition""#
            )),
            "FileComposition rel must wire file -> version; out:\n{out}"
        );
    }

    #[test]
    fn derive_meta_uid_is_uppercase_32_hex() {
        let uid = derive_meta_uid("DUID", "version");
        assert_eq!(uid.len(), 32, "derived UID must be 32 hex chars; got {uid}");
        assert!(
            uid.chars().all(|c| c.is_ascii_hexdigit() && (!c.is_ascii_alphabetic() || c.is_ascii_uppercase())),
            "derived UID must be uppercase hex only; got {uid}"
        );
    }

    #[test]
    fn derive_meta_uid_distinguishes_role_within_same_seed() {
        let v = derive_meta_uid("DUID", "version");
        let r = derive_meta_uid("DUID", "revision");
        let f = derive_meta_uid("DUID", "file");
        assert_ne!(v, r);
        assert_ne!(r, f);
        assert_ne!(v, f);
    }

    #[test]
    fn format_meta_date_returns_input_for_unrecognized_shapes() {
        // Anything that doesn't parse as YYYY/M/D is returned as-is
        // so the loader / debugger can still see what came through.
        assert_eq!(format_meta_date("2026-04-20"), "2026-04-20");
        assert_eq!(format_meta_date("not-a-date"), "not-a-date");
        assert_eq!(format_meta_date(""), "");
    }

    // -----------------------------------------------------------------
    // A11 — Note / InstrFunction writers
    // -----------------------------------------------------------------

    #[test]
    fn item_note_emits_pid_note_with_text_from_description() {
        // Note rows use `T_ModelItem.Description` as their primary
        // text source (verified against DWG-0202GP06-01_Data.xml).
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "NOTE-1".into(),
            item_type_name: "Note".into(),
            description: Some("量液孔".into()),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains("<PIDNote>"),
            "Note must render as <PIDNote>; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IObject UID="NOTE-1"/>"#),
            "Note IObject must carry the UID; out:\n{out}"
        );
        assert!(
            out.contains(r#"<INote NoteText="量液孔"/>"#),
            "Description should source NoteText; out:\n{out}"
        );
        assert!(
            out.contains("</PIDNote>"),
            "PIDNote must close; out:\n{out}"
        );
    }

    #[test]
    fn item_note_alias_routes_to_pid_note() {
        // SmartPlant ships the type as both "Note" and "ItemNote"
        // depending on the backup era; both must hit the same writer.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "NOTE-2".into(),
            item_type_name: "ItemNote".into(),
            description: Some("Hello".into()),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains("<PIDNote>"),
            "ItemNote must also dispatch to write_item_note; out:\n{out}"
        );
        assert!(
            out.contains(r#"<INote NoteText="Hello"/>"#),
            "ItemNote text round-trips; out:\n{out}"
        );
    }

    #[test]
    fn item_note_prefers_note_text_field_over_description() {
        // When a fixture stamps an explicit `NoteText` field on the
        // model item (some SmartPlant versions do), that wins over
        // the generic Description column.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "NOTE-3".into(),
            item_type_name: "Note".into(),
            description: Some("ignored".into()),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("NoteText".into(), "explicit-text".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<INote NoteText="explicit-text"/>"#),
            "fields[NoteText] must override Description; out:\n{out}"
        );
        assert!(
            !out.contains(r#"<INote NoteText="ignored"/>"#),
            "Description must not appear when NoteText is set; out:\n{out}"
        );
    }

    #[test]
    fn item_note_with_no_text_renders_empty_note_text_attribute() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "NOTE-4".into(),
            item_type_name: "Note".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<INote NoteText=""/>"#),
            "missing text should render as NoteText=\"\"; out:\n{out}"
        );
    }

    #[test]
    fn item_note_escapes_xml_special_chars_in_note_text() {
        // SmartPlant accepts entity-escaped attribute values, so
        // `<` / `>` / `&` / `"` in note bodies must round-trip
        // cleanly without breaking the document.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "NOTE-5".into(),
            item_type_name: "Note".into(),
            description: Some(r#"a < b & "c" > d"#.into()),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"NoteText="a &lt; b &amp; &quot;c&quot; &gt; d""#),
            "XML special chars must be entity-escaped; out:\n{out}"
        );
    }

    #[test]
    fn instr_function_emits_pid_control_system_function_with_derived_name() {
        // Mirrors DWG-0202GP06-01: MeasuredVariableCode + TagSequenceNo
        // → IObject Name = `LIA-060201`.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-1".into(),
            item_type_name: "InstrFunction".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("MeasuredVariableCode".into(), "LIA".into());
                m.insert("TagSequenceNo".into(), "060201".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains("<PIDControlSystemFunction>"),
            "InstrFunction must render as <PIDControlSystemFunction>; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IObject UID="INSTR-1" Name="LIA-060201"/>"#),
            "Name must combine MeasuredVariable + TagSequenceNo; out:\n{out}"
        );
        assert!(
            out.contains(r#"InstrTagSequenceNo="060201""#),
            "INamedInstrument must carry InstrTagSequenceNo; out:\n{out}"
        );
        assert!(
            out.contains(r#"MeasuredVariable="LIA""#),
            "INamedInstrument must carry MeasuredVariable; out:\n{out}"
        );
        assert!(
            out.contains("</PIDControlSystemFunction>"),
            "PIDControlSystemFunction must close; out:\n{out}"
        );
    }

    #[test]
    fn instrument_alias_dispatches_to_control_system_function() {
        // The sibling `Instrument` ItemTypeName routes to the same
        // writer, matching the SPPID convention where the logical
        // function tag is the one rendered to XML.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-2".into(),
            item_type_name: "Instrument".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("MeasuredVariableCode".into(), "FT".into());
                m.insert("TagSequenceNo".into(), "001".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains("<PIDControlSystemFunction>"),
            "Instrument must also dispatch to PIDControlSystemFunction; out:\n{out}"
        );
        assert!(
            out.contains(r#"Name="FT-001""#),
            "Name composition rule applies regardless of alias; out:\n{out}"
        );
    }

    #[test]
    fn instr_function_falls_back_when_only_measured_variable_present() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-3".into(),
            item_type_name: "InstrFunction".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("MeasuredVariableCode".into(), "LIA".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"Name="LIA""#),
            "missing TagSequenceNo should fall through to bare MeasuredVariable; out:\n{out}"
        );
    }

    #[test]
    fn instr_function_omits_name_attribute_when_no_signals_available() {
        // Without MeasuredVariableCode AND without TagSequenceNo
        // the writer must NOT emit `Name=""` (cosmetic noise);
        // omit the attribute entirely so SmartPlant's defaulting
        // can take over.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-4".into(),
            item_type_name: "InstrFunction".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IObject UID="INSTR-4"/>"#),
            "missing tag info should produce a bare IObject; out:\n{out}"
        );
        assert!(
            !out.contains(r#"Name="""#),
            "writer must not emit empty Name attributes; out:\n{out}"
        );
    }

    // -----------------------------------------------------------------
    // A16 — derived PIDSignalPort children
    // -----------------------------------------------------------------

    #[test]
    fn instr_function_emits_eight_derived_signal_ports() {
        // SmartPlant always derives 8 `<PIDSignalPort>` children
        // per InstrFunction (verified against
        // DWG-0202GP06-01_Data.xml: 2 InstrFunction × 8 ports = 16).
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-1".into(),
            item_type_name: "InstrFunction".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("MeasuredVariableCode".into(), "LIA".into());
                m.insert("TagSequenceNo".into(), "060201".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert_eq!(
            out.matches("<PIDSignalPort>").count(),
            8,
            "single InstrFunction must derive exactly 8 PIDSignalPort; out:\n{out}"
        );
        // Spot-check first / mid / last derived UIDs.
        for index in [1, 4, 8] {
            assert!(
                out.contains(&format!(
                    r#"<IObject UID="INSTR-1.{index}" Name="{index}"/>"#
                )),
                "derived port {index} must carry UID `INSTR-1.{index}` and Name=\"{index}\"; out:\n{out}"
            );
        }
        // Each derived block carries the full 5-interface skeleton.
        assert!(out.contains("<ISignalConnection/>"));
        assert!(out.contains("<ISignalPort/>"));
        assert!(out.contains("<IFacilityPoint/>"));
        assert!(out.contains("<IPIDTypical/>"));
    }

    #[test]
    fn instrument_alias_also_derives_eight_signal_ports() {
        // Both `Instrument` and `InstrFunction` ItemTypeNames must
        // produce the eight derived ports.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-2".into(),
            item_type_name: "Instrument".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert_eq!(out.matches("<PIDSignalPort>").count(), 8);
        assert!(out.contains(r#"<IObject UID="INSTR-2.1" Name="1"/>"#));
        assert!(out.contains(r#"<IObject UID="INSTR-2.8" Name="8"/>"#));
    }

    #[test]
    fn two_instr_functions_yield_sixteen_distinct_signal_ports() {
        // The DWG fixture's 2 × 8 = 16 ports must round-trip
        // distinct UIDs even when the two InstrFunctions live on
        // the same drawing.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![
            PublishObject {
                uid: "INSTR-A".into(),
                item_type_name: "InstrFunction".into(),
                ..PublishObject::default()
            },
            PublishObject {
                uid: "INSTR-B".into(),
                item_type_name: "InstrFunction".into(),
                ..PublishObject::default()
            },
        ];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert_eq!(
            out.matches("<PIDSignalPort>").count(),
            16,
            "two InstrFunctions must derive 2 × 8 = 16 PIDSignalPort; out:\n{out}"
        );
        for prefix in ["INSTR-A", "INSTR-B"] {
            for index in 1..=8u8 {
                assert!(
                    out.contains(&format!(
                        r#"<IObject UID="{prefix}.{index}" Name="{index}"/>"#
                    )),
                    "expected `{prefix}.{index}` in output",
                );
            }
        }
    }

    #[test]
    fn signal_port_derivation_count_matches_constant() {
        // Pin the constant against the writer's runtime behavior so
        // a future tweak that, say, lowers the count to 4 must
        // update both the constant and this assertion in lockstep.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-C".into(),
            item_type_name: "InstrFunction".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert_eq!(
            out.matches("<PIDSignalPort>").count(),
            INSTR_DERIVED_SIGNAL_PORT_COUNT as usize,
        );
    }

    #[test]
    fn signal_ports_appear_after_their_parent_control_system_function() {
        // SPPID-canonical emit order: the parent
        // <PIDControlSystemFunction> closes BEFORE the first
        // <PIDSignalPort> opens.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-ORD".into(),
            item_type_name: "InstrFunction".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");

        let i_function = out
            .find("<PIDControlSystemFunction>")
            .expect("control system function");
        let i_function_close = out
            .find("</PIDControlSystemFunction>")
            .expect("control system function closer");
        let i_first_port = out.find("<PIDSignalPort>").expect("first signal port");

        assert!(
            i_function < i_function_close && i_function_close < i_first_port,
            "PIDSignalPort must appear after the closer of its parent PIDControlSystemFunction; \
             got function={i_function} close={i_function_close} first_port={i_first_port}\nout:\n{out}"
        );
    }

    #[test]
    fn instr_function_named_instrument_passes_through_optional_fields() {
        // Verify all five INamedInstrument attributes round-trip
        // when the loader populates them (T_PlantItem.TagPrefix /
        // T_Instrument.{TagSuffix, LoopTagSuffix, InstrumentTypeModifier}).
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-5".into(),
            item_type_name: "InstrFunction".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("MeasuredVariableCode".into(), "LIA".into());
                m.insert("TagSequenceNo".into(), "060201".into());
                m.insert("TagPrefix".into(), "PFX".into());
                m.insert("TagSuffix".into(), "SFX".into());
                m.insert("LoopTagSuffix".into(), "LP".into());
                m.insert("InstrumentTypeModifier".into(), "MOD".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        for (attr, value) in [
            ("InstrTagPrefix", "PFX"),
            ("InstrTagSequenceNo", "060201"),
            ("InstrTagSuffix", "SFX"),
            ("InstrLoopSuffix", "LP"),
            ("InstrFuncModifier", "MOD"),
            ("MeasuredVariable", "LIA"),
        ] {
            assert!(
                out.contains(&format!(r#"{attr}="{value}""#)),
                "INamedInstrument must carry `{attr}=\"{value}\"`; out:\n{out}"
            );
        }
    }

    // -----------------------------------------------------------------
    // A13 — connector-derived endpoints
    // -----------------------------------------------------------------

    #[test]
    fn piperun_emits_two_derived_piping_ports_and_one_process_point() {
        // SmartPlant's exporter derives `<PIDPipingPort>.1`,
        // `<PIDPipingPort>.2`, and `<PIDProcessPoint>.PPT` from
        // every PipingConnector. The composition is purely
        // SmartPlant-side — no SQLite row carries those UIDs.
        let mut d = PublishDrawing::new("UID-A01", "A01");
        d.objects = vec![PublishObject {
            uid: "PIPE-1".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("NominalDiameter".into(), "250".into());
                m.insert("PipingMaterialsClass".into(), "B5".into());
                m.insert("TagSequenceNo".into(), "0102".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");

        // The pipeline + connector + 2 ports + 1 process point all
        // fire from this single PipeRun row.
        assert_eq!(
            out.matches("<PIDPipingPort>").count(),
            2,
            "PipeRun must emit exactly 2 derived <PIDPipingPort> nodes; out:\n{out}"
        );
        assert_eq!(
            out.matches("<PIDProcessPoint>").count(),
            1,
            "PipeRun must emit exactly 1 derived <PIDProcessPoint> node; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IObject UID="PIPE-1-CNX.1" Name="1"/>"#),
            "first derived port UID is `<connector>.1` with Name=\"1\"; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IObject UID="PIPE-1-CNX.2" Name="2"/>"#),
            "second derived port UID is `<connector>.2` with Name=\"2\"; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IObject UID="PIPE-1-CNX.PPT"/>"#),
            "derived process point UID is `<connector>.PPT`; out:\n{out}"
        );
        // Both derived ports inherit the connector's nominal
        // diameter (with the `mm` unit applied by format_diameter).
        assert_eq!(
            out.matches(r#"<IPipeCrossSectionItem NominalDiameter="250 mm"/>"#)
                .count(),
            3,
            "two derived ports + the connector itself carry NominalDiameter; out:\n{out}"
        );
    }

    #[test]
    fn piperun_with_no_nominal_diameter_still_derives_three_endpoints() {
        // Even when the upstream row has no diameter, the writer
        // must still derive the three virtual endpoints (with an
        // empty NominalDiameter attribute) so the Rel topology in
        // the eventual `_Data.xml` cross-references can resolve.
        let mut d = PublishDrawing::new("UID-A01", "A01");
        d.objects = vec![PublishObject {
            uid: "PIPE-EMPTY".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert_eq!(out.matches("<PIDPipingPort>").count(), 2);
        assert_eq!(out.matches("<PIDProcessPoint>").count(), 1);
        assert!(out.contains(r#"<IObject UID="PIPE-EMPTY-CNX.1" Name="1"/>"#));
        assert!(out.contains(r#"<IObject UID="PIPE-EMPTY-CNX.PPT"/>"#));
    }

    #[test]
    fn derived_endpoints_appear_after_connector_in_emit_order() {
        // SmartPlant's reference fixture emits the five PipeRun
        // children in this exact order: PIDPipeline,
        // PIDPipingConnector, PIDPipingPort×2, PIDProcessPoint.
        // Tests pin the order so a future refactor cannot quietly
        // swap them.
        let mut d = PublishDrawing::new("UID-A01", "A01");
        d.objects = vec![PublishObject {
            uid: "PIPE-ORD".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");

        let i_pipeline = out.find("<PIDPipeline>").expect("pipeline present");
        let i_connector = out
            .find("<PIDPipingConnector>")
            .expect("connector present");
        let i_first_port = out.find("<PIDPipingPort>").expect("first port");
        let i_process_point = out
            .find("<PIDProcessPoint>")
            .expect("process point present");

        assert!(
            i_pipeline < i_connector
                && i_connector < i_first_port
                && i_first_port < i_process_point,
            "emit order must be Pipeline < Connector < Port(.1) < ProcessPoint; got positions \
             pipeline={i_pipeline} connector={i_connector} first_port={i_first_port} \
             process_point={i_process_point}\nout:\n{out}"
        );
    }

    // -----------------------------------------------------------------
    // A14 — annotation/label representation filtering
    // -----------------------------------------------------------------

    #[test]
    fn write_representations_emits_one_pid_representation_per_publishable_row() {
        // The reference SmartPlant exporter emits PIDRepresentation
        // ONLY for representations that point at a model item.
        // Three reps wired to model items, two pure annotations
        // (model_item_uid None / Some("")), one with valid uid:
        // expect exactly four `<PIDRepresentation>` opens.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.representations = vec![
            PublishRepresentation {
                uid: "REP-1".into(),
                model_item_uid: Some("OBJ-1".into()),
                drawing_uid: "UID-D".into(),
                graphic_oid: Some(1),
                ..PublishRepresentation::default()
            },
            PublishRepresentation {
                uid: "REP-2-LABEL".into(),
                model_item_uid: None,
                drawing_uid: "UID-D".into(),
                graphic_oid: Some(2),
                ..PublishRepresentation::default()
            },
            PublishRepresentation {
                uid: "REP-3".into(),
                model_item_uid: Some("OBJ-3".into()),
                drawing_uid: "UID-D".into(),
                graphic_oid: Some(3),
                ..PublishRepresentation::default()
            },
            PublishRepresentation {
                uid: "REP-4-EMPTY".into(),
                model_item_uid: Some(String::new()),
                drawing_uid: "UID-D".into(),
                graphic_oid: Some(4),
                ..PublishRepresentation::default()
            },
            PublishRepresentation {
                uid: "REP-5".into(),
                model_item_uid: Some("OBJ-5".into()),
                drawing_uid: "UID-D".into(),
                graphic_oid: None,
                ..PublishRepresentation::default()
            },
        ];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert_eq!(
            out.matches("<PIDRepresentation>").count(),
            3,
            "only the three reps with non-empty model_item_uid should produce <PIDRepresentation>; out:\n{out}"
        );
        // Spot-check the three publishable UIDs are present.
        for uid in ["REP-1", "REP-3", "REP-5"] {
            assert!(
                out.contains(&format!(r#"<IObject UID="{uid}"/>"#)),
                "publishable rep `{uid}` must be present; out:\n{out}"
            );
        }
        // The two annotation-only reps must NOT appear.
        for uid in ["REP-2-LABEL", "REP-4-EMPTY"] {
            assert!(
                !out.contains(&format!(r#"<IObject UID="{uid}"/>"#)),
                "annotation rep `{uid}` must NOT be present; out:\n{out}"
            );
        }
    }

    #[test]
    fn drawing_items_rel_only_emitted_for_publishable_representations() {
        // The DrawingItems derived rel must follow the same filter
        // — otherwise the Rel section would dangle pointers to
        // PIDRepresentation tags we never wrote.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.representations = vec![
            PublishRepresentation {
                uid: "REP-OK".into(),
                model_item_uid: Some("OBJ-1".into()),
                drawing_uid: "UID-D".into(),
                ..PublishRepresentation::default()
            },
            PublishRepresentation {
                uid: "REP-LABEL".into(),
                model_item_uid: None,
                drawing_uid: "UID-D".into(),
                ..PublishRepresentation::default()
            },
        ];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"DefUID="DrawingItems""#),
            "DrawingItems rel for the publishable rep must be present; out:\n{out}"
        );
        // The dangling rel for the annotation rep must NOT exist.
        assert!(
            !out.contains("REP-LABEL"),
            "annotation rep UID must not appear in any DrawingItems / DwgRepresentationComposition rel; out:\n{out}"
        );
    }

    #[test]
    fn representation_is_publishable_classifier_unit_table() {
        // Pure helper test for the predicate that drives both the
        // representation block and the derived rels.
        let publishable = PublishRepresentation {
            uid: "REP".into(),
            model_item_uid: Some("OBJ".into()),
            drawing_uid: "UID".into(),
            ..PublishRepresentation::default()
        };
        let no_model = PublishRepresentation {
            uid: "REP".into(),
            model_item_uid: None,
            drawing_uid: "UID".into(),
            ..PublishRepresentation::default()
        };
        let empty_model = PublishRepresentation {
            uid: "REP".into(),
            model_item_uid: Some(String::new()),
            drawing_uid: "UID".into(),
            ..PublishRepresentation::default()
        };
        assert!(representation_is_publishable(&publishable));
        assert!(!representation_is_publishable(&no_model));
        assert!(!representation_is_publishable(&empty_model));
    }

    #[test]
    fn unsupported_item_types_still_fall_back_to_generic_placeholder() {
        // Exchanger / Mechanical have subtables registered but no
        // dedicated writer yet (TODO A12+); ensure they continue to
        // emit through the generic dispatch instead of being
        // silently dropped.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "EX-1".into(),
            item_type_name: "Exchanger".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains("<PIDItem>"),
            "Exchanger should still hit the generic <PIDItem> fallback; out:\n{out}"
        );
        assert!(
            out.contains("`Exchanger`"),
            "the generic comment should name the unsupported type; out:\n{out}"
        );
    }
}
