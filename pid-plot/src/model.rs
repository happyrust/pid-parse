//! Paper-space primitives for one parsed drawing.
//!
//! Coordinates are millimetres, origin at the bottom-left, y up. A symbol
//! body is placed by scaling, then rotating counter-clockwise, then
//! translating by the insertion point. A negative scale mirrors. Text keeps
//! the insertion point on the table; a centre or right paragraph moves the
//! PDF baseline back along the run when the page is written. A symbol's own
//! lettering that is only the placeholder `NULL`, including a doubled
//! `NULLNULL`, stays out of the table; text the drawing itself records is
//! kept. The sheet frame is not part of the table.

use std::path::{Path, PathBuf};

use pid_parse::style_link::{
    LineStyleIndex, ResolvedLineStyle, TextAlignment, TextStyleIndex,
};
use pid_parse::symbol_library::{
    PrimitiveStyle, SymbolLettering, SymbolLibrary, SymbolPrimitive,
};
use pid_parse::{
    PidGeometryConfidence, PidGraphicEntity, PidGraphicKind, PidParser, build_normalized_geometry,
};
use serde::{Deserialize, Serialize};

const MM_PER_METRE: f64 = 1000.0;

/// One placed drawing, ready to draw or to dump as JSON.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PaperModel {
    /// Which symbol library was searched, in order.
    pub library: Vec<String>,
    /// Page width and height in millimetres, when the drawing states them.
    pub page_mm: Option<[f64; 2]>,
    /// Primitives in drawing order. Expanded symbol strokes follow the
    /// placement they came from.
    pub primitives: Vec<PaperPrimitive>,
    /// Symbol files the drawing names and the library does not have.
    pub missing_symbols: Vec<String>,
    /// 30 mm squares around symbols that were not drawn at all.
    ///
    /// Half-size is 15 mm, the M4 mask when the drawing records no symbol box.
    pub missing_boxes: Vec<MissingSymbolBox>,
    /// Distinct symbol files the library resolved.
    pub symbols_found: usize,
    /// Distinct symbol files the drawing names.
    pub symbols_referenced: usize,
}

/// A 30 mm square around a symbol the library did not draw.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MissingSymbolBox {
    /// Library-relative path.
    pub path: String,
    /// Inclusive minimum, paper millimetres.
    pub min_mm: [f64; 2],
    /// Inclusive maximum, paper millimetres.
    pub max_mm: [f64; 2],
}

/// One paper-space primitive.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PaperPrimitive {
    /// How strongly the parser read the source record.
    pub confidence: &'static str,
    /// Stream the record came from, when the parser named it.
    pub stream_path: Option<String>,
    /// Byte range in that stream, when known.
    pub byte_range: Option<[usize; 2]>,
    /// Library path of the symbol this stroke belongs to.
    pub symbol: Option<String>,
    /// The pen the shape draws with. `None` leaves the writer's default
    /// pen — a 0.25 mm solid black stroke.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<PaperStroke>,
    /// The shape.
    pub shape: PaperShape,
}

/// The pen one primitive draws with: colour, width and dash as the
/// resolved style states them.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PaperStroke {
    /// Stroke colour.
    pub rgb: [u8; 3],
    /// Stroke width in millimetres.
    pub width_mm: f64,
    /// Dash segment magnitudes in millimetres, alternating pen-down and
    /// pen-up. Empty draws solid.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dash_mm: Vec<f64>,
}

/// A shape in paper millimetres.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PaperShape {
    /// Straight segment.
    Line {
        /// Start.
        start_mm: [f64; 2],
        /// End.
        end_mm: [f64; 2],
    },
    /// Circular arc, swept clockwise from `start_angle` to `end_angle`.
    Arc {
        /// Centre.
        center_mm: [f64; 2],
        /// Radius.
        radius_mm: f64,
        /// Start angle in radians, counter-clockwise from +X.
        start_angle: f64,
        /// End angle in radians, counter-clockwise from +X.
        end_angle: f64,
    },
    /// Full circle.
    Circle {
        /// Centre.
        center_mm: [f64; 2],
        /// Radius.
        radius_mm: f64,
    },
    /// A filled region: the ring's closed chain is painted, not stroked.
    ///
    /// A sheet `igBoundary2d` re-lists the lines its members already draw,
    /// so the region fills rather than outlining a second time; a symbol
    /// `Boundary` member is the same thing inside a body — the plug
    /// valve's concentric discs each fill their own ring.
    Fill {
        /// Ring vertices in order; the last connects back to the first.
        chain_mm: Vec<[f64; 2]>,
        /// Fill colour. `None` draws the layer's default, which these
        /// monochrome plots show as black.
        rgb: Option<[u8; 3]>,
    },
    /// Vertex chain.
    Polyline {
        /// Vertices.
        points_mm: Vec<[f64; 2]>,
        /// Whether the last vertex connects back to the first.
        closed: bool,
    },
    /// Text anchored at its insertion point.
    Text {
        /// Anchor, the insertion point.
        origin_mm: [f64; 2],
        /// Body size. Absent when the source record does not carry one.
        height_mm: Option<f64>,
        /// Counter-clockwise rotation of the baseline from +X, in radians.
        rotation: f64,
        /// Characters.
        value: String,
        /// Typeface the style table names. Symbol lettering has none: its
        /// record does not carry a font.
        #[serde(skip_serializing_if = "Option::is_none")]
        font: Option<String>,
        /// Which side of `origin_mm` the paragraph letters from.
        ///
        /// The stored point stays the insertion point. Centre and right move
        /// the PDF baseline back along the run by half the word or the whole
        /// word. Symbol lettering is [`TextAnchor::Left`]: its record has no
        /// paragraph.
        anchor: TextAnchor,
        /// Which line of `origin_mm` the paragraph letters from.
        ///
        /// The stored point stays the insertion point. Middle and top move
        /// the PDF baseline down across the run by half the body size or the
        /// whole body size, the same way the horizontal half moves it back
        /// along the run. Symbol lettering is [`TextLine::Middle`]: its record
        /// carries no body box, and the reference puts the templates' own
        /// lettering on the middle line.
        line: TextLine,
    },
}

/// Which side of its insertion point a paragraph letters from.
///
/// The values follow `TextAlignment`: left, centre, right. The symbol body
/// has no paragraph, so its lettering stays [`TextAnchor::Left`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAnchor {
    /// The insertion point is the left end of the baseline.
    Left,
    /// The insertion point is the centre of the baseline.
    Center,
    /// The insertion point is the right end of the baseline.
    Right,
}

impl TextAnchor {
    /// The horizontal hot point an `igTextBox` anchor code states:
    /// `(code >> 4) % 3` — `0` left, `1` centre, `2` right; the `+3`-shifted
    /// nibbles mark symbol-attached labels and fold into the same classes.
    fn from_anchor_code(code: u8) -> Self {
        match (code >> 4) % 3 {
            0 => Self::Left,
            1 => Self::Center,
            _ => Self::Right,
        }
    }

    /// Flip left and right when a mirrored placement absorbs a horizontal flip.
    fn flip_horizontal(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Center => Self::Center,
            Self::Right => Self::Left,
        }
    }
}

/// Which line of its insertion point a paragraph letters from.
///
/// The vertical half of the same anchor code. Where [`TextAnchor`] says which
/// point of the run the insertion sits at across the page, this says which
/// line of the body box it sits on: measured against the reference plots, the
/// labels the drawing letters sit 0.0 mm, 1.25 mm or 2.50 mm below the point
/// for a 2.5 mm body, one mode per class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextLine {
    /// The insertion point is on the baseline: the label letters at the point.
    Bottom,
    /// The insertion point is the middle of the body: the baseline runs half a
    /// body size below the point.
    Middle,
    /// The insertion point is the top of the body: the baseline runs a whole
    /// body size below the point.
    Top,
}

impl TextLine {
    /// The vertical hot point an `igTextBox` anchor code states:
    /// `(code & 0x0F) % 3` — `0` bottom, `1` top, `2` middle.
    fn from_anchor_code(code: u8) -> Self {
        match (code & 0x0F) % 3 {
            0 => Self::Bottom,
            1 => Self::Top,
            _ => Self::Middle,
        }
    }

    /// Flip top and bottom when a mirrored placement absorbs a vertical flip.
    fn flip_vertical(self) -> Self {
        match self {
            Self::Bottom => Self::Top,
            Self::Middle => Self::Middle,
            Self::Top => Self::Bottom,
        }
    }
}

/// Why a drawing could not be laid out.
#[derive(Debug)]
pub struct PaperModelError {
    message: String,
}

impl std::fmt::Display for PaperModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PaperModelError {}

/// Place `local` metres: scale, rotate counter-clockwise, then translate.
///
/// The result is paper millimetres. A negative scale component mirrors
/// across that axis before the rotation.
#[must_use]
pub fn place_metres(
    local: [f64; 2],
    insertion: [f64; 2],
    rotation: f64,
    scale: [f64; 2],
) -> [f64; 2] {
    let x = local[0] * scale[0];
    let y = local[1] * scale[1];
    let (sin, cos) = rotation.sin_cos();
    let rotated_x = x * cos - y * sin;
    let rotated_y = x * sin + y * cos;
    [
        (insertion[0] + rotated_x) * MM_PER_METRE,
        (insertion[1] + rotated_y) * MM_PER_METRE,
    ]
}

/// Lay out one `.pid`.
///
/// The symbol library is the first of: `PID_SYMBOL_LIBRARY`, a directory
/// under `<corpus>/Ref` or `<corpus>/../Ref` whose name contains `symbol`,
/// then `test-file/symbols-full`.
pub fn lay_out_drawing(pid: &Path, corpus: &Path) -> Result<PaperModel, PaperModelError> {
    let doc = PidParser::new()
        .parse_file(pid)
        .map_err(|error| PaperModelError {
            message: error.to_string(),
        })?;
    let styles = pid_parse::style_link::text_styles_for_document(&doc);
    let line_styles = pid_parse::style_link::line_styles_for_document(&doc);
    let fill_styles = pid_parse::style_link::fill_styles_for_document(&doc);
    let geometry = build_normalized_geometry(&doc);
    let (roots, library) = symbol_library(corpus);
    let mut symbols = SymbolLibrary::with_roots(roots);
    let page_mm = geometry
        .page_dimensions_mm
        .map(|(width, height)| [width, height]);
    let mut primitives = Vec::new();
    let mut referenced = std::collections::BTreeSet::new();
    let mut found = std::collections::BTreeSet::new();
    let mut missing = std::collections::BTreeSet::new();
    let mut missing_boxes = Vec::new();

    for entity in &geometry.entities {
        if entity
            .source_layer
            .as_ref()
            .and_then(|layer| layer.displayed)
            == Some(false)
        {
            continue;
        }
        match &entity.kind {
            PidGraphicKind::SymbolInstance {
                insertion,
                symbol_path,
                rotation,
                scale,
                definition,
            } => {
                let key = symbol_path.as_deref().map(symbol_key);
                if let Some(key) = key.clone() {
                    referenced.insert(key);
                }
                let body = symbol_path
                    .as_ref()
                    .and_then(|path| symbols.resolve(path).cloned());
                if symbol_path.is_some()
                    && body.is_some()
                    && let Some(key) = key.clone()
                {
                    found.insert(key);
                }
                let placement = Placement {
                    insertion: [insertion.x, insertion.y],
                    rotation: *rotation,
                    scale: *scale,
                    symbol: symbol_path.clone(),
                };
                let placement_style = entity_style(entity, &line_styles);
                if let Some(definition) =
                    definition.and_then(|reference| geometry.symbol_definition(reference))
                {
                    if symbol_path.is_some()
                        && body.is_none()
                        && let Some(key) = key.clone()
                    {
                        missing.insert(key);
                    }
                    for (primitive, body_style, lettering) in definition.visible_styled() {
                        push_symbol_primitive(
                            &mut primitives,
                            entity,
                            &placement,
                            primitive,
                            body_style,
                            lettering,
                            placement_style,
                            page_mm,
                        );
                    }
                } else if let Some(body) = body {
                    for styled in body.displayed_primitives() {
                        push_symbol_primitive(
                            &mut primitives,
                            entity,
                            &placement,
                            &styled.primitive,
                            styled.style.as_ref(),
                            styled.lettering.as_ref(),
                            placement_style,
                            page_mm,
                        );
                    }
                } else if let Some(key) = key {
                    missing_boxes.push(missing_box(&key, [insertion.x, insertion.y]));
                    missing.insert(key);
                }
            }
            _ => push_entity(
                &mut primitives,
                entity,
                page_mm,
                &styles,
                &line_styles,
                &fill_styles,
            ),
        }
    }

    Ok(PaperModel {
        library,
        page_mm,
        primitives,
        missing_symbols: missing.into_iter().collect(),
        missing_boxes,
        symbols_found: found.len(),
        symbols_referenced: referenced.len(),
    })
}

struct Placement {
    insertion: [f64; 2],
    rotation: f64,
    scale: [f64; 2],
    symbol: Option<String>,
}

fn symbol_library(corpus: &Path) -> (Vec<PathBuf>, Vec<String>) {
    if let Some(list) = std::env::var_os("PID_SYMBOL_LIBRARY") {
        let roots: Vec<PathBuf> = std::env::split_paths(&list)
            .filter(|path| path.is_dir())
            .collect();
        if !roots.is_empty() {
            let label = roots
                .iter()
                .map(|path| path.display().to_string())
                .collect();
            return (roots, label);
        }
    }
    for candidate in [
        Some(corpus.join("Ref")),
        corpus.parent().map(|parent| parent.join("Ref")),
    ]
    .into_iter()
    .flatten()
    {
        if !candidate.is_dir() {
            continue;
        }
        let mut roots = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&candidate) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                if path.is_dir() && name.contains("symbol") {
                    roots.push(path);
                }
            }
        }
        if !roots.is_empty() {
            roots.sort();
            let label = roots
                .iter()
                .map(|path| path.display().to_string())
                .collect();
            return (roots, label);
        }
    }
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../test-file/symbols-full");
    let label = vec![bundled.display().to_string()];
    (vec![bundled], label)
}

fn missing_box(path: &str, insertion_m: [f64; 2]) -> MissingSymbolBox {
    let origin = [insertion_m[0] * MM_PER_METRE, insertion_m[1] * MM_PER_METRE];
    const HALF_MM: f64 = 15.0;
    MissingSymbolBox {
        path: path.to_string(),
        min_mm: [origin[0] - HALF_MM, origin[1] - HALF_MM],
        max_mm: [origin[0] + HALF_MM, origin[1] + HALF_MM],
    }
}

/// Symbol-body lettering whose whole run is the template placeholder.
///
/// Whitespace is removed first, so ` NULL ` is the same word. A symbol that
/// stored two adjacent placeholders as `NULLNULL` is the same word twice.
/// A run that still carries other characters, such as `HH=NULL`, is lettering.
fn symbol_text_is_null_placeholder(text: &str) -> bool {
    let collapsed: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    let mut rest = collapsed.as_str();
    if rest.is_empty() {
        return false;
    }
    while let Some(next) = rest.strip_prefix("NULL") {
        rest = next;
    }
    rest.is_empty()
}

fn symbol_key(path: &str) -> String {
    pid_parse::symbol_library::library_relative_path(path)
        .unwrap_or(path)
        .replace('/', r"\")
        .to_ascii_lowercase()
}

/// The resolved line style one entity's record asks for.
fn entity_style<'a>(
    entity: &PidGraphicEntity,
    line_styles: &'a LineStyleIndex,
) -> Option<&'a ResolvedLineStyle> {
    let stream = entity.source.stream_path.as_ref()?;
    let oid = entity.graphic_oid?;
    line_styles.get(&(stream.clone(), oid))
}

/// The pen a resolved line style draws with.
fn stroke_of(style: &ResolvedLineStyle) -> PaperStroke {
    PaperStroke {
        rgb: style.symbology.rgb(),
        width_mm: style.symbology.width_mm(),
        dash_mm: style
            .dash
            .as_ref()
            .map(|dash| {
                dash.segments_mm()
                    .iter()
                    .map(|segment| segment.abs())
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// The pen a body stroke draws with: the placement's style paints the
/// whole body on `SmartPlant`'s screen, so its colour and width win over
/// the body's own; the dash falls back to the body's when the placement
/// draws solid — the same rule OpenCADStudio applies.
fn body_stroke(
    body_style: Option<&PrimitiveStyle>,
    placement_style: Option<&ResolvedLineStyle>,
) -> Option<PaperStroke> {
    if body_style.is_none() && placement_style.is_none() {
        return None;
    }
    Some(PaperStroke {
        rgb: placement_style.map_or_else(
            || body_style.map_or([0, 0, 0], |style| style.rgb),
            |style| style.symbology.rgb(),
        ),
        width_mm: placement_style.map_or_else(
            || body_style.map_or(0.25, |style| style.width_mm),
            |style| style.symbology.width_mm(),
        ),
        dash_mm: placement_style
            .and_then(|style| style.dash.as_ref())
            .map(|dash| {
                dash.segments_mm()
                    .iter()
                    .map(|segment| segment.abs())
                    .collect()
            })
            .or_else(|| {
                body_style.map(|style| {
                    style.dash_mm.iter().map(|segment| segment.abs()).collect()
                })
            })
            .unwrap_or_default(),
    })
}

fn push_entity(
    out: &mut Vec<PaperPrimitive>,
    entity: &PidGraphicEntity,
    page_mm: Option<[f64; 2]>,
    styles: &TextStyleIndex,
    line_styles: &LineStyleIndex,
    fill_styles: &pid_parse::style_link::FillIndex,
) {
    if entity
        .source_layer
        .as_ref()
        .and_then(|layer| layer.displayed)
        == Some(false)
    {
        return;
    }
    let placement = Placement {
        insertion: [0.0, 0.0],
        rotation: 0.0,
        scale: [1.0, 1.0],
        symbol: None,
    };
    let resolved = entity_style(entity, line_styles);
    let stroke = resolved.map(stroke_of);
    match &entity.kind {
        PidGraphicKind::Line { start, end } => {
            let start_mm = place_metres([start.x, start.y], [0.0, 0.0], 0.0, [1.0, 1.0]);
            let end_mm = place_metres([end.x, end.y], [0.0, 0.0], 0.0, [1.0, 1.0]);
            let shape = PaperShape::Line { start_mm, end_mm };
            if !sheet_edge(&shape, page_mm) {
                out.push(primitive(entity, &placement, stroke.clone(), shape));
                stamp_complex_dash(out, entity, &stroke, resolved, &[start_mm, end_mm]);
            }
        }
        PidGraphicKind::Polyline { points, closed } => {
            let points_mm: Vec<[f64; 2]> = points
                .iter()
                .map(|point| place_metres([point.x, point.y], [0.0, 0.0], 0.0, [1.0, 1.0]))
                .collect();
            // An `igBoundary2d` ring re-lists the member lines that are
            // already stroked on the sheet — it fills the region instead
            // of drawing the outline twice.
            if entity
                .source
                .record_id
                .as_deref()
                .is_some_and(|id| id.starts_with("igboundary2d"))
            {
                let rgb = entity
                    .source
                    .stream_path
                    .as_ref()
                    .and_then(|stream| {
                        entity
                            .graphic_oid
                            .and_then(|oid| fill_styles.get(&(stream.clone(), oid)))
                    })
                    .and_then(|fill| fill.rgb());
                out.push(primitive(
                    entity,
                    &placement,
                    None,
                    PaperShape::Fill {
                        chain_mm: points_mm,
                        rgb,
                    },
                ));
                return;
            }
            out.push(primitive(
                entity,
                &placement,
                stroke.clone(),
                PaperShape::Polyline {
                    points_mm: points_mm.clone(),
                    closed: *closed,
                },
            ));
            stamp_complex_dash(out, entity, &stroke, resolved, &points_mm);
        }
        PidGraphicKind::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            out.push(primitive(
                entity,
                &placement,
                stroke,
                arc_shape(
                    [center.x, center.y],
                    *radius,
                    *start_angle,
                    *end_angle,
                    &placement,
                ),
            ));
        }
        PidGraphicKind::Circle { center, radius } => {
            out.push(primitive(
                entity,
                &placement,
                stroke,
                PaperShape::Circle {
                    center_mm: place_metres([center.x, center.y], [0.0, 0.0], 0.0, [1.0, 1.0]),
                    radius_mm: radius.abs() * MM_PER_METRE,
                },
            ));
        }
        PidGraphicKind::Point { position } => {
            // A point draws the glyph its style's line terminator names.
            // The glyph's strokes are the style's own metres, unrotated —
            // the file states no per-point orientation.
            if let Some(marker) = resolved.and_then(|style| style.marker.as_ref())
                && marker.draws()
            {
                let at_mm = place_metres([position.x, position.y], [0.0, 0.0], 0.0, [1.0, 1.0]);
                for mark in marker.strokes() {
                    if mark.is_degenerate() {
                        continue;
                    }
                    out.push(primitive(
                        entity,
                        &placement,
                        stroke.clone(),
                        PaperShape::Line {
                            start_mm: [
                                at_mm[0] + mark.start.0 * MM_PER_METRE,
                                at_mm[1] + mark.start.1 * MM_PER_METRE,
                            ],
                            end_mm: [
                                at_mm[0] + mark.end.0 * MM_PER_METRE,
                                at_mm[1] + mark.end.1 * MM_PER_METRE,
                            ],
                        },
                    ));
                }
            }
        }
        PidGraphicKind::Text {
            insertion,
            value,
            height,
            rotation,
            anchor_code,
        } => {
            let styled = entity.source.stream_path.as_ref().and_then(|stream| {
                entity.graphic_oid.and_then(|oid| {
                    styles
                        .get(&(stream.clone(), oid))
                        .and_then(|style| style.effective())
                })
            });
            let height_mm = if height.abs() > 1e-9 {
                Some(height.abs() * MM_PER_METRE)
            } else if let Some(millimetres) = styled
                .as_ref()
                .map(pid_parse::style_link::ResolvedTextHeight::height_mm)
                .filter(|millimetres| *millimetres > 0.0)
            {
                Some(millimetres)
            } else {
                Some(0.0)
            };
            out.push(primitive(
                entity,
                &placement,
                None,
                PaperShape::Text {
                    origin_mm: place_metres(
                        [insertion.x, insertion.y],
                        [0.0, 0.0],
                        0.0,
                        [1.0, 1.0],
                    ),
                    height_mm,
                    rotation: *rotation,
                    value: value.clone(),
                    font: styled.as_ref().and_then(|style| style.font_name.clone()),
                    // The record's own anchor code overrides the paragraph
                    // style's declared alignment: `(code >> 4) % 3` is the
                    // horizontal hot point the insertion sits at.
                    anchor: TextAnchor::from_anchor_code(*anchor_code),
                    line: TextLine::from_anchor_code(*anchor_code),
                },
            ));
        }
        PidGraphicKind::SymbolInstance { .. }
        | PidGraphicKind::Annotation { .. }
        | PidGraphicKind::Unknown { .. } => {}
    }
}

/// Stamp one complex-dash glyph along `path` at the advance the style
/// names — the W drawing's hydraulic pipes, which print a `└`/`⊢` mark
/// once per fitted cell, centred in the cell, in the line's local frame
/// (tangent, left normal).
fn stamp_complex_dash(
    out: &mut Vec<PaperPrimitive>,
    entity: &PidGraphicEntity,
    stroke: &Option<PaperStroke>,
    resolved: Option<&ResolvedLineStyle>,
    path_mm: &[[f64; 2]],
) {
    let Some(complex) = resolved.and_then(|style| style.complex_dash.as_ref()) else {
        return;
    };
    let period = complex.period_mm();
    if period <= 1e-6 || path_mm.len() < 2 {
        return;
    }
    let total: f64 = path_mm
        .windows(2)
        .map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]))
        .sum();
    if total < period {
        return;
    }
    let cells = (total / period).floor() as usize;
    let placement = Placement {
        insertion: [0.0, 0.0],
        rotation: 0.0,
        scale: [1.0, 1.0],
        symbol: None,
    };
    for cell in 1..=cells {
        // The stamp sits at the centre of its fitted cell.
        let at = ((cell as f64) - 0.5) * (total / cells as f64);
        let mut walked = 0.0;
        let mut frame: Option<([f64; 2], [f64; 2])> = None;
        for pair in path_mm.windows(2) {
            let length = (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]);
            if length < 1e-9 {
                continue;
            }
            if walked + length >= at {
                let t = (at - walked) / length;
                let point = [
                    pair[0][0] + (pair[1][0] - pair[0][0]) * t,
                    pair[0][1] + (pair[1][1] - pair[0][1]) * t,
                ];
                let tangent = [
                    (pair[1][0] - pair[0][0]) / length,
                    (pair[1][1] - pair[0][1]) / length,
                ];
                frame = Some((point, tangent));
                break;
            }
            walked += length;
        }
        let Some((point, tangent)) = frame else { continue };
        let left = [-tangent[1], tangent[0]];
        for mark in complex.strokes() {
            if mark.is_degenerate() {
                continue;
            }
            let place = |local: (f64, f64)| {
                let (lx, ly) = (local.0 * MM_PER_METRE, local.1 * MM_PER_METRE);
                [
                    point[0] + lx * tangent[0] + ly * left[0],
                    point[1] + lx * tangent[1] + ly * left[1],
                ]
            };
            out.push(primitive(
                entity,
                &placement,
                stroke.clone(),
                PaperShape::Line {
                    start_mm: place(mark.start),
                    end_mm: place(mark.end),
                },
            ));
        }
    }
}

fn push_symbol_primitive(
    out: &mut Vec<PaperPrimitive>,
    entity: &PidGraphicEntity,
    placement: &Placement,
    primitive_shape: &SymbolPrimitive,
    body_style: Option<&PrimitiveStyle>,
    lettering: Option<&SymbolLettering>,
    placement_style: Option<&ResolvedLineStyle>,
    page_mm: Option<[f64; 2]>,
) {
    let shape = match primitive_shape {
        SymbolPrimitive::Line { start, end } => PaperShape::Line {
            start_mm: place_metres(
                [start.0, start.1],
                placement.insertion,
                placement.rotation,
                placement.scale,
            ),
            end_mm: place_metres(
                [end.0, end.1],
                placement.insertion,
                placement.rotation,
                placement.scale,
            ),
        },
        SymbolPrimitive::Circle { center, radius } => PaperShape::Circle {
            center_mm: place_metres(
                [center.0, center.1],
                placement.insertion,
                placement.rotation,
                placement.scale,
            ),
            radius_mm: radius.abs() * placement.scale[0].abs() * MM_PER_METRE,
        },
        SymbolPrimitive::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => arc_shape(
            [center.0, center.1],
            *radius,
            *start_angle,
            *end_angle,
            placement,
        ),
        SymbolPrimitive::Polyline {
            vertices,
            is_closed,
        } => PaperShape::Polyline {
            points_mm: vertices
                .iter()
                .map(|point| {
                    place_metres(
                        [point.0, point.1],
                        placement.insertion,
                        placement.rotation,
                        placement.scale,
                    )
                })
                .collect(),
            closed: *is_closed,
        },
        SymbolPrimitive::Text { text, at } => {
            if symbol_text_is_null_placeholder(text) {
                return;
            }
            symbol_text_shape(text, *at, placement, lettering)
        }
        SymbolPrimitive::BSpline { .. } => {
            let points = primitive_shape.bspline_points(8);
            PaperShape::Polyline {
                points_mm: points
                    .into_iter()
                    .map(|point| {
                        place_metres(
                            [point.0, point.1],
                            placement.insertion,
                            placement.rotation,
                            placement.scale,
                        )
                    })
                    .collect(),
                closed: false,
            }
        }
        // A boundary is a filled region over members that are also stroked
        // on their own — the flange dots and plug-valve discs `SmartPlant`
        // prints solid. Each circle fills its own ring (concentric members
        // stay solid, never punched through); line members that chain into
        // a ring fill the polygon they close, and a chain that does not
        // close falls back to stroking its members like before.
        SymbolPrimitive::Boundary { members, fill } => {
            let rgb = fill.as_ref().and_then(|fill| fill.rgb);
            let mut ring: Vec<[f64; 2]> = Vec::new();
            for member in members {
                match member {
                    SymbolPrimitive::Circle { center, radius } => {
                        let center_mm = place_metres(
                            [center.0, center.1],
                            placement.insertion,
                            placement.rotation,
                            placement.scale,
                        );
                        let radius_mm =
                            radius.abs() * placement.scale[0].abs() * MM_PER_METRE;
                        if radius_mm > 1e-4 {
                            out.push(primitive(
                                entity,
                                placement,
                                None,
                                PaperShape::Fill {
                                    chain_mm: circle_chain(center_mm, radius_mm),
                                    rgb,
                                },
                            ));
                        }
                    }
                    SymbolPrimitive::Line { start, end } => {
                        let start_mm = place_metres(
                            [start.0, start.1],
                            placement.insertion,
                            placement.rotation,
                            placement.scale,
                        );
                        let end_mm = place_metres(
                            [end.0, end.1],
                            placement.insertion,
                            placement.rotation,
                            placement.scale,
                        );
                        if ring.last().is_none_or(|last| {
                            (last[0] - start_mm[0]).hypot(last[1] - start_mm[1]) > 1e-3
                        }) {
                            ring.push(start_mm);
                        }
                        ring.push(end_mm);
                    }
                    _ => {}
                }
            }
            let closed = ring.len() >= 3
                && ring
                    .first()
                    .zip(ring.last())
                    .is_some_and(|(first, last)| {
                        (first[0] - last[0]).hypot(first[1] - last[1]) <= 1e-3
                    });
            if closed {
                out.push(primitive(
                    entity,
                    placement,
                    None,
                    PaperShape::Fill {
                        chain_mm: ring,
                        rgb,
                    },
                ));
            } else {
                for pair in ring.windows(2) {
                    out.push(primitive(
                        entity,
                        placement,
                        body_stroke(body_style, placement_style),
                        PaperShape::Line {
                            start_mm: pair[0],
                            end_mm: pair[1],
                        },
                    ));
                }
            }
            return;
        }
    };
    if !sheet_edge(&shape, page_mm) {
        out.push(primitive(
            entity,
            placement,
            body_stroke(body_style, placement_style),
            shape,
        ));
    }
}

/// A circle as a closed vertex ring, fine enough that the largest chord
/// error at symbol-body sizes stays under the reader's 0.05 mm flattening.
fn circle_chain(center_mm: [f64; 2], radius_mm: f64) -> Vec<[f64; 2]> {
    const SEGMENTS: usize = 48;
    (0..SEGMENTS)
        .map(|step| {
            let angle = std::f64::consts::TAU * step as f64 / SEGMENTS as f64;
            [
                center_mm[0] + radius_mm * angle.cos(),
                center_mm[1] + radius_mm * angle.sin(),
            ]
        })
        .collect()
}

fn symbol_text_shape(
    text: &str,
    at: (f64, f64),
    placement: &Placement,
    lettering: Option<&SymbolLettering>,
) -> PaperShape {
    let scale = placement.scale[0].abs();
    let height_mm = lettering.and_then(|lettering| {
        if lettering.letter_height_m.abs() > 1e-9 {
            Some(lettering.letter_height_m.abs() * scale * MM_PER_METRE)
        } else {
            lettering
                .height_m
                .filter(|height| *height > 0.0)
                .map(|height| height * scale * MM_PER_METRE)
        }
    });
    let mut anchor = lettering.map_or(TextAnchor::Left, |lettering| {
        if lettering.anchor_code != 0 {
            TextAnchor::from_anchor_code(lettering.anchor_code)
        } else {
            match lettering.alignment {
                Some(TextAlignment::Center) => TextAnchor::Center,
                Some(TextAlignment::Right) => TextAnchor::Right,
                Some(TextAlignment::Left) | None => TextAnchor::Left,
            }
        }
    });
    let mut line = lettering.map_or(TextLine::Middle, |lettering| {
        if lettering.anchor_code != 0 {
            TextLine::from_anchor_code(lettering.anchor_code)
        } else {
            TextLine::Middle
        }
    });
    let local_rotation = lettering.map_or(0.0, |lettering| lettering.rotation_rad);
    let mirrored = placement.scale[0] * placement.scale[1] < 0.0;
    let rotation = if mirrored {
        let raw = placement.rotation - local_rotation;
        let emit = (raw + std::f64::consts::FRAC_PI_2).rem_euclid(std::f64::consts::PI)
            - std::f64::consts::FRAC_PI_2;
        let delta = emit - placement.rotation;
        if delta.cos() < -0.5 {
            anchor = anchor.flip_horizontal();
        }
        if delta.cos() > 0.5 {
            line = line.flip_vertical();
        }
        emit
    } else {
        placement.rotation + local_rotation
    };
    PaperShape::Text {
        origin_mm: place_metres(
            [at.0, at.1],
            placement.insertion,
            placement.rotation,
            placement.scale,
        ),
        height_mm,
        rotation,
        value: text.to_string(),
        font: lettering.and_then(|lettering| lettering.font_name.clone()),
        anchor,
        line,
    }
}

fn arc_shape(
    center: [f64; 2],
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    placement: &Placement,
) -> PaperShape {
    let mut start = start_angle;
    let mut end = end_angle;
    let ccw = (start - end).rem_euclid(std::f64::consts::TAU);
    if ccw > std::f64::consts::PI + 1e-9 {
        std::mem::swap(&mut start, &mut end);
    }
    let mirrored = placement.scale[0] * placement.scale[1] < 0.0;
    let mut world_start = world_angle(start, placement.rotation, placement.scale);
    let mut world_end = world_angle(end, placement.rotation, placement.scale);
    if mirrored {
        std::mem::swap(&mut world_start, &mut world_end);
    }
    PaperShape::Arc {
        center_mm: place_metres(
            center,
            placement.insertion,
            placement.rotation,
            placement.scale,
        ),
        radius_mm: radius.abs() * placement.scale[0].abs() * MM_PER_METRE,
        start_angle: world_start,
        end_angle: world_end,
    }
}

fn world_angle(angle: f64, rotation: f64, scale: [f64; 2]) -> f64 {
    let x = angle.cos() * scale[0];
    let y = angle.sin() * scale[1];
    let (sin, cos) = rotation.sin_cos();
    (x * sin + y * cos).atan2(x * cos - y * sin)
}

fn sheet_edge(shape: &PaperShape, page_mm: Option<[f64; 2]>) -> bool {
    let Some([page_w, page_h]) = page_mm else {
        return false;
    };
    let PaperShape::Line { start_mm, end_mm } = shape else {
        return false;
    };
    let dx = (start_mm[0] - end_mm[0]).abs();
    let dy = (start_mm[1] - end_mm[1]).abs();
    (dy < 1.0 && dx > page_w * 0.90) || (dx < 1.0 && dy > page_h * 0.90)
}

fn primitive(
    entity: &PidGraphicEntity,
    placement: &Placement,
    stroke: Option<PaperStroke>,
    shape: PaperShape,
) -> PaperPrimitive {
    PaperPrimitive {
        confidence: confidence_name(entity.confidence),
        stream_path: entity.source.stream_path.clone(),
        byte_range: entity
            .source
            .byte_range
            .as_ref()
            .map(|range| [range.start, range.end]),
        symbol: placement.symbol.clone(),
        stroke,
        shape,
    }
}

fn confidence_name(confidence: PidGeometryConfidence) -> &'static str {
    match confidence {
        PidGeometryConfidence::Decoded => "decoded",
        PidGeometryConfidence::Inferred => "inferred",
        PidGeometryConfidence::ProbeOnly => "probe_only",
    }
}

#[cfg(test)]
mod tests {
    use pid_parse::style_link::TextAlignment;
    use pid_parse::symbol_library::{SymbolLettering, SymbolPrimitive};
    use pid_parse::{
        PidCoordinateContext, PidGeometryConfidence, PidGraphicEntity, PidGraphicKind,
        PidGraphicProvenance, PidPoint, PidSourceLayer,
    };

    use super::{
        PaperShape, Placement, TextAnchor, TextLine, arc_shape, place_metres, push_entity,
        push_symbol_primitive,
    };

    fn entity(kind: PidGraphicKind) -> PidGraphicEntity {
        PidGraphicEntity {
            id: "t".to_string(),
            drawing_id: None,
            graphic_oid: None,
            source_layer: None,
            kind,
            coordinate_context: PidCoordinateContext::default(),
            source: PidGraphicProvenance::default(),
            confidence: PidGeometryConfidence::Decoded,
        }
    }

    fn placement() -> Placement {
        Placement {
            insertion: [0.0, 0.0],
            rotation: 0.0,
            scale: [1.0, 1.0],
            symbol: Some("valve.sym".to_string()),
        }
    }

    #[test]
    fn placement_adds_the_insertion_in_millimetres() {
        let paper = place_metres([0.01, 0.02], [1.0, 2.0], 0.0, [1.0, 1.0]);
        assert!((paper[0] - 1010.0).abs() < 1e-6);
        assert!((paper[1] - 2020.0).abs() < 1e-6);
    }

    #[test]
    fn rotation_is_counter_clockwise() {
        let paper = place_metres(
            [0.01, 0.0],
            [0.0, 0.0],
            std::f64::consts::FRAC_PI_2,
            [1.0, 1.0],
        );
        assert!(paper[0].abs() < 1e-6);
        assert!((paper[1] - 10.0).abs() < 1e-6);
    }

    #[test]
    fn a_negative_scale_mirrors_before_the_rotation() {
        let paper = place_metres([0.0, 0.01], [0.0, 0.0], 0.0, [1.0, -1.0]);
        assert!(paper[0].abs() < 1e-6);
        assert!((paper[1] + 10.0).abs() < 1e-6);
    }

    #[test]
    fn text_anchor_is_the_insertion_point() {
        let paper = place_metres([0.0, 0.0], [0.25, 0.5], 0.0, [1.0, 1.0]);
        assert!((paper[0] - 250.0).abs() < 1e-6);
        assert!((paper[1] - 500.0).abs() < 1e-6);
    }

    #[test]
    fn a_symbol_placeholder_null_is_left_out_of_the_table() {
        let mut out = Vec::new();
        push_symbol_primitive(
            &mut out,
            &entity(PidGraphicKind::Point {
                position: PidPoint { x: 0.0, y: 0.0 },
            }),
            &placement(),
            &SymbolPrimitive::Text {
                text: " NULL ".to_string(),
                at: (0.0, 0.0),
            },
            None,
            None,
            None,
            None,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn a_doubled_placeholder_null_is_left_out_of_the_table() {
        let mut out = Vec::new();
        push_symbol_primitive(
            &mut out,
            &entity(PidGraphicKind::Point {
                position: PidPoint { x: 0.0, y: 0.0 },
            }),
            &placement(),
            &SymbolPrimitive::Text {
                text: "NULLNULL".to_string(),
                at: (0.0, 0.0),
            },
            None,
            None,
            None,
            None,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn a_symbol_label_that_mentions_null_stays_in_the_table() {
        let mut out = Vec::new();
        push_symbol_primitive(
            &mut out,
            &entity(PidGraphicKind::Point {
                position: PidPoint { x: 0.0, y: 0.0 },
            }),
            &placement(),
            &SymbolPrimitive::Text {
                text: "HH=NULL".to_string(),
                at: (0.01, 0.0),
            },
            None,
            None,
            None,
            None,
        );
        assert_eq!(out.len(), 1);
        let PaperShape::Text { value, .. } = &out[0].shape else {
            panic!("{:?}", out[0].shape);
        };
        assert_eq!(value, "HH=NULL");
    }

    #[test]
    fn drawing_text_that_says_null_is_still_drawn() {
        let mut out = Vec::new();
        push_entity(
            &mut out,
            &entity(PidGraphicKind::Text {
                insertion: PidPoint { x: 0.1, y: 0.2 },
                value: "NULL".to_string(),
                height: 0.0025,
                rotation: 0.0,
                anchor_code: 0,
            }),
            None,
            &pid_parse::style_link::TextStyleIndex::new(),
            &pid_parse::style_link::LineStyleIndex::new(),
            &pid_parse::style_link::FillIndex::new(),
        );
        assert_eq!(out.len(), 1);
        let PaperShape::Text { value, .. } = &out[0].shape else {
            panic!("{:?}", out[0].shape);
        };
        assert_eq!(value, "NULL");
    }

    #[test]
    fn hidden_drawing_layer_entities_are_left_out_of_the_table() {
        let mut out = Vec::new();
        let mut hidden = entity(PidGraphicKind::Text {
            insertion: PidPoint { x: 0.1, y: 0.2 },
            value: "PGGAS100 << P40".to_string(),
            height: 0.0025,
            rotation: 0.0,
            anchor_code: 0,
        });
        hidden.source_layer = Some(PidSourceLayer {
            oid: 100,
            name: Some("Label".to_string()),
            storage_path: "/".to_string(),
            displayed: Some(false),
        });
        push_entity(
            &mut out,
            &hidden,
            None,
            &pid_parse::style_link::TextStyleIndex::new(),
            &pid_parse::style_link::LineStyleIndex::new(),
            &pid_parse::style_link::FillIndex::new(),
        );
        assert!(out.is_empty());
    }

    #[test]
    fn symbol_lettering_supplies_height_font_rotation_and_anchor() {
        let mut out = Vec::new();
        let lettering = SymbolLettering {
            anchor_code: 0x10,
            rotation_rad: 0.25,
            letter_height_m: 0.0,
            height_m: Some(0.003),
            rgb: None,
            alignment: Some(TextAlignment::Right),
            line_spacing: None,
            font_name: Some("Arial Narrow".to_string()),
        };
        push_symbol_primitive(
            &mut out,
            &entity(PidGraphicKind::Point {
                position: PidPoint { x: 0.0, y: 0.0 },
            }),
            &placement(),
            &SymbolPrimitive::Text {
                text: "ESD".to_string(),
                at: (0.01, 0.02),
            },
            None,
            Some(&lettering),
            None,
            None,
        );
        assert_eq!(out.len(), 1);
        let PaperShape::Text {
            height_mm,
            rotation,
            font,
            anchor,
            line,
            ..
        } = &out[0].shape
        else {
            panic!("{:?}", out[0].shape);
        };
        assert_eq!(*height_mm, Some(3.0));
        assert!((*rotation - 0.25).abs() < 1e-9);
        assert_eq!(font.as_deref(), Some("Arial Narrow"));
        assert_eq!(*anchor, TextAnchor::Center);
        assert_eq!(*line, TextLine::Bottom);
    }

    #[test]
    fn mirrored_placement_lettering_stays_readable() {
        let lettering = SymbolLettering {
            anchor_code: 0x00,
            rotation_rad: 0.0,
            letter_height_m: 0.0025,
            height_m: None,
            rgb: None,
            alignment: Some(TextAlignment::Left),
            line_spacing: None,
            font_name: None,
        };
        let mut out = Vec::new();
        let mut mirrored_pi = placement();
        mirrored_pi.rotation = std::f64::consts::PI;
        mirrored_pi.scale = [1.0, -1.0];
        push_symbol_primitive(
            &mut out,
            &entity(PidGraphicKind::Point {
                position: PidPoint { x: 0.0, y: 0.0 },
            }),
            &mirrored_pi,
            &SymbolPrimitive::Text {
                text: "ESD".to_string(),
                at: (0.0, 0.0),
            },
            None,
            Some(&lettering),
            None,
            None,
        );
        let PaperShape::Text {
            rotation, anchor, ..
        } = &out[0].shape
        else {
            panic!("{:?}", out[0].shape);
        };
        assert!(rotation.abs() < 1e-9, "rotation {rotation}");
        assert_eq!(*anchor, TextAnchor::Right);

        let mut out_vert = Vec::new();
        let mut mirrored_neg_half = placement();
        mirrored_neg_half.rotation = -std::f64::consts::FRAC_PI_2;
        mirrored_neg_half.scale = [1.0, -1.0];
        let mut bottom_lettering = lettering.clone();
        bottom_lettering.anchor_code = 0x10;
        push_symbol_primitive(
            &mut out_vert,
            &entity(PidGraphicKind::Point {
                position: PidPoint { x: 0.0, y: 0.0 },
            }),
            &mirrored_neg_half,
            &SymbolPrimitive::Text {
                text: "D".to_string(),
                at: (0.0, 0.0),
            },
            None,
            Some(&bottom_lettering),
            None,
            None,
        );
        let PaperShape::Text { rotation, line, .. } = &out_vert[0].shape else {
            panic!("{:?}", out_vert[0].shape);
        };
        assert!((*rotation + std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        assert_eq!(*line, TextLine::Top);
    }

    #[test]
    fn arc_shape_normalizes_to_the_minor_clockwise_sweep_with_pi_ulp_tolerance() {
        let PaperShape::Arc {
            start_angle,
            end_angle,
            ..
        } = arc_shape([0.0, 0.0], 0.01, 1.1659, 1.4273, &placement())
        else {
            panic!("expected arc");
        };
        assert!((start_angle - 1.4273).abs() < 1e-6);
        assert!((end_angle - 1.1659).abs() < 1e-6);

        let pi_plus_ulp = 3.141_592_653_589_794;
        let PaperShape::Arc {
            start_angle: s_pi,
            end_angle: e_pi,
            ..
        } = arc_shape([0.0, 0.0], 0.01, pi_plus_ulp, 0.0, &placement())
        else {
            panic!("expected arc");
        };
        assert!((s_pi.abs() - std::f64::consts::PI).abs() < 1e-6);
        assert!(e_pi.abs() < 1e-6);
    }
}
