use cfb::CompoundFile;
use std::io::Read;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "test-file/DWG-0201GP06-01.pid".to_string());
    let mut cfb = CompoundFile::open(std::fs::File::open(&fixture)?)?;
    let mut stream = cfb.open_stream("/Sheet6")?;
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes)?;
    // Inspect a few known 0x0030 hit offsets.
    for &off in &[0x001195usize, 0x001439, 0x0015f6, 0x003676, 0x00379c] {
        let inner = off + 18;
        if inner + 64 > bytes.len() {
            continue;
        }
        println!("=== offset 0x{:06x} ===", off);
        for (label, field_off) in [
            ("center.x ", 0),
            ("center.y ", 8),
            ("axis_a.x ", 16),
            ("axis_a.y ", 24),
            ("axis_ratio (a2+32) ", 32),
            ("orient[40] byte", 40),
            ("sweep_start (a2+48)", 48),
            ("sweep_end   (a2+56)", 56),
        ] {
            let p = inner + field_off;
            if field_off == 40 {
                println!("  {label} : byte 0x{:02X} (= {})", bytes[p], bytes[p]);
            } else {
                let raw = &bytes[p..p + 8];
                let f = f64::from_le_bytes([
                    raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
                ]);
                println!(
                    "  {label} : {:?} bits=0x{:016X} f64={:+.10e}",
                    raw,
                    u64::from_le_bytes([
                        raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7]
                    ]),
                    f
                );
            }
        }
    }
    Ok(())
}
