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

/// The poles, weights and knots of an elliptical arc as an **exact**
/// rational quadratic B-spline -- what [`sample`] and a
/// [`crate::symbol_library::SymbolPrimitive::BSpline`] take.
///
/// The ellipse is `P(t) = center + cos t · major + sin t · minor`, with
/// `minor = ratio · (−major.y, major.x)`: the major semi-axis turned +90°.
/// The arc runs **clockwise** -- decreasing `t` -- from `t = sweep_start`
/// to `t = sweep_end`, the convention `igEllipticalArc2d` shares with
/// `igArc2d` (see
/// [`crate::parsers::sheet_records::SheetIgEllipticalArc2dDecoded`]). Its
/// sweep is `Δ = (sweep_start − sweep_end) mod 2π`, and a pair that is a
/// whole number of turns apart without being equal is a full turn.
///
/// An ellipse is a conic, so this has no approximation error: the arc is
/// cut into `n = ceil(Δ / 45°)` equal segments (`1 ≤ n ≤ 8`, `δ = Δ / n`),
/// each a rational quadratic Bézier whose end poles lie on the ellipse and
/// whose middle pole is the ellipse point at the segment's middle parameter
/// pushed out from the centre by `1 / cos(δ/2)`, with weight `cos(δ/2)`;
/// every other weight is 1. The knots are
/// `[0,0,0, 1,1, 2,2, …, n−1,n−1, n,n,n]`, `2n + 4` of them for `2n + 1`
/// poles, which [`sample`] reads as degree 2. Sampled with
/// [`SEGMENTS_PER_SPAN`] segments a span, each straight segment covers at
/// most 45° / 8 ≈ 5.6° of parameter: a chord error of about 0.03 mm on
/// A01's vessel heads (a 25.96 mm major semi-axis).
///
/// `None` when the sweep is degenerate (`sweep_start == sweep_end`), the
/// ellipse is (a zero major semi-axis, a ratio that is not positive), or an
/// input -- or a pole it leads to -- is not finite. Panic-free for every
/// input, NaN, infinities and huge angles included.
#[allow(clippy::type_complexity)]
pub fn elliptical_arc(
    center: (f64, f64),
    major: (f64, f64),
    ratio: f64,
    sweep_start: f64,
    sweep_end: f64,
) -> Option<(Vec<(f64, f64)>, Vec<f64>, Vec<f64>)> {
    use std::f64::consts::{FRAC_PI_4, TAU};

    let inputs = [
        center.0,
        center.1,
        major.0,
        major.1,
        ratio,
        sweep_start,
        sweep_end,
    ];
    if inputs.iter().any(|value| !value.is_finite())
        || sweep_start == sweep_end
        || major.0.hypot(major.1) <= 0.0
        || ratio <= 0.0
    {
        return None;
    }
    // Huge angles can overflow the difference; `rem_euclid` of an infinity
    // is NaN, and a NaN sweep has no segment count.
    let mut sweep = (sweep_start - sweep_end).rem_euclid(TAU);
    if !sweep.is_finite() {
        return None;
    }
    if sweep == 0.0 {
        sweep = TAU;
    }
    // `sweep` is in (0, 2π] here, so this is 1..=8; the clamp only keeps a
    // rounding slip from ever reaching the cast.
    let segments = (sweep / FRAC_PI_4).ceil().clamp(1.0, 8.0) as usize;
    let step = sweep / segments as f64;
    // At most π/8, so at least cos(π/8) ≈ 0.92: never a zero divisor.
    let half_cos = (step / 2.0).cos();
    let minor = (-major.1 * ratio, major.0 * ratio);
    // The ellipse point at `t`, pushed out from the centre by `1 / scale`.
    let point = |t: f64, scale: f64| {
        let (sin, cos) = t.sin_cos();
        (
            center.0 + (cos * major.0 + sin * minor.0) / scale,
            center.1 + (cos * major.1 + sin * minor.1) / scale,
        )
    };
    let mut poles = Vec::with_capacity(2 * segments + 1);
    let mut weights = Vec::with_capacity(2 * segments + 1);
    for k in 0..segments {
        let k = k as f64;
        poles.push(point(sweep_start - k * step, 1.0));
        weights.push(1.0);
        poles.push(point(sweep_start - (k + 0.5) * step, half_cos));
        weights.push(half_cos);
    }
    poles.push(point(sweep_start - segments as f64 * step, 1.0));
    weights.push(1.0);
    if poles.iter().any(|(x, y)| !x.is_finite() || !y.is_finite()) {
        return None;
    }
    let mut knots = Vec::with_capacity(2 * segments + 4);
    knots.extend([0.0; 3]);
    for k in 1..segments {
        knots.extend([k as f64; 2]);
    }
    knots.extend([segments as f64; 3]);
    Some((poles, weights, knots))
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

    use std::f64::consts::{FRAC_PI_4, PI, TAU};

    /// Seeded xorshift64 (no test dependency), seeded through splitmix64
    /// as in `parsers::tests`; `| 1` keeps the state non-zero.
    struct XorShift64(u64);

    impl XorShift64 {
        fn new(seed: u64) -> Self {
            let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            Self((z ^ (z >> 31)) | 1)
        }

        fn next_u64(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }

        /// Uniform in `[0, 1)`.
        fn unit(&mut self) -> f64 {
            (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
        }

        /// Uniform in `[lo, hi)`.
        fn range(&mut self, lo: f64, hi: f64) -> f64 {
            lo + (hi - lo) * self.unit()
        }
    }

    /// `P(t) = C + cos t · major + sin t · minor`, written out
    /// independently of [`elliptical_arc`].
    fn ellipse_at(center: (f64, f64), major: (f64, f64), ratio: f64, t: f64) -> (f64, f64) {
        let minor = (-major.1 * ratio, major.0 * ratio);
        (
            center.0 + t.cos() * major.0 + t.sin() * minor.0,
            center.1 + t.cos() * major.1 + t.sin() * minor.1,
        )
    }

    /// `p` in the ellipse's own frame, in semi-axis units: `u` along the
    /// major semi-axis, `v` along the minor one. On the ellipse,
    /// `u² + v² = 1`.
    fn frame_of(p: (f64, f64), center: (f64, f64), major: (f64, f64), ratio: f64) -> (f64, f64) {
        let minor = (-major.1 * ratio, major.0 * ratio);
        let d = (p.0 - center.0, p.1 - center.1);
        (
            (d.0 * major.0 + d.1 * major.1) / (major.0 * major.0 + major.1 * major.1),
            (d.0 * minor.0 + d.1 * minor.1) / (minor.0 * minor.0 + minor.1 * minor.1),
        )
    }

    fn sampled(
        center: (f64, f64),
        major: (f64, f64),
        ratio: f64,
        sweep_start: f64,
        sweep_end: f64,
    ) -> Vec<(f64, f64)> {
        let (poles, weights, knots) = elliptical_arc(center, major, ratio, sweep_start, sweep_end)
            .expect("a proper elliptical arc converts");
        sample(&poles, &weights, &knots, SEGMENTS_PER_SPAN)
    }

    /// Feature: pid-a01-elliptical-heads, Property 1: the rational B-spline
    /// of an elliptical arc samples onto its ellipse (`u² + v² = 1` within
    /// 1e-9), starts at `P(start)`, ends at `P(end)` and moves clockwise,
    /// for any centre, major axis, ratio in (0, 1] and sweep up to a full
    /// turn.
    #[test]
    fn an_elliptical_arc_samples_onto_its_ellipse_clockwise_from_start_to_end() {
        // Stored pairs exactly a whole turn apart: a full turn each.
        let full_turns = [(TAU, 0.0), (0.0, -TAU), (PI, -PI), (2.0 * TAU, TAU)];
        let mut rng = XorShift64::new(0x0E11_1971_CA12_0930);
        for case in 0..2000usize {
            let center = (rng.range(-1.0, 1.0), rng.range(-1.0, 1.0));
            let length = rng.range(0.01, 1.0);
            let direction = rng.range(-PI, PI);
            let major = (length * direction.cos(), length * direction.sin());
            let ratio = if case % 10 == 0 {
                1.0
            } else {
                1.0 - 0.95 * rng.unit()
            };
            let (sweep_start, sweep_end, sweep) = if case % 16 == 0 {
                let (start, end) = full_turns[(case / 16) % full_turns.len()];
                (start, end, TAU)
            } else {
                let start = rng.range(-4.0 * PI, 4.0 * PI);
                let sweep = rng.range(1e-3, TAU - 1e-3);
                (start, start - sweep, sweep)
            };
            let what = format!(
                "case {case}: C {center:?} major {major:?} ratio {ratio} \
                 {sweep_start} -> {sweep_end}"
            );
            let points = sampled(center, major, ratio, sweep_start, sweep_end);
            let spans = (sweep / FRAC_PI_4).ceil() as usize;
            assert_eq!(points.len(), spans * SEGMENTS_PER_SPAN + 1, "{what}");
            for &p in &points {
                let (u, v) = frame_of(p, center, major, ratio);
                assert!(
                    (u * u + v * v - 1.0).abs() <= 1e-9,
                    "{what}: {p:?} is off the ellipse"
                );
            }
            let first = ellipse_at(center, major, ratio, sweep_start);
            let last = ellipse_at(center, major, ratio, sweep_end);
            let near = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).hypot(a.1 - b.1) <= 1e-12;
            assert!(near(points[0], first), "{what}: starts at {:?}", points[0]);
            assert!(
                near(points[points.len() - 1], last),
                "{what}: ends at {:?}",
                points[points.len() - 1]
            );
            for pair in points.windows(2) {
                let (u0, v0) = frame_of(pair[0], center, major, ratio);
                let (u1, v1) = frame_of(pair[1], center, major, ratio);
                assert!(
                    u0 * v1 - v0 * u1 < 0.0,
                    "{what}: {:?} -> {:?} is not clockwise",
                    pair[0],
                    pair[1]
                );
            }
        }
    }

    #[test]
    fn a01s_left_head_is_half_an_ellipse_bulging_left() {
        // `/JSite121` oid 489: from 2π clockwise to π, the half through
        // `t = 3π/2`, which is `C − minor`.
        let center = (-0.010_296_148_41, 0.0889);
        let major = (4.768_329_956e-18, -0.025_958_436_58);
        let (poles, weights, knots) =
            elliptical_arc(center, major, 0.5, TAU, PI).expect("a half ellipse");
        assert_eq!(poles.len(), 9, "four 45° segments");
        assert_eq!(weights.len(), 9);
        assert_eq!(
            knots,
            [0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 4.0]
        );
        let points = sample(&poles, &weights, &knots, SEGMENTS_PER_SPAN);
        assert_eq!(points.len(), 33);
        let min_x = points.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
        assert!((min_x - -0.023_275_366_7).abs() < 1e-9, "apex x {min_x}");
        assert!(points.iter().all(|p| p.0 <= center.0 + 1e-12));
        // From the bottom end of the axis to the top one.
        assert!((points[0].1 - (0.0889 - 0.025_958_436_58)).abs() < 1e-12);
        assert!((points[32].1 - (0.0889 + 0.025_958_436_58)).abs() < 1e-12);
    }

    #[test]
    fn a_pair_a_whole_turn_apart_is_a_full_ellipse() {
        let (poles, weights, knots) =
            elliptical_arc((1.0, 2.0), (0.5, 0.0), 0.25, TAU, 0.0).expect("a full turn");
        assert_eq!(poles.len(), 17, "eight 45° segments");
        assert_eq!(weights.len(), 17);
        assert_eq!(knots.len(), 20);
        assert!((poles[0].0 - poles[16].0).abs() < 1e-12);
        assert!((poles[0].1 - poles[16].1).abs() < 1e-12);
    }

    #[test]
    fn a_degenerate_or_non_finite_arc_is_none() {
        let ok = ((0.0, 0.0), (1.0, 0.0), 0.5, PI, 0.0);
        assert!(elliptical_arc(ok.0, ok.1, ok.2, ok.3, ok.4).is_some());
        // The same angle twice sweeps nothing.
        assert!(elliptical_arc(ok.0, ok.1, ok.2, 1.0, 1.0).is_none());
        // No ellipse: a zero major semi-axis, a ratio that is not positive.
        assert!(elliptical_arc(ok.0, (0.0, 0.0), ok.2, ok.3, ok.4).is_none());
        assert!(elliptical_arc(ok.0, ok.1, 0.0, ok.3, ok.4).is_none());
        assert!(elliptical_arc(ok.0, ok.1, -0.5, ok.3, ok.4).is_none());
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(elliptical_arc((bad, 0.0), ok.1, ok.2, ok.3, ok.4).is_none());
            assert!(elliptical_arc((0.0, bad), ok.1, ok.2, ok.3, ok.4).is_none());
            assert!(elliptical_arc(ok.0, (bad, 0.0), ok.2, ok.3, ok.4).is_none());
            assert!(elliptical_arc(ok.0, (1.0, bad), ok.2, ok.3, ok.4).is_none());
            assert!(elliptical_arc(ok.0, ok.1, bad, ok.3, ok.4).is_none());
            assert!(elliptical_arc(ok.0, ok.1, ok.2, bad, ok.4).is_none());
            assert!(elliptical_arc(ok.0, ok.1, ok.2, ok.3, bad).is_none());
        }
        // Angles whose difference overflows, and poles that would.
        assert!(elliptical_arc(ok.0, ok.1, ok.2, f64::MAX, -f64::MAX).is_none());
        assert!(elliptical_arc(ok.0, (f64::MAX, f64::MAX), ok.2, ok.3, ok.4).is_none());
    }

    #[test]
    fn random_garbage_never_panics() {
        let specials = [
            0.0,
            -0.0,
            1.0,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::MAX,
            f64::MIN,
            f64::MIN_POSITIVE,
            f64::EPSILON,
            1e300,
            -1e300,
            TAU,
            PI,
        ];
        let mut rng = XorShift64::new(0xBAD_F00D_0930);
        let pick = |rng: &mut XorShift64| match rng.next_u64() % 3 {
            0 => specials[(rng.next_u64() % specials.len() as u64) as usize],
            1 => f64::from_bits(rng.next_u64()),
            _ => rng.range(-10.0, 10.0),
        };
        for _ in 0..20_000 {
            let center = (pick(&mut rng), pick(&mut rng));
            let major = (pick(&mut rng), pick(&mut rng));
            let ratio = pick(&mut rng);
            let (start, end) = (pick(&mut rng), pick(&mut rng));
            if let Some((poles, weights, knots)) = elliptical_arc(center, major, ratio, start, end)
            {
                assert_eq!(weights.len(), poles.len());
                assert_eq!(knots.len(), poles.len() + 3);
                assert!((3..=17).contains(&poles.len()));
                let _ = sample(&poles, &weights, &knots, SEGMENTS_PER_SPAN);
            }
        }
    }
}
