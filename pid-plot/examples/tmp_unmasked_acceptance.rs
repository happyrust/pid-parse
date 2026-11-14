use std::path::PathBuf;

use pid_plot::compare::{MaskRect, compare_plots_with_model, template_masks, write_report};
use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperModel, lay_out_drawing};
use pid_plot::read::read_plot;
use pid_plot::write::{PlotFont, write_plot};

const NAMES: [&str; 11] = [
    "I井场 注采阀组工艺及自控流程图",
    "J K井场 注采阀组工艺及自控流程图",
    "L M井场 注采阀组工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
    "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
    "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
    "集注站 烃水露点控制单元（一） 空冷器A集预冷分离器A工艺及自控流程图",
    "集注站 烃水露点控制单元（二） 空冷器B集预冷分离器B工艺及自控流程图",
    "集注站 烃水露点控制单元（三） 空冷器C集预冷分离器C工艺及自控流程图",
    "集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图",
];

fn main() {
    let corpus = std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    );
    let out_dir = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from(r"D:\work\tmp\unmasked"), PathBuf::from);
    std::fs::create_dir_all(&out_dir).expect("mkdir");
    let models: Vec<PaperModel> = NAMES
        .iter()
        .map(|name| lay_out_drawing(&corpus.join(format!("{name}.pid")), &corpus).expect("layout"))
        .collect();
    let mut dirs = font_dirs(Some(&corpus));
    let beside = PathBuf::from("D:/work/cad/PID自研软件验证/fonts");
    if beside.is_dir() {
        dirs.push(beside);
    }
    let prepared = prepare_fonts(&models.iter().collect::<Vec<_>>(), &dirs).expect("fonts");
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
            subset: face.subset,
        })
        .collect();
    println!(
        "sheet        cov_mask   cov_all   unc_all extra_all    txt_mask     txt_all  pen_all"
    );
    for (name, model) in NAMES.iter().zip(&models) {
        let reference = read_plot(&std::fs::read(corpus.join(format!("{name}.pdf"))).expect("ref"))
            .expect("read ref");
        let ours = read_plot(&write_plot(model, &fonts).expect("write")).expect("read ours");
        let missing: Vec<MaskRect> = model
            .missing_boxes
            .iter()
            .map(|box_| MaskRect {
                label: box_.path.clone(),
                min_mm: box_.min_mm,
                max_mm: box_.max_mm,
            })
            .collect();
        let masked = compare_plots_with_model(
            &reference,
            &ours,
            &template_masks(reference.page_mm),
            &missing,
            model,
        );
        let all = compare_plots_with_model(&reference, &ours, &[], &missing, model);
        write_report(&out_dir.join(format!("{name}.json")), &all).expect("json");
        let short: String = name.chars().take(12).collect();
        println!(
            "{:<12} {:>9.6} {:>9.6} {:>9.3} {:>9.3} {:>5}/{:<5} {:>5}/{:<5}  {}/{} unp {}",
            short,
            masked.coverage,
            all.coverage,
            all.reference_length_mm - all.covered_length_mm,
            all.extra_length_mm,
            masked.text_matched,
            masked.text_reference,
            all.text_matched,
            all.text_reference,
            all.pen.agreed,
            all.pen.paired,
            all.pen.unpaired
        );
    }
}
