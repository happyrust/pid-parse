//! Derived and source relationship XML emission.

use std::collections::HashMap;
use std::fmt::Write;

use super::super::model::{PublishDrawing, PublishError, PublishRelationship};
use super::common::{escape_attr, fmt_err, ordered_publishable_representations};
use super::pipeline::derived_pipe_connector_uid;

pub(super) fn write_relationships(
    buf: &mut String,
    drawing: &PublishDrawing,
) -> Result<(), PublishError> {
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
    // Naturally filtered to publishable reps (the rel's source IS
    // `model_item_uid`, so a pure annotation row with no model
    // item never produces one). Keep the inline check anyway so a
    // future loader change that accidentally injects `Some("")`
    // does not silently produce a malformed rel.
    let ordered_reps = ordered_publishable_representations(drawing);
    for rep in &ordered_reps {
        let Some(model_item_uid) = rep.model_item_uid.as_deref() else {
            continue;
        };
        write_rel(
            buf,
            &format!("DRC-{}-{}", model_item_uid, rep.uid),
            model_item_uid,
            &rep.uid,
            "DwgRepresentationComposition",
        )?;
    }

    // --- Derived: Drawing → Representation (DrawingItems)
    // A14: only emit `DrawingItems` for representations that survive
    // the A14 publishability filter. Otherwise we would generate a
    // rel pointing at a `<PIDRepresentation>` we never wrote — a
    // dangling reference SmartPlant validators reject.
    for rep in &ordered_reps {
        write_rel(
            buf,
            &format!("DRI-{}-{}", drawing.drawing_uid, rep.uid),
            &drawing.drawing_uid,
            &rep.uid,
            "DrawingItems",
        )?;
    }

    // --- A34: Derived rels for every PipeRun-driven
    // PipingConnector. SmartPlant's exporter pairs every
    // PipingConnector with five derived `<Rel>` rows that
    // wire the connector to its two virtual `<PIDPipingPort>`
    // children, the `<PIDProcessPoint>` collection, and the
    // two endpoint connections. The PIDxxx body emit happens
    // in `write_derived_connector_endpoints`; the rel emit
    // here keeps the per-connector rel count in lockstep so
    // the A33 `<Rel>` DefUID-count gate stays satisfied.
    //
    // A34b also derives the Pipeline → Connector composition
    // rel (`PipingConnectors`) here — same source object,
    // emitted alongside the five port-derived rels.
    //
    // UID derivation mirrors what `write_derived_connector_endpoints`
    // already does:
    //   * pipeline UID:  `<piperun>` (PipeRun obj.uid maps to
    //                    `<PIDPipeline>`)
    //   * connector UID: deterministic publish-time 32-hex UID
    //   * port UIDs:     `<connector>.1` / `<connector>.2`
    //   * process point: `<connector>.PPT`
    //
    // A34c — the PipingEnd1Conn / PipingEnd2Conn target is the
    // upstream ModelItem UID sitting at the connected end of the
    // pipe (e.g. a Nozzle or Vessel). The loader resolves this
    // via `T_Connector.SP_ConnectItem{1,2}ID` in
    // `attach_pipe_endpoint_connections` and stashes the result
    // in `obj.fields["EndConnectedItem1"]` /
    // `obj.fields["EndConnectedItem2"]`.
    //
    // Fallback: when the loader didn't populate an endpoint (no
    // T_Connector row, or the port is physically unconnected —
    // A01's port.2 behaves this way), we keep the pre-A34c
    // `<connector>.PPT` placeholder. The reference XML does
    // exactly the same for its unconnected port.2, so the
    // fallback is not a hack — it's the SPPID convention for
    // "no external connection".
    // --- A34b: Vessel → Nozzle composition (EquipmentComponentComposition).
    //
    // SmartPlant ties every nozzle to its parent vessel via
    // the T_Nozzle.SP_EquipmentID column (loaded into
    // `obj.fields["SP_EquipmentID"]` by `attach_business_columns`).
    // The reference XML's EquipmentComponentComposition row
    // is a derived `<Rel>` from this parent link, not a
    // T_Relationship row — A01's T_Relationship table only
    // carries Representation ↔ Representation rels.
    for obj in &drawing.objects {
        if obj.item_type_name != "Nozzle" {
            continue;
        }
        let Some(vessel_uid) = obj.fields.get("SP_EquipmentID") else {
            continue;
        };
        if vessel_uid.is_empty() {
            continue;
        }
        write_rel(
            buf,
            &format!("EQC-{vessel_uid}-{}", obj.uid),
            &obj.uid,
            vessel_uid,
            "EquipmentComponentComposition",
        )?;
    }

    for obj in &drawing.objects {
        if obj.item_type_name != "PipeRun" {
            continue;
        }
        let pipeline_uid = obj.uid.as_str();
        let connector_uid = derived_pipe_connector_uid(pipeline_uid);
        let port1_uid = format!("{connector_uid}.1");
        let port2_uid = format!("{connector_uid}.2");
        let ppt_uid = format!("{connector_uid}.PPT");
        let end1_uid = obj
            .fields
            .get("EndConnectedItem1")
            .cloned()
            .unwrap_or_else(|| ppt_uid.clone());
        let end2_uid = obj
            .fields
            .get("EndConnectedItem2")
            .cloned()
            .unwrap_or_else(|| ppt_uid.clone());
        // A34b: Pipeline → Connector composition.
        write_rel(
            buf,
            &format!("PCN-{pipeline_uid}"),
            pipeline_uid,
            &connector_uid,
            "PipingConnectors",
        )?;
        write_rel(
            buf,
            &format!("PPC-{connector_uid}-1"),
            &connector_uid,
            &port1_uid,
            "PipingPortComposition",
        )?;
        write_rel(
            buf,
            &format!("PPC-{connector_uid}-2"),
            &connector_uid,
            &port2_uid,
            "PipingPortComposition",
        )?;
        write_rel(
            buf,
            &format!("PRP-{connector_uid}"),
            &connector_uid,
            &ppt_uid,
            "ProcessPointCollection",
        )?;
        write_rel(
            buf,
            &format!("PE1-{port1_uid}"),
            &port1_uid,
            &end1_uid,
            "PipingEnd1Conn",
        )?;
        write_rel(
            buf,
            &format!("PE2-{port2_uid}"),
            &port2_uid,
            &end2_uid,
            "PipingEnd2Conn",
        )?;
    }

    // --- From T_Relationship, classified by endpoint item types
    //
    // A36b — skip rows whose source or target UID is NULL / empty.
    // SmartPlant's exporter never emits an `<IRel UID2=""/>` for
    // a half-wired relationship; shipping one produces a dangling
    // reference that validators reject. The A36b soundness gate
    // surfaced this when T_Relationship carried a row with an
    // unpaired endpoint on A01.
    //
    // A40 — also skip rows where BOTH endpoints resolve to
    // Representations. Investigation on the A01 fixture
    // revealed SmartPlant's exporter does not emit these as
    // their own `<Rel>` entries — every Rep↔Rep relationship
    // is already covered by the ModelItem → Rep derived
    // emits in `write_derived_*`, so re-emitting them here
    // classifies as DwgRepresentationComposition a second
    // time and produces an over-count. The A40 Rel DefUID
    // diff against the A01 reference surfaced this as a
    // DELTA row (writer 6, reference 4, +2 extras).
    for rel in &drawing.relationships {
        let uid1 = rel.source_uid.as_deref().unwrap_or("");
        let uid2 = rel.target_uid.as_deref().unwrap_or("");
        if uid1.is_empty() || uid2.is_empty() {
            continue;
        }
        let t1 = type_by_uid.get(uid1).copied().unwrap_or("");
        let t2 = type_by_uid.get(uid2).copied().unwrap_or("");
        if t1 == "Representation" && t2 == "Representation" {
            continue;
        }
        let def_uid = classify_relationship(rel, &type_by_uid);
        let prefix = defuid_prefix(&def_uid);
        let rel_uid = format!("{prefix}-{uid1}-{uid2}");
        write_rel(buf, &rel_uid, uid1, uid2, &def_uid)?;
    }
    Ok(())
}

/// Emit a single `<Rel>` node. `rel_uid` is the current writer-side
/// deterministic placeholder seed (`DRC-…`, `DRI-…`, `PCN-…`,
/// `PPC-…`, `PPP-…`, `EQC-…`, `PE1-…`, `PE2-…`, or the general
/// `prefix-uid1-uid2` form) and is used verbatim as the published
/// `<IObject UID>`.
///
/// This keeps repeated exports stable and debuggable, but it is not
/// yet the real `SmartPlant` 32-hex rel-IObject numbering rule. A01
/// delivery-contract and raw-residual tests therefore keep this slot
/// under explicit normalization until the publish-time rule is
/// reconstructed from TEST02.
fn write_rel(
    buf: &mut String,
    rel_uid: &str,
    uid1: &str,
    uid2: &str,
    def_uid: &str,
) -> Result<(), PublishError> {
    writeln!(buf, "   <Rel>").map_err(fmt_err)?;
    writeln!(buf, r#"      <IObject UID="{}"/>"#, escape_attr(rel_uid)).map_err(fmt_err)?;
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

/// Pick the SPPID `DefUID` for a `T_Relationship` row given a lookup
/// of endpoint `ItemTypeNames`. Stage-1 covers the combinations
/// observed in TEST02 A01; anything unknown falls back to the
/// generic `"Relationship"` so the writer stays total.
fn classify_relationship(rel: &PublishRelationship, type_by_uid: &HashMap<&str, &str>) -> String {
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
