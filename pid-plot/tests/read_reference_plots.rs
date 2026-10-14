//! The five reference plots read back to PyMuPDF's line counts. Text counts
//! match too, except I井场, where `1"` and `NO` are left as two labels.

use std::path::PathBuf;

use pid_plot::read::read_plot;

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"D:\work\cad\PID自研软件验证"))
}

#[test]
fn reference_plots_match_pymupdf_counts() {
    let dir = corpus_dir();
    let samples = [
        ("I井场 注采阀组工艺及自控流程图.pdf", 11_291, 589),
        (
            "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图.pdf",
            1_451,
            156,
        ),
        (
            "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pdf",
            1_662,
            165,
        ),
        (
            "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图.pdf",
            1_976,
            175,
        ),
        (
            "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图.pdf",
            3_695,
            254,
        ),
    ];
    if !dir.join(samples[0].0).is_file() {
        eprintln!(
            "skipping reference-plot counts; {} is absent",
            dir.display()
        );
        return;
    }
    for (name, segments, spans) in samples {
        let path = dir.join(name);
        let bytes =
            std::fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let ink = read_plot(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(ink.segments.len(), segments, "{name} segments");
        assert_eq!(ink.spans.len(), spans, "{name} spans");
        assert!(ink.to_json().is_ok(), "{name} json");
    }
}
