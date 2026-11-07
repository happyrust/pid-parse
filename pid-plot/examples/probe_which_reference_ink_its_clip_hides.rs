//! Which reference ink does its own clip hide?
//!
//! The plotter wraps each group of strokes in `q … re W n … Q` and ends every
//! polyline one device unit (0.48 pt, 0.169 mm at 150 dpi) past its last
//! vertex. A polyline that ends on its clip's edge puts that unit wholly
//! outside the clip, where nothing prints. This walks a reference page with the
//! interpreter `read.rs` uses, keeps the clip stack the way the reader does,
//! and prints every stroked or filled segment the clip cuts: how much of the
//! page's ink lies outside its clip, how much of that is whole device-unit
//! tails, and where each hidden piece sits in paper millimetres.
//!
//! ```text
//! cargo run -p pid-plot --example probe_which_reference_ink_its_clip_hides
//! cargo run -p pid-plot --example probe_which_reference_ink_its_clip_hides -- "<plot.pdf>"...
//! ```
//!
//! With no argument it reads every `.pdf` in `PID_ACCEPTANCE_CORPUS` (default
//! `D:\work\cad\pid-test-data\验收数据`). A stroke that runs the page is a sheet
//! frame edge: it is listed, but the reader drops it whether clipped or not.

use std::path::PathBuf;

use hayro::hayro_interpret::font::Glyph;
use hayro::hayro_interpret::hayro_syntax::Pdf;
use hayro::hayro_interpret::{
    BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache,
    InterpreterSettings, Paint, PathDrawMode, SoftMask, TransformExt, interpret_page,
};
use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape, flatten};
use pid_plot::read::{CHORD_MM, sheet_edge_mm, view_pt_to_paper_mm};

const MM_PER_PT: f64 = 25.4 / 72.0;
/// One device unit of the 150 dpi plotter, points.
const DEVICE_UNIT_PT: f64 = 0.48;

struct Hidden {
    start_mm: [f64; 2],
    end_mm: [f64; 2],
    length_mm: f64,
    hidden_mm: f64,
    width_mm: f64,
}

struct ClipWalk {
    page_pt: [f64; 2],
    clips: Vec<Rect>,
    /// Every stroked and filled segment, sheet edges and dashes' gaps
    /// included, so it is longer than the reader's ink.
    path_mm: f64,
    hidden: Vec<Hidden>,
}

impl ClipWalk {
    fn segment(&mut self, start: Point, end: Point, width_pt: f64) {
        let length_pt = (end - start).hypot();
        if length_pt <= 0.0 || !length_pt.is_finite() {
            return;
        }
        self.path_mm += length_pt * MM_PER_PT;
        let Some(clip) = self.clips.last() else {
            return;
        };
        let hidden_pt = length_pt - inside_length(*clip, start, end);
        if hidden_pt > 1e-6 {
            self.hidden.push(Hidden {
                start_mm: view_pt_to_paper_mm(start.x, start.y, self.page_pt[1]),
                end_mm: view_pt_to_paper_mm(end.x, end.y, self.page_pt[1]),
                length_mm: length_pt * MM_PER_PT,
                hidden_mm: hidden_pt * MM_PER_PT,
                width_mm: width_pt * MM_PER_PT,
            });
        }
    }
}

/// How much of `start` → `end` lies inside `clip`, points.
fn inside_length(clip: Rect, start: Point, end: Point) -> f64 {
    let delta = end - start;
    let mut enter = 0.0_f64;
    let mut leave = 1.0_f64;
    for (toward, room) in [
        (-delta.x, start.x - clip.x0),
        (delta.x, clip.x1 - start.x),
        (-delta.y, start.y - clip.y0),
        (delta.y, clip.y1 - start.y),
    ] {
        if toward == 0.0 {
            if room < 0.0 {
                return 0.0;
            }
            continue;
        }
        let at = room / toward;
        if toward < 0.0 {
            enter = enter.max(at);
        } else {
            leave = leave.min(at);
        }
    }
    ((leave - enter).max(0.0)) * delta.hypot()
}

impl<'a> Device<'a> for ClipWalk {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'a>>) {}
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn pop_transparency_group(&mut self) {}
    fn draw_image(&mut self, _: Image<'a, '_>, _: Affine) {}
    fn draw_glyph(
        &mut self,
        _: &Glyph<'a>,
        _: Affine,
        _: Affine,
        _: &Paint<'a>,
        _: &GlyphDrawMode,
    ) {
    }

    fn push_clip_path(&mut self, clip_path: &ClipPath) {
        let bounds = clip_path.path.bounding_box();
        let clip = self
            .clips
            .last()
            .map_or(bounds, |outer| outer.intersect(bounds));
        self.clips.push(clip);
    }

    fn pop_clip_path(&mut self) {
        self.clips.pop();
    }

    fn draw_path(
        &mut self,
        path: &BezPath,
        transform: Affine,
        _: &Paint<'a>,
        draw_mode: &PathDrawMode,
    ) {
        let width_pt = match draw_mode {
            PathDrawMode::Stroke(stroke) => {
                let origin = transform * Point::ORIGIN;
                let unit = transform * Point::new(1.0, 0.0);
                f64::from(stroke.line_width) * (unit - origin).hypot()
            }
            PathDrawMode::Fill(_) => 0.0,
        };
        let mut start: Option<Point> = None;
        let mut current: Option<Point> = None;
        flatten(
            transform * path.clone(),
            CHORD_MM / MM_PER_PT,
            |element| match element {
                PathEl::MoveTo(point) => {
                    start = Some(point);
                    current = Some(point);
                }
                PathEl::LineTo(point) => {
                    if let Some(from) = current {
                        self.segment(from, point, width_pt);
                    }
                    current = Some(point);
                }
                PathEl::ClosePath => {
                    if let (Some(from), Some(origin)) = (current, start) {
                        self.segment(from, origin, width_pt);
                    }
                    current = start;
                }
                PathEl::QuadTo(..) | PathEl::CurveTo(..) => {}
            },
        );
    }
}

fn corpus_pdfs() -> Vec<PathBuf> {
    let corpus = std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    );
    let mut pdfs: Vec<PathBuf> = std::fs::read_dir(&corpus)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "pdf"))
                .collect()
        })
        .unwrap_or_default();
    pdfs.sort();
    pdfs
}

fn main() {
    let named: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let pdfs = if named.is_empty() {
        corpus_pdfs()
    } else {
        named
    };
    for path in pdfs {
        let Ok(bytes) = std::fs::read(&path) else {
            eprintln!("skipping {}: unreadable", path.display());
            continue;
        };
        let Ok(pdf) = Pdf::new(bytes) else {
            eprintln!("skipping {}: not a PDF", path.display());
            continue;
        };
        let pages = pdf.pages();
        let Some(page) = pages.first() else {
            continue;
        };
        let (width_pt, height_pt) = page.render_dimensions();
        let (width_pt, height_pt) = (f64::from(width_pt), f64::from(height_pt));
        let cache = InterpreterCache::new();
        let settings = InterpreterSettings {
            render_annotations: false,
            ..Default::default()
        };
        let mut ctx = Context::new(
            page.initial_transform(true).to_kurbo(),
            Rect::new(0.0, 0.0, width_pt, height_pt),
            &cache,
            page.xref(),
            settings,
        );
        let mut walk = ClipWalk {
            page_pt: [width_pt, height_pt],
            clips: Vec::new(),
            path_mm: 0.0,
            hidden: Vec::new(),
        };
        interpret_page(page, &mut ctx, &mut walk);
        let page_mm = [width_pt * MM_PER_PT, height_pt * MM_PER_PT];
        let unit_mm = DEVICE_UNIT_PT * MM_PER_PT;
        let class = |piece: &Hidden| {
            if sheet_edge_mm(piece.start_mm, piece.end_mm, page_mm) {
                "sheet edge"
            } else if (piece.length_mm - unit_mm).abs() < 0.005
                && (piece.hidden_mm - piece.length_mm).abs() < 1e-6
            {
                "device-unit tail"
            } else {
                "other"
            }
        };
        let hidden_mm = walk
            .hidden
            .iter()
            .fold(0.0, |total, piece| total + piece.hidden_mm);
        let tails = walk
            .hidden
            .iter()
            .filter(|piece| class(piece) == "device-unit tail")
            .count();
        let name = path.file_stem().map_or_else(
            || path.display().to_string(),
            |stem| stem.to_string_lossy().into_owned(),
        );
        println!(
            "== {name}\n   paths {:.3} mm, hidden by their clip {hidden_mm:.3} mm in {} pieces ({tails} whole device-unit tails)",
            walk.path_mm,
            walk.hidden.len()
        );
        for piece in &walk.hidden {
            println!(
                "     {:<16} hidden {:.3} of {:.3} mm  ({:8.3},{:8.3}) → ({:8.3},{:8.3})  w {:.3} mm",
                class(piece),
                piece.hidden_mm,
                piece.length_mm,
                piece.start_mm[0],
                piece.start_mm[1],
                piece.end_mm[0],
                piece.end_mm[1],
                piece.width_mm
            );
        }
    }
}
