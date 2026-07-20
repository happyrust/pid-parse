//! Pipeline, piping connector, port, and process-point XML emitters.

use std::fmt::Write;

use super::super::model::{PublishError, PublishObject, PublishStyle};
use super::common::{
    canonical_construction_status, canonical_construction_status2, canonical_is_typical,
    dwg_field_with_aliases, escape_attr, fmt_err, format_diameter, format_insulation_inches,
    map_bool, non_empty_field, non_empty_field_any,
};

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

/// Derive the `<PIDPipingConnector>` `IObject` UID from the parent
/// `<PIDPipeline>` (`PipeRun`) UID.
///
/// Current state: this still uses the stage-1 placeholder
/// convention `<pipe_uid>-CNX`, with downstream `.1` / `.2` /
/// `.PPT` children appended on top. That keeps the document
/// internally self-consistent and byte-stable, but it is NOT yet
/// the real `SmartPlant` publish-time numbering rule. The A01 raw
/// residual gates intentionally keep this family under explicit
/// burn-down until the true rule is reconstructed from TEST02.
pub(super) fn derived_pipe_connector_uid(pipe_uid: &str) -> String {
    format!("{pipe_uid}-CNX")
}
/// Resolve the `ItemTag` attribute for a pipeline-like object (a
/// `PipeRun` row that drives both `<PIDPipeline>` and
/// `<PIDPipingConnector>`). Order of preference:
///
/// 1. `obj.fields["ItemTag"]` — populated by the loader from
///    `T_PlantItem.ItemTag`. This is the canonical tag `SmartPlant`
///    itself stores (e.g. `"A010102102-PH"` in the TEST02 fixture)
///    and is what Publish Data XML consumers expect.
/// 2. Legacy synthesized form `PH-{seq}-{dia}-{class}` — kept as a
///    fallback so drawings without a `T_PlantItem` row still emit a
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
///   shape: when `pipeline_name` is populated the `IObject`
///   emits all three of `UID` / `Name` / `ItemTag`; when
///   absent, only `UID` / `ItemTag`. This is the strict
///   superset shape that has been live since A19, so every
///   pre-A29 caller / fixture round-trips bit-for-bit.
/// * **DWG style** — the `IObject` drops `ItemTag` to match
///   the DWG reference (`<IObject UID="..." Name="..."/>`
///   two-attribute shape). When `pipeline_name` is absent
///   the writer emits a UID-only `IObject`; the DWG fixture
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
/// shape is a strict two-attribute `IObject` (no `Name`
/// attribute even when the loader has the data) — this
/// matches the connector's pre-A29 behavior, which only
/// flipped to Name-style `IObject` when `PipelineName` was
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

/// Emit the full `<PIDPipeline>` block for a `PipeRun` row.
///
/// The reference `SmartPlant` shape (confirmed byte-for-byte on
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
///   which `SmartPlant` always emits but our earlier writer
///   silently dropped.
/// * Populates `IFluidSystem FluidCode="..." FluidSystem="..."`
///   from `obj.fields["OperFluidCode"]` + `obj.fields["FluidSystem"]`
///   (`T_PipeRun` columns loaded by the `sqlite_load` layer). When
///   either value is absent the respective attribute renders
///   empty, matching the A01 fixture shape (`<IFluidSystem/>`
///   without attributes, which under A19 becomes
///   `<IFluidSystem FluidCode="" FluidSystem=""/>` — still a
///   fidelity improvement for downstream consumers expecting
///   the attributes to be declared).
///
/// A19 also adds an optional `Name=` attribute on the `IObject`
/// when the loader populates `obj.fields["PipelineName"]` or
/// falls back to the item tag. The DWG reference uses e.g.
/// `Name="A3jqz0101-OD"` for pipeline labels; A01 uses
/// unlabeled pipelines and the attribute is omitted to match.
///
/// A29 introduces an explicit [`PublishStyle`] selector so the
/// `IObject` shape no longer relies on `obj.fields["PipelineName"]`
/// alone. With `style = A01` (default), the pre-A29 behaviour
/// is preserved bit-for-bit. With `style = Dwg` (set by callers
/// that loaded a DWG-flavor `SQLite` mirror), the `IObject` drops
/// the `ItemTag` attribute — matching the DWG reference's
/// `<IObject UID="..." Name="..."/>` two-attribute shape.
pub(super) fn write_pipeline(
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

/// Emit the full `<PIDPipingConnector>` block for a `PipeRun` row.
///
/// The reference `SmartPlant` shape is a 22-interface block
/// (confirmed byte-for-byte on `A01_Data.xml:66–89` — 22
/// interfaces, all bare except the named/sized ones — and
/// `DWG-0202GP06-01_Data.xml:246–269` — the same 22 interfaces
/// plus populated optional attributes on `IConnector` /
/// `IPipingConnector` / `ISlopedPipingItem` / `IInsulatedItem`).
///
/// A20 closes a 15-interface fidelity gap. Pre-A20 writer emitted
/// only 7 interfaces (`IObject`, `IConnector`, `IPipingConnector`,
/// `INamedPipingConnector`, `IPipeCrossSectionItem`, `IPipingSpecifiedItem`,
/// `IPIDTypical`). The 15 missing wrapper interfaces
/// (`IPBSItem`, `IPlannedFacility`, `IDrawingItem`, `IPBSItemCollection`,
/// `IFabricatedItem`, `IHeatTracedItem`, `IProcessPointCollection`,
/// `IDocumentItem`, `IElecPowerConsumer`, `INoteCollection`,
/// `IProcessDataCaseComposition`, `IExpandableThing`,
/// `ISlopedPipingItem`, `IInsulatedItem`, `IJacketedItem`) are now
/// emitted unconditionally in SPPID-canonical order.
///
/// Attribute routing:
/// * `IObject` — `UID` is the derived `<piperun>-CNX`. A01 uses
///   `ItemTag="..."`; DWG uses `Name="..."` instead (same
///   field, different `SmartPlant` exporter versions). The writer
///   emits `Name="..."` when `obj.fields["PipelineName"]` is
///   populated (DWG-shape), otherwise `ItemTag="..."` (A01-shape).
/// * `IPBSItem` — same defaults as A17's `PIDPipingComponent`
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
/// arrive from `T_PipeRun` / `T_Connector`.
pub(super) fn write_piping_connector(
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

/// Emit the three virtual nodes `SmartPlant` always derives from a
/// `PipingConnector`: two `<PIDPipingPort>` children (suffixed `.1`
/// and `.2`, both inheriting the parent connector's nominal
/// diameter) plus one `<PIDProcessPoint>` (suffixed `.PPT`).
///
/// These nodes never appear as their own `SQLite` rows — they are
/// `SmartPlant` client-side composition members rendered by the
/// exporter at publish time. The base UID is the same
/// deterministic connector UID the `<PIDPipingConnector>`
/// block carries, with the `SmartPlant` suffixes `.1`, `.2`,
/// `.PPT`.
pub(super) fn write_derived_connector_endpoints(
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
pub(super) fn write_piping_port(buf: &mut String, obj: &PublishObject) -> Result<(), PublishError> {
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
