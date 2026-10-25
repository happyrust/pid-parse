//! Is a leftover gap on an arc-shaped body the decoder's, the print's, or the
//! reference read's own flattening?
//!
//! An arc we draw rarely lands exactly on the reference plot's ink, and two
//! very different causes look the same in a residual: the decoded arc can be
//! slightly wrong, or the reference can be a chord of the right arc. This probe
//! separates them by where the gap sits.
//!
//! A flattening — the print driver's, or [`pid_plot::read::CHORD_MM`] on a path
//! that is still a curve — cuts a chord's interior away from the true curve but
//! leaves its *vertices* on it. So a gap measured at a vertex cannot be
//! flattening, and a gap measured mid-chord of the right curvature is nothing
//! else. The probe prints, for one paper-millimetre window:
//!
//! - the reference page's own coordinate lattice, which fixes the measurement
//!   floor: every vertex of the print sits on a 1/600-inch grid, so a vertex
//!   position carries at most ±0.021 mm and a radius fitted through five of
//!   them about ±0.01 mm;
//! - every arc of the page in it, with the chord length the reader's own
//!   tolerance would cut that radius into (`sqrt(8 r CHORD_MM)`);
//! - every reference chord, its worst distance to those arcs and where on the
//!   chord it lands (`t 0.00` / `t 1.00` is a vertex);
//! - every reference vertex's radial error about each arc;
//! - the circle each printed lobe actually lies on (geometric fit, so a
//!   scattered lobe does not masquerade as a different radius);
//! - the same wave as a tangent-continuous pair of arcs with both radii free,
//!   which is the family a two-arc symbol body belongs to: when even that
//!   bottoms out above the lattice floor, no pair of circular arcs is what
//!   printed.
//!
//! ```text
//! cargo run -q -p pid-plot --example probe_arc_against_reference -- \
//!     <drawing.pid> <reference.pdf> <x0> <y0> <x1> <y1>
//! ```
//!
//! Read-only.

use std::path::Path;

use pid_plot::model::{PaperShape, lay_out_drawing};
use pid_plot::read::{CHORD_MM, read_plot};

const TAU: f64 = std::f64::consts::TAU;
/// Sampling pitch along a shape, in millimetres: a tenth of the reader's own
/// flattening tolerance, so a distance is measured to the curve rather than to
/// its samples.
const PITCH_MM: f64 = 0.001;
/// The device lattice the reference plot's coordinates land on: 1/600 inch.
const LATTICE_MM: f64 = 25.4 / 600.0;

/// One arc of the page: its centre and radius, and the points it walks.
type Arc = (([f64; 2], f64), Vec<[f64; 2]>);

fn arc_points(shape: &PaperShape) -> Option<Vec<[f64; 2]>> {
    let PaperShape::Arc {
        center_mm,
        radius_mm,
        start_angle,
        end_angle,
        clockwise,
    } = shape
    else {
        return None;
    };
    let sweep = if *clockwise {
        -(*start_angle - *end_angle).rem_euclid(TAU)
    } else {
        (*end_angle - *start_angle).rem_euclid(TAU)
    };
    let steps = ((sweep.abs() * radius_mm / PITCH_MM).ceil() as usize).clamp(8, 200_000);
    Some(
        (0..=steps)
            .map(|step| {
                let angle = start_angle + sweep * (step as f64 / steps as f64);
                [
                    center_mm[0] + radius_mm * angle.cos(),
                    center_mm[1] + radius_mm * angle.sin(),
                ]
            })
            .collect(),
    )
}

fn distance(point: [f64; 2], chain: &[[f64; 2]]) -> f64 {
    chain
        .windows(2)
        .map(|pair| {
            let (a, b) = (pair[0], pair[1]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let length = dx * dx + dy * dy;
            if length <= 1e-15 {
                return (point[0] - a[0]).hypot(point[1] - a[1]);
            }
            let t = (((point[0] - a[0]) * dx + (point[1] - a[1]) * dy) / length).clamp(0.0, 1.0);
            (point[0] - (a[0] + t * dx)).hypot(point[1] - (a[1] + t * dy))
        })
        .fold(f64::MAX, f64::min)
}

/// Gaussian elimination with partial pivoting, for the small fits below.
fn solve<const N: usize>(mut matrix: [[f64; N]; N], mut rhs: [f64; N]) -> Option<[f64; N]> {
    for column in 0..N {
        let pivot = (column..N).max_by(|left, right| {
            matrix[*left][column]
                .abs()
                .total_cmp(&matrix[*right][column].abs())
        })?;
        if matrix[pivot][column].abs() < 1e-14 {
            return None;
        }
        if pivot != column {
            matrix.swap(pivot, column);
            rhs.swap(pivot, column);
        }
        let pivot_row = matrix[column];
        for row in (column + 1)..N {
            let factor = matrix[row][column] / pivot_row[column];
            for (target, source) in matrix[row][column..N].iter_mut().zip(&pivot_row[column..N]) {
                *target -= factor * *source;
            }
            rhs[row] -= factor * rhs[column];
        }
    }
    let mut out = [0.0; N];
    for row in (0..N).rev() {
        let mut sum = rhs[row];
        for column in (row + 1)..N {
            sum -= matrix[row][column] * out[column];
        }
        out[row] = sum / matrix[row][row];
    }
    Some(out)
}

/// Least-squares (Kåsa) circle fit — `x² + y² + D x + E y + F = 0` — used only
/// to seed [`fit_circle`].
fn kasa_circle(points: &[[f64; 2]]) -> ([f64; 2], f64) {
    let n = points.len() as f64;
    let (mut sx, mut sy, mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
    let (mut sxz, mut syz, mut sz) = (0.0, 0.0, 0.0);
    for point in points {
        let z = point[0] * point[0] + point[1] * point[1];
        sx += point[0];
        sy += point[1];
        sxx += point[0] * point[0];
        syy += point[1] * point[1];
        sxy += point[0] * point[1];
        sxz += point[0] * z;
        syz += point[1] * z;
        sz += z;
    }
    let a = [[sxx, sxy, sx], [sxy, syy, sy], [sx, sy, n]];
    let (dd, ee, ff) = match solve(a, [-sxz, -syz, -sz]) {
        Some(solution) => (solution[0], solution[1], solution[2]),
        None => return ([0.0, 0.0], 1.0),
    };
    let center = [-dd / 2.0, -ee / 2.0];
    (
        center,
        (center[0] * center[0] + center[1] * center[1] - ff).sqrt(),
    )
}

/// Geometric circle fit: Gauss–Newton on `|p - c| - r`, seeded by Kåsa.
///
/// The algebraic fit alone is biased when a lobe's vertices scatter — on the
/// title block's wave it read a second radius 37 µm below the geometric one,
/// which looked like a shape difference that was not there.
fn fit_circle(points: &[[f64; 2]]) -> ([f64; 2], f64) {
    let (mut center, mut radius) = kasa_circle(points);
    for _ in 0..60 {
        let mut normal = [[0.0; 3]; 3];
        let mut rhs = [0.0; 3];
        for point in points {
            let (dx, dy) = (point[0] - center[0], point[1] - center[1]);
            let length = (dx * dx + dy * dy).sqrt();
            if length < 1e-12 {
                continue;
            }
            let residual = length - radius;
            let jacobian = [-dx / length, -dy / length, -1.0];
            for row in 0..3 {
                for column in 0..3 {
                    normal[row][column] += jacobian[row] * jacobian[column];
                }
                rhs[row] -= jacobian[row] * residual;
            }
        }
        let Some(step) = solve(normal, rhs) else {
            break;
        };
        center[0] += step[0];
        center[1] += step[1];
        radius += step[2];
        if step.iter().map(|value| value * value).sum::<f64>() < 1e-24 {
            break;
        }
    }
    (center, radius)
}

fn circle_residuals(points: &[[f64; 2]], center: [f64; 2], radius: f64) -> Vec<f64> {
    points
        .iter()
        .map(|point| (point[0] - center[0]).hypot(point[1] - center[1]) - radius)
        .collect()
}

fn rms(values: &[f64]) -> f64 {
    (values.iter().map(|value| value * value).sum::<f64>() / values.len() as f64).sqrt()
}

/// A tangent-continuous pair of arcs: both share a joint and the tangent
/// through it, and bulge to opposite sides. Five parameters — joint `x`, joint
/// `y`, the tangent's angle, and the two radii — which is the least a two-arc
/// symbol body can be described by once its joint is free.
fn two_arc_residuals(points: &[[f64; 2]], split: usize, parameters: [f64; 5]) -> Vec<f64> {
    let joint = [parameters[0], parameters[1]];
    let (sin, cos) = parameters[2].sin_cos();
    let normal = [-sin, cos];
    let first = [
        joint[0] + parameters[3] * normal[0],
        joint[1] + parameters[3] * normal[1],
    ];
    let second = [
        joint[0] - parameters[4] * normal[0],
        joint[1] - parameters[4] * normal[1],
    ];
    points
        .iter()
        .enumerate()
        .map(|(index, point)| {
            if index < split {
                (point[0] - first[0]).hypot(point[1] - first[1]) - parameters[3]
            } else {
                (point[0] - second[0]).hypot(point[1] - second[1]) - parameters[4]
            }
        })
        .collect()
}

/// Fit that pair by Gauss–Newton over a numeric Jacobian, from several starts,
/// with a halving line search, and keep the best local minimum.
fn fit_two_arcs(points: &[[f64; 2]], split: usize) -> ([f64; 5], f64) {
    let split = split.clamp(2, points.len() - 1);
    let (left, right) = (points[split - 1], points[split]);
    let mut best: Option<([f64; 5], f64)> = None;
    for tangent in [-0.4, 0.0, 0.4, 0.8] {
        for radius in [1.2, 1.5, 1.8] {
            let mut parameters = [
                (left[0] + right[0]) / 2.0,
                (left[1] + right[1]) / 2.0,
                tangent,
                radius,
                radius,
            ];
            let mut current = rms(&two_arc_residuals(points, split, parameters));
            for _ in 0..80 {
                let base = two_arc_residuals(points, split, parameters);
                let mut jacobian = vec![[0.0_f64; 5]; base.len()];
                for column in 0..5 {
                    let step = 1e-6 * parameters[column].abs().max(1.0);
                    let mut bumped = parameters;
                    bumped[column] += step;
                    let there = two_arc_residuals(points, split, bumped);
                    for row in 0..base.len() {
                        jacobian[row][column] = (there[row] - base[row]) / step;
                    }
                }
                let mut normal = [[0.0; 5]; 5];
                let mut rhs = [0.0; 5];
                for row in 0..base.len() {
                    for row_ in 0..5 {
                        for column in 0..5 {
                            normal[row_][column] += jacobian[row][row_] * jacobian[row][column];
                        }
                        rhs[row_] -= jacobian[row][row_] * base[row];
                    }
                }
                for (index, row) in normal.iter_mut().enumerate() {
                    row[index] += 1e-10;
                }
                let Some(step) = solve(normal, rhs) else {
                    break;
                };
                let mut scale = 1.0;
                let mut improved = false;
                while scale > 1e-4 {
                    let mut trial = parameters;
                    for (index, value) in trial.iter_mut().enumerate() {
                        *value += step[index] * scale;
                    }
                    let next = rms(&two_arc_residuals(points, split, trial));
                    if next < current {
                        parameters = trial;
                        current = next;
                        improved = true;
                        break;
                    }
                    scale *= 0.5;
                }
                if !improved {
                    break;
                }
            }
            if best.is_none_or(|(_, best_rms)| current < best_rms) {
                best = Some((parameters, current));
            }
        }
    }
    best.expect("a start")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(six) = args.get(..6) else {
        eprintln!("usage: <drawing.pid> <reference.pdf> <x0> <y0> <x1> <y1>");
        return Ok(());
    };
    let drawing = Path::new(&six[0]);
    let reference = Path::new(&six[1]);
    let window: [f64; 4] = six[2..6]
        .iter()
        .map(|value| value.parse())
        .collect::<Result<Vec<f64>, _>>()?
        .try_into()
        .map_err(|_| "window needs four numbers")?;
    let inside = |point: [f64; 2]| {
        point[0] >= window[0]
            && point[0] <= window[2]
            && point[1] >= window[1]
            && point[1] <= window[3]
    };

    let model = lay_out_drawing(drawing, reference.parent().unwrap_or(Path::new(".")))?;
    println!(
        "template: {:?}",
        model.template.as_ref().map(|template| &template.path)
    );
    let mut arcs: Vec<Arc> = Vec::new();
    println!("page arcs in the window:");
    for primitive in &model.primitives {
        let PaperShape::Arc {
            center_mm,
            radius_mm,
            start_angle,
            end_angle,
            clockwise,
        } = &primitive.shape
        else {
            continue;
        };
        let points = arc_points(&primitive.shape).expect("an arc walks");
        if !points.iter().any(|point| inside(*point)) {
            continue;
        }
        let (first, last) = (points[0], points[points.len() - 1]);
        let chord = (last[0] - first[0]).hypot(last[1] - first[1]);
        let sagitta = radius_mm
            - (radius_mm * radius_mm - chord * chord / 4.0)
                .max(0.0)
                .sqrt();
        println!(
            "  centre ({:.4},{:.4}) r {:.4} mm  {:.2}° -> {:.2}° cw {clockwise}  \
             chord {chord:.4}  sagitta {sagitta:.4}  \
             a {:.2} mm chord height cuts this radius every {:.4} mm",
            center_mm[0],
            center_mm[1],
            radius_mm,
            start_angle.to_degrees(),
            end_angle.to_degrees(),
            CHORD_MM,
            (8.0 * radius_mm * CHORD_MM).sqrt()
        );
        println!(
            "    chain ({:.4},{:.4}) -> ({:.4},{:.4})  confidence {} stream {:?} furniture {} \
             symbol {:?}",
            first[0],
            first[1],
            last[0],
            last[1],
            primitive.confidence,
            primitive.stream_path,
            primitive.furniture,
            primitive.symbol
        );
        arcs.push(((*center_mm, *radius_mm), points));
    }
    if arcs.is_empty() {
        println!("  none");
    }

    let plot = read_plot(&std::fs::read(reference)?)?;
    let mut worst_lattice: f64 = 0.0;
    for segment in &plot.segments {
        for point in [segment.start_mm, segment.end_mm] {
            for axis in point {
                let steps = axis / LATTICE_MM;
                worst_lattice = worst_lattice.max((steps - steps.round()).abs());
            }
        }
    }
    println!(
        "\nreference lattice: {} segments, worst endpoint {worst_lattice:.4} px off a 1/600 in \
         grid ({LATTICE_MM:.4} mm)",
        plot.segments.len()
    );
    println!(
        "  so a printed vertex carries at most ±{:.4} mm on each axis: no shape difference \
         below that is a measurement",
        LATTICE_MM / 2.0
    );

    let mut vertices: Vec<[f64; 2]> = Vec::new();
    println!("\nreference chords in the window:");
    for segment in &plot.segments {
        let (a, b) = (segment.start_mm, segment.end_mm);
        let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        if !inside(mid) {
            continue;
        }
        let length = (b[0] - a[0]).hypot(b[1] - a[1]);
        let steps = ((length / 0.01).ceil() as usize).max(2);
        let (mut worst, mut worst_t) = (0.0_f64, 0.0_f64);
        for step in 0..=steps {
            let t = step as f64 / steps as f64;
            let point = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            let here = arcs
                .iter()
                .map(|(_, points)| distance(point, points))
                .fold(f64::MAX, f64::min);
            if here > worst {
                worst = here;
                worst_t = t;
            }
        }
        let where_ = if worst_t < 0.02 || worst_t > 0.98 {
            "vertex"
        } else {
            "mid-chord"
        };
        println!(
            "  ({:7.3},{:7.3}) -> ({:7.3},{:7.3})  len {length:.3}  \
             worst {worst:.4} mm at t {worst_t:.2} ({where_})",
            a[0], a[1], b[0], b[1]
        );
        vertices.push(a);
        vertices.push(b);
    }
    vertices.dedup_by(|one, other| (*one)[0] == (*other)[0] && (*one)[1] == (*other)[1]);

    println!("\nreference vertices, radial error about each body arc:");
    for vertex in &vertices {
        let mut row = String::new();
        for ((center, radius), _) in &arcs {
            let radial = (vertex[0] - center[0]).hypot(vertex[1] - center[1]) - radius;
            let angle = (vertex[1] - center[1])
                .atan2(vertex[0] - center[0])
                .to_degrees();
            row.push_str(&format!("  [{angle:8.2}° {radial:+.4}]"));
        }
        println!("  ({:8.4},{:8.4}){row}", vertex[0], vertex[1]);
    }

    // Each vertex to its nearer arc, then the circle that lobe lies on. A print
    // that flattened a decoder-correct arc returns the arc's own centre and
    // radius with a micrometre of scatter; a lobe that fits a different circle
    // was drawn from a different body.
    println!("\nthe circle each printed lobe lies on:");
    for (index, ((arc_center, arc_radius), chain)) in arcs.iter().enumerate() {
        let group: Vec<[f64; 2]> = vertices
            .iter()
            .copied()
            .filter(|vertex| {
                let here = distance(*vertex, chain);
                arcs.iter()
                    .all(|(_, other)| std::ptr::eq(other, chain) || distance(*vertex, other) > here)
            })
            .collect();
        if group.len() < 3 {
            println!("  arc {index}: {} vertices, too few to fit", group.len());
            continue;
        }
        let (center, radius) = fit_circle(&group);
        let residuals = circle_residuals(&group, center, radius);
        println!(
            "  arc {index}: {} vertices  fit centre ({:.4},{:.4}) r {radius:.4} ± {:.4} mm  \
             rms {:.4}",
            group.len(),
            center[0],
            center[1],
            rms(&residuals) / (group.len() as f64).sqrt(),
            rms(&residuals)
        );
        println!(
            "    vs body centre ({:.4},{:.4}) r {arc_radius:.4}: \
             Δcentre {:.4} mm  Δr {:+.4} mm  residuals {:?}",
            arc_center[0],
            arc_center[1],
            (center[0] - arc_center[0]).hypot(center[1] - arc_center[1]),
            radius - arc_radius,
            residuals
                .iter()
                .map(|value| (value * 1000.0).round() / 1000.0)
                .collect::<Vec<f64>>()
        );
    }

    // The same wave as a pair of arcs that only has to be tangent at a joint
    // whose position and direction are free. This is the family every two-arc
    // symbol body belongs to; if its best fit still sits above the lattice
    // floor, the body that printed is not made of circular arcs.
    if arcs.len() >= 2 && vertices.len() > arcs.len() {
        let split = vertices
            .iter()
            .filter(|vertex| distance(**vertex, &arcs[0].1) < distance(**vertex, &arcs[1].1))
            .count();
        let (parameters, fit_rms) = fit_two_arcs(&vertices, split);
        println!(
            "\nthe same wave as a tangent-continuous pair of arcs, both radii free:\n  \
             joint ({:.4},{:.4})  tangent {:.2}°  r1 {:.4}  r2 {:.4}  rms {:.4} mm  \
             (split after vertex {split})",
            parameters[0],
            parameters[1],
            parameters[2].to_degrees(),
            parameters[3],
            parameters[4],
            fit_rms
        );
    }

    let worst_vertex = vertices
        .iter()
        .map(|vertex| {
            let ((center, radius), _) = arcs
                .iter()
                .min_by(|left, right| {
                    distance(*vertex, &left.1).total_cmp(&distance(*vertex, &right.1))
                })
                .expect("an arc");
            ((vertex[0] - center[0]).hypot(vertex[1] - center[1]) - radius).abs()
        })
        .fold(0.0_f64, f64::max);
    println!(
        "\nworst |vertex radial error| against the body arcs: {worst_vertex:.4} mm\n\
         a flattening cannot move a vertex, so any gap measured at one is the body's"
    );
    Ok(())
}
