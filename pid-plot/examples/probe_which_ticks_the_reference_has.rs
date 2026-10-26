//! Which of our 45° hydraulic tick strokes the reference actually has, and what
//! the reference inks where we stamp one it does not.
//!
//! ```text
//! cargo run -p pid-plot --example probe_which_ticks_the_reference_has -- "<ours.pdf>" "<reference.pdf>"
//! cargo run -p pid-plot --example probe_which_ticks_the_reference_has -- \
//!     "<ours.pdf>" "<reference.pdf>" window <x_mm> <y_mm> [radius_mm]
//! ```
//!
//! Read-only. A complex dash stamps a two-slash glyph once per fitted cell; the
//! glyph's slashes are 3.592 mm at 45° in the page's frame (the run's own
//! direction rotates them, so a vertical run's pair prints at 135°). Pairing
//! our slashes with the reference's by midpoint answers the one question the
//! residual raises: is a charged slash ours alone, or the same slash placed a
//! little differently? `window` then prints both sides' strokes near one place,
//! which is how the charged place's rail-vs-glyph answer is read.
//!
//! The pairing accepts both diagonals: the 45° filter alone would report a
//! vertical run's glyph as missing when the reference draws it at 135°
//! (`docs/analysis/2026-10-26-the-hydraulic-glyph-comes-off-sub-half-period-stubs.md`).
//! It runs both ways round on purpose — our slashes with no counterpart are
//! ink we added, the reference's with no counterpart are ink we are missing —
//! and prints, for each, the nearest stroke of the other side whether or not
//! it is a slash, because a "missing" slash within 0.2 mm of a stroke of the
//! same length is one stroke cut at a crossing rather than a glyph.

use std::path::PathBuf;

use pid_plot::read::read_plot;

type Stroke = ([f64; 2], [f64; 2]);

fn len(s: &Stroke) -> f64 {
    ((s.1[0] - s.0[0]).powi(2) + (s.1[1] - s.0[1]).powi(2)).sqrt()
}

/// The stroke's direction in [0, 180): a slash reads the same either way round.
fn angle(s: &Stroke) -> f64 {
    ((s.1[1] - s.0[1]).atan2(s.1[0] - s.0[0]).to_degrees() + 180.0) % 180.0
}

fn mid(s: &Stroke) -> [f64; 2] {
    [(s.0[0] + s.1[0]) / 2.0, (s.0[1] + s.1[1]) / 2.0]
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// The glyph's slashes on one page: 2.8–4.4 mm, on either diagonal.
///
/// The band has to clear both sides' spelling of the same stroke. This package
/// inks a hydraulic glyph's slash at 3.021 mm (I) / 3.592 mm (the CD sheets)
/// against the reference's 2.996 / 3.57, and a symbol-body diagonal at our
/// 4.245 against its 4.192: the fit's ~1% length difference straddles a band
/// edge that sits on a glyph, so 3.0–4.2 counted the reference's 4.192 strokes
/// and dropped our 4.245 copies of exactly those strokes, reporting 22 of them
/// as missing glyphs.
fn glyph_slashes(strokes: &[Stroke]) -> Vec<Stroke> {
    strokes
        .iter()
        .filter(|s| (2.8..=4.4).contains(&len(s)))
        .filter(|s| {
            let a = angle(s);
            (a - 45.0).abs() <= 6.0 || (a - 135.0).abs() <= 6.0
        })
        .copied()
        .collect()
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let read = |p: &str| {
        let ink = read_plot(&std::fs::read(PathBuf::from(p)).unwrap()).unwrap();
        ink.segments
            .iter()
            .map(|s| (s.start_mm, s.end_mm))
            .collect::<Vec<Stroke>>()
    };
    let ours = read(&a[0]);
    let reference = read(&a[1]);

    if a.get(2).map(String::as_str) == Some("window") {
        let cx: f64 = a[3].parse().expect("x_mm");
        let cy: f64 = a[4].parse().expect("y_mm");
        let r: f64 = a.get(5).map_or(8.0, |v| v.parse().expect("radius_mm"));
        for (tag, strokes) in [("OURS", &ours), ("REF ", &reference)] {
            let mut rows: Vec<(f64, Stroke)> = strokes
                .iter()
                .map(|s| (dist(mid(s), [cx, cy]), *s))
                .filter(|(d, _)| *d <= r)
                .collect();
            rows.sort_by(|l, r| l.0.total_cmp(&r.0));
            println!(
                "== {tag} {} strokes within {r} mm of ({cx},{cy})",
                rows.len()
            );
            for (d, s) in rows {
                println!(
                    "  d {d:7.3}  ({:8.3},{:8.3}) -> ({:8.3},{:8.3})  len {:6.3}  ang {:6.1}",
                    s.0[0],
                    s.0[1],
                    s.1[0],
                    s.1[1],
                    len(&s),
                    angle(&s)
                );
            }
        }
        return;
    }

    let ours_slashes = glyph_slashes(&ours);
    let ref_slashes = glyph_slashes(&reference);
    println!(
        "ours {} strokes, {} slashes; reference {} strokes, {} slashes",
        ours.len(),
        ours_slashes.len(),
        reference.len(),
        ref_slashes.len()
    );
    report("ours", &ours_slashes, &ref_slashes, &reference);
    report("reference", &ref_slashes, &ours_slashes, &ours);
}

/// Print one side's slashes the other side has nothing within 0.6 mm of.
///
/// Both directions, because they answer different questions: a slash of ours
/// with no counterpart is ink we added, a reference slash with no counterpart
/// is ink we are missing — which is the one that has to reach the report's
/// uncovered column if the missing-glyph reading is right.
///
/// `other_strokes` is every stroke of the other side, slash or not. A slash
/// that pairs with a longer stroke is not a glyph at all but the same stroke
/// cut at a crossing, and the length next to the distance is what says which:
/// a counterpart within 0.2 mm whose length matches is the same stroke, one
/// 40 mm away is not.
fn report(tag: &str, slashes: &[Stroke], other_slashes: &[Stroke], other_strokes: &[Stroke]) {
    let mut paired = 0;
    let mut alone: Vec<(f64, Stroke)> = Vec::new();
    for s in slashes {
        let d = other_slashes
            .iter()
            .map(|o| dist(mid(s), mid(o)))
            .fold(f64::INFINITY, f64::min);
        if d <= 0.6 {
            paired += 1;
        } else {
            alone.push((d, *s));
        }
    }
    println!("{tag} paired with a slash of the other side within 0.6 mm: {paired}");
    alone.sort_by(|l, r| r.0.total_cmp(&l.0));
    println!("{tag} with no such slash ({}):", alone.len());
    for (d, s) in &alone {
        let m = mid(s);
        let nearest = other_strokes.iter().map(|o| (dist(m, mid(o)), *o)).fold(
            (f64::INFINITY, *s),
            |best, c| {
                if c.0 < best.0 { c } else { best }
            },
        );
        println!(
            "  {tag} ({:7.3},{:7.3}) -> ({:7.3},{:7.3})  len {:5.3} ang {:5.1}  nearest \
             {tag} slash {d:6.3} mm; nearest other stroke {:.3} mm len {:.3} ang {:.1}",
            s.0[0],
            s.0[1],
            s.1[0],
            s.1[1],
            len(s),
            angle(s),
            nearest.0,
            len(&nearest.1),
            angle(&nearest.1)
        );
    }
}
