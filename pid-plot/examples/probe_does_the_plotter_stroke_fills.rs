//! Does the reference plotter stroke the outline of every fill, and with which
//! pen?
//!
//! Our fills paint their ring without stroking it (`f`, no `S`), and the reader
//! takes a fill's outline at width 0. The pen tally then charges those outlines
//! against whatever pen the reference used. Before changing the writer, the
//! question a page can answer is whether the reference's fills are stroked at
//! all: the same path painted fill-then-stroke reads back as two segments with
//! identical geometry, one at width 0 and one at the pen's width.
//!
//! So this probe pairs every 0-width reference segment with a non-zero segment
//! on the **same** geometry (same midpoint, same direction, same length) and
//! tallies the twin's width. Every tally is a fill the plotter outlined; a
//! 0-width segment with no twin is a fill it left unoutlined.
//!
//! It then asks the converse, which is what decides whether the writer has to
//! change: for every reference stroke there is, does **our** page print the same
//! pen at the same place? A reference hairline that our page never laid would be
//! a fill outline we are missing; a hairline we already lay means the fill needs
//! nothing.
//!
//! ```text
//! cargo run -p pid-plot --example probe_does_the_plotter_stroke_fills
//! ```
//!
//! Read-only. The corpus is `PID_PARSE_SELF_CHECK_DIR`, or
//! `D:\work\cad\PID自研软件验证`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::lay_out_drawing;
use pid_plot::read::{PlotInk, read_plot};
use pid_plot::write::{PlotFont, write_plot};

const NAMES: [&str; 5] = [
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

fn centi_mm(width: f64) -> u64 {
    (width * 100.0).round().max(0.0) as u64
}

fn length(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// For every reference stroke, whether our page prints that pen there.
fn converse(
    reference: &PlotInk,
    ours: &PlotInk,
    same: &mut usize,
    other: &mut usize,
    absent: &mut usize,
) {
    for segment in &reference.segments {
        let width = centi_mm(segment.width_mm);
        if width == 0 {
            continue;
        }
        let len = length(segment.start_mm, segment.end_mm);
        if len < 0.2 {
            continue;
        }
        let direction = [
            segment.end_mm[0] - segment.start_mm[0],
            segment.end_mm[1] - segment.start_mm[1],
        ];
        let unit = [direction[0] / len, direction[1] / len];
        let midpoint = [
            (segment.start_mm[0] + segment.end_mm[0]) / 2.0,
            (segment.start_mm[1] + segment.end_mm[1]) / 2.0,
        ];
        let mut seen: Option<u64> = None;
        for candidate in &ours.segments {
            let other_len = length(candidate.start_mm, candidate.end_mm);
            if other_len < 1e-9 {
                continue;
            }
            let other_mid = [
                (candidate.start_mm[0] + candidate.end_mm[0]) / 2.0,
                (candidate.start_mm[1] + candidate.end_mm[1]) / 2.0,
            ];
            if length(other_mid, midpoint) > 0.3 {
                continue;
            }
            let ratio = (other_len / len).min(len / other_len);
            if ratio < 0.5 {
                continue;
            }
            let other = [
                candidate.end_mm[0] - candidate.start_mm[0],
                candidate.end_mm[1] - candidate.start_mm[1],
            ];
            let dot = (unit[0] * other[0] / other_len + unit[1] * other[1] / other_len).abs();
            if dot < 0.99 {
                continue;
            }
            let candidate_width = centi_mm(candidate.width_mm);
            if candidate_width == width {
                seen = Some(width);
                break;
            }
            seen = Some(candidate_width);
        }
        match seen {
            Some(printed) if printed == width => *same += 1,
            Some(_) => *other += 1,
            None => *absent += 1,
        }
    }
}

fn main() {
    let corpus = corpus_dir();
    let dirs = font_dirs(Some(&corpus));
    let mut outlined: BTreeMap<u64, usize> = BTreeMap::new();
    let mut bare = 0usize;
    let (mut same, mut other, mut absent) = (0usize, 0usize, 0usize);
    for name in NAMES {
        let ink = read_plot(&std::fs::read(corpus.join(format!("{name}.pdf"))).expect("reference"))
            .expect("read reference");
        let mut page_outlined: BTreeMap<u64, usize> = BTreeMap::new();
        let mut page_bare = 0usize;
        for segment in &ink.segments {
            if centi_mm(segment.width_mm) != 0 {
                continue;
            }
            let len = length(segment.start_mm, segment.end_mm);
            if len < 0.2 {
                continue;
            }
            let direction = [
                segment.end_mm[0] - segment.start_mm[0],
                segment.end_mm[1] - segment.start_mm[1],
            ];
            let unit = [direction[0] / len, direction[1] / len];
            let midpoint = [
                (segment.start_mm[0] + segment.end_mm[0]) / 2.0,
                (segment.start_mm[1] + segment.end_mm[1]) / 2.0,
            ];
            let mut twin: Option<u64> = None;
            for candidate in &ink.segments {
                let width = centi_mm(candidate.width_mm);
                if width == 0 {
                    continue;
                }
                let other = [
                    candidate.end_mm[0] - candidate.start_mm[0],
                    candidate.end_mm[1] - candidate.start_mm[1],
                ];
                let other_len = other[0].hypot(other[1]);
                if other_len < 1e-9 {
                    continue;
                }
                let ratio = (other_len / len).min(len / other_len);
                if ratio < 0.95 {
                    continue;
                }
                let other_mid = [
                    (candidate.start_mm[0] + candidate.end_mm[0]) / 2.0,
                    (candidate.start_mm[1] + candidate.end_mm[1]) / 2.0,
                ];
                if length(other_mid, midpoint) > 0.02 {
                    continue;
                }
                let dot = (unit[0] * other[0] / other_len + unit[1] * other[1] / other_len).abs();
                if dot < 0.999 {
                    continue;
                }
                twin = Some(width);
                break;
            }
            match twin {
                Some(width) => *page_outlined.entry(width).or_default() += 1,
                None => page_bare += 1,
            }
        }
        let outlined_here: usize = page_outlined.values().sum();
        println!("{name}\n   {outlined_here} fill outlines stroked, {page_bare} left bare");
        for (width, count) in &page_outlined {
            *outlined.entry(*width).or_default() += count;
            println!("      twin {:.2} mm  {count}", *width as f64 / 100.0);
        }
        bare += page_bare;
        let model = lay_out_drawing(&corpus.join(format!("{name}.pid")), &corpus).expect("layout");
        let prepared = prepare_fonts(&[&model], &dirs).expect("fonts");
        let fonts: Vec<PlotFont<'_>> = prepared
            .iter()
            .map(|face| PlotFont {
                key: &face.key,
                bytes: &face.bytes,
                subset: face.subset,
            })
            .collect();
        let pdf = write_plot(&model, &fonts).expect("write");
        let ours = read_plot(&pdf).expect("read ours");
        let (mut page_same, mut page_other, mut page_absent) = (0usize, 0usize, 0usize);
        converse(
            &ink,
            &ours,
            &mut page_same,
            &mut page_other,
            &mut page_absent,
        );
        println!(
            "   of the reference's own strokes: {page_same} we print with the same pen, \
             {page_other} with another, {page_absent} not at all"
        );
        same += page_same;
        other += page_other;
        absent += page_absent;
    }
    let total: usize = outlined.values().sum();
    println!("\nTOTAL {total} fill outlines stroked, {bare} left bare");
    for (width, count) in &outlined {
        println!("   twin {:.2} mm  {count}", *width as f64 / 100.0);
    }
    println!(
        "\nREFERENCE STROKES: {same} printed with the same pen, {other} with another, \
         {absent} missing"
    );
}
