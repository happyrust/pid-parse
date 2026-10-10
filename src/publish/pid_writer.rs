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

use std::collections::BTreeSet;
use std::fmt::Write;

use super::model::{PublishDrawing, PublishError};
use super::pid_load::{PidPublishDrawing, PidPublishItem};
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

    let mut published = BTreeSet::new();
    for item in &drawing.items {
        if let Some(class) = item.class {
            write_item(buf, item, class)?;
            published.insert(item.uid.as_str());
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::publish::pid_load::{
        PidPublishAttribute, PidPublishRepresentation, PidPublishStats, PidValueSource,
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
}
