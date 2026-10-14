//! S3: unmatched lettering keeps its paper-table source, and I井场 is grouped.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pid_plot::compare::{
    MaskRect, compare_plots_with_model, template_masks, text_sources, write_report,
};
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
fn five_drawings_keep_text_sources() {
    let corpus = corpus_dir();
    let reference = corpus.join(format!("{}.pdf", DRAWINGS[0]));
    if !reference.is_file() {
        eprintln!(
            "skipping sourced compare; {} is absent",
            reference.display()
        );
        return;
    }
    let out_dir = corpus.join("plot/compare");
    std::fs::create_dir_all(&out_dir).expect("mkdir compare");
    let mut summary = Vec::new();
    for name in DRAWINGS {
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
        write_report(&out_dir.join(format!("{name}.json")), &report).expect("json");
        let sourced = report
            .unmatched_our_text
            .iter()
            .filter(|run| run.stream_path.is_some() || run.symbol.is_some())
            .count();
        summary.push(format!(
            "{name}\ncoverage {:.4} extra {:.0} inside {:.0} outside {:.0} text {}/{} sourced {sourced}/{} fit {}",
            report.coverage,
            report.extra_length_mm,
            report.extra_inside_missing_mm,
            report.extra_outside_missing_mm,
            report.text_matched,
            report.text_reference,
            report.unmatched_our_text.len(),
            report.fit.within_threshold,
        ));
        if name.starts_with("I井场") {
            assert!(
                sourced * 2 > report.unmatched_our_text.len(),
                "sourced {sourced} of {}",
                report.unmatched_our_text.len()
            );
            assert!(!report.uncovered_reference.is_empty(), "uncovered groups");
            let leaked = report.unmatched_reference_text.iter().any(|run| {
                run.origin_mm[0] < 40.0
                    && (run.text.contains("日期")
                        || run.text.contains("修改号")
                        || run.text.contains("会签"))
            });
            assert!(!leaked, "signature column still compared");
            summary.push(i_wellsite_notes(&report, &model));
        }
    }
    std::fs::write(out_dir.join("s3-summary.txt"), summary.join("\n\n")).expect("summary");
}

fn i_wellsite_notes(
    report: &pid_plot::compare::CompareReport,
    model: &pid_plot::model::PaperModel,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "fit scale {:.6} shift {:.3} {:.3} pairs {}",
        report.fit.scale,
        report.fit.translation_mm[0],
        report.fit.translation_mm[1],
        report.fit.pairs
    ));
    lines.push(format!(
        "lengths ref {:.0} covered {:.0} masked {:.0}",
        report.reference_length_mm, report.covered_length_mm, report.masked_reference_length_mm
    ));
    lines.push("extra groups".to_string());
    for group in &report.extra_outside_by_symbol {
        lines.push(format!("  {:.0} {}", group.length_mm, group.label));
    }
    lines.push("uncovered groups".to_string());
    for group in &report.uncovered_reference {
        lines.push(format!("  {:.0} {}", group.length_mm, group.label));
    }
    let sources = text_sources(model);
    let mut drawing: BTreeMap<String, usize> = BTreeMap::new();
    let mut symbol: BTreeMap<String, usize> = BTreeMap::new();
    let mut nulls = 0;
    for source in &sources {
        if source.text.contains("NULL") {
            nulls += 1;
        }
        let table = if source.symbol.is_some() {
            &mut symbol
        } else {
            &mut drawing
        };
        *table.entry(source.text.clone()).or_default() += 1;
    }
    lines.push(format!(
        "paper text drawing {} runs / {} distinct, symbol {} runs / {} distinct, containing NULL {nulls}",
        drawing.values().sum::<usize>(),
        drawing.len(),
        symbol.values().sum::<usize>(),
        symbol.len()
    ));
    lines.push("drawing text".to_string());
    for (text, count) in &drawing {
        lines.push(format!("  {count} {text}"));
    }
    lines.push("symbol text top".to_string());
    let mut symbol_rows: Vec<_> = symbol.iter().collect();
    symbol_rows.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    for (text, count) in symbol_rows.iter().take(20) {
        lines.push(format!("  {count} {text}"));
    }
    let ours: BTreeMap<String, usize> =
        report
            .unmatched_our_text
            .iter()
            .fold(BTreeMap::new(), |mut map, run| {
                *map.entry(run.text.clone()).or_default() += 1;
                map
            });
    let mut only_ref: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for run in &report.unmatched_reference_text {
        if ours.contains_key(&run.text) {
            continue;
        }
        only_ref
            .entry(bucket(&run.text))
            .or_default()
            .push(run.text.clone());
    }
    for (bucket, texts) in &mut only_ref {
        texts.sort();
        texts.dedup();
        lines.push(format!("reference only {bucket} {}", texts.len()));
        for text in texts.iter().take(12) {
            lines.push(format!("  {text}"));
        }
    }
    lines.join("\n")
}

fn bucket(text: &str) -> &'static str {
    const FRAME: [&str; 10] = [
        "会签",
        "签署",
        "修改号",
        "批准",
        "审定",
        "校对",
        "制图",
        "项目号",
        "文件号",
        "阶段",
    ];
    if FRAME.iter().any(|word| text.contains(word)) {
        return "图框";
    }
    if text.contains("管线")
        || text.contains("-PG-")
        || text.contains("-PC-")
        || text.contains("DWG-")
    {
        return "管线号";
    }
    if text.contains('"') || text.ends_with('寸') {
        return "其他";
    }
    "位号"
}
