//! Which records that draw on a page take no place in the order its sheets
//! draw in?
//!
//! A page is painted in the order its sheets list their objects in
//! (`pid_parse::parsers::sheet_display_order`): the drawing's own records
//! where its `JSheet` record lists them or their owners, the border template
//! where the drawing lists its frame, a sheet a frame embeds where the
//! drawing lists that frame. A record no list places is painted after
//! everything that has a place, and the layout says so
//! (`LayoutLedger::unplaced`, a suspect item of the export ledger, a
//! `warning:` line of `pid_plot`). This prints, for every drawing it is
//! given, one line:
//!
//! | column | what |
//! |---|---|
//! | `primitives` | how many the page holds |
//! | `own` | the drawing's own, laid out from its top-level sheets |
//! | `embedded` | the drawing's own, laid out from a sheet nested in a `JSite` storage — what a frame embeds |
//! | `template` | the border template's |
//! | `order` | whether the page states an order to be painted in |
//! | `unplaced` | records no list places, and the primitives they draw |
//!
//! and under it one line for each stream and reason of the unplaced, in the
//! words `pid_plot` warns in. Exit code 1 if any page has one.
//!
//! ```text
//! cargo run -p pid-plot --example probe_which_records_take_no_place_in_the_display_order -- <corpus> <drawing.pid | directory> [...]
//! ```
//!
//! `<corpus>` is the directory the drawings are laid out against, as their
//! gate lays them out: the package for the plant backup, `test-file` for
//! the fixtures, the acceptance corpus for the reference sheets.
//!
//! Over the 71 pages on hand it reads no unplaced record. Two pages hold
//! primitives of an embedded sheet — `A01.pid` 93 and `Q井场` 8 — which were
//! painted after everything else, and silently, until the frame that embeds
//! each was read (2026-11-11). Read-only.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_plot::model::lay_out_drawing;

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        entries.sort();
        for entry in entries {
            // This machine's own symbol library, not a drawing's directory.
            if entry.file_name().is_some_and(|name| name == "symbols-full") {
                continue;
            }
            collect(&entry, out);
        }
    } else if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pid"))
    {
        out.push(path.to_path_buf());
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(corpus) = args.next().map(PathBuf::from) else {
        eprintln!(
            "usage: probe_which_records_take_no_place_in_the_display_order <corpus> \
             <drawing.pid | directory> [...]"
        );
        return ExitCode::from(2);
    };
    let mut drawings = Vec::new();
    for arg in args {
        collect(Path::new(&arg), &mut drawings);
    }
    let (mut pages, mut with_unplaced, mut embedded_pages, mut embedded_primitives) =
        (0usize, 0usize, 0usize, 0usize);
    let (mut records, mut primitives) = (0u64, 0u64);
    for pid in &drawings {
        let name = pid
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let model = match lay_out_drawing(pid, &corpus) {
            Ok(model) => model,
            Err(error) => {
                println!("{name} | does not lay out: {error}");
                continue;
            }
        };
        pages += 1;
        let template = model.primitives.iter().filter(|p| p.furniture).count();
        let embedded = model
            .primitives
            .iter()
            .filter(|p| {
                !p.furniture
                    && p.stream_path
                        .as_deref()
                        .is_some_and(|stream| stream.matches('/').count() > 1)
            })
            .count();
        let own = model.primitives.len() - template - embedded;
        if embedded > 0 {
            embedded_pages += 1;
            embedded_primitives += embedded;
        }
        let unplaced = &model.ledger.unplaced;
        let (page_records, page_primitives) = unplaced.iter().fold((0u64, 0u64), |sum, item| {
            (sum.0 + item.records, sum.1 + item.primitives)
        });
        if !unplaced.is_empty() {
            with_unplaced += 1;
            records += page_records;
            primitives += page_primitives;
        }
        println!(
            "{name} | {} primitive(s) | own {own} | embedded {embedded} | template {template} | \
             order {} | unplaced {page_records} record(s), {page_primitives} primitive(s)",
            model.primitives.len(),
            if model.paint_sequence().is_some() {
                "stated"
            } else {
                "none"
            },
        );
        for warning in model.ledger.display_order_warnings() {
            println!("      {warning}");
        }
    }
    println!(
        "in all: {pages} page(s); {embedded_pages} with primitives of an embedded sheet \
         ({embedded_primitives}); {with_unplaced} with a record no list places ({records} \
         record(s), {primitives} primitive(s))"
    );
    if with_unplaced == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
