//! Acceptance gate: every drawing in `验收数据` that ships a SmartPlant plot,
//! written by this crate's own writer and compared to that plot.
//!
//! Eleven drawings live there: the five the crate already ratcheted
//! (`plot_compare.rs` measures them out of the same directory), the two A0
//! wellsite sheets, the three A1 air-cooler sheets, and one A2 sheet. Their
//! reference plots are the plotter's own output — Amyuni PDF Converter
//! 4.5.2.9, the driver SmartPlant ships — so the page sizes and pens they
//! carry are the paper the drawings were issued on.
//!
//! What the readings say, 2026-11-02:
//!
//! * Ten of the eleven sit at coverage 0.9990–0.9997; the three A1 air
//!   coolers rose from 0.9986 once `0x0063 igEllipse2d`, the fan ovals of
//!   `Air Cooler Type 1.sym`, was decoded (`docs/plans/2026-11-02-the-a1-stem.md` §7). The extra ink on the
//!   three A0 wellsites was a join: a valve stem 0.33 mm beside its pipe is
//!   inside the fit's 0.4 mm sideways tolerance, and the merged chord then
//!   misses the stem both pages draw. The extra sampler joins at 0.20 mm, so
//!   I, J K and L M read 0.000 mm (`docs/analysis/2026-11-04-the-extra-join-was-swallowing-the-stem.md`).
//!   Every label the reference counts is paired. A same-width chord within
//!   0.5 mm, lengths within a tenth, uses the fill outline's angle, so the
//!   three valve chords on J K and L M pair at 0.34 mm instead of the thinner
//!   stroke 1 mm away. The three A1 sheets' shared 8.210 mm residue
//!   was a lone complex-dash glyph at the run's midpoint, and they now read
//!   extra 0.000 mm (`docs/plans/2026-11-02-the-a1-stem.md`).
//!   Those ten now read coverage 1.000000 and uncovered 0.000 mm: what was
//!   left on them was the tip of a signal-line dash, and it closed once that
//!   pattern (`JStyleSimpleDashType` fit mode 2) is restarted on every vertex
//!   and stretched to each straight piece the way `style.dll` lays it out
//!   (`docs/analysis/2026-11-08-the-signal-dash-is-fitted-to-every-piece.md`).
//! * The A2 sheet reads 0.9819 / 210.712 mm because it is the first A2 page
//!   this crate has ever had a reference for, and its furniture is a third
//!   template family (`CPECCHB-SQ-A2.igr`, the one its own link record names).
//!   `pid-plot/assets/template-masks.json` had no A2 entry before this round,
//!   so the whole title block and sign-off column were charged as drawing ink:
//!   0.9379 / 1132.334 mm. The surveyed A2 masks are what closed that gap, and
//!   what is left is one instrument cluster drawn ~16.9 mm high and ~1.9 mm
//!   right of where the reference prints it — `TICA` and `H=-15℃` are both
//!   decoded and laid out, at 500.8/256.3 and 507.8/260.8 mm. Eleven reference
//!   labels still miss: nine are that cluster, and two are pipeline strings
//!   whose text is the `.pid`'s own. The two `动火点` labels pair, and so does
//!   the mirrored valve's `D` now that a symbol top hangs from the ascender
//!   line, so the text pin is 214 of 225
//!   (`docs/analysis/2026-11-04-the-a2-five-labels-outside-the-bubble.md`,
//!   `docs/analysis/2026-11-05-a-symbol-top-is-the-ascender-line.md`).
//!   The 79.143 mm grouped as `/Sheet6` is that leader at `x 500.69`, 2.00 mm
//!   beside the reference leader, not frame ink
//!   (`docs/analysis/2026-11-04-the-a2-sheet6-extra-is-the-leader.md`).
//!   The 120.511 mm of uncovered `/Sheet6` is the same leader's reference copy,
//!   the TT balloon at `(498.70, 220.37)`, and dash tips
//!   (`docs/analysis/2026-11-04-the-a2-sheet6-uncovered-is-the-reference-leader.md`).
//!   The cluster is the same body at
//!   the same size — 12.698 mm across against the reference's 12.869, one pen
//!   apart — simply translated, and its leader's far end stays put at `y≈342.4`
//!   while the near end follows the bubble: the `.pid` records the bubble dragged
//!   along its line, so no renderer change recovers the reference's position
//!   (`docs/analysis/2026-11-02-the-a2-labels-were-already-drawn.md`). The pins
//!   below are the reading, and raising them is a code change, not a threshold
//!   change.
//!
//! The pen column is the reading of `CompareReport::pen`: how many of our
//! strokes the reference prints at the width we print them. It reached zero
//! differences on all eleven pages only after `pen_width_mm` grew the fourth
//! pen — the CCHB A2 title block declares 1.0606 / 1.1313 / 1.4141 mm rails
//! and the plot prints every one of them at 1.02 mm.

use std::path::PathBuf;

use pid_plot::compare::{
    MaskRect, compare_plots_with_model, template_masks, write_overlay, write_report,
};
use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperModel, lay_out_drawing};
use pid_plot::read::read_plot;
use pid_plot::write::{PlotFont, write_plot};

/// One drawing's measured reading, as an acceptance floor/ceiling.
struct Pin {
    name: &'static str,
    /// Page the reference plot is, millimetres.
    page_mm: [f64; 2],
    min_coverage: f64,
    max_extra_mm: f64,
    /// Reference ink, millimetres, that no ink of ours covers. Ten pages read
    /// 0.000 once the signal-line dash is fitted to every straight piece; the
    /// A2 remainder is the instrument cluster the `.pid` stores elsewhere.
    max_uncovered_mm: f64,
    min_text_matched: usize,
    /// Reference labels the page counts, so a page that stops counting its
    /// reference's labels cannot pass by matching fewer of them.
    text_reference: usize,
    min_pen_agreed: usize,
    max_pen_differed: usize,
    /// Symbols the corpus cannot draw. The A0 wellsites both place a
    /// `pulsation damper.sym` the reference corpus never ships.
    missing_symbols: &'static [&'static str],
}

/// One bridge's worth of slack above the reading, as `plot_compare.rs` holds
/// its ceilings: a symbol body leaking back in fails, a threshold crossing
/// does not.
const EXTRA_SLACK_MM: f64 = 0.5;

/// Smallest lettering class whose mean down-shift is gated, in labels.
const CLASS_MIN_COUNT: usize = 10;

/// Largest mean down-shift a lettering class may show, in body heights.
///
/// Every class of ten or more labels on the eleven pages reads within
/// ±0.026. The valve and motor `M` (`symbol/Middle`, 14–29 labels on each A0
/// wellsite) read +0.094…+0.098 while a symbol body's ascent-measured height
/// was stepped down as if it were a cap height; this is the gate that would
/// have caught it.
const CLASS_DOWN_HEIGHTS: f64 = 0.04;

/// Largest mean width difference a lettering class may show, in body heights.
///
/// Every class of ten or more labels on the eleven pages reads within
/// ±0.092 (the widest is a `Left/Ascender` class on a 烃水露点 page). The A2
/// `Left/Ascender` class read −0.320 while its 8 pt Arial line labels were
/// lettered at the style size rather than the reference's `Tf 11.16`.
const CLASS_ADVANCE_HEIGHTS: f64 = 0.10;

const PULSATION_DAMPER: &str = r"piping\specialty components\misc\pulsation damper.sym";

const PINS: [Pin; 11] = [
    Pin {
        name: "I井场 注采阀组工艺及自控流程图",
        page_mm: [1189.0, 841.0],
        min_coverage: 0.9999,
        max_extra_mm: 0.0 + EXTRA_SLACK_MM,
        max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
        min_text_matched: 546,
        text_reference: 546,
        min_pen_agreed: 8685,
        max_pen_differed: 0,
        missing_symbols: &[PULSATION_DAMPER],
    },
    Pin {
        name: "J K井场 注采阀组工艺及自控流程图",
        page_mm: [1189.0, 841.0],
        min_coverage: 0.9999,
        max_extra_mm: 0.0 + EXTRA_SLACK_MM,
        max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
        min_text_matched: 775,
        text_reference: 775,
        min_pen_agreed: 13630,
        max_pen_differed: 0,
        missing_symbols: &[PULSATION_DAMPER],
    },
    Pin {
        name: "L M井场 注采阀组工艺及自控流程图",
        page_mm: [1189.0, 841.0],
        min_coverage: 0.9999,
        max_extra_mm: 0.0 + EXTRA_SLACK_MM,
        max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
        min_text_matched: 921,
        text_reference: 921,
        min_pen_agreed: 16481,
        max_pen_differed: 0,
        missing_symbols: &[PULSATION_DAMPER],
    },
    Pin {
        name: "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
        page_mm: [841.0, 594.0],
        min_coverage: 0.9999,
        max_extra_mm: 0.0 + EXTRA_SLACK_MM,
        max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
        min_text_matched: 151,
        text_reference: 151,
        min_pen_agreed: 1229,
        max_pen_differed: 0,
        missing_symbols: &[],
    },
    Pin {
        name: "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
        page_mm: [841.0, 594.0],
        min_coverage: 0.9999,
        max_extra_mm: 0.0 + EXTRA_SLACK_MM,
        max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
        min_text_matched: 123,
        text_reference: 123,
        min_pen_agreed: 1362,
        max_pen_differed: 0,
        missing_symbols: &[],
    },
    Pin {
        name: "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
        page_mm: [841.0, 594.0],
        min_coverage: 0.9999,
        max_extra_mm: 0.0 + EXTRA_SLACK_MM,
        max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
        min_text_matched: 167,
        text_reference: 167,
        min_pen_agreed: 1646,
        max_pen_differed: 0,
        missing_symbols: &[],
    },
    Pin {
        name: "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
        page_mm: [841.0, 594.0],
        min_coverage: 0.9999,
        max_extra_mm: 0.0 + EXTRA_SLACK_MM,
        max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
        min_text_matched: 249,
        text_reference: 249,
        min_pen_agreed: 2976,
        max_pen_differed: 0,
        missing_symbols: &[],
    },
    Pin {
        // The riser's stamped glyph centres itself now, so this sheet keeps
        // no extra ink at all: see `docs/plans/2026-11-02-the-a1-stem.md`.
        // The air cooler's two fan ovals are `0x0063 igEllipse2d` records in
        // both the `.sym` and the cached body; drawing them took uncovered
        // from 23.031 to 5.757 mm (plan §7).
        name: "集注站 烃水露点控制单元（一） 空冷器A集预冷分离器A工艺及自控流程图",
        page_mm: [841.0, 594.0],
        min_coverage: 0.9999,
        max_extra_mm: 0.0 + EXTRA_SLACK_MM,
        max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
        min_text_matched: 288,
        text_reference: 288,
        min_pen_agreed: 3253,
        max_pen_differed: 0,
        missing_symbols: &[],
    },
    Pin {
        // Same glyph, same fix as the A sheet: no extra ink left. The
        // fan ovals draw as on the A sheet (plan §7).
        name: "集注站 烃水露点控制单元（二） 空冷器B集预冷分离器B工艺及自控流程图",
        page_mm: [841.0, 594.0],
        min_coverage: 0.9999,
        max_extra_mm: 0.0 + EXTRA_SLACK_MM,
        max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
        min_text_matched: 290,
        text_reference: 290,
        min_pen_agreed: 3260,
        max_pen_differed: 0,
        missing_symbols: &[],
    },
    Pin {
        // Same glyph, same fix as the A sheet: no extra ink left. The
        // fan ovals draw as on the A sheet (plan §7).
        name: "集注站 烃水露点控制单元（三） 空冷器C集预冷分离器C工艺及自控流程图",
        page_mm: [841.0, 594.0],
        min_coverage: 0.9999,
        max_extra_mm: 0.0 + EXTRA_SLACK_MM,
        max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
        min_text_matched: 283,
        text_reference: 283,
        min_pen_agreed: 3132,
        max_pen_differed: 0,
        missing_symbols: &[],
    },
    Pin {
        // The two `动火点` labels are the only whole-string font fallback on
        // these sheets, both 2.5 mm and centred. Pitching that run at 1.524 mm
        // a glyph brings both inside 1 mm. The mirrored valve's `D` pairs
        // once it letters from the descender line. The other eleven stay out.
        // The template sits where the border frame places it, (0.388, 0.566)
        // mm off the origin, which pairs two more of its strokes.
        name: "集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图",
        page_mm: [594.0, 420.0],
        min_coverage: 0.9830,
        max_extra_mm: 210.048 + EXTRA_SLACK_MM,
        max_uncovered_mm: 206.935 + EXTRA_SLACK_MM,
        min_text_matched: 214,
        text_reference: 225,
        min_pen_agreed: 2648,
        max_pen_differed: 0,
        missing_symbols: &[],
    },
];

/// The three surveyed sheets, as `template_masks` answers them.
const MASK_SHEETS: [([f64; 2], [&str; 2]); 3] = [
    ([1189.0, 841.0], ["左侧会签栏与左上角框", "右下标题栏"]),
    ([841.0, 594.0], ["左侧会签栏与左上角框", "右下标题栏"]),
    // The CCHB A2 furniture: a 35.75 mm sign-off column down the left edge and
    // a 274 × 115 mm title block, both surveyed from the A2 reference plot.
    ([594.0, 420.0], ["左侧会签栏", "右下标题栏"]),
];

/// Font directories for this corpus, plus the self-check corpus's own set.
///
/// `验收数据` ships no fonts; the drawing corpus beside it does, and a run that
/// states `PID_PLOT_FONTS` still wins because `font_dirs` puts that list first.
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

/// A page that matches no surveyed sheet gets no masks at all.
#[test]
fn only_the_surveyed_sheets_carry_furniture_masks() {
    for (page_mm, labels) in MASK_SHEETS {
        let masks = template_masks(page_mm);
        let found: Vec<&str> = masks.iter().map(|mask| mask.label.as_str()).collect();
        assert_eq!(found, labels, "{page_mm:?}");
    }
    assert!(template_masks([300.0, 200.0]).is_empty());
    // The A2 survey must not swallow the drawing's own left-edge continuation
    // ink: its sign-off column stops at 35.75 mm, well inside the 42 mm double
    // rails the reference inks at x 50.969..92.964.
    let a2 = template_masks([594.0, 420.0]);
    assert!(a2.iter().all(|mask| mask.max_mm[0] <= 594.0));
    assert!(
        a2.iter()
            .any(|mask| mask.min_mm[0] <= 0.0 && mask.max_mm[0] <= 36.0),
        "the A2 sign-off column must stay inside the drawing's first 36 mm"
    );
}

#[test]
fn eleven_reference_plots_hold_their_readings() {
    let corpus = corpus_dir();
    let first_reference = corpus.join(format!("{}.pdf", PINS[0].name));
    if !first_reference.is_file() {
        eprintln!(
            "skipping the acceptance gate; {} is absent",
            first_reference.display()
        );
        return;
    }
    let out_dir = corpus.join("plot/acceptance");
    std::fs::create_dir_all(&out_dir).expect("mkdir acceptance");

    let models: Vec<PaperModel> = PINS
        .iter()
        .map(|pin| {
            let path = corpus.join(format!("{}.pid", pin.name));
            lay_out_drawing(&path, &corpus).unwrap_or_else(|error| {
                panic!("{}: layout: {error}", pin.name);
            })
        })
        .collect();
    let dirs = font_search_dirs(&corpus);
    let prepared = prepare_fonts(&models.iter().collect::<Vec<_>>(), &dirs).expect("fonts");
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
            subset: face.subset,
        })
        .collect();

    for (pin, model) in PINS.iter().zip(&models) {
        let name = pin.name;
        let reference_bytes = std::fs::read(corpus.join(format!("{name}.pdf"))).expect("reference");
        let reference = read_plot(&reference_bytes).expect("read reference");
        for axis in 0..2 {
            assert!(
                (reference.page_mm[axis] - pin.page_mm[axis]).abs() < 1.0,
                "{name} reference page {:?} is not {:?}",
                reference.page_mm,
                pin.page_mm
            );
        }
        let page = model.resolved_page_mm().expect("page");
        assert!(
            (page[0] - pin.page_mm[0]).abs() < 1.0 && (page[1] - pin.page_mm[1]).abs() < 1.0,
            "{name} page {page:?} is not the reference's {:?}",
            pin.page_mm
        );

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
        write_report(&out_dir.join(format!("{name}.json")), &report).expect("json");
        write_overlay(
            &out_dir.join(format!("{name}.png")),
            &reference,
            &ours,
            &report.fit,
        )
        .expect("overlay");

        assert!(
            report.fit.within_threshold,
            "{name} fit outside threshold: {:?}",
            report.fit
        );
        assert!(
            report.coverage >= pin.min_coverage,
            "{name} coverage {:.4} < {:.4}",
            report.coverage,
            pin.min_coverage
        );
        assert!(
            report.extra_length_mm <= pin.max_extra_mm,
            "{name} extra {:.3} mm > {:.3} mm",
            report.extra_length_mm,
            pin.max_extra_mm
        );
        let uncovered_mm = report.reference_length_mm - report.covered_length_mm;
        assert!(
            uncovered_mm <= pin.max_uncovered_mm,
            "{name} uncovered {uncovered_mm:.3} mm > {:.3} mm",
            pin.max_uncovered_mm
        );
        assert_eq!(
            report.text_reference, pin.text_reference,
            "{name} reference label count moved"
        );
        assert!(
            report.text_matched >= pin.min_text_matched,
            "{name} text_matched {} < {}",
            report.text_matched,
            pin.min_text_matched
        );
        let pen = &report.pen;
        assert_eq!(
            pen.paired,
            pen.agreed + pen.differed,
            "{name} pen pairs {}/{}/{}",
            pen.paired,
            pen.agreed,
            pen.differed
        );
        assert!(
            pen.agreed >= pin.min_pen_agreed,
            "{name} pen agreed {} < {}",
            pen.agreed,
            pin.min_pen_agreed
        );
        assert!(
            pen.differed <= pin.max_pen_differed,
            "{name} pen differed {} > {}",
            pen.differed,
            pin.max_pen_differed
        );
        for class in &report.text_offsets {
            if class.count < CLASS_MIN_COUNT || class.class == "unknown" {
                continue;
            }
            assert!(
                class.mean_down_heights.abs() <= CLASS_DOWN_HEIGHTS,
                "{name} {} down {:+.3} body heights over {} labels",
                class.class,
                class.mean_down_heights,
                class.count
            );
            assert!(
                class.mean_advance_heights.abs() <= CLASS_ADVANCE_HEIGHTS,
                "{name} {} advance {:+.3} body heights over {} labels",
                class.class,
                class.mean_advance_heights,
                class.count
            );
        }
        let missing_now: Vec<&str> = model.missing_symbols.iter().map(String::as_str).collect();
        assert_eq!(
            missing_now, pin.missing_symbols,
            "{name} missing symbol list moved"
        );
    }
}
