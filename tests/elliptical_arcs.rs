//! A01's vessel `V 010121A` draws its two 2:1 heads.
//!
//! Its cached body `/JSite121` sheet 481 (placed by oid 184) holds two
//! `0x007E igEllipticalArc2d` records, oids 489 and 490; the unplaced body
//! `/JSite39` sheet 96 holds two more, oids 114 and 115. They decode into
//! `JSiteNestedGeometry::elliptical_arcs` and reach the body as exact
//! rational B-splines (OCS plan
//! `2026-09-30-a-cached-body-draws-its-elliptical-arcs`).
//!
//! Soft-skips when the fixture is absent, like `parse_real_files.rs`.

use pid_parse::bspline::SEGMENTS_PER_SPAN;
use pid_parse::symbol_library::SymbolPrimitive;
use pid_parse::{build_normalized_geometry, PidDocument, PidParser};
use std::f64::consts::{PI, TAU};

const A01: &str = "test-file/export-test/publish-data/A01/A01.pid";

fn parse_a01() -> Option<PidDocument> {
    if !std::path::Path::new(A01).exists() {
        eprintln!("skipping: fixture {A01} not found");
        return None;
    }
    Some(
        PidParser::new()
            .parse_file(A01)
            .unwrap_or_else(|e| panic!("Failed to parse {A01}: {e}")),
    )
}

/// One stored head, as measured on the file.
struct Head {
    oid: u32,
    sweep_start: f64,
    sweep_end: f64,
    center: (f64, f64),
    major: (f64, f64),
}

/// One cached body with its two heads and the rectangle they close.
struct Body {
    site: u32,
    path: &'static str,
    sheet: u32,
    layer: u32,
    heads: [Head; 2],
    /// The rectangle's left and right edges, and its bottom and top.
    left_edge: f64,
    right_edge: f64,
    y_range: (f64, f64),
    /// Where the left head's leftmost and the right head's rightmost
    /// points lie: `C ∓ minor`.
    left_apex: f64,
    right_apex: f64,
}

fn bodies() -> [Body; 2] {
    [
        Body {
            site: 121,
            path: "/JSite121",
            sheet: 481,
            layer: 533,
            heads: [
                Head {
                    oid: 489,
                    sweep_start: TAU,
                    sweep_end: PI,
                    center: (-0.010_296_148_41, 0.0889),
                    major: (4.768_329_956e-18, -0.025_958_436_58),
                },
                Head {
                    oid: 490,
                    sweep_start: PI,
                    sweep_end: 0.0,
                    center: (0.123_204_406_4, 0.0889),
                    major: (7.947_216_593e-18, -0.025_958_436_58),
                },
            ],
            left_edge: -0.010_296,
            right_edge: 0.123_204,
            y_range: (0.062_941_5, 0.114_858_5),
            left_apex: -0.023_275_366_7,
            right_apex: 0.136_183_624_7,
        },
        Body {
            site: 39,
            path: "/JSite39",
            sheet: 96,
            layer: 149,
            heads: [
                Head {
                    oid: 114,
                    sweep_start: PI,
                    sweep_end: 0.0,
                    center: (0.167_64, 0.0889),
                    major: (6.221_000_277e-18, -0.020_32),
                },
                Head {
                    oid: 115,
                    sweep_start: TAU,
                    sweep_end: PI,
                    center: (-0.040_64, 0.0889),
                    major: (3.732_600_166e-18, -0.020_32),
                },
            ],
            left_edge: -0.040_64,
            right_edge: 0.167_64,
            y_range: (0.0889 - 0.020_32 - 1e-7, 0.0889 + 0.020_32 + 1e-7),
            left_apex: -0.0508,
            right_apex: 0.1778,
        },
    ]
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9
}

#[test]
fn a01s_cached_bodies_read_their_four_elliptical_arcs() {
    let Some(doc) = parse_a01() else {
        return;
    };
    for body in bodies() {
        let nested = doc
            .jsites
            .iter()
            .find(|site| site.path == body.path)
            .and_then(|site| site.nested_geometry.as_ref())
            .unwrap_or_else(|| panic!("{} is a definition cache", body.path));
        assert_eq!(nested.elliptical_arcs.len(), 2, "{}", body.path);
        for head in &body.heads {
            let arc = nested
                .elliptical_arcs
                .iter()
                .find(|arc| arc.oid == head.oid)
                .unwrap_or_else(|| panic!("{} holds oid {}", body.path, head.oid));
            let what = format!("{} oid {}", body.path, head.oid);
            assert_eq!(arc.sheet_layer_ref, body.layer, "{what}");
            assert_eq!(arc.index, 7, "{what}");
            assert!(near(arc.sweep_start, head.sweep_start), "{what}");
            assert!(near(arc.sweep_end, head.sweep_end), "{what}");
            // A01 stores π one ulp short of `PI` (2π is `TAU` exactly), which
            // is why `bspline::elliptical_arc`'s segment count allows for a
            // rounding error.
            for angle in [arc.sweep_start, arc.sweep_end] {
                if near(angle, PI) {
                    assert_eq!(angle.to_bits(), 0x4009_21FB_5444_2D17, "{what}: stored π");
                }
            }
            assert!(near(arc.center_x, head.center.0), "{what}");
            assert!(near(arc.center_y, head.center.1), "{what}");
            assert!(near(arc.major_x, head.major.0), "{what}");
            assert!(near(arc.major_y, head.major.1), "{what}");
            assert!(near(arc.ratio, 0.5), "{what}");
        }
    }
}

/// Feature: pid-a01-elliptical-heads, Property: A01's heads bulge outward.
///
/// Each body gains two B-spline primitives, sampled onto the far side of
/// the rectangle's left and right edges, reaching `C ∓ minor` and staying
/// between its bottom and top.
#[test]
fn a01s_heads_bulge_outward_from_their_bodies() {
    let Some(doc) = parse_a01() else {
        return;
    };
    let geometry = build_normalized_geometry(&doc);
    for body in bodies() {
        let definition = geometry
            .symbol_definitions
            .iter()
            .find(|d| d.reference.site == body.site && d.reference.sheet == body.sheet)
            .unwrap_or_else(|| panic!("site {} sheet {} is a body", body.site, body.sheet));
        let nested = doc
            .jsites
            .iter()
            .find(|site| site.path == body.path)
            .and_then(|site| site.nested_geometry.as_ref())
            .expect("the body's cache");
        let own_bsplines = nested
            .bsplines
            .iter()
            .filter(|b| definition.layers.binary_search(&b.sheet_layer_ref).is_ok())
            .count();
        let curves: Vec<(usize, &SymbolPrimitive)> = definition
            .primitives
            .iter()
            .enumerate()
            .filter(|(_, p)| matches!(p, SymbolPrimitive::BSpline { .. }))
            .collect();
        assert_eq!(
            curves.len(),
            own_bsplines + 2,
            "site {} sheet {}: two more B-splines than the body's own",
            body.site,
            body.sheet
        );
        // Appended last, on the heads' layer.
        let count = definition.primitives.len();
        for (index, _) in &curves[own_bsplines..] {
            assert!(*index >= count - 2, "the heads close the primitive list");
            assert_eq!(definition.primitive_layers[*index], body.layer);
        }
        let mut sampled: Vec<Vec<(f64, f64)>> = curves[own_bsplines..]
            .iter()
            .map(|(_, p)| p.bspline_points(SEGMENTS_PER_SPAN))
            .collect();
        let min_x =
            |points: &[(f64, f64)]| points.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
        let max_x =
            |points: &[(f64, f64)]| points.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
        sampled.sort_by(|a, b| min_x(a).total_cmp(&min_x(b)));
        let (left, right) = (&sampled[0], &sampled[1]);
        let what = format!("site {} sheet {}", body.site, body.sheet);
        // Both heads alike: four 45° segments each, so 9 poles, 9 weights
        // and 12 knots, sampled as 4 × SEGMENTS_PER_SPAN + 1 = 33 points.
        for (_, curve) in &curves[own_bsplines..] {
            let SymbolPrimitive::BSpline {
                poles,
                weights,
                knots,
            } = curve
            else {
                unreachable!("the filter keeps B-splines only");
            };
            assert_eq!(
                (poles.len(), weights.len(), knots.len()),
                (9, 9, 12),
                "{what}: a head's poles, weights and knots"
            );
        }
        for points in [left, right] {
            assert_eq!(
                points.len(),
                4 * SEGMENTS_PER_SPAN + 1,
                "{what}: a head's sampled points"
            );
        }
        assert!(
            (min_x(left) - body.left_apex).abs() <= 1e-6,
            "{what}: left apex {}",
            min_x(left)
        );
        assert!(
            left.iter().all(|p| p.0 <= body.left_edge + 1e-9),
            "{what}: the left head stays left of the body"
        );
        assert!(
            (max_x(right) - body.right_apex).abs() <= 1e-6,
            "{what}: right apex {}",
            max_x(right)
        );
        assert!(
            right.iter().all(|p| p.0 >= body.right_edge - 1e-9),
            "{what}: the right head stays right of the body"
        );
        for p in left.iter().chain(right.iter()) {
            assert!(
                (body.y_range.0..=body.y_range.1).contains(&p.1),
                "{what}: {p:?} left the rectangle's height"
            );
        }
    }
}
