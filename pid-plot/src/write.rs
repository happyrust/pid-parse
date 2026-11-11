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
//! split where the first lender draws only part of it. Which face lends is
//! one order for every label — [`crate::fonts::BORROW_ORDER`], NSimSun
//! first, the face the reference plots borrow — and never what the programs
//! in hand happen to hold, so a sheet letters the same whether its faces
//! were prepared for it alone or for a batch of sheets. A borrowed character
//! is drawn at the size of the face its label names and advances by the
//! lender's own width, except where a reference plot shows otherwise: the
//! `borrowed` module holds that lettering — the size the reference sets a
//! borrowed glyph at, and the step of a label borrowed whole — and says
//! which labels it reaches.
//! A string that changes script — Latin letters, Han, a full-width
//! parenthesis — is written as one text object for each script run, each on
//! the whole pixel the plotter's steps put it and spaced by its own glyphs,
//! the way the reference plots write `DWG-0100` and `集05-37`; the
//! `script_runs` module holds the cut.
//! A style height is written at the `Tf` the reference plotter would size it
//! at: the em it implies, its cell rounded to whole 150 dpi pixels (see
//! `plotted_size_pt`). Every face must be a static TrueType or
//! OpenType program pdfcraft will embed: under 16 MiB, not a collection, not
//! a variable font.

mod borrowed;
mod script_runs;
mod verdict;

pub use verdict::{
    PrimitiveFate, PrimitiveOutcome, SegmentTally, TextMarks, WriteLedger, WriteMismatch,
    primitive_verdict,
};

use self::verdict::{fill_fate, pen_fate, segment_fate, text_fate};

use std::f64::consts::TAU;

use kurbo::{Arc, BezPath, PathEl, flatten};
use pdfcraft_content::{Op, serialize_ops};
use pdfcraft_cos::{Dict, Document, Object, PdfString, Stream};
use pdfcraft_fonts::embedding::{self, embed};
use pdfcraft_fonts::pdf::Metrics;

use crate::fonts::{BORROW_ORDER, typeface_key};
use crate::model::PaperTextSpan;
use crate::model::{
    DashFit, PaperModel, PaperShape, PaperStroke, STANDARD_SHEETS_MM, TextAnchor, TextHeightBasis,
    TextLine,
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

/// The pens the reference plotter used, in paper millimetres.
///
/// Half, one, two and three times the mid pen: `0.169 / 0.339 / 0.678 /
/// 1.017` — and a fifth at four times it, [`PEN_HEAVY_MM`].
/// Three pens were all the five-drawing corpus ever showed. The A2
/// reference plot added on 2026-11-02 prints a fourth: its
/// `CPECCHB-SQ-A2.igr` title block declares 1.0606 / 1.1313 / 1.4141 mm
/// rules, and the plot prints every one of them at `1.02` — the wide pen,
/// not the thick one the table used to fold them onto.
const PEN_THIN_MM: f64 = 0.169;
const PEN_MID_MM: f64 = 0.339;
const PEN_THICK_MM: f64 = 0.678;
const PEN_WIDE_MM: f64 = 1.017;
/// The fifth pen, eight device units: `3.84 w`.
///
/// Only the A2 reference prints it, and only on the four lines of the
/// `CPECCHB-SQ-A2.igr` frame, which declare 1.4141 mm. They run the length of
/// the sheet, so the reader leaves them out as sheet edges and the pen
/// pairing that found the other four never saw them; `reference_plots_ink`
/// holds the sheet edges to their widths and does.
const PEN_HEAVY_MM: f64 = 1.355;

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
/// The wide/heavy boundary sits inside `(1.1313, 1.4141)`: the same title
/// block's 1.0606 and 1.1313 mm rules print wide (`2.88 w`) and its 1.4141 mm
/// frame prints heavy (`3.84 w`, [`PEN_HEAVY_MM`]). 1.27 mm is a choice
/// inside that interval, and those four frame lines are all the evidence
/// there is for the fifth pen.
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
    } else if declared_mm <= 1.27 {
        PEN_WIDE_MM
    } else {
        PEN_HEAVY_MM
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
    /// Canonical name from [`typeface_key`].
    ///
    /// Any other name — `pid_plot` uses the empty one — marks a face of the
    /// caller's own: no label is named in it, and a label asks it only after
    /// every face of [`BORROW_ORDER`].
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
    write_plot_audited(model, fonts).map(|written| written.pdf)
}

/// A page as written: its bytes, what the sheet edge decided, and the write
/// layer's own account of every primitive.
///
/// The three travel together because they are answers about the same write.
/// [`write_plot`] keeps only the bytes, [`write_ledger`] takes the account
/// without writing a page at all, and a gate that wants both reads the whole
/// value.
#[derive(Clone, Debug)]
pub struct WrittenPlot {
    /// The PDF.
    pub pdf: Vec<u8>,
    /// What the sheet edge, rather than the drawing, decided.
    pub audit: PlotAudit,
    /// One verdict per page primitive, in `PaperModel::primitives` order.
    pub ledger: WriteLedger,
}

/// The write layer's account of `model`, read without any typeface.
///
/// This is the same verdict [`write_plot_audited`] reaches — every primitive
/// gets its appendix A.5 answer — but nothing is written and no face is
/// loaded, so the two font marks (`text_condensed`, `text_clipped_at_floor`)
/// are absent and `fonts_measured` is `false`. A drawing that states no page
/// is [`PlotWriteError::NoPage`], exactly as the writer would say.
pub fn write_ledger(model: &PaperModel) -> Result<WriteLedger, PlotWriteError> {
    let Some(page) = model.resolved_page_mm() else {
        return Err(PlotWriteError::NoPage);
    };
    let fates = model
        .primitives
        .iter()
        .map(|primitive| primitive_verdict(primitive, page))
        .collect();
    Ok(WriteLedger {
        fonts_measured: false,
        fates,
        mismatches: Vec::new(),
    })
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

/// Write `model` to PDF bytes and report what the sheet edge decided, plus the
/// write layer's account of every primitive.
pub fn write_plot_audited(
    model: &PaperModel,
    fonts: &[PlotFont<'_>],
) -> Result<WrittenPlot, PlotWriteError> {
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
    // Every primitive is judged up front, including the `probe_only` ones
    // `push_layer` never visits: the ledger owes every primitive an answer, and
    // the answer has to be the same one the writing pass acts on.
    let page = [page_w, page_h];
    let fates = model
        .primitives
        .iter()
        .map(|primitive| primitive_verdict(primitive, page))
        .collect();
    let mut ledger = WriteLedger {
        fonts_measured: true,
        fates,
        mismatches: Vec::new(),
    };

    let mut doc = Document::new_empty();
    let mut font_resources = Dict::new();
    let fonts = embed_faces(&mut doc, &mut font_resources, fonts)?;

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
    {
        // Scoped so the two mutable borrows end before `audit` and `ledger`
        // are read again for the returned value.
        let mut sink = WriteSink {
            audit: &mut audit,
            ledger: &mut ledger,
        };
        // Whether the page has started stroking round-ended; see
        // [`starts_round_ends`]. One page, so the two layers share it.
        let mut round_ends = false;
        push_layer(
            &mut ops,
            model,
            &fonts,
            "decoded",
            "Decoded",
            page,
            &mut sink,
            &mut round_ends,
        )?;
        push_layer(
            &mut ops,
            model,
            &fonts,
            "inferred",
            "Inferred",
            page,
            &mut sink,
            &mut round_ends,
        )?;
    }

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
    Ok(WrittenPlot { pdf, audit, ledger })
}

/// Embed `fonts` in `doc`, filing each under `font_resources`, and hand them
/// back the way the page letters with them: the key a run asks for, the
/// resource name `Tf` selects, the metrics read back off what was embedded and
/// the face's own heights. The order is the caller's.
fn embed_faces<'a>(
    doc: &mut Document,
    font_resources: &mut Dict,
    fonts: &[PlotFont<'a>],
) -> Result<Vec<ReadyFace<'a>>, PlotWriteError> {
    let mut ready = Vec::new();
    for font in fonts {
        let (family, _) = embedding::names(font.bytes).map_err(PlotWriteError::Font)?;
        let resource =
            embed(doc, font_resources, font.bytes, &family).map_err(PlotWriteError::Font)?;
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
            declare_subset(doc, *reference, &family, font.bytes)?;
        }
        let font_dict = doc
            .dict(&font_ref)
            .ok_or_else(|| PlotWriteError::Font("embedded font is not a dictionary".into()))?;
        ready.push(ReadyFace {
            key: font.key,
            resource,
            metrics: Metrics::from_dict(doc, &font_dict),
            heights: FaceHeights::of(font.bytes),
        });
    }
    Ok(ready)
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
    /// cap, the ascender its ascent, the half ascender half of it and the
    /// descender its descent below, each
    /// over the reading `basis` names. A face [`REFERENCE_CAP_PX`] carries
    /// reads GDI's own pixels and the descent is the rest of the 72-pixel
    /// cell; any other face puts its own tables on that cell, as
    /// [`reference_scale`] does. So Arial's cap-height middle is 22/46 =
    /// 0.478 of the style height, not half, and SimSun's and FangSong's, 62
    /// of whose 72 pixels are ascent, are 0.528 and 0.539.
    ///
    /// A style that measures the ascent hangs its middle half the cap down
    /// instead: all 73 ascent-height Arial labels on the ten standard-sheet
    /// reference plots land on their reference pixel with the baseline 23/58
    /// of a style height down, where 22/58 puts 16 of them a pixel high.
    /// Arial Narrow's cap, 44 of its 58 pixels of ascent, makes the two the
    /// same.
    fn drop(self, key: &str, line: TextLine, basis: TextHeightBasis) -> f64 {
        let cell = (self.ascent + self.descent).max(1e-9);
        let cap72 = reference_cap_px(key, TextHeightBasis::Cap)
            .unwrap_or(72.0 * self.stated_cap.unwrap_or(UNSTATED_CAP_EM) / cell);
        let asc72 =
            reference_cap_px(key, TextHeightBasis::Ascent).unwrap_or(72.0 * self.ascent / cell);
        let desc72 = 72.0 - asc72;
        let line72 = match line {
            TextLine::Bottom => 0.0,
            TextLine::Middle => match basis {
                TextHeightBasis::Cap => (asc72 - desc72) / 2.0,
                TextHeightBasis::Ascent => cap72 / 2.0,
            },
            TextLine::Top => cap72,
            TextLine::Ascender => asc72,
            TextLine::Descender => -desc72,
            TextLine::HalfAscender => asc72 / 2.0,
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

/// The two accounts one writing pass fills: what the sheet edge decided, and
/// what the write layer's verdict said about each primitive.
///
/// They travel together because every branch of `push_layer` that observes one
/// observes the other — a condensed run bumps [`PlotAudit::runs_condensed`]
/// *and* marks its fate, and the two must never drift.
struct WriteSink<'a> {
    audit: &'a mut PlotAudit,
    ledger: &'a mut WriteLedger,
}

/// Whether a stroke of `primitive` is where its page starts stroking
/// round-ended and round-joined.
///
/// The reference plots state no cap for the page. They state `1 j 1 J`
/// directly behind the first width that is not `0.48 w`, one device unit,
/// and never take it back: a one-unit pen is GDI's cosmetic pen, which has
/// no ends to state, and the first geometric pen brings round caps and round
/// joins that every later path is stroked under, a one-unit one included. So
/// a path stroked before that pen is under PDF's defaults — butt ends,
/// mitred corners — and every path after it is round.
///
/// Which paths come before it is the sheet's display order
/// ([`PaperModel::paint_sequence`]), and the eleven reference plots show
/// three cases. Six draw their `CPE-SQ` border first, one-unit rules
/// throughout (28 paths, butt-ended), and the first wide pen is the
/// drawing's. The A2 sheet's `CPECCHB-SQ-A2` border strokes its sheet outline
/// and then its 1.4141 mm frame, `3.84 w`, with the statement in front of
/// it. And `I井场` lists one valve of the drawing ahead of its border frame:
/// the valve is stroked first, on a two-unit pen, and the whole border after
/// it is round.
///
/// So the rule reads the pen and nothing else: the first stroke on a pen
/// wider than the thin one.
fn starts_round_ends(primitive: &crate::model::PaperPrimitive) -> bool {
    if matches!(
        primitive.shape,
        PaperShape::Fill { .. } | PaperShape::Text { .. }
    ) {
        return false;
    }
    let declared_mm = primitive
        .stroke
        .as_ref()
        .map_or(STROKE_MM, |pen| pen.width_mm);
    pen_width_mm(declared_mm) > PEN_THIN_MM
}

#[allow(clippy::too_many_arguments)]
fn push_layer(
    ops: &mut Vec<Op>,
    model: &PaperModel,
    fonts: &[ReadyFace<'_>],
    confidence: &str,
    property: &str,
    page: [f64; 2],
    sink: &mut WriteSink<'_>,
    round_ends: &mut bool,
) -> Result<(), PlotWriteError> {
    // Indices, because the ledger's `fates` is indexed by the model's own
    // primitive order, not by this layer's view of it.
    let order = layer_order(model, confidence);
    if order.is_empty() {
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
    // The cap and join the page is under when this layer opens: PDF's
    // defaults until a stroke has started the round ends.
    let ends = i64::from(*round_ends);
    ops.push(Op::new("J", vec![Object::Int(ends)]));
    ops.push(Op::new("j", vec![Object::Int(ends)]));

    for index in order {
        let primitive = &model.primitives[index];
        if matches!(primitive.shape, PaperShape::Text { .. }) {
            letter_primitive(ops, fonts, primitive, index, page, sink)?;
        } else {
            stroke_primitive(ops, primitive, index, page, sink, round_ends);
        }
    }
    ops.push(Op::new("Q", Vec::new()));
    ops.push(Op::new("EMC", Vec::new()));
    Ok(())
}

/// The primitives of one layer, by index, in the order the page paints them.
///
/// A sheet that says what it draws over what ([`PaperModel::paint_sequence`])
/// is painted in that order, a label where its record stands among the
/// strokes: a stroke in the paper's colour — a label's origin mark — then
/// covers what the sheet drew before it and is covered by what it drew after,
/// as on the vendor's plot. A model that states no order is painted as a page
/// has always been written: its strokes in the model's order, then its labels
/// in the model's order.
fn layer_order(model: &PaperModel, confidence: &str) -> Vec<usize> {
    let of_layer = |index: &usize| model.primitives[*index].confidence == confidence;
    match model.paint_sequence() {
        Some(order) => order.into_iter().filter(of_layer).collect(),
        None => {
            let lettered =
                |index: &usize| matches!(model.primitives[*index].shape, PaperShape::Text { .. });
            let layer: Vec<usize> = (0..model.primitives.len()).filter(of_layer).collect();
            let strokes = layer.iter().copied().filter(|index| !lettered(index));
            let labels = layer.iter().copied().filter(lettered);
            strokes.chain(labels).collect()
        }
    }
}

/// The primitives [`write_plot`] visits, by index, in the order it puts them
/// on the page: the `decoded` layer and then the `inferred` one, each in the
/// sheet's display order when the model states one. `probe_only` primitives
/// are not written and are not listed.
///
/// A page read back holds its ink and its lettering in this order, which is
/// what lets a gate compare the two without repeating the rule.
#[must_use]
pub fn written_order(model: &PaperModel) -> Vec<usize> {
    ["decoded", "inferred"]
        .into_iter()
        .flat_map(|confidence| layer_order(model, confidence))
        .collect()
}

/// Paint one primitive that is not a label, and note what was written.
fn stroke_primitive(
    ops: &mut Vec<Op>,
    primitive: &crate::model::PaperPrimitive,
    index: usize,
    page: [f64; 2],
    sink: &mut WriteSink<'_>,
    round_ends: &mut bool,
) {
    let before = ops.len();
    let painted = paint_primitive(ops, primitive, page);
    if painted.wrote_path && !*round_ends && starts_round_ends(primitive) {
        // Stated in the layer's own state, ahead of the stroke's `q`, so it
        // holds for this stroke and for every one after it.
        ops.insert(before, Op::new("j", vec![Object::Int(1)]));
        ops.insert(before, Op::new("J", vec![Object::Int(1)]));
        *round_ends = true;
    }
    sink.ledger.observe_path(index, painted.wrote_path);
}

/// Letter one primitive if it is a label, and note what was written.
fn letter_primitive(
    ops: &mut Vec<Op>,
    fonts: &[ReadyFace<'_>],
    primitive: &crate::model::PaperPrimitive,
    index: usize,
    page: [f64; 2],
    sink: &mut WriteSink<'_>,
) -> Result<(), PlotWriteError> {
    let PaperShape::Text {
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
    else {
        return Ok(());
    };
    let shown = show_text(
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
        sink.audit,
    )?;
    sink.ledger.observe_run(index, shown.wrote_run);
    // The two condensation marks are partial-on-page facts, and they only
    // exist when a face actually lettered the run.
    if let Some(fate) = sink.ledger.fates.get_mut(index)
        && shown.wrote_run
    {
        fate.text = Some(TextMarks {
            condensed: shown.condensed,
            clipped_at_floor: shown.clipped_at_floor,
        });
    }
    Ok(())
}
/// What one primitive's paint pass actually wrote.
///
/// `wrote_path` is the one fact the ledger checks: a pen state is written even
/// when the shape has nothing to stroke, so only an `S` or an `f` means ink.
struct Painted {
    /// The pass emitted an `S` or an `f`.
    wrote_path: bool,
}

/// What one label's pass actually wrote.
///
/// `wrote_run` is the fact the ledger checks — a `BT … ET` block was emitted.
/// The two marks are appendix A.6's partial-on-page facts and are set on the
/// same branches that bump [`PlotAudit`], so the two can never disagree.
#[derive(Clone, Copy, Debug, Default)]
struct Shown {
    /// A `BT … ET` block was written.
    wrote_run: bool,
    /// The run was condensed to fit the page.
    condensed: bool,
    /// The run was condensed to the floor and still lost its tail.
    clipped_at_floor: bool,
}

/// Paint one non-text primitive in its own pen state: `q`, the stroke's
/// colour/width/dash or the fill's colour, the path, `S` or `f`, `Q`.
/// The layer's `J`/`j` ride inside every saved state, so a one-stroke
/// group keeps the layer's cap and join.
fn paint_primitive(
    ops: &mut Vec<Op>,
    primitive: &crate::model::PaperPrimitive,
    page: [f64; 2],
) -> Painted {
    if let PaperShape::Fill { chain_mm, rgb } = &primitive.shape {
        if fill_fate(chain_mm).is_some() {
            return Painted { wrote_path: false };
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
        return Painted { wrote_path: true };
    }
    if matches!(primitive.shape, PaperShape::Text { .. }) {
        return Painted { wrote_path: false };
    }
    let pen = primitive.stroke.clone().unwrap_or(PaperStroke {
        rgb: [0, 0, 0],
        width_mm: STROKE_MM,
        dash_mm: Vec::new(),
        dash_fit: None,
    });
    if pen_fate(pen.width_mm).is_some() {
        return Painted { wrote_path: false };
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
        let wrote_path = paint_fitted_pieces(ops, fit, &primitive.shape, page, primitive.furniture);
        ops.push(Op::new("Q", Vec::new()));
        return Painted { wrote_path };
    }
    if let Some(dash) = dash_array(&pen) {
        ops.push(Op::new("d", vec![Object::Array(dash), Object::Real(0.0)]));
    }
    let wrote_path = paint_shape(ops, &primitive.shape, page, primitive.furniture);
    if wrote_path {
        ops.push(Op::new("S", Vec::new()));
    }
    ops.push(Op::new("Q", Vec::new()));
    Painted { wrote_path }
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
) -> bool {
    let mut any = false;
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
            any = true;
        }
    }
    any
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
        // One copy of the rule, in `write::verdict`, so the page and the
        // ledger cannot disagree about whether this segment prints.
        let prints = segment_fate(start, end, page, keep_sheet_edge).is_none();
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

/// One run of a label: a stretch of it that one face letters.
struct FaceRun<'a> {
    /// The face that encodes the run.
    face: &'a ReadyFace<'a>,
    /// The run in that face's encoding.
    bytes: Vec<u8>,
    /// The characters `bytes` encode — [`run_advance`] needs them back to
    /// read the Latin 72-pixel tables by codepoint, since the bytes carry
    /// this writer's own assigned CID instead.
    text: String,
    /// Whether `face` lends the run: it is not the face the stretch is named
    /// in, which cannot draw these characters.
    borrowed: bool,
}

/// The faces a label's characters are borrowed from, in the order they are
/// asked: the faces of [`BORROW_ORDER`] that are embedded, and then any face
/// embedded under a name that order does not hold, as it was handed over.
///
/// The order is the rule. [`crate::fonts::prepare_fonts`] has put into each
/// face what that order sends it, so the answer does not turn on what else a
/// batch of sheets happened to put in a program, nor on where a face stands
/// in the list. A face of the order this machine has no file for is not
/// embedded, and the next one is asked.
///
/// What follows the order is not one of this crate's faces at all: a caller
/// may embed a face of its own under any other name — `pid_plot` embeds the
/// file `PID_PLOT_CJK_FONT` names under the empty one. Such a face is asked
/// last — for a character no face of the order draws, or a stretch none of
/// them draws whole — and among several the order they were handed over in
/// decides, since nothing else tells them apart.
fn lenders<'f, 'a>(faces: &'f [ReadyFace<'a>]) -> impl Iterator<Item = &'f ReadyFace<'a>> {
    BORROW_ORDER
        .into_iter()
        .filter_map(move |key| faces.iter().find(|face| face.key == key))
        .chain(
            faces
                .iter()
                .filter(|face| !BORROW_ORDER.contains(&face.key)),
        )
}

/// Split `value`, a stretch of `label` named in the face `key`, into runs
/// that share one face, preferring that face. A character the named face
/// cannot encode moves to the first of the [`lenders`] that can —
/// only that character moves, so `120℃` keeps its digits in the narrow face
/// instead of dragging the whole run to a lender whose Latin is wider.
///
/// A named face that encodes nothing in `value` is a different case. Taking
/// each character from the first lender that owns it could cut the label
/// apart where the first lender draws only some of it, and the reader then
/// sees two runs where the record states one. The whole string goes to the
/// first lender that can draw every character, as one borrowed run. A named
/// face that draws any character keeps the per-character split.
///
/// A run may name a face that is not embedded — the file is optional, and a
/// machine may lack it. That stretch is lettered as the label's own face
/// would letter it, so `key` then stands for `label.typeface`.
fn encode_runs<'a>(
    faces: &'a [ReadyFace<'a>],
    label: &ShownText<'_>,
    key: &str,
    value: &str,
) -> Result<Vec<FaceRun<'a>>, PlotWriteError> {
    let char_encodes = |face: &ReadyFace<'_>, ch: char| {
        let mut buf = [0u8; 4];
        face.metrics.encode(ch.encode_utf8(&mut buf)).is_some()
    };
    let named = faces
        .iter()
        .find(|face| face.key == key)
        .or_else(|| faces.iter().find(|face| face.key == label.typeface));
    let lent = |face: &ReadyFace<'_>| !named.is_some_and(|named| std::ptr::eq(named, face));
    let named_draws_any = named.is_some_and(|face| value.chars().any(|ch| char_encodes(face, ch)));
    if !named_draws_any
        && let Some((face, bytes)) =
            lenders(faces).find_map(|face| face.metrics.encode(value).map(|bytes| (face, bytes)))
    {
        return Ok(vec![FaceRun {
            face,
            bytes,
            text: value.to_string(),
            borrowed: lent(face),
        }]);
    }
    let face_for_char = |ch: char| -> Option<&'a ReadyFace<'a>> {
        named
            .filter(|face| char_encodes(face, ch))
            .or_else(|| lenders(faces).find(|face| char_encodes(face, ch)))
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
                .map(|bytes| FaceRun {
                    face,
                    bytes,
                    borrowed: lent(face),
                    text: text.clone(),
                })
                .ok_or_else(|| missing_glyph(&face.metrics, &text))
        })
        .collect()
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
        // embedded set does not carry sizes off the face [`encode_runs`]
        // letters it with — the label's own, and failing that the first of
        // the [`lenders`] able to encode the string — because
        // `Tf`'s size and the reference-font model in `run_advance` both
        // read `height_basis` off *a* face's own `FaceHeights`, and the two
        // have to be the same face or the size and the glyph widths stop
        // agreeing. Only a face list with nothing able to draw the string at
        // all falls back to [`FaceHeights::ARIAL`].
        let heights = faces
            .iter()
            .find(|face| face.key == typeface)
            .or_else(|| faces.iter().find(|face| face.key == text.typeface))
            .or_else(|| lenders(faces).find(|face| face.metrics.encode(text.value).is_some()))
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

/// Where the runs of a label's record start, other than its first: character
/// offsets into `inked`, ascending.
///
/// The record's partition is kept whatever the runs' styles are. `SP:` is the
/// one label of the eleven reference sheets that states one — `SP` and `:`,
/// both in the same face at the same height.
fn record_run_starts(text: &ShownText<'_>, inked: &str) -> Vec<usize> {
    let leading = |value: &str| value.chars().take_while(|c| c.is_whitespace()).count();
    let lead = leading(text.value).saturating_sub(leading(inked));
    let end = lead + inked.chars().count();
    let mut starts: Vec<usize> = text
        .spans
        .iter()
        .map(|span| usize::from(span.start))
        .filter(|start| *start > lead && *start < end)
        .map(|start| start - lead)
        .collect();
    starts.sort_unstable();
    starts.dedup();
    starts
}

/// Cut `runs` where a run of the record starts, and say which of the
/// resulting runs open one.
///
/// `starts` are character offsets into the label the runs spell out in
/// order ([`record_run_starts`]). A run with no start inside it is handed
/// back as it was; so is one whose pieces its face will not encode apart.
fn cut_at_record_runs<'a>(
    runs: Vec<(f64, FaceRun<'a>)>,
    starts: &[usize],
) -> (Vec<(f64, FaceRun<'a>)>, Vec<bool>) {
    let mut cut = Vec::with_capacity(runs.len());
    let mut opens = Vec::with_capacity(runs.len());
    // Characters of the label before the run in hand.
    let mut at = 0usize;
    for (size_pt, run) in runs {
        let chars: Vec<char> = run.text.chars().collect();
        let end = at + chars.len();
        let begins = starts.contains(&at);
        let mut ends: Vec<usize> = starts
            .iter()
            .filter(|start| **start > at && **start < end)
            .map(|start| start - at)
            .collect();
        ends.push(chars.len());
        let mut parts: Vec<(String, Vec<u8>)> = Vec::with_capacity(ends.len());
        let mut from = 0;
        for until in ends {
            let part: String = chars[from..until].iter().collect();
            match run.face.metrics.encode(&part) {
                Some(bytes) => parts.push((part, bytes)),
                None => {
                    parts.clear();
                    break;
                }
            }
            from = until;
        }
        if parts.len() < 2 {
            opens.push(begins);
            cut.push((size_pt, run));
        } else {
            for (index, (part, bytes)) in parts.into_iter().enumerate() {
                opens.push(index > 0 || begins);
                cut.push((
                    size_pt,
                    FaceRun {
                        face: run.face,
                        bytes,
                        text: part,
                        borrowed: run.borrowed,
                    },
                ));
            }
        }
        at = end;
    }
    (cut, opens)
}

/// What the plotter measures a run at, in its own pixels, before any glyph is
/// put on a whole one: the sum [`run_advance`] rounds glyph by glyph.
///
/// `sx` is the horizontal scale of the label's text matrix and `device` the
/// scale of the plotter's own sheet, as [`run_advance`] takes them. A run the
/// reference font has no cell for is measured at its own glyphs' widths.
#[allow(clippy::too_many_arguments)]
fn measured_width_px(
    face: &ReadyFace<'_>,
    bytes: &[u8],
    text: &str,
    basis: TextHeightBasis,
    height_mm: f64,
    size_pt: f64,
    width_permille: Option<u16>,
    sx: f64,
    device: f64,
) -> f64 {
    if whole_pixel_cell(face.heights, basis, height_mm).is_none() {
        return run_width_mm(&face.metrics, size_pt, bytes) * sx * device / (25.4 / PLOT_DPI);
    }
    let cell = face.heights.ascent + face.heights.descent;
    let a8 = reference_scale(face.key, face.heights, basis, height_mm * device);
    let wf_scale = wf_reference_scale(face.heights, width_permille);
    let mut chars = text.chars();
    face.metrics
        .codes(bytes)
        .iter()
        .map(|(code, _)| {
            let unicode = chars.next().map_or(*code, u32::from);
            reference_advance_px(face.key, unicode, face.metrics.width(*code), cell, wf_scale) * a8
        })
        .sum()
}

/// How many of the plotter's pixels after the label's own start a later run
/// of its record starts.
///
/// The plotter puts every text object on a whole pixel by itself, from where
/// the object starts on the sheet, not from where the object before it
/// ended up. `start_px` is the label's start before it is rounded and
/// `before_px` what the plotter measures the runs before this one at, also
/// unrounded ([`measured_width_px`]); the label starts on `round(start_px)`
/// and this run on `round(start_px + before_px)`.
///
/// `SP:` is the case. Its `S` and `P` each measure 10.9756 pixels, so the
/// colon starts 22 pixels after the label on six reference sheets and 21 on
/// the three air-cooler sheets, whose label starts at 2281.523: the start
/// rounds up and the colon's 2303.474 rounds down. Stepping `S` and `P` by
/// their own whole pixels, 11 and 11, put it at 22 on every sheet.
fn record_run_start_px(start_px: f64, before_px: f64) -> i64 {
    #[allow(clippy::cast_possible_truncation)]
    let pixels = ((start_px + before_px).round() - start_px.round()) as i64;
    pixels
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

fn show_text(
    ops: &mut Vec<Op>,
    faces: &[ReadyFace<'_>],
    text: ShownText<'_>,
    page: [f64; 2],
    audit: &mut PlotAudit,
) -> Result<Shown, PlotWriteError> {
    // One copy of the four text rules, in `write::verdict`, so the page and the
    // ledger cannot disagree about whether this label letters.
    if text_fate(text.value, text.rotation, text.origin_mm, text.height_mm).is_some() {
        return Ok(Shown::default());
    }
    let height_mm = text.height_mm.unwrap_or(DEFAULT_TEXT_MM);
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
        // `text_fate` already refuses this case, so the branch is a second
        // guard rather than a second rule.
        return Ok(Shown::default());
    }
    // A label that letters in more than one style is cut into the stretches
    // that share a size and a face; a plain label is one stretch. Each
    // stretch is then cut where its face cannot draw a character and another
    // lends it.
    let segments = text_segments(faces, &text, inked, height_mm);
    let mut runs: Vec<(f64, FaceRun<'_>)> = Vec::new();
    for segment in &segments {
        for run in encode_runs(faces, &text, segment.typeface, &segment.text)? {
            runs.push((segment.size_pt, run));
        }
    }
    let (sin, cos) = text.rotation.sin_cos();
    let upright = sin.abs() < 1e-9;
    // A borrowed run is set at its stretch's size and spaced by its lender's
    // own widths, like any other run — except the one the reference plots
    // show: `borrowed` names it, and the size the reference sets it at.
    let as_the_reference =
        borrowed::witnessed_run(faces, &text, height_mm, upright, &segments, &runs);
    if let Some(index) = as_the_reference {
        runs[index].0 = borrowed::size_pt();
    }
    // Two runs of the record in one style are one stretch above, and one
    // string on the page would start the second where the first one's
    // whole-pixel steps end. The plotter gives each run of the record its own
    // start ([`record_run_start_px`]), so the stretch is cut back where the
    // record cuts it. A label the borrowing samples letter is left as it is.
    let (runs, starts_record_run) = if upright && as_the_reference.is_none() {
        cut_at_record_runs(runs, &record_run_starts(&text, inked))
    } else {
        let whole = vec![false; runs.len()];
        (runs, whole)
    };
    // The reference plot letters a border template's text one glyph narrower
    // than the em it sets it in, and letters the sheet's own text at the full
    // em. Height is *not* touched by this: the two sides' glyph boxes are the
    // same 4.74 / 5.08 / 5.93 / 6.60 mm for the template's 3.2 / 3.5 / 4.0 /
    // 4.5 mm styles on both sheets; what differs is only the horizontal
    // advance [`permille_scale`] computes.
    //
    // The drawing's own lettering takes its style's stated width as it
    // stands, and the template's furniture reads the same field: the symbol
    // bodies' 7.2 pt ascent `M` states `800` and the reference letters it at
    // `Tf 8.16` under a 0.809 horizontal scale, the template's own styles
    // state the same `800` and letter at 0.742..0.797 across their nine
    // heights. See [`permille_scale`].
    let named_face = faces
        .iter()
        .find(|face| face.key == text.typeface)
        .or_else(|| runs.first().map(|(_, run)| run.face));
    let scale_heights = named_face.map_or(FaceHeights::ARIAL, |face| face.heights);
    let plate = permille_scale(
        text.typeface,
        scale_heights,
        text.height_basis,
        height_mm,
        text.width_permille,
    );
    // Each run letters with `render.dll`'s own glyph-by-glyph advance — the
    // 72-unit reference font's advances scaled by `a8` and a per-glyph `Tc`,
    // the device font's natural width only for a lone glyph and, rounded to
    // the pixel, a rotated pair ([`run_advance`]) — at `plate`, the scale this whole label's
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
    let mut emitted: Vec<LetteredPiece<'_, '_>> = Vec::with_capacity(runs.len());
    // What the plotter measures everything before the run in hand at, in its
    // own pixels and before any glyph is put on a whole one: where a run of
    // the record opens.
    let mut measured_px = 0.0;
    let mut width_mm = 0.0;
    let device = device_scale(page).unwrap_or(1.0);
    for (index, (size_pt, run)) in runs.into_iter().enumerate() {
        let FaceRun {
            face,
            bytes,
            text: run_text,
            ..
        } = run;
        let opens = starts_record_run[index].then_some(measured_px);
        measured_px += measured_width_px(
            face,
            &bytes,
            &run_text,
            text.height_basis,
            height_mm,
            size_pt,
            text.width_permille,
            plate,
            device,
        );
        let advance = match named_face {
            // The run the reference shows steps by the face the label names.
            Some(named) if as_the_reference == Some(index) => borrowed::advance(
                named,
                &face.metrics,
                &bytes,
                text.height_basis,
                height_mm,
                size_pt,
                plate,
                device,
            ),
            // `face.key`/`face.heights`, not `text.typeface`/`scale_heights`:
            // the 72-unit reference tables are calibrated per *embedded* font
            // identity, and `size_pt` was sized off this same face's own
            // heights once a named face with nothing to draw the run falls
            // back to the first face that can ([`text_segments`],
            // [`encode_runs`] — the two resolve the same fallback face, so the
            // heights agree either way).
            _ => run_advance(
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
                upright,
                device,
            ),
        };
        width_mm += advance.width_mm;
        let start = opens.map_or(PieceStart::Continues, PieceStart::RecordRun);
        // The string is one call to GDI, which hands the driver a text
        // object for each script run of it ([`script_runs`]): `DWG-0100` and
        // `集05-37` are two on the reference page, each on the whole pixel
        // the steps before it add up to and spaced by its own glyphs. A
        // turned run is set glyph by glyph already, and the run the
        // borrowing samples letter is left as it is.
        let script = if upright
            && as_the_reference != Some(index)
            && advance.glyphs.len() == run_text.chars().count()
        {
            script_runs::of(&run_text)
        } else {
            Vec::new()
        };
        if script.len() < 2 {
            emitted.extend(last_glyph_apart(LetteredPiece {
                size_pt,
                face,
                bytes,
                tc_pt: advance.tc_pt,
                steps_px: advance.steps_px,
                start,
                begins_string: true,
            }));
            continue;
        }
        let lens: Vec<usize> = face
            .metrics
            .codes(&bytes)
            .iter()
            .map(|(_, len)| *len)
            .collect();
        for (order, glyphs) in script.into_iter().enumerate() {
            let from: usize = lens[..glyphs.start].iter().sum();
            let until = from + lens[glyphs.clone()].iter().sum::<usize>();
            let first = order == 0;
            emitted.extend(last_glyph_apart(LetteredPiece {
                size_pt,
                face,
                bytes: bytes[from..until].to_vec(),
                tc_pt: script_runs::spacing_pt(&advance.glyphs[glyphs.clone()], plate),
                steps_px: Vec::new(),
                start: if first {
                    start
                } else {
                    PieceStart::ScriptRun(script_runs::start_px(&advance.glyphs, glyphs.start))
                },
                begins_string: first,
            }));
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
    let mut condensed = false;
    let mut clipped_at_floor = false;
    if laid_out_mm > 0.0 && laid_out_mm.is_finite() {
        let start = baseline_origin(origin_mm, text.rotation, line_mm, below_mm, text.anchor);
        let reach = page_reach_mm(start, [cos, sin], page);
        if reach > 0.0 && laid_out_mm > reach {
            condense = (reach / laid_out_mm).clamp(MIN_CONDENSE, 1.0);
            if condense < 1.0 {
                audit.runs_condensed += 1;
                audit.smallest_condense = audit.smallest_condense.min(condense);
                condensed = true;
            }
            // Even half width leaves it hanging over the edge, so the page
            // clip takes the tail. Said out loud in [`PlotAudit`]: a run whose
            // last glyphs are off the sheet is a defect of the page, and the
            // PDF alone cannot tell anyone it happened.
            let held_mm = laid_out_mm * MIN_CONDENSE;
            if held_mm > reach {
                audit.runs_clipped_at_the_condensation_floor += 1;
                audit.deepest_clip_mm = audit.deepest_clip_mm.max(held_mm - reach);
                clipped_at_floor = true;
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
    // Where the label starts on the plotter's grid before it is put on a
    // whole pixel: what each later run of the record is placed from. A run
    // the page had to condense is not on the plotter's steps at all.
    let unrounded_px = device_scale(page)
        .filter(|_| upright && condense == 1.0)
        .map(|sheet| origin_mm[0] * sheet / (25.4 / PLOT_DPI));
    let origin_mm =
        device_scale(page).map_or(origin_mm, |scale| on_the_device_grid(origin_mm, scale));
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
    // needed it. A rotated run instead sets each glyph on its own whole-pixel
    // step ([`glyph_by_glyph`]).
    // Where the text line in force starts, where the next glyph would be set
    // and where the string in hand began: each in the plotter's pixels along
    // the baseline, after the label's own start and before any condensing.
    let mut line_px = 0.0;
    let mut pen_px = 0.0;
    let mut string_px = 0.0;
    for piece in &emitted {
        #[allow(clippy::cast_precision_loss)]
        let at_px = match piece.start {
            PieceStart::Continues => None,
            PieceStart::RecordRun(before_px) => {
                unrounded_px.map(|start_px| record_run_start_px(start_px, before_px) as f64)
            }
            PieceStart::ScriptRun(steps) => Some(string_px + steps as f64),
        };
        if let Some(at_px) = at_px {
            // `Td` moves from the line's start, in text space: the label's
            // `Tm` scales it by `plate` on its way to the page, and by
            // `condense` where the page had to squeeze the label, together
            // with its glyphs. A pixel is the page's own here, as it is for
            // every glyph's step ([`stepped_advance`]).
            let along_pt = (at_px - line_px) * (72.0 / PLOT_DPI) / plate;
            ops.push(Op::new(
                "Td",
                vec![Object::Real(along_pt), Object::Real(0.0)],
            ));
            line_px = at_px;
            pen_px = at_px;
        }
        if piece.begins_string {
            string_px = pen_px;
        }
        ops.push(Op::new("Tc", vec![Object::Real(piece.tc_pt)]));
        ops.push(Op::new(
            "Tf",
            vec![
                Object::name(&piece.face.resource),
                Object::Real(piece.size_pt),
            ],
        ));
        if piece.steps_px.is_empty() {
            ops.push(Op::new(
                "Tj",
                vec![Object::String(PdfString::literal(piece.bytes.clone()))],
            ));
        } else {
            ops.push(Op::new(
                "TJ",
                vec![Object::Array(glyph_by_glyph(
                    &piece.face.metrics,
                    &piece.bytes,
                    &piece.steps_px,
                    piece.size_pt,
                    plate,
                ))],
            ));
        }
        pen_px += shown_px(piece, plate);
    }
    ops.push(Op::new("ET", Vec::new()));
    Ok(Shown {
        wrote_run: true,
        condensed,
        clipped_at_floor,
    })
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

/// Straight segments `write_plot` strokes, in the order it strokes them
/// ([`written_order`]): Decoded then Inferred. ProbeOnly is omitted.
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
    for index in written_order(model) {
        let primitive = &model.primitives[index];
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

/// The scale the reference's plotter draws a page at, or `None` where that
/// scale is not known.
///
/// The plotter fills the whole [`PLOT_DPI`] pixels its paper is wide with the
/// sheet's width, and draws both axes at that one scale from the page's
/// lower-left corner: an A0 sheet's 7021.65 pixels become 7021. Every label
/// start on the ten reference plots drawn on a standard sheet sits on that
/// grid. A page that is not itself one of the [`STANDARD_SHEETS_MM`] was
/// fitted to some other paper by a mapping this is not — the `594.33 × 420.57`
/// sheet A2 states was printed on `594 × 420` — so it has none.
fn device_scale(page_mm: [f64; 2]) -> Option<f64> {
    let is_paper = STANDARD_SHEETS_MM.iter().any(|sheet| {
        [*sheet, [sheet[1], sheet[0]]].iter().any(|paper| {
            (paper[0] - page_mm[0]).abs() < 1e-3 && (paper[1] - page_mm[1]).abs() < 1e-3
        })
    });
    let width_px = page_mm[0] * PLOT_DPI / 25.4;
    if is_paper && width_px >= 1.0 {
        Some(width_px.floor() / width_px)
    } else {
        None
    }
}

/// Where the reference's device sets a run that starts at `start_mm`.
///
/// The start is rounded once, after the page is scaled ([`device_scale`]), to
/// a whole pixel from the page's lower-left corner — not the insertion point
/// and the alignment offset each on their own. The page here is written at
/// 1:1, so the rounded start is carried back by the same scale, and the
/// comparison's fit takes that scale out with the rest of the page.
fn on_the_device_grid(start_mm: [f64; 2], scale: f64) -> [f64; 2] {
    let pixel_mm = 25.4 / PLOT_DPI;
    start_mm.map(|mm| (mm * scale / pixel_mm).round() * pixel_mm / scale)
}

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
///
/// NSimSun reads what SimSun reads — the two are one design and one set of
/// vertical tables — and has its row so that a run borrowed from it
/// ([`crate::fonts::BORROW_ORDER`]) is spaced exactly as the same run
/// borrowed from SimSun is. No label of the corpus names NSimSun itself.
const REFERENCE_CAP_PX: [(&str, f64, f64); 5] = [
    // (face key, cap72 — code 0x0D, asc72 — code 0x0C)
    ("SimSun", 49.2, 62.0),
    ("NSimSun", 49.2, 62.0),
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
/// A single glyph has no gap to space. A two-glyph run that is not `upright`
/// steps by the device font's own advance rounded to the pixel instead: GDI
/// does not hint a rotated device font, so there the two paths part, and the
/// reference letters a rotated Arial Narrow `NC` 13 px apart (`N`'s unhinted
/// 12.54 px) where the hinted table reads 12.
///
/// A rotated run is not lettered as one string either. The reference splits
/// it into glyphs, each under its own `Tm` at its cumulative step, so they
/// land on whole pixels however the 1/2400 in grid rounds the `Tf`
/// ([`plotted_size_pt`]); such a run comes back with `steps_px` and no
/// `tc_pt`, for [`glyph_by_glyph`]. An upright run keeps the one `Tj` and
/// `Tc` the reference writes for it, that grid's drift included.
///
/// Both `width_mm` and `tc_pt` are figured at `sx = 1` —
/// the one `Tm` horizontal scale the caller's whole label carries
/// ([`permille_scale`]) is what turns either into
/// what actually lands on the page, the same way it turns every glyph's
/// own outline narrower together with its advance (the reason `render.dll`
/// drew a condensed run with a condensed device font rather than a kerning
/// table). `sx` is only this function's divisor for `tc_pt`, so a `Tm` of
/// `1` reproduces it exactly; the caller never multiplies `tc_pt` again. See
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
    /// The whole-pixel step each glyph but the last takes, in 150 dpi
    /// pixels at `Tm`'s horizontal scale `sx`, for a run the reference
    /// letters glyph by glyph: a rotated one ([`glyph_by_glyph`]). Empty for
    /// a run lettered as one string under `tc_pt`.
    steps_px: Vec<i64>,
    /// Every glyph of an upright run as the plotter steps it — its whole
    /// pixels and its own width in the device font at `sx` — which is what a
    /// string cut into script runs places and spaces each of them from
    /// ([`script_runs`]). Empty for a run of one glyph, a rotated run and a
    /// run the reference font has no cell for.
    glyphs: Vec<script_runs::Stepped>,
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
    upright: bool,
    device: f64,
) -> RunAdvance {
    let codes = metrics.codes(bytes);
    let n = codes.len();
    let natural = || RunAdvance {
        tc_pt: 0.0,
        width_mm: run_width_mm(metrics, size_pt, bytes),
        steps_px: Vec::new(),
        glyphs: Vec::new(),
    };
    if n < 2 || !(sx.is_finite() && sx > 0.0) {
        return natural();
    }
    let cell = heights.ascent + heights.descent;
    let Some(cell_px) = whole_pixel_cell(heights, basis, height_mm) else {
        return natural();
    };
    let em_px = cell_px / cell;
    let widths_em: Vec<f64> = codes.iter().map(|(code, _)| metrics.width(*code)).collect();
    if n == 2 && !upright {
        #[allow(clippy::cast_possible_truncation)]
        let step = (widths_em[0] * em_px * sx).round() as i64;
        #[allow(clippy::cast_precision_loss)]
        let width_mm = step as f64 * 25.4 / PLOT_DPI / sx + widths_em[1] * size_pt / PT_PER_MM;
        return RunAdvance {
            tc_pt: 0.0,
            width_mm,
            steps_px: vec![step],
            glyphs: Vec::new(),
        };
    }
    // The steps are rounded in the plotter's own pixels, on a sheet `device`
    // times this page ([`device_scale`]), so a run whose drift sits within
    // that sliver of a whole pixel is nudged as the reference's is:
    // `AC-01031A`, Arial Narrow 10 pt on A1, drifts 0.996 px there and keeps
    // 106 px where this page's own 1.008 would push it to 107.
    let a8 = reference_scale(key, heights, basis, height_mm * device);
    let wf_scale = wf_reference_scale(heights, width_permille);
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
    let lp_dx = pixel_steps(codes.iter().zip(&widths_em).map(|((code, _), &width_em)| {
        let unicode = chars.next().map_or(*code, u32::from);
        reference_advance_px(key, unicode, width_em, cell, wf_scale) * a8
    }));
    stepped_advance(lp_dx, &widths_em, em_px, size_pt, sx, upright)
}

/// The whole-pixel step of each glyph of a run, from the continuous advance
/// the reference font measures each at, in device pixels.
///
/// `lpDx_i = round(Δref_i × a8)`, each glyph's own step rounded on its own —
/// not the running total rounded once per glyph, which drifts a full em's
/// worth of identical glyphs a pixel short (a three-glyph run of the same
/// full-em CJK code once settled a pixel under the reference's own reading
/// this way: every other glyph in the corpus happens to vary enough from its
/// neighbours that the two roundings agree). The running floating-point and
/// already-rounded totals are tracked so a step can still be nudged by one
/// when they drift more than a pixel apart — the error diffusion
/// [`RunAdvance`]'s own doc comment describes.
fn pixel_steps(advances_px: impl IntoIterator<Item = f64>) -> Vec<i64> {
    let mut float_acc = 0.0;
    let mut int_acc: i64 = 0;
    let mut lp_dx = Vec::new();
    for step_px in advances_px {
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
    lp_dx
}

/// The advance of a run of two or more glyphs whose whole-pixel steps are
/// `lp_dx`, one per glyph, drawn in a face whose glyphs are `widths_em` wide
/// at an em of `em_px` device pixels and `size_pt` points.
fn stepped_advance(
    mut lp_dx: Vec<i64>,
    widths_em: &[f64],
    em_px: f64,
    size_pt: f64,
    sx: f64,
    upright: bool,
) -> RunAdvance {
    let n = widths_em.len();
    let leading: i64 = lp_dx[..n - 1].iter().sum();
    #[allow(clippy::cast_precision_loss)]
    let leading_px = leading as f64;
    let embedded_px: f64 = widths_em[..n - 1].iter().sum::<f64>() * em_px * sx;
    #[allow(clippy::cast_precision_loss)]
    let tc_pt = (leading_px - embedded_px) / (n - 1) as f64 * (72.0 / PLOT_DPI) / sx;
    // The page renders the first `n - 1` glyphs at `leading_px` (that is
    // what `tc_pt` was solved to make them do, and where [`glyph_by_glyph`]
    // sets a rotated run's) and the last glyph at its
    // own embedded-font natural width, under `Tc = 0`
    // ([`last_glyph_byte_offset`]'s split) — not at the reference font's
    // own `lp_dx` reading of it, which `widths_em[n - 1]` never enters.
    // Summing all `n` entries of `lp_dx` instead double-counts the
    // reference font's own rounding on the glyph that is never drawn
    // under its correction.
    let width_mm = leading_px * 25.4 / PLOT_DPI / sx + widths_em[n - 1] * size_pt / PT_PER_MM;
    if upright {
        let glyphs = lp_dx
            .iter()
            .zip(widths_em)
            .map(|(&step_px, width_em)| script_runs::Stepped {
                step_px,
                natural_px: width_em * em_px * sx,
            })
            .collect();
        return RunAdvance {
            tc_pt,
            width_mm,
            steps_px: Vec::new(),
            glyphs,
        };
    }
    lp_dx.truncate(n - 1);
    RunAdvance {
        tc_pt: 0.0,
        width_mm,
        steps_px: lp_dx,
        glyphs: Vec::new(),
    }
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

/// Where a piece of a label starts along its baseline.
#[derive(Clone, Copy, Debug, PartialEq)]
enum PieceStart {
    /// Where the piece before it left the text position.
    Continues,
    /// On a whole pixel of its own, as a run of the record: what the plotter
    /// measures everything before it at ([`record_run_start_px`]).
    RecordRun(f64),
    /// As a later script run of its string: so many whole pixels after the
    /// string's first glyph ([`script_runs::start_px`]).
    ScriptRun(i64),
}

/// One piece of a label as the page letters it.
struct LetteredPiece<'f, 'a> {
    size_pt: f64,
    face: &'f ReadyFace<'a>,
    bytes: Vec<u8>,
    /// Its `Tc`, in points of text space.
    tc_pt: f64,
    /// The whole-pixel steps a rotated run is set at ([`glyph_by_glyph`]);
    /// empty for a piece shown as one string.
    steps_px: Vec<i64>,
    start: PieceStart,
    /// Whether it opens a string — one call to GDI — rather than carrying
    /// one on: the place a later script run of that string is counted from.
    begins_string: bool,
}

/// A piece as the page shows it: one string under its `Tc`, or, where it has
/// a `Tc` and more than one glyph, every glyph but the last under that `Tc`
/// and the last one on its own under none.
///
/// `Tc` is a graphics-state parameter: once set it adds to *every* glyph's
/// own displacement, including the last one's. The reference's `Tc` is only
/// the correction for the `n - 1` gaps *between* glyphs ([`run_advance`]), so
/// a piece that needs one is split here rather than let the PDF apply a
/// correction meant for `n - 1` transitions to `n` of them.
fn last_glyph_apart<'f, 'a>(piece: LetteredPiece<'f, 'a>) -> Vec<LetteredPiece<'f, 'a>> {
    let split = (piece.tc_pt != 0.0)
        .then(|| last_glyph_byte_offset(&piece.face.metrics, &piece.bytes))
        .flatten()
        .filter(|split| *split > 0 && *split < piece.bytes.len());
    let Some(split) = split else {
        return vec![piece];
    };
    let tail = LetteredPiece {
        size_pt: piece.size_pt,
        face: piece.face,
        bytes: piece.bytes[split..].to_vec(),
        tc_pt: 0.0,
        steps_px: Vec::new(),
        start: PieceStart::Continues,
        begins_string: false,
    };
    let mut head = piece;
    head.bytes.truncate(split);
    head.steps_px = Vec::new();
    vec![head, tail]
}

/// How far showing `piece` moves the text position along the baseline, in
/// the plotter's pixels under a text matrix scaled by `sx`: every glyph's
/// own width and the piece's `Tc`, or for a piece set glyph by glyph its
/// steps and its last glyph's own width.
fn shown_px(piece: &LetteredPiece<'_, '_>, sx: f64) -> f64 {
    let metrics = &piece.face.metrics;
    let px_per_pt = PLOT_DPI / 72.0;
    let codes = metrics.codes(&piece.bytes);
    if piece.steps_px.is_empty() {
        return codes
            .iter()
            .map(|(code, _)| (metrics.width(*code) * piece.size_pt + piece.tc_pt) * sx * px_per_pt)
            .sum();
    }
    let stepped: i64 = piece.steps_px.iter().sum();
    let last = codes.last().map_or(0.0, |(code, _)| {
        metrics.width(*code) * piece.size_pt * sx * px_per_pt
    });
    #[allow(clippy::cast_precision_loss)]
    let stepped = stepped as f64;
    stepped + last
}

/// A rotated run's `TJ` array: each glyph but the last followed by the
/// adjustment that moves the next one onto its own whole-pixel step
/// ([`RunAdvance::steps_px`]), the way the reference sets every glyph of a
/// rotated run under its own `Tm`. A `TJ` number is thousandths of the `Tf`
/// em taken off the glyph's own advance, so each step lands on its pixel
/// whatever the 1/2400 in grid did to `size_pt`; `sx` is the horizontal
/// scale of the label's `Tm` the steps were figured at.
fn glyph_by_glyph(
    metrics: &Metrics,
    bytes: &[u8],
    steps_px: &[i64],
    size_pt: f64,
    sx: f64,
) -> Vec<Object> {
    let mut array = Vec::with_capacity(2 * steps_px.len() + 1);
    let mut at = 0;
    for (index, (code, len)) in metrics.codes(bytes).into_iter().enumerate() {
        array.push(Object::String(PdfString::literal(
            bytes[at..at + len].to_vec(),
        )));
        at += len;
        if let Some(&step) = steps_px.get(index) {
            #[allow(clippy::cast_precision_loss)]
            let step_em = step as f64 * (72.0 / PLOT_DPI) / (size_pt * sx);
            array.push(Object::Real(1000.0 * (metrics.width(code) - step_em)));
        }
    }
    array
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
        embedding, pen_width_mm, plotted_size_pt, text_segments, write_plot, write_plot_audited,
    };
    use crate::model::{
        PaperModel, PaperPrimitive, PaperShape, PaperTextSpan, TextAnchor, TextLine,
    };
    use crate::read::read_plot;
    use pdfcraft_fonts::pdf::Metrics;

    use super::PlotFont;

    /// The operands of every `name` operator of `ops`, numbers only.
    fn numbers(ops: &[super::Op], name: &str) -> Vec<Vec<f64>> {
        ops.iter()
            .filter(|op| op.op == name.as_bytes())
            .map(|op| {
                op.operands
                    .iter()
                    .filter_map(|operand| match operand {
                        super::Object::Real(value) => Some(*value),
                        _ => None,
                    })
                    .collect()
            })
            .collect()
    }

    /// The bytes every `Tj` of `ops` shows, in order.
    fn strings_shown(ops: &[super::Op]) -> Vec<Vec<u8>> {
        ops.iter()
            .filter(|op| op.op == b"Tj")
            .map(|op| match &op.operands[0] {
                super::Object::String(string) => string.bytes.clone(),
                other => panic!("Tj shows a string, not {other:?}"),
            })
            .collect()
    }

    /// One label lettered on an A1 sheet in `face`, the operators it writes.
    fn label_ops(
        face: &super::ReadyFace<'_>,
        value: &str,
        height_mm: f64,
        rotation: f64,
        width_permille: Option<u16>,
    ) -> Vec<super::Op> {
        let text = ShownText {
            typeface: face.key,
            origin_mm: [200.0, 100.0],
            height_mm: Some(height_mm),
            rotation,
            value,
            anchor: TextAnchor::Left,
            line: TextLine::Bottom,
            height_basis: crate::model::TextHeightBasis::Cap,
            width_permille,
            paragraph: None,
            spans: &[],
            furniture: false,
        };
        let mut ops = Vec::new();
        let mut audit = super::PlotAudit::default();
        super::show_text(
            &mut ops,
            std::slice::from_ref(face),
            text,
            [841.0, 594.0],
            &mut audit,
        )
        .expect("the label letters");
        ops
    }

    /// `bytes` embedded the way [`write_plot`] embeds a face, as the
    /// [`Metrics`] the writer letters with.
    fn metrics_of(bytes: &[u8]) -> Metrics {
        let mut doc = Document::new_empty();
        let mut font_resources = Dict::new();
        let (family, _) = embedding::names(bytes).expect("font name");
        let resource = embed(&mut doc, &mut font_resources, bytes, &family).expect("embed");
        let font_ref = font_resources
            .get(resource.as_bytes())
            .cloned()
            .expect("resource");
        let font_dict = doc.dict(&font_ref).expect("font dict");
        Metrics::from_dict(&doc, &font_dict)
    }

    /// A string that changes script is one text object for each script run:
    /// the reference writes `DWG-0100集05-37` as `DWG-0100` and `集05-37`,
    /// the second on the whole pixel the steps of the first add up to and
    /// each spaced by its own glyphs. The string ends where it ended as one
    /// text object.
    #[test]
    fn a_string_that_changes_script_is_one_text_object_for_each_script_run() {
        let value = "DWG-0100集05-37";
        // Inter with `集` standing on the glyph of `A`: a face a character
        // map says draws the whole label.
        let font = crate::fonts::test_faces::drawing("集");
        let metrics = metrics_of(&font);
        let heights = super::FaceHeights::of(&font);
        let basis = crate::model::TextHeightBasis::Cap;
        let (height_mm, page) = (2.5, [841.0, 594.0]);
        let size_pt = super::plotted_size_pt(heights, basis, height_mm);
        let device = super::device_scale(page).expect("A1 is a standard sheet");
        let bytes = metrics.encode(value).expect("the face encodes the label");
        let whole = super::run_advance(
            "", &metrics, &bytes, value, heights, basis, height_mm, size_pt, None, 1.0, true,
            device,
        );
        assert_eq!(whole.glyphs.len(), 14, "one step for each glyph");
        let (latin, han) = whole.glyphs.split_at(8);
        let spaced = |glyphs: &[super::script_runs::Stepped]| {
            let (_, leading) = glyphs.split_last().expect("glyphs");
            let short_px: f64 = leading
                .iter()
                .map(|glyph| {
                    #[allow(clippy::cast_precision_loss)]
                    let step = glyph.step_px as f64;
                    step - glyph.natural_px
                })
                .sum();
            #[allow(clippy::cast_precision_loss)]
            let gaps = leading.len() as f64;
            short_px / gaps * 72.0 / super::PLOT_DPI
        };
        let (latin_bytes, han_bytes) = (
            metrics.encode("DWG-0100").expect("encodes"),
            metrics.encode("集05-37").expect("encodes"),
        );
        let face = super::ReadyFace {
            key: "",
            resource: "F1".to_string(),
            metrics,
            heights,
        };
        let ops = label_ops(&face, value, height_mm, 0.0, None);
        let moved: Vec<usize> = ops
            .iter()
            .enumerate()
            .filter(|(_, op)| op.op == b"Td")
            .map(|(index, _)| index)
            .collect();
        assert_eq!(
            moved.len(),
            1,
            "the second script run, and only it, is moved to a start of its own"
        );
        let (before, after) = ops.split_at(moved[0]);
        assert_eq!(strings_shown(before).concat(), latin_bytes, "the Latin run");
        assert_eq!(strings_shown(after).concat(), han_bytes, "the Han run");
        // The Han run starts on the whole pixel the Latin run's steps end on.
        let start_px: i64 = latin.iter().map(|glyph| glyph.step_px).sum();
        #[allow(clippy::cast_precision_loss)]
        let start_pt = start_px as f64 * 72.0 / super::PLOT_DPI;
        let td = &numbers(&ops, "Td")[0];
        assert!(
            (td[0] - start_pt).abs() < 1e-12 && td[1] == 0.0,
            "moved {td:?}, the Latin run's {start_px} pixels are {start_pt} pt"
        );
        // Each run is spaced by its own glyphs, and its last glyph by none.
        let spacings: Vec<f64> = numbers(&ops, "Tc").into_iter().map(|tc| tc[0]).collect();
        assert!(
            spaced(latin) != 0.0 && spaced(han) != 0.0,
            "both runs must need spacing for this label to show the cut"
        );
        let expected = [spaced(latin), 0.0, spaced(han), 0.0];
        assert_eq!(spacings.len(), expected.len(), "{spacings:?}");
        for (got, want) in spacings.iter().zip(expected) {
            assert!(
                (got - want).abs() < 1e-12,
                "Tc {spacings:?}, not {expected:?}"
            );
        }
        assert!(
            (spaced(latin) - spaced(han)).abs() > 1e-6,
            "the two runs must need different spacing for this label to tell them apart"
        );
    }

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
            1.0,
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

    /// A rotated pair steps by the device font's own advance rounded to the
    /// pixel ([`super::run_advance`]): GDI does not hint a rotated font, and
    /// the reference letters a rotated Arial Narrow `NC` 13 px apart, `N`'s
    /// unhinted 12.54 px, where the hinted `lpDx` reads 12.
    #[test]
    fn a_rotated_two_glyph_run_steps_by_its_rounded_natural_advance() {
        let (metrics, bytes) = metrics_and_bytes(pdfcraft_fonts::INTER_REGULAR, "AB");
        let inter = super::FaceHeights::of(pdfcraft_fonts::INTER_REGULAR);
        let cap = crate::model::TextHeightBasis::Cap;
        let size_pt = super::plotted_size_pt(inter, cap, 4.0);
        let advance = super::run_advance(
            "", &metrics, &bytes, "AB", inter, cap, 4.0, size_pt, None, 1.0, false, 1.0,
        );
        assert_eq!(
            advance.tc_pt, 0.0,
            "a rotated pair states no letter-spacing"
        );
        let codes = metrics.codes(&bytes);
        let cell_px = super::whole_pixel_cell(inter, cap, 4.0).expect("a cell");
        let natural_px = metrics.width(codes[0].0) * cell_px / (inter.ascent + inter.descent);
        #[allow(clippy::cast_possible_truncation)]
        let rounded = natural_px.round() as i64;
        assert_eq!(
            advance.steps_px,
            vec![rounded],
            "the first glyph steps its natural {natural_px} px rounded"
        );
        let last_natural_mm = metrics.width(codes[1].0) * size_pt / super::PT_PER_MM;
        let expected_mm = natural_px.round() * 25.4 / 150.0 + last_natural_mm;
        assert!(
            (advance.width_mm - expected_mm).abs() < 1e-9,
            "the pair is that step and the last glyph's own width: {} against {expected_mm}",
            advance.width_mm
        );
    }

    /// A rotated run is lettered glyph by glyph, the way the reference sets
    /// each of `40-WO-1001405-B5-EH40`'s 21 glyphs under its own `Tm` at the
    /// cumulative `lpDx`: one `Tj` under `Tc` drifts with the 1/2400 in grid
    /// its `Tf` is rounded to, while the `TJ` adjustments land every glyph on
    /// the whole-pixel step an upright run of the same glyphs reads.
    #[test]
    fn a_rotated_run_sets_every_glyph_on_its_own_pixel() {
        let (metrics, bytes) = metrics_and_bytes(pdfcraft_fonts::INTER_REGULAR, "ABAB");
        let inter = super::FaceHeights::of(pdfcraft_fonts::INTER_REGULAR);
        let cap = crate::model::TextHeightBasis::Cap;
        let size_pt = super::plotted_size_pt(inter, cap, 4.0);
        let sx = 0.9;
        let advance = |upright| {
            super::run_advance(
                "", &metrics, &bytes, "ABAB", inter, cap, 4.0, size_pt, None, sx, upright, 1.0,
            )
        };
        let (rotated, upright) = (advance(false), advance(true));
        assert_eq!(rotated.tc_pt, 0.0, "a rotated run states no letter-spacing");
        assert!(
            upright.steps_px.is_empty(),
            "an upright run stays one string"
        );
        assert_eq!(
            rotated.steps_px.len(),
            3,
            "every glyph but the last takes a step"
        );
        assert!(
            (rotated.width_mm - upright.width_mm).abs() < 1e-9,
            "both orientations read the same lpDx"
        );
        let array = super::glyph_by_glyph(&metrics, &bytes, &rotated.steps_px, size_pt, sx);
        let codes = metrics.codes(&bytes);
        assert_eq!(
            array.len(),
            2 * codes.len() - 1,
            "each glyph but the last is followed by its adjustment"
        );
        for (index, &step) in rotated.steps_px.iter().enumerate() {
            let super::Object::Real(adjust) = &array[2 * index + 1] else {
                panic!("glyph {index} is followed by its adjustment");
            };
            let landed_px =
                (metrics.width(codes[index].0) - *adjust / 1000.0) * size_pt * sx * 150.0 / 72.0;
            #[allow(clippy::cast_precision_loss)]
            let step_px = step as f64;
            assert!(
                (landed_px - step_px).abs() < 1e-9,
                "glyph {index} lands {landed_px} px, not {step}"
            );
        }
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
            1.0,
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

    /// `lpDx` is rounded on the plotter's own sheet, [`super::device_scale`]
    /// times this page: `AC-01031A` (Arial Narrow 10 pt) drifts 0.996 px over
    /// its leading eight glyphs on A1 and keeps the 106 px the reference
    /// letters it at, a pixel short of what this page's own scale takes;
    /// `CT-10017A` drifts past a pixel on both and takes 106 px either way.
    #[test]
    fn a_run_is_spaced_on_the_plotters_own_sheet() {
        let a1 = super::device_scale([841.0, 594.0]).expect("A1 is a standard sheet");
        let inter = super::FaceHeights::of(pdfcraft_fonts::INTER_REGULAR);
        let basis = crate::model::TextHeightBasis::Cap;
        let height_mm = 10.0 / super::PT_PER_MM;
        let size_pt = super::plotted_size_pt(inter, basis, height_mm);
        let width_px = |text: &'static str, device: f64| {
            let (metrics, bytes) = metrics_and_bytes(pdfcraft_fonts::INTER_REGULAR, text);
            let advance = super::run_advance(
                "Arial Narrow",
                &metrics,
                &bytes,
                text,
                inter,
                basis,
                height_mm,
                size_pt,
                None,
                1.0,
                true,
                device,
            );
            advance.width_mm * 150.0 / 25.4
        };
        let narrower = |text: &'static str| width_px(text, 1.0) - width_px(text, a1);
        assert!(
            (narrower("AC-01031A") - 1.0).abs() < 1e-6,
            "AC-01031A keeps a pixel less on the plotter's sheet: {}",
            narrower("AC-01031A")
        );
        assert!(
            narrower("CT-10017A").abs() < 1e-6,
            "CT-10017A is nudged on either sheet: {}",
            narrower("CT-10017A")
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
    fn declared_widths_snap_onto_the_five_plot_pens() {
        use super::{
            PEN_HEAVY_MM, PEN_MID_MM, PEN_THICK_MM, PEN_THIN_MM, PEN_WIDE_MM, pen_width_mm,
        };
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
        // The CCHB A2 title block's 1.0606 and 1.1313 mm rules print on the
        // wide pen …
        for wide in [1.0, 1.0606, 1.1313, 1.27] {
            assert_eq!(pen_width_mm(wide), PEN_WIDE_MM, "{wide} mm");
        }
        // … and its 1.4141 mm frame, and anything above it, on the heavy one:
        // the A2 reference strokes the frame's four lines `3.84 w`.
        for heavy in [1.28, 1.4141, 2.0] {
            assert_eq!(pen_width_mm(heavy), PEN_HEAVY_MM, "{heavy} mm");
        }
        // A width that is not a width draws the fallback pen.
        for nonsense in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(pen_width_mm(nonsense), PEN_MID_MM, "{nonsense}");
        }
        // 0.169 / 0.339 / 0.678 / 1.017 / 1.355 mm are 0.48 / 0.96 / 1.92 /
        // 2.88 / 3.84 pt — one, two, four, six and eight device units — the
        // only five widths the reference pages carry.
        let points = |mm: f64| mm * 72.0 / 25.4;
        assert!((points(PEN_THIN_MM) - 0.48).abs() < 0.005);
        assert!((points(PEN_MID_MM) - 0.96).abs() < 0.005);
        assert!((points(PEN_THICK_MM) - 1.92).abs() < 0.005);
        assert!((points(PEN_WIDE_MM) - 2.88).abs() < 0.005);
        assert!((points(PEN_HEAVY_MM) - 3.84).abs() < 0.005);
    }

    /// `SP:` states two runs in one style, `SP` and `:`. The reference writes
    /// them as two text objects and puts each on a whole pixel from where it
    /// starts on the sheet, so the colon is 22 pixels after the label on six
    /// of the nine sheets that letter it and 21 on the three whose label
    /// starts at a fraction in `[0.5, 0.5488)`.
    #[test]
    fn a_later_run_of_the_record_starts_on_its_own_whole_pixel() {
        // The nine reference starts, in device units, and the colon's place
        // after each: two glyphs of 10.9756 units lie before it.
        for (start_px, after) in [
            (4668.299, 22),
            (5017.359, 22),
            (4366.591, 22),
            (2008.960, 22),
            (1913.635, 22),
            (2281.523, 21),
        ] {
            assert_eq!(
                super::record_run_start_px(start_px, 2.0 * 10.9756),
                after,
                "{start_px}"
            );
        }

        // On a written page: the same label with and without the record's
        // partition, at a start where the two rules agree and at one where
        // they part.
        let scale = super::device_scale([841.0, 594.0]).expect("A1 is a standard sheet");
        let pixel_mm = 25.4 / super::PLOT_DPI;
        let label = |x: f64, y: f64, partitioned: bool| {
            let span = |start: u16, len: u16| PaperTextSpan {
                start,
                len,
                height_mm: None,
                font: None,
            };
            primitive(
                "decoded",
                PaperShape::Text {
                    origin_mm: [x, y],
                    height_mm: Some(2.54),
                    rotation: 0.0,
                    value: "SP:".to_string(),
                    font: None,
                    anchor: TextAnchor::Left,
                    line: TextLine::Bottom,
                    height_basis: crate::model::TextHeightBasis::Cap,
                    width_permille: None,
                    paragraph: None,
                    spans: if partitioned {
                        vec![span(0, 2), span(2, 1)]
                    } else {
                        Vec::new()
                    },
                },
            )
        };
        // What the plotter measures `SP` at in this face, and the whole
        // pixels its two glyphs step by on their own.
        let (metrics, bytes) = metrics_and_bytes(pdfcraft_fonts::INTER_REGULAR, "SP");
        let heights = super::FaceHeights::of(pdfcraft_fonts::INTER_REGULAR);
        let basis = crate::model::TextHeightBasis::Cap;
        let cell = heights.ascent + heights.descent;
        let a8 = super::reference_scale("", heights, basis, 2.54 * scale);
        let measured: Vec<f64> = metrics
            .codes(&bytes)
            .iter()
            .zip("SP".chars())
            .map(|((code, _), ch)| {
                super::reference_advance_px("", u32::from(ch), metrics.width(*code), cell, 1.0) * a8
            })
            .collect();
        let before_px: f64 = measured.iter().sum();
        let stepped: i64 = super::pixel_steps(measured.iter().copied()).iter().sum();
        // A start whose own rounding leaves the colon where the steps put
        // it, and one where it does not.
        let start_at = |agrees: bool| {
            (0..2000)
                .map(|step| 20.0 + f64::from(step) * 0.001)
                .find(|x| {
                    let start_px = x * scale / pixel_mm;
                    (super::record_run_start_px(start_px, before_px) == stepped) == agrees
                })
                .expect("both kinds of start within two millimetres")
        };
        let (same, apart) = (start_at(true), start_at(false));
        let mut model = one_page(label(same, 40.0, false), [841.0, 594.0]);
        model.primitives.extend([
            label(same, 50.0, true),
            label(apart, 60.0, false),
            label(apart, 70.0, true),
        ]);
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
            let y = super::on_the_device_grid([20.0, y], scale)[1];
            ink.spans
                .iter()
                .find(|span| (span.origin_mm[1] - y).abs() < 0.01)
                .unwrap_or_else(|| panic!("no run at y = {y}: {:?}", ink.spans))
                .advance_mm
        };
        // Where the rules agree the partition leaves the run as long as it
        // was, to the hundredth of a millimetre one string's `Tc` drifts on
        // the 1/2400 in grid: the colon's own start has none.
        assert!(
            (advance(50.0) - advance(40.0)).abs() < 0.01,
            "{} against {}",
            advance(50.0),
            advance(40.0)
        );
        // Where they part the unpartitioned label is what it was …
        assert!(
            (advance(60.0) - advance(40.0)).abs() < 1e-6,
            "{} against {}",
            advance(60.0),
            advance(40.0)
        );
        // … and the partitioned one's colon is a whole pixel off the steps,
        // and the run's end with it.
        let moved_px = super::record_run_start_px(apart * scale / pixel_mm, before_px) - stepped;
        assert_eq!(moved_px.abs(), 1, "one pixel apart");
        #[allow(clippy::cast_precision_loss)]
        let moved_mm = moved_px as f64 * pixel_mm;
        assert!(
            (advance(70.0) - advance(50.0) - moved_mm).abs() < 1e-4,
            "{} against {} + {moved_mm}",
            advance(70.0),
            advance(50.0)
        );
    }

    /// A page of straight strokes in drawing order: whether each is the
    /// template's, the width it declares, and where it runs.
    fn stroked_page(strokes: &[(bool, f64, [f64; 2], [f64; 2])]) -> PaperModel {
        let mut model = one_page(
            primitive(
                "decoded",
                PaperShape::Line {
                    start_mm: [0.0, 0.0],
                    end_mm: [1.0, 0.0],
                },
            ),
            [200.0, 100.0],
        );
        model.primitives = strokes
            .iter()
            .map(|(furniture, width_mm, start_mm, end_mm)| {
                let mut stroke = primitive(
                    "decoded",
                    PaperShape::Line {
                        start_mm: *start_mm,
                        end_mm: *end_mm,
                    },
                );
                stroke.furniture = *furniture;
                stroke.stroke = Some(crate::model::PaperStroke {
                    rgb: [0, 0, 0],
                    width_mm: *width_mm,
                    dash_mm: Vec::new(),
                    dash_fit: None,
                });
                stroke
            })
            .collect();
        model
    }

    /// The reference plots do not state a cap for the page. They state
    /// `1 j 1 J` behind the first pen wider than one device unit and never
    /// take it back, so what a one-unit stroke ends with depends on where it
    /// is painted: before that pen it is butt-ended and mitred, after it
    /// round. Whose stroke it is does not come into it.
    #[test]
    fn a_page_strokes_round_from_its_first_wide_pen() {
        use crate::read::{LineCap, LineJoin, read_plot_pens};
        let ends =
            |strokes: &[(bool, f64, [f64; 2], [f64; 2])]| -> Vec<(LineCap, LineJoin, usize)> {
                let pdf = write_plot(&stroked_page(strokes), &inter_font()).expect("write");
                read_plot_pens(&pdf)
                    .expect("read")
                    .ends
                    .iter()
                    .map(|ends| (ends.cap, ends.join, ends.paths))
                    .collect()
            };
        let butt = |paths| (LineCap::Butt, LineJoin::Miter, paths);
        let round = |paths| (LineCap::Round, LineJoin::Round, paths);
        let (template, drawing) = (true, false);

        // A template of one-unit rules, then the drawing: a thin stroke of
        // the drawing ahead of its first wide one is under no cap either.
        assert_eq!(
            ends(&[
                (template, 0.25, [10.0, 10.0], [60.0, 10.0]),
                (template, 0.25, [10.0, 20.0], [60.0, 20.0]),
                (drawing, 0.18, [10.0, 30.0], [60.0, 30.0]),
                (drawing, 0.70, [10.0, 40.0], [60.0, 40.0]),
                (drawing, 0.18, [10.0, 50.0], [60.0, 50.0]),
            ]),
            vec![butt(3), round(2)]
        );
        // A template whose second rule is a heavy one: the rule after it is
        // round though it is thin.
        assert_eq!(
            ends(&[
                (template, 0.25, [10.0, 10.0], [60.0, 10.0]),
                (template, 1.4141, [10.0, 20.0], [60.0, 20.0]),
                (template, 0.25, [10.0, 30.0], [60.0, 30.0]),
                (drawing, 0.18, [10.0, 40.0], [60.0, 40.0]),
            ]),
            vec![butt(1), round(3)]
        );
        // A wide stroke of the drawing ahead of the template, as `I井场`
        // lists one valve ahead of its border frame: all round.
        assert_eq!(
            ends(&[
                (drawing, 0.35, [10.0, 10.0], [60.0, 10.0]),
                (template, 0.25, [10.0, 20.0], [60.0, 20.0]),
                (template, 0.25, [10.0, 30.0], [60.0, 30.0]),
            ]),
            vec![round(3)]
        );
        // Thin strokes alone never state a cap.
        assert_eq!(
            ends(&[
                (template, 0.25, [10.0, 10.0], [60.0, 10.0]),
                (drawing, 0.18, [10.0, 20.0], [60.0, 20.0]),
            ]),
            vec![butt(2)]
        );
    }

    /// A page whose sheet states what it draws over what is painted in that
    /// order, a label where it stands among the strokes; a page that states
    /// none is painted strokes first and labels after, as it always was.
    #[test]
    fn a_page_that_states_its_order_is_painted_in_it() {
        use crate::read::read_plot_sequence;
        let label = primitive(
            "decoded",
            PaperShape::Text {
                origin_mm: [20.0, 40.0],
                height_mm: Some(3.0),
                rotation: 0.0,
                value: "AB".to_string(),
                font: None,
                anchor: TextAnchor::Left,
                line: TextLine::Bottom,
                height_basis: crate::model::TextHeightBasis::Cap,
                width_permille: None,
                paragraph: None,
                spans: Vec::new(),
            },
        );
        // In the model's order: a rule, the label, a white mark, a second
        // rule.
        let mut model = stroked_page(&[
            (false, 0.35, [10.0, 10.0], [60.0, 10.0]),
            (false, 0.25, [19.0, 40.0], [21.0, 40.0]),
            (false, 0.35, [10.0, 70.0], [60.0, 70.0]),
        ]);
        model.primitives[1].stroke.as_mut().expect("a pen").rgb = [255, 255, 255];
        model.primitives.insert(1, label);
        let font = [PlotFont {
            key: "",
            bytes: pdfcraft_fonts::INTER_REGULAR,
            subset: false,
        }];
        // What is painted, in order: `rule` at its height, `mark`, or a
        // glyph.
        let painted = |model: &PaperModel| -> Vec<String> {
            let page = read_plot_sequence(&write_plot(model, &font).expect("write")).expect("read");
            let mut steps: Vec<(u32, String)> = page
                .strokes
                .iter()
                .map(|stroke| {
                    let name = if stroke.paper {
                        "mark".to_string()
                    } else {
                        format!("rule {:.0}", stroke.segment.start_mm[1])
                    };
                    (stroke.step, name)
                })
                .chain(
                    page.glyphs
                        .iter()
                        .map(|glyph| (glyph.step, glyph.text.clone())),
                )
                .collect();
            steps.sort();
            steps.into_iter().map(|(_, name)| name).collect()
        };
        // The same rules as the list of segments the page strokes.
        let stroked = |model: &PaperModel| -> Vec<f64> {
            super::stroked_segments(model)
                .iter()
                .map(|(start, _)| start[1])
                .collect()
        };
        // No order stated: the three strokes, then the label.
        assert_eq!(model.paint_sequence(), None);
        assert_eq!(super::written_order(&model), [0, 2, 3, 1]);
        assert_eq!(painted(&model), ["rule 10", "mark", "rule 70", "A", "B"]);
        assert_eq!(stroked(&model), [10.0, 40.0, 70.0]);
        // The sheet's order: the second rule, the label, the mark over the
        // label, the first rule.
        model.ledger.paint_order = vec![3, 1, 2, 0];
        assert_eq!(model.paint_sequence(), Some(vec![3, 1, 2, 0]));
        assert_eq!(super::written_order(&model), [3, 1, 2, 0]);
        assert_eq!(painted(&model), ["rule 70", "A", "B", "mark", "rule 10"]);
        assert_eq!(stroked(&model), [70.0, 40.0, 10.0]);
        // The layers are still written one after the other, each in the
        // sheet's order, and a primitive of neither is not written.
        model.primitives[3].confidence = "inferred";
        model.primitives[2].confidence = "probe_only";
        assert_eq!(super::written_order(&model), [1, 0, 3]);
        assert_eq!(painted(&model), ["A", "B", "rule 10", "rule 70"]);
        model.primitives[3].confidence = "decoded";
        model.primitives[2].confidence = "decoded";
        // An order that is not the page's — too short, or naming a primitive
        // twice — is no order.
        model.ledger.paint_order = vec![3, 1, 2];
        assert_eq!(model.paint_sequence(), None);
        model.ledger.paint_order = vec![3, 1, 1, 0];
        assert_eq!(model.paint_sequence(), None);
        assert_eq!(painted(&model), ["rule 10", "mark", "rule 70", "A", "B"]);
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
        assert!((pen_width_mm(WIDEST_PEN_MM) - super::PEN_HEAVY_MM).abs() < 1e-9);
        assert!((pen_width_mm(1.1313) - super::PEN_WIDE_MM).abs() < 1e-9);
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
            ledger: crate::ledger::LayoutLedger::default(),
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
            ledger: crate::ledger::LayoutLedger::default(),
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
                ledger: crate::ledger::LayoutLedger::default(),
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
            ledger: crate::ledger::LayoutLedger::default(),
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
            ledger: crate::ledger::LayoutLedger::default(),
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
            ledger: crate::ledger::LayoutLedger::default(),
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
            "", &metrics, &bytes, "ABA", inter, basis, 5.0, size_pt, None, 1.0, true, 1.0,
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
    /// (0.525) and the 121 FangSong `0x05` labels (0.538). An ascent-height
    /// style hangs its middle half the cap down: 23/58 for Arial, where the
    /// 73 ascent-height Arial symbol labels on the standard-sheet plots land
    /// on their pixels, and 22/58 for Arial Narrow. A face the table does not
    /// carry puts its own tables on the same 72-pixel cell.
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
            23.0 / 58.0
        ));
        assert!(close(
            arial.drop("Arial Narrow", TextLine::Middle, Ascent),
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
            arial.drop("Arial Narrow", TextLine::HalfAscender, Cap),
            29.0 / 44.0
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

    /// NSimSun is SimSun with a fixed-pitch Latin: one set of vertical tables,
    /// and the same reading of the 72-unit reference font on GDI — a cap of
    /// 49.2 and an ascent of 62. So a run the page borrows from NSimSun is
    /// scaled, and so spaced, exactly as the same run borrowed from SimSun,
    /// at every style height and on either height basis. Left to its own
    /// tables NSimSun would read 49.21875 and 61.875, and a step here and
    /// there would round the other way.
    #[test]
    fn nsimsun_is_measured_as_simsun_is() {
        use crate::model::TextHeightBasis::{Ascent, Cap};
        let song = super::FaceHeights {
            ascent: 220.0 / 256.0,
            descent: 36.0 / 256.0,
            cap: 175.0 / 256.0,
            stated_cap: Some(175.0 / 256.0),
            avg_em: 128.0 / 256.0,
        };
        for basis in [Cap, Ascent] {
            assert_eq!(
                super::reference_cap_px("NSimSun", basis),
                super::reference_cap_px("SimSun", basis),
                "{basis:?}"
            );
            for line in [
                TextLine::Bottom,
                TextLine::Middle,
                TextLine::Top,
                TextLine::Ascender,
                TextLine::Descender,
                TextLine::HalfAscender,
            ] {
                assert_eq!(
                    song.drop("NSimSun", line, basis).to_bits(),
                    song.drop("SimSun", line, basis).to_bits(),
                    "{line:?} of a {basis:?} style"
                );
            }
            for hundredths in 150..=800 {
                let height_mm = f64::from(hundredths) / 100.0;
                assert_eq!(
                    super::reference_scale("NSimSun", song, basis, height_mm).to_bits(),
                    super::reference_scale("SimSun", song, basis, height_mm).to_bits(),
                    "a {basis:?} style {height_mm} mm tall"
                );
            }
        }
        assert_eq!(super::reference_cap_px("NSimSun", Cap), Some(49.2));
        assert_eq!(super::reference_cap_px("NSimSun", Ascent), Some(62.0));
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
            ledger: crate::ledger::LayoutLedger::default(),
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
            ledger: crate::ledger::LayoutLedger::default(),
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
            ledger: crate::ledger::LayoutLedger::default(),
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
            ledger: crate::ledger::LayoutLedger::default(),
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
        let scale = super::device_scale([841.0, 594.0]).expect("A1 is a standard sheet");
        let at = |y: f64| {
            let y = super::on_the_device_grid([20.0, y], scale)[1];
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
            ledger: crate::ledger::LayoutLedger::default(),
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
        let scale = super::device_scale([841.0, 594.0]).expect("A1 is a standard sheet");
        let advance = |y: f64| {
            let y = super::on_the_device_grid([20.0, y], scale)[1];
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

    /// A run on a standard sheet starts on a whole pixel of the reference
    /// device's grid, and a run on any other page starts where the model puts
    /// it.
    ///
    /// An A1 sheet is 4966.5 pixels wide at [`super::PLOT_DPI`], which the
    /// reference plotter fills with 4966, so both axes shrink by that ratio
    /// from the lower-left corner: a label set at 20, 40 mm starts on pixel
    /// 118, 236 rather than 118.10, 236.20. A 700 × 500 page is no paper the
    /// plotter draws at its own width, and its label keeps the model's start.
    #[test]
    fn a_run_on_a_standard_sheet_starts_on_the_device_pixel_grid() {
        let start_mm = |page_mm: [f64; 2]| {
            let model = PaperModel {
                library: Vec::new(),
                template: None,
                page_mm: Some(page_mm),
                off_sheet_primitives: 0,
                skipped_endpoint_pairs: 0,
                primitives: vec![primitive(
                    "decoded",
                    PaperShape::Text {
                        origin_mm: [20.0, 40.0],
                        height_mm: Some(3.0),
                        rotation: 0.0,
                        value: "A".to_string(),
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
                ledger: crate::ledger::LayoutLedger::default(),
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
            assert_eq!(ink.spans.len(), 1, "one label, one run: {:?}", ink.spans);
            ink.spans[0].origin_mm
        };
        let pixel_mm = 25.4 / super::PLOT_DPI;
        let scale = 4966.0 / (841.0 / pixel_mm);
        let start = start_mm([841.0, 594.0]).map(|mm| mm * scale / pixel_mm);
        assert!(
            (start[0] - 118.0).abs() < 0.01 && (start[1] - 236.0).abs() < 0.01,
            "an A1 run starts at device pixel {start:?}, not 118, 236"
        );
        let start = start_mm([700.0, 500.0]);
        assert!(
            (start[0] - 20.0).abs() < 1e-3 && (start[1] - 40.0).abs() < 1e-3,
            "a 700 × 500 run starts at {start:?}, not 20, 40"
        );
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
            ledger: crate::ledger::LayoutLedger::default(),
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
            ledger: crate::ledger::LayoutLedger::default(),
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
            ledger: crate::ledger::LayoutLedger::default(),
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
            ledger: crate::ledger::LayoutLedger::default(),
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
            ledger: crate::ledger::LayoutLedger::default(),
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
    /// characters, so the whole label stays on one lender. `120℃` still
    /// splits: the digits stay on Arial Narrow.
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
            ledger: crate::ledger::LayoutLedger::default(),
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
    /// for the three `?` and 2.286 for half of three 1.524 mm steps. `120℃`
    /// is centred in the same test on `120?`.
    ///
    /// The run itself reads back as the reference's does: NSimSun where the
    /// machine has it, an em of 20 device units (`Tf 9.6`, 3.387 mm), and
    /// 38 units from its start to the end of its last glyph — two steps of 9
    /// and one full em — which is the 6.435 mm the reference's own run reads.
    /// The `℃` that ends `120℃` is the same 20 units.
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
            ledger: crate::ledger::LayoutLedger::default(),
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
        let (ink, faces) = crate::read::read_plot_with_faces(&pdf).expect("read");
        let lender = if ready("nsimsun.ttf") {
            "NSimSun"
        } else {
            "SimSun"
        };
        let (borrowed, face) = ink
            .spans
            .iter()
            .zip(&faces)
            .find(|(span, _)| span.text == "动火点" && span.origin_mm[0] > 70.0)
            .expect("the centred 动火点");
        let px_mm = 25.4 / super::PLOT_DPI;
        assert_eq!(face, lender, "the face the borrowed label is set in");
        assert!(
            (borrowed.height_mm - 20.0 * px_mm).abs() < 1e-6,
            "the borrowed glyphs are {:.4} mm of em, not 20 device units ({:.4} mm)",
            borrowed.height_mm,
            20.0 * px_mm
        );
        assert!(
            (borrowed.advance_mm - 38.0 * px_mm).abs() < 1e-6,
            "the borrowed run is {:.4} mm long, not two steps of 9 units and an em of 20 \
             ({:.4} mm)",
            borrowed.advance_mm,
            38.0 * px_mm
        );
        for (degree, face) in ink
            .spans
            .iter()
            .zip(&faces)
            .filter(|(span, _)| span.text == "℃")
        {
            assert_eq!(face, lender, "the face a borrowed ℃ is set in");
            assert!(
                (degree.height_mm - 20.0 * px_mm).abs() < 1e-6,
                "a borrowed ℃ is {:.4} mm of em, not 20 device units",
                degree.height_mm
            );
        }
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

    /// A page of plain 2.5 mm labels, one under the other, each `(text, the
    /// font its style names)`.
    fn lettered_sheet(labels: &[(&str, &str)]) -> PaperModel {
        let primitives = labels
            .iter()
            .zip((0u8..).map(f64::from))
            .map(|((value, font), row)| {
                primitive(
                    "decoded",
                    PaperShape::Text {
                        origin_mm: [40.0, 20.0 + 10.0 * row],
                        height_mm: Some(2.5),
                        rotation: 0.0,
                        value: (*value).to_string(),
                        font: Some((*font).to_string()),
                        anchor: TextAnchor::Left,
                        line: TextLine::Bottom,
                        height_basis: crate::model::TextHeightBasis::Cap,
                        width_permille: None,
                        paragraph: None,
                        spans: Vec::new(),
                    },
                )
            })
            .collect();
        PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some([200.0, 200.0]),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            primitives,
            missing_symbols: Vec::new(),
            missing_templates: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
            ledger: crate::ledger::LayoutLedger::default(),
        }
    }

    fn plot_fonts(prepared: &[crate::fonts::PreparedFont]) -> Vec<PlotFont<'_>> {
        prepared
            .iter()
            .map(|face| PlotFont {
                key: &face.key,
                bytes: &face.bytes,
                subset: face.subset,
            })
            .collect()
    }

    /// One label as the page letters it.
    #[derive(Debug, PartialEq)]
    struct Lettered {
        /// The label's text.
        value: String,
        /// The face its style names, as [`super::typeface_key`] reads it.
        named: &'static str,
        /// Each run's face and the characters that face draws.
        runs: Vec<(String, String)>,
    }

    fn lettered(value: &str, named: &'static str, runs: &[(&str, &str)]) -> Lettered {
        Lettered {
            value: value.to_string(),
            named,
            runs: runs
                .iter()
                .map(|(face, text)| ((*face).to_string(), (*text).to_string()))
                .collect(),
        }
    }

    /// One label the page letters: its text, the face its style names, and
    /// the operators [`super::show_text`] emits for it.
    type ShownLabel = (String, &'static str, Vec<super::Op>);

    /// Every label `model` letters under `faces`, in primitive order, as the
    /// operators the page shows it with.
    fn shown_labels(model: &PaperModel, faces: &[super::ReadyFace<'_>]) -> Vec<ShownLabel> {
        let page = model.resolved_page_mm().expect("the model states a page");
        let mut labels = Vec::new();
        for primitive in &model.primitives {
            if !super::is_plotted(primitive.confidence) {
                continue;
            }
            let PaperShape::Text {
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
            else {
                continue;
            };
            let named = super::typeface_key(font.as_deref(), value);
            let mut ops = Vec::new();
            let mut audit = super::PlotAudit::default();
            let shown = super::show_text(
                &mut ops,
                faces,
                ShownText {
                    typeface: named,
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
                &mut audit,
            )
            .unwrap_or_else(|error| panic!("{value:?}: {error}"));
            if shown.wrote_run {
                labels.push((value.clone(), named, ops));
            }
        }
        labels
    }

    /// The faces `model`'s labels are lettered in under `fonts`, for every
    /// label the page letters, in primitive order.
    ///
    /// Read off the `Tf` and `Tj` operators [`super::show_text`] emits, so it
    /// is what the page says and not a second reading of the rule. Neighbouring
    /// runs in one face are joined: a run is also cut where only its spacing
    /// changes, which is not a change of face.
    fn lettered_faces(model: &PaperModel, fonts: &[PlotFont<'_>]) -> Vec<Lettered> {
        use super::Object;

        let mut doc = Document::new_empty();
        let mut font_resources = Dict::new();
        let faces = super::embed_faces(&mut doc, &mut font_resources, fonts).expect("embed");
        let mut labels = Vec::new();
        for (value, named, ops) in shown_labels(model, &faces) {
            let mut runs: Vec<(String, String)> = Vec::new();
            let mut face: Option<&super::ReadyFace<'_>> = None;
            for op in &ops {
                match op.op.as_slice() {
                    b"Tf" => {
                        face = op
                            .operands
                            .first()
                            .and_then(Object::as_name)
                            .and_then(|name| {
                                faces.iter().find(|face| face.resource.as_bytes() == name)
                            });
                    }
                    b"Tj" | b"TJ" => {
                        let face = face.expect("a run names its face before it shows text");
                        let mut text = String::new();
                        for operand in &op.operands {
                            let shown: &[Object] = match operand {
                                Object::Array(items) => items,
                                other => std::slice::from_ref(other),
                            };
                            for item in shown {
                                if let Object::String(string) = item {
                                    text.push_str(&face.metrics.decode(&string.bytes));
                                }
                            }
                        }
                        match runs.last_mut() {
                            Some((key, run)) if key == face.key => run.push_str(&text),
                            _ => runs.push((face.key.to_string(), text)),
                        }
                    }
                    _ => {}
                }
            }
            labels.push(Lettered { value, named, runs });
        }
        labels
    }

    /// One string a label shows.
    #[derive(Debug)]
    struct ShownRun {
        /// The face it is set in.
        face: String,
        /// The `Tf` size in force, points.
        size_pt: f64,
        /// The `Tc` in force, points.
        tc_pt: f64,
        /// The characters it shows.
        text: String,
        /// How many glyphs that is.
        glyphs: usize,
        /// How far the pen moves over them before `Tc`: each glyph's own
        /// width at `size_pt`, points.
        natural_pt: f64,
    }

    /// How `model`'s labels are set under `fonts`, for every label the page
    /// letters, in primitive order: the label's text, the horizontal scale of
    /// its `Tm`, and every string it shows. Read off the operators
    /// [`super::show_text`] emits.
    fn shown_runs(model: &PaperModel, fonts: &[PlotFont<'_>]) -> Vec<(String, f64, Vec<ShownRun>)> {
        use super::Object;

        let real = |operand: Option<&Object>| match operand {
            Some(Object::Real(value)) => *value,
            other => panic!("expected a number, found {other:?}"),
        };
        let mut doc = Document::new_empty();
        let mut font_resources = Dict::new();
        let faces = super::embed_faces(&mut doc, &mut font_resources, fonts).expect("embed");
        shown_labels(model, &faces)
            .into_iter()
            .map(|(value, _, ops)| {
                let mut scale = f64::NAN;
                let mut tc_pt = 0.0;
                let mut set: Option<(&super::ReadyFace<'_>, f64)> = None;
                let mut runs = Vec::new();
                for op in &ops {
                    match op.op.as_slice() {
                        b"Tm" => {
                            scale = real(op.operands.first()).hypot(real(op.operands.get(1)));
                        }
                        b"Tc" => tc_pt = real(op.operands.first()),
                        b"Tf" => {
                            let face = op
                                .operands
                                .first()
                                .and_then(Object::as_name)
                                .and_then(|name| {
                                    faces.iter().find(|face| face.resource.as_bytes() == name)
                                })
                                .expect("a run selects an embedded face");
                            set = Some((face, real(op.operands.get(1))));
                        }
                        b"Tj" | b"TJ" => {
                            let (face, size_pt) =
                                set.expect("a run names its face before it shows text");
                            let mut bytes = Vec::new();
                            for operand in &op.operands {
                                let shown: &[Object] = match operand {
                                    Object::Array(items) => items,
                                    other => std::slice::from_ref(other),
                                };
                                for item in shown {
                                    if let Object::String(string) = item {
                                        bytes.extend_from_slice(&string.bytes);
                                    }
                                }
                            }
                            let codes = face.metrics.codes(&bytes);
                            runs.push(ShownRun {
                                face: face.key.to_string(),
                                size_pt,
                                tc_pt,
                                text: face.metrics.decode(&bytes),
                                glyphs: codes.len(),
                                natural_pt: codes
                                    .iter()
                                    .map(|(code, _)| face.metrics.width(*code) * size_pt)
                                    .sum(),
                            });
                        }
                        _ => {}
                    }
                }
                (value, scale, runs)
            })
            .collect()
    }

    /// A style height, in millimetres, at which a face with `heights` is
    /// lettered on a cell of `cell` device units: the middle of the heights
    /// that are. The stand-in for Arial Narrow in these tests is Inter, whose
    /// tables are not Arial Narrow's, so the 2.5 mm of the reference's labels
    /// is not the height that puts it on their 24-unit cell.
    fn height_on_cell(heights: super::FaceHeights, cell: f64) -> f64 {
        let on_cell: Vec<f64> = (1500..4500)
            .map(|thousandths| f64::from(thousandths) / 1000.0)
            .filter(|height| {
                super::whole_pixel_cell(heights, crate::model::TextHeightBasis::Cap, *height)
                    == Some(cell)
            })
            .collect();
        *on_cell
            .get(on_cell.len() / 2)
            .expect("some style height letters the face on that cell")
    }

    /// Set every label of `sheet` at `height_mm`.
    fn at_height(mut sheet: PaperModel, height_mm: f64) -> PaperModel {
        for primitive in &mut sheet.primitives {
            if let PaperShape::Text {
                height_mm: stated, ..
            } = &mut primitive.shape
            {
                *stated = Some(height_mm);
            }
        }
        sheet
    }

    /// The A2 acceptance sheet's two `动火点` labels name Arial Narrow, which
    /// draws no Han, so the whole label is borrowed, as the `℃` of `120℃` is.
    /// Taking the lender to be the first program in the list that holds the
    /// characters would make it follow the batch: one face for the sheet
    /// prepared alone, another beside a sheet whose own 宋体 or 仿宋 label
    /// holds the same characters. Both borrow from NSimSun, the first face of
    /// the order, so the sheet letters the same either way.
    #[test]
    fn a_borrowed_label_is_lettered_in_the_same_face_alone_and_in_a_batch() {
        use crate::fonts::test_faces::{ascii_only, drawing, scratch_dir};

        let narrow = ascii_only();
        let song = drawing("动火点℃");
        let dirs = scratch_dir(
            "write-borrow-alone-and-batch",
            &[
                ("ARIALN.TTF", &narrow),
                ("nsimsun.ttf", &song),
                ("simfang.ttf", &song),
                ("simsun.ttf", &song),
            ],
        );
        let borrowing = lettered_sheet(&[("动火点", "Arial Narrow"), ("120℃", "Arial Narrow")]);
        let naming = lettered_sheet(&[("动火点", "宋体"), ("点", "仿宋")]);
        let alone = crate::fonts::prepare_fonts(&[&borrowing], &dirs).expect("alone");
        let batch = crate::fonts::prepare_fonts(&[&borrowing, &naming], &dirs).expect("in a batch");
        let expected = vec![
            lettered("动火点", "Arial Narrow", &[("NSimSun", "动火点")]),
            lettered(
                "120℃",
                "Arial Narrow",
                &[("Arial Narrow", "120"), ("NSimSun", "℃")],
            ),
        ];
        assert_eq!(
            lettered_faces(&borrowing, &plot_fonts(&alone)),
            expected,
            "the sheet prepared alone"
        );
        assert_eq!(
            lettered_faces(&borrowing, &plot_fonts(&batch)),
            expected,
            "the sheet prepared with one whose own labels hold the same characters"
        );
    }

    /// The writer asks the faces of the borrow order in that order, and not
    /// the embedded faces in the order they were handed over. The two part
    /// ways where a batch has put the borrowed characters into the program of
    /// a face that stands ahead of the lender in the list, which the batch
    /// above does not: its FangSong holds `点` alone and its SimSun no `℃`.
    ///
    /// Here one batch letters all of `动火点℃` in a 仿宋 label and in a 宋体
    /// one, so FangSong and SimSun — both ahead of NSimSun in the list, which
    /// is there only to lend — could each draw the whole of `动火点` and the
    /// `℃`; another names only 宋体. Either way both borrow from NSimSun, as
    /// the sheet does prepared alone.
    #[test]
    fn a_borrowed_label_keeps_its_lender_when_other_faces_hold_its_characters() {
        use crate::fonts::test_faces::{ascii_only, drawing, scratch_dir};

        let narrow = ascii_only();
        let song = drawing("动火点℃");
        let dirs = scratch_dir(
            "write-borrow-other-faces-hold",
            &[
                ("ARIALN.TTF", &narrow),
                ("nsimsun.ttf", &song),
                ("simfang.ttf", &song),
                ("simsun.ttf", &song),
            ],
        );
        let borrowing = lettered_sheet(&[("动火点", "Arial Narrow"), ("120℃", "Arial Narrow")]);
        let expected = vec![
            lettered("动火点", "Arial Narrow", &[("NSimSun", "动火点")]),
            lettered(
                "120℃",
                "Arial Narrow",
                &[("Arial Narrow", "120"), ("NSimSun", "℃")],
            ),
        ];
        let alone = crate::fonts::prepare_fonts(&[&borrowing], &dirs).expect("alone");
        assert_eq!(
            lettered_faces(&borrowing, &plot_fonts(&alone)),
            expected,
            "the sheet prepared alone"
        );
        for (naming, batch) in [
            (
                lettered_sheet(&[("动火点℃", "仿宋"), ("动火点℃", "宋体")]),
                "a sheet that letters the same characters in 仿宋 and in 宋体",
            ),
            (
                lettered_sheet(&[("动火点℃", "宋体")]),
                "a sheet that letters the same characters in 宋体 alone",
            ),
        ] {
            let together =
                crate::fonts::prepare_fonts(&[&borrowing, &naming], &dirs).expect("in a batch");
            assert_eq!(
                lettered_faces(&borrowing, &plot_fonts(&together)),
                expected,
                "the sheet prepared with {batch}"
            );
        }
    }

    /// A machine may have no NSimSun file. The borrowed characters then go to
    /// SimSun, the next face of the order and the same design — the label
    /// with a Han character and the one without alike — and not to FangSong,
    /// though a 仿宋 label of the batch puts FangSong ahead of SimSun in the
    /// list with every one of the characters in its program.
    #[test]
    fn without_nsimsun_a_borrowed_label_is_lettered_in_simsun() {
        use crate::fonts::test_faces::{ascii_only, drawing, scratch_dir};

        let narrow = ascii_only();
        let song = drawing("动火点℃");
        let dirs = scratch_dir(
            "write-borrow-without-nsimsun",
            &[
                ("ARIALN.TTF", &narrow),
                ("simfang.ttf", &song),
                ("simsun.ttf", &song),
            ],
        );
        let borrowing = lettered_sheet(&[("动火点", "Arial Narrow"), ("120℃", "Arial Narrow")]);
        let naming = lettered_sheet(&[("动火点℃", "仿宋")]);
        let expected = vec![
            lettered("动火点", "Arial Narrow", &[("SimSun", "动火点")]),
            lettered(
                "120℃",
                "Arial Narrow",
                &[("Arial Narrow", "120"), ("SimSun", "℃")],
            ),
        ];
        let alone = crate::fonts::prepare_fonts(&[&borrowing], &dirs).expect("alone");
        let batch = crate::fonts::prepare_fonts(&[&borrowing, &naming], &dirs).expect("in a batch");
        assert_eq!(
            lettered_faces(&borrowing, &plot_fonts(&alone)),
            expected,
            "the sheet prepared alone"
        );
        assert_eq!(
            lettered_faces(&borrowing, &plot_fonts(&batch)),
            expected,
            "the sheet prepared with one that letters the same characters in 仿宋"
        );
    }

    /// A blank is lettered like any other character: it inks nothing and
    /// still takes its advance, so a face has to carry it. `动　火　点` spaces
    /// its characters with the ideographic space U+3000, which Arial Narrow
    /// does not draw, so the string is borrowed whole, spaces and all; in a
    /// label the named face partly draws, the space alone is borrowed. Both
    /// borrow from SimSun on this machine, which has no NSimSun. The sheet
    /// prepared alone letters both the way it does beside a sheet whose own
    /// 宋体 label holds the same space.
    #[test]
    fn a_space_the_named_face_cannot_draw_is_borrowed_alone_and_in_a_batch() {
        use crate::fonts::test_faces::{ascii_only, drawing, scratch_dir};

        let narrow = ascii_only();
        let song = drawing("动火点审定\u{3000}");
        let dirs = scratch_dir(
            "write-borrowed-space",
            &[
                ("ARIALN.TTF", &narrow),
                ("simfang.ttf", &song),
                ("simsun.ttf", &song),
            ],
        );
        let borrowing = lettered_sheet(&[
            ("动\u{3000}火\u{3000}点", "Arial Narrow"),
            ("A\u{3000}B", "Arial Narrow"),
        ]);
        let naming = lettered_sheet(&[("审\u{3000}定", "宋体")]);
        let expected = vec![
            lettered(
                "动\u{3000}火\u{3000}点",
                "Arial Narrow",
                &[("SimSun", "动\u{3000}火\u{3000}点")],
            ),
            lettered(
                "A\u{3000}B",
                "Arial Narrow",
                &[
                    ("Arial Narrow", "A"),
                    ("SimSun", "\u{3000}"),
                    ("Arial Narrow", "B"),
                ],
            ),
        ];
        let alone = crate::fonts::prepare_fonts(&[&borrowing], &dirs).expect("alone");
        let batch = crate::fonts::prepare_fonts(&[&borrowing, &naming], &dirs).expect("in a batch");
        assert_eq!(
            lettered_faces(&borrowing, &plot_fonts(&alone)),
            expected,
            "the sheet prepared alone"
        );
        assert_eq!(
            lettered_faces(&borrowing, &plot_fonts(&batch)),
            expected,
            "the sheet prepared with one whose own 宋体 label holds the space"
        );
    }

    /// A character none of the order's Chinese faces draws is lettered in the
    /// first of the faces after them that draws it — Arial here, ahead of
    /// SimSun-ExtB — whether the string is borrowed whole or one character
    /// of it is. The sheet letters it so prepared alone and beside a sheet
    /// that names SimSun-ExtB for the same character, where the embedded
    /// faces in the order they were handed over would give SimSun-ExtB.
    #[test]
    fn a_character_neither_lender_draws_is_lettered_in_the_same_face_alone_and_in_a_batch() {
        use crate::fonts::test_faces::{ascii_only, drawing, scratch_dir};

        let latin = ascii_only();
        let han = drawing("甲");
        let dirs = scratch_dir(
            "write-last-resort",
            &[
                ("ARIALN.TTF", &latin),
                ("simsun.ttf", &latin),
                ("simfang.ttf", &latin),
                ("arial.ttf", &han),
                ("simsunb.ttf", &han),
            ],
        );
        let borrowing = lettered_sheet(&[("甲", "Arial Narrow"), ("A甲", "Arial Narrow")]);
        let naming = lettered_sheet(&[("甲", "SimSun-ExtB")]);
        let expected = vec![
            lettered("甲", "Arial Narrow", &[("Arial", "甲")]),
            lettered(
                "A甲",
                "Arial Narrow",
                &[("Arial Narrow", "A"), ("Arial", "甲")],
            ),
        ];
        let alone = crate::fonts::prepare_fonts(&[&borrowing], &dirs).expect("alone");
        let batch = crate::fonts::prepare_fonts(&[&borrowing, &naming], &dirs).expect("in a batch");
        assert_eq!(
            lettered_faces(&borrowing, &plot_fonts(&alone)),
            expected,
            "the sheet prepared alone"
        );
        assert_eq!(
            lettered_faces(&borrowing, &plot_fonts(&batch)),
            expected,
            "the sheet prepared with one that names SimSun-ExtB for the same character"
        );
    }

    /// A face the caller embeds under a name of its own — `pid_plot` does
    /// with `PID_PLOT_CJK_FONT`, under the empty name — is asked after every
    /// face a label borrows from, wherever it stands in the list it was
    /// handed over in, and draws what none of them can.
    #[test]
    fn a_callers_own_face_is_asked_after_the_faces_a_label_borrows_from() {
        use crate::fonts::test_faces::{ascii_only, drawing};

        let latin = ascii_only();
        let han = drawing("甲");
        let own = drawing("甲乙");
        let sheet = lettered_sheet(&[("甲", "Arial Narrow")]);
        let face = |key: &'static str, bytes| PlotFont {
            key,
            bytes,
            subset: false,
        };
        // The caller's own face is handed over first, so the order of the
        // list would pick it.
        assert_eq!(
            lettered_faces(
                &sheet,
                &[
                    face("", &own),
                    face("Arial Narrow", &latin),
                    face("Arial", &han)
                ]
            ),
            vec![lettered("甲", "Arial Narrow", &[("Arial", "甲")])],
            "Arial is one of the faces a label borrows from and the caller's own is not"
        );
        assert_eq!(
            lettered_faces(&sheet, &[face("", &own), face("Arial Narrow", &latin)]),
            vec![lettered("甲", "Arial Narrow", &[("", "甲")])],
            "no face a label borrows from draws the character, so the caller's own does"
        );
    }

    /// A label its named face draws none of goes whole to one lender, and
    /// where an earlier lender draws only part of it that is a later one:
    /// with no NSimSun on the machine `甲乙` asks SimSun first, this SimSun
    /// draws `甲` alone, and FangSong draws
    /// both. FangSong therefore has to be given `甲` as well as `乙`, or the
    /// label is cut in two — and it has to be given it whether or not another
    /// sheet in the batch happens to letter `甲` in FangSong.
    #[test]
    fn a_label_the_first_lender_draws_part_of_goes_whole_to_the_second() {
        use crate::fonts::test_faces::{ascii_only, drawing, scratch_dir};

        let narrow = ascii_only();
        let dirs = scratch_dir(
            "write-second-lender",
            &[
                ("ARIALN.TTF", &narrow),
                ("simsun.ttf", &drawing("甲")),
                ("simfang.ttf", &drawing("甲乙")),
            ],
        );
        let borrowing = lettered_sheet(&[("甲乙", "Arial Narrow")]);
        let naming = lettered_sheet(&[("甲", "宋体")]);
        let alone = crate::fonts::prepare_fonts(&[&borrowing], &dirs).expect("alone");
        let batch = crate::fonts::prepare_fonts(&[&borrowing, &naming], &dirs).expect("in a batch");
        let expected = vec![lettered("甲乙", "Arial Narrow", &[("FangSong", "甲乙")])];
        assert_eq!(
            lettered_faces(&borrowing, &plot_fonts(&alone)),
            expected,
            "the sheet prepared alone"
        );
        assert_eq!(
            lettered_faces(&borrowing, &plot_fonts(&batch)),
            expected,
            "the sheet prepared with one that letters 甲 in SimSun"
        );
    }

    /// A run may name a face this machine has no file for, and the label then
    /// letters that stretch in its own face. Taking the first program in the
    /// list that can draw it instead would follow the batch: the label's own
    /// face while the sheet is prepared alone, another sheet's Arial once a
    /// batch has put that ahead of it.
    #[test]
    fn a_run_whose_face_is_absent_letters_in_the_labels_own_face() {
        use crate::fonts::test_faces::{ascii_only, scratch_dir};

        let narrow = ascii_only();
        let dirs = scratch_dir(
            "write-absent-run-face",
            &[
                ("ARIALN.TTF", &narrow),
                ("arial.ttf", pdfcraft_fonts::INTER_REGULAR),
            ],
        );
        let mut spanned = lettered_sheet(&[("AB", "Arial Narrow")]);
        if let PaperShape::Text { spans, .. } = &mut spanned.primitives[0].shape {
            spans.push(PaperTextSpan {
                start: 0,
                len: 1,
                height_mm: Some(2.5),
                font: Some("宋体".into()),
            });
        }
        let naming = lettered_sheet(&[("CD", "Arial")]);
        let alone = crate::fonts::prepare_fonts(&[&spanned], &dirs).expect("alone");
        let batch = crate::fonts::prepare_fonts(&[&spanned, &naming], &dirs).expect("in a batch");
        let expected = vec![lettered("AB", "Arial Narrow", &[("Arial Narrow", "AB")])];
        assert_eq!(
            lettered_faces(&spanned, &plot_fonts(&alone)),
            expected,
            "the sheet prepared alone"
        );
        assert_eq!(
            lettered_faces(&spanned, &plot_fonts(&batch)),
            expected,
            "the sheet prepared with one that names Arial"
        );
    }

    /// The reference plots set the glyphs a label named in Arial Narrow on a
    /// 24-unit cell borrows at `Tf 9.6` — 20 device units, whatever size the
    /// label's own face is lettered at — under a text matrix with no
    /// horizontal scale. A label borrowed whole advances by Arial Narrow's
    /// `?` after each glyph but the last: nine units, less than the glyph is
    /// wide, so the glyphs overlap as the reference's `动火点` does. A lone
    /// borrowed glyph, the whole of a label or the end of one, has nothing to
    /// step to.
    #[test]
    fn a_borrowed_run_the_reference_shows_is_set_at_twenty_units() {
        use crate::fonts::test_faces::{ascii_only, drawing, scratch_dir};

        let latin = ascii_only();
        let han = drawing("动火点℃");
        let dirs = scratch_dir(
            "write-as-the-reference",
            &[("ARIALN.TTF", &latin), ("nsimsun.ttf", &han)],
        );
        let heights = super::FaceHeights::of(&latin);
        let cap = crate::model::TextHeightBasis::Cap;
        let height_mm = height_on_cell(heights, 24.0);
        let named_pt = super::plotted_size_pt(heights, cap, height_mm);
        assert!(
            (named_pt - 9.6).abs() > 1e-6,
            "the stand-in for Arial Narrow is itself lettered at Tf 9.6, so a borrowed glyph at \
             9.6 would show nothing"
        );
        let sheet = at_height(
            lettered_sheet(&[
                ("动火点", "Arial Narrow"),
                ("120℃", "Arial Narrow"),
                ("℃", "Arial Narrow"),
            ]),
            height_mm,
        );
        let prepared = crate::fonts::prepare_fonts(&[&sheet], &dirs).expect("fonts");
        let shown = shown_runs(&sheet, &plot_fonts(&prepared));
        let texts: Vec<&str> = shown.iter().map(|(value, ..)| value.as_str()).collect();
        assert_eq!(texts, ["动火点", "120℃", "℃"]);
        for (value, scale, _) in &shown {
            assert!(
                (scale - 1.0).abs() < 1e-12,
                "{value:?} is set under a horizontal scale of {scale}"
            );
        }

        // The label borrowed whole.
        let (_, _, whole) = &shown[0];
        let lettered: String = whole.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(lettered, "动火点");
        for run in whole {
            assert_eq!(run.face, "NSimSun", "{run:?}");
            assert!((run.size_pt - 9.6).abs() < 1e-9, "{run:?}");
        }
        let a8 = super::reference_scale("Arial Narrow", heights, cap, height_mm);
        let question_px = f64::from(super::ARIAL_NARROW_72PX[usize::from(b'?' - b' ')]) * a8;
        assert_eq!(
            question_px.round(),
            9.0,
            "the stand-in's `?` is {question_px:.3} units at this height, not the reference's 9"
        );
        let (stepped, last) = whole.split_at(whole.len() - 1);
        for run in stepped {
            #[allow(clippy::cast_precision_loss)]
            let step_pt = run.natural_pt / run.glyphs as f64 + run.tc_pt;
            assert!(
                (step_pt - 9.0 * 72.0 / super::PLOT_DPI).abs() < 1e-9,
                "each glyph of {:?} steps {:.4} units, not the 9 of the named face's `?`",
                run.text,
                step_pt * super::PLOT_DPI / 72.0
            );
            assert!(
                step_pt < run.natural_pt / 2.0,
                "the glyphs of a label borrowed whole overlap: {run:?}"
            );
        }
        assert_eq!(last[0].glyphs, 1, "{last:?}");
        assert!(last[0].tc_pt.abs() < 1e-12, "{last:?}");

        // The label whose last character alone is borrowed.
        let (_, _, tail) = &shown[1];
        let (borrowed, named) = tail.split_last().expect("runs");
        assert_eq!(
            (borrowed.face.as_str(), borrowed.text.as_str()),
            ("NSimSun", "℃")
        );
        assert!((borrowed.size_pt - 9.6).abs() < 1e-9, "{borrowed:?}");
        let digits: String = named.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(digits, "120");
        for run in named {
            assert_eq!(run.face, "Arial Narrow", "{run:?}");
            assert!(
                (run.size_pt - named_pt).abs() < 1e-9,
                "the label's own face keeps its size: {run:?}"
            );
        }

        // The label that is one borrowed character.
        let (_, _, lone) = &shown[2];
        assert_eq!(lone.len(), 1, "{lone:?}");
        assert_eq!(lone[0].face, "NSimSun");
        assert!((lone[0].size_pt - 9.6).abs() < 1e-9, "{lone:?}");
        assert!(lone[0].tc_pt.abs() < 1e-12, "{lone:?}");
    }

    /// The reference's lettering of a borrowed run is applied where a
    /// reference plot shows it and nowhere else. Each label here differs
    /// from the samples in one respect, and its borrowed characters are set
    /// like the rest of it: at the size its own face is lettered at.
    #[test]
    fn a_borrowed_run_the_samples_do_not_reach_keeps_the_size_of_its_label() {
        use crate::fonts::test_faces::{ascii_only, drawing, scratch_dir};

        let latin = ascii_only();
        let han = drawing("动火点℃总体施工图");
        let dirs = scratch_dir(
            "write-past-the-samples",
            &[
                ("ARIALN.TTF", &latin),
                ("arial.ttf", &latin),
                ("simsunb.ttf", &latin),
                ("nsimsun.ttf", &han),
            ],
        );
        let heights = super::FaceHeights::of(&latin);
        let cap = crate::model::TextHeightBasis::Cap;
        let on_the_cell = height_on_cell(heights, 24.0);
        let on_another = height_on_cell(heights, 25.0);
        let cases = [
            ("动火点", "Arial", "a label that names another face"),
            ("动火点", "Arial Narrow", "a label on another cell"),
            ("动火点", "Arial Narrow", "a rotated label"),
            ("动火点", "Arial Narrow", "a border template's label"),
            (
                "动火点",
                "Arial Narrow",
                "a label that states a character width",
            ),
            (
                "总  体",
                "Arial Narrow",
                "a borrowed character with one of the named face after it",
            ),
            (
                "A℃℃",
                "Arial Narrow",
                "two borrowed characters after one of the named face",
            ),
            (
                "施工图",
                "SimSun-ExtB",
                "a label named in a face of its own that draws none of it",
            ),
        ];
        let labels: Vec<(&str, &str)> =
            cases.iter().map(|(text, font, _)| (*text, *font)).collect();
        let mut sheet = at_height(lettered_sheet(&labels), on_the_cell);
        if let PaperShape::Text { height_mm, .. } = &mut sheet.primitives[1].shape {
            *height_mm = Some(on_another);
        }
        if let PaperShape::Text { rotation, .. } = &mut sheet.primitives[2].shape {
            *rotation = std::f64::consts::FRAC_PI_2;
        }
        sheet.primitives[3].furniture = true;
        if let PaperShape::Text { width_permille, .. } = &mut sheet.primitives[4].shape {
            *width_permille = Some(800);
        }
        let prepared = crate::fonts::prepare_fonts(&[&sheet], &dirs).expect("fonts");
        let shown = shown_runs(&sheet, &plot_fonts(&prepared));
        assert_eq!(shown.len(), cases.len());
        let mut resized = Vec::new();
        for (index, ((value, _, runs), (_, _, what))) in shown.iter().zip(&cases).enumerate() {
            let height_mm = if index == 1 { on_another } else { on_the_cell };
            // Every stand-in face here is Inter, so every label's own face is
            // lettered at the size Inter is at the label's height.
            let own_pt = super::plotted_size_pt(heights, cap, height_mm);
            assert!(
                runs.iter().any(|run| run.face == "NSimSun"),
                "{what} ({value:?}) borrows nothing: {runs:?}"
            );
            for run in runs {
                if (run.size_pt - own_pt).abs() > 1e-9 {
                    resized.push(format!(
                        "  {what} ({value:?}): {:?} in {} at Tf {:.3}, its label at {own_pt:.3}",
                        run.text, run.face, run.size_pt
                    ));
                }
            }
        }
        assert!(
            resized.is_empty(),
            "a run was resized on a label no reference plot shows:\n{}",
            resized.join("\n")
        );
    }

    /// A plot reads back the face of every span, by the name the page gives
    /// the font: the program's own family name, which the writer embeds it
    /// under, without the tag a subset's name carries. Both labels here are
    /// set in Inter — one in the whole program, one in a subset embedded
    /// under another key — and both come back as `Inter`, one name per span.
    #[test]
    fn a_plot_reads_back_the_face_of_every_span() {
        let subset = crate::fonts::test_faces::ascii_only();
        let sheet = lettered_sheet(&[("AB", "Arial"), ("CD", "Arial Narrow")]);
        let fonts = [
            PlotFont {
                key: "Arial",
                bytes: pdfcraft_fonts::INTER_REGULAR,
                subset: false,
            },
            PlotFont {
                key: "Arial Narrow",
                bytes: &subset,
                subset: true,
            },
        ];
        let pdf = write_plot(&sheet, &fonts).expect("write");
        let (ink, faces) = crate::read::read_plot_with_faces(&pdf).expect("read");
        assert_eq!(ink, read_plot(&pdf).expect("read"), "the ink is the same");
        let read: Vec<(&str, &str)> = ink
            .spans
            .iter()
            .map(|span| span.text.as_str())
            .zip(faces.iter().map(String::as_str))
            .collect();
        assert_eq!(read, [("AB", "Inter"), ("CD", "Inter")]);
    }

    /// Every acceptance sheet letters each label in the same faces whether its
    /// faces are prepared for it alone — what `pid_plot` does with one drawing
    /// — or for all the sheets of the corpus together, which is what the
    /// acceptance gate does with the eleven. The A2 sheet is the one a lender
    /// that followed the batch would show on: its two `动火点` labels name
    /// Arial Narrow and borrow all three characters. Both preparations letter
    /// them in the first face of the borrow order the machine has a file of —
    /// NSimSun, the face the reference plots set them in, or SimSun without
    /// it.
    ///
    /// A corpus with no drawing at all, or a machine without the three
    /// faces, says so and skips. A corpus that holds drawings and not the A2
    /// sheet fails: its sheets would otherwise pass with the pin unread.
    #[test]
    fn an_acceptance_sheet_letters_alone_as_it_does_among_the_eleven() {
        const A2_SHEET: &str = "集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图";
        let corpus = std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
            || std::path::PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
            std::path::PathBuf::from,
        );
        let mut drawings: Vec<std::path::PathBuf> = std::fs::read_dir(&corpus)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "pid"))
            .collect();
        drawings.sort();
        if drawings.is_empty() {
            eprintln!(
                "skipping the acceptance sheets' faces; no .pid is under {}",
                corpus.display()
            );
            return;
        }
        // A directory of drawings without the A2 sheet is not the corpus this
        // test is about, and the sheets it does hold would pass with the pin
        // on `动火点` never read.
        assert!(
            drawings
                .iter()
                .any(|path| path.file_stem().is_some_and(|stem| stem == A2_SHEET)),
            "{} holds {} drawing(s) and not {A2_SHEET}.pid, whose two `动火点` this test pins",
            corpus.display(),
            drawings.len()
        );
        // The acceptance corpus ships no faces; the gate reads the share's.
        let mut dirs = crate::fonts::font_dirs(Some(&corpus));
        dirs.push(std::path::PathBuf::from(
            "D:/work/cad/PID自研软件验证/fonts",
        ));
        let ready = |name: &str| dirs.iter().any(|dir| dir.join(name).is_file());
        if !ready("simsun.ttf") || !ready("simfang.ttf") || !ready("ARIALN.TTF") {
            eprintln!(
                "skipping the acceptance sheets' faces; FangSong, SimSun or Arial Narrow is absent"
            );
            return;
        }
        let sheets: Vec<(String, PaperModel)> = drawings
            .iter()
            .map(|path| {
                let name = path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let model = crate::model::lay_out_drawing(path, &corpus)
                    .unwrap_or_else(|error| panic!("{name}: layout: {error}"));
                (name, model)
            })
            .collect();
        let models: Vec<&PaperModel> = sheets.iter().map(|(_, model)| model).collect();
        let together = crate::fonts::prepare_fonts(&models, &dirs).expect("fonts for the corpus");
        let lender = if ready("nsimsun.ttf") {
            "NSimSun"
        } else {
            "SimSun"
        };
        let mut borrowed_han = 0usize;
        for (name, model) in &sheets {
            let alone = crate::fonts::prepare_fonts(&[model], &dirs)
                .unwrap_or_else(|error| panic!("{name}: fonts: {error}"));
            let lettered_alone = lettered_faces(model, &plot_fonts(&alone));
            let lettered_together = lettered_faces(model, &plot_fonts(&together));
            assert_eq!(
                lettered_alone.len(),
                lettered_together.len(),
                "{name}: the page letters a different number of labels under the two preparations"
            );
            let differing: Vec<String> = lettered_alone
                .iter()
                .zip(&lettered_together)
                .filter(|(alone, together)| alone != together)
                .map(|(alone, together)| {
                    format!(
                        "  {:?} in {}: alone {:?}, with the corpus {:?}",
                        alone.value, alone.named, alone.runs, together.runs
                    )
                })
                .collect();
            assert!(
                differing.is_empty(),
                "{name}: {} label(s) are lettered in other faces when the sheet is prepared \
                 alone than when the {} sheets are prepared together:\n{}",
                differing.len(),
                sheets.len(),
                differing.join("\n")
            );
            if name == A2_SHEET {
                let expected = lettered("动火点", "Arial Narrow", &[(lender, "动火点")]);
                for label in &lettered_together {
                    if label.value.trim() == "动火点" && label.named == "Arial Narrow" {
                        assert_eq!(
                            label.runs, expected.runs,
                            "{name}: a `动火点` its style's face cannot draw is borrowed whole \
                             from {lender}"
                        );
                        borrowed_han += 1;
                    }
                }
            }
        }
        assert_eq!(
            borrowed_han, 2,
            "the A2 sheet letters two `动火点` in a borrowed {lender}"
        );
        eprintln!(
            "{} acceptance sheets letter alone as they do together",
            sheets.len()
        );
    }

    /// A one-primitive page, for the verdict-versus-write and condensation
    /// tests below.
    fn one_page(primitive: PaperPrimitive, page_mm: [f64; 2]) -> PaperModel {
        PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some(page_mm),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            primitives: vec![primitive],
            missing_symbols: Vec::new(),
            missing_templates: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
            ledger: crate::ledger::LayoutLedger::default(),
        }
    }

    fn inter_font() -> [PlotFont<'static>; 1] {
        [PlotFont {
            key: "Arial",
            bytes: pdfcraft_fonts::INTER_REGULAR,
            subset: false,
        }]
    }

    /// Feature: pdf-export-ledger, Property 5: 判定与真实写出一致
    ///
    /// The set `write_ledger` calls on-page has to equal the set
    /// `write_plot_audited` actually wrote a path or a run for. The writer
    /// records that observation itself, so the property is that the
    /// observation never disagrees with the verdict — a non-empty
    /// `mismatches` is exactly the drift requirement 5.6 forbids.
    #[test]
    fn write_verdict_matches_the_real_write() {
        use proptest::prelude::*;

        fn coordinate() -> impl Strategy<Value = f64> {
            prop_oneof![
                Just(f64::NAN),
                Just(f64::INFINITY),
                Just(0.0),
                Just(-1.0),
                Just(1e-4),
                Just(0.9e-4),
                Just(50.0),
                Just(99.0),
                (0.0f64..100.0),
            ]
        }

        fn point() -> impl Strategy<Value = [f64; 2]> {
            (coordinate(), coordinate()).prop_map(|(x, y)| [x, y])
        }

        fn shape() -> impl Strategy<Value = PaperShape> {
            prop_oneof![
                (point(), point())
                    .prop_map(|(start_mm, end_mm)| PaperShape::Line { start_mm, end_mm }),
                (proptest::collection::vec(point(), 0..4), any::<bool>())
                    .prop_map(|(points_mm, closed)| PaperShape::Polyline { points_mm, closed }),
                (proptest::collection::vec(point(), 0..4), any::<bool>()).prop_map(
                    |(chain_mm, _)| PaperShape::Fill {
                        chain_mm,
                        rgb: None
                    }
                ),
                (point(), "[A-Za-z0-9 ]{0,5}").prop_map(|(origin_mm, value)| {
                    PaperShape::Text {
                        origin_mm,
                        height_mm: Some(2.5),
                        rotation: 0.0,
                        value,
                        font: Some("Arial".to_string()),
                        anchor: crate::model::TextAnchor::Left,
                        line: crate::model::TextLine::Middle,
                        height_basis: crate::model::TextHeightBasis::Cap,
                        width_permille: None,
                        paragraph: None,
                        spans: Vec::new(),
                    }
                }),
            ]
        }

        proptest!(ProptestConfig::with_cases(128), |(
            shape in shape(),
            plotted in any::<bool>(),
            furniture in any::<bool>(),
            width in prop_oneof![Just(0.35), Just(0.0), Just(f64::NAN)],
        )| {
            let mut primitive = primitive(if plotted { "decoded" } else { "probe_only" }, shape);
            primitive.stroke = Some(super::PaperStroke {
                rgb: [0, 0, 0],
                width_mm: width,
                dash_mm: Vec::new(),
                dash_fit: None,
            });
            primitive.furniture = furniture;
            let model = one_page(primitive, [100.0, 100.0]);
            let fonts = inter_font();
            let written = write_plot_audited(&model, &fonts).expect("the page writes");
            prop_assert!(
                written.ledger.mismatches.is_empty(),
                "verdict and writer disagreed: {:?}",
                written.ledger.mismatches
            );
            prop_assert_eq!(written.ledger.fates.len(), model.primitives.len());
            prop_assert!(written.ledger.fonts_measured);
        });
    }

    /// Feature: pdf-export-ledger, A.6: the ledger's two text marks equal the
    /// audit's two counts.
    ///
    /// A run wider than the sheet is condensed; one that is still too wide at
    /// half its width is clipped at the floor. Both are facts about the page,
    /// and the ledger has to repeat them rather than reach its own answer.
    #[test]
    fn text_marks_equal_the_audit_counts() {
        let page = [100.0, 50.0];
        let label = |value: &str, x: f64| {
            let mut primitive = primitive(
                "decoded",
                PaperShape::Text {
                    origin_mm: [x, 25.0],
                    height_mm: Some(2.5),
                    rotation: 0.0,
                    value: value.to_string(),
                    font: Some("Arial".to_string()),
                    anchor: TextAnchor::Left,
                    line: TextLine::Bottom,
                    height_basis: crate::model::TextHeightBasis::Cap,
                    width_permille: None,
                    paragraph: None,
                    spans: Vec::new(),
                },
            );
            primitive.stroke = None;
            primitive
        };
        let fonts = inter_font();

        // A run a little wider than the room it has: condensed, not clipped.
        let condensed = one_page(label(&"M".repeat(40), 10.0), page);
        let written = write_plot_audited(&condensed, &fonts).expect("write");
        assert_eq!(written.audit.runs_condensed, 1, "the run is condensed");
        let marks = written.ledger.fates[0]
            .text
            .expect("a written run has marks");
        assert!(marks.condensed, "{marks:?}");
        assert_eq!(
            marks.clipped_at_floor,
            written.audit.runs_clipped_at_the_condensation_floor > 0
        );

        // A run that overflows even at half width: clipped at the floor.
        let clipped = one_page(label(&"M".repeat(400), 1.0), page);
        let written = write_plot_audited(&clipped, &fonts).expect("write");
        assert!(
            written.audit.runs_clipped_at_the_condensation_floor > 0,
            "the run is clipped at the floor"
        );
        let marks = written.ledger.fates[0]
            .text
            .expect("a written run has marks");
        assert!(marks.condensed, "{marks:?}");
        assert!(marks.clipped_at_floor, "{marks:?}");
    }

    /// The face-free entry reaches the same verdicts as the writer, and says
    /// so by leaving `fonts_measured` false.
    #[test]
    fn write_ledger_judges_without_a_face() {
        let mut drawn = primitive(
            "decoded",
            PaperShape::Line {
                start_mm: [10.0, 10.0],
                end_mm: [90.0, 10.0],
            },
        );
        drawn.stroke = Some(super::PaperStroke {
            rgb: [0, 0, 0],
            width_mm: 0.35,
            dash_mm: Vec::new(),
            dash_fit: None,
        });
        let mut probe = drawn.clone();
        probe.confidence = "probe_only";
        let mut model = one_page(drawn, [100.0, 100.0]);
        model.primitives.push(probe);

        let ledger = super::write_ledger(&model).expect("no page stated? it is");
        assert!(!ledger.fonts_measured);
        assert_eq!(ledger.fates.len(), 2);
        assert!(ledger.fates[0].is_on_page());
        assert_eq!(
            ledger.fates[1].reason(),
            Some(crate::ledger::DropReason::ProbeOnly)
        );
        assert!(ledger.mismatches.is_empty());

        // A model that states no page and holds no ink has nothing to judge
        // against. (One that holds ink still resolves a page from the ink's
        // own extent — that is `resolved_page_mm`'s job, not this entry's.)
        let page_less = PaperModel {
            page_mm: None,
            primitives: Vec::new(),
            ..one_page(
                primitive(
                    "decoded",
                    PaperShape::Line {
                        start_mm: [0.0, 0.0],
                        end_mm: [0.0, 0.0],
                    },
                ),
                [100.0, 100.0],
            )
        };
        assert!(matches!(
            super::write_ledger(&page_less),
            Err(super::PlotWriteError::NoPage)
        ));
    }
}
