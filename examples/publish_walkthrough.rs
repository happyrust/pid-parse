//! End-to-end "MDF → `_Data.xml` / `_Meta.xml`" walkthrough.
//!
//! Mirrors the internal `pid_publish_xml` binary but trimmed to the
//! shape a library consumer would write: open a SmartPlant backup
//! MDF with the vendored `oxidized-mdf` reader, hand the path plus a
//! drawing UID to [`pid_parse::publish::load_drawing_graph_from_mdf`],
//! and turn the resulting [`pid_parse::publish::PublishDrawing`] into
//! two XML strings via the writer entry points.
//!
//! Usage:
//!   cargo run --example publish_walkthrough \
//!     -- path/to/Export.mdf DRAWING_UID PLANT_NAME
//!
//! If no arguments are given, the example falls back to the A01
//! fixture under `test-file/`. Missing fixture prints a soft-skip
//! notice and exits cleanly.

use std::error::Error;
use std::path::{Path, PathBuf};

use pid_parse::publish::{load_drawing_graph_from_mdf, write_data_xml, write_meta_xml};

const FALLBACK_MDF: &str = "test-file/backup-test/TEST02_p/extracted/Export.mdf";
const FALLBACK_DRAWING_UID: &str = "D9635C3C898840D1990B7E8BEE1D55DA";
const FALLBACK_PLANT: &str = "TEST02";

fn main() -> Result<(), Box<dyn Error>> {
    let Some((mdf_path, drawing_uid, plant_name)) = resolve_input() else {
        eprintln!(
            "publish_walkthrough: no input given and `{FALLBACK_MDF}` is missing — skipping."
        );
        return Ok(());
    };

    println!("== publish_walkthrough: {} ==", mdf_path.display());
    println!("  drawing_uid : {drawing_uid}");
    println!("  plant_name  : {plant_name}");

    let drawing = load_drawing_graph_from_mdf(&mdf_path, &drawing_uid)?;

    // Surface a few summary stats before emitting XML so the reader
    // sees what the loader produced. The fields exercised here are
    // the ones the integration tests treat as the "drawing graph
    // fidelity" contract — every PublishDrawing is a DocUID plus
    // three parallel inventories (objects / representations /
    // relationships) and a plant-level codelist.
    println!("\n-- PublishDrawing --");
    println!("  style           : {:?}", drawing.style);
    println!("  drawing_uid     : {}", drawing.drawing_uid);
    println!("  drawing_name    : {}", drawing.drawing_name);
    if let Some(cat) = drawing.document_category.as_deref() {
        println!("  category        : {cat}");
    }
    println!("  objects         : {}", drawing.objects.len());
    println!("  representations : {}", drawing.representations.len());
    println!("  relationships   : {}", drawing.relationships.len());

    let data_xml = write_data_xml(&drawing, &plant_name)?;
    let meta_xml = write_meta_xml(&drawing, &plant_name)?;

    println!("\n_Data.xml : {} bytes", data_xml.len());
    println!("_Meta.xml : {} bytes", meta_xml.len());

    // Small preview so the reader can sanity-check without writing
    // to disk. A real consumer would persist these via `fs::write`.
    let preview_chars = 240;
    println!(
        "\n-- _Data.xml preview --\n{}",
        preview(&data_xml, preview_chars)
    );
    println!(
        "\n-- _Meta.xml preview --\n{}",
        preview(&meta_xml, preview_chars)
    );

    Ok(())
}

fn resolve_input() -> Option<(PathBuf, String, String)> {
    let mut args = std::env::args().skip(1);
    match (args.next(), args.next(), args.next()) {
        (Some(mdf), Some(uid), Some(plant)) => Some((PathBuf::from(mdf), uid, plant)),
        _ => {
            let fallback = Path::new(FALLBACK_MDF);
            if fallback.exists() {
                Some((
                    PathBuf::from(FALLBACK_MDF),
                    FALLBACK_DRAWING_UID.to_string(),
                    FALLBACK_PLANT.to_string(),
                ))
            } else {
                None
            }
        }
    }
}

fn preview(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        format!("{truncated}…")
    }
}
