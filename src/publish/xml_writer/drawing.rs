//! Data-document orchestration, drawing node, object dispatch, and representations.

use std::fmt::Write;

use super::super::catalog::{self, PublishItemKind};
use super::super::model::{PublishDrawing, PublishError, PublishObject, PublishStyle};
use super::common::{
    a01_object_rank, escape_attr, fmt_err, ordered_publishable_representations,
    CONTAINER_SCHEMA_VERSION, CONTAINER_SCOPE, CONTAINER_SDECIMAL, CONTAINER_SOFTWARE_VERSION,
    CONTAINER_TOOL_ID, CONTAINER_TOOL_SIGNATURE,
};
use super::components_notes_branch::{
    write_generic_object, write_item_note, write_pid_branch_point, write_piping_branch_point,
    write_piping_component,
};
use super::instrument_signal::{
    write_control_system_function, write_derived_instr_signal_ports, write_signal_connector,
    INSTR_DERIVED_SIGNAL_PORT_COUNT,
};
use super::pipeline::{
    write_derived_connector_endpoints, write_pipeline, write_piping_connector, write_piping_port,
};
use super::relationships::write_relationships;
use super::vessel_nozzle::{write_nozzle, write_process_vessel};

/// Software-version / schema-version / tooling constants that the
/// `SmartPlant` reference implementation stamps onto every Publish
/// Data `<Container>`. Hard-coded here because they are not
/// carried by any backup table — the values are part of `SmartPlant`
/// 2014 R1's output contract.
const CONTAINER_COMP_SCHEMA: &str = "PIDComponent";
/// Emit a full `_Data.xml` document for the given drawing into a
/// string buffer. `plant_name` is a user-supplied value (e.g. the
/// `SmartPlant` plant identifier from MSCI / Manifest); stage-1
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
/// Emit every `PublishObject` as the corresponding `SmartPlant` XML
/// tag (`<PIDProcessVessel>` / `<PIDNozzle>` / ...). Stage-1
/// handles the four item types the TEST02 A01 fixture exercises
/// (Vessel, Nozzle, `PipeRun`, `PipingPoint`); A11 extends the
/// dispatcher to Note / `ItemNote` (→ `<PIDNote>`) and Instrument /
/// `InstrFunction` (→ `<PIDControlSystemFunction>`), modeled on the
/// DWG-0202GP06-01 reference fixture. Unknown types still fall
/// through with a generic `<PIDItem>` wrapper so the writer stays
/// total.
fn write_business_objects(buf: &mut String, drawing: &PublishDrawing) -> Result<(), PublishError> {
    for obj in ordered_business_objects(drawing) {
        let Some(spec) = catalog::item_spec(&obj.item_type_name) else {
            write_generic_object(buf, obj, &obj.item_type_name)?;
            continue;
        };
        match spec.kind {
            PublishItemKind::Vessel => write_process_vessel(buf, obj, drawing)?,
            PublishItemKind::Nozzle => write_nozzle(buf, obj, drawing)?,
            // PipeRun maps to the logical pipeline + its physical
            // connector + the connector's two derived piping ports
            // and one derived process point. SmartPlant's exporter
            // emits all five tags from a single PipeRun row in this
            // exact order; we mirror that to stay compatible with
            // the SemanticDiffReport contract.
            PublishItemKind::PipeRun => {
                write_pipeline(buf, obj, drawing.style)?;
                write_piping_connector(buf, obj, drawing.style)?;
                write_derived_connector_endpoints(buf, obj)?;
            }
            // PipingPoint as a top-level object is now treated as a
            // generic placeholder. T_PipingPoint rows never reach
            // here (the loader stopped injecting them in A13); this
            // arm exists for forward-compat in case a future fixture
            // surfaces a true standalone PIDPipingPort.
            PublishItemKind::PipingPoint => write_piping_port(buf, obj)?,
            // A11: Note / ItemNote → `<PIDNote>`. The reference
            // DWG fixture ships 11 of these — annotation labels
            // hung on the drawing canvas.
            PublishItemKind::Note => write_item_note(buf, obj)?,
            // A11+A16: Instrument / InstrFunction →
            // `<PIDControlSystemFunction>` plus the eight derived
            // `<PIDSignalPort>` children SmartPlant always
            // synthesizes (UIDs `<instr>.{1..8}`). The DWG fixture
            // ships 2 InstrFunction rows × 8 derived ports = 16
            // PIDSignalPort entries, matching the A15 backlog
            // count exactly.
            PublishItemKind::Instrument => {
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
            PublishItemKind::PipingComponent => write_piping_component(buf, obj)?,
            // A18: SignalRun → `<PIDSignalConnector>`. The
            // signal-side counterpart of PipeRun → PIDPipingConnector.
            // The DWG-0202GP06-01 fixture ships 1 SignalRun row
            // whose XML shape is deliberately minimal (8
            // interfaces, no IPBSItem business envelope) because
            // SmartPlant treats signal connectors as pure wiring
            // overlays rather than piped facilities.
            PublishItemKind::SignalRun => write_signal_connector(buf, obj)?,
            PublishItemKind::PipingBranchPoint => write_piping_branch_point(buf, obj)?,
            PublishItemKind::BranchPoint => write_pid_branch_point(buf, obj)?,
            // TODO(A12+): Exchanger / Mechanical have business
            // subtables registered in `subtables_for_item_type` but
            // no dedicated SmartPlant tag observed in the TEST02 +
            // DWG-0202GP06-01 reference fixtures. They fall through
            // to the generic placeholder until a fixture surfaces
            // their canonical XML shape.
            PublishItemKind::Exchanger | PublishItemKind::Mechanical => {
                write_generic_object(buf, obj, &obj.item_type_name)?;
            }
        }
    }
    Ok(())
}
fn ordered_business_objects(drawing: &PublishDrawing) -> Vec<&PublishObject> {
    let mut objects: Vec<&PublishObject> = drawing.objects.iter().collect();
    if matches!(drawing.style, PublishStyle::A01) {
        objects.sort_by_key(|obj| a01_object_rank(obj.item_type_name.as_str()));
    }
    objects
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
