//! Symbol bodies read from the `SmartPlant` `.sym` reference library.
//!
//! An `igSymbol2d` placement in a drawing says *where* a symbol goes and
//! which library file it comes from, but not what it looks like — the body
//! lives in a separate `.sym` file on the project's reference-data share.
//! Without it, a valve can only be marked, not drawn.
//!
//! A `.sym` turns out to be the same compound file as a `.pid`, down to the
//! cluster magic and the 6-byte PSM record envelope, with one structural
//! simplification: a symbol's `Sheet*` stream is a plain record chain
//! starting at [`CHAIN_START`], so the records can be walked rather than
//! searched for. Across the 618-file reference library that walk lands
//! exactly on the last byte of all 1134 sheet streams, which is what makes
//! it safe to read the chain instead of scanning for candidates the way the
//! drawing-side decoders must.
//!
//! Five record types carry something to draw — four geometry families and
//! the symbol's own lettering; the rest are connect points, styles and
//! structure. See [`SymbolPrimitive`].
//!
//! ```no_run
//! use pid_parse::symbol_library::SymbolLibrary;
//!
//! let mut library = SymbolLibrary::new(r"C:\Plant\Ref\Symbols");
//! if let Some(body) = library.resolve(r"\\WIN-SPID\p\Ref\Symbols\Piping\Valves\Angle\2-Way Angle Globe Valve.sym") {
//!     println!("{} primitives", body.primitives.len());
//! }
//! ```

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::api::PidParser;
use crate::error::PidError;
use crate::parsers::sheet_records::{
    igtextbox_body_shape, igtextbox_letter_height, igtextbox_runs, IGTEXTBOX_BODY_START,
    IGTEXTBOX_TAIL_KIND2_LEN, IGTEXTBOX_TAIL_KIND_AT, IGTEXTBOX_TAIL_KIND_BOX,
    IGTEXTBOX_TAIL_KIND_PLACEMENT, IGTEXTBOX_TAIL_LEN,
};
use crate::style_link::{DocumentStyleTable, ResolvedTextStyle, TextAlignment};
use crate::DecodedTextRun;

/// `u32` LE signature shared by every cluster-family stream, including a
/// symbol's `Sheet*` streams.
const CLUSTER_MAGIC: u32 = 0x6C90_F544;

/// Offset of the first record in a symbol `Sheet*` stream.
///
/// The first 8 bytes are the cluster magic and a record count. The bytes a
/// cluster-header reader would take as `stream_type` / `body_len` / `flags`
/// at 8..16 are in fact the first record's own PSM envelope, which is why
/// the chain starts here and not at 16.
pub const CHAIN_START: usize = 8;

/// Length of the PSM record envelope: `u16` type code + `u32` payload length.
const PSM_ENVELOPE_LEN: usize = 6;

/// Payload bytes ahead of the geometry: oid, parent reference, a `u32`, a
/// `u16` sub-type, and an index. Read positionally — the drawing-side
/// decoders name the same span but validate values a `.sym` does not share
/// (`igLine2d` at `+8` holds 12 in a `.pid` and 8 here).
const PAYLOAD_HEADER_LEN: usize = 18;

/// Offset of the style index inside the payload header, four bytes wide.
const STYLE_INDEX_OFFSET: usize = 14;

const TYPE_LINE: u16 = 0x0018;
const TYPE_CIRCLE: u16 = 0x0059;
const TYPE_ARC: u16 = 0x0061;
const TYPE_POLYLINE: u16 = 0x0084;
const TYPE_TEXT: u16 = 0x004D;
const TYPE_BSPLINE: u16 = 0x005D;
const TYPE_BOUNDARY: u16 = 0x0013;

/// Bytes of placement and style following a text record's body: the
/// insertion point (two `f64`s), the text direction pair, and a 4-byte
/// trailer.
const TEXT_TRAILING_LEN: usize = IGTEXTBOX_TAIL_LEN;

/// Payload offset of a text record's shape discriminator.
///
/// `igTextBox` (PSM `0x004D`) comes in three shapes and each puts its
/// character count and text at its own offset, so the discriminator is what
/// says where to look. Reading a fixed offset — this reader once tried `+30`
/// and `+22` — describes shapes 1 and 2 only, and misses shape 3 entirely.
const TEXT_SHAPE_AT: usize = 18;

/// Longest run of characters a symbol label can plausibly hold. The corpus
/// tops out at 117 (an embedded XML field reference); this only has to
/// reject a count that would reach past the payload anyway.
const TEXT_MAX_CHARS: usize = 1024;

/// Payload length of a line record: header plus four `f64`s.
const LINE_PAYLOAD_LEN: usize = PAYLOAD_HEADER_LEN + 32;
/// Payload length of a circle record: header, three `f64`s, one trailing byte.
const CIRCLE_PAYLOAD_LEN: usize = PAYLOAD_HEADER_LEN + 24 + 1;
/// Payload length of an arc record: header, five `f64`s, one trailing byte.
const ARC_PAYLOAD_LEN: usize = PAYLOAD_HEADER_LEN + 40 + 1;
/// Offset of a polyline's first vertex, past the vertex count and the
/// form/scope bytes.
const POLYLINE_VERTEX_START: usize = 24;
/// Guard against a corrupt vertex count claiming an implausible polyline.
const POLYLINE_MAX_VERTICES: usize = 4096;

/// Widest coordinate a symbol body can plausibly use, in source units
/// (metres). The reference library's own extent is under 0.6; an order of
/// magnitude of headroom rejects garbage without rejecting real geometry.
const COORDINATE_LIMIT: f64 = 10.0;

/// The path separator `SmartPlant` reference shares are addressed by, and the
/// marker that splits the site-specific UNC prefix from the library-relative
/// path. Placements name the same symbol through several different shares
/// (`\\WIN-SPID\...`, `\\SPID\...`, `\\MM-128\...`), and the part after this
/// marker is what they agree on.
const LIBRARY_MARKER: &str = r"\symbols\";

/// One drawable element of a symbol body, in the symbol's own coordinate
/// space (source units, i.e. metres — the same unit the drawing uses).
///
/// The library draws with five of the 27 PSM type codes that appear in it —
/// four geometry families and one text family; the others carry connect
/// points, line styles and object structure, none of which put anything on
/// the sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum SymbolPrimitive {
    /// Straight segment (PSM `0x0018` `igLine2d`) — 43% of all library
    /// records and the bulk of every symbol.
    Line {
        /// Segment start.
        start: (f64, f64),
        /// Segment end.
        end: (f64, f64),
    },
    /// Full circle (PSM `0x0059` `igCircle2d`).
    Circle {
        /// Circle centre.
        center: (f64, f64),
        /// Radius; always positive across the library.
        radius: f64,
    },
    /// Circular arc (PSM `0x0061`), from [`Self::Arc::start_angle`] to
    /// [`Self::Arc::end_angle`] in the direction
    /// [`Self::Arc::clockwise`] states.
    ///
    /// Both angles are absolute, not a start plus a sweep: the two arcs of
    /// `ElecTraceLine` run `2.944 -> 0.197` and `6.086 -> 3.339` radians,
    /// which are different numbers but the same sweep at the same radius,
    /// as a repeating wave symbol requires. Which of the two arcs between
    /// them the record means is its own trailing byte: `FT-Ultrasonic
    /// Flowmeter.sym` carries two records with the *same* `90° -> 270°`
    /// pair and opposite bytes, because the wave's two halves bulge opposite
    /// ways. See
    /// `docs/analysis/2026-10-23-the-arc-trailing-byte-is-the-direction.md`.
    Arc {
        /// Arc centre.
        center: (f64, f64),
        /// Radius; always positive across the library.
        radius: f64,
        /// Start angle in radians, counter-clockwise from +X.
        start_angle: f64,
        /// End angle in radians, counter-clockwise from +X.
        end_angle: f64,
        /// `true` when the arc runs clockwise from the start angle to the
        /// end angle; the record's trailing byte is `0` for clockwise and
        /// `1` for counter-clockwise.
        clockwise: bool,
    },
    /// Connected vertex run (PSM `0x0084` `igLineString2d`).
    Polyline {
        /// Vertices in source order.
        vertices: Vec<(f64, f64)>,
        /// Whether the authored `form` byte closes the last vertex back to
        /// the first (`form == 2`; `form == 1` is open on the corpus).
        is_closed: bool,
    },
    /// Lettering the symbol carries itself (PSM `0x004D` `igTextBox`) — the
    /// `设备位号` header on an equipment table, the `HH=` on an alarm, the
    /// `XXX` a note symbol shows as its own sample.
    ///
    /// The library is a set of templates, so a run is often the placeholder
    /// `NULL`, standing for a value the drawing supplies at placement time
    /// rather than text anyone means to read. It is decoded as it appears;
    /// deciding whether to draw it belongs to whoever is drawing.
    ///
    /// The record's own placement tail states a direction and an anchor, and
    /// its `+14` style index resolves to a height, a colour and a typeface —
    /// all of it on [`StyledPrimitive::lettering`], since it is symbology
    /// rather than shape.
    Text {
        /// Text content, decoded from UTF-16LE.
        text: String,
        /// Insertion point in the symbol's own coordinate space.
        at: (f64, f64),
    },
    /// Non-uniform B-spline curve (PSM `0x005D` `igBspCurve2d`), rational
    /// when `weights` is non-empty. One in the reference library's placed
    /// set -- the curved lip of `arrester breather valve(RD)` -- which the
    /// reader used to step over. Draw it through [`crate::bspline::sample`].
    BSpline {
        /// Control points, in order.
        poles: Vec<(f64, f64)>,
        /// One weight per pole, or empty for a polynomial curve.
        weights: Vec<f64>,
        /// The knot vector; its length minus the pole count minus one is the
        /// degree.
        knots: Vec<f64>,
    },
    /// A filled region (PSM `0x0013` `igBoundary2d`) — the flange dots of
    /// `Wellhead Christmas Tree 02.sym`, which `SmartPlant`'s plot fills
    /// solid where a reader that only stroked the member circles draws a
    /// hairline ring. The record's members re-list shapes that are also
    /// stroked on their own, so the area is what draws, not the outline:
    /// a consumer fills each member rather than stroking it.
    ///
    /// The record's style index names a fill-family style, which rides
    /// along resolved (the same slot a stroke's line style would carry,
    /// but a boundary's index resolves nothing through
    /// `resolve_line_style`).
    Boundary {
        /// The member shapes the region fills over — circles on the
        /// corpus (`0x73` groups), straight segments where a boundary
        /// lists lines (`0x67` groups). Only `Circle` and `Line` members
        /// ever appear here.
        members: Vec<SymbolPrimitive>,
        /// What the record's style index resolves to through the
        /// symbol's own table — `None` where it names no fill style.
        fill: Option<PrimitiveFill>,
    },
}

impl SymbolPrimitive {
    /// The straight segments a B-spline draws as, `segments_per_span` to a
    /// knot span; other primitives return nothing.
    pub fn bspline_points(&self, segments_per_span: usize) -> Vec<(f64, f64)> {
        match self {
            SymbolPrimitive::BSpline {
                poles,
                weights,
                knots,
            } => crate::bspline::sample(poles, weights, knots, segments_per_span),
            _ => Vec::new(),
        }
    }
}

/// The colour, width and dash one primitive draws with, as the symbol's own
/// style table states them.
///
/// A symbol states its own symbology: every `.sym` carries a `StyleCluster`
/// stream of the same shape a `.pid` does, and each of the symbol's own
/// records indexes into it exactly as a drawing's line work indexes into the
/// drawing's. The drawing's definition cache says the same thing the same
/// way -- each `/JSite<N>/` storage has its own `StyleCluster`, and the
/// cached body's strokes index into that one
/// ([`crate::JSite::stroke_styles`]) -- so a body from either source
/// arrives with this per stroke.
///
/// The placement record names one more style for the whole body
/// (`igSymbol2d +25`, `docs/analysis/2026-08-24-placement-names-the-body-style.md`),
/// and on `SmartPlant`'s screen that one's colour wins over these. What
/// these still decide is the dash -- the placement styles of the corpus are
/// all solid, and the dashed strokes of an off-page connector or a
/// wastewater pit are dashed by their own style alone -- and the whole
/// look of a body whose placement style does not resolve.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PrimitiveStyle {
    /// Stroke colour as `[r, g, b]`.
    pub rgb: [u8; 3],
    /// Stroke width in millimetres.
    pub width_mm: f64,
    /// The dash pattern the style names, as segment lengths in millimetres
    /// in the file's own order and sign
    /// ([`crate::style_link::DashPattern::segments_mm`]: a renderer that
    /// only needs the repeat uses the magnitudes), or empty for a stroke
    /// that draws solid -- which is what most of either corpus says.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dash_mm: Vec<f64>,
}

impl PrimitiveStyle {
    /// The style a resolved line style states, in this vocabulary.
    #[must_use]
    pub fn from_resolved(resolved: &crate::style_link::ResolvedLineStyle) -> Self {
        Self {
            rgb: resolved.symbology.rgb(),
            width_mm: resolved.symbology.width_mm(),
            dash_mm: resolved
                .dash
                .as_ref()
                .map(crate::style_link::DashPattern::segments_mm)
                .unwrap_or_default(),
        }
    }

    /// Whether the stroke draws dashed rather than solid.
    #[must_use]
    pub fn is_dashed(&self) -> bool {
        !self.dash_mm.is_empty()
    }
}

/// The fill a boundary record draws with, as the symbol's own style
/// table states it — the same vocabulary
/// [`crate::style_link::ResolvedFill`] answers in, slimmed to what a
/// renderer needs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PrimitiveFill {
    /// The stated fill colour, `None` where the style names a pattern
    /// fill or states no colour — draw it in the layer's colour then,
    /// the same rule the drawing-side fills follow.
    pub rgb: Option<[u8; 3]>,
    /// The PSM type code of the style reached — `0x002A` solid or
    /// `0x002B` hatch.
    pub type_code: u16,
}

impl PrimitiveFill {
    /// The fill a resolved fill style states, in this vocabulary.
    #[must_use]
    pub fn from_resolved(resolved: &crate::style_link::ResolvedFill) -> Self {
        Self {
            rgb: resolved.rgb(),
            type_code: resolved.type_code,
        }
    }
}

/// One drawable primitive together with the symbology its own `.sym` states
/// for it.
///
/// `style` is `None` where the record's index names nothing this reader can
/// follow — a fill or text style, or a `.sym` with no readable style table.
/// That is "draw it the way you would have anyway", not a failure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyledPrimitive {
    /// What to draw, in the symbol's own coordinate space.
    pub primitive: SymbolPrimitive,
    /// What to draw it with.
    pub style: Option<PrimitiveStyle>,
    /// The lettering a [`SymbolPrimitive::Text`] draws with — `Some` for
    /// every decoded text record, since the record's own tail always reads
    /// its anchor and direction even where the style index resolves
    /// nothing — and `None` for every other shape.
    #[serde(default)]
    pub lettering: Option<SymbolLettering>,
    /// The `JSheetLayer` oid the record's sub-header names — the same
    /// `sheet_layer_ref` slot the drawing-side families carry. `0` when the
    /// record names no layer; the name it resolves to rides along in
    /// [`Self::sheet_layer_name`].
    #[serde(default)]
    pub sheet_layer_ref: u32,
    /// The name [`Self::sheet_layer_ref`] resolves to through the symbol's
    /// own `JSheetLayer` table, `None` where the ref is `0`, names no layer
    /// in the table, or the symbol carries no table to ask.
    #[serde(default)]
    pub sheet_layer_name: Option<String>,
    /// The layer's number — the bit index the sheet's `Top ViewFilterSet`
    /// answers at — resolved through the same table as
    /// [`Self::sheet_layer_name`]. `u32::MAX` where no layer resolves.
    #[serde(default = "unresolved_layer_number")]
    pub sheet_layer_number: u32,
    /// The `JSheet` oid of the sheet stream the record was read from — the
    /// `N` in its `SheetN` stream name. A `.sym` often keeps the body twice:
    /// the sheet `SmartPlant` draws (the one its `Top ViewFilterSet` files
    /// the supplementary layers off on) and an alternate view of it that
    /// shows everything. `0` where the stream's name was not `SheetN`.
    #[serde(default)]
    pub sheet_ref: u32,
}

/// The lettering a `.sym` text record draws with: what the record's own
/// placement tail states — the `(cos, sin)` direction as rotation and, where
/// the pair is height-scaled, a letter height; the trailer's anchor byte —
/// joined with what the record's `+14` style index resolves to through the
/// symbol's `StyleCluster`.
///
/// The tail is the same 36-byte layout the drawing-side `igTextBox` reads
/// ([`crate::parsers::sheet_records::SheetIgTextBoxDecoded`]): insertion,
/// direction pair, 4-byte trailer. The style hop is the same one too — the
/// index names a `JStyleTextPara`, which names the `JStyleTextChar` the
/// metrics come off — resolved eagerly here because the table a `.sym` reads
/// is its own and [`read_symbol_geometry`] is the last place it is in scope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SymbolLettering {
    /// The record's own anchor: the last byte of the trailer that closes
    /// the placement tail. The high nibble reads as on a drawing sheet,
    /// `(code >> 4) % 3` horizontal (`0` left, `1` centre, `2` right), and
    /// overrides `alignment` the same way. The low nibble does not: a
    /// symbol's own label hangs from the middle unless `(code & 0x0F) % 3`
    /// is `1`, the top. See
    /// `docs/analysis/2026-10-19-symbol-label-nibble-hangs-from-the-middle.md`.
    pub anchor_code: u8,
    /// Text rotation in radians, from the tail's `(cos, sin)` pair.
    pub rotation_rad: f64,
    /// The letter height the direction pair is scaled to, `0.0` where the
    /// pair is the normalized unit vector — the same reading the sheet
    /// decoder makes. A `.sym` that scales its direction states its letter
    /// height here; one that keeps it unit asks the style's `height_m`.
    pub letter_height_m: f64,
    /// Character height in metres the record's style index resolves to,
    /// `None` where the index names no character style or there is no table
    /// to ask — `letter_height_m` or a consumer's own fallback answers then.
    pub height_m: Option<f64>,
    /// Character colour as `[r, g, b]` when the resolved style states one.
    pub rgb: Option<[u8; 3]>,
    /// The paragraph's stated horizontal justification — the default
    /// `anchor_code` overrides.
    pub alignment: Option<TextAlignment>,
    /// Line spacing as a multiple of the character height, for a label
    /// with a line break.
    pub line_spacing: Option<f64>,
    /// Typeface the character style names, verbatim.
    pub font_name: Option<String>,
    /// The character style the record's **widest formatting run** names, when
    /// one won — `None` where the record stores no run (shape 1) or its runs
    /// do not resolve, and the paragraph default at
    /// [`STYLE_INDEX_OFFSET`] alone answers.
    ///
    /// Height, colour and typeface come off this style; `alignment` and
    /// `line_spacing` stay on the paragraph, which is the only place they are
    /// stated. That is the same split the drawing side draws
    /// (`style_link::ResolvedTextStyle::effective`), and the reason a label is
    /// now lettered at the size its runs say rather than the paragraph's
    /// default. See `docs/analysis/2026-08-22-run-beats-paragraph-default.md`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_style_id: Option<u32>,
    /// The label's run partition, when it is lettered in more than one style:
    /// one entry per selector-1 run, in stored order, with its character
    /// range. Empty when there is nothing to segment — a record with no run
    /// (shape 1), one run covering the whole label (shape 2), a run list that
    /// does not resolve, or a label with a non-BMP character (whose UTF-16
    /// run lengths would no longer be character offsets).
    ///
    /// [`Self::height_m`], [`Self::font_name`] and [`Self::rgb`] stay the one
    /// style a single-run renderer draws; a renderer that can advance span by
    /// span draws these instead.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spans: Vec<SymbolLetteringSpan>,
}

/// One run of a symbol label that is lettered in its own style: `len`
/// characters from `start`, counted in the record's own characters.
///
/// A `.sym` label may mix styles — `Filter Description.sym` letters
/// `9x1#23 1x1#25 4x1#23`, the pipeline tags alternate two faces for their
/// separators — and a single style slot cannot say so. Across the library 165
/// of 2866 records carry more than one run, of which 2 change height and 4
/// change typeface; the rest differ only inside a run that resolves to one
/// height and face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SymbolLetteringSpan {
    /// First character the run covers, in `char`s of the label's text.
    pub start: u16,
    /// Characters the run covers.
    pub len: u16,
    /// The `JStyleTextChar` the run names.
    pub style_id: u32,
    /// Character height in metres the run's style states.
    pub height_m: Option<f64>,
    /// Typeface the run's style names, verbatim.
    pub font_name: Option<String>,
    /// Character colour as `[r, g, b]` when the run's style states one.
    pub rgb: Option<[u8; 3]>,
}

impl SymbolLettering {
    /// The lettering a cached body's decoded `igTextBox` draws with — the
    /// record's own reads (its anchor byte, its direction pair as rotation
    /// and any stated height) joined with what its `index` resolves to
    /// through the body's own style table.
    ///
    /// This is the model-record counterpart of the byte read `lettering_of`
    /// makes for a `.sym`: the same fields, off the same `JStyleTextPara` →
    /// `JStyleTextChar` hop, only the table differs — the drawing's
    /// definition cache keeps its own `StyleCluster` per `JSite` storage.
    pub(crate) fn from_decoded(
        record: &crate::DecodedIgTextBoxRecord,
        table: Option<&DocumentStyleTable>,
    ) -> Self {
        let resolved = table.and_then(|table| {
            crate::style_link::resolve_text_style(
                table,
                record.index,
                record.text_length,
                &record.runs,
            )
        });
        let effective = resolved.as_ref().and_then(ResolvedTextStyle::effective);
        Self {
            anchor_code: record.anchor_code,
            rotation_rad: record.rotation_rad,
            letter_height_m: record.letter_height_m,
            height_m: effective.as_ref().map(|style| style.height_m),
            rgb: effective
                .as_ref()
                .and_then(crate::style_link::ResolvedTextHeight::rgb),
            alignment: effective.as_ref().and_then(|style| style.alignment),
            line_spacing: effective.as_ref().and_then(|style| style.line_spacing),
            font_name: effective.and_then(|style| style.font_name),
            run_style_id: resolved
                .as_ref()
                .and_then(|style| style.run.as_ref())
                .map(|run| run.style_id),
            spans: guarded_spans(&record.text, resolved.as_ref()),
        }
    }
}

/// The run partition of one label, refused where its character offsets do not
/// survive the trip: a non-BMP character is one `char` and two UTF-16 units,
/// so a run length counted in units would land mid-character. The corpus has
/// none; refusing the partition rather than shifting it is the honest read for
/// a label that had one.
///
/// Both routes — a `.sym`'s own payload and a drawing's cached body — go
/// through here, so neither reader can hand a renderer offsets the other one
/// would already have refused.
fn guarded_spans(text: &str, resolved: Option<&ResolvedTextStyle>) -> Vec<SymbolLetteringSpan> {
    if text.chars().count() != text.encode_utf16().count() {
        return Vec::new();
    }
    spans_of(resolved)
}

/// The run partition a resolved label carries, mapped to the library's own
/// span DTO. Empty where the label is lettered in one style.
fn spans_of(resolved: Option<&ResolvedTextStyle>) -> Vec<SymbolLetteringSpan> {
    let Some(resolved) = resolved else {
        return Vec::new();
    };
    resolved
        .spans
        .iter()
        .map(|span| SymbolLetteringSpan {
            start: span.start,
            len: span.len,
            style_id: span.style.style_id,
            height_m: Some(span.style.height_m),
            font_name: span.style.font_name.clone(),
            rgb: span.style.rgb(),
        })
        .collect()
}

/// A symbol's drawable body, as read from one `.sym` file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SymbolGeometry {
    /// Drawable primitives in the symbol's own coordinate space.
    pub primitives: Vec<StyledPrimitive>,
    /// Records the walk stepped over because they carry no shape, keyed by
    /// PSM type code. Reported rather than dropped silently so a symbol that
    /// comes out looking too sparse can be told apart from one that is.
    pub skipped_records: BTreeMap<u16, usize>,
    /// The `JSheetLayer` objects the symbol's own `PSMcluster0` declares —
    /// the table [`StyledPrimitive::sheet_layer_name`] resolves against.
    #[serde(default)]
    pub sheet_layers: Vec<SymbolSheetLayer>,
    /// Each sheet's `Top ViewFilterSet` display state — one entry per
    /// decoded set, what [`Self::displayed_primitives`] classifies the
    /// file's sheets by.
    #[serde(default)]
    pub sheet_views: Vec<SymbolSheetView>,
}

/// One `JSheetLayer` of a `.sym` file: the oid a primitive's
/// [`StyledPrimitive::sheet_layer_ref`] names, the number its sheet's view
/// filter set indexes it at, and the layer's authored name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolSheetLayer {
    /// Storage-local persistent object id the shape records name.
    pub oid: u32,
    /// The layer's number: which bit of the `0x0057` view filter bitmap
    /// states its display.
    #[serde(default)]
    pub layer_number: u32,
    /// The layer's authored name, verbatim.
    pub name: String,
}

/// The display state one of a `.sym`'s sheets gives its layers: what the
/// sheet's `0x0057 Top ViewFilterSet` states, distilled to what
/// [`SymbolGeometry::displayed_primitives`] reads.
///
/// A `.sym` keeps a view set per sheet, and across the corpus they come in
/// two shapes: the sheet the drawing caches (and `SmartPlant` draws) files
/// the supplementary layers — `Label`, `HiddenObjects`, `Heat Trace`,
/// `Jacket`, `Construction`, `Dimension` — off and `Default` on, while the
/// alternate view a body also carries (the insulated/heat-traced
/// representation, or a label's editing view) files every layer on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolSheetView {
    /// The `JSheet` oid the set governs — the `N` in the `SheetN` stream
    /// name of the primitives it filters.
    pub sheet_ref: u32,
    /// The set's display bitmap, verbatim: bit `n` set means the layer
    /// numbered `n` is shown on this sheet.
    pub display: Vec<u8>,
    /// The `(number, name)` entries the set lists — what classifies the
    /// sheet as the body's normal view versus an everything-on alternate.
    pub layers: Vec<(u16, String)>,
}

impl SymbolSheetView {
    /// Whether this sheet's set files any supplementary layer on — the mark
    /// of the alternate representation the corpus's embedded bodies never
    /// take. A set that only ever shows `Default`-family layers is the
    /// normal view; one that explicitly displays a `Label` or `Heat Trace`
    /// layer is the alternate.
    fn shows_supplementary(&self) -> bool {
        self.layers
            .iter()
            .any(|(number, name)| hidden_symbol_layer(name) && bit_is_set(&self.display, *number))
    }

    /// Whether the set displays layer number `n`; `None` past the bitmap.
    fn is_displayed(&self, layer_number: u32) -> Option<bool> {
        u16::try_from(layer_number)
            .ok()
            .map(|number| bit_is_set(&self.display, number))
    }
}

/// Bit `n` of a little-endian bitmap, `false` where the map does not reach.
fn bit_is_set(bitmap: &[u8], n: u16) -> bool {
    bitmap
        .get(usize::from(n / 8))
        .is_some_and(|byte| byte & (1 << (n % 8)) != 0)
}

/// The serde default for [`StyledPrimitive::sheet_layer_number`]: a layer
/// number no `u16` bitmap bit can name, so a body read back from before
/// the field existed filters by layer name as before.
fn unresolved_layer_number() -> u32 {
    u32::MAX
}

/// The `.sym` layer names `SmartPlant` keeps off the sheet, compared
/// whitespace- and case-insensitively (`Hidden Objects` and `HiddenObjects`
/// are the same layer; `Deminsion` is the corpus's own spelling).
///
/// These are authored conventions, not file flags: the drawing-side
/// definition caches settle the same names to `displayed: false` — the
/// corpus marks every `Heat Trace`, `HiddenObjects`, `Jacket`, `Label`,
/// `Construction`, and `Dimension`/`Deminsion` layer off and every `Default`
/// layer on — and the reference plots draw none of the hidden family. The
/// `Label` layer carrying a symbol's catalogue text (`PG`, `MT`) is hidden
/// the same way the cached copies hide it: the sheet, not the body, letters
/// the instrument tag.
const HIDDEN_SYMBOL_LAYER_NAMES: &[&str] = &[
    "heattrace",
    "hiddenobjects",
    "jacket",
    "label",
    "construction",
    "dimension",
    "deminsion",
];

/// Whether `sheet_ref`'s own view set files the supplementary layers off —
/// the sheet `displayed_primitives` takes for the body's normal view.
fn is_normal_sheet(views: &[SymbolSheetView], sheet_ref: u32) -> bool {
    views
        .iter()
        .find(|view| view.sheet_ref == sheet_ref)
        .is_some_and(|view| !view.shows_supplementary())
}

/// Whether `name` belongs to the layer family a `.sym` keeps for itself
/// rather than the sheet.
fn hidden_symbol_layer(name: &str) -> bool {
    let normalized: String = name
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    HIDDEN_SYMBOL_LAYER_NAMES.contains(&normalized.as_str())
}

impl SymbolGeometry {
    /// Axis-aligned bounds of the body as `(min_x, min_y, max_x, max_y)`,
    /// or `None` when there is nothing to draw.
    ///
    /// Every primitive counts, hidden layers included, and with them the
    /// label templates a `.sym` parks well away from its body: across the
    /// library 85 bodies reach more than twice as far as what
    /// [`Self::displayed_primitives`] draws, 49 of them through lettering
    /// alone. Bound the displayed primitives for the extent that prints.
    ///
    /// Arcs contribute their endpoints and centre rather than a swept
    /// extent, which can under-report a bulge; that is enough for the
    /// placement-level framing this feeds and avoids a quadrant walk.
    pub fn bounds(&self) -> Option<(f64, f64, f64, f64)> {
        let mut acc: Option<(f64, f64, f64, f64)> = None;
        let mut add = |x: f64, y: f64| {
            acc = Some(match acc {
                None => (x, y, x, y),
                Some((min_x, min_y, max_x, max_y)) => {
                    (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y))
                }
            });
        };
        for styled in &self.primitives {
            match &styled.primitive {
                SymbolPrimitive::Line { start, end } => {
                    add(start.0, start.1);
                    add(end.0, end.1);
                }
                SymbolPrimitive::Circle { center, radius } => {
                    add(center.0 - radius, center.1 - radius);
                    add(center.0 + radius, center.1 + radius);
                }
                SymbolPrimitive::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                    ..
                } => {
                    add(center.0, center.1);
                    for angle in [*start_angle, *end_angle] {
                        add(
                            center.0 + radius * angle.cos(),
                            center.1 + radius * angle.sin(),
                        );
                    }
                }
                SymbolPrimitive::Polyline { vertices, .. } => {
                    for (x, y) in vertices {
                        add(*x, *y);
                    }
                }
                // The control polygon bounds the curve (convex hull
                // property), so the poles are a safe over-estimate.
                SymbolPrimitive::BSpline { poles, .. } => {
                    for (x, y) in poles {
                        add(*x, *y);
                    }
                }
                // The insertion point only. How far the lettering actually
                // reaches needs a height and a font, and the record carries
                // neither.
                SymbolPrimitive::Text { at, .. } => add(at.0, at.1),
                // The members' own extents — a boundary fills exactly what
                // its members outline.
                SymbolPrimitive::Boundary { members, .. } => {
                    for member in members {
                        match member {
                            SymbolPrimitive::Line { start, end } => {
                                add(start.0, start.1);
                                add(end.0, end.1);
                            }
                            SymbolPrimitive::Circle { center, radius } => {
                                add(center.0 - radius, center.1 - radius);
                                add(center.0 + radius, center.1 + radius);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        acc
    }

    /// The strokes `SmartPlant` puts on the sheet for this body: the
    /// primitives of the sheet the file presents as the body's normal
    /// view, on the layers that view displays.
    ///
    /// Two selections stack. The first picks **which sheets** draw: a
    /// `.sym` commonly keeps the same body twice — the sheet whose
    /// `0x0057 Top ViewFilterSet` files the supplementary layers off (the
    /// normal view the drawing's embedded caches mirror bit for bit) and
    /// an alternate view whose set files every layer on (the
    /// insulated/heat-traced representation). Where view sets decode, the
    /// normal sheets are the ones that do not show a supplementary layer
    /// **and keep at least one stroke** once their own set has had its say;
    /// where no sheet qualifies, the sheet that keeps the most wins, ties to
    /// the lower number; where no sheet keeps anything, the body comes back
    /// whole.
    ///
    /// The second picks **which layers** of a selected sheet draw: the
    /// sheet's own view set answers at the layer's number where the bitmap
    /// reaches it; where a layer has no number, or the set no bit, or the
    /// sheet no set at all, the drawing-side convention answers — the
    /// embedded caches file `Label`, `HiddenObjects`, `Heat Trace`,
    /// `Jacket`, `Construction`, and `Dimension`/`Deminsion` off and
    /// `Default`, `ConnectPoints`, and `Invisible` on.
    ///
    /// A clean set is not proof the body is on its sheet: it survives as the
    /// canonical `Default`-only map on `Sheet6` of symbols whose body was
    /// revised onto a second sheet, which then carries either no view set or
    /// the everything-on map. A body merely copied stays with the clean sheet
    /// ([`read_symbol_geometry`] keeps a shared stroke there), yet 206 of the
    /// corpus's 868 clean sets still hide every stroke they govern, so the
    /// pick requires a stroke. See
    /// `docs/analysis/2026-10-22-the-normal-view-that-drew-nothing.md`.
    ///
    /// Where **no** sheet has a drawing normal view, the body is the sheet
    /// that keeps the most strokes once its own set has had its say, not
    /// `Sheet6`. A
    /// `.sym` can keep its body only once — on a sheet whose view set files
    /// every layer on, which this classification reads as the alternate — and
    /// its `Sheet6` can then be a one-stroke stub: `Wellhead Christmas Tree
    /// 02.sym` files 83 strokes on `Sheet374` and one B-spline on `Sheet6`,
    /// and choosing `Sheet6` drew a single curve where the drawing's own
    /// cached body (45 lines, 23 circles, 11 fills) is what the vendor
    /// printed. Across 1262 library symbols, 530 bodies lose strokes to the
    /// sheet pick; the drawings' cached bodies match the fattest sheet on all
    /// 372 places that have no normal view, and the previous `Sheet6` rule on
    /// 36 % of them (`docs/analysis/2026-10-22-the-sheet-pick-that-shrank-
    /// a-symbol.md`).
    ///
    /// A body the filter would empty is returned whole instead: a symbol
    /// authored entirely on an unconventional name keeps its strokes
    /// rather than vanishing into a marker dot.
    #[must_use]
    pub fn displayed_primitives(&self) -> Vec<&StyledPrimitive> {
        let view_of = |sheet_ref: u32| {
            self.sheet_views
                .iter()
                .find(|view| view.sheet_ref == sheet_ref)
        };
        let sheet_of = |styled: &StyledPrimitive| styled.sheet_ref;
        let sheets_present: BTreeSet<u32> = self
            .primitives
            .iter()
            .map(sheet_of)
            .chain(self.sheet_views.iter().map(|view| view.sheet_ref))
            .collect();
        let normal: BTreeSet<u32> = sheets_present
            .iter()
            .copied()
            .filter(|sheet| view_of(*sheet).is_some_and(|view| !view.shows_supplementary()))
            .collect();
        let layer_shown = |styled: &&StyledPrimitive| {
            if styled.sheet_layer_ref == 0 {
                return true;
            }
            if let Some(answer) = view_of(styled.sheet_ref)
                .and_then(|view| view.is_displayed(styled.sheet_layer_number))
            {
                if !answer {
                    return false;
                }
            }
            styled
                .sheet_layer_name
                .as_deref()
                .is_none_or(|name| !hidden_symbol_layer(name))
        };

        // With no normal view the body is the sheet that keeps the most
        // strokes, not `Sheet6`: a body kept once on an all-on sheet leaves
        // `Sheet6` as a stub. Ties keep the lower sheet number.
        let fattest = {
            let mut best: Option<(u32, usize)> = None;
            for sheet in &sheets_present {
                let strokes = self
                    .primitives
                    .iter()
                    .filter(|styled| styled.sheet_ref == *sheet)
                    .filter(|styled| layer_shown(styled))
                    .count();
                if best.is_none_or(|(_, most)| strokes > most) {
                    best = Some((*sheet, strokes));
                }
            }
            best.filter(|(_, strokes)| *strokes > 0)
                .map(|(sheet, _)| sheet)
        };

        // A clean set is only a claim that the body is on its sheet: across
        // the corpus 206 of the 868 symbols that have one draw nothing once
        // that set has had its say, and 61 govern a sheet holding no
        // primitives at all. Keeping such a pick would either empty the body
        // (and hand it to the whole-body fallback, which pulls the other
        // sheet's strokes in) or ship a one-line stub of a body filed
        // elsewhere, so a normal sheet that draws nothing is not a pick.
        let drawing: BTreeSet<u32> = normal
            .iter()
            .copied()
            .filter(|sheet| {
                self.primitives
                    .iter()
                    .any(|styled| styled.sheet_ref == *sheet && layer_shown(&styled))
            })
            .collect();

        let selected: BTreeSet<u32> = if !drawing.is_empty() {
            drawing
        } else if let Some(sheet) = fattest {
            [sheet].into_iter().collect()
        } else {
            // `fattest` is `None` only where no sheet shows a primitive, so
            // no pick could show one either.
            return self.primitives.iter().collect();
        };

        let shown =
            |styled: &&StyledPrimitive| selected.contains(&styled.sheet_ref) && layer_shown(styled);
        let visible: Vec<&StyledPrimitive> = self.primitives.iter().filter(|s| shown(s)).collect();
        if visible.is_empty() {
            self.primitives.iter().collect()
        } else {
            visible
        }
    }
}

fn u16_le(bytes: &[u8], at: usize) -> Option<u16> {
    bytes
        .get(at..at + 2)
        .and_then(|s| <[u8; 2]>::try_from(s).ok())
        .map(u16::from_le_bytes)
}

fn u32_le(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..at + 4)
        .and_then(|s| <[u8; 4]>::try_from(s).ok())
        .map(u32::from_le_bytes)
}

fn f64_le(bytes: &[u8], at: usize) -> Option<f64> {
    bytes
        .get(at..at + 8)
        .and_then(|s| <[u8; 8]>::try_from(s).ok())
        .map(f64::from_le_bytes)
}

/// A finite coordinate inside the plausible symbol extent.
fn coordinate(bytes: &[u8], at: usize) -> Option<f64> {
    let value = f64_le(bytes, at)?;
    (value.is_finite() && value.abs() <= COORDINATE_LIMIT).then_some(value)
}

/// Read a text record's characters and insertion point.
///
/// Which shape the record is — and therefore where its count, its text and its
/// tail sit — is settled by the discriminator at [`TEXT_SHAPE_AT`] through
/// [`igtextbox_body_shape`], the table the drawing side uses: a `.sym` stores
/// `0x004D` payloads with the identical layout (22-byte header, body, 36-byte
/// tail), so both readers ask the same table. The earlier rule — try `+30` and
/// `+22`, believe whichever leaves exactly 36 bytes behind the characters —
/// described the two shapes whose count happens to live at those offsets and
/// refused shape 3 wholesale.
///
/// A record is read only where its tail's direction pair is a direction, by
/// the rule the drawing side refuses on (`igtextbox_letter_height`): the
/// layout leaves room for fields after the tail, so the pair is what checks
/// the count.
fn decode_text(payload: &[u8]) -> Option<SymbolPrimitive> {
    let (count, text_start, tail_start) = text_layout(payload)?;
    let cos = f64_le(payload, tail_start + 16)?;
    let sin = f64_le(payload, tail_start + 24)?;
    igtextbox_letter_height(cos, sin)?;
    let mut chars = Vec::with_capacity(count);
    for index in 0..count {
        chars.push(u16_le(payload, text_start + index * 2)?);
    }
    let text = String::from_utf16(&chars).ok()?;
    // An unpaired surrogate is caught above; a NUL means the run is not
    // really characters, whatever the arithmetic says.
    if text.contains('\0') {
        return None;
    }
    // The insertion point opens the tail. On shapes 1 and 2 that is the end
    // of the text; on shape 3 the `A + B` formatting runs come first, which is
    // why reading `text_end` there lands on denormal noise.
    let at = (
        coordinate(payload, tail_start)?,
        coordinate(payload, tail_start + 8)?,
    );
    Some(SymbolPrimitive::Text { text, at })
}

/// Where a text record's characters and tail sit: `(count, text_start,
/// tail_start)`, all payload-relative.
///
/// Every shape derives its body length from its own count, so the characters
/// always end inside the body; what a wrong count moves is the tail. What
/// follows the 36-byte placement tail is then held to the tail kind at `+20`
/// (`tail_kind_accounts_for`): across the library all 2321 kind-1 records end
/// with the placement tail, and all 543 kind-2 records carry the 32-byte
/// text-box tail plus 0–24 bytes of fields. `decode_text` checks the count
/// once more where the tail lands.
fn text_layout(payload: &[u8]) -> Option<(usize, usize, usize)> {
    let shape = u16_le(payload, TEXT_SHAPE_AT)?;
    let (count, text_start, body_len) = igtextbox_body_shape(payload, shape)?;
    let count = usize::from(count);
    if count == 0 || count > TEXT_MAX_CHARS {
        return None;
    }
    let tail_start = IGTEXTBOX_BODY_START.checked_add(body_len)?;
    let tail_end = tail_start.checked_add(TEXT_TRAILING_LEN)?;
    let after_tail = payload.len().checked_sub(tail_end)?;
    if !tail_kind_accounts_for(u16_le(payload, IGTEXTBOX_TAIL_KIND_AT)?, after_tail) {
        return None;
    }
    Some((count, text_start, tail_start))
}

/// Whether `after_tail` — the bytes past the 36-byte placement tail — is what
/// the tail kind says comes next: nothing for the placement tail alone, and
/// for the text-box tail its 32 fixed bytes (flag word, box, three counters)
/// followed by whole 8-byte fields. A kind this reader has not seen keeps the
/// rule that the tail only has to fit.
fn tail_kind_accounts_for(kind: u16, after_tail: usize) -> bool {
    let box_extra = IGTEXTBOX_TAIL_KIND2_LEN - IGTEXTBOX_TAIL_LEN;
    match kind {
        IGTEXTBOX_TAIL_KIND_PLACEMENT => after_tail == 0,
        IGTEXTBOX_TAIL_KIND_BOX => after_tail
            .checked_sub(box_extra)
            .is_some_and(|fields| fields.is_multiple_of(8)),
        _ => true,
    }
}

/// Turn one record payload into a primitive, or `None` when the record
/// carries no shape or fails a domain check.
fn decode_primitive(type_code: u16, payload: &[u8]) -> Option<SymbolPrimitive> {
    match type_code {
        TYPE_LINE if payload.len() >= LINE_PAYLOAD_LEN => {
            let start = (
                coordinate(payload, PAYLOAD_HEADER_LEN)?,
                coordinate(payload, PAYLOAD_HEADER_LEN + 8)?,
            );
            let end = (
                coordinate(payload, PAYLOAD_HEADER_LEN + 16)?,
                coordinate(payload, PAYLOAD_HEADER_LEN + 24)?,
            );
            // A zero-length segment draws nothing and only widens bounds.
            (start != end).then_some(SymbolPrimitive::Line { start, end })
        }
        TYPE_CIRCLE if payload.len() >= CIRCLE_PAYLOAD_LEN => {
            let center = (
                coordinate(payload, PAYLOAD_HEADER_LEN)?,
                coordinate(payload, PAYLOAD_HEADER_LEN + 8)?,
            );
            let radius = coordinate(payload, PAYLOAD_HEADER_LEN + 16)?;
            (radius > 0.0).then_some(SymbolPrimitive::Circle { center, radius })
        }
        TYPE_ARC if payload.len() >= ARC_PAYLOAD_LEN => {
            let center = (
                coordinate(payload, PAYLOAD_HEADER_LEN)?,
                coordinate(payload, PAYLOAD_HEADER_LEN + 8)?,
            );
            let radius = coordinate(payload, PAYLOAD_HEADER_LEN + 16)?;
            let start_angle = f64_le(payload, PAYLOAD_HEADER_LEN + 24)?;
            let end_angle = f64_le(payload, PAYLOAD_HEADER_LEN + 32)?;
            if radius <= 0.0 || !start_angle.is_finite() || !end_angle.is_finite() {
                return None;
            }
            // The trailing byte names the direction, `0` clockwise: the two
            // `90° -> 270°` records of the ultrasonic wave differ only here.
            let clockwise = *payload.get(ARC_PAYLOAD_LEN - 1)? == 0;
            Some(SymbolPrimitive::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                clockwise,
            })
        }
        TYPE_BSPLINE => {
            // The same payload shape a drawing's record has, validated the
            // same way; a curve whose poles leave the symbol extent is not a
            // symbol's curve.
            let curve = crate::parsers::sheet_records::bspcurve_geometry(payload)?;
            if curve
                .poles
                .iter()
                .any(|(x, y)| x.abs() > COORDINATE_LIMIT || y.abs() > COORDINATE_LIMIT)
            {
                return None;
            }
            Some(SymbolPrimitive::BSpline {
                poles: curve.poles,
                weights: curve.weights,
                knots: curve.knots,
            })
        }
        TYPE_POLYLINE if payload.len() > POLYLINE_VERTEX_START => {
            let span = payload.len() - POLYLINE_VERTEX_START;
            if !span.is_multiple_of(16) {
                return None;
            }
            let declared = u32_le(payload, PAYLOAD_HEADER_LEN)? as usize;
            let count = span / 16;
            if count != declared || !(2..=POLYLINE_MAX_VERTICES).contains(&count) {
                return None;
            }
            let mut vertices = Vec::with_capacity(count);
            for index in 0..count {
                let at = POLYLINE_VERTEX_START + index * 16;
                vertices.push((coordinate(payload, at)?, coordinate(payload, at + 8)?));
            }
            Some(SymbolPrimitive::Polyline {
                vertices,
                is_closed: payload[22] == 2,
            })
        }
        TYPE_TEXT => decode_text(payload),
        TYPE_BOUNDARY => {
            use crate::parsers::sheet_records::{decode_igsym_boundary_payload, IgBoundaryMember};
            let boundary = decode_igsym_boundary_payload(payload)?;
            let members = boundary
                .members
                .iter()
                .map(|member| match member {
                    IgBoundaryMember::Line { start, end } => SymbolPrimitive::Line {
                        start: *start,
                        end: *end,
                    },
                    IgBoundaryMember::Circle { center, radius } => SymbolPrimitive::Circle {
                        center: *center,
                        radius: *radius,
                    },
                })
                .collect();
            Some(SymbolPrimitive::Boundary {
                members,
                fill: None,
            })
        }
        _ => None,
    }
}

/// The fill a boundary record's own style index names, if the symbol's
/// style table defines one — the same hop [`style_of`] makes for a line
/// style, through [`crate::style_link::DocumentStyleTable::resolve_fill`]
/// instead.
fn fill_of(payload: &[u8], styles: Option<&DocumentStyleTable>) -> Option<PrimitiveFill> {
    let table = styles?;
    let index = u32_le(payload, STYLE_INDEX_OFFSET)?;
    table
        .resolve_fill(index)
        .as_ref()
        .map(PrimitiveFill::from_resolved)
}

/// Dedup key for one boundary member: its shape quantised to nanometres
/// plus the fill colour the record states, so overlapping records that name
/// the same region in the same colour draw it once.
pub(crate) fn boundary_member_key(member: &SymbolPrimitive, fill: Option<PrimitiveFill>) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    let coords: &[f64] = match member {
        SymbolPrimitive::Line { start, end } => &[start.0, start.1, end.0, end.1],
        SymbolPrimitive::Circle { center, radius } => &[center.0, center.1, *radius],
        _ => &[],
    };
    for v in coords {
        ((v * 1e9).round() as i64).hash(&mut hasher);
    }
    std::mem::discriminant(member).hash(&mut hasher);
    fill.and_then(|fill| fill.rgb).hash(&mut hasher);
    hasher.finish()
}

/// Walk one symbol `Sheet*` stream's record chain into `out`.
///
/// The chain is only read when it accounts for the stream exactly: a walk
/// that stops early or overruns means the bytes are not the layout this
/// reader knows, and half a symbol drawn from a misread chain is worse than
/// no symbol at all.
fn read_sheet_chain(
    bytes: &[u8],
    sheet_ref: u32,
    styles: Option<&DocumentStyleTable>,
    layers: Option<&[SymbolSheetLayer]>,
    out: &mut SymbolGeometry,
) {
    if u32_le(bytes, 0) != Some(CLUSTER_MAGIC) {
        return;
    }
    let layer = |reference: u32| {
        layers.and_then(|layers| layers.iter().find(|layer| layer.oid == reference))
    };
    let mut primitives = Vec::new();
    let mut skipped: BTreeMap<u16, usize> = BTreeMap::new();
    // Boundary records can overlap: the plug-valve bodies store two records
    // covering the same disc, and either one fills it identically. A member
    // shape another boundary already claimed is dropped so the disc draws
    // once, the way the plots draw it.
    let mut seen_boundary_members: std::collections::HashSet<u64> =
        std::collections::HashSet::new();
    let mut at = CHAIN_START;
    while at + PSM_ENVELOPE_LEN <= bytes.len() {
        let (Some(raw_type), Some(length)) = (u16_le(bytes, at), u32_le(bytes, at + 2)) else {
            return;
        };
        if raw_type == 0 {
            return;
        }
        let body = at + PSM_ENVELOPE_LEN;
        let Some(end) = body.checked_add(length as usize) else {
            return;
        };
        if end > bytes.len() {
            return;
        }
        let type_code = raw_type & 0x0FFF;
        let payload = &bytes[body..end];
        match decode_primitive(type_code, payload) {
            Some(mut primitive) => {
                // A boundary's style index names a fill, not a line —
                // resolve it through the same slot and keep it on the
                // primitive itself, since `StyledPrimitive::style` is the
                // line symbology slot.
                if let SymbolPrimitive::Boundary { fill, .. } = &mut primitive {
                    *fill = fill_of(payload, styles);
                }
                if let SymbolPrimitive::Boundary { members, fill } = &mut primitive {
                    members.retain(|member| {
                        seen_boundary_members.insert(boundary_member_key(member, *fill))
                    });
                    if members.is_empty() {
                        at = end;
                        continue;
                    }
                }
                let lettering = lettering_of(&primitive, payload, styles);
                // The sub-header slot every shape family carries its
                // `JSheetLayer` oid in — the same `payload[8..12]` read the
                // drawing-side decoders make.
                let sheet_layer_ref = u32_le(payload, 8).unwrap_or(0);
                let found = (sheet_layer_ref != 0)
                    .then(|| layer(sheet_layer_ref))
                    .flatten();
                primitives.push(StyledPrimitive {
                    primitive,
                    style: style_of(payload, styles),
                    lettering,
                    sheet_layer_ref,
                    sheet_layer_name: found.map(|layer| layer.name.clone()),
                    sheet_layer_number: found
                        .map_or_else(unresolved_layer_number, |layer| layer.layer_number),
                    sheet_ref,
                });
            }
            None => *skipped.entry(type_code).or_default() += 1,
        }
        at = end;
    }
    if at != bytes.len() {
        return;
    }
    out.primitives.append(&mut primitives);
    for (code, count) in skipped {
        *out.skipped_records.entry(code).or_default() += count;
    }
}

/// The symbology a record's own style index names, if the symbol's style
/// table defines it as a line style.
///
/// The index sits at [`STYLE_INDEX_OFFSET`] of the payload, the same slot the
/// drawing-side families carry it in — settled for those in
/// `docs/analysis/2026-08-05-geometry-index-is-the-style-link.md`.
fn style_of(payload: &[u8], styles: Option<&DocumentStyleTable>) -> Option<PrimitiveStyle> {
    let table = styles?;
    let index = u32_le(payload, STYLE_INDEX_OFFSET)?;
    let resolved = table.resolve_line_style(index)?;
    Some(PrimitiveStyle::from_resolved(&resolved))
}

/// The lettering a text record draws with: what its placement tail states
/// joined with what its **runs** say, and its paragraph index only where it
/// says nothing.
///
/// `Some` for every [`SymbolPrimitive::Text`] `decode_text` read from the same
/// payload — the tail is the record's own bytes and reads even with no table
/// to ask — and `None` for every other shape. The tail starts where the
/// shape's body ends (`text_layout`), not necessarily at `payload_len - 36`,
/// so the same reads the sheet decoder makes apply: direction pair at
/// `+16..32` of the tail, anchor at its last byte.
///
/// The style is [`text_style`]'s: a run's character style where the record has
/// a run that resolves, the paragraph default at [`STYLE_INDEX_OFFSET`]
/// otherwise — the rule the drawing side applies, not the paragraph-only hop
/// this reader used to make.
fn lettering_of(
    primitive: &SymbolPrimitive,
    payload: &[u8],
    styles: Option<&DocumentStyleTable>,
) -> Option<SymbolLettering> {
    if !matches!(primitive, SymbolPrimitive::Text { .. }) {
        return None;
    }
    let (count, _, tail) = text_layout(payload)?;
    let cos = f64_le(payload, tail + 16)?;
    let sin = f64_le(payload, tail + 24)?;
    // A unit pair is a direction only and states no height; a height-scaled
    // one states it. `decode_text` admits no other pair.
    let letter_height_m = igtextbox_letter_height(cos, sin)?;
    let resolved = text_style(payload, count, styles);
    let effective = resolved.as_ref().and_then(ResolvedTextStyle::effective);
    Some(SymbolLettering {
        anchor_code: payload
            .get(tail + TEXT_TRAILING_LEN - 1)
            .copied()
            .unwrap_or_default(),
        rotation_rad: sin.atan2(cos),
        letter_height_m,
        height_m: effective.as_ref().map(|style| style.height_m),
        rgb: effective
            .as_ref()
            .and_then(crate::style_link::ResolvedTextHeight::rgb),
        alignment: effective.as_ref().and_then(|style| style.alignment),
        line_spacing: effective.as_ref().and_then(|style| style.line_spacing),
        font_name: effective.and_then(|style| style.font_name),
        run_style_id: resolved
            .as_ref()
            .and_then(|style| style.run.as_ref())
            .map(|run| run.style_id),
        spans: text_spans(primitive, resolved.as_ref()),
    })
}

/// [`guarded_spans`] for a primitive: only a text record has characters to
/// measure, and every other shape carries no partition.
fn text_spans(
    primitive: &SymbolPrimitive,
    resolved: Option<&ResolvedTextStyle>,
) -> Vec<SymbolLetteringSpan> {
    let SymbolPrimitive::Text { text, .. } = primitive else {
        return Vec::new();
    };
    guarded_spans(text, resolved)
}

/// A text record's lettering from both places it states it: its paragraph
/// index at [`STYLE_INDEX_OFFSET`], and the formatting runs its shape stores.
///
/// One rule, shared with the drawing side
/// (`style_link::resolve_text_style`): the run covering the most characters
/// wins for height, colour and typeface, while `alignment` and `line_spacing`
/// come off the paragraph, which is the only place they are stated. The
/// library used to skip the runs entirely, which lettered 1350 of 2266 labels
/// at the paragraph's default size instead of the size their runs state.
///
/// `None` with no table to ask: a `.sym` knows its own states and nothing
/// else.
fn text_style(
    payload: &[u8],
    count: usize,
    styles: Option<&DocumentStyleTable>,
) -> Option<ResolvedTextStyle> {
    let table = styles?;
    let index = u32_le(payload, STYLE_INDEX_OFFSET)?;
    let count = u16::try_from(count).ok()?;
    crate::style_link::resolve_text_style(table, index, count, &text_runs(payload, count))
}

/// The formatting runs a text record stores, read by the drawing side's own
/// walker rather than a second one: `igtextbox_runs` knows the shape-2 entry
/// ahead of the count and the shape-3 entries after the text, and asking it
/// here is what keeps the two readers from drifting apart again.
fn text_runs(payload: &[u8], count: u16) -> Vec<DecodedTextRun> {
    let Some(shape) = u16_le(payload, TEXT_SHAPE_AT) else {
        return Vec::new();
    };
    igtextbox_runs(payload, shape, count)
        .into_iter()
        .map(DecodedTextRun::from)
        .collect()
}

/// The storage a stream lives in, which is what a `Sheet*` and the
/// `StyleCluster` that styles it have in common.
fn parent_storage(stream_path: &str) -> &str {
    stream_path.rfind('/').map_or("", |cut| &stream_path[..cut])
}

/// Read a `.sym` file's drawable body.
///
/// # Errors
///
/// Returns the underlying [`PidError`] when the file cannot be opened or is
/// not a readable compound file. A `.sym` that opens but holds no geometry
/// yields an empty [`SymbolGeometry`] rather than an error — an empty symbol
/// is a real thing in the library.
pub fn read_symbol_geometry(path: &Path) -> Result<SymbolGeometry, PidError> {
    let package = PidParser::new().parse_package(path)?;
    // Styles are scoped to the storage they sit in, the same rule
    // `style_link` applies to a drawing: a sheet is resolved against its own
    // document's table and never against another's.
    let mut tables: HashMap<&str, DocumentStyleTable> = HashMap::new();
    let mut layer_tables: HashMap<&str, Vec<SymbolSheetLayer>> = HashMap::new();
    for (stream_path, raw) in &package.streams {
        match stream_path.rsplit('/').next() {
            Some("StyleCluster") => {
                tables.insert(
                    parent_storage(stream_path),
                    DocumentStyleTable::from_stylecluster_bytes(&raw.data),
                );
            }
            Some("PSMcluster0") => {
                let layers: Vec<SymbolSheetLayer> =
                    crate::parsers::sheet_layers::decode_sheet_layers(&raw.data)
                        .into_iter()
                        .map(|layer| SymbolSheetLayer {
                            oid: layer.oid,
                            layer_number: layer.layer_number,
                            name: layer.name,
                        })
                        .collect();
                layer_tables.insert(parent_storage(stream_path), layers);
            }
            _ => {}
        }
    }

    let mut geometry = SymbolGeometry {
        sheet_layers: layer_tables.values().flatten().cloned().collect(),
        ..SymbolGeometry::default()
    };
    for (stream_path, raw) in &package.streams {
        if stream_path.rsplit('/').next() == Some("PSMcluster0") {
            for set in crate::parsers::view_filter_sets::decode_view_filter_sets(&raw.data) {
                geometry.sheet_views.push(SymbolSheetView {
                    sheet_ref: set.sheet_ref,
                    display: set.display,
                    layers: set
                        .layers
                        .iter()
                        .map(|entry| (entry.layer_number, entry.name.clone()))
                        .collect(),
                });
            }
        }
    }

    let mut sheets: Vec<(&str, u32, &[u8])> = Vec::new();
    for (stream_path, raw) in &package.streams {
        let name = stream_path.rsplit('/').next().unwrap_or("");
        let is_sheet = name.starts_with("Sheet");
        if !is_sheet || raw.data.len() <= CHAIN_START {
            continue;
        }
        // The stream name carries the JSheet oid (`Sheet68` is sheet 68) —
        // the same id a `0x0057` view filter set's `sheet_ref` names. A
        // name that does not parse reads as sheet 0, an id no corpus set
        // governs, so its records filter as a sheet with no view state.
        let sheet_ref = name
            .strip_prefix("Sheet")
            .and_then(|number| number.parse::<u32>().ok())
            .unwrap_or(0);
        sheets.push((stream_path.as_str(), sheet_ref, raw.data.as_slice()));
    }
    // The sheets the symbol presents as its normal view are read first, so a
    // stroke two sheets share survives on the normal one. In stream order a
    // twin can sort ahead (`Sheet46` before `Sheet6`) and take the whole body,
    // leaving the normal sheet its lettering alone — which is then all
    // `displayed_primitives` draws.
    sheets.sort_by_key(|&(_, sheet_ref, _)| !is_normal_sheet(&geometry.sheet_views, sheet_ref));

    for (stream_path, sheet_ref, data) in sheets {
        let mut sheet = SymbolGeometry::default();
        read_sheet_chain(
            data,
            sheet_ref,
            tables.get(parent_storage(stream_path)),
            layer_tables
                .get(parent_storage(stream_path))
                .map(Vec::as_slice),
            &mut sheet,
        );

        // A symbol often keeps more than one sheet holding the same body:
        // the angle globe valve's `Sheet63` is nine primitives that are
        // byte-identical to nine of `Sheet6`'s eleven. Drawing both doubles
        // every stroke. Duplicates are dropped across sheets but kept within
        // one, where a repeated stroke is the symbol's own doing. The shape
        // decides that, not the symbology: the same stroke twice in two
        // colours is still the same stroke twice.
        for styled in sheet.primitives {
            if !geometry
                .primitives
                .iter()
                .any(|kept| kept.primitive == styled.primitive)
            {
                geometry.primitives.push(styled);
            }
        }
        for (code, count) in sheet.skipped_records {
            *geometry.skipped_records.entry(code).or_default() += count;
        }
    }
    Ok(geometry)
}

/// The library-relative part of a placement's symbol path.
///
/// Placements carry a UNC path into whichever reference share the project
/// was drawn against — `\\WIN-SPID\qsmcqtaz13\Plant\Ref\Symbols\...`,
/// `\\SPID\XaLNG\P&ID Reference Data\Symbols\...`, `\\MM-128\...` — and the
/// same symbol is named through different shares in different drawings. The
/// part after the `Symbols` directory is the part they agree on, and it is
/// what a local copy of the library is laid out by.
///
/// Returns `None` when the path has no `Symbols` component to split on.
#[must_use]
pub fn library_relative_path(symbol_path: &str) -> Option<&str> {
    let normalized = symbol_path.replace('/', r"\").to_ascii_lowercase();
    let at = normalized.find(LIBRARY_MARKER)?;
    let tail = at + LIBRARY_MARKER.len();
    (tail < symbol_path.len()).then(|| symbol_path[tail..].trim())
}

/// A local copy of a `SmartPlant` symbol library, resolving the UNC paths
/// carried by placements to the bodies on disk.
///
/// More than one root is allowed, searched in order. Drawings from different
/// projects name different reference shares — the fixtures here cite five —
/// and a local machine usually holds several partial copies rather than one
/// merged tree, so a single root leaves whichever project it did not come
/// from undrawn.
///
/// Lookups are cached, including misses: a drawing places the same handful
/// of symbols dozens of times, and a project that lacks a symbol lacks it
/// for every placement.
#[derive(Debug)]
pub struct SymbolLibrary {
    roots: Vec<PathBuf>,
    cache: BTreeMap<String, Option<SymbolGeometry>>,
}

impl SymbolLibrary {
    /// Point the library at the directory that holds the `Design`,
    /// `Piping`, `Equipment` … trees.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            roots: vec![root.into()],
            cache: BTreeMap::new(),
        }
    }

    /// Point the library at several such directories, searched in order.
    pub fn with_roots<P: Into<PathBuf>>(roots: impl IntoIterator<Item = P>) -> Self {
        Self {
            roots: roots.into_iter().map(Into::into).collect(),
            cache: BTreeMap::new(),
        }
    }

    /// Build a library from a `PATH`-style list of roots — `;` separated on
    /// Windows, `:` elsewhere — in search order.
    pub fn from_path_list(list: &OsStr) -> Self {
        Self::with_roots(std::env::split_paths(list))
    }

    /// The directories this library reads from, in search order.
    #[must_use]
    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// Number of distinct symbol paths looked up so far, resolved or not.
    #[must_use]
    pub fn lookups(&self) -> usize {
        self.cache.len()
    }

    /// Number of distinct symbol paths that resolved to a body.
    #[must_use]
    pub fn resolved(&self) -> usize {
        self.cache.values().filter(|v| v.is_some()).count()
    }

    /// The symbol paths that had no body in this library, in sorted order.
    ///
    /// A miss is normally a symbol from a reference share the local copy was
    /// not taken from, not a decode failure — worth reporting so the gap is
    /// visible.
    #[must_use]
    pub fn missing(&self) -> Vec<&str> {
        self.cache
            .iter()
            .filter(|(_, body)| body.is_none())
            .map(|(path, _)| path.as_str())
            .collect()
    }

    /// Resolve a placement's symbol path to its body.
    ///
    /// Returns `None` when the path is not library-relative, or when no root
    /// holds a readable copy. A symbol that reads but holds no geometry
    /// resolves to an empty body, which callers can tell from a miss.
    pub fn resolve(&mut self, symbol_path: &str) -> Option<&SymbolGeometry> {
        if !self.cache.contains_key(symbol_path) {
            let body = self.read_from_roots(symbol_path);
            self.cache.insert(symbol_path.to_owned(), body);
        }
        self.cache.get(symbol_path)?.as_ref()
    }

    /// Read a symbol from the first root that draws it.
    ///
    /// A root can hold a symbol as a text-only or connect-point body with
    /// nothing to draw while another root has the drawn version, so an empty
    /// body is kept only as a fallback rather than ending the search.
    ///
    /// A stored path can carry a leading space on a component that the
    /// exported share does not have on disk: the plant backup's `集注站`
    /// drawings name `\Symbols\Piping\Piping OPC's\ Off-Unit SQ.sym` while
    /// the library file is `Off-Unit SQ.sym` (43 placements across 43
    /// drawings, plus 21 for `Filter Type 2.sym`). The stored spelling is
    /// tried first, then the same path with each component's leading spaces
    /// trimmed.
    fn read_from_roots(&self, symbol_path: &str) -> Option<SymbolGeometry> {
        let relative = library_relative_path(symbol_path)?.replace('\\', "/");
        let mut empty = None;
        for candidate in symbol_candidates(&relative) {
            for root in &self.roots {
                let path = root.join(&candidate);
                if !path.is_file() {
                    continue;
                }
                let Ok(body) = read_symbol_geometry(&path) else {
                    continue;
                };
                if !body.primitives.is_empty() {
                    return Some(body);
                }
                empty.get_or_insert(body);
            }
        }
        empty
    }
}

/// The library-relative spellings a stored symbol path may have on disk.
///
/// `SmartPlant` writes the path it was authored with; a share copied to
/// another machine can hold the same file without the stray leading space on
/// a component. One candidate when nothing needed trimming.
fn symbol_candidates(relative: &str) -> Vec<String> {
    let trimmed = relative
        .split('/')
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("/");
    if trimmed == relative {
        vec![relative.to_string()]
    } else {
        vec![relative.to_string(), trimmed]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbols_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-file")
            .join("symbols")
    }

    fn fixture(relative: &str) -> Option<PathBuf> {
        let path = symbols_root().join(relative);
        path.is_file().then_some(path)
    }

    fn scratch_root(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        let path = std::env::temp_dir().join(format!(
            "pid-parse-symlib-{tag}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("scratch root");
        path
    }

    const CIRCLE_UNC: &str = r"\\WIN-SPID\p\Ref\Symbols\Design\Annotation\Graphics\Circle.sym";

    #[test]
    fn library_relative_path_strips_every_site_prefix() {
        let cases = [
            (
                r"\\WIN-SPID\qsmcqtaz13\Plant\Ref\Symbols\Piping\Valves\Angle\Globe.sym",
                r"Piping\Valves\Angle\Globe.sym",
            ),
            (
                r"\\SPID\XaLNG\P&ID Reference Data\Symbols\Equipment Components\Nozzles\Flanged Nozzle.sym",
                r"Equipment Components\Nozzles\Flanged Nozzle.sym",
            ),
            (
                r"\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Design\Annotation\Graphics\Line.sym",
                r"Design\Annotation\Graphics\Line.sym",
            ),
        ];
        for (unc, expected) in cases {
            assert_eq!(library_relative_path(unc), Some(expected), "for {unc}");
        }
    }

    #[test]
    fn library_relative_path_rejects_a_path_with_no_library_root() {
        assert_eq!(library_relative_path(r"C:\scratch\thing.sym"), None);
        assert_eq!(library_relative_path(""), None);
    }

    #[test]
    fn circle_symbol_reads_as_a_single_circle() {
        let Some(path) = fixture(r"Design/Annotation/Graphics/Circle.sym") else {
            return;
        };
        let body = read_symbol_geometry(&path).expect("Circle.sym is a readable compound file");
        assert_eq!(
            body.primitives.len(),
            1,
            "the annotation Circle symbol is one circle: {:?}",
            body.primitives
        );
        let SymbolPrimitive::Circle { center, radius } = body.primitives[0].primitive else {
            panic!("expected a circle, got {:?}", body.primitives[0]);
        };
        assert!((radius - 0.003_810).abs() < 1e-9, "radius {radius}");
        assert!((center.0 - 0.101_600).abs() < 1e-9, "centre x {}", center.0);
        assert!((center.1 - 0.127_000).abs() < 1e-9, "centre y {}", center.1);
    }

    /// An arc's trailing byte is the direction it runs, and the reader hands
    /// it over.
    ///
    /// `FT-Ultrasonic Flowmeter.sym` states two arcs with the *same* angle
    /// pair, `90° -> 270°`, at mirrored centres and opposite directions: the
    /// wave's two halves bulge opposite ways, so the arcs are the two halves
    /// of their shared vertical diameter. Read without the byte both halves
    /// draw on the same side and the wave becomes a pair of C's.
    #[test]
    fn an_arc_carries_the_direction_its_trailing_byte_states() {
        let path = symbols_root()
            .with_file_name("symbols-full")
            .join(r"Instrumentation/In-Line/Flow/FT-Ultrasonic Flowmeter.sym");
        if !path.is_file() {
            return;
        }
        let body = read_symbol_geometry(&path).expect("a readable compound file");
        let halves: Vec<(&(f64, f64), bool)> = body
            .primitives
            .iter()
            .filter_map(|styled| match &styled.primitive {
                SymbolPrimitive::Arc {
                    center,
                    start_angle,
                    end_angle,
                    clockwise,
                    ..
                } if (start_angle.to_degrees() - 90.0).abs() < 1e-9
                    && (end_angle.to_degrees() - 270.0).abs() < 1e-9 =>
                {
                    Some((center, *clockwise))
                }
                _ => None,
            })
            .collect();
        assert!(!halves.is_empty(), "the wave's arcs: {body:?}");
        for (center, clockwise) in &halves {
            let mirror = halves
                .iter()
                .find(|(other, _)| {
                    (other.0 + center.0).abs() < 1e-9 && (other.1 + center.1).abs() < 1e-9
                })
                .unwrap_or_else(|| panic!("no mirrored half for {center:?} in {halves:?}"));
            assert_ne!(
                *clockwise, mirror.1,
                "the mirrored halves run opposite ways: {halves:?}"
            );
        }
    }

    /// A symbol states its own colour and width, and the reader hands them
    /// over with the stroke they belong to.
    ///
    /// The colour cannot come from the drawing: an `igSymbol2d` placement
    /// carries the `JSite` id where its siblings carry the style index, so
    /// nothing on the placement names a style at all. It comes from the
    /// `StyleCluster` the `.sym` carries itself. `ElecTraceLine` is the
    /// smallest symbol that proves the join: its table defines exactly one
    /// line style, `#0000FF` at 0.5mm, and every one of its ten strokes
    /// indexes it -- which is why the trace-heating row draws blue.
    #[test]
    fn a_symbol_carries_the_colour_and_width_its_own_style_table_states() {
        let Some(path) = fixture(r"Design/Annotation/Graphics/ElecTraceLine.sym") else {
            return;
        };
        let body =
            read_symbol_geometry(&path).expect("ElecTraceLine.sym is a readable compound file");
        assert_eq!(body.primitives.len(), 10, "8 lines and 2 arcs");
        for styled in &body.primitives {
            let style = styled
                .style
                .as_ref()
                .unwrap_or_else(|| panic!("every stroke names a style: {styled:?}"));
            assert_eq!(style.rgb, [0x00, 0x00, 0xFF], "{styled:?}");
            assert!(
                (style.width_mm - 0.5).abs() < 1e-9,
                "width {}",
                style.width_mm
            );
            assert!(!style.is_dashed(), "a solid stroke: {styled:?}");
        }
    }

    /// The dash a `.sym`'s own style names comes across with the stroke.
    /// `Wastewater Pit.sym` is the library body whose cached copy DWG-0202
    /// places: three of its seven lines name the table's `Dashed` style
    /// outright and dash at 3.5 / 1.75 mm; the rest draw solid.
    #[test]
    fn a_symbol_carries_the_dash_its_own_style_table_states() {
        // The reference library beside the fixtures, not the small sample
        // the other tests read: the pit is only in the full one.
        let path = symbols_root()
            .with_file_name("symbols-full")
            .join(r"Equipment/Vessels/Tanks/Wastewater Pit.sym");
        if !path.is_file() {
            return;
        }
        let body =
            read_symbol_geometry(&path).expect("Wastewater Pit.sym is a readable compound file");
        let dashed: Vec<&StyledPrimitive> = body
            .primitives
            .iter()
            .filter(|styled| styled.style.as_ref().is_some_and(PrimitiveStyle::is_dashed))
            .collect();
        assert_eq!(
            dashed.len(),
            3,
            "the pit's own style table dashes three of its strokes: {dashed:?}"
        );
        for styled in &dashed {
            let dash: Vec<f64> = styled
                .style
                .as_ref()
                .map(|style| {
                    style
                        .dash_mm
                        .iter()
                        .map(|mm| (mm.abs() * 100.0).round() / 100.0)
                        .collect()
                })
                .unwrap_or_default();
            assert_eq!(dash, vec![3.5, 1.75], "{styled:?}");
            assert!(
                matches!(styled.primitive, SymbolPrimitive::Line { .. }),
                "the dashed strokes are lines: {styled:?}"
            );
        }
        assert_eq!(body.primitives.len(), 7, "and the other four draw solid");
    }

    /// A `.sym`'s own lettering: the record's tail, plus its **runs** — the
    /// same rule the drawing side applies — with the paragraph index only as
    /// the default a run overrides. `Title Block Label.sym` letters its `NULL`
    /// placeholders in 宋体 3.0/3.5/4.0 mm and Arial Narrow 2.4694 mm (7 pt),
    /// every record keeping the unit direction (rotation `0`, no self-stated
    /// height) and one anchoring centre-top (`0x15`) where the rest anchor
    /// left-top.
    ///
    /// Two of those numbers are only reachable through the runs: the paragraph
    /// at `+14` says 3.5 mm and 2.5 mm where the run says 3.5278 mm (10 pt) and
    /// 2.4694 mm (7 pt), and one record's paragraph names Arial while its run
    /// names 宋体. See `docs/analysis/2026-08-22-run-beats-paragraph-default.md`.
    #[test]
    fn a_symbols_lettering_comes_off_its_own_style_table() {
        let path = symbols_root()
            .with_file_name("symbols-full")
            .join(r"Design/Title Block Label.sym");
        if !path.is_file() {
            return;
        }
        let body =
            read_symbol_geometry(&path).expect("Title Block Label.sym is a readable compound file");
        let lettered: Vec<&SymbolLettering> = body
            .primitives
            .iter()
            .filter(|styled| matches!(styled.primitive, SymbolPrimitive::Text { .. }))
            .map(|styled| {
                styled
                    .lettering
                    .as_ref()
                    .expect("every text record carries lettering")
            })
            .collect();
        assert!(!lettered.is_empty(), "the label letters placeholders");
        let styles: Vec<(u64, Option<&str>)> = lettered
            .iter()
            .map(|l| {
                (
                    l.height_m.map_or(0, |m| (m * 1e6).round() as u64),
                    l.font_name.as_deref(),
                )
            })
            .collect();
        assert!(
            styles.contains(&(3000, Some("宋体"))),
            "a 宋体 3 mm lettering: {styles:?}"
        );
        assert!(
            styles.contains(&(3500, Some("宋体"))),
            "a 宋体 3.5 mm lettering: {styles:?}"
        );
        assert!(
            styles.contains(&(3528, Some("SimSun-ExtB"))),
            "the run's 10 pt rather than the paragraph's 3.5 mm: {styles:?}"
        );
        assert!(
            styles.contains(&(2469, Some("Arial Narrow"))),
            "the run's 7 pt rather than the paragraph's 2.5 mm: {styles:?}"
        );
        assert!(
            styles.contains(&(4000, Some("宋体"))),
            "the run's typeface rather than the paragraph's Arial: {styles:?}"
        );
        for lettering in &lettered {
            assert_eq!(lettering.rotation_rad, 0.0, "unit direction: {lettering:?}");
            assert_eq!(
                lettering.letter_height_m, 0.0,
                "unit direction: {lettering:?}"
            );
            assert!(
                lettering.rgb.is_some(),
                "the char style's colour: {lettering:?}"
            );
        }
        assert!(
            lettered.iter().any(|l| l.anchor_code == 0x01),
            "a left-top anchored record: {lettered:?}"
        );
        assert!(
            lettered.iter().any(|l| l.anchor_code == 0x15),
            "a centre-top anchored record: {lettered:?}"
        );
    }

    /// An override in a `.sym`'s own table has its say over the base it
    /// names (`style_link`, plan 2026-10-10). `arrester breather valve(RD).sym`
    /// reaches `Dashed` for seven lines and its B-spline lip through an
    /// override stating 0.13 mm and *no* dash, so the whole body draws
    /// solid — which is what the drawing's cached copy on DWG-0202 says too.
    /// Until the override's fields were read, those eight strokes read as
    /// dashed.
    #[test]
    fn a_symbols_override_switches_off_the_dash_its_base_names() {
        let path = symbols_root()
            .with_file_name("symbols-full")
            .join(r"Piping/Valves/2 Way Other/arrester breather valve(RD).sym");
        if !path.is_file() {
            return;
        }
        let body = read_symbol_geometry(&path)
            .expect("arrester breather valve(RD).sym is a readable compound file");
        assert_eq!(body.primitives.len(), 30);
        assert!(
            body.primitives
                .iter()
                .any(|styled| matches!(styled.primitive, SymbolPrimitive::BSpline { .. })),
            "the lip is still drawn"
        );
        for styled in &body.primitives {
            let style = styled
                .style
                .as_ref()
                .unwrap_or_else(|| panic!("every stroke names a style: {styled:?}"));
            assert!(!style.is_dashed(), "a solid stroke: {styled:?}");
            assert!(
                (style.width_mm - 0.13).abs() < 1e-9,
                "the override's 0.13 mm, not the base's 0.35: {styled:?}"
            );
            assert_eq!(style.rgb, [0, 0, 0], "{styled:?}");
        }
    }

    #[test]
    fn valve_symbol_reads_as_lines_and_circles_around_its_own_origin() {
        let Some(path) = fixture(r"Piping/Valves/Angle/2-Way Angle Globe Valve.sym") else {
            return;
        };
        let body = read_symbol_geometry(&path).expect("valve .sym is a readable compound file");
        let lines = body
            .primitives
            .iter()
            .filter(|p| matches!(p.primitive, SymbolPrimitive::Line { .. }))
            .count();
        let circles = body
            .primitives
            .iter()
            .filter(|p| matches!(p.primitive, SymbolPrimitive::Circle { .. }))
            .count();
        assert!(lines >= 8, "a globe valve is drawn with lines: {lines}");
        assert!(circles >= 2, "and its seat circles: {circles}");

        let (min_x, min_y, max_x, max_y) = body.bounds().expect("a drawn symbol has bounds");
        // The body is authored around the placement origin at roughly the
        // size of a P&ID valve: a handful of millimetres, not metres.
        assert!(min_x >= -0.02 && min_y >= -0.02, "bounds {min_x} {min_y}");
        assert!(max_x <= 0.02 && max_y <= 0.02, "bounds {max_x} {max_y}");
        assert!(max_x - min_x > 0.001, "width {}", max_x - min_x);
        assert!(max_y - min_y > 0.001, "height {}", max_y - min_y);
    }

    #[test]
    fn a_symbols_hidden_layers_stay_off_the_sheet() {
        // The pressure gauge files two circles: its face on `Default`, and
        // a heat-trace halo on the layer of the same name. The plot only
        // ever draws the face — the halo is how the drawing-side cache of
        // this same body files its `displayed: false` layer.
        let Some(path) = fixture(
            r"Instrumentation/Off-Line/Without Implied Components/Pressure/PG-Pressure Gauge.sym",
        ) else {
            return;
        };
        let body = read_symbol_geometry(&path).expect("gauge .sym is a readable compound file");
        // The file keeps a layer table per storage; merged they repeat the
        // canonical family. A primitive's layer name resolves through its
        // own storage's table, so the `Heat Trace` halo and `Label` text
        // name themselves even where their oids repeat across storages.
        let table_names: Vec<&str> = body
            .sheet_layers
            .iter()
            .map(|layer| layer.name.as_str())
            .collect();
        assert!(table_names.contains(&"Default"), "{table_names:?}");
        assert!(table_names.contains(&"Heat Trace"), "{table_names:?}");

        // Face and halo on `Default`/`Heat Trace`; the catalogue text —
        // `PG` and its `NULL` placeholders — sits on the `Label` layer the
        // file files them under, which the drawing-side caches file as
        // `Label` hidden the same way.
        let layers: Vec<Option<&str>> = body
            .primitives
            .iter()
            .map(|styled| styled.sheet_layer_name.as_deref())
            .collect();
        assert!(layers.contains(&Some("Heat Trace")), "{layers:?}");
        assert!(layers.contains(&Some("Default")), "{layers:?}");

        let displayed = body.displayed_primitives();
        assert!(
            displayed
                .iter()
                .all(|styled| styled.sheet_layer_name.as_deref() == Some("Default")),
            "only the `Default` strokes draw: {displayed:?}"
        );
        assert!(
            displayed
                .iter()
                .all(|styled| !matches!(styled.primitive, SymbolPrimitive::Text { .. })),
            "the `PG` label letters on the sheet, not in the body: {displayed:?}"
        );
        // The second sheet is the alternate view: the file's own view
        // states decode only for sheet 6, so `Sheet68`'s catalogue `PG`
        // and its duplicate face stay off with it.
        assert!(
            displayed.iter().all(|styled| styled.sheet_ref == 6),
            "only the normal-view sheet draws: {displayed:?}"
        );
        assert_eq!(
            body.sheet_views
                .iter()
                .map(|view| view.sheet_ref)
                .collect::<Vec<_>>(),
            [6],
            "the file's decodable view state names its normal sheet"
        );
    }

    #[test]
    fn a_body_authored_entirely_on_hidden_names_still_draws() {
        // The filter decides which layers of a body draw, never whether the
        // body draws: a symbol that files everything under `HiddenObjects`
        // keeps its strokes rather than leaving a bare marker where a part
        // was meant to be.
        let styled = |name: &str, primitive| StyledPrimitive {
            primitive,
            style: None,
            lettering: None,
            sheet_layer_ref: 1,
            sheet_layer_name: Some(name.to_owned()),
            sheet_layer_number: 1,
            sheet_ref: 6,
        };
        let body = SymbolGeometry {
            primitives: vec![styled(
                "HiddenObjects",
                SymbolPrimitive::Line {
                    start: (0.0, 0.0),
                    end: (1.0, 1.0),
                },
            )],
            skipped_records: BTreeMap::new(),
            sheet_layers: vec![SymbolSheetLayer {
                oid: 1,
                layer_number: 1,
                name: "HiddenObjects".to_owned(),
            }],
            sheet_views: Vec::new(),
        };
        assert_eq!(body.displayed_primitives().len(), 1);
    }

    #[test]
    fn a_body_no_sheet_shows_comes_back_whole_across_its_sheets() {
        // Hidden lettering on a clean sheet, a hidden stroke on a sheet with
        // no view set, another under an all-on set: nothing shows anywhere,
        // so no pick shows anything and the body comes back whole.
        let styled = |sheet_ref: u32, primitive| StyledPrimitive {
            primitive,
            style: None,
            lettering: None,
            sheet_layer_ref: 1,
            sheet_layer_name: Some("Label".to_owned()),
            sheet_layer_number: 1,
            sheet_ref,
        };
        let line = |x: f64| SymbolPrimitive::Line {
            start: (x, 0.0),
            end: (x, 1.0),
        };
        let view = |sheet_ref: u32, display: u8| SymbolSheetView {
            sheet_ref,
            display: vec![display],
            layers: vec![(1, "Label".to_owned())],
        };
        let body = SymbolGeometry {
            primitives: vec![
                styled(6, line(0.0)),
                styled(7, line(1.0)),
                styled(8, line(2.0)),
            ],
            skipped_records: BTreeMap::new(),
            sheet_layers: Vec::new(),
            sheet_views: vec![view(6, 0b01), view(8, 0b11)],
        };
        assert_eq!(body.displayed_primitives().len(), 3);
    }

    /// A body kept only on an all-on sheet has no *normal* view: the sheet
    /// whose set files every layer on reads as the alternate. `Sheet6` is
    /// then a stub, and picking it drew one stroke of a tree.
    #[test]
    fn a_sole_all_on_view_does_not_shrink_a_body_to_its_sheet6_stub() {
        let styled = |sheet: u32, x: f64| StyledPrimitive {
            primitive: SymbolPrimitive::Line {
                start: (x, 0.0),
                end: (x, 1.0),
            },
            style: None,
            lettering: None,
            sheet_layer_ref: 1,
            sheet_layer_name: Some("Default".to_owned()),
            sheet_layer_number: 0,
            sheet_ref: sheet,
        };
        let body = SymbolGeometry {
            primitives: vec![styled(6, 0.0), styled(374, 1.0), styled(374, 2.0)],
            skipped_records: BTreeMap::new(),
            sheet_layers: vec![SymbolSheetLayer {
                oid: 1,
                layer_number: 0,
                name: "Default".to_owned(),
            }],
            sheet_views: vec![SymbolSheetView {
                sheet_ref: 374,
                display: vec![0xFF, 0xFF],
                layers: vec![
                    (0, "Default".to_owned()),
                    (1, "Label".to_owned()),
                    (2, "Heat Trace".to_owned()),
                ],
            }],
        };
        let displayed = body.displayed_primitives();
        assert_eq!(
            displayed.len(),
            2,
            "the governed sheet's strokes draw, not Sheet6's single stub: {displayed:?}"
        );
        assert!(
            displayed.iter().all(|styled| styled.sheet_ref == 374),
            "{displayed:?}"
        );
    }

    #[test]
    fn a_wellhead_tree_keeps_its_body_off_the_single_stroke_stub() {
        // The planted file keeps one B-spline on `Sheet6` and the tree — 48
        // lines, 23 circles, a B-spline, 11 fills — on `Sheet374`, whose only
        // view set files every layer on. The drawings' own cached bodies and
        // the vendor's reference plot both draw the tree.
        // The reference share is a local corpus, not a repository fixture, so
        // the test soft-skips where it is absent (CI).
        let corpus = std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
            || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
            PathBuf::from,
        );
        let path =
            corpus.join(r"Ref/Symbols/Equipment/Other Equipment/Wellhead Christmas Tree 02.sym");
        if !path.is_file() {
            eprintln!("skipping the wellhead body; {} is absent", path.display());
            return;
        }
        let body = read_symbol_geometry(&path).expect("wellhead .sym is a readable compound file");
        let displayed = body.displayed_primitives();
        assert!(
            displayed.len() >= 40,
            "the tree keeps its body: {} strokes of {}",
            displayed.len(),
            body.primitives.len()
        );
    }

    /// A clean view set that hides every stroke its own sheet holds is not a
    /// pick. `Insulation Joint.sym` files the canonical `Default`-only map on
    /// `Sheet6` beside one `Heat Trace` stroke that map turns off, and the
    /// body — 16 lines on `Default` — on a `Sheet182` with no view set at
    /// all; taking `Sheet6` emptied the pick and the whole-body fallback then
    /// drew the hidden stroke too (17).
    #[test]
    fn a_clean_view_set_that_draws_nothing_does_not_win() {
        let line = |start: f64| SymbolPrimitive::Line {
            start: (start, 0.0),
            end: (start, 1.0),
        };
        let styled_on = |sheet: u32, number: u32, name: &str, primitive| StyledPrimitive {
            primitive,
            style: None,
            lettering: None,
            sheet_layer_ref: number + 1,
            sheet_layer_name: Some(name.to_owned()),
            sheet_layer_number: number,
            sheet_ref: sheet,
        };
        let body = SymbolGeometry {
            primitives: vec![
                styled_on(6, 1, "Heat Trace", line(0.0)),
                styled_on(182, 0, "Default", line(1.0)),
                styled_on(182, 0, "Default", line(2.0)),
                styled_on(182, 0, "Default", line(3.0)),
            ],
            skipped_records: BTreeMap::new(),
            sheet_layers: Vec::new(),
            sheet_views: vec![SymbolSheetView {
                sheet_ref: 6,
                display: vec![0b0000_0001],
                layers: vec![(0, "Default".to_owned()), (1, "Heat Trace".to_owned())],
            }],
        };
        let drawn = body.displayed_primitives();
        assert_eq!(drawn.len(), 3, "the body, not the hidden stub: {drawn:?}");
        assert!(
            drawn.iter().all(|styled| styled.sheet_ref == 182),
            "every stroke comes off the body sheet: {drawn:?}"
        );
    }

    /// The check is only that the clean sheet draws something: a thin normal
    /// view still beats a fatter sheet, because the fatter one is how the
    /// alternate (insulated/heat-traced) representation is filed.
    #[test]
    fn a_drawing_clean_view_set_still_beats_a_fatter_sheet() {
        let line = |start: f64| SymbolPrimitive::Line {
            start: (start, 0.0),
            end: (start, 1.0),
        };
        let styled_on = |sheet: u32, primitive| StyledPrimitive {
            primitive,
            style: None,
            lettering: None,
            sheet_layer_ref: 1,
            sheet_layer_name: Some("Default".to_owned()),
            sheet_layer_number: 0,
            sheet_ref: sheet,
        };
        let mut primitives: Vec<StyledPrimitive> =
            (0..4).map(|i| styled_on(6, line(f64::from(i)))).collect();
        primitives.extend((0..6).map(|i| styled_on(39, line(f64::from(i)))));
        let body = SymbolGeometry {
            primitives,
            skipped_records: BTreeMap::new(),
            sheet_layers: Vec::new(),
            sheet_views: vec![
                SymbolSheetView {
                    sheet_ref: 6,
                    display: vec![0b0000_0001],
                    layers: vec![(0, "Default".to_owned()), (1, "Label".to_owned())],
                },
                SymbolSheetView {
                    sheet_ref: 39,
                    display: vec![0b0000_0011],
                    layers: vec![(0, "Default".to_owned()), (1, "Label".to_owned())],
                },
            ],
        };
        let drawn = body.displayed_primitives();
        assert_eq!(drawn.len(), 4, "the normal view's own strokes: {drawn:?}");
        assert!(
            drawn.iter().all(|styled| styled.sheet_ref == 6),
            "not the all-on sheet: {drawn:?}"
        );
    }

    #[test]
    fn hidden_layer_names_match_the_drawing_side_spellings() {
        for name in [
            "Heat Trace",
            "HiddenObjects",
            "Hidden Objects",
            "Jacket",
            "Label",
            "Construction",
            "Dimension",
            "Deminsion",
            "heat trace",
        ] {
            assert!(hidden_symbol_layer(name), "{name}");
        }
        for name in ["Default", "Invisible", "ConnectPoints", ""] {
            assert!(!hidden_symbol_layer(name), "{name}");
        }
    }

    #[test]
    fn library_resolves_through_a_unc_path_and_caches_the_miss() {
        let root = symbols_root();
        if !root.is_dir() {
            return;
        }
        let mut library = SymbolLibrary::new(&root);
        let hit = library
            .resolve(CIRCLE_UNC)
            .map(|body| body.primitives.len());
        assert_eq!(hit, Some(1));

        assert!(library
            .resolve(r"\\WIN-SPID\p\Ref\Symbols\Piping\Nope\Not Here.sym")
            .is_none());
        assert_eq!(library.lookups(), 2);
        assert_eq!(library.resolved(), 1);
        assert_eq!(library.missing().len(), 1);
    }

    #[test]
    fn library_searches_past_a_root_that_lacks_the_symbol() {
        // The fixtures cite five different reference shares, and a machine
        // normally holds a partial copy of each rather than one merged tree.
        if !symbols_root().is_dir() {
            return;
        }
        let absent = scratch_root("absent");
        let mut library = SymbolLibrary::with_roots([absent.clone(), symbols_root()]);
        let hit = library
            .resolve(CIRCLE_UNC)
            .map(|body| body.primitives.len());
        assert_eq!(hit, Some(1), "the second root holds the symbol");
        std::fs::remove_dir_all(&absent).ok();
    }

    /// A stored path can carry a leading space the share does not have.
    ///
    /// The plant backup's `集注站` drawings name
    /// `\Symbols\Piping\Piping OPC's\ Off-Unit SQ.sym`; the file beside them
    /// is `Off-Unit SQ.sym`. 64 placements across that set (43 `Off-Unit SQ`,
    /// 21 `Filter Type 2`) used to resolve to nothing and drew no symbol at
    /// all before the trimmed spelling was tried.
    #[test]
    fn a_component_leading_space_still_finds_the_file_on_disk() {
        if !symbols_root().is_dir() {
            return;
        }
        let mut library = SymbolLibrary::new(symbols_root());
        let stored = r"\\WIN-SPID\p\Ref\Symbols\Design\Annotation\Graphics\ Circle.sym";
        let hit = library.resolve(stored).map(|body| body.primitives.len());
        assert_eq!(hit, Some(1), "the stray space is the share's, not ours");
        assert!(library.missing().is_empty());
    }

    #[test]
    fn symbol_candidates_trim_only_a_component_leading_space() {
        assert_eq!(
            symbol_candidates("Piping/Piping OPC's/ Off-Unit SQ.sym"),
            vec![
                "Piping/Piping OPC's/ Off-Unit SQ.sym".to_string(),
                "Piping/Piping OPC's/Off-Unit SQ.sym".to_string(),
            ]
        );
        assert_eq!(
            symbol_candidates("Piping/Off-Unit SQ.sym"),
            vec!["Piping/Off-Unit SQ.sym".to_string()]
        );
    }

    #[test]
    fn library_searches_past_a_root_whose_copy_will_not_read() {
        // A partial copy can leave a placeholder or a truncated file where
        // another copy has the real symbol. That must not end the search.
        if !symbols_root().is_dir() {
            return;
        }
        let broken = scratch_root("broken");
        let dir = broken.join("Design").join("Annotation").join("Graphics");
        std::fs::create_dir_all(&dir).expect("scratch tree");
        std::fs::write(dir.join("Circle.sym"), b"not a compound file").expect("placeholder");

        let mut library = SymbolLibrary::with_roots([broken.clone(), symbols_root()]);
        let hit = library
            .resolve(CIRCLE_UNC)
            .map(|body| body.primitives.len());
        assert_eq!(hit, Some(1), "the readable copy wins");
        std::fs::remove_dir_all(&broken).ok();
    }

    /// A text record laid out the way the library writes one: the shape
    /// discriminator at `+18`, the shape's own count and text fields, then the
    /// fixed 36-byte tail — the insertion point, a unit direction along the
    /// page, and an empty trailer.
    fn text_payload(shape: u16, text: &str, at: (f64, f64)) -> Vec<u8> {
        text_payload_with_counters(shape, 3, 2, text, at)
    }

    /// The same, with shape 3's two counters — the `A + B` formatting runs
    /// that follow its text and push the tail past the end of the characters.
    fn text_payload_with_counters(
        shape: u16,
        a: u16,
        b: u16,
        text: &str,
        at: (f64, f64),
    ) -> Vec<u8> {
        let chars: Vec<u16> = text.encode_utf16().collect();
        let count = u16::try_from(chars.len()).expect("a short label");
        let mut payload = vec![0u8; IGTEXTBOX_BODY_START];
        payload[TEXT_SHAPE_AT..TEXT_SHAPE_AT + 2].copy_from_slice(&shape.to_le_bytes());
        let kind = IGTEXTBOX_TAIL_KIND_PLACEMENT.to_le_bytes();
        payload[IGTEXTBOX_TAIL_KIND_AT..IGTEXTBOX_TAIL_KIND_AT + 2].copy_from_slice(&kind);
        match shape {
            1 => payload.extend_from_slice(&count.to_le_bytes()),
            2 => {
                // The count appears twice: packed with a `0x10000` marker and
                // again, plain, four bytes later.
                let packed = u32::from(count) | 0x1_0000;
                payload.extend_from_slice(&packed.to_le_bytes());
                payload.extend_from_slice(&[0u8; 4]);
                payload.extend_from_slice(&count.to_le_bytes());
            }
            3 => {
                payload.extend_from_slice(&a.to_le_bytes());
                payload.extend_from_slice(&b.to_le_bytes());
                payload.extend_from_slice(&count.to_le_bytes());
            }
            other => panic!("no text shape {other}"),
        }
        for c in &chars {
            payload.extend_from_slice(&c.to_le_bytes());
        }
        if shape == 3 {
            // The runs: 8 bytes each, part of the body, ahead of the tail.
            let counters = 8 * usize::from(a + b);
            payload.resize(payload.len() + counters, 0);
        }
        payload.extend_from_slice(&at.0.to_le_bytes());
        payload.extend_from_slice(&at.1.to_le_bytes());
        payload.extend_from_slice(&1.0_f64.to_le_bytes());
        payload.extend_from_slice(&0.0_f64.to_le_bytes());
        payload.extend_from_slice(&[0u8; TEXT_TRAILING_LEN - 32]);
        payload
    }

    /// A text record's own tail — direction pair and trailer — plus `extra`
    /// bytes after it, which make it a text-box tail the way real records
    /// carry one: 32 fixed bytes, then whole 8-byte fields.
    fn text_payload_with_tail(
        shape: u16,
        text: &str,
        at: (f64, f64),
        direction: (f64, f64),
        anchor: u8,
        extra: usize,
    ) -> Vec<u8> {
        let mut payload = text_payload(shape, text, at);
        let tail = payload.len() - TEXT_TRAILING_LEN;
        payload[tail + 16..tail + 24].copy_from_slice(&direction.0.to_le_bytes());
        payload[tail + 24..tail + 32].copy_from_slice(&direction.1.to_le_bytes());
        payload[tail + TEXT_TRAILING_LEN - 1] = anchor;
        if extra > 0 {
            let kind = IGTEXTBOX_TAIL_KIND_BOX.to_le_bytes();
            payload[IGTEXTBOX_TAIL_KIND_AT..IGTEXTBOX_TAIL_KIND_AT + 2].copy_from_slice(&kind);
        }
        payload.resize(payload.len() + extra, 0);
        payload
    }

    #[test]
    fn text_record_decodes_shape_two_the_common_one() {
        let payload = text_payload(2, "HH=NULL ", (0.012, -0.034));
        assert_eq!(
            decode_primitive(TYPE_TEXT, &payload),
            Some(SymbolPrimitive::Text {
                text: "HH=NULL ".to_owned(),
                at: (0.012, -0.034),
            })
        );
    }

    #[test]
    fn text_record_decodes_shape_one() {
        let payload = text_payload(1, "设备位号", (0.05, 0.06));
        assert_eq!(
            decode_primitive(TYPE_TEXT, &payload),
            Some(SymbolPrimitive::Text {
                text: "设备位号".to_owned(),
                at: (0.05, 0.06),
            })
        );
    }

    #[test]
    fn text_record_decodes_shape_three_past_its_counters() {
        // 202 of the library's records are shape 3: the count sits at `+26`,
        // the text at `+28`, and the insertion point past the `A + B` run
        // entries. The old offset guess never read one of them.
        let payload = text_payload_with_counters(3, 3, 2, "CSC NULL", (0.10795, 0.1397));
        assert_eq!(
            decode_primitive(TYPE_TEXT, &payload),
            Some(SymbolPrimitive::Text {
                text: "CSC NULL".to_owned(),
                at: (0.10795, 0.1397),
            })
        );
        // The bytes at the end of the text are the counters, not the point.
        let (_, text_start, tail_start) = text_layout(&payload).expect("layout");
        assert_ne!(
            text_start + 2 * "CSC NULL".encode_utf16().count(),
            tail_start
        );
    }

    #[test]
    fn text_record_reads_with_optional_fields_after_the_tail() {
        // 404 records carry 32–56 bytes after their 36-byte tail. Demanding
        // that the tail end the payload refused every one of them.
        let payload = text_payload_with_tail(2, "TBD", (0.108, 0.1396), (1.0, 0.0), 0x01, 32);
        assert_eq!(
            decode_primitive(TYPE_TEXT, &payload),
            Some(SymbolPrimitive::Text {
                text: "TBD".to_owned(),
                at: (0.108, 0.1396),
            })
        );
    }

    #[test]
    fn text_lettering_reads_the_tail_and_survives_a_missing_table() {
        // `text_payload` writes a unit direction; it is patched to a
        // height-scaled pair and the trailer's last byte to the anchor.
        let payload = text_payload_with_tail(2, "P", (0.0, 0.0), (0.003_175, 0.0), 0x01, 0);
        let primitive = decode_primitive(TYPE_TEXT, &payload).expect("decodes");
        let lettering = lettering_of(&primitive, &payload, None).expect("text letters");
        // No table to ask, so the style fields stay unanswered — but the
        // tail is the record's own and reads anyway.
        assert_eq!(lettering.anchor_code, 0x01);
        assert_eq!(lettering.rotation_rad, 0.0);
        assert!((lettering.letter_height_m - 0.003_175).abs() < 1e-12);
        assert_eq!(lettering.height_m, None);
        assert_eq!(lettering.font_name, None);

        let straight = SymbolPrimitive::Line {
            start: (0.0, 0.0),
            end: (1.0, 0.0),
        };
        assert_eq!(lettering_of(&straight, &payload, None), None);
    }

    #[test]
    fn text_lettering_reads_a_rotated_unit_direction() {
        // Up the page: the unit pair (0, 1) states rotation and no height.
        let payload = text_payload_with_tail(2, "I", (0.0, 0.0), (0.0, 1.0), 0x15, 0);
        let primitive = decode_primitive(TYPE_TEXT, &payload).expect("decodes");
        let lettering = lettering_of(&primitive, &payload, None).expect("text letters");
        assert_eq!(lettering.rotation_rad.to_degrees(), 90.0);
        assert_eq!(lettering.letter_height_m, 0.0);
        assert_eq!(lettering.anchor_code, 0x15);
    }

    /// A `StyleCluster` holding just what a text lettering reads: one
    /// `JStyleTextChar` per `(id, height)`, plus a `JStyleTextPara` naming one
    /// where the record asks for the paragraph route.
    fn style_cluster(chars: &[(u32, f64)], para: Option<(u32, u32)>) -> DocumentStyleTable {
        let mut records: Vec<(u16, Vec<u8>)> = chars
            .iter()
            .map(|(id, height)| {
                let mut payload = vec![0u8; 94];
                payload[crate::style_link::STYLE_ID_OFFSET..crate::style_link::STYLE_ID_OFFSET + 4]
                    .copy_from_slice(&id.to_le_bytes());
                payload[crate::style_link::TEXT_CHAR_HEIGHT_OFFSET
                    ..crate::style_link::TEXT_CHAR_HEIGHT_OFFSET + 8]
                    .copy_from_slice(&height.to_le_bytes());
                (crate::style_link::PSM_TYPE_CODE_JSTYLE_TEXT_CHAR, payload)
            })
            .collect();
        if let Some((id, names)) = para {
            let mut payload = vec![0u8; 90];
            payload[crate::style_link::STYLE_ID_OFFSET..crate::style_link::STYLE_ID_OFFSET + 4]
                .copy_from_slice(&id.to_le_bytes());
            payload[crate::style_link::TEXT_PARA_CHAR_REFERENCE_OFFSET
                ..crate::style_link::TEXT_PARA_CHAR_REFERENCE_OFFSET + 4]
                .copy_from_slice(&names.to_le_bytes());
            records.push((crate::style_link::PSM_TYPE_CODE_JSTYLE_TEXT_PARA, payload));
        }
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&CLUSTER_MAGIC.to_le_bytes());
        bytes.extend_from_slice(&(records.len() as u32).to_le_bytes());
        for (type_code, payload) in &records {
            bytes.extend_from_slice(&type_code.to_le_bytes());
            bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            bytes.extend_from_slice(payload);
        }
        DocumentStyleTable::from_stylecluster_bytes(&bytes)
    }

    /// Overwrite one run of a shape-3 payload built by
    /// [`text_payload_with_counters`].
    fn write_run(payload: &mut [u8], count: usize, index: usize, len: u16, selector: u16, id: u32) {
        let at = 28 + 2 * count + 8 * index;
        payload[at..at + 2].copy_from_slice(&len.to_le_bytes());
        payload[at + 2..at + 4].copy_from_slice(&selector.to_le_bytes());
        payload[at + 4..at + 8].copy_from_slice(&id.to_le_bytes());
    }

    #[test]
    fn text_lettering_takes_its_size_off_the_widest_run() {
        // "ABC DEFG": three characters in `JStyleTextChar` 10 (2.5 mm), five
        // in 11 (5.0 mm), while the paragraph `+14` names 12 (3.175 mm) — the
        // library's old paragraph-only hop read the 3.175 mm default and the
        // label came out at the wrong size. The run covering the most
        // characters is the one to draw.
        let mut payload = text_payload_with_counters(3, 2, 0, "ABC DEFG", (0.1, 0.14));
        let (count, _, _) = text_layout(&payload).expect("layout");
        payload[STYLE_INDEX_OFFSET..STYLE_INDEX_OFFSET + 4].copy_from_slice(&9u32.to_le_bytes());
        write_run(&mut payload, count, 0, 3, 1, 10);
        write_run(&mut payload, count, 1, 5, 1, 11);
        let table = style_cluster(&[(10, 0.0025), (11, 0.005), (12, 0.003_175)], Some((9, 12)));

        let primitive = decode_primitive(TYPE_TEXT, &payload).expect("decodes");
        let lettering = lettering_of(&primitive, &payload, Some(&table)).expect("text letters");
        assert_eq!(lettering.run_style_id, Some(11), "the longer run wins");
        assert!((lettering.height_m.expect("a height") - 0.005).abs() < 1e-12);
    }

    #[test]
    fn a_multi_run_label_exposes_the_spans_it_letters_in() {
        // "ABC DEFG" in 2.5 mm then 5 mm: the widest run is what a one-style
        // renderer draws, and the partition is what a span-by-span one draws.
        let mut payload = text_payload_with_counters(3, 2, 0, "ABC DEFG", (0.1, 0.14));
        let (count, _, _) = text_layout(&payload).expect("layout");
        payload[STYLE_INDEX_OFFSET..STYLE_INDEX_OFFSET + 4].copy_from_slice(&9u32.to_le_bytes());
        write_run(&mut payload, count, 0, 3, 1, 10);
        write_run(&mut payload, count, 1, 5, 1, 11);
        let table = style_cluster(&[(10, 0.0025), (11, 0.005), (12, 0.003_175)], Some((9, 12)));

        let primitive = decode_primitive(TYPE_TEXT, &payload).expect("decodes");
        let lettering = lettering_of(&primitive, &payload, Some(&table)).expect("text letters");
        let spans: Vec<(u16, u16, u32, Option<f64>)> = lettering
            .spans
            .iter()
            .map(|span| (span.start, span.len, span.style_id, span.height_m))
            .collect();
        assert_eq!(
            spans,
            vec![(0, 3, 10, Some(0.0025)), (3, 5, 11, Some(0.005))]
        );
        // The flattened fields are still the widest run's, so a consumer that
        // cannot advance span by span draws what it always drew.
        assert_eq!(lettering.run_style_id, Some(11));
        assert!((lettering.height_m.expect("a height") - 0.005).abs() < 1e-12);
    }

    #[test]
    fn a_label_in_one_style_has_no_spans() {
        // Shape 2 stores one run covering the whole label: nothing to split.
        let mut payload = text_payload_with_counters(2, 0, 0, "ABC", (0.1, 0.14));
        payload[26..30].copy_from_slice(&11u32.to_le_bytes());
        payload[STYLE_INDEX_OFFSET..STYLE_INDEX_OFFSET + 4].copy_from_slice(&9u32.to_le_bytes());
        let table = style_cluster(&[(11, 0.005), (12, 0.003_175)], Some((9, 12)));

        let primitive = decode_primitive(TYPE_TEXT, &payload).expect("decodes");
        let lettering = lettering_of(&primitive, &payload, Some(&table)).expect("text letters");
        assert_eq!(lettering.run_style_id, Some(11));
        assert!(lettering.spans.is_empty(), "{lettering:?}");
    }

    #[test]
    fn a_label_with_a_non_bmp_character_keeps_its_spans_off() {
        // A run length counts UTF-16 units while a span offset counts chars, so
        // an astral character would land the second span mid-glyph. The corpus
        // has none; a label that had one draws in one style rather than wrong.
        let mut payload = text_payload_with_counters(3, 2, 0, "A😀B", (0.1, 0.14));
        let (count, _, _) = text_layout(&payload).expect("layout");
        payload[STYLE_INDEX_OFFSET..STYLE_INDEX_OFFSET + 4].copy_from_slice(&9u32.to_le_bytes());
        // The label is four UTF-16 units: `A`, the surrogate pair, `B`.
        write_run(&mut payload, count, 0, 1, 1, 10);
        write_run(&mut payload, count, 1, 3, 1, 11);
        let table = style_cluster(&[(10, 0.0025), (11, 0.005), (12, 0.003_175)], Some((9, 12)));

        let primitive = decode_primitive(TYPE_TEXT, &payload).expect("decodes");
        let lettering = lettering_of(&primitive, &payload, Some(&table)).expect("text letters");
        assert!(lettering.spans.is_empty(), "{lettering:?}");
        assert_eq!(
            lettering.run_style_id,
            Some(11),
            "the flattening still happens"
        );
    }

    #[test]
    fn text_lettering_keeps_the_paragraph_where_its_runs_resolve_to_nothing() {
        // A run naming a style this document does not define is not followed:
        // the paragraph route still answers, as it did before runs were read.
        let mut payload = text_payload_with_counters(3, 1, 0, "ABC", (0.1, 0.14));
        let (count, _, _) = text_layout(&payload).expect("layout");
        payload[STYLE_INDEX_OFFSET..STYLE_INDEX_OFFSET + 4].copy_from_slice(&9u32.to_le_bytes());
        write_run(&mut payload, count, 0, 3, 1, 404);
        let table = style_cluster(&[(12, 0.003_175)], Some((9, 12)));

        let primitive = decode_primitive(TYPE_TEXT, &payload).expect("decodes");
        let lettering = lettering_of(&primitive, &payload, Some(&table)).expect("text letters");
        assert_eq!(lettering.run_style_id, None);
        assert!((lettering.height_m.expect("a height") - 0.003_175).abs() < 1e-12);
    }

    #[test]
    fn text_lettering_ignores_runs_that_do_not_cover_the_text() {
        // The native sizer refuses a record whose selector-1 lengths do not sum
        // to its count, so a partial list is not a list that covers anything:
        // falling back to the paragraph is the honest read.
        let mut payload = text_payload_with_counters(3, 1, 0, "ABCDEFG", (0.1, 0.14));
        let (count, _, _) = text_layout(&payload).expect("layout");
        payload[STYLE_INDEX_OFFSET..STYLE_INDEX_OFFSET + 4].copy_from_slice(&9u32.to_le_bytes());
        write_run(&mut payload, count, 0, 3, 1, 11);
        let table = style_cluster(&[(11, 0.005), (12, 0.003_175)], Some((9, 12)));

        let primitive = decode_primitive(TYPE_TEXT, &payload).expect("decodes");
        let lettering = lettering_of(&primitive, &payload, Some(&table)).expect("text letters");
        assert_eq!(lettering.run_style_id, None);
        assert!((lettering.height_m.expect("a height") - 0.003_175).abs() < 1e-12);
    }

    #[test]
    fn text_lettering_follows_a_shape_two_record_single_run() {
        // Shape 2 stores its one run ahead of the count, in the dword the shape
        // check already reads: its style id sits at `+26`.
        let mut payload = text_payload_with_counters(2, 0, 0, "ABC", (0.1, 0.14));
        payload[26..30].copy_from_slice(&11u32.to_le_bytes());
        payload[STYLE_INDEX_OFFSET..STYLE_INDEX_OFFSET + 4].copy_from_slice(&9u32.to_le_bytes());
        let table = style_cluster(&[(11, 0.005), (12, 0.003_175)], Some((9, 12)));

        let primitive = decode_primitive(TYPE_TEXT, &payload).expect("decodes");
        let lettering = lettering_of(&primitive, &payload, Some(&table)).expect("text letters");
        assert_eq!(lettering.run_style_id, Some(11));
        assert!((lettering.height_m.expect("a height") - 0.005).abs() < 1e-12);
    }

    #[test]
    fn text_lettering_reads_the_tail_with_fields_after_it() {
        // Direction and anchor belong to the tail, not to the payload's last
        // 36 bytes — a shape-3 record with optional fields after the tail.
        let payload = text_payload_with_tail(3, "I", (0.0, 0.0), (0.0, 1.0), 0x15, 40);
        let primitive = decode_primitive(TYPE_TEXT, &payload).expect("decodes");
        let lettering = lettering_of(&primitive, &payload, None).expect("text letters");
        assert_eq!(lettering.rotation_rad.to_degrees(), 90.0);
        assert_eq!(lettering.anchor_code, 0x15);
    }

    #[test]
    fn text_record_is_skipped_when_its_count_over_reads_into_trailing_fields() {
        // With fields after the tail, a count eight characters too long still
        // finds room for a tail: the insertion point becomes characters and the
        // direction pair the insertion point. Only the pair it then lands on
        // tells it apart.
        let mut payload = text_payload_with_tail(1, "AB", (0.05, 0.06), (1.0, 0.0), 0x01, 48);
        payload[22..24].copy_from_slice(&10u16.to_le_bytes());
        assert!(
            text_layout(&payload).is_some(),
            "the layout alone cannot tell"
        );
        assert_eq!(decode_primitive(TYPE_TEXT, &payload), None);
    }

    #[test]
    fn text_record_is_skipped_when_its_direction_is_not_one() {
        // Neither the unit vector nor a pair scaled to a letter height.
        for direction in [(0.0, 0.0), (5.0, 0.0), (1.0e-300, 0.0)] {
            let payload = text_payload_with_tail(2, "TBD", (0.1, 0.1), direction, 0x01, 0);
            assert_eq!(decode_primitive(TYPE_TEXT, &payload), None, "{direction:?}");
        }
    }

    #[test]
    fn a_truncated_text_record_is_refused_without_panicking() {
        // Shape 3 with a text-box tail: no prefix that stops short of the
        // placement tail's end reads, and the whole record does.
        let payload = text_payload_with_tail(3, "CSC NULL", (0.108, 0.14), (1.0, 0.0), 0x15, 40);
        let tail_end = payload.len() - 40;
        for len in 0..tail_end {
            let decoded = decode_primitive(TYPE_TEXT, &payload[..len]);
            assert_eq!(decoded, None, "a {len}-byte prefix");
        }
        assert!(decode_primitive(TYPE_TEXT, &payload).is_some());
    }

    #[test]
    fn a_placement_tail_with_bytes_after_it_is_refused() {
        // A kind-1 record ends with its placement tail, as every library
        // record that says kind 1 does.
        let mut payload = text_payload(2, "TBD", (0.1, 0.1));
        payload.extend_from_slice(&[0u8; 8]);
        assert_eq!(decode_primitive(TYPE_TEXT, &payload), None);
    }

    #[test]
    fn a_text_box_tail_reads_only_in_whole_fields() {
        // The text-box tail is 32 fixed bytes, then 8-byte fields.
        for (extra, reads) in [(32, true), (40, true), (56, true), (24, false), (36, false)] {
            let payload = text_payload_with_tail(2, "TBD", (0.1, 0.1), (1.0, 0.0), 0x01, extra);
            assert_eq!(
                decode_primitive(TYPE_TEXT, &payload).is_some(),
                reads,
                "+{extra} B"
            );
        }
    }

    #[test]
    fn text_record_is_skipped_when_the_shape_is_unknown() {
        let mut payload = text_payload(2, "XXX", (0.0, 0.0));
        payload[TEXT_SHAPE_AT..TEXT_SHAPE_AT + 2].copy_from_slice(&7u16.to_le_bytes());
        assert_eq!(decode_primitive(TYPE_TEXT, &payload), None);
    }

    #[test]
    fn text_record_with_no_characters_is_skipped() {
        // A record stating an empty label draws nothing; the corpus has two.
        let payload = text_payload(1, "", (0.0, 0.0));
        assert_eq!(decode_primitive(TYPE_TEXT, &payload), None);
    }

    #[test]
    fn text_record_with_an_absurd_insertion_point_is_skipped() {
        let payload = text_payload(1, "XXX", (1.0e9, 0.0));
        assert_eq!(decode_primitive(TYPE_TEXT, &payload), None);
    }

    fn polyline_payload(form: u8, vertices: &[(f64, f64)]) -> Vec<u8> {
        let mut payload = vec![0u8; POLYLINE_VERTEX_START];
        payload[18..22].copy_from_slice(&(vertices.len() as u32).to_le_bytes());
        payload[22] = form;
        payload[23] = 1;
        for (x, y) in vertices {
            payload.extend_from_slice(&x.to_le_bytes());
            payload.extend_from_slice(&y.to_le_bytes());
        }
        payload
    }

    #[test]
    fn symbol_polyline_form_two_is_closed_and_form_one_is_open() {
        let open = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)];
        assert_eq!(
            decode_primitive(TYPE_POLYLINE, &polyline_payload(1, &open)),
            Some(SymbolPrimitive::Polyline {
                vertices: open.to_vec(),
                is_closed: false,
            })
        );

        let closed = [(0.0, 0.0), (1.0, 0.0), (0.0, 0.0)];
        assert_eq!(
            decode_primitive(TYPE_POLYLINE, &polyline_payload(2, &closed)),
            Some(SymbolPrimitive::Polyline {
                vertices: closed.to_vec(),
                is_closed: true,
            })
        );
    }

    #[test]
    fn bounds_take_in_the_text_insertion_point() {
        let unstyled = |primitive| StyledPrimitive {
            primitive,
            style: None,
            lettering: None,
            sheet_layer_ref: 0,
            sheet_layer_name: None,
            sheet_layer_number: unresolved_layer_number(),
            sheet_ref: 6,
        };
        let body = SymbolGeometry {
            primitives: vec![
                unstyled(SymbolPrimitive::Line {
                    start: (0.0, 0.0),
                    end: (0.1, 0.1),
                }),
                unstyled(SymbolPrimitive::Text {
                    text: "设备名称".to_owned(),
                    at: (-0.2, 0.3),
                }),
            ],
            skipped_records: BTreeMap::new(),
            sheet_layers: Vec::new(),
            sheet_views: Vec::new(),
        };
        assert_eq!(body.bounds(), Some((-0.2, 0.0, 0.1, 0.3)));
    }

    #[test]
    fn from_path_list_keeps_the_roots_in_order() {
        let joined =
            std::env::join_paths([Path::new("/first"), Path::new("/second")]).expect("plain paths");
        let library = SymbolLibrary::from_path_list(&joined);
        assert_eq!(library.roots().len(), 2);
        assert!(library.roots()[0].ends_with("first"));
        assert!(library.roots()[1].ends_with("second"));
    }
}
