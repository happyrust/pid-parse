//! Write the five corpus drawings and read them back.

use std::path::{Path, PathBuf};
use std::process::Command;

use pid_plot::fonts::prepare_local_fonts;
use pid_plot::model::{PaperModel, PaperShape, lay_out_drawing};
use pid_plot::read::read_plot;
use pid_plot::write::{DEFAULT_TEXT_MM, PlotFont, stroked_segments, write_plot};

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
fn five_drawings_round_trip_through_pdf() {
    let corpus = corpus_dir();
    let first = corpus.join(format!("{}.pid", DRAWINGS[0]));
    if !first.is_file() {
        eprintln!("skipping rendered plots; {} is absent", first.display());
        return;
    }
    if !PathBuf::from(r"C:\Windows\Fonts\arial.ttf").is_file() {
        eprintln!("skipping rendered plots; Windows fonts are absent");
        return;
    }

    let mut models = Vec::new();
    for name in DRAWINGS {
        let model = lay_out_drawing(&corpus.join(format!("{name}.pid")), &corpus)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        models.push((name, model));
    }
    let refs: Vec<&PaperModel> = models.iter().map(|(_, model)| model).collect();
    let prepared = prepare_local_fonts(&refs).unwrap_or_else(|error| panic!("fonts: {error}"));
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
        })
        .collect();
    let plot_dir = corpus.join("plot");
    std::fs::create_dir_all(&plot_dir).unwrap_or_else(|error| panic!("mkdir plot: {error}"));

    for (name, model) in &models {
        let pdf = write_plot(model, &fonts).unwrap_or_else(|error| panic!("{name}: {error}"));
        let out = plot_dir.join(format!("{name}.pdf"));
        std::fs::write(&out, &pdf)
            .unwrap_or_else(|error| panic!("write {}: {error}", out.display()));
        let ink = read_plot(&pdf).unwrap_or_else(|error| panic!("{name}: {error}"));
        if let Some(page) = model.page_mm {
            assert!((ink.page_mm[0] - page[0]).abs() < 0.05, "{name} width");
            assert!((ink.page_mm[1] - page[1]).abs() < 0.05, "{name} height");
        }
        let expected = stroked_segments(model);
        assert_eq!(
            ink.segments.len(),
            expected.len(),
            "{name} segment count read {} expected {}",
            ink.segments.len(),
            expected.len()
        );
        for (index, (segment, (start, end))) in ink.segments.iter().zip(expected).enumerate() {
            assert!(
                near(segment.start_mm, start) && near(segment.end_mm, end),
                "{name} segment {index}: {:?} != {:?} {:?}",
                segment,
                start,
                end
            );
        }
        let read_text: Vec<&str> = ink.spans.iter().map(|span| span.text.as_str()).collect();
        let expected_text = text_runs(model);
        if let Some(why) = runs_mismatch(&read_text, &expected_text) {
            eprintln!("READ {name}");
            for value in &read_text {
                eprintln!("  {value:?}");
            }
            eprintln!("MODEL {name}");
            for value in &expected_text {
                eprintln!("  {value:?}");
            }
            panic!("{name}: {why}");
        }
        pymupdf_opens(&out, &expected_text);
    }
}

/// Placed runs read back in order.
///
/// The reader may glue two runs that share a baseline, such as `1/2"` and
/// `NC`, and it may insert a one-character space where PyMuPDF would split a
/// gap. Reference plots still count those spaces; this check does not.
fn runs_mismatch(read: &[&str], expected: &[&str]) -> Option<String> {
    let mut read_at = 0;
    let mut expected_at = 0;
    while read_at < read.len() && expected_at < expected.len() {
        if read[read_at] == " " {
            read_at += 1;
            continue;
        }
        match glued_runs(read[read_at], &expected[expected_at..]) {
            Some(taken) => {
                read_at += 1;
                expected_at += taken;
            }
            None => {
                return Some(format!(
                    "read[{read_at}] {:?} does not cover model[{expected_at}] {:?}",
                    read[read_at], expected[expected_at]
                ));
            }
        }
    }
    while read_at < read.len() && read[read_at] == " " {
        read_at += 1;
    }
    if read_at == read.len() && expected_at == expected.len() {
        None
    } else {
        Some(format!(
            "unread {} model left {}",
            read.len() - read_at,
            expected.len() - expected_at
        ))
    }
}

fn glued_runs(read: &str, expected: &[&str]) -> Option<usize> {
    let mut acc = String::new();
    for (index, part) in expected.iter().enumerate() {
        acc.push_str(part);
        if acc == read {
            return Some(index + 1);
        }
        if !read.starts_with(&acc) {
            return None;
        }
    }
    None
}

fn text_runs(model: &PaperModel) -> Vec<&str> {
    let mut runs = Vec::new();
    for confidence in ["decoded", "inferred"] {
        for primitive in &model.primitives {
            if primitive.confidence != confidence {
                continue;
            }
            if let PaperShape::Text {
                value,
                height_mm,
                rotation,
                origin_mm,
                ..
            } = &primitive.shape
            {
                let height = height_mm.unwrap_or(DEFAULT_TEXT_MM);
                if value.is_empty()
                    || !rotation.is_finite()
                    || !origin_mm[0].is_finite()
                    || !origin_mm[1].is_finite()
                    || !height.is_finite()
                    || height <= 0.0
                {
                    continue;
                }
                runs.push(value.as_str());
            }
        }
    }
    runs
}

fn near(point: [f64; 2], expected: [f64; 2]) -> bool {
    (point[0] - expected[0]).abs() < 0.08 && (point[1] - expected[1]).abs() < 0.08
}

fn pymupdf_opens(pdf: &Path, texts: &[&str]) {
    let stamp = std::process::id();
    let script = std::env::temp_dir().join(format!("pid-plot-pymupdf-check-{stamp}.py"));
    std::fs::write(&script, PYMUPDF_PY).expect("write pymupdf script");
    let joined = texts.join("\n");
    let expect = std::env::temp_dir().join(format!("pid-plot-expect-text-{stamp}.txt"));
    std::fs::write(&expect, &joined).expect("write expected text");
    let status = Command::new("python")
        .arg(&script)
        .arg(pdf)
        .arg(&expect)
        .status()
        .unwrap_or_else(|error| panic!("python: {error}"));
    let _ = std::fs::remove_file(&script);
    let _ = std::fs::remove_file(&expect);
    assert!(
        status.success(),
        "pymupdf check failed for {}",
        pdf.display()
    );
}

const PYMUPDF_PY: &str = r#"
import sys
import pymupdf

pdf, expect_path = sys.argv[1:3]
doc = pymupdf.open(pdf)
assert doc.page_count == 1, doc.page_count
text = doc[0].get_text()
expect = open(expect_path, encoding="utf-8").read().splitlines()
for line in expect:
    if line and line not in text:
        raise SystemExit(f"missing {line!r}")
print("pymupdf chars", len(text))
"#;

#[test]
fn a_glued_inch_mark_still_covers_both_placed_runs() {
    let read = ["1/2\"NC", "PG"];
    let model = ["1/2\"", "NC", "PG"];
    assert_eq!(runs_mismatch(&read, &model), None);
}

#[test]
fn an_inserted_space_span_is_not_a_placed_run() {
    let read = ["2\"", " ", "NC"];
    let model = ["2\"", "NC"];
    assert_eq!(runs_mismatch(&read, &model), None);
}

#[test]
fn a_missing_placed_run_is_reported() {
    let read = ["PG"];
    let model = ["PG", "PT"];
    assert!(runs_mismatch(&read, &model).is_some());
}
