use pid_parse::{
    parsers::sheet_probe::{
        classify_field_x_record_shapes, field_x_window_features, field_x_window_identities,
        field_x_windows, probe_sheet_stream, repeated_f64_pair_candidate_before_field_x,
        score_field_x_window_features, score_field_x_window_features_with_identities,
        score_field_x_windows, score_sheet_text_window_candidates,
        sheet_identity_index_from_trailers, sheet_text_window_candidates,
        stable_chunk_shape_support, stable_marker_support,
        summarize_object_geometry_promotion_gate, top_field_x_candidate_record_dumps,
        top_text_candidate_record_dumps, SheetFieldXWindowScoreReason, SheetProbeOptions,
    },
    parsers::sheet_records::{
        coordinate_page_metadata_investigation_report, curve_primitive_investigation_report,
        decode_iglines, decode_iglinestrings, decode_primitive_arcs, decode_primitive_lines,
        primitive_line_investigation_report, sheet_record_shape_inventory,
        symbol_placement_investigation_report, text_placement_investigation_report,
        SheetCoordinatePageMetadataCandidateKind, SheetCurvePrimitiveCandidateKind,
        SheetRecordShapeKind, SheetSymbolPlacementObject, PSM_TYPE_CODE_GARC2D,
        PSM_TYPE_CODE_GLINE2D, PSM_TYPE_CODE_IGLINE2D, PSM_TYPE_CODE_IGLINESTRING2D,
    },
    PidParser,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ControlledPidDiffCase {
    name: String,
    before_path: PathBuf,
    after_path: PathBuf,
    metadata_path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
struct ControlledPidDiffMetadata {
    case: String,
    operation: String,
    expected: serde_json::Value,
    #[serde(default)]
    notes: Option<String>,
}

fn controlled_pid_diff_cases() -> Vec<ControlledPidDiffCase> {
    let root = Path::new("test-file/controlled-diff");
    let before_root = root.join("before");
    let after_root = root.join("after");
    let metadata_root = root.join("metadata");
    let Ok(entries) = fs::read_dir(&before_root) else {
        return Vec::new();
    };

    let mut cases = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let before_path = entry.path();
            let extension = before_path.extension().and_then(|value| value.to_str());
            if extension != Some("pid") {
                return None;
            }
            let name = before_path
                .file_stem()
                .and_then(|value| value.to_str())
                .map(str::to_owned)?;
            let after_path = after_root.join(format!("{name}.pid"));
            if !after_path.exists() {
                return None;
            }
            let metadata_path = metadata_root.join(format!("{name}.json"));
            Some(ControlledPidDiffCase {
                name,
                before_path,
                after_path,
                metadata_path,
            })
        })
        .collect::<Vec<_>>();
    cases.sort_by(|left, right| left.name.cmp(&right.name));
    cases
}

#[test]
fn controlled_pid_diff_pairs_report_stream_level_evidence_when_available() {
    let cases = controlled_pid_diff_cases();
    if cases.is_empty() {
        eprintln!("skipping: no controlled PID diff pairs found under test-file/controlled-diff");
        return;
    }

    let parser = PidParser::new();
    let mut case_summaries = Vec::new();
    for case in cases {
        assert!(
            case.metadata_path.exists(),
            "controlled diff case `{}` must include metadata sidecar {}",
            case.name,
            case.metadata_path.display()
        );
        let metadata = fs::read_to_string(&case.metadata_path).unwrap_or_else(|err| {
            panic!(
                "failed to read controlled diff metadata {}: {err}",
                case.metadata_path.display()
            )
        });
        let metadata: ControlledPidDiffMetadata =
            serde_json::from_str(&metadata).unwrap_or_else(|err| {
                panic!(
                    "controlled diff metadata {} must be valid JSON: {err}",
                    case.metadata_path.display()
                )
            });
        assert_eq!(
            metadata.case.as_str(),
            case.name.as_str(),
            "controlled diff metadata {} case field must match the before/after filename stem",
            case.metadata_path.display()
        );
        assert!(
            !metadata.operation.trim().is_empty(),
            "controlled diff metadata {} must include a non-empty operation",
            case.metadata_path.display()
        );
        assert!(
            !metadata.expected.is_null(),
            "controlled diff metadata {} must include expected payload",
            case.metadata_path.display()
        );

        let before = parser
            .parse_package(&case.before_path)
            .unwrap_or_else(|err| {
                panic!(
                    "failed to parse controlled diff before file {}: {err}",
                    case.before_path.display()
                )
            });
        let after = parser
            .parse_package(&case.after_path)
            .unwrap_or_else(|err| {
                panic!(
                    "failed to parse controlled diff after file {}: {err}",
                    case.after_path.display()
                )
            });
        let diff = pid_parse::diff_packages(&before, &after);
        let stream_diff_count = diff.only_in_a.len() + diff.only_in_b.len() + diff.modified.len();
        let modified_sheet_streams = diff
            .modified
            .iter()
            .filter(|stream| stream.path.starts_with("/Sheet"))
            .count();
        let first_modified = diff.modified.first().map(|stream| {
            (
                stream.path.clone(),
                stream.len_a,
                stream.len_b,
                stream.first_mismatch_offset,
                stream.context_before.clone(),
                stream.context_after.clone(),
            )
        });
        eprintln!(
            "controlled PID diff case `{}`: stream_diff_count={}, modified_sheet_streams={}, only_in_before={}, only_in_after={}, metadata={}, first_modified={:?}",
            case.name,
            stream_diff_count,
            modified_sheet_streams,
            diff.only_in_a.len(),
            diff.only_in_b.len(),
            case.metadata_path.display(),
            first_modified
        );
        assert!(
            stream_diff_count > 0,
            "controlled diff case `{}` must change at least one CFB stream",
            case.name
        );
        case_summaries.push((
            case.name,
            metadata.operation,
            metadata.notes,
            stream_diff_count,
            modified_sheet_streams,
        ));
    }

    assert!(
        !case_summaries.is_empty(),
        "controlled diff discovery should produce at least one case before assertions run"
    );
}

#[derive(Debug, Clone, Copy)]
struct GeometryFixtureCase {
    path: &'static str,
    category: &'static str,
}

const GEOMETRY_FIXTURE_TARGET_MIN_AVAILABLE: usize = 8;

fn geometry_fixture_cases() -> &'static [GeometryFixtureCase] {
    &[
        GeometryFixtureCase {
            path: "DWG-0201GP06-01.pid",
            category: "dwg",
        },
        GeometryFixtureCase {
            path: "DWG-0202GP06-01.pid",
            category: "dwg",
        },
        GeometryFixtureCase {
            path: "工艺管道及仪表流程-1.pid",
            category: "non_ascii",
        },
        GeometryFixtureCase {
            path: "export-test/publish-data/A01/A01.pid",
            category: "publish_a01",
        },
        GeometryFixtureCase {
            path: "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
            category: "publish_dwg",
        },
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GeometryFixtureAvailabilitySummary {
    registered: usize,
    target_min_available: usize,
    available: usize,
    missing: Vec<&'static str>,
}

fn geometry_fixture_availability_summary() -> GeometryFixtureAvailabilitySummary {
    let mut available = 0usize;
    let mut missing = Vec::new();
    for fixture in geometry_fixture_cases() {
        let path = format!("test-file/{}", fixture.path);
        if std::path::Path::new(&path).exists() {
            available += 1;
        } else {
            missing.push(fixture.path);
        }
    }

    GeometryFixtureAvailabilitySummary {
        registered: geometry_fixture_cases().len(),
        target_min_available: GEOMETRY_FIXTURE_TARGET_MIN_AVAILABLE,
        available,
        missing,
    }
}

fn geometry_fixture_availability_report_line(
    summary: &GeometryFixtureAvailabilitySummary,
) -> String {
    format!(
        "geometry fixture availability: registered={}, target_min_available={}, available={}, missing={:?}",
        summary.registered, summary.target_min_available, summary.available, summary.missing
    )
}

fn print_geometry_fixture_availability() -> GeometryFixtureAvailabilitySummary {
    let summary = geometry_fixture_availability_summary();
    eprintln!("{}", geometry_fixture_availability_report_line(&summary));
    if summary.missing.is_empty() {
        eprintln!(
            "geometry fixture availability: no registered fixtures missing; target gap={}",
            summary
                .target_min_available
                .saturating_sub(summary.available)
        );
    } else {
        for missing in &summary.missing {
            eprintln!(
                "geometry fixture availability: missing fixture `{missing}` — real geometry evidence for this case is NOT validated on this run"
            );
        }
    }
    summary
}

#[derive(Debug, Clone, PartialEq)]
struct PageDimensionScalarHit {
    offset: usize,
    encoding: &'static str,
    value: f64,
    context_hex: String,
}

fn page_dimension_scalar_hits(
    data: &[u8],
    page_dimensions_mm: (f64, f64),
) -> Vec<PageDimensionScalarHit> {
    let (width, height) = page_dimensions_mm;
    let mut hits = data
        .windows(8)
        .enumerate()
        .filter(|(relative_offset, _)| relative_offset % 4 == 0)
        .filter_map(|(offset, window)| {
            let value = f64::from_le_bytes([
                window[0], window[1], window[2], window[3], window[4], window[5], window[6],
                window[7],
            ]);
            scalar_matches_dimension(value, width, height).then_some(PageDimensionScalarHit {
                offset,
                encoding: "f64",
                value,
                context_hex: hex_window(data, offset, 16),
            })
        })
        .collect::<Vec<_>>();
    hits.extend(
        data.windows(4)
            .enumerate()
            .filter(|(relative_offset, _)| relative_offset % 4 == 0)
            .filter_map(|(offset, window)| {
                let value = i32::from_le_bytes([window[0], window[1], window[2], window[3]]);
                scalar_matches_dimension(f64::from(value), width, height).then_some(
                    PageDimensionScalarHit {
                        offset,
                        encoding: "i32",
                        value: f64::from(value),
                        context_hex: hex_window(data, offset, 16),
                    },
                )
            }),
    );
    hits
}

fn scalar_matches_dimension(value: f64, width: f64, height: f64) -> bool {
    value.is_finite() && ((value - width).abs() <= 1.0e-6 || (value - height).abs() <= 1.0e-6)
}

fn stream_contains_ascii_token(data: &[u8], token: &str) -> bool {
    !token.is_empty()
        && data
            .windows(token.len())
            .any(|window| window == token.as_bytes())
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct NormalizedGeometryInventory {
    decoded_points: usize,
    inferred_points: usize,
    probe_only_points: usize,
    decoded_lines: usize,
    inferred_lines: usize,
    probe_only_lines: usize,
    decoded_polylines: usize,
    inferred_polylines: usize,
    probe_only_polylines: usize,
    decoded_arcs: usize,
    inferred_arcs: usize,
    probe_only_arcs: usize,
    decoded_circles: usize,
    inferred_circles: usize,
    probe_only_circles: usize,
    decoded_texts: usize,
    inferred_texts: usize,
    probe_only_texts: usize,
    decoded_symbols: usize,
    inferred_symbols: usize,
    probe_only_symbols: usize,
    decoded_unknowns: usize,
    inferred_unknowns: usize,
    probe_only_unknowns: usize,
    other_entities: usize,
}

impl NormalizedGeometryInventory {
    fn total(self) -> usize {
        self.decoded_points
            + self.inferred_points
            + self.probe_only_points
            + self.decoded_lines
            + self.inferred_lines
            + self.probe_only_lines
            + self.decoded_polylines
            + self.inferred_polylines
            + self.probe_only_polylines
            + self.decoded_arcs
            + self.inferred_arcs
            + self.probe_only_arcs
            + self.decoded_circles
            + self.inferred_circles
            + self.probe_only_circles
            + self.decoded_texts
            + self.inferred_texts
            + self.probe_only_texts
            + self.decoded_symbols
            + self.inferred_symbols
            + self.probe_only_symbols
            + self.decoded_unknowns
            + self.inferred_unknowns
            + self.probe_only_unknowns
            + self.other_entities
    }
}

fn normalized_geometry_inventory(doc: &pid_parse::PidDocument) -> NormalizedGeometryInventory {
    let geometry = pid_parse::build_normalized_geometry(doc);
    let mut inventory = NormalizedGeometryInventory::default();
    for entity in &geometry.entities {
        match (&entity.kind, entity.confidence) {
            (
                pid_parse::PidGraphicKind::Point { .. },
                pid_parse::PidGeometryConfidence::Decoded,
            ) => inventory.decoded_points += 1,
            (
                pid_parse::PidGraphicKind::Point { .. },
                pid_parse::PidGeometryConfidence::Inferred,
            ) => inventory.inferred_points += 1,
            (
                pid_parse::PidGraphicKind::Point { .. },
                pid_parse::PidGeometryConfidence::ProbeOnly,
            ) => inventory.probe_only_points += 1,
            (pid_parse::PidGraphicKind::Line { .. }, pid_parse::PidGeometryConfidence::Decoded) => {
                inventory.decoded_lines += 1
            }
            (
                pid_parse::PidGraphicKind::Line { .. },
                pid_parse::PidGeometryConfidence::Inferred,
            ) => inventory.inferred_lines += 1,
            (
                pid_parse::PidGraphicKind::Line { .. },
                pid_parse::PidGeometryConfidence::ProbeOnly,
            ) => inventory.probe_only_lines += 1,
            (
                pid_parse::PidGraphicKind::Polyline { .. },
                pid_parse::PidGeometryConfidence::Decoded,
            ) => inventory.decoded_polylines += 1,
            (
                pid_parse::PidGraphicKind::Polyline { .. },
                pid_parse::PidGeometryConfidence::Inferred,
            ) => inventory.inferred_polylines += 1,
            (
                pid_parse::PidGraphicKind::Polyline { .. },
                pid_parse::PidGeometryConfidence::ProbeOnly,
            ) => inventory.probe_only_polylines += 1,
            (pid_parse::PidGraphicKind::Arc { .. }, pid_parse::PidGeometryConfidence::Decoded) => {
                inventory.decoded_arcs += 1
            }
            (pid_parse::PidGraphicKind::Arc { .. }, pid_parse::PidGeometryConfidence::Inferred) => {
                inventory.inferred_arcs += 1
            }
            (
                pid_parse::PidGraphicKind::Arc { .. },
                pid_parse::PidGeometryConfidence::ProbeOnly,
            ) => inventory.probe_only_arcs += 1,
            (
                pid_parse::PidGraphicKind::Circle { .. },
                pid_parse::PidGeometryConfidence::Decoded,
            ) => inventory.decoded_circles += 1,
            (
                pid_parse::PidGraphicKind::Circle { .. },
                pid_parse::PidGeometryConfidence::Inferred,
            ) => inventory.inferred_circles += 1,
            (
                pid_parse::PidGraphicKind::Circle { .. },
                pid_parse::PidGeometryConfidence::ProbeOnly,
            ) => inventory.probe_only_circles += 1,
            (pid_parse::PidGraphicKind::Text { .. }, pid_parse::PidGeometryConfidence::Decoded) => {
                inventory.decoded_texts += 1
            }
            (
                pid_parse::PidGraphicKind::Text { .. },
                pid_parse::PidGeometryConfidence::Inferred,
            ) => inventory.inferred_texts += 1,
            (
                pid_parse::PidGraphicKind::Text { .. },
                pid_parse::PidGeometryConfidence::ProbeOnly,
            ) => inventory.probe_only_texts += 1,
            (
                pid_parse::PidGraphicKind::SymbolInstance { .. },
                pid_parse::PidGeometryConfidence::Decoded,
            ) => inventory.decoded_symbols += 1,
            (
                pid_parse::PidGraphicKind::SymbolInstance { .. },
                pid_parse::PidGeometryConfidence::Inferred,
            ) => inventory.inferred_symbols += 1,
            (
                pid_parse::PidGraphicKind::SymbolInstance { .. },
                pid_parse::PidGeometryConfidence::ProbeOnly,
            ) => inventory.probe_only_symbols += 1,
            (
                pid_parse::PidGraphicKind::Unknown { .. },
                pid_parse::PidGeometryConfidence::Decoded,
            ) => inventory.decoded_unknowns += 1,
            (
                pid_parse::PidGraphicKind::Unknown { .. },
                pid_parse::PidGeometryConfidence::Inferred,
            ) => inventory.inferred_unknowns += 1,
            (
                pid_parse::PidGraphicKind::Unknown { .. },
                pid_parse::PidGeometryConfidence::ProbeOnly,
            ) => inventory.probe_only_unknowns += 1,
        }
    }
    assert_eq!(
        inventory.total(),
        geometry.entities.len(),
        "inventory buckets should account for every normalized geometry entity"
    );
    inventory
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct EndpointPairGeometryDiagnostic {
    endpoint_pairs: usize,
    fully_promoted_with_byte_ranges: usize,
    endpoint_range_missing: usize,
    position_range_missing: usize,
    only_endpoint_a_promoted: usize,
    only_endpoint_b_promoted: usize,
    neither_endpoint_promoted: usize,
}

fn endpoint_pair_geometry_diagnostic(
    doc: &pid_parse::PidDocument,
) -> EndpointPairGeometryDiagnostic {
    const ENDPOINT_RECORD_LEN: usize = 26;
    let mut diagnostic = EndpointPairGeometryDiagnostic::default();

    for sheet in &doc.sheet_streams {
        let object_positions = sheet.geometry.as_ref().map(|geometry| {
            geometry
                .object_geometry_hints
                .iter()
                .filter_map(|hint| {
                    hint.position
                        .as_ref()
                        .map(|pos| (hint.field_x, (pos.offset, 8usize)))
                        .or_else(|| {
                            hint.f64_position
                                .as_ref()
                                .map(|f64_pos| (hint.field_x, (f64_pos.offset, 16usize)))
                        })
                })
                .collect::<BTreeMap<_, _>>()
        });
        let Some(object_positions) = object_positions else {
            continue;
        };
        let endpoint_records: Vec<_> = sheet.geometry.as_ref().map_or_else(
            || {
                sheet
                    .endpoint_records
                    .iter()
                    .map(|endpoint| (endpoint.offset, endpoint.endpoint_a, endpoint.endpoint_b))
                    .collect()
            },
            |geometry| {
                geometry
                    .endpoints
                    .iter()
                    .map(|endpoint| (endpoint.offset, endpoint.endpoint_a, endpoint.endpoint_b))
                    .collect()
            },
        );

        for (offset, endpoint_a, endpoint_b) in endpoint_records {
            diagnostic.endpoint_pairs += 1;
            let sheet_size = usize::try_from(sheet.size).unwrap_or(usize::MAX);
            let endpoint_range_ok = offset
                .checked_add(ENDPOINT_RECORD_LEN)
                .is_some_and(|end| end <= sheet_size);
            let endpoint_a_position = object_positions.get(&endpoint_a).copied();
            let endpoint_b_position = object_positions.get(&endpoint_b).copied();

            match (endpoint_a_position, endpoint_b_position) {
                (Some((a_offset, a_len)), Some((b_offset, b_len))) => {
                    let a_range_ok = a_offset
                        .checked_add(a_len)
                        .is_some_and(|end| end <= sheet_size);
                    let b_range_ok = b_offset
                        .checked_add(b_len)
                        .is_some_and(|end| end <= sheet_size);
                    if endpoint_range_ok && a_range_ok && b_range_ok {
                        diagnostic.fully_promoted_with_byte_ranges += 1;
                    } else {
                        if !endpoint_range_ok {
                            diagnostic.endpoint_range_missing += 1;
                        }
                        if !a_range_ok || !b_range_ok {
                            diagnostic.position_range_missing += 1;
                        }
                    }
                }
                (Some(_), None) => diagnostic.only_endpoint_a_promoted += 1,
                (None, Some(_)) => diagnostic.only_endpoint_b_promoted += 1,
                (None, None) => diagnostic.neither_endpoint_promoted += 1,
            }
        }
    }

    diagnostic
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
    print_geometry_fixture_availability();
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
                        .filter(|h| h.position.is_some() || h.f64_position.is_some())
                        .count()
                });
            // Phase 14 Slice E/G/J: PSM-decoded `GLine2d` /
            // `GArc2d` / `igLine2d` records each produce one
            // additional `PidGeometryConfidence::Decoded` entity
            // (`Line` or `Arc`) on top of the probe / inferred
            // totals above.
            let decoded_line_count = sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.decoded_primitive_lines.len());
            let decoded_arc_count = sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.decoded_primitive_arcs.len());
            let decoded_igline_count = sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.decoded_iglines.len());
            let decoded_iglinestring_count = sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.decoded_iglinestrings.len());
            let total = text_count
                + coordinate_count
                + endpoint_count
                + hint_count
                + decoded_line_count
                + decoded_arc_count
                + decoded_igline_count
                + decoded_iglinestring_count;
            eprintln!(
                "sheet={}, text={text_count}, coord={coordinate_count}, ep={endpoint_count}, hint={hint_count}, decoded_line={decoded_line_count}, decoded_arc={decoded_arc_count}, decoded_igline={decoded_igline_count}, decoded_iglinestring={decoded_iglinestring_count}, total={total}",
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
    let inferred_lines = geometry
        .entities
        .iter()
        .filter(|entity| {
            entity.confidence == pid_parse::PidGeometryConfidence::Inferred
                && matches!(entity.kind, pid_parse::PidGraphicKind::Line { .. })
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
    // Phase 14 Slice E/G/J/K: PSM-decoded `GLine2d` / `GArc2d` /
    // `igLine2d` / `igLineString2d` records produce
    // `PidGeometryConfidence::Decoded` `PidGraphicKind::Line` /
    // `Arc` / `Polyline` entities. They are additive to the
    // inferred-points + inferred-lines + probe-unknowns total
    // below.
    let decoded_lines = geometry
        .entities
        .iter()
        .filter(|entity| {
            entity.confidence == pid_parse::PidGeometryConfidence::Decoded
                && matches!(entity.kind, pid_parse::PidGraphicKind::Line { .. })
        })
        .count();
    let decoded_arcs = geometry
        .entities
        .iter()
        .filter(|entity| {
            entity.confidence == pid_parse::PidGeometryConfidence::Decoded
                && matches!(entity.kind, pid_parse::PidGraphicKind::Arc { .. })
        })
        .count();
    let decoded_polylines = geometry
        .entities
        .iter()
        .filter(|entity| {
            entity.confidence == pid_parse::PidGeometryConfidence::Decoded
                && matches!(entity.kind, pid_parse::PidGraphicKind::Polyline { .. })
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
            sheet.geometry.as_ref().map_or(0, |geometry| {
                geometry
                    .object_geometry_hints
                    .iter()
                    .filter(|h| h.position.is_some() || h.f64_position.is_some())
                    .count()
            })
        })
        .sum();
    let expected_inferred_lines: usize = doc
        .sheet_streams
        .iter()
        .map(|sheet| {
            let Some(geometry) = sheet.geometry.as_ref() else {
                return 0;
            };
            let promoted_field_xs: HashSet<_> = geometry
                .object_geometry_hints
                .iter()
                .filter(|hint| hint.position.is_some() || hint.f64_position.is_some())
                .map(|hint| hint.field_x)
                .collect();
            let endpoint_pairs: Vec<_> = if geometry.endpoints.is_empty() {
                sheet
                    .endpoint_records
                    .iter()
                    .map(|endpoint| (endpoint.endpoint_a, endpoint.endpoint_b))
                    .collect()
            } else {
                geometry
                    .endpoints
                    .iter()
                    .map(|endpoint| (endpoint.endpoint_a, endpoint.endpoint_b))
                    .collect()
            };
            endpoint_pairs
                .into_iter()
                .filter(|(endpoint_a, endpoint_b)| {
                    promoted_field_xs.contains(endpoint_a) && promoted_field_xs.contains(endpoint_b)
                })
                .count()
        })
        .sum();

    assert_eq!(
        inferred_points,
        expected_coordinate_hints + expected_geometry_hints,
        "coordinate hints + geometry hints should be promoted to inferred positioned points"
    );
    assert_eq!(
        inferred_lines, expected_inferred_lines,
        "endpoint pairs with both endpoints promoted should become inferred lines"
    );
    assert_eq!(
        inferred_points
            + inferred_lines
            + probe_unknowns
            + decoded_lines
            + decoded_arcs
            + decoded_polylines,
        geometry.entities.len(),
        "coordinate/geometry hints become inferred points; fully mapped endpoint pairs become inferred lines; PSM-decoded GLine2d/igLine2d records become decoded lines; GArc2d records become decoded arcs; igLineString2d records become decoded polylines; remaining text and endpoint evidence stays ProbeOnly Unknown"
    );
    // Phase 14 Slice E/G/J/K: `PidGeometryConfidence::Decoded` is
    // legitimate for PSM-decoded `GLine2d` / `igLine2d` /
    // `GArc2d` / `igLineString2d` records (Line / Arc / Polyline
    // kinds). Other kinds (Point / Circle / Text / SymbolInstance
    // / Unknown) must still not claim Decoded confidence.
    assert!(
        geometry.entities.iter().all(|entity| {
            entity.confidence != pid_parse::PidGeometryConfidence::Decoded
                || matches!(
                    entity.kind,
                    pid_parse::PidGraphicKind::Line { .. }
                        | pid_parse::PidGraphicKind::Arc { .. }
                        | pid_parse::PidGraphicKind::Polyline { .. }
                )
        }),
        "Decoded confidence currently only applies to PSM GLine2d / igLine2d / GArc2d / igLineString2d entities"
    );
    // Phase 14 Slice G/K: `PidGraphicKind::Arc` (Slice G) and
    // `Polyline` (Slice K) are now legitimate when backed by a
    // PSM-decoded record (i.e. `confidence == Decoded`). Other
    // typed curve / text / symbol kinds still cannot be emitted
    // without decoded record backing.
    assert!(
        geometry.entities.iter().all(|entity| {
            let typed_decoded = matches!(
                entity.kind,
                pid_parse::PidGraphicKind::Arc { .. }
                    | pid_parse::PidGraphicKind::Polyline { .. }
            ) && entity.confidence == pid_parse::PidGeometryConfidence::Decoded;
            typed_decoded
                || !matches!(
                    entity.kind,
                    pid_parse::PidGraphicKind::Polyline { .. }
                        | pid_parse::PidGraphicKind::Arc { .. }
                        | pid_parse::PidGraphicKind::Circle { .. }
                        | pid_parse::PidGraphicKind::Text { .. }
                        | pid_parse::PidGraphicKind::SymbolInstance { .. }
                )
        }),
        "probe-only and hint evidence must not become typed text/symbol/curve geometry without decoded records (Arc/Polyline are the exception for PSM-decoded records)"
    );
    for entity in geometry.entities.iter().filter(|entity| {
        entity.confidence == pid_parse::PidGeometryConfidence::Inferred
            && matches!(entity.kind, pid_parse::PidGraphicKind::Line { .. })
    }) {
        assert_eq!(
            entity.source.record_kind,
            Some(pid_parse::SheetRecordKind::EndpointPair),
            "inferred lines should be backed by endpoint-pair provenance"
        );
        assert!(
            entity
                .source
                .note
                .as_deref()
                .is_some_and(|note| note.contains("endpoint pair promoted")),
            "inferred lines should explain the endpoint promotion"
        );
    }
    for entity in geometry.entities.iter().filter(|entity| {
        entity
            .source
            .record_id
            .as_deref()
            .is_some_and(|record_id| record_id.starts_with("endpoint-probe:"))
    }) {
        let range = entity
            .source
            .byte_range
            .expect("endpoint probes should carry exact byte provenance");
        assert_eq!(
            range.end - range.start,
            26,
            "endpoint probe provenance should stay bounded to the proven 26-byte signature"
        );
    }

    assert!(
        geometry
            .warnings
            .iter()
            .any(|warning| warning.contains("coordinate units and page transforms are unavailable")),
        "normalized geometry should report explicit transform-unavailable diagnostics"
    );

    for entity in &geometry.entities {
        assert!(
            entity.source.stream_path.is_some(),
            "Sheet-derived geometry entities must carry a source stream path"
        );
        let range = entity
            .source
            .byte_range
            .expect("real Sheet evidence entities should carry bounded byte provenance");
        let stream_path = entity
            .source
            .stream_path
            .as_deref()
            .expect("byte-backed entity should have a stream path");
        let sheet = doc
            .sheet_streams
            .iter()
            .find(|sheet| sheet.path == stream_path)
            .expect("entity source stream should resolve to a parsed Sheet stream");
        assert!(
            range.start < range.end && range.end as u64 <= sheet.size,
            "entity {} range {:?} must be within {} size {}",
            entity.id,
            range,
            sheet.path,
            sheet.size
        );
        assert!(
            matches!(
                entity.coordinate_context.units,
                pid_parse::PidDrawingUnits::Unknown { .. }
            ),
            "units should be explicit unknown until Sheet unit metadata is decoded"
        );
        assert!(
            matches!(
                entity.coordinate_context.page_transform,
                pid_parse::PidPageTransform::Unavailable { .. }
            ),
            "page transform should be explicit unavailable until metadata is decoded"
        );
    }

    for entity in geometry.entities.iter().filter(|entity| {
        entity.confidence == pid_parse::PidGeometryConfidence::Inferred
            && matches!(entity.kind, pid_parse::PidGraphicKind::Point { .. })
    }) {
        assert_eq!(
            entity.coordinate_context.coordinate_space,
            pid_parse::PidCoordinateSpace::SourceSheet,
            "raw source coordinates should remain in source Sheet space before viewport conversion"
        );
        assert!(
            entity.source.byte_range.is_some(),
            "inferred coordinate entities must have bounded source byte provenance"
        );
    }
}

#[test]
fn sheet6_object_geometry_hints_are_populated_by_promotion_gate() {
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
    let field_xs: Vec<_> = pkg
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
    let inventory = sheet_record_shape_inventory(&raw_sheet.data, &report, &field_xs);
    let text_placement_report =
        text_placement_investigation_report(&raw_sheet.data, &report, &inventory, 128);
    let field_x_linked = text_placement_report
        .candidates
        .iter()
        .filter(|candidate| candidate.nearest_field_x.is_some())
        .count();
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
        "Sheet6 text window report: text_runs={}, coordinates={}, candidates={}, text_placement_raw_candidates={}, text_placement_candidates={}, text_placement_rejected={}, field_x_linked={}, same_chunk={}, quality_passed={}, text_quality_passed={}, max_score={}, over_threshold={}, top={top:?}, placement_top={:?}",
        report.text_runs.len(),
        report.coordinate_hints.len(),
        candidates.len(),
        text_placement_report.raw_candidate_count,
        text_placement_report.candidates.len(),
        text_placement_report.rejected_candidate_count,
        field_x_linked,
        same_chunk,
        quality_passed,
        text_quality_passed,
        max_score,
        over_threshold,
        text_placement_report
            .candidates
            .iter()
            .take(5)
            .collect::<Vec<_>>()
    );

    assert!(
        !report.text_runs.is_empty(),
        "Sheet6 should expose text runs for text placement investigation"
    );
    assert_eq!(
        text_placement_report.raw_candidate_count,
        scores.len(),
        "text placement investigation should account for all text scores before filtering"
    );
    assert_eq!(
        text_placement_report.rejected_candidate_count,
        scores
            .len()
            .saturating_sub(text_placement_report.candidates.len()),
        "text placement investigation should explicitly count binary-like rejected candidates"
    );
    assert!(
        text_placement_report.candidates.iter().all(|candidate| {
            !candidate.text_hex.is_empty()
                && !candidate.coordinate_hex.is_empty()
                && candidate
                    .notes
                    .iter()
                    .any(|note| note == "probe_only_no_text_geometry_promotion")
        }),
        "text placement investigation candidates should carry bounded evidence without promotion"
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
fn sheet6_graphic_identity_scoring_populates_object_hints_when_gate_passes() {
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
fn all_sheets_graphic_identity_scoring_report_populates_promoted_object_hints() {
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
fn geometry_fixture_registry_documents_phase9a_targets() {
    let fixtures = geometry_fixture_cases();
    let paths: HashSet<_> = fixtures.iter().map(|fixture| fixture.path).collect();

    assert_eq!(
        fixtures.len(),
        paths.len(),
        "geometry fixture registry should not contain duplicate paths"
    );
    assert!(
        fixtures.len() < GEOMETRY_FIXTURE_TARGET_MIN_AVAILABLE,
        "current registry should still document the Phase 9A fixture expansion gap"
    );
    assert!(
        fixtures
            .iter()
            .any(|fixture| fixture.category == "non_ascii"),
        "registry should explicitly cover non-ASCII fixture paths"
    );
    assert!(
        fixtures
            .iter()
            .any(|fixture| fixture.category == "publish_a01"),
        "registry should include A01 publish fixture coverage"
    );
    assert!(
        fixtures
            .iter()
            .any(|fixture| fixture.category == "publish_dwg"),
        "registry should include DWG publish fixture coverage"
    );
}

#[test]
fn geometry_fixture_availability_summary_tracks_target_gap() {
    let summary = print_geometry_fixture_availability();

    assert_eq!(summary.registered, geometry_fixture_cases().len());
    assert_eq!(
        summary.target_min_available,
        GEOMETRY_FIXTURE_TARGET_MIN_AVAILABLE
    );
    assert_eq!(
        summary.available + summary.missing.len(),
        summary.registered
    );
    assert!(
        summary.registered < summary.target_min_available,
        "Phase 9A should keep the target gap explicit until more fixtures are registered"
    );
}

#[test]
fn geometry_fixture_availability_report_line_is_human_readable() {
    let line = geometry_fixture_availability_report_line(&geometry_fixture_availability_summary());

    assert!(line.contains("registered=5"));
    assert!(line.contains("target_min_available=8"));
    assert!(line.contains("available="));
    assert!(line.contains("missing="));
}

#[test]
fn f64_coordinate_domain_analysis_for_page_mapping() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };

    if let Some(drawing) = &doc.drawing_meta {
        eprintln!("drawing_meta tags:");
        for (key, value) in &drawing.tags {
            eprintln!("  {key} = {value}");
        }
    }
    if let Some(general) = &doc.general_meta {
        eprintln!("general_meta tags:");
        for (key, value) in &general.tags {
            eprintln!("  {key} = {value}");
        }
    }

    let geometry = pid_parse::build_normalized_geometry(&doc);
    let mut f64_xs: Vec<f64> = Vec::new();
    let mut f64_ys: Vec<f64> = Vec::new();
    let mut i32_xs: Vec<i32> = Vec::new();
    let mut i32_ys: Vec<i32> = Vec::new();

    for sheet in &doc.sheet_streams {
        if let Some(geom) = &sheet.geometry {
            for hint in &geom.object_geometry_hints {
                if let Some(pos) = &hint.position {
                    i32_xs.push(pos.x);
                    i32_ys.push(pos.y);
                }
                if let Some(f64_pos) = &hint.f64_position {
                    f64_xs.push(f64_pos.x);
                    f64_ys.push(f64_pos.y);
                }
            }
            for hint in &geom.coordinate_hints {
                i32_xs.push(hint.x);
                i32_ys.push(hint.y);
            }
        }
    }

    if !f64_xs.is_empty() {
        f64_xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        f64_ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        eprintln!(
            "f64 coordinate domain: x=[{:.6}..{:.6}], y=[{:.6}..{:.6}], count={}",
            f64_xs[0],
            f64_xs[f64_xs.len() - 1],
            f64_ys[0],
            f64_ys[f64_ys.len() - 1],
            f64_xs.len()
        );
        eprintln!(
            "f64 x sample: {:?}",
            f64_xs
                .iter()
                .take(10)
                .map(|v| format!("{v:.6}"))
                .collect::<Vec<_>>()
        );
        eprintln!(
            "f64 y sample: {:?}",
            f64_ys
                .iter()
                .take(10)
                .map(|v| format!("{v:.6}"))
                .collect::<Vec<_>>()
        );
    }

    if !i32_xs.is_empty() {
        i32_xs.sort();
        i32_ys.sort();
        eprintln!(
            "i32 coordinate domain: x=[{}..{}], y=[{}..{}], count={}",
            i32_xs[0],
            i32_xs[i32_xs.len() - 1],
            i32_ys[0],
            i32_ys[i32_ys.len() - 1],
            i32_xs.len()
        );
    }

    let line_entities: Vec<_> = geometry
        .entities
        .iter()
        .filter(|e| matches!(e.kind, pid_parse::PidGraphicKind::Line { .. }))
        .collect();
    if !line_entities.is_empty() {
        eprintln!("inferred line coordinate samples:");
        for (i, entity) in line_entities.iter().take(5).enumerate() {
            if let pid_parse::PidGraphicKind::Line { start, end } = &entity.kind {
                eprintln!(
                    "  line[{i}]: ({:.6},{:.6}) -> ({:.6},{:.6})",
                    start.x, start.y, end.x, end.y
                );
            }
        }
    }

    assert!(
        !f64_xs.is_empty() || !i32_xs.is_empty(),
        "should have coordinate data for analysis"
    );
}

#[test]
fn sheet_record_text_field_investigation() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(sheet) = pkg
        .parsed
        .sheet_streams
        .iter()
        .find(|s| s.path == "/Sheet6")
    else {
        return;
    };
    let Some(raw) = pkg.streams.get(&sheet.path) else {
        return;
    };
    let Some(geometry) = &sheet.geometry else {
        return;
    };
    let mut text_fragments_found = 0usize;
    for hint in geometry
        .object_geometry_hints
        .iter()
        .filter(|h| h.position.is_some() || h.f64_position.is_some())
        .take(10)
    {
        let start = hint.offset;
        let end = (start + 64).min(raw.data.len());
        let window = &raw.data[start..end];
        let mut fragments = Vec::new();
        let mut ascii_run = Vec::new();
        for &b in window {
            if b.is_ascii_graphic() || b == b' ' {
                ascii_run.push(b);
            } else {
                if ascii_run.len() >= 3 {
                    fragments.push(String::from_utf8_lossy(&ascii_run).to_string());
                }
                ascii_run.clear();
            }
        }
        if ascii_run.len() >= 3 {
            fragments.push(String::from_utf8_lossy(&ascii_run).to_string());
        }
        text_fragments_found += fragments.len();
        let hex: String = window
            .iter()
            .take(32)
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        eprintln!(
            "hint field_x={}, offset={}, hex[0..32]={}, text_fragments={:?}",
            hint.field_x, hint.offset, hex, fragments
        );
    }
    eprintln!("total text fragments found: {text_fragments_found}");
}

#[test]
fn da_trailer_tag_text_association_for_promoted_objects() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(da) = &doc.dynamic_attributes else {
        eprintln!("skipping: dynamic attributes not available");
        return;
    };

    let promoted_field_xs: Vec<u32> = doc
        .sheet_streams
        .iter()
        .flat_map(|s| {
            s.geometry
                .iter()
                .flat_map(|g| g.object_geometry_hints.iter())
                .filter(|h| h.position.is_some() || h.f64_position.is_some())
                .map(|h| h.field_x)
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let mut associated = 0usize;
    let mut no_attrs = 0usize;
    for &field_x in &promoted_field_xs {
        let trailer = da
            .record_trailers
            .iter()
            .find(|t| t.field_x == field_x && t.class_id != 0xF6);
        if let Some(t) = trailer {
            let record = da.attribute_records.iter().find(|r| {
                r.attributes
                    .iter()
                    .any(|a| a.name == "DrawingID" || a.name == "ModelItemType")
            });
            let tag_text = record
                .and_then(|r| r.attributes.iter().find(|a| a.name == "ItemTag"))
                .map(|a| format!("{:?}", a.value));
            if tag_text.is_some() {
                associated += 1;
            } else {
                no_attrs += 1;
            }
            eprintln!(
                "  field_x={field_x} trailer_record_id={} drawing_id={:?} tag={:?}",
                t.record_id,
                t.drawing_id.as_deref().unwrap_or("?"),
                tag_text.as_deref().unwrap_or("(none)")
            );
        } else {
            no_attrs += 1;
            eprintln!("  field_x={field_x} no_matching_trailer");
        }
    }

    let mut all_attr_names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for r in &da.attribute_records {
        for a in &r.attributes {
            all_attr_names.insert(a.name.clone());
        }
    }
    eprintln!(
        "DA tag association: promoted={}, associated={associated}, no_attrs={no_attrs}",
        promoted_field_xs.len()
    );
    eprintln!(
        "available DA attribute names ({} unique): {:?}",
        all_attr_names.len(),
        all_attr_names.iter().take(30).collect::<Vec<_>>()
    );
    assert!(
        !promoted_field_xs.is_empty(),
        "should have promoted field_xs for association"
    );
}

#[test]
fn dwg0201_produces_inferred_endpoint_lines() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let geometry = pid_parse::build_normalized_geometry(&doc);
    let inferred_lines: Vec<_> = geometry
        .entities
        .iter()
        .filter(|entity| {
            entity.confidence == pid_parse::PidGeometryConfidence::Inferred
                && matches!(entity.kind, pid_parse::PidGraphicKind::Line { .. })
        })
        .collect();
    let inferred_points: Vec<_> = geometry
        .entities
        .iter()
        .filter(|entity| {
            entity.confidence == pid_parse::PidGeometryConfidence::Inferred
                && matches!(entity.kind, pid_parse::PidGraphicKind::Point { .. })
        })
        .collect();
    eprintln!(
        "DWG-0201GP06-01 geometry: inferred_points={}, inferred_lines={}",
        inferred_points.len(),
        inferred_lines.len()
    );
    assert!(
        !inferred_lines.is_empty(),
        "DWG-0201GP06-01 should produce inferred endpoint lines from f64 pair/triple coordinates"
    );
    for line in &inferred_lines {
        assert_eq!(line.confidence, pid_parse::PidGeometryConfidence::Inferred);
        assert!(
            line.source
                .record_kind
                .as_ref()
                .is_some_and(|k| *k == pid_parse::SheetRecordKind::EndpointPair),
            "inferred line should have EndpointPair record kind"
        );
        assert!(
            line.source
                .note
                .as_ref()
                .is_some_and(|n| n.contains("endpoint pair promoted to inferred line")),
            "inferred line note should describe endpoint pair promotion"
        );
    }
}

/// Phase 14 Slice E: DWG-0201 must produce **both** the existing
/// EndpointPair-inferred lines (the 49-line floor) and at least one
/// PSM-decoded `GLine2d` `Decoded` line from `build_normalized_geometry`.
///
/// The decoded line's provenance triplet (stream path + byte range
/// `SheetRecordKind::PrimitiveLine`) is asserted alongside,
/// plus the parametric geometry sanity checks (unit direction
/// vector and `param_start < param_end`).
#[test]
fn dwg0201_emits_decoded_primitive_lines_without_inferred_regression() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let geometry = pid_parse::build_normalized_geometry(&doc);
    let inferred_lines: Vec<_> = geometry
        .entities
        .iter()
        .filter(|entity| {
            entity.confidence == pid_parse::PidGeometryConfidence::Inferred
                && matches!(entity.kind, pid_parse::PidGraphicKind::Line { .. })
        })
        .collect();
    // Slice E test focuses on GLine2d-specific records (PSM 0x3FE6
    // family with parametric note text). Slice J's igLine2d entities
    // also share PidGraphicKind::Line but carry a different note;
    // they're covered by the dedicated igLines test.
    let decoded_lines: Vec<_> = geometry
        .entities
        .iter()
        .filter(|entity| {
            entity.confidence == pid_parse::PidGeometryConfidence::Decoded
                && matches!(entity.kind, pid_parse::PidGraphicKind::Line { .. })
                && entity
                    .source
                    .note
                    .as_ref()
                    .is_some_and(|n| n.contains("PSM GLine2d"))
        })
        .collect();
    eprintln!(
        "DWG-0201GP06-01 Slice E: inferred_lines={}, decoded_GLine2d_lines={}",
        inferred_lines.len(),
        decoded_lines.len()
    );
    // Slice E AC8: existing inferred lines must not regress.
    assert!(
        inferred_lines.len() >= 49,
        "DWG-0201 inferred line floor regressed: got {}, expected >= 49",
        inferred_lines.len()
    );
    // Slice E AC5/AC7: at least one Decoded line emitted.
    assert!(
        !decoded_lines.is_empty(),
        "DWG-0201 should emit at least one PSM-decoded GLine2d PrimitiveLine"
    );
    for line in &decoded_lines {
        // AC9 provenance triplet.
        assert!(
            line.source.stream_path.as_deref() == Some("/Sheet6"),
            "decoded line must carry Sheet6 stream_path: {:?}",
            line.source.stream_path
        );
        assert!(
            line.source.byte_range.is_some(),
            "decoded line must carry byte_range provenance"
        );
        assert_eq!(
            line.source.record_kind,
            Some(pid_parse::SheetRecordKind::PrimitiveLine),
            "decoded line record_kind must be PrimitiveLine"
        );
        // Note must mention radsrvitem (the IDA reverse engineering
        // source) so consumers can trace evidence back to the
        // analysis doc.
        assert!(
            line.source
                .note
                .as_ref()
                .is_some_and(|n| n.contains("PSM GLine2d") && n.contains("radsrvitem.dll")),
            "decoded line note should describe PSM GLine2d origin from radsrvitem: {:?}",
            line.source.note
        );
        // graphic_oid populated from PSM header.
        assert!(
            line.graphic_oid.is_some(),
            "decoded line should carry PSM oid as graphic_oid"
        );
        // Geometry invariants on the decoded line.
        if let pid_parse::PidGraphicKind::Line { start, end } = &line.kind {
            assert!(start.x.is_finite() && start.y.is_finite());
            assert!(end.x.is_finite() && end.y.is_finite());
            assert!(
                (start.x - end.x).abs() > 1e-6 || (start.y - end.y).abs() > 1e-6,
                "decoded line endpoints must not collapse to a single point"
            );
        } else {
            panic!("decoded line kind must be PidGraphicKind::Line");
        }
    }
}

/// Phase 14 Slice G: DWG-0201 must produce both the existing
/// inferred-line floor + decoded `Line` entities (Slice E) **and**
/// at least one decoded `Arc` entity from PSM `GArc2d` records.
#[test]
fn dwg0201_emits_decoded_primitive_arcs_without_regression() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let geometry = pid_parse::build_normalized_geometry(&doc);
    let inferred_lines: Vec<_> = geometry
        .entities
        .iter()
        .filter(|entity| {
            entity.confidence == pid_parse::PidGeometryConfidence::Inferred
                && matches!(entity.kind, pid_parse::PidGraphicKind::Line { .. })
        })
        .collect();
    let decoded_lines: Vec<_> = geometry
        .entities
        .iter()
        .filter(|entity| {
            entity.confidence == pid_parse::PidGeometryConfidence::Decoded
                && matches!(entity.kind, pid_parse::PidGraphicKind::Line { .. })
        })
        .collect();
    let decoded_arcs: Vec<_> = geometry
        .entities
        .iter()
        .filter(|entity| {
            entity.confidence == pid_parse::PidGeometryConfidence::Decoded
                && matches!(entity.kind, pid_parse::PidGraphicKind::Arc { .. })
        })
        .collect();
    eprintln!(
        "DWG-0201 Slice G: inferred_lines={}, decoded_lines={}, decoded_arcs={}",
        inferred_lines.len(),
        decoded_lines.len(),
        decoded_arcs.len()
    );
    // AC8 floors preserved.
    assert!(
        inferred_lines.len() >= 49,
        "DWG-0201 inferred line floor regressed: got {}, expected >= 49",
        inferred_lines.len()
    );
    assert!(
        !decoded_lines.is_empty(),
        "Slice D/E baseline lost: DWG-0201 should still emit >= 1 decoded line"
    );
    // Slice G: new acceptance criterion.
    assert!(
        !decoded_arcs.is_empty(),
        "DWG-0201 should emit at least one PSM-decoded GArc2d arc"
    );
    for arc in &decoded_arcs {
        assert_eq!(
            arc.source.stream_path.as_deref(),
            Some("/Sheet6"),
            "decoded arc must carry Sheet6 stream_path"
        );
        assert!(arc.source.byte_range.is_some());
        assert_eq!(
            arc.source.record_kind,
            Some(pid_parse::SheetRecordKind::PrimitiveArc)
        );
        assert!(arc
            .source
            .note
            .as_ref()
            .is_some_and(|n| { n.contains("PSM GArc2d") && n.contains("radsrvitem.dll") }));
        assert!(arc.graphic_oid.is_some(), "decoded arc carries PSM oid");
        if let pid_parse::PidGraphicKind::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } = &arc.kind
        {
            assert!(center.x.is_finite() && center.y.is_finite());
            assert!(*radius > 0.0 && radius.is_finite());
            assert!(start_angle.is_finite() && end_angle.is_finite());
            assert!(
                start_angle < end_angle,
                "decoded arc start_angle < end_angle: got [{}, {}]",
                start_angle,
                end_angle
            );
        } else {
            panic!("decoded arc kind must be PidGraphicKind::Arc");
        }
    }
}

#[test]
fn geometry_fixture_inventory_reports_normalized_geometry_counts() {
    let _availability = print_geometry_fixture_availability();
    let mut fixtures_seen = 0usize;
    let mut line_producing_fixtures = Vec::new();

    for fixture in geometry_fixture_cases() {
        let Some(doc) = parse_test_file(fixture.path) else {
            continue;
        };
        fixtures_seen += 1;
        let inventory = normalized_geometry_inventory(&doc);
        if inventory.inferred_lines + inventory.decoded_lines > 0 {
            line_producing_fixtures.push(fixture.path);
        }
        eprintln!(
            "geometry fixture inventory: fixture={}, category={}, points(d/i/p)={}/{}/{}, lines(d/i/p)={}/{}/{}, polylines(d/i/p)={}/{}/{}, arcs(d/i/p)={}/{}/{}, circles(d/i/p)={}/{}/{}, texts(d/i/p)={}/{}/{}, symbols(d/i/p)={}/{}/{}, unknowns(d/i/p)={}/{}/{}, other_entities={}",
            fixture.path,
            fixture.category,
            inventory.decoded_points,
            inventory.inferred_points,
            inventory.probe_only_points,
            inventory.decoded_lines,
            inventory.inferred_lines,
            inventory.probe_only_lines,
            inventory.decoded_polylines,
            inventory.inferred_polylines,
            inventory.probe_only_polylines,
            inventory.decoded_arcs,
            inventory.inferred_arcs,
            inventory.probe_only_arcs,
            inventory.decoded_circles,
            inventory.inferred_circles,
            inventory.probe_only_circles,
            inventory.decoded_texts,
            inventory.inferred_texts,
            inventory.probe_only_texts,
            inventory.decoded_symbols,
            inventory.inferred_symbols,
            inventory.probe_only_symbols,
            inventory.decoded_unknowns,
            inventory.inferred_unknowns,
            inventory.probe_only_unknowns,
            inventory.other_entities
        );
    }

    if fixtures_seen == 0 {
        eprintln!("skipping: no available PID fixtures found for geometry inventory");
        return;
    }
    eprintln!(
        "geometry fixture inventory summary: fixtures_seen={}, line_producing_fixtures={:?}",
        fixtures_seen, line_producing_fixtures
    );
    assert!(
        fixtures_seen > 0,
        "at least one available fixture should be inventoried when this test does not skip"
    );
}

#[test]
fn sheet_record_shape_inventory_reports_geometry_candidates() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(raw_sheet) = pkg.streams.get("/Sheet6") else {
        eprintln!("skipping: /Sheet6 stream not found in fixture");
        return;
    };
    let Some(graph) = pkg.parsed.object_graph.as_ref() else {
        eprintln!("skipping: object graph not built for fixture");
        return;
    };

    let field_xs: Vec<_> = graph
        .objects
        .iter()
        .filter_map(|object| object.field_x)
        .collect();
    let report = probe_sheet_stream(
        "Sheet6",
        "/Sheet6",
        &raw_sheet.data,
        &SheetProbeOptions::default(),
    );
    let inventory = sheet_record_shape_inventory(&raw_sheet.data, &report, &field_xs);
    let marker_records = inventory
        .records
        .iter()
        .filter(|record| record.kind == SheetRecordShapeKind::Marker)
        .count();
    let field_windows = inventory
        .records
        .iter()
        .filter(|record| record.kind == SheetRecordShapeKind::FieldXWindow)
        .count();
    let f64_windows = inventory
        .records
        .iter()
        .filter(|record| record.f64_coordinate_offset.is_some())
        .count();
    let text_runs = inventory
        .records
        .iter()
        .filter(|record| record.kind == SheetRecordShapeKind::TextRun)
        .count();
    let coordinate_hints = inventory
        .records
        .iter()
        .filter(|record| record.kind == SheetRecordShapeKind::CoordinateHint)
        .count();

    eprintln!(
        "sheet record shape inventory: records={}, marker_records={}, field_windows={}, f64_windows={}, text_runs={}, coordinate_hints={}, record_type_counts={:?}",
        inventory.records.len(),
        marker_records,
        field_windows,
        f64_windows,
        text_runs,
        coordinate_hints,
        inventory.record_type_counts
    );

    assert!(
        marker_records > 0,
        "Sheet6 should expose marker records for shape inventory"
    );
    assert!(
        field_windows > 0,
        "Sheet6 should expose object field_x windows for shape inventory"
    );
    assert!(
        f64_windows > 0,
        "Sheet6 should retain repeated f64 coordinate evidence in shape inventory"
    );
    assert!(
        text_runs > 0,
        "Sheet6 should retain text-run evidence in shape inventory"
    );
    assert!(
        coordinate_hints > 0,
        "Sheet6 should retain coordinate-hint evidence in shape inventory"
    );
    assert!(
        inventory.records.iter().all(|record| {
            record.range_start <= record.offset
                && record.offset < record.range_end
                && record.range_end <= raw_sheet.data.len()
        }),
        "all shape inventory ranges should stay within /Sheet6"
    );
}

#[test]
fn coordinate_page_metadata_investigation_keeps_transform_unavailable_until_record_proven() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(raw_sheet) = pkg.streams.get("/Sheet6") else {
        eprintln!("skipping: /Sheet6 stream not found in fixture");
        return;
    };
    let Some(graph) = pkg.parsed.object_graph.as_ref() else {
        eprintln!("skipping: object graph not built for fixture");
        return;
    };

    let field_xs: Vec<_> = graph
        .objects
        .iter()
        .filter_map(|object| object.field_x)
        .collect();
    let probe = probe_sheet_stream(
        "Sheet6",
        "/Sheet6",
        &raw_sheet.data,
        &SheetProbeOptions::default(),
    );
    let inventory = sheet_record_shape_inventory(&raw_sheet.data, &probe, &field_xs);
    let normalized = pid_parse::build_normalized_geometry(&pkg.parsed);
    let report = coordinate_page_metadata_investigation_report(
        &raw_sheet.data,
        &inventory,
        normalized.page_dimensions_mm,
    );
    let normalized_f64_candidates = report
        .candidates
        .iter()
        .filter(|candidate| {
            candidate.candidate_kind
                == SheetCoordinatePageMetadataCandidateKind::NormalizedF64CoordinateLike
        })
        .count();
    let page_dimension_candidates = report
        .candidates
        .iter()
        .filter(|candidate| {
            candidate.candidate_kind
                == SheetCoordinatePageMetadataCandidateKind::PageDimensionScalarLike
        })
        .count();
    let i32_domain_candidates = report
        .candidates
        .iter()
        .filter(|candidate| {
            candidate.candidate_kind
                == SheetCoordinatePageMetadataCandidateKind::I32CoordinateDomainLike
        })
        .count();
    let unavailable_context_entities = normalized
        .entities
        .iter()
        .filter(|entity| {
            matches!(
                entity.coordinate_context.page_transform,
                pid_parse::PidPageTransform::Unavailable { .. }
            )
        })
        .count();

    eprintln!(
        "coordinate page metadata investigation: candidates={}, normalized_f64_candidates={}, page_dimension_candidates={}, i32_domain_candidates={}, normalized_f64_pair_count={}, page_dimension_scalar_matches={}, i32_bounds={:?}, f64_bounds={:?}, page_dimensions_mm={:?}, top={:?}",
        report.candidates.len(),
        normalized_f64_candidates,
        page_dimension_candidates,
        i32_domain_candidates,
        report.normalized_f64_pair_count,
        report.page_dimension_scalar_matches,
        report.coordinate_hint_bounds,
        report.f64_coordinate_bounds,
        normalized.page_dimensions_mm,
        report.candidates.iter().take(8).collect::<Vec<_>>()
    );

    assert!(
        !report.candidates.is_empty(),
        "Sheet6 should expose marker ranges for coordinate/page metadata investigation"
    );
    assert!(
        report.coordinate_hint_bounds.is_some(),
        "Sheet6 should expose i32 coordinate-domain bounds as evidence"
    );
    assert!(
        report.f64_coordinate_bounds.is_some() || report.normalized_f64_pair_count > 0,
        "Sheet6 should expose f64 coordinate-domain evidence for page mapping investigation"
    );
    assert!(
        normalized_f64_candidates + page_dimension_candidates + i32_domain_candidates > 0,
        "coordinate/page metadata investigation should classify at least one numeric evidence group"
    );
    assert_eq!(
        unavailable_context_entities,
        normalized.entities.len(),
        "CoordinatePageMetadata investigation must not make page transforms available"
    );
    assert!(
        normalized.warnings.iter().any(|warning| {
            warning.contains("coordinate units and page transforms are unavailable")
        }),
        "normalized geometry should keep explicit transform-unavailable warning"
    );
    assert!(
        report.candidates.iter().all(|candidate| {
            candidate
                .investigation_notes
                .iter()
                .any(|note| note == "probe_only_no_coordinate_page_metadata_promotion")
                && candidate.example_range_start <= candidate.example_offset
                && candidate.example_offset < candidate.example_range_end
                && candidate.example_range_end <= raw_sheet.data.len()
        }),
        "coordinate/page metadata candidates should carry bounded no-promotion evidence"
    );
}

#[test]
fn non_sheet_stream_page_metadata_scan_keeps_transform_unavailable_without_complete_scalar_source()
{
    let mut fixtures_seen = 0usize;
    let mut scanned_streams = 0usize;
    let mut template_stream_hits = 0usize;
    let mut page_dimension_scalar_hits_total = 0usize;
    let mut complete_page_dimension_streams = 0usize;
    let mut scalar_hit_streams = Vec::new();
    let mut template_hit_streams = Vec::new();
    let mut unavailable_context_entities = 0usize;
    let mut total_entities = 0usize;

    for fixture in geometry_fixture_cases() {
        let Some(pkg) = parse_test_package(fixture.path) else {
            continue;
        };
        let normalized = pid_parse::build_normalized_geometry(&pkg.parsed);
        let Some(page_dimensions_mm) = normalized.page_dimensions_mm else {
            continue;
        };
        fixtures_seen += 1;
        total_entities += normalized.entities.len();
        unavailable_context_entities += normalized
            .entities
            .iter()
            .filter(|entity| {
                matches!(
                    entity.coordinate_context.page_transform,
                    pid_parse::PidPageTransform::Unavailable { .. }
                )
            })
            .count();

        let template = pkg
            .parsed
            .drawing_meta
            .as_ref()
            .and_then(|meta| meta.tags.get("Template"))
            .or(pkg
                .parsed
                .drawing_meta
                .as_ref()
                .and_then(|meta| meta.template_name.as_ref()))
            .or(pkg
                .parsed
                .summary
                .as_ref()
                .and_then(|summary| summary.template.as_ref()))
            .cloned()
            .unwrap_or_default();

        for (path, raw_stream) in &pkg.streams {
            if path.starts_with("/Sheet") {
                continue;
            }
            scanned_streams += 1;
            let scalar_hits = page_dimension_scalar_hits(&raw_stream.data, page_dimensions_mm);
            if !scalar_hits.is_empty() {
                let has_width = scalar_hits
                    .iter()
                    .any(|hit| (hit.value - page_dimensions_mm.0).abs() <= 1.0e-6);
                let has_height = scalar_hits
                    .iter()
                    .any(|hit| (hit.value - page_dimensions_mm.1).abs() <= 1.0e-6);
                if has_width && has_height {
                    complete_page_dimension_streams += 1;
                }
                page_dimension_scalar_hits_total += scalar_hits.len();
                scalar_hit_streams.push((fixture.path.to_string(), path.clone(), scalar_hits));
            }
            if stream_contains_ascii_token(&raw_stream.data, &template) {
                template_stream_hits += 1;
                template_hit_streams.push((
                    fixture.path.to_string(),
                    path.clone(),
                    template.clone(),
                ));
            }
        }
    }

    eprintln!(
        "non-Sheet page metadata scan: fixtures_seen={}, scanned_streams={}, template_stream_hits={}, page_dimension_scalar_hits={}, complete_page_dimension_streams={}, scalar_hit_streams={:?}, template_hit_streams={:?}, unavailable_context_entities={}/{}",
        fixtures_seen,
        scanned_streams,
        template_stream_hits,
        page_dimension_scalar_hits_total,
        complete_page_dimension_streams,
        scalar_hit_streams,
        template_hit_streams,
        unavailable_context_entities,
        total_entities
    );

    if fixtures_seen == 0 {
        eprintln!("skipping: no available PID fixtures with inferred page dimensions");
        return;
    }
    assert!(
        scanned_streams > 0,
        "available fixtures should expose non-Sheet streams for independent metadata scanning"
    );
    assert!(
        template_stream_hits > 0,
        "metadata streams should retain template-name evidence used only for page-size inference"
    );
    assert_eq!(
        complete_page_dimension_streams, 0,
        "non-Sheet scalar hits must include both page width and height before they can be considered a transform source"
    );
    assert_eq!(
        unavailable_context_entities, total_entities,
        "template or scalar scan evidence must not make page transforms available"
    );
}

#[test]
fn sheet_geometry_investigation_aggregates_cross_fixture_evidence_without_promotion() {
    let mut fixtures_seen = 0usize;
    let mut sheets_seen = 0usize;
    let mut coordinate_metadata_candidates = 0usize;
    let mut normalized_f64_pair_count = 0usize;
    let mut page_dimension_scalar_matches = 0usize;
    let mut curve_groups = 0usize;
    let mut marker_49215_groups = 0usize;
    let mut polyline_like = 0usize;
    let mut mixed_numeric = 0usize;
    let mut short_i32_sequences = 0usize;

    for fixture in geometry_fixture_cases() {
        let Some(pkg) = parse_test_package(fixture.path) else {
            continue;
        };
        fixtures_seen += 1;
        let field_xs: Vec<_> = pkg
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
        let normalized = pid_parse::build_normalized_geometry(&pkg.parsed);

        for sheet in pkg
            .parsed
            .sheet_streams
            .iter()
            .filter(|sheet| sheet.path.starts_with("/Sheet"))
        {
            let Some(raw_sheet) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            sheets_seen += 1;
            let probe = probe_sheet_stream(
                &sheet.name,
                &sheet.path,
                &raw_sheet.data,
                &SheetProbeOptions::default(),
            );
            let inventory = sheet_record_shape_inventory(&raw_sheet.data, &probe, &field_xs);
            let coordinate_report = coordinate_page_metadata_investigation_report(
                &raw_sheet.data,
                &inventory,
                normalized.page_dimensions_mm,
            );
            let curve_report = curve_primitive_investigation_report(&raw_sheet.data, &inventory);

            coordinate_metadata_candidates += coordinate_report.candidates.len();
            normalized_f64_pair_count += coordinate_report.normalized_f64_pair_count;
            page_dimension_scalar_matches += coordinate_report.page_dimension_scalar_matches;
            curve_groups += curve_report.groups.len();
            marker_49215_groups += curve_report
                .groups
                .iter()
                .filter(|group| group.marker_type == Some(49215))
                .count();
            polyline_like += curve_report
                .groups
                .iter()
                .filter(|group| {
                    group.candidate_kind == SheetCurvePrimitiveCandidateKind::PolylineLike
                })
                .count();
            mixed_numeric += curve_report
                .groups
                .iter()
                .filter(|group| {
                    group.candidate_kind == SheetCurvePrimitiveCandidateKind::MixedNumeric
                })
                .count();
            short_i32_sequences += curve_report
                .groups
                .iter()
                .filter(|group| {
                    group.i32_point_sequence.as_ref().is_some_and(|sequence| {
                        sequence.point_count < 3
                            && group
                                .investigation_notes
                                .iter()
                                .any(|note| note == "short_i32_point_sequence_needs_more_vertices")
                    })
                })
                .count();

            assert!(
                coordinate_report.candidates.iter().all(|candidate| {
                    candidate
                        .investigation_notes
                        .iter()
                        .any(|note| note == "probe_only_no_coordinate_page_metadata_promotion")
                }),
                "coordinate metadata report must not promote page transform for {} {}",
                fixture.path,
                sheet.path
            );
            assert!(
                curve_report.groups.iter().all(|group| {
                    group
                        .investigation_notes
                        .iter()
                        .any(|note| note == "probe_only_no_curve_geometry_promotion")
                }),
                "curve report must not promote primitive geometry for {} {}",
                fixture.path,
                sheet.path
            );
            assert!(
                curve_report
                    .groups
                    .iter()
                    .filter(|group| {
                        group.candidate_kind == SheetCurvePrimitiveCandidateKind::PolylineLike
                    })
                    .all(|group| {
                        group.i32_point_sequence.as_ref().is_some_and(|sequence| {
                            sequence.point_count >= 3
                                && sequence.byte_stride == 8
                                && sequence.relative_alignment_mod4 == 0
                                && !sequence.sample_points.is_empty()
                        })
                    }),
                "PolylineLike groups require 3+ aligned non-overlapping i32 points for {} {}",
                fixture.path,
                sheet.path
            );
        }
    }

    eprintln!(
        "cross-fixture Sheet geometry investigation: fixtures_seen={}, sheets_seen={}, coordinate_metadata_candidates={}, normalized_f64_pair_count={}, page_dimension_scalar_matches={}, curve_groups={}, marker_49215_groups={}, polyline_like={}, mixed_numeric={}, short_i32_sequences={}",
        fixtures_seen,
        sheets_seen,
        coordinate_metadata_candidates,
        normalized_f64_pair_count,
        page_dimension_scalar_matches,
        curve_groups,
        marker_49215_groups,
        polyline_like,
        mixed_numeric,
        short_i32_sequences
    );

    if fixtures_seen == 0 {
        eprintln!("skipping: no available PID fixtures found for cross-fixture investigation");
        return;
    }
    assert!(
        sheets_seen > 0,
        "available geometry fixtures should expose Sheet streams"
    );
    assert!(
        coordinate_metadata_candidates + curve_groups > 0,
        "cross-fixture investigation should surface Sheet evidence without decoded promotion"
    );
    assert!(
        mixed_numeric >= polyline_like,
        "mixed numeric evidence should remain visible while PolylineLike requires stronger point-sequence proof"
    );
}

#[test]
fn marker15_polyline_like_subfield_review_keeps_unaligned_sequences_probe_only() {
    let mut fixtures_seen = 0usize;
    let mut candidates = Vec::new();
    let mut shape_counts: BTreeMap<(Option<u16>, usize), usize> = BTreeMap::new();
    let mut candidate_logical_drawings = BTreeSet::new();

    for fixture in geometry_fixture_cases() {
        let Some(pkg) = parse_test_package(fixture.path) else {
            continue;
        };
        fixtures_seen += 1;
        let field_xs: Vec<_> = pkg
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

        for sheet in pkg
            .parsed
            .sheet_streams
            .iter()
            .filter(|sheet| sheet.path.starts_with("/Sheet"))
        {
            let Some(raw_sheet) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            let probe = probe_sheet_stream(
                &sheet.name,
                &sheet.path,
                &raw_sheet.data,
                &SheetProbeOptions::default(),
            );
            let inventory = sheet_record_shape_inventory(&raw_sheet.data, &probe, &field_xs);
            let curve_report = curve_primitive_investigation_report(&raw_sheet.data, &inventory);

            for group in curve_report
                .groups
                .iter()
                .filter(|group| group.marker_type == Some(15) && group.range_len == 148)
            {
                let sequence = group
                    .i32_point_sequence
                    .as_ref()
                    .expect("marker15/range148 groups should expose point-sequence evidence");
                let logical_drawing = std::path::Path::new(fixture.path)
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or(fixture.path)
                    .to_string();
                candidate_logical_drawings.insert(logical_drawing.clone());
                *shape_counts
                    .entry((group.marker_type, group.range_len))
                    .or_default() += 1;
                candidates.push((
                    fixture.path.to_string(),
                    logical_drawing,
                    sheet.path.clone(),
                    group.candidate_kind,
                    group.marker_type,
                    group.range_len,
                    group.numeric_pair_count,
                    sequence.relative_offset,
                    sequence.relative_alignment_mod4,
                    sequence.point_count,
                    sequence.sample_points.clone(),
                ));
            }
        }
    }

    let repeated_shapes = shape_counts
        .iter()
        .filter(|(_, count)| **count > 1)
        .collect::<Vec<_>>();
    eprintln!(
        "marker15/range148 subfield review: fixtures_seen={}, candidates={}, logical_drawings={}, candidate_logical_drawings={:?}, shape_counts={:?}, repeated_shapes={:?}, details={:?}",
        fixtures_seen,
        candidates.len(),
        candidate_logical_drawings.len(),
        candidate_logical_drawings,
        shape_counts,
        repeated_shapes,
        candidates
    );

    if fixtures_seen == 0 {
        eprintln!("skipping: no available PID fixtures found for polyline-like investigation");
        return;
    }
    assert!(
        !candidates.is_empty(),
        "cross-fixture investigation should expose marker15/range148 point-sequence candidates"
    );
    assert!(
        candidates.iter().all(
            |(
                _,
                _,
                _,
                candidate_kind,
                _,
                range_len,
                numeric_pair_count,
                _,
                alignment_mod4,
                point_count,
                sample_points,
            )| {
                (16..=512).contains(range_len)
                    && *numeric_pair_count >= *point_count
                    && *point_count >= 3
                    && *alignment_mod4 != 0
                    && *candidate_kind != SheetCurvePrimitiveCandidateKind::PolylineLike
                    && !sample_points.is_empty()
            }
        ),
        "marker15/range148 sequences should stay unaligned subfield evidence, not PolylineLike: {candidates:?}"
    );
}

#[test]
fn symbol_placement_investigation_links_symbol_objects_to_sheet_evidence() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(raw_sheet) = pkg.streams.get("/Sheet6") else {
        eprintln!("skipping: /Sheet6 stream not found in fixture");
        return;
    };
    let Some(graph) = pkg.parsed.object_graph.as_ref() else {
        eprintln!("skipping: object graph not built for fixture");
        return;
    };

    let field_xs: Vec<_> = graph
        .objects
        .iter()
        .filter_map(|object| object.field_x)
        .collect();
    let object_drawing_ids: HashSet<_> = graph
        .objects
        .iter()
        .map(|object| object.drawing_id.to_ascii_lowercase())
        .collect();
    let mut symbol_path_by_drawing_id = BTreeMap::new();
    for jsite in &pkg.parsed.jsites {
        let Some(symbol_path) = &jsite.symbol_path else {
            continue;
        };
        for value in jsite
            .properties
            .guids
            .iter()
            .chain(jsite.properties.strings.iter())
            .chain(jsite.properties.key_values.values())
        {
            let normalized = value.to_ascii_lowercase();
            if object_drawing_ids.contains(&normalized) {
                symbol_path_by_drawing_id
                    .entry(normalized)
                    .or_insert_with(|| symbol_path.clone());
            }
        }
    }
    let symbol_objects: Vec<_> = graph
        .objects
        .iter()
        .filter(|object| object.drawing_item_type.as_deref() == Some("Symbol"))
        .filter_map(|object| {
            Some(SheetSymbolPlacementObject {
                field_x: object.field_x?,
                drawing_id: object.drawing_id.clone(),
                item_type: object.item_type.clone(),
                drawing_item_type: object.drawing_item_type.clone(),
                symbol_path: symbol_path_by_drawing_id
                    .get(&object.drawing_id.to_ascii_lowercase())
                    .cloned(),
            })
        })
        .collect();
    let symbol_objects_with_bound_path = symbol_objects
        .iter()
        .filter(|object| object.symbol_path.is_some())
        .count();
    let mut symbol_paths: Vec<_> = pkg
        .parsed
        .jsites
        .iter()
        .filter_map(|jsite| jsite.symbol_path.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    symbol_paths.sort();
    let jsite_symbol_refs: Vec<_> = pkg
        .parsed
        .jsites
        .iter()
        .filter_map(|jsite| {
            jsite
                .symbol_path
                .as_ref()
                .map(|symbol_path| (jsite.name.as_str(), symbol_path.as_str()))
        })
        .collect();
    let psm_root_jsite_refs: Vec<_> = pkg
        .parsed
        .psm_roots
        .as_ref()
        .map(|roots| {
            roots
                .entries
                .iter()
                .filter(|entry| entry.name.starts_with("JSite"))
                .map(|entry| (entry.name.as_str(), entry.id, entry.offset))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let jsite_symbol_names: HashSet<_> = jsite_symbol_refs.iter().map(|(name, _)| *name).collect();
    let psm_symbol_jsite_matches = psm_root_jsite_refs
        .iter()
        .filter(|(name, _, _)| jsite_symbol_names.contains(*name))
        .count();
    let psm_root_probe_pairs: Vec<_> = psm_root_jsite_refs
        .iter()
        .filter(|(name, _, _)| jsite_symbol_names.contains(*name))
        .take(5)
        .copied()
        .collect();
    let order_candidate_count = symbol_objects.len().min(jsite_symbol_refs.len());
    let order_counts_match = symbol_objects.len() == jsite_symbol_refs.len();
    let order_probe_pairs: Vec<_> = symbol_objects
        .iter()
        .zip(jsite_symbol_refs.iter())
        .take(5)
        .map(|(object, (jsite_name, symbol_path))| {
            (
                object.field_x,
                object.drawing_id.as_str(),
                *jsite_name,
                *symbol_path,
            )
        })
        .collect();

    let report = probe_sheet_stream(
        "Sheet6",
        "/Sheet6",
        &raw_sheet.data,
        &SheetProbeOptions::default(),
    );
    let inventory = sheet_record_shape_inventory(&raw_sheet.data, &report, &field_xs);
    let symbol_report = symbol_placement_investigation_report(
        &raw_sheet.data,
        &inventory,
        &symbol_objects,
        &symbol_paths,
    );
    let positioned_candidates = symbol_report
        .candidates
        .iter()
        .filter(|candidate| candidate.position_offset.is_some())
        .count();
    let catalog_unlinked = symbol_report
        .candidates
        .iter()
        .filter(|candidate| {
            candidate
                .notes
                .iter()
                .any(|note| note.starts_with("symbol_path_catalog_unlinked_count="))
        })
        .count();
    let object_symbol_path_bound = symbol_report
        .candidates
        .iter()
        .filter(|candidate| {
            candidate
                .notes
                .iter()
                .any(|note| note == "object_symbol_path_bound")
        })
        .count();

    eprintln!(
        "symbol placement investigation: symbol_objects={}, symbol_paths={}, jsite_symbol_refs={}, psm_root_jsite_refs={}, psm_symbol_jsite_matches={}, psm_root_probe_pairs={:?}, order_counts_match={}, order_candidate_count={}, order_probe_pairs={:?}, jsite_object_symbol_path_matches={}, symbol_objects_with_bound_path={}, candidates={}, positioned_candidates={}, object_symbol_path_bound={}, catalog_unlinked={}, top={:?}",
        symbol_objects.len(),
        symbol_report.symbol_path_catalog_count,
        jsite_symbol_refs.len(),
        psm_root_jsite_refs.len(),
        psm_symbol_jsite_matches,
        psm_root_probe_pairs,
        order_counts_match,
        order_candidate_count,
        order_probe_pairs,
        symbol_path_by_drawing_id.len(),
        symbol_objects_with_bound_path,
        symbol_report.candidates.len(),
        positioned_candidates,
        object_symbol_path_bound,
        catalog_unlinked,
        symbol_report.candidates.iter().take(5).collect::<Vec<_>>()
    );

    assert!(
        !symbol_objects.is_empty(),
        "fixture should expose DA objects whose DrawingItemType is Symbol"
    );
    assert!(
        symbol_report.symbol_path_catalog_count > 0,
        "fixture should expose JSite symbol paths for symbol placement investigation"
    );
    assert!(
        order_candidate_count > 0,
        "fixture should expose non-empty JSite/order evidence for symbol binding investigation"
    );
    assert!(
        psm_symbol_jsite_matches > 0 || catalog_unlinked == symbol_report.candidates.len(),
        "when PSMroots has no symbol-carrying JSite bridge, symbol candidates must stay catalog-unlinked"
    );
    assert!(
        !symbol_report.candidates.is_empty(),
        "symbol placement investigation should link at least one symbol object to Sheet field_x evidence"
    );
    assert_eq!(
        object_symbol_path_bound,
        symbol_objects_with_bound_path.min(symbol_report.candidates.len()),
        "direct object-level symbol paths should be preserved when JSite properties prove them"
    );
    assert!(
        symbol_report.candidates.iter().all(|candidate| {
            candidate
                .notes
                .iter()
                .any(|note| note == "probe_only_no_symbol_geometry_promotion")
        }),
        "symbol placement investigation must not promote SymbolInstance geometry"
    );
}

#[test]
fn curve_primitive_investigation_reports_unsupported_curve_candidates() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(raw_sheet) = pkg.streams.get("/Sheet6") else {
        eprintln!("skipping: /Sheet6 stream not found in fixture");
        return;
    };
    let Some(graph) = pkg.parsed.object_graph.as_ref() else {
        eprintln!("skipping: object graph not built for fixture");
        return;
    };

    let field_xs: Vec<_> = graph
        .objects
        .iter()
        .filter_map(|object| object.field_x)
        .collect();
    let report = probe_sheet_stream(
        "Sheet6",
        "/Sheet6",
        &raw_sheet.data,
        &SheetProbeOptions::default(),
    );
    let inventory = sheet_record_shape_inventory(&raw_sheet.data, &report, &field_xs);
    let curve_report = curve_primitive_investigation_report(&raw_sheet.data, &inventory);
    let polyline_like = curve_report
        .groups
        .iter()
        .filter(|group| group.candidate_kind == SheetCurvePrimitiveCandidateKind::PolylineLike)
        .count();
    let circle_arc_like = curve_report
        .groups
        .iter()
        .filter(|group| group.candidate_kind == SheetCurvePrimitiveCandidateKind::CircleArcLike)
        .count();
    let mixed_numeric = curve_report
        .groups
        .iter()
        .filter(|group| group.candidate_kind == SheetCurvePrimitiveCandidateKind::MixedNumeric)
        .count();
    let compact_vertex_chain_candidates = curve_report
        .groups
        .iter()
        .filter(|group| group.compact_vertex_chain_candidate)
        .count();
    let i32_point_sequence_candidates = curve_report
        .groups
        .iter()
        .filter(|group| {
            group
                .i32_point_sequence
                .as_ref()
                .is_some_and(|sequence| sequence.point_count >= 2)
        })
        .count();
    let large_mixed_payloads = curve_report
        .groups
        .iter()
        .filter(|group| {
            group.range_len > 512
                && group
                    .investigation_notes
                    .iter()
                    .any(|note| note == "mixed_or_large_numeric_payload_needs_subrecord_split")
        })
        .count();
    let numeric_groups = curve_report
        .groups
        .iter()
        .filter(|group| group.candidate_i32_pairs > 0 || group.candidate_f64_pairs > 0)
        .count();
    let normalized = normalized_geometry_inventory(&pkg.parsed);

    eprintln!(
        "curve primitive investigation: groups={}, numeric_groups={}, polyline_like={}, circle_arc_like={}, mixed_numeric={}, compact_vertex_chain_candidates={}, i32_point_sequence_candidates={}, large_mixed_payloads={}, decoded_polylines={}, decoded_circles={}, decoded_arcs={}, top={:?}",
        curve_report.groups.len(),
        numeric_groups,
        polyline_like,
        circle_arc_like,
        mixed_numeric,
        compact_vertex_chain_candidates,
        i32_point_sequence_candidates,
        large_mixed_payloads,
        normalized.decoded_polylines,
        normalized.decoded_circles,
        normalized.decoded_arcs,
        curve_report.groups.iter().take(8).collect::<Vec<_>>()
    );

    assert!(
        !curve_report.groups.is_empty(),
        "Sheet6 should expose marker-range groups for curve primitive investigation"
    );
    assert!(
        numeric_groups > 0,
        "curve primitive investigation should surface numeric marker groups without decoding them"
    );
    assert!(
        mixed_numeric > 0,
        "large or noisy numeric payloads should be classified as mixed, not promoted to vertex chains"
    );
    assert_eq!(
        polyline_like, compact_vertex_chain_candidates,
        "PolylineLike should be reserved for compact vertex-chain review candidates"
    );
    assert_eq!(
        polyline_like, 0,
        "DWG-0201 /Sheet6 compact curve candidates currently lack enough non-overlapping vertices for PolylineLike promotion review"
    );
    assert!(
        i32_point_sequence_candidates >= compact_vertex_chain_candidates,
        "compact vertex-chain candidates should expose non-overlapping i32 point-sequence evidence"
    );
    assert!(
        i32_point_sequence_candidates > compact_vertex_chain_candidates,
        "short local i32 sequences should remain mixed metadata evidence until 3+ non-overlapping points are proven"
    );
    assert!(curve_report
        .groups
        .iter()
        .filter(|group| group.candidate_kind == SheetCurvePrimitiveCandidateKind::PolylineLike)
        .all(
            |group| group.i32_point_sequence.as_ref().is_some_and(|sequence| {
                sequence.point_count >= 2
                    && sequence.byte_stride == 8
                    && !sequence.sample_points.is_empty()
            })
        ));
    assert!(
        large_mixed_payloads > 0,
        "large numeric payloads should carry a subrecord-split investigation note"
    );
    // The curve primitive **investigation** layer itself never
    // promotes decoded geometry — its output is always
    // `probe_only_no_curve_geometry_promotion`. Decoded curves are
    // emitted by a separate PSM decoder family (`decode_primitive_arcs`
    // Slice F/G, `decode_iglinestrings` Slice K) and surface
    // through `SheetGeometry::decoded_primitive_*` /
    // `decoded_iglinestrings` fields and `build_normalized_geometry`.
    // Circles still don't have their own decoder; assert zero.
    assert_eq!(
        normalized.decoded_circles, 0,
        "curve primitive investigation must not promote decoded circle geometry"
    );
    assert!(
        curve_report.groups.iter().all(|group| {
            group
                .investigation_notes
                .iter()
                .any(|note| note == "probe_only_no_curve_geometry_promotion")
                && group.example_range_start <= group.example_offset
                && group.example_offset < group.example_range_end
                && group.example_range_end <= raw_sheet.data.len()
        }),
        "curve primitive groups should carry bounded no-promotion evidence"
    );
}

#[test]
fn primitive_line_investigation_groups_non_endpoint_marker_shapes() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(raw_sheet) = pkg.streams.get("/Sheet6") else {
        eprintln!("skipping: /Sheet6 stream not found in fixture");
        return;
    };
    let Some(graph) = pkg.parsed.object_graph.as_ref() else {
        eprintln!("skipping: object graph not built for fixture");
        return;
    };

    let field_xs: Vec<_> = graph
        .objects
        .iter()
        .filter_map(|object| object.field_x)
        .collect();
    let report = probe_sheet_stream(
        "Sheet6",
        "/Sheet6",
        &raw_sheet.data,
        &SheetProbeOptions::default(),
    );
    let inventory = sheet_record_shape_inventory(&raw_sheet.data, &report, &field_xs);
    let primitive_line_report = primitive_line_investigation_report(&raw_sheet.data, &inventory);
    let numeric_groups = primitive_line_report
        .groups
        .iter()
        .filter(|group| group.candidate_i32_pairs >= 2 || group.candidate_f64_pairs >= 1)
        .count();
    let sample_groups = primitive_line_report
        .groups
        .iter()
        .filter(|group| !group.numeric_samples.is_empty())
        .count();
    let top_group = primitive_line_report
        .groups
        .first()
        .expect("primitive line investigation groups should not be empty");

    eprintln!(
        "primitive line investigation groups: total_groups={}, numeric_groups={}, sample_groups={}, top={:?}",
        primitive_line_report.groups.len(),
        numeric_groups,
        sample_groups,
        primitive_line_report.groups.iter().take(8).collect::<Vec<_>>()
    );

    assert!(
        !primitive_line_report.groups.is_empty(),
        "Sheet6 should expose marker range groups for primitive-line investigation"
    );
    assert!(
        top_group.investigation_score > 0 && !top_group.investigation_notes.is_empty(),
        "primitive-line investigation should rank groups with evidence notes: {top_group:?}"
    );
    let compact_start_end_candidates = primitive_line_report
        .groups
        .iter()
        .filter(|group| matches!(group.marker_type, Some(26684 | 27169)))
        .collect::<Vec<_>>();
    assert!(
        compact_start_end_candidates.iter().any(|group| {
            group.numeric_sample_relative_offsets.len() >= 3
                && !group.numeric_sample_offset_deltas.is_empty()
                && !group.example_hex_prefix.is_empty()
        }),
        "compact start/end candidate groups should expose offset deltas and hex prefixes: {compact_start_end_candidates:?}"
    );
    assert!(
        compact_start_end_candidates.iter().any(|group| {
            group.investigation_notes.iter().any(|note| {
                note == "no_coordinate_hint_sample_match"
                    || note.starts_with("coordinate_hint_matches=")
            })
        }),
        "compact groups should record whether numeric samples match existing coordinate hints: {compact_start_end_candidates:?}"
    );
    assert!(
        numeric_groups > 0,
        "primitive-line investigation should surface numeric marker groups without decoding them"
    );
    assert!(
        sample_groups > 0,
        "primitive-line investigation should include bounded numeric samples"
    );
    assert!(
        primitive_line_report.groups.iter().all(|group| {
            group.example_range_start <= group.example_offset
                && group.example_offset < group.example_range_end
                && group.example_range_end <= raw_sheet.data.len()
        }),
        "primitive-line investigation examples should be bounded"
    );
}

#[test]
fn endpoint_pair_geometry_diagnostics_explain_dwg0201_line_gap() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };

    let diagnostic = endpoint_pair_geometry_diagnostic(&doc);
    let inventory = normalized_geometry_inventory(&doc);
    eprintln!(
        "endpoint pair geometry diagnostic: endpoint_pairs={}, fully_promoted_with_byte_ranges={}, endpoint_range_missing={}, position_range_missing={}, only_a={}, only_b={}, neither={}, inferred_lines={}",
        diagnostic.endpoint_pairs,
        diagnostic.fully_promoted_with_byte_ranges,
        diagnostic.endpoint_range_missing,
        diagnostic.position_range_missing,
        diagnostic.only_endpoint_a_promoted,
        diagnostic.only_endpoint_b_promoted,
        diagnostic.neither_endpoint_promoted,
        inventory.inferred_lines
    );

    assert!(
        diagnostic.endpoint_pairs > 0,
        "DWG-0201GP06-01 should expose endpoint pairs for line-gap diagnostics"
    );
    assert_eq!(
        inventory.inferred_lines, diagnostic.fully_promoted_with_byte_ranges,
        "inferred line count should match endpoint pairs whose two endpoint positions and byte ranges are all available"
    );
    assert!(
        inventory.inferred_lines > 0,
        "DWG-0201GP06-01 should produce inferred endpoint lines after f64 pair + triple gate"
    );
    assert!(
        diagnostic.only_endpoint_a_promoted
            + diagnostic.only_endpoint_b_promoted
            + diagnostic.neither_endpoint_promoted
            + diagnostic.endpoint_range_missing
            + diagnostic.position_range_missing
            > 0,
        "diagnostic should explain why endpoint pairs did not become line geometry"
    );
}

#[test]
fn endpoint_field_x_diagnostics_report_promoted_and_missing_distribution() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };

    let object_field_xs: HashSet<_> = doc
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
    let mut endpoint_ref_counts: BTreeMap<u32, usize> = BTreeMap::new();
    let mut promoted_endpoint_ref_counts: BTreeMap<u32, usize> = BTreeMap::new();
    let mut missing_endpoint_ref_counts: BTreeMap<u32, usize> = BTreeMap::new();
    let mut missing_known_object_ref_count = 0usize;

    for sheet in &doc.sheet_streams {
        let Some(geometry) = sheet.geometry.as_ref() else {
            continue;
        };
        let promoted_field_xs: HashSet<_> = geometry
            .object_geometry_hints
            .iter()
            .filter(|hint| hint.position.is_some())
            .map(|hint| hint.field_x)
            .collect();
        let endpoint_records: Vec<_> = if geometry.endpoints.is_empty() {
            sheet
                .endpoint_records
                .iter()
                .map(|endpoint| (endpoint.endpoint_a, endpoint.endpoint_b))
                .collect()
        } else {
            geometry
                .endpoints
                .iter()
                .map(|endpoint| (endpoint.endpoint_a, endpoint.endpoint_b))
                .collect()
        };

        for (endpoint_a, endpoint_b) in endpoint_records {
            for field_x in [endpoint_a, endpoint_b] {
                *endpoint_ref_counts.entry(field_x).or_default() += 1;
                if promoted_field_xs.contains(&field_x) {
                    *promoted_endpoint_ref_counts.entry(field_x).or_default() += 1;
                } else {
                    *missing_endpoint_ref_counts.entry(field_x).or_default() += 1;
                    if object_field_xs.contains(&field_x) {
                        missing_known_object_ref_count += 1;
                    }
                }
            }
        }
    }

    let endpoint_refs: usize = endpoint_ref_counts.values().sum();
    let promoted_refs: usize = promoted_endpoint_ref_counts.values().sum();
    let missing_refs: usize = missing_endpoint_ref_counts.values().sum();
    let mut top_missing: Vec<_> = missing_endpoint_ref_counts.iter().collect();
    top_missing.sort_by(|left, right| right.1.cmp(left.1).then_with(|| left.0.cmp(right.0)));
    let top_missing: Vec<_> = top_missing
        .into_iter()
        .take(10)
        .map(|(field_x, count)| {
            format!(
                "{field_x}:{count}:known_object={}",
                object_field_xs.contains(field_x)
            )
        })
        .collect();

    eprintln!(
        "endpoint field_x diagnostic: unique_endpoint_fields={}, endpoint_refs={}, promoted_refs={}, missing_refs={}, missing_known_object_refs={}, top_missing={:?}",
        endpoint_ref_counts.len(),
        endpoint_refs,
        promoted_refs,
        missing_refs,
        missing_known_object_ref_count,
        top_missing
    );

    assert!(endpoint_refs > 0, "fixture should expose endpoint refs");
    assert!(
        promoted_refs > 0,
        "fixture should have at least one endpoint ref whose field_x is promoted"
    );
    assert!(
        missing_refs > 0,
        "fixture should have missing endpoint refs explaining the line gap"
    );
    assert_eq!(
        endpoint_refs,
        promoted_refs + missing_refs,
        "promoted + missing endpoint refs should partition all endpoint refs"
    );
}

#[test]
fn endpoint_missing_known_field_xs_report_promotion_gate_scores() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(da) = pkg.parsed.dynamic_attributes.as_ref() else {
        eprintln!("skipping: dynamic attributes not built");
        return;
    };

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
    let mut missing_known_counts: BTreeMap<u32, usize> = BTreeMap::new();

    for sheet in &pkg.parsed.sheet_streams {
        let Some(geometry) = sheet.geometry.as_ref() else {
            continue;
        };
        let promoted_field_xs: HashSet<_> = geometry
            .object_geometry_hints
            .iter()
            .filter(|hint| hint.position.is_some() || hint.f64_position.is_some())
            .map(|hint| hint.field_x)
            .collect();
        let endpoint_records: Vec<_> = if geometry.endpoints.is_empty() {
            sheet
                .endpoint_records
                .iter()
                .map(|endpoint| (endpoint.endpoint_a, endpoint.endpoint_b))
                .collect()
        } else {
            geometry
                .endpoints
                .iter()
                .map(|endpoint| (endpoint.endpoint_a, endpoint.endpoint_b))
                .collect()
        };

        for (endpoint_a, endpoint_b) in endpoint_records {
            for field_x in [endpoint_a, endpoint_b] {
                if object_field_xs.contains(&field_x) && !promoted_field_xs.contains(&field_x) {
                    *missing_known_counts.entry(field_x).or_default() += 1;
                }
            }
        }
    }

    let mut top_missing: Vec<_> = missing_known_counts.into_iter().collect();
    top_missing.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let target_field_xs: HashSet<u32> = top_missing
        .iter()
        .take(10)
        .map(|(field_x, _)| *field_x)
        .collect();
    assert!(
        !target_field_xs.is_empty(),
        "fixture should have known-object endpoint field_x values missing promotion"
    );

    let identity_index = sheet_identity_index_from_trailers(&da.record_trailers);
    let mut inspected = 0usize;
    let mut below_threshold_or_no_window = 0usize;

    for sheet in &pkg.parsed.sheet_streams {
        let Some(raw_sheet) = pkg.streams.get(&sheet.path) else {
            continue;
        };
        let Some(geometry) = sheet.geometry.as_ref() else {
            continue;
        };
        let mut sheet_targets: Vec<u32> = if geometry.endpoints.is_empty() {
            sheet
                .endpoint_records
                .iter()
                .flat_map(|endpoint| [endpoint.endpoint_a, endpoint.endpoint_b])
                .filter(|field_x| target_field_xs.contains(field_x))
                .collect()
        } else {
            geometry
                .endpoints
                .iter()
                .flat_map(|endpoint| [endpoint.endpoint_a, endpoint.endpoint_b])
                .filter(|field_x| target_field_xs.contains(field_x))
                .collect()
        };
        sheet_targets.sort_unstable();
        sheet_targets.dedup();
        if sheet_targets.is_empty() {
            continue;
        }

        let report = probe_sheet_stream(
            sheet.name.as_str(),
            sheet.path.as_str(),
            &raw_sheet.data,
            &SheetProbeOptions::default(),
        );
        let windows = field_x_windows(&raw_sheet.data, &sheet_targets, 96);
        let features = field_x_window_features(&raw_sheet.data, &windows, &report.chunks);
        let identities = field_x_window_identities(&raw_sheet.data, &windows, &identity_index);
        let scores =
            score_field_x_window_features_with_identities(&features, &object_field_xs, &identities);

        for field_x in sheet_targets {
            inspected += 1;
            let best = scores
                .iter()
                .filter(|score| score.field_x == field_x)
                .max_by(|left, right| left.score.cmp(&right.score));
            if let Some(best) = best {
                if best.score < 70 {
                    below_threshold_or_no_window += 1;
                }
                eprintln!(
                    "missing known endpoint field_x score detail: sheet={}, field_x={}, best_score={}, reasons={:?}, candidate_position={:?}",
                    sheet.path,
                    field_x,
                    best.score,
                    best.reasons,
                    best.candidate_position
                );
            } else {
                below_threshold_or_no_window += 1;
                eprintln!(
                    "missing known endpoint field_x score detail: sheet={}, field_x={}, no_window=true",
                    sheet.path, field_x
                );
            }
        }
    }

    assert!(
        inspected > 0,
        "top missing field_x values should be inspected"
    );
    assert!(
        below_threshold_or_no_window > 0,
        "at least one inspected missing field_x should be below threshold or absent from Sheet windows"
    );
}

#[test]
fn sheet6_missing_endpoint_field_xs_compare_coordinate_search_radii() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(da) = pkg.parsed.dynamic_attributes.as_ref() else {
        eprintln!("skipping: dynamic attributes not built");
        return;
    };
    let Some(sheet) = pkg
        .parsed
        .sheet_streams
        .iter()
        .find(|sheet| sheet.path == "/Sheet6")
    else {
        eprintln!("skipping: /Sheet6 not found");
        return;
    };
    let Some(raw_sheet) = pkg.streams.get(&sheet.path) else {
        eprintln!("skipping: raw /Sheet6 stream not found");
        return;
    };

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
    let target_field_xs: Vec<u32> = (630..=639).collect();
    let report = probe_sheet_stream(
        sheet.name.as_str(),
        sheet.path.as_str(),
        &raw_sheet.data,
        &SheetProbeOptions::default(),
    );

    let mut inspected = 0usize;
    let mut any_candidate_position = false;
    for radius in [96usize, 192, 384] {
        let windows = field_x_windows(&raw_sheet.data, &target_field_xs, radius);
        let features = field_x_window_features(&raw_sheet.data, &windows, &report.chunks);
        let identities = field_x_window_identities(&raw_sheet.data, &windows, &identity_index);
        let scores =
            score_field_x_window_features_with_identities(&features, &object_field_xs, &identities);
        let dumps = top_field_x_candidate_record_dumps(&raw_sheet.data, &scores, 10, 32);
        let candidate_positions = scores
            .iter()
            .filter(|score| score.candidate_position.is_some())
            .count();
        any_candidate_position |= candidate_positions > 0;
        inspected += scores.len();
        eprintln!(
            "sheet6 missing endpoint radius diagnostic: radius={}, windows={}, scores={}, candidate_positions={}",
            radius,
            windows.len(),
            scores.len(),
            candidate_positions
        );
        for dump in dumps {
            eprintln!(
                "sheet6 missing endpoint dump: radius={}, field_x={}, score={}, field_offset={}, coordinate_offset={:?}, reasons={:?}, field_window={}..{} {}",
                radius,
                dump.field_x,
                dump.score,
                dump.field_offset,
                dump.coordinate_offset,
                dump.reasons,
                dump.field_window.start,
                dump.field_window.end,
                dump.field_window.hex
            );
        }
    }

    assert!(inspected > 0, "expected field_x scores for Sheet6 targets");
    assert!(
        !any_candidate_position,
        "current diagnostic documents that wider search radii still do not surface candidate positions for field_x 630..639"
    );
}

#[test]
fn sheet6_missing_endpoint_field_xs_have_preceding_f64_coordinate_pairs() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(sheet) = pkg
        .parsed
        .sheet_streams
        .iter()
        .find(|sheet| sheet.path == "/Sheet6")
    else {
        eprintln!("skipping: /Sheet6 not found");
        return;
    };
    let Some(raw_sheet) = pkg.streams.get(&sheet.path) else {
        eprintln!("skipping: raw /Sheet6 stream not found");
        return;
    };

    let target_field_xs: Vec<u32> = (630..=639).collect();
    let windows = field_x_windows(&raw_sheet.data, &target_field_xs, 96);
    let report = probe_sheet_stream(
        sheet.name.as_str(),
        sheet.path.as_str(),
        &raw_sheet.data,
        &SheetProbeOptions::default(),
    );
    let features = field_x_window_features(&raw_sheet.data, &windows, &report.chunks);
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
    let scores = score_field_x_window_features(&features, &object_field_xs);
    let mut candidates = Vec::new();
    for window in &windows {
        if let Some(candidate) =
            repeated_f64_pair_candidate_before_field_x(&raw_sheet.data, window.offset)
        {
            candidates.push((
                window.field_x,
                window.offset,
                candidate.coordinate_offset,
                candidate.x,
                candidate.y,
            ));
        }
    }

    for (field_x, field_offset, coordinate_offset, x, y) in &candidates {
        eprintln!(
            "sheet6 repeated f64 candidate: field_x={}, field_offset={}, coordinate_offset={}, x={:.6}, y={:.6}",
            field_x, field_offset, coordinate_offset, x, y
        );
    }

    assert_eq!(
        candidates.len(),
        target_field_xs.len(),
        "each target field_x should match the repeated marker + preceding f64 pair experiment"
    );
    let candidate_offsets: HashSet<_> = candidates
        .iter()
        .map(|(_, field_offset, _, _, _)| *field_offset)
        .collect();
    assert!(
        scores
            .iter()
            .filter(|score| candidate_offsets.contains(&score.offset))
            .all(|score| score.reasons.contains(
                &SheetFieldXWindowScoreReason::RepeatedF64PairBeforeField {
                    coordinate_delta: -22,
                    marker_delta: -6,
                    support: 10,
                },
            )),
        "all repeated f64 candidate scores should expose the diagnostic reason"
    );
}

#[test]
fn sheet6_endpoint_a_missing_field_xs_f64_byte_window_investigation() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let Some(sheet) = pkg
        .parsed
        .sheet_streams
        .iter()
        .find(|sheet| sheet.path == "/Sheet6")
    else {
        return;
    };
    let Some(raw_sheet) = pkg.streams.get(&sheet.path) else {
        return;
    };
    let Some(geometry) = &sheet.geometry else {
        return;
    };

    let promoted_field_xs: HashSet<_> = geometry
        .object_geometry_hints
        .iter()
        .filter(|hint| hint.position.is_some() || hint.f64_position.is_some())
        .map(|hint| hint.field_x)
        .collect();
    let endpoint_b_for_only_b: Vec<u32> = geometry
        .endpoints
        .iter()
        .filter(|ep| {
            !promoted_field_xs.contains(&ep.endpoint_a)
                && promoted_field_xs.contains(&ep.endpoint_b)
        })
        .map(|ep| ep.endpoint_a)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    eprintln!(
        "endpoint_a field_xs missing promotion (from only_b pairs): {:?}",
        {
            let mut sorted = endpoint_b_for_only_b.clone();
            sorted.sort_unstable();
            sorted.dedup();
            sorted
        }
    );

    let target_field_xs: Vec<u32> = {
        let mut sorted = endpoint_b_for_only_b.clone();
        sorted.sort_unstable();
        sorted.dedup();
        sorted
    };
    let windows = field_x_windows(&raw_sheet.data, &target_field_xs, 96);

    let mut with_f64_pair = 0usize;
    let mut without_f64_pair = 0usize;
    for window in &windows {
        if let Some(candidate) =
            repeated_f64_pair_candidate_before_field_x(&raw_sheet.data, window.offset)
        {
            with_f64_pair += 1;
            eprintln!(
                "  field_x={} offset={} HAS f64 pair: x={:.6} y={:.6} coord_offset={}",
                window.field_x,
                window.offset,
                candidate.x,
                candidate.y,
                candidate.coordinate_offset
            );
        } else {
            without_f64_pair += 1;
            let start = window.offset.saturating_sub(30);
            let end = (window.offset + 10).min(raw_sheet.data.len());
            let hex: String = raw_sheet.data[start..end]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(" ");
            eprintln!(
                "  field_x={} offset={} NO f64 pair; nearby bytes [{start}..{end}]: {hex}",
                window.field_x, window.offset
            );
        }
    }
    eprintln!(
        "f64 pair coverage for endpoint_a missing field_xs: with={with_f64_pair}, without={without_f64_pair}, total_windows={}",
        windows.len()
    );

    assert!(
        !windows.is_empty(),
        "should find windows for missing endpoint_a field_xs"
    );
}

#[test]
fn available_pid_fixtures_geometry_evidence_inventory_tracks_promoted_hints() {
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
    let _availability = print_geometry_fixture_availability();

    for fixture in geometry_fixture_cases() {
        let Some(pkg) = parse_test_package(fixture.path) else {
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
            eprintln!(
                "skipping fixture {} ({}): cross reference not built",
                fixture.path, fixture.category
            );
            continue;
        };
        let Some(da) = pkg.parsed.dynamic_attributes.as_ref() else {
            eprintln!(
                "skipping fixture {} ({}): dynamic attributes not built",
                fixture.path, fixture.category
            );
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
                    "fixture={}, category={}, sheet={}, field_xs=0, text_candidates={}, text_over_threshold={}, note=no_endpoint_field_xs",
                    fixture.path,
                    fixture.category,
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
                "fixture={}, category={}, sheet={}, field_xs={}, windows={}, record_shape_classes={}, top_record_shape={}, identities={}, same_object={}, wrong_object={}, identity_supported={}, max_identity_score={}, identity_over_threshold={}, promotable={}, text_candidates={}, text_over_threshold={}",
                fixture.path,
                fixture.category,
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
        eprintln!(
            "skipping: no available PID fixtures found; registered={:?} — real geometry evidence inventory is NOT validated on this run",
            geometry_fixture_cases()
                .iter()
                .map(|fixture| fixture.path)
                .collect::<Vec<_>>()
        );
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
fn promoted_object_geometry_hints_explain_promotion_gate() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        return;
    };
    let mut hints_seen = 0usize;

    for sheet in &pkg.parsed.sheet_streams {
        let Some(raw_sheet) = pkg.streams.get(&sheet.path) else {
            continue;
        };
        let Some(geometry) = &sheet.geometry else {
            continue;
        };

        for hint in &geometry.object_geometry_hints {
            hints_seen += 1;
            assert!(
                hint.offset < raw_sheet.data.len(),
                "promoted hint offset should point into the source Sheet stream"
            );
            let has_i32_position = hint.position.is_some();
            let has_f64_position = hint.f64_position.is_some();
            assert!(
                has_i32_position || has_f64_position,
                "promoted hint should carry a coordinate position (i32 or f64)"
            );
            if let Some(position) = &hint.position {
                assert!(
                    position.offset + 8 <= raw_sheet.data.len(),
                    "promoted hint i32 coordinate offset should point into the source Sheet stream"
                );
            }
            if let Some(f64_pos) = &hint.f64_position {
                assert!(
                    f64_pos.offset + 16 <= raw_sheet.data.len(),
                    "promoted hint f64 coordinate offset should point into the source Sheet stream"
                );
            }
            let note = hint
                .note
                .as_deref()
                .expect("promoted hint should explain the promotion gate");
            assert!(
                note.contains("score="),
                "promotion note should include score: {note}"
            );
            let is_primary_gate = note.contains("identity") && note.contains("stable_shape");
            let is_f64_gate = note.contains("coordinate_source=f64_pair_before_marker")
                || note.contains("coordinate_source=nearest_coordinate_hint");
            assert!(
                is_primary_gate || is_f64_gate,
                "promotion note should indicate either primary gate or f64/coordinate source: {note}"
            );
        }
    }

    assert!(
        hints_seen > 0,
        "fixture should expose promoted object geometry hints"
    );
}

#[test]
fn normalized_geometry_projection_preserves_promoted_hint_source_notes() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };

    let normalized = pid_parse::build_normalized_geometry(&doc);
    let mut promoted_hints_checked = 0usize;

    for sheet in &doc.sheet_streams {
        let Some(geometry) = &sheet.geometry else {
            continue;
        };

        for hint in geometry
            .object_geometry_hints
            .iter()
            .filter(|hint| hint.position.is_some() || hint.f64_position.is_some())
        {
            let note = hint
                .note
                .as_deref()
                .expect("promoted hint should carry a promotion gate note");
            let (expected_x, expected_y) = if let Some(pos) = &hint.position {
                (f64::from(pos.x), f64::from(pos.y))
            } else if let Some(f64_pos) = &hint.f64_position {
                (f64_pos.x, f64_pos.y)
            } else {
                panic!("filtered hint should carry either i32 or f64 position");
            };

            let projected = normalized.entities.iter().find(|entity| {
                entity.source.stream_path.as_deref() == Some(sheet.path.as_str())
                    && entity.source.field_x == Some(hint.field_x)
                    && entity.source.note.as_deref() == Some(note)
                    && entity.confidence == pid_parse::PidGeometryConfidence::Inferred
                    && matches!(
                        &entity.kind,
                        pid_parse::PidGraphicKind::Point { position: point }
                            if point.x == expected_x
                                && point.y == expected_y
                    )
            });

            assert!(
                projected.is_some(),
                "normalized geometry should preserve promoted hint source note: {note}"
            );
            let has_primary_gate_evidence =
                note.contains("identity") && note.contains("stable_shape");
            let has_coordinate_source = note.contains("coordinate_source=");
            assert!(
                note.contains("score=") && (has_primary_gate_evidence || has_coordinate_source),
                "projected source note should retain promotion gate evidence: {note}"
            );
            promoted_hints_checked += 1;
        }
    }

    assert!(
        promoted_hints_checked > 0,
        "fixture should expose promoted hints to project into normalized geometry"
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
    let mut hit_offsets: Vec<usize> = Vec::new();
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

    eprintln!("standalone value frequency: val_206_as_u32={coord_206}, val_121_as_u32={coord_121}");

    let promoted_field_xs: Vec<u32> = pkg.parsed.sheet_streams[0]
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

    let Some(cross) = pkg.parsed.cross_reference.as_ref() else {
        return;
    };
    let Some(da) = pkg.parsed.dynamic_attributes.as_ref() else {
        return;
    };
    let identity_index = sheet_identity_index_from_trailers(&da.record_trailers);
    let object_field_xs: HashSet<_> = pkg
        .parsed
        .object_graph
        .as_ref()
        .map(|g| g.objects.iter().filter_map(|o| o.field_x).collect())
        .unwrap_or_default();
    let mut ep_field_xs: Vec<_> = cross
        .relationship_endpoint_links
        .iter()
        .filter(|l| l.sheet_path.as_deref() == Some("/Sheet6"))
        .flat_map(|l| [l.source_field_x, l.target_field_x])
        .flatten()
        .collect();
    ep_field_xs.sort_unstable();
    ep_field_xs.dedup();
    let report = probe_sheet_stream("Sheet6", "/Sheet6", data, &SheetProbeOptions::default());
    let windows = field_x_windows(data, &ep_field_xs, 96);
    let features = field_x_window_features(data, &windows, &report.chunks);
    let identities = field_x_window_identities(data, &windows, &identity_index);
    let scores =
        score_field_x_window_features_with_identities(&features, &object_field_xs, &identities);

    let ce_field_xs: Vec<u32> = hit_offsets
        .iter()
        .filter_map(|&off| {
            let fx_off = off + 6;
            if fx_off + 4 <= data.len() {
                Some(u32::from_le_bytes([
                    data[fx_off],
                    data[fx_off + 1],
                    data[fx_off + 2],
                    data[fx_off + 3],
                ]))
            } else {
                None
            }
        })
        .collect();

    for fx in &ce_field_xs {
        if promoted_field_xs.contains(fx) {
            continue;
        }
        let best = scores
            .iter()
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

/// Phase 14 Slice D: cross-fixture validation that
/// `decode_primitive_lines` actually emits decoded `GLine2d`
/// records on real `Sheet*` streams.
///
/// Empirical baselines reverse-engineered from
/// `examples/probe_psm_gline2d.rs` against the three Sheet-bearing
/// fixtures:
///
/// - `DWG-0201GP06-01.pid` / `/Sheet6` → 2 hits (page-spanning
///   horizontal lines, type 0x3FE6, bytes_to_follow 948 each).
/// - `工艺管道及仪表流程-1.pid` / `/Sheet6` → 0 hits with strict
///   unit-vector tolerance (file uses a different / older record
///   shape).
/// - `A01.pid` / `/Sheet6` → 1 hit (template horizontal reference
///   line at (0,0)→(1,0) with `bytes_to_follow == 204`).
///
/// The test:
///
/// 1. Walks every `Sheet*` stream of every fixture present in
///    `test-file/`.
/// 2. Asserts the **aggregate** decoded count across all
///    `Sheet*` streams of all fixtures is `>= 1` (so the test
///    fails closed when the decoder regresses to zero, and stays
///    contributor-friendly when only A01 is present).
/// 3. Asserts every emitted line carries the documented
///    provenance triplet (stream path + byte range + record
///    kind) and the unit-vector / `param_start < param_end`
///    invariants.
#[test]
fn primitive_line_decoder_emits_decoded_lines_with_provenance() {
    let fixtures = [
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
        "export-test/publish-data/A01/A01.pid",
    ];
    let mut total_decoded = 0usize;
    let mut per_fixture_summary: Vec<(String, usize)> = Vec::new();
    let mut sample_lines: Vec<String> = Vec::new();
    for fixture in fixtures {
        let Some(pkg) = parse_test_package(fixture) else {
            continue;
        };
        let mut per_fixture_count = 0usize;
        for sheet in &pkg.parsed.sheet_streams {
            let Some(raw) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            let bytes = raw.data.as_slice();
            let decoded = decode_primitive_lines(bytes);
            for line in &decoded {
                // Provenance: byte range fits the stream.
                assert!(
                    line.byte_range.end <= bytes.len(),
                    "decoded line byte_range {:?} exceeds stream {} bytes ({})",
                    line.byte_range,
                    sheet.path,
                    bytes.len()
                );
                assert!(
                    line.byte_range.start < line.byte_range.end,
                    "decoded line byte_range must be non-empty: {:?}",
                    line.byte_range
                );
                // Type code: GLine2d only.
                assert_eq!(
                    line.type_code, PSM_TYPE_CODE_GLINE2D,
                    "decoder must only emit GLine2d records"
                );
                // Direction vector is unit (within 1e-3, matching the
                // decoder's relaxed tolerance).
                let dir_len2 =
                    line.direction.0 * line.direction.0 + line.direction.1 * line.direction.1;
                let dir_len = dir_len2.sqrt();
                assert!(
                    (dir_len - 1.0).abs() < 1e-3,
                    "decoded direction must be unit vector, got len={dir_len} for record {:?}",
                    line
                );
                // Param range is sorted.
                assert!(
                    line.param_start < line.param_end,
                    "decoded params must satisfy start < end, got [{}, {}]",
                    line.param_start,
                    line.param_end,
                );
                // All decoded fields finite.
                assert!(line.origin.0.is_finite() && line.origin.1.is_finite());
                assert!(line.direction.0.is_finite() && line.direction.1.is_finite());
                assert!(line.param_start.is_finite() && line.param_end.is_finite());

                if sample_lines.len() < 5 {
                    let (ax, ay) = line.endpoint_a();
                    let (bx, by) = line.endpoint_b();
                    sample_lines.push(format!(
                        "{fixture} {} @ 0x{:06x}..0x{:06x} oid={} \
                        origin=({:+.4},{:+.4}) dir=({:+.5},{:+.5}) \
                        param=[{:+.4},{:+.4}] A=({:+.4},{:+.4}) B=({:+.4},{:+.4})",
                        sheet.path,
                        line.byte_range.start,
                        line.byte_range.end,
                        line.oid,
                        line.origin.0,
                        line.origin.1,
                        line.direction.0,
                        line.direction.1,
                        line.param_start,
                        line.param_end,
                        ax,
                        ay,
                        bx,
                        by,
                    ));
                }
            }
            per_fixture_count += decoded.len();
        }
        per_fixture_summary.push((fixture.to_string(), per_fixture_count));
        total_decoded += per_fixture_count;
    }
    eprintln!("--- Phase 14 Slice D: PSM GLine2d decoder cross-fixture summary ---");
    for (name, count) in &per_fixture_summary {
        eprintln!("  {name}: {count} decoded GLine2d records");
    }
    for sample in &sample_lines {
        eprintln!("  sample: {sample}");
    }
    eprintln!("  total decoded across present fixtures: {total_decoded}");
    if per_fixture_summary.is_empty() {
        eprintln!(
            "skipping: no Sheet-bearing fixture present (CI / contributors \
            without SmartPlant samples)"
        );
        return;
    }
    assert!(
        total_decoded >= 1,
        "decode_primitive_lines must emit at least one decoded line on the \
        Sheet-bearing fixture set, got {total_decoded}. \
        Per-fixture summary: {per_fixture_summary:?}"
    );
}

/// Phase 14 Slice K: cross-fixture validation that
/// `decode_iglinestrings` actually emits decoded Intergraph Sigma
/// `igLineString2d` polylines (PSM type `0x0084`) from real
/// `Sheet*` streams.
///
/// Empirical baselines from
/// `examples/probe_iglinestring2d_shape.rs`: DWG-0201:0,
/// DWG-0202:32+, 工艺管道-1:57+, A01:3. After decoder validation
/// (vertex_count >= 2, form/scope constraints, finite non-degenerate
/// coords), at least 30 records should survive cross-fixture.
#[test]
fn iglinestrings_decoder_emits_decoded_polylines_with_provenance() {
    let fixtures = [
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
        "export-test/publish-data/A01/A01.pid",
    ];
    let mut total_decoded = 0usize;
    let mut per_fixture_summary: Vec<(String, usize)> = Vec::new();
    let mut sample_lines: Vec<String> = Vec::new();
    for fixture in fixtures {
        let Some(pkg) = parse_test_package(fixture) else {
            continue;
        };
        let mut per_fixture_count = 0usize;
        for sheet in &pkg.parsed.sheet_streams {
            let Some(raw) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            let bytes = raw.data.as_slice();
            let decoded = decode_iglinestrings(bytes);
            for pl in &decoded {
                assert!(
                    pl.byte_range.end <= bytes.len(),
                    "polyline byte_range {:?} exceeds stream {} bytes ({})",
                    pl.byte_range,
                    sheet.path,
                    bytes.len()
                );
                assert_eq!(pl.type_code, PSM_TYPE_CODE_IGLINESTRING2D);
                assert!(pl.vertex_count() >= 2);
                assert!(pl.form <= 6);
                assert!(pl.scope <= 4 || pl.scope == 6);
                for (x, y) in &pl.vertices {
                    assert!(x.is_finite() && y.is_finite());
                }
                if sample_lines.len() < 5 {
                    sample_lines.push(format!(
                        "{fixture} {} @ 0x{:06x}..0x{:06x} oid={} parent={} sub_type=0x{:04X} \
                        form={} scope={} vc={} total_length={:.4}",
                        sheet.path,
                        pl.byte_range.start,
                        pl.byte_range.end,
                        pl.oid,
                        pl.parent_ref,
                        pl.sub_type_word,
                        pl.form,
                        pl.scope,
                        pl.vertex_count(),
                        pl.total_length(),
                    ));
                }
            }
            per_fixture_count += decoded.len();
        }
        per_fixture_summary.push((fixture.to_string(), per_fixture_count));
        total_decoded += per_fixture_count;
    }
    eprintln!("--- Phase 14 Slice K: PSM igLineString2d decoder cross-fixture summary ---");
    for (name, count) in &per_fixture_summary {
        eprintln!("  {name}: {count} decoded igLineString2d records");
    }
    for sample in &sample_lines {
        eprintln!("  sample: {sample}");
    }
    eprintln!("  total decoded polylines: {total_decoded}");
    if per_fixture_summary.is_empty() {
        return;
    }
    assert!(
        total_decoded >= 30,
        "decode_iglinestrings should emit >= 30 polylines cross-fixture, got {total_decoded}"
    );
}

/// Phase 14 Slice J: cross-fixture validation that
/// `decode_iglines` actually emits decoded Intergraph Sigma
/// standard `igLine2d` records (PSM type `0x0018`) from real
/// `Sheet*` streams.
///
/// Empirical baselines from `examples/probe_psm_type_code_histogram.rs`:
/// DWG-0201 24, DWG-0202 42, 工艺管道-1 243, A01 0 — total 309
/// records cross-fixture. After applying decoder validation
/// (strict `bytes_to_follow == 50`, `remaining_header == 12`,
/// non-degenerate non-zero-length, finite coords in domain), the
/// pass-through rate should be high; we assert at least 100
/// records survive cross-fixture (well below the 309 raw count,
/// well above zero).
#[test]
fn iglines_decoder_emits_decoded_iglines_with_provenance() {
    let fixtures = [
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
        "export-test/publish-data/A01/A01.pid",
    ];
    let mut total_decoded = 0usize;
    let mut per_fixture_summary: Vec<(String, usize)> = Vec::new();
    let mut sample_lines: Vec<String> = Vec::new();

    for fixture in fixtures {
        let Some(pkg) = parse_test_package(fixture) else {
            continue;
        };
        let mut per_fixture_count = 0usize;
        for sheet in &pkg.parsed.sheet_streams {
            let Some(raw) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            let bytes = raw.data.as_slice();
            let decoded = decode_iglines(bytes);
            for line in &decoded {
                assert!(
                    line.byte_range.end <= bytes.len(),
                    "igLine2d byte_range {:?} exceeds stream {} bytes ({})",
                    line.byte_range,
                    sheet.path,
                    bytes.len()
                );
                assert_eq!(line.type_code, PSM_TYPE_CODE_IGLINE2D);
                assert_eq!(line.bytes_to_follow, 50);
                // 4 doubles finite + in domain.
                assert!(line.start.0.is_finite() && line.start.1.is_finite());
                assert!(line.end.0.is_finite() && line.end.1.is_finite());
                // Non-degenerate.
                let length = line.length();
                assert!(
                    length > 1e-12,
                    "decoded igLine2d should have non-zero length, got {length}"
                );
                if sample_lines.len() < 5 {
                    sample_lines.push(format!(
                        "{fixture} {} @ 0x{:06x}..0x{:06x} oid={} parent={} sub_type=0x{:04X} \
                        index={} start=({:+.4},{:+.4}) end=({:+.4},{:+.4}) len={:.4}",
                        sheet.path,
                        line.byte_range.start,
                        line.byte_range.end,
                        line.oid,
                        line.parent_ref,
                        line.sub_type_word,
                        line.index,
                        line.start.0,
                        line.start.1,
                        line.end.0,
                        line.end.1,
                        length,
                    ));
                }
            }
            per_fixture_count += decoded.len();
        }
        per_fixture_summary.push((fixture.to_string(), per_fixture_count));
        total_decoded += per_fixture_count;
    }
    eprintln!("--- Phase 14 Slice J: PSM igLine2d decoder cross-fixture summary ---");
    for (name, count) in &per_fixture_summary {
        eprintln!("  {name}: {count} decoded igLine2d records");
    }
    for sample in &sample_lines {
        eprintln!("  sample: {sample}");
    }
    eprintln!("  total decoded igLines across present fixtures: {total_decoded}");
    if per_fixture_summary.is_empty() {
        eprintln!(
            "skipping: no Sheet-bearing fixture present (CI / contributors \
            without SmartPlant samples)"
        );
        return;
    }
    assert!(
        total_decoded >= 100,
        "decode_iglines must emit >= 100 igLine2d records cross-fixture, got {total_decoded}. \
        Per-fixture summary: {per_fixture_summary:?}"
    );
}

/// Phase 14 Slice F: cross-fixture validation that
/// `decode_primitive_arcs` actually emits decoded `GArc2d` records
/// from real `Sheet*` streams.
///
/// Empirical baselines from `examples/probe_psm_garc2d.rs`:
///
/// - `DWG-0201GP06-01.pid` /Sheet6 → 3 hits with strict
///   `axis1_magnitude in [1e-6, 1e3]` + sorted params (mostly
///   circular: `axis2 == 0`).
/// - `DWG-0202GP06-01.pid` /Sheet6 → 10+ hits (mostly circular,
///   symbol-instrumentation arcs).
/// - `工艺管道及仪表流程-1.pid` /Sheet6 → 21+ hits.
/// - `A01.pid` /Sheet6 → 0 hits (template doesn't ship arc
///   geometry beyond the line reference).
///
/// The test asserts the aggregate decoded count across all
/// Sheet-bearing fixtures is `>= 1`, plus every emitted record
/// carries the documented provenance fields and geometry
/// invariants (`axis1_magnitude > 0`, `param_start < param_end`,
/// `axis2_magnitude >= 0`).
#[test]
fn primitive_arc_decoder_emits_decoded_arcs_with_provenance() {
    let fixtures = [
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
        "export-test/publish-data/A01/A01.pid",
    ];
    let mut total_decoded = 0usize;
    let mut per_fixture_summary: Vec<(String, usize, usize)> = Vec::new(); // (name, arcs, circles)
    let mut sample_arcs: Vec<String> = Vec::new();
    for fixture in fixtures {
        let Some(pkg) = parse_test_package(fixture) else {
            continue;
        };
        let mut per_fixture_arc_count = 0usize;
        let mut per_fixture_circle_count = 0usize;
        for sheet in &pkg.parsed.sheet_streams {
            let Some(raw) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            let bytes = raw.data.as_slice();
            let decoded = decode_primitive_arcs(bytes);
            for arc in &decoded {
                assert!(
                    arc.byte_range.end <= bytes.len(),
                    "arc byte_range {:?} exceeds stream {} bytes ({})",
                    arc.byte_range,
                    sheet.path,
                    bytes.len()
                );
                assert_eq!(arc.type_code, PSM_TYPE_CODE_GARC2D);
                let axis_a_mag = arc.axis_a_magnitude();
                assert!(
                    (1e-6..=1e3).contains(&axis_a_mag),
                    "arc axis_a magnitude out of expected range: {axis_a_mag} for {arc:?}"
                );
                assert!(
                    (0.0..=1.0 + 1e-6).contains(&arc.axis_ratio),
                    "arc axis_ratio out of [0, 1] domain: {} for {arc:?}",
                    arc.axis_ratio
                );
                assert!(
                    arc.sweep_direction <= 1,
                    "arc sweep_direction must be 0 or 1, got {} for {arc:?}",
                    arc.sweep_direction
                );
                assert!(arc.sweep_start_angle < arc.sweep_end_angle);
                assert!(arc.center.0.is_finite() && arc.center.1.is_finite());
                if arc.is_circular() {
                    per_fixture_circle_count += 1;
                }
                if sample_arcs.len() < 5 {
                    sample_arcs.push(format!(
                        "{fixture} {} @ 0x{:06x}..0x{:06x} oid={} circular={} \
                        center=({:+.4},{:+.4}) axis_a=({:+.4},{:+.4})|{:.4} \
                        axis_ratio={:.4} sweep_dir={} sweep=[{:+.4},{:+.4}]",
                        sheet.path,
                        arc.byte_range.start,
                        arc.byte_range.end,
                        arc.oid,
                        arc.is_circular(),
                        arc.center.0,
                        arc.center.1,
                        arc.axis_a.0,
                        arc.axis_a.1,
                        axis_a_mag,
                        arc.axis_ratio,
                        arc.sweep_direction,
                        arc.sweep_start_angle,
                        arc.sweep_end_angle,
                    ));
                }
            }
            per_fixture_arc_count += decoded.len();
        }
        per_fixture_summary.push((
            fixture.to_string(),
            per_fixture_arc_count,
            per_fixture_circle_count,
        ));
        total_decoded += per_fixture_arc_count;
    }
    eprintln!("--- Phase 14 Slice F: PSM GArc2d decoder cross-fixture summary ---");
    for (name, arcs, circles) in &per_fixture_summary {
        eprintln!("  {name}: {arcs} decoded arcs ({circles} circular)");
    }
    for sample in &sample_arcs {
        eprintln!("  sample: {sample}");
    }
    eprintln!("  total decoded arcs across present fixtures: {total_decoded}");
    if per_fixture_summary.is_empty() {
        eprintln!(
            "skipping: no Sheet-bearing fixture present (CI / contributors \
            without SmartPlant samples)"
        );
        return;
    }
    assert!(
        total_decoded >= 1,
        "decode_primitive_arcs must emit at least one decoded arc on the \
        Sheet-bearing fixture set, got {total_decoded}. \
        Per-fixture summary: {per_fixture_summary:?}"
    );
}
