//! The page draws the sheet furniture its linked `.igr` template carries.
//!
//! A P&ID's border is a linked OLE frame, so the double frame, the title block
//! and the sign-off column are not in the `.pid` at all: they live in the
//! `.igr` template the frame record points at. This gate pins that the page
//! finds that template, draws it at the sheet it was authored for, letters
//! every one of its labels, and expands the one symbol the template places.
//!
//! Three paper sizes are gated, and each is measured. The A1 case
//! (`CPE-SQ-A1.igr`) letters the title block within the figures its reference
//! plot gives; the A0 case (`CPE-SQ-A0.igr`, `I井场`) does the same against its
//! own reference plot, whose figures are the A1 ones to 0.02 mm — one set of
//! sizes across both sheets — and its furniture sits 348.00 mm further right,
//! which is exactly the difference between the two page widths.
//!
//! The A2 case is the repository's own pair: `D06.pid` with
//! `CPECCHB-SQ-A2.igr`, which landed in `test-file/Ref/Template Files` on
//! 2026-10-28. It is a different template family from `CPE-SQ`, so its frame
//! insets (35.354 mm on the left, 7.071 mm on the other three), its stroke
//! census (72 lines, 1 polyline, 7 fills, 36 labels) and its cells are its own;
//! none of the A1 figures is copied onto it. Until that file was in the
//! repository this gate asserted the opposite for `D06` — that an A2 sheet drew
//! no furniture — and passed because the template directory held A0/A1 only,
//! not because the page had no frame.
//!
//! The second A2 case is `A01`, whose frame is embedded rather than linked and
//! so names nothing: `CPE-SQ-A2.igr` answers it by extent alone. That file is
//! the A1 family's own A2 — the same 175 strokes, the same 34 labels, the
//! `比例` wave 247.000 mm left (841 − 594) — and it joined
//! `test-file/Ref/Template Files` on 2026-11-01 from the corpus's own copy, so
//! the case no longer needs the authoring machine to be read.
//!
//! Soft-skips when the corpus or the fixture is absent, which is what CI sees.

use std::path::{Path, PathBuf};

use pid_parse::symbol_library::NameMiss;
use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperModel, PaperShape, SymbolBodySource, lay_out_drawing};
use pid_plot::read::read_plot;
use pid_plot::write::{PlotFont, write_plot};

const A1_DRAWING: &str = "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图";

/// The corpus's one A0 sheet: 1189 × 841 mm, and the same `比例` wave the A1
/// pages carry, 348.00 mm further right.
const A0_DRAWING: &str = "I井场 注采阀组工艺及自控流程图";

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

/// Whether a furniture line runs between `a` and `b`, in either direction.
fn has_line(model: &PaperModel, a: [f64; 2], b: [f64; 2], tolerance: f64) -> bool {
    model
        .primitives
        .iter()
        .filter(|primitive| primitive.furniture)
        .any(|primitive| {
            let PaperShape::Line { start_mm, end_mm } = &primitive.shape else {
                return false;
            };
            (near(*start_mm, a, tolerance) && near(*end_mm, b, tolerance))
                || (near(*start_mm, b, tolerance) && near(*end_mm, a, tolerance))
        })
}

/// The two endpoints an arc primitive sweeps between, paper millimetres.
fn arc_ends(shape: &PaperShape) -> Option<([f64; 2], [f64; 2])> {
    let PaperShape::Arc {
        center_mm,
        radius_mm,
        start_angle,
        end_angle,
        ..
    } = shape
    else {
        return None;
    };
    let at = |angle: f64| {
        [
            center_mm[0] + radius_mm * angle.cos(),
            center_mm[1] + radius_mm * angle.sin(),
        ]
    };
    Some((at(*start_angle), at(*end_angle)))
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
    // The `.igr` parses as 174 entities — 104 lines, 17 polylines, 18 arcs, 34
    // labels and one `igSymbol2d`. Every one of them is furniture, and the
    // placement expands the two arcs of its cached body, so 175 primitives
    // land on the page.
    assert_eq!(template.primitives, 175, "furniture primitives");
    assert_eq!(
        model
            .primitives
            .iter()
            .filter(|primitive| primitive.furniture)
            .count(),
        175,
        "primitives flagged as furniture"
    );

    // The `比例` value cell's wave: the two arcs of the symbol the template
    // places there (`BLFH.sym`, whose body is cached in the template's
    // `/JSite16`). This is the ink that was the last 14.2 mm no page
    // explained — the reference plot inks the same chain 0.00–0.11 mm away.
    let mut wave: Vec<([f64; 2], [f64; 2])> = model
        .primitives
        .iter()
        .filter(|primitive| primitive.furniture)
        .filter_map(|primitive| arc_ends(&primitive.shape))
        .filter(|(start, end)| {
            [start, end].iter().all(|point| {
                (740.0..=764.0).contains(&point[0]) && (17.0..=21.0).contains(&point[1])
            })
        })
        .collect();
    wave.sort_by(|left, right| left.0[0].total_cmp(&right.0[0]));
    assert_eq!(wave.len(), 2, "the 比例 cell's wave: {wave:?}");
    assert!(near(wave[0].0, [750.39, 19.41], 0.1), "{:?}", wave[0].0);
    assert!(near(wave[0].1, [752.69, 19.00], 0.1), "{:?}", wave[0].1);
    assert!(near(wave[1].0, [752.69, 19.00], 0.1), "{:?}", wave[1].0);
    assert!(near(wave[1].1, [755.01, 18.56], 0.1), "{:?}", wave[1].1);

    // The template's own symbol account, which is what tells a source gap from
    // a decode miss. The wave is the only placement the template carries, and
    // the only body it can be drawn from is the copy the `.igr` holds: the
    // placement names an absolute path on the authoring machine
    // (`…\Temp\BLFH.sym`), a spelling no library root can be addressed with.
    // So reference ink left near this wave is a second version of that body,
    // not a `.sym` the corpus forgot to ship — and the absent source must not
    // reach the drawing's own missing-symbols report either.
    assert_eq!(template.symbols.len(), 1, "{:?}", template.symbols);
    let symbol = &template.symbols[0];
    assert!(
        symbol.key.to_ascii_lowercase().ends_with("blfh.sym"),
        "the template places {} instead",
        symbol.key
    );
    assert_eq!(
        symbol.body,
        SymbolBodySource::Cached,
        "{} drew from {:?}",
        symbol.key,
        symbol.body
    );
    assert!(
        !symbol.library_available,
        "{} resolved a library body",
        symbol.key
    );
    // And the reason is the file, not a name the library holds twice: the last
    // 31 sheets' ledger says so, and the two causes are accounted for apart.
    assert_eq!(
        symbol.name_gap,
        Some(NameMiss::NoSuchFile),
        "{} was refused for some other reason",
        symbol.key
    );
    assert!(
        symbol.extent_mm.is_some(),
        "the placement drew nothing; the wave is from its cache"
    );
    assert!(
        model
            .template_symbol_at(symbol.insertion_mm, 0.0)
            .is_some_and(|owner| owner.key == symbol.key),
        "the placement does not own its own insertion point"
    );
    assert!(
        !model
            .missing_symbols
            .iter()
            .any(|missing| missing.to_ascii_lowercase().contains("blfh")),
        "the template's absent .sym leaked into missing_symbols: {:?}",
        model.missing_symbols
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
            subset: face.subset,
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
fn the_a0_sheet_draws_the_frame_title_block_and_the_same_wave() {
    let corpus = corpus_dir();
    let drawing = corpus.join(format!("{A0_DRAWING}.pid"));
    if !drawing.is_file() {
        eprintln!("skipping border template; {} is absent", drawing.display());
        return;
    }
    let model = lay_out_drawing(&drawing, &corpus).unwrap_or_else(|error| panic!("{error}"));

    let template = model
        .template
        .as_ref()
        .unwrap_or_else(|| panic!("{A0_DRAWING}: no border template was drawn"));
    assert!(
        template.path.ends_with("CPE-SQ-A0.igr"),
        "the A0 sheet drew {}",
        template.path
    );
    assert!(
        near(
            [template.extent_mm[0], template.extent_mm[1]],
            [1189.0, 841.0],
            0.5
        ),
        "the template's own extent is {:?}",
        template.extent_mm
    );
    // The same census as the A1 template: 174 entities — 104 lines, 17
    // polylines, 18 arcs, 34 labels and one `igSymbol2d` — where the symbol
    // expands into the two arcs of its cached body.
    assert_eq!(template.primitives, 175, "furniture primitives");
    assert_eq!(
        model
            .primitives
            .iter()
            .filter(|primitive| primitive.furniture)
            .count(),
        175,
        "primitives flagged as furniture"
    );
    assert_eq!(
        model
            .primitives
            .iter()
            .filter(|primitive| primitive.furniture)
            .filter(|primitive| matches!(primitive.shape, PaperShape::Text { .. }))
            .count(),
        34,
        "furniture labels"
    );

    // The `比例` value cell's wave, at the A0 insertion point: the two arcs of
    // the body the template caches. The A1 pages place the same symbol at
    // x = 752.692, which is 348.000 mm less — the difference between the two
    // page widths (1189 − 841). The reference plots ink the same chain there,
    // 0.02 mm from this insertion point.
    let mut wave: Vec<([f64; 2], [f64; 2])> = model
        .primitives
        .iter()
        .filter(|primitive| primitive.furniture)
        .filter_map(|primitive| arc_ends(&primitive.shape))
        .filter(|(start, end)| {
            [start, end].iter().all(|point| {
                (1090.0..=1112.0).contains(&point[0]) && (17.0..=21.0).contains(&point[1])
            })
        })
        .collect();
    wave.sort_by(|left, right| left.0[0].total_cmp(&right.0[0]));
    assert_eq!(wave.len(), 2, "the 比例 cell's wave: {wave:?}");
    assert!(near(wave[0].0, [1098.39, 19.41], 0.1), "{:?}", wave[0].0);
    assert!(near(wave[0].1, [1100.69, 19.00], 0.1), "{:?}", wave[0].1);
    assert!(near(wave[1].0, [1100.69, 19.00], 0.1), "{:?}", wave[1].0);
    assert!(near(wave[1].1, [1103.01, 18.56], 0.1), "{:?}", wave[1].1);

    // The same source account as the A1 sheet, on the same symbol: an absolute
    // path on the authoring machine, no library root that can address it, one
    // placement, and a cached body that draws. The two causes of a missing
    // body are told apart, and this one is the file.
    assert_eq!(template.symbols.len(), 1, "{:?}", template.symbols);
    let symbol = &template.symbols[0];
    assert!(
        symbol.key.to_ascii_lowercase().ends_with("blfh.sym"),
        "the template places {} instead",
        symbol.key
    );
    assert!(
        near(symbol.insertion_mm, [1100.692, 19.000], 0.001),
        "{:?}",
        symbol.insertion_mm
    );
    assert_eq!(
        symbol.body,
        SymbolBodySource::Cached,
        "{} drew from {:?}",
        symbol.key,
        symbol.body
    );
    assert!(
        !symbol.library_available,
        "{} resolved a library body",
        symbol.key
    );
    assert_eq!(
        symbol.name_gap,
        Some(NameMiss::NoSuchFile),
        "{} was refused for some other reason",
        symbol.key
    );
    let extent = symbol
        .extent_mm
        .unwrap_or_else(|| panic!("{} drew nothing; the wave is from its cache", symbol.key));
    assert!(
        near(extent[0], [1098.39, 18.56], 0.05) && near(extent[1], [1103.01, 19.41], 0.05),
        "{} reaches {extent:?}",
        symbol.key
    );
    assert!(
        model
            .template_symbol_at(symbol.insertion_mm, 0.0)
            .is_some_and(|owner| owner.key == symbol.key),
        "the placement does not own its own insertion point"
    );
    assert!(
        !model
            .missing_symbols
            .iter()
            .any(|missing| missing.to_ascii_lowercase().contains("blfh")),
        "the template's absent .sym leaked into missing_symbols: {:?}",
        model.missing_symbols
    );
    assert!(
        model.missing_boxes.is_empty(),
        "the placement drew a placeholder box: {:?}",
        model.missing_boxes
    );

    // The two frames at A0: the outer one on the page edge, the inner one 25 mm
    // in on the binding side and 10 mm elsewhere.
    let boxes = frame_boxes(&model);
    assert!(
        boxes
            .iter()
            .any(|(min, max)| near(*min, [0.0, 0.0], 0.5) && near(*max, [1189.0, 841.0], 0.5)),
        "no outer frame: {boxes:?}"
    );
    assert!(
        boxes
            .iter()
            .any(|(min, max)| near(*min, [25.0, 10.0], 0.5) && near(*max, [1179.0, 831.0], 0.5)),
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
            "{A0_DRAWING}: no furniture label {expected:?}"
        );
    }

    // The A0 page letters the title block exactly as the A1 page does. The
    // figure pairs are the ones the A1 gate pins off its reference plot, and
    // the A0 reference plot inks the same labels at the same width and height
    // — so the two sheets were drawn from one set of sizes, not two. Our page
    // is then measured against that reference, label by label.
    let reference_path = corpus.join(format!("{A0_DRAWING}.pdf"));
    if !reference_path.is_file() {
        eprintln!(
            "skipping the lettering measure; {} is absent",
            reference_path.display()
        );
        return;
    }
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
            subset: face.subset,
        })
        .collect();
    let pdf = write_plot(&model, &fonts).unwrap_or_else(|error| panic!("write: {error}"));
    let ink = read_plot(&pdf).unwrap_or_else(|error| panic!("read back: {error}"));
    let reference_bytes = std::fs::read(&reference_path)
        .unwrap_or_else(|error| panic!("{}: {error}", reference_path.display()));
    let reference =
        read_plot(&reference_bytes).unwrap_or_else(|error| panic!("reference read: {error}"));
    for (label, a1_width_mm, a1_height_mm) in [
        ("审  定", 11.17, 4.74),
        ("设计证书号", 20.08, 5.08),
        ("1 版", 6.78, 4.40),
        ("华北油田苏桥储气库群", 45.33, 5.93),
        ("中国石油集团工程设计有限责任公司华北分公司", 107.46, 6.60),
    ] {
        let theirs = reference
            .spans
            .iter()
            .find(|span| span.text.trim() == label)
            .unwrap_or_else(|| panic!("{A0_DRAWING}: the reference does not letter {label:?}"));
        assert!(
            (theirs.advance_mm - a1_width_mm).abs() <= 0.02,
            "the A0 reference letters {label:?} {:.2} mm wide; the A1 reference has {a1_width_mm:.2}",
            theirs.advance_mm
        );
        assert!(
            (theirs.height_mm - a1_height_mm).abs() <= 0.02,
            "the A0 reference letters {label:?} {:.2} mm high; the A1 reference has {a1_height_mm:.2}",
            theirs.height_mm
        );
        let ours = ink
            .spans
            .iter()
            .find(|span| span.text.trim() == label)
            .unwrap_or_else(|| panic!("{A0_DRAWING}: {label:?} did not reach the page"));
        assert!(
            (ours.advance_mm - theirs.advance_mm).abs() <= 0.025 * theirs.advance_mm + 0.05,
            "{label:?} letters {:.2} mm wide; the reference has {:.2}",
            ours.advance_mm,
            theirs.advance_mm
        );
        assert!(
            (ours.height_mm - a1_height_mm).abs() <= 0.05,
            "{label:?} letters {:.2} mm high; the reference has {a1_height_mm:.2}",
            ours.height_mm
        );
    }
}

#[test]
fn an_a2_sheet_draws_its_linked_a2_template() {
    // `D06` states 594.33 × 420.57 mm. The A2 CCHB template measures
    // 593.94 × 420.00 — 0.57 mm inside it — so the extent rule takes it. The
    // sheet's link record names `XIONGANA2.IGR`, a file no corpus in this
    // repository ships, which is why the pick here is the extent's.
    let fixture = fixture_dir().join("D06.pid");
    if !fixture.is_file() {
        eprintln!("skipping border template; {} is absent", fixture.display());
        return;
    }
    let corpus = fixture_dir();
    let model = lay_out_drawing(&fixture, &corpus).unwrap_or_else(|error| panic!("{error}"));

    let template = model
        .template
        .as_ref()
        .unwrap_or_else(|| panic!("D06 drew no furniture"));
    assert!(
        template.path.ends_with("CPECCHB-SQ-A2.igr"),
        "D06 drew {}",
        template.path
    );
    assert!(
        near(
            [template.extent_mm[0], template.extent_mm[1]],
            [593.94, 420.00],
            0.5
        ),
        "the template's own extent is {:?}",
        template.extent_mm
    );
    // The A2 CCHB census: 72 lines, one polyline, 7 fills and 36 labels. Not
    // the A1 CPE-SQ one (175 strokes, 34 labels) — a different family, drawn
    // to different margins.
    assert_eq!(template.primitives, 116, "furniture primitives");
    assert_eq!(
        model
            .primitives
            .iter()
            .filter(|primitive| primitive.furniture)
            .count(),
        116,
        "primitives flagged as furniture"
    );
    assert_eq!(
        model
            .primitives
            .iter()
            .filter(|primitive| primitive.furniture)
            .filter(|primitive| matches!(primitive.shape, PaperShape::Text { .. }))
            .count(),
        36,
        "furniture labels"
    );

    // The outer frame is one polyline along the page edge; the inner one is
    // four lines at 35.354 mm from the left edge and 7.071 mm from the other
    // three. The A2 CCHB insets are its own — the CPE-SQ family uses 25 mm and
    // 10 mm — so they are read here rather than borrowed.
    let boxes = frame_boxes(&model);
    assert!(
        boxes
            .iter()
            .any(|(min, max)| near(*min, [0.0, 0.0], 0.5) && near(*max, [593.94, 420.00], 0.5)),
        "no outer frame: {boxes:?}"
    );
    for (start, end) in [
        ([35.354, 7.071], [586.869, 7.071]),
        ([586.869, 7.071], [586.869, 412.929]),
        ([586.869, 412.929], [35.354, 412.929]),
        ([35.354, 412.929], [35.354, 7.071]),
    ] {
        assert!(
            has_line(&model, start, end, 0.05),
            "no inner frame edge {start:?} -> {end:?}"
        );
    }

    let labels = labels(&model);
    for expected in [
        // sign-off block
        "会 签 专 业",
        "签     署",
        "日    期",
        "审核",
        "校对",
        "设计",
        "制图",
        // title block
        "中国石油工程建设有限公司 ",
        "华北分公司",
        "CPECC",
        "项目号",
        "CADD号",
        "文件号",
        "专业",
        "版次",
        "阶段",
        "比例",
        "工程设计证书: A111017147 综甲     工程勘察证书: B111017147 综甲",
    ] {
        assert!(
            labels.contains(&expected),
            "D06: no furniture label {expected:?}"
        );
    }

    // The three placements, each drawn from the body the `.igr` caches. The
    // stored paths are absolute paths on the authoring machine
    // (`c:\users\administrator…\a$c40d91a4b.sym`, `…\hqz.sym` via its 8.3
    // spelling), so no library root can address them; the page must still get
    // their ink and no placeholder box.
    assert_eq!(template.symbols.len(), 3, "{:?}", template.symbols);
    for symbol in &template.symbols {
        assert_eq!(
            symbol.body,
            SymbolBodySource::Cached,
            "{} drew from {:?}",
            symbol.key,
            symbol.body
        );
        assert!(
            !symbol.library_available,
            "{} resolved a library body",
            symbol.key
        );
        assert_eq!(
            symbol.name_gap,
            Some(NameMiss::NoSuchFile),
            "{} was refused for some other reason",
            symbol.key
        );
        assert!(
            symbol.extent_mm.is_some(),
            "{} drew nothing; the strokes are from its cache",
            symbol.key
        );
        // A cached body's ink can start a fraction of a millimetre off the
        // point that places it — the `a$…` body begins 0.35 mm above its own —
        // so the owner is read with a millimetre of slack, which still names
        // one placement and not its neighbour.
        assert!(
            model
                .template_symbol_at(symbol.insertion_mm, 1.0)
                .is_some_and(|owner| owner.key == symbol.key),
            "the placement does not own its own insertion point"
        );
    }
    assert!(
        template.symbols.iter().any(|symbol| {
            symbol.key.to_ascii_lowercase().ends_with("a$c40d91a4b.sym")
                && near(symbol.insertion_mm, [0.0, 59.040], 0.02)
        }),
        "no `a$c40d91a4b.sym` at its insertion point: {:?}",
        template.symbols
    );
    let hqz: Vec<[f64; 2]> = template
        .symbols
        .iter()
        .filter(|symbol| symbol.key.to_ascii_lowercase().ends_with("hqz.sym"))
        .map(|symbol| symbol.insertion_mm)
        .collect();
    assert_eq!(hqz.len(), 2, "{hqz:?}");
    for expected in [[35.354, 412.929], [35.354, 306.869]] {
        assert!(
            hqz.iter().any(|insertion| near(*insertion, expected, 0.02)),
            "no `hqz.sym` at {expected:?}: {hqz:?}"
        );
    }
    assert!(
        model.missing_symbols.is_empty(),
        "the template's own gaps entered the drawing's report: {:?}",
        model.missing_symbols
    );
    assert!(
        model.missing_boxes.is_empty(),
        "a placement drew a placeholder box: {:?}",
        model.missing_boxes
    );

    // And the furniture reaches the page. There is no reference plot for this
    // fixture, so the labels are read back by their characters, not by widths.
    let dirs = font_dirs(Some(&corpus_dir()));
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
            subset: face.subset,
        })
        .collect();
    let pdf = write_plot(&model, &fonts).unwrap_or_else(|error| panic!("write: {error}"));
    let ink = read_plot(&pdf).unwrap_or_else(|error| panic!("read back: {error}"));
    for expected in [
        "中国石油工程建设有限公司",
        "华北分公司",
        "会 签 专 业",
        "CPECC",
    ] {
        assert!(
            ink.spans.iter().any(|span| span.text.trim() == expected),
            "D06: {expected:?} did not reach the page"
        );
    }

    // The revision cells' `A$C40D91A4B.sym` lettering states a width of
    // `1000` (`JStyleTextChar +52`) where every other template style states
    // `800`, and the A2 reference plot letters it at the full em: `修改`
    // 10.50 mm at 3.5354 mm, `修改号` 15.75. At the plate's 0.78 it was 8.19.
    for (label, width_mm) in [("修改", 10.50), ("修改号", 15.75)] {
        let ours = ink
            .spans
            .iter()
            .find(|span| span.text.trim() == label)
            .unwrap_or_else(|| panic!("D06: {label:?} did not reach the page"));
        assert!(
            (ours.advance_mm - width_mm).abs() <= 0.05,
            "D06: {label:?} letters {:.2} mm wide; the A2 reference has {width_mm:.2}",
            ours.advance_mm
        );
    }
}

#[test]
fn the_a01_sheets_embedded_frame_draws_the_a2_template_it_states() {
    // `A01`'s frame record is `Embedded / LocallyLinked`: the object it holds
    // is stored in the drawing rather than linked to a file, and it declares
    // 594.00 x 420.00 — A2. The furniture itself is not in the `.pid` (no
    // `Ole` stream, no OLE storage anywhere in its six `JSite`s), so the page
    // can only be completed the way an unnamed sheet always is: by the one
    // template whose own ink is that extent.
    //
    // Until `CPE-SQ-A2.igr` landed beside the A0/A1 files nothing in the
    // repository could answer that, so the case was only ever readable against
    // the corpus on the authoring machine. It is the A1 file's own family one
    // size down, and its census is the A1 census: 175 strokes, 34 labels, and
    // the `比例` wave 247.000 mm to the left — which is 841 - 594, the
    // difference between the two page widths, exactly as the A0 case is the
    // same wave 348.000 mm to the right.
    let fixture = fixture_dir().join("export-test/publish-data/A01/A01.pid");
    if !fixture.is_file() {
        eprintln!("skipping border template; {} is absent", fixture.display());
        return;
    }
    let corpus = fixture_dir();
    let model = lay_out_drawing(&fixture, &corpus).unwrap_or_else(|error| panic!("{error}"));
    assert!(
        near(model.page_mm.unwrap_or([0.0, 0.0]), [594.0, 420.0], 0.01),
        "A01 states {:?}",
        model.page_mm
    );

    let template = model
        .template
        .as_ref()
        .unwrap_or_else(|| panic!("A01's embedded frame drew no furniture"));
    assert!(
        template.path.ends_with("CPE-SQ-A2.igr"),
        "A01 drew {}",
        template.path
    );
    assert!(
        near(
            [template.extent_mm[0], template.extent_mm[1]],
            [594.0, 420.0],
            0.5
        ),
        "the template's own extent is {:?}",
        template.extent_mm
    );
    assert_eq!(template.primitives, 175, "furniture primitives");
    assert_eq!(
        model
            .primitives
            .iter()
            .filter(|primitive| primitive.furniture)
            .count(),
        175,
        "primitives flagged as furniture"
    );
    assert_eq!(
        model
            .primitives
            .iter()
            .filter(|primitive| primitive.furniture)
            .filter(|primitive| matches!(primitive.shape, PaperShape::Text { .. }))
            .count(),
        34,
        "furniture labels"
    );

    // The two frames: the outer one on the page edge, the inner one 25 mm in on
    // the binding side and 10 mm elsewhere — the CPE-SQ margins, not the A2
    // CCHB template's 35.354 / 7.071.
    let boxes = frame_boxes(&model);
    assert!(
        boxes
            .iter()
            .any(|(min, max)| near(*min, [0.0, 0.0], 0.5) && near(*max, [594.0, 420.0], 0.5)),
        "no outer frame: {boxes:?}"
    );
    assert!(
        boxes
            .iter()
            .any(|(min, max)| near(*min, [25.0, 10.0], 0.5) && near(*max, [584.0, 410.0], 0.5)),
        "no inner frame: {boxes:?}"
    );

    // The `比例` value cell's wave, moved with the page: the A1 page places the
    // same symbol's two arcs at 750.39 / 752.69 / 755.01, this one at those
    // less 247.000. The template's ink is the sheet's to the millimetre, so the
    // wave is read at its own insertion point rather than copied from A1.
    let mut wave: Vec<([f64; 2], [f64; 2])> = model
        .primitives
        .iter()
        .filter(|primitive| primitive.furniture)
        .filter_map(|primitive| arc_ends(&primitive.shape))
        .filter(|(start, end)| {
            [start, end].iter().all(|point| {
                (493.0..=513.0).contains(&point[0]) && (17.0..=21.0).contains(&point[1])
            })
        })
        .collect();
    wave.sort_by(|left, right| left.0[0].total_cmp(&right.0[0]));
    assert_eq!(wave.len(), 2, "the 比例 cell's wave: {wave:?}");
    assert!(near(wave[0].0, [503.39, 19.41], 0.1), "{:?}", wave[0].0);
    assert!(near(wave[0].1, [505.69, 19.00], 0.1), "{:?}", wave[0].1);
    assert!(near(wave[1].0, [505.69, 19.00], 0.1), "{:?}", wave[1].0);
    assert!(near(wave[1].1, [508.01, 18.56], 0.1), "{:?}", wave[1].1);

    // One placement, drawn from the template's own cache. The `.sym` it names
    // is an absolute path on the authoring machine, so no library root can
    // address it; the body the `.igr` keeps is what the wave is.
    assert_eq!(template.symbols.len(), 1, "{:?}", template.symbols);
    let symbol = &template.symbols[0];
    assert!(
        symbol.key.to_ascii_lowercase().ends_with("blfh.sym"),
        "the template places {} instead",
        symbol.key
    );
    assert!(
        near(symbol.insertion_mm, [505.692, 19.000], 0.001),
        "{:?}",
        symbol.insertion_mm
    );
    assert_eq!(
        symbol.body,
        SymbolBodySource::Cached,
        "{} drew from {:?}",
        symbol.key,
        symbol.body
    );
    assert!(
        !symbol.library_available,
        "{} resolved a library body",
        symbol.key
    );
    assert_eq!(
        symbol.name_gap,
        Some(NameMiss::NoSuchFile),
        "{} was refused for some other reason",
        symbol.key
    );
    assert!(
        symbol.extent_mm.is_some(),
        "the placement drew nothing; the wave is from its cache"
    );
    assert!(
        !model
            .missing_symbols
            .iter()
            .any(|missing| missing.to_ascii_lowercase().contains("blfh")),
        "the template's absent .sym leaked into missing_symbols: {:?}",
        model.missing_symbols
    );
    assert!(
        model.missing_boxes.is_empty(),
        "the placement drew a placeholder box: {:?}",
        model.missing_boxes
    );

    // And the furniture reaches the page. A01 has no reference plot — it is a
    // publish fixture, not a corpus drawing — so the labels are read back by
    // their characters rather than by measured widths.
    let labels = labels(&model);
    for expected in [
        "会 签 专 业",
        "签     署",
        "日    期",
        "修改号",
        "次数",
        "页次",
        "中国石油集团工程设计有限责任公司华北分公司",
        "设计证书号",
        "勘察证书号",
        "A111001641",
        "010065-kj",
        "华北油田苏桥储气库群",
    ] {
        assert!(
            labels.contains(&expected),
            "A01: no furniture label {expected:?}"
        );
    }

    let dirs = font_dirs(Some(&corpus_dir()));
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
            subset: face.subset,
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
            "A01: {expected:?} did not reach the page"
        );
    }
}
