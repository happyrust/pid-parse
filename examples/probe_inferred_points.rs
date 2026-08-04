//! What the inferred `Point` evidence actually contains.
//!
//! `OpenCADStudio`'s importer draws inferred annotations and inferred endpoint
//! pairs but drops every inferred `Point`, which is the largest inferred
//! category on every fixture. This measures whether that is a loss: each point
//! is classified by which hint produced it and tested against the page the
//! drawing states, so "could this be drawn" is answered from the corpus rather
//! than from the emitter's own caveats.
//!
//! A decoded coordinate is metres on a sheet under a metre across, so a hint
//! that is going to share that space has to land in roughly `0..1`.

use std::collections::BTreeMap;
use std::path::Path;

use pid_parse::{
    build_normalized_geometry, PidGeometryConfidence, PidGraphicKind, PidParser, PidPoint,
};

/// Which hint an inferred point came from. The three feed off different
/// evidence and deserve separate verdicts.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum Source {
    /// `coordinate_hints`: adjacent non-zero `i32` pairs on 4-byte alignment,
    /// capped at 64 per stream. A sliding window over raw bytes, not a record.
    CoordinateHint,
    /// `object_geometry_hints` with an `i32` position that passed the window
    /// score gate.
    GeometryHintI32,
    /// `object_geometry_hints` falling back to an `f64` pair.
    GeometryHintF64,
}

impl Source {
    fn label(self) -> &'static str {
        match self {
            Source::CoordinateHint => "coordinate_hints (i32 byte scan)",
            Source::GeometryHintI32 => "object_geometry_hints (i32)",
            Source::GeometryHintF64 => "object_geometry_hints (f64)",
        }
    }
}

/// Nearer than this and two places on the sheet are one place. A tenth of a
/// millimetre, the same bound the importer's connectivity filter uses.
const COINCIDENT_MM: f64 = 0.1;

#[derive(Default)]
struct Tally {
    total: usize,
    /// Both components inside `0..=1`, the band a decoded metre coordinate
    /// occupies on any of these sheets.
    in_metre_band: usize,
    /// Degenerate: the origin, which is what an unresolved coordinate decodes
    /// to, rather than a place on the sheet.
    at_origin: usize,
    /// On the sheet, not the origin, and not already occupied by a decoded
    /// entity -- the only ones that would add anything to the drawing.
    novel: usize,
    max_abs: f64,
    samples: Vec<(f64, f64)>,
}

impl Tally {
    fn add(&mut self, point: &PidPoint, decoded: &[(f64, f64)]) {
        self.total += 1;
        let (x, y) = (point.x, point.y);
        let on_sheet = x.abs() <= 1.0 && y.abs() <= 1.0;
        if on_sheet {
            self.in_metre_band += 1;
        }
        // Metres to millimetres, so the coincidence test is in the unit the
        // sheet is measured in.
        let at_origin = (x.hypot(y) * 1000.0) < COINCIDENT_MM;
        if at_origin {
            self.at_origin += 1;
        }
        if on_sheet && !at_origin {
            let covered = decoded
                .iter()
                .any(|(dx, dy)| ((x - dx).hypot(y - dy) * 1000.0) < COINCIDENT_MM);
            if !covered {
                self.novel += 1;
                if self.samples.len() < 6 {
                    self.samples.push((x, y));
                }
            }
        }
        self.max_abs = self.max_abs.max(x.abs()).max(y.abs());
    }
}

fn classify(id: &str, point: &PidPoint) -> Source {
    if id.contains(":coordinate-hint:") {
        return Source::CoordinateHint;
    }
    // An i32 hint is widened with `f64::from`, so it is always integral; the
    // f64 fallback carries the record's own double.
    if point.x.fract() == 0.0 && point.y.fract() == 0.0 {
        Source::GeometryHintI32
    } else {
        Source::GeometryHintF64
    }
}

fn probe(path: &Path) {
    let parsed = match PidParser::new().parse_file(path) {
        Ok(parsed) => parsed,
        Err(error) => {
            println!("{}: FAILED {error}", path.display());
            return;
        }
    };
    let geometry = build_normalized_geometry(&parsed);

    // Where the drawing already puts something. A hint landing on one of
    // these adds nothing even if its coordinate is sound.
    let mut occupied: Vec<(f64, f64)> = Vec::new();
    for entity in &geometry.entities {
        if entity.confidence != PidGeometryConfidence::Decoded {
            continue;
        }
        match &entity.kind {
            PidGraphicKind::Point { position } => occupied.push((position.x, position.y)),
            PidGraphicKind::Text { insertion, .. }
            | PidGraphicKind::SymbolInstance { insertion, .. } => {
                occupied.push((insertion.x, insertion.y))
            }
            PidGraphicKind::Line { start, end } => {
                occupied.push((start.x, start.y));
                occupied.push((end.x, end.y));
            }
            PidGraphicKind::Polyline { points, .. } => {
                occupied.extend(points.iter().map(|p| (p.x, p.y)))
            }
            PidGraphicKind::Circle { center, .. } | PidGraphicKind::Arc { center, .. } => {
                occupied.push((center.x, center.y))
            }
            PidGraphicKind::Annotation { .. } | PidGraphicKind::Unknown { .. } => {}
        }
    }

    let mut by_source: BTreeMap<Source, Tally> = BTreeMap::new();
    let mut decoded_points = 0usize;
    for entity in &geometry.entities {
        let PidGraphicKind::Point { position } = &entity.kind else {
            continue;
        };
        match entity.confidence {
            PidGeometryConfidence::Decoded => decoded_points += 1,
            PidGeometryConfidence::Inferred => by_source
                .entry(classify(&entity.id, position))
                .or_default()
                .add(position, &occupied),
            PidGeometryConfidence::ProbeOnly => {}
        }
    }

    let page = geometry
        .page_dimensions_mm
        .map(|(w, h)| format!("{w:.1} x {h:.1} mm"))
        .unwrap_or_else(|| "unknown".into());
    println!("\n=== {} (page {page}) ===", path.display());
    println!(
        "  decoded igPoint2d: {decoded_points}   decoded anchors on sheet: {}",
        occupied.len()
    );
    for (source, tally) in &by_source {
        println!(
            "  {:<34} n={:<4} on-sheet={:<4} at-origin={:<4} NOVEL={:<4} max|v|={:.3e}",
            source.label(),
            tally.total,
            tally.in_metre_band,
            tally.at_origin,
            tally.novel,
            tally.max_abs
        );
        if !tally.samples.is_empty() {
            let shown: Vec<String> = tally
                .samples
                .iter()
                .map(|(x, y)| format!("({:.1}, {:.1})mm", x * 1000.0, y * 1000.0))
                .collect();
            println!("      novel samples: {}", shown.join("  "));
        }
    }
}

fn main() {
    for fixture in [
        "test-file/DWG-0201GP06-01.pid",
        "test-file/DWG-0202GP06-01.pid",
        "test-file/D06.pid",
        "test-file/工艺管道及仪表流程-1.pid",
    ] {
        let path = Path::new(fixture);
        if path.exists() {
            probe(path);
        }
    }
}
