//! The eleven reference plots, compared as ink.
//!
//! `reference_plots_acceptance` compares paint with paint: its reader keeps
//! no colour, so a rule stroked in the paper's white counts as a rule, and it
//! reads ten of the eleven pages at coverage 1.000000 while their title
//! blocks print no rules at all
//! (`docs/analysis/2026-11-10-the-acceptance-gate-reads-paint-not-ink.md`).
//! This gate reads the same eleven pages for what prints. Both pages go
//! through [`PageInk::read`], and every page is held to:
//!
//! * the comparison of the two pages' **ink** — coverage, uncovered reference
//!   ink, extra ink, label pairing — with the surveyed furniture masked and
//!   on the whole page, and the pen pairing (which reads no mask);
//! * **colour**: of the strokes and labels the two pages share, how many
//!   print alike and how many one page inks where the other only paints the
//!   paper ([`stroke_colours`], [`label_colours`]), and whether the two pages
//!   ink in the same colours at all;
//! * the **strokes along the sheet** the reader leaves out of both pages —
//!   the frame lines — paired by place and held to their widths
//!   ([`sheet_edges`]);
//! * the **page**: how far our sheet is from the reference's paper, with no
//!   allowance beyond what the reference states its own size to;
//! * the **line ends**: how much of our ink is stroked with another cap or
//!   another join than the reference's stroke at the same place
//!   ([`ends_apart`]);
//! * the **order**: wherever the reference paints a stroke in the paper's
//!   colour on ink — a label's origin mark on a box edge or on a character —
//!   whether our page paints the two in the same order, which is what
//!   decides whether the mark takes a piece out of the ink ([`wipe_order`]).
//!
//! What decides that order is the sheet's own list, and a second test holds
//! it: `a_label_and_its_origin_mark_are_painted_in_the_order_the_sheet_lists_them`
//! reads every label whose symbol carries an origin mark — 96 on the eleven
//! sheets — for where the sheet lists the symbol and the text and which the
//! plot paints first ([`MARKED_LABELS`]). 95 are painted as listed. The one
//! that is not is pinned with the sheet that holds the same label at the
//! same places and is plotted the other way ([`AGAINST_ITS_LIST`],
//! [`ITS_TWIN`]), which is why that place is a known defect line and not a
//! rule our page could take up.
//!
//! # How a page is pinned
//!
//! A page that prints as its reference does reads the same on every line
//! above: coverage 1.000000, nothing uncovered, nothing extra, every label
//! paired, no pen apart, no colour apart, no edge apart, the same sheet, the
//! same ends. So a sheet's entry in [`SHEETS`] states only what is the
//! page's own — how many labels its reference letters, how many strokes pair
//! — and every reading that falls short of that is one line of its
//! `defects`, with the value it reads and why: not fixed yet, waiting on data
//! from the site, or not settled. The goal is a table with no such line.
//!
//! Every reading is an equality. One that gets worse fails; one that gets
//! better fails too, until its pin is tightened or its defect line is taken
//! out, so each step forward is a line of history.
//!
//! # Running it
//!
//! ```text
//! cargo test -p pid-plot --test reference_plots_ink -- --nocapture
//! ```
//!
//! The corpus is `PID_ACCEPTANCE_CORPUS`, default
//! `D:\work\cad\pid-test-data\验收数据`; when its first reference plot is
//! absent the gate says so and passes, as the acceptance gate does. It
//! writes nothing into the corpus, ever. `PID_INK_GATE_OUT=<dir>` makes it
//! write the table it prints to `<dir>/reference-plots-ink.txt`.

use std::fmt::Write as _;
use std::path::PathBuf;

use pid_plot::compare::{CompareReport, MaskRect, compare_plots_with_model, template_masks};
use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::ink::{PageInk, ends_apart, label_colours, sheet_edges, stroke_colours, wipe_order};
use pid_plot::model::{PaperModel, lay_out_drawing};
use pid_plot::read::{SheetEdge, read_plot_sequence};
use pid_plot::write::{PlotFont, write_plot};

#[path = "support/exported_page.rs"]
mod exported_page;
#[path = "support/marked_labels.rs"]
mod marked_labels;
#[path = "../src/scratch.rs"]
mod scratch;

/// One number a page is held to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    /// Our sheet's width less the reference paper's, millimetres.
    PageWidthApart,
    /// Our sheet's height less the reference paper's, millimetres.
    PageHeightApart,
    /// Coverage of the reference's ink outside the masks.
    MaskedCoverage,
    /// Reference ink outside the masks no ink of ours covers, millimetres.
    MaskedUncovered,
    /// Our ink outside the masks on no ink of the reference's, millimetres.
    MaskedExtra,
    /// Labels the reference letters outside the masks.
    MaskedLabels,
    /// Of those, the ones the two pages pair.
    MaskedLabelsPaired,
    /// Runs we letter outside the masks that pair with none of the
    /// reference's.
    MaskedLabelsAlone,
    /// Coverage of all the reference's ink.
    WholeCoverage,
    /// Reference ink no ink of ours covers, millimetres.
    WholeUncovered,
    /// Our ink on no ink of the reference's, millimetres.
    WholeExtra,
    /// Labels the reference letters.
    WholeLabels,
    /// Of those, the ones the two pages pair.
    WholeLabelsPaired,
    /// Runs we letter that pair with none of the reference's.
    WholeLabelsAlone,
    /// Inked strokes of ours the reference prints with the same pen.
    PenAgreed,
    /// Inked strokes of ours the reference prints with another pen.
    PenDiffered,
    /// Inked strokes of ours with no inked stroke of the reference's.
    PenUnpaired,
    /// Strokes the two pages share and print in the same colour.
    StrokesSameColour,
    /// Strokes the two pages share, inked on one and paper on the other.
    StrokesOtherColour,
    /// Labels the two pages share and letter in the same colour.
    LabelsSameColour,
    /// Labels the two pages share, inked on one and paper on the other.
    LabelsOtherColour,
    /// The characters of the labels both pages letter in ink, as the
    /// reference letters them.
    GlyphsSameColour,
    /// Colours one page inks in and the other does not.
    InkColoursApart,
    /// Inked strokes along the sheet on the reference.
    SheetEdges,
    /// Strokes along the sheet one page lacks or the two print at different
    /// widths.
    SheetEdgesApart,
    /// Our ink on a reference stroke that ends or turns its corners another
    /// way than ours, millimetres.
    EndsOther,
    /// Places where the reference paints a stroke in the paper's colour on
    /// ink.
    Wipes,
    /// Of those, the ones our page paints in the reference's order.
    WipesSame,
    /// Of those, the ones our page paints in the other order.
    WipesOther,
}

/// Which way a reading gets better.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Better {
    /// More is better.
    Up,
    /// Less is better.
    Down,
    /// Nearer zero is better.
    TowardZero,
    /// Neither: the number is the page's own, and a change is a change.
    Neither,
}

impl Field {
    const ALL: [Self; 29] = [
        Self::PageWidthApart,
        Self::PageHeightApart,
        Self::MaskedCoverage,
        Self::MaskedUncovered,
        Self::MaskedExtra,
        Self::MaskedLabels,
        Self::MaskedLabelsPaired,
        Self::MaskedLabelsAlone,
        Self::WholeCoverage,
        Self::WholeUncovered,
        Self::WholeExtra,
        Self::WholeLabels,
        Self::WholeLabelsPaired,
        Self::WholeLabelsAlone,
        Self::PenAgreed,
        Self::PenDiffered,
        Self::PenUnpaired,
        Self::StrokesSameColour,
        Self::StrokesOtherColour,
        Self::LabelsSameColour,
        Self::LabelsOtherColour,
        Self::GlyphsSameColour,
        Self::InkColoursApart,
        Self::SheetEdges,
        Self::SheetEdgesApart,
        Self::EndsOther,
        Self::Wipes,
        Self::WipesSame,
        Self::WipesOther,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::PageWidthApart => "page width apart (mm)",
            Self::PageHeightApart => "page height apart (mm)",
            Self::MaskedCoverage => "masked coverage",
            Self::MaskedUncovered => "masked uncovered (mm)",
            Self::MaskedExtra => "masked extra (mm)",
            Self::MaskedLabels => "masked reference labels",
            Self::MaskedLabelsPaired => "masked labels paired",
            Self::MaskedLabelsAlone => "masked runs of ours with no partner",
            Self::WholeCoverage => "whole-page coverage",
            Self::WholeUncovered => "whole-page uncovered (mm)",
            Self::WholeExtra => "whole-page extra (mm)",
            Self::WholeLabels => "whole-page reference labels",
            Self::WholeLabelsPaired => "whole-page labels paired",
            Self::WholeLabelsAlone => "whole-page runs of ours with no partner",
            Self::PenAgreed => "pen agreed",
            Self::PenDiffered => "pen differed",
            Self::PenUnpaired => "pen unpaired",
            Self::StrokesSameColour => "strokes of the same colour",
            Self::StrokesOtherColour => "strokes of another colour",
            Self::LabelsSameColour => "labels of the same colour",
            Self::LabelsOtherColour => "labels of another colour",
            Self::GlyphsSameColour => "characters lettered in ink on both pages",
            Self::InkColoursApart => "ink colours apart",
            Self::SheetEdges => "reference sheet edges",
            Self::SheetEdgesApart => "sheet edges apart",
            Self::EndsOther => "our ink under other ends than the reference's (mm)",
            Self::Wipes => "places the reference strokes in the paper's colour on ink",
            Self::WipesSame => "such places painted in the reference's order",
            Self::WipesOther => "such places painted in the other order",
        }
    }

    fn better(self) -> Better {
        match self {
            Self::PageWidthApart | Self::PageHeightApart => Better::TowardZero,
            Self::MaskedCoverage
            | Self::WholeCoverage
            | Self::MaskedLabelsPaired
            | Self::WholeLabelsPaired
            | Self::PenAgreed
            | Self::StrokesSameColour
            | Self::LabelsSameColour
            | Self::GlyphsSameColour
            | Self::WipesSame => Better::Up,
            Self::MaskedUncovered
            | Self::MaskedExtra
            | Self::MaskedLabelsAlone
            | Self::WholeUncovered
            | Self::WholeExtra
            | Self::WholeLabelsAlone
            | Self::PenDiffered
            | Self::PenUnpaired
            | Self::StrokesOtherColour
            | Self::LabelsOtherColour
            | Self::InkColoursApart
            | Self::SheetEdgesApart
            | Self::EndsOther
            | Self::WipesOther => Better::Down,
            Self::MaskedLabels | Self::WholeLabels | Self::SheetEdges | Self::Wipes => {
                Better::Neither
            }
        }
    }

    /// How far a reading may sit from its pin and still be the pin: half of
    /// the last place a pin is stated to, and for the page the 0.002 mm the
    /// reference states its own paper to (an A1 sheet's `MediaBox` is
    /// 840.9986 × 593.9991 mm).
    fn slack(self) -> f64 {
        match self {
            Self::PageWidthApart | Self::PageHeightApart => 0.002,
            Self::MaskedCoverage | Self::WholeCoverage => 0.000_000_5,
            Self::MaskedUncovered
            | Self::MaskedExtra
            | Self::WholeUncovered
            | Self::WholeExtra
            | Self::EndsOther => 0.000_5,
            _ => 0.0,
        }
    }

    fn show(self, value: f64) -> String {
        // A length that sums to nothing can come out as `-0.0`.
        let value = if value == 0.0 { 0.0 } else { value };
        match self {
            Self::MaskedCoverage | Self::WholeCoverage => format!("{value:.6}"),
            Self::PageWidthApart
            | Self::PageHeightApart
            | Self::MaskedUncovered
            | Self::MaskedExtra
            | Self::WholeUncovered
            | Self::WholeExtra
            | Self::EndsOther => format!("{value:.3}"),
            _ => format!("{value:.0}"),
        }
    }
}

/// Why a reading is not what a page that prints as its reference reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cause {
    /// A defect of ours whose fix is known and not landed.
    ///
    /// No line of the table is of this cause since the A2 sheet's page
    /// became its paper (2026-11-11); the cause stays for the next one.
    #[allow(dead_code)]
    NotFixed,
    /// The difference is in the source data: the reference was plotted from
    /// a state of the drawing we do not hold, and the page can only read as
    /// its reference does once the site supplies it.
    SiteData,
    /// Which of the two pages is right is not settled: the data on hand
    /// explains neither, and the site has been asked.
    Undetermined,
}

impl Cause {
    fn name(self) -> &'static str {
        match self {
            Self::NotFixed => "not fixed yet",
            Self::SiteData => "needs data from the site",
            Self::Undetermined => "not settled",
        }
    }
}

/// One reading a page is known to miss, pinned where it reads.
struct Defect {
    field: Field,
    /// What the field reads today.
    reads: f64,
    cause: Cause,
    /// What it is.
    what: &'static str,
}

/// One reference sheet: what is its own, and what it is known to miss.
struct Sheet {
    name: &'static str,
    /// Labels the reference letters outside the masks, then on the whole
    /// page.
    labels: [usize; 2],
    /// Inked strokes of ours the reference prints with the same pen.
    pen_agreed: usize,
    /// Strokes, then labels, the two pages share and print in the same
    /// colour, then the characters of the labels both letter in ink.
    same_colour: [usize; 3],
    /// Inked strokes along the sheet on the reference.
    sheet_edges: usize,
    /// Places where the reference paints a stroke in the paper's colour on
    /// ink.
    wipes: usize,
    defects: &'static [Defect],
}

impl Sheet {
    /// What a page that prints as its reference reads for `field`.
    fn ideal(&self, field: Field) -> f64 {
        match field {
            Field::MaskedCoverage | Field::WholeCoverage => 1.0,
            Field::MaskedLabels | Field::MaskedLabelsPaired => self.labels[0] as f64,
            Field::WholeLabels | Field::WholeLabelsPaired => self.labels[1] as f64,
            Field::PenAgreed => self.pen_agreed as f64,
            Field::StrokesSameColour => self.same_colour[0] as f64,
            Field::LabelsSameColour => self.same_colour[1] as f64,
            Field::GlyphsSameColour => self.same_colour[2] as f64,
            Field::SheetEdges => self.sheet_edges as f64,
            Field::Wipes | Field::WipesSame => self.wipes as f64,
            Field::PageWidthApart
            | Field::PageHeightApart
            | Field::MaskedUncovered
            | Field::MaskedExtra
            | Field::MaskedLabelsAlone
            | Field::WholeUncovered
            | Field::WholeExtra
            | Field::WholeLabelsAlone
            | Field::PenDiffered
            | Field::PenUnpaired
            | Field::StrokesOtherColour
            | Field::LabelsOtherColour
            | Field::InkColoursApart
            | Field::SheetEdgesApart
            | Field::EndsOther
            | Field::WipesOther => 0.0,
        }
    }

    fn defect(&self, field: Field) -> Option<&Defect> {
        self.defects.iter().find(|defect| defect.field == field)
    }

    /// What `field` is pinned at: its defect line when it has one.
    fn pinned(&self, field: Field) -> f64 {
        self.defect(field)
            .map_or_else(|| self.ideal(field), |defect| defect.reads)
    }
}

/// The border two reference plots do not print.
///
/// These are also the two plots that do not paint in the order the sheet on
/// hand lists its objects in: 459 of 1 603 painted elements and 499 of 2 135
/// are in that order, where the other nine plots read 98.3 to 99.3 %
/// (`docs/analysis/2026-11-11-a-sheet-draws-its-children-in-the-order-it-lists-them.md`).
const BARE_BORDER: &str = "our page draws the `CPE-SQ-A1.igr` border its drawing links and the \
                           reference plot carries none. The plot also paints its objects in an \
                           order the sheet we hold does not list them in, as no other of the \
                           eleven does: it was made from another state of the drawing or in \
                           another way than the rest, and nothing on hand says which, nor \
                           whether the drawing had its frame then";

/// The one mark `一级闪蒸分离器A`'s plot paints on the other side of its ink.
///
/// Whether a mark comes before the ink it lies on is not a rule of its own:
/// of the 90 places on the eleven sheets where both sides are ours, the
/// reference paints the ink first at 60 and the mark first at 30, and the
/// sheet's list puts 89 in the reference's order
/// (`probe_which_of_a_mark_and_its_ink_a_plot_paints_first`). This place is
/// the ninetieth, and
/// `a_label_and_its_origin_mark_are_painted_in_the_order_the_sheet_lists_them`
/// holds what is said of it here.
const OTHER_ORDER: &str = "the origin mark of the label `P70` at (339.85, 384.39) mm: the \
                           reference letters the `P` after the mark and our page before it. Our \
                           page follows the sheet, which lists the text ahead of its symbol \
                           (places 99 and 104). Of the 96 labels with an origin mark on the \
                           eleven sheets the plots paint 95 in the order their sheet lists them, \
                           49 symbol first and 46 text first; this is the other one. No rule over \
                           the records gives both: `一级闪蒸分离器B` holds the same label, at the \
                           same point and the same two places, and its plot letters the `P` \
                           first, as listed. So this plot was not painted from the list the file \
                           holds — it is one of the two that do not paint in the order the sheet \
                           on hand lists its objects in (see the border they do not print). The \
                           file was saved on 2026-07-06 and the plot made on 2026-10-12; whether \
                           the drawing changed in between or the plot was made another way, \
                           nothing on hand says";

/// The A2 sheet's instrument loop and its two pipeline strings.
const LATER_REVISION: &str = "the temperature loop TT/TICA-0114301 and two pipeline strings: the \
                              reference was plotted from a later state of the drawing than \
                              either copy of the `.pid`, the published XML and the backup's \
                              database, which all agree with what we draw";

const SHEETS: [Sheet; 11] = [
    Sheet {
        name: "I井场 注采阀组工艺及自控流程图",
        labels: [546, 583],
        pen_agreed: 9334,
        same_colour: [9370, 583, 3520],
        sheet_edges: 8,
        wipes: 20,
        defects: &[],
    },
    Sheet {
        name: "J K井场 注采阀组工艺及自控流程图",
        labels: [775, 813],
        pen_agreed: 14661,
        same_colour: [14709, 813, 4922],
        sheet_edges: 8,
        wipes: 20,
        defects: &[],
    },
    Sheet {
        name: "L M井场 注采阀组工艺及自控流程图",
        labels: [921, 959],
        pen_agreed: 17812,
        same_colour: [17866, 959, 5846],
        sheet_edges: 8,
        wipes: 20,
        defects: &[],
    },
    Sheet {
        name: "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
        labels: [151, 155],
        pen_agreed: 1272,
        same_colour: [1278, 155, 1279],
        sheet_edges: 0,
        wipes: 3,
        defects: &[
            // 3478.925 until the page was painted in the sheet's own order
            // (2026-11-11). The ink is the same; the comparison fits the two
            // pages on the strokes in the order it reads them, the fit moved
            // by 1.0e-5 in scale and 0.009 mm, and 0.036 mm of the border
            // came to lie on reference ink.
            Defect {
                field: Field::WholeExtra,
                reads: 3478.889,
                cause: Cause::Undetermined,
                what: BARE_BORDER,
            },
            Defect {
                field: Field::WholeLabelsAlone,
                reads: 34.0,
                cause: Cause::Undetermined,
                what: BARE_BORDER,
            },
            Defect {
                field: Field::PenUnpaired,
                reads: 286.0,
                cause: Cause::Undetermined,
                what: BARE_BORDER,
            },
            Defect {
                field: Field::SheetEdgesApart,
                reads: 8.0,
                cause: Cause::Undetermined,
                what: BARE_BORDER,
            },
            Defect {
                field: Field::WipesSame,
                reads: 2.0,
                cause: Cause::Undetermined,
                what: OTHER_ORDER,
            },
            Defect {
                field: Field::WipesOther,
                reads: 1.0,
                cause: Cause::Undetermined,
                what: OTHER_ORDER,
            },
        ],
    },
    Sheet {
        name: "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
        labels: [123, 161],
        pen_agreed: 1419,
        same_colour: [1425, 161, 1136],
        sheet_edges: 8,
        wipes: 3,
        defects: &[],
    },
    Sheet {
        name: "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
        labels: [167, 171],
        pen_agreed: 1691,
        same_colour: [1695, 171, 1189],
        sheet_edges: 0,
        wipes: 2,
        defects: &[
            Defect {
                field: Field::WholeExtra,
                reads: 3478.683,
                cause: Cause::Undetermined,
                what: BARE_BORDER,
            },
            Defect {
                field: Field::WholeLabelsAlone,
                reads: 34.0,
                cause: Cause::Undetermined,
                what: BARE_BORDER,
            },
            Defect {
                field: Field::PenUnpaired,
                reads: 286.0,
                cause: Cause::Undetermined,
                what: BARE_BORDER,
            },
            Defect {
                field: Field::SheetEdgesApart,
                reads: 8.0,
                cause: Cause::Undetermined,
                what: BARE_BORDER,
            },
        ],
    },
    Sheet {
        name: "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
        labels: [249, 253],
        pen_agreed: 3192,
        same_colour: [3196, 253, 1672],
        sheet_edges: 0,
        wipes: 2,
        defects: &[],
    },
    Sheet {
        name: "集注站 烃水露点控制单元（一） 空冷器A集预冷分离器A工艺及自控流程图",
        labels: [288, 326],
        pen_agreed: 3394,
        same_colour: [3404, 326, 2144],
        sheet_edges: 8,
        wipes: 6,
        defects: &[],
    },
    Sheet {
        name: "集注站 烃水露点控制单元（二） 空冷器B集预冷分离器B工艺及自控流程图",
        labels: [290, 328],
        pen_agreed: 3401,
        same_colour: [3411, 328, 2172],
        sheet_edges: 8,
        wipes: 6,
        defects: &[],
    },
    Sheet {
        name: "集注站 烃水露点控制单元（三） 空冷器C集预冷分离器C工艺及自控流程图",
        labels: [283, 321],
        pen_agreed: 3268,
        same_colour: [3278, 321, 2127],
        sheet_edges: 8,
        wipes: 6,
        defects: &[],
    },
    Sheet {
        name: "集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图",
        labels: [225, 261],
        pen_agreed: 2802,
        same_colour: [2806, 250, 1321],
        sheet_edges: 6,
        wipes: 3,
        // Three lines left this table when the page became the paper the
        // sheet states (2026-11-11): the page was 0.329 mm wider and
        // 0.566 mm taller than the reference's, and it stroked three sheet
        // edges the reference does not — the top edge of the template's
        // outline and its right edge twice, all of them off the 594 ×
        // 420 mm paper.
        defects: &[
            Defect {
                field: Field::MaskedCoverage,
                reads: 0.983088,
                cause: Cause::SiteData,
                what: LATER_REVISION,
            },
            Defect {
                field: Field::MaskedUncovered,
                reads: 206.766,
                cause: Cause::SiteData,
                what: LATER_REVISION,
            },
            Defect {
                field: Field::MaskedExtra,
                reads: 186.595,
                cause: Cause::SiteData,
                what: LATER_REVISION,
            },
            Defect {
                field: Field::MaskedLabelsPaired,
                reads: 214.0,
                cause: Cause::SiteData,
                what: LATER_REVISION,
            },
            Defect {
                field: Field::MaskedLabelsAlone,
                reads: 11.0,
                cause: Cause::SiteData,
                what: LATER_REVISION,
            },
            Defect {
                field: Field::WholeCoverage,
                reads: 0.988264,
                cause: Cause::SiteData,
                what: LATER_REVISION,
            },
            Defect {
                field: Field::WholeUncovered,
                reads: 206.935,
                cause: Cause::SiteData,
                what: "the later revision's loop, and 0.169 mm that is the stub the plotter \
                       adds to the left sheet edge, which the reader leaves out with the edge \
                       on our page",
            },
            Defect {
                field: Field::WholeExtra,
                reads: 186.595,
                cause: Cause::SiteData,
                what: LATER_REVISION,
            },
            Defect {
                field: Field::WholeLabelsPaired,
                reads: 250.0,
                cause: Cause::SiteData,
                what: LATER_REVISION,
            },
            Defect {
                field: Field::WholeLabelsAlone,
                reads: 11.0,
                cause: Cause::SiteData,
                what: LATER_REVISION,
            },
            Defect {
                field: Field::PenUnpaired,
                reads: 100.0,
                cause: Cause::SiteData,
                what: LATER_REVISION,
            },
            Defect {
                field: Field::WipesSame,
                reads: 2.0,
                cause: Cause::SiteData,
                what: "the origin mark of the `SP=` label in the instrument cluster the later \
                       revision moved: the reference paints it at (506.14, 237.91) mm, and the \
                       drawing on hand holds that label, mark and all, at (508.24, 254.72)",
            },
        ],
    },
];

/// Font directories for this corpus, plus the self-check corpus's own set,
/// as `reference_plots_acceptance` searches them.
fn font_search_dirs(corpus: &std::path::Path) -> Vec<PathBuf> {
    let mut dirs = font_dirs(Some(corpus));
    let beside = PathBuf::from("D:/work/cad/PID自研软件验证/fonts");
    if beside.is_dir() {
        dirs.push(beside);
    }
    dirs
}

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    )
}

fn uncovered_mm(report: &CompareReport) -> f64 {
    report.reference_length_mm - report.covered_length_mm
}

/// What one page reads: a value for each field, in [`Field::ALL`]'s order,
/// and what stands behind the colour, edge and end readings, for the table.
struct PageReading {
    values: Vec<f64>,
    notes: Vec<String>,
}

fn read_page(
    sheet: &Sheet,
    model: &PaperModel,
    reference_pdf: &[u8],
    our_pdf: &[u8],
) -> PageReading {
    let name = sheet.name;
    let reference = PageInk::read(reference_pdf).unwrap_or_else(|e| panic!("{name}: {e}"));
    let ours = PageInk::read(our_pdf).unwrap_or_else(|e| panic!("{name}: ours: {e}"));
    let template = template_masks(reference.ink.page_mm);
    let missing: Vec<MaskRect> = model
        .missing_boxes
        .iter()
        .map(|missing| MaskRect {
            label: missing.path.clone(),
            min_mm: missing.min_mm,
            max_mm: missing.max_mm,
        })
        .collect();
    let masked = compare_plots_with_model(&reference.ink, &ours.ink, &template, &missing, model);
    let whole = compare_plots_with_model(&reference.ink, &ours.ink, &[], &missing, model);
    assert!(
        whole.fit.within_threshold,
        "{name}: fit outside threshold: {:?}",
        whole.fit
    );
    let strokes = stroke_colours(&reference, &ours, &whole);
    let labels = label_colours(&reference, &ours, &whole);
    let edges = sheet_edges(&reference.pens, &ours.pens, &whole.fit);
    let ends = ends_apart(&reference.pens, &ours.pens, &whole.fit);
    let wipes = wipe_order(
        &read_plot_sequence(reference_pdf).unwrap_or_else(|e| panic!("{name}: {e}")),
        &read_plot_sequence(our_pdf).unwrap_or_else(|e| panic!("{name}: ours: {e}")),
        &whole.fit,
    );
    let colours_apart = reference
        .pens
        .inks
        .iter()
        .filter(|ink| !ours.pens.inks.iter().any(|other| other.rgba == ink.rgba))
        .count()
        + ours
            .pens
            .inks
            .iter()
            .filter(|ink| {
                !reference
                    .pens
                    .inks
                    .iter()
                    .any(|other| other.rgba == ink.rgba)
            })
            .count();
    let mut notes = vec![
        format!(
            "strokes: {} ink on ink, {} paper on paper, {} ink on reference paper, {} paper on \
             reference ink; alone: {} ink, {} paper",
            strokes.ink_on_ink,
            strokes.paper_on_paper,
            strokes.ink_on_paper,
            strokes.paper_on_ink,
            strokes.ink_alone,
            strokes.paper_alone
        ),
        format!(
            "labels: {} ink on ink ({} characters), {} paper on paper, {} ink on reference \
             paper, {} paper on reference ink; runs in the paper's colour: reference {}, ours {}",
            labels.ink_on_ink,
            labels.glyphs_ink_on_ink,
            labels.paper_on_paper,
            labels.ink_on_paper,
            labels.paper_on_ink,
            labels.paper_runs[0],
            labels.paper_runs[1]
        ),
    ];
    for (side, pens) in [("reference", &reference.pens), ("ours", &ours.pens)] {
        let inks: Vec<String> = pens
            .inks
            .iter()
            .map(|ink| {
                format!(
                    "{:?} on {} path(s) and {} glyph(s)",
                    ink.rgba, ink.paths, ink.glyphs
                )
            })
            .collect();
        let ends: Vec<String> = pens
            .ends
            .iter()
            .map(|ends| {
                format!(
                    "{:?}/{:?} {} path(s) {:.3} mm",
                    ends.cap, ends.join, ends.paths, ends.length_mm
                )
            })
            .collect();
        notes.push(format!(
            "{side}: inks {}; ends {}",
            inks.join(", "),
            ends.join(", ")
        ));
    }
    let pairs: Vec<String> = ends
        .apart
        .iter()
        .map(|pair| {
            format!(
                "{:?}/{:?} on the reference's {:?}/{:?} {:.3} mm",
                pair.ours.0, pair.ours.1, pair.reference.0, pair.reference.1, pair.length_mm
            )
        })
        .collect();
    notes.push(format!(
        "ends, our ink by the reference's stroke at its place: {:.3} mm the same cap and join, \
         {:.3} mm another ({}), {:.3} mm with no reference stroke",
        ends.same_mm,
        ends.other_mm,
        if pairs.is_empty() {
            "none".to_string()
        } else {
            pairs.join(", ")
        },
        ends.alone_mm
    ));
    notes.push(format!(
        "order: the reference strokes in the paper's colour on ink at {} place(s), the ink \
         painted first at {}; ours paints {} in that order, {} in the other, and lacks the \
         stroke or the ink at {}",
        wipes.sites, wipes.ink_first, wipes.same, wipes.other, wipes.absent
    ));
    for place in &wipes.apart {
        let ink = place.glyph.as_ref().map_or_else(
            || {
                format!(
                    "a stroke at ({:.2}, {:.2})",
                    place.ink_mm[0], place.ink_mm[1]
                )
            },
            |glyph| {
                format!(
                    "the character {glyph:?} at ({:.2}, {:.2})",
                    place.ink_mm[0], place.ink_mm[1]
                )
            },
        );
        notes.push(format!(
            "order apart: the stroke in the paper's colour at ({:.2}, {:.2}) lies on {ink}, which \
             the reference paints {} it; ours: {:?}",
            place.mark_mm[0],
            place.mark_mm[1],
            if place.ink_first { "before" } else { "after" },
            place.ours
        ));
    }
    notes.push(format!(
        "sheet edges: reference {}, ours {}, {} the same",
        edges.reference, edges.ours, edges.same
    ));
    let place = |edge: &SheetEdge| {
        format!(
            "({:.3}, {:.3})–({:.3}, {:.3}) {:.3} mm wide",
            edge.start_mm[0], edge.start_mm[1], edge.end_mm[0], edge.end_mm[1], edge.width_mm
        )
    };
    for pair in &edges.other_width {
        notes.push(format!(
            "sheet edge at another width: ours {}, the reference's {:.3} mm",
            place(&pair.ours),
            pair.reference_width_mm
        ));
    }
    for edge in &edges.ours_only {
        notes.push(format!("sheet edge of ours alone: {}", place(edge)));
    }
    for edge in &edges.reference_only {
        notes.push(format!(
            "sheet edge of the reference's alone: {}",
            place(edge)
        ));
    }
    let values = Field::ALL
        .iter()
        .map(|field| match field {
            Field::PageWidthApart => ours.ink.page_mm[0] - reference.ink.page_mm[0],
            Field::PageHeightApart => ours.ink.page_mm[1] - reference.ink.page_mm[1],
            Field::MaskedCoverage => masked.coverage,
            Field::MaskedUncovered => uncovered_mm(&masked),
            Field::MaskedExtra => masked.extra_length_mm,
            Field::MaskedLabels => masked.text_reference as f64,
            Field::MaskedLabelsAlone => masked.unmatched_our_text.len() as f64,
            Field::MaskedLabelsPaired => masked.text_matched as f64,
            Field::WholeCoverage => whole.coverage,
            Field::WholeUncovered => uncovered_mm(&whole),
            Field::WholeExtra => whole.extra_length_mm,
            Field::WholeLabels => whole.text_reference as f64,
            Field::WholeLabelsAlone => whole.unmatched_our_text.len() as f64,
            Field::WholeLabelsPaired => whole.text_matched as f64,
            Field::PenAgreed => whole.pen.agreed as f64,
            Field::PenDiffered => whole.pen.differed as f64,
            Field::PenUnpaired => whole.pen.unpaired as f64,
            Field::StrokesSameColour => strokes.same() as f64,
            Field::StrokesOtherColour => strokes.other() as f64,
            Field::LabelsSameColour => labels.same() as f64,
            Field::LabelsOtherColour => labels.other() as f64,
            Field::GlyphsSameColour => labels.glyphs_ink_on_ink as f64,
            Field::InkColoursApart => colours_apart as f64,
            Field::SheetEdges => edges.reference as f64,
            Field::SheetEdgesApart => edges.apart() as f64,
            Field::EndsOther => ends.other_mm,
            Field::Wipes => wipes.sites as f64,
            Field::WipesSame => wipes.same as f64,
            Field::WipesOther => wipes.other as f64,
        })
        .collect();
    PageReading { values, notes }
}

/// Hold one page's readings to its pins; every reading that is not its pin
/// is one line of the result.
fn hold(sheet: &Sheet, readings: &[f64]) -> Vec<String> {
    let mut lines = Vec::new();
    for (field, reads) in Field::ALL.iter().zip(readings) {
        let pinned = sheet.pinned(*field);
        let ideal = sheet.ideal(*field);
        if let Some(defect) = sheet.defect(*field) {
            assert!(
                (defect.reads - ideal).abs() > field.slack(),
                "{}: the defect line for {} is pinned at {}, which is what a page with no \
                 defect reads; take the line out",
                sheet.name,
                field.name(),
                field.show(defect.reads)
            );
        }
        if (reads - pinned).abs() <= field.slack() {
            continue;
        }
        let better = match field.better() {
            Better::Up => Some(*reads > pinned),
            Better::Down => Some(*reads < pinned),
            Better::TowardZero => Some(reads.abs() < pinned.abs()),
            Better::Neither => None,
        };
        let verdict = match (better, sheet.defect(*field)) {
            (Some(true), Some(_)) if (reads - ideal).abs() <= field.slack() => {
                "better, and what a page with no defect reads: take its defect line out".to_string()
            }
            (Some(true), Some(_)) => {
                "better: tighten the defect line to what it reads now".to_string()
            }
            (Some(true), None) => "better: raise the sheet's pin to what it reads now".to_string(),
            (Some(false), Some(defect)) => format!(
                "worse than the known defect ({}: {})",
                defect.cause.name(),
                defect.what
            ),
            (Some(false), None) => "worse: a page with no known defect here".to_string(),
            (None, _) => "moved: this number is the page's own".to_string(),
        };
        lines.push(format!(
            "{}: {} reads {}, pinned {} — {verdict}",
            sheet.name,
            field.name(),
            field.show(*reads),
            field.show(pinned)
        ));
    }
    lines
}

/// One page's readings as the table prints them.
fn table(sheet: &Sheet, reading: &PageReading) -> String {
    let readings = &reading.values;
    let get = |field: Field| {
        let at = Field::ALL
            .iter()
            .position(|other| *other == field)
            .expect("every field is in ALL");
        field.show(readings[at])
    };
    let mut out = String::new();
    let _ = writeln!(out, "== {}", sheet.name);
    let _ = writeln!(
        out,
        "   page apart ({}, {}) mm; ink colours apart {}",
        get(Field::PageWidthApart),
        get(Field::PageHeightApart),
        get(Field::InkColoursApart)
    );
    let _ = writeln!(
        out,
        "   masked  coverage {}  uncovered {} mm  extra {} mm  labels {} paired of {}, {} run(s) \
         of ours alone",
        get(Field::MaskedCoverage),
        get(Field::MaskedUncovered),
        get(Field::MaskedExtra),
        get(Field::MaskedLabelsPaired),
        get(Field::MaskedLabels),
        get(Field::MaskedLabelsAlone)
    );
    let _ = writeln!(
        out,
        "   whole   coverage {}  uncovered {} mm  extra {} mm  labels {} paired of {}, {} run(s) \
         of ours alone",
        get(Field::WholeCoverage),
        get(Field::WholeUncovered),
        get(Field::WholeExtra),
        get(Field::WholeLabelsPaired),
        get(Field::WholeLabels),
        get(Field::WholeLabelsAlone)
    );
    let _ = writeln!(
        out,
        "   pen {} agree / {} differ / {} unpaired; colour: strokes {} same / {} other, labels \
         {} same / {} other",
        get(Field::PenAgreed),
        get(Field::PenDiffered),
        get(Field::PenUnpaired),
        get(Field::StrokesSameColour),
        get(Field::StrokesOtherColour),
        get(Field::LabelsSameColour),
        get(Field::LabelsOtherColour)
    );
    let _ = writeln!(
        out,
        "   sheet edges {} on the reference, {} apart; ends: {} mm of our ink under other ends \
         than the reference's stroke there",
        get(Field::SheetEdges),
        get(Field::SheetEdgesApart),
        get(Field::EndsOther)
    );
    let _ = writeln!(
        out,
        "   order: {} place(s) where the reference strokes in the paper's colour on ink, {} \
         painted in its order and {} in the other",
        get(Field::Wipes),
        get(Field::WipesSame),
        get(Field::WipesOther)
    );
    let _ = writeln!(
        out,
        "   pins: labels: [{}, {}], pen_agreed: {}, same_colour: [{}, {}, {}], sheet_edges: {}, \
         wipes: {}",
        get(Field::MaskedLabels),
        get(Field::WholeLabels),
        get(Field::PenAgreed),
        get(Field::StrokesSameColour),
        get(Field::LabelsSameColour),
        get(Field::GlyphsSameColour),
        get(Field::SheetEdges),
        get(Field::Wipes)
    );
    for note in &reading.notes {
        let _ = writeln!(out, "   {note}");
    }
    for defect in sheet.defects {
        let _ = writeln!(
            out,
            "   known defect [{}] {} = {}: {}",
            defect.cause.name(),
            defect.field.name(),
            defect.field.show(defect.reads),
            defect.what
        );
    }
    // Every reading short of what a page with no defect reads, as the line
    // it takes in the sheet's `defects`: what a change to a pin is copied
    // from.
    for (field, reads) in Field::ALL.iter().zip(readings) {
        if field.better() != Better::Neither
            && !matches!(
                field,
                Field::PenAgreed
                    | Field::StrokesSameColour
                    | Field::LabelsSameColour
                    | Field::GlyphsSameColour
            )
            && (reads - sheet.ideal(*field)).abs() > field.slack()
        {
            let _ = writeln!(
                out,
                "   short of a clean page: {} reads {} (Field::{field:?})",
                field.name(),
                field.show(*reads)
            );
        }
    }
    out
}

/// A sheet's pins are its own numbers, its defects each name one field once,
/// and the three causes are all the table knows.
#[test]
fn every_known_defect_is_one_reading_with_a_cause() {
    let mut by_cause = [0usize; 3];
    for sheet in &SHEETS {
        for (index, defect) in sheet.defects.iter().enumerate() {
            assert!(
                !sheet.defects[..index]
                    .iter()
                    .any(|other| other.field == defect.field),
                "{}: two defect lines for {}",
                sheet.name,
                defect.field.name()
            );
            assert!(
                !defect.what.trim().is_empty(),
                "{}: a defect line for {} says nothing",
                sheet.name,
                defect.field.name()
            );
            assert!(
                defect.field.better() != Better::Neither,
                "{}: {} is the page's own number, not a reading to miss",
                sheet.name,
                defect.field.name()
            );
            by_cause[match defect.cause {
                Cause::NotFixed => 0,
                Cause::SiteData => 1,
                Cause::Undetermined => 2,
            }] += 1;
        }
    }
    eprintln!(
        "known defects: {} not fixed yet, {} needing data from the site, {} not settled",
        by_cause[0], by_cause[1], by_cause[2]
    );
}

/// The four labels of the A2 sheet whose record states a box taller than
/// the line it holds: two `低压放空总管` and two `仪表风管线`, boxes 4.034 mm
/// tall where every other lettered boxed record of the eleven sheets states
/// 3.689 mm. The reference letters each of them one device unit higher than
/// our page does.
///
/// One rule letters all 127 lettered boxed records of the eleven sheets
/// where their references do: the line hangs from the top of its box,
/// `baseline = insertion + box height / 2 − ascent`. On a 3.689 mm box that
/// is the middle line the page always lettered it from; on a 4.034 mm box
/// it is 0.173 mm higher, and until the layout read the box's height these
/// four sat 0.122 and 0.198 mm under their reference.
///
/// The rule is applied to the class these 127 are — anchor code `0x48`, one
/// line, FangSong, a box no taller than 4.034 mm — and to no other: what it
/// would do to the taller boxes and the other codes of the plant backup no
/// reference shows
/// (`docs/analysis/2026-11-11-a-boxed-label-hangs-from-the-top-of-its-box.md`).
#[test]
fn the_a2_sheets_tall_boxed_labels_are_lettered_where_the_reference_letters_them() {
    let corpus = corpus_dir();
    let name = "集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图";
    let reference_path = corpus.join(format!("{name}.pdf"));
    if !reference_path.is_file() {
        eprintln!(
            "skipping the boxed labels; {} is absent",
            reference_path.display()
        );
        return;
    }
    let drawing = corpus.join(format!("{name}.pid"));
    let model = lay_out_drawing(&drawing, &corpus)
        .unwrap_or_else(|error| panic!("{name}: layout: {error}"));
    let dirs = font_search_dirs(&corpus);
    let prepared = prepare_fonts(&[&model], &dirs).expect("fonts");
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
            subset: face.subset,
        })
        .collect();
    let reference = PageInk::read(&std::fs::read(&reference_path).expect("the reference"))
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    let our_pdf = write_plot(&model, &fonts).expect("write");
    // The page read here is the page `pid_plot` writes for the drawing.
    let exports = scratch::ScratchDir::new("ink-boxed-labels-pid-plot");
    let exported = exported_page::pid_plot_page(&drawing, &corpus, &dirs, &exports.join("a2.pdf"));
    exported_page::assert_is_the_page_pid_plot_writes(name, "reads", &our_pdf, &exported);
    let ours = PageInk::read(&our_pdf).unwrap_or_else(|error| panic!("{name}: ours: {error}"));
    let whole = compare_plots_with_model(&reference.ink, &ours.ink, &[], &[], &model);
    // How far below the reference's each of the four is lettered.
    let down: Vec<(&str, f64)> = whole
        .text_pairs
        .iter()
        .filter_map(|pair| {
            let text = pair.source.as_ref()?.text.trim();
            ["低压放空总管", "仪表风管线"]
                .into_iter()
                .find(|label| *label == text)
                .map(|label| (label, pair.down_mm))
        })
        .collect();
    assert_eq!(down.len(), 4, "the four boxed labels: {down:?}");
    eprintln!("the four boxed labels, millimetres below the reference: {down:?}");
    let half_a_unit = 25.4 / 150.0 / 2.0;
    let low: Vec<String> = down
        .iter()
        .filter(|(_, down)| down.abs() >= half_a_unit)
        .map(|(label, down)| format!("{label} {down:+.4} mm"))
        .collect();
    assert!(
        low.is_empty(),
        "lettered more than half a device unit ({half_a_unit:.4} mm) below the reference: {}; \
         all four read {down:?}",
        low.join(", ")
    );
}

#[test]
fn eleven_reference_plots_print_the_ink_their_pins_say() {
    let corpus = corpus_dir();
    let first_reference = corpus.join(format!("{}.pdf", SHEETS[0].name));
    if !first_reference.is_file() {
        eprintln!(
            "skipping the ink gate; {} is absent",
            first_reference.display()
        );
        return;
    }
    let models: Vec<PaperModel> = SHEETS
        .iter()
        .map(|sheet| {
            let path = corpus.join(format!("{}.pid", sheet.name));
            lay_out_drawing(&path, &corpus)
                .unwrap_or_else(|error| panic!("{}: layout: {error}", sheet.name))
        })
        .collect();
    let dirs = font_search_dirs(&corpus);

    let mut printed = String::new();
    let mut failures: Vec<String> = Vec::new();
    let exports = scratch::ScratchDir::new("ink-pid-plot");
    for (index, (sheet, model)) in SHEETS.iter().zip(&models).enumerate() {
        let reference_pdf = std::fs::read(corpus.join(format!("{}.pdf", sheet.name)))
            .unwrap_or_else(|error| panic!("{}: reference: {error}", sheet.name));
        // The faces are prepared for this sheet by itself, as `pid_plot` and
        // the acceptance gate prepare them, so the page read here is the
        // page that gate delivers — which is held just below, not taken on
        // trust: prepared for the eleven together the page reads the same
        // ink and is other bytes.
        let prepared = prepare_fonts(&[model], &dirs).expect("fonts");
        let fonts: Vec<PlotFont<'_>> = prepared
            .iter()
            .map(|face| PlotFont {
                key: &face.key,
                bytes: &face.bytes,
                subset: face.subset,
            })
            .collect();
        let our_pdf = write_plot(model, &fonts)
            .unwrap_or_else(|error| panic!("{}: write: {error}", sheet.name));
        // What is read below is the page `pid_plot` writes for this drawing,
        // byte for byte: the page the acceptance gate verifies and delivers.
        let exported = exported_page::pid_plot_page(
            &corpus.join(format!("{}.pid", sheet.name)),
            &corpus,
            &dirs,
            &exports.join(format!("{index}.pdf")),
        );
        exported_page::assert_is_the_page_pid_plot_writes(sheet.name, "reads", &our_pdf, &exported);
        let reading = read_page(sheet, model, &reference_pdf, &our_pdf);
        let page = table(sheet, &reading);
        eprint!("{page}");
        printed.push_str(&page);
        failures.extend(hold(sheet, &reading.values));
    }
    let known: usize = SHEETS.iter().map(|sheet| sheet.defects.len()).sum();
    let summary = format!(
        "ink gate: {} page(s), {} known defect line(s), {} reading(s) off their pins\n",
        SHEETS.len(),
        known,
        failures.len()
    );
    eprint!("{summary}");
    printed.push_str(&summary);
    if let Some(dir) = std::env::var_os("PID_INK_GATE_OUT") {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).expect("create PID_INK_GATE_OUT");
        std::fs::write(dir.join("reference-plots-ink.txt"), printed.as_bytes())
            .expect("write the ink gate's table");
    }
    assert!(
        failures.is_empty(),
        "{} reading(s) are not what they are pinned at:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// What each plot does with the labels whose symbol carries an origin mark,
/// in the order of [`SHEETS`]: how many the sheet has; how many it lists
/// text first, the text's owner ahead of the symbol (for every other one
/// the owner stands at the place right after the symbol's); how many the
/// plot letters ahead of the mark; and how many it paints in the order the
/// sheet lists them.
///
/// 96 labels: 49 listed symbol first and 47 text first, so neither comes
/// first as a rule, and 95 painted as listed. A label's symbol and its text
/// are two records the sheet lists apart — the text is an `igTextBox` whose
/// owner, a `DependencyObject`, names the symbol
/// (`tests/support/marked_labels.rs`) — and the plots draw them where the
/// list puts them, as they do everything else.
const MARKED_LABELS: [[usize; 4]; 11] = [
    [18, 10, 10, 18],
    [24, 13, 13, 24],
    [27, 14, 14, 27],
    [3, 1, 0, 2],
    [3, 1, 1, 3],
    [2, 0, 0, 2],
    [2, 1, 1, 2],
    [5, 2, 2, 5],
    [5, 2, 2, 5],
    [5, 2, 2, 5],
    [2, 1, 1, 2],
];

/// The one label a plot paints against its sheet's list: the sheet, the
/// text, the symbol, and where the sheet lists the symbol and the text.
///
/// This is the label of [`OTHER_ORDER`]. The sheet lists the text first —
/// its owner at place 99, the symbol at place 104 — and the plot strokes the
/// mark and letters the `P` at the very next step, the way it paints a label
/// whose owner is listed right after its symbol.
const AGAINST_ITS_LIST: (&str, &str, &str, [(u32, u32); 2]) = (
    "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
    "P70",
    "Remarks.sym",
    [(104, 0), (99, 1)],
);

/// The sheet that holds the same label: the same text and symbol at the
/// same point of the page, listed at the same two places. Its plot letters
/// the `P` ahead of the mark, as listed.
///
/// So the records do not decide what the plot of [`AGAINST_ITS_LIST`] did:
/// two plots, the same records at the same places, the two orders. (The
/// same but for numbers of each drawing's own: the label's three records
/// differ between the two sheets in their oids, in style ids and in the
/// references of the symbol's cached body, and in nothing else —
/// `tools/display_order/label_records_apart.py`.) No rule our page could
/// follow gives both.
const ITS_TWIN: &str = "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图";

/// A label and its origin mark are painted in the order the sheet lists
/// them: on every label of the eleven sheets but one, and that one has a
/// twin painted the other way.
///
/// This is what the two `WipesSame` / `WipesOther` lines of
/// `一级闪蒸分离器A` rest on. It reads the plots and the drawings and writes
/// no page, so it needs no typeface; like the gate beside it, it passes
/// with a word when the corpus is absent.
#[test]
fn a_label_and_its_origin_mark_are_painted_in_the_order_the_sheet_lists_them() {
    use marked_labels::{MarkedLabel, apart, marked_labels};

    let corpus = corpus_dir();
    let first_reference = corpus.join(format!("{}.pdf", SHEETS[0].name));
    if !first_reference.is_file() {
        eprintln!(
            "skipping the labels and their marks; {} is absent",
            first_reference.display()
        );
        return;
    }
    let mut read: Vec<(&str, Vec<MarkedLabel>)> = Vec::new();
    for sheet in &SHEETS {
        let name = sheet.name;
        let path = corpus.join(format!("{name}.pid"));
        let model = lay_out_drawing(&path, &corpus).unwrap_or_else(|e| panic!("{name}: {e}"));
        let doc = pid_parse::PidParser::new()
            .parse_file(&path)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let plot = std::fs::read(corpus.join(format!("{name}.pdf")))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let plot = read_plot_sequence(&plot).unwrap_or_else(|e| panic!("{name}: {e}"));
        let labels =
            marked_labels(&path, &model, &doc, &plot).unwrap_or_else(|e| panic!("{name}: {e}"));
        read.push((name, labels));
    }

    let mut against: Vec<(&str, &MarkedLabel)> = Vec::new();
    for ((name, labels), pinned) in read.iter().zip(&MARKED_LABELS) {
        let count = |test: fn(&MarkedLabel) -> bool| labels.iter().filter(|l| test(l)).count();
        assert_eq!(
            [
                labels.len(),
                count(MarkedLabel::listed_text_first),
                count(MarkedLabel::painted_text_first),
                count(MarkedLabel::as_listed),
            ],
            *pinned,
            "{name}: labels with a mark, listed text first, lettered ahead of the mark, painted \
             as listed"
        );
        for label in labels {
            // The text's owner stands right after the symbol or ahead of it.
            assert_ne!(
                label.listed_next(),
                label.listed_text_first(),
                "{name}: {label:?}"
            );
            if label.as_listed() {
                // And a plot that follows the list letters the text at the
                // step after the mark exactly when the owner is listed next.
                assert_eq!(
                    label.painted_next(),
                    label.listed_next(),
                    "{name}: {label:?}"
                );
            } else {
                against.push((*name, label));
            }
        }
    }
    let total: usize = read.iter().map(|(_, labels)| labels.len()).sum();
    eprintln!(
        "labels with an origin mark: {total} on {} sheet(s), {} painted as their sheet lists \
         them",
        read.len(),
        total - against.len()
    );

    // The one against its list, and what it is.
    let (sheet, value, symbol, [symbol_rank, text_rank]) = AGAINST_ITS_LIST;
    assert_eq!(
        against.len(),
        1,
        "labels painted against their sheet's list: {against:#?}"
    );
    let (found_on, label) = against[0];
    assert_eq!(
        (
            found_on,
            label.value.as_str(),
            label.symbol.as_str(),
            label.symbol_rank,
            label.text_rank
        ),
        (sheet, value, symbol, symbol_rank, text_rank)
    );
    assert!(label.listed_text_first() && !label.painted_text_first());
    assert!(
        label.painted_next(),
        "the plot letters the text at the step after the mark: {label:?}"
    );

    // Its twin: the same label at the same point and the same places, on a
    // sheet whose plot paints it as listed.
    let twins: Vec<&MarkedLabel> = read
        .iter()
        .filter(|(name, _)| *name == ITS_TWIN)
        .flat_map(|(_, labels)| labels)
        .filter(|twin| {
            twin.value == label.value
                && twin.symbol == label.symbol
                && twin.symbol_rank == label.symbol_rank
                && twin.text_rank == label.text_rank
                && apart(twin.mark_mm, label.mark_mm) <= 0.01
        })
        .collect();
    assert_eq!(twins.len(), 1, "{ITS_TWIN}: {twins:#?}");
    assert!(
        twins[0].as_listed() && twins[0].painted_text_first() && !twins[0].painted_next(),
        "{ITS_TWIN}: {:?}",
        twins[0]
    );
}

/// Where each of the eleven pages draws its symbol placements from, the
/// drawing's own and then its border template's: the body the document
/// caches of the symbol, the `.sym` of the symbol library, nothing.
///
/// A drawing keeps the body of every symbol it places, and a placement is
/// drawn from that copy; the library's file is read only for a placement
/// the document holds no body of. So a page that draws from a library body
/// is drawn in part from a file outside its drawing, whose damage the page
/// has to say (`symbol_file_chain_fault`), and whose revision need not be
/// the one the drawing was saved with. None of the eleven does: all 2 395
/// placements of the drawings' own and all 12 of their templates' are
/// drawn from a cached body, and none is missing one. (The 53 backup pages
/// and the 7 fixtures are held to the same by their ledger baselines,
/// which pin each page's placements by source: 9 225 and 170 of the
/// drawings' own, 64 and 10 of the templates', all cached.)
const PLACEMENTS: [[u64; 6]; 11] = [
    [343, 0, 0, 1, 0, 0],
    [505, 0, 0, 1, 0, 0],
    [592, 0, 0, 1, 0, 0],
    [75, 0, 0, 1, 0, 0],
    [65, 0, 0, 1, 0, 0],
    [85, 0, 0, 1, 0, 0],
    [156, 0, 0, 0, 0, 0],
    [144, 0, 0, 1, 0, 0],
    [145, 0, 0, 1, 0, 0],
    [143, 0, 0, 1, 0, 0],
    [142, 0, 0, 3, 0, 0],
];

/// Every placement of the eleven pages is drawn from a body its document
/// caches: [`PLACEMENTS`], page by page. Soft-skips with a word when the
/// corpus is absent.
#[test]
fn the_eleven_pages_draw_every_placement_from_a_cached_body() {
    use pid_plot::ledger::{ExportLedger, PlacementSource};
    use pid_plot::write::write_ledger;

    let corpus = corpus_dir();
    let first = corpus.join(format!("{}.pid", SHEETS[0].name));
    if !first.is_file() {
        eprintln!(
            "skipping the placements' sources; {} is absent",
            first.display()
        );
        return;
    }
    let mut measured = Vec::new();
    for sheet in &SHEETS {
        let name = sheet.name;
        let path = corpus.join(format!("{name}.pid"));
        let model = lay_out_drawing(&path, &corpus).unwrap_or_else(|e| panic!("{name}: {e}"));
        let write = write_ledger(&model).unwrap_or_else(|e| panic!("{name}: {e}"));
        let account = ExportLedger::assemble(&model, &write);
        let of = |placements: &std::collections::BTreeMap<PlacementSource, u64>| {
            [
                PlacementSource::Cached,
                PlacementSource::Library,
                PlacementSource::Missing,
            ]
            .map(|source| placements.get(&source).copied().unwrap_or(0))
        };
        let [drawing, furniture] =
            [&account.drawing, &account.furniture].map(|domain| of(&domain.bodies.placements));
        measured.push([
            drawing[0],
            drawing[1],
            drawing[2],
            furniture[0],
            furniture[1],
            furniture[2],
        ]);
        assert!(
            model.ledger.symbol_chain_fault_warnings().is_empty(),
            "{name}: {:?}",
            model.ledger.symbol_chain_fault_warnings()
        );
    }
    let total = measured.iter().fold([0u64; 6], |mut sum, page| {
        for (sum, count) in sum.iter_mut().zip(page) {
            *sum += count;
        }
        sum
    });
    eprintln!(
        "placements of the eleven pages: the drawings' own {} cached, {} from the library, {} \
         missing; the templates' {} cached, {} from the library, {} missing",
        total[0], total[1], total[2], total[3], total[4], total[5]
    );
    assert_eq!(
        measured, PLACEMENTS,
        "placements by source, page by page: the drawing's cached / library / missing, then \
         the template's"
    );
}
