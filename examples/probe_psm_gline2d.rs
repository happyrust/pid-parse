//! One-shot Slice D byte-level verification:
//!
//! Given the PSM record byte layout reverse-engineered from
//! `radsrvitem.dll` (see `docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md`):
//!
//! ```
//! 0..2   uint16_le  type_code           // 14-bit (top 2 bits flags)
//! 2..6   uint32_le  bytes_to_follow     // payload length excl. these 6 bytes
//! 6..10  uint32_le  oid
//! 10..18  8 bytes   aux
//! 18..   var        inner_payload
//! ```
//!
//! and the `GLine2d` 6 × f64 = 48-byte field layout
//! (origin.xy, direction.xy unit, param_start, param_end), this
//! program scans the `/Sheet6` stream of `DWG-0201GP06-01.pid` for
//! candidate records whose inner payload matches the GLine2d
//! validation rules:
//!
//! 1. all 6 doubles finite (non-NaN, non-inf)
//! 2. `sqrt(d2^2 + d3^2)` ≈ 1.0 (direction is unit)
//! 3. `param_start < param_end`
//! 4. `bytes_to_follow >= 48` (room for 6 doubles)
//!
//! Hits get printed with offset + decoded values. This is **not**
//! a production decoder — it is an exploratory probe to validate
//! the reverse-engineered byte layout against real fixture bytes
//! before writing `decode_primitive_line()` in
//! `src/parsers/sheet_records.rs`.

use std::path::PathBuf;

use cfb::CompoundFile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixture: PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "test-file/DWG-0201GP06-01.pid".to_string())
        .into();
    let stream_name = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "/Sheet6".to_string());

    eprintln!("opening: {}", fixture.display());
    eprintln!("stream:  {stream_name}");

    let mut cfb = CompoundFile::open(std::fs::File::open(&fixture)?)?;
    let mut stream = cfb.open_stream(&stream_name)?;
    let mut bytes = Vec::new();
    use std::io::Read;
    stream.read_to_end(&mut bytes)?;
    eprintln!("stream bytes: {}", bytes.len());

    // Scan every offset; check if a PSM-shaped record sits here whose
    // inner payload matches GLine2d's validation rules.
    let mut hits = 0;
    let max_offset = bytes.len().saturating_sub(18 + 48);
    for off in 0..=max_offset {
        // Read PSM header at off.
        let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
        let type_code = type_word & 0x3FFF;
        let flags = type_word >> 14;
        let bytes_to_follow = u32::from_le_bytes([
            bytes[off + 2],
            bytes[off + 3],
            bytes[off + 4],
            bytes[off + 5],
        ]);
        // Cheap reject: bytes_to_follow must be plausible (>= 48 for line,
        // < remaining stream).
        if bytes_to_follow < 48 || (bytes_to_follow as usize) > bytes.len() - off {
            continue;
        }
        let inner_start = off + 18;
        if inner_start + 48 > bytes.len() {
            continue;
        }

        // Decode 6 doubles starting at inner_start.
        let mut d = [0f64; 6];
        for (i, slot) in d.iter_mut().enumerate() {
            let pos = inner_start + i * 8;
            *slot = f64::from_le_bytes([
                bytes[pos],
                bytes[pos + 1],
                bytes[pos + 2],
                bytes[pos + 3],
                bytes[pos + 4],
                bytes[pos + 5],
                bytes[pos + 6],
                bytes[pos + 7],
            ]);
        }

        // GLine2d validation: all finite, direction is unit, params sorted.
        if !d.iter().all(|x| x.is_finite()) {
            continue;
        }
        let dir_len = (d[2] * d[2] + d[3] * d[3]).sqrt();
        let unit_err = (dir_len - 1.0).abs();
        // Allow 1e-3 to accept lines with coordinate-transform precision
        // loss; the IDA-decoded validation function uses a stricter
        // tol (1e-6 ish via `sub_56472D30()`) but real SmartPlant
        // writes occasionally have rounded direction vectors.
        if unit_err > 1e-3 {
            continue;
        }
        if d[4] >= d[5] {
            continue;
        }
        // Also reject all-zero (would be uninitialized memory),
        // and require direction to be non-trivial (not 0,0).
        if d[2].abs() < 1e-12 && d[3].abs() < 1e-12 {
            continue;
        }
        // Reject origins / params way outside any plausible domain.
        // SmartPlant typically uses millimeters / page-fractions.
        if d.iter().any(|x| x.abs() > 1e9) {
            continue;
        }
        if (d[5] - d[4]).abs() < 1e-9 {
            continue;
        }

        hits += 1;
        let endpoint_a_x = d[0] + d[4] * d[2];
        let endpoint_a_y = d[1] + d[4] * d[3];
        let endpoint_b_x = d[0] + d[5] * d[2];
        let endpoint_b_y = d[1] + d[5] * d[3];
        println!(
            "HIT @ off=0x{off:06x} type=0x{type_code:04x} flags={flags:#x} \
             bytes_to_follow={bytes_to_follow:<8} \
             origin=({:>+12.4}, {:>+12.4}) \
             dir=({:>+8.5}, {:>+8.5}) \
             param=[{:>+10.4}, {:>+10.4}] \
             endpoints A=({:>+12.4},{:>+12.4}) B=({:>+12.4},{:>+12.4})",
            d[0],
            d[1],
            d[2],
            d[3],
            d[4],
            d[5],
            endpoint_a_x,
            endpoint_a_y,
            endpoint_b_x,
            endpoint_b_y
        );
        if hits >= 50 {
            println!("... (capping at 50 hits)");
            break;
        }
    }
    eprintln!("total hits matching PSM+GLine2d shape: {hits}");
    Ok(())
}
