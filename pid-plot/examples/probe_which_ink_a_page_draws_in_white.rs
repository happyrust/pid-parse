//! Which strokes a page draws in white, and whether the vendor's plot has ink
//! there.
//!
//! **What "white" means moved on 2026-11-11.** This probe was written on
//! 2026-11-10 against the rule of that day, described in the next paragraph,
//! and its readings in
//! `docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
//! §11 are of that rule: the white strokes it listed were border-template
//! rules the vendor prints black. The colour rule that landed on `main`
//! (`dbc0468`, `docs/analysis/2026-11-11-white-prints-and-the-pages-colour-does-not.md`)
//! prints a style that states literal white (`0x00FFFFFF`) black, leaves a
//! stroke whose colour word is `0x8000_0000` — the page's colour — blank,
//! and prints every other colour black. It is applied in the page model, so
//! on a tree that carries it a white stroke of the model is one in the
//! page's colour, and what this probe lists there is the strokes the page
//! leaves blank: the label symbols' origin marks, where the vendor's plot
//! has no ink either.
//!
//! The writer plots through one black pen, with one exception: a stroke or a
//! fill whose style states exact white keeps it (`write.rs`'s `plot_ink`),
//! because a white pen was taken for the plot's wipe. That rule was written
//! when no page model held a white stroke. The border templates do — a
//! title block's grid, a sign-off column's rules, `CPECCHB-SQ-A2`'s inner
//! frame — and white ink on a white page is ink nobody sees.
//!
//! This probe reads the **model**, not the written page: for every sheet it
//! lays out, the strokes and fills whose colour is `[255, 255, 255]`, split
//! into template furniture and the drawing's own ink, with their length and
//! the widths their styles declare. For the eleven sheets that have a vendor
//! plot it also reads the plot (`read_plot`) and says how much of that white
//! length the vendor inks, sampling each stroke every 0.25 mm and counting a
//! sample as inked when reference ink lies within 0.5 mm of it.
//!
//! Two limits, stated so the reading is not taken for more than it is:
//!
//! * `read_plot` reports ink without its colour. A reference plot carries a
//!   few white strokes of its own — two to sixteen a page, each about a
//!   millimetre long (`docs/analysis/2026-10-20-the-page-is-plotted-through-one-black-pen.md`)
//!   — so "the vendor inks it" can be wrong by that much and no more. The
//!   A2 reference sets no colour at all before its frame path.
//! * `read_plot` drops every straight piece that runs more than nine tenths
//!   of a page side (`sheet_edge_mm`), on both sides of a comparison: the
//!   print driver's frame, and with it a template's outer frame and
//!   `CPECCHB-SQ-A2`'s inner one. White pieces that long are counted apart
//!   and left out of the reference reading, which therefore says nothing
//!   about them either way.
//!
//! ```text
//! cargo run -q -p pid-plot --example probe_which_ink_a_page_draws_in_white
//! ```
//!
//! Three batches, found the way the gates find them: the 53 sheets of the
//! plant backup against the package they were shelled out of (the directory
//! above `backup-plant-data` when it holds `RefData~*.zip`, else the share),
//! the committed fixtures under `test-file` against `test-file`, and the
//! eleven reference sheets against `PID_ACCEPTANCE_CORPUS`. A batch whose
//! corpus is absent is skipped, and the probe says so.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_plot::model::{PaperModel, PaperShape, lay_out_drawing};
use pid_plot::read::{InkSegment, read_plot, sheet_edge_mm};

/// The one colour the writer does not plot black.
const WHITE: [u8; 3] = [255, 255, 255];
/// How near reference ink has to lie for a sample to count as inked.
const NEAR_MM: f64 = 0.5;
/// Distance between samples along a white stroke.
const STEP_MM: f64 = 0.25;
/// Cell size of the grid the reference's segments are indexed on.
const CELL_MM: f64 = 4.0;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("pid-plot sits in the repository")
        .to_path_buf()
}

fn share_corpus() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn acceptance_corpus() -> PathBuf {
    std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    )
}

/// The shelled-out plant backup, in `export_plant_backup`'s lookup order.
fn backup_dir() -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(dir) = std::env::var_os("PID_TEST_DATA_DIR") {
        roots.push(PathBuf::from(dir));
    }
    roots.push(share_corpus());
    roots.push(PathBuf::from(r"D:\work\cad\pid-test-data"));
    roots.push(PathBuf::from(r"D:\work\plant-code\cad\pid-test-data"));
    roots
        .into_iter()
        .map(|root| root.join("backup-plant-data"))
        .find(|candidate| candidate.is_dir())
}

/// Whether `dir` holds a backup package's reference-data archive.
fn holds_refdata(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            name.starts_with("refdata~") && name.ends_with(".zip")
        })
    })
}

fn collect_pids(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            // A machine-local symbol library a checkout may link in.
            if path.file_name().is_some_and(|name| name == "symbols-full") {
                continue;
            }
            collect_pids(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "pid") {
            out.push(path);
        }
    }
}

fn length(points: &[[f64; 2]]) -> f64 {
    points
        .windows(2)
        .map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]))
        .sum()
}

/// A stroked shape as the chain of points its ink runs along; `None` for a
/// fill or a label.
fn chain(shape: &PaperShape) -> Option<Vec<[f64; 2]>> {
    let turn = std::f64::consts::TAU;
    let arc = |center: [f64; 2], radius: f64, from: f64, sweep: f64| {
        // `sweep` is signed: positive counter-clockwise.
        let pieces = ((sweep.abs() * radius / STEP_MM).ceil()).max(1.0);
        let mut points = Vec::new();
        let mut step = 0.0;
        while step <= pieces {
            let angle = from + sweep * step / pieces;
            points.push([
                center[0] + radius * angle.cos(),
                center[1] + radius * angle.sin(),
            ]);
            step += 1.0;
        }
        points
    };
    match shape {
        PaperShape::Line { start_mm, end_mm } => Some(vec![*start_mm, *end_mm]),
        PaperShape::Polyline { points_mm, closed } => {
            let mut points = points_mm.clone();
            if *closed && let Some(first) = points_mm.first() {
                points.push(*first);
            }
            Some(points)
        }
        PaperShape::Circle {
            center_mm,
            radius_mm,
        } => Some(arc(*center_mm, *radius_mm, 0.0, turn)),
        PaperShape::Arc {
            center_mm,
            radius_mm,
            start_angle,
            end_angle,
            clockwise,
        } => {
            let mut sweep = (end_angle - start_angle).rem_euclid(turn);
            if *clockwise {
                sweep -= turn;
            }
            if sweep == 0.0 {
                sweep = turn;
            }
            Some(arc(*center_mm, *radius_mm, *start_angle, sweep))
        }
        PaperShape::Fill { .. } | PaperShape::Text { .. } => None,
    }
}

/// A tally of strokes: how many, and how long together.
#[derive(Clone, Copy, Default)]
struct Tally {
    strokes: usize,
    mm: f64,
}

impl Tally {
    fn add(&mut self, mm: f64) {
        self.strokes += 1;
        self.mm += mm;
    }
}

impl std::fmt::Display for Tally {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} / {:.1} mm", self.strokes, self.mm)
    }
}

/// What one page draws in white.
#[derive(Default)]
struct White {
    /// Every stroked primitive of the page, white or not.
    stroked: Tally,
    furniture: Tally,
    drawing: Tally,
    /// White straight pieces over nine tenths of a page side, which
    /// `read_plot` drops as the sheet's frame.
    edge: Tally,
    fills: usize,
    /// White strokes by the width their style declares, millimetres.
    declared: BTreeMap<String, Tally>,
    /// The white pieces `read_plot` would keep, as point chains.
    chains: Vec<Vec<[f64; 2]>>,
}

fn white_of(model: &PaperModel) -> White {
    let mut white = White::default();
    let page = model.page_mm;
    for primitive in &model.primitives {
        if let PaperShape::Fill { rgb, .. } = &primitive.shape {
            if *rgb == Some(WHITE) {
                white.fills += 1;
            }
            continue;
        }
        let Some(points) = chain(&primitive.shape) else {
            continue;
        };
        let mm = length(&points);
        white.stroked.add(mm);
        let Some(stroke) = primitive.stroke.as_ref() else {
            continue;
        };
        if stroke.rgb != WHITE {
            continue;
        }
        if primitive.furniture {
            white.furniture.add(mm);
        } else {
            white.drawing.add(mm);
        }
        white
            .declared
            .entry(format!("{:.4}", stroke.width_mm))
            .or_default()
            .add(mm);
        for pair in points.windows(2) {
            if page.is_some_and(|page| sheet_edge_mm(pair[0], pair[1], page)) {
                white.edge.add(length(pair));
            } else {
                white.chains.push(pair.to_vec());
            }
        }
    }
    white
}

/// The reference's segments on a grid, so a sample asks a few cells.
struct InkGrid<'a> {
    segments: &'a [InkSegment],
    cells: BTreeMap<(i64, i64), Vec<usize>>,
}

#[allow(clippy::cast_possible_truncation)]
fn cell(value: f64) -> i64 {
    (value / CELL_MM).floor() as i64
}

impl<'a> InkGrid<'a> {
    fn new(segments: &'a [InkSegment]) -> Self {
        let mut cells: BTreeMap<(i64, i64), Vec<usize>> = BTreeMap::new();
        for (index, segment) in segments.iter().enumerate() {
            let (x0, x1) = (
                segment.start_mm[0].min(segment.end_mm[0]) - NEAR_MM,
                segment.start_mm[0].max(segment.end_mm[0]) + NEAR_MM,
            );
            let (y0, y1) = (
                segment.start_mm[1].min(segment.end_mm[1]) - NEAR_MM,
                segment.start_mm[1].max(segment.end_mm[1]) + NEAR_MM,
            );
            for column in cell(x0)..=cell(x1) {
                for row in cell(y0)..=cell(y1) {
                    cells.entry((column, row)).or_default().push(index);
                }
            }
        }
        Self { segments, cells }
    }

    /// Whether reference ink lies within [`NEAR_MM`] of `point`.
    fn inks(&self, point: [f64; 2]) -> bool {
        let Some(indices) = self.cells.get(&(cell(point[0]), cell(point[1]))) else {
            return false;
        };
        indices.iter().any(|&index| {
            let segment = &self.segments[index];
            let (ax, ay) = (segment.start_mm[0], segment.start_mm[1]);
            let (dx, dy) = (segment.end_mm[0] - ax, segment.end_mm[1] - ay);
            let span = dx * dx + dy * dy;
            let along = if span > 0.0 {
                (((point[0] - ax) * dx + (point[1] - ay) * dy) / span).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (point[0] - ax - along * dx).hypot(point[1] - ay - along * dy) <= NEAR_MM
        })
    }
}

/// Millimetres of `chains` the reference inks.
fn inked_mm(chains: &[Vec<[f64; 2]>], grid: &InkGrid<'_>) -> f64 {
    let mut inked = 0.0;
    for points in chains {
        for pair in points.windows(2) {
            let (dx, dy) = (pair[1][0] - pair[0][0], pair[1][1] - pair[0][1]);
            let span = dx.hypot(dy);
            if span <= 0.0 {
                continue;
            }
            let pieces = (span / STEP_MM).ceil().max(1.0);
            let mut hit = 0.0;
            let mut step = 0.0;
            while step < pieces {
                // The middle of each piece, so a piece is inked or it is not.
                let at = (step + 0.5) / pieces;
                if grid.inks([pair[0][0] + dx * at, pair[0][1] + dy * at]) {
                    hit += 1.0;
                }
                step += 1.0;
            }
            inked += span * hit / pieces;
        }
    }
    inked
}

/// One batch of pages laid out against one corpus.
struct Batch {
    name: &'static str,
    corpus: PathBuf,
    /// `(page key, .pid path, reference plot)`.
    pages: Vec<(String, PathBuf, Option<PathBuf>)>,
}

fn key_below(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn batches() -> Vec<Batch> {
    let mut out = Vec::new();
    match backup_dir() {
        Some(backup) => {
            let package = backup.parent().filter(|dir| holds_refdata(dir));
            let corpus = package.map_or_else(share_corpus, Path::to_path_buf);
            let mut pids = Vec::new();
            collect_pids(&backup, &mut pids);
            out.push(Batch {
                name: "backup",
                corpus,
                pages: pids
                    .iter()
                    .map(|path| (key_below(&backup, path), path.clone(), None))
                    .collect(),
            });
        }
        None => eprintln!("skipping the backup batch; no backup-plant-data directory is present"),
    }
    let fixtures = repo_root().join("test-file");
    let mut pids = Vec::new();
    collect_pids(&fixtures, &mut pids);
    out.push(Batch {
        name: "fixtures",
        corpus: fixtures.clone(),
        pages: pids
            .iter()
            .map(|path| (key_below(&fixtures, path), path.clone(), None))
            .collect(),
    });
    let acceptance = acceptance_corpus();
    if acceptance.is_dir() {
        let mut pages = Vec::new();
        let mut pids = Vec::new();
        collect_pids(&acceptance, &mut pids);
        for path in pids {
            // Only the sheets the vendor plotted: a `.pdf` of the same name
            // beside the drawing, not this crate's own output under `plot`.
            if path.parent() != Some(acceptance.as_path()) {
                continue;
            }
            let reference = path.with_extension("pdf");
            if reference.is_file() {
                pages.push((key_below(&acceptance, &path), path, Some(reference)));
            }
        }
        out.push(Batch {
            name: "reference",
            corpus: acceptance,
            pages,
        });
    } else {
        eprintln!(
            "skipping the reference batch; {} is absent",
            acceptance.display()
        );
    }
    out
}

fn template_name(model: &PaperModel) -> String {
    model.template.as_ref().map_or_else(
        || "no template".to_string(),
        |template| {
            template
                .path
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or(&template.path)
                .to_string()
        },
    )
}

fn main() {
    let mut failed = false;
    for batch in batches() {
        println!(
            "== {}: {} page(s) against {}",
            batch.name,
            batch.pages.len(),
            batch.corpus.display()
        );
        let mut pages_with_white = 0usize;
        let mut furniture = Tally::default();
        let mut drawing = Tally::default();
        let mut by_template: BTreeMap<String, (usize, Tally)> = BTreeMap::new();
        let mut vendor_inked = 0.0;
        let mut vendor_asked = 0.0;
        for (key, path, reference) in &batch.pages {
            let model = match lay_out_drawing(path, &batch.corpus) {
                Ok(model) => model,
                Err(error) => {
                    eprintln!("{key}: {error}");
                    failed = true;
                    continue;
                }
            };
            let white = white_of(&model);
            let template = template_name(&model);
            let total = white.furniture.strokes + white.drawing.strokes;
            if total > 0 || white.fills > 0 {
                pages_with_white += 1;
            }
            furniture.strokes += white.furniture.strokes;
            furniture.mm += white.furniture.mm;
            drawing.strokes += white.drawing.strokes;
            drawing.mm += white.drawing.mm;
            let row = by_template.entry(template.clone()).or_default();
            row.0 += 1;
            row.1.strokes += total;
            row.1.mm += white.furniture.mm + white.drawing.mm;
            let declared = white
                .declared
                .iter()
                .map(|(width, tally)| format!("{width} mm ×{tally}"))
                .collect::<Vec<_>>()
                .join(", ");
            let mut line = format!(
                "{key}: {template}; white furniture {}, white drawing ink {}, of {} stroked; \
                 pieces over 0.9 of a page side {}; white fills {}; declared [{declared}]",
                white.furniture, white.drawing, white.stroked, white.edge, white.fills
            );
            if let Some(reference) = reference {
                let asked = length_of(&white.chains);
                match std::fs::read(reference)
                    .map_err(|error| error.to_string())
                    .and_then(|bytes| read_plot(&bytes).map_err(|error| error.to_string()))
                {
                    Ok(ink) => {
                        let grid = InkGrid::new(&ink.segments);
                        let inked = inked_mm(&white.chains, &grid);
                        vendor_inked += inked;
                        vendor_asked += asked;
                        let share = if asked > 0.0 {
                            100.0 * inked / asked
                        } else {
                            0.0
                        };
                        line.push_str(&format!(
                            "; the vendor's plot inks {inked:.1} mm of the other {asked:.1} mm \
                             ({share:.1}%)"
                        ));
                    }
                    Err(error) => {
                        eprintln!("{}: {error}", reference.display());
                        failed = true;
                    }
                }
            }
            println!("{line}");
        }
        println!(
            "-- {}: {pages_with_white} of {} page(s) hold white ink; furniture {furniture}, \
             drawing ink {drawing}",
            batch.name,
            batch.pages.len()
        );
        for (template, (pages, tally)) in &by_template {
            println!("   {template}: {pages} page(s), white {tally}");
        }
        if vendor_asked > 0.0 {
            println!(
                "   the vendor's plots ink {vendor_inked:.1} mm of the {vendor_asked:.1} mm \
                 `read_plot` keeps ({:.1}%)",
                100.0 * vendor_inked / vendor_asked
            );
        }
    }
    if failed {
        std::process::exit(1);
    }
}

fn length_of(chains: &[Vec<[f64; 2]>]) -> f64 {
    chains.iter().map(|points| length(points)).sum()
}
