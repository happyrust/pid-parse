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

use super::model::{PublishDrawing, PublishError, PublishObject, PublishRelationship};

/// Software-version / schema-version / tooling constants that the
/// SmartPlant reference implementation stamps onto every Publish
/// Data `<Container>`. Hard-coded here because they are not
/// carried by any backup table — the values are part of SmartPlant
/// 2014 R1's output contract.
const CONTAINER_COMP_SCHEMA: &str = "PIDComponent";
const CONTAINER_SCOPE: &str = "Data";
const CONTAINER_SOFTWARE_VERSION: &str = "10.00.31.0023";
const CONTAINER_SCHEMA_VERSION: &str = "04.02.17.01";
const CONTAINER_TOOL_ID: &str = "SMARTPLANTPID";
const CONTAINER_TOOL_SIGNATURE: &str = "AAAD";
const CONTAINER_SDECIMAL: &str = ".";

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

/// Emit every `PublishObject` as the corresponding SmartPlant XML
/// tag (`<PIDProcessVessel>` / `<PIDNozzle>` / ...). Stage-1
/// handles the four item types the TEST02 A01 fixture exercises
/// (Vessel, Nozzle, PipeRun, PipingPoint); unknown types fall
/// through with a generic `<PIDItem>` wrapper so the writer stays
/// total.
fn write_business_objects(buf: &mut String, drawing: &PublishDrawing) -> Result<(), PublishError> {
    for obj in &drawing.objects {
        match obj.item_type_name.as_str() {
            "Vessel" => write_process_vessel(buf, obj)?,
            "Nozzle" => write_nozzle(buf, obj)?,
            // PipeRun maps to the logical pipeline + its physical
            // connector. We emit both tags from the same PipeRun
            // row so the resulting XML mirrors SmartPlant's dual
            // representation.
            "PipeRun" => {
                write_pipeline(buf, obj)?;
                write_piping_connector(buf, obj)?;
            }
            // Drawing-scoped PipingPoint rows do not show up in
            // stage-1's object list (they live via T_PipingPoint
            // and will layer in once we load that subtable). Keep
            // the arm so future expansion is a one-line addition.
            "PipingPoint" => write_piping_port(buf, obj)?,
            other => write_generic_object(buf, obj, other)?,
        }
    }
    Ok(())
}

fn write_process_vessel(buf: &mut String, obj: &PublishObject) -> Result<(), PublishError> {
    let item_tag = obj
        .fields
        .get("ItemTag")
        .cloned()
        .unwrap_or_else(|| format_equipment_tag(obj));
    let description = obj.description.as_deref().unwrap_or("");
    let eq_type_description = obj.fields.get("EquipmentType").cloned().unwrap_or_default();
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

fn write_nozzle(buf: &mut String, obj: &PublishObject) -> Result<(), PublishError> {
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
        r#"      <IEquipmentComponent ProcEqpCompTypeDescription="Flanged Nozzle"/>"#
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

fn write_pipeline(buf: &mut String, obj: &PublishObject) -> Result<(), PublishError> {
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
    // Compose an `ItemTag` resembling SmartPlant's line number
    // convention when we have enough signal; otherwise just pass
    // the UID through.
    let item_tag = if !tag_sequence.is_empty() {
        format!(
            "PH-{tag_sequence}-{nominal_diameter}-{piping_materials_class}"
        )
    } else {
        obj.uid.clone()
    };
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
    let item_tag = if !tag_sequence.is_empty() {
        format!("PH-{tag_sequence}-{nominal_diameter}-{piping_materials_class}")
    } else {
        obj.uid.clone()
    };
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
    for rep in &drawing.representations {
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
    for rep in &drawing.representations {
        let Some(model_item_uid) = rep.model_item_uid.as_deref() else {
            continue;
        };
        if model_item_uid.is_empty() {
            continue;
        }
        write_rel(
            buf,
            &format!("DRC-{}-{}", model_item_uid, rep.uid),
            model_item_uid,
            &rep.uid,
            "DwgRepresentationComposition",
        )?;
    }

    // --- Derived: Drawing → Representation (DrawingItems)
    for rep in &drawing.representations {
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
    fn empty_drawing_still_produces_well_formed_xml() {
        let d = PublishDrawing::new("UID-EMPTY", "NoName");
        let out = write_data_xml(&d, "Plant1").expect("write");
        assert!(out.contains("<PIDDrawing>"));
        assert!(out.contains("</PIDDrawing>"));
        // No representations or rels — but the container still
        // closes and the document is valid.
        assert!(out.trim_end().ends_with("</Container>"));
    }
}
