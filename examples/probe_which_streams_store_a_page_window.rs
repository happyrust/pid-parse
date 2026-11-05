//! Does any stream of a `.pid` hold a coordinate pair inside a page window?
//!
//! Scans every stream at every byte offset for two consecutive little-endian
//! `f64` values that, read as metres, land inside the window given in
//! millimetres, and prints each hit with its stream and offset. It settles
//! whether a position the reference plot prints is stored anywhere in the
//! file at all: a window with no hits outside the style defaults means no
//! record carries that position for an export to recompute from.
//!
//! `cargo run --example probe_which_streams_store_a_page_window -- "<file.pid>" x0 y0 x1 y1`
use std::path::Path;

use pid_parse::PidParser;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [path, rest @ ..] = args.as_slice() else {
        eprintln!("usage: <file.pid> x0 y0 x1 y1 (mm)");
        std::process::exit(2);
    };
    let window: Vec<f64> = rest.iter().map(|v| v.parse().expect("mm")).collect();
    let [x0, y0, x1, y1] = window[..] else {
        eprintln!("usage: <file.pid> x0 y0 x1 y1 (mm)");
        std::process::exit(2);
    };
    let package = PidParser::new()
        .parse_package(Path::new(path))
        .expect("compound file");
    let read = |data: &[u8], at: usize| -> Option<f64> {
        let bytes: [u8; 8] = data.get(at..at + 8)?.try_into().ok()?;
        let value = f64::from_le_bytes(bytes) * 1000.0;
        value.is_finite().then_some(value)
    };
    let mut hits = 0usize;
    for (stream_path, raw) in &package.streams {
        for at in 0..raw.data.len().saturating_sub(15) {
            let (Some(x), Some(y)) = (read(&raw.data, at), read(&raw.data, at + 8)) else {
                continue;
            };
            if (x0..=x1).contains(&x) && (y0..=y1).contains(&y) {
                hits += 1;
                println!("  {stream_path} +{at} ({x:.3}, {y:.3}) mm");
            }
        }
    }
    println!("{hits} coordinate pairs inside ({x0}, {y0})-({x1}, {y1}) mm");
}
