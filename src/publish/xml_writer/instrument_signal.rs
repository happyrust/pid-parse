//! Instrument, signal-port, and signal-connector XML emitters.

use std::fmt::Write;

use super::super::model::{PublishError, PublishObject};
use super::common::{escape_attr, fmt_err, map_bool};

/// Number of `<PIDSignalPort>` children `SmartPlant` always derives
/// from a single `InstrFunction` / Instrument row. Pinned at 8
/// because the DWG-0202GP06-01 reference fixture emits exactly
/// `<instr>.1` through `<instr>.8` for each of its two
/// `InstrFunction` objects (16 ports total = the A15 backlog
/// count). Future fixtures that exhibit a different cardinality
/// will let this constant become a per-object derivation rule.
pub(super) const INSTR_DERIVED_SIGNAL_PORT_COUNT: u8 = 8;
/// Emit `count` `<PIDSignalPort>` children `SmartPlant` derives
/// from a single `InstrFunction` / Instrument row.
///
/// Each port carries:
/// * `IObject UID="<instr>.N" Name="N"` — index-suffixed UID matching
///   the `SmartPlant` `<instr>.{1..count}` pattern (verified against
///   `DWG-0202GP06-01_Data.xml`).
/// * Five empty interface tags (`IConnection`, `ISignalConnection`,
///   `ISignalPort`, `IPipingConnection`, `IFacilityPoint`) — exact
///   shape of the reference fixture.
/// * `IPIDTypical` (no `IsTypical` attribute, matching the
///   reference; Vessel/Nozzle's typical attribute is omitted here).
///
/// Counts of zero render nothing but stay well-formed.
pub(super) fn write_derived_instr_signal_ports(
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
/// Emit a `<PIDControlSystemFunction>` block for an Instrument /
/// `InstrFunction` object.
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
/// (both columns live on `T_Instrument`, attached to `InstrFunction`
/// rows via `subtables_for_item_type`). When either piece is
/// missing the writer falls back to the bare `MeasuredVariable`
/// (or the bare sequence) so the human-readable label is still
/// non-empty whenever any signal is available.
pub(super) fn write_control_system_function(
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
/// Emit a `<PIDSignalConnector>` block for a `SignalRun` object.
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
///   fixture ships an empty `FlowDirection` on every signal
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
pub(super) fn write_signal_connector(
    buf: &mut String,
    obj: &PublishObject,
) -> Result<(), PublishError> {
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
