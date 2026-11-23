//! The five drawings lay out against the SQPlant symbol library
//! (`RefData~4~681.zip` unpacked to `Ref/Symbols`), and the found/referenced
//! counts stay pinned, including the one absent pulsation-damper symbol.
//!
//! Every drawing's circles and lines are also held to its page, which is the
//! paper its sheet states: all five state one, the drawing with no border
//! frame included. Which paper each is on, and which of them has a frame, is
//! pinned too.

use std::path::PathBuf;

use pid_plot::model::{PaperShape, lay_out_drawing};

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"D:\work\cad\PID自研软件验证"))
}

#[test]
fn five_drawings_match_the_symbol_gap() {
    let dir = corpus_dir();
    // File name, symbols found, symbols referenced, the paper its sheet
    // states, and whether the sheet has a border frame.
    let samples = [
        (
            "I井场 注采阀组工艺及自控流程图.pid",
            58,
            59,
            [1189.0, 841.0],
            true,
        ),
        (
            "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图.pid",
            29,
            29,
            [841.0, 594.0],
            true,
        ),
        (
            "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid",
            28,
            28,
            [841.0, 594.0],
            true,
        ),
        (
            "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图.pid",
            33,
            33,
            [841.0, 594.0],
            true,
        ),
        (
            "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图.pid",
            38,
            38,
            [841.0, 594.0],
            false,
        ),
    ];
    if !dir.join(samples[0].0).is_file() {
        eprintln!("skipping paper models; {} is absent", dir.display());
        return;
    }
    for (name, found, referenced, paper, framed) in samples {
        let model = lay_out_drawing(&dir.join(name), &dir)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(
            model.symbols_found, found,
            "{name} found; library_gaps: {:?}; missing: {:?}; library: {:?}",
            model.library_gaps, model.missing_symbols, model.library
        );
        assert_eq!(model.symbols_referenced, referenced, "{name} referenced");
        assert!(
            model.missing_symbols.is_empty(),
            "{name} undrawn missing symbols: {:?}",
            model.missing_symbols
        );
        assert!(
            model.missing_boxes.is_empty(),
            "{name} missing boxes: {:?}",
            model.missing_boxes
        );
        assert_eq!(
            model.library_gaps.len(),
            referenced - found,
            "{name} library_gaps"
        );
        if name == samples[0].0 {
            assert_eq!(
                model.library_gaps,
                [r"piping\specialty components\misc\pulsation damper.sym"],
                "{name} known library gap"
            );
        }
        assert!(!model.library.is_empty(), "{name} names the library");
        assert!(!model.primitives.is_empty(), "{name} has primitives");
        // Every one of the five states its paper, and that is its page.
        // `换热器CD` carries neither an `igSmartFrame2d` nor a template name;
        // until the paper was read its page was measured, onto the same A1
        // sheet its `JSheet` record states. Four have a border frame, whose
        // rectangle is the paper to a ten-thousandth of a millimetre here.
        // Pinned both ways round: a drawing that stops stating its paper is
        // a decode regression, and so is a frame that comes or goes.
        let page = model
            .resolved_page_mm()
            .unwrap_or_else(|| panic!("{name}: the model resolved no page"));
        assert_eq!(
            model.page_mm,
            Some(paper),
            "{name}: the paper its sheet states"
        );
        assert_eq!(page, paper, "{name}: the page is the paper");
        assert_eq!(
            model.frame_mm.is_some(),
            framed,
            "{name}: its border frame states {:?}",
            model.frame_mm
        );
        if let Some(frame) = model.frame_mm {
            assert!(
                (frame[0] - paper[0]).abs() < 0.001 && (frame[1] - paper[1]).abs() < 0.001,
                "{name}: the frame's rectangle {frame:?} is not its paper"
            );
        }
        // The guards run against the page, so the drawing with no frame is
        // held to its paper like the rest.
        let [page_w, page_h] = page;
        let mut circles = 0_usize;
        let mut lines = 0_usize;
        for primitive in &model.primitives {
            match &primitive.shape {
                PaperShape::Circle {
                    center_mm,
                    radius_mm,
                } => {
                    circles += 1;
                    let right_mm = center_mm[0] + radius_mm.abs();
                    let top_mm = center_mm[1] + radius_mm.abs();
                    assert!(
                        right_mm <= page_w + 25.0,
                        "{name} circle right edge within paper+25mm",
                    );
                    assert!(
                        top_mm <= page_h + 25.0,
                        "{name} circle top edge within paper+25mm",
                    );
                }
                PaperShape::Line { start_mm, end_mm } => {
                    lines += 1;
                    let dx = (start_mm[0] - end_mm[0]).abs();
                    let dy = (start_mm[1] - end_mm[1]).abs();
                    assert!(
                        dy >= 1.0 || dx <= page_w * 0.90,
                        "{name} keeps a sheet-width line"
                    );
                    assert!(
                        dx >= 1.0 || dy <= page_h * 0.90,
                        "{name} keeps a sheet-height line"
                    );
                }
                // These guards target circles and sheet-frame lines only.
                PaperShape::Arc { .. }
                | PaperShape::Fill { .. }
                | PaperShape::Polyline { .. }
                | PaperShape::Text { .. } => {}
            }
        }
        eprintln!(
            "{name}: the {page_w:.3} x {page_h:.3} mm paper its sheet states ({}) holds {circles} \
             circles and {lines} lines",
            if framed {
                "with a border frame"
            } else {
                "no border frame"
            }
        );
    }
}
