//! Compare a reference plot with one this crate wrote.
//!
//! Both sides are joined into centerlines where collinear gaps are under 3 mm.
//! A uniform scale and a translation are fit from long lines. Coverage samples
//! the reference every 0.5 mm; a sample counts when our nearest segment is
//! within 0.5 mm. Text matches after whitespace is collapsed, within 1 mm.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::model::{PaperModel, PaperPrimitive, PaperShape, TextAnchor, TextLine};
use crate::read::{InkSegment, PlotInk, TextSpan};

const JOIN_GAP_MM: f64 = 3.0;
const COVER_MM: f64 = 0.5;
const SAMPLE_MM: f64 = 0.5;
const TEXT_MM: f64 = 1.0;
const SOURCE_MM: f64 = 20.0;
const ATTRIB_MM: f64 = 15.0;
const LONG_MM: f64 = 8.0;
const SCALE_LIMIT: f64 = 0.002;
const SHIFT_LIMIT_MM: f64 = 5.0;

/// How far apart two runs of one label may be, in body heights.
///
/// A PDF splits a label into runs at a font change and at some kerning
/// pairs, so one side of a comparison can spell `2"NC` as one run and the
/// other as `2"` + `NC`. The margin is measured from the end of the first
/// run's advance to the origin of the next and is symmetric, because a
/// split run can also overlap the previous one: `2-1/2"` + `LO` on the B
/// drawing starts 1.43 mm early against a 3.59 mm body, 0.40 heights. The
/// bound is deliberately loose: a joined candidate still has to spell the
/// other side's text exactly, so a wrong merge costs a match but never
/// invents one.
const RUN_GAP_HEIGHTS: f64 = 0.6;
/// How far a run of one label may sit off the writing line, in heights.
const RUN_ACROSS_HEIGHTS: f64 = 0.35;
/// Most runs one printed label has been seen split into.
const RUNS_PER_LABEL: usize = 4;

/// Why a comparison could not be written out.
#[derive(Debug)]
pub enum CompareError {
    /// The page has no area.
    EmptyPage,
    /// The overlay or the report could not be written.
    Io(String),
}

impl std::fmt::Display for CompareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPage => write!(f, "the page has no area"),
            Self::Io(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CompareError {}

/// A rectangle that is reference-data-missing, not a content error.
#[derive(Clone, Debug, PartialEq)]
pub struct MaskRect {
    /// What the rectangle stands for.
    pub label: String,
    /// Inclusive minimum, paper millimetres.
    pub min_mm: [f64; 2],
    /// Inclusive maximum, paper millimetres.
    pub max_mm: [f64; 2],
}

impl MaskRect {
    /// Whether `point` lies inside the rectangle.
    #[must_use]
    pub fn contains(&self, point: [f64; 2]) -> bool {
        point[0] >= self.min_mm[0]
            && point[0] <= self.max_mm[0]
            && point[1] >= self.min_mm[1]
            && point[1] <= self.max_mm[1]
    }
}

/// Title-block rectangles surveyed from the five reference plots.
///
/// A0 is the I井场 sheet. A1 is one set shared by the other four. The
/// rectangles are `assets/template-masks.json`: millimetres, origin at the
/// bottom-left, y up. A page that matches neither sheet size gets an empty
/// list. How the clusters were measured is written up in
/// `docs/analysis/2026-10-12-template-mask-samples.md`.
#[must_use]
pub fn template_masks(page_mm: [f64; 2]) -> Vec<MaskRect> {
    let Some(sheet) = surveyed_sheet(page_mm) else {
        return Vec::new();
    };
    sheet
        .masks
        .iter()
        .map(|mask| MaskRect {
            label: mask.note.clone(),
            min_mm: [mask.x, mask.y],
            max_mm: [mask.x + mask.w, mask.y + mask.h],
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct MaskAsset {
    sheets: HashMap<String, SheetMasks>,
}

#[derive(Debug, Deserialize)]
struct SheetMasks {
    width_mm: f64,
    height_mm: f64,
    masks: Vec<JsonMask>,
}

#[derive(Debug, Deserialize)]
struct JsonMask {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    note: String,
}

fn mask_asset() -> &'static MaskAsset {
    static ASSET: OnceLock<MaskAsset> = OnceLock::new();
    ASSET.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/template-masks.json"))
            .expect("pid-plot/assets/template-masks.json is valid")
    })
}

fn surveyed_sheet(page_mm: [f64; 2]) -> Option<&'static SheetMasks> {
    const TOLERANCE_MM: f64 = 2.0;
    mask_asset().sheets.values().find(|sheet| {
        (sheet.width_mm - page_mm[0]).abs() <= TOLERANCE_MM
            && (sheet.height_mm - page_mm[1]).abs() <= TOLERANCE_MM
    })
}

/// Scale and translation taking reference millimetres into our plot.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Fit {
    /// `our ≈ scale * reference + translation`.
    pub scale: f64,
    /// Translation in millimetres, our space.
    pub translation_mm: [f64; 2],
    /// Long-line pairs used.
    pub pairs: usize,
    /// Scale within 0.2% of 1 and shift within 5 mm.
    pub within_threshold: bool,
    /// Set when the fit is outside the threshold or could not be solved.
    pub finding: Option<String>,
}

/// One text run that did not pair.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UnmatchedText {
    /// Whitespace-collapsed characters.
    pub text: String,
    /// Baseline start, paper millimetres.
    pub origin_mm: [f64; 2],
    /// Stream the paper-table record came from, when a source matched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_path: Option<String>,
    /// Byte range in that stream, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_range: Option<[usize; 2]>,
    /// Parser confidence of the paper-table record.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,
    /// Symbol file this lettering belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

/// Paper-table lettering that can be copied onto an unmatched run.
#[derive(Clone, Debug, PartialEq)]
pub struct TextSource {
    /// Characters as the paper table stored them.
    pub text: String,
    /// Insertion point, our paper millimetres.
    pub origin_mm: [f64; 2],
    /// Stream the record came from.
    pub stream_path: Option<String>,
    /// Byte range in that stream.
    pub byte_range: Option<[usize; 2]>,
    /// Parser confidence.
    pub confidence: String,
    /// Symbol file, when the run came from a symbol body.
    pub symbol: Option<String>,
    /// Which side of its insertion point the label letters from.
    pub anchor: TextAnchor,
    /// Which line of its insertion point the label letters from.
    pub line: TextLine,
}

/// How far our runs sit from the reference's, per lettering class.
///
/// A class whose mean is not zero is a systematic placement error that the
/// 1 mm pairing window hides: the I drawing's top-anchored labels sat 0.32
/// body heights high for two rounds, and every one of them still paired,
/// because 0.79 mm is inside the window. This report makes that number
/// visible while it is still small.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TextOffsetClass {
    /// The lettering rule the run obeyed: `Left/Middle`,
    /// `symbol/Ascender`, or `unknown` when no paper-table source was within
    /// reach of the run.
    pub class: String,
    /// Labels paired in this class.
    pub count: usize,
    /// Mean shift along the reference run's writing direction, in body
    /// heights. Positive overshoots the run to the right.
    pub mean_along_heights: f64,
    /// Mean shift toward the reference run's descenders, in body heights.
    /// Positive is a line too low.
    pub mean_down_heights: f64,
    /// Mean difference in run width, in body heights: ours minus the
    /// reference's. Positive means our face sets the same label wider.
    ///
    /// A centred anchor steps back by half its own width and a right anchor
    /// by all of it, so a run set wider than the reference's starts further
    /// left with the rule itself obeyed — half of this figure lands in
    /// [`Self::mean_along_heights`] for those two classes. Left-anchored runs
    /// step back by nothing and are unaffected.
    pub mean_advance_heights: f64,
    /// Largest shift in the class, millimetres.
    pub max_shift_mm: f64,
}

/// A length of ink sharing one label.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LengthGroup {
    /// Symbol file name, a stream path, `图纸线`, or `图框`.
    pub label: String,
    /// Millimetres in this group.
    pub length_mm: f64,
}

/// The three ratchet numbers, plus the fit and the text that missed.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompareReport {
    /// Reference page, millimetres.
    pub page_mm: [f64; 2],
    /// How our plot sits on the reference.
    pub fit: Fit,
    /// Reference centerline length outside the masks.
    pub reference_length_mm: f64,
    /// Of that length, the part within 0.5 mm of our ink.
    pub covered_length_mm: f64,
    /// Covered length over reference length. 0 when the reference length is 0.
    pub coverage: f64,
    /// Our centerline length outside the masks and farther than 0.5 mm from the reference.
    pub extra_length_mm: f64,
    /// Reference text runs outside the masks, blanks excluded.
    ///
    /// A run that normalizes to nothing — glyphs of spaces, which both plots
    /// carry — names no label, so it is counted on neither side.
    pub text_reference: usize,
    /// Our text runs outside the masks, blanks excluded.
    pub text_ours: usize,
    /// Runs that matched. Two blank runs never pair: blanks are not counted.
    pub text_matched: usize,
    /// Reference length inside a mask.
    pub masked_reference_length_mm: f64,
    /// Text runs inside a mask.
    pub masked_text: usize,
    /// Reference runs that did not match.
    pub unmatched_reference_text: Vec<UnmatchedText>,
    /// Our runs that did not match.
    pub unmatched_our_text: Vec<UnmatchedText>,
    /// Where the paired runs sit, per lettering class.
    ///
    /// Empty when no run paired. A class's mean is the systematic part of
    /// the placement error; the residuals above are only the part that fell
    /// outside the pairing window.
    #[serde(default)]
    pub text_offsets: Vec<TextOffsetClass>,
    /// Our ink far from the reference and inside a missing-symbol box.
    ///
    /// Zero unless [`compare_plots_with_model`] supplied the boxes.
    pub extra_inside_missing_mm: f64,
    /// Our ink far from the reference, outside the template and the missing-symbol boxes.
    pub extra_outside_missing_mm: f64,
    /// `extra_outside_missing_mm` grouped by the nearest paper-table stroke.
    pub extra_outside_by_symbol: Vec<LengthGroup>,
    /// Reference length outside the masks and farther than 0.5 mm from our ink.
    pub uncovered_reference: Vec<LengthGroup>,
}

/// Compare `reference` with `ours`. Masks are excluded from the three numbers.
#[must_use]
pub fn compare_plots(reference: &PlotInk, ours: &PlotInk, masks: &[MaskRect]) -> CompareReport {
    compare_plots_with_sources(reference, ours, masks, &[])
}

/// Compare, and copy paper-table provenance onto unmatched runs of ours.
///
/// A source matches when the collapsed text is the same and the insertion
/// point is within 20 mm of the run, after the fit. Centre and right
/// paragraphs store the insertion point, while the PDF reader reports the
/// left end of the baseline, so the window is wider than the 1 mm text match.
#[must_use]
pub fn compare_plots_with_sources(
    reference: &PlotInk,
    ours: &PlotInk,
    masks: &[MaskRect],
    sources: &[TextSource],
) -> CompareReport {
    let reference_lines = join_centerlines(&reference.segments);
    let our_lines = join_centerlines(&ours.segments);
    let fit = fit_lines(&reference_lines, &our_lines);
    let our_in_ref: Vec<Line> = our_lines
        .iter()
        .map(|line| line.mapped(|point| into_reference(point, &fit)))
        .collect();
    let reference_index = Grid::new(&reference_lines);
    let our_index = Grid::new(&our_in_ref);
    let (reference_length, covered, masked_reference) =
        cover(&reference_lines, &our_index, masks, true);
    let (_, extra, _) = cover(&our_in_ref, &reference_index, masks, false);
    let text = match_text(&reference.spans, &ours.spans, &fit, masks, sources);
    let coverage = if reference_length > 0.0 {
        covered / reference_length
    } else {
        0.0
    };
    CompareReport {
        page_mm: reference.page_mm,
        fit,
        reference_length_mm: reference_length,
        covered_length_mm: covered,
        coverage,
        extra_length_mm: extra,
        text_reference: text.reference,
        text_ours: text.ours,
        text_matched: text.matched,
        masked_reference_length_mm: masked_reference,
        masked_text: text.masked,
        unmatched_reference_text: text.unmatched_reference,
        unmatched_our_text: text.unmatched_ours,
        text_offsets: text.offsets,
        extra_inside_missing_mm: 0.0,
        extra_outside_missing_mm: 0.0,
        extra_outside_by_symbol: Vec::new(),
        uncovered_reference: Vec::new(),
    }
}

/// Compare one drawing whose paper table can name symbols and streams.
///
/// `template` and `missing` are both excluded from the three numbers, the
/// same way a single mask list is. The extra length is then split on
/// `missing` alone, and uncovered reference ink is grouped by the nearest
/// paper-table stroke.
#[must_use]
pub fn compare_plots_with_model(
    reference: &PlotInk,
    ours: &PlotInk,
    template: &[MaskRect],
    missing: &[MaskRect],
    model: &PaperModel,
) -> CompareReport {
    let mut masks = template.to_vec();
    masks.extend_from_slice(missing);
    let mut report = compare_plots_with_sources(reference, ours, &masks, &text_sources(model));
    let grouped = group_lengths(reference, ours, &report.fit, template, missing, model);
    report.extra_inside_missing_mm = grouped.extra_inside_missing_mm;
    report.extra_outside_missing_mm = grouped.extra_outside_missing_mm;
    report.extra_outside_by_symbol = grouped.extra_outside_by_symbol;
    report.uncovered_reference = grouped.uncovered_reference;
    report
}

/// One symbol file's share of the extra ink.
#[derive(Clone, Debug, PartialEq)]
pub struct ExtraSymbol {
    /// Symbol file name, not the library path.
    pub name: String,
    /// Byte ranges of the strokes that contributed, up to a handful.
    pub byte_ranges: Vec<[usize; 2]>,
    /// Ranges past the ones kept in [`Self::byte_ranges`].
    pub omitted_ranges: usize,
    /// Millimetres of extra ink attributed to this symbol.
    pub length_mm: f64,
}

/// Extra centreline length split into furniture, symbol bodies, and the
/// sheet's own strokes.
///
/// The three lengths sum to [`Self::extra_mm`]. Points inside a template
/// mask or a missing-symbol box are not extra, same as
/// [`CompareReport::extra_length_mm`].
#[derive(Clone, Debug, PartialEq)]
pub struct ExtraInkPiles {
    /// Our centreline outside the masks and farther than 0.5 mm from the reference.
    pub extra_mm: f64,
    /// Of that, the part whose nearest paper stroke is border furniture.
    pub furniture_mm: f64,
    /// Symbol bodies, longest first.
    pub symbols: Vec<ExtraSymbol>,
    /// Sheet strokes that are neither furniture nor a symbol body.
    pub sheet_mm: f64,
}

/// Split the extra ink the comparison already measures.
///
/// Does not change what is drawn. A sample is furniture when the nearest
/// paper stroke within 15 mm is marked furniture; a symbol when that stroke
/// names one; otherwise it is the sheet's own line.
#[must_use]
pub fn extra_ink_piles(
    reference: &PlotInk,
    ours: &PlotInk,
    template: &[MaskRect],
    missing: &[MaskRect],
    model: &PaperModel,
) -> ExtraInkPiles {
    let reference_lines = join_centerlines(&reference.segments);
    let our_joined = join_centerlines(&ours.segments);
    let fit = fit_lines(&reference_lines, &our_joined);
    let our_lines: Vec<Line> = our_joined
        .into_iter()
        .map(|line| line.mapped(|point| into_reference(point, &fit)))
        .collect();
    let reference_index = Grid::new(&reference_lines);
    let mut tagged: Vec<(Line, StrokeKind)> = Vec::new();
    for_each_model_line(model, |line, primitive| {
        tagged.push((line, StrokeKind::of(primitive)));
    });
    let stroke_lines: Vec<Line> = tagged.iter().map(|(line, _)| *line).collect();
    let stroke_index = Grid::new(&stroke_lines);

    let mut furniture_mm = 0.0;
    let mut sheet_mm = 0.0;
    let mut symbols: HashMap<String, SymbolBucket> = HashMap::new();
    for line in &our_lines {
        let length = line.length();
        if length < 1e-6 {
            continue;
        }
        let pieces = ((length / SAMPLE_MM).round() as usize).max(1);
        let weight = length / pieces as f64;
        for piece in 0..pieces {
            let t = (piece as f64 + 0.5) / pieces as f64;
            let point = [
                line.a[0] + (line.b[0] - line.a[0]) * t,
                line.a[1] + (line.b[1] - line.a[1]) * t,
            ];
            if template.iter().any(|mask| mask.contains(point)) {
                continue;
            }
            let near = reference_index
                .near(point, COVER_MM)
                .into_iter()
                .any(|index| {
                    dist_point_seg(
                        point,
                        reference_index.lines[index].a,
                        reference_index.lines[index].b,
                    ) <= COVER_MM
                });
            if near || missing.iter().any(|mask| mask.contains(point)) {
                continue;
            }
            match nearest_kind(point, &fit, &stroke_index, &tagged) {
                StrokeKind::Furniture => furniture_mm += weight,
                StrokeKind::Sheet => sheet_mm += weight,
                StrokeKind::Symbol { name, byte_range } => {
                    let bucket = symbols.entry(name).or_default();
                    bucket.length_mm += weight;
                    if let Some(range) = byte_range {
                        if bucket.byte_ranges.contains(&range) {
                            continue;
                        }
                        if bucket.byte_ranges.len() < 4 {
                            bucket.byte_ranges.push(range);
                        } else {
                            bucket.omitted_ranges += 1;
                        }
                    }
                }
            }
        }
    }
    let mut symbols: Vec<ExtraSymbol> = symbols
        .into_iter()
        .map(|(name, bucket)| ExtraSymbol {
            name,
            byte_ranges: bucket.byte_ranges,
            omitted_ranges: bucket.omitted_ranges,
            length_mm: bucket.length_mm,
        })
        .collect();
    symbols.sort_by(|left, right| right.length_mm.total_cmp(&left.length_mm));
    let symbol_mm: f64 = symbols.iter().map(|symbol| symbol.length_mm).sum();
    ExtraInkPiles {
        extra_mm: furniture_mm + symbol_mm + sheet_mm,
        furniture_mm,
        symbols,
        sheet_mm,
    }
}

#[derive(Clone, Default)]
struct SymbolBucket {
    length_mm: f64,
    byte_ranges: Vec<[usize; 2]>,
    omitted_ranges: usize,
}

#[derive(Clone)]
enum StrokeKind {
    Furniture,
    Sheet,
    Symbol {
        name: String,
        byte_range: Option<[usize; 2]>,
    },
}

impl StrokeKind {
    fn of(primitive: &PaperPrimitive) -> Self {
        if primitive.furniture {
            return Self::Furniture;
        }
        if let Some(symbol) = &primitive.symbol {
            let name = symbol
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or(symbol)
                .to_string();
            return Self::Symbol {
                name,
                byte_range: primitive.byte_range,
            };
        }
        Self::Sheet
    }
}

fn nearest_kind(
    point_in_ref: [f64; 2],
    fit: &Fit,
    stroke_index: &Grid,
    strokes: &[(Line, StrokeKind)],
) -> StrokeKind {
    let point = [
        fit.scale * point_in_ref[0] + fit.translation_mm[0],
        fit.scale * point_in_ref[1] + fit.translation_mm[1],
    ];
    let mut best: Option<(f64, usize)> = None;
    for index in stroke_index.near(point, ATTRIB_MM) {
        let line = strokes[index].0;
        let distance = dist_point_seg(point, line.a, line.b);
        if distance <= ATTRIB_MM && best.is_none_or(|(best_distance, _)| distance < best_distance) {
            best = Some((distance, index));
        }
    }
    best.map(|(_, index)| strokes[index].1.clone())
        .unwrap_or(StrokeKind::Sheet)
}

/// Lettering on `model`, in drawing order, including the fields a report can copy.
#[must_use]
pub fn text_sources(model: &PaperModel) -> Vec<TextSource> {
    model
        .primitives
        .iter()
        .filter_map(|primitive| {
            let PaperShape::Text {
                origin_mm,
                value,
                anchor,
                line,
                ..
            } = &primitive.shape
            else {
                return None;
            };
            if value.is_empty() {
                return None;
            }
            Some(TextSource {
                text: value.clone(),
                origin_mm: *origin_mm,
                stream_path: primitive.stream_path.clone(),
                byte_range: primitive.byte_range,
                confidence: primitive.confidence.to_string(),
                symbol: primitive.symbol.clone(),
                anchor: *anchor,
                line: *line,
            })
        })
        .collect()
}

/// Write the report as pretty JSON.
pub fn write_report(path: &Path, report: &CompareReport) -> Result<(), CompareError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|error| CompareError::Io(error.to_string()))?;
    }
    let json = serde_json::to_string_pretty(report)
        .map_err(|error| CompareError::Io(error.to_string()))?;
    std::fs::write(path, json).map_err(|error| CompareError::Io(error.to_string()))
}

/// Write a 1 px/mm overlay. Reference ink is blue, ours is red, overlap is purple.
pub fn write_overlay(
    path: &Path,
    reference: &PlotInk,
    ours: &PlotInk,
    fit: &Fit,
) -> Result<(), CompareError> {
    let width = reference.page_mm[0].round().max(1.0) as u32;
    let height = reference.page_mm[1].round().max(1.0) as u32;
    if width > 20_000 || height > 20_000 {
        return Err(CompareError::EmptyPage);
    }
    let mut pixels = vec![0u8; (width as usize) * (height as usize) * 3];
    paint(
        &mut pixels,
        width,
        height,
        &join_centerlines(&reference.segments),
        [30, 80, 210],
    );
    let our_lines = join_centerlines(&ours.segments)
        .into_iter()
        .map(|line| line.mapped(|point| into_reference(point, fit)))
        .collect::<Vec<_>>();
    paint(&mut pixels, width, height, &our_lines, [210, 50, 40]);
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|error| CompareError::Io(error.to_string()))?;
    }
    let file = std::fs::File::create(path).map_err(|error| CompareError::Io(error.to_string()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|error| CompareError::Io(error.to_string()))?
        .write_image_data(&pixels)
        .map_err(|error| CompareError::Io(error.to_string()))
}

#[derive(Clone, Copy)]
struct Line {
    a: [f64; 2],
    b: [f64; 2],
}

impl Line {
    fn length(self) -> f64 {
        hypot(self.a, self.b)
    }

    fn direction(self) -> [f64; 2] {
        let length = self.length();
        if length < 1e-9 {
            [1.0, 0.0]
        } else {
            [
                (self.b[0] - self.a[0]) / length,
                (self.b[1] - self.a[1]) / length,
            ]
        }
    }

    fn midpoint(self) -> [f64; 2] {
        [(self.a[0] + self.b[0]) * 0.5, (self.a[1] + self.b[1]) * 0.5]
    }

    fn mapped(self, mut map: impl FnMut([f64; 2]) -> [f64; 2]) -> Self {
        Self {
            a: map(self.a),
            b: map(self.b),
        }
    }
}

fn join_centerlines(segments: &[InkSegment]) -> Vec<Line> {
    let lines: Vec<Line> = segments
        .iter()
        .filter(|segment| hypot(segment.start_mm, segment.end_mm) >= 1e-4)
        .map(|segment| Line {
            a: segment.start_mm,
            b: segment.end_mm,
        })
        .collect();
    if lines.is_empty() {
        return lines;
    }
    let mut union = UnionFind::new(lines.len());
    let grid = endpoint_grid(&lines);
    for (index, line) in lines.iter().enumerate() {
        for end in [line.a, line.b] {
            for other in grid.near(end, JOIN_GAP_MM) {
                if other <= index {
                    continue;
                }
                if can_join(*line, lines[other]) {
                    union.join(index, other);
                }
            }
        }
    }
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for index in 0..lines.len() {
        groups.entry(union.find(index)).or_default().push(index);
    }
    groups
        .into_values()
        .filter_map(|members| merge_group(&lines, &members))
        .collect()
}

fn can_join(left: Line, right: Line) -> bool {
    let direction = left.direction();
    let other = right.direction();
    let dot = (direction[0] * other[0] + direction[1] * other[1]).abs();
    if dot < 0.999 {
        return false;
    }
    let perp = |point: [f64; 2]| {
        let vx = point[0] - left.a[0];
        let vy = point[1] - left.a[1];
        (vx * -direction[1] + vy * direction[0]).abs()
    };
    if perp(right.a) > 0.4 || perp(right.b) > 0.4 {
        return false;
    }
    let project = |point: [f64; 2]| {
        (point[0] - left.a[0]) * direction[0] + (point[1] - left.a[1]) * direction[1]
    };
    let left_end = left.length();
    let right_min = project(right.a).min(project(right.b));
    let right_max = project(right.a).max(project(right.b));
    let gap = if right_max < 0.0 {
        -right_max
    } else if left_end < right_min {
        right_min - left_end
    } else {
        0.0
    };
    gap <= JOIN_GAP_MM && ends_meet(left, right)
}

fn ends_meet(left: Line, right: Line) -> bool {
    [left.a, left.b].into_iter().any(|end| {
        [right.a, right.b]
            .into_iter()
            .any(|other| hypot(end, other) <= JOIN_GAP_MM)
    })
}

fn merge_group(lines: &[Line], members: &[usize]) -> Option<Line> {
    let first = lines[*members.first()?];
    let direction = first.direction();
    let mut min_t = 0.0;
    let mut max_t = 0.0;
    let mut min_point = first.a;
    let mut max_point = first.a;
    for member in members {
        for point in [lines[*member].a, lines[*member].b] {
            let t = (point[0] - first.a[0]) * direction[0] + (point[1] - first.a[1]) * direction[1];
            if t < min_t {
                min_t = t;
                min_point = point;
            }
            if t > max_t {
                max_t = t;
                max_point = point;
            }
        }
    }
    (hypot(min_point, max_point) >= 1e-4).then_some(Line {
        a: min_point,
        b: max_point,
    })
}

fn fit_lines(reference: &[Line], ours: &[Line]) -> Fit {
    let our_grid = Grid::new(ours);
    let mut pairs = Vec::new();
    for line in reference.iter().filter(|line| line.length() >= LONG_MM) {
        let Some(other) = nearest_aligned(*line, &our_grid, 50.0) else {
            continue;
        };
        pairs.push((line.midpoint(), other.midpoint()));
    }
    if pairs.len() < 4 {
        return Fit {
            scale: 1.0,
            translation_mm: [0.0, 0.0],
            pairs: pairs.len(),
            within_threshold: true,
            finding: Some(format!(
                "only {} long-line pairs, fit left at identity",
                pairs.len()
            )),
        };
    }
    let fit = solve_fit(&pairs).unwrap_or(Fit {
        scale: 1.0,
        translation_mm: [0.0, 0.0],
        pairs: pairs.len(),
        within_threshold: true,
        finding: Some("the long-line fit did not solve".to_string()),
    });
    let kept: Vec<_> = pairs
        .into_iter()
        .filter(|(reference_point, our_point)| {
            let predicted = [
                fit.scale * reference_point[0] + fit.translation_mm[0],
                fit.scale * reference_point[1] + fit.translation_mm[1],
            ];
            hypot(predicted, *our_point) <= 3.0
        })
        .collect();
    if kept.len() >= 4 && kept.len() != fit.pairs {
        return solve_fit(&kept).unwrap_or(fit);
    }
    fit
}

fn solve_fit(pairs: &[([f64; 2], [f64; 2])]) -> Option<Fit> {
    let n = pairs.len() as f64;
    let mut sum_rr = 0.0;
    let mut sum_rx = 0.0;
    let mut sum_ry = 0.0;
    let mut sum_dot = 0.0;
    let mut sum_ox = 0.0;
    let mut sum_oy = 0.0;
    for (reference_point, our_point) in pairs {
        sum_rr += reference_point[0] * reference_point[0] + reference_point[1] * reference_point[1];
        sum_rx += reference_point[0];
        sum_ry += reference_point[1];
        sum_dot += reference_point[0] * our_point[0] + reference_point[1] * our_point[1];
        sum_ox += our_point[0];
        sum_oy += our_point[1];
    }
    let solution = solve3(
        [[sum_rr, sum_rx, sum_ry], [sum_rx, n, 0.0], [sum_ry, 0.0, n]],
        [sum_dot, sum_ox, sum_oy],
    )?;
    let scale = solution[0];
    let translation = [solution[1], solution[2]];
    if !scale.is_finite() || scale < 0.5 || scale > 1.5 {
        return None;
    }
    let shift = translation[0].hypot(translation[1]);
    let within = (scale - 1.0).abs() <= SCALE_LIMIT && shift <= SHIFT_LIMIT_MM;
    let finding = if within {
        None
    } else {
        Some(format!(
            "fit scale {scale:.6}, translation {:.2} {:.2} mm",
            translation[0], translation[1]
        ))
    };
    Some(Fit {
        scale,
        translation_mm: translation,
        pairs: pairs.len(),
        within_threshold: within,
        finding,
    })
}

fn solve3(mut a: [[f64; 3]; 3], mut b: [f64; 3]) -> Option<[f64; 3]> {
    for col in 0..3 {
        let mut pivot = col;
        for row in col + 1..3 {
            if a[row][col].abs() > a[pivot][col].abs() {
                pivot = row;
            }
        }
        if a[pivot][col].abs() < 1e-12 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        let diag = a[col][col];
        for row in col + 1..3 {
            let factor = a[row][col] / diag;
            for k in col..3 {
                a[row][k] -= factor * a[col][k];
            }
            b[row] -= factor * b[col];
        }
    }
    let mut x = [0.0; 3];
    for row in (0..3).rev() {
        let mut acc = b[row];
        for col in row + 1..3 {
            acc -= a[row][col] * x[col];
        }
        x[row] = acc / a[row][row];
    }
    x.iter().all(|value| value.is_finite()).then_some(x)
}

fn into_reference(point: [f64; 2], fit: &Fit) -> [f64; 2] {
    [
        (point[0] - fit.translation_mm[0]) / fit.scale,
        (point[1] - fit.translation_mm[1]) / fit.scale,
    ]
}

fn nearest_aligned(line: Line, grid: &Grid, limit: f64) -> Option<Line> {
    let want = line.direction();
    let mut best: Option<(f64, Line)> = None;
    for index in grid.near(line.midpoint(), limit) {
        let other = grid.lines[index];
        let dot = (want[0] * other.direction()[0] + want[1] * other.direction()[1]).abs();
        if dot < 0.99 || other.length() < LONG_MM {
            continue;
        }
        let distance = hypot(line.midpoint(), other.midpoint());
        if distance <= limit && best.is_none_or(|(best_distance, _)| distance < best_distance) {
            best = Some((distance, other));
        }
    }
    best.map(|(_, other)| other)
}

struct CoverText {
    reference: usize,
    ours: usize,
    matched: usize,
    masked: usize,
    unmatched_reference: Vec<UnmatchedText>,
    unmatched_ours: Vec<UnmatchedText>,
    offsets: Vec<TextOffsetClass>,
}

/// Running mean of the paired runs' shifts, one bucket per class.
#[derive(Default)]
struct OffsetBucket {
    count: usize,
    along: f64,
    down: f64,
    advance: f64,
    max_mm: f64,
}

/// Where one pair put our run, relative to the reference's.
///
/// The shift is projected onto the reference run's own writing frame — along
/// the glyphs and toward the descenders — so a rotated label reads the same
/// way as an upright one, and divided by the reference body size, because a
/// 0.3 mm miss on a 2.5 mm label and on a 5 mm label are not the same error.
///
/// The run's own advance is recorded beside it: a centred run steps back by
/// half *its* width, so a run set wider than the reference's starts to the
/// left of it by half that difference with no placement rule wrong at all.
/// Without the two numbers side by side the I drawing's leading-space padding
/// read as a 0.05-body centring error for two rounds.
fn record_offset(
    buckets: &mut HashMap<String, OffsetBucket>,
    reference: &TextSpan,
    ours: &TextSpan,
    fit: &Fit,
    sources: &[TextSource],
) {
    let height = reference.height_mm;
    if !height.is_finite() || height <= 0.1 {
        return;
    }
    let ours_in_ref = into_reference(ours.origin_mm, fit);
    let delta = [
        ours_in_ref[0] - reference.origin_mm[0],
        ours_in_ref[1] - reference.origin_mm[1],
    ];
    let direction = reference.direction;
    let along = delta[0] * direction[0] + delta[1] * direction[1];
    let down = delta[0] * direction[1] - delta[1] * direction[0];
    let class = offset_class(ours_in_ref, &ours.text, sources, fit);
    let bucket = buckets.entry(class).or_default();
    bucket.count += 1;
    bucket.along += along / height;
    bucket.down += down / height;
    bucket.advance += (ours.advance_mm - reference.advance_mm) / height;
    bucket.max_mm = bucket.max_mm.max(along.hypot(down));
}

/// Which lettering rule produced our run, as a stable label.
fn offset_class(origin: [f64; 2], text: &str, sources: &[TextSource], fit: &Fit) -> String {
    let Some(source) = find_source(origin, text, sources, fit) else {
        return "unknown".to_string();
    };
    match source.symbol {
        Some(_) => format!("symbol/{:?}", source.line),
        None => format!("{:?}/{:?}", source.anchor, source.line),
    }
}

/// Pair reference text runs with ours, and report what did not pair.
///
/// A run that normalizes to nothing — the glyphs of a run that is only
/// spaces, which both plots carry — is not a label and is left out of the
/// tally on either side. Counting those as misses made the I drawing report
/// six reference runs it had no hope of pairing: the residual list is the
/// hunting ground for real placement errors, and blanks crowd it out.
///
/// A label can be split into runs differently on the two sides: the I
/// drawing's `2"NC` is one run in the reference and `2"` + `NC` here, and
/// one instance goes the other way. After runs that spell the same text
/// pair up, a leftover reference run is tried against our runs that
/// continue it, and a leftover reference run starts a chain of its own that
/// might spell one of our runs. Both passes keep the existing unit: a
/// reference run is counted once, whether it paired alone or as part of a
/// joined label.
fn match_text(
    reference: &[TextSpan],
    ours: &[TextSpan],
    fit: &Fit,
    masks: &[MaskRect],
    sources: &[TextSource],
) -> CoverText {
    // Tally both sides first. Counting and masking do not depend on the
    // pairing, and the second pass below needs to know which runs are in
    // play before it looks for a joined label.
    let mut tallied: Vec<usize> = Vec::new();
    // `hit` doubles as the taken set for chains, so a blank or masked run
    // — never a label — cannot end up inside a joined one.
    let mut hit = vec![false; reference.len()];
    let mut masked = 0;
    for (index, span) in reference.iter().enumerate() {
        if normalize(&span.text).is_empty() {
            hit[index] = true;
            continue;
        }
        if masks.iter().any(|mask| mask.contains(span.origin_mm)) {
            masked += 1;
            hit[index] = true;
            continue;
        }
        tallied.push(index);
    }
    let mut blocked = vec![false; ours.len()];
    let mut our_count = 0;
    for (index, span) in ours.iter().enumerate() {
        if normalize(&span.text).is_empty() {
            continue;
        }
        if masks
            .iter()
            .any(|mask| mask.contains(into_reference(span.origin_mm, fit)))
        {
            masked += 1;
            blocked[index] = true;
            continue;
        }
        our_count += 1;
    }
    // First pass: a run that spells the same text as the reference's run.
    let mut used = vec![false; ours.len()];
    let mut matched = 0;
    let mut buckets: HashMap<String, OffsetBucket> = HashMap::new();
    for index in tallied.iter().copied() {
        let wanted = normalize(&reference[index].text);
        let found = ours.iter().enumerate().find(|(slot, other)| {
            !used[*slot]
                && !blocked[*slot]
                && normalize(&other.text) == wanted
                && hypot(
                    reference[index].origin_mm,
                    into_reference(other.origin_mm, fit),
                ) <= TEXT_MM
        });
        if let Some((slot, _)) = found {
            used[slot] = true;
            hit[index] = true;
            matched += 1;
            record_offset(&mut buckets, &reference[index], &ours[slot], fit, sources);
        }
    }
    // Second pass: a label one side split into runs and the other did not.
    for index in tallied.iter().copied() {
        if hit[index] {
            continue;
        }
        let origin = reference[index].origin_mm;
        let wanted = normalize(&reference[index].text);
        if let Some(taken) = join_ours(&wanted, ours, &used, &blocked, origin, fit) {
            record_offset(
                &mut buckets,
                &reference[index],
                &ours[taken[0]],
                fit,
                sources,
            );
            for slot in taken {
                used[slot] = true;
            }
            hit[index] = true;
            matched += 1;
            continue;
        }
        let chain = run_chain(reference, index, &hit);
        for cuts in 2..=chain.len() {
            let texts = joined_text(reference, &chain, cuts);
            let found = ours.iter().enumerate().find(|(slot, other)| {
                !used[*slot]
                    && !blocked[*slot]
                    && texts.iter().any(|text| *text == normalize(&other.text))
                    && hypot(origin, into_reference(other.origin_mm, fit)) <= TEXT_MM
            });
            let Some((slot, _)) = found else {
                continue;
            };
            record_offset(&mut buckets, &reference[index], &ours[slot], fit, sources);
            used[slot] = true;
            for member in chain.iter().take(cuts) {
                hit[*member] = true;
            }
            matched += cuts;
            break;
        }
    }
    let mut unmatched_reference = Vec::new();
    for index in tallied.iter().copied() {
        if !hit[index] {
            unmatched_reference.push(unmatched(&reference[index]));
        }
    }
    let mut unmatched_ours = Vec::new();
    for (index, span) in ours.iter().enumerate() {
        if blocked[index] || used[index] || normalize(&span.text).is_empty() {
            continue;
        }
        let origin = into_reference(span.origin_mm, fit);
        let source = find_source(origin, &span.text, sources, fit);
        unmatched_ours.push(UnmatchedText {
            text: normalize(&span.text),
            origin_mm: origin,
            stream_path: source.and_then(|item| item.stream_path.clone()),
            byte_range: source.and_then(|item| item.byte_range),
            confidence: source.map(|item| item.confidence.clone()),
            symbol: source.and_then(|item| item.symbol.clone()),
        });
    }
    let mut offsets: Vec<TextOffsetClass> = buckets
        .into_iter()
        .map(|(class, bucket)| TextOffsetClass {
            class,
            count: bucket.count,
            mean_along_heights: bucket.along / bucket.count as f64,
            mean_down_heights: bucket.down / bucket.count as f64,
            mean_advance_heights: bucket.advance / bucket.count as f64,
            max_shift_mm: bucket.max_mm,
        })
        .collect();
    offsets.sort_by(|left, right| left.class.cmp(&right.class));
    CoverText {
        reference: tallied.len(),
        ours: our_count,
        matched,
        masked,
        unmatched_reference,
        unmatched_ours,
        offsets,
    }
}

/// Where a run's ink ends, along its writing direction.
fn end_of(span: &TextSpan) -> [f64; 2] {
    [
        span.origin_mm[0] + span.direction[0] * span.advance_mm,
        span.origin_mm[1] + span.direction[1] * span.advance_mm,
    ]
}

/// The runs that continue `start` on the same writing line, `start`
/// included, ordered by reading order.
///
/// A blank run is stepped over rather than joined: it inks nothing, so it
/// spells nothing. Runs already `taken` are never pulled in.
fn run_chain(spans: &[TextSpan], start: usize, taken: &[bool]) -> Vec<usize> {
    let first = &spans[start];
    let mut chain = vec![start];
    let mut cursor = end_of(first);
    while chain.len() < RUNS_PER_LABEL {
        let mut best: Option<(f64, usize)> = None;
        for (index, span) in spans.iter().enumerate() {
            if index == start || taken[index] || chain.contains(&index) {
                continue;
            }
            if normalize(&span.text).is_empty() {
                continue;
            }
            let dot =
                first.direction[0] * span.direction[0] + first.direction[1] * span.direction[1];
            if dot < 0.99 {
                continue;
            }
            let delta = [span.origin_mm[0] - cursor[0], span.origin_mm[1] - cursor[1]];
            let along = delta[0] * first.direction[0] + delta[1] * first.direction[1];
            let across = delta[1] * first.direction[0] - delta[0] * first.direction[1];
            let height = first.height_mm.max(span.height_mm).max(0.5);
            if across.abs() > RUN_ACROSS_HEIGHTS * height {
                continue;
            }
            if along.abs() > RUN_GAP_HEIGHTS * height {
                continue;
            }
            if best.is_none_or(|(best_gap, _)| along.abs() < best_gap) {
                best = Some((along.abs(), index));
            }
        }
        let Some((_, index)) = best else {
            break;
        };
        chain.push(index);
        cursor = end_of(&spans[index]);
    }
    chain
}

/// What the first `cuts` runs of `chain` spell, both fused and with one
/// space where they meet. A run split at a kerning pair fuses; a run split
/// at a word break does not, and the reference may spell either way.
///
/// Each member is trimmed first. A reference field pads its run — the L
/// drawing's two `0103501A` tags are read back as `␣0103501A` — and that pad
/// is not part of the label any more than ours is: left in, it fuses into an
/// interior space and the fused spelling misses by exactly the pad. That
/// cost the L drawing both tags for two rounds, on a pair the drawing places
/// where the reference does.
fn joined_text(spans: &[TextSpan], chain: &[usize], cuts: usize) -> Vec<String> {
    let mut fused = String::new();
    let mut spaced = String::new();
    for (position, index) in chain.iter().take(cuts).enumerate() {
        let text = spans[*index].text.trim();
        if position > 0 {
            spaced.push(' ');
        }
        fused.push_str(text);
        spaced.push_str(text);
    }
    let fused = normalize(&fused);
    let spaced = normalize(&spaced);
    if fused == spaced {
        vec![fused]
    } else {
        vec![fused, spaced]
    }
}

/// Pair one reference run with our run that starts at the same place plus
/// the runs continuing it. Returns the slots to mark used.
fn join_ours(
    wanted: &str,
    ours: &[TextSpan],
    used: &[bool],
    blocked: &[bool],
    origin: [f64; 2],
    fit: &Fit,
) -> Option<Vec<usize>> {
    // A masked run is not a label either, so a chain must walk around it the
    // same way it walks around one already paired.
    let taken: Vec<bool> = used
        .iter()
        .zip(blocked)
        .map(|(used, blocked)| *used || *blocked)
        .collect();
    for (start, span) in ours.iter().enumerate() {
        if taken[start] || normalize(&span.text).is_empty() {
            continue;
        }
        if hypot(origin, into_reference(span.origin_mm, fit)) > TEXT_MM {
            continue;
        }
        let chain = run_chain(ours, start, &taken);
        for cuts in 2..=chain.len() {
            if joined_text(ours, &chain, cuts)
                .iter()
                .any(|text| text == wanted)
            {
                return Some(chain[..cuts].to_vec());
            }
        }
    }
    None
}

fn unmatched(span: &TextSpan) -> UnmatchedText {
    UnmatchedText {
        text: normalize(&span.text),
        origin_mm: span.origin_mm,
        stream_path: None,
        byte_range: None,
        confidence: None,
        symbol: None,
    }
}

fn find_source<'a>(
    origin: [f64; 2],
    text: &str,
    sources: &'a [TextSource],
    fit: &Fit,
) -> Option<&'a TextSource> {
    let wanted = normalize(text);
    let mut best: Option<(f64, &TextSource)> = None;
    for source in sources {
        if normalize(&source.text) != wanted {
            continue;
        }
        let distance = hypot(origin, into_reference(source.origin_mm, fit));
        if distance <= SOURCE_MM && best.is_none_or(|(best_distance, _)| distance < best_distance) {
            best = Some((distance, source));
        }
    }
    best.map(|(_, source)| source)
}

fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn cover(
    lines: &[Line],
    other: &Grid,
    masks: &[MaskRect],
    reference_side: bool,
) -> (f64, f64, f64) {
    let mut open = 0.0;
    let mut hit = 0.0;
    let mut masked = 0.0;
    for line in lines {
        let length = line.length();
        if length < 1e-6 {
            continue;
        }
        let pieces = ((length / SAMPLE_MM).round() as usize).max(1);
        let weight = length / pieces as f64;
        for piece in 0..pieces {
            let t = (piece as f64 + 0.5) / pieces as f64;
            let point = [
                line.a[0] + (line.b[0] - line.a[0]) * t,
                line.a[1] + (line.b[1] - line.a[1]) * t,
            ];
            if masks.iter().any(|mask| mask.contains(point)) {
                if reference_side {
                    masked += weight;
                }
                continue;
            }
            open += weight;
            let near = other.near(point, COVER_MM).into_iter().any(|index| {
                dist_point_seg(point, other.lines[index].a, other.lines[index].b) <= COVER_MM
            });
            if near {
                hit += weight;
            }
        }
    }
    if reference_side {
        (open, hit, masked)
    } else {
        (open, open - hit, masked)
    }
}

fn paint(pixels: &mut [u8], width: u32, height: u32, lines: &[Line], color: [u8; 3]) {
    for line in lines {
        draw_line(pixels, width, height, line.a, line.b, color);
    }
}

fn draw_line(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    start: [f64; 2],
    end: [f64; 2],
    color: [u8; 3],
) {
    let mut x0 = start[0].round() as i32;
    let mut y0 = height as i32 - 1 - start[1].round() as i32;
    let x1 = end[0].round() as i32;
    let y1 = height as i32 - 1 - end[1].round() as i32;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        if x0 >= 0 && y0 >= 0 && (x0 as u32) < width && (y0 as u32) < height {
            let offset = (y0 as usize * width as usize + x0 as usize) * 3;
            let pixel = &mut pixels[offset..offset + 3];
            if pixel.iter().all(|channel| *channel == 0) {
                pixel.copy_from_slice(&color);
            } else {
                pixel.copy_from_slice(&[140, 40, 170]);
            }
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn dist_point_seg(point: [f64; 2], start: [f64; 2], end: [f64; 2]) -> f64 {
    let ab = [end[0] - start[0], end[1] - start[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    if len2 < 1e-12 {
        return hypot(point, start);
    }
    let t =
        (((point[0] - start[0]) * ab[0] + (point[1] - start[1]) * ab[1]) / len2).clamp(0.0, 1.0);
    hypot(point, [start[0] + ab[0] * t, start[1] + ab[1] * t])
}

fn hypot(start: [f64; 2], end: [f64; 2]) -> f64 {
    (start[0] - end[0]).hypot(start[1] - end[1])
}

struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
        }
    }

    fn find(&mut self, index: usize) -> usize {
        let parent = self.parent[index];
        if parent == index {
            index
        } else {
            let root = self.find(parent);
            self.parent[index] = root;
            root
        }
    }

    fn join(&mut self, left: usize, right: usize) {
        let left = self.find(left);
        let right = self.find(right);
        if left != right {
            self.parent[right] = left;
        }
    }
}

struct Grid {
    cell: f64,
    lines: Vec<Line>,
    buckets: HashMap<(i32, i32), Vec<usize>>,
}

impl Grid {
    fn new(lines: &[Line]) -> Self {
        let mut grid = Self {
            cell: 5.0,
            lines: lines.to_vec(),
            buckets: HashMap::new(),
        };
        for (index, line) in lines.iter().enumerate() {
            grid.insert(index, *line);
        }
        grid
    }

    fn insert(&mut self, index: usize, line: Line) {
        for key in cells_of(line, self.cell) {
            self.buckets.entry(key).or_default().push(index);
        }
    }

    fn near(&self, point: [f64; 2], radius: f64) -> Vec<usize> {
        let key = cell_key(point, self.cell);
        let reach = (radius / self.cell).ceil() as i32 + 1;
        let mut hits = Vec::new();
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                if let Some(bucket) = self.buckets.get(&(key.0 + dx, key.1 + dy)) {
                    hits.extend(bucket.iter().copied());
                }
            }
        }
        hits.sort_unstable();
        hits.dedup();
        hits
    }
}

fn endpoint_grid(lines: &[Line]) -> Grid {
    Grid::new(lines)
}

fn cells_of(line: Line, cell: f64) -> Vec<(i32, i32)> {
    let mut keys = vec![cell_key(line.a, cell), cell_key(line.b, cell)];
    let steps = (line.length() / cell).ceil() as usize + 1;
    for step in 0..steps {
        let t = step as f64 / steps as f64;
        keys.push(cell_key(
            [
                line.a[0] + (line.b[0] - line.a[0]) * t,
                line.a[1] + (line.b[1] - line.a[1]) * t,
            ],
            cell,
        ));
    }
    keys.sort_unstable();
    keys.dedup();
    keys
}

fn cell_key(point: [f64; 2], cell: f64) -> (i32, i32) {
    (
        (point[0] / cell).floor() as i32,
        (point[1] / cell).floor() as i32,
    )
}

struct GroupedLengths {
    extra_inside_missing_mm: f64,
    extra_outside_missing_mm: f64,
    extra_outside_by_symbol: Vec<LengthGroup>,
    uncovered_reference: Vec<LengthGroup>,
}

fn group_lengths(
    reference: &PlotInk,
    ours: &PlotInk,
    fit: &Fit,
    template: &[MaskRect],
    missing: &[MaskRect],
    model: &PaperModel,
) -> GroupedLengths {
    let reference_lines = join_centerlines(&reference.segments);
    let our_lines = join_centerlines(&ours.segments)
        .into_iter()
        .map(|line| line.mapped(|point| into_reference(point, fit)))
        .collect::<Vec<_>>();
    let reference_index = Grid::new(&reference_lines);
    let our_index = Grid::new(&our_lines);
    let strokes = model_strokes(model);
    let stroke_lines: Vec<Line> = strokes.iter().map(|(line, _)| *line).collect();
    let stroke_index = Grid::new(&stroke_lines);
    let (extra_inside, extra_outside, mut extra_groups) = split_extra(
        &our_lines,
        &reference_index,
        template,
        missing,
        fit,
        &stroke_index,
        &strokes,
    );
    let mut uncovered = HashMap::<String, f64>::new();
    for line in &reference_lines {
        let length = line.length();
        if length < 1e-6 {
            continue;
        }
        let pieces = ((length / SAMPLE_MM).round() as usize).max(1);
        let weight = length / pieces as f64;
        for piece in 0..pieces {
            let t = (piece as f64 + 0.5) / pieces as f64;
            let point = [
                line.a[0] + (line.b[0] - line.a[0]) * t,
                line.a[1] + (line.b[1] - line.a[1]) * t,
            ];
            if template.iter().any(|mask| mask.contains(point))
                || missing.iter().any(|mask| mask.contains(point))
            {
                continue;
            }
            let near = our_index.near(point, COVER_MM).into_iter().any(|index| {
                dist_point_seg(point, our_index.lines[index].a, our_index.lines[index].b)
                    <= COVER_MM
            });
            if near {
                continue;
            }
            let label = stroke_label_at(point, fit, &stroke_index, &strokes, reference.page_mm);
            *uncovered.entry(label).or_default() += weight;
        }
    }
    extra_groups.sort_by(|left, right| right.length_mm.total_cmp(&left.length_mm));
    GroupedLengths {
        extra_inside_missing_mm: extra_inside,
        extra_outside_missing_mm: extra_outside,
        extra_outside_by_symbol: extra_groups,
        uncovered_reference: top_groups(uncovered),
    }
}

fn split_extra(
    our_lines: &[Line],
    reference_index: &Grid,
    template: &[MaskRect],
    missing: &[MaskRect],
    fit: &Fit,
    stroke_index: &Grid,
    strokes: &[(Line, String)],
) -> (f64, f64, Vec<LengthGroup>) {
    let mut inside = 0.0;
    let mut outside = HashMap::<String, f64>::new();
    for line in our_lines {
        let length = line.length();
        if length < 1e-6 {
            continue;
        }
        let pieces = ((length / SAMPLE_MM).round() as usize).max(1);
        let weight = length / pieces as f64;
        for piece in 0..pieces {
            let t = (piece as f64 + 0.5) / pieces as f64;
            let point = [
                line.a[0] + (line.b[0] - line.a[0]) * t,
                line.a[1] + (line.b[1] - line.a[1]) * t,
            ];
            if template.iter().any(|mask| mask.contains(point)) {
                continue;
            }
            let near = reference_index
                .near(point, COVER_MM)
                .into_iter()
                .any(|index| {
                    dist_point_seg(
                        point,
                        reference_index.lines[index].a,
                        reference_index.lines[index].b,
                    ) <= COVER_MM
                });
            if near {
                continue;
            }
            if missing.iter().any(|mask| mask.contains(point)) {
                inside += weight;
            } else {
                let label = stroke_label_at(point, fit, stroke_index, strokes, [0.0, 0.0]);
                *outside.entry(label).or_default() += weight;
            }
        }
    }
    let outside_mm = outside.values().sum();
    (inside, outside_mm, top_groups(outside))
}

fn top_groups(groups: HashMap<String, f64>) -> Vec<LengthGroup> {
    let mut groups: Vec<LengthGroup> = groups
        .into_iter()
        .filter(|(_, length)| *length >= 1.0)
        .map(|(label, length_mm)| LengthGroup { label, length_mm })
        .collect();
    groups.sort_by(|left, right| right.length_mm.total_cmp(&left.length_mm));
    groups.truncate(8);
    groups
}

fn for_each_model_line(model: &PaperModel, mut visit: impl FnMut(Line, &PaperPrimitive)) {
    for primitive in &model.primitives {
        match &primitive.shape {
            PaperShape::Line { start_mm, end_mm } => {
                visit(
                    Line {
                        a: *start_mm,
                        b: *end_mm,
                    },
                    primitive,
                );
            }
            PaperShape::Polyline { points_mm, closed } => {
                let mut points = points_mm.clone();
                if *closed && points.len() > 2 {
                    points.push(points[0]);
                }
                for pair in points.windows(2) {
                    visit(
                        Line {
                            a: pair[0],
                            b: pair[1],
                        },
                        primitive,
                    );
                }
            }
            PaperShape::Arc {
                center_mm,
                radius_mm,
                start_angle,
                end_angle,
                clockwise,
            } => {
                let points = crate::write::arc_chain(
                    *center_mm,
                    *radius_mm,
                    *start_angle,
                    *end_angle,
                    *clockwise,
                );
                for pair in points.windows(2) {
                    visit(
                        Line {
                            a: pair[0],
                            b: pair[1],
                        },
                        primitive,
                    );
                }
            }
            PaperShape::Circle {
                center_mm,
                radius_mm,
            } => {
                let points = crate::write::circle_outline(*center_mm, *radius_mm);
                for pair in points.windows(2) {
                    visit(
                        Line {
                            a: pair[0],
                            b: pair[1],
                        },
                        primitive,
                    );
                }
            }
            PaperShape::Fill { chain_mm, .. } => {
                let mut points = chain_mm.clone();
                if points.len() > 2 {
                    points.push(points[0]);
                }
                for pair in points.windows(2) {
                    visit(
                        Line {
                            a: pair[0],
                            b: pair[1],
                        },
                        primitive,
                    );
                }
            }
            _ => {}
        }
    }
}

fn model_strokes(model: &PaperModel) -> Vec<(Line, String)> {
    let mut strokes = Vec::new();
    for_each_model_line(model, |line, primitive| {
        strokes.push((line, stroke_name(primitive)));
    });
    strokes
}

fn stroke_name(primitive: &PaperPrimitive) -> String {
    if let Some(symbol) = &primitive.symbol {
        symbol
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(symbol)
            .to_string()
    } else {
        primitive
            .stream_path
            .clone()
            .unwrap_or_else(|| "图纸线".to_string())
    }
}

fn stroke_label_at(
    point_in_ref: [f64; 2],
    fit: &Fit,
    stroke_index: &Grid,
    strokes: &[(Line, String)],
    page_mm: [f64; 2],
) -> String {
    let point = [
        fit.scale * point_in_ref[0] + fit.translation_mm[0],
        fit.scale * point_in_ref[1] + fit.translation_mm[1],
    ];
    let mut best: Option<(f64, usize)> = None;
    for index in stroke_index.near(point, ATTRIB_MM) {
        let line = strokes[index].0;
        let distance = dist_point_seg(point, line.a, line.b);
        if distance <= ATTRIB_MM && best.is_none_or(|(best_distance, _)| distance < best_distance) {
            best = Some((distance, index));
        }
    }
    if let Some((_, index)) = best {
        return strokes[index].1.clone();
    }
    let on_edge = page_mm[0] > 0.0
        && page_mm[1] > 0.0
        && (point_in_ref[0] <= 20.0
            || point_in_ref[1] <= 20.0
            || page_mm[0] - point_in_ref[0] <= 20.0
            || page_mm[1] - point_in_ref[1] <= 20.0);
    if on_edge {
        "图框".to_string()
    } else {
        "图纸线".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::compare_plots;
    use crate::read::{InkSegment, PlotInk, TextSpan};

    fn ink(segments: Vec<InkSegment>, spans: Vec<TextSpan>) -> PlotInk {
        PlotInk {
            page_mm: [200.0, 100.0],
            segments,
            spans,
        }
    }

    fn line(start: [f64; 2], end: [f64; 2]) -> InkSegment {
        InkSegment {
            start_mm: start,
            end_mm: end,
            width_mm: 0.2,
        }
    }

    fn word(origin: [f64; 2]) -> TextSpan {
        TextSpan {
            text: "Ab".into(),
            origin_mm: origin,
            height_mm: 4.0,
            advance_mm: 6.0,
            direction: [1.0, 0.0],
        }
    }

    fn run(text: &str, origin: [f64; 2], advance_mm: f64) -> TextSpan {
        TextSpan {
            text: text.into(),
            origin_mm: origin,
            height_mm: 2.5,
            advance_mm,
            direction: [1.0, 0.0],
        }
    }

    #[test]
    fn an_identical_plot_is_fully_covered() {
        let plot = ink(
            vec![line([0.0, 10.0], [40.0, 10.0])],
            vec![word([5.0, 20.0])],
        );
        let report = compare_plots(&plot, &plot, &[]);
        assert!(report.coverage > 0.99, "{}", report.coverage);
        assert!(report.extra_length_mm < 0.2, "{}", report.extra_length_mm);
        assert_eq!(report.text_matched, 1);
        assert!(report.fit.within_threshold, "{:?}", report.fit);
    }

    #[test]
    fn a_whitespace_only_run_is_not_a_label() {
        // Both plots carry a run that is only spaces. It has no text to pair,
        // so it must not enter either side's tally or the residual list.
        let blank = TextSpan {
            text: "  ".into(),
            origin_mm: [150.0, 80.0],
            height_mm: 4.0,
            advance_mm: 0.0,
            direction: [1.0, 0.0],
        };
        let reference = ink(vec![], vec![blank.clone(), word([5.0, 20.0])]);
        let ours = ink(vec![], vec![blank, word([5.0, 20.0])]);
        let report = compare_plots(&reference, &ours, &[]);
        assert_eq!(report.text_reference, 1);
        assert_eq!(report.text_ours, 1);
        assert_eq!(report.text_matched, 1);
        assert!(report.unmatched_reference_text.is_empty());
        assert!(report.unmatched_our_text.is_empty());
    }

    #[test]
    fn a_label_split_into_runs_pairs_with_its_whole_run() {
        // The reference printed `2"NC` as one run and we drew it as two.
        let reference = ink(vec![], vec![run("2\"NC", [100.0, 50.0], 4.8)]);
        let ours = ink(
            vec![],
            vec![
                run("2\"", [100.0, 50.0], 2.2),
                run("NC", [102.6, 50.0], 2.6),
            ],
        );
        let report = compare_plots(&reference, &ours, &[]);
        assert_eq!(report.text_reference, 1);
        assert_eq!(report.text_ours, 2);
        assert_eq!(report.text_matched, 1);
        assert!(report.unmatched_reference_text.is_empty());
        assert!(report.unmatched_our_text.is_empty());
    }

    #[test]
    fn a_label_fused_into_one_run_pairs_with_the_split_reference() {
        // The same drawing elsewhere: the reference split `2"NC`, we fused it.
        let reference = ink(
            vec![],
            vec![
                run("2\"", [100.0, 50.0], 2.2),
                run("NC", [102.6, 50.0], 2.6),
            ],
        );
        let ours = ink(vec![], vec![run("2\"NC", [100.0, 50.0], 4.8)]);
        let report = compare_plots(&reference, &ours, &[]);
        assert_eq!(report.text_reference, 2);
        assert_eq!(report.text_ours, 1);
        assert_eq!(report.text_matched, 2);
        assert!(report.unmatched_reference_text.is_empty());
        assert!(report.unmatched_our_text.is_empty());
    }

    #[test]
    fn a_padded_pair_fuses_without_its_pads() {
        // The L drawing's two `0103501A` tags sit 13.2 mm apart on one
        // baseline. The reference reads them back as two runs, each with the
        // field's leading pad; our reader glues them into one run with no
        // pad at all. The pad is not part of either label, so the fused
        // spelling has to be tried without it.
        let reference = ink(
            vec![],
            vec![
                run(" 0103501A", [100.0, 50.0], 13.2),
                run(" 0103501A", [113.2, 50.0], 13.2),
            ],
        );
        let ours = ink(vec![], vec![run("0103501A0103501A", [100.1, 50.0], 26.4)]);
        let report = compare_plots(&reference, &ours, &[]);
        assert_eq!(report.text_reference, 2);
        assert_eq!(report.text_ours, 1);
        assert_eq!(report.text_matched, 2);
        assert!(report.unmatched_reference_text.is_empty());
        assert!(report.unmatched_our_text.is_empty());
    }

    #[test]
    fn a_joined_label_still_has_to_spell_the_other_side() {
        // Two of our runs sit where the label should be, but they spell
        // something else; joining must not turn the near miss into a match.
        let reference = ink(vec![], vec![run("2\"NC", [100.0, 50.0], 4.8)]);
        let ours = ink(
            vec![],
            vec![
                run("2\"", [100.0, 50.0], 2.2),
                run("NQ", [102.6, 50.0], 2.6),
            ],
        );
        let report = compare_plots(&reference, &ours, &[]);
        assert_eq!(report.text_matched, 0);
        assert_eq!(report.unmatched_reference_text.len(), 1);
        assert_eq!(report.unmatched_our_text.len(), 2);
    }

    #[test]
    fn a_paired_run_reports_its_offset_in_body_heights() {
        // Ours sits half a millimetre below the reference's baseline on a
        // 2.5 mm body, which is a fifth of a body — inside the 1 mm pairing
        // window, and exactly the kind of systematic miss the report exists
        // to show while it is still too small to be a residual.
        let reference = ink(vec![], vec![run("Ab", [100.0, 50.0], 6.0)]);
        let ours = ink(vec![], vec![run("Ab", [100.0, 49.5], 6.0)]);
        let report = compare_plots(&reference, &ours, &[]);
        assert_eq!(report.text_matched, 1);
        let class = report.text_offsets.first().expect("one class");
        assert_eq!(class.class, "unknown");
        assert_eq!(class.count, 1);
        assert!(
            (class.mean_down_heights - 0.2).abs() < 1e-9,
            "down {}",
            class.mean_down_heights
        );
        assert!(class.mean_along_heights.abs() < 1e-9);
        assert!(class.mean_advance_heights.abs() < 1e-9);
        assert!((class.max_shift_mm - 0.5).abs() < 1e-9);
    }

    #[test]
    fn a_wider_run_reports_its_width_beside_its_shift() {
        // A centred run that steps back by half its own width starts further
        // left when that width is larger, with no placement rule wrong. The
        // class reports both numbers so this cannot be read as a centring
        // error: 1 mm wider on a 2.5 mm body is 0.4 of a body, half of which
        // is the 0.5 mm shift.
        let reference = ink(vec![], vec![run("Ab", [100.0, 50.0], 6.0)]);
        let ours = ink(vec![], vec![run("Ab", [99.5, 50.0], 7.0)]);
        let report = compare_plots(&reference, &ours, &[]);
        assert_eq!(report.text_matched, 1);
        let class = report.text_offsets.first().expect("one class");
        assert!(
            (class.mean_advance_heights - 0.4).abs() < 1e-9,
            "width {}",
            class.mean_advance_heights
        );
        assert!(
            (class.mean_along_heights + 0.2).abs() < 1e-9,
            "along {}",
            class.mean_along_heights
        );
    }

    #[test]
    fn a_paired_run_is_classed_by_its_lettering_rule() {
        let reference = ink(vec![], vec![run("Ab", [100.0, 50.0], 6.0)]);
        let ours = ink(vec![], vec![run("Ab", [100.0, 50.0], 6.0)]);
        let report = super::compare_plots_with_sources(
            &reference,
            &ours,
            &[],
            &[super::TextSource {
                text: "Ab".to_string(),
                origin_mm: [100.0, 50.0],
                stream_path: None,
                byte_range: None,
                confidence: "decoded".to_string(),
                symbol: None,
                anchor: super::TextAnchor::Right,
                line: super::TextLine::Ascender,
            }],
        );
        assert_eq!(report.text_offsets.len(), 1);
        assert_eq!(report.text_offsets[0].class, "Right/Ascender");
        assert_eq!(report.text_offsets[0].count, 1);
    }

    #[test]
    fn a_ten_millimetre_shift_is_a_finding() {
        // 40 mm apart, shifted 10 mm. A 20 mm spacing would put each
        // reference line exactly between two of ours, so the nearest
        // neighbour would not be the shifted copy.
        let reference = ink(
            vec![
                line([0.0, 0.0], [80.0, 0.0]),
                line([0.0, 40.0], [80.0, 40.0]),
                line([0.0, 80.0], [80.0, 80.0]),
                line([0.0, 120.0], [80.0, 120.0]),
            ],
            vec![],
        );
        let ours = ink(
            vec![
                line([0.0, 10.0], [80.0, 10.0]),
                line([0.0, 50.0], [80.0, 50.0]),
                line([0.0, 90.0], [80.0, 90.0]),
                line([0.0, 130.0], [80.0, 130.0]),
            ],
            vec![],
        );
        let report = compare_plots(&reference, &ours, &[]);
        assert!(!report.fit.within_threshold, "{:?}", report.fit);
        assert!(report.fit.finding.is_some());
        assert!(report.coverage > 0.95, "{}", report.coverage);
    }

    #[test]
    fn template_masks_follow_the_surveyed_sheets() {
        let a0 = super::template_masks([1189.0, 841.0]);
        assert!(a0.iter().any(|mask| mask.contains([24.21, 625.52])));
        assert!(a0.iter().any(|mask| mask.contains([24.05, 684.11])));
        assert!(a0.iter().any(|mask| mask.contains([1063.24, 74.17])));
        assert!(!a0.iter().any(|mask| mask.contains([95.33, 582.34])));
        assert!(!a0.iter().any(|mask| mask.contains([1069.00, 165.27])));

        let a1 = super::template_masks([841.0, 594.0]);
        assert!(a1.iter().any(|mask| mask.contains([24.21, 378.46])));
        assert!(a1.iter().any(|mask| mask.contains([692.91, 29.46])));
        assert!(a1.iter().any(|mask| mask.contains([723.73, 37.93])));
        assert!(!a1.iter().any(|mask| mask.contains([77.22, 532.38])));
        assert!(!a1.iter().any(|mask| mask.contains([81.96, 408.77])));
        assert!(!a1.iter().any(|mask| mask.contains([679.70, 110.24])));

        assert!(super::template_masks([200.0, 100.0]).is_empty());
    }

    #[test]
    fn a_nearby_source_is_copied_onto_unmatched_text() {
        let reference = ink(vec![], vec![]);
        let ours = ink(vec![], vec![word([10.0, 20.0])]);
        let report = super::compare_plots_with_sources(
            &reference,
            &ours,
            &[],
            &[super::TextSource {
                text: "Ab".to_string(),
                origin_mm: [12.0, 21.0],
                stream_path: Some("sheet".to_string()),
                byte_range: Some([4, 9]),
                confidence: "decoded".to_string(),
                symbol: Some("valve.sym".to_string()),
                anchor: super::TextAnchor::Left,
                line: super::TextLine::Middle,
            }],
        );
        let run = &report.unmatched_our_text[0];
        assert_eq!(run.stream_path.as_deref(), Some("sheet"));
        assert_eq!(run.byte_range, Some([4, 9]));
        assert_eq!(run.confidence.as_deref(), Some("decoded"));
        assert_eq!(run.symbol.as_deref(), Some("valve.sym"));
        assert!(report.extra_outside_by_symbol.is_empty());
    }
}
