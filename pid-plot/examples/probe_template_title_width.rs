//! Why is the template title `华北油田苏桥储气库群` 0.05 mm wider than the
//! reference's, on the sheets where the census flags it?
//!
//! The census (`probe_why_paired_labels_miss_the_reference`) flags three pairs
//! whose anchor point is where the plotter model puts it and whose run is
//! 0.0532 mm too wide — I, L M and 一级闪蒸分离器B. Its cause list has no
//! width rule for them, so it files them under `line-metrics` although nothing
//! is wrong vertically. This probe takes that one label on every sheet and
//! splits the width difference into the three places it can come from:
//!
//! * the horizontal scale `Tm` carries (`sx`), applied to every glyph and to
//!   every step,
//! * the per-gap letter spacing `Tc`,
//! * the last glyph's own advance, which no `Tc` corrects.
//!
//! ```text
//! cargo run -p pid-plot --example probe_template_title_width
//! ```
//!
//! Read-only: it writes the same plot bytes it reads and nothing else. The
//! corpus is `PID_ACCEPTANCE_CORPUS`, default `D:\work\cad\pid-test-data\验收数据`.
//!
//! Verdict vocabulary, one per sheet and then once for the class:
//! `scale` — the whole difference is the `sx` the driver wrote in `Tm`;
//! `spacing` — it is `Tc`; `last-glyph` — it is the final glyph's advance;
//! `split` — several of them, the table says how much each.

use std::path::PathBuf;
use std::process::ExitCode;

#[path = "support/glyphs.rs"]
mod glyphs;

use glyphs::{MM_PER_PT, PIXEL_PT, PageGlyphs, Run, read_glyphs};
use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::lay_out_drawing;
use pid_plot::read::read_plot;
use pid_plot::write::{PlotFont, write_plot};

/// The eleven acceptance sheets, short name first.
const SHEETS: [(&str, &str); 11] = [
    ("I", "I井场 注采阀组工艺及自控流程图"),
    ("JK", "J K井场 注采阀组工艺及自控流程图"),
    ("LM", "L M井场 注采阀组工艺及自控流程图"),
    (
        "FlashA",
        "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
    ),
    (
        "FlashB",
        "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
    ),
    (
        "ColdA",
        "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
    ),
    (
        "HxCD",
        "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
    ),
    (
        "AirA",
        "集注站 烃水露点控制单元（一） 空冷器A集预冷分离器A工艺及自控流程图",
    ),
    (
        "AirB",
        "集注站 烃水露点控制单元（二） 空冷器B集预冷分离器B工艺及自控流程图",
    ),
    (
        "AirC",
        "集注站 烃水露点控制单元（三） 空冷器C集预冷分离器C工艺及自控流程图",
    ),
    ("A2", "集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图"),
];

/// The label this probe is about.
const TITLE: &str = "华北油田苏桥储气库群";

/// What one side of one sheet says about the title.
struct Reading {
    /// `Tf`, points.
    size_pt: f64,
    /// `Tm`'s horizontal scale against the body.
    sx: f64,
    /// Mean `Tc` over the gaps inside the run, points.
    tc_pt: f64,
    /// Every gap's step, points.
    steps_pt: Vec<f64>,
    /// First glyph's origin to the last glyph's advance, millimetres.
    width_mm: f64,
    /// The last glyph's own advance, millimetres.
    last_advance_mm: f64,
    /// Body size in millimetres.
    body_mm: f64,
}

impl Reading {
    fn of(run: &Run) -> Option<Self> {
        let first = run.glyphs.first()?;
        let last = run.glyphs.last()?;
        let steps_pt: Vec<f64> = run
            .glyphs
            .windows(2)
            .map(|pair| {
                let delta = [
                    pair[1].origin_mm[0] - pair[0].origin_mm[0],
                    pair[1].origin_mm[1] - pair[0].origin_mm[1],
                ];
                (delta[0] * delta[0] + delta[1] * delta[1]).sqrt() / MM_PER_PT
            })
            .collect();
        let span_mm = [
            last.origin_mm[0] - first.origin_mm[0],
            last.origin_mm[1] - first.origin_mm[1],
        ];
        Some(Self {
            size_pt: first.body_mm / MM_PER_PT,
            sx: first.sx,
            tc_pt: run.mean_tc_pt(),
            steps_pt,
            width_mm: span_mm[0].hypot(span_mm[1]) + last.advance_mm,
            last_advance_mm: last.advance_mm,
            body_mm: first.body_mm,
        })
    }
}

/// One sheet's row.
struct Row {
    sheet: &'static str,
    reference: Reading,
    ours: Reading,
    /// Width difference, millimetres: ours minus the reference's.
    delta_mm: f64,
    /// How much of `delta_mm` the `sx` alone accounts for.
    scale_mm: f64,
    /// How much the per-gap `Tc` accounts for.
    spacing_mm: f64,
    /// How much the last glyph's own advance accounts for.
    last_mm: f64,
    verdict: &'static str,
}

fn main() -> ExitCode {
    let corpus = std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    );
    if !corpus.join(format!("{}.pdf", SHEETS[0].1)).is_file() {
        eprintln!("{} has no reference plots", corpus.display());
        return ExitCode::from(2);
    }
    let mut models = Vec::new();
    for (_, name) in SHEETS {
        match lay_out_drawing(&corpus.join(format!("{name}.pid")), &corpus) {
            Ok(model) => models.push(model),
            Err(error) => {
                eprintln!("{name}: {error}");
                return ExitCode::from(1);
            }
        }
    }
    let mut dirs = font_dirs(Some(&corpus));
    let beside = PathBuf::from("D:/work/cad/PID自研软件验证/fonts");
    if beside.is_dir() {
        dirs.push(beside);
    }
    let Ok(prepared) = prepare_fonts(&models.iter().collect::<Vec<_>>(), &dirs) else {
        eprintln!("no faces to letter with");
        return ExitCode::from(1);
    };
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
            subset: face.subset,
        })
        .collect();
    let mut rows: Vec<Row> = Vec::new();
    for ((sheet, name), model) in SHEETS.iter().zip(&models) {
        let Ok(reference_bytes) = std::fs::read(corpus.join(format!("{name}.pdf"))) else {
            eprintln!("{name}: no reference plot");
            return ExitCode::from(1);
        };
        let Ok(our_bytes) = write_plot(model, &fonts) else {
            eprintln!("{name}: could not write the plot");
            return ExitCode::from(1);
        };
        let (Ok(reference), Ok(ours), Some(reference_glyphs), Some(our_glyphs)) = (
            read_plot(&reference_bytes),
            read_plot(&our_bytes),
            read_glyphs(&reference_bytes),
            read_glyphs(&our_bytes),
        ) else {
            eprintln!("{name}: a plot could not be read back");
            return ExitCode::from(1);
        };
        let reference_page = PageGlyphs::new(reference_glyphs);
        let our_page = PageGlyphs::new(our_glyphs);
        let Some((reference_run, _)) = reference
            .spans
            .iter()
            .enumerate()
            .find(|(_, span)| span.text.trim() == TITLE)
        else {
            continue;
        };
        let Some((our_run, _)) = ours
            .spans
            .iter()
            .enumerate()
            .find(|(_, span)| span.text.trim() == TITLE)
        else {
            continue;
        };
        let reference = Run::of(&reference_page, &reference, reference_run);
        let ours = Run::of(&our_page, &ours, our_run);
        let (Some(reference), Some(ours)) = (Reading::of(&reference), Reading::of(&ours)) else {
            continue;
        };
        let delta_mm = ours.width_mm - reference.width_mm;
        // What each component alone would do to the width, holding the other
        // two at the reference's readings.
        let gaps = reference.steps_pt.len() as f64;
        let scale_mm =
            gaps * (reference.size_pt + reference.tc_pt) * (ours.sx - reference.sx) * MM_PER_PT
                + (ours.sx - reference.sx) * reference.last_advance_mm / reference.sx;
        let spacing_mm = gaps * (ours.tc_pt - reference.tc_pt) * reference.sx * MM_PER_PT;
        let last_mm = ours.last_advance_mm - reference.last_advance_mm;
        let residual = delta_mm - (scale_mm + spacing_mm + last_mm);
        let share = |part: f64| part.abs() / delta_mm.abs().max(1e-9);
        let verdict = if share(scale_mm) > 0.9 {
            "scale"
        } else if share(spacing_mm) > 0.9 {
            "spacing"
        } else if share(last_mm) > 0.9 {
            "last-glyph"
        } else if residual.abs() > 0.01 {
            "other"
        } else {
            "split"
        };
        println!(
            "{sheet:<7} sx ref {:.4} ours {:.4}  Tc ref {:+.4} ours {:+.4}  width ref {:.4} ours {:.4}  delta {:+.4} mm  (scale {:+.4} spacing {:+.4} last {:+.4} residual {:+.4})  {verdict}",
            reference.sx,
            ours.sx,
            reference.tc_pt,
            ours.tc_pt,
            reference.width_mm,
            ours.width_mm,
            delta_mm,
            scale_mm,
            spacing_mm,
            last_mm,
            residual,
        );
        rows.push(Row {
            sheet,
            reference,
            ours,
            delta_mm,
            scale_mm,
            spacing_mm,
            last_mm,
            verdict,
        });
    }
    if rows.is_empty() {
        eprintln!("no sheet letters {TITLE}");
        return ExitCode::from(1);
    }
    println!();
    println!("the two sides' steps, points, and in 150 dpi pixels:");
    for row in &rows {
        let steps = |reading: &Reading| {
            reading
                .steps_pt
                .iter()
                .map(|step| format!("{:.3}/{:.0}px", step, step / PIXEL_PT))
                .collect::<Vec<_>>()
                .join(" ")
        };
        println!("{:<7} ref {}", row.sheet, steps(&row.reference));
        println!("{:<7} our {}", "", steps(&row.ours));
        println!(
            "{:<7}     last glyph advance ref {:.4} ours {:.4} mm, body ref {:.4} ours {:.4} mm",
            "",
            row.reference.last_advance_mm,
            row.ours.last_advance_mm,
            row.reference.body_mm,
            row.ours.body_mm,
        );
    }
    let flagged: Vec<&Row> = rows
        .iter()
        .filter(|row| row.delta_mm.abs() > 0.05)
        .collect();
    println!();
    println!(
        "{} of {} sheets carry the label; {} are more than 0.05 mm wide",
        rows.len(),
        SHEETS.len(),
        flagged.len()
    );
    for row in &flagged {
        println!(
            "  {:<7} {:+.4} mm, {} — of which the horizontal scale {:+.4} ({:.0}%), \
             the letter spacing {:+.4}, the last glyph {:+.4}",
            row.sheet,
            row.delta_mm,
            row.verdict,
            row.scale_mm,
            100.0 * row.scale_mm / row.delta_mm,
            row.spacing_mm,
            row.last_mm,
        );
    }
    let all_scale = flagged.iter().all(|row| row.verdict == "scale");
    println!();
    if flagged.is_empty() {
        println!("verdict: no sheet misses the title's width");
    } else if all_scale {
        let (reference_sx, our_sx) = (flagged[0].reference.sx, flagged[0].ours.sx);
        println!(
            "verdict: source side — every flagged sheet's width difference is the horizontal \
             scale the plot driver wrote in `Tm` ({reference_sx:.3} against the {our_sx:.4} this \
             crate computes), applied to the whole run; no spacing rule is missing. The same \
             driver-written scale is what the census already files as `template-scale`.",
        );
    } else {
        println!(
            "verdict: not one cause — see the table; the rows marked `other` leave a residual \
             the three components do not explain"
        );
    }
    ExitCode::SUCCESS
}
