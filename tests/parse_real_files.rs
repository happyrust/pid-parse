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
        collect_normalized_f64_pairs, coordinate_page_metadata_investigation_report,
        coordinate_pair_spatial_analysis, curve_primitive_investigation_report,
        decode_attribute_fragments, decode_dependency_objects, decode_igboundaries, decode_iglines,
        decode_iglinestrings, decode_igpoints, decode_igsymbols, decode_igtextboxes,
        decode_jstyle_overrides, decode_primitive_lines, decode_sub_records_0x0010,
        primitive_line_investigation_report, sheet_record_shape_inventory,
        symbol_placement_investigation_report, text_placement_investigation_report,
        SheetCoordinatePageMetadataCandidateKind, SheetCurvePrimitiveCandidateKind,
        SheetRecordShapeKind, SheetSymbolPlacementObject, DEPENDENCY_OBJECT_MIN_PAYLOAD_LEN,
        JSTYLE_OVERRIDE_MIN_BYTES_TO_FOLLOW, JSTYLE_OVERRIDE_PAYLOAD_LEN,
        PSM_TYPE_CODE_DEPENDENCY_OBJECT, PSM_TYPE_CODE_GLINE2D, PSM_TYPE_CODE_IGBOUNDARY2D,
        PSM_TYPE_CODE_IGLINE2D, PSM_TYPE_CODE_IGLINESTRING2D, PSM_TYPE_CODE_IGPOINT2D,
        PSM_TYPE_CODE_IGSYMBOL2D, PSM_TYPE_CODE_IGTEXTBOX, PSM_TYPE_CODE_JSTYLE_OVERRIDE,
        PSM_TYPE_CODE_SUB_RECORD_0X0010, SUB_RECORD_0X0010_MAX_BYTES_TO_FOLLOW,
        SUB_RECORD_0X0010_MIN_BYTES_TO_FOLLOW,
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
            path: "D06.pid",
            category: "compact_d06",
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

/// Whether an entity is raw Sheet-stream evidence rather than a decoded
/// record.
///
/// These carry the stream's own numbers -- i32 coordinate hints, object
/// hints, endpoint and text probes -- which are not in the page space the
/// drawing's border frame states. Nothing may promote their coordinate
/// context, whatever else the file proves about its page.
fn is_raw_sheet_evidence(entity: &pid_parse::PidGraphicEntity) -> bool {
    [
        ":text-probe:",
        ":coordinate-hint:",
        ":geometry-hint:",
        ":endpoint-line:",
        ":endpoint-probe:",
    ]
    .iter()
    .any(|marker| entity.id.contains(marker))
}

/// Raw Sheet evidence whose page transform was promoted anyway.
fn promoted_raw_sheet_evidence(geometry: &pid_parse::NormalizedPidGeometry) -> Vec<&str> {
    geometry
        .entities
        .iter()
        .filter(|entity| is_raw_sheet_evidence(entity))
        .filter(|entity| {
            matches!(
                entity.coordinate_context.page_transform,
                pid_parse::PidPageTransform::Available { .. }
            )
        })
        .map(|entity| entity.id.as_str())
        .collect()
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
            // Phase 16: `Annotation` entities are emitted by the
            // `decoded_jstyle_overrides` collection. They live in
            // `other_entities` until a dedicated Annotation bucket
            // ships (Phase 17 candidate).
            (pid_parse::PidGraphicKind::Annotation { .. }, _) => inventory.other_entities += 1,
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
fn d06_pid_parses_with_expected_structure_and_geometry_summary() {
    let Some(doc) = parse_test_file("D06.pid") else {
        return;
    };

    assert_eq!(doc.streams.len(), 56, "D06 stream inventory drifted");
    assert_eq!(doc.jsites.len(), 10, "D06 JSite count drifted");
    assert_eq!(
        doc.sheet_streams.len(),
        1,
        "D06 should expose exactly one Sheet stream"
    );

    let roots = doc.psm_roots.as_ref().expect("D06 PSMroots should decode");
    assert_eq!(roots.entries.len(), 7, "D06 PSMroots count drifted");

    let cluster_table = doc
        .psm_cluster_table
        .as_ref()
        .expect("D06 PSMclustertable should decode");
    assert_eq!(cluster_table.count, 5, "D06 declared cluster count drifted");
    assert_eq!(
        cluster_table.entries.len(),
        5,
        "D06 PSM cluster entries drifted"
    );
    assert_eq!(
        cluster_table.decoded_records.len(),
        5,
        "D06 conservative PSM cluster decoded records drifted"
    );

    let segment_table = doc
        .psm_segment_table
        .as_ref()
        .expect("D06 PSMsegmenttable should decode");
    assert_eq!(segment_table.count, 4, "D06 PSM segment count drifted");
    assert_eq!(
        segment_table.entries.len(),
        4,
        "D06 PSM segment entries drifted"
    );

    let doc_version2 = doc
        .doc_version2_decoded
        .as_ref()
        .expect("D06 DocVersion2 should decode");
    let doc_version3 = doc
        .version_history
        .as_ref()
        .expect("D06 DocVersion3 should decode");
    assert_eq!(
        doc_version2.records.len(),
        2,
        "D06 DocVersion2 count drifted"
    );
    assert_eq!(
        doc_version3.records.len(),
        2,
        "D06 DocVersion3 count drifted"
    );

    let app_object = doc
        .app_object_registry
        .as_ref()
        .expect("D06 AppObject registry should decode");
    assert_eq!(
        app_object.entries.len(),
        5,
        "D06 AppObject entry count drifted"
    );

    let tagged = doc
        .tagged_storages
        .as_ref()
        .expect("D06 tagged storage list should decode");
    assert_eq!(tagged.entries.len(), 1, "D06 tagged storage count drifted");

    let da = doc
        .dynamic_attributes
        .as_ref()
        .expect("D06 dynamic attributes should decode");
    assert_eq!(
        da.attribute_records.len(),
        47,
        "D06 DA record count drifted"
    );
    assert_eq!(da.record_trailers.len(), 25, "D06 DA trailer count drifted");
    assert_eq!(
        da.relationship_probes.len(),
        10,
        "D06 relationship probe count drifted"
    );

    let inventory = doc
        .object_inventory
        .as_ref()
        .expect("D06 object inventory should derive");
    assert_eq!(
        inventory.items.len(),
        23,
        "D06 object inventory item count drifted"
    );

    let graph = doc
        .object_graph
        .as_ref()
        .expect("D06 object graph should derive");
    assert_eq!(
        graph.objects.len(),
        10,
        "D06 object graph object count drifted"
    );
    assert_eq!(
        graph.relationships.len(),
        10,
        "D06 relationship attributes should be retained as unresolved graph relationships"
    );
    assert_eq!(
        graph.counts_by_type.get("Relationship").copied(),
        Some(10),
        "D06 relationship count should be reflected in counts_by_type"
    );
    assert!(
        graph.relationships.iter().all(|rel| {
            rel.record_id.is_none()
                && rel.field_x.is_none()
                && rel.source_drawing_id.is_none()
                && rel.target_drawing_id.is_none()
        }),
        "D06 attribute-only relationships should remain unresolved until Sheet endpoints provide a field_x link"
    );

    let sheet = doc
        .sheet_streams
        .first()
        .expect("D06 should expose /Sheet6");
    assert_eq!(sheet.path, "/Sheet6");
    let geometry = sheet
        .geometry
        .as_ref()
        .expect("D06 Sheet6 geometry expected");
    assert_eq!(
        geometry.texts.len(),
        8,
        "D06 Sheet6 text probe count drifted"
    );
    assert_eq!(
        geometry.coordinate_hints.len(),
        64,
        "D06 Sheet6 coordinate hint count drifted"
    );
    assert_eq!(
        geometry.decoded_primitive_lines.len(),
        0,
        "D06 should not gain GLine2d records without updating the baseline"
    );
    assert_eq!(
        geometry.decoded_iglines.len(),
        0,
        "D06 should not gain igLine2d records without updating the baseline"
    );
    assert_eq!(
        geometry.decoded_iglinestrings.len(),
        6,
        "D06 igLineString2d count drifted"
    );
    assert_eq!(
        geometry.decoded_igpoints.len(),
        10,
        "D06 igPoint2d count drifted"
    );
    assert_eq!(
        geometry.decoded_igtextboxes.len(),
        4,
        "D06 igTextBox count drifted"
    );
    // Was 2 while the decoder read the placement matrix from a fixed
    // payload offset: the other four records failed the finite/in-domain
    // check on the noise that offset produced. Anchoring on the matrix
    // tag recovers all six.
    assert_eq!(
        geometry.decoded_igsymbols.len(),
        6,
        "D06 igSymbol2d count drifted"
    );
    assert_eq!(
        geometry.decoded_dependency_objects.len(),
        21,
        "D06 DependencyObject audit count drifted"
    );
    assert_eq!(
        geometry.decoded_jstyle_overrides.len(),
        3,
        "D06 JStyleOverride count drifted"
    );
    assert_eq!(
        geometry.decoded_sub_records_0x0010.len(),
        20,
        "D06 0x0010 audit count drifted"
    );

    let normalized = normalized_geometry_inventory(&doc);
    assert_eq!(
        normalized.total(),
        101,
        "D06 normalized geometry total drifted"
    );
    assert_eq!(normalized.decoded_lines, 0);
    assert_eq!(normalized.decoded_polylines, 6);
    assert_eq!(normalized.decoded_points, 10);
    assert_eq!(normalized.decoded_texts, 4);
    assert_eq!(normalized.decoded_symbols, 6);
    // D06's 3 JStyleOverride records used to surface here as inferred
    // annotations. style.dll's own reader shows the bytes that anchor was
    // built from are four u32, not two f64, so they now surface as probe-only
    // unknowns instead -- hence 0 annotations and 8 + 3 = 11 unknowns. See
    // `docs/analysis/2026-08-04-jstyleoverride-native-reader-settles-it.md`.
    assert_eq!(
        normalized.other_entities, 0,
        "D06 decoded annotations count drifted"
    );
    assert_eq!(normalized.inferred_points, 64);
    assert_eq!(normalized.inferred_lines, 0);
    assert_eq!(normalized.probe_only_unknowns, 11);
}

#[test]
fn d06_text_placement_regression_keeps_text_probes_unpromoted() {
    let Some(pkg) = parse_test_package("D06.pid") else {
        return;
    };
    let raw_sheet = pkg
        .streams
        .get("/Sheet6")
        .expect("D06 should expose /Sheet6 when the fixture is present");
    let report = probe_sheet_stream(
        "Sheet6",
        "/Sheet6",
        &raw_sheet.data,
        &SheetProbeOptions::default(),
    );
    let decoded_textboxes = decode_igtextboxes(&raw_sheet.data);
    let text_candidates = sheet_text_window_candidates(
        &report.text_runs,
        &report.coordinate_hints,
        &report.chunks,
        128,
    );
    let scores = score_sheet_text_window_candidates(&text_candidates);
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
    let normalized = pid_parse::build_normalized_geometry(&pkg.parsed);
    let text_probe_ids = normalized
        .entities
        .iter()
        .filter(|entity| {
            entity.source.stream_path.as_deref() == Some("/Sheet6")
                && entity.confidence == pid_parse::PidGeometryConfidence::ProbeOnly
                && matches!(entity.kind, pid_parse::PidGraphicKind::Unknown { .. })
        })
        .filter_map(|entity| entity.source.record_id.clone())
        .filter(|record_id| record_id.starts_with("text-probe:"))
        .collect::<BTreeSet<_>>();
    let expected_text_probe_ids = (0..8)
        .map(|index| format!("text-probe:{index}"))
        .collect::<BTreeSet<_>>();
    let inferred_text_count = normalized
        .entities
        .iter()
        .filter(|entity| {
            matches!(entity.kind, pid_parse::PidGraphicKind::Text { .. })
                && entity.confidence != pid_parse::PidGeometryConfidence::Decoded
        })
        .count();

    assert_eq!(
        report.text_runs.len(),
        8,
        "D06 raw text probe count drifted"
    );
    assert_eq!(
        decoded_textboxes.len(),
        4,
        "D06 decoded igTextBox count drifted"
    );
    assert_eq!(
        text_placement_report.raw_candidate_count,
        scores.len(),
        "D06 text placement report should account for every text-window score"
    );
    assert_eq!(
        text_placement_report.rejected_candidate_count,
        scores
            .len()
            .saturating_sub(text_placement_report.candidates.len()),
        "D06 text placement report should expose rejected binary-like candidates"
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
        "D06 text placement candidates should remain investigation-only evidence"
    );
    assert_eq!(
        text_probe_ids, expected_text_probe_ids,
        "D06 text probe identities drifted"
    );
    assert_eq!(
        inferred_text_count, 0,
        "D06 text probes must not be promoted to inferred Text geometry"
    );
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

/// The space map is read the way `radsrvitem.dll` reads it, and the two
/// things that prove the frame are the reader's own rules rather than
/// anything this crate chose:
///
/// * nothing states how long the entry region is, so the walk has to land
///   exactly on the end of the stream;
/// * `Segment::Load` files each entry by `persist_id & 0x1FFF` and refuses a
///   second entry on an index it has already filled, so indices must be under
///   `0x2000` and must not repeat.
///
/// A frame off by two bytes fails both at once. The header's own entry count
/// is a third, independent check: the walk never consults it.
#[test]
fn psm_space_map_walks_every_segment_to_its_last_byte() {
    let mut checked = 0usize;
    for fixture in [
        "D06.pid",
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
    ] {
        let Some(doc) = parse_test_file(fixture) else {
            continue;
        };
        assert!(
            !doc.psm_space_maps.is_empty(),
            "{fixture} should carry at least one PSMspacemap member"
        );
        for (path, map) in &doc.psm_space_maps {
            checked += 1;
            assert!(!map.legacy, "{fixture} {path}: expected the 'tseg' form");
            assert_eq!(
                map.trailing_bytes, 0,
                "{fixture} {path}: the walk must reach the last byte"
            );
            assert_eq!(
                map.entries.len(),
                usize::from(map.stated_entry_count),
                "{fixture} {path}: walked entries must match the header's count"
            );
            let mut seen = std::collections::BTreeSet::new();
            for entry in &map.entries {
                assert!(
                    entry.index < 0x2000,
                    "{fixture} {path}: index {} does not fit 13 bits",
                    entry.index
                );
                assert!(
                    seen.insert(entry.index),
                    "{fixture} {path}: index {} is claimed twice",
                    entry.index
                );
                assert!(
                    entry.form == 2 || entry.form == 3,
                    "{fixture} {path}: head form {} is one the vendor reader would refuse",
                    entry.form
                );
            }
        }
    }
    assert!(
        checked >= 38,
        "expected the four sheet fixtures to contribute 38 space-map members, saw {checked}"
    );
}

/// The `u16` in front of the member count is a fill level, not a copy of it:
/// the member array is addressed by position, the surplus slots are all-zero,
/// and they sit at the tail.
///
/// Three things have to hold at once for that reading to be the right one, and
/// all three are exact across the corpus -- an off-by-one in either `u16`, or a
/// member array that is compacted on delete rather than padded, breaks one of
/// them immediately.
#[test]
fn psm_space_map_states_how_many_member_slots_are_in_use() {
    let mut entries = 0usize;
    let mut slots = 0usize;
    let mut empty = 0usize;
    for fixture in [
        "D06.pid",
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
    ] {
        let Some(doc) = parse_test_file(fixture) else {
            continue;
        };
        for (path, map) in &doc.psm_space_maps {
            for entry in &map.entries {
                entries += 1;
                slots += entry.members.len();
                let at = format!("{fixture} {path} entry {}", entry.index);

                // An empty slot is empty in both halves; there is no member
                // that carries a tag without a value or the other way round.
                for member in &entry.members {
                    assert_eq!(
                        member.tag == 0,
                        member.value == 0,
                        "{at}: {member:?} is half empty"
                    );
                }

                let live = entry
                    .members
                    .iter()
                    .filter(|member| member.tag != 0)
                    .count();
                empty += entry.members.len() - live;
                assert_eq!(
                    usize::from(entry.live_member_count),
                    live,
                    "{at}: states {} slots in use against {live} non-empty of {}",
                    entry.live_member_count,
                    entry.members.len()
                );

                // The used slots come first, so the stated count names a
                // prefix rather than a subset.
                assert!(
                    entry.live_members().iter().all(|member| member.tag != 0),
                    "{at}: an empty slot sits in front of a used one"
                );
            }
        }
    }
    if entries == 0 {
        return;
    }
    assert_eq!(
        (entries, slots, empty),
        (1948, 4446, 457),
        "the four sheet fixtures should hold 1948 entries / 4446 slots / 457 empty"
    );
}

/// `tag` is a property of the object a member's `value` names -- the
/// **referrer**, since a member is an incoming reference -- not of the edge.
///
/// The agreement half of that needs nothing outside the map: collect every
/// object the corpus records in a member slot, and ask whether the members
/// naming it agree on a tag. Agreement means the tag says what that object
/// *is*; disagreement would mean it says what the edge is *for*. With 675 of
/// the 2415 member objects appearing on more than one entry and 13 tags to
/// choose from, an edge-assigned tag could not come out unanimous.
///
/// (The direction half is settled elsewhere: the record whose oid is a
/// member's `value` carries the entry's own id in its payload at a
/// family-fixed offset, so the member's object is the one holding the
/// reference. See `probe_psmspacemap_tag181_is_the_parent_ref` and
/// `docs/analysis/2026-08-27-the-spacemap-is-an-incoming-reference-index.md`;
/// this test only ratchets the tag-per-object agreement.)
///
/// Ids are scoped by the storage that issued them -- the top-level map and each
/// `JSite` registry number their objects independently -- so the grouping is by
/// container, not by document.
#[test]
fn psm_space_map_every_referrer_carries_one_tag() {
    let mut referrers = 0usize;
    let mut on_multiple_entries = 0usize;
    for fixture in [
        "D06.pid",
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
    ] {
        let Some(doc) = parse_test_file(fixture) else {
            continue;
        };
        // container path -> referrer persist id -> (tag, entries it appears on)
        let mut by_container: std::collections::BTreeMap<
            &str,
            std::collections::BTreeMap<u32, (u16, usize)>,
        > = std::collections::BTreeMap::new();
        for (path, map) in &doc.psm_space_maps {
            let container = match path.rfind("PSMspacemap") {
                Some(at) => &path[..at],
                None => path.as_str(),
            };
            let seen = by_container.entry(container).or_default();
            for entry in &map.entries {
                for member in entry.live_members() {
                    let (tag, hits) = seen.entry(member.value).or_insert((member.tag, 0));
                    assert_eq!(
                        *tag, member.tag,
                        "{fixture} {container}: object {} is recorded as class {} and as {}",
                        member.value, tag, member.tag
                    );
                    *hits += 1;
                }
            }
        }
        for hits in by_container.values().flat_map(|seen| seen.values()) {
            referrers += 1;
            if hits.1 > 1 {
                on_multiple_entries += 1;
            }
        }
    }
    if referrers == 0 {
        return;
    }
    assert_eq!(
        (referrers, on_multiple_entries),
        (2415, 675),
        "the four sheet fixtures should record 2415 referrers, 675 of them on more than one entry"
    );
}

/// A space-map member is an *incoming* edge: `value` names the object that
/// references the entry, and that referrer's own record carries the entry's
/// persist id in its payload -- while the entry's own record does not carry
/// the member's value. This is the direction the 2026-08-27 join settled
/// (`probe_psmspacemap_tag181_is_the_parent_ref`), ratcheted here on the tags
/// whose referrer record family pins the back-reference at a fixed offset.
///
/// For each such tag the two counts that must not drift are the number of
/// members, and how many of them have the entry id somewhere in the referrer
/// record. `mentioned == with_record` on every one of them: whenever the
/// referrer record exists, it names the entry back. Tag 249 is the one place
/// `total != with_record` -- the single stale edge in `DWG-0201GP06-01.pid`,
/// a member pointing at a freed `0x00FA` id whose record is gone.
#[test]
fn psm_space_map_members_are_incoming_edges() {
    use std::collections::BTreeMap;
    use std::io::Read;

    const CHAIN_MAGIC: u32 = 0x6C90_F544;
    const SEGMENT_SHIFT: u32 = 13;

    fn u32_at(data: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
    }

    // Canonical storage: "" for the top level, "JSiteN" for a registry. Both
    // a member stream path and a record-chain stream path reduce to it, so an
    // id only resolves against records in its own storage.
    fn storage_of(path: &str) -> String {
        let norm = path.replace('\\', "/");
        let norm = norm.strip_prefix('/').unwrap_or(&norm);
        if let Some(at) = norm.find("PSMspacemap") {
            norm[..at].trim_end_matches('/').to_string()
        } else if let Some((pre, _)) = norm.rsplit_once('/') {
            pre.to_string()
        } else {
            String::new()
        }
    }
    fn segment_of(path: &str) -> Option<u32> {
        let leaf = path.rsplit(['/', '\\']).next()?;
        u32::from_str_radix(leaf.strip_prefix("0x")?, 16)
            .ok()
            .map(|address| address >> SEGMENT_SHIFT)
    }

    // The tags whose referrer family fixes the back-reference at one offset:
    // (total members, members whose value resolves to a record) across the
    // four sheet fixtures. `mentioned` must equal the second on every tag.
    let expected: BTreeMap<u16, (usize, usize)> = BTreeMap::from([
        (181, (84, 84)),
        (183, (357, 357)),
        (185, (279, 279)),
        (190, (1446, 1446)),
        (201, (72, 72)),
        (205, (8, 8)),
        (249, (731, 730)),
    ]);

    let mut total: BTreeMap<u16, usize> = BTreeMap::new();
    let mut with_record: BTreeMap<u16, usize> = BTreeMap::new();
    let mut mentioned: BTreeMap<u16, usize> = BTreeMap::new();
    // The forward reading, for the cleanest tag: a member's value is almost
    // never in the entry's *own* record. Locks the direction, not just the
    // fact of a link.
    let mut tag181_members = 0usize;
    let mut tag181_value_in_own_record = 0usize;
    let mut any = false;

    for fixture in [
        "D06.pid",
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
    ] {
        let path = format!("test-file/{fixture}");
        if !std::path::Path::new(&path).exists() {
            continue;
        }
        let Some(doc) = parse_test_file(fixture) else {
            continue;
        };

        // Every record-chain stream, walked, grouped storage -> oid -> payloads.
        let mut records: BTreeMap<String, BTreeMap<u32, Vec<Vec<u8>>>> = BTreeMap::new();
        let file = std::fs::File::open(&path).expect("fixture opens");
        let mut cfb = cfb::CompoundFile::open(file).expect("fixture is a compound file");
        let stream_paths: Vec<String> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|entry| entry.path().to_string_lossy().into_owned())
            .collect();
        for stream_path in stream_paths {
            let mut data = Vec::new();
            let Ok(mut stream) = cfb.open_stream(&stream_path) else {
                continue;
            };
            if stream.read_to_end(&mut data).is_err() || u32_at(&data, 0) != Some(CHAIN_MAGIC) {
                continue;
            }
            let starts = pid_parse::parsers::sheet_records::sheet_record_starts(&data);
            let storage = records.entry(storage_of(&stream_path)).or_default();
            for at in starts {
                let Some(len) = u32_at(&data, at + 2) else {
                    continue;
                };
                let Some(payload) = data.get(at + 6..at + 6 + len as usize) else {
                    continue;
                };
                let Some(oid) = u32_at(payload, 0) else {
                    continue;
                };
                storage.entry(oid).or_default().push(payload.to_vec());
            }
        }

        let payload_has = |payloads: &[Vec<u8>], needle: u32| -> bool {
            payloads.iter().any(|payload| {
                (0..payload.len().saturating_sub(3)).any(|at| u32_at(payload, at) == Some(needle))
            })
        };

        for (map_path, map) in &doc.psm_space_maps {
            let Some(segment) = segment_of(map_path) else {
                continue;
            };
            let storage = records.get(&storage_of(map_path));
            for entry in &map.entries {
                let id = (segment << SEGMENT_SHIFT) | u32::from(entry.index);
                let own = storage.and_then(|by_oid| by_oid.get(&id));
                for member in entry.live_members() {
                    if member.tag == 181 {
                        tag181_members += 1;
                        if own.is_some_and(|payloads| payload_has(payloads, member.value)) {
                            tag181_value_in_own_record += 1;
                        }
                    }
                    if !expected.contains_key(&member.tag) {
                        continue;
                    }
                    any = true;
                    *total.entry(member.tag).or_default() += 1;
                    let Some(referrer) = storage.and_then(|by_oid| by_oid.get(&member.value))
                    else {
                        continue;
                    };
                    *with_record.entry(member.tag).or_default() += 1;
                    if payload_has(referrer, id) {
                        *mentioned.entry(member.tag).or_default() += 1;
                    }
                }
            }
        }
    }

    if !any {
        return;
    }

    for (tag, (want_total, want_with_record)) in &expected {
        let got_total = total.get(tag).copied().unwrap_or_default();
        let got_with_record = with_record.get(tag).copied().unwrap_or_default();
        let got_mentioned = mentioned.get(tag).copied().unwrap_or_default();
        assert_eq!(
            (got_total, got_with_record),
            (*want_total, *want_with_record),
            "tag {tag}: expected {want_total} members / {want_with_record} with a referrer record"
        );
        assert_eq!(
            got_mentioned, got_with_record,
            "tag {tag}: {got_with_record} referrer records exist but only {got_mentioned} name \
             the entry back -- the edge should point from the member to the entry"
        );
    }

    assert_eq!(
        (tag181_members, tag181_value_in_own_record),
        (84, 0),
        "tag 181 is the reciprocal pair: the member value belongs to the referrer's record, \
         never to the entry's own"
    );
}

/// Tag 184 is the sheet / layer / view hierarchy, and its referrers do state
/// their own membership -- by name.
///
/// Every family the tag touches is named by the RAD class registry:
/// `0x0057` / `0x0060` `Top ViewFilterSet` (`viewfil.dex`), `0x0076`
/// `SheetView`, `0x0114` `JSheet`, `0x0042` `JSheetLayerManager`, `0x0081`
/// `JSheetLayer`. The shape is fixed: one `0x0060` per storage holding a
/// `SheetView` and its `0x0057` sets, and each `0x0057` holding one `JSheet`,
/// one `JSheetLayerManager` and its layers.
///
/// The claim worth ratcheting is the one that corrected the earlier reading of
/// these edges as living only in the space map: a set writes the **name** of
/// every layer it points at into its own payload as UTF-16LE, so the table
/// resolves a name to an object rather than storing the relation outright.
/// The two id fields that are written as ids are locked too -- the `JSheet`
/// at a set's `+16`, and the number of sets a `0x0060` holds at its `+12`.
///
/// See `docs/analysis/2026-08-27-tag-184-is-the-sheet-layer-view-hierarchy.md`
/// and `examples/probe_psmspacemap_tag184_viewfilterset_edges.rs`.
#[test]
fn psm_space_map_184_edges_are_the_view_filter_sets_named_layers() {
    use std::collections::BTreeMap;
    use std::io::Read;

    const CHAIN_MAGIC: u32 = 0x6C90_F544;
    const SEGMENT_SHIFT: u32 = 13;
    const VF_TOP: u16 = 0x0060;
    const VF_SET: u16 = 0x0057;
    const SHEET_VIEW: u16 = 0x0076;
    const JSHEET: u16 = 0x0114;
    const LAYER_MANAGER: u16 = 0x0042;
    const LAYER: u16 = 0x0081;

    fn u32_at(data: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
    }
    fn storage_of(path: &str) -> String {
        let norm = path.replace('\\', "/");
        let norm = norm.strip_prefix('/').unwrap_or(&norm);
        if let Some(at) = norm.find("PSMspacemap") {
            norm[..at].trim_end_matches('/').to_string()
        } else if let Some((pre, _)) = norm.rsplit_once('/') {
            pre.to_string()
        } else {
            String::new()
        }
    }
    fn segment_of(path: &str) -> Option<u32> {
        let leaf = path.rsplit(['/', '\\']).next()?;
        u32::from_str_radix(leaf.strip_prefix("0x")?, 16)
            .ok()
            .map(|address| address >> SEGMENT_SHIFT)
    }
    /// A `JSheetLayer` states its name as `u32` char count at `+20` and
    /// UTF-16LE characters from `+24`; return those raw bytes.
    fn layer_name_bytes(payload: &[u8]) -> Option<&[u8]> {
        let count = u32_at(payload, 20)? as usize;
        if count == 0 || count > 128 {
            return None;
        }
        payload.get(24..24 + count * 2)
    }

    let mut edges = 0usize;
    let mut referrers: BTreeMap<u16, usize> = BTreeMap::new();
    let mut targets: BTreeMap<u16, usize> = BTreeMap::new();
    let mut layer_edges = (0usize, 0usize);
    let mut jsheet_at_16 = (0usize, 0usize);
    let mut sets_at_12 = (0usize, 0usize);
    let mut layers_listed_once = (0usize, 0usize);
    let mut any = false;

    for fixture in [
        "D06.pid",
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
    ] {
        let path = format!("test-file/{fixture}");
        if !std::path::Path::new(&path).exists() {
            continue;
        }
        let Some(doc) = parse_test_file(fixture) else {
            continue;
        };

        let mut records: BTreeMap<String, BTreeMap<u32, (u16, Vec<u8>)>> = BTreeMap::new();
        let file = std::fs::File::open(&path).expect("fixture opens");
        let mut cfb = cfb::CompoundFile::open(file).expect("fixture is a compound file");
        let stream_paths: Vec<String> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|entry| entry.path().to_string_lossy().into_owned())
            .collect();
        for stream_path in stream_paths {
            let mut data = Vec::new();
            let Ok(mut stream) = cfb.open_stream(&stream_path) else {
                continue;
            };
            if stream.read_to_end(&mut data).is_err() || u32_at(&data, 0) != Some(CHAIN_MAGIC) {
                continue;
            }
            let starts = pid_parse::parsers::sheet_records::sheet_record_starts(&data);
            let storage = records.entry(storage_of(&stream_path)).or_default();
            for at in starts {
                let Some(type_code) = u32_at(&data, at).map(|word| (word & 0x3FFF) as u16) else {
                    continue;
                };
                let Some(len) = u32_at(&data, at + 2) else {
                    continue;
                };
                let Some(payload) = data.get(at + 6..at + 6 + len as usize) else {
                    continue;
                };
                let Some(oid) = u32_at(payload, 0) else {
                    continue;
                };
                storage.entry(oid).or_insert((type_code, payload.to_vec()));
            }
        }

        // Entry id -> its members, so a referrer's whole edge set is reachable.
        let mut members_by_storage: BTreeMap<String, Vec<(u32, u32, u16)>> = BTreeMap::new();
        for (map_path, map) in &doc.psm_space_maps {
            let Some(segment) = segment_of(map_path) else {
                continue;
            };
            let bucket = members_by_storage.entry(storage_of(map_path)).or_default();
            for entry in &map.entries {
                let id = (segment << SEGMENT_SHIFT) | u32::from(entry.index);
                for member in entry.live_members() {
                    bucket.push((id, member.value, member.tag));
                }
            }
        }

        for (storage_name, bucket) in &members_by_storage {
            let Some(by_oid) = records.get(storage_name) else {
                continue;
            };
            let family_of = |oid: u32| by_oid.get(&oid).map(|(family, _)| *family);

            // Tag 183: every JSheetLayer is listed by exactly one manager.
            for (oid, (family, _)) in by_oid {
                if *family != LAYER {
                    continue;
                }
                layers_listed_once.0 += 1;
                let listings = bucket
                    .iter()
                    .filter(|(id, _, tag)| id == oid && *tag == 183)
                    .count();
                if listings == 1 {
                    layers_listed_once.1 += 1;
                }
            }

            let mut held_sets: BTreeMap<u32, usize> = BTreeMap::new();
            for (target, referrer, tag) in bucket {
                if *tag != 184 {
                    continue;
                }
                any = true;
                edges += 1;
                let Some(referrer_family) = family_of(*referrer) else {
                    continue;
                };
                *referrers.entry(referrer_family).or_default() += 1;
                let Some(target_family) = family_of(*target) else {
                    continue;
                };
                *targets.entry(target_family).or_default() += 1;
                let Some((_, referrer_payload)) = by_oid.get(referrer) else {
                    continue;
                };
                match target_family {
                    LAYER => {
                        layer_edges.0 += 1;
                        let named = by_oid
                            .get(target)
                            .and_then(|(_, payload)| layer_name_bytes(payload))
                            .is_some_and(|name| {
                                referrer_payload
                                    .windows(name.len())
                                    .any(|window| window == name)
                            });
                        if named {
                            layer_edges.1 += 1;
                        }
                    }
                    JSHEET => {
                        jsheet_at_16.0 += 1;
                        if u32_at(referrer_payload, 16) == Some(*target) {
                            jsheet_at_16.1 += 1;
                        }
                    }
                    VF_SET => *held_sets.entry(*referrer).or_default() += 1,
                    _ => {}
                }
            }
            for (top, held) in &held_sets {
                let Some((family, payload)) = by_oid.get(top) else {
                    continue;
                };
                if *family != VF_TOP {
                    continue;
                }
                sets_at_12.0 += 1;
                if u32_at(payload, 12) == Some(*held as u32) {
                    sets_at_12.1 += 1;
                }
            }
        }
    }

    if !any {
        return;
    }

    assert_eq!(edges, 366, "tag-184 member count across the four fixtures");
    assert_eq!(
        referrers,
        BTreeMap::from([(VF_SET, 306), (VF_TOP, 60)]),
        "every tag-184 referrer is one of the two Top ViewFilterSet families"
    );
    assert_eq!(
        targets,
        BTreeMap::from([
            (LAYER_MANAGER, 49),
            (SHEET_VIEW, 11),
            (VF_SET, 49),
            (LAYER, 208),
            (JSHEET, 49),
        ]),
        "the 184 subgraph is the sheet / layer / view hierarchy and nothing else"
    );
    assert_eq!(
        layer_edges.1, layer_edges.0,
        "{} of {} JSheetLayer edges have the layer's own name in the referring set's payload -- \
         the membership is written by name, not by id",
        layer_edges.1, layer_edges.0
    );
    assert_eq!(layer_edges.0, 208, "JSheetLayer edge count");
    assert_eq!(
        jsheet_at_16,
        (49, 49),
        "a view filter set writes its JSheet as an id, at payload +16"
    );
    assert_eq!(
        sets_at_12,
        (11, 11),
        "a 0x0060 states at +12 how many sets it holds -- it has no room for their ids"
    );
    assert_eq!(
        layers_listed_once,
        (290, 290),
        "every JSheetLayer is listed by exactly one JSheetLayerManager (tag 183)"
    );
}

/// The layer evidence is part of the decoded document rather than remaining
/// trapped in probes: all four primary fixtures expose every storage-local
/// `JSheetLayer`, and tag 183 reconciles each one to exactly one manager.
#[test]
fn jsheet_layers_are_decoded_per_storage_and_registered_once() {
    let expected = [
        ("D06.pid", 50usize),
        ("DWG-0201GP06-01.pid", 110),
        ("DWG-0202GP06-01.pid", 71),
        ("工艺管道及仪表流程-1.pid", 59),
    ];
    let mut total = 0usize;
    for (fixture, expected_layers) in expected {
        let Some(doc) = parse_test_file(fixture) else {
            continue;
        };
        let layers: Vec<_> = doc.sheet_layers.values().flatten().collect();
        assert_eq!(layers.len(), expected_layers, "{fixture}: layer count");
        assert!(
            layers.iter().all(|layer| !layer.name.is_empty()),
            "{fixture}: every layer has its authored name"
        );
        assert!(
            layers.iter().all(|layer| {
                layer.manager_oid.is_some() && layer.manager_registration_count == 1
            }),
            "{fixture}: every layer is registered by exactly one manager"
        );
        assert!(
            layers
                .iter()
                .all(|layer| layer.storage_path == "/" || layer.storage_path.starts_with("/JSite")),
            "{fixture}: storage identity stays explicit"
        );
        total += layers.len();
    }
    if total != 0 {
        assert_eq!(total, 290, "four-fixture JSheetLayer total");
    }
}

/// The layer display state is in the file, and the parser reads it (plan
/// 2026-09-07, L1): every `0x0057 Top ViewFilterSet` closes exactly under one
/// layout, names its sheet, and states two bitmaps over layer numbers -- the
/// first the display state -- plus the `(name, number)` of every layer it
/// governs. Each entry resolves through the sheet's `JSheetLayerManager`
/// (tag 183) to exactly one `JSheetLayer`, so every layer of every storage
/// gets the file's own answer to "is this shown".
///
/// Pinned here: the count, the exact close (decoded == raw), the resolution,
/// and the readings the importer's name criterion is measured against --
/// `Hidden` / `HiddenObjects` off on every top-level sheet, `Default` on
/// everywhere, `Dimension` / `Construction` off in every symbol definition,
/// `Invisible` **on** in the two definitions that have it. Payload `+32`,
/// unread since 08-27, equals `Default`'s layer number on every record.
///
/// See `docs/analysis/2026-09-14-viewfilterset-carries-the-layer-display-state.md`
/// and `examples/probe_viewfilterset_display_state.rs`.
#[test]
fn view_filter_sets_state_each_sheets_layer_display_and_close_exactly() {
    use std::io::Read;

    const CHAIN_MAGIC: u32 = 0x6C90_F544;
    const VF_SET: u16 = 0x0057;

    fn u32_at(data: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
    }

    /// Raw `0x0057` records per fixture, so a set that refused the layout
    /// shows up as a count that does not match.
    fn raw_set_count(name: &str) -> usize {
        let path = format!("test-file/{name}");
        let Ok(file) = std::fs::File::open(&path) else {
            return 0;
        };
        let Ok(mut cfb) = cfb::CompoundFile::open(file) else {
            return 0;
        };
        let paths: Vec<String> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|entry| entry.path().to_string_lossy().replace('\\', "/"))
            .collect();
        let mut count = 0usize;
        for stream_path in paths {
            let Ok(mut stream) = cfb.open_stream(&stream_path) else {
                continue;
            };
            let mut data = Vec::new();
            if stream.read_to_end(&mut data).is_err() || u32_at(&data, 0) != Some(CHAIN_MAGIC) {
                continue;
            }
            for at in pid_parse::parsers::sheet_records::sheet_record_starts(&data) {
                if data
                    .get(at..at + 2)
                    .map(|w| u16::from_le_bytes([w[0], w[1]]) & 0x3FFF)
                    == Some(VF_SET)
                {
                    count += 1;
                }
            }
        }
        count
    }

    let expected = [
        ("D06.pid", 8usize),
        ("DWG-0201GP06-01.pid", 20),
        ("DWG-0202GP06-01.pid", 12),
        ("工艺管道及仪表流程-1.pid", 9),
    ];
    let mut total_sets = 0usize;
    let mut total_entries = 0usize;
    let mut invisible_in_definitions = 0usize;
    for (fixture, expected_sets) in expected {
        let Some(doc) = parse_test_file(fixture) else {
            continue;
        };
        let sets: Vec<_> = doc.view_filter_sets.values().flatten().collect();
        assert_eq!(
            sets.len(),
            expected_sets,
            "{fixture}: view filter set count"
        );
        assert_eq!(
            sets.len(),
            raw_set_count(fixture),
            "{fixture}: a 0x0057 record refused the layout"
        );
        total_sets += sets.len();

        for set in &sets {
            assert!(
                !set.layers.is_empty(),
                "{fixture}: set {} names no layer",
                set.oid
            );
            let default = set
                .layers
                .iter()
                .find(|layer| layer.name == "Default")
                .unwrap_or_else(|| panic!("{fixture}: set {} has no Default", set.oid));
            assert_eq!(
                set.active_layer_number,
                u32::from(default.layer_number),
                "{fixture}: set {}: +32 is Default's layer number",
                set.oid
            );
            for layer in &set.layers {
                total_entries += 1;
                assert!(
                    layer.layer_oid.is_some(),
                    "{fixture}: set {} entry {:?} #{} resolves to no single JSheetLayer",
                    set.oid,
                    layer.name,
                    layer.layer_number
                );
                assert!(
                    layer.displayed.is_some() && layer.locatable.is_some(),
                    "{fixture}: set {} entry {:?} #{} is past its bitmaps",
                    set.oid,
                    layer.name,
                    layer.layer_number
                );
                let top = set.storage_path == "/";
                match layer.name.as_str() {
                    "Default" => assert_eq!(layer.displayed, Some(true), "{fixture}: Default"),
                    "Hidden" | "HiddenObjects" if top => assert_eq!(
                        layer.displayed,
                        Some(false),
                        "{fixture}: {} on the sheet",
                        layer.name
                    ),
                    "Dimension" | "Construction" => assert_eq!(
                        layer.displayed,
                        Some(false),
                        "{fixture}: {} in a definition (set {})",
                        layer.name,
                        set.oid
                    ),
                    "Invisible" if !top => {
                        invisible_in_definitions += 1;
                        assert_eq!(
                            layer.displayed,
                            Some(true),
                            "{fixture}: Invisible in a definition (set {})",
                            set.oid
                        );
                    }
                    _ => {}
                }
            }
        }

        // The sheet itself: one set, on JSheet 6, and every layer object of
        // the root storage now carries the file's answer.
        let top: Vec<_> = sets.iter().filter(|set| set.storage_path == "/").collect();
        assert_eq!(top.len(), 1, "{fixture}: one view filter set on the sheet");
        assert_eq!(top[0].sheet_ref, 6, "{fixture}: the sheet's JSheet");
        let root_layers = doc.sheet_layers.get("/").expect("root layers");
        assert!(
            root_layers.iter().all(|layer| layer.displayed.is_some()),
            "{fixture}: a root layer without a display state"
        );
        assert!(
            root_layers
                .iter()
                .all(|layer| layer.view_filter_set_oid == Some(top[0].oid)),
            "{fixture}: every root layer is governed by the sheet's set"
        );
        for (name, shown) in [
            ("Default", true),
            ("Labels", true),
            ("ConsistencyChecks", true),
            ("DrawingBorder", true),
            ("Hidden", false),
            ("HiddenObjects", false),
            ("Label", false),
        ] {
            let layer = root_layers
                .iter()
                .find(|layer| layer.name == name)
                .unwrap_or_else(|| panic!("{fixture}: no root layer {name}"));
            assert_eq!(
                layer.displayed,
                Some(shown),
                "{fixture}: {name} on the sheet"
            );
        }

        // And it reaches the geometry the importer consumes.
        let geometry = pid_parse::build_normalized_geometry(&doc);
        let mut hidden_entities = 0usize;
        for entity in &geometry.entities {
            let Some(layer) = &entity.source_layer else {
                continue;
            };
            let Some(name) = layer.name.as_deref() else {
                continue;
            };
            if layer.storage_path != "/" {
                continue;
            }
            match name {
                "HiddenObjects" | "Hidden" => {
                    assert_eq!(layer.displayed, Some(false), "{fixture}: {name} entity");
                    hidden_entities += 1;
                }
                "Default" | "Labels" | "ConsistencyChecks" => {
                    assert_eq!(layer.displayed, Some(true), "{fixture}: {name} entity");
                }
                _ => {}
            }
        }
        assert!(
            hidden_entities > 0,
            "{fixture}: the sheet has content on a switched-off layer"
        );
    }
    if total_sets != 0 {
        assert_eq!(total_sets, 49, "four-fixture view filter set total");
        assert_eq!(total_entries, 283, "four-fixture layer entry total");
        assert_eq!(
            invisible_in_definitions, 2,
            "the two definitions naming Invisible display it"
        );
    }
}

/// `aux_hi` -- payload `+8`, the high half of the PSM envelope's 8-byte `aux`
/// -- is the sheet layer the object sits on.
///
/// `shlyhp.dll` gave the direction: `AddObjectToSheetLayer` hands the layer to
/// the graphic and bumps a counter in the layer, so the layer keeps a tally and
/// the graphic keeps the reference. The tally is the layer's own `+12`, and it
/// is what makes this measurable without guessing: the layers of a storage
/// declare a multiset of counts, and grouping the storage's objects by `+8`
/// has to reproduce it exactly -- every layer, zeros included. Nothing else in
/// the first 256 bytes at either width does, in any storage.
///
/// The same word is the one this crate has carried since Phase 14 as
/// `remaining_header`, then `aux_hi` -- the field whose `== 12` rule silently
/// refused 88 real lines. `12` is not a framing constant: it is the oid of the
/// `Labels` layer, and `8`, the value on A01's refused page border, is the oid
/// of `Default`.
///
/// Three claims are ratcheted: the tally identity, that a layer is never on a
/// layer, and that the families writing a layer are exactly the graphic ones.
///
/// See `docs/analysis/2026-08-27-aux-hi-is-the-sheet-layer.md` and
/// `examples/probe_sheetlayer_edge_lives_on_the_graphic.rs`.
#[test]
fn psm_aux_hi_is_the_sheet_layer_every_object_sits_on() {
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::Read;

    const CHAIN_MAGIC: u32 = 0x6C90_F544;
    const LAYER: u16 = 0x0081;
    /// The layer subsystem itself: managers, view filter sets, layers, groups.
    /// None of them is on a layer, so none is counted as a member.
    const LAYER_SUBSYSTEM: [u16; 5] = [0x0042, 0x0057, 0x0060, 0x0081, 0x0088];

    fn u32_at(data: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
    }
    fn storage_of(path: &str) -> String {
        let norm = path.replace('\\', "/");
        let norm = norm.strip_prefix('/').unwrap_or(&norm);
        match norm.rsplit_once('/') {
            Some((pre, _)) => pre.to_string(),
            None => String::new(),
        }
    }

    let mut layers_seen = 0usize;
    let mut layers_matching = 0usize;
    let mut objects_on_a_layer = 0usize;
    let mut declared_total = 0usize;
    let mut layers_with_zero_aux_hi = 0usize;
    let mut carriers: BTreeMap<u16, usize> = BTreeMap::new();
    let mut layerless_outside_stylecluster = 0usize;
    let mut any = false;

    for fixture in [
        "D06.pid",
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
    ] {
        let path = format!("test-file/{fixture}");
        if !std::path::Path::new(&path).exists() {
            continue;
        }
        any = true;

        // oid -> (family, aux_hi), first record wins. Every object of this
        // corpus whose records repeat agrees with itself at +8, so folding
        // records onto objects cannot move a member.
        let mut storages: BTreeMap<String, BTreeMap<u32, (u16, u32, String)>> = BTreeMap::new();
        let mut layer_counts: BTreeMap<String, BTreeMap<u32, u32>> = BTreeMap::new();
        let file = std::fs::File::open(&path).expect("fixture opens");
        let mut cfb = cfb::CompoundFile::open(file).expect("fixture is a compound file");
        let stream_paths: Vec<String> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|entry| entry.path().to_string_lossy().into_owned())
            .collect();
        for stream_path in stream_paths {
            let mut data = Vec::new();
            let Ok(mut stream) = cfb.open_stream(&stream_path) else {
                continue;
            };
            if stream.read_to_end(&mut data).is_err() || u32_at(&data, 0) != Some(CHAIN_MAGIC) {
                continue;
            }
            let leaf = stream_path
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(&stream_path)
                .to_string();
            let storage = storage_of(&stream_path);
            for at in pid_parse::parsers::sheet_records::sheet_record_starts(&data) {
                let Some(type_code) = u32_at(&data, at).map(|word| (word & 0x3FFF) as u16) else {
                    continue;
                };
                let Some(len) = u32_at(&data, at + 2) else {
                    continue;
                };
                let Some(payload) = data.get(at + 6..at + 6 + len as usize) else {
                    continue;
                };
                let (Some(oid), Some(aux_hi)) = (u32_at(payload, 0), u32_at(payload, 8)) else {
                    continue;
                };
                if type_code == LAYER {
                    if let Some(objects) = u32_at(payload, 12) {
                        layer_counts
                            .entry(storage.clone())
                            .or_default()
                            .entry(oid)
                            .or_insert(objects);
                    }
                }
                storages
                    .entry(storage.clone())
                    .or_default()
                    .entry(oid)
                    .or_insert((type_code, aux_hi, leaf.clone()));
            }
        }

        for (storage, layers) in &layer_counts {
            let objects = storages
                .get(storage)
                .expect("a layer's storage has records");
            let mut observed: BTreeMap<u32, usize> = BTreeMap::new();
            for (family, aux_hi, leaf) in objects.values() {
                if LAYER_SUBSYSTEM.contains(family) {
                    if *family == LAYER {
                        layers_with_zero_aux_hi += usize::from(*aux_hi == 0);
                    }
                    continue;
                }
                if layers.contains_key(aux_hi) {
                    *observed.entry(*aux_hi).or_default() += 1;
                    *carriers.entry(*family).or_default() += 1;
                } else if *aux_hi == 0 && leaf != "StyleCluster" {
                    // Only the style library's glyph lines are graphics that
                    // sit on no sheet; anything else would break the reading.
                    let graphic = matches!(family, 0x0013 | 0x0018 | 0x004D | 0x005E | 0x0084);
                    layerless_outside_stylecluster += usize::from(graphic);
                }
            }
            for (oid, declared) in layers {
                layers_seen += 1;
                declared_total += *declared as usize;
                let seen = observed.get(oid).copied().unwrap_or_default();
                objects_on_a_layer += seen;
                if seen == *declared as usize {
                    layers_matching += 1;
                }
            }
        }
    }

    if !any {
        eprintln!("skip: no fixture present");
        return;
    }

    assert_eq!(
        layers_seen, 290,
        "JSheetLayer count across the four fixtures"
    );
    assert_eq!(
        layers_matching, layers_seen,
        "{layers_matching} of {layers_seen} layers have exactly as many objects naming them at \
         +8 as their own +12 tally declares"
    );
    assert_eq!(
        (objects_on_a_layer, declared_total),
        (1240, 1240),
        "every object the layers count is an object that names one at +8"
    );
    assert_eq!(
        layers_with_zero_aux_hi, 290,
        "a JSheetLayer is not itself on a layer -- its own aux_hi is zero"
    );
    assert_eq!(
        layerless_outside_stylecluster, 0,
        "the only graphics with no layer are the style library's glyph lines"
    );
    assert_eq!(
        carriers.keys().copied().collect::<BTreeSet<u16>>(),
        BTreeSet::from([
            0x0013, 0x0018, 0x003D, 0x004D, 0x0059, 0x005D, 0x005E, 0x0061, 0x0084, 0x00CE, 0x0115
        ]),
        "aux_hi names a layer for the graphic families and for nothing else -- not dynamic \
         attribute rows, dependencies, styles or the layer subsystem"
    );
}

/// The tag-181 incoming edge and `igSymbol2d::jsite_ref` are the same fact
/// reached two ways. A symbol placed in a site references it, so the site's
/// space-map entry lists the symbol as an incoming reference tagged 181; and
/// the symbol's own record states the site id at `jsite_ref` (the u32 before
/// the placement-matrix tag). Decoding the referrer record and comparing the
/// two must agree on every `igSymbol2d` edge -- 80 of the 84 tag-181 members
/// across the four fixtures (the other four are `0x003D igSmartFrame2d`,
/// which has no `jsite_ref`).
#[test]
fn psm_space_map_181_edges_match_igsymbol_jsite_ref() {
    use std::collections::BTreeMap;
    use std::io::Read;

    const CHAIN_MAGIC: u32 = 0x6C90_F544;
    const SEGMENT_SHIFT: u32 = 13;

    fn u32_at(data: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
    }
    fn storage_of(path: &str) -> String {
        let norm = path.replace('\\', "/");
        let norm = norm.strip_prefix('/').unwrap_or(&norm);
        if let Some(at) = norm.find("PSMspacemap") {
            norm[..at].trim_end_matches('/').to_string()
        } else if let Some((pre, _)) = norm.rsplit_once('/') {
            pre.to_string()
        } else {
            String::new()
        }
    }
    fn segment_of(path: &str) -> Option<u32> {
        let leaf = path.rsplit(['/', '\\']).next()?;
        u32::from_str_radix(leaf.strip_prefix("0x")?, 16)
            .ok()
            .map(|address| address >> SEGMENT_SHIFT)
    }

    let mut matched = 0usize;
    let mut mismatched: Vec<String> = Vec::new();
    let mut any = false;

    for fixture in [
        "D06.pid",
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
    ] {
        let fpath = format!("test-file/{fixture}");
        if !std::path::Path::new(&fpath).exists() {
            continue;
        }
        let Some(doc) = parse_test_file(fixture) else {
            continue;
        };

        // storage -> symbol oid -> jsite_ref, via the real decoder.
        let mut jsite_ref: BTreeMap<String, BTreeMap<u32, u32>> = BTreeMap::new();
        let file = std::fs::File::open(&fpath).expect("fixture opens");
        let mut cfb = cfb::CompoundFile::open(file).expect("fixture is a compound file");
        let stream_paths: Vec<String> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|entry| entry.path().to_string_lossy().into_owned())
            .collect();
        for stream_path in stream_paths {
            let mut data = Vec::new();
            let Ok(mut stream) = cfb.open_stream(&stream_path) else {
                continue;
            };
            if stream.read_to_end(&mut data).is_err() || u32_at(&data, 0) != Some(CHAIN_MAGIC) {
                continue;
            }
            let by_oid = jsite_ref.entry(storage_of(&stream_path)).or_default();
            for symbol in pid_parse::parsers::sheet_records::decode_igsymbols(&data) {
                by_oid.insert(symbol.oid, symbol.jsite_ref);
            }
        }

        for (map_path, map) in &doc.psm_space_maps {
            let Some(segment) = segment_of(map_path) else {
                continue;
            };
            let by_oid = jsite_ref.get(&storage_of(map_path));
            for entry in &map.entries {
                let site_id = (segment << SEGMENT_SHIFT) | u32::from(entry.index);
                for member in entry.live_members() {
                    if member.tag != 181 {
                        continue;
                    }
                    let Some(&referrer_jsite) = by_oid.and_then(|m| m.get(&member.value)) else {
                        continue; // the 0x003D igSmartFrame2d referrers
                    };
                    any = true;
                    if referrer_jsite == site_id {
                        matched += 1;
                    } else {
                        mismatched.push(format!(
                            "{fixture}: symbol {} on site {site_id} has jsite_ref {referrer_jsite}",
                            member.value
                        ));
                    }
                }
            }
        }
    }

    if !any {
        return;
    }
    assert!(
        mismatched.is_empty(),
        "every igSymbol2d tag-181 edge should have jsite_ref == the site entry: {mismatched:?}"
    );
    assert_eq!(
        matched, 80,
        "the four sheet fixtures should hold 80 igSymbol2d tag-181 edges, all agreeing"
    );
}

/// The members whose `value` names no record at all are not scattered: every
/// one of them sits on a `0x00C7` entry, and `0x00C7` is one tight structure.
///
/// Each `0x00C7` is a 24-byte leaf whose `parent_ref` names an `0x00EA` group
/// record, every one of them has a space-map entry, and every member on such
/// an entry is tagged 182. Where the referrer does have a record it is a
/// `0x00BD` (the object `PSMroots` calls `SymbolInformation`) or a `0x006F`,
/// twelve of each; the other 191 members name nothing this storage persisted.
/// That is the whole recordless population of the corpus apart from the one
/// known stale `0x00FA` edge, so "no record" is a property of one layer of one
/// structure, not a general class of object -- see
/// `docs/analysis/2026-08-27-the-recordless-182-referrers-are-symbolinformation.md`.
#[test]
fn psm_space_map_recordless_referrers_only_sit_on_0x00c7_entries() {
    use std::collections::BTreeMap;
    use std::io::Read;

    const CHAIN_MAGIC: u32 = 0x6C90_F544;
    const SEGMENT_SHIFT: u32 = 13;

    fn u16_at(data: &[u8], at: usize) -> Option<u16> {
        Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
    }
    fn u32_at(data: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
    }
    fn storage_of(path: &str) -> String {
        let norm = path.replace('\\', "/");
        let norm = norm.strip_prefix('/').unwrap_or(&norm);
        if let Some(at) = norm.find("PSMspacemap") {
            norm[..at].trim_end_matches('/').to_string()
        } else if let Some((pre, _)) = norm.rsplit_once('/') {
            pre.to_string()
        } else {
            String::new()
        }
    }
    fn segment_of(path: &str) -> Option<u32> {
        let leaf = path.rsplit(['/', '\\']).next()?;
        u32::from_str_radix(leaf.strip_prefix("0x")?, 16)
            .ok()
            .map(|address| address >> SEGMENT_SHIFT)
    }

    // oid -> the (type code, payload) of every record carrying that oid.
    type ByOid = BTreeMap<u32, Vec<(u16, Vec<u8>)>>;

    let mut recordless_by_tag: BTreeMap<u16, usize> = BTreeMap::new();
    let mut recordless_on_c7 = 0usize;
    let mut c7_records = 0usize;
    let mut c7_with_entry = 0usize;
    let mut c7_parent_is_0x00ea = 0usize;
    let mut c7_member_tags: BTreeMap<u16, usize> = BTreeMap::new();
    let mut c7_referrer_families: BTreeMap<u16, usize> = BTreeMap::new();
    let mut any = false;

    for fixture in [
        "D06.pid",
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
    ] {
        let path = format!("test-file/{fixture}");
        if !std::path::Path::new(&path).exists() {
            continue;
        }
        let Some(doc) = parse_test_file(fixture) else {
            continue;
        };

        // storage -> oid -> (type code, payload) for every record-chain stream.
        let mut records: BTreeMap<String, ByOid> = BTreeMap::new();
        let file = std::fs::File::open(&path).expect("fixture opens");
        let mut cfb = cfb::CompoundFile::open(file).expect("fixture is a compound file");
        let stream_paths: Vec<String> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|entry| entry.path().to_string_lossy().into_owned())
            .collect();
        for stream_path in stream_paths {
            let mut data = Vec::new();
            let Ok(mut stream) = cfb.open_stream(&stream_path) else {
                continue;
            };
            if stream.read_to_end(&mut data).is_err() || u32_at(&data, 0) != Some(CHAIN_MAGIC) {
                continue;
            }
            let starts = pid_parse::parsers::sheet_records::sheet_record_starts(&data);
            let storage = records.entry(storage_of(&stream_path)).or_default();
            for at in starts {
                let (Some(type_word), Some(len)) = (u16_at(&data, at), u32_at(&data, at + 2))
                else {
                    continue;
                };
                let Some(payload) = data.get(at + 6..at + 6 + len as usize) else {
                    continue;
                };
                let Some(oid) = u32_at(payload, 0) else {
                    continue;
                };
                storage
                    .entry(oid)
                    .or_default()
                    .push((type_word & 0x3FFF, payload.to_vec()));
            }
        }

        let family_is = |by_oid: &ByOid, oid: u32, want: u16| {
            by_oid
                .get(&oid)
                .is_some_and(|found| found.iter().any(|(code, _)| *code == want))
        };

        for (map_path, map) in &doc.psm_space_maps {
            let Some(segment) = segment_of(map_path) else {
                continue;
            };
            let Some(by_oid) = records.get(&storage_of(map_path)) else {
                continue;
            };
            for entry in &map.entries {
                any = true;
                let id = (segment << SEGMENT_SHIFT) | u32::from(entry.index);
                let on_c7 = family_is(by_oid, id, 0x00C7);
                for member in entry.live_members() {
                    let referrer = by_oid.get(&member.value);
                    if referrer.is_none() {
                        *recordless_by_tag.entry(member.tag).or_default() += 1;
                        if on_c7 {
                            recordless_on_c7 += 1;
                        }
                    }
                    if !on_c7 {
                        continue;
                    }
                    *c7_member_tags.entry(member.tag).or_default() += 1;
                    for (code, _) in referrer.into_iter().flatten() {
                        *c7_referrer_families.entry(*code).or_default() += 1;
                    }
                }
            }
        }

        for (storage, by_oid) in &records {
            for (oid, found) in by_oid {
                let Some((_, payload)) = found.iter().find(|(code, _)| *code == 0x00C7) else {
                    continue;
                };
                c7_records += 1;
                if let Some(parent) = u32_at(payload, 4) {
                    if family_is(by_oid, parent, 0x00EA) {
                        c7_parent_is_0x00ea += 1;
                    }
                }
                let has_entry = doc.psm_space_maps.iter().any(|(map_path, map)| {
                    storage_of(map_path) == *storage
                        && segment_of(map_path) == Some(oid >> SEGMENT_SHIFT)
                        && map
                            .entries
                            .iter()
                            .any(|entry| u32::from(entry.index) == oid & 0x1FFF)
                });
                if has_entry {
                    c7_with_entry += 1;
                }
            }
        }
    }

    if !any {
        return;
    }

    assert_eq!(
        recordless_by_tag,
        BTreeMap::from([(182, 191), (249, 1)]),
        "the corpus should hold 191 recordless tag-182 members plus the one stale 0x00FA edge"
    );
    assert_eq!(
        recordless_on_c7, 191,
        "every recordless tag-182 member should sit on a 0x00C7 entry"
    );
    assert_eq!(
        (c7_records, c7_with_entry, c7_parent_is_0x00ea),
        (203, 203, 203),
        "all 203 0x00C7 records should have an entry and an 0x00EA parent"
    );
    assert_eq!(
        c7_member_tags,
        BTreeMap::from([(182, 215)]),
        "a 0x00C7 entry should only ever be referenced with tag 182"
    );
    assert_eq!(
        c7_referrer_families,
        BTreeMap::from([(0x006F, 12), (0x00BD, 12)]),
        "the 24 recorded referrers of a 0x00C7 should be twelve 0x006F and twelve 0x00BD"
    );
}

/// `SymbolInformation` is the one named root the corpus does not always
/// persist -- which is what identifies the recordless referrers.
///
/// Every `PSMroots` entry names an object by its persist id, and for all the
/// other names (`DocStore`, `StyleLibrarian`, `TopVFSet`, `_SupportOnlyList`,
/// the two document roots, the dynamic-attribute set table) that id resolves
/// to a record in the same storage, every time. `SymbolInformation` resolves
/// only 25 times out of 41, and when it does the record is always `0x00BD`.
/// Five of the ones that do not resolve are members of the recordless tag-182
/// population -- named by the root table, referenced by the space map, absent
/// from every cluster.
#[test]
fn psm_roots_symbol_information_is_the_only_root_without_a_record() {
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::Read;

    const CHAIN_MAGIC: u32 = 0x6C90_F544;
    const SEGMENT_SHIFT: u32 = 13;

    fn u16_at(data: &[u8], at: usize) -> Option<u16> {
        Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
    }
    fn u32_at(data: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
    }
    fn storage_of(path: &str) -> String {
        let norm = path.replace('\\', "/");
        let norm = norm.strip_prefix('/').unwrap_or(&norm);
        if let Some(at) = norm.find("PSMspacemap") {
            norm[..at].trim_end_matches('/').to_string()
        } else if let Some((pre, _)) = norm.rsplit_once('/') {
            pre.to_string()
        } else {
            String::new()
        }
    }
    fn segment_of(path: &str) -> Option<u32> {
        let leaf = path.rsplit(['/', '\\']).next()?;
        u32::from_str_radix(leaf.strip_prefix("0x")?, 16)
            .ok()
            .map(|address| address >> SEGMENT_SHIFT)
    }

    // root name -> (with a record, without one)
    let mut by_name: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut families_by_name: BTreeMap<String, BTreeSet<u16>> = BTreeMap::new();
    let mut recordless_roots_used_as_referrer = 0usize;
    let mut any = false;

    for fixture in [
        "D06.pid",
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
    ] {
        let path = format!("test-file/{fixture}");
        if !std::path::Path::new(&path).exists() {
            continue;
        }
        let Some(doc) = parse_test_file(fixture) else {
            continue;
        };

        let mut records: BTreeMap<String, BTreeMap<u32, u16>> = BTreeMap::new();
        let mut roots: Vec<(String, u32, String)> = Vec::new();
        let file = std::fs::File::open(&path).expect("fixture opens");
        let mut cfb = cfb::CompoundFile::open(file).expect("fixture is a compound file");
        let stream_paths: Vec<String> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|entry| entry.path().to_string_lossy().into_owned())
            .collect();
        for stream_path in stream_paths {
            let mut data = Vec::new();
            let Ok(mut stream) = cfb.open_stream(&stream_path) else {
                continue;
            };
            if stream.read_to_end(&mut data).is_err() {
                continue;
            }
            let storage = storage_of(&stream_path);
            if stream_path.rsplit(['/', '\\']).next() == Some("PSMroots") {
                if let Some(parsed) = pid_parse::parsers::psm_tables::parse_psm_roots(&data) {
                    for root in parsed.entries {
                        roots.push((storage.clone(), root.id, root.name));
                    }
                }
                continue;
            }
            if u32_at(&data, 0) != Some(CHAIN_MAGIC) {
                continue;
            }
            let by_oid = records.entry(storage).or_default();
            for at in pid_parse::parsers::sheet_records::sheet_record_starts(&data) {
                let (Some(type_word), Some(len)) = (u16_at(&data, at), u32_at(&data, at + 2))
                else {
                    continue;
                };
                let Some(oid) = data
                    .get(at + 6..at + 6 + len as usize)
                    .and_then(|payload| u32_at(payload, 0))
                else {
                    continue;
                };
                by_oid.entry(oid).or_insert(type_word & 0x3FFF);
            }
        }

        // Every id the space map records as a referrer, per storage.
        let mut referrers: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
        for (map_path, map) in &doc.psm_space_maps {
            if segment_of(map_path).is_none() {
                continue;
            }
            let seen = referrers.entry(storage_of(map_path)).or_default();
            for entry in &map.entries {
                for member in entry.live_members() {
                    seen.insert(member.value);
                }
            }
        }

        for (storage, id, name) in roots {
            any = true;
            let counts = by_name.entry(name.clone()).or_default();
            match records.get(&storage).and_then(|by_oid| by_oid.get(&id)) {
                Some(code) => {
                    counts.0 += 1;
                    families_by_name.entry(name).or_default().insert(*code);
                }
                None => {
                    counts.1 += 1;
                    if referrers
                        .get(&storage)
                        .is_some_and(|seen| seen.contains(&id))
                    {
                        recordless_roots_used_as_referrer += 1;
                    }
                }
            }
        }
    }

    if !any {
        return;
    }

    let unresolved: Vec<_> = by_name
        .iter()
        .filter(|(name, (_, without))| *without > 0 && name.as_str() != "SymbolInformation")
        .collect();
    assert!(
        unresolved.is_empty(),
        "only SymbolInformation should ever miss its record, but so did {unresolved:?}"
    );
    assert_eq!(
        by_name.get("SymbolInformation").copied(),
        Some((25, 16)),
        "the corpus should name 41 SymbolInformation roots, 16 of them without a record"
    );
    assert_eq!(
        families_by_name.get("SymbolInformation"),
        Some(&BTreeSet::from([0x00BD])),
        "a SymbolInformation root that does resolve should always be a 0x00BD record"
    );
    assert_eq!(
        recordless_roots_used_as_referrer, 5,
        "five recordless SymbolInformation roots should appear as space-map referrers"
    );
    assert_eq!(
        by_name
            .values()
            .map(|(with, without)| with + without)
            .sum::<usize>(),
        96,
        "the four fixtures should hold 96 PSMroots entries across all storages"
    );
}

/// The long form of `0x00BD` is what a missing referrer would have been.
///
/// `0x00BD` is `JSymbolInformation` (`symbol.dex`) and `0x00C7` is a
/// `Double Value Object` (`exprdex.dll`), both named through the type-code
/// table — so this is a symbol carrying named variables. It has two shapes
/// behind one 44-byte head: a stub, and — when the `u16` at `+14` is `0x0010`
/// — a `u32` count followed by that many variables, each `u8 1`, `u32 1`, an
/// `f64` value, a `u16` char count, a UTF-16LE name (`Left` / `Right` /
/// `Bottom` / `Top`) and the `u32` oid of the `0x00C7` holding that value.
/// The eight long forms in the corpus consume their payload to the last byte
/// under that reading, and where the named `0x00C7` lives in the same storage
/// its own `f64` at `+12` is the one the inline entry carries — as it must,
/// the record being that double.
///
/// The partition is the point. Every one of the 203 `0x00C7` records is
/// either listed by a surviving long form (12) or carries a referrer with no
/// record (191), never both and never neither. So the records the space map
/// points at and cannot find are exactly the `JSymbolInformation` long forms
/// that would have listed those variables.
#[test]
fn symbol_information_long_form_lists_the_0x00c7_it_refers_to() {
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::Read;

    const CHAIN_MAGIC: u32 = 0x6C90_F544;
    const SEGMENT_SHIFT: u32 = 13;
    const HAS_CONNECT_POINTS: u16 = 0x0010;

    fn u16_at(data: &[u8], at: usize) -> Option<u16> {
        Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
    }
    fn u32_at(data: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
    }
    fn f64_at(data: &[u8], at: usize) -> Option<f64> {
        Some(f64::from_le_bytes(data.get(at..at + 8)?.try_into().ok()?))
    }
    fn storage_of(path: &str) -> String {
        let norm = path.replace('\\', "/");
        let norm = norm.strip_prefix('/').unwrap_or(&norm);
        if let Some(at) = norm.find("PSMspacemap") {
            norm[..at].trim_end_matches('/').to_string()
        } else if let Some((pre, _)) = norm.rsplit_once('/') {
            pre.to_string()
        } else {
            String::new()
        }
    }
    fn segment_of(path: &str) -> Option<u32> {
        let leaf = path.rsplit(['/', '\\']).next()?;
        u32::from_str_radix(leaf.strip_prefix("0x")?, 16)
            .ok()
            .map(|address| address >> SEGMENT_SHIFT)
    }

    /// `(f64, name, referenced oid)` per named variable, or `None` when the
    /// payload does not end exactly where the reading says it should.
    fn named_variables(payload: &[u8]) -> Option<Vec<(f64, String, u32)>> {
        if u16_at(payload, 14)? != HAS_CONNECT_POINTS {
            return None;
        }
        let mut at = 44usize;
        let count = u32_at(payload, at)?;
        at += 4;
        let mut out = Vec::new();
        for _ in 0..count {
            if *payload.get(at)? != 1 || u32_at(payload, at + 1)? != 1 {
                return None;
            }
            at += 5;
            let value = f64_at(payload, at)?;
            at += 8;
            let chars = usize::from(u16_at(payload, at)?);
            at += 2;
            let name: String =
                char::decode_utf16((0..chars).map_while(|i| u16_at(payload, at + i * 2)))
                    .collect::<Result<_, _>>()
                    .ok()?;
            if name.chars().count() != chars {
                return None;
            }
            at += chars * 2;
            out.push((value, name, u32_at(payload, at)?));
            at += 4;
        }
        (at == payload.len()).then_some(out)
    }

    // oid -> the (type code, payload) of every record carrying that oid.
    type ByOid = BTreeMap<u32, Vec<(u16, Vec<u8>)>>;

    let mut long_forms = 0usize;
    let mut stubs = 0usize;
    let mut points = 0usize;
    let mut resolved_points = 0usize;
    let mut value_agrees = 0usize;
    let mut names: BTreeMap<String, usize> = BTreeMap::new();
    let mut c7_total = 0usize;
    let mut c7_listed = 0usize;
    let mut c7_with_phantom = 0usize;
    let mut c7_both = 0usize;
    let mut any = false;

    for fixture in [
        "D06.pid",
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
    ] {
        let path = format!("test-file/{fixture}");
        if !std::path::Path::new(&path).exists() {
            continue;
        }
        let Some(doc) = parse_test_file(fixture) else {
            continue;
        };

        let mut records: BTreeMap<String, ByOid> = BTreeMap::new();
        let file = std::fs::File::open(&path).expect("fixture opens");
        let mut cfb = cfb::CompoundFile::open(file).expect("fixture is a compound file");
        let stream_paths: Vec<String> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|entry| entry.path().to_string_lossy().into_owned())
            .collect();
        for stream_path in stream_paths {
            let mut data = Vec::new();
            let Ok(mut stream) = cfb.open_stream(&stream_path) else {
                continue;
            };
            if stream.read_to_end(&mut data).is_err() || u32_at(&data, 0) != Some(CHAIN_MAGIC) {
                continue;
            }
            let by_oid = records.entry(storage_of(&stream_path)).or_default();
            for at in pid_parse::parsers::sheet_records::sheet_record_starts(&data) {
                let (Some(type_word), Some(len)) = (u16_at(&data, at), u32_at(&data, at + 2))
                else {
                    continue;
                };
                let Some(payload) = data.get(at + 6..at + 6 + len as usize) else {
                    continue;
                };
                let Some(oid) = u32_at(payload, 0) else {
                    continue;
                };
                by_oid
                    .entry(oid)
                    .or_default()
                    .push((type_word & 0x3FFF, payload.to_vec()));
            }
        }

        // storage -> the 0x00C7 oids some surviving long form lists
        let mut listed: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
        for (storage, by_oid) in &records {
            for found in by_oid.values() {
                for (code, payload) in found {
                    if *code != 0x00BD {
                        continue;
                    }
                    any = true;
                    let Some(entries) = named_variables(payload) else {
                        assert_eq!(
                            u16_at(payload, 14),
                            Some(0),
                            "{fixture}: a 0x00BD flagged 0x0010 did not decode as named variables"
                        );
                        stubs += 1;
                        continue;
                    };
                    long_forms += 1;
                    for (value, name, referenced) in entries {
                        points += 1;
                        *names.entry(name).or_default() += 1;
                        listed
                            .entry(storage.clone())
                            .or_default()
                            .insert(referenced);
                        let Some(leaf) = by_oid.get(&referenced) else {
                            continue;
                        };
                        for (leaf_code, leaf_payload) in leaf {
                            if *leaf_code != 0x00C7 {
                                continue;
                            }
                            resolved_points += 1;
                            if f64_at(leaf_payload, 12) == Some(value) {
                                value_agrees += 1;
                            }
                        }
                    }
                }
            }
        }

        // storage -> the 0x00C7 oids whose entry names a referrer with no record
        let mut with_phantom: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
        for (map_path, map) in &doc.psm_space_maps {
            let Some(segment) = segment_of(map_path) else {
                continue;
            };
            let storage = storage_of(map_path);
            let Some(by_oid) = records.get(&storage) else {
                continue;
            };
            for entry in &map.entries {
                let id = (segment << SEGMENT_SHIFT) | u32::from(entry.index);
                let is_c7 = by_oid
                    .get(&id)
                    .is_some_and(|found| found.iter().any(|(code, _)| *code == 0x00C7));
                if is_c7
                    && entry
                        .live_members()
                        .iter()
                        .any(|member| !by_oid.contains_key(&member.value))
                {
                    with_phantom.entry(storage.clone()).or_default().insert(id);
                }
            }
        }

        for (storage, by_oid) in &records {
            let listed = listed.get(storage).cloned().unwrap_or_default();
            let phantom = with_phantom.get(storage).cloned().unwrap_or_default();
            for (oid, found) in by_oid {
                if !found.iter().any(|(code, _)| *code == 0x00C7) {
                    continue;
                }
                c7_total += 1;
                if listed.contains(oid) {
                    c7_listed += 1;
                }
                if phantom.contains(oid) {
                    c7_with_phantom += 1;
                }
                if listed.contains(oid) && phantom.contains(oid) {
                    c7_both += 1;
                }
            }
        }
    }

    if !any {
        return;
    }

    assert_eq!(
        (long_forms, stubs, points),
        (8, 37, 24),
        "the corpus should hold 8 long-form 0x00BD records carrying 24 named variables, \
         plus 37 records that stop at the head"
    );
    assert_eq!(
        names,
        BTreeMap::from([
            ("Bottom".to_string(), 4),
            ("Left".to_string(), 6),
            ("Right".to_string(), 8),
            ("Top".to_string(), 6),
        ]),
        "the variables should only carry the four side names"
    );
    assert_eq!(
        (resolved_points, value_agrees),
        (12, 12),
        "every variable resolving to a 0x00C7 in the same storage should carry that \
         record's own f64"
    );
    assert_eq!(
        (c7_total, c7_listed, c7_with_phantom, c7_both),
        (203, 12, 191, 0),
        "every 0x00C7 is either listed by a surviving 0x00BD long form or carries a \
         referrer with no record, never both"
    );
}

/// `0x006F` closes the family: a variable drives a dimension through a
/// formula.
///
/// The type-code table calls it `Assoc subsystem Standard Relation
/// implementation` (`jengine.dll`), and the payload says what it relates. A
/// constant class GUID at `+12`, the `JBExpression object` CLSID at `+38`,
/// the `Double Value Object` CLSID at `+58` for the expression's value type,
/// then a `u32`-counted ASCII signature (`%>i%<i`, `%>` for the output
/// operand and `%<` for each input), one `u32 oid` + interface-GUID slot per
/// marker, and a `u32`-counted UTF-16 formula that ends the payload exactly.
/// All thirteen decode with nothing left over.
///
/// The operands are the point. The output is always an `0x0115` JDim, and
/// the inputs are the `0x00C7` Double Value Objects — the same twelve a
/// surviving `JSymbolInformation` long form lists. So each of those values
/// has exactly two referrers for two different reasons: the symbol
/// information that names it `Left` / `Right` / `Bottom` / `Top`, and the
/// relation that feeds it into a dimension.
#[test]
fn standard_relation_binds_a_double_value_to_a_dimension() {
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::Read;

    const CHAIN_MAGIC: u32 = 0x6C90_F544;
    /// `DE264241-E929-11CE-A608-080036C61102` — `JBExpression object`.
    const EXPRESSION_CLSID: [u8; 16] = [
        0x41, 0x42, 0x26, 0xDE, 0x29, 0xE9, 0xCE, 0x11, 0xA6, 0x08, 0x08, 0x00, 0x36, 0xC6, 0x11,
        0x02,
    ];
    /// `D97A3FB0-1601-11CE-B7EE-08003601E53B` — `Double Value Object`.
    const DOUBLE_VALUE_CLSID: [u8; 16] = [
        0xB0, 0x3F, 0x7A, 0xD9, 0x01, 0x16, 0xCE, 0x11, 0xB7, 0xEE, 0x08, 0x00, 0x36, 0x01, 0xE5,
        0x3B,
    ];
    /// `0145EEC0-1602-11CE-B7EE-08003601E53B` — the per-operand interface.
    const OPERAND_IID: [u8; 16] = [
        0xC0, 0xEE, 0x45, 0x01, 0x02, 0x16, 0xCE, 0x11, 0xB7, 0xEE, 0x08, 0x00, 0x36, 0x01, 0xE5,
        0x3B,
    ];

    fn u16_at(data: &[u8], at: usize) -> Option<u16> {
        Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
    }
    fn u32_at(data: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
    }
    fn storage_of(path: &str) -> String {
        let norm = path.replace('\\', "/");
        let norm = norm.strip_prefix('/').unwrap_or(&norm);
        if let Some((pre, _)) = norm.rsplit_once('/') {
            pre.to_string()
        } else {
            String::new()
        }
    }
    fn find(haystack: &[u8], needle: &[u8; 16], from: usize) -> Option<usize> {
        (from..haystack.len().saturating_sub(15)).find(|at| &haystack[*at..*at + 16] == needle)
    }

    /// `(signature, operand oids, formula)`, or `None` when the payload does
    /// not end exactly where the reading says it should.
    fn relation(payload: &[u8]) -> Option<(String, Vec<u32>, String)> {
        if find(payload, &EXPRESSION_CLSID, 0)? != 38 {
            return None;
        }
        let mut at = find(payload, &DOUBLE_VALUE_CLSID, 0)? + 16;
        let signature = String::from_utf8(
            payload
                .get(at + 4..at + 4 + u32_at(payload, at)? as usize)?
                .iter()
                .take_while(|byte| **byte != 0)
                .copied()
                .collect(),
        )
        .ok()?;
        at += 4 + u32_at(payload, at)? as usize;
        let mut operands = Vec::new();
        while let Some(slot) = find(payload, &OPERAND_IID, at) {
            operands.push(u32_at(payload, slot - 4)?);
            at = slot + 16;
        }
        let chars = u32_at(payload, at)? as usize;
        (at + 4 + chars * 2 == payload.len()).then(|| {
            let formula: String =
                char::decode_utf16((0..chars).map_while(|i| u16_at(payload, at + 4 + i * 2)))
                    .filter_map(Result::ok)
                    .filter(|glyph| *glyph != '\0')
                    .collect();
            (signature, operands, formula)
        })
    }

    // oid -> the (type code, payload) of every record carrying that oid.
    type ByOid = BTreeMap<u32, Vec<(u16, Vec<u8>)>>;

    let mut relations = 0usize;
    let mut exact = 0usize;
    let mut signatures: BTreeMap<String, usize> = BTreeMap::new();
    let mut formulas: BTreeMap<String, usize> = BTreeMap::new();
    let mut output_families: BTreeMap<u16, usize> = BTreeMap::new();
    let mut input_families: BTreeMap<u16, usize> = BTreeMap::new();
    let mut double_value_inputs: BTreeSet<(String, u32)> = BTreeSet::new();
    let mut any = false;

    for fixture in [
        "D06.pid",
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
    ] {
        let path = format!("test-file/{fixture}");
        if !std::path::Path::new(&path).exists() {
            continue;
        }

        let mut records: BTreeMap<String, ByOid> = BTreeMap::new();
        let file = std::fs::File::open(&path).expect("fixture opens");
        let mut cfb = cfb::CompoundFile::open(file).expect("fixture is a compound file");
        let stream_paths: Vec<String> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|entry| entry.path().to_string_lossy().into_owned())
            .collect();
        for stream_path in stream_paths {
            let mut data = Vec::new();
            let Ok(mut stream) = cfb.open_stream(&stream_path) else {
                continue;
            };
            if stream.read_to_end(&mut data).is_err() || u32_at(&data, 0) != Some(CHAIN_MAGIC) {
                continue;
            }
            let by_oid = records.entry(storage_of(&stream_path)).or_default();
            for at in pid_parse::parsers::sheet_records::sheet_record_starts(&data) {
                let (Some(type_word), Some(len)) = (u16_at(&data, at), u32_at(&data, at + 2))
                else {
                    continue;
                };
                let Some(payload) = data.get(at + 6..at + 6 + len as usize) else {
                    continue;
                };
                let Some(oid) = u32_at(payload, 0) else {
                    continue;
                };
                by_oid
                    .entry(oid)
                    .or_default()
                    .push((type_word & 0x3FFF, payload.to_vec()));
            }
        }

        for (storage, by_oid) in &records {
            for found in by_oid.values() {
                for (code, payload) in found {
                    if *code != 0x006F {
                        continue;
                    }
                    any = true;
                    relations += 1;
                    let Some((signature, operands, formula)) = relation(payload) else {
                        continue;
                    };
                    if signature.matches('%').count() != operands.len() {
                        continue;
                    }
                    exact += 1;
                    *signatures.entry(signature).or_default() += 1;
                    *formulas.entry(formula).or_default() += 1;
                    for (index, operand) in operands.iter().enumerate() {
                        let family = by_oid
                            .get(operand)
                            .and_then(|rows| rows.first().map(|(code, _)| *code))
                            .unwrap_or_default();
                        if index == 0 {
                            *output_families.entry(family).or_default() += 1;
                        } else {
                            *input_families.entry(family).or_default() += 1;
                            if family == 0x00C7 {
                                double_value_inputs.insert((storage.clone(), *operand));
                            }
                        }
                    }
                }
            }
        }
    }

    if !any {
        return;
    }

    assert_eq!(
        (relations, exact),
        (13, 13),
        "all thirteen 0x006F relations should decode with the operand count the signature \
         promises and a formula that ends the payload"
    );
    assert_eq!(
        signatures,
        BTreeMap::from([("%>i%<i".to_string(), 12), ("%>i%<i%<i".to_string(), 1)]),
        "a relation should take one output and one or two inputs"
    );
    assert_eq!(
        formulas,
        BTreeMap::from([
            ("0E$1".to_string(), 8),
            ("0E$1+0.01".to_string(), 2),
            ("0E$1+0.1".to_string(), 2),
            ("0E($1+$2)/10".to_string(), 1),
        ]),
        "the corpus should hold these four formulas"
    );
    assert_eq!(
        (output_families, input_families),
        (
            BTreeMap::from([(0x0115, 13)]),
            BTreeMap::from([(0x00C7, 12), (0x0115, 2)])
        ),
        "the output operand should always be a JDim, and twelve inputs should be the \
         Double Value Objects"
    );
    assert_eq!(
        double_value_inputs.len(),
        12,
        "the twelve Double Value inputs are the twelve a JSymbolInformation long form lists"
    );
}

/// The four decoders for the symbol-information / expression family, run
/// over the streams that actually hold it.
///
/// These records live only in a `JSite<N>/PSMcluster0`, so this walks those
/// streams and asserts both the census and the links between the families:
/// every `Double Value` names a `Variables` group that lists it back, every
/// variable a `JSymbolInformation` names resolves to a `Double Value`
/// carrying the same double, and every relation input is one of those
/// values. Each decoder validates its own framing to the byte, so a count
/// that survives is a count of records that read cleanly end to end.
#[test]
fn symbol_information_family_decodes_across_fixtures() {
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::Read;

    use pid_parse::parsers::sheet_records::{
        decode_double_values, decode_standard_relations, decode_symbol_informations,
        decode_variables,
    };

    const CHAIN_MAGIC: u32 = 0x6C90_F544;

    fn u32_at(data: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
    }

    let mut values = 0usize;
    let mut groups = 0usize;
    let mut symbols = 0usize;
    let mut symbols_with_variables = 0usize;
    let mut variables = 0usize;
    let mut relations = 0usize;
    let mut names: BTreeMap<String, usize> = BTreeMap::new();
    let mut value_in_its_group = 0usize;
    let mut variable_matches_value = 0usize;
    let mut relation_inputs_that_are_values = 0usize;
    let mut any = false;

    for fixture in [
        "D06.pid",
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
    ] {
        let path = format!("test-file/{fixture}");
        if !std::path::Path::new(&path).exists() {
            continue;
        }
        let file = std::fs::File::open(&path).expect("fixture opens");
        let mut cfb = cfb::CompoundFile::open(file).expect("fixture is a compound file");
        let stream_paths: Vec<String> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|entry| entry.path().to_string_lossy().into_owned())
            .collect();
        for stream_path in stream_paths {
            if !stream_path.replace('\\', "/").ends_with("/PSMcluster0") {
                continue;
            }
            let mut data = Vec::new();
            let Ok(mut stream) = cfb.open_stream(&stream_path) else {
                continue;
            };
            if stream.read_to_end(&mut data).is_err() || u32_at(&data, 0) != Some(CHAIN_MAGIC) {
                continue;
            }
            any = true;

            let decoded_values = decode_double_values(&data);
            let decoded_groups = decode_variables(&data);
            let decoded_symbols = decode_symbol_informations(&data);
            let decoded_relations = decode_standard_relations(&data);

            let by_oid: BTreeMap<u32, f64> = decoded_values
                .iter()
                .map(|value| (value.oid, value.value))
                .collect();
            let group_members: BTreeMap<u32, BTreeSet<u32>> = decoded_groups
                .iter()
                .map(|group| (group.oid, group.members.iter().copied().collect()))
                .collect();

            values += decoded_values.len();
            groups += decoded_groups.len();
            symbols += decoded_symbols.len();
            relations += decoded_relations.len();

            for value in &decoded_values {
                if group_members
                    .get(&value.parent_ref)
                    .is_some_and(|members| members.contains(&value.oid))
                {
                    value_in_its_group += 1;
                }
            }
            for symbol in &decoded_symbols {
                if symbol.variables.is_empty() {
                    continue;
                }
                symbols_with_variables += 1;
                for variable in &symbol.variables {
                    variables += 1;
                    *names.entry(variable.name.clone()).or_default() += 1;
                    if by_oid.get(&variable.value_ref) == Some(&variable.value) {
                        variable_matches_value += 1;
                    }
                }
            }
            for relation in &decoded_relations {
                for input in relation.operands.iter().skip(1) {
                    if by_oid.contains_key(input) {
                        relation_inputs_that_are_values += 1;
                    }
                }
            }
        }
    }

    if !any {
        return;
    }

    assert_eq!(
        (values, groups, symbols, relations),
        (203, 76, 45, 13),
        "the four fixtures should decode 203 Double Value, 76 Variables, 45 \
         JSymbolInformation and 13 Standard Relation records"
    );
    assert_eq!(
        (symbols_with_variables, variables),
        (8, 24),
        "eight JSymbolInformation records should carry the corpus's 24 named variables"
    );
    assert_eq!(
        names,
        BTreeMap::from([
            ("Bottom".to_string(), 4),
            ("Left".to_string(), 6),
            ("Right".to_string(), 8),
            ("Top".to_string(), 6),
        ]),
        "the variables should only carry the four side names"
    );
    assert_eq!(
        value_in_its_group, values,
        "every Double Value's parent_ref should name a Variables group that lists it back"
    );
    assert_eq!(
        (variable_matches_value, relation_inputs_that_are_values),
        (12, 12),
        "the twelve variables whose value object is in the same storage should agree with \
         it, and those same twelve should be the relations' inputs"
    );

    // The same records, reached the way a consumer would: off the parsed
    // document's JSite surface rather than by walking the cluster.
    let mut surfaced = (0usize, 0usize, 0usize, 0usize);
    let mut sites_with_family = 0usize;
    for fixture in [
        "D06.pid",
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
    ] {
        let Some(doc) = parse_test_file(fixture) else {
            continue;
        };
        for site in &doc.jsites {
            let Some(family) = &site.symbol_information else {
                continue;
            };
            sites_with_family += 1;
            surfaced.0 += family.double_values.len();
            surfaced.1 += family.variable_groups.len();
            surfaced.2 += family.symbol_informations.len();
            surfaced.3 += family.relations.len();
        }
    }
    assert_eq!(
        surfaced,
        (values, groups, symbols, relations),
        "PidDocument's JSite surface should carry exactly what the decoders find"
    );
    assert_eq!(
        sites_with_family, 7,
        "the seven JSite storages with a PSMcluster0 all hold at least a JSymbolInformation; \
         the rest of the sites carry only JProperties"
    );
}

/// The curve records the nested sites hold, counted against the roster that
/// found them without a decoder.
///
/// `docs/analysis/2026-08-27-aux-hi-is-the-sheet-layer.md` grouped every
/// record of the four fixtures by its layer edge and found 12 `igCircle2d`
/// and 12 `igArc2d` sitting on layers, all inside nested `JSite<N>/PSMcluster0`
/// storages and none in a top-level `Sheet*` stream. The decoders read the
/// bytes the `imagdex.dex` `DoIO` workers read
/// (`docs/analysis/2026-08-31-imagdex-geometry-doio-ida.md`); if they find
/// the same 24 records, each on a layer the same storage declares, the two
/// readings agree. That closes the first half of the exit gate in
/// `docs/analysis/2026-08-31-jsite-geometry-coverage-gap.md` -- a count
/// ratchet per family -- and leaves the second half, the page transform, as
/// the only thing between these records and the drawing.
#[test]
fn nested_site_curves_decode_across_fixtures() {
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::Read;

    use pid_parse::parsers::sheet_layers::decode_sheet_layers;
    use pid_parse::parsers::sheet_records::{decode_igarcs, decode_igcircles};

    const CHAIN_MAGIC: u32 = 0x6C90_F544;

    fn u32_at(data: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
    }

    // (circles, arcs) per fixture, from the decoders walking the streams.
    let mut per_fixture: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    let mut on_a_declared_layer = 0usize;
    let mut on_no_layer = 0usize;
    let mut in_a_sheet_stream = 0usize;
    let mut any = false;

    for fixture in [
        "D06.pid",
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
    ] {
        let path = format!("test-file/{fixture}");
        if !std::path::Path::new(&path).exists() {
            continue;
        }
        let file = std::fs::File::open(&path).expect("fixture opens");
        let mut cfb = cfb::CompoundFile::open(file).expect("fixture is a compound file");
        let stream_paths: Vec<String> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|entry| entry.path().to_string_lossy().replace('\\', "/"))
            .collect();
        for stream_path in stream_paths {
            let leaf = stream_path.rsplit('/').next().unwrap_or_default();
            let is_cluster = leaf == "PSMcluster0";
            let is_sheet = leaf.starts_with("Sheet");
            if !is_cluster && !is_sheet {
                continue;
            }
            let mut data = Vec::new();
            let Ok(mut stream) = cfb.open_stream(&stream_path) else {
                continue;
            };
            if stream.read_to_end(&mut data).is_err() || u32_at(&data, 0) != Some(CHAIN_MAGIC) {
                continue;
            }
            any = true;
            let circles = decode_igcircles(&data);
            let arcs = decode_igarcs(&data);
            if is_sheet {
                in_a_sheet_stream += circles.len() + arcs.len();
                continue;
            }
            let layers: BTreeSet<u32> = decode_sheet_layers(&data)
                .into_iter()
                .map(|layer| layer.oid)
                .collect();
            for layer_ref in circles
                .iter()
                .map(|circle| circle.sheet_layer_ref)
                .chain(arcs.iter().map(|arc| arc.sheet_layer_ref))
            {
                if layers.contains(&layer_ref) {
                    on_a_declared_layer += 1;
                } else {
                    on_no_layer += 1;
                }
            }
            let entry = per_fixture.entry(fixture).or_default();
            entry.0 += circles.len();
            entry.1 += arcs.len();
        }
    }

    if !any {
        return;
    }

    let circles: usize = per_fixture.values().map(|(circles, _)| circles).sum();
    let arcs: usize = per_fixture.values().map(|(_, arcs)| arcs).sum();
    assert_eq!(
        (circles, arcs),
        (12, 12),
        "the decoders should find the 12 igCircle2d and 12 igArc2d the layer-edge roster \
         counted; per fixture: {per_fixture:?}"
    );
    assert_eq!(
        in_a_sheet_stream, 0,
        "no curve record should sit in a top-level Sheet* stream"
    );
    assert_eq!(
        (on_a_declared_layer, on_no_layer),
        (24, 0),
        "every curve record should name a JSheetLayer its own storage declares"
    );

    // The same records, reached the way a consumer would: off the parsed
    // document's JSite surface rather than by walking the cluster.
    let mut surfaced: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for fixture in [
        "D06.pid",
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
    ] {
        let Some(doc) = parse_test_file(fixture) else {
            continue;
        };
        let entry = surfaced.entry(fixture).or_default();
        for site in &doc.jsites {
            let Some(curves) = &site.nested_geometry else {
                continue;
            };
            assert!(
                !curves.is_empty(),
                "{fixture} {}: an empty nested_geometry should have been left as None",
                site.path
            );
            entry.0 += curves.circles.len();
            entry.1 += curves.arcs.len();
        }
        // And the geometry projection names every held-back site.
        let geometry = pid_parse::build_normalized_geometry(&doc);
        for site in doc
            .jsites
            .iter()
            .filter(|site| site.nested_geometry.is_some())
        {
            assert!(
                geometry.warnings.iter().any(|warning| {
                    warning.contains(&format!("{}/PSMcluster0", site.path))
                        && warning.contains("symbol bodies in symbol-local coordinates")
                }),
                "{fixture}: the bodies in {} are not named in any warning: {:?}",
                site.path,
                geometry.warnings
            );
        }
    }
    assert_eq!(
        surfaced, per_fixture,
        "PidDocument's JSite surface should carry exactly what the decoders find"
    );
}

/// Every placement names the body it draws, and the drawing carries that body.
///
/// The last two words of an `igSymbol2d` payload are `(JSheet oid, LdcSite
/// id)`: the definition cache storage and the sheet inside it that is this
/// symbol's body. The sheet's tag-183 space-map edge names a layer manager,
/// the manager's layers carry the body's records, and the placement's own
/// matrix and insertion put them on the page
/// (`docs/analysis/2026-09-07-placement-tail-names-the-cached-definition.md`).
/// Ground truth is the `.sym` library: where a body has circles or arcs, they
/// equal the placed symbol's library body to a nanometre -- except for the
/// parametric manifold, whose cached body is the resized instance the
/// library only has the default of.
#[test]
fn every_placement_names_a_body_the_drawing_carries() {
    use std::collections::BTreeMap;

    use pid_parse::symbol_library::SymbolPrimitive;
    use pid_parse::{PidGeometryConfidence, PidGraphicKind};

    // (placements, placements with a resolved body, distinct bodies named,
    // bodies the caches carry) per fixture.
    let mut tally: BTreeMap<&str, (usize, usize, usize, usize)> = BTreeMap::new();
    let mut any = false;

    let radii_of = |primitives: &[SymbolPrimitive]| -> Vec<u32> {
        let mut radii: Vec<u32> = primitives
            .iter()
            .filter_map(|p| match p {
                SymbolPrimitive::Circle { radius, .. } | SymbolPrimitive::Arc { radius, .. } => {
                    Some((radius * 1e5).round() as u32)
                }
                _ => None,
            })
            .collect();
        radii.sort_unstable();
        radii
    };

    for fixture in [
        "D06.pid",
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
    ] {
        let Some(doc) = parse_test_file(fixture) else {
            continue;
        };
        any = true;
        let geometry = pid_parse::build_normalized_geometry(&doc);
        let mut placements = 0usize;
        let mut resolved = 0usize;
        let mut named: std::collections::BTreeSet<pid_parse::PidSymbolDefinitionRef> =
            Default::default();
        for entity in &geometry.entities {
            if entity.confidence != PidGeometryConfidence::Decoded {
                continue;
            }
            let PidGraphicKind::SymbolInstance {
                symbol_path,
                definition,
                ..
            } = &entity.kind
            else {
                continue;
            };
            placements += 1;
            let Some(reference) = definition else {
                continue;
            };
            resolved += 1;
            named.insert(*reference);
            let body = geometry
                .symbol_definition(*reference)
                .unwrap_or_else(|| panic!("{fixture}: {reference:?} is named but not carried"));
            assert!(
                !body.layers.is_empty(),
                "{fixture}: {reference:?} resolved to a manager with no layers"
            );
            let name = symbol_path
                .as_deref()
                .and_then(|p| p.rsplit(['\\', '/']).next())
                .unwrap_or_default();
            // The bodies whose curves the library pins, in 10 µm units.
            let expected: Option<Vec<u32>> = match (fixture, name) {
                ("D06.pid", "PT-Pressure Transmitter.sym") => Some(vec![635, 757]),
                ("D06.pid", "Ball Valve Type 1.sym") => Some(vec![127]),
                ("D06.pid", "2 Way Ball Type 1.sym") => Some(vec![159]),
                ("DWG-0201GP06-01.pid", "LG-Magnetic Float Gauge.sym") => Some(vec![635, 757]),
                ("DWG-0201GP06-01.pid", "Ball Valve Type 2.sym") => Some(vec![127]),
                // The resized instance, not the 20.32 mm library default.
                ("DWG-0201GP06-01.pid", "Parametric Manifold.sym") => Some(vec![3559, 3559]),
                ("DWG-0202GP06-01.pid", "ElecTraceLine.sym") => Some(vec![162, 162]),
                ("DWG-0202GP06-01.pid", "DCS Field Mounted.sym") => Some(vec![635]),
                _ => None,
            };
            if let Some(expected) = expected {
                assert_eq!(
                    radii_of(&body.primitives),
                    expected,
                    "{fixture}: the cached body of {name} should carry the library's curves"
                );
            }
        }
        tally.insert(
            fixture,
            (
                placements,
                resolved,
                named.len(),
                geometry.symbol_definitions.len(),
            ),
        );
    }

    if !any {
        return;
    }
    assert_eq!(
        tally,
        BTreeMap::from([
            ("D06.pid", (6, 6, 6, 9)),
            ("DWG-0201GP06-01.pid", (20, 20, 17, 21)),
            ("DWG-0202GP06-01.pid", (23, 23, 11, 12)),
            ("工艺管道及仪表流程-1.pid", (58, 58, 7, 10)),
        ]),
        "(placements, resolved, distinct bodies named, bodies carried) per fixture"
    );
}

/// The corpus's three rectangles are parents of four edge lines each, and its
/// one B-spline is a leaf that reaches the drawing as part of a symbol body.
///
/// A rectangle's five doubles read `(origin, width, rotation, height / width)`
/// -- the A2 border of the `A01` export is `0.594 x 0.707071 = 594 x 420 mm`
/// -- and its tail lists four oids that are `igLine2d` records of the same
/// stream whose endpoints are the rectangle's corners. So the rectangle
/// emits nothing: its edges already do. The B-spline sits in the cached body
/// of `arrester breather valve(RD)` and in that symbol's `.sym`, pole for
/// pole, and both readers now carry it
/// (`docs/analysis/2026-09-07-rectangle-owns-its-edges-bspline-is-a-leaf.md`).
#[test]
fn rectangles_own_their_edges_and_the_bspline_reaches_its_body() {
    use pid_parse::symbol_library::{read_symbol_geometry, SymbolPrimitive};
    use pid_parse::PidGraphicKind;

    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
    let mut rectangles_checked = 0usize;

    for fixture in [
        "DWG-0202GP06-01.pid",
        "export-test/publish-data/A01/A01.pid",
    ] {
        let Some(doc) = parse_test_file(fixture) else {
            continue;
        };
        for sheet in &doc.sheet_streams {
            let Some(geometry) = sheet.geometry.as_ref() else {
                continue;
            };
            for rectangle in &geometry.decoded_igrectangles {
                assert_eq!(
                    rectangle.edges.len(),
                    4,
                    "{fixture} {}: a rectangle lists its four edges",
                    sheet.path
                );
                assert_eq!(rectangle.rotation, 0.0);
                // Every listed edge is a line of the same stream, and the
                // four lines' endpoints are exactly the rectangle's corners.
                let (x0, y0) = (rectangle.origin_x, rectangle.origin_y);
                let (x1, y1) = (x0 + rectangle.width, y0 + rectangle.height());
                let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
                for edge in &rectangle.edges {
                    let line = geometry
                        .decoded_iglines
                        .iter()
                        .find(|line| line.oid == *edge)
                        .unwrap_or_else(|| {
                            panic!(
                                "{fixture} {}: edge {edge} is not a line of the stream",
                                sheet.path
                            )
                        });
                    for point in [(line.start_x, line.start_y), (line.end_x, line.end_y)] {
                        assert!(
                            corners.iter().any(|c| close(c.0, point.0) && close(c.1, point.1)),
                            "{fixture} {}: edge {edge} endpoint {point:?} is not a corner of {corners:?}",
                            sheet.path
                        );
                    }
                }
                rectangles_checked += 1;
            }
        }
        // And the projection emits the edges, not the rectangle: no entity
        // of the stream has the rectangle's oid.
        let projection = pid_parse::build_normalized_geometry(&doc);
        for sheet in &doc.sheet_streams {
            let Some(geometry) = sheet.geometry.as_ref() else {
                continue;
            };
            for rectangle in &geometry.decoded_igrectangles {
                assert!(
                    !projection
                        .entities
                        .iter()
                        .any(|entity| entity.graphic_oid == Some(rectangle.oid)),
                    "{fixture}: rectangle {} was emitted on top of its edges",
                    rectangle.oid
                );
            }
        }
        if fixture == "DWG-0202GP06-01.pid" {
            assert!(
                projection.dropped_graphic_records.is_empty(),
                "the /Sheet6615 rectangle was the last graphic record without a decoder"
            );
            assert_eq!(
                projection
                    .entities
                    .iter()
                    .filter(|e| e.source.stream_path.as_deref() == Some("/Sheet6615"))
                    .filter(|e| e.confidence == pid_parse::PidGeometryConfidence::Decoded)
                    .filter(|e| matches!(e.kind, PidGraphicKind::Line { .. }))
                    .count(),
                4,
                "the orphan storage's decoded line work is exactly the four edge lines"
            );
        }
    }
    if rectangles_checked > 0 {
        assert_eq!(
            rectangles_checked, 3,
            "three rectangles across DWG-0202 and A01, all checked"
        );
    }

    // The B-spline: one, in the cached body of the arrester breather valve.
    let Some(doc) = parse_test_file("DWG-0202GP06-01.pid") else {
        return;
    };
    let cache = doc
        .jsites
        .iter()
        .find(|site| site.path == "/JSite793")
        .and_then(|site| site.nested_geometry.as_ref())
        .expect("DWG-0202's definition cache");
    assert_eq!(cache.bsplines.len(), 1);
    let curve = &cache.bsplines[0];
    assert_eq!(
        (curve.pole_xs.len(), curve.knots.len(), curve.degree()),
        (5, 9, 3)
    );
    assert!(curve.weights.is_empty(), "the corpus curve is polynomial");
    assert_eq!(curve.sheet_layer_ref, 2824);

    let projection = pid_parse::build_normalized_geometry(&doc);
    let body = projection
        .entities
        .iter()
        .find_map(|entity| match &entity.kind {
            PidGraphicKind::SymbolInstance {
                symbol_path: Some(path),
                definition: Some(definition),
                ..
            } if path.ends_with("arrester breather valve(RD).sym") => {
                projection.symbol_definition(*definition)
            }
            _ => None,
        })
        .expect("the arrester breather valve placement resolves its cached body");
    let cached: Vec<&SymbolPrimitive> = body
        .primitives
        .iter()
        .filter(|p| matches!(p, SymbolPrimitive::BSpline { .. }))
        .collect();
    assert_eq!(cached.len(), 1, "the cached body carries the curve");
    let points = cached[0].bspline_points(8);
    assert_eq!(
        points.len(),
        17,
        "two knot spans of eight segments plus the end"
    );
    // Clamped at both ends: the curve starts and ends on its end poles.
    let poles = curve.poles();
    assert!(close(points[0].0, poles[0].0) && close(points[0].1, poles[0].1));
    assert!(close(points[16].0, poles[4].0) && close(points[16].1, poles[4].1));

    // The library's copy of the same symbol carries the same curve -- to a
    // femtometre, not to the bit: one pole differs by two ulps (2e-18 m), so
    // the cache is a re-serialised copy that went through arithmetic once,
    // not a byte copy of the .sym record.
    let sym = std::path::Path::new(
        "test-file/symbols-full/Piping/Valves/2 Way Other/arrester breather valve(RD).sym",
    );
    if sym.exists() {
        let library = read_symbol_geometry(sym).expect("the .sym reads");
        let from_library: Vec<&SymbolPrimitive> = library
            .primitives
            .iter()
            .map(|styled| &styled.primitive)
            .filter(|p| matches!(p, SymbolPrimitive::BSpline { .. }))
            .collect();
        assert_eq!(from_library.len(), 1, "the .sym carries one B-spline");
        let (
            SymbolPrimitive::BSpline {
                poles: library_poles,
                weights: library_weights,
                knots: library_knots,
            },
            SymbolPrimitive::BSpline {
                poles: cached_poles,
                weights: cached_weights,
                knots: cached_knots,
            },
        ) = (from_library[0], cached[0])
        else {
            unreachable!("both filtered to B-splines");
        };
        let within = |a: &[f64], b: &[f64]| {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-15)
        };
        assert_eq!(library_poles.len(), cached_poles.len(), "same pole count");
        for (index, (library_pole, cached_pole)) in
            library_poles.iter().zip(cached_poles).enumerate()
        {
            assert!(
                (library_pole.0 - cached_pole.0).abs() < 1e-15
                    && (library_pole.1 - cached_pole.1).abs() < 1e-15,
                "pole {index}: library {library_pole:?} vs cached {cached_pole:?}"
            );
        }
        assert!(within(library_weights, cached_weights), "same weights");
        assert!(within(library_knots, cached_knots), "same knots");
    }
}

/// Ratchet for plan J2 (`OpenCADStudio/docs/plans/2026-09-07-jdim-driving-dimensions-and-layer-panel.md`):
/// the `0x0115` `JDim` records are the driving dimensions of the parametric
/// symbol bodies, and the definition cache now carries them instead of
/// stepping over them in silence.
///
/// Corpus: 18 records — D06 5 / DWG-0201 5 / DWG-0202 0 / 工艺 4, plus 4 in
/// the A01 export (soft-skipped with its fixture). Every one sits in a
/// `/JSite<N>/PSMcluster0` on a layer named `Dimension`, its `parent_ref` is
/// a `JSheet` of that storage (a body some placement names, or the template
/// body no placement does, like `/JSite329`'s sheet 49), its value is a
/// multiple of 0.05 inch, and the geometry it measures is a line or point
/// of the same storage
/// (`docs/analysis/2026-09-14-jdim-is-a-framed-record-whose-blocks-follow-the-dimension-kind.md`,
/// `docs/analysis/2026-09-15-tag-188-members-land-in-jdim-reference-slots.md`).
/// Nothing is drawn: the family emits no entity, so the golden snapshot is
/// untouched by design.
#[test]
fn jdims_are_the_driving_dimensions_of_parametric_bodies() {
    const EXPECTED: &[(&str, usize)] = &[
        ("D06.pid", 5),
        ("DWG-0201GP06-01.pid", 5),
        ("DWG-0202GP06-01.pid", 0),
        ("工艺管道及仪表流程-1.pid", 4),
        ("export-test/publish-data/A01/A01.pid", 4),
    ];
    const INCH_M: f64 = 0.0254;

    for (fixture, expected) in EXPECTED {
        let Some(doc) = parse_test_file(fixture) else {
            continue;
        };

        let mut seen = 0usize;
        for site in &doc.jsites {
            let Some(nested) = site.nested_geometry.as_ref() else {
                continue;
            };
            let layers = doc.sheet_layers.get(&site.path);
            let line_oids: std::collections::BTreeSet<u32> =
                nested.lines.iter().map(|line| line.oid).collect();
            for dimension in &nested.dimensions {
                seen += 1;
                assert_eq!(
                    dimension.kind, 1,
                    "{fixture} {}: only linear decodes",
                    site.path
                );
                assert!(
                    nested.sheets.contains(&dimension.parent_ref),
                    "{fixture} {}: JDim {} hangs off sheet {}, which is not a JSheet of the storage {:?}",
                    site.path,
                    dimension.oid,
                    dimension.parent_ref,
                    nested.sheets
                );
                if let Some(layers) = layers {
                    let layer = layers
                        .iter()
                        .find(|layer| layer.oid == dimension.sheet_layer_ref)
                        .unwrap_or_else(|| {
                            panic!(
                                "{fixture} {}: JDim {} sits on layer {}, unknown to the storage",
                                site.path, dimension.oid, dimension.sheet_layer_ref
                            )
                        });
                    assert_eq!(
                        layer.name, "Dimension",
                        "{fixture} {}: JDim {} is on layer {:?}",
                        site.path, dimension.oid, layer.name
                    );
                }
                // A whole number of twentieths of an inch: 0.15″ … 4.5″ on
                // the corpus, and the 35.56 mm on D06 is the Parametric
                // Manifold cache's 35.59 mm rounded.
                let twentieths = dimension.value_m / INCH_M * 20.0;
                assert!(
                    (twentieths - twentieths.round()).abs() < 1e-6 && twentieths > 0.0,
                    "{fixture} {}: JDim {} value {} m is not a multiple of 0.05 inch",
                    site.path,
                    dimension.oid,
                    dimension.value_m
                );
                // The measured slot's marker tells the class; the corpus only
                // ever writes a line (0x00CB) or a point (0x00F0) there, and a
                // line is a record of the same storage this crate carries.
                match dimension.measured_marker {
                    0x00CB => assert!(
                        line_oids.contains(&dimension.measured_oid),
                        "{fixture} {}: JDim {} measures line {}, not a line of the storage",
                        site.path,
                        dimension.oid,
                        dimension.measured_oid
                    ),
                    0x00F0 => {}
                    other => panic!(
                        "{fixture} {}: JDim {} has an unread measured marker 0x{other:04X}",
                        site.path, dimension.oid
                    ),
                }
                assert!(
                    dimension.raw_tail.len() + 82 == 34 + dimension.main_len as usize,
                    "{fixture} {}: raw tail must span +82 .. 34 + main_len",
                    site.path
                );
            }
        }
        assert_eq!(
            seen, *expected,
            "{fixture}: driving dimensions decoded out of the definition caches"
        );

        // The projection files each dimension under exactly one body, with
        // the measured line resolved to its endpoints, and draws none of them.
        let projection = pid_parse::build_normalized_geometry(&doc);
        let filed: usize = projection
            .symbol_definitions
            .iter()
            .map(|body| body.dimensions.len())
            .sum();
        assert_eq!(
            filed, *expected,
            "{fixture}: every dimension belongs to one definition and none to two"
        );
        for body in &projection.symbol_definitions {
            for dimension in &body.dimensions {
                assert!(
                    body.layers.contains(&dimension.sheet_layer_ref),
                    "{fixture}: dimension {} filed under a body whose layers do not hold it",
                    dimension.oid
                );
                let nested = doc
                    .jsites
                    .iter()
                    .find(|site| site.path == format!("/JSite{}", body.reference.site))
                    .and_then(|site| site.nested_geometry.as_ref())
                    .expect("the body's storage");
                let measures_a_line = nested
                    .lines
                    .iter()
                    .any(|line| line.oid == dimension.measured_oid);
                assert_eq!(
                    dimension.endpoints.is_some(),
                    measures_a_line,
                    "{fixture}: dimension {} resolves its endpoints exactly when it measures a line",
                    dimension.oid
                );
            }
        }
        assert!(
            !projection.entities.iter().any(|entity| {
                entity
                    .source
                    .stream_path
                    .as_deref()
                    .is_some_and(|p| p.contains("PSMcluster0"))
            }),
            "{fixture}: nothing from a definition cache is page content"
        );
        assert!(
            projection.warnings.iter().any(|warning| warning
                .contains(&format!("{expected} dimensions"))
                || *expected == 0),
            "{fixture}: the cache summary counts the dimensions: {:?}",
            projection.warnings
        );
    }
}

/// Evaluate a `Standard Relation` formula (`0E$1`, `0E$1+0.01`,
/// `0E($1+$2)/10`, `0E$1/2` …) over its inputs, in whatever unit the inputs
/// are handed in. Mirrors the evaluator of
/// `examples/probe_parametric_chain_resolves_a_cached_body.rs`; kept out of
/// `src` on purpose — reading the formula is evidence, not a decoder.
fn eval_relation_formula(formula: &str, inputs: &[f64]) -> Option<f64> {
    struct P<'a> {
        s: &'a [u8],
        i: usize,
        inputs: &'a [f64],
    }
    impl P<'_> {
        fn peek(&self) -> Option<u8> {
            self.s.get(self.i).copied()
        }
        fn expr(&mut self) -> Option<f64> {
            let mut v = self.term()?;
            while let Some(op @ (b'+' | b'-')) = self.peek() {
                self.i += 1;
                let r = self.term()?;
                v = if op == b'+' { v + r } else { v - r };
            }
            Some(v)
        }
        fn term(&mut self) -> Option<f64> {
            let mut v = self.factor()?;
            while let Some(op @ (b'*' | b'/')) = self.peek() {
                self.i += 1;
                let r = self.factor()?;
                v = if op == b'*' { v * r } else { v / r };
            }
            Some(v)
        }
        fn factor(&mut self) -> Option<f64> {
            match self.peek()? {
                b'(' => {
                    self.i += 1;
                    let v = self.expr()?;
                    (self.peek()? == b')').then(|| self.i += 1)?;
                    Some(v)
                }
                b'-' => {
                    self.i += 1;
                    Some(-self.factor()?)
                }
                b'$' => {
                    self.i += 1;
                    let start = self.i;
                    while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                        self.i += 1;
                    }
                    let n: usize = std::str::from_utf8(&self.s[start..self.i])
                        .ok()?
                        .parse()
                        .ok()?;
                    self.inputs.get(n.checked_sub(1)?).copied()
                }
                _ => {
                    let start = self.i;
                    while self.peek().is_some_and(|c| c.is_ascii_digit() || c == b'.') {
                        self.i += 1;
                    }
                    std::str::from_utf8(&self.s[start..self.i])
                        .ok()?
                        .parse()
                        .ok()
                }
            }
        }
    }
    let body = formula.strip_prefix("0E")?;
    let mut p = P {
        s: body.as_bytes(),
        i: 0,
        inputs,
    };
    let v = p.expr()?;
    (p.i == body.len()).then_some(v)
}

/// Ratchet for plan J3 (`OpenCADStudio/docs/plans/2026-09-07-jdim-driving-dimensions-and-layer-panel.md`):
/// the parametric chain `SymbolInformation variable -> Double Value ->
/// Standard Relation formula -> JDim` closes, and it closes on the
/// **template** body — the plan's premise that a placed instance carries
/// its own dimension values does not hold.
///
/// What the corpus says
/// (`docs/analysis/2026-09-18-the-parametric-chain-closes-on-the-template-not-the-instance.md`):
///
/// * every Standard Relation's output is a `JDim` of the same storage, and
///   the formula reproduces the stored value **with its constants read in
///   inches** — 17/17 across the corpus; the four D06 relations with a
///   constant (`$1+0.01`, `$1+0.1`) are what settle the unit, since they
///   miss by millimetres when read in metres;
/// * every body that carries a dimension is one no placement names — the
///   library-default template in the `Server Document` storage;
/// * every placed parametric instance (`Imagineer Document` storage) has
///   no `JDim`, no relation, no `Double Value`: one `SymbolInformation`
///   whose variables repeat the template's values, and baked geometry;
/// * `DWG-0201`'s Manifold: the template's two arcs have the radius of its
///   `Top` dimension (20.32 mm) and are centred where the lines its
///   `Left` / `Right` dimensions measure begin; the instance's two arcs are
///   35.59 mm, a value no dimension in the file holds, while its
///   `SymbolInformation` still says `Top = 20.32`;
/// * `D06`'s Cone Roof Tank instance is the template's formulas evaluated
///   with the constants read in **millimetres** (half-width 60.96 + 0.1),
///   so the same relation was run twice in two units.
#[test]
fn the_parametric_chain_closes_on_the_template_not_on_the_placed_instance() {
    const INCH_M: f64 = 0.0254;
    // (fixture, relations, relations that close only in inches)
    const EXPECTED: &[(&str, usize, usize)] = &[
        ("D06.pid", 5, 4),
        ("DWG-0201GP06-01.pid", 4, 0),
        ("DWG-0202GP06-01.pid", 0, 0),
        ("工艺管道及仪表流程-1.pid", 4, 0),
        ("export-test/publish-data/A01/A01.pid", 4, 0),
    ];
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;

    for (fixture, expected_relations, expected_inch_only) in EXPECTED {
        let Some(doc) = parse_test_file(fixture) else {
            continue;
        };
        let placed: std::collections::BTreeSet<(u32, u32)> = doc
            .sheet_streams
            .iter()
            .filter_map(|sheet| sheet.geometry.as_ref())
            .flat_map(|geometry| {
                geometry
                    .decoded_igsymbols
                    .iter()
                    .map(|p| (p.definition_site_ref, p.definition_sheet_ref))
            })
            .collect();
        let site_id = |site: &pid_parse::model::JSite| -> u32 {
            site.name
                .strip_prefix("JSite")
                .and_then(|id| id.parse().ok())
                .unwrap_or_else(|| panic!("{fixture}: {} is not a JSite<N>", site.name))
        };

        // 1. Every relation writes a JDim of its own storage, and the
        //    formula reproduces the stored value when read in inches.
        let mut relations = 0usize;
        let mut inch_only = 0usize;
        for site in &doc.jsites {
            let Some(info) = site.symbol_information.as_ref() else {
                continue;
            };
            if info.relations.is_empty() {
                continue;
            }
            let nested = site
                .nested_geometry
                .as_ref()
                .unwrap_or_else(|| panic!("{fixture} {}: relations but no cache body", site.path));
            let value_of = |oid: u32| -> f64 {
                info.double_values
                    .iter()
                    .find(|d| d.oid == oid)
                    .map(|d| d.value)
                    .or_else(|| {
                        nested
                            .dimensions
                            .iter()
                            .find(|d| d.oid == oid)
                            .map(|d| d.value_m)
                    })
                    .unwrap_or_else(|| {
                        panic!(
                            "{fixture} {}: operand {oid} is neither a Double Value nor a JDim",
                            site.path
                        )
                    })
            };
            for relation in &info.relations {
                let (out, ins) = relation.operands.split_first().unwrap_or_else(|| {
                    panic!("{fixture}: relation {} has no operands", relation.oid)
                });
                let jdim = nested
                    .dimensions
                    .iter()
                    .find(|d| d.oid == *out)
                    .unwrap_or_else(|| {
                        panic!(
                            "{fixture} {}: relation {} writes {out}, which is not a JDim",
                            site.path, relation.oid
                        )
                    });
                let inputs: Vec<f64> = ins.iter().map(|oid| value_of(*oid)).collect();
                let in_inches = eval_relation_formula(
                    &relation.formula,
                    &inputs.iter().map(|v| v / INCH_M).collect::<Vec<_>>(),
                )
                .map(|v| v * INCH_M)
                .unwrap_or_else(|| {
                    panic!("{fixture}: formula {:?} does not parse", relation.formula)
                });
                assert!(
                    close(in_inches, jdim.value_m),
                    "{fixture} {} relation {} {:?}: inputs {inputs:?} m give {in_inches} m in inches, the JDim holds {}",
                    site.path,
                    relation.oid,
                    relation.formula,
                    jdim.value_m
                );
                let in_metres =
                    eval_relation_formula(&relation.formula, &inputs).expect("parsed once already");
                if !close(in_metres, jdim.value_m) {
                    inch_only += 1;
                }
                relations += 1;
            }
        }
        assert_eq!(
            relations, *expected_relations,
            "{fixture}: Standard Relations"
        );
        assert_eq!(
            inch_only, *expected_inch_only,
            "{fixture}: relations whose constant only makes sense in inches"
        );

        // 2. A body with a dimension is one no placement names; a placed
        //    parametric instance carries no dimension, no relation, and a
        //    SymbolInformation that repeats its template's values.
        for site in &doc.jsites {
            let Some(nested) = site.nested_geometry.as_ref() else {
                continue;
            };
            for definition in &nested.definitions {
                let on_body = |layer: u32| definition.layers.binary_search(&layer).is_ok();
                if nested.dimensions.iter().any(|d| on_body(d.sheet_layer_ref)) {
                    assert!(
                        !placed.contains(&(site_id(site), definition.sheet_oid)),
                        "{fixture} {} sheet {}: a body with dimensions is placed",
                        site.path,
                        definition.sheet_oid
                    );
                }
            }
            let Some(info) = site.symbol_information.as_ref() else {
                continue;
            };
            if !info.relations.is_empty() {
                continue;
            }
            for record in info
                .symbol_informations
                .iter()
                .filter(|r| !r.variables.is_empty())
            {
                assert!(
                    nested.dimensions.is_empty() && info.double_values.is_empty(),
                    "{fixture} {}: a placed parametric instance carries dimensions or values of its own",
                    site.path
                );
                let template = doc.jsites.iter().find(|other| {
                    other.path != site.path
                        && other.symbol_information.as_ref().is_some_and(|other_info| {
                            !other_info.relations.is_empty()
                                && other_info.symbol_informations.iter().any(|t| {
                                    t.variables.len() == record.variables.len()
                                        && t.variables.iter().all(|tv| {
                                            record.variables.iter().any(|iv| {
                                                iv.name == tv.name && close(iv.value, tv.value)
                                            })
                                        })
                                })
                        })
                });
                assert!(
                    template.is_some(),
                    "{fixture} {} SymbolInformation {}: no template storage names the same variables with the same values",
                    site.path,
                    record.oid
                );
            }
        }
    }

    // 3. DWG-0201's Parametric Manifold: template and instance side by side.
    if let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") {
        let cache = |path: &str| {
            doc.jsites
                .iter()
                .find(|site| site.path == path)
                .unwrap_or_else(|| panic!("{path} missing"))
        };
        let template = cache("/JSite329");
        let template_body = template.nested_geometry.as_ref().expect("template cache");
        let template_info = template
            .symbol_information
            .as_ref()
            .expect("template chain");
        let sheet_49 = template_body.definition(49).expect("the Manifold template");
        let on_49 = |layer: u32| sheet_49.layers.binary_search(&layer).is_ok();
        let top = template_info
            .symbol_informations
            .iter()
            .flat_map(|r| r.variables.iter())
            .find(|v| v.name == "Top" && close(v.value, 0.02032))
            .expect("the template's Top variable");
        let jdim_36 = template_body
            .dimensions
            .iter()
            .find(|d| d.oid == 36)
            .expect("JDim 36");
        assert!(
            close(jdim_36.value_m, top.value),
            "JDim 36 is the Top variable"
        );
        let arcs: Vec<_> = template_body
            .arcs
            .iter()
            .filter(|a| on_49(a.sheet_layer_ref))
            .collect();
        assert_eq!(arcs.len(), 2, "the template Manifold has two end arcs");
        for arc in &arcs {
            assert!(
                close(arc.radius, jdim_36.value_m),
                "template arc {} radius {} is the Top dimension {}",
                arc.oid,
                arc.radius,
                jdim_36.value_m
            );
        }
        // The Left / Right dimensions measure the stub lines that start at
        // the arc centres and run to the body's outer ends; their value is
        // the distance from the symbol axis (where JDim 36's line stands)
        // to that outer end.
        let axis_x = template_body
            .lines
            .iter()
            .find(|l| l.oid == jdim_36.measured_oid)
            .map(|l| l.start_x)
            .expect("JDim 36 measures a line of the template");
        for (jdim_oid, arc_index) in [(38u32, 1usize), (40, 0)] {
            let jdim = template_body
                .dimensions
                .iter()
                .find(|d| d.oid == jdim_oid)
                .unwrap_or_else(|| panic!("JDim {jdim_oid}"));
            let line = template_body
                .lines
                .iter()
                .find(|l| l.oid == jdim.measured_oid)
                .unwrap_or_else(|| panic!("JDim {jdim_oid} measures a line"));
            let arc = arcs
                .iter()
                .find(|a| close(a.center_x, line.start_x) && close(a.center_y, line.start_y))
                .unwrap_or_else(|| {
                    panic!("JDim {jdim_oid}'s line starts at no arc centre (arc index {arc_index})")
                });
            assert!(close(arc.center_y, line.end_y));
            assert!(
                close((line.end_x - axis_x).abs(), jdim.value_m),
                "JDim {jdim_oid} = {} is the axis-to-end distance {}",
                jdim.value_m,
                (line.end_x - axis_x).abs()
            );
        }

        let instance = cache("/JSite396");
        let instance_body = instance.nested_geometry.as_ref().expect("instance cache");
        let instance_info = instance.symbol_information.as_ref().expect("instance copy");
        assert!(
            instance_body.dimensions.is_empty(),
            "the placed Manifold has no JDim"
        );
        assert!(instance_info.relations.is_empty() && instance_info.double_values.is_empty());
        let sheet_113 = instance_body
            .definition(113)
            .expect("the placed Manifold body");
        let on_113 = |layer: u32| sheet_113.layers.binary_search(&layer).is_ok();
        let instance_arcs: Vec<_> = instance_body
            .arcs
            .iter()
            .filter(|a| on_113(a.sheet_layer_ref))
            .collect();
        assert_eq!(instance_arcs.len(), 2);
        for arc in &instance_arcs {
            assert!(
                (arc.radius - 0.03559).abs() < 1e-5,
                "instance arc {} r {} is the 35.59 mm of 08-31",
                arc.oid,
                arc.radius
            );
            let any_dimension_holds_it = doc.jsites.iter().any(|site| {
                site.nested_geometry.as_ref().is_some_and(|nested| {
                    nested
                        .dimensions
                        .iter()
                        .any(|d| (d.value_m - arc.radius).abs() < 1e-6)
                })
            });
            assert!(
                !any_dimension_holds_it,
                "no dimension in the file holds the instance radius {}",
                arc.radius
            );
        }
        let copied_top = instance_info
            .symbol_informations
            .iter()
            .flat_map(|r| r.variables.iter())
            .find(|v| v.name == "Top")
            .expect("the instance copy names Top");
        assert!(
            close(copied_top.value, top.value),
            "the instance's SymbolInformation still carries the template's Top {}",
            top.value
        );
        // The ` Line2` beside it, whose formula has no constant: template
        // sheet 501 and placed sheet 119 are the same body to the metre.
        let extent = |nested: &pid_parse::model::JSiteNestedGeometry, sheet: u32| {
            let definition = nested
                .definition(sheet)
                .unwrap_or_else(|| panic!("sheet {sheet}"));
            let on = |layer: u32| definition.layers.binary_search(&layer).is_ok();
            nested
                .lines
                .iter()
                .filter(|l| on(l.sheet_layer_ref))
                .flat_map(|l| [(l.start_x, l.start_y), (l.end_x, l.end_y)])
                .fold(
                    (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
                    |(x0, y0, x1, y1), (x, y)| (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                )
        };
        let (t, i) = (extent(template_body, 501), extent(instance_body, 119));
        assert!(
            close(t.0, i.0) && close(t.1, i.1) && close(t.2, i.2) && close(t.3, i.3),
            "Line2: template {t:?} vs instance {i:?}"
        );
    }

    // 4. D06's Cone Roof Parametric Tank: the instance is the template's
    //    formulas run again with the constants read in millimetres.
    if let Some(doc) = parse_test_file("D06.pid") {
        let template = doc
            .jsites
            .iter()
            .find(|site| site.path == "/JSite145")
            .and_then(|site| site.nested_geometry.as_ref())
            .expect("D06 template cache");
        let instance = doc
            .jsites
            .iter()
            .find(|site| site.path == "/JSite151")
            .and_then(|site| site.nested_geometry.as_ref())
            .expect("D06 instance cache");
        let left = doc
            .jsites
            .iter()
            .find(|site| site.path == "/JSite145")
            .and_then(|site| site.symbol_information.as_ref())
            .and_then(|info| {
                info.symbol_informations
                    .iter()
                    .flat_map(|r| r.variables.iter())
                    .find(|v| v.name == "Left")
            })
            .expect("the template's Left variable")
            .value;
        let half_width = |nested: &pid_parse::model::JSiteNestedGeometry, sheet: u32| {
            let definition = nested
                .definition(sheet)
                .unwrap_or_else(|| panic!("sheet {sheet}"));
            let on = |layer: u32| definition.layers.binary_search(&layer).is_ok();
            let (x0, x1) = nested
                .lines
                .iter()
                .filter(|l| on(l.sheet_layer_ref))
                .flat_map(|l| [l.start_x, l.end_x])
                .fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
            (x1 - x0) / 2.0
        };
        // Template: Left + 0.1 inch = 63.5 mm, which is JDim 20 / 21.
        let template_half = half_width(template, 15);
        assert!(
            close(template_half, left + 0.1 * INCH_M),
            "template half-width {template_half}"
        );
        for oid in [20u32, 21] {
            let jdim = template
                .dimensions
                .iter()
                .find(|d| d.oid == oid)
                .unwrap_or_else(|| panic!("JDim {oid}"));
            assert!(close(jdim.value_m, template_half));
        }
        // Instance: Left + 0.1 millimetre = 61.06 mm, and no dimension. The
        // other two formulas agree — Bottom + 0.01 mm for the half-height,
        // width / 10 for the roof peak — so it is the same chain run in the
        // drawing's unit, not a body someone dragged to nearly the same size.
        let instance_half = half_width(instance, 47);
        assert!(
            close(instance_half, left + 0.1e-3),
            "instance half-width {instance_half} is not Left + 0.1 mm"
        );
        let bottom = doc
            .jsites
            .iter()
            .find(|site| site.path == "/JSite145")
            .and_then(|site| site.symbol_information.as_ref())
            .and_then(|info| {
                info.symbol_informations
                    .iter()
                    .flat_map(|r| r.variables.iter())
                    .find(|v| v.name == "Bottom")
            })
            .expect("the template's Bottom variable")
            .value;
        let sheet_47 = instance.definition(47).expect("the placed tank");
        let body: Vec<_> = instance
            .lines
            .iter()
            .filter(|l| sheet_47.layers.binary_search(&l.sheet_layer_ref).is_ok())
            .collect();
        let peak = body
            .iter()
            .flat_map(|l| [l.start_y, l.end_y])
            .fold(f64::MIN, f64::max);
        // Floor and eaves are the two horizontal lines of the shell; the
        // roof's two slopes meet above the eaves at the peak.
        let (floor, eaves) = body
            .iter()
            .filter(|l| close(l.start_y, l.end_y))
            .map(|l| l.start_y)
            .fold((f64::MAX, f64::MIN), |(lo, hi), y| (lo.min(y), hi.max(y)));
        assert!(
            close((eaves - floor) / 2.0, bottom + 0.01e-3),
            "instance half-height {} is not Bottom + 0.01 mm",
            (eaves - floor) / 2.0
        );
        assert!(
            close(peak - eaves, 2.0 * instance_half / 10.0),
            "instance roof peak {} is not width / 10",
            peak - eaves
        );
        assert!(instance.dimensions.is_empty());
    }
}

/// Ratchet for plan K1
/// (`OpenCADStudio/docs/plans/2026-09-18-driving-dimensions-reach-the-panel-as-library-defaults.md`):
/// what J3 joined inside a probe is on the DTO. Each driving dimension of a
/// template body names the variable that drives it and the formula doing
/// it; each parametric body carries its `SymbolInformation` variables; and
/// each placed parametric body names the template body it was placed from,
/// so a consumer holding a placement's definition reference reaches the
/// library defaults in two lookups and never mistakes them for the
/// instance's own size.
///
/// Corpus
/// (`docs/analysis/2026-09-18-the-parametric-chain-closes-on-the-template-not-the-instance.md` §2):
/// five template / instance pairs, four paired because the instance's
/// variables still reference the template storage's `Double Value`s and one
/// (工艺's Black Box, whose references resolve nowhere) by variable names
/// and values; 18 dimensions on the four main drawings of which 15 are
/// named -- the three without a name are 0201's JDim 503 (no relation
/// writes it, so no formula either) and the two derived dimensions whose
/// relation reads other dimensions, D06's JDim 19 (`0E($1+$2)/10`) and
/// A01's JDim 82 (`0E$1/2`). Nothing is drawn differently: `entities` and
/// the golden snapshot are untouched, the schema only gains fields.
#[test]
fn a_placed_parametric_body_names_its_template_and_the_template_names_its_dimensions() {
    /// (placed body, template body, paired through `value_ref`), bodies as
    /// `(site, sheet)`.
    type Pair = ((u32, u32), (u32, u32), bool);
    struct Expected {
        fixture: &'static str,
        pairs: &'static [Pair],
        /// Dimensions with a name, over every template body.
        named: usize,
        /// (oid, has a formula) for every dimension without a name.
        unnamed: &'static [(u32, bool)],
    }
    const EXPECTED: &[Expected] = &[
        Expected {
            fixture: "D06.pid",
            pairs: &[((151, 47), (145, 15), true)],
            named: 4,
            unnamed: &[(19, true)],
        },
        Expected {
            fixture: "DWG-0201GP06-01.pid",
            pairs: &[
                ((396, 113), (329, 49), true),
                ((396, 119), (329, 501), true),
            ],
            named: 4,
            unnamed: &[(503, false)],
        },
        Expected {
            fixture: "DWG-0202GP06-01.pid",
            pairs: &[],
            named: 0,
            unnamed: &[],
        },
        Expected {
            fixture: "工艺管道及仪表流程-1.pid",
            pairs: &[((6963, 21), (7559, 72), false)],
            named: 4,
            unnamed: &[],
        },
        Expected {
            fixture: "export-test/publish-data/A01/A01.pid",
            pairs: &[((121, 481), (39, 96), true)],
            named: 3,
            unnamed: &[(82, true)],
        },
    ];
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
    let as_ref = |(site, sheet): (u32, u32)| pid_parse::PidSymbolDefinitionRef { site, sheet };

    for expected in EXPECTED {
        let fixture = expected.fixture;
        let Some(doc) = parse_test_file(fixture) else {
            continue;
        };
        let placed: std::collections::BTreeSet<pid_parse::PidSymbolDefinitionRef> = doc
            .sheet_streams
            .iter()
            .filter_map(|sheet| sheet.geometry.as_ref())
            .flat_map(|geometry| {
                geometry
                    .decoded_igsymbols
                    .iter()
                    .map(|p| as_ref((p.definition_site_ref, p.definition_sheet_ref)))
            })
            .collect();
        let geometry = pid_parse::build_normalized_geometry(&doc);

        // 1. Template bodies: every dimension either names a variable of the
        //    body or is one of the listed exceptions; a name always comes
        //    with a formula, and `0E$1` means the dimension *is* the
        //    variable's value.
        let mut named = 0usize;
        let mut unnamed: Vec<(u32, bool)> = Vec::new();
        for body in geometry
            .symbol_definitions
            .iter()
            .filter(|body| !body.dimensions.is_empty())
        {
            assert!(
                body.template.is_none(),
                "{fixture} {:?}: a body carrying dimensions is a template, not a placed instance",
                body.reference
            );
            assert!(
                !body.variables.is_empty(),
                "{fixture} {:?}: a template body without variables",
                body.reference
            );
            for dimension in &body.dimensions {
                match &dimension.name {
                    Some(name) => {
                        named += 1;
                        let formula = dimension.formula.as_deref().unwrap_or_else(|| {
                            panic!(
                                "{fixture} {:?}: JDim {} is named {name} but has no formula",
                                body.reference, dimension.oid
                            )
                        });
                        let variable = body
                            .variables
                            .iter()
                            .find(|variable| variable.name == *name)
                            .unwrap_or_else(|| {
                                panic!(
                                    "{fixture} {:?}: JDim {} is driven by {name}, which is not a variable of the body {:?}",
                                    body.reference,
                                    dimension.oid,
                                    body.variables
                                )
                            });
                        if formula == "0E$1" {
                            assert!(
                                close(dimension.value_m, variable.value_m),
                                "{fixture} {:?}: JDim {} = {} is {name} = {} through `0E$1`",
                                body.reference,
                                dimension.oid,
                                dimension.value_m,
                                variable.value_m
                            );
                        }
                    }
                    None => unnamed.push((dimension.oid, dimension.formula.is_some())),
                }
            }
        }
        assert_eq!(named, expected.named, "{fixture}: named driving dimensions");
        assert_eq!(
            unnamed, expected.unnamed,
            "{fixture}: the dimensions without a name, and whether a relation still writes them"
        );

        // 2. Placed instances: exactly the expected pairs, each pointing at
        //    a body a placement names, from a body no placement names, with
        //    the template's variables copied name for name and value for
        //    value -- and paired through `value_ref` exactly when the
        //    corpus says so.
        let pairs: Vec<(u32, u32, u32, u32)> = geometry
            .symbol_definitions
            .iter()
            .filter_map(|body| {
                body.template
                    .map(|t| (body.reference.site, body.reference.sheet, t.site, t.sheet))
            })
            .collect();
        let expected_pairs: Vec<(u32, u32, u32, u32)> = expected
            .pairs
            .iter()
            .map(|(instance, template, _)| (instance.0, instance.1, template.0, template.1))
            .collect();
        assert_eq!(
            pairs, expected_pairs,
            "{fixture}: placed parametric bodies and the templates they name"
        );
        for (instance, template, by_reference) in expected.pairs {
            let instance_ref = as_ref(*instance);
            let template_ref = as_ref(*template);
            assert!(
                placed.contains(&instance_ref) && !placed.contains(&template_ref),
                "{fixture}: a placement names the instance {instance:?} and none names the template {template:?}"
            );
            let instance = geometry
                .symbol_definition(instance_ref)
                .expect("the pair was just listed");
            let template = geometry
                .symbol_definition(template_ref)
                .unwrap_or_else(|| panic!("{fixture}: template {template_ref:?} is not a body"));
            assert!(
                instance.dimensions.is_empty(),
                "{fixture} {instance_ref:?}: a placed instance carries dimensions"
            );
            assert_eq!(
                instance.variables.len(),
                template.variables.len(),
                "{fixture} {instance_ref:?}: variables against the template's"
            );
            for (i, t) in instance.variables.iter().zip(&template.variables) {
                assert!(
                    i.name == t.name && close(i.value_m, t.value_m),
                    "{fixture} {instance_ref:?}: variable {} = {} against the template's {} = {}",
                    i.name,
                    i.value_m,
                    t.name,
                    t.value_m
                );
            }
            let references_resolve = instance.variables.iter().all(|i| {
                template
                    .variables
                    .iter()
                    .any(|t| t.value_ref == i.value_ref)
            });
            assert_eq!(
                references_resolve, *by_reference,
                "{fixture} {instance_ref:?}: whether the instance's value_refs are the template's"
            );
            // Two lookups from a placement to the library defaults.
            let defaults: Vec<(&str, f64)> = geometry
                .symbol_definition(instance_ref)
                .and_then(|body| body.template)
                .and_then(|template| geometry.symbol_definition(template))
                .map(|template| {
                    template
                        .dimensions
                        .iter()
                        .filter_map(|d| d.name.as_deref().map(|name| (name, d.value_m)))
                        .collect()
                })
                .unwrap_or_default();
            assert!(
                !defaults.is_empty(),
                "{fixture} {instance_ref:?}: no named library default reachable from the placement"
            );
        }

        // 3. Everything else -- the bodies that are not parametric -- has
        //    no variables and no template.
        for body in geometry
            .symbol_definitions
            .iter()
            .filter(|body| body.dimensions.is_empty() && body.template.is_none())
        {
            assert!(
                body.variables.is_empty(),
                "{fixture} {:?}: variables on a body that is neither a template nor a paired instance: {:?}",
                body.reference,
                body.variables
            );
        }
    }

    // D06's tank names four of its five dimensions after its four
    // variables, and the fifth -- the roof peak, width / 10 -- after none.
    if let Some(doc) = parse_test_file("D06.pid") {
        let geometry = pid_parse::build_normalized_geometry(&doc);
        let tank = geometry
            .symbol_definition(as_ref((145, 15)))
            .expect("the tank template");
        let mut names: Vec<Option<&str>> =
            tank.dimensions.iter().map(|d| d.name.as_deref()).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            vec![
                None,
                Some("Bottom"),
                Some("Left"),
                Some("Right"),
                Some("Top")
            ],
            "D06 tank dimension names"
        );
        let peak = tank
            .dimensions
            .iter()
            .find(|d| d.oid == 19)
            .expect("JDim 19");
        assert_eq!(peak.formula.as_deref(), Some("0E($1+$2)/10"));
    }
    // 0201's ` Line2`: one variable, one named dimension, and JDim 503 that
    // nothing writes; the Manifold's three, in on-disk order.
    if let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") {
        let geometry = pid_parse::build_normalized_geometry(&doc);
        let line2 = geometry
            .symbol_definition(as_ref((329, 501)))
            .expect("the Line2 template");
        assert_eq!(
            line2
                .variables
                .iter()
                .map(|v| (v.name.as_str(), v.value_m))
                .collect::<Vec<_>>(),
            vec![("Right", 0.0254)]
        );
        let named: Vec<(u32, Option<&str>, Option<&str>)> = line2
            .dimensions
            .iter()
            .map(|d| (d.oid, d.name.as_deref(), d.formula.as_deref()))
            .collect();
        assert_eq!(
            named,
            vec![(499, Some("Right"), Some("0E$1")), (503, None, None)]
        );
        let manifold = geometry
            .symbol_definition(as_ref((329, 49)))
            .expect("the Manifold template");
        assert_eq!(
            manifold
                .dimensions
                .iter()
                .map(|d| (d.oid, d.name.as_deref(), (d.value_m * 1e5).round() / 1e5))
                .collect::<Vec<_>>(),
            vec![
                (36, Some("Top"), 0.02032),
                (38, Some("Left"), 0.1143),
                (40, Some("Right"), 0.1143)
            ],
            "the Manifold's library defaults, in on-disk order"
        );
    }
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
            // PSM-decoded line/text/symbol/annotation records each
            // produce one additional `PidGeometryConfidence::Decoded`
            // entity on top of the probe / inferred totals above.
            let decoded_line_count = sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.decoded_primitive_lines.len());
            let decoded_igline_count = sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.decoded_iglines.len());
            let decoded_iglinestring_count = sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.decoded_iglinestrings.len());
            let decoded_igpoint_count = sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.decoded_igpoints.len());
            let decoded_igtextbox_count = sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.decoded_igtextboxes.len());
            let decoded_igsymbol_count = sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.decoded_igsymbols.len());
            // Phase 16 Slice F: `decoded_jstyle_overrides` emits
            // `PidGraphicKind::Annotation` entities on top of the
            // probe / Phase 14 totals above.
            let decoded_jstyle_override_count = sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.decoded_jstyle_overrides.len());
            let total = text_count
                + coordinate_count
                + endpoint_count
                + hint_count
                + decoded_line_count
                + decoded_igline_count
                + decoded_iglinestring_count
                + decoded_igpoint_count
                + decoded_igtextbox_count
                + decoded_igsymbol_count
                + decoded_jstyle_override_count;
            eprintln!(
                "sheet={}, text={text_count}, coord={coordinate_count}, ep={endpoint_count}, hint={hint_count}, decoded_line={decoded_line_count}, decoded_igline={decoded_igline_count}, decoded_iglinestring={decoded_iglinestring_count}, decoded_igpoint={decoded_igpoint_count}, decoded_igtextbox={decoded_igtextbox_count}, decoded_igsymbol={decoded_igsymbol_count}, decoded_jstyle_override={decoded_jstyle_override_count}, total={total}",
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
    // PSM-decoded line/polyline/point/text/symbol/annotation records
    // produce `PidGeometryConfidence::Decoded` entities. They are
    // additive to the inferred-points + inferred-lines +
    // probe-unknowns total below.
    let decoded_lines = geometry
        .entities
        .iter()
        .filter(|entity| {
            entity.confidence == pid_parse::PidGeometryConfidence::Decoded
                && matches!(entity.kind, pid_parse::PidGraphicKind::Line { .. })
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
    let decoded_points = geometry
        .entities
        .iter()
        .filter(|entity| {
            entity.confidence == pid_parse::PidGeometryConfidence::Decoded
                && matches!(entity.kind, pid_parse::PidGraphicKind::Point { .. })
        })
        .count();
    let decoded_texts = geometry
        .entities
        .iter()
        .filter(|entity| {
            entity.confidence == pid_parse::PidGeometryConfidence::Decoded
                && matches!(entity.kind, pid_parse::PidGraphicKind::Text { .. })
        })
        .count();
    let decoded_symbols = geometry
        .entities
        .iter()
        .filter(|entity| {
            entity.confidence == pid_parse::PidGeometryConfidence::Decoded
                && matches!(
                    entity.kind,
                    pid_parse::PidGraphicKind::SymbolInstance { .. }
                )
        })
        .count();
    // The JStyleOverride record layout is decoded, but the annotation anchor
    // remains a probe-derived interpretation of bytes that IDA reads as four
    // u32 fields. The normalized geometry entity must therefore stay Inferred.
    let inferred_annotations = geometry
        .entities
        .iter()
        .filter(|entity| {
            entity.confidence == pid_parse::PidGeometryConfidence::Inferred
                && matches!(entity.kind, pid_parse::PidGraphicKind::Annotation { .. })
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
            + decoded_polylines
            + decoded_points
            + decoded_texts
            + decoded_symbols
            + inferred_annotations,
        geometry.entities.len(),
        "coordinate/geometry hints become inferred points; fully mapped endpoint pairs become inferred lines; PSM-decoded GLine2d/igLine2d records become decoded lines; igLineString2d records become decoded polylines; igPoint2d records become decoded points; igTextBox records become decoded texts; igSymbol2d records become decoded symbol instances; JStyleOverride records become inferred annotations; remaining probe evidence stays ProbeOnly Unknown"
    );
    // `PidGeometryConfidence::Decoded` is legitimate for decoded
    // line / polyline / point / text / symbol records. Arc / Circle /
    // Annotation / Unknown must still not claim Decoded confidence.
    assert!(
        geometry.entities.iter().all(|entity| {
            entity.confidence != pid_parse::PidGeometryConfidence::Decoded
                || matches!(
                    entity.kind,
                    pid_parse::PidGraphicKind::Line { .. }
                        | pid_parse::PidGraphicKind::Polyline { .. }
                        | pid_parse::PidGraphicKind::Point { .. }
                        | pid_parse::PidGraphicKind::Text { .. }
                        | pid_parse::PidGraphicKind::SymbolInstance { .. }
                )
        }),
        "Decoded confidence currently only applies to PSM GLine2d / igLine2d / igLineString2d / igPoint2d / igTextBox / igSymbol2d entities"
    );
    assert!(
        geometry.entities.iter().all(|entity| {
            !matches!(entity.kind, pid_parse::PidGraphicKind::Annotation { .. })
                || entity.confidence == pid_parse::PidGeometryConfidence::Inferred
        }),
        "JStyleOverride annotations must remain Inferred while anchor semantics are ambiguous"
    );
    // `Polyline`, `Text` and `SymbolInstance` are legitimate when
    // backed by decoded records (`confidence == Decoded`). `Arc`
    // has no decoded Sheet source after Phase 17.
    assert!(
        geometry.entities.iter().all(|entity| {
            let typed_decoded = matches!(
                entity.kind,
                pid_parse::PidGraphicKind::Polyline { .. }
                    | pid_parse::PidGraphicKind::Text { .. }
                    | pid_parse::PidGraphicKind::SymbolInstance { .. }
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
        "probe-only and hint evidence must not become typed curve/text/symbol geometry without decoded records (Polyline/Text/Symbol are exceptions for decoded records)"
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
        geometry.warnings.iter().any(|warning| {
            warning.contains("coordinate units and page transforms are unavailable")
                || warning.contains("keeps unconverted source values")
        }),
        "normalized geometry should say which of its evidence is still unconverted"
    );
    assert!(
        promoted_raw_sheet_evidence(&geometry).is_empty(),
        "raw Sheet evidence must keep its unavailable coordinate context: {:?}",
        promoted_raw_sheet_evidence(&geometry)
    );
    assert!(
        geometry
            .warnings
            .iter()
            .any(|warning| warning.contains("geometry decode remains partial")),
        "normalized geometry should report partial, not absent, Sheet decoding"
    );
    assert!(
        geometry
            .warnings
            .iter()
            .all(|warning| !warning.contains("geometry decode not yet implemented")),
        "normalized geometry must not claim that implemented decoders are absent"
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
        if is_raw_sheet_evidence(entity) {
            assert!(
                matches!(
                    entity.coordinate_context.units,
                    pid_parse::PidDrawingUnits::Unknown { .. }
                ),
                "raw Sheet evidence keeps explicit unknown units until its own metadata is decoded"
            );
            assert!(
                matches!(
                    entity.coordinate_context.page_transform,
                    pid_parse::PidPageTransform::Unavailable { .. }
                ),
                "raw Sheet evidence keeps an explicit unavailable page transform"
            );
        }
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
    // Phase 14 Slice M legitimately emits Text entities from PSM
    // igTextBox records with `confidence: Decoded`. The text-window
    // *probe* path (this test's subject) must still never produce
    // Inferred Text entities — only Decoded text is allowed.
    let inferred_text_count = normalized
        .entities
        .iter()
        .filter(|entity| {
            matches!(entity.kind, pid_parse::PidGraphicKind::Text { .. })
                && entity.confidence != pid_parse::PidGeometryConfidence::Decoded
        })
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

    // Phase 34-A follow-up normalized the registry to six local fixtures
    // (D06 joined the Phase 9A five).
    assert!(line.contains("registered=6"));
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

/// DWG-0201 keeps its EndpointPair-inferred lines and emits **no**
/// `GLine2d` line.
///
/// Phase 14 Slice E asserted the opposite — at least one decoded `GLine2d`
/// — and that assertion held for six months on two entities that were never
/// records. They were the top two bytes of an `igSmartFrame2d`'s `1/√2` page
/// ratio, 160 bytes inside that record's payload, picked up because the
/// decoder scanned every byte offset instead of walking the chain. All of it
/// is measured in
/// `docs/analysis/2026-08-10-gline2d-is-the-iso-page-ratio-not-a-record.md`.
///
/// The direction is now pinned the other way: zero is the right count for a
/// corpus with no `GLine2d` in it, and the inferred floor is what must not
/// move. Should a real one ever turn up, this test is where it announces
/// itself.
#[test]
fn dwg0201_emits_no_gline2d_lines_and_keeps_its_inferred_floor() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let geometry = pid_parse::build_normalized_geometry(&doc);
    let inferred_lines = geometry
        .entities
        .iter()
        .filter(|entity| {
            entity.confidence == pid_parse::PidGeometryConfidence::Inferred
                && matches!(entity.kind, pid_parse::PidGraphicKind::Line { .. })
        })
        .count();
    // `GLine2d`-specific: Slice J's igLine2d entities share
    // `PidGraphicKind::Line` but carry a different note, and are covered by
    // the dedicated igLines test.
    let gline2d_lines: Vec<_> = geometry
        .entities
        .iter()
        .filter(|entity| {
            entity
                .source
                .note
                .as_ref()
                .is_some_and(|note| note.contains("PSM GLine2d"))
        })
        .collect();
    eprintln!(
        "DWG-0201GP06-01: inferred_lines={inferred_lines}, GLine2d_lines={}",
        gline2d_lines.len()
    );

    assert!(
        inferred_lines >= 49,
        "DWG-0201 inferred line floor regressed: got {inferred_lines}, expected >= 49"
    );
    assert!(
        gline2d_lines.is_empty(),
        "DWG-0201 has no GLine2d record; anything here is a scan artifact reaching \
         the drawing again: {:?}",
        gline2d_lines
            .iter()
            .map(|entity| &entity.source)
            .collect::<Vec<_>>()
    );
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
        "coordinate page metadata investigation: candidates={}, top_evidence={}, normalized_f64_candidates={}, page_dimension_candidates={}, i32_domain_candidates={}, normalized_f64_pair_count={}, page_dimension_scalar_matches={}, i32_bounds={:?}, f64_bounds={:?}, page_dimensions_mm={:?}, top={:?}",
        report.candidates.len(),
        report.top_evidence.len(),
        normalized_f64_candidates,
        page_dimension_candidates,
        i32_domain_candidates,
        report.normalized_f64_pair_count,
        report.page_dimension_scalar_matches,
        report.coordinate_hint_bounds,
        report.f64_coordinate_bounds,
        normalized.page_dimensions_mm,
        report.top_evidence
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
    assert!(
        !report.top_evidence.is_empty() && report.top_evidence.len() <= 8,
        "coordinate/page metadata report should expose a bounded top-evidence summary"
    );
    assert!(
        report.top_evidence.iter().all(|evidence| {
            evidence.example_offset < raw_sheet.data.len()
                && !evidence.example_hex_prefix.is_empty()
                && evidence.candidate_i32_pairs
                    + evidence.candidate_f64_pairs
                    + evidence.normalized_f64_pairs
                    + evidence.page_dimension_scalar_matches
                    > 0
        }),
        "top evidence should carry bounded offsets, hex prefixes, and numeric support"
    );
    assert!(
        unavailable_context_entities > 0,
        "CoordinatePageMetadata investigation must leave its own evidence unpromoted"
    );
    assert!(
        promoted_raw_sheet_evidence(&normalized).is_empty(),
        "CoordinatePageMetadata investigation must not make page transforms available: {:?}",
        promoted_raw_sheet_evidence(&normalized)
    );
    assert!(
        normalized.warnings.iter().any(|warning| {
            warning.contains("coordinate units and page transforms are unavailable")
                || warning.contains("keeps unconverted source values")
        }),
        "normalized geometry should keep the unconverted evidence visible in its warnings"
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

/// Only the drawing's own border frame promotes the page transform; a
/// template name never does.
///
/// The two are distinguishable on this fixture: `XIONGANA2.pid` can only
/// yield the A2 nominal 594.0mm, while the frame measures 594.3mm. Reading
/// the nominal back means the frame was not consulted, and any promotion
/// would then be resting on a name.
#[test]
fn only_a_border_frame_promotes_the_page_transform() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };

    let normalized = pid_parse::build_normalized_geometry(&doc);
    let (width, height) = normalized
        .page_dimensions_mm
        .expect("DWG-0201 states a page through its border frame");
    assert!(
        (width - 594.0).abs() > 0.1,
        "page came back at the template's nominal {width} x {height} mm, so the frame was not read"
    );
    assert!(
        !normalized.entities.is_empty(),
        "fixture should have normalized entities for coordinate-context checking"
    );
    assert!(
        promoted_raw_sheet_evidence(&normalized).is_empty(),
        "raw Sheet evidence is not in page space and must not be promoted: {:?}",
        promoted_raw_sheet_evidence(&normalized)
    );
    assert!(
        normalized.entities.iter().any(|entity| {
            matches!(
                entity.coordinate_context.page_transform,
                pid_parse::PidPageTransform::Available { .. }
            )
        }),
        "decoded records should carry the page the border frame states"
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
    let mut promoted_raw_sheet_entities: Vec<String> = Vec::new();

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
        promoted_raw_sheet_entities.extend(
            promoted_raw_sheet_evidence(&normalized)
                .into_iter()
                .map(str::to_owned),
        );

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
    assert!(
        unavailable_context_entities > 0 && unavailable_context_entities <= total_entities,
        "template or scalar scan evidence must not make page transforms available"
    );
    assert!(
        promoted_raw_sheet_entities.is_empty(),
        "template or scalar scan evidence must not make page transforms available: {promoted_raw_sheet_entities:?}"
    );
}

#[test]
fn sheet_geometry_investigation_aggregates_cross_fixture_evidence_without_promotion() {
    let mut fixtures_seen = 0usize;
    let mut sheets_seen = 0usize;
    let mut coordinate_metadata_candidates = 0usize;
    let mut coordinate_top_evidence = 0usize;
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
            coordinate_top_evidence += coordinate_report.top_evidence.len();
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
            if !coordinate_report.candidates.is_empty() {
                assert!(
                    !coordinate_report.top_evidence.is_empty()
                        && coordinate_report.top_evidence.len() <= 8,
                    "coordinate metadata report should expose bounded top evidence for {} {}",
                    fixture.path,
                    sheet.path
                );
                assert!(
                    coordinate_report.top_evidence.iter().all(|evidence| {
                        evidence.example_offset < raw_sheet.data.len()
                            && !evidence.example_hex_prefix.is_empty()
                    }),
                    "coordinate metadata top evidence must keep bounded example offsets for {} {}",
                    fixture.path,
                    sheet.path
                );
            }
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
        "cross-fixture Sheet geometry investigation: fixtures_seen={}, sheets_seen={}, coordinate_metadata_candidates={}, coordinate_top_evidence={}, normalized_f64_pair_count={}, page_dimension_scalar_matches={}, curve_groups={}, marker_49215_groups={}, polyline_like={}, mixed_numeric={}, short_i32_sequences={}",
        fixtures_seen,
        sheets_seen,
        coordinate_metadata_candidates,
        coordinate_top_evidence,
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
        coordinate_top_evidence > 0,
        "cross-fixture coordinate metadata investigation should expose compact top evidence"
    );
    assert!(
        mixed_numeric >= polyline_like,
        "mixed numeric evidence should remain visible while PolylineLike requires stronger point-sequence proof"
    );
}

/// Phase 25-A Slice E: cross-fixture ratchet for
/// `coordinate_pair_spatial_analysis`.
///
/// Extracts every normalized `(x, y)` f64 pair from each fixture's
/// `/Sheet*` streams (via `collect_normalized_f64_pairs`), runs the
/// Phase 25-A spatial-distribution analysis, and locks a positive
/// non-uniform baseline. This is the cross-fixture aggregate the
/// Phase 25-A plan §2.2 / verification Slice E requires. It is
/// read-only evidence and asserts the analysis never invents promotion
/// (cluster ids are topology hints, not coordinate authority).
#[test]
fn spatial_analysis_cross_fixture() {
    const GRID_N: usize = 20;

    // Per-present-fixture cluster floors derived from the Slice A probe
    // (docs/analysis/2026-05-23-phase25-slice-a-probe-output.md) with
    // comfortable margin so the ratchet is stable but meaningful.
    fn fixture_cluster_floor(path: &str) -> usize {
        match path {
            "DWG-0201GP06-01.pid" => 10,
            "DWG-0202GP06-01.pid" => 10,
            "工艺管道及仪表流程-1.pid" => 18,
            "export-test/publish-data/A01/A01.pid" => 6,
            "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid" => 10,
            _ => 1,
        }
    }

    let mut fixtures_seen = 0usize;
    let mut sheets_with_pairs = 0usize;
    let mut multi_cluster_sheets = 0usize;
    let mut non_uniform_fixtures = 0usize;
    let mut total_clusters = 0usize;
    let mut total_pairs = 0usize;
    let mut max_clusters_on_single_sheet = 0usize;
    let mut determinism_checked = false;

    for fixture in geometry_fixture_cases() {
        let Some(pkg) = parse_test_package(fixture.path) else {
            continue;
        };
        fixtures_seen += 1;
        let mut fixture_clusters = 0usize;
        let mut fixture_non_uniform = false;

        for sheet in pkg
            .parsed
            .sheet_streams
            .iter()
            .filter(|sheet| sheet.path.starts_with("/Sheet"))
        {
            let Some(raw_sheet) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            let pairs = collect_normalized_f64_pairs(&raw_sheet.data);
            let report = coordinate_pair_spatial_analysis(&pairs, GRID_N);

            // Structural invariants: read-only, internally consistent.
            assert_eq!(
                report.pair_count,
                pairs.len(),
                "report pair_count must equal scanned pairs for {} {}",
                fixture.path,
                sheet.path
            );
            assert_eq!(
                report.grid_resolution, GRID_N,
                "report must record the grid resolution it ran with for {} {}",
                fixture.path, sheet.path
            );
            assert_eq!(
                report.clusters.iter().map(|c| c.pair_count).sum::<usize>(),
                report.pair_count,
                "cluster pair_count must partition all pairs for {} {}",
                fixture.path,
                sheet.path
            );
            assert_eq!(
                report.uniform_distribution,
                report.clusters.len() <= 1,
                "uniform_distribution must mirror cluster count for {} {}",
                fixture.path,
                sheet.path
            );
            for cluster in &report.clusters {
                let ((min_x, min_y), (max_x, max_y)) = cluster.bbox;
                assert!(
                    (0.0..=1.0).contains(&min_x)
                        && (0.0..=1.0).contains(&min_y)
                        && (0.0..=1.0).contains(&max_x)
                        && (0.0..=1.0).contains(&max_y)
                        && min_x <= max_x
                        && min_y <= max_y,
                    "cluster bbox must stay ordered inside [0, 1]² for {} {}: {:?}",
                    fixture.path,
                    sheet.path,
                    cluster
                );
                let (cx, cy) = cluster.centroid;
                assert!(
                    (min_x..=max_x).contains(&cx) && (min_y..=max_y).contains(&cy),
                    "cluster centroid must lie inside its bbox for {} {}: {:?}",
                    fixture.path,
                    sheet.path,
                    cluster
                );
                assert!(
                    cluster.pair_count > 0,
                    "discovered clusters must be non-empty for {} {}",
                    fixture.path,
                    sheet.path
                );
            }

            if !pairs.is_empty() {
                sheets_with_pairs += 1;
                // Determinism: identical input ⇒ identical report.
                if !determinism_checked {
                    let rerun = coordinate_pair_spatial_analysis(&pairs, GRID_N);
                    assert_eq!(
                        report, rerun,
                        "spatial analysis must be deterministic for {} {}",
                        fixture.path, sheet.path
                    );
                    determinism_checked = true;
                }
            }
            if report.clusters.len() >= 2 {
                multi_cluster_sheets += 1;
                fixture_non_uniform = true;
            }
            fixture_clusters += report.clusters.len();
            total_clusters += report.clusters.len();
            total_pairs += report.pair_count;
            max_clusters_on_single_sheet = max_clusters_on_single_sheet.max(report.clusters.len());
        }

        if fixture_non_uniform {
            non_uniform_fixtures += 1;
        }

        let floor = fixture_cluster_floor(fixture.path);
        assert!(
            fixture_clusters >= floor,
            "fixture {} should yield >= {} clusters across /Sheet* (got {}); Slice A baseline regression?",
            fixture.path,
            floor,
            fixture_clusters
        );
    }

    eprintln!(
        "cross-fixture spatial analysis: fixtures_seen={}, sheets_with_pairs={}, multi_cluster_sheets={}, non_uniform_fixtures={}, total_pairs={}, total_clusters={}, max_clusters_on_single_sheet={}",
        fixtures_seen,
        sheets_with_pairs,
        multi_cluster_sheets,
        non_uniform_fixtures,
        total_pairs,
        total_clusters,
        max_clusters_on_single_sheet
    );

    if fixtures_seen == 0 {
        eprintln!("skipping: no available PID fixtures found for cross-fixture spatial analysis");
        return;
    }
    assert!(
        sheets_with_pairs > 0,
        "available fixtures should expose Sheet streams carrying normalized f64 pairs"
    );
    // Positive non-uniform signal (Phase 25-A Slice A decision: continue,
    // not negative closeout). At least one sheet must produce >= 3
    // distinct clusters and at least one fixture must be non-uniform.
    assert!(
        max_clusters_on_single_sheet >= 3,
        "at least one sheet must produce >= 3 distinct spatial clusters (positive signal), got {}",
        max_clusters_on_single_sheet
    );
    assert!(
        non_uniform_fixtures > 0,
        "at least one fixture must show a non-uniform (multi-cluster) spatial distribution"
    );
    assert!(
        multi_cluster_sheets >= 1,
        "at least one sheet must carry >= 2 spatial clusters"
    );
}

/// Phase 25-A Slice C+D: the cluster pipeline populates
/// `SheetGeometry::spatial_analysis` from real fixture bytes, and the
/// populated model report stays consistent with a direct
/// `coordinate_pair_spatial_analysis` run over the same sheet bytes
/// (pipeline ↔ API parity). Read-only; asserts no promotion.
#[test]
fn spatial_analysis_pipeline_populates_sheet_geometry() {
    let Some(pkg) = parse_test_package("DWG-0201GP06-01.pid") else {
        eprintln!("skipping: DWG-0201GP06-01.pid fixture absent");
        return;
    };
    let sheet = pkg
        .parsed
        .sheet_streams
        .iter()
        .find(|sheet| sheet.path == "/Sheet6")
        .expect("DWG-0201 should expose /Sheet6");
    let geometry = sheet
        .geometry
        .as_ref()
        .expect("/Sheet6 should carry decoded geometry");
    let spatial = geometry
        .spatial_analysis
        .as_ref()
        .expect("/Sheet6 should carry Phase 25-A spatial analysis");

    // Pipeline ↔ direct API parity over the same raw sheet bytes.
    let raw = pkg
        .streams
        .get("/Sheet6")
        .expect("/Sheet6 raw bytes should be present");
    let pairs = collect_normalized_f64_pairs(&raw.data);
    let direct = coordinate_pair_spatial_analysis(&pairs, 20);
    assert_eq!(
        spatial.grid_resolution, 20,
        "pipeline must use the documented default grid resolution"
    );
    assert_eq!(spatial.pair_count, direct.pair_count);
    assert_eq!(spatial.clusters.len(), direct.clusters.len());
    assert_eq!(spatial.uniform_distribution, direct.uniform_distribution);

    // Positive non-uniform signal locked at Slice A (DWG-0201 = 15
    // clusters); pipeline must not collapse it to uniform.
    assert!(
        spatial.clusters.len() >= 2,
        "DWG-0201 /Sheet6 spatial analysis should be non-uniform, got {} clusters",
        spatial.clusters.len()
    );
    assert!(!spatial.uniform_distribution);
    assert_eq!(
        spatial.clusters.iter().map(|c| c.pair_count).sum::<usize>(),
        spatial.pair_count,
        "model cluster pair_count must partition all pairs"
    );
    for cluster in &spatial.clusters {
        assert!(
            (0.0..=1.0).contains(&cluster.min_x)
                && (0.0..=1.0).contains(&cluster.min_y)
                && (0.0..=1.0).contains(&cluster.max_x)
                && (0.0..=1.0).contains(&cluster.max_y)
                && cluster.min_x <= cluster.max_x
                && cluster.min_y <= cluster.max_y,
            "model cluster bbox must stay ordered inside [0, 1]²: {cluster:?}"
        );
        assert!(
            (cluster.min_x..=cluster.max_x).contains(&cluster.centroid_x)
                && (cluster.min_y..=cluster.max_y).contains(&cluster.centroid_y),
            "model cluster centroid must lie inside its bbox: {cluster:?}"
        );
    }
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
    // emitted by separate typed decoder families (for example
    // `decode_iglinestrings` Slice K) and surface through
    // `SheetGeometry::decoded_*` fields and `build_normalized_geometry`.
    // Circles and arcs currently don't have authoritative decoded
    // Sheet sources; assert zero for circles here.
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

/// Cross-fixture ratchet: `decode_primitive_lines` finds no `GLine2d` on
/// any `Sheet*` stream in the corpus, and anything it ever does find holds
/// the documented invariants.
///
/// Phase 14 Slice D asserted an aggregate of `>= 1` against baselines of
/// "DWG-0201 → 2 hits, A01 → 1 hit". Those three were never records: each
/// sat 160 bytes inside an `igSmartFrame2d`, on the two bytes where its
/// `1/√2` page ratio spells `0x3FE6`. Requiring at least one therefore
/// pinned the artifact in place. Measured in
/// `docs/analysis/2026-08-10-gline2d-is-the-iso-page-ratio-not-a-record.md`.
///
/// The floor is now zero and the per-record invariants stay, so a genuine
/// `GLine2d` appearing in a future fixture trips the count assertion and
/// gets checked on the way in rather than slipping through.
#[test]
fn primitive_line_decoder_finds_no_gline2d_and_holds_invariants_if_it_ever_does() {
    let fixtures = [
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
        "export-test/publish-data/A01/A01.pid",
        "D06.pid",
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
    eprintln!("--- PSM GLine2d decoder cross-fixture summary ---");
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
    assert_eq!(
        total_decoded, 0,
        "the corpus contains no GLine2d record. A non-zero count is either a \
        genuine one in a newly added fixture — in which case check it against \
        radsrvitem.dll's type enumeration before raising this floor — or the \
        chain-membership gate has been lost and scan artifacts are back. \
        Per-fixture summary: {per_fixture_summary:?}"
    );
}

/// Phase 15 Slice C: fixture-level ratchet for the conservative
/// PSM `0x00FA` DependencyObject / GraphicPersist parser.
///
/// This intentionally stays at parser level. It locks the stable record
/// envelope and count evidence from `examples/probe_psm_0x00fa_shape.rs`
/// without promoting candidate child OIDs into the public model/schema.
/// The conservative decoder currently emits 352 records across the
/// four-fixture set: one fewer than the broad bounded probe count
/// because the decoder applies additional header validation.
#[test]
fn dependency_object_decoder_ratchets_fixture_counts_and_header_fields() {
    let fixtures = [
        ("DWG-0201GP06-01.pid", 135usize),
        ("DWG-0202GP06-01.pid", 84usize),
        ("工艺管道及仪表流程-1.pid", 125usize),
        ("export-test/publish-data/A01/A01.pid", 8usize),
    ];
    let mut total_decoded = 0usize;
    let mut total_expected = 0usize;
    let mut per_fixture_summary: Vec<(String, usize, usize)> = Vec::new();
    let mut sample_groups: Vec<String> = Vec::new();

    for (fixture, expected_count) in fixtures {
        let Some(pkg) = parse_test_package(fixture) else {
            continue;
        };
        let mut per_fixture_count = 0usize;
        for sheet in &pkg.parsed.sheet_streams {
            let Some(raw) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            let bytes = raw.data.as_slice();
            let decoded = decode_dependency_objects(bytes);
            let model_decoded_count = sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.decoded_dependency_objects.len());
            assert_eq!(
                model_decoded_count,
                decoded.len(),
                "SheetGeometry audit collection must mirror parser-level DependencyObject count for {} {}",
                fixture,
                sheet.path
            );
            for group in &decoded {
                assert!(
                    group.byte_range.end <= bytes.len(),
                    "DependencyObject byte_range {:?} exceeds stream {} bytes ({})",
                    group.byte_range,
                    sheet.path,
                    bytes.len()
                );
                assert!(
                    group.byte_range.start < group.byte_range.end,
                    "DependencyObject byte_range must be non-empty: {:?}",
                    group.byte_range
                );
                assert_eq!(group.type_code, PSM_TYPE_CODE_DEPENDENCY_OBJECT);
                assert_eq!(group.type_flags, 0);
                assert!(
                    group.bytes_to_follow as usize >= DEPENDENCY_OBJECT_MIN_PAYLOAD_LEN,
                    "DependencyObject bytes_to_follow below conservative floor: {:?}",
                    group
                );
                assert_eq!(group.bytes_to_follow % 2, 0);
                assert_ne!(group.oid, 0);
                assert_eq!(group.parent_ref, 6);
                assert!((1..=16).contains(&group.group_kind_word));
                assert_eq!(
                    18 + group.raw_reference_payload.len(),
                    group.bytes_to_follow as usize,
                    "raw reference tail must cover payload bytes after stable 18-byte prefix"
                );

                if sample_groups.len() < 6 {
                    sample_groups.push(format!(
                        "{fixture} {} @ 0x{:06x}..0x{:06x} oid={} kind={} sub_type=0x{:04X} tail_len={}",
                        sheet.path,
                        group.byte_range.start,
                        group.byte_range.end,
                        group.oid,
                        group.group_kind_word,
                        group.sub_type_word,
                        group.raw_reference_payload.len(),
                    ));
                }
            }
            per_fixture_count += decoded.len();
        }

        per_fixture_summary.push((fixture.to_string(), per_fixture_count, expected_count));
        total_decoded += per_fixture_count;
        total_expected += expected_count;
    }

    eprintln!("--- Phase 15 Slice C: PSM DependencyObject decoder fixture ratchet ---");
    for (name, actual, expected) in &per_fixture_summary {
        eprintln!("  {name}: {actual} decoded DependencyObject records (expected {expected})");
    }
    for sample in &sample_groups {
        eprintln!("  sample: {sample}");
    }
    eprintln!("  total decoded DependencyObject records: {total_decoded}");

    if per_fixture_summary.is_empty() {
        eprintln!(
            "skipping: no Sheet-bearing fixture present (CI / contributors \
            without SmartPlant samples)"
        );
        return;
    }

    for (fixture, actual, expected) in &per_fixture_summary {
        assert_eq!(
            actual, expected,
            "DependencyObject fixture count drift for {fixture}: expected {expected}, got {actual}. \
            Re-run `cargo run --release --example probe_psm_0x00fa_shape` before updating this ratchet."
        );
    }
    assert_eq!(
        total_decoded, total_expected,
        "DependencyObject aggregate count drift. Per-fixture summary: {per_fixture_summary:?}"
    );
}

/// Phase 16 Slice D: cross-fixture sanity test for the additive
/// `decode_jstyle_overrides` parser (PSM type `0x0030`, RAD
/// `JStyleOverride` Version-3 IO, `style.dll` CLSID
/// `{47FCC338-2D0F-11D0-A1FF-080036A1CF02}`).
///
/// The IDA-confirmed schema accepts the full PSM `0x0030`
/// `JStyleOverride` family. probe v5 reports 98 raw 0x0030 hits
/// across the four-fixture set; this conservative typed decoder is
/// expected to stay near that count.
#[test]
fn jstyle_override_decoder_emits_audit_records_with_provenance() {
    let fixtures = [
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
        "export-test/publish-data/A01/A01.pid",
    ];
    let mut total_decoded = 0usize;
    let mut per_fixture_summary: Vec<(String, usize)> = Vec::new();
    let mut sample_records: Vec<String> = Vec::new();

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
            let decoded = decode_jstyle_overrides(bytes);
            let model_decoded_count = sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.decoded_jstyle_overrides.len());
            assert_eq!(
                model_decoded_count,
                decoded.len(),
                "SheetGeometry::decoded_jstyle_overrides must mirror parser-level count for {} {}",
                fixture,
                sheet.path
            );
            for rec in &decoded {
                assert!(rec.byte_range.end <= bytes.len(), "byte_range overflow");
                assert!(rec.byte_range.start < rec.byte_range.end);
                assert_eq!(rec.type_code, PSM_TYPE_CODE_JSTYLE_OVERRIDE);
                assert_eq!(rec.type_flags, 0);
                assert!(
                    rec.bytes_to_follow >= JSTYLE_OVERRIDE_MIN_BYTES_TO_FOLLOW,
                    "bytes_to_follow below floor: {:?}",
                    rec
                );
                assert!(rec.field_1_f64.is_finite() && rec.field_2_f64.is_finite());
                assert!(rec.field_3_f64.is_finite() && rec.field_4_f64.is_finite());
                // `bytes_to_follow` covers `oid(4) + aux(8) + payload(64) +
                // attribute_tail`, so tail length = btf - 76.
                let expected_tail_len =
                    rec.bytes_to_follow as usize - (JSTYLE_OVERRIDE_PAYLOAD_LEN + 12);
                assert_eq!(
                    rec.raw_attribute_tail.len(),
                    expected_tail_len,
                    "raw_attribute_tail length must equal bytes_to_follow - 76 \
                     (oid + aux + payload subtracted)"
                );

                if sample_records.len() < 6 {
                    sample_records.push(format!(
                        "{fixture} {} @ 0x{:06x}..0x{:06x} oid={} btf={} f64#2={:.4} (rotation candidate)",
                        sheet.path,
                        rec.byte_range.start,
                        rec.byte_range.end,
                        rec.oid,
                        rec.bytes_to_follow,
                        rec.field_2_f64
                    ));
                }
            }
            per_fixture_count += decoded.len();
        }
        per_fixture_summary.push((fixture.to_string(), per_fixture_count));
        total_decoded += per_fixture_count;
    }

    eprintln!(
        "--- Phase 16 Slice D: JStyleOverride decoder cross-fixture (RAD style.dll CLSID 47FCC338) ---"
    );
    for (name, actual) in &per_fixture_summary {
        eprintln!("  {name}: {actual} decoded JStyleOverride records");
    }
    for sample in &sample_records {
        eprintln!("  sample: {sample}");
    }
    eprintln!("  total decoded JStyleOverride records: {total_decoded}");

    if per_fixture_summary.is_empty() {
        eprintln!(
            "skipping: no Sheet-bearing fixture present (CI / contributors \
            without SmartPlant samples)"
        );
        return;
    }

    // Sanity: at least one fixture must produce a record. The probe
    // v5 evidence shows ~98 hits cross-fixture; the typed decoder is
    // expected to be conservatively close. We assert a generous
    // range to avoid premature ratchet drift.
    assert!(
        (1..=200).contains(&total_decoded),
        "JStyleOverride decoder produced an unexpected aggregate count {total_decoded}; \
         Per-fixture summary: {per_fixture_summary:?}. \
         Cross-check with `cargo run --release --example probe_garc2d_packed_bytes`."
    );
}

/// Phase 14 Slice N: cross-fixture validation that
/// `decode_igsymbols` emits decoded `igSymbol2d` symbol instances
/// (PSM type `0x00CE`) from real `Sheet*` streams.
#[test]
fn igsymbols_decoder_emits_decoded_symbols_with_provenance() {
    let fixtures = [
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
        "export-test/publish-data/A01/A01.pid",
        // Phase 22 micro: D06 contributes 2 decoded igSymbol2d records;
        // the floor below is ratcheted from 20 → 22 to reflect this.
        "D06.pid",
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
            let decoded = decode_igsymbols(bytes);
            for s in &decoded {
                assert!(s.byte_range.end <= bytes.len());
                assert_eq!(s.type_code, PSM_TYPE_CODE_IGSYMBOL2D);
                assert!(s.bytes_to_follow >= 113);
                assert!(s.insertion.0.is_finite() && s.insertion.1.is_finite());
                if sample_lines.len() < 5 {
                    sample_lines.push(format!(
                        "{fixture} oid={} parent={} insertion=({:.4}, {:.4}) \
                         transform=[{:.2}, {:.2}, {:.2}, {:.2}]",
                        s.oid,
                        s.parent_ref,
                        s.insertion.0,
                        s.insertion.1,
                        s.transform[0],
                        s.transform[1],
                        s.transform[2],
                        s.transform[3],
                    ));
                }
            }
            per_fixture_count += decoded.len();
        }
        per_fixture_summary.push((fixture.to_string(), per_fixture_count));
        total_decoded += per_fixture_count;
    }
    eprintln!("--- Phase 14 Slice N: PSM igSymbol2d decoder cross-fixture summary ---");
    for (name, count) in &per_fixture_summary {
        eprintln!("  {name}: {count} decoded igSymbol2d records");
    }
    for s in &sample_lines {
        eprintln!("  sample: {s}");
    }
    eprintln!("  total decoded symbols: {total_decoded}");
    if per_fixture_summary.is_empty() {
        return;
    }
    assert!(
        total_decoded >= 22,
        "decode_igsymbols should emit >= 22 symbols cross-fixture (Phase 22 micro: \
         D06 contributes 2 igSymbol2d records), got {total_decoded}"
    );
}

/// Phase 35-C: every real `igSymbol2d` record's `jsite_ref` (the u32
/// immediately before the placement-matrix tag) resolves to a
/// same-file `JSite<id>` storage, and normalized geometry surfaces
/// that site's `.sym` library path on the `SymbolInstance` entity.
#[test]
fn igsymbol2d_jsite_ref_resolves_to_symbol_paths() {
    let fixtures = [
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
        "D06.pid",
        "export-test/publish-data/A01/A01.pid",
        "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
    ];
    for fixture in fixtures {
        let Some(doc) = parse_test_file(fixture) else {
            continue;
        };
        let jsite_ids: BTreeSet<u32> = doc
            .jsites
            .iter()
            .filter_map(|site| site.name.strip_prefix("JSite")?.parse().ok())
            .collect();

        let mut record_count = 0usize;
        for sheet in &doc.sheet_streams {
            let Some(geometry) = &sheet.geometry else {
                continue;
            };
            for record in &geometry.decoded_igsymbols {
                record_count += 1;
                assert!(
                    jsite_ids.contains(&record.jsite_ref),
                    "{fixture} {} oid={}: jsite_ref={} has no matching JSite storage \
                     (known ids: {jsite_ids:?})",
                    sheet.path,
                    record.oid,
                    record.jsite_ref,
                );
            }
        }
        if record_count == 0 {
            continue;
        }

        let geometry = pid_parse::build_normalized_geometry(&doc);
        let mut symbol_instances = 0usize;
        let mut with_sym_path = 0usize;
        for entity in &geometry.entities {
            if let pid_parse::PidGraphicKind::SymbolInstance { symbol_path, .. } = &entity.kind {
                symbol_instances += 1;
                if symbol_path
                    .as_deref()
                    .is_some_and(|path| path.to_ascii_lowercase().ends_with(".sym"))
                {
                    with_sym_path += 1;
                }
            }
        }
        assert_eq!(
            with_sym_path, symbol_instances,
            "{fixture}: every SymbolInstance should resolve its JSite's .sym path \
             (Phase 35-C probe: 132/132 records)"
        );
        eprintln!(
            "{fixture}: {record_count} igSymbol2d records, {with_sym_path}/{symbol_instances} \
             symbol instances carry a .sym path"
        );
    }

    // Fixture-specific spot check: A01's two placed symbols are a
    // flanged nozzle and the horizontal drum it sits on.
    if let Some(doc) = parse_test_file("export-test/publish-data/A01/A01.pid") {
        let geometry = pid_parse::build_normalized_geometry(&doc);
        let paths: BTreeSet<String> = geometry
            .entities
            .iter()
            .filter_map(|entity| match &entity.kind {
                pid_parse::PidGraphicKind::SymbolInstance {
                    symbol_path: Some(path),
                    ..
                } => Some(path.clone()),
                _ => None,
            })
            .collect();
        assert!(
            paths.iter().any(|p| p.ends_with("Flanged Nozzle.sym")),
            "A01 should reference Flanged Nozzle.sym, got {paths:?}"
        );
        assert!(
            paths.iter().any(|p| p.ends_with("Horizontal Drum.sym")),
            "A01 should reference Horizontal Drum.sym, got {paths:?}"
        );
    }
}

/// A placement names the style its body draws with — `igSymbol2d +25` —
/// and it is that style, not the `.sym`'s own, that `SmartPlant` puts on
/// screen. The discriminating case is `DWG-0201`'s vessel: authored black
/// in `Parametric Manifold.sym`, screenshotted in `#800000` maroon, and
/// its placement (oid 326) names style id 75, which the root
/// `StyleCluster` defines as exactly `#800000` 0.35mm.
///
/// Reverting the `+25` read (any fixed value, or a shifted offset) breaks
/// the oid → style_ref table below; resolution and palette are ratcheted
/// separately in `tests/style_link_ratchet.rs`. See
/// `docs/analysis/2026-08-24-placement-names-the-body-style.md`.
#[test]
fn igsymbol2d_placements_name_the_style_their_body_draws_with() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let mut style_refs: BTreeMap<u32, u32> = BTreeMap::new();
    for sheet in &doc.sheet_streams {
        let Some(geometry) = &sheet.geometry else {
            continue;
        };
        for record in &geometry.decoded_igsymbols {
            style_refs.insert(record.oid, record.style_ref);
        }
    }
    // The seven equipment placements the screenshot shows in maroon: the
    // vessel (Parametric Manifold, via id 75), three Flanged Nozzles, one
    // Flanged Nozzle with blind, Manway-Large and Gauge Hatch (id 82).
    for (oid, expected) in [
        (326u32, 75u32),
        (35, 82),
        (139, 82),
        (147, 82),
        (157, 82),
        (169, 82),
        (229, 82),
        // Off-Unit: authored with cyan strokes in its .sym, screenshotted
        // olive — the placement names id 83, #808000 0.35mm.
        (537, 83),
        // LG-Magnetic Float Gauge: instruments are class-coloured green.
        (68, 80),
    ] {
        assert_eq!(
            style_refs.get(&oid),
            Some(&expected),
            "DWG-0201 oid {oid}: placement style_ref should be {expected}, got {:?}",
            style_refs.get(&oid)
        );
    }
    assert_eq!(style_refs.len(), 20, "DWG-0201 places 20 symbols");
    assert!(
        style_refs.values().all(|&id| id != 0),
        "every DWG-0201 placement names a style: {style_refs:?}"
    );
}

/// Whether a point shows a mark is stated by the file, not inferred from its
/// colour.
///
/// A point's line style may name a `JStyleLineTerminator`, which names a
/// `JStylePointSymbol`, which owns a group of `igLine2d` — the glyph. A blank
/// symbol is stored as a full group whose lines are zero-length, and those
/// are the points `SmartPlant` draws nothing for.
///
/// The counts below are the screen truth for `DWG-0201`: eleven marks, on ten
/// riser tops and the vessel inlet. The rule reproduces them, and reproduces
/// every other fixture's count too. It is strictly stronger than the colour
/// rule it replaced — `DWG-0202` and the gongyi drawing both define a
/// live-glyph `#FF0000` symbol that no point uses, which a colour rule cannot
/// see at all.
///
/// See `docs/analysis/2026-08-25-a-point-draws-the-symbol-its-terminator-names.md`.
#[test]
fn a_point_draws_the_symbol_its_line_terminator_names() {
    // Three states, and the file distinguishes all three: a live glyph, a
    // terminator whose glyph is blank, and no terminator named at all. Only
    // the first draws. The middle one is the discovery — a blank symbol is
    // stored as a whole group of zero-length lines rather than by omission.
    for (fixture, expected_drawn, expected_blank, expected_none) in [
        ("DWG-0201GP06-01.pid", 11usize, 53usize, 11usize),
        ("DWG-0202GP06-01.pid", 5, 22, 4),
        ("工艺管道及仪表流程-1.pid", 11, 23, 2),
        ("D06.pid", 1, 9, 0),
    ] {
        let path = format!("test-file/{fixture}");
        if !Path::new(&path).exists() {
            eprintln!("skipping: fixture {fixture} not found");
            continue;
        }
        let styles = pid_parse::style_link::line_styles_for_file(Path::new(&path))
            .unwrap_or_else(|e| panic!("{fixture}: line_styles_for_file failed: {e}"));
        let doc = parse_test_file(fixture).expect("fixture exists");

        let (mut drawn, mut blank, mut none) = (0usize, 0usize, 0usize);
        for sheet in &doc.sheet_streams {
            let Some(geometry) = &sheet.geometry else {
                continue;
            };
            for record in &geometry.decoded_igpoints {
                let Some(style) = styles.get(&(sheet.path.clone(), record.oid)) else {
                    continue;
                };
                match style.marker {
                    Some(marker) if marker.draws() => drawn += 1,
                    Some(_) => blank += 1,
                    // The riser feet: a plain 54-byte line style, too short
                    // to hold the reference at all.
                    None => none += 1,
                }
            }
        }
        assert_eq!(
            (drawn, blank, none),
            (expected_drawn, expected_blank, expected_none),
            "{fixture}: expected {expected_drawn} marked, {expected_blank} blank-glyph \
             and {expected_none} terminator-less points"
        );
    }
}

/// The marks are a review status, and the drawing says so in words.
///
/// The `JStyleLibrarian` at the head of every `StyleCluster` names each point
/// symbol: `psOk`, `psWarning`, `psError`, `psApproved`. That is what the
/// glyphs are — not decoration, and not a per-discipline tick. It also settles
/// the blank symbol: `psOk` stores two zero-length lines because **an item
/// that passed has nothing to draw**.
///
/// Every marked point in the corpus is a `psWarning` or a `psApproved`, and
/// the blank ones are all `psOk`. Not one point is a `psError` — that state is
/// defined by the two richest drawings and triggered by nothing in either, so
/// the cross glyph nobody references is the state nothing is in rather than a
/// leftover template.
///
/// See `docs/analysis/2026-08-25-a-point-draws-the-symbol-its-terminator-names.md`.
#[test]
fn a_point_marks_the_review_status_the_librarian_names() {
    use pid_parse::style_link::MarkerStatus;

    // (ok, warning, error, approved, named-something-else)
    for (fixture, expected) in [
        (
            "DWG-0201GP06-01.pid",
            (53usize, 11usize, 0usize, 0usize, 0usize),
        ),
        ("DWG-0202GP06-01.pid", (22, 5, 0, 0, 0)),
        ("工艺管道及仪表流程-1.pid", (23, 1, 0, 10, 0)),
        ("D06.pid", (9, 1, 0, 0, 0)),
    ] {
        let path = format!("test-file/{fixture}");
        if !Path::new(&path).exists() {
            eprintln!("skipping: fixture {fixture} not found");
            continue;
        }
        let styles = pid_parse::style_link::line_styles_for_file(Path::new(&path))
            .unwrap_or_else(|e| panic!("{fixture}: line_styles_for_file failed: {e}"));
        let doc = parse_test_file(fixture).expect("fixture exists");

        let mut tally = (0usize, 0usize, 0usize, 0usize, 0usize);
        for sheet in &doc.sheet_streams {
            let Some(geometry) = &sheet.geometry else {
                continue;
            };
            for record in &geometry.decoded_igpoints {
                let Some(marker) = styles
                    .get(&(sheet.path.clone(), record.oid))
                    .and_then(|style| style.marker)
                else {
                    continue;
                };
                match marker.status {
                    Some(MarkerStatus::Ok) => tally.0 += 1,
                    Some(MarkerStatus::Warning) => tally.1 += 1,
                    Some(MarkerStatus::Error) => tally.2 += 1,
                    Some(MarkerStatus::Approved) => tally.3 += 1,
                    None => tally.4 += 1,
                }
                // The status and the glyph have to agree, or one of the two
                // is being read wrong: only `psOk` is blank.
                assert_eq!(
                    marker.status == Some(MarkerStatus::Ok),
                    !marker.draws(),
                    "{fixture}: point {} is {:?} but draws() is {}",
                    record.oid,
                    marker.status,
                    marker.draws()
                );
            }
        }
        assert_eq!(
            tally, expected,
            "{fixture}: expected (ok, warning, error, approved, other) {expected:?}"
        );
    }
}

/// The glyph is the file's, and there is more than one of them.
///
/// `DWG-0201`'s marked points draw a two-stroke slash; the gongyi drawing
/// gives its ten instrument points a check mark instead, and defines a
/// five-millimetre cross for a third status. A renderer that hard-codes one
/// shape gets two of the three wrong.
#[test]
fn a_point_symbols_glyph_is_read_from_the_group_it_owns() {
    let path = Path::new("test-file/DWG-0201GP06-01.pid");
    if !path.exists() {
        eprintln!("skipping: fixture DWG-0201GP06-01.pid not found");
        return;
    }
    let styles = pid_parse::style_link::line_styles_for_file(path).expect("index builds");
    let marker = styles
        .values()
        .find_map(|style| style.marker.filter(|marker| marker.draws()))
        .expect("DWG-0201 defines a live point symbol");

    let strokes = marker.strokes();
    assert_eq!(
        strokes.len(),
        2,
        "the slash glyph is two strokes: {strokes:?}"
    );
    // Millimetres, as the group's igLine2d state them.
    assert_eq!(strokes[0].start, (0.0, 0.0));
    assert!((strokes[0].end.0 * 1000.0 - 3.0).abs() < 1e-9);
    assert!((strokes[0].end.1 * 1000.0 - 6.0).abs() < 1e-9);
    assert!(
        (strokes[0].length_mm() - 6.708_203).abs() < 1e-5,
        "long stroke reads {}mm",
        strokes[0].length_mm()
    );
    assert!(
        strokes[1].length_mm() < strokes[0].length_mm(),
        "the second stroke is the short stub"
    );
    assert!(strokes.iter().all(|s| !s.is_degenerate()));
}

/// Phase 14 Slice M: cross-fixture validation that
/// `decode_igtextboxes` emits decoded `igTextBox` text annotations
/// (PSM type `0x004D`) from real `Sheet*` streams.
#[test]
fn igtextboxes_decoder_emits_decoded_texts_with_provenance() {
    let fixtures = [
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
        "export-test/publish-data/A01/A01.pid",
        // Phase 22 micro: D06 contributes 4 decoded igTextBox records;
        // the floor below is ratcheted from 20 → 24 to reflect this.
        "D06.pid",
    ];
    let mut total_decoded = 0usize;
    let mut per_fixture_summary: Vec<(String, usize)> = Vec::new();
    let mut sample_texts: Vec<String> = Vec::new();
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
            let decoded = decode_igtextboxes(bytes);
            for t in &decoded {
                assert!(t.byte_range.end <= bytes.len());
                assert_eq!(t.type_code, PSM_TYPE_CODE_IGTEXTBOX);
                // 60 is the family's real floor: sub-type 1 with no text is
                // 22 header + 2 body + 36 tail. 68 was sub-type 2's overhead,
                // asserted here while it was mistaken for the family's.
                assert!(t.bytes_to_follow >= 60);
                assert!(matches!(t.text_sub_type, 1..=3));
                assert!(t.text_length <= 1024);
                assert!(t.trailing_double_1.is_finite());
                if sample_texts.len() < 8 {
                    sample_texts.push(format!(
                        "{fixture} {} oid={} parent={} text_length={} text={:?}",
                        sheet.path, t.oid, t.parent_ref, t.text_length, t.text
                    ));
                }
            }
            per_fixture_count += decoded.len();
        }
        per_fixture_summary.push((fixture.to_string(), per_fixture_count));
        total_decoded += per_fixture_count;
    }
    eprintln!("--- Phase 14 Slice M: PSM igTextBox decoder cross-fixture summary ---");
    for (name, count) in &per_fixture_summary {
        eprintln!("  {name}: {count} decoded igTextBox records");
    }
    for s in &sample_texts {
        eprintln!("  sample: {s}");
    }
    eprintln!("  total decoded texts: {total_decoded}");
    if per_fixture_summary.is_empty() {
        return;
    }
    assert!(
        total_decoded >= 24,
        "decode_igtextboxes should emit >= 24 text records cross-fixture (Phase 22 micro: \
         D06 contributes 4 igTextBox records), got {total_decoded}"
    );
}

/// Phase 14 Slice L: cross-fixture validation that
/// `decode_igpoints` emits decoded `igPoint2d` records (PSM type
/// `0x005E`) from real `Sheet*` streams.
#[test]
fn igpoints_decoder_emits_decoded_points_with_provenance() {
    let fixtures = [
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
        "export-test/publish-data/A01/A01.pid",
        // Phase 22 micro: D06 contributes 10 decoded igPoint2d records;
        // the floor below is ratcheted from 30 → 40 to reflect this.
        "D06.pid",
    ];
    let mut total_decoded = 0usize;
    let mut per_fixture_summary: Vec<(String, usize)> = Vec::new();
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
            let decoded = decode_igpoints(bytes);
            for p in &decoded {
                assert!(p.byte_range.end <= bytes.len());
                assert_eq!(p.type_code, PSM_TYPE_CODE_IGPOINT2D);
                assert_eq!(p.bytes_to_follow, 34);
                assert!(p.point.0.is_finite() && p.point.1.is_finite());
            }
            per_fixture_count += decoded.len();
        }
        per_fixture_summary.push((fixture.to_string(), per_fixture_count));
        total_decoded += per_fixture_count;
    }
    eprintln!("--- Phase 14 Slice L: PSM igPoint2d decoder cross-fixture summary ---");
    for (name, count) in &per_fixture_summary {
        eprintln!("  {name}: {count} decoded igPoint2d records");
    }
    eprintln!("  total decoded points: {total_decoded}");
    if per_fixture_summary.is_empty() {
        return;
    }
    assert!(
        total_decoded >= 40,
        "decode_igpoints should emit >= 40 points cross-fixture (Phase 22 micro: \
         D06 contributes 10 igPoint2d records), got {total_decoded}"
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
        // Phase 22 micro: D06 contributes 6 decoded igLineString2d polylines;
        // the floor below is ratcheted from 30 → 36 to reflect this.
        "D06.pid",
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
        total_decoded >= 36,
        "decode_iglinestrings should emit >= 36 polylines cross-fixture (Phase 22 micro: \
         D06 contributes 6 igLineString2d records), got {total_decoded}"
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
        // D06 currently has 0 decoded igLine2d records; including it
        // here is a panic-safety / parse-package guard rather than a
        // baseline contribution. See Phase 22 micro follow-up.
        "D06.pid",
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

/// Phase 34-D: cross-fixture ratchet for the fully-typed audit-only
/// PSM `igBoundary2d` (`0x0013`) decoder.
///
/// Grammar evidence: `examples/probe_0013_igboundary2d_grammar.rs` +
/// `docs/analysis/2026-07-07-phase34d-0013-igboundary2d-grammar-decode.md`.
/// The 20 fixture records live in primary `/Sheet6` only, carry 3
/// segments each, close into a loop within `1e-9`, and their trailer
/// member references resolve to canonical `igLine2d` records whose
/// geometry equals the same-index segment in forward order.
#[test]
fn igboundaries_decoder_emits_typed_audit_records_with_provenance() {
    let fixtures = [
        ("DWG-0201GP06-01.pid", 0usize),
        ("DWG-0202GP06-01.pid", 5usize),
        ("工艺管道及仪表流程-1.pid", 10usize),
        ("D06.pid", 0usize),
        ("export-test/publish-data/A01/A01.pid", 0usize),
        (
            "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
            5usize,
        ),
    ];
    let mut total_decoded = 0usize;
    let mut total_expected = 0usize;
    let mut member_matches = 0usize;
    let mut per_fixture_summary: Vec<(String, usize, usize)> = Vec::new();
    let mut sample_records: Vec<String> = Vec::new();

    for (fixture, expected_count) in fixtures {
        let Some(pkg) = parse_test_package(fixture) else {
            continue;
        };
        let mut per_fixture_count = 0usize;
        for sheet in &pkg.parsed.sheet_streams {
            let Some(raw) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            let bytes = raw.data.as_slice();
            let decoded = decode_igboundaries(bytes);
            // Member-resolution oracle: strict igLine2d records by oid.
            let lines_by_oid: BTreeMap<u32, _> = decode_iglines(bytes)
                .into_iter()
                .map(|line| (line.oid, line))
                .collect();
            for boundary in &decoded {
                assert!(
                    boundary.byte_range.end <= bytes.len(),
                    "igBoundary2d byte_range {:?} exceeds stream {} bytes ({})",
                    boundary.byte_range,
                    sheet.path,
                    bytes.len()
                );
                assert_eq!(boundary.type_code, PSM_TYPE_CODE_IGBOUNDARY2D);
                assert_eq!(
                    boundary.bytes_to_follow, 172,
                    "all fixture igBoundary2d records carry 3 segments (btf=172)"
                );
                assert_eq!(boundary.segment_count, 3);
                assert_eq!(boundary.segments.len(), 3);
                assert_eq!(boundary.member_refs.len(), 3);
                assert_eq!(boundary.sub_type_word, 0x0010);
                assert_eq!(boundary.sub_header_tail, [2, 1]);
                assert_eq!(boundary.trailer_flag, 1);
                // The `aux_hi == 12` gate is gone (it admitted one
                // sheet layer and refused the rest). Every boundary
                // reachable from a `Sheet*` stream happens to sit on
                // `Labels` anyway, so the counts above are unchanged;
                // the nine records the gate really refused live in
                // `JSite*/PSMcluster0`, which this pipeline does not
                // scan yet. See
                // `docs/analysis/2026-08-27-aux-hi-is-the-sheet-layer.md`.
                assert_eq!(
                    boundary.sheet_layer_ref, 12,
                    "every Sheet-stream igBoundary2d sits on `Labels` (oid 12): oid={} in {}",
                    boundary.oid, sheet.path
                );
                assert!(
                    boundary.is_closed_loop(1e-9),
                    "fixture igBoundary2d must close into a loop: oid={} in {}",
                    boundary.oid,
                    sheet.path
                );
                // Anchor inside the segment bounding box.
                let xs: Vec<f64> = boundary
                    .segments
                    .iter()
                    .flat_map(|s| [s.start.0, s.end.0])
                    .collect();
                let ys: Vec<f64> = boundary
                    .segments
                    .iter()
                    .flat_map(|s| [s.start.1, s.end.1])
                    .collect();
                let min_x = xs.iter().cloned().fold(f64::INFINITY, f64::min);
                let max_x = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let min_y = ys.iter().cloned().fold(f64::INFINITY, f64::min);
                let max_y = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                assert!(
                    (min_x..=max_x).contains(&boundary.anchor.0)
                        && (min_y..=max_y).contains(&boundary.anchor.1),
                    "anchor must sit inside the segment bbox: oid={}",
                    boundary.oid
                );
                // Every trailer member resolves to a canonical igLine2d
                // whose geometry equals the same-index segment (forward).
                for (i, member) in boundary.member_refs.iter().enumerate() {
                    assert_eq!(
                        member.class_word, 0x00CB,
                        "fixture member class word is always 0x00CB"
                    );
                    let line = lines_by_oid.get(&member.member_oid).unwrap_or_else(|| {
                        panic!(
                            "member oid {} of boundary oid={} must resolve to a decoded \
                             igLine2d in {}",
                            member.member_oid, boundary.oid, sheet.path
                        )
                    });
                    let seg = &boundary.segments[i];
                    let close = |a: f64, b: f64| (a - b).abs() <= 1e-9;
                    assert!(
                        close(seg.start.0, line.start.0)
                            && close(seg.start.1, line.start.1)
                            && close(seg.end.0, line.end.0)
                            && close(seg.end.1, line.end.1),
                        "segment[{i}] of boundary oid={} must equal member igLine2d oid={} \
                         geometry (forward order)",
                        boundary.oid,
                        member.member_oid
                    );
                    member_matches += 1;
                }
                if sample_records.len() < 4 {
                    sample_records.push(format!(
                        "{fixture} {} @ 0x{:06x}..0x{:06x} oid={} parent={} members={:?} \
                         anchor=({:+.4},{:+.4}) closed={}",
                        sheet.path,
                        boundary.byte_range.start,
                        boundary.byte_range.end,
                        boundary.oid,
                        boundary.parent_ref,
                        boundary
                            .member_refs
                            .iter()
                            .map(|m| m.member_oid)
                            .collect::<Vec<_>>(),
                        boundary.anchor.0,
                        boundary.anchor.1,
                        boundary.is_closed_loop(1e-9),
                    ));
                }
            }
            per_fixture_count += decoded.len();
        }
        per_fixture_summary.push((fixture.to_string(), per_fixture_count, expected_count));
        total_decoded += per_fixture_count;
        total_expected += expected_count;
    }
    eprintln!("--- Phase 34-D: PSM igBoundary2d decoder cross-fixture summary ---");
    for (name, count, expected) in &per_fixture_summary {
        eprintln!("  {name}: {count} decoded igBoundary2d records (expected {expected})");
    }
    for sample in &sample_records {
        eprintln!("  sample: {sample}");
    }
    eprintln!(
        "  total decoded igBoundaries across present fixtures: {total_decoded} \
         (member geometry matches: {member_matches})"
    );
    if per_fixture_summary.is_empty() {
        eprintln!(
            "skipping: no Sheet-bearing fixture present (CI / contributors \
            without SmartPlant samples)"
        );
        return;
    }
    for (name, count, expected) in &per_fixture_summary {
        assert_eq!(
            count, expected,
            "igBoundary2d count ratchet drifted for {name}; \
             per-fixture summary: {per_fixture_summary:?}"
        );
    }
    assert_eq!(
        total_decoded, total_expected,
        "igBoundary2d cross-fixture total ratchet drifted"
    );
}

/// Phase 18: cross-fixture ratchet for the conservative PSM `0x0010`
/// sub-record audit-only decoder.
///
/// The probe (`examples/probe_psm_0x0010_shape.rs`) finds 638 matches
/// under a non-advancing scan that counts overlapping hits. This
/// conservative audit-only decoder advances past each matched record
/// (Phase 15 DependencyObject template), yielding 582 cross-fixture
/// records: `DWG-0201=161, DWG-0202=104, 工艺管道-1=306, A01=11`.
/// The decoder admits more records in A01 than the probe because the
/// probe's `max_offset` reserves a 16-byte dump buffer at the end of
/// the stream and skips records there; the decoder only needs the
/// 6-byte header + 8-byte minimum payload.
///
/// Each emitted record must carry stable provenance: byte range fits
/// within the stream, `type_code == 0x0010`, and `raw_payload.len()`
/// matches the declared `bytes_to_follow`.
#[test]
fn sub_records_0x0010_decoder_emits_audit_records_with_provenance() {
    let fixtures = [
        ("DWG-0201GP06-01.pid", 161usize),
        ("DWG-0202GP06-01.pid", 104usize),
        ("工艺管道及仪表流程-1.pid", 306usize),
        ("export-test/publish-data/A01/A01.pid", 11usize),
    ];
    let mut total_decoded = 0usize;
    let mut total_expected = 0usize;
    let mut per_fixture_summary: Vec<(String, usize, usize)> = Vec::new();
    let mut sample_records: Vec<String> = Vec::new();

    for (fixture, expected_count) in fixtures {
        let Some(pkg) = parse_test_package(fixture) else {
            continue;
        };
        let mut per_fixture_count = 0usize;
        for sheet in &pkg.parsed.sheet_streams {
            let Some(raw) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            let bytes = raw.data.as_slice();
            let decoded = decode_sub_records_0x0010(bytes);
            let model_decoded_count = sheet
                .geometry
                .as_ref()
                .map_or(0, |geometry| geometry.decoded_sub_records_0x0010.len());
            assert_eq!(
                model_decoded_count,
                decoded.len(),
                "SheetGeometry audit collection must mirror parser-level 0x0010 count for {} {}",
                fixture,
                sheet.path
            );
            for rec in &decoded {
                assert!(
                    rec.byte_range.end <= bytes.len(),
                    "0x0010 byte_range {:?} exceeds stream {} bytes ({})",
                    rec.byte_range,
                    sheet.path,
                    bytes.len()
                );
                assert!(
                    rec.byte_range.start < rec.byte_range.end,
                    "0x0010 byte_range must be non-empty: {:?}",
                    rec.byte_range
                );
                assert_eq!(rec.type_code, PSM_TYPE_CODE_SUB_RECORD_0X0010);
                assert!(
                    (SUB_RECORD_0X0010_MIN_BYTES_TO_FOLLOW..=SUB_RECORD_0X0010_MAX_BYTES_TO_FOLLOW)
                        .contains(&rec.bytes_to_follow),
                    "0x0010 bytes_to_follow outside audit envelope: {}",
                    rec.bytes_to_follow
                );
                assert_eq!(
                    rec.raw_payload.len(),
                    rec.bytes_to_follow as usize,
                    "0x0010 raw_payload length must equal bytes_to_follow"
                );
                assert_eq!(
                    rec.byte_range.end - rec.byte_range.start,
                    6 + rec.raw_payload.len(),
                    "0x0010 byte_range must cover 6-byte header + payload"
                );

                if sample_records.len() < 6 {
                    sample_records.push(format!(
                        "{fixture} {} @ 0x{:06x}..0x{:06x} type_flags={} bytes_to_follow={} payload_len={}",
                        sheet.path,
                        rec.byte_range.start,
                        rec.byte_range.end,
                        rec.type_flags,
                        rec.bytes_to_follow,
                        rec.raw_payload.len(),
                    ));
                }
            }
            per_fixture_count += decoded.len();
        }

        per_fixture_summary.push((fixture.to_string(), per_fixture_count, expected_count));
        total_decoded += per_fixture_count;
        total_expected += expected_count;
    }

    eprintln!("--- Phase 18: PSM 0x0010 sub-record decoder fixture ratchet ---");
    for (name, actual, expected) in &per_fixture_summary {
        eprintln!("  {name}: {actual} decoded 0x0010 sub-records (expected {expected})");
    }
    for sample in &sample_records {
        eprintln!("  sample: {sample}");
    }
    eprintln!("  total decoded 0x0010 sub-records: {total_decoded}");

    if per_fixture_summary.is_empty() {
        eprintln!(
            "skipping: no Sheet-bearing fixture present (CI / contributors \
            without SmartPlant samples)"
        );
        return;
    }

    for (fixture, actual, expected) in &per_fixture_summary {
        assert_eq!(
            actual, expected,
            "0x0010 sub-record count drift for {fixture}: expected {expected}, got {actual}. \
            Re-run `cargo run --release --example probe_psm_0x0010_shape` before updating this ratchet."
        );
    }
    assert_eq!(
        total_decoded, total_expected,
        "0x0010 aggregate count drift. Per-fixture summary: {per_fixture_summary:?}"
    );
}

/// Phase 19: cross-fixture ratchet for the `leading_word` audit field
/// (`payload[0..2]` as little-endian `u16`) on the Phase 18 PSM
/// `0x0010` sub-record audit collection.
///
/// Probe baseline from `examples/probe_psm_0x0010_sub_kind.rs` over
/// 578 records: `0x0002 = 164 (28%)`, `0x0003 = 21 (3.6%)`,
/// `0x0001 = 18 (3.1%)`. The Phase 18 decoder may admit a few extra
/// records vs the probe (582 vs 578) because the probe carries a small
/// off-by-N at the stream tail; this ratchet uses the decoder count
/// as ground truth and reconciles per-word numbers accordingly.
///
/// Invariants asserted:
/// - `leading_word == None` count = 0 (decoder min payload = 8 ≥ 2).
/// - The three Phase-19-probe-confirmed top words remain the top 3
///   when ranked by count.
/// - Top word `0x0002` covers ≥ 25% of all records.
/// - Total record count matches the Phase 18 ratchet (= 582).
///
/// This ratchet does **not** assert sub-kind semantics; `leading_word`
/// is byte-position-named only (see
/// `goals/phase19-psm-0x0010-leading-word-audit/`).
#[test]
fn sub_records_0x0010_leading_word_distribution_matches_phase19_probe() {
    let fixtures = [
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
        "export-test/publish-data/A01/A01.pid",
    ];
    let mut total_records = 0usize;
    let mut none_count = 0usize;
    let mut leading_word_hist: std::collections::BTreeMap<u16, usize> =
        std::collections::BTreeMap::new();
    let mut per_fixture_totals: Vec<(String, usize)> = Vec::new();

    for fixture in fixtures {
        let Some(pkg) = parse_test_package(fixture) else {
            continue;
        };
        let mut per_fixture = 0usize;
        for sheet in &pkg.parsed.sheet_streams {
            let Some(raw) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            let decoded = decode_sub_records_0x0010(raw.data.as_slice());
            for rec in &decoded {
                total_records += 1;
                per_fixture += 1;
                match rec.leading_word {
                    None => none_count += 1,
                    Some(w) => *leading_word_hist.entry(w).or_insert(0) += 1,
                }
            }
        }
        per_fixture_totals.push((fixture.to_string(), per_fixture));
    }

    if per_fixture_totals.is_empty() {
        eprintln!(
            "skipping: no Sheet-bearing fixture present (CI / contributors \
            without SmartPlant samples)"
        );
        return;
    }

    let mut top_words: Vec<(u16, usize)> =
        leading_word_hist.iter().map(|(w, c)| (*w, *c)).collect();
    top_words.sort_by_key(|(_, c)| std::cmp::Reverse(*c));

    eprintln!("--- Phase 19: PSM 0x0010 leading_word distribution ---");
    for (name, count) in &per_fixture_totals {
        eprintln!("  {name}: {count} records");
    }
    eprintln!("  total records: {total_records}");
    eprintln!("  None (payload.len() < 2): {none_count}");
    eprintln!("  top 6 leading_word values:");
    for (word, count) in top_words.iter().take(6) {
        let pct = (*count as f64) * 100.0 / (total_records.max(1) as f64);
        eprintln!("    0x{word:04X} : {count:>4}  ({pct:.1}%)");
    }

    // Hard invariants:
    // 1. Decoder admits min 8-byte payload, so leading_word is never None.
    assert_eq!(
        none_count, 0,
        "Phase 18 decoder min bytes_to_follow = 8 ≥ 2, so leading_word must never be None"
    );
    // 2. Total must match Phase 18 ratchet count.
    assert_eq!(
        total_records, 582,
        "Phase 19 leading_word ratchet must observe exactly Phase 18's 582 records"
    );
    // 3. The three Phase-19-probe-confirmed dominant words are present
    //    and rank in the top 3 by count.
    let top3: Vec<u16> = top_words.iter().take(3).map(|(w, _)| *w).collect();
    assert_eq!(
        top3,
        vec![0x0002, 0x0003, 0x0001],
        "Phase 19 top-3 leading_word ranking changed: got {top3:?}, expected [0x0002, 0x0003, 0x0001]. \
        Re-run `cargo run --release --example probe_psm_0x0010_sub_kind` to diagnose."
    );
    // 4. Top word coverage ≥ 25% (probe measured 28%).
    let top_count = top_words[0].1;
    let top_pct = (top_count as f64) * 100.0 / (total_records as f64);
    assert!(
        top_pct >= 25.0,
        "Phase 19 leading_word == 0x0002 coverage must be ≥ 25%, got {top_pct:.1}% \
        ({top_count}/{total_records})"
    );
    // 5. Per-fixture sanity: each non-empty fixture must contribute
    //    ≥ 1 record (Phase 18 already enforces specific per-fixture
    //    counts; we don't re-assert them here to avoid double-locking).
    for (fixture, count) in &per_fixture_totals {
        assert!(
            *count >= 1,
            "fixture {fixture} contributed 0 records to leading_word histogram \
            but Phase 18 ratchet expected non-zero"
        );
    }
}

/// Phase 26: PSM 0x0010 attribute-fragment decoder cross-fixture ratchet.
///
/// Asserts the additive attribute decoder extracts engineering text
/// (instrument tags / line numbers / sizes / drawing refs) from the
/// Sheet-bearing fixtures, AND that the raw Phase 18
/// `decoded_sub_records_0x0010` baseline (582) is unchanged — the new
/// decoder is strictly additive. See
/// `docs/analysis/2026-05-31-psm-0x0010-ida-recheck-plan.md`.
#[test]
fn attribute_fragments_extract_engineering_text_cross_fixture() {
    let fixtures = [
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "工艺管道及仪表流程-1.pid",
        "export-test/publish-data/A01/A01.pid",
    ];
    let mut total_fragments = 0usize;
    let mut total_strings = 0usize;
    let mut raw_0x0010 = 0usize;
    let mut per_fixture: Vec<(String, usize, usize)> = Vec::new();
    let mut samples: Vec<String> = Vec::new();

    for fixture in fixtures {
        let Some(pkg) = parse_test_package(fixture) else {
            continue;
        };
        let mut frags = 0usize;
        let mut strs = 0usize;
        for sheet in &pkg.parsed.sheet_streams {
            let Some(raw) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            raw_0x0010 += decode_sub_records_0x0010(raw.data.as_slice()).len();
            for f in &decode_attribute_fragments(raw.data.as_slice()) {
                frags += 1;
                strs += f.strings.len();
                for s in &f.strings {
                    if samples.len() < 24 && s.text.trim().chars().count() >= 2 {
                        samples.push(s.text.clone());
                    }
                }
            }
        }
        total_fragments += frags;
        total_strings += strs;
        per_fixture.push((fixture.to_string(), frags, strs));
    }

    if per_fixture.is_empty() {
        eprintln!("skipping: no Sheet-bearing fixture present");
        return;
    }

    eprintln!("--- Phase 26: PSM 0x0010 attribute fragments ---");
    for (name, frags, strs) in &per_fixture {
        eprintln!("  {name}: {frags} fragments, {strs} strings");
    }
    eprintln!("  total fragments: {total_fragments}, total strings: {total_strings}");
    eprintln!("  raw 0x0010 (Phase 18 path): {raw_0x0010}");
    eprintln!("  samples: {samples:?}");

    if per_fixture.len() == fixtures.len() {
        // All fixtures present: lock the Phase 18 raw baseline (the
        // additive decoder must not change it) plus Phase 26 counts.
        assert_eq!(
            raw_0x0010, 582,
            "Phase 26 attribute decoder must not change the Phase 18 raw 0x0010 baseline (582)"
        );
        assert_eq!(
            total_fragments, 84,
            "Phase 26 attribute fragment baseline drifted (got {total_fragments})"
        );
        assert_eq!(
            total_strings, 84,
            "Phase 26 attribute string baseline drifted (got {total_strings})"
        );
        let counts: Vec<usize> = per_fixture.iter().map(|(_, f, _)| *f).collect();
        assert_eq!(
            counts,
            vec![34, 26, 24, 0],
            "Phase 26 per-fixture attribute baseline drifted: {per_fixture:?}"
        );
    } else {
        // Partial fixture set (some samples absent): soft floor only.
        assert!(
            total_strings >= 1,
            "expected extractable attribute strings, got {total_strings}"
        );
    }
}

/// Phase 29 Slice B: `/PSMcluster0` audit-only body record-chain walker
/// cross-fixture ratchet.
///
/// Triage evidence
/// (`docs/analysis/2026-06-08-phase29-psmcluster0-leftover-triage.md`)
/// proved the post-string-table body of every local fixture is a single
/// continuous PSM-envelope record chain with
/// `chain_records == header.record_count - 2`. This test ratchets:
///
/// 1. the full-coverage chain decodes on every present fixture;
/// 2. the `record_count - 2` invariant holds;
/// 3. the byte-audit consumed ratio for `/PSMcluster0` is ≥ 0.99.
#[test]
fn psmcluster0_body_chain_matches_record_count_invariant() {
    let fixtures = [
        "D06.pid",
        "工艺管道及仪表流程-1.pid",
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "export-test/publish-data/A01/A01.pid",
        "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
    ];
    let mut checked = 0usize;
    for fixture in fixtures {
        let Some(pkg) = parse_test_package(fixture) else {
            continue;
        };
        let Some(stream) = pkg.streams.get("/PSMcluster0") else {
            eprintln!("skipping: {fixture} has no /PSMcluster0 stream");
            continue;
        };
        let data = stream.data.as_slice();
        let header = pid_parse::parsers::cluster_header::parse_header(data)
            .unwrap_or_else(|| panic!("{fixture}: /PSMcluster0 must carry a cluster header"));
        let records = pid_parse::parsers::cluster_header::decode_psm_cluster0_body_records(data);
        assert!(
            !records.is_empty(),
            "{fixture}: body chain must decode under the full-coverage gate"
        );
        assert_eq!(
            records.len() as u32,
            header.record_count.saturating_sub(2),
            "{fixture}: chain_records must equal header.record_count - 2"
        );
        assert_eq!(
            records.last().map(|r| r.byte_range.end),
            Some(data.len()),
            "{fixture}: chain must run exactly to end-of-stream"
        );

        let report = pid_parse::byte_audit_report(&pkg);
        let summary = report
            .per_stream
            .get("/PSMcluster0")
            .unwrap_or_else(|| panic!("{fixture}: byte-audit must cover /PSMcluster0"));
        let ratio = summary.consumed_bytes as f64 / data.len() as f64;
        assert!(
            ratio >= 0.99,
            "{fixture}: /PSMcluster0 consumed ratio must be >= 0.99 after the walker, got {ratio}"
        );
        eprintln!(
            "{fixture}: records={} record_count={} consumed_ratio={ratio:.4}",
            records.len(),
            header.record_count
        );
        checked += 1;
    }
    if checked == 0 {
        eprintln!("skipping: no fixture with /PSMcluster0 present");
    }
}

/// Phase 29 Slice B follow-up: `/StyleCluster` audit-only body
/// record-chain walker cross-fixture ratchet.
///
/// Probe evidence (triage doc) shows every fixture's `/StyleCluster`
/// body ends in a single record chain that runs exactly to
/// end-of-stream after a variable-length unparsed prefix. Unlike
/// `/PSMcluster0`, `header.record_count` does not consistently match
/// the chain length, so this test ratchets only structure:
///
/// 1. a qualifying chain (>= 3 records) decodes on every present fixture;
/// 2. the chain is end-anchored;
/// 3. the byte-audit consumed ratio for `/StyleCluster` reaches >= 0.6.
#[test]
fn stylecluster_body_chain_is_end_anchored_across_fixtures() {
    let fixtures = [
        "D06.pid",
        "工艺管道及仪表流程-1.pid",
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "export-test/publish-data/A01/A01.pid",
        "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
    ];
    let mut checked = 0usize;
    for fixture in fixtures {
        let Some(pkg) = parse_test_package(fixture) else {
            continue;
        };
        let Some(stream) = pkg.streams.get("/StyleCluster") else {
            eprintln!("skipping: {fixture} has no /StyleCluster stream");
            continue;
        };
        let data = stream.data.as_slice();
        let records = pid_parse::parsers::cluster_header::decode_style_cluster_body_records(data);
        assert!(
            records.len() >= 3,
            "{fixture}: /StyleCluster must decode a qualifying chain, got {} records",
            records.len()
        );
        assert_eq!(
            records.last().map(|r| r.byte_range.end),
            Some(data.len()),
            "{fixture}: chain must be end-anchored"
        );
        assert!(
            records.first().map(|r| r.byte_range.start) > Some(16),
            "{fixture}: chain must start after the 16-byte header"
        );

        let report = pid_parse::byte_audit_report(&pkg);
        let summary = report
            .per_stream
            .get("/StyleCluster")
            .unwrap_or_else(|| panic!("{fixture}: byte-audit must cover /StyleCluster"));
        let ratio = summary.consumed_bytes as f64 / data.len() as f64;
        assert!(
            ratio >= 0.6,
            "{fixture}: /StyleCluster consumed ratio must be >= 0.6 after the walker, got {ratio}"
        );
        eprintln!(
            "{fixture}: records={} chain_start={} consumed_ratio={ratio:.4}",
            records.len(),
            records.first().map(|r| r.byte_range.start).unwrap_or(0)
        );
        checked += 1;
    }
    if checked == 0 {
        eprintln!("skipping: no fixture with /StyleCluster present");
    }
}

/// Phase 29 Slice C ratchet: the `/Unclustered Dynamic Attributes`
/// stream body is an 8-byte prologue (cluster magic + u32 counter)
/// plus a single end-anchored `0x0089` record chain on every local
/// fixture, and the audit-only walker plus the landmark scanner
/// account for every byte (leftover 0). Chain lengths are pinned to
/// the Phase 29 Slice C triage probe results
/// (`docs/analysis/2026-06-08-phase29-dynamic-attributes-body-backlog.md`).
#[test]
fn da_body_chain_is_end_anchored_across_fixtures() {
    let fixtures: [(&str, usize); 6] = [
        ("D06.pid", 47),
        ("工艺管道及仪表流程-1.pid", 69),
        ("DWG-0201GP06-01.pid", 231),
        ("DWG-0202GP06-01.pid", 169),
        ("export-test/publish-data/A01/A01.pid", 22),
        (
            "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
            169,
        ),
    ];
    let mut checked = 0usize;
    for (fixture, expected_records) in fixtures {
        let Some(pkg) = parse_test_package(fixture) else {
            continue;
        };
        let Some(stream) = pkg.streams.get("/Unclustered Dynamic Attributes") else {
            eprintln!("skipping: {fixture} has no /Unclustered Dynamic Attributes stream");
            continue;
        };
        let data = stream.data.as_slice();
        let records = pid_parse::parsers::cluster_header::decode_unclustered_da_body_records(data);
        assert_eq!(
            records.len(),
            expected_records,
            "{fixture}: DA body chain length must match the Slice C triage probe"
        );
        assert_eq!(
            records.first().map(|r| r.byte_range.start),
            Some(pid_parse::parsers::cluster_header::UNCLUSTERED_DA_PROLOGUE_LEN),
            "{fixture}: chain must start right after the 8-byte prologue"
        );
        assert_eq!(
            records.last().map(|r| r.byte_range.end),
            Some(data.len()),
            "{fixture}: chain must be end-anchored"
        );
        assert!(
            records.iter().all(|r| r.type_code == 0x0089),
            "{fixture}: every DA body record masks to type code 0x0089"
        );

        let report = pid_parse::byte_audit_report(&pkg);
        let summary = report
            .per_stream
            .get("/Unclustered Dynamic Attributes")
            .unwrap_or_else(|| {
                panic!("{fixture}: byte-audit must cover /Unclustered Dynamic Attributes")
            });
        assert_eq!(
            summary.parser_name.as_deref(),
            Some("parse_unclustered_da"),
            "{fixture}: DA branch must run the combined walker + landmark parser"
        );
        assert_eq!(
            summary.leftover_bytes, 0,
            "{fixture}: walker + landmarks must account for every DA byte"
        );
        assert_eq!(summary.consumed_bytes, data.len() as u64);
        eprintln!(
            "{fixture}: records={} stream_bytes={} leftover=0",
            records.len(),
            data.len()
        );
        checked += 1;
    }
    if checked == 0 {
        eprintln!("skipping: no fixture with /Unclustered Dynamic Attributes present");
    }
}

/// Phase 29 nested follow-up ratchet: every one-level nested
/// `JSite*/PSMcluster0`, `JSite*/StyleCluster`, and
/// `JSite*/Unclustered Dynamic Attributes` stream reuses its top-level
/// twin's body layout (probe: 23/23 streams end-anchored), and the
/// byte-audit dispatches them to the full audit-only walkers.
#[test]
fn nested_jsite_cluster_bodies_are_end_anchored_across_fixtures() {
    let fixtures = [
        "D06.pid",
        "工艺管道及仪表流程-1.pid",
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "export-test/publish-data/A01/A01.pid",
        "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
    ];
    let mut checked = 0usize;
    for fixture in fixtures {
        let Some(pkg) = parse_test_package(fixture) else {
            continue;
        };
        let report = pid_parse::byte_audit_report(&pkg);
        for (path, stream) in &pkg.streams {
            let trimmed = path.trim_start_matches('/');
            let Some((parent, child)) = trimmed.split_once('/') else {
                continue;
            };
            if !parent.starts_with("JSite") || child.contains('/') {
                continue;
            }
            let data = stream.data.as_slice();
            let summary = report
                .per_stream
                .get(path)
                .unwrap_or_else(|| panic!("{fixture}: byte-audit must cover {path}"));
            match child {
                "PSMcluster0" => {
                    let records =
                        pid_parse::parsers::cluster_header::decode_psm_cluster0_body_records(data);
                    let header = pid_parse::parsers::cluster_header::parse_header(data)
                        .unwrap_or_else(|| panic!("{fixture}: {path} must have a cluster header"));
                    assert_eq!(
                        records.len() as u32,
                        header.record_count - 2,
                        "{fixture}: {path} must keep the record_count - 2 invariant"
                    );
                    assert_eq!(
                        records.last().map(|r| r.byte_range.end),
                        Some(data.len()),
                        "{fixture}: {path} chain must be end-anchored"
                    );
                    assert_eq!(
                        summary.parser_name.as_deref(),
                        Some("parse_psm_cluster0"),
                        "{fixture}: {path} must dispatch to the full walker"
                    );
                    assert_eq!(
                        summary.leftover_bytes, 0,
                        "{fixture}: {path} must be fully accounted"
                    );
                }
                "StyleCluster" => {
                    let records =
                        pid_parse::parsers::cluster_header::decode_style_cluster_body_records(data);
                    assert!(
                        records.len() >= 3,
                        "{fixture}: {path} must decode a qualifying chain"
                    );
                    assert_eq!(
                        records.last().map(|r| r.byte_range.end),
                        Some(data.len()),
                        "{fixture}: {path} chain must be end-anchored"
                    );
                    assert_eq!(
                        summary.parser_name.as_deref(),
                        Some("parse_style_cluster"),
                        "{fixture}: {path} must dispatch to the full walker"
                    );
                    let chain_start = records.first().map(|r| r.byte_range.start).unwrap_or(0);
                    assert_eq!(
                        summary.leftover_bytes,
                        (chain_start - 16) as u64,
                        "{fixture}: {path} leftover must be exactly the unparsed prefix"
                    );
                }
                "Unclustered Dynamic Attributes" => {
                    let records =
                        pid_parse::parsers::cluster_header::decode_unclustered_da_body_records(
                            data,
                        );
                    assert!(
                        !records.is_empty(),
                        "{fixture}: {path} must decode an end-anchored chain"
                    );
                    assert_eq!(
                        summary.parser_name.as_deref(),
                        Some("parse_unclustered_da"),
                        "{fixture}: {path} must dispatch to the full walker"
                    );
                    assert_eq!(
                        summary.leftover_bytes, 0,
                        "{fixture}: {path} must be fully accounted"
                    );
                }
                _ => continue,
            }
            eprintln!(
                "{fixture}: {path} bytes={} leftover={}",
                data.len(),
                summary.leftover_bytes
            );
            checked += 1;
        }
    }
    if checked == 0 {
        eprintln!("skipping: no fixture with nested JSite cluster streams present");
    }
}

/// Phase 29 Slice L ratchet: nested `JSite*` registry child streams
/// reuse the top-level stream parsers byte-for-byte. DocVersion2/3,
/// PSMclustertable, and PSMsegmenttable parse fully; PSMroots keeps the
/// same 4-byte tail as its top-level twin; 4-byte AppObject stubs gate
/// out (registered, zero claim); the JSite204 summary pair parses
/// partially like its top-level twin.
#[test]
fn nested_jsite_registry_streams_reuse_top_level_parsers() {
    let fixtures = [
        "D06.pid",
        "工艺管道及仪表流程-1.pid",
        "DWG-0201GP06-01.pid",
        "DWG-0202GP06-01.pid",
        "export-test/publish-data/A01/A01.pid",
        "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
    ];
    let mut checked = 0usize;
    for fixture in fixtures {
        let Some(pkg) = parse_test_package(fixture) else {
            continue;
        };
        let report = pid_parse::byte_audit_report(&pkg);
        for (path, stream) in &pkg.streams {
            let trimmed = path.trim_start_matches('/');
            let Some((parent, child)) = trimmed.split_once('/') else {
                continue;
            };
            if !parent.starts_with("JSite") || child.contains('/') {
                continue;
            }
            let expected = match child {
                "PSMclustertable" => ("parse_psm_cluster_table", Some(0)),
                "PSMroots" => ("parse_psm_roots", Some(4)),
                "PSMsegmenttable" => ("parse_psm_segment_table", Some(0)),
                "DocVersion2" => ("parse_doc_version2", Some(0)),
                "DocVersion3" => ("parse_doc_version3", Some(0)),
                "AppObject" => ("parse_app_object", None),
                "\u{5}SummaryInformation" | "\u{5}DocumentSummaryInformation" => {
                    ("parse_summary_property_set", None)
                }
                _ => continue,
            };
            let summary = report
                .per_stream
                .get(path)
                .unwrap_or_else(|| panic!("{fixture}: byte-audit must cover {path}"));
            assert_eq!(
                summary.parser_name.as_deref(),
                Some(expected.0),
                "{fixture}: {path} must dispatch to the top-level parser"
            );
            if let Some(expected_leftover) = expected.1 {
                assert_eq!(
                    summary.leftover_bytes, expected_leftover,
                    "{fixture}: {path} leftover drifted"
                );
            }
            if child.starts_with('\u{5}') {
                assert!(
                    summary.consumed_bytes > 0,
                    "{fixture}: {path} summary stream must parse partially"
                );
            }
            eprintln!(
                "{fixture}: {} bytes={} consumed={} leftover={}",
                path.replace('\u{5}', "\\x05"),
                stream.data.len(),
                summary.consumed_bytes,
                summary.leftover_bytes
            );
            checked += 1;
        }
    }
    if checked == 0 {
        eprintln!("skipping: no fixture with nested JSite registry streams present");
    }
}

/// Phase 29 Slice M ratchet: the top-level `/JSitesList` stream is
/// `"OLEM"` magic + `u32` count + `count × u32` entries with an exact
/// stream-length match, and the entry values correlate with `JSite<id>`
/// storages in the same package. The 0-byte `/TaggedTxtData/Revision`
/// placeholder is registered with zero claims.
#[test]
fn jsites_list_parses_with_exact_size_and_matches_jsite_storages() {
    let fixtures: [(&str, u32, usize); 6] = [
        ("D06.pid", 9, 0),
        ("工艺管道及仪表流程-1.pid", 10, 0),
        ("DWG-0201GP06-01.pid", 20, 0),
        ("DWG-0202GP06-01.pid", 13, 3),
        ("export-test/publish-data/A01/A01.pid", 5, 0),
        (
            "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
            13,
            3,
        ),
    ];
    let mut checked = 0usize;
    for (fixture, expected_count, expected_trailing) in fixtures {
        let Some(pkg) = parse_test_package(fixture) else {
            continue;
        };
        let Some(stream) = pkg.streams.get("/JSitesList") else {
            eprintln!("skipping: {fixture} has no /JSitesList stream");
            continue;
        };
        let data = stream.data.as_slice();
        let list = pid_parse::parsers::jsites_list::parse_jsites_list(data)
            .unwrap_or_else(|| panic!("{fixture}: /JSitesList must pass the slot-table gate"));
        assert_eq!(
            list.count, expected_count,
            "{fixture}: /JSitesList entry count drifted"
        );
        assert_eq!(
            list.trailing_slots.len(),
            expected_trailing,
            "{fixture}: stale trailing slot count drifted"
        );
        assert_eq!(
            data.len(),
            8 + 4 * (list.entries.len() + list.trailing_slots.len())
        );

        let storage_matches = list
            .entries
            .iter()
            .filter(|id| {
                pkg.streams
                    .keys()
                    .any(|p| p.starts_with(&format!("/JSite{id}/")))
            })
            .count();
        assert!(
            storage_matches > 0,
            "{fixture}: at least one /JSitesList entry must match a JSite storage id"
        );

        let report = pid_parse::byte_audit_report(&pkg);
        let summary = report
            .per_stream
            .get("/JSitesList")
            .unwrap_or_else(|| panic!("{fixture}: byte-audit must cover /JSitesList"));
        assert_eq!(summary.parser_name.as_deref(), Some("parse_jsites_list"));
        assert_eq!(
            summary.leftover_bytes,
            (4 * expected_trailing) as u64,
            "{fixture}: only stale trailing slots may stay leftover"
        );
        if let Some(revision) = report.per_stream.get("/TaggedTxtData/Revision") {
            assert_eq!(
                revision.parser_name.as_deref(),
                Some("revision_empty_stream"),
                "{fixture}: Revision placeholder must be registered"
            );
            assert_eq!(revision.total_bytes, 0, "{fixture}: Revision must be empty");
        }
        eprintln!(
            "{fixture}: entries={} storage_matches={storage_matches}",
            list.entries.len()
        );
        checked += 1;
    }
    if checked == 0 {
        eprintln!("skipping: no fixture with /JSitesList present");
    }
}

/// Phase 29 Slice K ratchet: the chain-scoped DA attribute extraction
/// (`parse_attribute_records_chain_scoped`) is at least as complete as
/// the legacy whole-stream scan on every fixture, and recovers the
/// flagged-head record the byte-literal scan misses on
/// `工艺管道及仪表流程-1.pid`.
#[test]
fn da_chain_scoped_attribute_extraction_matches_or_beats_legacy_scan() {
    let fixtures: [(&str, usize); 6] = [
        ("D06.pid", 47),
        ("工艺管道及仪表流程-1.pid", 69),
        ("DWG-0201GP06-01.pid", 231),
        ("DWG-0202GP06-01.pid", 169),
        ("export-test/publish-data/A01/A01.pid", 22),
        (
            "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
            169,
        ),
    ];
    let mut checked = 0usize;
    for (fixture, expected_records) in fixtures {
        let Some(pkg) = parse_test_package(fixture) else {
            continue;
        };
        let Some(stream) = pkg.streams.get("/Unclustered Dynamic Attributes") else {
            continue;
        };
        let data = stream.data.as_slice();
        let (legacy, _) = pid_parse::parsers::dynamic_attr_records::parse_attribute_records(data);
        let (scoped, _) =
            pid_parse::parsers::dynamic_attr_records::parse_attribute_records_chain_scoped(data);
        assert!(
            scoped.len() >= legacy.len(),
            "{fixture}: chain-scoped extraction must never lose records (legacy {} vs scoped {})",
            legacy.len(),
            scoped.len()
        );
        assert_eq!(
            scoped.len(),
            expected_records,
            "{fixture}: chain-scoped record count drifted"
        );
        let da = pkg
            .parsed
            .dynamic_attributes
            .as_ref()
            .expect("DA blob must parse");
        assert_eq!(
            da.attribute_records.len(),
            scoped.len(),
            "{fixture}: document pipeline must use the chain-scoped extraction"
        );
        eprintln!(
            "{fixture}: legacy={} chain_scoped={}",
            legacy.len(),
            scoped.len()
        );
        checked += 1;
    }
    if checked == 0 {
        eprintln!("skipping: no fixture with /Unclustered Dynamic Attributes present");
    }
}

/// The page each fixture states through its own `0x003D` border frame,
/// ratcheted against the measured table in
/// `docs/analysis/2026-07-27-smartframe-003d-native-reader.md`.
///
/// The ratchet is on the measurement rather than on a nominal size, because
/// that is the difference this evidence makes: a template name can only ever
/// produce the ISO nominal, three of these five sheets differ from it, and
/// two of them carry no template name at all.
#[test]
fn every_fixture_states_its_page_through_its_own_border_frame() {
    const MEASURED_PAGES_MM: &[(&str, f64, f64)] = &[
        ("DWG-0201GP06-01.pid", 594.3, 420.3),
        ("DWG-0202GP06-01.pid", 593.7, 419.6),
        ("工艺管道及仪表流程-1.pid", 841.0, 594.0),
        ("D06.pid", 594.3, 420.6),
        ("export-test/publish-data/A01/A01.pid", 594.0, 420.0),
    ];
    // The table is quoted to a tenth of a millimetre.
    const TOLERANCE_MM: f64 = 0.05;

    let mut checked = 0usize;
    for (fixture, expected_width, expected_height) in MEASURED_PAGES_MM {
        let Some(doc) = parse_test_file(fixture) else {
            continue;
        };
        let geometry = pid_parse::build_normalized_geometry(&doc);
        let (width, height) = geometry
            .page_dimensions_mm
            .unwrap_or_else(|| panic!("{fixture}: border frame states a page"));

        assert!(
            (width - expected_width).abs() <= TOLERANCE_MM
                && (height - expected_height).abs() <= TOLERANCE_MM,
            "{fixture}: page is {width:.3} x {height:.3} mm, the analysis table measured \
             {expected_width:.1} x {expected_height:.1}"
        );
        assert!(
            !geometry
                .warnings
                .iter()
                .any(|warning| warning.contains("page transforms are unavailable")),
            "{fixture}: the page is decoded, so nothing may still report it unavailable"
        );
        eprintln!("{fixture}: page {width:.3} x {height:.3} mm");
        checked += 1;
    }

    assert!(
        checked > 0,
        "no fixture with a border frame was available to check"
    );
}

/// Decoded coordinates are metres on the page the border frame states; the
/// raw coordinate hints are not, and must keep saying so.
///
/// The two are in different spaces: a decoded record carries the drawing's
/// normalized values, while a coordinate hint is a raw source pair that
/// reaches the +/-900k range. Promoting both would claim a point 900km off
/// the sheet is on it.
#[test]
fn only_decoded_coordinates_carry_the_page_the_border_frame_states() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else {
        return;
    };
    let geometry = pid_parse::build_normalized_geometry(&doc);

    let mut decoded = 0usize;
    let mut raw_hints = 0usize;
    for entity in &geometry.entities {
        let available = matches!(
            entity.coordinate_context.page_transform,
            pid_parse::PidPageTransform::Available { .. }
        );
        if entity.confidence == pid_parse::PidGeometryConfidence::Decoded {
            decoded += 1;
            assert!(available, "decoded entity {} lost its page", entity.id);
            assert_eq!(
                entity.coordinate_context.units,
                pid_parse::PidDrawingUnits::Known { unit: "m".into() },
                "decoded entity {} lost its unit",
                entity.id
            );
        } else if entity.id.contains(":coordinate-hint:") {
            raw_hints += 1;
            assert!(
                !available,
                "raw coordinate hint {} was promoted onto the page",
                entity.id
            );
        }
    }

    assert!(decoded > 0, "DWG-0201 has decoded geometry");
    assert!(raw_hints > 0, "DWG-0201 has raw coordinate hints");
    eprintln!("page promotion: decoded={decoded}, raw_hints_left_alone={raw_hints}");
}
