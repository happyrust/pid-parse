//! One-shot byte-level probe for `GArc2d` PSM records.
//!
//! `radsrvitem.dll!sub_56524150` (`GArc2d::Validate`) decompilation
//! says the in-memory `GArc2d` payload is 64 bytes = 8 × `f64` LE:
//!
//! ```
//! 0..7    center.x
//! 8..15   center.y
//! 16..23  axis1.x        (primary axis vector)
//! 24..31  axis1.y
//! 32..39  axis2.x        (secondary axis vector; circle = 0)
//! 40..47  axis2.y
//! 48..55  param_start    (sweep start)
//! 56..63  param_end      (sweep end)
//! ```
//!
//! This probe walks `/Sheet*` streams and reports every offset that
//! looks like an 18-byte PSM header followed by 64 bytes whose 8
//! doubles satisfy:
//!
//! 1. All 8 finite, `|x| <= 1e9` (per
//!    `GLINE2D_COORDINATE_DOMAIN_LIMIT`-style sanity).
//! 2. `axis1` (and ideally `axis2` if non-zero) has a plausible
//!    magnitude (not all-zero, but bounded). Pure circles have
//!    `axis2 == (0, 0)`; pure ellipse arcs have both axes non-zero.
//! 3. `param_start < param_end`.
//! 4. `bytes_to_follow >= 64`.
//!
//! Output prints candidate PSM `type_code` values; aggregating
//! across fixtures reveals the canonical GArc2d type code (the
//! GLine2d analogue was empirically `0x3FE6`).

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

    let mut hits = 0;
    let mut type_code_counter = std::collections::BTreeMap::<u16, usize>::new();
    let header_len = 18;
    let payload_len = 64;
    let max_offset = bytes.len().saturating_sub(header_len + payload_len);
    for off in 0..=max_offset {
        let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
        let type_code = type_word & 0x3FFF;
        let flags = type_word >> 14;
        let bytes_to_follow = u32::from_le_bytes([
            bytes[off + 2],
            bytes[off + 3],
            bytes[off + 4],
            bytes[off + 5],
        ]);
        if bytes_to_follow < 64 || (bytes_to_follow as usize) > bytes.len() - off {
            continue;
        }
        let inner_start = off + header_len;
        if inner_start + payload_len > bytes.len() {
            continue;
        }

        let mut d = [0f64; 8];
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

        if !d.iter().all(|x| x.is_finite()) {
            continue;
        }
        if d.iter().any(|x| x.abs() > 1e9) {
            continue;
        }
        let axis1_len = (d[2] * d[2] + d[3] * d[3]).sqrt();
        let axis2_len = (d[4] * d[4] + d[5] * d[5]).sqrt();
        // axis1 must be non-trivial; axis2 may be 0 (pure circle).
        if !(1e-6..=1e6).contains(&axis1_len) {
            continue;
        }
        if axis2_len > 1e6 {
            continue;
        }
        let param_start = d[6];
        let param_end = d[7];
        if param_start >= param_end {
            continue;
        }
        if (param_end - param_start).abs() < 1e-9 {
            continue;
        }
        // Reject obvious all-zeros (uninit memory) and obvious noise:
        if d[0] == 0.0 && d[1] == 0.0 && d[2] == 0.0 && d[3] == 0.0 {
            continue;
        }
        // Reject degenerate "axis1 == 1 0 axis2 == 0 0" (this is what
        // a GLine2d would look like decoded as 8 doubles instead of 6;
        // already covered by 0x3FE6).
        if (d[2] - 1.0).abs() < 1e-6 && d[3].abs() < 1e-6 && d[4].abs() < 1e-6 && d[5].abs() < 1e-6
        {
            continue;
        }

        hits += 1;
        *type_code_counter.entry(type_code).or_insert(0) += 1;
        if hits <= 30 {
            println!(
                "HIT @ off=0x{off:06x} type=0x{type_code:04x} flags={flags:#x} \
                 bytes_to_follow={bytes_to_follow:<6} \
                 center=({:>+10.4},{:>+10.4}) axis1=({:>+8.4},{:>+8.4})|{:.3} \
                 axis2=({:>+8.4},{:>+8.4})|{:.3} param=[{:>+8.4},{:>+8.4}]",
                d[0], d[1], d[2], d[3], axis1_len, d[4], d[5], axis2_len, param_start, param_end
            );
        }
    }

    eprintln!("total hits matching GArc2d shape: {hits}");
    eprintln!("type_code distribution:");
    for (tc, n) in &type_code_counter {
        eprintln!("  0x{tc:04x} ({tc}): {n}");
    }
    Ok(())
}
