//! S4 ratchet gate: the three comparison figures on the five corpus drawings
//! may improve (higher coverage, less extra ink, more matched text) and may
//! not regress below the measured baseline.

use std::path::PathBuf;

use pid_plot::compare::{
    MaskRect, compare_plots_with_model, template_masks, write_overlay, write_report,
};
use pid_plot::model::lay_out_drawing;
use pid_plot::read::read_plot;

struct Baseline {
    name: &'static str,
    min_coverage: f64,
    max_extra_mm: f64,
    min_text_matched: usize,
}

// Baselines re-locked after styles, complex-dash hydraulic ticks, boundary
// fills, symbol lettering, non-uniform arc/circle scaling and ConsistencyChecks
// review-layer suppression landed.
const BASELINES: [Baseline; 5] = [
    Baseline {
        name: "I井场 注采阀组工艺及自控流程图",
        min_coverage: 0.9980,
        max_extra_mm: 65.0,
        min_text_matched: 520,
    },
    Baseline {
        name: "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
        min_coverage: 0.9990,
        max_extra_mm: 20.0,
        min_text_matched: 125,
    },
    Baseline {
        name: "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
        min_coverage: 0.9970,
        max_extra_mm: 15.0,
        min_text_matched: 103,
    },
    Baseline {
        name: "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
        min_coverage: 0.9975,
        max_extra_mm: 35.0,
        min_text_matched: 145,
    },
    Baseline {
        name: "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
        min_coverage: 0.9980,
        max_extra_mm: 50.0,
        min_text_matched: 234,
    },
];

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

#[test]
fn five_drawings_hold_the_ratchet() {
    let corpus = corpus_dir();
    let first_ours = corpus.join(format!("plot/{}.pdf", BASELINES[0].name));
    let first_reference = corpus.join(format!("{}.pdf", BASELINES[0].name));
    if !first_ours.is_file() || !first_reference.is_file() {
        eprintln!("skipping plot ratchet; {} is absent", first_ours.display());
        return;
    }
    let out_dir = corpus.join("plot/compare");
    std::fs::create_dir_all(&out_dir).expect("mkdir compare");
    for baseline in BASELINES {
        let name = baseline.name;
        let reference_bytes = std::fs::read(corpus.join(format!("{name}.pdf"))).expect("reference");
        let our_bytes = std::fs::read(corpus.join(format!("plot/{name}.pdf"))).expect("ours");
        let reference = read_plot(&reference_bytes).expect("read reference");
        let ours = read_plot(&our_bytes).expect("read ours");
        let model = lay_out_drawing(&corpus.join(format!("{name}.pid")), &corpus).expect("layout");
        let template = template_masks(reference.page_mm);
        let missing: Vec<MaskRect> = model
            .missing_boxes
            .iter()
            .map(|box_| MaskRect {
                label: box_.path.clone(),
                min_mm: box_.min_mm,
                max_mm: box_.max_mm,
            })
            .collect();
        let report = compare_plots_with_model(&reference, &ours, &template, &missing, &model);
        let json_path = out_dir.join(format!("{name}.json"));
        let png_path = out_dir.join(format!("{name}.png"));
        write_report(&json_path, &report).expect("json");
        write_overlay(&png_path, &reference, &ours, &report.fit).expect("overlay");
        assert!(
            png_path.metadata().expect("png meta").len() > 100,
            "{name} overlay png"
        );
        assert!(
            report.fit.within_threshold,
            "{name} fit outside threshold: {:?}",
            report.fit
        );
        assert!(
            report.coverage >= baseline.min_coverage,
            "{name} coverage {:.4} < {:.4}",
            report.coverage,
            baseline.min_coverage
        );
        assert!(
            report.extra_length_mm <= baseline.max_extra_mm,
            "{name} extra {:.1} mm > {:.1} mm",
            report.extra_length_mm,
            baseline.max_extra_mm
        );
        assert!(
            report.text_matched >= baseline.min_text_matched,
            "{name} text_matched {} < {}",
            report.text_matched,
            baseline.min_text_matched
        );
    }
}
