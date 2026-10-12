//! Paper-space primitives for one parsed drawing.
//!
//! Coordinates are millimetres, origin at the bottom-left, y up. A symbol
//! body is placed by scaling, then rotating counter-clockwise, then
//! translating by the insertion point. A negative scale mirrors. Text keeps
//! the insertion point as its anchor. The sheet frame is not part of the
//! table.

use std::path::{Path, PathBuf};

use pid_parse::symbol_library::{SymbolLibrary, SymbolPrimitive};
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
    /// The shape.
    pub shape: PaperShape,
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
    },
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
/// under `<corpus>/Ref` whose name contains `symbol`, then
/// `test-file/symbols-full`.
pub fn lay_out_drawing(pid: &Path, corpus: &Path) -> Result<PaperModel, PaperModelError> {
    let doc = PidParser::new()
        .parse_file(pid)
        .map_err(|error| PaperModelError {
            message: error.to_string(),
        })?;
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
                if let Some(body) = body {
                    for styled in &body.primitives {
                        push_symbol_primitive(
                            &mut primitives,
                            entity,
                            &placement,
                            &styled.primitive,
                            page_mm,
                        );
                    }
                } else if let Some(definition) =
                    definition.and_then(|reference| geometry.symbol_definition(reference))
                {
                    if symbol_path.is_some()
                        && let Some(key) = key.clone()
                    {
                        missing.insert(key);
                    }
                    for primitive in definition.visible_primitives() {
                        push_symbol_primitive(
                            &mut primitives,
                            entity,
                            &placement,
                            primitive,
                            page_mm,
                        );
                    }
                } else if let Some(key) = key {
                    missing_boxes.push(missing_box(&key, [insertion.x, insertion.y]));
                    missing.insert(key);
                }
            }
            _ => push_entity(&mut primitives, entity, page_mm),
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
    let reference = corpus.join("Ref");
    if reference.is_dir() {
        let mut roots = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&reference) {
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

fn symbol_key(path: &str) -> String {
    pid_parse::symbol_library::library_relative_path(path)
        .unwrap_or(path)
        .replace('/', r"\")
        .to_ascii_lowercase()
}

fn push_entity(
    out: &mut Vec<PaperPrimitive>,
    entity: &PidGraphicEntity,
    page_mm: Option<[f64; 2]>,
) {
    let placement = Placement {
        insertion: [0.0, 0.0],
        rotation: 0.0,
        scale: [1.0, 1.0],
        symbol: None,
    };
    match &entity.kind {
        PidGraphicKind::Line { start, end } => {
            let shape = PaperShape::Line {
                start_mm: place_metres([start.x, start.y], [0.0, 0.0], 0.0, [1.0, 1.0]),
                end_mm: place_metres([end.x, end.y], [0.0, 0.0], 0.0, [1.0, 1.0]),
            };
            if !sheet_edge(&shape, page_mm) {
                out.push(primitive(entity, &placement, shape));
            }
        }
        PidGraphicKind::Polyline { points, closed } => {
            out.push(primitive(
                entity,
                &placement,
                PaperShape::Polyline {
                    points_mm: points
                        .iter()
                        .map(|point| place_metres([point.x, point.y], [0.0, 0.0], 0.0, [1.0, 1.0]))
                        .collect(),
                    closed: *closed,
                },
            ));
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
                PaperShape::Circle {
                    center_mm: place_metres([center.x, center.y], [0.0, 0.0], 0.0, [1.0, 1.0]),
                    radius_mm: radius.abs() * MM_PER_METRE,
                },
            ));
        }
        PidGraphicKind::Text {
            insertion,
            value,
            height,
            rotation,
        } => {
            out.push(primitive(
                entity,
                &placement,
                PaperShape::Text {
                    origin_mm: place_metres(
                        [insertion.x, insertion.y],
                        [0.0, 0.0],
                        0.0,
                        [1.0, 1.0],
                    ),
                    height_mm: Some(height.abs() * MM_PER_METRE),
                    rotation: *rotation,
                    value: value.clone(),
                },
            ));
        }
        PidGraphicKind::SymbolInstance { .. }
        | PidGraphicKind::Point { .. }
        | PidGraphicKind::Annotation { .. }
        | PidGraphicKind::Unknown { .. } => {}
    }
}

fn push_symbol_primitive(
    out: &mut Vec<PaperPrimitive>,
    entity: &PidGraphicEntity,
    placement: &Placement,
    primitive_shape: &SymbolPrimitive,
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
        SymbolPrimitive::Text { text, at } => PaperShape::Text {
            origin_mm: place_metres(
                [at.0, at.1],
                placement.insertion,
                placement.rotation,
                placement.scale,
            ),
            height_mm: None,
            rotation: placement.rotation,
            value: text.clone(),
        },
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
    };
    if !sheet_edge(&shape, page_mm) {
        out.push(primitive(entity, placement, shape));
    }
}

fn arc_shape(
    center: [f64; 2],
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    placement: &Placement,
) -> PaperShape {
    let mirrored = placement.scale[0] * placement.scale[1] < 0.0;
    let mut start = world_angle(start_angle, placement.rotation, placement.scale);
    let mut end = world_angle(end_angle, placement.rotation, placement.scale);
    if mirrored {
        std::mem::swap(&mut start, &mut end);
    }
    PaperShape::Arc {
        center_mm: place_metres(
            center,
            placement.insertion,
            placement.rotation,
            placement.scale,
        ),
        radius_mm: radius.abs() * placement.scale[0].abs() * MM_PER_METRE,
        start_angle: start,
        end_angle: end,
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
    use super::place_metres;

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
}
