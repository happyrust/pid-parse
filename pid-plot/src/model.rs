//! Paper-space primitives for one parsed drawing.
//!
//! Coordinates are millimetres, origin at the bottom-left, y up. A symbol
//! body is placed by scaling, then rotating counter-clockwise, then
//! translating by the insertion point. A negative scale mirrors. Text keeps
//! the insertion point on the table; a centre or right paragraph moves the
//! PDF baseline back along the run when the page is written. A symbol's own
//! lettering that is only the placeholder `NULL`, including a doubled
//! `NULLNULL`, stays out of the table; text the drawing itself records is
//! kept.
//!
//! The sheet's furniture — its double frame, title block and sign-off column —
//! is not in the drawing: the drawing links an OLE frame to an `.igr` template
//! file that carries it. That template is laid out first, so the drawing's own
//! records ink over it, and the page reports which file it came from in
//! [`PaperModel::template`].

use std::path::{Path, PathBuf};

use pid_parse::refdata_archive::{
    ArchiveFile, archive_members, read_archive_file, refdata_archives,
};
use pid_parse::style_link::{
    LineStyleIndex, MarkerStroke, ResolvedLineStyle, ResolvedTextStyle, TextAlignment,
    TextStyleIndex,
};
use pid_parse::symbol_library::{
    NameMiss, PrimitiveStyle, SymbolLettering, SymbolLibrary, SymbolPrimitive,
};
use pid_parse::{
    NormalizedPidGeometry, PidDocument, PidGeometryConfidence, PidGraphicEntity, PidGraphicKind,
    PidParser, SheetRecordKind, build_normalized_geometry,
};
use serde::{Deserialize, Serialize};

use crate::write::carries_a_pen;

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
    /// The `.igr` border template drawn under the content, when the sheet has
    /// one.
    ///
    /// A P&ID's border is a linked OLE object, so the frame, title block and
    /// sign-off column are not in the `.pid` at all — they live in the template
    /// file the frame record points at. This is the file whose furniture the
    /// page carries; see [`border_template_files`] for how it is found.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub template: Option<BorderTemplate>,
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

/// One `.igr` border template, as drawn.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BorderTemplate {
    /// Template file the furniture came from, as found on disk or — when the
    /// corpus carries none, and a backup package's `RefData~*.zip` does — as
    /// the archive path with the member's own name on the end.
    pub path: String,
    /// The template's own ink extent, paper millimetres.
    ///
    /// It equals the sheet, because the template's outer frame runs along the
    /// page edge; that equality is also how the file was picked.
    pub extent_mm: [f64; 2],
    /// How many primitives the template contributed, labels included.
    pub primitives: usize,
    /// Every symbol the template places, with what drew it.
    ///
    /// The page keeps a tally of its own for the template's placements — a
    /// template's missing `.sym` files are not the drawing's — so this is
    /// where those misses are read. A placement drawn from
    /// [`SymbolBodySource::Cached`] with
    /// [`TemplateSymbol::library_available`] `false` is a symbol whose only
    /// source is the template's own copy: leftover reference ink there is a
    /// missing source, not a decoding error.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbols: Vec<TemplateSymbol>,
}

/// What a placement's strokes were drawn from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolBodySource {
    /// The `.sym` the placement names, as the symbol library read it.
    Library,
    /// The body the file itself carries — the placement's own
    /// `(JSheet, LdcSite)` pair, resolved against the file's definition cache.
    Cached,
    /// Neither body: nothing was drawn, and the page carries a box where the
    /// placement stood.
    Missing,
}

/// One symbol a page placed, and what drew it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TemplateSymbol {
    /// Symbol path the placement names, spelled as the file stores it.
    ///
    /// Usually share-relative (`Temp\BLFH.sym`), but a title-block placement
    /// authored on the engineer's own machine stores an absolute local path
    /// (`c:\users\…\temp\blfh.sym`). The library has no root to address that
    /// with, so such a path resolves by file name instead — a share holding
    /// exactly one `<name>.sym` supplies the body — and misses when it holds
    /// none or several — see [`SymbolBodySource`].
    pub key: String,
    /// Placement point, paper millimetres.
    pub insertion_mm: [f64; 2],
    /// Which body the strokes came from.
    pub body: SymbolBodySource,
    /// Whether the `.sym` the placement names also resolved in the library.
    ///
    /// `false` beside [`SymbolBodySource::Cached`] is the case worth an
    /// account: the page draws the file's cache of a symbol whose library copy
    /// is gone, so a difference from the reference plot there is a source the
    /// corpus does not carry rather than a decoding error. (When both bodies
    /// exist the cache is still the one drawn — see [`push_symbol_instance`] —
    /// so `true` here does not mean the `.sym` was used.) A placement naming an
    /// author-machine path counts as available only when a root holds one
    /// `.sym` of that file name; several is a refusal, not a pick.
    pub library_available: bool,
    /// Extent the drawn strokes reach, paper millimetres; `None` when nothing
    /// was drawn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extent_mm: Option<[[f64; 2]; 2]>,
    /// Why a placement that names no share got no body by file name, when the
    /// file name is the reason.
    ///
    /// `NoSuchFile` is a source the corpus does not carry. `NotUnique` is not:
    /// the library holds that file name more than once and refuses to pick, so
    /// the ink is a refusal rather than a missing file. `None` beside
    /// [`SymbolBodySource::Cached`] with
    /// [`TemplateSymbol::library_available`] `false` is the remaining case — no
    /// share in the path and no decoded body under that name either.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_gap: Option<NameMiss>,
}

/// Half-size of the box a placement that drew nothing leaves behind.
const MISSING_BOX_HALF_MM: f64 = 15.0;

/// The five ISO A-series sheets, portrait (short side first), millimetres.
///
/// A drawing that states no page falls back to one of these rather than to its
/// ink's own box, so a page whose sheet the file never records still lands on
/// paper a plotter has ([`PaperModel::resolved_page_mm`]).
pub const STANDARD_SHEETS_MM: [[f64; 2]; 5] = [
    [297.0, 210.0],
    [420.0, 297.0],
    [594.0, 420.0],
    [841.0, 594.0],
    [1189.0, 841.0],
];

/// How far past a standard sheet's edge ink may reach and still be called that
/// sheet, millimetres.
///
/// The plant backup carries sheets whose stated frame is `594.33 × 420.57` or
/// `841.15 × 594.40`; those state their own page, so this slack only ever
/// matters for a drawing that states none.
const SHEET_FIT_SLACK_MM: f64 = 5.0;

impl PaperModel {
    /// The border-template placement that owns `point_mm`, grown by
    /// `slack_mm` on every side.
    ///
    /// A placement that drew owns the extent its strokes reach; one that drew
    /// nothing owns the box the page puts where it stood, which is the same
    /// 15 mm half-size [`MissingSymbolBox`] uses.
    pub fn template_symbol_at(&self, point_mm: [f64; 2], slack_mm: f64) -> Option<&TemplateSymbol> {
        self.template.as_ref()?.symbols.iter().find(|symbol| {
            let [min, max] = symbol.extent_mm.unwrap_or_else(|| {
                [
                    [
                        symbol.insertion_mm[0] - MISSING_BOX_HALF_MM,
                        symbol.insertion_mm[1] - MISSING_BOX_HALF_MM,
                    ],
                    [
                        symbol.insertion_mm[0] + MISSING_BOX_HALF_MM,
                        symbol.insertion_mm[1] + MISSING_BOX_HALF_MM,
                    ],
                ]
            });
            point_mm[0] >= min[0] - slack_mm
                && point_mm[0] <= max[0] + slack_mm
                && point_mm[1] >= min[1] - slack_mm
                && point_mm[1] <= max[1] + slack_mm
        })
    }

    /// The page the PDF will be given.
    ///
    /// A drawing that states its sheet — a decoded `igSmartFrame2d` whose
    /// extent is the page, or a template name — has that sheet here, exactly as
    /// [`PaperModel::page_mm`] reports it. One that states none is measured
    /// instead: the smallest [`STANDARD_SHEETS_MM`] sheet its ink fits, and only
    /// when it fits none of them its ink's own box. Four drawings across the
    /// plant backup and the five-drawing corpus take that path (their `.pid`
    /// carries neither a frame nor a template name); the rule is stated once,
    /// here, so the writer and every gate agree on the same number.
    ///
    /// `None` means the drawing states no page **and** has no ink to measure.
    #[must_use]
    pub fn resolved_page_mm(&self) -> Option<[f64; 2]> {
        if let Some(page) = self.page_mm {
            return Some(page);
        }
        let mut max_x = 0.0_f64;
        let mut max_y = 0.0_f64;
        let mut any = false;
        for primitive in &self.primitives {
            for point in extent_points(&primitive.shape) {
                if !(point[0].is_finite() && point[1].is_finite()) {
                    continue;
                }
                any = true;
                max_x = max_x.max(point[0]);
                max_y = max_y.max(point[1]);
            }
        }
        if !any {
            return None;
        }
        for sheet in STANDARD_SHEETS_MM {
            for page in [sheet, [sheet[1], sheet[0]]] {
                if max_x <= page[0] + SHEET_FIT_SLACK_MM && max_y <= page[1] + SHEET_FIT_SLACK_MM {
                    return Some(page);
                }
            }
        }
        Some([max_x.ceil().max(1.0), max_y.ceil().max(1.0)])
    }
}

/// Every point a primitive defines that can set the page's extent.
///
/// A circle or arc contributes its bounding box, not its centre: the ink runs
/// to `center ± radius`. Text contributes its insertion point, which is the
/// only position the shape carries.
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
        PaperShape::Fill { chain_mm, .. } => chain_mm.clone(),
        PaperShape::Text { origin_mm, .. } => vec![*origin_mm],
    }
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
    /// Whether this stroke is sheet furniture from the border template.
    ///
    /// The writer drops a stroke that runs along a page edge, because on a
    /// drawing that stroke is the print driver's frame rather than content.
    /// A template's outer frame *is* such a stroke, and it is the furniture
    /// being drawn, so furniture is exempt. Reading the page back still drops
    /// it (`read::sheet_edge_mm`), which keeps the comparison symmetric.
    #[serde(default, skip_serializing_if = "is_false")]
    pub furniture: bool,
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
    /// Circular arc, from `start_angle` to `end_angle` in the direction
    /// `clockwise` states.
    ///
    /// A source record states two endpoints-angles and a direction; the two
    /// readings are complements, so the direction decides whether the arc
    /// takes the short way or the rest of the circle.
    Arc {
        /// Centre.
        center_mm: [f64; 2],
        /// Radius.
        radius_mm: f64,
        /// Start angle in radians, counter-clockwise from +X.
        start_angle: f64,
        /// End angle in radians, counter-clockwise from +X.
        end_angle: f64,
        /// `true` when the arc runs clockwise on the page from
        /// `start_angle` to `end_angle`; `false` for counter-clockwise.
        clockwise: bool,
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
/// One line of a label, and where it came from in the record's text.
///
/// `text` holds the record's characters `start .. start + (text chars)` when
/// `exact` — which is what a formatting run's character offsets need to land
/// on the line they belong to. A dropped control character (no typeface has a
/// glyph for one) breaks that correspondence, so such a line says so instead
/// of pretending its offsets still line up.
struct LabelLine {
    text: String,
    start: usize,
    exact: bool,
}

/// Split a label into the lines it letters, each with its own place in the
/// record's text.
fn label_line_ranges(value: &str) -> Vec<LabelLine> {
    let mut lines: Vec<LabelLine> = Vec::new();
    let mut current = String::new();
    let mut start = 0usize;
    let mut dropped = false;
    let mut chars = value.chars().enumerate().peekable();
    while let Some((index, ch)) = chars.next() {
        let mut consumed_next = false;
        let ends = match ch {
            '\r' => {
                if chars.peek().is_some_and(|(_, next)| *next == '\n') {
                    chars.next();
                    consumed_next = true;
                }
                true
            }
            '\n' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}' => true,
            // A tab is the one control character with a visible meaning: it
            // separates words, so it becomes the space it stands for rather
            // than vanishing and gluing two words together.
            '\t' => {
                current.push(' ');
                false
            }
            _ if ch.is_control() => {
                dropped = true;
                false
            }
            _ => {
                current.push(ch);
                false
            }
        };
        if ends {
            lines.push(LabelLine {
                text: std::mem::take(&mut current),
                start,
                exact: !dropped,
            });
            // The break itself is not part of either line, so the next one
            // starts after it — `\r\n` counts as one break, but both its
            // characters have to be stepped over.
            start = index + 1 + usize::from(consumed_next);
            dropped = false;
        }
    }
    lines.push(LabelLine {
        text: current,
        start,
        exact: !dropped,
    });
    // A trailing break names a line with nothing on it; it inks nothing and
    // would only add a run the writer then drops.
    while lines.len() > 1 && lines.last().is_some_and(|line| line.text.is_empty()) {
        lines.pop();
    }
    lines
}
/// A sheet record's run partition, in the record's own character offsets.
///
/// `None` where the record resolves no style at all, and empty for the labels
/// that letter in one style — a single run only restates what the shape's own
/// `height_mm` and `font` already say. A label holding a non-BMP character
/// keeps its partition off: a run length counts UTF-16 units while a span
/// offset counts `char`s, so the second run would land mid-glyph. The library
/// applies the same rule at `symbol_library::guarded_spans`.
fn sheet_text_spans(style: Option<&ResolvedTextStyle>, value: &str) -> Vec<PaperTextSpan> {
    let Some(style) = style else {
        return Vec::new();
    };
    if value.chars().count() != value.encode_utf16().count() {
        return Vec::new();
    }
    style
        .spans
        .iter()
        .map(|span| PaperTextSpan {
            start: span.start,
            len: span.len,
            height_mm: Some(span.style.height_mm()),
            font: span.style.font_name.clone(),
        })
        .collect()
}

/// The part of a record's partition that letters one of its lines, re-based to
/// that line's own first character.
///
/// A line the writer split off keeps its characters verbatim, so a span that
/// covers characters on both sides of a break becomes one span per line. A
/// line that had a control character dropped no longer lines up with the
/// record's offsets and gets none.
fn line_spans(partition: &[PaperTextSpan], line: &LabelLine) -> Vec<PaperTextSpan> {
    if partition.is_empty() || !line.exact {
        return Vec::new();
    }
    let length = line.text.chars().count();
    let mut out = Vec::new();
    for span in partition {
        let start = usize::from(span.start);
        let end = start.saturating_add(usize::from(span.len));
        let from = start.max(line.start);
        let to = end.min(line.start.saturating_add(length));
        if to <= from {
            continue;
        }
        let (Ok(start), Ok(len)) = (u16::try_from(from - line.start), u16::try_from(to - from))
        else {
            return Vec::new();
        };
        out.push(PaperTextSpan {
            start,
            len,
            height_mm: span.height_mm,
            font: span.font.clone(),
        });
    }
    out
}

/// Whether `entity` sits on a layer `SmartPlant` leaves off a printed page:
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

/// Lay out one `.pid`, furniture first.
///
/// The symbol library is the first of: `PID_SYMBOL_LIBRARY`, a directory
/// under `<corpus>/Ref` or `<corpus>/../Ref` whose name contains `symbol`,
/// then `test-file/symbols-full` — and, after every directory, the
/// `RefData~*.zip` a backup package keeps beside them, which carries the same
/// trees without being unpacked.
///
/// The sheet's border template comes from `<corpus>/Ref/Template Files` and
/// `<corpus>/Ref` (or the corpus's parent), or from a `RefData~*.zip` in the
/// corpus or beside it: the `.igr` whose own extent is the sheet — and, when
/// two of them are the same size, the one the sheet's link record names —
/// which is how a linked OLE frame record is resolved to a file
/// ([`load_border_template`]). Its furniture goes onto the page before the
/// drawing's own records, so content inks over the frame rather than under it.
pub fn lay_out_drawing(pid: &Path, corpus: &Path) -> Result<PaperModel, PaperModelError> {
    let (roots, archives, library) = symbol_library(corpus);
    lay_out_with_symbol_roots(pid, corpus, roots, archives, &library)
}

/// [`lay_out_drawing`], with where the symbol library lives stated instead of
/// discovered.
///
/// The roots — and the `RefData~*.zip` archives searched after them — are
/// normally found from the corpus (see `symbol_library`); a caller that knows
/// them — a probe comparing two copies, or a test that must not depend on what
/// happens to be installed — says so here. `library` is the label the page
/// reports for those sources.
pub fn lay_out_with_symbol_roots(
    pid: &Path,
    corpus: &Path,
    roots: Vec<PathBuf>,
    archives: Vec<PathBuf>,
    library: &[String],
) -> Result<PaperModel, PaperModelError> {
    let mut symbols = SymbolLibrary::with_roots(roots).with_archives(archives);
    let doc = PidParser::new()
        .parse_file(pid)
        .map_err(|error| PaperModelError {
            message: error.to_string(),
        })?;
    let styles = pid_parse::style_link::text_styles_for_document(&doc);
    let line_styles = pid_parse::style_link::line_styles_for_document(&doc);
    let fill_styles = pid_parse::style_link::fill_styles_for_document(&doc);
    let geometry = build_normalized_geometry(&doc);
    let page_mm = geometry
        .page_dimensions_mm
        .map(|(width, height)| [width, height]);
    let mut primitives = Vec::new();
    let mut placements = SymbolPlacements::default();
    let mut seen_sheet_fills = std::collections::HashSet::new();

    // The sheet's furniture is not in the `.pid`: the border is a linked OLE
    // object whose content is the `.igr` template. It is parsed as the drawing
    // it is, with its own styles, and laid out at 1:1 because a template is
    // authored in page millimetres from the page's own origin — the corpus A1
    // template runs `(0, 0)` to `(841, 594)`, exactly the sheet B states.
    let mut template: Option<BorderTemplate> = None;
    // What the sheet's own frames point at, kept for the pick: a record's path
    // does not resolve here, but its file name still says which template the
    // sheet was drawn on when two of them are the same size.
    let linked_templates: Vec<String> = doc
        .jsites
        .iter()
        .flat_map(|site| site.ole_links.iter().cloned())
        .collect();
    if let Some(page) = page_mm
        && let Some(furniture) = load_border_template(corpus, page, &linked_templates)
    {
        let template_styles = pid_parse::style_link::text_styles_for_document(&furniture.doc);
        let template_lines = pid_parse::style_link::line_styles_for_document(&furniture.doc);
        let template_fills = pid_parse::style_link::fill_styles_for_document(&furniture.doc);
        let before = primitives.len();
        // One tally for the whole template, kept rather than dropped: which of
        // its placements had to be drawn from the cache, and which found
        // nothing at all, is the account this page carries.
        let mut furniture_placements = SymbolPlacements::default();
        for entity in &furniture.geometry.entities {
            // A template is a drawing, so a symbol it places draws like any
            // other: the two `CPE-SQ` templates keep the `比例` value cell's
            // 5 mm wave in a placement whose body is the `/JSite16` cache.
            // A tally of its own keeps the template's own `.sym` misses out
            // of the drawing's report — and out of its missing-symbol boxes.
            if let PidGraphicKind::SymbolInstance { .. } = entity.kind {
                push_symbol_instance(
                    &mut primitives,
                    entity,
                    &furniture.geometry,
                    &mut symbols,
                    &template_lines,
                    Some(page),
                    &mut furniture_placements,
                );
                continue;
            }
            push_furniture_entity(
                &mut primitives,
                entity,
                Some(page),
                &template_styles,
                &template_lines,
                &template_fills,
                &mut seen_sheet_fills,
            );
        }
        // Everything the template contributed is furniture, and only that:
        // the flag exempts it from the writer's page-edge rule, which would
        // otherwise drop the frame this draw-in exists to put on the page.
        for primitive in &mut primitives[before..] {
            primitive.furniture = true;
        }
        template = Some(BorderTemplate {
            path: furniture.path.display().to_string(),
            extent_mm: furniture.extent_mm,
            primitives: primitives.len() - before,
            symbols: furniture_placements.placed,
        });
    }

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
            PidGraphicKind::SymbolInstance { .. } => push_symbol_instance(
                &mut primitives,
                entity,
                &geometry,
                &mut symbols,
                &line_styles,
                page_mm,
                &mut placements,
            ),
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
        library: library.to_vec(),
        page_mm,
        template,
        primitives,
        off_sheet_primitives: off_sheet,
        skipped_endpoint_pairs,
        missing_symbols: placements.missing.into_iter().collect(),
        missing_boxes: placements.missing_boxes,
        symbols_found: placements.found.len(),
        symbols_referenced: placements.referenced.len(),
    })
}

/// What a page learns from the placements it expanded: which symbol files it
/// named, which resolved, which did not, and where a square marks a miss.
#[derive(Default)]
struct SymbolPlacements {
    referenced: std::collections::BTreeSet<String>,
    found: std::collections::BTreeSet<String>,
    missing: std::collections::BTreeSet<String>,
    missing_boxes: Vec<MissingSymbolBox>,
    /// One entry per placement expanded, in page order: what drew it and where
    /// its strokes reached. Only a template's are read back
    /// ([`BorderTemplate::symbols`]).
    placed: Vec<TemplateSymbol>,
}

/// Expand one `SymbolInstance` into the strokes of its body.
///
/// Two sources, in order: the body embedded in the file — the placement's own
/// `(definition JSheet, LdcSite)` pair, resolved against the drawing's
/// definition caches, which is how a symbol survives its `.sym` being absent
/// (the templates' `C:\…\Temp\BLFH.sym` is long gone) — then the `.sym` the
/// placement names, from the symbol library. A placement that finds neither
/// leaves a box where it stood rather than disappearing.
fn push_symbol_instance(
    out: &mut Vec<PaperPrimitive>,
    entity: &PidGraphicEntity,
    geometry: &NormalizedPidGeometry,
    symbols: &mut SymbolLibrary,
    line_styles: &LineStyleIndex,
    page_mm: Option<[f64; 2]>,
    tally: &mut SymbolPlacements,
) {
    let PidGraphicKind::SymbolInstance {
        insertion,
        symbol_path,
        rotation,
        scale,
        definition,
    } = &entity.kind
    else {
        return;
    };
    let key = symbol_path.as_deref().map(symbol_key);
    if let Some(key) = key.clone() {
        tally.referenced.insert(key);
    }
    let body = symbol_path
        .as_ref()
        .and_then(|path| symbols.resolve(path).cloned());
    let name_gap = symbol_path
        .as_deref()
        .and_then(|path| symbols.name_miss(path));
    if symbol_path.is_some()
        && body.is_some()
        && let Some(key) = key.clone()
    {
        tally.found.insert(key);
    }
    let placement = Placement {
        insertion: [insertion.x, insertion.y],
        rotation: *rotation,
        scale: *scale,
        symbol: symbol_path.clone(),
    };
    let placement_style = entity_style(entity, line_styles);
    let library_available = body.is_some();
    let before = out.len();
    let source = if let Some(definition) =
        definition.and_then(|reference| geometry.symbol_definition(reference))
    {
        if symbol_path.is_some()
            && body.is_none()
            && let Some(key) = key.clone()
        {
            tally.missing.insert(key);
        }
        for (primitive, body_style, lettering) in definition.visible_styled() {
            push_symbol_primitive(
                out,
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
        SymbolBodySource::Cached
    } else if let Some(body) = body {
        for styled in body.displayed_primitives() {
            push_symbol_primitive(
                out,
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
        SymbolBodySource::Library
    } else {
        if let Some(key) = key.clone() {
            tally
                .missing_boxes
                .push(missing_box(&key, [insertion.x, insertion.y]));
            tally.missing.insert(key);
        }
        SymbolBodySource::Missing
    };
    if let Some(key) = key {
        tally.placed.push(TemplateSymbol {
            key,
            insertion_mm: [insertion.x * MM_PER_METRE, insertion.y * MM_PER_METRE],
            body: source,
            library_available,
            extent_mm: extent_of(&out[before..]),
            name_gap,
        });
    }
}

/// Ink extent of a run of page primitives, paper millimetres.
///
/// `None` when nothing in the run reaches a finite point — a placement that
/// drew nothing has no extent, and [`PaperModel::template_symbol_at`] falls
/// back to the box it left.
fn extent_of(primitives: &[PaperPrimitive]) -> Option<[[f64; 2]; 2]> {
    let mut min = [f64::MAX; 2];
    let mut max = [f64::MIN; 2];
    let mut seen = false;
    for primitive in primitives {
        for point in shape_points(&primitive.shape) {
            if !finite(point) {
                continue;
            }
            min = [min[0].min(point[0]), min[1].min(point[1])];
            max = [max[0].max(point[0]), max[1].max(point[1])];
            seen = true;
        }
    }
    seen.then_some([min, max])
}

/// The points a shape's ink reaches.
///
/// An arc contributes its ends and every quadrant its sweep crosses, so the box
/// is the ink's and not the whole circle's: a 5° arc is not as big as its
/// radius.
fn shape_points(shape: &PaperShape) -> Vec<[f64; 2]> {
    match shape {
        PaperShape::Line { start_mm, end_mm } => vec![*start_mm, *end_mm],
        PaperShape::Arc {
            center_mm,
            radius_mm,
            start_angle,
            end_angle,
            clockwise,
        } => {
            use std::f64::consts::{FRAC_PI_2, TAU};
            let sweep = if *clockwise {
                -(*start_angle - *end_angle).rem_euclid(TAU)
            } else {
                (*end_angle - *start_angle).rem_euclid(TAU)
            };
            let mut angles = vec![*start_angle, *end_angle];
            for quarter in 0..4 {
                let angle = quarter as f64 * FRAC_PI_2;
                let travelled = if *clockwise {
                    (*start_angle - angle).rem_euclid(TAU)
                } else {
                    (angle - *start_angle).rem_euclid(TAU)
                };
                if travelled <= sweep.abs() + 1e-12 {
                    angles.push(angle);
                }
            }
            angles
                .into_iter()
                .map(|angle| {
                    [
                        center_mm[0] + radius_mm * angle.cos(),
                        center_mm[1] + radius_mm * angle.sin(),
                    ]
                })
                .collect()
        }
        PaperShape::Circle {
            center_mm,
            radius_mm,
        } => vec![
            [center_mm[0] - radius_mm, center_mm[1] - radius_mm],
            [center_mm[0] + radius_mm, center_mm[1] + radius_mm],
        ],
        PaperShape::Polyline { points_mm, .. } => points_mm.clone(),
        PaperShape::Fill { chain_mm, .. } => chain_mm.clone(),
        PaperShape::Text { origin_mm, .. } => vec![*origin_mm],
    }
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
    // A label's origin may sit past the edge and still letter onto the page —
    // the anchor decides where the run lands — so only its finiteness is read
    // here. Every other shape is judged by the ink it reaches.
    if let PaperShape::Text { origin_mm, .. } = shape {
        return !finite(*origin_mm);
    }
    let points = shape_points(shape);
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

/// The symbol-library sources searched, in order: the directory roots, the
/// `RefData~*.zip` archives after them, and the label naming both.
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
///
/// A backup package keeps its library inside `RefData~*.zip` rather than
/// exploded on a share, so the archives in the corpus and beside it are
/// returned as further sources (see [`SymbolLibrary::with_archives`]).
/// Directories are still searched first, so unpacking a backup stays optional
/// and cannot change a page.
fn symbol_library(corpus: &Path) -> (Vec<PathBuf>, Vec<PathBuf>, Vec<String>) {
    if let Some(list) = std::env::var_os("PID_SYMBOL_LIBRARY") {
        // A listed path is a root, unless it is an archive — the override
        // replaces the search outright, and a backup's library is a file.
        let listed: Vec<PathBuf> = std::env::split_paths(&list).collect();
        let roots: Vec<PathBuf> = listed
            .iter()
            .filter(|path| path.is_dir())
            .cloned()
            .collect();
        let archives: Vec<PathBuf> = listed.into_iter().filter(|path| path.is_file()).collect();
        if !roots.is_empty() || !archives.is_empty() {
            let label = roots
                .iter()
                .chain(archives.iter())
                .map(|path| path.display().to_string())
                .collect();
            return (roots, archives, label);
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
    let archives = refdata_archive_files(corpus);
    if roots.is_empty() && archives.is_empty() {
        // Nothing on disk or in an archive to search: the bundled path is
        // still the name to report, and the reader will refuse every symbol
        // by name.
        let label = vec![bundled.display().to_string()];
        return (roots, archives, label);
    }
    let label = roots
        .iter()
        .chain(archives.iter())
        .map(|path| path.display().to_string())
        .collect();
    (roots, archives, label)
}

/// How far a template's own ink may sit from the sheet it borders, in
/// millimetres.
///
/// A sheet states its size in metres and carries float noise (`841.00005`),
/// while the template's outer frame runs along exactly the same edge. Two
/// millimetres is far outside that noise and 348 mm inside the gap between one
/// ISO size and the next, so a near miss can only mean the same sheet.
const TEMPLATE_EXTENT_TOLERANCE_MM: f64 = 2.0;

/// How close two extents must be before their difference is the same reading.
///
/// A sheet states its size in `f64` metres and carries noise of its own
/// (`841.0000500894253` for an A1), and two templates authored for one size
/// differ only in the last bits of theirs (`841.0000000000001` against
/// `841.0000000003785`). Compared exactly, whichever noise happens to lean the
/// sheet's way wins — at 4e-10 mm, a difference nothing measured. A micrometre
/// is 150× finer than the closest two templates of *different* sizes come
/// (841.00 against 841.15), so quantizing the error here costs no distinction
/// and stops the noise from outvoting the name.
const TEMPLATE_EXTENT_QUANTUM_MM: f64 = 1e-3;

/// Directories that can hold `.igr` border templates, in search order.
///
/// A project keeps them in `<corpus>/Ref/Template Files` — both the five-drawing
/// corpus and `test-file/Ref` do — so that directory is the first place to look.
/// `Ref` itself and the corpus's parent follow, which is how a fixture reaches
/// the same shared `Ref` the symbol library already searches.
fn border_template_dirs(corpus: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for candidate in [
        Some(corpus.join("Ref").join("Template Files")),
        Some(corpus.join("Ref")),
        corpus
            .parent()
            .map(|parent| parent.join("Ref").join("Template Files")),
        corpus.parent().map(|parent| parent.join("Ref")),
    ]
    .into_iter()
    .flatten()
    {
        if candidate.is_dir() && !dirs.iter().any(|seen| seen == &candidate) {
            dirs.push(candidate);
        }
    }
    dirs
}

/// One file a sheet's border could have come from.
///
/// A corpus carries templates either exploded under `Ref/Template Files` or
/// inside the `RefData~*.zip` a backup package keeps at its own root, and the
/// pick has to read both the same way. A member is named by
/// [`ArchiveFile::label`], which is the archive path with the member's own
/// name on the end, so everything downstream — the extent error, the link-name
/// prefix, the reported path — reads it as the file it is.
enum TemplateSource {
    /// A `.igr` under one of the corpus's template directories.
    File(PathBuf),
    /// A `.igr` member of a `RefData~*.zip` in the corpus or beside it.
    Member(ArchiveFile),
}

/// Every `.igr` a sheet's furniture could come from: the corpus's template
/// directories first, then the `RefData~*.zip` archives in the corpus and its
/// parent.
///
/// Directories come first so an extracted tree still decides, which keeps
/// unpacking a backup optional and stops it from changing any page: the two
/// sources hold the same files, and the pick's remaining keys are equal for
/// them.
fn border_template_sources(corpus: &Path) -> Vec<TemplateSource> {
    let mut sources: Vec<TemplateSource> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();
    for dir in border_template_dirs(corpus) {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut found: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("igr"))
            })
            .collect();
        found.sort();
        for path in found {
            if !seen.contains(&path) {
                seen.push(path.clone());
                sources.push(TemplateSource::File(path));
            }
        }
    }
    for archive in refdata_archive_files(corpus) {
        let Ok(members) = archive_members(&archive, "igr") else {
            continue;
        };
        sources.extend(members.into_iter().map(TemplateSource::Member));
    }
    sources
}

/// The `RefData~*.zip` archives a corpus keeps its reference data in, in
/// search order: the corpus's own directory, then its parent's.
///
/// A backup package holds them beside `Manifest.txt`, so a drawing laid out
/// with the package directory as its corpus finds them, and one laid out with
/// a subdirectory of the package (a plant's extracted drawing folder) finds
/// them one level up. A project corpus has none, which is the usual case and
/// why an empty list is not a problem.
fn refdata_archive_files(corpus: &Path) -> Vec<PathBuf> {
    let mut archives: Vec<PathBuf> = Vec::new();
    for dir in [Some(corpus), corpus.parent()].into_iter().flatten() {
        for archive in refdata_archives(dir) {
            if !archives.contains(&archive) {
                archives.push(archive);
            }
        }
    }
    archives
}

/// The extent of a document's stroked geometry, paper millimetres.
///
/// Only the shapes a border is drawn with count: a template's frame is its own
/// outermost ink, and a label that hangs a millimetre past it must not move the
/// sheet it is being matched against.
fn ink_extent_mm(geometry: &NormalizedPidGeometry) -> Option<[f64; 2]> {
    let mut min = [f64::MAX; 2];
    let mut max = [f64::MIN; 2];
    let mut seen = false;
    let mut take = |point: [f64; 2]| {
        if point[0].is_finite() && point[1].is_finite() {
            min[0] = min[0].min(point[0]);
            min[1] = min[1].min(point[1]);
            max[0] = max[0].max(point[0]);
            max[1] = max[1].max(point[1]);
            seen = true;
        }
    };
    for entity in &geometry.entities {
        match &entity.kind {
            PidGraphicKind::Line { start, end } => {
                take([start.x, start.y]);
                take([end.x, end.y]);
            }
            PidGraphicKind::Polyline { points, .. } => {
                for point in points {
                    take([point.x, point.y]);
                }
            }
            PidGraphicKind::Arc { center, radius, .. }
            | PidGraphicKind::Circle { center, radius } => {
                take([center.x - radius, center.y - radius]);
                take([center.x + radius, center.y + radius]);
            }
            PidGraphicKind::Point { .. }
            | PidGraphicKind::Text { .. }
            | PidGraphicKind::SymbolInstance { .. }
            | PidGraphicKind::Annotation { .. }
            | PidGraphicKind::Unknown { .. } => {}
        }
    }
    seen.then_some([
        (max[0] - min[0]) * MM_PER_METRE,
        (max[1] - min[1]) * MM_PER_METRE,
    ])
}

/// One parsed `.igr` template.
struct BorderTemplateDocument {
    path: PathBuf,
    extent_mm: [f64; 2],
    doc: PidDocument,
    geometry: NormalizedPidGeometry,
}

/// The file-name stem a sheet's link record spells, as one a template's own
/// file name can begin with.
///
/// The record stores an authoring-machine path, usually through Windows' 8.3
/// short name — `C:\PID_SQ~1\SQPlant\Ref\TEMPLA~1\CPE-SQ~2.IGR` names the file
/// this machine has as `CPE-SQ-A1.igr`. Only the last component is the file
/// name (the directories carry `~` of their own); cutting it at the `~` leaves
/// the six characters Windows kept (`CPE-SQ`). A record authored on a share that
/// kept long names spells the whole stem (`\CPECCHB-SQ-A2.igr`). Either way the
/// stem is a prefix of the file it names — `CPE-SQ` is not a prefix of
/// `CPECCHB-SQ-A1` — which is what lets it decide what an extent cannot.
fn linked_template_prefix(link: &str) -> Option<String> {
    let file_name = link.rsplit(['\\', '/']).next().unwrap_or(link);
    let (stem, extension) = file_name.rsplit_once('.')?;
    if !extension.eq_ignore_ascii_case("igr") {
        return None;
    }
    let prefix = stem.split('~').next().unwrap_or(stem);
    (!prefix.is_empty()).then(|| prefix.to_ascii_uppercase())
}

/// Whether `path`'s own name is the one a link record's stem spells.
fn template_named_by(path: &Path, prefixes: &[String]) -> bool {
    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    let stem = stem.to_ascii_uppercase();
    prefixes
        .iter()
        .any(|prefix| stem.starts_with(prefix.as_str()))
}

/// How one eligible template ranks against another, smallest first: the ones a
/// sheet names, then the closer extent, then the name it has on disk.
///
/// The last key is reached only by templates the sheet names none of and whose
/// extents agree to [`TEMPLATE_EXTENT_QUANTUM_MM`], where the choice is
/// evidence-free and only has to be the same one twice.
fn template_rank(path: &Path, error: f64, prefixes: &[String]) -> (bool, f64, String) {
    (
        !template_named_by(path, prefixes),
        (error / TEMPLATE_EXTENT_QUANTUM_MM).round(),
        path.to_string_lossy().to_ascii_lowercase(),
    )
}

/// The template authored for `page_mm`, if the corpus ships one.
///
/// Every `.igr` under the corpus's template directories is parsed and its
/// stroked extent measured; the file whose extent is the sheet is the one the
/// sheet was drawn on. That equality is the primary evidence, because it is the
/// one thing the sheet's geometry and the template state independently.
///
/// It is not the only evidence, and on its own it is not always enough. A
/// project can hold two templates of the same size — `CPE-SQ-A1.igr` and
/// `CPECCHB-SQ-A1.igr` are both authored for A1 — and then the two extents
/// differ only in the last bits of their `f64` metres (`841.0000000000001`
/// against `841.0000000003785`), so which one is closer to the sheet's own
/// noise (a page states `841.0000500894253`) is a 4e-10 mm accident, not a
/// reading. The link record breaks that tie: its stem is a prefix of the name
/// it points at, so a template the sheet names wins over a stranger of the same
/// size. A page that names nothing still falls back to the extent, and a page
/// with no template within [`TEMPLATE_EXTENT_TOLERANCE_MM`] gets no furniture
/// rather than a scaled-up guess from another size.
fn load_border_template(
    corpus: &Path,
    page_mm: [f64; 2],
    links: &[String],
) -> Option<BorderTemplateDocument> {
    let prefixes: Vec<String> = links
        .iter()
        .filter_map(|link| linked_template_prefix(link))
        .collect();
    let mut best: Option<((bool, f64, String), BorderTemplateDocument)> = None;
    for source in border_template_sources(corpus) {
        let (path, doc) = match source {
            TemplateSource::File(path) => {
                let Ok(doc) = PidParser::new().parse_file(&path) else {
                    continue;
                };
                (path, doc)
            }
            TemplateSource::Member(file) => {
                let Ok(bytes) = read_archive_file(&file) else {
                    continue;
                };
                let Ok(doc) = PidParser::new().parse_reader(std::io::Cursor::new(bytes)) else {
                    continue;
                };
                (file.label(), doc)
            }
        };
        let geometry = build_normalized_geometry(&doc);
        let Some(extent_mm) = ink_extent_mm(&geometry) else {
            continue;
        };
        let error = (extent_mm[0] - page_mm[0])
            .abs()
            .max((extent_mm[1] - page_mm[1]).abs());
        if error > TEMPLATE_EXTENT_TOLERANCE_MM {
            continue;
        }
        let document = BorderTemplateDocument {
            path,
            extent_mm,
            doc,
            geometry,
        };
        let rank = template_rank(&document.path, error, &prefixes);
        if best
            .as_ref()
            .is_some_and(|(best_rank, _)| *best_rank <= rank)
        {
            continue;
        }
        best = Some((rank, document));
    }
    best.map(|(_, template)| template)
}

fn missing_box(path: &str, insertion_m: [f64; 2]) -> MissingSymbolBox {
    let origin = [insertion_m[0] * MM_PER_METRE, insertion_m[1] * MM_PER_METRE];
    MissingSymbolBox {
        path: path.to_string(),
        min_mm: [
            origin[0] - MISSING_BOX_HALF_MM,
            origin[1] - MISSING_BOX_HALF_MM,
        ],
        max_mm: [
            origin[0] + MISSING_BOX_HALF_MM,
            origin[1] + MISSING_BOX_HALF_MM,
        ],
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
///
/// The one exception is a placement width no pen could carry, which is a
/// validation mark rather than a pen: `Q井场`'s globe valve is placed under
/// `BandAid`, the 10 mm band `SmartPlant` paints over an object an engineer has
/// flagged, and it sits in the style table beside `lsOk` and `lsWarning`. That
/// width repainted the valve's 0.13 mm outline with the page's thickest pen — a
/// black blob where every other valve on the sheet is a hairline. The body's
/// own width stands instead; the colour still comes from the placement, which
/// the plot's monochrome table inks black anyway.
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
        width_mm: placement_style
            .filter(|style| carries_a_pen(style.symbology.width_mm()))
            .map_or_else(
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

/// A four-corner axis-aligned ring, stored in the order a written page reads
/// it back.
///
/// A rectangle is the same ring from any of its corners, and the writer hands
/// one to the PDF as `m`/`l`/`h`, which the reader recognizes as a `Rect` and
/// gives back from its own corner (`+x` first, then `+y`). Storing the ring in
/// that order is the only thing this changes — the region is identical — and
/// it is what lets `write_five_drawings` compare a page endpoint for endpoint
/// instead of set by set. A chain that is not a four-corner axis-aligned box
/// is returned untouched.
fn canonical_rect_chain(chain_mm: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    const EPSILON: f64 = 1e-6;
    if chain_mm.len() != 4 || !chain_mm.iter().all(|point| finite(*point)) {
        return chain_mm;
    }
    let closed = |first: [f64; 2], second: [f64; 2]| {
        (first[0] - second[0]).abs() <= EPSILON || (first[1] - second[1]).abs() <= EPSILON
    };
    for index in 0..4 {
        let next = (index + 1) % 4;
        if !closed(chain_mm[index], chain_mm[next]) {
            return chain_mm;
        }
    }
    let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
    for point in &chain_mm {
        min_x = min_x.min(point[0]);
        max_x = max_x.max(point[0]);
        min_y = min_y.min(point[1]);
        max_y = max_y.max(point[1]);
    }
    vec![
        [min_x, min_y],
        [max_x, min_y],
        [max_x, max_y],
        [min_x, max_y],
    ]
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

/// Push one record of the drawing itself.
fn push_entity(
    out: &mut Vec<PaperPrimitive>,
    entity: &PidGraphicEntity,
    page_mm: Option<[f64; 2]>,
    styles: &TextStyleIndex,
    line_styles: &LineStyleIndex,
    fill_styles: &pid_parse::style_link::FillIndex,
    seen_sheet_fills: &mut std::collections::HashSet<u64>,
) {
    push_entity_with(
        out,
        entity,
        page_mm,
        styles,
        line_styles,
        fill_styles,
        seen_sheet_fills,
        false,
    );
}

/// Push one record of the sheet's border template.
///
/// The one rule that changes is `sheet_edge`: a stroke that runs along the page
/// edge is dropped from a drawing because it is the print driver's frame rather
/// than content, while in a template that stroke *is* the frame. Reading the
/// written page back still drops both frames (`read::sheet_edge_mm`), so a
/// page with furniture compares against a reference plot that has it exactly as
/// one without does against a reference that has none.
fn push_furniture_entity(
    out: &mut Vec<PaperPrimitive>,
    entity: &PidGraphicEntity,
    page_mm: Option<[f64; 2]>,
    styles: &TextStyleIndex,
    line_styles: &LineStyleIndex,
    fill_styles: &pid_parse::style_link::FillIndex,
    seen_sheet_fills: &mut std::collections::HashSet<u64>,
) {
    push_entity_with(
        out,
        entity,
        page_mm,
        styles,
        line_styles,
        fill_styles,
        seen_sheet_fills,
        true,
    );
}

/// The shared body of [`push_entity`] and [`push_furniture_entity`], with
/// `keep_sheet_edge` saying whether a page-edge stroke is content.
#[allow(clippy::too_many_arguments)]
fn push_entity_with(
    out: &mut Vec<PaperPrimitive>,
    entity: &PidGraphicEntity,
    page_mm: Option<[f64; 2]>,
    styles: &TextStyleIndex,
    line_styles: &LineStyleIndex,
    fill_styles: &pid_parse::style_link::FillIndex,
    seen_sheet_fills: &mut std::collections::HashSet<u64>,
    keep_sheet_edge: bool,
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
            if keep_sheet_edge || !sheet_edge(&shape, page_mm) {
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
                let chain_mm = canonical_rect_chain(points_mm);
                if seen_sheet_fills.insert(sheet_fill_key(&chain_mm, rgb)) {
                    out.push(primitive(
                        entity,
                        &placement,
                        None,
                        PaperShape::Fill { chain_mm, rgb },
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
            clockwise,
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
                    *clockwise,
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
            let resolved = entity.source.stream_path.as_ref().and_then(|stream| {
                entity
                    .graphic_oid
                    .and_then(|oid| styles.get(&(stream.clone(), oid)))
            });
            let styled = resolved.and_then(|style| style.effective());
            // The record's run partition, in the record's own character
            // offsets — the same one a symbol label draws span by span, read
            // off the same `resolve_text_style` the symbol library calls.
            let partitions = sheet_text_spans(resolved, value);
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
            for (index, line) in label_line_ranges(value).into_iter().enumerate() {
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
                        spans: line_spans(&partitions, &line),
                        value: line.text,
                        font: styled.as_ref().and_then(|style| style.font_name.clone()),
                        // A boxed record hangs from the box edge its
                        // paragraph's alignment names (above). An unboxed one
                        // keeps the record's own anchor code: `(code >> 4) % 3`
                        // is the horizontal hot point the insertion sits at.
                        anchor,
                        line: TextLine::from_anchor_code(*anchor_code),
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
/// once per fitted cell, in an undirected frame so a reversed pipe run keeps
/// the same tick orientation as a forward run.
///
/// Where the glyphs land is [`complex_dash_positions`]'s question: the advance
/// is divided over the length left once one glyph has been reserved, not over
/// the whole run, and a run that fits a single glyph keeps the pattern's own
/// advance instead.
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
    let glyph_span = complex_dash_glyph_span_mm(complex.strokes());
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
        for at in complex_dash_positions(length, period, glyph_span) {
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

/// How far the glyph reaches along the run it stamps, in millimetres: the
/// widest tangential extent of any stroke it is drawn from.
///
/// The glyph's local `x` is the run's tangent — [`stamp_complex_dash`] maps
/// `local.0` onto `frame_tangent` — so this is the largest `|end.x − start.x|`
/// over its non-degenerate strokes. The corpus's hydraulic mark is two parallel
/// slanted strokes, each 2.5 mm along the run and 2.6 mm across it, so the
/// glyph claims 2.5 mm — not the 3.6 mm of slash it draws.
fn complex_dash_glyph_span_mm(strokes: &[MarkerStroke]) -> f64 {
    strokes
        .iter()
        .filter(|mark| !mark.is_degenerate())
        .map(|mark| (mark.end.0 - mark.start.0).abs() * MM_PER_METRE)
        .fold(0.0_f64, f64::max)
}

/// How many copies of the glyph fit along a run of `length_mm`.
///
/// The advance is divided over the length that is left once one glyph has been
/// reserved, so the last cell's mark cannot hang over the end of the run:
/// `round((L − span) / period)`, never fewer than one. Reserving nothing
/// (`span = 0`) is the plain `round(L / period)`.
///
/// The rule is measured, not assumed: the two corpus runs whose `L / period`
/// sits just under `3.5` — 37.04 mm (`3.646`) and 36.50 mm (`3.593`) — each
/// carry three marks on the reference plot, where `round(L / period)` stamps a
/// fourth; the 28.13 mm run (`2.769`) carries three, where `floor(L / period)`
/// stamps two. All eight multi-mark runs agree with this rule and no other
/// candidate does (`docs/analysis/2026-10-24-the-45.8-mm-broken-down.md`).
fn complex_dash_cells(length_mm: f64, period_mm: f64, glyph_span_mm: f64) -> usize {
    if period_mm <= 1e-6 {
        return 1;
    }
    let fitted = (length_mm - glyph_span_mm).max(0.0) / period_mm;
    (fitted.round() as usize).max(1)
}

/// Where the glyphs land along a run of `length_mm`, measured from its start.
///
/// Two or more fitted cells divide the run between them and each glyph sits at
/// its cell's centre, `(k + 0.5) · L / cells`. A run that fits a **single**
/// glyph does not divide by one: it keeps the pattern's own advance, so the
/// glyph sits half a period from the run's start rather than at its end.
///
/// That half period is measured, not assumed. The corpus's 10.16 mm advance
/// stamps its one glyph on the 6.19 mm stubs at 5.08 mm and on the 6.54 mm
/// stubs at 5.02 mm — `period / 2` within 0.07 mm, where the run's own end is
/// 1.1–1.5 mm away, and the 13.2 mm the cells rule left over is the uncovered
/// far half of exactly those ticks (`docs/analysis/2026-10-24-
/// one-glyph-stubs-centre-a-half-period-in.md`).
///
/// A run shorter than half an advance carries **no** glyph at all. The centre
/// is not pulled back to the run's end: the reference inks only the rail on
/// those runs. Its nearest glyph on either side of them is 40.3–40.8 mm away,
/// and the walls of the three stubs carry rail pieces we draw to 0.06 mm —
/// the corpus's 4.23 mm vertical stub prints `0.169 + 4.233 mm` of rail and no
/// mark, its 1.32 and 3.81 mm stubs the same. The 3.83 mm run on the 7.62 mm
/// advance sits 0.02 mm over half that advance and does keep the mark the
/// reference inks there, so the boundary is the half advance, not the run list
/// (`docs/analysis/2026-10-26-the-hydraulic-glyph-comes-off-sub-half-period-stubs.md`).
fn complex_dash_positions(length_mm: f64, period_mm: f64, glyph_span_mm: f64) -> Vec<f64> {
    let cells = complex_dash_cells(length_mm, period_mm, glyph_span_mm);
    if cells <= 1 {
        let half = period_mm * 0.5;
        if length_mm < half {
            return Vec::new();
        }
        return vec![half];
    }
    let step = length_mm / cells as f64;
    (0..cells).map(|cell| (cell as f64 + 0.5) * step).collect()
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
            clockwise,
        } => arc_shape(
            [center.0, center.1],
            *radius,
            *start_angle,
            *end_angle,
            *clockwise,
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

/// Map one arc into paper space.
///
/// `clockwise` is the body's own direction, the record's trailing byte. The
/// body's two angles and that direction name one of the two arcs between
/// them; the placement moves the pair and, when it mirrors, reverses the
/// direction with it -- a clockwise body arc is counter-clockwise on the
/// page when the placement flips an axis.
fn arc_shape(
    center: [f64; 2],
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    clockwise: bool,
    placement: &Placement,
) -> PaperShape {
    let sweep = |clockwise: bool| {
        if clockwise {
            -(start_angle - end_angle).rem_euclid(std::f64::consts::TAU)
        } else {
            (end_angle - start_angle).rem_euclid(std::f64::consts::TAU)
        }
    };
    let sx = placement.scale[0].abs();
    let sy = placement.scale[1].abs();
    if (sx - sy).abs() > 1e-6 {
        // A non-uniform scale turns the circle into an ellipse, which no
        // arc record can state: sample the sweep the record asked for.
        let sweep = sweep(clockwise);
        let r = radius.abs();
        let steps =
            ((sweep.abs() * r * sx.max(sy) * MM_PER_METRE / 0.15).ceil() as usize).clamp(8, 64);
        let points_mm = (0..=steps)
            .map(|step| {
                let angle = start_angle + sweep * (step as f64 / steps as f64);
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
    PaperShape::Arc {
        center_mm: place_metres(
            center,
            placement.insertion,
            placement.rotation,
            placement.scale,
        ),
        radius_mm: radius.abs() * sx * MM_PER_METRE,
        start_angle: world_angle(start_angle, placement.rotation, placement.scale),
        end_angle: world_angle(end_angle, placement.rotation, placement.scale),
        clockwise: clockwise != mirrored,
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
        furniture: false,
    }
}

fn is_false(value: &bool) -> bool {
    !*value
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
    use super::{label_line_ranges, line_spans, sheet_text_spans};
    use pid_parse::style_link::{MarkerStroke, TextAlignment};
    use pid_parse::symbol_library::{NameMiss, PrimitiveStyle, SymbolLettering, SymbolPrimitive};
    use pid_parse::{
        PidCoordinateContext, PidGeometryConfidence, PidGraphicEntity, PidGraphicKind,
        PidGraphicProvenance, PidPoint, PidSourceLayer,
    };

    use super::{
        BorderTemplate, PaperModel, PaperPrimitive, PaperShape, PaperTextSpan, Placement,
        SymbolBodySource, SymbolItem, TemplateSymbol, TextAnchor, TextLine, arc_shape, body_stroke,
        canonical_rect_chain, complex_dash_cells, complex_dash_glyph_span_mm,
        complex_dash_positions, place_metres, push_entity, push_symbol_primitive, shape_points,
    };

    fn stroke(start: (f64, f64), end: (f64, f64)) -> MarkerStroke {
        MarkerStroke { start, end }
    }

    /// The corpus's hydraulic glyph, as `probe_complex_dash` reads it off
    /// `低温分离器A` / `换热器CD`: two parallel slanted strokes, each 2.5 mm
    /// along the run and 2.6 mm across it.
    fn hydraulic_glyph() -> [MarkerStroke; 2] {
        [
            stroke((-0.0019, -0.0013), (0.0006, 0.0013)),
            stroke((-0.0006, -0.0013), (0.0019, 0.0013)),
        ]
    }

    #[test]
    fn a_complex_dash_glyph_claims_only_what_it_reaches_along_the_run() {
        let glyph = hydraulic_glyph();
        assert!((complex_dash_glyph_span_mm(&glyph) - 2.5).abs() < 1e-9);
        // A stroke authored backwards reaches just as far.
        let mirrored = [stroke((0.0019, -0.0013), (-0.0006, 0.0013))];
        assert!((complex_dash_glyph_span_mm(&mirrored) - 2.5).abs() < 1e-9);
        // What it does not claim is its own length: 3.6 mm of 45° slash
        // projects to 2.5 mm of run, which is the room the layout reserves.
        assert!(glyph[0].length_mm() > 3.5);
        assert!(complex_dash_glyph_span_mm(&glyph) < glyph[0].length_mm());
    }

    #[test]
    fn a_stroke_with_no_length_claims_no_room() {
        assert_eq!(complex_dash_glyph_span_mm(&[]), 0.0);
        assert_eq!(
            complex_dash_glyph_span_mm(&[stroke((0.5, 0.5), (0.5, 0.5))]),
            0.0
        );
    }

    #[test]
    fn a_run_that_fits_three_glyphs_stamps_three_not_four() {
        // The two segments this rule was settled on: 37.04 mm (3.646 advances)
        // and 36.50 mm (3.593) each carry three marks on the reference plot,
        // where dividing the whole run rounds both up to a fourth.
        let span = complex_dash_glyph_span_mm(&hydraulic_glyph());
        for length in [37.04_f64, 36.50] {
            assert_eq!((length / 10.16).round() as usize, 4);
            assert_eq!(complex_dash_cells(length, 10.16, span), 3);
        }
        // The run just under three advances is why `floor` was the wrong
        // answer instead: it drops a mark the reference plot carries.
        assert_eq!((28.13_f64 / 10.16).floor().max(1.0) as usize, 2);
        assert_eq!(complex_dash_cells(28.13, 10.16, span), 3);
    }

    #[test]
    fn the_rest_of_the_multi_mark_runs_keep_their_count() {
        // 20.052 / 2.212 / 3.405 / 5.322 / 5.107 advances: the reference plot
        // inks the same count with and without the reserve.
        let span = complex_dash_glyph_span_mm(&hydraulic_glyph());
        for (length, cells) in [
            (203.73_f64, 20_usize),
            (22.47, 2),
            (34.60, 3),
            (54.07, 5),
            (51.88, 5),
        ] {
            assert_eq!(complex_dash_cells(length, 10.16, span), cells);
            assert_eq!((length / 10.16).round() as usize, cells);
        }
    }

    #[test]
    fn a_stub_shorter_than_one_advance_still_gets_its_single_mark() {
        // The 6.19 / 5.17 / 4.2 / 3.8 mm stubs are each under one advance, so
        // the rule reads zero fitted cells and one glyph is stamped.
        let span = complex_dash_glyph_span_mm(&hydraulic_glyph());
        for length in [6.19_f64, 5.17, 4.2, 3.8] {
            assert_eq!(complex_dash_cells(length, 10.16, span), 1);
        }
        assert_eq!(complex_dash_cells(0.5, 10.16, span), 1);
    }

    #[test]
    fn one_glyph_on_a_stub_sits_half_a_period_in_not_at_the_end() {
        // `probe_complex_dash_marks` inks the 6.19 mm stubs' tick at 5.08 and
        // the 6.54 mm stubs' at 5.02 — half the 10.16 mm advance, while the
        // run's own end is 1.1-1.5 mm further on. The 7.62 mm advance's
        // 3.83 mm stubs land at 3.81, the same half period, so it is the
        // advance that anchors the mark and not the run.
        let span = complex_dash_glyph_span_mm(&hydraulic_glyph());
        for (length, period, expected) in [
            (6.19_f64, 10.16_f64, 5.08_f64),
            (6.54, 10.16, 5.08),
            (5.17, 10.16, 5.08),
            (3.83, 7.62, 3.81),
        ] {
            let positions = complex_dash_positions(length, period, span);
            assert_eq!(positions.len(), 1);
            assert!((positions[0] - expected).abs() < 1e-9);
        }
        // A run shorter than half an advance gets no glyph: the reference
        // inks only the rail there, so there is no mark to pull back to the
        // end. `probe_complex_dash_marks` reads the 4.23 mm stub's walls as
        // `0.169 + 4.233 mm` of rail with no mark, and the nearest reference
        // glyph 40 mm away.
        for length in [5.07_f64, 4.23, 4.2, 3.81, 1.32, 0.5] {
            assert!(
                complex_dash_positions(length, 10.16, span).is_empty(),
                "{length} mm is under the 5.08 mm half advance"
            );
        }
    }

    #[test]
    fn more_than_one_cell_divides_the_run_between_them() {
        // The cells stay where the reference inks them: 22.47 mm at 5.61 and
        // 16.78, 34.60 at 5.81 / 17.41 / 28.92.
        let span = complex_dash_glyph_span_mm(&hydraulic_glyph());
        for (length, cells) in [(22.47_f64, 2_usize), (34.60, 3), (36.50, 3), (28.13, 3)] {
            let step = length / cells as f64;
            let expected: Vec<f64> = (0..cells).map(|cell| (cell as f64 + 0.5) * step).collect();
            assert_eq!(complex_dash_positions(length, 10.16, span), expected);
        }
        // The longest run keeps one glyph per advance, the first at half of it.
        let long = complex_dash_positions(203.73, 10.16, span);
        assert_eq!(long.len(), 20);
        assert!((long[0] - 5.09).abs() < 0.01);
    }

    #[test]
    fn a_glyph_with_no_tangent_extent_leaves_the_advance_alone() {
        assert_eq!(complex_dash_cells(37.04, 10.16, 0.0), 4);
    }

    #[test]
    fn a_zero_advance_never_divides() {
        assert_eq!(complex_dash_cells(37.04, 0.0, 2.5), 1);
        assert_eq!(complex_dash_cells(37.04, -1.0, 2.5), 1);
        // A zero advance never reaches the layout: `stamp_complex_dash`
        // returns before asking, and half of nothing is pinned to the start.
        assert_eq!(complex_dash_positions(37.04, 0.0, 2.5), vec![0.0]);
    }

    fn plain_item(shape: &SymbolPrimitive) -> SymbolItem<'_> {
        SymbolItem {
            shape,
            style: None,
            lettering: None,
        }
    }

    #[test]
    fn a_rectangle_ring_is_stored_from_the_corner_the_reader_returns() {
        // The ring is the same rectangle from any corner; storing it from the
        // low corner in the order a written page reads back keeps the round
        // trip endpoint for endpoint (`write_five_drawings`).
        let ring = vec![[4.0, 2.0], [4.0, 6.0], [1.0, 6.0], [1.0, 2.0]];
        assert_eq!(
            canonical_rect_chain(ring),
            vec![[1.0, 2.0], [4.0, 2.0], [4.0, 6.0], [1.0, 6.0]]
        );
    }

    #[test]
    fn a_chain_that_is_not_a_rectangle_is_left_alone() {
        let triangle = vec![[0.0, 0.0], [4.0, 0.0], [0.0, 6.0]];
        assert_eq!(canonical_rect_chain(triangle.clone()), triangle);
        let slanted = vec![[0.0, 0.0], [4.0, 1.0], [5.0, 6.0], [1.0, 5.0]];
        assert_eq!(canonical_rect_chain(slanted.clone()), slanted);
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

    /// A sheet label's run partition reaches the paper, cut against the lines
    /// it letters: the record here is `AB` in one style and `CDE` in another
    /// across a break, so the first line carries the first run and the second
    /// is re-based to its own first character.
    #[test]
    fn a_sheet_label_carries_its_run_partition_onto_the_paper() {
        use pid_parse::style_link::{
            ResolvedTextHeight, ResolvedTextStyle, TextRunStatus, TextStyleSpan,
        };

        let char_style = |id: u32, height_m: f64, font: &str| ResolvedTextHeight {
            style_id: id,
            height_m,
            colour: None,
            alignment: None,
            line_spacing: None,
            font_name: Some(font.to_string()),
        };
        let mut styles = pid_parse::style_link::TextStyleIndex::new();
        styles.insert(
            ("/Sheet6".to_string(), 11),
            ResolvedTextStyle {
                paragraph: Some(char_style(3, 0.0035, "仿宋")),
                run: Some(char_style(9, 0.0035, "仿宋")),
                runs: TextRunStatus::Uniform,
                alignment: None,
                line_spacing: Some(1.5),
                spans: vec![
                    TextStyleSpan {
                        start: 0,
                        len: 2,
                        style: char_style(9, 0.0035, "仿宋"),
                    },
                    // The break itself letters nothing, so it covers no glyph
                    // on either line.
                    TextStyleSpan {
                        start: 2,
                        len: 2,
                        style: char_style(10, 0.007, "宋体"),
                    },
                    TextStyleSpan {
                        start: 4,
                        len: 1,
                        style: char_style(9, 0.0035, "仿宋"),
                    },
                    TextStyleSpan {
                        start: 5,
                        len: 2,
                        style: char_style(10, 0.007, "宋体"),
                    },
                ],
            },
        );
        let mut out = Vec::new();
        push_entity(
            &mut out,
            &note(0.0, "AB\r\nCDE"),
            None,
            &styles,
            &pid_parse::style_link::LineStyleIndex::new(),
            &pid_parse::style_link::FillIndex::new(),
            &mut std::collections::HashSet::new(),
        );
        assert_eq!(out.len(), 2, "one text per line: {out:?}");
        type DrawnSpan = (u16, u16, Option<f64>, Option<String>);
        let spans: Vec<Vec<DrawnSpan>> = out
            .iter()
            .filter_map(|primitive| match &primitive.shape {
                PaperShape::Text { spans, .. } if !spans.is_empty() => Some(
                    spans
                        .iter()
                        .map(|span| (span.start, span.len, span.height_mm, span.font.clone()))
                        .collect(),
                ),
                _ => None,
            })
            .collect();
        assert_eq!(
            spans,
            vec![
                vec![(0, 2, Some(3.5), Some("仿宋".to_string()))],
                vec![
                    (0, 1, Some(3.5), Some("仿宋".to_string())),
                    (1, 2, Some(7.0), Some("宋体".to_string())),
                ],
            ],
            "each line keeps the runs that land on it, in its own offsets"
        );
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
        let lines = |value: &str| -> Vec<String> {
            label_line_ranges(value)
                .into_iter()
                .map(|line| line.text)
                .collect()
        };
        assert_eq!(lines("a\rb"), ["a", "b"]);
        assert_eq!(lines("a\r\nb"), ["a", "b"]);
        assert_eq!(lines("a\nb"), ["a", "b"]);
        assert_eq!(
            lines("a\u{0B}b\u{0C}c\u{2028}d\u{2029}e"),
            ["a", "b", "c", "d", "e"]
        );
        assert_eq!(lines("a\r"), ["a"]);
        assert_eq!(lines("a\u{0}b\tc"), ["ab c"]);
        assert_eq!(lines("\u{0}"), [""]);
        assert_eq!(lines(""), [""]);
    }

    /// Each line knows where it came from, so a record's run offsets can be
    /// cut per line — and a line that lost a control character says its
    /// offsets no longer line up.
    #[test]
    fn a_line_states_where_in_the_record_it_starts() {
        let runs = label_line_ranges("ab\r\ncdef");
        assert_eq!(
            runs.iter()
                .map(|line| (line.text.as_str(), line.start, line.exact))
                .collect::<Vec<_>>(),
            vec![("ab", 0, true), ("cdef", 4, true)],
            "the `\\r\\n` pair is stepped over, not counted as a character"
        );
        let dropped = label_line_ranges("a\u{0}b");
        assert_eq!(dropped[0].text, "ab");
        assert!(
            !dropped[0].exact,
            "the dropped character breaks the offsets"
        );
    }

    /// A record's runs are cut against the lines they land on: the first run
    /// here ends on the break (so the second line does not inherit it), and the
    /// second is re-based to that line's own first character.
    #[test]
    fn a_run_is_cut_against_the_line_it_lands_on() {
        let partition = vec![
            // `ab\r\n` in one style — the break belongs to neither line.
            PaperTextSpan {
                start: 0,
                len: 4,
                height_mm: Some(2.0),
                font: None,
            },
            // `cdef` in another.
            PaperTextSpan {
                start: 4,
                len: 4,
                height_mm: Some(4.0),
                font: None,
            },
        ];
        let lines = label_line_ranges("ab\r\ncdef");
        let first: Vec<(u16, u16)> = line_spans(&partition, &lines[0])
            .iter()
            .map(|span| (span.start, span.len))
            .collect();
        let second: Vec<(u16, u16)> = line_spans(&partition, &lines[1])
            .iter()
            .map(|span| (span.start, span.len))
            .collect();
        assert_eq!(
            first,
            vec![(0, 2)],
            "the break itself letters nothing: the first line keeps two characters"
        );
        assert_eq!(second, vec![(0, 4)], "the second re-bases to its origin");
        assert!(
            sheet_text_spans(None, "ab").is_empty(),
            "no resolved style, no partition"
        );
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

    /// The bundled library holds one label that really mixes styles, and it
    /// reaches the page in pieces: `Wastewater Pit.sym`'s description letters
    /// `NULL  NULL*NULL*NULL` with a 3.175 mm 宋体 `*` inside a 2.822 mm Arial
    /// line. This is the end-to-end proof that the partition is not merely
    /// carried in the model — the PDF is cut into three runs, the middle one
    /// larger, each starting where the previous ended.
    #[test]
    fn a_library_label_that_mixes_sizes_is_lettered_in_three_runs() {
        use super::PaperModel;
        use crate::read::read_plot;
        use crate::write::{PlotFont, write_plot};

        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../test-file/symbols-full/Equipment/Labels - Equipment/\
             Descriptions new/Wastewater Pit.sym",
        );
        if !path.is_file() {
            eprintln!("skipping: {} is not in this checkout", path.display());
            return;
        }
        let geometry =
            pid_parse::symbol_library::read_symbol_geometry(&path).expect("the symbol reads");
        let styled = geometry
            .primitives
            .iter()
            .find(|styled| {
                matches!(
                    &styled.primitive,
                    SymbolPrimitive::Text { text, .. } if text == "NULL  NULL*NULL*NULL"
                )
            })
            .expect("the mixed label");
        let mut out = Vec::new();
        push_symbol_primitive(
            &mut out,
            &entity(PidGraphicKind::Point {
                position: PidPoint { x: 0.0, y: 0.0 },
            }),
            &placement(),
            SymbolItem {
                shape: &styled.primitive,
                style: styled.style.as_ref(),
                lettering: styled.lettering.as_ref(),
            },
            None,
            None,
        );
        assert_eq!(out.len(), 1, "one label in the table: {out:?}");
        let PaperShape::Text { value, spans, .. } = &out[0].shape else {
            panic!("{:?}", out[0].shape);
        };
        assert_eq!(value, "NULL  NULL*NULL*NULL");
        let sizes: Vec<f64> = spans
            .iter()
            .map(|span| span.height_mm.unwrap_or_default())
            .collect();
        assert_eq!(sizes.len(), 3, "three runs: {spans:?}");
        assert!(
            (sizes[0] - 2.822).abs() < 0.001
                && (sizes[1] - 3.175).abs() < 0.001
                && (sizes[2] - 2.822).abs() < 0.001,
            "the middle run is the larger one: {sizes:?}"
        );

        let model = PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some([200.0, 100.0]),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            primitives: out,
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
        let text: String = ink.spans.iter().map(|span| span.text.as_str()).collect();
        assert_eq!(
            text, "NULL  NULL*NULL*NULL",
            "every character still reaches the page: {:?}",
            ink.spans
        );
        assert_eq!(
            ink.spans.len(),
            3,
            "three runs on the page: {:?}",
            ink.spans
        );
        let heights: Vec<f64> = ink.spans.iter().map(|span| span.height_mm).collect();
        assert!(
            heights[1] > heights[0] + 0.2 && heights[1] > heights[2] + 0.2,
            "the middle run letters larger: {heights:?}"
        );
        for pair in ink.spans.windows(2) {
            let expected = pair[0].origin_mm[0] + pair[0].advance_mm;
            assert!(
                (pair[1].origin_mm[0] - expected).abs() < 0.1,
                "each run starts where the one before it ended: {:?}",
                ink.spans
            );
        }
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
    fn arc_shape_keeps_the_endpoints_and_the_records_direction() {
        // A pair one way round is the record's own start/end; the direction
        // decides which of the two arcs between them is drawn, and the angles
        // are never swapped to fold it back to the short side.
        let PaperShape::Arc {
            start_angle,
            end_angle,
            clockwise,
            ..
        } = arc_shape([0.0, 0.0], 0.01, 1.1659, 1.4273, false, &placement())
        else {
            panic!("expected arc");
        };
        assert!((start_angle - 1.1659).abs() < 1e-6);
        assert!((end_angle - 1.4273).abs() < 1e-6);
        assert!(!clockwise);

        // A semicircle has no "short side": the two records of the
        // ultrasonic wave carry the same pair and opposite bytes, and the two
        // arcs they draw are the two halves.
        for flag_zero in [true, false] {
            let PaperShape::Arc { clockwise, .. } = arc_shape(
                [0.0, 0.0],
                0.01,
                PI / 2.0,
                3.0 * PI / 2.0,
                flag_zero,
                &placement(),
            ) else {
                panic!("expected arc");
            };
            assert_eq!(clockwise, flag_zero);
        }

        // A mirrored placement reverses the direction with the geometry.
        let mut mirrored = placement();
        mirrored.scale = [-1.0, 1.0];
        let PaperShape::Arc { clockwise, .. } =
            arc_shape([0.0, 0.0], 0.01, 1.1659, 1.4273, true, &mirrored)
        else {
            panic!("expected arc");
        };
        assert!(!clockwise);
    }

    #[test]
    fn an_arc_reaches_only_the_quadrants_its_sweep_crosses() {
        // 10° -> 100° crosses 90° and nothing else: the ink stops at the
        // quadrant, not at the whole circle the arc belongs to.
        let shape = PaperShape::Arc {
            center_mm: [0.0, 0.0],
            radius_mm: 2.0,
            start_angle: 10.0_f64.to_radians(),
            end_angle: 100.0_f64.to_radians(),
            clockwise: false,
        };
        let points = shape_points(&shape);
        let min = [
            points.iter().map(|p| p[0]).fold(f64::MAX, f64::min),
            points.iter().map(|p| p[1]).fold(f64::MAX, f64::min),
        ];
        let max = [
            points.iter().map(|p| p[0]).fold(f64::MIN, f64::max),
            points.iter().map(|p| p[1]).fold(f64::MIN, f64::max),
        ];
        assert!((min[0] + 0.3473).abs() < 0.002, "the 100° end: {min:?}");
        assert!((min[1] - 0.3473).abs() < 0.002, "the 10° end: {min:?}");
        assert!((max[0] - 1.9696).abs() < 0.002, "the 10° end: {max:?}");
        assert!((max[1] - 2.0).abs() < 1e-9, "90° is crossed: {max:?}");

        // A clockwise record takes the other side of the circle: −10° → −100°
        // crosses −90°, so the ink reaches the bottom instead of the top.
        let below = PaperShape::Arc {
            center_mm: [0.0, 0.0],
            radius_mm: 2.0,
            start_angle: (-10.0_f64).to_radians(),
            end_angle: (-100.0_f64).to_radians(),
            clockwise: true,
        };
        let points = shape_points(&below);
        let low = points.iter().map(|p| p[1]).fold(f64::MAX, f64::min);
        let high = points.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
        assert!((low + 2.0).abs() < 1e-9, "the sweep crosses −90°: {low}");
        assert!(
            (high + 0.347).abs() < 0.002,
            "the high end is the −10° end: {high}"
        );
    }

    #[test]
    fn a_cached_only_placement_owns_the_place_it_drew() {
        // The title block's wave: two arcs of the file's own cache, drawn at
        // (1100.69, 19.00) mm while the named `.sym` is absent. A residual
        // inside the ink is the missing source's business, not a decoder's.
        let model = PaperModel {
            library: Vec::new(),
            template: Some(BorderTemplate {
                path: "CPE-SQ-A0.igr".to_string(),
                extent_mm: [1189.0, 841.0],
                primitives: 0,
                symbols: vec![TemplateSymbol {
                    key: "Temp\\BLFH.sym".to_string(),
                    insertion_mm: [1100.6925, 19.0],
                    body: SymbolBodySource::Cached,
                    library_available: false,
                    extent_mm: Some([[1098.39, 18.56], [1103.01, 19.41]]),
                    name_gap: Some(NameMiss::NoSuchFile),
                }],
            }),
            page_mm: Some([1189.0, 841.0]),
            primitives: Vec::new(),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            missing_symbols: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
        };
        let owner = model
            .template_symbol_at([1100.0, 19.0], 0.5)
            .expect("inside the wave");
        assert_eq!(owner.key, "Temp\\BLFH.sym");
        assert_eq!(owner.body, SymbolBodySource::Cached);
        assert!(!owner.library_available);
        assert!(
            model.template_symbol_at([1100.0, 21.0], 0.5).is_none(),
            "the box is the ink's, not the whole page"
        );

        // A placement that drew nothing still owns the box the page leaves.
        let mut undrawn = model.clone();
        if let Some(template) = undrawn.template.as_mut() {
            template.symbols = vec![TemplateSymbol {
                key: "Gone.sym".to_string(),
                insertion_mm: [100.0, 100.0],
                body: SymbolBodySource::Missing,
                library_available: false,
                extent_mm: None,
                name_gap: None,
            }];
        }
        assert!(undrawn.template_symbol_at([112.0, 90.0], 0.0).is_some());
        assert!(undrawn.template_symbol_at([120.0, 100.0], 0.0).is_none());
    }

    const PI: f64 = std::f64::consts::PI;

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
            true,
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

    /// The page the PDF gets is the model's own answer: a stated sheet is
    /// reported unchanged, and a drawing that states none is measured onto a
    /// standard sheet rather than onto its ink's box.
    #[test]
    fn a_measured_page_is_a_standard_sheet_and_a_stated_one_is_reported_as_is() {
        fn model(page_mm: Option<[f64; 2]>, lines: &[([f64; 2], [f64; 2])]) -> PaperModel {
            PaperModel {
                library: Vec::new(),
                template: None,
                page_mm,
                primitives: lines
                    .iter()
                    .map(|(start, end)| PaperPrimitive {
                        shape: PaperShape::Line {
                            start_mm: *start,
                            end_mm: *end,
                        },
                        stroke: None,
                        symbol: None,
                        confidence: "decoded",
                        stream_path: None,
                        byte_range: None,
                        furniture: false,
                    })
                    .collect(),
                off_sheet_primitives: 0,
                skipped_endpoint_pairs: 0,
                missing_symbols: Vec::new(),
                missing_boxes: Vec::new(),
                symbols_found: 0,
                symbols_referenced: 0,
            }
        }

        // A stated sheet wins even when the ink would fit a smaller one: the
        // drawing is the authority on its own paper.
        assert_eq!(
            model(Some([841.0, 594.0]), &[([10.0, 10.0], [20.0, 20.0])]).resolved_page_mm(),
            Some([841.0, 594.0])
        );
        // No stated sheet, ink that fits A4: A4 is the answer, not the 20 mm box.
        assert_eq!(
            model(None, &[([10.0, 10.0], [100.0, 50.0])]).resolved_page_mm(),
            Some([297.0, 210.0])
        );
        // Ink too big for every standard sheet keeps its own box rather than
        // being squeezed onto the largest one.
        assert_eq!(
            model(None, &[([0.0, 0.0], [2000.0, 1500.0])]).resolved_page_mm(),
            Some([2000.0, 1500.0])
        );
        // Neither a sheet nor ink: nothing to put on paper.
        assert_eq!(model(None, &[]).resolved_page_mm(), None);
    }

    #[test]
    fn a_link_spells_the_template_stem_it_points_at() {
        use std::path::PathBuf;

        use super::{linked_template_prefix, template_named_by};

        // A record written through Windows' 8.3 name keeps six characters of
        // it. The `~` in the *directories* is not the file's.
        assert_eq!(
            linked_template_prefix(r"C:\PID_SQ~1\SQPlant\Ref\TEMPLA~1\CPE-SQ~2.IGR").as_deref(),
            Some("CPE-SQ")
        );
        // A record authored on a share that kept long names spells the whole
        // stem, and one on the authoring machine spells its own path.
        assert_eq!(
            linked_template_prefix(r"\CPECCHB-SQ-A2.igr").as_deref(),
            Some("CPECCHB-SQ-A2")
        );
        assert_eq!(
            linked_template_prefix(r"Ref\Template Files\CPECCHB-SQ-A1.igr").as_deref(),
            Some("CPECCHB-SQ-A1")
        );
        // Anything that is not a template names no template.
        assert_eq!(linked_template_prefix(r"Ref\Symbols\Gate Valve.sym"), None);
        assert_eq!(linked_template_prefix("CPE-SQ~2"), None);

        let prefixes = vec!["CPE-SQ".to_string()];
        assert!(template_named_by(
            &PathBuf::from("CPE-SQ-A1.igr"),
            &prefixes
        ));
        assert!(template_named_by(
            &PathBuf::from("CPE-SQ-A0.igr"),
            &prefixes
        ));
        // `CPE-SQ` is not a prefix of `CPECCHB-SQ-A1`: another family's A1 of
        // the same size stays a stranger.
        assert!(!template_named_by(
            &PathBuf::from("CPECCHB-SQ-A1.igr"),
            &prefixes
        ));
    }

    #[test]
    fn the_named_template_beats_a_stranger_of_the_same_size() {
        use std::path::PathBuf;

        use super::template_rank;

        // Twenty A1 sheets say `CPE-SQ~2.IGR`. `CPECCHB-SQ-A1.igr` is the same
        // size, and its own float noise sits 3.8e-10 mm closer to the sheet's
        // — closer by less than an atom, and enough to have decided it before
        // the name was read.
        let prefixes = vec!["CPE-SQ".to_string()];
        let named = template_rank(&PathBuf::from("CPE-SQ-A1.igr"), 4.0e-10, &prefixes);
        let stranger = template_rank(&PathBuf::from("CPECCHB-SQ-A1.igr"), 0.0, &prefixes);
        assert!(named < stranger, "named {named:?} vs stranger {stranger:?}");
    }

    /// One of `Q井场`'s placement styles, at the width asked for.
    ///
    /// `style_id 92` and the salmon colour are that drawing's `BandAid` entry;
    /// the same shape serves for an ordinary pen simply by stating its width.
    fn placement_style(width_mm: f64) -> pid_parse::style_link::ResolvedLineStyle {
        pid_parse::style_link::ResolvedLineStyle {
            style_id: 92,
            symbology: pid_parse::style_link::LineSymbology {
                width_m: width_mm / 1000.0,
                colour: 0x0077_A7FF,
            },
            dash: None,
            marker: None,
            complex_dash: None,
            hop: pid_parse::style_link::StyleHop::Direct,
        }
    }

    #[test]
    fn a_placement_pen_still_paints_the_body_and_a_validation_band_does_not() {
        let body = PrimitiveStyle {
            rgb: [0, 0, 0],
            width_mm: 0.13,
            dash_mm: Vec::new(),
        };
        // The valve's own pen: nothing placed, and every pen a placement can
        // carry, keeps the body's outline as the body draws it.
        let plain = body_stroke(Some(&body), None).expect("a pen");
        assert!((plain.width_mm - 0.13).abs() < 1e-9);
        for pen in [0.13, 0.35, 0.70] {
            let styled = body_stroke(Some(&body), Some(&placement_style(pen))).expect("a pen");
            assert!(
                (styled.width_mm - pen).abs() < 1e-9,
                "a placement's {pen} mm pen paints the body, not {} mm",
                styled.width_mm
            );
            assert_eq!(styled.rgb, [255, 167, 119], "the placement's colour stands");
        }
        // `Q井场`'s globe valve is placed under `BandAid`: 10 mm of validation
        // band, which used to repaint a hairline valve with the thick pen.
        let band_aid = body_stroke(Some(&body), Some(&placement_style(10.0))).expect("a pen");
        assert!(
            (band_aid.width_mm - 0.13).abs() < 1e-9,
            "the band's 10 mm is a mark, so the body keeps its own pen ({}) mm",
            band_aid.width_mm
        );
        // A body that states nothing falls back the same way a bare placement
        // does, so the mark still leaves a drawable pen behind it.
        let bare = body_stroke(None, Some(&placement_style(10.0))).expect("a pen");
        assert!((bare.width_mm - 0.25).abs() < 1e-9, "{} mm", bare.width_mm);
    }

    #[test]
    fn an_error_a_micrometre_apart_is_the_same_error() {
        use std::path::PathBuf;

        use super::template_rank;

        // The sheet that names nothing (`600td`, 842.54 x 595.54 mm) is 1.54 mm
        // off both A1 templates, and the two errors differ by the same 3.8e-10:
        // with no name to go on, the rule still has to answer the same way
        // twice, so the name on disk decides.
        let cpe = template_rank(&PathBuf::from("CPE-SQ-A1.igr"), 1.54, &[]);
        let cchb = template_rank(&PathBuf::from("CPECCHB-SQ-A1.igr"), 1.5399999, &[]);
        assert_eq!(cpe.1, cchb.1, "the two errors are one error");
        assert!(
            cpe < cchb,
            "the name on disk breaks it: {cpe:?} vs {cchb:?}"
        );

        // A tenth of a millimetre is a reading, not noise: the closer extent
        // still wins over the same name's order.
        let close = template_rank(&PathBuf::from("CPE-SQ-A1.igr"), 0.0, &[]);
        let far = template_rank(&PathBuf::from("CPE-SQ-A1.igr"), 0.15, &[]);
        assert!(close < far, "0.15 mm is not quantized away");
        assert!(close.1 < far.1, "the quantized errors still differ");
    }
}
