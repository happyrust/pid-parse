//! Publish Data XML from a [`PidPublishDrawing`]: only what the `.pid` states.
//!
//! The `.pid`-only counterpart of [`super::xml_writer`] (plan
//! `docs/plans/2026-10-10-publish-from-the-pid-alone.md`; on 2026-10-10 the
//! user chose a separate writer that emits nothing the file does not give).
//! An object carries the interface elements its class's row of
//! `catalog::CLASS_INTERFACES` lists, in that order: the ones every
//! reference object of the class carries, bare when the file gives them no
//! value, and the others only with a value. A representation carries its
//! `GraphicOID`; the relationships are the two the items'
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

use super::catalog::{self, AttributeRule, InterfaceRule, PidTagRole};
use super::model::{PublishDrawing, PublishError};
use super::pid_load::{
    ConnectivityEnd, ExpRelation, PidBranchPoint, PidOpcPoint, PidPipeline, PidPublishDrawing,
    PidPublishItem, PidValueSource,
};
#[cfg(test)]
use super::xml_writer::derive_meta_uid;
use super::xml_writer::{defuid_prefix, escape_attr, write_data_container_open, write_meta_xml};

/// Render the `_Data.xml` document of a `.pid`-only drawing.
pub fn write_pid_data_xml(drawing: &PidPublishDrawing, plant_name: &str) -> String {
    let mut buf = String::with_capacity(64 * 1024);
    // Writing into a `String` cannot fail.
    let _ = write_document(&mut buf, drawing, plant_name);
    buf
}

/// Render the `_Meta.xml` companion via [`write_meta_xml`].
///
/// Version, revision, file and the three rels use `derive_meta_uid` from
/// the drawing UID. `DocVersionDate` and `FilePath` stay empty: the `.pid`
/// does not state them, and these UIDs are not the ones in a `SmartPlant`
/// reference export.
pub fn write_pid_meta_xml(
    drawing: &PidPublishDrawing,
    plant_name: &str,
) -> Result<String, PublishError> {
    write_meta_xml(
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
    write_data_container_open(buf, plant_name, &drawing.drawing_uid, &drawing.drawing_name)?;
    write_object(
        buf,
        "PIDDrawing",
        &[
            Stated::new("IObject", "UID", &drawing.drawing_uid),
            Stated::new("IObject", "Name", &drawing.drawing_name),
        ],
    )?;

    for item in &drawing.items {
        if let Some(class) = item.class {
            write_item(buf, item, class)?;
        }
    }
    for point in &drawing.process_points {
        write_object(
            buf,
            "PIDProcessPoint",
            &[Stated::new("IObject", "UID", &point.uid)],
        )?;
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
    for pipeline in &drawing.pipelines {
        write_pipeline(buf, pipeline)?;
    }
    let published = published_items(drawing);
    for representation in &drawing.representations {
        write_object(
            buf,
            "PIDRepresentation",
            &[
                Stated::new("IObject", "UID", &representation.uid),
                Stated::owned(
                    "IDrawingRepresentation",
                    "GraphicOID",
                    representation.graphic_oid.to_string(),
                ),
            ],
        )?;
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
    for pipeline in &drawing.pipelines {
        for connector in &pipeline.connector_uids {
            write_rel(buf, "PipingConnectors", connector, &pipeline.uid)?;
        }
    }
    writeln!(buf, "  </Container>")
}

fn write_pipeline(buf: &mut String, pipeline: &PidPipeline) -> std::fmt::Result {
    let mut stated = vec![
        Stated::new("IObject", "UID", &pipeline.uid),
        Stated::new("IObject", "Name", &pipeline.name),
    ];
    if let Some(code) = &pipeline.fluid_code {
        stated.push(Stated::new("IFluidSystem", "FluidCode", code));
    }
    if let Some(fluid) = &pipeline.fluid_system {
        stated.push(Stated::new("IFluidSystem", "FluidSystem", fluid));
    }
    write_object(buf, "PIDPipeline", &stated)
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
    write_object(
        buf,
        point.class,
        &[Stated::new("IObject", "UID", &point.uid)],
    )
}

fn write_opc_point(buf: &mut String, point: &PidOpcPoint) -> std::fmt::Result {
    let mut stated = vec![Stated::new("IObject", "UID", &point.uid)];
    if !point.name.is_empty() {
        stated.push(Stated::new("IObject", "Name", &point.name));
    }
    if !point.description.is_empty() {
        stated.push(Stated::new("IObject", "Description", &point.description));
    }
    // The reference states `IOffpageConnector` only with a `ToFromText`:
    // of the 142 off-page connectors in the thirteen reference drawings,
    // 123 carry one and the other 19 leave the element out; none writes
    // it empty. The 19 are on two drawings, 14 on one and 5 on the other.
    stated.push(Stated::new(
        "IOffpageConnector",
        "ToFromText",
        &point.to_from_text,
    ));
    write_object(buf, "PIDBranchPoint", &stated)
}

fn write_port(buf: &mut String, port: &super::pid_load::PidPort) -> std::fmt::Result {
    let name = port
        .attributes
        .iter()
        .find(|attribute| attribute.interface == "IObject" && attribute.name == "Name")
        .map_or("", |attribute| attribute.value.as_str());
    let mut stated = vec![
        Stated::new("IObject", "UID", &port.uid),
        Stated::new("IObject", "Name", name),
    ];
    stated.extend(
        port.attributes
            .iter()
            .filter(|attribute| attribute.interface != "IObject")
            .map(|attribute| Stated::new(attribute.interface, attribute.name, &attribute.value)),
    );
    write_object(buf, port.class, &stated)
}

fn write_item(buf: &mut String, item: &PidPublishItem, class: &str) -> std::fmt::Result {
    let mut stated = vec![Stated::new("IObject", "UID", &item.uid)];
    stated.extend(
        item.attributes
            .iter()
            .map(|attribute| Stated::new(attribute.interface, attribute.name, &attribute.value)),
    );
    write_object(buf, class, &stated)
}

/// One value an object states: its interface, attribute and value.
struct Stated<'a> {
    interface: &'a str,
    name: &'a str,
    value: std::borrow::Cow<'a, str>,
}

impl<'a> Stated<'a> {
    fn new(interface: &'a str, name: &'a str, value: &'a str) -> Self {
        Self {
            interface,
            name,
            value: value.into(),
        }
    }

    fn owned(interface: &'a str, name: &'a str, value: String) -> Self {
        Self {
            interface,
            name,
            value: value.into(),
        }
    }
}

/// Write one object block: the interface elements `catalog::class_interfaces`
/// lists for `class` in their order, each with the values `stated` gives it.
///
/// An element the class's row does not list, or an object of a class with no
/// row, is written after the listed ones with its values in the order
/// `stated` gives them, so no stated value is dropped.
fn write_object(buf: &mut String, class: &str, stated: &[Stated<'_>]) -> std::fmt::Result {
    writeln!(buf, "   <{class}>")?;
    let slots = catalog::class_interfaces(class).unwrap_or_default();
    for slot in slots {
        let values: Vec<&Stated<'_>> = stated
            .iter()
            .filter(|value| value.interface == slot.interface)
            .collect();
        let written = match slot.rule {
            InterfaceRule::Always => true,
            InterfaceRule::IfValue | InterfaceRule::ByEquipmentType(_) => {
                values.iter().any(|value| !value.value.is_empty())
            }
        };
        if !written {
            continue;
        }
        let mut pairs = String::new();
        for &(name, rule) in slot.attributes {
            match values.iter().find(|value| value.name == name) {
                Some(value) => write_pair(&mut pairs, name, &value.value)?,
                None if rule == AttributeRule::AlwaysEmpty => write_pair(&mut pairs, name, "")?,
                None => {}
            }
        }
        for value in values
            .iter()
            .filter(|value| !slot.attributes.iter().any(|(name, _)| *name == value.name))
        {
            write_pair(&mut pairs, value.name, &value.value)?;
        }
        writeln!(buf, "      <{}{pairs}/>", slot.interface)?;
    }
    let mut unlisted: Vec<(&str, String)> = Vec::new();
    for value in stated
        .iter()
        .filter(|value| !slots.iter().any(|slot| slot.interface == value.interface))
    {
        let index = match unlisted
            .iter()
            .position(|(interface, _)| *interface == value.interface)
        {
            Some(index) => index,
            None => {
                unlisted.push((value.interface, String::new()));
                unlisted.len() - 1
            }
        };
        write_pair(&mut unlisted[index].1, value.name, &value.value)?;
    }
    for (interface, pairs) in unlisted {
        writeln!(buf, "      <{interface}{pairs}/>")?;
    }
    writeln!(buf, "   </{class}>")
}

fn write_pair(pairs: &mut String, name: &str, value: &str) -> std::fmt::Result {
    write!(pairs, r#" {name}="{}""#, escape_attr(value))
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

/// The top-level blocks `write_document` emits for a `Document` /
/// `Enrichment` tag: the drawing itself, its representations, the
/// exp-synthesized pipelines and the OPC branch points.
fn document_block_count(drawing: &PidPublishDrawing, class: &str) -> usize {
    match class {
        "PIDDrawing" => 1,
        "PIDRepresentation" => drawing.representations.len(),
        "PIDPipeline" => drawing.pipelines.len(),
        "PIDBranchPoint" => drawing.opc_points.len(),
        _ => 0,
    }
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
    pid_coverage_json(drawing, source, &drawing.drawing_name)
}

/// [`write_pid_coverage_json`] for files written under `file_stem`, which a
/// backup package derives from the drawing name when that name holds
/// characters a file name cannot.
pub(crate) fn pid_coverage_json(
    drawing: &PidPublishDrawing,
    source: &Path,
    file_stem: &str,
) -> String {
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
    // The classes below are not named here: the catalog says which tags a
    // published collection is tallied under, and its role says which
    // collection that is (see `PidTagRole`).
    for &(class, role) in catalog::pid_line_tag_roles() {
        match role {
            PidTagRole::ProcessPoint => {
                if !drawing.process_points.is_empty() {
                    classes.insert(class, (drawing.process_points.len(), 0, 0));
                }
            }
            PidTagRole::BranchPoint => {
                let count = drawing
                    .branch_points
                    .iter()
                    .filter(|point| point.class == class)
                    .count();
                if count > 0 {
                    classes.insert(class, (count, 0, 0));
                }
            }
            PidTagRole::Port => {
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
                        port.attributes.iter().any(|attribute| {
                            attribute.interface == "IObject" && attribute.name == "Name"
                        })
                    })
                    .count();
                let values = matched.iter().map(|port| port.attributes.len()).sum();
                classes.insert(class, (matched.len(), with_name, values));
            }
            // Item rows are tallied above; the document and enrichment
            // tags count the sibling document's own top-level blocks
            // (the drawing itself, the representations, the synthesized
            // pipelines, the OPC branch points).
            PidTagRole::Class => {}
            PidTagRole::Document | PidTagRole::Enrichment => {
                let count = document_block_count(drawing, class);
                if count > 0 {
                    classes.insert(class, (count, 0, 0));
                }
            }
        }
    }
    let left_out: Vec<serde_json::Value> = drawing
        .items
        .iter()
        .filter(|item| item.class.is_none())
        .map(|item| json!({ "uid": item.uid, "model_item_type": item.model_item_type }))
        .collect();
    let classes_total: usize = classes.values().map(|cell| cell.0).sum();
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
            "data": format!("{file_stem}_Data.xml"),
            "meta": format!("{file_stem}_Meta.xml"),
        },
        "classified_items": {
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
        "classes_total": classes_total,
        "left_out": left_out,
        "left_out_by_design": LEFT_OUT_BY_DESIGN,
    });
    if let Some(exp) = &drawing.exp {
        if let Some(list) = report
            .get_mut("left_out_by_design")
            .and_then(serde_json::Value::as_array_mut)
        {
            list[0] = json!(if drawing.pipelines_attempted {
                "ports: emitted from the Oracle exp for parents this .pid already publishes; pipelines: synthesized from T_PIPERUN by the PipeLinePublisher name rule; the UID is a UUID v5 of the plant and the name"
            } else {
                "ports: emitted from the Oracle exp for parents this .pid already publishes; pipelines are still not stated"
            });
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
            .get_mut("classified_items")
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
            "placed_column_values": drawing
                .items
                .iter()
                .filter(|item| item.class.is_some())
                .flat_map(|item| &item.attributes)
                .filter(|attribute| matches!(attribute.source, PidValueSource::PlacedColumn(_)))
                .count(),
            "nominal_diameter_unspelled": exp.nominal_diameter_unspelled,
            "connectivity_ends": drawing.connectivity_ends.len(),
            "connectivity_rels": written.len(),
            "connectivity_ends_left_out": drawing.connectivity_ends.len() - written.len(),
            "opc_branch_points": drawing.opc_points.len(),
            "dump_equipment_compositions": drawing
                .exp_relations
                .iter()
                .filter(|rel| rel.def_uid == "EquipmentComponentComposition")
                .count(),
            "equipment_composition_rels": compositions
                .iter()
                .filter(|rel| rel.def_uid == "EquipmentComponentComposition")
                .count(),
            "dump_item_notes": drawing
                .exp_relations
                .iter()
                .filter(|rel| rel.def_uid == "PBSItemNotes")
                .count(),
            "item_note_rels": compositions
                .iter()
                .filter(|rel| rel.def_uid == "PBSItemNotes")
                .count(),
            "pipelines": drawing.pipelines.len(),
            "piping_connectors": drawing
                .pipelines
                .iter()
                .map(|pipeline| pipeline.connector_uids.len())
                .sum::<usize>(),
            "fluid_system": drawing
                .pipelines
                .iter()
                .filter(|pipeline| pipeline.fluid_system.is_some())
                .count(),
            "fluid_code_missing": drawing.pipeline_synthesis.fluid_code_missing,
            "fluid_system_unmapped": drawing.pipeline_synthesis.fluid_system_unmapped,
            "without_sequence": drawing.pipeline_synthesis.without_sequence,
            "item_tag_not_a_name": drawing.pipeline_synthesis.item_tag_not_a_name,
        });
    }
    let mut text = serde_json::to_string_pretty(&report).unwrap_or_default();
    text.push('\n');
    text
}

/// Render the coverage report that goes beside a database-line export —
/// the SQL Server MTF package branch. It names the drawing, the
/// `T_Drawing.Template` and `T_Drawing.Path` the row loaded (`null` when
/// the row states neither), the three files this run wrote beside it, and
/// the same object / value / relationship triple the sibling `_Index.json`
/// row carries: top-level blocks that are not `<Rel>`, non-empty attribute
/// values, and `<Rel>` blocks.
///
/// A `.pid`-only export goes through [`write_pid_coverage_json`] instead:
/// a `.pid` file states neither a template nor an archive path, so that
/// report omits both rather than inventing them.
///
/// `file_stem` is the stem the three files were written under; it differs
/// from the drawing name when the row states no name or one that cannot be
/// a file name.
pub fn write_database_coverage_json(
    drawing: &PublishDrawing,
    source: &Path,
    counts: (usize, usize, usize),
    file_stem: &str,
) -> String {
    let (objects, values, relationships) = counts;
    let report = json!({
        "source": source.display().to_string(),
        "drawing": { "uid": drawing.drawing_uid, "name": drawing.drawing_name },
        "template": drawing.template,
        "path": drawing.path,
        "files": {
            "data": format!("{file_stem}_Data.xml"),
            "meta": format!("{file_stem}_Meta.xml"),
            "coverage": format!("{file_stem}_Coverage.json"),
        },
        "document_counts": {
            "objects": objects,
            "values": values,
            "relationships": relationships,
        },
    });
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
            pipelines: Vec::new(),
            pipelines_attempted: false,
            pipeline_synthesis: crate::publish::pid_load::PipelineSynthesis::default(),
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

    /// The interface elements of the block whose `IObject UID` is `uid`, in
    /// document order.
    fn interfaces_of(xml: &str, uid: &str) -> Vec<String> {
        let object = format!("<IObject UID=\"{uid}\"");
        let start = xml.find(&object).expect("the object is written");
        let block = &xml[start..];
        let end = block.find("\n   </").expect("the block ends");
        block[..end]
            .lines()
            .map(|line| {
                let element = line.trim_start().trim_start_matches('<');
                element
                    .split([' ', '/'])
                    .next()
                    .unwrap_or_default()
                    .to_string()
            })
            .collect()
    }

    fn item(
        uid: &str,
        class: &'static str,
        attributes: &[(&'static str, &'static str, &str)],
    ) -> PidPublishItem {
        PidPublishItem {
            uid: uid.to_string(),
            model_item_type: String::new(),
            class: Some(class),
            attributes: attributes
                .iter()
                .map(|&(interface, name, value)| PidPublishAttribute {
                    interface,
                    name,
                    value: value.to_string(),
                    source: PidValueSource::Label { row: 1 },
                })
                .collect(),
        }
    }

    #[test]
    fn an_item_carries_its_classs_interfaces_in_order_bare_where_it_states_nothing() {
        let xml = write_pid_data_xml(&drawing(), "P01");
        assert!(xml.contains(r#"DocName="A &amp; B""#));
        assert!(xml.contains(concat!(
            "   <PIDInstrument>\n",
            "      <IObject UID=\"I1\" Name=\"TT-0112101A\"/>\n",
            "      <IInstrument/>\n",
            "      <IInstrumentOcc/>\n",
            "      <IPBSItem/>\n",
            "      <IPlannedMatl/>\n",
            "      <IDrawingItem/>\n",
            "      <IExpandableThing/>\n",
            "      <IInlineComponentOcc/>\n",
            "      <IPipingPortComposition/>\n",
            "      <IHeatTracedItem HTraceRqmt=\"\"/>\n",
            "      <ILoopMember/>\n",
            "      <IPartOcc/>\n",
            "      <IPressureReliefItem/>\n",
            "      <IProcessPointCollection/>\n",
            "      <ISignalPortComposition/>\n",
            "      <IDocumentItem/>\n",
            "      <IElecPowerConsumer/>\n",
            "      <IPart/>\n",
            "      <INoteCollection/>\n",
            "      <INamedInstrument InstrFuncModifier=\"\" InstrLoopSuffix=\"\" ",
            "InstrTagPrefix=\"\" InstrTagSequenceNo=\"0112101A\" InstrTagSuffix=\"\" ",
            "MeasuredVariable=\"TT\"/>\n",
            "      <IPIDTypical/>\n",
            "   </PIDInstrument>\n",
        )));
        assert!(
            !xml.contains("UID=\"I2\""),
            "an unclassified item is left out"
        );
    }

    #[test]
    fn an_always_interface_is_written_bare_with_its_always_empty_attributes() {
        let mut drawing = drawing();
        drawing.items = vec![item("CSF", "PIDControlSystemFunction", &[])];
        let xml = write_pid_data_xml(&drawing, "P01");
        assert!(xml.contains(
            "      <INamedInstrument InstrFuncModifier=\"\" InstrLoopSuffix=\"\" \
             InstrTagPrefix=\"\" InstrTagSuffix=\"\"/>\n   </PIDControlSystemFunction>\n"
        ));
        assert_eq!(interfaces_of(&xml, "CSF").len(), 12);
    }

    #[test]
    fn an_if_value_interface_is_written_only_where_a_value_is_stated() {
        let mut drawing = drawing();
        drawing.items = vec![
            item(
                "RUN1",
                "PIDPipingConnector",
                &[
                    ("IInsulatedItem", "InsulSpec", "H"),
                    ("INamedPipingConnector", "PipingConnectorSeqNo", "1001"),
                    ("IPipeCrossSectionItem", "NominalDiameter", "50 mm"),
                    ("IPipingSpecifiedItem", "PipingMaterialsClass", "A1"),
                ],
            ),
            item("RUN2", "PIDPipingConnector", &[]),
            item(
                "RUN3",
                "PIDPipingConnector",
                &[("INamedPipingConnector", "PipingConnectorSeqNo", "")],
            ),
        ];
        let xml = write_pid_data_xml(&drawing, "P01");
        let common = [
            "IObject",
            "IPBSItem",
            "IPlannedFacility",
            "IConnector",
            "IDrawingItem",
            "IPBSItemCollection",
            "IPipingConnector",
            "IFabricatedItem",
            "IHeatTracedItem",
            "IProcessPointCollection",
            "IDocumentItem",
            "IElecPowerConsumer",
            "INoteCollection",
            "IProcessDataCaseComposition",
            "IExpandableThing",
        ];
        let mut with_values: Vec<&str> = common.to_vec();
        with_values.extend([
            "INamedPipingConnector",
            "IPipeCrossSectionItem",
            "IPipingSpecifiedItem",
            "IInsulatedItem",
            "IJacketedItem",
            "IPIDTypical",
        ]);
        assert_eq!(interfaces_of(&xml, "RUN1"), with_values);
        let mut bare: Vec<&str> = common.to_vec();
        bare.extend(["IPipeCrossSectionItem", "IJacketedItem", "IPIDTypical"]);
        assert_eq!(interfaces_of(&xml, "RUN2"), bare);
        assert_eq!(
            interfaces_of(&xml, "RUN3"),
            bare,
            "an empty value is no value"
        );
        assert!(xml.contains(
            r#"      <INamedPipingConnector PipingConnectorPrefix="" PipingConnectorSeqNo="1001" PipingConnectorSuff=""/>"#
        ));
    }

    #[test]
    fn an_interface_the_equipment_type_decides_is_left_out() {
        let mut drawing = drawing();
        drawing.items = vec![
            item("V1", "PIDProcessVessel", &[]),
            item("E1", "PIDHeatTransferEquipment", &[]),
        ];
        let xml = write_pid_data_xml(&drawing, "P01");
        let vessel = interfaces_of(&xml, "V1");
        assert!(!vessel
            .iter()
            .any(|name| name.starts_with("ILowPressureTank")));
        assert!(!vessel.iter().any(|name| name == "INamedEquipment"));
        let exchanger = interfaces_of(&xml, "E1");
        assert!(!exchanger.iter().any(|name| name.starts_with("IAirCool")));
        assert_eq!(
            exchanger[exchanger.len() - 3..],
            [
                "IShellAndTubeHeatExchanger",
                "INamedEquipment",
                "IPIDTypical"
            ]
        );
        assert!(xml.contains(r#"      <INamedEquipment EquipSuff=""/>"#));
    }

    #[test]
    fn mechanical_equipment_writes_only_what_a_pump_and_a_motor_write_in_one_order() {
        let mut drawing = drawing();
        drawing.items = vec![
            item("M1", "PIDMechanicalEquipment", &[]),
            item(
                "M2",
                "PIDMechanicalEquipment",
                &[
                    ("INamedEquipment", "EquipPrefix", "P"),
                    ("INamedEquipment", "EquipSeqNo", "101"),
                ],
            ),
        ];
        let xml = write_pid_data_xml(&drawing, "P01");
        let shared = [
            "IObject",
            "IPIDMechanicalEquipmentOcc",
            "IProcessEquipmentOcc",
            "IPIDMechanicalEquipment",
            "IProcessEquipment",
            "IEquipmentOcc",
            "IEquipment",
            "IPlannedMatl",
            "IDrawingItem",
            "IPBSItem",
            "IPump",
        ];
        let mut bare: Vec<&str> = shared.to_vec();
        bare.push("IPIDTypical");
        assert_eq!(interfaces_of(&xml, "M1"), bare);
        let mut named: Vec<&str> = shared.to_vec();
        named.extend(["INamedEquipment", "IPIDTypical"]);
        assert_eq!(interfaces_of(&xml, "M2"), named);
        assert!(xml
            .contains(r#"      <INamedEquipment EquipPrefix="P" EquipSeqNo="101" EquipSuff=""/>"#));
    }

    #[test]
    fn an_interface_its_class_does_not_list_follows_the_listed_ones() {
        let mut drawing = drawing();
        drawing.items = vec![
            item(
                "E1",
                "PIDHeatTransferEquipment",
                &[("ISpecifiedMatlItem", "LongMaterialDescription", "new")],
            ),
            item("X1", "PIDUnlisted", &[("IThing", "Value", "1")]),
        ];
        let xml = write_pid_data_xml(&drawing, "P01");
        let exchanger = interfaces_of(&xml, "E1");
        assert_eq!(
            exchanger[exchanger.len() - 2..],
            ["IPIDTypical", "ISpecifiedMatlItem"]
        );
        assert!(xml.contains(
            "   <PIDUnlisted>\n      <IObject UID=\"X1\"/>\n      <IThing Value=\"1\"/>\n   </PIDUnlisted>\n"
        ));
    }

    #[test]
    fn the_drawing_carries_its_seven_interfaces() {
        let xml = write_pid_data_xml(&drawing(), "P01");
        assert!(xml.contains(concat!(
            "   <PIDDrawing>\n",
            "      <IObject UID=\"D1\" Name=\"A &amp; B\" Description=\"\"/>\n",
            "      <IDocument DocSubtype=\"\"/>\n",
            "      <IDocVersionComposition/>\n",
            "      <IDwgRepresentationComposition/>\n",
            "      <IPIDDrawing/>\n",
            "      <ISchematicDwg/>\n",
            "      <IPBSItem/>\n",
            "   </PIDDrawing>\n",
        )));
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
        assert_eq!(report["classified_items"]["items_named_by_the_file"], 2);
        assert_eq!(report["classified_items"]["items_published"], 1);
        assert_eq!(
            report["classified_items"]["items_left_out_without_a_class"],
            1
        );
        assert_eq!(
            report["classified_items"]["instruments_classed_by_symbol_library"],
            1
        );
        assert_eq!(report["classified_items"]["representations"], 3);
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
    fn the_database_coverage_report_carries_the_template_and_path() {
        let mut drawing = PublishDrawing::new("UID-A01", "A01");
        drawing.template = Some("A2-W-New.pid".to_string());
        drawing.path = Some(r"\01\01\A01.pid".to_string());
        let text = write_database_coverage_json(
            &drawing,
            Path::new("PlantData~2~711.zip:01/01/A01.pid"),
            (12, 29, 15),
            "A01",
        );
        let report: serde_json::Value = serde_json::from_str(&text).expect("report json");
        assert_eq!(report["drawing"]["uid"], "UID-A01");
        assert_eq!(report["template"], "A2-W-New.pid");
        assert_eq!(report["path"], r"\01\01\A01.pid");
        assert_eq!(report["files"]["data"], "A01_Data.xml");
        assert_eq!(report["files"]["meta"], "A01_Meta.xml");
        assert_eq!(report["files"]["coverage"], "A01_Coverage.json");
        assert_eq!(report["document_counts"]["objects"], 12);
        assert_eq!(report["document_counts"]["values"], 29);
        assert_eq!(report["document_counts"]["relationships"], 15);

        // A row that states neither is reported as null, never invented.
        let bare = PublishDrawing::new("UID-B", "B");
        let report: serde_json::Value = serde_json::from_str(&write_database_coverage_json(
            &bare,
            Path::new("Export.dmp"),
            (0, 0, 0),
            "UID-B",
        ))
        .expect("report json");
        assert!(report["template"].is_null());
        assert!(report["path"].is_null());
        assert_eq!(report["drawing"]["name"], "B");
        assert_eq!(
            report["files"]["data"], "UID-B_Data.xml",
            "the files are the ones written, under the stem the package chose"
        );
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
    fn a_pid_meta_xml_derives_version_revision_and_file_from_the_drawing_uid() {
        let text = write_pid_meta_xml(&drawing(), "P01").expect("meta");
        let version = derive_meta_uid("D1", "version");
        let revision = derive_meta_uid("D1", "revision");
        let file = derive_meta_uid("D1", "file");
        assert!(text.contains(r#"DocUID="D1""#), "{text}");
        assert!(text.contains(r#"DocName="A &amp; B""#), "{text}");
        assert!(
            text.contains(&format!(r#"UID="{version}" Name="A &amp; B Version""#)),
            "{text}"
        );
        assert!(
            text.contains(&format!(r#"UID="{revision}" Name="A &amp; B Revision""#)),
            "{text}"
        );
        assert!(
            text.contains(&format!(r#"UID="{file}" Name="A &amp; B.pid""#)),
            "{text}"
        );
        assert!(text.contains(r#"DocVersionDate="""#), "{text}");
        assert!(text.contains(r#"FilePath="""#), "{text}");
        assert!(text.contains(r#"DefUID="VersionedDoc""#), "{text}");
        assert!(text.contains(r#"DefUID="RevisedDocument""#), "{text}");
        assert!(text.contains(r#"DefUID="FileComposition""#), "{text}");
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
        assert!(xml.contains(concat!(
            "   <PIDProcessPoint>\n",
            "      <IObject UID=\"RUN.PPT\"/>\n",
            "      <IProcessPoint/>\n",
            "      <IFacilityPoint/>\n",
            "      <IProcessPointCaseComposition/>\n",
            "      <IBulkMaxProcessPoint PhaseTemperatureMax=\"\" PressureMax=\"\"/>\n",
            "      <IBulkMinProcessPoint PhaseTemperatureMin=\"\" PressureMin=\"\"/>\n",
            "      <IBulkNormProcessPoint MolecularWeightNorm=\"\" CpCvRatioNorm=\"\"/>\n",
            "      <IBulkBaseProcessPoint PhaseTemperatureBase=\"\" PressureBase=\"\"/>\n",
            "   </PIDProcessPoint>\n",
        )));
        assert!(xml.contains(
            "   <Rel>\n      <IObject UID=\"PRP-RUN.PPT-RUN\"/>\n      <IRel UID1=\"RUN.PPT\" UID2=\"RUN\" DefUID=\"ProcessPointCollection\"/>\n   </Rel>\n"
        ));
        let text = write_pid_coverage_json(&drawing, Path::new("a.pid"));
        let report: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(report["classified_items"]["process_points"], 1);
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
        assert_eq!(report["classified_items"]["branch_points"], 2);
        assert_eq!(report["classes"]["PIDPipingBranchPoint"]["objects"], 1);
        assert_eq!(report["classes"]["PIDSignalBranchPoint"]["objects"], 1);
        assert_eq!(report["attribute_values"]["derived_by_native_rule"], 2);
    }

    #[test]
    fn the_report_tallies_exactly_the_tags_the_catalog_gives_a_tally_role() {
        // The derived `classes` rows come from the catalog's `PidTagRole`
        // now, not from names here. Pin the resulting key set so a catalog
        // role change has to show up as a deliberate report change.
        use crate::publish::pid_load::PidPort;
        let mut drawing = drawing();
        drawing.items.clear();
        drawing.representations.clear();
        drawing.process_points.push(PidProcessPoint {
            uid: "RUN.PPT".to_string(),
            connector_uid: "RUN".to_string(),
            source: PidValueSource::DerivedByNativeRule {
                analysis_page: "docs/analysis/n2a-port-relation-filter.md",
                address: "0x5F5CFD40",
            },
        });
        for (uid, class) in [
            ("REP.BPT", "PIDPipingBranchPoint"),
            ("SIG.BPT", "PIDSignalBranchPoint"),
        ] {
            drawing.branch_points.push(PidBranchPoint {
                uid: uid.to_string(),
                representation_uid: uid.trim_end_matches(".BPT").to_string(),
                class,
                source: PidValueSource::DerivedByNativeRule {
                    analysis_page: "docs/analysis/n2a-port-relation-filter.md",
                    address: "0x5F5CFC10",
                },
            });
        }
        for (uid, class) in [("P1", "PIDPipingPort"), ("S1", "PIDSignalPort")] {
            drawing.ports.push(PidPort {
                uid: uid.to_string(),
                parent_uid: "RUN".to_string(),
                class,
                attributes: Vec::new(),
            });
        }
        let text = write_pid_coverage_json(&drawing, Path::new("a.pid"));
        let report: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        let classes = report["classes"].as_object().expect("classes object");
        let mut keys: Vec<&str> = classes.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "PIDDrawing",
                "PIDPipingBranchPoint",
                "PIDPipingPort",
                "PIDProcessPoint",
                "PIDSignalBranchPoint",
                "PIDSignalPort",
            ],
            "derived `classes` rows drifted: {text}"
        );
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
        assert_eq!(report["oracle_exp"]["dump_equipment_compositions"], 2);
        assert_eq!(report["oracle_exp"]["equipment_composition_rels"], 1);
        assert_eq!(report["oracle_exp"]["dump_item_notes"], 1);
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

        drawing.opc_points[0].to_from_text.clear();
        let xml = write_pid_data_xml(&drawing, "P01");
        assert!(xml.contains(r#"      <IObject UID="OPC" Name="565" Description="去G井场"/>"#));
        assert!(
            !xml.contains("IOffpageConnector"),
            "no ToFromText, no IOffpageConnector element: {xml}"
        );
    }

    #[test]
    fn a_pipeline_writes_its_fluid_code_before_its_fluid_system() {
        let mut drawing = drawing();
        drawing.pipelines.push(PidPipeline {
            uid: "PL1".to_string(),
            name: "80-AB-001".to_string(),
            fluid_code: Some("@{CODE}".to_string()),
            fluid_system: Some("@{SYSTEM}".to_string()),
            connector_uids: vec!["RUN".to_string()],
        });
        drawing.pipelines.push(PidPipeline {
            uid: "PL2".to_string(),
            name: "80-AB-002".to_string(),
            fluid_code: Some("@{CODE}".to_string()),
            fluid_system: None,
            connector_uids: vec!["RUN".to_string()],
        });
        let xml = write_pid_data_xml(&drawing, "P01");
        assert!(
            xml.contains(r#"      <IFluidSystem FluidCode="@{CODE}" FluidSystem="@{SYSTEM}"/>"#)
        );
        assert!(xml.contains(r#"      <IFluidSystem FluidCode="@{CODE}"/>"#));
    }

    #[test]
    fn classes_is_a_census_of_the_sibling_documents_top_level_blocks() {
        let mut drawing = drawing();
        drawing.pipelines.push(PidPipeline {
            uid: "PL1".to_string(),
            name: "80-AB-001".to_string(),
            fluid_code: None,
            fluid_system: Some("@{GUID}".to_string()),
            connector_uids: vec!["RUN".to_string()],
        });
        drawing.opc_points.push(PidOpcPoint {
            uid: "OPC".to_string(),
            representation_uid: "R1".to_string(),
            name: "565".to_string(),
            description: String::new(),
            to_from_text: String::new(),
        });
        let text = write_pid_coverage_json(&drawing, Path::new("a.pid"));
        let report: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        let classes = report["classes"].as_object().expect("classes object");
        assert_eq!(classes["PIDInstrument"]["objects"], 1);
        for (class, objects) in [
            ("PIDDrawing", 1),
            ("PIDRepresentation", 3),
            ("PIDPipeline", 1),
            ("PIDBranchPoint", 1),
        ] {
            assert_eq!(classes[class]["objects"], objects, "{class}");
            assert_eq!(classes[class]["with_name"], 0, "{class}");
            assert_eq!(classes[class]["attribute_values"], 0, "{class}");
        }
        let census: usize = classes
            .values()
            .map(|row| row["objects"].as_u64().expect("objects") as usize)
            .sum();
        let blocks = write_pid_data_xml(&drawing, "P01")
            .lines()
            .filter(|line| {
                line.starts_with("   <")
                    && !line.starts_with("   </")
                    && !line.starts_with("   <Rel>")
            })
            .count();
        assert_eq!(report["classes_total"], census);
        assert_eq!(census, blocks);
    }
}
