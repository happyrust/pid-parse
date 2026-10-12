//! Write one `.pid` drawing to a PDF.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use pid_plot::model::lay_out_drawing;
use pid_plot::write::write_plot;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(pid) = args.next() else {
        eprintln!("usage: pid_plot <drawing.pid> <out.pdf> [corpus]");
        eprintln!("PID_PLOT_CJK_FONT must be a static TrueType/OpenType file under 16 MiB");
        return ExitCode::from(2);
    };
    let Some(out) = args.next() else {
        eprintln!("usage: pid_plot <drawing.pid> <out.pdf> [corpus]");
        return ExitCode::from(2);
    };
    let corpus = args.next().map_or_else(default_corpus, PathBuf::from);
    let font_path = match env::var("PID_PLOT_CJK_FONT") {
        Ok(path) => PathBuf::from(path),
        Err(_) => {
            eprintln!("PID_PLOT_CJK_FONT is unset");
            return ExitCode::from(2);
        }
    };
    let font = match fs::read(&font_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("read {}: {error}", font_path.display());
            return ExitCode::from(1);
        }
    };
    let model = match lay_out_drawing(PathBuf::from(&pid).as_path(), &corpus) {
        Ok(model) => model,
        Err(error) => {
            eprintln!("{pid}: {error}");
            return ExitCode::from(1);
        }
    };
    let pdf = match write_plot(&model, &font) {
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
