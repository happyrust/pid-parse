//! Which strokes does the plot reader leave out as sheet edges, and do the two
//! pages agree about them?
//!
//! `read::read_plot` drops every stroke that runs more than 90 % of the page's
//! width or height (`read::sheet_edge_mm`), on the reference plot and on ours
//! alike, because the print driver stores the sheet frame as a quad. So no
//! reading of the acceptance gate — masked or whole — says whether a page's
//! frame lines are there, where they are, how wide they print or whether they
//! print at all. This lists those strokes for each of the eleven `验收数据`
//! sheets, straight off both content streams, and pairs them: an edge of ours
//! with the reference's edge whose two ends lie within 1 mm of its own.
//!
//! ```text
//! cargo run -p pid-plot --example probe_which_strokes_the_reader_drops_as_sheet_edges -- [<sheet name part> ...]
//! ```
//!
//! Exit code 1 when a page has an edge with no partner on the other page, a
//! pair printed at different widths, or an edge painted in the paper's colour
//! on one page and inked on the other.
//!
//! The corpus is `PID_ACCEPTANCE_CORPUS`, default
//! `D:\work\cad\pid-test-data\验收数据`. Read-only.

#[path = "support/acceptance.rs"]
mod acceptance;

use std::process::ExitCode;

use hayro::hayro_interpret::font::Glyph;
use hayro::hayro_interpret::hayro_syntax::Pdf;
use hayro::hayro_interpret::{
    BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache,
    InterpreterSettings, Paint, PathDrawMode, SoftMask, TransformExt, interpret_page,
};
use kurbo::{Affine, BezPath, PathEl, Point, Rect, flatten};
use pid_plot::read::{CHORD_MM, sheet_edge_mm, view_pt_to_paper_mm};

use acceptance::{pages, short};

const MM_PER_PT: f64 = 25.4 / 72.0;
/// How far apart two pages' edges may end and still be the same edge. The A2
/// sheet is laid out 0.33 × 0.57 mm larger than the reference's page and the
/// fit moves it by (0.16, 0.00) mm, so its sheet frame's far corner lands
/// 0.50 mm from the reference's; one millimetre keeps that a pair.
const SAME_EDGE_MM: f64 = 1.0;

/// One stroke the reader's sheet-edge rule leaves out.
#[derive(Clone, Copy, Debug)]
struct Edge {
    start_mm: [f64; 2],
    end_mm: [f64; 2],
    width_mm: f64,
    /// Whether it is painted in the paper's own colour.
    paper: bool,
}

impl Edge {
    fn length(&self) -> f64 {
        (self.end_mm[0] - self.start_mm[0]).hypot(self.end_mm[1] - self.start_mm[1])
    }
    fn horizontal(&self) -> bool {
        (self.end_mm[1] - self.start_mm[1]).abs() < (self.end_mm[0] - self.start_mm[0]).abs()
    }
    /// Its two ends, lower coordinate first.
    fn ordered(&self) -> ([f64; 2], [f64; 2]) {
        let axis = usize::from(!self.horizontal());
        if self.start_mm[axis] <= self.end_mm[axis] {
            (self.start_mm, self.end_mm)
        } else {
            (self.end_mm, self.start_mm)
        }
    }
    /// Whether `other` is the same edge: the same way up, both ends within
    /// [`SAME_EDGE_MM`].
    fn same_as(&self, other: &Self) -> bool {
        if self.horizontal() != other.horizontal() {
            return false;
        }
        let (a0, a1) = self.ordered();
        let (b0, b1) = other.ordered();
        let near = |p: [f64; 2], q: [f64; 2]| (p[0] - q[0]).hypot(p[1] - q[1]) <= SAME_EDGE_MM;
        near(a0, b0) && near(a1, b1)
    }
}

/// Collects exactly the strokes `read::read_plot` refuses as sheet edges.
struct EdgeDevice {
    page_pt: [f64; 2],
    edges: Vec<Edge>,
}

impl<'a> Device<'a> for EdgeDevice {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'a>>) {}
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn pop_clip_path(&mut self) {}
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

    fn draw_rect(
        &mut self,
        rect: &Rect,
        transform: Affine,
        paint: &Paint<'a>,
        draw_mode: &PathDrawMode,
    ) {
        let mut path = BezPath::new();
        path.move_to((rect.x0, rect.y0));
        path.line_to((rect.x1, rect.y0));
        path.line_to((rect.x1, rect.y1));
        path.line_to((rect.x0, rect.y1));
        path.close_path();
        self.draw_path(&path, transform, paint, draw_mode);
    }

    fn draw_path(
        &mut self,
        path: &BezPath,
        transform: Affine,
        paint: &Paint<'a>,
        draw_mode: &PathDrawMode,
    ) {
        let PathDrawMode::Stroke(stroke) = draw_mode else {
            return;
        };
        let coefficients = transform.as_coeffs();
        let scale = (coefficients[0] * coefficients[3] - coefficients[1] * coefficients[2])
            .abs()
            .sqrt();
        let width_mm = f64::from(stroke.line_width) * scale * MM_PER_PT;
        let paper = match paint {
            Paint::Color(colour) => colour.to_rgba().to_rgba8() == [255, 255, 255, 255],
            Paint::Pattern(_) => false,
        };
        let page_mm = [self.page_pt[0] * MM_PER_PT, self.page_pt[1] * MM_PER_PT];
        let page_h = self.page_pt[1];
        let mut start: Option<Point> = None;
        let mut current: Option<Point> = None;
        let push = |device: &mut Self, from: Point, to: Point| {
            // The reader's own test, in the reader's own frame: page-view
            // millimetres, before the flip to paper space.
            let view = |point: Point| [point.x * MM_PER_PT, point.y * MM_PER_PT];
            if sheet_edge_mm(view(from), view(to), page_mm) {
                device.edges.push(Edge {
                    start_mm: view_pt_to_paper_mm(from.x, from.y, page_h),
                    end_mm: view_pt_to_paper_mm(to.x, to.y, page_h),
                    width_mm,
                    paper,
                });
            }
        };
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
                        push(self, from, point);
                    }
                    current = Some(point);
                }
                PathEl::ClosePath => {
                    if let (Some(from), Some(origin)) = (current, start)
                        && (from - origin).hypot() > 1e-6
                    {
                        push(self, from, origin);
                    }
                    current = start;
                }
                PathEl::QuadTo(_, _) | PathEl::CurveTo(_, _, _) => {}
            },
        );
    }
}

/// The strokes of `pdf_bytes`' first page the reader leaves out as sheet
/// edges, and the page's size in millimetres.
fn sheet_edges(pdf_bytes: &[u8]) -> Option<(Vec<Edge>, [f64; 2])> {
    let pdf = Pdf::new(pdf_bytes.to_vec()).ok()?;
    let pages = pdf.pages();
    let page = pages.first()?;
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
    let mut device = EdgeDevice {
        page_pt: [width_pt, height_pt],
        edges: Vec::new(),
    };
    interpret_page(page, &mut ctx, &mut device);
    Some((device.edges, [width_pt * MM_PER_PT, height_pt * MM_PER_PT]))
}

fn describe(edge: &Edge) -> String {
    let (start, end) = edge.ordered();
    format!(
        "{} ({:8.3},{:8.3})–({:8.3},{:8.3}) {:8.3} mm long, {:.3} mm wide, {}",
        if edge.horizontal() {
            "horizontal"
        } else {
            "vertical  "
        },
        start[0],
        start[1],
        end[0],
        end[1],
        edge.length(),
        edge.width_mm,
        if edge.paper { "PAPER colour" } else { "inked" }
    )
}

fn main() -> ExitCode {
    let wanted: Vec<String> = std::env::args().skip(1).collect();
    let pages = match pages(&wanted) {
        Ok(pages) => pages,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    let mut findings = 0usize;
    let mut total = [0.0f64; 2];
    for page in &pages {
        let (Some((reference, ref_page)), Some((ours, our_page))) =
            (sheet_edges(&page.reference), sheet_edges(&page.ours))
        else {
            eprintln!("{}: a plot could not be read", page.name);
            return ExitCode::from(1);
        };
        let length = |edges: &[Edge]| edges.iter().map(Edge::length).sum::<f64>();
        println!(
            "== {} — reference {:.2} × {:.2} mm drops {} stroke(s), {:.3} mm; ours {:.2} × {:.2} mm \
             drops {} stroke(s), {:.3} mm",
            short(page.name),
            ref_page[0],
            ref_page[1],
            reference.len(),
            length(&reference),
            our_page[0],
            our_page[1],
            ours.len(),
            length(&ours),
        );
        total[0] += length(&reference);
        total[1] += length(&ours);
        let mut taken = vec![false; reference.len()];
        for edge in &ours {
            let partner = reference
                .iter()
                .enumerate()
                .find(|(index, candidate)| !taken[*index] && edge.same_as(candidate));
            match partner {
                Some((index, theirs)) => {
                    taken[index] = true;
                    let width = (edge.width_mm - theirs.width_mm).abs() > 0.02;
                    let colour = edge.paper != theirs.paper;
                    if width || colour {
                        findings += 1;
                        println!("   DIFFERS  ours      {}", describe(edge));
                        println!("            reference {}", describe(theirs));
                    } else {
                        println!("   same     {}", describe(edge));
                    }
                }
                None => {
                    // A second stroke of ours on an edge already paired is the
                    // same line drawn twice, not a line the reference lacks.
                    let twice = reference.iter().any(|candidate| edge.same_as(candidate));
                    if twice {
                        println!("   again    {}", describe(edge));
                    } else {
                        findings += 1;
                        println!("   OURS ONLY      {}", describe(edge));
                    }
                }
            }
        }
        for (index, edge) in reference.iter().enumerate() {
            if !taken[index] {
                findings += 1;
                println!("   REFERENCE ONLY {}", describe(edge));
            }
        }
    }
    println!(
        "{} sheet(s): the reader leaves out {:.3} mm of the references' strokes and {:.3} mm of \
         ours as sheet edges; {} of them differ between the two pages",
        pages.len(),
        total[0],
        total[1],
        findings
    );
    if findings > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
