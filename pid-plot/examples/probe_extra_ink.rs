//! Split the five reference drawings' extra millimetres into three piles,
//! and count arc trailing bytes that are not 0 or 1.
//!
//! The piles use the same masks and the same 0.5 mm rule as `plot_compare`.
//! Furniture is whatever the nearest paper stroke marks as the border
//! template. A and 低温A are labelled 参考没画框 because those reference
//! pages were plotted without the frame their `.pid` links. Nothing here
//! writes a PDF or changes a drawing.
//!
//! ```text
//! cargo run -p pid-plot --example probe_extra_ink
//! ```
//!
//! Read-only. The flag census walks the five drawings, every `.sym` under
//! `test-file/symbols-full`, and `.sym` / `.igr` under `<corpus>/Ref`.
//! It uses the same `0x0061` envelope test as `examples/probe_arc2d_flag.rs`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_plot::compare::{
    MaskRect, compare_plots_with_model, extra_ink_piles, template_masks,
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

const ARC_PAYLOAD_LEN: usize = 59;
const TYPE_ARC: u16 = 0x0061;
const COORDINATE_LIMIT: f64 = 1e6;
const TAU: f64 = std::f64::consts::TAU;

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn reference_has_no_furniture(name: &str) -> bool {
    name.contains("分离器A")
}

fn piles(corpus: &Path) {
    println!("== extra ink ==");
    for name in DRAWINGS {
        let reference_path = corpus.join(format!("{name}.pdf"));
        let ours_path = corpus.join(format!("plot/{name}.pdf"));
        let pid_path = corpus.join(format!("{name}.pid"));
        if !reference_path.is_file() || !ours_path.is_file() || !pid_path.is_file() {
            println!("{name}: skip (reference, plot, or pid missing)");
            continue;
        }
        let reference = match read_plot(&std::fs::read(&reference_path).expect("reference")) {
            Ok(ink) => ink,
            Err(error) => {
                println!("{name}: reference unreadable: {error}");
                continue;
            }
        };
        let ours = match read_plot(&std::fs::read(&ours_path).expect("ours")) {
            Ok(ink) => ink,
            Err(error) => {
                println!("{name}: plot unreadable: {error}");
                continue;
            }
        };
        let model = match lay_out_drawing(&pid_path, corpus) {
            Ok(model) => model,
            Err(error) => {
                println!("{name}: layout failed: {error}");
                continue;
            }
        };
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
        let piles = extra_ink_piles(&reference, &ours, &template, &missing, &model);
        let symbol_mm: f64 = piles.symbols.iter().map(|symbol| symbol.length_mm).sum();
        println!(
            "{name}\n  compare extra {:.1} mm   piles {:.1} mm (furniture {:.1} + symbols {:.1} + sheet {:.1})",
            report.extra_length_mm, piles.extra_mm, piles.furniture_mm, symbol_mm, piles.sheet_mm
        );
        if reference_has_no_furniture(name) {
            println!(
                "  furniture {:.1} mm — 参考没画框（这一堆是预期多画，不是要改的缺陷）",
                piles.furniture_mm
            );
        } else {
            println!("  furniture {:.1} mm", piles.furniture_mm);
        }
        let shown = piles.symbols.len().min(8);
        for symbol in piles.symbols.iter().take(shown) {
            let ranges: Vec<String> = symbol
                .byte_ranges
                .iter()
                .map(|range| format!("{}..{}", range[0], range[1]))
                .collect();
            let more = if symbol.omitted_ranges > 0 {
                format!(" (+{} ranges)", symbol.omitted_ranges)
            } else {
                String::new()
            };
            let range_text = if ranges.is_empty() {
                "—".to_string()
            } else {
                ranges.join(", ")
            };
            println!(
                "  symbol {:>7.1} mm  {}  bytes {range_text}{more}",
                symbol.length_mm, symbol.name
            );
        }
        if piles.symbols.len() > shown {
            let rest: f64 = piles
                .symbols
                .iter()
                .skip(shown)
                .map(|symbol| symbol.length_mm)
                .sum();
            println!(
                "  symbol {:>7.1} mm  ({} more symbols)",
                rest,
                piles.symbols.len() - shown
            );
        }
        println!("  sheet {:.1} mm", piles.sheet_mm);
    }
}

struct Hit {
    path: String,
    at: usize,
    flag: u8,
    full_turn: bool,
}

fn u16_le(bytes: &[u8], at: usize) -> Option<u16> {
    bytes
        .get(at..at + 2)
        .and_then(|slice| <[u8; 2]>::try_from(slice).ok())
        .map(u16::from_le_bytes)
}

fn u32_le(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..at + 4)
        .and_then(|slice| <[u8; 4]>::try_from(slice).ok())
        .map(u32::from_le_bytes)
}

fn f64_le(bytes: &[u8], at: usize) -> Option<f64> {
    bytes
        .get(at..at + 8)
        .and_then(|slice| <[u8; 8]>::try_from(slice).ok())
        .map(f64::from_le_bytes)
}

fn full_turn(start: f64, end: f64) -> bool {
    let delta = (start - end).abs();
    delta > 1e-6 && (delta / TAU - 1.0).abs() < 1e-4
}

fn scan_arcs(
    path: &Path,
    hits: &mut Vec<Hit>,
    by_flag: &mut BTreeMap<u8, usize>,
    full_turns: &mut usize,
) {
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let mut at = 0usize;
    while at + 6 + ARC_PAYLOAD_LEN <= bytes.len() {
        let raw = u16_le(&bytes, at).unwrap_or(0);
        if raw & 0x0FFF == TYPE_ARC && u32_le(&bytes, at + 2) == Some(ARC_PAYLOAD_LEN as u32) {
            let body = at + 6;
            let payload = &bytes[body..body + ARC_PAYLOAD_LEN];
            let values = [
                f64_le(payload, 18),
                f64_le(payload, 26),
                f64_le(payload, 34),
                f64_le(payload, 42),
                f64_le(payload, 50),
            ];
            if let [Some(cx), Some(cy), Some(radius), Some(start), Some(end)] = values {
                let in_domain = [cx, cy, radius, start, end]
                    .iter()
                    .all(|value| value.is_finite() && value.abs() <= COORDINATE_LIMIT);
                if in_domain && radius > 0.0 {
                    let flag = payload[ARC_PAYLOAD_LEN - 1];
                    *by_flag.entry(flag).or_default() += 1;
                    let turn = full_turn(start, end);
                    if turn {
                        *full_turns += 1;
                    }
                    if (flag != 0 && flag != 1) || turn {
                        if hits.len() < 30 {
                            hits.push(Hit {
                                path: path.display().to_string(),
                                at,
                                flag,
                                full_turn: turn,
                            });
                        }
                    }
                }
            }
        }
        at += 1;
    }
}

fn collect(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect(&path, extension, out);
        } else if path.extension().is_some_and(|ext| ext == extension) {
            out.push(path);
        }
    }
}

fn flag_census(corpus: &Path) {
    println!("== arc trailing bytes ==");
    let mut files = Vec::new();
    for name in DRAWINGS {
        let path = corpus.join(format!("{name}.pid"));
        if path.is_file() {
            files.push(path);
        }
    }
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../test-file/symbols-full");
    collect(&bundled, "sym", &mut files);
    collect(&corpus.join("Ref"), "sym", &mut files);
    collect(&corpus.join("Ref"), "igr", &mut files);
    let mut by_flag = BTreeMap::new();
    let mut full_turns = 0usize;
    let mut hits = Vec::new();
    for path in &files {
        scan_arcs(path, &mut hits, &mut by_flag, &mut full_turns);
    }
    let other: usize = by_flag
        .iter()
        .filter(|(flag, _)| **flag != 0 && **flag != 1)
        .map(|(_, count)| count)
        .sum();
    println!("files {}", files.len());
    println!("flag census: {by_flag:?}");
    println!("trailing byte other than 0 or 1: {other}");
    println!("arcs whose ends differ by one full turn: {full_turns}");
    for hit in &hits {
        if hit.flag != 0 && hit.flag != 1 {
            println!("  flag {}  @{:<8} {}", hit.flag, hit.at, hit.path);
        } else if hit.full_turn {
            println!("  full-turn flag {}  @{:<8} {}", hit.flag, hit.at, hit.path);
        }
    }
}

fn main() {
    let corpus = corpus_dir();
    println!("corpus {}", corpus.display());
    piles(&corpus);
    flag_census(&corpus);
}
