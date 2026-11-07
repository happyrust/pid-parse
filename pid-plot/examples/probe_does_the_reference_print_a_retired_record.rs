//! Does the reference plot print a record whose type word carries `0x8000`?
//!
//! `PSMSerializeIn` seeks past such a record before reading its oid, and the
//! parser follows it (`parse_live_psm_header`): the record decodes nowhere and
//! draws nothing. On the five fixtures every one was an earlier copy of a live
//! record; on the 53-sheet plant backup most are not — 719 of 1549 have no
//! live record of their oid at all (lines and texts on `Labels`, line strings
//! and placements on `Default`) — so "the file retired it" is no longer
//! visible in the file itself. This asks the eleven reference plots instead.
//!
//! Every retired stroke record of each acceptance drawing is decoded as if it
//! were live (the flag cleared in a copy of the stream), placed on our page
//! the way its live neighbours are, mapped into the reference page through the
//! compare fit, and sampled every 0.25 mm. Each sample is asked two things:
//! does the reference ink there, and do we? A retired record the reference
//! inks and we do not is a stroke the skip rule loses; one neither of us inks
//! is a stroke the vendor's own plot leaves out too; one both ink lies under a
//! live record and says nothing either way. Retired lettering is asked the
//! same of the reference's and our text runs, retired placements whether a
//! live placement of the same body stands at the same point.
//!
//! ```powershell
//! $env:PID_PLOT_FONTS='D:\work\cad\PID自研软件验证\fonts'
//! cargo run -p pid-plot --example probe_does_the_reference_print_a_retired_record
//! ```
//!
//! The corpus is `PID_ACCEPTANCE_CORPUS` (default
//! `D:\work\cad\pid-test-data\验收数据`); a drawing without a reference PDF
//! beside it is skipped.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use pid_parse::PidParser;
use pid_parse::parsers::sheet_records::{
    decode_igarc_at, decode_igcircle_at, decode_igline_at, decode_iglines, decode_iglinestring_at,
    decode_igsymbol_at, decode_igsymbols, decode_igtextbox_at, parse_psm_header,
    sheet_record_starts,
};
use pid_plot::compare::{MaskRect, compare_plots_with_model, template_masks};
use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperShape, lay_out_drawing};
use pid_plot::read::{PlotInk, read_plot};
use pid_plot::write::{PlotFont, write_plot};

/// How close a sample has to sit to a segment to count as inked — half a
/// thin pen plus the fit's own residue.
const ON_INK_MM: f64 = 0.3;
/// Spacing of the samples along a retired stroke.
const STEP_MM: f64 = 0.25;
/// How far a text run's origin may sit from a retired label's and still be
/// that label.
const TEXT_NEAR_MM: f64 = 3.0;
/// The reach around a retired placement's insertion point in which the
/// reference's ink is held against ours — wider than the bodies in question.
const PLACEMENT_WINDOW_MM: f64 = 6.0;

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    )
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

fn dist_point_seg(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length2 = dx * dx + dy * dy;
    if length2 <= 1e-12 {
        return dist(p, a);
    }
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / length2).clamp(0.0, 1.0);
    dist(p, [a[0] + dx * t, a[1] + dy * t])
}

/// Segments bucketed on a 2 mm grid, for "is there ink within r of p".
struct Grid {
    segments: Vec<([f64; 2], [f64; 2])>,
    buckets: HashMap<(i64, i64), Vec<usize>>,
}

impl Grid {
    const CELL: f64 = 2.0;

    fn new(segments: Vec<([f64; 2], [f64; 2])>) -> Self {
        let mut buckets: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (index, (a, b)) in segments.iter().enumerate() {
            let steps = (dist(*a, *b) / Self::CELL).ceil().max(1.0) as usize;
            let mut keys = Vec::new();
            for step in 0..=steps {
                let t = step as f64 / steps as f64;
                let key = Self::key([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
            for key in keys {
                buckets.entry(key).or_default().push(index);
            }
        }
        Self { segments, buckets }
    }

    fn key(p: [f64; 2]) -> (i64, i64) {
        (
            (p[0] / Self::CELL).floor() as i64,
            (p[1] / Self::CELL).floor() as i64,
        )
    }

    fn inked(&self, p: [f64; 2], radius: f64) -> bool {
        let (kx, ky) = Self::key(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                let Some(bucket) = self.buckets.get(&(kx + dx, ky + dy)) else {
                    continue;
                };
                if bucket.iter().any(|&index| {
                    let (a, b) = self.segments[index];
                    dist_point_seg(p, a, b) <= radius
                }) {
                    return true;
                }
            }
        }
        false
    }
}

/// What a retired record would draw, in sheet metres.
enum Retired {
    Stroke(Vec<[f64; 2]>),
    Text(String, [f64; 2]),
    Placement {
        insertion: [f64; 2],
        /// `(definition_site_ref, definition_sheet_ref)`.
        definition: (u32, u32),
        live_twin: bool,
    },
}

fn arc_points(
    centre: (f64, f64),
    radius: f64,
    start: f64,
    end: f64,
    clockwise: bool,
) -> Vec<[f64; 2]> {
    use std::f64::consts::TAU;
    let sweep = if clockwise {
        -(start - end).rem_euclid(TAU)
    } else {
        (end - start).rem_euclid(TAU)
    };
    let sweep = if sweep == 0.0 { TAU } else { sweep };
    (0..=64)
        .map(|step| {
            let angle = start + sweep * f64::from(step) / 64.0;
            [
                centre.0 + radius * angle.cos(),
                centre.1 + radius * angle.sin(),
            ]
        })
        .collect()
}

/// Every retired record of one stream, decoded from a copy with its flag
/// cleared.
fn retired_records(data: &[u8]) -> Vec<(usize, u16, Retired)> {
    let live_symbols = decode_igsymbols(data);
    let mut out = Vec::new();
    for at in sheet_record_starts(data) {
        let Some(header) = parse_psm_header(data, at) else {
            continue;
        };
        if !header.native_reader_skips() {
            continue;
        }
        let mut copy = data.to_vec();
        copy[at + 1] &= 0x7F;
        let code = header.type_code;
        let retired = match code {
            0x0018 => decode_igline_at(&copy, at)
                .map(|l| Retired::Stroke(vec![[l.start.0, l.start.1], [l.end.0, l.end.1]])),
            0x0084 => decode_iglinestring_at(&copy, at)
                .map(|l| Retired::Stroke(l.vertices.iter().map(|v| [v.0, v.1]).collect())),
            0x0059 => decode_igcircle_at(&copy, at)
                .map(|c| Retired::Stroke(arc_points(c.center, c.radius, 0.0, 0.0, false))),
            0x0061 => decode_igarc_at(&copy, at).map(|a| {
                Retired::Stroke(arc_points(
                    a.center,
                    a.radius,
                    a.start_angle,
                    a.end_angle,
                    a.flag == 0,
                ))
            }),
            0x004D => decode_igtextbox_at(&copy, at)
                .map(|t| Retired::Text(t.text.clone(), [t.trailing_double_1, t.trailing_double_2])),
            0x00CE => decode_igsymbol_at(&copy, at).map(|s| {
                let insertion = [s.insertion.0, s.insertion.1];
                let definition = (s.definition_site_ref, s.definition_sheet_ref);
                let live_twin = live_symbols.iter().any(|live| {
                    (live.definition_site_ref, live.definition_sheet_ref) == definition
                        && (live.insertion.0 - s.insertion.0).abs() < 1e-9
                        && (live.insertion.1 - s.insertion.1).abs() < 1e-9
                });
                Retired::Placement {
                    insertion,
                    definition,
                    live_twin,
                }
            }),
            _ => None,
        };
        if let Some(retired) = retired {
            out.push((at, code, retired));
        }
    }
    out
}

fn main() {
    let corpus = corpus_dir();
    let Ok(entries) = std::fs::read_dir(&corpus) else {
        println!("no corpus at {}; soft-skip", corpus.display());
        return;
    };
    let mut pids: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "pid"))
        .filter(|path| path.with_extension("pdf").is_file())
        .collect();
    pids.sort();
    let dirs = font_dirs(Some(&corpus));

    // (code, verdict) -> (records, millimetres)
    let mut totals: BTreeMap<(u16, &'static str), (usize, f64)> = BTreeMap::new();
    let mut lost: Vec<String> = Vec::new();
    for pid in &pids {
        let name = pid.file_stem().unwrap().to_string_lossy().into_owned();
        let model = lay_out_drawing(pid, &corpus).expect("layout");
        let prepared = prepare_fonts(&[&model], &dirs).expect("fonts");
        let fonts: Vec<PlotFont<'_>> = prepared
            .iter()
            .map(|face| PlotFont {
                key: &face.key,
                bytes: &face.bytes,
                subset: face.subset,
            })
            .collect();
        let ours = read_plot(&write_plot(&model, &fonts).expect("write")).expect("read ours");
        let reference =
            read_plot(&std::fs::read(pid.with_extension("pdf")).unwrap()).expect("read reference");
        let missing: Vec<MaskRect> = model
            .missing_boxes
            .iter()
            .map(|box_| MaskRect {
                label: box_.path.clone(),
                min_mm: box_.min_mm,
                max_mm: box_.max_mm,
            })
            .collect();
        let fit = compare_plots_with_model(
            &reference,
            &ours,
            &template_masks(reference.page_mm),
            &missing,
            &model,
        )
        .fit;
        let into_reference = |p: [f64; 2]| {
            [
                (p[0] - fit.translation_mm[0]) / fit.scale,
                (p[1] - fit.translation_mm[1]) / fit.scale,
            ]
        };
        let segments_of = |ink: &PlotInk, map: &dyn Fn([f64; 2]) -> [f64; 2]| {
            Grid::new(
                ink.segments
                    .iter()
                    .map(|s| (map(s.start_mm), map(s.end_mm)))
                    .collect(),
            )
        };
        let reference_grid = segments_of(&reference, &|p| p);
        let our_grid = segments_of(&ours, &into_reference);

        let package = PidParser::new().parse_package(pid).expect("parse");
        let mut retired_here = 0usize;
        for (stream, raw) in &package.streams {
            if !stream.rsplit('/').next().unwrap_or("").starts_with("Sheet") {
                continue;
            }
            let records = retired_records(&raw.data);
            if records.is_empty() {
                continue;
            }
            // Where the page puts this stream's metres: the offset a live
            // line of the same stream was drawn with.
            let offset = decode_iglines(&raw.data)
                .iter()
                .find_map(|line| {
                    model.primitives.iter().find_map(|primitive| {
                        let PaperShape::Line { start_mm, .. } = primitive.shape else {
                            return None;
                        };
                        (primitive.stream_path.as_deref() == Some(stream.as_str())
                            && primitive.byte_range
                                == Some([line.byte_range.start, line.byte_range.end]))
                        .then(|| {
                            [
                                start_mm[0] - line.start.0 * 1000.0,
                                start_mm[1] - line.start.1 * 1000.0,
                            ]
                        })
                    })
                })
                .unwrap_or([0.0, 0.0]);
            let place = |p: [f64; 2]| {
                into_reference([p[0] * 1000.0 + offset[0], p[1] * 1000.0 + offset[1]])
            };
            for (at, code, retired) in records {
                retired_here += 1;
                let (verdict, length) = match &retired {
                    Retired::Stroke(points) => {
                        let mut samples = 0usize;
                        let (mut on_reference, mut on_ours) = (0usize, 0usize);
                        let mut length = 0.0;
                        for pair in points.windows(2) {
                            let (a, b) = (place(pair[0]), place(pair[1]));
                            let span = dist(a, b);
                            length += span;
                            let steps = (span / STEP_MM).ceil().max(1.0) as usize;
                            for step in 0..steps {
                                let t = (step as f64 + 0.5) / steps as f64;
                                let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
                                samples += 1;
                                on_reference += usize::from(reference_grid.inked(p, ON_INK_MM));
                                on_ours += usize::from(our_grid.inked(p, ON_INK_MM));
                            }
                        }
                        let r = on_reference as f64 / samples.max(1) as f64;
                        let o = on_ours as f64 / samples.max(1) as f64;
                        let verdict = if length < 0.2 {
                            "too short to read"
                        } else if o >= 0.8 {
                            "under a live stroke we draw"
                        } else if r <= 0.2 {
                            "neither plot inks it"
                        } else if r >= 0.8 {
                            "REFERENCE INKS IT, WE DO NOT"
                        } else {
                            "partly inked by the reference"
                        };
                        if verdict.starts_with("REFERENCE") || verdict.starts_with("partly") {
                            lost.push(format!(
                                "{name} {stream} +{at} 0x{code:04X} {length:.2} mm: reference {r:.2} ours {o:.2}"
                            ));
                        }
                        (verdict, length)
                    }
                    Retired::Text(text, origin) => {
                        let at_ref = place(*origin);
                        let wanted = text.trim();
                        let near = |ink: &PlotInk, map: &dyn Fn([f64; 2]) -> [f64; 2]| {
                            !wanted.is_empty()
                                && ink.spans.iter().any(|span| {
                                    span.text.trim() == wanted
                                        && dist(map(span.origin_mm), at_ref) <= TEXT_NEAR_MM
                                })
                        };
                        let (r, o) = (near(&reference, &|p| p), near(&ours, &into_reference));
                        let verdict = match (r, o) {
                            _ if wanted.is_empty() => "empty lettering",
                            (_, true) => "a live label we letter says the same there",
                            (false, false) => "neither plot letters it",
                            (true, false) => "REFERENCE LETTERS IT, WE DO NOT",
                        };
                        if r && !o {
                            lost.push(format!("{name} {stream} +{at} text {wanted:?}"));
                        }
                        (verdict, 0.0)
                    }
                    Retired::Placement {
                        insertion,
                        definition,
                        live_twin,
                    } => {
                        let at_ref = place(*insertion);
                        // The reference's own ink around the insertion, and
                        // how much of it we do not print: a body the vendor
                        // drew from a retired placement would be ink there
                        // that no live record of ours accounts for.
                        let (mut nearby, mut uncovered) = (0.0, 0.0);
                        for &(a, b) in &reference_grid.segments {
                            if dist_point_seg(at_ref, a, b) > PLACEMENT_WINDOW_MM {
                                continue;
                            }
                            let span = dist(a, b);
                            let steps = (span / STEP_MM).ceil().max(1.0) as usize;
                            for step in 0..steps {
                                let t = (step as f64 + 0.5) / steps as f64;
                                let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
                                if dist(p, at_ref) > PLACEMENT_WINDOW_MM {
                                    continue;
                                }
                                nearby += span / steps as f64;
                                if !our_grid.inked(p, 0.5) {
                                    uncovered += span / steps as f64;
                                }
                            }
                        }
                        let verdict = match (live_twin, uncovered > 0.05) {
                            (true, _) => "a live placement of the same body stands there",
                            (false, false) => {
                                "no live twin; every reference stroke near it is ours"
                            }
                            (false, true) => "NO LIVE TWIN, REFERENCE INK NEAR IT WE LACK",
                        };
                        if !live_twin {
                            lost.push(format!(
                                "{name} {stream} +{at} placement {definition:?} at ({:.2}, {:.2}) mm: reference ink within {PLACEMENT_WINDOW_MM} mm {nearby:.2} mm, of it uncovered by ours {uncovered:.3} mm",
                                at_ref[0], at_ref[1]
                            ));
                        }
                        (verdict, 0.0)
                    }
                };
                let row = totals.entry((code, verdict)).or_default();
                row.0 += 1;
                row.1 += length;
            }
        }
        println!(
            "{name}: {retired_here} retired record(s); fit scale {:.5} translation ({:.3}, {:.3}) mm",
            fit.scale, fit.translation_mm[0], fit.translation_mm[1]
        );
    }

    println!("\n=== retired records on the reference pages, by type code and verdict ===");
    for ((code, verdict), (records, length)) in &totals {
        let length = if *length > 0.0 {
            format!("{length:9.2} mm")
        } else {
            String::new()
        };
        println!("  0x{code:04X} {verdict:<46} {records:>5} {length}");
    }
    println!("\n=== the ones to look at ===");
    if lost.is_empty() {
        println!("  none");
    }
    for line in &lost {
        println!("  {line}");
    }
}
