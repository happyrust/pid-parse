//! Write the five plot-comparison reports and overlays.

use std::path::PathBuf;

use pid_plot::compare::{compare_plots, template_masks, write_overlay, write_report};
use pid_plot::model::lay_out_drawing;
use pid_plot::read::read_plot;

const DRAWINGS: [&str; 5] = [
    "I井场 注采阀组工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
    "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
    "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
];

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

#[test]
fn five_drawings_write_compare_reports() {
    let corpus = corpus_dir();
    let ours = corpus.join(format!("plot/{}.pdf", DRAWINGS[0]));
    let reference = corpus.join(format!("{}.pdf", DRAWINGS[0]));
    if !ours.is_file() || !reference.is_file() {
        eprintln!("skipping plot compare; {} is absent", ours.display());
        return;
    }
    let out_dir = corpus.join("plot/compare");
    std::fs::create_dir_all(&out_dir).expect("mkdir compare");
    for name in DRAWINGS {
        let reference_bytes = std::fs::read(corpus.join(format!("{name}.pdf"))).expect("reference");
        let our_bytes = std::fs::read(corpus.join(format!("plot/{name}.pdf"))).expect("ours");
        let reference = read_plot(&reference_bytes).expect("read reference");
        let ours = read_plot(&our_bytes).expect("read ours");
        let model = lay_out_drawing(&corpus.join(format!("{name}.pid")), &corpus).expect("layout");
        let mut masks = template_masks(reference.page_mm);
        for box_ in &model.missing_boxes {
            masks.push(pid_plot::compare::MaskRect {
                label: box_.path.clone(),
                min_mm: box_.min_mm,
                max_mm: box_.max_mm,
            });
        }
        let report = compare_plots(&reference, &ours, &masks);
        let json_path = out_dir.join(format!("{name}.json"));
        let png_path = out_dir.join(format!("{name}.png"));
        write_report(&json_path, &report).expect("json");
        write_overlay(&png_path, &reference, &ours, &report.fit).expect("overlay");
        assert!(json_path.is_file(), "{name} json");
        assert!(
            png_path.metadata().expect("png meta").len() > 100,
            "{name} png"
        );
        assert!(report.coverage.is_finite());
        if !report.fit.within_threshold {
            assert!(report.fit.finding.is_some(), "{name} fit finding");
        }
        eprintln!(
            "{name} coverage {:.4} extra {:.0} mm text {}/{} fit {}",
            report.coverage,
            report.extra_length_mm,
            report.text_matched,
            report.text_reference,
            report.fit.within_threshold
        );
    }
}

#[test]
fn survey_margin_text_once() {
    let corpus = corpus_dir();
    let reference = corpus.join(format!("{}.pdf", DRAWINGS[0]));
    if !reference.is_file() {
        eprintln!("skip survey");
        return;
    }
    for name in DRAWINGS {
        let bytes = std::fs::read(corpus.join(format!("{name}.pdf"))).expect("pdf");
        let ink = read_plot(&bytes).expect("read");
        let page = ink.page_mm;
        eprintln!("PAGE {name} {:.1} x {:.1}", page[0], page[1]);
        let mut edge: Vec<(&str, [f64; 2])> = Vec::new();
        for span in &ink.spans {
            let x = span.origin_mm[0];
            let y = span.origin_mm[1];
            let from_edge = x.min(page[0] - x).min(y).min(page[1] - y);
            if from_edge <= 55.0 {
                edge.push((span.text.trim(), span.origin_mm));
            }
        }
        edge.sort_by(|a, b| a.1[0].total_cmp(&b.1[0]).then(a.1[1].total_cmp(&b.1[1])));
        eprintln!("  edge spans {}", edge.len());
        let mut band: Vec<(&str, [f64; 2])> = Vec::new();
        for span in &ink.spans {
            let x = span.origin_mm[0];
            let y = span.origin_mm[1];
            if x < 90.0 || y < 120.0 || x > page[0] - 220.0 {
                band.push((span.text.trim(), span.origin_mm));
            }
        }
        band.sort_by(|a, b| a.1[0].total_cmp(&b.1[0]).then(a.1[1].total_cmp(&b.1[1])));
        let plot_dir = corpus.join("plot");
        std::fs::create_dir_all(&plot_dir).expect("mkdir plot");
        let dump = plot_dir.join(format!("edge-{name}.txt"));
        let mut lines = Vec::new();
        for (text, origin) in &band {
            lines.push(format!("{:.2}\t{:.2}\t{text}", origin[0], origin[1]));
        }
        std::fs::write(&dump, lines.join("\n")).expect("dump");
        eprintln!("  dumped {}", band.len());
    }
}
