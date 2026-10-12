//! The five drawings lay out, and the missing-symbol counts stay on the
//! figures measured when the plan was opened.

use std::path::PathBuf;

use pid_plot::model::{PaperShape, lay_out_drawing};

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"D:\work\cad\PID自研软件验证"))
}

#[test]
fn five_drawings_match_the_symbol_gap() {
    let dir = corpus_dir();
    let samples = [
        ("I井场 注采阀组工艺及自控流程图.pid", 39, 59),
        (
            "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图.pid",
            23,
            29,
        ),
        (
            "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid",
            23,
            28,
        ),
        (
            "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图.pid",
            26,
            33,
        ),
        (
            "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图.pid",
            29,
            38,
        ),
    ];
    if !dir.join(samples[0].0).is_file() {
        eprintln!("skipping paper models; {} is absent", dir.display());
        return;
    }
    for (name, found, referenced) in samples {
        let model = lay_out_drawing(&dir.join(name), &dir)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(model.symbols_found, found, "{name} found");
        assert_eq!(model.symbols_referenced, referenced, "{name} referenced");
        assert_eq!(
            model.missing_symbols.len(),
            referenced - found,
            "{name} missing"
        );
        assert!(!model.library.is_empty(), "{name} names the library");
        assert!(!model.primitives.is_empty(), "{name} has primitives");
        if let Some([page_w, page_h]) = model.page_mm {
            for primitive in &model.primitives {
                if let PaperShape::Line { start_mm, end_mm } = &primitive.shape {
                    let dx = (start_mm[0] - end_mm[0]).abs();
                    let dy = (start_mm[1] - end_mm[1]).abs();
                    assert!(
                        dy >= 1.0 || dx <= page_w * 0.90,
                        "{name} keeps a sheet-width line"
                    );
                    assert!(
                        dx >= 1.0 || dy <= page_h * 0.90,
                        "{name} keeps a sheet-height line"
                    );
                }
            }
        }
    }
}
