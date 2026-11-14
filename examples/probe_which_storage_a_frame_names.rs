//! Which storage does an `igSmartFrame2d` record name, and what does that
//! storage hold?
//!
//! A frame is an OLE container: what it contains is kept in a storage of the
//! document. The display order needs to know which one, because a sheet a
//! frame embeds — a stream `Sheet<M>` nested in a `JSite<N>` storage — is
//! drawn where the drawing lists that frame
//! (`parsers::sheet_display_order::smart_frame_sites`).
//!
//! For every drawing it is given this prints one line per live frame record
//! of every sheet stream:
//!
//! | column | what |
//! |---|---|
//! | `frame` | the stream, the record's offset and its oid |
//! | `state` | the native reader's OLE state for it |
//! | `site` | the `u32` at payload `+156` |
//! | `storage` | whether the file has a storage `JSite<site>`, and how many `Sheet*` streams with a live record it holds |
//! | `place` | where the sheet's display order lists the frame |
//!
//! and one line per sheet stream nested in a `JSite` storage that holds a
//! live record: how many records, how many the storage's own `JSheet` record
//! lists, how many take a rank from it, and the frame that embeds it.
//!
//! The last line counts, over all the drawings: frames on top-level sheets,
//! how many of them name a storage of their own file, nested sheets with
//! records, and how many of those have both an order of their own and a
//! frame that places them. Exit code 1 if a top-level frame names no
//! storage of its file. Whether a nested sheet that is not placed draws
//! anything is the page's to say:
//! `pid-plot/examples/probe_which_records_take_no_place_in_the_display_order`.
//!
//! ```text
//! cargo run --example probe_which_storage_a_frame_names -- <drawing.pid | directory> [...]
//! ```
//!
//! Over the 71 drawings on hand (53 of the plant backup, seven fixtures,
//! eleven acceptance sheets) it reads 68 frames on top-level sheets, all 68
//! naming a storage of their file; 66 are linked and their storage holds no
//! sheet; two are embedded — `A01.pid`'s frame 64, storage 204, and `Q井场`'s
//! frame 67755, storage 68432 — and each of those storages holds a sheet
//! with records (107 and 8), listed whole by its own `JSheet` record and
//! placed by its frame (places 0 and 2 937). `A01`'s storage holds a second
//! sheet with a record, `/JSite204/Sheet12`: one point and no list, which
//! draws nothing. The one frame record that names no storage is a locally
//! linked frame inside `A01`'s embedded sheet. Read-only.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_parse::parsers::sheet_display_order::smart_frame_sites;
use pid_parse::parsers::sheet_records::{parse_live_psm_header, SheetChain};
use pid_parse::PidParser;

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        entries.sort();
        for entry in entries {
            collect(&entry, out);
        }
    } else if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pid"))
    {
        out.push(path.to_path_buf());
    }
}

/// Live records of a chain stream, as the walk and the envelope reader give
/// them.
fn live_records(data: &[u8]) -> usize {
    SheetChain::walk(data)
        .starts
        .into_iter()
        .filter(|at| parse_live_psm_header(data, *at).is_some())
        .count()
}

/// The storage number of a path under `/JSite<N>/`.
fn site_of(path: &str) -> Option<u32> {
    path.strip_prefix("/JSite")?.split('/').next()?.parse().ok()
}

fn main() -> ExitCode {
    let mut drawings = Vec::new();
    for arg in std::env::args().skip(1) {
        collect(Path::new(&arg), &mut drawings);
    }
    if drawings.is_empty() {
        eprintln!("usage: probe_which_storage_a_frame_names <drawing.pid | directory> [...]");
        return ExitCode::from(2);
    }
    let (mut frames, mut named, mut embedded, mut placed) = (0usize, 0usize, 0usize, 0usize);
    for path in &drawings {
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let package = match PidParser::new().parse_package(path) {
            Ok(package) => package,
            Err(error) => {
                println!("{name} | does not parse: {error}");
                continue;
            }
        };
        let doc = &package.parsed;
        let storages: BTreeSet<u32> = package.streams.keys().filter_map(|p| site_of(p)).collect();
        // Sheets with a live record, by the storage they are nested in.
        let nested: Vec<(&String, u32, usize)> = package
            .streams
            .iter()
            .filter_map(|(stream, raw)| {
                let site = site_of(stream)?;
                let leaf = stream.rsplit('/').next()?;
                (leaf.starts_with("Sheet") && stream.matches('/').count() == 2)
                    .then(|| (stream, site, live_records(&raw.data)))
            })
            .filter(|(_, _, records)| *records > 0)
            .collect();
        for (stream, raw) in &package.streams {
            let leaf = stream.rsplit('/').next().unwrap_or_default();
            if !leaf.starts_with("Sheet") {
                continue;
            }
            let top_level = stream.matches('/').count() == 1;
            let states: std::collections::BTreeMap<usize, String> = doc
                .sheet_streams
                .iter()
                .find(|sheet| &sheet.path == stream)
                .and_then(|sheet| sheet.geometry.as_ref())
                .map(|geometry| {
                    geometry
                        .decoded_igsmartframes
                        .iter()
                        .map(|frame| {
                            (
                                frame.byte_start,
                                format!("oid {} {:?}", frame.oid, frame.state),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            for (at, site) in smart_frame_sites(&raw.data) {
                let has_storage = storages.contains(&site);
                let sheets = nested.iter().filter(|(_, of, _)| *of == site).count();
                if top_level {
                    frames += 1;
                    named += usize::from(has_storage);
                }
                let place = doc
                    .sheet_display_order
                    .get(stream)
                    .and_then(|order| order.ranks.get(&at))
                    .map_or_else(|| "none".to_string(), |rank| rank.place.to_string());
                println!(
                    "{name} | frame {stream} +{at} {} | site {site} | storage {} with {sheets} \
                     sheet(s) that hold a record | place {place}",
                    states.get(&at).map_or("not decoded", String::as_str),
                    if has_storage { "JSite" } else { "none" },
                );
            }
        }
        for (stream, _, records) in &nested {
            embedded += 1;
            match doc.embedded_sheet_display_order.get(*stream) {
                None => println!("{name} | sheet {stream} | {records} record(s) | no list read"),
                Some(sheet) => {
                    let frame = sheet.frame.as_ref().map_or_else(
                        || "no frame names its storage".to_string(),
                        |frame| {
                            format!(
                                "embedded by the frame at place {} of {}",
                                frame.rank.place, frame.stream_path
                            )
                        },
                    );
                    placed +=
                        usize::from(sheet.frame.is_some() && sheet.order.ranks.len() == *records);
                    println!(
                        "{name} | sheet {stream} | {records} record(s) | lists {}, ranks {} | \
                         {frame}",
                        sheet.order.children,
                        sheet.order.ranks.len()
                    );
                }
            }
        }
    }
    println!(
        "in all: {} drawing(s); {frames} frame(s) on top-level sheets, {named} naming a storage \
         of their file; {embedded} nested sheet(s) with records, {placed} listed whole and \
         placed by a frame",
        drawings.len()
    );
    if named == frames {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
