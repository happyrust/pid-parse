//! Write one `.pid` drawing to a PDF.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::lay_out_drawing;
use pid_plot::write::{PlotFont, write_plot};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(pid) = args.next() else {
        eprintln!("usage: pid_plot <drawing.pid> <out.pdf> [corpus]");
        eprintln!("typefaces come from PID_PLOT_FONTS, then <corpus>/fonts");
        return ExitCode::from(2);
    };
    let Some(out) = args.next() else {
        eprintln!("usage: pid_plot <drawing.pid> <out.pdf> [corpus]");
        return ExitCode::from(2);
    };
    let corpus = args.next().map_or_else(default_corpus, PathBuf::from);
    let model = match lay_out_drawing(PathBuf::from(&pid).as_path(), &corpus) {
        Ok(model) => model,
        Err(error) => {
            eprintln!("{pid}: {error}");
            return ExitCode::from(1);
        }
    };
    let mut prepared = match prepare_fonts(&[&model], &font_dirs(Some(&corpus))) {
        Ok(faces) => faces,
        Err(error) => {
            eprintln!("{pid}: {error}");
            return ExitCode::from(1);
        }
    };
    if let Ok(path) = env::var("PID_PLOT_CJK_FONT") {
        match fs::read(&path) {
            Ok(bytes) => prepared.push(pid_plot::fonts::PreparedFont {
                key: String::new(),
                bytes,
            }),
            Err(error) => {
                eprintln!("read {path}: {error}");
                return ExitCode::from(1);
            }
        }
    }
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
        })
        .collect();
    let pdf = match write_plot(&model, &fonts) {
        Ok(pdf) => pdf,
        Err(error) => {
            eprintln!("{pid}: {error}");
            return ExitCode::from(1);
        }
    };
    if let Some(parent) = PathBuf::from(&out).parent()
        && !parent.as_os_str().is_empty()
        && let Err(error) = fs::create_dir_all(parent)
    {
        eprintln!("mkdir {}: {error}", parent.display());
        return ExitCode::from(1);
    }
    if let Err(error) = fs::write(&out, &pdf) {
        eprintln!("write {out}: {error}");
        return ExitCode::from(1);
    }
    println!(
        "primitives {} segments_written_page_mm {:.2} {:.2} bytes {}",
        model.primitives.len(),
        model.page_mm.map(|page| page[0]).unwrap_or(0.0),
        model.page_mm.map(|page| page[1]).unwrap_or(0.0),
        pdf.len()
    );
    ExitCode::SUCCESS
}

fn default_corpus() -> PathBuf {
    env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}
