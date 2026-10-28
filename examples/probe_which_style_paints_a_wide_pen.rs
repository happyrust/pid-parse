//! Which style a drawing's widest pens belong to, by name.
//!
//! A page's pen is its entity's `JStyleSimpleLine` width, and the style table
//! also holds entries that are not pens at all: `SmartPlant` writes its
//! validation marks there (`BandAid`, `lsOk`, `lsWarning`) with the mark's own
//! on-screen size — `BandAid` states 10 mm, fifteen times the widest drafting
//! width in this corpus. Asking a page which style paints an outlier width is
//! what tells a pen from a mark.
//!
//! ```text
//! cargo run --example probe_which_style_paints_a_wide_pen -- <a.pid> [more ...]
//! ```
//!
//! Per drawing: the entity count, then one row per style wide to thin —
//! `width  style <id>  <name>  rgb  entities` — so a style the drawing never
//! paints is absent and one that paints half the page is unmistakable.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pid_parse::api::PidParser;
use pid_parse::build_normalized_geometry;
use pid_parse::style_link::{line_styles_for_document, style_names_for_document};

fn main() {
    let paths: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if paths.is_empty() {
        eprintln!("usage: probe_which_style_paints_a_wide_pen <a.pid> [more ...]");
        std::process::exit(2);
    }
    for path in &paths {
        let name = path.file_stem().map_or_else(
            || path.display().to_string(),
            |stem| stem.to_string_lossy().into_owned(),
        );
        let doc = match PidParser::new().parse_file(path) {
            Ok(doc) => doc,
            Err(error) => {
                println!("FAIL {name}: {error}");
                continue;
            }
        };
        let styles = line_styles_for_document(&doc);
        let names = style_names_for_document(&doc);
        let geometry = build_normalized_geometry(&doc);

        // How many entities each resolved style actually paints, so a style
        // nothing uses is told apart from one that draws half a page.
        let mut users: BTreeMap<(String, u32), usize> = BTreeMap::new();
        for entity in &geometry.entities {
            let (Some(stream), Some(oid)) = (entity.source.stream_path.clone(), entity.graphic_oid)
            else {
                continue;
            };
            *users.entry((stream, oid)).or_default() += 1;
        }

        // One row per (width, name, colour): the table's own shape, not one row
        // per record that names a style.
        let mut table: BTreeMap<(i64, String, [u8; 3]), (u32, usize)> = BTreeMap::new();
        for ((stream, oid), style) in &styles {
            let style_name = names
                .get(&(stream.clone(), style.style_id))
                .cloned()
                .unwrap_or_else(|| "-".into());
            let painted = users.get(&(stream.clone(), *oid)).copied().unwrap_or(0);
            let key = (
                (style.symbology.width_mm() * 1000.0).round() as i64,
                style_name,
                style.symbology.rgb(),
            );
            let entry = table.entry(key).or_insert((style.style_id, 0));
            entry.1 += painted;
        }
        println!(
            "{name}: {} entities, {} resolved line styles",
            geometry.entities.len(),
            styles.len()
        );
        for ((thousandths, style_name, rgb), (style_id, painted)) in table.iter().rev() {
            println!(
                "  {:8.3} mm  style {style_id:4}  {style_name:<28} rgb {rgb:?}  entities {painted}",
                *thousandths as f64 / 1000.0
            );
        }
    }
}
