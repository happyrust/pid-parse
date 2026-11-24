//! Factory Amyuni plots of four backup sheets, against the page we lay out.
//!
//! The plots are `publish/sppid导出/导出dxf pdf/导出pdf` under
//! `PID_TEST_DATA_DIR` (then the two local `pid-test-data` copies). Soft-skips
//! when that directory is absent, which is what CI sees.
//!
//! `EF井场`'s hand-drawn dot (oid 52844) is a filled circle of radius 0.463 mm.
//! The factory plot fills that circle; the 150 dpi driver snaps the outline
//! onto its pixel grid, and every vertex of the fill stays within one pixel
//! of the circle the `.pid` states. Our page fills the same circle as a
//! 26-point ring.
//!
//! `N井场`'s sideways `15-TD-4101416-B2-EH40` is clipped in the factory plot
//! the way the `.pid` states it: the first glyph's origin is on the device
//! grid at (262.128, −1.524) mm, under the edge. The model keeps the
//! frame-adjusted insertion and does not move the run onto the page.
//!
//! `D井场` is bare in the factory plot and on our page. The factory plot of
//! `凝析油一级闪蒸分离器B气相出口改造` letters the `CPECCHB-SQ-A2` title
//! block (certificate `A111017147`), and the `.pid` names no template, so the
//! page stays bare.

use std::path::{Path, PathBuf};

use pid_plot::model::{PaperModel, PaperShape, lay_out_drawing};
use pid_plot::read::{PlotInk, PlotSequence, read_plot, read_plot_sequence};

/// One device pixel of the 150 dpi plotter, in millimetres.
const PIXEL_MM: f64 = 25.4 / 150.0;

/// `EF井场` oid 52844, the circle the boundary fills, in the drawing's own
/// millimetres (the record's doubles, before the frame's range origin).
const EF_CENTRE_MM: [f64; 2] = [851.878_611_467_249_1, 130.386_382_989_912_9];
const EF_RADIUS_MM: f64 = 0.463_127_226_856_374_64;

/// Where the factory plot letters the first glyph of `N井场`'s sideways tag,
/// paper millimetres. Nine pixels under the edge, on the driver's grid.
const N_FACTORY_GLYPH_MM: [f64; 2] = [262.128, -1.524];

/// The insertion our page keeps for that tag. `PAST_THE_PAGE` in
/// `export_plant_backup` holds the same point.
const N_INSERTION_MM: [f64; 2] = [259.113_810, -0.207_788];

fn data_root() -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(dir) = std::env::var_os("PID_TEST_DATA_DIR") {
        roots.push(PathBuf::from(dir));
    }
    roots.push(PathBuf::from(r"D:\work\cad\pid-test-data"));
    roots.push(PathBuf::from(r"D:\work\plant-code\cad\pid-test-data"));
    roots.into_iter().find(|root| {
        root.join("backup-plant-data").is_dir()
            && root
                .join("publish")
                .join("sppid导出")
                .join("导出dxf pdf")
                .join("导出pdf")
                .is_dir()
    })
}

fn pdf_dir(root: &Path) -> PathBuf {
    root.join("publish")
        .join("sppid导出")
        .join("导出dxf pdf")
        .join("导出pdf")
}

fn backup(root: &Path) -> PathBuf {
    root.join("backup-plant-data")
}

fn read_pdf(path: &Path) -> PlotInk {
    let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    read_plot(&bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn read_sequence(path: &Path) -> PlotSequence {
    let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    read_plot_sequence(&bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn hypot(point: [f64; 2], centre: [f64; 2]) -> f64 {
    (point[0] - centre[0]).hypot(point[1] - centre[1])
}

fn text_of(model: &PaperModel) -> String {
    model
        .primitives
        .iter()
        .filter_map(|primitive| match &primitive.shape {
            PaperShape::Text { value, .. } => Some(value.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn pdf_text(ink: &PlotInk) -> String {
    ink.spans
        .iter()
        .map(|span| span.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The factory plot fills `EF井场`'s hand-drawn circle, and our page fills
/// the same circle as a 26-point ring.
#[test]
fn the_ef_dot_is_the_filled_circle_the_factory_plots() {
    let Some(root) = data_root() else {
        eprintln!("skipping the factory plots; no backup with its Amyuni PDFs is present");
        return;
    };
    let pid = backup(&root).join(r"00\00\EF井场 注采阀组工艺及自控流程图.pid");
    let pdf = pdf_dir(&root).join("EF井场 注采阀组工艺及自控流程图.pdf");
    let model =
        lay_out_drawing(&pid, &root).unwrap_or_else(|error| panic!("{}: {error}", pid.display()));
    let mut rings = Vec::new();
    for primitive in &model.primitives {
        let PaperShape::Fill { chain_mm, .. } = &primitive.shape else {
            continue;
        };
        if chain_mm.len() != 26 {
            continue;
        }
        let centre = [
            chain_mm.iter().map(|point| point[0]).sum::<f64>() / 26.0,
            chain_mm.iter().map(|point| point[1]).sum::<f64>() / 26.0,
        ];
        if hypot(centre, EF_CENTRE_MM) > 2.0 {
            continue;
        }
        let err = chain_mm
            .iter()
            .map(|point| (hypot(*point, centre) - EF_RADIUS_MM).abs())
            .fold(0.0_f64, f64::max);
        rings.push((centre, err));
    }
    assert_eq!(
        rings.len(),
        1,
        "one 26-point fill of the hand-drawn circle, found {rings:?}"
    );
    let (centre, err) = rings[0];
    assert!(
        err < 0.001,
        "the ring leaves the circle by {err} mm (centre {centre:?})"
    );

    let sequence = read_sequence(&pdf);
    let mut vertices = Vec::new();
    for stroke in &sequence.strokes {
        if !stroke.filled {
            continue;
        }
        for point in [stroke.segment.start_mm, stroke.segment.end_mm] {
            let near = hypot(point, EF_CENTRE_MM) < 2.0;
            let fresh = !vertices
                .iter()
                .any(|kept: &[f64; 2]| hypot(*kept, point) < 0.01);
            if near && fresh {
                vertices.push(point);
            }
        }
    }
    assert!(
        (6..=16).contains(&vertices.len()),
        "the factory fill of the dot has {} vertices, {vertices:?}",
        vertices.len()
    );
    for point in &vertices {
        let radial = hypot(*point, EF_CENTRE_MM);
        assert!(
            (radial - EF_RADIUS_MM).abs() < PIXEL_MM,
            "a factory vertex {point:?} is {radial} mm from the circle's centre \
             (radius {EF_RADIUS_MM}, one pixel is {PIXEL_MM})"
        );
    }
}

/// The factory plot clips `N井场`'s tag where the `.pid` states it. The model
/// keeps the frame-adjusted insertion.
#[test]
fn the_n_label_stays_where_the_pid_puts_it() {
    let Some(root) = data_root() else {
        eprintln!("skipping the factory plots; no backup with its Amyuni PDFs is present");
        return;
    };
    let pid = backup(&root).join(r"00\00\N井场 注采阀组工艺及自控流程图.pid");
    let pdf = pdf_dir(&root).join("N井场 注采阀组工艺及自控流程图.pdf");
    let model =
        lay_out_drawing(&pid, &root).unwrap_or_else(|error| panic!("{}: {error}", pid.display()));
    let mut insertions = Vec::new();
    for primitive in &model.primitives {
        let PaperShape::Text {
            origin_mm, value, ..
        } = &primitive.shape
        else {
            continue;
        };
        if value == "15-TD-4101416-B2-EH40" {
            insertions.push(*origin_mm);
        }
    }
    assert_eq!(insertions.len(), 1, "the tag is on the page once");
    for axis in 0..2 {
        let delta = (insertions[0][axis] - N_INSERTION_MM[axis]).abs();
        assert!(
            delta < 0.0001,
            "insertion {:?} against the pin {N_INSERTION_MM:?} (delta {delta} mm)",
            insertions[0]
        );
    }

    let ink = read_pdf(&pdf);
    let spans: Vec<_> = ink
        .spans
        .iter()
        .filter(|span| span.text == "15-TD-4101416-B2-EH40")
        .collect();
    assert_eq!(spans.len(), 1, "the factory plot letters the tag once");
    let origin = spans[0].origin_mm;
    assert!(
        origin[1] < 0.0,
        "the factory plot's first glyph starts under the edge, at {origin:?}"
    );
    for axis in 0..2 {
        assert!(
            (origin[axis] - N_FACTORY_GLYPH_MM[axis]).abs() < 0.05,
            "factory glyph origin {origin:?}, pinned {N_FACTORY_GLYPH_MM:?}"
        );
    }
}

/// `D井场` wears no border in the factory plot or on our page. The gas-outlet
/// sheet's factory plot does, and the `.pid` names no template to wear.
#[test]
fn bare_sheets_follow_the_factory_plot_only_when_the_pid_names_a_template() {
    let Some(root) = data_root() else {
        eprintln!("skipping the factory plots; no backup with its Amyuni PDFs is present");
        return;
    };
    let cases = [
        (
            r"00\00\D井场 注采阀组工艺及自控流程图.pid",
            "D井场 注采阀组工艺及自控流程图.pdf",
            false,
        ),
        (
            r"00\02\集注站 凝析油一级闪蒸分离器B气相出口改造工艺及自控流程图.pid",
            "集注站 凝析油一级闪蒸分离器B气相出口改造工艺及自控流程图.pdf",
            true,
        ),
    ];
    for (relative, pdf_name, factory_has_border) in cases {
        let pid = backup(&root).join(relative);
        let model = lay_out_drawing(&pid, &root)
            .unwrap_or_else(|error| panic!("{}: {error}", pid.display()));
        assert!(
            model.template.is_none(),
            "{} names no template and wears none",
            pid.display()
        );
        assert!(
            model.missing_templates.is_empty(),
            "{}: unanswered names {:?}",
            pid.display(),
            model.missing_templates
        );
        assert!(
            !text_of(&model).contains("A111017147"),
            "{} draws the CPECCHB certificate",
            pid.display()
        );
        let ink = read_pdf(&pdf_dir(&root).join(pdf_name));
        let text = pdf_text(&ink);
        let has_border = text.contains("A111017147");
        assert_eq!(
            has_border, factory_has_border,
            "{pdf_name}: factory title block present {has_border}"
        );
    }
}
