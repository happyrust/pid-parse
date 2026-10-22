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

use pid_parse::style_link::{LineStyleIndex, ResolvedLineStyle, TextAlignment, TextStyleIndex};
use pid_parse::symbol_library::{PrimitiveStyle, SymbolLettering, SymbolLibrary, SymbolPrimitive};
use pid_parse::{
    PidGeometryConfidence, PidGraphicEntity, PidGraphicKind, PidParser, SheetRecordKind,
    build_normalized_geometry,
};
use serde::{Deserialize, Serialize};

const MM_PER_METRE: f64 = 1000.0;

/// Line pitch of a label whose paragraph states no usable multiple.
///
/// `JStyleTextPara +66` carries the multiple, and `pid_parse::style_link`
/// hands it over only inside `0.25..=4.0` — the stored `0.0` reads as
/// undeclared. A consumer then has nothing better to assume than single
/// spacing; the one option ruled out is stacking the lines on top of each
/// other.
const SINGLE_LINE_SPACING: f64 = 1.0;

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
    /// Strokes left out because they lie a page diagonal or more away from
    /// the sheet.
    ///
    /// An inferred record can promote a pair of doubles that are not
    /// coordinates; printing those would draw a diagonal across the page that
    /// no plot has. The parser keeps them as audit evidence, the page does
    /// not print them, and this count is how many were refused.
    pub off_sheet_primitives: usize,
    /// Entities left out because their only geometry is an `EndpointPair`
    /// promotion — a guess that the pair of numbers a record names are the
    /// endpoints of a line.
    ///
    /// The parser keeps those as inferred lines because they are the evidence
    /// for how the record reads at all; a page draws what the drawing states.
    pub skipped_endpoint_pairs: usize,
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
        /// The stored point stays the insertion point. The middle moves the
        /// PDF baseline down across the run by half the body size, the top by
        /// one body size, and the ascender line by 1.32 body sizes, the same
        /// way the horizontal half moves it back along the run. Symbol
        /// lettering is [`TextLine::Middle`]: its record carries no body box,
        /// and the reference puts the templates' own lettering on the middle
        /// line.
        line: TextLine,
        /// The label's own run partition, when it letters its characters in
        /// more than one style. Empty for every sheet label and for a symbol
        /// label in one style; `height_mm` and `font` are the single style a
        /// plain run draws.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        spans: Vec<PaperTextSpan>,
    },
}

/// One run of a label that letters its characters in its own style: `len`
/// characters from `start` (counted in `char`s), in `height_mm` and set in
/// `font`.
///
/// A `.sym` label may mix styles — the pipeline tags alternate two faces for
/// their separators, `Wastewater Pit.sym` letters one character a size larger
/// — and a `PaperShape::Text` carries only one body size under one typeface.
/// Carrying the partition lets the writer advance span by span, which it can
/// do because it measures each run's glyphs anyway.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PaperTextSpan {
    /// First character the run covers, in `char`s of the label's text.
    pub start: u16,
    /// Characters the run covers.
    pub len: u16,
    /// Body size the run letters at, in paper millimetres; `None` takes the
    /// shape's own `height_mm`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height_mm: Option<f64>,
    /// Typeface the run is set in; `None` takes the shape's own `font`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
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
/// labels the drawing letters sit 0.0 mm, 1.25 mm or 3.26 mm below the point
/// for a 2.5 mm body, one mode per class.
///
/// The nibble's two halves name different things, which is only visible once
/// a class with a nibble of `2` is measured on its own: see
/// [`TextLine::from_anchor_code`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextLine {
    /// The insertion point is on the baseline: the label letters at the point.
    Bottom,
    /// The insertion point is the middle of the body: the baseline runs half a
    /// body size below the point.
    Middle,
    /// The insertion point is the top of the body box: the baseline runs one
    /// body size below the point.
    ///
    /// This is the step a symbol body's own `1` label still takes, and the
    /// sheet's `2` label: both measure one body, where the sheet's `1`
    /// measures a third more.
    Top,
    /// The insertion point is the font's ascender line: the baseline runs 1.32
    /// body sizes below the point.
    ///
    /// A sheet `igTextBox` top does not stop at the cap line. Its labels sit
    /// a third of a body below a one-body step, which is Arial's ascent
    /// (0.905 em) measured against the body size the style table stores as the
    /// cap height (0.688 em): 0.905 / 0.688 = 1.32. See
    /// [`TextLine::from_anchor_code`].
    Ascender,
}

impl TextLine {
    /// The vertical hot point an `igTextBox` anchor code states.
    ///
    /// The corpus carries five low nibbles, and each one's mode against the
    /// reference plots is unambiguous (`examples/probe_text_anchor_modes.rs`,
    /// offset over letter height; the I drawing alone is quoted here, the
    /// five-drawing run gives the same modes):
    ///
    /// | nibble | mode | n on I |
    /// |---:|---:|---:|
    /// | 0 | 0.00 body — the point is the baseline | 3 |
    /// | 1 | 1.25–1.38 body — the ascender line | 58 |
    /// | 2 | 0.99–1.02 body — the body's top | 4 |
    /// | 5 | 0.47–0.50 body — the body's middle | 400 |
    /// | 8 | 0.49–0.69 body — the body's middle | 52 |
    ///
    /// So the two halves of the nibble are separate flags, not one index: the
    /// high half (`4`/`8`) asks for the body's middle and overrides; otherwise
    /// the low half names the line, and it names **three** lines, not one —
    /// `0` the baseline, `1` the ascender, `2` the body's top. The four `2`
    /// records on the I drawing are the spec-break fragments `B5` / `E5` /
    /// `F9`, which the reference puts 2.5 mm below the point against a 2.47 mm
    /// body, where the 58 `1` records sit 3.26 mm down.
    ///
    /// Reading the nibble as an index — `% 3` — puts every `2` at the middle
    /// instead of the top, half a body too high: that is the 18 records
    /// whose labels lettered 1.2 mm above the reference
    /// (`docs/analysis/2026-10-19-a-pair-of-flags-not-an-index.md`).
    ///
    /// The top step used to be read as one whole body — nibble `1` lettered
    /// 1.0 body below the point. Every one of those labels then sat about
    /// 0.8 mm high: 0.32 body of ascender it never stepped down. The I
    /// drawing's 58 nibble-`1` labels measure 1.25–1.38 bodies, and 0.32 of a
    /// 2.47 mm body is the 0.79 mm the `L=200mm` labels were off by
    /// (`docs/analysis/2026-10-19-top-anchored-labels-sit-on-the-ascender-line.md`).
    fn from_anchor_code(code: u8) -> Self {
        match code & 0x0F {
            0 => Self::Bottom,
            1 | 3 => Self::Ascender,
            2 => Self::Top,
            _ => Self::Middle,
        }
    }

    /// The same field on a symbol body's own lettering record.
    ///
    /// A `.sym` body states its labels' anchor in the same nibble but not with
    /// the same reading. Measured against the reference plots
    /// (`pid-plot/examples/probe_symbol_label_offsets.rs`), the two low nibbles
    /// the corpus's symbol bodies carry both letter from the middle: `0x10` —
    /// the `M` inside the valve bodies — read as a baseline lands 1.06 mm
    /// above the reference, and as the middle line within 0.3 mm, exactly as
    /// `0x12` does. The low nibble `1` classes (the converters' and
    /// flowmeter's `M`) letter from the body's top. So the middle is the
    /// symbol side's default and only the top class stands out; the sheet's
    /// flag pair ([`TextLine::from_anchor_code`]) does not apply here.
    ///
    /// Reading the nibble as an index — `0 => bottom` — is what put the eleven
    /// valve and motor labels about a millimetre high until this reading
    /// landed; see
    /// `docs/analysis/2026-10-19-symbol-label-nibble-hangs-from-the-middle.md`.
    fn from_symbol_anchor_code(code: u8) -> Self {
        match (code & 0x0F) % 3 {
            1 => Self::Top,
            _ => Self::Middle,
        }
    }

    /// Flip top and bottom when a mirrored placement absorbs a vertical flip.
    ///
    /// Only symbol lettering reaches this: a mirrored placement flips a
    /// `.sym` body's own label, never a sheet paragraph. An ascender-anchored
    /// line is kept total anyway and folds to the baseline.
    fn flip_vertical(self) -> Self {
        match self {
            Self::Bottom => Self::Top,
            Self::Middle => Self::Middle,
            Self::Top => Self::Bottom,
            Self::Ascender => Self::Bottom,
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

/// Split one label's text into its printed lines.
///
/// `igTextBox` ends a line with a bare `U+000D` — the corpus has 4 of them and
/// no `U+000A` at all (`docs/analysis/2026-08-22-four-labels-have-a-second-
/// line.md`). The other break code points are accepted too, since a decoder
/// that reads a different vendor's file does not get to choose which one it
/// meets, and `\r\n` counts as one break rather than two.
///
/// Any remaining control character is dropped: no typeface has a glyph for
/// one, and the writer refuses a run it cannot encode, so a stray `\u{0}`
/// would take the whole page down rather than ink nothing. A label that is
/// empty still yields one line, so the primitive count of a blank record does
/// not change.
fn label_lines(value: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                lines.push(std::mem::take(&mut current));
            }
            '\n' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}' => {
                lines.push(std::mem::take(&mut current));
            }
            // A tab is the one control character with a visible meaning: it
            // separates words, so it becomes the space it stands for rather
            // than vanishing and gluing two words together.
            '\t' => current.push(' '),
            _ if ch.is_control() => {}
            _ => current.push(ch),
        }
    }
    lines.push(current);
    // A trailing break names a line with nothing on it; it inks nothing and
    // would only add a run the writer then drops.
    while lines.len() > 1 && lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

///Whether `entity` sits on a layer `SmartPlant` leaves off a printed page:
/// a layer the view filter set hides (`displayed == Some(false)`), or the
/// authored `ConsistencyChecks` review overlay — whose `igPoint2d` records
/// resolve to `lsWarning` / `lsOk` markers shown on screen and omitted from
/// every reference plot.
fn suppressed_from_plot(entity: &PidGraphicEntity) -> bool {
    let Some(layer) = entity.source_layer.as_ref() else {
        return false;
    };
    layer.displayed == Some(false) || layer.name.as_deref() == Some("ConsistencyChecks")
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
    let mut seen_sheet_fills = std::collections::HashSet::new();

    let mut skipped_endpoint_pairs = 0usize;
    for entity in &geometry.entities {
        if suppressed_from_plot(entity) {
            continue;
        }
        // An `EndpointPair` record says two objects are connected; promoting
        // the two positions it names into a *line* is a guess about which
        // numbers in the payload are coordinates. On `DWG-0201GP06-01` that
        // guess draws strokes that cross the tank and run off the sheet, and
        // it is the only drawing in the tree with any: the five corpus
        // drawings, `D06`, `工艺管道及仪表流程-1`, `A01` and the B fixture all
        // produce zero inferred primitives, so nothing measured moves. The
        // parser keeps those entities on purpose (they are the evidence for
        // the endpoint-pair reading, pinned by `tests/parse_real_files.rs`);
        // the page does not print a guess.
        if entity.source.record_kind == Some(SheetRecordKind::EndpointPair) {
            skipped_endpoint_pairs += 1;
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
                            SymbolItem {
                                shape: primitive,
                                style: body_style,
                                lettering,
                            },
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
                            SymbolItem {
                                shape: &styled.primitive,
                                style: styled.style.as_ref(),
                                lettering: styled.lettering.as_ref(),
                            },
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
                &mut seen_sheet_fills,
            ),
        }
    }

    // A stroke that cannot reach the sheet is not paper ink. The inferred
    // `EndpointPair` lines are where this bites: DWG-0202 promotes three of
    // them from pairs the record never meant as coordinates, and their
    // endpoints land 143 and 337 *kilometres* out, which the writer then clips
    // into a diagonal across the page that no plot has. The parse side keeps
    // them on purpose (they are audit evidence for the endpoint-pair reading,
    // pinned by `tests/parse_real_files.rs`); the page refuses them.
    let mut off_sheet = 0usize;
    if let Some(page) = page_mm {
        primitives.retain(|primitive| {
            let printable = !beyond_the_sheet(&primitive.shape, page);
            if !printable {
                off_sheet += 1;
            }
            printable
        });
    }

    Ok(PaperModel {
        library,
        page_mm,
        primitives,
        off_sheet_primitives: off_sheet,
        skipped_endpoint_pairs,
        missing_symbols: missing.into_iter().collect(),
        missing_boxes,
        symbols_found: found.len(),
        symbols_referenced: referenced.len(),
    })
}

/// How many page diagonals a stroke's own extent may span.
///
/// A leader or a label may start off the sheet and run onto it, and the writer
/// clips what hangs over the way a printer does; a stroke longer than its
/// sheet can hold is not content at all.
const SHEET_REACH: f64 = 4.0;

/// Whether both coordinates are finite numbers.
fn finite(point: [f64; 2]) -> bool {
    point[0].is_finite() && point[1].is_finite()
}

/// Whether `shape` spans more than its sheet could hold.
///
/// The inferred `EndpointPair` lines are where this bites: DWG-0202 promotes
/// three of them from pairs the record never meant as coordinates, and one runs
/// from 143 km out to the origin — every point finite, nothing to clamp, and
/// the writer clips it into a diagonal across the page that no plot has. Length
/// is the tell, not position: a real line is at most a few page widths.
fn beyond_the_sheet(shape: &PaperShape, page: [f64; 2]) -> bool {
    let points: Vec<[f64; 2]> = match shape {
        PaperShape::Line { start_mm, end_mm } => vec![*start_mm, *end_mm],
        PaperShape::Arc {
            center_mm,
            radius_mm,
            ..
        }
        | PaperShape::Circle {
            center_mm,
            radius_mm,
        } => {
            let (centre, radius) = (*center_mm, *radius_mm);
            vec![
                [centre[0] - radius, centre[1] - radius],
                [centre[0] + radius, centre[1] + radius],
            ]
        }
        PaperShape::Polyline { points_mm, .. } => points_mm.clone(),
        PaperShape::Fill { chain_mm, .. } => chain_mm.clone(),
        PaperShape::Text { origin_mm, .. } => return !finite(*origin_mm),
    };
    if points.iter().any(|point| !finite(*point)) {
        return true;
    }
    if points.is_empty() {
        return false;
    }
    let (mut min, mut max) = (points[0], points[0]);
    for point in &points[1..] {
        min = [min[0].min(point[0]), min[1].min(point[1])];
        max = [max[0].max(point[0]), max[1].max(point[1])];
    }
    let span = (max[0] - min[0]).hypot(max[1] - min[1]);
    if span > page[0].hypot(page[1]) * SHEET_REACH {
        return true;
    }
    // A short stroke that sits entirely off the sheet inks nothing either.
    let touches = max[0] >= 0.0 && min[0] <= page[0] && max[1] >= 0.0 && min[1] <= page[1];
    !touches
}

struct Placement {
    insertion: [f64; 2],
    rotation: f64,
    scale: [f64; 2],
    symbol: Option<String>,
}

/// The symbol-library roots searched, in order.
///
/// `PID_SYMBOL_LIBRARY` replaces the search outright. Otherwise the corpus's
/// own `Ref` directory comes first, then the library bundled with this
/// repository (`test-file/symbols-full`), and every root is kept — the corpus
/// on this machine carries 644 `.sym` files and the bundled one 618, and
/// neither is a superset: the shipped `Ref` has `I-P Converter.sym` that the
/// bundle lacks, while the bundle has `Gauge Hatch.sym`, ` Line2.sym`,
/// `Cap2.sym` and `ElecTraceLine.sym` that the shipped `Ref` lacks. Stopping
/// at the first directory that exists dropped every one of those on the
/// drawings that needed them, which is why `DWG-0201GP06-01` reported three
/// missing symbols and never drew them.
///
/// A path found in an earlier root still wins, so a project's own library
/// keeps deciding what its drawings look like; the later roots only answer
/// for names the earlier ones do not carry.
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
    let mut roots: Vec<PathBuf> = Vec::new();
    for candidate in [
        Some(corpus.join("Ref")),
        corpus.parent().map(|parent| parent.join("Ref")),
    ]
    .into_iter()
    .flatten()
    {
        let Ok(entries) = std::fs::read_dir(&candidate) else {
            continue;
        };
        let mut found: Vec<PathBuf> = entries
            .flatten()
            .filter(|entry| {
                entry.path().is_dir()
                    && entry
                        .file_name()
                        .to_string_lossy()
                        .to_ascii_lowercase()
                        .contains("symbol")
            })
            .map(|entry| entry.path())
            .collect();
        found.sort();
        for root in found {
            if !roots.iter().any(|seen| seen == &root) {
                roots.push(root);
            }
        }
    }
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../test-file/symbols-full");
    if bundled.is_dir() && !roots.iter().any(|seen| seen == &bundled) {
        roots.push(bundled.clone());
    }
    if roots.is_empty() {
        // Nothing on disk to search: the bundled path is still the name to
        // report, and the reader will refuse every symbol by name.
        let label = vec![bundled.display().to_string()];
        return (roots, label);
    }
    let label = roots
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    (roots, label)
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
                body_style.map(|style| style.dash_mm.iter().map(|segment| segment.abs()).collect())
            })
            .unwrap_or_default(),
    })
}

/// Dedup key for one sheet boundary fill: its ring vertices quantised to
/// micrometres plus the fill colour, so duplicate `igBoundary2d` records on
/// `/Sheet6` (e.g. A's stop-valve checkerboard oid 3033↔3066, 3044↔3056)
/// emit once.
fn sheet_fill_key(chain_mm: &[[f64; 2]], rgb: Option<[u8; 3]>) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    for point in chain_mm {
        ((point[0] * 1e6).round() as i64).hash(&mut hasher);
        ((point[1] * 1e6).round() as i64).hash(&mut hasher);
    }
    rgb.hash(&mut hasher);
    hasher.finish()
}

fn push_entity(
    out: &mut Vec<PaperPrimitive>,
    entity: &PidGraphicEntity,
    page_mm: Option<[f64; 2]>,
    styles: &TextStyleIndex,
    line_styles: &LineStyleIndex,
    fill_styles: &pid_parse::style_link::FillIndex,
    seen_sheet_fills: &mut std::collections::HashSet<u64>,
) {
    if suppressed_from_plot(entity) {
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
                if seen_sheet_fills.insert(sheet_fill_key(&points_mm, rgb)) {
                    out.push(primitive(
                        entity,
                        &placement,
                        None,
                        PaperShape::Fill {
                            chain_mm: points_mm,
                            rgb,
                        },
                    ));
                }
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
            text_box_width,
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
            // A boxed record's run is laid out in its box: the anchor code
            // says where inside that box the insertion sits — left, middle
            // or right — which recovers the box, and the paragraph's
            // alignment then says which edge of it the run hangs from. The
            // title-block fields are the measured case: their insertion is
            // the box's middle, and the reference plots start a left-aligned
            // field half a box width further left (every one of them in the
            // corpus but three) while a right-aligned field ends on the
            // box's right edge — `DWG-0000集05-07` on the I drawing and
            // `DWG-0100集05-42` twice on A, whose runs end 0.05 mm inside
            // it. The vertical stays on the record's anchor code against the
            // *letter* height, which is where the reference puts a boxed
            // label's line. See
            // `docs/analysis/2026-10-19-igtextbox-box-is-where-the-text-starts.md`
            // and `docs/analysis/2026-10-20-a-field-hangs-from-its-box-edge.md`.
            let (origin_m, anchor) = match text_box_width.filter(|width| *width > 0.0) {
                Some(width) => {
                    let across = match TextAnchor::from_anchor_code(*anchor_code) {
                        TextAnchor::Left => 0.0,
                        TextAnchor::Center => width / 2.0,
                        TextAnchor::Right => width,
                    };
                    let (anchor, hang) = match styled.as_ref().and_then(|style| style.alignment) {
                        Some(TextAlignment::Center) => (TextAnchor::Center, width / 2.0),
                        Some(TextAlignment::Right) => (TextAnchor::Right, width),
                        Some(TextAlignment::Left) | None => (TextAnchor::Left, 0.0),
                    };
                    let along = across - hang;
                    (
                        [
                            insertion.x - rotation.cos() * along,
                            insertion.y - rotation.sin() * along,
                        ],
                        anchor,
                    )
                }
                None => (
                    [insertion.x, insertion.y],
                    TextAnchor::from_anchor_code(*anchor_code),
                ),
            };
            // One `igTextBox` can letter several lines: the text carries a
            // `U+000D` where the line ends, and the paragraph's `+66` says how
            // far apart the lines sit as a multiple of the character height.
            // Every one of the corpus's multi-line labels states `1.5`, every
            // single-line one `1.0` (`docs/analysis/2026-08-22-four-labels-
            // have-a-second-line.md`), so the same shape a sibling project
            // imports is used here: one text per line, each shifted along the
            // baseline normal by `height × spacing`.
            //
            // The shift is along the normal, not straight down: labels run at
            // 0°, 90° and 180° on this corpus and a vertical label stacked
            // downward would print over itself. Leaving the lines unstacked
            // is the one answer that is certainly wrong, so a paragraph that
            // states no usable spacing stacks at single spacing.
            let spacing = styled
                .as_ref()
                .and_then(|style| style.line_spacing)
                .unwrap_or(SINGLE_LINE_SPACING);
            let pitch_mm = height_mm
                .filter(|millimetres| *millimetres > 0.0 && millimetres.is_finite())
                .map_or(0.0, |millimetres| millimetres * spacing);
            // The writer drops a run with no usable height anyway, so a label
            // whose height is unknown stacks at zero and still reads one line
            // per value.
            // `origin_m` and the insertion point it came from are metres, so
            // the pitch goes back through the same unit as the height.
            let normal = [rotation.sin(), -rotation.cos()];
            for (index, line) in label_lines(value).into_iter().enumerate() {
                let shift_m = pitch_mm * index as f64 / MM_PER_METRE;
                out.push(primitive(
                    entity,
                    &placement,
                    None,
                    PaperShape::Text {
                        origin_mm: place_metres(
                            [
                                origin_m[0] + normal[0] * shift_m,
                                origin_m[1] + normal[1] * shift_m,
                            ],
                            [0.0, 0.0],
                            0.0,
                            [1.0, 1.0],
                        ),
                        height_mm,
                        rotation: *rotation,
                        value: line,
                        font: styled.as_ref().and_then(|style| style.font_name.clone()),
                        // A boxed record hangs from the box edge its
                        // paragraph's alignment names (above). An unboxed one
                        // keeps the record's own anchor code: `(code >> 4) % 3`
                        // is the horizontal hot point the insertion sits at.
                        anchor,
                        line: TextLine::from_anchor_code(*anchor_code),
                        // A sheet `igTextBox`'s runs are the same partition,
                        // but this path draws one line per value with the
                        // style resolved for the whole record; the sheet side
                        // has not opted into spans yet.
                        spans: Vec::new(),
                    },
                ));
            }
        }
        PidGraphicKind::SymbolInstance { .. }
        | PidGraphicKind::Annotation { .. }
        | PidGraphicKind::Unknown { .. } => {}
    }
}

/// Stamp one complex-dash glyph along `path` at the advance the style
/// names — the W drawing's hydraulic pipes, which print a `└`/`⊢` mark
/// once per fitted cell (`round(len / period)`), centred in the cell or at
/// the segment end for a stub (`cells <= 1`), in an undirected frame so a
/// reversed pipe run keeps the same tick orientation as a forward run.
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
    if !complex.draws() {
        return;
    }
    let period = complex.period_mm();
    if period <= 1e-6 || path_mm.len() < 2 {
        return;
    }
    let placement = Placement {
        insertion: [0.0, 0.0],
        rotation: 0.0,
        scale: [1.0, 1.0],
        symbol: None,
    };
    for pair in path_mm.windows(2) {
        let dx = pair[1][0] - pair[0][0];
        let dy = pair[1][1] - pair[0][1];
        let length = dx.hypot(dy);
        if length < 1e-6 {
            continue;
        }
        let dir_tangent = [dx / length, dy / length];
        // Normalize the glyph's frame to the undirected segment angle in
        // (-π/2, π/2]: a run authored right-to-left (149.8 → 117.1) prints
        // its ticks with the same orientation as a left-to-right run.
        let frame_tangent = if dx < -1e-9 || (dx.abs() <= 1e-9 && dy < 0.0) {
            [-dir_tangent[0], -dir_tangent[1]]
        } else {
            dir_tangent
        };
        let left = [-frame_tangent[1], frame_tangent[0]];
        let cells = (length / period).round() as usize;
        let positions: Vec<f64> = if cells <= 1 {
            vec![length]
        } else {
            let step = length / cells as f64;
            (0..cells).map(|cell| (cell as f64 + 0.5) * step).collect()
        };
        for at in positions {
            let point = [
                pair[0][0] + dir_tangent[0] * at,
                pair[0][1] + dir_tangent[1] * at,
            ];
            for mark in complex.strokes() {
                if mark.is_degenerate() {
                    continue;
                }
                let place = |local: (f64, f64)| {
                    let (lx, ly) = (local.0 * MM_PER_METRE, local.1 * MM_PER_METRE);
                    [
                        point[0] + lx * frame_tangent[0] + ly * left[0],
                        point[1] + lx * frame_tangent[1] + ly * left[1],
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
}

#[derive(Clone, Copy)]
struct SymbolItem<'a> {
    shape: &'a SymbolPrimitive,
    style: Option<&'a PrimitiveStyle>,
    lettering: Option<&'a SymbolLettering>,
}

fn push_symbol_primitive(
    out: &mut Vec<PaperPrimitive>,
    entity: &PidGraphicEntity,
    placement: &Placement,
    item: SymbolItem<'_>,
    placement_style: Option<&ResolvedLineStyle>,
    page_mm: Option<[f64; 2]>,
) {
    let primitive_shape = item.shape;
    let body_style = item.style;
    let lettering = item.lettering;
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
        SymbolPrimitive::Circle { center, radius } => {
            circle_shape([center.0, center.1], *radius, placement)
        }
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
                        let radius_mm = radius.abs()
                            * placement.scale[0].abs().max(placement.scale[1].abs())
                            * MM_PER_METRE;
                        if radius_mm > 1e-4 {
                            out.push(primitive(
                                entity,
                                placement,
                                None,
                                PaperShape::Fill {
                                    chain_mm: circle_chain(
                                        [center.0, center.1],
                                        *radius,
                                        placement,
                                    ),
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
                && ring.first().zip(ring.last()).is_some_and(|(first, last)| {
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

/// A circle as a closed vertex ring in paper millimetres, placed through
/// [`place_metres`] so non-uniform scales deform the ring into an ellipse.
fn circle_chain(center: [f64; 2], radius: f64, placement: &Placement) -> Vec<[f64; 2]> {
    const SEGMENTS: usize = 48;
    let r = radius.abs();
    (0..SEGMENTS)
        .map(|step| {
            let angle = std::f64::consts::TAU * step as f64 / SEGMENTS as f64;
            place_metres(
                [center[0] + r * angle.cos(), center[1] + r * angle.sin()],
                placement.insertion,
                placement.rotation,
                placement.scale,
            )
        })
        .collect()
}

fn circle_shape(center: [f64; 2], radius: f64, placement: &Placement) -> PaperShape {
    let sx = placement.scale[0].abs();
    let sy = placement.scale[1].abs();
    if (sx - sy).abs() > 1e-6 {
        return PaperShape::Polyline {
            points_mm: circle_chain(center, radius, placement),
            closed: true,
        };
    }
    PaperShape::Circle {
        center_mm: place_metres(
            center,
            placement.insertion,
            placement.rotation,
            placement.scale,
        ),
        radius_mm: radius.abs() * sx * MM_PER_METRE,
    }
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
            TextLine::from_symbol_anchor_code(lettering.anchor_code)
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
        spans: lettering.map_or_else(Vec::new, |lettering| {
            lettering
                .spans
                .iter()
                .map(|span| PaperTextSpan {
                    start: span.start,
                    len: span.len,
                    height_mm: span
                        .height_m
                        .filter(|height| *height > 0.0)
                        .map(|height| height * scale * MM_PER_METRE),
                    font: span.font_name.clone(),
                })
                .collect()
        }),
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
    let sx = placement.scale[0].abs();
    let sy = placement.scale[1].abs();
    if (sx - sy).abs() > 1e-6 {
        let sweep = (start - end).rem_euclid(std::f64::consts::TAU);
        let r = radius.abs();
        let steps = ((sweep * r * sx.max(sy) * MM_PER_METRE / 0.15).ceil() as usize).clamp(8, 64);
        let points_mm = (0..=steps)
            .map(|step| {
                let angle = start - sweep * (step as f64 / steps as f64);
                place_metres(
                    [center[0] + r * angle.cos(), center[1] + r * angle.sin()],
                    placement.insertion,
                    placement.rotation,
                    placement.scale,
                )
            })
            .collect();
        return PaperShape::Polyline {
            points_mm,
            closed: false,
        };
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
        radius_mm: radius.abs() * sx * MM_PER_METRE,
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
    use super::label_lines;
    use pid_parse::style_link::TextAlignment;
    use pid_parse::symbol_library::{SymbolLettering, SymbolPrimitive};
    use pid_parse::{
        PidCoordinateContext, PidGeometryConfidence, PidGraphicEntity, PidGraphicKind,
        PidGraphicProvenance, PidPoint, PidSourceLayer,
    };

    use super::{
        PaperShape, Placement, SymbolItem, TextAnchor, TextLine, arc_shape, place_metres,
        push_entity, push_symbol_primitive,
    };

    fn plain_item(shape: &SymbolPrimitive) -> SymbolItem<'_> {
        SymbolItem {
            shape,
            style: None,
            lettering: None,
        }
    }

    fn lettered_item<'a>(
        shape: &'a SymbolPrimitive,
        lettering: &'a SymbolLettering,
    ) -> SymbolItem<'a> {
        SymbolItem {
            shape,
            style: None,
            lettering: Some(lettering),
        }
    }

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

    /// A boxed record letters from its box's left edge, not from the
    /// insertion point the anchor code measures inside it.
    ///
    /// The title-block fields are the records this is about: 58 of them
    /// across the five drawings carry a 45.307 mm box and state a centred
    /// anchor, and the reference plots start every one of them half a box
    /// width to the left of the insertion point, which is where a centred
    /// placement puts it. The vertical half is untouched: the reference keeps
    /// the record's own anchor line against the letter height, so only the
    /// horizontal run moves.
    #[test]
    fn a_boxed_record_letters_from_the_box_left_edge() {
        let mut out = Vec::new();
        push_entity(
            &mut out,
            &entity(PidGraphicKind::Text {
                insertion: PidPoint { x: 0.1, y: 0.2 },
                value: "DWG-0100集05-42".to_string(),
                height: 0.0025,
                rotation: 0.0,
                // Centre, middle — the code every one of those records states.
                anchor_code: 0x48,
                text_box_width: Some(0.045_307),
            }),
            None,
            &pid_parse::style_link::TextStyleIndex::new(),
            &pid_parse::style_link::LineStyleIndex::new(),
            &pid_parse::style_link::FillIndex::new(),
            &mut std::collections::HashSet::new(),
        );
        let PaperShape::Text {
            origin_mm,
            anchor,
            line,
            ..
        } = &out[0].shape
        else {
            panic!("{:?}", out[0].shape);
        };
        assert_eq!(*anchor, TextAnchor::Left);
        assert_eq!(*line, TextLine::Middle);
        assert!(
            (origin_mm[0] - (100.0 - 0.045_307 / 2.0 * 1000.0)).abs() < 1e-6,
            "box left edge, got {origin_mm:?}"
        );
        assert!((origin_mm[1] - 200.0).abs() < 1e-6);
    }

    fn text_style(line_spacing: Option<f64>) -> pid_parse::style_link::TextStyleIndex {
        use pid_parse::style_link::{ResolvedTextHeight, ResolvedTextStyle, TextRunStatus};

        let mut styles = pid_parse::style_link::TextStyleIndex::new();
        styles.insert(
            ("/Sheet6".to_string(), 11),
            ResolvedTextStyle {
                paragraph: Some(ResolvedTextHeight {
                    style_id: 3,
                    height_m: 0.0035,
                    colour: None,
                    alignment: None,
                    line_spacing,
                    font_name: Some("仿宋".to_string()),
                }),
                run: None,
                runs: TextRunStatus::NoRuns,
                alignment: None,
                line_spacing,
                spans: Vec::new(),
            },
        );
        styles
    }

    fn note(rotation: f64, value: &str) -> PidGraphicEntity {
        let mut record = entity(PidGraphicKind::Text {
            insertion: PidPoint { x: 0.1, y: 0.2 },
            value: value.to_string(),
            height: 0.0035,
            rotation,
            anchor_code: 0x00,
            text_box_width: None,
        });
        record.graphic_oid = Some(11);
        record.source.stream_path = Some("/Sheet6".to_string());
        record
    }

    /// A label that states two lines prints two runs, the second one a full
    /// line pitch below the first.
    ///
    /// The 2-line note on `DWG-0202GP06-01` is the case: its text carries a
    /// `U+000D` and its paragraph states `1.5`, so the second line sits
    /// `1.5 × 3.5 mm = 5.25 mm` under the first one's baseline. Before this the
    /// whole value went to the writer as one run, and the writer refused the
    /// page because no face has a glyph for `\r`.
    #[test]
    fn a_two_line_label_stacks_at_the_stated_spacing() {
        let mut out = Vec::new();
        push_entity(
            &mut out,
            &note(
                0.0,
                "1、上放空管线高度应大于5m。\r2、电伴热带缠至地面以上2m。",
            ),
            None,
            &text_style(Some(1.5)),
            &pid_parse::style_link::LineStyleIndex::new(),
            &pid_parse::style_link::FillIndex::new(),
            &mut std::collections::HashSet::new(),
        );
        assert_eq!(out.len(), 2, "one run per line");
        let (first, second) = match (&out[0].shape, &out[1].shape) {
            (PaperShape::Text { origin_mm: a, .. }, PaperShape::Text { origin_mm: b, .. }) => {
                (*a, *b)
            }
            other => panic!("{other:?}"),
        };
        assert!((first[0] - second[0]).abs() < 1e-9, "same column");
        assert!(
            (first[1] - second[1] - 3.5 * 1.5).abs() < 1e-6,
            "a line pitch of height × 1.5, got {} mm",
            first[1] - second[1]
        );
    }

    /// A vertical label stacks along its baseline normal, not straight down:
    /// text turned 90° would otherwise print over itself.
    #[test]
    fn a_vertical_label_stacks_along_its_baseline_normal() {
        let mut out = Vec::new();
        push_entity(
            &mut out,
            &note(std::f64::consts::FRAC_PI_2, "安3集气站\r排污单元"),
            None,
            &text_style(Some(1.5)),
            &pid_parse::style_link::LineStyleIndex::new(),
            &pid_parse::style_link::FillIndex::new(),
            &mut std::collections::HashSet::new(),
        );
        assert_eq!(out.len(), 2);
        let (first, second) = match (&out[0].shape, &out[1].shape) {
            (PaperShape::Text { origin_mm: a, .. }, PaperShape::Text { origin_mm: b, .. }) => {
                (*a, *b)
            }
            other => panic!("{other:?}"),
        };
        assert!(
            (second[0] - first[0] - 3.5 * 1.5).abs() < 1e-6,
            "the next line of vertical text sits on +x, got {second:?} against {first:?}"
        );
        assert!((second[1] - first[1]).abs() < 1e-9, "same column");
    }

    /// A paragraph that states no usable spacing stacks at single spacing —
    /// stacking on one baseline is the only answer that is certainly wrong.
    #[test]
    fn a_paragraph_without_spacing_stacks_at_single_spacing() {
        let mut out = Vec::new();
        push_entity(
            &mut out,
            &note(0.0, "第一行\r第二行"),
            None,
            &text_style(None),
            &pid_parse::style_link::LineStyleIndex::new(),
            &pid_parse::style_link::FillIndex::new(),
            &mut std::collections::HashSet::new(),
        );
        assert_eq!(out.len(), 2);
        let (first, second) = match (&out[0].shape, &out[1].shape) {
            (PaperShape::Text { origin_mm: a, .. }, PaperShape::Text { origin_mm: b, .. }) => {
                (*a, *b)
            }
            other => panic!("{other:?}"),
        };
        assert!((first[1] - second[1] - 3.5).abs() < 1e-6);
    }

    /// A stroke whose every point is more than a page diagonal off the sheet
    /// is refused; one that merely starts off the sheet and runs onto it is
    /// kept, because the writer clips the overhang the way a printer does.
    ///
    /// DWG-0202's three `EndpointPair` promotions are the case: 143 and 337 km
    /// away, and the writer clipped them into a diagonal across the page that
    /// no plot has.
    #[test]
    fn a_stroke_that_cannot_reach_the_sheet_is_refused() {
        use super::beyond_the_sheet;

        let page = [841.0, 594.0];
        // One end at the origin, the other 143 km out: finite, and the writer
        // would clip it into a diagonal right across the page.
        let far = PaperShape::Line {
            start_mm: [143_616_000.0, 143_872_000.0],
            end_mm: [0.0, 0.0],
        };
        assert!(beyond_the_sheet(&far, page));

        // A leader that starts off the sheet and runs onto it prints, and the
        // writer clips the overhang the way a printer does.
        let overhang = PaperShape::Line {
            start_mm: [-40.0, 300.0],
            end_mm: [200.0, 300.0],
        };
        assert!(!beyond_the_sheet(&overhang, page));

        // A label just past the right edge is content, not noise.
        let off_page = PaperShape::Text {
            origin_mm: [841.0 + 40.0, 30.0],
            height_mm: Some(3.0),
            rotation: 0.0,
            value: "M".to_string(),
            font: None,
            anchor: TextAnchor::Left,
            line: TextLine::Middle,
            spans: Vec::new(),
        };
        assert!(!beyond_the_sheet(&off_page, page));

        // A short stroke parked entirely off the sheet inks nothing.
        let parked = PaperShape::Line {
            start_mm: [143_616_000.0, 143_872_000.0],
            end_mm: [143_616_001.0, 143_872_001.0],
        };
        assert!(beyond_the_sheet(&parked, page));

        // A non-finite coordinate is never ink.
        let broken = PaperShape::Line {
            start_mm: [f64::NAN, 0.0],
            end_mm: [10.0, 10.0],
        };
        assert!(beyond_the_sheet(&broken, page));
    }

    /// Every break spelling splits, `\r\n` counts once, a trailing break adds
    /// no empty line, and a control character no face can encode never reaches
    /// the writer.
    #[test]
    fn a_label_splits_on_every_line_break_and_drops_other_controls() {
        assert_eq!(label_lines("a\rb"), ["a", "b"]);
        assert_eq!(label_lines("a\r\nb"), ["a", "b"]);
        assert_eq!(label_lines("a\nb"), ["a", "b"]);
        assert_eq!(
            label_lines("a\u{0B}b\u{0C}c\u{2028}d\u{2029}e"),
            ["a", "b", "c", "d", "e"]
        );
        assert_eq!(label_lines("a\r"), ["a"]);
        assert_eq!(label_lines("a\u{0}b\tc"), ["ab c"]);
        assert_eq!(label_lines("\u{0}"), [""]);
        assert_eq!(label_lines(""), [""]);
    }

    /// A boxed field whose paragraph states right alignment ends on the
    /// box's right edge, not its left one.
    ///
    /// The three records this is about are the title block's drawing-number
    /// fields on the I and A drawings (`DWG-0000集05-07` once, `DWG-0100集05-42`
    /// twice): each states a centred anchor in a 45.307 mm box, and the
    /// reference plots end their runs 0.05 mm inside the box's right edge.
    /// Reading every boxed record as left-aligned put them 17.6 mm left.
    #[test]
    fn a_right_aligned_boxed_record_ends_on_the_box_right_edge() {
        use pid_parse::style_link::{ResolvedTextHeight, ResolvedTextStyle, TextRunStatus};

        let mut styles = pid_parse::style_link::TextStyleIndex::new();
        styles.insert(
            ("/Sheet6".to_string(), 7),
            ResolvedTextStyle {
                paragraph: Some(ResolvedTextHeight {
                    style_id: 3,
                    height_m: 0.0037,
                    colour: None,
                    alignment: Some(TextAlignment::Right),
                    line_spacing: None,
                    font_name: Some("仿宋".to_string()),
                }),
                run: None,
                runs: TextRunStatus::NoRuns,
                alignment: Some(TextAlignment::Right),
                line_spacing: None,
                spans: Vec::new(),
            },
        );
        let mut record = entity(PidGraphicKind::Text {
            insertion: PidPoint { x: 0.1, y: 0.2 },
            value: "DWG-0000集05-07".to_string(),
            height: 0.0037,
            rotation: 0.0,
            anchor_code: 0x48,
            text_box_width: Some(0.045_307),
        });
        record.graphic_oid = Some(7);
        record.source.stream_path = Some("/Sheet6".to_string());
        let mut out = Vec::new();
        push_entity(
            &mut out,
            &record,
            None,
            &styles,
            &pid_parse::style_link::LineStyleIndex::new(),
            &pid_parse::style_link::FillIndex::new(),
            &mut std::collections::HashSet::new(),
        );
        let PaperShape::Text {
            origin_mm, anchor, ..
        } = &out[0].shape
        else {
            panic!("{:?}", out[0].shape);
        };
        assert_eq!(*anchor, TextAnchor::Right);
        assert!(
            (origin_mm[0] - (100.0 + 0.045_307 / 2.0 * 1000.0)).abs() < 1e-6,
            "box right edge, got {origin_mm:?}"
        );
    }

    /// A left-anchored boxed record already sits on its box's left edge, so
    /// the box moves it nowhere — the anchor code, not the box alone, is what
    /// says where the run starts.
    #[test]
    fn a_left_anchored_boxed_record_does_not_move() {
        let mut out = Vec::new();
        push_entity(
            &mut out,
            &entity(PidGraphicKind::Text {
                insertion: PidPoint { x: 0.1, y: 0.2 },
                value: "M".to_string(),
                height: 0.0025,
                rotation: 0.0,
                // Left, middle: the insertion is the box's left edge.
                anchor_code: 0x08,
                text_box_width: Some(0.045_307),
            }),
            None,
            &pid_parse::style_link::TextStyleIndex::new(),
            &pid_parse::style_link::LineStyleIndex::new(),
            &pid_parse::style_link::FillIndex::new(),
            &mut std::collections::HashSet::new(),
        );
        let PaperShape::Text {
            origin_mm, anchor, ..
        } = &out[0].shape
        else {
            panic!("{:?}", out[0].shape);
        };
        assert_eq!(*anchor, TextAnchor::Left);
        assert!((origin_mm[0] - 100.0).abs() < 1e-6, "got {origin_mm:?}");
    }

    /// The same record without a box keeps its anchor code's own placement.
    #[test]
    fn an_unboxed_record_keeps_its_anchor_code() {
        let mut out = Vec::new();
        push_entity(
            &mut out,
            &entity(PidGraphicKind::Text {
                insertion: PidPoint { x: 0.1, y: 0.2 },
                value: "TAG".to_string(),
                height: 0.0025,
                rotation: 0.0,
                anchor_code: 0x48,
                text_box_width: None,
            }),
            None,
            &pid_parse::style_link::TextStyleIndex::new(),
            &pid_parse::style_link::LineStyleIndex::new(),
            &pid_parse::style_link::FillIndex::new(),
            &mut std::collections::HashSet::new(),
        );
        let PaperShape::Text {
            origin_mm, anchor, ..
        } = &out[0].shape
        else {
            panic!("{:?}", out[0].shape);
        };
        assert_eq!(*anchor, TextAnchor::Center);
        assert!((origin_mm[0] - 100.0).abs() < 1e-6);
    }

    /// The five low nibbles the corpus carries, against the line each one's
    /// labels actually letter on in the reference plots. `2` is a top, not a
    /// middle: the nibble's halves are two flags, and an index reading puts
    /// those labels half a body too high.
    #[test]
    fn the_vertical_nibble_is_a_pair_of_flags_not_an_index() {
        let lines: Vec<TextLine> = [0x00_u8, 0x01, 0x02, 0x05, 0x08]
            .into_iter()
            .map(TextLine::from_anchor_code)
            .collect();
        assert_eq!(
            lines,
            vec![
                TextLine::Bottom,
                TextLine::Ascender,
                TextLine::Top,
                TextLine::Middle,
                TextLine::Middle,
            ]
        );
        // The horizontal half is unchanged, and the two halves do not
        // interact: `0x48` is still centre/middle.
        assert_eq!(TextAnchor::from_anchor_code(0x48), TextAnchor::Center);
        assert_eq!(TextLine::from_anchor_code(0x48), TextLine::Middle);
        assert_eq!(TextAnchor::from_anchor_code(0x12), TextAnchor::Center);
        // `2` is the body's top, one body down; only `1` climbs to the
        // ascender line a third of a body higher.
        assert_eq!(TextLine::from_anchor_code(0x12), TextLine::Top);
        assert_eq!(TextLine::from_anchor_code(0x11), TextLine::Ascender);
    }

    /// A symbol body's own labels keep the other reading: `0x10` is the
    /// middle line, not the baseline — the ten `M` labels inside the corpus's
    /// valves were the only class the sheet's flag pair got wrong.
    #[test]
    fn a_symbol_label_nibble_hangs_from_the_middle() {
        let lines: Vec<TextLine> = [0x10_u8, 0x11, 0x12, 0x15, 0x18]
            .into_iter()
            .map(TextLine::from_symbol_anchor_code)
            .collect();
        assert_eq!(
            lines,
            vec![
                TextLine::Middle,
                TextLine::Top,
                TextLine::Middle,
                TextLine::Middle,
                TextLine::Middle,
            ]
        );
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
        let shape = SymbolPrimitive::Text {
            text: " NULL ".to_string(),
            at: (0.0, 0.0),
        };
        push_symbol_primitive(
            &mut out,
            &entity(PidGraphicKind::Point {
                position: PidPoint { x: 0.0, y: 0.0 },
            }),
            &placement(),
            plain_item(&shape),
            None,
            None,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn a_doubled_placeholder_null_is_left_out_of_the_table() {
        let mut out = Vec::new();
        let shape = SymbolPrimitive::Text {
            text: "NULLNULL".to_string(),
            at: (0.0, 0.0),
        };
        push_symbol_primitive(
            &mut out,
            &entity(PidGraphicKind::Point {
                position: PidPoint { x: 0.0, y: 0.0 },
            }),
            &placement(),
            plain_item(&shape),
            None,
            None,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn a_symbol_label_that_mentions_null_stays_in_the_table() {
        let mut out = Vec::new();
        let shape = SymbolPrimitive::Text {
            text: "HH=NULL".to_string(),
            at: (0.01, 0.0),
        };
        push_symbol_primitive(
            &mut out,
            &entity(PidGraphicKind::Point {
                position: PidPoint { x: 0.0, y: 0.0 },
            }),
            &placement(),
            plain_item(&shape),
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
                text_box_width: None,
            }),
            None,
            &pid_parse::style_link::TextStyleIndex::new(),
            &pid_parse::style_link::LineStyleIndex::new(),
            &pid_parse::style_link::FillIndex::new(),
            &mut std::collections::HashSet::new(),
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
            text_box_width: None,
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
            &mut std::collections::HashSet::new(),
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
            run_style_id: None,
            spans: Vec::new(),
        };
        let shape = SymbolPrimitive::Text {
            text: "ESD".to_string(),
            at: (0.01, 0.02),
        };
        push_symbol_primitive(
            &mut out,
            &entity(PidGraphicKind::Point {
                position: PidPoint { x: 0.0, y: 0.0 },
            }),
            &placement(),
            lettered_item(&shape, &lettering),
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
        // `0x10` is the middle line on a symbol body, not the baseline.
        assert_eq!(*line, TextLine::Middle);
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
            run_style_id: None,
            spans: Vec::new(),
        };
        let mut out = Vec::new();
        let mut mirrored_pi = placement();
        mirrored_pi.rotation = std::f64::consts::PI;
        mirrored_pi.scale = [1.0, -1.0];
        let esd = SymbolPrimitive::Text {
            text: "ESD".to_string(),
            at: (0.0, 0.0),
        };
        push_symbol_primitive(
            &mut out,
            &entity(PidGraphicKind::Point {
                position: PidPoint { x: 0.0, y: 0.0 },
            }),
            &mirrored_pi,
            lettered_item(&esd, &lettering),
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
        // A symbol body states no baseline class, so the flip is exercised
        // from the top: `0x11` letters from the body's top and a vertical
        // mirror puts it on the baseline.
        let mut top_lettering = lettering.clone();
        top_lettering.anchor_code = 0x11;
        let d_shape = SymbolPrimitive::Text {
            text: "D".to_string(),
            at: (0.0, 0.0),
        };
        push_symbol_primitive(
            &mut out_vert,
            &entity(PidGraphicKind::Point {
                position: PidPoint { x: 0.0, y: 0.0 },
            }),
            &mirrored_neg_half,
            lettered_item(&d_shape, &top_lettering),
            None,
            None,
        );
        let PaperShape::Text { rotation, line, .. } = &out_vert[0].shape else {
            panic!("{:?}", out_vert[0].shape);
        };
        assert!((*rotation + std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        assert_eq!(*line, TextLine::Bottom);
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

    #[test]
    fn non_uniform_scaled_arc_and_circle_tessellate_into_polylines() {
        let mut stretched = placement();
        stretched.scale = [1.0, 2.0];
        let PaperShape::Polyline {
            points_mm: arc_pts,
            closed: arc_closed,
        } = arc_shape(
            [0.0, 0.0],
            0.01,
            std::f64::consts::FRAC_PI_2,
            0.0,
            &stretched,
        )
        else {
            panic!("expected polyline for non-uniform arc");
        };
        assert!(!arc_closed);
        assert!(arc_pts.len() >= 9);
        assert!(arc_pts[0][0].abs() < 1e-6 && (arc_pts[0][1] - 20.0).abs() < 1e-6);
        let last = *arc_pts.last().unwrap();
        assert!((last[0] - 10.0).abs() < 1e-6 && last[1].abs() < 1e-6);

        let mut out = Vec::new();
        let circle = SymbolPrimitive::Circle {
            center: (0.0, 0.0),
            radius: 0.01,
        };
        push_symbol_primitive(
            &mut out,
            &entity(PidGraphicKind::Point {
                position: PidPoint { x: 0.0, y: 0.0 },
            }),
            &stretched,
            plain_item(&circle),
            None,
            None,
        );
        let PaperShape::Polyline {
            points_mm: circle_pts,
            closed: circle_closed,
        } = &out[0].shape
        else {
            panic!("expected polyline for non-uniform circle");
        };
        assert!(*circle_closed);
        assert_eq!(circle_pts.len(), 48);
    }

    #[test]
    fn consistency_checks_layer_entities_are_suppressed_from_plot() {
        let mut out = Vec::new();
        let mut review_mark = entity(PidGraphicKind::Line {
            start: PidPoint { x: 0.1, y: 0.1 },
            end: PidPoint { x: 0.103, y: 0.106 },
        });
        review_mark.source_layer = Some(PidSourceLayer {
            oid: 18,
            name: Some("ConsistencyChecks".to_string()),
            storage_path: "/".to_string(),
            displayed: Some(true),
        });
        push_entity(
            &mut out,
            &review_mark,
            None,
            &pid_parse::style_link::TextStyleIndex::new(),
            &pid_parse::style_link::LineStyleIndex::new(),
            &pid_parse::style_link::FillIndex::new(),
            &mut std::collections::HashSet::new(),
        );
        assert!(out.is_empty());
    }

    #[test]
    fn duplicate_sheet_boundary_fills_emit_once() {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let make = |oid: u32| {
            let mut e = entity(PidGraphicKind::Polyline {
                points: vec![
                    PidPoint { x: 0.1, y: 0.1 },
                    PidPoint { x: 0.11, y: 0.1 },
                    PidPoint { x: 0.105, y: 0.11 },
                ],
                closed: true,
            });
            e.graphic_oid = Some(oid);
            e.source.record_id = Some("igboundary2d:0".to_string());
            e
        };
        push_entity(
            &mut out,
            &make(3033),
            None,
            &pid_parse::style_link::TextStyleIndex::new(),
            &pid_parse::style_link::LineStyleIndex::new(),
            &pid_parse::style_link::FillIndex::new(),
            &mut seen,
        );
        push_entity(
            &mut out,
            &make(3066),
            None,
            &pid_parse::style_link::TextStyleIndex::new(),
            &pid_parse::style_link::LineStyleIndex::new(),
            &pid_parse::style_link::FillIndex::new(),
            &mut seen,
        );
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0].shape, PaperShape::Fill { .. }));
    }
}
