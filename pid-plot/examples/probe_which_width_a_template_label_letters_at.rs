//! Which advance does the reference letter a template label at, against the
//! width its style states?
//!
//! For every label of one or more glyphs on the named acceptance sheets, the
//! reference run's advance over ours, grouped by sheet-vs-template, face,
//! style height and `JStyleTextChar +52` (`width_permille`). `DETAIL=1` prints
//! every pair. Run with the sheet names as arguments:
//!
//! ```text
//! cargo run --release -p pid-plot --example probe_which_width_a_template_label_letters_at -- \
//!     "集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图"
//! ```
use std::collections::BTreeMap;
use std::path::PathBuf;

use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperShape, lay_out_drawing};
use pid_plot::read::{TextSpan, read_plot};
use pid_plot::write::{PlotFont, write_plot};

fn norm(s: &str) -> String {
    s.split_whitespace().collect()
}

fn nearest<'a>(
    spans: &'a [TextSpan],
    text: &str,
    at: [f64; 2],
    within: f64,
) -> Option<&'a TextSpan> {
    spans
        .iter()
        .filter(|s| norm(&s.text) == text && s.advance_mm > 0.0)
        .map(|s| (s, (s.origin_mm[0] - at[0]).hypot(s.origin_mm[1] - at[1])))
        .filter(|(_, d)| *d <= within)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(s, _)| s)
}

fn main() {
    let corpus = std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    );
    let names: Vec<String> = std::env::args().skip(1).collect();
    let mut dirs = font_dirs(Some(&corpus));
    dirs.push(PathBuf::from("D:/work/cad/PID自研软件验证/fonts"));
    let detail = std::env::var_os("DETAIL").is_some();
    let mut classes: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for name in &names {
        let model = lay_out_drawing(&corpus.join(format!("{name}.pid")), &corpus).expect("layout");
        let prepared = prepare_fonts(&[&model], &dirs).expect("fonts");
        let fonts: Vec<PlotFont<'_>> = prepared
            .iter()
            .map(|f| PlotFont {
                key: &f.key,
                bytes: &f.bytes,
                subset: f.subset,
            })
            .collect();
        let ours = read_plot(&write_plot(&model, &fonts).expect("write")).expect("read ours");
        let reference = read_plot(&std::fs::read(corpus.join(format!("{name}.pdf"))).expect("pdf"))
            .expect("read reference");
        let short: String = name.chars().take(8).collect();
        for primitive in &model.primitives {
            let PaperShape::Text {
                origin_mm,
                height_mm,
                value,
                font,
                width_permille,
                ..
            } = &primitive.shape
            else {
                continue;
            };
            let text = norm(value);
            if text.is_empty() {
                continue;
            }
            let Some(our) = nearest(&ours.spans, &text, *origin_mm, 40.0) else {
                continue;
            };
            let Some(theirs) = nearest(&reference.spans, &text, our.origin_mm, 3.0) else {
                continue;
            };
            let ratio = theirs.advance_mm / our.advance_mm;
            let key = format!(
                "{} {:>5} {:<12} h{:.3} w{}",
                if primitive.furniture {
                    "template"
                } else {
                    "sheet"
                },
                short,
                font.clone().unwrap_or_default(),
                height_mm.unwrap_or(0.0),
                width_permille.map_or("-".into(), |w| w.to_string())
            );
            if detail {
                let symbol = primitive
                    .symbol
                    .as_deref()
                    .map(|s| s.rsplit(['/', '\\']).next().unwrap_or(s));
                println!(
                    "{key} {symbol:?} {text} ours {:.2} ref {:.2} ratio {ratio:.3} tf {:.3}/{:.3} along {:+.2}",
                    our.advance_mm,
                    theirs.advance_mm,
                    our.height_mm,
                    theirs.height_mm,
                    (our.origin_mm[0] - theirs.origin_mm[0]) * theirs.direction[0]
                        + (our.origin_mm[1] - theirs.origin_mm[1]) * theirs.direction[1]
                );
            }
            classes.entry(key).or_default().push(ratio);
        }
    }
    for (key, ratios) in classes {
        let n = ratios.len() as f64;
        let mean = ratios.iter().sum::<f64>() / n;
        let lo = ratios.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = ratios.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        println!("{key}  n={n:>4} ref/ours mean {mean:.3} [{lo:.3}, {hi:.3}]");
    }
}
