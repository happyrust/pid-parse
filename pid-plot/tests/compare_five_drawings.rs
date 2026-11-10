//! The five plots stored under the self-check corpus are each that drawing's
//! own plot.
//!
//! `<corpus>/plot/<name>.pdf` is the copy of a page a reader opens, and
//! `write_five_drawings::five_drawings_round_trip_through_pdf` is what writes
//! it. This gate reads the five stored files back and compares each with the
//! reference plot of the same name: the page has to be the reference's sheet,
//! the long-line fit has to land inside its threshold, and 99 in 100 of the
//! reference's labels have to pair. A file stored under another drawing's name
//! fails one of the three, as does a page that has lost its content; a file
//! that no longer reads fails before any of them.
//!
//! The stored files are as new as the last `write_five_drawings` run, which
//! may be older than the build under test, so nothing here is an exact
//! reading. The writer under test is held to its floors by
//! `plot_compare::five_drawings_hold_the_ratchet`, on pages it writes itself.
//! A stored set that fails here is put right by running `write_five_drawings`
//! again, which rewrites all five.
//!
//! Each comparison's report and overlay go to
//! `<corpus>/plot/compare/<name>.json` and `.png`. `plot_compare` and
//! `plot_text_sources` write theirs to the same paths, so the files on disk
//! describe whichever of the three ran last.

use std::path::PathBuf;

use pid_plot::compare::{
    MaskRect, compare_plots_with_model, template_masks, write_overlay, write_report,
};
use pid_plot::model::lay_out_drawing;
use pid_plot::read::read_plot;

const DRAWINGS: [&str; 5] = [
    "I井场 注采阀组工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
    "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
    "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
];

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

#[test]
fn five_drawings_write_compare_reports() {
    let corpus = corpus_dir();
    let first_reference = corpus.join(format!("{}.pdf", DRAWINGS[0]));
    if !first_reference.is_file() {
        eprintln!(
            "skipping plot compare; {} is absent",
            first_reference.display()
        );
        return;
    }
    let plot_dir = corpus.join("plot");
    // A corpus `write_five_drawings` has not written to stores nothing to
    // check. One that stores some of the five and not the rest is a broken
    // set, and that is a failure rather than a reason to skip.
    let absent: Vec<&str> = DRAWINGS
        .iter()
        .copied()
        .filter(|name| !plot_dir.join(format!("{name}.pdf")).is_file())
        .collect();
    if absent.len() == DRAWINGS.len() {
        eprintln!(
            "skipping plot compare; no plot is stored under {} (`write_five_drawings` writes them)",
            plot_dir.display()
        );
        return;
    }
    assert!(
        absent.is_empty(),
        "{} stores no plot for {absent:?}",
        plot_dir.display()
    );
    let out_dir = plot_dir.join("compare");
    std::fs::create_dir_all(&out_dir).expect("mkdir compare");
    for name in DRAWINGS {
        let reference_bytes = std::fs::read(corpus.join(format!("{name}.pdf"))).expect("reference");
        let reference = read_plot(&reference_bytes).expect("read reference");
        let stored = plot_dir.join(format!("{name}.pdf"));
        let our_bytes = std::fs::read(&stored)
            .unwrap_or_else(|error| panic!("read {}: {error}", stored.display()));
        let ours = read_plot(&our_bytes)
            .unwrap_or_else(|error| panic!("{name}: the stored plot does not read back: {error}"));
        // The stored page is the reference's sheet, to the millimetre
        // `reference_plots_acceptance` holds a page to its reference's. A plot
        // of a drawing on another sheet size stops here, before any ink is
        // compared.
        for axis in 0..2 {
            assert!(
                (ours.page_mm[axis] - reference.page_mm[axis]).abs() < 1.0,
                "{name}: the stored plot's page {:?} is not the reference's {:?}",
                ours.page_mm,
                reference.page_mm
            );
        }
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
        eprintln!(
            "{name} coverage {:.4} extra {:.0} mm text {}/{} fit {}",
            report.coverage,
            report.extra_length_mm,
            report.text_matched,
            report.text_reference,
            report.fit.within_threshold
        );
        assert!(json_path.is_file(), "{name} json");
        assert!(
            png_path.metadata().expect("png meta").len() > 100,
            "{name} png"
        );
        assert!(report.coverage.is_finite());
        // Another drawing on the same sheet size does not line up with this
        // reference's long lines: `换热器CD`'s plot under `低温分离器A`'s name
        // fits at scale 1.037.
        assert!(
            report.fit.within_threshold,
            "{name}: the stored plot does not fit its reference: {:?}",
            report.fit
        );
        // The floor `plot_text_sources` holds this build's pages to, and the
        // one check left when the fit lands anyway. Flash drum B's plot under
        // A's name fits A's reference and pairs 84 of its 151 labels; a blank
        // page fits at identity, for want of lines to fit, and pairs none.
        assert!(
            report.text_matched * 100 >= report.text_reference * 99,
            "{name}: the stored plot pairs {} of the reference's {} labels",
            report.text_matched,
            report.text_reference
        );
    }
}
