//! Two-phase Sheet geometry construction with an ephemeral probe cache.

use crate::model::{
    decode_all_families_into, sheet_geometry_has_no_family_records, DecodedSpatialAnalysis,
    SheetCoordinateHintDto, SheetGeometry, SheetObjectGeometryHint, SheetText,
};
use crate::parsers::{
    sheet_probe::{
        field_x_window_features, field_x_window_identities, field_x_windows,
        populate_object_geometry_hints, probe_sheet_stream,
        score_field_x_window_features_with_identities, SheetChunk, SheetIdentityIndex,
        SheetProbeOptions, SheetProbeReport, SheetTextEncoding,
    },
    sheet_records::{
        collect_normalized_f64_pairs, coordinate_pair_spatial_analysis,
        SPATIAL_ANALYSIS_DEFAULT_GRID_N,
    },
};
use std::collections::HashSet;

/// Probe evidence retained only until the post-cross-reference hint phase.
pub(crate) struct SheetProbeCache {
    pub(crate) chunks: Vec<SheetChunk>,
}

/// Initial Sheet result plus the ephemeral evidence needed by phase two.
pub(crate) struct InitialSheetGeometry {
    pub(crate) geometry: Option<SheetGeometry>,
    pub(crate) probe_cache: SheetProbeCache,
}

/// Probe one Sheet stream, decode its registered record families, and retain
/// only the chunk evidence needed after cross-reference construction.
pub(crate) fn build_initial_sheet_geometry(
    sheet_name: &str,
    sheet_path: &str,
    data: &[u8],
) -> InitialSheetGeometry {
    let report = probe_sheet_stream(sheet_name, sheet_path, data, &SheetProbeOptions::default());
    geometry_and_cache_from_probe(report, data)
}

/// Complete the post-cross-reference phase using chunks retained from the
/// initial probe instead of probing the Sheet stream a second time.
pub(crate) fn object_geometry_hints_from_cache(
    data: &[u8],
    field_xs: &[u32],
    identity_index: &SheetIdentityIndex,
    object_field_xs: &HashSet<u32>,
    cache: SheetProbeCache,
) -> Vec<SheetObjectGeometryHint> {
    let windows = field_x_windows(data, field_xs, 96);
    let features = field_x_window_features(data, &windows, &cache.chunks);
    let identities = field_x_window_identities(data, &windows, identity_index);
    let scores =
        score_field_x_window_features_with_identities(&features, object_field_xs, &identities);
    populate_object_geometry_hints(&scores, 70)
}

fn geometry_and_cache_from_probe(
    report: SheetProbeReport,
    raw_data: &[u8],
) -> InitialSheetGeometry {
    let SheetProbeReport {
        chunks,
        text_runs,
        coordinate_hints,
        ..
    } = report;
    let texts = text_runs
        .into_iter()
        .map(|run| SheetText {
            offset: run.offset,
            encoding: match run.encoding {
                SheetTextEncoding::Ascii => "ascii",
                SheetTextEncoding::Utf16Le => "utf16_le",
            }
            .to_string(),
            text: run.text,
            byte_len: run.byte_len,
        })
        .collect();
    let coordinate_hints = coordinate_hints
        .into_iter()
        .map(|hint| SheetCoordinateHintDto {
            offset: hint.offset,
            x: hint.x,
            y: hint.y,
        })
        .collect();

    let mut geometry = SheetGeometry {
        texts,
        endpoints: Vec::new(),
        coordinate_hints,
        object_geometry_hints: Vec::new(),
        ..SheetGeometry::default()
    };
    decode_all_families_into(raw_data, &mut geometry);

    let geometry = if geometry.texts.is_empty()
        && geometry.coordinate_hints.is_empty()
        && sheet_geometry_has_no_family_records(&geometry)
    {
        None
    } else {
        let spatial_pairs = collect_normalized_f64_pairs(raw_data);
        geometry.spatial_analysis = if spatial_pairs.is_empty() {
            None
        } else {
            Some(DecodedSpatialAnalysis::from(
                coordinate_pair_spatial_analysis(&spatial_pairs, SPATIAL_ANALYSIS_DEFAULT_GRID_N),
            ))
        };
        Some(geometry)
    };

    InitialSheetGeometry {
        geometry,
        probe_cache: SheetProbeCache { chunks },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::sheet_probe::{
        probe_sheet_stream, SheetCoordinateHint, SheetProbeOptions, SheetTextRun,
    };
    use std::collections::BTreeMap;

    #[test]
    fn initial_build_retains_the_probe_chunks_for_the_post_crossref_phase() {
        let mut data = vec![0x11; 64];
        data.extend_from_slice(b"PUMP-101");
        data.extend_from_slice(&[0; 32]);

        let expected =
            probe_sheet_stream("Sheet6", "/Sheet6", &data, &SheetProbeOptions::default());
        let build = build_initial_sheet_geometry("Sheet6", "/Sheet6", &data);

        assert_eq!(build.probe_cache.chunks.len(), expected.chunks.len());
        for (cached, original) in build.probe_cache.chunks.iter().zip(&expected.chunks) {
            assert_eq!(
                (cached.start, cached.end, &cached.kind_hint),
                (original.start, original.end, &original.kind_hint)
            );
        }
    }

    #[test]
    fn initial_build_normalizes_text_and_coordinate_probe_evidence() {
        let report = SheetProbeReport {
            sheet_name: "Sheet6".into(),
            path: "/Sheet6".into(),
            size: 64,
            candidate_boundaries: Vec::new(),
            chunks: Vec::new(),
            record_type_counts: BTreeMap::new(),
            text_runs: vec![SheetTextRun {
                offset: 8,
                encoding: SheetTextEncoding::Utf16Le,
                text: "PUMP-101".into(),
                byte_len: 16,
            }],
            coordinate_hints: vec![SheetCoordinateHint {
                offset: 32,
                x: 1200,
                y: -450,
            }],
        };

        let build = geometry_and_cache_from_probe(report, &[]);
        let geometry = build.geometry.expect("geometry evidence");

        assert_eq!(geometry.texts.len(), 1);
        assert_eq!(geometry.texts[0].encoding, "utf16_le");
        assert_eq!(geometry.texts[0].text, "PUMP-101");
        assert_eq!(geometry.coordinate_hints.len(), 1);
        assert_eq!(geometry.coordinate_hints[0].x, 1200);
        assert_eq!(geometry.coordinate_hints[0].y, -450);
        assert!(geometry.endpoints.is_empty());
    }

    #[test]
    fn cached_hint_scoring_matches_an_explicit_fresh_probe() {
        let field_x = 0x1020_3040_u32;
        let mut data = vec![0u8; 192];
        data[88..92].copy_from_slice(&1200i32.to_le_bytes());
        data[92..96].copy_from_slice(&(-450i32).to_le_bytes());
        data[100..104].copy_from_slice(&field_x.to_le_bytes());

        let fresh = probe_sheet_stream("Sheet6", "/Sheet6", &data, &SheetProbeOptions::default());
        let build = build_initial_sheet_geometry("Sheet6", "/Sheet6", &data);
        let field_xs = [field_x];
        let object_field_xs = HashSet::from([field_x]);
        let identity_index = SheetIdentityIndex::default();

        let expected = object_geometry_hints_from_cache(
            &data,
            &field_xs,
            &identity_index,
            &object_field_xs,
            SheetProbeCache {
                chunks: fresh.chunks,
            },
        );
        let actual = object_geometry_hints_from_cache(
            &data,
            &field_xs,
            &identity_index,
            &object_field_xs,
            build.probe_cache,
        );

        assert_eq!(actual, expected);
    }
}
