use std::io::Read;
fn main() {
    for arg in std::env::args().skip(1) {
        let mut cfb = cfb::CompoundFile::open(std::fs::File::open(&arg).unwrap()).unwrap();
        let mut data = Vec::new();
        cfb.open_stream("/Sheet6")
            .unwrap()
            .read_to_end(&mut data)
            .unwrap();
        println!(
            "{arg}  Sheet6={}B  last_0x004D_end=0x33fb={}",
            data.len(),
            0x33fb
        );
    }
}
