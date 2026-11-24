//! Factory Amyuni plots of the six backup sheets whose border template sits
//! past a corner of the paper, and `N井场`'s sideways label under the bottom
//! edge (#59).
//!
//! Soft-skips when no backup directory carrying `publish/sppid导出/导出dxf
//! pdf/导出pdf` is present, which is what CI sees.

use std::path::{Path, PathBuf};

use pid_plot::model::{PaperModel, PaperShape, lay_out_drawing};
use pid_plot::read::{read_plot, read_plot_pens};
use pid_plot::witness::{
    PAPER_EDGE_OFFSET_SLACK_MM, PAPER_EDGE_WITNESSES, PaperEdgeSource, paper_edge_witness,
};

/// One device pixel of the 150 dpi Amyuni plotter, in millimetres.
const PIXEL_MM: f64 = 25.4 / 150.0;

struct FactorySheet {
    relative_pid: &'static str,
    pdf_stem: &'static str,
    template_stem: &'static str,
    paper_mm: [f64; 2],
    offset_mm: [f64; 2],
    factory_draws_border: bool,
}

/// Backup sheets whose border template sits off a corner of the paper and
/// whose factory Amyuni plot is present.
///
/// `1200td`'s factory plot is plotted bare on A1 paper (`840.9986 × 593.9991`
/// mm) rather than its `843.09 × 596.09` frame; its sibling `600td` plots
/// `CPE-SQ-A1` off-corner clipped to A1 paper.
const SIX_SHEETS: [FactorySheet; 6] = [
    FactorySheet {
        relative_pid: r"00\00\A井场 注采阀组工艺及自控流程图.pid",
        pdf_stem: "A井场 注采阀组工艺及自控流程图",
        template_stem: "CPE-SQ-A0",
        paper_mm: [1189.0, 841.0],
        offset_mm: [0.152_593, 0.152_593],
        factory_draws_border: true,
    },
    FactorySheet {
        relative_pid: r"00\00\N井场 注采阀组工艺及自控流程图.pid",
        pdf_stem: "N井场 注采阀组工艺及自控流程图",
        template_stem: "CPECCHB-SQ-A1",
        paper_mm: [841.0, 594.0],
        offset_mm: [1.297_400, 1.553_387],
        factory_draws_border: true,
    },
    FactorySheet {
        relative_pid: r"00\00\O井场 注采阀组工艺及自控流程图.pid",
        pdf_stem: "O井场 注采阀组工艺及自控流程图",
        template_stem: "CPECCHB-SQ-A1",
        paper_mm: [841.0, 594.0],
        offset_mm: [0.150_015, 0.399_965],
        factory_draws_border: true,
    },
    FactorySheet {
        relative_pid: r"00\00\P井场 注采阀组工艺及自控流程图.pid",
        pdf_stem: "P井场 注采阀组工艺及自控流程图",
        template_stem: "CPECCHB-SQ-A1",
        paper_mm: [841.0, 594.0],
        offset_mm: [0.150_015, 0.399_965],
        factory_draws_border: true,
    },
    FactorySheet {
        relative_pid: r"00\00\集注站 凝析油稳定塔、泵及换热器(1200td)工艺及自控流程图.pid",
        pdf_stem: "集注站 凝析油稳定单元 凝析油稳定塔、泵及换热器(1200td)工艺及自控流程图",
        template_stem: "CPE-SQ-A1",
        paper_mm: [841.0, 594.0],
        offset_mm: [1.042_526, 1.042_526],
        factory_draws_border: false,
    },
    FactorySheet {
        relative_pid: r"00\00\集注站 凝析油稳定塔、泵及换热器(600td)工艺及自控流程图.pid",
        pdf_stem: "集注站 凝析油稳定单元 凝析油稳定塔、泵及换热器(600td)工艺及自控流程图",
        template_stem: "CPE-SQ-A1",
        paper_mm: [841.0, 594.0],
        offset_mm: [0.768_634, 0.768_634],
        factory_draws_border: true,
    },
];

/// Where `N井场`'s factory plot letters the first glyph of `15-TD-4101416-B2-EH40`,
/// paper millimetres: nine 150-dpi device units under the bottom edge.
const N_FACTORY_GLYPH_MM: [f64; 2] = [262.128, -1.524];

/// The frame-adjusted insertion our model keeps for that sideways tag.
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

#[test]
fn six_offset_templates_are_clipped_at_the_paper_edge_in_their_factory_plots() {
    let Some(root) = data_root() else {
        eprintln!("skipping: no backup directory with factory Amyuni PDFs is present");
        return;
    };
    let backup = root.join("backup-plant-data");
    let pdfs = pdf_dir(&root);

    let mut witnessed_rows: Vec<(&str, &str)> = Vec::new();

    for sheet in SIX_SHEETS {
        let FactorySheet {
            relative_pid: relative,
            pdf_stem: name,
            template_stem,
            paper_mm,
            offset_mm,
            factory_draws_border,
        } = sheet;
        let pid = backup.join(relative);
        let pdf = pdfs.join(format!("{name}.pdf"));
        let model: PaperModel = lay_out_drawing(&pid, &root)
            .unwrap_or_else(|error| panic!("{}: {error}", pid.display()));
        let template = model
            .template
            .as_ref()
            .unwrap_or_else(|| panic!("{name}: expected border template {template_stem}"));
        let actual_page = model.resolved_page_mm().expect("declared paper");
        for axis in 0..2 {
            assert!(
                (actual_page[axis] - paper_mm[axis]).abs() < 1e-6,
                "{name}: page {actual_page:?} != {paper_mm:?}"
            );
            assert!(
                (template.offset_mm[axis] - offset_mm[axis]).abs() <= PAPER_EDGE_OFFSET_SLACK_MM,
                "{name}: offset {:?} != {offset_mm:?}",
                template.offset_mm
            );
        }

        let witness = paper_edge_witness(template).unwrap_or_else(|| {
            panic!(
                "{name}: {template_stem} at {:?} has no row in PAPER_EDGE_WITNESSES",
                template.offset_mm
            )
        });
        assert_eq!(witness.source, PaperEdgeSource::FactoryBackup);
        witnessed_rows.push((witness.plot, witness.template));

        let pdf_bytes =
            std::fs::read(&pdf).unwrap_or_else(|error| panic!("{}: {error}", pdf.display()));
        let ink = read_plot(&pdf_bytes).expect("read factory plot");
        let pens = read_plot_pens(&pdf_bytes).expect("read factory pens");

        // Every factory plot's page box is the sheet's declared ISO paper, not
        // the border frame's rectangle.
        for axis in 0..2 {
            assert!(
                (ink.page_mm[axis] - paper_mm[axis]).abs() < 0.05,
                "{name}: factory PDF page {:?} != declared paper {paper_mm:?}",
                ink.page_mm
            );
        }

        // No ordinary ink segment extends past the paper box.
        let past_right = ink
            .segments
            .iter()
            .filter(|s| {
                s.start_mm[0] > ink.page_mm[0] + 0.05 || s.end_mm[0] > ink.page_mm[0] + 0.05
            })
            .count();
        let past_top = ink
            .segments
            .iter()
            .filter(|s| {
                s.start_mm[1] > ink.page_mm[1] + 0.05 || s.end_mm[1] > ink.page_mm[1] + 0.05
            })
            .count();
        assert_eq!(
            past_right, 0,
            "{name}: factory plot printed ink past right edge"
        );
        assert_eq!(
            past_top, 0,
            "{name}: factory plot printed ink past top edge"
        );

        // No sheet-edge border line is printed at the off-paper top or right
        // outline position. Bottom/left border lines clipped against the page
        // box can carry a one-pixel clip tail on the right edge (0.169 mm on
        // 150 dpi Amyuni), and O/P's 0.15 mm right edge grazes x = 841.0787
        // (half-pen 0.085 mm inside 840.999 mm); anything more than 1.5 pixels
        // past the right edge or 0.5 pixels past the top edge would be an
        // unclipped off-paper outline.
        let edges_past_top = pens
            .sheet_edges
            .iter()
            .filter(|e| {
                e.start_mm[1] > ink.page_mm[1] + 0.5 * PIXEL_MM
                    && e.end_mm[1] > ink.page_mm[1] + 0.5 * PIXEL_MM
            })
            .count();
        let edges_past_right = pens
            .sheet_edges
            .iter()
            .filter(|e| {
                e.start_mm[0] > ink.page_mm[0] + 1.5 * PIXEL_MM
                    && e.end_mm[0] > ink.page_mm[0] + 1.5 * PIXEL_MM
            })
            .count();
        assert_eq!(
            edges_past_top, 0,
            "{name}: factory plot printed a top outline edge past the paper"
        );
        assert_eq!(
            edges_past_right, 0,
            "{name}: factory plot printed a right outline edge past the paper"
        );
        assert_eq!(
            !pens.sheet_edges.is_empty(),
            factory_draws_border,
            "{name}: factory border presence changed ({} sheet_edges)",
            pens.sheet_edges.len()
        );
    }

    // Two-way check: every FactoryBackup row in PAPER_EDGE_WITNESSES is backed
    // by a real .pid + factory Amyuni PDF checked above.
    for witness in PAPER_EDGE_WITNESSES
        .iter()
        .filter(|witness| witness.source == PaperEdgeSource::FactoryBackup)
    {
        assert!(
            witnessed_rows.contains(&(witness.plot, witness.template)),
            "PAPER_EDGE_WITNESSES lists factory plot {} for {} at ({:.6}, {:.6}) mm, \
             not verified in SIX_SHEETS",
            witness.plot,
            witness.template,
            witness.offset_mm[0],
            witness.offset_mm[1]
        );
    }
}

#[test]
fn n_wellpad_sideways_label_starts_under_the_bottom_edge_in_its_factory_plot() {
    let Some(root) = data_root() else {
        eprintln!("skipping: no backup directory with factory Amyuni PDFs is present");
        return;
    };
    let pid = root
        .join("backup-plant-data")
        .join(r"00\00\N井场 注采阀组工艺及自控流程图.pid");
    let pdf = pdf_dir(&root).join("N井场 注采阀组工艺及自控流程图.pdf");
    let model =
        lay_out_drawing(&pid, &root).unwrap_or_else(|error| panic!("{}: {error}", pid.display()));

    let insertions: Vec<[f64; 2]> = model
        .primitives
        .iter()
        .filter_map(|primitive| match &primitive.shape {
            PaperShape::Text {
                origin_mm, value, ..
            } if value == "15-TD-4101416-B2-EH40" => Some(*origin_mm),
            _ => None,
        })
        .collect();
    assert_eq!(insertions.len(), 1, "the tag is on the page once");
    for (actual, expected) in insertions[0].into_iter().zip(N_INSERTION_MM) {
        assert!(
            (actual - expected).abs() < 0.0001,
            "insertion {:?} != {N_INSERTION_MM:?}",
            insertions[0]
        );
    }

    let pdf_bytes =
        std::fs::read(&pdf).unwrap_or_else(|error| panic!("{}: {error}", pdf.display()));
    let ink = read_plot(&pdf_bytes).expect("read N井场 factory plot");
    let spans: Vec<_> = ink
        .spans
        .iter()
        .filter(|span| span.text == "15-TD-4101416-B2-EH40")
        .collect();
    assert_eq!(spans.len(), 1, "factory plot letters the tag once");
    assert!(
        spans[0].origin_mm[1] < 0.0,
        "factory plot first glyph starts under the bottom edge, at {:?}",
        spans[0].origin_mm
    );
    for (actual, expected) in spans[0].origin_mm.into_iter().zip(N_FACTORY_GLYPH_MM) {
        assert!(
            (actual - expected).abs() < 0.05,
            "factory glyph origin {:?} != {N_FACTORY_GLYPH_MM:?}",
            spans[0].origin_mm
        );
    }
}
