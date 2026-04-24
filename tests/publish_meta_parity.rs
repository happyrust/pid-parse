//! `_Meta.xml` fidelity gates.
//!
//! The publish writer already has broad `_Data.xml` parity coverage
//! (tag / interface / attr / rel gates), but `_Meta.xml` only had
//! smoke checks on the CLI surface. This file adds a lightweight
//! fixture-driven contract for the document-versioning sibling:
//!
//! * both bundled reference fixtures (A01 + DWG) must keep the same
//!   canonical top-level shape,
//! * the writer's generated A01 `_Meta.xml` must match the A01
//!   reference on every semantic field except the internally-derived
//!   helper UIDs,
//! * repeated emits of the same drawing must stay byte-identical,
//! * the writer's generated DWG `_Meta.xml` matches the DWG
//!   reference under the same semantic-field contract —
//!   soft-skipped until the DWG MDF fixture lands (see
//!   [`common::DWG_MDF_MISSING_HINT`]).

use std::collections::{BTreeMap, BTreeSet};

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

mod common;
use common::{
    generate_a01_meta_xml, generate_dwg_meta_xml, load_reference_a01_meta_xml,
    load_reference_dwg_meta_xml, DWG_PLANT_NAME, PLANT_NAME,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct DocumentVersionBlock {
    uid: String,
    name: String,
    doc_revision: String,
    doc_version_date: String,
    doc_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DocumentRevisionBlock {
    uid: String,
    name: String,
    major_rev_for_revise: String,
    minor_rev_for_revise: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileBlock {
    uid: String,
    name: String,
    description: String,
    file_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RelBlock {
    def_uid: String,
    uid1: String,
    uid2: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedMeta {
    container: BTreeMap<String, String>,
    top_level_order: Vec<String>,
    document_version: DocumentVersionBlock,
    document_revision: DocumentRevisionBlock,
    file: FileBlock,
    rels: Vec<RelBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CanonicalMetaSummary {
    comp_schema: String,
    scope: String,
    plant: String,
    doc_uid: String,
    doc_name: String,
    top_level_order: Vec<String>,
    version_name: String,
    version_date: String,
    version_number: String,
    revision_name: String,
    revision_major: String,
    revision_minor: String,
    file_name: String,
    file_description: String,
    file_path: String,
    rel_defuids: Vec<String>,
    rel_edges: Vec<(String, String, String)>,
}

fn attr_map(e: &BytesStart<'_>, reader: &Reader<&[u8]>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for attr in e.attributes().with_checks(false) {
        let attr = attr.expect("valid xml attribute");
        let key = std::str::from_utf8(attr.key.as_ref())
            .expect("utf-8 attr key")
            .to_string();
        let value = attr
            .decode_and_unescape_value(reader.decoder())
            .expect("decode attr value")
            .into_owned();
        out.insert(key, value);
    }
    out
}

fn parse_meta_xml(xml: &str) -> ParsedMeta {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut container = BTreeMap::new();
    let mut top_level_order = Vec::new();

    let mut current_block: Option<String> = None;
    let mut document_version: Option<DocumentVersionBlock> = None;
    let mut document_revision: Option<DocumentRevisionBlock> = None;
    let mut file: Option<FileBlock> = None;
    let mut rels = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = std::str::from_utf8(e.name().as_ref())
                    .expect("utf-8 tag")
                    .to_string();
                match name {
                    ref tag if tag == "Container" => {
                        container = attr_map(&e, &reader);
                    }
                    ref tag
                        if matches!(
                            tag.as_str(),
                            "DocumentVersion" | "DocumentRevision" | "File" | "Rel"
                        ) =>
                    {
                        top_level_order.push(tag.clone());
                        current_block = Some(tag.clone());
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let name = std::str::from_utf8(e.name().as_ref())
                    .expect("utf-8 empty tag")
                    .to_string();
                let attrs = attr_map(&e, &reader);
                match (current_block.as_deref(), name.as_str()) {
                    (Some("DocumentVersion"), "IObject") => {
                        let uid = attrs.get("UID").cloned().unwrap_or_default();
                        let name = attrs.get("Name").cloned().unwrap_or_default();
                        document_version = Some(DocumentVersionBlock {
                            uid,
                            name,
                            doc_revision: String::new(),
                            doc_version_date: String::new(),
                            doc_version: String::new(),
                        });
                    }
                    (Some("DocumentVersion"), "IDocumentVersion") => {
                        let block = document_version
                            .as_mut()
                            .expect("IObject must precede IDocumentVersion");
                        block.doc_revision = attrs.get("DocRevision").cloned().unwrap_or_default();
                        block.doc_version_date =
                            attrs.get("DocVersionDate").cloned().unwrap_or_default();
                        block.doc_version = attrs.get("DocVersion").cloned().unwrap_or_default();
                    }
                    (Some("DocumentRevision"), "IObject") => {
                        let uid = attrs.get("UID").cloned().unwrap_or_default();
                        let name = attrs.get("Name").cloned().unwrap_or_default();
                        document_revision = Some(DocumentRevisionBlock {
                            uid,
                            name,
                            major_rev_for_revise: String::new(),
                            minor_rev_for_revise: String::new(),
                        });
                    }
                    (Some("DocumentRevision"), "IDocumentRevision") => {
                        let block = document_revision
                            .as_mut()
                            .expect("IObject must precede IDocumentRevision");
                        block.major_rev_for_revise = attrs
                            .get("MajorRev_ForRevise")
                            .cloned()
                            .unwrap_or_default();
                        block.minor_rev_for_revise = attrs
                            .get("MinorRev_ForRevise")
                            .cloned()
                            .unwrap_or_default();
                    }
                    (Some("File"), "IObject") => {
                        let uid = attrs.get("UID").cloned().unwrap_or_default();
                        let name = attrs.get("Name").cloned().unwrap_or_default();
                        let description = attrs.get("Description").cloned().unwrap_or_default();
                        file = Some(FileBlock {
                            uid,
                            name,
                            description,
                            file_path: String::new(),
                        });
                    }
                    (Some("File"), "IFile") => {
                        let block = file.as_mut().expect("IObject must precede IFile");
                        block.file_path = attrs.get("FilePath").cloned().unwrap_or_default();
                    }
                    (Some("Rel"), "IRel") => {
                        rels.push(RelBlock {
                            def_uid: attrs.get("DefUID").cloned().unwrap_or_default(),
                            uid1: attrs.get("UID1").cloned().unwrap_or_default(),
                            uid2: attrs.get("UID2").cloned().unwrap_or_default(),
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let name = std::str::from_utf8(e.name().as_ref())
                    .expect("utf-8 end tag")
                    .to_string();
                if matches!(
                    name.as_str(),
                    "DocumentVersion" | "DocumentRevision" | "File" | "Rel"
                ) {
                    current_block = None;
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(err) => panic!("parse meta xml: {err}"),
        }
    }

    ParsedMeta {
        container,
        top_level_order,
        document_version: document_version.expect("DocumentVersion block"),
        document_revision: document_revision.expect("DocumentRevision block"),
        file: file.expect("File block"),
        rels,
    }
}

fn canonical_summary(meta: &ParsedMeta) -> CanonicalMetaSummary {
    let doc_uid = meta.container.get("DocUID").cloned().unwrap_or_default();
    let doc_name = meta.container.get("DocName").cloned().unwrap_or_default();

    let roles = BTreeMap::from([
        (doc_uid.clone(), "Drawing".to_string()),
        (meta.document_version.uid.clone(), "Version".to_string()),
        (meta.document_revision.uid.clone(), "Revision".to_string()),
        (meta.file.uid.clone(), "File".to_string()),
    ]);

    let rel_edges = meta
        .rels
        .iter()
        .map(|rel| {
            let uid1 = roles
                .get(&rel.uid1)
                .cloned()
                .unwrap_or_else(|| format!("Unknown({})", rel.uid1));
            let uid2 = roles
                .get(&rel.uid2)
                .cloned()
                .unwrap_or_else(|| format!("Unknown({})", rel.uid2));
            (rel.def_uid.clone(), uid1, uid2)
        })
        .collect();

    CanonicalMetaSummary {
        comp_schema: meta.container.get("CompSchema").cloned().unwrap_or_default(),
        scope: meta.container.get("Scope").cloned().unwrap_or_default(),
        plant: meta.container.get("Plant").cloned().unwrap_or_default(),
        doc_uid,
        doc_name,
        top_level_order: meta.top_level_order.clone(),
        version_name: meta.document_version.name.clone(),
        version_date: meta.document_version.doc_version_date.clone(),
        version_number: meta.document_version.doc_version.clone(),
        revision_name: meta.document_revision.name.clone(),
        revision_major: meta.document_revision.major_rev_for_revise.clone(),
        revision_minor: meta.document_revision.minor_rev_for_revise.clone(),
        file_name: meta.file.name.clone(),
        file_description: meta.file.description.clone(),
        file_path: meta.file.file_path.clone(),
        rel_defuids: meta.rels.iter().map(|r| r.def_uid.clone()).collect(),
        rel_edges,
    }
}

fn assert_common_meta_contract(meta: &ParsedMeta, expected_plant: &str, expected_doc_name: &str) {
    assert_eq!(
        meta.container.get("CompSchema").map(String::as_str),
        Some("DocVersioningComponent")
    );
    assert_eq!(meta.container.get("Scope").map(String::as_str), Some("Data"));
    assert_eq!(meta.container.get("Plant").map(String::as_str), Some(expected_plant));
    assert_eq!(
        meta.container.get("DocName").map(String::as_str),
        Some(expected_doc_name)
    );
    assert_eq!(
        meta.top_level_order,
        vec![
            "DocumentVersion".to_string(),
            "Rel".to_string(),
            "DocumentRevision".to_string(),
            "Rel".to_string(),
            "File".to_string(),
            "Rel".to_string(),
        ],
        "top-level block order drifted"
    );
    let rel_defuids: Vec<&str> = meta.rels.iter().map(|r| r.def_uid.as_str()).collect();
    assert_eq!(
        rel_defuids,
        vec!["VersionedDoc", "RevisedDocument", "FileComposition"],
        "meta relation order drifted"
    );
    assert_eq!(
        meta.document_version.name,
        format!("{expected_doc_name} Version"),
        "DocumentVersion IObject naming drifted"
    );
    assert_eq!(
        meta.document_revision.name,
        format!("{expected_doc_name} Revision"),
        "DocumentRevision IObject naming drifted"
    );
    assert_eq!(
        meta.file.name,
        format!("{expected_doc_name}.pid"),
        "File IObject naming drifted"
    );
    assert_eq!(meta.document_version.doc_revision, "0");
    assert_eq!(meta.document_version.doc_version, "1");
    assert_eq!(meta.document_revision.major_rev_for_revise, "0");
    assert_eq!(meta.document_revision.minor_rev_for_revise, "");
    assert_eq!(meta.file.description, "");
    assert_eq!(meta.rels.len(), 3, "meta document must carry exactly 3 rels");

    let unique_uids: BTreeSet<&str> = BTreeSet::from([
        meta.document_version.uid.as_str(),
        meta.document_revision.uid.as_str(),
        meta.file.uid.as_str(),
    ]);
    assert_eq!(unique_uids.len(), 3, "version/revision/file UIDs must be unique");

    let summary = canonical_summary(meta);
    assert_eq!(
        summary.rel_edges,
        vec![
            (
                "VersionedDoc".to_string(),
                "Drawing".to_string(),
                "Version".to_string(),
            ),
            (
                "RevisedDocument".to_string(),
                "Revision".to_string(),
                "Drawing".to_string(),
            ),
            (
                "FileComposition".to_string(),
                "File".to_string(),
                "Version".to_string(),
            ),
        ],
        "meta rel endpoint graph drifted"
    );
}

#[test]
fn a01_reference_meta_fixture_has_expected_canonical_shape() {
    let Some(xml) = load_reference_a01_meta_xml() else {
        return;
    };
    let parsed = parse_meta_xml(&xml);
    assert_common_meta_contract(&parsed, PLANT_NAME, "A01");
    assert_eq!(
        parsed.document_version.doc_version_date, "2026/04/20",
        "A01 reference date should already be normalized"
    );
    assert_eq!(
        parsed.file.file_path, "",
        "A01 reference fixture should carry an empty FilePath"
    );
}

#[test]
fn dwg_reference_meta_fixture_has_expected_canonical_shape() {
    let Some(xml) = load_reference_dwg_meta_xml() else {
        return;
    };
    let parsed = parse_meta_xml(&xml);
    assert_common_meta_contract(&parsed, DWG_PLANT_NAME, "DWG-0202GP06-01");
    assert_eq!(
        parsed.document_version.doc_version_date, "2026/04/02",
        "DWG reference date should already be normalized"
    );
    assert!(
        !parsed.file.file_path.is_empty(),
        "DWG reference fixture should carry a non-empty FilePath"
    );
}

#[test]
fn generated_a01_meta_matches_reference_summary_ignoring_derived_uids() {
    let Some(reference_xml) = load_reference_a01_meta_xml() else {
        return;
    };
    let Some(generated_result) = generate_a01_meta_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("writer should succeed on A01");

    let reference = parse_meta_xml(&reference_xml);
    let generated = parse_meta_xml(&generated_xml);

    assert_eq!(
        canonical_summary(&generated),
        canonical_summary(&reference),
        "writer-generated A01 _Meta.xml drifted from the reference on semantic fields"
    );
}

#[test]
fn generated_a01_meta_is_byte_stable_across_repeated_emits() {
    let Some(first_result) = generate_a01_meta_xml() else {
        return;
    };
    let first = first_result.expect("first writer run should succeed");
    let Some(second_result) = generate_a01_meta_xml() else {
        return;
    };
    let second = second_result.expect("second writer run should succeed");
    assert_eq!(
        first, second,
        "write_meta_xml must stay byte-identical for the same drawing input"
    );
}

/// DWG-side sibling of the A01 parity gate. When the DWG
/// MDF fixture is bundled, the writer's generated
/// `_Meta.xml` must carry the same canonical shape as the
/// bundled DWG reference (sans derived UIDs, per the same
/// contract as A01). Soft-skipped until the MDF lands —
/// see `DWG_MDF_MISSING_HINT` for the rationale message
/// the helper logs.
///
/// This is the test entry point that binds the DWG reference
/// XML to the DWG MDF fixture; callers no longer need to
/// fall back to the A01 fixture as a DWG-shape stand-in.
#[test]
fn generated_dwg_meta_matches_reference_summary_when_mirror_available() {
    let Some(reference_xml) = load_reference_dwg_meta_xml() else {
        return;
    };
    let Some(generated_result) = generate_dwg_meta_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("writer should succeed on DWG");

    let reference = parse_meta_xml(&reference_xml);
    let generated = parse_meta_xml(&generated_xml);

    assert_eq!(
        canonical_summary(&generated),
        canonical_summary(&reference),
        "writer-generated DWG _Meta.xml drifted from the reference on semantic fields"
    );
}

/// Byte-stability for DWG `_Meta.xml`, mirror-gated the same
/// way as the semantic parity test above.
#[test]
fn generated_dwg_meta_is_byte_stable_across_repeated_emits() {
    let Some(first_result) = generate_dwg_meta_xml() else {
        return;
    };
    let first = first_result.expect("first writer run should succeed");
    let Some(second_result) = generate_dwg_meta_xml() else {
        return;
    };
    let second = second_result.expect("second writer run should succeed");
    assert_eq!(
        first, second,
        "write_meta_xml must stay byte-identical for the same DWG drawing input"
    );
}
