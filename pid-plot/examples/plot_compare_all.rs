//! Run the S4 comparison for all five corpus drawings and print the figures
//! without asserting — for re-measuring baselines after an export change.
use std::path::PathBuf;

use pid_plot::compare::{
    MaskRect, compare_plots_with_model, template_masks, write_overlay, write_report,
};
use pid_plot::model::lay_out_drawing;
use pid_plot::read::read_plot;

const NAMES: [&str; 5] = [
    "I井场 注采阀组工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
    "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
    "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
];

fn main() {
    let corpus = std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    );
    let out_dir = corpus.join("plot/compare");
    std::fs::create_dir_all(&out_dir).expect("mkdir compare");
    for name in NAMES {
        let reference =
            read_plot(&std::fs::read(corpus.join(format!("{name}.pdf"))).expect("reference"))
                .expect("read reference");
        let ours =
            read_plot(&std::fs::read(corpus.join(format!("plot/{name}.pdf"))).expect("ours"))
                .expect("read ours");
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
        write_report(&out_dir.join(format!("{name}.json")), &report).expect("json");
        write_overlay(
            &out_dir.join(format!("{name}.png")),
            &reference,
            &ours,
            &report.fit,
        )
        .expect("png");
        println!(
            "{name}: fit ok={} scale={:.4} t=({:.2},{:.2}) coverage={:.4} extra={:.1} text={}/{}",
            report.fit.within_threshold,
            report.fit.scale,
            report.fit.translation_mm[0],
            report.fit.translation_mm[1],
            report.coverage,
            report.extra_length_mm,
            report.text_matched,
            report.text_reference,
        );
        for class in &report.text_offsets {
            println!(
                "   {:<18} n {:>4}  along {:+.3}  down {:+.3}  width {:+.3}  max {:.2} mm",
                class.class,
                class.count,
                class.mean_along_heights,
                class.mean_down_heights,
                class.mean_advance_heights,
                class.max_shift_mm
            );
        }
    }
}
