//! Parse the .igr boundary member blocks: mixed tags 0x67/0x68/0x74, then a
//! ~21B trailer. Report member kinds + arc f64 fields for every record.

use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\Ref\Template Files\CPE-SQ-A0.igr";
    let mut cfb = cfb::CompoundFile::open(std::fs::File::open(path)?)?;
    let mut data = Vec::new();
    cfb.open_stream("/Sheet6")?.read_to_end(&mut data)?;
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
        let oid = u32::from_le_bytes(p[0..4].try_into()?);
        let n = u32::from_le_bytes(p[22..26].try_into()?) as usize;
        // members from +28; walk tag-prefixed blocks
        let f64at = |i: usize| f64::from_le_bytes(p[i..i + 8].try_into().unwrap());
        let mut pos = 28usize;
        let mut members = Vec::new();
        let trailer_len = 21usize;
        while members.len() < n && pos < p.len().saturating_sub(trailer_len) {
            match p[pos] {
                0x67 => {
                    members.push(format!(
                        "L ({:.3},{:.3})->({:.3},{:.3})",
                        f64at(pos + 1),
                        f64at(pos + 9),
                        f64at(pos + 17),
                        f64at(pos + 25)
                    ));
                    pos += 33;
                }
                0x68 => {
                    let cnt = u32::from_le_bytes(p[pos + 1..pos + 5].try_into()?) as usize;
                    // +5..+6 = '02 01' tail, then cnt*16 bytes of xy pairs
                    members.push(format!(
                        "R n={cnt} first=({:.3},{:.3}) last=({:.3},{:.3})",
                        f64at(pos + 7),
                        f64at(pos + 15),
                        f64at(pos + 7 + 16 * (cnt - 1)),
                        f64at(pos + 15 + 16 * (cnt - 1))
                    ));
                    pos += 7 + 16 * cnt;
                }
                0x74 => {
                    let v: Vec<f64> = (0..7).map(|i| f64at(pos + 1 + 8 * i)).collect();
                    members.push(format!(
                        "A [{:.4} {:.4} {:.4} {:.4} {:.4} {:.4} {:.4}]",
                        v[0], v[1], v[2], v[3], v[4], v[5], v[6]
                    ));
                    pos += 57;
                }
                t => {
                    members.push(format!("?tag{t:#x}@{pos}"));
                    break;
                }
            }
        }
        println!(
            "@{off:#x} oid={oid:#x} n={n} btf={btf} consumed={pos} remain={}",
            p.len() - pos
        );
        for m in &members {
            println!("    {m}");
        }
        // trailer hex
        print!("    tail:");
        for b in &p[pos..] {
            print!(" {b:02x}");
        }
        println!();
        off += 6 + btf;
    }
    Ok(())
}
