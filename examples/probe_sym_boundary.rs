//! Dump every type-19 (igBoundary2d) record in a `.sym`'s Sheet chains,
//! exactly the way `read_sheet_chain` walks them, plus a raw payload
//! hex head so the layout can be compared with the drawing-side decoder.

use pid_parse::PidParser;
use std::path::Path;

fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn f64le(b: &[u8], o: usize) -> f64 {
    f64::from_le_bytes([
        b[o],
        b[o + 1],
        b[o + 2],
        b[o + 3],
        b[o + 4],
        b[o + 5],
        b[o + 6],
        b[o + 7],
    ])
}

fn main() {
    for arg in std::env::args().skip(1) {
        let path = Path::new(&arg);
        let package = PidParser::new().parse_package(path).expect("package");
        println!("=== {}", path.file_name().unwrap().to_string_lossy());
        for (stream_path, raw) in &package.streams {
            let name = stream_path.rsplit('/').next().unwrap_or("");
            if !name.starts_with("Sheet") {
                continue;
            }
            let bytes = &raw.data;
            if bytes.len() < 8 || u32le(bytes, 0) != 0x6C90_F544 {
                println!("  {name}: no cluster magic (len {})", bytes.len());
                continue;
            }
            let mut at = 8usize;
            let mut found = 0usize;
            while at + 6 <= bytes.len() {
                let raw_type = u16le(bytes, at);
                let length = u32le(bytes, at + 2) as usize;
                if raw_type == 0 {
                    break;
                }
                let body = at + 6;
                let Some(end) = body.checked_add(length) else {
                    break;
                };
                if end > bytes.len() {
                    break;
                }
                let tc = raw_type & 0x0FFF;
                if tc == 0x13 {
                    found += 1;
                    let p = &bytes[body..end];
                    println!(
                        "  {name}@{at} rawtype=0x{raw_type:04X} len={length} payload[..40]={:02X?}",
                        &p[..p.len().min(40)]
                    );
                    if p.len() >= 28 {
                        let sub = u16le(p, 12);
                        let one = u32le(p, 18);
                        let seg = u32le(p, 22);
                        println!(
                            "     oid={} layer={} sub=0x{sub:04X} one={one} segs={seg} idx={}",
                            u32le(p, 0),
                            u32le(p, 8),
                            u32le(p, 14),
                        );
                        for s in 0..seg.min(4) as usize {
                            let g = 28 + s * 33;
                            if g + 33 <= p.len() {
                                println!(
                                    "     seg{s} tag=0x{:02X} [{:.4},{:.4} -> {:.4},{:.4}]",
                                    p[g],
                                    f64le(p, g + 1),
                                    f64le(p, g + 9),
                                    f64le(p, g + 17),
                                    f64le(p, g + 25),
                                );
                            }
                        }
                    }
                }
                at = end;
            }
            println!(
                "  {name}: {found} boundaries (chain end at={at}/{})",
                bytes.len()
            );
        }
    }
}
