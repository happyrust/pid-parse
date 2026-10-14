//! Write a [`PaperModel`](crate::model::PaperModel) to a one-page PDF.
//!
//! Decoded and Inferred primitives go on two optional-content groups. ProbeOnly
//! is left off the page. Strokes are a constant 0.25 mm: the paper table does
//! not carry line style yet, and phase one does not compare width. Arcs and
//! circles are flattened at the same 0.05 mm chord the reader uses, then stored
//! as straight segments, so a read-back can match the table endpoint for endpoint.
//!
//! Each text run is set in the face [`crate::fonts::typeface_key`] names.
//! A style height that was measured on the reference plots is written at
//! that plot's `Tf`; every other height stays the style height in points.
//! Every face must be a static TrueType or OpenType program pdfcraft will
//! embed: under 16 MiB, not a collection, not a variable font.

use std::f64::consts::TAU;

use kurbo::{Arc, BezPath, PathEl, flatten};
use pdfcraft_content::{Op, serialize_ops};
use pdfcraft_cos::{Dict, Document, Object, PdfString, Stream};
use pdfcraft_fonts::embedding::{self, embed};
use pdfcraft_fonts::pdf::Metrics;

use crate::fonts::typeface_key;
use crate::model::{PaperModel, PaperShape, TextAnchor};
use crate::read::CHORD_MM;

const PT_PER_MM: f64 = 72.0 / 25.4;
const STROKE_MM: f64 = 0.25;
/// Body size for symbol text whose source record does not carry one.
pub const DEFAULT_TEXT_MM: f64 = 2.5;

/// One face [`write_plot`] may select for a text run.
pub struct PlotFont<'a> {
    /// Canonical name from [`typeface_key`]. Empty means last resort only.
    pub key: &'a str,
    /// Static TrueType or OpenType bytes.
    pub bytes: &'a [u8],
}

/// Why a paper model could not be written.
#[derive(Debug)]
pub enum PlotWriteError {
    /// The drawing does not state a page size.
    NoPage,
    /// The font bytes cannot be embedded.
    Font(String),
    /// `character` is not in the embedded font.
    MissingGlyph(char),
    /// pdfcraft could not save the file.
    Save(String),
}

impl std::fmt::Display for PlotWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPage => write!(f, "the drawing has no page size"),
            Self::Font(message) => write!(f, "font: {message}"),
            Self::MissingGlyph(ch) => write!(f, "the font has no glyph for {ch:?}"),
            Self::Save(message) => write!(f, "save: {message}"),
        }
    }
}

impl std::error::Error for PlotWriteError {}

/// Write `model` to PDF bytes, setting each run in the matching face.
pub fn write_plot(model: &PaperModel, fonts: &[PlotFont<'_>]) -> Result<Vec<u8>, PlotWriteError> {
    if fonts.is_empty() {
        return Err(PlotWriteError::Font("no font was provided".into()));
    }
    let [page_w, page_h] = resolved_page(model)?;

    let mut doc = Document::new_empty();
    let mut font_resources = Dict::new();
    let mut ready = Vec::new();
    for font in fonts {
        let (family, _) = embedding::names(font.bytes).map_err(PlotWriteError::Font)?;
        let resource = embed(&mut doc, &mut font_resources, font.bytes, &family)
            .map_err(PlotWriteError::Font)?;
        let font_ref = font_resources
            .get(resource.as_bytes())
            .cloned()
            .ok_or_else(|| {
                PlotWriteError::Font("embedded font is missing from the resource dict".into())
            })?;
        let font_dict = doc
            .dict(&font_ref)
            .ok_or_else(|| PlotWriteError::Font("embedded font is not a dictionary".into()))?;
        ready.push(ReadyFace {
            key: font.key,
            resource,
            metrics: Metrics::from_dict(&doc, &font_dict),
        });
    }
    let fonts = ready;

    let decoded_ocg = add_ocg(&mut doc, "Decoded");
    let inferred_ocg = add_ocg(&mut doc, "Inferred");
    let root = doc
        .root()
        .ok_or_else(|| PlotWriteError::Save("the catalog is missing".into()))?;
    doc.update_dict(root, |catalog| {
        let mut default = Dict::new();
        let on = Object::Array(vec![Object::Ref(decoded_ocg), Object::Ref(inferred_ocg)]);
        default.set(b"ON".to_vec(), on.clone());
        default.set(b"Order".to_vec(), on);
        let mut props = Dict::new();
        props.set(
            b"OCGs".to_vec(),
            Object::Array(vec![Object::Ref(decoded_ocg), Object::Ref(inferred_ocg)]),
        );
        props.set(b"D".to_vec(), Object::Dict(default));
        catalog.set(b"OCProperties".to_vec(), Object::Dict(props));
    })
    .map_err(|error| PlotWriteError::Save(error.to_string()))?;

    let mut ops = Vec::new();
    let page = [page_w, page_h];
    push_layer(&mut ops, model, &fonts, "decoded", "Decoded", page)?;
    push_layer(&mut ops, model, &fonts, "inferred", "Inferred", page)?;

    let contents = doc.add(Object::Stream(Stream::flate(
        Dict::new(),
        &serialize_ops(&ops),
    )));
    let mut resources = Dict::new();
    resources.set(b"Font".to_vec(), Object::Dict(font_resources));
    let mut properties = Dict::new();
    properties.set(b"Decoded".to_vec(), Object::Ref(decoded_ocg));
    properties.set(b"Inferred".to_vec(), Object::Ref(inferred_ocg));
    resources.set(b"Properties".to_vec(), Object::Dict(properties));

    let pages = pages_ref(&doc)?;
    let mut page = Dict::new();
    page.set(b"Type".to_vec(), Object::name("Page"));
    page.set(b"Parent".to_vec(), Object::Ref(pages));
    page.set(
        b"MediaBox".to_vec(),
        Object::Array(vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(page_w * PT_PER_MM),
            Object::Real(page_h * PT_PER_MM),
        ]),
    );
    page.set(b"Contents".to_vec(), Object::Ref(contents));
    page.set(b"Resources".to_vec(), Object::Dict(resources));
    let page_ref = doc.add(Object::Dict(page));
    doc.update_dict(pages, |dict| {
        let mut kids = match dict.get(b"Kids") {
            Some(Object::Array(kids)) => kids.clone(),
            _ => Vec::new(),
        };
        kids.push(Object::Ref(page_ref));
        let count = i64::try_from(kids.len()).unwrap_or(i64::MAX);
        dict.set(b"Kids".to_vec(), Object::Array(kids));
        dict.set(b"Count".to_vec(), Object::Int(count));
    })
    .map_err(|error| PlotWriteError::Save(error.to_string()))?;

    pdfcraft_cos::write_full(&doc, &pdfcraft_cos::SaveOptions::default())
        .map_err(|error| PlotWriteError::Save(error.to_string()))
}

fn add_ocg(doc: &mut Document, name: &str) -> pdfcraft_cos::ObjRef {
    let mut ocg = Dict::new();
    ocg.set(b"Type".to_vec(), Object::name("OCG"));
    ocg.set(b"Name".to_vec(), Object::String(PdfString::text(name)));
    doc.add(Object::Dict(ocg))
}

fn pages_ref(doc: &Document) -> Result<pdfcraft_cos::ObjRef, PlotWriteError> {
    let root = doc
        .root()
        .ok_or_else(|| PlotWriteError::Save("the catalog is missing".into()))?;
    let catalog = doc
        .dict(&Object::Ref(root))
        .ok_or_else(|| PlotWriteError::Save("the catalog is not a dictionary".into()))?;
    match catalog.get(b"Pages") {
        Some(Object::Ref(pages)) => Ok(*pages),
        _ => Err(PlotWriteError::Save("the page tree is missing".into())),
    }
}

struct ReadyFace<'a> {
    key: &'a str,
    resource: String,
    metrics: Metrics,
}

fn push_layer(
    ops: &mut Vec<Op>,
    model: &PaperModel,
    fonts: &[ReadyFace<'_>],
    confidence: &str,
    property: &str,
    page: [f64; 2],
) -> Result<(), PlotWriteError> {
    let primitives: Vec<_> = model
        .primitives
        .iter()
        .filter(|primitive| primitive.confidence == confidence)
        .collect();
    if primitives.is_empty() {
        return Ok(());
    }
    ops.push(Op::new(
        "BDC",
        vec![Object::name("OC"), Object::name(property)],
    ));
    ops.push(Op::new("q", Vec::new()));
    ops.push(Op::new(
        "RG",
        vec![Object::Real(0.0), Object::Real(0.0), Object::Real(0.0)],
    ));
    ops.push(Op::new(
        "rg",
        vec![Object::Real(0.0), Object::Real(0.0), Object::Real(0.0)],
    ));
    ops.push(Op::new("w", vec![Object::Real(STROKE_MM * PT_PER_MM)]));
    ops.push(Op::new("J", vec![Object::Int(0)]));
    ops.push(Op::new("j", vec![Object::Int(0)]));

    let mut stroked = false;
    for primitive in &primitives {
        if paint_shape(ops, &primitive.shape, page) {
            stroked = true;
        }
    }
    if stroked {
        ops.push(Op::new("S", Vec::new()));
    }
    for primitive in &primitives {
        if let PaperShape::Text {
            origin_mm,
            height_mm,
            rotation,
            value,
            font,
            anchor,
        } = &primitive.shape
        {
            let face = face_for(fonts, font.as_deref(), value)?;
            show_text(
                ops,
                &face.metrics,
                ShownText {
                    resource: &face.resource,
                    typeface: typeface_key(font.as_deref(), value),
                    origin_mm: *origin_mm,
                    height_mm: *height_mm,
                    rotation: *rotation,
                    value,
                    anchor: *anchor,
                },
            )?;
        }
    }
    ops.push(Op::new("Q", Vec::new()));
    ops.push(Op::new("EMC", Vec::new()));
    Ok(())
}

fn paint_shape(ops: &mut Vec<Op>, shape: &PaperShape, page: [f64; 2]) -> bool {
    match shape {
        PaperShape::Line { start_mm, end_mm } => segment(ops, *start_mm, *end_mm, page),
        PaperShape::Polyline { points_mm, closed } => {
            let mut any = false;
            for pair in points_mm.windows(2) {
                any |= segment(ops, pair[0], pair[1], page);
            }
            if *closed && points_mm.len() >= 2 {
                any |= segment(ops, points_mm[points_mm.len() - 1], points_mm[0], page);
            }
            any
        }
        PaperShape::Arc {
            center_mm,
            radius_mm,
            start_angle,
            end_angle,
        } => chain(
            ops,
            &arc_points(
                *center_mm,
                *radius_mm,
                *start_angle,
                clockwise_sweep(*start_angle, *end_angle),
            ),
            page,
        ),
        PaperShape::Circle {
            center_mm,
            radius_mm,
        } => chain(ops, &arc_points(*center_mm, *radius_mm, 0.0, -TAU), page),
        PaperShape::Text { .. } => false,
    }
}

fn segment(ops: &mut Vec<Op>, start: [f64; 2], end: [f64; 2], page: [f64; 2]) -> bool {
    if !finite(start)
        || !finite(end)
        || hypot(start, end) < 1e-4
        || reader_sheet_edge(start, end, page)
    {
        return false;
    }
    ops.push(Op::new("m", vec![pt(start[0]), pt(start[1])]));
    ops.push(Op::new("l", vec![pt(end[0]), pt(end[1])]));
    true
}

fn chain(ops: &mut Vec<Op>, points: &[[f64; 2]], page: [f64; 2]) -> bool {
    let mut any = false;
    for pair in points.windows(2) {
        any |= segment(ops, pair[0], pair[1], page);
    }
    any
}

fn face_for<'a>(
    faces: &'a [ReadyFace<'a>],
    requested: Option<&str>,
    text: &str,
) -> Result<&'a ReadyFace<'a>, PlotWriteError> {
    let key = typeface_key(requested, text);
    if let Some(face) = faces
        .iter()
        .find(|face| face.key == key && face.metrics.encode(text).is_some())
    {
        return Ok(face);
    }
    faces
        .iter()
        .find(|face| face.metrics.encode(text).is_some())
        .ok_or_else(|| {
            let have: Vec<&str> = faces.iter().map(|face| face.key).collect();
            PlotWriteError::Font(format!(
                "no face can draw {text:?} (wanted {key}; embedded {})",
                have.join(", ")
            ))
        })
}

struct ShownText<'a> {
    resource: &'a str,
    typeface: &'a str,
    origin_mm: [f64; 2],
    height_mm: Option<f64>,
    rotation: f64,
    value: &'a str,
    anchor: TextAnchor,
}

fn show_text(
    ops: &mut Vec<Op>,
    metrics: &Metrics,
    text: ShownText<'_>,
) -> Result<(), PlotWriteError> {
    if text.value.is_empty() || !text.rotation.is_finite() || !finite(text.origin_mm) {
        return Ok(());
    }
    let height_mm = text.height_mm.unwrap_or(DEFAULT_TEXT_MM);
    if !height_mm.is_finite() || height_mm <= 0.0 {
        return Ok(());
    }
    let bytes = metrics
        .encode(text.value)
        .ok_or_else(|| missing_glyph(metrics, text.value))?;
    let size = plotted_size_pt(text.typeface, height_mm);
    let origin_mm = baseline_origin(
        text.origin_mm,
        text.rotation,
        run_width_mm(metrics, size, &bytes),
        text.anchor,
    );
    let (sin, cos) = text.rotation.sin_cos();
    ops.push(Op::new("BT", Vec::new()));
    ops.push(Op::new(
        "Tf",
        vec![Object::name(text.resource), Object::Real(size)],
    ));
    ops.push(Op::new(
        "Tm",
        vec![
            Object::Real(cos),
            Object::Real(sin),
            Object::Real(-sin),
            Object::Real(cos),
            pt(origin_mm[0]),
            pt(origin_mm[1]),
        ],
    ));
    ops.push(Op::new(
        "Tj",
        vec![Object::String(PdfString::literal(bytes))],
    ));
    ops.push(Op::new("ET", Vec::new()));
    Ok(())
}

/// Advance of `bytes` at `size_pt`, in paper millimetres.
fn run_width_mm(metrics: &Metrics, size_pt: f64, bytes: &[u8]) -> f64 {
    let units: f64 = metrics
        .codes(bytes)
        .iter()
        .map(|(code, _)| metrics.width(*code))
        .sum();
    units * size_pt / PT_PER_MM
}

/// Left end of the baseline. Centre and right step back along the run.
fn baseline_origin(
    origin_mm: [f64; 2],
    rotation: f64,
    width_mm: f64,
    anchor: TextAnchor,
) -> [f64; 2] {
    let fraction = match anchor {
        TextAnchor::Left => 0.0,
        TextAnchor::Center => 0.5,
        TextAnchor::Right => 1.0,
    };
    if fraction == 0.0 || !width_mm.is_finite() || width_mm <= 0.0 {
        return origin_mm;
    }
    let (sin, cos) = rotation.sin_cos();
    let shift = width_mm * fraction;
    [origin_mm[0] - cos * shift, origin_mm[1] - sin * shift]
}

fn missing_glyph(metrics: &Metrics, value: &str) -> PlotWriteError {
    value
        .chars()
        .find(|ch| metrics.encode(ch.encode_utf8(&mut [0; 4])).is_none())
        .map_or(
            PlotWriteError::Font("the font could not encode the text".into()),
            PlotWriteError::MissingGlyph,
        )
}

fn arc_points(center: [f64; 2], radius: f64, start: f64, sweep: f64) -> Vec<[f64; 2]> {
    if !center[0].is_finite()
        || !center[1].is_finite()
        || !radius.is_finite()
        || radius <= 0.0
        || !sweep.is_finite()
        || sweep.abs() < 1e-8
    {
        return Vec::new();
    }
    let arc = Arc::new(
        kurbo::Point::new(center[0], center[1]),
        kurbo::Vec2::new(radius, radius),
        start,
        sweep,
        0.0,
    );
    let mut path = BezPath::new();
    path.move_to(kurbo::Point::new(
        center[0] + radius * start.cos(),
        center[1] + radius * start.sin(),
    ));
    for element in arc.append_iter(CHORD_MM) {
        path.push(element);
    }
    let mut points = Vec::new();
    flatten(path, CHORD_MM, |element| match element {
        PathEl::MoveTo(point) | PathEl::LineTo(point) => points.push([point.x, point.y]),
        PathEl::ClosePath | PathEl::QuadTo(_, _) | PathEl::CurveTo(_, _, _) => {}
    });
    if sweep.abs() > TAU - 1e-6
        && points.len() >= 2
        && hypot(points[0], points[points.len() - 1]) > 1e-4
    {
        points.push(points[0]);
    }
    points
}

fn clockwise_sweep(start: f64, end: f64) -> f64 {
    let ccw = (end - start).rem_euclid(TAU);
    if ccw < 1e-12 { 0.0 } else { ccw - TAU }
}

fn resolved_page(model: &PaperModel) -> Result<[f64; 2], PlotWriteError> {
    if let Some(page) = model.page_mm {
        return Ok(page);
    }
    let mut max_x = 0.0_f64;
    let mut max_y = 0.0_f64;
    let mut any = false;
    for primitive in &model.primitives {
        for point in extent_points(&primitive.shape) {
            if !finite(point) {
                continue;
            }
            any = true;
            max_x = max_x.max(point[0]);
            max_y = max_y.max(point[1]);
        }
    }
    if !any {
        return Err(PlotWriteError::NoPage);
    }
    const SHEETS: [[f64; 2]; 5] = [
        [297.0, 210.0],
        [420.0, 297.0],
        [594.0, 420.0],
        [841.0, 594.0],
        [1189.0, 841.0],
    ];
    for sheet in SHEETS {
        for page in [sheet, [sheet[1], sheet[0]]] {
            if max_x <= page[0] + 5.0 && max_y <= page[1] + 5.0 {
                return Ok(page);
            }
        }
    }
    Ok([max_x.ceil().max(1.0), max_y.ceil().max(1.0)])
}

fn extent_points(shape: &PaperShape) -> Vec<[f64; 2]> {
    match shape {
        PaperShape::Line { start_mm, end_mm } => vec![*start_mm, *end_mm],
        PaperShape::Polyline { points_mm, .. } => points_mm.clone(),
        PaperShape::Arc {
            center_mm,
            radius_mm,
            ..
        }
        | PaperShape::Circle {
            center_mm,
            radius_mm,
        } => vec![
            [center_mm[0] - radius_mm, center_mm[1] - radius_mm],
            [center_mm[0] + radius_mm, center_mm[1] + radius_mm],
        ],
        PaperShape::Text { origin_mm, .. } => vec![*origin_mm],
    }
}

/// The reader's sheet-frame rule, in millimetres. A line this drops is not ink.
fn reader_sheet_edge(start: [f64; 2], end: [f64; 2], page: [f64; 2]) -> bool {
    let dx = (start[0] - end[0]).abs();
    let dy = (start[1] - end[1]).abs();
    let short = 0.8 * 25.4 / 72.0;
    (dy < short && dx > page[0] * 0.90) || (dx < short && dy > page[1] * 0.90)
}

/// Straight segments `write_plot` strokes, Decoded then Inferred. ProbeOnly is omitted.
///
/// Arc and circle chords use the same 0.05 mm tolerance as the reader.
#[must_use]
pub fn stroked_segments(model: &PaperModel) -> Vec<([f64; 2], [f64; 2])> {
    let Ok(page) = resolved_page(model) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for confidence in ["decoded", "inferred"] {
        for primitive in &model.primitives {
            if primitive.confidence != confidence {
                continue;
            }
            match &primitive.shape {
                PaperShape::Line { start_mm, end_mm } => {
                    push_segment(&mut out, *start_mm, *end_mm, page);
                }
                PaperShape::Polyline { points_mm, closed } => {
                    for pair in points_mm.windows(2) {
                        push_segment(&mut out, pair[0], pair[1], page);
                    }
                    if *closed && points_mm.len() >= 2 {
                        push_segment(&mut out, points_mm[points_mm.len() - 1], points_mm[0], page);
                    }
                }
                PaperShape::Arc {
                    center_mm,
                    radius_mm,
                    start_angle,
                    end_angle,
                } => push_chain(
                    &mut out,
                    &arc_points(
                        *center_mm,
                        *radius_mm,
                        *start_angle,
                        clockwise_sweep(*start_angle, *end_angle),
                    ),
                    page,
                ),
                PaperShape::Circle {
                    center_mm,
                    radius_mm,
                } => push_chain(
                    &mut out,
                    &arc_points(*center_mm, *radius_mm, 0.0, -TAU),
                    page,
                ),
                PaperShape::Text { .. } => {}
            }
        }
    }
    out
}

fn push_segment(
    out: &mut Vec<([f64; 2], [f64; 2])>,
    start: [f64; 2],
    end: [f64; 2],
    page: [f64; 2],
) {
    if finite(start)
        && finite(end)
        && hypot(start, end) >= 1e-4
        && !reader_sheet_edge(start, end, page)
    {
        out.push((start, end));
    }
}

fn push_chain(out: &mut Vec<([f64; 2], [f64; 2])>, points: &[[f64; 2]], page: [f64; 2]) {
    for pair in points.windows(2) {
        push_segment(out, pair[0], pair[1], page);
    }
}

/// `Tf` for one run.
///
/// The five reference plots do not scale a style height by one factor.
/// Heights measured there are written at the `Tf` those plots used. A
/// height that was not measured stays the style height in points.
fn plotted_size_pt(typeface: &str, height_mm: f64) -> f64 {
    let style_pt = height_mm * PT_PER_MM;
    let hundredths = (style_pt * 100.0).round() as i64;
    measured_tf(typeface, hundredths).unwrap_or(style_pt)
}

/// Reference `Tf` for a typeface and a style height in hundredths of a point.
fn measured_tf(typeface: &str, hundredths: i64) -> Option<f64> {
    const PAIRS: &[(&str, i64, f64)] = &[
        ("Arial Narrow", 700, 10.17),
        ("Arial Narrow", 1000, 14.40),
        ("Arial", 900, 12.45),
        ("FangSong", 700, 10.56),
        ("FangSong", 720, 10.56),
        ("FangSong", 900, 13.44),
        ("SimSun", 698, 10.08),
        ("SimSun", 709, 10.56),
        ("SimSun", 720, 10.56),
        ("SimSun", 850, 12.48),
        ("SimSun", 907, 13.44),
        ("SimSun", 992, 14.40),
        ("SimSun", 1134, 16.80),
        ("SimSun", 1276, 18.72),
    ];
    PAIRS
        .iter()
        .find(|row| row.0 == typeface && row.1 == hundredths)
        .map(|row| row.2)
}

fn pt(mm: f64) -> Object {
    Object::Real(mm * PT_PER_MM)
}

fn finite(point: [f64; 2]) -> bool {
    point[0].is_finite() && point[1].is_finite()
}

fn hypot(start: [f64; 2], end: [f64; 2]) -> f64 {
    let dx = start[0] - end[0];
    let dy = start[1] - end[1];
    dx.hypot(dy)
}

#[cfg(test)]
mod tests {
    use super::{plotted_size_pt, write_plot};
    use crate::model::{PaperModel, PaperPrimitive, PaperShape, TextAnchor};
    use crate::read::read_plot;

    use super::PlotFont;

    fn primitive(confidence: &'static str, shape: PaperShape) -> PaperPrimitive {
        PaperPrimitive {
            confidence,
            stream_path: None,
            byte_range: None,
            symbol: None,
            shape,
        }
    }

    #[test]
    fn measured_style_heights_use_the_reference_tf() {
        let mm = |pt: f64| pt * 25.4 / 72.0;
        assert_eq!(plotted_size_pt("Arial Narrow", mm(7.0)), 10.17);
        assert_eq!(plotted_size_pt("Arial Narrow", mm(10.0)), 14.40);
        assert_eq!(plotted_size_pt("Arial", mm(9.0)), 12.45);
        assert_eq!(plotted_size_pt("FangSong", mm(7.0)), 10.56);
        assert_eq!(plotted_size_pt("FangSong", mm(7.2)), 10.56);
        assert_eq!(plotted_size_pt("SimSun", 2.4638), 10.08);
        assert_eq!(plotted_size_pt("SimSun", 2.5), 10.56);
        assert_eq!(plotted_size_pt("SimSun", 3.0), 12.48);
        let plain = 5.0 * 72.0 / 25.4;
        assert!((plotted_size_pt("Arial Narrow", 5.0) - plain).abs() < 1e-9);
    }

    #[test]
    fn a_line_and_a_word_read_back_on_their_layers() {
        let model = PaperModel {
            library: Vec::new(),
            page_mm: Some([200.0, 100.0]),
            primitives: vec![
                primitive(
                    "decoded",
                    PaperShape::Line {
                        start_mm: [10.0, 20.0],
                        end_mm: [80.0, 20.0],
                    },
                ),
                primitive(
                    "decoded",
                    PaperShape::Text {
                        origin_mm: [12.0, 40.0],
                        height_mm: Some(5.0),
                        rotation: 0.0,
                        value: "Ab".into(),
                        font: None,
                        anchor: TextAnchor::Left,
                    },
                ),
                primitive(
                    "inferred",
                    PaperShape::Line {
                        start_mm: [10.0, 60.0],
                        end_mm: [40.0, 70.0],
                    },
                ),
                primitive(
                    "probe_only",
                    PaperShape::Line {
                        start_mm: [1.0, 1.0],
                        end_mm: [50.0, 50.0],
                    },
                ),
            ],
            missing_symbols: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
        };
        let pdf = write_plot(
            &model,
            &[PlotFont {
                key: "",
                bytes: pdfcraft_fonts::INTER_REGULAR,
            }],
        )
        .expect("write");
        let ink = read_plot(&pdf).expect("read");
        assert!((ink.page_mm[0] - 200.0).abs() < 0.05);
        assert!((ink.page_mm[1] - 100.0).abs() < 0.05);
        assert_eq!(ink.segments.len(), 2, "{:?}", ink.segments);
        assert!(ink.segments.iter().any(|segment| {
            near(segment.start_mm, [10.0, 20.0]) && near(segment.end_mm, [80.0, 20.0])
        }));
        assert!(ink.segments.iter().any(|segment| {
            near(segment.start_mm, [10.0, 60.0]) && near(segment.end_mm, [40.0, 70.0])
        }));
        assert!(
            ink.spans.iter().any(|span| span.text == "Ab"),
            "{:?}",
            ink.spans
        );
    }

    #[test]
    fn centre_and_right_step_back_along_the_baseline() {
        let horizontal = |anchor| {
            placed(
                "WW",
                [80.0, 20.0 + f64::from(anchor_row(anchor))],
                0.0,
                anchor,
            )
        };
        let vertical = |anchor| {
            placed(
                "WW",
                [20.0 + f64::from(anchor_row(anchor)), 80.0],
                std::f64::consts::FRAC_PI_2,
                anchor,
            )
        };
        let left_h = horizontal(TextAnchor::Left);
        let centre_h = horizontal(TextAnchor::Center);
        let right_h = horizontal(TextAnchor::Right);
        let left_v = vertical(TextAnchor::Left);
        let centre_v = vertical(TextAnchor::Center);
        let along_x = left_h[0] - centre_h[0];
        let along_y = left_v[1] - centre_v[1];
        assert!(along_x > 2.0, "centre shift {along_x}");
        assert!((along_x - along_y).abs() < 0.4, "x {along_x} y {along_y}");
        assert!((left_h[1] - 20.0).abs() < 0.5, "left y {}", left_h[1]);
        assert!((centre_h[1] - 44.0).abs() < 0.5, "centre y {}", centre_h[1]);
        assert!((left_v[0] - 20.0).abs() < 0.5, "left x {}", left_v[0]);
        assert!((centre_v[0] - 44.0).abs() < 0.5, "centre x {}", centre_v[0]);
        let right_shift = left_h[0] - right_h[0];
        assert!(
            (right_shift - 2.0 * along_x).abs() < 0.5,
            "right {right_shift} centre {along_x}"
        );
    }

    fn anchor_row(anchor: TextAnchor) -> u8 {
        match anchor {
            TextAnchor::Left => 0,
            TextAnchor::Center => 24,
            TextAnchor::Right => 48,
        }
    }

    fn placed(value: &str, origin_mm: [f64; 2], rotation: f64, anchor: TextAnchor) -> [f64; 2] {
        let model = PaperModel {
            library: Vec::new(),
            page_mm: Some([200.0, 140.0]),
            primitives: vec![primitive(
                "decoded",
                PaperShape::Text {
                    origin_mm,
                    height_mm: Some(5.0),
                    rotation,
                    value: value.into(),
                    font: None,
                    anchor,
                },
            )],
            missing_symbols: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
        };
        let pdf = write_plot(
            &model,
            &[PlotFont {
                key: "",
                bytes: pdfcraft_fonts::INTER_REGULAR,
            }],
        )
        .expect("write");
        let ink = read_plot(&pdf).expect("read");
        let span = ink
            .spans
            .iter()
            .find(|span| span.text == value)
            .unwrap_or_else(|| panic!("{value} missing in {:?}", ink.spans));
        span.origin_mm
    }

    fn near(point: [f64; 2], expected: [f64; 2]) -> bool {
        (point[0] - expected[0]).abs() < 0.05 && (point[1] - expected[1]).abs() < 0.05
    }
}
