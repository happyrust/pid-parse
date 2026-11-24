//! Which style vocabulary a page draws with, and whether any page that has a
//! reference plot has ever exercised it.
//!
//! A page can only be proved correct by comparing it with the plot a vendor's
//! own plotter produced from it. Forty-eight of the corpus's fifty-three plant
//! drawings have no such plot, so the question "is this page right?" has no
//! direct answer. It does have a decomposition: a page's ink is drawn with a
//! small vocabulary — a pen width per stroke, a dash pattern, a face per text
//! run, and the furniture of the border template it wears — and each of those
//! entries is either one a *validated* page also uses, in which case the
//! machinery that turned it into ink has been checked against a real plot, or
//! it is not, in which case only that entry is unproven and everything else on
//! the page still stands on the validated ones.
//!
//! That is the whole point of this module: it turns "three of these fifty-three
//! sheets have no reference plot" into "these seven pen widths and this one
//! dash pattern have never been plotted", which is a list short enough to
//! settle by hand.
//!
//! A *validated* page is one whose own plot sits beside it — [`crate::compare`]
//! is what decides that a page matches, and a page whose comparison failed must
//! not be used as a witness, or the vocabulary it contributes would be
//! certified by a page that is itself wrong.
//!
//! ```text
//! use pid_plot::model::lay_out_drawing;
//! use pid_plot::witness::Witness;
//!
//! let witnessed = Witness::of(&[("A", &model_a), ("B", &model_b)]);
//! let report = witnessed.unwitnessed(&[("C", &model_c)]);
//! for entry in &report.entries {
//!     println!("{} {} on {} sheet(s)", entry.dimension, entry.value, entry.sheets.len());
//! }
//! ```
//!
//! A width and a dash are compared by value, because that value is what the
//! pen table and the dash walk consume; a face is compared by the name the
//! writer embeds, because that name is what selects the file. A template is
//! compared by its file name — the sheet's furniture is that file, drawn 1:1,
//! so nothing about it depends on which sheet wears it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::fonts::typeface_key;
use crate::model::{BorderTemplate, PaperModel, PaperShape};

/// Which plot set carries the PDF a [`PaperEdgeWitness`] was read from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaperEdgeSource {
    /// One of the eleven acceptance plots (`验收数据`), checked two-way by
    /// `reference_plots_acceptance`.
    Acceptance,
    /// One of the backup's factory Amyuni plots
    /// (`publish/sppid导出/导出dxf pdf/导出pdf`), checked two-way by
    /// `paper_edge_factory_plots`.
    FactoryBackup,
}

/// A border template put past its paper's corner by a sheet's frame, as a
/// vendor's plot shows it printed.
///
/// A frame may place its template off the corner of the paper, and then the
/// top and the right edge of the template's outline lie off the paper. What
/// a plot does with that ink is read off a plot, and a plot shows it for the
/// template and the offset it was plotted with and for no other: a different
/// offset puts different pieces off the paper, and another template is
/// another outline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaperEdgeWitness {
    /// The template's file name without its extension.
    pub template: &'static str,
    /// Where the frame puts the template, paper millimetres
    /// ([`BorderTemplate::offset_mm`]), to the micrometre.
    pub offset_mm: [f64; 2],
    /// The drawing whose reference plot shows it, by its file name without
    /// the extension; the plot is the `.pdf` beside it.
    pub plot: &'static str,
    /// Which plot set carries that `.pdf`.
    pub source: PaperEdgeSource,
}

/// Every template placement past a paper's corner that a vendor's plot
/// shows.
///
/// Six rows covering all fifteen frame-offset sheets of the 53-sheet plant
/// backup:
///
/// - One acceptance plot (`轻烃分离器I气出口接口位置改造`), wearing
///   `CPECCHB-SQ-A2` at `(0.388407, 0.565648)` mm, covering the nine
///   `CPECCHB-SQ-A2` sheets.
/// - Five distinct `(template, offset_mm)` placements from the six backup
///   factory Amyuni plots (`A井场`, `N井场`, `O井场`/`P井场`, `1200td`,
///   `600td`, #59): every one of the six PDFs has the sheet's declared ISO
///   paper as its page box and prints no outline segment past the paper edge
///   (`O井场` and `P井场` share `CPECCHB-SQ-A1` at `(0.150015, 0.399965)` mm;
///   `1200td`'s factory plot is bare on A1 paper and `600td` plots
///   `CPE-SQ-A1` off-corner clipped to A1 paper).
///
/// Each row is held two-way to its corpus by `reference_plots_acceptance`
/// ([`PaperEdgeSource::Acceptance`]) or `paper_edge_factory_plots`
/// ([`PaperEdgeSource::FactoryBackup`]).
pub const PAPER_EDGE_WITNESSES: [PaperEdgeWitness; 6] = [
    PaperEdgeWitness {
        template: "CPECCHB-SQ-A2",
        offset_mm: [0.388_407, 0.565_648],
        plot: "集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图",
        source: PaperEdgeSource::Acceptance,
    },
    PaperEdgeWitness {
        template: "CPE-SQ-A0",
        offset_mm: [0.152_593, 0.152_593],
        plot: "A井场 注采阀组工艺及自控流程图",
        source: PaperEdgeSource::FactoryBackup,
    },
    PaperEdgeWitness {
        template: "CPECCHB-SQ-A1",
        offset_mm: [1.297_400, 1.553_387],
        plot: "N井场 注采阀组工艺及自控流程图",
        source: PaperEdgeSource::FactoryBackup,
    },
    PaperEdgeWitness {
        template: "CPECCHB-SQ-A1",
        offset_mm: [0.150_015, 0.399_965],
        plot: "O井场 注采阀组工艺及自控流程图",
        source: PaperEdgeSource::FactoryBackup,
    },
    PaperEdgeWitness {
        template: "CPE-SQ-A1",
        offset_mm: [1.042_526, 1.042_526],
        plot: "集注站 凝析油稳定单元 凝析油稳定塔、泵及换热器(1200td)工艺及自控流程图",
        source: PaperEdgeSource::FactoryBackup,
    },
    PaperEdgeWitness {
        template: "CPE-SQ-A1",
        offset_mm: [0.768_634, 0.768_634],
        plot: "集注站 凝析油稳定单元 凝析油稳定塔、泵及换热器(600td)工艺及自控流程图",
        source: PaperEdgeSource::FactoryBackup,
    },
];

/// How near a template's offset has to be to a witnessed one to be it, in
/// millimetres: a micrometre, the precision the rows are stated to.
pub const PAPER_EDGE_OFFSET_SLACK_MM: f64 = 0.001;

/// The reference plot that shows `template` placed as it is, if one does:
/// the row of [`PAPER_EDGE_WITNESSES`] with its file name and its offset.
///
/// A template the frame scales is no row's: every plot on hand draws its
/// template 1:1.
#[must_use]
pub fn paper_edge_witness(template: &BorderTemplate) -> Option<&'static PaperEdgeWitness> {
    let leaf = leaf_name(&template.path);
    let stem = leaf
        .rsplit_once('.')
        .map_or(leaf.as_str(), |(stem, _extension)| stem);
    #[allow(clippy::float_cmp)]
    let drawn_one_to_one = template.scale == 1.0;
    PAPER_EDGE_WITNESSES.iter().find(|witness| {
        drawn_one_to_one
            && witness.template.eq_ignore_ascii_case(stem)
            && (0..2).all(|axis| {
                (witness.offset_mm[axis] - template.offset_mm[axis]).abs()
                    <= PAPER_EDGE_OFFSET_SLACK_MM
            })
    })
}

/// One dimension of the style vocabulary a page draws with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Dimension {
    /// Stroke width in millimetres, as the style declares it.
    ///
    /// This is the declared width, not the plotter pen the writer snaps it to:
    /// the pen is a measured, banded mapping, so the width is the value whose
    /// band has to have been exercised.
    PenWidth,
    /// Dash pattern, in the order and magnitudes the model carries (millimetres,
    /// pen-down then pen-up).
    DashPattern,
    /// Text face, by the name whose file the writer embeds.
    Face,
    /// Border template, by file name.
    Template,
}

impl fmt::Display for Dimension {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::PenWidth => "pen width",
            Self::DashPattern => "dash pattern",
            Self::Face => "face",
            Self::Template => "template",
        };
        formatter.write_str(name)
    }
}

/// One vocabulary entry that no validated page uses, and what the pages that
/// do use it draw with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unwitnessed {
    /// Which dimension the entry belongs to.
    pub dimension: Dimension,
    /// The entry itself, in the form its dimension compares by.
    pub value: String,
    /// Strokes or runs drawn from the sheet's own content.
    ///
    /// Furniture is counted apart from it because the two carry different
    /// consequences: a template's stroke is a file drawn 1:1, so an
    /// unexercised width there changes how heavy a known shape prints, while
    /// the same width on the drawing's own ink would put a wrong weight on the
    /// process lines an engineer reads.
    pub drawing: usize,
    /// Strokes or runs drawn from the border template's furniture.
    pub furniture: usize,
    /// Sheets carrying the entry, by the name they were handed over with.
    pub sheets: BTreeSet<String>,
}

impl Unwitnessed {
    /// Uses in total, furniture and drawing together.
    #[must_use]
    pub fn uses(&self) -> usize {
        self.drawing.saturating_add(self.furniture)
    }
}

/// The style vocabulary a set of validated pages draws with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Witness {
    entries: BTreeSet<(Dimension, String)>,
}

impl Witness {
    /// Collect the vocabulary of pages whose plot has been compared and agreed.
    ///
    /// Passing a page that has *not* been validated certifies its vocabulary
    /// against nothing, so the caller owns that boundary: the entry point that
    /// uses this reads it from a plot being present beside the drawing.
    #[must_use]
    pub fn of<'a, I>(pages: I) -> Self
    where
        I: IntoIterator<Item = (&'a str, &'a PaperModel)>,
    {
        let mut entries = BTreeSet::new();
        for (_, model) in pages {
            vocabularies(model, |dimension, value, _| {
                entries.insert((dimension, value));
            });
        }
        Self { entries }
    }

    /// How many entries the validated pages witness.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing has been witnessed at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The witnessed entries of one dimension, sorted, for a report header.
    #[must_use]
    pub fn values(&self, dimension: Dimension) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|(entry_dimension, _)| *entry_dimension == dimension)
            .map(|(_, value)| value.as_str())
            .collect()
    }

    /// Whether a given entry has been exercised by a validated page.
    #[must_use]
    pub fn covers(&self, dimension: Dimension, value: &str) -> bool {
        self.entries.contains(&(dimension, value.to_string()))
    }

    /// Every entry the given pages use that no validated page has exercised.
    ///
    /// Entries are keyed by dimension and value; the count and the sheets are
    /// the pages that need them, so one entry names every page it affects.
    #[must_use]
    pub fn unwitnessed<'a, I>(&self, pages: I) -> Report
    where
        I: IntoIterator<Item = (&'a str, &'a PaperModel)>,
    {
        let mut found: BTreeMap<(Dimension, String), Unwitnessed> = BTreeMap::new();
        let mut sheets_checked = 0usize;
        let mut sheets_clean = 0usize;
        for (sheet, model) in pages {
            sheets_checked += 1;
            let mut unwitnessed_here = 0usize;
            vocabularies(model, |dimension, value, furniture| {
                if self.covers(dimension, &value) {
                    return;
                }
                unwitnessed_here += 1;
                let entry =
                    found
                        .entry((dimension, value.clone()))
                        .or_insert_with(|| Unwitnessed {
                            dimension,
                            value,
                            drawing: 0,
                            furniture: 0,
                            sheets: BTreeSet::new(),
                        });
                if furniture {
                    entry.furniture += 1;
                } else {
                    entry.drawing += 1;
                }
                entry.sheets.insert(sheet.to_string());
            });
            if unwitnessed_here == 0 {
                sheets_clean += 1;
            }
        }
        Report {
            entries: found.into_values().collect(),
            sheets_checked,
            sheets_clean,
        }
    }
}

/// What the closure found over a set of pages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// Every entry no validated page exercises, sorted by dimension then value.
    pub entries: Vec<Unwitnessed>,
    /// Pages examined.
    pub sheets_checked: usize,
    /// Pages whose every entry a validated page exercises.
    ///
    /// `sheets_checked - sheets_clean` is how many pages have something to
    /// prove, which is the number this module exists to make small.
    pub sheets_clean: usize,
}

impl Report {
    /// Entries of one dimension.
    pub fn of_dimension(&self, dimension: Dimension) -> impl Iterator<Item = &Unwitnessed> {
        self.entries
            .iter()
            .filter(move |entry| entry.dimension == dimension)
    }

    /// Sheets carrying at least one unwitnessed entry.
    #[must_use]
    pub fn sheets_affected(&self) -> BTreeSet<&str> {
        self.entries
            .iter()
            .flat_map(|entry| entry.sheets.iter().map(String::as_str))
            .collect()
    }

    /// Whether the drawing's own ink — everything but the border template —
    /// uses only exercised vocabulary.
    ///
    /// A template's pen is a file's declaration drawn 1:1; the drawing's own
    /// ink is what an engineer reads a process from. A page whose *drawing* is
    /// fully witnessed is a page whose unexercised entries are all furniture.
    #[must_use]
    pub fn drawing_is_witnessed(&self) -> bool {
        self.entries.iter().all(|entry| entry.drawing == 0)
    }
}

/// Walk one page's vocabulary, one entry per distinct use.
///
/// The last argument says whether the use is the sheet's own ink. A text run
/// counts one use per face it letters with; a shape counts one per pen and
/// dash it carries, which is how the page states them.
fn vocabularies(model: &PaperModel, mut on_entry: impl FnMut(Dimension, String, bool)) {
    if let Some(template) = &model.template {
        on_entry(Dimension::Template, leaf_name(&template.path), true);
    }
    for primitive in &model.primitives {
        let furniture = primitive.furniture;
        if let Some(stroke) = &primitive.stroke {
            on_entry(
                Dimension::PenWidth,
                format!("{:.4}", stroke.width_mm),
                furniture,
            );
            if !stroke.dash_mm.is_empty() {
                let pattern: Vec<String> = stroke
                    .dash_mm
                    .iter()
                    .map(|segment| format!("{segment:.3}"))
                    .collect();
                on_entry(Dimension::DashPattern, pattern.join("/"), furniture);
            }
        }
        if let PaperShape::Text {
            value, font, spans, ..
        } = &primitive.shape
        {
            // The name the writer embeds, not the spelling the file used. A
            // face is covered when the *file* it selects is one a validated
            // page exercised, and `宋体`, `ËÎÌå` and `SimSun` select one
            // file; reporting the spelling made every page that said `宋体`
            // unexercised against a validated page that said `SimSun`.
            let faces = std::iter::once(font.as_deref())
                .chain(spans.iter().map(|span| span.font.as_deref()))
                .flatten();
            for face in faces {
                on_entry(
                    Dimension::Face,
                    typeface_key(Some(face), value).to_string(),
                    furniture,
                );
            }
        }
    }
}

/// The file name a stored template path ends in.
///
/// A template arrives from a share as `…\Ref\Template Files\CPE-SQ-A1.igr` and
/// from a backup's archive as `…\RefData~4~682.zip\CPE-SQ-A1.igr`; the file is
/// the same file, so the name is what identifies it.
fn leaf_name(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or(path).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PaperPrimitive, PaperShape, PaperStroke};

    fn pen(width_mm: f64, dash_mm: Vec<f64>) -> Option<PaperStroke> {
        Some(PaperStroke {
            rgb: [0, 0, 0],
            width_mm,
            dash_mm,
            dash_fit: None,
        })
    }

    fn line(pen: Option<PaperStroke>, furniture: bool) -> PaperPrimitive {
        PaperPrimitive {
            confidence: "decoded",
            stream_path: None,
            byte_range: None,
            symbol: None,
            stroke: pen,
            shape: PaperShape::Line {
                start_mm: [0.0, 0.0],
                end_mm: [10.0, 0.0],
            },
            furniture,
        }
    }

    fn text(face: &str) -> PaperPrimitive {
        PaperPrimitive {
            confidence: "decoded",
            stream_path: None,
            byte_range: None,
            symbol: None,
            stroke: None,
            shape: PaperShape::Text {
                origin_mm: [0.0, 0.0],
                height_mm: Some(2.5),
                value: "N".to_string(),
                font: Some(face.to_string()),
                anchor: crate::model::TextAnchor::Left,
                line: crate::model::TextLine::Middle,
                height_basis: crate::model::TextHeightBasis::Cap,
                width_permille: None,
                paragraph: None,
                spans: Vec::new(),
                rotation: 0.0,
            },
            furniture: false,
        }
    }

    fn model(primitives: Vec<PaperPrimitive>) -> PaperModel {
        PaperModel {
            library: Vec::new(),
            page_mm: Some([841.0, 594.0]),
            template: None,
            primitives,
            off_sheet_primitives: 0,
            frame_mm: None,
            skipped_endpoint_pairs: 0,
            missing_symbols: Vec::new(),
            library_gaps: Vec::new(),
            missing_templates: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
            ledger: crate::ledger::LayoutLedger::default(),
        }
    }

    fn template(path: &str) -> crate::model::BorderTemplate {
        crate::model::BorderTemplate {
            path: path.to_string(),
            extent_mm: [841.0, 594.0],
            offset_mm: [0.0, 0.0],
            scale: 1.0,
            primitives: 0,
            symbols: Vec::new(),
        }
    }

    /// `path` placed at `offset_mm`, drawn 1:1.
    fn placed(path: &str, offset_mm: [f64; 2]) -> crate::model::BorderTemplate {
        crate::model::BorderTemplate {
            offset_mm,
            ..template(path)
        }
    }

    #[test]
    fn the_a2_template_is_witnessed_at_the_offset_its_plot_shows() {
        // Wherever the file was read from: a share's directory or a
        // backup's archive.
        for path in [
            r"C:\Ref\Template Files\CPECCHB-SQ-A2.igr",
            r"C:\pkg\RefData~4~682.zip\CPECCHB-SQ-A2.igr",
            "/corpus/Ref/Template Files/cpecchb-sq-a2.IGR",
        ] {
            let witness = paper_edge_witness(&placed(path, [0.388_407, 0.565_648]))
                .unwrap_or_else(|| panic!("{path}: no witness"));
            assert_eq!(witness.template, "CPECCHB-SQ-A2");
            assert!(witness.plot.contains("轻烃分离器I"));
        }
        // The offset as a layout computes it, the last bits off the row's.
        assert!(
            paper_edge_witness(&placed("CPECCHB-SQ-A2.igr", [0.388_407_4, 0.565_647_6])).is_some()
        );
    }

    #[test]
    fn the_six_factory_plotted_backup_placements_are_witnessed() {
        for (path, offset_mm, expected_plot) in [
            ("CPE-SQ-A0.igr", [0.152_593, 0.152_593], "A井场"),
            ("CPECCHB-SQ-A1.igr", [1.297_400, 1.553_387], "N井场"),
            ("CPECCHB-SQ-A1.igr", [0.150_015, 0.399_965], "O井场"),
            ("CPE-SQ-A1.igr", [1.042_526, 1.042_526], "1200td"),
            ("CPE-SQ-A1.igr", [0.768_634, 0.768_634], "600td"),
        ] {
            let witness = paper_edge_witness(&placed(path, offset_mm))
                .unwrap_or_else(|| panic!("{path} at {offset_mm:?}: no witness"));
            assert_eq!(witness.source, PaperEdgeSource::FactoryBackup);
            assert!(
                witness.plot.contains(expected_plot),
                "{path}: expected {expected_plot}, got {}",
                witness.plot
            );
        }
    }

    #[test]
    fn a_placement_no_plot_shows_has_no_witness() {
        for (path, offset_mm) in [
            // A fixture template with no factory plot.
            ("XIONGANA2.igr", [0.500_000, 0.500_000]),
            // A witnessed template somewhere no plot shows it.
            ("CPE-SQ-A1.igr", [2.500_000, 2.500_000]),
            ("CPECCHB-SQ-A2.igr", [0.150_015, 0.399_965]),
            ("CPECCHB-SQ-A2.igr", [0.388_407, 0.565_648 + 0.002]),
            ("CPECCHB-SQ-A2.igr", [0.388_407 - 0.002, 0.565_648]),
            ("CPECCHB-SQ-A2.igr", [0.0, 0.0]),
            // Another file whose name only begins the same.
            ("CPECCHB-SQ-A2 - New.igr", [0.388_407, 0.565_648]),
        ] {
            assert_eq!(
                paper_edge_witness(&placed(path, offset_mm)),
                None,
                "{path} at {offset_mm:?}"
            );
        }
        // The witnessed template at the witnessed offset, scaled.
        let scaled = crate::model::BorderTemplate {
            scale: 2.0,
            ..placed("CPECCHB-SQ-A2.igr", [0.388_407, 0.565_648])
        };
        assert_eq!(paper_edge_witness(&scaled), None);
    }

    #[test]
    fn a_width_a_witnessed_page_draws_with_is_covered_and_another_is_not() {
        let witnessed = model(vec![line(pen(0.35, Vec::new()), false)]);
        let witness = Witness::of([("A", &witnessed)]);
        assert!(witness.covers(Dimension::PenWidth, "0.3500"));
        let other = model(vec![line(pen(0.30, Vec::new()), false)]);
        let report = witness.unwitnessed([("B", &other)]);
        assert_eq!(report.entries.len(), 1);
        let entry = &report.entries[0];
        assert_eq!(entry.dimension, Dimension::PenWidth);
        assert_eq!(entry.value, "0.3000");
        assert_eq!(entry.drawing, 1);
        assert_eq!(entry.furniture, 0);
        assert_eq!(report.sheets_checked, 1);
        assert_eq!(report.sheets_clean, 0);
        assert!(!report.drawing_is_witnessed());
    }

    #[test]
    fn a_template_stroke_is_counted_apart_from_the_drawings_own_ink() {
        let witnessed = model(vec![line(pen(0.35, Vec::new()), false)]);
        let witness = Witness::of([("A", &witnessed)]);
        let sheet = model(vec![
            line(pen(0.30, Vec::new()), true),
            line(pen(0.30, Vec::new()), true),
            line(pen(0.35, Vec::new()), false),
        ]);
        let report = witness.unwitnessed([("B", &sheet)]);
        let entry = report.entries.first().expect("one entry");
        assert_eq!(entry.furniture, 2);
        assert_eq!(entry.drawing, 0);
        assert_eq!(entry.uses(), 2);
        // The page's own ink is all witnessed: only its furniture is in doubt.
        assert!(report.drawing_is_witnessed());
    }

    #[test]
    fn one_entry_names_every_sheet_that_needs_it() {
        let witness = Witness::of([("A", &model(vec![line(pen(0.35, Vec::new()), false)]))]);
        let first = model(vec![line(pen(0.30, Vec::new()), false)]);
        let second = model(vec![line(pen(0.30, Vec::new()), false)]);
        let report = witness.unwitnessed([("B", &first), ("C", &second)]);
        assert_eq!(report.entries.len(), 1);
        assert_eq!(
            report.sheets_affected(),
            ["B", "C"].into_iter().collect::<BTreeSet<_>>()
        );
        assert_eq!(report.entries[0].uses(), 2);
    }

    #[test]
    fn a_clean_sheet_reports_nothing_and_still_counts_as_checked() {
        let page = model(vec![line(pen(0.35, Vec::new()), false)]);
        let witness = Witness::of([("A", &page)]);
        let report = witness.unwitnessed([("B", &page)]);
        assert!(report.entries.is_empty());
        assert_eq!(report.sheets_checked, 1);
        assert_eq!(report.sheets_clean, 1);
        assert!(report.drawing_is_witnessed());
    }

    #[test]
    fn a_dash_pattern_compares_by_its_magnitudes_in_order() {
        let dashed = model(vec![line(pen(0.35, vec![1.905, 1.270]), false)]);
        let witness = Witness::of([("A", &dashed)]);
        assert!(witness.covers(Dimension::DashPattern, "1.905/1.270"));
        let longer = model(vec![line(pen(0.35, vec![3.5, 1.75]), false)]);
        let report = witness.unwitnessed([("B", &longer)]);
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].dimension, Dimension::DashPattern);
        assert_eq!(report.entries[0].value, "3.500/1.750");
    }

    #[test]
    fn a_face_is_witnessed_by_the_name_the_writer_embeds() {
        let witnessed = model(vec![text("Arial Narrow")]);
        let witness = Witness::of([("A", &witnessed)]);
        assert!(witness.covers(Dimension::Face, "Arial Narrow"));
        let report = witness.unwitnessed([("B", &model(vec![text("仿宋")]))]);
        assert_eq!(report.entries.len(), 1);
        assert_eq!(
            report.entries[0].value, "FangSong",
            "a face is compared by the file the writer embeds, not by the spelling the drawing used"
        );
    }

    #[test]
    fn a_template_is_identified_by_its_file_name_wherever_it_was_read_from() {
        let mut disk = model(Vec::new());
        disk.template = Some(template(r"C:\Ref\Template Files\CPE-SQ-A1.igr"));
        let witness = Witness::of([("A", &disk)]);
        assert!(witness.covers(Dimension::Template, "CPE-SQ-A1.igr"));

        let mut archive = model(Vec::new());
        archive.template = Some(template(r"C:\pkg\RefData~4~682.zip\CPE-SQ-A1.igr"));
        assert!(witness.unwitnessed([("B", &archive)]).entries.is_empty());

        let mut other = model(Vec::new());
        other.template = Some(template(r"C:\Ref\Template Files\CPECCHB-SQ-A2.igr"));
        let report = witness.unwitnessed([("C", &other)]);
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].dimension, Dimension::Template);
    }
}
