use pid_parse::PidParser;

fn parse_test_file(name: &str) -> pid_parse::PidDocument {
    let path = format!("test-file/{}", name);
    PidParser::new()
        .parse_file(&path)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", name, e))
}

#[test]
fn container_structure_has_streams() {
    let doc = parse_test_file("DWG-0201GP06-01.pid");
    assert!(!doc.streams.is_empty(), "streams should not be empty");
    assert!(
        doc.streams.len() > 10,
        "expected many streams, got {}",
        doc.streams.len()
    );
}

#[test]
fn cfb_tree_root_has_children() {
    let doc = parse_test_file("DWG-0201GP06-01.pid");
    assert!(
        !doc.cfb_tree.children.is_empty(),
        "root node should have children"
    );
}

#[test]
fn drawing_meta_extracted() {
    let doc = parse_test_file("DWG-0201GP06-01.pid");
    let dm = doc.drawing_meta.as_ref().expect("drawing_meta should exist");
    assert_eq!(dm.drawing_number.as_deref(), Some("DWG-0201GP06-01"));
    assert_eq!(dm.document_category.as_deref(), Some("Piping Documents"));
    assert_eq!(dm.template_name.as_deref(), Some("XIONGANA2.pid"));
    assert!(!dm.tags.is_empty(), "tags should have been extracted");
}

#[test]
fn general_meta_extracted() {
    let doc = parse_test_file("DWG-0201GP06-01.pid");
    let gm = doc.general_meta.as_ref().expect("general_meta should exist");
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
    let doc = parse_test_file("DWG-0201GP06-01.pid");
    assert!(!doc.jsites.is_empty(), "should detect JSites");
    assert!(
        doc.jsites.len() > 5,
        "expected multiple JSites, got {}",
        doc.jsites.len()
    );
}

#[test]
fn jsite_symbol_paths_are_clean() {
    let doc = parse_test_file("DWG-0201GP06-01.pid");
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
    let doc = parse_test_file("DWG-0201GP06-01.pid");
    assert!(!doc.clusters.is_empty(), "should detect clusters");
    let names: Vec<&str> = doc.clusters.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"PSMcluster0"));
    assert!(names.contains(&"StyleCluster"));
}

#[test]
fn dynamic_attributes_detected() {
    let doc = parse_test_file("DWG-0201GP06-01.pid");
    let da = doc
        .dynamic_attributes
        .as_ref()
        .expect("dynamic_attributes should exist");
    assert!(da.size > 0);
    assert!(!da.strings.is_empty());
}

#[test]
fn sheet_streams_detected() {
    let doc = parse_test_file("DWG-0201GP06-01.pid");
    assert!(!doc.sheet_streams.is_empty(), "should detect Sheet streams");
}

#[test]
fn second_file_parses_successfully() {
    let doc = parse_test_file("DWG-0202GP06-01.pid");
    assert!(!doc.streams.is_empty());
    let dm = doc.drawing_meta.as_ref().expect("drawing_meta should exist");
    assert!(dm.drawing_number.is_some());
}

#[test]
fn json_serialization_roundtrip() {
    let doc = parse_test_file("DWG-0201GP06-01.pid");
    let json = serde_json::to_string(&doc).expect("should serialize to JSON");
    assert!(json.contains("DWG-0201GP06-01"));
    let _: pid_parse::PidDocument =
        serde_json::from_str(&json).expect("should deserialize from JSON");
}

#[test]
fn psm_roots_extracts_known_entries() {
    let doc = parse_test_file("DWG-0201GP06-01.pid");
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
    let doc = parse_test_file("DWG-0201GP06-01.pid");
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
    let doc = parse_test_file("DWG-0201GP06-01.pid");
    let t = doc
        .psm_segment_table
        .as_ref()
        .expect("PSMsegmenttable should be decoded");
    assert_eq!(t.count as usize, t.flags.len());
    assert!(t.flags.iter().all(|&b| b == 0x01));
}
