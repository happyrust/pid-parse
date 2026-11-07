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
//! A style height is written at the `Tf` the reference plotter would size it
//! at: the em it implies, its cell rounded to whole 150 dpi pixels (see
//! `plotted_size_pt`). Every face must be a static TrueType or
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
use crate::model::{
    DashFit, PaperModel, PaperShape, PaperStroke, TextAnchor, TextHeightBasis, TextLine,
};
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
///
/// `stated_cap` is `sCapHeight` alone, with no glyph standing in for it: the
/// plotter sizes a cap-height style from the stated figure, and a face that
/// states none takes [`UNSTATED_CAP_EM`] (see [`plotted_size_pt`]).
#[derive(Clone, Copy, Debug, PartialEq)]
struct FaceHeights {
    ascent: f64,
    descent: f64,
    cap: f64,
    stated_cap: Option<f64>,
    /// `OS/2` `xAvgCharWidth`, as a fraction of the em — GDI's `ntmAvgWidth`.
    /// Zero where the table has none (`0` is also what an absent table
    /// reads, so [`permille_scale`]'s `wf` rule treats both alike). See
    /// [`REFERENCE_CAP_PX`].
    avg_em: f64,
}

impl FaceHeights {
    /// Arial's, for a face whose tables cannot be read.
    const ARIAL: Self = Self {
        ascent: 1854.0 / 2048.0,
        descent: 434.0 / 2048.0,
        cap: 1467.0 / 2048.0,
        stated_cap: Some(1467.0 / 2048.0),
        avg_em: 904.0 / 2048.0,
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
        let stated_cap = os2
            .as_ref()
            .and_then(|os2| os2.s_cap_height())
            .filter(|cap| *cap > 0)
            .map(f64::from);
        let cap = stated_cap.or_else(|| {
            let glyph = font.charmap().map('H')?;
            let bounds = font
                .glyph_metrics(Size::unscaled(), LocationRef::default())
                .bounds(glyph)?;
            Some(f64::from(bounds.y_max)).filter(|top| *top > 0.0)
        })?;
        let avg_em = os2
            .as_ref()
            .map(|os2| f64::from(os2.x_avg_char_width()))
            .filter(|avg| *avg > 0.0)
            .map_or(0.0, |avg| avg / em);
        let heights = Self {
            ascent: ascent / em,
            descent: descent / em,
            cap: cap / em,
            stated_cap: stated_cap.map(|cap| cap / em),
            avg_em,
        };
        (heights.ascent > 0.0 && heights.cap > 0.0 && heights.descent >= 0.0).then_some(heights)
    }

    /// How far below the insertion point the baseline runs, as a multiple of
    /// a style height that measures `basis`, for the face keyed `key`.
    ///
    /// `render.dll` reads every line off the same 72-unit reference font its
    /// scale `a8` divides the style height by ([`reference_scale`]): the
    /// middle is halfway between that font's ascent and descent, the top its
    /// cap, the ascender its ascent and the descender its descent below, each
    /// over the reading `basis` names. A face [`REFERENCE_CAP_PX`] carries
    /// reads GDI's own pixels and the descent is the rest of the 72-pixel
    /// cell; any other face puts its own tables on that cell, as
    /// [`reference_scale`] does. So Arial's cap-height middle is 22/46 =
    /// 0.478 of the style height, not half, and SimSun's and FangSong's, 62
    /// of whose 72 pixels are ascent, are 0.528 and 0.539.
    fn drop(self, key: &str, line: TextLine, basis: TextHeightBasis) -> f64 {
        let cell = (self.ascent + self.descent).max(1e-9);
        let cap72 = reference_cap_px(key, TextHeightBasis::Cap)
            .unwrap_or(72.0 * self.stated_cap.unwrap_or(UNSTATED_CAP_EM) / cell);
        let asc72 =
            reference_cap_px(key, TextHeightBasis::Ascent).unwrap_or(72.0 * self.ascent / cell);
        let desc72 = 72.0 - asc72;
        let line72 = match line {
            TextLine::Bottom => 0.0,
            TextLine::Middle => (asc72 - desc72) / 2.0,
            TextLine::Top => cap72,
            TextLine::Ascender => asc72,
            TextLine::Descender => -desc72,
        };
        let measures = match basis {
            TextHeightBasis::Cap => cap72,
            TextHeightBasis::Ascent => asc72,
        };
        if measures > 0.0 {
            line72 / measures
        } else {
            0.0
        }
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
            paragraph,
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
                    paragraph: *paragraph,
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
        dash_fit: None,
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
    if let Some(fit) = fitted_dash(&pen, &primitive.shape) {
        paint_fitted_pieces(ops, fit, &primitive.shape, page, primitive.furniture);
        ops.push(Op::new("Q", Vec::new()));
        return;
    }
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

/// The fit a pen lays its dash out with on `shape`, or `None` for a pattern
/// that flows unfitted along the whole run.
///
/// Only straight pieces are fitted: an arc reaches the page as chords, and a
/// chord is not a piece `style.dll` would lay a pattern out on.
fn fitted_dash<'a>(pen: &'a PaperStroke, shape: &PaperShape) -> Option<&'a DashFit> {
    let fit = pen.dash_fit.as_ref()?;
    dash_array(pen)?;
    matches!(shape, PaperShape::Line { .. } | PaperShape::Polyline { .. }).then_some(fit)
}

/// Each straight piece as its own subpath and its own dash state, so the
/// pattern starts on every vertex and is stretched to that piece's length.
fn paint_fitted_pieces(
    ops: &mut Vec<Op>,
    fit: &DashFit,
    shape: &PaperShape,
    page: [f64; 2],
    keep_sheet_edge: bool,
) {
    for run in stroke_runs(&shape_chain(shape), page, keep_sheet_edge) {
        for pair in run.windows(2) {
            let steps: Vec<Object> = fit
                .pattern_mm(hypot(pair[0], pair[1]))
                .into_iter()
                .map(|step| Object::Real(step * PT_PER_MM))
                .collect();
            ops.push(Op::new("d", vec![Object::Array(steps), Object::Real(0.0)]));
            ops.push(Op::new("m", vec![pt(pair[0][0]), pt(pair[0][1])]));
            ops.push(Op::new("l", vec![pt(pair[1][0]), pt(pair[1][1])]));
            ops.push(Op::new("S", Vec::new()));
        }
    }
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
/// Each run's face, its encoded bytes, and the original text those bytes
/// encode — [`run_advance`] needs the text back to read the Latin 72-pixel
/// tables by codepoint, since the bytes carry this writer's own assigned
/// CID instead.
type FaceRuns<'a> = Vec<(&'a ReadyFace<'a>, Vec<u8>, String)>;

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
        return Ok((vec![(face, bytes, value.to_string())], true));
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
                .map(|bytes| (face, bytes, text.clone()))
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
    /// Where the paragraph sets the trimmed run inside the box `value`'s
    /// whole stored run spans; `None` is the anchor's own place.
    paragraph: Option<TextAnchor>,
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
/// Whatever padding `show_text` dropped in front of `inked` is skipped first,
/// so a span's character offsets — which count from the record's own first
/// character — land on the right glyphs.
fn text_segments<'a>(
    faces: &[ReadyFace<'_>],
    text: &ShownText<'a>,
    inked: &str,
    height_mm: f64,
) -> Vec<TextSegment<'a>> {
    let size_in = |typeface: &str, height_mm: f64| {
        // The named face is sized by its own tables; a named face the
        // embedded set does not carry sizes off whichever face
        // [`encode_runs`] actually draws it with (the first face able to
        // encode the string), because `Tf`'s size and the reference-font
        // model in `run_advance` both read `height_basis` off *a* face's
        // own `FaceHeights`, and the two have to be the same face or the
        // size and the glyph widths stop agreeing. Only a face list with
        // nothing able to draw the string at all falls back to
        // [`FaceHeights::ARIAL`].
        let heights = faces
            .iter()
            .find(|face| face.key == typeface)
            .or_else(|| {
                faces
                    .iter()
                    .find(|face| face.metrics.encode(text.value).is_some())
            })
            .map_or(FaceHeights::ARIAL, |face| face.heights);
        plotted_size_pt(heights, text.height_basis, height_mm)
    };
    let base_size = size_in(text.typeface, height_mm);
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
    let leading = |value: &str| value.chars().take_while(|c| c.is_whitespace()).count();
    let lead = leading(text.value).saturating_sub(leading(inked));
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
        let size_pt = size_in(typeface, span.height_mm.unwrap_or(height_mm));
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

/// Advance a run with a stated character width (`JStyleTextChar +52`,
/// thousandths of the face's own) is drawn at, as a horizontal scale on the
/// em its run is set in.
///
/// `None` and `1000` both leave the device font's proportional widths
/// alone — `render.dll` only builds a replacement `lfWidth` when the
/// stated width is some other value
/// (`docs/findings/2026-11-07-text-geometry-writer.md` §2: "`wf` 为 1 或未写
/// 时，两者都是 0"). Otherwise the plotter derives a *pixel* target width
/// from the face's own average-advance-over-cell ratio (`OS/2
/// xAvgCharWidth`, [`FaceHeights::avg_em`]) the same way it derives
/// `lfHeight` ([`whole_pixel_cell`]), and the horizontal scale this run is
/// set at is that pixel target divided by what the glyphs would measure at
/// their own proportional widths:
///
/// ```text
/// lw_ref = trunc(72 × avg/cell × w/1000)
/// lfW    = round(lw_ref × a8)
/// sx     = lfW / (lfH × avg/cell)
/// ```
///
/// This is why a template's `SimSun` furniture (every stated width here is
/// `800`) does not letter at a clean 0.8: the truncation and the two
/// roundings land at 0.742..0.797 across its nine style heights
/// (3.0..7.778 mm), matching the reference plot glyph for glyph, where a
/// flat 0.8 over- or under-shoots by up to a pixel over a long label. A
/// style with no readable `avg_em` (an unembeddable font whose `OS/2` table
/// could not be read) falls back to the stated fraction at its face value,
/// the same as `1000`.
fn permille_scale(
    key: &str,
    heights: FaceHeights,
    basis: TextHeightBasis,
    height_mm: f64,
    width_permille: Option<u16>,
) -> f64 {
    let Some(width) = width_permille.filter(|w| *w != 1000) else {
        return 1.0;
    };
    let cell = heights.ascent + heights.descent;
    let avg_over_cell = heights.avg_em / cell;
    let Some(cell_px) = whole_pixel_cell(heights, basis, height_mm) else {
        return f64::from(width) / 1000.0;
    };
    if !(avg_over_cell > 0.0 && cell_px > 0.0) {
        return f64::from(width) / 1000.0;
    }
    let a8 = reference_scale(key, heights, basis, height_mm);
    let lw_ref = (72.0 * avg_over_cell * f64::from(width) / 1000.0).trunc();
    let lf_w = (lw_ref * a8).round();
    let target = cell_px * avg_over_cell;
    if target > 0.0 { lf_w / target } else { 1.0 }
}

/// The 72-unit reference font's own narrowing, `lw_ref / (72 × avg/cell)`,
/// for a style whose character width ([`permille_scale`]) is not `1000`.
///
/// This is a different ratio from [`permille_scale`]'s `sx` — `lw_ref`
/// rather than the device font's rounded `lfW`, `72` rather than the device
/// font's own rounded `lfH` — because the reference font `render.dll`
/// measures a string's advances against is a second, smaller device font
/// request of its own ([`reference_advance_px`]), not the one the glyphs
/// are finally drawn with.
fn wf_reference_scale(heights: FaceHeights, width_permille: Option<u16>) -> f64 {
    let Some(width) = width_permille.filter(|w| *w != 1000) else {
        return 1.0;
    };
    let cell = heights.ascent + heights.descent;
    let avg_over_cell = heights.avg_em / cell;
    if avg_over_cell <= 0.0 {
        return 1.0;
    }
    let lw_ref = (72.0 * avg_over_cell * f64::from(width) / 1000.0).trunc();
    let denom = 72.0 * avg_over_cell;
    if denom > 0.0 { lw_ref / denom } else { 1.0 }
}

/// Advance of one glyph, as a fraction of the em, when the named face can
/// draw none of a 2.5 mm label and the whole string is handed to another face.
///
/// The only such labels on the eleven acceptance sheets are the two
/// `动火点` runs. The reference pitches them at 4.32 pt (1.524 mm) in a
/// 9.6 pt NSimSun, and centres on that pitch. `1.524 / 2.5` is that pitch
/// over the style height, and [`show_text`] applies it to the em the run is
/// set in, so the pitch stays 1.524 mm whatever `Tf` the run carries. A
/// different body size is not multiplied: nothing
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
    // (`L=200mm `, ` 1001102A`). The two pads are not the same thing to the
    // plotter. A leading pad is part of the line: the reference run starts
    // with the space glyph, the space takes its `lpDx` like any other glyph
    // (` 1001501B` reads `Tc −0.210`, the nine-glyph model's `−0.2098`;
    // trimmed to eight glyphs it would be `−0.251`), and the width a centred
    // or right anchor steps back by includes it. A trailing pad is never
    // lettered — no reference run ends in a space — so only the tail is
    // dropped here; what that pad does to the placement is the paragraph's
    // business (`right_pad`). See
    // `docs/analysis/2026-11-07-every-label-miss-is-a-reference-font-rule-or-the-pixel-grid.md`.
    let inked = text.value.trim_end();
    if inked.trim_start().is_empty() {
        return Ok(());
    }
    // A label that letters in more than one style is cut into the stretches
    // that share a size and a face; a plain label is one stretch. `encode_runs`
    // itself runs first, ahead of sizing, because the width scale below
    // ([`BORROWED_HAN_ADVANCE`]) needs its `borrowed` reading.
    let segments = text_segments(faces, &text, inked, height_mm);
    let mut runs: Vec<(f64, &ReadyFace<'_>, Vec<u8>, String)> = Vec::new();
    let mut borrowed_whole = !segments.is_empty();
    for segment in &segments {
        let (segment_runs, borrowed) = encode_runs(faces, segment.typeface, &segment.text)?;
        borrowed_whole &= borrowed;
        for (face, bytes, run_text) in segment_runs {
            runs.push((segment.size_pt, face, bytes, run_text));
        }
    }
    let (sin, cos) = text.rotation.sin_cos();
    // The reference plot letters a border template's text one glyph narrower
    // than the em it sets it in, and letters the sheet's own text at the full
    // em. Height is *not* touched by this: the two sides' glyph boxes are the
    // same 4.74 / 5.08 / 5.93 / 6.60 mm for the template's 3.2 / 3.5 / 4.0 /
    // 4.5 mm styles on both sheets; what differs is only the horizontal
    // advance [`permille_scale`] computes.
    //
    // A 2.5 mm string whose named face draws nothing is the A2 `动火点`
    // class: the reference pitches it at 1.524 mm a glyph and centres on
    // that width. Any other body, or a string the named face still partly
    // draws (`120℃`), stays one em. See [`BORROWED_HAN_ADVANCE`].
    let han_pitch = if !text.furniture && borrowed_whole && (height_mm - 2.5).abs() < 1e-3 {
        // The pitch is a length on the page, so it is taken against the em
        // the run is actually set in, not the style height.
        let set_pt = segments.first().map_or(0.0, |segment| segment.size_pt);
        if set_pt > 0.0 {
            BORROWED_HAN_ADVANCE * height_mm * PT_PER_MM / set_pt
        } else {
            BORROWED_HAN_ADVANCE
        }
    } else {
        1.0
    };
    // The drawing's own lettering takes its style's stated width as it
    // stands, and the template's furniture reads the same field: the symbol
    // bodies' 7.2 pt ascent `M` states `800` and the reference letters it at
    // `Tf 8.16` under a 0.809 horizontal scale, the template's own styles
    // state the same `800` and letter at 0.742..0.797 across their nine
    // heights. See [`permille_scale`].
    let named_face = faces
        .iter()
        .find(|face| face.key == text.typeface)
        .or_else(|| runs.first().map(|(_, face, ..)| *face));
    let scale_heights = named_face.map_or(FaceHeights::ARIAL, |face| face.heights);
    let permille = permille_scale(
        text.typeface,
        scale_heights,
        text.height_basis,
        height_mm,
        text.width_permille,
    );
    let plate = permille * han_pitch;
    // Each run letters with `render.dll`'s own glyph-by-glyph advance — the
    // 72-unit reference font's advances scaled by `a8` and a per-glyph `Tc`,
    // the device font's natural width only for a lone glyph or a rotated
    // pair ([`run_advance`]) — at `plate`, the scale this whole label's
    // `Tm` carries: the same scale narrows every glyph's outline and its
    // advance together, which is why it belongs in `Tm` rather than folded
    // into the width this loop sums.
    //
    // `run_advance` is handed `plate`, not `plate * condense`: `condense`
    // ([`PaperModel::lay_out`] below) is solved from this loop's own
    // `width_mm`, so it cannot exist yet when `Tc` is. The gap this leaves
    // — the emitted `Tc` is exact for `Tm`'s horizontal scale `plate`, a
    // hair off for `plate * condense` — only shows on a run the page
    // cannot hold at its stated width, already the rarest path through
    // this function.
    let mut emitted: Vec<(f64, &ReadyFace<'_>, Vec<u8>, f64)> = Vec::with_capacity(runs.len());
    let mut width_mm = 0.0;
    for (size_pt, face, bytes, run_text) in runs {
        // `face.key`/`face.heights`, not `text.typeface`/`scale_heights`:
        // the 72-unit reference tables are calibrated per *embedded* font
        // identity, and `size_pt` was sized off this same face's own
        // heights once a named face with nothing to draw the run falls
        // back to the first face that can ([`text_segments`],
        // [`encode_runs`] — the two resolve the same fallback face, so the
        // heights agree either way).
        let advance = run_advance(
            face.key,
            &face.metrics,
            &bytes,
            &run_text,
            face.heights,
            text.height_basis,
            height_mm,
            size_pt,
            text.width_permille,
            plate,
            han_pitch == 1.0,
            sin.abs() < 1e-9,
        );
        width_mm += advance.width_mm;
        // `Tc` is a graphics-state parameter: once set it adds to *every*
        // glyph's own displacement, including the run's last one. The
        // reference's `Tc` is only the correction for the `n - 1` gaps
        // *between* glyphs ([`run_advance`]), so a run that needed one is
        // split here — the head under `Tc`, the last glyph alone under
        // `Tc = 0` — rather than let the PDF apply a correction meant for
        // `n - 1` transitions to `n` of them.
        if advance.tc_pt != 0.0
            && let Some(split) = last_glyph_byte_offset(&face.metrics, &bytes)
            && split > 0
            && split < bytes.len()
        {
            let tail = bytes[split..].to_vec();
            let mut head = bytes;
            head.truncate(split);
            emitted.push((size_pt, face, head, advance.tc_pt));
            emitted.push((size_pt, face, tail, 0.0));
        } else {
            emitted.push((size_pt, face, bytes, advance.tc_pt));
        }
    }
    let laid_out_mm = width_mm * plate;
    // The anchor hangs a box from the insertion point as wide as the stored
    // run, its trailing pad included, and the paragraph sets the trimmed line
    // inside it. Both widths are the reference font's
    // ([`reference_extent_mm`]), not the widths the glyphs are laid out at. A
    // paragraph that sits where the anchor does leaves the line where the box
    // is; any other moves it by the pad times how far the paragraph's
    // alignment sits from the anchor's, and the pad itself is not drawn. The
    // two L M line numbers `25-CI-…-B8- ` (left anchor, right paragraph)
    // start one Arial Narrow space right of the insertion, and the CCHB A2
    // company name (centre anchor, left paragraph) starts half a space left
    // of where its trimmed glyphs would centre. The pad is the record's own
    // characters in the face its style names: `L=-18℃ ` letters its `℃`
    // in a borrowed face, and that face's space is twice the Arial Narrow
    // one the plot measures.
    let extent_mm = |value: &str| {
        named_face.map(|face| {
            reference_extent_mm(
                face,
                value,
                text.height_basis,
                height_mm,
                text.width_permille,
            )
        })
    };
    let line_mm = extent_mm(inked).unwrap_or(laid_out_mm);
    let pad_mm = match text.paragraph {
        Some(paragraph) if paragraph != text.anchor => {
            let pad_width_mm = extent_mm(text.value).map_or(0.0, |box_mm| box_mm - line_mm);
            pad_width_mm * (anchor_fraction(paragraph) - anchor_fraction(text.anchor))
        }
        _ => 0.0,
    };
    let origin_mm = [
        text.origin_mm[0] + cos * pad_mm,
        text.origin_mm[1] + sin * pad_mm,
    ];
    // The insertion line is a line of the face the label names, so its depth
    // is read off that face; a label whose named face draws none of it takes
    // the face that does — the same face [`permille_scale`] already sized
    // the run's width from.
    let scale_key = named_face.map_or("Arial", |face| face.key);
    let below_mm = height_mm * scale_heights.drop(scale_key, text.line, text.height_basis);
    // A run the page still cannot hold loses its tail to the page box, so it
    // is condensed along its own direction until it fits: the template states
    // the label and the page states its edge, and keeping every glyph on the
    // sheet beats inventing a smaller size for the letters. Furniture reaches
    // this rule only if it overflows even at its own stated character width
    // ([`permille_scale`]) — the I drawing's 21-glyph company name did, back
    // when it was drawn at the full em and lost its last two characters to
    // the sheet edge.
    let mut condense = 1.0;
    if laid_out_mm > 0.0 && laid_out_mm.is_finite() {
        let start = baseline_origin(origin_mm, text.rotation, line_mm, below_mm, text.anchor);
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
        line_mm * condense,
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
    // `Tc` is a per-run correction ([`run_advance`]), so every run sets its
    // own — including back to zero for a lone glyph that follows one that
    // needed it.
    for (size, face, bytes, tc_pt) in &emitted {
        ops.push(Op::new("Tc", vec![Object::Real(*tc_pt)]));
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

/// How far along a run an alignment sits: `0` at its left end, `0.5` at its
/// middle, `1` at its right end.
fn anchor_fraction(anchor: TextAnchor) -> f64 {
    match anchor {
        TextAnchor::Left => 0.0,
        TextAnchor::Center => 0.5,
        TextAnchor::Right => 1.0,
    }
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
    let back = if width_mm.is_finite() && width_mm > 0.0 {
        width_mm * anchor_fraction(anchor)
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
            if let Some(fit) = primitive
                .stroke
                .as_ref()
                .and_then(|pen| fitted_dash(pen, &primitive.shape))
            {
                for run in stroke_runs(&shape_chain(&primitive.shape), page, false) {
                    for pair in run.windows(2) {
                        let mut walk = DashWalk::new(&fit.pattern_mm(hypot(pair[0], pair[1])), 0.0);
                        out.extend(walk.split(pair[0], pair[1]));
                    }
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

/// `Tf` for one run: the size the reference plotter letters a style height at.
///
/// The plotter sizes a font the way GDI does at the plot's 150 dpi. The style
/// height is a height of the face — its cap height for a cap-height style,
/// its Windows ascent for an ascent style ([`TextHeightBasis`]) — so the em
/// it implies is that height over the face's own ratio. The font is then
/// requested by its **cell** height (`usWinAscent + usWinDescent`), and that
/// is a whole number of 1/150 in pixels; the `Tf` written is the em of that
/// rounded cell, rounded to the 1/2400 in the plot's coordinates use.
///
/// That one rule reproduces every `Tf` the eleven reference plots carry for
/// SimSun, FangSong and Arial — 21 distinct sizes, cap and ascent styles,
/// drawing and furniture lettering alike (Arial 9 pt → 12.45, 8 pt → 11.16,
/// 14.4 pt → 20.19; SimSun 8.50 pt → 12.48, 10.02 pt → 14.88; Arial ascent
/// 7.2 pt → 8.16, Arial Narrow ascent 14.4 pt → 15.69). `ARIALN.TTF` is an
/// `OS/2` version 1 face with no `sCapHeight`; see [`UNSTATED_CAP_EM`].
///
/// See `docs/analysis/2026-11-07-the-plot-size-is-a-whole-pixel-cell.md`.
fn plotted_size_pt(heights: FaceHeights, basis: TextHeightBasis, height_mm: f64) -> f64 {
    let style_pt = height_mm * PT_PER_MM;
    let cell = heights.ascent + heights.descent;
    let Some(cell_px) = whole_pixel_cell(heights, basis, height_mm) else {
        return style_pt;
    };
    let em_pt = cell_px * 72.0 / PLOT_DPI / cell;
    (em_pt * PLOT_UNITS_PER_INCH / 72.0).round() * 72.0 / PLOT_UNITS_PER_INCH
}

/// The style height's cell, rounded to a whole 150 dpi pixel — the plotter's
/// own `lfHeight` request to GDI, `render.dll`'s `round(72 × a8)` worked
/// back from `basis` rather than [`reference_scale`]'s GDI-measured `a8`
/// (the two agree for every face this corpus carries; see
/// [`REFERENCE_CAP_PX`]'s doc comment). `None` where the style height, the
/// named height basis or the face's own cell cannot size a font at all.
fn whole_pixel_cell(heights: FaceHeights, basis: TextHeightBasis, height_mm: f64) -> Option<f64> {
    let style_pt = height_mm * PT_PER_MM;
    let measures = match basis {
        TextHeightBasis::Cap => heights.stated_cap.unwrap_or(UNSTATED_CAP_EM),
        TextHeightBasis::Ascent => heights.ascent,
    };
    let cell = heights.ascent + heights.descent;
    if !(style_pt.is_finite() && style_pt > 0.0 && measures > 0.0 && cell > 0.0) {
        return None;
    }
    let cell_px = (style_pt / measures * cell * PLOT_DPI / 72.0).round();
    (cell_px >= 1.0).then_some(cell_px)
}

/// The resolution the reference plotter sizes fonts at, dots per inch.
const PLOT_DPI: f64 = 150.0;

/// The plot's coordinate grid: every `Tf`, `Tm` and path coordinate the
/// reference plots carry is a multiple of 0.03 pt, 1/2400 in.
const PLOT_UNITS_PER_INCH: f64 = 2400.0;

/// GDI's own measured cap and ascent heights, in pixels, for a face asked
/// for at `lfHeight = 72`.
///
/// `render.dll`'s text path (`sub_1007EF20` in `RADInstallA~\render.dll`)
/// builds a "72-unit reference font" to measure the string it is about to
/// letter, and derives every other figure — the device font's own
/// `lfHeight`/`lfWidth`, and each glyph's `lpDx` advance — from the ratio of
/// the style height to *this table's* reading, not from the face's raw
/// `unitsPerEm` tables. For a hinted TrueType face (Arial, Arial Narrow) the
/// two differ by GDI's own pixel-grid-fitting at 72 px; for the CJK faces
/// here (256 units per em, no hinting at this scale) they agree to the
/// first decimal. Measured on this machine's GDI, matching
/// `docs/findings/2026-11-07-text-geometry-writer.md` §2.
const REFERENCE_CAP_PX: [(&str, f64, f64); 4] = [
    // (face key, cap72 — code 0x0D, asc72 — code 0x0C)
    ("SimSun", 49.2, 62.0),
    ("FangSong", 48.2, 62.0),
    ("Arial", 46.0, 58.0),
    ("Arial Narrow", 44.0, 58.0),
];

/// GDI's measured advance, in pixels, of `0x20..=0x7E` at `lfHeight = 72`,
/// `lfWidth = 0`, for a hinted Latin TrueType face.
///
/// Arial and Arial Narrow are the two faces this corpus letters in Latin
/// text where GDI's 72-pixel hinting moves an advance off the face's raw
/// `hmtx` table (27 and 72 of 95 printable ASCII glyphs respectively); every
/// other face here computes its 72-pixel advance straight from `hmtx`
/// ([`reference_advance_px`]), which reproduces GDI exactly because a 256
/// unit-per-em bitmap-style CJK face has no hinting to apply at that size.
/// Index `code - 0x20`. See
/// `docs/findings/2026-11-07-text-geometry-writer.md` appendix.
#[rustfmt::skip]
const ARIAL_72PX: [u16; 95] = [
    18, 20, 23, 36, 36, 57, 43, 12, 21, 21, 25, 37, 18, 21, 18, 18,
    36, 36, 36, 36, 36, 36, 36, 36, 36, 36, 18, 18, 37, 37, 37, 36,
    65, 43, 43, 46, 46, 43, 39, 50, 46, 18, 32, 43, 36, 53, 46, 50,
    43, 50, 46, 43, 40, 46, 43, 64, 41, 42, 39, 18, 18, 18, 27, 36,
    21, 36, 36, 32, 36, 36, 18, 36, 36, 14, 15, 33, 14, 55, 36, 36,
    36, 36, 21, 32, 18, 36, 33, 47, 31, 31, 31, 21, 17, 21, 37,
];

/// `ARIALN.TTF`'s own reading of the same 72-pixel table as
/// [`ARIAL_72PX`].
#[rustfmt::skip]
const ARIAL_NARROW_72PX: [u16; 95] = [
    14, 14, 18, 29, 28, 46, 33, 10, 17, 17, 19, 29, 14, 17, 14, 14,
    28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 14, 14, 29, 29, 29, 28,
    51, 33, 33, 36, 36, 33, 31, 39, 36, 14, 25, 33, 28, 42, 36, 39,
    33, 39, 36, 33, 31, 36, 33, 47, 33, 33, 31, 14, 14, 14, 23, 28,
    17, 28, 28, 25, 28, 28, 14, 28, 28, 11, 11, 25, 11, 42, 28, 28,
    28, 28, 17, 25, 14, 28, 25, 36, 25, 25, 25, 17, 13, 17, 29,
];

/// The GDI reference pixel the plotter measures `basis` against for `key`,
/// where [`REFERENCE_CAP_PX`] carries one.
fn reference_cap_px(key: &str, basis: TextHeightBasis) -> Option<f64> {
    REFERENCE_CAP_PX
        .iter()
        .find(|(face, ..)| *face == key)
        .map(|(_, cap72, asc72)| match basis {
            TextHeightBasis::Cap => *cap72,
            TextHeightBasis::Ascent => *asc72,
        })
}

/// `render.dll`'s `a8`: the continuous scale from the 72-unit reference font
/// to the device font a style height asks for.
///
/// `a8 = h_px / M`, `h_px` the style height in 150 dpi pixels and `M` the
/// face's own [`REFERENCE_CAP_PX`] reading of `basis`. A face with no row
/// there (every face but the four Latin/CJK families the corpus letters in)
/// falls back to the same cell-to-em ratio [`plotted_size_pt`] rounds from,
/// which is what a GDI reading and a raw `unitsPerEm` reading agree on for
/// an unhinted face.
fn reference_scale(key: &str, heights: FaceHeights, basis: TextHeightBasis, height_mm: f64) -> f64 {
    let h_px = height_mm * PT_PER_MM * PLOT_DPI / 72.0;
    let measures = match basis {
        TextHeightBasis::Cap => heights.stated_cap.unwrap_or(UNSTATED_CAP_EM),
        TextHeightBasis::Ascent => heights.ascent,
    };
    let cell = heights.ascent + heights.descent;
    let m = reference_cap_px(key, basis).unwrap_or_else(|| 72.0 * measures / cell.max(1e-9));
    if m > 0.0 { h_px / m } else { 0.0 }
}

/// A glyph's advance in the 72-unit reference font, in pixels.
///
/// Arial and Arial Narrow read [`ARIAL_72PX`] / [`ARIAL_NARROW_72PX`] for
/// `0x20..=0x7E` — GDI's own hinted measurement at `lfWidth = 0`, so a
/// stated character width never touches it. Every other code, and every
/// other face, computes the advance from its own `hmtx` table: `round(W ×
/// 72/cell)`, `W` the glyph's advance as a fraction of the em
/// ([`pdfcraft_fonts::pdf::Metrics::width`] already reads it that way) and
/// `cell` the face's ascent-plus-descent fraction — the conversion that
/// puts a glyph's natural advance on the same 72-pixel cell the reference
/// font is built at — then scaled by `wf_scale`, `lw_ref/(72 × avg/cell)`
/// when the style states a character width other than `1000` (`1.0`
/// otherwise): the reference font itself is narrower or wider than its
/// natural proportions whenever `JStyleTextChar +52` is. See
/// [`REFERENCE_CAP_PX`].
fn reference_advance_px(key: &str, code: u32, width_em: f64, cell: f64, wf_scale: f64) -> f64 {
    let table = match key {
        "Arial" => Some(&ARIAL_72PX),
        "Arial Narrow" => Some(&ARIAL_NARROW_72PX),
        _ => None,
    };
    if let Some(table) = table
        && let Ok(index) = usize::try_from(code.wrapping_sub(0x20))
        && let Some(px) = table.get(index)
    {
        return f64::from(*px);
    }
    if cell <= 0.0 {
        return 0.0;
    }
    (width_em * 72.0 / cell).round() * wf_scale
}

/// `text`'s continuous advance in `face`'s 72-unit reference font, in
/// millimetres on the page: what `GetTextExtentExPointA` measures on that
/// font, times `a8` ([`reference_scale`]).
///
/// This is the width `sub_1000bcd0` hangs a label's box on before it spaces
/// a single glyph, so every character is measured in the face the label
/// names, whichever face letters it. Arial and Arial Narrow read
/// [`ARIAL_72PX`] / [`ARIAL_NARROW_72PX`] and measure a character they do
/// not carry as `?`, what the ANSI conversion in front of
/// `GetTextExtentExPointA` turns it into: the A2 `动火点`, named Arial
/// Narrow, centres on three `?` (−2.386 mm against the reference's
/// −2.336 / −2.391). Every other face reads its own advance on the
/// 72-pixel cell. The style's character width narrows the reference font
/// itself ([`wf_reference_scale`]), the hinted tables included: the symbol
/// bodies' `M`, Arial at `800`, centres on 1.795 mm, not on the table's
/// 2.321.
fn reference_extent_mm(
    face: &ReadyFace<'_>,
    text: &str,
    basis: TextHeightBasis,
    height_mm: f64,
    width_permille: Option<u16>,
) -> f64 {
    let a8 = reference_scale(face.key, face.heights, basis, height_mm);
    let wf_scale = wf_reference_scale(face.heights, width_permille);
    let cell = face.heights.ascent + face.heights.descent;
    let table = match face.key {
        "Arial" => Some(&ARIAL_72PX),
        "Arial Narrow" => Some(&ARIAL_NARROW_72PX),
        _ => None,
    };
    let px: f64 = text
        .chars()
        .map(|glyph| {
            if let Some(table) = table {
                let ansi = if (' '..='~').contains(&glyph) {
                    glyph
                } else {
                    '?'
                };
                let index = usize::try_from(u32::from(ansi) - 0x20).unwrap_or(0);
                return table.get(index).map_or(0.0, |px| f64::from(*px)) * wf_scale;
            }
            let width_em = face
                .metrics
                .encode(glyph.encode_utf8(&mut [0; 4]))
                .and_then(|bytes| {
                    face.metrics
                        .codes(&bytes)
                        .first()
                        .map(|(code, _)| face.metrics.width(*code))
                })
                .unwrap_or(0.0);
            if cell > 0.0 {
                (width_em * 72.0 / cell).round() * wf_scale
            } else {
                0.0
            }
        })
        .sum();
    px * a8 * 25.4 / PLOT_DPI
}

/// One run's letter-spacing and glyph advances, `render.dll`'s way.
///
/// Every glyph but the last advances by a whole 150 dpi pixel: its 72-unit
/// reference-font advance ([`reference_advance_px`], narrowed by
/// [`wf_reference_scale`] when the style states a character width) scaled by
/// `a8` ([`reference_scale`]), rounded with the running error carried into
/// the next glyph so the cumulative pixel position never drifts by more than
/// one; `Tc` is the per-glyph correction (excluding the last glyph, which
/// carries none) that makes a PDF reader's own glyph-width arithmetic land on
/// the same `lpDx` the vendor wrote.
///
/// A two-glyph run is spaced the same way. `sub_1007EF20` only leaves `lpDx`
/// out for a run under three glyphs whose justification bits are clear, and
/// then GDI's own device-font advance is a whole pixel too — Arial Narrow's
/// at a 24 px cell is `round(72 px advance × a8)` glyph for glyph — so the
/// reference spaces all 1101 of its horizontal two-glyph runs by this rule
/// (`docs/analysis/2026-11-07-every-label-miss-is-a-reference-font-rule-or-the-pixel-grid.md`).
/// A single glyph has no gap to space, and a two-glyph run that is not
/// `upright` keeps its natural advance: GDI does not hint a rotated device
/// font, so there the two paths part. Both `width_mm` and `tc_pt` are figured at `sx = 1` —
/// the one `Tm` horizontal scale the caller's whole label carries,
/// `permille` and the borrowed-Han pitch alike, is what turns either into
/// what actually lands on the page, the same way it turns every glyph's
/// own outline narrower together with its advance (the reason `render.dll`
/// drew a condensed run with a condensed device font rather than a kerning
/// table). `sx` is only this function's divisor for `tc_pt`, so a `Tm` of
/// `1` reproduces it exactly; the caller never multiplies `tc_pt` again.
///
/// `reference_font_applies` is `false` for a label stepping to the
/// borrowed-Han pitch ([`BORROWED_HAN_ADVANCE`]): `render.dll` never built
/// that pitch, it is this crate's own stand-in for a face that cannot draw
/// the string at all, so there is no 72-unit reference font for it either
/// — such a run keeps its own even spacing rather than feeding an invented
/// ratio through a formula calibrated on real `lfWidth` values. See
/// `docs/findings/2026-11-07-text-geometry-writer.md` §2.
struct RunAdvance {
    /// This run's letter-spacing, in points, at `Tm`'s horizontal scale `1`
    /// (`0` for a single glyph); the caller's own `Tm` carries `sx` from
    /// here on.
    tc_pt: f64,
    /// This run's total advance to the end of its last glyph, in
    /// millimetres, at `Tm`'s horizontal scale `1`; the caller's own `Tm`
    /// carries `sx`. The leading `n - 1` glyphs advance by `tc_pt`-corrected
    /// `lpDx`; the last glyph (drawn under its own `Tc = 0`, see
    /// [`last_glyph_byte_offset`]) advances by its own embedded-font natural
    /// width — not the reference font's `lpDx` reading of it, which this
    /// field never folds in.
    width_mm: f64,
}

#[allow(clippy::too_many_arguments)]
fn run_advance(
    key: &str,
    metrics: &Metrics,
    bytes: &[u8],
    text: &str,
    heights: FaceHeights,
    basis: TextHeightBasis,
    height_mm: f64,
    size_pt: f64,
    width_permille: Option<u16>,
    sx: f64,
    reference_font_applies: bool,
    upright: bool,
) -> RunAdvance {
    let codes = metrics.codes(bytes);
    let n = codes.len();
    let spaced = n >= 3 || (n == 2 && upright);
    if !spaced || !reference_font_applies || !(sx.is_finite() && sx > 0.0) {
        return RunAdvance {
            tc_pt: 0.0,
            width_mm: run_width_mm(metrics, size_pt, bytes),
        };
    }
    let cell = heights.ascent + heights.descent;
    let Some(cell_px) = whole_pixel_cell(heights, basis, height_mm) else {
        return RunAdvance {
            tc_pt: 0.0,
            width_mm: run_width_mm(metrics, size_pt, bytes),
        };
    };
    let a8 = reference_scale(key, heights, basis, height_mm);
    let wf_scale = wf_reference_scale(heights, width_permille);
    let em_px = cell_px / cell;
    // `reference_advance_px` keys the hinted Arial/Arial Narrow tables on
    // the glyph's own *Unicode* codepoint — the table is GDI's reading of
    // `0x20..=0x7E` — but `codes` are this run's *embedded* encoding
    // (`Metrics::codes`), a CID `pdfcraft_fonts::embedding` assigns in the
    // order its own font subset first meets each character, unrelated to
    // the character itself. `text`'s own `char`s, one per code in the
    // same order ([`encode_runs`] builds both from the same
    // one-`char`-at-a-time walk), are the codepoint the table actually
    // wants.
    let mut chars = text.chars();
    let mut float_acc = 0.0;
    let mut int_acc: i64 = 0;
    let mut lp_dx = Vec::with_capacity(n);
    let mut widths_em = Vec::with_capacity(n);
    for (code, _) in &codes {
        let width_em = metrics.width(*code);
        widths_em.push(width_em);
        let unicode = chars.next().map_or(*code, u32::from);
        // `lpDx_i = round(Δref_i × a8)`, each glyph's own step rounded on
        // its own — not the running total rounded once per glyph, which
        // drifts a full em's worth of identical glyphs a pixel short (a
        // three-glyph run of the same full-em CJK code once settled a
        // pixel under the reference's own reading this way: every other
        // glyph in the corpus happens to vary enough from its neighbours
        // that the two roundings agree). `float_acc`/`int_acc` track the
        // running floating-point and already-rounded totals so a step can
        // still be nudged by one when they drift more than a pixel apart —
        // the error diffusion [`RunAdvance`]'s own doc comment describes.
        let step_px = reference_advance_px(key, unicode, width_em, cell, wf_scale) * a8;
        float_acc += step_px;
        #[allow(clippy::cast_possible_truncation)]
        let mut step_i = step_px.round() as i64;
        #[allow(clippy::cast_precision_loss)]
        let drift = float_acc - (int_acc + step_i) as f64;
        if drift > 1.0 {
            step_i += 1;
        } else if drift < -1.0 {
            step_i -= 1;
        }
        int_acc += step_i;
        lp_dx.push(step_i);
    }
    let leading: i64 = lp_dx[..n - 1].iter().sum();
    #[allow(clippy::cast_precision_loss)]
    let leading_px = leading as f64;
    let embedded_px: f64 = widths_em[..n - 1].iter().sum::<f64>() * em_px * sx;
    #[allow(clippy::cast_precision_loss)]
    let tc_pt = (leading_px - embedded_px) / (n - 1) as f64 * (72.0 / PLOT_DPI) / sx;
    // The page renders the first `n - 1` glyphs at `leading_px` (that is
    // what `tc_pt` was solved to make them do) and the last glyph at its
    // own embedded-font natural width, under `Tc = 0`
    // ([`last_glyph_byte_offset`]'s split) — not at the reference font's
    // own `lp_dx` reading of it, which `widths_em[n - 1]` never enters.
    // Summing all `n` entries of `lp_dx` instead double-counts the
    // reference font's own rounding on the glyph that is never drawn
    // under its correction.
    let width_mm = leading_px * 25.4 / PLOT_DPI / sx + widths_em[n - 1] * size_pt / PT_PER_MM;
    RunAdvance { tc_pt, width_mm }
}

/// The byte offset where `bytes`' last glyph starts, or `None` for a run of
/// one glyph (there is no "head" to split off) or an empty run.
///
/// [`run_advance`]'s `Tc` corrects only the gaps *between* glyphs — `n - 1`
/// of them for `n` glyphs — but a PDF's `Tc` graphics-state parameter adds
/// to every glyph's own displacement once set, the trailing one included.
/// Splitting the run here, so the trailing glyph can go out under its own
/// `Tj` with `Tc` reset to `0`, is what keeps that one extra correction off
/// the page.
fn last_glyph_byte_offset(metrics: &Metrics, bytes: &[u8]) -> Option<usize> {
    let codes = metrics.codes(bytes);
    if codes.len() < 2 {
        return None;
    }
    let last_len = codes.last()?.1;
    bytes.len().checked_sub(last_len)
}

/// The cap height, in em, the plotter assumes for a face whose `OS/2` table
/// states none.
///
/// Only `ARIALN.TTF` (`OS/2` version 1) is such a face here. Its `H` stands
/// 1466/2048 = 0.716 em tall, and sizing from that puts every cap-height
/// Arial Narrow style one pixel short of the reference: 7 pt → 9.75 against
/// 10.17, 10 pt → 13.98 against 14.40. This is not a free-standing fudge: it
/// is `render.dll`'s own cap reading for the face, recovered from its GDI
/// 72-pixel cap height — `REFERENCE_CAP_PX`'s Arial Narrow row, 44.0 — run
/// back through the cell-to-em conversion [`plotted_size_pt`] otherwise uses
/// (`44.0 / (72 × 2048/2319)`). The reference's own cells (24 px and 34 px)
/// bound the ratio to 0.685..0.702 em; 0.6923 is inside it.
const UNSTATED_CAP_EM: f64 = 44.0 / (72.0 * 2048.0 / 2319.0);
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
        Dict, Document, PEN_MID_MM, PEN_THICK_MM, PEN_THIN_MM, STROKE_MM, ShownText, embed,
        embedding, pen_width_mm, plotted_size_pt, text_segments, write_plot,
    };
    use crate::model::{
        PaperModel, PaperPrimitive, PaperShape, PaperTextSpan, TextAnchor, TextLine,
    };
    use crate::read::read_plot;
    use pdfcraft_fonts::pdf::Metrics;

    use super::PlotFont;

    /// `JStyleTextChar +52`'s `800` narrows a SimSun furniture label by a
    /// ratio that changes at every `Tf` tier the style height rounds to —
    /// four of this corpus's own tiers, each `lfW / (lfH × avg/cell)` with
    /// `lfW = round(lw_ref × a8)` and `a8` read off SimSun's GDI-measured
    /// 49.2 px cap ([`REFERENCE_CAP_PX`]), not the old per-mille-of-cell
    /// rule that was a whole device pixel off `lfW` at each of these four.
    #[test]
    fn furniture_w800_matches_simsun_four_height_tiers() {
        let simsun = super::FaceHeights {
            ascent: 220.0 / 256.0,
            descent: 36.0 / 256.0,
            cap: 175.0 / 256.0,
            stated_cap: Some(175.0 / 256.0),
            avg_em: 0.5,
        };
        let cases = [
            (4.0, 16.8, 26.0 / 35.0),
            (4.243_916_666_666_666, 17.76, 28.0 / 37.0),
            (4.5, 18.72, 10.0 / 13.0),
            (7.778, 32.16, 52.0 / 67.0),
        ];
        for (height_mm, expected_tf, expected_sx) in cases {
            let tf = super::plotted_size_pt(simsun, crate::model::TextHeightBasis::Cap, height_mm);
            assert!(
                (tf - expected_tf).abs() < 1e-9,
                "h={height_mm}: Tf {tf} != {expected_tf}"
            );
            let sx = super::permille_scale(
                "SimSun",
                simsun,
                crate::model::TextHeightBasis::Cap,
                height_mm,
                Some(800),
            );
            assert!(
                (sx - expected_sx).abs() < 1e-9,
                "h={height_mm}: sx {sx} != {expected_sx}"
            );
        }
    }

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

    /// Embeds `bytes` the same way [`write_plot`] does, and hands back the
    /// font's own [`Metrics`] and the glyph bytes `value` encodes to — the
    /// same pair [`super::run_advance`] takes, built outside any
    /// [`ShownText`] so the error-diffusion rule can be pinned on its own.
    fn metrics_and_bytes(bytes: &'static [u8], value: &str) -> (Metrics, Vec<u8>) {
        let mut doc = Document::new_empty();
        let mut font_resources = Dict::new();
        let (family, _) = embedding::names(bytes).expect("font name");
        let resource = embed(&mut doc, &mut font_resources, bytes, &family).expect("embed");
        let font_ref = font_resources
            .get(resource.as_bytes())
            .cloned()
            .expect("resource");
        let font_dict = doc.dict(&font_ref).expect("font dict");
        let metrics = Metrics::from_dict(&doc, &font_dict);
        let encoded = metrics.encode(value).expect("value encodes");
        (metrics, encoded)
    }

    /// An upright two-glyph run advances its first glyph by a whole 150 dpi
    /// pixel, the same `lpDx` a longer run uses ([`super::run_advance`]):
    /// the reference spaces `HS`, `LT` and every other horizontal pair that
    /// way, `Tc −0.26` on Arial Narrow's `HS`, where lettering the pair at
    /// its glyphs' own fractional advance left it 0.09 mm wide.
    #[test]
    fn an_upright_two_glyph_run_is_spaced_like_a_longer_one() {
        let (metrics, bytes) = metrics_and_bytes(pdfcraft_fonts::INTER_REGULAR, "AB");
        let inter = super::FaceHeights::of(pdfcraft_fonts::INTER_REGULAR);
        let size_pt = super::plotted_size_pt(inter, crate::model::TextHeightBasis::Cap, 4.0);
        let advance = super::run_advance(
            "",
            &metrics,
            &bytes,
            "AB",
            inter,
            crate::model::TextHeightBasis::Cap,
            4.0,
            size_pt,
            None,
            1.0,
            true,
            true,
        );
        let codes = metrics.codes(&bytes);
        let last_code = codes.last().expect("two codes").0;
        let last_natural_mm = metrics.width(last_code) * size_pt / super::PT_PER_MM;
        let leading_px = (advance.width_mm - last_natural_mm) * 150.0 / 25.4;
        assert!(
            (leading_px - leading_px.round()).abs() < 1e-6,
            "the first glyph advances a whole 150 dpi pixel: {leading_px}"
        );
        assert_ne!(
            advance.tc_pt, 0.0,
            "a fractional natural advance is corrected onto that pixel"
        );
    }

    /// A rotated pair is the one short run that keeps the device font's own
    /// natural advance: GDI does not hint a rotated font, and the reference
    /// letters a rotated `NC` a whole pixel off its `lpDx`.
    #[test]
    fn a_rotated_two_glyph_run_keeps_its_natural_advance() {
        let (metrics, bytes) = metrics_and_bytes(pdfcraft_fonts::INTER_REGULAR, "AB");
        let inter = super::FaceHeights::of(pdfcraft_fonts::INTER_REGULAR);
        let size_pt = super::plotted_size_pt(inter, crate::model::TextHeightBasis::Cap, 4.0);
        let advance = super::run_advance(
            "",
            &metrics,
            &bytes,
            "AB",
            inter,
            crate::model::TextHeightBasis::Cap,
            4.0,
            size_pt,
            None,
            1.0,
            true,
            false,
        );
        assert_eq!(
            advance.tc_pt, 0.0,
            "a rotated pair states no letter-spacing"
        );
        assert_eq!(
            advance.width_mm,
            super::run_width_mm(&metrics, size_pt, &bytes),
            "a rotated pair keeps every glyph's own natural advance"
        );
    }

    /// `n >= 3` instead rounds each glyph's *cumulative* reference-font
    /// advance to a whole 150 dpi pixel and carries the leftover into the
    /// next glyph ([`super::run_advance`]'s `float_acc`/`int_acc`), the
    /// same error-diffusion `render.dll` itself performs building an
    /// `lpDx` array — so the run's total advance is always a whole number
    /// of device pixels even though no individual glyph's own advance is,
    /// and `Tc` differs from `0` once a stated character width or the
    /// reference font's own rounding has moved a gap off the glyphs'
    /// natural spacing.
    #[test]
    fn a_run_of_three_or_more_glyphs_diffuses_its_rounding_error() {
        let (metrics, bytes) = metrics_and_bytes(pdfcraft_fonts::INTER_REGULAR, "ABA");
        let inter = super::FaceHeights::of(pdfcraft_fonts::INTER_REGULAR);
        let size_pt = super::plotted_size_pt(inter, crate::model::TextHeightBasis::Cap, 4.0);
        let advance = super::run_advance(
            "",
            &metrics,
            &bytes,
            "ABA",
            inter,
            crate::model::TextHeightBasis::Cap,
            4.0,
            size_pt,
            None,
            1.0,
            true,
            true,
        );
        assert_ne!(
            advance.tc_pt, 0.0,
            "three glyphs letter-space once the reference font's own rounding moves a gap"
        );
        let natural_mm = super::run_width_mm(&metrics, size_pt, &bytes);
        assert_ne!(
            advance.width_mm, natural_mm,
            "the lpDx-rounded width is not the glyphs' own natural advance"
        );
        // The page draws the leading `n - 1` glyphs under `Tc` and the
        // trailing one alone under `Tc = 0` ([`super::last_glyph_byte_offset`]),
        // so only the leading portion sits on the diffused `lpDx` grid —
        // the trailing glyph keeps its own embedded-font natural width,
        // which is not generally a whole 150 dpi pixel.
        let codes = metrics.codes(&bytes);
        let last_code = codes.last().expect("three codes").0;
        let last_natural_mm = metrics.width(last_code) * size_pt / super::PT_PER_MM;
        let leading_px = (advance.width_mm - last_natural_mm) * 150.0 / 25.4;
        assert!(
            (leading_px - leading_px.round()).abs() < 1e-6,
            "the diffused leading portion lands on a whole 150 dpi pixel: {leading_px}"
        );
    }

    /// A label's box is its stored value measured in the 72-unit reference
    /// font of the face it names ([`super::reference_extent_mm`]): Arial
    /// Narrow's hinted table for `120`, and `?` for the `℃` it does not
    /// carry; the style's character width narrows the hinted table as it
    /// narrows any face; every other face measures its own advance on the
    /// 72-pixel cell.
    #[test]
    fn a_label_box_is_its_value_in_the_named_reference_font() {
        let inter = super::FaceHeights::of(pdfcraft_fonts::INTER_REGULAR);
        let face = |key: &'static str| super::ReadyFace {
            key,
            resource: String::new(),
            metrics: metrics_and_bytes(pdfcraft_fonts::INTER_REGULAR, "AB").0,
            heights: inter,
        };
        let cap = crate::model::TextHeightBasis::Cap;
        let ascent = crate::model::TextHeightBasis::Ascent;
        let px_mm = 25.4 / super::PLOT_DPI;

        let a8 = super::reference_scale("Arial Narrow", inter, cap, 2.5);
        let narrow = face("Arial Narrow");
        let measured = super::reference_extent_mm(&narrow, "120℃", cap, 2.5, None);
        let expected = 4.0 * 28.0 * a8 * px_mm;
        assert!(
            (measured - expected).abs() < 1e-9,
            "120℃ measures {measured} mm, `120?` is {expected}"
        );

        let wf = super::wf_reference_scale(inter, Some(800));
        assert!(wf < 1.0, "800 narrows the reference font: {wf}");
        let a8 = super::reference_scale("Arial", inter, ascent, 2.54);
        let measured = super::reference_extent_mm(&face("Arial"), "M", ascent, 2.54, Some(800));
        let expected = 53.0 * wf * a8 * px_mm;
        assert!(
            (measured - expected).abs() < 1e-9,
            "M at 800 measures {measured} mm, the narrowed table {expected}"
        );

        let (metrics, bytes) = metrics_and_bytes(pdfcraft_fonts::INTER_REGULAR, "AB");
        let cell = inter.ascent + inter.descent;
        let own_px: f64 = metrics
            .codes(&bytes)
            .iter()
            .map(|(code, _)| (metrics.width(*code) * 72.0 / cell).round())
            .sum();
        let a8 = super::reference_scale("", inter, cap, 4.0);
        let measured = super::reference_extent_mm(&face(""), "AB", cap, 4.0, None);
        let expected = own_px * a8 * px_mm;
        assert!(
            (measured - expected).abs() < 1e-9,
            "AB measures {measured} mm, its own advances {expected}"
        );
    }

    /// One rule sizes every face: the style height's em, its cell rounded to
    /// whole 150 dpi pixels, written on the plot's 1/2400 in grid. Each row is
    /// a `Tf` read off the eleven reference plots.
    #[test]
    fn measured_style_heights_use_the_reference_tf() {
        use crate::model::TextHeightBasis::{Ascent, Cap};
        let face = |upm: f64, ascent: f64, descent: f64, cap: Option<f64>| super::FaceHeights {
            ascent: ascent / upm,
            descent: descent / upm,
            cap: cap.unwrap_or(1466.0) / upm,
            stated_cap: cap.map(|cap| cap / upm),
            avg_em: 0.0,
        };
        let arial = face(2048.0, 1854.0, 434.0, Some(1467.0));
        let narrow = face(2048.0, 1888.0, 431.0, None);
        let simsun = face(256.0, 220.0, 36.0, Some(175.0));
        let fangsong = face(256.0, 220.0, 36.0, Some(171.0));
        let mm = |pt: f64| pt * 25.4 / 72.0;
        let rows = [
            (narrow, Cap, 7.0, 10.17),
            (narrow, Cap, 10.0, 14.40),
            (narrow, Ascent, 14.4, 15.69),
            (arial, Cap, 9.0, 12.45),
            (arial, Cap, 8.0, 11.16),
            (arial, Cap, 14.4, 20.19),
            (arial, Ascent, 7.2, 8.16),
            (arial, Ascent, 7.76, 8.58),
            (arial, Ascent, 11.58, 12.90),
            (fangsong, Cap, 7.0, 10.56),
            (fangsong, Cap, 7.2, 10.56),
            (fangsong, Cap, 9.0, 13.44),
            (simsun, Cap, 6.98, 10.08),
            (simsun, Cap, 7.09, 10.56),
            (simsun, Cap, 8.50, 12.48),
            (simsun, Cap, 9.07, 13.44),
            (simsun, Cap, 9.92, 14.40),
            (simsun, Cap, 10.02, 14.88),
            (simsun, Cap, 11.34, 16.80),
            (simsun, Cap, 12.03, 17.76),
            (simsun, Cap, 12.76, 18.72),
        ];
        for (heights, basis, style_pt, reference) in rows {
            let got = plotted_size_pt(heights, basis, mm(style_pt));
            assert!(
                (got - reference).abs() < 1e-9,
                "{style_pt} pt {basis:?} {heights:?}: {got} against the reference's {reference}"
            );
        }
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
            dash_fit: None,
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
            dash_fit: None,
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

    /// A fitted dash starts on every vertex and is stretched per piece, and
    /// the reader sees exactly the pieces `stroked_segments` predicts.
    #[test]
    fn a_fitted_dash_restarts_on_every_vertex() {
        let mut line = primitive(
            "decoded",
            PaperShape::Polyline {
                points_mm: vec![[10.0, 20.0], [17.298, 20.0], [17.298, 60.0]],
                closed: false,
            },
        );
        line.stroke = Some(super::PaperStroke {
            rgb: [0, 0, 0],
            width_mm: 0.18,
            dash_mm: vec![1.905, 1.27],
            dash_fit: Some(crate::model::DashFit {
                ratio: f64::from(7.0_f32 / 6.0),
                signed_mm: vec![-1.905, -1.27],
            }),
        });
        let model = PaperModel {
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
        };
        let expected = super::stroked_segments(&model);
        let pdf = write_plot(&model, &[]).expect("write");
        let ink = read_plot(&pdf).expect("read");
        assert_eq!(ink.segments.len(), expected.len());
        // The vertical piece's first dash starts on the vertex, not partway
        // through the horizontal piece's pattern.
        let vertical: Vec<_> = expected
            .iter()
            .filter(|(start, end)| {
                (start[0] - 17.298).abs() < 1e-6 && (end[0] - 17.298).abs() < 1e-6
            })
            .collect();
        assert!((vertical[0].0[1] - 20.0).abs() < 1e-6, "{:?}", vertical[0]);
        // The 7.298 mm piece holds two stretched dashes, the second ending
        // half a gap short of the vertex.
        let horizontal = expected.len() - vertical.len();
        assert_eq!(horizontal, 2);
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
                dash_fit: None,
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
                    paragraph: None,
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
                        paragraph: None,
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

    /// A centred label hangs half its reference-font width left of the
    /// insertion point and a right-aligned one all of it
    /// ([`super::reference_extent_mm`]), not its laid-out width.
    #[test]
    fn centre_and_right_step_back_by_the_reference_width() {
        let (metrics, bytes) = metrics_and_bytes(pdfcraft_fonts::INTER_REGULAR, "ABA");
        let inter = super::FaceHeights::of(pdfcraft_fonts::INTER_REGULAR);
        let basis = crate::model::TextHeightBasis::Cap;
        let size_pt = super::plotted_size_pt(inter, basis, 5.0);
        let laid_out_mm = super::run_advance(
            "", &metrics, &bytes, "ABA", inter, basis, 5.0, size_pt, None, 1.0, true, true,
        )
        .width_mm;
        let face = super::ReadyFace {
            key: "",
            resource: String::new(),
            metrics,
            heights: inter,
        };
        let box_mm = super::reference_extent_mm(&face, "ABA", basis, 5.0, None);
        assert!(
            (box_mm - laid_out_mm).abs() > 0.02,
            "the two widths must differ for this run to tell them apart: {box_mm} vs {laid_out_mm}"
        );
        let left = placed("ABA", [80.0, 20.0], 0.0, TextAnchor::Left, TextLine::Bottom);
        let centre = placed(
            "ABA",
            [80.0, 44.0],
            0.0,
            TextAnchor::Center,
            TextLine::Bottom,
        );
        let right = placed(
            "ABA",
            [80.0, 68.0],
            0.0,
            TextAnchor::Right,
            TextLine::Bottom,
        );
        for (start, fraction) in [(centre, 0.5), (right, 1.0)] {
            let back = left[0] - start[0];
            assert!(
                (back - fraction * box_mm).abs() < 0.01,
                "steps back {back:.4} mm, {fraction} of the reference width is {:.4} mm",
                fraction * box_mm
            );
        }
    }

    /// The vertical half of the anchor: the label letters below the point.
    ///
    /// The reference plots put the baseline 0.00 mm / 1.25 mm / 3.26 mm below
    /// a 2.5 mm body's insertion point, one mode per class, so the step is
    /// measured against the body size and not against the pen. Each line is
    /// the reference font's ([`super::FaceHeights::drop`]): the top one body
    /// down, the middle and the ascender where that font's own heights put
    /// them.
    #[test]
    fn middle_and_top_step_down_across_the_baseline() {
        let inter = super::FaceHeights::of(pdfcraft_fonts::INTER_REGULAR);
        let cap = crate::model::TextHeightBasis::Cap;
        let middle_mm = 5.0 * inter.drop("", TextLine::Middle, cap);
        let ascender_mm = 5.0 * inter.drop("", TextLine::Ascender, cap);
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
        assert!(
            (middle[1] - (20.0 - middle_mm)).abs() < 0.01,
            "middle {} against {}",
            middle[1],
            20.0 - middle_mm
        );
        assert!((top[1] - 15.0).abs() < 0.05, "top {}", top[1]);
        assert!(
            (ascender[1] - (20.0 - ascender_mm)).abs() < 0.01,
            "ascender {} against {}",
            ascender[1],
            20.0 - ascender_mm
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
            (dropped[0] - (upright[0] + middle_mm)).abs() < 0.1,
            "turned {dropped:?}"
        );
        assert!((dropped[1] - upright[1]).abs() < 0.1, "turned {dropped:?}");
    }

    /// The insertion line's depth is a line of the face's 72-unit reference
    /// font, measured in the height the style states.
    ///
    /// Arial's 72-pixel font is 58 pixels of ascent, 14 of descent and 46 of
    /// cap, so a cap-height style hangs its middle 22/46 of a style height
    /// down and its ascender line 58/46 — the 266 Arial `0x01` labels on
    /// the eleven reference plots sit 1.258 style heights down. SimSun and
    /// FangSong are 62 pixels of ascent: their middles are 26/49.2 and
    /// 26/48.2, where the reference letters the 115 SimSun template middles
    /// (0.525) and the 121 FangSong `0x05` labels (0.538). A face the table
    /// does not carry puts its own tables on the same 72-pixel cell.
    #[test]
    fn the_line_depth_is_a_height_of_the_face() {
        use crate::model::TextHeightBasis::{Ascent, Cap};
        let arial = super::FaceHeights::ARIAL;
        let close = |got: f64, want: f64| (got - want).abs() < 1e-9;
        assert!(close(arial.drop("Arial", TextLine::Bottom, Cap), 0.0));
        assert!(close(
            arial.drop("Arial", TextLine::Middle, Cap),
            22.0 / 46.0
        ));
        assert!(close(arial.drop("Arial", TextLine::Top, Cap), 1.0));
        assert!(close(
            arial.drop("Arial", TextLine::Ascender, Cap),
            58.0 / 46.0
        ));
        assert!(close(
            arial.drop("Arial", TextLine::Descender, Cap),
            -14.0 / 46.0
        ));
        assert!(close(
            arial.drop("Arial", TextLine::Middle, Ascent),
            22.0 / 58.0
        ));
        assert!(close(
            arial.drop("Arial", TextLine::Top, Ascent),
            46.0 / 58.0
        ));
        assert!(close(arial.drop("Arial", TextLine::Ascender, Ascent), 1.0));
        assert!(close(
            arial.drop("Arial Narrow", TextLine::Middle, Cap),
            0.5
        ));
        assert!(close(
            arial.drop("Arial Narrow", TextLine::Ascender, Cap),
            58.0 / 44.0
        ));
        assert!(close(
            arial.drop("SimSun", TextLine::Middle, Cap),
            26.0 / 49.2
        ));
        assert!(close(
            arial.drop("FangSong", TextLine::Middle, Cap),
            26.0 / 48.2
        ));

        let unlisted = arial.drop("", TextLine::Middle, Cap);
        let own = (arial.ascent - arial.descent) / 2.0 / arial.cap;
        assert!(close(unlisted, own), "{unlisted} against {own}");
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

    /// A trailing pad is dropped before the anchor measures the run; a
    /// leading one is lettered and measured with it.
    ///
    /// The drawing files pad their fields with spaces — `H=11.8MPa ` with a
    /// trailing one, ` 1001102A` with a leading one. No reference run ends in
    /// a space, while every leading-pad tag's run starts with one, takes its
    /// letter spacing over all nine glyphs and is centred on that width: a
    /// centred run steps back half the pad further, a right-aligned one all
    /// of it — the pad as the reference font measures it
    /// ([`super::reference_extent_mm`]).
    #[test]
    fn a_leading_pad_is_lettered_and_a_trailing_one_is_not() {
        let inter = super::FaceHeights::of(pdfcraft_fonts::INTER_REGULAR);
        let reference_width = |value: &str| {
            let face = super::ReadyFace {
                key: "",
                resource: String::new(),
                metrics: metrics_and_bytes(pdfcraft_fonts::INTER_REGULAR, value).0,
                heights: inter,
            };
            super::reference_extent_mm(&face, value, crate::model::TextHeightBasis::Cap, 5.0, None)
        };
        for (anchor, step) in [(TextAnchor::Center, 0.5), (TextAnchor::Right, 1.0)] {
            let tight = placed_span("WW", [80.0, 20.0], 0.0, anchor, TextLine::Bottom);
            let trailing = placed_span("WW ", [80.0, 20.0], 0.0, anchor, TextLine::Bottom);
            assert!(
                (trailing.origin_mm[0] - tight.origin_mm[0]).abs() < 0.05 && trailing.text == "WW",
                "{anchor:?}: a trailing pad is not lettered: {trailing:?} against {tight:?}"
            );
            let leading = placed_span(" WW", [80.0, 20.0], 0.0, anchor, TextLine::Bottom);
            assert_eq!(
                leading.text, " WW",
                "{anchor:?}: the leading pad is a glyph"
            );
            let lettered_mm = leading.advance_mm - tight.advance_mm;
            assert!(
                lettered_mm > 0.5,
                "{anchor:?}: the pad has an advance: {lettered_mm}"
            );
            let pad_mm = reference_width(" ");
            assert!(
                (leading.origin_mm[0] - (tight.origin_mm[0] - step * pad_mm)).abs() < 0.05,
                "{anchor:?}: the pad counts in the width the anchor steps back by: \
                 {leading:?} against {tight:?}"
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
        let value = "A".repeat(45);
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
                    paragraph: None,
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
        let right = Some(TextAnchor::Right);
        let shape =
            |value: &str, y: f64, rotation: f64, paragraph: Option<TextAnchor>| PaperShape::Text {
                origin_mm: [40.0, y],
                height_mm: Some(3.175),
                rotation,
                value: value.to_string(),
                font: None,
                anchor: TextAnchor::Left,
                line: TextLine::Bottom,
                height_basis: crate::model::TextHeightBasis::Cap,
                width_permille: None,
                paragraph,
                spans: Vec::new(),
            };
        let model = PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some([200.0, 200.0]),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            primitives: vec![
                primitive("decoded", shape("AB", 40.0, 0.0, None)),
                primitive("decoded", shape("AB ", 60.0, 0.0, None)),
                primitive("decoded", shape("AB ", 80.0, 0.0, right)),
                primitive("decoded", shape("AB  ", 100.0, 0.0, right)),
                primitive(
                    "decoded",
                    shape("AB ", 140.0, std::f64::consts::FRAC_PI_2, right),
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

    /// Between the centre and an edge the paragraph sits half the box away
    /// from the anchor, so a trailing pad moves the trimmed glyphs by half
    /// its width: back on a centre anchor with a left paragraph (the CCHB A2
    /// company name) and on a right anchor with a centred one (`L=200mm `),
    /// forward on a left anchor with a centred one.
    #[test]
    fn a_pad_between_centre_and_an_edge_moves_the_run_by_half_its_width() {
        let shape = |value: &str, y: f64, anchor: TextAnchor, paragraph: Option<TextAnchor>| {
            PaperShape::Text {
                origin_mm: [100.0, y],
                height_mm: Some(3.175),
                rotation: 0.0,
                value: value.to_string(),
                font: None,
                anchor,
                line: TextLine::Bottom,
                height_basis: crate::model::TextHeightBasis::Cap,
                width_permille: None,
                paragraph,
                spans: Vec::new(),
            }
        };
        let (left, center, right) = (TextAnchor::Left, TextAnchor::Center, TextAnchor::Right);
        let model = PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some([200.0, 200.0]),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            primitives: [
                shape("AB", 20.0, center, None),
                shape("AB ", 40.0, center, Some(left)),
                shape("AB", 60.0, right, None),
                shape("AB ", 80.0, right, Some(center)),
                shape("AB", 100.0, left, None),
                shape("AB ", 120.0, left, Some(center)),
                shape("AB ", 140.0, left, Some(right)),
            ]
            .into_iter()
            .map(|shape| primitive("decoded", shape))
            .collect(),
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
        let start = |y: f64| {
            ink.spans
                .iter()
                .find(|span| (span.origin_mm[1] - y).abs() < 0.5)
                .unwrap_or_else(|| panic!("no run near y = {y}: {:?}", ink.spans))
                .origin_mm[0]
        };
        let pad = start(140.0) - start(100.0);
        assert!(pad > 0.2, "one trailing space: {pad}");
        for (tight, padded, moved) in [(20.0, 40.0, -0.5), (60.0, 80.0, -0.5), (100.0, 120.0, 0.5)]
        {
            let shift = start(padded) - start(tight);
            assert!(
                (shift - moved * pad).abs() < 0.02,
                "y {padded}: moved {shift:.3} mm, half a pad is {:.3}",
                moved * pad
            );
        }
    }

    /// A template's lettering is drawn one narrower glyph than the sheet's
    /// own, at the same height, when its style states a character width.
    ///
    /// Every `.igr` furniture style in this corpus states `800`
    /// (`JStyleTextChar +52`), which [`super::permille_scale`] turns into a
    /// horizontal scale below one; a sheet run with no stated width keeps
    /// the full em. Neither moves the body size. The two runs here are the
    /// same character at the same height, so only the horizontal scale can
    /// separate them, and the expected ratio is [`super::permille_scale`]'s
    /// own reading of this test's face at `800` — the same function
    /// [`write_plot`] itself calls, so this is a round trip through the PDF
    /// rather than a second copy of the formula.
    #[test]
    fn furniture_lettering_is_condensed_horizontally_and_keeps_its_height() {
        // A lone glyph has no gap to letter-space ([`super::run_advance`]),
        // so the realised ratio matches [`super::permille_scale`] exactly
        // rather than an `lpDx`-rounded approximation of it.
        let value = "A";
        let shape = |y: f64, width_permille: Option<u16>| PaperShape::Text {
            origin_mm: [20.0, y],
            height_mm: Some(3.0),
            rotation: 0.0,
            value: value.to_string(),
            font: None,
            anchor: TextAnchor::Left,
            line: TextLine::Bottom,
            height_basis: crate::model::TextHeightBasis::Cap,
            width_permille,
            paragraph: None,
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
                primitive("decoded", shape(50.0, Some(800))),
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
        let expected = super::permille_scale(
            "",
            super::FaceHeights::of(pdfcraft_fonts::INTER_REGULAR),
            crate::model::TextHeightBasis::Cap,
            3.0,
            Some(800),
        );
        assert!(
            (ratio - expected).abs() < 0.005,
            "furniture letters at {ratio:.3} of the sheet's advance, not {expected:.3}"
        );
        assert!(
            (plate.height_mm - sheet.height_mm).abs() < 0.01,
            "the body size is not what is condensed: {} vs {}",
            plate.height_mm,
            sheet.height_mm
        );
    }

    /// A label whose style states its own character width letters at that
    /// width, furniture and sheet alike: `1000` and no stated width both
    /// leave the face's own advance alone, and any other value narrows it
    /// by [`super::permille_scale`] — the same rule a sheet label (`900`
    /// here) and a template label (`800`) both go through. A lone glyph has
    /// no gap to letter-space, so the realised ratio matches
    /// `permille_scale` exactly rather than an `lpDx`-rounded approximation.
    #[test]
    fn a_template_label_letters_at_the_width_its_style_states() {
        let shape = |y: f64, width_permille: Option<u16>| PaperShape::Text {
            origin_mm: [20.0, y],
            height_mm: Some(3.0),
            rotation: 0.0,
            value: "A".to_string(),
            font: None,
            anchor: TextAnchor::Left,
            line: TextLine::Bottom,
            height_basis: crate::model::TextHeightBasis::Cap,
            width_permille,
            paragraph: None,
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
        let heights = super::FaceHeights::of(pdfcraft_fonts::INTER_REGULAR);
        let basis = crate::model::TextHeightBasis::Cap;
        for (y, width_permille) in [
            (50.0, Some(1000)),
            (60.0, Some(800)),
            (70.0, Some(900)),
            (80.0, Some(1000)),
        ] {
            let ratio = advance(y) / sheet;
            let expected = super::permille_scale("", heights, basis, 3.0, width_permille);
            assert!(
                (ratio - expected).abs() < 0.005,
                "the run at y = {y} letters at {ratio:.3} of the sheet's advance, not {expected:.3}"
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
                    paragraph: None,
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
            paragraph: None,
            spans: &spans,
            furniture: false,
        };
        let segments = text_segments(&[], &text, "NULL*NUL", 2.822);
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
        // Different, not larger: each stretch takes its own face's plotted
        // size.
        assert!(
            (segments[1].size_pt - segments[0].size_pt).abs() > 0.5,
            "the middle stretch letters at its own size: {} vs {}",
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
                    paragraph: None,
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
                    paragraph: None,
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
                    paragraph: None,
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
        placed_span(value, origin_mm, rotation, anchor, line).origin_mm
    }

    /// The run one label reads back as, whatever padding it letters.
    fn placed_span(
        value: &str,
        origin_mm: [f64; 2],
        rotation: f64,
        anchor: TextAnchor,
        line: TextLine,
    ) -> crate::read::TextSpan {
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
                    paragraph: None,
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
        ink.spans
            .iter()
            .find(|span| span.text.trim() == value.trim())
            .cloned()
            .unwrap_or_else(|| panic!("{value:?} missing in {:?}", ink.spans))
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
                    paragraph: None,
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

    /// The two A2 `动火点` labels are centred, and the face they name, Arial
    /// Narrow, carries none of the three characters: the box is three `?` in
    /// its reference font ([`super::reference_extent_mm`]), and the reference
    /// starts the run 2.336 / 2.391 mm left of the insertion, against 2.386
    /// for the three `?` and 2.286 for half of the 1.524 mm pitch the glyphs
    /// are lettered at. `120℃` is centred in the same test on `120?`.
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
                    paragraph: None,
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
        let px_mm = 25.4 / super::PLOT_DPI;
        let a8 = super::reference_scale(
            "Arial Narrow",
            super::FaceHeights::ARIAL,
            crate::model::TextHeightBasis::Cap,
            2.5,
        );
        let ansi_px = |value: &str| -> f64 {
            value
                .bytes()
                .map(|byte| f64::from(super::ARIAL_NARROW_72PX[usize::from(byte - b' ')]))
                .sum()
        };
        let boxed = 100.0 - ansi_px("???") * a8 * px_mm / 2.0;
        let pitched = 100.0 - 3.0 * 1.524 / 2.0;
        assert!(
            (borrowed.origin_mm[0] - boxed).abs() < 0.05,
            "left edge {:.3} mm, three `?` centre it at {boxed:.3}, the pitch at {pitched:.3}",
            borrowed.origin_mm[0]
        );
        let centred_digits = ink
            .spans
            .iter()
            .find(|span| span.text == "120" && span.origin_mm[0] > 70.0)
            .expect("centred digits");
        let retreat = 100.0 - centred_digits.origin_mm[0];
        let half_box = ansi_px("120?") * a8 * px_mm / 2.0;
        assert!(
            (retreat - half_box).abs() < 0.05,
            "120℃ retreats {retreat:.3} mm, half of `120?` is {half_box:.3}"
        );
    }
}
