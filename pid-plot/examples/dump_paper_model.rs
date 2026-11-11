//! Write a drawing's page model as JSON, for a tool outside this crate to
//! read.
//!
//! The scripts under `tools/display_order/` set a reference plot's content
//! stream against the records of its `.pid`, and what ties a piece of ink
//! to a record is the page model: every primitive carries the stream and
//! the byte range of the record it was laid out from. This writes that
//! model, one file a drawing, where those scripts look for it:
//! `<out dir>/<drawing's stem>.model.json`.
//!
//! ```text
//! cargo run -p pid-plot --example dump_paper_model -- <corpus> <drawing.pid | directory> <out dir>
//! ```
//!
//! `<corpus>` is the directory the drawings are laid out against, as their
//! gate lays them out. A directory is read for the `.pid` files directly in
//! it. The order the page is painted in is not part of the model's JSON; it
//! is written beside it as `<stem>.paint-order.json`, the indices of the
//! model's primitives, first painted first, when the page states one.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_plot::model::lay_out_drawing;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [corpus, drawings, out] = &args[..] else {
        eprintln!("usage: dump_paper_model <corpus> <drawing.pid | directory> <out dir>");
        return ExitCode::from(2);
    };
    let (corpus, drawings, out) = (Path::new(corpus), Path::new(drawings), Path::new(out));
    let mut pids: Vec<PathBuf> = if drawings.is_dir() {
        std::fs::read_dir(drawings)
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.extension()
                            .is_some_and(|extension| extension.eq_ignore_ascii_case("pid"))
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else {
        vec![drawings.to_path_buf()]
    };
    pids.sort();
    if let Err(error) = std::fs::create_dir_all(out) {
        eprintln!("{}: {error}", out.display());
        return ExitCode::from(1);
    }
    let mut failed = false;
    for pid in &pids {
        let stem = pid
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let written = lay_out_drawing(pid, corpus)
            .map_err(|error| error.to_string())
            .and_then(|model| {
                let json = serde_json::to_string(&model).map_err(|error| error.to_string())?;
                std::fs::write(out.join(format!("{stem}.model.json")), json)
                    .map_err(|error| error.to_string())?;
                if let Some(order) = model.paint_sequence() {
                    let json = serde_json::to_string(&order).map_err(|error| error.to_string())?;
                    std::fs::write(out.join(format!("{stem}.paint-order.json")), json)
                        .map_err(|error| error.to_string())?;
                }
                Ok(model.primitives.len())
            });
        match written {
            Ok(primitives) => println!("{stem}: {primitives} primitive(s)"),
            Err(error) => {
                println!("{stem}: {error}");
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
