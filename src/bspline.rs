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
//!
//! Two things about that loop are not left to the record. The recursion
//! costs the square of the degree for every point, and the degree is the
//! difference of two counts, so it is bounded ([`MAX_DEGREE`]). And the
//! weights of a rational curve are a ratio, so they are brought to one scale
//! before they are used: a curve is the same curve whatever the size of its
//! weights.

use std::cmp::Ordering;

/// How many straight segments each knot span is drawn with, by every consumer
/// that draws the curve as a polyline (the sheet-level emitter here, the
/// symbol-body renderer downstream). The corpus's curve is a 2 mm lip of two
/// spans; eight a span keeps its chord error under a hundredth of a
/// millimetre.
pub const SEGMENTS_PER_SPAN: usize = 8;

/// The highest degree [`sample`] evaluates, and so the highest an
/// `igBspCurve2d` record may state
/// ([`crate::parsers::sheet_records::bspcurve_geometry`]).
///
/// A record has no field for its degree: it is the knot count less the pole
/// count less one, and de Boor's recursion pays `degree · (degree + 1) / 2`
/// blends for every point. Until 2026-11-12 the only bounds were on the two
/// counts — 4 096 poles, and 4 096 more knots than poles — which let a
/// record state any degree up to 4 095. 4 096 poles under 5 121 knots are
/// 106 KB and degree 1 024, and took minutes to sample: 358 s in the debug
/// build of the review that found it, 445 s unoptimized and 32 s optimized
/// on the machine this was written on.
///
/// Every `igBspCurve2d` record of the corpora is a cubic: 112 records on
/// the record chains of 885 documents — template sheets, symbol sheets and
/// the symbol caches of drawings — with 75 poles and 79 knots at the most
/// (`examples/probe_how_high_a_degree_a_bspline_record_states.rs`). The
/// bound is ten times the degree the corpora hold and a guard on the cost,
/// not a reading of the format. At degree 32 the largest record the decoder
/// takes (4 096 poles, 99 KB) is 32 513 points and 17 million blends, 0.6 s
/// unoptimized and 0.03 s optimized, so the work a curve asks for stays in
/// proportion to its bytes.
pub const MAX_DEGREE: usize = 32;

/// Points along the curve, from the first to the last usable knot.
///
/// The curve is walked span by span -- every interval between two distinct
/// knots inside the parameter domain -- with `per_span` segments each, so a
/// tight span gets as many segments as a wide one and the sampling follows
/// the knot vector's own idea of where the shape changes. The first point is
/// the curve at the domain's start and the last point the curve at its end;
/// for a clamped knot vector those are the first and last poles.
///
/// The weights of a rational curve are a ratio -- every multiple of them but
/// zero states the same curve -- so they are brought to one scale before they
/// are used: each is multiplied by the power of two that puts the largest in
/// `[1, 2)`. A power of two changes no bit of a weight it leaves in the
/// normal range, so a curve whose weights are ordinary is sampled to the
/// points it always was; and weights that are all near the smallest number,
/// or near the largest, draw the curve they state, where the first used to
/// come out as points at the origin and the second as points that are not
/// numbers.
///
/// Returns the poles themselves when the record cannot be drawn as a curve:
/// fewer than two poles; a knot vector too short for degree one, or long
/// enough for a degree past [`MAX_DEGREE`]; a weight list that is neither
/// empty nor one per pole; a weight that is not finite, or weights that are
/// all zero; a point whose weight is not a normal number -- the weights that
/// bear on it cancel, or are more than 2^1022 times lighter than the
/// heaviest of the curve, and there is nothing to divide by; a point that is
/// not finite. A consumer then draws the control polygon, which is wrong in
/// a visible way rather than a silent way. Every point of a curve that does
/// come back is finite.
pub fn sample(
    poles: &[(f64, f64)],
    weights: &[f64],
    knots: &[f64],
    per_span: usize,
) -> Vec<(f64, f64)> {
    curve(poles, weights, knots, per_span).unwrap_or_else(|| poles.to_vec())
}

/// [`sample`]'s curve, or `None` where [`sample`] hands back the poles: for
/// a caller that takes a curve or nothing and has to tell the two apart. A
/// boundary's member is the edge of a fill, and a control polygon is not the
/// curve it re-lists
/// (`parsers::sheet_records`, the `0x7F` and `0x74` members of an
/// `igBoundary2d`).
pub(crate) fn try_sample(
    poles: &[(f64, f64)],
    weights: &[f64],
    knots: &[f64],
    per_span: usize,
) -> Option<Vec<(f64, f64)>> {
    curve(poles, weights, knots, per_span)
}

/// [`sample`]'s curve, or `None` where it hands back the poles.
fn curve(
    poles: &[(f64, f64)],
    weights: &[f64],
    knots: &[f64],
    per_span: usize,
) -> Option<Vec<(f64, f64)>> {
    let n = poles.len();
    if n < 2 || knots.len() < n + 2 || !(weights.is_empty() || weights.len() == n) {
        return None;
    }
    let degree = knots.len() - n - 1;
    if degree > MAX_DEGREE {
        return None;
    }
    let per_span = per_span.max(1);
    // The parameter domain of a B-spline with this many knots.
    let start = knots[degree];
    let end = knots[n];
    // `partial_cmp` rather than `<=`, so a NaN knot ends here too.
    if end.partial_cmp(&start) != Some(Ordering::Greater) {
        return None;
    }
    let (first, second) = weight_scale(weights)?;
    let homogeneous: Vec<[f64; 3]> = poles
        .iter()
        .enumerate()
        .map(|(index, (x, y))| {
            let w = weights
                .get(index)
                .map_or(1.0, |weight| weight * first * second);
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
            out.push(de_boor(&homogeneous, knots, degree, span, u)?);
        }
    }
    // The end of the domain, evaluated on the last non-empty span so the
    // clamped end pole comes out exactly.
    if let Some(last_span) = (degree..n)
        .rev()
        .find(|&span| knots[span + 1] > knots[span])
    {
        out.push(de_boor(&homogeneous, knots, degree, last_span, end)?);
    }
    Some(out)
}

/// The two powers of two that, multiplied in turn, bring the largest of
/// `weights` into `[1, 2)`: `(1, 1)` when there are none, the curve being
/// polynomial, and whenever the largest is in `[1, 2)` already, as it is on
/// every curve [`elliptical_arc`] makes. `None` when a weight is not finite
/// or none is other than zero.
///
/// Two factors because the scale runs from 2^-1023, for a weight near the
/// largest number, to 2^1074, for the smallest there is, and the far end of
/// that is no `f64`; each half of it is. Scaling up is exact. Scaling down is
/// exact for every weight it leaves in the normal range; one it leaves below
/// that is more than 2^1022 times lighter than the heaviest, and has no pull
/// on a point a heavier pole bears on.
fn weight_scale(weights: &[f64]) -> Option<(f64, f64)> {
    let mut largest = 0.0_f64;
    for weight in weights {
        if !weight.is_finite() {
            return None;
        }
        largest = largest.max(weight.abs());
    }
    if weights.is_empty() {
        return Some((1.0, 1.0));
    }
    if largest == 0.0 {
        return None;
    }
    let up = -binary_exponent(largest);
    let first = up / 2;
    Some((power_of_two(first), power_of_two(up - first)))
}

/// `floor(log2(value))` of a finite `value` above zero, read off its bits:
/// from -1074, the smallest subnormal, to 1023.
fn binary_exponent(value: f64) -> i32 {
    let bits = value.to_bits();
    let biased = ((bits >> 52) & 0x7FF) as i32;
    if biased == 0 {
        // Subnormal: the place of the highest bit set in the fraction.
        let fraction = bits & ((1_u64 << 52) - 1);
        (63 - fraction.leading_zeros() as i32) - 1074
    } else {
        biased - 1023
    }
}

/// `2^power`, for a `power` from -1022 to 1023: the bits of a normal number
/// whose fraction is zero.
fn power_of_two(power: i32) -> f64 {
    f64::from_bits(((power + 1023) as u64) << 52)
}

/// The curve at `u`, which lies in `[knots[span], knots[span + 1])`, by the
/// triangular de Boor recursion over the `degree + 1` poles that span uses.
///
/// `None` when the point's weight is not a normal number -- zero, or with
/// fewer bits than a coordinate has -- or the point is not finite.
fn de_boor(
    poles: &[[f64; 3]],
    knots: &[f64],
    degree: usize,
    span: usize,
    u: f64,
) -> Option<(f64, f64)> {
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
    // Until 2026-11-12 a weight under `f64::EPSILON` was taken for no weight
    // and the point left undivided, which put every point of a curve under
    // weights that small within a hair of the origin. A weight is a ratio
    // and has no such scale: the point is divided by any weight that still
    // has all its bits.
    if !w.is_normal() {
        return None;
    }
    let point = (x / w, y / w);
    (point.0.is_finite() && point.1.is_finite()).then_some(point)
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
/// every other weight is 1. The count allows `1e-9` of a segment, so a
/// sweep a rounding error past a multiple of 45° takes that many segments:
/// A01 stores π one ulp short of `std::f64::consts::PI`, which puts its left
/// head's sweep from 2π down to π a hair over 180°. The knots are
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
    elliptical_arc_directed(center, major, ratio, sweep_start, sweep_end, true)
}

/// [`elliptical_arc`], with the traversal direction stated instead of assumed.
///
/// An `igEllipticalArc2d` record names two parameters and no direction: the
/// half turned between them depends on whether the parameter runs up or down.
/// The record's last byte says which — `0` is the clockwise traverse
/// [`elliptical_arc`] makes, `1` is the counter-clockwise one, under which
/// `sweep_start + n·Δ` replaces `sweep_start − n·Δ` and the sweep is
/// `(end − start) mod 2π`.
///
/// Both readings are measured, not assumed:
///
/// * A01's four vessel heads (`/JSite121` oids 489/490, `/JSite39` 114/115)
///   all carry `0`, and the clockwise reading puts each apex on the far side
///   of the rectangle that closes the head — the outward bulge
///   `tests/elliptical_arcs.rs` pins.
/// * The diaphragm actuator's dome (every one of the corpus's five drawings
///   caches exactly this one record) carries `1`, and only the
///   counter-clockwise reading puts its apex at `y = 11.43 mm` under a chord
///   at `8.89 mm` — which is where the reference plot draws it
///   (`docs/analysis/2026-10-20-the-actuator-dome-is-on-the-wrong-side.md`).
///
/// `None` on every input [`elliptical_arc`] rejects.
#[allow(clippy::type_complexity)]
pub fn elliptical_arc_directed(
    center: (f64, f64),
    major: (f64, f64),
    ratio: f64,
    sweep_start: f64,
    sweep_end: f64,
    clockwise: bool,
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
    let mut sweep = if clockwise {
        (sweep_start - sweep_end).rem_euclid(TAU)
    } else {
        (sweep_end - sweep_start).rem_euclid(TAU)
    };
    if !sweep.is_finite() {
        return None;
    }
    if sweep == 0.0 {
        sweep = TAU;
    }
    // `sweep` is in (0, 2π] here, so this is 1..=8; the clamp only keeps a
    // rounding slip from ever reaching the cast. The `1e-9` lets a sweep a
    // rounding error past a multiple of 45° take that many segments.
    let segments = (sweep / FRAC_PI_4 - 1e-9).ceil().clamp(1.0, 8.0) as usize;
    let step = sweep / segments as f64;
    // At most π/8 plus 1e-9 of it, so about cos(π/8) ≈ 0.92 at least: never
    // a zero divisor.
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
    let sign = if clockwise { -1.0 } else { 1.0 };
    for k in 0..segments {
        let k = k as f64;
        poles.push(point(sweep_start + sign * k * step, 1.0));
        weights.push(1.0);
        poles.push(point(sweep_start + sign * (k + 0.5) * step, half_cos));
        weights.push(half_cos);
    }
    poles.push(point(sweep_start + sign * segments as f64 * step, 1.0));
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

    /// `count` poles that zigzag a millimetre across a line a millimetre a
    /// pole long, in metres like a sheet's.
    fn zigzag(count: usize) -> Vec<(f64, f64)> {
        (0..count)
            .map(|index| (index as f64 * 0.001, (index % 2) as f64 * 0.001))
            .collect()
    }

    /// The clamped knot vector of `poles` poles at `degree`: `degree + 1`
    /// zeros, the whole numbers between, and `degree + 1` times the last —
    /// `poles + degree + 1` knots over `poles − degree` spans.
    fn clamped_knots(poles: usize, degree: usize) -> Vec<f64> {
        let spans = poles - degree;
        let mut knots = vec![0.0; degree + 1];
        knots.extend((1..spans).map(|span| span as f64));
        knots.extend(std::iter::repeat_n(spans as f64, degree + 1));
        knots
    }

    /// `2^power`, built from its bits: exact down to the smallest subnormal.
    fn two_to(power: i32) -> f64 {
        assert!((-1074..=1023).contains(&power));
        if power < -1022 {
            f64::from_bits(1u64 << (power + 1074))
        } else {
            f64::from_bits(((power + 1023) as u64) << 52)
        }
    }

    /// How long a curve may take, in a debug build on a machine that is busy
    /// with other things: the bound a case of
    /// `tests/arbitrary_bytes_never_panic.rs` is held to.
    const TIME_LIMIT: std::time::Duration = std::time::Duration::from_secs(10);

    /// `work`'s answer and how long it took, if it comes within
    /// [`TIME_LIMIT`]. The work runs on a thread of its own, which is left
    /// running when it does not.
    fn in_time<T: Send + 'static>(
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Option<(T, std::time::Duration)> {
        let (answer, wait) = std::sync::mpsc::channel();
        let started = std::time::Instant::now();
        std::thread::spawn(move || {
            let _ = answer.send(work());
        });
        let answer = wait.recv_timeout(TIME_LIMIT).ok()?;
        Some((answer, started.elapsed()))
    }

    /// The recursion costs the square of the degree for every point, so the
    /// degree is bounded: a curve of [`MAX_DEGREE`] is evaluated and one of a
    /// degree higher is handed back as its poles, like every other record
    /// that cannot be drawn as a curve.
    #[test]
    fn a_curve_is_evaluated_up_to_the_highest_degree_and_no_further() {
        let poles = zigzag(MAX_DEGREE + 9);
        let highest = sample(
            &poles,
            &[],
            &clamped_knots(poles.len(), MAX_DEGREE),
            SEGMENTS_PER_SPAN,
        );
        assert_eq!(
            highest.len(),
            SEGMENTS_PER_SPAN * (poles.len() - MAX_DEGREE) + 1,
            "nine spans of eight segments, plus the end"
        );
        assert!(close(highest[0], poles[0]));
        assert!(close(highest[highest.len() - 1], poles[poles.len() - 1]));
        // Inside the hull of the poles, as a curve is, and not on the
        // zigzag: at this degree the curve runs near the middle of it.
        for (x, y) in &highest {
            assert!(
                (-1e-12..=0.040 + 1e-12).contains(x) && (-1e-12..=0.001 + 1e-12).contains(y),
                "({x}, {y}) left the hull"
            );
        }
        let middle = highest[highest.len() / 2];
        assert!((middle.1 - 0.0005).abs() < 0.0001, "{middle:?}");

        let past = sample(
            &poles,
            &[],
            &clamped_knots(poles.len(), MAX_DEGREE + 1),
            SEGMENTS_PER_SPAN,
        );
        assert_eq!(past, poles, "one degree past the highest: the poles");
    }

    /// What the bound is for. A record of 4 096 poles at degree 1 024 took
    /// 358 seconds to sample in a debug build before there was one, and the
    /// most work that many poles could ask for was at degree 2 730 (the
    /// spans times the square of the degree is greatest at two thirds of the
    /// pole count). Both come back as their poles; the most a decoder hands
    /// over that is evaluated — the same poles at [`MAX_DEGREE`] — comes
    /// back as a curve; and each does within [`TIME_LIMIT`].
    #[test]
    fn a_curve_of_any_degree_comes_back_in_bounded_time() {
        const POLES: usize = 4096;
        for degree in [1024, 2730, MAX_DEGREE + 1] {
            assert_eq!(
                timed(POLES, degree),
                zigzag(POLES),
                "degree {degree}: the poles"
            );
        }
        assert_eq!(
            timed(POLES, MAX_DEGREE).len(),
            SEGMENTS_PER_SPAN * (POLES - MAX_DEGREE) + 1
        );
        // What the largest record of the corpora asks for.
        assert_eq!(timed(75, 3).len(), SEGMENTS_PER_SPAN * (75 - 3) + 1);
    }

    /// The points [`sample`] gives for `poles` poles on a zigzag at `degree`,
    /// which have to come within [`TIME_LIMIT`]; how long they took is
    /// printed.
    fn timed(poles: usize, degree: usize) -> Vec<(f64, f64)> {
        let (points, took) = in_time(move || {
            sample(
                &zigzag(poles),
                &[],
                &clamped_knots(poles, degree),
                SEGMENTS_PER_SPAN,
            )
        })
        .unwrap_or_else(|| {
            panic!(
                "{poles} poles at degree {degree} did not come back in {} s",
                TIME_LIMIT.as_secs()
            )
        });
        println!(
            "{poles} poles at degree {degree}: {} points in {took:.3?}",
            points.len()
        );
        points
    }

    /// A rational curve is the same curve when every weight is multiplied by
    /// one positive number, and it is sampled to the same bits when that
    /// number is a power of two — from weights so small that the smallest
    /// is the smallest number there is, to ones so large that a pole times
    /// its weight is past the largest.
    #[test]
    fn a_rational_curve_is_the_same_whatever_the_scale_of_its_weights() {
        let poles = [(0.0, 0.0), (2.0, 4.0), (4.0, 0.0), (6.0, 4.0)];
        let weights = [1.0, 0.5, 0.25, 1.0];
        let knots = [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let plain = sample(&poles, &weights, &knots, SEGMENTS_PER_SPAN);
        assert_eq!(plain.len(), SEGMENTS_PER_SPAN + 1);
        assert!(close(plain[0], poles[0]) && close(plain[SEGMENTS_PER_SPAN], poles[3]));
        // The light middle poles pull less than they would unweighted.
        let unweighted = sample(&poles, &[], &knots, SEGMENTS_PER_SPAN);
        assert!(plain[SEGMENTS_PER_SPAN / 2].1 != unweighted[SEGMENTS_PER_SPAN / 2].1);

        for power in [
            -1072, -1023, -1022, -537, -53, -52, -1, 1, 52, 537, 1021, 1023,
        ] {
            let scaled: Vec<f64> = weights
                .iter()
                .map(|weight| weight * two_to(power))
                .collect();
            // The scaling itself is exact: each is the weight's own bits
            // under another exponent.
            assert_eq!(scaled[2] / scaled[0], 0.25, "2^{power}");
            assert_eq!(
                sample(&poles, &scaled, &knots, SEGMENTS_PER_SPAN),
                plain,
                "the weights times 2^{power}"
            );
        }
    }

    /// Weights that are all alike state the unweighted curve, however small
    /// or large the one number is. Until 2026-11-12 a weight under
    /// `f64::EPSILON` was taken for no weight at the end of the recursion and
    /// the point left undivided — every point of such a curve within a hair
    /// of the origin — and a weight near the largest number sent a pole
    /// times its weight past it, and the points with it.
    #[test]
    fn weights_that_are_all_alike_draw_the_unweighted_curve() {
        let corners = [(-1e9, -1e9), (1e9, -1e9), (1e9, 1e9), (-1e9, 1e9)];
        let knots = [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let unweighted = sample(&corners, &[], &knots, SEGMENTS_PER_SPAN);
        assert!(unweighted
            .iter()
            .all(|(x, y)| x.abs() <= 1e9 && y.abs() <= 1e9));

        // A power of two brings back the very bits.
        for alike in [5e-324, f64::MIN_POSITIVE, 1.0 / 1024.0, 1.0, 2.0] {
            assert_eq!(
                sample(&corners, &[alike; 4], &knots, SEGMENTS_PER_SPAN),
                unweighted,
                "weights of {alike:e}"
            );
        }
        // Any other number brings back the curve to the last few bits.
        for alike in [1e-300, 1e-20, f64::EPSILON / 2.0, 0.1, 3.0, 1e20, 1e308] {
            let points = sample(&corners, &[alike; 4], &knots, SEGMENTS_PER_SPAN);
            assert_eq!(points.len(), unweighted.len(), "weights of {alike:e}");
            for (point, plain) in points.iter().zip(&unweighted) {
                assert!(
                    (point.0 - plain.0).abs() <= 1e-3 && (point.1 - plain.1).abs() <= 1e-3,
                    "weights of {alike:e}: {point:?} against {plain:?}"
                );
            }
        }
    }

    /// A weight list that states no curve gives the poles, like a knot
    /// vector that states none: a weight that is not a number, weights that
    /// are all zero, and weights under which some point of the curve has no
    /// weight to be divided by. None of them gives a point that is not
    /// finite, and none a point at the origin that no pole is near.
    #[test]
    fn a_weight_list_that_states_no_curve_gives_the_poles() {
        let poles = [(1.0, 1.0), (2.0, 3.0), (3.0, 1.0), (4.0, 3.0)];
        let knots = [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        for (what, weights) in [
            ("a weight that is not a number", [1.0, f64::NAN, 1.0, 1.0]),
            ("an infinite weight", [1.0, 1.0, f64::INFINITY, 1.0]),
            (
                "an infinite weight below zero",
                [f64::NEG_INFINITY, 1.0, 1.0, 1.0],
            ),
            ("weights that are all zero", [0.0; 4]),
        ] {
            assert_eq!(
                sample(&poles, &weights, &knots, SEGMENTS_PER_SPAN),
                poles.to_vec(),
                "{what}"
            );
        }

        // A span none of whose four poles has a weight: the point there is
        // nothing over nothing.
        let poles = zigzag(12);
        let mut weights = [1.0; 12];
        weights[4..8].fill(0.0);
        assert_eq!(
            sample(&poles, &weights, &clamped_knots(12, 3), SEGMENTS_PER_SPAN),
            poles,
            "a span whose weights are all zero"
        );
        // A weight under zero that brings the divisor to zero at the middle
        // of the curve, where two segments to the span take a point.
        let poles = [(0.0, 0.0), (2.0, 4.0), (4.0, 0.0)];
        assert_eq!(
            sample(
                &poles,
                &[1.0, -1.0, 1.0],
                &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                2
            ),
            poles.to_vec(),
            "a divisor of zero at the middle"
        );
        // A weight of zero that leaves every point a weight is a weight: the
        // pole it is on has no pull, and the curve is the line between the
        // other two.
        let straight = sample(&poles, &[1.0, 0.0, 1.0], &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0], 4);
        assert_eq!(straight.len(), 5);
        assert!(straight.iter().all(|(_, y)| *y == 0.0), "{straight:?}");
    }

    /// A point is divided by its weight when that is a normal number, and
    /// by nothing else. Twelve poles of which the last eight are 2^1000
    /// times lighter than the first four: the spans those eight carry alone
    /// are the unweighted curve's to the bit — their weights are alike, and
    /// 2^-1000 is still a number with all its bits — and where a heavy pole
    /// bears, it has all of the pull. Make the eight 2^1023 times lighter and
    /// their spans would be divided by a number with fewer bits than a
    /// coordinate has: the curve is given up as its poles.
    #[test]
    fn poles_far_lighter_than_the_rest_draw_the_spans_they_carry_alone() {
        let poles = zigzag(12);
        let knots = clamped_knots(12, 3);
        let unweighted = sample(&poles, &[], &knots, SEGMENTS_PER_SPAN);
        assert_eq!(unweighted.len(), 9 * SEGMENTS_PER_SPAN + 1);

        let mut weights = [1.0; 12];
        weights[4..].fill(two_to(-1000));
        let curve = sample(&poles, &weights, &knots, SEGMENTS_PER_SPAN);
        assert_eq!(curve.len(), unweighted.len());
        // The first span is poles 0 to 3, which weigh one; spans five to
        // nine are poles 4 to 11.
        assert_eq!(curve[..SEGMENTS_PER_SPAN], unweighted[..SEGMENTS_PER_SPAN]);
        assert_eq!(
            curve[4 * SEGMENTS_PER_SPAN..],
            unweighted[4 * SEGMENTS_PER_SPAN..]
        );
        // The last point before them is still pole 3's: one heavy pole among
        // three light ones.
        assert!(close(curve[4 * SEGMENTS_PER_SPAN - 1], poles[3]));

        for power in [-1012, -1022] {
            weights[4..].fill(two_to(power));
            let curve = sample(&poles, &weights, &knots, SEGMENTS_PER_SPAN);
            assert_eq!(curve.len(), unweighted.len(), "2^{power}");
            for (point, plain) in curve[4 * SEGMENTS_PER_SPAN..]
                .iter()
                .zip(&unweighted[4 * SEGMENTS_PER_SPAN..])
            {
                assert!(
                    close(*point, *plain),
                    "2^{power}: {point:?} against {plain:?}"
                );
            }
        }
        for power in [-1023, -1040, -1074] {
            weights[4..].fill(two_to(power));
            assert_eq!(
                sample(&poles, &weights, &knots, SEGMENTS_PER_SPAN),
                poles,
                "2^{power}"
            );
        }
    }

    /// The evaluation as it stood until 2026-11-12: no bound on the degree,
    /// the weights used as they come, and a point left undivided when its
    /// weight is under `f64::EPSILON`. Kept for the one test that holds
    /// [`sample`] to its bits on the curves both evaluate.
    fn sample_as_it_was(
        poles: &[(f64, f64)],
        weights: &[f64],
        knots: &[f64],
        per_span: usize,
    ) -> Vec<(f64, f64)> {
        fn de_boor(
            poles: &[[f64; 3]],
            knots: &[f64],
            degree: usize,
            span: usize,
            u: f64,
        ) -> (f64, f64) {
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

        let n = poles.len();
        if n < 2 || knots.len() < n + 2 || !(weights.is_empty() || weights.len() == n) {
            return poles.to_vec();
        }
        let degree = knots.len() - n - 1;
        let per_span = per_span.max(1);
        let start = knots[degree];
        let end = knots[n];
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
        if let Some(last_span) = (degree..n)
            .rev()
            .find(|&span| knots[span + 1] > knots[span])
        {
            out.push(de_boor(&homogeneous, knots, degree, last_span, end));
        }
        out
    }

    /// The bits of `points`: `==` on the numbers would take `-0.0` for `0.0`.
    fn bits(points: &[(f64, f64)]) -> Vec<(u64, u64)> {
        points
            .iter()
            .map(|(x, y)| (x.to_bits(), y.to_bits()))
            .collect()
    }

    /// Bringing the weights to one scale changes no bit of a curve the old
    /// evaluation drew: the scale is a power of two, and a power of two
    /// times a number in the normal range is that number under another
    /// exponent. Twenty thousand curves of degree 1 to 7 — a quarter of them
    /// polynomial, the rest under weights from within a factor of twenty of
    /// one another to eighteen orders of magnitude apart, with a knot
    /// repeated here and there — and four thousand elliptical arcs, whose
    /// weights are 1 and the cosine of a half segment, sample to the very
    /// points they did.
    #[test]
    fn the_scale_changes_no_bit_of_a_curve_with_ordinary_weights() {
        let mut rng = XorShift64::new(0x0068_5CA1_E000);
        let (mut rational, mut points) = (0, 0);
        for curve in 0..20_000 {
            let degree = 1 + (rng.next_u64() % 7) as usize;
            let n = degree + 1 + (rng.next_u64() % 12) as usize;
            let poles: Vec<(f64, f64)> = (0..n)
                .map(|_| (rng.range(-2.0, 2.0), rng.range(-2.0, 2.0)))
                .collect();
            let weights: Vec<f64> = match rng.next_u64() % 4 {
                0 => Vec::new(),
                1 => (0..n).map(|_| rng.range(0.05, 1.0)).collect(),
                2 => (0..n).map(|_| rng.range(0.5, 40.0)).collect(),
                _ => (0..n).map(|_| rng.range(1e-9, 1e9)).collect(),
            };
            let mut knots = vec![0.0; degree + 1];
            let mut at = 0.0;
            for _ in 0..n - degree - 1 {
                if !rng.next_u64().is_multiple_of(5) {
                    at += rng.range(0.01, 1.0);
                }
                knots.push(at);
            }
            at += rng.range(0.01, 1.0);
            knots.extend(std::iter::repeat_n(at, degree + 1));

            let now = sample(&poles, &weights, &knots, SEGMENTS_PER_SPAN);
            assert_eq!(
                bits(&now),
                bits(&sample_as_it_was(
                    &poles,
                    &weights,
                    &knots,
                    SEGMENTS_PER_SPAN
                )),
                "curve {curve}: degree {degree}, weights {weights:?}"
            );
            rational += usize::from(!weights.is_empty());
            points += now.len();
        }
        // Curves, not control polygons: nine points to a span at the least.
        assert!(
            rational > 14_000 && points > 800_000,
            "{rational}, {points}"
        );

        let mut arcs = 0;
        while arcs < 4_000 {
            let center = (rng.range(-1.0, 1.0), rng.range(-1.0, 1.0));
            let major = (rng.range(-0.1, 0.1), rng.range(-0.1, 0.1));
            let ratio = rng.range(0.05, 3.0);
            let (start, end) = (rng.range(-7.0, 7.0), rng.range(-7.0, 7.0));
            let clockwise = rng.next_u64().is_multiple_of(2);
            let Some((poles, weights, knots)) =
                elliptical_arc_directed(center, major, ratio, start, end, clockwise)
            else {
                continue;
            };
            assert_eq!(
                bits(&sample(&poles, &weights, &knots, SEGMENTS_PER_SPAN)),
                bits(&sample_as_it_was(
                    &poles,
                    &weights,
                    &knots,
                    SEGMENTS_PER_SPAN
                )),
                "arc {arcs}: {center:?} {major:?} {ratio} {start} {end} {clockwise}"
            );
            arcs += 1;
        }
    }

    use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

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
    fn the_direction_flag_picks_the_half_between_two_ends() {
        // The diaphragm actuator's dome: chord at y = 8.89 mm, semi-axes
        // 3.175 / 2.54, stored π -> 2π with flag 1. The reference plot's
        // apex is the counter-clockwise half's, 2.54 mm above the chord.
        let center = (0.0, 0.008_89);
        let major = (-0.003_175, 0.0);
        let ys = |clockwise: bool| {
            let (poles, weights, knots) =
                elliptical_arc_directed(center, major, 0.8, PI, TAU, clockwise)
                    .expect("a half ellipse");
            let points = sample(&poles, &weights, &knots, SEGMENTS_PER_SPAN);
            (
                points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min),
                points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max),
            )
        };
        // Clockwise walks through π/2: the apex is the chord minus the minor
        // semi-axis.
        let (low, high) = ys(true);
        assert!((low - 0.006_35).abs() < 1e-9, "clockwise low {low}");
        assert!((high - center.1).abs() < 1e-9, "clockwise high {high}");
        // Counter-clockwise walks through 3π/2: the apex is the chord plus it.
        let (low, high) = ys(false);
        assert!((low - center.1).abs() < 1e-9, "counter-clockwise low {low}");
        assert!(
            (high - 0.011_43).abs() < 1e-9,
            "counter-clockwise high {high}"
        );
        // Both readings start and end on the same two ends.
        for clockwise in [true, false] {
            let (poles, _, _) = elliptical_arc_directed(center, major, 0.8, PI, TAU, clockwise)
                .expect("a half ellipse");
            let last = *poles.last().expect("poles");
            assert!((poles[0].1 - center.1).abs() < 1e-12);
            assert!((last.1 - center.1).abs() < 1e-12);
            assert!((poles[0].0.abs() - 0.003_175).abs() < 1e-12);
            assert!((last.0.abs() - 0.003_175).abs() < 1e-12);
        }
    }

    #[test]
    fn a_sweep_a_rounding_error_past_a_multiple_of_45_degrees_takes_that_many_segments() {
        // A01 stores π one ulp short of `PI`, so its 2π → π sweeps a hair
        // over 180° and its π → 0 a hair under: four segments each.
        let pi_stored = f64::from_bits(0x4009_21FB_5444_2D17);
        assert_eq!(pi_stored.to_bits() + 1, PI.to_bits());
        let lengths = |sweep_start: f64, sweep_end: f64| {
            let (poles, weights, knots) =
                elliptical_arc((0.0, 0.0), (0.0, -1.0), 0.5, sweep_start, sweep_end)
                    .expect("an elliptical arc");
            (poles.len(), weights.len(), knots.len())
        };
        assert_eq!(lengths(TAU, pi_stored), (9, 9, 12), "2π down to A01's π");
        assert_eq!(lengths(pi_stored, 0.0), (9, 9, 12), "A01's π down to 0");
        assert_eq!(lengths(FRAC_PI_2 + 1e-15, 0.0), (5, 5, 8), "90° and 1e-15");
        // The slack is tiny: a millionth of a segment past 45° is a second
        // segment.
        assert_eq!(
            lengths(FRAC_PI_4 * (1.0 + 1e-6), 0.0),
            (5, 5, 8),
            "45° and a millionth"
        );
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
