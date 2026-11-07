//! Why does each label the comparison pairs still miss its reference run?
//!
//! `reference_plots_acceptance` pairs every label and gates each lettering
//! class on its *mean* shift, so a class can average zero while single labels
//! in it miss by half a millimetre. This walks every pair the comparison makes
//! on the eleven `验收数据` sheets — the whole page, no template mask, only the
//! missing-symbol boxes — and flags the ones whose anchor point (the baseline
//! start for a left anchor, its middle or end for a centred or right one) lands
//! more than 0.15 mm from the reference's, or whose run is more than 0.05 mm
//! wider or narrower.
//!
//! Each pair carries the evidence that names its cause: both sides' face,
//! body size, horizontal scale and per-glyph letter spacing, read back off the
//! glyphs themselves rather than off the writer's own bookkeeping; the stored
//! value with its padding; the record's raw anchor code and its paragraph's
//! alignment; and where the first and last *inked* glyph sit on each side, so
//! a space the reference draws is not mistaken for a shift of the ink.
//!
//! ```text
//! cargo run -p pid-plot --example probe_why_paired_labels_miss_the_reference -- [out-dir]
//! ```
//!
//! Prints one line per sheet and one per cause, and writes `pairs.tsv` (every
//! pair) and `flagged.tsv` into `out-dir` (default `target/text-census`).
//! `CAUSE=<name>` also prints that cause's flagged pairs. The corpus is
//! `PID_ACCEPTANCE_CORPUS`, default `D:\work\cad\pid-test-data\验收数据`. Read-only.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use hayro::hayro_interpret::font::Glyph;
use hayro::hayro_interpret::hayro_cmap::BfString;
use hayro::hayro_interpret::hayro_syntax::Pdf;
use hayro::hayro_interpret::{
    BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache,
    InterpreterSettings, Paint, PathDrawMode, SoftMask, TransformExt, interpret_page,
};
use kurbo::{Affine, BezPath, Point, Rect};
use pid_plot::compare::{Fit, MaskRect, TextPair, compare_plots_with_model};
use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperModel, PaperPrimitive, PaperShape, TextAnchor, lay_out_drawing};
use pid_plot::read::{PlotInk, TextSpan, read_plot, view_pt_to_paper_mm};
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

/// Anchor-point miss that flags a pair, millimetres.
const POSITION_MM: f64 = 0.15;
/// Run-width difference that flags a pair, millimetres.
const WIDTH_MM: f64 = 0.05;
/// Letter-spacing difference read as a different spacing rule, points.
///
/// The plot driver writes a template label's horizontal scale to three
/// decimals and puts the rounding back into the spacing — `0.785` for
/// `0.7857` leaves up to `0.024 pt` on one gap of `CADD号` — while a two-glyph
/// run spaced by `lpDx` differs from an unspaced one by `0.1 pt` or more.
const TC_PT: f64 = 0.05;
/// How far a paper-table label may sit from our run when the comparison's own
/// 20 mm source window missed it, millimetres. A boxed field or a long centred
/// title puts its insertion point further than that from the run's start.
const WIDE_SOURCE_MM: f64 = 80.0;

const MM_PER_PT: f64 = 25.4 / 72.0;

/// One glyph as the page draws it.
#[derive(Clone, Debug)]
struct PlacedGlyph {
    text: String,
    origin_mm: [f64; 2],
    body_mm: f64,
    /// Horizontal scale against the body: `Tm`'s own, times `Tz`.
    sx: f64,
    /// The glyph's own advance on the page, its letter spacing excluded.
    advance_mm: f64,
    face: String,
}

/// Collects every glyph the same way `read::read_plot` does, keeping the face.
struct GlyphDevice {
    page_height_pt: f64,
    glyphs: Vec<PlacedGlyph>,
}

impl<'a> Device<'a> for GlyphDevice {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'a>>) {}
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn draw_path(&mut self, _: &BezPath, _: Affine, _: &Paint<'a>, _: &PathDrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_image(&mut self, _: Image<'a, '_>, _: Affine) {}
    fn pop_clip_path(&mut self) {}
    fn pop_transparency_group(&mut self) {}

    fn draw_glyph(
        &mut self,
        glyph: &Glyph<'a>,
        transform: Affine,
        glyph_transform: Affine,
        _: &Paint<'a>,
        _: &GlyphDrawMode,
    ) {
        let text = match glyph.as_unicode() {
            Some(BfString::Char(ch)) => ch.to_string(),
            Some(BfString::String(text)) => text,
            None => return,
        };
        if text.chars().all(char::is_control) {
            return;
        }
        let (advance, face) = match glyph {
            Glyph::Outline(glyph) => (
                glyph
                    .advance_width()
                    .filter(|width| *width > 0.0)
                    .map_or(500.0, f64::from),
                glyph
                    .font_data()
                    .and_then(|data| data.postscript_name)
                    .map(|name| match name.split_once('+') {
                        Some((prefix, rest)) if prefix.len() == 6 => rest.to_string(),
                        _ => name,
                    })
                    .unwrap_or_default(),
            ),
            Glyph::Type3(glyph) => (
                glyph
                    .advance_width()
                    .filter(|width| width.is_finite() && *width > 0.0)
                    .map_or(600.0, f64::from),
                "Type3".to_string(),
            ),
        };
        let placed = transform * glyph_transform;
        let origin = placed * Point::ORIGIN;
        let height_pt = (placed * Point::new(0.0, 1000.0) - origin).hypot();
        if !height_pt.is_finite() || height_pt > 10_000.0 {
            return;
        }
        let width_pt = (placed * Point::new(1000.0, 0.0) - origin).hypot();
        let advance_pt = (placed * Point::new(advance, 0.0) - origin).hypot();
        self.glyphs.push(PlacedGlyph {
            text,
            origin_mm: view_pt_to_paper_mm(origin.x, origin.y, self.page_height_pt),
            body_mm: height_pt * MM_PER_PT,
            sx: if height_pt > 0.0 {
                width_pt / height_pt
            } else {
                1.0
            },
            advance_mm: advance_pt * MM_PER_PT,
            face,
        });
    }
}

fn read_glyphs(bytes: &[u8]) -> Option<Vec<PlacedGlyph>> {
    let pdf = Pdf::new(bytes.to_vec()).ok()?;
    let pages = pdf.pages();
    let page = pages.first()?;
    let (width_pt, height_pt) = page.render_dimensions();
    let cache = InterpreterCache::new();
    let settings = InterpreterSettings {
        render_annotations: false,
        ..Default::default()
    };
    let mut ctx = Context::new(
        page.initial_transform(true).to_kurbo(),
        Rect::new(0.0, 0.0, f64::from(width_pt), f64::from(height_pt)),
        &cache,
        page.xref(),
        settings,
    );
    let mut device = GlyphDevice {
        page_height_pt: f64::from(height_pt),
        glyphs: Vec::new(),
    };
    interpret_page(page, &mut ctx, &mut device);
    Some(device.glyphs)
}

/// The glyphs of one page, findable by the origin a span starts at.
struct PageGlyphs {
    glyphs: Vec<PlacedGlyph>,
    by_origin: HashMap<(i64, i64), Vec<usize>>,
}

impl PageGlyphs {
    fn new(glyphs: Vec<PlacedGlyph>) -> Self {
        let mut by_origin: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (slot, glyph) in glyphs.iter().enumerate() {
            by_origin
                .entry(origin_key(glyph.origin_mm))
                .or_default()
                .push(slot);
        }
        Self { glyphs, by_origin }
    }

    /// The glyphs `span` was clustered from: the run of glyphs, in drawing
    /// order, that starts where the span starts and spells it.
    fn of(&self, span: &TextSpan) -> &[PlacedGlyph] {
        let Some(starts) = self.by_origin.get(&origin_key(span.origin_mm)) else {
            return &[];
        };
        for &start in starts {
            let mut text = String::new();
            let mut end = start;
            while end < self.glyphs.len() && text.len() < span.text.len() {
                text.push_str(&self.glyphs[end].text);
                end += 1;
            }
            if text == span.text {
                return &self.glyphs[start..end];
            }
        }
        &[]
    }
}

fn origin_key(point: [f64; 2]) -> (i64, i64) {
    (
        (point[0] * 1e5).round() as i64,
        (point[1] * 1e5).round() as i64,
    )
}

/// One side of a pair, read off its glyphs.
#[derive(Clone, Debug, Default)]
struct Side {
    face: String,
    body_pt: f64,
    sx: f64,
    /// Mean letter spacing of the first run, text-space points.
    tc_pt: f64,
    /// Glyph count of each run of the chain.
    run_glyphs: Vec<usize>,
    /// Every glyph of the chain in drawing order.
    glyphs: Vec<PlacedGlyph>,
    /// Letter spacing after each glyph that has a successor in its own run,
    /// text-space points, indexed like `glyphs` (`NaN` at a run's last glyph).
    gaps_pt: Vec<f64>,
}

fn side(ink: &PlotInk, runs: &[usize], page: &PageGlyphs) -> Side {
    let mut out = Side::default();
    for (position, run) in runs.iter().enumerate() {
        let span = &ink.spans[*run];
        let glyphs = page.of(span);
        let direction = span.direction;
        out.run_glyphs.push(glyphs.len());
        for (index, glyph) in glyphs.iter().enumerate() {
            let gap = glyphs.get(index + 1).map_or(f64::NAN, |next| {
                let delta = [
                    next.origin_mm[0] - glyph.origin_mm[0],
                    next.origin_mm[1] - glyph.origin_mm[1],
                ];
                let step = delta[0] * direction[0] + delta[1] * direction[1];
                (step - glyph.advance_mm) / MM_PER_PT / glyph.sx
            });
            out.glyphs.push(glyph.clone());
            out.gaps_pt.push(gap);
        }
        if position == 0
            && let Some(first) = glyphs.first()
        {
            out.face = first.face.clone();
            out.body_pt = first.body_mm / MM_PER_PT;
            out.sx = first.sx;
            let gaps: Vec<f64> = out
                .gaps_pt
                .iter()
                .copied()
                .filter(|gap| gap.is_finite())
                .collect();
            out.tc_pt = if gaps.is_empty() {
                0.0
            } else {
                gaps.iter().sum::<f64>() / gaps.len() as f64
            };
        }
    }
    out
}

/// The paper-table label one pair came from, as the census needs it.
#[derive(Clone, Debug)]
struct Label {
    value: String,
    /// Insertion point, our paper millimetres.
    origin_mm: [f64; 2],
    kind: &'static str,
    anchor: TextAnchor,
    line: String,
    rotation_deg: f64,
    face: String,
    height_mm: f64,
    basis: String,
    width_permille: Option<u16>,
    stream: String,
    bytes: Option<[usize; 2]>,
    symbol: String,
}

fn label_of(primitive: &PaperPrimitive) -> Option<Label> {
    let PaperShape::Text {
        origin_mm,
        value,
        anchor,
        line,
        rotation,
        font,
        height_mm,
        height_basis,
        width_permille,
        ..
    } = &primitive.shape
    else {
        return None;
    };
    Some(Label {
        value: value.clone(),
        origin_mm: *origin_mm,
        kind: if primitive.furniture {
            "template"
        } else if primitive.symbol.is_some() {
            "symbol"
        } else {
            "sheet"
        },
        anchor: *anchor,
        line: format!("{line:?}"),
        rotation_deg: rotation.to_degrees(),
        face: font.clone().unwrap_or_default(),
        height_mm: height_mm.unwrap_or(f64::NAN),
        basis: format!("{height_basis:?}"),
        width_permille: *width_permille,
        stream: primitive.stream_path.clone().unwrap_or_default(),
        bytes: primitive.byte_range,
        symbol: primitive.symbol.clone().unwrap_or_default(),
    })
}

fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn into_reference(point: [f64; 2], fit: &Fit) -> [f64; 2] {
    [
        (point[0] - fit.translation_mm[0]) / fit.scale,
        (point[1] - fit.translation_mm[1]) / fit.scale,
    ]
}

fn anchor_fraction(anchor: TextAnchor) -> f64 {
    match anchor {
        TextAnchor::Left => 0.0,
        TextAnchor::Center => 0.5,
        TextAnchor::Right => 1.0,
    }
}

/// The label behind a pair: the one spelling our first run's text whose
/// insertion point sits nearest where its own anchor hangs that run — the
/// run's start, middle or end.
///
/// The comparison's own source is the nearest same-spelled label to the run's
/// *start*, which for a centred label can be its neighbour: two ` 1001701A`
/// tags a few millimetres apart swap insertion points that way.
fn find_label(model: &PaperModel, pair: &TextPair, ours: &PlotInk, fit: &Fit) -> Option<Label> {
    let first = &ours.spans[*pair.our_runs.first()?];
    let wanted = normalize(&first.text);
    let start = into_reference(first.origin_mm, fit);
    // A label one face cannot letter whole — `H=120℃ ` — is two runs on both
    // pages, so a run may spell only part of the label it came from; an exact
    // spelling still wins over a partial one at any distance.
    let mut best: Option<(bool, f64, &PaperPrimitive)> = None;
    for primitive in &model.primitives {
        let PaperShape::Text {
            origin_mm,
            value,
            anchor,
            ..
        } = &primitive.shape
        else {
            continue;
        };
        let spelled = normalize(value);
        let exact = spelled == wanted;
        if !exact && !spelled.contains(&wanted) {
            continue;
        }
        let insertion = into_reference(*origin_mm, fit);
        let fraction = if exact { anchor_fraction(*anchor) } else { 0.0 };
        let at = [
            start[0] + first.direction[0] * first.advance_mm * fraction,
            start[1] + first.direction[1] * first.advance_mm * fraction,
        ];
        let distance = (at[0] - insertion[0]).hypot(at[1] - insertion[1]);
        let better = best.is_none_or(|(best_exact, nearest, _)| {
            (exact && !best_exact) || (exact == best_exact && distance < nearest)
        });
        if distance <= WIDE_SOURCE_MM && better {
            best = Some((exact, distance, primitive));
        }
    }
    best.and_then(|(_, _, primitive)| label_of(primitive))
}

/// What the parsed `.pid` says about one sheet label: its raw anchor code and
/// its paragraph's alignment.
#[derive(Clone, Debug)]
struct RecordFacts {
    anchor_code: u8,
    paragraph: Option<TextAnchor>,
    /// Whether the record states a text box.
    boxed: bool,
    /// How the record's own character-style runs resolved, and the line
    /// spacing its paragraph states.
    lettering: String,
}

fn record_facts(path: &std::path::Path) -> HashMap<(String, usize), RecordFacts> {
    let mut out = HashMap::new();
    let Ok(doc) = pid_parse::PidParser::new().parse_file(path) else {
        return out;
    };
    let geometry = pid_parse::build_normalized_geometry(&doc);
    let styles = pid_parse::style_link::text_styles_for_document(&doc);
    for entity in &geometry.entities {
        let pid_parse::PidGraphicKind::Text {
            anchor_code,
            text_box_width,
            ..
        } = &entity.kind
        else {
            continue;
        };
        let (Some(stream), Some(range)) = (&entity.source.stream_path, &entity.source.byte_range)
        else {
            continue;
        };
        let resolved = entity
            .graphic_oid
            .and_then(|oid| styles.get(&(stream.clone(), oid)));
        let lettering = resolved.map_or_else(
            || "-".to_string(),
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
        let paragraph = resolved
            .and_then(|style| style.effective())
            .and_then(|style| style.alignment)
            .map(|alignment| match alignment {
                pid_parse::style_link::TextAlignment::Left => TextAnchor::Left,
                pid_parse::style_link::TextAlignment::Center => TextAnchor::Center,
                pid_parse::style_link::TextAlignment::Right => TextAnchor::Right,
            });
        out.insert(
            (stream.clone(), range.start),
            RecordFacts {
                anchor_code: *anchor_code,
                paragraph,
                boxed: text_box_width.is_some_and(|width| width > 0.0),
                lettering,
            },
        );
    }
    out
}

/// GDI's 72-pixel advances of `0x20..=0x7E`, the same tables `write.rs`
/// letters with (`docs/findings/2026-11-07-text-geometry-writer.md`, appendix).
#[rustfmt::skip]
const ARIAL_72PX: [u16; 95] = [
    18, 20, 23, 36, 36, 57, 43, 12, 21, 21, 25, 37, 18, 21, 18, 18,
    36, 36, 36, 36, 36, 36, 36, 36, 36, 36, 18, 18, 37, 37, 37, 36,
    65, 43, 43, 46, 46, 43, 39, 50, 46, 18, 32, 43, 36, 53, 46, 50,
    43, 50, 46, 43, 40, 46, 43, 64, 41, 42, 39, 18, 18, 18, 27, 36,
    21, 36, 36, 32, 36, 36, 18, 36, 36, 14, 15, 33, 14, 55, 36, 36,
    36, 36, 21, 32, 18, 36, 33, 47, 31, 31, 31, 21, 17, 21, 37,
];
#[rustfmt::skip]
const ARIAL_NARROW_72PX: [u16; 95] = [
    14, 14, 18, 29, 28, 46, 33, 10, 17, 17, 19, 29, 14, 17, 14, 14,
    28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 14, 14, 29, 29, 29, 28,
    51, 33, 33, 36, 36, 33, 31, 39, 36, 14, 25, 33, 28, 42, 36, 39,
    33, 39, 36, 33, 31, 36, 33, 47, 33, 33, 31, 14, 14, 14, 23, 28,
    17, 28, 28, 25, 28, 28, 14, 28, 28, 11, 11, 25, 11, 42, 28, 28,
    28, 28, 17, 25, 14, 28, 25, 36, 25, 25, 25, 17, 13, 17, 29,
];
/// GDI's cap (`ADMXZ` glyph origin) and ascent at `lfHeight = 72`, pixels:
/// `render.dll`'s `M` for a cap-height and an ascent style.
const REFERENCE_PX: [(&str, f64, f64); 4] = [
    ("SimSun", 49.2, 62.0),
    ("FangSong", 48.2, 62.0),
    ("Arial", 46.0, 58.0),
    ("Arial Narrow", 44.0, 58.0),
];
/// One 150 dpi pixel of the reference plot's device grid, millimetres.
const PIXEL_MM: f64 = 25.4 / 150.0;

/// The face tables the plotter model reads, per face key.
struct FaceTables {
    key: &'static str,
    bytes: Vec<u8>,
    /// `usWinAscent + usWinDescent`, as a fraction of the em.
    cell: f64,
    /// `xAvgCharWidth` over the cell.
    avg_over_cell: f64,
}

impl FaceTables {
    fn load(key: &'static str, dirs: &[PathBuf]) -> Option<Self> {
        use fontcull_read_fonts::{FontRef, TableProvider};
        let file = match key {
            "Arial" => "arial.ttf",
            "Arial Narrow" => "ARIALN.TTF",
            "SimSun" => "simsun.ttf",
            "FangSong" => "simfang.ttf",
            "NSimSun" => "nsimsun.ttf",
            _ => return None,
        };
        let bytes = dirs
            .iter()
            .find_map(|dir| std::fs::read(dir.join(file)).ok())?;
        let font = FontRef::new(&bytes).ok()?;
        let em = f64::from(font.head().ok()?.units_per_em());
        let os2 = font.os2().ok()?;
        let cell = (f64::from(os2.us_win_ascent()) + f64::from(os2.us_win_descent())) / em;
        let avg_over_cell = f64::from(os2.x_avg_char_width()) / em / cell;
        Some(Self {
            key,
            bytes,
            cell,
            avg_over_cell,
        })
    }

    /// A character's advance as a fraction of the em.
    fn advance_em(&self, ch: char) -> f64 {
        use fontcull_read_fonts::{FontRef, TableProvider};
        use fontcull_skrifa::MetadataProvider;
        use fontcull_skrifa::instance::{LocationRef, Size};
        let Ok(font) = FontRef::new(&self.bytes) else {
            return 0.0;
        };
        let em = font
            .head()
            .map_or(1.0, |head| f64::from(head.units_per_em()));
        font.charmap()
            .map(ch)
            .and_then(|glyph| {
                font.glyph_metrics(Size::unscaled(), LocationRef::default())
                    .advance_width(glyph)
            })
            .map_or(0.0, |width| f64::from(width) / em)
    }

    /// The character's advance in the plotter's 72-unit reference font,
    /// pixels, narrowed by the style's character width: GDI's hinted table
    /// for printable ASCII in the two Arial faces, the face's own advance on
    /// the 72-pixel cell otherwise. A character an Arial face does not carry
    /// is measured as `?`, what the ANSI conversion in front of
    /// `GetTextExtentExPointA` turns it into.
    fn reference_advance_px(&self, ch: char, width_scale: f64) -> f64 {
        let table = match self.key {
            "Arial" => Some(&ARIAL_72PX),
            "Arial Narrow" => Some(&ARIAL_NARROW_72PX),
            _ => None,
        };
        if let Some(table) = table {
            let ansi = if (' '..='~').contains(&ch) { ch } else { '?' };
            let index = u32::from(ansi) - 0x20;
            return table
                .get(index as usize)
                .map_or(0.0, |px| f64::from(*px) * width_scale);
        }
        (self.advance_em(ch) * 72.0 / self.cell).round() * width_scale
    }
}

/// The plotter's two fonts for one label: the 72-unit reference font every
/// advance, extent and line is measured on, and the device font `a8` scales
/// it to (`render.dll` `sub_1007EF20` / `sub_1000D300`).
struct PlotterFont<'a> {
    face: &'a FaceTables,
    cap72: f64,
    asc72: f64,
    /// Device pixels per reference pixel.
    a8: f64,
    /// The style's character width, applied to the reference font.
    width_scale: f64,
    /// The device font's horizontal scale against its own proportions.
    sx: f64,
    /// The device font's em, pixels.
    em_px: f64,
}

impl<'a> PlotterFont<'a> {
    fn new(label: &Label, face: &'a FaceTables) -> Option<Self> {
        let (_, cap72, asc72) = REFERENCE_PX.iter().find(|(key, ..)| *key == face.key)?;
        let measures = if label.basis == "Ascent" {
            *asc72
        } else {
            *cap72
        };
        let a8 = label.height_mm / PIXEL_MM / measures;
        if !(a8.is_finite() && a8 > 0.0) {
            return None;
        }
        let lf_h = (72.0 * a8).round();
        let (width_scale, sx) = match label.width_permille.filter(|width| *width != 1000) {
            Some(width) => {
                let lw_ref = (72.0 * face.avg_over_cell * f64::from(width) / 1000.0).trunc();
                let lf_w = (lw_ref * a8).round();
                (
                    lw_ref / (72.0 * face.avg_over_cell),
                    lf_w / (lf_h * face.avg_over_cell),
                )
            }
            None => (1.0, 1.0),
        };
        Some(Self {
            face,
            cap72: *cap72,
            asc72: *asc72,
            a8,
            width_scale,
            sx,
            em_px: lf_h / face.cell,
        })
    }

    /// A string's continuous extent on the device, pixels.
    fn extent_px(&self, text: &str) -> f64 {
        text.chars()
            .map(|ch| self.face.reference_advance_px(ch, self.width_scale) * self.a8)
            .sum()
    }

    /// `lpDx`: each glyph's reference advance times `a8`, rounded on its own,
    /// nudged by one wherever the rounded total drifts a pixel off.
    fn lp_dx(&self, chars: &[char]) -> Vec<f64> {
        let mut float_acc = 0.0;
        let mut int_acc: i64 = 0;
        chars
            .iter()
            .map(|ch| {
                let step = self.face.reference_advance_px(*ch, self.width_scale) * self.a8;
                float_acc += step;
                let mut whole = step.round() as i64;
                let drift = float_acc - (int_acc + whole) as f64;
                if drift > 1.0 {
                    whole += 1;
                } else if drift < -1.0 {
                    whole -= 1;
                }
                int_acc += whole;
                whole as f64
            })
            .collect()
    }

    /// A glyph's own advance in the device font, pixels.
    fn natural_px(&self, ch: char) -> f64 {
        self.face.advance_em(ch) * self.em_px * self.sx
    }

    /// The one `Tc` the plot driver writes for a run, device pixels: the
    /// mean of `lpDx` less the glyphs' own advances over the `n − 1` gaps.
    fn tc_px(&self, chars: &[char]) -> Option<f64> {
        let gaps = chars.len().checked_sub(1).filter(|gaps| *gaps > 0)?;
        let spaced: f64 = self.lp_dx(chars)[..gaps].iter().sum();
        let natural: f64 = chars[..gaps].iter().map(|ch| self.natural_px(*ch)).sum();
        Some((spaced - natural) / gaps as f64)
    }
}

/// Where `render.dll` letters a label's first inked glyph, along the baseline
/// from the insertion point, millimetres.
///
/// The box is the stored value's continuous extent in the 72-unit reference
/// font times `a8`, trailing pad included; the anchor code hangs it from the
/// insertion point. The line is the value with its trailing whitespace
/// dropped, aligned inside the box by the paragraph; a leading pad stays in
/// the line and is lettered. A boxed field is already hung from the edge its
/// paragraph names, so only the line is stepped back.
///
/// Inside the line the glyphs sit where the PDF puts them: one `Tc` per run,
/// so a glyph's offset is its predecessors' own advances plus that many `Tc`
/// — except on a rotated run, which the driver letters glyph by glyph at the
/// `lpDx` positions themselves.
fn modelled_ink_start(
    label: &Label,
    record: Option<&RecordFacts>,
    face: &FaceTables,
    run_text: &str,
) -> Option<f64> {
    let font = PlotterFont::new(label, face)?;
    let line = label.value.trim_end();
    let box_px = font.extent_px(&label.value);
    let line_px = font.extent_px(line);
    let anchor = anchor_fraction(label.anchor);
    let paragraph = record
        .filter(|record| !record.boxed)
        .and_then(|record| record.paragraph)
        .map_or(anchor, anchor_fraction);
    let line_start = if record.is_some_and(|record| record.boxed) {
        -anchor * line_px
    } else {
        -anchor * box_px + paragraph * (box_px - line_px)
    };
    // The run may be the second half of a label one face could not letter
    // whole (`H=120` + `℃`): start at its own first character.
    let wanted: String = run_text.trim().chars().take(1).collect();
    let first = line
        .char_indices()
        .find(|(index, _)| !wanted.is_empty() && line[*index..].starts_with(wanted.as_str()))
        .map_or(0, |(index, _)| line[..index].chars().count());
    let chars: Vec<char> = line.chars().collect();
    let offset = if is_rotated(label) {
        font.lp_dx(&chars).iter().take(first).sum::<f64>()
    } else {
        let tc = font.tc_px(&chars).unwrap_or(0.0);
        chars
            .iter()
            .take(first)
            .map(|ch| font.natural_px(*ch) + tc)
            .sum::<f64>()
    };
    Some((line_start + offset) * PIXEL_MM)
}

fn is_rotated(label: &Label) -> bool {
    (label.rotation_deg.abs() - 90.0).abs() < 1.0
}

/// The letter spacing the `lpDx` model gives the run the reference letters,
/// text-space points, padding included: a leading space is a glyph of the run.
fn modelled_tc(label: &Label, face: &FaceTables, run_text: &str) -> Option<f64> {
    let font = PlotterFont::new(label, face)?;
    let chars: Vec<char> = run_text.trim_end().chars().collect();
    let tc = font.tc_px(&chars)? * PIXEL_MM / MM_PER_PT / font.sx;
    tc.is_finite().then_some(tc)
}

/// Where a rotated run's glyphs sit against the `lpDx` positions, the
/// largest miss in device pixels: the driver letters a rotated run one glyph
/// at a time, so each glyph should land on its own cumulative `lpDx`.
fn rotated_miss_px(
    label: &Label,
    face: &FaceTables,
    side: &Side,
    direction: [f64; 2],
) -> Option<f64> {
    if !is_rotated(label) || side.run_glyphs.len() != 1 {
        return None;
    }
    let font = PlotterFont::new(label, face)?;
    let chars: Vec<char> = side
        .glyphs
        .iter()
        .flat_map(|glyph| glyph.text.chars())
        .collect();
    if chars.len() != side.glyphs.len() || chars.len() < 2 {
        return None;
    }
    let first = side.glyphs[0].origin_mm;
    let mut at = 0.0;
    let mut worst: f64 = 0.0;
    for (glyph, step) in side.glyphs.iter().zip(font.lp_dx(&chars)) {
        let delta = [glyph.origin_mm[0] - first[0], glyph.origin_mm[1] - first[1]];
        let along = (delta[0] * direction[0] + delta[1] * direction[1]) / PIXEL_MM;
        worst = worst.max((along - at).abs());
        at += step;
    }
    Some(worst)
}

/// How far below the insertion point `render.dll` runs a label's baseline,
/// millimetres: the line the anchor names, measured on the same 72-unit
/// reference font `a8` scales from — the bottom is the baseline, the middle
/// halfway between its ascent and descent, the top its cap, the ascender its
/// ascent and the descender its descent below.
fn modelled_drop(label: &Label, face: &FaceTables) -> Option<f64> {
    let font = PlotterFont::new(label, face)?;
    let descent72 = 72.0 - font.asc72;
    let line = match label.line.as_str() {
        "Bottom" => 0.0,
        "Middle" => (font.asc72 - descent72) / 2.0,
        "Top" => font.cap72,
        "Ascender" => font.asc72,
        "Descender" => -descent72,
        _ => return None,
    };
    let drop = line * font.a8 * PIXEL_MM;
    drop.is_finite().then_some(drop)
}

/// Why a pair misses, in the order a pair carrying several is listed: the
/// cause that moves the ink furthest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Cause {
    /// The A2 title block's company name and `CPECC`, owned by another task.
    A2TitleBlock,
    /// A glyph the named Latin face lacks, which the reference letters in the
    /// face Windows associates with it (`FontAssoc`: 新宋体) at its own size.
    FontLink,
    /// An Arial Narrow 10 pt equipment tag whose middle the reference puts
    /// at half the ascent, not where the 72-unit model puts it.
    TagMiddle,
    /// The stored value starts with a space the reference letters as a glyph:
    /// it counts in the letter spacing and in the run the anchor centres.
    LeadingPad,
    /// A trailing pad: the box is the whole stored string, the trimmed run is
    /// aligned in it by the paragraph, which the anchor alone does not say.
    TrailingPadParagraph,
    /// A one- or two-glyph run the reference letter-spaces by `lpDx` like a
    /// longer one.
    TwoGlyphSpacing,
    /// A template label whose horizontal scale the plot driver wrote off its
    /// own spacing's scale.
    TemplateScale,
    /// A rotated run, which the reference letters one glyph at a time, each at
    /// its own whole-pixel `lpDx` position.
    RotatedGlyphs,
    /// A baseline the plotter steps down by the 72-unit reference font's own
    /// line, which our face tables put elsewhere.
    LineMetrics,
    /// A centred or right-anchored run the plotter steps back by its
    /// continuous reference-font extent, not by the width it letters.
    AlignedExtent,
    /// A one-glyph run the driver scaled by about 1%.
    OneGlyphScale,
    /// Separately placed labels paired as one chain: each start is rounded to
    /// the reference's pixel grid on its own, so the span across them carries
    /// two roundings.
    ChainQuantization,
    /// Nothing above; read the pair.
    Unexplained,
}

impl Cause {
    fn name(self) -> &'static str {
        match self {
            Self::A2TitleBlock => "a2-title-block",
            Self::LeadingPad => "leading-pad",
            Self::FontLink => "font-link",
            Self::TrailingPadParagraph => "trailing-pad-paragraph",
            Self::RotatedGlyphs => "rotated-glyphs",
            Self::TwoGlyphSpacing => "two-glyph-spacing",
            Self::AlignedExtent => "aligned-extent",
            Self::LineMetrics => "line-metrics",
            Self::TagMiddle => "tag-middle",
            Self::TemplateScale => "template-scale",
            Self::OneGlyphScale => "one-glyph-scale",
            Self::ChainQuantization => "chain-quantization",
            Self::Unexplained => "unexplained",
        }
    }
}

/// Everything the census knows about one pair.
struct Row {
    sheet: &'static str,
    text: String,
    reference_raw: String,
    our_raw: String,
    class: String,
    label: Option<Label>,
    record: Option<RecordFacts>,
    reference: Side,
    ours: Side,
    /// The reference's first run's writing direction.
    direction: [f64; 2],
    along_mm: f64,
    down_mm: f64,
    advance_mm: f64,
    anchor_along_mm: f64,
    position_mm: f64,
    ink: Option<InkShift>,
    /// Where each side's first inked glyph sits from the insertion point,
    /// along the reference's writing direction: the plotter model's, the
    /// reference's and ours, millimetres.
    starts: Option<Starts>,
    flagged: bool,
    causes: Vec<Cause>,
}

#[derive(Clone, Copy, Debug)]
struct Starts {
    model_mm: f64,
    reference_mm: f64,
    ours_mm: f64,
    /// How far below the insertion point each side's baseline runs: the
    /// plotter model's, the reference's and ours.
    model_drop_mm: Option<f64>,
    reference_drop_mm: f64,
    our_drop_mm: f64,
    /// The `lpDx` model's letter spacing for the first run, text-space points.
    model_tc_pt: Option<f64>,
    /// On a rotated run, the reference glyph furthest from its own `lpDx`
    /// position, device pixels.
    rotated_miss_px: Option<f64>,
}

fn inked_start(
    side: &Side,
    insertion: [f64; 2],
    direction: [f64; 2],
    map: &dyn Fn([f64; 2]) -> [f64; 2],
) -> Option<(f64, f64)> {
    let first = side
        .glyphs
        .iter()
        .find(|glyph| !glyph.text.trim().is_empty())?;
    let at = map(first.origin_mm);
    let delta = [at[0] - insertion[0], at[1] - insertion[1]];
    Some((
        delta[0] * direction[0] + delta[1] * direction[1],
        delta[1] * direction[0] - delta[0] * direction[1],
    ))
}

/// Where our inked glyphs sit against the reference's, in its writing frame.
#[derive(Clone, Copy, Debug)]
struct InkShift {
    along_mm: f64,
    down_mm: f64,
    width_mm: f64,
}

fn ink_shift(reference: &Side, ours: &Side, direction: [f64; 2], fit: &Fit) -> Option<InkShift> {
    let reach = |side: &Side, map: &dyn Fn([f64; 2]) -> [f64; 2]| {
        let inked: Vec<&PlacedGlyph> = side
            .glyphs
            .iter()
            .filter(|glyph| !glyph.text.trim().is_empty())
            .collect();
        let (first, last) = (inked.first()?, inked.last()?);
        let start = map(first.origin_mm);
        let end = map(last.origin_mm);
        let span = (end[0] - start[0]) * direction[0] + (end[1] - start[1]) * direction[1];
        Some((start, span + last.advance_mm / fit.scale))
    };
    let (reference_start, reference_width) = reach(reference, &|point| point)?;
    let (our_start, our_width) = reach(ours, &|point| into_reference(point, fit))?;
    let delta = [
        our_start[0] - reference_start[0],
        our_start[1] - reference_start[1],
    ];
    Some(InkShift {
        along_mm: delta[0] * direction[0] + delta[1] * direction[1],
        down_mm: delta[0] * direction[1] - delta[1] * direction[0],
        width_mm: our_width - reference_width,
    })
}

/// Every cause whose evidence the pair carries, most specific first.
fn causes(row: &Row) -> Vec<Cause> {
    let mut out = Vec::new();
    let value = row.label.as_ref().map_or("", |label| label.value.as_str());
    let inked: String = row.text.chars().filter(|c| !c.is_whitespace()).collect();
    if row.sheet == "A2" && (inked.contains("华北分公司") || inked.contains("CPECC")) {
        out.push(Cause::A2TitleBlock);
    }
    let reference_starts_blank = row
        .reference
        .glyphs
        .first()
        .is_some_and(|glyph| glyph.text.trim().is_empty());
    let ours_starts_blank = row
        .ours
        .glyphs
        .first()
        .is_some_and(|glyph| glyph.text.trim().is_empty());
    let padded = row.label.is_none() || value.starts_with(char::is_whitespace);
    if padded && reference_starts_blank && !ours_starts_blank {
        out.push(Cause::LeadingPad);
    }
    let linked = row
        .reference
        .glyphs
        .iter()
        .any(|glyph| glyph.face == "NSimSun");
    // No label on these sheets names 新宋体: the reference only letters in it
    // a glyph the named face lacks.
    let names_nsimsun = row
        .label
        .as_ref()
        .is_some_and(|label| label.face == "新宋体" || label.face == "NSimSun");
    if linked && row.ours.face != row.reference.face && !names_nsimsun {
        out.push(Cause::FontLink);
    }
    if let (Some(label), Some(record)) = (&row.label, &row.record)
        && value.ends_with(char::is_whitespace)
        && let Some(paragraph) = record.paragraph
        && paragraph != label.anchor
        && !(label.anchor == TextAnchor::Left && paragraph == TextAnchor::Right)
        && anchor_fraction(paragraph) != anchor_fraction(label.anchor)
    {
        out.push(Cause::TrailingPadParagraph);
    }
    if row.direction[1].abs() > 0.5 {
        out.push(Cause::RotatedGlyphs);
    }
    // A two-glyph run on either side whose one gap the other side spaces
    // differently. The inked glyphs of both chains spell the same text, so
    // they pair one for one.
    let reference_inked: Vec<usize> = (0..row.reference.glyphs.len())
        .filter(|index| !row.reference.glyphs[*index].text.trim().is_empty())
        .collect();
    let our_inked: Vec<usize> = (0..row.ours.glyphs.len())
        .filter(|index| !row.ours.glyphs[*index].text.trim().is_empty())
        .collect();
    let two_glyph_gap = |this: &Side, inked: &[usize], other: &Side, other_inked: &[usize]| {
        let mut at = 0usize;
        for count in &this.run_glyphs {
            if *count == 2
                && inked.len() == other_inked.len()
                && let Some(position) = inked.iter().position(|index| *index == at)
                && let (Some(gap), Some(other_gap)) = (
                    this.gaps_pt.get(at),
                    other_inked
                        .get(position)
                        .and_then(|index| other.gaps_pt.get(*index)),
                )
                && gap.is_finite()
                && other_gap.is_finite()
                && (gap - other_gap).abs() > TC_PT
            {
                return true;
            }
            at += count;
        }
        false
    };
    // Two labels the reader merged into one run hide a two-glyph label's
    // run boundary, but not its spacing: our writer leaves exactly no letter
    // spacing only on a run of fewer than three glyphs.
    let unspaced = reference_inked.len() == our_inked.len()
        && reference_inked
            .iter()
            .zip(&our_inked)
            .any(|(reference_index, our_index)| {
                let (Some(reference_gap), Some(our_gap)) = (
                    row.reference.gaps_pt.get(*reference_index),
                    row.ours.gaps_pt.get(*our_index),
                ) else {
                    return false;
                };
                reference_gap.is_finite()
                    && our_gap.is_finite()
                    && our_gap.abs() < 1e-4
                    && (reference_gap - our_gap).abs() > TC_PT
            });
    if unspaced
        || two_glyph_gap(&row.reference, &reference_inked, &row.ours, &our_inked)
        || two_glyph_gap(&row.ours, &our_inked, &row.reference, &reference_inked)
    {
        out.push(Cause::TwoGlyphSpacing);
    }
    // The model explains where the reference starts, and ours starts
    // elsewhere along the baseline.
    if let (Some(label), Some(starts)) = (&row.label, row.starts)
        && label.anchor != TextAnchor::Left
        && (starts.reference_mm - starts.model_mm).abs() <= PIXEL_MM * 0.75
        && (starts.ours_mm - starts.model_mm).abs() > 0.03
    {
        out.push(Cause::AlignedExtent);
    }
    if let Some(starts) = row.starts
        && let Some(model) = starts.model_drop_mm
    {
        let reference_off = starts.reference_drop_mm - model;
        if reference_off.abs() <= PIXEL_MM * 0.75 && (starts.our_drop_mm - model).abs() > 0.03 {
            out.push(Cause::LineMetrics);
        } else if reference_off.abs() > 0.3
            && row
                .label
                .as_ref()
                .is_some_and(|label| label.line == "Middle" && label.height_mm > 3.0)
        {
            out.push(Cause::TagMiddle);
        }
    }
    if let (Some(label), Some(ink)) = (&row.label, row.ink)
        && label.kind == "template"
        && (row.reference.sx - row.ours.sx).abs() > 0.0005
        && ink.width_mm.abs() > WIDTH_MM
    {
        out.push(Cause::TemplateScale);
    }
    let scale = if row.ours.body_pt > 0.0 {
        row.reference.body_pt / row.ours.body_pt
    } else {
        1.0
    };
    if reference_inked.len() == 1 && (1.005..1.015).contains(&scale) {
        out.push(Cause::OneGlyphScale);
    }
    let chained = row.reference.run_glyphs.len() > 1 || row.ours.run_glyphs.len() > 1;
    if out.is_empty()
        && chained
        && row
            .ink
            .is_some_and(|ink| ink.width_mm.abs() <= 2.0 * PIXEL_MM)
    {
        out.push(Cause::ChainQuantization);
    }
    if out.is_empty() {
        out.push(Cause::Unexplained);
    }
    out.sort();
    out.dedup();
    out
}

fn chain_text(ink: &PlotInk, runs: &[usize], separator: &str) -> String {
    runs.iter()
        .map(|run| ink.spans[*run].text.as_str())
        .collect::<Vec<_>>()
        .join(separator)
}

fn tsv_header() -> &'static str {
    "sheet\ttext\tglyphs\tclass\tkind\tstream\tbytes\tsymbol\tvalue\tface\theight_mm\tbasis\twidth_permille\trotation\tanchor_code\tparagraph\tlettering\truns\tref_face\tref_tf\tref_sx\tref_tc\tour_face\tour_tf\tour_sx\tour_tc\talong_mm\tdown_mm\tadvance_mm\tanchor_along_mm\tposition_mm\tink_along_mm\tink_down_mm\tink_width_mm\tmodel_start_mm\tref_start_mm\tour_start_mm\tmodel_drop_mm\tref_drop_mm\tour_drop_mm\tmodel_tc\trotated_miss_px\tflagged\tcause\talso\tref_runs\tour_runs\tref_gaps_pt\tour_gaps_pt\n"
}

fn tsv_line(row: &Row) -> String {
    let label = row.label.as_ref();
    let record = row.record.as_ref();
    let ink = row.ink;
    let causes: Vec<&str> = row.causes.iter().map(|cause| cause.name()).collect();
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:?}\t{}\t{:.3}\t{}\t{}\t{:.0}\t{}\t{}\t{}\t{}/{}\t{}\t{:.3}\t{:.4}\t{:+.4}\t{}\t{:.3}\t{:.4}\t{:+.4}\t{:+.4}\t{:+.4}\t{:+.4}\t{:+.4}\t{:.4}\t{:+.4}\t{:+.4}\t{:+.4}\t{:+.4}\t{:+.4}\t{:+.4}\t{:+.4}\t{:+.4}\t{:+.4}\t{:+.4}\t{:.2}\t{}\t{}\t{}\t{:?}\t{:?}\t{}\t{}\n",
        row.sheet,
        row.text,
        row.reference.glyphs.len(),
        row.class,
        label.map_or("unknown", |label| label.kind),
        label.map_or("", |label| label.stream.as_str()),
        label
            .and_then(|label| label.bytes)
            .map_or_else(String::new, |range| format!("{}..{}", range[0], range[1])),
        label.map_or("", |label| label.symbol.as_str()),
        label.map_or("", |label| label.value.as_str()),
        label.map_or("", |label| label.face.as_str()),
        label.map_or(f64::NAN, |label| label.height_mm),
        label.map_or("", |label| label.basis.as_str()),
        label
            .and_then(|label| label.width_permille)
            .map_or_else(|| "-".to_string(), |width| width.to_string()),
        label.map_or(f64::NAN, |label| label.rotation_deg),
        record.map_or_else(
            || "-".to_string(),
            |record| format!("{:#04x}", record.anchor_code)
        ),
        record
            .and_then(|record| record.paragraph)
            .map_or_else(|| "-".to_string(), |paragraph| format!("{paragraph:?}")),
        record.map_or("-", |record| record.lettering.as_str()),
        row.reference.run_glyphs.len(),
        row.ours.run_glyphs.len(),
        row.reference.face,
        row.reference.body_pt,
        row.reference.sx,
        row.reference.tc_pt,
        row.ours.face,
        row.ours.body_pt,
        row.ours.sx,
        row.ours.tc_pt,
        row.along_mm,
        row.down_mm,
        row.advance_mm,
        row.anchor_along_mm,
        row.position_mm,
        ink.map_or(f64::NAN, |ink| ink.along_mm),
        ink.map_or(f64::NAN, |ink| ink.down_mm),
        ink.map_or(f64::NAN, |ink| ink.width_mm),
        row.starts.map_or(f64::NAN, |starts| starts.model_mm),
        row.starts.map_or(f64::NAN, |starts| starts.reference_mm),
        row.starts.map_or(f64::NAN, |starts| starts.ours_mm),
        row.starts
            .and_then(|starts| starts.model_drop_mm)
            .unwrap_or(f64::NAN),
        row.starts
            .map_or(f64::NAN, |starts| starts.reference_drop_mm),
        row.starts.map_or(f64::NAN, |starts| starts.our_drop_mm),
        row.starts
            .and_then(|starts| starts.model_tc_pt)
            .unwrap_or(f64::NAN),
        row.starts
            .and_then(|starts| starts.rotated_miss_px)
            .unwrap_or(f64::NAN),
        u8::from(row.flagged),
        causes.first().copied().unwrap_or("-"),
        causes.get(1..).map_or(String::new(), |rest| rest.join("+")),
        row.reference_raw,
        row.our_raw,
        gaps(&row.reference),
        gaps(&row.ours),
    )
}

/// How well the plotter model reads the reference, over every pair it covers:
/// the horizontal start, the baseline drop, and each run's letter spacing.
/// A reading within one rounding of the reference's 1/150 in grid is the
/// model's; the rest are listed by cause.
fn model_check(rows: &[Row]) {
    let summary = |name: &str, values: &[f64], bound: f64| {
        let within = values.iter().filter(|value| value.abs() <= bound).count();
        let mean = values.iter().sum::<f64>() / values.len().max(1) as f64;
        let spread = (values
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / values.len().max(1) as f64)
            .sqrt();
        println!(
            "model {name:<22} {within:>5}/{:<5} within {bound:.3}, mean {mean:+.4}, sd {spread:.4}",
            values.len()
        );
    };
    let starts: Vec<f64> = rows
        .iter()
        .filter_map(|row| row.starts)
        .map(|starts| starts.reference_mm - starts.model_mm)
        .filter(|value| value.is_finite())
        .collect();
    summary("start (ref - model mm)", &starts, 0.12);
    let drops: Vec<f64> = rows
        .iter()
        .filter_map(|row| row.starts)
        .filter_map(|starts| Some(starts.reference_drop_mm - starts.model_drop_mm?))
        .filter(|value| value.is_finite())
        .collect();
    summary("drop (ref - model mm)", &drops, 0.12);
    for (name, short) in [
        ("tc n=2 (ref - model pt)", true),
        ("tc n>2 (ref - model pt)", false),
    ] {
        let spacing: Vec<f64> = rows
            .iter()
            .filter(|row| row.reference.run_glyphs.len() == 1 && row.ours.run_glyphs.len() == 1)
            .filter(|row| row.direction[1].abs() < 0.5)
            .filter(|row| (row.reference.run_glyphs[0] == 2) == short)
            .filter_map(|row| Some(row.reference.tc_pt - row.starts?.model_tc_pt?))
            .filter(|value| value.is_finite())
            .collect();
        summary(name, &spacing, 0.005);
    }
    for (name, short) in [("rotated n=2 (px)", true), ("rotated n>2 (px)", false)] {
        let rotated: Vec<f64> = rows
            .iter()
            .filter(|row| (row.reference.glyphs.len() == 2) == short)
            .filter_map(|row| row.starts?.rotated_miss_px)
            .filter(|value| value.is_finite())
            .collect();
        summary(name, &rotated, 0.5);
    }
    // Per lettering class: how far the reference sits from the model (the
    // model's own error, one pixel rounding if it is right) and how far ours
    // does (the rule this crate gets wrong, if it is not zero).
    let mut classes: BTreeMap<(String, String), Vec<&Starts>> = BTreeMap::new();
    for row in rows {
        if let Some(starts) = &row.starts {
            let class = row
                .class
                .rsplit('~')
                .next()
                .unwrap_or(&row.class)
                .to_string();
            classes
                .entry((class, row.reference.face.clone()))
                .or_default()
                .push(starts);
        }
    }
    println!(
        "{:<32} {:>5}  {:>22}  {:>22}",
        "class / face", "n", "start ref-m sd ours-m", "drop ref-m sd ours-m"
    );
    for ((class, face), members) in &classes {
        if members.len() < 5 {
            continue;
        }
        let stats = |pick: &dyn Fn(&Starts) -> Option<(f64, f64)>| {
            let pairs: Vec<(f64, f64)> = members
                .iter()
                .filter_map(|starts| pick(starts))
                .filter(|(reference, ours)| reference.is_finite() && ours.is_finite())
                .collect();
            let n = pairs.len().max(1) as f64;
            let mean = pairs.iter().map(|(reference, _)| reference).sum::<f64>() / n;
            let spread = (pairs
                .iter()
                .map(|(reference, _)| (reference - mean).powi(2))
                .sum::<f64>()
                / n)
                .sqrt();
            let ours = pairs.iter().map(|(_, ours)| ours).sum::<f64>() / n;
            format!("{mean:+.3} {spread:.3} {ours:+.3}")
        };
        println!(
            "{:<32} {:>5}  {:>22}  {:>22}",
            format!("{class} / {face}"),
            members.len(),
            stats(&|starts| Some((
                starts.reference_mm - starts.model_mm,
                starts.ours_mm - starts.model_mm
            ))),
            stats(&|starts| {
                let model = starts.model_drop_mm?;
                Some((starts.reference_drop_mm - model, starts.our_drop_mm - model))
            }),
        );
    }
}

/// A side's letter spacing glyph by glyph, `|` between runs.
fn gaps(side: &Side) -> String {
    side.gaps_pt
        .iter()
        .map(|gap| {
            if gap.is_finite() {
                format!("{gap:+.3}")
            } else {
                "|".to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn main() -> ExitCode {
    let corpus = std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    );
    let out_dir = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("target/text-census"), PathBuf::from);
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
    let faces: Vec<FaceTables> = ["Arial", "Arial Narrow", "SimSun", "FangSong", "NSimSun"]
        .into_iter()
        .filter_map(|key| FaceTables::load(key, &dirs))
        .collect();
    let detail = std::env::var("CAUSE").ok();
    let mut rows: Vec<Row> = Vec::new();
    let (mut grid_on, mut grid_total) = (0usize, 0usize);
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
        // The reference starts every run on a whole 1/150 in pixel, counted
        // from the page's bottom-left corner; the glyphs after the first sit
        // where the run's `Tc` puts them.
        let on_grid = |mm: f64| {
            let units = mm / MM_PER_PT / 0.48;
            (units - units.round()).abs() < 1e-3
        };
        let inked: Vec<&TextSpan> = reference
            .spans
            .iter()
            .filter(|span| !span.text.trim().is_empty())
            .collect();
        grid_total += inked.len();
        grid_on += inked
            .iter()
            .filter(|span| on_grid(span.origin_mm[0]) && on_grid(span.origin_mm[1]))
            .count();
        let reference_page = PageGlyphs::new(reference_glyphs);
        let our_page = PageGlyphs::new(our_glyphs);
        let missing: Vec<MaskRect> = model
            .missing_boxes
            .iter()
            .map(|missing| MaskRect {
                label: missing.path.clone(),
                min_mm: missing.min_mm,
                max_mm: missing.max_mm,
            })
            .collect();
        let report = compare_plots_with_model(&reference, &ours, &[], &missing, model);
        let records = record_facts(&corpus.join(format!("{name}.pid")));
        for pair in &report.text_pairs {
            let reference_side = side(&reference, &pair.reference_runs, &reference_page);
            let our_side = side(&ours, &pair.our_runs, &our_page);
            let label = find_label(model, pair, &ours, &report.fit);
            let record = label.as_ref().and_then(|label| {
                let range = label.bytes?;
                records.get(&(label.stream.clone(), range[0])).cloned()
            });
            let fraction = label
                .as_ref()
                .map_or(0.0, |label| anchor_fraction(label.anchor));
            let anchor_along_mm = pair.along_mm + fraction * pair.advance_mm;
            let position_mm = anchor_along_mm.hypot(pair.down_mm);
            let direction = reference.spans[pair.reference_runs[0]].direction;
            let fit = &report.fit;
            let starts = label.as_ref().and_then(|label| {
                let key = pid_plot::fonts::typeface_key(Some(&label.face), &label.value);
                let face = faces.iter().find(|face| face.key == key)?;
                let run_text = &ours.spans[*pair.our_runs.first()?].text;
                let model_mm = modelled_ink_start(label, record.as_ref(), face, run_text)?;
                let insertion = into_reference(label.origin_mm, fit);
                let (reference_mm, reference_up) =
                    inked_start(&reference_side, insertion, direction, &|point| point)?;
                let (ours_mm, our_up) = inked_start(&our_side, insertion, direction, &|point| {
                    into_reference(point, fit)
                })?;
                Some(Starts {
                    model_mm,
                    reference_mm,
                    ours_mm,
                    model_drop_mm: modelled_drop(label, face),
                    reference_drop_mm: -reference_up,
                    our_drop_mm: -our_up,
                    model_tc_pt: modelled_tc(
                        label,
                        face,
                        &reference.spans[pair.reference_runs[0]].text,
                    ),
                    rotated_miss_px: rotated_miss_px(label, face, &reference_side, direction),
                })
            });
            let mut row = Row {
                sheet,
                text: chain_text(&reference, &pair.reference_runs, "|")
                    .trim()
                    .to_string(),
                reference_raw: chain_text(&reference, &pair.reference_runs, "+"),
                our_raw: chain_text(&ours, &pair.our_runs, "+"),
                class: match (&pair.source, &label) {
                    (None, Some(label)) => format!(
                        "unknown~{:?}/{}",
                        label.anchor,
                        if label.kind == "symbol" {
                            "symbol"
                        } else {
                            &label.line
                        }
                    ),
                    _ => pair.class.clone(),
                },
                ink: ink_shift(&reference_side, &our_side, direction, &report.fit),
                starts,
                label,
                record,
                reference: reference_side,
                ours: our_side,
                direction,
                along_mm: pair.along_mm,
                down_mm: pair.down_mm,
                advance_mm: pair.advance_mm,
                anchor_along_mm,
                position_mm,
                flagged: position_mm > POSITION_MM || pair.advance_mm.abs() > WIDTH_MM,
                causes: Vec::new(),
            };
            row.causes = causes(&row);
            rows.push(row);
        }
        let paired = rows.iter().filter(|row| row.sheet == *sheet).count();
        let flagged = rows
            .iter()
            .filter(|row| row.sheet == *sheet && row.flagged)
            .count();
        println!(
            "{sheet:<7} labels {:>4}/{:<4} pairs {paired:>4} flagged {flagged:>4}",
            report.text_matched, report.text_reference
        );
    }
    let mut all = String::from(tsv_header());
    let mut flagged = String::from(tsv_header());
    for row in &rows {
        let line = tsv_line(row);
        all.push_str(&line);
        if row.flagged {
            flagged.push_str(&line);
        }
    }
    if std::fs::create_dir_all(&out_dir).is_err()
        || std::fs::write(out_dir.join("pairs.tsv"), all).is_err()
        || std::fs::write(out_dir.join("flagged.tsv"), flagged).is_err()
    {
        eprintln!("could not write into {}", out_dir.display());
        return ExitCode::from(1);
    }
    let total = rows.len();
    let flagged_total = rows.iter().filter(|row| row.flagged).count();
    println!("pairs {total} flagged {flagged_total}");
    println!("reference run origins on the 1/150 in grid: {grid_on}/{grid_total}");
    model_check(&rows);
    println!(
        "{:<24} {:>7} {:>8} {:>9} {:>9} {:>9}  example",
        "cause", "primary", "involved", "ink_al", "ink_dn", "ink_w"
    );
    let mut by_cause: BTreeMap<Cause, Vec<&Row>> = BTreeMap::new();
    let mut involved: BTreeMap<Cause, usize> = BTreeMap::new();
    for row in rows.iter().filter(|row| row.flagged) {
        by_cause.entry(row.causes[0]).or_default().push(row);
        for cause in &row.causes {
            *involved.entry(*cause).or_default() += 1;
        }
    }
    for (cause, members) in &by_cause {
        let mean = |pick: fn(&InkShift) -> f64| {
            let values: Vec<f64> = members
                .iter()
                .filter_map(|row| row.ink.as_ref().map(pick))
                .collect();
            if values.is_empty() {
                f64::NAN
            } else {
                values.iter().sum::<f64>() / values.len() as f64
            }
        };
        let example = members
            .first()
            .map_or_else(String::new, |row| format!("{} {:?}", row.sheet, row.text));
        println!(
            "{:<24} {:>7} {:>8} {:>+9.3} {:>+9.3} {:>+9.3}  {example}",
            cause.name(),
            members.len(),
            involved.get(cause).copied().unwrap_or(0),
            mean(|ink| ink.along_mm),
            mean(|ink| ink.down_mm),
            mean(|ink| ink.width_mm),
        );
        let per_sheet: Vec<String> = SHEETS
            .iter()
            .map(|(sheet, _)| {
                let count = members.iter().filter(|row| row.sheet == *sheet).count();
                format!("{sheet} {count}")
            })
            .collect();
        println!("{:<24} {}", "", per_sheet.join(", "));
        if detail.as_deref() == Some(cause.name()) {
            for row in members {
                let mut line = String::new();
                let _ = write!(line, "    {}", tsv_line(row));
                print!("{line}");
            }
        }
    }
    ExitCode::SUCCESS
}
