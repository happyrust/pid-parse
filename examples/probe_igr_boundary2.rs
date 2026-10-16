//! Full-payload dump of each distinct segment-tag boundary record in the .igr.

use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\Ref\Template Files\CPE-SQ-A0.igr";
    let mut cfb = cfb::CompoundFile::open(std::fs::File::open(path)?)?;
    let mut data = Vec::new();
    cfb.open_stream("/Sheet6")?.read_to_end(&mut data)?;
    let mut seen: std::collections::BTreeSet<u8> = Default::default();
    let mut off = 0usize;
    while off + 6 <= data.len() {
        let tw = u16::from_le_bytes([data[off], data[off + 1]]);
        if tw & 0x3FFF != 0x0013 {
            off += 1;
            continue;
        }
        let btf = u32::from_le_bytes(data[off + 2..off + 6].try_into()?) as usize;
        if !(20..=4000).contains(&btf) || off + 6 + btf > data.len() {
            off += 1;
            continue;
        }
        let p = &data[off + 6..off + 6 + btf];
        if p.len() < 34 {
            off += 1;
            continue;
        }
        let tag = p[28];
        if !seen.insert(tag) {
            off += 6 + btf;
            continue;
        }
        let oid = u32::from_le_bytes(p[0..4].try_into()?);
        println!(
            "== tag {tag:#04x} record @{off:#x} oid={oid:#x} btf={btf} n={}",
            u32::from_le_bytes(p[22..26].try_into()?)
        );
        // hex dump whole payload, 16/row with offsets
        for (i, row) in p.chunks(16).enumerate() {
            print!("   +{:03x}:", i * 16);
            for b in row {
                print!(" {b:02x}");
            }
            // ascii side: try f64 pairs for rows starting at f64 boundary
            println!();
        }
        // also f64 interpretation from +34 on (after 02 01)
        println!("   f64s from +34:");
        let mut j = 34usize;
        while j + 16 <= p.len().min(34 + 16 * 8) {
            let x = f64::from_le_bytes(p[j..j + 8].try_into()?);
            let y = f64::from_le_bytes(p[j + 8..j + 16].try_into()?);
            println!(
                "     +{j}: ({x:.5},{y:.5})  ->mm ({:.2},{:.2})",
                x * 1000.0,
                y * 1000.0
            );
            j += 16;
        }
        off += 6 + btf;
    }
    Ok(())
}
