//! Which paper-table label is each paired run read against, and what is left
//! with none?
//!
//! `reference_plots_acceptance` holds every lettering class to two thresholds
//! and pins how many labels land past the text census's two limits. Both
//! readings rest on the comparison's source lookup: the label a pair of runs
//! is read against decides its class and where its anchor point is. This
//! prints, for each of the eleven `验收数据` sheets, compared whole and with
//! the template masks:
//!
//! - every pair the lookup read as a piece of a label (a ` part` class) or
//!   found no label for (`unknown`), with the label it was read against;
//! - every label past 0.15 mm of anchor-point miss or 0.05 mm of width
//!   difference, the count the gate pins as `WholePage::text_off`;
//! - every lettering class with its count and means.
//!
//! ```text
//! cargo run -p pid-plot --example probe_which_label_a_paired_run_is_read_against
//! ```
//!
//! The corpus is `PID_ACCEPTANCE_CORPUS`, default
//! `D:\work\cad\pid-test-data\验收数据`; the faces come from the corpus's own
//! search path and the share's `fonts`, as the gate takes them. Read-only.

use std::path::PathBuf;
use std::process::ExitCode;

use pid_plot::compare::{
    CompareReport, MaskRect, compare_plots_with_model, template_masks, text_sources,
};
use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperModel, TextAnchor, lay_out_drawing};
use pid_plot::read::{PlotInk, read_plot};
use pid_plot::write::{PlotFont, write_plot};

/// The eleven acceptance sheets, in the gate's order.
const SHEETS: [&str; 11] = [
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

/// Anchor-point miss past which a label is listed, millimetres: the text
/// census's limit, and the gate's `LABEL_POSITION_MM`.
const POSITION_MM: f64 = 0.15;
/// Run-width difference past which a label is listed, millimetres: the text
/// census's limit, and the gate's `LABEL_WIDTH_MM`.
const WIDTH_MM: f64 = 0.05;

/// What the runs at `indices` of `ink` spell, run by run.
fn runs<'a>(ink: &'a PlotInk, indices: &[usize]) -> Vec<&'a str> {
    indices
        .iter()
        .map(|index| ink.spans[*index].text.as_str())
        .collect()
}

/// Print one view's labels past the two limits and its lettering classes.
fn print_view(view: &str, report: &CompareReport, reference: &PlotInk) {
    let mut off: Vec<String> = Vec::new();
    for pair in &report.text_pairs {
        let fraction = match pair.source.as_ref().map(|source| source.anchor) {
            Some(TextAnchor::Center) => 0.5,
            Some(TextAnchor::Right) => 1.0,
            Some(TextAnchor::Left) | None => 0.0,
        };
        let position_mm = (pair.along_mm + fraction * pair.advance_mm).hypot(pair.down_mm);
        if position_mm > POSITION_MM || pair.advance_mm.abs() > WIDTH_MM {
            off.push(format!(
                "      off   {:<22} {:?}: {position_mm:.4} mm off, {:+.4} mm wide",
                pair.class,
                runs(reference, &pair.reference_runs),
                pair.advance_mm
            ));
        }
    }
    println!(
        "   {view}: {} pairs, {} past the label limits",
        report.text_pairs.len(),
        off.len()
    );
    for line in off {
        println!("{line}");
    }
    for class in &report.text_offsets {
        println!(
            "      class {:<22} n={:>4} down {:+.4} advance {:+.6} along {:+.4} max {:.3} mm",
            class.class,
            class.count,
            class.mean_down_heights,
            class.mean_advance_heights,
            class.mean_along_heights,
            class.max_shift_mm
        );
    }
}

fn main() -> ExitCode {
    let corpus = std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    );
    let mut models: Vec<PaperModel> = Vec::new();
    for name in SHEETS {
        let path = corpus.join(format!("{name}.pid"));
        match lay_out_drawing(&path, &corpus) {
            Ok(model) => models.push(model),
            Err(error) => {
                eprintln!("{}: {error}", path.display());
                return ExitCode::from(1);
            }
        }
    }
    let mut dirs = font_dirs(Some(&corpus));
    dirs.push(PathBuf::from("D:/work/cad/PID自研软件验证/fonts"));
    let prepared = match prepare_fonts(&models.iter().collect::<Vec<_>>(), &dirs) {
        Ok(prepared) => prepared,
        Err(error) => {
            eprintln!("fonts: {error}");
            return ExitCode::from(1);
        }
    };
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
            subset: face.subset,
        })
        .collect();
    for (name, model) in SHEETS.iter().zip(&models) {
        let reference = std::fs::read(corpus.join(format!("{name}.pdf")))
            .ok()
            .and_then(|bytes| read_plot(&bytes).ok());
        let ours = write_plot(model, &fonts)
            .ok()
            .and_then(|bytes| read_plot(&bytes).ok());
        let (Some(reference), Some(ours)) = (reference, ours) else {
            eprintln!("{name}: a plot could not be written or read back");
            return ExitCode::from(1);
        };
        let missing: Vec<MaskRect> = model
            .missing_boxes
            .iter()
            .map(|missing| MaskRect {
                label: missing.path.clone(),
                min_mm: missing.min_mm,
                max_mm: missing.max_mm,
            })
            .collect();
        let whole = compare_plots_with_model(&reference, &ours, &[], &missing, model);
        let masked = compare_plots_with_model(
            &reference,
            &ours,
            &template_masks(reference.page_mm),
            &missing,
            model,
        );
        println!(
            "== {name}: {} paper-table labels",
            text_sources(model).len()
        );
        for pair in &whole.text_pairs {
            if !(pair.class.ends_with(" part") || pair.class == "unknown") {
                continue;
            }
            let first = &ours.spans[pair.our_runs[0]];
            let source = pair.source.as_ref().map_or_else(
                || "no label".to_string(),
                |source| {
                    format!(
                        "{:?} at ({:.2}, {:.2}) {:?}/{:?}",
                        source.text,
                        source.origin_mm[0],
                        source.origin_mm[1],
                        source.anchor,
                        source.line
                    )
                },
            );
            println!(
                "   {:<22} ours {:?} reference {:?} at ({:.2}, {:.2}) {:+.4} mm wide on {:.3} mm | {source}",
                pair.class,
                runs(&ours, &pair.our_runs),
                runs(&reference, &pair.reference_runs),
                first.origin_mm[0],
                first.origin_mm[1],
                pair.advance_mm,
                pair.height_mm,
            );
        }
        print_view("whole", &whole, &reference);
        print_view("masked", &masked, &reference);
    }
    ExitCode::SUCCESS
}
