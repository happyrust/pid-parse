//! Does a `.sym` (or a `.pid`'s cached bodies) carry `0x0063 igEllipse2d`
//! records, and what do they read?
//!
//! Walks every stream of each argument, decodes every `igEllipse2d` record
//! with `decode_igellipses`, and prints its oid, layer, centre, major axis,
//! ratio and perimeter in millimetres (storage units are metres). For a
//! `.sym` it also prints how many B-splines the library reader draws, so the
//! record count and the body agree at a glance.
//!
//! `cargo run --example probe_sym_ellipse_records -- "<file.sym|file.pid>"...`
use std::path::Path;

use pid_parse::parsers::sheet_records::decode_igellipses;
use pid_parse::symbol_library::{read_symbol_geometry, SymbolPrimitive};
use pid_parse::PidParser;

/// Ramanujan's second approximation, good to far below a micrometre here.
fn perimeter(a: f64, b: f64) -> f64 {
    let h = ((a - b) / (a + b)).powi(2);
    std::f64::consts::PI * (a + b) * (1.0 + 3.0 * h / (10.0 + (4.0 - 3.0 * h).sqrt()))
}

fn main() {
    for arg in std::env::args().skip(1) {
        let path = Path::new(&arg);
        let package = PidParser::new().parse_package(path).expect("compound file");
        println!("== {arg}");
        let mut total = 0usize;
        for (stream_path, raw) in &package.streams {
            for e in decode_igellipses(&raw.data) {
                total += 1;
                let a = e.major_axis.0.hypot(e.major_axis.1) * 1000.0;
                let b = a * e.ratio;
                println!(
                    "  {stream_path} +{} oid {} layer {} centre ({:.3}, {:.3}) mm major ({:.4}, {:.4}) mm ratio {:.5} a {a:.3} b {b:.3} perimeter {:.3} mm flag {}",
                    e.byte_range.start,
                    e.oid,
                    e.sheet_layer_ref,
                    e.center.0 * 1000.0,
                    e.center.1 * 1000.0,
                    e.major_axis.0 * 1000.0,
                    e.major_axis.1 * 1000.0,
                    e.ratio,
                    perimeter(a, b),
                    e.flag,
                );
            }
        }
        println!("  {total} igEllipse2d records");
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("sym"))
        {
            match read_symbol_geometry(path) {
                Ok(body) => {
                    let splines = body
                        .primitives
                        .iter()
                        .filter(|p| matches!(p.primitive, SymbolPrimitive::BSpline { .. }))
                        .count();
                    println!(
                        "  library body: {} primitives, {splines} B-splines",
                        body.primitives.len()
                    );
                }
                Err(error) => println!("  library body: {error}"),
            }
        }
    }
}
