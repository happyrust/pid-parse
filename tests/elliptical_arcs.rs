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

const XA_TEMPLATE: &str = "test-file/Ref/Template Files/XA-A1.igr";

/// The 熊安 border template draws two elliptical arcs on its own sheet, and
/// they close the two counters of a letter.
///
/// `XA-A1.igr`'s `/Sheet6` holds two `0x007E` records on its own chain:
/// oids 1012 and 1013, on layer 14 with style 11, among the lines and the
/// B-spline of one outlined `B` two millimetres high in the title block
/// (oids 1004–1016). The family was read in cached symbol bodies and `.sym`
/// files only, so the letter printed with both counters open on the right.
/// They are the only sheet-level arcs that run counter-clockwise (direction
/// byte 1); the sixteen of the `-W` templates run the other way
/// (`the_w_templates_logo_ring_is_four_clockwise_arcs_of_one_ellipse`).
///
/// Each arc reaches the page as the polyline `bspline::sample` makes of the
/// rational B-spline `bspline::elliptical_arc_directed` builds from the
/// record's own fields — the function, and the reading of the direction
/// byte, that a cached body's dome is drawn through and the reference plots
/// hold to the vendor's ink (`sym_elliptical_arc_matches_cached_body.rs`).
/// What says the reading is right on this sheet, which has no vendor plot:
/// each arc starts on the right end of its counter's lower edge, ends on the
/// right end of its upper edge, and bulges away from the counter. Turned the
/// other way it would run through the inside of the box it closes.
#[test]
fn the_xa_templates_sheet_level_arcs_close_the_counters_of_its_b() {
    use pid_parse::bspline::{elliptical_arc_directed, sample};
    use pid_parse::parsers::sheet_records::{decode_igellipticalarcs, decode_iglines};
    use pid_parse::PidGraphicKind;

    // The template is committed: a checkout without it fails here.
    assert!(
        std::path::Path::new(XA_TEMPLATE).exists(),
        "{XA_TEMPLATE} is committed and has to be present"
    );
    let package = PidParser::new()
        .parse_package(XA_TEMPLATE)
        .unwrap_or_else(|e| panic!("Failed to parse {XA_TEMPLATE}: {e}"));
    let data = package
        .streams
        .get("/Sheet6")
        .unwrap_or_else(|| panic!("{XA_TEMPLATE} has a /Sheet6"))
        .data
        .as_slice();
    let arcs = decode_igellipticalarcs(data);
    let lines = decode_iglines(data);
    assert_eq!(
        arcs.iter().map(|arc| arc.oid).collect::<Vec<_>>(),
        [1012, 1013],
        "the sheet's elliptical arcs"
    );
    let normalized = build_normalized_geometry(&package.parsed);

    // (arc, the counter's lower edge, its upper edge, centre in mm, semi-axes in mm)
    let counters = [
        (
            1012u32,
            1008u32,
            1007u32,
            (682.934_1, 56.147_1),
            (0.227_9, 0.208_2),
        ),
        (1013, 1010, 1009, (682.948_5, 55.320_9), (0.269_4, 0.261_7)),
    ];
    for (oid, lower, upper, centre_mm, (major_mm, minor_mm)) in counters {
        let arc = arcs
            .iter()
            .find(|arc| arc.oid == oid)
            .expect("listed above");
        assert_eq!(
            (arc.sheet_layer_ref, arc.index, arc.flag),
            (14, 11, 1),
            "oid {oid}: layer, style, direction byte"
        );
        assert!(
            (arc.sweep_start - PI).abs() < 1e-9 && (arc.sweep_end - TAU).abs() < 1e-9,
            "oid {oid} sweeps {} -> {}",
            arc.sweep_start,
            arc.sweep_end
        );
        assert!(
            (arc.center.0 * 1e3 - centre_mm.0).abs() < 1e-4
                && (arc.center.1 * 1e3 - centre_mm.1).abs() < 1e-4,
            "oid {oid} centre {:?}",
            arc.center
        );
        let major = arc.major_axis.0.hypot(arc.major_axis.1);
        assert!(
            (major * 1e3 - major_mm).abs() < 1e-4
                && (major * arc.ratio * 1e3 - minor_mm).abs() < 1e-4,
            "oid {oid} semi-axes {} / {} mm",
            major * 1e3,
            major * arc.ratio * 1e3
        );

        // What the page is given: the record's own B-spline, sampled.
        let (poles, weights, knots) = elliptical_arc_directed(
            arc.center,
            arc.major_axis,
            arc.ratio,
            arc.sweep_start,
            arc.sweep_end,
            arc.flag == 0,
        )
        .expect("a half ellipse");
        let expected = sample(&poles, &weights, &knots, SEGMENTS_PER_SPAN);
        let entity = normalized
            .entities
            .iter()
            .find(|entity| {
                entity.graphic_oid == Some(oid)
                    && entity.source.stream_path.as_deref() == Some("/Sheet6")
            })
            .unwrap_or_else(|| panic!("oid {oid} reaches no entity of the template's sheet"));
        let PidGraphicKind::Polyline { points, closed } = &entity.kind else {
            panic!("oid {oid} is drawn as {:?}", entity.kind);
        };
        assert!(!closed, "oid {oid} is an arc, not a ring");
        assert_eq!(
            points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>(),
            expected,
            "oid {oid} is the sampled B-spline of its own fields"
        );
        assert_eq!(
            entity
                .source
                .byte_range
                .as_ref()
                .map(|range| (range.start, range.end)),
            Some((arc.byte_range.start, arc.byte_range.end)),
            "oid {oid} names the record it was drawn from"
        );

        // Where it lies among the letter's own lines.
        let right_end = |line_oid: u32| {
            let line = lines
                .iter()
                .find(|line| line.oid == line_oid)
                .unwrap_or_else(|| panic!("line oid {line_oid} is on the sheet"));
            if line.start.0 > line.end.0 {
                line.start
            } else {
                line.end
            }
        };
        let near = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).hypot(a.1 - b.1) < 1e-6;
        let (first, last) = (expected[0], expected[expected.len() - 1]);
        assert!(
            near(first, right_end(lower)),
            "oid {oid} starts at {first:?}, the lower edge ends at {:?}",
            right_end(lower)
        );
        assert!(
            near(last, right_end(upper)),
            "oid {oid} ends at {last:?}, the upper edge ends at {:?}",
            right_end(upper)
        );
        let reach = expected
            .iter()
            .map(|point| point.0 - arc.center.0)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            (reach * 1e3 - minor_mm).abs() < 1e-3
                && expected.iter().all(|point| point.0 >= arc.center.0 - 1e-9),
            "oid {oid} bulges {} mm to the right of its centre and nowhere to the left",
            reach * 1e3
        );
    }
}

/// Every stroke of the template's outlined wordmark draws with the pen its
/// style names: the lines, the two elliptical arcs of the `B`'s counters, and
/// the B-splines of every letter's bowls.
///
/// `XA-A1.igr`'s `/Sheet6` keeps 23 `igBspCurve2d` records on its own chain
/// — oid 1014, the two outer bowls of the `B`, is one — and across the 71
/// pages the export is recorded on it is the only sheet that keeps any (the
/// `-W` templates keep four each, and no page wears one). The
/// family's records carry the same style reference at payload `+14` the
/// lines and arcs do, and a symbol body's B-spline is drawn with it; the
/// line-style index did not list the family, so the bowls printed with the
/// default pen, twice the width of the strokes they join.
#[test]
fn the_xa_templates_wordmark_draws_every_stroke_with_the_pen_it_names() {
    use pid_parse::parsers::sheet_records::decode_igbspcurves;

    // The template is committed: a checkout without it fails here.
    assert!(
        std::path::Path::new(XA_TEMPLATE).exists(),
        "{XA_TEMPLATE} is committed and has to be present"
    );
    let package = PidParser::new()
        .parse_package(XA_TEMPLATE)
        .unwrap_or_else(|e| panic!("Failed to parse {XA_TEMPLATE}: {e}"));
    let styles = pid_parse::style_link::line_styles_for_document(&package.parsed);
    let pen = |oid: u32| {
        styles
            .get(&("/Sheet6".to_string(), oid))
            .map(|style| (style.symbology.width_mm(), style.symbology.rgb()))
    };
    // A line of the `B`: the lower edge of its lower counter.
    let line = pen(1010).expect("the letter's lines resolve their style");
    assert!(
        (line.0 - 0.25).abs() < 1e-9,
        "style 11 states a 0.25 mm pen, read {} mm",
        line.0
    );
    for (oid, what) in [
        (1012, "the upper counter's arc"),
        (1013, "the lower counter's arc"),
    ] {
        assert_eq!(
            pen(oid),
            Some(line),
            "oid {oid}, {what}, draws with the pen of the letter's lines"
        );
    }

    let data = package
        .streams
        .get("/Sheet6")
        .unwrap_or_else(|| panic!("{XA_TEMPLATE} has a /Sheet6"))
        .data
        .as_slice();
    let curves = decode_igbspcurves(data);
    assert_eq!(curves.len(), 23, "the sheet's own B-splines");
    assert!(
        curves.iter().any(|curve| curve.oid == 1014),
        "the B's outer bowls are one of them"
    );
    for curve in &curves {
        assert_eq!(curve.index, 11, "oid {} names style 11", curve.oid);
        assert_eq!(
            pen(curve.oid),
            Some(line),
            "oid {}, a B-spline, draws with the pen of the letter's lines",
            curve.oid
        );
    }
}

/// The archive the `-W` border templates are committed in.
#[cfg(feature = "refdata-archive")]
const W_TEMPLATES: &str = "test-file/backup-test/DWG-0202GP06-01_p/RefData~4~682.zip";

/// One `-W` template of the committed archive, parsed. The archive is
/// committed, so a checkout without it or without the member fails here.
#[cfg(feature = "refdata-archive")]
fn w_template(name: &str) -> pid_parse::PidPackage {
    let archive = std::path::Path::new(W_TEMPLATES);
    let members = pid_parse::refdata_archive::archive_members(archive, "igr")
        .unwrap_or_else(|error| panic!("{W_TEMPLATES} is committed and reads: {error:?}"));
    let member = members
        .iter()
        .find(|member| member.label().file_name().is_some_and(|file| file == name))
        .unwrap_or_else(|| panic!("{W_TEMPLATES} holds {name}"));
    let bytes = pid_parse::refdata_archive::read_archive_file(member)
        .unwrap_or_else(|error| panic!("{name} reads out of {W_TEMPLATES}: {error:?}"));
    PidParser::new()
        .parse_package_from_reader(std::io::Cursor::new(bytes))
        .unwrap_or_else(|error| panic!("{name} parses: {error}"))
}

/// The `-W` border templates draw the ring of their logo as four elliptical
/// arcs on the sheet's own chain, and these run clockwise.
///
/// `A0-W.igr`, `A1-W.igr`, `A2-W.igr` and `A3-W.igr` each keep four `0x007E`
/// records among the 37 members of the logo's Group: pieces of one ellipse
/// 10.976 mm high and 7.958 mm wide — its top cap, its bottom cap, and two
/// short pieces of its left side — with the direction byte 0, where
/// `XA-A1.igr`'s two carry 1. They are the only sheet-level sample of that
/// value, and they have been drawn since sheet-level arcs were
/// (`db94e60`) with nothing holding how.
///
/// What says the reading is right, on a logo no vendor plot shows: each cap
/// starts and ends on the two ends of the chord line that closes it and
/// stays on the far side of that chord from the centre, 106.26° of the
/// ellipse; each side piece runs 14.07° between the ends of two of the
/// logo's lines and stays on the left. Read the other way a cap would run
/// 253.74° round the far side and through the other cap, and a side piece
/// all but the whole way round.
#[cfg(feature = "refdata-archive")]
#[test]
fn the_w_templates_logo_ring_is_four_clockwise_arcs_of_one_ellipse() {
    use pid_parse::bspline::{elliptical_arc_directed, sample};
    use pid_parse::parsers::sheet_records::{
        decode_igbspcurves, decode_igellipticalarcs, decode_iggroups, decode_iglines,
    };
    use pid_parse::PidGraphicKind;

    // Every one of the four keeps the same eight records in its logo.
    for name in ["A0-W.igr", "A1-W.igr", "A2-W.igr", "A3-W.igr"] {
        let package = w_template(name);
        let data = package.streams["/Sheet6"].data.as_slice();
        let logo = decode_iggroups(data)
            .into_iter()
            .find(|group| group.members.len() == 37)
            .unwrap_or_else(|| panic!("{name}: the logo's group of 37"));
        let arcs = decode_igellipticalarcs(data);
        let curves = decode_igbspcurves(data);
        assert_eq!((arcs.len(), curves.len()), (4, 4), "{name}");
        let normalized = build_normalized_geometry(&package.parsed);
        let drawn = |oid: u32| {
            normalized.entities.iter().any(|entity| {
                entity.graphic_oid == Some(oid)
                    && entity.source.stream_path.as_deref() == Some("/Sheet6")
                    && matches!(entity.kind, PidGraphicKind::Polyline { closed: false, .. })
            })
        };
        for arc in &arcs {
            assert_eq!(
                (arc.parent_ref, arc.flag),
                (logo.oid, 0),
                "{name} oid {}: a member of the logo, clockwise",
                arc.oid
            );
            let major = arc.major_axis.0.hypot(arc.major_axis.1);
            assert!(
                (major * 1e3 - 5.488).abs() < 1e-3 && (arc.ratio - 0.725).abs() < 1e-9,
                "{name} oid {}: semi-axes {} mm x {}",
                arc.oid,
                major * 1e3,
                arc.ratio
            );
            assert!(drawn(arc.oid), "{name} oid {} reaches the sheet", arc.oid);
        }
        for curve in &curves {
            assert_eq!(curve.parent_ref, logo.oid, "{name} oid {}", curve.oid);
            assert!(
                drawn(curve.oid),
                "{name} oid {} reaches the sheet",
                curve.oid
            );
        }
    }

    // `A1-W.igr`, arc by arc.
    let package = w_template("A1-W.igr");
    let data = package.streams["/Sheet6"].data.as_slice();
    let arcs = decode_igellipticalarcs(data);
    let lines = decode_iglines(data);
    assert_eq!(
        arcs.iter().map(|arc| arc.oid).collect::<Vec<_>>(),
        [194, 197, 333, 354],
        "the sheet's elliptical arcs"
    );
    let normalized = build_normalized_geometry(&package.parsed);
    let centre_mm = (706.431_8, 94.070_3);
    let (semi_major_mm, semi_minor_mm) = (5.488_0, 3.978_8);
    let end_of = |line_oid: u32, start: bool| {
        let line = lines
            .iter()
            .find(|line| line.oid == line_oid)
            .unwrap_or_else(|| panic!("line oid {line_oid} is on the sheet"));
        if start {
            line.start
        } else {
            line.end
        }
    };
    // (oid, sweep as stored, the line end it starts on, the line end it ends on)
    let pieces = [
        (
            197u32,
            (4.068_888, 2.214_297),
            (336u32, true),
            (336u32, false),
        ),
        (354, (0.927_295, 5.355_890), (335, false), (335, true)),
        (194, (4.587_061, 4.341_440), (318, true), (196, true)),
        (333, (5.083_338, 4.837_717), (317, true), (188, true)),
    ];
    for (oid, (sweep_start, sweep_end), from, to) in pieces {
        let arc = arcs
            .iter()
            .find(|arc| arc.oid == oid)
            .expect("listed above");
        assert_eq!(
            (arc.sheet_layer_ref, arc.index, arc.flag),
            (8, 14, 0),
            "oid {oid}: layer, style, direction byte"
        );
        assert!(
            (arc.sweep_start - sweep_start).abs() < 1e-5
                && (arc.sweep_end - sweep_end).abs() < 1e-5,
            "oid {oid} sweeps {} -> {}",
            arc.sweep_start,
            arc.sweep_end
        );
        assert!(
            (arc.center.0 * 1e3 - centre_mm.0).abs() < 1e-4
                && (arc.center.1 * 1e3 - centre_mm.1).abs() < 1e-4
                && arc.major_axis.0.abs() < 1e-12
                && (arc.major_axis.1 * 1e3 + semi_major_mm).abs() < 1e-4,
            "oid {oid}: centre {:?}, major axis {:?}",
            arc.center,
            arc.major_axis
        );

        // What the page is given: the record's own B-spline, sampled.
        let (poles, weights, knots) = elliptical_arc_directed(
            arc.center,
            arc.major_axis,
            arc.ratio,
            arc.sweep_start,
            arc.sweep_end,
            arc.flag == 0,
        )
        .expect("an arc of an ellipse");
        let expected = sample(&poles, &weights, &knots, SEGMENTS_PER_SPAN);
        let entity = normalized
            .entities
            .iter()
            .find(|entity| {
                entity.graphic_oid == Some(oid)
                    && entity.source.stream_path.as_deref() == Some("/Sheet6")
            })
            .unwrap_or_else(|| panic!("oid {oid} reaches no entity of the template's sheet"));
        let PidGraphicKind::Polyline { points, closed } = &entity.kind else {
            panic!("oid {oid} is drawn as {:?}", entity.kind);
        };
        assert!(!closed, "oid {oid} is an arc, not a ring");
        assert_eq!(
            points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>(),
            expected,
            "oid {oid} is the sampled B-spline of its own fields"
        );
        assert_eq!(
            entity
                .source
                .byte_range
                .as_ref()
                .map(|range| (range.start, range.end)),
            Some((arc.byte_range.start, arc.byte_range.end)),
            "oid {oid} names the record it was drawn from"
        );

        // Where it lies among the logo's own lines.
        let near = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).hypot(a.1 - b.1) < 1e-6;
        let (first, last) = (expected[0], expected[expected.len() - 1]);
        assert!(
            near(first, end_of(from.0, from.1)) && near(last, end_of(to.0, to.1)),
            "oid {oid} runs {first:?} -> {last:?}; line {} gives {:?}, line {} gives {:?}",
            from.0,
            end_of(from.0, from.1),
            to.0,
            end_of(to.0, to.1)
        );
        // The angle it turns through, seen from the centre on the ellipse's
        // own circle: the short way round, clockwise.
        let on_circle = |point: (f64, f64)| {
            ((point.1 - arc.center.1) * 1e3 / semi_major_mm)
                .atan2((point.0 - arc.center.0) * 1e3 / semi_minor_mm)
        };
        let turned: f64 = expected
            .windows(2)
            .map(|pair| {
                let step = on_circle(pair[1]) - on_circle(pair[0]);
                (step + PI).rem_euclid(TAU) - PI
            })
            .sum();
        let (ys, xs): (Vec<f64>, Vec<f64>) =
            expected.iter().map(|p| (p.1 * 1e3, p.0 * 1e3)).unzip();
        let low = |values: &[f64]| values.iter().copied().fold(f64::INFINITY, f64::min);
        let high = |values: &[f64]| values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        match oid {
            // The top cap: above its chord, up to the ellipse's top.
            197 => {
                assert!(
                    (turned.to_degrees() + 106.26).abs() < 0.01,
                    "oid {oid} turns {} deg",
                    turned.to_degrees()
                );
                assert!(
                    (low(&ys) - 97.363_1).abs() < 1e-3
                        && (high(&ys) - (centre_mm.1 + semi_major_mm)).abs() < 1e-3,
                    "oid {oid} spans y {} .. {} mm",
                    low(&ys),
                    high(&ys)
                );
            }
            // The bottom cap: below its chord, down to the ellipse's bottom.
            354 => {
                assert!(
                    (turned.to_degrees() + 106.26).abs() < 0.01,
                    "oid {oid} turns {} deg",
                    turned.to_degrees()
                );
                assert!(
                    (high(&ys) - 90.777_5).abs() < 1e-3
                        && (low(&ys) - (centre_mm.1 - semi_major_mm)).abs() < 1e-3,
                    "oid {oid} spans y {} .. {} mm",
                    low(&ys),
                    high(&ys)
                );
            }
            // The two pieces of the left side.
            _ => {
                assert!(
                    (turned.to_degrees() + 14.07).abs() < 0.01,
                    "oid {oid} turns {} deg",
                    turned.to_degrees()
                );
                assert!(
                    low(&xs) > centre_mm.0 - semi_minor_mm - 1e-3 && high(&xs) < 702.73,
                    "oid {oid} spans x {} .. {} mm, the ellipse's left side",
                    low(&xs),
                    high(&xs)
                );
            }
        }
    }

    // The logo's four B-splines draw with the pen of its lines.
    let styles = pid_parse::style_link::line_styles_for_document(&package.parsed);
    let pen = |oid: u32| {
        styles
            .get(&("/Sheet6".to_string(), oid))
            .map(|style| style.symbology.width_mm())
    };
    let line = pen(336).expect("the logo's lines resolve their style");
    assert!(
        (line - 0.13).abs() < 1e-9,
        "style 14 states a 0.13 mm pen, read {line} mm"
    );
    let curves = decode_igbspcurves(data);
    assert_eq!(
        curves.iter().map(|curve| curve.oid).collect::<Vec<_>>(),
        [332, 337, 338, 353],
        "the sheet's own B-splines"
    );
    for curve in &curves {
        assert_eq!(
            (curve.sheet_layer_ref, curve.index),
            (8, 14),
            "oid {}",
            curve.oid
        );
        assert_eq!(
            pen(curve.oid),
            Some(line),
            "oid {} draws with the lines' pen",
            curve.oid
        );
    }
    for arc in &arcs {
        assert_eq!(
            pen(arc.oid),
            Some(line),
            "oid {} draws with the lines' pen",
            arc.oid
        );
    }
}
