use pid_parse::PidParser;

/// Parse a real `.pid` fixture from `test-file/`. Returns `None` when the
/// fixture isn't present (typical for CI and for contributors without
/// access to SmartPlant samples) so the test can cleanly skip instead of
/// panicking. See `writer_real_files.rs` for the matching pattern.
fn parse_test_file(name: &str) -> Option<pid_parse::PidDocument> {
    let path = format!("test-file/{}", name);
    if !std::path::Path::new(&path).exists() {
        eprintln!("skipping: fixture {} not found", name);
        return None;
    }
    Some(
        PidParser::new()
            .parse_file(&path)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", name, e)),
    )
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
                "symbol_path should be a clean UNC or drive path, got: {}",
                sp
            );
            assert!(
                sp.ends_with(".sym"),
                "symbol_path should end with .sym: {}",
                sp
            );
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
    assert!(
        layout.segments.len() >= 5,
        "expected readable layout to recover at least 5 segments, got {}",
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
            "missing expected PSMroots entry: {}",
            expected
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
            "PSMclustertable should list {}",
            expected
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
    assert!(t.flags.iter().all(|&b| b == 0x01));
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
    // "SA" / "SV" strings one-to-one.
    for (v2, v3) in dv2.records.iter().zip(dv3.records.iter()) {
        let label = pid_parse::parsers::doc_version2::op_type_label(v2.op_type);
        let expected = match v3.operation.as_str() {
            "SA" => "SaveAs",
            "SV" => "Save",
            other => panic!("unexpected DocVersion3 op {other}"),
        };
        assert_eq!(
            label, expected,
            "op_type mismatch for v3 op {}",
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
    // The fixture has 64 relationships; the endpoint parser should
    // recover a pair for almost all of them — we allow at most one
    // unresolved relationship to keep the assertion stable.
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
    assert!(
        resolved >= 40,
        "expected ≥40 fully resolved relationships, got {} / {}",
        resolved,
        g.relationships.len()
    );
    assert!(
        unresolved <= 1,
        "expected ≤1 fully unresolved relationship, got {} / {}",
        unresolved,
        g.relationships.len()
    );
    // The resolved endpoints must live in the drawing's object set —
    // regression against field_x → drawing_id misalignment.
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
    // The fixture references some off-page endpoints (OPC); expect a
    // handful but not hundreds.
    assert!(
        foreign_endpoints < 20,
        "too many endpoints point to objects absent from graph: {}",
        foreign_endpoints
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
    assert_eq!(
        sheet.endpoint_records.len(),
        doc.object_graph
            .as_ref()
            .expect("object_graph")
            .relationships
            .len(),
        "each relationship should have exactly one Sheet endpoint record"
    );
    // The endpoint record's `rel_field_x` must match its relationship
    // counterpart (sanity check on parser bookkeeping).
    let rel_field_xs: std::collections::HashSet<u32> = doc
        .object_graph
        .as_ref()
        .unwrap()
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
