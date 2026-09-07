//! The bytes of every `igRectangle2d` (`0x0020`) and `igBspCurve2d`
//! (`0x005D`) record the corpus holds, laid out against the layouts the
//! `imagdex.dex` `DoIO` workers read
//! (`docs/analysis/2026-08-31-imagdex-geometry-doio-ida.md` §5 / §5bis).
//!
//! Four records in the four sheet fixtures -- three rectangles and one
//! B-spline -- which is too few for byte statistics and exactly enough to
//! check a native-reader layout field by field before a decoder is written
//! against it. For each record: where it is, its sub-header, every `f64` the
//! layout predicts, and whatever bytes remain after the geometry.
//!
//! ```powershell
//! cargo run --quiet --example probe_rectangle_and_bspline_bytes
//! ```

use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::sheet_record_starts;

/// The four sheet fixtures, the A01 export, and the one `.sym` whose cached
/// body carries the corpus's B-spline -- to see whether the library copy has
/// the same record.
const FIXTURES: [&str; 6] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
    "test-file/symbols-full/Piping/Valves/2 Way Other/arrester breather valve(RD).sym",
];
const CHAIN_MAGIC: u32 = 0x6C90_F544;
const RECTANGLE: u16 = 0x0020;
const BSPLINE: u16 = 0x005D;

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn f64_at(data: &[u8], at: usize) -> Option<f64> {
    Some(f64::from_le_bytes(data.get(at..at + 8)?.try_into().ok()?))
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn sub_header(payload: &[u8]) -> String {
    format!(
        "oid={} parent={} layer={} sub_type=0x{:04X} index={}",
        u32_at(payload, 0).unwrap_or_default(),
        u32_at(payload, 4).unwrap_or_default(),
        u32_at(payload, 8).unwrap_or_default(),
        u16_at(payload, 12).unwrap_or_default(),
        u32_at(payload, 14).unwrap_or_default(),
    )
}

fn rectangle(payload: &[u8]) {
    println!("     {}", sub_header(payload));
    println!(
        "     payload {} bytes; +18 on: {}",
        payload.len(),
        hex(&payload[18.min(payload.len())..])
    );
    // The layout the DoIO worker reads: five doubles after the sub-header.
    let doubles: Vec<String> = (0..5)
        .filter_map(|i| f64_at(payload, 18 + i * 8).map(|v| format!("+{}={v:.6}", 18 + i * 8)))
        .collect();
    println!("     five f64 from +18: {}", doubles.join("  "));
    if payload.len() > 58 {
        println!(
            "     tail after +58 ({} bytes): {}",
            payload.len() - 58,
            hex(&payload[58..])
        );
    }
    // Any other double-looking value in the payload, in case the geometry
    // starts elsewhere.
    let others: Vec<String> = (18..payload.len().saturating_sub(7))
        .filter_map(|at| {
            let v = f64_at(payload, at)?;
            (v.is_finite() && v != 0.0 && v.abs() > 1e-6 && v.abs() < 10.0)
                .then(|| format!("+{at}={v:.5}"))
        })
        .collect();
    println!(
        "     every plausible f64 at any offset: {}",
        others.join(" ")
    );
}

fn bspline(payload: &[u8]) {
    println!("     {}", sub_header(payload));
    println!("     payload {} bytes", payload.len());
    let mut at = 18usize;
    let n = u32_at(payload, at).unwrap_or_default() as usize;
    println!("     +{at} u32 pole_count N = {n}");
    at += 4;
    for i in 0..n.min(64) {
        let x = f64_at(payload, at).unwrap_or(f64::NAN);
        let y = f64_at(payload, at + 8).unwrap_or(f64::NAN);
        println!("        pole[{i}] = ({x:.6}, {y:.6})   @+{at}");
        at += 16;
    }
    let weight_flag = u32_at(payload, at).unwrap_or_default();
    println!("     +{at} u32 weight_flag = {weight_flag}");
    at += 4;
    if weight_flag != 0 {
        for i in 0..n.min(64) {
            println!(
                "        weight[{i}] = {:.6}",
                f64_at(payload, at).unwrap_or(f64::NAN)
            );
            at += 8;
        }
    }
    let m = u32_at(payload, at).unwrap_or_default() as usize;
    println!("     +{at} u32 knot_count M = {m}");
    at += 4;
    let knots: Vec<String> = (0..m.min(128))
        .map(|i| format!("{:.4}", f64_at(payload, at + i * 8).unwrap_or(f64::NAN)))
        .collect();
    println!("        knots = [{}]", knots.join(", "));
    at += m.min(128) * 8;
    println!(
        "     +{at} f64 = {:.6}; then bytes: {}",
        f64_at(payload, at).unwrap_or(f64::NAN),
        hex(&payload[(at + 8).min(payload.len())..])
    );
    println!(
        "     implied degree = M - N - 1 = {}",
        m as i64 - n as i64 - 1
    );
}

fn main() {
    for fixture in FIXTURES {
        if !Path::new(fixture).exists() {
            println!("skip: {fixture} is absent");
            continue;
        }
        let Ok(file) = std::fs::File::open(fixture) else {
            continue;
        };
        let Ok(mut cfb) = CompoundFile::open(file) else {
            continue;
        };
        let paths: Vec<String> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|e| e.path().to_string_lossy().replace('\\', "/"))
            .collect();
        for path in paths {
            let Ok(mut stream) = cfb.open_stream(&path) else {
                continue;
            };
            let mut data = Vec::new();
            if stream.read_to_end(&mut data).is_err() || u32_at(&data, 0) != Some(CHAIN_MAGIC) {
                continue;
            }
            for at in sheet_record_starts(&data) {
                let Some(type_code) = u16_at(&data, at).map(|w| w & 0x3FFF) else {
                    continue;
                };
                if type_code != RECTANGLE && type_code != BSPLINE {
                    continue;
                }
                let len = u32_at(&data, at + 2).unwrap_or_default() as usize;
                let Some(payload) = data.get(at + 6..at + 6 + len) else {
                    continue;
                };
                println!(
                    "\n== {} {} 0x{type_code:04X} {} at stream offset {at}",
                    fixture.rsplit('/').next().unwrap_or(fixture),
                    path,
                    if type_code == RECTANGLE {
                        "igRectangle2d"
                    } else {
                        "igBspCurve2d"
                    }
                );
                if type_code == RECTANGLE {
                    rectangle(payload);
                } else {
                    bspline(payload);
                }
            }
        }
    }
}
