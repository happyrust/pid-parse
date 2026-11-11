//! A boundary of two loops fills the area between them.
//!
//! `XA-A1.igr`, the border template the 熊安 fixture wears, letters its logo
//! with four `igBoundary2d` fills on `/Sheet6`. Two of them — oid 5074 and
//! oid 5076 — are letters with a counter: an outer loop of 104 and of 162
//! lines, and inside each a second loop of 77 and of 54 lines wound the
//! other way. They are the only boundaries of more than one loop among the
//! 2 158 the corpora keep on sheet chains, and the decoder, which held the
//! `u32` at payload `+18` to `1`, refused both: the logo printed one letter
//! of three.
//!
//! What says the second loop is a hole, on a template no vendor plot wears:
//!
//! * the record's own anchor — the point the fill was seeded at — lies inside
//!   the loop on every one of the 2 049 one-loop records that carry member
//!   references, and on these two it lies inside the outer loop and outside
//!   the inner one;
//! * the two loops are wound opposite ways, the inner one wholly inside the
//!   outer;
//! * the vendor's plots paint every fill with `f*`, the even-odd rule, under
//!   which a loop inside a loop is unpainted whichever way it is wound.
//!
//! See `docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
//! §10.6.

use pid_parse::parsers::sheet_records::{
    decode_igboundaries, decode_iglines, parse_psm_header, sheet_record_starts,
};
use pid_parse::{build_normalized_geometry, PidGraphicKind, PidParser};

const XA_TEMPLATE: &str = "test-file/Ref/Template Files/XA-A1.igr";

/// One fill with a counter, as measured on the file: the boundary's oid, the
/// area between its two loops in mm², its anchor and a point inside its
/// counter in mm, and the members of each loop.
struct Letter {
    oid: u32,
    area_mm2: f64,
    anchor_mm: (f64, f64),
    in_the_counter_mm: (f64, f64),
    members: [usize; 2],
    references: usize,
}

const LETTERS: [Letter; 2] = [
    Letter {
        oid: 5074,
        area_mm2: 40.495_377,
        anchor_mm: (666.927_4, 58.127_3),
        in_the_counter_mm: (669.331_3, 60.172_3),
        members: [104, 77],
        references: 181,
    },
    Letter {
        oid: 5076,
        area_mm2: 36.024_948,
        anchor_mm: (658.460_5, 58.076_3),
        in_the_counter_mm: (660.642_6, 57.194_3),
        members: [162, 54],
        // One more than its 216 members: it also names oid 4836, a line
        // 32 µm long that the outer loop steps over.
        references: 217,
    },
];

/// Whether `point` is inside the closed ring by the even-odd rule.
fn contains(ring: &[(f64, f64)], point: (f64, f64)) -> bool {
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
fn signed_area(ring: &[(f64, f64)]) -> f64 {
    ring.iter()
        .enumerate()
        .map(|(index, &(x0, y0))| {
            let (x1, y1) = ring[(index + 1) % ring.len()];
            x0 * y1 - x1 * y0
        })
        .sum::<f64>()
        / 2.0
}

#[test]
fn a_two_loop_boundary_reaches_the_page_as_the_area_between_its_loops() {
    if !std::path::Path::new(XA_TEMPLATE).exists() {
        eprintln!("skipping: fixture {XA_TEMPLATE} not found");
        return;
    }
    let package = PidParser::new()
        .parse_package(XA_TEMPLATE)
        .unwrap_or_else(|e| panic!("Failed to parse {XA_TEMPLATE}: {e}"));
    let normalized = build_normalized_geometry(&package.parsed);

    for letter in &LETTERS {
        let entity = normalized
            .entities
            .iter()
            .find(|entity| {
                entity.graphic_oid == Some(letter.oid)
                    && entity.source.stream_path.as_deref() == Some("/Sheet6")
            })
            .unwrap_or_else(|| {
                panic!(
                    "oid {} reaches no entity of the template's sheet",
                    letter.oid
                )
            });
        let PidGraphicKind::Polyline { points, closed } = &entity.kind else {
            panic!("oid {} is drawn as {:?}", letter.oid, entity.kind);
        };
        assert!(*closed, "oid {} is a ring", letter.oid);
        assert!(
            entity
                .source
                .record_id
                .as_deref()
                .is_some_and(|id| id.starts_with("igboundary2d")),
            "oid {} is told apart as a boundary, which is what makes a renderer fill it: {:?}",
            letter.oid,
            entity.source.record_id
        );
        // Both loops, the outer one closed on its first vertex before the
        // ring crosses to the inner one, which closes on its own.
        assert_eq!(
            points.len(),
            letter.members[0] + letter.members[1] + 2,
            "oid {}: ring vertices",
            letter.oid
        );
        let ring: Vec<(f64, f64)> = points.iter().map(|p| (p.x * 1e3, p.y * 1e3)).collect();
        // The ring's own area is the outer loop's less the counter's only if
        // the counter is walked against the outer loop; walked with it, the
        // two would add, and a painter filling by winding number would
        // paint the counter in.
        let area = signed_area(&ring).abs();
        assert!(
            (area - letter.area_mm2).abs() < 1e-4,
            "oid {}: the ring encloses {area:.6} mm², the area between the loops is {:.6} mm²",
            letter.oid,
            letter.area_mm2
        );
        assert!(
            contains(&ring, letter.anchor_mm),
            "oid {}: the anchor {:?} is in the painted area",
            letter.oid,
            letter.anchor_mm
        );
        assert!(
            !contains(&ring, letter.in_the_counter_mm),
            "oid {}: {:?} is in the counter, which stays unpainted",
            letter.oid,
            letter.in_the_counter_mm
        );
    }

    // Nothing on the sheet is refused any more, and nothing lacks a decoder.
    assert!(
        normalized.refused_graphic_records.is_empty(),
        "refused: {:?}",
        normalized.refused_graphic_records
    );
    assert!(
        normalized.dropped_graphic_records.is_empty(),
        "without a decoder: {:?}",
        normalized.dropped_graphic_records
    );
}

#[test]
fn a_two_loop_boundary_decodes_its_loops_its_anchor_and_its_references() {
    if !std::path::Path::new(XA_TEMPLATE).exists() {
        eprintln!("skipping: fixture {XA_TEMPLATE} not found");
        return;
    }
    let package = PidParser::new()
        .parse_package(XA_TEMPLATE)
        .unwrap_or_else(|e| panic!("Failed to parse {XA_TEMPLATE}: {e}"));
    let data = package
        .streams
        .get("/Sheet6")
        .unwrap_or_else(|| panic!("{XA_TEMPLATE} has a /Sheet6"))
        .data
        .as_slice();
    // The live `igLine2d` records of the sheet, by oid.
    let lines: Vec<u32> = decode_iglines(data).iter().map(|line| line.oid).collect();

    let boundaries = decode_igboundaries(data);
    assert_eq!(
        boundaries
            .iter()
            .map(|boundary| (boundary.oid, boundary.loop_segment_counts.len()))
            .collect::<Vec<_>>(),
        [(3923, 0), (5069, 0), (5074, 2), (5076, 2)],
        "the sheet's four fills, and how many loops each of the two with a counter states"
    );

    for letter in &LETTERS {
        let boundary = boundaries
            .iter()
            .find(|boundary| boundary.oid == letter.oid)
            .expect("listed above");
        assert_eq!(
            boundary
                .loop_segment_counts
                .iter()
                .map(|count| *count as usize)
                .collect::<Vec<_>>(),
            letter.members,
            "oid {}: every member is a line, so a loop has one segment a member",
            letter.oid
        );
        assert_eq!(
            boundary.segment_count as usize,
            letter.members[0] + letter.members[1],
            "oid {}: members over both loops",
            letter.oid
        );
        assert_eq!(boundary.sub_type_word, 0x0000);
        assert_eq!(boundary.trailer_flag, 1);

        // Each loop as the ring of its segments' starts, in millimetres.
        let rings: Vec<Vec<(f64, f64)>> = boundary
            .loops()
            .map(|segments| {
                segments
                    .iter()
                    .map(|segment| (segment.start.0 * 1e3, segment.start.1 * 1e3))
                    .collect()
            })
            .collect();
        assert_eq!(rings.len(), 2, "oid {}", letter.oid);
        // Every loop ends where it began.
        for segments in boundary.loops() {
            let (first, last) = (segments[0].start, segments[segments.len() - 1].end);
            assert!(
                (first.0 - last.0).hypot(first.1 - last.1) < 1e-12,
                "oid {}: a loop ends {last:?} and began {first:?}",
                letter.oid
            );
        }
        // Wound opposite ways, the second wholly inside the first.
        let (outer, inner) = (signed_area(&rings[0]), signed_area(&rings[1]));
        assert!(
            outer > 0.0 && inner < 0.0,
            "oid {}: loop areas {outer:.6} and {inner:.6} mm²",
            letter.oid
        );
        assert!(
            (outer + inner - letter.area_mm2).abs() < 1e-4,
            "oid {}: {outer:.6} less {:.6} mm²",
            letter.oid,
            -inner
        );
        assert!(
            rings[1].iter().all(|vertex| contains(&rings[0], *vertex)),
            "oid {}: the second loop lies inside the first",
            letter.oid
        );
        // The anchor is in the area that is painted: inside the outer loop,
        // outside the counter.
        let anchor = (boundary.anchor.0 * 1e3, boundary.anchor.1 * 1e3);
        assert!(
            (anchor.0 - letter.anchor_mm.0).abs() < 1e-3
                && (anchor.1 - letter.anchor_mm.1).abs() < 1e-3,
            "oid {}: anchor {anchor:?}",
            letter.oid
        );
        assert!(
            contains(&rings[0], anchor) && !contains(&rings[1], anchor),
            "oid {}: the anchor lies between the loops",
            letter.oid
        );
        // The references close the record, and each names a live line of
        // the sheet under the class word every other boundary carries.
        assert_eq!(
            boundary.member_refs.len(),
            letter.references,
            "oid {}: references",
            letter.oid
        );
        for reference in &boundary.member_refs {
            assert_eq!(reference.class_word, 0x00CB, "oid {}", letter.oid);
            assert!(
                lines.contains(&reference.member_oid),
                "oid {}: reference {} names no live igLine2d of the sheet",
                letter.oid,
                reference.member_oid
            );
        }
        // The record is on the chain and ends where the chain's next begins.
        let end = sheet_record_starts(data)
            .into_iter()
            .find(|at| *at == boundary.byte_range.start)
            .and_then(|at| parse_psm_header(data, at))
            .map(|header| header.body_start + header.bytes_to_follow as usize);
        assert_eq!(
            end,
            Some(boundary.byte_range.end),
            "oid {} covers one chain record",
            letter.oid
        );
    }
}
