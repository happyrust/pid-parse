//! Where does a sheet's template ink sit against the reference, and does its
//! drawing ink sit the same way?
//!
//! Pairs every straight horizontal / vertical stroke of 10 mm or more on our
//! page with the nearest reference stroke of the same span, and reports the
//! offset across the stroke (ours − reference) split into template furniture
//! and the drawing's own ink. A template whose border frame places it off the
//! page origin shows here as a template-only offset.
//!
//! ```text
//! cargo run --release -p pid-plot --example probe_where_the_template_ink_sits -- \
//!     "集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图"
//! ```
use std::path::PathBuf;

use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperShape, lay_out_drawing};
use pid_plot::read::{InkSegment, read_plot};
use pid_plot::write::{PlotFont, write_plot};

/// One straight stroke: its across coordinate, its span along, and its ends.
type Stroke = (f64, f64, f64, [f64; 2], [f64; 2]);

fn stats(values: &mut [f64]) -> String {
    if values.is_empty() {
        return "none".into();
    }
    values.sort_by(f64::total_cmp);
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    format!(
        "n {} mean {mean:+.3} median {:+.3} [{:+.3}, {:+.3}]",
        values.len(),
        values[values.len() / 2],
        values[0],
        values[values.len() - 1]
    )
}

fn straight(segments: &[InkSegment], horizontal: bool) -> Vec<Stroke> {
    segments
        .iter()
        .filter_map(|segment| {
            let (a, b) = (segment.start_mm, segment.end_mm);
            if (a[0] - b[0]).hypot(a[1] - b[1]) < 10.0 {
                return None;
            }
            if horizontal && (a[1] - b[1]).abs() < 0.005 {
                Some((a[1], a[0].min(b[0]), a[0].max(b[0]), a, b))
            } else if !horizontal && (a[0] - b[0]).abs() < 0.005 {
                Some((a[0], a[1].min(b[1]), a[1].max(b[1]), a, b))
            } else {
                None
            }
        })
        .collect()
}

fn main() {
    let corpus = std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    );
    let mut dirs = font_dirs(Some(&corpus));
    dirs.push(PathBuf::from("D:/work/cad/PID自研软件验证/fonts"));
    for name in std::env::args().skip(1) {
        // `<drawing.pid>|<reference.pdf>` reads a pair outside the corpus.
        let (pid, pdf) = match name.split_once('|') {
            Some((pid, pdf)) => (PathBuf::from(pid), PathBuf::from(pdf)),
            None => (
                corpus.join(format!("{name}.pid")),
                corpus.join(format!("{name}.pdf")),
            ),
        };
        let model = lay_out_drawing(&pid, &corpus).expect("layout");
        let prepared = prepare_fonts(&[&model], &dirs).expect("fonts");
        let fonts: Vec<PlotFont<'_>> = prepared
            .iter()
            .map(|face| PlotFont {
                key: &face.key,
                bytes: &face.bytes,
                subset: face.subset,
            })
            .collect();
        let ours = read_plot(&write_plot(&model, &fonts).expect("write")).expect("read ours");
        let theirs = read_plot(&std::fs::read(&pdf).expect("pdf")).expect("read reference");
        let furniture: Vec<([f64; 2], [f64; 2])> = model
            .primitives
            .iter()
            .filter(|primitive| primitive.furniture)
            .filter_map(|primitive| match &primitive.shape {
                PaperShape::Line { start_mm, end_mm } => Some((*start_mm, *end_mm)),
                _ => None,
            })
            .collect();
        let near =
            |p: [f64; 2], q: [f64; 2]| (p[0] - q[0]).abs() < 0.05 && (p[1] - q[1]).abs() < 0.05;
        let is_furniture = |a: [f64; 2], b: [f64; 2]| {
            furniture
                .iter()
                .any(|(s, e)| (near(a, *s) && near(b, *e)) || (near(a, *e) && near(b, *s)))
        };
        println!("== {name}");
        for horizontal in [true, false] {
            let reference = straight(&theirs.segments, horizontal);
            let (mut template, mut drawing) = (Vec::new(), Vec::new());
            for (across, lo, hi, a, b) in straight(&ours.segments, horizontal) {
                let best = reference
                    .iter()
                    .filter(|(_, l2, h2, _, _)| {
                        (h2 - l2 - (hi - lo)).abs() < 0.6 && (l2 - lo).abs() < 1.5
                    })
                    .map(|(c2, _, _, _, _)| across - c2)
                    .filter(|d| d.abs() < 1.5)
                    .min_by(|x, y| x.abs().total_cmp(&y.abs()));
                if let Some(d) = best {
                    if is_furniture(a, b) {
                        template.push(d);
                    } else {
                        drawing.push(d);
                    }
                }
            }
            let axis = if horizontal { "y" } else { "x" };
            println!("{axis} template {}", stats(&mut template));
            println!("{axis} drawing  {}", stats(&mut drawing));
        }
    }
}
