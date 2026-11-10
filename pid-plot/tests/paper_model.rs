//! The five drawings lay out against the SQPlant symbol library
//! (`RefData~4~681.zip` unpacked to `Ref/Symbols`), and the found/referenced
//! counts stay pinned, including the one absent pulsation-damper symbol.
//!
//! Every drawing's circles and lines are also held to its page: the sheet the
//! `.pid` states or, for the one drawing that states none, the standard sheet
//! its ink is measured onto. Which drawing takes which path is pinned too.

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
    // File name, symbols found, symbols referenced and, for a drawing that
    // states no sheet, the page it is measured onto.
    let samples = [
        ("I井场 注采阀组工艺及自控流程图.pid", 58, 59, None),
        (
            "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图.pid",
            29,
            29,
            None,
        ),
        (
            "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid",
            28,
            28,
            None,
        ),
        (
            "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图.pid",
            33,
            33,
            None,
        ),
        (
            "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图.pid",
            38,
            38,
            Some([841.0, 594.0]),
        ),
    ];
    if !dir.join(samples[0].0).is_file() {
        eprintln!("skipping paper models; {} is absent", dir.display());
        return;
    }
    for (name, found, referenced, measured_page) in samples {
        let model = lay_out_drawing(&dir.join(name), &dir)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(
            model.symbols_found, found,
            "{name} found; missing: {:?}; library: {:?}",
            model.missing_symbols, model.library
        );
        assert_eq!(model.symbols_referenced, referenced, "{name} referenced");
        assert_eq!(
            model.missing_symbols.len(),
            referenced - found,
            "{name} missing"
        );
        if name == samples[0].0 {
            assert_eq!(
                model.missing_symbols,
                [r"piping\specialty components\misc\pulsation damper.sym"],
                "{name} known library gap"
            );
        }
        assert!(!model.library.is_empty(), "{name} names the library");
        assert!(!model.primitives.is_empty(), "{name} has primitives");
        // Four of the five state their sheet. `换热器CD` carries neither an
        // `igSmartFrame2d` nor a template name, so its page is the standard
        // sheet its ink fits, A1. Pinned both ways round: a drawing that stops
        // stating its sheet is a decode regression, and one that starts
        // stating it moves the page every guard below is held to.
        let page = model
            .resolved_page_mm()
            .unwrap_or_else(|| panic!("{name}: the model resolved no page"));
        match measured_page {
            None => assert!(
                model.page_mm.is_some(),
                "{name} is pinned as stating its sheet and states none; \
                 its page was measured as {page:?}"
            ),
            Some(sheet) => {
                assert_eq!(model.page_mm, None, "{name} is pinned as stating no sheet");
                assert_eq!(page, sheet, "{name} is measured onto another sheet");
            }
        }
        // The guards run against the resolved page, not only a stated one, so
        // the drawing whose sheet is measured is held to it like the rest.
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
            "{name}: {} page {page_w:.3} x {page_h:.3} mm holds {circles} circles and {lines} lines",
            if measured_page.is_some() {
                "measured"
            } else {
                "stated"
            }
        );
    }
}
