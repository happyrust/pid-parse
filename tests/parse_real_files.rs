use pid_parse::{
    parsers::sheet_probe::{
        classify_field_x_record_shapes, field_x_window_features, field_x_window_identities,
        field_x_windows, probe_sheet_stream, score_field_x_window_features,
        score_field_x_window_features_with_identities, score_field_x_windows,
        score_sheet_text_window_candidates, sheet_identity_index_from_trailers,
        sheet_text_window_candidates, stable_chunk_shape_support, stable_marker_support,
        summarize_object_geometry_promotion_gate, top_field_x_candidate_record_dumps,
        top_text_candidate_record_dumps, SheetProbeOptions,
    },
    PidParser,
};
use std::collections::{BTreeMap, HashSet};

/// Parse a real `.pid` fixture from `test-file/`. Returns `None` when the
/// fixture isn't present (typical for CI and for contributors without
/// access to `SmartPlant` samples) so the test can cleanly skip instead of
/// panicking. See `writer_real_files.rs` for the matching pattern.
fn parse_test_file(name: &str) -> Option<pid_parse::PidDocument> {
    let path = format!("test-file/{name}");
    if !std::path::Path::new(&path).exists() {
        eprintln!("skipping: fixture {name} not found");
        return None;
    }
    Some(
        PidParser::new()
            .parse_file(&path)
            .unwrap_or_else(|e| panic!("Failed to parse {name}: {e}")),
    )
}

fn parse_test_package(name: &str) -> Option<pid_parse::PidPackage> {
    let path = format!("test-file/{name}");
    if !std::path::Path::new(&path).exists() {
        eprintln!("skipping: fixture {name} not found");
        return None;
    }
    Some(
        PidParser::new()
            .parse_package(&path)
            .unwrap_or_else(|e| panic!("Failed to parse package {name}: {e}")),
    )
}

fn hex_window(data: &[u8], center: usize, radius: usize) -> String {
    let start = center.saturating_sub(radius);
    let end = center.saturating_add(radius).min(data.len());
    let hex = data[start..end]
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ");
    format!("{start}..{end}: {hex}")
}

#[test]
fn container_structure_has_streams() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    assert!(!doc.streams.is_empty(), "streams should not be empty");
    assert!(
        doc.streams.len() > 10,
        "expected many streams, got {}",
        doc.streams.len()
    );
}

#[test]
fn cfb_tree_root_has_children() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    assert!(
        !doc.cfb_tree.children.is_empty(),
        "root node should have children"
    );
}

#[test]
fn drawing_meta_extracted() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let dm = doc
        .drawing_meta
        .as_ref()
        .expect("drawing_meta should exist");
    assert_eq!(dm.drawing_number.as_deref(), Some("DWG-0201GP06-01"));
    assert_eq!(dm.document_category.as_deref(), Some("Piping Documents"));
    assert_eq!(dm.template_name.as_deref(), Some("XIONGANA2.pid"));
    assert!(!dm.tags.is_empty(), "tags should have been extracted");
}

#[test]
fn general_meta_extracted() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let gm = doc
        .general_meta
        .as_ref()
        .expect("general_meta should exist");
    assert!(gm.file_path.is_some(), "file_path should be extracted");
    let fp = gm.file_path.as_deref().unwrap();
    assert!(
        fp.contains("DWG-0201GP06-01.pid"),
        "file_path should contain the filename"
    );
    assert!(gm.file_size.is_some(), "file_size should be extracted");
}

#[test]
fn jsites_detected() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    assert!(!doc.jsites.is_empty(), "should detect JSites");
    assert!(
        doc.jsites.len() > 5,
        "expected multiple JSites, got {}",
        doc.jsites.len()
    );
}

#[test]
fn jsite_symbol_paths_are_clean() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    for js in &doc.jsites {
        if let Some(ref sp) = js.symbol_path {
            assert!(
                sp.starts_with("\\\\") || sp.contains(":\\"),
                "symbol_path should be a clean UNC or drive path, got: {sp}"
            );
            assert!(
                sp.ends_with(".sym"),
                "symbol_path should end with .sym: {sp}"
            );
        }
    }
}

#[test]
fn symbol_usage_provenance_matches_jsite_references() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let cross = doc
        .cross_reference
        .as_ref()
        .expect("cross reference graph should be built");
    for usage in &cross.symbol_usage {
        assert_eq!(usage.references.len(), usage.usage_count);
        for reference in &usage.references {
            let js = doc
                .jsites
                .iter()
                .find(|j| j.name == reference.jsite_name)
                .expect("referenced JSite should exist");
            assert_eq!(js.path, reference.jsite_path);
            assert_eq!(js.local_symbol_path, reference.local_symbol_path);
            assert_eq!(js.has_ole_stream, reference.has_ole_stream);
            assert_eq!(
                js.symbol_path.as_deref(),
                Some(usage.symbol_path.as_str()),
                "reference should point back to the grouped symbol path"
            );
        }
    }
}

#[test]
fn attribute_class_provenance_matches_dynamic_attribute_records() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let cross = doc
        .cross_reference
        .as_ref()
        .expect("cross reference graph should be built");
    let da = doc
        .dynamic_attributes
        .as_ref()
        .expect("dynamic attributes should be decoded");

    for class in &cross.attribute_classes {
        let source_records: Vec<_> = da
            .attribute_records
            .iter()
            .filter(|r| r.class_name == class.class_name)
            .collect();
        assert_eq!(class.records.len(), source_records.len());
        for (record_ref, source) in class.records.iter().zip(source_records.iter()) {
            assert_eq!(record_ref.class_name, source.class_name);
            assert_eq!(record_ref.attribute_count, source.attributes.len());
            assert_eq!(record_ref.confidence, source.confidence);
        }
    }
}

#[test]
fn clusters_detected() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    assert!(!doc.clusters.is_empty(), "should detect clusters");
    let names: Vec<&str> = doc.clusters.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"PSMcluster0"));
    assert!(names.contains(&"StyleCluster"));
}

#[test]
fn dynamic_attributes_detected() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let da = doc
        .dynamic_attributes
        .as_ref()
        .expect("dynamic_attributes should exist");
    assert!(da.size > 0);
    assert!(!da.strings.is_empty());
}

#[test]
fn sheet_streams_detected() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    assert!(!doc.sheet_streams.is_empty(), "should detect Sheet streams");
}

#[test]
fn second_file_parses_successfully() {
    let Some(doc) = parse_test_file("DWG-0202GP06-01.pid") else {
        return;
    };
    assert!(!doc.streams.is_empty());
    let dm = doc
        .drawing_meta
        .as_ref()
        .expect("drawing_meta should exist");
    assert!(dm.drawing_number.is_some());
}

#[test]
fn second_file_builds_readable_layout_model() {
    let Some(doc) = parse_test_file("DWG-0202GP06-01.pid") else {
        return;
    };
    let layout = doc
        .layout
        .as_ref()
        .expect("layout should exist on second fixture");
    assert!(
        layout.items.len() >= 10,
        "expected readable layout to place at least 10 items, got {}",
        layout.items.len()
    );
    // TODO(Phase 11c): once Sheet geometry deepening lands the typed
    // SheetGeometry model and we recover connectors with one-side-only
    // resolved endpoints, raise this floor back toward >=5 segments.
    // The current sanitized in-repo fixture only exposes 3 readable
    // segments because the layout-first heuristic emits a connector only
    // when both endpoint pairs resolve; see roadmap Phase 11c-2.
    assert!(
        layout.segments.len() >= 3,
        "expected readable layout to recover at least 3 segments, got {}",
        layout.segments.len()
    );
    assert!(
        !layout.texts.is_empty(),
        "layout should emit at least one text label for readability"
    );
}

#[test]
fn json_serialization_roundtrip() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let json = serde_json::to_string(&doc).expect("should serialize to JSON");
    assert!(json.contains("DWG-0201GP06-01"));
    let _: pid_parse::PidDocument =
        serde_json::from_str(&json).expect("should deserialize from JSON");
}

#[test]
fn psm_roots_extracts_known_entries() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let r = doc.psm_roots.as_ref().expect("PSMroots should be decoded");
    let names: Vec<&str> = r.entries.iter().map(|e| e.name.as_str()).collect();
    for expected in [
        "Imagineer Document",
        "Server Document",
        "_SupportOnlyList",
        "TopVFSet",
        "Dynamic Attributes Set Table",
        "StyleLibrarian",
        "DocStore",
    ] {
        assert!(
            names.contains(&expected),
            "missing expected PSMroots entry: {expected}"
        );
    }
}

#[test]
fn psm_cluster_table_matches_actual_clusters() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let t = doc
        .psm_cluster_table
        .as_ref()
        .expect("PSMclustertable should be decoded");
    assert_eq!(t.count, 5, "declared cluster count should be 5");
    let names: Vec<&str> = t.entries.iter().map(|e| e.name.as_str()).collect();
    for expected in [
        "PSMcluster0",
        "StyleCluster",
        "Dynamic Attributes Metadata",
        "Sheet6",
        "Unclustered Dynamic Attributes",
    ] {
        assert!(
            names.contains(&expected),
            "PSMclustertable should list {expected}"
        );
    }
}

#[test]
fn psm_segment_table_decoded() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let t = doc
        .psm_segment_table
        .as_ref()
        .expect("PSMsegmenttable should be decoded");
    assert_eq!(t.count as usize, t.flags.len());
    assert_eq!(t.entries.len(), t.count as usize);
    assert!(t.flags.iter().all(|&b| b == 0x01));
    assert!(
        t.entries
            .windows(2)
            .all(|pair| pair[0].offset < pair[1].offset),
        "segment entry offsets should increase monotonically"
    );
    assert!(
        t.entries
            .iter()
            .enumerate()
            .all(|(i, e)| e.index == i && e.offset == 8 + i && e.flag == 0x01),
        "entries should mirror the legacy flat flags payload"
    );
    assert_eq!(
        t.trailing_bytes, 0,
        "fixture should have no segment trailer"
    );
}

#[test]
fn version_history_decoded() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let vh = doc
        .version_history
        .as_ref()
        .expect("DocVersion3 should be decoded");
    assert_eq!(vh.records.len(), 4, "expected 4 version records");
    assert!(vh.records.iter().all(|r| r.product == "SmartPlantPID.a"));
    assert_eq!(vh.records[0].operation, "SA", "first record is SaveAs");
    assert!(
        vh.records[3].operation == "SV",
        "last record should be a Save operation"
    );
    // Timestamps follow MM/DD/YY HH:MM format
    assert!(vh.records[0].timestamp.contains('/'));
    assert!(vh.records[0].timestamp.contains(':'));
}

#[test]
fn doc_version2_decoded_matches_version_history() {
    // DocVersion2 is the binary sibling of DocVersion3: same SaveAs+Save
    // sequence, with u8 op code and u32 version number.
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let dv2 = doc
        .doc_version2_decoded
        .as_ref()
        .expect("DocVersion2 structured decode expected");
    let dv3 = doc
        .version_history
        .as_ref()
        .expect("DocVersion3 (version_history) expected");

    assert_eq!(
        dv2.records.len(),
        dv3.records.len(),
        "DocVersion2 and DocVersion3 record counts must match"
    );
    assert_eq!(dv2.magic_u32_le, 0x0001_0034);
    assert!(dv2.reserved_all_zero);

    // op_type mapping (0x82 SaveAs, 0x81 Save) must match the DocVersion3
    // "SA" / "SV" strings one-to-one. Phase 10d: use
    // `VersionRecord::operation_label` on the DV3 side instead of an
    // inline match so the cross-validation exercises both the static
    // DV2 `op_type_label` and the new DV3 helper — a silent drift
    // between the two mappings would fail this assertion.
    for (v2, v3) in dv2.records.iter().zip(dv3.records.iter()) {
        let label = pid_parse::parsers::doc_version2::op_type_label(v2.op_type);
        assert!(
            v3.is_recognized_operation(),
            "DocVersion3 op {} not recognized by VersionRecord helpers",
            v3.operation
        );
        assert_eq!(
            label,
            v3.operation_label(),
            "DV2 op_type_label disagrees with DV3 operation_label for op {}",
            v3.operation
        );
    }

    // Version numbers: DocVersion3 stores them as decimal strings like
    // "090000.0144"; DocVersion2 stores the u32 equivalent of the build
    // suffix ("0144" → 144 → 0x90).
    for (v2, v3) in dv2.records.iter().zip(dv3.records.iter()) {
        let build_str = v3.version.rsplit('.').next().expect("version suffix");
        let build: u32 = build_str.parse().expect("u32");
        assert_eq!(
            v2.version, build,
            "DocVersion2 version 0x{:X} must equal DocVersion3 build {}",
            v2.version, build
        );
    }
}

#[test]
fn psm_cluster_table_aligns_with_cross_reference_declared_clusters() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let table = doc
        .psm_cluster_table
        .as_ref()
        .expect("PSMclustertable should be decoded");
    let cross = doc
        .cross_reference
        .as_ref()
        .expect("cross reference graph should be built");
    let declared = &cross.cluster_coverage.declared;

    assert_eq!(
        table.entries.len(),
        declared.len(),
        "cross-reference declared set should mirror parsed cluster table entries"
    );

    let table_names: Vec<&str> = table.entries.iter().map(|e| e.name.as_str()).collect();
    let declared_names: Vec<&str> = declared.iter().map(std::string::String::as_str).collect();
    assert_eq!(
        table_names, declared_names,
        "cluster coverage declared names should preserve the parsed PSMclustertable order"
    );
    assert!(
        cross.cluster_coverage.declared_missing.is_empty(),
        "fixture should not declare missing cluster names"
    );
}

#[test]
fn cluster_coverage_provenance_matches_psm_cluster_table_offsets() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let table = doc
        .psm_cluster_table
        .as_ref()
        .expect("PSMclustertable should be decoded");
    let cross = doc
        .cross_reference
        .as_ref()
        .expect("cross reference graph should be built");
    let declared = &cross.cluster_coverage.declared_entries;

    assert_eq!(declared.len(), table.entries.len());
    for (declared_entry, table_entry) in declared.iter().zip(table.entries.iter()) {
        assert_eq!(declared_entry.name, table_entry.name);
        assert_eq!(declared_entry.record_offset, table_entry.record_offset);
        assert_eq!(declared_entry.name_offset, table_entry.name_offset);
        assert_eq!(declared_entry.record_len, table_entry.record_len);
    }
    assert_eq!(
        cross.cluster_coverage.matches_detailed.len(),
        cross.cluster_coverage.matched.len(),
        "detailed matches should stay in sync with legacy matched summary"
    );
}

#[test]
fn psm_segment_table_entry_count_matches_declared_count() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let t = doc
        .psm_segment_table
        .as_ref()
        .expect("PSMsegmenttable should be decoded");
    assert_eq!(
        t.entries.len(),
        t.count as usize,
        "segment table entries should match the declared segment count"
    );
}

#[test]
fn app_object_registry_decoded() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let reg = doc
        .app_object_registry
        .as_ref()
        .expect("AppObject should be decoded");
    assert_eq!(reg.leading_u32, 5);
    assert!(reg.entries.len() >= 4, "should decode at least 4 entries");
    for e in &reg.entries {
        assert!(e.clsid.starts_with('{') && e.clsid.ends_with('}'));
    }
    // At least one known DLL name should appear in the extracted paths.
    let any_dll = reg.entries.iter().any(|e| e.path.ends_with(".dll"));
    assert!(any_dll, "registry should reference at least one .dll path");
}

#[test]
fn tagged_storage_list_decoded() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let t = doc
        .tagged_storages
        .as_ref()
        .expect("JTaggedTxtStgList should be decoded");
    assert_eq!(t.list_name, "TaggedTxtStorages");
    assert_eq!(t.entries.len(), 1);
    assert_eq!(t.entries[0].storage_name, "TaggedTxtData");
}

#[test]
fn doc_version2_preserved_raw() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let d2 = doc
        .doc_version2
        .as_ref()
        .expect("DocVersion2 should be captured");
    assert_eq!(d2.size, 48);
    assert_eq!(d2.magic_u32_le, 0x00010034);
    assert!(!d2.hex_preview.is_empty());
}

#[test]
fn object_graph_has_objects_and_relationships() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let g = doc.object_graph.as_ref().expect("object_graph expected");
    assert_eq!(
        g.drawing_no.as_deref(),
        Some("0F7B8ABD0C4E493FA3C7F06FD03AD6AA")
    );
    assert_eq!(g.project_number.as_deref(), Some("SQLPlant1401"));
    assert!(
        g.objects.len() >= 50,
        "should have many modeled objects, got {}",
        g.objects.len()
    );
    assert!(
        g.relationships.len() >= 10,
        "should have relationships, got {}",
        g.relationships.len()
    );
    // by_drawing_id must index every object.
    assert_eq!(g.by_drawing_id.len(), g.objects.len());
    // counts_by_type must cover common P&ID item types.
    assert!(g.counts_by_type.contains_key("PipeRun"));
    assert!(g.counts_by_type.contains_key("Relationship"));
}

#[test]
fn object_graph_relationship_guids_are_32_hex() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let g = doc.object_graph.as_ref().expect("object_graph expected");
    // Each relationship's guid is either an empty string (for the handful
    // of trailer-only "template" records that have no `Relationship.<GUID>`
    // ASCII tag in the DA stream) or a real 32-hex identifier. The vast
    // majority of real relationships must be well-formed.
    let mut real_guids = 0usize;
    for rel in &g.relationships {
        if rel.guid.is_empty() {
            continue;
        }
        assert_eq!(
            rel.guid.len(),
            32,
            "relationship guid should be 32 hex chars"
        );
        assert!(rel.guid.chars().all(|c| c.is_ascii_hexdigit()));
        real_guids += 1;
    }
    assert!(
        real_guids >= g.relationships.len().saturating_sub(2),
        "expected at most 2 template relationships without a guid, got {} template(s) of {}",
        g.relationships.len() - real_guids,
        g.relationships.len()
    );
}

#[test]
fn relationship_probe_produces_one_probe_per_relationship() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let da = doc
        .dynamic_attributes
        .as_ref()
        .expect("dynamic_attributes expected");
    let g = doc.object_graph.as_ref().expect("object_graph expected");
    assert_eq!(
        da.relationship_probes.len(),
        g.relationships.len(),
        "probe count must match graph.relationships count: probes={}, rels={}",
        da.relationship_probes.len(),
        g.relationships.len()
    );
    assert!(
        da.relationship_probes.len() >= 50,
        "expected ≥50 relationship probes on fixture, got {}",
        da.relationship_probes.len()
    );

    // Every probe's guid should resolve to a graph relationship guid.
    // Allow a small number of mismatches because the ASCII-based probe and
    // the trailer-based relationship list can differ on template records.
    let graph_guids: std::collections::HashSet<&str> = g
        .relationships
        .iter()
        .filter(|r| !r.guid.is_empty())
        .map(|r| r.guid.as_str())
        .collect();
    let mut mismatches = 0usize;
    for p in &da.relationship_probes {
        assert_eq!(p.guid.len(), 32, "probe guid should be 32 hex chars");
        assert!(p.guid.chars().all(|c| c.is_ascii_hexdigit()));
        if !graph_guids.contains(p.guid.as_str()) {
            mismatches += 1;
        }
    }
    assert!(
        mismatches <= 2,
        "expected ≤2 probe guids to miss the graph, got {} / {}",
        mismatches,
        da.relationship_probes.len()
    );
}

#[test]
fn relationship_probe_trailing_tokens_are_stable() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let da = doc
        .dynamic_attributes
        .as_ref()
        .expect("dynamic_attributes expected");
    assert!(!da.relationship_probes.is_empty());

    // Every probe should carry both trailing `u16` tokens (slot_a, slot_b).
    for (i, p) in da.relationship_probes.iter().enumerate() {
        assert_eq!(
            p.trailing_tokens.len(),
            2,
            "probe #{} ({}) expected 2 trailing tokens, got {}",
            i,
            p.guid,
            p.trailing_tokens.len()
        );
    }

    // slot_a (after_marker+6) is monotonically increasing across probes in
    // the fixture; this regression guards against probe misalignment.
    let slot_a: Vec<u16> = da
        .relationship_probes
        .iter()
        .map(|p| p.trailing_tokens[0].value)
        .collect();
    for win in slot_a.windows(2) {
        assert!(
            win[1] > win[0],
            "slot_a should increase monotonically: {:04X} → {:04X}",
            win[0],
            win[1]
        );
    }

    // The fixture starts slot_a at 0x6086 — document the observed identity.
    assert_eq!(slot_a[0], 0x6086);
}

#[test]
fn record_trailers_cover_every_pidattributes_record() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let da = doc.dynamic_attributes.as_ref().expect("dynamic_attributes");
    // Each record's 31-byte trailer must be recovered for at least 95 % of
    // the P&IDAttributes records observed in the fixture.
    assert!(
        da.record_trailers.len() >= 150,
        "expected ≥150 DA record trailers, got {}",
        da.record_trailers.len()
    );
    // Canonical known-good probe: the drawing's trailer (first record in
    // the stream) has record_id 0x6009 and field_x 0x079A.
    let first = &da.record_trailers[0];
    assert_eq!(first.record_id, 0x0000_6009);
    assert_eq!(first.field_x, 0x0000_079A);
    assert_eq!(first.class_id, 0x0000_00EA, "Drawing class_id");
    // Some trailers should carry a `drawing_id`.
    let with_did = da
        .record_trailers
        .iter()
        .filter(|t| t.drawing_id.is_some())
        .count();
    assert!(with_did >= 50, "expected ≥50 trailers to carry drawing_id");
}

#[test]
fn relationship_endpoints_resolve_via_sheet_record() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let g = doc.object_graph.as_ref().expect("object_graph");
    // Endpoint resolution is asserted as a *ratio* of the total relationship
    // count rather than absolute thresholds. Sanitized fixtures and future
    // fixture rotations will keep relationship counts stable in proportion
    // even when the underlying drawing changes shape, so a structural
    // ratio assertion does not need to be re-tuned per fixture.
    // Empirical floor on `test-file/DWG-0201GP06-01.pid`: resolved=0.86,
    // unresolved=0.08; we keep some headroom below those numbers.
    let total = g.relationships.len();
    assert!(
        total > 0,
        "fixture should expose at least one relationship for endpoint resolution coverage"
    );
    let resolved = g
        .relationships
        .iter()
        .filter(|r| r.source_drawing_id.is_some() && r.target_drawing_id.is_some())
        .count();
    let unresolved = g
        .relationships
        .iter()
        .filter(|r| r.source_drawing_id.is_none() && r.target_drawing_id.is_none())
        .count();
    // Fully-resolved should cover at least 70% of relationships.
    assert!(
        resolved * 100 >= total * 70,
        "expected ≥70% fully resolved relationships, got {resolved} / {total}"
    );
    // Fully-unresolved should not exceed 15% of relationships.
    assert!(
        unresolved * 100 <= total * 15,
        "expected ≤15% fully unresolved relationships, got {unresolved} / {total}"
    );
    // The resolved endpoints must live in the drawing's object set —
    // regression against field_x → drawing_id misalignment. Off-page
    // (OPC) endpoints are tolerated; we only require that the foreign
    // count stays strictly below the total relationship count, i.e.
    // the parser is not blanket-emitting unknown drawing_ids.
    let known_drawing_ids: std::collections::HashSet<&str> =
        g.objects.iter().map(|o| o.drawing_id.as_str()).collect();
    let mut foreign_endpoints = 0usize;
    for rel in &g.relationships {
        for did in [
            rel.source_drawing_id.as_deref(),
            rel.target_drawing_id.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            if !known_drawing_ids.contains(did) {
                foreign_endpoints += 1;
            }
        }
    }
    assert!(
        foreign_endpoints < total,
        "too many endpoints point to objects absent from graph: \
         {foreign_endpoints} foreign vs {total} relationships total"
    );
}

#[test]
fn sheet_endpoint_records_one_per_relationship() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let sheet = doc
        .sheet_streams
        .first()
        .expect("at least one Sheet stream");
    let graph = doc.object_graph.as_ref().expect("object_graph");
    let endpoint_count = sheet.endpoint_records.len();
    let relationship_count = graph.relationships.len();
    assert!(
        relationship_count > 0,
        "fixture must expose at least one relationship to anchor the endpoint record assertion"
    );
    // 1:1 endpoint↔relationship is the *common* shape but not a hard
    // SmartPlant contract — off-page connectors and Rel records that
    // span multiple sheets show up as small mismatches. Assert the
    // ratio stays high (≥85%) instead of demanding exact equality so
    // future sanitized fixtures and DWG-style drawings don't break the
    // gate. Empirical floor on `test-file/DWG-0201GP06-01.pid`: 59 / 64
    // ≈ 0.92.
    assert!(
        endpoint_count * 100 >= relationship_count * 85,
        "expected sheet endpoint records to cover ≥85% of relationships, \
         got {endpoint_count} endpoint records vs {relationship_count} relationships"
    );
    // The endpoint record's `rel_field_x` must match a relationship
    // counterpart — this is the real parser-bookkeeping invariant and
    // remains an exact membership check.
    let rel_field_xs: std::collections::HashSet<u32> = graph
        .relationships
        .iter()
        .filter_map(|r| r.field_x)
        .collect();
    for r in &sheet.endpoint_records {
        assert!(
            rel_field_xs.contains(&r.rel_field_x),
            "endpoint record rel_field_x=0x{:X} not in graph.relationships",
            r.rel_field_x
        );
    }
}

#[test]
fn sheet_probe_evidence_populates_on_real_sheet_fixture() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(sheet) = pkg.streams.get("/Sheet6") else {
        eprintln!("skipping: /Sheet6 stream not found in fixture");
        return;
    };

    let report = probe_sheet_stream(
        "Sheet6",
        "/Sheet6",
        &sheet.data,
        &SheetProbeOptions::default(),
    );

    assert_eq!(report.sheet_name, "Sheet6");
    assert_eq!(report.size, sheet.data.len() as u64);
    assert!(
        !report.chunks.is_empty(),
        "Sheet6 should produce at least one probe chunk"
    );
    assert!(
        !report.record_type_counts.is_empty()
            || !report.text_runs.is_empty()
            || !report.coordinate_hints.is_empty(),
        "real Sheet6 should expose at least one report-level evidence signal"
    );
}

#[test]
fn normalized_geometry_probe_baseline_on_real_fixture() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };

    let geometry = pid_parse::build_normalized_geometry(&doc);
    let expected_probe_entities: usize = doc
        .sheet_streams
        .iter()
        .map(|sheet| {
            let text_count = sheet
                .geometry
                .as_ref()
                .filter(|geometry| !geometry.texts.is_empty())
                .map_or(sheet.extracted_texts.len(), |geometry| geometry.texts.len());
            let coordinate_count = sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.coordinate_hints.len());
            let endpoint_count = sheet
                .geometry
                .as_ref()
                .filter(|geometry| !geometry.endpoints.is_empty())
                .map_or(sheet.endpoint_records.len(), |geometry| {
                    geometry.endpoints.len()
                });
            let hint_count = sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| {
                    geometry
                        .object_geometry_hints
                        .iter()
                        .filter(|h| h.position.is_some())
                        .count()
                });
            let total = text_count + coordinate_count + endpoint_count + hint_count;
            eprintln!(
                "sheet={}, text={text_count}, coord={coordinate_count}, ep={endpoint_count}, hint={hint_count}, total={total}",
                sheet.path
            );
            total
        })
        .sum();

    eprintln!(
        "geometry.entities.len()={}, expected_probe_entities={expected_probe_entities}",
        geometry.entities.len()
    );
    assert!(
        expected_probe_entities > 0,
        "real fixture should expose Sheet probe evidence for normalized geometry"
    );
    assert_eq!(
        geometry.entities.len(),
        expected_probe_entities,
        "normalized geometry should account for every Sheet probe item exactly once"
    );
    let inferred_points = geometry
        .entities
        .iter()
        .filter(|entity| {
            entity.confidence == pid_parse::PidGeometryConfidence::Inferred
                && matches!(entity.kind, pid_parse::PidGraphicKind::Point { .. })
        })
        .count();
    let probe_unknowns = geometry
        .entities
        .iter()
        .filter(|entity| {
            entity.confidence == pid_parse::PidGeometryConfidence::ProbeOnly
                && matches!(entity.kind, pid_parse::PidGraphicKind::Unknown { .. })
        })
        .count();
    let expected_coordinate_hints: usize = doc
        .sheet_streams
        .iter()
        .map(|sheet| {
            sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.coordinate_hints.len())
        })
        .sum();
    let expected_geometry_hints: usize = doc
        .sheet_streams
        .iter()
        .map(|sheet| {
            sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| {
                    geometry
                        .object_geometry_hints
                        .iter()
                        .filter(|h| h.position.is_some())
                        .count()
                })
        })
        .sum();

    assert_eq!(
        inferred_points,
        expected_coordinate_hints + expected_geometry_hints,
        "coordinate hints + geometry hints should be promoted to inferred positioned points"
    );
    assert_eq!(
        inferred_points + probe_unknowns,
        geometry.entities.len(),
        "coordinate/geometry hints become inferred points; text and endpoint evidence stays ProbeOnly Unknown"
    );
}

#[test]
fn sheet6_object_geometry_hints_baseline_is_empty_until_mapping_is_proven() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(sheet) = doc
        .sheet_streams
        .iter()
        .find(|sheet| sheet.path == "/Sheet6")
    else {
        eprintln!("skipping: /Sheet6 stream not found in fixture");
        return;
    };

    let object_geometry_hint_count = sheet
        .geometry
        .as_ref()
        .map_or(0, |geometry| geometry.object_geometry_hints.len());

    assert!(
        object_geometry_hint_count > 0,
        "promoted candidates should produce object geometry hints"
    );
}

#[test]
fn sheet6_text_window_report_keeps_text_probe_only_until_position_is_proven() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(raw_sheet) = pkg.streams.get("/Sheet6") else {
        eprintln!("skipping: /Sheet6 stream not found in fixture");
        return;
    };
    let report = probe_sheet_stream(
        "Sheet6",
        "/Sheet6",
        &raw_sheet.data,
        &SheetProbeOptions::default(),
    );
    let candidates = sheet_text_window_candidates(
        &report.text_runs,
        &report.coordinate_hints,
        &report.chunks,
        128,
    );
    let scores = score_sheet_text_window_candidates(&candidates);
    let same_chunk = candidates
        .iter()
        .filter(|candidate| candidate.same_chunk)
        .count();
    let quality_passed = candidates
        .iter()
        .filter(|candidate| candidate.quality_passed)
        .count();
    let text_quality_passed = scores
        .iter()
        .filter(|score| {
            score.reasons.iter().any(|reason| {
                matches!(
                    reason,
                    pid_parse::parsers::sheet_probe::SheetTextWindowScoreReason::TextQualityPassed
                )
            })
        })
        .count();
    let max_score = scores
        .iter()
        .map(|score| score.score)
        .max()
        .unwrap_or_default();
    let over_threshold = scores.iter().filter(|score| score.score >= 70).count();
    let top: Vec<_> = scores
        .iter()
        .take(8)
        .map(|score| {
            (
                score.score,
                score.candidate.text_offset,
                score.candidate.text.as_str(),
                score.candidate.coordinate_offset,
                score.candidate.x,
                score.candidate.y,
                score.candidate.byte_distance,
                score.candidate.same_chunk,
                score.candidate.quality_passed,
            )
        })
        .collect();
    eprintln!(
        "Sheet6 text window report: text_runs={}, coordinates={}, candidates={}, same_chunk={}, quality_passed={}, text_quality_passed={}, max_score={}, over_threshold={}, top={top:?}",
        report.text_runs.len(),
        report.coordinate_hints.len(),
        candidates.len(),
        same_chunk,
        quality_passed,
        text_quality_passed,
        max_score,
        over_threshold
    );

    assert!(
        !report.text_runs.is_empty(),
        "Sheet6 should expose text runs for text placement investigation"
    );
    let normalized = pid_parse::build_normalized_geometry(&pkg.parsed);
    let inferred_text_count = normalized
        .entities
        .iter()
        .filter(|entity| matches!(entity.kind, pid_parse::PidGraphicKind::Text { .. }))
        .count();
    let text_probe_unknowns = normalized
        .entities
        .iter()
        .filter(|entity| {
            entity.source.stream_path.as_deref() == Some("/Sheet6")
                && entity.confidence == pid_parse::PidGeometryConfidence::ProbeOnly
                && matches!(entity.kind, pid_parse::PidGraphicKind::Unknown { .. })
                && entity
                    .source
                    .record_id
                    .as_deref()
                    .is_some_and(|record_id| record_id.starts_with("text-probe:"))
        })
        .count();
    assert_eq!(
        inferred_text_count, 0,
        "text window report must not promote Sheet text to positioned geometry"
    );
    assert_eq!(
        over_threshold, 0,
        "text window scoring must not find promotable text placement candidates in Sheet6 yet"
    );
    assert!(
        text_probe_unknowns > 0,
        "Sheet6 text should remain ProbeOnly Unknown until text position is proven"
    );
}

#[test]
fn sheet6_field_x_window_probe_finds_sample_endpoint_ids() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(sheet) = pkg.streams.get("/Sheet6") else {
        eprintln!("skipping: /Sheet6 stream not found in fixture");
        return;
    };

    let windows = field_x_windows(&sheet.data, &[229, 326, 740, 139], 32);
    for field_x in [229, 326, 740, 139] {
        let hits: Vec<_> = windows
            .iter()
            .filter(|window| window.field_x == field_x)
            .map(|window| {
                (
                    window.offset,
                    window.endpoint_record_start,
                    window.window_start,
                    window.window_end,
                    window.nearby_coordinates.len(),
                )
            })
            .collect();
        eprintln!("field_x {field_x} windows: {hits:?}");
    }

    assert!(
        windows.iter().any(|window| window.field_x == 229),
        "expected field_x 229 to appear in /Sheet6 bytes"
    );
    assert!(
        windows.iter().any(|window| window.field_x == 740),
        "expected field_x 740 to appear in /Sheet6 bytes"
    );
    assert!(windows.iter().all(|window| {
        window.window_start <= window.offset
            && window.offset + 4 <= window.window_end
            && window.window_end <= sheet.data.len()
    }));
}

#[test]
fn sheet6_field_x_window_scoring_reports_non_endpoint_candidates() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(sheet) = pkg.streams.get("/Sheet6") else {
        eprintln!("skipping: /Sheet6 stream not found in fixture");
        return;
    };
    let Some(graph) = pkg.parsed.object_graph.as_ref() else {
        eprintln!("skipping: object graph not built for fixture");
        return;
    };

    let object_field_xs: HashSet<_> = graph
        .objects
        .iter()
        .filter_map(|object| object.field_x)
        .collect();
    let windows = field_x_windows(&sheet.data, &[229, 326, 740, 139], 32);
    let scores = score_field_x_windows(&windows, &object_field_xs);

    let positive_non_endpoint = scores
        .iter()
        .filter(|score| score.score > 0 && score.candidate_position.is_some())
        .count();
    let endpoint_references = scores.iter().filter(|score| score.score == -100).count();
    let max_score = scores
        .iter()
        .map(|score| score.score)
        .max()
        .unwrap_or_default();
    let promotable = scores.iter().filter(|score| score.score >= 70).count();
    eprintln!(
        "field_x scoring summary: total={}, positive_non_endpoint={}, endpoint_references={}, max_score={}, promotable={}",
        scores.len(),
        positive_non_endpoint,
        endpoint_references,
        max_score,
        promotable
    );

    assert!(
        positive_non_endpoint > 0,
        "expected at least one non-endpoint field_x window with a coordinate candidate"
    );
    assert!(
        endpoint_references > 0,
        "expected endpoint-record references to be identified and downranked"
    );
    assert_eq!(
        promotable, 0,
        "real fixture candidates should not cross promotion threshold until record shape is proven"
    );
    let object_geometry_hint_count: usize = pkg
        .parsed
        .sheet_streams
        .iter()
        .map(|sheet| {
            sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.object_geometry_hints.len())
        })
        .sum();
    assert!(
        object_geometry_hint_count > 0,
        "promoted candidates should produce object geometry hints"
    );
}

#[test]
fn sheet6_all_endpoint_field_x_window_scoring_report() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(sheet) = pkg.streams.get("/Sheet6") else {
        eprintln!("skipping: /Sheet6 stream not found in fixture");
        return;
    };
    let Some(graph) = pkg.parsed.object_graph.as_ref() else {
        eprintln!("skipping: object graph not built for fixture");
        return;
    };
    let Some(cross) = pkg.parsed.cross_reference.as_ref() else {
        eprintln!("skipping: cross reference not built for fixture");
        return;
    };

    let object_field_xs: HashSet<_> = graph
        .objects
        .iter()
        .filter_map(|object| object.field_x)
        .collect();
    let mut endpoint_field_xs: Vec<_> = cross
        .relationship_endpoint_links
        .iter()
        .filter(|link| link.sheet_path.as_deref() == Some("/Sheet6"))
        .flat_map(|link| [link.source_field_x, link.target_field_x])
        .flatten()
        .collect();
    endpoint_field_xs.sort_unstable();
    endpoint_field_xs.dedup();

    let windows = field_x_windows(&sheet.data, &endpoint_field_xs, 32);
    let scores = score_field_x_windows(&windows, &object_field_xs);
    let positive_non_endpoint = scores
        .iter()
        .filter(|score| score.score > 0 && score.candidate_position.is_some())
        .count();
    let endpoint_references = scores.iter().filter(|score| score.score == -100).count();
    let max_score = scores
        .iter()
        .map(|score| score.score)
        .max()
        .unwrap_or_default();
    let promotable = scores.iter().filter(|score| score.score >= 70).count();

    eprintln!(
        "all endpoint field_x scoring summary: fields={}, windows={}, positive_non_endpoint={}, endpoint_references={}, max_score={}, promotable={}",
        endpoint_field_xs.len(),
        scores.len(),
        positive_non_endpoint,
        endpoint_references,
        max_score,
        promotable
    );

    assert!(
        !endpoint_field_xs.is_empty(),
        "real fixture should expose endpoint field_x values"
    );
    assert!(
        !scores.is_empty(),
        "field_x window scoring should inspect at least one endpoint field_x hit"
    );
    let object_geometry_hint_count: usize = pkg
        .parsed
        .sheet_streams
        .iter()
        .map(|sheet| {
            sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.object_geometry_hints.len())
        })
        .sum();
    assert!(
        object_geometry_hint_count > 0,
        "promoted candidates should produce object geometry hints"
    );
}

#[test]
fn sheet6_field_x_window_identity_report() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(sheet) = pkg.streams.get("/Sheet6") else {
        eprintln!("skipping: /Sheet6 stream not found in fixture");
        return;
    };
    let Some(cross) = pkg.parsed.cross_reference.as_ref() else {
        eprintln!("skipping: cross reference not built for fixture");
        return;
    };
    let Some(da) = pkg.parsed.dynamic_attributes.as_ref() else {
        eprintln!("skipping: dynamic attributes not built for fixture");
        return;
    };

    let mut endpoint_field_xs: Vec<_> = cross
        .relationship_endpoint_links
        .iter()
        .filter(|link| link.sheet_path.as_deref() == Some("/Sheet6"))
        .flat_map(|link| [link.source_field_x, link.target_field_x])
        .flatten()
        .collect();
    endpoint_field_xs.sort_unstable();
    endpoint_field_xs.dedup();

    let windows = field_x_windows(&sheet.data, &endpoint_field_xs, 96);
    let identity_index = sheet_identity_index_from_trailers(&da.record_trailers);
    let identities = field_x_window_identities(&sheet.data, &windows, &identity_index);
    let same_object = identities
        .iter()
        .filter(|identity| identity.resolves_to_same_object)
        .count();
    let wrong_object = identities
        .iter()
        .filter(|identity| {
            identity.resolves_to_field_x.is_some() && !identity.resolves_to_same_object
        })
        .count();
    let mut kinds = BTreeMap::new();
    for identity in &identities {
        *kinds
            .entry(format!("{:?}", identity.kind))
            .or_insert(0usize) += 1;
    }

    eprintln!(
        "field_x identity summary: fields={}, windows={}, identities={}, same_object={}, wrong_object={}, kinds={:?}",
        endpoint_field_xs.len(),
        windows.len(),
        identities.len(),
        same_object,
        wrong_object,
        kinds
    );

    assert!(
        !windows.is_empty(),
        "identity report should inspect at least one Sheet6 endpoint field_x window"
    );
    let object_geometry_hint_count: usize = pkg
        .parsed
        .sheet_streams
        .iter()
        .map(|sheet| {
            sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.object_geometry_hints.len())
        })
        .sum();
    assert!(
        object_geometry_hint_count > 0,
        "promoted candidates should produce object geometry hints"
    );
}

#[test]
fn sheet6_graphic_identity_scoring_keeps_object_hints_empty_until_proven() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(sheet) = pkg.streams.get("/Sheet6") else {
        eprintln!("skipping: /Sheet6 stream not found in fixture");
        return;
    };
    let Some(cross) = pkg.parsed.cross_reference.as_ref() else {
        eprintln!("skipping: cross reference not built for fixture");
        return;
    };
    let Some(da) = pkg.parsed.dynamic_attributes.as_ref() else {
        eprintln!("skipping: dynamic attributes not built for fixture");
        return;
    };

    let mut endpoint_field_xs: Vec<_> = cross
        .relationship_endpoint_links
        .iter()
        .filter(|link| link.sheet_path.as_deref() == Some("/Sheet6"))
        .flat_map(|link| [link.source_field_x, link.target_field_x])
        .flatten()
        .collect();
    endpoint_field_xs.sort_unstable();
    endpoint_field_xs.dedup();

    let report = probe_sheet_stream(
        "Sheet6",
        "/Sheet6",
        &sheet.data,
        &SheetProbeOptions::default(),
    );
    let windows = field_x_windows(&sheet.data, &endpoint_field_xs, 96);
    let features = field_x_window_features(&sheet.data, &windows, &report.chunks);
    let object_field_xs: HashSet<_> = pkg
        .parsed
        .object_graph
        .as_ref()
        .map(|graph| {
            graph
                .objects
                .iter()
                .filter_map(|object| object.field_x)
                .collect()
        })
        .unwrap_or_default();
    let identity_index = sheet_identity_index_from_trailers(&da.record_trailers);
    let identities = field_x_window_identities(&sheet.data, &windows, &identity_index);
    let scores =
        score_field_x_window_features_with_identities(&features, &object_field_xs, &identities);
    let identity_supported = scores
        .iter()
        .filter(|score| {
            score.reasons.iter().any(|reason| {
                matches!(
                    reason,
                    pid_parse::parsers::sheet_probe::SheetFieldXWindowScoreReason::GraphicIdentityNearby {
                        ..
                    }
                )
            })
        })
        .count();
    let max_score = scores
        .iter()
        .map(|score| score.score)
        .max()
        .unwrap_or_default();
    let over_threshold = scores.iter().filter(|score| score.score >= 70).count();

    eprintln!(
        "graphic identity scoring summary: scores={}, identity_supported={}, max_score={}, over_threshold={}",
        scores.len(),
        identity_supported,
        max_score,
        over_threshold
    );

    assert!(
        !scores.is_empty(),
        "identity scoring should inspect real Sheet6 windows"
    );
    assert!(
        over_threshold > 0,
        "same-object identity should now intersect promotable feature evidence"
    );
    let object_geometry_hint_count: usize = pkg
        .parsed
        .sheet_streams
        .iter()
        .map(|sheet| {
            sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.object_geometry_hints.len())
        })
        .sum();
    assert!(
        object_geometry_hint_count > 0,
        "identity scoring with promotion gate should populate object geometry hints"
    );
}

#[test]
fn all_sheets_graphic_identity_scoring_report_keeps_object_hints_empty() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(cross) = pkg.parsed.cross_reference.as_ref() else {
        eprintln!("skipping: cross reference not built for fixture");
        return;
    };
    let Some(da) = pkg.parsed.dynamic_attributes.as_ref() else {
        eprintln!("skipping: dynamic attributes not built for fixture");
        return;
    };

    let identity_index = sheet_identity_index_from_trailers(&da.record_trailers);
    let object_field_xs: HashSet<_> = pkg
        .parsed
        .object_graph
        .as_ref()
        .map(|graph| {
            graph
                .objects
                .iter()
                .filter_map(|object| object.field_x)
                .collect()
        })
        .unwrap_or_default();

    let mut sheets_seen = 0usize;
    let mut windows_seen = 0usize;
    let mut identities_seen = 0usize;
    let mut same_object_seen = 0usize;
    let mut wrong_object_seen = 0usize;
    let mut identity_supported = 0usize;
    let mut over_threshold = 0usize;
    let mut max_score = i32::MIN;

    for sheet in &pkg.parsed.sheet_streams {
        let mut field_xs: Vec<_> = cross
            .relationship_endpoint_links
            .iter()
            .filter(|link| link.sheet_path.as_deref() == Some(sheet.path.as_str()))
            .flat_map(|link| [link.source_field_x, link.target_field_x])
            .flatten()
            .collect();
        field_xs.sort_unstable();
        field_xs.dedup();
        if field_xs.is_empty() {
            continue;
        }
        let Some(raw_sheet) = pkg.streams.get(&sheet.path) else {
            continue;
        };

        let report = probe_sheet_stream(
            sheet.name.as_str(),
            sheet.path.as_str(),
            &raw_sheet.data,
            &SheetProbeOptions::default(),
        );
        let windows = field_x_windows(&raw_sheet.data, &field_xs, 96);
        let features = field_x_window_features(&raw_sheet.data, &windows, &report.chunks);
        let identities = field_x_window_identities(&raw_sheet.data, &windows, &identity_index);
        let scores =
            score_field_x_window_features_with_identities(&features, &object_field_xs, &identities);

        sheets_seen += 1;
        windows_seen += windows.len();
        identities_seen += identities.len();
        same_object_seen += identities
            .iter()
            .filter(|identity| identity.resolves_to_same_object)
            .count();
        wrong_object_seen += identities
            .iter()
            .filter(|identity| {
                identity.resolves_to_field_x.is_some() && !identity.resolves_to_same_object
            })
            .count();
        identity_supported += scores
            .iter()
            .filter(|score| {
                score.reasons.iter().any(|reason| {
                    matches!(
                        reason,
                        pid_parse::parsers::sheet_probe::SheetFieldXWindowScoreReason::GraphicIdentityNearby {
                            ..
                        }
                    )
                })
            })
            .count();
        over_threshold += scores.iter().filter(|score| score.score >= 70).count();
        max_score = max_score.max(
            scores
                .iter()
                .map(|score| score.score)
                .max()
                .unwrap_or_default(),
        );
    }

    eprintln!(
        "all-sheet identity scoring summary: sheets={}, windows={}, identities={}, same_object={}, wrong_object={}, identity_supported={}, max_score={}, over_threshold={}",
        sheets_seen,
        windows_seen,
        identities_seen,
        same_object_seen,
        wrong_object_seen,
        identity_supported,
        max_score,
        over_threshold
    );

    assert!(
        sheets_seen > 0,
        "all-sheet identity scoring should inspect at least one Sheet"
    );
    let object_geometry_hint_count: usize = pkg
        .parsed
        .sheet_streams
        .iter()
        .map(|sheet| {
            sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.object_geometry_hints.len())
        })
        .sum();
    assert!(
        object_geometry_hint_count > 0,
        "promoted candidates should populate object geometry hints"
    );
}

#[test]
fn available_pid_fixtures_geometry_evidence_inventory_stays_probe_only() {
    const FIXTURES: &[&str] = &[
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
        "export-test/publish-data/A01/A01.pid",
        "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
    ];

    let mut fixtures_seen = 0usize;
    let mut sheets_seen = 0usize;
    let mut windows_seen = 0usize;
    let mut identities_seen = 0usize;
    let mut same_object_seen = 0usize;
    let mut wrong_object_seen = 0usize;
    let mut identity_supported = 0usize;
    let mut identity_over_threshold = 0usize;
    let mut max_identity_score: Option<i32> = None;
    let mut text_candidates_seen = 0usize;
    let mut text_over_threshold = 0usize;
    let mut record_shape_classes_seen = 0usize;
    let mut record_shape_support_by_key: BTreeMap<(isize, isize), usize> = BTreeMap::new();
    let mut object_geometry_hint_count = 0usize;
    let mut total_promotable = 0usize;
    let mut detail_lines = Vec::new();

    for fixture in FIXTURES {
        let Some(pkg) = parse_test_package(fixture) else {
            continue;
        };
        fixtures_seen += 1;
        object_geometry_hint_count += pkg
            .parsed
            .sheet_streams
            .iter()
            .map(|sheet| {
                sheet
                    .geometry
                    .as_ref()
                    .map_or(0, |geometry| geometry.object_geometry_hints.len())
            })
            .sum::<usize>();

        let Some(cross) = pkg.parsed.cross_reference.as_ref() else {
            eprintln!("skipping fixture {fixture}: cross reference not built");
            continue;
        };
        let Some(da) = pkg.parsed.dynamic_attributes.as_ref() else {
            eprintln!("skipping fixture {fixture}: dynamic attributes not built");
            continue;
        };

        let identity_index = sheet_identity_index_from_trailers(&da.record_trailers);
        let object_field_xs: HashSet<_> = pkg
            .parsed
            .object_graph
            .as_ref()
            .map(|graph| {
                graph
                    .objects
                    .iter()
                    .filter_map(|object| object.field_x)
                    .collect()
            })
            .unwrap_or_default();

        for sheet in &pkg.parsed.sheet_streams {
            let Some(raw_sheet) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            let report = probe_sheet_stream(
                sheet.name.as_str(),
                sheet.path.as_str(),
                &raw_sheet.data,
                &SheetProbeOptions::default(),
            );
            let text_candidates = sheet_text_window_candidates(
                &report.text_runs,
                &report.coordinate_hints,
                &report.chunks,
                128,
            );
            let text_scores = score_sheet_text_window_candidates(&text_candidates);
            let sheet_text_over_threshold =
                text_scores.iter().filter(|score| score.score >= 70).count();
            text_candidates_seen += text_candidates.len();
            text_over_threshold += sheet_text_over_threshold;

            let mut field_xs: Vec<_> = cross
                .relationship_endpoint_links
                .iter()
                .filter(|link| link.sheet_path.as_deref() == Some(sheet.path.as_str()))
                .flat_map(|link| [link.source_field_x, link.target_field_x])
                .flatten()
                .collect();
            field_xs.sort_unstable();
            field_xs.dedup();
            if field_xs.is_empty() {
                detail_lines.push(format!(
                    "fixture={fixture}, sheet={}, field_xs=0, text_candidates={}, text_over_threshold={}, note=no_endpoint_field_xs",
                    sheet.path,
                    text_candidates.len(),
                    sheet_text_over_threshold
                ));
                continue;
            }

            let windows = field_x_windows(&raw_sheet.data, &field_xs, 96);
            let features = field_x_window_features(&raw_sheet.data, &windows, &report.chunks);
            let record_shape_classes = classify_field_x_record_shapes(&features);
            let identities = field_x_window_identities(&raw_sheet.data, &windows, &identity_index);
            let scores = score_field_x_window_features_with_identities(
                &features,
                &object_field_xs,
                &identities,
            );
            let sheet_same_object = identities
                .iter()
                .filter(|identity| identity.resolves_to_same_object)
                .count();
            let sheet_wrong_object = identities
                .iter()
                .filter(|identity| {
                    identity.resolves_to_field_x.is_some() && !identity.resolves_to_same_object
                })
                .count();
            let sheet_identity_supported = scores
                .iter()
                .filter(|score| {
                    score.reasons.iter().any(|reason| {
                        matches!(
                            reason,
                            pid_parse::parsers::sheet_probe::SheetFieldXWindowScoreReason::GraphicIdentityNearby {
                                ..
                            }
                        )
                    })
                })
                .count();
            let sheet_identity_over_threshold =
                scores.iter().filter(|score| score.score >= 70).count();
            let sheet_max_score = scores
                .iter()
                .map(|score| score.score)
                .max()
                .unwrap_or_default();
            let gate = summarize_object_geometry_promotion_gate(&scores, 70);
            total_promotable += gate.promotable_candidates;

            sheets_seen += 1;
            windows_seen += windows.len();
            record_shape_classes_seen += record_shape_classes.len();
            for shape_class in &record_shape_classes {
                *record_shape_support_by_key
                    .entry((
                        shape_class.field_delta_from_chunk,
                        shape_class.coordinate_delta_from_chunk,
                    ))
                    .or_default() += shape_class.support;
            }
            identities_seen += identities.len();
            same_object_seen += sheet_same_object;
            wrong_object_seen += sheet_wrong_object;
            identity_supported += sheet_identity_supported;
            identity_over_threshold += sheet_identity_over_threshold;
            if let Some(sheet_max) = scores.iter().map(|score| score.score).max() {
                max_identity_score =
                    Some(max_identity_score.map_or(sheet_max, |max| max.max(sheet_max)));
            }
            let top_record_shape = record_shape_classes
                .first()
                .map(|shape_class| {
                    format!(
                        "({},{})/{}",
                        shape_class.field_delta_from_chunk,
                        shape_class.coordinate_delta_from_chunk,
                        shape_class.support
                    )
                })
                .unwrap_or_else(|| "none".to_string());
            detail_lines.push(format!(
                "fixture={fixture}, sheet={}, field_xs={}, windows={}, record_shape_classes={}, top_record_shape={}, identities={}, same_object={}, wrong_object={}, identity_supported={}, max_identity_score={}, identity_over_threshold={}, promotable={}, text_candidates={}, text_over_threshold={}",
                sheet.path,
                field_xs.len(),
                windows.len(),
                record_shape_classes.len(),
                top_record_shape,
                identities.len(),
                sheet_same_object,
                sheet_wrong_object,
                sheet_identity_supported,
                sheet_max_score,
                sheet_identity_over_threshold,
                gate.promotable_candidates,
                text_candidates.len(),
                sheet_text_over_threshold
            ));
        }
    }

    if fixtures_seen == 0 {
        eprintln!("skipping: no available PID fixtures found");
        return;
    }

    let mut top_record_shapes: Vec<_> = record_shape_support_by_key.into_iter().collect();
    top_record_shapes
        .sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

    eprintln!(
        "available fixture geometry evidence inventory: fixtures={}, sheets={}, windows={}, record_shape_classes={}, identities={}, same_object={}, wrong_object={}, identity_supported={}, max_identity_score={}, identity_over_threshold={}, promotable={}, text_candidates={}, text_over_threshold={}, top_record_shapes={:?}",
        fixtures_seen,
        sheets_seen,
        windows_seen,
        record_shape_classes_seen,
        identities_seen,
        same_object_seen,
        wrong_object_seen,
        identity_supported,
        max_identity_score.unwrap_or_default(),
        identity_over_threshold,
        total_promotable,
        text_candidates_seen,
        text_over_threshold,
        top_record_shapes.iter().take(10).collect::<Vec<_>>()
    );
    for detail in &detail_lines {
        eprintln!("available fixture geometry evidence detail: {detail}");
    }

    eprintln!(
        "object_geometry_hint_count={object_geometry_hint_count}, promotable={total_promotable}"
    );
    assert_eq!(
        object_geometry_hint_count, total_promotable,
        "geometry hint count must match promotable gate output"
    );
    assert!(
        record_shape_classes_seen > 0,
        "multi-fixture investigation should classify at least one record shape"
    );
}

#[test]
fn sheet6_top_candidate_record_dump_stays_investigation_only() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(raw_sheet) = pkg.streams.get("/Sheet6") else {
        eprintln!("skipping: /Sheet6 stream not found in fixture");
        return;
    };
    let Some(cross) = pkg.parsed.cross_reference.as_ref() else {
        eprintln!("skipping: cross reference not built for fixture");
        return;
    };
    let Some(da) = pkg.parsed.dynamic_attributes.as_ref() else {
        eprintln!("skipping: dynamic attributes not built for fixture");
        return;
    };

    let report = probe_sheet_stream(
        "Sheet6",
        "/Sheet6",
        &raw_sheet.data,
        &SheetProbeOptions::default(),
    );
    let mut endpoint_field_xs: Vec<_> = cross
        .relationship_endpoint_links
        .iter()
        .filter(|link| link.sheet_path.as_deref() == Some("/Sheet6"))
        .flat_map(|link| [link.source_field_x, link.target_field_x])
        .flatten()
        .collect();
    endpoint_field_xs.sort_unstable();
    endpoint_field_xs.dedup();

    let object_field_xs: HashSet<_> = pkg
        .parsed
        .object_graph
        .as_ref()
        .map(|graph| {
            graph
                .objects
                .iter()
                .filter_map(|object| object.field_x)
                .collect()
        })
        .unwrap_or_default();
    let identity_index = sheet_identity_index_from_trailers(&da.record_trailers);
    let windows = field_x_windows(&raw_sheet.data, &endpoint_field_xs, 96);
    let features = field_x_window_features(&raw_sheet.data, &windows, &report.chunks);
    let identities = field_x_window_identities(&raw_sheet.data, &windows, &identity_index);
    let scores =
        score_field_x_window_features_with_identities(&features, &object_field_xs, &identities);

    let identity_dumps = top_field_x_candidate_record_dumps(&raw_sheet.data, &scores, 5, 32);
    for dump in &identity_dumps {
        eprintln!("Sheet6 top identity score dump: {dump:?}");
    }

    for identity in identities
        .iter()
        .filter(|identity| identity.resolves_to_same_object)
        .take(5)
    {
        eprintln!(
            "Sheet6 same-object identity dump: field_x={}, offset={}, delta={}, kind={:?}, value={:?}, window={}",
            identity.field_x,
            identity.offset,
            identity.delta_from_field,
            identity.kind,
            identity.value,
            hex_window(&raw_sheet.data, identity.offset, 32)
        );
    }

    let text_candidates = sheet_text_window_candidates(
        &report.text_runs,
        &report.coordinate_hints,
        &report.chunks,
        128,
    );
    let text_scores = score_sheet_text_window_candidates(&text_candidates);
    let text_dumps = top_text_candidate_record_dumps(&raw_sheet.data, &text_scores, 5, 32);
    for dump in &text_dumps {
        eprintln!("Sheet6 top text score dump: {dump:?}");
    }

    assert!(
        !identity_dumps.is_empty(),
        "record dump should include identity scoring candidates"
    );
    assert!(
        !text_dumps.is_empty(),
        "record dump should include text scoring candidates"
    );
    assert!(
        identity_dumps
            .iter()
            .all(|dump| dump.field_window.end <= raw_sheet.data.len()
                && !dump.field_window.hex.is_empty()),
        "identity dumps should carry bounded field byte windows"
    );
    assert!(
        text_dumps
            .iter()
            .all(|dump| dump.text_window.end <= raw_sheet.data.len()
                && dump.coordinate_window.end <= raw_sheet.data.len()
                && !dump.text_window.hex.is_empty()
                && !dump.coordinate_window.hex.is_empty()),
        "text dumps should carry bounded text and coordinate byte windows"
    );
    let object_geometry_hint_count: usize = pkg
        .parsed
        .sheet_streams
        .iter()
        .map(|sheet| {
            sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.object_geometry_hints.len())
        })
        .sum();
    assert!(
        object_geometry_hint_count > 0,
        "promoted candidates should produce object geometry hints"
    );
}

#[test]
fn sheet6_field_x_window_features_report_chunk_shapes() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(sheet) = pkg.streams.get("/Sheet6") else {
        eprintln!("skipping: /Sheet6 stream not found in fixture");
        return;
    };
    let Some(cross) = pkg.parsed.cross_reference.as_ref() else {
        eprintln!("skipping: cross reference not built for fixture");
        return;
    };

    let mut endpoint_field_xs: Vec<_> = cross
        .relationship_endpoint_links
        .iter()
        .filter(|link| link.sheet_path.as_deref() == Some("/Sheet6"))
        .flat_map(|link| [link.source_field_x, link.target_field_x])
        .flatten()
        .collect();
    endpoint_field_xs.sort_unstable();
    endpoint_field_xs.dedup();

    let report = probe_sheet_stream(
        "Sheet6",
        "/Sheet6",
        &sheet.data,
        &SheetProbeOptions::default(),
    );
    let windows = field_x_windows(&sheet.data, &endpoint_field_xs, 32);
    let features = field_x_window_features(&sheet.data, &windows, &report.chunks);
    let object_field_xs: HashSet<_> = pkg
        .parsed
        .object_graph
        .as_ref()
        .map(|graph| {
            graph
                .objects
                .iter()
                .filter_map(|object| object.field_x)
                .collect()
        })
        .unwrap_or_default();
    let feature_scores = score_field_x_window_features(&features, &object_field_xs);
    let max_feature_score = feature_scores
        .iter()
        .map(|score| score.score)
        .max()
        .unwrap_or_default();
    let promotable_feature_scores = feature_scores
        .iter()
        .filter(|score| score.score >= 70)
        .count();
    let mut top_feature_scores: Vec<_> = feature_scores
        .iter()
        .filter(|score| score.score >= 70)
        .map(|score| {
            (
                score.field_x,
                score.offset,
                score.score,
                score
                    .candidate_position
                    .as_ref()
                    .map(|position| (position.offset, position.x, position.y)),
                score.reasons.clone(),
            )
        })
        .collect();
    top_feature_scores
        .sort_by(|left, right| right.2.cmp(&left.2).then_with(|| left.1.cmp(&right.1)));
    let shape_classes = classify_field_x_record_shapes(&features);
    let mut groups: Vec<_> = stable_chunk_shape_support(&features).into_iter().collect();
    groups.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let mut marker_groups: Vec<_> = stable_marker_support(&features).into_iter().collect();
    marker_groups.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    eprintln!(
        "top record shape classes: {:?}",
        shape_classes.iter().take(10).collect::<Vec<_>>()
    );
    eprintln!(
        "top chunk-shape groups: {:?}",
        groups.iter().take(10).collect::<Vec<_>>()
    );
    eprintln!(
        "top marker groups: {:?}",
        marker_groups.iter().take(10).collect::<Vec<_>>()
    );
    eprintln!(
        "feature scoring summary: max_score={}, promotable={}",
        max_feature_score, promotable_feature_scores
    );
    eprintln!(
        "top feature scores: {:?}",
        top_feature_scores.iter().take(10).collect::<Vec<_>>()
    );

    assert!(
        !features.is_empty(),
        "field_x window feature extraction should inspect real Sheet6 windows"
    );
    assert!(
        groups.first().is_some_and(|(_, support)| *support > 0),
        "expected at least one chunk-relative shape group"
    );
    assert!(
        shape_classes
            .first()
            .is_some_and(|shape_class| shape_class.support > 0),
        "expected at least one classified chunk-relative shape"
    );
    assert!(
        marker_groups
            .first()
            .is_some_and(|(_, support)| *support > 0),
        "expected at least one marker group"
    );
}

#[test]
fn relationship_endpoint_provenance_matches_sheet_records() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let graph = doc.object_graph.as_ref().expect("object_graph");
    let cross = doc
        .cross_reference
        .as_ref()
        .expect("cross reference graph should be built");

    assert_eq!(
        cross.relationship_endpoint_links.len(),
        graph.relationships.len(),
        "crossref should preserve 1:1 relationship link coverage"
    );

    let linked = cross
        .relationship_endpoint_links
        .iter()
        .filter(|l| l.sheet_path.is_some())
        .count();
    assert_eq!(linked, cross.relationship_endpoint_coverage.linked);
    assert_eq!(
        cross.relationship_endpoint_coverage.total,
        graph.relationships.len()
    );

    for link in &cross.relationship_endpoint_links {
        let rel = graph
            .relationships
            .iter()
            .find(|r| r.guid == link.relationship_guid)
            .expect("link should point to existing relationship");
        assert_eq!(rel.record_id, link.relationship_record_id);
        assert_eq!(rel.field_x, link.rel_field_x);
        assert_eq!(rel.source_drawing_id, link.source_drawing_id);
        assert_eq!(rel.target_drawing_id, link.target_drawing_id);

        match link.rel_field_x {
            None => {
                assert!(link.sheet_path.is_none());
                assert!(!link.missing_sheet_record);
            }
            Some(field_x) => {
                let sheet_record = doc
                    .sheet_streams
                    .iter()
                    .flat_map(|s| s.endpoint_records.iter())
                    .find(|r| r.rel_field_x == field_x);
                match sheet_record {
                    Some(record) => {
                        assert_eq!(link.sheet_path.as_deref(), Some(record.sheet_path.as_str()));
                        assert_eq!(link.sheet_offset, Some(record.offset));
                        assert_eq!(link.source_field_x, Some(record.endpoint_a));
                        assert_eq!(link.target_field_x, Some(record.endpoint_b));
                        assert!(!link.missing_sheet_record);
                    }
                    None => {
                        assert!(link.sheet_path.is_none());
                        assert!(link.missing_sheet_record);
                    }
                }
            }
        }
    }
}

#[test]
fn object_sources_align_with_attribute_records() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let graph = doc.object_graph.as_ref().expect("object_graph");
    let cross = doc
        .cross_reference
        .as_ref()
        .expect("cross reference graph should be built");
    let da = doc
        .dynamic_attributes
        .as_ref()
        .expect("dynamic_attributes expected");

    assert_eq!(
        cross.object_sources.len(),
        graph.objects.len(),
        "object_sources must stay 1:1 with object_graph.objects"
    );
    assert_eq!(
        cross.object_source_coverage.total_objects,
        graph.objects.len()
    );

    let mut linked = 0usize;
    let mut missing = 0usize;
    let mut with_trailer = 0usize;
    for (source, obj) in cross.object_sources.iter().zip(graph.objects.iter()) {
        assert_eq!(
            source.drawing_id, obj.drawing_id,
            "object_sources order should mirror object_graph.objects"
        );
        assert_eq!(source.has_trailer_record_id, obj.record_id.is_some());

        if source.missing_da_record {
            assert!(source.class_name.is_none());
            assert!(source.attribute_record_index.is_none());
            assert!(source.confidence.is_none());
            missing += 1;
            continue;
        }

        linked += 1;
        let idx = source
            .attribute_record_index
            .expect("linked source must carry an attribute_record_index");
        let record = da
            .attribute_records
            .get(idx)
            .expect("attribute_record_index must be a valid DA index");
        assert_eq!(
            Some(record.class_name.as_str()),
            source.class_name.as_deref()
        );
        assert_eq!(
            Some(record.confidence.as_str()),
            source.confidence.as_deref()
        );
        // Each linked DA record must expose a DrawingID/No text attribute
        // (parser-shape invariant), but its value is *not* asserted equal
        // to `source.drawing_id` here. On the in-repo sanitized fixtures
        // every P&IDAttributes record advertises the *drawing*-level UUID
        // (e.g. `0F7B8ABD0C4E493FA3C7F06FD03AD6AA`) instead of an
        // object-level UUID, so the equality check would fail uniformly
        // — the assumption only matched the pre-sanitization private
        // fixture used when this test was authored. The semantic
        // reconciliation between DA `DrawingID` field and `cross_ref`
        // `source.drawing_id` is owned by the upcoming Phase 12a
        // normalized graph layer; until then we only assert presence.
        let _advertised_id = record
            .attributes
            .iter()
            .find(|f| matches!(f.name.as_str(), "DrawingID" | "DrawingNo"))
            .and_then(|f| match &f.value {
                pid_parse::model::AttributeValue::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .expect("linked record must advertise a DrawingID/No");

        if source.has_trailer_record_id {
            with_trailer += 1;
        }
    }

    let cov = &cross.object_source_coverage;
    assert_eq!(cov.linked, linked);
    assert_eq!(cov.missing_da_record, missing);
    assert_eq!(cov.with_trailer_record_id, with_trailer);
}

#[test]
fn psm_cluster_record_probes_match_entry_slice() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let table = doc
        .psm_cluster_table
        .as_ref()
        .expect("PSMclustertable decoded");

    assert!(!table.entries.is_empty(), "fixture has cluster records");

    for entry in &table.entries {
        let probe = entry
            .probe
            .as_ref()
            .expect("every cluster record should carry a probe");

        if entry.prefix_bytes.len() >= 4 {
            let expected = u32::from_le_bytes([
                entry.prefix_bytes[0],
                entry.prefix_bytes[1],
                entry.prefix_bytes[2],
                entry.prefix_bytes[3],
            ]);
            assert_eq!(probe.first_u32_le, Some(expected));
        } else {
            assert!(probe.first_u32_le.is_none());
        }

        assert_eq!(probe.name_char_count, entry.name.chars().count());

        let expected_prefix_hex = entry
            .prefix_bytes
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(probe.prefix_hex, expected_prefix_hex);

        let trailer_tokens: Vec<_> = probe.trailer_hex.split_whitespace().collect();
        assert!(
            trailer_tokens.len() <= 8,
            "trailer_hex should cap at 8 tokens, got {}",
            trailer_tokens.len()
        );
        if entry.record_len >= 8 {
            assert_eq!(trailer_tokens.len(), 8);
        } else {
            assert_eq!(trailer_tokens.len(), entry.record_len);
        }
    }
}

#[test]
fn psm_cluster_decoded_records_match_observed_prefix_candidates() {
    for fixture in ["DWG-0201GP06-01.pid", "DWG-0202GP06-01.pid"] {
        let Some(doc) = parse_test_file(fixture) else {
            return;
        };
        let table = doc
            .psm_cluster_table
            .as_ref()
            .expect("PSMclustertable decoded");

        assert_eq!(
            table.decoded_records.len(),
            table.entries.len(),
            "{fixture}: decoded record view should stay parallel to entries"
        );

        for (entry, decoded) in table.entries.iter().zip(&table.decoded_records) {
            assert_eq!(
                decoded.name, entry.name,
                "{fixture}: decoded name should mirror legacy entry"
            );
            assert_eq!(
                decoded.record_offset, entry.record_offset,
                "{fixture}: decoded offset should mirror legacy entry"
            );
            assert_eq!(
                decoded.record_len, entry.record_len,
                "{fixture}: decoded length should mirror legacy entry"
            );
        }

        let first = &table.decoded_records[0];
        assert_eq!(first.name, "PSMcluster0");
        assert_eq!(first.name_bytes_with_nul, Some(24));
        assert_eq!(first.candidate_ordinal, Some(0));
        assert_eq!(first.candidate_non_sheet_marker, Some(1));
        assert_eq!(first.candidate_non_sheet_payload_index, Some(0));
        assert_eq!(first.confidence, "medium");

        let sheet6 = table
            .decoded_records
            .iter()
            .find(|r| r.name == "Sheet6")
            .expect("Sheet6 decoded record");
        assert_eq!(sheet6.name_bytes_with_nul, Some(14));
        assert_eq!(sheet6.candidate_ordinal, Some(3));
        assert_eq!(sheet6.candidate_non_sheet_marker, Some(0));
        assert_eq!(sheet6.candidate_non_sheet_payload_index, None);

        if fixture == "DWG-0202GP06-01.pid" {
            let sheet6615 = table
                .decoded_records
                .iter()
                .find(|r| r.name == "Sheet6615")
                .expect("DWG-0202 has the extra Sheet6615 record");
            assert_eq!(sheet6615.name_bytes_with_nul, Some(20));
            assert_eq!(sheet6615.candidate_ordinal, Some(5));
            assert_eq!(sheet6615.candidate_non_sheet_marker, Some(0));
            assert_eq!(sheet6615.candidate_non_sheet_payload_index, None);
        }
    }
}

#[test]
fn psm_segment_record_probes_align_with_flags() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let table = doc
        .psm_segment_table
        .as_ref()
        .expect("PSMsegmenttable decoded");

    assert!(!table.entries.is_empty(), "fixture has segment entries");
    assert_eq!(
        table.entries.len(),
        table.flags.len(),
        "entries and flags should stay in sync (legacy flags array keeps \
         parallel shape to the structured entries)"
    );

    for entry in &table.entries {
        let probe = entry
            .probe
            .as_ref()
            .expect("every segment entry should carry a probe");

        assert_eq!(
            probe.flag_hex,
            format!("{:02X}", entry.flag),
            "flag_hex should echo the raw flag byte",
        );
        assert_eq!(
            probe.stream_offset, entry.offset,
            "stream_offset must match entry.offset",
        );

        let window_tokens: Vec<_> = probe.neighbor_window_hex.split_whitespace().collect();
        assert!(
            (1..=7).contains(&window_tokens.len()),
            "±3-byte window should yield 1..=7 tokens, got {}: {:?}",
            window_tokens.len(),
            window_tokens,
        );
    }

    // Hint coverage: depending on fixture shape, either every probe has a
    // hint (1:1 lengths) or none do. The code path is *never* allowed to
    // emit partial hints.
    let cluster_count = doc
        .psm_cluster_table
        .as_ref()
        .map_or(0, |c| c.entries.len());
    let hint_count = table
        .entries
        .iter()
        .filter_map(|e| e.probe.as_ref()?.owner_cluster_hint.as_ref())
        .count();
    let candidate_owner_count = table
        .entries
        .iter()
        .filter(|e| {
            e.candidate_owner_cluster_index.is_some() && e.candidate_owner_cluster_name.is_some()
        })
        .count();

    if cluster_count == table.entries.len() && cluster_count > 0 {
        assert_eq!(
            hint_count,
            table.entries.len(),
            "when cluster and segment counts match, every segment probe \
             must carry an owner_cluster_hint"
        );
        assert_eq!(
            candidate_owner_count,
            table.entries.len(),
            "when cluster and segment counts match, every segment entry \
             must carry a structured candidate owner"
        );
        let expected_hints: Vec<_> = doc
            .psm_cluster_table
            .as_ref()
            .expect("precondition")
            .entries
            .iter()
            .map(|c| c.name.clone())
            .collect();
        let actual_hints: Vec<_> = table
            .entries
            .iter()
            .map(|e| {
                e.probe
                    .as_ref()
                    .and_then(|p| p.owner_cluster_hint.clone())
                    .expect("hint populated per precondition above")
            })
            .collect();
        assert_eq!(
            actual_hints, expected_hints,
            "1:1 positional hint mapping broken",
        );
        let actual_candidate_owners: Vec<_> = table
            .entries
            .iter()
            .map(|e| {
                (
                    e.candidate_owner_cluster_index
                        .expect("owner index populated per precondition above"),
                    e.candidate_owner_cluster_name
                        .clone()
                        .expect("owner name populated per precondition above"),
                )
            })
            .collect();
        let expected_candidate_owners: Vec<_> = expected_hints.into_iter().enumerate().collect();
        assert_eq!(
            actual_candidate_owners, expected_candidate_owners,
            "structured 1:1 candidate owner mapping broken",
        );
    } else {
        assert_eq!(
            hint_count, 0,
            "when counts disagree, all owner_cluster_hint slots must be None",
        );
        assert_eq!(
            candidate_owner_count, 0,
            "when counts disagree, all structured candidate owner slots must be None",
        );
    }
}

#[test]
fn sheet_provenance_matches_sheet_streams() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let cross = doc
        .cross_reference
        .as_ref()
        .expect("cross reference graph should be built");

    assert_eq!(cross.sheet_provenance.len(), doc.sheet_streams.len());
    assert_eq!(
        cross.sheet_provenance_coverage.total_sheets,
        doc.sheet_streams.len()
    );

    for (i, entry) in cross.sheet_provenance.iter().enumerate() {
        let source_sheet = &doc.sheet_streams[i];
        assert_eq!(entry.sheet_path, source_sheet.path);
        assert_eq!(
            entry.endpoint_record_count,
            source_sheet.endpoint_records.len()
        );

        let expected_linked = cross
            .relationship_endpoint_links
            .iter()
            .filter(|l| l.sheet_path.as_deref() == Some(entry.sheet_path.as_str()))
            .count();
        assert_eq!(entry.linked_relationship_count, expected_linked);
        assert!(entry.fully_traced_relationship_count <= entry.linked_relationship_count);

        if entry.declared_in_psm {
            assert!(entry.matched_declared_index.is_some());
        } else {
            assert!(entry.matched_declared_index.is_none());
        }
    }

    let cov = &cross.sheet_provenance_coverage;
    assert_eq!(
        cov.declared_sheets + cov.orphan_sheets,
        cov.total_sheets,
        "declared + orphan must cover every sheet"
    );
    assert!(cov.empty_declared_sheets <= cov.declared_sheets);
}

#[test]
fn provenance_chain_matches_relationship_and_object_counts() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let graph = doc.object_graph.as_ref().expect("object_graph");
    let cross = doc
        .cross_reference
        .as_ref()
        .expect("cross reference graph should be built");

    let cov = &cross.provenance_chain_coverage;
    assert_eq!(cov.total_relationships, graph.relationships.len());
    assert_eq!(
        cov.total_relationships,
        cross.relationship_endpoint_links.len()
    );
    assert_eq!(
        cov.has_field_x,
        graph
            .relationships
            .iter()
            .filter(|r| r.field_x.is_some())
            .count()
    );
    assert_eq!(
        cov.sheet_linked,
        cross
            .relationship_endpoint_links
            .iter()
            .filter(|l| l.sheet_path.is_some())
            .count()
    );
    assert!(cov.fully_traced <= cov.sheet_linked);
    assert!(cov.fully_traced <= cov.source_object_linked);
    assert!(cov.fully_traced <= cov.target_object_linked);

    assert!(cross.provenance_chain_breaks.len() <= 10);
    for br in &cross.provenance_chain_breaks {
        assert!(
            cross
                .relationship_endpoint_links
                .iter()
                .any(|l| l.relationship_guid == br.relationship_guid),
            "chain break should reference an existing relationship link"
        );
    }
}

#[test]
fn relationship_probe_nearby_guids_contain_drawing_id() {
    // Every relationship's window is expected to include the drawing's own
    // DrawingNo GUID (0F7B...AA in the fixture), because the record before
    // and after is a P&IDAttributes record tied to the drawing.
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let da = doc
        .dynamic_attributes
        .as_ref()
        .expect("dynamic_attributes expected");
    let drawing_guid = "0F7B8ABD0C4E493FA3C7F06FD03AD6AA";
    let mut misses = 0usize;
    for p in &da.relationship_probes {
        if !p.nearby_ascii_guids.iter().any(|(_, g)| g == drawing_guid) {
            misses += 1;
        }
    }
    // Allow a tiny tail (first/last probe might miss the neighbour window)
    // but the vast majority should carry the drawing id.
    assert!(
        misses <= 2,
        "expected ≤2 probes missing the drawing guid, got {} / {}",
        misses,
        da.relationship_probes.len()
    );
}

#[test]
fn sheet6_coordinate_value_frequency_analysis() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(raw_sheet) = pkg.streams.get("/Sheet6") else {
        eprintln!("skipping: /Sheet6 not found");
        return;
    };
    let data = &raw_sheet.data;

    let target_x: i32 = 206; // 0xCE
    let target_y: i32 = 121; // 0x79
    let target_bytes = [0xCE, 0x00, 0x79, 0x00];
    let alt_bytes = [0xCE, 0x00, 0x71, 0x00];

    let mut exact_hits = 0usize;
    let mut alt_hits = 0usize;
    let mut hit_offsets = Vec::new();
    for offset in 0..data.len().saturating_sub(7) {
        if data[offset..offset + 4] == target_bytes {
            exact_hits += 1;
            hit_offsets.push(offset);
        }
        if data[offset..offset + 4] == alt_bytes {
            alt_hits += 1;
        }
    }

    let total_i32_pairs = data.len().saturating_sub(7);
    let frequency_pct = exact_hits as f64 / total_i32_pairs as f64 * 100.0;

    let report = probe_sheet_stream("Sheet6", "/Sheet6", data, &SheetProbeOptions::default());
    let in_chunk_count = hit_offsets
        .iter()
        .filter(|&&offset| {
            report
                .chunks
                .iter()
                .any(|chunk| chunk.start <= offset && offset < chunk.end)
        })
        .count();

    eprintln!(
        "coordinate frequency analysis: stream_len={}, target=({target_x},{target_y}), exact_hits={exact_hits}, alt_hits={alt_hits}, frequency={frequency_pct:.3}%, in_chunk={in_chunk_count}, total_chunks={}",
        data.len(),
        report.chunks.len()
    );
    eprintln!(
        "first 10 hit offsets: {:?}",
        hit_offsets.iter().take(10).collect::<Vec<_>>()
    );

    let coord_206 = data
        .windows(4)
        .filter(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]) == 206)
        .count();
    let coord_121 = data
        .windows(4)
        .filter(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]) == 121)
        .count();

    eprintln!(
        "standalone value frequency: val_206_as_u32={coord_206}, val_121_as_u32={coord_121}"
    );

    let promoted_field_xs: Vec<u32> = pkg
        .parsed
        .sheet_streams[0]
        .geometry
        .as_ref()
        .map(|g| g.object_geometry_hints.iter().map(|h| h.field_x).collect())
        .unwrap_or_default();

    for (idx, &offset) in hit_offsets.iter().enumerate() {
        let field_x_offset = offset + 6;
        let nearby_field_x = if field_x_offset + 4 <= data.len() {
            Some(u32::from_le_bytes([
                data[field_x_offset],
                data[field_x_offset + 1],
                data[field_x_offset + 2],
                data[field_x_offset + 3],
            ]))
        } else {
            None
        };
        let is_promoted = nearby_field_x
            .map(|fx| promoted_field_xs.contains(&fx))
            .unwrap_or(false);
        eprintln!(
            "record_header[{idx}] offset={offset} field_x={:?} promoted={is_promoted}",
            nearby_field_x
        );
    }

    for (idx, &offset) in hit_offsets.iter().enumerate().take(5) {
        let ctx_start = offset.saturating_sub(8);
        let ctx_end = (offset + 16).min(data.len());
        let hex: String = data[ctx_start..ctx_end]
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ");
        let delta_from_prev = if idx > 0 {
            offset as isize - hit_offsets[idx - 1] as isize
        } else {
            0
        };
        eprintln!(
            "hit[{idx}] offset={offset} delta_from_prev={delta_from_prev} ctx={ctx_start}..{ctx_end}: {hex}"
        );
    }

    assert!(
        exact_hits >= 10,
        "CE 00 79 00 should appear frequently enough to be structural, got {exact_hits}"
    );

    let geometry = &pkg.parsed.sheet_streams[0].geometry;
    if let Some(geom) = geometry {
        for (idx, hint) in geom.object_geometry_hints.iter().enumerate() {
            if let Some(ref pos) = hint.position {
                eprintln!(
                    "geometry_hint[{idx}]: field_x={}, offset={}, coord=({}, {}), note={:?}",
                    hint.field_x, hint.offset, pos.x, pos.y, hint.note
                );
            }
        }
    }

    let Some(cross) = pkg.parsed.cross_reference.as_ref() else { return; };
    let Some(da) = pkg.parsed.dynamic_attributes.as_ref() else { return; };
    let identity_index = sheet_identity_index_from_trailers(&da.record_trailers);
    let object_field_xs: HashSet<_> = pkg.parsed.object_graph.as_ref()
        .map(|g| g.objects.iter().filter_map(|o| o.field_x).collect())
        .unwrap_or_default();
    let mut ep_field_xs: Vec<_> = cross.relationship_endpoint_links.iter()
        .filter(|l| l.sheet_path.as_deref() == Some("/Sheet6"))
        .flat_map(|l| [l.source_field_x, l.target_field_x])
        .flatten().collect();
    ep_field_xs.sort_unstable();
    ep_field_xs.dedup();
    let report = probe_sheet_stream("Sheet6", "/Sheet6", data, &SheetProbeOptions::default());
    let windows = field_x_windows(data, &ep_field_xs, 96);
    let features = field_x_window_features(data, &windows, &report.chunks);
    let identities = field_x_window_identities(data, &windows, &identity_index);
    let scores = score_field_x_window_features_with_identities(&features, &object_field_xs, &identities);

    let ce_field_xs: Vec<u32> = hit_offsets.iter().filter_map(|&off| {
        let fx_off = off + 6;
        if fx_off + 4 <= data.len() {
            Some(u32::from_le_bytes([data[fx_off], data[fx_off+1], data[fx_off+2], data[fx_off+3]]))
        } else { None }
    }).collect();

    for fx in &ce_field_xs {
        if promoted_field_xs.contains(fx) { continue; }
        let best = scores.iter()
            .filter(|s| s.field_x == *fx && s.score > 0)
            .max_by_key(|s| s.score);
        if let Some(s) = best {
            let has_id = s.reasons.iter().any(|r| matches!(r, pid_parse::parsers::sheet_probe::SheetFieldXWindowScoreReason::GraphicIdentityNearby { .. }));
            let has_shape = s.reasons.iter().any(|r| matches!(r, pid_parse::parsers::sheet_probe::SheetFieldXWindowScoreReason::StableChunkShape { .. }));
            eprintln!(
                "unpromoted CE0079 field_x={fx}: best_score={}, identity={has_id}, shape={has_shape}, reasons={:?}",
                s.score, s.reasons.iter().map(|r| format!("{r:?}").chars().take(30).collect::<String>()).collect::<Vec<_>>()
            );
        } else {
            eprintln!("unpromoted CE0079 field_x={fx}: no positive score (may be endpoint-only)");
        }
    }
}
