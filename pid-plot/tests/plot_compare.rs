//! S4 ratchet gate: the three comparison figures on the five corpus drawings
//! may improve (higher coverage, less extra ink, more matched text) and may
//! not regress below the measured baseline.
//!
//! The text figures count labels only: a text run that is only spaces is
//! skipped on both sides, so two blank runs no longer pair as a "match".
//! Matched counts are therefore one to four lower than the numbers this gate
//! held before blanks were excluded — the drop is four phantom pairs, not
//! four lost labels (the I drawing's reference total fell 552 → 546 for the
//! same reason). Later rounds raised them past that point again: the I
//! drawing is at 543 of 546, and two of the five drawings (B and H) pair
//! every reference label they count. The L drawing sits at 165 of 167 for a
//! reason of its own — see the baseline note below.

use std::path::PathBuf;

use pid_plot::compare::{
    MaskRect, compare_plots_with_model, template_masks, write_overlay, write_report,
};
use pid_plot::model::lay_out_drawing;
use pid_plot::read::read_plot;

struct Baseline {
    name: &'static str,
    min_coverage: f64,
    max_extra_mm: f64,
    min_text_matched: usize,
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
// (`docs/analysis/2026-10-20-a-pad-is-not-part-of-the-label.md`). It also
// cost the L drawing two labels without losing any ink: that drawing's two
// `0103501A` tags sit 13.22 mm apart while the same nine characters ink
// 13.32 mm, so the runs touch and this crate's reader glues them into one
// 18-character span. PyMuPDF splits at every text object and so keeps the
// reference's pair apart; ours has no text-object signal to split on. Both
// tags are drawn where the reference puts them, so the baseline records the
// reader's blind spot rather than a placement loss.
//
// Five reference runs are left in the I drawing: two electric-motor `M`s whose
// height no field explains, and three title-block `DWG-…集…` strings drawn
// 17.6 mm left of the reference.
const BASELINES: [Baseline; 5] = [
    Baseline {
        name: "I井场 注采阀组工艺及自控流程图",
        min_coverage: 0.9980,
        max_extra_mm: 65.0,
        min_text_matched: 543,
    },
    Baseline {
        name: "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
        min_coverage: 0.9990,
        max_extra_mm: 20.0,
        min_text_matched: 149,
    },
    Baseline {
        name: "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
        min_coverage: 0.9970,
        max_extra_mm: 15.0,
        min_text_matched: 123,
    },
    Baseline {
        name: "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
        min_coverage: 0.9975,
        max_extra_mm: 35.0,
        min_text_matched: 165,
    },
    Baseline {
        name: "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
        min_coverage: 0.9980,
        max_extra_mm: 50.0,
        min_text_matched: 249,
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
    let first_ours = corpus.join(format!("plot/{}.pdf", BASELINES[0].name));
    let first_reference = corpus.join(format!("{}.pdf", BASELINES[0].name));
    if !first_ours.is_file() || !first_reference.is_file() {
        eprintln!("skipping plot ratchet; {} is absent", first_ours.display());
        return;
    }
    let out_dir = corpus.join("plot/compare");
    std::fs::create_dir_all(&out_dir).expect("mkdir compare");
    for baseline in BASELINES {
        let name = baseline.name;
        let reference_bytes = std::fs::read(corpus.join(format!("{name}.pdf"))).expect("reference");
        let our_bytes = std::fs::read(corpus.join(format!("plot/{name}.pdf"))).expect("ours");
        let reference = read_plot(&reference_bytes).expect("read reference");
        let ours = read_plot(&our_bytes).expect("read ours");
        let model = lay_out_drawing(&corpus.join(format!("{name}.pid")), &corpus).expect("layout");
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
        let report = compare_plots_with_model(&reference, &ours, &template, &missing, &model);
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
