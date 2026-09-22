//! The `Geometry` parse profile draws what `Full` draws.
//!
//! `ParseProfile::Geometry` (plan `docs/plans/2026-09-21-a-geometry-parse-profile.md`)
//! runs the passes a renderer needs and skips the ones only an inspector or a
//! probe reads. The claim that makes it safe to hand `OpenCADStudio` is not
//! "it is faster" but "nothing it skips reaches the drawing": every Decoded
//! entity, every Inferred line (the connectivity links, whose endpoint
//! positions ride on the geometry hints, the object graph and the
//! cross-reference -- which is why those three run under `Geometry`), the
//! cached symbol bodies, the page size and both loss censuses must come out
//! identical to `Full`'s. What may be missing is exactly the probe yield:
//! `ProbeOnly` evidence and the inferred points the coordinate probe promotes,
//! neither of which a renderer draws (G1, 2026-09-22).
//!
//! Soft-skips a fixture that is not checked out, like the other real-file
//! suites.

use std::path::Path;
use std::time::Instant;

use pid_parse::{
    build_normalized_geometry, NormalizedPidGeometry, ParseOptions, PidGeometryConfidence,
    PidGraphicEntity, PidGraphicKind, PidParser,
};

const FIXTURES: [&str; 5] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
];

/// The entities a renderer draws: everything Decoded, and every Inferred
/// entity that is not a bare point. `OpenCADStudio` draws no inferred point
/// (`src/io/pid.rs`, module doc: of the 321 across the fixtures none is
/// drawing content) and no `ProbeOnly` evidence at all.
fn drawn(geometry: &NormalizedPidGeometry) -> Vec<&PidGraphicEntity> {
    geometry
        .entities
        .iter()
        .filter(|entity| match entity.confidence {
            PidGeometryConfidence::Decoded => true,
            PidGeometryConfidence::Inferred => !matches!(entity.kind, PidGraphicKind::Point { .. }),
            PidGeometryConfidence::ProbeOnly => false,
        })
        .collect()
}

fn record_prefix(entity: &PidGraphicEntity) -> &str {
    entity
        .source
        .record_id
        .as_deref()
        .and_then(|id| id.split(':').next())
        .unwrap_or("")
}

#[test]
fn the_geometry_profile_draws_what_full_draws_and_skips_only_probe_yield() {
    let mut checked = 0usize;
    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.is_file() {
            eprintln!("skipping {fixture}: not checked out");
            continue;
        }

        // Geometry first, so the Full parse -- not the one under test --
        // is the one that benefits from a warm file cache.
        let started = Instant::now();
        let geometry = PidParser::with_options(ParseOptions::geometry())
            .parse_file(path)
            .unwrap_or_else(|error| panic!("{fixture}: Geometry parse: {error}"));
        let geometry_elapsed = started.elapsed();
        let started = Instant::now();
        let full = PidParser::new()
            .parse_file(path)
            .unwrap_or_else(|error| panic!("{fixture}: Full parse: {error}"));
        let full_elapsed = started.elapsed();
        eprintln!(
            "{fixture}: parse_file Full {:.0} ms, Geometry {:.0} ms",
            full_elapsed.as_secs_f64() * 1000.0,
            geometry_elapsed.as_secs_f64() * 1000.0
        );

        // What the profile skipped is absent, what it kept is Full's.
        assert!(geometry.summary.is_none(), "{fixture}: no summary");
        assert!(
            geometry.object_inventory.is_none(),
            "{fixture}: no inventory"
        );
        assert!(geometry.layout.is_none(), "{fixture}: no layout");
        assert!(geometry.doc_version2.is_none(), "{fixture}: no DocVersion2");
        assert!(
            geometry.app_object_registry.is_none() && geometry.version_history.is_none(),
            "{fixture}: no document registry"
        );
        assert!(
            geometry.unknown_streams.is_empty(),
            "{fixture}: no unknown-stream diagnostics"
        );
        assert!(
            geometry.cross_reference.is_some(),
            "{fixture}: the cross-reference runs -- the connectivity links need it"
        );
        assert_eq!(
            geometry.sheet_layers, full.sheet_layers,
            "{fixture}: sheet layers"
        );
        assert_eq!(
            geometry.view_filter_sets, full.view_filter_sets,
            "{fixture}: view filter sets"
        );
        assert_eq!(
            geometry.style_tables, full.style_tables,
            "{fixture}: style tables"
        );
        assert_eq!(
            serde_json::to_value(&geometry.drawing_meta).expect("serialises"),
            serde_json::to_value(&full.drawing_meta).expect("serialises"),
            "{fixture}: drawing metadata"
        );
        assert_eq!(
            geometry.jsites.len(),
            full.jsites.len(),
            "{fixture}: every JSite decoded"
        );
        for (g, f) in geometry.jsites.iter().zip(&full.jsites) {
            assert_eq!(
                g.nested_geometry, f.nested_geometry,
                "{fixture}: {} cached body",
                g.path
            );
            assert_eq!(
                g.stroke_styles, f.stroke_styles,
                "{fixture}: {} stroke styles",
                g.path
            );
            assert_eq!(
                g.symbol_information, f.symbol_information,
                "{fixture}: {} chain",
                g.path
            );
            assert_eq!(
                g.symbol_path, f.symbol_path,
                "{fixture}: {} symbol path",
                g.path
            );
        }
        assert_eq!(
            geometry.sheet_streams.len(),
            full.sheet_streams.len(),
            "{fixture}: every sheet stream listed"
        );
        for (g, f) in geometry.sheet_streams.iter().zip(&full.sheet_streams) {
            assert_eq!(g.path, f.path);
            assert_eq!(
                serde_json::to_value(&g.endpoint_records).expect("serialises"),
                serde_json::to_value(&f.endpoint_records).expect("serialises"),
                "{fixture}: {} endpoints",
                g.path
            );
            let Some(gg) = g.geometry.as_ref() else {
                // No probes, so `None` means no family record decoded -- and
                // then Full decoded none either.
                if let Some(fg) = f.geometry.as_ref() {
                    assert!(
                        fg.decoded_iglines.is_empty()
                            && fg.decoded_igpoints.is_empty()
                            && fg.decoded_iglinestrings.is_empty()
                            && fg.decoded_igtextboxes.is_empty()
                            && fg.decoded_igsymbols.is_empty()
                            && fg.decoded_igboundaries.is_empty(),
                        "{fixture}: {} has records under Full and none under Geometry",
                        g.path
                    );
                }
                continue;
            };
            let fg = f
                .geometry
                .as_ref()
                .unwrap_or_else(|| panic!("{fixture}: {} decoded under Geometry only", g.path));
            assert!(
                gg.texts.is_empty(),
                "{fixture}: {} text probe skipped",
                g.path
            );
            assert!(
                gg.coordinate_hints.is_empty(),
                "{fixture}: {} coordinate probe skipped",
                g.path
            );
            assert!(
                gg.spatial_analysis.is_none(),
                "{fixture}: {} spatial analysis skipped",
                g.path
            );
            assert_eq!(
                gg.decoded_iglines, fg.decoded_iglines,
                "{fixture}: {} lines",
                g.path
            );
            assert_eq!(
                gg.decoded_igpoints, fg.decoded_igpoints,
                "{fixture}: {} points",
                g.path
            );
            assert_eq!(
                gg.decoded_iglinestrings, fg.decoded_iglinestrings,
                "{fixture}: {} line strings",
                g.path
            );
            assert_eq!(
                gg.decoded_igtextboxes, fg.decoded_igtextboxes,
                "{fixture}: {} text boxes",
                g.path
            );
            assert_eq!(
                gg.decoded_igsymbols, fg.decoded_igsymbols,
                "{fixture}: {} symbols",
                g.path
            );
            assert_eq!(
                gg.decoded_igboundaries, fg.decoded_igboundaries,
                "{fixture}: {} boundaries",
                g.path
            );
            assert_eq!(
                gg.undecoded_type_codes, fg.undecoded_type_codes,
                "{fixture}: {} undecoded census",
                g.path
            );
            assert_eq!(
                gg.refused_records, fg.refused_records,
                "{fixture}: {} refused census",
                g.path
            );
            assert_eq!(
                gg.endpoints, fg.endpoints,
                "{fixture}: {} endpoint rescan",
                g.path
            );
            assert_eq!(
                gg.object_geometry_hints, fg.object_geometry_hints,
                "{fixture}: {} geometry hints -- the connectivity links' positions",
                g.path
            );
        }

        // The projection: what is drawn is identical, and what is missing is
        // probe yield only.
        let full_geometry = build_normalized_geometry(&full);
        let profile_geometry = build_normalized_geometry(&geometry);
        assert_eq!(
            drawn(&profile_geometry),
            drawn(&full_geometry),
            "{fixture}: the drawn entities differ"
        );
        assert_eq!(
            profile_geometry.symbol_definitions, full_geometry.symbol_definitions,
            "{fixture}: symbol definitions"
        );
        assert_eq!(
            profile_geometry.page_dimensions_mm, full_geometry.page_dimensions_mm,
            "{fixture}: page size"
        );
        assert_eq!(
            profile_geometry.dropped_graphic_records, full_geometry.dropped_graphic_records,
            "{fixture}: dropped census"
        );
        assert_eq!(
            profile_geometry.refused_graphic_records, full_geometry.refused_graphic_records,
            "{fixture}: refused census"
        );
        for entity in &full_geometry.entities {
            if profile_geometry.entities.contains(entity) {
                continue;
            }
            let probe_yield = match entity.confidence {
                PidGeometryConfidence::ProbeOnly => true,
                PidGeometryConfidence::Inferred => {
                    matches!(entity.kind, PidGraphicKind::Point { .. })
                        && record_prefix(entity) == "coordinate-hint"
                }
                PidGeometryConfidence::Decoded => false,
            };
            assert!(
                probe_yield,
                "{fixture}: Geometry lacks {} ({:?}, {}), which is not probe yield",
                entity.id,
                entity.confidence,
                record_prefix(entity)
            );
        }
        for entity in &profile_geometry.entities {
            assert!(
                full_geometry.entities.contains(entity),
                "{fixture}: Geometry has {} that Full does not",
                entity.id
            );
        }
        let drawn_count = drawn(&full_geometry).len();
        assert!(
            drawn_count > 0,
            "{fixture}: nothing drawn, so agreement proves nothing"
        );
        eprintln!(
            "{fixture}: {drawn_count} drawn entities agree; Full {} entities, Geometry {}",
            full_geometry.entities.len(),
            profile_geometry.entities.len()
        );
        checked += 1;
    }
    eprintln!(
        "the_geometry_profile_draws_what_full_draws: {checked}/{} fixtures",
        FIXTURES.len()
    );
}

/// The three profiles answer the pass gates the way their documentation
/// says, so a reader that asks the gates gets the profile it was handed.
#[test]
fn the_pass_gates_answer_for_each_profile() {
    let full = ParseOptions::default();
    let light = ParseOptions::light();
    let geometry = ParseOptions::geometry();

    assert!(full.runs_summary() && light.runs_summary() && !geometry.runs_summary());
    assert!(full.runs_tagged_text() && !light.runs_tagged_text() && geometry.runs_tagged_text());
    assert!(full.runs_jsites() && !light.runs_jsites() && geometry.runs_jsites());
    assert!(full.runs_sheet_probes() && light.runs_sheet_probes() && !geometry.runs_sheet_probes());
    assert!(
        full.runs_semantic_passes()
            && !light.runs_semantic_passes()
            && geometry.runs_semantic_passes()
    );
    assert!(full.runs_registry() && !light.runs_registry() && !geometry.runs_registry());
    assert!(
        full.runs_derived_passes()
            && !light.runs_derived_passes()
            && !geometry.runs_derived_passes()
    );
    assert!(!geometry.scan_strings && !geometry.keep_unknown_streams);
}
