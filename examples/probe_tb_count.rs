use std::io::Read;
fn main() {
    for arg in std::env::args().skip(1) {
        let mut cfb = cfb::CompoundFile::open(std::fs::File::open(&arg).unwrap()).unwrap();
        let paths: Vec<_> = cfb
            .walk()
            .filter(|e| e.is_stream())
            .map(|e| e.path().to_path_buf())
            .collect();
        for p in paths {
            let mut d = Vec::new();
            cfb.open_stream(&p).unwrap().read_to_end(&mut d).unwrap();
            let n = pid_parse::parsers::sheet_records::decode_igtextboxes(&d).len();
            if n > 0 {
                println!("{arg} {} -> {n}", p.display());
            }
        }
    }
}
