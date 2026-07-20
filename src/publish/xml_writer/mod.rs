//! Publish-Data XML writer — DTO → SmartPlant-compatible XML.
//!
//! Current scope:
//!
//! * emits both `_Data.xml` and `_Meta.xml`;
//! * covers the 15 PID tag families currently declared in
//!   `publish::supported_pid_tags()` plus the drawing-scoped
//!   derived nodes already modeled on the DTO side
//!   (`PIDPipingPort`, `PIDProcessPoint`, `PIDSignalPort`);
//! * preserves the explicit `PublishStyle::{A01,Dwg}` selector
//!   rather than auto-detecting plant flavor.
//!
//! The remaining publish backlog is concentrated in DWG-mirror-
//! gated work: loader canonical-field enrichment for DWG-only
//! attributes and closing the A24/A27b tolerated divergences.
//! The `PIDBranchPoint` and `PIDPipingBranchPoint` writer arms
//! are implemented (Stage-4) but the loader-side item-type
//! mapping is provisional until the DWG mirror confirms it.
//!
//! ## Format guarantees
//!
//! * UTF-8 output, indented for human inspection (two-space
//!   indent, trailing newline). `SmartPlant` accepts compact and
//!   indented forms alike.
//! * Strings go through XML entity escaping so names / paths with
//!   `&`, `<`, quotes, CR/LF round-trip cleanly.
//! * Unknown optional fields render as empty attribute values
//!   (`Description=""`) — matches how SPPID itself emits
//!   blank-but-present attributes.
//! * Writer-synthesized publish-only identifiers are
//!   deterministic, so repeated exports remain byte-stable.
//!   A01 raw parity is already closed at the contract level, but
//!   the connector-family UID, `<Rel><IObject UID="..."/>`, and
//!   `GraphicOID` publish numbering still use writer-side
//!   placeholder strategies pending full SmartPlant-rule
//!   reconstruction.

mod common;
mod components_notes_branch;
mod drawing;
mod instrument_signal;
mod meta;
mod pipeline;
mod relationships;
mod vessel_nozzle;

pub use drawing::write_data_xml;
pub use meta::write_meta_xml;

#[cfg(test)]
use super::model::{PublishDrawing, PublishStyle};
#[cfg(test)]
use common::{escape_attr, representation_is_publishable};
#[cfg(test)]
use instrument_signal::INSTR_DERIVED_SIGNAL_PORT_COUNT;
#[cfg(test)]
use meta::{derive_meta_uid, format_meta_date};

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
                symbol_path: Some(
                    r"\Equipment\Vessels\Horizontal Drums\Horizontal Drum.sym".into(),
                ),
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
    fn xml_skips_rep_to_rep_t_relationship_rows_to_avoid_drc_double_emit() {
        // A40 — the example drawing's T_Relationship row
        // ties the Nozzle representation to the Vessel
        // representation. The writer *used* to classify that
        // pair as `DwgRepresentationComposition` and emit a
        // DRC- prefixed rel from it, but that over-counts
        // DwgRepresentationComposition on A01 (reference
        // emits 4, pre-A40 writer emitted 6). Investigation
        // on the A01 fixture showed SmartPlant's exporter
        // never emits Rep↔Rep T_Relationship rows as their
        // own `<Rel>` entries — the ModelItem → Rep derived
        // loop already produces the correct DRC inventory.
        let out = write_data_xml(&example_drawing(), "TEST02").expect("write");
        // The REP↔REP composite UID must NOT appear.
        assert!(
            !out.contains(
                r#"<IObject UID="DRC-C33E5BD9B9CC4287B244A925A7A1F29B-CA8A0A9DD1784E3BB6913445CE3F6375"/>"#
            ),
            "Rep↔Rep T_Relationship row must no longer produce a DRC rel; out:\n{out}"
        );
        // The ModelItem → Rep derived DRC rels must still
        // be present (one per representation that carries a
        // `model_item_uid`). This is the single source of
        // truth for DwgRepresentationComposition after A40.
        assert!(out.contains(
            r#"<IRel UID1="C57494A1B154442C9DF0F4BA713E88EC" UID2="CA8A0A9DD1784E3BB6913445CE3F6375" DefUID="DwgRepresentationComposition"/>"#
        ));
        assert!(out.contains(
            r#"<IRel UID1="7465E81219DB49B492BDF60A055AA391" UID2="C33E5BD9B9CC4287B244A925A7A1F29B" DefUID="DwgRepresentationComposition"/>"#
        ));
    }

    // -------------------------------------------------------------
    // A34c — PipingEnd1Conn / PipingEnd2Conn UID2 real endpoint inference
    // -------------------------------------------------------------

    #[test]
    fn a34c_piping_end1_conn_uses_upstream_model_item_uid_when_loader_attached() {
        // Simulate what `attach_pipe_endpoint_connections` would
        // have written to the PipeRun obj: port.1 connects to the
        // Nozzle (7465...), port.2 is unconnected (no field).
        let mut d = example_drawing();
        let pipe = d
            .objects
            .iter_mut()
            .find(|o| o.item_type_name == "PipeRun")
            .expect("PipeRun in fixture");
        pipe.fields.insert(
            "EndConnectedItem1".into(),
            "7465E81219DB49B492BDF60A055AA391".into(),
        );
        let out = write_data_xml(&d, "TEST02").expect("write");
        // PipingEnd1Conn UID2 is the real Nozzle UID, not the
        // placeholder `.PPT`.
        assert!(
            out.contains(
                r#"<IRel UID1="185EF98B03E844158E3BD8E82806E6CF-CNX.1" UID2="7465E81219DB49B492BDF60A055AA391" DefUID="PipingEnd1Conn"/>"#
            ),
            "PipingEnd1Conn UID2 must be the Nozzle ModelItem UID; out:\n{out}"
        );
        // PipingEnd2Conn still falls back to `.PPT` because the
        // loader did not populate EndConnectedItem2.
        assert!(
            out.contains(
                r#"<IRel UID1="185EF98B03E844158E3BD8E82806E6CF-CNX.2" UID2="185EF98B03E844158E3BD8E82806E6CF-CNX.PPT" DefUID="PipingEnd2Conn"/>"#
            ),
            "PipingEnd2Conn UID2 without loader field must fall back to the .PPT placeholder; out:\n{out}"
        );
    }

    #[test]
    fn a34c_piping_end_conn_falls_back_to_ppt_when_fields_absent() {
        // Legacy path: no loader attachment, both ends use the
        // pre-A34c `.PPT` placeholder. Keeps synthetic unit tests
        // and pid-only bundles working unchanged.
        let d = example_drawing();
        let pipe = d
            .objects
            .iter()
            .find(|o| o.item_type_name == "PipeRun")
            .expect("PipeRun in fixture");
        assert!(
            !pipe.fields.contains_key("EndConnectedItem1"),
            "precondition: fixture has no loader-attached endpoint"
        );
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(out.contains(
            r#"<IRel UID1="185EF98B03E844158E3BD8E82806E6CF-CNX.1" UID2="185EF98B03E844158E3BD8E82806E6CF-CNX.PPT" DefUID="PipingEnd1Conn"/>"#
        ));
        assert!(out.contains(
            r#"<IRel UID1="185EF98B03E844158E3BD8E82806E6CF-CNX.2" UID2="185EF98B03E844158E3BD8E82806E6CF-CNX.PPT" DefUID="PipingEnd2Conn"/>"#
        ));
    }

    #[test]
    fn a34c_piping_end2_conn_honors_end2_field_when_both_populated() {
        // Two-ended pipe: the loader resolved both port.1 and
        // port.2 to real ModelItems. Writer must route each end
        // independently.
        let mut d = example_drawing();
        let pipe = d
            .objects
            .iter_mut()
            .find(|o| o.item_type_name == "PipeRun")
            .expect("PipeRun in fixture");
        pipe.fields.insert(
            "EndConnectedItem1".into(),
            "7465E81219DB49B492BDF60A055AA391".into(),
        );
        pipe.fields.insert(
            "EndConnectedItem2".into(),
            "C57494A1B154442C9DF0F4BA713E88EC".into(),
        );
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"UID2="7465E81219DB49B492BDF60A055AA391" DefUID="PipingEnd1Conn""#),
            "PipingEnd1Conn UID2 = Nozzle UID"
        );
        assert!(
            out.contains(r#"UID2="C57494A1B154442C9DF0F4BA713E88EC" DefUID="PipingEnd2Conn""#),
            "PipingEnd2Conn UID2 = Vessel UID"
        );
        // The PPT placeholder must no longer appear as a target
        // on either PipingEnd rel.
        assert!(
            !out.contains(r#"UID2="185EF98B03E844158E3BD8E82806E6CF-CNX.PPT" DefUID="PipingEnd"#),
            "PPT placeholder must be fully displaced when both fields are set; out:\n{out}"
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
    fn vessel_eq_type_uses_codelist_lookup_when_available() {
        // When the drawing ships a codelist entry for
        // EquipmentType = "0", the writer MUST prefer the codelist
        // text over the symbol-path stem. This mirrors SmartPlant's
        // own rendering (the enum display text is the source of
        // truth; the symbol path is a UI convention).
        let mut d = example_drawing();
        // Seed the vessel with an EquipmentType code so the codelist
        // path has something to resolve.
        let vessel = d
            .objects
            .iter_mut()
            .find(|o| o.item_type_name == "Vessel")
            .expect("vessel in fixture");
        vessel.fields.insert("EquipmentType".into(), "3".into());
        // Register EquipmentType → codelist 28 → "3" = "Reactor".
        d.codelist.insert_attribute_mapping("EquipmentType", "28");
        d.codelist.insert_entry("28", "3", "Reactor");

        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IEquipment EqTypeDescription="Reactor"/>"#),
            "codelist-resolved description should win over the symbol-path stem; out:\n{out}"
        );
        // The symbol path stem ("Horizontal Drum") must NOT appear
        // as the EqType description — it's still in the XML as part
        // of the rep's FileName chain, but not on <IEquipment>.
        assert!(
            !out.contains(r#"EqTypeDescription="Horizontal Drum""#),
            "codelist lookup should beat symbol-path fallback; out:\n{out}"
        );
    }

    #[test]
    fn vessel_eq_type_falls_back_to_symbol_path_when_codelist_empty() {
        // No codelist metadata loaded → the writer's second-tier
        // fallback (symbol-path stem) must still produce the human
        // name so legacy fixtures keep working.
        let d = example_drawing();
        assert!(d.codelist.is_empty(), "fixture ships with empty codelist");
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IEquipment EqTypeDescription="Horizontal Drum"/>"#),
            "symbol path should still surface when codelist is empty; out:\n{out}"
        );
    }

    #[test]
    fn vessel_eq_type_falls_back_to_raw_code_when_no_symbol_and_no_codelist() {
        // Neither a codelist mapping nor a symbol path — the writer
        // must still emit something, and the raw EquipmentType code
        // is the last-ditch choice. (A blank attribute would silently
        // hide data, which is strictly worse than an opaque code.)
        let mut d = PublishDrawing::new("UID-V", "V");
        d.objects = vec![PublishObject {
            uid: "V1".into(),
            item_type_name: "Vessel".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("EquipmentType".into(), "7".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IEquipment EqTypeDescription="7"/>"#),
            "raw EquipmentType code must still land when both preferred \
             lookups miss; out:\n{out}"
        );
    }

    #[test]
    fn nozzle_proc_eq_comp_uses_codelist_lookup_when_available() {
        // Same three-tier fallback as the vessel, but keyed on
        // `NozzleType`. When the catalog ships the mapping the
        // writer must prefer it.
        let mut d = example_drawing();
        let nozzle = d
            .objects
            .iter_mut()
            .find(|o| o.item_type_name == "Nozzle")
            .expect("nozzle in fixture");
        nozzle.fields.insert("NozzleType".into(), "2".into());
        d.codelist.insert_attribute_mapping("NozzleType", "12");
        d.codelist.insert_entry("12", "2", "Pressurized Nozzle");

        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"ProcEqpCompTypeDescription="Pressurized Nozzle""#),
            "codelist-resolved nozzle description should win; out:\n{out}"
        );
    }

    #[test]
    fn nozzle_proc_eq_comp_keeps_default_when_nothing_else_available() {
        // No codelist, no symbol path — the default `"Flanged Nozzle"`
        // still lands so every nozzle has a non-empty description.
        let mut d = PublishDrawing::new("UID-N", "N");
        d.objects = vec![PublishObject {
            uid: "NZ1".into(),
            item_type_name: "Nozzle".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"ProcEqpCompTypeDescription="Flanged Nozzle""#),
            "hard-coded `Flanged Nozzle` fallback still fires; out:\n{out}"
        );
    }

    #[test]
    fn a01_pipeline_prefers_expanded_publish_tags_when_pipe_fields_available() {
        // A01 reference XML expands the pipe tag from PipeRun
        // business fields even when T_PlantItem.ItemTag carries
        // the shorter catalog tag (`A010102102-PH` in TEST02).
        let mut d = PublishDrawing::new("UID-A01", "A01");
        d.objects = vec![PublishObject {
            uid: "PIPE-UID".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("ItemTag".into(), "A010102102-PH".into());
                m.insert("TagSequenceNo".into(), "0102102".into());
                m.insert("NominalDiameter".into(), "250".into());
                m.insert("PipingMaterialsClass".into(), "B5".into());
                m.insert("InsulThick".into(), "40\"".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"ItemTag="PH- 0102102-DN250 mm-B5-P-40.000 in""#),
            "A01 PIDPipeline should use the expanded publish tag; out:\n{out}"
        );
        assert!(
            out.contains(r#"ItemTag="PH-0102102-250 mm-B5""#),
            "A01 PIDPipingConnector should use the compact pipe tag; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_synthesizes_tag_when_plantitem_itemtag_absent() {
        // No `ItemTag` key in obj.fields → the legacy `PH-…`
        // synthesis path should still fire so drawings without
        // T_PlantItem data remain readable.
        let mut d = PublishDrawing::new("UID-A01", "A01");
        d.objects = vec![PublishObject {
            uid: "PIPE-UID".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("TagSequenceNo".into(), "0102102".into());
                m.insert("NominalDiameter".into(), "250".into());
                m.insert("PipingMaterialsClass".into(), "B5".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"ItemTag="PH-0102102-250 mm-B5""#),
            "synthesized PH- tag should fire when T_PlantItem.ItemTag is missing; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_empty_itemtag_treated_as_absent_and_falls_back() {
        // A T_PlantItem row that is present but with an EMPTY ItemTag
        // (SmartPlant's legal "tag not yet assigned" state) must not
        // overrule the synthesized fallback — otherwise the XML would
        // emit `ItemTag=""` which is less useful than a synthesized
        // placeholder.
        let mut d = PublishDrawing::new("UID-A01", "A01");
        d.objects = vec![PublishObject {
            uid: "PIPE-UID".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("ItemTag".into(), "".into());
                m.insert("TagSequenceNo".into(), "0102102".into());
                m.insert("NominalDiameter".into(), "250".into());
                m.insert("PipingMaterialsClass".into(), "B5".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"ItemTag="PH-0102102-250 mm-B5""#),
            "empty PlantItem ItemTag should fall through to synthesized form; out:\n{out}"
        );
        assert!(
            !out.contains(r#"ItemTag="""#),
            "writer must never emit an empty ItemTag attribute; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_without_any_tag_info_falls_back_to_uid() {
        // Final fallback tier — no ItemTag, no TagSequenceNo —
        // emit the raw UID so the attribute is at least uniquely
        // identifying even if it's not human-readable.
        let mut d = PublishDrawing::new("UID-A01", "A01");
        d.objects = vec![PublishObject {
            uid: "BARE-UID".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"ItemTag="BARE-UID""#),
            "bare UID should surface when no ItemTag / TagSequenceNo present; out:\n{out}"
        );
    }

    #[test]
    fn piping_point_emits_pid_piping_port_tag_with_nominal_diameter() {
        // A9: PipingPoint objects synthesized from T_PipingPoint
        // rows must render as <PIDPipingPort> with the nominal
        // diameter carried through.
        let mut d = PublishDrawing::new("UID-A01", "A01");
        d.objects = vec![PublishObject {
            uid: "PP-UID".into(),
            item_type_name: "PipingPoint".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("NominalDiameter".into(), "250".into());
                m.insert("SP_PlantItemID".into(), "NOZZLE1".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains("<PIDPipingPort>"),
            "PipingPoint should render as <PIDPipingPort>; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IObject UID="PP-UID"/>"#),
            "IObject should carry the PipingPoint UID; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IPipeCrossSectionItem NominalDiameter="250 mm"/>"#),
            "NominalDiameter should round-trip with the `mm` unit; out:\n{out}"
        );
        assert!(
            out.contains("</PIDPipingPort>"),
            "PIDPipingPort must close; out:\n{out}"
        );
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

    // -----------------------------------------------------------------
    // A10.2 — _Meta.xml writer
    // -----------------------------------------------------------------

    #[test]
    fn meta_xml_renders_doc_versioning_container_header() {
        let d = PublishDrawing::new("D9635C3C898840D1990B7E8BEE1D55DA", "A01");
        let out = write_meta_xml(&d, "TEST02").expect("write meta");
        assert!(
            out.contains(r#"CompSchema="DocVersioningComponent""#),
            "meta document must advertise the DocVersioningComponent schema; out:\n{out}"
        );
        assert!(
            out.contains(r#"Plant="TEST02""#),
            "Plant attribute must round-trip; out:\n{out}"
        );
        assert!(
            out.contains(r#"DocUID="D9635C3C898840D1990B7E8BEE1D55DA""#),
            "DocUID must equal drawing_uid; out:\n{out}"
        );
        assert!(
            out.contains(r#"DocName="A01""#),
            "DocName must equal drawing_name; out:\n{out}"
        );
        assert!(
            out.trim_end().ends_with("</Container>"),
            "container must close; out:\n{out}"
        );
    }

    #[test]
    fn meta_xml_emits_three_main_blocks_and_three_rels_in_order() {
        let d = PublishDrawing::new("UID-ABC", "DRAW1");
        let out = write_meta_xml(&d, "P01").expect("write meta");

        let pos_version = out
            .find("<DocumentVersion>")
            .expect("DocumentVersion present");
        let pos_revision = out
            .find("<DocumentRevision>")
            .expect("DocumentRevision present");
        let pos_file = out.find("<File>").expect("File present");

        assert!(
            pos_version < pos_revision && pos_revision < pos_file,
            "blocks must be ordered DocumentVersion < DocumentRevision < File; out:\n{out}"
        );

        let rel_count = out.matches("<Rel>").count();
        assert_eq!(
            rel_count, 3,
            "meta document carries exactly three Rel rows; out:\n{out}"
        );

        for def_uid in ["VersionedDoc", "RevisedDocument", "FileComposition"] {
            assert!(
                out.contains(&format!(r#"DefUID="{def_uid}""#)),
                "DefUID `{def_uid}` must appear; out:\n{out}"
            );
        }
    }

    #[test]
    fn meta_xml_uids_are_deterministic_across_runs() {
        let d = PublishDrawing::new("UID-ABC", "DRAW1");
        let out_a = write_meta_xml(&d, "P01").expect("write");
        let out_b = write_meta_xml(&d, "P01").expect("write again");
        assert_eq!(
            out_a, out_b,
            "deterministic UID derivation must produce byte-identical meta XML"
        );
    }

    #[test]
    fn meta_xml_uses_drawing_name_for_version_revision_and_file_node() {
        let d = PublishDrawing::new("UID-1", "TANK-01");
        let out = write_meta_xml(&d, "P01").expect("write");
        assert!(
            out.contains(r#"Name="TANK-01 Version""#),
            "DocumentVersion IObject Name should embed drawing name; out:\n{out}"
        );
        assert!(
            out.contains(r#"Name="TANK-01 Revision""#),
            "DocumentRevision IObject Name should embed drawing name; out:\n{out}"
        );
        assert!(
            out.contains(r#"Name="TANK-01.pid""#),
            "File IObject Name should be `<drawing>.pid`; out:\n{out}"
        );
    }

    #[test]
    fn meta_xml_normalizes_date_created_to_yyyy_mm_dd() {
        let mut d = PublishDrawing::new("UID-1", "A01");
        d.date_created = Some("2026/4/20 10:32:46".into());
        let out = write_meta_xml(&d, "P01").expect("write");
        assert!(
            out.contains(r#"DocVersionDate="2026/04/20""#),
            "MDF loader raw date should zero-pad to YYYY/MM/DD; out:\n{out}"
        );
        assert!(
            !out.contains(r#"DocVersionDate="2026/4/20 10:32:46""#),
            "raw timestamp must not appear verbatim; out:\n{out}"
        );
    }

    #[test]
    fn meta_xml_handles_missing_date_with_empty_attribute() {
        let d = PublishDrawing::new("UID-1", "A01");
        let out = write_meta_xml(&d, "P01").expect("write");
        assert!(
            out.contains(r#"DocVersionDate="""#),
            "missing date_created should surface as DocVersionDate=\"\"; out:\n{out}"
        );
    }

    #[test]
    fn meta_xml_rel_uid1_uid2_match_expected_topology() {
        let d = PublishDrawing::new("DUID", "A01");
        let out = write_meta_xml(&d, "P01").expect("write");

        let version_uid = derive_meta_uid("DUID", "version");
        let revision_uid = derive_meta_uid("DUID", "revision");
        let file_uid = derive_meta_uid("DUID", "file");

        // Drawing -> Version (VersionedDoc)
        assert!(
            out.contains(&format!(
                r#"UID1="DUID" UID2="{version_uid}" DefUID="VersionedDoc""#
            )),
            "VersionedDoc rel must wire drawing -> version; out:\n{out}"
        );
        // Revision -> Drawing (RevisedDocument)
        assert!(
            out.contains(&format!(
                r#"UID1="{revision_uid}" UID2="DUID" DefUID="RevisedDocument""#
            )),
            "RevisedDocument rel must wire revision -> drawing; out:\n{out}"
        );
        // File -> Version (FileComposition)
        assert!(
            out.contains(&format!(
                r#"UID1="{file_uid}" UID2="{version_uid}" DefUID="FileComposition""#
            )),
            "FileComposition rel must wire file -> version; out:\n{out}"
        );
    }

    #[test]
    fn derive_meta_uid_is_uppercase_32_hex() {
        let uid = derive_meta_uid("DUID", "version");
        assert_eq!(uid.len(), 32, "derived UID must be 32 hex chars; got {uid}");
        assert!(
            uid.chars()
                .all(|c| c.is_ascii_hexdigit()
                    && (!c.is_ascii_alphabetic() || c.is_ascii_uppercase())),
            "derived UID must be uppercase hex only; got {uid}"
        );
    }

    #[test]
    fn derive_meta_uid_distinguishes_role_within_same_seed() {
        let v = derive_meta_uid("DUID", "version");
        let r = derive_meta_uid("DUID", "revision");
        let f = derive_meta_uid("DUID", "file");
        assert_ne!(v, r);
        assert_ne!(r, f);
        assert_ne!(v, f);
    }

    #[test]
    fn format_meta_date_returns_input_for_unrecognized_shapes() {
        // Anything that doesn't parse as YYYY/M/D is returned as-is
        // so the loader / debugger can still see what came through.
        assert_eq!(format_meta_date("2026-04-20"), "2026-04-20");
        assert_eq!(format_meta_date("not-a-date"), "not-a-date");
        assert_eq!(format_meta_date(""), "");
    }

    // -----------------------------------------------------------------
    // A11 — Note / InstrFunction writers
    // -----------------------------------------------------------------

    #[test]
    fn item_note_emits_pid_note_with_text_from_description() {
        // Note rows use `T_ModelItem.Description` as their primary
        // text source (verified against DWG-0202GP06-01_Data.xml).
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "NOTE-1".into(),
            item_type_name: "Note".into(),
            description: Some("量液孔".into()),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains("<PIDNote>"),
            "Note must render as <PIDNote>; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IObject UID="NOTE-1"/>"#),
            "Note IObject must carry the UID; out:\n{out}"
        );
        assert!(
            out.contains(r#"<INote NoteText="量液孔"/>"#),
            "Description should source NoteText; out:\n{out}"
        );
        assert!(
            out.contains("</PIDNote>"),
            "PIDNote must close; out:\n{out}"
        );
    }

    #[test]
    fn item_note_alias_routes_to_pid_note() {
        // SmartPlant ships the type as both "Note" and "ItemNote"
        // depending on the backup era; both must hit the same writer.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "NOTE-2".into(),
            item_type_name: "ItemNote".into(),
            description: Some("Hello".into()),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains("<PIDNote>"),
            "ItemNote must also dispatch to write_item_note; out:\n{out}"
        );
        assert!(
            out.contains(r#"<INote NoteText="Hello"/>"#),
            "ItemNote text round-trips; out:\n{out}"
        );
    }

    #[test]
    fn item_note_prefers_note_text_field_over_description() {
        // When a fixture stamps an explicit `NoteText` field on the
        // model item (some SmartPlant versions do), that wins over
        // the generic Description column.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "NOTE-3".into(),
            item_type_name: "Note".into(),
            description: Some("ignored".into()),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("NoteText".into(), "explicit-text".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<INote NoteText="explicit-text"/>"#),
            "fields[NoteText] must override Description; out:\n{out}"
        );
        assert!(
            !out.contains(r#"<INote NoteText="ignored"/>"#),
            "Description must not appear when NoteText is set; out:\n{out}"
        );
    }

    #[test]
    fn item_note_with_no_text_renders_bare_inote() {
        // A24 aligned the empty-text path with the DWG reference
        // shape: SmartPlant emits `<INote/>` (bare), not
        // `<INote NoteText=""/>`. Both are semantically equivalent
        // but SPPID validators compare byte-level, so matching
        // the bare form removes a spurious diff signal.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "NOTE-4".into(),
            item_type_name: "Note".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains("<INote/>"),
            "missing text must render as bare <INote/>; out:\n{out}"
        );
        // And must NOT emit the pre-A24 `<INote NoteText=""/>`
        // attribute form.
        assert!(
            !out.contains(r#"<INote NoteText=""/>"#),
            "A24 must no longer emit the empty-attribute form; out:\n{out}"
        );
    }

    #[test]
    fn item_note_escapes_xml_special_chars_in_note_text() {
        // SmartPlant accepts entity-escaped attribute values, so
        // `<` / `>` / `&` / `"` in note bodies must round-trip
        // cleanly without breaking the document.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "NOTE-5".into(),
            item_type_name: "Note".into(),
            description: Some(r#"a < b & "c" > d"#.into()),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"NoteText="a &lt; b &amp; &quot;c&quot; &gt; d""#),
            "XML special chars must be entity-escaped; out:\n{out}"
        );
    }

    #[test]
    fn instr_function_emits_pid_control_system_function_with_derived_name() {
        // Mirrors DWG-0202GP06-01: MeasuredVariableCode + TagSequenceNo
        // → IObject Name = `LIA-060201`.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-1".into(),
            item_type_name: "InstrFunction".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("MeasuredVariableCode".into(), "LIA".into());
                m.insert("TagSequenceNo".into(), "060201".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains("<PIDControlSystemFunction>"),
            "InstrFunction must render as <PIDControlSystemFunction>; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IObject UID="INSTR-1" Name="LIA-060201"/>"#),
            "Name must combine MeasuredVariable + TagSequenceNo; out:\n{out}"
        );
        assert!(
            out.contains(r#"InstrTagSequenceNo="060201""#),
            "INamedInstrument must carry InstrTagSequenceNo; out:\n{out}"
        );
        assert!(
            out.contains(r#"MeasuredVariable="LIA""#),
            "INamedInstrument must carry MeasuredVariable; out:\n{out}"
        );
        assert!(
            out.contains("</PIDControlSystemFunction>"),
            "PIDControlSystemFunction must close; out:\n{out}"
        );
    }

    #[test]
    fn instrument_alias_dispatches_to_control_system_function() {
        // The sibling `Instrument` ItemTypeName routes to the same
        // writer, matching the SPPID convention where the logical
        // function tag is the one rendered to XML.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-2".into(),
            item_type_name: "Instrument".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("MeasuredVariableCode".into(), "FT".into());
                m.insert("TagSequenceNo".into(), "001".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains("<PIDControlSystemFunction>"),
            "Instrument must also dispatch to PIDControlSystemFunction; out:\n{out}"
        );
        assert!(
            out.contains(r#"Name="FT-001""#),
            "Name composition rule applies regardless of alias; out:\n{out}"
        );
    }

    #[test]
    fn instr_function_falls_back_when_only_measured_variable_present() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-3".into(),
            item_type_name: "InstrFunction".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("MeasuredVariableCode".into(), "LIA".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"Name="LIA""#),
            "missing TagSequenceNo should fall through to bare MeasuredVariable; out:\n{out}"
        );
    }

    #[test]
    fn instr_function_omits_name_attribute_when_no_signals_available() {
        // Without MeasuredVariableCode AND without TagSequenceNo
        // the writer must NOT emit `Name=""` (cosmetic noise);
        // omit the attribute entirely so SmartPlant's defaulting
        // can take over.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-4".into(),
            item_type_name: "InstrFunction".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IObject UID="INSTR-4"/>"#),
            "missing tag info should produce a bare IObject; out:\n{out}"
        );
        assert!(
            !out.contains(r#"Name="""#),
            "writer must not emit empty Name attributes; out:\n{out}"
        );
    }

    // -----------------------------------------------------------------
    // A16 — derived PIDSignalPort children
    // -----------------------------------------------------------------

    #[test]
    fn instr_function_emits_eight_derived_signal_ports() {
        // SmartPlant always derives 8 `<PIDSignalPort>` children
        // per InstrFunction (verified against
        // DWG-0202GP06-01_Data.xml: 2 InstrFunction × 8 ports = 16).
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-1".into(),
            item_type_name: "InstrFunction".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("MeasuredVariableCode".into(), "LIA".into());
                m.insert("TagSequenceNo".into(), "060201".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert_eq!(
            out.matches("<PIDSignalPort>").count(),
            8,
            "single InstrFunction must derive exactly 8 PIDSignalPort; out:\n{out}"
        );
        // Spot-check first / mid / last derived UIDs.
        for index in [1, 4, 8] {
            assert!(
                out.contains(&format!(
                    r#"<IObject UID="INSTR-1.{index}" Name="{index}"/>"#
                )),
                "derived port {index} must carry UID `INSTR-1.{index}` and Name=\"{index}\"; out:\n{out}"
            );
        }
        // Each derived block carries the full 5-interface skeleton.
        assert!(out.contains("<ISignalConnection/>"));
        assert!(out.contains("<ISignalPort/>"));
        assert!(out.contains("<IFacilityPoint/>"));
        assert!(out.contains("<IPIDTypical/>"));
    }

    #[test]
    fn instrument_alias_also_derives_eight_signal_ports() {
        // Both `Instrument` and `InstrFunction` ItemTypeNames must
        // produce the eight derived ports.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-2".into(),
            item_type_name: "Instrument".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert_eq!(out.matches("<PIDSignalPort>").count(), 8);
        assert!(out.contains(r#"<IObject UID="INSTR-2.1" Name="1"/>"#));
        assert!(out.contains(r#"<IObject UID="INSTR-2.8" Name="8"/>"#));
    }

    #[test]
    fn two_instr_functions_yield_sixteen_distinct_signal_ports() {
        // The DWG fixture's 2 × 8 = 16 ports must round-trip
        // distinct UIDs even when the two InstrFunctions live on
        // the same drawing.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![
            PublishObject {
                uid: "INSTR-A".into(),
                item_type_name: "InstrFunction".into(),
                ..PublishObject::default()
            },
            PublishObject {
                uid: "INSTR-B".into(),
                item_type_name: "InstrFunction".into(),
                ..PublishObject::default()
            },
        ];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert_eq!(
            out.matches("<PIDSignalPort>").count(),
            16,
            "two InstrFunctions must derive 2 × 8 = 16 PIDSignalPort; out:\n{out}"
        );
        for prefix in ["INSTR-A", "INSTR-B"] {
            for index in 1..=8u8 {
                assert!(
                    out.contains(&format!(
                        r#"<IObject UID="{prefix}.{index}" Name="{index}"/>"#
                    )),
                    "expected `{prefix}.{index}` in output",
                );
            }
        }
    }

    #[test]
    fn signal_port_derivation_count_matches_constant() {
        // Pin the constant against the writer's runtime behavior so
        // a future tweak that, say, lowers the count to 4 must
        // update both the constant and this assertion in lockstep.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-C".into(),
            item_type_name: "InstrFunction".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert_eq!(
            out.matches("<PIDSignalPort>").count(),
            INSTR_DERIVED_SIGNAL_PORT_COUNT as usize,
        );
    }

    #[test]
    fn signal_ports_appear_after_their_parent_control_system_function() {
        // SPPID-canonical emit order: the parent
        // <PIDControlSystemFunction> closes BEFORE the first
        // <PIDSignalPort> opens.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-ORD".into(),
            item_type_name: "InstrFunction".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");

        let i_function = out
            .find("<PIDControlSystemFunction>")
            .expect("control system function");
        let i_function_close = out
            .find("</PIDControlSystemFunction>")
            .expect("control system function closer");
        let i_first_port = out.find("<PIDSignalPort>").expect("first signal port");

        assert!(
            i_function < i_function_close && i_function_close < i_first_port,
            "PIDSignalPort must appear after the closer of its parent PIDControlSystemFunction; \
             got function={i_function} close={i_function_close} first_port={i_first_port}\nout:\n{out}"
        );
    }

    #[test]
    fn instr_function_named_instrument_passes_through_optional_fields() {
        // Verify all five INamedInstrument attributes round-trip
        // when the loader populates them (T_PlantItem.TagPrefix /
        // T_Instrument.{TagSuffix, LoopTagSuffix, InstrumentTypeModifier}).
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-5".into(),
            item_type_name: "InstrFunction".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("MeasuredVariableCode".into(), "LIA".into());
                m.insert("TagSequenceNo".into(), "060201".into());
                m.insert("TagPrefix".into(), "PFX".into());
                m.insert("TagSuffix".into(), "SFX".into());
                m.insert("LoopTagSuffix".into(), "LP".into());
                m.insert("InstrumentTypeModifier".into(), "MOD".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        for (attr, value) in [
            ("InstrTagPrefix", "PFX"),
            ("InstrTagSequenceNo", "060201"),
            ("InstrTagSuffix", "SFX"),
            ("InstrLoopSuffix", "LP"),
            ("InstrFuncModifier", "MOD"),
            ("MeasuredVariable", "LIA"),
        ] {
            assert!(
                out.contains(&format!(r#"{attr}="{value}""#)),
                "INamedInstrument must carry `{attr}=\"{value}\"`; out:\n{out}"
            );
        }
    }

    // -----------------------------------------------------------------
    // A13 — connector-derived endpoints
    // -----------------------------------------------------------------

    #[test]
    fn piperun_emits_two_derived_piping_ports_and_one_process_point() {
        // SmartPlant's exporter derives `<PIDPipingPort>.1`,
        // `<PIDPipingPort>.2`, and `<PIDProcessPoint>.PPT` from
        // every PipingConnector. The composition is purely
        // SmartPlant-side — no SQLite row carries those UIDs.
        let mut d = PublishDrawing::new("UID-A01", "A01");
        d.objects = vec![PublishObject {
            uid: "PIPE-1".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("NominalDiameter".into(), "250".into());
                m.insert("PipingMaterialsClass".into(), "B5".into());
                m.insert("TagSequenceNo".into(), "0102".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");

        // The pipeline + connector + 2 ports + 1 process point all
        // fire from this single PipeRun row.
        assert_eq!(
            out.matches("<PIDPipingPort>").count(),
            2,
            "PipeRun must emit exactly 2 derived <PIDPipingPort> nodes; out:\n{out}"
        );
        assert_eq!(
            out.matches("<PIDProcessPoint>").count(),
            1,
            "PipeRun must emit exactly 1 derived <PIDProcessPoint> node; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IObject UID="PIPE-1-CNX.1" Name="1"/>"#),
            "first derived port UID is `<connector>.1` with Name=\"1\"; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IObject UID="PIPE-1-CNX.2" Name="2"/>"#),
            "second derived port UID is `<connector>.2` with Name=\"2\"; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IObject UID="PIPE-1-CNX.PPT"/>"#),
            "derived process point UID is `<connector>.PPT`; out:\n{out}"
        );
        // Both derived ports inherit the connector's nominal
        // diameter (with the `mm` unit applied by format_diameter).
        assert_eq!(
            out.matches(r#"<IPipeCrossSectionItem NominalDiameter="250 mm"/>"#)
                .count(),
            3,
            "two derived ports + the connector itself carry NominalDiameter; out:\n{out}"
        );
    }

    #[test]
    fn piperun_with_no_nominal_diameter_still_derives_three_endpoints() {
        // Even when the upstream row has no diameter, the writer
        // must still derive the three virtual endpoints (with an
        // empty NominalDiameter attribute) so the Rel topology in
        // the eventual `_Data.xml` cross-references can resolve.
        let mut d = PublishDrawing::new("UID-A01", "A01");
        d.objects = vec![PublishObject {
            uid: "PIPE-EMPTY".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert_eq!(out.matches("<PIDPipingPort>").count(), 2);
        assert_eq!(out.matches("<PIDProcessPoint>").count(), 1);
        assert!(out.contains(r#"<IObject UID="PIPE-EMPTY-CNX.1" Name="1"/>"#));
        assert!(out.contains(r#"<IObject UID="PIPE-EMPTY-CNX.PPT"/>"#));
    }

    #[test]
    fn derived_endpoints_appear_after_connector_in_emit_order() {
        // SmartPlant's reference fixture emits the five PipeRun
        // children in this exact order: PIDPipeline,
        // PIDPipingConnector, PIDPipingPort×2, PIDProcessPoint.
        // Tests pin the order so a future refactor cannot quietly
        // swap them.
        let mut d = PublishDrawing::new("UID-A01", "A01");
        d.objects = vec![PublishObject {
            uid: "PIPE-ORD".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");

        let i_pipeline = out.find("<PIDPipeline>").expect("pipeline present");
        let i_connector = out.find("<PIDPipingConnector>").expect("connector present");
        let i_first_port = out.find("<PIDPipingPort>").expect("first port");
        let i_process_point = out
            .find("<PIDProcessPoint>")
            .expect("process point present");

        assert!(
            i_pipeline < i_connector
                && i_connector < i_first_port
                && i_first_port < i_process_point,
            "emit order must be Pipeline < Connector < Port(.1) < ProcessPoint; got positions \
             pipeline={i_pipeline} connector={i_connector} first_port={i_first_port} \
             process_point={i_process_point}\nout:\n{out}"
        );
    }

    // -----------------------------------------------------------------
    // A14 — annotation/label representation filtering
    // -----------------------------------------------------------------

    #[test]
    fn write_representations_emits_one_pid_representation_per_publishable_row() {
        // The reference SmartPlant exporter emits PIDRepresentation
        // ONLY for representations that point at a model item.
        // Three reps wired to model items, two pure annotations
        // (model_item_uid None / Some("")), one with valid uid:
        // expect exactly four `<PIDRepresentation>` opens.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.representations = vec![
            PublishRepresentation {
                uid: "REP-1".into(),
                model_item_uid: Some("OBJ-1".into()),
                drawing_uid: "UID-D".into(),
                graphic_oid: Some(1),
                ..PublishRepresentation::default()
            },
            PublishRepresentation {
                uid: "REP-2-LABEL".into(),
                model_item_uid: None,
                drawing_uid: "UID-D".into(),
                graphic_oid: Some(2),
                ..PublishRepresentation::default()
            },
            PublishRepresentation {
                uid: "REP-3".into(),
                model_item_uid: Some("OBJ-3".into()),
                drawing_uid: "UID-D".into(),
                graphic_oid: Some(3),
                ..PublishRepresentation::default()
            },
            PublishRepresentation {
                uid: "REP-4-EMPTY".into(),
                model_item_uid: Some(String::new()),
                drawing_uid: "UID-D".into(),
                graphic_oid: Some(4),
                ..PublishRepresentation::default()
            },
            PublishRepresentation {
                uid: "REP-5".into(),
                model_item_uid: Some("OBJ-5".into()),
                drawing_uid: "UID-D".into(),
                graphic_oid: None,
                ..PublishRepresentation::default()
            },
        ];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert_eq!(
            out.matches("<PIDRepresentation>").count(),
            3,
            "only the three reps with non-empty model_item_uid should produce <PIDRepresentation>; out:\n{out}"
        );
        // Spot-check the three publishable UIDs are present.
        for uid in ["REP-1", "REP-3", "REP-5"] {
            assert!(
                out.contains(&format!(r#"<IObject UID="{uid}"/>"#)),
                "publishable rep `{uid}` must be present; out:\n{out}"
            );
        }
        // The two annotation-only reps must NOT appear.
        for uid in ["REP-2-LABEL", "REP-4-EMPTY"] {
            assert!(
                !out.contains(&format!(r#"<IObject UID="{uid}"/>"#)),
                "annotation rep `{uid}` must NOT be present; out:\n{out}"
            );
        }
    }

    #[test]
    fn drawing_items_rel_only_emitted_for_publishable_representations() {
        // The DrawingItems derived rel must follow the same filter
        // — otherwise the Rel section would dangle pointers to
        // PIDRepresentation tags we never wrote.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.representations = vec![
            PublishRepresentation {
                uid: "REP-OK".into(),
                model_item_uid: Some("OBJ-1".into()),
                drawing_uid: "UID-D".into(),
                ..PublishRepresentation::default()
            },
            PublishRepresentation {
                uid: "REP-LABEL".into(),
                model_item_uid: None,
                drawing_uid: "UID-D".into(),
                ..PublishRepresentation::default()
            },
        ];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"DefUID="DrawingItems""#),
            "DrawingItems rel for the publishable rep must be present; out:\n{out}"
        );
        // The dangling rel for the annotation rep must NOT exist.
        assert!(
            !out.contains("REP-LABEL"),
            "annotation rep UID must not appear in any DrawingItems / DwgRepresentationComposition rel; out:\n{out}"
        );
    }

    #[test]
    fn representation_is_publishable_classifier_unit_table() {
        // Pure helper test for the predicate that drives both the
        // representation block and the derived rels.
        let publishable = PublishRepresentation {
            uid: "REP".into(),
            model_item_uid: Some("OBJ".into()),
            drawing_uid: "UID".into(),
            ..PublishRepresentation::default()
        };
        let no_model = PublishRepresentation {
            uid: "REP".into(),
            model_item_uid: None,
            drawing_uid: "UID".into(),
            ..PublishRepresentation::default()
        };
        let empty_model = PublishRepresentation {
            uid: "REP".into(),
            model_item_uid: Some(String::new()),
            drawing_uid: "UID".into(),
            ..PublishRepresentation::default()
        };
        assert!(representation_is_publishable(&publishable));
        assert!(!representation_is_publishable(&no_model));
        assert!(!representation_is_publishable(&empty_model));
    }

    #[test]
    fn unsupported_item_types_still_fall_back_to_generic_placeholder() {
        // Exchanger / Mechanical have subtables registered but no
        // dedicated writer yet (TODO A12+); ensure they continue to
        // emit through the generic dispatch instead of being
        // silently dropped.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "EX-1".into(),
            item_type_name: "Exchanger".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains("<PIDItem>"),
            "Exchanger should still hit the generic <PIDItem> fallback; out:\n{out}"
        );
        assert!(
            out.contains("`Exchanger`"),
            "the generic comment should name the unsupported type; out:\n{out}"
        );
    }

    // -----------------------------------------------------------------
    // A17 — PIDPipingComponent writer (PipingComp → full 19-interface block)
    // -----------------------------------------------------------------

    #[test]
    fn piping_comp_emits_pid_piping_component_with_all_interfaces() {
        // Mirrors the `Cap` sample at
        // DWG-0202GP06-01_Data.xml:204–224. Fill the fields with
        // representative SPPID data so every attribute-bearing
        // interface can round-trip end-to-end.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "C5A2865821394E019D7DAA0CAFE0490D".into(),
            item_type_name: "PipingComp".into(),
            is_typical: Some("0".into()),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert(
                    "PipingComponentType1".into(),
                    "@{81CD929C-BC07-11D6-BDBC-00104BCC2B69}".into(),
                );
                m.insert(
                    "PipingComponentType2".into(),
                    "@{81CD9804-BC07-11D6-BDBC-00104BCC2B69}".into(),
                );
                m.insert(
                    "PipingComponentType3".into(),
                    "@{81CD9816-BC07-11D6-BDBC-00104BCC2B69}".into(),
                );
                m.insert("PipModelCode".into(), "Cap".into());
                m.insert(
                    "CommoditySpecialtyType".into(),
                    "@{5F7F8F6E-BC29-11D6-BDBC-00104BCC2B69}".into(),
                );
                m.insert("NominalDiameter".into(), "100".into());
                m.insert("IsFlowDirectional".into(), "0".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert_eq!(
            out.matches("<PIDPipingComponent>").count(),
            1,
            "single PipingComp must open exactly one <PIDPipingComponent>; out:\n{out}"
        );
        // All 19 interface opens in canonical order. Ordering is
        // asserted via increasing `find` positions below.
        for needle in [
            r#"<IObject UID="C5A2865821394E019D7DAA0CAFE0490D"/>"#,
            r#"<IPBSItem ConstructionStatus="@NewConstruction" ConstructionStatus2="@{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}"/>"#,
            "<IPipingPortComposition/>",
            "<IPlannedMatl/>",
            "<IDrawingItem/>",
            r#"PipingComponentType1="@{81CD929C-BC07-11D6-BDBC-00104BCC2B69}""#,
            r#"PipingComponentType3="@{81CD9816-BC07-11D6-BDBC-00104BCC2B69}""#,
            r#"PipingComponentType2="@{81CD9804-BC07-11D6-BDBC-00104BCC2B69}""#,
            r#"PipModelCode="Cap""#,
            r#"CommoditySpecialtyType="@{5F7F8F6E-BC29-11D6-BDBC-00104BCC2B69}""#,
            "<IPipingComponentOcc/>",
            "<IInlineComponentOcc/>",
            "<IFabricatedItem/>",
            r#"<IHeatTracedItem HTraceRqmt=""/>"#,
            "<IPartOcc/>", // Cap sample ships no CatalogPartNumber.
            "<IPressureReliefItem/>",
            "<IDocumentItem/>",
            "<IElecPowerConsumer/>",
            "<IPart/>",
            "<INoteCollection/>",
            r#"<IPipeCrossSectionItem NominalDiameter="100 mm"/>"#,
            r#"<IInlineComponent IsFlowDirectional="False"/>"#,
            r#"<IPIDTypical IsTypical="False"/>"#,
        ] {
            assert!(
                out.contains(needle),
                "PIDPipingComponent block must carry `{needle}`; out:\n{out}"
            );
        }
    }

    #[test]
    fn piping_comp_with_catalog_part_number_renders_valve_variant() {
        // Mirrors the `Conduit gate valve` sample at
        // DWG-0202GP06-01_Data.xml:225–245, which carries the
        // optional `CatalogPartNumber="A3"` attribute on
        // <IPartOcc>. Verifies the conditional path doesn't drop
        // it when present.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "6CC683FA4C6A409D8CB4D3F22BBE194E".into(),
            item_type_name: "PipingComp".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("PipModelCode".into(), "Conduit gate valve".into());
                m.insert("NominalDiameter".into(), "80".into());
                m.insert("CatalogPartNumber".into(), "A3".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IPartOcc CatalogPartNumber="A3"/>"#),
            "<IPartOcc> must carry CatalogPartNumber when the field is set; out:\n{out}"
        );
        assert!(
            !out.contains("<IPartOcc/>"),
            "empty <IPartOcc/> must NOT appear when CatalogPartNumber is set; out:\n{out}"
        );
        assert!(
            out.contains(r#"PipModelCode="Conduit gate valve""#),
            "valve sample's PipModelCode must round-trip; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IPipeCrossSectionItem NominalDiameter="80 mm"/>"#),
            "80-mm diameter must acquire the mm suffix; out:\n{out}"
        );
    }

    #[test]
    fn piping_comp_with_empty_fields_still_opens_pid_piping_component() {
        // A PipingComp row with no business-subtable fields — i.e.
        // a fixture where the loader found T_ModelItem but the
        // companion tables are missing — must still produce a
        // syntactically complete block so downstream validators
        // don't choke. Optional attributes render empty.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "PC-BARE".into(),
            item_type_name: "PipingComp".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert_eq!(out.matches("<PIDPipingComponent>").count(), 1);
        assert!(
            out.contains(r#"<IObject UID="PC-BARE"/>"#),
            "bare PipingComp must still carry its UID; out:\n{out}"
        );
        // With no PipingComponentType* columns, the attributes are
        // empty strings — still present, still well-formed.
        assert!(
            out.contains(r#"PipingComponentType1="" PipingComponentType3="" PipingComponentType2="" PipModelCode="" CommoditySpecialtyType="""#),
            "bare PipingComp must emit empty PipingComponentType*/PipModelCode/CommoditySpecialtyType attributes; out:\n{out}"
        );
        assert!(
            out.contains("<IPartOcc/>"),
            "bare PipingComp must default to empty <IPartOcc/>; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IPipeCrossSectionItem NominalDiameter=""/>"#),
            "bare PipingComp must still emit <IPipeCrossSectionItem NominalDiameter=\"\"/>; out:\n{out}"
        );
        // Default booleans must resolve to False via map_bool.
        assert!(
            out.contains(r#"<IInlineComponent IsFlowDirectional="False"/>"#),
            "default IsFlowDirectional must be `False`; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IPIDTypical IsTypical="False"/>"#),
            "default IsTypical must be `False`; out:\n{out}"
        );
    }

    #[test]
    fn piping_comp_maps_is_flow_directional_true() {
        // A fixture where T_InlineComp.IsFlowDirectional is `"1"`
        // (SPPID boolean true) must surface as `IsFlowDirectional="True"`
        // via map_bool. Pins the mapping so a future refactor of
        // map_bool's call sites cannot silently degrade the
        // PipingComponent attribute.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "PC-TRUE".into(),
            item_type_name: "PipingComp".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("IsFlowDirectional".into(), "1".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IInlineComponent IsFlowDirectional="True"/>"#),
            "PipingComp with IsFlowDirectional=1 must emit `True`; out:\n{out}"
        );
    }

    #[test]
    fn two_piping_comps_yield_two_distinct_pid_piping_component_blocks() {
        // The DWG backlog row pins count=2 for PIDPipingComponent;
        // this test locks the per-row cardinality so a future
        // refactor cannot accidentally collapse them into one.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![
            PublishObject {
                uid: "PC-A".into(),
                item_type_name: "PipingComp".into(),
                ..PublishObject::default()
            },
            PublishObject {
                uid: "PC-B".into(),
                item_type_name: "PipingComp".into(),
                ..PublishObject::default()
            },
        ];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert_eq!(
            out.matches("<PIDPipingComponent>").count(),
            2,
            "two PipingComp rows must open two <PIDPipingComponent> blocks; out:\n{out}"
        );
        assert!(out.contains(r#"<IObject UID="PC-A"/>"#));
        assert!(out.contains(r#"<IObject UID="PC-B"/>"#));
    }

    #[test]
    fn piping_comp_emits_interfaces_in_sppid_canonical_order() {
        // SPPID emits the 19 interfaces in a fixed order (confirmed
        // against both DWG samples at lines 204–224 and 225–245).
        // Pin that order so a future refactor cannot shuffle them.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "PC-ORD".into(),
            item_type_name: "PipingComp".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");

        let positions = [
            "<PIDPipingComponent>",
            r#"<IObject UID="PC-ORD"/>"#,
            "<IPBSItem ConstructionStatus=",
            "<IPipingPortComposition/>",
            "<IPlannedMatl/>",
            "<IDrawingItem/>",
            "<IPipingComponent ",
            "<IPipingComponentOcc/>",
            "<IInlineComponentOcc/>",
            "<IFabricatedItem/>",
            "<IHeatTracedItem ",
            "<IPartOcc/>",
            "<IPressureReliefItem/>",
            "<IDocumentItem/>",
            "<IElecPowerConsumer/>",
            "<IPart/>",
            "<INoteCollection/>",
            "<IPipeCrossSectionItem ",
            "<IInlineComponent ",
            "<IPIDTypical ",
            "</PIDPipingComponent>",
        ];
        let mut last_pos = 0usize;
        for needle in positions {
            let pos = out[last_pos..].find(needle).unwrap_or_else(|| {
                panic!("missing `{needle}` after offset {last_pos}\nout:\n{out}")
            });
            last_pos += pos + needle.len();
        }
    }

    // -----------------------------------------------------------------
    // A18 — PIDSignalConnector writer (SignalRun → 8-interface block)
    // -----------------------------------------------------------------

    #[test]
    fn signal_run_emits_pid_signal_connector_with_all_interfaces() {
        // Mirrors the reference sample at
        // DWG-0202GP06-01_Data.xml:1111–1120. A bare SignalRun
        // row must open every one of the 8 interfaces.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "E871304702F74D39B15BD2D8B41D34B3".into(),
            item_type_name: "SignalRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert_eq!(
            out.matches("<PIDSignalConnector>").count(),
            1,
            "single SignalRun must open exactly one <PIDSignalConnector>; out:\n{out}"
        );
        for needle in [
            r#"<IObject UID="E871304702F74D39B15BD2D8B41D34B3"/>"#,
            "<IPlannedFacility/>",
            r#"<IConnector FlowDirection=""/>"#,
            "<IDrawingItem/>",
            "<ISignalConnector/>",
            "<IDocumentItem/>",
            "<IExpandableThing/>",
            r#"<IPIDTypical IsTypical="False"/>"#,
            "</PIDSignalConnector>",
        ] {
            assert!(
                out.contains(needle),
                "PIDSignalConnector block must carry `{needle}`; out:\n{out}"
            );
        }
    }

    #[test]
    fn signal_run_propagates_populated_flow_direction() {
        // When a future loader learns to populate FlowDirection
        // from the appropriate T_* column, the writer must
        // surface it on <IConnector> instead of forcing the
        // empty-string default. This test pins the passthrough
        // path so the upgrade is a pure loader-side change.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "SR-FLOW".into(),
            item_type_name: "SignalRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("FlowDirection".into(), "@EE872".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IConnector FlowDirection="@EE872"/>"#),
            "populated FlowDirection must round-trip onto <IConnector>; out:\n{out}"
        );
    }

    #[test]
    fn signal_run_emits_no_piping_interfaces() {
        // PIDSignalConnector is deliberately minimal compared to
        // PIDPipingConnector — no IPBSItem envelope, no piping-
        // specific interfaces. Pin that contrast so a well-meaning
        // refactor that tries to share a common "connector"
        // writer doesn't accidentally inject piping-only
        // interfaces into the signal shape.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "SR-MIN".into(),
            item_type_name: "SignalRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        // Slice the output between `<PIDSignalConnector>` and
        // `</PIDSignalConnector>` so we only inspect the signal
        // block, not the rest of the document (Rel nodes etc.
        // may legitimately mention piping-adjacent strings).
        let open = out.find("<PIDSignalConnector>").expect("open");
        let close = out.find("</PIDSignalConnector>").expect("close");
        let block = &out[open..=close + "</PIDSignalConnector>".len()];
        for forbidden in [
            "IPBSItem",
            "IPipingConnector",
            "INamedPipingConnector",
            "IPipeCrossSectionItem",
            "IPipingSpecifiedItem",
            "IPipingPort",
        ] {
            assert!(
                !block.contains(forbidden),
                "PIDSignalConnector must NOT contain `{forbidden}`; block:\n{block}"
            );
        }
    }

    #[test]
    fn signal_run_maps_is_typical_true() {
        // IsTypical="True" on a SignalRun must round-trip via
        // map_bool when T_ModelItem.SP_IsTypical is `"1"`.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "SR-TYP".into(),
            item_type_name: "SignalRun".into(),
            is_typical: Some("1".into()),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IPIDTypical IsTypical="True"/>"#),
            "SignalRun with IsTypical=1 must emit `True`; out:\n{out}"
        );
    }

    #[test]
    fn signal_run_emits_interfaces_in_sppid_canonical_order() {
        // Pin the 8-interface canonical order observed in the DWG
        // reference fixture.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "SR-ORD".into(),
            item_type_name: "SignalRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        let positions = [
            "<PIDSignalConnector>",
            r#"<IObject UID="SR-ORD"/>"#,
            "<IPlannedFacility/>",
            "<IConnector ",
            "<IDrawingItem/>",
            "<ISignalConnector/>",
            "<IDocumentItem/>",
            "<IExpandableThing/>",
            "<IPIDTypical ",
            "</PIDSignalConnector>",
        ];
        let mut last_pos = 0usize;
        for needle in positions {
            let pos = out[last_pos..].find(needle).unwrap_or_else(|| {
                panic!("missing `{needle}` after offset {last_pos}\nout:\n{out}")
            });
            last_pos += pos + needle.len();
        }
    }

    // -----------------------------------------------------------------
    // A19 — PIDPipeline fidelity upgrade (add 4 missing interfaces +
    // FluidCode / FluidSystem attribute routing)
    // -----------------------------------------------------------------

    #[test]
    fn pipeline_emits_full_ten_interface_block_matching_reference() {
        // A01_Data.xml:54–65 and DWG-0202GP06-01_Data.xml:1369–1380
        // both ship the same 10-interface shape. Pin it end-to-end
        // so a future refactor that drops one of the wrapper
        // interfaces (IPBSItem / IPlannedFacility / IPBSItemCollection
        // / INoteCollection) will trip this assertion immediately.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "PIPE-F".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        for needle in [
            "<PIDPipeline>",
            "<IObject UID=",
            "<IPBSItem/>",
            "<IPlannedFacility/>",
            "<IPBSItemCollection/>",
            "<IPipeline/>",
            "<IPipingConnectorComposition/>",
            "<IFluidSystem",
            "<INoteCollection/>",
            "<IExpandableThing/>",
            "<IPIDTypical/>",
            "</PIDPipeline>",
        ] {
            assert!(
                out.contains(needle),
                "PIDPipeline block must carry `{needle}`; out:\n{out}"
            );
        }
    }

    #[test]
    fn pipeline_fluid_system_attrs_route_from_loader_fields() {
        // When the loader populates OperFluidCode + FluidSystem
        // from T_PipeRun, the writer surfaces them on
        // <IFluidSystem FluidCode="..." FluidSystem="..."/>. This
        // test locks that routing so a loader-side upgrade to
        // stamp those columns becomes immediately visible in the
        // emitted XML.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "PIPE-FS".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert(
                    "OperFluidCode".into(),
                    "@{63A6FC56-CB92-402D-8D92-BF9E2F204CE4}".into(),
                );
                m.insert(
                    "FluidSystem".into(),
                    "@{104E7730-99EF-49C6-A928-D8CD78394381}".into(),
                );
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(
                r#"<IFluidSystem FluidCode="@{63A6FC56-CB92-402D-8D92-BF9E2F204CE4}" FluidSystem="@{104E7730-99EF-49C6-A928-D8CD78394381}"/>"#
            ),
            "<IFluidSystem> must carry populated FluidCode + FluidSystem; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_fluid_system_attrs_default_to_empty_when_absent() {
        // DWG declares the FluidCode / FluidSystem attributes even
        // when the loader has not populated values; A01 keeps the
        // bare `<IFluidSystem/>` shape.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "PIPE-E".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IFluidSystem/>"#),
            "empty fluid fields should render the bare IFluidSystem shape; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_name_attribute_added_when_pipeline_name_field_present() {
        // DWG fixtures like `<IObject Name="A3jqz0101-OD" .../>`
        // stamp a human-readable pipeline name. When the loader
        // populates obj.fields["PipelineName"] the writer surfaces
        // it between UID and ItemTag on the IObject.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "PIPE-N".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("PipelineName".into(), "A3jqz0101-OD".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(
                r#"<IObject UID="PIPE-N" Name="A3jqz0101-OD" ItemTag=""#
            ),
            "Name attribute must appear between UID and ItemTag when PipelineName is populated; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_name_attribute_omitted_when_pipeline_name_empty() {
        // A01 fixtures do not ship PipelineName; the writer must
        // omit the Name="" attribute entirely to keep the IObject
        // element compact.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "PIPE-NONAME".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            !out.contains(r#"<IObject UID="PIPE-NONAME" Name="""#),
            "empty PipelineName must not emit a Name=\"\" attribute; out:\n{out}"
        );
        // The UID + ItemTag shape must still be intact.
        assert!(
            out.contains(r#"<IObject UID="PIPE-NONAME" ItemTag="#),
            "IObject must carry UID + ItemTag even without Name; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_emits_interfaces_in_sppid_canonical_order() {
        // Pin the 10-interface canonical order end-to-end via
        // find() cursor so a reorder would trip the assertion at
        // the exact problematic needle.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "PIPE-ORD".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        let positions = [
            "<PIDPipeline>",
            "<IObject UID=",
            "<IPBSItem/>",
            "<IPlannedFacility/>",
            "<IPBSItemCollection/>",
            "<IPipeline/>",
            "<IPipingConnectorComposition/>",
            "<IFluidSystem",
            "<INoteCollection/>",
            "<IExpandableThing/>",
            "<IPIDTypical/>",
            "</PIDPipeline>",
        ];
        let mut last_pos = 0usize;
        for needle in positions {
            let pos = out[last_pos..].find(needle).unwrap_or_else(|| {
                panic!("missing `{needle}` after offset {last_pos}\nout:\n{out}")
            });
            last_pos += pos + needle.len();
        }
    }

    // -----------------------------------------------------------------
    // A20 — PIDPipingConnector fidelity upgrade (7 → 22 interfaces,
    // + optional DWG-style attribute routing)
    // -----------------------------------------------------------------

    #[test]
    fn piping_connector_emits_full_twenty_two_interface_block() {
        // A01_Data.xml:66–89 ships this 22-interface shape bare.
        // Pin every wrapper interface so a regression that drops
        // one trips immediately.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "PIPE-F".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        for needle in [
            "<PIDPipingConnector>",
            "<IObject UID=",
            "<IPBSItem ConstructionStatus=",
            "<IPlannedFacility/>",
            "<IConnector/>",
            "<IDrawingItem/>",
            "<IPBSItemCollection/>",
            "<IPipingConnector/>",
            "<IFabricatedItem/>",
            "<IHeatTracedItem ",
            "<IProcessPointCollection/>",
            "<IDocumentItem/>",
            "<IElecPowerConsumer/>",
            "<INoteCollection/>",
            "<IProcessDataCaseComposition/>",
            "<IExpandableThing/>",
            "<INamedPipingConnector ",
            "<IPipeCrossSectionItem ",
            "<IPipingSpecifiedItem ",
            "<ISlopedPipingItem/>",
            "<IInsulatedItem/>",
            "<IJacketedItem/>",
            "<IPIDTypical ",
            "</PIDPipingConnector>",
        ] {
            assert!(
                out.contains(needle),
                "PIDPipingConnector block must carry `{needle}`; out:\n{out}"
            );
        }
    }

    #[test]
    fn piping_connector_defaults_match_a01_reference_construction_status() {
        // A01 ships `<IPBSItem ConstructionStatus="@NewConstruction"
        // ConstructionStatus2="@{78398AB4-...}"/>`; our writer must
        // emit the same canonical defaults when the loader has
        // not stamped the ConstructionStatus field.
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "PIPE-CON".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(
                r#"<IPBSItem ConstructionStatus="@NewConstruction" ConstructionStatus2="@{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}"/>"#
            ),
            "A01-canonical IPBSItem defaults must land on PIDPipingConnector; out:\n{out}"
        );
    }

    #[test]
    fn piping_connector_optional_attrs_render_bare_when_absent() {
        // A01 fixture's PIDPipingConnector ships bare
        // <IConnector/>, <IPipingConnector/>, <ISlopedPipingItem/>,
        // <IInsulatedItem/> — no attributes. Pin the bare paths so
        // they do not accidentally emit empty-attribute versions
        // (which would still parse but diverge from A01 bytes).
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "PIPE-BARE".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        for bare in [
            "<IConnector/>",
            "<IPipingConnector/>",
            "<ISlopedPipingItem/>",
            "<IInsulatedItem/>",
        ] {
            assert!(
                out.contains(bare),
                "A01 shape requires bare `{bare}`; out:\n{out}"
            );
        }
    }

    #[test]
    fn piping_connector_populates_optional_attrs_when_loader_supplies_them() {
        // DWG shape populates IConnector / IPipingConnector /
        // ISlopedPipingItem / IInsulatedItem with attributes. A20
        // routes those fields so a future loader-side upgrade
        // becomes immediately visible in the emitted XML.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "PIPE-DWG".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("FlowDirection".into(), "@EE872".into());
                m.insert("RepresentationsAreAllZeroLength".into(), "0".into());
                m.insert("PipingConnectorType".into(), "@EE690".into());
                m.insert("SlopedPipingAngle".into(), "2.9999910000486E-03 rad".into());
                m.insert(
                    "SlopedPipeDirection".into(),
                    "@{FAC6E20B-6B3C-48C4-BEE8-409B224925C2}".into(),
                );
                m.insert(
                    "InsulThickSrc".into(),
                    "@{1B53D013-9B24-11D6-BDA4-00104BCC2B69}".into(),
                );
                m.insert("TotalInsulThick".into(), "50 mm".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(out.contains(
            r#"<IConnector FlowDirection="@EE872" RepresentationsAreAllZeroLength="False"/>"#
        ));
        assert!(out.contains(r#"<IPipingConnector PipingConnectorType="@EE690"/>"#));
        assert!(out.contains(
            r#"<ISlopedPipingItem SlopedPipingAngle="2.9999910000486E-03 rad" SlopedPipeDirection="@{FAC6E20B-6B3C-48C4-BEE8-409B224925C2}"/>"#
        ));
        assert!(out.contains(
            r#"<IInsulatedItem InsulThickSrc="@{1B53D013-9B24-11D6-BDA4-00104BCC2B69}" TotalInsulThick="50 mm"/>"#
        ));
    }

    #[test]
    fn piping_connector_uses_name_attribute_on_dwg_shape() {
        // A29 makes the IObject shape an explicit decision via
        // [`PublishStyle`]. Pre-A29 the connector flipped to
        // Name-style as soon as `obj.fields["PipelineName"]` was
        // populated (an implicit, data-driven flip). Post-A29 the
        // caller must opt in via `drawing.style = Dwg` for the
        // DWG-shape IObject (`UID + Name`); the A01 shape
        // (`UID + ItemTag`) remains the default.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "PIPE-N".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert(
                    "PipelineName".into(),
                    "A3jqz0101-OD-100 mm-1.6AR12-WE-50mm".into(),
                );
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IObject UID="PIPE-N-CNX" Name="A3jqz0101-OD-100 mm-1.6AR12-WE-50mm"/>"#),
            "DWG-style PipingConnector with PipelineName populated must emit UID + Name IObject; out:\n{out}"
        );
        // The A01-shape ItemTag must NOT appear when we've
        // switched to the DWG shape.
        assert!(
            !out.contains(r#"<IObject UID="PIPE-N-CNX" ItemTag="#),
            "Name path must suppress the A01-shape ItemTag IObject; out:\n{out}"
        );
    }

    #[test]
    fn piping_connector_named_prefix_and_suffix_route_from_tag_columns() {
        // <INamedPipingConnector> needs all three SPPID tag
        // columns (TagPrefix / TagSequenceNo / TagSuffix) to
        // round-trip. A01 stamps only the sequence; DWG stamps
        // only the sequence. Pin both paths: prefix and suffix
        // remain empty when absent but populate when the loader
        // has them, so future fixtures with tag-prefixed
        // connectors continue working without further code
        // changes.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "PIPE-PF".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("TagPrefix".into(), "PFX".into());
                m.insert("TagSequenceNo".into(), "0101".into());
                m.insert("TagSuffix".into(), "SFX".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<INamedPipingConnector PipingConnectorPrefix="PFX" PipingConnectorSeqNo="0101" PipingConnectorSuff="SFX"/>"#),
            "all three tag columns must round-trip; out:\n{out}"
        );
    }

    #[test]
    fn piping_connector_emits_interfaces_in_sppid_canonical_order() {
        // Pin the 22-interface canonical order via find() cursor.
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "PIPE-O".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        let positions = [
            "<PIDPipingConnector>",
            "<IObject UID=",
            "<IPBSItem ",
            "<IPlannedFacility/>",
            "<IConnector",
            "<IDrawingItem/>",
            "<IPBSItemCollection/>",
            "<IPipingConnector",
            "<IFabricatedItem/>",
            "<IHeatTracedItem ",
            "<IProcessPointCollection/>",
            "<IDocumentItem/>",
            "<IElecPowerConsumer/>",
            "<INoteCollection/>",
            "<IProcessDataCaseComposition/>",
            "<IExpandableThing/>",
            "<INamedPipingConnector ",
            "<IPipeCrossSectionItem ",
            "<IPipingSpecifiedItem ",
            "<ISlopedPipingItem",
            "<IInsulatedItem",
            "<IJacketedItem/>",
            "<IPIDTypical ",
            "</PIDPipingConnector>",
        ];
        let mut last_pos = 0usize;
        for needle in positions {
            let pos = out[last_pos..].find(needle).unwrap_or_else(|| {
                panic!("missing `{needle}` after offset {last_pos}\nout:\n{out}")
            });
            last_pos += pos + needle.len();
        }
    }

    // -----------------------------------------------------------------
    // A21 — PIDProcessVessel fidelity upgrade (10 → 15 interfaces,
    // + DWG-style attribute routing on IEquipment / IPBSItem /
    // IProcessVessel / ISpecifiedMatlItem)
    // -----------------------------------------------------------------

    #[test]
    fn process_vessel_emits_full_fifteen_interface_block() {
        // Reference A01_Data.xml:12–28 ships 15 interfaces. Pin
        // every wrapper so a regression that drops one of the
        // five A21-added interfaces (IPBSItemCollection,
        // IPlannedMatl, IProcessEquipmentOcc, IDrawingItem,
        // ISpecifiedMatlItem) trips the assertion immediately.
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "V-F".into(),
            item_type_name: "Vessel".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        for needle in [
            "<PIDProcessVessel>",
            "<IObject UID=",
            "<IPIDProcessVesselOcc/>",
            "<IProcessVesselOcc/>",
            "<IEquipment ",
            "<IEquipmentOcc/>",
            "<IPBSItem ",
            "<IPBSItemCollection/>",
            "<IPlannedMatl/>",
            "<IProcessEquipment/>",
            "<IProcessEquipmentOcc/>",
            "<IProcessVessel/>",
            "<IDrawingItem/>",
            "<IPIDProcessVessel/>",
            "<ISpecifiedMatlItem/>",
            "<IPIDTypical ",
            "</PIDProcessVessel>",
        ] {
            assert!(
                out.contains(needle),
                "PIDProcessVessel block must carry `{needle}`; out:\n{out}"
            );
        }
    }

    #[test]
    fn process_vessel_ipbsitem_uses_canonical_defaults_when_height_absent() {
        // A01 fixture emits `<IPBSItem ConstructionStatus=
        // "@NewConstruction" ConstructionStatus2="@{78398AB4-...}"/>`
        // with no HeightRelativeToGrade. A21's writer must
        // reproduce that exact two-attribute form; adding
        // HeightRelativeToGrade would diverge from A01 bytes.
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "V-DEF".into(),
            item_type_name: "Vessel".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(
                r#"<IPBSItem ConstructionStatus="@NewConstruction" ConstructionStatus2="@{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}"/>"#
            ),
            "A01-canonical IPBSItem defaults (no HeightRelativeToGrade) must land; out:\n{out}"
        );
    }

    #[test]
    fn process_vessel_ipbsitem_includes_height_when_populated() {
        // DWG fixture emits
        // `<IPBSItem HeightRelativeToGrade="3 m" ConstructionStatus="..."
        // ConstructionStatus2="..."/>`. A21 must populate the
        // attribute when the loader supplies it.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "V-H".into(),
            item_type_name: "Vessel".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("HeightRelativeToGrade".into(), "3 m".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(
                r#"<IPBSItem HeightRelativeToGrade="3 m" ConstructionStatus="@NewConstruction" ConstructionStatus2="@{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}"/>"#
            ),
            "populated HeightRelativeToGrade must precede the two default attrs; out:\n{out}"
        );
    }

    #[test]
    fn process_vessel_iequipment_expands_with_eqtype_attrs() {
        // DWG ships the expanded IEquipment form with EqType0-3 +
        // EquipmentTrimSpec. A21 routes those fields so the DWG
        // shape round-trips when the loader stamps them.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "V-EQ".into(),
            item_type_name: "Vessel".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert(
                    "EqType0".into(),
                    "@{47BF0267-DD41-4E1A-9B41-C4B714C8FF92}".into(),
                );
                m.insert(
                    "EqType3".into(),
                    "@{9B3ED983-16AE-4AD7-A19F-A337149DF437}".into(),
                );
                m.insert("EqType2".into(), "@EE7A6".into());
                m.insert("EqType1".into(), "@EE793".into());
                m.insert("EquipmentTrimSpec".into(), "1.6AR12".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        // The expanded shape pins the DWG-canonical attribute
        // order: EqType0 / EqType3 / EqType2 / EqType1 /
        // EquipmentTrimSpec / EqTypeDescription.
        assert!(
            out.contains(
                r#"<IEquipment EqType0="@{47BF0267-DD41-4E1A-9B41-C4B714C8FF92}" EqType3="@{9B3ED983-16AE-4AD7-A19F-A337149DF437}" EqType2="@EE7A6" EqType1="@EE793" EquipmentTrimSpec="1.6AR12" EqTypeDescription="#
            ),
            "populated EqType0-3 + TrimSpec must expand IEquipment shape; out:\n{out}"
        );
    }

    #[test]
    fn process_vessel_iprocessvessel_includes_volumetric_capacity() {
        // DWG pairs `<IProcessVessel ProcessVessel_VesselVolumetricCapacity="27 m^3"/>`
        // when the loader stamps that column. A01 emits the bare
        // form `<IProcessVessel/>`.
        let mut d_a01 = PublishDrawing::new("UID-D", "A01");
        d_a01.objects = vec![PublishObject {
            uid: "V-A".into(),
            item_type_name: "Vessel".into(),
            ..PublishObject::default()
        }];
        let out_a01 = write_data_xml(&d_a01, "TEST02").expect("write");
        assert!(
            out_a01.contains("<IProcessVessel/>"),
            "A01 shape requires bare <IProcessVessel/>; out:\n{out_a01}"
        );
        let mut d_dwg = PublishDrawing::new("UID-D", "DWG");
        d_dwg.objects = vec![PublishObject {
            uid: "V-D".into(),
            item_type_name: "Vessel".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("VesselVolumetricCapacity".into(), "27 m^3".into());
                m
            },
            ..PublishObject::default()
        }];
        let out_dwg = write_data_xml(&d_dwg, "TEST02").expect("write");
        assert!(
            out_dwg
                .contains(r#"<IProcessVessel ProcessVessel_VesselVolumetricCapacity="27 m^3"/>"#),
            "populated VesselVolumetricCapacity must expand <IProcessVessel>; out:\n{out_dwg}"
        );
    }

    #[test]
    fn process_vessel_ispecifiedmatlitem_gains_long_material_description() {
        // DWG pairs `<ISpecifiedMatlItem LongMaterialDescription="新建"/>`
        // — round-trip the Chinese text through XML escaping.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "V-MAT".into(),
            item_type_name: "Vessel".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("LongMaterialDescription".into(), "新建".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<ISpecifiedMatlItem LongMaterialDescription="新建"/>"#),
            "populated LongMaterialDescription must land with Chinese text intact; out:\n{out}"
        );
    }

    #[test]
    fn process_vessel_emits_interfaces_in_sppid_canonical_order() {
        // Pin the 15-interface canonical order via find() cursor.
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "V-ORD".into(),
            item_type_name: "Vessel".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        let positions = [
            "<PIDProcessVessel>",
            "<IObject UID=",
            "<IPIDProcessVesselOcc/>",
            "<IProcessVesselOcc/>",
            "<IEquipment ",
            "<IEquipmentOcc/>",
            "<IPBSItem ",
            "<IPBSItemCollection/>",
            "<IPlannedMatl/>",
            "<IProcessEquipment/>",
            "<IProcessEquipmentOcc/>",
            "<IProcessVessel",
            "<IDrawingItem/>",
            "<IPIDProcessVessel/>",
            "<ISpecifiedMatlItem",
            "<IPIDTypical ",
            "</PIDProcessVessel>",
        ];
        let mut last_pos = 0usize;
        for needle in positions {
            let pos = out[last_pos..].find(needle).unwrap_or_else(|| {
                panic!("missing `{needle}` after offset {last_pos}\nout:\n{out}")
            });
            last_pos += pos + needle.len();
        }
    }

    // -----------------------------------------------------------------
    // A25 — PIDProcessVessel low-pressure-tank variant emission.
    // DWG-style "Open top tank" vessel variants emit two extra
    // interfaces (ILowPressureTankOcc + ILowPressureTank) between
    // IPIDProcessVessel and ISpecifiedMatlItem. The writer routes
    // the conditional emission off `obj.fields["IsLowPressureTank"]`
    // using `map_bool` for truthy evaluation so explicit False / 0
    // / empty stays in the non-tank branch.
    // -----------------------------------------------------------------

    #[test]
    fn process_vessel_omits_tank_interfaces_by_default() {
        // Pre-A25 contract must hold: with no IsLowPressureTank
        // signal, the writer emits the 15-interface A01 shape and
        // NEVER inserts ILowPressureTank[Occ] (regression guard).
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "V-NONTANK".into(),
            item_type_name: "Vessel".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            !out.contains("<ILowPressureTank"),
            "default vessel should not emit any ILowPressureTank-prefixed interface, got:\n{out}"
        );
    }

    #[test]
    fn process_vessel_omits_tank_interfaces_when_flag_is_explicit_false() {
        // Explicit "False" / "0" / "" must behave the same as
        // the absent case — no tank interfaces.
        for falsey in ["False", "false", "0", ""] {
            let mut d = PublishDrawing::new("UID-D", "A01");
            d.objects = vec![PublishObject {
                uid: "V-FLAG".into(),
                item_type_name: "Vessel".into(),
                fields: std::collections::BTreeMap::from([(
                    "IsLowPressureTank".to_string(),
                    falsey.to_string(),
                )]),
                ..PublishObject::default()
            }];
            let out = write_data_xml(&d, "TEST02").expect("write");
            assert!(
                !out.contains("<ILowPressureTank"),
                "falsey flag `{falsey}` must not emit tank interfaces, got:\n{out}"
            );
        }
    }

    #[test]
    fn process_vessel_emits_tank_interfaces_when_flag_is_true() {
        // Truthy signals must trigger both interfaces in
        // canonical (ILowPressureTankOcc before ILowPressureTank)
        // order, inserted AFTER IPIDProcessVessel and BEFORE
        // ISpecifiedMatlItem to match the DWG byte shape.
        for truthy in ["True", "true", "1"] {
            let mut d = PublishDrawing::new("UID-D", "DWG");
            d.objects = vec![PublishObject {
                uid: "V-TANK".into(),
                item_type_name: "Vessel".into(),
                fields: std::collections::BTreeMap::from([(
                    "IsLowPressureTank".to_string(),
                    truthy.to_string(),
                )]),
                ..PublishObject::default()
            }];
            let out = write_data_xml(&d, "TEST02").expect("write");
            assert!(
                out.contains("<ILowPressureTankOcc/>"),
                "truthy flag `{truthy}` must emit ILowPressureTankOcc, got:\n{out}"
            );
            assert!(
                out.contains("<ILowPressureTank/>"),
                "truthy flag `{truthy}` must emit ILowPressureTank, got:\n{out}"
            );
            let occ_pos = out.find("<ILowPressureTankOcc/>").unwrap();
            let tank_pos = out.find("<ILowPressureTank/>").unwrap();
            assert!(
                occ_pos < tank_pos,
                "flag `{truthy}`: ILowPressureTankOcc must come before ILowPressureTank, got Occ@{occ_pos} vs Tank@{tank_pos}",
            );
            let pidv_pos = out.find("<IPIDProcessVessel/>").unwrap();
            let spec_pos = out.find("<ISpecifiedMatlItem").unwrap();
            assert!(
                pidv_pos < occ_pos && tank_pos < spec_pos,
                "flag `{truthy}`: tank interfaces must slot between IPIDProcessVessel and ISpecifiedMatlItem, got PIDv@{pidv_pos} Occ@{occ_pos} Tank@{tank_pos} Spec@{spec_pos}",
            );
        }
    }

    #[test]
    fn process_vessel_tank_variant_emits_seventeen_interface_block() {
        // Pin the tank variant's full 17-interface canonical
        // order via find() cursor, mirroring the DWG fixture
        // sample at lines 1429–1447.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "V-TANK-ORD".into(),
            item_type_name: "Vessel".into(),
            description: Some("污水池".into()),
            fields: std::collections::BTreeMap::from([
                ("IsLowPressureTank".to_string(), "True".to_string()),
                (
                    "EqType0".to_string(),
                    "@{47BF0267-DD41-4E1A-9B41-C4B714C8FF92}".to_string(),
                ),
                ("EqType1".to_string(), "@EE793".to_string()),
                ("EqType2".to_string(), "@EE7A6".to_string()),
                (
                    "EqType3".to_string(),
                    "@{9B3ED983-16AE-4AD7-A19F-A337149DF437}".to_string(),
                ),
                ("EquipmentTrimSpec".to_string(), "1.6AR12".to_string()),
                ("HeightRelativeToGrade".to_string(), "3 m".to_string()),
                ("VesselVolumetricCapacity".to_string(), "27 m^3".to_string()),
                ("LongMaterialDescription".to_string(), "新建".to_string()),
            ]),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        let positions = [
            "<PIDProcessVessel>",
            "<IObject UID=",
            "<IPIDProcessVesselOcc/>",
            "<IProcessVesselOcc/>",
            "<IEquipment ",
            "<IEquipmentOcc/>",
            "<IPBSItem ",
            "<IPBSItemCollection/>",
            "<IPlannedMatl/>",
            "<IProcessEquipment/>",
            "<IProcessEquipmentOcc/>",
            "<IProcessVessel ",
            "<IDrawingItem/>",
            "<IPIDProcessVessel/>",
            "<ILowPressureTankOcc/>",
            "<ILowPressureTank/>",
            "<ISpecifiedMatlItem ",
            "<IPIDTypical ",
            "</PIDProcessVessel>",
        ];
        let mut last_pos = 0usize;
        for needle in positions {
            let pos = out[last_pos..].find(needle).unwrap_or_else(|| {
                panic!("missing `{needle}` after offset {last_pos}\nout:\n{out}")
            });
            last_pos += pos + needle.len();
        }
    }

    #[test]
    fn process_vessel_tank_variant_preserves_all_other_attributes() {
        // A25 must be strictly additive: enabling the flag
        // must not change any other interface's attribute
        // content. Compare the non-tank and tank shapes for
        // the same inputs and assert the tank version is the
        // non-tank version with exactly two new lines inserted
        // (ILowPressureTankOcc + ILowPressureTank).
        let mk = |is_tank: &str| -> String {
            let mut d = PublishDrawing::new("UID-D", "DWG");
            d.objects = vec![PublishObject {
                uid: "V-CMP".into(),
                item_type_name: "Vessel".into(),
                fields: std::collections::BTreeMap::from([(
                    "IsLowPressureTank".to_string(),
                    is_tank.to_string(),
                )]),
                ..PublishObject::default()
            }];
            write_data_xml(&d, "TEST02").expect("write")
        };
        let non_tank = mk("False");
        let tank = mk("True");
        let tank_without_extras = tank
            .replace("      <ILowPressureTankOcc/>\n", "")
            .replace("      <ILowPressureTank/>\n", "");
        assert_eq!(
            non_tank, tank_without_extras,
            "A25 tank variant should differ from non-tank ONLY by the two inserted interfaces, not by any other byte change.\nnon_tank:\n{non_tank}\ntank_without_extras:\n{tank_without_extras}"
        );
    }

    // -----------------------------------------------------------------
    // A22 — PIDNozzle fidelity upgrade (9 → 22 interfaces,
    // + DWG-style ProcessEqCompType1/2 attr routing,
    // + conditional bare shape for IPipeCrossSectionItem /
    //   IPipingSpecifiedItem)
    // -----------------------------------------------------------------

    // -----------------------------------------------------------------
    // A29 — explicit PublishStyle selector for IObject shape on
    // PIDPipeline / PIDPipingConnector / PIDProcessVessel.
    // -----------------------------------------------------------------

    #[test]
    fn default_publish_style_is_a01_so_pre_a29_callers_round_trip() {
        // Documents the default-style contract: an unset
        // `style` field equals A01, so every pre-A29 caller
        // round-trips bit-for-bit. A regression that flips
        // the default would silently change every existing
        // caller's emitted XML.
        let d = PublishDrawing::default();
        assert_eq!(d.style, PublishStyle::A01);
    }

    #[test]
    fn pipeline_dwg_style_iobject_drops_itemtag_and_uses_name_only() {
        // DWG reference emits `<IObject UID="..." Name="..."/>`
        // on PIDPipeline (two-attr shape, no ItemTag). Pre-A29
        // the writer would emit Name + ItemTag together when
        // PipelineName was populated; A29 routes it via the
        // explicit style selector and gives a clean DWG shape.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "PIPE-DWG".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("PipelineName".into(), "A3jqz0101-OD".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IObject UID="PIPE-DWG" Name="A3jqz0101-OD"/>"#),
            "DWG-style PIDPipeline IObject must be UID + Name only; out:\n{out}"
        );
        assert!(
            !out.contains(r#"<IObject UID="PIPE-DWG" Name="A3jqz0101-OD" ItemTag="#),
            "DWG-style must not retain the A01-shape ItemTag tail; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_dwg_style_with_no_pipeline_name_emits_uid_only_iobject() {
        // Defensive branch: DWG fixture itself only ships
        // pipelines with names, but we still need a sensible
        // emit for inputs that lack PipelineName under
        // style = Dwg.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "PIPE-DWG-NONAME".into(),
            item_type_name: "PipeRun".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IObject UID="PIPE-DWG-NONAME"/>"#),
            "DWG-style with no PipelineName must emit UID-only IObject; out:\n{out}"
        );
        assert!(
            !out.contains(r#"<IObject UID="PIPE-DWG-NONAME" ItemTag="#),
            "DWG-style must never fall back to ItemTag; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_a01_style_with_pipeline_name_keeps_pre_a29_three_attr_shape() {
        // Behavioral lock: style=A01 + PipelineName populated
        // continues to emit `Name + ItemTag` together,
        // matching the pre-A29 superset shape that A23 / A27
        // gates ratify.
        let mut d = PublishDrawing::new("UID-D", "A01");
        // No explicit assignment — default already PublishStyle::A01.
        d.objects = vec![PublishObject {
            uid: "PIPE-A01".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("PipelineName".into(), "PIPE-A01-NAME".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IObject UID="PIPE-A01" Name="PIPE-A01-NAME" ItemTag="#),
            "A01 default style + PipelineName must emit Name + ItemTag together; out:\n{out}"
        );
    }

    #[test]
    fn piping_connector_dwg_style_drops_itemtag() {
        // PipingConnector under DWG style: UID + Name when
        // PipelineName populated; matches DWG reference shape.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "PIPE-CNX-DWG".into(),
            item_type_name: "PipeRun".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("PipelineName".into(), "PIPE-CNX-DWG-NAME".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IObject UID="PIPE-CNX-DWG-CNX" Name="PIPE-CNX-DWG-NAME"/>"#),
            "DWG-style PipingConnector IObject must be UID + Name only; out:\n{out}"
        );
        assert!(
            !out.contains(r#"<IObject UID="PIPE-CNX-DWG-CNX" ItemTag="#),
            "DWG-style PipingConnector must drop ItemTag; out:\n{out}"
        );
    }

    #[test]
    fn process_vessel_dwg_style_drops_itemtag_keeps_description() {
        // DWG reference vessel IObject is `UID + Description`
        // (no ItemTag, no Name). The `污水池` sample at DWG:
        // 1430 demonstrates the shape. A29 routes it through
        // the new style flag.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "V-DWG".into(),
            item_type_name: "Vessel".into(),
            description: Some("污水池".into()),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("ItemTag".into(), "V-DWG-TAG".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IObject UID="V-DWG" Description="污水池"/>"#),
            "DWG-style PIDProcessVessel IObject must be UID + Description only; out:\n{out}"
        );
        // Even when the field carries an ItemTag, DWG style
        // must NOT emit the ItemTag attribute.
        assert!(
            !out.contains(r#"<IObject UID="V-DWG" ItemTag="#),
            "DWG-style vessel must drop ItemTag even when the field is populated; out:\n{out}"
        );
    }

    #[test]
    fn process_vessel_a01_style_keeps_three_attr_shape() {
        // Lock in the pre-A29 three-attr shape under default
        // style. This is the contract every supported_pid_tags()
        // / A23 / A27 gate already enforces; the explicit
        // assertion below makes the lock obvious.
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "V-A01".into(),
            item_type_name: "Vessel".into(),
            description: Some("Horizontal Drum".into()),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("ItemTag".into(), "V-A01-TAG".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(
                r#"<IObject UID="V-A01" ItemTag="V-A01-TAG" Description="Horizontal Drum"/>"#
            ),
            "A01-style vessel IObject must be UID + ItemTag + Description; out:\n{out}"
        );
    }

    #[test]
    fn nozzle_emits_full_twenty_two_interface_block() {
        // A01_Data.xml:29–53 reference has 22 interfaces. Pin
        // every wrapper so a regression that drops one of the 13
        // A22-added interfaces trips immediately.
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "NZ-F".into(),
            item_type_name: "Nozzle".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        for needle in [
            "<PIDNozzle>",
            r#"<IObject UID="NZ-F"/>"#,
            "<IPBSItem ",
            "<IPipingPortComposition/>",
            "<IPlannedMatl/>",
            "<IDrawingItem/>",
            "<INozzleOcc/>",
            "<INozzle/>",
            "<IEquipmentComponent ",
            "<IEquipmentComponentOcc/>",
            "<IFabricatedItem/>",
            "<IHeatTracedItem ",
            "<IPBSItemCollection/>",
            "<IProcessPointCollection/>",
            "<ISignalPortComposition/>",
            "<IPartOcc/>",
            "<IDocumentItem/>",
            "<IElecPowerConsumer/>",
            "<IPart/>",
            "<INoteCollection/>",
            "<IProcessDataCaseComposition/>",
            "<IPipeCrossSectionItem/>",
            "<IPipingSpecifiedItem/>",
            "<IPIDTypical ",
            "</PIDNozzle>",
        ] {
            assert!(
                out.contains(needle),
                "PIDNozzle block must carry `{needle}`; out:\n{out}"
            );
        }
    }

    #[test]
    fn nozzle_ipipe_cross_section_item_bare_when_nominal_diameter_absent() {
        // A01 ships bare <IPipeCrossSectionItem/> (no attribute)
        // on PIDNozzle; only PIDPipingConnector populates
        // NominalDiameter. Pre-A22 writer forced an empty
        // attribute here, diverging from A01 bytes.
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "NZ-BARE".into(),
            item_type_name: "Nozzle".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains("<IPipeCrossSectionItem/>"),
            "A01 shape requires bare <IPipeCrossSectionItem/> on PIDNozzle; out:\n{out}"
        );
        assert!(
            out.contains("<IPipingSpecifiedItem/>"),
            "A01 shape requires bare <IPipingSpecifiedItem/> on PIDNozzle; out:\n{out}"
        );
    }

    #[test]
    fn nozzle_expands_cross_section_and_specified_item_when_populated() {
        // DWG-style populated form. When the loader stamps
        // NominalDiameter + PipingMaterialsClass, the writer must
        // switch to the attribute-bearing shape.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "NZ-POP".into(),
            item_type_name: "Nozzle".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("NominalDiameter".into(), "100".into());
                m.insert("PipingMaterialsClass".into(), "1.6AR12".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IPipeCrossSectionItem NominalDiameter="100 mm"/>"#),
            "populated NominalDiameter must land with mm suffix; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IPipingSpecifiedItem PipingMaterialsClass="1.6AR12"/>"#),
            "populated PipingMaterialsClass must land; out:\n{out}"
        );
    }

    #[test]
    fn nozzle_iequipment_component_gains_process_eq_comp_type_attrs() {
        // DWG ships `<IEquipmentComponent ProcessEqCompType1="@EE6D4"
        // ProcessEqCompType2="@{...}" ProcEqpCompTypeDescription="..."/>`.
        // A22 routes those fields so a future loader-side upgrade
        // becomes visible without touching the writer again.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "NZ-TYPE".into(),
            item_type_name: "Nozzle".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("ProcessEqCompType1".into(), "@EE6D4".into());
                m.insert(
                    "ProcessEqCompType2".into(),
                    "@{B88907F5-D4FC-49D8-BA8E-C1F76F392A52}".into(),
                );
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(
                r#"<IEquipmentComponent ProcessEqCompType1="@EE6D4" ProcessEqCompType2="@{B88907F5-D4FC-49D8-BA8E-C1F76F392A52}" ProcEqpCompTypeDescription="Flanged Nozzle"/>"#
            ),
            "expanded DWG-shape IEquipmentComponent must carry all three attrs in canonical order; out:\n{out}"
        );
    }

    #[test]
    fn nozzle_defaults_to_a17_canonical_ipbsitem_values() {
        // Every pipe-composed tag (PipingComponent / PipingConnector
        // / Vessel / Nozzle) uses the same canonical IPBSItem
        // defaults. Pin them so a future change that diverges on
        // one tag alone will trip here.
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "NZ-DEF".into(),
            item_type_name: "Nozzle".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(
                r#"<IPBSItem ConstructionStatus="@NewConstruction" ConstructionStatus2="@{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}"/>"#
            ),
            "A01-canonical IPBSItem defaults must land on PIDNozzle; out:\n{out}"
        );
    }

    #[test]
    fn nozzle_emits_interfaces_in_sppid_canonical_order() {
        // Pin the 22-interface canonical order via find() cursor.
        let mut d = PublishDrawing::new("UID-D", "A01");
        d.objects = vec![PublishObject {
            uid: "NZ-O".into(),
            item_type_name: "Nozzle".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        let positions = [
            "<PIDNozzle>",
            "<IObject UID=",
            "<IPBSItem ",
            "<IPipingPortComposition/>",
            "<IPlannedMatl/>",
            "<IDrawingItem/>",
            "<INozzleOcc/>",
            "<INozzle/>",
            "<IEquipmentComponent ",
            "<IEquipmentComponentOcc/>",
            "<IFabricatedItem/>",
            "<IHeatTracedItem ",
            "<IPBSItemCollection/>",
            "<IProcessPointCollection/>",
            "<ISignalPortComposition/>",
            "<IPartOcc/>",
            "<IDocumentItem/>",
            "<IElecPowerConsumer/>",
            "<IPart/>",
            "<INoteCollection/>",
            "<IProcessDataCaseComposition/>",
            "<IPipeCrossSectionItem",
            "<IPipingSpecifiedItem",
            "<IPIDTypical ",
            "</PIDNozzle>",
        ];
        let mut last_pos = 0usize;
        for needle in positions {
            let pos = out[last_pos..].find(needle).unwrap_or_else(|| {
                panic!("missing `{needle}` after offset {last_pos}\nout:\n{out}")
            });
            last_pos += pos + needle.len();
        }
    }

    // -----------------------------------------------------------------
    // A24 — final fidelity pass: IPBSItem defaults on
    // PIDControlSystemFunction + bare-when-empty INote on PIDNote
    // -----------------------------------------------------------------

    #[test]
    fn control_system_function_ipbsitem_uses_canonical_defaults() {
        // A24: PIDControlSystemFunction joins the uniform IPBSItem
        // defaults used by A17/A20/A21/A22. The DWG reference has
        // `<IPBSItem ConstructionStatus="@NewConstruction"
        // ConstructionStatus2="@{78398AB4-...}"/>` and pre-A24 our
        // writer emitted a bare `<IPBSItem/>` which diverged.
        // NB: PIDDrawing legitimately keeps `<IPBSItem/>` bare
        // (its reference shape), so the bare-form check must be
        // scoped to the ControlSystemFunction block only.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-A24".into(),
            item_type_name: "InstrFunction".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(
                r#"<IPBSItem ConstructionStatus="@NewConstruction" ConstructionStatus2="@{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}"/>"#
            ),
            "PIDControlSystemFunction must use canonical IPBSItem defaults; out:\n{out}"
        );
        // Slice out the ControlSystemFunction block and assert
        // the bare form does not appear there (it's fine inside
        // PIDDrawing, which has a different canonical shape).
        let open = out
            .find("<PIDControlSystemFunction>")
            .expect("function block open");
        let close = out
            .find("</PIDControlSystemFunction>")
            .expect("function block close");
        let block = &out[open..=close + "</PIDControlSystemFunction>".len()];
        assert!(
            !block.contains("<IPBSItem/>"),
            "A24 must no longer emit bare <IPBSItem/> inside PIDControlSystemFunction; block:\n{block}"
        );
    }

    #[test]
    fn control_system_function_ipbsitem_allows_field_override() {
        // The canonical defaults are overridable when the loader
        // populates alternate ConstructionStatus columns — matches
        // the same override path used by PipingComponent /
        // PipingConnector / ProcessVessel / Nozzle.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "INSTR-OVR".into(),
            item_type_name: "InstrFunction".into(),
            fields: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("ConstructionStatus".into(), "@Revised".into());
                m.insert("ConstructionStatus2".into(), "@{CUSTOM-GUID}".into());
                m
            },
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(
                r#"<IPBSItem ConstructionStatus="@Revised" ConstructionStatus2="@{CUSTOM-GUID}"/>"#
            ),
            "loader-supplied ConstructionStatus values must override the SPPID defaults; out:\n{out}"
        );
    }

    #[test]
    fn note_with_populated_text_still_emits_attribute_form() {
        // Keep the populated-text path intact (A24 only changed
        // the empty-text path). Verifies that notes with Chinese
        // CR/LF content still escape correctly.
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "NOTE-TEXT".into(),
            item_type_name: "Note".into(),
            description: Some("量液孔".into()),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<INote NoteText="量液孔"/>"#),
            "populated NoteText must round-trip through the attribute form; out:\n{out}"
        );
    }

    #[test]
    fn pipeline_dwg_style_falls_back_to_raw_name_field() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "PIPE-RAW-NAME".into(),
            item_type_name: "PipeRun".into(),
            fields: std::collections::BTreeMap::from([(
                "Name".to_string(),
                "A3jqz0101-OD".to_string(),
            )]),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IObject UID="PIPE-RAW-NAME" Name="A3jqz0101-OD"/>"#),
            "DWG-style PIDPipeline must accept raw T_PlantItem.Name as fallback; out:\n{out}"
        );
    }

    #[test]
    fn piping_connector_dwg_style_uses_raw_pipe_run_field_aliases() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "PIPE-RAW-ALIAS".into(),
            item_type_name: "PipeRun".into(),
            fields: std::collections::BTreeMap::from([
                ("Name".to_string(), "RAW-CONNECTOR".to_string()),
                ("FlowDirection".to_string(), "@EE873".to_string()),
                ("SP_ConnectorsZeroLength".to_string(), "1".to_string()),
                ("PipeRunType".to_string(), "Stub".to_string()),
                ("Slope".to_string(), "0.125".to_string()),
                ("SlopeDirection".to_string(), "@UP".to_string()),
                ("InsulationThkSource".to_string(), "@SRC".to_string()),
                ("InsulThick".to_string(), "12 mm".to_string()),
            ]),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"<IObject UID="PIPE-RAW-ALIAS-CNX" Name="RAW-CONNECTOR"/>"#),
            "DWG-style connector IObject must accept raw Name fallback; out:\n{out}"
        );
        assert!(
            out.contains(
                r#"<IConnector FlowDirection="@EE873" RepresentationsAreAllZeroLength="True"/>"#
            ),
            "raw zero-length flag must map onto RepresentationsAreAllZeroLength; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IPipingConnector PipingConnectorType="Stub"/>"#),
            "PipeRunType must map onto PipingConnectorType under DWG style; out:\n{out}"
        );
        assert!(
            out.contains(
                r#"<ISlopedPipingItem SlopedPipingAngle="0.125" SlopedPipeDirection="@UP"/>"#
            ),
            "Slope/SlopeDirection must feed the DWG sloped-piping shape; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IInsulatedItem InsulThickSrc="@SRC" TotalInsulThick="12 mm"/>"#),
            "InsulationThkSource/InsulThick must feed the DWG insulated-item shape; out:\n{out}"
        );
    }

    #[test]
    fn process_vessel_dwg_style_uses_trimspec_and_volume_rating_fallbacks() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.style = PublishStyle::Dwg;
        d.objects = vec![PublishObject {
            uid: "VESSEL-RAW-FALLBACK".into(),
            item_type_name: "Vessel".into(),
            fields: std::collections::BTreeMap::from([
                ("TrimSpec".to_string(), "1.6AR12".to_string()),
                ("VolumeRating".to_string(), "27 m^3".to_string()),
            ]),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "TEST02").expect("write");
        assert!(
            out.contains(r#"EquipmentTrimSpec="1.6AR12""#),
            "DWG-style vessel must accept TrimSpec as EquipmentTrimSpec fallback; out:\n{out}"
        );
        assert!(
            out.contains(r#"<IProcessVessel ProcessVessel_VesselVolumetricCapacity="27 m^3"/>"#),
            "DWG-style vessel must accept VolumeRating as volumetric-capacity fallback; out:\n{out}"
        );
    }

    // -----------------------------------------------------------------
    // Stage-4 — PIDBranchPoint + PIDPipingBranchPoint writer arms.
    // -----------------------------------------------------------------

    #[test]
    fn piping_branch_point_emits_six_interface_shape() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "CCB3BA926FC54BF89691BC690FAF7D74.BPT".into(),
            item_type_name: "PipingBranchPoint".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "P01").expect("write");
        for needle in [
            "<PIDPipingBranchPoint>",
            r#"<IObject UID="CCB3BA926FC54BF89691BC690FAF7D74.BPT"/>"#,
            "<IConnection/>",
            "<IPipingConnection/>",
            "<IDrawingItem/>",
            "<IPipingBranchPoint/>",
            "<IDocumentItem/>",
            "</PIDPipingBranchPoint>",
        ] {
            assert!(
                out.contains(needle),
                "PIDPipingBranchPoint must carry `{needle}`; out:\n{out}"
            );
        }
    }

    #[test]
    fn piping_branch_point_interface_ordering_matches_reference() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "AAA.BPT".into(),
            item_type_name: "PipingBranchPoint".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "P01").expect("write");
        let positions = [
            "<PIDPipingBranchPoint>",
            "<IObject UID=",
            "<IConnection/>",
            "<IPipingConnection/>",
            "<IDrawingItem/>",
            "<IPipingBranchPoint/>",
            "<IDocumentItem/>",
            "</PIDPipingBranchPoint>",
        ];
        let mut last_pos = 0usize;
        for needle in positions {
            let pos = out[last_pos..].find(needle).unwrap_or_else(|| {
                panic!(
                    "PIDPipingBranchPoint: `{needle}` not found after position {last_pos}; out:\n{out}"
                )
            }) + last_pos;
            last_pos = pos + needle.len();
        }
    }

    #[test]
    fn pid_branch_point_emits_eight_interface_shape_with_name() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "0DFD856D382C42F88DA8CDDFD37D4227".into(),
            item_type_name: "BranchPoint".into(),
            fields: std::collections::BTreeMap::from([("Name".to_string(), "272".to_string())]),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "P01").expect("write");
        for needle in [
            "<PIDBranchPoint>",
            r#"<IObject UID="0DFD856D382C42F88DA8CDDFD37D4227" Name="272"/>"#,
            "<IPIDBranchPoint/>",
            "<IDuctConnection/>",
            "<IConnection/>",
            "<IDrawingItem/>",
            "<IPipingConnection/>",
            "<ISignalConnection/>",
            "<IDocumentItem/>",
            "</PIDBranchPoint>",
        ] {
            assert!(
                out.contains(needle),
                "PIDBranchPoint must carry `{needle}`; out:\n{out}"
            );
        }
    }

    #[test]
    fn pid_branch_point_interface_ordering_matches_reference() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "BBB".into(),
            item_type_name: "BranchPoint".into(),
            fields: std::collections::BTreeMap::from([("Name".to_string(), "1".to_string())]),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "P01").expect("write");
        let positions = [
            "<PIDBranchPoint>",
            r#"<IObject UID="BBB" Name="1"/>"#,
            "<IPIDBranchPoint/>",
            "<IDuctConnection/>",
            "<IConnection/>",
            "<IDrawingItem/>",
            "<IPipingConnection/>",
            "<ISignalConnection/>",
            "<IDocumentItem/>",
            "</PIDBranchPoint>",
        ];
        let mut last_pos = 0usize;
        for needle in positions {
            let pos = out[last_pos..].find(needle).unwrap_or_else(|| {
                panic!(
                    "PIDBranchPoint: `{needle}` not found after position {last_pos}; out:\n{out}"
                )
            }) + last_pos;
            last_pos = pos + needle.len();
        }
    }

    #[test]
    fn pid_branch_point_omits_name_attr_when_field_is_empty() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "CCC".into(),
            item_type_name: "BranchPoint".into(),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "P01").expect("write");
        assert!(
            out.contains(r#"<IObject UID="CCC"/>"#),
            "PIDBranchPoint IObject must omit Name when the field is missing; out:\n{out}"
        );
        let branch_block = out
            .split("<PIDBranchPoint>")
            .nth(1)
            .and_then(|rest| rest.split("</PIDBranchPoint>").next())
            .unwrap_or("");
        assert!(
            !branch_block.contains("Name="),
            "PIDBranchPoint block must not contain Name= when field is missing; block:\n{branch_block}"
        );
    }

    #[test]
    fn pid_branch_point_falls_back_to_description_for_name() {
        let mut d = PublishDrawing::new("UID-D", "DWG");
        d.objects = vec![PublishObject {
            uid: "DDD".into(),
            item_type_name: "BranchPoint".into(),
            description: Some("99".to_string()),
            ..PublishObject::default()
        }];
        let out = write_data_xml(&d, "P01").expect("write");
        assert!(
            out.contains(r#"<IObject UID="DDD" Name="99"/>"#),
            "PIDBranchPoint must fall back to description for Name; out:\n{out}"
        );
    }
}
