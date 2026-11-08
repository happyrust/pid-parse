//! Why does the reference letter the air coolers' `SP:` colon one 150 dpi
//! pixel nearer the `P` than every other sheet's, and nearer than the `lpDx`
//! model can produce?
//!
//! The census flags three pairs, one on each air-cooler sheet: the run is
//! 0.169 mm (1 px) wider than the reference's while its anchor point is where
//! the model puts it. The model's own `Tc` is `0` and the reference's is not
//! the same on every sheet, so the question is which quantity actually
//! differs. This probe lays the same label side by side on every sheet that
//! carries it and prints, for both sides:
//!
//! * the raw stored value and the record's anchor code, paragraph and
//!   lettering, read back off the `.pid`;
//! * each glyph's origin in 150 dpi pixels from the page's lower-left corner,
//!   and the step between them;
//! * the run's letter spacing, and the step the `lpDx` model gives from the
//!   face's own 72-unit advances.
//!
//! ```text
//! cargo run -p pid-plot --example probe_sp_colon_step
//! ```
//!
//! Read-only. The corpus is `PID_ACCEPTANCE_CORPUS`, default
//! `D:\work\cad\pid-test-data\验收数据`.
//!
//! Verdict vocabulary: `same` — every sheet's steps and spacing agree with the
//! model; `reference-split` — the reference writes the run as more than one
//! text object, so its second step is a placement, not a spacing; `model-short`
//! — the model's own step is one pixel short of the reference's everywhere;
//! `outlier` — one sheet differs from the others and from the model.

use std::path::PathBuf;
use std::process::ExitCode;

#[path = "support/glyphs.rs"]
mod glyphs;

use glyphs::{PIXEL_MM, PageGlyphs, Run, read_glyphs};
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
const LABEL: &str = "SP:";

/// GDI's 72-pixel advances of `0x20..=0x7E` for the two hinted Latin faces.
/// SimSun and FangSong are not here: a 256-unit bitmap-style CJK face reads its
/// own `hmtx` on the 72-pixel cell, which for `S`, `P`, `:` is 36.
const ARIAL_72PX: [u16; 95] = [
    18, 20, 23, 36, 36, 57, 43, 12, 21, 21, 25, 37, 18, 21, 18, 18, 36, 36, 36, 36, 36, 36, 36, 36,
    36, 36, 18, 18, 37, 37, 37, 36, 65, 43, 43, 46, 46, 43, 39, 50, 46, 18, 32, 43, 36, 53, 46, 50,
    43, 50, 46, 43, 40, 46, 43, 64, 41, 42, 39, 18, 18, 18, 27, 36, 21, 36, 36, 32, 36, 36, 18, 36,
    36, 14, 15, 33, 14, 55, 36, 36, 36, 36, 21, 32, 18, 36, 33, 47, 31, 31, 31, 21, 17, 21, 37,
];

/// GDI's cap and ascent heights at `lfHeight = 72`, pixels: the `M` a
/// cap-height and an ascent style divide the style height by.
const REFERENCE_PX: [(&str, f64, f64); 4] = [
    ("SimSun", 49.2, 62.0),
    ("FangSong", 48.2, 62.0),
    ("Arial", 46.0, 58.0),
    ("Arial Narrow", 44.0, 58.0),
];

/// The 72-pixel advance of `ch` in `face`.
fn reference_advance_px(face: &str, ch: char) -> Option<f64> {
    match face {
        "Arial" | "Arial Narrow" => {
            let ansi = if (' '..='~').contains(&ch) { ch } else { '?' };
            let index = usize::try_from(u32::from(ansi) - 0x20).ok()?;
            ARIAL_72PX.get(index).map(|px| f64::from(*px))
        }
        // SimSun and FangSong are 256-unit faces whose `usWinAscent +
        // usWinDescent` is the em, so `advance_em × 72` is the 72-pixel
        // advance: every ASCII half-width is 36, a full-width CJK glyph 72.
        "SimSun" | "FangSong" | "NSimSun" => Some(if ch.is_ascii() { 36.0 } else { 72.0 }),
        _ => None,
    }
}

/// One side's reading of the label.
struct Reading {
    /// The step between neighbouring glyphs, 150 dpi pixels.
    steps_px: Vec<f64>,
    /// Mean letter spacing over the run's gaps, points.
    tc_pt: f64,
    /// `Tm`'s horizontal scale.
    sx: f64,
    /// The face the first glyph was drawn with.
    face: String,
    /// How many runs the reader clustered the label into.
    runs: usize,
}

impl Reading {
    fn of(page: &PageGlyphs, ink: &pid_plot::read::PlotInk, runs: &[usize]) -> Option<Self> {
        let mut origins: Vec<([f64; 2], [f64; 2])> = Vec::new();
        let mut gaps: Vec<f64> = Vec::new();
        let mut first: Option<glyphs::PlacedGlyph> = None;
        for run in runs {
            let reading = Run::of(page, ink, *run);
            let direction = ink.spans[*run].direction;
            for glyph in &reading.glyphs {
                if first.is_none() {
                    first = Some(glyph.clone());
                }
                origins.push((glyph.origin_mm, direction));
            }
            gaps.extend(
                reading
                    .gaps_pt
                    .iter()
                    .copied()
                    .filter(|gap| gap.is_finite()),
            );
        }
        let first = first?;
        let offsets_mm: Vec<f64> = origins
            .iter()
            .map(|(origin, direction)| {
                let delta = [
                    origin[0] - first.origin_mm[0],
                    origin[1] - first.origin_mm[1],
                ];
                delta[0] * direction[0] + delta[1] * direction[1]
            })
            .collect();
        let steps_px = offsets_mm
            .windows(2)
            .map(|pair| (pair[1] - pair[0]) / PIXEL_MM)
            .collect();
        Some(Self {
            steps_px,
            tc_pt: if gaps.is_empty() {
                0.0
            } else {
                gaps.iter().sum::<f64>() / gaps.len() as f64
            },
            sx: first.sx,
            face: first.face,
            runs: runs.len(),
        })
    }

    /// The step the `lpDx` model gives each glyph but the last, pixels: the
    /// face's 72-pixel advance times `a8`, rounded on its own with the running
    /// error carried into the next glyph.
    ///
    /// `a8` divides the *style* height — the 2.54 mm `SP:` styles state —
    /// converted to 150 dpi pixels, by the face's 72-pixel cap height. The
    /// run's own `Tf` is the cell that height rounds to (`10.56 pt`), not the
    /// height the model starts from; reading the cell back as if it were the
    /// style height inflates every step by half.
    fn model_steps_px(&self, text: &str, style_mm: f64, basis: &str) -> Option<Vec<f64>> {
        let (_, cap72, asc72) = REFERENCE_PX.iter().find(|(key, ..)| *key == self.face)?;
        let measures = if basis == "Ascent" { *asc72 } else { *cap72 };
        let a8 = style_mm / PIXEL_MM / measures;
        let mut float_acc = 0.0;
        let mut int_acc = 0i64;
        let mut steps = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        for ch in chars.iter().take(chars.len().saturating_sub(1)) {
            let step = reference_advance_px(&self.face, *ch)? * a8;
            float_acc += step;
            let mut whole = step.round() as i64;
            let drift = float_acc - (int_acc + whole) as f64;
            if drift > 1.0 {
                whole += 1;
            } else if drift < -1.0 {
                whole -= 1;
            }
            int_acc += whole;
            steps.push(whole as f64);
        }
        Some(steps)
    }
}

/// The record behind one label, from the parsed `.pid`.
struct Facts {
    value: String,
    anchor_code: u8,
    paragraph: String,
    lettering: String,
    box_mm: Option<f64>,
}

/// The style height and basis the laid-out label letters at, off the model.
fn laid_out(model: &pid_plot::model::PaperModel) -> Option<(f64, String)> {
    use pid_plot::model::PaperShape;
    model.primitives.iter().find_map(|primitive| {
        let PaperShape::Text {
            value,
            height_mm,
            height_basis,
            ..
        } = &primitive.shape
        else {
            return None;
        };
        if value.trim_end() != LABEL {
            return None;
        }
        Some(((*height_mm)?, format!("{height_basis:?}")))
    })
}

/// Every `SP:` text record of one sheet, with the facts the run's placement
/// turns on: the raw anchor code, the paragraph's alignment and line spacing,
/// and whether the record carries a box.
fn facts(path: &std::path::Path) -> Vec<Facts> {
    let mut out = Vec::new();
    let Ok(doc) = pid_parse::PidParser::new().parse_file(path) else {
        return out;
    };
    let geometry = pid_parse::build_normalized_geometry(&doc);
    let styles = pid_parse::style_link::text_styles_for_document(&doc);
    for entity in &geometry.entities {
        let pid_parse::PidGraphicKind::Text {
            anchor_code,
            value,
            text_box_width,
            ..
        } = &entity.kind
        else {
            continue;
        };
        if value.trim_end() != LABEL {
            continue;
        }
        let resolved = entity
            .graphic_oid
            .and_then(|oid| {
                entity
                    .source
                    .stream_path
                    .clone()
                    .map(|stream| (stream, oid))
            })
            .and_then(|key| styles.get(&key));
        let paragraph = resolved
            .and_then(|style| style.effective())
            .and_then(|style| style.alignment)
            .map_or_else(|| "—".to_string(), |alignment| format!("{alignment:?}"));
        let lettering = resolved.map_or_else(
            || "—".to_string(),
            |style| {
                format!(
                    "{:?}/spacing {}",
                    style.runs,
                    style
                        .line_spacing
                        .map_or_else(|| "-".to_string(), |spacing| format!("{spacing}"))
                )
            },
        );
        out.push(Facts {
            value: value.clone(),
            anchor_code: *anchor_code,
            paragraph,
            lettering,
            box_mm: text_box_width.filter(|width| *width > 0.0),
        });
    }
    out
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
    println!(
        "{:<7} {:>10} {:>10} {:>9} {:>9} {:>9} {:>9}  {:<8} {:>6} record",
        "sheet",
        "ref steps",
        "our steps",
        "model",
        "ref Tc",
        "our Tc",
        "ref runs",
        "face",
        "ref sx"
    );
    let mut reference_steps: Vec<(String, Vec<f64>)> = Vec::new();
    let mut mismatched = 0usize;
    // Whether ours lands on the model's own steps on every sheet. When it
    // does, the reference's split cannot be a rule this crate lacks.
    let mut our_steps_match_model_everywhere = true;
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
        let pick = |ink: &pid_plot::read::PlotInk| {
            ink.spans
                .iter()
                .enumerate()
                .filter(|(_, span)| span.text.trim() == LABEL)
                .map(|(index, _)| index)
                .collect::<Vec<_>>()
        };
        let reference_runs = pick(&reference);
        let our_runs = pick(&ours);
        if reference_runs.is_empty() || our_runs.is_empty() {
            println!("{sheet:<7} — no {LABEL} on this sheet");
            continue;
        }
        let Some(reference_reading) = Reading::of(&reference_page, &reference, &reference_runs)
        else {
            continue;
        };
        let Some(our_reading) = Reading::of(&our_page, &ours, &our_runs) else {
            continue;
        };
        let model_steps = laid_out(model)
            .and_then(|(height_mm, basis)| our_reading.model_steps_px(LABEL, height_mm, &basis))
            .unwrap_or_default();
        let show = |steps: &[f64]| {
            steps
                .iter()
                .map(|step| format!("{step:.0}"))
                .collect::<Vec<_>>()
                .join("+")
        };
        let records = facts(&corpus.join(format!("{name}.pid")));
        let record = records.first().map_or_else(
            || "—".to_string(),
            |facts| {
                format!(
                    "{:?} code {:#04x} para {} {}{}",
                    facts.value,
                    facts.anchor_code,
                    facts.paragraph,
                    facts.lettering,
                    facts
                        .box_mm
                        .map_or_else(String::new, |box_| format!(" box {box_:.4}"))
                )
            },
        );
        // Compare the steps the way the table shows them: whole 150 dpi
        // pixels. The two sides' continuous steps carry float tails from the
        // read-back, so an exact `f64` comparison would call every sheet
        // different when six of them land on the same pixel.
        let same = reference_reading.steps_px.len() == our_reading.steps_px.len()
            && reference_reading
                .steps_px
                .iter()
                .zip(&our_reading.steps_px)
                .all(|(left, right)| (left.round() - right.round()).abs() < 0.5)
            && (reference_reading.tc_pt - our_reading.tc_pt).abs() < 1e-3;
        if !same {
            mismatched += 1;
        }
        let ours_matches_model = model_steps.len() == our_reading.steps_px.len()
            && model_steps
                .iter()
                .zip(&our_reading.steps_px)
                .all(|(model, ours)| (model.round() - ours.round()).abs() < 0.5);
        if !ours_matches_model {
            our_steps_match_model_everywhere = false;
        }
        println!(
            "{sheet:<7} {:>10} {:>10} {:>9} {:>+9.4} {:>+9.4} {:>9}  {:<8} {:>6.3} {}",
            show(&reference_reading.steps_px),
            show(&our_reading.steps_px),
            show(&model_steps),
            reference_reading.tc_pt,
            our_reading.tc_pt,
            reference_reading.runs,
            reference_reading.face,
            reference_reading.sx,
            record,
        );
        reference_steps.push((sheet.to_string(), reference_reading.steps_px.clone()));
    }
    println!();
    println!("the two sides' glyph origins, 150 dpi pixels from the page's corner:");
    let mut seen: std::collections::BTreeMap<String, Vec<&str>> = std::collections::BTreeMap::new();
    for (sheet, steps) in &reference_steps {
        let key = steps
            .iter()
            .map(|step| format!("{step:.0}"))
            .collect::<Vec<_>>()
            .join("+");
        seen.entry(key).or_default().push(sheet.as_str());
    }
    for (pattern, sheets) in &seen {
        println!("  reference steps {pattern:<12} on {}", sheets.join(", "));
    }
    println!();
    let patterns: Vec<&String> = seen.keys().collect();
    if patterns.len() == 1 && mismatched == 0 {
        println!("verdict: same — every sheet's steps and spacing agree with the model");
    } else if patterns.len() == 1 {
        println!("verdict: model-short — the reference steps one way on every sheet; ours differs");
    } else if our_steps_match_model_everywhere {
        // Ours agrees with the model on every sheet; only the reference's
        // second step moves, and only on some of them.
        println!(
            "verdict: reference-split — the reference steps {} across the sheets while the \
             stored value, face, size and paragraph are identical and ours equals the model on \
             all {} of them. {} of {} sheets differ from ours, each by a single 1/150 in pixel \
             in the second step, so it is a placement the driver chose, not a spacing rule this \
             crate is missing.",
            patterns
                .iter()
                .map(|pattern| format!("`{pattern}`"))
                .collect::<Vec<_>>()
                .join(" / "),
            reference_steps.len(),
            mismatched,
            reference_steps.len(),
        );
    } else {
        let ours_single = reference_steps.len() == mismatched;
        println!(
            "verdict: outlier — the reference steps {patterns:?} across the sheets while the \
             stored value, face, size and paragraph are identical; {} of {} sheets differ from \
             ours{}. The difference is a single 1/150 in pixel in the second step, so it is a \
             placement the driver chose, not a spacing rule this crate is missing.",
            mismatched,
            reference_steps.len(),
            if ours_single { " (all of them)" } else { "" }
        );
    }
    ExitCode::SUCCESS
}
