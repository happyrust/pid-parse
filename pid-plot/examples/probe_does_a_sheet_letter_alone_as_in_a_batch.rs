//! Whether a sheet letters every label in the same faces alone and in a batch.
//!
//! `pid_plot` prepares the faces for the one drawing it exports; a gate
//! prepares them once for all of its sheets. A character the face it is named
//! in cannot draw is borrowed from another face, and which face lends it must
//! not follow the company a sheet keeps: `pid_plot::fonts::borrow_order` reads
//! the lender off the label, and `prepare_fonts` judges a lender by its whole
//! program rather than by the subset one call happened to build.
//!
//! This probe asks the written pages. Every drawing it is given is written
//! twice — with the faces prepared for that drawing alone, and with the faces
//! prepared for all the drawings together — and each page's lettering is read
//! back out of its content stream: the face of every `Tf` by the `BaseFont` of
//! the font it selects, the characters of every `Tj` by that font's
//! `ToUnicode`. Nothing here repeats the writer's rule, so the answer is the
//! page's own. It prints one line per sheet, every label the two pages letter
//! differently, and exits non-zero if there is one.
//!
//! ```text
//! cargo run -q -p pid-plot --example probe_does_a_sheet_letter_alone_as_in_a_batch -- \
//!     <corpus> <drawing.pid | directory>...
//! ```
//!
//! `<corpus>` is what the drawings are laid out against, the way `pid_plot`'s
//! third argument is: its border templates, its symbols and `<corpus>/fonts`.
//! A directory is walked for `*.pid`. Faces come from `PID_PLOT_FONTS` first,
//! as they do everywhere else.
//!
//! # The sets on this machine
//!
//! ```text
//! corpus                                 drawings                     sheets  labels
//! D:\work\cad\PID自研软件验证             the corpus itself                 5    1390
//! D:\work\cad\pid-test-data\验收数据      the corpus itself                11    4405
//! D:\work\plant-code\cad\pid-test-data   <corpus>\backup-plant-data       53   16918
//! D:\work\cad\PID自研软件验证             <repository>\test-file            7     441
//! <repository>\test-file                 <repository>\test-file            7     628
//! ```
//!
//! The acceptance corpus and `test-file` keep no `fonts`, so those runs name
//! the share's in `PID_PLOT_FONTS`. The last row lays the seven fixtures out
//! against the repository's own `Ref`, which letters their border templates
//! as well.
//!
//! Read on 2026-11-10: every sheet of all five runs letters each label in the
//! same faces alone and together. A sheet that does not is reported label by
//! label. This is what the A2 acceptance sheet reads when the lender is taken
//! to be the first program in hand that holds the characters:
//!
//! ```text
//! 集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图: 258 labels, 2 lettered in other faces alone than together
//!     alone    FangSong:"动火点"
//!     together SimSun:"动火点"
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use pdfcraft_cos::{Document, Object};
use pdfcraft_fonts::pdf::Metrics;
use pid_plot::fonts::{PreparedFont, font_dirs, prepare_fonts};
use pid_plot::model::{PaperModel, lay_out_drawing};
use pid_plot::write::{PlotFont, write_plot};

/// What a command line has to look like.
const USAGE: &str =
    "usage: probe_does_a_sheet_letter_alone_as_in_a_batch <corpus> <drawing.pid | directory>...";

/// One label as a page letters it: each run's face and the characters that
/// face draws, in the order the page shows them.
type Label = Vec<(String, String)>;

fn collect_pids(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_pids(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "pid") {
            out.push(path);
        }
    }
}

fn plot_fonts(prepared: &[PreparedFont]) -> Vec<PlotFont<'_>> {
    prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
            subset: face.subset,
        })
        .collect()
}

/// The labels a written page letters, in content order.
///
/// A label is one `BT … ET` block. Inside it every `Tf` selects a font of the
/// page's resource dictionary, whose `BaseFont` is the face's own name behind
/// the subset tag, and every `Tj` or `TJ` shows bytes that font's `ToUnicode`
/// turns back into characters. Neighbouring runs in one face are joined: the
/// writer also cuts a run where only its spacing changes.
fn lettered(pdf: Vec<u8>) -> Result<Vec<Label>, String> {
    let doc = Document::open(Arc::new(pdf)).map_err(|error| format!("reopen: {error:?}"))?;
    let root = doc.root().ok_or("the page has no catalog")?;
    let catalog = doc
        .dict(&Object::Ref(root))
        .ok_or("the catalog is not a dictionary")?;
    let pages = catalog
        .get(b"Pages")
        .and_then(|pages| doc.dict(pages))
        .ok_or("the catalog has no page tree")?;
    let page = pages
        .get(b"Kids")
        .and_then(Object::as_array)
        .and_then(|kids| kids.first())
        .and_then(|kid| doc.dict(kid))
        .ok_or("the page tree has no page")?;
    let fonts = page
        .get(b"Resources")
        .and_then(|resources| doc.dict(resources))
        .and_then(|resources| resources.get(b"Font").and_then(|fonts| doc.dict(fonts)))
        .ok_or("the page has no font dictionary")?;
    let mut faces: Vec<(Vec<u8>, String, Metrics)> = Vec::new();
    for (resource, font) in fonts.iter() {
        let font = doc
            .dict(font)
            .ok_or("a font resource is not a dictionary")?;
        let base = font
            .name(b"BaseFont")
            .map(|name| String::from_utf8_lossy(name).into_owned())
            .ok_or("a font has no BaseFont")?;
        let face = base.rsplit('+').next().unwrap_or(&base).to_string();
        faces.push((resource.clone(), face, Metrics::from_dict(&doc, &font)));
    }
    let contents = page
        .get(b"Contents")
        .map(|contents| doc.resolve(contents))
        .ok_or("the page has no contents")?;
    let Object::Stream(stream) = contents.as_ref() else {
        return Err("the page's contents are not one stream".into());
    };
    let data = stream
        .decoded()
        .map_err(|error| format!("contents: {error:?}"))?;

    let mut labels = Vec::new();
    let mut label: Label = Vec::new();
    let mut face: Option<&(Vec<u8>, String, Metrics)> = None;
    for op in &pdfcraft_content::parse(&data).ops {
        if op.is("BT") {
            label.clear();
        } else if op.is("Tf") {
            face = op
                .name(0)
                .and_then(|name| faces.iter().find(|(resource, ..)| resource == name));
        } else if op.is("Tj") || op.is("TJ") {
            let (_, name, metrics) = face.ok_or("text is shown before a face is selected")?;
            let mut text = String::new();
            for operand in &op.operands {
                let shown: &[Object] = match operand {
                    Object::Array(items) => items,
                    other => std::slice::from_ref(other),
                };
                for item in shown {
                    if let Object::String(string) = item {
                        text.push_str(&metrics.decode(&string.bytes));
                    }
                }
            }
            match label.last_mut() {
                Some((last, run)) if last == name => run.push_str(&text),
                _ => label.push((name.clone(), text)),
            }
        } else if op.is("ET") {
            labels.push(std::mem::take(&mut label));
        }
    }
    Ok(labels)
}

fn show(label: &Label) -> String {
    label
        .iter()
        .map(|(face, text)| format!("{face}:{text:?}"))
        .collect::<Vec<_>>()
        .join(" + ")
}

/// Write `model` with `faces` and read its lettering back.
fn page_lettering(model: &PaperModel, faces: &[PreparedFont]) -> Result<Vec<Label>, String> {
    let pdf = write_plot(model, &plot_fonts(faces)).map_err(|error| error.to_string())?;
    lettered(pdf)
}

fn run(corpus: &Path, drawings: &[PathBuf]) -> Result<usize, String> {
    let dirs = font_dirs(Some(corpus));
    let mut models: Vec<PaperModel> = Vec::new();
    for path in drawings {
        let model = lay_out_drawing(path, corpus)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        models.push(model);
    }
    let all: Vec<&PaperModel> = models.iter().collect();
    let together = prepare_fonts(&all, &dirs)?;
    let mut labels = 0usize;
    let mut differing_sheets = 0usize;
    for (path, model) in drawings.iter().zip(&models) {
        let name = path.file_stem().map_or_else(
            || path.display().to_string(),
            |stem| stem.to_string_lossy().into_owned(),
        );
        let alone = prepare_fonts(&[model], &dirs).map_err(|error| format!("{name}: {error}"))?;
        let lettered_alone =
            page_lettering(model, &alone).map_err(|error| format!("{name}: alone: {error}"))?;
        let lettered_together = page_lettering(model, &together)
            .map_err(|error| format!("{name}: together: {error}"))?;
        if lettered_alone.len() != lettered_together.len() {
            return Err(format!(
                "{name}: {} labels alone, {} together",
                lettered_alone.len(),
                lettered_together.len()
            ));
        }
        labels += lettered_alone.len();
        let differing: Vec<(&Label, &Label)> = lettered_alone
            .iter()
            .zip(&lettered_together)
            .filter(|(alone, together)| alone != together)
            .collect();
        if differing.is_empty() {
            println!(
                "{name}: {} labels, the same faces alone and together",
                lettered_alone.len()
            );
        } else {
            differing_sheets += 1;
            println!(
                "{name}: {} labels, {} lettered in other faces alone than together",
                lettered_alone.len(),
                differing.len()
            );
            for (alone, together) in differing {
                println!("    alone    {}", show(alone));
                println!("    together {}", show(together));
            }
        }
    }
    println!(
        "{} sheets, {labels} labels: {differing_sheets} sheet(s) letter a label in other faces \
         alone than together",
        drawings.len()
    );
    Ok(differing_sheets)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(corpus) = args.next().map(PathBuf::from) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let mut drawings = Vec::new();
    for arg in args {
        let path = PathBuf::from(arg);
        if path.is_dir() {
            collect_pids(&path, &mut drawings);
        } else {
            drawings.push(path);
        }
    }
    if drawings.is_empty() {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    match run(&corpus, &drawings) {
        Ok(0) => ExitCode::SUCCESS,
        Ok(_) => ExitCode::from(1),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
