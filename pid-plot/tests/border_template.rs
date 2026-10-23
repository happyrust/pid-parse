//! The page draws the sheet furniture its linked `.igr` template carries.
//!
//! A P&ID's border is a linked OLE frame, so the double frame, the title block
//! and the sign-off column are not in the `.pid` at all: they live in the
//! `.igr` template the frame record points at. This gate pins that the page
//! finds that template, draws it at the sheet it was authored for, and letters
//! every one of its labels.
//!
//! Soft-skips when the corpus or the fixture is absent, which is what CI sees.

use std::path::{Path, PathBuf};

use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperModel, PaperShape, lay_out_drawing};
use pid_plot::read::read_plot;
use pid_plot::write::{PlotFont, write_plot};

const A1_DRAWING: &str = "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图";

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-file")
}

/// Every label the page letters, in model order.
fn labels(model: &PaperModel) -> Vec<&str> {
    model
        .primitives
        .iter()
        .filter_map(|primitive| match &primitive.shape {
            PaperShape::Text { value, .. } => Some(value.as_str()),
            _ => None,
        })
        .collect()
}

/// The bounding box of one furniture polyline, if it is wide enough to be a
/// frame rather than a title-block rule.
fn frame_boxes(model: &PaperModel) -> Vec<([f64; 2], [f64; 2])> {
    model
        .primitives
        .iter()
        .filter(|primitive| primitive.furniture)
        .filter_map(|primitive| match &primitive.shape {
            PaperShape::Polyline { points_mm, .. } if points_mm.len() > 1 => {
                let mut min = points_mm[0];
                let mut max = points_mm[0];
                for point in &points_mm[1..] {
                    min = [min[0].min(point[0]), min[1].min(point[1])];
                    max = [max[0].max(point[0]), max[1].max(point[1])];
                }
                Some((min, max))
            }
            _ => None,
        })
        .collect()
}

fn near(point: [f64; 2], expected: [f64; 2], tolerance: f64) -> bool {
    (point[0] - expected[0]).abs() <= tolerance && (point[1] - expected[1]).abs() <= tolerance
}

#[test]
fn the_a1_sheet_draws_the_frame_title_block_and_sign_off_column() {
    let corpus = corpus_dir();
    let drawing = corpus.join(format!("{A1_DRAWING}.pid"));
    if !drawing.is_file() {
        eprintln!("skipping border template; {} is absent", drawing.display());
        return;
    }
    let model = lay_out_drawing(&drawing, &corpus).unwrap_or_else(|error| panic!("{error}"));

    let template = model
        .template
        .as_ref()
        .unwrap_or_else(|| panic!("{A1_DRAWING}: no border template was drawn"));
    assert!(
        template.path.ends_with("CPE-SQ-A1.igr"),
        "the A1 sheet drew {}",
        template.path
    );
    assert!(
        near(
            [template.extent_mm[0], template.extent_mm[1]],
            [841.0, 594.0],
            0.5
        ),
        "the template's own extent is {:?}",
        template.extent_mm
    );
    // The `.igr` parses as 173 entities — 104 lines, 17 polylines, 18 arcs and
    // 34 labels. Every one of them is furniture and lands on the page.
    assert_eq!(template.primitives, 173, "furniture primitives");
    assert_eq!(
        model
            .primitives
            .iter()
            .filter(|primitive| primitive.furniture)
            .count(),
        173,
        "primitives flagged as furniture"
    );

    // The two frames: the outer one runs along the page edge, the inner one
    // sits 25 mm in on the binding side and 10 mm elsewhere (the draughting
    // margin), which is the same quad the reference plot draws.
    let boxes = frame_boxes(&model);
    assert!(
        boxes
            .iter()
            .any(|(min, max)| near(*min, [0.0, 0.0], 0.5) && near(*max, [841.0, 594.0], 0.5)),
        "no outer frame: {boxes:?}"
    );
    assert!(
        boxes
            .iter()
            .any(|(min, max)| near(*min, [25.0, 10.0], 0.5) && near(*max, [831.0, 584.0], 0.5)),
        "no inner frame: {boxes:?}"
    );

    let labels = labels(&model);
    for expected in [
        // sign-off column
        "会 签 专 业",
        "签     署",
        "日    期",
        "修改号",
        "次数",
        "页次",
        // title block
        "中国石油集团工程设计有限责任公司华北分公司",
        "设计证书号",
        "勘察证书号",
        "A111001641",
        "010065-kj",
        "华北油田苏桥储气库群",
    ] {
        assert!(
            labels.contains(&expected),
            "{A1_DRAWING}: no furniture label {expected:?}"
        );
    }

    // Every furniture label reaches the page, and letters at the width the
    // reference plot gives it.
    //
    // A template's text is drawn one 0.78 em glyph at a time, the sheet's own
    // text at the full em — so the furniture is neither wider nor narrower
    // than the reference. The figures below are measured off
    // `集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pdf`'s spans; the
    // two long ones carry the measured spread of the ratio (0.765–0.791) and
    // so get the looser gate. Body height is the other half of the rule and is
    // pinned separately: the reference letters these same labels at
    // 4.74 / 5.08 / 5.93 / 6.60 mm, which is the size the style states times
    // this writer's own `Tf` mapping.
    let dirs = font_dirs(Some(&corpus));
    if !dirs.iter().any(|dir| dir.is_dir()) {
        eprintln!("skipping the page render; no font directory is present");
        return;
    }
    let prepared = prepare_fonts(&[&model], &dirs).unwrap_or_else(|error| panic!("fonts: {error}"));
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
        })
        .collect();
    let pdf = write_plot(&model, &fonts).unwrap_or_else(|error| panic!("write: {error}"));
    let ink = read_plot(&pdf).unwrap_or_else(|error| panic!("read back: {error}"));
    for expected in [
        "中国石油集团工程设计有限责任公司华北分公司",
        "会 签 专 业",
        "勘察证书号",
    ] {
        assert!(
            ink.spans.iter().any(|span| span.text.trim() == expected),
            "{A1_DRAWING}: {expected:?} did not reach the page"
        );
    }
    for (label, width_mm, tolerance_mm, height_mm) in [
        ("\u{5ba1}  \u{5b9a}", 11.17, 0.3, 4.74),
        ("\u{8bbe}\u{8ba1}\u{8bc1}\u{4e66}\u{53f7}", 20.08, 0.5, 5.08),
        ("1 \u{7248}", 6.78, 0.2, 4.40),
        (
            "\u{534e}\u{5317}\u{6cb9}\u{7530}\u{82cf}\u{6865}\u{50a8}\u{6c14}\u{5e93}\u{7fa4}",
            45.33,
            1.0,
            5.93,
        ),
        (
            "\u{4e2d}\u{56fd}\u{77f3}\u{6cb9}\u{96c6}\u{56e2}\u{5de5}\u{7a0b}\u{8bbe}\u{8ba1}\u{6709}\u{9650}\u{8d23}\u{4efb}\u{516c}\u{53f8}\u{534e}\u{5317}\u{5206}\u{516c}\u{53f8}",
            107.46,
            1.5,
            6.60,
        ),
    ] {
        let span = ink
            .spans
            .iter()
            .find(|span| span.text.trim() == label)
            .unwrap_or_else(|| panic!("{A1_DRAWING}: {label:?} did not reach the page"));
        assert!(
            (span.advance_mm - width_mm).abs() <= tolerance_mm,
            "{label:?} letters {:.2} mm wide; the reference has {width_mm:.2}",
            span.advance_mm
        );
        assert!(
            (span.height_mm - height_mm).abs() <= 0.05,
            "{label:?} letters {:.2} mm high; the reference has {height_mm:.2}",
            span.height_mm
        );
    }
}

#[test]
fn a_sheet_no_template_matches_draws_no_furniture() {
    // `D06` is an A2 sheet. The corpus ships A0 and A1 templates only, so the
    // extent rule has to refuse the near-miss rather than stretching one of
    // them onto a smaller page.
    let fixture = fixture_dir().join("D06.pid");
    if !fixture.is_file() {
        eprintln!("skipping border template; {} is absent", fixture.display());
        return;
    }
    let corpus = fixture_dir();
    let model = lay_out_drawing(&fixture, &corpus).unwrap_or_else(|error| panic!("{error}"));
    assert!(
        model.template.is_none(),
        "D06 drew furniture from {:?}",
        model.template.as_ref().map(|template| &template.path)
    );
    assert!(
        !model.primitives.iter().any(|primitive| primitive.furniture),
        "D06 carries furniture primitives"
    );
    assert!(
        !labels(&model)
            .iter()
            .any(|label| label.contains("会 签") || label.contains("中国石油")),
        "D06 carries a title-block label"
    );
}
