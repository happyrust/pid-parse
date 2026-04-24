//! Publish-Data XML writer — DTO → SmartPlant-compatible XML.
//!
//! Current scope:
//!
//! * emits both `_Data.xml` and `_Meta.xml`;
//! * covers the 15 PID tag families currently declared in
//!   `publish::supported_pid_tags()` plus the drawing-scoped
//!   derived nodes already modeled on the DTO side
//!   (`PIDPipingPort`, `PIDProcessPoint`, `PIDSignalPort`);
//! * preserves the explicit `PublishStyle::{A01,Dwg}` selector
//!   rather than auto-detecting plant flavor.
//!
//! The remaining publish backlog is concentrated in DWG-mirror-
//! gated work: loader canonical-field enrichment for DWG-only
//! attributes and closing the A24/A27b tolerated divergences.
//! The `PIDBranchPoint` and `PIDPipingBranchPoint` writer arms
//! are implemented (Stage-4) but the loader-side item-type
//! mapping is provisional until the DWG mirror confirms it.
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
//! * Writer-synthesized publish-only identifiers are
//!   deterministic, so repeated exports remain byte-stable.
//!   A01 raw parity is already closed at the contract level, but
//!   the connector-family UID, `<Rel><IObject UID="..."/>`, and
//!   `GraphicOID` publish numbering still use writer-side
//!   placeholder strategies pending full SmartPlant-rule
//!   reconstruction.

use std::collections::HashMap;
use std::fmt::Write;

use super::model::{
    PublishDrawing, PublishError, PublishObject, PublishRelationship, PublishRepresentation,
    PublishStyle,
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
    writeln!(buf, "  </Container>").map_err(fmt_err)?;
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
    for obj in ordered_business_objects(drawing) {
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
                write_pipeline(buf, obj, drawing.style)?;
                write_piping_connector(buf, obj, drawing.style)?;
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
            // A17: PipingComp → `<PIDPipingComponent>`. SQLite
            // loader already stitches T_PlantItem + T_InlineComp +
            // T_PipingComp fields onto the `PublishObject`; the
            // writer renders the 19-interface shape observed in
            // DWG-0202GP06-01_Data.xml (Cap / Conduit gate valve
            // samples). Closes PIDPipingComponent × 2 in the A15
            // backlog.
            "PipingComp" => write_piping_component(buf, obj)?,
            // A18: SignalRun → `<PIDSignalConnector>`. The
            // signal-side counterpart of PipeRun → PIDPipingConnector.
            // The DWG-0202GP06-01 fixture ships 1 SignalRun row
            // whose XML shape is deliberately minimal (8
            // interfaces, no IPBSItem business envelope) because
            // SmartPlant treats signal connectors as pure wiring
            // overlays rather than piped facilities.
            "SignalRun" => write_signal_connector(buf, obj)?,
            "PipingBranchPoint" => write_piping_branch_point(buf, obj)?,
            "BranchPoint" => write_pid_branch_point(buf, obj)?,
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

fn ordered_business_objects(drawing: &PublishDrawing) -> Vec<&PublishObject> {
    let mut objects: Vec<&PublishObject> = drawing.objects.iter().collect();
    if matches!(drawing.style, PublishStyle::A01) {
        objects.sort_by_key(|obj| a01_object_rank(obj.item_type_name.as_str()));
    }
    objects
}

fn ordered_publishable_representations(drawing: &PublishDrawing) -> Vec<&PublishRepresentation> {
    let mut reps: Vec<&PublishRepresentation> = drawing
        .representations
        .iter()
        .filter(|rep| representation_is_publishable(rep))
        .collect();
    if matches!(drawing.style, PublishStyle::A01) {
        let rank_by_object: HashMap<&str, u8> = drawing
            .objects
            .iter()
            .map(|obj| {
                (
                    obj.uid.as_str(),
                    a01_object_rank(obj.item_type_name.as_str()),
                )
            })
            .collect();
        reps.sort_by(|a, b| {
            let a_rank = a
                .model_item_uid
                .as_deref()
                .and_then(|uid| rank_by_object.get(uid).copied())
                .unwrap_or(u8::MAX);
            let b_rank = b
                .model_item_uid
                .as_deref()
                .and_then(|uid| rank_by_object.get(uid).copied())
                .unwrap_or(u8::MAX);
            a_rank
                .cmp(&b_rank)
                .then_with(|| b.graphic_oid.cmp(&a.graphic_oid))
                .then_with(|| a.uid.cmp(&b.uid))
        });
    }
    reps
}

fn a01_object_rank(item_type_name: &str) -> u8 {
    match item_type_name {
        "Vessel" => 0,
        "Nozzle" => 1,
        "PipeRun" => 2,
        _ => 9,
    }
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

fn non_empty_field<'a>(obj: &'a PublishObject, key: &str) -> Option<&'a str> {
    obj.fields
        .get(key)
        .map(String::as_str)
        .filter(|s| !s.trim().is_empty())
}

fn non_empty_field_any<'a>(obj: &'a PublishObject, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| non_empty_field(obj, key))
}

fn dwg_field_with_aliases<'a>(
    obj: &'a PublishObject,
    style: PublishStyle,
    canonical_key: &str,
    dwg_aliases: &[&str],
) -> Option<&'a str> {
    non_empty_field(obj, canonical_key).or_else(|| {
        if matches!(style, PublishStyle::Dwg) {
            non_empty_field_any(obj, dwg_aliases)
        } else {
            None
        }
    })
}

fn canonical_construction_status(obj: &PublishObject, style: PublishStyle) -> String {
    match (
        style,
        obj.fields.get("ConstructionStatus").map(|s| s.trim()),
    ) {
        (PublishStyle::A01, None | Some("") | Some("2")) => "@NewConstruction".to_string(),
        (_, Some(value)) => value.to_string(),
        (_, None) => "@NewConstruction".to_string(),
    }
}

fn canonical_construction_status2(obj: &PublishObject, style: PublishStyle) -> String {
    match (
        style,
        obj.fields.get("ConstructionStatus2").map(|s| s.trim()),
    ) {
        (PublishStyle::A01, None | Some("")) => {
            "@{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}".to_string()
        }
        (_, Some(value)) => value.to_string(),
        (_, None) => "@{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}".to_string(),
    }
}

fn canonical_is_typical(obj: &PublishObject, style: PublishStyle) -> &'static str {
    match style {
        PublishStyle::A01 => "False",
        PublishStyle::Dwg => obj.is_typical.as_deref().map_or("False", map_bool),
    }
}

fn canonical_vessel_item_tag(obj: &PublishObject, style: PublishStyle) -> String {
    match style {
        PublishStyle::A01 => {
            let formatted = format_equipment_tag(obj);
            if formatted.is_empty() {
                obj.fields.get("ItemTag").cloned().unwrap_or_default()
            } else {
                formatted
            }
        }
        PublishStyle::Dwg => obj
            .fields
            .get("ItemTag")
            .cloned()
            .unwrap_or_else(|| format_equipment_tag(obj)),
    }
}

fn canonical_pipeline_item_tag(obj: &PublishObject, style: PublishStyle) -> String {
    if matches!(style, PublishStyle::A01) {
        // A01 Publish Data exposes the expanded pipe tag even
        // when T_PlantItem.ItemTag carries the shorter catalog tag
        // (TEST02 stores `A010102102-PH`). Prefer the fully
        // reconstructed publish form when the PipeRun fields are
        // present, then fall back to ItemTag for partial fixtures.
        let seq = non_empty_field(obj, "TagSequenceNo").unwrap_or("");
        let dia = non_empty_field(obj, "NominalDiameter")
            .map(format_diameter)
            .unwrap_or_default();
        let class = non_empty_field(obj, "PipingMaterialsClass").unwrap_or("");
        let insul = non_empty_field(obj, "InsulThick")
            .map(format_insulation_inches)
            .unwrap_or_default();
        if !seq.is_empty() && !dia.is_empty() && !class.is_empty() && !insul.is_empty() {
            return format!("PH- {seq}-DN{dia}-{class}-P-{insul}");
        }
    }
    if let Some(tag) = obj.fields.get("ItemTag") {
        if !tag.is_empty() {
            return tag.clone();
        }
    }
    resolve_pipe_item_tag(obj)
}

fn canonical_connector_item_tag(obj: &PublishObject, style: PublishStyle) -> String {
    if matches!(style, PublishStyle::A01) {
        // Connector tags in A01 use the same PipeRun business
        // fields as the pipeline, but without the DN/insulation
        // adornments. Keep ItemTag as the compatibility fallback.
        let seq = non_empty_field(obj, "TagSequenceNo").unwrap_or("");
        let dia = non_empty_field(obj, "NominalDiameter")
            .map(format_diameter)
            .unwrap_or_default();
        let class = non_empty_field(obj, "PipingMaterialsClass").unwrap_or("");
        if !seq.is_empty() && !dia.is_empty() && !class.is_empty() {
            return format!("PH-{seq}-{dia}-{class}");
        }
    }
    if let Some(tag) = obj.fields.get("ItemTag") {
        if !tag.is_empty() {
            return tag.clone();
        }
    }
    resolve_pipe_item_tag(obj)
}

/// Derive the `<PIDPipingConnector>` IObject UID from the parent
/// `<PIDPipeline>` (PipeRun) UID.
///
/// Current state: this still uses the stage-1 placeholder
/// convention `<pipe_uid>-CNX`, with downstream `.1` / `.2` /
/// `.PPT` children appended on top. That keeps the document
/// internally self-consistent and byte-stable, but it is NOT yet
/// the real SmartPlant publish-time numbering rule. The A01 raw
/// residual gates intentionally keep this family under explicit
/// burn-down until the true rule is reconstructed from TEST02.
fn derived_pipe_connector_uid(pipe_uid: &str) -> String {
    format!("{pipe_uid}-CNX")
}

/// Emit the full `<PIDProcessVessel>` block for a Vessel row.
///
/// The reference shape has 15 interfaces, confirmed byte-for-byte
/// against both `A01_Data.xml:12–28` and
/// `DWG-0202GP06-01_Data.xml:1429–1447`.
///
/// A21 closes a 5-interface fidelity gap: the pre-A21 writer
/// emitted only 10 interfaces (IObject, IPIDProcessVesselOcc,
/// IProcessVesselOcc, IEquipment, IEquipmentOcc, IPBSItem,
/// IProcessEquipment, IProcessVessel, IPIDProcessVessel,
/// IPIDTypical). The five missing wrapper interfaces are now
/// emitted in SPPID-canonical order:
///
/// * `IPBSItemCollection` (between IPBSItem and IPlannedMatl)
/// * `IPlannedMatl`
/// * `IProcessEquipmentOcc` (next to IProcessEquipment)
/// * `IDrawingItem` (between IProcessVessel and IPIDProcessVessel)
/// * `ISpecifiedMatlItem` (after IPIDProcessVessel)
///
/// A21 also populates:
/// * `IPBSItem ConstructionStatus="..." ConstructionStatus2="..."`
///   with the same SPPID canonical defaults used for the
///   PipingComponent/PipingConnector writers (overridable via
///   `obj.fields["ConstructionStatus"]` /
///   `["ConstructionStatus2"]`).
/// * Optional DWG-specific attributes:
///   - `IPBSItem HeightRelativeToGrade` from
///     `obj.fields["HeightRelativeToGrade"]`.
///   - `IEquipment EqType0/1/2/3 + EquipmentTrimSpec` from the
///     corresponding `T_Vessel` / `T_Equipment` columns.
///   - `IProcessVessel ProcessVessel_VesselVolumetricCapacity`
///     from `obj.fields["VesselVolumetricCapacity"]`.
///   - `ISpecifiedMatlItem LongMaterialDescription` from
///     `obj.fields["LongMaterialDescription"]`.
///
/// Optional attributes render as empty / bare when absent,
/// preserving A01 byte-shape compatibility; render populated
/// when loader-side fields arrive.
///
/// A25 closes the "tank variant" shape gap A24 discovered:
/// DWG-style "Open top tank" vessel variants (EqType1="@EE793"
/// / EqType0="@{47BF0267-DD41-4E1A-9B41-C4B714C8FF92}") emit
/// two extra interfaces between `IPIDProcessVessel` and
/// `ISpecifiedMatlItem` — `ILowPressureTankOcc` then
/// `ILowPressureTank` — while "Horizontal Drum" / non-tank
/// variants do not. The writer routes a loader-side
/// `obj.fields["IsLowPressureTank"]` boolean-ish signal into
/// this conditional emission so the same writer produces the
/// 15-interface A01 shape AND the 17-interface DWG-tank
/// shape, bit-for-bit. The loader side (inferring the flag
/// from T_ProcessEquipment's EqType columns) is deferred to
/// A25b — until then, callers synthesising PublishObjects
/// manually can set the field directly.
fn write_process_vessel(
    buf: &mut String,
    obj: &PublishObject,
    drawing: &PublishDrawing,
) -> Result<(), PublishError> {
    let item_tag = canonical_vessel_item_tag(obj, drawing.style);
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
    let construction_status = canonical_construction_status(obj, drawing.style);
    let construction_status2 = canonical_construction_status2(obj, drawing.style);
    let height_relative_to_grade = obj
        .fields
        .get("HeightRelativeToGrade")
        .cloned()
        .unwrap_or_default();
    let eq_type0 = obj.fields.get("EqType0").cloned().unwrap_or_default();
    let eq_type1 = obj.fields.get("EqType1").cloned().unwrap_or_default();
    let eq_type2 = obj.fields.get("EqType2").cloned().unwrap_or_default();
    let eq_type3 = obj.fields.get("EqType3").cloned().unwrap_or_default();
    let equipment_trim_spec =
        dwg_field_with_aliases(obj, drawing.style, "EquipmentTrimSpec", &["TrimSpec"])
            .unwrap_or_default()
            .to_string();
    let vessel_volumetric_capacity = dwg_field_with_aliases(
        obj,
        drawing.style,
        "VesselVolumetricCapacity",
        &["VolumeRating"],
    )
    .unwrap_or_default()
    .to_string();
    let long_material_description = obj
        .fields
        .get("LongMaterialDescription")
        .cloned()
        .unwrap_or_default();
    // A25 · Low-pressure-tank variant flag. The writer uses
    // `map_bool` (shared with other boolean-ish passthrough
    // attributes like `IsFlowDirectional` / `IsTypical`) so
    // that explicit "False" / "0" / "" stays in the non-tank
    // branch, keeping the default behavior stable for callers
    // that pre-populate the field unconditionally.
    let is_low_pressure_tank = obj
        .fields
        .get("IsLowPressureTank")
        .is_some_and(|v| map_bool(v) == "True");
    writeln!(buf, "   <PIDProcessVessel>").map_err(fmt_err)?;
    write_process_vessel_iobject(buf, &obj.uid, &item_tag, description, drawing.style)?;
    writeln!(buf, "      <IPIDProcessVesselOcc/>").map_err(fmt_err)?;
    writeln!(buf, "      <IProcessVesselOcc/>").map_err(fmt_err)?;
    // IEquipment renders EqTypeDescription always; EqType0-3 and
    // EquipmentTrimSpec render as an additional leading block
    // when any of them is populated (DWG shape).
    if eq_type0.is_empty()
        && eq_type1.is_empty()
        && eq_type2.is_empty()
        && eq_type3.is_empty()
        && equipment_trim_spec.is_empty()
    {
        writeln!(
            buf,
            r#"      <IEquipment EqTypeDescription="{}"/>"#,
            escape_attr(&eq_type_description)
        )
        .map_err(fmt_err)?;
    } else {
        writeln!(
            buf,
            concat!(
                r#"      <IEquipment EqType0="{}" EqType3="{}" EqType2="{}" "#,
                r#"EqType1="{}" EquipmentTrimSpec="{}" EqTypeDescription="{}"/>"#,
            ),
            escape_attr(&eq_type0),
            escape_attr(&eq_type3),
            escape_attr(&eq_type2),
            escape_attr(&eq_type1),
            escape_attr(&equipment_trim_spec),
            escape_attr(&eq_type_description),
        )
        .map_err(fmt_err)?;
    }
    writeln!(buf, "      <IEquipmentOcc/>").map_err(fmt_err)?;
    // IPBSItem: defaults to A17-canonical defaults; gains
    // HeightRelativeToGrade when populated (DWG shape).
    if height_relative_to_grade.is_empty() {
        writeln!(
            buf,
            r#"      <IPBSItem ConstructionStatus="{}" ConstructionStatus2="{}"/>"#,
            escape_attr(&construction_status),
            escape_attr(&construction_status2),
        )
        .map_err(fmt_err)?;
    } else {
        writeln!(
            buf,
            r#"      <IPBSItem HeightRelativeToGrade="{}" ConstructionStatus="{}" ConstructionStatus2="{}"/>"#,
            escape_attr(&height_relative_to_grade),
            escape_attr(&construction_status),
            escape_attr(&construction_status2),
        )
        .map_err(fmt_err)?;
    }
    writeln!(buf, "      <IPBSItemCollection/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPlannedMatl/>").map_err(fmt_err)?;
    writeln!(buf, "      <IProcessEquipment/>").map_err(fmt_err)?;
    writeln!(buf, "      <IProcessEquipmentOcc/>").map_err(fmt_err)?;
    if vessel_volumetric_capacity.is_empty() {
        writeln!(buf, "      <IProcessVessel/>").map_err(fmt_err)?;
    } else {
        writeln!(
            buf,
            r#"      <IProcessVessel ProcessVessel_VesselVolumetricCapacity="{}"/>"#,
            escape_attr(&vessel_volumetric_capacity),
        )
        .map_err(fmt_err)?;
    }
    writeln!(buf, "      <IDrawingItem/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPIDProcessVessel/>").map_err(fmt_err)?;
    if is_low_pressure_tank {
        writeln!(buf, "      <ILowPressureTankOcc/>").map_err(fmt_err)?;
        writeln!(buf, "      <ILowPressureTank/>").map_err(fmt_err)?;
    }
    if long_material_description.is_empty() {
        writeln!(buf, "      <ISpecifiedMatlItem/>").map_err(fmt_err)?;
    } else {
        writeln!(
            buf,
            r#"      <ISpecifiedMatlItem LongMaterialDescription="{}"/>"#,
            escape_attr(&long_material_description),
        )
        .map_err(fmt_err)?;
    }
    writeln!(
        buf,
        r#"      <IPIDTypical IsTypical="{}"/>"#,
        canonical_is_typical(obj, drawing.style)
    )
    .map_err(fmt_err)?;
    writeln!(buf, "   </PIDProcessVessel>").map_err(fmt_err)
}

/// Emit the full `<PIDNozzle>` block for a Nozzle row.
///
/// The reference shape has 22 interfaces, confirmed byte-for-byte
/// against both `A01_Data.xml:29–53` and DWG sample 117–141.
///
/// A22 closes a 13-interface fidelity gap: pre-A22 writer emitted
/// only 9 interfaces (IObject, IPipingPortComposition, INozzleOcc,
/// INozzle, IEquipmentComponent, IEquipmentComponentOcc,
/// IPipeCrossSectionItem, IPipingSpecifiedItem, IPIDTypical).
/// The 13 missing wrapper interfaces are now emitted in SPPID
/// canonical order:
///
/// * `IPBSItem ConstructionStatus=... ConstructionStatus2=...`
///   (inserted right after IObject with A17-canonical defaults).
/// * `IPlannedMatl`, `IDrawingItem` (before the per-nozzle
///   INozzleOcc / INozzle pair).
/// * `IFabricatedItem`, `IHeatTracedItem HTraceRqmt="..."` (after
///   IEquipmentComponentOcc).
/// * `IPBSItemCollection`, `IProcessPointCollection`,
///   `ISignalPortComposition`, `IPartOcc`, `IDocumentItem`,
///   `IElecPowerConsumer`, `IPart`, `INoteCollection`,
///   `IProcessDataCaseComposition` (between IHeatTracedItem and
///   IPipeCrossSectionItem).
///
/// A22 also upgrades:
/// * `IEquipmentComponent` — expanded-attr DWG form gains
///   `ProcessEqCompType1` + `ProcessEqCompType2` leading
///   attributes when the loader populates the corresponding
///   T_Nozzle columns. A01 shape stays single-attribute
///   (`ProcEqpCompTypeDescription` alone).
/// * `IPipeCrossSectionItem` — now renders bare (A01) when
///   NominalDiameter is absent; with the attribute (DWG) when
///   populated. Pre-A22 forced an empty attribute even when
///   absent, diverging from the A01 bare shape.
/// * `IPipingSpecifiedItem` — same conditional path for
///   `PipingMaterialsClass` (bare in A01; populated in DWG).
fn write_nozzle(
    buf: &mut String,
    obj: &PublishObject,
    drawing: &PublishDrawing,
) -> Result<(), PublishError> {
    let nominal_diameter = if matches!(drawing.style, PublishStyle::A01) {
        String::new()
    } else {
        obj.fields
            .get("NominalDiameter")
            .cloned()
            .map(|v| format_diameter(&v))
            .unwrap_or_default()
    };
    let piping_materials_class = if matches!(drawing.style, PublishStyle::A01) {
        String::new()
    } else {
        obj.fields
            .get("PipingMaterialsClass")
            .cloned()
            .unwrap_or_default()
    };
    let construction_status = canonical_construction_status(obj, drawing.style);
    let construction_status2 = canonical_construction_status2(obj, drawing.style);
    let htrace_rqmt = obj.fields.get("HTraceRqmt").cloned().unwrap_or_default();
    let process_eq_comp_type1 = obj
        .fields
        .get("ProcessEqCompType1")
        .cloned()
        .unwrap_or_default();
    let process_eq_comp_type2 = obj
        .fields
        .get("ProcessEqCompType2")
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
    writeln!(buf, r#"      <IObject UID="{}"/>"#, escape_attr(&obj.uid)).map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IPBSItem ConstructionStatus="{}" ConstructionStatus2="{}"/>"#,
        escape_attr(&construction_status),
        escape_attr(&construction_status2),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IPipingPortComposition/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPlannedMatl/>").map_err(fmt_err)?;
    writeln!(buf, "      <IDrawingItem/>").map_err(fmt_err)?;
    writeln!(buf, "      <INozzleOcc/>").map_err(fmt_err)?;
    writeln!(buf, "      <INozzle/>").map_err(fmt_err)?;
    if process_eq_comp_type1.is_empty() && process_eq_comp_type2.is_empty() {
        writeln!(
            buf,
            r#"      <IEquipmentComponent ProcEqpCompTypeDescription="{}"/>"#,
            escape_attr(&proc_eq_comp_description)
        )
        .map_err(fmt_err)?;
    } else {
        writeln!(
            buf,
            r#"      <IEquipmentComponent ProcessEqCompType1="{}" ProcessEqCompType2="{}" ProcEqpCompTypeDescription="{}"/>"#,
            escape_attr(&process_eq_comp_type1),
            escape_attr(&process_eq_comp_type2),
            escape_attr(&proc_eq_comp_description),
        )
        .map_err(fmt_err)?;
    }
    writeln!(buf, "      <IEquipmentComponentOcc/>").map_err(fmt_err)?;
    writeln!(buf, "      <IFabricatedItem/>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IHeatTracedItem HTraceRqmt="{}"/>"#,
        escape_attr(&htrace_rqmt),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IPBSItemCollection/>").map_err(fmt_err)?;
    writeln!(buf, "      <IProcessPointCollection/>").map_err(fmt_err)?;
    writeln!(buf, "      <ISignalPortComposition/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPartOcc/>").map_err(fmt_err)?;
    writeln!(buf, "      <IDocumentItem/>").map_err(fmt_err)?;
    writeln!(buf, "      <IElecPowerConsumer/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPart/>").map_err(fmt_err)?;
    writeln!(buf, "      <INoteCollection/>").map_err(fmt_err)?;
    writeln!(buf, "      <IProcessDataCaseComposition/>").map_err(fmt_err)?;
    if nominal_diameter.is_empty() {
        writeln!(buf, "      <IPipeCrossSectionItem/>").map_err(fmt_err)?;
    } else {
        writeln!(
            buf,
            r#"      <IPipeCrossSectionItem NominalDiameter="{}"/>"#,
            escape_attr(&nominal_diameter)
        )
        .map_err(fmt_err)?;
    }
    if piping_materials_class.is_empty() {
        writeln!(buf, "      <IPipingSpecifiedItem/>").map_err(fmt_err)?;
    } else {
        writeln!(
            buf,
            r#"      <IPipingSpecifiedItem PipingMaterialsClass="{}"/>"#,
            escape_attr(&piping_materials_class)
        )
        .map_err(fmt_err)?;
    }
    writeln!(
        buf,
        r#"      <IPIDTypical IsTypical="{}"/>"#,
        canonical_is_typical(obj, drawing.style)
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
    let tag_sequence = obj.fields.get("TagSequenceNo").map_or("", String::as_str);
    if !tag_sequence.is_empty() {
        let piping_materials_class = obj
            .fields
            .get("PipingMaterialsClass")
            .map_or("", String::as_str);
        let nominal_diameter = obj
            .fields
            .get("NominalDiameter")
            .map(|v| format_diameter(v))
            .unwrap_or_default();
        return format!("PH-{tag_sequence}-{nominal_diameter}-{piping_materials_class}");
    }
    obj.uid.clone()
}

/// A29 · Render `<IObject>` for the `<PIDPipeline>` body.
///
/// Two attribute conventions:
///
/// * **A01 style** (default) — preserves the pre-A29
///   shape: when `pipeline_name` is populated the IObject
///   emits all three of `UID` / `Name` / `ItemTag`; when
///   absent, only `UID` / `ItemTag`. This is the strict
///   superset shape that has been live since A19, so every
///   pre-A29 caller / fixture round-trips bit-for-bit.
/// * **DWG style** — the IObject drops `ItemTag` to match
///   the DWG reference (`<IObject UID="..." Name="..."/>`
///   two-attribute shape). When `pipeline_name` is absent
///   the writer emits a UID-only IObject; the DWG fixture
///   itself only ships pipelines with names, so the
///   UID-only branch is purely defensive against malformed
///   input.
fn write_pipeline_iobject(
    buf: &mut String,
    uid: &str,
    item_tag: &str,
    pipeline_name: Option<&str>,
    style: PublishStyle,
) -> Result<(), PublishError> {
    let name = pipeline_name.filter(|s| !s.is_empty());
    match (style, name) {
        (PublishStyle::A01, Some(name)) => writeln!(
            buf,
            r#"      <IObject UID="{}" Name="{}" ItemTag="{}"/>"#,
            escape_attr(uid),
            escape_attr(name),
            escape_attr(item_tag),
        ),
        (PublishStyle::A01, None) => writeln!(
            buf,
            r#"      <IObject UID="{}" ItemTag="{}"/>"#,
            escape_attr(uid),
            escape_attr(item_tag),
        ),
        (PublishStyle::Dwg, Some(name)) => writeln!(
            buf,
            r#"      <IObject UID="{}" Name="{}"/>"#,
            escape_attr(uid),
            escape_attr(name),
        ),
        (PublishStyle::Dwg, None) => {
            writeln!(buf, r#"      <IObject UID="{}"/>"#, escape_attr(uid),)
        }
    }
    .map_err(fmt_err)
}

/// A29 · Render `<IObject>` for the `<PIDPipingConnector>`
/// body. Same A01 / DWG split as
/// [`write_pipeline_iobject`], but the connector's A01
/// shape is a strict two-attribute IObject (no `Name`
/// attribute even when the loader has the data) — this
/// matches the connector's pre-A29 behavior, which only
/// flipped to Name-style IObject when PipelineName was
/// populated. A29 makes the flip an explicit
/// `style = Dwg` decision rather than an implicit field-
/// presence side-effect.
fn write_piping_connector_iobject(
    buf: &mut String,
    uid: &str,
    item_tag: &str,
    pipeline_name: Option<&str>,
    style: PublishStyle,
) -> Result<(), PublishError> {
    let name = pipeline_name.filter(|s| !s.is_empty());
    match (style, name) {
        (PublishStyle::A01, _) => writeln!(
            buf,
            r#"      <IObject UID="{}" ItemTag="{}"/>"#,
            escape_attr(uid),
            escape_attr(item_tag),
        ),
        (PublishStyle::Dwg, Some(name)) => writeln!(
            buf,
            r#"      <IObject UID="{}" Name="{}"/>"#,
            escape_attr(uid),
            escape_attr(name),
        ),
        (PublishStyle::Dwg, None) => {
            writeln!(buf, r#"      <IObject UID="{}"/>"#, escape_attr(uid),)
        }
    }
    .map_err(fmt_err)
}

/// A29 · Render `<IObject>` for the `<PIDProcessVessel>`
/// body. Vessel IObject shape:
///
/// * **A01 style** (default) — `UID + ItemTag + Description`
///   three-attribute shape that has been live since the
///   initial publish writer landed. Every existing test
///   exercises this path.
/// * **DWG style** — `UID + Description` two-attribute
///   shape. The DWG reference omits the identifier on the
///   vessel IObject entirely (the `污水池` sample carries
///   just `UID` + `Description="污水池"`); this branch
///   matches that fixture byte-for-byte.
fn write_process_vessel_iobject(
    buf: &mut String,
    uid: &str,
    item_tag: &str,
    description: &str,
    style: PublishStyle,
) -> Result<(), PublishError> {
    match style {
        PublishStyle::A01 => writeln!(
            buf,
            r#"      <IObject UID="{}" ItemTag="{}" Description="{}"/>"#,
            escape_attr(uid),
            escape_attr(item_tag),
            escape_attr(description),
        ),
        PublishStyle::Dwg => writeln!(
            buf,
            r#"      <IObject UID="{}" Description="{}"/>"#,
            escape_attr(uid),
            escape_attr(description),
        ),
    }
    .map_err(fmt_err)
}

/// Emit the full `<PIDPipeline>` block for a PipeRun row.
///
/// The reference SmartPlant shape (confirmed byte-for-byte on
/// both `A01_Data.xml:54-65` and `DWG-0202GP06-01_Data.xml:1369-1380`):
/// ```text
/// <PIDPipeline>
///    <IObject UID="..." Name="..." ItemTag="..."/>
///    <IPBSItem/>
///    <IPlannedFacility/>
///    <IPBSItemCollection/>
///    <IPipeline/>
///    <IPipingConnectorComposition/>
///    <IFluidSystem FluidCode="@{...}" FluidSystem="@{...}"/>
///    <INoteCollection/>
///    <IExpandableThing/>
///    <IPIDTypical/>
/// </PIDPipeline>
/// ```
///
/// A19 closes four pre-existing fidelity gaps:
/// * Adds the empty wrapper interfaces `IPBSItem`,
///   `IPlannedFacility`, `IPBSItemCollection`, `INoteCollection`
///   which SmartPlant always emits but our earlier writer
///   silently dropped.
/// * Populates `IFluidSystem FluidCode="..." FluidSystem="..."`
///   from `obj.fields["OperFluidCode"]` + `obj.fields["FluidSystem"]`
///   (T_PipeRun columns loaded by the sqlite_load layer). When
///   either value is absent the respective attribute renders
///   empty, matching the A01 fixture shape (`<IFluidSystem/>`
///   without attributes, which under A19 becomes
///   `<IFluidSystem FluidCode="" FluidSystem=""/>` — still a
///   fidelity improvement for downstream consumers expecting
///   the attributes to be declared).
///
/// A19 also adds an optional `Name=` attribute on the IObject
/// when the loader populates `obj.fields["PipelineName"]` or
/// falls back to the item tag. The DWG reference uses e.g.
/// `Name="A3jqz0101-OD"` for pipeline labels; A01 uses
/// unlabeled pipelines and the attribute is omitted to match.
///
/// A29 introduces an explicit [`PublishStyle`] selector so the
/// IObject shape no longer relies on `obj.fields["PipelineName"]`
/// alone. With `style = A01` (default), the pre-A29 behaviour
/// is preserved bit-for-bit. With `style = Dwg` (set by callers
/// that loaded a DWG-flavor SQLite mirror), the IObject drops
/// the `ItemTag` attribute — matching the DWG reference's
/// `<IObject UID="..." Name="..."/>` two-attribute shape.
fn write_pipeline(
    buf: &mut String,
    obj: &PublishObject,
    style: PublishStyle,
) -> Result<(), PublishError> {
    let item_tag = canonical_pipeline_item_tag(obj, style);
    let fluid_code = if matches!(style, PublishStyle::A01) {
        String::new()
    } else {
        obj.fields.get("OperFluidCode").cloned().unwrap_or_default()
    };
    let fluid_system = if matches!(style, PublishStyle::A01) {
        String::new()
    } else {
        obj.fields.get("FluidSystem").cloned().unwrap_or_default()
    };
    // Name takes the loader-provided `PipelineName` when present.
    // Current SQLite mirrors often preserve only the raw
    // `T_PlantItem.Name` column, so accept it as a fallback.
    let pipeline_name = non_empty_field(obj, "PipelineName")
        .or_else(|| non_empty_field(obj, "Name"))
        .map(str::to_string);
    writeln!(buf, "   <PIDPipeline>").map_err(fmt_err)?;
    write_pipeline_iobject(buf, &obj.uid, &item_tag, pipeline_name.as_deref(), style)?;
    writeln!(buf, "      <IPBSItem/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPlannedFacility/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPBSItemCollection/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPipeline/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPipingConnectorComposition/>").map_err(fmt_err)?;
    if fluid_code.is_empty() && fluid_system.is_empty() {
        writeln!(buf, "      <IFluidSystem/>").map_err(fmt_err)?;
    } else {
        writeln!(
            buf,
            r#"      <IFluidSystem FluidCode="{}" FluidSystem="{}"/>"#,
            escape_attr(&fluid_code),
            escape_attr(&fluid_system),
        )
        .map_err(fmt_err)?;
    }
    writeln!(buf, "      <INoteCollection/>").map_err(fmt_err)?;
    writeln!(buf, "      <IExpandableThing/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPIDTypical/>").map_err(fmt_err)?;
    writeln!(buf, "   </PIDPipeline>").map_err(fmt_err)
}

/// Emit the full `<PIDPipingConnector>` block for a PipeRun row.
///
/// The reference SmartPlant shape is a 22-interface block
/// (confirmed byte-for-byte on `A01_Data.xml:66–89` — 22
/// interfaces, all bare except the named/sized ones — and
/// `DWG-0202GP06-01_Data.xml:246–269` — the same 22 interfaces
/// plus populated optional attributes on `IConnector` /
/// `IPipingConnector` / `ISlopedPipingItem` / `IInsulatedItem`).
///
/// A20 closes a 15-interface fidelity gap. Pre-A20 writer emitted
/// only 7 interfaces (IObject, IConnector, IPipingConnector,
/// INamedPipingConnector, IPipeCrossSectionItem, IPipingSpecifiedItem,
/// IPIDTypical). The 15 missing wrapper interfaces
/// (IPBSItem, IPlannedFacility, IDrawingItem, IPBSItemCollection,
/// IFabricatedItem, IHeatTracedItem, IProcessPointCollection,
/// IDocumentItem, IElecPowerConsumer, INoteCollection,
/// IProcessDataCaseComposition, IExpandableThing,
/// ISlopedPipingItem, IInsulatedItem, IJacketedItem) are now
/// emitted unconditionally in SPPID-canonical order.
///
/// Attribute routing:
/// * `IObject` — `UID` is the derived `<piperun>-CNX`. A01 uses
///   `ItemTag="..."`; DWG uses `Name="..."` instead (same
///   field, different SmartPlant exporter versions). The writer
///   emits `Name="..."` when `obj.fields["PipelineName"]` is
///   populated (DWG-shape), otherwise `ItemTag="..."` (A01-shape).
/// * `IPBSItem` — same defaults as A17's PIDPipingComponent
///   (`@NewConstruction` + the fixed `{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}`
///   GUID), overridable via `obj.fields["ConstructionStatus"]`
///   and `obj.fields["ConstructionStatus2"]`.
/// * `IConnector` — `FlowDirection` +
///   `RepresentationsAreAllZeroLength` optional, sourced from
///   `obj.fields["FlowDirection"]` + the SPPID boolean
///   `obj.fields["RepresentationsAreAllZeroLength"]` via
///   [`map_bool`]. Both render `False`/empty in the A01 shape;
///   both populate from the DWG shape.
/// * `IPipingConnector` — `PipingConnectorType` optional.
/// * `IHeatTracedItem` — `HTraceRqmt` (standard SPPID field).
/// * `INamedPipingConnector` — three always-declared attributes
///   (`PipingConnectorPrefix`, `PipingConnectorSeqNo`,
///   `PipingConnectorSuff`) routed from
///   `obj.fields["TagPrefix"/"TagSequenceNo"/"TagSuffix"]`.
/// * `IPipeCrossSectionItem` — `NominalDiameter` with mm
///   suffix via [`format_diameter`].
/// * `IPipingSpecifiedItem` — `PipingMaterialsClass`.
/// * `ISlopedPipingItem` — `SlopedPipingAngle` +
///   `SlopedPipeDirection` optional (DWG populates with
///   radians + enum GUID; A01 emits bare).
/// * `IInsulatedItem` — `InsulThickSrc` + `TotalInsulThick`
///   optional (DWG populates; A01 emits bare).
/// * `IPIDTypical` — `IsTypical` routed from
///   `obj.is_typical` via [`map_bool`].
///
/// Optional attributes render as empty strings when the loader
/// has not populated the corresponding column. This keeps the
/// A01 byte-shape identical to the pre-A20 empty case while
/// unlocking the DWG-specific populated shape when the columns
/// arrive from T_PipeRun / T_Connector.
fn write_piping_connector(
    buf: &mut String,
    obj: &PublishObject,
    style: PublishStyle,
) -> Result<(), PublishError> {
    let tag_prefix = obj.fields.get("TagPrefix").cloned().unwrap_or_default();
    let tag_sequence = obj.fields.get("TagSequenceNo").cloned().unwrap_or_default();
    let tag_suffix = obj.fields.get("TagSuffix").cloned().unwrap_or_default();
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
    let construction_status = canonical_construction_status(obj, style);
    let construction_status2 = canonical_construction_status2(obj, style);
    let flow_direction = obj.fields.get("FlowDirection").cloned().unwrap_or_default();
    let has_reps_all_zero_length = non_empty_field(obj, "RepresentationsAreAllZeroLength")
        .is_some()
        || matches!(style, PublishStyle::Dwg)
            && non_empty_field_any(obj, &["SP_ConnectorsZeroLength", "IsZeroLength"]).is_some();
    let reps_all_zero_length = obj
        .fields
        .get("RepresentationsAreAllZeroLength")
        .map(String::as_str)
        .or_else(|| {
            if matches!(style, PublishStyle::Dwg) {
                non_empty_field_any(obj, &["SP_ConnectorsZeroLength", "IsZeroLength"])
            } else {
                None
            }
        })
        .map_or("False", map_bool);
    let piping_connector_type =
        dwg_field_with_aliases(obj, style, "PipingConnectorType", &["PipeRunType"])
            .unwrap_or_default()
            .to_string();
    let htrace_rqmt = obj
        .fields
        .get("HTraceRqmt")
        .cloned()
        .or_else(|| obj.fields.get("HTraceReqmt").cloned())
        .unwrap_or_default();
    let sloped_piping_angle = dwg_field_with_aliases(obj, style, "SlopedPipingAngle", &["Slope"])
        .unwrap_or_default()
        .to_string();
    let sloped_pipe_direction =
        dwg_field_with_aliases(obj, style, "SlopedPipeDirection", &["SlopeDirection"])
            .unwrap_or_default()
            .to_string();
    let insul_thick_src =
        dwg_field_with_aliases(obj, style, "InsulThickSrc", &["InsulationThkSource"])
            .unwrap_or_default()
            .to_string();
    let total_insul_thick = dwg_field_with_aliases(obj, style, "TotalInsulThick", &["InsulThick"])
        .unwrap_or_default()
        .to_string();
    let pipeline_name = non_empty_field(obj, "PipelineName")
        .or_else(|| non_empty_field(obj, "Name"))
        .map(str::to_string);
    // The connector inherits its ItemTag from the pipeline it is
    // the physical half of — SmartPlant renders them identically.
    let item_tag = canonical_connector_item_tag(obj, style);
    // The connector is a publish-time synthetic node, so the
    // final artifact uses a deterministic SmartPlant-style
    // 32-hex UID rather than exposing a writer-internal
    // `<pipe>-CNX` seed.
    let connector_uid = derived_pipe_connector_uid(&obj.uid);
    writeln!(buf, "   <PIDPipingConnector>").map_err(fmt_err)?;
    // A29 routes the IObject shape through the explicit
    // PublishStyle selector. Pre-A29 the writer used the
    // presence of `obj.fields["PipelineName"]` as an
    // implicit DWG marker, which conflated "data has a name"
    // with "fixture is DWG-flavor". Post-A29 the style flag
    // is authoritative; PipelineName still controls whether
    // the Name attribute carries a value, but the choice
    // between `Name` and `ItemTag` keys is made by
    // [`PublishStyle`].
    write_piping_connector_iobject(
        buf,
        &connector_uid,
        &item_tag,
        pipeline_name.as_deref(),
        style,
    )?;
    writeln!(
        buf,
        r#"      <IPBSItem ConstructionStatus="{}" ConstructionStatus2="{}"/>"#,
        escape_attr(&construction_status),
        escape_attr(&construction_status2),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IPlannedFacility/>").map_err(fmt_err)?;
    // IConnector renders bare when both optional attributes are
    // absent (matches A01); renders with both attributes when any
    // is present (matches DWG). Compat invariant: the two shapes
    // are byte-identical to their respective reference fixtures.
    if flow_direction.is_empty() && !has_reps_all_zero_length {
        writeln!(buf, "      <IConnector/>").map_err(fmt_err)?;
    } else {
        writeln!(
            buf,
            r#"      <IConnector FlowDirection="{}" RepresentationsAreAllZeroLength="{}"/>"#,
            escape_attr(&flow_direction),
            reps_all_zero_length,
        )
        .map_err(fmt_err)?;
    }
    writeln!(buf, "      <IDrawingItem/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPBSItemCollection/>").map_err(fmt_err)?;
    if piping_connector_type.is_empty() {
        writeln!(buf, "      <IPipingConnector/>").map_err(fmt_err)?;
    } else {
        writeln!(
            buf,
            r#"      <IPipingConnector PipingConnectorType="{}"/>"#,
            escape_attr(&piping_connector_type),
        )
        .map_err(fmt_err)?;
    }
    writeln!(buf, "      <IFabricatedItem/>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IHeatTracedItem HTraceRqmt="{}"/>"#,
        escape_attr(&htrace_rqmt),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IProcessPointCollection/>").map_err(fmt_err)?;
    writeln!(buf, "      <IDocumentItem/>").map_err(fmt_err)?;
    writeln!(buf, "      <IElecPowerConsumer/>").map_err(fmt_err)?;
    writeln!(buf, "      <INoteCollection/>").map_err(fmt_err)?;
    writeln!(buf, "      <IProcessDataCaseComposition/>").map_err(fmt_err)?;
    writeln!(buf, "      <IExpandableThing/>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <INamedPipingConnector PipingConnectorPrefix="{}" PipingConnectorSeqNo="{}" PipingConnectorSuff="{}"/>"#,
        escape_attr(&tag_prefix),
        escape_attr(&tag_sequence),
        escape_attr(&tag_suffix),
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
    if sloped_piping_angle.is_empty() && sloped_pipe_direction.is_empty() {
        writeln!(buf, "      <ISlopedPipingItem/>").map_err(fmt_err)?;
    } else {
        writeln!(
            buf,
            r#"      <ISlopedPipingItem SlopedPipingAngle="{}" SlopedPipeDirection="{}"/>"#,
            escape_attr(&sloped_piping_angle),
            escape_attr(&sloped_pipe_direction),
        )
        .map_err(fmt_err)?;
    }
    if insul_thick_src.is_empty() && total_insul_thick.is_empty() {
        writeln!(buf, "      <IInsulatedItem/>").map_err(fmt_err)?;
    } else {
        writeln!(
            buf,
            r#"      <IInsulatedItem InsulThickSrc="{}" TotalInsulThick="{}"/>"#,
            escape_attr(&insul_thick_src),
            escape_attr(&total_insul_thick),
        )
        .map_err(fmt_err)?;
    }
    writeln!(buf, "      <IJacketedItem/>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IPIDTypical IsTypical="{}"/>"#,
        canonical_is_typical(obj, style),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "   </PIDPipingConnector>").map_err(fmt_err)
}

/// Emit the three virtual nodes SmartPlant always derives from a
/// PipingConnector: two `<PIDPipingPort>` children (suffixed `.1`
/// and `.2`, both inheriting the parent connector's nominal
/// diameter) plus one `<PIDProcessPoint>` (suffixed `.PPT`).
///
/// These nodes never appear as their own SQLite rows — they are
/// SmartPlant client-side composition members rendered by the
/// exporter at publish time. The base UID is the same
/// deterministic connector UID the `<PIDPipingConnector>`
/// block carries, with the SmartPlant suffixes `.1`, `.2`,
/// `.PPT`.
fn write_derived_connector_endpoints(
    buf: &mut String,
    obj: &PublishObject,
) -> Result<(), PublishError> {
    let connector_uid = derived_pipe_connector_uid(&obj.uid);
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
    writeln!(buf, r#"      <IObject UID="{}"/>"#, escape_attr(&obj.uid)).map_err(fmt_err)?;
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
    writeln!(buf, r#"      <IObject UID="{}"/>"#, escape_attr(&obj.uid)).map_err(fmt_err)?;
    writeln!(buf, "      <IDrawingItem/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPBSNote/>").map_err(fmt_err)?;
    // A24: When NoteText is empty the reference fixture ships a
    // bare `<INote/>` (no attribute), not `<INote NoteText=""/>`.
    // Match that shape so diff tools and SmartPlant validators
    // see byte-identical output for empty notes. Populated notes
    // still carry the attribute (Chinese CR/LF-embedded strings
    // and all).
    if note_text.is_empty() {
        writeln!(buf, "      <INote/>").map_err(fmt_err)?;
    } else {
        writeln!(
            buf,
            r#"      <INote NoteText="{}"/>"#,
            escape_attr(&note_text)
        )
        .map_err(fmt_err)?;
    }
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
    let tag_sequence_no = obj.fields.get("TagSequenceNo").cloned().unwrap_or_default();
    let tag_prefix = obj.fields.get("TagPrefix").cloned().unwrap_or_default();
    let tag_suffix = obj.fields.get("TagSuffix").cloned().unwrap_or_default();
    let loop_suffix = obj.fields.get("LoopTagSuffix").cloned().unwrap_or_default();
    let func_modifier = obj
        .fields
        .get("InstrumentTypeModifier")
        .cloned()
        .unwrap_or_default();
    // A24: IPBSItem now carries the SPPID canonical defaults
    // uniformly with PIDPipingComponent (A17), PIDPipingConnector
    // (A20), PIDProcessVessel (A21), PIDNozzle (A22). Before A24
    // the ControlSystemFunction emitted a bare `<IPBSItem/>`
    // which diverged from the DWG reference's
    // `<IPBSItem ConstructionStatus="@NewConstruction" ...>`
    // shape. Overridable via `obj.fields["ConstructionStatus"]`
    // and `obj.fields["ConstructionStatus2"]`.
    let construction_status = obj
        .fields
        .get("ConstructionStatus")
        .cloned()
        .unwrap_or_else(|| "@NewConstruction".to_string());
    let construction_status2 = obj
        .fields
        .get("ConstructionStatus2")
        .cloned()
        .unwrap_or_else(|| "@{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}".to_string());

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
        writeln!(buf, r#"      <IObject UID="{}"/>"#, escape_attr(&obj.uid),).map_err(fmt_err)?;
    } else {
        writeln!(
            buf,
            r#"      <IObject UID="{}" Name="{}"/>"#,
            escape_attr(&obj.uid),
            escape_attr(&name),
        )
        .map_err(fmt_err)?;
    }
    writeln!(
        buf,
        r#"      <IPBSItem ConstructionStatus="{}" ConstructionStatus2="{}"/>"#,
        escape_attr(&construction_status),
        escape_attr(&construction_status2),
    )
    .map_err(fmt_err)?;
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

/// Emit a `<PIDPipingComponent>` block for a PipingComp object.
///
/// The reference DWG fixture shows the canonical shape (the Cap
/// sample at `DWG-0202GP06-01_Data.xml:204-224`):
/// ```text
/// <PIDPipingComponent>
///    <IObject UID="..."/>
///    <IPBSItem ConstructionStatus="@NewConstruction"
///       ConstructionStatus2="@{...}"/>
///    <IPipingPortComposition/>
///    <IPlannedMatl/>
///    <IDrawingItem/>
///    <IPipingComponent PipingComponentType1="@{...}"
///       PipingComponentType3="@{...}"
///       PipingComponentType2="@{...}"
///       PipModelCode="Cap"
///       CommoditySpecialtyType="@{...}"/>
///    <IPipingComponentOcc/>
///    <IInlineComponentOcc/>
///    <IFabricatedItem/>
///    <IHeatTracedItem HTraceRqmt=""/>
///    <IPartOcc CatalogPartNumber="A3"/>
///    <IPressureReliefItem/>
///    <IDocumentItem/>
///    <IElecPowerConsumer/>
///    <IPart/>
///    <INoteCollection/>
///    <IPipeCrossSectionItem NominalDiameter="100 mm"/>
///    <IInlineComponent IsFlowDirectional="False"/>
///    <IPIDTypical IsTypical="False"/>
/// </PIDPipingComponent>
/// ```
///
/// Attribute provenance:
/// * `PipingComponentType1/2/3` — SPPID enum codelist IDs sourced
///   from `T_PipingComp` columns of the same name. They are
///   opaque GUIDs in the `@{...}` form SPPID canonical; the writer
///   renders them verbatim when present.
/// * `PipModelCode` — the component-kind display string
///   (`"Cap"` / `"Conduit gate valve"` etc.), sourced from
///   `T_PipingComp.PipModelCode`.
/// * `CommoditySpecialtyType` — another `@{...}` codelist ref from
///   `T_PipingComp.CommoditySpecialtyType`.
/// * `CatalogPartNumber` on `IPartOcc` — sourced from
///   `T_PlantItem.CatalogPartNumber`. DWG fixtures ship this only
///   on the valve sample (`"A3"`), so it's rendered empty by
///   default to match the Cap shape.
/// * `HTraceRqmt` on `IHeatTracedItem` — sourced from the same
///   column on `T_PlantItem`. Empty by default, matching both
///   samples.
/// * `NominalDiameter` on `IPipeCrossSectionItem` — standard SPPID
///   numeric field, formatted with the `" mm"` suffix via
///   [`format_diameter`].
/// * `IsFlowDirectional` on `IInlineComponent` — SPPID boolean
///   from `T_InlineComp.IsFlowDirectional`, rendered through
///   [`map_bool`]; defaults to `"False"` when absent (matches both
///   reference samples).
/// * `IsTypical` on `IPIDTypical` — the `T_ModelItem.SP_IsTypical`
///   standard boolean, rendered through [`map_bool`]; defaults to
///   `"False"`.
fn write_piping_component(buf: &mut String, obj: &PublishObject) -> Result<(), PublishError> {
    let construction_status = obj
        .fields
        .get("ConstructionStatus")
        .cloned()
        .unwrap_or_else(|| "@NewConstruction".to_string());
    let construction_status2 = obj
        .fields
        .get("ConstructionStatus2")
        .cloned()
        .unwrap_or_else(|| "@{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}".to_string());
    let pip_ct1 = obj
        .fields
        .get("PipingComponentType1")
        .cloned()
        .unwrap_or_default();
    let pip_ct2 = obj
        .fields
        .get("PipingComponentType2")
        .cloned()
        .unwrap_or_default();
    let pip_ct3 = obj
        .fields
        .get("PipingComponentType3")
        .cloned()
        .unwrap_or_default();
    let pip_model_code = obj.fields.get("PipModelCode").cloned().unwrap_or_default();
    let commodity_specialty = obj
        .fields
        .get("CommoditySpecialtyType")
        .cloned()
        .unwrap_or_default();
    let catalog_part_number = obj
        .fields
        .get("CatalogPartNumber")
        .cloned()
        .unwrap_or_default();
    let htrace_rqmt = obj.fields.get("HTraceRqmt").cloned().unwrap_or_default();
    let nominal_diameter = obj
        .fields
        .get("NominalDiameter")
        .cloned()
        .map(|v| format_diameter(&v))
        .unwrap_or_default();
    let is_flow_directional = obj
        .fields
        .get("IsFlowDirectional")
        .map_or("False", |s| map_bool(s));

    writeln!(buf, "   <PIDPipingComponent>").map_err(fmt_err)?;
    writeln!(buf, r#"      <IObject UID="{}"/>"#, escape_attr(&obj.uid)).map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IPBSItem ConstructionStatus="{}" ConstructionStatus2="{}"/>"#,
        escape_attr(&construction_status),
        escape_attr(&construction_status2),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IPipingPortComposition/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPlannedMatl/>").map_err(fmt_err)?;
    writeln!(buf, "      <IDrawingItem/>").map_err(fmt_err)?;
    writeln!(
        buf,
        concat!(
            r#"      <IPipingComponent PipingComponentType1="{}" "#,
            r#"PipingComponentType3="{}" PipingComponentType2="{}" "#,
            r#"PipModelCode="{}" CommoditySpecialtyType="{}"/>"#,
        ),
        escape_attr(&pip_ct1),
        escape_attr(&pip_ct3),
        escape_attr(&pip_ct2),
        escape_attr(&pip_model_code),
        escape_attr(&commodity_specialty),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IPipingComponentOcc/>").map_err(fmt_err)?;
    writeln!(buf, "      <IInlineComponentOcc/>").map_err(fmt_err)?;
    writeln!(buf, "      <IFabricatedItem/>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IHeatTracedItem HTraceRqmt="{}"/>"#,
        escape_attr(&htrace_rqmt),
    )
    .map_err(fmt_err)?;
    // CatalogPartNumber is only emitted when present — the Cap
    // sample omits it entirely (`<IPartOcc/>`) while the valve
    // sample carries `CatalogPartNumber="A3"`. Matching the
    // conditional shape keeps the writer byte-compatible with
    // both reference variants.
    if catalog_part_number.is_empty() {
        writeln!(buf, "      <IPartOcc/>").map_err(fmt_err)?;
    } else {
        writeln!(
            buf,
            r#"      <IPartOcc CatalogPartNumber="{}"/>"#,
            escape_attr(&catalog_part_number),
        )
        .map_err(fmt_err)?;
    }
    writeln!(buf, "      <IPressureReliefItem/>").map_err(fmt_err)?;
    writeln!(buf, "      <IDocumentItem/>").map_err(fmt_err)?;
    writeln!(buf, "      <IElecPowerConsumer/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPart/>").map_err(fmt_err)?;
    writeln!(buf, "      <INoteCollection/>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IPipeCrossSectionItem NominalDiameter="{}"/>"#,
        escape_attr(&nominal_diameter),
    )
    .map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IInlineComponent IsFlowDirectional="{is_flow_directional}"/>"#,
    )
    .map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IPIDTypical IsTypical="{}"/>"#,
        obj.is_typical.as_deref().map_or("False", map_bool),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "   </PIDPipingComponent>").map_err(fmt_err)
}

/// Emit a `<PIDSignalConnector>` block for a SignalRun object.
///
/// The reference DWG fixture shows the canonical shape (at
/// `DWG-0202GP06-01_Data.xml:1111-1120`):
/// ```text
/// <PIDSignalConnector>
///    <IObject UID="E871304702F74D39B15BD2D8B41D34B3"/>
///    <IPlannedFacility/>
///    <IConnector FlowDirection=""/>
///    <IDrawingItem/>
///    <ISignalConnector/>
///    <IDocumentItem/>
///    <IExpandableThing/>
///    <IPIDTypical IsTypical="False"/>
/// </PIDSignalConnector>
/// ```
///
/// Compared with `PIDPipingConnector` the signal variant is
/// intentionally minimal:
/// * No `IPBSItem` — signal wiring is not a planned-build
///   component of the facility's pressure system, so SPPID skips
///   the construction-status envelope.
/// * No `IPipingConnector` / `INamedPipingConnector` /
///   `IPipeCrossSectionItem` / `IPipingSpecifiedItem` — those are
///   piping-only interfaces.
/// * `IConnector FlowDirection=""` instead of populated — the DWG
///   fixture ships an empty FlowDirection on every signal
///   connector. Future fixtures may surface a populated value
///   sourced from a column the loader doesn't yet read; for now
///   the attribute renders as whatever the loader places in
///   `obj.fields["FlowDirection"]`, defaulting to empty.
/// * `IPIDTypical IsTypical="False"` matches the reference
///   default; sourced from `T_ModelItem.SP_IsTypical` via
///   [`map_bool`] when populated.
///
/// Endpoint `<Rel UID1="..." UID2="..." DefUID="SignalEnd1Conn">`
/// / `SignalEnd2Conn` rows are NOT derived here — they live on
/// `T_Relationship` and flow through the generic relationship
/// emitter in `write_relationships`. A17/A18 stay focused on the
/// per-object tag shape.
fn write_signal_connector(buf: &mut String, obj: &PublishObject) -> Result<(), PublishError> {
    let flow_direction = obj.fields.get("FlowDirection").cloned().unwrap_or_default();
    writeln!(buf, "   <PIDSignalConnector>").map_err(fmt_err)?;
    writeln!(buf, r#"      <IObject UID="{}"/>"#, escape_attr(&obj.uid)).map_err(fmt_err)?;
    writeln!(buf, "      <IPlannedFacility/>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IConnector FlowDirection="{}"/>"#,
        escape_attr(&flow_direction),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "      <IDrawingItem/>").map_err(fmt_err)?;
    writeln!(buf, "      <ISignalConnector/>").map_err(fmt_err)?;
    writeln!(buf, "      <IDocumentItem/>").map_err(fmt_err)?;
    writeln!(buf, "      <IExpandableThing/>").map_err(fmt_err)?;
    writeln!(
        buf,
        r#"      <IPIDTypical IsTypical="{}"/>"#,
        obj.is_typical.as_deref().map_or("False", map_bool),
    )
    .map_err(fmt_err)?;
    writeln!(buf, "   </PIDSignalConnector>").map_err(fmt_err)
}

/// Emit a `<PIDPipingBranchPoint>` block.
///
/// DWG reference shape (`DWG-0202GP06-01_Data.xml:1337–1344`):
/// ```text
/// <PIDPipingBranchPoint>
///    <IObject UID="CCB3BA926FC54BF89691BC690FAF7D74.BPT"/>
///    <IConnection/>
///    <IPipingConnection/>
///    <IDrawingItem/>
///    <IPipingBranchPoint/>
///    <IDocumentItem/>
/// </PIDPipingBranchPoint>
/// ```
///
/// UID carries the `.BPT` suffix in the reference — the writer
/// emits whatever `obj.uid` the loader supplies, leaving the
/// suffix convention to the loader side. All interfaces are bare
/// (no attributes) so the function needs nothing beyond the UID.
fn write_piping_branch_point(buf: &mut String, obj: &PublishObject) -> Result<(), PublishError> {
    writeln!(buf, "   <PIDPipingBranchPoint>").map_err(fmt_err)?;
    writeln!(buf, r#"      <IObject UID="{}"/>"#, escape_attr(&obj.uid),).map_err(fmt_err)?;
    writeln!(buf, "      <IConnection/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPipingConnection/>").map_err(fmt_err)?;
    writeln!(buf, "      <IDrawingItem/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPipingBranchPoint/>").map_err(fmt_err)?;
    writeln!(buf, "      <IDocumentItem/>").map_err(fmt_err)?;
    writeln!(buf, "   </PIDPipingBranchPoint>").map_err(fmt_err)
}

/// Emit a `<PIDBranchPoint>` block.
///
/// DWG reference shape (`DWG-0202GP06-01_Data.xml:1448–1457`):
/// ```text
/// <PIDBranchPoint>
///    <IObject UID="0DFD856D382C42F88DA8CDDFD37D4227" Name="272"/>
///    <IPIDBranchPoint/>
///    <IDuctConnection/>
///    <IConnection/>
///    <IDrawingItem/>
///    <IPipingConnection/>
///    <ISignalConnection/>
///    <IDocumentItem/>
/// </PIDBranchPoint>
/// ```
///
/// UID is a plain 32-hex and `Name` holds an internal sequence
/// number. All interfaces below IObject are bare. `Name` is
/// sourced from `obj.fields["Name"]` falling back to
/// `obj.description` — the loader must populate one of them.
fn write_pid_branch_point(buf: &mut String, obj: &PublishObject) -> Result<(), PublishError> {
    let name = non_empty_field(obj, "Name")
        .or(obj.description.as_deref())
        .unwrap_or("");
    writeln!(buf, "   <PIDBranchPoint>").map_err(fmt_err)?;
    if name.is_empty() {
        writeln!(buf, r#"      <IObject UID="{}"/>"#, escape_attr(&obj.uid),).map_err(fmt_err)?;
    } else {
        writeln!(
            buf,
            r#"      <IObject UID="{}" Name="{}"/>"#,
            escape_attr(&obj.uid),
            escape_attr(name),
        )
        .map_err(fmt_err)?;
    }
    writeln!(buf, "      <IPIDBranchPoint/>").map_err(fmt_err)?;
    writeln!(buf, "      <IDuctConnection/>").map_err(fmt_err)?;
    writeln!(buf, "      <IConnection/>").map_err(fmt_err)?;
    writeln!(buf, "      <IDrawingItem/>").map_err(fmt_err)?;
    writeln!(buf, "      <IPipingConnection/>").map_err(fmt_err)?;
    writeln!(buf, "      <ISignalConnection/>").map_err(fmt_err)?;
    writeln!(buf, "      <IDocumentItem/>").map_err(fmt_err)?;
    writeln!(buf, "   </PIDBranchPoint>").map_err(fmt_err)
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
    writeln!(buf, r#"      <IObject UID="{}"/>"#, escape_attr(&obj.uid),).map_err(fmt_err)?;
    writeln!(buf, "   </PIDItem>").map_err(fmt_err)
}

/// Render the SmartPlant composite equipment tag ("TagPrefix
/// TagSequenceNo") from a vessel / equipment row's business
/// fields. Returns an empty string when neither field is present.
fn format_equipment_tag(obj: &PublishObject) -> String {
    let prefix = obj
        .fields
        .get("TagPrefix")
        .map_or("", std::string::String::as_str);
    let seq = obj
        .fields
        .get("TagSequenceNo")
        .map_or("", std::string::String::as_str);
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

fn format_insulation_inches(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Some(number) = trimmed.strip_suffix('"').map(str::trim) {
        if let Ok(value) = number.parse::<f64>() {
            return format!("{value:.3} in");
        }
    }
    trimmed.to_string()
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
    writeln!(buf, r#"      <IObject UID="{}"/>"#, escape_attr(rel_uid),).map_err(fmt_err)?;
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
/// The MDF loader may surface the value as a SQL Server-style raw
/// render, for example `"2026/4/20 10:32:46"`. We zero-pad the
/// month and day and drop the time component, matching the reference
/// fixtures' `"2026/04/20"`. Any input that does not parse as
/// `YYYY/M/D[ ...]` is returned verbatim so callers retain enough
/// debug context to spot unsupported formats.
fn format_meta_date(raw: &str) -> String {
    let date_part = raw.split_whitespace().next().unwrap_or(raw);
    let mut parts = date_part.split('/');
    let (Some(y), Some(m), Some(d), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
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
    for rep in ordered_publishable_representations(drawing) {
        writeln!(buf, "   <PIDRepresentation>").map_err(fmt_err)?;
        writeln!(buf, r#"      <IObject UID="{}"/>"#, escape_attr(&rep.uid)).map_err(fmt_err)?;
        // Current behavior still passes through the staged-table
        // `GraphicOID`. A01 contract parity masks the publish-time
        // remap slot explicitly until the SmartPlant numbering rule
        // is reconstructed.
        match rep.graphic_oid {
            Some(oid) => writeln!(buf, r#"      <IDrawingRepresentation GraphicOID="{oid}"/>"#)
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
    let ordered_reps = ordered_publishable_representations(drawing);
    for rep in &ordered_reps {
        let Some(model_item_uid) = rep.model_item_uid.as_deref() else {
            continue;
        };
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
    for rep in &ordered_reps {
        write_rel(
            buf,
            &format!("DRI-{}-{}", drawing.drawing_uid, rep.uid),
            &drawing.drawing_uid,
            &rep.uid,
            "DrawingItems",
        )?;
    }

    // --- A34: Derived rels for every PipeRun-driven
    // PipingConnector. SmartPlant's exporter pairs every
    // PipingConnector with five derived `<Rel>` rows that
    // wire the connector to its two virtual `<PIDPipingPort>`
    // children, the `<PIDProcessPoint>` collection, and the
    // two endpoint connections. The PIDxxx body emit happens
    // in `write_derived_connector_endpoints`; the rel emit
    // here keeps the per-connector rel count in lockstep so
    // the A33 `<Rel>` DefUID-count gate stays satisfied.
    //
    // A34b also derives the Pipeline → Connector composition
    // rel (`PipingConnectors`) here — same source object,
    // emitted alongside the five port-derived rels.
    //
    // UID derivation mirrors what `write_derived_connector_endpoints`
    // already does:
    //   * pipeline UID:  `<piperun>` (PipeRun obj.uid maps to
    //                    `<PIDPipeline>`)
    //   * connector UID: deterministic publish-time 32-hex UID
    //   * port UIDs:     `<connector>.1` / `<connector>.2`
    //   * process point: `<connector>.PPT`
    //
    // A34c — the PipingEnd1Conn / PipingEnd2Conn target is the
    // upstream ModelItem UID sitting at the connected end of the
    // pipe (e.g. a Nozzle or Vessel). The loader resolves this
    // via `T_Connector.SP_ConnectItem{1,2}ID` in
    // `attach_pipe_endpoint_connections` and stashes the result
    // in `obj.fields["EndConnectedItem1"]` /
    // `obj.fields["EndConnectedItem2"]`.
    //
    // Fallback: when the loader didn't populate an endpoint (no
    // T_Connector row, or the port is physically unconnected —
    // A01's port.2 behaves this way), we keep the pre-A34c
    // `<connector>.PPT` placeholder. The reference XML does
    // exactly the same for its unconnected port.2, so the
    // fallback is not a hack — it's the SPPID convention for
    // "no external connection".
    // --- A34b: Vessel → Nozzle composition (EquipmentComponentComposition).
    //
    // SmartPlant ties every nozzle to its parent vessel via
    // the T_Nozzle.SP_EquipmentID column (loaded into
    // `obj.fields["SP_EquipmentID"]` by `attach_business_columns`).
    // The reference XML's EquipmentComponentComposition row
    // is a derived `<Rel>` from this parent link, not a
    // T_Relationship row — A01's T_Relationship table only
    // carries Representation ↔ Representation rels.
    for obj in &drawing.objects {
        if obj.item_type_name != "Nozzle" {
            continue;
        }
        let Some(vessel_uid) = obj.fields.get("SP_EquipmentID") else {
            continue;
        };
        if vessel_uid.is_empty() {
            continue;
        }
        write_rel(
            buf,
            &format!("EQC-{vessel_uid}-{}", obj.uid),
            &obj.uid,
            vessel_uid,
            "EquipmentComponentComposition",
        )?;
    }

    for obj in &drawing.objects {
        if obj.item_type_name != "PipeRun" {
            continue;
        }
        let pipeline_uid = obj.uid.as_str();
        let connector_uid = derived_pipe_connector_uid(pipeline_uid);
        let port1_uid = format!("{connector_uid}.1");
        let port2_uid = format!("{connector_uid}.2");
        let ppt_uid = format!("{connector_uid}.PPT");
        let end1_uid = obj
            .fields
            .get("EndConnectedItem1")
            .cloned()
            .unwrap_or_else(|| ppt_uid.clone());
        let end2_uid = obj
            .fields
            .get("EndConnectedItem2")
            .cloned()
            .unwrap_or_else(|| ppt_uid.clone());
        // A34b: Pipeline → Connector composition.
        write_rel(
            buf,
            &format!("PCN-{pipeline_uid}"),
            pipeline_uid,
            &connector_uid,
            "PipingConnectors",
        )?;
        write_rel(
            buf,
            &format!("PPC-{connector_uid}-1"),
            &connector_uid,
            &port1_uid,
            "PipingPortComposition",
        )?;
        write_rel(
            buf,
            &format!("PPC-{connector_uid}-2"),
            &connector_uid,
            &port2_uid,
            "PipingPortComposition",
        )?;
        write_rel(
            buf,
            &format!("PRP-{connector_uid}"),
            &connector_uid,
            &ppt_uid,
            "ProcessPointCollection",
        )?;
        write_rel(
            buf,
            &format!("PE1-{port1_uid}"),
            &port1_uid,
            &end1_uid,
            "PipingEnd1Conn",
        )?;
        write_rel(
            buf,
            &format!("PE2-{port2_uid}"),
            &port2_uid,
            &end2_uid,
            "PipingEnd2Conn",
        )?;
    }

    // --- From T_Relationship, classified by endpoint item types
    //
    // A36b — skip rows whose source or target UID is NULL / empty.
    // SmartPlant's exporter never emits an `<IRel UID2=""/>` for
    // a half-wired relationship; shipping one produces a dangling
    // reference that validators reject. The A36b soundness gate
    // surfaced this when T_Relationship carried a row with an
    // unpaired endpoint on A01.
    //
    // A40 — also skip rows where BOTH endpoints resolve to
    // Representations. Investigation on the A01 fixture
    // revealed SmartPlant's exporter does not emit these as
    // their own `<Rel>` entries — every Rep↔Rep relationship
    // is already covered by the ModelItem → Rep derived
    // emits in `write_derived_*`, so re-emitting them here
    // classifies as DwgRepresentationComposition a second
    // time and produces an over-count. The A40 Rel DefUID
    // diff against the A01 reference surfaced this as a
    // DELTA row (writer 6, reference 4, +2 extras).
    for rel in &drawing.relationships {
        let uid1 = rel.source_uid.as_deref().unwrap_or("");
        let uid2 = rel.target_uid.as_deref().unwrap_or("");
        if uid1.is_empty() || uid2.is_empty() {
            continue;
        }
        let t1 = type_by_uid.get(uid1).copied().unwrap_or("");
        let t2 = type_by_uid.get(uid2).copied().unwrap_or("");
        if t1 == "Representation" && t2 == "Representation" {
            continue;
        }
        let def_uid = classify_relationship(rel, &type_by_uid);
        let prefix = defuid_prefix(&def_uid);
        let rel_uid = format!("{prefix}-{uid1}-{uid2}");
        write_rel(buf, &rel_uid, uid1, uid2, &def_uid)?;
    }
    Ok(())
}

/// Emit a single `<Rel>` node. `rel_uid` is the current writer-side
/// deterministic placeholder seed (`DRC-…`, `DRI-…`, `PCN-…`,
/// `PPC-…`, `PPP-…`, `EQC-…`, `PE1-…`, `PE2-…`, or the general
/// `prefix-uid1-uid2` form) and is used verbatim as the published
/// `<IObject UID>`.
///
/// This keeps repeated exports stable and debuggable, but it is not
/// yet the real SmartPlant 32-hex rel-IObject numbering rule. A01
/// delivery-contract and raw-residual tests therefore keep this slot
/// under explicit normalization until the publish-time rule is
/// reconstructed from TEST02.
fn write_rel(
    buf: &mut String,
    rel_uid: &str,
    uid1: &str,
    uid2: &str,
    def_uid: &str,
) -> Result<(), PublishError> {
    writeln!(buf, "   <Rel>").map_err(fmt_err)?;
    writeln!(buf, r#"      <IObject UID="{}"/>"#, escape_attr(rel_uid)).map_err(fmt_err)?;
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
fn classify_relationship(rel: &PublishRelationship, type_by_uid: &HashMap<&str, &str>) -> String {
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
                symbol_path: Some(
                    r"\Equipment\Vessels\Horizontal Drums\Horizontal Drum.sym".into(),
                ),
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
    fn xml_skips_rep_to_rep_t_relationship_rows_to_avoid_drc_double_emit() {
        // A40 — the example drawing's T_Relationship row
        // ties the Nozzle representation to the Vessel
        // representation. The writer *used* to classify that
        // pair as `DwgRepresentationComposition` and emit a
        // DRC- prefixed rel from it, but that over-counts
        // DwgRepresentationComposition on A01 (reference
        // emits 4, pre-A40 writer emitted 6). Investigation
        // on the A01 fixture showed SmartPlant's exporter
        // never emits Rep↔Rep T_Relationship rows as their
        // own `<Rel>` entries — the ModelItem → Rep derived
        // loop already produces the correct DRC inventory.
        let out = write_data_xml(&example_drawing(), "TEST02").expect("write");
        // The REP↔REP composite UID must NOT appear.
        assert!(
            !out.contains(
                r#"<IObject UID="DRC-C33E5BD9B9CC4287B244A925A7A1F29B-CA8A0A9DD1784E3BB6913445CE3F6375"/>"#
            ),
            "Rep↔Rep T_Relationship row must no longer produce a DRC rel; out:\n{out}"
        );
        // The ModelItem → Rep derived DRC rels must still
        // be present (one per representation that carries a
        // `model_item_uid`). This is the single source of
        // truth for DwgRepresentationComposition after A40.
        assert!(out.contains(
            r#"<IRel UID1="C57494A1B154442C9DF0F4BA713E88EC" UID2="CA8A0A9DD1784E3BB6913445CE3F6375" DefUID="DwgRepresentationComposition"/>"#
        ));
        assert!(out.contains(
            r#"<IRel UID1="7465E81219DB49B492BDF60A055AA391" UID2="C33E5BD9B9CC4287B244A925A7A1F29B" DefUID="DwgRepresentationComposition"/>"#
        ));
    }

    // -------------------------------------------------------------
    // A34c — PipingEnd1Conn / PipingEnd2Conn UID2 real endpoint inference
    // -------------------------------------------------------------

    #[test]
    fn a34c_piping_end1_conn_uses_upstream_model_item_uid_when_loader_attached() {
        // Simulate what `attach_pipe_endpoint_connections` would
        // have written to the PipeRun obj: port.1 connects to the
        // Nozzle (7465...), port.2 is unconnected (no field).
        let mut d = example_drawing();
        let pipe = d
            .objects
            .iter_mut()
            .find(|o| o.item_type_name == "PipeRun")
            .expect("PipeRun in fixture");
        pipe.fields.insert(
            "EndConnectedItem1".into(),
            "7465E81219DB49B492BDF60A055AA391".into(),
        );
        let out = write_data_xml(&d, "TEST02").expect("write");
        // PipingEnd1Conn UID2 is the real Nozzle UID, not the
        // placeholder `.PPT`.
        assert!(
            out.contains(
                r#"<IRel UID1="185EF98B03E844158E3BD8E82806E6CF-CNX.1" UID2="7465E81219DB49B492BDF60A055AA391" DefUID="PipingEnd1Conn"/>"#
            ),
            "PipingEnd1Conn UID2 must be the Nozzle ModelItem UID; out:\n{out}"
        );
        // PipingEnd2Conn still falls back to `.PPT` because the
        // loader did not populate EndConnectedItem2.
        assert!(
            out.contains(
                r#"<IRel UID1="185EF98B03E844158E3BD8E82806E6CF-CNX.2" UID2="185EF98B03E844158E3BD8E82806E6CF-CNX.PPT" DefUID="PipingEnd2Conn"/>"#
            ),
            "PipingEnd2Conn UID2 without loader field must fall back to the .PPT placeholder; out:\n{out}"
        );
    }

    #[test]
    fn a34c_piping_end_conn_falls_back_to_ppt_when_fields_absent() {
        // Legacy path: no loader attachment, both ends use the
        // pre-A34c `.PPT` placeholder. Keeps synthetic unit tests
        // and pid-only bundles working unchanged.
        let d = example_drawing();
        let pipe = d
            .objects
            .iter()
            .find(|o| o.item_type_name == "PipeRun")
            .expect("PipeRun in fixture");
        assert!(
            !pipe.fields.contains_key("EndConnectedItem1"),
            "precondition: fixture has no loader-attached endpoint"
        );
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(out.contains(
            r#"<IRel UID1="185EF98B03E844158E3BD8E82806E6CF-CNX.1" UID2="185EF98B03E844158E3BD8E82806E6CF-CNX.PPT" DefUID="PipingEnd1Conn"/>"#
        ));
        assert!(out.contains(
            r#"<IRel UID1="185EF98B03E844158E3BD8E82806E6CF-CNX.2" UID2="185EF98B03E844158E3BD8E82806E6CF-CNX.PPT" DefUID="PipingEnd2Conn"/>"#
        ));
    }

    #[test]
    fn a34c_piping_end2_conn_honors_end2_field_when_both_populated() {
        // Two-ended pipe: the loader resolved both port.1 and
        // port.2 to real ModelItems. Writer must route each end
        // independently.
        let mut d = example_drawing();
        let pipe = d
            .objects
            .iter_mut()
            .find(|o| o.item_type_name == "PipeRun")
            .expect("PipeRun in fixture");
        pipe.fields.insert(
            "EndConnectedItem1".into(),
            "7465E81219DB49B492BDF60A055AA391".into(),
        );
        pipe.fields.insert(
            "EndConnectedItem2".into(),
            "C57494A1B154442C9DF0F4BA713E88EC".into(),
        );
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"UID2="7465E81219DB49B492BDF60A055AA391" DefUID="PipingEnd1Conn""#),
            "PipingEnd1Conn UID2 = Nozzle UID"
        );
        assert!(
            out.contains(r#"UID2="C57494A1B154442C9DF0F4BA713E88EC" DefUID="PipingEnd2Conn""#),
            "PipingEnd2Conn UID2 = Vessel UID"
        );
        // The PPT placeholder must no longer appear as a target
        // on either PipingEnd rel.
        assert!(
            !out.contains(r#"UID2="185EF98B03E844158E3BD8E82806E6CF-CNX.PPT" DefUID="PipingEnd"#),
            "PPT placeholder must be fully displaced when both fields are set; out:\n{out}"
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
    fn a01_pipeline_prefers_expanded_publish_tags_when_pipe_fields_available() {
        // A01 reference XML expands the pipe tag from PipeRun
        // business fields even when T_PlantItem.ItemTag carries
        // the shorter catalog tag (`A010102102-PH` in TEST02).
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
                m.insert("InsulThick".into(), "40\"".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"ItemTag="PH- 0102102-DN250 mm-B5-P-40.000 in""#),
            "A01 PIDPipeline should use the expanded publish tag; out:\n{out}"
        );
        assert!(
            out.contains(r#"ItemTag="PH-0102102-250 mm-B5""#),
            "A01 PIDPipingConnector should use the compact pipe tag; out:\n{out}"
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

        let pos_version = out
            .find("<DocumentVersion>")
            .expect("DocumentVersion present");
        let pos_revision = out
            .find("<DocumentRevision>")
            .expect("DocumentRevision present");
        let pos_file = out.find("<File>").expect("File present");

        assert!(
            pos_version < pos_revision && pos_revision < pos_file,
            "blocks must be ordered DocumentVersion < DocumentRevision < File; out:\n{out}"
        );

        let rel_count = out.matches("<Rel>").count();
        assert_eq!(
            rel_count, 3,
            "meta document carries exactly three Rel rows; out:\n{out}"
        );

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
            "MDF loader raw date should zero-pad to YYYY/MM/DD; out:\n{out}"
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
            uid.chars()
                .all(|c| c.is_ascii_hexdigit()
                    && (!c.is_ascii_alphabetic() || c.is_ascii_uppercase())),
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
    fn item_note_with_no_text_renders_bare_inote() {
        // A24 aligned the empty-text path with the DWG reference
        // shape: SmartPlant emits `<INote/>` (bare), not
        // `<INote NoteText=""/>`. Both are semantically equivalent
        // but SPPID validators compare byte-level, so matching
        // the bare form removes a spurious diff signal.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "NOTE-4".into(),
            item_type_name: "Note".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains("<INote/>"),
            "missing text must render as bare <INote/>; out:\n{out}"
        );
        // And must NOT emit the pre-A24 `<INote NoteText=""/>`
        // attribute form.
        assert!(
            !out.contains(r#"<INote NoteText=""/>"#),
            "A24 must no longer emit the empty-attribute form; out:\n{out}"
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
        let i_connector = out.find("<PIDPipingConnector>").expect("connector present");
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

    // -----------------------------------------------------------------
    // A17 — PIDPipingComponent writer (PipingComp → full 19-interface block)
    // -----------------------------------------------------------------

    #[test]
    fn piping_comp_emits_pid_piping_component_with_all_interfaces() {
        // Mirrors the `Cap` sample at
        // DWG-0202GP06-01_Data.xml:204–224. Fill the fields with
        // representative SPPID data so every attribute-bearing
        // interface can round-trip end-to-end.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "C5A2865821394E019D7DAA0CAFE0490D".into(),
            item_type_name: "PipingComp".into(),
            is_typical: Some("0".into()),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert(
                    "PipingComponentType1".into(),
                    "@{81CD929C-BC07-11D6-BDBC-00104BCC2B69}".into(),
                );
                m.insert(
                    "PipingComponentType2".into(),
                    "@{81CD9804-BC07-11D6-BDBC-00104BCC2B69}".into(),
                );
                m.insert(
                    "PipingComponentType3".into(),
                    "@{81CD9816-BC07-11D6-BDBC-00104BCC2B69}".into(),
                );
                m.insert("PipModelCode".into(), "Cap".into());
                m.insert(
                    "CommoditySpecialtyType".into(),
                    "@{5F7F8F6E-BC29-11D6-BDBC-00104BCC2B69}".into(),
                );
                m.insert("NominalDiameter".into(), "100".into());
                m.insert("IsFlowDirectional".into(), "0".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert_eq!(
            out.matches("<PIDPipingComponent>").count(),
            1,
            "single PipingComp must open exactly one <PIDPipingComponent>; out:\n{out}"
        );
        // All 19 interface opens in canonical order. Ordering is
        // asserted via increasing `find` positions below.
        for needle in [
            r#"<IObject UID="C5A2865821394E019D7DAA0CAFE0490D"/>"#,
            r#"<IPBSItem ConstructionStatus="@NewConstruction" ConstructionStatus2="@{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}"/>"#,
            "<IPipingPortComposition/>",
            "<IPlannedMatl/>",
            "<IDrawingItem/>",
            r#"PipingComponentType1="@{81CD929C-BC07-11D6-BDBC-00104BCC2B69}""#,
            r#"PipingComponentType3="@{81CD9816-BC07-11D6-BDBC-00104BCC2B69}""#,
            r#"PipingComponentType2="@{81CD9804-BC07-11D6-BDBC-00104BCC2B69}""#,
            r#"PipModelCode="Cap""#,
            r#"CommoditySpecialtyType="@{5F7F8F6E-BC29-11D6-BDBC-00104BCC2B69}""#,
            "<IPipingComponentOcc/>",
            "<IInlineComponentOcc/>",
            "<IFabricatedItem/>",
            r#"<IHeatTracedItem HTraceRqmt=""/>"#,
            "<IPartOcc/>", // Cap sample ships no CatalogPartNumber.
            "<IPressureReliefItem/>",
            "<IDocumentItem/>",
            "<IElecPowerConsumer/>",
            "<IPart/>",
            "<INoteCollection/>",
            r#"<IPipeCrossSectionItem NominalDiameter="100 mm"/>"#,
            r#"<IInlineComponent IsFlowDirectional="False"/>"#,
            r#"<IPIDTypical IsTypical="False"/>"#,
        ] {
            assert!(
                out.contains(needle),
                "PIDPipingComponent block must carry `{needle}`; out:\n{out}"
            );
        }
    }

    #[test]
    fn piping_comp_with_catalog_part_number_renders_valve_variant() {
        // Mirrors the `Conduit gate valve` sample at
        // DWG-0202GP06-01_Data.xml:225–245, which carries the
        // optional `CatalogPartNumber="A3"` attribute on
        // <IPartOcc>. Verifies the conditional path doesn't drop
        // it when present.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "6CC683FA4C6A409D8CB4D3F22BBE194E".into(),
            item_type_name: "PipingComp".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("PipModelCode".into(), "Conduit gate valve".into());
                m.insert("NominalDiameter".into(), "80".into());
                m.insert("CatalogPartNumber".into(), "A3".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IPartOcc CatalogPartNumber="A3"/>"#),
            "<IPartOcc> must carry CatalogPartNumber when the field is set; out:\n{out}"
        );
        assert!(
            !out.contains("<IPartOcc/>"),
            "empty <IPartOcc/> must NOT appear when CatalogPartNumber is set; out:\n{out}"
        );
        assert!(
            out.contains(r#"PipModelCode="Conduit gate valve""#),
            "valve sample's PipModelCode must round-trip; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IPipeCrossSectionItem NominalDiameter="80 mm"/>"#),
            "80-mm diameter must acquire the mm suffix; out:\n{out}"
        );
    }

    #[test]
    fn piping_comp_with_empty_fields_still_opens_pid_piping_component() {
        // A PipingComp row with no business-subtable fields — i.e.
        // a fixture where the loader found T_ModelItem but the
        // companion tables are missing — must still produce a
        // syntactically complete block so downstream validators
        // don't choke. Optional attributes render empty.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "PC-BARE".into(),
            item_type_name: "PipingComp".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert_eq!(out.matches("<PIDPipingComponent>").count(), 1);
        assert!(
            out.contains(r#"<IObject UID="PC-BARE"/>"#),
            "bare PipingComp must still carry its UID; out:\n{out}"
        );
        // With no PipingComponentType* columns, the attributes are
        // empty strings — still present, still well-formed.
        assert!(
            out.contains(r#"PipingComponentType1="" PipingComponentType3="" PipingComponentType2="" PipModelCode="" CommoditySpecialtyType="""#),
            "bare PipingComp must emit empty PipingComponentType*/PipModelCode/CommoditySpecialtyType attributes; out:\n{out}"
        );
        assert!(
            out.contains("<IPartOcc/>"),
            "bare PipingComp must default to empty <IPartOcc/>; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IPipeCrossSectionItem NominalDiameter=""/>"#),
            "bare PipingComp must still emit <IPipeCrossSectionItem NominalDiameter=\"\"/>; out:\n{out}"
        );
        // Default booleans must resolve to False via map_bool.
        assert!(
            out.contains(r#"<IInlineComponent IsFlowDirectional="False"/>"#),
            "default IsFlowDirectional must be `False`; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IPIDTypical IsTypical="False"/>"#),
            "default IsTypical must be `False`; out:\n{out}"
        );
    }

    #[test]
    fn piping_comp_maps_is_flow_directional_true() {
        // A fixture where T_InlineComp.IsFlowDirectional is `"1"`
        // (SPPID boolean true) must surface as `IsFlowDirectional="True"`
        // via map_bool. Pins the mapping so a future refactor of
        // map_bool's call sites cannot silently degrade the
        // PipingComponent attribute.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "PC-TRUE".into(),
            item_type_name: "PipingComp".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("IsFlowDirectional".into(), "1".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IInlineComponent IsFlowDirectional="True"/>"#),
            "PipingComp with IsFlowDirectional=1 must emit `True`; out:\n{out}"
        );
    }

    #[test]
    fn two_piping_comps_yield_two_distinct_pid_piping_component_blocks() {
        // The DWG backlog row pins count=2 for PIDPipingComponent;
        // this test locks the per-row cardinality so a future
        // refactor cannot accidentally collapse them into one.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![
            PublishObject {
                uid: "PC-A".into(),
                item_type_name: "PipingComp".into(),
                ..PublishObject::default()
            },
            PublishObject {
                uid: "PC-B".into(),
                item_type_name: "PipingComp".into(),
                ..PublishObject::default()
            },
        ];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert_eq!(
            out.matches("<PIDPipingComponent>").count(),
            2,
            "two PipingComp rows must open two <PIDPipingComponent> blocks; out:\n{out}"
        );
        assert!(out.contains(r#"<IObject UID="PC-A"/>"#));
        assert!(out.contains(r#"<IObject UID="PC-B"/>"#));
    }

    #[test]
    fn piping_comp_emits_interfaces_in_sppid_canonical_order() {
        // SPPID emits the 19 interfaces in a fixed order (confirmed
        // against both DWG samples at lines 204–224 and 225–245).
        // Pin that order so a future refactor cannot shuffle them.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "PC-ORD".into(),
            item_type_name: "PipingComp".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");

        let positions = [
            "<PIDPipingComponent>",
            r#"<IObject UID="PC-ORD"/>"#,
            "<IPBSItem ConstructionStatus=",
            "<IPipingPortComposition/>",
            "<IPlannedMatl/>",
            "<IDrawingItem/>",
            "<IPipingComponent ",
            "<IPipingComponentOcc/>",
            "<IInlineComponentOcc/>",
            "<IFabricatedItem/>",
            "<IHeatTracedItem ",
            "<IPartOcc/>",
            "<IPressureReliefItem/>",
            "<IDocumentItem/>",
            "<IElecPowerConsumer/>",
            "<IPart/>",
            "<INoteCollection/>",
            "<IPipeCrossSectionItem ",
            "<IInlineComponent ",
            "<IPIDTypical ",
            "</PIDPipingComponent>",
        ];
        let mut last_pos = 0usize;
        for needle in positions {
            let pos = out[last_pos..].find(needle).unwrap_or_else(|| {
                panic!("missing `{needle}` after offset {last_pos}\nout:\n{out}")
            });
            last_pos += pos + needle.len();
        }
    }

    // -----------------------------------------------------------------
    // A18 — PIDSignalConnector writer (SignalRun → 8-interface block)
    // -----------------------------------------------------------------

    #[test]
    fn signal_run_emits_pid_signal_connector_with_all_interfaces() {
        // Mirrors the reference sample at
        // DWG-0202GP06-01_Data.xml:1111–1120. A bare SignalRun
        // row must open every one of the 8 interfaces.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "E871304702F74D39B15BD2D8B41D34B3".into(),
            item_type_name: "SignalRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert_eq!(
            out.matches("<PIDSignalConnector>").count(),
            1,
            "single SignalRun must open exactly one <PIDSignalConnector>; out:\n{out}"
        );
        for needle in [
            r#"<IObject UID="E871304702F74D39B15BD2D8B41D34B3"/>"#,
            "<IPlannedFacility/>",
            r#"<IConnector FlowDirection=""/>"#,
            "<IDrawingItem/>",
            "<ISignalConnector/>",
            "<IDocumentItem/>",
            "<IExpandableThing/>",
            r#"<IPIDTypical IsTypical="False"/>"#,
            "</PIDSignalConnector>",
        ] {
            assert!(
                out.contains(needle),
                "PIDSignalConnector block must carry `{needle}`; out:\n{out}"
            );
        }
    }

    #[test]
    fn signal_run_propagates_populated_flow_direction() {
        // When a future loader learns to populate FlowDirection
        // from the appropriate T_* column, the writer must
        // surface it on <IConnector> instead of forcing the
        // empty-string default. This test pins the passthrough
        // path so the upgrade is a pure loader-side change.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "SR-FLOW".into(),
            item_type_name: "SignalRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("FlowDirection".into(), "@EE872".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IConnector FlowDirection="@EE872"/>"#),
            "populated FlowDirection must round-trip onto <IConnector>; out:\n{out}"
        );
    }

    #[test]
    fn signal_run_emits_no_piping_interfaces() {
        // PIDSignalConnector is deliberately minimal compared to
        // PIDPipingConnector — no IPBSItem envelope, no piping-
        // specific interfaces. Pin that contrast so a well-meaning
        // refactor that tries to share a common "connector"
        // writer doesn't accidentally inject piping-only
        // interfaces into the signal shape.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "SR-MIN".into(),
            item_type_name: "SignalRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        // Slice the output between `<PIDSignalConnector>` and
        // `</PIDSignalConnector>` so we only inspect the signal
        // block, not the rest of the document (Rel nodes etc.
        // may legitimately mention piping-adjacent strings).
        let open = out.find("<PIDSignalConnector>").expect("open");
        let close = out.find("</PIDSignalConnector>").expect("close");
        let block = &out[open..=close + "</PIDSignalConnector>".len()];
        for forbidden in [
            "IPBSItem",
            "IPipingConnector",
            "INamedPipingConnector",
            "IPipeCrossSectionItem",
            "IPipingSpecifiedItem",
            "IPipingPort",
        ] {
            assert!(
                !block.contains(forbidden),
                "PIDSignalConnector must NOT contain `{forbidden}`; block:\n{block}"
            );
        }
    }

    #[test]
    fn signal_run_maps_is_typical_true() {
        // IsTypical="True" on a SignalRun must round-trip via
        // map_bool when T_ModelItem.SP_IsTypical is `"1"`.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "SR-TYP".into(),
            item_type_name: "SignalRun".into(),
            is_typical: Some("1".into()),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IPIDTypical IsTypical="True"/>"#),
            "SignalRun with IsTypical=1 must emit `True`; out:\n{out}"
        );
    }

    #[test]
    fn signal_run_emits_interfaces_in_sppid_canonical_order() {
        // Pin the 8-interface canonical order observed in the DWG
        // reference fixture.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "SR-ORD".into(),
            item_type_name: "SignalRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        let positions = [
            "<PIDSignalConnector>",
            r#"<IObject UID="SR-ORD"/>"#,
            "<IPlannedFacility/>",
            "<IConnector ",
            "<IDrawingItem/>",
            "<ISignalConnector/>",
            "<IDocumentItem/>",
            "<IExpandableThing/>",
            "<IPIDTypical ",
            "</PIDSignalConnector>",
        ];
        let mut last_pos = 0usize;
        for needle in positions {
            let pos = out[last_pos..].find(needle).unwrap_or_else(|| {
                panic!("missing `{needle}` after offset {last_pos}\nout:\n{out}")
            });
            last_pos += pos + needle.len();
        }
    }

    // -----------------------------------------------------------------
    // A19 — PIDPipeline fidelity upgrade (add 4 missing interfaces +
    // FluidCode / FluidSystem attribute routing)
    // -----------------------------------------------------------------

    #[test]
    fn pipeline_emits_full_ten_interface_block_matching_reference() {
        // A01_Data.xml:54–65 and DWG-0202GP06-01_Data.xml:1369–1380
        // both ship the same 10-interface shape. Pin it end-to-end
        // so a future refactor that drops one of the wrapper
        // interfaces (IPBSItem / IPlannedFacility / IPBSItemCollection
        // / INoteCollection) will trip this assertion immediately.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "PIPE-F".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        for needle in [
            "<PIDPipeline>",
            "<IObject UID=",
            "<IPBSItem/>",
            "<IPlannedFacility/>",
            "<IPBSItemCollection/>",
            "<IPipeline/>",
            "<IPipingConnectorComposition/>",
            "<IFluidSystem",
            "<INoteCollection/>",
            "<IExpandableThing/>",
            "<IPIDTypical/>",
            "</PIDPipeline>",
        ] {
            assert!(
                out.contains(needle),
                "PIDPipeline block must carry `{needle}`; out:\n{out}"
            );
        }
    }

    #[test]
    fn pipeline_fluid_system_attrs_route_from_loader_fields() {
        // When the loader populates OperFluidCode + FluidSystem
        // from T_PipeRun, the writer surfaces them on
        // <IFluidSystem FluidCode="..." FluidSystem="..."/>. This
        // test locks that routing so a loader-side upgrade to
        // stamp those columns becomes immediately visible in the
        // emitted XML.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "PIPE-FS".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert(
                    "OperFluidCode".into(),
                    "@{63A6FC56-CB92-402D-8D92-BF9E2F204CE4}".into(),
                );
                m.insert(
                    "FluidSystem".into(),
                    "@{104E7730-99EF-49C6-A928-D8CD78394381}".into(),
                );
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(
                r#"<IFluidSystem FluidCode="@{63A6FC56-CB92-402D-8D92-BF9E2F204CE4}" FluidSystem="@{104E7730-99EF-49C6-A928-D8CD78394381}"/>"#
            ),
            "<IFluidSystem> must carry populated FluidCode + FluidSystem; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_fluid_system_attrs_default_to_empty_when_absent() {
        // DWG declares the FluidCode / FluidSystem attributes even
        // when the loader has not populated values; A01 keeps the
        // bare `<IFluidSystem/>` shape.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "PIPE-E".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IFluidSystem/>"#),
            "empty fluid fields should render the bare IFluidSystem shape; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_name_attribute_added_when_pipeline_name_field_present() {
        // DWG fixtures like `<IObject Name="A3jqz0101-OD" .../>`
        // stamp a human-readable pipeline name. When the loader
        // populates obj.fields["PipelineName"] the writer surfaces
        // it between UID and ItemTag on the IObject.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "PIPE-N".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("PipelineName".into(), "A3jqz0101-OD".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(
                r#"<IObject UID="PIPE-N" Name="A3jqz0101-OD" ItemTag=""#
            ),
            "Name attribute must appear between UID and ItemTag when PipelineName is populated; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_name_attribute_omitted_when_pipeline_name_empty() {
        // A01 fixtures do not ship PipelineName; the writer must
        // omit the Name="" attribute entirely to keep the IObject
        // element compact.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "PIPE-NONAME".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            !out.contains(r#"<IObject UID="PIPE-NONAME" Name="""#),
            "empty PipelineName must not emit a Name=\"\" attribute; out:\n{out}"
        );
        // The UID + ItemTag shape must still be intact.
        assert!(
            out.contains(r#"<IObject UID="PIPE-NONAME" ItemTag="#),
            "IObject must carry UID + ItemTag even without Name; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_emits_interfaces_in_sppid_canonical_order() {
        // Pin the 10-interface canonical order end-to-end via
        // find() cursor so a reorder would trip the assertion at
        // the exact problematic needle.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "PIPE-ORD".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        let positions = [
            "<PIDPipeline>",
            "<IObject UID=",
            "<IPBSItem/>",
            "<IPlannedFacility/>",
            "<IPBSItemCollection/>",
            "<IPipeline/>",
            "<IPipingConnectorComposition/>",
            "<IFluidSystem",
            "<INoteCollection/>",
            "<IExpandableThing/>",
            "<IPIDTypical/>",
            "</PIDPipeline>",
        ];
        let mut last_pos = 0usize;
        for needle in positions {
            let pos = out[last_pos..].find(needle).unwrap_or_else(|| {
                panic!("missing `{needle}` after offset {last_pos}\nout:\n{out}")
            });
            last_pos += pos + needle.len();
        }
    }

    // -----------------------------------------------------------------
    // A20 — PIDPipingConnector fidelity upgrade (7 → 22 interfaces,
    // + optional DWG-style attribute routing)
    // -----------------------------------------------------------------

    #[test]
    fn piping_connector_emits_full_twenty_two_interface_block() {
        // A01_Data.xml:66–89 ships this 22-interface shape bare.
        // Pin every wrapper interface so a regression that drops
        // one trips immediately.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "PIPE-F".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        for needle in [
            "<PIDPipingConnector>",
            "<IObject UID=",
            "<IPBSItem ConstructionStatus=",
            "<IPlannedFacility/>",
            "<IConnector/>",
            "<IDrawingItem/>",
            "<IPBSItemCollection/>",
            "<IPipingConnector/>",
            "<IFabricatedItem/>",
            "<IHeatTracedItem ",
            "<IProcessPointCollection/>",
            "<IDocumentItem/>",
            "<IElecPowerConsumer/>",
            "<INoteCollection/>",
            "<IProcessDataCaseComposition/>",
            "<IExpandableThing/>",
            "<INamedPipingConnector ",
            "<IPipeCrossSectionItem ",
            "<IPipingSpecifiedItem ",
            "<ISlopedPipingItem/>",
            "<IInsulatedItem/>",
            "<IJacketedItem/>",
            "<IPIDTypical ",
            "</PIDPipingConnector>",
        ] {
            assert!(
                out.contains(needle),
                "PIDPipingConnector block must carry `{needle}`; out:\n{out}"
            );
        }
    }

    #[test]
    fn piping_connector_defaults_match_a01_reference_construction_status() {
        // A01 ships `<IPBSItem ConstructionStatus="@NewConstruction"
        // ConstructionStatus2="@{78398AB4-...}"/>`; our writer must
        // emit the same canonical defaults when the loader has
        // not stamped the ConstructionStatus field.
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "PIPE-CON".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(
                r#"<IPBSItem ConstructionStatus="@NewConstruction" ConstructionStatus2="@{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}"/>"#
            ),
            "A01-canonical IPBSItem defaults must land on PIDPipingConnector; out:\n{out}"
        );
    }

    #[test]
    fn piping_connector_optional_attrs_render_bare_when_absent() {
        // A01 fixture's PIDPipingConnector ships bare
        // <IConnector/>, <IPipingConnector/>, <ISlopedPipingItem/>,
        // <IInsulatedItem/> — no attributes. Pin the bare paths so
        // they do not accidentally emit empty-attribute versions
        // (which would still parse but diverge from A01 bytes).
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "PIPE-BARE".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        for bare in [
            "<IConnector/>",
            "<IPipingConnector/>",
            "<ISlopedPipingItem/>",
            "<IInsulatedItem/>",
        ] {
            assert!(
                out.contains(bare),
                "A01 shape requires bare `{bare}`; out:\n{out}"
            );
        }
    }

    #[test]
    fn piping_connector_populates_optional_attrs_when_loader_supplies_them() {
        // DWG shape populates IConnector / IPipingConnector /
        // ISlopedPipingItem / IInsulatedItem with attributes. A20
        // routes those fields so a future loader-side upgrade
        // becomes immediately visible in the emitted XML.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "PIPE-DWG".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("FlowDirection".into(), "@EE872".into());
                m.insert("RepresentationsAreAllZeroLength".into(), "0".into());
                m.insert("PipingConnectorType".into(), "@EE690".into());
                m.insert("SlopedPipingAngle".into(), "2.9999910000486E-03 rad".into());
                m.insert(
                    "SlopedPipeDirection".into(),
                    "@{FAC6E20B-6B3C-48C4-BEE8-409B224925C2}".into(),
                );
                m.insert(
                    "InsulThickSrc".into(),
                    "@{1B53D013-9B24-11D6-BDA4-00104BCC2B69}".into(),
                );
                m.insert("TotalInsulThick".into(), "50 mm".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(out.contains(
            r#"<IConnector FlowDirection="@EE872" RepresentationsAreAllZeroLength="False"/>"#
        ));
        assert!(out.contains(r#"<IPipingConnector PipingConnectorType="@EE690"/>"#));
        assert!(out.contains(
            r#"<ISlopedPipingItem SlopedPipingAngle="2.9999910000486E-03 rad" SlopedPipeDirection="@{FAC6E20B-6B3C-48C4-BEE8-409B224925C2}"/>"#
        ));
        assert!(out.contains(
            r#"<IInsulatedItem InsulThickSrc="@{1B53D013-9B24-11D6-BDA4-00104BCC2B69}" TotalInsulThick="50 mm"/>"#
        ));
    }

    #[test]
    fn piping_connector_uses_name_attribute_on_dwg_shape() {
        // A29 makes the IObject shape an explicit decision via
        // [`PublishStyle`]. Pre-A29 the connector flipped to
        // Name-style as soon as `obj.fields["PipelineName"]` was
        // populated (an implicit, data-driven flip). Post-A29 the
        // caller must opt in via `drawing.style = Dwg` for the
        // DWG-shape IObject (`UID + Name`); the A01 shape
        // (`UID + ItemTag`) remains the default.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "PIPE-N".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert(
                    "PipelineName".into(),
                    "A3jqz0101-OD-100 mm-1.6AR12-WE-50mm".into(),
                );
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IObject UID="PIPE-N-CNX" Name="A3jqz0101-OD-100 mm-1.6AR12-WE-50mm"/>"#),
            "DWG-style PipingConnector with PipelineName populated must emit UID + Name IObject; out:\n{out}"
        );
        // The A01-shape ItemTag must NOT appear when we've
        // switched to the DWG shape.
        assert!(
            !out.contains(r#"<IObject UID="PIPE-N-CNX" ItemTag="#),
            "Name path must suppress the A01-shape ItemTag IObject; out:\n{out}"
        );
    }

    #[test]
    fn piping_connector_named_prefix_and_suffix_route_from_tag_columns() {
        // <INamedPipingConnector> needs all three SPPID tag
        // columns (TagPrefix / TagSequenceNo / TagSuffix) to
        // round-trip. A01 stamps only the sequence; DWG stamps
        // only the sequence. Pin both paths: prefix and suffix
        // remain empty when absent but populate when the loader
        // has them, so future fixtures with tag-prefixed
        // connectors continue working without further code
        // changes.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "PIPE-PF".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("TagPrefix".into(), "PFX".into());
                m.insert("TagSequenceNo".into(), "0101".into());
                m.insert("TagSuffix".into(), "SFX".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<INamedPipingConnector PipingConnectorPrefix="PFX" PipingConnectorSeqNo="0101" PipingConnectorSuff="SFX"/>"#),
            "all three tag columns must round-trip; out:\n{out}"
        );
    }

    #[test]
    fn piping_connector_emits_interfaces_in_sppid_canonical_order() {
        // Pin the 22-interface canonical order via find() cursor.
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "PIPE-O".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        let positions = [
            "<PIDPipingConnector>",
            "<IObject UID=",
            "<IPBSItem ",
            "<IPlannedFacility/>",
            "<IConnector",
            "<IDrawingItem/>",
            "<IPBSItemCollection/>",
            "<IPipingConnector",
            "<IFabricatedItem/>",
            "<IHeatTracedItem ",
            "<IProcessPointCollection/>",
            "<IDocumentItem/>",
            "<IElecPowerConsumer/>",
            "<INoteCollection/>",
            "<IProcessDataCaseComposition/>",
            "<IExpandableThing/>",
            "<INamedPipingConnector ",
            "<IPipeCrossSectionItem ",
            "<IPipingSpecifiedItem ",
            "<ISlopedPipingItem",
            "<IInsulatedItem",
            "<IJacketedItem/>",
            "<IPIDTypical ",
            "</PIDPipingConnector>",
        ];
        let mut last_pos = 0usize;
        for needle in positions {
            let pos = out[last_pos..].find(needle).unwrap_or_else(|| {
                panic!("missing `{needle}` after offset {last_pos}\nout:\n{out}")
            });
            last_pos += pos + needle.len();
        }
    }

    // -----------------------------------------------------------------
    // A21 — PIDProcessVessel fidelity upgrade (10 → 15 interfaces,
    // + DWG-style attribute routing on IEquipment / IPBSItem /
    // IProcessVessel / ISpecifiedMatlItem)
    // -----------------------------------------------------------------

    #[test]
    fn process_vessel_emits_full_fifteen_interface_block() {
        // Reference A01_Data.xml:12–28 ships 15 interfaces. Pin
        // every wrapper so a regression that drops one of the
        // five A21-added interfaces (IPBSItemCollection,
        // IPlannedMatl, IProcessEquipmentOcc, IDrawingItem,
        // ISpecifiedMatlItem) trips the assertion immediately.
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "V-F".into(),
            item_type_name: "Vessel".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        for needle in [
            "<PIDProcessVessel>",
            "<IObject UID=",
            "<IPIDProcessVesselOcc/>",
            "<IProcessVesselOcc/>",
            "<IEquipment ",
            "<IEquipmentOcc/>",
            "<IPBSItem ",
            "<IPBSItemCollection/>",
            "<IPlannedMatl/>",
            "<IProcessEquipment/>",
            "<IProcessEquipmentOcc/>",
            "<IProcessVessel/>",
            "<IDrawingItem/>",
            "<IPIDProcessVessel/>",
            "<ISpecifiedMatlItem/>",
            "<IPIDTypical ",
            "</PIDProcessVessel>",
        ] {
            assert!(
                out.contains(needle),
                "PIDProcessVessel block must carry `{needle}`; out:\n{out}"
            );
        }
    }

    #[test]
    fn process_vessel_ipbsitem_uses_canonical_defaults_when_height_absent() {
        // A01 fixture emits `<IPBSItem ConstructionStatus=
        // "@NewConstruction" ConstructionStatus2="@{78398AB4-...}"/>`
        // with no HeightRelativeToGrade. A21's writer must
        // reproduce that exact two-attribute form; adding
        // HeightRelativeToGrade would diverge from A01 bytes.
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "V-DEF".into(),
            item_type_name: "Vessel".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(
                r#"<IPBSItem ConstructionStatus="@NewConstruction" ConstructionStatus2="@{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}"/>"#
            ),
            "A01-canonical IPBSItem defaults (no HeightRelativeToGrade) must land; out:\n{out}"
        );
    }

    #[test]
    fn process_vessel_ipbsitem_includes_height_when_populated() {
        // DWG fixture emits
        // `<IPBSItem HeightRelativeToGrade="3 m" ConstructionStatus="..."
        // ConstructionStatus2="..."/>`. A21 must populate the
        // attribute when the loader supplies it.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "V-H".into(),
            item_type_name: "Vessel".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("HeightRelativeToGrade".into(), "3 m".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(
                r#"<IPBSItem HeightRelativeToGrade="3 m" ConstructionStatus="@NewConstruction" ConstructionStatus2="@{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}"/>"#
            ),
            "populated HeightRelativeToGrade must precede the two default attrs; out:\n{out}"
        );
    }

    #[test]
    fn process_vessel_iequipment_expands_with_eqtype_attrs() {
        // DWG ships the expanded IEquipment form with EqType0-3 +
        // EquipmentTrimSpec. A21 routes those fields so the DWG
        // shape round-trips when the loader stamps them.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "V-EQ".into(),
            item_type_name: "Vessel".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert(
                    "EqType0".into(),
                    "@{47BF0267-DD41-4E1A-9B41-C4B714C8FF92}".into(),
                );
                m.insert(
                    "EqType3".into(),
                    "@{9B3ED983-16AE-4AD7-A19F-A337149DF437}".into(),
                );
                m.insert("EqType2".into(), "@EE7A6".into());
                m.insert("EqType1".into(), "@EE793".into());
                m.insert("EquipmentTrimSpec".into(), "1.6AR12".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        // The expanded shape pins the DWG-canonical attribute
        // order: EqType0 / EqType3 / EqType2 / EqType1 /
        // EquipmentTrimSpec / EqTypeDescription.
        assert!(
            out.contains(
                r#"<IEquipment EqType0="@{47BF0267-DD41-4E1A-9B41-C4B714C8FF92}" EqType3="@{9B3ED983-16AE-4AD7-A19F-A337149DF437}" EqType2="@EE7A6" EqType1="@EE793" EquipmentTrimSpec="1.6AR12" EqTypeDescription="#
            ),
            "populated EqType0-3 + TrimSpec must expand IEquipment shape; out:\n{out}"
        );
    }

    #[test]
    fn process_vessel_iprocessvessel_includes_volumetric_capacity() {
        // DWG pairs `<IProcessVessel ProcessVessel_VesselVolumetricCapacity="27 m^3"/>`
        // when the loader stamps that column. A01 emits the bare
        // form `<IProcessVessel/>`.
        let mut d_a01 = PublishDrawing::new("UID-D", "A01");
        d_a01.objects = vec![PublishObject {
            uid: "V-A".into(),
            item_type_name: "Vessel".into(),
            ..PublishObject::default()
        }];
        let out_a01 = write_data_xml(&d_a01, "TEST02").expect("write");
        assert!(
            out_a01.contains("<IProcessVessel/>"),
            "A01 shape requires bare <IProcessVessel/>; out:\n{out_a01}"
        );
        let mut d_dwg = PublishDrawing::new("UID-D", "DWG");
        d_dwg.objects = vec![PublishObject {
            uid: "V-D".into(),
            item_type_name: "Vessel".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("VesselVolumetricCapacity".into(), "27 m^3".into());
                m
            },
            ..PublishObject::default()
        }];
        let out_dwg = write_data_xml(&d_dwg, "TEST02").expect("write");
        assert!(
            out_dwg
                .contains(r#"<IProcessVessel ProcessVessel_VesselVolumetricCapacity="27 m^3"/>"#),
            "populated VesselVolumetricCapacity must expand <IProcessVessel>; out:\n{out_dwg}"
        );
    }

    #[test]
    fn process_vessel_ispecifiedmatlitem_gains_long_material_description() {
        // DWG pairs `<ISpecifiedMatlItem LongMaterialDescription="新建"/>`
        // — round-trip the Chinese text through XML escaping.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "V-MAT".into(),
            item_type_name: "Vessel".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("LongMaterialDescription".into(), "新建".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<ISpecifiedMatlItem LongMaterialDescription="新建"/>"#),
            "populated LongMaterialDescription must land with Chinese text intact; out:\n{out}"
        );
    }

    #[test]
    fn process_vessel_emits_interfaces_in_sppid_canonical_order() {
        // Pin the 15-interface canonical order via find() cursor.
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "V-ORD".into(),
            item_type_name: "Vessel".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        let positions = [
            "<PIDProcessVessel>",
            "<IObject UID=",
            "<IPIDProcessVesselOcc/>",
            "<IProcessVesselOcc/>",
            "<IEquipment ",
            "<IEquipmentOcc/>",
            "<IPBSItem ",
            "<IPBSItemCollection/>",
            "<IPlannedMatl/>",
            "<IProcessEquipment/>",
            "<IProcessEquipmentOcc/>",
            "<IProcessVessel",
            "<IDrawingItem/>",
            "<IPIDProcessVessel/>",
            "<ISpecifiedMatlItem",
            "<IPIDTypical ",
            "</PIDProcessVessel>",
        ];
        let mut last_pos = 0usize;
        for needle in positions {
            let pos = out[last_pos..].find(needle).unwrap_or_else(|| {
                panic!("missing `{needle}` after offset {last_pos}\nout:\n{out}")
            });
            last_pos += pos + needle.len();
        }
    }

    // -----------------------------------------------------------------
    // A25 — PIDProcessVessel low-pressure-tank variant emission.
    // DWG-style "Open top tank" vessel variants emit two extra
    // interfaces (ILowPressureTankOcc + ILowPressureTank) between
    // IPIDProcessVessel and ISpecifiedMatlItem. The writer routes
    // the conditional emission off `obj.fields["IsLowPressureTank"]`
    // using `map_bool` for truthy evaluation so explicit False / 0
    // / empty stays in the non-tank branch.
    // -----------------------------------------------------------------

    #[test]
    fn process_vessel_omits_tank_interfaces_by_default() {
        // Pre-A25 contract must hold: with no IsLowPressureTank
        // signal, the writer emits the 15-interface A01 shape and
        // NEVER inserts ILowPressureTank[Occ] (regression guard).
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "V-NONTANK".into(),
            item_type_name: "Vessel".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            !out.contains("<ILowPressureTank"),
            "default vessel should not emit any ILowPressureTank-prefixed interface, got:\n{out}"
        );
    }

    #[test]
    fn process_vessel_omits_tank_interfaces_when_flag_is_explicit_false() {
        // Explicit "False" / "0" / "" must behave the same as
        // the absent case — no tank interfaces.
        for falsey in ["False", "false", "0", ""] {
            let mut d = PublishDrawing::new("UID-D", "A01");
            d.objects = vec![PublishObject {
                uid: "V-FLAG".into(),
                item_type_name: "Vessel".into(),
                fields: std::collections::BTreeMap::from([(
                    "IsLowPressureTank".to_string(),
                    falsey.to_string(),
                )]),
                ..PublishObject::default()
            }];
            let out = write_data_xml(&d, "TEST02").expect("write");
            assert!(
                !out.contains("<ILowPressureTank"),
                "falsey flag `{falsey}` must not emit tank interfaces, got:\n{out}"
            );
        }
    }

    #[test]
    fn process_vessel_emits_tank_interfaces_when_flag_is_true() {
        // Truthy signals must trigger both interfaces in
        // canonical (ILowPressureTankOcc before ILowPressureTank)
        // order, inserted AFTER IPIDProcessVessel and BEFORE
        // ISpecifiedMatlItem to match the DWG byte shape.
        for truthy in ["True", "true", "1"] {
            let mut d = PublishDrawing::new("UID-D", "DWG");
            d.objects = vec![PublishObject {
                uid: "V-TANK".into(),
                item_type_name: "Vessel".into(),
                fields: std::collections::BTreeMap::from([(
                    "IsLowPressureTank".to_string(),
                    truthy.to_string(),
                )]),
                ..PublishObject::default()
            }];
            let out = write_data_xml(&d, "TEST02").expect("write");
            assert!(
                out.contains("<ILowPressureTankOcc/>"),
                "truthy flag `{truthy}` must emit ILowPressureTankOcc, got:\n{out}"
            );
            assert!(
                out.contains("<ILowPressureTank/>"),
                "truthy flag `{truthy}` must emit ILowPressureTank, got:\n{out}"
            );
            let occ_pos = out.find("<ILowPressureTankOcc/>").unwrap();
            let tank_pos = out.find("<ILowPressureTank/>").unwrap();
            assert!(
                occ_pos < tank_pos,
                "flag `{truthy}`: ILowPressureTankOcc must come before ILowPressureTank, got Occ@{occ_pos} vs Tank@{tank_pos}",
            );
            let pidv_pos = out.find("<IPIDProcessVessel/>").unwrap();
            let spec_pos = out.find("<ISpecifiedMatlItem").unwrap();
            assert!(
                pidv_pos < occ_pos && tank_pos < spec_pos,
                "flag `{truthy}`: tank interfaces must slot between IPIDProcessVessel and ISpecifiedMatlItem, got PIDv@{pidv_pos} Occ@{occ_pos} Tank@{tank_pos} Spec@{spec_pos}",
            );
        }
    }

    #[test]
    fn process_vessel_tank_variant_emits_seventeen_interface_block() {
        // Pin the tank variant's full 17-interface canonical
        // order via find() cursor, mirroring the DWG fixture
        // sample at lines 1429–1447.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "V-TANK-ORD".into(),
            item_type_name: "Vessel".into(),
            description: Some("污水池".into()),
            fields: std::collections::BTreeMap::from([
                ("IsLowPressureTank".to_string(), "True".to_string()),
                (
                    "EqType0".to_string(),
                    "@{47BF0267-DD41-4E1A-9B41-C4B714C8FF92}".to_string(),
                ),
                ("EqType1".to_string(), "@EE793".to_string()),
                ("EqType2".to_string(), "@EE7A6".to_string()),
                (
                    "EqType3".to_string(),
                    "@{9B3ED983-16AE-4AD7-A19F-A337149DF437}".to_string(),
                ),
                ("EquipmentTrimSpec".to_string(), "1.6AR12".to_string()),
                ("HeightRelativeToGrade".to_string(), "3 m".to_string()),
                ("VesselVolumetricCapacity".to_string(), "27 m^3".to_string()),
                ("LongMaterialDescription".to_string(), "新建".to_string()),
            ]),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        let positions = [
            "<PIDProcessVessel>",
            "<IObject UID=",
            "<IPIDProcessVesselOcc/>",
            "<IProcessVesselOcc/>",
            "<IEquipment ",
            "<IEquipmentOcc/>",
            "<IPBSItem ",
            "<IPBSItemCollection/>",
            "<IPlannedMatl/>",
            "<IProcessEquipment/>",
            "<IProcessEquipmentOcc/>",
            "<IProcessVessel ",
            "<IDrawingItem/>",
            "<IPIDProcessVessel/>",
            "<ILowPressureTankOcc/>",
            "<ILowPressureTank/>",
            "<ISpecifiedMatlItem ",
            "<IPIDTypical ",
            "</PIDProcessVessel>",
        ];
        let mut last_pos = 0usize;
        for needle in positions {
            let pos = out[last_pos..].find(needle).unwrap_or_else(|| {
                panic!("missing `{needle}` after offset {last_pos}\nout:\n{out}")
            });
            last_pos += pos + needle.len();
        }
    }

    #[test]
    fn process_vessel_tank_variant_preserves_all_other_attributes() {
        // A25 must be strictly additive: enabling the flag
        // must not change any other interface's attribute
        // content. Compare the non-tank and tank shapes for
        // the same inputs and assert the tank version is the
        // non-tank version with exactly two new lines inserted
        // (ILowPressureTankOcc + ILowPressureTank).
        let mk = |is_tank: &str| -> String {
            let mut d = PublishDrawing::new("UID-D", "DWG");
            d.objects = vec![PublishObject {
                uid: "V-CMP".into(),
                item_type_name: "Vessel".into(),
                fields: std::collections::BTreeMap::from([(
                    "IsLowPressureTank".to_string(),
                    is_tank.to_string(),
                )]),
                ..PublishObject::default()
            }];
            write_data_xml(&d, "TEST02").expect("write")
        };
        let non_tank = mk("False");
        let tank = mk("True");
        let tank_without_extras = tank
            .replace("      <ILowPressureTankOcc/>\n", "")
            .replace("      <ILowPressureTank/>\n", "");
        assert_eq!(
            non_tank, tank_without_extras,
            "A25 tank variant should differ from non-tank ONLY by the two inserted interfaces, not by any other byte change.\nnon_tank:\n{non_tank}\ntank_without_extras:\n{tank_without_extras}"
        );
    }

    // -----------------------------------------------------------------
    // A22 — PIDNozzle fidelity upgrade (9 → 22 interfaces,
    // + DWG-style ProcessEqCompType1/2 attr routing,
    // + conditional bare shape for IPipeCrossSectionItem /
    //   IPipingSpecifiedItem)
    // -----------------------------------------------------------------

    // -----------------------------------------------------------------
    // A29 — explicit PublishStyle selector for IObject shape on
    // PIDPipeline / PIDPipingConnector / PIDProcessVessel.
    // -----------------------------------------------------------------

    #[test]
    fn default_publish_style_is_a01_so_pre_a29_callers_round_trip() {
        // Documents the default-style contract: an unset
        // `style` field equals A01, so every pre-A29 caller
        // round-trips bit-for-bit. A regression that flips
        // the default would silently change every existing
        // caller's emitted XML.
        let d = PublishDrawing::default();
        assert_eq!(d.style, PublishStyle::A01);
    }

    #[test]
    fn pipeline_dwg_style_iobject_drops_itemtag_and_uses_name_only() {
        // DWG reference emits `<IObject UID="..." Name="..."/>`
        // on PIDPipeline (two-attr shape, no ItemTag). Pre-A29
        // the writer would emit Name + ItemTag together when
        // PipelineName was populated; A29 routes it via the
        // explicit style selector and gives a clean DWG shape.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "PIPE-DWG".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("PipelineName".into(), "A3jqz0101-OD".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IObject UID="PIPE-DWG" Name="A3jqz0101-OD"/>"#),
            "DWG-style PIDPipeline IObject must be UID + Name only; out:\n{out}"
        );
        assert!(
            !out.contains(r#"<IObject UID="PIPE-DWG" Name="A3jqz0101-OD" ItemTag="#),
            "DWG-style must not retain the A01-shape ItemTag tail; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_dwg_style_with_no_pipeline_name_emits_uid_only_iobject() {
        // Defensive branch: DWG fixture itself only ships
        // pipelines with names, but we still need a sensible
        // emit for inputs that lack PipelineName under
        // style = Dwg.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "PIPE-DWG-NONAME".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IObject UID="PIPE-DWG-NONAME"/>"#),
            "DWG-style with no PipelineName must emit UID-only IObject; out:\n{out}"
        );
        assert!(
            !out.contains(r#"<IObject UID="PIPE-DWG-NONAME" ItemTag="#),
            "DWG-style must never fall back to ItemTag; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_a01_style_with_pipeline_name_keeps_pre_a29_three_attr_shape() {
        // Behavioral lock: style=A01 + PipelineName populated
        // continues to emit `Name + ItemTag` together,
        // matching the pre-A29 superset shape that A23 / A27
        // gates ratify.
        let mut d = PublishDrawing::new("UID-D", "A01");
        // No explicit assignment — default already PublishStyle::A01.
        d.objects = vec![PublishObject {
            uid: "PIPE-A01".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("PipelineName".into(), "PIPE-A01-NAME".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IObject UID="PIPE-A01" Name="PIPE-A01-NAME" ItemTag="#),
            "A01 default style + PipelineName must emit Name + ItemTag together; out:\n{out}"
        );
    }

    #[test]
    fn piping_connector_dwg_style_drops_itemtag() {
        // PipingConnector under DWG style: UID + Name when
        // PipelineName populated; matches DWG reference shape.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "PIPE-CNX-DWG".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("PipelineName".into(), "PIPE-CNX-DWG-NAME".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IObject UID="PIPE-CNX-DWG-CNX" Name="PIPE-CNX-DWG-NAME"/>"#),
            "DWG-style PipingConnector IObject must be UID + Name only; out:\n{out}"
        );
        assert!(
            !out.contains(r#"<IObject UID="PIPE-CNX-DWG-CNX" ItemTag="#),
            "DWG-style PipingConnector must drop ItemTag; out:\n{out}"
        );
    }

    #[test]
    fn process_vessel_dwg_style_drops_itemtag_keeps_description() {
        // DWG reference vessel IObject is `UID + Description`
        // (no ItemTag, no Name). The `污水池` sample at DWG:
        // 1430 demonstrates the shape. A29 routes it through
        // the new style flag.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "V-DWG".into(),
            item_type_name: "Vessel".into(),
            description: Some("污水池".into()),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("ItemTag".into(), "V-DWG-TAG".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IObject UID="V-DWG" Description="污水池"/>"#),
            "DWG-style PIDProcessVessel IObject must be UID + Description only; out:\n{out}"
        );
        // Even when the field carries an ItemTag, DWG style
        // must NOT emit the ItemTag attribute.
        assert!(
            !out.contains(r#"<IObject UID="V-DWG" ItemTag="#),
            "DWG-style vessel must drop ItemTag even when the field is populated; out:\n{out}"
        );
    }

    #[test]
    fn process_vessel_a01_style_keeps_three_attr_shape() {
        // Lock in the pre-A29 three-attr shape under default
        // style. This is the contract every supported_pid_tags()
        // / A23 / A27 gate already enforces; the explicit
        // assertion below makes the lock obvious.
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "V-A01".into(),
            item_type_name: "Vessel".into(),
            description: Some("Horizontal Drum".into()),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("ItemTag".into(), "V-A01-TAG".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(
                r#"<IObject UID="V-A01" ItemTag="V-A01-TAG" Description="Horizontal Drum"/>"#
            ),
            "A01-style vessel IObject must be UID + ItemTag + Description; out:\n{out}"
        );
    }

    #[test]
    fn nozzle_emits_full_twenty_two_interface_block() {
        // A01_Data.xml:29–53 reference has 22 interfaces. Pin
        // every wrapper so a regression that drops one of the 13
        // A22-added interfaces trips immediately.
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "NZ-F".into(),
            item_type_name: "Nozzle".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        for needle in [
            "<PIDNozzle>",
            r#"<IObject UID="NZ-F"/>"#,
            "<IPBSItem ",
            "<IPipingPortComposition/>",
            "<IPlannedMatl/>",
            "<IDrawingItem/>",
            "<INozzleOcc/>",
            "<INozzle/>",
            "<IEquipmentComponent ",
            "<IEquipmentComponentOcc/>",
            "<IFabricatedItem/>",
            "<IHeatTracedItem ",
            "<IPBSItemCollection/>",
            "<IProcessPointCollection/>",
            "<ISignalPortComposition/>",
            "<IPartOcc/>",
            "<IDocumentItem/>",
            "<IElecPowerConsumer/>",
            "<IPart/>",
            "<INoteCollection/>",
            "<IProcessDataCaseComposition/>",
            "<IPipeCrossSectionItem/>",
            "<IPipingSpecifiedItem/>",
            "<IPIDTypical ",
            "</PIDNozzle>",
        ] {
            assert!(
                out.contains(needle),
                "PIDNozzle block must carry `{needle}`; out:\n{out}"
            );
        }
    }

    #[test]
    fn nozzle_ipipe_cross_section_item_bare_when_nominal_diameter_absent() {
        // A01 ships bare <IPipeCrossSectionItem/> (no attribute)
        // on PIDNozzle; only PIDPipingConnector populates
        // NominalDiameter. Pre-A22 writer forced an empty
        // attribute here, diverging from A01 bytes.
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "NZ-BARE".into(),
            item_type_name: "Nozzle".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains("<IPipeCrossSectionItem/>"),
            "A01 shape requires bare <IPipeCrossSectionItem/> on PIDNozzle; out:\n{out}"
        );
        assert!(
            out.contains("<IPipingSpecifiedItem/>"),
            "A01 shape requires bare <IPipingSpecifiedItem/> on PIDNozzle; out:\n{out}"
        );
    }

    #[test]
    fn nozzle_expands_cross_section_and_specified_item_when_populated() {
        // DWG-style populated form. When the loader stamps
        // NominalDiameter + PipingMaterialsClass, the writer must
        // switch to the attribute-bearing shape.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "NZ-POP".into(),
            item_type_name: "Nozzle".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("NominalDiameter".into(), "100".into());
                m.insert("PipingMaterialsClass".into(), "1.6AR12".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IPipeCrossSectionItem NominalDiameter="100 mm"/>"#),
            "populated NominalDiameter must land with mm suffix; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IPipingSpecifiedItem PipingMaterialsClass="1.6AR12"/>"#),
            "populated PipingMaterialsClass must land; out:\n{out}"
        );
    }

    #[test]
    fn nozzle_iequipment_component_gains_process_eq_comp_type_attrs() {
        // DWG ships `<IEquipmentComponent ProcessEqCompType1="@EE6D4"
        // ProcessEqCompType2="@{...}" ProcEqpCompTypeDescription="..."/>`.
        // A22 routes those fields so a future loader-side upgrade
        // becomes visible without touching the writer again.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "NZ-TYPE".into(),
            item_type_name: "Nozzle".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("ProcessEqCompType1".into(), "@EE6D4".into());
                m.insert(
                    "ProcessEqCompType2".into(),
                    "@{B88907F5-D4FC-49D8-BA8E-C1F76F392A52}".into(),
                );
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(
                r#"<IEquipmentComponent ProcessEqCompType1="@EE6D4" ProcessEqCompType2="@{B88907F5-D4FC-49D8-BA8E-C1F76F392A52}" ProcEqpCompTypeDescription="Flanged Nozzle"/>"#
            ),
            "expanded DWG-shape IEquipmentComponent must carry all three attrs in canonical order; out:\n{out}"
        );
    }

    #[test]
    fn nozzle_defaults_to_a17_canonical_ipbsitem_values() {
        // Every pipe-composed tag (PipingComponent / PipingConnector
        // / Vessel / Nozzle) uses the same canonical IPBSItem
        // defaults. Pin them so a future change that diverges on
        // one tag alone will trip here.
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "NZ-DEF".into(),
            item_type_name: "Nozzle".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(
                r#"<IPBSItem ConstructionStatus="@NewConstruction" ConstructionStatus2="@{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}"/>"#
            ),
            "A01-canonical IPBSItem defaults must land on PIDNozzle; out:\n{out}"
        );
    }

    #[test]
    fn nozzle_emits_interfaces_in_sppid_canonical_order() {
        // Pin the 22-interface canonical order via find() cursor.
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "NZ-O".into(),
            item_type_name: "Nozzle".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        let positions = [
            "<PIDNozzle>",
            "<IObject UID=",
            "<IPBSItem ",
            "<IPipingPortComposition/>",
            "<IPlannedMatl/>",
            "<IDrawingItem/>",
            "<INozzleOcc/>",
            "<INozzle/>",
            "<IEquipmentComponent ",
            "<IEquipmentComponentOcc/>",
            "<IFabricatedItem/>",
            "<IHeatTracedItem ",
            "<IPBSItemCollection/>",
            "<IProcessPointCollection/>",
            "<ISignalPortComposition/>",
            "<IPartOcc/>",
            "<IDocumentItem/>",
            "<IElecPowerConsumer/>",
            "<IPart/>",
            "<INoteCollection/>",
            "<IProcessDataCaseComposition/>",
            "<IPipeCrossSectionItem",
            "<IPipingSpecifiedItem",
            "<IPIDTypical ",
            "</PIDNozzle>",
        ];
        let mut last_pos = 0usize;
        for needle in positions {
            let pos = out[last_pos..].find(needle).unwrap_or_else(|| {
                panic!("missing `{needle}` after offset {last_pos}\nout:\n{out}")
            });
            last_pos += pos + needle.len();
        }
    }

    // -----------------------------------------------------------------
    // A24 — final fidelity pass: IPBSItem defaults on
    // PIDControlSystemFunction + bare-when-empty INote on PIDNote
    // -----------------------------------------------------------------

    #[test]
    fn control_system_function_ipbsitem_uses_canonical_defaults() {
        // A24: PIDControlSystemFunction joins the uniform IPBSItem
        // defaults used by A17/A20/A21/A22. The DWG reference has
        // `<IPBSItem ConstructionStatus="@NewConstruction"
        // ConstructionStatus2="@{78398AB4-...}"/>` and pre-A24 our
        // writer emitted a bare `<IPBSItem/>` which diverged.
        // NB: PIDDrawing legitimately keeps `<IPBSItem/>` bare
        // (its reference shape), so the bare-form check must be
        // scoped to the ControlSystemFunction block only.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-A24".into(),
            item_type_name: "InstrFunction".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(
                r#"<IPBSItem ConstructionStatus="@NewConstruction" ConstructionStatus2="@{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}"/>"#
            ),
            "PIDControlSystemFunction must use canonical IPBSItem defaults; out:\n{out}"
        );
        // Slice out the ControlSystemFunction block and assert
        // the bare form does not appear there (it's fine inside
        // PIDDrawing, which has a different canonical shape).
        let open = out
            .find("<PIDControlSystemFunction>")
            .expect("function block open");
        let close = out
            .find("</PIDControlSystemFunction>")
            .expect("function block close");
        let block = &out[open..=close + "</PIDControlSystemFunction>".len()];
        assert!(
            !block.contains("<IPBSItem/>"),
            "A24 must no longer emit bare <IPBSItem/> inside PIDControlSystemFunction; block:\n{block}"
        );
    }

    #[test]
    fn control_system_function_ipbsitem_allows_field_override() {
        // The canonical defaults are overridable when the loader
        // populates alternate ConstructionStatus columns — matches
        // the same override path used by PipingComponent /
        // PipingConnector / ProcessVessel / Nozzle.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-OVR".into(),
            item_type_name: "InstrFunction".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("ConstructionStatus".into(), "@Revised".into());
                m.insert("ConstructionStatus2".into(), "@{CUSTOM-GUID}".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(
                r#"<IPBSItem ConstructionStatus="@Revised" ConstructionStatus2="@{CUSTOM-GUID}"/>"#
            ),
            "loader-supplied ConstructionStatus values must override the SPPID defaults; out:\n{out}"
        );
    }

    #[test]
    fn note_with_populated_text_still_emits_attribute_form() {
        // Keep the populated-text path intact (A24 only changed
        // the empty-text path). Verifies that notes with Chinese
        // CR/LF content still escape correctly.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "NOTE-TEXT".into(),
            item_type_name: "Note".into(),
            description: Some("量液孔".into()),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<INote NoteText="量液孔"/>"#),
            "populated NoteText must round-trip through the attribute form; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_dwg_style_falls_back_to_raw_name_field() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "PIPE-RAW-NAME".into(),
            item_type_name: "PipeRun".into(),
            fields: std::collections::BTreeMap::from([(
                "Name".to_string(),
                "A3jqz0101-OD".to_string(),
            )]),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IObject UID="PIPE-RAW-NAME" Name="A3jqz0101-OD"/>"#),
            "DWG-style PIDPipeline must accept raw T_PlantItem.Name as fallback; out:\n{out}"
        );
    }

    #[test]
    fn piping_connector_dwg_style_uses_raw_pipe_run_field_aliases() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "PIPE-RAW-ALIAS".into(),
            item_type_name: "PipeRun".into(),
            fields: std::collections::BTreeMap::from([
                ("Name".to_string(), "RAW-CONNECTOR".to_string()),
                ("FlowDirection".to_string(), "@EE873".to_string()),
                ("SP_ConnectorsZeroLength".to_string(), "1".to_string()),
                ("PipeRunType".to_string(), "Stub".to_string()),
                ("Slope".to_string(), "0.125".to_string()),
                ("SlopeDirection".to_string(), "@UP".to_string()),
                ("InsulationThkSource".to_string(), "@SRC".to_string()),
                ("InsulThick".to_string(), "12 mm".to_string()),
            ]),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IObject UID="PIPE-RAW-ALIAS-CNX" Name="RAW-CONNECTOR"/>"#),
            "DWG-style connector IObject must accept raw Name fallback; out:\n{out}"
        );
        assert!(
            out.contains(
                r#"<IConnector FlowDirection="@EE873" RepresentationsAreAllZeroLength="True"/>"#
            ),
            "raw zero-length flag must map onto RepresentationsAreAllZeroLength; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IPipingConnector PipingConnectorType="Stub"/>"#),
            "PipeRunType must map onto PipingConnectorType under DWG style; out:\n{out}"
        );
        assert!(
            out.contains(
                r#"<ISlopedPipingItem SlopedPipingAngle="0.125" SlopedPipeDirection="@UP"/>"#
            ),
            "Slope/SlopeDirection must feed the DWG sloped-piping shape; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IInsulatedItem InsulThickSrc="@SRC" TotalInsulThick="12 mm"/>"#),
            "InsulationThkSource/InsulThick must feed the DWG insulated-item shape; out:\n{out}"
        );
    }

    #[test]
    fn process_vessel_dwg_style_uses_trimspec_and_volume_rating_fallbacks() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "VESSEL-RAW-FALLBACK".into(),
            item_type_name: "Vessel".into(),
            fields: std::collections::BTreeMap::from([
                ("TrimSpec".to_string(), "1.6AR12".to_string()),
                ("VolumeRating".to_string(), "27 m^3".to_string()),
            ]),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"EquipmentTrimSpec="1.6AR12""#),
            "DWG-style vessel must accept TrimSpec as EquipmentTrimSpec fallback; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IProcessVessel ProcessVessel_VesselVolumetricCapacity="27 m^3"/>"#),
            "DWG-style vessel must accept VolumeRating as volumetric-capacity fallback; out:\n{out}"
        );
    }

    // -----------------------------------------------------------------
    // Stage-4 — PIDBranchPoint + PIDPipingBranchPoint writer arms.
    // -----------------------------------------------------------------

    #[test]
    fn piping_branch_point_emits_six_interface_shape() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "CCB3BA926FC54BF89691BC690FAF7D74.BPT".into(),
            item_type_name: "PipingBranchPoint".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "P01").expect("write");
        for needle in [
            "<PIDPipingBranchPoint>",
            r#"<IObject UID="CCB3BA926FC54BF89691BC690FAF7D74.BPT"/>"#,
            "<IConnection/>",
            "<IPipingConnection/>",
            "<IDrawingItem/>",
            "<IPipingBranchPoint/>",
            "<IDocumentItem/>",
            "</PIDPipingBranchPoint>",
        ] {
            assert!(
                out.contains(needle),
                "PIDPipingBranchPoint must carry `{needle}`; out:\n{out}"
            );
        }
    }

    #[test]
    fn piping_branch_point_interface_ordering_matches_reference() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "AAA.BPT".into(),
            item_type_name: "PipingBranchPoint".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "P01").expect("write");
        let positions = [
            "<PIDPipingBranchPoint>",
            "<IObject UID=",
            "<IConnection/>",
            "<IPipingConnection/>",
            "<IDrawingItem/>",
            "<IPipingBranchPoint/>",
            "<IDocumentItem/>",
            "</PIDPipingBranchPoint>",
        ];
        let mut last_pos = 0usize;
        for needle in positions {
            let pos = out[last_pos..].find(needle).unwrap_or_else(|| {
                panic!(
                    "PIDPipingBranchPoint: `{needle}` not found after position {last_pos}; out:\n{out}"
                )
            }) + last_pos;
            last_pos = pos + needle.len();
        }
    }

    #[test]
    fn pid_branch_point_emits_eight_interface_shape_with_name() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "0DFD856D382C42F88DA8CDDFD37D4227".into(),
            item_type_name: "BranchPoint".into(),
            fields: std::collections::BTreeMap::from([("Name".to_string(), "272".to_string())]),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "P01").expect("write");
        for needle in [
            "<PIDBranchPoint>",
            r#"<IObject UID="0DFD856D382C42F88DA8CDDFD37D4227" Name="272"/>"#,
            "<IPIDBranchPoint/>",
            "<IDuctConnection/>",
            "<IConnection/>",
            "<IDrawingItem/>",
            "<IPipingConnection/>",
            "<ISignalConnection/>",
            "<IDocumentItem/>",
            "</PIDBranchPoint>",
        ] {
            assert!(
                out.contains(needle),
                "PIDBranchPoint must carry `{needle}`; out:\n{out}"
            );
        }
    }

    #[test]
    fn pid_branch_point_interface_ordering_matches_reference() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "BBB".into(),
            item_type_name: "BranchPoint".into(),
            fields: std::collections::BTreeMap::from([("Name".to_string(), "1".to_string())]),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "P01").expect("write");
        let positions = [
            "<PIDBranchPoint>",
            r#"<IObject UID="BBB" Name="1"/>"#,
            "<IPIDBranchPoint/>",
            "<IDuctConnection/>",
            "<IConnection/>",
            "<IDrawingItem/>",
            "<IPipingConnection/>",
            "<ISignalConnection/>",
            "<IDocumentItem/>",
            "</PIDBranchPoint>",
        ];
        let mut last_pos = 0usize;
        for needle in positions {
            let pos = out[last_pos..].find(needle).unwrap_or_else(|| {
                panic!(
                    "PIDBranchPoint: `{needle}` not found after position {last_pos}; out:\n{out}"
                )
            }) + last_pos;
            last_pos = pos + needle.len();
        }
    }

    #[test]
    fn pid_branch_point_omits_name_attr_when_field_is_empty() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "CCC".into(),
            item_type_name: "BranchPoint".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "P01").expect("write");
        assert!(
            out.contains(r#"<IObject UID="CCC"/>"#),
            "PIDBranchPoint IObject must omit Name when the field is missing; out:\n{out}"
        );
        let branch_block = out
            .split("<PIDBranchPoint>")
            .nth(1)
            .and_then(|rest| rest.split("</PIDBranchPoint>").next())
            .unwrap_or("");
        assert!(
            !branch_block.contains("Name="),
            "PIDBranchPoint block must not contain Name= when field is missing; block:\n{branch_block}"
        );
    }

    #[test]
    fn pid_branch_point_falls_back_to_description_for_name() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "DDD".into(),
            item_type_name: "BranchPoint".into(),
            description: Some("99".to_string()),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "P01").expect("write");
        assert!(
            out.contains(r#"<IObject UID="DDD" Name="99"/>"#),
            "PIDBranchPoint must fall back to description for Name; out:\n{out}"
        );
    }
}
