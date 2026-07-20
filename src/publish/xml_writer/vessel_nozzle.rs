//! Vessel and nozzle business-object XML emitters.

use std::fmt::Write;

use super::super::model::{PublishDrawing, PublishError, PublishObject, PublishStyle};
use super::common::{
    canonical_construction_status, canonical_construction_status2, canonical_is_typical,
    dwg_field_with_aliases, escape_attr, fmt_err, format_diameter, map_bool,
};

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

/// Emit the full `<PIDProcessVessel>` block for a Vessel row.
///
/// The reference shape has 15 interfaces, confirmed byte-for-byte
/// against both `A01_Data.xml:12–28` and
/// `DWG-0202GP06-01_Data.xml:1429–1447`.
///
/// A21 closes a 5-interface fidelity gap: the pre-A21 writer
/// emitted only 10 interfaces (`IObject`, `IPIDProcessVesselOcc`,
/// `IProcessVesselOcc`, `IEquipment`, `IEquipmentOcc`, `IPBSItem`,
/// `IProcessEquipment`, `IProcessVessel`, `IPIDProcessVessel`,
/// `IPIDTypical`). The five missing wrapper interfaces are now
/// emitted in SPPID-canonical order:
///
/// * `IPBSItemCollection` (between `IPBSItem` and `IPlannedMatl`)
/// * `IPlannedMatl`
/// * `IProcessEquipmentOcc` (next to `IProcessEquipment`)
/// * `IDrawingItem` (between `IProcessVessel` and `IPIDProcessVessel`)
/// * `ISpecifiedMatlItem` (after `IPIDProcessVessel`)
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
/// from `T_ProcessEquipment`'s `EqType` columns) is deferred to
/// A25b — until then, callers synthesising `PublishObjects`
/// manually can set the field directly.
pub(super) fn write_process_vessel(
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
/// only 9 interfaces (`IObject`, `IPipingPortComposition`, `INozzleOcc`,
/// `INozzle`, `IEquipmentComponent`, `IEquipmentComponentOcc`,
/// `IPipeCrossSectionItem`, `IPipingSpecifiedItem`, `IPIDTypical`).
/// The 13 missing wrapper interfaces are now emitted in SPPID
/// canonical order:
///
/// * `IPBSItem ConstructionStatus=... ConstructionStatus2=...`
///   (inserted right after `IObject` with A17-canonical defaults).
/// * `IPlannedMatl`, `IDrawingItem` (before the per-nozzle
///   `INozzleOcc` / `INozzle` pair).
/// * `IFabricatedItem`, `IHeatTracedItem HTraceRqmt="..."` (after
///   `IEquipmentComponentOcc`).
/// * `IPBSItemCollection`, `IProcessPointCollection`,
///   `ISignalPortComposition`, `IPartOcc`, `IDocumentItem`,
///   `IElecPowerConsumer`, `IPart`, `INoteCollection`,
///   `IProcessDataCaseComposition` (between `IHeatTracedItem` and
///   `IPipeCrossSectionItem`).
///
/// A22 also upgrades:
/// * `IEquipmentComponent` — expanded-attr DWG form gains
///   `ProcessEqCompType1` + `ProcessEqCompType2` leading
///   attributes when the loader populates the corresponding
///   `T_Nozzle` columns. A01 shape stays single-attribute
///   (`ProcEqpCompTypeDescription` alone).
/// * `IPipeCrossSectionItem` — now renders bare (A01) when
///   `NominalDiameter` is absent; with the attribute (DWG) when
///   populated. Pre-A22 forced an empty attribute even when
///   absent, diverging from the A01 bare shape.
/// * `IPipingSpecifiedItem` — same conditional path for
///   `PipingMaterialsClass` (bare in A01; populated in DWG).
pub(super) fn write_nozzle(
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
/// A29 · Render `<IObject>` for the `<PIDProcessVessel>`
/// body. Vessel `IObject` shape:
///
/// * **A01 style** (default) — `UID + ItemTag + Description`
///   three-attribute shape that has been live since the
///   initial publish writer landed. Every existing test
///   exercises this path.
/// * **DWG style** — `UID + Description` two-attribute
///   shape. The DWG reference omits the identifier on the
///   vessel `IObject` entirely (the `污水池` sample carries
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
/// Render the `SmartPlant` composite equipment tag ("`TagPrefix`
/// `TagSequenceNo`") from a vessel / equipment row's business
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
