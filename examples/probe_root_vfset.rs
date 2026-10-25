//! Decode the root PSMcluster0's layer records + Top ViewFilterSet via
//! resync scan (the strict chain fails on the 1.2MB root stream).

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
        // resync: candidate record = u16 type + u32 len within bounds
        let n = data.len();
        for off in 0..n.saturating_sub(6) {
            let tc = u16::from_le_bytes([data[off], data[off + 1]]);
            let len =
                u32::from_le_bytes([data[off + 2], data[off + 3], data[off + 4], data[off + 5]])
                    as usize;
            if !(6..=4096).contains(&len) || off + len > n {
                continue;
            }
            if tc == pid_parse::parsers::view_filter_sets::PSM_TYPE_CODE_VIEW_FILTER_SET {
                let payload = &data[off + 6..off + len];
                // reuse strict payload decoder by faking a single-record stream
                // is not possible; decode manually via the public struct fields
                println!(
                    "  VFSet cand @{off} len={len} oid={} sheet_ref={}",
                    u32::from_le_bytes(payload[0..4].try_into().unwrap()),
                    u32::from_le_bytes(payload[16..20].try_into().unwrap()),
                );
            }
        }
    }
}
