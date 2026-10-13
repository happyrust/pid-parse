//! Publish Data XML from a [`PidPublishDrawing`]: only what the `.pid` states.
//!
//! The `.pid`-only counterpart of [`super::xml_writer`] (plan
//! `docs/plans/2026-10-10-publish-from-the-pid-alone.md`; on 2026-10-10 the
//! user chose a separate writer that emits nothing the file does not give).
//! An object carries `IObject` and only the interfaces it has values for; a
//! representation its `GraphicOID`; the relationships are the two the items'
//! own rows state -- `DwgRepresentationComposition` (drawing -> representation)
//! and `DrawingItems` (representation -> item) -- plus one
//! `ProcessPointCollection` per derived process point, under the
//! `{prefix}-{UID1}-{UID2}` UIDs `SmartPlant` gives them. Items whose class the
//! file does not decide are left out, and so are their `DrawingItems`. The
//! `<Container>` envelope constants are `SmartPlant`'s, not the file's.
//! With an Oracle `exp` attached, the ports, their compositions, the
//! six connectivity definitions ([`PidPublishDrawing::connectivity_ends`])
//! and the nozzle / item-note relations
//! ([`PidPublishDrawing::exp_relations`]) follow, and so do the off-page
//! `PIDBranchPoint` objects ([`PidPublishDrawing::opc_points`]). A
//! relation is written only between two published objects.
//!
//! [`write_pid_coverage_json`] renders the report that goes beside the two
//! documents (plan P5): what the file named, what was written, what was left
//! out and why.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::Path;

use serde_json::json;

use super::model::{PublishDrawing, PublishError};
use super::pid_load::{
    ConnectivityEnd, ExpRelation, PidBranchPoint, PidOpcPoint, PidPublishDrawing, PidPublishItem,
    PidValueSource,
};
use super::xml_writer::{defuid_prefix, escape_attr, write_meta_envelope};

const SOFTWARE_VERSION: &str = "10.00.31.0023";
const SCHEMA_VERSION: &str = "04.02.17.01";
const TOOL_ID: &str = "SMARTPLANTPID";
const TOOL_SIGNATURE: &str = "AAAD";

/// Render the `_Data.xml` document of a `.pid`-only drawing.
pub fn write_pid_data_xml(drawing: &PidPublishDrawing, plant_name: &str) -> String {
    let mut buf = String::with_capacity(64 * 1024);
    // Writing into a `String` cannot fail.
    let _ = write_document(&mut buf, drawing, plant_name);
    buf
}

/// Render the `_Meta.xml` companion. The container states the drawing UID
/// and name. Version, revision and file objects are not emitted: the `.pid`
/// does not carry their UIDs, `DocVersionDate` or `FilePath`.
pub fn write_pid_meta_xml(
    drawing: &PidPublishDrawing,
    plant_name: &str,
) -> Result<String, PublishError> {
    write_meta_envelope(
        &PublishDrawing::new(drawing.drawing_uid.clone(), drawing.drawing_name.clone()),
        plant_name,
    )
}

fn write_document(
    buf: &mut String,
    drawing: &PidPublishDrawing,
    plant_name: &str,
) -> std::fmt::Result {
    writeln!(buf, r#"<?xml version ="1.0" encoding="UTF-8"?>"#)?;
    writeln!(
        buf,
        r#"<Container CompSchema="PIDComponent" Scope="Data" SoftwareVersion="{SOFTWARE_VERSION}" IsValidated="False" SchemaVersion="{SCHEMA_VERSION}" LoginUser="" LoginPWD="" Plant="{}" Project="" DocUID="{}" DocName="{}" Version="" ToolID="{TOOL_ID}" ToolSignature="{TOOL_SIGNATURE}" SDECIMAL=".">"#,
        escape_attr(plant_name),
        escape_attr(&drawing.drawing_uid),
        escape_attr(&drawing.drawing_name),
    )?;
    writeln!(buf, "   <PIDDrawing>")?;
    writeln!(
        buf,
        r#"      <IObject UID="{}" Name="{}"/>"#,
        escape_attr(&drawing.drawing_uid),
        escape_attr(&drawing.drawing_name),
    )?;
    writeln!(buf, "   </PIDDrawing>")?;

    for item in &drawing.items {
        if let Some(class) = item.class {
            write_item(buf, item, class)?;
        }
    }
    for point in &drawing.process_points {
        writeln!(buf, "   <PIDProcessPoint>")?;
        writeln!(buf, r#"      <IObject UID="{}"/>"#, escape_attr(&point.uid))?;
        writeln!(buf, "   </PIDProcessPoint>")?;
    }
    for point in &drawing.branch_points {
        write_branch_point(buf, point)?;
    }
    for point in &drawing.opc_points {
        write_opc_point(buf, point)?;
    }
    for port in &drawing.ports {
        write_port(buf, port)?;
    }
    let published = published_items(drawing);
    for representation in &drawing.representations {
        writeln!(buf, "   <PIDRepresentation>")?;
        writeln!(
            buf,
            r#"      <IObject UID="{}"/>"#,
            escape_attr(&representation.uid)
        )?;
        writeln!(
            buf,
            r#"      <IDrawingRepresentation GraphicOID="{}"/>"#,
            representation.graphic_oid
        )?;
        writeln!(buf, "   </PIDRepresentation>")?;
    }
    for representation in &drawing.representations {
        write_rel(
            buf,
            "DwgRepresentationComposition",
            &drawing.drawing_uid,
            &representation.uid,
        )?;
        if let Some(item_uid) = representation
            .item_uid
            .as_deref()
            .filter(|uid| published.contains(uid))
        {
            write_rel(buf, "DrawingItems", &representation.uid, item_uid)?;
        }
    }
    for point in &drawing.process_points {
        // UID2 is the pipe run. UID1 is the process point. Measured on
        // all 341 reference `ProcessPointCollection` rows.
        write_rel(
            buf,
            "ProcessPointCollection",
            &point.uid,
            &point.connector_uid,
        )?;
    }
    for point in &drawing.branch_points {
        // UID1 is the representation. UID2 is the branch point. Measured
        // on all 199 reference `DrawingItems` rows whose UID2 ends in `.BPT`.
        write_rel(buf, "DrawingItems", &point.representation_uid, &point.uid)?;
    }
    for point in &drawing.opc_points {
        // UID1 is the representation drawn on this sheet. UID2 is the
        // lesser OPC id. Measured on I井场: the 2 rows the `.pid` does not
        // class, because that class is left empty without an exp.
        write_rel(buf, "DrawingItems", &point.representation_uid, &point.uid)?;
    }
    for port in &drawing.ports {
        let def_uid = if port.class == "PIDSignalPort" {
            "SignalPortComposition"
        } else {
            "PipingPortComposition"
        };
        write_rel(buf, def_uid, &port.uid, &port.parent_uid)?;
    }
    for end in connectivity_rels(drawing) {
        // UID1 is the symbol end (port, branch point or OPC), UID2 the
        // run. Measured on all 738 reference rows of the six definitions.
        write_rel(buf, end.def_uid, &end.symbol_uid, &end.run_uid)?;
    }
    for rel in composition_rels(drawing) {
        // UID1 is the nozzle or the note, UID2 the equipment or the
        // annotated item. Measured on I井场: 7 compositions, 6 item notes.
        write_rel(buf, rel.def_uid, &rel.uid1, &rel.uid2)?;
    }
    writeln!(buf, "  </Container>")
}

/// The connectivity relationships the `_Data.xml` carries: those whose
/// symbol end and run end are both published objects.
fn connectivity_rels(drawing: &PidPublishDrawing) -> Vec<&ConnectivityEnd> {
    if drawing.connectivity_ends.is_empty() {
        return Vec::new();
    }
    let objects = published_objects(drawing);
    drawing
        .connectivity_ends
        .iter()
        .filter(|end| {
            objects.contains(end.symbol_uid.as_str()) && objects.contains(end.run_uid.as_str())
        })
        .collect()
}

/// Every object UID the `_Data.xml` carries besides the drawing: the
/// classified items, the process points, the branch points and the ports.
fn published_objects(drawing: &PidPublishDrawing) -> BTreeSet<&str> {
    let mut objects = published_items(drawing);
    objects.extend(
        drawing
            .process_points
            .iter()
            .map(|point| point.uid.as_str()),
    );
    objects.extend(drawing.branch_points.iter().map(|point| point.uid.as_str()));
    objects.extend(drawing.opc_points.iter().map(|point| point.uid.as_str()));
    objects.extend(drawing.ports.iter().map(|port| port.uid.as_str()));
    objects
}

/// Connectivity relationships written, and the ones held back because an
/// end is not a published object. The same two numbers are
/// `oracle_exp.connectivity_rels` / `oracle_exp.connectivity_ends_left_out`
/// in [`write_pid_coverage_json`].
pub fn connectivity_rel_counts(drawing: &PidPublishDrawing) -> (usize, usize) {
    let written = connectivity_rels(drawing).len();
    (written, drawing.connectivity_ends.len() - written)
}

/// Nozzle and item-note rows kept from the exp, and how many of each
/// become a `<Rel>` because both ends are published objects.
///
/// The four numbers are `(nozzle rows, nozzle rels, note rows, note rels)`.
pub fn composition_rel_counts(drawing: &PidPublishDrawing) -> (usize, usize, usize, usize) {
    let written = composition_rels(drawing);
    let nozzles = drawing
        .exp_relations
        .iter()
        .filter(|rel| rel.def_uid == "EquipmentComponentComposition")
        .count();
    let notes = drawing.exp_relations.len() - nozzles;
    let nozzle_rels = written
        .iter()
        .filter(|rel| rel.def_uid == "EquipmentComponentComposition")
        .count();
    let note_rels = written.len() - nozzle_rels;
    (nozzles, nozzle_rels, notes, note_rels)
}

/// Nozzle compositions and item notes whose ends are both published.
fn composition_rels(drawing: &PidPublishDrawing) -> Vec<&ExpRelation> {
    if drawing.exp_relations.is_empty() {
        return Vec::new();
    }
    let objects = published_objects(drawing);
    drawing
        .exp_relations
        .iter()
        .filter(|rel| objects.contains(rel.uid1.as_str()) && objects.contains(rel.uid2.as_str()))
        .collect()
}

fn write_branch_point(buf: &mut String, point: &PidBranchPoint) -> std::fmt::Result {
    let interfaces: &[&str] = if point.class == "PIDSignalBranchPoint" {
        &[
            "IConnection",
            "IDrawingItem",
            "ISignalConnection",
            "ISignalBranchPoint",
            "IFacilityPoint",
            "IDocumentItem",
        ]
    } else {
        &[
            "IConnection",
            "IPipingConnection",
            "IDrawingItem",
            "IPipingBranchPoint",
            "IDocumentItem",
        ]
    };
    writeln!(buf, "   <{}>", point.class)?;
    writeln!(buf, r#"      <IObject UID="{}"/>"#, escape_attr(&point.uid))?;
    for interface in interfaces {
        writeln!(buf, "      <{interface}/>")?;
    }
    writeln!(buf, "   </{}>", point.class)
}

fn write_opc_point(buf: &mut String, point: &PidOpcPoint) -> std::fmt::Result {
    writeln!(buf, "   <PIDBranchPoint>")?;
    let mut object = format!(r#"UID="{}""#, escape_attr(&point.uid));
    if !point.name.is_empty() {
        write!(object, r#" Name="{}""#, escape_attr(&point.name))?;
    }
    if !point.description.is_empty() {
        write!(
            object,
            r#" Description="{}""#,
            escape_attr(&point.description)
        )?;
    }
    writeln!(buf, "      <IObject {object}/>")?;
    writeln!(buf, "      <IPIDBranchPoint/>")?;
    if point.to_from_text.is_empty() {
        writeln!(buf, "      <IOffpageConnector/>")?;
    } else {
        writeln!(
            buf,
            r#"      <IOffpageConnector ToFromText="{}"/>"#,
            escape_attr(&point.to_from_text)
        )?;
    }
    for interface in [
        "IDuctConnection",
        "IConnection",
        "IDrawingItem",
        "IPipingConnection",
        "ISignalConnection",
        "IDocumentItem",
    ] {
        writeln!(buf, "      <{interface}/>")?;
    }
    writeln!(buf, "   </PIDBranchPoint>")
}

fn write_port(buf: &mut String, port: &super::pid_load::PidPort) -> std::fmt::Result {
    let interfaces: &[&str] = if port.class == "PIDSignalPort" {
        &[
            "IConnection",
            "ISignalConnection",
            "ISignalPort",
            "IPipingConnection",
            "IFacilityPoint",
            "IPIDTypical",
        ]
    } else {
        &[
            "IConnection",
            "IPipingPort",
            "IPipingConnection",
            "IPort",
            "IPipeCrossSectionItem",
        ]
    };
    writeln!(buf, "   <{}>", port.class)?;
    let name = port
        .attributes
        .iter()
        .find(|attribute| attribute.interface == "IObject" && attribute.name == "Name")
        .map_or("", |attribute| attribute.value.as_str());
    writeln!(
        buf,
        r#"      <IObject UID="{}" Name="{}"/>"#,
        escape_attr(&port.uid),
        escape_attr(name)
    )?;
    for interface in interfaces {
        let mut pairs = String::new();
        for attribute in port
            .attributes
            .iter()
            .filter(|attribute| attribute.interface == *interface)
        {
            write!(
                pairs,
                r#" {}="{}""#,
                attribute.name,
                escape_attr(&attribute.value)
            )?;
        }
        if pairs.is_empty() {
            writeln!(buf, "      <{interface}/>")?;
        } else {
            writeln!(buf, "      <{interface}{pairs}/>")?;
        }
    }
    writeln!(buf, "   </{}>", port.class)
}

fn write_item(buf: &mut String, item: &PidPublishItem, class: &str) -> std::fmt::Result {
    writeln!(buf, "   <{class}>")?;
    let mut object = format!(r#"UID="{}""#, escape_attr(&item.uid));
    for name in ["Name", "Description"] {
        if let Some(attribute) = item
            .attributes
            .iter()
            .find(|attribute| attribute.interface == "IObject" && attribute.name == name)
        {
            write!(object, r#" {name}="{}""#, escape_attr(&attribute.value))?;
        }
    }
    writeln!(buf, "      <IObject {object}/>")?;

    let mut interfaces: Vec<(&str, String)> = Vec::new();
    for attribute in item
        .attributes
        .iter()
        .filter(|attribute| attribute.interface != "IObject")
    {
        let pair = format!(r#" {}="{}""#, attribute.name, escape_attr(&attribute.value));
        match interfaces.last_mut() {
            Some((interface, pairs)) if *interface == attribute.interface => pairs.push_str(&pair),
            _ => interfaces.push((attribute.interface, pair)),
        }
    }
    for (interface, pairs) in interfaces {
        writeln!(buf, "      <{interface}{pairs}/>")?;
    }
    writeln!(buf, "   </{class}>")
}

fn write_rel(buf: &mut String, def_uid: &str, uid1: &str, uid2: &str) -> std::fmt::Result {
    writeln!(buf, "   <Rel>")?;
    writeln!(
        buf,
        r#"      <IObject UID="{}-{}-{}"/>"#,
        defuid_prefix(def_uid),
        escape_attr(uid1),
        escape_attr(uid2)
    )?;
    writeln!(
        buf,
        r#"      <IRel UID1="{}" UID2="{}" DefUID="{def_uid}"/>"#,
        escape_attr(uid1),
        escape_attr(uid2)
    )?;
    writeln!(buf, "   </Rel>")
}

/// The items the `_Data.xml` carries: those whose class the file decides.
fn published_items(drawing: &PidPublishDrawing) -> BTreeSet<&str> {
    drawing
        .items
        .iter()
        .filter(|item| item.class.is_some())
        .map(|item| item.uid.as_str())
        .collect()
}

/// The `DrawingItems` relationships the `_Data.xml` carries: one for every
/// representation whose stated item is published, plus one for every
/// derived branch point.
fn drawing_items(drawing: &PidPublishDrawing, published: &BTreeSet<&str>) -> usize {
    drawing
        .representations
        .iter()
        .filter(|representation| {
            representation
                .item_uid
                .as_deref()
                .is_some_and(|uid| published.contains(uid))
        })
        .count()
        + drawing.branch_points.len()
        + drawing.opc_points.len()
}

/// Published values fixed by an `EFAdapter.dll` rule
/// ([`PidValueSource::DerivedByNativeRule`]). The same number is
/// `attribute_values.derived_by_native_rule` in [`write_pid_coverage_json`]
/// and the `.pid` line on stderr.
pub fn derived_by_native_rule_count(drawing: &PidPublishDrawing) -> usize {
    let attributes = drawing
        .items
        .iter()
        .filter(|item| item.class.is_some())
        .flat_map(|item| &item.attributes)
        .filter(|attribute| matches!(attribute.source, PidValueSource::DerivedByNativeRule { .. }))
        .count();
    // Each process-point UID is one derived value (`0x5F5CFD40`), and
    // each branch-point UID is one (`0x5F5CFC10`).
    attributes + drawing.process_points.len() + drawing.branch_points.len()
}

/// What a `.pid`-only export never carries, for the reader of the report.
const LEFT_OUT_BY_DESIGN: [&str; 5] = [
    "ports: a .pid does not state the point number, so they are not emitted; pipelines are not stated either",
    "off-page connector items: the ModelID on an OPC's row is not always the item SmartPlant publishes",
    "instruments whose graphic has neither a SymbolAttr row nor a symbol filed under an Instrumentation library family: the file does not decide their class, so the item, its DrawingItems and its label values are left out",
    "codelist attributes and concatenated pipeline / component names: they come from the plant database",
    "an attribute two labels on one item disagree about",
];

/// Render the coverage report that goes beside a `.pid`-only export: the
/// drawing, the counts of what the file named and what was written, the
/// per-class tally, the items left out, and what the export never carries.
/// `source` is the `.pid` the report describes.
pub fn write_pid_coverage_json(drawing: &PidPublishDrawing, source: &Path) -> String {
    let published = published_items(drawing);
    let stats = drawing.stats;
    let mut classes: BTreeMap<&str, (usize, usize, usize)> = BTreeMap::new();
    let mut composed_names = 0usize;
    for item in &drawing.items {
        let Some(class) = item.class else {
            continue;
        };
        let cell = classes.entry(class).or_default();
        cell.0 += 1;
        cell.1 += usize::from(
            item.attributes
                .iter()
                .any(|attribute| attribute.interface == "IObject" && attribute.name == "Name"),
        );
        cell.2 += item.attributes.len();
        composed_names += item
            .attributes
            .iter()
            .filter(|attribute| matches!(attribute.source, PidValueSource::NameRule(_)))
            .count();
    }
    if !drawing.process_points.is_empty() {
        classes.insert("PIDProcessPoint", (drawing.process_points.len(), 0, 0));
    }
    for class in ["PIDPipingBranchPoint", "PIDSignalBranchPoint"] {
        let count = drawing
            .branch_points
            .iter()
            .filter(|point| point.class == class)
            .count();
        if count > 0 {
            classes.insert(class, (count, 0, 0));
        }
    }
    for class in ["PIDPipingPort", "PIDSignalPort"] {
        let matched: Vec<_> = drawing
            .ports
            .iter()
            .filter(|port| port.class == class)
            .collect();
        if matched.is_empty() {
            continue;
        }
        let with_name = matched
            .iter()
            .filter(|port| {
                port.attributes
                    .iter()
                    .any(|attribute| attribute.interface == "IObject" && attribute.name == "Name")
            })
            .count();
        let values = matched.iter().map(|port| port.attributes.len()).sum();
        classes.insert(class, (matched.len(), with_name, values));
    }
    let left_out: Vec<serde_json::Value> = drawing
        .items
        .iter()
        .filter(|item| item.class.is_none())
        .map(|item| json!({ "uid": item.uid, "model_item_type": item.model_item_type }))
        .collect();
    let classes: serde_json::Map<String, serde_json::Value> = classes
        .into_iter()
        .map(|(class, (objects, with_name, values))| {
            (
                class.to_string(),
                json!({ "objects": objects, "with_name": with_name, "attribute_values": values }),
            )
        })
        .collect();
    let mut report = json!({
        "source": source.display().to_string(),
        "drawing": { "uid": drawing.drawing_uid, "name": drawing.drawing_name },
        "files": {
            "data": format!("{}_Data.xml", drawing.drawing_name),
            "meta": format!("{}_Meta.xml", drawing.drawing_name),
        },
        "objects": {
            "items_named_by_the_file": drawing.items.len(),
            "items_published": published.len(),
            "items_left_out_without_a_class": left_out.len(),
            "instruments_classed_by_symbol_library": stats.instruments_classed_by_library,
            "representations": drawing.representations.len(),
            "process_points": drawing.process_points.len(),
            "branch_points": drawing.branch_points.len(),
        },
        "relationships": {
            "DwgRepresentationComposition": drawing.representations.len(),
            "DrawingItems": drawing_items(drawing, &published),
            "ProcessPointCollection": drawing.process_points.len(),
        },
        "attribute_values": {
            "label_values": stats.label_values,
            "label_values_reaching_an_item": stats.label_values_reached,
            "published_from_labels": stats.label_values_published,
            "composed_names": composed_names,
            "derived_by_native_rule": derived_by_native_rule_count(drawing),
            "nominal_diameter_not_emitted": stats.nominal_diameter_not_emitted,
            "left_out_on_disagreeing_labels": stats.conflicting_attributes,
        },
        "attribute_rows": {
            "decoded": stats.attribute_records.saturating_sub(stats.undecoded_records),
            "undecoded": stats.undecoded_records,
        },
        "classes": classes,
        "left_out": left_out,
        "left_out_by_design": LEFT_OUT_BY_DESIGN,
    });
    if let Some(exp) = &drawing.exp {
        if let Some(list) = report
            .get_mut("left_out_by_design")
            .and_then(serde_json::Value::as_array_mut)
        {
            list[0] = json!(
                "ports: emitted from the Oracle exp for parents this .pid already publishes; pipelines are still not stated"
            );
        }
        let piping = drawing
            .ports
            .iter()
            .filter(|port| port.class == "PIDPipingPort")
            .count();
        let signal = drawing.ports.len() - piping;
        let backup_rows = drawing
            .ports
            .iter()
            .flat_map(|port| &port.attributes)
            .filter(|attribute| matches!(attribute.source, PidValueSource::BackupRow { .. }))
            .count();
        if let Some(objects) = report
            .get_mut("objects")
            .and_then(|value| value.as_object_mut())
        {
            objects.insert("piping_ports".to_string(), json!(piping));
            objects.insert("signal_ports".to_string(), json!(signal));
        }
        let written = connectivity_rels(drawing);
        let compositions = composition_rels(drawing);
        let mut by_def: BTreeMap<&str, usize> = BTreeMap::new();
        for end in &written {
            *by_def.entry(end.def_uid).or_default() += 1;
        }
        for rel in &compositions {
            *by_def.entry(rel.def_uid).or_default() += 1;
        }
        if let Some(relationships) = report
            .get_mut("relationships")
            .and_then(|value| value.as_object_mut())
        {
            relationships.insert("PipingPortComposition".to_string(), json!(piping));
            relationships.insert("SignalPortComposition".to_string(), json!(signal));
            for (def_uid, count) in &by_def {
                relationships.insert((*def_uid).to_string(), json!(count));
            }
        }
        if let Some(values) = report
            .get_mut("attribute_values")
            .and_then(|value| value.as_object_mut())
        {
            values.insert("backup_rows".to_string(), json!(backup_rows));
        }
        report["oracle_exp"] = json!({
            "dump": exp.dump_path,
            "piping_ports": piping,
            "signal_ports": signal,
            "piping_port_composition": piping,
            "signal_port_composition": signal,
            "backup_row_values": backup_rows,
            "nominal_diameter_unspelled": exp.nominal_diameter_unspelled,
            "connectivity_ends": drawing.connectivity_ends.len(),
            "connectivity_rels": written.len(),
            "connectivity_ends_left_out": drawing.connectivity_ends.len() - written.len(),
            "opc_branch_points": drawing.opc_points.len(),
            "equipment_compositions": drawing
                .exp_relations
                .iter()
                .filter(|rel| rel.def_uid == "EquipmentComponentComposition")
                .count(),
            "equipment_composition_rels": compositions
                .iter()
                .filter(|rel| rel.def_uid == "EquipmentComponentComposition")
                .count(),
            "item_notes": drawing
                .exp_relations
                .iter()
                .filter(|rel| rel.def_uid == "PBSItemNotes")
                .count(),
            "item_note_rels": compositions
                .iter()
                .filter(|rel| rel.def_uid == "PBSItemNotes")
                .count(),
        });
    }
    let mut text = serde_json::to_string_pretty(&report).unwrap_or_default();
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::publish::pid_load::{
        PidBranchPoint, PidProcessPoint, PidPublishAttribute, PidPublishRepresentation,
        PidPublishStats, PidValueSource,
    };

    fn drawing() -> PidPublishDrawing {
        let attribute = |interface, name, value: &str| PidPublishAttribute {
            interface,
            name,
            value: value.to_string(),
            source: PidValueSource::Label { row: 7 },
        };
        PidPublishDrawing {
            drawing_uid: "D1".to_string(),
            drawing_name: "A & B".to_string(),
            items: vec![
                PidPublishItem {
                    uid: "I1".to_string(),
                    model_item_type: "Instrument".to_string(),
                    class: Some("PIDInstrument"),
                    attributes: vec![
                        attribute("INamedInstrument", "InstrTagSequenceNo", "0112101A"),
                        attribute("INamedInstrument", "MeasuredVariable", "TT"),
                        PidPublishAttribute {
                            interface: "IObject",
                            name: "Name",
                            value: "TT-0112101A".to_string(),
                            source: PidValueSource::NameRule("rule"),
                        },
                    ],
                },
                PidPublishItem {
                    uid: "I2".to_string(),
                    model_item_type: "Instrument".to_string(),
                    class: None,
                    attributes: Vec::new(),
                },
            ],
            process_points: Vec::new(),
            branch_points: Vec::new(),
            ports: Vec::new(),
            connectivity_ends: Vec::new(),
            exp_relations: Vec::new(),
            opc_points: Vec::new(),
            exp: None,
            representations: vec![
                PidPublishRepresentation {
                    uid: "R1".to_string(),
                    item_uid: Some("I1".to_string()),
                    graphic_oid: 3810,
                },
                PidPublishRepresentation {
                    uid: "R2".to_string(),
                    item_uid: Some("I2".to_string()),
                    graphic_oid: 42,
                },
                PidPublishRepresentation {
                    uid: "R3".to_string(),
                    item_uid: None,
                    graphic_oid: 43,
                },
            ],
            stats: PidPublishStats::default(),
        }
    }

    #[test]
    fn an_item_carries_only_the_interfaces_it_has_values_for() {
        let xml = write_pid_data_xml(&drawing(), "P01");
        assert!(xml.contains(r#"DocName="A &amp; B""#));
        assert!(xml.contains(
            "   <PIDInstrument>\n      <IObject UID=\"I1\" Name=\"TT-0112101A\"/>\n      <INamedInstrument InstrTagSequenceNo=\"0112101A\" MeasuredVariable=\"TT\"/>\n   </PIDInstrument>\n"
        ));
        assert!(
            !xml.contains("UID=\"I2\""),
            "an unclassified item is left out"
        );
    }

    #[test]
    fn representations_carry_their_graphic_oid_and_the_two_stated_relationships() {
        let xml = write_pid_data_xml(&drawing(), "P01");
        assert!(xml.contains(r#"<IDrawingRepresentation GraphicOID="3810"/>"#));
        assert!(xml.contains(r#"<IObject UID="DRC-D1-R1"/>"#));
        assert!(xml.contains(r#"<IRel UID1="R1" UID2="I1" DefUID="DrawingItems"/>"#));
        assert!(xml.contains(r#"<IObject UID="DRC-D1-R2"/>"#));
        assert!(
            !xml.contains(r#"UID2="I2""#),
            "no DrawingItems to an item that is left out"
        );
        assert!(xml.contains(r#"<IObject UID="DRC-D1-R3"/>"#));
        assert!(
            !xml.contains(r#"UID1="R3" UID2"#),
            "no DrawingItems for a representation whose item is not stated"
        );
        assert!(xml.ends_with("  </Container>\n"));
    }

    #[test]
    fn the_coverage_report_counts_what_is_written_and_names_what_is_left_out() {
        let mut drawing = drawing();
        drawing.stats = PidPublishStats {
            attribute_records: 10,
            undecoded_records: 1,
            label_values: 5,
            label_values_reached: 4,
            label_values_published: 2,
            conflicting_attributes: 1,
            instruments_classed_by_library: 1,
            nominal_diameter_not_emitted: 0,
        };
        let text = write_pid_coverage_json(&drawing, Path::new(r"D:\plant\A & B.pid"));
        let report: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(report["source"], r"D:\plant\A & B.pid");
        assert_eq!(report["drawing"]["uid"], "D1");
        assert_eq!(report["files"]["data"], "A & B_Data.xml");
        assert_eq!(report["objects"]["items_named_by_the_file"], 2);
        assert_eq!(report["objects"]["items_published"], 1);
        assert_eq!(report["objects"]["items_left_out_without_a_class"], 1);
        assert_eq!(
            report["objects"]["instruments_classed_by_symbol_library"],
            1
        );
        assert_eq!(report["objects"]["representations"], 3);
        assert_eq!(report["relationships"]["DwgRepresentationComposition"], 3);
        assert_eq!(report["relationships"]["DrawingItems"], 1);
        assert_eq!(report["attribute_values"]["published_from_labels"], 2);
        assert_eq!(report["attribute_values"]["composed_names"], 1);
        assert_eq!(report["attribute_values"]["derived_by_native_rule"], 0);
        assert_eq!(
            report["attribute_values"]["nominal_diameter_not_emitted"],
            0
        );
        assert_eq!(
            report["attribute_values"]["left_out_on_disagreeing_labels"],
            1
        );
        assert_eq!(report["attribute_rows"]["decoded"], 9);
        assert_eq!(report["classes"]["PIDInstrument"]["objects"], 1);
        assert_eq!(report["classes"]["PIDInstrument"]["with_name"], 1);
        assert_eq!(report["classes"]["PIDInstrument"]["attribute_values"], 3);
        assert_eq!(report["left_out"][0]["uid"], "I2");
        assert_eq!(report["left_out"][0]["model_item_type"], "Instrument");
        assert_eq!(
            report["left_out_by_design"].as_array().map(Vec::len),
            Some(LEFT_OUT_BY_DESIGN.len())
        );
        assert!(text.ends_with("}\n"));
    }

    #[test]
    fn a_native_rule_value_is_counted_apart_from_labels_and_name_rules() {
        let mut drawing = drawing();
        drawing.items[0].attributes.push(PidPublishAttribute {
            interface: "IObject",
            name: "Description",
            value: "derived".to_string(),
            source: PidValueSource::DerivedByNativeRule {
                analysis_page: "docs/analysis/n2a-port-relation-filter.md",
                address: "0x5F5CEF20",
            },
        });
        drawing.items[1].attributes.push(PidPublishAttribute {
            interface: "IObject",
            name: "Description",
            value: "not published".to_string(),
            source: PidValueSource::DerivedByNativeRule {
                analysis_page: "docs/analysis/n2a-port-relation-filter.md",
                address: "0x5F5CEF20",
            },
        });
        let text = write_pid_coverage_json(&drawing, Path::new("a.pid"));
        let report: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(report["attribute_values"]["derived_by_native_rule"], 1);
        assert_eq!(report["attribute_values"]["composed_names"], 1);
        assert_eq!(derived_by_native_rule_count(&drawing), 1);
    }

    #[test]
    fn a_pid_meta_xml_states_the_drawing_and_does_not_invent_version_objects() {
        let text = write_pid_meta_xml(&drawing(), "P01").expect("meta");
        assert!(text.contains(r#"DocUID="D1""#), "{text}");
        assert!(text.contains(r#"DocName="A &amp; B""#), "{text}");
        assert!(!text.contains("DocumentVersion"), "{text}");
        assert!(!text.contains("DocumentRevision"), "{text}");
        assert!(!text.contains("<File"), "{text}");
        assert!(!text.contains("DocVersionDate"), "{text}");
        assert!(!text.contains("FilePath"), "{text}");
        assert!(!text.contains("<Rel"), "{text}");
    }

    #[test]
    fn a_process_point_is_the_connector_uid_plus_ppt_and_one_collection() {
        let mut drawing = drawing();
        drawing.process_points.push(PidProcessPoint {
            uid: "RUN.PPT".to_string(),
            connector_uid: "RUN".to_string(),
            source: PidValueSource::DerivedByNativeRule {
                analysis_page: "docs/analysis/n2a-port-relation-filter.md",
                address: "0x5F5CFD40",
            },
        });
        let xml = write_pid_data_xml(&drawing, "P01");
        assert!(xml.contains(
            "   <PIDProcessPoint>\n      <IObject UID=\"RUN.PPT\"/>\n   </PIDProcessPoint>\n"
        ));
        assert!(xml.contains(
            "   <Rel>\n      <IObject UID=\"PRP-RUN.PPT-RUN\"/>\n      <IRel UID1=\"RUN.PPT\" UID2=\"RUN\" DefUID=\"ProcessPointCollection\"/>\n   </Rel>\n"
        ));
        let text = write_pid_coverage_json(&drawing, Path::new("a.pid"));
        let report: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(report["objects"]["process_points"], 1);
        assert_eq!(report["relationships"]["ProcessPointCollection"], 1);
        assert_eq!(report["classes"]["PIDProcessPoint"]["objects"], 1);
        assert_eq!(report["attribute_values"]["derived_by_native_rule"], 1);
    }

    #[test]
    fn a_branch_point_is_the_representation_uid_plus_bpt() {
        let mut drawing = drawing();
        drawing.branch_points.push(PidBranchPoint {
            uid: "REP.BPT".to_string(),
            representation_uid: "REP".to_string(),
            class: "PIDPipingBranchPoint",
            source: PidValueSource::DerivedByNativeRule {
                analysis_page: "docs/analysis/n2a-port-relation-filter.md",
                address: "0x5F5CFC10",
            },
        });
        drawing.branch_points.push(PidBranchPoint {
            uid: "SIG.BPT".to_string(),
            representation_uid: "SIG".to_string(),
            class: "PIDSignalBranchPoint",
            source: PidValueSource::DerivedByNativeRule {
                analysis_page: "docs/analysis/n2a-port-relation-filter.md",
                address: "0x5F5CFC10",
            },
        });
        let xml = write_pid_data_xml(&drawing, "P01");
        assert!(xml.contains(
            "   <PIDPipingBranchPoint>\n      <IObject UID=\"REP.BPT\"/>\n      <IConnection/>\n      <IPipingConnection/>\n      <IDrawingItem/>\n      <IPipingBranchPoint/>\n      <IDocumentItem/>\n   </PIDPipingBranchPoint>\n"
        ));
        assert!(xml.contains(
            "   <PIDSignalBranchPoint>\n      <IObject UID=\"SIG.BPT\"/>\n      <IConnection/>\n      <IDrawingItem/>\n      <ISignalConnection/>\n      <ISignalBranchPoint/>\n      <IFacilityPoint/>\n      <IDocumentItem/>\n   </PIDSignalBranchPoint>\n"
        ));
        assert!(xml.contains(
            "   <Rel>\n      <IObject UID=\"DRI-REP-REP.BPT\"/>\n      <IRel UID1=\"REP\" UID2=\"REP.BPT\" DefUID=\"DrawingItems\"/>\n   </Rel>\n"
        ));
        let text = write_pid_coverage_json(&drawing, Path::new("a.pid"));
        let report: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(report["objects"]["branch_points"], 2);
        assert_eq!(report["classes"]["PIDPipingBranchPoint"]["objects"], 1);
        assert_eq!(report["classes"]["PIDSignalBranchPoint"]["objects"], 1);
        assert_eq!(report["attribute_values"]["derived_by_native_rule"], 2);
    }

    #[test]
    fn a_connectivity_rel_is_written_only_between_two_published_objects() {
        use crate::publish::pid_load::{ConnectivityEnd, ExpPublish, PidPort};
        let mut drawing = drawing();
        drawing.items.push(PidPublishItem {
            uid: "RUN".to_string(),
            model_item_type: "PipeRun".to_string(),
            class: Some("PIDPipingConnector"),
            attributes: Vec::new(),
        });
        drawing.ports.push(PidPort {
            uid: "I1.1".to_string(),
            parent_uid: "I1".to_string(),
            class: "PIDPipingPort",
            attributes: Vec::new(),
        });
        drawing.exp = Some(ExpPublish {
            dump_path: "Export.dmp".to_string(),
            nominal_diameter_unspelled: 0,
        });
        let end = |symbol: &str, run: &str, def_uid| ConnectivityEnd {
            symbol_uid: symbol.to_string(),
            run_uid: run.to_string(),
            on_start: true,
            on_end: false,
            def_uid,
        };
        drawing.connectivity_ends = vec![
            end("I1.1", "RUN", "PipingEnd1Conn"),
            // The symbol end is not a published object: held back.
            end("OPC", "RUN", "PipingEnd2Conn"),
            // The run is not a published object: held back.
            end("I1.1", "I2", "PipingTapOrFitting"),
        ];
        let xml = write_pid_data_xml(&drawing, "P01");
        assert!(xml.contains(
            "   <Rel>\n      <IObject UID=\"PE1-I1.1-RUN\"/>\n      <IRel UID1=\"I1.1\" UID2=\"RUN\" DefUID=\"PipingEnd1Conn\"/>\n   </Rel>\n"
        ));
        assert!(!xml.contains("UID1=\"OPC\""), "{xml}");
        assert!(!xml.contains("DefUID=\"PipingTapOrFitting\""), "{xml}");
        assert_eq!(connectivity_rel_counts(&drawing), (1, 2));
        let text = write_pid_coverage_json(&drawing, Path::new("a.pid"));
        let report: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(report["relationships"]["PipingEnd1Conn"], 1);
        assert!(report["relationships"].get("PipingEnd2Conn").is_none());
        assert_eq!(report["oracle_exp"]["connectivity_ends"], 3);
        assert_eq!(report["oracle_exp"]["connectivity_rels"], 1);
        assert_eq!(report["oracle_exp"]["connectivity_ends_left_out"], 2);
    }

    #[test]
    fn a_composition_rel_is_written_only_between_two_published_objects() {
        use crate::publish::pid_load::{ExpPublish, ExpRelation};
        let mut drawing = drawing();
        drawing.items.push(PidPublishItem {
            uid: "N1".to_string(),
            model_item_type: "Nozzle".to_string(),
            class: Some("PIDNozzle"),
            attributes: Vec::new(),
        });
        drawing.items.push(PidPublishItem {
            uid: "NOTE".to_string(),
            model_item_type: "ItemNote".to_string(),
            class: Some("PIDNote"),
            attributes: Vec::new(),
        });
        drawing.exp = Some(ExpPublish {
            dump_path: "Export.dmp".to_string(),
            nominal_diameter_unspelled: 0,
        });
        drawing.exp_relations = vec![
            ExpRelation {
                def_uid: "EquipmentComponentComposition",
                uid1: "N1".to_string(),
                uid2: "I1".to_string(),
            },
            // Equipment is not a published object: held back.
            ExpRelation {
                def_uid: "EquipmentComponentComposition",
                uid1: "N2".to_string(),
                uid2: "MISSING".to_string(),
            },
            ExpRelation {
                def_uid: "PBSItemNotes",
                uid1: "NOTE".to_string(),
                uid2: "I1".to_string(),
            },
        ];
        let xml = write_pid_data_xml(&drawing, "P01");
        assert!(xml.contains(
            "   <Rel>\n      <IObject UID=\"EQC-N1-I1\"/>\n      <IRel UID1=\"N1\" UID2=\"I1\" DefUID=\"EquipmentComponentComposition\"/>\n   </Rel>\n"
        ));
        assert!(xml.contains(
            "   <Rel>\n      <IObject UID=\"ITN-NOTE-I1\"/>\n      <IRel UID1=\"NOTE\" UID2=\"I1\" DefUID=\"PBSItemNotes\"/>\n   </Rel>\n"
        ));
        assert!(!xml.contains("MISSING"), "{xml}");
        assert_eq!(composition_rel_counts(&drawing), (2, 1, 1, 1));
        let text = write_pid_coverage_json(&drawing, Path::new("a.pid"));
        let report: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(report["relationships"]["EquipmentComponentComposition"], 1);
        assert_eq!(report["relationships"]["PBSItemNotes"], 1);
        assert_eq!(report["oracle_exp"]["equipment_compositions"], 2);
        assert_eq!(report["oracle_exp"]["equipment_composition_rels"], 1);
        assert_eq!(report["oracle_exp"]["item_notes"], 1);
        assert_eq!(report["oracle_exp"]["item_note_rels"], 1);
    }

    #[test]
    fn an_opc_point_is_published_with_its_drawing_item_and_connectivity() {
        use crate::publish::pid_load::{ConnectivityEnd, PidOpcPoint};
        let mut drawing = drawing();
        drawing.items.push(PidPublishItem {
            uid: "RUN".to_string(),
            model_item_type: "PipeRun".to_string(),
            class: Some("PIDPipingConnector"),
            attributes: Vec::new(),
        });
        drawing.opc_points.push(PidOpcPoint {
            uid: "OPC".to_string(),
            representation_uid: "R1".to_string(),
            name: "565".to_string(),
            description: "去G井场".to_string(),
            to_from_text: "DWG-1".to_string(),
        });
        drawing.connectivity_ends.push(ConnectivityEnd {
            symbol_uid: "OPC".to_string(),
            run_uid: "RUN".to_string(),
            on_start: false,
            on_end: true,
            def_uid: "PipingEnd2Conn",
        });
        let xml = write_pid_data_xml(&drawing, "P01");
        assert!(xml.contains(r#"      <IObject UID="OPC" Name="565" Description="去G井场"/>"#));
        assert!(xml.contains(r#"      <IOffpageConnector ToFromText="DWG-1"/>"#));
        assert!(xml.contains(r#"      <IRel UID1="R1" UID2="OPC" DefUID="DrawingItems"/>"#));
        assert!(xml.contains(r#"      <IRel UID1="OPC" UID2="RUN" DefUID="PipingEnd2Conn"/>"#));
        assert_eq!(connectivity_rel_counts(&drawing), (1, 0));
    }
}
