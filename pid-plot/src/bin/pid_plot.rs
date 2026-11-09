//! Write one `.pid` drawing to a PDF.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::ledger::ExportLedger;
use pid_plot::model::lay_out_drawing;
use pid_plot::write::{PlotFont, write_plot_audited};

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
                subset: false,
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
            subset: face.subset,
        })
        .collect();
    let written = match write_plot_audited(&model, &fonts) {
        Ok(written) => written,
        Err(error) => {
            eprintln!("{pid}: {error}");
            return ExitCode::from(1);
        }
    };
    // The page's account, joined from the layout half on the model and the
    // write half the writer just produced. Assembled before `pdf` is moved so
    // the summary describes the same write the file does.
    let ledger = ExportLedger::assemble(&model, &written.ledger);
    let pdf = written.pdf;
    let audit = written.audit;
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
    // The page actually written. A drawing that states none is measured, and
    // this is the number the PDF carries — reporting nothing here would say
    // `0.00 0.00` about a page that was plotted on real paper.
    let page = model.resolved_page_mm().unwrap_or([0.0, 0.0]);
    println!(
        "primitives {} segments_written_page_mm {:.2} {:.2} bytes {}",
        model.primitives.len(),
        page[0],
        page[1],
        pdf.len()
    );
    // Said out loud even when zero: a run the sheet cannot hold at half width
    // loses its tail to the page clip, and nothing on the page records that it
    // happened.
    println!(
        "text_clipped_at_condensation_floor {} deepest_clip_mm {:.3}",
        audit.runs_clipped_at_the_condensation_floor, audit.deepest_clip_mm
    );
    // A run the sheet cannot hold at its set width is condensed instead of
    // clipped wherever it can fit that way, which draws the label narrower than
    // the drawing asked — printed for the same reason, zero included.
    println!(
        "text_condensed {} smallest_condense {:.3}",
        audit.runs_condensed, audit.smallest_condense
    );
    // The ledger's own summary (requirement 14.3): what the page's entities
    // did, how much of the export was deliberately not drawn, and how much
    // nothing accounts for. `unattributed` is printed even at zero — that is
    // the number a reader has to see to know the page balanced.
    println!("{}", ledger.summary());
    ExitCode::SUCCESS
}

fn default_corpus() -> PathBuf {
    env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}
