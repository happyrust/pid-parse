use std::io::Read;
use std::path::PathBuf;
fn main() {
    for arg in std::env::args().skip(1) {
        let mut cfb = cfb::CompoundFile::open(std::fs::File::open(&arg).unwrap()).unwrap();
        let sheets: Vec<PathBuf> = cfb
            .walk()
            .filter(|e| e.is_stream())
            .map(|e| e.path().to_path_buf())
            .collect();
        println!("{arg}: {} streams", sheets.len());
        for path in &sheets {
            let mut data = Vec::new();
            cfb.open_stream(path)
                .unwrap()
                .read_to_end(&mut data)
                .unwrap();
            let rs = pid_parse::parsers::sheet_records::decode_igtextboxes(&data);
            if rs.is_empty() {
                continue;
            }
            println!("  {}: {} textbox(es)", path.display(), rs.len());
            for r in &rs {
                let t = r.text.replace('\n', "\\n");
                println!(
                    "    oid={} idx={} sub={} x={:.4} y={:.4} rot={:.3} text={:?}",
                    r.oid,
                    r.index,
                    r.text_sub_type,
                    r.trailing_double_1,
                    r.trailing_double_2,
                    r.rotation_rad,
                    &t[..t.len().min(60)]
                );
            }
        }
    }
}
