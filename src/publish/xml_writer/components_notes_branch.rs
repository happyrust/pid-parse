//! Piping-component, note, branch-point, and generic fallback XML emitters.

use std::fmt::Write;

use super::super::model::{PublishError, PublishObject};
use super::common::{escape_attr, fmt_err, format_diameter, map_bool, non_empty_field};

/// Emit a `<PIDNote>` block for a Note / `ItemNote` object.
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
/// `SmartPlant` client puts the note body for plain annotation rows).
/// Fixtures whose business subtable has stamped a dedicated
/// `NoteText` field win over `Description`. When neither is
/// present the attribute renders empty rather than fabricating a
/// placeholder.
pub(super) fn write_item_note(buf: &mut String, obj: &PublishObject) -> Result<(), PublishError> {
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
/// Emit a `<PIDPipingComponent>` block for a `PipingComp` object.
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
pub(super) fn write_piping_component(
    buf: &mut String,
    obj: &PublishObject,
) -> Result<(), PublishError> {
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
pub(super) fn write_piping_branch_point(
    buf: &mut String,
    obj: &PublishObject,
) -> Result<(), PublishError> {
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
/// number. All interfaces below `IObject` are bare. `Name` is
/// sourced from `obj.fields["Name"]` falling back to
/// `obj.description` — the loader must populate one of them.
pub(super) fn write_pid_branch_point(
    buf: &mut String,
    obj: &PublishObject,
) -> Result<(), PublishError> {
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

pub(super) fn write_generic_object(
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
