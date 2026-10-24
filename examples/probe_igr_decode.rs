//! Verify decode_igboundaries admits the linked-template variant records
//! in CPE-SQ-A0/A1.igr and that the flattened rings close.

use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for path in [
        r"D:\work\cad\PID自研软件验证\Ref\Template Files\CPE-SQ-A0.igr",
        r"D:\work\cad\PID自研软件验证\Ref\Template Files\CPE-SQ-A1.igr",
    ] {
        let mut cfb = cfb::CompoundFile::open(std::fs::File::open(path)?)?;
        let streams: Vec<String> = cfb
            .walk()
            .filter(|e| e.is_stream())
            .map(|e| e.path().to_string_lossy().to_string())
            .filter(|p| p.contains("Sheet"))
            .collect();
        for sp in streams {
            let mut data = Vec::new();
            cfb.open_stream(&sp)?.read_to_end(&mut data)?;
            let records = pid_parse::parsers::sheet_records::decode_igboundaries(&data);
            if records.is_empty() {
                continue;
            }
            println!("{path} {sp}: {} boundaries", records.len());
            for r in &records {
                let (mut minx, mut miny, mut maxx, mut maxy) =
                    (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
                for s in &r.segments {
                    for (x, y) in [s.start, s.end] {
                        minx = minx.min(x);
                        miny = miny.min(y);
                        maxx = maxx.max(x);
                        maxy = maxy.max(y);
                    }
                }
                println!(
                    "  oid={:#x} st={:#06x} idx={} members={} segs={} closed={} bbox=({:.4},{:.4})-({:.4},{:.4})",
                    r.oid,
                    r.sub_type_word,
                    r.index,
                    r.segment_count,
                    r.segments.len(),
                    r.is_closed_loop(1e-6),
                    minx * 1000.0,
                    miny * 1000.0,
                    maxx * 1000.0,
                    maxy * 1000.0
                );
            }
        }
    }
    Ok(())
}
