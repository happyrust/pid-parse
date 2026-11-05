//! Write a [`PaperModel`](crate::model::PaperModel) to a one-page PDF.
//!
//! Decoded and Inferred primitives go on two optional-content groups. ProbeOnly
//! is left off the page. Every stroke draws with the pen its resolved line
//! style states — colour, width and dash — and a filled region paints its ring
//! with `f`; a stroke whose record resolved nothing falls back to a 0.25 mm
//! solid black pen. Arcs and circles are flattened at the same 0.05 mm chord
//! the reader uses, then stored as straight segments, so a read-back can match
//! the table endpoint for endpoint.
//!
//! Colour and width do not follow the record's pen literally. All five
//! reference plots draw every stroke and fill in black and use three widths
//! only, so the page is plotted through the vendor's monochrome pen table:
//! [`PLOT_INK`] is every path's ink, and [`pen_width_mm`] snaps a declared
//! width onto the thin / mid / thick pens. See
//! `docs/analysis/2026-10-17-monochrome-plot-three-pen-table.md` and
//! `docs/analysis/2026-10-20-the-page-is-plotted-through-one-black-pen.md`.
//!
//! Each text run is set in the face [`crate::fonts::typeface_key`] names.
//! A run mixes faces where the named face can encode some of it and not the
//! rest (`120℃` keeps its Latin digits in Arial Narrow and only the `℃`
//! borrows a CJK face), so an unsupported character never moves the
//! encodable characters' widths. A named face that can encode none of the
//! characters hands the whole string to one face that can, so a label is not
//! split because an earlier face happens to carry its last character. At a
//! 2.5 mm body that borrowed run is pitched at 1.524 mm a glyph, the spacing
//! the reference uses for that one label; every other height stays one em.
//! A style height that was measured on the
//! reference plots is written at that plot's `Tf`; every other height stays
//! the style height in points. Every face must be a static TrueType or
//! OpenType program pdfcraft will embed: under 16 MiB, not a collection, not
//! a variable font.

use std::f64::consts::TAU;

use kurbo::{Arc, BezPath, PathEl, flatten};
use pdfcraft_content::{Op, serialize_ops};
use pdfcraft_cos::{Dict, Document, Object, PdfString, Stream};
use pdfcraft_fonts::embedding::{self, embed};
use pdfcraft_fonts::pdf::Metrics;

use crate::fonts::typeface_key;
use crate::model::PaperTextSpan;
use crate::model::{PaperModel, PaperShape, PaperStroke, TextAnchor, TextHeightBasis, TextLine};
use crate::read::{CHORD_MM, DashWalk, sheet_edge_mm};

const PT_PER_MM: f64 = 72.0 / 25.4;
/// Body size for symbol text whose source record does not carry one.
pub const DEFAULT_TEXT_MM: f64 = 2.5;

/// The ink every stroke and fill of a plot is drawn in.
///
/// The reference plotter runs a monochrome pen table (the equivalent of
/// AutoCAD's monochrome CTB): all five reference PDFs carry 0/0/0 for every
/// stroke colour, every fill colour and every glyph. The sheet's own colours —
/// the olive and green this crate's model resolves off the styles — are screen
/// colours, and the comparison is against the plot.
const PLOT_INK: [u8; 3] = [0, 0, 0];

/// What a pen declaring `rgb` inks on paper.
///
/// Black for every colour except white: a white pen is the plot's wipe — the
/// reference pages carry two to sixteen of them, thin and a millimetre long,
/// where something has to un-ink a filled region (a valve's seal dot, a
/// dimension's gap). Painting those black would put ink on the page where the
/// reference has none, so the wipe survives the monochrome table.
fn plot_ink(rgb: [u8; 3]) -> [u8; 3] {
    if rgb == [255, 255, 255] {
        [255, 255, 255]
    } else {
        PLOT_INK
    }
}

/// The four pens the reference plotter used, in paper millimetres.
///
/// Half, one, two and three times the mid pen: `0.169 / 0.339 / 0.678 /
/// 1.017`. Three pens were all the five-drawing corpus ever showed. The A2
/// reference plot added on 2026-11-02 prints a fourth: its
/// `CPECCHB-SQ-A2.igr` title block declares 1.0606 / 1.1313 / 1.4141 mm
/// rules, and the plot prints every one of them at `1.02` — the wide pen,
/// not the thick one the table used to fold them onto.
const PEN_THIN_MM: f64 = 0.169;
const PEN_MID_MM: f64 = 0.339;
const PEN_THICK_MM: f64 = 0.678;
const PEN_WIDE_MM: f64 = 1.017;

/// The pen a stroke with no resolved style falls back to.
///
/// It is the mid pen itself, not a width beside it. A page states its default
/// pen once, before any stroke, and a viewer or a compare pass reads that
/// statement as one of the page's widths: a default outside the three makes
/// every page claim a fourth pen the reference plotter never carries. The
/// resolved path already sends a width it cannot use to the mid pen
/// ([`pen_width_mm`]); a stroke with no style at all lands on the same one.
const STROKE_MM: f64 = PEN_MID_MM;

/// Snap a declared stroke width onto the plot's four pens.
///
/// A `.pid` declares its widths on the ISO 128 ladder — 0.13 / 0.18 / 0.35 /
/// 0.70 mm on the I drawing — and plotting those literally gives more widths
/// than the plotter carries. The boundaries are measured, and they
/// are intervals rather than values: the title block's 0.25 mm rules print thin
/// while 0.35 mm symbol strokes print mid, so the thin/mid boundary lies inside
/// `(0.25, 0.35)`; 0.70 mm process mains print thick, so the mid/thick boundary
/// lies inside `(0.35, 0.70)`. No reference page prints ink that declares a
/// width between 0.35 and 0.70 — the 0.50 mm strokes this table used to cite as
/// a mid-pen anchor were the drawing's `As Drawn` placeholder, which prints
/// *thin* (see `docs/analysis/2026-10-30-as-drawn-is-not-a-pen.md`) — so the
/// 0.55 mm threshold below is a choice inside that interval, not a measurement.
///
/// The thick/wide boundary sits inside `(0.70, 1.06)`: the CCHB A2 title block
/// declares 1.0606 mm and up, and the plot prints those wide, while every
/// 0.70 mm stroke on the same sheet prints thick. 0.85 mm is a choice inside
/// that interval.
///
/// A width that is not finite or not positive falls back to the mid pen, which
/// is what an unresolved style draws with.
#[must_use]
pub fn pen_width_mm(declared_mm: f64) -> f64 {
    if !declared_mm.is_finite() || declared_mm <= 0.0 {
        return PEN_MID_MM;
    }
    if declared_mm <= 0.30 {
        PEN_THIN_MM
    } else if declared_mm <= 0.55 {
        PEN_MID_MM
    } else if declared_mm <= 0.85 {
        PEN_THICK_MM
    } else {
        PEN_WIDE_MM
    }
}

/// The widest width a drafting pen declares in this corpus, in millimetres.
///
/// The ISO 128 ladder these drawings are authored on tops out at 0.70 mm, which
/// [`pen_width_mm`] snaps onto the thick pen. A sheet template is authored on a
/// different scale and can declare more: the CCHB A2 title block states
/// 1.4141 mm for its heavy rails, and the plot prints it on the wide pen.
/// Past that a width stops being a pen: `SmartPlant` writes its *validation
/// marks* into the same style table, and those carry the mark's own on-screen
/// size instead — `BandAid`, which flags an object an engineer has to look at,
/// states 10 mm.
pub const WIDEST_PEN_MM: f64 = 1.45;

/// Whether a declared width is one a plotter pen can carry.
///
/// False for a width no drafting pen declares — see [`WIDEST_PEN_MM`] — and for
/// the non-positive or non-finite values [`pen_width_mm`] also refuses.
#[must_use]
pub fn carries_a_pen(declared_mm: f64) -> bool {
    declared_mm.is_finite() && declared_mm > 0.0 && declared_mm <= WIDEST_PEN_MM
}

/// One face [`write_plot`] may select for a text run.
pub struct PlotFont<'a> {
    /// Canonical name from [`typeface_key`]. Empty means last resort only.
    pub key: &'a str,
    /// Static TrueType or OpenType bytes, whole or subset.
    pub bytes: &'a [u8],
    /// Whether `bytes` is a subset of the face's file rather than the file.
    ///
    /// ISO 32000-1 §9.6.4 requires a subset's `BaseFont` and `FontName` to be
    /// `ABCDEF+Name`, and a whole program's names to carry no such tag; the
    /// writer can only honour that if it is told which one it was handed.
    pub subset: bool,
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
///
/// What the sheet's edge, rather than the drawing, decided is
/// [`write_plot_audited`]'s answer; this drops it.
pub fn write_plot(model: &PaperModel, fonts: &[PlotFont<'_>]) -> Result<Vec<u8>, PlotWriteError> {
    write_plot_audited(model, fonts).map(|(pdf, _)| pdf)
}

/// What the writer had to do because the sheet ends, not because the drawing
/// says so.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotAudit {
    /// Runs condensed all the way to [`MIN_CONDENSE`] that still run past the
    /// sheet edge, so the page clip takes their tail.
    ///
    /// This count used to be invisible: a label longer than the sheet was
    /// squeezed to half width and then lost whatever still spilled, and
    /// nothing in the PDF or in the export's output said which runs they
    /// were. Half is the floor because a run squashed further is not a label
    /// any more — but a non-zero count is a page whose lettering is not all
    /// on the sheet, and that is a fact the export owes its caller.
    pub runs_clipped_at_the_condensation_floor: usize,
    /// How far past the edge the worst of those runs reaches, in millimetres.
    pub deepest_clip_mm: f64,
    /// Runs the sheet had no room for at the width the drawing set, so they
    /// were condensed along their own direction until they fitted.
    ///
    /// This was silent until now: a run squeezed to 0.8 is still legible, so
    /// nothing said it happened, and no one could answer whether a sheet held
    /// any. The size is not touched — glyphs keep their height and only the
    /// advance shrinks — so [`Self::smallest_condense`] is how far that went.
    pub runs_condensed: usize,
    /// The smallest condensation applied to any run; `1.0` when none was.
    pub smallest_condense: f64,
}

impl Default for PlotAudit {
    fn default() -> Self {
        Self {
            runs_clipped_at_the_condensation_floor: 0,
            deepest_clip_mm: 0.0,
            runs_condensed: 0,
            smallest_condense: 1.0,
        }
    }
}

/// Write `model` to PDF bytes and report what the sheet edge decided.
pub fn write_plot_audited(
    model: &PaperModel,
    fonts: &[PlotFont<'_>],
) -> Result<(Vec<u8>, PlotAudit), PlotWriteError> {
    // A drawing can hold no lettering at all: `D03` in the plant backup is a
    // template with zero primitives, so `prepare_fonts` loads nothing. The
    // page still writes; only a run that needs a face without one is fatal.
    if fonts.is_empty() && has_inked_text(model) {
        return Err(PlotWriteError::Font("no font was provided".into()));
    }
    // The page is the model's own answer, not a second guess here: a drawing
    // that states no sheet still has one number, and the gates read the same
    // one to check the written page against.
    let Some([page_w, page_h]) = model.resolved_page_mm() else {
        return Err(PlotWriteError::NoPage);
    };

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
        if font.subset {
            let Object::Ref(reference) = &font_ref else {
                return Err(PlotWriteError::Font(
                    "embedded font is not a reference".into(),
                ));
            };
            declare_subset(&mut doc, *reference, &family, font.bytes)?;
        }
        let font_dict = doc
            .dict(&font_ref)
            .ok_or_else(|| PlotWriteError::Font("embedded font is not a dictionary".into()))?;
        ready.push(ReadyFace {
            key: font.key,
            resource,
            metrics: Metrics::from_dict(&doc, &font_dict),
            heights: FaceHeights::of(font.bytes),
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
    let mut audit = PlotAudit::default();
    let page = [page_w, page_h];
    push_layer(
        &mut ops, model, &fonts, "decoded", "Decoded", page, &mut audit,
    )?;
    push_layer(
        &mut ops, model, &fonts, "inferred", "Inferred", page, &mut audit,
    )?;

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

    let pdf = pdfcraft_cos::write_full(&doc, &pdfcraft_cos::SaveOptions::default())
        .map_err(|error| PlotWriteError::Save(error.to_string()))?;
    Ok((pdf, audit))
}

/// Declare an embedded program a subset in the PDF's own names.
///
/// ISO 32000-1 §9.6.4: "For a font subset, the PostScript name of the font —
/// the value of the `BaseFont` entry and the font descriptor's `FontName`
/// entry — shall begin with a tag followed by a plus sign", the tag being six
/// upper-case letters. `pdfcraft`'s embedder writes the family name it was
/// given into all three places, so the page said `Arial` about a two-glyph
/// program and `pdffonts` reported `sub no` for every face on the sheet.
///
/// The tag is derived from the program, so the same subset always carries the
/// same tag and two subsets of one face never share one.
fn declare_subset(
    doc: &mut Document,
    font: pdfcraft_cos::ObjRef,
    family: &str,
    bytes: &[u8],
) -> Result<(), PlotWriteError> {
    let tagged = format!("{}+{family}", subset_tag(bytes));
    let Some(top) = doc.dict(&Object::Ref(font)) else {
        return Err(PlotWriteError::Font(
            "embedded font is not a dictionary".into(),
        ));
    };
    // A composite face names the program's own CIDFont as a descendant; a
    // simple one carries its descriptor directly. Both spell `BaseFont`, and
    // the descriptor spells `FontName`.
    let mut targets: Vec<(pdfcraft_cos::ObjRef, &'static [u8])> = vec![(font, b"BaseFont")];
    let descendant = match top.get(b"DescendantFonts") {
        Some(Object::Array(kids)) => kids.first().and_then(|kid| match kid {
            Object::Ref(child) => Some(*child),
            _ => None,
        }),
        _ => None,
    };
    if let Some(cid) = descendant {
        if let Some(Object::Ref(descriptor)) = doc
            .dict(&Object::Ref(cid))
            .and_then(|dict| dict.get(b"FontDescriptor").cloned())
        {
            targets.push((descriptor, b"FontName"));
        }
        targets.push((cid, b"BaseFont"));
    } else if let Some(Object::Ref(descriptor)) = top.get(b"FontDescriptor").cloned() {
        targets.push((descriptor, b"FontName"));
    }
    for (target, key) in targets {
        doc.update_dict(target, |dict| {
            dict.set(key.to_vec(), Object::name(&tagged));
        })
        .map_err(|error| PlotWriteError::Save(error.to_string()))?;
    }
    Ok(())
}

/// The six upper-case letters ISO 32000-1 §9.6.4 asks a subset tag to be.
///
/// FNV-1a over the program: deterministic for the same bytes, different for a
/// different subset, and free of the allocation and dependency a random tag
/// would need — a plot has to be byte-identical between two runs of the same
/// export.
fn subset_tag(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let mut tag = String::with_capacity(6);
    for shift in 0..6 {
        let letter = b'A' + ((hash >> (shift * 5)) % 26) as u8;
        tag.push(char::from(letter));
    }
    tag
}

fn add_ocg(doc: &mut Document, name: &str) -> pdfcraft_cos::ObjRef {
    let mut ocg = Dict::new();
    ocg.set(b"Type".to_vec(), Object::name("OCG"));
    ocg.set(b"Name".to_vec(), Object::String(PdfString::text(name)));
    doc.add(Object::Dict(ocg))
}

/// Whether any run on the page needs a typeface.
///
/// The same rule [`show_text`] applies: a run of spaces letters nothing and is
/// trimmed away, so a page that holds only padding must not ask for a face
/// either.
fn has_inked_text(model: &PaperModel) -> bool {
    model.primitives.iter().any(|primitive| {
        is_plotted(primitive.confidence)
            && matches!(
                &primitive.shape,
                PaperShape::Text { value, .. } if !value.trim().is_empty()
            )
    })
}

/// Whether a primitive's confidence puts it on the page.
///
/// Two layers are painted — `decoded` and `inferred` — and `ProbeOnly` is not:
/// its byte provenance is sound but no reading of its bytes is, so it carries
/// no position and draws nothing. Everything downstream of "what the page
/// holds" has to use this one predicate, or a record that draws nothing still
/// asks the export for a typeface: `0x0030` `JStyleOverride` lettering is
/// `ProbeOnly`, and a page whose only Han characters came from one of those
/// used to make [`crate::fonts::prepare_fonts`] load and embed a face the page
/// never drew.
#[must_use]
pub fn is_plotted(confidence: &str) -> bool {
    confidence == "decoded" || confidence == "inferred"
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
    heights: FaceHeights,
}

/// The three heights of a face a label's insertion line can name, as
/// fractions of its em.
///
/// `ascent` and `descent` are the Windows metrics (`OS/2`
/// `usWinAscent` / `usWinDescent`) — GDI's `tmAscent` / `tmDescent`, which
/// is what the vendor's renderer fills its font cache from — and `cap` is
/// `OS/2` `sCapHeight`, or the top of `H` where the table is too old to state
/// one (`ARIALN.TTF` is version 1). Arial reads 0.905 / 0.212 / 0.716.
#[derive(Clone, Copy, Debug, PartialEq)]
struct FaceHeights {
    ascent: f64,
    descent: f64,
    cap: f64,
}

impl FaceHeights {
    /// Arial's, for a face whose tables cannot be read.
    const ARIAL: Self = Self {
        ascent: 1854.0 / 2048.0,
        descent: 434.0 / 2048.0,
        cap: 1467.0 / 2048.0,
    };

    fn of(bytes: &[u8]) -> Self {
        Self::read(bytes).unwrap_or(Self::ARIAL)
    }

    fn read(bytes: &[u8]) -> Option<Self> {
        use fontcull_read_fonts::{FontRef, TableProvider};
        use fontcull_skrifa::MetadataProvider;
        use fontcull_skrifa::instance::{LocationRef, Size};

        let font = FontRef::new(bytes).ok()?;
        let em = f64::from(font.head().ok()?.units_per_em());
        if em <= 0.0 {
            return None;
        }
        let os2 = font.os2().ok();
        let (ascent, descent) = match &os2 {
            Some(os2) if os2.us_win_ascent() > 0 => (
                f64::from(os2.us_win_ascent()),
                f64::from(os2.us_win_descent()),
            ),
            _ => {
                let hhea = font.hhea().ok()?;
                (
                    f64::from(hhea.ascender().to_i16()),
                    -f64::from(hhea.descender().to_i16()),
                )
            }
        };
        let cap = os2
            .as_ref()
            .and_then(|os2| os2.s_cap_height())
            .filter(|cap| *cap > 0)
            .map(f64::from)
            .or_else(|| {
                let glyph = font.charmap().map('H')?;
                let bounds = font
                    .glyph_metrics(Size::unscaled(), LocationRef::default())
                    .bounds(glyph)?;
                Some(f64::from(bounds.y_max)).filter(|top| *top > 0.0)
            })?;
        let heights = Self {
            ascent: ascent / em,
            descent: descent / em,
            cap: cap / em,
        };
        (heights.ascent > 0.0 && heights.cap > 0.0 && heights.descent >= 0.0).then_some(heights)
    }

    /// How far below the insertion point the baseline runs, as a multiple of
    /// a style height that measures `basis`.
    ///
    /// A cap-height style's middle and top are half and one style height
    /// down by definition; its ascender line is the face's ascent over its cap
    /// (Arial 1.264, Arial Narrow 1.288). An ascent style's ascender is one
    /// style height; its middle and top are the face's cap over its ascent,
    /// halved for the middle (Arial 0.396).
    fn drop(self, line: TextLine, basis: TextHeightBasis) -> f64 {
        let reference = match basis {
            TextHeightBasis::Cap => self.cap,
            TextHeightBasis::Ascent => self.ascent,
        };
        let line_height = match line {
            TextLine::Bottom => 0.0,
            TextLine::Middle => self.cap / 2.0,
            TextLine::Top => self.cap,
            TextLine::Ascender => self.ascent,
            TextLine::Descender => -self.descent,
        };
        line_height / reference
    }
}

fn push_layer(
    ops: &mut Vec<Op>,
    model: &PaperModel,
    fonts: &[ReadyFace<'_>],
    confidence: &str,
    property: &str,
    page: [f64; 2],
    audit: &mut PlotAudit,
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

    for primitive in &primitives {
        paint_primitive(ops, primitive, page);
    }
    for primitive in &primitives {
        if let PaperShape::Text {
            origin_mm,
            height_mm,
            rotation,
            value,
            font,
            anchor,
            line,
            height_basis,
            width_permille,
            right_pad,
            spans,
        } = &primitive.shape
        {
            show_text(
                ops,
                fonts,
                ShownText {
                    typeface: typeface_key(font.as_deref(), value),
                    origin_mm: *origin_mm,
                    height_mm: *height_mm,
                    rotation: *rotation,
                    value,
                    anchor: *anchor,
                    line: *line,
                    height_basis: *height_basis,
                    width_permille: *width_permille,
                    right_pad: *right_pad,
                    spans,
                    furniture: primitive.furniture,
                },
                page,
                audit,
            )?;
        }
    }
    ops.push(Op::new("Q", Vec::new()));
    ops.push(Op::new("EMC", Vec::new()));
    Ok(())
}

/// Paint one non-text primitive in its own pen state: `q`, the stroke's
/// colour/width/dash or the fill's colour, the path, `S` or `f`, `Q`.
/// The layer's `J`/`j` ride inside every saved state, so a one-stroke
/// group keeps the layer's cap and join.
fn paint_primitive(ops: &mut Vec<Op>, primitive: &crate::model::PaperPrimitive, page: [f64; 2]) {
    if let PaperShape::Fill { chain_mm, rgb } = &primitive.shape {
        if chain_mm.len() < 3 || !chain_mm.iter().all(|point| finite(*point)) {
            return;
        }
        let ink = plot_ink(rgb.unwrap_or(PLOT_INK));
        ops.push(Op::new("q", Vec::new()));
        ops.push(Op::new(
            "rg",
            vec![
                Object::Real(f64::from(ink[0]) / 255.0),
                Object::Real(f64::from(ink[1]) / 255.0),
                Object::Real(f64::from(ink[2]) / 255.0),
            ],
        ));
        ops.push(Op::new("m", vec![pt(chain_mm[0][0]), pt(chain_mm[0][1])]));
        for point in &chain_mm[1..] {
            ops.push(Op::new("l", vec![pt(point[0]), pt(point[1])]));
        }
        ops.push(Op::new("h", Vec::new()));
        ops.push(Op::new("f", Vec::new()));
        ops.push(Op::new("Q", Vec::new()));
        return;
    }
    if matches!(primitive.shape, PaperShape::Text { .. }) {
        return;
    }
    let pen = primitive.stroke.clone().unwrap_or(PaperStroke {
        rgb: [0, 0, 0],
        width_mm: STROKE_MM,
        dash_mm: Vec::new(),
    });
    if !pen.width_mm.is_finite() || pen.width_mm <= 0.0 {
        return;
    }
    let ink = plot_ink(pen.rgb);
    ops.push(Op::new("q", Vec::new()));
    ops.push(Op::new(
        "RG",
        vec![
            Object::Real(f64::from(ink[0]) / 255.0),
            Object::Real(f64::from(ink[1]) / 255.0),
            Object::Real(f64::from(ink[2]) / 255.0),
        ],
    ));
    ops.push(Op::new(
        "w",
        vec![Object::Real(pen_width_mm(pen.width_mm) * PT_PER_MM)],
    ));
    if let Some(dash) = dash_array(&pen) {
        ops.push(Op::new("d", vec![Object::Array(dash), Object::Real(0.0)]));
    }
    if paint_shape(ops, &primitive.shape, page, primitive.furniture) {
        ops.push(Op::new("S", Vec::new()));
    }
    ops.push(Op::new("Q", Vec::new()));
}

/// The dash state a pen asks for, or `None` when its pattern inks no dash.
///
/// A PDF dash array's numbers have to be non-negative and **not all zero**.
/// The model keeps a pattern exactly as the record states it, and the D04
/// fixture's own style declares one zero-length segment — a walk over that
/// pattern is solid (`DashWalk` treats a zero period as no dash at all), so
/// the page has to draw it that way too instead of writing the invalid
/// `[0] 0 d` state a stricter viewer than MuPDF may refuse.
fn dash_array(pen: &PaperStroke) -> Option<Vec<Object>> {
    let steps: Vec<f64> = pen
        .dash_mm
        .iter()
        .map(|segment| segment.abs() * PT_PER_MM)
        .collect();
    if steps.iter().any(|step| !step.is_finite()) || steps.iter().all(|step| *step <= 0.0) {
        return None;
    }
    Some(steps.into_iter().map(Object::Real).collect())
}

fn paint_shape(
    ops: &mut Vec<Op>,
    shape: &PaperShape,
    page: [f64; 2],
    keep_sheet_edge: bool,
) -> bool {
    let mut any = false;
    for run in stroke_runs(&shape_chain(shape), page, keep_sheet_edge) {
        ops.push(Op::new("m", vec![pt(run[0][0]), pt(run[0][1])]));
        for point in &run[1..] {
            ops.push(Op::new("l", vec![pt(point[0]), pt(point[1])]));
        }
        any = true;
    }
    any
}

/// The point chain one shape strokes, its closing edge included.
fn shape_chain(shape: &PaperShape) -> Vec<[f64; 2]> {
    match shape {
        PaperShape::Line { start_mm, end_mm } => vec![*start_mm, *end_mm],
        PaperShape::Polyline { points_mm, closed } => {
            let mut points = points_mm.clone();
            if *closed && points.len() >= 2 {
                points.push(points[0]);
            }
            points
        }
        PaperShape::Arc {
            center_mm,
            radius_mm,
            start_angle,
            end_angle,
            clockwise,
        } => {
            let (start, sweep) = directed_arc(*start_angle, *end_angle, *clockwise);
            arc_points(*center_mm, *radius_mm, start, sweep)
        }
        PaperShape::Circle {
            center_mm,
            radius_mm,
        } => arc_points(*center_mm, *radius_mm, 0.0, -TAU),
        PaperShape::Fill { .. } | PaperShape::Text { .. } => Vec::new(),
    }
}

/// The parts of `points` that print, split where a dropped piece breaks the
/// chain.
///
/// One run becomes one subpath, so a dash pattern flows along a whole polyline
/// instead of restarting at every vertex.
fn stroke_runs(points: &[[f64; 2]], page: [f64; 2], keep_sheet_edge: bool) -> Vec<Vec<[f64; 2]>> {
    let mut runs = Vec::new();
    let mut run: Vec<[f64; 2]> = Vec::new();
    for pair in points.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        let prints = finite(start)
            && finite(end)
            && hypot(start, end) >= 1e-4
            && (keep_sheet_edge || !sheet_edge_mm(start, end, page));
        if !prints {
            if run.len() >= 2 {
                runs.push(std::mem::take(&mut run));
            } else {
                run.clear();
            }
            continue;
        }
        if run.is_empty() {
            run.push(start);
        }
        run.push(end);
    }
    if run.len() >= 2 {
        runs.push(run);
    }
    runs
}

/// Byte runs of one label, each with the face that encodes it.
type FaceRuns<'a> = Vec<(&'a ReadyFace<'a>, Vec<u8>)>;

/// Split `value` into runs that share one face, preferring the face
/// `typeface_key` names. A character the named face cannot encode moves to
/// the first face that can — only that character moves, so `120℃` keeps its
/// digits in the narrow face instead of dragging the whole run to a
/// fallback whose Latin is wider.
///
/// A named face that encodes nothing in `value` is a different case. The
/// first face that happens to own one character would cut the label apart
/// (`动火` in SimSun, `点` in an earlier FangSong that has only that
/// character), and the reader then sees two runs where the record states
/// one. The whole string goes to the first face that can draw every
/// character, and that run is reported as borrowed so the caller can pitch
/// it. A named face that draws any character keeps the per-character split.
fn encode_runs<'a>(
    faces: &'a [ReadyFace<'a>],
    key: &str,
    value: &str,
) -> Result<(FaceRuns<'a>, bool), PlotWriteError> {
    let char_encodes = |face: &ReadyFace<'_>, ch: char| {
        let mut buf = [0u8; 4];
        face.metrics.encode(ch.encode_utf8(&mut buf)).is_some()
    };
    let named = faces.iter().find(|face| face.key == key);
    let named_draws_any = named.is_some_and(|face| value.chars().any(|ch| char_encodes(face, ch)));
    if !named_draws_any
        && let Some((face, bytes)) = faces
            .iter()
            .find_map(|face| face.metrics.encode(value).map(|bytes| (face, bytes)))
    {
        return Ok((vec![(face, bytes)], true));
    }
    let face_for_char = |ch: char| -> Option<&'a ReadyFace<'a>> {
        let mut buf = [0u8; 4];
        let text = ch.encode_utf8(&mut buf);
        faces
            .iter()
            .find(|face| face.key == key && face.metrics.encode(text).is_some())
            .or_else(|| {
                faces
                    .iter()
                    .find(|face| face.metrics.encode(text).is_some())
            })
    };
    let mut runs: Vec<(&ReadyFace<'_>, String)> = Vec::new();
    for ch in value.chars() {
        let Some(face) = face_for_char(ch) else {
            let have: Vec<&str> = faces.iter().map(|face| face.key).collect();
            return Err(PlotWriteError::Font(format!(
                "no face can draw {ch:?} in {value:?} (wanted {key}; embedded {})",
                have.join(", ")
            )));
        };
        match runs.last_mut() {
            Some((last_face, text)) if std::ptr::eq(*last_face, face) => text.push(ch),
            _ => runs.push((face, ch.to_string())),
        }
    }
    runs.into_iter()
        .map(|(face, text)| {
            face.metrics
                .encode(&text)
                .map(|bytes| (face, bytes))
                .ok_or_else(|| missing_glyph(&face.metrics, &text))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|runs| (runs, false))
}

struct ShownText<'a> {
    typeface: &'a str,
    origin_mm: [f64; 2],
    height_mm: Option<f64>,
    rotation: f64,
    value: &'a str,
    anchor: TextAnchor,
    line: TextLine,
    height_basis: TextHeightBasis,
    /// The style's character width in thousandths, `JStyleTextChar +52`.
    width_permille: Option<u16>,
    /// Shift the trimmed run forward by the width of `value`'s trailing pad.
    right_pad: bool,
    spans: &'a [PaperTextSpan],
    /// Whether this run is the sheet's furniture — the lettering a linked
    /// `.igr` border template carries, rather than the drawing's own.
    furniture: bool,
}

/// One stretch of a label that letters at its own size in its own face.
struct TextSegment<'a> {
    text: String,
    typeface: &'a str,
    size_pt: f64,
}

/// Split a label into the stretches it letters differently.
///
/// A label with no run partition is one stretch, exactly what this writer
/// drew before spans existed. One that carries a partition is cut where the
/// size or the face changes — **and nowhere else**: two neighbouring runs that
/// resolve to the same size and face stay one run on the page, which keeps a
/// style change indistinguishable from the record's own repetition from
/// adding runs to the PDF (and to the comparison's text pairing).
///
/// The leading and trailing padding `show_text` trims is skipped first, so a
/// span's character offsets — which count from the record's own first
/// character — land on the right glyphs.
fn text_segments<'a>(text: &ShownText<'a>, inked: &str, height_mm: f64) -> Vec<TextSegment<'a>> {
    let base_size = plotted_size_pt(text.typeface, height_mm);
    let plain = || {
        vec![TextSegment {
            text: inked.to_string(),
            typeface: text.typeface,
            size_pt: base_size,
        }]
    };
    if text.spans.is_empty() {
        return plain();
    }
    let chars: Vec<char> = text.value.chars().collect();
    let lead = chars.iter().take_while(|c| c.is_whitespace()).count();
    let inked_len = inked.chars().count();
    let end = lead.saturating_add(inked_len).min(chars.len());
    if end <= lead {
        return plain();
    }
    let mut per_char: Vec<(&'a str, f64)> = vec![(text.typeface, base_size); end - lead];
    for span in text.spans {
        let start = usize::from(span.start);
        let stop = start.saturating_add(usize::from(span.len)).min(chars.len());
        // A run names its face the way the file spelled it — `宋体`, `Arial
        // Narrow` — while `text.typeface` is the face *this writer embeds*
        // (`typeface_key`). Resolving here is what lets a run that names the
        // label's own face merge back into it instead of splitting the label
        // over a spelling the two sides never shared.
        let typeface = match span.font.as_deref() {
            Some(named) => typeface_key(Some(named), text.value),
            None => text.typeface,
        };
        let size_pt = plotted_size_pt(typeface, span.height_mm.unwrap_or(height_mm));
        for index in start..stop {
            if index < lead || index >= end {
                continue;
            }
            per_char[index - lead] = (typeface, size_pt);
        }
    }
    let mut segments: Vec<TextSegment<'a>> = Vec::new();
    for (offset, (typeface, size_pt)) in per_char.iter().enumerate() {
        let ch = inked.chars().nth(offset).unwrap_or_default();
        match segments.last_mut() {
            Some(last) if last.typeface == *typeface && last.size_pt == *size_pt => {
                last.text.push(ch);
            }
            _ => segments.push(TextSegment {
                text: ch.to_string(),
                typeface,
                size_pt: *size_pt,
            }),
        }
    }
    if segments.iter().all(|segment| segment.text.is_empty()) {
        return plain();
    }
    segments
}

/// How far a run may advance from `start` along `direction` before it leaves
/// the page, in millimetres.
///
/// `0` when the start is already outside the sheet, and `f64::INFINITY` only
/// for a zero direction.
fn page_reach_mm(start: [f64; 2], direction: [f64; 2], page: [f64; 2]) -> f64 {
    let mut reach = f64::INFINITY;
    for axis in 0..2 {
        let step = direction[axis];
        if step.abs() < 1e-9 {
            continue;
        }
        let left = if step > 0.0 {
            page[axis] - start[axis]
        } else {
            start[axis]
        };
        reach = reach.min(left / step.abs());
    }
    if reach.is_finite() {
        reach.max(0.0)
    } else {
        reach
    }
}

/// The smallest a run is squeezed to before it is left to the page clip.
///
/// A label the sheet cannot hold is condensed along its own direction until it
/// fits, which is what keeps every glyph on the page; a run that would have to
/// be squashed to half its width is not a label any more, so it keeps its size
/// and loses its tail the way it always did.
const MIN_CONDENSE: f64 = 0.5;

/// Advance a border template's lettering is drawn at, as a fraction of the em
/// its run is set in.
///
/// A sheet's linked `.igr` border letters its labels in the full em the
/// drawing uses (宋体 at 1000 units per glyph), but the reference plot draws
/// them one 0.78 em apart: measured on the two corpus sheets that carry
/// furniture, every template label's per-character advance lands in
/// 0.765..0.791 of its em — `审  定` 0.785, `设计证书号` 0.791, `A111001641`
/// 0.784, `CPENC-DD11008` 0.774, `中国石油…华北分公司` 0.775, `华北油田苏桥储气库群`
/// 0.765, `1 版` 0.770, `会 签 专 业` 0.770 — and every label of a given
/// letter height shares it, on both sheets, while the drawing's own labels are
/// drawn at 1.0.
///
/// **This is not a font size.** The two sides' glyph *heights* already agree
/// exactly (4.74 / 5.08 / 5.93 / 6.60 mm for the template's 3.2 / 3.5 / 4.0 /
/// 4.5 mm styles on both sheets); what differs is only the horizontal advance.
/// A PDF library that reports one `size` per run multiplies the two scales
/// together and lands 1.29 against this crate's 1.47, which is where the
/// "furniture lettering is 12 % larger" reading came from — the run is
/// condensed, not shrunk. The template's styles do state a width —
/// `JStyleTextChar +52` is `800` on every one of them, thousandths of the
/// face's advance (`style.dll` divides it by `1000.0` and never scales it by
/// the placement) — but the reference realises that as 0.765..0.791, not 0.8,
/// so the constant stays the measured figure, like [`plotted_size_pt`]'s
/// table. [`furniture_advance`] reads the stated width and keeps this for
/// `800`.
///
/// See
/// `docs/analysis/2026-10-23-the-furniture-text-height-already-matched.md`.
const PLATE_TEXT_ADVANCE: f64 = 0.78;

/// Advance a border template's run is drawn at, from its style's character
/// width (`JStyleTextChar +52`, thousandths of the face's own).
///
/// Every template `SimSun` style in `CPE-SQ-A0/A1` and `CPECCHB-SQ-A2`
/// states `800`, and the reference letters those at the measured
/// [`PLATE_TEXT_ADVANCE`] — the plotter's own realisation of 0.8, which this
/// crate cannot reproduce from the record. The A2 sign-off column's
/// `A$C40D91A4B.sym` states `1000` and the reference letters it at exactly
/// one em (修改, 审核, 日期, 页次 … all 10.50 mm against our 8.19 at 0.78). Any
/// other stated width is taken at its face value; an unstated one keeps the
/// measured constant, which is what every template letter drew before the
/// width was read.
fn furniture_advance(width_permille: Option<u16>) -> f64 {
    match width_permille {
        Some(800) | None => PLATE_TEXT_ADVANCE,
        Some(width) => f64::from(width) / 1000.0,
    }
}

/// Advance of one glyph, as a fraction of the em, when the named face can
/// draw none of a 2.5 mm label and the whole string is handed to another face.
///
/// The only such labels on the eleven acceptance sheets are the two
/// `动火点` runs. The reference pitches them at 4.32 pt (1.524 mm) in a
/// 9.6 pt NSimSun, and centres on that pitch. `1.524 / 2.5` is that pitch
/// over the style height. A different body size is not multiplied: nothing
/// else on those sheets takes this path, so there is no second height to
/// measure. `120℃` is not this path either — its named face still draws the
/// digits. See
/// `docs/analysis/2026-11-04-the-a2-five-labels-outside-the-bubble.md`.
const BORROWED_HAN_ADVANCE: f64 = 1.524 / 2.5;

fn show_text(
    ops: &mut Vec<Op>,
    faces: &[ReadyFace<'_>],
    text: ShownText<'_>,
    page: [f64; 2],
    audit: &mut PlotAudit,
) -> Result<(), PlotWriteError> {
    if text.value.is_empty() || !text.rotation.is_finite() || !finite(text.origin_mm) {
        return Ok(());
    }
    let height_mm = text.height_mm.unwrap_or(DEFAULT_TEXT_MM);
    if !height_mm.is_finite() || height_mm <= 0.0 {
        return Ok(());
    }
    // A drawing file pads label fields with spaces, on either side
    // (`L=200mm `, ` 1001102A`). They ink nothing, so the reference plot has
    // no run for them at all: it measures and letters the visible glyphs.
    // Keeping the padding costs nothing on the page but pushes a centred or
    // right-anchored run half a space off — the I drawing's 105
    // leading-space labels sat 0.4 mm left of the reference for exactly that
    // reason, and trimming only the tail left them there. So the padding is
    // dropped before the run is measured or drawn.
    let inked = text.value.trim();
    if inked.is_empty() {
        return Ok(());
    }
    // A label that letters in more than one style is cut into the stretches
    // that share a size and a face; a plain label is one stretch.
    let segments = text_segments(&text, inked, height_mm);
    let mut emitted: Vec<(f64, &ReadyFace<'_>, Vec<u8>)> = Vec::new();
    let mut width_mm = 0.0;
    let mut borrowed_whole = !segments.is_empty();
    for segment in &segments {
        let (runs, borrowed) = encode_runs(faces, segment.typeface, &segment.text)?;
        borrowed_whole &= borrowed;
        for (face, bytes) in runs {
            width_mm += run_width_mm(&face.metrics, segment.size_pt, &bytes);
            emitted.push((segment.size_pt, face, bytes));
        }
    }
    let (sin, cos) = text.rotation.sin_cos();
    // The reference plot letters a border template's text one glyph narrower
    // than the em it sets it in, and letters the sheet's own text at the full
    // em. Height is *not* touched by this: the two sides' glyph boxes are the
    // same 4.74 / 5.08 / 5.93 / 6.60 mm for the 3.2 / 3.5 / 4.0 / 4.5 mm
    // template styles on both sheets. See [`PLATE_TEXT_ADVANCE`].
    //
    // A 2.5 mm string whose named face draws nothing is the A2 `动火点`
    // class: the reference pitches it at 1.524 mm a glyph and centres on
    // that width. Any other body, or a string the named face still partly
    // draws (`120℃`), stays one em. See [`BORROWED_HAN_ADVANCE`].
    let han_pitch = if !text.furniture && borrowed_whole && (height_mm - 2.5).abs() < 1e-3 {
        BORROWED_HAN_ADVANCE
    } else {
        1.0
    };
    let plate = if text.furniture {
        furniture_advance(text.width_permille)
    } else {
        1.0
    } * han_pitch;
    let laid_out_mm = width_mm * plate;
    // A left anchor is the left of the stored run, padding included. A
    // right-aligned paragraph letters the trimmed glyphs at the right of
    // that run, so the pad's own advance is added along the baseline and
    // the pad itself is not drawn. The two L M line numbers `25-CI-…-B8- `
    // start one Arial space to the right of the insertion; the same
    // paragraph with no trailing pad stays on it.
    let pad_mm = if text.right_pad && text.anchor == TextAnchor::Left {
        let kept = text.value.trim_end();
        let pad = &text.value[kept.len()..];
        emitted
            .last()
            .and_then(|(size_pt, face, _)| {
                face.metrics
                    .encode(pad)
                    .map(|bytes| run_width_mm(&face.metrics, *size_pt, &bytes) * plate)
            })
            .unwrap_or(0.0)
    } else {
        0.0
    };
    let origin_mm = [
        text.origin_mm[0] + cos * pad_mm,
        text.origin_mm[1] + sin * pad_mm,
    ];
    // The insertion line is a line of the face the label names, so its depth
    // is read off that face; a label whose named face draws none of it takes
    // the face that does.
    let heights = faces
        .iter()
        .find(|face| face.key == text.typeface)
        .or_else(|| emitted.first().map(|(_, face, _)| *face))
        .map_or(FaceHeights::ARIAL, |face| face.heights);
    let below_mm = height_mm * heights.drop(text.line, text.height_basis);
    // A run the page still cannot hold loses its tail to the page box, so it
    // is condensed along its own direction until it fits: the template states
    // the label and the page states its edge, and keeping every glyph on the
    // sheet beats inventing a smaller size for the letters. Furniture reaches
    // this rule only if it overflows even at [`PLATE_TEXT_ADVANCE`] — the I
    // drawing's 21-glyph company name did, back when it was drawn at the full
    // em and lost its last two characters to the sheet edge.
    let mut condense = 1.0;
    if laid_out_mm > 0.0 && laid_out_mm.is_finite() {
        let start = baseline_origin(origin_mm, text.rotation, laid_out_mm, below_mm, text.anchor);
        let reach = page_reach_mm(start, [cos, sin], page);
        if reach > 0.0 && laid_out_mm > reach {
            condense = (reach / laid_out_mm).clamp(MIN_CONDENSE, 1.0);
            if condense < 1.0 {
                audit.runs_condensed += 1;
                audit.smallest_condense = audit.smallest_condense.min(condense);
            }
            // Even half width leaves it hanging over the edge, so the page
            // clip takes the tail. Said out loud in [`PlotAudit`]: a run whose
            // last glyphs are off the sheet is a defect of the page, and the
            // PDF alone cannot tell anyone it happened.
            let held_mm = laid_out_mm * MIN_CONDENSE;
            if held_mm > reach {
                audit.runs_clipped_at_the_condensation_floor += 1;
                audit.deepest_clip_mm = audit.deepest_clip_mm.max(held_mm - reach);
            }
        }
    }
    let scale = plate * condense;
    let origin_mm = baseline_origin(
        origin_mm,
        text.rotation,
        width_mm * scale,
        below_mm,
        text.anchor,
    );
    ops.push(Op::new("BT", Vec::new()));
    ops.push(Op::new(
        "Tm",
        vec![
            Object::Real(cos * scale),
            Object::Real(sin * scale),
            Object::Real(-sin),
            Object::Real(cos),
            pt(origin_mm[0]),
            pt(origin_mm[1]),
        ],
    ));
    // `Tj` advances the text position itself, so consecutive runs continue
    // along the same baseline — across a change of face, and across a change
    // of size, since the advance is the glyph width at the size in force.
    for (size, face, bytes) in &emitted {
        ops.push(Op::new(
            "Tf",
            vec![Object::name(&face.resource), Object::Real(*size)],
        ));
        ops.push(Op::new(
            "Tj",
            vec![Object::String(PdfString::literal(bytes.clone()))],
        ));
    }
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

/// Left end of the baseline, from the insertion point.
///
/// Centre and right step back **along** the run by half the word or the whole
/// word; the insertion line steps **down across** it by `below_mm`
/// ([`FaceHeights::drop`] of the style height). The insertion point itself
/// never moves.
fn baseline_origin(
    origin_mm: [f64; 2],
    rotation: f64,
    width_mm: f64,
    below_mm: f64,
    anchor: TextAnchor,
) -> [f64; 2] {
    let along = match anchor {
        TextAnchor::Left => 0.0,
        TextAnchor::Center => 0.5,
        TextAnchor::Right => 1.0,
    };
    let back = if width_mm.is_finite() && width_mm > 0.0 {
        width_mm * along
    } else {
        0.0
    };
    let below = if below_mm.is_finite() { below_mm } else { 0.0 };
    if back == 0.0 && below == 0.0 {
        return origin_mm;
    }
    let (sin, cos) = rotation.sin_cos();
    [
        origin_mm[0] - cos * back + sin * below,
        origin_mm[1] - sin * back - cos * below,
    ]
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

/// A circle's flattened outline — one full clockwise turn, the same way
/// [`paint_shape`] draws it. Shared with the comparator.
pub(crate) fn circle_outline(center_mm: [f64; 2], radius_mm: f64) -> Vec<[f64; 2]> {
    arc_points(center_mm, radius_mm, 0.0, -TAU)
}

/// The minor-clockwise arc as a flattened vertex chain — shared with the
/// comparator so `model_strokes` attributes an arc's ink the same way the
/// writer draws it.
pub(crate) fn arc_chain(
    center_mm: [f64; 2],
    radius_mm: f64,
    start_angle: f64,
    end_angle: f64,
    clockwise: bool,
) -> Vec<[f64; 2]> {
    let (start, sweep) = directed_arc(start_angle, end_angle, clockwise);
    arc_points(center_mm, radius_mm, start, sweep)
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

/// The sweep one arc states: `(start, sweep)` with `sweep` signed the way
/// [`kurbo::Arc`] wants it -- negative clockwise.
///
/// `clockwise` is the source record's own trailing byte. The two readings are
/// complements that reach the same end angle; an arc the record means the long
/// way round must not be folded back to the short one, which is what a
/// minor-sweep normalisation did before the byte was read.
pub(crate) fn directed_arc(start: f64, end: f64, clockwise: bool) -> (f64, f64) {
    let sweep = if clockwise {
        -(start - end).rem_euclid(TAU)
    } else {
        (end - start).rem_euclid(TAU)
    };
    if sweep.abs() < 1e-12 {
        (start, 0.0)
    } else {
        (start, sweep)
    }
}

/// The reader's sheet-frame rule, in millimetres. A line this drops is not ink.
fn reader_sheet_edge(start: [f64; 2], end: [f64; 2], page: [f64; 2]) -> bool {
    sheet_edge_mm(start, end, page)
}

/// Straight segments `write_plot` strokes, Decoded then Inferred. ProbeOnly is omitted.
///
/// Arc and circle chords use the same 0.05 mm tolerance as the reader, and a
/// dashed pen contributes only the pieces its pattern inks — the same walk the
/// reader takes, so a read-back matches this list endpoint for endpoint.
#[must_use]
pub fn stroked_segments(model: &PaperModel) -> Vec<([f64; 2], [f64; 2])> {
    let Some(page) = model.resolved_page_mm() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for confidence in ["decoded", "inferred"] {
        for primitive in &model.primitives {
            if primitive.confidence != confidence {
                continue;
            }
            // A fill reads back as its outline, the closing edge included —
            // the reader's `ClosePath` segment.
            if let PaperShape::Fill { chain_mm, .. } = &primitive.shape {
                push_chain(&mut out, chain_mm, page);
                if chain_mm.len() >= 2 {
                    push_segment(&mut out, chain_mm[chain_mm.len() - 1], chain_mm[0], page);
                }
                continue;
            }
            let dash: Vec<f64> = primitive
                .stroke
                .as_ref()
                .map(|pen| pen.dash_mm.clone())
                .unwrap_or_default();
            let mut walk = DashWalk::new(&dash, 0.0);
            for run in stroke_runs(&shape_chain(&primitive.shape), page, false) {
                walk.restart();
                for pair in run.windows(2) {
                    for (start, end) in walk.split(pair[0], pair[1]) {
                        out.push((start, end));
                    }
                }
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
///
/// A symbol body's own label is the one place this table deliberately does
/// *not* follow the reference. Those labels are 7.2 pt style heights, and
/// the plots write them at 8.25 pt — 14.6% larger. But the face this crate
/// can actually load for `Arial` inks 14.6% wider per character than the
/// plots' does, so at 8.25 pt our runs come out 0.11 body heights wider than
/// the reference's and the whole `symbol/Middle` class drifts 0.075 along
/// (measured on the I drawing: the ratchet's along gate is 0.05). At the
/// 7.2 pt style height the printed extent matches and the class holds at
/// 0.035, which is why no row is measured in here for it.
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
        // The A2 sign-off column: 2.5 / 3.0 mm symbol lettering at a 1.414
        // placement scale, which the reference letters at 5.25 / 6.27 mm.
        ("SimSun", 1002, 14.88),
        ("SimSun", 1203, 17.76),
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
    use super::{
        PEN_MID_MM, PEN_THICK_MM, PEN_THIN_MM, STROKE_MM, ShownText, pen_width_mm, plotted_size_pt,
        text_segments, write_plot,
    };
    use crate::model::{
        PaperModel, PaperPrimitive, PaperShape, PaperTextSpan, TextAnchor, TextLine,
    };
    use crate::read::read_plot;

    use super::PlotFont;

    fn primitive(confidence: &'static str, shape: PaperShape) -> PaperPrimitive {
        PaperPrimitive {
            confidence,
            stream_path: None,
            byte_range: None,
            symbol: None,
            stroke: None,
            shape,
            furniture: false,
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
    fn declared_widths_snap_onto_the_four_plot_pens() {
        use super::{PEN_MID_MM, PEN_THICK_MM, PEN_THIN_MM, PEN_WIDE_MM, pen_width_mm};
        // The I drawing's own ladder: 0.13 / 0.18 / 0.35 / 0.70 mm.
        for thin in [0.13, 0.18, 0.25, 0.30] {
            assert_eq!(pen_width_mm(thin), PEN_THIN_MM, "{thin} mm");
        }
        for mid in [0.35, 0.50, 0.55] {
            assert_eq!(pen_width_mm(mid), PEN_MID_MM, "{mid} mm");
        }
        for thick in [0.70, 0.85] {
            assert_eq!(pen_width_mm(thick), PEN_THICK_MM, "{thick} mm");
        }
        // The CCHB A2 title block's heavy rails, and anything above them,
        // print on the wide pen.
        for wide in [1.0, 1.0606, 1.1313, 1.4141, 2.0] {
            assert_eq!(pen_width_mm(wide), PEN_WIDE_MM, "{wide} mm");
        }
        // A width that is not a width draws the fallback pen.
        for nonsense in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(pen_width_mm(nonsense), PEN_MID_MM, "{nonsense}");
        }
        // 0.169 / 0.339 / 0.678 / 1.017 mm are 0.48 / 0.96 / 1.92 / 2.88 pt,
        // the only four widths the reference pages carry.
        let points = |mm: f64| mm * 72.0 / 25.4;
        assert!((points(PEN_THIN_MM) - 0.48).abs() < 0.005);
        assert!((points(PEN_MID_MM) - 0.96).abs() < 0.005);
        assert!((points(PEN_THICK_MM) - 1.92).abs() < 0.005);
        assert!((points(PEN_WIDE_MM) - 2.88).abs() < 0.005);
    }

    #[test]
    fn a_validation_marks_width_is_not_a_pen() {
        use super::{WIDEST_PEN_MM, carries_a_pen, pen_width_mm};
        // Every width a drafting pen in this corpus declares is a pen — the
        // CCHB A2 title block's 1.0606 / 1.1313 / 1.4141 mm rules included —
        // and the top of the ladder prints on the widest pen.
        for pen in [
            0.10, 0.13, 0.18, 0.25, 0.35, 0.50, 0.70, 1.0, 1.0606, 1.1313, 1.4141,
        ] {
            assert!(carries_a_pen(pen), "{pen} mm is a pen");
            assert!(pen_width_mm(pen) >= super::PEN_THIN_MM, "{pen} mm");
        }
        assert!((pen_width_mm(WIDEST_PEN_MM) - super::PEN_WIDE_MM).abs() < 1e-9);
        assert!((pen_width_mm(0.70) - super::PEN_THICK_MM).abs() < 1e-9);
        // `BandAid` is the 10 mm band SmartPlant paints over a flagged object;
        // its size is the mark's, not a pen's.
        for mark in [WIDEST_PEN_MM + 0.01, 10.0] {
            assert!(!carries_a_pen(mark), "{mark} mm is a validation mark");
        }
        // The same values `pen_width_mm` refuses are not pens either.
        for nonsense in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(!carries_a_pen(nonsense), "{nonsense}");
        }
    }

    #[test]
    fn a_white_pen_is_the_plots_wipe_and_survives_the_monochrome_table() {
        use super::plot_ink;
        // The reference pages carry two to sixteen thin white strokes, each
        // about a millimetre long: the plot's wipe. A white pen has to reach
        // the page white even though every other colour is black.
        assert_eq!(plot_ink([255, 255, 255]), [255, 255, 255]);
        for colour in [[128, 128, 0], [0, 128, 0], [128, 0, 0], [0, 0, 255]] {
            assert_eq!(plot_ink(colour), [0, 0, 0], "{colour:?}");
        }
        // Nearly white is not white: the wipe is the one exact value.
        assert_eq!(plot_ink([254, 255, 255]), [0, 0, 0]);
    }

    #[test]
    fn the_page_is_plotted_in_the_three_pens() {
        // A sheet declares its widths on the ISO 128 ladder; the page carries
        // the plot's three pens instead. 0.13 mm thin, 0.70 mm thick.
        let mut thin = primitive(
            "decoded",
            PaperShape::Line {
                start_mm: [10.0, 20.0],
                end_mm: [80.0, 20.0],
            },
        );
        thin.stroke = Some(super::PaperStroke {
            rgb: [128, 128, 0],
            width_mm: 0.13,
            dash_mm: Vec::new(),
        });
        let mut thick = primitive(
            "decoded",
            PaperShape::Line {
                start_mm: [10.0, 40.0],
                end_mm: [80.0, 40.0],
            },
        );
        thick.stroke = Some(super::PaperStroke {
            rgb: [0, 128, 0],
            width_mm: 0.70,
            dash_mm: Vec::new(),
        });
        let model = PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some([200.0, 100.0]),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            primitives: vec![thin, thick],
            missing_symbols: Vec::new(),
            missing_templates: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
        };
        let pdf = write_plot(
            &model,
            &[PlotFont {
                key: "",
                bytes: pdfcraft_fonts::INTER_REGULAR,
                subset: false,
            }],
        )
        .expect("write");
        let ink = read_plot(&pdf).expect("read");
        let mut widths: Vec<f64> = ink
            .segments
            .iter()
            .map(|segment| (segment.width_mm * 100.0).round() / 100.0)
            .collect();
        widths.sort_by(f64::total_cmp);
        widths.dedup();
        assert_eq!(widths, vec![0.17, 0.68], "the pens are not snapped");
    }

    /// A style can state a dash whose only segment is zero — the plant
    /// backup's `D04` does. A PDF dash array may not be all zero, and a
    /// zero-period pattern inks solid, so the page has to carry no dash state
    /// at all: `[0] 0 d` is invalid PDF that a stricter viewer than MuPDF may
    /// refuse to draw.
    #[test]
    fn a_zero_length_dash_pattern_is_written_solid() {
        let page = |dash_mm: Vec<f64>| {
            let mut line = primitive(
                "decoded",
                PaperShape::Line {
                    start_mm: [10.0, 20.0],
                    end_mm: [80.0, 20.0],
                },
            );
            line.stroke = Some(super::PaperStroke {
                rgb: [0, 0, 0],
                width_mm: 0.35,
                dash_mm,
            });
            PaperModel {
                library: Vec::new(),
                template: None,
                page_mm: Some([200.0, 100.0]),
                off_sheet_primitives: 0,
                skipped_endpoint_pairs: 0,
                primitives: vec![line],
                missing_symbols: Vec::new(),
                missing_templates: Vec::new(),
                missing_boxes: Vec::new(),
                symbols_found: 0,
                symbols_referenced: 0,
            }
        };
        let solid = write_plot(&page(Vec::new()), &[]).expect("solid");
        let zero = write_plot(&page(vec![0.0]), &[]).expect("zero-period dash");
        assert_eq!(
            zero, solid,
            "a zero-period dash has to draw the solid pen's page"
        );
        let dashed = write_plot(&page(vec![3.5, 1.75]), &[]).expect("dashed");
        assert_ne!(dashed, solid, "a real dash still reaches the page");
    }

    /// `D03` in the plant backup is a drawing with a declared sheet and no
    /// content at all: `prepare_fonts` loads no face for it, and the page used
    /// to be refused with `no font was provided`.
    #[test]
    fn a_drawing_with_no_ink_at_all_still_writes_its_declared_sheet() {
        let model = PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some([594.33, 420.57]),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            primitives: Vec::new(),
            missing_symbols: Vec::new(),
            missing_templates: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
        };
        let pdf = write_plot(&model, &[]).expect("an empty page still writes");
        let ink = read_plot(&pdf).expect("read");
        assert!((ink.page_mm[0] - 594.33).abs() < 0.05, "{ink:?}");
        assert!((ink.page_mm[1] - 420.57).abs() < 0.05, "{ink:?}");
        assert!(ink.segments.is_empty() && ink.spans.is_empty(), "{ink:?}");
    }

    /// Lettering with no loaded face is still a failure: a page that silently
    /// dropped its labels would look like a blank success.
    #[test]
    fn lettering_without_a_face_is_refused() {
        let model = PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some([200.0, 100.0]),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            primitives: vec![primitive(
                "decoded",
                PaperShape::Text {
                    origin_mm: [12.0, 40.0],
                    height_mm: Some(5.0),
                    rotation: 0.0,
                    value: "Ab".into(),
                    font: None,
                    anchor: TextAnchor::Left,
                    line: TextLine::Bottom,
                    height_basis: crate::model::TextHeightBasis::Cap,
                    width_permille: None,
                    right_pad: false,
                    spans: Vec::new(),
                },
            )],
            missing_symbols: Vec::new(),
            missing_templates: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
        };
        assert!(write_plot(&model, &[]).is_err());
    }

    #[test]
    fn a_line_and_a_word_read_back_on_their_layers() {
        let model = PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some([200.0, 100.0]),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
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
                        line: TextLine::Bottom,
                        height_basis: crate::model::TextHeightBasis::Cap,
                        width_permille: None,
                        right_pad: false,
                        spans: Vec::new(),
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
            missing_templates: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
        };
        let pdf = write_plot(
            &model,
            &[PlotFont {
                key: "",
                bytes: pdfcraft_fonts::INTER_REGULAR,
                subset: false,
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
                TextLine::Bottom,
            )
        };
        let vertical = |anchor| {
            placed(
                "WW",
                [20.0 + f64::from(anchor_row(anchor)), 80.0],
                std::f64::consts::FRAC_PI_2,
                anchor,
                TextLine::Bottom,
            )
        };
        let left_h = horizontal(TextAnchor::Left);
        let centre_h = horizontal(TextAnchor::Center);
        let right_h = horizontal(TextAnchor::Right);
        let left_v = vertical(TextAnchor::Left);
        // A bottom anchor is the only one the horizontal cases use, so the
        // rows below measure the along-the-run step alone.
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

    /// The vertical half of the anchor: the label letters below the point.
    ///
    /// The reference plots put the baseline 0.00 mm / 1.25 mm / 3.26 mm below
    /// a 2.5 mm body's insertion point, one mode per class, so the step is
    /// measured against the body size and not against the pen. A sheet top
    /// stops on the face's ascender line: its ascent over its cap height of
    /// a cap-height body.
    #[test]
    fn middle_and_top_step_down_across_the_baseline() {
        let inter = super::FaceHeights::of(pdfcraft_fonts::INTER_REGULAR);
        let bottom = placed("WW", [80.0, 20.0], 0.0, TextAnchor::Left, TextLine::Bottom);
        let middle = placed("WW", [80.0, 20.0], 0.0, TextAnchor::Left, TextLine::Middle);
        let top = placed("WW", [80.0, 20.0], 0.0, TextAnchor::Left, TextLine::Top);
        let ascender = placed(
            "WW",
            [80.0, 20.0],
            0.0,
            TextAnchor::Left,
            TextLine::Ascender,
        );
        assert!((bottom[1] - 20.0).abs() < 0.05, "bottom {}", bottom[1]);
        assert!((middle[1] - 17.5).abs() < 0.05, "middle {}", middle[1]);
        assert!((top[1] - 15.0).abs() < 0.05, "top {}", top[1]);
        assert!(
            (ascender[1] - (20.0 - 5.0 * inter.ascent / inter.cap)).abs() < 0.05,
            "ascender {}",
            ascender[1]
        );
        for (name, point) in [
            ("bottom", bottom),
            ("middle", middle),
            ("top", top),
            ("ascender", ascender),
        ] {
            assert!((point[0] - bottom[0]).abs() < 0.05, "{name} moved along x");
        }
        // Rotated a quarter turn the run climbs +Y, so the step lands on +X.
        // `placed` states a 5 mm body, so the middle step is 2.5 mm.
        let turned = std::f64::consts::FRAC_PI_2;
        let upright = placed(
            "WW",
            [20.0, 80.0],
            turned,
            TextAnchor::Left,
            TextLine::Bottom,
        );
        let dropped = placed(
            "WW",
            [20.0, 80.0],
            turned,
            TextAnchor::Left,
            TextLine::Middle,
        );
        assert!(
            (dropped[0] - (upright[0] + 2.5)).abs() < 0.1,
            "turned {dropped:?}"
        );
        assert!((dropped[1] - upright[1]).abs() < 0.1, "turned {dropped:?}");
    }

    /// The insertion line's depth is a height of the face, measured in the
    /// height the style states.
    ///
    /// A cap-height style (`0x0D`, every drawing-sheet label) hangs its middle
    /// and top half and one style height down by definition, and its ascender
    /// line at Arial's 0.905 / 0.716 = 1.264 — the 1.267 the A0/A1 sheets'
    /// 192 Arial ascender labels measure. An ascent style (`0x0C`, the valve
    /// and motor `M`) is one style height to its ascender line, and its
    /// middle is Arial's cap over twice its ascent, 0.396 — the reference's
    /// `M` baselines sit 0.37–0.45 style heights below their insertion.
    #[test]
    fn the_line_depth_is_a_height_of_the_face() {
        use crate::model::TextHeightBasis::{Ascent, Cap};
        let arial = super::FaceHeights::ARIAL;
        let close = |got: f64, want: f64| (got - want).abs() < 2e-3;
        assert!(close(arial.drop(TextLine::Bottom, Cap), 0.0));
        assert!(close(arial.drop(TextLine::Middle, Cap), 0.5));
        assert!(close(arial.drop(TextLine::Top, Cap), 1.0));
        assert!(close(arial.drop(TextLine::Ascender, Cap), 1.264));
        assert!(close(arial.drop(TextLine::Descender, Cap), -0.296));
        assert!(close(arial.drop(TextLine::Middle, Ascent), 0.396));
        assert!(close(arial.drop(TextLine::Top, Ascent), 0.791));
        assert!(close(arial.drop(TextLine::Ascender, Ascent), 1.0));
    }

    /// The heights come off the face's own tables, with the top of `H`
    /// standing in for a cap height an old `OS/2` does not state.
    #[test]
    fn face_heights_read_the_face_tables() {
        let inter = super::FaceHeights::of(pdfcraft_fonts::INTER_REGULAR);
        assert_ne!(inter, super::FaceHeights::ARIAL);
        assert!(inter.cap > 0.6 && inter.cap < inter.ascent, "{inter:?}");
        assert!(inter.descent > 0.0, "{inter:?}");
        // Bytes that are no font fall back to Arial's.
        assert_eq!(
            super::FaceHeights::of(b"not a font"),
            super::FaceHeights::ARIAL
        );
    }

    /// A padded label anchors on its visible glyphs alone, on either side.
    ///
    /// The drawing files pad their fields with spaces — `H=11.8MPa ` with a
    /// trailing one, ` 1001102A` with a leading one. A pad inks nothing, so
    /// measuring it in steps a centred run half a space back and a
    /// right-aligned run a whole one. Trimming only the tail left the I
    /// drawing's 105 leading-space labels 0.4 mm left of the reference; the
    /// comparison's width column now reports what a run-width difference can
    /// explain and what only a placement rule can.
    #[test]
    fn padding_at_either_end_is_dropped_before_the_anchor_is_measured() {
        for anchor in [TextAnchor::Center, TextAnchor::Right] {
            let padded = placed(" WW ", [80.0, 20.0], 0.0, anchor, TextLine::Bottom);
            let tight = placed("WW", [80.0, 20.0], 0.0, anchor, TextLine::Bottom);
            assert!(
                (padded[0] - tight[0]).abs() < 0.05,
                "{anchor:?}: padded {padded:?} tight {tight:?}"
            );
        }
    }

    #[test]
    fn the_page_reach_follows_the_run_direction() {
        let page = [100.0, 50.0];
        assert!((super::page_reach_mm([20.0, 10.0], [1.0, 0.0], page) - 80.0).abs() < 1e-9);
        assert!((super::page_reach_mm([20.0, 10.0], [-1.0, 0.0], page) - 20.0).abs() < 1e-9);
        assert!((super::page_reach_mm([20.0, 10.0], [0.0, 1.0], page) - 40.0).abs() < 1e-9);
        // A run that starts off the sheet has nowhere to go.
        assert_eq!(super::page_reach_mm([120.0, 10.0], [1.0, 0.0], page), 0.0);
    }

    /// A run the sheet cannot hold is condensed along its own direction rather
    /// than running off the page.
    ///
    /// The I drawing's title-block company name is 21 full-width glyphs at the
    /// height its `.igr` template states, and the face this writer embeds
    /// advances them 13 % wider than the reference plot's face did: the last
    /// two characters fell past the sheet edge and MuPDF extracted 19 of them.
    #[test]
    fn a_run_wider_than_the_page_is_condensed_until_it_fits() {
        let page = [100.0, 40.0];
        let value = "A".repeat(60);
        let model = PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some(page),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            primitives: vec![primitive(
                "decoded",
                PaperShape::Text {
                    origin_mm: [10.0, 10.0],
                    height_mm: Some(4.0),
                    rotation: 0.0,
                    value: value.clone(),
                    font: None,
                    anchor: TextAnchor::Left,
                    line: TextLine::Bottom,
                    height_basis: crate::model::TextHeightBasis::Cap,
                    width_permille: None,
                    right_pad: false,
                    spans: Vec::new(),
                },
            )],
            missing_symbols: Vec::new(),
            missing_templates: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
        };
        let pdf = write_plot(
            &model,
            &[PlotFont {
                key: "",
                bytes: pdfcraft_fonts::INTER_REGULAR,
                subset: false,
            }],
        )
        .expect("write");
        let ink = read_plot(&pdf).expect("read");
        let read: String = ink.spans.iter().map(|span| span.text.as_str()).collect();
        assert_eq!(read, value, "every glyph stayed on the page");
        let end = ink
            .spans
            .iter()
            .map(|span| span.origin_mm[0] + span.advance_mm)
            .fold(f64::MIN, f64::max);
        assert!(
            end <= page[0] + 0.05,
            "the run ends {end:.2} mm along a {} mm page",
            page[0]
        );
        assert!(
            ink.spans[0].height_mm > 3.9,
            "condensing is horizontal; the body stays {}",
            ink.spans[0].height_mm
        );
    }

    /// A left anchor stays on the insertion when its trailing pad is only
    /// storage. A right-aligned paragraph letters the trimmed glyphs at the
    /// right of that stored run, so the same pad moves the run by its own
    /// width — and a second space moves it twice as far. L M's
    /// `25-CI-…-B8- ` line numbers are that case.
    #[test]
    fn a_right_aligned_pad_shifts_the_trimmed_run_by_the_pad() {
        let shape = |value: &str, y: f64, rotation: f64, right_pad: bool| PaperShape::Text {
            origin_mm: [40.0, y],
            height_mm: Some(3.175),
            rotation,
            value: value.to_string(),
            font: None,
            anchor: TextAnchor::Left,
            line: TextLine::Bottom,
            height_basis: crate::model::TextHeightBasis::Cap,
            width_permille: None,
            right_pad,
            spans: Vec::new(),
        };
        let model = PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some([200.0, 200.0]),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            primitives: vec![
                primitive("decoded", shape("AB", 40.0, 0.0, false)),
                primitive("decoded", shape("AB ", 60.0, 0.0, false)),
                primitive("decoded", shape("AB ", 80.0, 0.0, true)),
                primitive("decoded", shape("AB  ", 100.0, 0.0, true)),
                primitive(
                    "decoded",
                    shape("AB ", 140.0, std::f64::consts::FRAC_PI_2, true),
                ),
            ],
            missing_symbols: Vec::new(),
            missing_templates: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
        };
        let pdf = write_plot(
            &model,
            &[PlotFont {
                key: "",
                bytes: pdfcraft_fonts::INTER_REGULAR,
                subset: false,
            }],
        )
        .expect("write");
        let ink = read_plot(&pdf).expect("read");
        let at = |y: f64| {
            ink.spans
                .iter()
                .find(|span| (span.origin_mm[1] - y).abs() < 0.5)
                .unwrap_or_else(|| panic!("no run near y = {y}: {:?}", ink.spans))
        };
        let plain = at(40.0);
        let kept = at(60.0);
        let one = at(80.0);
        let two = at(100.0);
        let turned = ink
            .spans
            .iter()
            .find(|span| span.direction[1].abs() > 0.9)
            .expect("vertical run");
        assert!((kept.origin_mm[0] - plain.origin_mm[0]).abs() < 0.01);
        let space = one.origin_mm[0] - plain.origin_mm[0];
        assert!(space > 0.2, "one trailing space moves the run: {space}");
        let twice = two.origin_mm[0] - plain.origin_mm[0];
        assert!(
            (twice - 2.0 * space).abs() < 0.05,
            "two spaces move twice one: {twice} vs {space}"
        );
        assert!((turned.origin_mm[0] - 40.0).abs() < 0.05);
        assert!(
            (turned.origin_mm[1] - (140.0 + space)).abs() < 0.05,
            "a vertical run takes the same pad along its baseline: {:?}",
            turned.origin_mm
        );
    }

    /// A template's lettering is drawn one narrower glyph than the sheet's
    /// own, at the same height.
    ///
    /// The reference plots the two the same way: its `.igr` labels advance
    /// 0.78 of their em while the drawing's labels advance the full em, and
    /// neither moves the body size. The two runs here are the same character
    /// at the same height, so only the horizontal scale can separate them.
    #[test]
    fn furniture_lettering_is_condensed_horizontally_and_keeps_its_height() {
        let value = "ABCD";
        let shape = |y: f64| PaperShape::Text {
            origin_mm: [20.0, y],
            height_mm: Some(3.0),
            rotation: 0.0,
            value: value.to_string(),
            font: None,
            anchor: TextAnchor::Left,
            line: TextLine::Bottom,
            height_basis: crate::model::TextHeightBasis::Cap,
            width_permille: None,
            right_pad: false,
            spans: Vec::new(),
        };
        let mut model = PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some([841.0, 594.0]),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            primitives: vec![
                primitive("decoded", shape(40.0)),
                primitive("decoded", shape(50.0)),
            ],
            missing_symbols: Vec::new(),
            missing_templates: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
        };
        model.primitives[1].furniture = true;
        let pdf = write_plot(
            &model,
            &[PlotFont {
                key: "",
                bytes: pdfcraft_fonts::INTER_REGULAR,
                subset: false,
            }],
        )
        .expect("write");
        let ink = read_plot(&pdf).expect("read");
        assert_eq!(ink.spans.len(), 2, "two labels, two runs: {:?}", ink.spans);
        let at = |y: f64| {
            ink.spans
                .iter()
                .find(|span| (span.origin_mm[1] - y).abs() < 0.01)
                .unwrap_or_else(|| panic!("no run at y = {y}: {:?}", ink.spans))
        };
        let sheet = at(40.0);
        let plate = at(50.0);
        let ratio = plate.advance_mm / sheet.advance_mm;
        assert!(
            (ratio - 0.78).abs() < 0.01,
            "furniture letters at {ratio:.3} of the sheet's advance"
        );
        assert!(
            (plate.height_mm - sheet.height_mm).abs() < 0.01,
            "the body size is not what is condensed: {} vs {}",
            plate.height_mm,
            sheet.height_mm
        );
    }

    /// A template label whose style states its own width letters at that
    /// width: `1000` is the face's own advance, `800` stays at the measured
    /// plate advance, and a sheet label ignores the field.
    #[test]
    fn a_template_label_letters_at_the_width_its_style_states() {
        let shape = |y: f64, width_permille: Option<u16>| PaperShape::Text {
            origin_mm: [20.0, y],
            height_mm: Some(3.0),
            rotation: 0.0,
            value: "ABCD".to_string(),
            font: None,
            anchor: TextAnchor::Left,
            line: TextLine::Bottom,
            height_basis: crate::model::TextHeightBasis::Cap,
            width_permille,
            right_pad: false,
            spans: Vec::new(),
        };
        let mut model = PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some([841.0, 594.0]),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            primitives: vec![
                primitive("decoded", shape(40.0, None)),
                primitive("decoded", shape(50.0, Some(1000))),
                primitive("decoded", shape(60.0, Some(800))),
                primitive("decoded", shape(70.0, Some(900))),
                primitive("decoded", shape(80.0, Some(1000))),
            ],
            missing_symbols: Vec::new(),
            missing_templates: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
        };
        for primitive in &mut model.primitives[1..4] {
            primitive.furniture = true;
        }
        let pdf = write_plot(
            &model,
            &[PlotFont {
                key: "",
                bytes: pdfcraft_fonts::INTER_REGULAR,
                subset: false,
            }],
        )
        .expect("write");
        let ink = read_plot(&pdf).expect("read");
        let advance = |y: f64| {
            ink.spans
                .iter()
                .find(|span| (span.origin_mm[1] - y).abs() < 0.01)
                .unwrap_or_else(|| panic!("no run at y = {y}: {:?}", ink.spans))
                .advance_mm
        };
        let sheet = advance(40.0);
        for (y, expected) in [(50.0, 1.0), (60.0, 0.78), (70.0, 0.9), (80.0, 1.0)] {
            let ratio = advance(y) / sheet;
            assert!(
                (ratio - expected).abs() < 0.01,
                "the run at y = {y} letters at {ratio:.3} of the sheet's advance, not {expected}"
            );
        }
    }

    /// A symbol label that letters one character at another size is cut where
    /// the size changes, and the pieces continue along one baseline: `Tj`
    /// advances by the glyph width at the size in force, so the second piece
    /// starts exactly where the first ended rather than at the label's origin.
    #[test]
    fn a_span_at_its_own_size_continues_where_the_previous_one_ended() {
        let model = PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some([200.0, 100.0]),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            primitives: vec![primitive(
                "decoded",
                PaperShape::Text {
                    origin_mm: [20.0, 40.0],
                    height_mm: Some(5.0),
                    rotation: 0.0,
                    value: "AB".into(),
                    font: None,
                    anchor: TextAnchor::Left,
                    line: TextLine::Bottom,
                    height_basis: crate::model::TextHeightBasis::Cap,
                    width_permille: None,
                    right_pad: false,
                    spans: vec![PaperTextSpan {
                        start: 0,
                        len: 1,
                        height_mm: Some(10.0),
                        font: None,
                    }],
                },
            )],
            missing_symbols: Vec::new(),
            missing_templates: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
        };
        let pdf = write_plot(
            &model,
            &[PlotFont {
                key: "",
                bytes: pdfcraft_fonts::INTER_REGULAR,
                subset: false,
            }],
        )
        .expect("write");
        let ink = read_plot(&pdf).expect("read");
        assert_eq!(ink.spans.len(), 2, "one span per size: {:?}", ink.spans);
        let (first, second) = (&ink.spans[0], &ink.spans[1]);
        assert_eq!(first.text, "A");
        assert_eq!(second.text, "B");
        assert!(
            (first.height_mm - second.height_mm * 2.0).abs() < 0.05,
            "the first character is drawn twice the body: {:?}",
            ink.spans
        );
        let expected = first.origin_mm[0] + first.advance_mm;
        assert!(
            (second.origin_mm[0] - expected).abs() < 0.05,
            "the second span starts where the first ended: {:?}",
            ink.spans
        );
    }

    /// A run that only changes the **face** is cut too, at the same size:
    /// `Wastewater Pit.sym` letters one `*` in SimSun inside an Arial label, and
    /// the writer has to ask for that face on that stretch alone.
    #[test]
    fn a_span_in_another_face_is_cut_at_the_same_size() {
        let spans = [
            PaperTextSpan {
                start: 0,
                len: 4,
                height_mm: Some(2.822),
                font: Some("Arial".into()),
            },
            PaperTextSpan {
                start: 4,
                len: 1,
                height_mm: Some(3.175),
                font: Some("\u{5b8b}\u{4f53}".into()),
            },
            PaperTextSpan {
                start: 5,
                len: 3,
                height_mm: Some(2.822),
                font: Some("Arial".into()),
            },
        ];
        let text = ShownText {
            typeface: "Arial",
            origin_mm: [20.0, 40.0],
            height_mm: Some(2.822),
            rotation: 0.0,
            value: "NULL*NUL",
            anchor: TextAnchor::Left,
            line: TextLine::Bottom,
            height_basis: crate::model::TextHeightBasis::Cap,
            width_permille: None,
            right_pad: false,
            spans: &spans,
            furniture: false,
        };
        let segments = text_segments(&text, "NULL*NUL", 2.822);
        let shape: Vec<(&str, &str)> = segments
            .iter()
            .map(|segment| (segment.text.as_str(), segment.typeface))
            .collect();
        assert_eq!(
            shape,
            vec![("NULL", "Arial"), ("*", "SimSun"), ("NUL", "Arial")],
            "the partition is cut where size or face changes, and each run's \
             face resolves to the one this writer embeds"
        );
        assert!(
            segments[1].size_pt > segments[0].size_pt,
            "the middle stretch letters larger: {} vs {}",
            segments[1].size_pt,
            segments[0].size_pt
        );
    }

    /// Two neighbouring runs that resolve to the same size and face are one
    /// run on the page: a partition that changes nothing must not split the
    /// label (and hand the comparison two runs where the record states one).
    #[test]
    fn spans_that_change_nothing_are_written_as_one_run() {
        let model = PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some([200.0, 100.0]),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            primitives: vec![primitive(
                "decoded",
                PaperShape::Text {
                    origin_mm: [20.0, 40.0],
                    height_mm: Some(5.0),
                    rotation: 0.0,
                    value: "AB".into(),
                    font: None,
                    anchor: TextAnchor::Left,
                    line: TextLine::Bottom,
                    height_basis: crate::model::TextHeightBasis::Cap,
                    width_permille: None,
                    right_pad: false,
                    spans: vec![
                        PaperTextSpan {
                            start: 0,
                            len: 1,
                            height_mm: Some(5.0),
                            font: None,
                        },
                        PaperTextSpan {
                            start: 1,
                            len: 1,
                            height_mm: Some(5.0),
                            font: None,
                        },
                    ],
                },
            )],
            missing_symbols: Vec::new(),
            missing_templates: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
        };
        let pdf = write_plot(
            &model,
            &[PlotFont {
                key: "",
                bytes: pdfcraft_fonts::INTER_REGULAR,
                subset: false,
            }],
        )
        .expect("write");
        let ink = read_plot(&pdf).expect("read");
        assert_eq!(ink.spans.len(), 1, "one run on the page: {:?}", ink.spans);
        assert_eq!(ink.spans[0].text, "AB");
    }

    /// A run that is only spaces inks nothing, so it is not written at all.
    #[test]
    fn a_run_of_spaces_writes_no_text() {
        let model = PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some([200.0, 100.0]),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            primitives: vec![primitive(
                "decoded",
                PaperShape::Text {
                    origin_mm: [12.0, 40.0],
                    height_mm: Some(5.0),
                    rotation: 0.0,
                    value: "   ".into(),
                    font: None,
                    anchor: TextAnchor::Center,
                    line: TextLine::Middle,
                    height_basis: crate::model::TextHeightBasis::Cap,
                    width_permille: None,
                    right_pad: false,
                    spans: Vec::new(),
                },
            )],
            missing_symbols: Vec::new(),
            missing_templates: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
        };
        let pdf = write_plot(
            &model,
            &[PlotFont {
                key: "",
                bytes: pdfcraft_fonts::INTER_REGULAR,
                subset: false,
            }],
        )
        .expect("write");
        let ink = read_plot(&pdf).expect("read");
        assert!(
            ink.spans.iter().all(|span| span.text.trim().is_empty()),
            "{:?}",
            ink.spans
        );
    }

    /// A page whose only "text" is spaces letters nothing, so it needs no
    /// face: `has_inked_text` must read the rule `show_text` applies, not a
    /// second one of its own.
    #[test]
    fn a_page_of_spaces_needs_no_face() {
        let model = PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some([200.0, 100.0]),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            primitives: vec![primitive(
                "decoded",
                PaperShape::Text {
                    origin_mm: [12.0, 40.0],
                    height_mm: Some(5.0),
                    rotation: 0.0,
                    value: "   ".into(),
                    font: None,
                    anchor: TextAnchor::Center,
                    line: TextLine::Middle,
                    height_basis: crate::model::TextHeightBasis::Cap,
                    width_permille: None,
                    right_pad: false,
                    spans: Vec::new(),
                },
            )],
            missing_symbols: Vec::new(),
            missing_templates: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
        };
        let pdf = write_plot(&model, &[]).expect("no run letters, so no face is needed");
        let ink = read_plot(&pdf).expect("read");
        assert!(ink.spans.is_empty(), "{:?}", ink.spans);
    }

    /// The page's default pen is one of the three the reference plotter used.
    ///
    /// The writer states a `w` once per page before any stroke, and the compare
    /// side reads those statements as the page's pen vocabulary. Every page
    /// used to announce `0.25 mm` there — a fourth width no reference page
    /// carries — even though the next stroke always overrode it.
    #[test]
    fn the_page_default_pen_is_one_of_the_three() {
        for pen in [PEN_THIN_MM, PEN_MID_MM, PEN_THICK_MM] {
            assert_eq!(pen_width_mm(pen), pen, "a table pen must survive the snap");
        }
        assert_eq!(
            pen_width_mm(STROKE_MM),
            STROKE_MM,
            "the default pen is a width the table does not carry"
        );
    }

    fn anchor_row(anchor: TextAnchor) -> u8 {
        match anchor {
            TextAnchor::Left => 0,
            TextAnchor::Center => 24,
            TextAnchor::Right => 48,
        }
    }

    fn placed(
        value: &str,
        origin_mm: [f64; 2],
        rotation: f64,
        anchor: TextAnchor,
        line: TextLine,
    ) -> [f64; 2] {
        let model = PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some([200.0, 140.0]),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            primitives: vec![primitive(
                "decoded",
                PaperShape::Text {
                    origin_mm,
                    height_mm: Some(5.0),
                    rotation,
                    value: value.into(),
                    font: None,
                    anchor,
                    line,
                    height_basis: crate::model::TextHeightBasis::Cap,
                    width_permille: None,
                    right_pad: false,
                    spans: Vec::new(),
                },
            )],
            missing_symbols: Vec::new(),
            missing_templates: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
        };
        let pdf = write_plot(
            &model,
            &[PlotFont {
                key: "",
                bytes: pdfcraft_fonts::INTER_REGULAR,
                subset: false,
            }],
        )
        .expect("write");
        let ink = read_plot(&pdf).expect("read");
        let span = ink
            .spans
            .iter()
            .find(|span| span.text == value.trim())
            .unwrap_or_else(|| panic!("{value:?} missing in {:?}", ink.spans));
        span.origin_mm
    }

    fn near(point: [f64; 2], expected: [f64; 2]) -> bool {
        (point[0] - expected[0]).abs() < 0.05 && (point[1] - expected[1]).abs() < 0.05
    }

    #[test]
    fn directed_arc_takes_the_side_the_record_names() {
        use super::{TAU, directed_arc};
        // Counter-clockwise is the short way here: 0.2614 rad, leaves the
        // start unchanged.
        let (s, sweep) = directed_arc(1.1659, 1.4273, false);
        assert!((s - 1.1659).abs() < 1e-12);
        assert!((sweep - (1.4273 - 1.1659)).abs() < 1e-6);
        // Clockwise is the rest of the circle, not a folded-back complement.
        let (s_cw, sweep_cw) = directed_arc(1.1659, 1.4273, true);
        assert!((s_cw - 1.1659).abs() < 1e-12);
        assert!((sweep_cw + TAU - (1.4273 - 1.1659)).abs() < 1e-6);

        // Both readings of a semicircle end on the same two ends and differ
        // only in which half is drawn -- the ultrasonic wave's two records.
        let (_, cw_half) = directed_arc(
            std::f64::consts::FRAC_PI_2,
            3.0 * std::f64::consts::FRAC_PI_2,
            true,
        );
        let (_, ccw_half) = directed_arc(
            std::f64::consts::FRAC_PI_2,
            3.0 * std::f64::consts::FRAC_PI_2,
            false,
        );
        assert!((cw_half + std::f64::consts::PI).abs() < 1e-9);
        assert!((ccw_half - std::f64::consts::PI).abs() < 1e-9);

        // A zero sweep is still no ink, and a pi-ulp semicircle is not a full
        // turn.
        assert_eq!(directed_arc(1.0, 1.0, true).1, 0.0);
        assert_eq!(directed_arc(1.0, 1.0, false).1, 0.0);
        let pi_plus_ulp = 3.141_592_653_589_794;
        let (s_pi, sweep_pi) = directed_arc(pi_plus_ulp, 0.0, true);
        assert!((s_pi - pi_plus_ulp).abs() < 1e-12);
        assert!((sweep_pi + std::f64::consts::PI).abs() < 1e-9);
    }

    /// FangSong carrying only `点` and SimSun carrying `动火点` used to cut an
    /// Arial Narrow label into `动火` plus `点`, because the earlier face won
    /// the one character it had. The named face encodes none of the
    /// characters, so the whole label stays on SimSun. `120℃` still splits:
    /// the digits stay on Arial Narrow.
    #[test]
    fn a_han_label_the_named_face_cannot_draw_stays_one_run() {
        let corpus = std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
            || std::path::PathBuf::from(r"D:\work\cad\PID自研软件验证"),
            std::path::PathBuf::from,
        );
        let dirs = crate::fonts::font_dirs(Some(&corpus));
        let ready = |name: &str| dirs.iter().any(|dir| dir.join(name).is_file());
        if !ready("simsun.ttf") || !ready("simfang.ttf") || !ready("ARIALN.TTF") {
            eprintln!("skipping; FangSong, SimSun or Arial Narrow is absent");
            return;
        }
        let label = |value: &str, font: &str, y: f64| {
            primitive(
                "decoded",
                PaperShape::Text {
                    origin_mm: [40.0, y],
                    height_mm: Some(2.5),
                    rotation: 0.0,
                    value: value.to_string(),
                    font: Some(font.to_string()),
                    anchor: TextAnchor::Left,
                    line: TextLine::Bottom,
                    height_basis: crate::model::TextHeightBasis::Cap,
                    width_permille: None,
                    right_pad: false,
                    spans: Vec::new(),
                },
            )
        };
        let model = PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some([200.0, 200.0]),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            primitives: vec![
                label("动火点", "SimSun", 20.0),
                label("点", "FangSong", 40.0),
                label("动火点", "Arial Narrow", 80.0),
                label("120℃", "Arial Narrow", 120.0),
            ],
            missing_symbols: Vec::new(),
            missing_templates: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
        };
        let prepared = crate::fonts::prepare_fonts(&[&model], &dirs).expect("fonts");
        let fonts: Vec<PlotFont<'_>> = prepared
            .iter()
            .map(|face| PlotFont {
                key: &face.key,
                bytes: &face.bytes,
                subset: face.subset,
            })
            .collect();
        let pdf = write_plot(&model, &fonts).expect("write");
        let ink = read_plot(&pdf).expect("read");
        let texts: Vec<&str> = ink.spans.iter().map(|span| span.text.as_str()).collect();
        assert_eq!(
            texts.iter().filter(|text| **text == "动火点").count(),
            2,
            "the borrowed label stays one run: {texts:?}"
        );
        assert!(
            !texts.contains(&"动火"),
            "the label is not cut before its last character: {texts:?}"
        );
        assert_eq!(
            texts.iter().filter(|text| **text == "点").count(),
            1,
            "only the FangSong seed letters 点 on its own: {texts:?}"
        );
        assert!(
            texts.contains(&"120") && texts.iter().any(|text| text.contains('℃')),
            "digits stay on the named face: {texts:?}"
        );
    }

    /// The two A2 `动火点` labels are centred. The reference pitches that
    /// borrowed run at 1.524 mm a glyph, so the baseline steps back by
    /// `3 × 1.524 / 2` mm rather than by half of three ems. `120℃` is centred
    /// in the same test and still steps back by its own em: the named face
    /// draws the digits, so the pitch does not apply.
    #[test]
    fn a_centred_borrowed_han_run_steps_back_by_the_reference_pitch() {
        let corpus = std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
            || std::path::PathBuf::from(r"D:\work\cad\PID自研软件验证"),
            std::path::PathBuf::from,
        );
        let dirs = crate::fonts::font_dirs(Some(&corpus));
        let ready = |name: &str| dirs.iter().any(|dir| dir.join(name).is_file());
        if !ready("simsun.ttf") || !ready("simfang.ttf") || !ready("ARIALN.TTF") {
            eprintln!("skipping; FangSong, SimSun or Arial Narrow is absent");
            return;
        }
        let label = |value: &str, font: &str, x: f64, y: f64, anchor: TextAnchor| {
            primitive(
                "decoded",
                PaperShape::Text {
                    origin_mm: [x, y],
                    height_mm: Some(2.5),
                    rotation: 0.0,
                    value: value.to_string(),
                    font: Some(font.to_string()),
                    anchor,
                    line: TextLine::Bottom,
                    height_basis: crate::model::TextHeightBasis::Cap,
                    width_permille: None,
                    right_pad: false,
                    spans: Vec::new(),
                },
            )
        };
        let model = PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some([200.0, 200.0]),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            primitives: vec![
                label("动火点", "SimSun", 40.0, 20.0, TextAnchor::Left),
                label("点", "FangSong", 40.0, 40.0, TextAnchor::Left),
                label("动火点", "Arial Narrow", 100.0, 80.0, TextAnchor::Center),
                label("120℃", "Arial Narrow", 40.0, 120.0, TextAnchor::Left),
                label("120℃", "Arial Narrow", 100.0, 160.0, TextAnchor::Center),
            ],
            missing_symbols: Vec::new(),
            missing_templates: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
        };
        let prepared = crate::fonts::prepare_fonts(&[&model], &dirs).expect("fonts");
        let fonts: Vec<PlotFont<'_>> = prepared
            .iter()
            .map(|face| PlotFont {
                key: &face.key,
                bytes: &face.bytes,
                subset: face.subset,
            })
            .collect();
        let pdf = write_plot(&model, &fonts).expect("write");
        let ink = read_plot(&pdf).expect("read");
        let borrowed = ink
            .spans
            .iter()
            .find(|span| span.text == "动火点" && span.origin_mm[0] > 70.0)
            .expect("the centred 动火点");
        let pitched = 100.0 - 3.0 * 1.524 / 2.0;
        let full_em = 100.0 - 3.0 * 2.5 / 2.0;
        assert!(
            (borrowed.origin_mm[0] - pitched).abs() < 0.05,
            "left edge {:.3} mm, pitched {pitched:.3}, full em {full_em:.3}",
            borrowed.origin_mm[0]
        );
        let digits = ink
            .spans
            .iter()
            .find(|span| span.text == "120" && span.origin_mm[0] < 70.0)
            .expect("left-anchored digits");
        let degree = ink
            .spans
            .iter()
            .find(|span| {
                span.text.contains('℃') && (span.origin_mm[1] - digits.origin_mm[1]).abs() < 1.0
            })
            .expect("the degree that follows the left-anchored digits");
        let centred_digits = ink
            .spans
            .iter()
            .find(|span| span.text == "120" && span.origin_mm[0] > 70.0)
            .expect("centred digits");
        let em_width = degree.origin_mm[0] + degree.advance_mm - digits.origin_mm[0];
        let retreat = 100.0 - centred_digits.origin_mm[0];
        assert!(
            (retreat - em_width / 2.0).abs() < 0.05,
            "120℃ retreats {retreat:.3} mm, half its em is {:.3}",
            em_width / 2.0
        );
        assert!(
            (retreat - em_width / 2.0 * super::BORROWED_HAN_ADVANCE).abs() > 0.3,
            "120℃ must not take the borrowed pitch"
        );
    }
}
