//! A boundary re-lists curves of every kind, and each member is the curve
//! its reference names.
//!
//! `A0-W.igr`, `A1-W.igr`, `A2-W.igr` and `A3-W.igr` — border templates a
//! backup package keeps in its `RefData~4~682.zip` — draw one logo, and five
//! of its 37 records are `igBoundary2d` fills: two stripes, the two caps of
//! an ellipse, and a notched crescent. Their loops are not made of lines alone. Each
//! member opens with a tag byte and then carries the geometry of one curve
//! exactly as that curve's own record states it after its 18-byte
//! sub-header:
//!
//! | tag | member | the record it re-lists |
//! |---|---|---|
//! | `0x67` | line: start, end | `igLine2d` |
//! | `0x74` | elliptical arc: two parameters, centre, major semi-axis vector, ratio, direction byte | `igEllipticalArc2d` — and `igArc2d`, written as the ellipse whose major semi-axis is `(radius, 0)` and whose ratio is one |
//! | `0x7F` | B-spline: poles, weights, knots, a double, four bytes | `igBspCurve2d` |
//!
//! and the record ends as a drawing's boundary does: the point the fill was
//! seeded at, a flag byte, and one 8-byte reference to each member's own
//! record. The decoder knew `0x74` as a circle's arc, read where the major
//! semi-axis vector's second half and the ratio stand as constants, had no
//! `0x7F`, and looked for a loop of such members only in a record that names
//! none; it refused all twenty, and the four `.pid` templates that embed the
//! same frames refused twenty more.
//!
//! What says the reading is right, on a logo no vendor plot shows, is the
//! records the references name. Every member is held to its own here: a line
//! to the two ends of its `igLine2d`, an arc to the circle and the two ends
//! of its `igArc2d`, an elliptical arc to the ellipse and the two ends of its
//! `igEllipticalArc2d`, a B-spline to the curve of its `igBspCurve2d` at
//! equal parameters — the member is that curve with every interior knot
//! raised to the degree, its Bézier pieces — each run the way bit 2 of the
//! reference's last word says: forward when it is set, against the record's
//! own direction when it is clear. The anchor lies inside every one of the
//! twenty fills.
//!
//! The probe that measured this is
//! `examples/probe_what_curve_each_boundary_member_relists.rs`; see
//! `docs/analysis/2026-11-12-a-boundary-member-is-the-curve-its-reference-names.md`.

#![cfg(feature = "refdata-archive")]

use std::collections::BTreeMap;
use std::f64::consts::TAU;

use pid_parse::bspline::{sample, SEGMENTS_PER_SPAN};
use pid_parse::parsers::sheet_records::{
    decode_igarcs, decode_igboundaries, decode_igbspcurves, decode_igellipticalarcs,
    decode_iggroups, decode_iglines, SheetIgBoundary2dDecoded,
};
use pid_parse::{build_normalized_geometry, PidGraphicKind, PidPackage, PidParser};

/// The archive the `-W` border templates are committed in.
const W_TEMPLATES: &str = "test-file/backup-test/DWG-0202GP06-01_p/RefData~4~682.zip";

/// One document of the committed archive, parsed. The archive is committed,
/// so a checkout without it or without the member fails here.
fn archived(name: &str) -> PidPackage {
    let archive = std::path::Path::new(W_TEMPLATES);
    let extension = name.rsplit('.').next().unwrap_or_default();
    let members = pid_parse::refdata_archive::archive_members(archive, extension)
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

type Point = (f64, f64);

fn distance(a: Point, b: Point) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// Whether `point` is inside the closed ring by the even-odd rule.
fn contains(ring: &[Point], point: Point) -> bool {
    let mut inside = false;
    for (index, &(x0, y0)) in ring.iter().enumerate() {
        let (x1, y1) = ring[(index + 1) % ring.len()];
        if (y0 > point.1) != (y1 > point.1) && point.0 < x0 + (point.1 - y0) * (x1 - x0) / (y1 - y0)
        {
            inside = !inside;
        }
    }
    inside
}

/// The signed area of the closed ring: positive counter-clockwise.
fn signed_area(ring: &[Point]) -> f64 {
    ring.iter()
        .enumerate()
        .map(|(index, &(x0, y0))| {
            let (x1, y1) = ring[(index + 1) % ring.len()];
            x0 * y1 - x1 * y0
        })
        .sum::<f64>()
        / 2.0
}

/// The point of the ellipse at parameter `t`: `C + cos t · major + sin t ·
/// minor`, `minor` the major semi-axis turned a quarter and scaled.
fn ellipse_at(center: Point, major: Point, ratio: f64, t: f64) -> Point {
    let minor = (-major.1 * ratio, major.0 * ratio);
    (
        center.0 + t.cos() * major.0 + t.sin() * minor.0,
        center.1 + t.cos() * major.1 + t.sin() * minor.1,
    )
}

/// The signed parameter an arc turns through from `start` to `end`: down
/// when the direction byte is `0`, up otherwise.
fn sweep(start: f64, end: f64, direction: u8) -> f64 {
    if direction == 0 {
        -(start - end).rem_euclid(TAU)
    } else {
        (end - start).rem_euclid(TAU)
    }
}

/// One member of a decoded boundary: its tag byte and the vertices its
/// chords run through, first to last.
struct Member {
    tag: u8,
    vertices: Vec<Point>,
}

/// The members of a decoded boundary, in order: the chords that share a tag
/// offset are one member's.
fn members_of(data: &[u8], boundary: &SheetIgBoundary2dDecoded) -> Vec<Member> {
    let payload = &data[boundary.byte_range.start + 6..boundary.byte_range.end];
    let mut members: Vec<(usize, Member)> = Vec::new();
    for segment in &boundary.segments {
        match members.last_mut() {
            Some((at, member)) if *at == segment.tag_offset => member.vertices.push(segment.end),
            _ => members.push((
                segment.tag_offset,
                Member {
                    tag: payload[segment.tag_offset],
                    vertices: vec![segment.start, segment.end],
                },
            )),
        }
    }
    members.into_iter().map(|(_, member)| member).collect()
}

/// The five fills of the logo, as measured on `A1-W.igr`: the boundary's
/// oid there, its members' tags in order, and its area in mm².
struct Fill {
    what: &'static str,
    a1_oid: u32,
    tags: &'static [u8],
    area_mm2: f64,
}

const LINE: u8 = 0x67;
const ARC: u8 = 0x74;
const SPLINE: u8 = 0x7F;

/// In the order `A1-W.igr`'s chain holds them.
const FILLS: [Fill; 5] = [
    Fill {
        what: "the upper stripe",
        a1_oid: 191,
        tags: &[LINE, ARC, LINE, ARC],
        area_mm2: 10.344_136,
    },
    Fill {
        what: "the lower stripe",
        a1_oid: 198,
        tags: &[LINE, ARC, LINE, ARC],
        area_mm2: 10.344_149,
    },
    Fill {
        what: "the notched crescent",
        a1_oid: 341,
        tags: &[
            ARC, SPLINE, LINE, ARC, LINE, SPLINE, LINE, ARC, LINE, SPLINE, LINE, ARC, LINE, SPLINE,
        ],
        area_mm2: 40.638_130,
    },
    Fill {
        what: "the ellipse's lower cap",
        a1_oid: 343,
        tags: &[LINE, ARC],
        area_mm2: 9.586_107,
    },
    Fill {
        what: "the ellipse's upper cap",
        a1_oid: 347,
        tags: &[ARC, LINE],
        area_mm2: 9.586_107,
    },
];

/// What the members of one template's five fills were held against.
#[derive(Debug, Default, PartialEq)]
struct Held {
    lines: usize,
    arcs: usize,
    elliptical_arcs: usize,
    splines: usize,
    forward: usize,
    reversed: usize,
}

/// Hold every member of every boundary of `stream` to the record its
/// reference names; returns the tally and each boundary's ring area in mm²
/// keyed by the tags of its members.
fn hold_the_logo(name: &str, data: &[u8]) -> (Held, Vec<(Vec<u8>, f64)>) {
    let lines: BTreeMap<u32, _> = decode_iglines(data)
        .into_iter()
        .map(|line| (line.oid, line))
        .collect();
    let arcs: BTreeMap<u32, _> = decode_igarcs(data)
        .into_iter()
        .map(|arc| (arc.oid, arc))
        .collect();
    let elliptical: BTreeMap<u32, _> = decode_igellipticalarcs(data)
        .into_iter()
        .map(|arc| (arc.oid, arc))
        .collect();
    let splines: BTreeMap<u32, _> = decode_igbspcurves(data)
        .into_iter()
        .map(|curve| (curve.oid, curve))
        .collect();
    let logo = decode_iggroups(data)
        .into_iter()
        .find(|group| group.members.len() == 37)
        .unwrap_or_else(|| panic!("{name}: the logo's group of 37"));

    let boundaries = decode_igboundaries(data);
    assert_eq!(
        boundaries.len(),
        5,
        "{name}: the logo's five fills decode, read {:?}",
        boundaries
            .iter()
            .map(|boundary| boundary.oid)
            .collect::<Vec<_>>()
    );

    let mut held = Held::default();
    let mut areas = Vec::new();
    for boundary in &boundaries {
        let site = format!("{name} oid {}", boundary.oid);
        assert_eq!(
            (boundary.parent_ref, boundary.sub_type_word),
            (logo.oid, 0x0000),
            "{site}: a member of the logo"
        );
        assert!(
            boundary.loop_segment_counts.is_empty(),
            "{site}: one loop, so no loop tally"
        );
        assert_eq!(boundary.trailer_flag, 1, "{site}");
        let members = members_of(data, boundary);
        assert_eq!(
            (members.len(), boundary.member_refs.len()),
            (
                boundary.segment_count as usize,
                boundary.segment_count as usize
            ),
            "{site}: as many members, and as many references, as the record states"
        );

        for (index, (member, reference)) in members.iter().zip(&boundary.member_refs).enumerate() {
            let what = format!("{site} member {index} -> oid {}", reference.member_oid);
            // Bit 2 of the reference's last word: the member runs the way
            // its record does.
            let forward = reference.sub_word & 4 != 0;
            assert_eq!(
                reference.sub_word & 1 != 0,
                index == 0,
                "{what}: bit 0 marks the first reference"
            );
            if forward {
                held.forward += 1;
            } else {
                held.reversed += 1;
            }
            let (first, last) = (
                member.vertices[0],
                member.vertices[member.vertices.len() - 1],
            );
            // The two ends a record's curve has, in the order this member
            // runs them.
            let ends = |start: Point, end: Point| if forward { (start, end) } else { (end, start) };
            match (member.tag, reference.class_word) {
                (LINE, 0x00CB) => {
                    let line = lines
                        .get(&reference.member_oid)
                        .unwrap_or_else(|| panic!("{what}: no live igLine2d"));
                    assert_eq!(
                        member.vertices,
                        {
                            let (from, to) = ends(line.start, line.end);
                            vec![from, to]
                        },
                        "{what}: the line, bit for bit"
                    );
                    held.lines += 1;
                }
                (ARC, 0x00D3) if arcs.contains_key(&reference.member_oid) => {
                    let arc = &arcs[&reference.member_oid];
                    for vertex in &member.vertices {
                        assert!(
                            (distance(*vertex, arc.center) - arc.radius).abs() < 1e-12,
                            "{what}: {vertex:?} is off the circle"
                        );
                    }
                    let on = |angle: f64| {
                        (
                            arc.center.0 + arc.radius * angle.cos(),
                            arc.center.1 + arc.radius * angle.sin(),
                        )
                    };
                    let (from, to) = ends(on(arc.start_angle), on(arc.end_angle));
                    assert!(
                        distance(first, from) < 1e-12 && distance(last, to) < 1e-12,
                        "{what}: runs {first:?} -> {last:?}, the arc {from:?} -> {to:?}"
                    );
                    // The angle it turns through about the centre is the
                    // record's, the other way round when it runs reversed.
                    let angle =
                        |point: Point| (point.1 - arc.center.1).atan2(point.0 - arc.center.0);
                    let turned: f64 = member
                        .vertices
                        .windows(2)
                        .map(|pair| {
                            (angle(pair[1]) - angle(pair[0]) + TAU / 2.0).rem_euclid(TAU)
                                - TAU / 2.0
                        })
                        .sum();
                    let own = sweep(arc.start_angle, arc.end_angle, arc.flag);
                    let expected = if forward { own } else { -own };
                    assert!(
                        (turned - expected).abs() < 1e-9,
                        "{what}: turns {turned} rad, the arc {expected}"
                    );
                    held.arcs += 1;
                }
                (ARC, 0x00D3) => {
                    let arc = elliptical
                        .get(&reference.member_oid)
                        .unwrap_or_else(|| panic!("{what}: no live igArc2d or igEllipticalArc2d"));
                    // Each vertex in the ellipse's own frame, in semi-axis
                    // units: on the ellipse, `u² + v² = 1`.
                    let minor = (-arc.major_axis.1 * arc.ratio, arc.major_axis.0 * arc.ratio);
                    let frame = |point: Point| {
                        let d = (point.0 - arc.center.0, point.1 - arc.center.1);
                        (
                            (d.0 * arc.major_axis.0 + d.1 * arc.major_axis.1)
                                / (arc.major_axis.0.powi(2) + arc.major_axis.1.powi(2)),
                            (d.0 * minor.0 + d.1 * minor.1) / (minor.0.powi(2) + minor.1.powi(2)),
                        )
                    };
                    for vertex in &member.vertices {
                        let (u, v) = frame(*vertex);
                        assert!(
                            (u * u + v * v - 1.0).abs() < 1e-9,
                            "{what}: {vertex:?} is off the ellipse"
                        );
                    }
                    let on = |t: f64| ellipse_at(arc.center, arc.major_axis, arc.ratio, t);
                    let (from, to) = ends(on(arc.sweep_start), on(arc.sweep_end));
                    assert!(
                        distance(first, from) < 1e-12 && distance(last, to) < 1e-12,
                        "{what}: runs {first:?} -> {last:?}, the arc {from:?} -> {to:?}"
                    );
                    let parameter = |point: Point| {
                        let (u, v) = frame(point);
                        v.atan2(u)
                    };
                    let turned: f64 = member
                        .vertices
                        .windows(2)
                        .map(|pair| {
                            (parameter(pair[1]) - parameter(pair[0]) + TAU / 2.0).rem_euclid(TAU)
                                - TAU / 2.0
                        })
                        .sum();
                    let own = sweep(arc.sweep_start, arc.sweep_end, arc.flag);
                    let expected = if forward { own } else { -own };
                    assert!(
                        (turned - expected).abs() < 1e-9,
                        "{what}: turns {turned} rad of parameter, the arc {expected}"
                    );
                    held.elliptical_arcs += 1;
                }
                (SPLINE, 0x00D4) => {
                    let curve = splines
                        .get(&reference.member_oid)
                        .unwrap_or_else(|| panic!("{what}: no live igBspCurve2d"));
                    // The record's own curve, sampled the way a sheet's
                    // B-spline is drawn. The member has the same breaks, so
                    // its vertices are the same parameters.
                    let mut own = sample(
                        &curve.poles,
                        &curve.weights,
                        &curve.knots,
                        SEGMENTS_PER_SPAN,
                    );
                    if !forward {
                        own.reverse();
                    }
                    assert_eq!(
                        member.vertices.len(),
                        own.len(),
                        "{what}: as many vertices as the record's curve sampled"
                    );
                    for (vertex, on_curve) in member.vertices.iter().zip(&own) {
                        assert!(
                            distance(*vertex, *on_curve) < 1e-12,
                            "{what}: {vertex:?} against the record's curve at {on_curve:?}"
                        );
                    }
                    held.splines += 1;
                }
                (tag, class) => panic!("{what}: tag 0x{tag:02X} under class word 0x{class:04X}"),
            }
        }

        // The members run on from one another and back to the first: within
        // ten micrometres, which is what the file's own arcs leave (an arc
        // of each stripe overshoots its line by 4 µm).
        for (index, member) in members.iter().enumerate() {
            let next = &members[(index + 1) % members.len()];
            let gap = distance(member.vertices[member.vertices.len() - 1], next.vertices[0]);
            assert!(
                gap < 1e-5,
                "{site}: member {index} ends {} m from where the next begins",
                gap
            );
        }
        // The anchor is the point the fill was seeded at: inside the ring.
        let ring: Vec<Point> = boundary
            .segments
            .iter()
            .map(|segment| segment.start)
            .collect();
        assert!(
            contains(&ring, boundary.anchor),
            "{site}: the anchor {:?} lies inside the fill",
            boundary.anchor
        );
        areas.push((
            members.iter().map(|member| member.tag).collect(),
            signed_area(&ring).abs() * 1e6,
        ));
    }
    (held, areas)
}

#[test]
fn the_w_templates_fills_decode_and_every_member_is_the_curve_its_reference_names() {
    // The five fills by their member tags and their areas, in an order of
    // their own: a template's chain keeps them in an order of its own too.
    let mut expected: Vec<(Vec<u8>, f64)> = FILLS
        .iter()
        .map(|fill| (fill.tags.to_vec(), fill.area_mm2))
        .collect();
    let in_order = |fills: &mut Vec<(Vec<u8>, f64)>| {
        fills.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    };
    in_order(&mut expected);

    for name in ["A0-W.igr", "A1-W.igr", "A2-W.igr", "A3-W.igr"] {
        let package = archived(name);
        let data = package.streams["/Sheet6"].data.as_slice();
        let (held, mut fills) = hold_the_logo(name, data);
        // 12 lines, 6 arcs of a circle, 4 of the ellipse and 4 B-splines
        // over the five fills; 13 run their record's way and 13 against it.
        assert_eq!(
            held,
            Held {
                lines: 12,
                arcs: 6,
                elliptical_arcs: 4,
                splines: 4,
                forward: 13,
                reversed: 13,
            },
            "{name}: what the logo's 26 members were held against"
        );
        // One logo, placed four times: the same members, the same areas.
        in_order(&mut fills);
        eprintln!(
            "{name}: fills of {:?} mm²",
            fills.iter().map(|(_, area)| *area).collect::<Vec<_>>()
        );
        assert_eq!(
            fills.iter().map(|(tags, _)| tags).collect::<Vec<_>>(),
            expected.iter().map(|(tags, _)| tags).collect::<Vec<_>>(),
            "{name}: the members of the five fills"
        );
        for ((_, area), (_, measured)) in fills.iter().zip(&expected) {
            assert!(
                (area - measured).abs() < 1e-4,
                "{name}: a fill of {area:.6} mm², measured {measured:.6} on A1-W.igr"
            );
        }
    }

    // `A1-W.igr`, fill by fill.
    let package = archived("A1-W.igr");
    let data = package.streams["/Sheet6"].data.as_slice();
    let boundaries = decode_igboundaries(data);
    assert_eq!(
        boundaries
            .iter()
            .map(|boundary| boundary.oid)
            .collect::<Vec<_>>(),
        FILLS.iter().map(|fill| fill.a1_oid).collect::<Vec<_>>(),
        "the sheet's fills, in chain order"
    );
    for (boundary, fill) in boundaries.iter().zip(&FILLS) {
        let members = members_of(data, boundary);
        assert_eq!(
            members.iter().map(|member| member.tag).collect::<Vec<_>>(),
            fill.tags,
            "oid {}, {}: member tags",
            boundary.oid,
            fill.what
        );
        let ring: Vec<Point> = boundary
            .segments
            .iter()
            .map(|segment| segment.start)
            .collect();
        let area = signed_area(&ring).abs() * 1e6;
        assert!(
            (area - fill.area_mm2).abs() < 1e-4,
            "oid {}, {}: {area:.6} mm², measured {:.6}",
            boundary.oid,
            fill.what,
            fill.area_mm2
        );
        assert_eq!(
            (boundary.sheet_layer_ref, boundary.index),
            (8, 15),
            "oid {}: layer and style",
            boundary.oid
        );
    }
}

/// Each fill reaches the sheet as one ring to be filled, and the template
/// refuses nothing.
#[test]
fn the_w_templates_fills_reach_the_sheet_and_nothing_is_refused() {
    for name in ["A0-W.igr", "A1-W.igr", "A2-W.igr", "A3-W.igr"] {
        let package = archived(name);
        let data = package.streams["/Sheet6"].data.as_slice();
        let normalized = build_normalized_geometry(&package.parsed);
        let fills = pid_parse::style_link::fill_styles_for_document(&package.parsed);
        let boundaries = decode_igboundaries(data);
        assert_eq!(boundaries.len(), 5, "{name}: the logo's five fills decode");
        for boundary in &boundaries {
            let entity = normalized
                .entities
                .iter()
                .find(|entity| {
                    entity.graphic_oid == Some(boundary.oid)
                        && entity.source.stream_path.as_deref() == Some("/Sheet6")
                })
                .unwrap_or_else(|| {
                    panic!(
                        "{name} oid {} reaches no entity of the template's sheet",
                        boundary.oid
                    )
                });
            let PidGraphicKind::Polyline { points, closed } = &entity.kind else {
                panic!("{name} oid {} is drawn as {:?}", boundary.oid, entity.kind);
            };
            assert!(*closed, "{name} oid {} is a ring", boundary.oid);
            assert!(
                entity
                    .source
                    .record_id
                    .as_deref()
                    .is_some_and(|id| id.starts_with("igboundary2d")),
                "{name} oid {} is told apart as a boundary, which is what makes a renderer fill \
                 it: {:?}",
                boundary.oid,
                entity.source.record_id
            );
            // The ring is the record's chords, one vertex a chord.
            assert_eq!(
                points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>(),
                boundary
                    .segments
                    .iter()
                    .map(|segment| segment.start)
                    .collect::<Vec<_>>(),
                "{name} oid {}: the ring's vertices",
                boundary.oid
            );
            assert!(
                fills.contains_key(&("/Sheet6".to_string(), boundary.oid)),
                "{name} oid {}: style {} resolves to a fill",
                boundary.oid,
                boundary.index
            );
        }
        assert!(
            normalized.refused_graphic_records.is_empty(),
            "{name} refuses: {:?}",
            normalized.refused_graphic_records
        );
        assert!(
            normalized.dropped_graphic_records.is_empty(),
            "{name} has records without a decoder: {:?}",
            normalized.dropped_graphic_records
        );
    }
}

/// The `.pid` templates beside them embed the same frame, and its fills
/// decode there too: each in the nested sheet of the storage that holds the
/// border.
#[test]
fn the_w_drawing_templates_embedded_frames_decode_the_same_fills() {
    for (name, frame) in [
        ("A0-W.pid", "/JSite289/Sheet6"),
        ("A1-W.pid", "/JSite247/Sheet6"),
        ("A2-W.pid", "/JSite106/Sheet6"),
        ("A3-W.pid", "/JSite117/Sheet6"),
    ] {
        let package = archived(name);
        let data = package
            .streams
            .get(frame)
            .unwrap_or_else(|| panic!("{name} has a {frame}"))
            .data
            .as_slice();
        let (held, _) = hold_the_logo(name, data);
        assert_eq!(
            (held.lines, held.arcs, held.elliptical_arcs, held.splines),
            (12, 6, 4, 4),
            "{name} {frame}: the logo's 26 members"
        );
        let normalized = build_normalized_geometry(&package.parsed);
        assert!(
            normalized.refused_graphic_records.is_empty(),
            "{name} refuses: {:?}",
            normalized.refused_graphic_records
        );
    }
}

/// A boundary that names no member states no anchor.
///
/// The two doubles where a boundary keeps the point its fill was seeded at
/// are both `1e30` on every record whose reference count is zero, and a
/// point inside the fill on every record that names its members: the
/// 23 distinct templates and drawings the repository commits hold 71 of the
/// first kind and 61 of the second, and the 111 distinct documents of the
/// corpora 100 and 1 643
/// (`examples/probe_what_curve_each_boundary_member_relists.rs`).
///
/// For a record of the first kind the decoder's `anchor` is not the file's:
/// it is the first vertex of the ring, a point on the fill's edge. That is
/// all there is to the 21 boundaries of `CPECCHBA2.igr` and
/// `qsmcqtaz13A2.igr` whose anchor was found outside their ring — the two
/// logos name no member, so there is no seed to be inside it.
#[test]
fn a_boundary_that_names_no_member_states_no_anchor() {
    for (name, count) in [("CPECCHBA2.igr", 14usize), ("qsmcqtaz13A2.igr", 7)] {
        let package = archived(name);
        let data = package.streams["/Sheet6"].data.as_slice();
        let boundaries = decode_igboundaries(data);
        assert_eq!(boundaries.len(), count, "{name}: its fills");
        for boundary in &boundaries {
            let site = format!("{name} oid {}", boundary.oid);
            assert!(boundary.member_refs.is_empty(), "{site} names no member");
            // The trailer is the last 21 bytes: two doubles, a flag byte,
            // the reference count.
            let payload = &data[boundary.byte_range.start + 6..boundary.byte_range.end];
            let trailer = &payload[payload.len() - 21..];
            let double =
                |at: usize| f64::from_le_bytes(trailer[at..at + 8].try_into().expect("8 bytes"));
            assert_eq!(
                (double(0), double(8)),
                (1e30, 1e30),
                "{site}: the file states no anchor"
            );
            assert_eq!(
                (trailer[16], &trailer[17..21]),
                (1, &[0u8; 4][..]),
                "{site}: the flag byte, and a reference count of zero"
            );
            // What the record decodes with instead: its ring's first vertex.
            assert_eq!(
                boundary.anchor, boundary.segments[0].start,
                "{site}: the decoded anchor is the ring's first vertex"
            );
        }
    }

    // The rule, on every boundary of every document committed on disk and in
    // the archive: no reference, no anchor; a reference, an anchor inside
    // the fill — inside an odd number of its loops.
    let mut documents: Vec<(String, PidPackage)> = Vec::new();
    for name in [
        "A0-W.igr",
        "A1-W.igr",
        "A2-W.igr",
        "A3-W.igr",
        "CPECCHBA2.igr",
        "qsmcqtaz13A2.igr",
    ] {
        documents.push((name.to_string(), archived(name)));
    }
    for path in [
        "test-file/Ref/Template Files/XA-A1.igr",
        "test-file/Ref/Template Files/CPECCHB-SQ-A2.igr",
        "test-file/DWG-0202GP06-01.pid",
        "test-file/工艺管道及仪表流程-1.pid",
    ] {
        let package = PidParser::new()
            .parse_package(path)
            .unwrap_or_else(|error| panic!("{path} is committed and parses: {error}"));
        documents.push((path.to_string(), package));
    }
    let (mut stated, mut unstated) = (0usize, 0usize);
    for (name, package) in &documents {
        for (stream, raw) in &package.streams {
            if !stream
                .rsplit('/')
                .next()
                .is_some_and(|leaf| leaf.starts_with("Sheet"))
            {
                continue;
            }
            let data = raw.data.as_slice();
            for boundary in decode_igboundaries(data) {
                let site = format!("{name} {stream} oid {}", boundary.oid);
                let payload = &data[boundary.byte_range.start + 6..boundary.byte_range.end];
                let anchor_at = payload.len() - 21 - 8 * boundary.member_refs.len();
                let double = |at: usize| {
                    f64::from_le_bytes(payload[at..at + 8].try_into().expect("8 bytes"))
                };
                let on_file = (double(anchor_at), double(anchor_at + 8));
                if boundary.member_refs.is_empty() {
                    assert_eq!(on_file, (1e30, 1e30), "{site}: no reference, no anchor");
                    unstated += 1;
                } else {
                    assert_eq!(on_file, boundary.anchor, "{site}: the anchor is the file's");
                    let around = boundary
                        .loops()
                        .filter(|segments| {
                            let ring: Vec<Point> =
                                segments.iter().map(|segment| segment.start).collect();
                            contains(&ring, boundary.anchor)
                        })
                        .count();
                    assert_eq!(
                        around % 2,
                        1,
                        "{site}: the anchor {:?} lies inside the fill",
                        boundary.anchor
                    );
                    stated += 1;
                }
            }
        }
    }
    // 38 name their members: the twenty of the `-W` logos, three of
    // `XA-A1.igr`, five of DWG-0202 and ten of the 熊安 fixture. 29 do not:
    // the two logos above, `CPECCHB-SQ-A2.igr`'s seven and one of
    // `XA-A1.igr`.
    assert_eq!(
        (stated, unstated),
        (38, 29),
        "boundaries that name their members and state an anchor, and those that do neither"
    );
}
