//! The write layer's verdict: whether a page primitive reaches the paper, and
//! why not when it does not.
//!
//! The writer used to answer this question inline, in three separate places:
//! `paint_primitive` refused a degenerate fill and an invalid pen, `stroke_runs`
//! refused a segment, and `show_text` refused a label. Those refusals were
//! invisible — a page drew less than the model held and nothing said so.
//!
//! This module holds **one** copy of each rule, and `write.rs` calls it. The
//! predicates and their short-circuit order are copied expression for
//! expression from the code that used to own them, because the order *is* the
//! contract: appendix A.5 lists the eleven reasons in priority order and
//! requirement 5.3 takes the first one that fires. Changing a comparison here
//! changes which reason a primitive is filed under, and changing a threshold
//! changes what the page draws — the 71-page byte comparison is the guardrail
//! on the second of those.
//!
//! # The twelve reasons, in the appendix's order
//!
//! | Reason | Where the predicate lives |
//! |---|---|
//! | `probe_only` | [`primitive_verdict`] step 1 (`is_plotted`) |
//! | `fill_degenerate` | [`fill_fate`] |
//! | `pen_width_invalid` | [`pen_fate`] |
//! | `chain_empty` | [`primitive_verdict`] step 4 (`shape_chain` under 2 points) |
//! | `segment_non_finite` | [`segment_fate`] |
//! | `segment_too_short` | [`segment_fate`] |
//! | `segment_on_sheet_edge` | [`segment_fate`] (furniture exempt) |
//! | `segment_beyond_the_paper` | [`segment_fate`] |
//! | `text_empty` | [`text_fate`] |
//! | `text_placement_non_finite` | [`text_fate`] |
//! | `text_height_invalid` | [`text_fate`] |
//! | `text_whitespace_only` | [`text_fate`] |
//!
//! # Panic-free
//!
//! Every predicate here reads drawing bytes through the model, so the module
//! denies the four ways a count goes wrong silently. There is no indexing, no
//! `unwrap`, no `expect` and no `panic!`; windows are walked with
//! `slice::windows`, and a reason that fires is recorded rather than raised.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use crate::model::{PaperPrimitive, PaperShape, PaperStroke};
use crate::read::sheet_edge_mm;

use super::{DEFAULT_TEXT_MM, STROKE_MM, finite, hypot, is_plotted, pen_width_mm, shape_chain};

/// A segment shorter than this does not print. The same 1e-4 mm
/// `stroke_runs` has always used.
const MIN_SEGMENT_MM: f64 = 1e-4;

/// What happened to one page primitive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimitiveOutcome {
    /// The primitive wrote ink — a path (`S` or `f`) or a run (`BT … ET`).
    OnPage,
    /// The primitive wrote nothing, for this reason. The first reason that
    /// fires in appendix A.5's order.
    Dropped(crate::ledger::DropReason),
}

/// How many segments of one stroke were walked, written, and dropped for each
/// of the three segment reasons.
///
/// A segment is one adjacent vertex pair of the shape's chain, so a stroke with
/// `n` chain points has `n - 1` of them. `total = written + non_finite +
/// too_short + on_sheet_edge + beyond_the_paper`; that is the write layer's
/// conservation equation (requirement 6.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SegmentTally {
    /// Every adjacent pair the shape's chain holds.
    pub total: u64,
    /// Pairs that printed.
    pub written: u64,
    /// Pairs with a non-finite endpoint.
    pub non_finite: u64,
    /// Pairs shorter than [`MIN_SEGMENT_MM`].
    pub too_short: u64,
    /// Non-furniture pairs the reader calls on the sheet edge.
    pub on_sheet_edge: u64,
    /// Pairs the whole pen of which lies past one edge of the paper.
    pub beyond_the_paper: u64,
}

/// The two ways a written run falls short of the width the drawing set.
///
/// Both are partial-on-page marks rather than drops: the label is on the paper,
/// just not all of it at its stated width.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextMarks {
    /// The run was condensed along its own direction so it fitted.
    pub condensed: bool,
    /// The run was condensed to the floor and still lost its tail to the page
    /// clip.
    pub clipped_at_floor: bool,
}

/// One primitive's whole write-layer verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrimitiveFate {
    /// On the page, or dropped for a named reason.
    pub outcome: PrimitiveOutcome,
    /// The segment account. Zero for a fill or a label, which have no segments.
    pub segments: SegmentTally,
    /// The stroke is on the page but at least one of its segments was dropped
    /// (`segments_dropped`, A.6).
    pub segments_dropped: bool,
    /// The condensation marks, for a label that reached the page in a face.
    /// `None` for every other primitive, and for a label no face wrote.
    pub text: Option<TextMarks>,
}

impl PrimitiveFate {
    /// Whether the primitive reached the page.
    #[must_use]
    pub fn is_on_page(&self) -> bool {
        matches!(self.outcome, PrimitiveOutcome::OnPage)
    }

    /// The drop reason, when it did not reach the page.
    #[must_use]
    pub fn reason(&self) -> Option<crate::ledger::DropReason> {
        match self.outcome {
            PrimitiveOutcome::OnPage => None,
            PrimitiveOutcome::Dropped(reason) => Some(reason),
        }
    }
}

/// One primitive where the writer's own observation disagreed with the
/// verdict.
///
/// Requirement 5.6: the set the ledger calls on-page has to equal the set the
/// writer actually wrote a path or a run for. The gate requires this list
/// empty; a non-empty one means the two rules have drifted apart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteMismatch {
    /// Index into `PaperModel::primitives`.
    pub index: usize,
    /// What the verdict said.
    pub verdict: String,
    /// What the writer did.
    pub writer: String,
}

/// The writer's own account of one page.
///
/// `fates` is the same length and order as `PaperModel::primitives`, so
/// `fates[i]` is primitive `i`'s verdict whatever layer it is on — including
/// the `probe_only` primitives `push_layer` never visits, which
/// `write_plot_audited` still judges because the ledger owes every primitive an
/// answer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WriteLedger {
    /// Whether the fates were checked against a real write, so the font marks
    /// (`text_condensed`, `text_clipped_at_floor`) exist. `false` from
    /// `write_ledger`, which reads no face.
    pub fonts_measured: bool,
    /// One verdict per page primitive, in model order.
    pub fates: Vec<PrimitiveFate>,
    /// Primitives where the observation and the verdict disagreed.
    pub mismatches: Vec<WriteMismatch>,
}

impl WriteLedger {
    /// An empty ledger.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The verdict for primitive `index`, if the ledger holds one.
    #[must_use]
    pub fn fate(&self, index: usize) -> Option<&PrimitiveFate> {
        self.fates.get(index)
    }

    /// Record a non-text primitive's observed write against its verdict.
    ///
    /// `wrote_path` is whether the writer pushed an `S` or an `f` for it. A
    /// stroke whose chain is empty still writes its pen state, so the pen state
    /// alone is not evidence of ink — only the path operator is.
    pub fn observe_path(&mut self, index: usize, wrote_path: bool) {
        let Some(fate) = self.fates.get(index) else {
            self.mismatches.push(WriteMismatch {
                index,
                verdict: "no verdict recorded".to_string(),
                writer: format!("wrote path: {wrote_path}"),
            });
            return;
        };
        let said = fate.is_on_page();
        if said != wrote_path {
            self.mismatches.push(WriteMismatch {
                index,
                verdict: describe(fate),
                writer: format!("wrote path: {wrote_path}"),
            });
        }
    }

    /// Record a label's observed write against its verdict.
    ///
    /// `wrote_run` is whether the writer emitted a `BT … ET` block for it. A
    /// label the verdict keeps on the page must produce one; a label it drops
    /// must produce none.
    pub fn observe_run(&mut self, index: usize, wrote_run: bool) {
        let Some(fate) = self.fates.get(index) else {
            self.mismatches.push(WriteMismatch {
                index,
                verdict: "no verdict recorded".to_string(),
                writer: format!("wrote run: {wrote_run}"),
            });
            return;
        };
        let said = fate.is_on_page();
        if said != wrote_run {
            self.mismatches.push(WriteMismatch {
                index,
                verdict: describe(fate),
                writer: format!("wrote run: {wrote_run}"),
            });
        }
    }
}

fn describe(fate: &PrimitiveFate) -> String {
    match fate.outcome {
        PrimitiveOutcome::OnPage => "on page".to_string(),
        PrimitiveOutcome::Dropped(reason) => format!("dropped: {}", reason.code()),
    }
}

/// Whether a `Fill` ring can be painted.
///
/// Copied from `paint_primitive`: a ring needs at least three vertices and
/// every one of them finite. Fewer than three cannot enclose an area, and a
/// non-finite vertex would put a coordinate in the content stream no viewer can
/// read.
pub(crate) fn fill_fate(chain_mm: &[[f64; 2]]) -> Option<crate::ledger::DropReason> {
    if chain_mm.len() < 3 || !chain_mm.iter().all(|point| finite(*point)) {
        Some(crate::ledger::DropReason::FillDegenerate)
    } else {
        None
    }
}

/// Whether a pen width can be used.
///
/// Copied from `paint_primitive`: a width that is not finite or not positive is
/// unusable. The default pen a stroke with no resolved style falls back to is
/// [`STROKE_MM`] and is not this case — it is a real width.
pub(crate) fn pen_fate(width_mm: f64) -> Option<crate::ledger::DropReason> {
    if !width_mm.is_finite() || width_mm <= 0.0 {
        Some(crate::ledger::DropReason::PenWidthInvalid)
    } else {
        None
    }
}

/// Why one segment does not print, if it does not.
///
/// Copied from `stroke_runs`' `prints` expression, in its own order: finite
/// endpoints first, then the 1e-4 mm floor, then the sheet-edge rule for
/// anything that is not furniture, then the paper's own edge.
/// `keep_sheet_edge` is `primitive.furniture` — a border template's outer
/// frame *is* a line along the page edge, so the furniture domain keeps it
/// while the drawing's own frame rail is dropped. `half_pen_mm` is half the
/// width the segment is stroked at ([`beyond_the_paper`]).
pub(crate) fn segment_fate(
    start: [f64; 2],
    end: [f64; 2],
    page_mm: [f64; 2],
    keep_sheet_edge: bool,
    half_pen_mm: f64,
) -> Option<crate::ledger::DropReason> {
    if !finite(start) || !finite(end) {
        Some(crate::ledger::DropReason::SegmentNonFinite)
    } else if hypot(start, end) < MIN_SEGMENT_MM {
        Some(crate::ledger::DropReason::SegmentTooShort)
    } else if !keep_sheet_edge && sheet_edge_mm(start, end, page_mm) {
        Some(crate::ledger::DropReason::SegmentOnSheetEdge)
    } else if beyond_the_paper(start, end, page_mm, half_pen_mm) {
        Some(crate::ledger::DropReason::SegmentBeyondThePaper)
    } else {
        None
    }
}

/// Whether the whole pen of `start` → `end` lies past one edge of the page:
/// both ends at least `half_pen_mm` beyond the same edge.
///
/// Such a segment leaves no ink on the paper, and the reference plots do not
/// print it. The page is the paper the sheet states, and a border frame may
/// put its template past the paper's corner: on the A2 reference sheet the
/// template's outline has its top edge 0.566 mm above the paper and its
/// right edge 0.328 mm to the right of it, and neither inks the plot — the
/// top edge is not in the content stream at all.
///
/// A segment whose centre line is on the paper's edge, or past it by less
/// than half its pen, still prints the part of its pen that is on the
/// paper: the `CPE-SQ` templates' outlines run along the page edge itself
/// and the reference plots show half of them. A segment that crosses an
/// edge is written whole and the page box cuts it, as before.
pub(crate) fn beyond_the_paper(
    start: [f64; 2],
    end: [f64; 2],
    page_mm: [f64; 2],
    half_pen_mm: f64,
) -> bool {
    let reach = if half_pen_mm.is_finite() {
        half_pen_mm.max(0.0)
    } else {
        0.0
    };
    // Left, bottom, right, top: how far each end lies past the edge.
    let past = |point: [f64; 2]| {
        [
            -point[0],
            -point[1],
            point[0] - page_mm[0],
            point[1] - page_mm[1],
        ]
    };
    let (start, end) = (past(start), past(end));
    start
        .iter()
        .zip(&end)
        .any(|(a, b)| *a >= reach && *b >= reach && (*a > 0.0 || *b > 0.0))
}

/// Why one label is not lettered, if it is not.
///
/// Copied from `show_text`'s three early returns, split into the four reasons
/// appendix A.5 names. The order is the appendix's: an empty string first, then
/// a non-finite placement, then an unusable height, then a string that is only
/// whitespace.
///
/// `height_mm` of `None` falls back to [`DEFAULT_TEXT_MM`] and is **not**
/// `text_height_invalid` — the layout fills `Some(0.0)` when neither the record
/// nor its style states a height, and that one is.
pub(crate) fn text_fate(
    value: &str,
    rotation: f64,
    origin_mm: [f64; 2],
    height_mm: Option<f64>,
) -> Option<crate::ledger::DropReason> {
    if value.is_empty() {
        return Some(crate::ledger::DropReason::TextEmpty);
    }
    if !rotation.is_finite() || !finite(origin_mm) {
        return Some(crate::ledger::DropReason::TextPlacementNonFinite);
    }
    let height_mm = height_mm.unwrap_or(DEFAULT_TEXT_MM);
    if !height_mm.is_finite() || height_mm <= 0.0 {
        return Some(crate::ledger::DropReason::TextHeightInvalid);
    }
    // The writer trims the tail first and then asks whether anything is left.
    // A string of spaces reaches `inked == ""`, and `trim_start` on it is still
    // empty, so it letters nothing.
    if value.trim_end().trim_start().is_empty() {
        return Some(crate::ledger::DropReason::TextWhitespaceOnly);
    }
    None
}

/// The verdict for one page primitive.
///
/// The steps are appendix A.5's order, and each is answered by the predicate
/// the writer itself calls, so the two can never disagree about a single
/// primitive:
///
/// 1. a confidence layer the page does not paint (`probe_only`);
/// 2. a `Fill` ring that cannot enclose an area;
/// 3. a label, by its four text rules;
/// 4. any other stroke: its pen, then its chain, then each segment.
///
/// A stroke with at least one printable segment is on the page; when some
/// segments were dropped it carries `segments_dropped`. A stroke with none is
/// dropped for the first reason appendix A.5 lists among its segment reasons —
/// `min()` over the reasons that fired, which is the appendix's row order
/// because [`DropReason`](crate::ledger::DropReason) derives `Ord` from it.
#[must_use]
pub fn primitive_verdict(primitive: &PaperPrimitive, page_mm: [f64; 2]) -> PrimitiveFate {
    if !is_plotted(primitive.confidence) {
        return dropped(crate::ledger::DropReason::ProbeOnly);
    }
    match &primitive.shape {
        PaperShape::Fill { chain_mm, .. } => match fill_fate(chain_mm) {
            Some(reason) => dropped(reason),
            None => PrimitiveFate {
                outcome: PrimitiveOutcome::OnPage,
                segments: SegmentTally::default(),
                segments_dropped: false,
                text: None,
            },
        },
        PaperShape::Text {
            origin_mm,
            height_mm,
            rotation,
            value,
            ..
        } => match text_fate(value, *rotation, *origin_mm, *height_mm) {
            Some(reason) => dropped(reason),
            None => PrimitiveFate {
                outcome: PrimitiveOutcome::OnPage,
                segments: SegmentTally::default(),
                segments_dropped: false,
                // The marks need a face and a page reach; only the writer can
                // fill them, and only when it wrote a run.
                text: None,
            },
        },
        _ => stroke_verdict(primitive, page_mm),
    }
}

/// A primitive that writes nothing, for `reason`.
fn dropped(reason: crate::ledger::DropReason) -> PrimitiveFate {
    PrimitiveFate {
        outcome: PrimitiveOutcome::Dropped(reason),
        segments: SegmentTally::default(),
        segments_dropped: false,
        text: None,
    }
}

/// The stroke path: pen, chain, then each segment.
fn stroke_verdict(primitive: &PaperPrimitive, page_mm: [f64; 2]) -> PrimitiveFate {
    let pen = primitive.stroke.clone().unwrap_or(PaperStroke {
        rgb: [0, 0, 0],
        width_mm: STROKE_MM,
        dash_mm: Vec::new(),
        dash_fit: None,
    });
    if let Some(reason) = pen_fate(pen.width_mm) {
        return dropped(reason);
    }
    let chain = shape_chain(&primitive.shape);
    if chain.len() < 2 {
        return dropped(crate::ledger::DropReason::ChainEmpty);
    }
    let half_pen_mm = pen_width_mm(pen.width_mm) / 2.0;
    let mut tally = SegmentTally::default();
    let mut first_reason: Option<crate::ledger::DropReason> = None;
    for pair in chain.windows(2) {
        let (Some(start), Some(end)) = (pair.first().copied(), pair.get(1).copied()) else {
            continue;
        };
        tally.total = tally.total.saturating_add(1);
        match segment_fate(start, end, page_mm, primitive.furniture, half_pen_mm) {
            None => tally.written = tally.written.saturating_add(1),
            Some(reason) => {
                match reason {
                    crate::ledger::DropReason::SegmentNonFinite => {
                        tally.non_finite = tally.non_finite.saturating_add(1);
                    }
                    crate::ledger::DropReason::SegmentTooShort => {
                        tally.too_short = tally.too_short.saturating_add(1);
                    }
                    crate::ledger::DropReason::SegmentOnSheetEdge => {
                        tally.on_sheet_edge = tally.on_sheet_edge.saturating_add(1);
                    }
                    crate::ledger::DropReason::SegmentBeyondThePaper => {
                        tally.beyond_the_paper = tally.beyond_the_paper.saturating_add(1);
                    }
                    _ => {}
                }
                // The reasons are the appendix's own rows, so the smallest is
                // the first one the appendix lists.
                first_reason = Some(match first_reason {
                    Some(held) => held.min(reason),
                    None => reason,
                });
            }
        }
    }
    match first_reason {
        None => PrimitiveFate {
            outcome: PrimitiveOutcome::OnPage,
            segments: tally,
            segments_dropped: false,
            text: None,
        },
        Some(reason) if tally.written == 0 => PrimitiveFate {
            outcome: PrimitiveOutcome::Dropped(reason),
            segments: tally,
            segments_dropped: false,
            text: None,
        },
        Some(_) => PrimitiveFate {
            outcome: PrimitiveOutcome::OnPage,
            segments: tally,
            segments_dropped: true,
            text: None,
        },
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{
        PrimitiveFate, PrimitiveOutcome, SegmentTally, WriteLedger, beyond_the_paper, fill_fate,
        pen_fate, primitive_verdict, segment_fate, text_fate,
    };
    use crate::ledger::DropReason;
    use crate::model::{PaperPrimitive, PaperShape, PaperStroke};

    /// Half the thin pen, the one a template's outline is stroked with.
    const HALF_PEN: f64 = 0.169 / 2.0;

    fn primitive(shape: PaperShape) -> PaperPrimitive {
        PaperPrimitive {
            confidence: "decoded",
            stream_path: None,
            byte_range: None,
            symbol: None,
            stroke: None,
            shape,
            furniture: false,
        }
    }

    fn pen(width_mm: f64) -> PaperStroke {
        PaperStroke {
            rgb: [0, 0, 0],
            width_mm,
            dash_mm: Vec::new(),
            dash_fit: None,
        }
    }

    fn line(start: [f64; 2], end: [f64; 2]) -> PaperPrimitive {
        let mut primitive = primitive(PaperShape::Line {
            start_mm: start,
            end_mm: end,
        });
        primitive.stroke = Some(pen(0.35));
        primitive
    }

    /// `probe_only` is the first step: a confidence the page does not paint is
    /// dropped before its shape is even looked at.
    #[test]
    fn probe_only_is_dropped_before_any_shape_rule() {
        let mut probe = primitive(PaperShape::Line {
            start_mm: [10.0, 10.0],
            end_mm: [90.0, 10.0],
        });
        probe.confidence = "probe_only";
        let fate = primitive_verdict(&probe, [100.0, 100.0]);
        assert_eq!(
            fate.outcome,
            PrimitiveOutcome::Dropped(DropReason::ProbeOnly)
        );
        // Even a degenerate fill is filed as probe_only, because the layer rule
        // fires first.
        let mut probe_fill = primitive(PaperShape::Fill {
            chain_mm: vec![[0.0, 0.0]],
            rgb: None,
        });
        probe_fill.confidence = "probe_only";
        assert_eq!(
            primitive_verdict(&probe_fill, [100.0, 100.0]).outcome,
            PrimitiveOutcome::Dropped(DropReason::ProbeOnly)
        );
    }

    /// `fill_degenerate` on both sides of its two conditions: fewer than three
    /// vertices, and a non-finite vertex.
    #[test]
    fn fill_degenerate_ring_under_three_points() {
        assert_eq!(
            fill_fate(&[[0.0, 0.0], [10.0, 0.0]]),
            Some(DropReason::FillDegenerate)
        );
        assert_eq!(fill_fate(&[[0.0, 0.0], [10.0, 0.0], [0.0, 10.0]]), None);
        assert_eq!(
            fill_fate(&[[0.0, 0.0], [f64::NAN, 0.0], [0.0, 10.0]]),
            Some(DropReason::FillDegenerate)
        );
        let fate = primitive_verdict(
            &primitive(PaperShape::Fill {
                chain_mm: vec![[0.0, 0.0], [10.0, 0.0]],
                rgb: None,
            }),
            [100.0, 100.0],
        );
        assert_eq!(
            fate.outcome,
            PrimitiveOutcome::Dropped(DropReason::FillDegenerate)
        );
    }

    /// `pen_width_invalid` rejects zero, negative, NaN and infinity, and keeps
    /// every positive finite width.
    #[test]
    fn pen_width_invalid_rejects_zero_and_nan() {
        assert_eq!(pen_fate(0.0), Some(DropReason::PenWidthInvalid));
        assert_eq!(pen_fate(-0.25), Some(DropReason::PenWidthInvalid));
        assert_eq!(pen_fate(f64::NAN), Some(DropReason::PenWidthInvalid));
        assert_eq!(pen_fate(f64::INFINITY), Some(DropReason::PenWidthInvalid));
        assert_eq!(pen_fate(1e-9), None);
        assert_eq!(pen_fate(0.35), None);
        let mut zero = line([10.0, 10.0], [90.0, 10.0]);
        zero.stroke = Some(pen(0.0));
        assert_eq!(
            primitive_verdict(&zero, [100.0, 100.0]).outcome,
            PrimitiveOutcome::Dropped(DropReason::PenWidthInvalid)
        );
    }

    /// `chain_empty` covers a polyline with under two vertices and an arc or
    /// circle whose flattening comes back empty.
    #[test]
    fn chain_empty_under_two_points() {
        let mut polyline = primitive(PaperShape::Polyline {
            points_mm: vec![[10.0, 10.0]],
            closed: false,
        });
        polyline.stroke = Some(pen(0.35));
        assert_eq!(
            primitive_verdict(&polyline, [100.0, 100.0]).outcome,
            PrimitiveOutcome::Dropped(DropReason::ChainEmpty)
        );
        let mut arc = primitive(PaperShape::Arc {
            center_mm: [50.0, 50.0],
            radius_mm: 0.0,
            start_angle: 0.0,
            end_angle: 1.0,
            clockwise: false,
        });
        arc.stroke = Some(pen(0.35));
        assert_eq!(
            primitive_verdict(&arc, [100.0, 100.0]).outcome,
            PrimitiveOutcome::Dropped(DropReason::ChainEmpty)
        );
    }

    /// `segment_non_finite` fires on either endpoint, per segment.
    #[test]
    fn segment_non_finite_on_either_endpoint() {
        let page = [100.0, 100.0];
        assert_eq!(
            segment_fate([f64::NAN, 10.0], [50.0, 10.0], page, false, HALF_PEN),
            Some(DropReason::SegmentNonFinite)
        );
        assert_eq!(
            segment_fate([10.0, 10.0], [50.0, f64::INFINITY], page, false, HALF_PEN),
            Some(DropReason::SegmentNonFinite)
        );
        // Finiteness is checked before the length, so a NaN segment is never
        // also "too short".
        assert_eq!(
            segment_fate([f64::NAN, 10.0], [f64::NAN, 10.0], page, false, HALF_PEN),
            Some(DropReason::SegmentNonFinite)
        );
    }

    /// `segment_too_short` straddles the 1e-4 mm floor.
    #[test]
    fn segment_too_short_at_one_e_minus_four() {
        let page = [100.0, 100.0];
        assert_eq!(
            segment_fate([0.0, 10.0], [0.9e-4, 10.0], page, false, HALF_PEN),
            Some(DropReason::SegmentTooShort)
        );
        assert_eq!(
            segment_fate([0.0, 10.0], [1.1e-4, 10.0], page, false, HALF_PEN),
            None
        );
        // The boundary itself prints: the rule is `>= 1e-4`, not `>`.
        assert_eq!(
            segment_fate([0.0, 10.0], [1e-4, 10.0], page, false, HALF_PEN),
            None
        );
    }

    /// `segment_on_sheet_edge` straddles the reader's 0.8/72 in short side and
    /// its 0.9-of-page long side, on both axes.
    ///
    /// The short side is the *difference between the two endpoints* on the
    /// across axis, not their distance to the page edge: the rule asks whether
    /// the segment itself runs along the frame, which is why a run 0.29 mm
    /// inside the bottom edge still counts while a slightly tilted one does
    /// not.
    #[test]
    fn segment_on_sheet_edge_at_0_2822() {
        let page = [100.0, 50.0];
        let short = 0.8 * 25.4 / 72.0;
        // A long horizontal run: across-axis difference 0, along-axis 92 of
        // the 100 mm page — on the edge.
        assert_eq!(
            segment_fate([4.0, 0.2], [96.0, 0.2], page, false, HALF_PEN),
            Some(DropReason::SegmentOnSheetEdge)
        );
        // Tilted past the short side (0.35 mm across): not the frame.
        assert_eq!(
            segment_fate(
                [4.0, 0.2],
                [96.0, 0.2 + short + 0.07],
                page,
                false,
                HALF_PEN
            ),
            None
        );
        // Just inside the short side: still the frame.
        assert_eq!(
            segment_fate(
                [4.0, 0.2],
                [96.0, 0.2 + short - 0.02],
                page,
                false,
                HALF_PEN
            ),
            Some(DropReason::SegmentOnSheetEdge)
        );
        // A long vertical run 0.2 mm inside the left edge: on the edge. The
        // along axis has to pass 0.9 of the page *height*, 45 mm here.
        assert_eq!(
            segment_fate([0.2, 2.0], [0.2, 48.0], page, false, HALF_PEN),
            Some(DropReason::SegmentOnSheetEdge)
        );
        // A run that is on the short side but not long enough is not the frame.
        assert_eq!(
            segment_fate([4.0, 0.2], [50.0, 0.2], page, false, HALF_PEN),
            None
        );
    }

    /// The furniture domain keeps a line along the page edge — a border
    /// template's outer frame is exactly that line.
    #[test]
    fn furniture_segment_exempt_from_edge_rule() {
        let page = [100.0, 50.0];
        assert_eq!(
            segment_fate([4.0, 0.2], [96.0, 0.2], page, false, HALF_PEN),
            Some(DropReason::SegmentOnSheetEdge)
        );
        assert_eq!(
            segment_fate([4.0, 0.2], [96.0, 0.2], page, true, HALF_PEN),
            None
        );

        let mut furniture = line([4.0, 0.2], [96.0, 0.2]);
        furniture.furniture = true;
        let fate = primitive_verdict(&furniture, page);
        assert_eq!(fate.outcome, PrimitiveOutcome::OnPage);
        assert_eq!(fate.segments.written, 1);
        assert!(!fate.segments_dropped);
    }

    /// `text_empty`, `text_placement_non_finite`, `text_height_invalid` and
    /// `text_whitespace_only` each fire in the appendix's order.
    #[test]
    fn text_rules_fire_in_appendix_order() {
        // text_empty comes before the placement check.
        assert_eq!(
            text_fate("", f64::NAN, [f64::NAN, 0.0], Some(0.0)),
            Some(DropReason::TextEmpty)
        );
        assert_eq!(
            text_fate("A", f64::NAN, [10.0, 10.0], Some(2.5)),
            Some(DropReason::TextPlacementNonFinite)
        );
        assert_eq!(
            text_fate("A", 0.0, [f64::INFINITY, 10.0], Some(2.5)),
            Some(DropReason::TextPlacementNonFinite)
        );
        assert_eq!(
            text_fate("A", 0.0, [10.0, 10.0], Some(0.0)),
            Some(DropReason::TextHeightInvalid)
        );
        assert_eq!(
            text_fate("A", 0.0, [10.0, 10.0], Some(f64::NAN)),
            Some(DropReason::TextHeightInvalid)
        );
        assert_eq!(
            text_fate("   ", 0.0, [10.0, 10.0], Some(2.5)),
            Some(DropReason::TextWhitespaceOnly)
        );
        assert_eq!(text_fate("A", 0.0, [10.0, 10.0], Some(2.5)), None);
    }

    /// A height of `None` falls back to `DEFAULT_TEXT_MM`, so it is not
    /// `text_height_invalid` — only the `Some(0.0)` the layout fills in is.
    #[test]
    fn text_height_none_falls_back_to_default() {
        assert_eq!(text_fate("A", 0.0, [10.0, 10.0], None), None);
        let fate = primitive_verdict(
            &primitive(PaperShape::Text {
                origin_mm: [10.0, 10.0],
                height_mm: None,
                rotation: 0.0,
                value: "A".to_string(),
                font: None,
                anchor: crate::model::TextAnchor::Left,
                line: crate::model::TextLine::Middle,
                height_basis: crate::model::TextHeightBasis::Cap,
                width_permille: None,
                paragraph: None,
                spans: Vec::new(),
            }),
            [100.0, 100.0],
        );
        assert_eq!(fate.outcome, PrimitiveOutcome::OnPage);
    }

    /// A stroke with some segments dropped is on the page and carries
    /// `segments_dropped`; one with none is dropped for the earliest reason.
    #[test]
    fn partially_stroked_line_is_on_page_with_the_dropped_mark() {
        let mut polyline = primitive(PaperShape::Polyline {
            points_mm: vec![
                [10.0, 10.0],
                [50.0, 10.0],
                [50.0 + 0.5e-4, 10.0],
                [90.0, 10.0],
            ],
            closed: false,
        });
        polyline.stroke = Some(pen(0.35));
        let fate = primitive_verdict(&polyline, [100.0, 100.0]);
        assert_eq!(fate.outcome, PrimitiveOutcome::OnPage);
        assert!(fate.segments_dropped);
        assert_eq!(fate.segments.total, 3);
        assert_eq!(fate.segments.written, 2);
        assert_eq!(fate.segments.too_short, 1);

        // Two segments dropped for different reasons: the dropped verdict is
        // the earlier appendix row, and the conservation sum still holds.
        let mut both = primitive(PaperShape::Polyline {
            points_mm: vec![[f64::NAN, 0.0], [0.0, 0.0], [0.5e-4, 0.0]],
            closed: false,
        });
        both.stroke = Some(pen(0.35));
        let fate = primitive_verdict(&both, [100.0, 100.0]);
        assert_eq!(
            fate.outcome,
            PrimitiveOutcome::Dropped(DropReason::SegmentNonFinite)
        );
        let tally: SegmentTally = fate.segments;
        assert_eq!(
            tally.total,
            tally.written + tally.non_finite + tally.too_short + tally.on_sheet_edge
        );
    }

    /// `WriteLedger::observe_*` records a mismatch only when the observation
    /// and the verdict disagree.
    #[test]
    fn ledger_records_a_mismatch_only_on_disagreement() {
        let on_page = primitive_verdict(&line([10.0, 10.0], [90.0, 10.0]), [100.0, 100.0]);
        let mut ledger = WriteLedger {
            fonts_measured: true,
            fates: vec![on_page],
            mismatches: Vec::new(),
        };
        ledger.observe_path(0, true);
        assert!(ledger.mismatches.is_empty(), "{:?}", ledger.mismatches);
        ledger.observe_path(0, false);
        assert_eq!(ledger.mismatches.len(), 1);
        assert_eq!(ledger.mismatches[0].index, 0);
        assert_eq!(ledger.mismatches[0].verdict, "on page");

        // An index with no verdict is itself a mismatch, never a panic.
        ledger.observe_run(7, true);
        assert_eq!(ledger.mismatches.len(), 2);
        assert_eq!(ledger.mismatches[1].index, 7);
    }

    /// Feature: pdf-export-ledger, Property 3: 写出判定取固定优先级中的第一个命中
    ///
    /// The oracle below is written from appendix A.5 on its own, in the
    /// appendix's row order, and does not call any of this module's
    /// predicates. Random shapes, coordinates drawn from NaN, ±∞, zero,
    /// negative and threshold-adjacent values, random confidence and furniture
    /// flags: the verdict must equal the oracle's first hit.
    #[test]
    fn write_verdict_prefers_first_hit() {
        use proptest::prelude::*;

        fn value() -> impl Strategy<Value = f64> {
            prop_oneof![
                Just(f64::NAN),
                Just(f64::INFINITY),
                Just(f64::NEG_INFINITY),
                Just(0.0),
                Just(-0.5),
                Just(1e-4),
                Just(0.9e-4),
                Just(1.1e-4),
                Just(0.2),
                Just(0.282_2),
                Just(0.3),
                Just(50.0),
                Just(95.0),
                Just(100.0),
                (-100.0f64..200.0),
            ]
        }

        fn point() -> impl Strategy<Value = [f64; 2]> {
            (value(), value()).prop_map(|(x, y)| [x, y])
        }

        fn shape() -> impl Strategy<Value = PaperShape> {
            // Arcs and circles are left out on purpose: their vertex chain is
            // the shared `arc_points` flattening, and an oracle that re-derived
            // it would only restate that one function instead of checking the
            // rule this test is about. A `Line` and a `Polyline` reach the same
            // segment loop with chains the oracle can build from their own
            // fields.
            prop_oneof![
                (point(), point())
                    .prop_map(|(start_mm, end_mm)| PaperShape::Line { start_mm, end_mm }),
                (proptest::collection::vec(point(), 0..5), any::<bool>())
                    .prop_map(|(points_mm, closed)| PaperShape::Polyline { points_mm, closed }),
                (proptest::collection::vec(point(), 0..5), any::<bool>()).prop_map(
                    |(chain_mm, _)| PaperShape::Fill {
                        chain_mm,
                        rgb: None
                    }
                ),
                (point(), proptest::option::of(value())).prop_map(|(origin_mm, height_mm)| {
                    PaperShape::Text {
                        origin_mm,
                        height_mm,
                        rotation: 0.0,
                        value: "AB".to_string(),
                        font: None,
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

        /// The oracle: appendix A.5's rows, checked in order, with no call into
        /// the module under test.
        fn oracle(primitive: &PaperPrimitive, page: [f64; 2]) -> Option<DropReason> {
            let plotted = primitive.confidence == "decoded" || primitive.confidence == "inferred";
            if !plotted {
                return Some(DropReason::ProbeOnly);
            }
            match &primitive.shape {
                PaperShape::Fill { chain_mm, .. } => {
                    let ok = chain_mm.len() >= 3
                        && chain_mm
                            .iter()
                            .all(|point| point[0].is_finite() && point[1].is_finite());
                    (!ok).then_some(DropReason::FillDegenerate)
                }
                PaperShape::Text {
                    origin_mm,
                    height_mm,
                    rotation,
                    value,
                    ..
                } => {
                    if value.is_empty() {
                        return Some(DropReason::TextEmpty);
                    }
                    if !rotation.is_finite()
                        || !origin_mm[0].is_finite()
                        || !origin_mm[1].is_finite()
                    {
                        return Some(DropReason::TextPlacementNonFinite);
                    }
                    let height = height_mm.unwrap_or(crate::write::DEFAULT_TEXT_MM);
                    if !height.is_finite() || height <= 0.0 {
                        return Some(DropReason::TextHeightInvalid);
                    }
                    if value.trim_end().trim_start().is_empty() {
                        return Some(DropReason::TextWhitespaceOnly);
                    }
                    None
                }
                _ => {
                    let width = primitive
                        .stroke
                        .as_ref()
                        .map_or(crate::write::STROKE_MM, |pen| pen.width_mm);
                    if !width.is_finite() || width <= 0.0 {
                        return Some(DropReason::PenWidthInvalid);
                    }
                    let chain: Vec<[f64; 2]> = match &primitive.shape {
                        PaperShape::Line { start_mm, end_mm } => vec![*start_mm, *end_mm],
                        PaperShape::Polyline { points_mm, closed } => {
                            let mut points = points_mm.clone();
                            if *closed
                                && points.len() >= 2
                                && let Some(first) = points.first().copied()
                            {
                                points.push(first);
                            }
                            points
                        }
                        PaperShape::Circle {
                            center_mm,
                            radius_mm,
                        } => {
                            let ok = center_mm[0].is_finite()
                                && center_mm[1].is_finite()
                                && radius_mm.is_finite()
                                && *radius_mm > 0.0;
                            if ok {
                                // A real circle flattens to many points; the
                                // oracle only needs to know it is non-empty.
                                vec![[0.0, 0.0], [1.0, 0.0]]
                            } else {
                                Vec::new()
                            }
                        }
                        PaperShape::Arc { .. } => vec![[0.0, 0.0], [1.0, 0.0]],
                        PaperShape::Fill { .. } | PaperShape::Text { .. } => Vec::new(),
                    };
                    if chain.len() < 2 {
                        return Some(DropReason::ChainEmpty);
                    }
                    let mut reasons: Vec<DropReason> = Vec::new();
                    let mut written = 0usize;
                    for pair in chain.windows(2) {
                        let (Some(start), Some(end)) =
                            (pair.first().copied(), pair.get(1).copied())
                        else {
                            continue;
                        };
                        let finite = start[0].is_finite()
                            && start[1].is_finite()
                            && end[0].is_finite()
                            && end[1].is_finite();
                        if !finite {
                            reasons.push(DropReason::SegmentNonFinite);
                            continue;
                        }
                        let dx = start[0] - end[0];
                        let dy = start[1] - end[1];
                        if dx.hypot(dy) < 1e-4 {
                            reasons.push(DropReason::SegmentTooShort);
                            continue;
                        }
                        if !primitive.furniture && crate::read::sheet_edge_mm(start, end, page) {
                            reasons.push(DropReason::SegmentOnSheetEdge);
                            continue;
                        }
                        // Both ends half the stroked pen or more past one
                        // edge of the page.
                        let half = crate::write::pen_width_mm(width) / 2.0;
                        let off = |low: f64, high: f64, side: f64| {
                            (high <= -half && low < 0.0) || (low >= side + half && high > side)
                        };
                        if off(start[0].min(end[0]), start[0].max(end[0]), page[0])
                            || off(start[1].min(end[1]), start[1].max(end[1]), page[1])
                        {
                            reasons.push(DropReason::SegmentBeyondThePaper);
                            continue;
                        }
                        written += 1;
                    }
                    if reasons.is_empty() {
                        None
                    } else if written == 0 {
                        reasons.into_iter().min()
                    } else {
                        None
                    }
                }
            }
        }

        proptest!(ProptestConfig::with_cases(256), |(
            shape in shape(),
            plotted in any::<bool>(),
            furniture in any::<bool>(),
            width in value(),
            page in (value(), value()),
        )| {
            let mut primitive = primitive_with(shape, plotted, furniture);
            primitive.stroke = Some(PaperStroke {
                rgb: [0, 0, 0],
                width_mm: width,
                dash_mm: Vec::new(),
                dash_fit: None,
            });
            let page = [page.0.abs(), page.1.abs()];
            let fate = primitive_verdict(&primitive, page);
            let expected = oracle(&primitive, page);
            prop_assert_eq!(fate.reason(), expected);
            prop_assert_eq!(fate.is_on_page(), expected.is_none());
            let tally = fate.segments;
            prop_assert_eq!(
                tally.total,
                tally.written
                    + tally.non_finite
                    + tally.too_short
                    + tally.on_sheet_edge
                    + tally.beyond_the_paper
            );
        });
    }

    fn primitive_with(shape: PaperShape, plotted: bool, furniture: bool) -> PaperPrimitive {
        PaperPrimitive {
            confidence: if plotted { "decoded" } else { "probe_only" },
            stream_path: None,
            byte_range: None,
            symbol: None,
            stroke: None,
            shape,
            furniture,
        }
    }

    /// The A2 reference sheet in its own numbers. The paper is 594 × 420 mm
    /// and the border frame puts `CPECCHB-SQ-A2.igr` at (0.388, 0.566) mm,
    /// so the template's outline runs from (0.388, 0.566) to
    /// (594.328, 420.566): its bottom and left edges are on the paper and
    /// cross its far edges, its top and right edges are off it. The plot
    /// writes the first two and inks neither of the others.
    #[test]
    fn a_segment_the_whole_pen_of_which_is_off_the_paper_does_not_print() {
        let page = [594.0, 420.0];
        let (low, high) = ([0.388, 0.566], [594.328, 420.566]);
        let beyond = Some(DropReason::SegmentBeyondThePaper);
        let fate = |start, end| segment_fate(start, end, page, true, HALF_PEN);
        // bottom and left: on the paper, running off its far edge
        assert_eq!(fate(low, [high[0], low[1]]), None);
        assert_eq!(fate([low[0], high[1]], low), None);
        // right and top: off it
        assert_eq!(fate([high[0], low[1]], high), beyond);
        assert_eq!(fate(high, [low[0], high[1]]), beyond);
        // A template is not exempt, and neither is the drawing.
        assert_eq!(
            segment_fate([high[0], 10.0], [high[0], 60.0], page, false, HALF_PEN),
            beyond
        );

        // The pen decides, not the centre line: an edge on the paper's own
        // edge prints half its pen, and so does one a hair past it. `A01`'s
        // frame puts its template 0.001 mm off the corner.
        assert_eq!(fate([594.0, 0.0], [594.0, 420.0]), None);
        assert_eq!(fate([594.001, 0.001], [594.001, 420.001]), None);
        assert_eq!(
            fate(
                [594.0 + HALF_PEN - 1e-9, 0.0],
                [594.0 + HALF_PEN - 1e-9, 420.0]
            ),
            None
        );
        assert_eq!(
            fate([594.0 + HALF_PEN, 0.0], [594.0 + HALF_PEN, 420.0]),
            beyond
        );
        // A heavier pen reaches further back onto the paper.
        assert_eq!(
            segment_fate([594.328, 0.0], [594.328, 420.0], page, true, 0.35),
            None
        );

        // Each of the four edges, and only when both ends are past the same
        // one: a stroke round the corner outside the paper is written and
        // left to the page box.
        for (start, end) in [
            ([-1.0, 10.0], [-1.0, 90.0]),
            ([10.0, -1.0], [90.0, -1.0]),
            ([595.0, 10.0], [595.0, 90.0]),
            ([10.0, 421.0], [90.0, 421.0]),
            ([600.0, 430.0], [700.0, 500.0]),
        ] {
            assert_eq!(fate(start, end), beyond, "{start:?} -> {end:?}");
        }
        assert_eq!(fate([595.0, 10.0], [10.0, 421.0]), None);
        assert_eq!(fate([-5.0, 10.0], [10.0, 10.0]), None);

        // No pen at all is the centre line's own answer, and a pen that is
        // not a number reaches nowhere.
        assert!(!beyond_the_paper([594.0, 0.0], [594.0, 420.0], page, 0.0));
        assert!(beyond_the_paper([594.0, 0.0], [594.001, 420.0], page, 0.0));
        assert!(beyond_the_paper(
            [595.0, 0.0],
            [595.0, 420.0],
            page,
            f64::NAN
        ));
    }

    /// The outline as the page holds it: one closed polyline, half of it off
    /// the paper. It is on the page with two pieces dropped; the line that
    /// restates its right edge is dropped whole, for the same reason.
    #[test]
    fn an_outline_half_off_the_paper_keeps_the_half_that_is_on_it() {
        let page = [594.0, 420.0];
        let mut outline = primitive(PaperShape::Polyline {
            points_mm: vec![
                [0.388, 0.566],
                [594.328, 0.566],
                [594.328, 420.566],
                [0.388, 420.566],
                [0.388, 0.566],
            ],
            closed: false,
        });
        outline.stroke = Some(pen(0.25));
        outline.furniture = true;
        let fate = primitive_verdict(&outline, page);
        assert!(fate.is_on_page());
        assert!(fate.segments_dropped);
        assert_eq!(
            fate.segments,
            SegmentTally {
                total: 4,
                written: 2,
                beyond_the_paper: 2,
                ..SegmentTally::default()
            }
        );

        let mut restated = line([594.328, 420.566], [594.328, 0.566]);
        restated.stroke = Some(pen(0.25));
        restated.furniture = true;
        let fate = primitive_verdict(&restated, page);
        assert_eq!(
            fate.outcome,
            PrimitiveOutcome::Dropped(DropReason::SegmentBeyondThePaper)
        );
        assert_eq!(fate.segments.beyond_the_paper, 1);
    }

    /// The fate type answers the two questions a caller asks, and a dropped
    /// one never claims to be on the page.
    #[test]
    fn fate_accessors_agree_with_the_outcome() {
        let on_page: PrimitiveFate =
            primitive_verdict(&line([10.0, 10.0], [90.0, 10.0]), [100.0, 100.0]);
        assert!(on_page.is_on_page());
        assert_eq!(on_page.reason(), None);
        let mut probe = line([10.0, 10.0], [90.0, 10.0]);
        probe.confidence = "probe_only";
        let dropped = primitive_verdict(&probe, [100.0, 100.0]);
        assert!(!dropped.is_on_page());
        assert_eq!(dropped.reason(), Some(DropReason::ProbeOnly));
    }
}
