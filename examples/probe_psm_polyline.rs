//! Probe for PSM-encoded `GLineString2d` (polyline) records.
//!
//! `radsrvitem.dll!sub_56524DD0` (`GLineString2d::Validate`)
//! reveals the in-memory layout:
//!
//! ```
//! a2 + 0:   _DWORD   vertex_array_ptr (pointer to f64 pairs)
//! a2 + 4:   _DWORD   vertex_count (must be >= 2)
//! a2 + 8:   _BYTE    form (must be <= 6)
//! a2 + 9:   _BYTE    scope (must be <= 4 or == 6)
//! ```
//!
//! On disk the pointer is replaced by inline vertex bytes:
//!
//! ```
//! 0..4   uint32_le  vertex_count (>= 2)
//! 4..5   uint8      form (0..6)
//! 5..6   uint8      scope (0..4 or 6)
//! 6..8   padding (likely 2 bytes for alignment)
//! 8..    vertex_count * 16 bytes of f64 LE pairs (x, y)
//! ```
//!
//! Probe every offset; aggregate type codes whose record passes
//! the validation rules. Report distribution to identify the
//! GLineString2d PSM type code (the GLine2d and GArc2d analogues
//! turned out to be `0x3FE6` and `0x0030` respectively).

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

    let mut cfb = CompoundFile::open(std::fs::File::open(&fixture)?)?;
    let mut stream = cfb.open_stream(&stream_name)?;
    let mut bytes = Vec::new();
    use std::io::Read;
    stream.read_to_end(&mut bytes)?;
    eprintln!("fixture: {} {stream_name} bytes: {}", fixture.display(), bytes.len());

    let header_len = 18;
    let min_payload = 8 + 2 * 16; // 8 bytes prefix + 2 vertices
    let mut hits = 0;
    let mut type_code_counter = std::collections::BTreeMap::<u16, usize>::new();
    let mut hits_by_count: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
    let max_offset = bytes.len().saturating_sub(header_len + min_payload);

    for off in 0..=max_offset {
        let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
        let type_code = type_word & 0x3FFF;
        let bytes_to_follow = u32::from_le_bytes([
            bytes[off + 2], bytes[off + 3], bytes[off + 4], bytes[off + 5],
        ]);
        if bytes_to_follow < min_payload as u32 {
            continue;
        }
        if (bytes_to_follow as usize) > bytes.len() - off {
            continue;
        }
        // Skip GLine2d (0x3FE6) and GArc2d (0x0030) PSM type codes.
        if type_code == 0x3FE6 || type_code == 0x0030 {
            continue;
        }
        let inner_start = off + header_len;
        let vc_off = inner_start;
        if vc_off + 8 > bytes.len() {
            continue;
        }
        let vertex_count = u32::from_le_bytes([
            bytes[vc_off], bytes[vc_off + 1], bytes[vc_off + 2], bytes[vc_off + 3],
        ]);
        if !(2..=10000).contains(&vertex_count) {
            continue;
        }
        let form = bytes[vc_off + 4];
        let scope = bytes[vc_off + 5];
        if form > 6 {
            continue;
        }
        if scope > 4 && scope != 6 {
            continue;
        }
        let payload_len = 8 + (vertex_count as usize) * 16;
        if payload_len > bytes_to_follow as usize {
            continue;
        }
        let vertices_start = vc_off + 8;
        if vertices_start + (vertex_count as usize) * 16 > bytes.len() {
            continue;
        }
        // Read every vertex and check all-finite + in-domain.
        let mut all_ok = true;
        let mut any_nonzero = false;
        for i in 0..vertex_count as usize {
            let pos = vertices_start + i * 16;
            let x = f64::from_le_bytes([
                bytes[pos], bytes[pos+1], bytes[pos+2], bytes[pos+3],
                bytes[pos+4], bytes[pos+5], bytes[pos+6], bytes[pos+7],
            ]);
            let y = f64::from_le_bytes([
                bytes[pos+8], bytes[pos+9], bytes[pos+10], bytes[pos+11],
                bytes[pos+12], bytes[pos+13], bytes[pos+14], bytes[pos+15],
            ]);
            if !x.is_finite() || !y.is_finite() || x.abs() > 1e9 || y.abs() > 1e9 {
                all_ok = false;
                break;
            }
            if x.abs() > 1e-6 || y.abs() > 1e-6 {
                any_nonzero = true;
            }
        }
        if !all_ok || !any_nonzero {
            continue;
        }

        hits += 1;
        *type_code_counter.entry(type_code).or_insert(0) += 1;
        *hits_by_count.entry(vertex_count).or_insert(0) += 1;
        if hits <= 20 {
            // Print first 3 vertices as sample.
            let mut sample = String::new();
            for i in 0..vertex_count.min(3) as usize {
                let pos = vertices_start + i * 16;
                let x = f64::from_le_bytes([
                    bytes[pos], bytes[pos+1], bytes[pos+2], bytes[pos+3],
                    bytes[pos+4], bytes[pos+5], bytes[pos+6], bytes[pos+7],
                ]);
                let y = f64::from_le_bytes([
                    bytes[pos+8], bytes[pos+9], bytes[pos+10], bytes[pos+11],
                    bytes[pos+12], bytes[pos+13], bytes[pos+14], bytes[pos+15],
                ]);
                if i > 0 { sample.push_str(" "); }
                sample.push_str(&format!("({:+.4},{:+.4})", x, y));
            }
            println!(
                "HIT @ off=0x{off:06x} type=0x{type_code:04x} bytes_to_follow={bytes_to_follow} vc={vertex_count} form={form} scope={scope} v[0..3]={sample}"
            );
        }
    }
    eprintln!("total hits matching GLineString2d shape: {hits}");
    eprintln!("type_code distribution:");
    for (tc, n) in &type_code_counter {
        eprintln!("  0x{tc:04x} ({tc}): {n}");
    }
    eprintln!("vertex_count distribution (top 10):");
    let mut counts: Vec<_> = hits_by_count.iter().collect();
    counts.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (vc, n) in counts.iter().take(10) {
        eprintln!("  vc={vc}: {n}");
    }
    Ok(())
}
