//! Temporary dump of style-table text heights. Deleted after the check.

use std::path::PathBuf;

use pid_plot::model::{PaperShape, lay_out_drawing};

const DRAWINGS: [&str; 5] = [
    "I井场 注采阀组工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
    "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
    "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
];

fn main() {
    let corpus = PathBuf::from(r"D:\work\cad\PID自研软件验证");
    for name in DRAWINGS {
        let model = lay_out_drawing(&corpus.join(format!("{name}.pid")), &corpus)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        println!("DRAWING\t{name}");
        for primitive in &model.primitives {
            let PaperShape::Text {
                origin_mm,
                height_mm,
                value,
                font,
                ..
            } = &primitive.shape
            else {
                continue;
            };
            let (Some(font), Some(height_mm)) = (font.as_deref(), *height_mm) else {
                continue;
            };
            if height_mm <= 0.0 || value.is_empty() {
                continue;
            }
            println!(
                "TEXT\t{}\t{height_mm:.6}\t{:.4}\t{:.4}\t{}",
                font.replace('\t', " "),
                origin_mm[0],
                origin_mm[1],
                value.replace(['\t', '\n', '\r'], " ")
            );
        }
    }
}
