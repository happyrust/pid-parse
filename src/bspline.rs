//! Evaluate the B-spline curves a `.pid` or `.sym` stores as
//! `igBspCurve2d` records (poles, optional weights, a knot vector), so a
//! renderer that draws straight segments can draw them.
//!
//! One routine, [`sample`], and it is de Boor's algorithm on homogeneous
//! coordinates: rational curves fall out of the same loop as polynomial ones
//! by carrying the weight as a third coordinate and dividing at the end. The
//! degree is not stored in the record; it is `knots - poles - 1`, which is
//! how the corpus's one curve (five poles, nine knots) reads as the clamped
//! cubic it draws as.

use std::cmp::Ordering;

/// How many straight segments each knot span is drawn with, by every consumer
/// that draws the curve as a polyline (the sheet-level emitter here, the
/// symbol-body renderer downstream). The corpus's curve is a 2 mm lip of two
/// spans; eight a span keeps its chord error under a hundredth of a
/// millimetre.
pub const SEGMENTS_PER_SPAN: usize = 8;

/// Points along the curve, from the first to the last usable knot.
///
/// The curve is walked span by span -- every interval between two distinct
/// knots inside the parameter domain -- with `per_span` segments each, so a
/// tight span gets as many segments as a wide one and the sampling follows
/// the knot vector's own idea of where the shape changes. The first point is
/// the curve at the domain's start and the last point the curve at its end;
/// for a clamped knot vector those are the first and last poles.
///
/// Returns the poles themselves when the record cannot be a curve: fewer than
/// two poles, a knot vector too short for degree one, or a weight list that
/// is neither empty nor one per pole. A consumer then draws the control
/// polygon, which is wrong in a visible way rather than a silent way.
pub fn sample(
    poles: &[(f64, f64)],
    weights: &[f64],
    knots: &[f64],
    per_span: usize,
) -> Vec<(f64, f64)> {
    let n = poles.len();
    if n < 2 || knots.len() < n + 2 || !(weights.is_empty() || weights.len() == n) {
        return poles.to_vec();
    }
    let degree = knots.len() - n - 1;
    let per_span = per_span.max(1);
    // The parameter domain of a B-spline with this many knots.
    let start = knots[degree];
    let end = knots[n];
    // `partial_cmp` rather than `<=`, so a NaN knot ends here too.
    if end.partial_cmp(&start) != Some(Ordering::Greater) {
        return poles.to_vec();
    }
    let homogeneous: Vec<[f64; 3]> = poles
        .iter()
        .enumerate()
        .map(|(index, (x, y))| {
            let w = weights.get(index).copied().unwrap_or(1.0);
            [x * w, y * w, w]
        })
        .collect();

    let mut out = Vec::new();
    for span in degree..n {
        let (lo, hi) = (knots[span], knots[span + 1]);
        if hi.partial_cmp(&lo) != Some(Ordering::Greater) {
            continue;
        }
        for step in 0..per_span {
            let u = lo + (hi - lo) * (step as f64) / (per_span as f64);
            out.push(de_boor(&homogeneous, knots, degree, span, u));
        }
    }
    // The end of the domain, evaluated on the last non-empty span so the
    // clamped end pole comes out exactly.
    if let Some(last_span) = (degree..n)
        .rev()
        .find(|&span| knots[span + 1] > knots[span])
    {
        out.push(de_boor(&homogeneous, knots, degree, last_span, end));
    }
    out
}

/// The curve at `u`, which lies in `[knots[span], knots[span + 1])`, by the
/// triangular de Boor recursion over the `degree + 1` poles that span uses.
fn de_boor(poles: &[[f64; 3]], knots: &[f64], degree: usize, span: usize, u: f64) -> (f64, f64) {
    let mut d: Vec<[f64; 3]> = (0..=degree).map(|j| poles[j + span - degree]).collect();
    for r in 1..=degree {
        for j in (r..=degree).rev() {
            let i = j + span - degree;
            let denominator = knots[i + degree - r + 1] - knots[i];
            let alpha = if denominator.abs() < f64::EPSILON {
                0.0
            } else {
                (u - knots[i]) / denominator
            };
            d[j] = [
                (1.0 - alpha) * d[j - 1][0] + alpha * d[j][0],
                (1.0 - alpha) * d[j - 1][1] + alpha * d[j][1],
                (1.0 - alpha) * d[j - 1][2] + alpha * d[j][2],
            ];
        }
    }
    let [x, y, w] = d[degree];
    if w.abs() < f64::EPSILON {
        (x, y)
    } else {
        (x / w, y / w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-12 && (a.1 - b.1).abs() < 1e-12
    }

    #[test]
    fn a_degree_one_spline_is_its_own_control_polygon() {
        let poles = [(0.0, 0.0), (1.0, 2.0), (3.0, 2.0)];
        let knots = [0.0, 0.0, 0.5, 1.0, 1.0];
        let points = sample(&poles, &[], &knots, 2);
        assert_eq!(points.len(), 5);
        assert!(close(points[0], (0.0, 0.0)));
        assert!(close(points[1], (0.5, 1.0)));
        assert!(close(points[2], (1.0, 2.0)));
        assert!(close(points[3], (2.0, 2.0)));
        assert!(close(points[4], (3.0, 2.0)));
    }

    #[test]
    fn a_quadratic_bezier_hits_its_midpoint() {
        // With knots [0,0,0,1,1,1] a three-pole quadratic is a Bezier curve,
        // and B(1/2) = (P0 + 2 P1 + P2) / 4.
        let poles = [(0.0, 0.0), (2.0, 4.0), (4.0, 0.0)];
        let knots = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        let points = sample(&poles, &[], &knots, 2);
        assert_eq!(points.len(), 3);
        assert!(close(points[0], (0.0, 0.0)));
        assert!(close(points[1], (2.0, 2.0)));
        assert!(close(points[2], (4.0, 0.0)));
    }

    #[test]
    fn the_corpus_cubic_starts_and_ends_on_its_end_poles_and_stays_inside_its_hull() {
        let poles = [
            (0.005_041, 0.005_076),
            (0.005_379, 0.004_890),
            (0.006_078, 0.004_280),
            (0.005_379, 0.003_669),
            (0.005_041, 0.003_483),
        ];
        let knots = [0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0];
        let points = sample(&poles, &[], &knots, 8);
        assert_eq!(
            points.len(),
            17,
            "two spans of eight segments, plus the end"
        );
        assert!(close(points[0], poles[0]));
        assert!(close(points[16], poles[4]));
        for (x, y) in &points {
            assert!((0.005_041..=0.006_078).contains(x), "x {x} left the hull");
            assert!((0.003_483..=0.005_076).contains(y), "y {y} left the hull");
        }
        // The middle of the curve bulges towards the middle pole.
        assert!(points[8].0 > 0.0055);
    }

    #[test]
    fn a_rational_weight_pulls_the_curve_towards_its_pole() {
        let poles = [(0.0, 0.0), (2.0, 4.0), (4.0, 0.0)];
        let knots = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        let plain = sample(&poles, &[], &knots, 2)[1];
        let pulled = sample(&poles, &[1.0, 4.0, 1.0], &knots, 2)[1];
        assert!(
            pulled.1 > plain.1,
            "a heavier middle pole should raise the midpoint"
        );
        assert!(
            (pulled.0 - 2.0).abs() < 1e-12,
            "symmetry keeps the midpoint centred"
        );
    }

    #[test]
    fn a_record_that_cannot_be_a_curve_falls_back_to_its_poles() {
        let poles = [(0.0, 0.0), (1.0, 1.0)];
        assert_eq!(sample(&poles, &[], &[0.0, 1.0], 4), poles.to_vec());
        assert_eq!(
            sample(&poles, &[1.0], &[0.0, 0.0, 1.0, 1.0], 4),
            poles.to_vec()
        );
        assert_eq!(
            sample(&poles[..1], &[], &[0.0, 0.0, 1.0, 1.0], 4),
            poles[..1].to_vec()
        );
        // A degenerate domain (all knots equal) is not a curve either.
        assert_eq!(
            sample(&poles, &[], &[1.0, 1.0, 1.0, 1.0], 4),
            poles.to_vec()
        );
    }
}
