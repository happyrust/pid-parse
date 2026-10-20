//! Every `igArc2d` (`0x0061`) payload in a symbol's body sheet, in hex, so a
//! field the decoder leaves unread can be compared between two arcs that must
//! bulge opposite ways.
//!
//! ```text
//! cargo run -p pid-parse --example probe_arc_payload_bytes -- "<symbol.sym>"
//! ```
//!
//! Read-only.

use std::path::PathBuf;

use pid_parse::PidParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .expect("usage: probe_arc_payload_bytes <symbol.sym>");
    let package = PidParser::new().parse_package(PathBuf::from(&path).as_path())?;
    for (stream_path, raw) in &package.streams {
        let data = &raw.data;
        let mut offset = 0usize;
        while offset + 6 <= data.len() {
            let type_word = u16::from_le_bytes([data[offset], data[offset + 1]]);
            if type_word & 0x3FFF != 0x0061 {
                offset += 1;
                continue;
            }
            let len = u32::from_le_bytes([
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
            ]) as usize;
            let start = offset + 6;
            let Some(payload) = data.get(start..start + len) else {
                offset += 1;
                continue;
            };
            if len < 59 {
                offset += 1;
                continue;
            }
            let f = |at: usize| {
                payload
                    .get(at..at + 8)
                    .map(|slice| f64::from_le_bytes(slice.try_into().expect("8 bytes")))
                    .unwrap_or(f64::NAN)
            };
            let hex: String = payload
                .iter()
                .skip(56)
                .map(|byte| format!("{byte:02X}"))
                .collect::<Vec<_>>()
                .join(" ");
            println!(
                "{stream_path} @0x{offset:06X} len {len} centre ({:.6},{:.6}) r {:.6} \
                 start {:.6} end {:.6} tail {hex}",
                f(18),
                f(26),
                f(34),
                f(42),
                f(50)
            );
            offset = start + len;
        }
    }
    Ok(())
}
