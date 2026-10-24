//! Count record type codes in the root PSMcluster0 of each drawing, and
//! decode the root JSheetLayer table + view filter sets.

use std::collections::BTreeMap;
use std::io::Read;

fn main() {
    let files = [
        r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图.pid",
    ];
    for file in files {
        let f = std::fs::File::open(file).unwrap();
        let mut cfb = cfb::CompoundFile::open(f).unwrap();
        let name = std::path::Path::new(file)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        println!("=== {name}");
        let mut data = Vec::new();
        cfb.open_stream("/PSMcluster0")
            .unwrap()
            .read_to_end(&mut data)
            .unwrap();
        let mut hist: BTreeMap<u16, usize> = BTreeMap::new();
        for r in pid_parse::parsers::cluster_header::resync_cluster_body_records(&data) {
            *hist.entry(r.type_code).or_default() += 1;
        }
        for (tc, c) in &hist {
            println!("   0x{tc:04X} x{c}");
        }
        let layers = pid_parse::parsers::sheet_layers::decode_sheet_layers_scattered(&data);
        println!("   JSheetLayer decoded: {}", layers.len());
        for l in &layers {
            println!(
                "     oid={} num={} name={:?} secondary={:?}",
                l.oid, l.layer_number, l.name, l.secondary_name
            );
        }
        let sets = pid_parse::parsers::view_filter_sets::decode_view_filter_sets_scattered(&data);
        println!("   VFSet decoded: {}", sets.len());
        for s in &sets {
            println!(
                "     oid={} sheet_ref={} layers={:?}",
                s.oid,
                s.sheet_ref,
                s.layers
                    .iter()
                    .map(|e| (
                        e.name.clone(),
                        e.layer_number,
                        s.is_displayed(e.layer_number)
                    ))
                    .collect::<Vec<_>>()
            );
        }
    }
}
