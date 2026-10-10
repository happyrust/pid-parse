//! Publish Data XML from a [`PidPublishDrawing`]: only what the `.pid` states.
//!
//! The `.pid`-only counterpart of [`super::xml_writer`] (plan
//! `docs/plans/2026-10-10-publish-from-the-pid-alone.md`; on 2026-10-10 the
//! user chose a separate writer that emits nothing the file does not give).
//! An object carries `IObject` and only the interfaces it has values for; a
//! representation its `GraphicOID`; the relationships are the two the items'
//! own rows state -- `DwgRepresentationComposition` (drawing -> representation)
//! and `DrawingItems` (representation -> item) -- under the
//! `{prefix}-{UID1}-{UID2}` UIDs `SmartPlant` gives them. Items whose class the
//! file does not decide are left out, and so are their `DrawingItems`. The
//! `<Container>` envelope constants are `SmartPlant`'s, not the file's.
//!
//! [`write_pid_coverage_json`] renders the report that goes beside the two
//! documents (plan P5): what the file named, what was written, what was left
//! out and why.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::Path;

use serde_json::json;

use super::model::{PublishDrawing, PublishError};
use super::pid_load::{PidPublishDrawing, PidPublishItem, PidValueSource};
use super::xml_writer::{defuid_prefix, escape_attr, write_meta_xml};

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

/// Render the `_Meta.xml` companion: the document-versioning envelope of
/// [`write_meta_xml`] for the drawing's UID and name.
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
    writeln!(buf, "  </Container>")
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
/// representation whose stated item is published.
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
}

/// What a `.pid`-only export never carries, for the reader of the report.
const LEFT_OUT_BY_DESIGN: [&str; 5] = [
    "ports, process points, branch points and pipelines: a .pid does not state them",
    "off-page connector items: the ModelID on an OPC's row is not always the item SmartPlant publishes",
    "instruments without a SymbolAttr row: the file does not decide their class, so the item, its DrawingItems and its label values are left out",
    "codelist attributes, nominal diameters and concatenated pipeline / component names: they come from the plant database",
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
    let report = json!({
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
            "representations": drawing.representations.len(),
        },
        "relationships": {
            "DwgRepresentationComposition": drawing.representations.len(),
            "DrawingItems": drawing_items(drawing, &published),
        },
        "attribute_values": {
            "label_values": stats.label_values,
            "label_values_reaching_an_item": stats.label_values_reached,
            "published_from_labels": stats.label_values_published,
            "composed_names": composed_names,
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
    let mut text = serde_json::to_string_pretty(&report).unwrap_or_default();
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::publish::pid_load::{
        PidPublishAttribute, PidPublishRepresentation, PidPublishStats,
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
        };
        let text = write_pid_coverage_json(&drawing, Path::new(r"D:\plant\A & B.pid"));
        let report: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(report["source"], r"D:\plant\A & B.pid");
        assert_eq!(report["drawing"]["uid"], "D1");
        assert_eq!(report["files"]["data"], "A & B_Data.xml");
        assert_eq!(report["objects"]["items_named_by_the_file"], 2);
        assert_eq!(report["objects"]["items_published"], 1);
        assert_eq!(report["objects"]["items_left_out_without_a_class"], 1);
        assert_eq!(report["objects"]["representations"], 3);
        assert_eq!(report["relationships"]["DwgRepresentationComposition"], 3);
        assert_eq!(report["relationships"]["DrawingItems"], 1);
        assert_eq!(report["attribute_values"]["published_from_labels"], 2);
        assert_eq!(report["attribute_values"]["composed_names"], 1);
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
}
