//! S4 ratchet gate: the three comparison figures on the five corpus drawings
//! may improve (higher coverage, less extra ink, more matched text) and may
//! not regress below the measured baseline.
//!
//! The text figures count labels only: a text run that is only spaces is
//! skipped on both sides, so two blank runs no longer pair as a "match".
//! Matched counts are therefore one to four lower than the numbers this gate
//! held before blanks were excluded — the drop is four phantom pairs, not
//! four lost labels (the I drawing's reference total fell 552 → 546 for the
//! same reason). Later rounds recovered them and then some: all five drawings
//! pair every label they count today — I 546/546, A 151/151, B 123/123,
//! L 167/167, H 249/249 — and every floor is its reading exactly. I's floor
//! came up from 544 once its last two runs, the electric-motor `M`s the
//! baseline note describes, paired.
//!
//! The pen figures are gated the same way: how many of our strokes the
//! reference prints at the width we print them, and how many it does not
//! (`CompareReport::pen`). A page can cover its reference exactly and pair
//! every label while every stroke carries a pen two bands off, which is what
//! five symbol families did until `body_stroke` stopped reading their
//! placeholder style (`docs/analysis/2026-10-30-as-drawn-is-not-a-pen.md`) —
//! 305 of our strokes printed a band heavier than the reference's, and neither
//! the coverage nor the extra-ink figure could see it, because a heavier pen
//! covers the same line. The floors are exact readings too, and every ceiling
//! is **zero**. The 14 strokes they used to hold were not pens we got wrong:
//! each was an edge our decoder cuts into 0.21-0.29 mm chords where the
//! reference's fill path carries one 0.61 mm chord (and one `Diaphragm
//! Actuator` arc chord of 0.22 against 0.85 mm). The pairing's length guard
//! refused a piece that short, so the outline fell back to whatever pen nearby
//! survived — the 0.17 mm hairline the page *does* print along that very edge.
//! `probe_pen_width_sites` names all 14 and the primitive behind each;
//! `probe_does_the_plotter_stroke_fills` shows the reference strokes 730 of its
//! 890 fill outlines, and that 11 782 of its own strokes come back from our page
//! at the same pen. A fill's ring is now compared on locus and rough direction,
//! and a stroke that is a collinear piece of the reference's — or the same pen a
//! sagitta further off — pairs. Neither forgives a different width, so a stroke
//! whose pen really changed still lands in `max_pen_differed` and fails.
//!
//! The pen count only pairs a stroke with ink at the same place, direction and
//! (for a stroke) run, so it is not a second coverage figure: a stroke we never
//! drew is counted unpaired, not wrong. That is what keeps the two gates
//! independent — coverage and extra ink say the ink is in the right place, this
//! one says it is laid with the right pen.

use std::path::PathBuf;

use pid_plot::compare::{
    MaskRect, compare_plots_with_model, template_masks, write_overlay, write_report,
};
use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperModel, lay_out_drawing};
use pid_plot::read::read_plot;
use pid_plot::write::{PlotFont, write_plot};

struct Baseline {
    name: &'static str,
    min_coverage: f64,
    max_extra_mm: f64,
    min_text_matched: usize,
    /// Smallest number of our paired strokes the reference plots at the width
    /// we plot them. A pen regression moves strokes out of this count into
    /// `max_pen_differed`, so the two figures are one measurement stated as a
    /// floor and a ceiling.
    min_pen_agreed: usize,
    /// Largest number of our paired strokes the reference plots at another
    /// width. Zero on every page: a ring edge cut into different chords and a
    /// same-pen stroke a sagitta off both pair, and nothing else is forgiven.
    max_pen_differed: usize,
}

/// Smallest lettering class the offset gate judges, in labels.
///
/// Below this the mean of five symbol labels is noise; at and above it the
/// mean is the class's own rule showing through.
const CLASS_RATCHET_MIN_COUNT: usize = 10;
/// Largest mean down-slope a class may show, in body heights.
///
/// 0.10 of a 2.5 mm body is 0.25 mm — inside the pairing window by design,
/// and the point of the gate. The measured classes sit at 0.03–0.06.
const CLASS_RATCHET_HEIGHTS: f64 = 0.10;
/// Largest mean along-run shift a class may show, in body heights.
///
/// Tighter than the down gate because the along reading has a clean zero to
/// hold: no class steps along the run. Part of every class's figure is a run
/// width difference rather than a placement error — a centred run steps back
/// by half its own width — and the report's width column states that half
/// separately. Before the drawing's padding spaces were dropped from the
/// measured run the I drawing's `Center/Middle` class sat at −0.053 and this
/// gate would have caught it; the measured classes now sit at 0.03–0.04.
const CLASS_RATCHET_ALONG_HEIGHTS: f64 = 0.05;

// Baselines re-locked after styles, complex-dash hydraulic ticks, boundary
// fills, symbol lettering, non-uniform arc/circle scaling and ConsistencyChecks
// review-layer suppression landed, and again after the kind-2 `igTextBox`
// text box started lettering the title-block fields from the box's left edge
// (58 records, 55 of which pair up now — see
// `docs/analysis/2026-10-19-igtextbox-box-is-where-the-text-starts.md`), and
// again after the vertical anchor nibble stopped being read as an index: the
// 18 records whose nibble is 2 letter from the body's top, not its middle
// (`docs/analysis/2026-10-19-a-pair-of-flags-not-an-index.md`), and once more
// after a symbol body's own labels stopped reading `0x10` as the baseline —
// the ten valve `M` labels letter from the body's middle
// (`docs/analysis/2026-10-19-symbol-label-nibble-hangs-from-the-middle.md`),
// and once more when whitespace-only runs stopped counting as labels — see
// `docs/analysis/2026-10-19-blank-runs-are-not-labels.md`.
//
// The last three moves raise the numbers: a label one side split into runs
// now pairs with the other side's whole run
// (`docs/analysis/2026-10-19-split-labels-pair-across-runs.md`), a sheet top
// anchor now steps to the ascender line instead of one body
// (`docs/analysis/2026-10-19-top-anchored-labels-sit-on-the-ascender-line.md`),
// and a padded label right-aligns on its last inked glyph.
//
// The latest move is a padded label no longer measuring its pad at all — a
// leading space pushed every centred run half a space left
// (`docs/analysis/2026-10-20-a-pad-is-not-part-of-the-label.md`). That move
// also cost the L drawing two labels without losing any ink: that drawing's
// two `0103501A` tags sit 13.22 mm apart while the same nine characters ink
// 13.32 mm, so the runs touch and this crate's reader glues them into one
// 18-character span, which the reference keeps as two. Both tags were drawn
// where the reference puts them, so the join is where the two sides are now
// paired — a fused spelling is tried with each member's pad trimmed, since a
// pad is not part of the label (`docs/analysis/2026-10-20-a-field-hangs-from-
// its-box-edge.md`). L counts every reference label it preserves.
//
// All five drawings now pair every label they count, the I drawing's two
// electric-motor `M`s above included, so each floor below is its reading
// exactly: I's came up from 544 when those two paired.
//
// The last two runs in the I drawing to pair were both electric-motor `M`s.
// The reading that pinned I's floor found our body lettering them at 2.04 mm
// from a 2.54 mm style height scaled by the placement's 0.804, against a
// reference 12.9 pt (4.55 mm). Every other body label's own style height plots
// within 0.05 mm of the reference and is drawn as the style states.
//
// The three title-block `DWG-…集…` strings that sat 17.6 mm left of the
// reference are placed now: a boxed record hangs from the box edge its
// paragraph's alignment names, so a right-aligned field ends on the box's right
// edge (`docs/analysis/2026-10-20-a-field-hangs-from-its-box-edge.md`). A is at
// every label it counts.
//
// The ink move is the `0x007E` direction byte: the diaphragm actuator's dome
// was drawn as its own mirror on all five pages — the record says `flag 1`,
// which sweeps the parameters the other way, and only that reading puts the
// apex where the reference draws it
// (`docs/analysis/2026-10-20-the-actuator-dome-is-on-the-wrong-side.md`, top
// section). Extra ink 147.3 → 98.4 mm across the five pages, coverage up on
// every one of them, and the `Diaphragm Actuator.sym` group is gone from every
// residual list.
//
// The current move is the border template
// (`docs/analysis/2026-10-23-the-border-template-draws-the-sheet-furniture.md`):
// all four A0/A1 sheets now carry the frame, title block and sign-off column
// their linked `.igr` states. `I` and `B`'s reference plots carry the same
// furniture, and their coverage rises (0.9989 → 0.9993, 0.9979 → 0.9997) as
// the parts of it that fall outside the two mask rectangles start matching.
// `A` and `低温分离器A`'s references were plotted without any furniture at all,
// so the same ink is charged as extra — 15 mm each, which is what moves their
// ceilings (7.6 → 22.6 and 22.9 → 37.9 mm). That is a real difference between
// those two reference pages and every drawing in the set, not a regression:
// their `.pid` states the same linked A1 frame as `B`'s. That charge is gone as
// of the mask move below: the furniture is masked on every sheet now, whether
// or not that page's reference drew it.
//
// Today's move is the coverage figure's own sampling bug, the mirror of the
// extra one. It was measured on the centre lines `join_centerlines` merges out
// of the reference's strokes, and that merge bridges every blank under 3 mm —
// the sheets' dashed lines leave 1.0–1.2 mm between dashes, so the merge
// filled their gaps with reference ink, and our dashes (phase-shifted, their
// own 1.905 / 1.270 mm against the reference's 2.032 + 0.169 dash and 1.0 mm
// gap) covered the filled blanks. Four of the five sheets therefore read
// exactly 1.0000 and a missing dashed line could not move the number. Sampled
// on the strokes as drawn, on both sides, the five read 0.9995 / 0.9997 /
// 0.9996 / 0.9996 / 0.9990. Nearly everything left uncovered is dash ends —
// 0.17 to 0.31 mm each, collinear to 0.05 mm, our ink 0.55–0.65 mm away inside
// the neighbouring blank, measured by
// `pid-plot/examples/probe_which_reference_ink_is_uncovered.rs`: 73 of `I`'s
// 75 runs, 15.53 of its 15.62 mm. The two left over total 0.085 mm — an
// uncovered sample on a 0.169 mm reference sliver at the page edge `x = 0`,
// whose nearest ink is 10.1 mm away, plus a zero-length reference stroke. No
// page is missing geometry: no uncovered run is longer than 0.31 mm.
//
// The coverage floors follow that reading minus 0.0001, which is 1 mm on an A1
// sheet and 3 mm on the A0: a lost symbol body (tens of millimetres) fails at
// once, a single dash end does not, and the old floors — 0.9977 to 0.9995
// against readings then reported as 1.0000 — could not see either.
//
// The ceilings are re-locked again today because the extra figure itself was
// wrong: it used to be sampled on the centre lines `join_centerlines` merges
// out of the drawn strokes, and that merge closes any gap under 3 mm —
// including the blank inside a symbol body. A ball valve's diagonals stop at
// its circle, so 1.5 mm of empty paper became ink on every valve: 8 joins per
// valve type on `I`, 4 on `A` and `低温分离器A`. Sampling the drawn segments
// instead (`docs/analysis/2026-10-26-the-extra-ink-was-a-join-across-a-blank.md`)
// reads 0.30 / 15.00 / 0.00 / 18.08 / 8.72 mm. Almost all of that was still not
// ink of ours that the reference lacks, but the mask's own right edge: the left
// rectangle stopped at `x = 70` (A1) / `72` (A0) while the template's 50 × 10 mm
// sign-off-column box reaches `x = 75.125` — `74.999970` mm of rail plus half
// its 0.25 mm stroke. Widening the rectangle to that rail
// (`pid-plot/assets/template-masks.json`) drops the furniture on every sheet,
// and the five read **0.30 / 0.00 / 0.00 / 3.08 / 8.72 mm**. No reference content
// was hidden by the wider rectangle: `A`, `低温分离器A` and `换热器CD` lose
// nothing from their reference length (`masked_reference_length_mm` stays 0.000
// mm on all three), and on `I` and `B` the 12.99 / 14.99 mm the rectangle takes
// in is the box itself, ink for ink the same as ours — coverage holds at
// 0.9995 / 0.9997 / 0.9996 / 0.9996 / 0.9990.
//
// What was left after that was the complex-dash 45° ticks, and they were ours:
// one glyph on `低温分离器A` (3.08 mm) and two on `换热器CD` (8.72 mm). All
// three sat on runs shorter than half the pattern's 10.16 mm advance — 1.32,
// 3.81 and 4.23 mm — where `complex_dash_positions` used to pull the single
// glyph back to the run's end. The reference inks only the rail on those runs,
// with its nearest glyph 40.3–40.8 mm away, and the clamp was never measured:
// taking the glyph off them drops `低温分离器A` to 0.00 and `换热器CD` to 0.00
// with both coverages bit-identical, which is what says the 11.8 mm was ink the
// reference never had and not a phase
// (`docs/analysis/2026-10-26-the-hydraulic-glyph-comes-off-sub-half-period-stubs.md`).
//
// What is left is `I`'s 0.31 mm: sixteen fragments of our arc chain on
// `Wellhead Christmas Tree 02`, 0.011–0.093 mm each, every one of them
// 0.501–0.550 mm from the reference's nearest ink — threshold crossings, not
// geometry. The ceilings follow one bridge's worth of slack above the reading —
// **1 / 0.5 / 0.5 / 0.5 / 0.5 mm** — so a single symbol body leaking back in
// fails the gate.
const BASELINES: [Baseline; 5] = [
    Baseline {
        name: "I井场 注采阀组工艺及自控流程图",
        min_coverage: 0.9993,
        max_extra_mm: 1.0,
        min_text_matched: 546,
        min_pen_agreed: 8390,
        max_pen_differed: 0,
    },
    Baseline {
        name: "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
        min_coverage: 0.9995,
        max_extra_mm: 0.5,
        min_text_matched: 151,
        min_pen_agreed: 1196,
        max_pen_differed: 0,
    },
    Baseline {
        name: "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
        min_coverage: 0.9995,
        max_extra_mm: 0.5,
        min_text_matched: 123,
        min_pen_agreed: 1335,
        max_pen_differed: 0,
    },
    Baseline {
        name: "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
        min_coverage: 0.9995,
        max_extra_mm: 0.5,
        min_text_matched: 167,
        min_pen_agreed: 1619,
        max_pen_differed: 0,
    },
    Baseline {
        name: "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
        min_coverage: 0.9989,
        max_extra_mm: 0.5,
        min_text_matched: 249,
        min_pen_agreed: 2869,
        max_pen_differed: 0,
    },
];

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

#[test]
fn five_drawings_hold_the_ratchet() {
    let corpus = corpus_dir();
    let first_reference = corpus.join(format!("{}.pdf", BASELINES[0].name));
    if !first_reference.is_file() {
        eprintln!(
            "skipping plot ratchet; {} is absent",
            first_reference.display()
        );
        return;
    }
    let out_dir = corpus.join("plot/compare");
    std::fs::create_dir_all(&out_dir).expect("mkdir compare");
    // Our page is written here, from the model, rather than read from
    // `plot/<name>.pdf`. Reading the stored file made the gate blind to the
    // writer: raising `pen_width_mm`'s thin/mid boundary to 0.40 moved 5 877
    // strokes off the reference's pen and this gate stayed green, because the
    // PDF on disk had been written by the previous build. The stored plot is
    // still the deliverable — `write_five_drawings` writes it — but what this
    // gate measures is the writer that would write it.
    let models: Vec<PaperModel> = BASELINES
        .iter()
        .map(|baseline| {
            lay_out_drawing(&corpus.join(format!("{}.pid", baseline.name)), &corpus)
                .expect("layout")
        })
        .collect();
    let dirs = font_dirs(Some(&corpus));
    let prepared = prepare_fonts(&models.iter().collect::<Vec<_>>(), &dirs).expect("fonts");
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
            subset: face.subset,
        })
        .collect();
    for (baseline, model) in BASELINES.iter().zip(&models) {
        let name = baseline.name;
        let reference_bytes = std::fs::read(corpus.join(format!("{name}.pdf"))).expect("reference");
        let reference = read_plot(&reference_bytes).expect("read reference");
        let our_bytes = write_plot(model, &fonts).expect("write ours");
        let ours = read_plot(&our_bytes).expect("read ours");
        let template = template_masks(reference.page_mm);
        let missing: Vec<MaskRect> = model
            .missing_boxes
            .iter()
            .map(|box_| MaskRect {
                label: box_.path.clone(),
                min_mm: box_.min_mm,
                max_mm: box_.max_mm,
            })
            .collect();
        let report = compare_plots_with_model(&reference, &ours, &template, &missing, model);
        let json_path = out_dir.join(format!("{name}.json"));
        let png_path = out_dir.join(format!("{name}.png"));
        write_report(&json_path, &report).expect("json");
        write_overlay(&png_path, &reference, &ours, &report.fit).expect("overlay");
        assert!(
            png_path.metadata().expect("png meta").len() > 100,
            "{name} overlay png"
        );
        assert!(
            report.fit.within_threshold,
            "{name} fit outside threshold: {:?}",
            report.fit
        );
        assert!(
            report.coverage >= baseline.min_coverage,
            "{name} coverage {:.4} < {:.4}",
            report.coverage,
            baseline.min_coverage
        );
        assert!(
            report.extra_length_mm <= baseline.max_extra_mm,
            "{name} extra {:.1} mm > {:.1} mm",
            report.extra_length_mm,
            baseline.max_extra_mm
        );
        assert!(
            report.text_matched >= baseline.min_text_matched,
            "{name} text_matched {} < {}",
            report.text_matched,
            baseline.min_text_matched
        );
        // The pen section is one measurement stated three ways, so it is
        // checked for internal agreement before its ratchet: a stroke is
        // either agreed or differed once it has a partner, and no pair may
        // count a stroke twice.
        let pen = &report.pen;
        assert_eq!(
            pen.paired,
            pen.agreed + pen.differed,
            "{name} pen pairs {}/{}/{}",
            pen.paired,
            pen.agreed,
            pen.differed
        );
        let counted: usize = pen.pairs.iter().map(|pair| pair.strokes).sum();
        assert_eq!(
            counted, pen.paired,
            "{name} pen pairs carry {counted} of {} paired strokes",
            pen.paired
        );
        let differed: usize = pen
            .pairs
            .iter()
            .filter(|pair| !pair.agrees())
            .map(|pair| pair.strokes)
            .sum();
        assert_eq!(
            differed, pen.differed,
            "{name} disagreeing pairs carry {differed} of {} differing strokes",
            pen.differed
        );
        assert!(
            pen.agreed >= baseline.min_pen_agreed,
            "{name} pen agreed {} < {} ({} strokes now differ: {})",
            pen.agreed,
            baseline.min_pen_agreed,
            pen.differed,
            pen.differences()
                .map(|pair| format!(
                    "{:.2} vs {:.2} mm x{}",
                    pair.ours_mm(),
                    pair.reference_mm(),
                    pair.strokes
                ))
                .collect::<Vec<_>>()
                .join(", ")
        );
        assert!(
            pen.differed <= baseline.max_pen_differed,
            "{name} pen differed {} > {} ({}): {}",
            pen.differed,
            baseline.max_pen_differed,
            pen.agreement(),
            pen.differences()
                .map(|pair| format!(
                    "{:.2} vs {:.2} mm x{}",
                    pair.ours_mm(),
                    pair.reference_mm(),
                    pair.strokes
                ))
                .collect::<Vec<_>>()
                .join(", ")
        );
        // A class whose mean is not zero is a placement rule that is wrong
        // for every label in it, while every one of those labels still pairs
        // — 0.79 mm is inside the 1 mm window. The I drawing's top-anchored
        // labels sat at +0.32 body heights for two rounds this way, so the
        // mean is gated separately from the residual counts.
        for class in &report.text_offsets {
            if class.count < CLASS_RATCHET_MIN_COUNT {
                continue;
            }
            assert!(
                class.mean_along_heights.abs() <= CLASS_RATCHET_ALONG_HEIGHTS,
                "{name} {} along {:+.3} body heights over {} labels (run width {:+.3})",
                class.class,
                class.mean_along_heights,
                class.count,
                class.mean_advance_heights
            );
            assert!(
                class.mean_down_heights.abs() <= CLASS_RATCHET_HEIGHTS,
                "{name} {} down {:+.3} body heights over {} labels",
                class.class,
                class.mean_down_heights,
                class.count
            );
        }
    }
}
